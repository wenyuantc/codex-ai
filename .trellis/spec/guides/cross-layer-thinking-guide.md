# Cross-Layer Thinking Guide

> Map Codex AI’s real boundaries before implementing multi-layer changes.

---

## The Layers

```text
UI Component / Page
  → Zustand store (cache + orchestration)
    → src/lib/backend.ts (invoke wrappers + normalization)
      → #[tauri::command] Rust (validate + read/mutate)
        → SQLite / filesystem / SSH / AI process
      ← serde models / Result<String errors>
    ← store refresh via list/get commands (or returned entity)
  ← render helpers (formatDate, labels, status colors)
```

Most bugs sit on the arrows, not inside a single file.

---

## Step 1: Draw The Flow For Your Change

For the feature, write the path of one successful request and one failure:

1. Who triggers it? (button, drag-drop, automation, tray)
2. Which store action?
3. Which `backend.ts` function + invoke name?
4. Which Rust command module?
5. Which tables/files/remote hosts mutate?
6. What activity log / notification / engine event fires?
7. How does UI refresh?

If any step is “unknown”, stop and find the existing closest flow (`create_task`, `update_project`, session start, git confirm).

---

## Step 2: Contracts To Keep Aligned

| Boundary | Contract | Reference |
|----------|----------|-----------|
| Command DTO ↔ TS entity | snake_case field names | `src/lib/types.ts`, `db/models.rs` |
| invoke args ↔ command params | existing camelCase/payload style in `backend.ts` | `create_task` + `CreateTask` |
| Update omit vs null | `Option<Option<T>>` + explicit null deserializer | `UpdateTask`, `UpdateProject` |
| Soft delete | `deleted_at` filters on active lists | projects/tasks fetch helpers |
| Activity action | stable snake_case key + Chinese label | `insert_activity_log`, `getActivityActionLabel` |
| Execution target | `local` / `ssh` + artifact mode | shared constants, SSH banner |
| AI provider | `ai_provider` union + start/stop/label/one-shot branches for **every** engine | `types.ts` `AiProvider`, engine `src/lib/*.ts`, `task_automation`, settings normalize |
| Employee membership | only `employees.project_id` | employees commands + CLAUDE/README |
| Schema | migration version bump | `db/migrations.rs` |
| Permissions | frontend cannot SQL-read or SQL-write | `capabilities/default.json`, `database.ts` (hard-fail stub) |

---

## Step 3: Local + SSH Dual Path

Ask explicitly:

- Does this feature read a local git worktree path?
- Does it upload/read attachments?
- Does it start an AI session?
- Does it capture diffs/artifacts?

If yes, check the SSH path:
- project `project_type == "ssh"`
- `ssh_config_id` / remote repo path validation
- remote runtime health / password probe gates
- artifact capture limitations (`ssh_git_status`, `ssh_none`)
- for a new AI engine: remote spawn + one-shot + health (or explicit out-of-scope), not only local start

Do not ship a local-only happy path for a domain that already has remote execution.

When touching `ai_provider` branches, search the whole tree for `claude`/`opencode` ternaries and `SUPPORTED_ONE_SHOT_PROVIDERS` — “else Codex” silently breaks new providers like Grok.

**Lifting an execution-target restriction** (e.g. “engine X does not support SSH”) is never a one-file change. The gate is usually mirrored in 5+ places — grep the engine name and audit each before declaring done:

- session launch early-return in `<engine>/process/mod.rs`
- one-shot dispatch in `codex/process/one_shot.rs`
- `normalize_one_shot_provider` `is_remote` gate in `codex/settings.rs`
- `resolve_remote_one_shot_runtime` status branch in `app/remote.rs`
- coordinator/tester refusals in `codex/process/ai_commands.rs`
- frontend settings/runtime entry + any `getActivityActionLabel()` key
- the spec sentence that documented the restriction (`ai-engines.md` / `ssh-remote.md`)

A stale “not supported” left in any one of them re-blocks the feature or lies to the next session.

---

## Step 4: Failure & Cleanup

For multi-step commands:

```text
DB transaction? → file writes? → remote uploads? → engine spawn?
```

On error, which of those must roll back?
Follow `create_task` attachment compensation and git confirm flows.

For UI:
- command `Err(String)` should be visible
- stores should not mark success if invoke threw
- listeners must still clean up

---

## Common Cross-Layer Mistakes (This Repo)

### 1. Frontend SQL read or write

**Bad**: calling plugin `select` / `execute` from UI  
**Good**: add/use a Tauri command via `backend.ts`; keep `database.ts` hard-fail

### 2. New DB field only in UI

**Bad**: add to `types.ts` and a form  
**Good**: migration → Rust model → command SQL → types → UI

### 3. Activity without label

**Bad**: log `foo_bar_created` only in Rust  
**Good**: also add Chinese label in `getActivityActionLabel`

### 4. Null-clear bugs

**Bad**: `description: Option<String>` update cannot distinguish omit vs clear  
**Good**: explicit nullable deserializer pattern already used in models

### 5. Assuming full SSH artifacts

**Bad**: session change UI assumes local_full diffs  
**Good**: honor `artifact_capture_mode` and limited-mode messaging

### 6. Employee project denormalization

**Bad**: store project membership on tasks/sessions as a second source of truth for employment  
**Good**: keep `employees.project_id` authoritative for membership; other tables reference employees by id

### 7. Date display drift

**Bad**: `new Date(x).toLocaleString()` in one page  
**Good**: `formatDate()` everywhere

---

## Checklist Before Implementation

- [ ] Identified owning frontend store + backend module
- [ ] Listed tables/files/events touched
- [ ] Migration needed? version planned?
- [ ] Command + `backend.ts` wrapper planned together
- [ ] SSH implications listed
- [ ] Activity action + label planned
- [ ] Tests: pure helper unit test and/or `app/tests` case
- [ ] Validation commands: `npm run build` and/or `cargo test --manifest-path src-tauri/Cargo.toml`

---

## Checklist After Implementation

- [ ] Active list queries still filter soft-deleted rows
- [ ] Trash/restore paths still coherent if delete semantics changed
- [ ] Dashboard/activity shows Chinese text for new actions
- [ ] No new direct `invoke` outside bridge modules
- [ ] No secrets logged
