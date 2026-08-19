# Design · B2 任务模板

## Boundaries

- 新模块 `src-tauri/src/app/templates.rs`：模板 CRUD + 套用。不把模板逻辑塞进已 3k+ 行的 `tasks.rs`。
- 创建任务仍走与 `create_task` 相同的默认值解析（`resolve_project_task_default_settings`）和审查员/经办人校验。套用**不带附件**，避免 SSH 上传。
- 前端只经 `src/lib/backend.ts`。模板列表是对话框本地状态，不进 Zustand（非全局缓存）。套用成功后 `taskStore.fetchTasks` + 刷新标签图。
- 组件：`src/components/tasks/TaskTemplateManagerDialog.tsx`；看板头部入口；`TaskCard` 右键「存为模板」。

## Schema（v47）

```sql
CREATE TABLE task_templates (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  project_id TEXT REFERENCES projects(id),
  title_template TEXT NOT NULL,
  description_template TEXT,
  priority TEXT NOT NULL DEFAULT 'medium',
  use_worktree INTEGER NOT NULL DEFAULT 0,
  tags_json TEXT NOT NULL DEFAULT '[]',
  subtasks_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  deleted_at TEXT
);
CREATE INDEX idx_task_templates_project ON task_templates(project_id, deleted_at);
```

`tags_json` = `string[]`（名称）。`subtasks_json` = `{title, sort_order}[]`。列表过滤 `deleted_at IS NULL`。`latest_migration_version` 46→47。

## Contracts

| 命令 | 行为 |
|---|---|
| `list_task_templates(project_id?)` | 全局 + 指定项目；不含软删 |
| `create_task_template` / `update_task_template` / `delete_task_template` | 软删；更新用 `Option<Option<T>>` 清可空字段 |
| `create_task_template_from_task(task_id, name?)` | 读任务 + `list_task_tags` + `fetch_task_subtasks`；默认 name=任务标题 |
| `apply_task_template` | `{template_id, project_id, variable_sets: [{k:v}], assignee_id?, reviewer_id?}` → `Vec<Task>` |

变量纯函数（单测）：

- `extract_template_variables(text) -> Vec<String>`：`{{ident}}`，ident = `[A-Za-z_][A-Za-z0-9_]*`，去重保序。
- `render_template(text, values) -> Result<String,String>`：缺键 → `模板变量「x」未填写`；未知键忽略。
- 扫描 `title_template` + `description_template`。渲染后标题 trim 为空 → `任务标题不能为空`。
- `variable_sets.len() > 100` → `单次批量操作最多 100 个任务`。无占位且 sets 空 → 当作 1 组空 map。

套用顺序：先渲染并校验全部组 → 再开事务插入。任一组失败则不写库。事务内每条：`insert_task_record` + 自动质控 state（若默认开启）+ 按名称 `UNIQUE(project_id,name)` 找或建标签 + `task_tags` + 子任务（status 默认待办）。提交后写 `task_created`×N 与一条 `task_template_applied`（details 含数量与模板名）。

`use_worktree` 以模板值为准。自动质控开启且未传审查员 → 与 `create_task` 同一中文错，整批失败。

## Compatibility

- SSH：模板在本地 SQLite；套用只建任务行，不碰远程文件。默认 worktree / 质控仍按目标项目设置解析。
- 不改 `create_task` 对外契约；可抽内部 helper 给套用复用，避免复制校验。
- 活动键：`task_template_created` / `task_template_applied` / `task_template_deleted`，zh-CN + en 同步。

## UI

- 看板「模板」在新建任务与归档管理之间。
- 管理对话框：左列表、右编辑（名称/绑定项目或全局/标题/详情/优先级/worktree/标签名/子任务标题）。详情用 Textarea，对齐 `CreateTaskDialog`。
- 套用：解析变量列成表格，可增删行；可选经办人/审查员；提交后关闭并刷新看板。
- 存为模板：小对话框填名称（预填任务标题）后调用 `from_task`。

## Tradeoffs

- 表存储而非 JSON 文件：可软删、按项目过滤、进 SQL 备份；导入导出本波不做。
- 套用自建事务而不循环调 `create_task`：才能整批回滚；代价是要复用 insert/质控初始化，不能走附件分支。
- 标签存名字不存 ID：跨项目可套用；目标项目无同名则新建。

## Rollback

独立提交。失败则 revert 该提交，并确认迁移版本断言回到 46。已升级用户库保留 v47 空表可接受（additive）。
