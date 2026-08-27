# 内置 Agent API 报错重试

## Goal

内置 Agent 调用模型 API 遇到可恢复的服务端/网络错误时，不再立刻结束任务。最多重试 10 次、每次间隔 3 秒；每次重试前在会话终端打出 `[重试]` 行，让用户知道仍在等待。

## Background

`ModelClient::post_stream` 已有 `RetryConfig`，但默认 5 次、2s 指数退避 + 抖动。HTTP 200 网关错误、空响应、解析失败会立刻返回，会话 `break` 后任务记为 `failed`。重试过程对用户不可见。

## Requirements

1. 默认最多重试 10 次（首次失败后再试 10 次），固定间隔 3 秒，无抖动。
2. 可重试：网络/超时、HTTP 408/409/429/5xx、HTTP 2xx 的空响应/网关 error/解析失败（服务端抖动类文案）。
3. 不可重试：401/403/404 及其它 4xx、`insufficient quota` / `invalid api key` / `unauthorized`。400 `max_tokens is too large` 仍走 `chat()` 一次性降级，不占用 10 次重试。
4. 渠道测通 `RetryConfig::none()` 不重试。
5. 有会话事件通道时，每次 sleep 前立刻输出：`[重试] {错误摘要}，3 秒后进行第 n/10 次重试`（脱敏，不回显 API key）。路径与 `[思考]` / `[ERROR]` 相同：`emit` → `codex_session_events(stdout)` → `native-stdout`。
6. 10 次仍失败才 `[ERROR]` 并结束任务。用户停止时，重试等待可在约 200ms 内响应取消。
7. 无会话通道的 `run_native_one_shot` 仍做 HTTP 重试，不另造终端通道。
8. 本地与 SSH 共用同一条 in-process `ModelClient` 路径。

## Acceptance Criteria

- [x] 503 / 可恢复 200 网关错误随后成功：会话继续，不标记 failed
- [x] 每次重试前终端立刻出现 `[重试] …第 n/10 次重试`
- [x] 401 立即失败，无 `[重试]` 行
- [x] 400 max_tokens 超限仍只降级重试一次，不走 10 次 HTTP 重试
- [x] 用户停止不必空等满 3 秒
- [x] `clippy -D warnings`、`format:check`、相关 cargo test 通过

## Out of Scope

- 改 Claude / Codex / Grok / OpenCode CLI 进程重试
- 前端新 UI
- 渠道测通请求重试
