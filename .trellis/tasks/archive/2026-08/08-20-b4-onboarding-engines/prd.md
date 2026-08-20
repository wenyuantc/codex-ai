# PRD · B4 引导四引擎健康检查

父任务:`08-20-product-trust-ops` · 优先级 P1

## Goal

首次引导「检查 SDK」只调 Codex `health_check`。只用 Claude / Grok / OpenCode 的用户会被判未就绪。

## 证据

- `src/components/dashboard/OnboardingChecklist.tsx:56-60` 只 `healthCheck()`
- `src/lib/backend.ts:344` → `health_check`
- `src-tauri/src/app/database.rs:689-771` 查 Codex CLI + Codex SDK，不含其他引擎

## Requirements

1. 引导 SDK 步骤视为「至少一个已配置引擎可用」，或分引擎列出就绪/未就绪。
2. 复用设置页已有的各引擎健康检查命令，不发明新协议。
3. SSH 模式下检查远程对应引擎（已有 remote health 的用 remote；没有的标明仅本地）。
4. 跳转仍到设置 SDK 相关 section。
5. 全未就绪才显示未完成；仅 Codex 未装但 Claude 已装应算该步完成（或明确列出 Claude 已就绪）。

## Acceptance Criteria

- [x] 只装 Claude/Grok/OpenCode 之一时，引导不因 Codex 缺失而永远红
- [x] 四引擎都没有可用 CLI/SDK 时该步仍为未完成
- [x] SSH 模式不把本地 Codex 假当成远程就绪
- [x] i18n zh-CN + en

## Out of Scope

自动安装四引擎、改设置页安装流程。
