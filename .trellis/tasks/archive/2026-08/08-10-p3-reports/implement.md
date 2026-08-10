# Reports R1 — Implement

## Checklist

1. [ ] Inspect `get_dashboard_report_summary` + `DashboardReportSummary` + milestone schema
2. [ ] Extend Rust aggregation: configurable trend range + milestone remaining/burndown series
3. [ ] Add Rust unit tests for range bucket edges and empty milestone
4. [ ] Update `backend.ts` types + `DashboardPage` controls (range + milestone) + charts/空态
5. [ ] Keep error/retry banner behavior
6. [ ] `format:check` + clippy + `npm run test:ci` + targeted cargo tests

## Validation

```bash
npm run format:check
npm run test:ci
cargo test --manifest-path src-tauri/Cargo.toml dashboard_report
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## Rollback

Revert this child commit only; does not touch send_input/i18n.

## Depends on

None (O1 first).
