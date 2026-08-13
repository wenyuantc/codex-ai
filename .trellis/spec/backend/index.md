# Backend Development Guidelines

> Tauri v2 + Rust 2021 + SQLx/SQLite service layer for Codex AI.

---

## Stack Snapshot

| Piece | Local choice |
|-------|--------------|
| Shell | Tauri 2 (`src-tauri/`) |
| Async | Tokio |
| DB | SQLite via `tauri-plugin-sql` + SQLx 0.8 |
| IPC | `#[tauri::command]` functions registered in `src-tauri/src/lib.rs` |
| Models | `src-tauri/src/db/models.rs` |
| Migrations | Inline versioned list in `src-tauri/src/db/migrations.rs` |
| AI engines | `codex/`, `claude/`, `opencode/`, `grok/` managers + process submodules |
| Git | `git_workflow/` (domain slices) + `git_runtime.rs` |
| Automation | `task_automation` (+ `prompt` submodule and domain slices) |

## Hard Rules

1. **Business reads and writes happen in Rust commands**, not in the webview SQL plugin.
2. **Capabilities** (`src-tauri/capabilities/default.json`) do **not** grant `sql:allow-select` or `sql:allow-execute`. Frontend `database.ts` is a hard-fail stub; all data access goes through Tauri commands.
3. **`employees.project_id` is the single source of truth** for employee↔project membership. Do not denormalize membership elsewhere.
4. **Schema changes require a new migration version** in `get_all_migrations()`.
5. **Local + SSH execution targets** must both be considered for sessions, attachments, git, and health checks.
6. **User-facing errors** are often Chinese strings returned as `Result<T, String>`.
7. **Activity logs** should be written for meaningful state changes (`insert_activity_log` / `log_activity`).

## Guidelines Index

| Guide | Description |
|-------|-------------|
| [Directory Structure](./directory-structure.md) | Module layout and ownership |
| [Command Guidelines](./command-guidelines.md) | Tauri command shape, validation, transactions |
| [Database & Migrations](./database-migrations.md) | Models, SQL style, migration rules |
| [Error Handling](./error-handling.md) | `Result<T, String>`, notifications, cleanup |
| [AI Engines](./ai-engines.md) | Codex/Claude/OpenCode/Grok process lifecycle and per-engine contracts |
| [SSH Remote](./ssh-remote.md) | `build_ssh_command` multiplexing, askpass env, quit cleanup, remote SDK runtimes |
| [Testing](./testing.md) | Integration/unit test patterns |
| [Dashboard Report](./dashboard-report.md) | `get_dashboard_report_summary` R1 trend/milestone + token usage contract |

## Data Flow

```text
UI invoke(command, args)
  → #[tauri::command] in app/* or git_workflow / task_automation / tray
  → validate + authorize against SQLite / filesystem / SSH
  → mutate via SQLx (often in a transaction)
  → optional activity log + notifications + engine side effects
  → return serde models to UI
```

## Primary References

- Entry / handler list: `src-tauri/src/lib.rs`
- App modules: `src-tauri/src/app/mod.rs`
- Shared helpers/constants: `src-tauri/src/app/shared.rs`
- Models: `src-tauri/src/db/models.rs`
- Migrations: `src-tauri/src/db/migrations.rs`
- Tests: `src-tauri/src/app/tests/*`
