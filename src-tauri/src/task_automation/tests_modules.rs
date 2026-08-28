// Test modules for task_automation

#[cfg(test)]
mod automation_working_dir_tests {
    use sqlx::SqlitePool;

    use super::resolve_automation_execution_context;
    use crate::app::{build_current_migrator, PROJECT_TYPE_LOCAL, PROJECT_TYPE_SSH};
    use crate::db::models::{Project, Task};

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

    fn build_project(
        project_type: &str,
        repo_path: Option<&str>,
        remote_repo_path: Option<&str>,
    ) -> Project {
        Project {
            id: "project-1".to_string(),
            name: "demo".to_string(),
            description: None,
            status: "active".to_string(),
            repo_path: repo_path.map(str::to_string),
            project_type: project_type.to_string(),
            ssh_config_id: Some("ssh-1".to_string()),
            remote_repo_path: remote_repo_path.map(str::to_string),
            test_command: None,
            deleted_at: None,
            created_at: "2026-04-17 00:00:00".to_string(),
            updated_at: "2026-04-17 00:00:00".to_string(),
        }
    }

    fn build_task(project_id: &str) -> Task {
        Task {
            id: "task-1".to_string(),
            title: "demo task".to_string(),
            description: None,
            status: "review".to_string(),
            priority: "medium".to_string(),
            project_id: project_id.to_string(),
            use_worktree: true,
            assignee_id: Some("emp-1".to_string()),
            reviewer_id: Some("reviewer-1".to_string()),
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: Some("review_fix_loop_v1".to_string()),
            last_codex_session_id: Some("exec-1".to_string()),
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
            native_subagent_id: None,
            created_at: "2026-04-17 00:00:00".to_string(),
            updated_at: "2026-04-17 00:00:00".to_string(),
        }
    }

    #[test]
    fn resolves_local_execution_worktree_for_local_project() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let project = build_project(PROJECT_TYPE_LOCAL, Some("/tmp/demo"), None);
            let task = build_task(&project.id);

            sqlx::query(
                r#"
                INSERT INTO projects (
                    id,
                    name,
                    description,
                    status,
                    repo_path,
                    project_type,
                    created_at,
                    updated_at
                ) VALUES ($1, $2, NULL, 'active', $3, 'local', '2026-04-17 00:00:00', '2026-04-17 00:00:00')
                "#,
            )
            .bind(&project.id)
            .bind(&project.name)
            .bind("/tmp/demo")
            .execute(&pool)
            .await
            .expect("insert project");

            sqlx::query(
                r#"
                INSERT INTO tasks (
                    id,
                    title,
                    description,
                    status,
                    priority,
                    project_id,
                    use_worktree,
                    assignee_id,
                    reviewer_id,
                    automation_mode,
                    last_codex_session_id,
                    last_review_session_id,
                    created_at,
                    updated_at
                ) VALUES ($1, $2, NULL, 'review', 'medium', $3, 1, NULL, NULL, 'review_fix_loop_v1', $4, NULL, '2026-04-17 00:00:00', '2026-04-17 00:00:00')
                "#,
            )
            .bind(&task.id)
            .bind(&task.title)
            .bind(&project.id)
            .bind(task.last_codex_session_id.as_deref())
            .execute(&pool)
            .await
            .expect("insert task");

            sqlx::query(
                r#"
                INSERT INTO task_git_contexts (
                    id,
                    task_id,
                    project_id,
                    base_branch,
                    task_branch,
                    target_branch,
                    worktree_path,
                    repo_head_commit_at_prepare,
                    state,
                    context_version,
                    created_at,
                    updated_at
                ) VALUES (
                    $1, $2, $3, 'main', 'codex/task-task-1', 'main', $4, NULL, 'merge_ready', 1, '2026-04-17 00:00:00', '2026-04-17 00:00:00'
                )
                "#,
            )
            .bind("ctx-1")
            .bind(&task.id)
            .bind(&project.id)
            .bind("/tmp/demo/.codex-ai-worktrees/task-1")
            .execute(&pool)
            .await
            .expect("insert task git context");

            sqlx::query(
                r#"
                INSERT INTO codex_sessions (
                    id,
                    task_id,
                    project_id,
                    task_git_context_id,
                    working_dir,
                    execution_target,
                    artifact_capture_mode,
                    session_kind,
                    status,
                    started_at,
                    created_at
                ) VALUES ($1, $2, $3, $4, $5, 'local', 'local_full', 'execution', 'exited', '2026-04-17 00:00:01', '2026-04-17 00:00:01')
                "#,
            )
            .bind("exec-1")
            .bind(&task.id)
            .bind(&project.id)
            .bind("ctx-1")
            .bind("/tmp/demo/.codex-ai-worktrees/task-1")
            .execute(&pool)
            .await
            .expect("insert execution session");

            let context = resolve_automation_execution_context(&pool, &task, &project)
                .await
                .expect("resolve automation execution context");

            assert_eq!(context.working_dir, "/tmp/demo/.codex-ai-worktrees/task-1");
            assert_eq!(context.task_git_context_id.as_deref(), Some("ctx-1"));

            pool.close().await;
        });
    }

    #[test]
    fn resolves_remote_execution_worktree_for_ssh_project() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let project = build_project(PROJECT_TYPE_SSH, None, Some("/srv/demo"));
            let task = build_task(&project.id);

            sqlx::query(
                r#"
                INSERT INTO ssh_configs (
                    id,
                    name,
                    host,
                    port,
                    username,
                    auth_type,
                    private_key_path,
                    known_hosts_mode,
                    password_ref,
                    passphrase_ref,
                    last_checked_at,
                    last_check_status,
                    last_check_message,
                    password_probe_checked_at,
                    password_probe_status,
                    password_probe_message,
                    created_at,
                    updated_at
                ) VALUES (
                    'ssh-1', 'SSH Demo', 'example.com', 22, 'demo', 'key', NULL, 'accept-new',
                    NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '2026-04-17 00:00:00', '2026-04-17 00:00:00'
                )
                "#,
            )
            .execute(&pool)
            .await
            .expect("insert ssh config");

            sqlx::query(
                r#"
                INSERT INTO projects (
                    id,
                    name,
                    description,
                    status,
                    repo_path,
                    project_type,
                    ssh_config_id,
                    remote_repo_path,
                    created_at,
                    updated_at
                ) VALUES ($1, $2, NULL, 'active', NULL, 'ssh', 'ssh-1', $3, '2026-04-17 00:00:00', '2026-04-17 00:00:00')
                "#,
            )
            .bind(&project.id)
            .bind(&project.name)
            .bind("/srv/demo")
            .execute(&pool)
            .await
            .expect("insert ssh project");

            sqlx::query(
                r#"
                INSERT INTO tasks (
                    id,
                    title,
                    description,
                    status,
                    priority,
                    project_id,
                    use_worktree,
                    assignee_id,
                    reviewer_id,
                    automation_mode,
                    last_codex_session_id,
                    last_review_session_id,
                    created_at,
                    updated_at
                ) VALUES ($1, $2, NULL, 'review', 'medium', $3, 1, NULL, NULL, 'review_fix_loop_v1', $4, NULL, '2026-04-17 00:00:00', '2026-04-17 00:00:00')
                "#,
            )
            .bind(&task.id)
            .bind(&task.title)
            .bind(&project.id)
            .bind(task.last_codex_session_id.as_deref())
            .execute(&pool)
            .await
            .expect("insert ssh task");

            sqlx::query(
                r#"
                INSERT INTO task_git_contexts (
                    id,
                    task_id,
                    project_id,
                    base_branch,
                    task_branch,
                    target_branch,
                    worktree_path,
                    state,
                    context_version,
                    created_at,
                    updated_at
                ) VALUES (
                    $1, $2, $3, 'main', 'codex/task-task-1', 'main', $4, 'merge_ready', 1, '2026-04-17 00:00:00', '2026-04-17 00:00:00'
                )
                "#,
            )
            .bind("ctx-ssh-1")
            .bind(&task.id)
            .bind(&project.id)
            .bind("/srv/demo/.codex-ai-worktrees/task-1")
            .execute(&pool)
            .await
            .expect("insert ssh task git context");

            sqlx::query(
                r#"
                INSERT INTO codex_sessions (
                    id,
                    task_id,
                    project_id,
                    task_git_context_id,
                    working_dir,
                    execution_target,
                    artifact_capture_mode,
                    session_kind,
                    status,
                    started_at,
                    created_at
                ) VALUES ($1, $2, $3, $4, $5, 'ssh', 'ssh_full', 'execution', 'exited', '2026-04-17 00:00:01', '2026-04-17 00:00:01')
                "#,
            )
            .bind("exec-ssh-1")
            .bind(&task.id)
            .bind(&project.id)
            .bind("ctx-ssh-1")
            .bind("/srv/demo/.codex-ai-worktrees/task-1")
            .execute(&pool)
            .await
            .expect("insert ssh execution session");

            let context = resolve_automation_execution_context(&pool, &task, &project)
                .await
                .expect("resolve remote execution worktree");

            assert_eq!(context.working_dir, "/srv/demo/.codex-ai-worktrees/task-1");
            assert_eq!(context.task_git_context_id.as_deref(), Some("ctx-ssh-1"));

            pool.close().await;
        });
    }
}

#[cfg(test)]
mod automation_guard_tests {
    use sqlx::SqlitePool;

    use super::{
        fetch_pending_automation_task_ids, fetch_session_exit_facts,
        record_automation_completed_without_auto_commit, recover_fix_verdict_json_for_task,
        resolve_restart_target, should_auto_commit_task_worktree,
        validate_task_automation_restart, AutomationRestartTarget,
        AUTOMATION_MODE_REVIEW_FIX_LOOP_V1, PHASE_BLOCKED, PHASE_COMMITTING_CODE,
        PHASE_COMMIT_FAILED, PHASE_COMPLETED, PHASE_MANUAL_CONTROL, PHASE_REVIEW_LAUNCH_FAILED,
        WORKTREE_DISABLED_AUTO_COMMIT_SKIPPED_MESSAGE,
    };
    use crate::app::{build_current_migrator, TASK_STATUS_ARCHIVED};
    use crate::db::models::{Task, TaskAutomationStateRecord};

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

    fn build_task(task_id: &str, status: &str) -> Task {
        Task {
            id: task_id.to_string(),
            title: format!("task {task_id}"),
            description: None,
            status: status.to_string(),
            priority: "medium".to_string(),
            project_id: "project-1".to_string(),
            use_worktree: false,
            assignee_id: None,
            reviewer_id: None,
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1.to_string()),
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
            native_subagent_id: None,
            created_at: "2026-04-21 00:00:00".to_string(),
            updated_at: "2026-04-21 00:00:00".to_string(),
        }
    }

    async fn insert_project(pool: &SqlitePool) {
        sqlx::query(
            r#"
            INSERT INTO projects (
                id,
                name,
                description,
                status,
                repo_path,
                created_at,
                updated_at
            ) VALUES ('project-1', 'demo', NULL, 'active', NULL, '2026-04-21 00:00:00', '2026-04-21 00:00:00')
            "#,
        )
        .execute(pool)
        .await
        .expect("insert project");
    }

    async fn insert_task(pool: &SqlitePool, task: &Task) {
        sqlx::query(
            r#"
            INSERT INTO tasks (
                id,
                title,
                description,
                status,
                priority,
                project_id,
                use_worktree,
                assignee_id,
                reviewer_id,
                automation_mode,
                created_at,
                updated_at
            ) VALUES ($1, $2, NULL, $3, 'medium', $4, 0, NULL, NULL, $5, $6, $7)
            "#,
        )
        .bind(&task.id)
        .bind(&task.title)
        .bind(&task.status)
        .bind(&task.project_id)
        .bind(task.automation_mode.as_deref())
        .bind(&task.created_at)
        .bind(&task.updated_at)
        .execute(pool)
        .await
        .expect("insert task");
    }

    async fn insert_automation_state(
        pool: &SqlitePool,
        task: &Task,
        phase: &str,
        last_error: Option<&str>,
    ) {
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
            ) VALUES ($1, $2, 0, 'session-review', 'session-review', NULL, NULL, $3, $4, '2026-04-21 00:00:02')
            "#,
        )
        .bind(&task.id)
        .bind(phase)
        .bind(last_error)
        .bind(
            r#"{"passed":true,"needs_human":false,"blocking_issue_count":0,"summary":"审核通过。"}"#,
        )
        .execute(pool)
        .await
        .expect("insert automation state");
    }

    async fn insert_session(
        pool: &SqlitePool,
        session_id: &str,
        task_id: &str,
        session_kind: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO codex_sessions (
                id,
                task_id,
                project_id,
                execution_target,
                artifact_capture_mode,
                session_kind,
                status,
                started_at,
                created_at
            ) VALUES ($1, $2, 'project-1', 'local', 'local_full', $3, 'exited', '2026-04-21 00:00:01', '2026-04-21 00:00:01')
            "#,
        )
        .bind(session_id)
        .bind(task_id)
        .bind(session_kind)
        .execute(pool)
        .await
        .expect("insert session");
    }

    async fn insert_session_event(
        pool: &SqlitePool,
        event_id: &str,
        session_id: &str,
        event_type: &str,
        message: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO codex_session_events (id, session_id, event_type, message, created_at)
            VALUES ($1, $2, $3, $4, '2026-04-21 00:00:02')
            "#,
        )
        .bind(event_id)
        .bind(session_id)
        .bind(event_type)
        .bind(message)
        .execute(pool)
        .await
        .expect("insert session event");
    }

    #[test]
    fn worktree_disabled_tasks_do_not_auto_commit_after_passed_review() {
        let mut task = build_task("task-no-worktree", "review");
        assert!(!should_auto_commit_task_worktree(&task));

        task.use_worktree = true;
        assert!(should_auto_commit_task_worktree(&task));
    }

    #[test]
    fn worktree_disabled_completion_marks_automation_completed_without_commit_phase() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            insert_project(&pool).await;
            let task = build_task("task-review-passed", "review");
            insert_task(&pool, &task).await;
            insert_automation_state(&pool, &task, PHASE_REVIEW_LAUNCH_FAILED, None).await;

            record_automation_completed_without_auto_commit(
                &pool,
                &task,
                Some("session-review"),
                Some(
                    r#"{"passed":true,"needs_human":false,"blocking_issue_count":0,"summary":"审核通过。"}"#,
                ),
                None,
                None,
            )
            .await
            .expect("record automation completion without auto commit");

            let (phase, last_error): (String, Option<String>) = sqlx::query_as(
                "SELECT phase, last_error FROM task_automation_state WHERE task_id = $1",
            )
            .bind(&task.id)
            .fetch_one(&pool)
            .await
            .expect("fetch automation state");
            assert_eq!(phase, PHASE_COMPLETED);
            assert!(last_error.is_none());

            let completed_detail: Option<String> = sqlx::query_scalar(
                "SELECT details FROM activity_logs WHERE task_id = $1 AND action = 'task_automation_completed' ORDER BY created_at DESC LIMIT 1",
            )
            .bind(&task.id)
            .fetch_one(&pool)
            .await
            .expect("fetch completion activity");
            assert_eq!(
                completed_detail.as_deref(),
                Some(WORKTREE_DISABLED_AUTO_COMMIT_SKIPPED_MESSAGE)
            );

            pool.close().await;
        });
    }

    #[test]
    fn pending_commit_for_worktree_disabled_task_completes_without_git_context() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            insert_project(&pool).await;

            for (task_id, initial_phase, last_error) in [
                ("task-pending-commit", PHASE_COMMITTING_CODE, None),
                ("task-failed-commit", PHASE_COMMIT_FAILED, Some("缺少 Git worktree")),
            ] {
                let task = build_task(task_id, "review");
                insert_task(&pool, &task).await;
                insert_automation_state(&pool, &task, initial_phase, last_error).await;

                let result = record_automation_completed_without_auto_commit(
                    &pool,
                    &task,
                    Some("session-review"),
                    Some(
                        r#"{"passed":true,"needs_human":false,"blocking_issue_count":0,"summary":"审核通过。"}"#,
                    ),
                    None,
                    None,
                )
                .await;
                result.expect("pending commit should complete non-worktree tasks");

                let (phase, last_error): (String, Option<String>) = sqlx::query_as(
                    "SELECT phase, last_error FROM task_automation_state WHERE task_id = $1",
                )
                .bind(&task.id)
                .fetch_one(&pool)
                .await
                .expect("fetch automation state");
                assert_eq!(phase, PHASE_COMPLETED);
                assert!(last_error.is_none());

                let commit_failed_count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM activity_logs WHERE task_id = $1 AND action = 'task_automation_commit_failed'",
                )
                .bind(&task.id)
                .fetch_one(&pool)
                .await
                .expect("count commit failed logs");
                assert_eq!(commit_failed_count, 0);
            }

            pool.close().await;
        });
    }

    #[test]
    fn archived_tasks_are_excluded_from_pending_automation_resume_queue() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            sqlx::query(
                r#"
                INSERT INTO projects (
                    id,
                    name,
                    description,
                    status,
                    repo_path,
                    created_at,
                    updated_at
                ) VALUES ('project-1', 'demo', NULL, 'active', NULL, '2026-04-21 00:00:00', '2026-04-21 00:00:00')
                "#,
            )
            .execute(&pool)
            .await
            .expect("insert project");

            for task in [
                build_task("task-active", "review"),
                build_task("task-archived", TASK_STATUS_ARCHIVED),
            ] {
                sqlx::query(
                    r#"
                    INSERT INTO tasks (
                        id,
                        title,
                        description,
                        status,
                        priority,
                        project_id,
                        use_worktree,
                        assignee_id,
                        reviewer_id,
                        automation_mode,
                        created_at,
                        updated_at
                    ) VALUES ($1, $2, NULL, $3, 'medium', $4, 0, NULL, NULL, $5, $6, $7)
                    "#,
                )
                .bind(&task.id)
                .bind(&task.title)
                .bind(&task.status)
                .bind(&task.project_id)
                .bind(task.automation_mode.as_deref())
                .bind(&task.created_at)
                .bind(&task.updated_at)
                .execute(&pool)
                .await
                .expect("insert task");

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
                    ) VALUES ($1, $2, 0, NULL, NULL, 'start_review', NULL, NULL, NULL, '2026-04-21 00:00:00')
                    "#,
                )
                .bind(&task.id)
                .bind(PHASE_REVIEW_LAUNCH_FAILED)
                .execute(&pool)
                .await
                .expect("insert task automation state");
            }

            let task_ids = fetch_pending_automation_task_ids(&pool)
                .await
                .expect("load pending automation task ids");
            assert_eq!(task_ids, vec!["task-active".to_string()]);

            pool.close().await;
        });
    }

    #[test]
    fn archived_task_restart_is_rejected() {
        let task = build_task("task-archived", TASK_STATUS_ARCHIVED);

        let error = validate_task_automation_restart(&task).expect_err("archived task restart");
        assert_eq!(error, "已归档任务不能重启自动质控");
    }

    #[test]
    fn blocked_review_state_with_recoverable_verdict_resolves_fix_restart_target() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-review", "blocked");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-review", &task.id, "review").await;
            insert_session_event(
                &pool,
                "event-review-1",
                "session-review",
                "stdout",
                r#"<review_verdict>{"passed":false,"needs_human":false,"blocking_issue_count":1,"summary":"发现 1 个阻断问题。"}<\/review_verdict>"#,
            )
            .await;

            let state = TaskAutomationStateRecord {
                task_id: task.id.clone(),
                phase: PHASE_BLOCKED.to_string(),
                round_count: 0,
                consumed_session_id: Some("session-review".to_string()),
                last_trigger_session_id: None,
                pending_action: None,
                pending_round_count: None,
                last_error: Some("审核结果结构化输出无效".to_string()),
                last_verdict_json: None,
                pipeline_active: false,
            pipeline_step_index: None,
            updated_at: "2026-04-21 00:00:02".to_string(),
            };

            let target = resolve_restart_target(&pool, &state)
                .await
                .expect("resolve blocked review target");
            assert_eq!(target, Some(AutomationRestartTarget::Fix));

            pool.close().await;
        });
    }

    #[test]
    fn blocked_review_state_without_recoverable_verdict_resolves_review_restart_target() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-review-fallback", "blocked");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-review-fallback", &task.id, "review").await;

            let state = TaskAutomationStateRecord {
                task_id: task.id.clone(),
                phase: PHASE_BLOCKED.to_string(),
                round_count: 0,
                consumed_session_id: Some("session-review-fallback".to_string()),
                last_trigger_session_id: None,
                pending_action: None,
                pending_round_count: None,
                last_error: Some("审核结果结构化输出无效".to_string()),
                last_verdict_json: None,
                pipeline_active: false,
            pipeline_step_index: None,
            updated_at: "2026-04-21 00:00:02".to_string(),
            };

            let target = resolve_restart_target(&pool, &state)
                .await
                .expect("resolve blocked review fallback target");
            assert_eq!(target, Some(AutomationRestartTarget::Review));

            pool.close().await;
        });
    }

    #[test]
    fn manual_control_execution_state_resolves_fix_restart_target() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-execution", "blocked");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-execution", &task.id, "execution").await;

            let state = TaskAutomationStateRecord {
                task_id: task.id.clone(),
                phase: PHASE_MANUAL_CONTROL.to_string(),
                round_count: 1,
                consumed_session_id: Some("session-execution".to_string()),
                last_trigger_session_id: Some("session-execution".to_string()),
                pending_action: None,
                pending_round_count: None,
                last_error: Some("执行已被人工停止".to_string()),
                last_verdict_json: Some(
                    r#"{"passed":false,"needs_human":false,"blocking_issue_count":1,"summary":"发现 1 个阻断问题。"}"#
                        .to_string(),
                ),
                pipeline_active: false,
                pipeline_step_index: None,
                updated_at: "2026-04-21 00:00:02".to_string(),
            };

            let target = resolve_restart_target(&pool, &state)
                .await
                .expect("resolve manual control execution target");
            assert_eq!(target, Some(AutomationRestartTarget::Fix));

            pool.close().await;
        });
    }

    #[test]
    fn review_exit_facts_recover_verdict_from_stdout_when_event_missing() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-native-review", "review");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-native-review", &task.id, "review").await;
            insert_session_event(
                &pool,
                "evt-native-prompt",
                "session-native-review",
                "stdout",
                "[USER_INPUT] 最终判定必须输出在 <review_verdict> 和 </review_verdict> 之间",
            )
            .await;
            insert_session_event(
                &pool,
                "evt-native-answer",
                "session-native-review",
                "stdout",
                concat!(
                    "我已完成本次只读审查。\n",
                    r#"<review_verdict>{"passed":false,"needs_human":true,"blocking_issue_count":1,"summary":"混入无关改动"}</review_verdict>"#,
                ),
            )
            .await;

            let facts = fetch_session_exit_facts(&pool, "session-native-review")
                .await
                .expect("fetch exit facts")
                .expect("facts present");
            let verdict = facts.review_verdict.expect("recovered verdict");
            assert!(!verdict.passed);
            assert!(verdict.needs_human);
            assert_eq!(verdict.blocking_issue_count, 1);
            assert_eq!(verdict.summary, "混入无关改动");

            pool.close().await;
        });
    }

    #[test]
    fn review_exit_facts_prefer_stored_verdict_event_over_stdout() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-stored-review", "review");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-stored-review", &task.id, "review").await;
            insert_session_event(
                &pool,
                "evt-stdout",
                "session-stored-review",
                "stdout",
                r#"<review_verdict>{"passed":false,"needs_human":true,"blocking_issue_count":9,"summary":"stdout 旧结论"}</review_verdict>"#,
            )
            .await;
            insert_session_event(
                &pool,
                "evt-verdict",
                "session-stored-review",
                "review_verdict",
                r#"{"passed":true,"needs_human":false,"blocking_issue_count":0,"summary":"事件结论优先"}"#,
            )
            .await;

            let facts = fetch_session_exit_facts(&pool, "session-stored-review")
                .await
                .expect("fetch exit facts")
                .expect("facts present");
            let verdict = facts.review_verdict.expect("stored verdict");
            assert!(verdict.passed);
            assert_eq!(verdict.summary, "事件结论优先");

            pool.close().await;
        });
    }

    #[test]
    fn execution_exit_facts_without_stopping_event_are_not_manual_stop() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-exec-ok", "in_progress");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-exec-ok", &task.id, "execution").await;

            let facts = fetch_session_exit_facts(&pool, "session-exec-ok")
                .await
                .expect("fetch exit facts")
                .expect("facts present");
            assert!(!facts.has_stopping_requested);
            assert!(!facts.has_restart_requested);

            pool.close().await;
        });
    }

    #[test]
    fn execution_exit_facts_with_stopping_event_are_manual_stop() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-exec-stop", "in_progress");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-exec-stop", &task.id, "execution").await;
            insert_session_event(
                &pool,
                "evt-stop",
                "session-exec-stop",
                "stopping_requested",
                "收到停止请求",
            )
            .await;

            let facts = fetch_session_exit_facts(&pool, "session-exec-stop")
                .await
                .expect("fetch exit facts")
                .expect("facts present");
            assert!(facts.has_stopping_requested);
            assert!(!facts.has_restart_requested);

            pool.close().await;
        });
    }

    #[test]
    fn execution_exit_facts_with_restart_event_are_restart() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-exec-restart", "in_progress");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-exec-restart", &task.id, "execution").await;
            insert_session_event(
                &pool,
                "evt-restart",
                "session-exec-restart",
                "automation_restart_requested",
                "自动质控正在重启执行步骤",
            )
            .await;

            let facts = fetch_session_exit_facts(&pool, "session-exec-restart")
                .await
                .expect("fetch exit facts")
                .expect("facts present");
            assert!(facts.has_restart_requested);
            assert!(!facts.has_stopping_requested);

            pool.close().await;
        });
    }

    #[test]
    fn fix_restart_recovers_verdict_from_latest_review_when_consumed_is_execution() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-fix-recover", "in_progress");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-review-ok", &task.id, "review").await;
            insert_session(&pool, "session-fix-failed", &task.id, "execution").await;
            insert_session_event(
                &pool,
                "evt-review-stdout",
                "session-review-ok",
                "stdout",
                r#"<review_verdict>{"passed":false,"needs_human":true,"blocking_issue_count":1,"summary":"混入无关改动"}</review_verdict>"#,
            )
            .await;
            insert_session_event(
                &pool,
                "evt-fix-error",
                "session-fix-failed",
                "stdout",
                "[ERROR] 模型请求失败（HTTP 400）: max_tokens is too large: 384000",
            )
            .await;

            let state = TaskAutomationStateRecord {
                task_id: task.id.clone(),
                phase: PHASE_MANUAL_CONTROL.to_string(),
                round_count: 0,
                consumed_session_id: Some("session-fix-failed".to_string()),
                last_trigger_session_id: Some("session-fix-failed".to_string()),
                pending_action: None,
                pending_round_count: None,
                last_error: Some("自动修复执行异常失败，需人工接管".to_string()),
                last_verdict_json: None,
                pipeline_active: false,
                pipeline_step_index: None,
                updated_at: "2026-08-21 06:32:10".to_string(),
            };

            let recovered = recover_fix_verdict_json_for_task(&pool, &task.id, &state)
                .await
                .expect("recover verdict")
                .expect("verdict from latest review");
            assert!(recovered.contains("混入无关改动"));

            pool.close().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{is_no_reviewable_code_changes_error, restart_fix_round_state, PHASE_BLOCKED};
    use crate::db::models::TaskAutomationStateRecord;

    #[test]
    fn blocked_phase_constant_kept_stable() {
        assert_eq!(PHASE_BLOCKED, "blocked");
    }

    #[test]
    fn no_reviewable_code_changes_errors_are_terminal_automation_outcomes() {
        assert!(is_no_reviewable_code_changes_error(
            "当前工作区没有可审核的代码改动"
        ));
        assert!(is_no_reviewable_code_changes_error(
            "当前工作区没有可审核的代码 diff"
        ));
        assert!(!is_no_reviewable_code_changes_error("审核员启动失败"));
    }

    #[test]
    fn blocked_fix_restart_resets_round_count() {
        let state = TaskAutomationStateRecord {
            task_id: "task-1".to_string(),
            phase: PHASE_BLOCKED.to_string(),
            round_count: 3,
            consumed_session_id: Some("session-1".to_string()),
            last_trigger_session_id: Some("session-1".to_string()),
            pending_action: None,
            pending_round_count: Some(4),
            last_error: Some("blocked".to_string()),
            last_verdict_json: None,
            pipeline_active: false,
            pipeline_step_index: None,
            updated_at: "2026-04-22 00:00:00".to_string(),
        };

        assert_eq!(restart_fix_round_state(&state), (Some(0), 0));
    }

    #[test]
    fn manual_control_fix_restart_resets_round_count() {
        let state = TaskAutomationStateRecord {
            task_id: "task-1".to_string(),
            phase: "manual_control".to_string(),
            round_count: 3,
            consumed_session_id: Some("session-1".to_string()),
            last_trigger_session_id: Some("session-1".to_string()),
            pending_action: None,
            pending_round_count: Some(4),
            last_error: Some("manual".to_string()),
            last_verdict_json: None,
            pipeline_active: false,
            pipeline_step_index: None,
            updated_at: "2026-04-22 00:00:00".to_string(),
        };

        assert_eq!(restart_fix_round_state(&state), (Some(0), 0));
    }

    #[test]
    fn non_terminal_fix_restart_keeps_existing_round_count() {
        let state = TaskAutomationStateRecord {
            task_id: "task-1".to_string(),
            phase: "launching_fix".to_string(),
            round_count: 3,
            consumed_session_id: Some("session-1".to_string()),
            last_trigger_session_id: Some("session-1".to_string()),
            pending_action: None,
            pending_round_count: Some(4),
            last_error: Some("launching".to_string()),
            last_verdict_json: None,
            pipeline_active: false,
            pipeline_step_index: None,
            updated_at: "2026-04-22 00:00:00".to_string(),
        };

        assert_eq!(restart_fix_round_state(&state), (Some(4), 3));
    }
}

#[cfg(test)]
mod coordinator_session_kind_tests {
    use super::latest_execution_session_id;
    use crate::app::build_current_migrator;

    #[test]
    fn latest_execution_session_id_ignores_coordinator_rows() {
        tauri::async_runtime::block_on(async {
            let pool = sqlx::SqlitePool::connect("sqlite::memory:")
                .await
                .expect("create sqlite memory pool");
            let migrator = build_current_migrator();
            let mut connection = pool.acquire().await.expect("acquire sqlite connection");
            migrator
                .run_direct(&mut *connection)
                .await
                .expect("run migrations");
            drop(connection);

            sqlx::query(
                r#"
                INSERT INTO projects (id, name, status, project_type, created_at, updated_at)
                VALUES ('proj-1', 'demo', 'active', 'local', '2026-04-28 00:00:00', '2026-04-28 00:00:00')
                "#,
            )
            .execute(&pool)
            .await
            .expect("insert project");
            sqlx::query(
                r#"
                INSERT INTO tasks (
                    id, title, status, priority, project_id, use_worktree, automation_mode,
                    created_at, updated_at
                ) VALUES (
                    'task-1', 'demo', 'todo', 'medium', 'proj-1', 0, 'off',
                    '2026-04-28 00:00:00', '2026-04-28 00:00:00'
                )
                "#,
            )
            .execute(&pool)
            .await
            .expect("insert task");
            sqlx::query(
                r#"
                INSERT INTO codex_sessions (id, task_id, session_kind, status, started_at, created_at)
                VALUES
                    ('sess-exec', 'task-1', 'execution', 'exited', '2026-04-28 09:00:00', '2026-04-28 09:00:00'),
                    ('sess-coord', 'task-1', 'coordinator', 'exited', '2026-04-28 10:00:00', '2026-04-28 10:00:00')
                "#,
            )
            .execute(&pool)
            .await
            .expect("insert sessions");

            let latest = latest_execution_session_id(&pool, "task-1")
                .await
                .expect("resolve latest execution session");
            assert_eq!(latest.as_deref(), Some("sess-exec"));
            pool.close().await;
        });
    }
}

