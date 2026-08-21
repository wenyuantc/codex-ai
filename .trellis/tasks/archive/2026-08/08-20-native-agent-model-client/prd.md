# PRD · 三协议模型客户端

> 父任务：`08-20-native-agent`。依赖渠道的 base_url / protocol / key，不依赖 UI。

## Goal

Rust 内提供统一 Chat/Stream 接口，对接 OpenAI Chat Completions、Anthropic Messages、OpenAI Responses（Codex）。

## Requirements

- R1 内部类型：`Message` `ToolCall` `StreamEvent` `Usage`（prompt/completion/cached）。
- R2 `openai`：`/v1/chat/completions` SSE；tool_calls 增量拼接。
- R3 `anthropic`：`/v1/messages` SSE；`anthropic-version` 头；content blocks → 统一 Message。
- R4 `codex`：`/v1/responses` SSE；input items；`function_call` / `function_call_output` 往返。
- R5 429/5xx 有限重试；API key 不写日志。
- R6 mock HTTP 单测覆盖三协议：纯文本流、带 tool-call、usage 解析。
- R7 可供渠道测通与后续 agent loop 调用。

## Out of Scope

- Agent 循环、工具执行、会话管理
- ChatGPT 网页 Codex 后端

## Acceptance Criteria

- [x] 三协议都能把「助手文本 + 一个 tool call」解析成内部 Message
- [x] 把 tool 结果喂回去能构造合法第二轮请求体（单测断言 JSON 形状）
- [x] 错误含 HTTP 状态，不含 Authorization
- [x] clippy / cargo test 通过
