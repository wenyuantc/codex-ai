# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Frontend dev server only (no Rust backend)
npm run dev

# Full Tauri dev environment (frontend + backend)
npm run tauri:dev

# Build (TypeScript check + Vite bundle)
npm run build

# Bump app version across package.json / Cargo.toml / tauri.conf.json
npm run bump-version

# Rust backend tests (only test suite in the project)
cargo test --manifest-path src-tauri/Cargo.toml

# Run a single test
cargo test --manifest-path src-tauri/Cargo.toml <test_name>

# Desktop packaging
npm run tauri:dmg:no-sign  # macOS (unsigned)
npm run tauri:linux        # Linux (AppImage/deb/rpm)
npm run tauri:windows      # Windows (NSIS/MSI)
```

There is no linting configured (no ESLint, Clippy rules, or Prettier).

## Architecture

**Codex AI** is a Tauri v2 desktop app — a project/task manager that drives four AI coding CLIs (Codex, Claude, Grok, OpenCode) as "AI employees", with Git workflow, code review, and task automation on top. Local and SSH-remote projects are both first-class. The data flow is strictly:

```
React (UI) → Tauri IPC commands → Rust service layer → SQLite
```

**All business writes go through Rust Tauri commands.** The frontend never writes directly to the database. Zustand stores only cache frontend state fetched from Rust.

### Frontend (`src/`)

- React 19 + TypeScript + Vite (dev port 1420) + TailwindCSS 4
- **6 Zustand stores**: `project`, `task`, `employee`, `dashboard`, `notification`, `log`
- **8 pages / routes** (`src/App.tsx`): `/` Dashboard, `/projects`, `/projects/:id`, `/kanban`, `/sessions`, `/employees`, `/settings`, `/trash`
- **Path alias**: `@/*` maps to `src/*`
- Component folders are feature-organized: `ai/`, `codex/`, `dashboard/`, `employees/`, `git/`, `keyboard/`, `layout/`, `projects/`, `search/`, `sessions/`, `settings/`, `tasks/`, `trash/`, `ui/`
- Keyboard shortcuts are declared in one place: `src/lib/shortcuts.ts` (`NAV_SHORTCUTS` ⌘1–⌘7, `GLOBAL_SHORTCUTS` ⌘K/⌘T/⌘B/?, `PAGE_SHORTCUTS` N/A/R). Add new shortcuts there, not inline.

**Frontend never touches SQLite.** All reads and writes go through Tauri commands via `src/lib/backend.ts`. `src/lib/database.ts` is a hard-fail stub (`select` / `execute` / `getDb` throw). `capabilities/default.json` does not grant `sql:allow-select` or `sql:allow-execute` (only `sql:default` remains for plugin registration if needed).

### Backend (`src-tauri/src/`)

- Rust 2021, Tokio async, SQLx 0.8 (compile-time checked queries)
- Entry: `lib.rs` → `pub fn run()` registers plugins, the four engine managers, tray, and window restoration
- **168 Tauri commands**, all registered in the single `invoke_handler!` list in `lib.rs`. Largest sources:
  - `git_workflow.rs` (43), `app/tasks.rs` (18), `app/delivery.rs` (12), `app/remote.rs` (10)
  - `codex/process/ai_commands.rs` (9), `app/sessions.rs` (9), `app/projects.rs` (7), `app/database.rs` (7), `app/employees.rs` (6)
- `app/` submodules: `projects`, `employees`, `tasks`, `delivery`, `sessions`, `review`, `remote`, `database`, `shared`
- `db/migrations.rs` — versioned DDL, **80 migrations** inline
- `db/models.rs` — SQLx `query_as!` type definitions for all tables
- `task_automation.rs` — task state machine (2,976 lines; `task_automation/prompt.rs` is the only extracted piece)
- `git_workflow.rs` — Git operations (5,515 lines, the largest file in the repo)
- `git_runtime.rs` — low-level git process execution (local + SSH)
- `process_spawn.rs` — shared process spawning; engines are launched from Rust, never via the frontend shell plugin
- `notifications.rs` — event notification system
- `tray.rs`, `window_state.rs`, `window_event.rs` — tray, window size persistence, close-to-tray

### AI engines (`src-tauri/src/{codex,claude,grok,opencode}/`)

Four engines, each with the same internal shape — `manager.rs` + `settings.rs` + `process/{mod,lifecycle,session_runtime,stream,context}.rs`:

| Engine | LOC | Capabilities (`app/database.rs::get_ai_provider_capabilities`) |
|---|---|---|
| `codex/` | 12.3k | start / stop / restart / send_input / resume — the only fully-featured engine |
| `grok/` | 3.4k | start / stop / resume |
| `claude/` | 3.3k | start / stop / resume |
| `opencode/` | 3.2k | start / stop / resume (SDK server spawned at startup from `lib.rs`) |

**There is no shared engine trait** — the codebase has zero `pub trait` declarations. `claude/` and `grok/` are near-copies (`manager.rs` and `process/lifecycle.rs` are 100% identical modulo the engine name; `session_runtime.rs` ~92%); only `process/stream.rs` genuinely differs, since each CLI has its own output protocol. When fixing a session-lifecycle bug, check whether the same bug exists in the other three engines. Codex-only extras: `cli.rs`, `mcp.rs`, `prompt_templates.rs`, `secret_store.rs`, `process/{ai_commands,changes,one_shot}.rs`.

### Database

SQLite at `$APPCONFIG/codex-ai.db`, 22 tables:

- Core: `projects`, `employees`, `tasks`, `subtasks`, `comments`, `activity_logs`, `employee_metrics`
- Delivery: `milestones`, `tags`, `task_tags`, `task_dependencies`, `task_attachments`
- Sessions: `codex_sessions`, `codex_session_events`, `codex_session_file_changes`, `codex_session_file_change_details`
- Ops: `notifications`, `ssh_configs`, `task_automation_state`, `task_git_contexts`
- Legacy, not a read/write source: `project_employees`, `codex_sessions_new`

**Constraints**:
- `employees.project_id` is the single source of truth for employee-project relationships — do not denormalize this elsewhere.
- All engines share `codex_sessions` / `codex_session_events` regardless of provider; the table names are historical.
- Rows are soft-deleted — queries must filter `deleted_at IS NULL`.

### Tests

Rust only (no frontend tests): **246 test cases**, mostly `#[cfg(test)]` modules colocated with the code they cover. Densest: `codex/process/tests.rs` (48), `codex/settings.rs` (22), `git_workflow.rs` (16), `task_automation.rs` (15). The non-codex engines are thinly covered (3–11 each).

Cross-cutting integration tests live in `src-tauri/src/app/tests/`:
- `runtime_and_paths.rs` — app runtime setup
- `sql_and_session.rs` — DB + session logic
- `task_lifecycle.rs` — task state transitions
- `review_and_attachments.rs` — review + file attachments

### Pre-execution validation (all engines)

All four engines call `app/shared.rs::validate_runtime_working_dir` before starting a session. It validates: working directory exists, is a directory, is accessible, and contains `.git`.

### CI/CD

`.github/workflows/build.yml` — builds installers on tag push or manual dispatch across Windows, Linux, macOS runners.
