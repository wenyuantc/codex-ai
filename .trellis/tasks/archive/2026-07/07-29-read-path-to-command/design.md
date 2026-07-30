# Design: 前端读路径下沉与查询分页

## Architecture

目标数据流：

```text
Component / Page
    ↓
Zustand store（编排，签名尽量不变）
    ↓
backend.ts typed invoke wrappers
    ↓
Rust Tauri commands
    ↓
SQLite (sqlx)
```

迁移后前端 **不再** 持有业务 SQL。`sql:allow-select` 移除；`database.select` 硬失败。

对齐既有约定（`.trellis/spec/backend/command-guidelines.md`、`frontend/data-access.md` 将在完成后改写）。

## Command Surface

### 1. 基础 list（对齐 `list_trashed_*` 风格）

| Command | 模块 | 输入 | 输出 | 语义 |
|---|---|---|---|---|
| `list_projects` | `app/projects.rs` | — | `Vec<Project>` | `deleted_at IS NULL ORDER BY updated_at DESC` |
| `list_employees` | `app/employees.rs` | 可选 `status_ne` / 或前端过滤 | `Vec<Employee>` | `ORDER BY created_at`；`suggestAssignee` 可前端过滤 `status != 'error'` |
| `list_tasks` | `app/tasks.rs` | `project_id?`, `status?`, `project_ids?`, `limit?`, `offset?` | `Vec<Task>` | 见分页规则 |
| `list_task_attachments` | `app/tasks.rs` | `task_id` | `Vec<TaskAttachment>` | 包装已有 `fetch_task_attachments` |
| `list_task_subtasks` | `app/tasks.rs` | `task_id` | `Vec<Subtask>` | 包装已有 `fetch_task_subtasks` |
| `list_task_comments` | `app/tasks.rs` | `task_id` | `Vec<Comment>` | 新 fetch helper |
| `list_employee_metrics` | `app/employees.rs` 或 `database.rs` | `days: i64` | `Vec<EmployeeMetric>` | `period_start >= datetime('now', '-N days')` |

`list_tasks` 规则（PRD 策略 A）：

```text
WHERE deleted_at IS NULL
  [AND project_id = $pid]
  [AND status = $status]
  [AND project_id IN (...)]   -- 归档对话框：可见项目集合
ORDER BY updated_at DESC[, id DESC]
-- 若 project_id 缺失：LIMIT clamp(limit, 1..=1000) default 500, OFFSET offset default 0
-- 若 project_id 存在：不加 LIMIT（项目内全量）
-- 若仅 project_ids（归档）：按 IN 过滤，可不加 LIMIT（归档集通常小）；若担心可加默认 2000 上限
```

归档对话框当前传 `status = 'archived'` + 可见 `project_ids`。command 用 payload 表达，避免前端拼 SQL。

### 2. Dashboard stats 聚合

| Command | 模块 | 输入 | 输出 |
|---|---|---|---|
| `get_dashboard_stats` | `app/database.rs` | `environment_mode?`, `selected_ssh_config_id?`, `project_id?` | `DashboardStats` DTO |

字段对齐现 `dashboardStore` 的 `DashboardStats`：

- `total_projects`, `active_projects`
- `total_tasks`, `tasks_by_status: Record/map`, `completion_rate`
- `total_employees`, `online_employees`（status ∈ `online` \| `busy`）
- `unread_notifications`, `high_severity_notifications`（`state = 'active'`）

实现要点：

- 复用/扩展 `count_tasks_with_scope` 模式做按 status 计数（`GROUP BY status` 一次查出更佳）
- projects 过滤：`deleted_at IS NULL` + `project_type`（local/ssh）+ 可选 `ssh_config_id`
- employees：`project_id` 落在 scoped projects 内；无 project 过滤时含「无 project 绑定」逻辑需与现前端一致（`employee.project_id ? scoped : !projectId`）
- notifications：可 SQL 聚合 count，不必 list 全表

**不**复用 `get_dashboard_report_summary` 直接替换（字段集不同：report 含 weekly/workload；主页 stats 含 notifications/employees online）。可共享 scope helper 函数。

### 3. Activity logs

前端当前有两套：

1. `logStore`：简单 `ORDER BY created_at DESC LIMIT n`
2. `dashboardStore`：JOIN employees/projects，scope + keyword/action/date 过滤，LIMIT/OFFSET + COUNT + DISTINCT actions
3. `TaskSessionChainPanel`：`task_id` + `action = 'task_automation_fix_started'`

建议统一为：

| Command | 输入 | 输出 |
|---|---|---|
| `list_activity_logs` | `ListActivityLogsPayload` | `ActivityLogPage` |

```text
ListActivityLogsPayload {
  limit?: number,          // default 50, clamp 1..=500
  offset?: number,         // default 0
  task_id?: string,
  action?: string,
  project_id?: string,     // filter single project
  environment_mode?: string,
  selected_ssh_config_id?: string | null,
  keyword?: string,
  start_date?: string,     // YYYY-MM-DD
  end_date?: string,
  include_total?: bool,    // page 模式需要 total + available_actions
  project_ids?: string[],  // 可选：前端已算好的 visible set；若缺省则后端按 env/ssh 自己算
}
```

```text
ActivityLogPage {
  items: ActivityLogRow[],  // 含 employee_name?, project_name?（LEFT JOIN）
  total: i64,               // include_total 时
  available_actions: string[]  // include_total 时
}
```

Scope 语义必须从 `dashboardStore.ts` 原样下沉：

- 有 `project_id`：仅该项目且必须在可见集合内，否则空
- 无 `project_id`：`project_id IN visible` OR（local：`project_id IS NULL`；ssh：`project_id IS NULL AND global action 前缀/集合`）

全局 action 常量与前端 `GLOBAL_ACTIVITY_PREFIXES` / `GLOBAL_ACTIVITY_ACTIONS` 对齐，放在 Rust 常量，避免漂移。

Keyword 匹配中文 label 依赖前端 `getActivityActionLabel` / `getStatusLabel`。两种做法：

1. **推荐（MVP）**：keyword 的「中文 label 反查 action/status」仍在前端做，把匹配到的 `matched_actions[]` / `matched_statuses[]` 作为 payload 字段传给 Rust；Rust 只做 SQL LIKE + IN。这样不把整份中文 map 复制进 Rust。
2. 备选：把 label map 复制到 Rust（双份维护，不推荐）。

`logStore` 与 `TaskSessionChainPanel` 走同一 command 的窄参数子集。

### 4. 硬关闭前端 SQL

1. `capabilities/default.json`：删除 `sql:allow-select`（可保留 `sql:default` 若插件要求；若插件无其他用途，评估是否整段 sql 权限可去——**本任务至少去掉 allow-select**）
2. `database.ts`：`select` throw；`getDb` 若无引用则一并废弃 throw 或删除导出
3. 验收：全仓无业务 import

## Data Flow by Call Site

| 调用点 | 新路径 |
|---|---|
| `projectStore.selectProjectsFromDatabase` | `listProjects()` |
| `employeeStore.fetchEmployees` | `listEmployees()` |
| `taskStore.fetchTasks` | `listTasks({ projectId, limit?, offset? })`；前端仍可按 visible projects 二次 filter |
| `taskStore` attachments/subtasks/comments | `listTaskAttachments/Subtasks/Comments(taskId)` |
| `dashboardStore.fetchStats` | `getDashboardStats({ environmentMode, selectedSshConfigId, projectId })` |
| `dashboardStore` activities | `listActivityLogs` + 前端 keyword 预匹配 |
| `logStore.fetchLogs` | `listActivityLogs({ limit })` |
| `ArchiveManagementDialog` | `listTasks({ status: 'archived', projectIds })` |
| `EmployeePerformanceChart` | `listEmployeeMetrics({ days })` + `listEmployees` + `listProjects`（或一个组合 command；MVP 三次 invoke 可接受） |
| `TaskSessionChainPanel` | `listActivityLogs({ taskId, action })` |
| `ai.suggestAssignee` | `listEmployees()` 后前端过滤 |

## Frontend Boundaries

- 所有新 invoke **仅**经 `src/lib/backend.ts`
- Store 方法签名尽量不变；内部换 command
- `filterProjectsByScope` / `normalizeProject` 继续用
- `fetchTasks` 无 projectId：传 `limit: 500`（或依赖后端默认）

## Models / DTOs

新增于 `db/models.rs`（或 command 旁私有 struct + Serialize）：

- `ListTasksPayload`
- `ListActivityLogsPayload`
- `ActivityLogRow`（ActivityLog + optional names）
- `ActivityLogPage`
- `DashboardStats`（主页用，勿与 `DashboardReportSummary` 混淆）
- `ListEmployeeMetricsPayload`（若需要）

## Compatibility

- **SSH**：数据在本地库；scope 按 `project_type` / `ssh_config_id` 过滤，与现前端一致
- **软删**：活跃 list 一律 `deleted_at IS NULL`
- **不改表结构**，无 migration（除非发现缺索引且必要；默认无 migration）
- **无新 activity action**

## Trade-offs

| 选择 | 理由 | 代价 |
|---|---|---|
| 项目内 tasks 全量 | 看板列分布需要 | 超大单项目仍可能重（C4 再优化） |
| stats 独立聚合 command | 避免全表行 | 与 report summary 两套统计，需共享 scope helper |
| activity keyword label 反查留前端 | 避免中文 map 双份 | payload 多两个可选数组 |
| 硬关 select | 防回潮 | 漏迁即运行失败——用搜索+build 兜底 |

## Rollback

- 单分支提交；回滚即 revert 该提交
- 能力与 `select` 硬关与迁移同提交，避免「半迁移可运行」状态长期存在
- 若需紧急 hotfix：可临时恢复 `sql:allow-select`（不推荐作为常态）

## Testing

Rust：

- `list_tasks`：有/无 project_id 的 LIMIT 行为；status/project_ids 过滤
- `get_dashboard_stats`：scope 下计数合理（可用现有 test pool 模式）
- `list_activity_logs`：task_id 过滤；limit/offset；空 visible 时为空

前端：

- `npm run build`
- 手工：`tauri:dev` — 看板、仪表盘、归档、员工页、任务详情会话链路、SSH 环境切换

## Spec update (post-implement)

更新 `.trellis/spec/frontend/data-access.md`：删除「SQL select 读门」架构，改为「读写均走 command」；`database.ts` 仅废弃桩说明。
