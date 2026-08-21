// Domain slice: review_data (included into task_automation)

async fn fetch_task_session_ids(
    pool: &SqlitePool,
    task_id: &str,
    session_kind: &str,
) -> Result<TaskSessionIds, String> {
    let ids = sqlx::query_scalar::<_, String>(
        "SELECT id FROM codex_sessions WHERE task_id = $1 AND session_kind = $2",
    )
    .bind(task_id)
    .bind(session_kind)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch task session ids: {}", error))?;
    Ok(TaskSessionIds {
        ids: ids.into_iter().collect(),
    })
}

async fn resolve_new_task_session_id(
    pool: &SqlitePool,
    task_id: &str,
    session_kind: &str,
    existing_ids: &TaskSessionIds,
) -> Result<String, String> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM codex_sessions
        WHERE task_id = $1 AND session_kind = $2
        ORDER BY created_at DESC, started_at DESC, id DESC
        "#,
    )
    .bind(task_id)
    .bind(session_kind)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to resolve new task session id: {}", error))?;

    rows.into_iter()
        .find(|session_id| !existing_ids.contains(session_id))
        .ok_or_else(|| {
            format!(
                "Failed to resolve new {} session id for task {}",
                session_kind, task_id
            )
        })
}

async fn review_report_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let stored_report = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT message
        FROM codex_session_events
        WHERE session_id = $1
          AND event_type = 'review_report'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch review report for session: {}", error))
    .map(|value| value.flatten())?;

    let recovered_report = recover_review_report_for_session(pool, session_id).await?;
    Ok(stored_report.or(recovered_report))
}

async fn review_raw_output_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let raw = crate::app::collect_session_output_text(pool, session_id).await?;
    if raw.is_empty() {
        Ok(None)
    } else {
        Ok(Some(raw))
    }
}

async fn recover_review_verdict_json_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let stored_verdict = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT message
        FROM codex_session_events
        WHERE session_id = $1
          AND event_type = 'review_verdict'
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch review verdict for session: {}", error))?
    .flatten();
    if let Some(verdict_json) = stored_verdict {
        if parse_review_verdict_json(&verdict_json).is_ok() {
            return Ok(Some(verdict_json));
        }
    }

    let raw_output = review_raw_output_for_session(pool, session_id).await?;
    let recovered = raw_output
        .as_deref()
        .and_then(extract_review_verdict)
        .filter(|value| parse_review_verdict_json(value).is_ok());
    Ok(recovered)
}

async fn recover_fix_verdict_json_for_task(
    pool: &SqlitePool,
    task_id: &str,
    state_record: &TaskAutomationStateRecord,
) -> Result<Option<String>, String> {
    if let Some(verdict_json) = state_record.last_verdict_json.as_deref() {
        if parse_review_verdict_json(verdict_json).is_ok() {
            return Ok(Some(verdict_json.to_string()));
        }
    }

    let mut candidates = Vec::new();
    for session_id in [
        state_record.consumed_session_id.clone(),
        state_record.last_trigger_session_id.clone(),
    ]
    .into_iter()
    .flatten()
    {
        if !candidates.iter().any(|item| item == &session_id) {
            candidates.push(session_id);
        }
    }

    let latest_review = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM codex_sessions
        WHERE task_id = $1 AND session_kind = 'review'
        ORDER BY created_at DESC, started_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch latest review session: {}", error))?;
    if let Some(session_id) = latest_review {
        if !candidates.iter().any(|item| item == &session_id) {
            candidates.push(session_id);
        }
    }

    for session_id in candidates {
        if let Some(json) = recover_review_verdict_json_for_session(pool, &session_id).await? {
            return Ok(Some(json));
        }
    }
    Ok(None)
}

async fn recover_review_report_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let raw_output = review_raw_output_for_session(pool, session_id).await?;
    Ok(raw_output.as_deref().and_then(extract_review_report))
}

async fn fetch_task_subtasks(pool: &SqlitePool, task_id: &str) -> Result<Vec<Subtask>, String> {
    sqlx::query_as::<_, Subtask>("SELECT * FROM subtasks WHERE task_id = $1 ORDER BY sort_order")
        .bind(task_id)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("Failed to fetch task subtasks: {}", error))
}

async fn fetch_task_attachments(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<TaskAttachment>, String> {
    sqlx::query_as::<_, TaskAttachment>(
        "SELECT * FROM task_attachments WHERE task_id = $1 ORDER BY sort_order, created_at",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch task attachments: {}", error))
}

