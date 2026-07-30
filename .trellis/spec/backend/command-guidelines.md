# Command Guidelines

> How Tauri commands are structured in this codebase.

---

## Canonical Shape

```rust
#[tauri::command]
pub async fn create_project<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateProject,
) -> Result<Project, String> {
    let pool = sqlite_pool(&app).await?;
    // validate → build model → execute SQL → return fresh row
    fetch_project_by_id(&pool, &project.id).await
}
```

Reference commands:
- `create_project` / `update_project` — `src-tauri/src/app/projects.rs`
- `create_task` — `src-tauri/src/app/tasks.rs`
- `log_activity` — `src-tauri/src/app/sessions.rs`

## Conventions

1. **Generic runtime** — most commands take `app: AppHandle<R>` with `R: Runtime`.
2. **Return type** — `Result<T, String>` (UI shows the string).
3. **Payload structs** — defined in `db/models.rs` (`CreateX`, `UpdateX`, specialized payloads).
4. **IDs** — `new_id()` (UUID) from shared helpers.
5. **Timestamps** — `now_sqlite()` formatted as `%Y-%m-%d %H:%M:%S`.
6. **Optional text** — normalize with `normalize_optional_text` (trim empty → `None`).
7. **Existence checks** — `ensure_project_exists`, `fetch_*_by_id`, SSH config ensure helpers.
8. **Soft delete** — set `deleted_at`; list queries filter it unless trash APIs.

## Validation Order (preferred)

1. Resolve DB pool (`sqlite_pool`).
2. Validate parent entities (project/employee/ssh config/milestone).
3. Normalize enums (`normalize_project_type`, auth types, automation mode).
4. Enforce product rules (e.g. automation default requires reviewer).
5. Begin transaction if multi-row or file+db coupling.
6. Persist.
7. Side effects (activity log, remote upload, notifications) with cleanup on failure.
8. Return re-fetched model when useful.

## Transactions & Compensating Cleanup

`create_task` is the reference for mixed DB + filesystem + SSH side effects:
- begin transaction
- insert task (+ automation state)
- build local attachments
- sync remote images for SSH projects
- on failure: rollback, delete local attachment files, cleanup remote uploads

Do not leave orphan files when DB insert fails.

## Update DTOs (`Option<Option<T>>`)

Nullable fields that can be cleared use:

```rust
#[serde(default, deserialize_with = "deserialize_explicit_nullable")]
pub description: Option<Option<String>>,
```

Meaning:
- field missing → leave unchanged
- field `null` → set SQL NULL
- field value → set value

Mirror this carefully when adding update commands; partial `Option<T>` alone cannot express clear-vs-omit.

## SQL Style

- Prefer `sqlx::query` / `query_as` / `query_scalar` with `$1` binds.
- Use `QueryBuilder` for dynamic UPDATE sets (see `employees.rs` / task updates).
- Keep `deleted_at IS NULL` on active-entity fetches.
- After insert/update, re-fetch with `query_as` when returning entities to UI.

## Activity Logs

For meaningful domain events, call `insert_activity_log` with a stable snake_case `action` and human `details`.

Frontend dashboard maps actions to Chinese via `getActivityActionLabel`. When adding a new action key, update that map too (cross-layer requirement).

## Git High-Risk Commands

Git mutations that can destroy history use request/confirm/cancel:
- `request_git_action`
- `confirm_git_action`
- `cancel_git_action`

Do not short-circuit this for merge/push/rebase/cherry-pick/stash/cleanup from new UI.

## Working Directory Validation

Before starting engine sessions, validate cwd with `validate_runtime_working_dir` (exists, directory, accessible, `.git` present). Used across codex/claude/opencode process modules.

## Anti-Patterns

- Returning rich error enums the frontend does not understand (stick to `String` unless introducing a coordinated typed error channel).
- Accepting unvalidated filesystem paths.
- Writing secrets into SQLite plain columns (SSH passwords use secret refs; at-rest values in OS keychain via `codex/secret_store`, not JSON `value` fields).
- Performing long remote operations without clear error strings and cleanup.
