//! 全局任务执行并发闸门与运行队列。
//!
//! 只对「任务执行会话」(有 task_id、session_kind=execution、非 resume)排队;
//! review/fix/pipeline 等内部会话与员工即席会话计入并发数但不排队,
//! 排队会打断 task_automation 状态机,它们属于已获准工作流的后续动作。

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Emitter, Manager};

use crate::app::{fetch_task_by_id, insert_activity_log, sqlite_pool, start_task_timer_internal};
use crate::codex::load_codex_settings;

pub const TASK_RUN_QUEUE_CHANGED_EVENT: &str = "task-run-queue-changed";

/// 并发闸门共享状态:已放行但尚未注册进引擎管理器的「在途预约」计数。
#[derive(Default)]
pub struct RunQueueGate {
    reserved: std::sync::Mutex<usize>,
}

/// RAII 预约:持有期间占用一个并发额度,drop 时释放。
pub struct RunSlotReservation {
    gate: Arc<RunQueueGate>,
}

impl Drop for RunSlotReservation {
    fn drop(&mut self) {
        let mut reserved = self.gate.reserved.lock().unwrap();
        *reserved = reserved.saturating_sub(1);
    }
}

/// 引擎 start 命令的返回值:直接启动或已入队。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StartSessionOutcome {
    Started,
    Queued { position: i64 },
}

pub enum GateOutcome {
    Proceed(RunSlotReservation),
    Queued { position: i64 },
}

/// 重放引擎启动所需的完整入参(task_id 单独存列,resume 恒为 None)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTaskRun {
    pub provider: String,
    pub employee_id: String,
    pub task_description: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub task_git_context_id: Option<String>,
    #[serde(default)]
    pub image_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRunQueueItem {
    pub id: i64,
    pub task_id: String,
    pub provider: String,
    pub employee_id: String,
    pub enqueued_at: String,
    pub position: i64,
}

/// 判断一次 start 调用是否受闸门管辖。
pub fn should_gate_task_run(
    task_id: Option<&str>,
    resume_session_id: Option<&str>,
    session_kind: Option<&str>,
) -> bool {
    if task_id.is_none() || resume_session_id.is_some() {
        return false;
    }
    matches!(session_kind, None | Some("execution"))
}

/// 0 或负数表示不限并发。
pub(crate) fn effective_session_limit(max_concurrent_sessions: i32) -> Option<usize> {
    if max_concurrent_sessions <= 0 {
        None
    } else {
        Some(max_concurrent_sessions as usize)
    }
}

pub(crate) fn has_capacity(running: usize, reserved: usize, limit: Option<usize>) -> bool {
    match limit {
        None => true,
        Some(limit) => running + reserved < limit,
    }
}

/// 统计四个引擎管理器中的存活会话进程总数。
async fn count_running_sessions(app: &AppHandle) -> usize {
    let codex = {
        let manager = app.state::<Arc<std::sync::Mutex<crate::codex::CodexManager>>>();
        let guard = manager.lock().unwrap();
        guard.get_processes().len()
    };
    let claude = {
        let manager = app.state::<Arc<tokio::sync::Mutex<crate::claude::ClaudeManager>>>();
        let guard = manager.lock().await;
        guard.get_processes().len()
    };
    let grok = {
        let manager = app.state::<Arc<tokio::sync::Mutex<crate::grok::GrokManager>>>();
        let guard = manager.lock().await;
        guard.get_processes().len()
    };
    let opencode = {
        let manager = app.state::<Arc<tokio::sync::Mutex<crate::opencode::OpenCodeManager>>>();
        let guard = manager.lock().await;
        guard.get_processes().len()
    };
    codex + claude + grok + opencode
}

fn gate_state(app: &AppHandle) -> Arc<RunQueueGate> {
    app.state::<Arc<RunQueueGate>>().inner().clone()
}

/// 有余量则预约一个 slot;满载则将任务写入运行队列并返回排队位次。
pub async fn gate_or_enqueue(
    app: &AppHandle,
    task_id: &str,
    run: QueuedTaskRun,
) -> Result<GateOutcome, String> {
    let settings = load_codex_settings(app)?;
    let limit = effective_session_limit(settings.max_concurrent_sessions);
    let gate = gate_state(app);

    if limit.is_none() {
        return Ok(GateOutcome::Proceed(RunSlotReservation { gate }));
    }

    let running = count_running_sessions(app).await;
    let slot_reserved = {
        let mut reserved = gate.reserved.lock().unwrap();
        if has_capacity(running, *reserved, limit) {
            *reserved += 1;
            true
        } else {
            false
        }
    };
    if slot_reserved {
        return Ok(GateOutcome::Proceed(RunSlotReservation { gate }));
    }

    let pool = sqlite_pool(app).await?;
    let position = enqueue_task_run(&pool, task_id, &run).await?;
    let project_id = fetch_task_by_id(&pool, task_id)
        .await
        .ok()
        .map(|task| task.project_id);
    let _ = insert_activity_log(
        &pool,
        "task_run_queued",
        &format!("并发已达上限，任务已加入运行队列（第 {position} 位）"),
        Some(&run.employee_id),
        Some(task_id),
        project_id.as_deref(),
    )
    .await;
    let _ = app.emit(TASK_RUN_QUEUE_CHANGED_EVENT, ());
    Ok(GateOutcome::Queued { position })
}

/// 会话退出/应用启动时触发:容量富余则按入队顺序放行队首任务。
pub fn spawn_drain(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = drain_queue(&app).await {
            eprintln!("[run-queue] 队列放行失败: {error}");
        }
    });
}

async fn drain_queue(app: &AppHandle) -> Result<(), String> {
    loop {
        let settings = load_codex_settings(app)?;
        let limit = effective_session_limit(settings.max_concurrent_sessions);
        let pool = sqlite_pool(app).await?;
        let gate = gate_state(app);

        let running = count_running_sessions(app).await;
        let reservation = {
            let mut reserved = gate.reserved.lock().unwrap();
            if !has_capacity(running, *reserved, limit) {
                None
            } else {
                *reserved += 1;
                Some(RunSlotReservation { gate: gate.clone() })
            }
        };
        let Some(reservation) = reservation else {
            return Ok(());
        };

        let Some((task_id, run)) = claim_next_task_run(&pool).await? else {
            drop(reservation);
            return Ok(());
        };

        let project_id = fetch_task_by_id(&pool, &task_id)
            .await
            .ok()
            .map(|task| task.project_id);

        match replay_task_run(app, &task_id, &run).await {
            Ok(()) => {
                let _ = insert_activity_log(
                    &pool,
                    "task_run_dequeued",
                    "并发闸门放行，排队任务已启动执行",
                    Some(&run.employee_id),
                    Some(&task_id),
                    project_id.as_deref(),
                )
                .await;
            }
            Err(error) => {
                let _ = insert_activity_log(
                    &pool,
                    "task_run_dequeue_failed",
                    &format!("排队任务启动失败：{error}"),
                    Some(&run.employee_id),
                    Some(&task_id),
                    project_id.as_deref(),
                )
                .await;
            }
        }
        drop(reservation);
        let _ = app.emit(TASK_RUN_QUEUE_CHANGED_EVENT, ());
    }
}

/// 按 provider 重放启动;走 *_with_manager 内部路径,天然绕过闸门防止再入队。
async fn replay_task_run(
    app: &AppHandle,
    task_id: &str,
    run: &QueuedTaskRun,
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    if let Ok(task) = fetch_task_by_id(&pool, task_id).await {
        if task.status != "in_progress" {
            let _ = crate::app::tasks::update_task_status(
                app.clone(),
                task_id.to_string(),
                "in_progress".to_string(),
            )
            .await;
        }
    }
    let _ = sqlx::query("UPDATE employees SET status = 'busy' WHERE id = $1")
        .bind(&run.employee_id)
        .execute(&pool)
        .await;

    let result = match run.provider.as_str() {
        "claude" => {
            let manager = app
                .state::<Arc<tokio::sync::Mutex<crate::claude::ClaudeManager>>>()
                .inner()
                .clone();
            crate::claude::start_claude_with_manager(
                app.clone(),
                manager,
                run.employee_id.clone(),
                run.task_description.clone(),
                run.model.clone(),
                run.reasoning_effort.clone(),
                run.system_prompt.clone(),
                run.working_dir.clone(),
                Some(task_id.to_string()),
                run.task_git_context_id.clone(),
                None,
                run.image_paths.clone(),
                Some("execution".to_string()),
            )
            .await
        }
        "grok" => {
            let manager = app
                .state::<Arc<tokio::sync::Mutex<crate::grok::GrokManager>>>()
                .inner()
                .clone();
            crate::grok::start_grok_with_manager(
                app.clone(),
                manager,
                run.employee_id.clone(),
                run.task_description.clone(),
                run.model.clone(),
                run.reasoning_effort.clone(),
                run.system_prompt.clone(),
                run.working_dir.clone(),
                Some(task_id.to_string()),
                run.task_git_context_id.clone(),
                None,
                run.image_paths.clone(),
                Some("execution".to_string()),
            )
            .await
        }
        "opencode" => {
            let manager = app
                .state::<Arc<tokio::sync::Mutex<crate::opencode::OpenCodeManager>>>()
                .inner()
                .clone();
            crate::opencode::start_opencode_with_manager(
                app.clone(),
                manager,
                run.employee_id.clone(),
                run.task_description.clone(),
                run.model.clone(),
                run.working_dir.clone(),
                Some(task_id.to_string()),
                run.task_git_context_id.clone(),
                None,
                run.image_paths.clone(),
            )
            .await
        }
        _ => {
            let manager = app
                .state::<Arc<std::sync::Mutex<crate::codex::CodexManager>>>()
                .inner()
                .clone();
            crate::codex::start_codex_with_manager(
                app.clone(),
                manager,
                run.employee_id.clone(),
                run.task_description.clone(),
                run.model.clone(),
                run.reasoning_effort.clone(),
                run.system_prompt.clone(),
                run.working_dir.clone(),
                Some(task_id.to_string()),
                run.task_git_context_id.clone(),
                None,
                run.image_paths.clone(),
                Some("execution".to_string()),
            )
            .await
        }
    };

    if result.is_ok() {
        let _ = start_task_timer_internal(&pool, task_id).await;
    }
    result
}

// ---------- 队列表读写(pool 级,便于单测) ----------

pub(crate) async fn enqueue_task_run(
    pool: &SqlitePool,
    task_id: &str,
    run: &QueuedTaskRun,
) -> Result<i64, String> {
    let payload = serde_json::to_string(run)
        .map_err(|error| format!("序列化运行队列 payload 失败: {error}"))?;
    sqlx::query(
        r#"
        INSERT INTO task_run_queue (task_id, provider, payload, status)
        VALUES ($1, $2, $3, 'queued')
        ON CONFLICT(task_id) DO NOTHING
        "#,
    )
    .bind(task_id)
    .bind(&run.provider)
    .bind(&payload)
    .execute(pool)
    .await
    .map_err(|error| format!("写入运行队列失败: {error}"))?;

    queue_position(pool, task_id).await
}

pub(crate) async fn queue_position(pool: &SqlitePool, task_id: &str) -> Result<i64, String> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM task_run_queue
        WHERE status = 'queued'
          AND id <= (SELECT id FROM task_run_queue WHERE task_id = $1)
        "#,
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(|error| format!("查询排队位次失败: {error}"))
}

/// 取出并删除队首(FIFO);返回 None 表示队列为空。
pub(crate) async fn claim_next_task_run(
    pool: &SqlitePool,
) -> Result<Option<(String, QueuedTaskRun)>, String> {
    loop {
        let Some(row) = sqlx::query(
            "SELECT id, task_id, payload FROM task_run_queue WHERE status = 'queued' ORDER BY id ASC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .map_err(|error| format!("读取运行队列失败: {error}"))?
        else {
            return Ok(None);
        };

        let id: i64 = row.get("id");
        let task_id: String = row.get("task_id");
        let payload: String = row.get("payload");

        sqlx::query("DELETE FROM task_run_queue WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|error| format!("移除运行队列条目失败: {error}"))?;

        match serde_json::from_str::<QueuedTaskRun>(&payload) {
            Ok(run) => return Ok(Some((task_id, run))),
            Err(error) => {
                eprintln!("[run-queue] 跳过损坏的队列 payload(task_id={task_id}): {error}");
                continue;
            }
        }
    }
}

pub(crate) async fn remove_queued_task_run(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<bool, String> {
    let result = sqlx::query("DELETE FROM task_run_queue WHERE task_id = $1")
        .bind(task_id)
        .execute(pool)
        .await
        .map_err(|error| format!("取消排队失败: {error}"))?;
    Ok(result.rows_affected() > 0)
}

pub(crate) async fn list_queue_items(pool: &SqlitePool) -> Result<Vec<TaskRunQueueItem>, String> {
    let rows = sqlx::query(
        r#"
        SELECT id, task_id, provider, payload, enqueued_at
        FROM task_run_queue
        WHERE status = 'queued'
        ORDER BY id ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("读取运行队列失败: {error}"))?;

    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let payload: String = row.get("payload");
            let employee_id = serde_json::from_str::<QueuedTaskRun>(&payload)
                .map(|run| run.employee_id)
                .unwrap_or_default();
            TaskRunQueueItem {
                id: row.get("id"),
                task_id: row.get("task_id"),
                provider: row.get("provider"),
                employee_id,
                enqueued_at: row.get("enqueued_at"),
                position: index as i64 + 1,
            }
        })
        .collect())
}

// ---------- Tauri 命令 ----------

#[tauri::command]
pub async fn list_task_run_queue(app: AppHandle) -> Result<Vec<TaskRunQueueItem>, String> {
    let pool = sqlite_pool(&app).await?;
    list_queue_items(&pool).await
}

#[tauri::command]
pub async fn cancel_queued_task_run(app: AppHandle, task_id: String) -> Result<bool, String> {
    let pool = sqlite_pool(&app).await?;
    let removed = remove_queued_task_run(&pool, &task_id).await?;
    if removed {
        let project_id = fetch_task_by_id(&pool, &task_id)
            .await
            .ok()
            .map(|task| task.project_id);
        let _ = insert_activity_log(
            &pool,
            "task_run_queue_cancelled",
            "任务已取消排队，不再等待并发闸门放行",
            None,
            Some(&task_id),
            project_id.as_deref(),
        )
        .await;
        let _ = app.emit(TASK_RUN_QUEUE_CHANGED_EVENT, ());
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;

    use super::*;

    async fn setup_queue_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory");
        for migration in crate::db::migrations::get_all_migrations() {
            sqlx::raw_sql(migration.sql)
                .execute(&pool)
                .await
                .unwrap_or_else(|error| panic!("run migration {}: {}", migration.version, error));
        }
        pool
    }

    fn sample_run(provider: &str, employee_id: &str) -> QueuedTaskRun {
        QueuedTaskRun {
            provider: provider.to_string(),
            employee_id: employee_id.to_string(),
            task_description: "执行任务".to_string(),
            model: None,
            reasoning_effort: None,
            system_prompt: None,
            working_dir: Some("/tmp/repo".to_string()),
            task_git_context_id: None,
            image_paths: None,
        }
    }

    #[test]
    fn capacity_math_honors_unlimited_and_reservations() {
        assert!(has_capacity(100, 50, None));
        assert!(has_capacity(2, 0, Some(3)));
        assert!(!has_capacity(2, 1, Some(3)));
        assert!(!has_capacity(3, 0, Some(3)));
        assert_eq!(effective_session_limit(0), None);
        assert_eq!(effective_session_limit(-1), None);
        assert_eq!(effective_session_limit(5), Some(5));
    }

    #[test]
    fn gate_scope_only_covers_fresh_task_executions() {
        assert!(should_gate_task_run(Some("task-1"), None, None));
        assert!(should_gate_task_run(
            Some("task-1"),
            None,
            Some("execution")
        ));
        assert!(!should_gate_task_run(None, None, None));
        assert!(!should_gate_task_run(
            Some("task-1"),
            Some("resume-1"),
            None
        ));
        assert!(!should_gate_task_run(Some("task-1"), None, Some("review")));
        assert!(!should_gate_task_run(
            Some("task-1"),
            None,
            Some("one_shot")
        ));
    }

    #[test]
    fn queue_roundtrip_is_fifo_and_dedupes_by_task() {
        tauri::async_runtime::block_on(async {
            let pool = setup_queue_pool().await;

            let first = enqueue_task_run(&pool, "task-1", &sample_run("codex", "emp-1"))
                .await
                .expect("enqueue task-1");
            let second = enqueue_task_run(&pool, "task-2", &sample_run("claude", "emp-2"))
                .await
                .expect("enqueue task-2");
            assert_eq!(first, 1);
            assert_eq!(second, 2);

            // 重复入队同一任务不产生新行,位次不变。
            let duplicated = enqueue_task_run(&pool, "task-1", &sample_run("codex", "emp-1"))
                .await
                .expect("re-enqueue task-1");
            assert_eq!(duplicated, 1);

            let items = list_queue_items(&pool).await.expect("list queue");
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].task_id, "task-1");
            assert_eq!(items[0].employee_id, "emp-1");
            assert_eq!(items[1].position, 2);

            let (task_id, run) = claim_next_task_run(&pool)
                .await
                .expect("claim")
                .expect("row exists");
            assert_eq!(task_id, "task-1");
            assert_eq!(run.provider, "codex");

            let remaining = list_queue_items(&pool).await.expect("list queue");
            assert_eq!(remaining.len(), 1);
            assert_eq!(remaining[0].task_id, "task-2");
            assert_eq!(remaining[0].position, 1);
        });
    }

    #[test]
    fn cancel_removes_row_and_reports_missing() {
        tauri::async_runtime::block_on(async {
            let pool = setup_queue_pool().await;

            enqueue_task_run(&pool, "task-1", &sample_run("grok", "emp-1"))
                .await
                .expect("enqueue");
            assert!(remove_queued_task_run(&pool, "task-1")
                .await
                .expect("cancel"));
            assert!(!remove_queued_task_run(&pool, "task-1")
                .await
                .expect("cancel again"));
            assert!(claim_next_task_run(&pool).await.expect("claim").is_none());
        });
    }

    #[test]
    fn claim_skips_corrupted_payload_rows() {
        tauri::async_runtime::block_on(async {
            let pool = setup_queue_pool().await;

            sqlx::query(
                "INSERT INTO task_run_queue (task_id, provider, payload, status) VALUES ('task-bad', 'codex', '{broken', 'queued')",
            )
            .execute(&pool)
            .await
            .expect("insert corrupted row");
            enqueue_task_run(&pool, "task-good", &sample_run("opencode", "emp-9"))
                .await
                .expect("enqueue good row");

            let (task_id, run) = claim_next_task_run(&pool)
                .await
                .expect("claim")
                .expect("good row exists");
            assert_eq!(task_id, "task-good");
            assert_eq!(run.employee_id, "emp-9");
        });
    }
}
