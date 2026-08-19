# Design · B1 前端对接（后端已落地）

## Boundary

本轮只改前端与设置类型。不改 `run_queue.rs` 语义，除非发现返回值/事件无法被 UI 消费。

闸门仍是本机进程计数：本地与 SSH 项目的执行会话走同一 `start_*`，计入同一上限。

## Contracts

**Start 返回值**（Rust 已是 tagged `status`）：

```ts
type StartSessionOutcome =
  | { status: "started" }
  | { status: "queued"; position: number };
```

四引擎 `start_*` 包装从 `Promise<void>` 改为返回该类型。`restart_*` 不排队，保持原样。

**队列条目**（`list_task_run_queue`）：`id`, `task_id`, `provider`, `employee_id`, `enqueued_at`, `position`。

**事件**：`task-run-queue-changed`（空 payload）。前端收到后重新 `list_task_run_queue`。

**设置**：`CodexSettings.max_concurrent_sessions: number`。`UpdateCodexSettingsInput` 增加同名字段。闸门只读本地 `load_codex_settings`，因此控件始终走 `updateCodexSettings`，即使 Settings 处于 SSH 视图。

## Data Flow

```text
用户点运行 / 批量运行
  → startTaskRunSession
  → startCodex|Claude|Grok|OpenCode
  → Rust gate_or_enqueue
      started → 现有 busy / in_progress / timer
      queued  → 只刷新队列缓存；不改员工、任务状态、计时
  → emit task-run-queue-changed
  → taskStore.fetchRunQueue

会话退出 / 应用启动
  → Rust spawn_drain
  → 队首真正 start + timer
  → emit task-run-queue-changed
  → 看板徽标消失，既有 session listener 刷新任务
```

取消：`cancel_queued_task_run(task_id)` → 活动日志 `task_run_queue_cancelled` → 事件刷新。

## Frontend Shape

- **类型**：`src/lib/types.ts` 增加 `StartSessionOutcome`、`TaskRunQueueItem`；`CodexSettings` 补字段。
- **IPC**：`backend.ts` 增加 `listTaskRunQueue` / `cancelQueuedTaskRun`。引擎 start 包装放在现有 `codex.ts` / `claude.ts` / `grok.ts` / `opencode.ts`。
- **Store**：`taskStore` 增加 `runQueue: TaskRunQueueItem[]`、`fetchRunQueue`、`cancelQueuedRun`、`initRunQueueListener`。Listener 与现有 session listener 一样在 `MainLayout` 初始化并返回 unsubscribe。
- **启动内核**：`startTaskRunSession` 先 invoke，再按 outcome 做副作用。返回 outcome 给批量条汇总。
- **其它执行入口**：`TaskDetailDialog` / `CodexControls` 若仍直接 `start_*` 且带 `taskId`，改为复用 `startTaskRunSession` 或同等「queued 跳过副作用」。`SessionsPage` / `TaskSessionChainPanel` 的 resume/即席路径不排队，只需能解析新返回值。
- **CTA**：`resolveTaskPrimaryCta` 增加 `queued`（disabled，文案「排队中」），优先级低于 stop / starting / running_locked，高于普通 run。
- **卡片**：徽标「排队中(第 N 位)」+ 右键「取消排队」。`enqueued_at` 若展示则走 `formatDate()`。
- **设置**：`RuntimeSettingsTab` 数字输入（整数，最小 0）。放在本地运行偏好（主题/语言附近），文案说明 0=不限。
- **批量**：`KanbanPage` 多选条加按钮。对选中任务用现有可运行判断（有负责人、非归档、依赖完成、非已在跑/已在队）；顺序 `await startTaskRunSession`；toast/行内文案汇总 started/queued/skipped。
- **i18n**：`settings` / `kanban` / `tasks` / `activity` 的 zh-CN + en。活动 key：`task_run_queued`、`task_run_dequeued`、`task_run_queue_cancelled`、`task_run_dequeue_failed`。

## Compatibility / Rollback

- SSH：无单独远端闸门；设置控件写本地即可。
- 旧前端忽略 start 返回值仍能启动，但会误标 busy——本轮必须改内核。
- 回滚：还原前端提交即可，队列表与后端命令可留。

## Tests

- 纯函数：`resolveTaskPrimaryCta` 增加 queued 用例（`taskPrimaryCta.test.ts`）。
- 可选：`startTaskRunSession` 若可抽 outcome 分支为纯函数则单测；否则不新增组件测试。
- 回归：`cargo test run_queue`、`npm run test:ci`、`npm run build`。
