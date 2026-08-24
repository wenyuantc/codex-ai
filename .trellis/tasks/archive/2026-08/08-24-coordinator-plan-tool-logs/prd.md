# 协调员计划日志补齐工具过程

## Goal

协调员执行计划弹窗在生成/重新生成计划时，终端日志实时显示与执行终端同类的工具与过程行（`[读取]` / `[工具]` / `[思考]` / `[待办]` / `[用量]`），而不只是开始与结果。结束后仍落库 Markdown 计划 + 结构化工作包。

用户价值：规划阶段可观察模型是否在读仓库、调用了哪些只读工具，而不是对着空白等待。

## Confirmed Facts

- 弹窗日志来自 `TaskCard` / `TaskDetailDialog` 的 `generateCoordinatorPlan`，invoke 前后打点，无流。
- `ai_generate_coordinator_task_plan` → `run_ai_command`：CLI `wait_with_output`；SDK 只解析最终 `{ok,text}`。
- 内置 Agent `run_native_one_shot`：HTTP `chat()`，`tools: &[]`。
- 弹窗用 `getAiLogColor`，不上色 `[工具]`/`[读取]`。
- `session_kind` 只有 `execution|review`，不可扩第三值；规划不得走 `start_*` / run-queue / `handle_session_exit`。
- 决策：规划阶段 **只读工具**（不写仓库）。CLI 尽力只读；native 硬保证。

## Requirements

1. 生成计划期间，协调员终端日志实时出现过程行，配色对齐执行终端。
2. 内置 Agent 协调员：`AgentRunner` + 工作目录 + SSH；允许 Read/Glob/Grep/TodoRead/TodoWrite/WebFetch/WebSearch；禁止 Write/Edit/Bash/MCP。不注册 NativeAgentManager。
3. CLI 引擎：coordinator 路径对流式解析并 emit；无过程则不强行编造。
4. 独立事件 `ai-command-stdout`（`request_id`），不得污染 `codex-stdout`/`native-stdout`/`taskLogs`。
5. 命令仍阻塞返回最终计划；落库、活动日志、工作包替换保持现网。
6. 测试员验收等其它 one-shot 仍无工具、无过程流。
7. 本地 + SSH。更新 `coordinator_plan` 模板：可读仓库、禁止改文件。
8. 关掉弹窗可不持久化过程日志（与现状一致）。

## Acceptance Criteria

- [ ] AC1：生成计划时弹窗出现 `[读取]`/`[工具]`/`[思考]` 等过程行，而不只是开始/结果。
- [ ] AC2：内置 Agent 协调员只读工具可执行；Write/Edit/Bash 被拒且不改仓库。
- [ ] AC3：执行任务终端日志不被这次规划污染。
- [ ] AC4：计划 Markdown + 工作包仍落库；`task_plan_generated` 活动日志仍写。
- [ ] AC5：SSH 项目规划同样能出现过程行（native 走远端只读工具；CLI 按行读远程 stdout）。
- [ ] AC6：测试员验收行为不变。
- [ ] AC7：`clippy -D warnings` / `format:check` / `test:ci` / `build` 通过。

## Out of Scope

- 规划变成对话页可回放的完整会话
- 编排步骤会话日志
- 执行终端 TodoWrite 内容显示
- tester / commit / prompt 优化 one-shot
