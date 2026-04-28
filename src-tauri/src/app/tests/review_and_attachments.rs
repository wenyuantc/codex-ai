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
        created_at: "2026-04-16 10:00:00".to_string(),
        updated_at: "2026-04-16 10:00:00".to_string(),
    };

    let prompt =
        build_task_review_prompt(&task, &project, "/srv/demo", "## Git 状态\n M src/main.rs");

    assert!(prompt.contains("仓库路径：/srv/demo"));
    assert!(prompt.contains("执行目标：SSH 远程工作区"));
    assert!(!prompt.contains("仓库路径：（未配置）"));
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
        created_at: "2026-04-16 10:00:00".to_string(),
        updated_at: "2026-04-16 10:00:00".to_string(),
    };

    let prompt =
        build_task_review_prompt(&task, &project, "/tmp/demo", "## Git 状态\n M src/main.rs");

    assert!(prompt.contains("执行目标：本地工作区"));
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
