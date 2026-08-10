# i18n I2 leftovers tracker

Strings still hard-coded Chinese (or mixed) after the I2 pass. Track here so a
follow-up extraction (post `send_input` / U1) can finish them without dual sources.

## Intentionally deferred (deep dialogs / git UX)

- Most `src/components/git/*` dialog copy (commit/merge/rollback confirmations)
- Most `src/components/projects/ProjectGit*` dialogs
- Large settings tab body copy beyond theme/language chrome (`RuntimeSettingsTab` Codex/Claude/OpenCode/Grok panels, `GitAutomationSettingsTab`, `McpSettingsTab`, `PromptSettingsTab`, `DatabaseSettingsTab`, `SshSettingsTab` field labels)
- Task detail panel dense copy (`TaskReviewPanel`, `TaskPipelineProgress`, `TaskDeliverySection`, MCP binding dialogs)
- Create/Edit task & employee dialog field labels beyond primary CTAs
- Shortcut help dialog descriptions (`ShortcutsHelpDialog`)
- Monaco / file preview chrome strings
- `ProjectDetailPage` dense project/git surfaces
- `ActivityListDialog` filter chrome (feed labels already i18n via `getActivityActionLabel`)
- `TASK_STATUSES` / `PRIORITIES` `.label` fields in `src/lib/types.ts` (display paths should use `getStatusLabel` / `getPriorityLabel`; raw `.label` may still appear in unmigrated dialogs)

## Backend errors

- Majority of Rust `Err(String)` Chinese messages pass through via `mapBackendError` (passthrough).
- Add stable phrase → `errors:*` keys only when wording is frozen across releases.

## Follow-up trigger

After U1 (`send_input`) lands, re-scan `rg '[\u4e00-\u9fff]' src` and extract new + leftover UI strings.
