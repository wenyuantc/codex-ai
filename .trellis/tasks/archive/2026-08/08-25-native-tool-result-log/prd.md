# 内置 Agent 终端显示完整工具结果

## Goal

内置 Agent 任务/会话终端里的 `[工具结果]` 显示工具返回的完整正文，而不只是第一行 200 字。用户能从日志看出 Read/Grep/Bash/MCP 实际返回了什么。

## Confirmed Facts

- `tool_result_line`（`native/agent/loop.rs`）对非 Todo 工具只取第一行非空行，再 `truncate_chars(..., 200)`。
- 模型侧 `Message::tool_result` 仍是完整 output；只截 UI 事件。
- `TodoRead` 已经按全文发出；`TodoWrite` 清单用「已更新 N 项」，因为 `[待办]` 启动行已列出全文。
- `CodexTerminal` 用 `whitespace-pre-wrap`，一条事件里的换行可以完整画出。
- `taskLogs` 只留最近 199 条，工具结果必须仍是 **一条** stdout，不能按行拆发。
- `native/agent/truncate.rs` 是给模型压缩上下文的，不是 UI，本任务不改。

## Requirements

1. Read / Grep / Bash / Glob / Write / Edit / Web* / MCP / 错误等：终端发 `[工具结果]\n{output}`，不再压成一行。
2. TodoWrite 且输出是清单：保持 `[工具结果] 已更新 N 项`。
3. 超长只截 UI：超过 2000 行或 64KB 时保留前缀，末尾 `…（已截断，共 N 行 / M 字）`。模型仍拿完整 output。
4. 本地与 SSH 共用 `AgentRunner.emit`，不分工。
5. 不新增折叠 UI、不改其它引擎的 `[工具结果]` 摘要、不改活动日志/数据库。

## Acceptance Criteria

- [x] AC1：`tool_result_line("Read", "line1\nline2")` 为 `[工具结果]\nline1\nline2`，不再是 `[工具结果] line1`。
- [x] AC2：TodoWrite 清单结果仍是「已更新 N 项」。
- [x] AC3：超长输出带截断提示；未超限的短结果不截。
- [x] AC4：`clippy -D warnings`、相关 `cargo test`、`format:check` 通过。

## Out of Scope

- Codex / Claude / Grok / OpenCode 终端工具摘要
- 折叠/展开交互
- 给模型的 `truncate_messages`
- plan-mode / AskQuestion 行为
