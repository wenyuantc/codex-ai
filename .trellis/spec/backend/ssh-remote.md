# SSH Remote Execution

> Contracts for `app/remote.rs` — the single entry that builds all SSH commands.

---

## Scenario: ControlMaster short-command multiplexing (C1)

### 1. Scope / Trigger

- Trigger: any change to SSH command construction, long-session launch (`allocate_tty`), tray quit lifecycle, or secret/password auth path.
- Owner: `src-tauri/src/app/remote.rs` (`build_ssh_command`, `multiplex_args*`, `cleanup_ssh_mux_masters`).
- Call sites (~20) must keep going through `build_ssh_command` — do not invent parallel SSH arg builders.

### 2. Signatures

```rust
// Production: platform from cfg!(windows)
fn multiplex_args(allocate_tty: bool, known_hosts_mode: &str) -> Vec<String>

// Testable pure core
fn multiplex_args_impl(
    is_windows: bool,
    allocate_tty: bool,
    known_hosts_mode: &str,
) -> Vec<String>

fn ensure_ssh_mux_dir() -> Result<PathBuf, String>
pub(crate) fn cleanup_ssh_mux_masters()  // best-effort; never fatal
```

`build_ssh_command(..., allocate_tty: bool)` injects `multiplex_args` after `-p` / BatchMode / ConnectTimeout (and after `-tt` when long session), before known_hosts matching.

### 3. Contracts

| Case | Args injected |
|------|----------------|
| Windows (any) | `-o ControlMaster=no` + `-o ControlPath=none` (must override user `~/.ssh/config` Host * mux) |
| Unix + `allocate_tty=true` (long AI session) | `-o ControlMaster=no` only |
| Unix + short command | `-o ControlMaster=auto` + `ControlPath=<mux_dir>/cm-<mode>-%C` + `ControlPersist=60s` |

**ControlPath rules:**

- `%C` = OpenSSH connection hash (~40 hex) — avoid path-length blowups from user@host:port literals.
- `<mode>` = sanitized `known_hosts_mode` (`off` / `strict` / `accept-new` or other → `accept-new`). **Different modes must not share a socket** (`%C` does not include `UserKnownHostsFile`).
- Mux dir name: `codex-ai-ssh-mux`, mode `0700` on Unix.
- **macOS sun_path ~104**: if `temp_dir()/codex-ai-ssh-mux/cm-accept-new-<40hex>` would exceed ~100 bytes, use `/tmp/codex-ai-ssh-mux` instead of `$TMPDIR`.

**Env (password auth, unchanged by C1 R5 mitigate-not-eliminate):**

| Key | Role |
|-----|------|
| `CODEX_SSH_SECRET` | Password for askpass script (`printf '%s' "$CODEX_SSH_SECRET"`) — **required**, not redundant |
| `SSH_ASKPASS` / `SSH_ASKPASS_REQUIRE` / `DISPLAY` | Force askpass path |

Do not delete `CODEX_SSH_SECRET` without replacing the askpass contract. Plaintext at-rest secrets are a separate task (OS keychain).

**Quit cleanup:**

- Hook: `tray.rs` `"quit"` **before** `app.exit(0)`.
- Not on `window_event` `CloseRequested` (that is close-to-tray).
- Per socket: `ssh -o BatchMode=yes -o ConnectTimeout=2 -o ControlPath=<path> -O exit dummy`; if file remains, `remove_file`. Failures → `eprintln!` only.

### 4. Validation & Error Matrix

| Condition | Behavior |
|-----------|----------|
| Mux dir create fails | `Err("创建 SSH 复用目录失败: …")` from short-path setup |
| Password probe not passed | Existing hard error before spawn (unchanged) |
| Missing password_ref | Existing hard error (unchanged) |
| Quit cleanup fails | Log only; still exit |

### 5. Good / Base / Bad Cases

- **Good**: Unix short git ops reuse one ControlMaster; second command faster; socket under mux dir with `cm-<mode>-` prefix.
- **Base**: Windows remote still works even if user ssh config enables ControlMaster.
- **Bad**: Long AI session shares ControlMaster with short ops → master exit kills sessions / or reverse lifetime coupling.

### 6. Tests Required

- Table: Windows/Unix × long/short × `off`/`strict`/`accept-new` → exact arg sequences (`multiplex_args_impl`).
- Distinct ControlPath for different known_hosts modes.
- macOS sun_path length regression (`unix_control_path_stays_within_mac_sun_path_limit` class).
- Do **not** hit real network/SSH in unit tests.

### 7. Wrong vs Correct

#### Wrong

```rust
// Windows: omit mux flags and hope user config is fine
// Unix long session: omit ControlMaster (Host * may still enable mux)
// ControlPath: format!("{}@{}:{}", user, host, port)  // path length + no mode isolation
// Cleanup on CloseRequested  // close-to-tray, not quit
```

#### Correct

```rust
// Windows always: ControlMaster=no + ControlPath=none
// allocate_tty: ControlMaster=no
// short: ControlPath=.../cm-{sanitize(mode)}-%C under length-safe mux dir
// cleanup_ssh_mux_masters() only on tray quit before exit
```

---

## Design Decision: R5 password channel

**Context**: Password was passed via process env; naive “delete env” breaks askpass (`let _ = secret` in script builder).

**Decision**: Keep `CODEX_SSH_SECRET`; multiplexing shortens how often auth re-prompts. FIFO/embedded-script alternatives rejected for Windows parity and disk/constraint reasons.

**At-rest**: JSON plaintext secret store is out of C1 scope → OS keychain child task.
