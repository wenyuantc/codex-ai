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
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 AND deleted_at IS NULL LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Project {} not found: {}", id, error))
}

pub(crate) async fn fetch_any_project_by_id(pool: &SqlitePool, id: &str) -> Result<Project, String> {
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
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = $1 AND deleted_at IS NULL")
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
        deleted_at: None,
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
    let project = fetch_project_by_id(&pool, &id).await?;
    let now = now_sqlite();

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("开始项目事务失败: {}", error))?;

    sqlx::query("UPDATE projects SET deleted_at = $1, updated_at = $1 WHERE id = $2")
        .bind(&now)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("软删除项目失败: {}", error))?;

    sqlx::query("UPDATE tasks SET deleted_at = $1, updated_at = $1 WHERE project_id = $2 AND deleted_at IS NULL")
        .bind(&now)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("软删除项目任务失败: {}", error))?;

    tx.commit()
        .await
        .map_err(|error| format!("提交项目软删除失败: {}", error))?;

    insert_activity_log(
        &pool,
        "project_deleted",
        &project.name,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn permanently_delete_project<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let project = fetch_any_project_by_id(&pool, &id).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("开始项目事务失败: {}", error))?;

    sqlx::query(
        "DELETE FROM activity_logs WHERE project_id = $1 OR task_id IN (SELECT id FROM tasks WHERE project_id = $1)",
    )
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(|error| format!("删除项目活动日志失败: {}", error))?;
    sqlx::query("UPDATE employees SET project_id = NULL WHERE project_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("清除员工项目归属失败: {}", error))?;
    sqlx::query("DELETE FROM tasks WHERE project_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("删除项目任务失败: {}", error))?;

    insert_activity_log(
        &mut *tx,
        "project_permanently_deleted",
        &project.name,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("永久删除项目失败: {}", error))?;

    tx.commit()
        .await
        .map_err(|error| format!("提交项目永久删除失败: {}", error))?;

    Ok(())
}

#[tauri::command]
pub async fn restore_project<R: Runtime>(app: AppHandle<R>, id: String) -> Result<Project, String> {
    let pool = sqlite_pool(&app).await?;
    let project = fetch_any_project_by_id(&pool, &id).await?;
    if project.deleted_at.is_none() {
        return Err("项目不在回收站中".to_string());
    }

    sqlx::query("UPDATE projects SET deleted_at = NULL, updated_at = $1 WHERE id = $2")
        .bind(now_sqlite())
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("恢复项目失败: {}", error))?;

    sqlx::query("UPDATE tasks SET deleted_at = NULL, updated_at = $1 WHERE project_id = $2 AND deleted_at IS NOT NULL")
        .bind(now_sqlite())
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("恢复项目任务失败: {}", error))?;

    fetch_project_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn list_trashed_projects<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Project>, String> {
    let pool = sqlite_pool(&app).await?;
    sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("获取回收站项目列表失败: {}", error))
}

fn run_local_git_capture(repo_path: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|error| format!("执行 git 失败: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "git 命令执行失败".to_string()
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[tauri::command]
pub async fn check_project_repo_health<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<crate::db::models::ProjectRepoHealth, String> {
    use crate::db::models::{ProjectRepoHealth, ProjectRepoHealthCheck};

    let pool = sqlite_pool(&app).await?;
    let project = fetch_project_by_id(&pool, &project_id).await?;
    let mut checks = Vec::new();

    if project.project_type == PROJECT_TYPE_SSH {
        let remote_path = project.remote_repo_path.clone().unwrap_or_default();
        let has_ssh = project.ssh_config_id.is_some();
        let has_path = !remote_path.trim().is_empty();
        checks.push(ProjectRepoHealthCheck {
            key: "ssh_config".into(),
            label: "SSH 配置".into(),
            passed: has_ssh,
            detail: if has_ssh {
                "已绑定 SSH 配置".into()
            } else {
                "未绑定 SSH 配置".into()
            },
        });
        checks.push(ProjectRepoHealthCheck {
            key: "remote_path".into(),
            label: "远程仓库路径".into(),
            passed: has_path,
            detail: if has_path {
                remote_path.clone()
            } else {
                "未配置远程仓库目录".into()
            },
        });
        checks.push(ProjectRepoHealthCheck {
            key: "remote_git".into(),
            label: "远程 Git 预检".into(),
            passed: false,
            detail: "SSH 仓库健康需连接远程主机验证；请在设置中测试 SSH，并在远程确认目录含 .git".into(),
        });

        let accessible = has_ssh && has_path;
        return Ok(ProjectRepoHealth {
            project_id: project.id,
            project_type: project.project_type,
            path: project.remote_repo_path,
            path_exists: has_path,
            is_directory: has_path,
            is_git_repo: false,
            current_branch: None,
            is_dirty: None,
            accessible,
            message: if accessible {
                "SSH 项目基本配置完整；请在远程主机确认仓库可用性。".into()
            } else {
                "SSH 项目配置不完整，请补齐 SSH 配置与远程目录。".into()
            },
            checks,
        });
    }

    let raw_path = project.repo_path.clone().unwrap_or_default();
    let path_obj = PathBuf::from(raw_path.trim());
    let path_exists = path_obj.exists();
    let is_directory = path_exists && path_obj.is_dir();
    let is_git_repo = is_directory && path_obj.join(".git").exists();
    let accessible = is_directory;

    checks.push(ProjectRepoHealthCheck {
        key: "path_exists".into(),
        label: "路径存在".into(),
        passed: path_exists,
        detail: if path_exists {
            path_obj.display().to_string()
        } else {
            format!("路径不存在: {}", raw_path)
        },
    });
    checks.push(ProjectRepoHealthCheck {
        key: "is_directory".into(),
        label: "是目录".into(),
        passed: is_directory,
        detail: if is_directory {
            "路径是可用目录".into()
        } else {
            "路径不是目录或不可访问".into()
        },
    });
    checks.push(ProjectRepoHealthCheck {
        key: "is_git_repo".into(),
        label: "Git 仓库".into(),
        passed: is_git_repo,
        detail: if is_git_repo {
            "检测到 .git".into()
        } else {
            "缺少 .git，不是 Git 仓库".into()
        },
    });

    let mut current_branch = None;
    let mut is_dirty = None;
    if is_git_repo {
        let path_str = path_obj.to_string_lossy().to_string();
        match run_local_git_capture(&path_str, &["rev-parse", "--abbrev-ref", "HEAD"]) {
            Ok(branch) => {
                current_branch = Some(branch.clone());
                checks.push(ProjectRepoHealthCheck {
                    key: "current_branch".into(),
                    label: "当前分支".into(),
                    passed: true,
                    detail: branch,
                });
            }
            Err(error) => {
                checks.push(ProjectRepoHealthCheck {
                    key: "current_branch".into(),
                    label: "当前分支".into(),
                    passed: false,
                    detail: error,
                });
            }
        }

        match run_local_git_capture(&path_str, &["status", "--porcelain"]) {
            Ok(status) => {
                let dirty = !status.is_empty();
                is_dirty = Some(dirty);
                checks.push(ProjectRepoHealthCheck {
                    key: "worktree_dirty".into(),
                    label: "工作区状态".into(),
                    passed: true,
                    detail: if dirty {
                        format!("工作区有未提交变更（{} 行）", status.lines().count())
                    } else {
                        "工作区干净".into()
                    },
                });
            }
            Err(error) => {
                checks.push(ProjectRepoHealthCheck {
                    key: "worktree_dirty".into(),
                    label: "工作区状态".into(),
                    passed: false,
                    detail: error,
                });
            }
        }
    }

    let message = if !path_exists {
        "仓库路径不存在，请重新选择本地目录。".to_string()
    } else if !is_git_repo {
        "目录存在但不是 Git 仓库，Codex 会话启动前需要 .git。".to_string()
    } else {
        format!(
            "仓库可用。当前分支：{}。{}",
            current_branch.as_deref().unwrap_or("未知"),
            if is_dirty == Some(true) {
                "工作区有未提交变更。"
            } else {
                "工作区干净。"
            }
        )
    };

    Ok(ProjectRepoHealth {
        project_id: project.id,
        project_type: project.project_type,
        path: project.repo_path,
        path_exists,
        is_directory,
        is_git_repo,
        current_branch,
        is_dirty,
        accessible: accessible && is_git_repo,
        message,
        checks,
    })
}
