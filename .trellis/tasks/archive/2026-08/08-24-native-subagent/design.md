# Design · native 会话内子 Agent

## 边界

只改 **native 进程内循环**。不改四引擎 CLI，不新增 Tauri 启动命令，不改 SQLite schema。

子 Agent 是 loop 层元工具，不在 `execute_tool` 里跑模型，避免 `tools` ↔ `agent` 循环依赖。

## 数据流

```
consume_assistant(client, assistant)
  扫描 tool_calls
  非 Agent：现有 execute_logged_tool（串行）
  连续 Agent：run_agent_batch
        → 解析 SubagentSpec
        → semaphore 3 + JoinSet
        → 每个：装配 child AgentRunner(depth=1)
              general: extra_tools=父 MCP specs，read_only=false
              explore: allowed=READ_ONLY，read_only=true，无 extra MCP
        → child.run_with_client(同一 ModelClient)
        → 截断最终文本作为父 tool result
        → 事件行加 `[子 Agent {n}]` 前缀
```

取消：共享 `CancelFlag`。权限：共享 requester + `allow_all_high_risk`。MCP：`Arc<Mutex<McpSession>>`。

## 合约

### Agent 工具

见 PRD。解析失败不启动。结果上限约 16KB 字符，超出截断并标注。

### 权限 FIFO

`pending_permission: VecDeque<(String, oneshot::Sender<NativePermissionDecision>)>`。

- 入队：队列从空变非空才 emit `native-permission-request`。
- `resolve_permission`：按 `request_id` 弹出对应项；再 emit 新队首。
- 禁止：新请求 `take()` 后 Deny 旧请求（`session.rs` 现逻辑必须删）。
- stop / `deny_pending_permission`：全部 Deny。

前端 Dialog 仍一次一条。

### MCP

`ToolCtx.mcp` 改为可共享的锁（空会话保持廉价）。`call` 在锁内完成一轮 JSON-RPC。SSH 远端 MCP 仍是父会话那条管道，子循环不得本机回退。

### 子 runner 装配

共享：workspace、ssh、cancel、mcp 锁、allow_all_high_risk、request_permission、on_usage、context_char_limit。

隔离：messages、todos、read_files、turns、last_tool_*。

`max_turns = min(parent, 20)`，0（不限制）时子循环仍用 20。

system = 父第一条 system + 子 Agent 附录。user = prompt。

### 活动日志

有 `task_id` 时 `insert_activity_log`。自由会话只打终端。key：`native_subagent_started` / `native_subagent_finished`。

### 回滚

去掉 `Agent` spec 与 loop 批次；权限改回单 oneshot（并行会回归旧缺陷）；MCP 改回独占 `&mut`。

## 风险

- 两 general 写同一文件：不设锁，提示词要求路径不重叠。
- 渠道 QPS：硬顶 3。
- `AgentRunner` 膨胀：装配放 `native/agent/subagent.rs`。
