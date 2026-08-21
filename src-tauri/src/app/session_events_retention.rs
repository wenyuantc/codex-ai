use super::*;
use crate::app::session_events_policy::{
    load_session_events_policy, normalize_retention_days, save_session_events_policy,
};
use crate::db::models::{PurgeSessionEventsResult, SessionEventsPolicy, SessionEventsStats};

/// Delete expired `codex_session_events` rows older than `retention_days`.
/// Does not delete `codex_sessions` rows.
pub(crate) async fn purge_expired_session_events(
    pool: &SqlitePool,
    retention_days: i32,
) -> Result<u64, String> {
    let days = normalize_retention_days(retention_days);
    let modifier = format!("-{days} days");

    let result = sqlx::query(
        r#"
        DELETE FROM codex_session_events
        WHERE created_at < datetime('now', $1)
        "#,
    )
    .bind(&modifier)
    .execute(pool)
    .await
    .map_err(|error| format!("清理过期会话事件失败: {error}"))?;

    Ok(result.rows_affected())
}

pub(crate) async fn vacuum_database(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query("VACUUM")
        .execute(pool)
        .await
        .map_err(|error| format!("数据库 VACUUM 失败: {error}"))?;
    Ok(())
}

pub(crate) async fn fetch_session_events_stats(
    pool: &SqlitePool,
    retention_days: i32,
) -> Result<SessionEventsStats, String> {
    let days = normalize_retention_days(retention_days);
    let modifier = format!("-{days} days");

    let total_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM codex_session_events")
        .fetch_one(pool)
        .await
        .map_err(|error| format!("统计会话事件总数失败: {error}"))?;

    let expired_events: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM codex_session_events
        WHERE created_at < datetime('now', $1)
        "#,
    )
    .bind(&modifier)
    .fetch_one(pool)
    .await
    .map_err(|error| format!("统计过期会话事件失败: {error}"))?;

    let oldest_created_at: Option<String> =
        sqlx::query_scalar("SELECT MIN(created_at) FROM codex_session_events")
            .fetch_one(pool)
            .await
            .map_err(|error| format!("查询最早会话事件时间失败: {error}"))?;

    let newest_created_at: Option<String> =
        sqlx::query_scalar("SELECT MAX(created_at) FROM codex_session_events")
            .fetch_one(pool)
            .await
            .map_err(|error| format!("查询最新会话事件时间失败: {error}"))?;

    Ok(SessionEventsStats {
        total_events,
        expired_events,
        oldest_created_at,
        newest_created_at,
    })
}

/// Startup best-effort purge: DELETE only, no VACUUM, never blocks UI fatally.
pub(crate) async fn run_startup_session_events_purge<R: Runtime>(app: &AppHandle<R>) {
    let policy = match load_session_events_policy(app) {
        Ok(policy) => policy,
        Err(error) => {
            eprintln!("[db] 启动时读取会话事件保留策略失败: {error}");
            return;
        }
    };

    let pool = match sqlite_pool(app).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("[db] 启动时获取数据库连接失败，跳过会话事件清理: {error}");
            return;
        }
    };

    match purge_expired_session_events(&pool, policy.retention_days).await {
        Ok(deleted) => {
            if deleted > 0 {
                println!(
                    "[db] 启动清理过期会话事件完成: deleted={deleted}, retention_days={}",
                    policy.retention_days
                );
            }
        }
        Err(error) => {
            eprintln!("[db] 启动清理过期会话事件失败: {error}");
        }
    }
}

#[tauri::command]
pub async fn get_session_events_policy<R: Runtime>(
    app: AppHandle<R>,
) -> Result<SessionEventsPolicy, String> {
    load_session_events_policy(&app)
}

#[tauri::command]
pub async fn update_session_events_policy<R: Runtime>(
    app: AppHandle<R>,
    retention_days: i32,
) -> Result<SessionEventsPolicy, String> {
    // Out-of-range values are normalized to the default (30) before persist.
    save_session_events_policy(&app, retention_days)
}

#[tauri::command]
pub async fn get_session_events_stats<R: Runtime>(
    app: AppHandle<R>,
) -> Result<SessionEventsStats, String> {
    let policy = load_session_events_policy(&app)?;
    let pool = sqlite_pool(&app).await?;
    fetch_session_events_stats(&pool, policy.retention_days).await
}

#[tauri::command]
pub async fn purge_session_events<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PurgeSessionEventsResult, String> {
    let policy = load_session_events_policy(&app)?;
    let pool = sqlite_pool(&app).await?;

    let deleted = purge_expired_session_events(&pool, policy.retention_days).await?;

    let details = format!(
        "deleted={}, retention_days={}",
        deleted, policy.retention_days
    );
    if let Err(error) =
        insert_activity_log(&pool, "session_events_purged", &details, None, None, None).await
    {
        eprintln!("[db] 写入会话事件清理活动日志失败: {error}");
    }

    let (vacuum_ok, vacuum_error) = match vacuum_database(&pool).await {
        Ok(()) => (true, None),
        Err(error) => (false, Some(error)),
    };

    Ok(PurgeSessionEventsResult {
        deleted,
        retention_days: policy.retention_days,
        vacuum_ok,
        vacuum_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::database::build_current_migrator;
    use crate::app::session_events_policy::{normalize_retention_days, DEFAULT_RETENTION_DAYS};

    async fn setup_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");
        let migrator = build_current_migrator();
        let mut connection = pool.acquire().await.expect("acquire sqlite connection");
        migrator
            .run_direct(&mut *connection)
            .await
            .expect("run migrations");
        drop(connection);
        pool
    }

    async fn insert_session(pool: &SqlitePool, session_id: &str) {
        sqlx::query(
            r#"
            INSERT INTO codex_sessions (
                id, session_kind, status, started_at, created_at
            ) VALUES ($1, 'task', 'exited', '2026-04-16 10:00:00', '2026-04-16 10:00:00')
            "#,
        )
        .bind(session_id)
        .execute(pool)
        .await
        .expect("insert session");
    }

    async fn insert_event(pool: &SqlitePool, id: &str, session_id: &str, created_at: &str) {
        sqlx::query(
            r#"
            INSERT INTO codex_session_events (
                id, session_id, event_type, message, created_at
            ) VALUES ($1, $2, 'stdout', 'hello', $3)
            "#,
        )
        .bind(id)
        .bind(session_id)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("insert event");
    }

    #[test]
    fn normalize_retention_days_edges() {
        assert_eq!(normalize_retention_days(0), DEFAULT_RETENTION_DAYS);
        assert_eq!(normalize_retention_days(4000), DEFAULT_RETENTION_DAYS);
        assert_eq!(normalize_retention_days(1), 1);
        assert_eq!(normalize_retention_days(3650), 3650);
    }

    #[test]
    fn purge_deletes_only_expired_events() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            insert_session(&pool, "sess-1").await;

            // 40 days ago → expired under 30-day retention
            insert_event(
                &pool,
                "evt-old",
                "sess-1",
                &chrono::Utc::now()
                    .checked_sub_signed(chrono::Duration::days(40))
                    .expect("subtract days")
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            )
            .await;

            // 1 day ago → keep
            insert_event(
                &pool,
                "evt-new",
                "sess-1",
                &chrono::Utc::now()
                    .checked_sub_signed(chrono::Duration::days(1))
                    .expect("subtract days")
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            )
            .await;

            let deleted = purge_expired_session_events(&pool, 30)
                .await
                .expect("purge");
            assert_eq!(deleted, 1);

            let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM codex_session_events")
                .fetch_one(&pool)
                .await
                .expect("count remaining");
            assert_eq!(remaining, 1);

            let remaining_id: String =
                sqlx::query_scalar("SELECT id FROM codex_session_events LIMIT 1")
                    .fetch_one(&pool)
                    .await
                    .expect("fetch remaining id");
            assert_eq!(remaining_id, "evt-new");

            // Sessions must not be deleted
            let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM codex_sessions")
                .fetch_one(&pool)
                .await
                .expect("count sessions");
            assert_eq!(sessions, 1);
        });
    }

    #[test]
    fn stats_count_expired_relative_to_retention() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            insert_session(&pool, "sess-stats").await;
            insert_event(
                &pool,
                "evt-old",
                "sess-stats",
                &chrono::Utc::now()
                    .checked_sub_signed(chrono::Duration::days(40))
                    .expect("subtract days")
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            )
            .await;
            insert_event(
                &pool,
                "evt-new",
                "sess-stats",
                &chrono::Utc::now()
                    .checked_sub_signed(chrono::Duration::days(1))
                    .expect("subtract days")
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            )
            .await;

            let stats = fetch_session_events_stats(&pool, 30).await.expect("stats");
            assert_eq!(stats.total_events, 2);
            assert_eq!(stats.expired_events, 1);
            assert!(stats.oldest_created_at.is_some());
            assert!(stats.newest_created_at.is_some());
        });
    }
}
