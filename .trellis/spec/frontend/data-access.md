# Data Access

> Frontend data enters through **one door only**: Tauri **commands** via `src/lib/backend.ts`.

---

## Architecture

```text
Component / Page
    ↓
Zustand store (orchestration)
    ↓
backend.ts typed invoke wrappers
    ↓
Rust Tauri commands (read + write)
    ↓
SQLite (sqlx)
```

Frontend **must not** open SQLite directly. `sql:allow-select` is removed from
`src-tauri/capabilities/default.json`. `src/lib/database.ts` is a hard-fail stub
(`select` / `execute` / `getDb` all throw).

## IPC Bridge (`src/lib/backend.ts`)

All command access should go through this module.

Patterns:
1. Import `invoke` only here.
2. Export `async function` wrappers with typed inputs/outputs from `@/lib/types` (or local DTOs).
3. Normalize backend quirks at the boundary (execution target, artifact mode, SSH password flags).
4. Use camelCase keys for invoke args when matching Tauri's serde rename expectations already used in the file (`projectId`, nested `{ payload }` with snake_case fields, etc.).

### Core read commands (post read-path migration)

| Domain | Wrappers |
|--------|----------|
| Projects | `listProjects`, `listTrashedProjects` |
| Employees | `listEmployees`, `listEmployeeMetrics` |
| Tasks | `listTasks`, `listTaskAttachments`, `listTaskSubtasks`, `listTaskComments`, `listTrashedTasks` |
| Activity | `listActivityLogs` |
| Dashboard | `getDashboardStats`, `getDashboardReportSummary` |
| Notifications | `listNotifications` |

`listTasks` rules:
- With `projectId` (or non-empty `projectIds`): full active list for that filter (no forced LIMIT)
- Without project scope: server default **LIMIT 500** (max **1000**) + optional `offset`

`listActivityLogs`:
- Supports task/action filters, environment/ssh scope, keyword (+ frontend `matchedActions` / `matchedStatuses` for Chinese label reverse-match), date range, pagination
- `includeTotal` returns `total` + scope-only `available_actions`

When adding a new Rust command:
1. Register it in `src-tauri/src/lib.rs` generate_handler.
2. Add a typed wrapper in `backend.ts`.
3. Call the wrapper from a store or feature hook — not from random JSX with raw `invoke`.

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
- `src/lib/opencode.ts` — includes remote runtime wrappers (`validateRemoteOpenCodeHealth`, `installRemoteOpenCodeSdk`)
- `src/lib/grok.ts`
- `src/lib/ai.ts` — higher-level AI helpers

Use these for event subscription and engine-facing UX; keep DB mutations on command wrappers.

A new engine health/install command belongs in its engine client, **not** `backend.ts`, and its result type belongs next to the wrapper (e.g. `RemoteOpenCodeHealthCheck`). The settings page imports from the engine client so the runtime tab stays one screen per engine.

## Attachments / Files

- Dialog file picks go through `@tauri-apps/plugin-dialog` helpers in `src/lib/taskAttachments.ts`.
- Opening/reading attachments uses backend commands (`openTaskAttachment`, `readImageFile`).
- SSH projects may sync image attachments remotely; UI must tolerate remote limitations messaging already shown in SSH mode banner.

## Soft Delete

Active list commands filter `deleted_at IS NULL`. Trash APIs (`listTrashed*`) intentionally return soft-deleted rows.

## Anti-Patterns

- Importing `@tauri-apps/api/core` `invoke` outside `backend.ts` / engine client modules.
- Using frontend SQL for reads or writes (`database.select` / `execute`).
- Re-adding `sql:allow-select` to capabilities without a documented exception.
- Assuming SSH artifact capture equals local full diffs (`ArtifactCaptureMode` exists for a reason).
- Forgetting trash/soft-delete filters in new list queries.
