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
- Strictly increasing `version` integers (currently through **40**)
- Each entry has `description` + SQL string
- Applied at app startup by `tauri-plugin-sql` in `lib.rs`

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
