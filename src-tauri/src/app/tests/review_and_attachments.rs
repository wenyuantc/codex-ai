use super::*;

#[test]
fn remote_task_attachment_dir_uses_home_scoped_task_folder() {
    assert_eq!(
        remote_task_attachment_dir("/home/demo", "task-1"),
        "/home/demo/.codex-ai/img/task-1"
    );
}

#[test]
fn remote_task_attachment_path_reuses_managed_file_name() {
    let attachment = TaskAttachment {
        id: "att-1".to_string(),
        task_id: "task-1".to_string(),
        original_name: "ui.png".to_string(),
        stored_path: "/tmp/task-attachments/task-1/att-1.png".to_string(),
        mime_type: "image/png".to_string(),
        file_size: 123,
        sort_order: 1,
        created_at: "2026-04-16 10:00:00".to_string(),
    };

    assert_eq!(
        remote_task_attachment_path("/home/demo", &attachment).expect("remote attachment path"),
        "/home/demo/.codex-ai/img/task-1/att-1.png"
    );
}

#[test]
fn task_attachment_is_image_accepts_image_records_and_rejects_non_image_records() {
    let image_attachment = TaskAttachment {
        id: "att-img".to_string(),
        task_id: "task-1".to_string(),
        original_name: "ui.png".to_string(),
        stored_path: "/tmp/task-attachments/task-1/att-img.png".to_string(),
        mime_type: "image/png".to_string(),
        file_size: 123,
        sort_order: 1,
        created_at: "2026-04-16 10:00:00".to_string(),
    };
    let file_attachment = TaskAttachment {
        id: "att-pdf".to_string(),
        task_id: "task-1".to_string(),
        original_name: "spec.pdf".to_string(),
        stored_path: "/tmp/task-attachments/task-1/att-pdf.pdf".to_string(),
        mime_type: "application/pdf".to_string(),
        file_size: 456,
        sort_order: 2,
        created_at: "2026-04-16 10:00:00".to_string(),
    };

    assert!(task_attachment_is_image(&image_attachment));
    assert!(!task_attachment_is_image(&file_attachment));
}

#[test]
fn filter_image_attachments_keeps_only_image_records() {
    let image_attachment = TaskAttachment {
        id: "att-img".to_string(),
        task_id: "task-1".to_string(),
        original_name: "ui.png".to_string(),
        stored_path: "/tmp/task-attachments/task-1/att-img.png".to_string(),
        mime_type: "image/png".to_string(),
        file_size: 123,
        sort_order: 1,
        created_at: "2026-04-16 10:00:00".to_string(),
    };
    let file_attachment = TaskAttachment {
        id: "att-pdf".to_string(),
        task_id: "task-1".to_string(),
        original_name: "spec.pdf".to_string(),
        stored_path: "/tmp/task-attachments/task-1/att-pdf.pdf".to_string(),
        mime_type: "application/pdf".to_string(),
        file_size: 456,
        sort_order: 2,
        created_at: "2026-04-16 10:00:00".to_string(),
    };

    let filtered = filter_image_attachments(&[image_attachment.clone(), file_attachment]);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, image_attachment.id);
}

#[test]
fn review_context_builder_accepts_remote_untracked_summary_without_snippets() {
    let context = build_task_review_context_from_git_outputs(
        " M src/main.rs\n?? notes.txt\n",
        " src/main.rs | 2 ++\n 1 file changed, 2 insertions(+)\n",
        "diff --git a/src/main.rs b/src/main.rs\n+println!(\"hi\");\n",
        "",
        "",
        &["notes.txt".to_string()],
        "未跟踪文件列表：\n- notes.txt\n\n未跟踪文本文件摘录：\n（SSH 模式暂不采集远程未跟踪文件内容摘录，请结合未跟踪文件列表人工确认）",
    )
    .expect("build review context");

    assert!(context.contains("## Git 状态"));
    assert!(context.contains("notes.txt"));
    assert!(context.contains("SSH 模式暂不采集远程未跟踪文件内容摘录"));
}

#[test]
fn review_prompt_uses_explicit_remote_working_dir_for_ssh_projects() {
    let task = Task {
        id: "task-1".to_string(),
        title: "审核远程改动".to_string(),
        description: Some("检查 SSH 项目的改动".to_string()),
        status: "review".to_string(),
        priority: "high".to_string(),
        project_id: "project-1".to_string(),
        use_worktree: true,
        assignee_id: None,
        reviewer_id: Some("reviewer-1".to_string()),
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
        native_subagent_id: None,
        created_at: "2026-04-16 10:00:00".to_string(),
        updated_at: "2026-04-16 10:00:00".to_string(),
    };
    let project = Project {
        id: "project-1".to_string(),
        name: "SSH 项目".to_string(),
        description: None,
        status: "active".to_string(),
        repo_path: None,
        project_type: PROJECT_TYPE_SSH.to_string(),
        ssh_config_id: Some("ssh-1".to_string()),
        remote_repo_path: Some("/srv/demo".to_string()),
        test_command: None,
        deleted_at: None,
        created_at: "2026-04-16 10:00:00".to_string(),
        updated_at: "2026-04-16 10:00:00".to_string(),
    };

    let prompt =
        build_task_review_prompt(&task, &project, "/srv/demo", "## Git 状态\n M src/main.rs");

    assert!(prompt.contains("仓库路径：/srv/demo"));
    assert!(prompt.contains("执行目标：SSH 远程工作区"));
    assert!(!prompt.contains("仓库路径：（未配置）"));
    assert!(prompt.contains("<review_findings>"));
    assert!(prompt.contains("</review_findings>"));
}

#[test]
fn review_prompt_marks_local_projects_as_local_workspace() {
    let task = Task {
        id: "task-2".to_string(),
        title: "审核本地改动".to_string(),
        description: None,
        status: "review".to_string(),
        priority: "medium".to_string(),
        project_id: "project-2".to_string(),
        use_worktree: true,
        assignee_id: None,
        reviewer_id: Some("reviewer-2".to_string()),
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
        native_subagent_id: None,
        created_at: "2026-04-16 10:00:00".to_string(),
        updated_at: "2026-04-16 10:00:00".to_string(),
    };
    let project = Project {
        id: "project-2".to_string(),
        name: "本地项目".to_string(),
        description: None,
        status: "active".to_string(),
        repo_path: Some("/tmp/demo".to_string()),
        project_type: PROJECT_TYPE_LOCAL.to_string(),
        ssh_config_id: None,
        remote_repo_path: None,
        test_command: None,
        deleted_at: None,
        created_at: "2026-04-16 10:00:00".to_string(),
        updated_at: "2026-04-16 10:00:00".to_string(),
    };

    let prompt =
        build_task_review_prompt(&task, &project, "/tmp/demo", "## Git 状态\n M src/main.rs");

    assert!(prompt.contains("执行目标：本地工作区"));
    assert!(prompt.contains("<review_findings>"));
    assert!(prompt.contains("</review_findings>"));
}

#[test]
fn default_review_prompt_template_requires_findings_block() {
    let template =
        crate::codex::find_default_ai_prompt_template("review").expect("default review template");
    assert!(template.scene_requirement.contains("<review_findings>"));
    assert!(template.scene_requirement.contains("</review_findings>"));
}

#[test]
fn local_review_context_prefers_latest_execution_worktree() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    runtime.block_on(async {
        let pool = setup_test_pool().await;
        let repo_root = std::env::temp_dir().join(format!("codex-ai-review-root-{}", uuid::Uuid::new_v4()));
        let worktree_root =
            std::env::temp_dir().join(format!("codex-ai-review-worktree-{}", uuid::Uuid::new_v4()));

        fs::create_dir_all(&repo_root).expect("create review repo root");
        let repo_root_str = repo_root.to_string_lossy().to_string();
        let worktree_root_str = worktree_root.to_string_lossy().to_string();

        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .status()
                .expect("run git command");
            assert!(status.success(), "git {:?} should succeed", args);
        };

        git(&["init", "-b", "main", &repo_root_str]);
        git(&["-C", &repo_root_str, "config", "user.email", "test@example.com"]);
        git(&["-C", &repo_root_str, "config", "user.name", "Test User"]);
        fs::write(repo_root.join("src.txt"), "base\n").expect("write initial file");
        git(&["-C", &repo_root_str, "add", "src.txt"]);
        git(&["-C", &repo_root_str, "commit", "-m", "init"]);
        git(&[
            "-C",
            &repo_root_str,
            "worktree",
            "add",
            "-b",
            "codex/task-task-1",
            &worktree_root_str,
            "main",
        ]);
        fs::write(worktree_root.join("src.txt"), "base\nchange\n").expect("write worktree change");

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
            ) VALUES ($1, $2, NULL, 'active', $3, 'local', '2026-04-16 10:00:00', '2026-04-16 10:00:00')
            "#,
        )
        .bind("proj-review")
        .bind("Review Project")
        .bind(&repo_root_str)
        .execute(&pool)
        .await
        .expect("insert project with repo path");

        let task = Task {
            id: "task-review".to_string(),
            title: "审核 worktree 改动".to_string(),
            description: None,
            status: "review".to_string(),
            priority: "medium".to_string(),
            project_id: "proj-review".to_string(),
            use_worktree: true,
            assignee_id: None,
            reviewer_id: None,
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: None,
            last_codex_session_id: Some("sess-exec-1".to_string()),
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
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        };

        let mut tx = pool.begin().await.expect("begin task transaction");
        insert_task_record(&mut tx, &task)
            .await
            .expect("insert task record");
        tx.commit().await.expect("commit task transaction");

        sqlx::query(
            r#"
            INSERT INTO codex_sessions (
                id,
                task_id,
                project_id,
                working_dir,
                execution_target,
                artifact_capture_mode,
                session_kind,
                status,
                started_at,
                created_at
            ) VALUES ($1, $2, $3, $4, 'local', 'local_full', 'execution', 'exited', '2026-04-16 10:00:01', '2026-04-16 10:00:01')
            "#,
        )
        .bind("sess-exec-1")
        .bind("task-review")
        .bind("proj-review")
        .bind(&worktree_root_str)
        .execute(&pool)
        .await
        .expect("insert execution session");

        let saved_task = fetch_task_by_id(&pool, "task-review")
            .await
            .expect("fetch review task");
        let project = Project {
            id: "proj-review".to_string(),
            name: "Review Project".to_string(),
            description: None,
            status: "active".to_string(),
            repo_path: Some(repo_root_str.clone()),
            project_type: PROJECT_TYPE_LOCAL.to_string(),
            ssh_config_id: None,
            remote_repo_path: None,
            test_command: None,
            deleted_at: None,
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        };

        let (review_working_dir, review_context) =
            collect_local_task_review_context_for_task(&pool, &saved_task, &project)
                .await
                .expect("collect review context from worktree");

        assert_eq!(review_working_dir, worktree_root_str);
        assert!(review_context.contains("src.txt"));
        assert!(review_context.contains("change"));

        let _ = Command::new("git")
            .args(["-C", &repo_root_str, "worktree", "remove", &worktree_root_str, "--force"])
            .status();
        let _ = fs::remove_dir_all(&repo_root);
        let _ = fs::remove_dir_all(&worktree_root);
        pool.close().await;
    });
}

#[test]
fn rewrite_file_change_diff_labels_only_updates_headers() {
    let raw = concat!(
        "diff --git a/before.txt b/after.txt\n",
        "index 1111111..2222222 100644\n",
        "--- a/before.txt\n",
        "+++ b/after.txt\n",
        "@@ -1 +1 @@\n",
        "-const path = \"a/before.txt\";\n",
        "+const path = \"b/after.txt\";\n",
    );

    let rewritten = rewrite_file_change_diff_labels(raw, "src/old.ts", "src/new.ts");

    assert!(rewritten.contains("diff --git a/src/old.ts b/src/new.ts"));
    assert!(rewritten.contains("--- a/src/old.ts"));
    assert!(rewritten.contains("+++ b/src/new.ts"));
    assert!(rewritten.contains("-const path = \"a/before.txt\";"));
    assert!(rewritten.contains("+const path = \"b/after.txt\";"));
}

#[test]
fn rewrite_file_change_diff_labels_keeps_dev_null_unprefixed() {
    let raw = concat!(
        "diff --git a/before.txt b/after.txt\n",
        "--- a/before.txt\n",
        "+++ b/after.txt\n",
    );

    let rewritten = rewrite_file_change_diff_labels(raw, "/dev/null", "src/new.ts");

    assert!(rewritten.contains("diff --git /dev/null b/src/new.ts"));
    assert!(rewritten.contains("--- /dev/null"));
    assert!(rewritten.contains("+++ b/src/new.ts"));
    assert!(!rewritten.contains("a//dev/null"));
    assert!(!rewritten.contains("b//dev/null"));
}

#[test]
fn persist_review_session_events_skips_malformed_findings() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_session(&pool, "review-malformed", None, "review").await;

        persist_review_session_events(
            &pool,
            "review-malformed",
            concat!(
                r#"<review_verdict>{"passed":true,"needs_human":false,"blocking_issue_count":0,"summary":"通过"}</review_verdict>"#,
                "\n<review_report>## 结论\n通过</review_report>\n",
                "<review_findings>not-json</review_findings>",
            ),
        )
        .await
        .expect("persist review events");

        let findings_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM codex_session_events WHERE session_id = $1 AND event_type = 'review_findings'",
        )
        .bind("review-malformed")
        .fetch_one(&pool)
        .await
        .expect("count findings events");
        let verdict_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM codex_session_events WHERE session_id = $1 AND event_type = 'review_verdict'",
        )
        .bind("review-malformed")
        .fetch_one(&pool)
        .await
        .expect("count verdict events");

        assert_eq!(findings_count, 0);
        assert_eq!(verdict_count, 1);
        pool.close().await;
    });
}

#[test]
fn persist_review_session_events_writes_valid_findings() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_session(&pool, "review-valid", None, "review").await;

        persist_review_session_events(
            &pool,
            "review-valid",
            r#"<review_findings>[{"file":"src/main.rs","line":8,"severity":"warning","message":"未处理 Result"}]</review_findings>"#,
        )
        .await
        .expect("persist findings");

        let message = sqlx::query_scalar::<_, Option<String>>(
            "SELECT message FROM codex_session_events WHERE session_id = $1 AND event_type = 'review_findings' ORDER BY created_at DESC LIMIT 1",
        )
        .bind("review-valid")
        .fetch_optional(&pool)
        .await
        .expect("fetch findings event")
        .flatten()
        .expect("findings event message");

        assert!(message.contains("src/main.rs"));
        pool.close().await;
    });
}

#[test]
fn latest_review_returns_parsed_findings() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-review-findings").await;
        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, title, status, priority, project_id, use_worktree, created_at, updated_at
            ) VALUES (
                'task-review-findings', 'Review findings', 'review', 'medium', 'proj-review-findings', 0,
                '2026-04-16 10:00:00', '2026-04-16 10:00:00'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert task");
        sqlx::query(
            r#"
            INSERT INTO codex_sessions (
                id, task_id, session_kind, status, started_at, created_at
            ) VALUES (
                'review-latest', 'task-review-findings', 'review', 'exited',
                '2026-04-16 10:00:00', '2026-04-16 10:00:00'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert review session");

        persist_review_session_events(
            &pool,
            "review-latest",
            r#"<review_findings>[{"file":"src/app.rs","line":21,"severity":"blocker","message":"逻辑错误"}]</review_findings>"#,
        )
        .await
        .expect("persist findings");

        let latest = fetch_task_latest_review(&pool, "task-review-findings")
            .await
            .expect("fetch latest review")
            .expect("latest review exists");

        assert!(latest.has_findings_event);
        assert_eq!(latest.findings.len(), 1);
        assert_eq!(latest.findings[0].file, "src/app.rs");
        assert_eq!(latest.findings[0].line, Some(21));
        assert_eq!(latest.findings[0].severity, "blocker");
        pool.close().await;
    });
}

#[test]
fn latest_review_treats_malformed_findings_event_as_empty() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-review-bad-findings").await;
        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, title, status, priority, project_id, use_worktree, created_at, updated_at
            ) VALUES (
                'task-review-bad-findings', 'Review findings', 'review', 'medium', 'proj-review-bad-findings', 0,
                '2026-04-16 10:00:00', '2026-04-16 10:00:00'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert task");
        sqlx::query(
            r#"
            INSERT INTO codex_sessions (
                id, task_id, session_kind, status, started_at, created_at
            ) VALUES (
                'review-bad-findings', 'task-review-bad-findings', 'review', 'exited',
                '2026-04-16 10:00:00', '2026-04-16 10:00:00'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert review session");
        sqlx::query(
            r#"
            INSERT INTO codex_session_events (id, session_id, event_type, message, created_at)
            VALUES ('evt-bad-findings', 'review-bad-findings', 'review_findings', 'not-json', '2026-04-16 10:00:01')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert malformed findings event");

        let latest = fetch_task_latest_review(&pool, "task-review-bad-findings")
            .await
            .expect("fetch latest review")
            .expect("latest review exists");

        assert!(!latest.has_findings_event);
        assert!(latest.findings.is_empty());
        pool.close().await;
    });
}

#[test]
fn persist_review_session_events_from_session_logs_extracts_native_stdout() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_session(&pool, "review-native-stdout", None, "review").await;
        sqlx::query(
            r#"
            INSERT INTO codex_session_events (id, session_id, event_type, message, created_at)
            VALUES ($1, 'review-native-stdout', 'stdout', $2, $3)
            "#,
        )
        .bind("evt-prompt")
        .bind("[USER_INPUT] 最终判定必须输出在 <review_verdict> 和 </review_verdict> 之间")
        .bind("2026-08-21 06:00:00")
        .execute(&pool)
        .await
        .expect("insert prompt stdout");
        sqlx::query(
            r#"
            INSERT INTO codex_session_events (id, session_id, event_type, message, created_at)
            VALUES ($1, 'review-native-stdout', 'stdout', $2, $3)
            "#,
        )
        .bind("evt-answer")
        .bind(concat!(
            "我已完成本次只读审查。\n",
            r#"<review_verdict>{"passed":false,"needs_human":true,"blocking_issue_count":1,"summary":"混入无关改动"}</review_verdict>"#,
            "\n<review_report>## 结论\n不通过。</review_report>\n",
            r#"<review_findings>[{"file":"src/a.rs","line":4,"severity":"blocker","message":"阻断"}]</review_findings>"#,
        ))
        .bind("2026-08-21 06:00:01")
        .execute(&pool)
        .await
        .expect("insert review stdout");

        persist_review_session_events_from_session_logs(&pool, "review-native-stdout")
            .await
            .expect("persist from stdout logs");

        let verdict = sqlx::query_scalar::<_, Option<String>>(
            "SELECT message FROM codex_session_events WHERE session_id = $1 AND event_type = 'review_verdict' LIMIT 1",
        )
        .bind("review-native-stdout")
        .fetch_optional(&pool)
        .await
        .expect("fetch verdict")
        .flatten()
        .expect("verdict event");
        let findings_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM codex_session_events WHERE session_id = $1 AND event_type = 'review_findings'",
        )
        .bind("review-native-stdout")
        .fetch_one(&pool)
        .await
        .expect("count findings");

        assert!(verdict.contains("\"passed\":false"));
        assert_eq!(findings_count, 1);
        pool.close().await;
    });
}

#[test]
fn persist_review_session_events_writes_empty_findings_array() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_session(&pool, "review-empty-findings", None, "review").await;

        persist_review_session_events(
            &pool,
            "review-empty-findings",
            "<review_findings>[]</review_findings>",
        )
        .await
        .expect("persist empty findings");

        let findings_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM codex_session_events WHERE session_id = $1 AND event_type = 'review_findings'",
        )
        .bind("review-empty-findings")
        .fetch_one(&pool)
        .await
        .expect("count findings events");

        assert_eq!(findings_count, 1);
        pool.close().await;
    });
}
