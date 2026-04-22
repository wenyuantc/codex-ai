use super::*;

async fn validate_project_storage_fields(
    pool: &SqlitePool,
    project_type: &str,
    repo_path: Option<&str>,
    ssh_config_id: Option<&str>,
    remote_repo_path: Option<&str>,
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    match project_type {
        PROJECT_TYPE_LOCAL => Ok((validate_project_repo_path(repo_path)?, None, None)),
        PROJECT_TYPE_SSH => {
            let ssh_config_id = normalize_optional_text(ssh_config_id)
                .ok_or_else(|| "SSH 项目必须绑定 SSH 配置".to_string())?;
            ensure_ssh_config_exists(pool, &ssh_config_id).await?;
            let remote_repo_path = validate_remote_repo_path(remote_repo_path)?
                .ok_or_else(|| "SSH 项目必须提供远程仓库目录".to_string())?;
            Ok((None, Some(ssh_config_id), Some(remote_repo_path)))
        }
        other => Err(format!("不支持的项目类型: {other}")),
    }
}

pub(crate) async fn fetch_project_by_id(pool: &SqlitePool, id: &str) -> Result<Project, String> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Project {} not found: {}", id, error))
}

pub(crate) async fn ensure_project_exists(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<(), String> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Failed to verify project: {}", error))
        .and_then(|count| {
            if count > 0 {
                Ok(())
            } else {
                Err(format!("Project {} does not exist", project_id))
            }
        })
}

#[tauri::command]
pub async fn create_project<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateProject,
) -> Result<Project, String> {
    let pool = sqlite_pool(&app).await?;
    let project_type = normalize_project_type(payload.project_type.as_deref())?;
    let (repo_path, ssh_config_id, remote_repo_path) = validate_project_storage_fields(
        &pool,
        &project_type,
        payload.repo_path.as_deref(),
        payload.ssh_config_id.as_deref(),
        payload.remote_repo_path.as_deref(),
    )
    .await?;
    let project = Project {
        id: new_id(),
        name: payload.name.trim().to_string(),
        description: normalize_optional_text(payload.description.as_deref()),
        status: "active".to_string(),
        repo_path,
        project_type,
        ssh_config_id,
        remote_repo_path,
        created_at: now_sqlite(),
        updated_at: now_sqlite(),
    };

    if project.name.is_empty() {
        return Err("项目名称不能为空".to_string());
    }

    sqlx::query(
        "INSERT INTO projects (id, name, description, status, repo_path, project_type, ssh_config_id, remote_repo_path, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(&project.id)
    .bind(&project.name)
    .bind(&project.description)
    .bind(&project.status)
    .bind(&project.repo_path)
    .bind(&project.project_type)
    .bind(&project.ssh_config_id)
    .bind(&project.remote_repo_path)
    .bind(&project.created_at)
    .bind(&project.updated_at)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to create project: {}", error))?;

    fetch_project_by_id(&pool, &project.id).await
}

#[tauri::command]
pub async fn update_project<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateProject,
) -> Result<Project, String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_project_by_id(&pool, &id).await?;
    let resolved_project_type = normalize_project_type(
        updates
            .project_type
            .as_deref()
            .or(Some(&current.project_type)),
    )?;
    let resolved_repo_path = match updates.repo_path.as_ref() {
        Some(Some(value)) => Some(value.as_str()),
        Some(None) => None,
        None => current.repo_path.as_deref(),
    };
    let resolved_ssh_config_id = match updates.ssh_config_id.as_ref() {
        Some(Some(value)) => Some(value.as_str()),
        Some(None) => None,
        None => current.ssh_config_id.as_deref(),
    };
    let resolved_remote_repo_path = match updates.remote_repo_path.as_ref() {
        Some(Some(value)) => Some(value.as_str()),
        Some(None) => None,
        None => current.remote_repo_path.as_deref(),
    };
    let (validated_repo_path, validated_ssh_config_id, validated_remote_repo_path) =
        validate_project_storage_fields(
            &pool,
            &resolved_project_type,
            resolved_repo_path,
            resolved_ssh_config_id,
            resolved_remote_repo_path,
        )
        .await?;
    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE projects SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(name) = updates.name {
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return Err("项目名称不能为空".to_string());
        }
        separated.push("name = ").push_bind_unseparated(trimmed);
        touched = true;
    }
    if let Some(description) = updates.description {
        separated.push("description = ").push_bind_unseparated(
            description.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(status) = updates.status {
        separated.push("status = ").push_bind_unseparated(status);
        touched = true;
    }
    if updates.project_type.is_some() {
        separated
            .push("project_type = ")
            .push_bind_unseparated(resolved_project_type.clone());
        touched = true;
    }
    if let Some(repo_path) = updates.repo_path {
        separated
            .push("repo_path = ")
            .push_bind_unseparated(match repo_path {
                Some(_) => validated_repo_path.clone(),
                None => None,
            });
        touched = true;
    }
    if updates.ssh_config_id.is_some() || updates.project_type.is_some() {
        separated
            .push("ssh_config_id = ")
            .push_bind_unseparated(validated_ssh_config_id.clone());
        touched = true;
    }
    if updates.remote_repo_path.is_some() || updates.project_type.is_some() {
        separated
            .push("remote_repo_path = ")
            .push_bind_unseparated(validated_remote_repo_path.clone());
        touched = true;
    }

    if !touched {
        return Ok(current);
    }

    builder.push(" WHERE id = ").push_bind(&id);
    builder
        .build()
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update project: {}", error))?;

    fetch_project_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn delete_project<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start project transaction: {}", error))?;

    sqlx::query(
        "DELETE FROM activity_logs WHERE project_id = $1 OR task_id IN (SELECT id FROM tasks WHERE project_id = $1)",
    )
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(|error| format!("Failed to delete project activity logs: {}", error))?;
    sqlx::query("UPDATE employees SET project_id = NULL WHERE project_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to clear employee project ownership: {}", error))?;
    sqlx::query("DELETE FROM tasks WHERE project_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete project tasks: {}", error))?;
    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete project: {}", error))?;

    tx.commit()
        .await
        .map_err(|error| format!("Failed to commit project delete: {}", error))?;

    Ok(())
}
