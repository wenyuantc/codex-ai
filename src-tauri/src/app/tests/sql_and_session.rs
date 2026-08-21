use super::*;

#[test]
fn sanitizes_sql_control_statements() {
    let source = "\u{feff}BEGIN TRANSACTION;\nCREATE TABLE demo(id INTEGER);\nCOMMIT;\nPRAGMA foreign_keys=OFF;\nINSERT INTO demo VALUES (1);\n";
    let sanitized = sanitize_sql_backup_script(source);

    assert!(sanitized.contains("CREATE TABLE demo(id INTEGER);"));
    assert!(sanitized.contains("INSERT INTO demo VALUES (1);"));
    assert!(!sanitized.contains("BEGIN TRANSACTION"));
    assert!(!sanitized.contains("COMMIT"));
    assert!(!sanitized.contains("PRAGMA foreign_keys=OFF"));
}

#[test]
fn terminates_sql_statement_once() {
    assert_eq!(
        ensure_statement_terminated("CREATE TABLE demo(id INTEGER)"),
        "CREATE TABLE demo(id INTEGER);"
    );
    assert_eq!(
        ensure_statement_terminated("CREATE TABLE demo(id INTEGER);"),
        "CREATE TABLE demo(id INTEGER);"
    );
    assert_eq!(ensure_statement_terminated("   "), "");
}

#[test]
fn global_search_types_default_to_all_supported_kinds() {
    let actual = normalize_global_search_types(None);
    let expected = HashSet::from([
        "project".to_string(),
        "task".to_string(),
        "employee".to_string(),
        "session".to_string(),
    ]);

    assert_eq!(actual, expected);
}

#[test]
fn global_search_types_ignore_unknown_values_and_keep_valid_entries() {
    let actual = normalize_global_search_types(Some(vec![
        " project ".to_string(),
        "TASK".to_string(),
        "unknown".to_string(),
        "task".to_string(),
    ]));

    let expected = HashSet::from(["project".to_string(), "task".to_string()]);
    assert_eq!(actual, expected);
}

#[test]
fn global_search_item_sort_prefers_score_then_recency_then_title() {
    let mut items = vec![
        GlobalSearchItem {
            item_type: "task".to_string(),
            item_id: "task-2".to_string(),
            title: "Bravo".to_string(),
            subtitle: None,
            summary: None,
            navigation_path: "/kanban?taskId=task-2".to_string(),
            score: 120,
            updated_at: Some("2026-04-16 10:00:00".to_string()),
            project_id: Some("proj-1".to_string()),
            task_id: Some("task-2".to_string()),
            employee_id: None,
            session_id: None,
        },
        GlobalSearchItem {
            item_type: "task".to_string(),
            item_id: "task-1".to_string(),
            title: "Alpha".to_string(),
            subtitle: None,
            summary: None,
            navigation_path: "/kanban?taskId=task-1".to_string(),
            score: 120,
            updated_at: Some("2026-04-18 10:00:00".to_string()),
            project_id: Some("proj-1".to_string()),
            task_id: Some("task-1".to_string()),
            employee_id: None,
            session_id: None,
        },
        GlobalSearchItem {
            item_type: "project".to_string(),
            item_id: "proj-9".to_string(),
            title: "Zulu".to_string(),
            subtitle: None,
            summary: None,
            navigation_path: "/projects/proj-9".to_string(),
            score: 180,
            updated_at: Some("2026-04-10 10:00:00".to_string()),
            project_id: Some("proj-9".to_string()),
            task_id: None,
            employee_id: None,
            session_id: None,
        },
    ];

    items.sort_by(compare_global_search_items);

    let ordered_ids = items
        .into_iter()
        .map(|item| item.item_id)
        .collect::<Vec<_>>();
    assert_eq!(ordered_ids, vec!["proj-9", "task-1", "task-2"]);
}

#[test]
fn session_resume_state_requires_cli_session_id() {
    let (status, message, can_resume) = resolve_session_resume_state(
        None,
        Some("emp-1"),
        Some("Alice"),
        "exited",
        false,
        "关联任务当前已有运行中的对话，请先停止后再继续。",
    );

    assert_eq!(status, "missing_cli_session");
    assert!(!can_resume);
    assert!(message.unwrap_or_default().contains("CLI 对话 ID"));
}

#[test]
fn session_resume_state_blocks_when_employee_missing() {
    let (status, _, can_resume) = resolve_session_resume_state(
        Some("sess-1"),
        None,
        None,
        "exited",
        false,
        "关联任务当前已有运行中的对话，请先停止后再继续。",
    );

    assert_eq!(status, "missing_employee");
    assert!(!can_resume);
}

#[test]
fn session_resume_state_blocks_when_session_is_stopping() {
    let (status, message, can_resume) = resolve_session_resume_state(
        Some("sess-1"),
        Some("emp-1"),
        Some("Alice"),
        "stopping",
        false,
        "关联任务当前已有运行中的对话，请先停止后再继续。",
    );

    assert_eq!(status, "stopping");
    assert!(!can_resume);
    assert!(message.unwrap_or_default().contains("正在停止"));
}

#[test]
fn session_resume_state_blocks_when_task_conflicts() {
    let (status, message, can_resume) = resolve_session_resume_state(
        Some("sess-1"),
        Some("emp-1"),
        Some("Alice"),
        "exited",
        true,
        "关联任务当前已有运行中的对话，请先停止后再继续。",
    );

    assert_eq!(status, "running");
    assert!(!can_resume);
    assert!(message.unwrap_or_default().contains("关联任务"));
}

#[test]
fn session_resume_state_allows_resumable_exited_session() {
    let (status, message, can_resume) = resolve_session_resume_state(
        Some("sess-1"),
        Some("emp-1"),
        Some("Alice"),
        "exited",
        false,
        "关联任务当前已有运行中的对话，请先停止后再继续。",
    );

    assert_eq!(status, "ready");
    assert!(can_resume);
    assert!(message.is_none());
}

#[test]
fn running_conflict_message_distinguishes_task_and_employee_scope() {
    assert!(resolve_running_conflict_message(Some("task-1")).contains("关联任务"));
    assert!(resolve_running_conflict_message(None).contains("关联员工"));
}

#[test]
fn record_task_review_requested_activity_ignores_missing_activity_log_table() {
    tauri::async_runtime::block_on(async {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        record_task_review_requested_activity(
            &pool,
            "reviewer-1",
            "Reviewer",
            "task-1",
            "project-1",
        )
        .await;

        pool.close().await;
    });
}

#[test]
fn fetch_execution_change_history_item_returns_existing_changes() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_session(&pool, "sess-1", Some("cli-sess-1"), "execution").await;
        insert_file_change(
            &pool,
            "change-1",
            "sess-1",
            "src/pages/SessionsPage.tsx",
            "sdk_event",
        )
        .await;

        let item = fetch_execution_change_history_item_by_session_id(&pool, "sess-1")
            .await
            .expect("fetch execution change history item");

        assert_eq!(item.session.id, "sess-1");
        assert_eq!(item.capture_mode, "sdk_event");
        assert_eq!(item.changes.len(), 1);
        assert_eq!(item.changes[0].path, "src/pages/SessionsPage.tsx");

        pool.close().await;
    });
}

#[test]
fn fetch_execution_change_history_item_returns_empty_changes_when_missing() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_session(&pool, "sess-2", Some("cli-sess-2"), "execution").await;

        let item = fetch_execution_change_history_item_by_session_id(&pool, "sess-2")
            .await
            .expect("fetch empty execution change history item");

        assert_eq!(item.session.id, "sess-2");
        assert!(item.changes.is_empty());
        assert_eq!(item.capture_mode, "git_fallback");

        pool.close().await;
    });
}

#[test]
fn fetch_execution_change_history_item_falls_back_to_session_started_provider() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_session(&pool, "sess-3", Some("cli-sess-3"), "execution").await;
        insert_session_started_event(
            &pool,
            "sess-3",
            "通过 SDK 启动，使用模型 gpt-5.4 / 推理强度 high / 图片 0 张",
        )
        .await;

        let item = fetch_execution_change_history_item_by_session_id(&pool, "sess-3")
            .await
            .expect("fetch provider fallback execution change history item");

        assert!(item.changes.is_empty());
        assert_eq!(item.capture_mode, "sdk_event");

        pool.close().await;
    });
}

#[test]
fn list_tasks_applies_global_limit_and_project_scope() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-a").await;
        insert_project(&pool, "proj-b").await;

        for index in 0..5 {
            let task = Task {
                id: format!("task-{index}"),
                title: format!("Task {index}"),
                description: None,
                status: if index % 2 == 0 {
                    "todo".to_string()
                } else {
                    "archived".to_string()
                },
                priority: "medium".to_string(),
                project_id: if index < 3 {
                    "proj-a".to_string()
                } else {
                    "proj-b".to_string()
                },
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
                deleted_at: None,
                due_date: None,
                blocked_reason: None,
                milestone_id: None,
                acceptance_checklist: None,
                last_acceptance_status: None,
                mcp_server_ids: None,
                created_at: format!("2026-04-21 00:00:0{index}"),
                updated_at: format!("2026-04-21 00:00:0{index}"),
            };
            let mut tx = pool.begin().await.expect("begin");
            insert_task_record(&mut tx, &task)
                .await
                .expect("insert task");
            tx.commit().await.expect("commit");
        }

        let limited = list_tasks_with_pool(
            &pool,
            &crate::db::models::ListTasksPayload {
                limit: Some(2),
                ..Default::default()
            },
        )
        .await
        .expect("list limited tasks");
        assert_eq!(limited.len(), 2);

        let by_project = list_tasks_with_pool(
            &pool,
            &crate::db::models::ListTasksPayload {
                project_id: Some("proj-a".to_string()),
                limit: Some(1), // ignored when project_id set
                ..Default::default()
            },
        )
        .await
        .expect("list project tasks");
        assert_eq!(by_project.len(), 3);

        let archived = list_tasks_with_pool(
            &pool,
            &crate::db::models::ListTasksPayload {
                status: Some("archived".to_string()),
                project_ids: Some(vec!["proj-a".to_string(), "proj-b".to_string()]),
                ..Default::default()
            },
        )
        .await
        .expect("list archived tasks");
        assert_eq!(archived.len(), 2);
        assert!(archived.iter().all(|task| task.status == "archived"));

        pool.close().await;
    });
}

#[test]
fn apply_codex_session_usage_keeps_null_until_first_delta_then_accumulates() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_session(&pool, "sess-usage", Some("cli-1"), "execution").await;

        let before = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(
            "SELECT input_tokens, output_tokens, total_tokens, reasoning_tokens, cached_tokens FROM codex_sessions WHERE id = $1",
        )
        .bind("sess-usage")
        .fetch_one(&pool)
        .await
        .expect("fetch unused session");
        assert_eq!(before, (None, None, None, None, None));

        crate::app::apply_codex_session_usage(
            &pool,
            "sess-usage",
            &crate::engine::UsageDelta {
                input_tokens: Some(10),
                output_tokens: Some(5),
                total_tokens: Some(15),
                reasoning_tokens: None,
                cached_tokens: Some(4),
            },
        )
        .await
        .expect("apply first usage");

        let after_first = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(
            "SELECT input_tokens, output_tokens, total_tokens, reasoning_tokens, cached_tokens FROM codex_sessions WHERE id = $1",
        )
        .bind("sess-usage")
        .fetch_one(&pool)
        .await
        .expect("fetch after first usage");
        assert_eq!(after_first, (Some(10), Some(5), Some(15), None, Some(4)));

        crate::app::apply_codex_session_usage(
            &pool,
            "sess-usage",
            &crate::engine::UsageDelta {
                input_tokens: Some(2),
                output_tokens: Some(3),
                total_tokens: Some(5),
                reasoning_tokens: Some(4),
                cached_tokens: Some(1),
            },
        )
        .await
        .expect("apply second usage");

        let after_second = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(
            "SELECT input_tokens, output_tokens, total_tokens, reasoning_tokens, cached_tokens FROM codex_sessions WHERE id = $1",
        )
        .bind("sess-usage")
        .fetch_one(&pool)
        .await
        .expect("fetch after second usage");
        assert_eq!(
            after_second,
            (Some(12), Some(8), Some(20), Some(4), Some(5))
        );

        crate::app::apply_codex_session_usage(
            &pool,
            "sess-usage",
            &crate::engine::UsageDelta::default(),
        )
        .await
        .expect("empty delta is a no-op");

        let after_empty = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(
            "SELECT input_tokens, output_tokens, total_tokens, reasoning_tokens, cached_tokens FROM codex_sessions WHERE id = $1",
        )
        .bind("sess-usage")
        .fetch_one(&pool)
        .await
        .expect("fetch after empty delta");
        assert_eq!(after_empty, after_second);

        pool.close().await;
    });
}

#[test]
fn task_token_usage_sum_ignores_sessions_without_usage() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-usage").await;
        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, title, status, priority, project_id, use_worktree, created_at, updated_at
            ) VALUES (
                'task-usage', 'Usage', 'todo', 'medium', 'proj-usage', 0,
                '2026-08-12 10:00:00', '2026-08-12 10:00:00'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert task");

        sqlx::query(
            r#"
            INSERT INTO codex_sessions (
                id, task_id, project_id, session_kind, status, ai_provider, started_at, created_at
            ) VALUES
            ('sess-known', 'task-usage', 'proj-usage', 'execution', 'exited', 'codex', '2026-08-12 11:00:00', '2026-08-12 11:00:00'),
            ('sess-unknown', 'task-usage', 'proj-usage', 'execution', 'exited', 'claude', '2026-08-12 12:00:00', '2026-08-12 12:00:00')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert sessions");

        crate::app::apply_codex_session_usage(
            &pool,
            "sess-known",
            &crate::engine::UsageDelta {
                input_tokens: Some(100),
                output_tokens: Some(20),
                total_tokens: Some(120),
                reasoning_tokens: None,
                cached_tokens: Some(30),
            },
        )
        .await
        .expect("apply known usage");

        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64)>(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0), \
             COALESCE(SUM(total_tokens), 0), COALESCE(SUM(reasoning_tokens), 0), \
             COALESCE(SUM(cached_tokens), 0), \
             COALESCE(SUM(CASE WHEN input_tokens IS NOT NULL OR output_tokens IS NOT NULL OR total_tokens IS NOT NULL OR reasoning_tokens IS NOT NULL OR cached_tokens IS NOT NULL THEN 1 ELSE 0 END), 0), \
             COALESCE(SUM(CASE WHEN cached_tokens IS NOT NULL THEN 1 ELSE 0 END), 0), \
             COUNT(*) \
             FROM codex_sessions WHERE task_id = $1",
        )
        .bind("task-usage")
        .fetch_one(&pool)
        .await
        .expect("aggregate task usage");

        assert_eq!(row, (100, 20, 120, 0, 30, 1, 1, 2));

        pool.close().await;
    });
}

#[test]
fn mark_codex_session_origin_pipeline_updates_existing_row() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_session(&pool, "sess-origin", None, "execution").await;

        let default_origin = sqlx::query_scalar::<_, String>(
            "SELECT session_origin FROM codex_sessions WHERE id = $1",
        )
        .bind("sess-origin")
        .fetch_one(&pool)
        .await
        .expect("read default session_origin");
        assert_eq!(default_origin, "direct");

        crate::app::mark_codex_session_origin_pipeline(&pool, "sess-origin")
            .await
            .expect("mark session origin pipeline");

        let origin = sqlx::query_scalar::<_, String>(
            "SELECT session_origin FROM codex_sessions WHERE id = $1",
        )
        .bind("sess-origin")
        .fetch_one(&pool)
        .await
        .expect("read updated session_origin");
        assert_eq!(origin, "pipeline");

        pool.close().await;
    });
}
