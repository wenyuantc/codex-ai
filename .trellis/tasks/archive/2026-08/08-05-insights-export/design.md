# Design — 报表洞察与任务 JSON 导入导出

## 1. 边界与现状

| 能力 | 现状 | 本任务动作 |
|------|------|------------|
| 统计卡 | `get_dashboard_stats` + SSH/project 作用域 | 不动契约，作过滤对齐参考 |
| 增强报表 | `get_dashboard_report_summary(project_id, environment_mode)` | **扩展 DTO + 对齐 SSH 过滤** |
| CSV 导出 | `export_tasks_csv` 扁平字段 | UI 降级/替换；不必作为 AC |
| JSON 导入导出 | 无 | **新增 commands + UI** |
| SQL 备份 | settings `backup_database` | 不改 |

数据流保持：

```text
UI → backend.ts invoke → Rust command → SQLite 聚合/事务 → serde DTO
```

前端禁止直写 SQL；不新增前端侧业务聚合。

## 2. 洞察（R1）

### 2.1 Command 契约调整

将 `get_dashboard_report_summary` 改为与 stats 一致的 payload 风格（推荐，减少参数漂移）：

```rust
// 建议
struct GetDashboardReportPayload {
  project_id: Option<String>,
  environment_mode: Option<String>,
  selected_ssh_config_id: Option<String>,
  aging_days: Option<i64>, // default 7
}

struct DashboardReportSummary {
  // 既有
  total_tasks, completed_tasks, overdue_tasks, blocked_tasks,
  in_progress_tasks, completion_rate,
  weekly_completed: Vec<DashboardTrendPoint>, // 保留：近 7 日（字段名历史遗留，语义=按日）
  employee_workload: Vec<DashboardWorkloadItem>,
  // 新增
  daily_completed: Vec<DashboardTrendPoint>,   // 显式近 7 日；可与 weekly_completed 同源后 deprecate 其一
  weekly_completed_series: Vec<DashboardTrendPoint>, // 近 8 周，label 如 "2026-W31"
  aging_in_progress: i64,
  aging_days: i64,
}
```

**兼容策略**：前端已读 `weekly_completed` 作为「近 7 日」。实现时：

1. 保持 `weekly_completed` = 近 7 日（避免 silent break）；
2. 新增 `weekly_completed_series` = 真正按周；
3. UI 文案改为「近 7 日完成趋势」+「近 8 周完成趋势」双图或 Tab。

若实现阶段希望更干净，也可只加 `weekly_completed_series` + `aging_*`，日序列继续用 `weekly_completed`。

### 2.2 作用域

复用 `get_dashboard_stats` / activity 的项目可见性解析：

- `resolve_scoped_project_ids(environment_mode, selected_ssh_config_id)` 一类 helper（database.rs 已有类似逻辑）。
- `count_tasks_with_scope` 今日只看 `project_type`；需扩展支持 **project id 列表** 或 **ssh_config_id**，避免 SSH 多主机串数据。
- 单项目过滤：`project_id` 必须落在 scoped 集合内，否则返回空/0。

### 2.3 老化指标

```sql
-- 语义：status = in_progress 且 updated_at（或 time_started_at 优先）早于 now - N days
AND t.status = 'in_progress'
AND date(COALESCE(t.time_started_at, t.updated_at)) <= date('now', printf('-%d day', N))
```

优先 `time_started_at`，空则 `updated_at`。不新增 migration。

### 2.4 周序列

对 `completed_at` 按 `strftime('%Y-%W', completed_at)` 或 ISO 周聚合最近 8 个桶；空日/周 count=0 填齐，保证前端定长图。

## 3. 任务 JSON 导出/导入

### 3.1 Envelope（v1）

```json
{
  "format": "codex-ai.tasks",
  "version": 1,
  "exported_at": "2026-08-05T12:00:00Z",
  "source": {
    "project_id": "optional",
    "environment_mode": "local|ssh|null",
    "app": "codex-ai"
  },
  "tasks": [
    {
      "source_id": "uuid-original",
      "title": "...",
      "status": "todo|in_progress|review|blocked|completed|archived|...",
      "priority": "low|medium|high|urgent|...",
      "description": null,
      "due_date": null,
      "blocked_reason": null,
      "completed_at": null,
      "tags": ["bug", "p0"],
      "subtasks": [
        { "title": "...", "status": "todo", "sort_order": 0 }
      ],
      "depends_on_source_ids": ["other-source-id-in-file"]
    }
  ]
}
```

**明确省略**：assignee/reviewer/coordinator、attachments、comments、sessions、automation、git context、plan_content、ai_suggestion。

### 3.2 Commands

| Command | 入参 | 出参 |
|---------|------|------|
| `export_tasks_json` | `project_id?`, `environment_mode?`, `selected_ssh_config_id?`, `limit?` | `{ json: string, task_count, truncated: bool }` |
| `import_tasks_json` | `project_id`（必填）, `json` 或 `content`, `conflict_strategy: create_new\|skip_existing` | `{ created, skipped, failed, errors: [{index, message}], task_ids: [...] }` |

实现位置建议：`src-tauri/src/app/tasks.rs`（与 `export_tasks_csv` 并列）；models 放 `db/models.rs`；`lib.rs` 注册。

### 3.3 导出算法

1. 解析 scoped project ids（同 stats）。
2. 查询 tasks `deleted_at IS NULL` + scope + `ORDER BY updated_at DESC LIMIT N`。
3. 批量拉 tags（名称）、subtasks、dependencies（仅 depends_on 在导出集合内的边）。
4. 序列化 envelope；**不写磁盘**（与 CSV 一致返回字符串，前端 Blob 下载），避免 Tauri 权限分叉；若后续要路径导出可复用 backup 路径解析。

### 3.4 导入算法

1. 校验目标项目存在且未删除；记录 `project_type`（local/ssh 仅用于日志，不改变 JSON 语义）。
2. 解析 JSON：`format == codex-ai.tasks` 且 `version == 1`；否则中文错误。
3. 单事务（或分批事务，上限同导出）：
   - `create_new`：新 task id；建立 `source_id → new_id` map；插入 task（合法 status/priority 白名单，非法则失败该条或整批——**推荐单条失败计入 errors，成功条提交**；若实现简单可整批事务失败回滚——**推荐整批事务 + 预校验**，避免半导入）。
   - **推荐：预校验全部通过后单事务提交**；任一条 schema 失败则 0 写入并返回 errors。
   - `skip_existing`：`source_id` 已存在于 `tasks.id`（含软删？**仅未删除**）则 skip；否则同 create_new。
4. 标签：`SELECT id FROM tags WHERE project_id=? AND name=?`；无则 insert tag。
5. 子任务：新 id，挂新 task_id。
6. 依赖：仅当两端都在 map 内时插入 `task_dependencies`；悬空边记 warning 不失败。
7. Activity：
   - `tasks_json_exported`（导出，details 含 count）
   - `tasks_json_imported`（导入，details 含 created/skipped）

### 3.5 状态/优先级白名单

复用前端 `TASK_STATUSES` / 后端 create_task 已有校验；导入时 normalize 小写；未知 status → 该条 error 或 fallback `todo`（**推荐 error**，避免脏数据）。

## 4. 前端

### 4.1 仪表盘

- `DashboardPage`：拉取 report 时传入 `environmentMode` + `selectedSshConfigId` + `currentProjectId`。
- 展示：近 7 日柱图（已有）+ 近 8 周柱图 + 「老化进行中：N（>7 天）」文案。
- 导出按钮改为「导出任务 JSON」；新增「导入任务 JSON」（需当前项目，否则 toast 提示先选项目）。
- 导入：`<input type="file" accept="application/json,.json">` 读文本 → `importTasksJson`；结果摘要中文展示。

### 4.2 backend.ts / types

- 新增 `exportTasksJson` / `importTasksJson` / 扩展 `DashboardReportSummary` 类型。
- 可选：`dashboardStore` 缓存 report（非必须，页面 local state 可继续）。

### 4.3 Activity labels

`getActivityActionLabel` 增加：

| key | 中文 |
|-----|------|
| `tasks_json_exported` | 导出任务 JSON |
| `tasks_json_imported` | 导入任务 JSON |

## 5. 测试

Rust（`tasks` 模块 `#[cfg(test)]` 或 `app/tests`）：

1. Envelope round-trip 序列化字段齐全且无 assignee。
2. Import `create_new` 双任务+依赖重映射。
3. Import `skip_existing` 跳过已存在 id。
4. 非法 format/version 拒绝。
5. Report aging / week buckets 在内存 sqlite fixture 上计数（若现有 test harness 支持）。

## 6. 兼容与回滚

- **无 schema migration**（纯 command + UI）。
- 回滚：移除新 command 注册与 UI 按钮；旧 CSV command 若保留则行为不变。
- 旧前端若仍调无 ssh 参数的 report：serde 默认 `None`，退化为仅 environment_mode（与今日行为接近）。

## 7. 风险

| 风险 | 缓解 |
|------|------|
| `weekly_completed` 命名歧义 | UI 文案明确「近 7 日」；周序列用新字段 |
| 大 JSON 主线程卡顿 | limit 5000；导入预校验失败快速返回 |
| 标签名冲突颜色 | 新建 tag 用默认色 |
| SSH 项目导入 | 任务落在目标 project_id 即可，不碰远程附件 |

## 8. 不做

- CSV 产品化 / 双向 OAuth
- 导入恢复员工外键与附件
- 独立 BI 页面
