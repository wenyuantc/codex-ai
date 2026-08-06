# MCP 任务级深度绑定

## Goal

在已有「设置 → MCP 服务器清单」之上，支持 **按任务选择启用的 MCP（三态）**，并在 **Codex 会话启动时** 注入有效工具集，使应用内 MCP 从「配置文件 + 手工导出」变为「执行时按任务生效」。

## Background

- 全局清单：`src-tauri/src/codex/mcp.rs` + `McpSettingsTab`；落盘 `app_config_dir/mcp-servers.json`（不在 SQL 备份内）。
- `McpServerConfig.enabled` 表示全局默认启用；`export_mcp_servers_snippet` 只导出 enabled。
- 四引擎启动与 `task_automation` **当前不读** 应用 MCP 配置；用户 `~/.codex/config.toml` 的 MCP 与应用清单是两套来源。
- 任务/员工表无绑定字段；活动已有 `mcp_servers_updated` / `mcp_servers_reset` 中文映射。
- Codex CLI 支持 `-c key=value`、`--ignore-user-config`、`-p profile`，配置键为 `[mcp_servers.<name>]`（已在本机 Codex 验证）。

## Decisions

| ID | 决策 | 选择 |
|----|------|------|
| D1 | 绑定层级 | **仅任务级**；员工级 out of scope |
| D2 | 任务绑定语义 | **三态**：继承全局 / 显式空集 / 指定 id 列表 |
| D3 | 引擎范围 | **Codex 本地必达**；Claude/Grok/OpenCode 复用解析器，注入能接则接，否则中文提示不阻塞验收 |
| D4 | 有效集真源 | 应用 `mcp-servers.json` + 任务绑定；会话注入以该解析结果为准（不依赖用户手工合并 snippet） |

## Requirements

### R1 绑定模型（三态）

| 状态 | 存储 | 解析结果 |
|------|------|----------|
| 继承全局（默认） | `mcp_server_ids IS NULL` | 全局 `enabled=true` 的 server 定义 |
| 显式空集 | `mcp_server_ids = '[]'` | 无 MCP |
| 指定集合 | `mcp_server_ids = '["id1",…]'` | 全局定义中匹配 id 且仍存在的 server（忽略未知 id，可告警） |

- 全局 server 定义与 `enabled` 仍在设置页维护。
- 绑定只存 server **id** 引用，不复制 command/env。

### R2 会话注入

- 有 `task_id` 的 Codex 会话启动前：`resolve_effective_mcp_servers(task_id)` → 注入进程配置。
- 本地：CLI 路径用 Codex `-c` / profile / 必要的 config 隔离，达到 **以解析结果替换本会话 MCP 集**（含空集）。
- SDK 路径：若 SDK API 无法传 MCP，则回退可注入的 CLI，或终端中文说明本次未注入（不得静默当成已注入）。
- SSH：能把等效配置送到远程命令则同步；否则终端中文降级提示。
- 无 `task_id` 的会话：使用全局 enabled 集（与「继承」一致），保持行为可预期。

### R3 UI

- 任务详情：三态控件（「使用全局默认」+ 关闭后的多选列表，允许全不选）。
- 展示当前将生效的 server 名称摘要（只读预览可选但推荐）。
- 设置页仍管定义；不在员工页绑定。

### R4 安全与可观测

- `set` 绑定写 activity：`task_mcp_binding_updated`（或等价 key），`details` 仅含模式 + id/name 列表，**禁止 env 密钥**。
- `getActivityActionLabel` 增加中文。

### R5 数据与兼容

- Migration 为 `tasks` 增加可空绑定列（或等价存储）；默认 `NULL` = 继承，旧任务零行为变化直至用户改绑定。
- 前端类型 / `UpdateTask` 或专用 command 与 Rust 模型对齐；写路径走 Tauri command。

## Acceptance Criteria

- [ ] AC1：任务设为指定 MCP 子集后，下一次 **Codex 本地** 会话启动使用该子集（可通过配置探测、会话日志或 `codex mcp` 等效观察点验证）
- [ ] AC2：任务绑定为 `NULL`（继承）时，有效集 = 全局 `enabled=true`
- [ ] AC3：任务绑定为 `[]` 时，该会话不启用应用侧 MCP（显式空集）
- [ ] AC4：绑定变更产生中文活动日志，且 details 无密钥明文
- [ ] AC5：SSH Codex：注入成功或明确中文降级提示（不静默）
- [ ] AC6：`npm run build` 与 `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 通过

## Out of Scope

- MCP 协议服务端实现；商店式安装
- 员工级绑定
- 将 `mcp-servers.json` 迁入 SQLite
- 强制四引擎注入对等（非 Codex 为 best-effort）
- 改写用户 `~/.codex/config.toml` 持久内容（仅会话级覆盖）

## Open Questions

（无阻塞项）
