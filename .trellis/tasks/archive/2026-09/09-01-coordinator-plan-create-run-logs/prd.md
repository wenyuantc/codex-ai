# 看板创建并运行后协调员计划弹窗无终端日志

## Goal

看板「新建任务 → 创建并运行」在指定协调员时，后台生成计划期间，用户打开「协调员执行计划」弹窗应能看到与「重新生成计划」相同的实时终端日志（`[计划]` / `[读取]` / `[工具]` / `[思考]` 等），而不是空白「等待运行日志...」。

## Confirmed Facts

- 「创建并运行」走 `continueCreatedTaskRun` → `runExistingTaskInBackground(regenerateCoordinatorPlan: true)` → `generateAndPersistCoordinatorPlan`。
- `generateAndPersistCoordinatorPlan` 只调用 `aiGenerateCoordinatorTaskPlan`（无 `request_id`、无 `withCoordinatorPlanLogStream`），也不写入 `coordinatorPlanStore`。
- 弹窗日志来自 `coordinatorPlanStore`；「运行 / 重新生成计划」走 `generateCoordinatorPlanForTask`，会挂日志流并 `appendLog`。
- 两条路径共用 `runExclusiveCoordinatorPlanGenerate`。后台生成占锁时，打开弹窗只能把 `terminalVisible` 设为 true，无法接到过程行。
- 后端 `ai_generate_coordinator_task_plan` 已落库 `plan_content` 与工作包；不需要第二条前端 `updateTask` 才能持久化。
- 原产品决策：创建并运行不弹确认框、后台生成后自动执行。本任务不改变该决策，只补齐弹窗可观察性。

## Requirements

1. 创建并运行（有协调员）后台生成计划时，必须走与弹窗「重新生成计划」相同的生成+日志流路径，把过程行写入 `coordinatorPlanStore`。
2. 生成过程中或刚结束后打开协调员执行计划弹窗，终端默认可见，能看到正在跑/刚跑完的计划日志，而不是空终端或仅「已加载已保存计划」。
3. 仍保持创建并运行不强制弹出计划确认框；失败时保留已创建任务、不跳过计划直接执行（现网行为）。
4. 本地与 SSH 项目行为一致。过程日志仍可不随弹窗关闭而持久化到 DB（与现网「重新生成计划」一致）。
5. 看板卡片「运行」打开弹窗并重新生成计划的现网行为不变。

## Acceptance Criteria

- [ ] AC1：看板新建任务（有协调员）点「创建并运行」，生成计划期间打开协调员执行计划弹窗，终端显示实时过程日志，而不是「等待运行日志...」。
- [ ] AC2：同一弹窗点「重新生成计划」仍实时显示终端日志（回归）。
- [ ] AC3：无协调员的创建并运行不走计划生成，执行终端行为不变。
- [ ] AC4：计划 Markdown 与工作包仍落库；`task_plan_generated` 活动日志仍写。
- [ ] AC5：`npm run test:ci`、`npm run format:check`、`npm run build` 通过。

## Out of Scope

- 创建并运行自动弹出协调员计划确认框
- 过程日志落库 / 跨刷新回放
- 编排步骤会话日志
- 执行终端（`codex-stdout` / `taskLogs`）展示
