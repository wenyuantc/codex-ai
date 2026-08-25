# Design · native 看板计划运行

## 循环

`run_native_loop` 在 `plan_mode` 时 `set_read_only(true)`。第一轮 `run_with_client` 成功且未取消 → `set_read_only(false)`，注入实施续聊，再跑一轮。然后按原 `await_followups` 退出。不新增 followup 变体或确认命令。

`combined_tools`：`read_only` 时只保留 `is_read_only_native_tool`（含过滤 MCP extra_tools）。

## 启动

`start_native_session` / `start_native_with_manager` 增加 `plan_mode: Option<bool>` / `bool`，默认 false。编排 / 审核 / 修复显式 false。

`QueuedTaskRun.plan_mode`：`#[serde(default)]`。native 入队与 drain 透传。

绑定子智能体：计划轮不 wrap、不写 required_subagent 系统块；切执行时再设 `required_subagent_type` 并 wrap 续聊。

## 前端

`TaskCard` → `startTaskRunSession({ planMode: true })` → `startByProvider` → `startNative`。仅这一条入口为 true。

## 提示

环境块 `Permission mode: plan`。identity 说明 plan 只读、系统会自动实施，不要等确认。
