# AI Engines

> Codex, Claude, OpenCode, and Grok share session storage and a **process kernel** (`EngineChild`). The fifth provider `native` (UI: 内置 Agent) is an **in-process** tokio loop: it shares `codex_sessions` and `ExecutionContext`, but must **not** be registered in `engine::ProcessManager`.

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
- **Do not copy** manager / lifecycle / context implementations into a new **CLI** engine — re-export or type-alias the shared kernel.
- Keep **`process/stream.rs` per engine** (CLI JSON protocols differ).
- Keep **`process/mod.rs` launch/CLI args** per engine unless a pure helper is already proven identical.
- Codex extras (`provider`, execution-change baseline, sdk file-change store) live in `CodexProcessExtra` on `ManagedProcess::extra`.
- OpenCode keeps `sdk_server` on `OpenCodeManager` (not in the generic process table).
- `start_*` / `stop_*` remain engine-specific Tauri commands; do not introduce a single `dyn AiEngine::start` dispatcher unless product scope expands.
- Task-execution `start_*` returns tagged `StartSessionOutcome` (`started` \| `queued`). Gate/queue rules live in [run-queue.md](./run-queue.md), not in five engine copies.
- **`native` is not a CLI engine.** Do not wrap it in `EngineChild` / `ProcessManager`. Live sessions live in `NativeAgentManager` (HashMap of cancel + followup mpsc + `JoinHandle`). Count those sessions in `run_queue` live total.

## Engine Modules

| Engine | Root | Responsibilities |
|--------|------|------------------|
| Codex | `src-tauri/src/codex/` | Primary OpenAI Codex CLI/SDK integration, secrets, MCP config, prompt templates, one-shot routing |
| Claude | `src-tauri/src/claude/` | Anthropic Claude SDK bridge + process lifecycle (local + SSH CLI) |
| OpenCode | `src-tauri/src/opencode/` | OpenCode SDK server/process integration; local Node bridge + **remote SDK bridge over SSH** |
| Grok | `src-tauri/src/grok/` | xAI Grok Build CLI (`grok`) headless sessions + settings/health (local + SSH) |
| Native | `src-tauri/src/native/` | In-process coding agent: channels + three HTTP protocols + tool loop + `NativeAgentManager` |

Common internal layout per **CLI** engine:
- `manager.rs` — thin wrapper / type alias over `engine::ProcessManager` (Codex/OpenCode may wrap extras)
- `settings.rs` — load/save engine settings
- `process/` — launch (`mod.rs`), stream, session_runtime; lifecycle/context re-export shared kernel
- `*_sdk_bridge.mjs` — Node bridge assets where needed (Codex/Claude/OpenCode; Grok is CLI-only)

`ai_provider` stored values: `"codex" | "claude" | "opencode" | "grok" | "native"`.

`normalize_employee_ai_provider` **must** recognize `"native"`; unknown values still default to `"codex"`. Frontend `normalizeAiProvider` must do the same — otherwise a native employee silently starts Codex.

`normalize_one_shot_provider_for_target` **must** recognize `"native"`. Employee-scoped one-shots pass the employee's provider; if that employee is `native`, **never** remap `native` → Codex SDK/exec. Missing `employee_id` is a Chinese error, not a Codex fallback.

- **Tester / generic one-shot** (`ai_generate_tester_acceptance`, commit message, prompt optimize): `native::run_native_one_shot` — HTTP `ModelClient.chat()`, `tools: &[]`.
- **Coordinator plan** (`ai_generate_coordinator_task_plan`): `native::run_native_read_only_one_shot` — in-process `AgentRunner` with read-only tools (`Read`/`Glob`/`Grep`/`Todo*`/`WebFetch`/`WebSearch`). Insert a `codex_sessions` row with `session_kind=coordinator` and apply token usage (`apply_codex_session_usage`); persist progress lines to `codex_session_events`. Do **not** register `NativeAgentManager`, do **not** gate run-queue, do **not** call `handle_session_exit`. SSH uses `SshToolRuntime`. Progress lines also go to `ai-command-stdout` (not `native-stdout`). Write/Edit/Bash/MCP are rejected. If working dir is missing, fall back to HTTP no-tools and emit a warning line.

Settings Git / one-shot dropdowns still use `CLI_AI_PROVIDER_OPTIONS` (exclude `native`).

Native employees **must** bind `employees.ai_channel_id` to an enabled `ai_channels` row. Other providers store `ai_channel_id = NULL`.

### Native custom sub-agents

App-global catalog in `native-subagents.json` (not SQLite). Settings tab CRUD: `list/create/update/delete_native_subagent`. Name is `Agent.subagent_type` and stays globally unique. Built-in `general` / `explore` stay reserved. `model_mode=inherit` uses the parent session client; `channel` builds a new `ModelClient` from that `ai_channels` row (HTTP stays on this machine; SSH file tools still use the parent workspace). `tool_mode=all` matches `general` (built-in tools minus `Agent` + parent MCP). `tool_mode=custom` is the 9-tool whitelist; `TodoWrite` implies `TodoRead`; no MCP. Custom system prompt replaces parent identity; env/Git always inject; `inject_agents_md` (default true) controls project AGENTS.md. Depth remains 1. Coordinator read-only one-shot still has no `Agent` tool.

Scope: `scope=all` (default; missing JSON field deserializes as all) is visible to every project. `scope=projects` plus non-empty `project_ids` (live `projects.id` where `deleted_at IS NULL`) limits the task picker and the parent Agent catalog to those projects. Sessions without a task only see `all`. An explicit task bind still runs (and stays in the catalog) if the agent is later edited out of that project. New binds must currently match the task's project.

`normalize_employee_reasoning_effort` **must not** reuse Grok's `low|medium|high` whitelist for `native`. Built-in Agent thinking levels come from the model catalog (`none` / `no_think` / `minimal` / `low` / `medium` / `high` / `xhigh` / `max`). Grouping `native` with Grok silently maps `xhigh`/`max` to `high`, so employee "最高/极高" saves have no effect. Session HTTP already forwards `xhigh`/`max` (`openai::normalize_effort`, Anthropic budget map). Frontend `normalizeReasoningEffortForProvider` must keep the same split.

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
| `start` / `stop` / `resume` | Per-engine session lifecycle (all **five** providers) |
| `restart` | Stop live managed processes for the employee, then call that engine's `start_*` (**not** CLI resume of the old session id) |
| `send_input` | Mid-session write to a **live** session. **Codex / Claude / OpenCode = true** (SDK bridge keeps stdin; NDJSON follow-ups). **Native = true** (in-process `mpsc` `NativeFollowup::Input`). **Grok = false** (B1: headless `-p` + `Stdio::null`). Do not set `true` without a verifiable write path; never advertise a capability that always fails. CLI-only Codex/Claude sessions may still reject at command time with a clear Chinese error. |
| `mcp` | App-managed MCP from `mcp-servers.json` + `tasks.mcp_server_ids`. **Codex = true** (CLI `-c mcp_servers.*` on local and SSH). **Native = true** (stdio MCP tools injected into the in-process loop: local spawn on this machine, SSH spawn on the remote host via `build_ssh_command` / `spawn_ssh_stdio_command`; handshake failure skips that server and must **not** fall back to local MCP). **Claude / OpenCode / Grok = false** — Settings/task binding may still save the list, but UI must not claim those engines will run it. |

UI rules: Settings shows the five-engine comparison; any control for restart/send_input must gate on the matrix (`can(provider, cap)`) with Chinese disabled reasons. Prefer fail-closed when the matrix fails to load. Git / one-shot **settings** dropdowns use `CLI_AI_PROVIDER_OPTIONS` (exclude `native`). That exclusion does **not** apply when a coordinator/tester **employee** is `native` — those one-shots use the employee's channel HTTP path.

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
| `send_native_input` | same | `mpsc` `NativeFollowup::Input` to the live runner |
| `finish_codex_input` / `finish_claude_input` / `finish_opencode_input` | `employee_id: String` | Close retained stdin (EOF) so interactive wait exits; process ends → orchestration can advance |
| `finish_native_input` | `employee_id: String` | Cancels + joins **all** live native sessions for that employee (stop, not EOF). UI: SessionInputBar **结束** only. Do **not** call it when closing a log (`useSessionLogAwaitFollowups` skips native like grok). |

Frontend wrappers: `send*` / `finish*` in `src/lib/{codex,claude,opencode,grok,native}.ts`. Unified start/stop: `src/lib/aiEngine.ts` `startByProvider` / `stopSessionByProvider` / `restartByProvider` — unknown provider **throws**, never falls through to Codex. Shared UI: `SessionInputBar` — **Send** queues follow-up; **结束会话 / End session** closes stdin (CLI) or stops native. Hosts: TaskLog / SessionLog / EmployeeRunningSessions (+ review panel).

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
- **Stop path**: `EngineChild::kill` closes retained stdin (EOF to bridge wait loop) then kills the child; stop commands also `kill_process_group`. Persist `stopping_requested` **before** kill. Native has no child: `stop_native_process` must insert `stopping_requested` (user stop / `finish_native_input`) or `automation_restart_requested` (automation restart) **before** cancel. Native cancel still exits `exited`/`0`; without that event `handle_session_exit` treats success and auto-starts review. Restart is checked before stopping.
- **Grok B1**: headless `grok -p` + `Stdio::null`; matrix stays `false`; command returns Chinese reason (live vs no-session variants). Evidence: task `notes-b1-grok.md`.
- **Native**: no `ChildStdin`. Follow-ups are `NativeFollowup::{Input, Finish}` on an mpsc. User stop / `finish_native_input` persist `stopping_requested` before cancel; automation restart persists `automation_restart_requested`. Do not skip that write — cancel exits `0` and otherwise auto-reviews. Task-linked start uses `await_followups=false` (exit after one turn). Free sessions wait on the channel. Images: local files read as base64 on the first user message (OpenAI `image_url` / Anthropic `image.source` / Responses `input_image`). SSH does **not** sync images to the remote host — the HTTP call stays on this machine. Cap 8 images / 8MB each; missing or oversized files are logged and skipped. Mid-session `send_input` is text-only. High-risk tools wait on `native-permission-request` (`allow_session | allow_once | deny`). **Plan run** (`start_native_session` `plan_mode=true`, kanban right-click when assignee is native): first `run_with_client` is `read_only` (Read/Glob/Grep/Todo/Web + `AskQuestion`; MCP extra tools omitted). `AskQuestion` blocks on `native-plan-question` until `answer_native_plan_question` (answers or skip). If the model never calls it, the plan turn ends and the same loop auto-executes. Failure/cancel/stop skip the execute turn. Do not write `tasks.plan_content` or start the coordinator pipeline. `QueuedTaskRun.plan_mode` (serde default false) must survive drain. Bound task sub-agents are not required during the plan turn; wrap them on the execute follow-up. Coordinator read-only one-shot has no `AskQuestion`. Terminal `[工具结果]` is **one** stdout event with the full tool output (not the first line). TodoWrite list results stay as a count because the `[待办]` start line already lists items. Display-only cap: 2000 lines or 65536 chars, then `…（已截断，共 N 行 / M 字）`. The model still receives the uncapped `tool` message. Do not split the result into per-line events (`taskLogs` keeps 199 entries). `truncate_messages` is context compression for the model, not the UI.
- **Claude images**: local SDK keeps `claude_sdk_bridge.mjs` image blocks. Local CLI attaches images via `--input-format stream-json` (no `--image` flag). SSH Claude still skips images. High-risk tools (overwrite/delete/push/force git/MCP) emit `native-permission-request` and wait for `allow_session | allow_once | deny`.
- **Claude images**: local SDK still uses `claude_sdk_bridge.mjs` image blocks. Local CLI uses `--input-format stream-json` with Anthropic `type:image` base64 content (no `--image` flag). SSH Claude still skips images and must keep the preflight warning.

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
- Native `parse_max_output_token_limit` + `chat()` one-shot retry when gateway rejects oversized `max_tokens`
- Native `RetryConfig` default 10×3s: 503 then success; HTTP 200 gateway error then success; 401 no retry; retry line `[重试]` `n/10`; cancel during wait returns `已取消`
- Native HTTP 200 JSON completion / wrapped `data.choices` / 200 `error.message` / Responses `response.completed` without deltas; packed SSE `data:` lines without blank separators
- DeepSeek V4 `thinking.type` enabled/disabled in OpenAI body; one-shot uses plan-shaped `reasoning_content` or retries with thinking off; empty content error mentions 思考内容 not timeout
- Native execution Git snapshot: capture only for `session_kind=execution`; review does not; persist uses Cli/`git_fallback` like Grok
- `normalize_one_shot_provider_for_target("native")` stays `"native"`; native one-shot without `employee_id` errors in Chinese (no Codex fallback)
- Native session startup banner includes channel/protocol/model/effort; coordinator plan runtime label uses `内置 Agent` not `native`

### 7. Wrong vs Correct
#### Wrong
```rust
// Advertise support while stdin is null / stub always fails
send_input: true  // in get_ai_provider_capabilities for grok
// Or: on send_input, call start_*/resume_* to "continue" the chat
// Or: task-linked idle run awaits stdin (stuck unless user opens log / End session)
// Or: coordinator ai_provider=native remapped to Codex SDK/exec in run_ai_command
Some("native") => "codex".to_string()  // in normalize_one_shot_provider_for_target
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
- Stream output/error events to the frontend (`onCodexOutput` / `onClaude*` / `onOpenCode*` / `onGrok*` / `onNative*`)
- Keep employee runtime status coherent (`busy` / online / error)
- When a usage event is parsed, runtime (not `stream.rs`) calls `apply_codex_session_usage` — stream parsers stay pure.
- Native: `client.chat()` already returns `Usage` (including `cached_tokens` from `native/model/usage.rs`). `AgentRunner` converts it with `usage_to_delta` after each turn, emits `[用量]`, and persists via `on_usage` → `apply_codex_session_usage`. Do not invent a native-only usage table. `chat_stream` is a replay helper and is **not** the persist path.
- Native HTTP 400 `max_tokens is too large` / `supports at most N`: `chat()` retries **once** with `max_output_tokens = N`. Catalog max is capacity, not a gateway guarantee (e.g. DeepSeek V4 catalog 384000 vs Console Go 131072). Do not treat the first 400 as a terminal Auto QC failure if the limit can be parsed.
- Native HTTP **200** body parsing (`ModelClient::parse_success_body`): try protocol SSE, then a complete JSON object (optional one-layer `data`/`result` wrapper). A JSON `error` object on 200 is a gateway error (`模型返回错误：…`), **not** `模型返回空响应`. Responses must also read `response.output_text.done` and `response.completed.output` when deltas are missing. Empty is last-resort and should include a redacted body snippet. Do not treat “gateway returned JSON instead of SSE” as an empty model. Coordinator/tester one-shots share this client — a stub JSON fallback will fail plan generation with `内置 Agent 一次性调用失败：模型返回空响应`.
- Native API retry (`RetryConfig::default`): **10 retries**, **fixed 3s**, no jitter. `post_stream` retries network/timeout, HTTP 408/409/429/5xx, and retryable HTTP 200 gateway/empty/parse failures. Do **not** retry 401/403/404/other 4xx, `insufficient quota` / `invalid api key` / `unauthorized`. If `parse_max_output_token_limit` matches, return immediately so `chat()` can do its one-shot limit downgrade — do not spend the 10-retry budget. Channel `probe` keeps `RetryConfig::none()`. Before each sleep, emit `[重试] {error}，3 秒后进行第 n/10 次重试` via `on_retry` → `AgentRunner.emit` → `native-stdout` (same path as `[思考]` / `[ERROR]`). Poll `CancelFlag` during the wait so Stop does not block for a full 3s. Tester `run_native_one_shot` still retries HTTP but has no session terminal.
- DeepSeek V4 (`deepseek-v4-pro` / `deepseek-v4-flash`) **defaults thinking ON**. OpenAI-compat body must send `thinking: {type: enabled|disabled}`; omitting it leaves thinking on. `reasoning_effort` is low/high/max (not `none`). One-shot `未返回可用内容` after a successful parse means empty `content` (often reasoning-only at `max`), **not** HTTP timeout (`模型请求失败`). One-shot: prefer `content`; if empty, use reasoning when it looks like JSON/Markdown; else retry once with thinking disabled. Thinking sessions/one-shots use a 300s HTTP timeout; non-thinking stays 120s.
- Auto QC execution `status != exited || exit_code != 0` still hands off to manual, but **must keep** `last_verdict_json` so a later restart can still run the fix round.

On exit:
- Finalize session record
- Review sessions: persist tagged `<review_verdict>` / `<review_report>` / `<review_findings>` **before** `handle_session_exit`. Native has no captured stream buffer — call `persist_review_session_events_from_session_logs` after stdout events are flushed.
- Native **execution** sessions: capture a Git working-tree baseline at start (local + SSH, same helpers as Grok: `capture_external_execution_change_baseline` / `capture_external_remote_execution_change_baseline`). On exit call `persist_external_execution_change_history` with `CodexExecutionProvider::Cli` and that baseline **before** `handle_session_exit`. Review sessions skip capture. Do **not** skip this path — the task detail “改动文件” panel shows `emptyGit` when `codex_session_file_changes` is empty.
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

When adding another **CLI** engine, prefer **Claude/Grok process shape** over Codex SDK unless SDK is required. A process-in-app agent belongs under `native/`, not a fifth `EngineChild`.

### Native-specific contracts

| Item | Contract |
|------|----------|
| Provider id | `"native"` (UI label `内置 Agent`) |
| Runtime | In-process tokio task. **No** sidecar binary, **no** `EngineChild`, **no** remote agent install |
| Channels | Table `ai_channels` (migration **v48**). Protocols `openai` / `anthropic` / `codex`. `base_url` trimmed of trailing `/`; HTTP path appended by protocol. `models_json` is an array of `{id, context_tokens, max_output_tokens, thinking_enabled, thinking_level}` (legacy string arrays still parse). Bundled catalog: `native/model_catalog.json` via `list_model_catalog` |
| HTTP | openai → `POST {base}/v1/chat/completions` SSE; anthropic → `POST {base}/v1/messages` SSE + `anthropic-version: 2023-06-01`; codex → `POST {base}/v1/responses` SSE |
| Secrets | SQLite `ai_channels.api_key` (plaintext config). DTO returns `api_key` + `api_key_configured`. Legacy `api_key_ref` / keyring `codex-ai-channel` is a one-time migrate-on-read; new writes do not touch the OS keyring. SQL backup includes the key |
| Employee bind | `employees.ai_channel_id` required iff `ai_provider=native` and channel enabled. Delete channel fails while referenced |
| Tools | `Read` `Write` `Edit` `Bash` `Glob` `Grep` `TodoRead` `TodoWrite` `WebFetch` `WebSearch` `Agent`. Workspace-only (file tools). Permission = confirm-high-risk (overwrite/delete/push/force git/MCP). `Agent` spawns an in-process child loop (depth 1, only **`general`** or **`explore`**); concurrent children per turn come from `max_concurrent_subagents` (default **3**, range **1–16**). How often the parent *chooses* to call `Agent` is steered by `subagent_policy`. It does **not** insert `codex_sessions` or occupy the run-queue. Web tools run on this machine even for SSH projects |
| System prompt | `Message::system` = identity.md + **子 Agent 策略块** (`subagent_policy`) + env + optional git + **全局提示词模板** `native_agent_global` (`ai-prompt-templates.json`) + project `AGENTS.md`/`Agents.md`/`CLAUDE.md` + employee `system_prompt`. Task text stays in the user message. SSH reads project files via `SshToolRuntime`. Do **not** load `.zcli/AGENTS.md` |
| Compact | After tool-result truncation, if chars ≥ 85% of `context_char_limit`, summarize older user-turns locally and keep the latest turn; log `[工具] 已压缩上下文` |
| Images | First user message may include local files as base64. Native never uses remote image paths. Frontend does **not** skip `native` in `resolveImageAttachmentSkip`. |
| One-shot | Employee-scoped (`ai_generate_coordinator_task_plan`, `ai_generate_tester_acceptance`): `run_native_one_shot` → channel HTTP `chat()`, **no tools**, no `NativeAgentManager` session, no Codex SDK/exec. Return text + optional `[用量]` line. Coordinator plan UI shows `内置 Agent / {model} / {effort}` then `[计划] 用量：…`. Local images only. SSH: HTTP still on this machine. Settings one-shot dropdown stays CLI-only. |
| SSH tools | Loop stays local. File/shell via `SshToolRuntime` + `build_ssh_command` / `execute_ssh_command_with_input`. Workspace prefix is the remote cwd |
| Commands | `start_native_session` `stop_native_session` `stop_native` `restart_native_session` `resume_native_session` `send_native_input` `finish_native_input` |
| Internal start | Automation / run-queue drain call `start_native_with_manager` (bypasses gate). Restart-safe stop: `stop_native_for_automation_restart` |
| Events | `native-stdout`, `native-session`, `native-exit` (no stderr event). First stdout line: `[内置 Agent] 启动会话 渠道=… 协议=… model=… effort=… thinking=on\|off` (Grok-style banner). Then `[USER_INPUT]`, Claude/Grok-compatible tool tags (`[读取]`/`[写入]`/`[编辑]`/`[命令]`/`[工具]`/`[工具结果]`/`[思考]`/`[待办]`), child-agent lines prefixed `[子 Agent {n}({explore|general}) - {description}]` (description = Agent tool short title, collapsed whitespace, max 32 chars), `[用量]`, assistant text, `[ERROR]`. `native-exit.line` is **null** (do not duplicate `[ERROR]` into the log). Tool-loop cap from `native-settings.json` `max_turns` (default **40**, **0** = unlimited, last turn sends no tools and asks for a final answer instead of failing the session). High-risk tools wait on `native-permission-request` (`allow_session | allow_once | deny`); concurrent confirms are FIFO. |
| Settings | Local file `app_config_dir/native-settings.json`. Commands `get_native_settings` / `update_native_settings`. Fields: `max_turns` (default **40**, **0** = unlimited), `confirm_high_risk` (default **true**; false skips the high-risk permission dialog for sessions started after the change), `max_concurrent_subagents` (default **3**, range **1–16**), and `subagent_policy` (`conservative` \| `balanced` \| `aggressive`, default **balanced**). UI: Settings 界面与运行. Policy only changes system prompt + Agent tool description (does not force tool calls). Not per-SSH-profile: the loop always runs on this machine. Activity key `native_settings_updated`. |
| Frontend | Channel CRUD in `src/lib/backend.ts`; session IPC in `src/lib/native.ts`; start/stop via `aiEngine.ts` |
| Settings UI | `AiChannelsSettingsTab`; employee dialogs bind enabled channels + `models` from `models_json` |

Channel CRUD commands: `list_ai_channels` `create_ai_channel` `update_ai_channel` `delete_ai_channel` `test_ai_channel` `list_ai_channel_models`. `list_ai_channel_models` uses GET `{base}/v1/models` with the same auth as chat (Bearer or `x-api-key`); it fills the settings form and does **not** write `models_json` until Save. Activity keys: `ai_channel_created` / `updated` / `deleted` / `tested` / `models_fetched`.

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
- MCP server config is file-backed (`codex/mcp.rs` → `mcp-servers.json` under app config dir). Codex consumes it at session launch. Native injects listed tools via `native/tools/mcp.rs` (local stdio or SSH remote stdio). Other engines must not silently ignore a UI that claims MCP is on.
- Grok settings are separate (`grok/settings.rs`); do not fold Grok defaults into `codex-settings.json` except one-shot/git provider preference fields

## Task Automation Coupling

`task_automation.rs` orchestrates review → fix loops using engine sessions:
- modes like `review_fix_loop_v1`
- phases such as `launching_review`, `waiting_review`, `launching_fix`, `waiting_execution`, `committing_code`
- startup resumes pending automation via `spawn_resume_pending_automation`
- assignee/reviewer `ai_provider` must dispatch `start_*_with_manager` for **each** engine including `grok` and `native` (`start_native_with_manager`)

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

## Scenario: Built-in native agent (in-process)

### 1. Scope / Trigger
New `ai_provider=native`, `ai_channels` / keyring, in-process tool loop, or any start/stop/send_input/run-queue/automation branch that must not fall through to Codex.

### 2. Signatures
- `list_ai_channels` / `create_ai_channel` / `update_ai_channel` / `delete_ai_channel` / `test_ai_channel` / `list_ai_channel_models`
- `start_native_session(employee_id, task_description, model?, reasoning_effort?, system_prompt?, working_dir?, task_id?, task_git_context_id?, resume_session_id?, image_paths?, session_kind?) -> StartSessionOutcome`
- `stop_native_session(session_record_id)` / `stop_native(employee_id)` / `restart_native_session` / `resume_native_session`
- `send_native_input(employee_id, input)` / `finish_native_input(employee_id)`
- Schema v48: `ai_channels(...)`; `employees.ai_channel_id TEXT REFERENCES ai_channels(id)`

### 3. Contracts
- Request: native employee + enabled channel + keyring key + validated working dir (local `.git` or SSH via `ExecutionContext`)
- Response: `StartSessionOutcome` for start/restart/resume; other commands `Result<(), String>`
- Env: none for API keys. Keyring service `codex-ai-channel`, user `api_key_ref`
- Protocols: `openai` | `anthropic` | `codex` (aliases `openai-compatible` → openai; `claude` → anthropic; `responses` → codex)
- Tools stay inside workspace root; SSH uses remote cwd as root. Loop never SSHs an agent binary

### 4. Validation & Error Matrix
| Case | Behavior |
|------|----------|
| Employee not `native` | `员工不是内置 Agent` |
| Missing / empty `ai_channel_id` | `请先为内置 Agent 员工配置渠道` / create employee `内置 Agent 员工必须选择已启用的渠道` |
| Channel disabled | `渠道「…」已停用` |
| Missing keyring key | `渠道未配置 API 密钥` / `渠道 API 密钥不存在或无法读取` |
| Invalid protocol / base_url | Chinese validation from `native/protocol.rs` |
| Delete channel still bound | Count employees with `ai_channel_id`; refuse with Chinese reason |
| Blank `send_native_input` | `输入内容不能为空` |
| No live native session | `员工 … 当前没有运行中的内置 Agent 会话` |
| Unknown frontend provider | `startByProvider` / SessionInputBar throw `未知 AI 引擎` — never Codex |

### 5. Good / Base / Bad Cases
- Good: create enabled openai channel → native employee binds it → start task → `codex_sessions.ai_provider=native` + stdout tool lines → stop cancels the join
- Good: SSH project — tools run via `build_ssh_command`; no remote `native` binary
- Base: free session (`task_id` none) waits on mpsc; `send_native_input` starts the next turn
- Bad: `else { start_codex }` after grok/opencode checks; `EngineChild` for native; key in `ai_channels.api_key`

### 6. Tests Required
- Migration 48: `ai_channels` columns + `employees.ai_channel_id`
- Protocol normalize + URL join (`protocol.rs`)
- Secret redaction never leaks `sk-` in errors
- Glob matcher includes `**/*.rs`
- Tool workspace-only + SSH command builders
- Capability matrix length **5**, native `send_input=true`
- `normalize_employee_ai_provider("native") == "native"`
- Run-queue drain `provider == "native"`
- Frontend: `normalizeAiProvider("native")`, `formatAiProviderLabel`, image skip `native_images`

### 7. Wrong vs Correct
#### Wrong
```rust
if provider == "claude" { start_claude }
else if provider == "grok" { start_grok }
else { start_codex } // native silently becomes Codex
// or: manager.add(EngineChild::spawn("native-agent", ...))
```
#### Correct
```rust
match provider {
  "native" => start_native_with_manager(...),
  "grok" => start_grok_with_manager(...),
  // ...
  other => return Err(format!("未知 AI 引擎：{other}")),
}
```

## Anti-Patterns

- Starting processes without working-dir validation.
- Engine-specific session tables that fragment history UI.
- Blocking the main async runtime on long interactive CLI without the established process modules.
- Logging full prompts that embed secrets.
- Treating “not claude/opencode/grok” as Codex in UI/backend branches (breaks Grok **and native**).
- Registering native live sessions in `engine::ProcessManager` / spawning a sidecar for the built-in agent.
- Storing channel API keys in SQLite or SQL export (`api_key_ref` + keyring only).
- Calling `finish_native_input` when a log dialog closes — that command **stops** the employee’s native sessions; skip native in `useSessionLogAwaitFollowups`.
- Injecting third-party API keys over SSH when the product convention is remote self-auth (Grok/Claude CLI pattern).
- Forgetting one-shot normalize + health + settings UI when adding a provider to `SUPPORTED_ONE_SHOT_PROVIDERS`. (`native` is **not** a one-shot/Git-commit provider; keep it out of `CLI_AI_PROVIDER_OPTIONS`.)
- Gating a provider on `if !is_remote` inside `normalize_one_shot_provider` — that silently rewrites the user's choice to `codex` instead of surfacing a reason. Gate on runtime availability and report it, don't rewrite the provider.
- Passing `None` for `ssh_config` into `capture_execution_change_baseline` on an SSH path — baseline capture then fails at runtime with a generic error while the code looks correct.
- Hard-failing an execution target with 「尚未实现」. Either implement the channel or return a specific Chinese reason the user can act on (missing Node, SDK not installed, missing `ssh_config_id`).
- Session SDK bridge returning from `main()` after drain while stdin stays open (listeners keep the event loop alive → process never exits → automation stuck on `[执行中]`). Always `exit(0)` after the drain-then-exit window.
