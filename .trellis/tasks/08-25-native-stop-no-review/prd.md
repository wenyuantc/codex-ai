# 内置 Agent 停止后不再自动审核

## Goal

看板任务在内置 Agent 运行中点击停止后，会话彻底结束，不再自动进入审核并继续跑。

## Background

自动质控是否继续只看 `codex_session_events.event_type = stopping_requested`。Codex/Claude/Grok/OpenCode 在 kill 前会写该事件；native 的 `stop_native_process` 只 cancel + join。取消还被记成 `exited` / `exit_code=0`，`handle_session_exit` 会当成执行成功并 `retry_pending_review`。

## Requirements

1. 用户停止 native 会话（看板/详情「停止」、`stop_native` / `stop_native_session`、`finish_native_input`）必须在 cancel 前写入 `stopping_requested`。
2. 有该事件时：不启动审核、测试员、下一编排步骤；自动质控/编排进入 `manual_control`；任务 status 保持现列。
3. `stop_native_for_automation_restart` 必须写 `automation_restart_requested`，不能写成用户停止。
4. 模型正常结束且无停止事件时，审核闭环不变。
5. 本地与 SSH 工作区同一条 in-process 停止路径。

## Acceptance Criteria

- [ ] native 运行中点停止：进程停、不进「审核中」、不拉起审核会话
- [ ] 开启 `review_fix_loop_v1` 时停止后相位为 `manual_control`
- [ ] 自动化重启仍能区分 `automation_restart_requested`
- [ ] 成功退出（无停止事件）仍可自动审核
- [ ] clippy -D warnings、format:check、相关 cargo test、test:ci 通过

## Out of Scope

- 改自动质控状态机本身
- 其它四引擎停止路径
- plan-mode 未提交改动
