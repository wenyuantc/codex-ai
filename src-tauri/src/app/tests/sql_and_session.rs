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
