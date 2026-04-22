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
