use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tauri::{AppHandle, Runtime};

use crate::app::{
    new_id, parse_activity_date_bound, resolve_scoped_project_ids_for_stats, sqlite_pool,
};
use crate::db::models::{
    ListNativeApiCallLogsPayload, NativeApiCallLogDetail, NativeApiCallLogListItem,
    NativeApiCallLogPage, NativeApiCallLogStats,
};
use crate::native::model::call_log::{redact_and_truncate_text, NativeApiCallLogInsert};

const LIST_API_CALL_LOGS_DEFAULT_LIMIT: i64 = 20;
const LIST_API_CALL_LOGS_MAX_LIMIT: i64 = 500;

const LIST_SELECT: &str = r#"
SELECT
    l.id, l.call_id, l.attempt, l.channel_id, l.channel_name, l.protocol, l.response_encoding,
    l.model, l.thinking_enabled, l.thinking_level, l.request_format, l.input_tokens,
    l.output_tokens, l.cached_tokens, l.total_tokens, l.first_token_ms, l.duration_ms,
    l.status, l.http_status, l.session_id, l.employee_id, l.task_id, l.project_id,
    p.name AS project_name, e.name AS employee_name, l.execution_target, l.call_kind, l.created_at
FROM native_api_call_logs l
LEFT JOIN projects p ON p.id = l.project_id
LEFT JOIN employees e ON e.id = l.employee_id
"#;

const DETAIL_SELECT: &str = r#"
SELECT
    l.id, l.call_id, l.attempt, l.channel_id, l.channel_name, l.protocol, l.response_encoding,
    l.model, l.thinking_enabled, l.thinking_level, l.request_format, l.request_body,
    l.request_truncated, l.response_body, l.response_truncated, l.input_tokens,
    l.output_tokens, l.cached_tokens, l.total_tokens, l.first_token_ms, l.duration_ms,
    l.status, l.http_status, l.error_message, l.session_id, l.employee_id, l.task_id,
    l.project_id, p.name AS project_name, e.name AS employee_name, l.subagent_id,
    l.call_kind, l.execution_target, l.created_at
FROM native_api_call_logs l
LEFT JOIN projects p ON p.id = l.project_id
LEFT JOIN employees e ON e.id = l.employee_id
"#;

pub fn spawn_insert_native_api_call_log(pool: SqlitePool, record: NativeApiCallLogInsert) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = insert_native_api_call_log(&pool, &record).await {
            eprintln!("[native] 写入 API 调用记录失败: {error}");
        }
    });
}

pub async fn insert_native_api_call_log(
    pool: &SqlitePool,
    record: &NativeApiCallLogInsert,
) -> Result<String, String> {
    let id = if record.id.trim().is_empty() {
        new_id()
    } else {
        record.id.clone()
    };
    let request = record.request_body.as_deref().map(redact_and_truncate_text);
    let response = record
        .response_body
        .as_deref()
        .map(redact_and_truncate_text);
    let error_message = record.error_message.as_deref().map(|text| {
        let redacted = redact_and_truncate_text(text);
        redacted.text
    });
    let request_truncated =
        record.request_truncated || request.as_ref().is_some_and(|item| item.truncated);
    let response_truncated =
        record.response_truncated || response.as_ref().is_some_and(|item| item.truncated);

    sqlx::query(
        r#"
        INSERT INTO native_api_call_logs (
            id, call_id, attempt, channel_id, channel_name, protocol, response_encoding,
            model, thinking_enabled, thinking_level, request_format, request_body,
            request_truncated, response_body, response_truncated, input_tokens,
            output_tokens, cached_tokens, total_tokens, first_token_ms, duration_ms,
            status, http_status, error_message, session_id, employee_id, task_id,
            project_id, subagent_id, call_kind, execution_target
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7,
            $8, $9, $10, $11, $12,
            $13, $14, $15, $16,
            $17, $18, $19, $20, $21,
            $22, $23, $24, $25, $26, $27,
            $28, $29, $30, $31
        )
        "#,
    )
    .bind(&id)
    .bind(&record.call_id)
    .bind(record.attempt)
    .bind(record.channel_id.as_deref())
    .bind(record.channel_name.as_deref())
    .bind(&record.protocol)
    .bind(record.response_encoding.as_deref())
    .bind(record.model.as_deref())
    .bind(if record.thinking_enabled { 1 } else { 0 })
    .bind(record.thinking_level.as_deref())
    .bind(&record.request_format)
    .bind(request.as_ref().map(|item| item.text.as_str()))
    .bind(if request_truncated { 1 } else { 0 })
    .bind(response.as_ref().map(|item| item.text.as_str()))
    .bind(if response_truncated { 1 } else { 0 })
    .bind(record.input_tokens)
    .bind(record.output_tokens)
    .bind(record.cached_tokens)
    .bind(record.total_tokens)
    .bind(record.first_token_ms)
    .bind(record.duration_ms)
    .bind(&record.status)
    .bind(record.http_status)
    .bind(error_message.as_deref())
    .bind(record.session_id.as_deref())
    .bind(record.employee_id.as_deref())
    .bind(record.task_id.as_deref())
    .bind(record.project_id.as_deref())
    .bind(record.subagent_id.as_deref())
    .bind(record.call_kind.as_deref())
    .bind(record.execution_target.as_deref())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to insert native API call log: {error}"))?;

    Ok(id)
}

pub fn sqlite_call_log_sink(pool: SqlitePool) -> crate::native::model::client::CallLogSink {
    std::sync::Arc::new(move |record: NativeApiCallLogInsert| {
        spawn_insert_native_api_call_log(pool.clone(), record);
    })
}

fn empty_stats() -> NativeApiCallLogStats {
    NativeApiCallLogStats::default()
}

fn empty_page() -> NativeApiCallLogPage {
    NativeApiCallLogPage {
        items: Vec::new(),
        total: 0,
        stats: empty_stats(),
    }
}

fn escape_sql_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn optional_trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|item| !item.is_empty())
}

fn date_range_invalid(payload: &ListNativeApiCallLogsPayload) -> bool {
    let start = payload
        .start_date
        .as_deref()
        .and_then(|date| parse_activity_date_bound(date, false));
    let end = payload
        .end_date
        .as_deref()
        .and_then(|date| parse_activity_date_bound(date, true));
    matches!((start, end), (Some(start), Some(end)) if start > end)
}

fn push_and(builder: &mut QueryBuilder<'_, Sqlite>, has_where: &mut bool) {
    if *has_where {
        builder.push(" AND ");
    } else {
        builder.push(" WHERE ");
        *has_where = true;
    }
}

fn scoped_environment_mode(payload: &ListNativeApiCallLogsPayload) -> &str {
    optional_trimmed(payload.environment_mode.as_deref()).unwrap_or("local")
}

fn scoped_execution_target(payload: &ListNativeApiCallLogsPayload) -> Option<String> {
    optional_trimmed(payload.execution_target.as_deref())
        .or_else(|| optional_trimmed(payload.environment_mode.as_deref()))
        .map(str::to_ascii_lowercase)
}

fn requested_scope_project_id(payload: &ListNativeApiCallLogsPayload) -> Option<&str> {
    optional_trimmed(payload.project_id.as_deref())
}

fn include_unscoped_project_logs(payload: &ListNativeApiCallLogsPayload) -> bool {
    requested_scope_project_id(payload).is_none()
}

fn push_project_scope(
    builder: &mut QueryBuilder<'_, Sqlite>,
    payload: &ListNativeApiCallLogsPayload,
    scoped_ids: &[String],
) {
    let include_unscoped = include_unscoped_project_logs(payload);
    if scoped_ids.is_empty() && !include_unscoped {
        builder.push("1 = 0");
        return;
    }

    builder.push("(");
    let mut wrote_any = false;
    if !scoped_ids.is_empty() {
        builder.push("l.project_id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in scoped_ids {
                separated.push_bind(id.clone());
            }
        }
        builder.push(")");
        wrote_any = true;
    }
    if include_unscoped {
        if wrote_any {
            builder.push(" OR ");
        }
        builder.push("(l.project_id IS NULL AND LOWER(COALESCE(l.execution_target, 'local')) = ");
        builder.push_bind(scoped_environment_mode(payload).to_ascii_lowercase());
        builder.push(")");
    }
    builder.push(")");
}

fn push_scope_and_filters(
    builder: &mut QueryBuilder<'_, Sqlite>,
    payload: &ListNativeApiCallLogsPayload,
    scoped_ids: &[String],
) {
    let mut has_where = false;
    if scoped_ids.is_empty() && !include_unscoped_project_logs(payload) {
        push_and(builder, &mut has_where);
        builder.push("1 = 0");
        return;
    }

    push_and(builder, &mut has_where);
    push_project_scope(builder, payload, scoped_ids);

    if let Some(channel_name) = optional_trimmed(payload.channel_name.as_deref()) {
        push_and(builder, &mut has_where);
        builder.push("LOWER(COALESCE(l.channel_name, '')) LIKE ");
        builder.push_bind(format!(
            "%{}%",
            escape_sql_like(&channel_name.to_ascii_lowercase())
        ));
        builder.push(" ESCAPE '\\'");
    }
    if let Some(model) = optional_trimmed(payload.model.as_deref()) {
        push_and(builder, &mut has_where);
        builder.push("LOWER(COALESCE(l.model, '')) LIKE ");
        builder.push_bind(format!(
            "%{}%",
            escape_sql_like(&model.to_ascii_lowercase())
        ));
        builder.push(" ESCAPE '\\'");
    }
    if let Some(status) = optional_trimmed(payload.status.as_deref()) {
        push_and(builder, &mut has_where);
        builder.push("l.status = ");
        builder.push_bind(status.to_string());
    }
    if let Some(session_id) = optional_trimmed(payload.session_id.as_deref()) {
        push_and(builder, &mut has_where);
        builder.push("l.session_id = ");
        builder.push_bind(session_id.to_string());
    }
    if let Some(execution_target) = scoped_execution_target(payload) {
        push_and(builder, &mut has_where);
        builder.push("LOWER(COALESCE(l.execution_target, 'local')) = ");
        builder.push_bind(execution_target);
    }
    if let Some(start) = payload
        .start_date
        .as_deref()
        .and_then(|date| parse_activity_date_bound(date, false))
    {
        push_and(builder, &mut has_where);
        builder.push("CAST(strftime('%s', l.created_at) AS INTEGER) >= ");
        builder.push_bind(start);
    }
    if let Some(end) = payload
        .end_date
        .as_deref()
        .and_then(|date| parse_activity_date_bound(date, true))
    {
        push_and(builder, &mut has_where);
        builder.push("CAST(strftime('%s', l.created_at) AS INTEGER) <= ");
        builder.push_bind(end);
    }
}

pub(crate) async fn list_native_api_call_logs_with_pool(
    pool: &SqlitePool,
    payload: &ListNativeApiCallLogsPayload,
) -> Result<NativeApiCallLogPage, String> {
    if date_range_invalid(payload) {
        return Ok(empty_page());
    }

    let environment_mode = optional_trimmed(payload.environment_mode.as_deref());
    let selected_ssh = optional_trimmed(payload.selected_ssh_config_id.as_deref());
    let project_id = optional_trimmed(payload.project_id.as_deref());
    let scoped_ids =
        resolve_scoped_project_ids_for_stats(pool, environment_mode, selected_ssh, project_id)
            .await?;
    if scoped_ids.is_empty() && project_id.is_some() {
        return Ok(empty_page());
    }

    let limit = payload
        .limit
        .filter(|value| *value > 0)
        .unwrap_or(LIST_API_CALL_LOGS_DEFAULT_LIMIT)
        .clamp(1, LIST_API_CALL_LOGS_MAX_LIMIT);
    let offset = payload.offset.unwrap_or(0).max(0);
    let include_total = payload.include_total.unwrap_or(true);

    let mut items_q = QueryBuilder::<Sqlite>::new(LIST_SELECT);
    push_scope_and_filters(&mut items_q, payload, &scoped_ids);
    items_q.push(" ORDER BY l.created_at DESC, l.id DESC LIMIT ");
    items_q.push_bind(limit);
    items_q.push(" OFFSET ");
    items_q.push_bind(offset);
    let items: Vec<NativeApiCallLogListItem> = items_q
        .build_query_as::<NativeApiCallLogListItem>()
        .fetch_all(pool)
        .await
        .map_err(|error| format!("获取 API 调用记录失败: {error}"))?;

    let mut stats_q = QueryBuilder::<Sqlite>::new(
        "SELECT COUNT(*) AS call_count, \
         SUM(l.input_tokens) AS input_tokens_sum, \
         SUM(l.output_tokens) AS output_tokens_sum, \
         SUM(l.cached_tokens) AS cached_tokens_sum, \
         SUM(l.total_tokens) AS total_tokens_sum, \
         AVG(l.first_token_ms) AS avg_first_token_ms, \
         AVG(l.duration_ms) AS avg_duration_ms \
         FROM native_api_call_logs l \
         LEFT JOIN projects p ON p.id = l.project_id \
         LEFT JOIN employees e ON e.id = l.employee_id",
    );
    push_scope_and_filters(&mut stats_q, payload, &scoped_ids);
    let stats: NativeApiCallLogStats = stats_q
        .build_query_as::<NativeApiCallLogStats>()
        .fetch_one(pool)
        .await
        .map_err(|error| format!("统计 API 调用记录失败: {error}"))?;
    let total = if include_total { stats.call_count } else { 0 };

    Ok(NativeApiCallLogPage {
        items,
        total,
        stats,
    })
}

pub(crate) async fn get_native_api_call_log_with_pool(
    pool: &SqlitePool,
    id: &str,
    payload: Option<&ListNativeApiCallLogsPayload>,
) -> Result<NativeApiCallLogDetail, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("API 调用记录 ID 不能为空".to_string());
    }

    let mut query = QueryBuilder::<Sqlite>::new(DETAIL_SELECT);
    query.push(" WHERE l.id = ");
    query.push_bind(id.to_string());

    let payload = payload.cloned().unwrap_or_default();
    let environment_mode = optional_trimmed(payload.environment_mode.as_deref());
    let selected_ssh = optional_trimmed(payload.selected_ssh_config_id.as_deref());
    let project_id = optional_trimmed(payload.project_id.as_deref());
    let scoped_ids =
        resolve_scoped_project_ids_for_stats(pool, environment_mode, selected_ssh, project_id)
            .await?;
    if scoped_ids.is_empty() && project_id.is_some() {
        return Err("API 调用记录不存在".to_string());
    }
    query.push(" AND ");
    push_project_scope(&mut query, &payload, &scoped_ids);
    if let Some(execution_target) = scoped_execution_target(&payload) {
        query.push(" AND LOWER(COALESCE(l.execution_target, 'local')) = ");
        query.push_bind(execution_target);
    }

    query
        .build_query_as::<NativeApiCallLogDetail>()
        .fetch_optional(pool)
        .await
        .map_err(|error| format!("读取 API 调用记录失败: {error}"))?
        .ok_or_else(|| "API 调用记录不存在".to_string())
}

#[tauri::command]
pub async fn list_native_api_call_logs<R: Runtime>(
    app: AppHandle<R>,
    payload: Option<ListNativeApiCallLogsPayload>,
) -> Result<NativeApiCallLogPage, String> {
    let pool = sqlite_pool(&app).await?;
    let payload = payload.unwrap_or_default();
    list_native_api_call_logs_with_pool(&pool, &payload).await
}

#[tauri::command]
pub async fn get_native_api_call_log<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    payload: Option<ListNativeApiCallLogsPayload>,
) -> Result<NativeApiCallLogDetail, String> {
    let pool = sqlite_pool(&app).await?;
    get_native_api_call_log_with_pool(&pool, &id, payload.as_ref()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::{get_all_migrations, latest_migration_version};
    use crate::native::model::call_log::{
        CALL_KIND_CHAT, CALL_LOG_BODY_CHAR_LIMIT, CALL_STATUS_CANCELLED, CALL_STATUS_FAILED,
        CALL_STATUS_SUCCESS,
    };

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");
        for migration in get_all_migrations() {
            if migration.version > latest_migration_version() {
                continue;
            }
            sqlx::raw_sql(migration.sql)
                .execute(&pool)
                .await
                .unwrap_or_else(|error| panic!("run migration {}: {}", migration.version, error));
        }
        pool
    }

    fn sample_record() -> NativeApiCallLogInsert {
        NativeApiCallLogInsert {
            id: "log-1".to_string(),
            call_id: "call-1".to_string(),
            attempt: 1,
            channel_id: Some("ch-1".to_string()),
            channel_name: Some("OpenAI".to_string()),
            protocol: "openai".to_string(),
            response_encoding: Some("sse".to_string()),
            model: Some("gpt-4o".to_string()),
            thinking_enabled: false,
            thinking_level: None,
            request_format: "openai".to_string(),
            request_body: Some(r#"{"model":"gpt-4o"}"#.to_string()),
            request_truncated: false,
            response_body: Some(r#"{"choices":[{"delta":{"content":"ok"}}]}"#.to_string()),
            response_truncated: false,
            input_tokens: Some(10),
            output_tokens: Some(4),
            cached_tokens: Some(3),
            total_tokens: Some(14),
            first_token_ms: Some(12),
            duration_ms: Some(40),
            status: CALL_STATUS_SUCCESS.to_string(),
            http_status: Some(200),
            error_message: None,
            session_id: Some("sess-1".to_string()),
            employee_id: Some("emp-1".to_string()),
            task_id: Some("task-1".to_string()),
            project_id: Some("proj-1".to_string()),
            subagent_id: None,
            call_kind: Some(CALL_KIND_CHAT.to_string()),
            execution_target: Some("local".to_string()),
        }
    }

    #[tokio::test]
    async fn insert_stores_nullable_tokens_and_cached_usage() {
        let pool = setup_pool().await;
        insert_native_api_call_log(&pool, &sample_record())
            .await
            .expect("insert");
        let row = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>, Option<i64>, String)>(
            "SELECT input_tokens, output_tokens, cached_tokens, total_tokens, status FROM native_api_call_logs WHERE id = 'log-1'",
        )
        .fetch_one(&pool)
        .await
        .expect("read row");
        assert_eq!(row.0, Some(10));
        assert_eq!(row.1, Some(4));
        assert_eq!(row.2, Some(3));
        assert_eq!(row.3, Some(14));
        assert_eq!(row.4, CALL_STATUS_SUCCESS);
    }

    #[tokio::test]
    async fn insert_keeps_unknown_tokens_null() {
        let pool = setup_pool().await;
        let mut record = sample_record();
        record.id = "log-null".to_string();
        record.input_tokens = None;
        record.output_tokens = None;
        record.cached_tokens = None;
        record.total_tokens = None;
        record.first_token_ms = None;
        insert_native_api_call_log(&pool, &record)
            .await
            .expect("insert");
        let row = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(
            "SELECT input_tokens, output_tokens, cached_tokens, first_token_ms FROM native_api_call_logs WHERE id = 'log-null'",
        )
        .fetch_one(&pool)
        .await
        .expect("read row");
        assert_eq!(row, (None, None, None, None));
    }

    #[tokio::test]
    async fn insert_redacts_error_and_bodies() {
        let pool = setup_pool().await;
        let mut record = sample_record();
        record.id = "log-secret".to_string();
        record.status = CALL_STATUS_FAILED.to_string();
        record.http_status = Some(401);
        record.error_message = Some("Authorization: Bearer sk-secret-key denied".to_string());
        record.request_body = Some(
            r#"{"api_key":"sk-secret-key","Authorization":"Bearer sk-secret-key","model":"gpt-4o"}"#
                .to_string(),
        );
        record.request_truncated = false;
        record.response_body = Some("x".repeat(CALL_LOG_BODY_CHAR_LIMIT + 4));
        record.response_truncated = false;
        insert_native_api_call_log(&pool, &record)
            .await
            .expect("insert");
        let (error, request, request_truncated, response_truncated): (
            Option<String>,
            Option<String>,
            i64,
            i64,
        ) = sqlx::query_as(
            "SELECT error_message, request_body, request_truncated, response_truncated FROM native_api_call_logs WHERE id = 'log-secret'",
        )
        .fetch_one(&pool)
        .await
        .expect("read secret row");
        let error = error.expect("error");
        let request = request.expect("request");
        assert!(!error.contains("sk-secret-key"));
        assert!(!error.to_ascii_lowercase().contains("bearer sk"));
        assert!(!request.contains("sk-secret-key"));
        assert!(request.contains("[redacted]"));
        assert_eq!(request_truncated, 0);
        assert_eq!(response_truncated, 1);
    }

    #[tokio::test]
    async fn insert_allows_cancelled_without_session() {
        let pool = setup_pool().await;
        let mut record = sample_record();
        record.id = "log-cancel".to_string();
        record.session_id = None;
        record.task_id = None;
        record.status = CALL_STATUS_CANCELLED.to_string();
        record.error_message = Some("已取消".to_string());
        insert_native_api_call_log(&pool, &record)
            .await
            .expect("insert oneshot cancel");
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM native_api_call_logs WHERE id = 'log-cancel'")
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(count, 1);
    }

    async fn insert_ssh_config(pool: &SqlitePool, id: &str, name: &str) {
        sqlx::query(
            r#"
            INSERT INTO ssh_configs (
                id, name, host, port, username, auth_type, known_hosts_mode, created_at, updated_at
            ) VALUES ($1, $2, 'example.test', 22, 'user', 'key', 'accept-new', '2026-08-01 10:00:00', '2026-08-01 10:00:00')
            "#,
        )
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .expect("insert ssh config");
    }

    async fn insert_project(
        pool: &SqlitePool,
        id: &str,
        name: &str,
        project_type: &str,
        ssh_config_id: Option<&str>,
    ) {
        sqlx::query(
            r#"
            INSERT INTO projects (
                id, name, description, status, repo_path, project_type, ssh_config_id,
                created_at, updated_at
            ) VALUES ($1, $2, NULL, 'active', NULL, $3, $4, '2026-08-01 10:00:00', '2026-08-01 10:00:00')
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(project_type)
        .bind(ssh_config_id)
        .execute(pool)
        .await
        .expect("insert project");
    }

    async fn insert_employee(pool: &SqlitePool, id: &str, name: &str) {
        sqlx::query(
            r#"
            INSERT INTO employees (
                id, name, role, model, reasoning_effort, status, specialization, system_prompt,
                project_id, created_at, updated_at
            ) VALUES ($1, $2, 'developer', 'gpt-4o', 'high', 'offline', NULL, NULL, NULL, '2026-08-01 10:00:00', '2026-08-01 10:00:00')
            "#,
        )
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .expect("insert employee");
    }

    #[tokio::test]
    async fn list_filters_by_ssh_scope_and_keeps_unknown_stats_null() {
        let pool = setup_pool().await;
        insert_ssh_config(&pool, "ssh-a", "Host A").await;
        insert_ssh_config(&pool, "ssh-b", "Host B").await;
        insert_project(&pool, "proj-local", "Local", "local", None).await;
        insert_project(&pool, "proj-ssh-a", "SSH A", "ssh", Some("ssh-a")).await;
        insert_project(&pool, "proj-ssh-b", "SSH B", "ssh", Some("ssh-b")).await;
        insert_employee(&pool, "emp-1", "Alice").await;

        let mut local = sample_record();
        local.id = "log-local".to_string();
        local.project_id = Some("proj-local".to_string());
        local.execution_target = Some("local".to_string());
        insert_native_api_call_log(&pool, &local)
            .await
            .expect("insert local");

        let mut ssh_a = sample_record();
        ssh_a.id = "log-ssh-a".to_string();
        ssh_a.project_id = Some("proj-ssh-a".to_string());
        ssh_a.execution_target = Some("ssh".to_string());
        ssh_a.input_tokens = None;
        ssh_a.output_tokens = None;
        ssh_a.cached_tokens = None;
        ssh_a.total_tokens = None;
        ssh_a.first_token_ms = None;
        ssh_a.duration_ms = None;
        insert_native_api_call_log(&pool, &ssh_a)
            .await
            .expect("insert ssh a");

        let mut ssh_b = sample_record();
        ssh_b.id = "log-ssh-b".to_string();
        ssh_b.project_id = Some("proj-ssh-b".to_string());
        ssh_b.execution_target = Some("ssh".to_string());
        insert_native_api_call_log(&pool, &ssh_b)
            .await
            .expect("insert ssh b");

        let empty = list_native_api_call_logs_with_pool(
            &pool,
            &ListNativeApiCallLogsPayload {
                environment_mode: Some("ssh".to_string()),
                selected_ssh_config_id: None,
                ..Default::default()
            },
        )
        .await
        .expect("list empty ssh host");
        assert!(empty.items.is_empty());
        assert_eq!(empty.total, 0);

        let page = list_native_api_call_logs_with_pool(
            &pool,
            &ListNativeApiCallLogsPayload {
                environment_mode: Some("ssh".to_string()),
                selected_ssh_config_id: Some("ssh-a".to_string()),
                include_total: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("list ssh a");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "log-ssh-a");
        assert_eq!(page.total, 1);
        assert_eq!(page.stats.call_count, 1);
        assert_eq!(page.stats.input_tokens_sum, None);
        assert_eq!(page.stats.avg_first_token_ms, None);
        assert_eq!(page.items[0].project_name.as_deref(), Some("SSH A"));
        assert_eq!(page.items[0].employee_name.as_deref(), Some("Alice"));

        let missing = get_native_api_call_log_with_pool(&pool, "missing", None)
            .await
            .expect_err("missing id");
        assert!(missing.contains("不存在"));

        let scoped_miss = get_native_api_call_log_with_pool(
            &pool,
            "log-ssh-b",
            Some(&ListNativeApiCallLogsPayload {
                environment_mode: Some("ssh".to_string()),
                selected_ssh_config_id: Some("ssh-a".to_string()),
                ..Default::default()
            }),
        )
        .await
        .expect_err("cross host");
        assert!(scoped_miss.contains("不存在"));
    }

    #[tokio::test]
    async fn list_includes_unscoped_channel_one_shot_and_ssh_employee_one_shot() {
        let pool = setup_pool().await;
        insert_ssh_config(&pool, "ssh-a", "Host A").await;
        insert_ssh_config(&pool, "ssh-b", "Host B").await;
        insert_project(&pool, "proj-local", "Local", "local", None).await;
        insert_project(&pool, "proj-ssh-a", "SSH A", "ssh", Some("ssh-a")).await;
        insert_project(&pool, "proj-ssh-b", "SSH B", "ssh", Some("ssh-b")).await;
        insert_employee(&pool, "emp-ssh", "SSH Alice").await;

        let mut local_channel = sample_record();
        local_channel.id = "log-channel-local".to_string();
        local_channel.project_id = None;
        local_channel.employee_id = None;
        local_channel.execution_target = Some("local".to_string());
        local_channel.call_kind =
            Some(crate::native::model::call_log::CALL_KIND_ONE_SHOT.to_string());
        insert_native_api_call_log(&pool, &local_channel)
            .await
            .expect("insert local channel");

        let mut ssh_channel = sample_record();
        ssh_channel.id = "log-channel-ssh".to_string();
        ssh_channel.project_id = None;
        ssh_channel.employee_id = None;
        ssh_channel.execution_target = Some("ssh".to_string());
        ssh_channel.call_kind =
            Some(crate::native::model::call_log::CALL_KIND_ONE_SHOT.to_string());
        insert_native_api_call_log(&pool, &ssh_channel)
            .await
            .expect("insert ssh channel");

        let mut ssh_employee = sample_record();
        ssh_employee.id = "log-employee-ssh".to_string();
        ssh_employee.project_id = Some("proj-ssh-a".to_string());
        ssh_employee.employee_id = Some("emp-ssh".to_string());
        ssh_employee.execution_target = Some("ssh".to_string());
        ssh_employee.call_kind =
            Some(crate::native::model::call_log::CALL_KIND_ONE_SHOT.to_string());
        insert_native_api_call_log(&pool, &ssh_employee)
            .await
            .expect("insert ssh employee");

        let local_page = list_native_api_call_logs_with_pool(
            &pool,
            &ListNativeApiCallLogsPayload {
                environment_mode: Some("local".to_string()),
                include_total: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("list local");
        assert_eq!(
            local_page
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["log-channel-local"]
        );
        assert_eq!(local_page.stats.call_count, 1);

        let ssh_page = list_native_api_call_logs_with_pool(
            &pool,
            &ListNativeApiCallLogsPayload {
                environment_mode: Some("ssh".to_string()),
                selected_ssh_config_id: Some("ssh-a".to_string()),
                include_total: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("list ssh a");
        let ssh_ids: Vec<&str> = ssh_page.items.iter().map(|item| item.id.as_str()).collect();
        assert!(ssh_ids.contains(&"log-channel-ssh"));
        assert!(ssh_ids.contains(&"log-employee-ssh"));
        assert_eq!(ssh_page.stats.call_count, 2);

        let default_get = get_native_api_call_log_with_pool(&pool, "log-employee-ssh", None)
            .await
            .expect_err("unscoped get");
        assert!(default_get.contains("不存在"));

        let local_get = get_native_api_call_log_with_pool(
            &pool,
            "log-employee-ssh",
            Some(&ListNativeApiCallLogsPayload {
                environment_mode: Some("local".to_string()),
                ..Default::default()
            }),
        )
        .await
        .expect_err("ssh employee hidden from local");
        assert!(local_get.contains("不存在"));

        let ssh_get = get_native_api_call_log_with_pool(
            &pool,
            "log-employee-ssh",
            Some(&ListNativeApiCallLogsPayload {
                environment_mode: Some("ssh".to_string()),
                selected_ssh_config_id: Some("ssh-a".to_string()),
                ..Default::default()
            }),
        )
        .await
        .expect("ssh employee detail");
        assert_eq!(ssh_get.id, "log-employee-ssh");
        assert_eq!(ssh_get.execution_target.as_deref(), Some("ssh"));
    }
}
