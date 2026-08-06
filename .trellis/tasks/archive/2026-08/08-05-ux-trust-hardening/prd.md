# 任务详情主路径与 SSH 可信提示

## Goal

降低两类信任问题：

1. 任务详情/卡片上「不知道点哪」——按状态给出**唯一主路径 CTA**  
2. SSH 产物不完整却被当成完整审查依据——在**全局可感知**处提示风险

## Background / Confirmed Facts

- `TaskDetailDialog`（~1500 行）将运行、审核、验收等操作分散在多个 Tab；对话框级无单一主 CTA。
- `TaskCard` 已有主操作条（运行/停止/审核）与大量菜单项，条件分散在组件内。
- 自动化展示统一入口已存在：`getTaskAutomationDisplayState`、`getTaskActionRuntimeState`（`src/lib/utils.ts`）；卡片与详情均会读 `taskStore.automationStates`。
- SSH artifact 模式：`local_full` / `ssh_full` / `ssh_git_status` / `ssh_none`；`isArtifactCaptureLimited` + `SshArtifactLimitedNotice` 已在会话/变更史等深层使用。
- `MainLayout` 在 `environmentMode === "ssh"` 时已有顶栏 amber 提示，但文案较弱、无设置跳转、未组件化。

## Requirements

### R1 状态驱动主 CTA

- 以纯函数（见 `design.md`）根据 status + automation + 运行中 session + 指派/能力，计算**唯一**主按钮。
- 至少覆盖：运行、停止、审核/审核中、修复中…（automation 占用）、提交、验收（及 blocked soft CTA）。
- 次要操作进入菜单/既有 Tab，不与主 CTA 抢主色。
- `TaskCard` 与 `TaskDetailDialog` **同源解析器**。

### R2 自动化状态单一数据源

- 卡片与详情均经 `getTaskAutomationDisplayState` + `getTaskActionRuntimeState`。
- 自动化/pipeline 锁定时主 CTA 不得呈现为可启动「运行」却实际失败。

### R3 SSH 全局提示

- 全局 SSH 模式：顶栏 sticky banner 明确「审查依据可能不完整」，可跳转设置。
- 会话 artifact 受限：继续使用（可微调）`SshArtifactLimitedNotice`，用户在看板/主壳层即可感知 SSH 风险，无需先点进变更明细。

### R4 活动

- 默认不新增 activity action；若新增，必须补 `getActivityActionLabel` 中文。

## Acceptance Criteria

- [ ] 至少 5 种常见状态下主 CTA 语义正确：todo 运行、in_progress 运行中/停止、review 审核、completed 可提交或验收、blocked soft CTA（或 automation 修复中 disabled）
- [ ] SSH 模式下用户在看板或详情壳层无需进入深层会话即可看到提示
- [ ] 不回归现有自动化按钮锁定逻辑
- [ ] `npm run build` 通过

## Out of Scope

- 完整重写 `TaskDetailDialog` 信息架构
- 新后端 command / migration / 改 artifact 采集实现
- 本地模式下对 SSH 项目强制全局 banner

## Technical Notes

- 设计见 `design.md`；执行清单见 `implement.md`。
- 无 Rust 变更预期；验证以 frontend build + 手工状态矩阵为主。
