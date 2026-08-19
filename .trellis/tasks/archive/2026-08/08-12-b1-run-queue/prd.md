# PRD · B1 并发闸门与运行队列

父任务:`08-12-product-gap-wave` · 优先级 N-P0

## Goal

用户能限制本机同时运行的 AI 会话数；超限的任务执行进入持久化队列，看板可见位次并可取消；看板支持批量入队运行。这样批量启动不会把机器打满。

## Background

功能闭环已通，主矛盾之一是「管不住并发」。后端已于 `8dbd82a` 落地：`task_run_queue` 表、四引擎 `start_*` 闸门、`StartSessionOutcome`、FIFO drain、重启恢复、活动日志。本任务剩余交付是把设置、排队状态和批量运行接到 UI。

## Confirmed Facts

- 闸门只对「任务执行会话」排队：有 `task_id`、`session_kind=execution`（或缺省）、非 resume。review / fix / pipeline / 即席会话计入并发但不排队。
- 默认上限 3；`0` 或不限制。配置在本地 `CodexSettings.max_concurrent_sessions`，闸门读本地设置，不读 SSH 远端配置。
- 命令已注册：`list_task_run_queue`、`cancel_queued_task_run`；事件 `task-run-queue-changed`。
- 入队时后端不改任务状态、不占员工 busy、不计时；drain 真正启动时才写 `in_progress` / busy / timer。
- 前端 `startCodex` 等包装仍把返回值当 `void`；`startTaskRunSession` 在 invoke 前就把员工标 busy、任务标进行中并启动计时——排队路径必须改掉这些副作用。
- `activity:actions` 还没有 `task_run_queued` / `task_run_dequeued` / `task_run_queue_cancelled` / `task_run_dequeue_failed`。

## Requirements

1. **并发上限可配**：设置页 Runtime 展示并保存本地 `max_concurrent_sessions`（默认 3，0=不限）。SSH 设置视图下该控件仍写本地设置。
2. **启动适配**：四引擎 `start_*` 包装返回 `{status:"started"} | {status:"queued", position}`。任务执行内核 `startTaskRunSession` 在 queued 时跳过员工 busy、任务状态、计时；started 保持现副作用。其它带 `taskId` 的执行启动路径必须走同一语义。
3. **看板队列可见**：taskStore 监听 `task-run-queue-changed` 并拉取 `list_task_run_queue`。卡片显示「排队中(第 N 位)」；右键可取消。排队中主 CTA 锁定，不新增 kanban 列。
4. **批量运行**：看板已有多选条增加「批量运行」。对选中且可运行的任务顺序调用既有启动内核；闸门在 Rust，前 N 个启动、其余排队。不可运行的跳过并汇总结果。
5. **活动日志文案**：上述 4 个 action key 在 zh-CN / en 的 `activity:actions` 可读。

## Acceptance Criteria

- [ ] 设置页可改并发上限并持久化；重启后仍生效；默认 3。
- [ ] 超限启动进入队列且不失败；FIFO；进程退出后队首自动启动。
- [ ] 重启应用后队列不丢；看板重新打开仍显示排队位次。
- [ ] 卡片显示排队位次，可取消；排队中不能再点「运行」重复入队。
- [ ] 批量运行选中任务可用，并反馈启动/排队/跳过数量。
- [ ] i18n zh-CN + en 齐全；仪表盘活动流显示中文动作名。
- [ ] Rust 单测覆盖闸门计数、入队/出队/取消、重启恢复（后端已有，回归不得回退）。

## Out of Scope

- 按项目 / 按员工 / 按引擎拆分并发池。
- 给 review / fix / pipeline 也做排队。
- 新增独立「运行队列」页面或 kanban 列。
- 改闸门算法或队列表结构（除非前端对接发现契约缺口）。
