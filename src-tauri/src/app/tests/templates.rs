use super::*;

use std::collections::HashMap;

use crate::app::templates::{
    apply_task_template_with_pool, create_task_template_from_task_with_pool,
    create_task_template_with_pool, delete_task_template_with_pool, list_task_templates_with_pool,
};
use crate::db::models::{ApplyTaskTemplatePayload, CreateTaskTemplate, TaskTemplateSubtaskSpec};

async fn insert_tag(pool: &SqlitePool, id: &str, project_id: &str, name: &str) {
    sqlx::query(
        "INSERT INTO tags (id, project_id, name, created_at) VALUES ($1, $2, $3, '2026-04-16 10:00:00')",
    )
    .bind(id)
    .bind(project_id)
    .bind(name)
    .execute(pool)
    .await
    .expect("insert tag");
}

fn sample_template(project_id: &str) -> CreateTaskTemplate {
    CreateTaskTemplate {
        name: "补 i18n".to_string(),
        description: Some("批量补文案".to_string()),
        project_id: Some(project_id.to_string()),
        title_template: "给 {{module}} 补 i18n".to_string(),
        description_template: Some("模块 {{module}}".to_string()),
        priority: Some("high".to_string()),
        use_worktree: Some(true),
        tags: vec!["i18n".to_string(), "frontend".to_string()],
        subtasks: vec![
            TaskTemplateSubtaskSpec {
                title: "扫文案".to_string(),
                sort_order: 1,
            },
            TaskTemplateSubtaskSpec {
                title: "补翻译".to_string(),
                sort_order: 2,
            },
        ],
    }
}

fn module_set(module: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    values.insert("module".to_string(), module.to_string());
    values
}

#[test]
fn list_task_templates_hides_soft_deleted() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;

        let created = create_task_template_with_pool(&pool, sample_template("proj-1"))
            .await
            .expect("create template");
        let listed = list_task_templates_with_pool(&pool, Some("proj-1"))
            .await
            .expect("list templates");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        delete_task_template_with_pool(&pool, &created.id)
            .await
            .expect("soft delete template");

        let listed = list_task_templates_with_pool(&pool, Some("proj-1"))
            .await
            .expect("list after delete");
        assert!(listed.is_empty());

        pool.close().await;
    });
}

#[test]
fn apply_task_template_roundtrip_reuses_tags_and_creates_subtasks() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;
        insert_tag(&pool, "tag-i18n", "proj-1", "i18n").await;

        let template = create_task_template_with_pool(&pool, sample_template("proj-1"))
            .await
            .expect("create template");

        let created = apply_task_template_with_pool(
            &pool,
            ApplyTaskTemplatePayload {
                template_id: template.id.clone(),
                project_id: "proj-1".to_string(),
                variable_sets: vec![module_set("auth"), module_set("billing")],
                assignee_id: None,
                reviewer_id: None,
            },
            false,
        )
        .await
        .expect("apply template");

        assert_eq!(created.len(), 2);
        assert_eq!(created[0].title, "给 auth 补 i18n");
        assert_eq!(created[0].description.as_deref(), Some("模块 auth"));
        assert_eq!(created[1].title, "给 billing 补 i18n");
        assert!(created.iter().all(|task| task.use_worktree));
        assert!(created.iter().all(|task| task.priority == "high"));
        assert!(created.iter().all(|task| task.status == "todo"));
        assert!(created.iter().all(|task| task.project_id == "proj-1"));

        let i18n_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tags WHERE project_id = 'proj-1' AND name = 'i18n'",
        )
        .fetch_one(&pool)
        .await
        .expect("count i18n tags");
        assert_eq!(i18n_count, 1);

        let frontend_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM tags WHERE project_id = 'proj-1' AND name = 'frontend'",
        )
        .fetch_all(&pool)
        .await
        .expect("load frontend tags");
        assert_eq!(frontend_ids.len(), 1);

        for task in &created {
            let tag_names: Vec<String> = sqlx::query_scalar(
                r#"
                SELECT tags.name
                FROM tags
                INNER JOIN task_tags ON task_tags.tag_id = tags.id
                WHERE task_tags.task_id = $1
                ORDER BY tags.name
                "#,
            )
            .bind(&task.id)
            .fetch_all(&pool)
            .await
            .expect("load task tags");
            assert_eq!(tag_names, vec!["frontend", "i18n"]);

            let reused_i18n: String = sqlx::query_scalar(
                r#"
                SELECT tags.id
                FROM tags
                INNER JOIN task_tags ON task_tags.tag_id = tags.id
                WHERE task_tags.task_id = $1 AND tags.name = 'i18n'
                "#,
            )
            .bind(&task.id)
            .fetch_one(&pool)
            .await
            .expect("load reused i18n tag");
            assert_eq!(reused_i18n, "tag-i18n");

            let subtasks: Vec<(String, String)> = sqlx::query_as(
                "SELECT title, status FROM subtasks WHERE task_id = $1 ORDER BY sort_order",
            )
            .bind(&task.id)
            .fetch_all(&pool)
            .await
            .expect("load subtasks");
            assert_eq!(
                subtasks,
                vec![
                    ("扫文案".to_string(), "todo".to_string()),
                    ("补翻译".to_string(), "todo".to_string())
                ]
            );
        }

        let applied_details: Option<String> = sqlx::query_scalar(
            "SELECT details FROM activity_logs WHERE action = 'task_template_applied' LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("load apply activity");
        assert_eq!(applied_details.as_deref(), Some("补 i18n（生成 2 个任务）"));

        let created_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM activity_logs WHERE action = 'task_created'")
                .fetch_one(&pool)
                .await
                .expect("count created logs");
        assert_eq!(created_count, 2);

        pool.close().await;
    });
}

#[test]
fn apply_task_template_missing_variable_writes_nothing() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;
        let template = create_task_template_with_pool(&pool, sample_template("proj-1"))
            .await
            .expect("create template");

        let error = apply_task_template_with_pool(
            &pool,
            ApplyTaskTemplatePayload {
                template_id: template.id,
                project_id: "proj-1".to_string(),
                variable_sets: vec![HashMap::new()],
                assignee_id: None,
                reviewer_id: None,
            },
            false,
        )
        .await
        .expect_err("missing var");
        assert_eq!(error, "模板变量「module」未填写");

        let task_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE project_id = 'proj-1'")
                .fetch_one(&pool)
                .await
                .expect("count tasks");
        assert_eq!(task_count, 0);

        pool.close().await;
    });
}

#[test]
fn apply_task_template_over_limit_writes_nothing() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;
        let template = create_task_template_with_pool(&pool, sample_template("proj-1"))
            .await
            .expect("create template");

        let error = apply_task_template_with_pool(
            &pool,
            ApplyTaskTemplatePayload {
                template_id: template.id,
                project_id: "proj-1".to_string(),
                variable_sets: vec![module_set("auth"); 101],
                assignee_id: None,
                reviewer_id: None,
            },
            false,
        )
        .await
        .expect_err("over limit");
        assert_eq!(error, "单次批量操作最多 100 个任务");

        let task_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE project_id = 'proj-1'")
                .fetch_one(&pool)
                .await
                .expect("count tasks");
        assert_eq!(task_count, 0);

        pool.close().await;
    });
}

#[test]
fn apply_task_template_partial_missing_variable_writes_nothing() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;
        let template = create_task_template_with_pool(&pool, sample_template("proj-1"))
            .await
            .expect("create template");

        let error = apply_task_template_with_pool(
            &pool,
            ApplyTaskTemplatePayload {
                template_id: template.id,
                project_id: "proj-1".to_string(),
                variable_sets: vec![module_set("auth"), HashMap::new()],
                assignee_id: None,
                reviewer_id: None,
            },
            false,
        )
        .await
        .expect_err("missing var in later set");
        assert_eq!(error, "模板变量「module」未填写");

        let task_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE project_id = 'proj-1'")
                .fetch_one(&pool)
                .await
                .expect("count tasks");
        assert_eq!(task_count, 0);

        pool.close().await;
    });
}

#[test]
fn create_task_template_from_task_copies_tags_and_subtasks() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;
        insert_tag(&pool, "tag-i18n", "proj-1", "i18n").await;

        let now = "2026-04-16 10:00:00";
        sqlx::query(
            "INSERT INTO tasks (id, title, description, status, priority, project_id, use_worktree, time_spent_seconds, created_at, updated_at) VALUES ('task-1', '给 auth 补 i18n', '模块说明', 'todo', 'high', 'proj-1', 1, 0, $1, $1)",
        )
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert task");
        sqlx::query("INSERT INTO task_tags (task_id, tag_id) VALUES ('task-1', 'tag-i18n')")
            .execute(&pool)
            .await
            .expect("link tag");
        sqlx::query(
            "INSERT INTO subtasks (id, task_id, title, status, sort_order, created_at, updated_at) VALUES ('sub-1', 'task-1', '扫文案', 'done', 1, $1, $1)",
        )
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert subtask");

        let template = create_task_template_from_task_with_pool(&pool, "task-1", Some("模块模板"))
            .await
            .expect("save as template");

        assert_eq!(template.name, "模块模板");
        assert_eq!(template.project_id.as_deref(), Some("proj-1"));
        assert_eq!(template.title_template, "给 auth 补 i18n");
        assert_eq!(template.description_template.as_deref(), Some("模块说明"));
        assert_eq!(template.priority, "high");
        assert!(template.use_worktree);
        assert_eq!(template.tags, vec!["i18n"]);
        assert_eq!(template.subtasks.len(), 1);
        assert_eq!(template.subtasks[0].title, "扫文案");
        assert_eq!(template.subtasks[0].sort_order, 1);

        pool.close().await;
    });
}

#[test]
fn apply_task_template_requires_reviewer_when_automation_default_on() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-1").await;
        let template = create_task_template_with_pool(&pool, sample_template("proj-1"))
            .await
            .expect("create template");

        let error = apply_task_template_with_pool(
            &pool,
            ApplyTaskTemplatePayload {
                template_id: template.id,
                project_id: "proj-1".to_string(),
                variable_sets: vec![module_set("auth")],
                assignee_id: None,
                reviewer_id: None,
            },
            true,
        )
        .await
        .expect_err("reviewer required");
        assert_eq!(error, "当前已开启“新建任务默认自动质控”，请先指定审查员。");

        let task_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE project_id = 'proj-1'")
                .fetch_one(&pool)
                .await
                .expect("count tasks");
        assert_eq!(task_count, 0);

        pool.close().await;
    });
}
