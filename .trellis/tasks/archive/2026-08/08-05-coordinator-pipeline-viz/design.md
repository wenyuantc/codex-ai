# Design — 协调员编排可视化

## 1. Summary

本任务是 **前端可观测性补齐**，不重做编排引擎。

在已有 `task_pipeline_steps` + `task_automation_state.pipeline_*` + `CoordinatorPlanDialog` 操作面之上：

1. **任务详情概览** 增加「有 steps 才渲染」的阶段进度组件（摘要 + 时间线）
2. **看板卡片** 增加基于 automation 游标的轻提示
3. **计划弹窗** 将现有步骤列表升级为更清晰的阶段时间线，并在 pipeline 相关事件时自动刷新
4. **Backend** 默认零改动；仅当事件/字段不足以支撑 AC 时做最小补齐

## 2. Boundaries

| 在内 | 在外 |
|------|------|
| 详情阶段条/时间线、看板 N/M 或失败徽标、弹窗时间线增强、事件驱动 steps 刷新、状态中文 helper 收敛、可选轻量 activity 补齐 | 并行编排、跳过步、全局画布、新生命周期 phase、测试员闭环 |

### 模块归属

| 层 | 路径 | 职责 |
|----|------|------|
| 展示组件 | `src/components/tasks/detail/TaskPipelineProgress.tsx`（新建） | 阶段摘要 + 时间线；纯展示 + 回调 |
| 状态文案 | `src/lib/utils.ts` 或同文件导出的 pipeline helpers | `pipelineStatusLabel` / 整体进度文案；与 Dialog 共用 |
| 详情 | `TaskOverviewPanel.tsx` + `TaskDetailDialog.tsx` | 注入 steps / automation；打开计划弹窗入口 |
| 看板 | `TaskCard.tsx`（轻提示） | 读 `automationStates` 显示编排中/失败 |
| 弹窗 | `CoordinatorPlanDialog.tsx` | 复用 progress 组件或对齐视觉；保留操作按钮 |
| 数据 | 既有 `backend.ts` + `listTaskPipelineSteps` + `getTaskAutomationState` | 无新 command（默认） |
| 事件 | `onTaskAutomationStateChanged`（`src/lib/codex.ts`） | 详情/弹窗订阅后重拉 steps |

## 3. Data & Contracts（现成）

### 3.1 Steps

```ts
TaskPipelineStep {
  id, task_id, step_index, title, goal, success_criteria,
  employee_id, status, session_id, handoff_summary, last_error,
  started_at, ended_at, created_at, updated_at
}
```

Status：`pending | launching | running | succeeded | failed | cancelled | skipped`

### 3.2 Automation cursor

```ts
TaskAutomationState {
  pipeline_active: boolean
  pipeline_step_index: number | null
  phase: ... | pipeline_launching_step | pipeline_waiting_step | pipeline_step_failed
  last_error: string | null
}
```

### 3.3 Event

```ts
TaskAutomationStateChanged { task_id, project_id, phase }
```

**注意**：事件 **不含** steps 快照 → 前端收到后对 **当前打开的 task** 调用 `listTaskPipelineSteps` + 刷新 automation state。

### 3.4 Commands（只消费）

| Command | 用途 |
|---------|------|
| `list_task_pipeline_steps` | 详情/弹窗数据源 |
| `start_task_pipeline` | 按计划编排 |
| `retry_task_pipeline_step` | 失败重试 |
| `abort_task_pipeline` | 转人工 |
| `get_task_automation_state` | 游标/phase |

## 4. UI 设计

### 4.1 `TaskPipelineProgress`（共享）

**Props（示意）**

- `steps: TaskPipelineStep[]`
- `pipelineActive?: boolean`
- `pipelineStepIndex?: number | null`
- `employees?: Employee[]`（显示执行人名）
- `compact?: boolean`（弹窗内可 denser）
- `onOpenStepSession?: (step) => void`
- `onRetry?` / `onAbort?` / `onRefresh?`（详情可只放「打开编排」；完整操作保留弹窗）
- `loading` / `error`

**可见性规则**

```
if steps.length === 0 → return null
```

**布局**

1. **Header**：`编排进度 2/5 · 执行中` + 可选刷新
2. **Stage strip**（横向）：圆点/段，succeeded 实心、current 高亮、pending 灰、failed 红
3. **Timeline list**（纵向）：标题、状态徽章、错误一行、handoff 一行（可选）、`执行日志` 按钮

时间：`started_at` / `ended_at` 用 `formatDate()`。

### 4.2 任务详情

- `TaskDetailDialog` 在打开时 / taskId 变化时 `listTaskPipelineSteps`
- 订阅 `onTaskAutomationStateChanged`：若 `event.task_id === task.id` 则节流刷新 steps + automation store 条目
- `TaskOverviewPanel` 在「协调员计划」区块附近插入 `<TaskPipelineProgress />`（或独立 section）
- 快捷：「打开编排面板」→ 既有 `openCoordinatorPlanFlow`

### 4.3 看板轻提示

数据优先：

```
automation = automationStates[task.id]
if automation?.pipeline_active:
  show "编排中 · 第 {pipeline_step_index+1} 步"  (总数可选：若本地无 steps 缓存则不显示 /M)
if automation?.phase === 'pipeline_step_failed':
  show "编排失败"
```

避免为每张卡片 `listTaskPipelineSteps`（N+1）。若后续需要精确 N/M，再考虑：

- 轻量 command 聚合（`succeeded_count/total`），或
- store 级 steps 缓存仅对 `pipeline_active` 任务批量拉取

**MVP 推荐**：卡片用「编排中 · 第 k 步」+ 失败态，**不强制 /M**；详情用完整 N/M。

### 4.4 计划弹窗

- 用 `TaskPipelineProgress` 替换/包裹现有 `pipelineSteps.map` 块，保留：改执行人 select、重试、转人工、刷新、按计划编排
- 弹窗打开时加载 steps；pipeline phase 事件时自动 refresh
- `pipelineStatusLabel` 抽到共享 helper，Dialog 删除重复 switch

## 5. 数据流

```text
用户打开 TaskDetail
  → listTaskPipelineSteps(taskId)
  → get/use automationStates[taskId]
  → TaskPipelineProgress (steps.length > 0)

backend 推进步骤
  → emit task-automation-state-changed { task_id, phase }
  → Detail/Dialog listener (task_id match)
  → listTaskPipelineSteps + refresh automation state
  → UI 更新当前步

用户点重试/转人工
  → 既有 retry/abort command
  → activity log (backend 已有)
  → 事件 + 前端 refresh
```

## 6. Compatibility

- **SSH**：无新远程 IO；会话日志沿用现网 SessionLogDialog / session_id
- **四引擎**：可视化不感知 provider；步骤员工名来自 employees map
- **无 migration**（默认）；禁止前端写 SQL
- **Activity**：复用已有 `task_pipeline_*` 中文映射；新增 key 必须双端同步

## 7. Trade-offs

| 方案 | 取舍 |
|------|------|
| 详情为主 vs 仅弹窗 | 用户已选详情为主：默认可见性更好，TaskDetail 改动略多 |
| 看板用游标 vs 拉全 steps | 游标避免 N+1；牺牲精确 /M（可接受） |
| 事件刷新 vs 轮询 | 事件对齐现网 automation；活跃时可辅以打开态 5–10s 兜底轮询（可选，非必须） |
| 抽共享组件 vs 复制 UI | 抽组件防 Dialog/Detail 漂移 |

## 8. Rollback

- UI-only 回滚：移除 `TaskPipelineProgress` 挂载即可；编排 runtime 不受影响
- 无 schema 变更则无需 DB 回滚
- 若曾加 command/字段：migration 只增可空列，可关 UI 入口

## 9. Testing strategy

- 前端：优先纯函数（状态汇总、label、可见性）若引入 `frontend-test-net` 可测；本任务至少 `npm run build`
- 手工冒烟：
  1. 有 steps 任务 → 详情见进度
  2. 启动编排 → 进度随步前进（或刷新后正确）
  3. 失败 → 错误摘要 + 重试/转人工
  4. 无 plan/steps 任务 → 无空壳
  5. 看板编排中徽标
- Rust：仅在改 backend 时 `cargo test` + clippy
