use super::*;

use sqlx::FromRow;

use crate::db::models::{
    ApplyTaskTemplatePayload, CreateTaskTemplate, TaskTemplate, TaskTemplateSubtaskSpec,
    UpdateTaskTemplate,
};

const TEMPLATE_BATCH_LIMIT: usize = 100;

#[derive(Debug, Clone, FromRow)]
struct TaskTemplateRow {
    id: String,
    name: String,
    description: Option<String>,
    project_id: Option<String>,
    title_template: String,
    description_template: Option<String>,
    priority: String,
    use_worktree: bool,
    tags_json: String,
    subtasks_json: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

pub(crate) fn extract_template_variables(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '{' && index + 1 < chars.len() && chars[index + 1] == '{' {
            let start = index + 2;
            if start < chars.len() && is_ident_start(chars[start]) {
                let mut end = start + 1;
                while end < chars.len() && is_ident_continue(chars[end]) {
                    end += 1;
                }
                if end + 1 < chars.len() && chars[end] == '}' && chars[end + 1] == '}' {
                    let ident: String = chars[start..end].iter().collect();
                    if !result.iter().any(|existing| existing == &ident) {
                        result.push(ident);
                    }
                    index = end + 2;
                    continue;
                }
            }
        }
        index += 1;
    }
    result
}

pub(crate) fn collect_template_variables(
    title_template: &str,
    description_template: Option<&str>,
) -> Vec<String> {
    let mut variables = extract_template_variables(title_template);
    if let Some(description) = description_template {
        for variable in extract_template_variables(description) {
            if !variables.iter().any(|existing| existing == &variable) {
                variables.push(variable);
            }
        }
    }
    variables
}

pub(crate) fn render_template(
    text: &str,
    values: &HashMap<String, String>,
) -> Result<String, String> {
    for variable in extract_template_variables(text) {
        if !values.contains_key(&variable) {
            return Err(format!("模板变量「{}」未填写", variable));
        }
    }

    let chars: Vec<char> = text.chars().collect();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '{' && index + 1 < chars.len() && chars[index + 1] == '{' {
            let start = index + 2;
            if start < chars.len() && is_ident_start(chars[start]) {
                let mut end = start + 1;
                while end < chars.len() && is_ident_continue(chars[end]) {
                    end += 1;
                }
                if end + 1 < chars.len() && chars[end] == '}' && chars[end + 1] == '}' {
                    let ident: String = chars[start..end].iter().collect();
                    if let Some(value) = values.get(&ident) {
                        output.push_str(value);
                        index = end + 2;
                        continue;
                    }
                }
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    Ok(output)
}

fn normalize_tag_names(tags: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for tag in tags {
        let name = tag.trim().to_string();
        if name.is_empty() {
            continue;
        }
        if !normalized.iter().any(|existing| existing == &name) {
            normalized.push(name);
        }
    }
    normalized
}

fn normalize_subtask_specs(subtasks: Vec<TaskTemplateSubtaskSpec>) -> Vec<TaskTemplateSubtaskSpec> {
    subtasks
        .into_iter()
        .filter_map(|subtask| {
            let title = subtask.title.trim().to_string();
            if title.is_empty() {
                None
            } else {
                Some(TaskTemplateSubtaskSpec {
                    title,
                    sort_order: subtask.sort_order,
                })
            }
        })
        .collect()
}

fn parse_tags_json(raw: &str) -> Result<Vec<String>, String> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let parsed: Vec<String> =
        serde_json::from_str(raw).map_err(|error| format!("模板标签数据损坏: {}", error))?;
    Ok(normalize_tag_names(parsed))
}

fn parse_subtasks_json(raw: &str) -> Result<Vec<TaskTemplateSubtaskSpec>, String> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let parsed: Vec<TaskTemplateSubtaskSpec> =
        serde_json::from_str(raw).map_err(|error| format!("模板子任务数据损坏: {}", error))?;
    Ok(normalize_subtask_specs(parsed))
}

fn serialize_tags_json(tags: Vec<String>) -> Result<String, String> {
    serde_json::to_string(&normalize_tag_names(tags))
        .map_err(|error| format!("Failed to serialize template tags: {}", error))
}

fn serialize_subtasks_json(subtasks: Vec<TaskTemplateSubtaskSpec>) -> Result<String, String> {
    serde_json::to_string(&normalize_subtask_specs(subtasks))
        .map_err(|error| format!("Failed to serialize template subtasks: {}", error))
}

fn template_from_row(row: TaskTemplateRow) -> Result<TaskTemplate, String> {
    Ok(TaskTemplate {
        id: row.id,
        name: row.name,
        description: row.description,
        project_id: row.project_id,
        title_template: row.title_template,
        description_template: row.description_template,
        priority: row.priority,
        use_worktree: row.use_worktree,
        tags: parse_tags_json(&row.tags_json)?,
        subtasks: parse_subtasks_json(&row.subtasks_json)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
    })
}

async fn fetch_template_row(pool: &SqlitePool, id: &str) -> Result<TaskTemplateRow, String> {
    sqlx::query_as::<_, TaskTemplateRow>(
        "SELECT * FROM task_templates WHERE id = $1 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|_| "任务模板不存在".to_string())
}

async fn fetch_template_by_id(pool: &SqlitePool, id: &str) -> Result<TaskTemplate, String> {
    template_from_row(fetch_template_row(pool, id).await?)
}

async fn list_task_tag_names(pool: &SqlitePool, task_id: &str) -> Result<Vec<String>, String> {
    let names: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT tags.name
        FROM tags
        INNER JOIN task_tags ON task_tags.tag_id = tags.id
        WHERE task_tags.task_id = $1
        ORDER BY tags.name COLLATE NOCASE, tags.created_at
        "#,
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to list task tags: {}", error))?;
    Ok(normalize_tag_names(names))
}

fn normalize_template_name(name: &str) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("模板名称不能为空".to_string());
    }
    Ok(name)
}

fn normalize_title_template(title_template: &str) -> Result<String, String> {
    let title_template = title_template.trim().to_string();
    if title_template.is_empty() {
        return Err("任务标题模板不能为空".to_string());
    }
    Ok(title_template)
}

fn normalize_priority(priority: Option<&str>) -> String {
    normalize_optional_text(priority).unwrap_or_else(|| "medium".to_string())
}

async fn ensure_optional_project_exists(
    pool: &SqlitePool,
    project_id: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(project_id) = normalize_optional_text(project_id) else {
        return Ok(None);
    };
    ensure_project_exists(pool, &project_id).await?;
    Ok(Some(project_id))
}

pub(crate) fn resolve_apply_variable_sets(
    variable_sets: Vec<HashMap<String, String>>,
) -> Result<Vec<HashMap<String, String>>, String> {
    if variable_sets.len() > TEMPLATE_BATCH_LIMIT {
        return Err("单次批量操作最多 100 个任务".to_string());
    }
    if variable_sets.is_empty() {
        return Ok(vec![HashMap::new()]);
    }
    Ok(variable_sets)
}

fn render_task_fields(
    title_template: &str,
    description_template: Option<&str>,
    values: &HashMap<String, String>,
) -> Result<(String, Option<String>), String> {
    for variable in collect_template_variables(title_template, description_template) {
        if !values.contains_key(&variable) {
            return Err(format!("模板变量「{}」未填写", variable));
        }
    }
    let title = render_template(title_template, values)?.trim().to_string();
    if title.is_empty() {
        return Err("任务标题不能为空".to_string());
    }
    let description = match description_template {
        Some(template) => {
            normalize_optional_text(Some(render_template(template, values)?.as_str()))
        }
        None => None,
    };
    Ok((title, description))
}

pub(crate) async fn list_task_templates_with_pool(
    pool: &SqlitePool,
    project_id: Option<&str>,
) -> Result<Vec<TaskTemplate>, String> {
    let rows = if let Some(project_id) = project_id {
        sqlx::query_as::<_, TaskTemplateRow>(
            r#"
            SELECT * FROM task_templates
            WHERE deleted_at IS NULL
              AND (project_id IS NULL OR project_id = $1)
            ORDER BY updated_at DESC, created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, TaskTemplateRow>(
            r#"
            SELECT * FROM task_templates
            WHERE deleted_at IS NULL
              AND project_id IS NULL
            ORDER BY updated_at DESC, created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
    }
    .map_err(|error| format!("Failed to list task templates: {}", error))?;

    rows.into_iter().map(template_from_row).collect()
}

pub(crate) async fn create_task_template_with_pool(
    pool: &SqlitePool,
    payload: CreateTaskTemplate,
) -> Result<TaskTemplate, String> {
    let name = normalize_template_name(&payload.name)?;
    let title_template = normalize_title_template(&payload.title_template)?;
    let project_id = ensure_optional_project_exists(pool, payload.project_id.as_deref()).await?;
    let description = normalize_optional_text(payload.description.as_deref());
    let description_template = normalize_optional_text(payload.description_template.as_deref());
    let priority = normalize_priority(payload.priority.as_deref());
    let use_worktree = payload.use_worktree.unwrap_or(false);
    let tags_json = serialize_tags_json(payload.tags)?;
    let subtasks_json = serialize_subtasks_json(payload.subtasks)?;
    let id = new_id();
    let now = now_sqlite();

    sqlx::query(
        r#"
        INSERT INTO task_templates (
            id, name, description, project_id, title_template, description_template,
            priority, use_worktree, tags_json, subtasks_json, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(&id)
    .bind(&name)
    .bind(&description)
    .bind(&project_id)
    .bind(&title_template)
    .bind(&description_template)
    .bind(&priority)
    .bind(use_worktree)
    .bind(&tags_json)
    .bind(&subtasks_json)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to create task template: {}", error))?;

    insert_activity_log(
        pool,
        "task_template_created",
        &name,
        None,
        None,
        project_id.as_deref(),
    )
    .await?;

    fetch_template_by_id(pool, &id).await
}

pub(crate) async fn update_task_template_with_pool(
    pool: &SqlitePool,
    id: &str,
    updates: UpdateTaskTemplate,
) -> Result<TaskTemplate, String> {
    let current = fetch_template_row(pool, id).await?;
    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE task_templates SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(name) = updates.name {
        separated
            .push("name = ")
            .push_bind_unseparated(normalize_template_name(&name)?);
        touched = true;
    }
    if let Some(description) = updates.description {
        separated.push("description = ").push_bind_unseparated(
            description.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(project_id) = updates.project_id {
        let project_id = ensure_optional_project_exists(pool, project_id.as_deref()).await?;
        separated
            .push("project_id = ")
            .push_bind_unseparated(project_id);
        touched = true;
    }
    if let Some(title_template) = updates.title_template {
        separated
            .push("title_template = ")
            .push_bind_unseparated(normalize_title_template(&title_template)?);
        touched = true;
    }
    if let Some(description_template) = updates.description_template {
        separated
            .push("description_template = ")
            .push_bind_unseparated(
                description_template.and_then(|value| normalize_optional_text(Some(&value))),
            );
        touched = true;
    }
    if let Some(priority) = updates.priority {
        separated
            .push("priority = ")
            .push_bind_unseparated(normalize_priority(Some(&priority)));
        touched = true;
    }
    if let Some(use_worktree) = updates.use_worktree {
        separated
            .push("use_worktree = ")
            .push_bind_unseparated(use_worktree);
        touched = true;
    }
    if let Some(tags) = updates.tags {
        separated
            .push("tags_json = ")
            .push_bind_unseparated(serialize_tags_json(tags)?);
        touched = true;
    }
    if let Some(subtasks) = updates.subtasks {
        separated
            .push("subtasks_json = ")
            .push_bind_unseparated(serialize_subtasks_json(subtasks)?);
        touched = true;
    }

    if !touched {
        return template_from_row(current);
    }

    separated
        .push("updated_at = ")
        .push_bind_unseparated(now_sqlite());
    builder.push(" WHERE id = ").push_bind(id);
    builder
        .build()
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to update task template: {}", error))?;

    fetch_template_by_id(pool, id).await
}

pub(crate) async fn delete_task_template_with_pool(
    pool: &SqlitePool,
    id: &str,
) -> Result<(), String> {
    let current = fetch_template_by_id(pool, id).await?;
    let now = now_sqlite();
    sqlx::query(
        "UPDATE task_templates SET deleted_at = $1, updated_at = $1 WHERE id = $2 AND deleted_at IS NULL",
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to delete task template: {}", error))?;

    insert_activity_log(
        pool,
        "task_template_deleted",
        &current.name,
        None,
        None,
        current.project_id.as_deref(),
    )
    .await?;

    Ok(())
}

pub(crate) async fn create_task_template_from_task_with_pool(
    pool: &SqlitePool,
    task_id: &str,
    name: Option<&str>,
) -> Result<TaskTemplate, String> {
    let task = fetch_task_by_id(pool, task_id).await?;
    let tags = list_task_tag_names(pool, task_id).await?;
    let subtasks = fetch_task_subtasks(pool, task_id)
        .await?
        .into_iter()
        .map(|subtask| TaskTemplateSubtaskSpec {
            title: subtask.title,
            sort_order: subtask.sort_order,
        })
        .collect();

    let template_name = name
        .and_then(|value| normalize_optional_text(Some(value)))
        .unwrap_or_else(|| task.title.clone());

    create_task_template_with_pool(
        pool,
        CreateTaskTemplate {
            name: template_name,
            description: None,
            project_id: Some(task.project_id),
            title_template: task.title,
            description_template: task.description,
            priority: Some(task.priority),
            use_worktree: Some(task.use_worktree),
            tags,
            subtasks,
        },
    )
    .await
}

async fn find_or_create_tag_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    project_id: &str,
    name: &str,
    cache: &mut HashMap<String, String>,
) -> Result<String, String> {
    if let Some(existing) = cache.get(name) {
        return Ok(existing.clone());
    }

    let existing_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM tags WHERE project_id = $1 AND name = $2 LIMIT 1")
            .bind(project_id)
            .bind(name)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|error| format!("Failed to look up tag: {}", error))?;

    let tag_id = if let Some(existing_id) = existing_id {
        existing_id
    } else {
        let id = new_id();
        sqlx::query(
            "INSERT INTO tags (id, project_id, name, color, created_at) VALUES ($1, $2, $3, NULL, $4)",
        )
        .bind(&id)
        .bind(project_id)
        .bind(name)
        .bind(now_sqlite())
        .execute(&mut **tx)
        .await
        .map_err(|error| format!("Failed to create tag: {}", error))?;
        id
    };

    cache.insert(name.to_string(), tag_id.clone());
    Ok(tag_id)
}

pub(crate) async fn apply_task_template_with_pool(
    pool: &SqlitePool,
    payload: ApplyTaskTemplatePayload,
    automation_default_enabled: bool,
) -> Result<Vec<Task>, String> {
    let template = fetch_template_by_id(pool, &payload.template_id).await?;
    ensure_project_exists(pool, &payload.project_id).await?;
    validate_assignee_for_project(pool, payload.assignee_id.as_deref(), &payload.project_id)
        .await?;
    validate_reviewer_for_project(pool, payload.reviewer_id.as_deref(), &payload.project_id)
        .await?;

    let assignee_id = normalize_optional_text(payload.assignee_id.as_deref());
    let reviewer_id = normalize_optional_text(payload.reviewer_id.as_deref());
    if automation_default_enabled && reviewer_id.is_none() {
        return Err("当前已开启“新建任务默认自动质控”，请先指定审查员。".to_string());
    }

    let variable_sets = resolve_apply_variable_sets(payload.variable_sets)?;
    let mut drafts = Vec::with_capacity(variable_sets.len());
    for values in &variable_sets {
        drafts.push(render_task_fields(
            &template.title_template,
            template.description_template.as_deref(),
            values,
        )?);
    }

    let automation_mode = if automation_default_enabled {
        Some(TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1.to_string())
    } else {
        None
    };

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start template apply transaction: {}", error))?;

    let mut created_ids = Vec::with_capacity(drafts.len());
    let mut tag_id_by_name: HashMap<String, String> = HashMap::new();
    let now = now_sqlite();

    for (title, description) in drafts {
        let task = Task {
            id: new_id(),
            title,
            description,
            status: "todo".to_string(),
            priority: template.priority.clone(),
            project_id: payload.project_id.clone(),
            use_worktree: template.use_worktree,
            assignee_id: assignee_id.clone(),
            reviewer_id: reviewer_id.clone(),
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: automation_mode.clone(),
            last_codex_session_id: None,
            last_review_session_id: None,
            time_started_at: None,
            time_spent_seconds: 0,
            completed_at: None,
            deleted_at: None,
            due_date: None,
            blocked_reason: None,
            milestone_id: None,
            acceptance_checklist: None,
            last_acceptance_status: None,
            mcp_server_ids: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        insert_task_record(&mut tx, &task).await?;

        if task.automation_mode.is_some() {
            sqlx::query(
                r#"
                INSERT INTO task_automation_state (
                    task_id,
                    phase,
                    round_count,
                    consumed_session_id,
                    last_trigger_session_id,
                    pending_action,
                    pending_round_count,
                    last_error,
                    last_verdict_json,
                    updated_at
                ) VALUES ($1, 'idle', 0, NULL, NULL, NULL, NULL, NULL, NULL, $2)
                "#,
            )
            .bind(&task.id)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|error| format!("Failed to initialize task automation state: {}", error))?;
        }

        for tag_name in &template.tags {
            let tag_id =
                find_or_create_tag_id(&mut tx, &payload.project_id, tag_name, &mut tag_id_by_name)
                    .await?;
            sqlx::query("INSERT INTO task_tags (task_id, tag_id) VALUES ($1, $2)")
                .bind(&task.id)
                .bind(&tag_id)
                .execute(&mut *tx)
                .await
                .map_err(|error| format!("Failed to set task tag: {}", error))?;
        }

        for subtask in &template.subtasks {
            sqlx::query(
                "INSERT INTO subtasks (id, task_id, title, status, sort_order, created_at, updated_at) VALUES ($1, $2, $3, 'todo', $4, $5, $6)",
            )
            .bind(new_id())
            .bind(&task.id)
            .bind(&subtask.title)
            .bind(subtask.sort_order)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|error| format!("Failed to create subtask: {}", error))?;
        }

        created_ids.push(task.id);
    }

    tx.commit()
        .await
        .map_err(|error| format!("Failed to commit template apply: {}", error))?;

    let mut created = Vec::with_capacity(created_ids.len());
    for task_id in &created_ids {
        let task = fetch_task_by_id(pool, task_id).await?;
        insert_activity_log(
            pool,
            "task_created",
            &task.title,
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
        created.push(task);
    }

    insert_activity_log(
        pool,
        "task_template_applied",
        &format!("{}（生成 {} 个任务）", template.name, created.len()),
        None,
        None,
        Some(&payload.project_id),
    )
    .await?;

    Ok(created)
}

#[tauri::command]
pub async fn list_task_templates<R: Runtime>(
    app: AppHandle<R>,
    project_id: Option<String>,
) -> Result<Vec<TaskTemplate>, String> {
    let pool = sqlite_pool(&app).await?;
    list_task_templates_with_pool(&pool, project_id.as_deref()).await
}

#[tauri::command]
pub async fn create_task_template<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateTaskTemplate,
) -> Result<TaskTemplate, String> {
    let pool = sqlite_pool(&app).await?;
    create_task_template_with_pool(&pool, payload).await
}

#[tauri::command]
pub async fn update_task_template<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateTaskTemplate,
) -> Result<TaskTemplate, String> {
    let pool = sqlite_pool(&app).await?;
    update_task_template_with_pool(&pool, &id, updates).await
}

#[tauri::command]
pub async fn delete_task_template<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    delete_task_template_with_pool(&pool, &id).await
}

#[tauri::command]
pub async fn create_task_template_from_task<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    name: Option<String>,
) -> Result<TaskTemplate, String> {
    let pool = sqlite_pool(&app).await?;
    create_task_template_from_task_with_pool(&pool, &task_id, name.as_deref()).await
}

#[tauri::command]
pub async fn apply_task_template<R: Runtime>(
    app: AppHandle<R>,
    payload: ApplyTaskTemplatePayload,
) -> Result<Vec<Task>, String> {
    let pool = sqlite_pool(&app).await?;
    let project = fetch_project_by_id(&pool, &payload.project_id).await?;
    let settings = resolve_project_task_default_settings(
        &project.project_type,
        project.ssh_config_id.as_deref(),
        || load_codex_settings(&app),
        |ssh_config_id| load_remote_codex_settings(&app, ssh_config_id),
    );
    let automation_default_enabled = settings
        .as_ref()
        .is_some_and(|settings| settings.task_automation_default_enabled);
    apply_task_template_with_pool(&pool, payload, automation_default_enabled).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_template_variables_keeps_unique_order() {
        let vars = extract_template_variables("给 {{module}} 补 {{module}} 与 {{name}}");
        assert_eq!(vars, vec!["module", "name"]);
    }

    #[test]
    fn extract_template_variables_ignores_invalid_idents() {
        let vars = extract_template_variables("{{1bad}} {{中文}} {{ok_1}} {{_x}}");
        assert_eq!(vars, vec!["ok_1", "_x"]);
    }

    #[test]
    fn render_template_replaces_known_keys() {
        let mut values = HashMap::new();
        values.insert("module".to_string(), "auth".to_string());
        let rendered = render_template("给 {{module}} 补 i18n", &values).expect("render");
        assert_eq!(rendered, "给 auth 补 i18n");
    }

    #[test]
    fn render_template_errors_on_missing_variable() {
        let values = HashMap::new();
        let error = render_template("给 {{module}} 补 i18n", &values).expect_err("missing");
        assert_eq!(error, "模板变量「module」未填写");
    }

    #[test]
    fn resolve_apply_variable_sets_rejects_over_limit() {
        let sets = vec![HashMap::new(); 101];
        let error = resolve_apply_variable_sets(sets).expect_err("limit");
        assert_eq!(error, "单次批量操作最多 100 个任务");
    }

    #[test]
    fn resolve_apply_variable_sets_uses_empty_map_when_none() {
        let sets = resolve_apply_variable_sets(Vec::new()).expect("empty");
        assert_eq!(sets.len(), 1);
        assert!(sets[0].is_empty());
    }

    #[test]
    fn render_task_fields_rejects_blank_title() {
        let mut values = HashMap::new();
        values.insert("module".to_string(), "   ".to_string());
        let error = render_task_fields("{{module}}", None, &values).expect_err("blank");
        assert_eq!(error, "任务标题不能为空");
    }
}
