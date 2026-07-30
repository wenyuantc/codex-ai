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
    │   ├── delivery.rs         # milestones/tags/dependencies
    │   ├── employees.rs
    │   ├── sessions.rs         # sessions, search, activity log
    │   ├── remote.rs           # SSH configs + remote runtime
    │   ├── review.rs           # attachments + code review launch
    │   └── tests/              # integration-style tests + helpers
    ├── db/
    │   ├── models.rs           # FromRow entities + command DTOs
    │   └── migrations.rs       # ordered Migration list
    ├── codex/                  # Codex engine manager/process/settings/mcp/secrets
    ├── claude/                 # Claude engine
    ├── opencode/               # OpenCode engine
    ├── grok/                   # Grok Build CLI engine (manager/settings/process)
    ├── git_workflow.rs         # project/task git commands
    ├── git_runtime.rs
    ├── task_automation.rs      # review-fix loop orchestration
    ├── notifications.rs
    ├── process_spawn.rs
    ├── tray.rs
    ├── window_state.rs
    └── window_event.rs
```

## Ownership Rules

| Concern | Owner module |
|---------|--------------|
| CRUD for projects/tasks/employees | `app/projects.rs`, `app/tasks.rs`, `app/employees.rs` |
| Delivery fields (due/tags/deps/milestones) | `app/delivery.rs` |
| Session list/resume/log/search/activity | `app/sessions.rs` |
| SSH config + remote command execution | `app/remote.rs` (single `build_ssh_command`; ControlMaster cleanup on tray quit) |
| Attachments + review prompts | `app/review.rs` |
| DB maintenance / provider capabilities | `app/database.rs` |
| Shared pure helpers / constants | `app/shared.rs` |
| Git UX commands | `git_workflow.rs` |
| Engine process lifecycle | `codex|claude|opencode|grok` |
| Auto review/fix state machine | `task_automation.rs` |
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
