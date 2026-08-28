# 首次执行中自动质控徽章显示等待执行完成后审核

## Goal

看板/详情里，自动质控已开启且相位仍是 `idle` 时，如果该任务正在跑首次开发执行，徽章不要显示「待命」，改成「执行中」，避免用户以为闭环没接上。

## Background

自动质控闭环等**执行成功退出**后才启动审核。首次执行期间后端相位就是 `idle`。任务 `52fd7af6-f9a0-43c9-a188-0b552c6b18ad` 已核实：执行结束后相位切到 `waiting_review`，功能正常。问题只在展示。

## Requirements

1. `phase=idle` 且本任务有 running **execution** 会话时，自动质控状态文案为「执行中」（en: Executing）。
2. `phase=idle` 且没有 execution 会话时，仍显示「待命」/ Idle。
3. `waiting_review`、`waiting_execution` 等其它相位不被 overlay。
4. 运行中判定用真实 execution 会话（`executionActions.isRunning`），不含编排 pipeline、协调员生成计划、排队、审核会话。
5. **不改** `task_automation_state.phase`、不改 CTA/重启/归档对 `idle` 的判断；只改展示文案。
6. 文案走 i18n，zh-CN + en。
7. 覆盖卡片徽章、卡片右键「当前：」、详情底部自动化标签、详情概览「闭环阶段」。

## Acceptance Criteria

- [ ] 开启自动质控的任务首次执行中：卡片徽章为「自动质控·执行中」
- [ ] 同一任务无运行中 execution：徽章仍为「自动质控·待命」
- [ ] 审核已启动（`waiting_review`）：仍为「自动审核中」，即使误传 executionRunning
- [ ] 自动修复中（`waiting_execution`）：仍为「自动修复中」
- [ ] `TaskAutomationDisplayState.status` 保持 `idle`，不发明新相位
- [ ] `npm run test:ci`、`npm run format:check`、`npm run build` 通过

## Out of Scope

- 改 Rust 自动质控状态机或新增 SQLite 相位
- 内置 Agent 停止后误触发审核（`08-25-native-stop-no-review`）
- 协调员编排自动推进
- `last_codex_session_id` 未回写
- 无关未提交改动（如 `AiChannelsSettingsTab`）
