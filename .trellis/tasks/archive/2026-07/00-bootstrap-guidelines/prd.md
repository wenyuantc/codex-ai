# Bootstrap Trellis Specs From Codebase

**You (the AI) are running this task. The developer does not read this file.**

## Goal

Rewrite `.trellis/spec/` from the real Codex AI codebase so future agents follow local architecture, contracts, and anti-patterns instead of generic templates.

## Scope

### Spec directories
- `.trellis/spec/frontend/` — React/Vite UI, stores, lib bridge
- `.trellis/spec/backend/` — Tauri/Rust commands, DB, AI engines, git workflow
- `.trellis/spec/guides/` — project-specific cross-layer and reuse thinking

### Source directories inspected
- `src/` (pages, components, stores, lib, hooks)
- `src-tauri/src/` (`app/`, `db/`, `codex/`, `claude/`, `opencode/`, `git_workflow.rs`, `task_automation.rs`, tests)
- `src-tauri/capabilities/default.json`
- `AGENTS.md`, `CLAUDE.md`, `README.md`, `package.json`, `components.json`

### Out of scope
- Product source code changes (except fixing broken Trellis script syntax that blocks task tooling)
- UI redesign, feature work, migrations, dependency upgrades

## Architecture Context

Codex AI is a **single-repo Tauri v2 modular monolith**:

```
React UI → Tauri IPC (`src/lib/backend.ts`) → Rust commands (`src-tauri/src/app/*`) → SQLite
```

Hard boundaries observed in code:
1. **All business writes go through Rust Tauri commands.** Frontend SQL `execute` is intentionally disabled (`src/lib/database.ts` throws). Capabilities allow `sql:allow-select` only.
2. **Zustand stores cache UI state.** Reads may use `select()` from SQLite; mutations call `backend.ts` wrappers.
3. **Domain modules in Rust:** `app/projects|tasks|employees|sessions|remote|review|delivery|database`, plus `git_workflow`, `task_automation`, multi-engine managers (`codex`/`claude`/`opencode`).
4. **Dual execution target:** local + SSH (`project_type` / `execution_target`). New features must remain SSH-compatible.
5. **Activity logs + Chinese dashboard labels** are product requirements (`getActivityActionLabel` in `src/lib/utils.ts`).
6. **Soft delete** via `deleted_at` for projects/tasks; trash restore/permanent delete commands exist.
7. **Schema evolution** is versioned inline migrations in `src-tauri/src/db/migrations.rs` (currently through v40).

## Files Created Or Updated

### Frontend
- `.trellis/spec/frontend/index.md`
- `.trellis/spec/frontend/directory-structure.md`
- `.trellis/spec/frontend/component-guidelines.md`
- `.trellis/spec/frontend/hook-guidelines.md`
- `.trellis/spec/frontend/state-management.md`
- `.trellis/spec/frontend/type-safety.md`
- `.trellis/spec/frontend/quality-guidelines.md`
- `.trellis/spec/frontend/data-access.md` (new)

### Backend (new layer)
- `.trellis/spec/backend/index.md`
- `.trellis/spec/backend/directory-structure.md`
- `.trellis/spec/backend/command-guidelines.md`
- `.trellis/spec/backend/database-migrations.md`
- `.trellis/spec/backend/error-handling.md`
- `.trellis/spec/backend/ai-engines.md`
- `.trellis/spec/backend/testing.md`

### Guides
- `.trellis/spec/guides/index.md`
- `.trellis/spec/guides/cross-layer-thinking-guide.md`
- `.trellis/spec/guides/code-reuse-thinking-guide.md`

### Tooling fix
- `.trellis/scripts/common/task_context.py` — fixed Python 3.11-incompatible multi-line f-string so `task.py` imports again

## Acceptance Criteria
- [x] Frontend specs describe actual React/Zustand/IPC patterns with examples
- [x] Backend layer exists and documents command/DB/engine/test patterns
- [x] Guides are project-specific (IPC + SQLite + SSH + activity keys)
- [x] Index files match the final file set
- [x] `rg` finds no `To be filled` / `TODO: fill` / template placeholders in `.trellis/spec`
- [x] Claims are backed by source files, tests, or project docs

## Status
- [x] Repository analysis
- [x] Specs written
- [x] Placeholder verification
