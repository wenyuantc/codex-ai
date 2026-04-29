use super::*;

#[test]
fn archived_task_rejects_enabling_automation() {
    let task = Task {
        id: "task-archived".to_string(),
        title: "归档任务".to_string(),
        description: None,
        status: TASK_STATUS_ARCHIVED.to_string(),
        priority: "medium".to_string(),
        project_id: "proj-1".to_string(),
        use_worktree: false,
        assignee_id: None,
        reviewer_id: None,
        coordinator_id: None,
        complexity: None,
        ai_suggestion: None,
        plan_content: None,
        automation_mode: None,
        last_codex_session_id: None,
        last_review_session_id: None,
        time_started_at: None,
        time_spent_seconds: 0,
        completed_at: None,
        created_at: "2026-04-21 00:00:00".to_string(),
        updated_at: "2026-04-21 00:00:00".to_string(),
    };

    let error =
        validate_task_automation_mode_change(&task, Some(TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1))
            .expect_err("archived task should reject automation enable");
    assert_eq!(error, "已归档任务不能开启自动质控");
}

#[test]
fn task_archival_guard_blocks_running_workflows() {
    let error =
        validate_task_archival_guard(true, true, Some(TASK_AUTOMATION_PHASE_WAITING_EXECUTION))
            .expect_err("active execution/review/automation should block archiving");

    assert!(error.contains("执行、审核、自动质控"));
    assert!(error.contains("不能归档"));
}

#[test]
fn task_archival_guard_only_treats_active_automation_phases_as_blocking() {
    for phase in [
        TASK_AUTOMATION_PHASE_LAUNCHING_REVIEW,
        TASK_AUTOMATION_PHASE_WAITING_REVIEW,
        TASK_AUTOMATION_PHASE_LAUNCHING_FIX,
        TASK_AUTOMATION_PHASE_WAITING_EXECUTION,
        TASK_AUTOMATION_PHASE_COMMITTING_CODE,
    ] {
        assert!(
            is_task_automation_active_for_archival(phase),
            "phase {phase} should block archiving"
        );
    }

    for phase in [
        "idle",
        "completed",
        "blocked",
        "manual_control",
        "review_launch_failed",
        "fix_launch_failed",
    ] {
        assert!(
            !is_task_automation_active_for_archival(phase),
            "phase {phase} should not block archiving"
        );
    }

    validate_task_archival_guard(false, false, Some("idle"))
        .expect("idle automation state should allow archiving");
    validate_task_archival_guard(false, false, None)
        .expect("missing automation state should allow archiving");
}

#[test]
fn archiving_task_clears_pending_automation_state_and_logs_disable_activity() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;

        let task = Task {
            id: "task-archive".to_string(),
            title: "归档自动质控任务".to_string(),
            description: None,
            status: "review".to_string(),
            priority: "medium".to_string(),
            project_id: "proj-1".to_string(),
            use_worktree: false,
            assignee_id: None,
            reviewer_id: None,
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: Some(TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1.to_string()),
            last_codex_session_id: None,
            last_review_session_id: None,
            time_started_at: None,
            time_spent_seconds: 0,
            completed_at: None,
            created_at: "2026-04-21 00:00:00".to_string(),
            updated_at: "2026-04-21 00:00:00".to_string(),
        };

        let mut tx = pool.begin().await.expect("begin task transaction");
        insert_task_record(&mut tx, &task)
            .await
            .expect("insert task record");
        tx.commit().await.expect("commit task transaction");

        sqlx::query(
            r#"
            INSERT INTO task_automation_state (
                task_id,
                phase,
                round_count,
                consumed_session_id,
                last_trigger_session_id,
                pending_action,
                pending_round_count,
                last_error,
                last_verdict_json,
                updated_at
            ) VALUES (
                $1, 'review_launch_failed', 2, NULL, NULL, 'start_review', 2,
                'launch failed', '{"summary":"needs retry"}', '2026-04-21 00:00:00'
            )
            "#,
        )
        .bind(&task.id)
        .execute(&pool)
        .await
        .expect("insert automation state");

        sqlx::query("UPDATE tasks SET status = $1, automation_mode = NULL WHERE id = $2")
            .bind(TASK_STATUS_ARCHIVED)
            .bind(&task.id)
            .execute(&pool)
            .await
            .expect("archive task record");

        disable_task_automation_for_archived_task(&pool, &task)
            .await
            .expect("disable task automation for archive");

        let saved_task = fetch_task_by_id(&pool, &task.id)
            .await
            .expect("fetch archived task");
        assert_eq!(saved_task.status, TASK_STATUS_ARCHIVED);
        assert_eq!(saved_task.automation_mode, None);

        let state = fetch_task_automation_state_record(&pool, &task.id)
            .await
            .expect("fetch task automation state")
            .expect("task automation state exists");
        assert_eq!(state.phase, "idle");
        assert_eq!(state.pending_action, None);
        assert_eq!(state.pending_round_count, None);
        assert_eq!(state.last_verdict_json, None);

        let latest_action = sqlx::query_scalar::<_, Option<String>>(
            "SELECT action FROM activity_logs WHERE task_id = $1 ORDER BY created_at DESC, id DESC LIMIT 1",
        )
        .bind(&task.id)
        .fetch_one(&pool)
        .await
        .expect("fetch activity action");
        assert_eq!(latest_action.as_deref(), Some("task_automation_disabled"));

        clear_task_automation_state_for_disabled_mode(&pool, &task.id)
            .await
            .expect("re-clear disabled automation state");

        pool.close().await;
    });
}

#[test]
fn insert_task_record_persists_reviewer_id() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;
        insert_employee(&pool, "reviewer-1", "Reviewer", "reviewer").await;
        insert_employee(&pool, "coordinator-1", "Coordinator", "coordinator").await;

        let task = Task {
            id: "task-1".to_string(),
            title: "测试任务".to_string(),
            description: Some("验证审核员持久化".to_string()),
            status: "todo".to_string(),
            priority: "medium".to_string(),
            project_id: "proj-1".to_string(),
            use_worktree: false,
            assignee_id: None,
            reviewer_id: Some("reviewer-1".to_string()),
            coordinator_id: Some("coordinator-1".to_string()),
            complexity: None,
            ai_suggestion: None,
            plan_content: Some("协调员计划内容".to_string()),
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            time_started_at: None,
            time_spent_seconds: 0,
            completed_at: None,
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        };

        let mut tx = pool.begin().await.expect("begin task transaction");
        insert_task_record(&mut tx, &task)
            .await
            .expect("insert task record");
        tx.commit().await.expect("commit task transaction");

        let saved_task = fetch_task_by_id(&pool, &task.id)
            .await
            .expect("fetch inserted task");
        assert_eq!(saved_task.reviewer_id.as_deref(), Some("reviewer-1"));
        assert_eq!(saved_task.coordinator_id.as_deref(), Some("coordinator-1"));
        assert_eq!(saved_task.plan_content.as_deref(), Some("协调员计划内容"));
        assert!(!saved_task.use_worktree);
        assert_eq!(saved_task.time_started_at, None);
        assert_eq!(saved_task.time_spent_seconds, 0);
        assert_eq!(saved_task.completed_at, None);

        pool.close().await;
    });
}

#[test]
fn start_task_timer_is_idempotent_and_logs_once() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;

        let task = Task {
            id: "task-timer".to_string(),
            title: "计时任务".to_string(),
            description: None,
            status: "in_progress".to_string(),
            priority: "medium".to_string(),
            project_id: "proj-1".to_string(),
            use_worktree: false,
            assignee_id: None,
            reviewer_id: None,
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            time_started_at: None,
            time_spent_seconds: 0,
            completed_at: None,
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        };

        let mut tx = pool.begin().await.expect("begin task transaction");
        insert_task_record(&mut tx, &task)
            .await
            .expect("insert task record");
        tx.commit().await.expect("commit task transaction");

        let first = start_task_timer_internal(&pool, &task.id)
            .await
            .expect("start timer");
        let second = start_task_timer_internal(&pool, &task.id)
            .await
            .expect("start timer again");

        assert!(first.time_started_at.is_some());
        assert_eq!(second.time_started_at, first.time_started_at);

        let start_log_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM activity_logs WHERE task_id = $1 AND action = 'task_timer_started'",
        )
        .bind(&task.id)
        .fetch_one(&pool)
        .await
        .expect("fetch timer activity count");
        assert_eq!(start_log_count, 1);

        pool.close().await;
    });
}

#[test]
fn stop_task_timer_accumulates_without_marking_completed() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;

        let task = Task {
            id: "task-timer-stop".to_string(),
            title: "停止计时任务".to_string(),
            description: None,
            status: "in_progress".to_string(),
            priority: "medium".to_string(),
            project_id: "proj-1".to_string(),
            use_worktree: false,
            assignee_id: None,
            reviewer_id: None,
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            time_started_at: Some("2026-04-16 10:00:00".to_string()),
            time_spent_seconds: 60,
            completed_at: None,
            created_at: "2026-04-16 09:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        };

        let mut tx = pool.begin().await.expect("begin task transaction");
        insert_task_record(&mut tx, &task)
            .await
            .expect("insert task record");
        tx.commit().await.expect("commit task transaction");

        let stopped = stop_task_timer_internal(&pool, &task.id, "自动质控阻塞")
            .await
            .expect("stop timer");

        assert_eq!(stopped.time_started_at, None);
        assert!(stopped.time_spent_seconds >= 60);
        assert_eq!(stopped.completed_at, None);

        let stop_log_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM activity_logs WHERE task_id = $1 AND action = 'task_timer_stopped'",
        )
        .bind(&task.id)
        .fetch_one(&pool)
        .await
        .expect("fetch timer stop activity count");
        assert_eq!(stop_log_count, 1);

        pool.close().await;
    });
}

#[test]
fn completion_timer_update_accumulates_elapsed_time_and_reopen_clears_completion() {
    let mut task = Task {
        id: "task-completion".to_string(),
        title: "完成计时任务".to_string(),
        description: None,
        status: "in_progress".to_string(),
        priority: "medium".to_string(),
        project_id: "proj-1".to_string(),
        use_worktree: false,
        assignee_id: None,
        reviewer_id: None,
        coordinator_id: None,
        complexity: None,
        ai_suggestion: None,
        plan_content: None,
        automation_mode: None,
        last_codex_session_id: None,
        last_review_session_id: None,
        time_started_at: Some("2026-04-16 10:00:00".to_string()),
        time_spent_seconds: 120,
        completed_at: None,
        created_at: "2026-04-16 09:00:00".to_string(),
        updated_at: "2026-04-16 10:00:00".to_string(),
    };
    let completed_at =
        chrono::NaiveDateTime::parse_from_str("2026-04-16 10:05:30", "%Y-%m-%d %H:%M:%S")
            .expect("parse completed_at");

    let (completed_at_label, total_seconds) =
        build_task_completion_timer_update(&task, completed_at);

    assert_eq!(completed_at_label, "2026-04-16 10:05:30");
    assert_eq!(total_seconds, 450);

    task.status = "completed".to_string();
    task.completed_at = Some(completed_at_label);
    task.time_started_at = None;
    task.time_spent_seconds = total_seconds;

    assert!(should_clear_task_completed_at(&task, "in_progress"));
    assert!(!should_clear_task_completed_at(&task, "completed"));
}

#[test]
fn completion_metric_uses_tracked_task_time_when_available() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_employee(&pool, "emp-1", "Timer User", "developer").await;

        let task = Task {
            id: "task-metric".to_string(),
            title: "计入指标".to_string(),
            description: None,
            status: "completed".to_string(),
            priority: "medium".to_string(),
            project_id: "proj-1".to_string(),
            use_worktree: false,
            assignee_id: Some("emp-1".to_string()),
            reviewer_id: None,
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            time_started_at: None,
            time_spent_seconds: 3661,
            completed_at: Some("2026-04-16 11:01:01".to_string()),
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 11:01:01".to_string(),
        };

        record_completion_metric(&pool, &task)
            .await
            .expect("record completion metric");

        let average_completion_time = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT average_completion_time FROM employee_metrics WHERE employee_id = $1 LIMIT 1",
        )
        .bind("emp-1")
        .fetch_one(&pool)
        .await
        .expect("fetch metric");

        assert_eq!(average_completion_time, Some(3661.0));

        pool.close().await;
    });
}
