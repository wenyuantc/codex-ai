# CODEBUDDY.md

This file provides guidance to CodeBuddy Code when working with code in this repository.

## Project Overview

AI Employee Collaboration System (AI员工协作系统) — a Tauri 2.0 desktop app that manages AI "employees" who execute tasks via the `codex` CLI. Built with React 19 + TypeScript + Tailwind CSS 4 on the frontend, Rust on the backend, and SQLite for persistence.

## Build & Dev Commands

```bash
# Frontend-only dev server (no Tauri)
npm run dev

# Full Tauri dev (Rust + frontend, requires Rust toolchain)
npm run tauri dev

# Build for production
npm run build          # frontend only (tsc && vite build)
npm run tauri build    # full Tauri app bundle

# Type checking
npx tsc --noEmit

# Add a shadcn/ui component
npx shadcn@latest add <component-name>
```

There are no test frameworks configured in this project.

## Architecture

### Two-Layer Structure

**Frontend** (`src/`): React SPA; all business data access goes through Tauri `invoke()` wrappers in `src/lib/backend.ts` (no direct SQLite).

**Backend** (`src-tauri/src/`): Rust Tauri app that owns SQLite (SQLx), manages AI CLI subprocesses, and serves Tauri commands.

### Data Flow

1. **Tauri commands for reads and writes**: Stores call typed wrappers in `src/lib/backend.ts` → Rust `#[tauri::command]` → SQLite. `src/lib/database.ts` is a hard-fail stub; capabilities do not allow frontend SQL select/execute.
2. **Tauri commands for process management**: Codex/Claude/Grok/OpenCode process lifecycle goes through `invoke()` to Rust engine modules.
3. **Tauri events for real-time output**: Rust emits engine stdout/stderr/exit events; the frontend listens via `@tauri-apps/api/event`.

### Frontend Structure

- **Pages** (`src/pages/`): Route-level components — Dashboard, Projects, ProjectDetail, Kanban, Employees, Settings.
- **Components** (`src/components/`): Domain-organized — `ai/`, `codex/`, `dashboard/`, `employees/`, `layout/`, `projects/`, `tasks/`, `ui/` (shadcn).
- **Stores** (`src/stores/`): Zustand stores (`taskStore`, `projectStore`, `employeeStore`, `dashboardStore`, `logStore`). Fetch via `backend.ts` list/get commands.
- **Lib** (`src/lib/`):
  - `backend.ts` — typed Tauri command wrappers (primary data access door)
  - `database.ts` — hard-fail stub (do not reintroduce frontend SQL)
  - `types.ts` — TypeScript interfaces mirroring DB tables, plus status/priority constants
  - `ai.ts` — Client-side AI heuristics (assignee suggestion, complexity analysis) — fallback when CLI is unavailable
  - `codex.ts` — Tauri invoke/event wrappers for Codex process management
  - `utils.ts` — `cn()` (tailwind-merge), labels, date/status/priority formatters

### Backend Structure

- `lib.rs` — App setup: registers plugins (SQL with migrations, shell, dialog, opener), manages `CodexManager` as Tauri state, registers invoke handlers.
- `codex/process.rs` — Tauri commands: `start_codex`, `stop_codex`, `restart_codex`, `send_codex_input`, `ai_suggest_assignee`, `ai_analyze_complexity`, `ai_generate_comment`. Spawns `codex` CLI as a subprocess via `tauri-plugin-shell`.
- `codex/manager.rs` — `CodexManager`: HashMap of `employee_id -> CommandChild` for tracking running processes.
- `db/migrations.rs` — 11 migrations: tables (projects, employees, tasks, subtasks, comments, activity_logs, employee_metrics, project_employees), indexes, updated_at triggers, seed data.
- `db/models.rs` — Rust structs for DB rows, DTOs, and event payloads.
- `tray.rs` — System tray with "show window" and "quit" menu items.
- `window_event.rs` — Close button hides window instead of quitting (minimize to tray).

### Database

SQLite via `tauri-plugin-sql`. Migrations run automatically on app start. DB file: `sqlite:codex-ai.db`. Frontend uses `$1`-style parameterized queries. All timestamps are UTC via `datetime('now')`. The `updated_at` triggers auto-update on row changes.

### Key Patterns

- **Path alias**: `@/` maps to `src/` (configured in both `tsconfig.json` and `vite.config.ts`).
- **UI components**: shadcn/ui with `base-nova` style, `lucide-react` icons. Add new ones via `npx shadcn@latest add`.
- **State management**: Zustand stores with async actions that call SQL directly. Optimistic updates for Kanban drag (`moveTask`), then `updateTaskStatus` persists.
- **DnD**: `@dnd-kit/core` + `@dnd-kit/sortable` for Kanban board drag-and-drop.
- **Activity logging**: `logActivity()` in `utils.ts` inserts into `activity_logs` table after mutations.
- **Keyboard shortcuts**: `Cmd/Ctrl+N` (kanban), `Cmd/Ctrl+E` (employees), `Cmd/Ctrl+D` (dashboard), `Cmd/Ctrl+P` (projects).
- **Dual AI layer**: `src/lib/ai.ts` provides client-side heuristic AI features; `src-tauri/src/codex/process.rs` provides server-side AI commands via Codex CLI. The `run_ai_command` helper currently uses `echo` as placeholder when `codex` CLI is not available.
