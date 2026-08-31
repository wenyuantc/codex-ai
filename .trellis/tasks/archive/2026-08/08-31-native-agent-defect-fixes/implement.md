# Implement · Native Agent 缺陷修复

1. `#1` `loop.rs` `reserve_model_call` 有限预算必带 cap；`settle_model_usage` 无 usage 按文本估算
2. `#5` 带 tools 的请求 settle 耗尽后仍执行 tool_calls；planned last_turn 不变
3. `#9` `run_agent_batch` 共享一个 ChildQuota；更新设置 hint
4. `#2` `permission.rs` unwrap 包装器 / git 全局选项 / 解释器
5. `#3` `truncate.rs` head 收到完整行再算 offset
6. `#4` `compact.rs` `handoff_transcript`；放宽 local_summary
7. `#6` `compact_with_model` 可用性校验 + 一次纠偏
8. `#7` `client.rs` max_tokens 最多 3 次下调
9. `#8` `list_models` truncated → IPC → 设置页 + 活动日志文案
10. `#11` transcript 指纹去重 + 失败日志
11. 更新 `ai-engines.md` 与 `native/README.md`
12. clippy / cargo test native / format:check / test:ci / build

## Validation

```bash
cargo test --manifest-path src-tauri/Cargo.toml native::
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run test:ci
npm run format:check
npm run build
```
