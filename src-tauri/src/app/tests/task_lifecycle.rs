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
        complexity: None,
        ai_suggestion: None,
        automation_mode: None,
        last_codex_session_id: None,
        last_review_session_id: None,
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
            complexity: None,
            ai_suggestion: None,
            automation_mode: Some(TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1.to_string()),
            last_codex_session_id: None,
            last_review_session_id: None,
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
            complexity: None,
            ai_suggestion: None,
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
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
        assert!(!saved_task.use_worktree);

        pool.close().await;
    });
}
