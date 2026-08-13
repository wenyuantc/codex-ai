# Dashboard Report Summary Contract

> Executable contract for `get_dashboard_report_summary` (R1: trend range + milestone remaining).

---

## 1. Scope / Trigger

- Command: `get_dashboard_report_summary`
- Layers: Rust (`app/database.rs` + `db/models.rs`) ↔ frontend (`backend.ts` → `DashboardPage`)
- Additive fields only; legacy `weekly_completed` / `weekly_completed_series` must remain populated for older UI assumptions
- Out of scope: Issues sync, dedicated `/reports` route, ideal-line classic burndown

## 2. Signatures

**IPC**: `invoke("get_dashboard_report_summary", { payload })`

**Payload (`GetDashboardReportPayload`)**:

| Field | Type | Notes |
|-------|------|-------|
| `project_id` | `Option<String>` | Scope filter |
| `environment_mode` | `Option<String>` | `local` / `ssh` |
| `selected_ssh_config_id` | `Option<String>` | SSH host scope |
| `aging_days` | `Option<i64>` | Existing aging window |
| `trend_range` | `Option<String>` | `"7d"` \| `"30d"` \| `"8w"`; default `"7d"` when missing/invalid |
| `milestone_id` | `Option<String>` | Optional; if omitted, auto-pick first scoped milestone when any exist |

**Response (`DashboardReportSummary`)** — additive keys:

| Field | Type | Notes |
|-------|------|-------|
| `trend_range` | `String` | Echo effective range |
| `trend_series` | `Vec<{label,count}>` | Daily for `7d`/`30d`, weekly for `8w` |
| `milestone_burndown` | `Vec<{label,remaining}>` | Open-task **remaining** over time (not ideal line) |
| `milestone_burndown_empty_reason` | `Option<String>` | Chinese reason when series empty |
| `selected_milestone_id` | `Option<String>` | Milestone used for series |
| `milestones` | `Vec<{id,name,...}>` | Scoped picker options |
| `token_usage` | `TokenUsageSummary` | Scoped SUM of known session tokens; `sessions_with_usage=0` means none known |
| `token_usage_series` | `Vec<{label,count}>` | Same buckets as `trend_series`; `count` = total_tokens that day/week |
| `token_usage_by_provider` | `Vec<DashboardTokenProviderUsage>` | Group by `codex_sessions.ai_provider` |

Frontend wrapper: `getDashboardReportSummary({ trendRange, milestoneId, ... })` maps camelCase → snake_case payload.

## 3. Contracts

- Soft-delete: tasks/milestones/projects with `deleted_at IS NOT NULL` excluded
- Scope: same project + environment + SSH host rules as pre-R1 report
- SSH without selected host: milestone list/series must not leak other hosts; empty-state OK
- Unknown `trend_range` → coerce to `"7d"`
- Unknown / out-of-scope `milestone_id` → empty burndown + reason (do not 500)
- UI chart keys must not rely on `label` alone (`MM-DD` can collide across years) — include index
- Token empty state: hide charts / show empty copy when `token_usage.sessions_with_usage === 0`. Do not draw a 0-token “cost” chart. Phase 1 has **no currency conversion**.

## 4. Validation & Error Matrix

| Case | Behavior |
|------|----------|
| Valid range + scope | 200 summary with series |
| Invalid range string | Treat as `7d` |
| No milestones in scope | `milestone_burndown=[]`, non-null Chinese `milestone_burndown_empty_reason` |
| Milestone deleted / wrong scope | Empty burndown + reason; do not panic |
| DB / query failure | `Result::Err(String)` → UI error + retry |

## 5. Good / Base / Bad Cases

- **Good**: `trend_range=30d` + valid `milestone_id` → `trend_series` length 30; burndown labels unique by index in UI
- **Base**: omit `trend_range` / `milestone_id` → `7d` + auto first milestone or empty reason
- **Bad**: claim Issues sync or ideal burndown in UI copy; drop legacy `weekly_*` fields

## 6. Tests Required

| Assertion | Location |
|-----------|----------|
| Range bucket edges (`7d`/`30d`/`8w`) | `app/tests/dashboard_report.rs` + unit helpers in `database.rs` |
| Empty milestone / soft-delete remaining | `dashboard_report.rs` |
| Local vs SSH milestone scope isolation | `dashboard_report.rs` |
| Frontend range label helpers | `src/lib/dashboardReport.test.ts` |
| Token SUM / by-provider / empty `sessions_with_usage` | `app/tests/dashboard_report.rs` |
| Compact token formatting | `src/lib/dashboardReport.test.ts` (`formatTokenCount`) |

## 7. Wrong vs Correct

| Wrong | Correct |
|-------|---------|
| Flip matrix / invent Issues sync for “stronger reports” | Only dashboard additive analytics |
| Replace `weekly_*` with `trend_*` only | Keep both; UI prefers `trend_series` |
| React `key={point.label}` | `key={`${point.label}-${index}`}` |
| Count completed-as-burndown without labeling | Label as **剩余未完成** / remaining open tasks |

## Scenario: Dashboard + task token usage (A1)

### 1. Scope / Trigger
- Additive analytics on `get_dashboard_report_summary` plus `get_task_token_usage`. Same project/environment/SSH scope as the rest of the report. Aggregate by `codex_sessions.started_at`.

### 2. Signatures
- IPC: `invoke("get_task_token_usage", { taskId })` → `TokenUsageSummary`
- IPC: `get_dashboard_report_summary` additive fields above
- `TokenUsageSummary { input_tokens, output_tokens, total_tokens, reasoning_tokens, sessions_with_usage, session_count }`
- `DashboardTokenProviderUsage { provider, input_tokens, output_tokens, total_tokens, sessions_with_usage }`

### 3. Contracts
- SUM uses `COALESCE(SUM(col), 0)` **only among rows that have usage**. `sessions_with_usage` counts rows where any token column is NOT NULL.
- Sessions with all-NULL tokens do not increment `sessions_with_usage` and must not make the UI show “0 tokens used”.
- Task UI (`TaskExecutionPanel`) renders the block only when `sessions_with_usage > 0`.
- Types for the dashboard DTO live next to `DashboardReportSummary` in `src/lib/backend.ts` (same as other report-only shapes). Session row fields stay on `CodexSessionRecord` in `types.ts`.

### 4. Validation & Error Matrix
| Case | Behavior |
|------|----------|
| Task with no sessions / all NULL | `{ sessions_with_usage: 0, *_tokens: 0 }` — UI hides |
| Mixed known + unknown sessions | SUM known only; `session_count` includes all |
| Missing `task_id` | command error string |
| Dashboard empty scope | `token_usage.sessions_with_usage=0`; series empty or zeros unused by UI |

### 5. Good / Base / Bad Cases
- Good: 7d range + two Codex sessions with totals → series buckets match `trend_series` length; provider row `codex`
- Base: omit filters → same scope as the rest of the dashboard; ad-hoc sessions (`project_id` NULL) included when no project filter
- Bad: convert tokens to USD; show “0” on the task execution tab when usage was never parsed

### 6. Tests Required
- Dashboard by-provider + unknown sessions excluded from `sessions_with_usage`
- Task SUM ignores sessions with all-NULL tokens
- Frontend `formatTokenCount`

### 7. Wrong vs Correct
#### Wrong
```tsx
<span>{tokenUsage.total_tokens}</span> // always shown, including 0 from empty SUM
```
#### Correct
```tsx
{tokenUsage.sessions_with_usage > 0 ? <span>{formatTokenCount(tokenUsage.total_tokens)}</span> : null}
```

## Primary References

- `src-tauri/src/app/database.rs` (`get_dashboard_report_summary`)
- `src-tauri/src/db/models.rs` (`GetDashboardReportPayload`, `DashboardReportSummary`)
- `src/lib/backend.ts`, `src/lib/dashboardReport.ts`, `src/pages/DashboardPage.tsx`
- Tests: `src-tauri/src/app/tests/dashboard_report.rs`
