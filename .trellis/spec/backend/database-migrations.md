# Database & Migrations

> SQLite schema is evolved only through versioned migrations in Rust.

---

## Database Location

- Runtime DB: app config dir + `codex-ai.db` (`DB_FILE_NAME` in `app/shared.rs`)
- Plugin URL: `sqlite:codex-ai.db`
- Frontend read access uses the same DB name via `src/lib/database.ts`

## Models

File: `src-tauri/src/db/models.rs`

- Table rows: `#[derive(FromRow, Serialize, Deserialize)]` structs (`Project`, `Task`, `Employee`, ...)
- Command DTOs: `Create*`, `Update*`, search/report payloads
- Some views strip secrets (e.g. `SshConfig` vs `SshConfigRecord` with password refs)

When adding columns:
1. Update the `FromRow` struct(s)
2. Update insert/update SQL
3. Update frontend `src/lib/types.ts`
4. Add a migration

## Migrations

File: `src-tauri/src/db/migrations.rs`

- `get_all_migrations() -> Vec<Migration>`
- Strictly increasing `version` integers (`1..=latest_migration_version()`; `migration_versions_are_contiguous` enforces this)
- Each entry has `description` + SQL string
- Applied at app startup by `tauri-plugin-sql` in `lib.rs`
- After adding a version, update the `latest_migration_version()` assertion in `migrations.rs` tests
- Current latest: **52** (`tasks.native_subagent_id`). Do not rewrite published versions; empty unused channels are fine.

Version 48 adds `ai_channels` + `employees.ai_channel_id`. Version 49 adds nullable `cached_tokens` on `codex_sessions` (NULL = unknown, same as v45). Version 50 adds `ai_channels.api_key` (plaintext channel config; `api_key_ref` kept only as a one-time keyring migrate hint). Version 51 adds `codex_sessions.session_origin` (`direct` | `pipeline`, default `direct`; backfill from `task_pipeline_steps.session_id`). Display type is still derived: `session_kind=review` → 审核, else `session_origin=pipeline` → 编排, else 执行. Do not extend `session_kind` with `pipeline`.

Version 52 adds nullable `tasks.native_subagent_id` (optional custom built-in-Agent sub-agent binding). Missing/NULL = employee defaults. Unknown id is rejected on create/update; a deleted catalog entry is ignored at session start.

### Adding A Migration

```rust
Migration {
    version: 41,
    description: "short imperative description",
    sql: r#"
        ALTER TABLE ...;
        CREATE TABLE ...;
    "#,
    kind: MigrationKind::Up,
},
```

Rules:
- **Never edit an already-shipped migration** that may have run on user machines.
- Prefer additive `ALTER TABLE ... ADD COLUMN` and new tables.
- Use `TEXT` timestamps compatible with `datetime('now')` / `now_sqlite()`.
- Add indexes when list filters need them (follow nearby migrations).
- Keep SQL idempotent only when safe; do not assume re-run of old versions.

### Recent Domain Example

Version 40 adds delivery management: `tasks.due_date`, `blocked_reason`, `milestone_id`, plus `milestones`, `tags`, `task_tags`, dependency tables. Follow this style for related delivery features.

Version 41 adds `idx_codex_session_events_created_at` for session-events retention purge by `created_at`.

Version 45 adds nullable token columns on `codex_sessions`: `input_tokens`, `output_tokens`, `total_tokens`, `reasoning_tokens` (all `INTEGER`, no default). **NULL means unknown** — never backfill 0. `SELECT *` / `CodexSessionRecord` / explicit INSERT fixtures must stay in sync; omitting the columns on INSERT is correct (they stay NULL until the first usage event).

Version 46 adds `task_run_queue` (unique `task_id`, JSON payload, FIFO drain). Gate/replay contracts live in [run-queue.md](./run-queue.md) — do not treat it as a kanban status.

Version 47 adds `task_templates` (soft-delete, optional `project_id`, `tags_json` names + `subtasks_json`). Apply is a separate transaction in `app/templates.rs` — see [task-templates.md](./task-templates.md).

## Scenario: Session token columns (v45)

### 1. Scope / Trigger
- New persisted telemetry on shared `codex_sessions` (all engines). Cross-layer: migration + `FromRow` + insert lists + dashboard/task SUM commands.

### 2. Signatures
- DDL: `ALTER TABLE codex_sessions ADD COLUMN {input,output,total,reasoning}_tokens INTEGER;`
- Persist: `apply_codex_session_usage(pool, session_record_id, &UsageDelta) -> Result<(), String>`
- Read: `get_task_token_usage(task_id) -> TokenUsageSummary`
- v49: `ALTER TABLE codex_sessions ADD COLUMN cached_tokens INTEGER;` (cache **hit** only; not cache-creation)

### 3. Contracts
- Columns nullable. First delta for a field uses `COALESCE(col, 0) + delta`; a NULL delta leaves that column unchanged.
- Empty `UsageDelta` is a no-op (does not write zeros).
- Token telemetry is **not** an activity-log domain event (would flood logs per stream tick).

### 4. Validation & Error Matrix
| Case | Behavior |
|------|----------|
| Empty delta | `Ok(())`, row stays NULL |
| Unknown session id | UPDATE affects 0 rows; caller currently ignores (`let _ = apply_...`) like other stdout persist paths |
| Parse miss | leave columns NULL; do not store 0 |

### 5. Good / Base / Bad Cases
- Good: first usage writes 812/45; second usage adds; NULL fields stay NULL until seen
- Base: session never emits usage → token columns stay NULL; UI shows 未知, not 0
- Bad: `DEFAULT 0` on the migration; `SUM` treating NULL sessions as 0 in a “has usage” empty state

### 6. Tests Required
- `latest_migration_version` / contiguous versions
- Pool test: NULL until first delta, then accumulate (`app/tests/sql_and_session.rs`)

### 7. Wrong vs Correct
#### Wrong
```sql
ALTER TABLE codex_sessions ADD COLUMN total_tokens INTEGER NOT NULL DEFAULT 0;
```
#### Correct
```sql
ALTER TABLE codex_sessions ADD COLUMN total_tokens INTEGER; -- NULL = unknown
```

### Session events retention (C2)

- Policy file: `$APPCONFIG/session-events-policy.json` — `{ "retention_days": 30 }` (default 30, valid **1..=3650**, else default).
- Purge only `codex_session_events` where `created_at < datetime('now', '-N days')`; never delete `codex_sessions` for retention.
- Commands: `get_session_events_policy`, `update_session_events_policy`, `get_session_events_stats`, `purge_session_events`.
- Manual purge: DELETE + VACUUM; startup purge: DELETE only (best-effort).
- Activity action: `session_events_purged` + frontend Chinese label「清理会话事件」.

## Soft Delete

Projects and tasks use `deleted_at`:
- Active fetch helpers filter `deleted_at IS NULL`
- Delete commands set timestamp
- Restore clears it (projects restore also restores related tasks)
- Permanent delete physically removes rows after trash flows

New entities should only use soft delete if trash UX is intentionally supported.

## Key Relational Constraints

- Tasks always belong to a project (`project_id`)
- Milestones/tags are project-scoped
- Employee membership is `employees.project_id` only
- Sessions/events live in `codex_sessions` / `codex_session_events` (shared across engines historically named “codex_”)

## Backup / Restore

Commands in `app/database.rs`:
- `backup_database`
- `restore_database`
- `get_database_backup_scope`
- `open_database_folder`

Sanitize/validate backup scripts carefully (`sanitize_sql_backup_script` helpers exist for tests/safety).

## Anti-Patterns

- Ad-hoc `CREATE TABLE` at runtime outside migrations.
- Frontend-driven schema changes.
- Reusing migration version numbers.
- Storing large binary blobs in SQLite when filesystem attachments already exist.
