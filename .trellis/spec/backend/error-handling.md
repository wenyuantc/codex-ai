# Error Handling

> Backend failures are explicit `Err(String)` values plus optional notifications.

---

## Primary Pattern

Commands return `Result<T, String>`.

```rust
.map_err(|error| format!("Failed to create project: {}", error))?;
// or user-facing Chinese:
return Err("项目名称不能为空".to_string());
```

Guidance:
- Validation/user mistakes → clear Chinese messages (product UI language).
- Internal/sql/IO failures → may be English or Chinese prefixes already used in module; stay consistent with neighboring code in that file.
- Always propagate with `?` after `map_err`; do not panic in commands.

## Validation Helpers

Shared validators in `app/shared.rs` and domain modules:
- `validate_project_repo_path`
- `validate_remote_repo_path`
- `validate_runtime_working_dir`
- `normalize_project_type` / `normalize_optional_text`
- task automation guards (`validate_task_automation_mode_change`, `validate_task_archival_guard`)

Prefer pure helper functions that return `Result<_, String>` so unit tests can cover them without Tauri.

## Notifications

`notifications.rs` supports sticky and transient notifications for operational issues:
- SDK unavailable
- database errors
- SSH config/health problems

Use existing dedupe keys and severity constants when emitting system-level failures so the UI notification center does not spam duplicates.

## Side-Effect Failures

When a command performs multi-step work:
1. Prefer transaction for DB multi-writes.
2. Track external side effects (files uploaded, remote paths created).
3. On failure, compensate (delete files, rollback tx, cleanup remote paths).

`create_task` attachment/SSH sync is the reference implementation.

## Process / Engine Errors

Engine modules convert process failures into session events and status updates rather than only returning once to the caller. When changing lifecycle code:
- keep session records consistent (`running` → terminal state)
- emit events the frontend listeners already understand
- avoid leaving employees stuck in `busy` without exit handling

## Logging

- `eprintln!` is used sparingly for startup/window restore failures and best-effort quit cleanup (e.g. SSH ControlMaster socket sweep in `cleanup_ssh_mux_masters`).
- Prefer activity logs for user-auditable actions and notifications for actionable system problems.
- Do not log secrets (passwords, private key material, resolved secret store values, `CODEX_SSH_SECRET`, keyring payload).
- SSH at-rest keychain / Secret Service failures return Chinese `String` errors; never silently fall back to plaintext `ssh-secrets.json`.

## Anti-Patterns

- `unwrap()` / `expect()` in command paths on user input.
- Swallowing SQL errors and returning success.
- Returning empty Ok when a write partially failed.
- Exposing raw SSH private key paths’ file contents in errors.
