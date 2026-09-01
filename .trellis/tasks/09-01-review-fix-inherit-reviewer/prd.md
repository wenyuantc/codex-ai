# 审核页新建修复任务未继承审查员

## Goal

任务详情「审核 → 新建修复任务」创建的新任务必须带上原任务已指定的审查员（以及同一套人员指派里的协调员、Native 子代理），避免设置开启「新建任务默认自动质控」时误报「请先指定审查员」。

## Requirements

1. 新建修复任务走现有 `create_task`，不改后端「默认自动质控必须有审查员」的校验。
2. 创建 payload 必须包含 `reviewer_id`：优先详情侧栏当前值，否则用原任务已保存的 `reviewer_id`。
3. 同一 payload 一并带上 `coordinator_id`、`native_subagent_id`（同样侧栏优先，否则原任务字段）。
4. 原任务没有审查员时不要伪造 id；默认自动质控开启时仍由后端返回现有错误文案。
5. 创建后的立即运行路径不变：仍检查审核报告、开发负责人、仓库路径，成功后 `startTaskRunSession`。
6. 不复制标签、依赖、附件、file refs；不引入全局默认审查员。

## Acceptance Criteria

- [x] 原任务已指定审查员且设置开启「新建任务默认自动质控」时，「新建修复任务」不再弹出「请先指定审查员」。
- [x] 新建出的修复任务 `reviewer_id` 与详情侧栏当前审查员一致；侧栏为空时回退到原任务已保存的审查员。
- [x] 侧栏审查员与 `task.reviewer_id` 不一致时，以侧栏为准。
- [x] 原任务没有审查员时 payload 不含伪造的 `reviewer_id`。
- [x] 纯函数单测覆盖以上三种审查员场景；`npm run test:ci`、`npm run format:check`、`npm run build` 通过。

## Notes

- 根因：`TaskDetailDialog.handleConfirmReviewFix` 调用 `createTask` 时只传了 assignee，没传 reviewer。
- 与 Native Agent 缺陷任务无关，不要改 `src-tauri/src/native/`。
