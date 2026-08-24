# Implement · P0-2a native 高风险工具权限确认

1. `classify_native_tool_risk` + 单测（覆盖/删除/push/force git/MCP/低风险）
2. `ToolCtx` 增加确认回调；`execute_tool` 高风险先 await
3. `NativeLiveSession`：pending request + `allow_all_high_risk`；命令 `resolve_native_tool_permission`（allow_session | allow_once | deny）
4. 前端 Dialog 三按钮（本会话全部允许 / 仅允许一次 / 不允许）；关闭=deny；停止会话 deny 未决
5. 改 `identity.md` 与 environment permission 文案
6. 会话事件：permission_requested / allowed / denied
7. 门禁：clippy、cargo test native、format:check、test:ci、build

风险文件：`native/tools/dispatch.rs`、`native/agent/loop.rs`、`native/session.rs`、`native/prompt/*`、会话 UI

依赖：P0-1 合入后给 MCP 工具打 High(mcp)；若 P0-1 未完成，先对未知工具名当 High。
