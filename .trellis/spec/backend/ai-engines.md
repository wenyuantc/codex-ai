# AI Engines

> Codex, Claude, OpenCode, and Grok share session storage and a process kernel; they diverge in stream protocols and CLI/SDK launch.

---

## Shared Kernel (`src-tauri/src/engine/`)

| Module | Responsibility |
|--------|----------------|
| `engine/context.rs` | `ExecutionContext` + `resolve_*_execution_context` / one-shot working dir (engine label injected into Chinese errors) |
| `engine/child.rs` | `EngineChild` + `EngineProcessHandle` trait (killpg / kill / try_wait / stdio) |
| `engine/manager.rs` | `ProcessManager<SessionKind, Extra>` + `ManagedProcess` + `EngineProcessRegistry` trait |
| `engine/status.rs` | `resolve_final_session_status` (stopping / exit 0 / failed) |

Rules:
- **Do not copy** manager / lifecycle / context implementations into a new engine — re-export or type-alias the shared kernel.
- Keep **`process/stream.rs` per engine** (CLI JSON protocols differ).
- Keep **`process/mod.rs` launch/CLI args** per engine unless a pure helper is already proven identical.
- Codex extras (`provider`, execution-change baseline, sdk file-change store) live in `CodexProcessExtra` on `ManagedProcess::extra`.
- OpenCode keeps `sdk_server` on `OpenCodeManager` (not in the generic process table).
- `start_*` / `stop_*` remain engine-specific Tauri commands; do not introduce a single `dyn AiEngine::start` dispatcher unless product scope expands.

## Engine Modules

| Engine | Root | Responsibilities |
|--------|------|------------------|
| Codex | `src-tauri/src/codex/` | Primary OpenAI Codex CLI/SDK integration, secrets, MCP config, prompt templates, one-shot routing |
| Claude | `src-tauri/src/claude/` | Anthropic Claude SDK bridge + process lifecycle (local + SSH CLI) |
| OpenCode | `src-tauri/src/opencode/` | OpenCode SDK server/process integration (SSH sessions limited) |
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
- Remote health commands live under `app/remote.rs` (Codex SDK install/health, Grok lightweight `grok --version`)

New engine features must not assume local filesystem diffs always exist.

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
| One-shot | Local + SSH supported via `codex/process/one_shot.rs`；固定 `--output-format json`（unlike OpenCode SSH one-shot which stays disabled） |
| Settings file | `app_config_dir/grok-settings.json` |
| Frontend wrapper | `src/lib/grok.ts` (not only `backend.ts`) |
| Events | `grok-stdout`, `grok-stderr`, `grok-exit`, `grok-session` |

When adding another CLI engine, prefer **Claude/Grok process shape** over Codex SDK unless SDK is required.

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
- Regression: OpenCode SSH one-shot remains disabled; Grok SSH one-shot enabled

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
