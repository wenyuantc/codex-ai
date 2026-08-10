# Reports R1 — Design

## Boundaries

- Extend dashboard report only (`DashboardPage` + Rust report command/models)
- No new route; no Issues sync
- Soft-delete aware; respect project + SSH scope like existing `get_dashboard_report_summary`

## Data Flow

```
DashboardPage (range + milestone select)
  → getDashboardReportSummary / extended payload
  → Rust: aggregate tasks + milestones
  → JSON → charts (daily/weekly series + burndown/remaining)
```

## Contracts

### Input (additive)

- `range_days` or preset (`7d` / `30d` / `8w`) for trend buckets
- optional `milestone_id` for burndown series

### Output (additive)

- existing fields preserved
- `trend_series`: labeled points for selected range
- `milestone_burndown`: `{ label, remaining }` points or null + empty reason when no milestone / no due window

## Tradeoffs

- Prefer extending one command over a second IPC to keep UI fetch simple
- Burndown needs milestone `due_date` / task links; if schema lacks ideal Ideal burndown, ship **remaining open tasks over time** from activity/`updated_at`/`completed_at` with clear UI label

## Compatibility

- Older UI ignoring new fields still works
- SSH scope unchanged

## Rollback

- Revert command fields + dashboard UI; CSV/JSON export untouched
