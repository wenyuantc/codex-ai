# send_input — Design

## Goal

Real mid-session write path for as many engines as feasible (B + B1); UI U1 inline composer on live terminals.

## Architecture

1. **Shared process kernel**: keep stdin handle alive on interactive sessions (`EngineChild` / manager), not `Stdio::null()` + immediate shutdown for interactive mode.
2. **Per-engine adapter**: protocol framing differs (CLI prompt vs SDK JSON). Prefer kernel ownership of pipe + engine-specific `write_input` framing.
3. **IPC**: implement real `send_codex_input`; add `send_claude_input` / `send_grok_input` / `send_opencode_input` (or one routed command with provider). Fail with clear Chinese errors when unsupported/no live session.
4. **Capabilities**: flip `send_input` only after smoke path exists; document exemptions with evidence in task notes + analysis doc.
5. **UI**: `SessionInputBar` under `CodexTerminal` in TaskLog / SessionLog / EmployeeRunningSessions; gate with `can(provider, "send_input")` + live process presence.

## Constraints

- Must not fake via resume/new session
- SSH: implement if bridge supports; else disable + reason
- Bottom line: ≥2 engines true including Codex

## Tradeoffs

- Interactive mode may require separate launch flags vs one-shot batch; prefer additive mode over breaking existing automation batch runs
- Grok/CLI-only engines may hit B1 exemption

## Rollback

- Matrix back to false; remove UI bar; restore null stdin spawn for batch
