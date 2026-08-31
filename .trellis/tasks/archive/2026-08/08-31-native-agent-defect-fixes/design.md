# Design · Native Agent 缺陷修复

## Boundaries

- 预算 / last_turn / child quota：`native/agent/loop.rs` + `compact.rs` 的 `RolloutBudget` / `ChildQuota`
- Bash 分类：`native/tools/permission.rs`（本地与 SSH 共用 `dispatch`）
- 截断 offset：`native/agent/truncate.rs`，Read 仍是 1-based 行号
- Compact handoff / 摘要质量：`compact.rs` + `loop.rs::compact_with_model`
- max_tokens 重试 / 模型列表：`native/model/client.rs` → `channels.rs` → `ListAiChannelModelsResult` → 设置页
- Transcript：`transcript.rs` + `session.rs::persist_native_transcript`

## Contracts

- 有限预算：`request_cap = min(available, caller.unwrap_or(16384))`；无限预算不改写调用方。
- last_turn 拆两种：planned（`tools_now` 空）丢弃 tool_calls；带 tools 的请求即使 settle 后耗尽也执行本批。
- 同一 `run_agent_batch` 共用一个 `Arc<ChildQuota>`。
- `list_models` 返回 `{ models, truncated }`；IPC 字段 `truncated: bool`，serde default false，无 migration。
- 活动 key 不变：`ai_channel_models_fetched`。

## Tradeoffs

- Bash 用递归 unwrap，不是 POSIX AST：漏判面缩小，解析失败走 Opaque。
- Compact 不塞完整历史，handoff 约 24k 字，避免 compact 自己打满预算。
- Transcript 用指纹去重，不做分片表。

## Rollback

各改动独立，可按文件回退。IPC `truncated` 有 serde default，旧前端可忽略。
