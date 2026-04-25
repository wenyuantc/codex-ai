use super::*;

pub(crate) async fn insert_activity_log(
    pool: &SqlitePool,
    action: &str,
    details: &str,
    employee_id: Option<&str>,
    task_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO activity_logs (id, employee_id, action, details, task_id, project_id) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(new_id())
    .bind(employee_id)
    .bind(action)
    .bind(details)
    .bind(task_id)
    .bind(project_id)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to insert activity log: {}", error))?;

    Ok(())
}

pub(crate) async fn insert_codex_session_event_with_id(
    pool: &SqlitePool,
    session_id: &str,
    event_type: &str,
    message: Option<&str>,
) -> Result<String, String> {
    let event_id = new_id();

    sqlx::query(
        "INSERT INTO codex_session_events (id, session_id, event_type, message) VALUES ($1, $2, $3, $4)",
    )
    .bind(&event_id)
    .bind(session_id)
    .bind(event_type)
    .bind(message)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to insert session event: {}", error))?;

    Ok(event_id)
}

pub(crate) async fn insert_codex_session_event(
    pool: &SqlitePool,
    session_id: &str,
    event_type: &str,
    message: Option<&str>,
) -> Result<(), String> {
    insert_codex_session_event_with_id(pool, session_id, event_type, message)
        .await
        .map(|_| ())
}

#[tauri::command]
pub async fn get_codex_session_log_lines<R: Runtime>(
    app: AppHandle<R>,
    session_id: String,
) -> Result<Vec<CodexSessionLogLine>, String> {
    let pool = sqlite_pool(&app).await?;
    let resolved_session_record_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM codex_sessions
        WHERE id = $1 OR cli_session_id = $1
        ORDER BY CASE WHEN id = $1 THEN 0 ELSE 1 END, started_at DESC
        LIMIT 1
        "#,
    )
    .bind(&session_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to resolve session log target: {}", error))?;

    let Some(resolved_session_record_id) = resolved_session_record_id else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        r#"
        WITH recent AS (
            SELECT rowid AS event_rowid, id, event_type, message
            FROM codex_session_events
            WHERE session_id = $1
              AND message IS NOT NULL
            ORDER BY event_rowid DESC
            LIMIT 2000
        )
        SELECT id, event_type, message
        FROM recent
        ORDER BY event_rowid ASC
        "#,
    )
    .bind(&resolved_session_record_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to fetch session log lines: {}", error))?;

    Ok(rows
        .into_iter()
        .filter_map(|(event_id, event_type, message)| {
            message.and_then(|value| {
                format_session_log_line(&event_type, &value)
                    .map(|line| CodexSessionLogLine { event_id, line })
            })
        })
        .collect())
}

pub(crate) async fn replace_codex_session_file_changes<R: Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
    changes: &[CodexSessionFileChangeInput],
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;

    sqlx::query("DELETE FROM codex_session_file_changes WHERE session_id = $1")
        .bind(session_id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to clear session file changes: {}", error))?;

    for change in changes {
        let change_id = new_id();
        sqlx::query(
            "INSERT INTO codex_session_file_changes (id, session_id, path, change_type, capture_mode, previous_path) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&change_id)
        .bind(session_id)
        .bind(&change.path)
        .bind(&change.change_type)
        .bind(&change.capture_mode)
        .bind(&change.previous_path)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to insert session file change: {}", error))?;

        if let Some(detail) = &change.detail {
            sqlx::query(
                "INSERT INTO codex_session_file_change_details (id, change_id, absolute_path, previous_absolute_path, before_status, before_text, before_truncated, after_status, after_text, after_truncated) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(new_id())
            .bind(&change_id)
            .bind(&detail.absolute_path)
            .bind(&detail.previous_absolute_path)
            .bind(&detail.before_status)
            .bind(&detail.before_text)
            .bind(if detail.before_truncated { 1 } else { 0 })
            .bind(&detail.after_status)
            .bind(&detail.after_text)
            .bind(if detail.after_truncated { 1 } else { 0 })
            .execute(&pool)
            .await
            .map_err(|error| format!("Failed to insert session file change detail: {}", error))?;
        }
    }

    Ok(())
}

pub(crate) async fn insert_codex_session_record<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: Option<&str>,
    task_id: Option<&str>,
    task_git_context_id: Option<&str>,
    working_dir: Option<&str>,
    resume_session_id: Option<&str>,
    session_kind: &str,
    status: &str,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    target_host_label: Option<&str>,
    artifact_capture_mode: &str,
    ai_provider: Option<&str>,
    thinking_budget_tokens: Option<i32>,
) -> Result<CodexSessionRecord, String> {
    let pool = sqlite_pool(app).await?;
    let project_id = match task_id {
        Some(task_id) => sqlx::query_scalar::<_, Option<String>>(
            "SELECT project_id FROM tasks WHERE id = $1 LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(&pool)
        .await
        .map_err(|error| format!("Failed to resolve session project: {}", error))?
        .flatten(),
        None => None,
    };

    let record = CodexSessionRecord {
        id: new_id(),
        employee_id: employee_id.map(ToOwned::to_owned),
        task_id: task_id.map(ToOwned::to_owned),
        project_id,
        task_git_context_id: task_git_context_id.map(ToOwned::to_owned),
        cli_session_id: None,
        working_dir: working_dir.map(ToOwned::to_owned),
        execution_target: execution_target.to_string(),
        ssh_config_id: ssh_config_id.map(ToOwned::to_owned),
        target_host_label: target_host_label.map(ToOwned::to_owned),
        artifact_capture_mode: artifact_capture_mode.to_string(),
        session_kind: session_kind.to_string(),
        status: status.to_string(),
        started_at: now_sqlite(),
        ended_at: None,
        exit_code: None,
        resume_session_id: resume_session_id.map(ToOwned::to_owned),
        ai_provider: ai_provider.unwrap_or("codex").to_string(),
        thinking_budget_tokens,
        created_at: now_sqlite(),
    };

    sqlx::query(
        "INSERT INTO codex_sessions (id, employee_id, task_id, project_id, task_git_context_id, cli_session_id, working_dir, execution_target, ssh_config_id, target_host_label, artifact_capture_mode, session_kind, status, started_at, ended_at, exit_code, resume_session_id, ai_provider, thinking_budget_tokens, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)",
    )
    .bind(&record.id)
    .bind(&record.employee_id)
    .bind(&record.task_id)
    .bind(&record.project_id)
    .bind(&record.task_git_context_id)
    .bind(&record.cli_session_id)
    .bind(&record.working_dir)
    .bind(&record.execution_target)
    .bind(&record.ssh_config_id)
    .bind(&record.target_host_label)
    .bind(&record.artifact_capture_mode)
    .bind(&record.session_kind)
    .bind(&record.status)
    .bind(&record.started_at)
    .bind(&record.ended_at)
    .bind(record.exit_code)
    .bind(&record.resume_session_id)
    .bind(&record.ai_provider)
    .bind(record.thinking_budget_tokens)
    .bind(&record.created_at)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to insert session record: {}", error))?;

    Ok(record)
}

pub(crate) async fn update_codex_session_record<R: Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
    status: Option<&str>,
    cli_session_id: Option<Option<&str>>,
    exit_code: Option<Option<i32>>,
    ended_at: Option<Option<&str>>,
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE codex_sessions SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(status) = status {
        separated.push("status = ").push_bind_unseparated(status);
        touched = true;
    }
    if let Some(cli_session_id) = cli_session_id {
        separated
            .push("cli_session_id = ")
            .push_bind_unseparated(cli_session_id.map(ToOwned::to_owned));
        touched = true;
    }
    if let Some(exit_code) = exit_code {
        separated
            .push("exit_code = ")
            .push_bind_unseparated(exit_code);
        touched = true;
    }
    if let Some(ended_at) = ended_at {
        separated
            .push("ended_at = ")
            .push_bind_unseparated(ended_at.map(ToOwned::to_owned));
        touched = true;
    }

    if !touched {
        return Ok(());
    }

    builder.push(" WHERE id = ").push_bind(session_id);
    builder
        .build()
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update session record: {}", error))?;

    Ok(())
}

pub(crate) async fn fetch_codex_session_by_id<R: Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
) -> Result<CodexSessionRecord, String> {
    let pool = sqlite_pool(app).await?;
    sqlx::query_as::<_, CodexSessionRecord>("SELECT * FROM codex_sessions WHERE id = $1 LIMIT 1")
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch session record: {}", error))
}

pub(crate) fn resolve_session_resume_state(
    cli_session_id: Option<&str>,
    employee_id: Option<&str>,
    employee_name: Option<&str>,
    status: &str,
    has_running_conflict: bool,
    running_conflict_message: &str,
) -> (String, Option<String>, bool) {
    if cli_session_id.is_none() {
        return (
            "missing_cli_session".to_string(),
            Some("该对话缺少可恢复的 CLI 对话 ID，只能查看，不能继续。".to_string()),
            false,
        );
    }

    if employee_id.is_none() || employee_name.is_none() {
        return (
            "missing_employee".to_string(),
            Some("该对话缺少有效的关联员工，暂时无法恢复。".to_string()),
            false,
        );
    }

    if status == "stopping" {
        return (
            "stopping".to_string(),
            Some("该对话正在停止，请稍后再试。".to_string()),
            false,
        );
    }

    if has_running_conflict {
        return (
            "running".to_string(),
            Some(running_conflict_message.to_string()),
            false,
        );
    }

    ("ready".to_string(), None, true)
}

fn running_task_session_key(task_id: &str, session_kind: &str) -> String {
    format!("{task_id}::{session_kind}")
}

fn resolve_session_kind(session_kind: &str) -> CodexSessionKind {
    match session_kind {
        "review" => CodexSessionKind::Review,
        _ => CodexSessionKind::Execution,
    }
}

fn resolve_claude_session_kind(session_kind: &str) -> crate::claude::ClaudeSessionKind {
    match session_kind {
        "review" => crate::claude::ClaudeSessionKind::Review,
        _ => crate::claude::ClaudeSessionKind::Execution,
    }
}

pub(crate) fn resolve_running_conflict_message(task_id: Option<&str>) -> &'static str {
    if task_id.is_some() {
        "关联任务当前已有运行中的对话，请先停止后再继续。"
    } else {
        "关联员工当前已有运行中的对话，请先停止后再继续。"
    }
}

async fn has_running_session_conflict<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    claude_manager_state: &Arc<tokio::sync::Mutex<ClaudeManager>>,
    employee_id: Option<&str>,
    task_id: Option<&str>,
    session_kind: &str,
) -> Result<bool, String> {
    if let Some(task_id) = task_id {
        let has_codex_conflict = crate::codex::get_live_task_process_by_task(
            app,
            manager_state,
            task_id,
            resolve_session_kind(session_kind),
        )
        .await?
        .is_some();
        let has_claude_conflict = {
            let manager = claude_manager_state.lock().await;
            manager
                .get_task_process_any(task_id, resolve_claude_session_kind(session_kind))
                .is_some()
        };
        return Ok(has_codex_conflict || has_claude_conflict);
    }

    let Some(employee_id) = employee_id else {
        return Ok(false);
    };

    let has_codex_processes =
        !crate::codex::list_live_employee_processes(app, manager_state, employee_id)
            .await?
            .is_empty();
    let has_claude_processes =
        !crate::claude::list_live_claude_employee_processes(claude_manager_state, employee_id)
            .await
            .is_empty();

    Ok(has_codex_processes || has_claude_processes)
}

fn format_session_log_line(event_type: &str, message: &str) -> Option<String> {
    let preserved = message.trim_end_matches(['\r', '\n']);
    if preserved.trim().is_empty() {
        return None;
    }

    match event_type {
        "stdout" => Some(preserved.to_string()),
        "stderr" => Some(if preserved.starts_with('[') {
            preserved.to_string()
        } else {
            format!("[ERROR] {}", preserved)
        }),
        "session_failed"
        | "spawn_failed"
        | "validation_failed"
        | "activity_log_failed"
        | "session_file_changes_failed" => Some(format!("[ERROR] {}", preserved.trim())),
        "session_exited" => Some(format!("[EXIT] {}", preserved.trim())),
        "review_report" => None,
        _ => Some(format!("[SYSTEM] {}", preserved.trim())),
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct TaskSearchRow {
    id: String,
    title: String,
    description: Option<String>,
    status: String,
    priority: String,
    project_id: String,
    project_name: String,
    updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct EmployeeSearchRow {
    id: String,
    name: String,
    role: String,
    specialization: Option<String>,
    status: String,
    project_id: Option<String>,
    project_name: Option<String>,
    updated_at: String,
}

fn normalize_search_query(value: &str) -> String {
    value.trim().to_lowercase()
}

pub(crate) fn normalize_global_search_types(raw_types: Option<Vec<String>>) -> HashSet<String> {
    let mut kinds = HashSet::new();

    if let Some(raw_types) = raw_types {
        for item in raw_types {
            match item.trim().to_lowercase().as_str() {
                GLOBAL_SEARCH_TYPE_PROJECT => {
                    kinds.insert(GLOBAL_SEARCH_TYPE_PROJECT.to_string());
                }
                GLOBAL_SEARCH_TYPE_TASK => {
                    kinds.insert(GLOBAL_SEARCH_TYPE_TASK.to_string());
                }
                GLOBAL_SEARCH_TYPE_EMPLOYEE => {
                    kinds.insert(GLOBAL_SEARCH_TYPE_EMPLOYEE.to_string());
                }
                GLOBAL_SEARCH_TYPE_SESSION => {
                    kinds.insert(GLOBAL_SEARCH_TYPE_SESSION.to_string());
                }
                _ => {}
            }
        }
    }

    if kinds.is_empty() {
        kinds.extend([
            GLOBAL_SEARCH_TYPE_PROJECT.to_string(),
            GLOBAL_SEARCH_TYPE_TASK.to_string(),
            GLOBAL_SEARCH_TYPE_EMPLOYEE.to_string(),
            GLOBAL_SEARCH_TYPE_SESSION.to_string(),
        ]);
    }

    kinds
}

fn text_match_score(
    normalized_query: &str,
    value: Option<&str>,
    exact_score: i64,
    prefix_score: i64,
    contains_score: i64,
) -> i64 {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return 0;
    };
    let normalized_value = value.to_lowercase();

    if normalized_value == normalized_query {
        exact_score
    } else if normalized_value.starts_with(normalized_query) {
        prefix_score
    } else if normalized_value.contains(normalized_query) {
        contains_score
    } else {
        0
    }
}

fn best_match_score(
    normalized_query: &str,
    fields: &[Option<&str>],
    exact_score: i64,
    prefix_score: i64,
    contains_score: i64,
) -> i64 {
    fields
        .iter()
        .map(|field| {
            text_match_score(
                normalized_query,
                *field,
                exact_score,
                prefix_score,
                contains_score,
            )
        })
        .max()
        .unwrap_or(0)
}

fn search_recency_bonus(value: Option<&str>) -> i64 {
    let Some(value) = value else {
        return 0;
    };
    let Some(updated_at) = parse_sqlite_datetime(value) else {
        return 0;
    };
    let age = Utc::now().naive_utc() - updated_at;

    if age <= Duration::days(3) {
        40
    } else if age <= Duration::days(14) {
        24
    } else if age <= Duration::days(30) {
        12
    } else {
        0
    }
}

fn compact_search_text(value: Option<&str>, max_chars: usize) -> Option<String> {
    let normalized = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\r', " ").replace('\n', " "))?;

    if normalized.chars().count() <= max_chars {
        return Some(normalized);
    }

    Some(
        normalized
            .chars()
            .take(max_chars)
            .collect::<String>()
            .trim_end()
            .to_string()
            + "…",
    )
}

fn search_status_label(status: &str) -> &str {
    match status {
        "todo" => "待办",
        "in_progress" => "进行中",
        "review" => "审核中",
        "completed" => "已完成",
        "blocked" => "已阻塞",
        "online" => "在线",
        "busy" => "忙碌",
        "offline" => "离线",
        "error" => "错误",
        "active" => "活跃",
        "archived" => "已归档",
        "pending" => "待启动",
        "running" => "运行中",
        "stopping" => "停止中",
        "exited" => "已结束",
        "failed" => "失败",
        _ => status,
    }
}

fn search_priority_label(priority: &str) -> &str {
    match priority {
        "low" => "低",
        "medium" => "中",
        "high" => "高",
        "urgent" => "紧急",
        _ => priority,
    }
}

fn search_employee_role_label(role: &str) -> &str {
    match role {
        "developer" => "开发者",
        "reviewer" => "审查员",
        "tester" => "测试员",
        "coordinator" => "协调员",
        _ => role,
    }
}

fn search_project_type_label(project_type: &str) -> &str {
    if project_type == PROJECT_TYPE_SSH {
        "SSH 项目"
    } else {
        "本地项目"
    }
}

fn search_session_kind_label(session_kind: &str) -> &str {
    if session_kind == "review" {
        "审核对话"
    } else {
        "执行对话"
    }
}

pub(crate) fn compare_global_search_items(
    left: &GlobalSearchItem,
    right: &GlobalSearchItem,
) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.title.cmp(&right.title))
}

fn build_project_search_item(project: Project, normalized_query: &str) -> Option<GlobalSearchItem> {
    let primary_score = best_match_score(
        normalized_query,
        &[Some(project.name.as_str())],
        1400,
        1100,
        860,
    );
    let secondary_score = best_match_score(
        normalized_query,
        &[
            project.description.as_deref(),
            project.repo_path.as_deref(),
            project.remote_repo_path.as_deref(),
        ],
        720,
        560,
        320,
    );
    let score = primary_score.max(secondary_score)
        + search_recency_bonus(Some(project.updated_at.as_str()));

    if score <= 0 {
        return None;
    }

    Some(GlobalSearchItem {
        item_type: GLOBAL_SEARCH_TYPE_PROJECT.to_string(),
        item_id: project.id.clone(),
        title: project.name.clone(),
        subtitle: Some(format!(
            "{} · {}",
            search_project_type_label(&project.project_type),
            search_status_label(&project.status)
        )),
        summary: compact_search_text(
            project
                .description
                .as_deref()
                .or(project.remote_repo_path.as_deref())
                .or(project.repo_path.as_deref()),
            96,
        ),
        navigation_path: format!("/projects/{}", project.id),
        score,
        updated_at: Some(project.updated_at.clone()),
        project_id: Some(project.id.clone()),
        task_id: None,
        employee_id: None,
        session_id: None,
    })
}

fn build_task_search_item(task: TaskSearchRow, normalized_query: &str) -> Option<GlobalSearchItem> {
    let primary_score = best_match_score(
        normalized_query,
        &[Some(task.title.as_str())],
        1450,
        1180,
        900,
    );
    let alias_score = best_match_score(
        normalized_query,
        &[
            task.description.as_deref(),
            Some(task.project_name.as_str()),
        ],
        760,
        580,
        340,
    );
    let score =
        primary_score.max(alias_score) + search_recency_bonus(Some(task.updated_at.as_str()));

    if score <= 0 {
        return None;
    }

    Some(GlobalSearchItem {
        item_type: GLOBAL_SEARCH_TYPE_TASK.to_string(),
        item_id: task.id.clone(),
        title: task.title.clone(),
        subtitle: Some(format!(
            "{} · {} · {}",
            task.project_name,
            search_status_label(&task.status),
            search_priority_label(&task.priority)
        )),
        summary: compact_search_text(task.description.as_deref(), 110),
        navigation_path: format!("/kanban?taskId={}", task.id),
        score,
        updated_at: Some(task.updated_at.clone()),
        project_id: Some(task.project_id.clone()),
        task_id: Some(task.id.clone()),
        employee_id: None,
        session_id: None,
    })
}

fn build_employee_search_item(
    employee: EmployeeSearchRow,
    normalized_query: &str,
) -> Option<GlobalSearchItem> {
    let primary_score = best_match_score(
        normalized_query,
        &[Some(employee.name.as_str())],
        1380,
        1080,
        820,
    );
    let alias_score = best_match_score(
        normalized_query,
        &[
            employee.specialization.as_deref(),
            Some(employee.role.as_str()),
            employee.project_name.as_deref(),
        ],
        700,
        520,
        300,
    );
    let score =
        primary_score.max(alias_score) + search_recency_bonus(Some(employee.updated_at.as_str()));

    if score <= 0 {
        return None;
    }

    let project_label = employee
        .project_name
        .clone()
        .unwrap_or_else(|| "未分配项目".to_string());

    Some(GlobalSearchItem {
        item_type: GLOBAL_SEARCH_TYPE_EMPLOYEE.to_string(),
        item_id: employee.id.clone(),
        title: employee.name.clone(),
        subtitle: Some(format!(
            "{} · {} · {}",
            search_employee_role_label(&employee.role),
            project_label,
            search_status_label(&employee.status)
        )),
        summary: compact_search_text(employee.specialization.as_deref(), 96),
        navigation_path: format!("/employees?employeeId={}", employee.id),
        score,
        updated_at: Some(employee.updated_at.clone()),
        project_id: employee.project_id.clone(),
        task_id: None,
        employee_id: Some(employee.id.clone()),
        session_id: None,
    })
}

fn build_session_search_item(
    session: CodexSessionListItem,
    normalized_query: &str,
) -> Option<GlobalSearchItem> {
    let primary_score = best_match_score(
        normalized_query,
        &[
            Some(session.display_name.as_str()),
            Some(session.session_id.as_str()),
            session.cli_session_id.as_deref(),
        ],
        1500,
        1200,
        960,
    );
    let secondary_score = best_match_score(
        normalized_query,
        &[
            session.summary.as_deref(),
            session.content_preview.as_deref(),
            session.task_title.as_deref(),
            session.project_name.as_deref(),
            session.employee_name.as_deref(),
            session.working_dir.as_deref(),
        ],
        760,
        600,
        360,
    );
    let score = primary_score.max(secondary_score)
        + search_recency_bonus(Some(session.last_updated_at.as_str()));

    if score <= 0 {
        return None;
    }

    Some(GlobalSearchItem {
        item_type: GLOBAL_SEARCH_TYPE_SESSION.to_string(),
        item_id: session.session_record_id.clone(),
        title: session.display_name.clone(),
        subtitle: Some(format!(
            "{} · {} · {}",
            search_session_kind_label(&session.session_kind),
            search_status_label(&session.status),
            session
                .project_name
                .clone()
                .unwrap_or_else(|| "无关联项目".to_string()),
        )),
        summary: compact_search_text(
            session
                .content_preview
                .as_deref()
                .or(session.summary.as_deref())
                .or(session.task_title.as_deref()),
            110,
        ),
        navigation_path: format!("/sessions?sessionId={}", session.session_id),
        score,
        updated_at: Some(session.last_updated_at.clone()),
        project_id: session.project_id.clone(),
        task_id: session.task_id.clone(),
        employee_id: session.employee_id.clone(),
        session_id: Some(session.session_id.clone()),
    })
}

#[tauri::command]
pub async fn search_global<R: Runtime>(
    app: AppHandle<R>,
    payload: SearchGlobalPayload,
) -> Result<GlobalSearchResponse, String> {
    let normalized_query = normalize_search_query(&payload.query);
    if normalized_query.is_empty() {
        return Ok(GlobalSearchResponse {
            query: payload.query,
            normalized_query,
            state: "empty_query".to_string(),
            message: Some("输入关键词后开始搜索。".to_string()),
            min_query_length: GLOBAL_SEARCH_MIN_QUERY_LENGTH,
            total: 0,
            items: Vec::new(),
        });
    }

    if normalized_query.chars().count() < GLOBAL_SEARCH_MIN_QUERY_LENGTH {
        return Ok(GlobalSearchResponse {
            query: payload.query,
            normalized_query,
            state: "query_too_short".to_string(),
            message: Some(format!(
                "至少输入 {} 个字符后再搜索。",
                GLOBAL_SEARCH_MIN_QUERY_LENGTH
            )),
            min_query_length: GLOBAL_SEARCH_MIN_QUERY_LENGTH,
            total: 0,
            items: Vec::new(),
        });
    }

    let selected_types = normalize_global_search_types(payload.types);
    let environment_mode = match payload.environment_mode.as_deref() {
        Some(EXECUTION_TARGET_SSH) => PROJECT_TYPE_SSH,
        _ => PROJECT_TYPE_LOCAL,
    };
    let limit = payload
        .limit
        .unwrap_or(GLOBAL_SEARCH_DEFAULT_LIMIT)
        .clamp(1, GLOBAL_SEARCH_MAX_LIMIT);
    let offset = payload.offset.unwrap_or(0);
    let pool = sqlite_pool(&app).await?;
    let mut items = Vec::new();

    if selected_types.contains(GLOBAL_SEARCH_TYPE_PROJECT) {
        let projects = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE project_type = $1 ORDER BY updated_at DESC, created_at DESC",
        )
        .bind(environment_mode)
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("Failed to fetch searchable projects: {}", error))?;

        items.extend(
            projects
                .into_iter()
                .filter_map(|project| build_project_search_item(project, &normalized_query)),
        );
    }

    if selected_types.contains(GLOBAL_SEARCH_TYPE_TASK) {
        let tasks = sqlx::query_as::<_, TaskSearchRow>(
            r#"
            SELECT
                t.id AS id,
                t.title AS title,
                t.description AS description,
                t.status AS status,
                t.priority AS priority,
                t.project_id AS project_id,
                p.name AS project_name,
                t.updated_at AS updated_at
            FROM tasks t
            INNER JOIN projects p ON p.id = t.project_id
            WHERE p.project_type = $1
            ORDER BY t.updated_at DESC, t.created_at DESC
            "#,
        )
        .bind(environment_mode)
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("Failed to fetch searchable tasks: {}", error))?;

        items.extend(
            tasks
                .into_iter()
                .filter_map(|task| build_task_search_item(task, &normalized_query)),
        );
    }

    if selected_types.contains(GLOBAL_SEARCH_TYPE_EMPLOYEE) {
        let employees = sqlx::query_as::<_, EmployeeSearchRow>(
            r#"
            SELECT
                e.id AS id,
                e.name AS name,
                e.role AS role,
                e.specialization AS specialization,
                e.status AS status,
                e.project_id AS project_id,
                p.name AS project_name,
                e.updated_at AS updated_at
            FROM employees e
            LEFT JOIN projects p ON p.id = e.project_id
            WHERE e.project_id IS NULL OR p.project_type = $1
            ORDER BY e.updated_at DESC, e.created_at DESC
            "#,
        )
        .bind(environment_mode)
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("Failed to fetch searchable employees: {}", error))?;

        items.extend(
            employees
                .into_iter()
                .filter_map(|employee| build_employee_search_item(employee, &normalized_query)),
        );
    }

    if selected_types.contains(GLOBAL_SEARCH_TYPE_SESSION) {
        let sessions = query_codex_session_list(&app).await?;
        items.extend(
            sessions
                .into_iter()
                .filter(|session| session.execution_target == environment_mode)
                .filter_map(|session| build_session_search_item(session, &normalized_query)),
        );
    }

    items.sort_by(compare_global_search_items);
    let total = items.len();
    let items = items.into_iter().skip(offset).take(limit).collect();

    Ok(GlobalSearchResponse {
        query: payload.query,
        normalized_query,
        state: "ok".to_string(),
        message: None,
        min_query_length: GLOBAL_SEARCH_MIN_QUERY_LENGTH,
        total,
        items,
    })
}

async fn query_codex_session_list<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<CodexSessionListItem>, String> {
    let pool = sqlite_pool(app).await?;
    sqlx::query_as::<_, CodexSessionListItem>(
        r#"
        SELECT
            s.id AS session_record_id,
            COALESCE(s.cli_session_id, s.id) AS session_id,
            s.cli_session_id AS cli_session_id,
            s.ai_provider AS ai_provider,
            s.session_kind AS session_kind,
            s.status AS status,
            COALESCE(
                (
                    SELECT MAX(e.created_at)
                    FROM codex_session_events e
                    WHERE e.session_id = s.id
                ),
                s.ended_at,
                s.started_at,
                s.created_at
            ) AS last_updated_at,
            COALESCE(
                t.title,
                CASE
                    WHEN s.session_kind = 'review' THEN '代码审核对话'
                    ELSE 'Codex 执行对话'
                END
            ) AS display_name,
            CASE
                WHEN t.title IS NOT NULL AND p.name IS NOT NULL THEN p.name || ' · ' || t.title
                WHEN p.name IS NOT NULL THEN p.name
                WHEN s.working_dir IS NOT NULL THEN s.working_dir
                ELSE NULL
            END AS summary,
            SUBSTR(
                (
                    SELECT GROUP_CONCAT(message, ' ')
                    FROM (
                        SELECT TRIM(REPLACE(REPLACE(e.message, char(10), ' '), char(13), ' ')) AS message
                        FROM codex_session_events e
                        WHERE e.session_id = s.id
                          AND e.message IS NOT NULL
                          AND TRIM(e.message) <> ''
                        ORDER BY e.created_at DESC
                        LIMIT 5
                    )
                ),
                1,
                600
            ) AS content_preview,
            s.employee_id AS employee_id,
            e.name AS employee_name,
            s.task_id AS task_id,
            t.title AS task_title,
            t.status AS task_status,
            s.project_id AS project_id,
            p.name AS project_name,
            s.working_dir AS working_dir,
            s.execution_target AS execution_target,
            s.ssh_config_id AS ssh_config_id,
            s.target_host_label AS target_host_label,
            s.artifact_capture_mode AS artifact_capture_mode,
            '' AS resume_status,
            NULL AS resume_message,
            0 AS can_resume
        FROM codex_sessions s
        LEFT JOIN employees e ON e.id = s.employee_id
        LEFT JOIN tasks t ON t.id = s.task_id
        LEFT JOIN projects p ON p.id = s.project_id
        ORDER BY last_updated_at DESC, s.created_at DESC
        "#,
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to fetch codex sessions: {}", error))
}

#[tauri::command]
pub async fn list_codex_sessions<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    claude_state: State<'_, Arc<tokio::sync::Mutex<ClaudeManager>>>,
) -> Result<Vec<CodexSessionListItem>, String> {
    let mut items = query_codex_session_list(&app).await?;
    let employee_ids = items
        .iter()
        .filter_map(|item| item.employee_id.clone())
        .collect::<HashSet<_>>();
    let mut running_by_employee = HashMap::new();
    let mut running_by_task_session = HashSet::new();

    for employee_id in employee_ids {
        let live_processes =
            crate::codex::list_live_employee_processes(&app, state.inner(), &employee_id).await?;
        let live_claude_processes =
            crate::claude::list_live_claude_employee_processes(claude_state.inner(), &employee_id)
                .await;
        running_by_employee.insert(
            employee_id.clone(),
            !live_processes.is_empty() || !live_claude_processes.is_empty(),
        );
        for process in live_processes {
            if let Some(task_id) = process.task_id.as_deref() {
                running_by_task_session.insert(running_task_session_key(
                    task_id,
                    process.session_kind.as_str(),
                ));
            }
        }
        for process in live_claude_processes {
            if let Some(task_id) = process.task_id.as_deref() {
                running_by_task_session.insert(running_task_session_key(
                    task_id,
                    process.session_kind.as_str(),
                ));
            }
        }
    }

    for item in &mut items {
        let has_running_conflict = item
            .task_id
            .as_ref()
            .map(|task_id| {
                running_by_task_session
                    .contains(&running_task_session_key(task_id, &item.session_kind))
            })
            .unwrap_or_else(|| {
                item.employee_id
                    .as_ref()
                    .and_then(|employee_id| running_by_employee.get(employee_id))
                    .copied()
                    .unwrap_or(false)
            });
        let (resume_status, resume_message, can_resume) = resolve_session_resume_state(
            item.cli_session_id.as_deref(),
            item.employee_id.as_deref(),
            item.employee_name.as_deref(),
            &item.status,
            has_running_conflict,
            resolve_running_conflict_message(item.task_id.as_deref()),
        );
        item.resume_status = resume_status;
        item.resume_message = resume_message;
        item.can_resume = can_resume;
    }
    Ok(items)
}

#[tauri::command]
pub async fn prepare_codex_session_resume<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    claude_state: State<'_, Arc<tokio::sync::Mutex<ClaudeManager>>>,
    session_id: String,
) -> Result<CodexSessionResumePreview, String> {
    let mut items = query_codex_session_list(&app).await?;
    let item = items.drain(..).find(|current| {
        current.session_id == session_id || current.session_record_id == session_id
    });

    let Some(item) = item else {
        return Ok(CodexSessionResumePreview {
            requested_session_id: session_id,
            resolved_session_id: None,
            session_record_id: None,
            ai_provider: None,
            session_kind: None,
            session_status: None,
            display_name: None,
            summary: None,
            employee_id: None,
            employee_name: None,
            task_id: None,
            task_title: None,
            project_id: None,
            project_name: None,
            working_dir: None,
            execution_target: None,
            ssh_config_id: None,
            target_host_label: None,
            artifact_capture_mode: None,
            resume_status: "invalid".to_string(),
            resume_message: Some("无效对话 ID，未找到对应对话。".to_string()),
            can_resume: false,
        });
    };

    let has_running_conflict = has_running_session_conflict(
        &app,
        state.inner(),
        claude_state.inner(),
        item.employee_id.as_deref(),
        item.task_id.as_deref(),
        &item.session_kind,
    )
    .await?;
    let (resume_status, resume_message, can_resume) = resolve_session_resume_state(
        item.cli_session_id.as_deref(),
        item.employee_id.as_deref(),
        item.employee_name.as_deref(),
        &item.status,
        has_running_conflict,
        resolve_running_conflict_message(item.task_id.as_deref()),
    );

    Ok(CodexSessionResumePreview {
        requested_session_id: session_id,
        resolved_session_id: item.cli_session_id.clone(),
        session_record_id: Some(item.session_record_id),
        ai_provider: Some(item.ai_provider),
        session_kind: Some(item.session_kind),
        session_status: Some(item.status),
        display_name: Some(item.display_name),
        summary: item.summary,
        employee_id: item.employee_id,
        employee_name: item.employee_name,
        task_id: item.task_id,
        task_title: item.task_title,
        project_id: item.project_id,
        project_name: item.project_name,
        working_dir: item.working_dir,
        execution_target: Some(item.execution_target),
        ssh_config_id: item.ssh_config_id,
        target_host_label: item.target_host_label,
        artifact_capture_mode: Some(item.artifact_capture_mode),
        resume_status,
        resume_message,
        can_resume,
    })
}

#[tauri::command]
pub async fn get_task_latest_review<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Option<TaskLatestReview>, String> {
    let pool = sqlite_pool(&app).await?;
    let session = sqlx::query_as::<_, CodexSessionRecord>(
        "SELECT * FROM codex_sessions WHERE task_id = $1 AND session_kind = 'review' ORDER BY started_at DESC LIMIT 1",
    )
    .bind(&task_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch task latest review session: {}", error))?;

    let Some(session) = session else {
        return Ok(None);
    };

    let report = sqlx::query_scalar::<_, Option<String>>(
        "SELECT message FROM codex_session_events WHERE session_id = $1 AND event_type = 'review_report' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&session.id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch task latest review report: {}", error))?
    .flatten();

    let reviewer_name = match session.employee_id.as_deref() {
        Some(employee_id) => sqlx::query_scalar::<_, Option<String>>(
            "SELECT name FROM employees WHERE id = $1 LIMIT 1",
        )
        .bind(employee_id)
        .fetch_optional(&pool)
        .await
        .map_err(|error| format!("Failed to fetch task reviewer name: {}", error))?
        .flatten(),
        None => None,
    };

    Ok(Some(TaskLatestReview {
        session,
        report,
        reviewer_name,
    }))
}

async fn resolve_execution_session_capture_mode(
    pool: &SqlitePool,
    session_id: &str,
    changes: &[CodexSessionFileChange],
) -> Result<String, String> {
    if let Some(change) = changes.first() {
        return Ok(change.capture_mode.clone());
    }

    let session_started_message = sqlx::query_scalar::<_, Option<String>>(
        "SELECT message FROM codex_session_events WHERE session_id = $1 AND event_type = 'session_started' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch session provider info: {}", error))?
    .flatten()
    .unwrap_or_default();

    if session_started_message.contains("通过 SDK 启动") {
        Ok("sdk_event".to_string())
    } else {
        Ok("git_fallback".to_string())
    }
}

async fn build_execution_change_history_item(
    pool: &SqlitePool,
    session: CodexSessionRecord,
) -> Result<TaskExecutionChangeHistoryItem, String> {
    let changes = sqlx::query_as::<_, CodexSessionFileChange>(
        "SELECT * FROM codex_session_file_changes WHERE session_id = $1 ORDER BY path ASC, created_at ASC",
    )
    .bind(&session.id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch task execution file changes: {}", error))?;

    let capture_mode = resolve_execution_session_capture_mode(pool, &session.id, &changes).await?;

    Ok(TaskExecutionChangeHistoryItem {
        session,
        capture_mode,
        changes,
    })
}

pub(crate) async fn fetch_execution_change_history_item_by_session_id(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<TaskExecutionChangeHistoryItem, String> {
    let session = sqlx::query_as::<_, CodexSessionRecord>(
        r#"
        SELECT *
        FROM codex_sessions
        WHERE id = $1 OR cli_session_id = $1
        ORDER BY CASE WHEN id = $1 THEN 0 ELSE 1 END, started_at DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch execution session: {}", error))?
    .ok_or_else(|| "找不到对应的 Session 记录".to_string())?;

    if session.session_kind != "execution" {
        return Err("只有 execution 会话支持查看改动文件".to_string());
    }

    build_execution_change_history_item(pool, session).await
}

#[tauri::command]
pub async fn get_task_execution_change_history<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Vec<TaskExecutionChangeHistoryItem>, String> {
    let pool = sqlite_pool(&app).await?;
    let sessions = sqlx::query_as::<_, CodexSessionRecord>(
        "SELECT * FROM codex_sessions WHERE task_id = $1 AND session_kind = 'execution' ORDER BY started_at DESC",
    )
    .bind(&task_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to fetch task execution sessions: {}", error))?;

    let mut items = Vec::with_capacity(sessions.len());
    for session in sessions {
        items.push(build_execution_change_history_item(&pool, session).await?);
    }

    Ok(items)
}

#[tauri::command]
pub async fn get_codex_session_execution_change_history<R: Runtime>(
    app: AppHandle<R>,
    session_id: String,
) -> Result<TaskExecutionChangeHistoryItem, String> {
    let pool = sqlite_pool(&app).await?;
    fetch_execution_change_history_item_by_session_id(&pool, &session_id).await
}

fn build_file_change_diff_preview(
    before_label: &str,
    before_text: Option<&str>,
    after_label: &str,
    after_text: Option<&str>,
) -> Result<(Option<String>, bool), String> {
    if before_text.is_none() && after_text.is_none() {
        return Ok((None, false));
    }

    let temp_dir = std::env::temp_dir().join(format!("codex-ai-diff-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).map_err(|error| format!("创建 diff 临时目录失败: {}", error))?;
    let before_file = temp_dir.join("before.txt");
    let after_file = temp_dir.join("after.txt");

    let write_result = (|| -> Result<(), String> {
        fs::write(&before_file, before_text.unwrap_or(""))
            .map_err(|error| format!("写入 diff 前镜像失败: {}", error))?;
        fs::write(&after_file, after_text.unwrap_or(""))
            .map_err(|error| format!("写入 diff 后镜像失败: {}", error))?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(error);
    }

    let mut command = Command::new("git");
    configure_std_command(&mut command);
    let output = command
        .current_dir(&temp_dir)
        .args([
            "diff",
            "--no-index",
            "--no-ext-diff",
            "--unified=3",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--",
            "before.txt",
            "after.txt",
        ])
        .output()
        .map_err(|error| format!("生成文件 diff 失败: {}", error))?;
    let _ = fs::remove_dir_all(&temp_dir);

    if !output.status.success() && output.status.code() != Some(1) {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let diff = rewrite_file_change_diff_labels(
        &String::from_utf8_lossy(&output.stdout),
        before_label,
        after_label,
    );
    let trimmed = diff.trim();
    if trimmed.is_empty() {
        return Ok((None, false));
    }

    let (diff_text, diff_truncated) = truncate_review_text(trimmed, FILE_CHANGE_DIFF_CHAR_LIMIT);
    Ok((Some(diff_text), diff_truncated))
}

fn file_change_diff_display_label(prefix: &str, label: &str) -> String {
    if label == "/dev/null" {
        label.to_string()
    } else {
        format!("{prefix}/{label}")
    }
}

pub(crate) fn rewrite_file_change_diff_labels(
    diff: &str,
    before_label: &str,
    after_label: &str,
) -> String {
    let before_display = file_change_diff_display_label("a", before_label);
    let after_display = file_change_diff_display_label("b", after_label);

    diff.lines()
        .map(|line| {
            if line == "diff --git a/before.txt b/after.txt" {
                format!("diff --git {} {}", before_display, after_display)
            } else if line == "--- a/before.txt" {
                format!("--- {}", before_display)
            } else if line == "+++ b/after.txt" {
                format!("+++ {}", after_display)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tauri::command]
pub async fn get_codex_session_file_change_detail<R: Runtime>(
    app: AppHandle<R>,
    change_id: String,
) -> Result<CodexSessionFileChangeDetail, String> {
    let pool = sqlite_pool(&app).await?;
    let change = sqlx::query_as::<_, CodexSessionFileChange>(
        "SELECT * FROM codex_session_file_changes WHERE id = $1",
    )
    .bind(&change_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch session file change: {}", error))?
    .ok_or_else(|| "找不到对应的文件变更记录".to_string())?;
    let session =
        sqlx::query_as::<_, CodexSessionRecord>("SELECT * FROM codex_sessions WHERE id = $1")
            .bind(&change.session_id)
            .fetch_optional(&pool)
            .await
            .map_err(|error| {
                format!(
                    "Failed to fetch session record for change detail: {}",
                    error
                )
            })?
            .ok_or_else(|| "找不到对应的执行会话".to_string())?;
    let detail = sqlx::query_as::<_, CodexSessionFileChangeDetailRecord>(
        "SELECT * FROM codex_session_file_change_details WHERE change_id = $1",
    )
    .bind(&change.id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch session file change detail: {}", error))?;

    let fallback_absolute_path = session
        .working_dir
        .as_ref()
        .map(|dir| path_to_runtime_string(&Path::new(dir).join(&change.path)));
    let fallback_previous_absolute_path = session
        .working_dir
        .as_ref()
        .zip(change.previous_path.as_ref())
        .map(|(dir, path)| path_to_runtime_string(&Path::new(dir).join(path)));

    let Some(detail) = detail else {
        return Ok(CodexSessionFileChangeDetail {
            change,
            working_dir: session.working_dir,
            absolute_path: fallback_absolute_path,
            previous_absolute_path: fallback_previous_absolute_path,
            before_status: "missing".to_string(),
            before_text: None,
            before_truncated: false,
            after_status: "missing".to_string(),
            after_text: None,
            after_truncated: false,
            diff_text: None,
            diff_truncated: false,
            snapshot_status: "unavailable".to_string(),
            snapshot_message: Some(
                "该执行记录生成于旧版本，只保留了文件级变更，没有保存可预览的文本快照。"
                    .to_string(),
            ),
        });
    };

    let before_label = if change.change_type == "added" {
        "/dev/null"
    } else {
        change
            .previous_path
            .as_deref()
            .unwrap_or(change.path.as_str())
    };
    let after_label = if change.change_type == "deleted" {
        "/dev/null"
    } else {
        change.path.as_str()
    };
    let can_build_diff = (detail.before_status == "text" && detail.before_text.is_some())
        || (detail.after_status == "text" && detail.after_text.is_some());
    let (diff_text, raw_diff_truncated) = if can_build_diff {
        build_file_change_diff_preview(
            before_label,
            detail.before_text.as_deref(),
            after_label,
            detail.after_text.as_deref(),
        )?
    } else {
        (None, false)
    };
    let diff_truncated =
        raw_diff_truncated || detail.before_truncated != 0 || detail.after_truncated != 0;

    Ok(CodexSessionFileChangeDetail {
        change,
        working_dir: session.working_dir,
        absolute_path: detail.absolute_path.or(fallback_absolute_path),
        previous_absolute_path: detail
            .previous_absolute_path
            .or(fallback_previous_absolute_path),
        before_status: detail.before_status,
        before_text: detail.before_text,
        before_truncated: detail.before_truncated != 0,
        after_status: detail.after_status,
        after_text: detail.after_text,
        after_truncated: detail.after_truncated != 0,
        diff_text,
        diff_truncated,
        snapshot_status: "ready".to_string(),
        snapshot_message: None,
    })
}
