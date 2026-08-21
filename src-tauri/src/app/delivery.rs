use super::*;

use crate::db::models::{
    AddTaskDependencyPayload, CreateMilestone, CreateTag, Milestone, SetTaskTagsPayload, Tag,
    TaskDependency, UpdateMilestone,
};

#[tauri::command]
pub async fn list_milestones<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<Vec<Milestone>, String> {
    let pool = sqlite_pool(&app).await?;
    ensure_project_exists(&pool, &project_id).await?;

    sqlx::query_as::<_, Milestone>(
        "SELECT * FROM milestones WHERE project_id = $1 ORDER BY due_date IS NULL, due_date, created_at",
    )
    .bind(&project_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to list milestones: {}", error))
}

#[tauri::command]
pub async fn create_milestone<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateMilestone,
) -> Result<Milestone, String> {
    let pool = sqlite_pool(&app).await?;
    ensure_project_exists(&pool, &payload.project_id).await?;

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err("里程碑名称不能为空".to_string());
    }

    let id = new_id();
    let due_date = normalize_optional_text(payload.due_date.as_deref());
    let description = normalize_optional_text(payload.description.as_deref());
    let now = now_sqlite();

    sqlx::query(
        "INSERT INTO milestones (id, project_id, name, due_date, description, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&id)
    .bind(&payload.project_id)
    .bind(&name)
    .bind(&due_date)
    .bind(&description)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to create milestone: {}", error))?;

    insert_activity_log(
        &pool,
        "milestone_created",
        &format!(
            "{}{}",
            name,
            due_date
                .as_deref()
                .map(|value| format!("（截止日期：{}）", value))
                .unwrap_or_default()
        ),
        None,
        None,
        Some(&payload.project_id),
    )
    .await?;

    sqlx::query_as::<_, Milestone>("SELECT * FROM milestones WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch created milestone: {}", error))
}

#[tauri::command]
pub async fn update_milestone<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateMilestone,
) -> Result<Milestone, String> {
    let pool = sqlite_pool(&app).await?;
    let current = sqlx::query_as::<_, Milestone>("SELECT * FROM milestones WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Milestone {} not found: {}", id, error))?;

    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE milestones SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(name) = updates.name {
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return Err("里程碑名称不能为空".to_string());
        }
        separated.push("name = ").push_bind_unseparated(trimmed);
        touched = true;
    }
    if let Some(due_date) = updates.due_date {
        separated.push("due_date = ").push_bind_unseparated(
            due_date.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(description) = updates.description {
        separated.push("description = ").push_bind_unseparated(
            description.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }

    if !touched {
        return Ok(current);
    }

    separated
        .push("updated_at = ")
        .push_bind_unseparated(now_sqlite());
    builder.push(" WHERE id = ").push_bind(&id);
    builder
        .build()
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update milestone: {}", error))?;

    sqlx::query_as::<_, Milestone>("SELECT * FROM milestones WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch updated milestone: {}", error))
}

#[tauri::command]
pub async fn delete_milestone<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let milestone =
        sqlx::query_as::<_, Milestone>("SELECT * FROM milestones WHERE id = $1 LIMIT 1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .map_err(|error| format!("Milestone {} not found: {}", id, error))?;

    sqlx::query("UPDATE tasks SET milestone_id = NULL WHERE milestone_id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to clear task milestone references: {}", error))?;

    sqlx::query("DELETE FROM milestones WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to delete milestone: {}", error))?;

    insert_activity_log(
        &pool,
        "milestone_deleted",
        &milestone.name,
        None,
        None,
        Some(&milestone.project_id),
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn list_tags<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<Vec<Tag>, String> {
    let pool = sqlite_pool(&app).await?;
    ensure_project_exists(&pool, &project_id).await?;

    sqlx::query_as::<_, Tag>(
        "SELECT * FROM tags WHERE project_id = $1 ORDER BY name COLLATE NOCASE, created_at",
    )
    .bind(&project_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to list tags: {}", error))
}

#[tauri::command]
pub async fn create_tag<R: Runtime>(app: AppHandle<R>, payload: CreateTag) -> Result<Tag, String> {
    let pool = sqlite_pool(&app).await?;
    ensure_project_exists(&pool, &payload.project_id).await?;

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err("标签名称不能为空".to_string());
    }

    let id = new_id();
    let color = normalize_optional_text(payload.color.as_deref());

    sqlx::query(
        "INSERT INTO tags (id, project_id, name, color, created_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(&payload.project_id)
    .bind(&name)
    .bind(&color)
    .bind(now_sqlite())
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to create tag: {}", error))?;

    sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch created tag: {}", error))
}

#[tauri::command]
pub async fn delete_tag<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let tag = sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Tag {} not found: {}", id, error))?;

    sqlx::query("DELETE FROM tags WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to delete tag: {}", error))?;

    insert_activity_log(
        &pool,
        "tag_deleted",
        &tag.name,
        None,
        None,
        Some(&tag.project_id),
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn list_task_tags<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Vec<Tag>, String> {
    let pool = sqlite_pool(&app).await?;
    let _task = fetch_task_by_id(&pool, &task_id).await?;

    sqlx::query_as::<_, Tag>(
        r#"
        SELECT tags.*
        FROM tags
        INNER JOIN task_tags ON task_tags.tag_id = tags.id
        WHERE task_tags.task_id = $1
        ORDER BY tags.name COLLATE NOCASE, tags.created_at
        "#,
    )
    .bind(&task_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to list task tags: {}", error))
}

#[tauri::command]
pub async fn set_task_tags<R: Runtime>(
    app: AppHandle<R>,
    payload: SetTaskTagsPayload,
) -> Result<Vec<Tag>, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;

    let mut unique_tag_ids = Vec::new();
    for tag_id in &payload.tag_ids {
        let trimmed = tag_id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !unique_tag_ids.iter().any(|existing| existing == trimmed) {
            unique_tag_ids.push(trimmed.to_string());
        }
    }

    for tag_id in &unique_tag_ids {
        let tag = sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE id = $1 LIMIT 1")
            .bind(tag_id)
            .fetch_one(&pool)
            .await
            .map_err(|error| format!("Tag {} not found: {}", tag_id, error))?;
        if tag.project_id != task.project_id {
            return Err(format!("标签 {} 不属于当前任务所在项目", tag.name));
        }
    }

    let previous_tag_ids: Vec<String> =
        sqlx::query_scalar("SELECT tag_id FROM task_tags WHERE task_id = $1 ORDER BY tag_id")
            .bind(&payload.task_id)
            .fetch_all(&pool)
            .await
            .map_err(|error| format!("Failed to load existing task tags: {}", error))?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start task tags transaction: {}", error))?;

    sqlx::query("DELETE FROM task_tags WHERE task_id = $1")
        .bind(&payload.task_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to clear task tags: {}", error))?;

    for tag_id in &unique_tag_ids {
        sqlx::query("INSERT INTO task_tags (task_id, tag_id) VALUES ($1, $2)")
            .bind(&payload.task_id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| format!("Failed to set task tag: {}", error))?;
    }

    tx.commit()
        .await
        .map_err(|error| format!("Failed to commit task tags: {}", error))?;

    let added_tag_ids: Vec<&String> = unique_tag_ids
        .iter()
        .filter(|tag_id| !previous_tag_ids.iter().any(|existing| existing == *tag_id))
        .collect();

    for tag_id in added_tag_ids {
        let tag_name = sqlx::query_scalar::<_, String>("SELECT name FROM tags WHERE id = $1")
            .bind(tag_id)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|_| tag_id.clone());
        insert_activity_log(
            &pool,
            "task_tag_added",
            &format!("{}（标签：{}）", task.title, tag_name),
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    list_task_tags(app, payload.task_id).await
}

#[tauri::command]
pub async fn list_task_dependencies<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Vec<TaskDependency>, String> {
    let pool = sqlite_pool(&app).await?;
    let _task = fetch_task_by_id(&pool, &task_id).await?;

    sqlx::query_as::<_, TaskDependency>(
        "SELECT * FROM task_dependencies WHERE task_id = $1 ORDER BY created_at, id",
    )
    .bind(&task_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to list task dependencies: {}", error))
}

/// Returns true if adding edge `task_id -> depends_on_task_id` would create a cycle.
/// Edge meaning: task depends on depends_on (task waits for depends_on).
pub(crate) fn dependency_would_cycle(
    edges: &[(String, String)],
    task_id: &str,
    depends_on_task_id: &str,
) -> bool {
    if task_id == depends_on_task_id {
        return true;
    }

    // From depends_on, follow "depends_on" edges. If we reach task_id, cycle.
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    for (from, to) in edges {
        adjacency.entry(from.clone()).or_default().push(to.clone());
    }
    adjacency
        .entry(task_id.to_string())
        .or_default()
        .push(depends_on_task_id.to_string());

    let mut stack = vec![depends_on_task_id.to_string()];
    let mut visited = HashSet::new();
    while let Some(current) = stack.pop() {
        if current == task_id {
            return true;
        }
        if !visited.insert(current.clone()) {
            continue;
        }
        if let Some(nexts) = adjacency.get(&current) {
            for next in nexts {
                stack.push(next.clone());
            }
        }
    }
    false
}

async fn project_dependency_edges(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<Vec<(String, String)>, String> {
    sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT d.task_id, d.depends_on_task_id
        FROM task_dependencies d
        INNER JOIN tasks t ON t.id = d.task_id
        WHERE t.project_id = $1
          AND t.deleted_at IS NULL
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to load task dependencies: {}", error))
}

#[tauri::command]
pub async fn add_task_dependency<R: Runtime>(
    app: AppHandle<R>,
    payload: AddTaskDependencyPayload,
) -> Result<TaskDependency, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    let depends_on = fetch_task_by_id(&pool, &payload.depends_on_task_id).await?;

    if task.id == depends_on.id {
        return Err("任务不能依赖自身".to_string());
    }
    if task.project_id != depends_on.project_id {
        return Err("依赖任务必须属于同一项目".to_string());
    }

    let edges = project_dependency_edges(&pool, &task.project_id).await?;
    if dependency_would_cycle(&edges, &task.id, &depends_on.id) {
        return Err("添加该依赖会形成循环依赖，请调整依赖关系".to_string());
    }

    let id = new_id();
    sqlx::query(
        "INSERT INTO task_dependencies (id, task_id, depends_on_task_id, created_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(&id)
    .bind(&payload.task_id)
    .bind(&payload.depends_on_task_id)
    .bind(now_sqlite())
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to add task dependency: {}", error))?;

    insert_activity_log(
        &pool,
        "task_dependency_added",
        &format!("{} 依赖 {}", task.title, depends_on.title),
        None,
        Some(&task.id),
        Some(&task.project_id),
    )
    .await?;

    sqlx::query_as::<_, TaskDependency>("SELECT * FROM task_dependencies WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch created task dependency: {}", error))
}

#[tauri::command]
pub async fn remove_task_dependency<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let dependency = sqlx::query_as::<_, TaskDependency>(
        "SELECT * FROM task_dependencies WHERE id = $1 LIMIT 1",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .map_err(|error| format!("Task dependency {} not found: {}", id, error))?;

    let task = fetch_task_by_id(&pool, &dependency.task_id).await?;

    sqlx::query("DELETE FROM task_dependencies WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to remove task dependency: {}", error))?;

    insert_activity_log(
        &pool,
        "task_dependency_removed",
        &task.title,
        None,
        Some(&task.id),
        Some(&task.project_id),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::dependency_would_cycle;

    #[test]
    fn dependency_cycle_detects_direct_loop() {
        let edges = vec![("b".to_string(), "a".to_string())];
        assert!(dependency_would_cycle(&edges, "a", "b"));
    }

    #[test]
    fn dependency_cycle_detects_transitive_loop() {
        let edges = vec![
            ("b".to_string(), "c".to_string()),
            ("c".to_string(), "a".to_string()),
        ];
        assert!(dependency_would_cycle(&edges, "a", "b"));
    }

    #[test]
    fn dependency_cycle_allows_dag() {
        let edges = vec![
            ("a".to_string(), "b".to_string()),
            ("b".to_string(), "c".to_string()),
        ];
        assert!(!dependency_would_cycle(&edges, "d", "a"));
        assert!(!dependency_would_cycle(&edges, "a", "x"));
    }

    #[test]
    fn dependency_cycle_rejects_self() {
        assert!(dependency_would_cycle(&[], "a", "a"));
    }
}
