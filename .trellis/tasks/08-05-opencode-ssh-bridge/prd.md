# OpenCode SSH 远程补齐

## Goal

消除 OpenCode 在 SSH 项目上的硬缺口：远程可启动/管理会话（在技术约束内），或提供与本地等价的明确降级路径且不静默失败。目标是 **SSH 模式下 OpenCode 可真实干活**，而不是仅本地可用。

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| 远程通道 | 对齐 Codex：**远程 Node + OpenCode SDK bridge**（非 Claude/Grok 式远程 CLI） | 2026-08-05 |
| 失败语义 | 成功可干活；失败必须中文受控原因，禁止「尚未实现」裸错 | 2026-08-05 |
| one-shot | SSH OpenCode 一并打通远程 bridge `one_shot` | 2026-08-05 |

## Background

- 代码证据：`opencode/process/mod.rs` 对 SDK bridge 远程返回「尚未实现」。
- 其它引擎（Codex/Claude/Grok）已有不同程度 SSH 支持；Codex 已有远程 SDK layout 可作镜像基线。

## Requirements

### R1 远程会话

- SSH 项目可启动 OpenCode 会话（优先对齐现有远程进程/工作目录校验）。
- 若不支持 SDK bridge，提供可用的 CLI/远程替代路径并在 UI 标明模式。

### R2 生命周期

- stop / resume（在引擎能力内）与会话落库一致（`codex_sessions` + `ai_provider`）。

### R3 产物

- 远程文件变更/基线采集与现有 SSH artifact 模式兼容；受限时走统一提示（与 ux-trust-hardening 联动）。

### R4 设置与健康

- 远程 OpenCode 可用性检测/安装路径与 SSH 设置一致。

## Acceptance Criteria

- [ ] SSH 项目可启动 OpenCode 并产生会话记录（或受控失败带中文原因，不再「未实现」裸错）
- [ ] 停止后状态正确
- [ ] 与 local 项目互不污染
- [ ] 相关 Rust 测试 + build/clippy 通过

## Out of Scope

- OpenCode 云托管多租户
- 与 Codex 完全相同的 send_input 协议（可归 engine-capability-parity）
