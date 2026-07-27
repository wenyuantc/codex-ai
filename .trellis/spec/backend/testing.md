# Backend Testing

> Rust tests are the only automated suite in this repository.

---

## How To Run

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml <test_name>
```

## Test Layout

### Integration-style app tests

`src-tauri/src/app/tests/`:
- `mod.rs` — shared imports, `setup_test_pool()`, fixture helpers
- `task_lifecycle.rs` — automation/archive guards and task state rules
- `sql_and_session.rs` — SQL/session behaviors
- `review_and_attachments.rs` — review/attachment helpers
- `runtime_and_paths.rs` — path/working-dir validation

`setup_test_pool()` creates an in-memory SQLite pool and runs `build_current_migrator()` so tests see the real schema.

### Module unit tests

Many modules include `#[cfg(test)]` blocks:
- `notifications.rs`
- `db/migrations.rs`
- engine process modules (`codex/process`, `opencode/process`, ...)
- `app/shared.rs`
- `task_automation/prompt.rs`

Prefer pure functions for new business rules so they can be tested without spawning full Tauri.

## What Good Tests Look Like Here

From `task_lifecycle.rs`:
- Construct plain `Task` structs with required fields
- Call pure validators (`validate_task_automation_mode_change`, `validate_task_archival_guard`)
- Assert exact Chinese error strings when those strings are user contracts
- Table-drive phase lists for automation active/inactive checks

From pool tests:
- Insert fixtures through helpers (`insert_task_record`, `insert_session`, ...)
- Assert SQL-visible outcomes after command helper execution

## When Adding Features

| Change type | Test expectation |
|-------------|------------------|
| New validation rule | Unit test on pure helper |
| Migration | Ensure migrator still builds; add focused SQL test if constraint is subtle |
| Task/session invariant | Extend `app/tests/*` |
| Engine parsing/lifecycle pure logic | `#[cfg(test)]` in that module |
| Pure UI wiring | manual smoke (`npm run tauri:dev`) — no frontend runner yet |

## Anti-Patterns

- Tests that require a real home-directory app config when memory pool fixtures suffice.
- Asserting only that “error is some string” when the UI depends on a specific message.
- Hitting network/SSH in unit tests.
- Ignoring failures with `let _ =` in test setup — use `expect` with reason.

## Review Checklist (backend)

- [ ] Command registered in `lib.rs`
- [ ] Models + migration updated together
- [ ] SSH/local implications considered
- [ ] Activity action added (and frontend label if user-visible)
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` passes for touched logic
