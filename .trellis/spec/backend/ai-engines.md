# AI Engines

> Codex, Claude, and OpenCode share session storage and diverge in process adapters.

---

## Engine Modules

| Engine | Root | Responsibilities |
|--------|------|------------------|
| Codex | `src-tauri/src/codex/` | Primary OpenAI Codex CLI/SDK integration, secrets, MCP config, prompt templates |
| Claude | `src-tauri/src/claude/` | Anthropic Claude SDK bridge + process lifecycle |
| OpenCode | `src-tauri/src/opencode/` | OpenCode SDK server/process integration |

Common internal layout per engine:
- `manager.rs` — in-memory runtime state held in Tauri `State`
- `settings.rs` — load/save engine settings
- `process/` — launch, stream, lifecycle, context
- `*_sdk_bridge.mjs` — Node bridge assets where needed

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
4. Persist session row, emit session events

During run:
- Stream output/error events to the frontend (`onCodexOutput`-style listeners)
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
- Remote SDK install/health commands live under `app/remote.rs`

New engine features must not assume local filesystem diffs always exist.

## Settings & Secrets

- Codex settings loaders support local and remote variants (`load_codex_settings`, `load_remote_codex_settings`)
- Secrets use `codex/secret_store.rs` ref indirection — do not store raw passwords in SSH config rows beyond refs
- MCP server config is file-backed (`codex/mcp.rs` → `mcp-servers.json` under app config dir)

## Task Automation Coupling

`task_automation.rs` orchestrates review → fix loops using engine sessions:
- modes like `review_fix_loop_v1`
- phases such as `launching_review`, `waiting_review`, `launching_fix`, `waiting_execution`, `committing_code`
- startup resumes pending automation via `spawn_resume_pending_automation`

When changing session exit semantics, verify automation still observes the events it needs.

## Prompt / Review Integration

- Review launch and attachment handling: `app/review.rs`
- Prompt templates: `codex/prompt_templates.rs` + settings UI
- Coordinator/tester generation commands are backend AI one-shots exposed through `backend.ts`

## Anti-Patterns

- Starting processes without working-dir validation.
- Engine-specific session tables that fragment history UI.
- Blocking the main async runtime on long interactive CLI without the established process modules.
- Logging full prompts that embed secrets.
