# 任务详情主路径与 SSH 可信提示

## Goal

降低「不知道点哪」与「SSH diff 不完整却误判通过」两类信任问题：任务详情按状态给出 **单一主路径 CTA**；SSH 产物捕获降级时 **全局可感知**。

## Background

- `TaskDetailDialog` 体量大，运行/审核/自动化/提交入口分散。
- SSH artifact 捕获模式：`local_full` / `ssh_full` / `ssh_git_status` / `ssh_none`；已有局部 `SshArtifactLimitedNotice`。

## Requirements

### R1 状态驱动主 CTA

- 按任务 status + automation state + 运行中 session 计算唯一主按钮（如：运行 / 停止 / 审核 / 修复中… / 提交 / 验收）。
- 次要操作收入菜单，不与主 CTA 抢视觉。

### R2 自动化状态单一数据源

- 卡片与详情展示同源；避免按钮可点但业务已锁定。

### R3 SSH 全局提示

- 当当前上下文（全局 SSH 模式或当前任务/项目为 SSH 且 artifact 受限）时，顶栏或通知区 sticky/banner 提示。
- 可跳转设置或帮助说明「审查依据可能不完整」。

### R4 活动

- 如有新 action，补中文 label。

## Acceptance Criteria

- [ ] 至少 5 种常见状态下主 CTA 语义正确（todo/in_progress 运行中/review/completed 可提交/blocked）
- [ ] SSH 受限场景下用户在看板或详情无需点进深层即可看到提示
- [ ] 不回归现有自动化按钮逻辑
- [ ] build 通过

## Out of Scope

- 完整重写 TaskDetailDialog 信息架构（可增量拆分）
