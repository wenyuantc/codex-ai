# PRD · native 看板计划运行

来源：`TASK.md` 五引擎缺口「plan 模式」。用户 2026-08-25 批准：看板右键入口 + 计划轮结束自动执行。不替代协调员。

## Goal

执行人为内置 Agent 的看板任务，可通过右键 **计划运行** 启动同一条 native 会话：先只读摸底并写出计划，计划轮成功结束后自动解锁写工具并实施。普通运行与协调员编排不变。

## Requirements

1. 看板 `TaskCard` 右键「计划运行」仅当执行人 `ai_provider === "native"` 时显示。未指定执行人或其它引擎不显示。
2. 禁用条件与「运行」一致（进行中 / 排队 / 依赖未完成 / 归档 / CTA 非 run）。
3. 「计划运行」走 `startTaskRunSession`（worktree、附件、图片跳过、队列），`planMode: true`。不打开协调员弹窗，不写 `plan_content`，不启动流水线。
4. 计划轮：`read_only`；工具仅 Read/Glob/Grep/Todo*/Web*；Write/Edit/Bash/MCP/Agent 不出现在工具列表，误调用中文错误且不改文件。
5. 计划轮成功结束：同一会话自动执行轮（内部续聊「按刚才方案实施」）；高风险仍走现有确认。失败 / 取消 / 停止不进入执行轮。
6. 执行轮结束后有 `task_id` 则退出，走现有 `handle_session_exit`。`await_followups` 规则不变。
7. 排队 payload 必须带上 `plan_mode`（serde 默认 false），drain 后仍是计划运行。
8. SSH 与本地同一套只读拒绝；模型 HTTP 仍在本机。
9. 活动：`native_plan_mode_entered` / `native_plan_mode_executed`（有 task_id）；zh-CN + en。
10. 绑定了任务子智能体时：计划轮不强制委派；执行轮内部续聊再 wrap 委派。

## Acceptance Criteria

- [ ] native 执行人右键可见「计划运行」；其它引擎不可见
- [ ] 计划轮不能改仓库；成功后同一会话开始改；覆盖仍确认
- [ ] 计划中停止则不写文件
- [ ] 普通运行 / 立即执行 / 协调员编排不变
- [ ] 排队后再启动仍是计划运行
- [ ] SSH 同样只读拒绝写工具
- [ ] zh-CN + en；clippy -D warnings；format:check；test:ci；native/run_queue cargo test；build 通过

## Out of Scope

设置默认开关、输入条「执行计划」、计划弹窗、写 `tasks.plan_content`、其它四引擎、explore 子 Agent、Skills/Hooks/Browser。
