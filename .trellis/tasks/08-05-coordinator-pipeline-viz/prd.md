# 协调员编排可视化

## Goal

把协调员编排 v2 已有的「后台流水线 + 弹窗步骤列表」升级为用户可理解的 **阶段视图**：在任务详情默认可见当前/历史阶段与失败点，看板有轻量进度提示，计划弹窗保留完整操作（重试 / 转人工 / 跳会话），减少「编排在跑但看不懂到哪一步」的黑盒感。

## Background / Confirmed Facts

1. `08-03-coordinator-orchestration-v2` 已落地：结构化 `task_pipeline_steps`、串行调度、`start/retry/abort`、activity 中文 key、`task-automation-state-changed` 事件。
2. 前端 IPC 已有：`listTaskPipelineSteps` / `startTaskPipeline` / `retryTaskPipelineStep` / `abortTaskPipeline` / `updateTaskPipelineStep`。
3. `CoordinatorPlanDialog` 已有步骤列表、状态中文、失败摘要、会话日志入口、重试/转人工/刷新；**主要埋在弹窗内**。
4. `TaskOverviewPanel` / 看板卡片 **没有** 默认可见的阶段条或 N/M 进度；步骤刷新依赖打开弹窗与手动「刷新」。
5. 看板已有 `onTaskAutomationStateChanged` 监听，但主要用于 git overview 刷新，**未驱动步骤列表重拉**。
6. 父任务约束：前端不直写 SQL；local + SSH 一等；活动 key 中文；时间走 `formatDate()`。

## Key Decisions

| ID | 决策 | 结论 | 日期 |
|----|------|------|------|
| D1 | 阶段视图主入口 | **详情为主 + 看板轻提示**：TaskDetail 概览展示阶段条/步骤时间线（有 steps 才显示）；看板卡片仅「编排中 N/M」或失败徽标；`CoordinatorPlanDialog` 增强时间线与操作 | 2026-08-05 |
| D2 | 实时更新 | 优先复用 `task-automation-state-changed` + 现有 automation 状态拉取；打开详情/弹窗时加载 steps；pipeline 活跃时对当前任务重拉 steps（可短节流） | 2026-08-05 |
| D3 | Backend 范围 | **默认不扩编排运行时**；仅当可视化发现缺字段/缺事件时做最小补齐。操作对齐已有 start/retry/abort | 2026-08-05 |

## Requirements

### R1 阶段模型展示（详情）

- 当任务存在 `task_pipeline_steps`（长度 > 0）时，在 **任务详情概览** 展示阶段进度：
  - 摘要：当前步 index / 总数、整体状态（待开始 / 编排中 / 失败 / 已完成 / 已转人工）。
  - 阶段条或纵向时间线：每步标题 + 状态（pending / launching / running / succeeded / failed / cancelled / skipped）中文标签。
- 无 steps 的任务 **不渲染** 空阶段面板（无空壳噪音）。
- 仅有 `plan_content`、尚未生成结构化 steps 时：不展示假进度；可通过既有「协调员计划」入口生成。

### R2 看板轻提示

- 当 `pipeline_active` 或存在 running/launching/failed 步骤语义可判断时，看板卡片展示轻量文案/徽标，例如「编排 2/5」或「编排失败」。
- 不在看板渲染完整步骤列表。
- 无 steps / 非编排任务不增加额外噪音。

### R3 会话联动与失败信息

- 详情时间线与计划弹窗：失败步骤展示 `last_error` 摘要（截断可读）。
- 有 `session_id` 的步骤可打开既有会话日志（复用 `SessionLogDialog` 路径）。
- 成功步骤可展示 `handoff_summary` 摘要（折叠或单行，避免刷屏）。

### R4 操作（对齐现网 backend）

- 在计划弹窗（及详情区若放快捷入口）支持：
  - **重试失败步骤** → `retryTaskPipelineStep`
  - **转人工 / 取消编排** → `abortTaskPipeline`
  - **刷新** → `listTaskPipelineSteps`（+ 必要时刷新 automation state）
- 不新增「跳过步骤」；不改「立即执行」语义。
- `actionsLocked` 等执行中锁逻辑保持与现网一致。

### R5 中文、活动、兼容

- UI 文案中文；步骤/ phase 状态复用或集中 `pipelineStatusLabel` 一类 helper，避免三处漂移。
- 既有 pipeline activity key 已有中文映射；若新增 action 必须补 `getActivityActionLabel`。
- 大文本计划仍用 Monaco；时间字段用 `formatDate()`。
- 不破坏 SSH 项目上的编排（可视化本身不区分 local/SSH；会话跳转与现网一致）。

## Acceptance Criteria

- [ ] AC1：存在 pipeline steps 的任务，打开任务详情即可看到阶段进度（不必先开协调员计划弹窗）。
- [ ] AC2：编排运行中，详情/弹窗步骤状态可随 `task-automation-state-changed`（或等价刷新）更新到当前步；支持手动刷新。
- [ ] AC3：无 steps 的任务详情与看板不出现空阶段面板/假进度。
- [ ] AC4：看板在编排中显示 N/M（或等价轻提示）；失败时有可识别徽标/文案。
- [ ] AC5：失败步骤可见错误摘要；有 session 的步骤可打开执行日志。
- [ ] AC6：弹窗内重试 / 转人工 / 按计划编排仍可用，且与 backend 行为一致。
- [ ] AC7：`npm run build` 通过；若改 Rust 则 clippy + 相关 cargo test 通过。
- [ ] AC8：新增 activity action（若有）仪表盘中文可读。

## Out of Scope

- 通用 BPM / 跨任务全局编排画布
- 并行 fan-out、跳过步骤、动态重规划
- 重写编排运行时或 Meta-Agent
- 测试员自动验收门禁（属其它子任务）
- 为可视化单独新增复杂 DB 表（除非缺字段阻塞 AC）

## Risks

| 风险 | 缓解 |
|------|------|
| TaskCard / TaskDetailDialog 管道逻辑重复，改 UI 易双份漂移 | 抽共享 `TaskPipelineProgress`（展示）+ 可选 hook 拉 steps |
| 仅靠 phase 事件不带 steps 快照 | 事件触发后 `listTaskPipelineSteps` + `getTaskAutomationState` |
| 看板 N/M 需要 steps 全量导致 N+1 | 优先用 `automationStates.pipeline_active` + `pipeline_step_index`；steps 仅在详情/弹窗拉；看板文案可用 index+1 与「编排中」而不强求总数，若需总数则打开详情再精确或缓存轻量聚合（实现时选成本更低者并写清） |
| 与 ux-trust / tester 子任务同改 TaskDetail | 本任务限定 pipeline 可视化文件面；尽量独立组件 |

## Open Questions

（无阻塞项）
