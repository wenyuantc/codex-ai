# Design · 任务项目文件引用

## Architecture

引用是任务级元数据：相对仓库根的路径列表。不拷贝文件，不走附件目录。

```
UI picker (CreateTaskDialog / TaskCollaborationPanel)
  → list_project_files(projectId, query)
    → git_runtime + git_bridge list_files (local | SSH)
  → create_task.file_ref_paths | add_task_file_refs | delete_task_file_ref
    → SQLite task_file_refs + activity_log
  → 运行：taskStore.fileRefs → buildTaskExecutionInput → start_* prompt
  → 协调员计划 / 自动修复：Rust fetch_task_file_refs → 提示词段落
```

## Schema (migration 53)

```sql
CREATE TABLE task_file_refs (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  relative_path TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX idx_task_file_refs_task_path ON task_file_refs(task_id, relative_path);
CREATE INDEX idx_task_file_refs_task_sort ON task_file_refs(task_id, sort_order, created_at);
```

## Path contract

- Normalize: trim, backslash → `/`, strip leading `./`
- Reject: empty, absolute (`/` or `X:`), any `..` segment, NUL
- Store posix relative path
- Duplicate path on same task is no-op (unique index)

## Commands

| Command | Owner | Notes |
|---|---|---|
| `list_project_files` | `git_workflow` | `{ projectId, query? }` → relative paths, cap 200 |
| `list_task_file_refs` | `app/tasks.rs` | ordered by sort_order |
| `add_task_file_refs` | `app/tasks.rs` | validate + insert + activity |
| `delete_task_file_ref` | `app/tasks.rs` | by id + activity |
| `create_task` | extend | `file_ref_paths?: string[]` inserted in same tx as attachments |

`list_project_files` uses existing project resolve (local `repo_path` / SSH `remote_repo_path` + `ssh_config_id`) and git-bridge command `list_files`:

- `git ls-files -z` + `git ls-files -z --others --exclude-standard`
- substring filter (case-insensitive)
- limit 200

Do not invent a parallel SSH `find`.

## Prompt injection

Shared copy (zh, matches current prompt language):

```
项目文件引用:
1. src/foo.ts
2. src/bar.rs

说明：以上路径相对项目仓库根目录。请优先阅读这些文件再动手。
```

Frontend: `buildTaskExecutionInput({ fileRefs })`. All call sites must pass store refs after `fetchFileRefs`.

Rust: coordinator plan (`ai_commands.rs`) and automation fix prompt (`task_automation/prompt.rs`) fetch refs the same way they fetch attachments.

Missing files at run time: still include the path.

## Frontend

- Type `TaskFileRef` in `src/lib/types.ts`
- Store cache `fileRefs: Record<string, TaskFileRef[]>` mirroring attachments
- Shared picker: `src/components/tasks/ProjectFileRefPicker.tsx` (Dialog + search + multi-select)
- Create dialog: chips + picker; disabled until `selectedProjectId`; clear on project change
- Detail: `TaskCollaborationPanel` sibling of attachments
- i18n: `tasks` + `activity` zh-CN/en
- Tests: `taskPrompt` file-ref section; locale/activity keys

## Compatibility

- SSH: listing via git-bridge remote path; stored paths still relative, so worktree/SSH execution cwd is enough
- Worktree: relative paths apply to the task worktree copy
- Soft delete task: rows remain until permanent delete CASCADE
- Templates / JSON import: out of scope

## Rollback

Drop is not provided (migrations are additive). Feature is unused if UI unused; table can remain.
