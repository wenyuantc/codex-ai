# Implement · native 会话内子 Agent

1. 权限 FIFO：`manager.rs` 队列化 `pending_permission`；`session.rs` 入队不再 Deny 旧请求；resolve 后 emit 下一条；stop 全 Deny。单测：两条 High 同时请求，resolve 第一条后第二条仍在。
2. `ModelClient` Clone；`McpSession` 经 `Arc<Mutex<_>>` 串行 `call`。空 MCP 不强制加锁成本以外的行为变化。
3. `catalog.rs` 增加 `agent_tool_spec`；`identity.md` 写委派约定。
4. 新文件 `native/agent/subagent.rs`：parse / type / depth / 并发常量 / 装配 child / 截断 / 事件前缀。
5. `loop.rs`：`depth`；仅 depth=0 且非 read_only 注入 Agent；`consume_assistant` 接收 `Option<&ModelClient>`；连续 Agent 走 JoinSet。
6. `session.rs`：usage/activity 回调接到子循环；不注册新会话、不进 run_queue。
7. 前端：`getLineColor` 识别 `[子 Agent]`；`activity.json` zh-CN+en 两 key；`utils.test.ts` / `locale.test.ts`。
8. 文档：`native/README.md`、`.trellis/spec/backend/ai-engines.md`（Tools 含 Agent；Permission 改为 confirm-high-risk）、`TASK.md` 勾选子 Agent。
9. 门禁：clippy -D warnings、`cargo test --manifest-path src-tauri/Cargo.toml native::`、format:check、test:ci、build。

风险文件：`native/agent/loop.rs`、`native/tools/dispatch.rs`、`native/tools/mcp.rs`、`native/manager.rs`、`native/session.rs`。

回滚点：步骤 1 可独立合入（即使不做子 Agent 也修并行确认）。步骤 4–6 失败可只留 FIFO + Mutex。
