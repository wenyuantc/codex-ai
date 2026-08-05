# 引擎能力对齐与会话体验

## Goal

让用户 **看清** 并 **尽量使用** 四引擎真实能力：Codex 保持完整；Claude/Grok/OpenCode 在可行范围内补齐；不可补齐处用统一能力徽章与禁用说明，避免「点了没反应」。长会话日志可读、可滚、不卡。

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| 对齐目标 | **诚实边界 + 尽力补齐**：不承诺四引擎行为完全一致 | 2026-08-05 |

## Background

- `get_ai_provider_capabilities`：Codex 支持 start/stop/restart/send_input/resume；其余多为 start/stop/resume。
- 会话页与运行入口若未按能力门禁，会造成假操作。

## Requirements

### R1 能力模型

- 后端能力矩阵为唯一真源；前端统一读取展示。
- UI：引擎徽章 + 无能力按钮禁用 + tooltip 原因。

### R2 尽力对齐（在引擎 CLI/SDK 允许范围内）

- 评估并为 Claude/Grok/OpenCode 增加 restart 和/或 send_input（能做就做，不能做则文档化 + UI 禁用）。
- 不伪造已支持。

### R3 会话日志体验

- 长日志虚拟化或窗口化；搜索/过滤保留。
- 续聊/停止状态与能力一致。

### R4 活动与设置

- 设置页展示各引擎能力对照表。

## Acceptance Criteria

- [ ] 无能力操作不可点，且有中文说明
- [ ] 设置页有四引擎能力对照
- [ ] 已补齐的能力有 Rust 单测或集成烟测路径
- [ ] 长会话滚动无明显卡顿（手工标准：千级事件可浏览）
- [ ] build + clippy 通过

## Out of Scope

- 重写全部引擎 stream 协议
- 保证四引擎 100% 行为一致（CLI 差异允许）
