# 修复看板状态与停止按钮 UI 不同步

## Goal

看板（及同一套 TaskCard 组件）在任务状态、会话运行态变化后，**无需切页或整页刷新**即可展示正确列位置与运行/停止按钮态，消除“库已更新、UI 仍是旧状态”的错觉。

## Background / Confirmed Facts

用户报告两个可复现现象：

1. **状态列不刷新**：任务从「进行中」进入「审核中」后，看板仍停在「进行中」；离开再进看板后正确。
2. **停止后按钮仍 loading**：进行中任务停止已成功，按钮仍呈加载/运行中；整页刷新后恢复。

### 仓库证据（当前行为）

| 现象 | 证据 | 结论 |
|------|------|------|
| 后端改任务状态不主动推列表 | `task_automation/state.rs` `update_task_status_internal` 只写库 + 活动日志 + 通知，**无任务列表事件** | 前端只能靠旁路事件重拉 |
| 执行完成→审核路径不发 automation 事件 | `session_exit.rs` `handle_execution_exit` → `retry_pending_review` 改 status=`review`、phase=`waiting_review` 后 **无** `emit_task_automation_state_changed`；`finalize_launched_action` 只写库 | `onTaskAutomationStateChanged` 不会触发 `fetchTasks` |
| 退出事件与写库竞态 | 进程先 `*-exit` 再 `handle_session_exit` 改 status；`taskStore` 在 `onCodexExit` 立即 `fetchTasks` | 常拉到 **改库前** 的 `in_progress`，之后再无刷新 |
| 非 Codex 引擎列表更不刷新 | `taskStore.initCodexSessionListeners` 只订 `onCodexExit` / `onCodexSession`，不订 Claude/Grok/OpenCode exit | Claude 等引擎退出后看板完全不重拉任务 |
| 人工停止不推 automation 相位 | `handle_execution_exit` 在 `has_stopping_requested` 时写 `manual_control` 后 **直接 return，无 emit** | 前端 `automationStates` 仍可能是 `waiting_execution` 等 |
| 停止后仍显示 loading 的 UI 路径 | `TaskCard`：`isRunning && !executionActions.isRunning` → 绿色 `Loader2`「运行中」；`isRunning` 含 `isTaskAutomationExecutionActive`（`launching_fix` / `waiting_execution` / `committing_code`） | 进程已停但 automation 相位陈旧 → **永久假 loading** |
| 页面刷新“修好” | `KanbanPage` `useEffect` 依赖项目切换才 `fetchTasks`；刷新会重拉任务 + runtime + automation | 与“事件缺失/竞态”一致，非后端未写库 |

### 相关代码锚点

- Frontend：`src/stores/taskStore.ts`（listeners / `fetchTasks` / `setTaskLastSessionId`）
- Frontend：`src/components/tasks/TaskCard.tsx`（运行/停止 UI 分支）
- Frontend：`src/components/tasks/hooks/useTaskExecutionActions.ts`（stop + `loading` + runtime）
- Frontend：`src/lib/utils.ts`（`getTaskActionRuntimeState` / automation active phases）
- Backend：`src-tauri/src/task_automation/session_exit.rs`、`state.rs`、`fix_loop.rs`

## Requirements

### R1 — 任务状态变化后看板即时对齐

- 当后端将任务状态改为 `review` / `blocked` / `completed` / 其他合法状态时，**已打开的看板**中该任务应进入正确列，无需切换路由或 F5。
- 覆盖：自动质控闭环（执行完成→审核）、以及同一会话链路引起的状态变更。
- 覆盖引擎：至少 Codex + Claude；Grok / OpenCode 与 Claude 同属“非 Codex 退出事件”路径，须一并接入或共用统一刷新入口。

### R2 — 会话退出后列表与 automation 相位可靠刷新

- 任意引擎会话退出后，前端应刷新受影响任务的：
  - 任务列表字段（含 `status`）
  - `task_automation_state` 展示相位（若任务开启自动质控）
- 刷新时机须覆盖“退出事件早于自动化写库完成”的竞态（不能只在 exit 瞬间拉一次就结束）。

### R3 — 人工停止后按钮态立即正确

- 用户点击停止且后端停止成功后：
  - 不再显示假的「运行中 / loading」
  - 进程 runtime 为未运行时，应回到可运行或与当前 `task.status` / automation 相位一致的按钮（如 `in_progress` 显示「运行」，`review` 显示「审核」）
- 开启自动质控时，人工停止写入 `manual_control` 后，卡片 automation 徽标/运行态须同步，无需刷新页面。

### R4 — 不改变既有业务语义（除非与同步直接相关）

- 人工停止后任务 status 默认保持现有后端策略（不强制改 status，除非现有代码已改）。
- 不改动看板拖拽改状态、批量改状态等已能本地更新 store 的路径（可顺带验证不被破坏）。

## Acceptance Criteria

- [ ] **AC1** 任务在执行完成后被自动质控置为 `review` 时，看板列在 **5 秒内**（通常应接近即时）从「进行中」变为「审核中」，无需离开看板。
- [ ] **AC2** 使用 Claude（及 Codex）引擎复现 AC1 均通过。
- [ ] **AC3** 进行中任务点击停止成功后，卡片不再持续显示 `Loader2`「运行中」；runtime 清空后可再次点击「运行」（或按当前 status 显示对应主按钮）。
- [ ] **AC4** 开启 `review_fix_loop_v1` 时人工停止后，automation 展示不再卡在 `waiting_execution` / `launching_fix` 等“执行中”相位（应与后端 `manual_control` 或后续真实相位一致）。
- [ ] **AC5** 手动拖拽改列、批量改状态、打开详情后保存 status 的既有路径仍正常。
- [ ] **AC6** `npm run build` 与相关 `cargo test`（至少 task_automation / 触及模块）通过。

## Out of Scope

- 看板性能优化、虚拟列表
- 全新“任务状态 WebSocket 订阅”架构（本任务优先修 emit 缺口 + 前端多引擎监听 + 竞态）
- 重做自动质控状态机业务规则
- 非看板专用但共用组件的视觉改版

## Key Decisions

| 决策 | 选择 | 理由 |
|------|------|------|
| 修复层级 | **后端关键路径补 emit** + **前端统一会话退出/automation 刷新** | 单靠前端 exit 拉取会继续竞态；单靠 emit 不足以覆盖全引擎 exit |
| 状态列数据源 | 继续以 `taskStore.tasks[].status` + `fetchTasks` / 局部 patch 为准 | 与现架构一致，Kanban 已订阅 store |
| 假 loading 根因处理 | 停止/退出后必须同步 `automationStates` 与 `employeeRuntime` | UI 把 automation 相位当成 running |

## Risks

- 过频 `fetchTasks` 导致看板闪烁或 IPC 压力 → 优先定点更新单任务；全量拉取做去抖或仅在 automation/status 事件后执行。
- 多引擎重复监听导致重复拉取 → 统一 refresh helper + 按 `task_id` 合并。

## Open Questions

（无阻塞问题；产品意图已由缺陷描述与现有行为覆盖。）
