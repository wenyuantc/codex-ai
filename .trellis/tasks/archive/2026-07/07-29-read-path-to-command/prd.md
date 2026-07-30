# 前端读路径下沉与查询分页

## Goal

把前端业务读路径从「直接 `select` SQL」收敛到 Tauri command，消除无界 `tasks` 全表拉取；读侧与写侧统一走 Rust → SQLite，并为后续渲染优化（C4）提供稳定的 store 接口。

用户价值：任务/看板/仪表盘在数据量变大时不再整表灌入前端；数据访问边界清晰，防止直读 SQL 回潮。

## Background

父任务覆盖原发现 #5。本任务是 `07-29-codex-ai-optimization` 的子任务 C3；可与 C1b 穿插，**应在 C4（前端渲染）之前完成**。

### 直读 SQL 证据（2026-07-30）

| # | 文件 | 用途 | 风险 |
|---|---|---|---|
| 1 | `src/stores/taskStore.ts` | tasks / attachments / subtasks / comments | 无 `projectId` 时 tasks 全表无 LIMIT |
| 2 | `src/stores/dashboardStore.ts` | stats 全表聚合；activities 动态 SQL 分页 | stats 客户端拉全表 tasks |
| 3 | `src/stores/projectStore.ts` | projects 列表 | 中等 |
| 4 | `src/stores/employeeStore.ts` | employees 列表 | 中等 |
| 5 | `src/stores/logStore.ts` | activity_logs + LIMIT | 路径不统一 |
| 6 | `src/components/tasks/ArchiveManagementDialog.tsx` | 归档 tasks | 直读 |
| 7 | `src/components/dashboard/EmployeePerformanceChart.tsx` | employee_metrics + employees + projects | 直读 |
| 8 | `src/components/tasks/detail/TaskSessionChainPanel.tsx` | task 级 automation fix activity | 直读 |
| 9 | `src/lib/ai.ts` | `suggestAssignee` 读 employees | 相对路径 import，父任务「8 处 @/」未计入 |

能力层仍开放 `sql:allow-select`（`src-tauri/capabilities/default.json`）。写路径已禁止。

后端已有 `list_trashed_*`、`get_dashboard_report_summary`、`list_notifications` 等模式；**缺失**活跃 projects/employees/tasks 基础 list、task 附属 list、activity 查询、dashboard 主页 stats、employee_metrics list。看板多为项目作用域全量；真正无界的是无 `projectId` 的 tasks 与 `fetchStats` 全表。

## Requirements

### R1 — 读路径下沉

9 处业务读点全部改为 Tauri command + `src/lib/backend.ts` 包装；store/组件不再 import `database` 的 `select`。

覆盖：projects、employees、tasks（含归档筛选）、attachments/subtasks/comments、activity_logs（简单 limit / 按 task+action / dashboard 分页 filters）、dashboard stats、employee_metrics、notifications 汇总（复用或并入 stats command）。

### R2 — 消除 tasks 无界全表（策略 A）

- 有 `project_id`：`deleted_at IS NULL` 项目内全量（看板/项目详情），无 UI 分页
- 无 `project_id`：服务端强制 `limit`（默认 **500**，可钳制上限）+ 可选 `offset`
- 仪表盘 stats：后端聚合 command，禁止为计数拉全部 task 行
- 本任务不引入看板/任务列表 UI 分页或无限滚动

### R3 — 接口与兼容

- Store 对外方法签名尽量保持（`fetchTasks` / `fetchProjects` / `fetchEmployees` / `fetchStats` 等）
- 新 command 注册 `lib.rs` invoke_handler，经 `backend.ts` 暴露
- local / SSH 数据均在本地 SQLite；environment / ssh_config 作用域过滤语义与迁移前一致

### R4 — 读路径硬关闭（策略 A）

迁移完成后：

1. 从 `capabilities/default.json` 移除 `sql:allow-select`
2. `src/lib/database.ts` 的 `select()`（及若无调用方的 `getDb`）改为硬失败，与 `execute()` 一致
3. 防止业务代码再直读 SQL

### R5 — 活动日志

本任务不新增 activity `action` 键；若意外新增，必须同步 `getActivityActionLabel` 中文标签。

## Decisions

| 决策 | 选择 | 日期 |
|---|---|---|
| 无界 tasks 保护 | **A**：项目内全量；全局 limit 默认 500 + offset；stats 聚合；无列表分页 UI | 2026-07-30 |
| 读路径硬关闭 | **A**：本任务内移除 `sql:allow-select` 且 `select()` throw | 2026-07-30 |

## Out of Scope

- UI 视觉改版、看板虚拟滚动 / memo（C4）
- 任务列表/看板 UI 分页与无限滚动
- 表结构语义变更、历史表改名
- 写路径改造（已完成）
- 其他父任务子项（SSH 复用、引擎 trait、lint 等）
- 服务端缓存 / 多用户权限模型

## Acceptance Criteria

- [ ] 9 处业务读点不再调用前端 `select`；`src` 内对 `database` 的 import 仅剩废弃桩或无业务调用
- [ ] command 覆盖 projects、employees、tasks（含附件/子任务/评论）、activities、dashboard stats、employee_metrics
- [ ] `list_tasks`：有 `project_id` 项目内全量；无 `project_id` 强制 LIMIT（默认 500）+ 可选 offset；`fetchStats` 不再全表加载 tasks
- [ ] 看板、项目详情、仪表盘 stats/最近活动、归档对话框、员工绩效图、会话链路 fix 日志、员工推荐 行为与迁移前一致（含 local / SSH 环境过滤）
- [ ] `capabilities/default.json` 无 `sql:allow-select`；前端 `select()` 调用即失败
- [ ] `npm run build` 通过；`cargo test --manifest-path src-tauri/Cargo.toml` 全绿且用例数不下降
- [ ] 规格文档 `.trellis/spec/frontend/data-access.md` 更新为「读也走 command」（实现后 / Phase 3.3）

## Notes

- 复杂任务：需 `design.md` + `implement.md`
- 顺序：先于 C4；独立分支 + 单独提交合回 `main`
