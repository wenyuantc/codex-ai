use super::*;

#[test]
fn delete_employee_records_clears_reviewer_comments_and_metrics() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;
        insert_employee(&pool, "emp-1", "Ada", "reviewer").await;
        insert_employee(&pool, "emp-keep", "Bob", "developer").await;

        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, title, status, priority, project_id, use_worktree,
                assignee_id, reviewer_id, created_at, updated_at
            ) VALUES (
                'task-1', 'Review me', 'review', 'medium', 'proj-1', 0,
                'emp-1', 'emp-1', '2026-08-24 10:00:00', '2026-08-24 10:00:00'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert task");

        sqlx::query(
            r#"
            INSERT INTO comments (id, task_id, employee_id, content, is_ai_generated)
            VALUES ('comment-1', 'task-1', 'emp-1', 'needs a test', 0)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert comment");

        sqlx::query(
            r#"
            INSERT INTO activity_logs (id, employee_id, action, details, created_at)
            VALUES ('log-1', 'emp-1', 'task_reviewed', 'ok', '2026-08-24 10:01:00')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert activity log");

        sqlx::query(
            r#"
            INSERT INTO employee_metrics (
                id, employee_id, tasks_completed, period_start, period_end, created_at
            ) VALUES (
                'metric-1', 'emp-1', 1, '2026-08-01', '2026-08-31', '2026-08-24 10:02:00'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert metrics");

        let mut connection = pool.acquire().await.expect("acquire sqlite connection");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("enable foreign keys on connection");
        let blocked = sqlx::query("DELETE FROM employees WHERE id = $1")
            .bind("emp-1")
            .execute(&mut *connection)
            .await;
        drop(connection);
        assert!(
            blocked.is_err(),
            "reviewer/comment FKs should block a raw employee delete"
        );

        crate::app::employees::delete_employee_records(&pool, "emp-1")
            .await
            .expect("delete employee records");

        let employee_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM employees WHERE id = 'emp-1'")
                .fetch_one(&pool)
                .await
                .expect("count deleted employee");
        let remaining: String = sqlx::query_scalar("SELECT id FROM employees")
            .fetch_one(&pool)
            .await
            .expect("remaining employee");
        let assignee: Option<String> =
            sqlx::query_scalar("SELECT assignee_id FROM tasks WHERE id = 'task-1'")
                .fetch_one(&pool)
                .await
                .expect("assignee");
        let reviewer: Option<String> =
            sqlx::query_scalar("SELECT reviewer_id FROM tasks WHERE id = 'task-1'")
                .fetch_one(&pool)
                .await
                .expect("reviewer");
        let comment_employee: Option<String> =
            sqlx::query_scalar("SELECT employee_id FROM comments WHERE id = 'comment-1'")
                .fetch_one(&pool)
                .await
                .expect("comment author");
        let log_employee: Option<String> =
            sqlx::query_scalar("SELECT employee_id FROM activity_logs WHERE id = 'log-1'")
                .fetch_one(&pool)
                .await
                .expect("activity log");
        let metrics: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM employee_metrics WHERE employee_id = 'emp-1'")
                .fetch_one(&pool)
                .await
                .expect("metrics count");

        assert_eq!(employee_count, 0);
        assert_eq!(remaining, "emp-keep");
        assert_eq!(assignee, None);
        assert_eq!(reviewer, None);
        assert_eq!(comment_employee, None);
        assert_eq!(log_employee, None);
        assert_eq!(metrics, 0);

        pool.close().await;
    });
}
