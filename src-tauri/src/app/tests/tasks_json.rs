use super::*;

use crate::db::models::{ExportTasksJsonPayload, ImportTasksJsonPayload};

#[test]
fn import_tasks_json_create_new_roundtrip_with_deps() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-import").await;

        let json = r#"{
          "format": "codex-ai.tasks",
          "version": 1,
          "exported_at": "2026-08-05T12:00:00Z",
          "source": { "project_id": "src", "environment_mode": "local", "app": "codex-ai" },
          "tasks": [
            {
              "source_id": "src-a",
              "title": "任务 A",
              "status": "todo",
              "priority": "high",
              "description": "说明 A",
              "tags": ["bug", "p0"],
              "subtasks": [{ "title": "子 A1", "status": "todo", "sort_order": 0 }],
              "depends_on_source_ids": []
            },
            {
              "source_id": "src-b",
              "title": "任务 B",
              "status": "in_progress",
              "priority": "medium",
              "tags": ["bug"],
              "subtasks": [],
              "depends_on_source_ids": ["src-a"]
            }
          ]
        }"#;

        let result = import_tasks_json_with_pool(
            &pool,
            &ImportTasksJsonPayload {
                project_id: "proj-import".to_string(),
                json: json.to_string(),
                conflict_strategy: Some("create_new".to_string()),
            },
        )
        .await
        .expect("import create_new");

        assert_eq!(result.created, 2);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.failed, 0);
        assert_eq!(result.task_ids.len(), 2);

        let titles: Vec<(String,)> = sqlx::query_as(
            "SELECT title FROM tasks WHERE project_id = 'proj-import' AND deleted_at IS NULL ORDER BY title",
        )
        .fetch_all(&pool)
        .await
        .expect("list titles");
        assert_eq!(titles.len(), 2);

        let tag_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tags WHERE project_id = 'proj-import'",
        )
        .fetch_one(&pool)
        .await
        .expect("tag count");
        assert_eq!(tag_count, 2);

        let dep_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM task_dependencies d
            INNER JOIN tasks t ON t.id = d.task_id
            WHERE t.project_id = 'proj-import'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("dep count");
        assert_eq!(dep_count, 1);

        let sub_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM subtasks s
            INNER JOIN tasks t ON t.id = s.task_id
            WHERE t.project_id = 'proj-import'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("sub count");
        assert_eq!(sub_count, 1);

        // Export and ensure field safety
        let exported = export_tasks_json_with_pool(
            &pool,
            &ExportTasksJsonPayload {
                project_id: Some("proj-import".to_string()),
                environment_mode: Some("local".to_string()),
                selected_ssh_config_id: None,
                limit: None,
            },
        )
        .await
        .expect("export");
        assert_eq!(exported.task_count, 2);
        assert!(!exported.truncated);
        assert!(tasks_json_payload_is_field_safe(&exported.json));
        assert!(!exported.json.contains("assignee_id"));

        pool.close().await;
    });
}

#[test]
fn import_tasks_json_skip_existing() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-skip").await;

        let existing = Task {
            id: "src-existing".to_string(),
            title: "已有任务".to_string(),
            description: None,
            status: "todo".to_string(),
            priority: "medium".to_string(),
            project_id: "proj-skip".to_string(),
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
            created_at: "2026-08-05 00:00:00".to_string(),
            updated_at: "2026-08-05 00:00:00".to_string(),
        };
        let mut tx = pool.begin().await.expect("begin");
        insert_task_record(&mut tx, &existing)
            .await
            .expect("insert existing");
        tx.commit().await.expect("commit");

        let json = r#"{
          "format": "codex-ai.tasks",
          "version": 1,
          "exported_at": "2026-08-05T12:00:00Z",
          "source": { "app": "codex-ai" },
          "tasks": [
            {
              "source_id": "src-existing",
              "title": "应跳过",
              "status": "todo",
              "priority": "low",
              "tags": [],
              "subtasks": [],
              "depends_on_source_ids": []
            },
            {
              "source_id": "src-new",
              "title": "新建任务",
              "status": "todo",
              "priority": "medium",
              "tags": [],
              "subtasks": [],
              "depends_on_source_ids": []
            }
          ]
        }"#;

        let result = import_tasks_json_with_pool(
            &pool,
            &ImportTasksJsonPayload {
                project_id: "proj-skip".to_string(),
                json: json.to_string(),
                conflict_strategy: Some("skip_existing".to_string()),
            },
        )
        .await
        .expect("import skip");

        assert_eq!(result.created, 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.failed, 0);

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE project_id = 'proj-skip' AND deleted_at IS NULL",
        )
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(total, 2);

        let title: String = sqlx::query_scalar("SELECT title FROM tasks WHERE id = 'src-existing'")
            .fetch_one(&pool)
            .await
            .expect("title");
        assert_eq!(title, "已有任务");

        pool.close().await;
    });
}

#[test]
fn import_tasks_json_rejects_invalid_status_without_writes() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-invalid").await;

        let json = r#"{
          "format": "codex-ai.tasks",
          "version": 1,
          "exported_at": "2026-08-05T12:00:00Z",
          "source": { "app": "codex-ai" },
          "tasks": [
            {
              "source_id": "bad-1",
              "title": "坏状态",
              "status": "not-a-status",
              "priority": "medium",
              "tags": [],
              "subtasks": [],
              "depends_on_source_ids": []
            }
          ]
        }"#;

        let result = import_tasks_json_with_pool(
            &pool,
            &ImportTasksJsonPayload {
                project_id: "proj-invalid".to_string(),
                json: json.to_string(),
                conflict_strategy: None,
            },
        )
        .await
        .expect("prevalidate returns Ok with errors");

        assert_eq!(result.created, 0);
        assert_eq!(result.failed, 1);
        assert!(!result.errors.is_empty());

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE project_id = 'proj-invalid'",
        )
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(total, 0);

        pool.close().await;
    });
}
