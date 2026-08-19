# Task Templates

> Reusable task blueprints with `{{ident}}` substitution and batch apply.

## 1. Scope / Trigger

Use this spec when changing `task_templates`, `app/templates.rs`, apply/create-from-task, or the Kanban template dialogs.

Table: migration **v47**. Commands live in `app/templates.rs`, not `tasks.rs`. Apply does **not** call `create_task` (no attachments / SSH upload).

## 2. Signatures

```rust
async fn list_task_templates(app, project_id: Option<String>) -> Result<Vec<TaskTemplate>, String>
async fn create_task_template(app, payload: CreateTaskTemplate) -> Result<TaskTemplate, String>
async fn update_task_template(app, id, updates: UpdateTaskTemplate) -> Result<TaskTemplate, String>
async fn delete_task_template(app, id) -> Result<(), String> // soft delete
async fn create_task_template_from_task(app, task_id, name: Option<String>) -> Result<TaskTemplate, String>
async fn apply_task_template(app, payload: ApplyTaskTemplatePayload) -> Result<Vec<Task>, String>
```

`ApplyTaskTemplatePayload`: `template_id`, `project_id`, `variable_sets: Vec<HashMap<String,String>>`, `assignee_id?`, `reviewer_id?`.

Variables: `{{ident}}` where `ident = [A-Za-z_][A-Za-z0-9_]*`. Only `title_template` + `description_template` are scanned.

## 3. Contracts

- List: `deleted_at IS NULL` AND (`project_id` IS NULL OR equals filter). Kanban passes the current project so the list is **global + that project**.
- Stored fields: name, optional notes, optional binding `project_id`, title/description templates, priority, `use_worktree`, `tags_json` (names), `subtasks_json` (`{title, sort_order}`).
- Apply lands tasks on payload `project_id` (current board). `use_worktree` comes from the template. Automation default uses the same `resolve_project_task_default_settings` as `create_task`.
- Apply: render **all** sets first, then one transaction (`insert_task_record` + automation state + find-or-create tag by `UNIQUE(project_id, name)` + subtasks status `todo`).
- Empty sets + no placeholders → one empty map. Limit **100** sets (same wording as `batch_update_tasks`).
- Activity: `task_template_created` / `task_template_applied` / `task_template_deleted`, plus `task_created` per new task.
- SSH: local SQLite only; no attachment sync.

## 4. Validation & Error Matrix

| Condition | Behavior |
|---|---|
| Missing `{{ident}}` key | `模板变量「x」未填写`; no writes |
| Rendered title empty | `任务标题不能为空`; no writes |
| `variable_sets.len() > 100` | `单次批量操作最多 100 个任务` |
| Automation default on, no reviewer | same string as `create_task` |
| Soft-deleted id | not found |
| Extra keys in a set | ignored |

## 5. Good / Base / Bad Cases

- **Good**: title `给 {{module}} 补 i18n`, two sets → two todos; tag name reused on the target project; subtask titles copied as todo.
- **Base**: no placeholders, empty sets → one task with the literal title.
- **Bad**: loop `create_task` per row (partial creates + SSH uploads); apply unsaved dialog edits instead of the stored template.

## 6. Tests Required

- `templates.rs` unit: extract unique order, ignore invalid idents, render, missing var.
- `app/tests/templates.rs`: soft-delete hidden; apply roundtrip tags+subtasks; second-row missing var writes nothing; over-limit; reviewer required; `from_task` copies tag names and subtask titles.
- Frontend: `getActivityActionLabel("task_template_*")` in both locales.

## 7. Wrong vs Correct

#### Wrong
Call `create_task` N times, then attach tags. A later row failure leaves tasks (and possible SSH uploads). Read variable names from dirty form fields that were never saved.

#### Correct
Validate/render every set, then one DB transaction. Dialog apply reads `selectedTemplate.title_template` / `description_template`. Close the manager after a successful apply and `fetchTasks` + refresh the tag map.
