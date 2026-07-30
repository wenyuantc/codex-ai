# Implement: 前端读路径下沉与查询分页

## Preconditions

- [x] PRD 决策锁定（分页 A、硬关 select A）
- [x] design.md 已写
- [ ] 用户批准最终规划摘要后 `task.py start`
- 分支：当前 `feat/read-path-to-command`（相对 main 独立提交）

## Ordered Checklist

### Phase A — Rust DTOs + list commands

1. [ ] `db/models.rs`：增加 `ListTasksPayload`、`ListActivityLogsPayload`、`ActivityLogRow`、`ActivityLogPage`、`DashboardStats`（主页）等
2. [ ] `app/projects.rs`：`list_projects`
3. [ ] `app/employees.rs`：`list_employees`；`list_employee_metrics`（或放 database）
4. [ ] `app/tasks.rs`：
   - `list_tasks`（project_id / status / project_ids / limit / offset 规则）
   - `list_task_attachments` / `list_task_subtasks` / `list_task_comments`（复用 fetch helper）
5. [ ] `app/database.rs` 或 `app/sessions.rs`：`list_activity_logs`（JOIN + scope + filters + total/actions）
6. [ ] `app/database.rs`：`get_dashboard_stats`（聚合；共享 scope helper）
7. [ ] `lib.rs`：注册全部新 command
8. [ ] 单元测试：list_tasks LIMIT 行为、activity 过滤 smoke、stats 基本计数

### Phase B — Frontend backend.ts + stores

9. [ ] `src/lib/backend.ts`：typed wrappers（camelCase invoke 参数对齐现有风格）
10. [ ] `projectStore` → `listProjects`
11. [ ] `employeeStore` → `listEmployees`
12. [ ] `taskStore` → list tasks / attachments / subtasks / comments
13. [ ] `logStore` → `listActivityLogs`
14. [ ] `dashboardStore` → `getDashboardStats` + `listActivityLogs`（keyword 预匹配仍在前端）
15. [ ] `ArchiveManagementDialog` → `listTasks`
16. [ ] `EmployeePerformanceChart` → metrics/employees/projects commands
17. [ ] `TaskSessionChainPanel` → `listActivityLogs`
18. [ ] `src/lib/ai.ts` → `listEmployees` + 过滤

### Phase C — Hard close + docs

19. [ ] 确认 `rg` 无业务 `select` / database import
20. [ ] `capabilities/default.json` 移除 `sql:allow-select`
21. [ ] `database.ts`：`select`（及无用 `getDb`）硬失败
22. [ ] 更新 `.trellis/spec/frontend/data-access.md`
23. [ ] 若 CLAUDE.md / Agents 中「前端可读 SQL」表述过时则更新

### Phase D — Validate

24. [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
25. [ ] `npm run build`
26. [ ] 手工冒烟（见下）
27. [ ] `trellis-check` / 质量门

## Validation Commands

```bash
# 无业务直读
rg 'from ["'\'']@/lib/database|from ["'\'']\./database|select<' src --glob '*.{ts,tsx}'

# capability
rg 'sql:allow-select' src-tauri/capabilities

cargo test --manifest-path src-tauri/Cargo.toml
npm run build
```

手工冒烟（`npm run tauri:dev`）：

1. 本地环境：看板加载任务列；创建/改状态后刷新
2. 项目详情任务列表与任务详情附件/子任务/评论
3. 仪表盘数字与最近活动、活动分页筛选
4. 归档管理对话框
5. 员工页列表 + 仪表盘绩效图
6. 任务详情会话链路 fix 日志
7. 切换 SSH 环境：项目/任务/活动 scope 与迁移前一致

## Risky Files / Rollback Points

| 风险点 | 文件 | 缓解 |
|---|---|---|
| activity 动态 SQL 语义漂移 | `dashboardStore.ts` ↔ 新 Rust command | 对照常量/scope 分支写测试 |
| stats 数字不一致 | `get_dashboard_stats` | 与旧客户端聚合逻辑逐字段对齐 |
| 硬关 select 漏迁 | capabilities + database.ts | 同提交前 rg 清零 |
| invoke 参数命名 | backend.ts serde | 对齐现有 camelCase 惯例 |

回滚：revert 本任务提交即可恢复旧读路径与 capability。

## Default Constants

```text
LIST_TASKS_DEFAULT_LIMIT = 500
LIST_TASKS_MAX_LIMIT     = 1000
LIST_ACTIVITY_DEFAULT    = 50
LIST_ACTIVITY_MAX        = 500
```

## Out of implement scope

- 看板虚拟列表 / React.memo（C4）
- UI 分页控件
- DB migration（无表变更）
