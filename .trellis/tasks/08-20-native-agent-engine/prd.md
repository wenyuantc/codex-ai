# PRD · 第五引擎接入

> 父任务：`08-20-native-agent`。依赖渠道、模型客户端、agent 循环。

## Goal

把进程内 Agent 接进现有任务/会话系统，使 `ai_provider=native` 的员工能 start/stop/restart/resume/send_input，并被编排、审核、run queue 正确对待。

## Requirements

- R1 commands：`start_native_session` `stop_native_session` `restart_native_session` `resume_native_session` `send_native_input` `finish_native_input`。
- R2 落 `codex_sessions`（ai_provider=`native`）+ events；usage 走 `apply_codex_session_usage`。
- R3 `NativeAgentManager` 独立登记；计入 run_queue live count。
- R4 冲突检测包含 native；`else` 不得把 native 当 Codex。
- R5 `pipeline.rs` / `review.rs` / 会话状态 增加 native 分支。
- R6 能力矩阵加入 native：start/stop/restart/resume/send_input 全 true。
- R7 任务会话 turn 结束退出；自由会话可 send_input。
- R8 启动前校验：员工 native、渠道存在且启用、有 key、工作目录合法（含 SSH）。
- R9 活动日志沿用 `task_execution_started` 等现有 key，必要时加 `native_session_*` 并配中文。

## Out of Scope

- 员工创建 UI 的渠道下拉（子任务 ui）
- Skills / 权限弹窗

## Acceptance Criteria

- [x] native 员工启动任务不产生 codex/claude/grok/opencode 子进程
- [x] stop 取消循环；send_input 在自由会话进入下一轮
- [x] 并发满时任务进入 run queue
- [x] 编排/审核选 native 员工走 start_native_*
- [x] 能力矩阵测试期望 5 个 provider
- [x] clippy / cargo test 通过
