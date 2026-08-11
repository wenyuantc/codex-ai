# 真会话 send_input

## Goal

为 AI 引擎提供**真实**的会话中可写输入路径（非停止重跑、非新开会话冒充），并在能力矩阵与 UI 中诚实开放。

## Background（证据）

- `get_ai_provider_capabilities` 四引擎 `send_input: false`
- `send_codex_input` 恒失败（非交互批处理文案）
- 共享 engine spawn 多为 `Stdio::null()`；Codex 仅在启动时短暂写 stdin 后 shutdown
- 历史错误宣称 Codex 支持已被纠正；本任务禁止再伪造

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| 引擎范围 | **四引擎同波**（父决策 B） | 2026-08-10 |
| 不可行豁免 | **B1**：证据充分时可保持 `false`；验收底线 ≥2 引擎真实 `true` 且含 Codex | 2026-08-10 |
| UI 入口 | **U1**：活终端内联输入条（任务日志 / Session 日志 / 员工运行终端共用） | 2026-08-10 |

## Requirements

- Codex / Claude / Grok / OpenCode 均需评估并尽力实现真实 mid-session 可写路径（Local；SSH 能做则做，不能则禁用并说明）
- 矩阵 `true` 仅当该引擎存在可验证的 mid-session 写入路径
- 有可复核证据证明不可行时：矩阵保持 `false`、UI 禁用 + 中文原因，并在子任务 Notes/文档记录证据
- UI：在运行中终端视图内联输入+发送；仅 `can(..., "send_input")` 且会话存活时可点；三入口共用组件
- 统一或分引擎 `send_*_input` command；禁止恒失败 stub 冒充支持

## Acceptance Criteria

- [ ] 至少 **2** 个引擎（**必须含 Codex**）具备可验证 mid-session `send_input=true`
- [ ] 其余引擎：实现成功则 `true`，否则 `false` + 豁免证据与 UI 说明
- [ ] 任务日志 / Session 日志 / 员工运行终端三处均可在支持时发送；无能力时禁用并有中文（后接 i18n）说明
- [ ] 能力矩阵与 UI 一致；无能力引擎仍 fail-closed
- [ ] 有 Rust 单测或集成烟测覆盖「可发送 / 无会话 / 不支持」路径
- [ ] 文档更新诚实边界；`TASK.md` 对应项可勾选

## Out of Scope

- 用 resume / 新会话伪装 send_input
- 保证四引擎 100% 协议/延迟一致（允许引擎差异，但矩阵必须诚实）

## Notes

仓库现状：多数会话 spawn 启动后关闭或 null stdin；Codex `send_codex_input` 仍为恒失败 stub。实现需改共享内核与/或各引擎 session runtime。

**依赖顺序（父 O1）**：本子任务在 `08-10-p3-reports` 验收后启动；完成后才启动 `08-10-p3-i18n`（避免 U1 新文案被 i18n 二次返工遗漏）。
