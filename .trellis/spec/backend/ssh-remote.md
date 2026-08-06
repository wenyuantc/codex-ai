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

**At-rest (C1b)**: passwords live in the OS credential store via `keyring` — see next scenario.

---

## Scenario: SSH password at-rest via OS keychain (C1b)

### 1. Scope / Trigger

- Trigger: any change to `codex/secret_store.rs`, secret ref lifecycle, SSH password/passphrase save paths, or keyring dependency.
- Owner: `src-tauri/src/codex/secret_store.rs`.
- Callers (`app/remote.rs`) keep the same public API; do not read/write password files ad hoc.

### 2. Signatures

```rust
pub fn store_secret_value<R: Runtime>(app, value: Option<&str>, replace_ref: Option<&str>) -> Result<Option<String>, String>
pub fn resolve_secret_value<R: Runtime>(app, secret_ref: Option<&str>) -> Result<Option<String>, String>
pub fn delete_secret_value<R: Runtime>(app, secret_ref: Option<&str>) -> Result<(), String>
pub fn sweep_orphan_secret_refs<R: Runtime>(app, active_refs: &HashSet<String>) -> Result<usize, String>
```

Internal: `SecretBackend` trait; production `KeyringBackend`; tests use `MemoryBackend`.

### 3. Contracts

| Layer | What | Where |
|-------|------|--------|
| Credential | password plaintext | OS keychain — `keyring` service `"codex-ai-ssh"`, account = `secret_ref` |
| Index | ref metadata only (**no `value`**) | `$APPCONFIG/ssh-secret-index.json` (version 2) |
| Legacy | migration source only | `$APPCONFIG/ssh-secrets.json` — deleted after successful migrate |

- Index writes are atomic (temp + rename); Unix mode 0600.
- `store`: persist new keyring + index first; on index failure compensate with `backend.delete`; delete replaced ref only after new index is durable.
- Runtime injection still uses `CODEX_SSH_SECRET` (C1 R5) — keychain is **at-rest**, not the process channel.

### 4. Validation & Error Matrix

| Condition | Behavior |
|-----------|----------|
| Secret Service / keychain unavailable | Chinese `Err` (e.g. Linux: install/enable gnome-keyring); **no** plaintext JSON fallback |
| Legacy migrate partial failure | Keep `ssh-secrets.json`; `Err("迁移 SSH 密钥到系统凭据库失败: …")`; retryable |
| Index has ref, keyring missing | Resolve returns Chinese corruption-style error (not silent empty success if design marks corrupt) |
| `NoEntry` on delete | Idempotent success |

### 5. Good / Base / Bad Cases

- **Good**: new password SSH config → only index file on disk; password only in OS store.
- **Base**: upgrade with old `ssh-secrets.json` → one-shot migrate, legacy file gone, resolve still works.
- **Bad**: write passwords into index JSON or reintroduce plaintext `ssh-secrets.json` as production path.

### 6. Tests Required

- MemoryBackend: store/resolve, delete, replace_ref, sweep, migrate (index has no password plaintext), migrate failure keeps legacy, atomic/serialize contracts.
- Do not require interactive real Keychain in default `cargo test`.

### 7. Wrong vs Correct

#### Wrong

```rust
// Production path keeps writing entry.value into JSON
// On Linux keyring failure, fall back to ssh-secrets.json
// store: delete old secret before new index is durable
```

#### Correct

```rust
// value → KeyringBackend only; index = meta only
// keyring errors → Chinese Err, never plaintext fallback
// ensure_migrated before every public API entry
```

---

## Scenario: Remote OpenCode SDK bridge runtime

### 1. Scope / Trigger

- Trigger: any change to remote Node/SDK runtime layout, remote engine health/install commands, or the OpenCode SSH launch path.
- Owner: `src-tauri/src/app/remote.rs` (`*_remote_opencode_*`), `opencode/process/{mod,session_runtime}.rs`, `codex/process/one_shot.rs`.
- Pattern: **mirrors the Codex remote SDK runtime** (`ensure_remote_sdk_runtime_layout` / `install_remote_codex_sdk` / `inspect_remote_codex_runtime`). A new SDK-based engine should copy this shape, not invent a third one.

### 2. Signatures

```rust
// app/remote.rs
pub(crate) fn default_remote_opencode_sdk_install_dir(ssh_config_id: &str) -> String
pub(crate) fn remote_opencode_sdk_bridge_path(install_dir: &str) -> String
pub(crate) fn build_remote_opencode_sdk_bridge_command(
    install_dir: &str,
    node_path_override: Option<&str>,
) -> String
pub(crate) async fn ensure_remote_opencode_sdk_runtime_layout<R>(app, ssh_config_id) -> Result<String, String>  // → install_dir
pub(crate) async fn inspect_remote_opencode_runtime<R>(app, ssh_config, install_dir, node_path_override)
    -> Result<RemoteOpenCodeHealthCheck, String>

#[tauri::command] pub async fn validate_remote_opencode_health<R>(app, ssh_config_id: String)
    -> Result<RemoteOpenCodeHealthCheck, String>
#[tauri::command] pub async fn install_remote_opencode_sdk<R>(app, ssh_config_id: String)
    -> Result<RemoteOpenCodeSdkInstallResult, String>

// opencode/process/session_runtime.rs
pub async fn launch_opencode_bridge_via_ssh<R>(
    app, ssh_config, install_dir: &str, node_path_override: Option<&str>, config: &OpenCodeBridgeConfig,
) -> Result<(OpenCodeChild, Vec<PathBuf>), String>   // .1 = askpass paths the caller must clean up
```

No DB migration: SSH sessions reuse `codex_sessions` (`execution_target='ssh'`, `ssh_config_id`, `ai_provider='opencode'`).

### 3. Contracts

**Remote layout** — one directory per SSH config, isolated from the Codex runtime:

| Item | Value |
|------|-------|
| Install dir | `~/.codex-ai/opencode-sdk-runtime/<ssh_config_id>` |
| Bridge file | `<install_dir>/opencode_sdk_bridge.mjs`, uploaded via `cat >` from `include_str!` |
| Package marker | `<install_dir>/package.json` = `{"name":"codex-ai-opencode-sdk-runtime","private":true,"type":"module"}` |
| npm package | `@opencode-ai/sdk@latest`, `npm install --no-audit --no-fund` |
| Min Node | major `>= 18` (`OPENCODE_MINIMUM_NODE_MAJOR`) |
| Launch command | `install_dir=<expr>; bridge_path=<expr>; cd "$install_dir" && exec node "$bridge_path"` |

`RemoteOpenCodeHealthCheck`: `available`, `node_available`, `node_version`, `sdk_installed`, `sdk_version`, `sdk_install_dir`, `message` (Chinese), `checked_at`.
`available == node_available && node_major >= 18 && sdk_installed` — never just `sdk_installed`.

**SSH invariants** (all inherited from the C1 scenario above, restated because they are easy to lose in a new engine):

- Every remote call goes through `build_ssh_command` / `execute_ssh_command(_with_input)`. No parallel arg builder.
- Long session + one-shot use `allocate_tty = true` → `ControlMaster=no`, so a dying mux master cannot kill a live AI session.
- Bridge stdin JSON is produced by `serialize_opencode_bridge_config` — the same serializer as local. `workingDirectory` is the **remote** `run_cwd`.
- Remote `<run_cwd>/opencode.json` is written through `write_remote_opencode_runtime_config_file`, which returns a backup handle; every failure path and the exit path must `restore_async`.
- `build_ssh_command` may return an askpass script path; every early return must `remove_file` it.
- All remote stderr surfaced to the user passes through `redact_secret_text`.

**Auto-install policy**: on launch, `inspect` first; if `node_available && !sdk_installed`, call `install_remote_opencode_sdk` once, then re-inspect. Never loop. The settings page keeps an explicit install button for the manual path.

### 4. Validation & Error Matrix

| Condition | Behavior |
|-----------|----------|
| `execution_target == ssh` but no `ssh_config_id` | `finalize_launch_failure("runtime_prepare_failed")` + `Err("SSH 会话缺少 ssh_config_id")` |
| SSH config row missing | `finalize_launch_failure("runtime_prepare_failed")` + fetch error |
| Inspect command fails | `finalize_launch_failure("remote_runtime_inspect_failed")` |
| Node missing | `Err("远程 Node 不可用，请先在远端安装 Node.js 18+")` |
| Node major `< 18` | `Err("远程 Node 版本过低（当前 N），OpenCode SDK 需要 Node.js 18+")` |
| SDK missing, auto-install fails | `finalize_launch_failure("remote_sdk_install_failed")` + npm stderr (redacted) |
| Still unavailable after install | `finalize_launch_failure("remote_runtime_unavailable")` + `runtime.message` |
| Remote `opencode.json` unparseable | Do **not** overwrite; same rule as local |
| Any launch failure | Restore remote config backup + delete askpass + finalize session as failed |

Failures are always a specific Chinese reason. 「尚未实现」 is not an acceptable terminal error for a supported execution target.

### 5. Good / Base / Bad Cases

- **Good**: SSH project + OpenCode employee, remote has Node 20 and the SDK → terminal prints `[SSH] 运行通道: 远程 SDK`, session row lands with `execution_target='ssh'` / `ai_provider='opencode'`, stop marks it non-running.
- **Base**: remote has Node but no SDK → one automatic `npm install`, then the session starts; explicit install button in settings does the same thing.
- **Bad**: remote install dir shared with the Codex runtime (bridge filename collision); baseline capture called with `ssh_config = None` on the SSH path; a launch failure that leaves the user's remote `opencode.json` overwritten.

### 6. Tests Required

- `remote_opencode_install_dir_is_independent_of_codex` — asserts the OpenCode install dir never equals the Codex one for the same `ssh_config_id`.
- `remote_opencode_sdk_bridge_command_expands_home_and_spaces` — `~` expansion + quoting via `remote_shell_path_expression` for dirs containing spaces.
- `opencode_one_shot_provider_is_allowed_for_remote` — `normalize_one_shot_provider(Some("opencode"), is_remote)` returns `"opencode"` for **both** `false` and `true`.
- Existing local OpenCode runtime-config tests (backup/restore, invalid-JSON no-overwrite) must keep passing — they now cover a contract shared with the remote path.
- No real SSH/network in unit tests: assert on constructed command strings only.

### 7. Wrong vs Correct

#### Wrong

```rust
// SSH target rejected outright
if execution_target == EXECUTION_TARGET_SSH {
    return Err("SSH 模式下暂不支持 OpenCode，尚未实现".into());
}

// remote provider silently rewritten instead of reported
if !is_remote && provider == "opencode" { "opencode" } else { "codex" }

// health = package presence only
let available = sdk_installed;

// SSH path passes None
capture_execution_change_baseline(&app, /* ssh_config */ None, run_cwd).await;
```

#### Correct

```rust
// inspect → auto-install once → re-inspect → controlled Chinese failure
let mut runtime = inspect_remote_opencode_runtime(&app, ssh_config, &install_dir, None).await?;
if !runtime.available && runtime.node_available && !runtime.sdk_installed {
    install_remote_opencode_sdk(app.clone(), ssh_config_id.to_string()).await?;
    runtime = inspect_remote_opencode_runtime(&app, ssh_config, &install_dir, None).await?;
}
if !runtime.available { return Err(runtime.message); }

let available = node_available && node_supported && sdk_installed;
capture_execution_change_baseline(&app, Some(&ssh_config), run_cwd).await;
```
