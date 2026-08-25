# PRD · native 会话内子 Agent

优先级 P0。来源：`TASK.md`「下一波 · 2026-08-24」native 装备缺口「子 Agent」。用户于 2026-08-24 批准计划后立项。

上一波 `08-24-engine-ecosystem` 明确把子 Agent 排除在外；本任务单独立项，不挂已归档父任务。

## Goal

内置 Agent 能在**当前会话内**把独立子任务委派给子循环：子循环有自己的对话上下文，共用工作区（本地或 SSH），完成后把报告交回父模型。同一轮可并行多个子 Agent。用户在终端能看见子 Agent 进度；高风险工具仍走现有确认框。

一句话：**有手脚之后，要能分工，而且并行时不能把权限确认或 MCP 弄坏。**

## 证据

- 工具目录无委派：`src-tauri/src/native/tools/catalog.rs` 只有 Read/Write/Edit/Bash/Glob/Grep/Todo*/Web*；`dispatch.rs:62-75` 无 Agent 分支。
- 循环串行：`native/agent/loop.rs:232-242` 逐个 `execute_logged_tool`；`consume_assistant` 拿不到 `ModelClient`。
- 只读规划已有：`set_read_only` + `READ_ONLY_NATIVE_TOOL_NAMES`（`catalog.rs:5-20`）；协调员 `run_native_read_only_one_shot`（`session.rs:458-461`）必须继续不含委派工具。
- 权限单通道：`NativeLiveSession.pending_permission: Option<(id, oneshot)>`（`manager.rs:30`）。新请求 `take()` 后 Deny 旧请求（`session.rs:837-842`）。并行子 Agent 的第二个 Write 会掐掉第一个确认。
- MCP stdio 无锁：`mcp.rs:153-191` `call` 写同一 stdin。并行 `tools/call` 会交错 JSON-RPC。
- 可共享：`LocalWorkspace` Clone、`SshToolRuntime` Clone、`CancelFlag` Arc、`allow_all_high_risk` Arc。`McpSession` 持有 Child，不可 Clone。
- 终端色：`src/components/tasks/detail/taskDetailViewHelpers.ts:28-54` 无 `[子 Agent]`。
- 活动标签：`getActivityActionLabel` → `activity:actions.*`；尚无 subagent key。

## Requirements

1. **工具 `Agent`**：参数 `description`（短标题）、`prompt`（必填、自包含）、`subagent_type`（`general` | `explore`，默认 `general`）。同一 assistant 轮次里连续的 `Agent` 调用并行，上限 3。夹在中间的其它工具仍串行。
2. **`general`**：除 `Agent` 外与父相同的内置工具 + 共享 MCP。可写，高风险走确认。
3. **`explore`**：仅 `READ_ONLY_NATIVE_TOOL_NAMES`，`read_only=true`，不注入 MCP。Write/Edit/Bash 返回现成只读错误且不改文件。
4. **深度 1**：子循环 `combined_tools` 不含 `Agent`。模型若仍调用，按 unknown tool 失败，不嵌套。
5. **进程内嵌套**：新 `AgentRunner`，同一 `session_record_id`。不插入 `codex_sessions`，不占 run_queue。停止父会话（共享 `CancelFlag`）必须停子循环，未决权限全部 Deny。
6. **隔离**：子循环自有 `messages` / `todos` / `read_files`。复制父 system（身份/环境/Git/AGENTS/员工）再追加「你是子 Agent」；user = `prompt`。看不到父对话。
7. **权限**：共享 `allow_all_high_risk` 与同一 Dialog。待确认改为 FIFO 队列；新请求不得 Deny 旧请求；同时只展示一条。关闭/不允许/停止 = 该次工具失败，父循环继续。
8. **MCP**：`general` 共享父 MCP，调用必须 Mutex 串行化。`explore` 不注入。禁止为子循环在本机再 spawn 一套 MCP 冒充 SSH。
9. **SSH**：子循环 `ctx.ssh = parent.ssh.clone()`。文件/Shell 走现有 `SshToolRuntime`。模型 HTTP 仍在本机（与现 native 一致）。
10. **用量**：子循环 `on_usage` 接到父通道，沿用 `apply_codex_session_usage`。
11. **终端**：`[子 Agent {n}]` 前缀（n 为会话内从 1 起）。启动/结束/子工具行都进父 `native-stdout`。前端 `getLineColor` 给独立色。
12. **活动日志**：`native_subagent_started` / `native_subagent_finished`，仅当会话有 `task_id`。details 含类型、短标题、成功/失败。zh-CN + en。
13. **提示词**：父 `identity.md` 写清何时用 `Agent`、何时不要、两种 type、子 Agent 看不到父对话。
14. **常量**：并发 3；子循环 `max_turns = min(父, 20)`。不新增设置项、不改 schema。
15. **只读 one-shot**：协调员计划路径继续无 `Agent`。

## Acceptance Criteria

- [ ] native 会话模型可调用 `Agent`；缺 `prompt` / 未知 type 返回中文错误且不启动子循环
- [ ] 同一轮连续两个 `Agent` 并行执行；终端出现 `[子 Agent 1]` / `[子 Agent 2]`；父收到两份报告后可继续
- [ ] `explore` 不能改文件；`general` 可写，覆盖已有文件仍弹确认
- [ ] 子循环不能再委派（无 `Agent` 工具）
- [ ] 「本会话全部允许」后，子循环高风险不再问；「仅允许一次」下次仍问
- [ ] 两个高风险同时待确认：先问第一条，resolve 后才问第二条；第一条不会被自动 Deny
- [ ] 停止会话后子循环停止，未决确认视为不允许
- [ ] SSH 项目子 Agent 工具走远程工作区；模型请求仍在本机
- [ ] 子循环 token 计入父会话用量；有任务时仪表盘活动显示中文「启动子 Agent / 子 Agent 结束」
- [ ] 协调员只读计划生成仍无 `Agent`
- [ ] zh-CN + en；clippy / `cargo test` native 相关 / format:check / test:ci / build 通过

## Out of Scope

- plan 模式「计划 → 确认 → 执行」工作流
- Skills / Hooks / ApplyPatch / Browser
- Codex / Claude / Grok / OpenCode 的子 Agent
- 子 Agent 独立会话行、独立终端面板、跨会话记忆
- 设置页并发/轮次配置、文件锁、孙 Agent
