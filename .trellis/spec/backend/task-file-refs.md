# Task Project File Refs

> Relative repo paths bound to a task. Distinct from copied `task_attachments`.

## 1. Scope / Trigger

Use this spec when changing `task_file_refs`, `list_project_files`, create/detail file-ref UI, or AI prompt injection for those paths.

Table: migration **v53**. Paths are relative to the project repo root. Files are **not** copied. Local and SSH listing both go through `git_runtime` + `git_bridge` `list_files` — do not open a system file dialog and do not invent a parallel SSH `find`.

## 2. Signatures

```rust
async fn list_project_files(app, project_id, query: Option<String>, limit: Option<usize>) -> Result<Vec<String>, String>
async fn list_task_file_refs(app, task_id) -> Result<Vec<TaskFileRef>, String>
async fn add_task_file_refs(app, task_id, paths: Vec<String>) -> Result<Vec<TaskFileRef>, String>
async fn delete_task_file_ref(app, id) -> Result<(), String>
```

`CreateTask.file_ref_paths: Option<Vec<String>>` is inserted in the same `create_task` transaction as attachments.

`TaskFileRef`: `id`, `task_id`, `relative_path`, `sort_order`, `created_at`. Unique `(task_id, relative_path)`. `ON DELETE CASCADE` from `tasks`.

## 3. Contracts

- Path normalize: trim, `\` → `/`, strip `./` and empty/`.` segments. Store posix relative path.
- Reject: empty, absolute (`/` or `X:`), any `..` segment, NUL.
- Duplicate path on add is a no-op (return existing list).
- Listing: `git ls-files -z` + untracked `--others --exclude-standard`; case-insensitive substring; default 200, max 500.
- Prompt: inject relative paths only (never file contents) via `buildTaskExecutionInput` **and** Rust coordinator-plan / automation-fix builders. Missing files at run time still go into the prompt.
- Activity: `task_file_refs_added`, `task_file_ref_removed`; `task_created` details may include `含 N 个项目文件引用`. Both locale packs required.
- UI: picker disabled until a project is selected; changing project clears draft refs (same as tags/deps).
- SSH: same picker; `list_project_files` uses remote `repo_path` + git-bridge. Worktree tasks still use relative paths.

## 4. Validation & Error Matrix

| Condition | Behavior |
|---|---|
| Empty / whitespace path | `项目文件引用路径不能为空` |
| `..` segment | `项目文件引用不能包含上级目录` |
| Absolute path | `项目文件引用必须是相对仓库根的路径` |
| Duplicate on add | skip insert, return current list |
| Missing task id | task not found |
| SSH git-bridge timeout | existing 12s remote git error |

## 5. Good / Base / Bad Cases

- **Good**: select project → search `src/lib/taskPrompt.ts` → create → run prompt contains `项目文件引用`.
- **Base**: no refs → prompt omits the section; attachments unchanged.
- **Bad**: OS file dialog of arbitrary local files; inlining file contents; local-only listing that leaves SSH picker empty.

## 6. Tests Required

- `normalize_task_file_ref_path`: posix, backslash, reject `..` / absolute / empty.
- `build_automation_fix_prompt` includes the file-ref section.
- `latest_migration_version() == 53` and contiguous versions.
- Frontend: `buildTaskExecutionInput` file-ref block; `getActivityActionLabel("task_file_refs_added"|"task_file_ref_removed")` in zh-CN + en.

## 7. Wrong vs Correct

#### Wrong
Copy files into `task-attachments/` or dump file bodies into the prompt. Use `@tauri-apps/plugin-dialog` so SSH projects cannot pick remote files.

#### Correct
Store relative paths in `task_file_refs`. List via `list_project_files` (git-bridge, local + SSH). Inject paths in every `buildTaskExecutionInput` call site plus Rust plan/fix prompts.
