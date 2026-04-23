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

**Codex AI** is a Tauri v2 desktop app — a project/task manager with Codex CLI integration. The data flow is strictly:

```
React (UI) → Tauri IPC commands → Rust service layer → SQLite
```

**All business writes go through Rust Tauri commands.** The frontend never writes directly to the database. Zustand stores only cache frontend state fetched from Rust.

### Frontend (`src/`)

- React 19 + TypeScript + Vite (dev port 1420) + TailwindCSS 4
- **6 Zustand stores**: `project`, `task`, `employee`, `dashboard`, `notification`, `log`
- **7 pages**: Dashboard, Projects, ProjectDetail, Kanban, Sessions, Employees, Settings
- **Path alias**: `@/*` maps to `src/*`
- Component folders are feature-organized: `ai/`, `codex/`, `dashboard/`, `employees/`, `git/`, `projects/`, `sessions/`, `tasks/`

### Backend (`src-tauri/src/`)

- Rust 2021, Tokio async, SQLx 0.8 (compile-time checked queries)
- Entry: `lib.rs` → `pub fn run()` sets up Tauri plugins + window restoration
- **45 Tauri commands** spread across `app/` submodules:
  - `app/projects.rs`, `app/employees.rs`, `app/tasks.rs`, `app/sessions.rs`
  - `app/review.rs`, `app/remote.rs`, `app/database.rs`
- `codex/` — Codex CLI process lifecycle manager (`CodexManager` state)
- `db/migrations.rs` — versioned DDL (40+ migrations inline)
- `db/models.rs` — SQLx `query_as!` type definitions for all tables
- `task_automation.rs` — task state machine (large file)
- `git_workflow.rs` — Git operations (large file)
- `notifications.rs` — event notification system
- `tray.rs` — system tray

### Database

SQLite at `$APPCONFIG/codex-ai.db`. Key tables: `projects`, `employees`, `tasks`, `subtasks`, `comments`, `activity_logs`, `employee_metrics`, `codex_sessions`, `codex_session_events`.

**Constraint**: `employees.project_id` is the single source of truth for employee-project relationships — do not denormalize this elsewhere.

### Tests

Integration tests only (no frontend tests). Located in `src-tauri/src/app/tests/`:
- `runtime_and_paths.rs` — app runtime setup
- `sql_and_session.rs` — DB + session logic
- `task_lifecycle.rs` — task state transitions
- `review_and_attachments.rs` — review + file attachments

### Pre-execution validation (Codex sessions)

Before starting a Codex session, Rust validates: working directory exists, is a directory, is accessible, and contains `.git`.

### CI/CD

`.github/workflows/build.yml` — builds installers on tag push or manual dispatch across Windows, Linux, macOS runners.
