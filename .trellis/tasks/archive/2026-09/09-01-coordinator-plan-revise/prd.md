# 协调员计划按意见修改

## Goal

计划生成后，用户能在「协调员执行计划」弹窗里用一句话让协调员基于当前计划修订，同时更新 Markdown 与工作包。不再只能「重新生成」推倒重来。

## Confirmed Facts

- 生成是 one-shot：`ai_generate_coordinator_task_plan` → `run_ai_command` / `run_native_ai_command`，插入 `session_kind=coordinator` 后立刻 `exited`。
- 内置 Agent 规划走 `run_native_read_only_one_shot`，当前不写 `native_session_transcripts`。
- 「重新生成」不含当前计划；工作包整表 DELETE 再 INSERT。
- 「保存计划」只写 `tasks.plan_content`，不同步工作包。
- 工作包 UI 只能换执行人；后端 `update_task_pipeline_step` 已能改 title/goal/criteria。
- 规划不得走 `start_*` / run-queue / `handle_session_exit`（08-24 设计约束仍有效）。
- Grok `send_input=false`。

## Requirements

1. 已有计划时，弹窗显示修改意见输入条；发送后走同一只读规划路径，prompt 含当前 Markdown、当前工作包、用户意见。
2. 输出仍是 `{ markdown, steps }`，覆盖 `plan_content` 并替换工作包。
3. 未保存的 Monaco 草稿作为当前 Markdown；工作包读后端表。
4. native：首次规划保存 transcript；修订时能恢复则恢复，否则注入当前计划。CLI/Grok 注入兜底，日志诚实说明。
5. 修订仍新建 coordinator session 行，不把旧行改回 running。
6. 执行/编排中锁定，与现网 `actionsLocked` 一致。
7. 「重新生成」保留，确认后才从头规划。
8. 活动 `task_plan_revised`（中文「修订协调员计划」）+ 仍写 `task_pipeline_plan_saved`。
9. 创建并运行后台生成不传 revision，行为不变。本地 + SSH。

## Acceptance Criteria

- [ ] AC1：已有计划时输入意见点「修改计划」，过程日志可见，Markdown 与工作包都更新。（代码已接；待桌面冒烟）
- [x] AC2：无计划 / 空意见 / 执行中不能修订。（弹窗 `canRevise` + `actionsLocked`）
- [ ] AC3：「重新生成」确认后仍整份新规划。（已加 confirm；待桌面冒烟）
- [x] AC4：创建并运行、无协调员路径不变。（不传 revision）
- [ ] AC5：native 有 transcript 时修订日志出现续聊恢复；无则「基于当前计划修改」。
- [ ] AC6：SSH 可修订；只读、不改仓库。
- [x] AC7：仪表盘 `task_plan_revised` 显示中文。
- [x] AC8：`npm run test:ci`、src format、`build`、`clippy -D warnings` 通过。全库 `format:check` 仍会被既有 `.opencode/` 拦住，与本任务无关。

## Out of Scope

- 活协调会话 / `send_input` 改计划
- 编排进行中局部改步、步骤状态合并
- 计划版本历史
- 工作包增删改排序编辑器
- CLI coordinator one-shot 真 resume
- 改创建并运行语义
