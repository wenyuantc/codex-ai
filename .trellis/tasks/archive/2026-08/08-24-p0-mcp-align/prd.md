# PRD · P0-1 MCP 对齐：native 注入 + UI 能力声明

父任务：`08-24-engine-ecosystem` · 优先级 P0

依赖：无。后续 P0-2a 把 MCP 工具纳入高风险分类，应在本任务之后做。

## Goal

用户配了 MCP，不能再「配了=没配」。Codex 继续真用；native 在承诺范围内真用；Claude / Grok / OpenCode **明确不用**，入口禁选或降级提示，禁止假生效。

## 证据

- 配置入口：`src/components/settings/McpSettingsTab.tsx`、`src/components/tasks/detail/TaskMcpBindingSection.tsx`（挂在 `TaskOverviewPanel.tsx:443`，不看员工引擎）
- 持久化：`src-tauri/src/codex/mcp.rs`（`mcp-servers.json` + `get_task_mcp_binding` / `set_task_mcp_binding`）
- 唯一消费：`codex/process/command_builders.rs:61` `append_mcp_config_args`；Claude/Grok/OpenCode/native 零命中
- 文案撒谎：`src/locales/zh-CN/settings.json:625`「合并到 Codex、Claude 等引擎」；`src/locales/zh-CN/tasks.json:282`「控制本任务会话启用的 MCP」
- 能力矩阵无 MCP 字段：`src-tauri/src/db/models.rs:1193-1202`、`src/lib/aiCapabilities.ts` `EngineCapabilityKey` 只有 start/stop/restart/send_input/resume
- native 工具固定 10 个：`src-tauri/src/native/tools/catalog.rs`；循环 `tool_specs()`（`native/agent/loop.rs:91`）

## Requirements

1. **能力真源**：`AiProviderCapabilities` 增加 `mcp`（或等价字段）。Codex=`true`，native=`true`（本任务交付后），Claude/Grok/OpenCode=`false`。notes 写清边界。前端 `EngineCapabilityKey` / Settings 徽章同步。
2. **设置页 MCP tab**：标题/说明改为「被 Codex 会话直接消费；内置 Agent 按任务绑定注入工具；其他引擎本波不执行」。禁止继续写「合并到 Claude 等」。
3. **任务 MCP 绑定**：按指派员工（及审核/编排实际运行引擎）显示生效范围。非 Codex/native 时禁用保存为「将生效」或改为只读警告「当前员工引擎不会使用这些服务器」。绑定数据仍可保存（给 Codex/native 员工以后用），但不得声称当前会话会带上。
4. **Codex 回归**：本地 + SSH 仍通过现有 `-c mcp_servers.*` 注入；不改绑定存储格式。
5. **native 注入（本地项目）**：启动 native 会话时读取与 Codex 相同的有效 MCP 列表（全局 inherit / 任务 override / 显式空集），把每个 enabled 服务器的工具动态并入 `tool_specs()`，模型可调用；工具结果回灌循环。stdio MCP 在本机拉起。
6. **native SSH（已锁定：远程也要做）**：在 **远程主机** 上用同一套 `command/args/env` spawn MCP stdio；JSON-RPC 经已有 `build_ssh_command` 通道转发到本机 native 循环。会话结束必须关掉远端 MCP 进程。禁止在本机 spawn 再假装是远程工具（否则 Write/Bash 类 MCP 会打到开发机）。远端缺二进制/handshake 失败：该服务器跳过并 `[WARN]`，不得让整段 native 会话失败，也不得回退到本机 MCP。
7. **失败可见**：某个 MCP 服务器启动失败不得静默当成功；会话日志写清跳过/失败原因，其余工具仍可用。
8. **活动日志**：若新增 key（如 native MCP 注入/失败），必须进 `activity.json` 中文 + 英文。
9. **测试**：解析/过滤有效服务器的单测；native 至少用 mock MCP 或 fixture 证明工具出现在 catalog 且 dispatch 可达。SSH 侧测命令构造（经 `build_ssh_command`，不打真网）。Codex `append_mcp_config_args` 回归。

## Acceptance Criteria

- [x] Settings 引擎能力对比能看出谁支持 MCP；Claude/Grok/OpenCode 为不支持
- [x] 设置 MCP tab 与任务绑定不再对 Claude/Grok/OpenCode 说「本会话会启用」
- [x] Codex 员工运行任务：行为与现在一致（inherit/override/空集）
- [x] native **本地**员工：启用的 MCP 工具出现在该会话工具目录并可被调用（至少一条 happy path）
- [x] native **SSH**员工：MCP 在远端拉起；工具调用不打到开发机工作区。远端失败可见，不回退本机 MCP
- [x] MCP 服务器挂掉时会话不假装「已连接」
- [x] zh-CN + en；相关 clippy / cargo test / format:check / test:ci / build 通过

## Out of Scope

- 实现 Claude/Grok/OpenCode 的 MCP 执行
- MCP 资源/prompt 模板全套、OAuth、可视化工具市场、远程安装 npx/uvx 引导向导
- 子 Agent / Skills
- 为 MCP 单独做一套 SSH 参数构造（必须走 `build_ssh_command`）
