# 会话事件保留与清理策略

父任务：`.trellis/tasks/07-29-codex-ai-optimization`  
覆盖源发现：**#2**（`codex_session_events` 只增不删）

## Goal

为 `codex_session_events` 建立按天数的保留策略：默认 30 天、设置页可调、支持立即清理，并在清理后可选回收磁盘（VACUUM），避免会话日志无限膨胀。

## Background

- 表结构（migration v14）：`id`, `session_id` → `codex_sessions` CASCADE, `event_type`, `message`, `created_at`。
- 全仓库无 `DELETE FROM codex_session_events`，无保留策略，无针对该表的 VACUUM 运维入口。
- 写入路径：四引擎 + `sessions.rs` / `task_automation` 等，持续 `INSERT`。
- 读路径示例：`get_codex_session_log_lines` 已对单会话 `LIMIT 2000`，但库文件体积仍随历史事件增长。

## 前置条件

- 无。不依赖 C1/C1b 或其他子任务；可与 C3 并行。
- 父任务已确认约束：**默认 30 天 + Settings 可调 +「立即清理」**。

## Requirements

### R1 — 默认保留 30 天

未配置时默认 `session_events_retention_days = 30`。  
超过保留期的事件行应可被清理（`created_at` 早于阈值）。

### R2 — 设置项可调

在 `SettingsPage`（建议 **数据库** 分区 / `DatabaseSettingsTab`）暴露：

- 保留天数输入（整数）
- 合法范围：**1–3650**（约 10 年）；非法值回退默认 30 并提示
- 持久化跨重启

不在本任务做「按条数上限」保留（父约束仅天数；条数策略若需要另开任务）。

### R3 — 立即清理

提供「立即清理」操作：

- 按**当前保存的**保留天数删除过期事件
- 返回删除行数（及可选：清理前后事件总数）给 UI
- 清理成功写 **activity_log**，仪表盘中文 key

### R4 — 磁盘回收

清理后执行 SQLite `VACUUM`（或文档化的等效回收），使删除尽量反映到数据库文件体积。  
若 VACUUM 失败：事件删除结果仍算成功，向用户返回警告（中文），不静默吞掉。

### R5 — 自动清理（启动时）

应用启动时 best-effort 按当前策略清理一次过期事件（不阻塞 UI；失败仅日志/`eprintln!`）。  
避免用户从未打开设置页时库无限增长。

### R6 — 安全与兼容

- 仅删除 `codex_session_events` 过期行；**不**删除 `codex_sessions` 会话头、附件、file_changes
- `ON DELETE CASCADE` 语义不变：删会话仍级联删其事件
- 进行中会话：若事件 `created_at` 仍在窗口内则保留；过期 stdout 历史可删（可接受）
- 无前端直写 SQL；清理/读配置/改配置均走 Tauri command
- SSH 兼容：策略与清理作用于**本地 SQLite**（与现有「SSH 只切换执行上下文、不切换库位置」一致）

## Acceptance Criteria

- [x] 默认保留天数为 30；设置保存后重启仍生效
- [x] 设置页可修改天数（1–3650）并看到校验反馈
- [x] 「立即清理」删除 `created_at` 早于阈值的事件，UI 显示删除数量
- [x] 清理写 activity_log，仪表盘有中文标签（`session_events_purged`）
- [x] 启动时至少尝试一次自动清理（代码 + 单测覆盖 purge 语义）
- [x] 清理后尝试 VACUUM；失败时有中文警告且删除已提交
- [x] `cargo test` 全绿且测试数 ≥ 基线（275 → 284）；`npm run build` 通过
- [ ] 独立分支 + 单独提交合回 `main`（提交后合入时勾选）

## Out of Scope

- 不按 event_type 差异化保留（全部事件同一天数）
- 不按条数 / 按会话大小配额
- 不远程清理 SSH 机器上的库（库在本地）
- 不做会话列表虚拟滚动（属 C4）
- 不改动事件写入协议

## Notes

- complex：需 `design.md` + `implement.md`
- 交付：分支建议 `feat/session-events-retention`
