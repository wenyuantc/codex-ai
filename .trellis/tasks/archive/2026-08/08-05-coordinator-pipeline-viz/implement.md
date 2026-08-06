# Implement — 协调员编排可视化

## Pre-flight

- [ ] 用户已批准本任务最终规划摘要
- [ ] `python3 ./.trellis/scripts/task.py start .trellis/tasks/08-05-coordinator-pipeline-viz`
- [ ] 加载 `trellis-before-dev`（frontend 为主，必要时 backend）
- [ ] 分支：`feat/coordinator-pipeline-viz`（已存在则沿用）

## Ordered checklist

### 1. 共享展示与文案

- [ ] 新增 `src/components/tasks/detail/TaskPipelineProgress.tsx`
  - steps 为空 → `null`
  - header：进度文案（当前/总数 + 整体状态）
  - stage strip + timeline 列表
  - 失败 `last_error`、可选 `handoff_summary`、时间 `formatDate`
  - `onOpenStepSession` 回调
- [ ] 抽取 `pipelineStatusLabel` / `getPipelineProgressSummary` 到 `src/lib/utils.ts`（或 `src/lib/pipelineUi.ts` 若 utils 过重）
- [ ] 状态色与中文与现 Dialog 一致；`skipped`/`cancelled` 覆盖

### 2. 任务详情接入

- [ ] `TaskDetailDialog`：打开/切 task 时 `listTaskPipelineSteps`；保持与计划流共用 state
- [ ] 订阅 `onTaskAutomationStateChanged`（组件 unmount 取消）：匹配 `task.id` 时刷新 steps + automation
- [ ] `TaskOverviewPanel`：接收 steps/automation/loading；渲染 `TaskPipelineProgress`；「打开编排」入口
- [ ] 无 steps 不渲染 section

### 3. 计划弹窗增强

- [ ] `CoordinatorPlanDialog` 步骤区改为复用 `TaskPipelineProgress`（保留执行人 select 与操作条）
- [ ] 删除 Dialog 内重复 `pipelineStatusLabel` switch
- [ ] 弹窗打开 + pipeline phase 事件时自动 `onRefreshPipeline`
- [ ] 确认 retry/abort/start 接线不变

### 4. 看板轻提示

- [ ] `TaskCard`：从 `automationStates[task.id]` 读 `pipeline_active` / `pipeline_step_index` / phase
- [ ] 展示「编排中 · 第 k 步」或「编排失败」；无活跃编排不展示
- [ ] **不要** 为每张卡请求 `listTaskPipelineSteps`
- [ ] 确认 `taskStore` 已对 active tasks 拉 automation（现有逻辑）；必要时仅在 phase 事件后刷新对应 task automation

### 5. 事件与 store 粘合（最小）

- [ ] 详情/弹窗路径保证事件后 steps 更新
- [ ] 若看板徽标滞后：在既有 kanban / store 监听里对 `event.task_id` 调 `getTaskAutomationState` 写回 `automationStates`（避免全量 fetchTasks 风暴；可节流）

### 6. Activity / 中文（按需）

- [ ] 核对 `getActivityActionLabel` 已覆盖 pipeline keys（已有则跳过）
- [ ] 若新增 action：Rust `insert_activity_log` + 中文 label 同步

### 7. Backend 最小补齐（仅阻塞时）

- [ ] 默认跳过
- [ ] 仅当确认缺字段/事件导致 AC 失败：最小 diff（例如 event 增加 `pipeline_step_index`）+ 测试 + migration（若有）

## Validation

```bash
npm run build
npm run lint          # 若改动面大
# 仅当改了 Rust:
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

手工冒烟：

1. 有 steps 的任务 → 详情见阶段条
2. 按计划编排 → 当前步推进（事件或刷新）
3. 人为失败（或已有失败态）→ 摘要 + 重试/转人工
4. 无 steps 任务 → 无空壳
5. 看板编排中徽标
6. SSH 项目（若环境有）：打开详情不报错，步骤 session 日志可开

## Risky files / rollback points

| 文件 | 风险 | 回滚 |
|------|------|------|
| `TaskDetailDialog.tsx` / `TaskCard.tsx` | 逻辑重、易回归执行锁 | 组件独立可先卸挂载 |
| `CoordinatorPlanDialog.tsx` | 操作回归 | 保留原 props 接线 |
| `taskStore.ts` | 事件刷新风暴 | 节流 / 仅单 task 刷新 |
| `pipeline.rs` | 仅在做 backend 补齐时 | 避免动调度核心 |

## Definition of Done

- [ ] PRD AC1–AC8 满足
- [ ] `implement.jsonl` / `check.jsonl` 已有真实 spec 条目
- [ ] `trellis-check` 通过
- [ ] Conventional Commit（如 `feat(pipeline): 任务详情编排阶段可视化`）
- [ ] 子任务可 archive（父任务仍保持跟踪）

## Do not start until

用户对本规划摘要的 **显式批准** 之后，再 `task.py start`。
