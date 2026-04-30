use super::*;
use crate::app::fetch_project_by_id;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutionContext {
    pub(crate) execution_target: String,
    pub(crate) working_dir: Option<String>,
    pub(crate) ssh_config_id: Option<String>,
    pub(crate) target_host_label: Option<String>,
    pub(crate) artifact_capture_mode: String,
}

impl ExecutionContext {
    pub(super) fn local_default() -> Self {
        Self {
            execution_target: EXECUTION_TARGET_LOCAL.to_string(),
            working_dir: None,
            ssh_config_id: None,
            target_host_label: None,
            artifact_capture_mode: ARTIFACT_CAPTURE_MODE_LOCAL_FULL.to_string(),
        }
    }
}

pub(super) async fn resolve_task_project_execution_context<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
) -> Result<ExecutionContext, String> {
    let pool = sqlite_pool(app).await?;
    let row = sqlx::query_as::<_, (Option<String>, String, Option<String>, Option<String>)>(
        "SELECT projects.repo_path, projects.project_type, projects.ssh_config_id, projects.remote_repo_path FROM tasks INNER JOIN projects ON projects.id = tasks.project_id WHERE tasks.id = $1 AND tasks.deleted_at IS NULL AND projects.deleted_at IS NULL LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| {
        format!(
            "Failed to resolve task {} project execution context: {}",
            task_id, error
        )
    })?
    .ok_or_else(|| format!("Task {} not found when resolving project path", task_id))?;

    let (repo_path, project_type, ssh_config_id, remote_repo_path) = row;
    if project_type == EXECUTION_TARGET_SSH {
        let ssh_config_id = ssh_config_id
            .ok_or_else(|| "当前 SSH 项目缺少 ssh_config_id，无法启动 Codex。".to_string())?;
        let ssh_config = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
        let working_dir = remote_repo_path
            .map(|value| normalize_runtime_path_string(&value))
            .ok_or_else(|| "当前 SSH 项目缺少远程仓库目录，无法启动 Codex。".to_string())?;
        Ok(ExecutionContext {
            execution_target: EXECUTION_TARGET_SSH.to_string(),
            working_dir: Some(working_dir),
            ssh_config_id: Some(ssh_config_id),
            target_host_label: Some(format!(
                "{}@{}:{}",
                ssh_config.username, ssh_config.host, ssh_config.port
            )),
            artifact_capture_mode: ARTIFACT_CAPTURE_MODE_SSH_FULL.to_string(),
        })
    } else {
        Ok(ExecutionContext {
            execution_target: EXECUTION_TARGET_LOCAL.to_string(),
            working_dir: repo_path,
            ssh_config_id: None,
            target_host_label: None,
            artifact_capture_mode: ARTIFACT_CAPTURE_MODE_LOCAL_FULL.to_string(),
        })
    }
}

pub(super) async fn resolve_project_execution_context<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
) -> Result<ExecutionContext, String> {
    let pool = sqlite_pool(app).await?;
    let project = fetch_project_by_id(&pool, project_id).await?;

    if project.project_type == EXECUTION_TARGET_SSH {
        let ssh_config_id = project
            .ssh_config_id
            .ok_or_else(|| "当前 SSH 项目缺少 ssh_config_id，无法启动 Codex。".to_string())?;
        let ssh_config = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
        let working_dir = project
            .remote_repo_path
            .map(|value| normalize_runtime_path_string(&value))
            .ok_or_else(|| "当前 SSH 项目缺少远程仓库目录，无法启动 Codex。".to_string())?;
        Ok(ExecutionContext {
            execution_target: EXECUTION_TARGET_SSH.to_string(),
            working_dir: Some(working_dir),
            ssh_config_id: Some(ssh_config_id),
            target_host_label: Some(format!(
                "{}@{}:{}",
                ssh_config.username, ssh_config.host, ssh_config.port
            )),
            artifact_capture_mode: ARTIFACT_CAPTURE_MODE_SSH_FULL.to_string(),
        })
    } else {
        Ok(ExecutionContext {
            execution_target: EXECUTION_TARGET_LOCAL.to_string(),
            working_dir: project.repo_path,
            ssh_config_id: None,
            target_host_label: None,
            artifact_capture_mode: ARTIFACT_CAPTURE_MODE_LOCAL_FULL.to_string(),
        })
    }
}

pub(super) async fn resolve_one_shot_working_dir<R: Runtime>(
    app: &AppHandle<R>,
    task_id: Option<&str>,
    project_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<Option<String>, String> {
    let execution_context = match task_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(task_id) => Some(resolve_task_project_execution_context(app, task_id).await?),
        None => match project_id.map(str::trim).filter(|value| !value.is_empty()) {
            Some(project_id) => Some(resolve_project_execution_context(app, project_id).await?),
            None => None,
        },
    };

    if let Some(explicit_working_dir) = working_dir.map(str::trim).filter(|value| !value.is_empty())
    {
        if matches!(
            execution_context
                .as_ref()
                .map(|context| context.execution_target.as_str()),
            Some(EXECUTION_TARGET_SSH)
        ) {
            return Ok(Some(normalize_runtime_path_string(explicit_working_dir)));
        }
        return validate_project_repo_path(Some(explicit_working_dir));
    }

    match execution_context {
        Some(context) => match context.execution_target.as_str() {
            EXECUTION_TARGET_LOCAL => match context.working_dir {
                Some(repo_path) => validate_project_repo_path(Some(&repo_path)),
                None => Ok(None),
            },
            EXECUTION_TARGET_SSH => Ok(context.working_dir),
            _ => Ok(None),
        },
        None => Ok(None),
    }
}

pub(crate) async fn resolve_session_execution_context<R: Runtime>(
    app: &AppHandle<R>,
    task_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<ExecutionContext, String> {
    let task_context = match task_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(task_id) => Some(resolve_task_project_execution_context(app, task_id).await?),
        None => None,
    };

    if let Some(explicit_working_dir) = working_dir.map(str::trim).filter(|value| !value.is_empty())
    {
        if let Some(mut context) = task_context {
            if context.execution_target == EXECUTION_TARGET_SSH {
                context.working_dir = Some(normalize_runtime_path_string(explicit_working_dir));
                return Ok(context);
            }
        }
        return Ok(ExecutionContext {
            execution_target: EXECUTION_TARGET_LOCAL.to_string(),
            working_dir: validate_project_repo_path(Some(explicit_working_dir))?,
            ssh_config_id: None,
            target_host_label: None,
            artifact_capture_mode: ARTIFACT_CAPTURE_MODE_LOCAL_FULL.to_string(),
        });
    }

    match task_context {
        Some(context) => {
            if context.execution_target == EXECUTION_TARGET_LOCAL {
                match context.working_dir.as_deref() {
                    Some(repo_path) => Ok(ExecutionContext {
                        working_dir: validate_project_repo_path(Some(repo_path))?,
                        ..context
                    }),
                    None => Err("当前任务所属项目未配置仓库路径，无法启动 Codex。".to_string()),
                }
            } else {
                Ok(context)
            }
        }
        None => Ok(ExecutionContext::local_default()),
    }
}
