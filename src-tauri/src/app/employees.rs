use super::*;
use crate::opencode;

fn normalize_employee_ai_provider(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("claude") => "claude".to_string(),
        Some("opencode") => "opencode".to_string(),
        Some("grok") => "grok".to_string(),
        Some("native") => "native".to_string(),
        _ => "codex".to_string(),
    }
}

/// 按 provider 归一化员工推理强度。Grok / OpenCode 仅 low|medium|high；
/// 内置 Agent 保留模型目录档位（含 xhigh/max/minimal）。
fn normalize_employee_reasoning_effort(provider: &str, value: Option<&str>) -> String {
    match provider {
        "native" => match value.map(str::trim) {
            Some(value)
                if matches!(
                    value,
                    "none" | "no_think" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
                ) =>
            {
                value.to_string()
            }
            _ => "high".to_string(),
        },
        "grok" => crate::grok::normalize_grok_reasoning_effort(value),
        "opencode" => match value.map(str::trim) {
            Some(value) if matches!(value, "low" | "medium" | "high") => value.to_string(),
            _ => "high".to_string(),
        },
        "claude" => match value.map(str::trim) {
            Some(value)
                if matches!(value, "low" | "medium" | "high" | "xhigh" | "max" | "auto") =>
            {
                value.to_string()
            }
            _ => "high".to_string(),
        },
        _ => match value.map(str::trim) {
            Some(value) if matches!(value, "low" | "medium" | "high" | "xhigh" | "max") => {
                value.to_string()
            }
            _ => "high".to_string(),
        },
    }
}

fn normalize_employee_model(provider: &str, value: Option<&str>) -> String {
    match provider {
        "grok" => crate::grok::normalize_grok_model(value),
        "native" => value
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "default".to_string()),
        "claude" => crate::claude::normalize_claude_model(value),
        "opencode" => value
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "openai/gpt-4o".to_string()),
        _ => value
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "gpt-5.4".to_string()),
    }
}

async fn list_live_opencode_employee_processes(
    state: &Arc<tokio::sync::Mutex<opencode::OpenCodeManager>>,
    employee_id: &str,
) -> Vec<opencode::ManagedOpenCodeProcess> {
    let manager: tokio::sync::MutexGuard<'_, opencode::OpenCodeManager> = state.lock().await;
    manager.get_employee_processes(employee_id)
}

async fn list_live_grok_employee_processes(
    state: &Arc<tokio::sync::Mutex<crate::grok::GrokManager>>,
    employee_id: &str,
) -> Vec<crate::grok::ManagedGrokProcess> {
    crate::grok::list_live_grok_employee_processes(state, employee_id).await
}

async fn resolve_employee_channel_id(
    pool: &SqlitePool,
    provider: &str,
    channel_id: Option<&str>,
) -> Result<Option<String>, String> {
    if provider != "native" {
        return Ok(None);
    }
    let channel_id = channel_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "内置 Agent 员工必须选择已启用的渠道".to_string())?;
    let record = crate::native::channels::fetch_channel_record(pool, channel_id).await?;
    if record.enabled == 0 {
        return Err(format!("渠道「{}」已停用，无法绑定", record.name));
    }
    Ok(Some(record.id))
}

pub(crate) async fn fetch_employee_by_id(pool: &SqlitePool, id: &str) -> Result<Employee, String> {
    sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Employee {} not found: {}", id, error))
}

async fn fetch_latest_employee_session<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
) -> Result<Option<CodexSessionRecord>, String> {
    let pool = sqlite_pool(app).await?;
    sqlx::query_as::<_, CodexSessionRecord>(
        "SELECT * FROM codex_sessions WHERE employee_id = $1 ORDER BY started_at DESC LIMIT 1",
    )
    .bind(employee_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch runtime status: {}", error))
}

async fn build_employee_runtime_status<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    claude_manager_state: &Arc<tokio::sync::Mutex<ClaudeManager>>,
    opencode_manager_state: &Arc<tokio::sync::Mutex<opencode::OpenCodeManager>>,
    grok_manager_state: &Arc<tokio::sync::Mutex<crate::grok::GrokManager>>,
    native_manager_state: &Arc<tokio::sync::Mutex<crate::native::NativeAgentManager>>,
    employee_id: &str,
) -> Result<EmployeeRuntimeStatus, String> {
    let live_codex_processes =
        crate::codex::list_live_employee_processes(app, manager_state, employee_id).await?;
    let live_claude_processes =
        crate::claude::list_live_claude_employee_processes(claude_manager_state, employee_id).await;
    let live_opencode_processes =
        list_live_opencode_employee_processes(opencode_manager_state, employee_id).await;
    let live_grok_processes =
        list_live_grok_employee_processes(grok_manager_state, employee_id).await;
    let live_native_processes =
        crate::native::list_live_native_employee_processes(native_manager_state, employee_id).await;
    let pool = sqlite_pool(app).await?;
    let latest_session = fetch_latest_employee_session(app, employee_id).await?;
    let total = live_codex_processes.len()
        + live_claude_processes.len()
        + live_opencode_processes.len()
        + live_grok_processes.len()
        + live_native_processes.len();
    let mut sessions = Vec::with_capacity(total);

    for session_record_id in live_codex_processes
        .into_iter()
        .map(|process| process.session_record_id)
        .chain(
            live_claude_processes
                .into_iter()
                .map(|process| process.session_record_id),
        )
        .chain(
            live_opencode_processes
                .into_iter()
                .map(|process| process.session_record_id),
        )
        .chain(
            live_grok_processes
                .into_iter()
                .map(|process| process.session_record_id),
        )
        .chain(
            live_native_processes
                .into_iter()
                .map(|process| process.session_record_id),
        )
    {
        let session = fetch_codex_session_by_id(app, &session_record_id).await?;
        let task_title = if let Some(task_id) = session.task_id.as_deref() {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT title FROM tasks WHERE id = $1 AND deleted_at IS NULL LIMIT 1",
            )
            .bind(task_id)
            .fetch_optional(&pool)
            .await
            .map_err(|error| format!("Failed to fetch task title: {}", error))?
            .flatten()
        } else {
            None
        };

        sessions.push(EmployeeRunningSession {
            session_record_id: session.id.clone(),
            cli_session_id: session.cli_session_id.clone(),
            task_id: session.task_id.clone(),
            task_title,
            ai_provider: session.ai_provider.clone(),
            session_kind: session.session_kind.clone(),
            session_origin: session.session_origin.clone(),
            started_at: session.started_at.clone(),
            status: session.status.clone(),
        });
    }

    sessions.sort_by(|left, right| {
        right
            .started_at
            .cmp(&left.started_at)
            .then_with(|| right.session_record_id.cmp(&left.session_record_id))
    });

    Ok(EmployeeRuntimeStatus {
        running: !sessions.is_empty(),
        sessions,
        latest_session,
    })
}

#[tauri::command]
pub async fn get_employee_runtime_status<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    claude_state: State<'_, Arc<tokio::sync::Mutex<ClaudeManager>>>,
    opencode_state: State<'_, Arc<tokio::sync::Mutex<opencode::OpenCodeManager>>>,
    grok_state: State<'_, Arc<tokio::sync::Mutex<crate::grok::GrokManager>>>,
    native_state: State<'_, Arc<tokio::sync::Mutex<crate::native::NativeAgentManager>>>,
    employee_id: String,
) -> Result<EmployeeRuntimeStatus, String> {
    build_employee_runtime_status(
        &app,
        state.inner(),
        claude_state.inner(),
        opencode_state.inner(),
        grok_state.inner(),
        native_state.inner(),
        &employee_id,
    )
    .await
}

#[tauri::command]
pub async fn get_codex_session_status<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    claude_state: State<'_, Arc<tokio::sync::Mutex<ClaudeManager>>>,
    opencode_state: State<'_, Arc<tokio::sync::Mutex<opencode::OpenCodeManager>>>,
    grok_state: State<'_, Arc<tokio::sync::Mutex<crate::grok::GrokManager>>>,
    native_state: State<'_, Arc<tokio::sync::Mutex<crate::native::NativeAgentManager>>>,
    employee_id: String,
) -> Result<CodexRuntimeStatus, String> {
    let runtime = build_employee_runtime_status(
        &app,
        state.inner(),
        claude_state.inner(),
        opencode_state.inner(),
        grok_state.inner(),
        native_state.inner(),
        &employee_id,
    )
    .await?;
    let session = if let Some(running_session) = runtime.sessions.first() {
        Some(fetch_codex_session_by_id(&app, &running_session.session_record_id).await?)
    } else {
        runtime.latest_session
    };

    Ok(CodexRuntimeStatus {
        running: runtime.running,
        session,
    })
}

#[tauri::command]
pub async fn create_employee<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateEmployee,
) -> Result<Employee, String> {
    let pool = sqlite_pool(&app).await?;
    let project_id = normalize_optional_text(payload.project_id.as_deref());
    if let Some(project_id) = project_id.as_deref() {
        ensure_project_exists(&pool, project_id).await?;
    }

    let ai_provider = normalize_employee_ai_provider(payload.ai_provider.as_deref());
    let ai_channel_id = resolve_employee_channel_id(
        &pool,
        ai_provider.as_str(),
        payload.ai_channel_id.as_deref(),
    )
    .await?;
    let employee = Employee {
        id: new_id(),
        name: payload.name.trim().to_string(),
        role: payload.role,
        model: normalize_employee_model(ai_provider.as_str(), payload.model.as_deref()),
        reasoning_effort: normalize_employee_reasoning_effort(
            ai_provider.as_str(),
            payload.reasoning_effort.as_deref(),
        ),
        status: "offline".to_string(),
        specialization: normalize_optional_text(payload.specialization.as_deref()),
        system_prompt: normalize_optional_text(payload.system_prompt.as_deref()),
        project_id,
        ai_provider,
        ai_channel_id,
        created_at: now_sqlite(),
        updated_at: now_sqlite(),
    };

    if employee.name.is_empty() {
        return Err("员工名称不能为空".to_string());
    }

    sqlx::query(
        "INSERT INTO employees (id, name, role, model, reasoning_effort, status, specialization, system_prompt, project_id, ai_provider, ai_channel_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(&employee.id)
    .bind(&employee.name)
    .bind(&employee.role)
    .bind(&employee.model)
    .bind(&employee.reasoning_effort)
    .bind(&employee.status)
    .bind(&employee.specialization)
    .bind(&employee.system_prompt)
    .bind(&employee.project_id)
    .bind(&employee.ai_provider)
    .bind(&employee.ai_channel_id)
    .bind(&employee.created_at)
    .bind(&employee.updated_at)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to create employee: {}", error))?;

    fetch_employee_by_id(&pool, &employee.id).await
}

#[tauri::command]
pub async fn update_employee<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateEmployee,
) -> Result<Employee, String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_employee_by_id(&pool, &id).await?;
    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE employees SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(name) = updates.name {
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return Err("员工名称不能为空".to_string());
        }
        separated.push("name = ").push_bind_unseparated(trimmed);
        touched = true;
    }
    if let Some(role) = updates.role {
        separated.push("role = ").push_bind_unseparated(role);
        touched = true;
    }
    let next_provider = updates
        .ai_provider
        .as_deref()
        .map(|value| normalize_employee_ai_provider(Some(value)))
        .unwrap_or_else(|| current.ai_provider.clone());
    let model_updated = updates.model.is_some();
    let reasoning_effort_updated = updates.reasoning_effort.is_some();
    let provider_updated = updates.ai_provider.is_some();

    if let Some(model) = updates.model {
        separated
            .push("model = ")
            .push_bind_unseparated(normalize_employee_model(
                next_provider.as_str(),
                Some(model.as_str()),
            ));
        touched = true;
    }
    if let Some(reasoning_effort) = updates.reasoning_effort {
        separated.push("reasoning_effort = ").push_bind_unseparated(
            normalize_employee_reasoning_effort(
                next_provider.as_str(),
                Some(reasoning_effort.as_str()),
            ),
        );
        touched = true;
    }
    if let Some(status) = updates.status {
        separated.push("status = ").push_bind_unseparated(status);
        touched = true;
    }
    if let Some(specialization) = updates.specialization {
        separated.push("specialization = ").push_bind_unseparated(
            specialization.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(system_prompt) = updates.system_prompt {
        separated.push("system_prompt = ").push_bind_unseparated(
            system_prompt.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(project_id) = updates.project_id {
        let project_id = match project_id {
            Some(project_id) => {
                let project_id = normalize_optional_text(Some(&project_id));
                if let Some(project_id) = project_id.as_deref() {
                    ensure_project_exists(&pool, project_id).await?;
                }
                project_id
            }
            None => None,
        };
        separated
            .push("project_id = ")
            .push_bind_unseparated(project_id);
        touched = true;
    }
    if provider_updated {
        separated
            .push("ai_provider = ")
            .push_bind_unseparated(next_provider.clone());
        touched = true;
        // 切换 provider 时，即使未显式改 effort，也按新 provider 收敛当前 effort
        if !reasoning_effort_updated {
            separated.push("reasoning_effort = ").push_bind_unseparated(
                normalize_employee_reasoning_effort(
                    next_provider.as_str(),
                    Some(current.reasoning_effort.as_str()),
                ),
            );
        }
        if !model_updated {
            separated
                .push("model = ")
                .push_bind_unseparated(normalize_employee_model(
                    next_provider.as_str(),
                    Some(current.model.as_str()),
                ));
        }
    }

    let requested_channel = updates
        .ai_channel_id
        .as_ref()
        .map(|value| value.as_deref())
        .unwrap_or(current.ai_channel_id.as_deref());
    if next_provider == "native" || updates.ai_channel_id.is_some() || provider_updated {
        let channel_id =
            resolve_employee_channel_id(&pool, next_provider.as_str(), requested_channel).await?;
        separated
            .push("ai_channel_id = ")
            .push_bind_unseparated(channel_id);
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
        .map_err(|error| format!("Failed to update employee: {}", error))?;

    fetch_employee_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn delete_employee<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    claude_state: State<'_, Arc<tokio::sync::Mutex<ClaudeManager>>>,
    opencode_state: State<'_, Arc<tokio::sync::Mutex<opencode::OpenCodeManager>>>,
    grok_state: State<'_, Arc<tokio::sync::Mutex<crate::grok::GrokManager>>>,
    native_state: State<'_, Arc<tokio::sync::Mutex<crate::native::NativeAgentManager>>>,
    id: String,
) -> Result<(), String> {
    if !crate::codex::list_live_employee_processes(&app, state.inner(), &id)
        .await?
        .is_empty()
    {
        return Err("员工仍有运行中的 Codex 会话，不能删除".to_string());
    }
    if !crate::claude::list_live_claude_employee_processes(claude_state.inner(), &id)
        .await
        .is_empty()
    {
        return Err("员工仍有运行中的 Claude 会话，不能删除".to_string());
    }
    if !list_live_opencode_employee_processes(opencode_state.inner(), &id)
        .await
        .is_empty()
    {
        return Err("员工仍有运行中的 OpenCode 会话，不能删除".to_string());
    }
    if !list_live_grok_employee_processes(grok_state.inner(), &id)
        .await
        .is_empty()
    {
        return Err("员工仍有运行中的 Grok 会话，不能删除".to_string());
    }
    if !crate::native::list_live_native_employee_processes(native_state.inner(), &id)
        .await
        .is_empty()
    {
        return Err("员工仍有运行中的内置 Agent 会话，不能删除".to_string());
    }

    let pool = sqlite_pool(&app).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start employee transaction: {}", error))?;

    sqlx::query("UPDATE tasks SET assignee_id = NULL WHERE assignee_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to clear employee assignments: {}", error))?;
    sqlx::query("UPDATE activity_logs SET employee_id = NULL WHERE employee_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to preserve employee activity logs: {}", error))?;
    sqlx::query("DELETE FROM employee_metrics WHERE employee_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete employee metrics: {}", error))?;
    sqlx::query("DELETE FROM employees WHERE id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete employee: {}", error))?;

    tx.commit()
        .await
        .map_err(|error| format!("Failed to commit employee delete: {}", error))?;

    Ok(())
}

#[tauri::command]
pub async fn update_employee_status<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    status: String,
) -> Result<Employee, String> {
    let pool = sqlite_pool(&app).await?;
    sqlx::query("UPDATE employees SET status = $1 WHERE id = $2")
        .bind(&status)
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update employee status: {}", error))?;

    fetch_employee_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn list_employees<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Employee>, String> {
    let pool = sqlite_pool(&app).await?;
    sqlx::query_as::<_, Employee>("SELECT * FROM employees ORDER BY created_at")
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("获取员工列表失败: {}", error))
}

#[tauri::command]
pub async fn list_employee_metrics<R: Runtime>(
    app: AppHandle<R>,
    payload: Option<crate::db::models::ListEmployeeMetricsPayload>,
) -> Result<Vec<EmployeeMetric>, String> {
    let pool = sqlite_pool(&app).await?;
    let days = payload
        .and_then(|p| p.days)
        .filter(|d| *d > 0)
        .unwrap_or(30)
        .clamp(1, 365);
    let offset = format!("-{days} days");

    sqlx::query_as::<_, EmployeeMetric>(
        "SELECT * FROM employee_metrics
         WHERE period_start >= datetime('now', $1)
         ORDER BY tasks_completed DESC",
    )
    .bind(&offset)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("获取员工绩效失败: {}", error))
}

#[cfg(test)]
mod tests {
    use super::normalize_employee_reasoning_effort;

    #[test]
    fn native_keeps_catalog_thinking_levels() {
        assert_eq!(
            normalize_employee_reasoning_effort("native", Some("xhigh")),
            "xhigh"
        );
        assert_eq!(
            normalize_employee_reasoning_effort("native", Some("max")),
            "max"
        );
        assert_eq!(
            normalize_employee_reasoning_effort("native", Some("minimal")),
            "minimal"
        );
        assert_eq!(
            normalize_employee_reasoning_effort("native", Some("none")),
            "none"
        );
        assert_eq!(
            normalize_employee_reasoning_effort("native", Some("no_think")),
            "no_think"
        );
        assert_eq!(
            normalize_employee_reasoning_effort("native", Some("high")),
            "high"
        );
    }

    #[test]
    fn native_unknown_effort_falls_back_to_high() {
        assert_eq!(normalize_employee_reasoning_effort("native", None), "high");
        assert_eq!(
            normalize_employee_reasoning_effort("native", Some("auto")),
            "high"
        );
        assert_eq!(
            normalize_employee_reasoning_effort("native", Some("")),
            "high"
        );
    }

    #[test]
    fn grok_still_clamps_xhigh_to_high() {
        assert_eq!(
            normalize_employee_reasoning_effort("grok", Some("xhigh")),
            "high"
        );
        assert_eq!(
            normalize_employee_reasoning_effort("grok", Some("medium")),
            "medium"
        );
    }
}
