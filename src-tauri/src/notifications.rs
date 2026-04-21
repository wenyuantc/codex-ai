use tauri::{AppHandle, Emitter, Runtime};
use uuid::Uuid;

use crate::app::{insert_activity_log, now_sqlite, sqlite_pool};
use crate::db::models::{
    AppNotification, DesktopNotificationEvent, NotificationCenterChanged, TransientNotification,
};

pub const NOTIFICATION_TYPE_REVIEW_PENDING: &str = "review_pending";
pub const NOTIFICATION_TYPE_RUN_FAILED: &str = "run_failed";
pub const NOTIFICATION_TYPE_RUN_COMPLETED: &str = "run_completed";
pub const NOTIFICATION_TYPE_TASK_COMPLETED: &str = "task_completed";
pub const NOTIFICATION_TYPE_SDK_UNAVAILABLE: &str = "sdk_unavailable";
pub const NOTIFICATION_TYPE_DATABASE_ERROR: &str = "database_error";
pub const NOTIFICATION_TYPE_SSH_CONFIG_ERROR: &str = "ssh_config_error";

pub const NOTIFICATION_SEVERITY_INFO: &str = "info";
pub const NOTIFICATION_SEVERITY_SUCCESS: &str = "success";
pub const NOTIFICATION_SEVERITY_WARNING: &str = "warning";
pub const NOTIFICATION_SEVERITY_ERROR: &str = "error";
pub const NOTIFICATION_SEVERITY_CRITICAL: &str = "critical";

const NOTIFICATION_CENTER_CHANGED_EVENT: &str = "notification-center-changed";
const NOTIFICATION_CENTER_DELIVER_EVENT: &str = "notification-center-deliver";
const TRANSIENT_NOTIFICATION_ID_PREFIX: &str = "transient:";

pub fn settings_route(section: &str, ssh_config_id: Option<&str>) -> String {
    match ssh_config_id {
        Some(ssh_config_id) => format!("/settings?section={section}&sshConfigId={ssh_config_id}"),
        None => format!("/settings?section={section}"),
    }
}

pub fn task_route(task_id: &str) -> String {
    format!("/kanban?taskId={task_id}")
}

pub fn review_pending_dedupe_key(task_id: &str) -> String {
    format!("review_pending:{task_id}")
}

pub fn sdk_unavailable_dedupe_key(scope: &str) -> String {
    format!("sdk_unavailable:{scope}")
}

pub fn database_error_dedupe_key(scope: &str) -> String {
    format!("database_error:{scope}")
}

pub fn ssh_missing_selection_dedupe_key() -> &'static str {
    "ssh_config_error:missing_selection"
}

pub fn ssh_selected_config_dedupe_key(ssh_config_id: &str) -> String {
    format!("ssh_config_error:selected:{ssh_config_id}")
}

pub fn ssh_password_probe_dedupe_key(ssh_config_id: &str) -> String {
    format!("ssh_config_error:password_probe:{ssh_config_id}")
}

pub fn ssh_health_check_dedupe_key(ssh_config_id: &str) -> String {
    format!("ssh_config_error:health:{ssh_config_id}")
}

pub fn transient_notification_id(dedupe_key: &str) -> String {
    format!("{TRANSIENT_NOTIFICATION_ID_PREFIX}{dedupe_key}")
}

fn should_emit_desktop_notification(reason: &str) -> bool {
    matches!(reason, "created" | "reactivated" | "updated" | "transient")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotificationDeliveryMode {
    OneTime,
    Sticky,
}

impl NotificationDeliveryMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::OneTime => "one_time",
            Self::Sticky => "sticky",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotificationDraft {
    pub notification_type: String,
    pub severity: String,
    pub source_module: String,
    pub title: String,
    pub message: String,
    pub recommendation: Option<String>,
    pub action_label: Option<String>,
    pub action_route: Option<String>,
    pub related_object_type: Option<String>,
    pub related_object_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub ssh_config_id: Option<String>,
    pub dedupe_key: Option<String>,
    delivery_mode: NotificationDeliveryMode,
}

impl NotificationDraft {
    pub fn one_time(
        notification_type: impl Into<String>,
        severity: impl Into<String>,
        source_module: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            notification_type: notification_type.into(),
            severity: severity.into(),
            source_module: source_module.into(),
            title: title.into(),
            message: message.into(),
            recommendation: None,
            action_label: None,
            action_route: None,
            related_object_type: None,
            related_object_id: None,
            project_id: None,
            task_id: None,
            ssh_config_id: None,
            dedupe_key: None,
            delivery_mode: NotificationDeliveryMode::OneTime,
        }
    }

    pub fn sticky(
        notification_type: impl Into<String>,
        severity: impl Into<String>,
        source_module: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            notification_type: notification_type.into(),
            severity: severity.into(),
            source_module: source_module.into(),
            title: title.into(),
            message: message.into(),
            recommendation: None,
            action_label: None,
            action_route: None,
            related_object_type: None,
            related_object_id: None,
            project_id: None,
            task_id: None,
            ssh_config_id: None,
            dedupe_key: None,
            delivery_mode: NotificationDeliveryMode::Sticky,
        }
    }

    pub fn with_recommendation(mut self, value: impl Into<String>) -> Self {
        self.recommendation = Some(value.into());
        self
    }

    pub fn with_action(mut self, label: impl Into<String>, route: impl Into<String>) -> Self {
        self.action_label = Some(label.into());
        self.action_route = Some(route.into());
        self
    }

    pub fn with_related_object(
        mut self,
        object_type: impl Into<String>,
        object_id: impl Into<String>,
    ) -> Self {
        self.related_object_type = Some(object_type.into());
        self.related_object_id = Some(object_id.into());
        self
    }

    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    pub fn with_dedupe_key(mut self, dedupe_key: impl Into<String>) -> Self {
        self.dedupe_key = Some(dedupe_key.into());
        self
    }

    pub fn with_ssh_config_id(mut self, ssh_config_id: impl Into<String>) -> Self {
        self.ssh_config_id = Some(ssh_config_id.into());
        self
    }
}

fn new_notification_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn build_notification_activity_detail(notification: &AppNotification) -> String {
    format!(
        "{}｜{}｜{}",
        notification.source_module, notification.title, notification.severity
    )
}

async fn emit_notification_changed<R: Runtime>(
    app: &AppHandle<R>,
    reason: &str,
    notification_id: Option<&str>,
) {
    let _ = app.emit(
        NOTIFICATION_CENTER_CHANGED_EVENT,
        NotificationCenterChanged {
            reason: reason.to_string(),
            notification_id: notification_id.map(ToOwned::to_owned),
        },
    );
}

fn build_desktop_notification_event(
    notification: &AppNotification,
    reason: &str,
) -> DesktopNotificationEvent {
    DesktopNotificationEvent {
        reason: reason.to_string(),
        notification_id: notification.id.clone(),
        title: notification.title.clone(),
        message: notification.message.clone(),
        severity: notification.severity.clone(),
        action_route: notification.action_route.clone(),
        project_id: notification.project_id.clone(),
        task_id: notification.task_id.clone(),
        ssh_config_id: notification.ssh_config_id.clone(),
        is_transient: false,
        last_triggered_at: notification.last_triggered_at.clone(),
    }
}

fn build_transient_desktop_notification_event(
    notification: &TransientNotification,
) -> DesktopNotificationEvent {
    DesktopNotificationEvent {
        reason: "transient".to_string(),
        notification_id: notification.id.clone(),
        title: notification.title.clone(),
        message: notification.message.clone(),
        severity: notification.severity.clone(),
        action_route: notification.action_route.clone(),
        project_id: notification.project_id.clone(),
        task_id: notification.task_id.clone(),
        ssh_config_id: notification.ssh_config_id.clone(),
        is_transient: true,
        last_triggered_at: notification.last_triggered_at.clone(),
    }
}

async fn emit_desktop_notification_event<R: Runtime>(
    app: &AppHandle<R>,
    reason: &str,
    notification: &AppNotification,
) {
    if !should_emit_desktop_notification(reason) {
        return;
    }

    let _ = app.emit(
        NOTIFICATION_CENTER_DELIVER_EVENT,
        build_desktop_notification_event(notification, reason),
    );
}

pub fn emit_transient_notification<R: Runtime>(
    app: &AppHandle<R>,
    notification: TransientNotification,
) {
    let desktop_event = build_transient_desktop_notification_event(&notification);
    let _ = app.emit("notification-center-transient", notification);
    let _ = app.emit(NOTIFICATION_CENTER_DELIVER_EVENT, desktop_event);
}

async fn fetch_notification_by_id(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> Result<AppNotification, String> {
    sqlx::query_as::<_, AppNotification>("SELECT * FROM notifications WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Failed to fetch notification {}: {}", id, error))
}

async fn fetch_notification_by_dedupe_key(
    pool: &sqlx::SqlitePool,
    dedupe_key: &str,
) -> Result<Option<AppNotification>, String> {
    sqlx::query_as::<_, AppNotification>(
        r#"
        SELECT *
        FROM notifications
        WHERE dedupe_key = $1
        ORDER BY CASE WHEN state = 'active' THEN 0 ELSE 1 END, updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(dedupe_key)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        format!(
            "Failed to fetch notification by dedupe key {}: {}",
            dedupe_key, error
        )
    })
}

fn sticky_notification_changed(existing: &AppNotification, draft: &NotificationDraft) -> bool {
    existing.notification_type != draft.notification_type
        || existing.severity != draft.severity
        || existing.source_module != draft.source_module
        || existing.title != draft.title
        || existing.message != draft.message
        || existing.recommendation != draft.recommendation
        || existing.action_label != draft.action_label
        || existing.action_route != draft.action_route
        || existing.related_object_type != draft.related_object_type
        || existing.related_object_id != draft.related_object_id
        || existing.project_id != draft.project_id
        || existing.task_id != draft.task_id
        || existing.ssh_config_id != draft.ssh_config_id
}

fn sticky_refresh_reason(existing: &AppNotification, draft: &NotificationDraft) -> &'static str {
    if sticky_notification_changed(existing, draft) {
        "updated"
    } else {
        "retriggered"
    }
}

async fn insert_notification_row<R: Runtime>(
    app: &AppHandle<R>,
    draft: &NotificationDraft,
    last_triggered_at: &str,
) -> Result<AppNotification, String> {
    let pool = sqlite_pool(app).await?;
    let id = new_notification_id();

    sqlx::query(
        r#"
        INSERT INTO notifications (
            id,
            notification_type,
            severity,
            source_module,
            title,
            message,
            recommendation,
            action_label,
            action_route,
            related_object_type,
            related_object_id,
            project_id,
            task_id,
            ssh_config_id,
            delivery_mode,
            state,
            is_read,
            dedupe_key,
            occurrence_count,
            first_triggered_at,
            last_triggered_at,
            read_at,
            resolved_at,
            created_at,
            updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, 'active',
            0, $16, 1, $17, $17, NULL, NULL, $17, $17
        )
        "#,
    )
    .bind(&id)
    .bind(&draft.notification_type)
    .bind(&draft.severity)
    .bind(&draft.source_module)
    .bind(&draft.title)
    .bind(&draft.message)
    .bind(&draft.recommendation)
    .bind(&draft.action_label)
    .bind(&draft.action_route)
    .bind(&draft.related_object_type)
    .bind(&draft.related_object_id)
    .bind(&draft.project_id)
    .bind(&draft.task_id)
    .bind(&draft.ssh_config_id)
    .bind(draft.delivery_mode.as_str())
    .bind(&draft.dedupe_key)
    .bind(last_triggered_at)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to insert notification: {}", error))?;

    let notification = fetch_notification_by_id(&pool, &id).await?;
    let _ = insert_activity_log(
        &pool,
        "notification_created",
        &build_notification_activity_detail(&notification),
        None,
        notification.task_id.as_deref(),
        notification.project_id.as_deref(),
    )
    .await;
    emit_desktop_notification_event(app, "created", &notification).await;
    emit_notification_changed(app, "created", Some(notification.id.as_str())).await;
    Ok(notification)
}

pub async fn publish_one_time_notification<R: Runtime>(
    app: &AppHandle<R>,
    draft: NotificationDraft,
) -> Result<AppNotification, String> {
    insert_notification_row(app, &draft, &now_sqlite()).await
}

pub async fn ensure_sticky_notification<R: Runtime>(
    app: &AppHandle<R>,
    draft: NotificationDraft,
) -> Result<AppNotification, String> {
    if draft.delivery_mode != NotificationDeliveryMode::Sticky {
        return Err("Sticky notification requires sticky delivery mode".to_string());
    }

    let dedupe_key = draft
        .dedupe_key
        .as_deref()
        .ok_or_else(|| "Sticky notification requires dedupe key".to_string())?;
    let pool = sqlite_pool(app).await?;
    let existing = fetch_notification_by_dedupe_key(&pool, dedupe_key).await?;
    let now = now_sqlite();

    match existing {
        None => insert_notification_row(app, &draft, &now).await,
        Some(current) if current.state == "active" => {
            let change_reason = sticky_refresh_reason(&current, &draft);

            sqlx::query(
                r#"
                UPDATE notifications
                SET notification_type = $2,
                    severity = $3,
                    source_module = $4,
                    title = $5,
                    message = $6,
                    recommendation = $7,
                    action_label = $8,
                    action_route = $9,
                    related_object_type = $10,
                    related_object_id = $11,
                    project_id = $12,
                    task_id = $13,
                    ssh_config_id = $14,
                    delivery_mode = $15,
                    state = 'active',
                    is_read = 0,
                    read_at = NULL,
                    resolved_at = NULL,
                    occurrence_count = occurrence_count + 1,
                    last_triggered_at = $16,
                    updated_at = $16
                WHERE id = $1
                "#,
            )
            .bind(&current.id)
            .bind(&draft.notification_type)
            .bind(&draft.severity)
            .bind(&draft.source_module)
            .bind(&draft.title)
            .bind(&draft.message)
            .bind(&draft.recommendation)
            .bind(&draft.action_label)
            .bind(&draft.action_route)
            .bind(&draft.related_object_type)
            .bind(&draft.related_object_id)
            .bind(&draft.project_id)
            .bind(&draft.task_id)
            .bind(&draft.ssh_config_id)
            .bind(draft.delivery_mode.as_str())
            .bind(&now)
            .execute(&pool)
            .await
            .map_err(|error| format!("Failed to refresh sticky notification: {}", error))?;

            let notification = fetch_notification_by_id(&pool, &current.id).await?;
            emit_desktop_notification_event(app, change_reason, &notification).await;
            emit_notification_changed(app, change_reason, Some(notification.id.as_str())).await;
            Ok(notification)
        }
        Some(current) => {
            sqlx::query(
                r#"
                UPDATE notifications
                SET notification_type = $2,
                    severity = $3,
                    source_module = $4,
                    title = $5,
                    message = $6,
                    recommendation = $7,
                    action_label = $8,
                    action_route = $9,
                    related_object_type = $10,
                    related_object_id = $11,
                    project_id = $12,
                    task_id = $13,
                    ssh_config_id = $14,
                    delivery_mode = $15,
                    state = 'active',
                    is_read = 0,
                    read_at = NULL,
                    resolved_at = NULL,
                    occurrence_count = occurrence_count + 1,
                    last_triggered_at = $16,
                    updated_at = $16
                WHERE id = $1
                "#,
            )
            .bind(&current.id)
            .bind(&draft.notification_type)
            .bind(&draft.severity)
            .bind(&draft.source_module)
            .bind(&draft.title)
            .bind(&draft.message)
            .bind(&draft.recommendation)
            .bind(&draft.action_label)
            .bind(&draft.action_route)
            .bind(&draft.related_object_type)
            .bind(&draft.related_object_id)
            .bind(&draft.project_id)
            .bind(&draft.task_id)
            .bind(&draft.ssh_config_id)
            .bind(draft.delivery_mode.as_str())
            .bind(&now)
            .execute(&pool)
            .await
            .map_err(|error| format!("Failed to reactivate sticky notification: {}", error))?;

            let notification = fetch_notification_by_id(&pool, &current.id).await?;
            let _ = insert_activity_log(
                &pool,
                "notification_created",
                &build_notification_activity_detail(&notification),
                None,
                notification.task_id.as_deref(),
                notification.project_id.as_deref(),
            )
            .await;
            emit_desktop_notification_event(app, "reactivated", &notification).await;
            emit_notification_changed(app, "reactivated", Some(notification.id.as_str())).await;
            Ok(notification)
        }
    }
}

pub async fn resolve_sticky_notification<R: Runtime>(
    app: &AppHandle<R>,
    dedupe_key: &str,
    recovery: Option<NotificationDraft>,
) -> Result<Option<AppNotification>, String> {
    let pool = sqlite_pool(app).await?;
    let existing = fetch_notification_by_dedupe_key(&pool, dedupe_key).await?;
    let Some(current) = existing else {
        return Ok(None);
    };

    if current.state != "active" {
        return Ok(None);
    }

    let now = now_sqlite();
    sqlx::query(
        r#"
        UPDATE notifications
        SET state = 'resolved',
            resolved_at = $2,
            updated_at = $2
        WHERE id = $1
        "#,
    )
    .bind(&current.id)
    .bind(&now)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to resolve sticky notification: {}", error))?;

    let notification = fetch_notification_by_id(&pool, &current.id).await?;
    let _ = insert_activity_log(
        &pool,
        "notification_resolved",
        &build_notification_activity_detail(&notification),
        None,
        notification.task_id.as_deref(),
        notification.project_id.as_deref(),
    )
    .await;
    emit_notification_changed(app, "resolved", Some(notification.id.as_str())).await;

    if let Some(mut recovery) = recovery {
        recovery.delivery_mode = NotificationDeliveryMode::OneTime;
        let _ = publish_one_time_notification(app, recovery).await?;
    }

    Ok(Some(notification))
}

#[tauri::command]
pub async fn list_notifications<R: Runtime>(
    app: AppHandle<R>,
    limit: Option<usize>,
) -> Result<Vec<AppNotification>, String> {
    let pool = sqlite_pool(&app).await?;
    let safe_limit = limit.unwrap_or(60).clamp(1, 200) as i64;
    sqlx::query_as::<_, AppNotification>(
        r#"
        SELECT *
        FROM notifications
        WHERE state = 'active'
        ORDER BY last_triggered_at DESC, created_at DESC
        LIMIT $1
        "#,
    )
    .bind(safe_limit)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to list notifications: {}", error))
}

#[tauri::command]
pub async fn mark_notification_read<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<AppNotification, String> {
    let pool = sqlite_pool(&app).await?;
    let now = now_sqlite();
    sqlx::query(
        r#"
        UPDATE notifications
        SET is_read = 1,
            read_at = COALESCE(read_at, $2),
            updated_at = $2
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .bind(&now)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to mark notification {} as read: {}", id, error))?;

    let notification = fetch_notification_by_id(&pool, &id).await?;
    emit_notification_changed(&app, "read", Some(notification.id.as_str())).await;
    Ok(notification)
}

#[tauri::command]
pub async fn mark_all_notifications_read<R: Runtime>(app: AppHandle<R>) -> Result<u64, String> {
    let pool = sqlite_pool(&app).await?;
    let now = now_sqlite();
    let result = sqlx::query(
        r#"
        UPDATE notifications
        SET is_read = 1,
            read_at = COALESCE(read_at, $1),
            updated_at = $1
        WHERE state = 'active' AND is_read = 0
        "#,
    )
    .bind(&now)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to mark all notifications as read: {}", error))?;

    emit_notification_changed(&app, "all_read", None).await;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_notification() -> AppNotification {
        AppNotification {
            id: "notification-1".to_string(),
            notification_type: NOTIFICATION_TYPE_SDK_UNAVAILABLE.to_string(),
            severity: NOTIFICATION_SEVERITY_ERROR.to_string(),
            source_module: "sdk_health".to_string(),
            title: "SDK 不可用".to_string(),
            message: "本地 SDK 当前不可用".to_string(),
            recommendation: Some("请检查 SDK 安装".to_string()),
            action_label: Some("打开设置".to_string()),
            action_route: Some(settings_route("sdk", None)),
            related_object_type: Some("environment".to_string()),
            related_object_id: Some("local".to_string()),
            project_id: None,
            task_id: None,
            ssh_config_id: None,
            delivery_mode: "sticky".to_string(),
            state: "active".to_string(),
            is_read: false,
            dedupe_key: Some(sdk_unavailable_dedupe_key("local")),
            occurrence_count: 1,
            first_triggered_at: "2026-04-20 10:00:00".to_string(),
            last_triggered_at: "2026-04-20 10:00:00".to_string(),
            read_at: None,
            resolved_at: None,
            created_at: "2026-04-20 10:00:00".to_string(),
            updated_at: "2026-04-20 10:00:00".to_string(),
        }
    }

    #[test]
    fn notification_builder_keeps_dedupe_and_target_metadata() {
        let draft = NotificationDraft::sticky(
            NOTIFICATION_TYPE_SSH_CONFIG_ERROR,
            NOTIFICATION_SEVERITY_WARNING,
            "ssh_config",
            "SSH 配置异常",
            "当前 SSH 配置认证失败",
        )
        .with_dedupe_key(ssh_password_probe_dedupe_key("cfg-1"))
        .with_action("打开 SSH 设置", settings_route("ssh", Some("cfg-1")))
        .with_related_object("ssh_config", "cfg-1")
        .with_ssh_config_id("cfg-1");

        assert_eq!(
            draft.dedupe_key.as_deref(),
            Some("ssh_config_error:password_probe:cfg-1")
        );
        assert_eq!(
            draft.action_route.as_deref(),
            Some("/settings?section=ssh&sshConfigId=cfg-1")
        );
        assert_eq!(draft.related_object_type.as_deref(), Some("ssh_config"));
        assert_eq!(draft.related_object_id.as_deref(), Some("cfg-1"));
        assert_eq!(draft.ssh_config_id.as_deref(), Some("cfg-1"));
    }

    #[test]
    fn sticky_change_detector_ignores_identical_payloads() {
        let current = sample_notification();
        let draft = NotificationDraft::sticky(
            NOTIFICATION_TYPE_SDK_UNAVAILABLE,
            NOTIFICATION_SEVERITY_ERROR,
            "sdk_health",
            "SDK 不可用",
            "本地 SDK 当前不可用",
        )
        .with_recommendation("请检查 SDK 安装")
        .with_action("打开设置", settings_route("sdk", None))
        .with_related_object("environment", "local")
        .with_dedupe_key(sdk_unavailable_dedupe_key("local"));

        assert!(!sticky_notification_changed(&current, &draft));
    }

    #[test]
    fn sticky_change_detector_detects_message_and_route_updates() {
        let current = sample_notification();
        let draft = NotificationDraft::sticky(
            NOTIFICATION_TYPE_SDK_UNAVAILABLE,
            NOTIFICATION_SEVERITY_WARNING,
            "sdk_health",
            "SDK 状态波动",
            "远程 SDK 当前不可用",
        )
        .with_recommendation("请检查远程 Node 与 SDK")
        .with_action("打开远程设置", settings_route("sdk", Some("cfg-1")))
        .with_related_object("ssh_config", "cfg-1")
        .with_ssh_config_id("cfg-1")
        .with_dedupe_key(sdk_unavailable_dedupe_key("ssh:cfg-1"));

        assert!(sticky_notification_changed(&current, &draft));
    }

    #[test]
    fn routes_and_activity_details_follow_expected_format() {
        let notification = sample_notification();

        assert_eq!(
            settings_route("database", None),
            "/settings?section=database"
        );
        assert_eq!(task_route("task-1"), "/kanban?taskId=task-1");
        assert_eq!(review_pending_dedupe_key("task-1"), "review_pending:task-1");
        assert_eq!(database_error_dedupe_key("local"), "database_error:local");
        assert_eq!(
            ssh_missing_selection_dedupe_key(),
            "ssh_config_error:missing_selection"
        );
        assert_eq!(
            ssh_selected_config_dedupe_key("cfg-1"),
            "ssh_config_error:selected:cfg-1"
        );
        assert_eq!(
            ssh_password_probe_dedupe_key("cfg-1"),
            "ssh_config_error:password_probe:cfg-1"
        );
        assert_eq!(
            ssh_health_check_dedupe_key("cfg-1"),
            "ssh_config_error:health:cfg-1"
        );
        assert_eq!(
            transient_notification_id("database_error:local"),
            "transient:database_error:local"
        );
        assert_eq!(
            build_notification_activity_detail(&notification),
            "sdk_health｜SDK 不可用｜error"
        );
        assert_eq!(NOTIFICATION_TYPE_RUN_COMPLETED, "run_completed");
    }

    #[test]
    fn sticky_refresh_reason_distinguishes_retrigger_and_payload_update() {
        let current = sample_notification();
        let same_draft = NotificationDraft::sticky(
            NOTIFICATION_TYPE_SDK_UNAVAILABLE,
            NOTIFICATION_SEVERITY_ERROR,
            "sdk_health",
            "SDK 不可用",
            "本地 SDK 当前不可用",
        )
        .with_recommendation("请检查 SDK 安装")
        .with_action("打开设置", settings_route("sdk", None))
        .with_related_object("environment", "local")
        .with_dedupe_key(sdk_unavailable_dedupe_key("local"));
        let changed_draft = NotificationDraft::sticky(
            NOTIFICATION_TYPE_SDK_UNAVAILABLE,
            NOTIFICATION_SEVERITY_WARNING,
            "sdk_health",
            "SDK 状态波动",
            "远程 SDK 当前不可用",
        )
        .with_recommendation("请检查远程 Node 与 SDK")
        .with_action("打开远程设置", settings_route("sdk", Some("cfg-1")))
        .with_related_object("ssh_config", "cfg-1")
        .with_ssh_config_id("cfg-1")
        .with_dedupe_key(sdk_unavailable_dedupe_key("ssh:cfg-1"));

        assert_eq!(sticky_refresh_reason(&current, &same_draft), "retriggered");
        assert_eq!(sticky_refresh_reason(&current, &changed_draft), "updated");
    }

    #[test]
    fn desktop_notification_reason_filter_matches_plan() {
        assert!(should_emit_desktop_notification("created"));
        assert!(should_emit_desktop_notification("reactivated"));
        assert!(should_emit_desktop_notification("updated"));
        assert!(should_emit_desktop_notification("transient"));
        assert!(!should_emit_desktop_notification("retriggered"));
        assert!(!should_emit_desktop_notification("resolved"));
        assert!(!should_emit_desktop_notification("read"));
        assert!(!should_emit_desktop_notification("all_read"));
    }

    #[test]
    fn desktop_notification_payload_contains_navigation_context() {
        let notification = sample_notification();
        let payload = build_desktop_notification_event(&notification, "created");

        assert_eq!(payload.reason, "created");
        assert_eq!(payload.notification_id, notification.id);
        assert_eq!(payload.title, "SDK 不可用");
        assert_eq!(payload.message, "本地 SDK 当前不可用");
        assert_eq!(payload.severity, NOTIFICATION_SEVERITY_ERROR);
        assert_eq!(
            payload.action_route.as_deref(),
            Some("/settings?section=sdk")
        );
        assert_eq!(payload.project_id, None);
        assert_eq!(payload.task_id, None);
        assert_eq!(payload.ssh_config_id, None);
        assert!(!payload.is_transient);
        assert_eq!(payload.last_triggered_at, "2026-04-20 10:00:00");
    }

    #[test]
    fn transient_desktop_notification_payload_marks_transient_state() {
        let payload = build_transient_desktop_notification_event(&TransientNotification {
            id: "transient:database_error:local".to_string(),
            notification_type: NOTIFICATION_TYPE_DATABASE_ERROR.to_string(),
            severity: NOTIFICATION_SEVERITY_CRITICAL.to_string(),
            source_module: "database".to_string(),
            title: "数据库异常".to_string(),
            message: "数据库当前不可用".to_string(),
            recommendation: Some("请检查数据库状态".to_string()),
            action_label: Some("打开设置".to_string()),
            action_route: Some(settings_route("database", None)),
            related_object_type: Some("environment".to_string()),
            related_object_id: Some("local".to_string()),
            project_id: None,
            task_id: None,
            ssh_config_id: None,
            delivery_mode: "sticky".to_string(),
            occurrence_count: 1,
            first_triggered_at: "2026-04-20 10:10:00".to_string(),
            last_triggered_at: "2026-04-20 10:10:00".to_string(),
            is_read: false,
            is_transient: true,
        });

        assert_eq!(payload.reason, "transient");
        assert_eq!(payload.notification_id, "transient:database_error:local");
        assert!(payload.is_transient);
        assert_eq!(
            payload.action_route.as_deref(),
            Some("/settings?section=database")
        );
    }
}
