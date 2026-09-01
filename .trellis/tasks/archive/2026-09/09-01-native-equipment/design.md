# Design · native 装备补齐

## Boundaries

- 只改 `native/` 工具链、设置 JSON、MCP 设置 UI、identity/README/账本。
- 不改 SQLite schema。hooks 存在 `native-settings.json`。
- Browser 不新增工具，只新增 MCP 预设。

## Data flow

```
compose_system ← skills.rs 发现工作区+全局 SKILL.md
combined_tools ← catalog Skill + ApplyPatch
execute_tool
  → PreToolUse hooks (exit 2 阻断)
  → confirm_if_high_risk
  → local.rs / ssh.rs / patch.rs / Skill
  → PostToolUse hooks (失败附警告)
```

## Contracts

### ApplyPatch

- Parser: `native/tools/patch.rs` 纯函数 `parse_patch` / `apply_actions`.
- 信封：Begin/End Patch；Add/Update(+Move to)/Delete；hunk `@@` + ` `/`-`/`+` 行。
- 应用：先全部解析，再逐文件应用；任一 hunk 失败则该工具失败。本地已写入的文件在单次调用内按内存工作副本提交（先算完再写盘）以避免半成品。
- SSH：逐文件 read（缺失则空）、内存应用、write/delete。
- 权限摘要：`应用补丁：N 个文件（新增 a / 修改 b / 删除 c）`。

### Skills

- `native/skills.rs`：`NativeSkill { name, description, source, dir, skill_md }`。
- 重名：工作区 `.agents` > `.claude` > 全局。
- `Skill` 工具参数 `name`。SSH 全局技能加载时追加提示。
- `list_native_global_skills` 返回 `{ dir, skills }`；`open_native_skills_dir` 用 opener（目录不存在则创建）。

### Hooks

- `NativeHook` 进 `NativeSettings.hooks`。
- matcher：`*` 或逗号分隔工具名，大小写不敏感。
- timeout 默认 30，范围 1–120；0 归一成 30。
- 最多 32 条。空 command 或未知 event 丢弃。
- 本地 `bash_with_status` 注入 env；SSH 用 `NATIVE_HOOK_PAYLOAD=... bash -lc`。
- Agent 不走 `execute_tool`，因此不触发 hooks。

### Browser

- 前端一键插入 `{ name: playwright, command: npx, args: ["@playwright/mcp@latest"], enabled: false }`，已存在同名则提示。

## Compatibility

- 旧 `native-settings.json` 无 `hooks` → 空数组。
- `NativePromptParts` 新字段 `skills` 默认空。
- `ToolCtx` 增 `skills` / `hooks`；子 Agent clone 父的这两项。
- `CUSTOM_TOOL_NAMES` 增加 `ApplyPatch`、`Skill`；`custom_tools_are_read_only` 把 `ApplyPatch` 当写工具。

## Rollback

删除新文件与 catalog 条目即可回退；设置 JSON 多出的 `hooks` 字段对旧版本 serde 忽略需 `#[serde(default)]`（旧代码读新文件时若无该字段则失败——因此新字段必须 default）。旧前端忽略未知命令。
