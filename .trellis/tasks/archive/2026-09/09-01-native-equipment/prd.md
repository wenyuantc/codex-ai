# PRD · native 装备补齐（Skills / Hooks / ApplyPatch / Browser MCP）

优先级 P0。来源：`TASK.md`「下一波 · 2026-08-24」native 装备缺口最后一条。用户于 2026-09-01 批准计划：前三项内置实现，Browser 走 Playwright MCP 预设。

上一波已交付交互式权限、子 Agent、自定义子智能体、plan 模式、续聊/权限/资源收口。本任务关闭同一条目下剩余装备。

## Goal

内置 Agent 具备与自研编程 Agent 承诺匹配的装备：

1. 从工作区与全局目录发现 Skills，并按需加载全文。
2. 用户可配置工具执行前后的 Hooks（可阻断）。
3. 一次调用应用多文件补丁（ApplyPatch）。
4. MCP 设置页能一键添加 Playwright 浏览器自动化预设，并诚实声明能力边界。

全程兼容本地与 SSH 工作区；plan 只读与高风险确认语义保持不变。

## Requirements

### ApplyPatch

1. 新工具 `ApplyPatch`，参数 `patch`（Codex 信封：`*** Begin Patch` / `*** Add File:` / `*** Update File:` + 可选 `*** Move to:` / `*** Delete File:` / `@@` hunk）。
2. 上下文精确匹配失败时按去尾空白宽松匹配；再失败报逐 hunk 中文错误，不部分提交。
3. 本地 fs 与 SSH `read→apply→write` 两路；成功后把涉及路径写入 `read_files`。不要求事先 Read。
4. 权限：含 Delete File → `Delete`，否则 → `Overwrite`。摘要含新增/修改/删除计数。
5. 非只读；plan 模式自动排除。加入自定义子智能体可选工具白名单。

### Skills

1. 发现：工作区 `.agents/skills/*/SKILL.md`、`.claude/skills/*/SKILL.md`（本地 fs；SSH find+cat），以及全局 `$APPCONFIG/native-skills/*/SKILL.md`。
2. 手写解析 YAML frontmatter 的 `name`/`description`（不引入 serde_yaml）。最多 50 条，描述截断。
3. 系统提示词注入「# 可用技能」块（名称 + 描述 + 来源），指示用 `Skill` 工具加载全文。
4. 工具 `Skill`（只读，plan 可用）：按名返回 SKILL.md 全文 + 技能目录文件清单。SSH 会话加载全局技能时附带「附属文件不在远端」提示。
5. 设置页技能卡片：全局目录路径、已发现列表、打开目录。命令 `list_native_global_skills`；打开目录用 opener。

### Hooks

1. `native-settings.json` 增 `hooks[]`：id、event（`pre_tool_use`/`post_tool_use`）、matcher（工具名逗号分隔或 `*`，不用 regex）、command、timeout_secs、enabled。带归一化与默认值。
2. Pre 在权限确认前运行；退出码 2 阻断，stderr 作为原因回给模型。Post 在成功后运行，失败在工具结果附警告行。
3. payload 经环境变量 `NATIVE_HOOK_PAYLOAD`（JSON：event/tool_name/arguments/workspace）。本地 bash 与 SSH bash 均支持。
4. 设置页钩子卡片：增删改、事件选择、启用开关。活动日志复用 `native_settings_updated`。
5. 不做工作区级 hooks 文件、不做 SessionStart/End。

### Browser

1. MCP 设置页「预设」快捷添加 Playwright：`npx @playwright/mcp@latest`。
2. 文案声明：浏览器自动化经 Playwright MCP 提供，Codex 与内置 Agent 可真实执行；本地需 Node，SSH 远端需 node + 浏览器依赖。
3. 不内置 CDP。

### Cross-cutting

- zh-CN + en。
- identity.md 增加 Skill / ApplyPatch 一句话指引。
- native README、CLAUDE.md 命令计数、TASK.md 账本更新。

## Acceptance Criteria

- [x] 模型可调用 `ApplyPatch`；合法补丁在本地与 SSH 正确新增/修改/删除/移动文件；上下文失配返回中文错误且不改其它文件
- [x] ApplyPatch 覆盖/删除走现有确认框；plan 模式不能调用
- [x] 工作区与全局 SKILL.md 出现在系统提示词；`Skill` 可加载全文；plan 模式可用 Skill
- [x] 设置页能列出全局技能并打开目录
- [x] 匹配的 Pre hook 退出码 2 能阻断工具且不改文件；Post hook 失败只警告
- [x] 设置页能增删改 hooks，保存进 `native-settings.json`
- [x] MCP 页一键添加 Playwright 预设；能力声明可见
- [x] clippy / cargo test / format:check / test:ci / build 通过

## Out of Scope

- 工作区级 hooks 文件（避免仓库注入命令）
- SessionStart / SessionEnd 钩子
- 内置 CDP 浏览器
- 技能市场 / 安装 UI
- 给 Codex/Claude/Grok/OpenCode 增加同等内置 Skills/Hooks/ApplyPatch
