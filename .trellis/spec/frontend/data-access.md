# Data Access

> Frontend data enters through two doors only: SQL **select** and Tauri **commands**.

---

## Architecture

```text
Component / Page
    ↓
Zustand store (orchestration)
    ↓                    ↓
database.select()    backend.ts invoke wrappers
    ↓                    ↓
SQLite (read)        Rust Tauri commands (read/write)
```

## SQL Select

File: `src/lib/database.ts`

- DB name: `sqlite:codex-ai.db`
- Singleton load via `getDb()`
- Use parameterized queries (`$1`, `$2`, ...)

Typical store read:

```ts
const tasks = await select<Task>(
  "SELECT * FROM tasks WHERE project_id = $1 AND deleted_at IS NULL ORDER BY updated_at DESC",
  [projectId],
);
```

Rules:
- Soft-deleted rows: filter `deleted_at IS NULL` unless implementing trash views.
- Prefer store-owned queries over ad-hoc selects in deep leaf components.
- Join queries are fine for dashboard/activity (see `dashboardStore.ts`).

## IPC Bridge (`src/lib/backend.ts`)

All command access should go through this module (~130+ wrappers).

Patterns:
1. Import `invoke` only here.
2. Export `async function` wrappers with typed inputs/outputs from `@/lib/types`.
3. Normalize backend quirks at the boundary (execution target, artifact mode, SSH password flags).
4. Use camelCase keys for invoke args when matching Tauri's serde rename expectations already used in the file (`projectId`, `destinationPath`, or nested `{ payload }`).

Examples:
- `createTask` / `updateTask` / `updateTaskStatus`
- `logActivity`
- `listSshConfigs`, git workflow helpers, session helpers, MCP settings

When adding a new Rust command:
1. Register it in `src-tauri/src/lib.rs` generate_handler.
2. Add a typed wrapper in `backend.ts`.
3. Call the wrapper from a store or feature hook — not from random JSX.

## Normalization Boundary

Backend payloads are not always perfectly shaped for UI unions. Normalize in `backend.ts` (or a small lib helper) before stores/UI see them.

Existing normalizers:
- `normalizeExecutionTarget`
- `normalizeArtifactCaptureMode`
- `normalizeHealthCheck`
- `normalizeSshConfig`
- `normalizeProject` in `src/lib/projects.ts`

Do not scatter `value === "ssh" ? ...` fixes across pages.

## Activity Logging

User-visible side effects should often write activity logs.

- Frontend entry: `logActivity({ action, details, employee_id?, task_id?, project_id? })`
- Backend entry: many commands log themselves via `insert_activity_log`
- Dashboard labels: **always** add Chinese mapping in `getActivityActionLabel()` (`src/lib/utils.ts`) when introducing a new `action` key

If the dashboard would show a raw snake_case key, the label map is incomplete.

## Engine Clients

Besides `backend.ts`, engine-specific helpers live in:
- `src/lib/codex.ts` — listen helpers (`onCodexOutput`, `onCodexSession`, ...)
- `src/lib/claude.ts`
- `src/lib/opencode.ts`
- `src/lib/ai.ts` — higher-level AI helpers

Use these for event subscription and engine-facing UX; keep DB mutations on command wrappers.

## Attachments / Files

- Dialog file picks go through `@tauri-apps/plugin-dialog` helpers in `src/lib/taskAttachments.ts`.
- Opening/reading attachments uses backend commands (`openTaskAttachment`, `readImageFile`).
- SSH projects may sync image attachments remotely; UI must tolerate remote limitations messaging already shown in SSH mode banner.

## Anti-Patterns

- Importing `@tauri-apps/api/core` `invoke` outside `backend.ts` / engine client modules.
- Using frontend SQL for writes, bulk updates, or schema changes.
- Assuming SSH artifact capture equals local full diffs (`ArtifactCaptureMode` exists for a reason).
- Forgetting trash/soft-delete filters in new list queries.
