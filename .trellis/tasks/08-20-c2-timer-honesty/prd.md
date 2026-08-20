# PRD · C2 工时计时器去伪

父任务:`08-20-product-trust-ops` · 优先级 P2

## Goal

任务侧栏展示计时状态（未开始/进行中/已暂停/已完成），但用户不能开始或暂停。秒数主要由完成流程回写。这是假功能。

## 证据

- 只读展示：`src/components/tasks/detail/TaskPropertiesSidebar.tsx:123-137`、`:294-307`
- 字段：`Task.time_started_at` / `time_spent_seconds`
- 完成时回写：`src-tauri/src/task_automation/state.rs:277-329`
- 全库无 start/pause timer 的用户操作入口

## Requirements

**推荐（更小）**：空闲且 `time_spent_seconds=0` 且未开始时，侧栏不展示计时器模块；仅在真正在跑或已有累计耗时/已完成时显示只读耗时。

若实现时选择「可操作」方案，则必须同时提供开始/暂停，并走 Rust command 写库（前端不直写 SQL），SSH 同样生效。

禁止继续展示「未开始」却没有任何按钮。

## Acceptance Criteria

- [ ] 新建待办任务侧栏不再出现无法操作的「未开始」计时器
- [ ] 运行中或已有耗时的任务仍能看到耗时（`formatDate` / duration）
- [ ] 不破坏完成任务时已有的 `time_spent_seconds` 回写
- [ ] i18n zh-CN + en

## Out of Scope

工时报表、按员工汇总工时、番茄钟。
