# 技术设计 — 会话事件保留与清理策略

## 边界

| 层 | 改动 |
|---|---|
| DB | 可选 migration：全局 `created_at` 索引加速按时间删除 |
| Backend | 清理/统计/策略读写 command；启动钩子 |
| Frontend | `DatabaseSettingsTab` + Settings 接线；activity 中文 label |
| 不改 | 四引擎事件写入、`insert_codex_session_event*` 签名 |

## 存储策略配置

**不**把保留天数塞进 `CodexSettings` 的 SDK/任务自动化语义里。

新建轻量文件配置（对齐 `window_state` / 小 JSON 惯例）：

- 路径：`$APPCONFIG/session-events-policy.json`
- 形状：

```json
{
  "retention_days": 30
}
```

- 默认：文件不存在 → `30`
- 规范化：`clamp(1..=3650)`，非法 → `30`

模块建议：`src-tauri/src/app/session_events_policy.rs`（或 `database.rs` 内私有模块，优先独立文件保持 database.rs 不继续膨胀）。

## 数据面

### 删除语义

```sql
DELETE FROM codex_session_events
WHERE created_at < datetime('now', printf('-%d days', :days));
```

SQLite 绑定：用 `format!("-{days} days")` 作为 `datetime('now', $1)` 修饰符（与现有 SQLite 时间字符串一致）。

返回：`rows_affected`。

### 索引（migration v41）

现有 `idx_codex_events_session_created (session_id, created_at)` 不利全局时间扫。

```sql
CREATE INDEX IF NOT EXISTS idx_codex_session_events_created_at
ON codex_session_events(created_at);
```

`get_all_migrations()` 追加 version **41**（当前最新 40）。

### VACUUM

- `DELETE` 提交后：`sqlx::query("VACUUM").execute(&pool)`  
- 注意：部分连接上 VACUUM 与打开事务冲突；使用独立连接或确保无未结束事务。
- 失败 → 结果结构带 `vacuum_error: Option<String>`，删除计数仍有效。

## IPC 合约

```rust
// 读策略
get_session_events_policy() -> SessionEventsPolicy { retention_days: i32 }

// 写策略
update_session_events_policy(retention_days: i32) -> SessionEventsPolicy

// 统计（设置页展示）
get_session_events_stats() -> SessionEventsStats {
  total_events: i64,
  expired_events: i64,  // 相对当前策略
  oldest_created_at: Option<String>,
  newest_created_at: Option<String>,
}

// 立即清理（按已保存策略；可选覆盖 days 仅当 UI 已保存）
purge_session_events() -> PurgeSessionEventsResult {
  deleted: u64,
  retention_days: i32,
  vacuum_ok: bool,
  vacuum_error: Option<String>,
}
```

全部 `Result<T, String>`，中文错误。注册于 `lib.rs` `invoke_handler`。

**Activity log**：`purge` 成功时 `insert_activity_log`，action 建议：

- key: `session_events_purged`
- details: 含 deleted / retention_days

前端仪表盘 activity 映射表增加中文：**「清理会话事件」**。

## 启动自动清理

在 `lib.rs` setup 中与 `log_database_startup_status` 相邻：

```rust
tokio::spawn(async move {
  let _ = app::database::run_startup_session_events_purge(&app_handle).await;
});
```

- 读策略 → DELETE 过期 → 可选 VACUUM（启动时 **默认跳过 VACUUM** 以免拖慢启动；仅手动「立即清理」做 VACUUM）
- 失败 `eprintln!`，不阻断启动

## UI

`DatabaseSettingsTab` 新增卡片「会话事件保留」：

- 显示 `total_events` / `expired_events`（打开 tab 时拉取 stats）
- 数字输入：保留天数 + 保存按钮（或 blur 保存，跟现有设置风格一致）
- 按钮「立即清理」：确认对话框（将删除约 N 条）→ invoke purge → toast/行内消息
- 非 Tauri：disable + 提示

`SettingsPage` 传入 handlers 或 tab 内直接 `invoke`（优先 tab 内自包含 + `lib/backend.ts` 封装，少改 SettingsPage 巨型状态）。

## 测试

| 用例 | 方式 |
|---|---|
| normalize days | 纯函数 |
| DELETE 过期保留窗口内 | `setup_test_pool` + insert events 改 `created_at` |
| 策略读写 JSON | temp dir 或 mock path helper |
| migration 41 索引存在 | 既有 migration 测试风格 |

不在单测中强制真实 VACUUM 文件缩小断言（平台相关）；可 assert 调用不 panic / 结果字段。

## 回滚

- 还原 migration 41 仅影响新装；已迁移 DB 保留索引无害
- 删 command + UI + 策略文件即可回到只增不删

## 风险

| 风险 | 缓解 |
|---|---|
| 误删仍需的审计日志 | 默认 30 天；范围上限 3650；UI 确认 |
| VACUUM 锁库 | 手动清理时执行；启动跳过 |
| 大表 DELETE 卡顿 | 索引 + 可后续分批（本任务单次 DELETE 可接受桌面规模） |
