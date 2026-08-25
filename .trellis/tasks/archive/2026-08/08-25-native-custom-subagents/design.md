# Design · 自定义子智能体

## 边界

只改 **native 进程内循环 + 设置 UI**。不改四引擎 CLI，不改 SQLite schema，不扫 `.grok/agents/`。

配置文件：`app_config_dir/native-subagents.json`（与 `native-settings.json` 并列）。

## 数据流

```
设置 Tab
  → native.ts create/update/delete_native_subagent
  → native/subagents.rs 校验写入 JSON + activity_log

父循环 combined_tools
  → 重读 JSON，Agent 描述列出 general/explore + 自定义

Agent({ subagent_type, prompt, description })
  → parse_subagent_args(known names)
  → spawn_child_runner
        general / custom-all: 父 MCP，可写
        explore: READ_ONLY，无 MCP
        custom-custom: allowed_tools，无 MCP；无 Write/Edit/Bash 则 read_only
        inherit: 父 ModelClient + model_turn
        channel: 按 channel_id 建 ModelClient
        system: 自定义 prompt + env/git + 可选 AGENTS.md + 子 Agent 附录
```

## 合约

### JSON 记录

见计划表。`id` UUID 做 CRUD 主键；`name` 做 `subagent_type`。最多 32 条。

### 命令

`list_native_subagents` / `create_native_subagent` / `update_native_subagent` / `delete_native_subagent`。

channel 模式：创建/更新时校验渠道存在且启用、模型在该渠道列表中。spawn 时再校验一次（渠道可能被删）。

### 子 system

不复制父 identity。块顺序：自定义 system_prompt → 环境/Git → 可选项目 AGENTS.md → 子 Agent 附录（类型名、禁止再委派、中文报告）。

### 回滚

删除 `native/subagents.rs` 与设置 Tab，恢复 `subagent_type` enum 与 identity 文案。
