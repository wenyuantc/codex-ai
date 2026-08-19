# AI Engines

> Codex, Claude, OpenCode, and Grok share session storage and a process kernel; they diverge in stream protocols and CLI/SDK launch.

---

## Shared Kernel (`src-tauri/src/engine/`)

| Module | Responsibility |
|--------|----------------|
| `engine/context.rs` | `ExecutionContext` + `resolve_*_execution_context` / one-shot working dir (engine label injected into Chinese errors) |
| `engine/child.rs` | `EngineChild` + `EngineProcessHandle` trait (killpg / kill / try_wait / stdio); optional retained `ChildStdin` for interactive sessions |
| `engine/stdin.rs` | NDJSON follow-up framing (`encode_session_followup_input`) + `write_stdin_bytes` (flush, keep pipe open) |
| `engine/manager.rs` | `ProcessManager<SessionKind, Extra>` + `ManagedProcess` + `EngineProcessRegistry` trait |
| `engine/status.rs` | `resolve_final_session_status` (stopping / exit 0 / failed) |
| `engine/usage.rs` | `UsageDelta` + `parse_usage_value` / `usage_u64` — shared token parsing for all engines |

Rules:
- **Do not copy** manager / lifecycle / context implementations into a new engine — re-export or type-alias the shared kernel.
- Keep **`process/stream.rs` per engine** (CLI JSON protocols differ).
- Keep **`process/mod.rs` launch/CLI args** per engine unless a pure helper is already proven identical.
- Codex extras (`provider`, execution-change baseline, sdk file-change store) live in `CodexProcessExtra` on `ManagedProcess::extra`.
- OpenCode keeps `sdk_server` on `OpenCodeManager` (not in the generic process table).
- `start_*` / `stop_*` remain engine-specific Tauri commands; do not introduce a single `dyn AiEngine::start` dispatcher unless product scope expands.
- Task-execution `start_*` returns tagged `StartSessionOutcome` (`started` \| `queued`). Gate/queue rules live in [run-queue.md](./run-queue.md), not in four engine copies.

## Engine Modules

| Engine | Root | Responsibilities |
|--------|------|------------------|
| Codex | `src-tauri/src/codex/` | Primary OpenAI Codex CLI/SDK integration, secrets, MCP config, prompt templates, one-shot routing |
| Claude | `src-tauri/src/claude/` | Anthropic Claude SDK bridge + process lifecycle (local + SSH CLI) |
| OpenCode | `src-tauri/src/opencode/` | OpenCode SDK server/process integration; local Node bridge + **remote SDK bridge over SSH** |
| Grok | `src-tauri/src/grok/` | xAI Grok Build CLI (`grok`) headless sessions + settings/health (local + SSH) |

Common internal layout per engine:
- `manager.rs` — thin wrapper / type alias over `engine::ProcessManager` (Codex/OpenCode may wrap extras)
- `settings.rs` — load/save engine settings
- `process/` — launch (`mod.rs`), stream, session_runtime; lifecycle/context re-export shared kernel
- `*_sdk_bridge.mjs` — Node bridge assets where needed (Codex/Claude/OpenCode; Grok is CLI-only)

`ai_provider` stored values: `"codex" | "claude" | "opencode" | "grok"`.

## Shared Session Model

Despite the historical `codex_` table prefix, sessions for engines are stored in:
- `codex_sessions`
- `codex_session_events`
- related file-change tables

Employees choose `ai_provider` and model/reasoning settings. Runtime status helpers resolve the correct manager (`get_employee_runtime_status`, `get_codex_session_status`).

## Provider Capability Matrix

Single source of truth: `app/database.rs::get_ai_provider_capabilities` (frontend: `getAiProviderCapabilities` / `src/lib/aiCapabilities.ts`).

| Capability | Meaning |
|------------|---------|
| `start` / `stop` / `resume` | Per-engine session lifecycle (all four engines) |
| `restart` | Stop live managed processes for the employee, then call that engine's `start_*` (**not** CLI resume of the old session id) |
| `send_input` | Mid-session stdin to a **live** process. **Codex / Claude / OpenCode = true** (SDK bridge keeps stdin; NDJSON `{"type":"input","prompt":...}` follow-ups). **Grok = false** (B1: headless `-p` + `Stdio::null`; see task notes). Do not set `true` without a verifiable write path; never advertise a capability that always fails. CLI-only Codex/Claude sessions may still reject at command time with a clear Chinese error. |

UI rules: Settings shows the four-engine comparison; any control for restart/send_input must gate on the matrix (`can(provider, cap)`) with Chinese disabled reasons. Prefer fail-closed when the matrix fails to load.

## Mid-session `send_input` (code-spec)

### 1. Scope / Trigger
Real mid-session stdin to a **live** managed process. Never implement via resume / new session / restart. Flip matrix `send_input: true` only after a verifiable write path exists for that engine.

### 2. Signatures
| Command | Args | Result |
|---------|------|--------|
| `send_codex_input` | `employee_id: String`, `input: String` | `Result<(), String>` |
| `send_claude_input` | same | same |
| `send_opencode_input` | same | same |
| `send_grok_input` | same | **always** `Err` (B1 honesty) |
| `finish_codex_input` / `finish_claude_input` / `finish_opencode_input` | `employee_id: String` | Close retained stdin (EOF) so interactive wait exits; process ends → orchestration can advance |

Frontend wrappers: `send*` / `finish*` in `src/lib/{codex,claude,opencode,grok}.ts`. Shared UI: `SessionInputBar` — **Send** queues follow-up; **结束会话 / End session** closes stdin. Hosts: TaskLog / SessionLog / EmployeeRunningSessions (+ review panel).

### 3. Contracts
- **Framing** (Codex / Claude / OpenCode SDK bridges): one NDJSON line `{"type":"input","prompt":"<trimmed>"}` + `\n`; pipe stays open (`engine/stdin.rs`).
- **Terminal echo tags** (stable, localized in UI via `formatTerminalLine`): `[USER_INPUT] …` on send; `[END_SESSION] …` on finish/end-session.
- **Retention**: interactive SDK session mode keeps `ChildStdin` on `EngineChild`; batch / CLI / one-shot may still use `Stdio::null()` or close after bootstrap.
- **`awaitFollowups` bootstrap + runtime control** (Codex / Claude / OpenCode):
  - Bootstrap via `resolve_await_session_followups(task_id)`: free employee session (no task) → `true`; **task-linked → `false`** (auto-exit if log never opens).
  - Runtime NDJSON control `{"type":"await_followups","enabled":true|false}` (commands `set_*_await_followups`): when **terminal log UI opens** on a live session, frontend enables wait; after turn the bridge emits `[SDK] 等待会话中输入...` and blocks on `nextLine()`.
  - When **log UI closes** while still live, frontend calls `finish_*_input` (close stdin / EOF) so the process exits and the task continues — same as End session.
  - Mid-turn `send_input` still queues follow-ups before exit/wait.
  - Do **not** advertise post-exit `send_input`. After exit, use resume/start.
  - Do **not** rely on `main()` return while stdin listeners are registered — always `exit(0)` after the chosen mode finishes.
- **Stop path**: `EngineChild::kill` closes retained stdin (EOF to bridge wait loop) then kills the child; stop commands also `kill_process_group`.
- **Grok B1**: headless `grok -p` + `Stdio::null`; matrix stays `false`; command returns Chinese reason (live vs no-session variants). Evidence: task `notes-b1-grok.md`.

### 4. Validation & Error Matrix
| Case | Behavior |
|------|----------|
| Blank / whitespace-only input | Reject in encoder (`输入内容不能为空`) before write |
| No live employee process | Chinese "没有运行中的 … 会话" |
| Live but no writable stdin / wrong channel (e.g. Codex CLI batch) | Chinese channel-specific reason; do not flip matrix for that channel |
| Grok any call | Always fail with B1 message; never resume-to-fake |
| Capabilities still loading (UI) | Fail-closed: bar disabled, **no** false "unsupported" flash |
| Session ended (not live) | UI bar stays visible, disabled; copy explains resume/new session |

### 5. Good / Base / Bad Cases
- Good: free employee session (no task) → after turn, wait for follow-ups / End session
- Good: task-linked start (with or without log open) → `awaitFollowups:false` → drain then exit → task/orchestration advances without manual End session
- Base: Grok live session → matrix `false`; UI disabled with reason; `send_grok_input` errors honestly
- Bad: matrix `true` while command is stub/`Stdio::null`; UI enabled without live session; faking send via `resume_*`; task run that waits on stdin when the log was never opened

### 6. Tests Required
- `engine/stdin.rs`: follow-up NDJSON shape + blank reject
- `EngineChild` stdin retain / write lifecycle
- Grok: live-unsupported + no-session error strings (`grok_send_input_error`)
- Capability matrix unit coverage for `send_input` flags
- `resolve_await_session_followups` (task-linked → false; no task → true)
- `is_orchestration_awaiting_session_exit` phase table (pipeline + review-fix + interactive)

### 7. Wrong vs Correct
#### Wrong
```rust
// Advertise support while stdin is null / stub always fails
send_input: true  // in get_ai_provider_capabilities for grok
// Or: on send_input, call start_*/resume_* to "continue" the chat
// Or: task-linked idle run awaits stdin (stuck unless user opens log / End session)
```
#### Correct
```rust
// Matrix matches a real live-process write path; unsupported engines stay false
send_input: false  // grok — B1
// send_*_input only writes retained stdin on the live ManagedProcess
// Free session: awaitFollowups true → wait on nextLine after turn
// Task-linked: awaitFollowups false → drain then exit(0)
```

## Lifecycle Expectations

Before launch:
1. Resolve employee + project + working directory
2. `validate_runtime_working_dir` for local paths
3. For SSH projects, build remote command path via `app/remote.rs` helpers
4. Persist session row with `ai_provider`, emit session events
5. Run cross-provider conflict checks **independently per engine** (do not short-circuit with `else if` chains that skip later managers)

During run:
- Stream output/error events to the frontend (`onCodexOutput` / `onClaude*` / `onOpenCode*` / `onGrok*`)
- Keep employee runtime status coherent (`busy` / online / error)
- When a usage event is parsed, runtime (not `stream.rs`) calls `apply_codex_session_usage` — stream parsers stay pure.

On exit:
- Finalize session record
- Trigger task automation transitions when applicable
- Allow UI stores to refresh tasks/employees

## Local vs SSH Execution

Constants in `app/shared.rs`:
- `EXECUTION_TARGET_LOCAL` / `EXECUTION_TARGET_SSH`
- Artifact capture modes: `local_full`, `ssh_full`, `ssh_git_status`, `ssh_none`

SSH realities encoded in product behavior:
- Password auth may be probed and gated (`password_execution_allowed`)
- Artifact capture can be limited; UI shows SSH mode banner
- Remote health commands live under `app/remote.rs` (Codex SDK install/health, OpenCode SDK install/health, Grok lightweight `grok --version`)

New engine features must not assume local filesystem diffs always exist.

### Claude-specific contracts

| Item | Contract |
|------|----------|
| Provider id | `"claude"` (label `Claude`) |
| Session mode (CLI fallback) | `claude -p ... --output-format stream-json --verbose --permission-mode bypassPermissions` |
| Print + stream-json | **Must** pass `--verbose` with `-p`/`--print` and `--output-format stream-json`; Claude CLI exits 1 otherwise |
| Args builder | Local + SSH share `build_claude_cli_args` in `claude/process/mod.rs` — keep them in sync |
| System prompt | `--system-prompt` when non-empty |
| Resume | `--resume <session_id>` when resuming |

### Grok-specific contracts

| Item | Contract |
|------|----------|
| Provider id | `"grok"` (label `Grok`) |
| Default model / effort | `grok-4.5` / `high` (effort: `low\|medium\|high`) |
| Session mode | Headless CLI only: `grok -p ... --output-format streaming-json --permission-mode bypassPermissions`；本地图片用 `--prompt-json`（base64 content blocks）；SSH 图片跳过 |
| System prompt | 仅 `--system-prompt-override`，不嵌进 user prompt |
| Local resolve | `GROK_CLI_PATH` / settings `cli_path_override` / known dirs including `~/.grok/bin` |
| SSH auth | Remote host must already have `grok` installed and authenticated (`grok login` or host-side env). **Do not inject `XAI_API_KEY` from the app.** |
| Local health | `check_grok_health` — version + `grok models` 登录态轻探测（`auth_ok`） |
| Remote health | `validate_remote_grok_health` — version + models 登录态；no remote install |
| Models | `list_grok_models` 解析 `grok models`，失败回落静态 `grok-4.5` |
| One-shot | Local + SSH supported via `codex/process/one_shot.rs`；固定 `--output-format json` |
| Settings file | `app_config_dir/grok-settings.json` |
| Frontend wrapper | `src/lib/grok.ts` (not only `backend.ts`) |
| Events | `grok-stdout`, `grok-stderr`, `grok-exit`, `grok-session` |

### OpenCode-specific contracts

| Item | Contract |
|------|----------|
| Provider id | `"opencode"` (label `OpenCode`) |
| Session mode | Node bridge only (no headless CLI): `node opencode_sdk_bridge.mjs`, config pushed as one JSON blob on **stdin** |
| Bridge modes | `session` / `resume_session` / `one_shot` (`mode` field of the stdin JSON) |
| Local channel | `launch_opencode_bridge` — local Node, `current_dir = settings.sdk_install_dir` |
| SSH channel | `launch_opencode_bridge_via_ssh` — remote Node + remote copy of the same `.mjs`; see [SSH Remote](./ssh-remote.md) “Remote OpenCode SDK bridge runtime” |
| Runtime model/effort | Written into `<run_cwd>/opencode.json` before launch, restored from backup on exit/failure (local: FS; SSH: over SSH) |
| Remote health / install | `validate_remote_opencode_health` / `install_remote_opencode_sdk`; local stays `check_opencode_sdk_health` |
| One-shot | Local `run_opencode_one_shot_via_sdk`; SSH `run_opencode_one_shot_via_remote_sdk` |
| Frontend wrapper | `src/lib/opencode.ts` |
| Events | `opencode-stdout`, `opencode-stderr`, `opencode-exit`, `opencode-session` |

Both channels must share `serialize_opencode_bridge_config` — the bridge protocol has exactly one serializer. Adding a field for local only silently desyncs SSH.

When adding another CLI engine, prefer **Claude/Grok process shape** over Codex SDK unless SDK is required.

## Scenario: Token usage parse + persist (A1)

### 1. Scope / Trigger
- Any engine stdout/SDK event that reports token counts. Persist onto `codex_sessions` before/during session end. Local and SSH share the same reader (no SSH-only parser).

### 2. Signatures
- `parse_usage_value(&Value) -> Option<UsageDelta>`
- `UsageDelta { input_tokens, output_tokens, total_tokens, reasoning_tokens: Option<u64> }`
- Persist: `crate::app::apply_codex_session_usage(pool, session_record_id, &delta)`
- Display: `UsageDelta::format_terminal_line() -> Option<String>` → `[用量] in=… out=…`

### 3. Contracts
- `parse_usage_value` accepts a wrapper (`usage` / `data`) or a bare usage object. Key fallbacks: `input_tokens` / `inputTokens` / `prompt_tokens` / `input`; same pattern for output/total/reasoning.
- Missing `total` may be derived as input+output when both exist.
- **Marker lines** (reuse `[CODEX_FILE_CHANGE]` style, **not** `[[codex-ai:usage]]`):
  - Codex SDK: `[CODEX_USAGE] {json}`
  - Claude SDK: `[CLAUDE_USAGE] {json}`
  - Codex CLI `--json`: `turn.completed` + `usage`
  - Claude CLI: `result` event + top-level `usage`
  - Grok: JSON `type=usage`
  - OpenCode: **SDK bridge only** (no second CLI stream). Bridge emits `{ "type": "usage", "data": { input_tokens, output_tokens, ... } }`; `stream_opencode_output` matches `"usage"`.
- Stream `let _ = apply_codex_session_usage(...)` — persist failure must not kill the session (same as other stdout event writes).

### 4. Validation & Error Matrix
| Case | Behavior |
|------|----------|
| Unrecognized JSON / no keys | `None` — columns stay NULL |
| Marker line with invalid JSON | skip persist |
| Empty delta | persist no-op |
| OpenCode CLI (does not exist) | N/A; do not invent a second parser |

### 5. Good / Base / Bad Cases
- Good: Codex `turn.completed` usage → session row + terminal `[用量]` line
- Base: engine never emits usage → NULL columns, UI hidden
- Bad: coerce parse miss to 0; put DB writes inside `process/stream.rs`; invent a unique `[[codex-ai:usage]]` marker that bypasses the existing `[CODEX_*]` convention

### 6. Tests Required
- `engine/usage.rs`: multi-key parse + OpenCode native `input`/`output` keys
- Codex: CLI `turn.completed` + `[CODEX_USAGE]` marker
- Claude: CLI `result` + `[CLAUDE_USAGE]` marker
- Grok: `type=usage` structured delta
- OpenCode: bridge `{"type":"usage","data":...}` in `opencode/process/mod.rs` tests

### 7. Wrong vs Correct
#### Wrong
```rust
// stream.rs opens sqlite and writes zeros on parse miss
let total = parsed.total.unwrap_or(0);
```
#### Correct
```rust
if let Some(delta) = parse_usage_value(&value) {
    let _ = apply_codex_session_usage(&pool, &session_id, &delta).await;
}
```

## Settings & Secrets

- Codex settings loaders support local and remote variants (`load_codex_settings`, `load_remote_codex_settings`)
- One-shot / AI-commit preferred provider lists live in Codex settings normalize (`SUPPORTED_ONE_SHOT_PROVIDERS` includes `grok`)
- Local/remote one-shot **health channel resolution** must branch on preferred provider (`resolve_local_one_shot_runtime` / `resolve_remote_one_shot_runtime`) — missing branches silently show Codex status for other providers
- Secrets use `codex/secret_store.rs` ref indirection — do not store raw passwords in SSH config rows beyond refs
- MCP server config is file-backed (`codex/mcp.rs` → `mcp-servers.json` under app config dir)
- Grok settings are separate (`grok/settings.rs`); do not fold Grok defaults into `codex-settings.json` except one-shot/git provider preference fields

## Task Automation Coupling

`task_automation.rs` orchestrates review → fix loops using engine sessions:
- modes like `review_fix_loop_v1`
- phases such as `launching_review`, `waiting_review`, `launching_fix`, `waiting_execution`, `committing_code`
- startup resumes pending automation via `spawn_resume_pending_automation`
- assignee/reviewer `ai_provider` must dispatch `start_*_with_manager` for **each** engine including `grok`

When changing session exit semantics, verify automation still observes the events it needs.

**Frontend refresh contract**: after automation commits a phase or task status change that the UI must show without a full page reload, call `emit_task_automation_state_changed` (`task-automation-state-changed`). Required on at least:
- successful launch finalize (`waiting_review` / `waiting_execution`)
- launch failures (`review_launch_failed` / `fix_launch_failed`)
- manual stop → `manual_control`
- existing terminal paths (`completed` / `blocked` / `commit_failed` / …)

`*-exit` alone is not enough: the process exit event often arrives **before** automation finishes writing task status/phase.

## Prompt / Review Integration

- Review launch and attachment handling: `app/review.rs`
- Prompt templates: `codex/prompt_templates.rs` + settings UI
- Coordinator/tester generation commands are backend AI one-shots exposed through `backend.ts` / engine wrappers

## Scenario: Add or extend a CLI engine (Grok reference)

### 1. Scope / Trigger
- New `ai_provider` value, Tauri start/stop commands, SSH remote exec, one-shot preference, or engine health IPC.

### 2. Signatures
- `start_<engine>(employee_id, task_description, model?, reasoning_effort?, system_prompt?, working_dir?, task_id?, ...)`
- `stop_<engine>` / `stop_<engine>_session`
- `get_<engine>_settings` / `update_<engine>_settings` / `check_<engine>_health`
- Optional: `validate_remote_<engine>_health(ssh_config_id)`
- Session row: `ai_provider` string on shared `codex_sessions`

### 3. Contracts
- Request: employee/task binding, model/effort normalized per engine
- Response: `Result<(), String>` for start/stop; settings/health DTOs in `db/models.rs` + `src/lib/types.ts`
- Env: engine-specific path overrides (e.g. `GROK_CLI_PATH`); SSH uses existing secret refs — not engine API keys in-app for Grok

### 4. Validation & Error Matrix
- Missing local binary → Chinese error instructing install/path
- SSH missing `ssh_config_id` / remote repo → hard error before spawn
- Remote unauthenticated CLI → surface stderr; for Grok mention `grok login`
- Cross-provider same employee/task conflict → reject start
- Unknown one-shot provider normalize → default `codex` (do not invent silent aliases)

### 5. Good/Base/Bad Cases
- Good: local Grok session streams events; SSH Grok after remote login
- Base: one-shot preferred provider `grok` runs Grok CLI local/SSH
- Bad: `else if` conflict checks that skip Grok; one-shot health still reporting Codex when provider is Grok

### 6. Tests Required
- Normalize provider/model/effort unit tests (`codex/settings.rs` style)
- Remote shell command construction escaping
- Stream parser lenient samples (unknown JSON → raw line)
- Regression: Grok SSH one-shot enabled; OpenCode SSH one-shot enabled via remote SDK bridge (`normalize_one_shot_provider("opencode", is_remote=true) == "opencode"`)

### 7. Wrong vs Correct
#### Wrong
```rust
// Short-circuits: ClaudeManager always present skips Grok check
if has_claude { Err(...) } else if has_grok { Err(...) }
// One-shot health ignores preferred provider
let channel = codex_runtime.one_shot_channel;
```
#### Correct
```rust
if has_claude { return Err(...); }
if has_grok { return Err(...); }
// Branch health on preferred provider
match one_shot_provider.as_str() {
  "grok" => inspect_grok_runtime(...),
  ...
}
```

## Anti-Patterns

- Starting processes without working-dir validation.
- Engine-specific session tables that fragment history UI.
- Blocking the main async runtime on long interactive CLI without the established process modules.
- Logging full prompts that embed secrets.
- Treating “not claude/opencode” as Codex in UI/backend branches (breaks Grok).
- Injecting third-party API keys over SSH when the product convention is remote self-auth (Grok/Claude CLI pattern).
- Forgetting one-shot normalize + health + settings UI when adding a provider to `SUPPORTED_ONE_SHOT_PROVIDERS`.
- Gating a provider on `if !is_remote` inside `normalize_one_shot_provider` — that silently rewrites the user's choice to `codex` instead of surfacing a reason. Gate on runtime availability and report it, don't rewrite the provider.
- Passing `None` for `ssh_config` into `capture_execution_change_baseline` on an SSH path — baseline capture then fails at runtime with a generic error while the code looks correct.
- Hard-failing an execution target with 「尚未实现」. Either implement the channel or return a specific Chinese reason the user can act on (missing Node, SDK not installed, missing `ssh_config_id`).
- Session SDK bridge returning from `main()` after drain while stdin stays open (listeners keep the event loop alive → process never exits → automation stuck on `[执行中]`). Always `exit(0)` after the drain-then-exit window.
