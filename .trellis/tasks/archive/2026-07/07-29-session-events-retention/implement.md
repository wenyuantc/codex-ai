# 执行计划 — 会话事件保留与清理策略

前置：prd/design 评审通过，`task.py start` → `in_progress`。  
分支：`feat/session-events-retention`

基线：记录 `cargo test` 通过数（预期 ≥ 275）。

## 步骤

### S1 分支与基线

```bash
git checkout -b feat/session-events-retention
cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8
```

### S2 Migration 41 `[R1]`

`src-tauri/src/db/migrations.rs` 追加：

- version 41
- `CREATE INDEX IF NOT EXISTS idx_codex_session_events_created_at ON codex_session_events(created_at);`

如有 migration 单测惯例，补一条索引存在断言。

### S3 策略读写 + 纯函数 `[R2]`

新建 `src-tauri/src/app/session_events_policy.rs`（或等价）：

- `DEFAULT_RETENTION_DAYS = 30`
- `normalize_retention_days(i32) -> i32`
- load/save `$APPCONFIG/session-events-policy.json`
- 在 `app/mod.rs` 声明模块

### S4 清理核心 + commands `[R3]`

在 `database.rs` 或独立 `session_events_retention.rs`：

- `count/stats` 查询
- `purge_expired_session_events(pool, days) -> deleted`
- `vacuum_database(pool) -> Result<(), String>`
- commands：`get_session_events_policy` / `update_session_events_policy` / `get_session_events_stats` / `purge_session_events`
- purge 写 `insert_activity_log(..., "session_events_purged", ...)`
- `run_startup_session_events_purge`：只 DELETE，不 VACUUM

`lib.rs` 注册 4 个 command；setup 里 spawn 启动清理。

### S5 前端 `[R4]`

- `src/lib/backend.ts`（或现有 API 封装处）增加 invoke 包装
- `DatabaseSettingsTab`：保留天数 + 保存 + 统计 + 立即清理
- 仪表盘 activity key `session_events_purged` → 中文「清理会话事件」

### S6 测试 `[R5]`

- normalize 边界：0 → 30，4000 → 30 或 clamp 到 3650（与 design 一致：非法回默认；范围内 clamp）
- 插入 `created_at` 为 40 天前 / 1 天前，days=30 → 只删前者
- `cargo test` + `npm run build`

### S7 手工

设置页改天数 → 立即清理 → 看 deleted；重启后策略仍在；activity 中文。

### S8 收尾

更新 `.trellis/spec/backend/database-migrations.md` 或 sessions 相关说明（若有）；提交合 main。

## 回滚

| 点 | 回退 |
|---|---|
| R1 | 去掉 migration 41 |
| R2–R3 | 删模块与 command 注册 |
| R4 | 还原前端 tab |
| R5 | 删测试 |

## 验证

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
npm run tauri:dev   # S7
```
