# 协调员编排可视化

## Goal

把协调员编排 v2 的「后台流水线」变成用户可理解的 **阶段视图**：当前阶段、历史阶段、失败点、可重试入口，减少黑盒感。

## Background

- `08-03-coordinator-orchestration-v2` 已落地计划生成与 pipeline 执行优先级。
- 用户侧缺少时间线/阶段条等可视化。

## Requirements

### R1 阶段模型展示

- 展示协调员计划步骤与执行状态（pending/running/done/failed/skipped）。
- 任务详情或独立面板可打开。

### R2 与会话联动

- 步骤可跳到关联 session（若有）。
- 失败步骤显示错误摘要。

### R3 操作

- 在安全前提下支持「从失败处继续 / 取消编排」（对齐现有 backend 能力，缺则最小补齐）。

### R4 中文与活动

- UI 中文；关键状态变更写 activity。

## Acceptance Criteria

- [ ] 有协调员计划的任务可看到阶段进度
- [ ] 运行中阶段实时或可刷新更新
- [ ] 无计划任务不展示空壳噪音
- [ ] build 通过

## Out of Scope

- 通用 BPM 引擎
- 跨任务全局编排画布（v1 以单任务为主）
