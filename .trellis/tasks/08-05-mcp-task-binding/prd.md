# MCP 任务级深度绑定

## Goal

在已有「设置 → MCP 服务器清单」之上，支持 **按员工/任务选择启用的 MCP**，并在启动会话时注入或导出到引擎可消费的配置，使 MCP 从「全局配置文件」变成「执行时可用工具」。

## Background

- `McpSettingsTab` + `get/update/reset/export_mcp_servers` 已存在。
- 缺任务/员工级绑定与会话启动联动。

## Requirements

### R1 绑定模型

- MCP server 可标记全局默认启用。
- 员工和/或任务可覆盖启用集合（具体层级在 design 中定：员工优先 vs 任务优先）。

### R2 会话注入

- 启动 Codex/Claude 等会话时，按绑定生成有效 MCP 集（导出 snippet 或运行时配置路径）。
- SSH 模式：远程配置同步策略明确（能同步则同步，否则提示）。

### R3 UI

- 任务详情 / 员工编辑可多选 MCP。
- 设置页仍管理 server 定义。

### R4 安全

- 不把密钥明文写进 activity；密钥仍走现有 secret 机制（若 MCP 含 env）。

## Acceptance Criteria

- [ ] 可为任务选择 MCP 子集并在下次运行生效（至少 Codex 路径验证）
- [ ] 无绑定时回退全局默认
- [ ] 变更有中文活动日志
- [ ] build + clippy 通过

## Out of Scope

- 实现 MCP 协议服务端本身
- 市场商店式 MCP 浏览安装
