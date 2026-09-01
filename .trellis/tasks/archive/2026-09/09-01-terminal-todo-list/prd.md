# 终端日志显示内置 Agent 待办列表

## Goal

看板「查看终端日志」、会话「终端日志」、任务审核「审核终端输出」打开终端时，若内置 Agent 调用过 `TodoWrite`，在日志上方展示当前待办列表；后续再调用并勾选完成项时，面板跟着更新。

## Confirmed Facts

- 内置 Agent `TodoWrite` 已向终端发一条多行事件：`[待办]\n- [status] content (priority)`。
- `TodoWrite` 结果行是 `[工具结果] 已更新 N 项`，清单正文在启动行，不在结果行。
- 看板任务日志、会话日志、审核终端都走 `CodexTerminal`。
- 不新增数据库字段：从现有 stdout 日志解析最近一次完整清单。

## Requirements

1. `CodexTerminal` 在「终端输出」标题和日志正文之间展示最新待办面板；无 `TodoWrite` 清单时不占位。
2. `completed` 打钩并划掉；`in_progress` 高亮；`pending` 空心圆；后续 `TodoWrite` 以最新快照替换。
3. 关键字过滤日志时，待办仍取完整会话/任务日志，不被过滤条件藏掉。
4. 忽略 `[待办] 读取任务清单` / `[待办] 更新任务清单`；`[待办] (空)` 视为清空。
5. 不解析子 Agent 前缀行；不改 `TodoWrite` 后端格式；无新活动日志。

## Acceptance Criteria

- [x] 看板任务终端、会话终端日志、审核终端输出在内置 Agent 调用 `TodoWrite` 后于日志上方显示同一份待办列表。
- [x] 再一次 `TodoWrite` 把某项改为 `completed` 后，该项打钩更新，不必刷新页面。
- [x] 从未调用 `TodoWrite` 的会话不出现待办面板。
- [x] 解析纯函数单测覆盖清单快照、清空、忽略非清单标签、跨多次调用取最新。
- [x] `npm run test:ci`、`npm run format:check`、`npm run build` 通过。

## Out of Scope

- Codex / Claude / Grok / OpenCode 各自的 todo 工具摘要格式
- 把待办写入 SQLite 或活动日志
- 折叠/编辑待办
