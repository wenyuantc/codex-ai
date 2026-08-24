# Design · P0-1 MCP 对齐

## 边界

- **配置仍归** `codex/mcp.rs`（文件 + 任务列）。不要为 native 再做一份 `mcp-servers.json`。
- **执行分叉**：Codex 继续 CLI `-c mcp_servers.*`；native 在进程内把 MCP 工具并进 catalog；其余引擎零执行。
- **能力矩阵**是 UI 真源。新增字段必须前后端同步，Settings 徽章与 `can()` 共用。

## 数据流

```
mcp-servers.json + tasks.mcp_server_ids
        │
        ▼
codex::mcp::resolve_effective_mcp_servers(_lenient)
        │
        ├── Codex start_*  → append_mcp_config_args（现状）
        ├── native start_* →（本地）spawn stdio MCP → list tools → 合并 ToolSpec
        │                   （SSH）远端 spawn 同一 command/args/env，stdio JSON-RPC
        │                         经 build_ssh_command 转发；失败 skip，不回退本机
        └── claude/grok/opencode start_* → 不读、不传、UI 标明无效
```

## 合约

### 能力矩阵

`AiProviderCapabilities` 增加：

- `mcp: bool`
- notes 补充：Codex=会话启动注入（本地+SSH CLI）；native=进程内工具注入（本地 spawn / SSH 远端 spawn）；其余=不执行

前端：

- `src/lib/backend.ts` 接口
- `EngineCapabilityKey` 加 `mcp`
- `EngineCapabilityBadges` 多一枚徽章
- i18n `capability.mcp` + 各引擎 notes

### native 注入

建议新模块 `src-tauri/src/native/tools/mcp.rs`（或 `native/mcp/`），不要把 stdio 客户端塞进 `codex/mcp.rs`。

会话启动（`native/session.rs` / `AgentRunner`）：

1. 解析有效服务器（复用 `resolve_effective_mcp_servers_lenient`）
2. 对每个 enabled server：
   - 本地：本机 spawn `command args`，env 注入，stdio JSON-RPC
   - SSH：`build_ssh_command(..., allocate_tty=false 或长会话所需值)` 在远端执行同一 `command args`，stdin/stdout 对接 MCP 帧；**不要**在本机 spawn
3. initialize + tools/list；把 `mcp_<serverId>_<tool>` 合成 `ToolSpec` 追加到 catalog
4. dispatch：未知内置名则路由到对应 MCP `tools/call`，content 转字符串回灌
5. 会话结束 / 取消：关掉本机 child 或远端进程（SSH 侧需显式杀，Drop 本地 ssh 进程不够可靠时补 `pkill`/记录 pid）

SSH 硬约束：
- 走 `app/remote.rs::build_ssh_command`，禁止平行 SSH 参数构造
- 长 MCP stdio 不得占用会杀死会话的 ControlMaster 组合（对齐 ssh-remote.md：长连接 `ControlMaster=no`）
- 远端 PATH 找不到 command：该 server skip + 中文 WARN（提示远程未安装），不回退本机

### UI

- `McpSettingsTab` 说明改能力矩阵口径
- `TaskMcpBindingSection` 读取任务指派员工的 `ai_provider`（及必要时审核员）。`mcp=false` 时展示警告、禁用「当前将生效」措辞
- 绑定仍允许保存，避免 Codex/native 员工接手后丢失配置

### 失败

单服务器 spawn/handshake 失败：该服务器工具不出现在 catalog；终端 `[WARN]`；不让整个 native 会话失败。

## 权衡

| 方案 | 收益 | 成本 |
|---|---|---|
| A. 只改 UI「仅 Codex」 | 最快诚实 | native 仍无装备，违背 TASK 建议 B |
| B. native 注入 + UI 声明（本任务） | 真话 + 自研 Agent 最低装备 | 需 stdio MCP 客户端 |
| C. 四外部引擎都接 MCP | 表面整齐 | Claude/Grok/OpenCode 启动路径差很大，本波不做 |

## 兼容 / 回滚

- 不改 `mcp_server_ids` 列语义
- 矩阵新字段旧前端会忽略；同版本前后端一起发
- 回滚：native 启动跳过注入模块即可，Codex 不受影响

## SSH

用户已锁定「远程也要做」。native 循环仍在本机，但 MCP **进程**必须在远端。本机只做 JSON-RPC 代理。若误在本机 spawn，filesystem/git MCP 会改开发机，视为验收失败。

Codex SSH 现状是把 `mcp_servers.*` 配进远程 `codex exec`，由远端 Codex 自己拉 MCP——native 没有这层，必须自建远端 stdio 代理。
