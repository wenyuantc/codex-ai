# Technical Design

## Boundaries

- `src-tauri/src/native/agent/`: context windows, compaction, shared rollout budget, child propagation, tool-result truncation.
- `src-tauri/src/native/model/`: protocol-specific continuation fields and request construction.
- `src-tauri/src/native/session.rs`: initialize runner limits, retain compatibility for one-shot/coordinator/session flows.
- `src-tauri/src/db/`: nullable session diagnostics migration and model/query updates when persisted diagnostics are needed.
- `src-tauri/src/native/settings.rs`, `src-tauri/src/db/models.rs`, `src/pages/SettingsPage.tsx`, locale packs: user-adjustable limits with backward-compatible defaults.

## Data Flow

1. Session startup loads limits and creates a shared `RolloutBudget` plus a `ContextWindow` state.
2. Before each model request, the runner estimates serialized input tokens and checks window/budget thresholds.
3. Threshold hit invokes remote summarization with no tools, then replaces old history with a compact summary and current task state. If summarization cannot run, a local structured reset is used.
4. Model usage updates the shared budget. Child runners receive the same budget handle and their own window identifier.
5. Tool execution emits the full result to the event stream, but appends a token-bounded head/tail representation to model messages.
6. Session completion persists nullable diagnostics and emits one activity entry with the Chinese action label.

## Compatibility

- Responses/Codex requests carry a continuation identifier when the channel supports it; absence of the identifier falls back to the existing full-message request.
- Chat/Anthropic retain current wire shapes and use compacted local messages.
- Existing `native-settings.json` fields remain valid; missing limits use conservative defaults without changing explicit employee model settings.
- SSH uses the same runner and budget types; only tool execution remains remote.

## Failure Handling

- Summary request errors never lose the current user turn; fall back to local reset and emit a diagnostic line.
- Unknown/missing usage does not decrement the budget; an estimation guard still prevents unbounded context growth.
- Cancellation stops summary, child, and model requests through the existing `CancelFlag`.
- Nullable diagnostics remain unknown rather than being coerced to zero.
