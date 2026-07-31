# Design: 看板状态与停止按钮 UI 同步

## Architecture / Boundaries

```text
[引擎进程退出 / 人工停止]
        │
        ▼
 Rust session lifecycle ──emit──► *-exit / *-session
        │
        ▼
 task_automation::handle_session_exit
   · 写 tasks.status
   · 写 task_automation_state.phase
   · emit task-automation-state-changed   ◄── 补齐缺口
        │
        ▼
 Frontend taskStore + employeeStore
   · 全引擎 exit → refresh runtime
   · automation / 可靠时机 → fetchTasks 或 patch task
        │
        ▼
 KanbanBoard / TaskCard 订阅 store 重渲染
```

**边界**

- 业务写库仍只在 Rust。
- 前端不新增直写 SQLite。
- 看板不单独维护任务副本；继续用 `useTaskStore().tasks`。

## Data Flow & Contracts

### 1. Backend：automation 状态变化必须可观察

在下列路径 **写库成功后** 调用已有 `emit_task_automation_state_changed(app, task, phase)`（payload 已有 `task_id` / `project_id` / `phase`）：

| 路径 | 当前 | 目标 |
|------|------|------|
| 执行成功 → 启动审核（`retry_pending_review` / `finalize_launched_action` 成功后） | 不 emit | emit `waiting_review`（或实际 phase） |
| 执行/审核 **人工停止** → `manual_control` | 写库无 emit | emit `manual_control` |
| 启动修复成功、`waiting_execution` 等对称路径 | 检查后补齐 | 与 review 对称 |
| 已有 terminal 路径（completed / blocked / commit_failed 等） | 已有 emit | 保持 |

可选增强（推荐，降低竞态）：在 `update_task_status_internal` 成功后额外 emit 同一事件或专用 `task-status-changed`。为减少事件种类，**MVP 优先补 automation 事件 + 前端 exit 后再拉**；若仍竞态，再在 `update_task_status_internal` 末尾 emit（phase 可传当前 automation phase 或 `"status_changed"`）。

### 2. Frontend：统一“会话退出后刷新任务”

`taskStore.initCodexSessionListeners` 扩展为“任务会话监听”（命名可暂不改，行为要全引擎）：

- 订阅：`onCodexExit` + `onClaudeExit` + `onGrokExit` + `onOpenCodeExit`
- 处理：若有 `task_id`：
  1. `fetchTaskAutomationState(task_id)`
  2. `fetchTasks(activeProjectId)` **或** 更优：拉取单任务后 `map` 替换（若已有 list/get 命令则复用；无则先全量）
- 为覆盖 **exit 早于 automation 写库**：
  - 方案 A：依赖后端补 emit，前端 `onTaskAutomationStateChanged` 再拉一次（推荐主路径）
  - 方案 B：exit 后短延迟二次刷新（fallback，避免仅靠竞态）
  - MVP：**A 必做**；若单测/手工仍见竞态再加 B（debounce 300–500ms 一次）

`on*Session`：若需乐观更新 status，应覆盖全引擎或放弃乐观、只信 DB 刷新，避免 Codex-only 乐观与 Claude 不一致。推荐：**减少错误乐观**——`setTaskLastSessionId` 不要用 session_kind 强行覆盖 `status`，只更新 session id 字段；status 交给 fetch / 后端权威。

### 3. 停止按钮态

`useTaskExecutionActions.stopTask` 现有 `finally { setLoading(null) }` 保留。

停止成功后额外保证：

1. `refreshEmployeeRuntimeStatus`（已有）
2. `fetchTaskAutomationState(task.id)`（新增，不依赖 exit 事件时序）
3. 可选：`fetchTasks` 或单任务 patch（计时字段 `time_started_at` 等）

`TaskCard` 展示逻辑可保持；根因消除后 `executionActive` 假阳性应消失。若仍要防御：当 `!isExecutionRunning && automation 相位为执行中` 超过合理窗口时降级，**MVP 不强制改 UI 分支**。

## Compatibility

- 本地 + SSH 任务：状态写库路径相同，事件在本机 app 内 emit，不依赖 SSH 反向通道。
- 四引擎：runtime 已聚合；任务刷新须四引擎 exit 对齐。
- 无 DB migration。

## Trade-offs

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| 仅前端轮询看板 | 实现快 | 延迟/浪费/仍不同步 | 否 |
| 仅补后端 emit | 权威 | exit 竞态、非 automation 状态变更 | 不够 |
| emit + 全引擎 listener + stop 后拉 automation | 对症 | 多几次 IPC | **采用** |
| 乐观 set status by session_kind | 即时 | 易覆盖正确 DB 状态 | **收敛/移除 status 乐观** |

## Rollback

- 回退 backend emit 补丁与 frontend listener 变更即可；无数据迁移。
