# Implementation Plan

1. Add pure token-estimation, truncation, context-window, and shared-budget helpers with unit tests.
2. Replace `compact_local`/`truncate_messages` integration in `AgentRunner` with threshold checks, remote summary fallback, local reset, and budget-aware child runners.
3. Add protocol continuation support where the existing Responses client can safely carry it; preserve fallback for stateless channels.
4. Add nullable session diagnostics migration/models/queries and activity labels, keeping SSH and coordinator flows aligned.
5. Add settings fields and Settings UI controls with backward-compatible normalization and bilingual copy.
6. Run focused Rust tests, then clippy, format check, frontend build, and a Tauri smoke test; compare a simple task against the 490万-token baseline.

## Review Gates

- No model request may continue after shared budget exhaustion.
- No full tool result may be appended to model history when it exceeds its configured token budget.
- Compaction must preserve the current task and system constraints.
- No direct frontend IPC or SQLite writes; all persistence goes through existing Rust commands.
