# Backend Directory Structure

> Where Rust code lives and what each module owns.

---

## Layout

```text
src-tauri/
├── capabilities/default.json   # IPC/plugin permissions for the main window
├── Cargo.toml
└── src/
    ├── main.rs                 # binary entry
    ├── lib.rs                  # plugins, state, generate_handler!
    ├── app/                    # domain Tauri commands + helpers
    │   ├── mod.rs              # re-exports + submodule wiring
    │   ├── shared.rs           # ids, time, path validation, constants
    │   ├── database.rs         # health, backup/restore, dashboard, capabilities
    │   ├── projects.rs
    │   ├── tasks.rs
    │   ├── templates.rs        # task templates CRUD + batch apply
    │   ├── delivery.rs         # milestones/tags/dependencies
    │   ├── employees.rs
    │   ├── sessions.rs         # sessions, search, activity log
    │   ├── remote.rs           # SSH configs + remote runtime
    │   ├── review.rs           # attachments + code review launch
    │   └── tests/              # integration-style tests + helpers
    ├── db/
    │   ├── models.rs           # FromRow entities + command DTOs
    │   └── migrations.rs       # ordered Migration list
    ├── engine/                 # Shared AI process kernel (context/child/manager/status/usage)
    ├── codex/                  # Codex engine manager/process/settings/mcp/secrets
    ├── claude/                 # Claude engine
    ├── opencode/               # OpenCode engine
    ├── grok/                   # Grok Build CLI engine (manager/settings/process)
    ├── git_workflow/           # project/task git commands (domain file-split)
    │   ├── mod.rs              # imports + include! wiring; crate path git_workflow::*
    │   ├── types.rs            # DTOs / state constants
    │   ├── runtime.rs          # runtime resolve, run_git helpers
    │   ├── worktree.rs         # worktree list/stage/commit/remove/merge
    │   ├── context.rs          # task_git_contexts + prepare/engine hooks
    │   ├── project_ops.rs      # overview/commits/preview/main-tree stage-commit
    │   ├── branch.rs           # push/pull/checkout/create/delete/merge branches
    │   ├── pending_action.rs   # request/confirm/cancel git action
    │   └── tests.rs
    ├── git_runtime.rs          # low-level git process (local + SSH)
    ├── task_automation.rs      # review-fix loop root (mod prompt + include! slices)
    ├── task_automation/
    │   ├── prompt.rs           # fix-prompt builder (real submodule)
    │   ├── state.rs            # phases/policy/state upsert
    │   ├── session_exit.rs     # resume/orphan + session exit handlers
    │   ├── fix_loop.rs         # review/fix retries + fix round launch
    │   ├── restart.rs          # restart_task_automation*
    │   ├── review_data.rs      # review report/verdict recovery helpers
    │   └── tests_modules.rs    # #[cfg(test)] modules
    ├── notifications.rs
    ├── process_spawn.rs
    ├── tray.rs
    ├── window_state.rs
    └── window_event.rs
```

> **Split note**: `git_workflow/*` and most `task_automation/*` slices are composed with `include!` so items share one module namespace (Tauri command inventory + private helper visibility stay stable). `task_automation/prompt.rs` remains a real `mod prompt` submodule.

## Ownership Rules

| Concern | Owner module |
|---------|--------------|
| CRUD for projects/tasks/employees | `app/projects.rs`, `app/tasks.rs`, `app/employees.rs` |
| Task templates (batch create) | `app/templates.rs` (see [task-templates.md](./task-templates.md)) |
| Delivery fields (due/tags/deps/milestones) | `app/delivery.rs` |
| Session list/resume/log/search/activity | `app/sessions.rs` |
| SSH config + remote command execution | `app/remote.rs` (single `build_ssh_command`; ControlMaster cleanup on tray quit) |
| Attachments + review prompts | `app/review.rs` |
| DB maintenance / provider capabilities | `app/database.rs` |
| Shared pure helpers / constants | `app/shared.rs` |
| Git UX commands | `git_workflow/` (`crate::git_workflow`) |
| Shared engine process kernel | `engine/` (`ExecutionContext`, `EngineChild`, `ProcessManager`) |
| Engine process lifecycle / protocol | `codex|claude|opencode|grok` (stream + launch; manager/context re-export kernel) |
| Auto review/fix state machine | `task_automation` (+ `task_automation/*` slices) |
| Sticky/transient notifications | `notifications.rs` |

## Adding A New Command

1. Implement in the owning module as `pub async fn ...<R: Runtime>(...) -> Result<..., String>`.
2. Export/use through `app/mod.rs` if other modules need internals.
3. Register in `lib.rs` `generate_handler![...]`.
4. Add frontend wrapper in `src/lib/backend.ts`.
5. If schema changes, append a migration.
6. If behavior is invariant-heavy, add a test under `app/tests/` or `#[cfg(test)]` in the module.

## State Managed At Startup (`lib.rs`)

- SQL plugin with migrations for `sqlite:codex-ai.db`
- `CodexManager`, `ClaudeManager`, `OpenCodeManager`, `GrokManager` in Tauri state
- Tray + window size restore
- Resume pending task automation
- Optional OpenCode SDK server spawn on startup

## Anti-Patterns

- Putting new domain CRUD into `lib.rs` or engine process files.
- Creating a second SQLite access path that bypasses `sqlite_pool` / plugin DB.
- Duplicating employee-project relations outside `employees.project_id`.
- Editing `src-tauri/target/` or generated schema files by hand.
