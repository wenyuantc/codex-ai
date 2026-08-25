# PRD · 自定义子智能体配置

优先级 P1。来源：用户 2026-08-25 需求「设置增加 tab 子智能体配置」。上一波 `08-24-native-subagent` 已交付会话内 `Agent`（仅 `general` / `explore`）。本任务在其上增加用户可配置类型。

## Goal

内置 Agent 用户能在设置里配置自定义子智能体（名称、模型继承或渠道模型、描述、工具、系统提示词、是否注入 AGENTS.md）。父会话通过现有 `Agent` 工具的 `subagent_type=<名称>` 委派。内置 `general` / `explore` 保留。

一句话：**子 Agent 从两种写死类型，变成可配置的类型目录。**

## Evidence

- `parse_subagent_args` 只认 `general|explore`，未知类型中文错误。
- `Agent` schema `enum: ["general", "explore"]`。
- `identity.md` 写死「子 Agent 只有两种」。
- 设置只有并发上限与策略，没有类型 CRUD。
- 子循环复制父 system + 父 ModelClient，无法换渠道或换提示词。

## Requirements

1. 设置页新 Tab「子智能体」，应用全局一份配置（所有内置 Agent 员工共用）。
2. 字段：名称、描述、渠道模型或继承默认、工具模式（全部权限 / 自定义勾选 9 工具）、系统提示词、是否注入 AGENTS.md（默认开）。
3. 名称即 `subagent_type`，可中文；禁止与 `general` / `explore` / `general-purpose` 冲突。
4. 「继承默认」= 当前会话员工绑定的内置 Agent 渠道+模型。「指定渠道模型」= 已启用 `ai_channels` 中的渠道 + 该渠道模型 id。
5. 「默认所有权限」= 与 `general` 相同（全部内置工具不含 Agent + 父 MCP）。自定义 = 仅勾选工具，不含 MCP / Agent；勾 TodoWrite 时自动带 TodoRead。
6. 系统提示词替换父 identity/员工设定；始终注入环境与 Git；AGENTS.md 由开关控制。
7. 父 Agent 工具描述动态列出内置 + 自定义类型。调用方式：`Agent({ subagent_type, description, prompt })`。深度仍为 1。
8. 配置存 `native-subagents.json`，不改 SQLite schema。CRUD 活动日志 + zh-CN/en。
9. SSH：子循环仍走父工作区；换渠道只换本机 HTTP。
10. 协调员只读 one-shot 仍无 Agent。

## Acceptance Criteria

- [ ] 设置可新建/编辑/删除自定义子智能体；校验失败中文错误
- [ ] 父模型能用 `subagent_type=<名称>` 委派；未知名称中文错误且不启动
- [ ] 自定义 all 可写且共享 MCP；custom 只暴露勾选工具
- [ ] inject_agents_md 关则子 system 无 AGENTS.md 块
- [ ] inherit 用父模型；channel 模式用指定渠道模型（停用则这次委派失败）
- [ ] 内置 general/explore 行为不变；子循环不能再委派
- [ ] 仪表盘活动：创建/更新/删除子智能体有中文标签
- [ ] zh-CN + en；clippy / format:check / test:ci / build 通过

## Out of Scope

- 四引擎 CLI 子 Agent、项目目录 markdown agents、Persona、worktree isolation、后台运行
- 修改内置 general/explore 工具集
- 每员工一份目录
