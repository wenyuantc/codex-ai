# Implement: session_origin

## Order

1. Migration 51 + `CodexSessionRecord.session_origin` + insert 默认 `direct`
2. `mark_session_origin_pipeline`；pipeline 绑定 session_id 后调用
3. List / search / EmployeeRunningSession 传出 origin
4. TS types + `sessionDisplayKind` / `formatSessionKind` / badge helper
5. SessionsPage + SessionCard：Badge、类型列、筛选
6. TaskSessionChainPanel + EmployeeRunningSessionsDialog + locales
7. Tests + build / format / clippy

## Validation

- `cargo test --manifest-path src-tauri/Cargo.toml session_origin`
- `npm run test:ci -- src/lib/sessions.test.ts`
- `npm run build`、`npm run format:check`、`clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Rollback

删 v51 前的代码改动；已升级的本地库保留列无害（默认 `direct`）。不要改已发布的 migration 1–50。
