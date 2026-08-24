# Implement · P0-1 MCP 对齐

1. 扩展 `AiProviderCapabilities.mcp` + 单测 `capability_matrix_is_honest_and_complete`；前端类型、`EngineCapabilityKey`、徽章、i18n
2. 改 Settings MCP 说明与任务绑定：按员工引擎显示生效/无效
3. native MCP 客户端：stdio initialize / tools/list / tools/call；会话结束杀进程（本地 child / 远端 pid）
4. 本地 native 启动合并动态 ToolSpec
5. SSH：经 `build_ssh_command` 远端 spawn + stdio 转发；构造命令单测（不打真网）；失败 skip 不回退本机
6. 单测：有效服务器解析、命名、失败隔离、catalog 合并；Codex args 回归
6. 活动日志 key（如有）中英对照
7. 更新 `.trellis/spec/backend/ai-engines.md` MCP 段落
8. 门禁：clippy -D warnings、相关 cargo test、npm run format:check、test:ci、build

风险文件：`codex/mcp.rs`、`native/tools/*`、`native/agent/loop.rs`、`native/session.rs`、`app/remote.rs`（只复用不平行造 SSH）、`app/database.rs`、`McpSettingsTab.tsx`、`TaskMcpBindingSection.tsx`、`aiCapabilities.ts`

回滚点：native 启动加开关跳过 MCP 注入；矩阵字段可保留 false。
