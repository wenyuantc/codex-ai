use super::*;

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
    employee_id: &str,
) -> Result<EmployeeRuntimeStatus, String> {
    let live_codex_processes =
        crate::codex::list_live_employee_processes(app, manager_state, employee_id).await?;
    let live_claude_processes =
        crate::claude::list_live_claude_employee_processes(claude_manager_state, employee_id).await;
    let pool = sqlite_pool(app).await?;
    let latest_session = fetch_latest_employee_session(app, employee_id).await?;
    let mut sessions = Vec::with_capacity(live_codex_processes.len() + live_claude_processes.len());

    for session_record_id in live_codex_processes
        .into_iter()
        .map(|process| process.session_record_id)
        .chain(
            live_claude_processes
                .into_iter()
                .map(|process| process.session_record_id),
        )
    {
        let session = fetch_codex_session_by_id(app, &session_record_id).await?;
        let task_title = if let Some(task_id) = session.task_id.as_deref() {
            sqlx::query_scalar::<_, Option<String>>("SELECT title FROM tasks WHERE id = $1 LIMIT 1")
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
    employee_id: String,
) -> Result<EmployeeRuntimeStatus, String> {
    build_employee_runtime_status(&app, state.inner(), claude_state.inner(), &employee_id).await
}

#[tauri::command]
pub async fn get_codex_session_status<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    claude_state: State<'_, Arc<tokio::sync::Mutex<ClaudeManager>>>,
    employee_id: String,
) -> Result<CodexRuntimeStatus, String> {
    let runtime =
        build_employee_runtime_status(&app, state.inner(), claude_state.inner(), &employee_id)
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

    let employee = Employee {
        id: new_id(),
        name: payload.name.trim().to_string(),
        role: payload.role,
        model: payload.model.unwrap_or_else(|| "gpt-5.4".to_string()),
        reasoning_effort: payload
            .reasoning_effort
            .unwrap_or_else(|| "high".to_string()),
        status: "offline".to_string(),
        specialization: normalize_optional_text(payload.specialization.as_deref()),
        system_prompt: normalize_optional_text(payload.system_prompt.as_deref()),
        project_id,
        ai_provider: payload.ai_provider.unwrap_or_else(|| "codex".to_string()),
        created_at: now_sqlite(),
        updated_at: now_sqlite(),
    };

    if employee.name.is_empty() {
        return Err("员工名称不能为空".to_string());
    }

    sqlx::query(
        "INSERT INTO employees (id, name, role, model, reasoning_effort, status, specialization, system_prompt, project_id, ai_provider, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
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
    if let Some(model) = updates.model {
        separated.push("model = ").push_bind_unseparated(model);
        touched = true;
    }
    if let Some(reasoning_effort) = updates.reasoning_effort {
        separated
            .push("reasoning_effort = ")
            .push_bind_unseparated(reasoning_effort);
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
    if let Some(ai_provider) = updates.ai_provider {
        separated
            .push("ai_provider = ")
            .push_bind_unseparated(ai_provider);
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
