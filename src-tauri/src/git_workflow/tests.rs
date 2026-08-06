// git_workflow tests

use super::*;

use std::path::PathBuf;

use sqlx::SqlitePool;

use crate::app::build_current_migrator;

    async fn setup_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");
        let migrator = build_current_migrator();
        let mut connection: sqlx::pool::PoolConnection<sqlx::Sqlite> =
            pool.acquire().await.expect("acquire sqlite connection");
        migrator
            .run_direct(&mut *connection)
            .await
            .expect("run migrations");
        drop(connection);
        pool
    }

    fn default_test_git_preferences() -> GitPreferences {
        GitPreferences {
            default_task_use_worktree: false,
            worktree_location_mode: "repo_sibling_hidden".to_string(),
            worktree_custom_root: None,
            ai_commit_message_length: "title_with_body".to_string(),
            ai_commit_preferred_provider: "codex".to_string(),
            ai_commit_model_source: "inherit_one_shot".to_string(),
            ai_commit_model: "gpt-5.4".to_string(),
            ai_commit_reasoning_effort: "high".to_string(),
        }
    }

    fn build_task_worktree_path_for_local_test(
        repo_path: &str,
        task_id: &str,
        git_preferences: &GitPreferences,
    ) -> Result<String, String> {
        if git_preferences.worktree_location_mode != "repo_child_hidden" {
            return build_worktree_path(repo_path, task_id, git_preferences);
        }

        let task_slug = sanitize_git_fragment(task_id);
        let git_common_dir_path = resolve_repo_child_worktree_root_local(repo_path)?;
        let path = build_repo_child_worktree_path(&git_common_dir_path, &task_slug)?;
        Ok(path.to_string_lossy().to_string())
    }

    async fn insert_project_and_task(pool: &SqlitePool, repo_path: &str) -> (Project, Task) {
        let project = Project {
            id: "proj-1".to_string(),
            name: "Demo".to_string(),
            description: None,
            status: "active".to_string(),
            repo_path: Some(repo_path.to_string()),
            project_type: EXECUTION_TARGET_LOCAL.to_string(),
            ssh_config_id: None,
            remote_repo_path: None,
            test_command: None,
            deleted_at: None,
            created_at: now_sqlite(),
            updated_at: now_sqlite(),
        };
        sqlx::query(
            r#"INSERT INTO projects (id, name, description, status, repo_path, project_type, ssh_config_id, remote_repo_path, created_at, updated_at)
               VALUES ($1, $2, NULL, $3, $4, $5, NULL, NULL, $6, $7)"#,
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.status)
        .bind(project.repo_path.as_deref())
        .bind(&project.project_type)
        .bind(&project.created_at)
        .bind(&project.updated_at)
        .execute(pool)
        .await
        .expect("insert project");

        let task = Task {
            id: "task-1".to_string(),
            title: "Prepare".to_string(),
            description: Some("demo".to_string()),
            status: "todo".to_string(),
            priority: "high".to_string(),
            project_id: project.id.clone(),
            use_worktree: true,
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
            created_at: now_sqlite(),
            updated_at: now_sqlite(),
        };
        sqlx::query(
            r#"INSERT INTO tasks (id, title, description, status, priority, project_id, use_worktree, assignee_id, reviewer_id, complexity, ai_suggestion, automation_mode, last_codex_session_id, last_review_session_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, NULL, NULL, NULL, NULL, NULL, NULL, $8, $9)"#,
        )
        .bind(&task.id)
        .bind(&task.title)
        .bind(task.description.as_deref())
        .bind(&task.status)
        .bind(&task.priority)
        .bind(&task.project_id)
        .bind(task.use_worktree)
        .bind(&task.created_at)
        .bind(&task.updated_at)
        .execute(pool)
        .await
        .expect("insert task");

        (project, task)
    }

    fn build_local_preview_project(repo_path: &str) -> Project {
        Project {
            id: "proj-preview".to_string(),
            name: "Preview".to_string(),
            description: None,
            status: "active".to_string(),
            repo_path: Some(repo_path.to_string()),
            project_type: EXECUTION_TARGET_LOCAL.to_string(),
            ssh_config_id: None,
            remote_repo_path: None,
            test_command: None,
            deleted_at: None,
            created_at: now_sqlite(),
            updated_at: now_sqlite(),
        }
    }

    fn capture_revision_text_snapshot_local(
        repo_path: &str,
        revision: &str,
        relative_path: &str,
    ) -> git_runtime::GitRuntimeTextSnapshot {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["show", &format!("{revision}:{relative_path}")])
            .output();
        match output {
            Ok(output) if output.status.success() => {
                if output.stdout.contains(&0) {
                    git_runtime::GitRuntimeTextSnapshot {
                        status: "binary".to_string(),
                        text: None,
                        truncated: false,
                    }
                } else {
                    git_runtime::GitRuntimeTextSnapshot {
                        status: "text".to_string(),
                        text: Some(String::from_utf8_lossy(&output.stdout).to_string()),
                        truncated: output.stdout.len() > 256 * 1024,
                    }
                }
            }
            _ => missing_project_git_text_snapshot(),
        }
    }

    fn build_project_git_commit_file_preview_local(
        project: &Project,
        commit_sha: &str,
        relative_path: &str,
        previous_path: Option<String>,
        change_type: Option<String>,
    ) -> Result<ProjectGitFilePreview, String> {
        let runtime = resolve_project_runtime_context(project)?;
        let trimmed_commit_sha = commit_sha.trim().to_string();
        if trimmed_commit_sha.is_empty() {
            return Err("提交 SHA 不能为空".to_string());
        }

        let trimmed_path = relative_path.trim();
        if trimmed_path.is_empty() {
            return Err("文件路径不能为空".to_string());
        }

        let normalized_previous_path = normalize_project_git_relative_path(previous_path);
        let normalized_change_type = normalize_project_git_change_type(change_type);
        let parent_revision = format!("{trimmed_commit_sha}^1");
        let before_path = if normalized_change_type == "renamed" {
            normalized_previous_path.as_deref().unwrap_or(trimmed_path)
        } else {
            trimmed_path
        };
        let before_snapshot = if normalized_change_type == "added" {
            missing_project_git_text_snapshot()
        } else {
            capture_revision_text_snapshot_local(&runtime.repo_path, &parent_revision, before_path)
        };
        let after_snapshot = if normalized_change_type == "deleted" {
            missing_project_git_text_snapshot()
        } else {
            capture_revision_text_snapshot_local(
                &runtime.repo_path,
                &trimmed_commit_sha,
                trimmed_path,
            )
        };
        let (before_label, after_label) =
            build_project_git_commit_preview_labels(&trimmed_commit_sha);

        Ok(build_project_git_file_preview(
            project.id.clone(),
            runtime.execution_target,
            &runtime.repo_path,
            trimmed_path,
            normalized_previous_path,
            normalized_change_type,
            before_label,
            after_label,
            before_snapshot,
            after_snapshot,
        ))
    }

    fn init_git_repo() -> String {
        let repo_root = std::env::temp_dir().join(format!(
            "codex-git-workflow-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        fs::create_dir_all(repo_root.join("src")).expect("create repo dir");
        fs::write(repo_root.join("src/main.ts"), "console.log('hello');\n").expect("write file");
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(&repo_root)
                .args(args)
                .status()
                .expect("run git");
            assert!(status.success(), "git {:?} should succeed", args);
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "codex@example.com"]);
        run(&["config", "user.name", "Codex"]);
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        repo_root.to_string_lossy().to_string()
    }

    async fn update_context_after_prepare_for_test(
        pool: &SqlitePool,
        task: &Task,
        project: &Project,
        preferred_target_branch: Option<String>,
    ) -> Result<TaskGitContextRecord, String> {
        let repo_path = project
            .repo_path
            .clone()
            .ok_or_else(|| "当前项目未配置本地仓库目录".to_string())?;
        ensure_git_repository(&repo_path)?;
        let target_branch = match preferred_target_branch {
            Some(branch) => branch,
            None => determine_current_branch_local(&repo_path)?,
        };
        let task_branch = build_task_branch(&task.id);
        let worktree_path = build_task_worktree_path_for_local_test(
            &repo_path,
            &task.id,
            &default_test_git_preferences(),
        )?;
        let head_commit = run_git_text(&repo_path, &["rev-parse", &target_branch])?;

        if let Some(existing) = fetch_task_git_context_by_task_id(pool, &task.id).await? {
            if existing.target_branch != target_branch {
                return Err(format!(
                    "当前任务已绑定目标分支 {}，不能切换到 {}",
                    existing.target_branch, target_branch
                ));
            }
            if git_ref_exists_local(&repo_path, &format!("refs/heads/{}", existing.task_branch))
                && Path::new(&existing.worktree_path).join(".git").exists()
            {
                if matches!(
                    existing.state.as_str(),
                    TASK_GIT_STATE_FAILED | TASK_GIT_STATE_DRIFTED
                ) {
                    return mark_task_git_context_reconciled_after_prepare(
                        pool,
                        existing,
                        head_commit,
                        "任务 Git 上下文已恢复可用",
                    )
                    .await;
                }
                return Ok(existing);
            }

            let full_ref = format!("refs/heads/{}", existing.task_branch);
            if !git_ref_exists_local(&repo_path, &full_ref) {
                run_git_command(
                    &repo_path,
                    &["branch", &existing.task_branch, &existing.target_branch],
                )?;
            }
            let worktree = Path::new(&existing.worktree_path);
            if !worktree.join(".git").exists() {
                if worktree.exists() {
                    let is_empty = fs::read_dir(worktree)
                        .map_err(|error| format!("读取 worktree 目录失败: {}", error))?
                        .next()
                        .is_none();
                    if !is_empty {
                        return Err(format!(
                            "worktree 目录已存在且非空：{}",
                            existing.worktree_path
                        ));
                    }
                } else if let Some(parent) = worktree.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("创建 worktree 父目录失败: {}", error))?;
                }
                run_git_command(
                    &repo_path,
                    &[
                        "worktree",
                        "add",
                        &existing.worktree_path,
                        &existing.task_branch,
                    ],
                )?;
            }
            return mark_task_git_context_reconciled_after_prepare(
                pool,
                existing,
                head_commit,
                "任务 Git 上下文已恢复可用",
            )
            .await;
        }

        let full_ref = format!("refs/heads/{task_branch}");
        if !git_ref_exists_local(&repo_path, &full_ref) {
            run_git_command(&repo_path, &["branch", &task_branch, &target_branch])?;
        }
        let worktree = Path::new(&worktree_path);
        if !worktree.join(".git").exists() {
            if worktree.exists() {
                let is_empty = fs::read_dir(worktree)
                    .map_err(|error| format!("读取 worktree 目录失败: {}", error))?
                    .next()
                    .is_none();
                if !is_empty {
                    return Err(format!("worktree 目录已存在且非空：{}", worktree_path));
                }
            } else if let Some(parent) = worktree.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("创建 worktree 父目录失败: {}", error))?;
            }
            run_git_command(
                &repo_path,
                &["worktree", "add", &worktree_path, &task_branch],
            )?;
        }

        let now = now_sqlite();
        let record = TaskGitContextRecord {
            id: Uuid::new_v4().to_string(),
            task_id: task.id.clone(),
            project_id: project.id.clone(),
            base_branch: target_branch.clone(),
            task_branch,
            target_branch,
            worktree_path,
            repo_head_commit_at_prepare: Some(head_commit),
            state: TASK_GIT_STATE_READY.to_string(),
            context_version: 1,
            pending_action_type: None,
            pending_action_token_hash: None,
            pending_action_payload_json: None,
            pending_action_nonce: None,
            pending_action_requested_at: None,
            pending_action_expires_at: None,
            pending_action_repo_revision: None,
            pending_action_bound_context_version: None,
            last_reconciled_at: None,
            last_error: None,
            created_at: now.clone(),
            updated_at: now,
        };
        insert_task_git_context(pool, &record).await
    }

    #[test]
    fn list_local_branches_returns_local_heads() {
        let repo_path = init_git_repo();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .args(args)
                .status()
                .expect("run git");
            assert!(status.success(), "git {:?} should succeed", args);
        };
        run(&["branch", "release/1.0"]);
        run(&["branch", "feature/git-panel"]);

        let output = run_git_text(
            &repo_path,
            &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
        )
        .expect("list local branches");
        let branches = output
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();

        assert_eq!(
            branches,
            vec![
                "feature/git-panel".to_string(),
                "main".to_string(),
                "release/1.0".to_string(),
            ]
        );

        let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
    }

    #[test]
    fn build_worktree_path_supports_all_configured_modes() {
        let repo_path = "/tmp/demo-repo";
        let task_id = "task/42";

        let sibling_path = build_worktree_path(repo_path, task_id, &default_test_git_preferences())
            .expect("build sibling path");
        assert_eq!(sibling_path, "/tmp/.codex-ai-worktrees-demo-repo/task-42");

        let child_path = build_worktree_path(
            repo_path,
            task_id,
            &GitPreferences {
                worktree_location_mode: "repo_child_hidden".to_string(),
                ..default_test_git_preferences()
            },
        )
        .expect("build child path");
        assert_eq!(child_path, "/tmp/demo-repo/.git/codex-ai-worktrees/task-42");

        let custom_path = build_worktree_path(
            repo_path,
            task_id,
            &GitPreferences {
                worktree_location_mode: "custom_root".to_string(),
                worktree_custom_root: Some("/worktrees".to_string()),
                ..default_test_git_preferences()
            },
        )
        .expect("build custom path");
        assert_eq!(custom_path, "/worktrees/demo-repo/task-42");
    }

    #[test]
    fn resolve_repo_child_worktree_root_uses_common_git_dir_for_linked_worktree() {
        let repo_path = init_git_repo();
        let linked_worktree = PathBuf::from(&repo_path)
            .parent()
            .expect("repo parent")
            .join("linked-worktree");
        run_git_command(
            &repo_path,
            &[
                "worktree",
                "add",
                "-b",
                "feature/linked",
                linked_worktree.to_string_lossy().as_ref(),
                "main",
            ],
        )
        .expect("create linked worktree");

        let resolved =
            resolve_repo_child_worktree_root_local(linked_worktree.to_string_lossy().as_ref())
                .expect("resolve linked worktree common git dir");
        assert!(
            resolved.ends_with("/.git"),
            "unexpected common git dir: {resolved}"
        );

        let child_path = build_repo_child_worktree_path(&resolved, "task-42")
            .expect("build repo child worktree path");
        assert!(
            child_path
                .to_string_lossy()
                .ends_with("/.git/codex-ai-worktrees/task-42"),
            "unexpected child path: {}",
            child_path.to_string_lossy()
        );

        let _ = fs::remove_dir_all(linked_worktree);
        let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
    }

    #[test]
    fn parse_worktree_list_porcelain_handles_flags_and_reasons() {
        let parsed = parse_worktree_list_porcelain(
            r#"worktree /tmp/demo
HEAD 0123456789abcdef
branch refs/heads/main

worktree /tmp/demo-feature
HEAD fedcba9876543210
branch refs/heads/feature/demo
locked manual keep

worktree /tmp/demo-stale
HEAD 1111111111111111
detached
prunable gitdir file points to non-existent location
"#,
        )
        .expect("parse worktree list");

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].path, "/tmp/demo");
        assert_eq!(parsed[0].branch_name().as_deref(), Some("main"));
        assert!(!parsed[0].is_locked);
        assert!(!parsed[0].is_prunable);

        assert_eq!(parsed[1].branch_name().as_deref(), Some("feature/demo"));
        assert!(parsed[1].is_locked);
        assert_eq!(parsed[1].lock_reason.as_deref(), Some("manual keep"));

        assert!(parsed[2].is_detached);
        assert!(parsed[2].is_prunable);
        assert_eq!(
            parsed[2].prunable_reason.as_deref(),
            Some("gitdir file points to non-existent location")
        );
    }

    #[test]
    fn parse_worktree_list_porcelain_reads_real_git_output() {
        let repo_path = init_git_repo();
        let linked_worktree = PathBuf::from(&repo_path)
            .parent()
            .expect("repo parent")
            .join(format!("linked-worktree-parse-{}", Uuid::new_v4()));
        run_git_command(
            &repo_path,
            &[
                "worktree",
                "add",
                "-b",
                "feature/parse",
                linked_worktree.to_string_lossy().as_ref(),
                "main",
            ],
        )
        .expect("create linked worktree");

        let output =
            run_git_text(&repo_path, &["worktree", "list", "--porcelain"]).expect("list worktrees");
        let parsed = parse_worktree_list_porcelain(&output).expect("parse real worktree output");
        let linked_worktree_canonical = linked_worktree
            .canonicalize()
            .expect("canonical linked worktree path");

        assert_eq!(parsed.len(), 2);
        assert!(parsed
            .iter()
            .any(|entry| entry.branch_name().as_deref() == Some("main")));
        assert!(parsed.iter().any(|entry| {
            entry.branch_name().as_deref() == Some("feature/parse")
                && PathBuf::from(&entry.path)
                    .canonicalize()
                    .map(|path| path == linked_worktree_canonical)
                    .unwrap_or(false)
        }));

        let _ = fs::remove_dir_all(linked_worktree);
        let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
    }

    #[test]
    fn cleanup_worktree_action_is_allowed_after_completion() {
        assert!(action_allows_completed_context("cleanup_worktree"));
        assert!(!action_allows_completed_context("merge"));
        assert!(!action_allows_completed_context("push"));
    }

    #[test]
    fn cleanup_worktree_force_remove_is_allowed_after_failure() {
        assert!(action_allows_failed_context(
            "cleanup_worktree",
            &serde_json::json!({ "force_remove": true }),
        )
        .expect("force cleanup payload should parse"));
        assert!(
            action_allows_failed_context("cleanup_worktree", &serde_json::json!({}))
                .expect("default cleanup payload should parse")
        );
        assert!(!action_allows_failed_context(
            "cleanup_worktree",
            &serde_json::json!({ "force_remove": false }),
        )
        .expect("non-force cleanup payload should parse"));
        assert!(!action_allows_failed_context(
            "merge",
            &serde_json::json!({ "force_remove": true })
        )
        .expect("non-cleanup action should not inspect payload"));
    }

    #[test]
    fn prepare_task_git_execution_is_idempotent() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;

            let first = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("first prepare");
            let second = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("second prepare");

            assert_eq!(first.id, second.id);
            assert_eq!(first.worktree_path, second.worktree_path);
            assert!(Path::new(&first.worktree_path).join(".git").exists());

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&first.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn prepare_task_git_execution_recovers_failed_healthy_context() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;

            let first = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("first prepare");
            let mut failed = fetch_task_git_context_by_id(&pool, &first.id)
                .await
                .expect("fetch context");
            failed.state = TASK_GIT_STATE_FAILED.to_string();
            failed.last_error = Some("previous launch failed".to_string());
            failed.context_version += 1;
            let failed = save_task_git_context(&pool, &failed)
                .await
                .expect("save failed context");

            let recovered = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("recover failed context");

            assert_eq!(recovered.id, first.id);
            assert_eq!(recovered.state, TASK_GIT_STATE_READY);
            assert_eq!(recovered.last_error, None);
            assert!(recovered.context_version > failed.context_version);
            assert!(Path::new(&recovered.worktree_path).join(".git").exists());

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&recovered.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn prepare_task_git_execution_prefers_current_branch_over_origin_head() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let repo_root = PathBuf::from(&repo_path);
            let remote_root = std::env::temp_dir().join(format!(
                "codex-git-workflow-origin-{}-{}",
                std::process::id(),
                Uuid::new_v4()
            ));
            fs::create_dir_all(&remote_root).expect("create remote dir");
            let remote_root_str = remote_root.to_string_lossy().to_string();
            let run = |args: &[&str]| {
                let status = Command::new("git")
                    .arg("-C")
                    .arg(&repo_path)
                    .args(args)
                    .status()
                    .expect("run git");
                assert!(status.success(), "git {:?} should succeed", args);
            };

            run(&["init", "--bare", "-q", &remote_root_str]);
            run(&["remote", "add", "origin", &remote_root_str]);
            run(&["push", "-u", "origin", "main"]);
            run(&["remote", "set-head", "origin", "main"]);
            run(&["checkout", "-q", "-b", "feature/current-base"]);

            fs::write(repo_root.join("FEATURE.md"), "feature base\n").expect("write feature file");
            run(&["add", "FEATURE.md"]);
            run(&["commit", "-q", "-m", "feature base"]);

            let main_head = run_git_text(&repo_path, &["rev-parse", "main"]).expect("main head");
            let feature_head =
                run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("feature head");
            assert_ne!(main_head, feature_head);

            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");
            let worktree_head = run_git_text(&context.worktree_path, &["rev-parse", "HEAD"])
                .expect("worktree head");

            assert_eq!(context.target_branch, "feature/current-base");
            assert_eq!(context.base_branch, "feature/current-base");
            assert_eq!(
                context.repo_head_commit_at_prepare.as_deref(),
                Some(feature_head.as_str())
            );
            assert_eq!(worktree_head, feature_head);

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            let _ = fs::remove_dir_all(&remote_root);
            pool.close().await;
        });
    }

    #[test]
    fn pending_merge_requires_task_branch_to_be_ahead() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let repo_root = PathBuf::from(&repo_path);
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");

            assert!(
                !task_git_context_has_pending_merge_local(&context)
                    .expect("initial pending merge state"),
                "fresh task branch should not require merge action"
            );

            fs::write(repo_root.join("README.md"), "# demo repo\n").expect("write repo update");
            run_git_command(&repo_path, &["add", "README.md"]).expect("stage repo update");
            run_git_command(&repo_path, &["commit", "-q", "-m", "repo update"])
                .expect("commit repo update");

            assert!(
                !task_git_context_has_pending_merge_local(&context)
                    .expect("target-ahead pending merge state"),
                "target branch moving ahead alone should not mark task branch merge-ready"
            );

            fs::write(
                PathBuf::from(&context.worktree_path).join("src/main.ts"),
                "console.log('task change');\n",
            )
            .expect("write task branch update");
            run_git_command(&context.worktree_path, &["add", "src/main.ts"])
                .expect("stage task branch update");
            run_git_command(
                &context.worktree_path,
                &["commit", "-q", "-m", "task branch update"],
            )
            .expect("commit task branch update");

            assert!(
                task_git_context_has_pending_merge_local(&context)
                    .expect("task-ahead pending merge state"),
                "task branch commits should still be recognized as pending merge work"
            );

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn pending_merge_uses_worktree_head_when_detached() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");
            let initial_task_branch_head =
                run_git_text(&repo_path, &["rev-parse", &context.task_branch]).expect("task ref");

            run_git_command(&context.worktree_path, &["checkout", "--detach"])
                .expect("detach worktree head");
            fs::write(
                PathBuf::from(&context.worktree_path).join("src/main.ts"),
                "console.log('detached head change');\n",
            )
            .expect("write detached head update");
            run_git_command(&context.worktree_path, &["add", "src/main.ts"])
                .expect("stage detached head update");
            run_git_command(
                &context.worktree_path,
                &["commit", "-q", "-m", "detached head update"],
            )
            .expect("commit detached head update");

            let detached_head =
                run_git_text(&context.worktree_path, &["rev-parse", "HEAD"]).expect("head");
            let task_branch_head =
                run_git_text(&repo_path, &["rev-parse", &context.task_branch]).expect("task ref");

            assert_ne!(
                detached_head, task_branch_head,
                "detached HEAD commit should not advance the tracked task branch ref"
            );
            assert_eq!(
                task_branch_head, initial_task_branch_head,
                "task branch ref should stay unchanged when committing from detached HEAD"
            );
            assert!(
                task_git_context_has_pending_merge_local(&context)
                    .expect("detached head pending merge state"),
                "worktree HEAD commits should still be recognized as pending merge work"
            );

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn request_and_cancel_git_action_invalidate_token() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");
            let mut stored = fetch_task_git_context_by_id(&pool, &context.id)
                .await
                .expect("fetch context");
            let input = RequestGitActionInput {
                task_git_context_id: context.id.clone(),
                action_type: "stash".to_string(),
                payload: serde_json::json!({ "include_untracked": true }),
            };
            let normalized_payload = normalize_git_action_payload("stash", &stored, &input.payload)
                .expect("normalize payload");
            let expires_at = sqlite_now_with_offset(PENDING_ACTION_TTL_MINUTES);
            let nonce = Uuid::new_v4().to_string();
            let signature = build_pending_action_signature(
                &stored.id,
                "stash",
                &normalized_payload,
                &nonce,
                &expires_at,
                stored.context_version + 1,
            );
            stored.state = TASK_GIT_STATE_ACTION_PENDING.to_string();
            stored.context_version += 1;
            stored.pending_action_type = Some("stash".to_string());
            stored.pending_action_token_hash = Some(signature.clone());
            stored.pending_action_payload_json = Some(normalized_payload);
            stored.pending_action_nonce = Some(nonce.clone());
            stored.pending_action_requested_at = Some(now_sqlite());
            stored.pending_action_expires_at = Some(expires_at);
            stored.pending_action_repo_revision =
                Some(run_git_text(&stored.worktree_path, &["rev-parse", "HEAD"]).expect("head"));
            stored.pending_action_bound_context_version = Some(stored.context_version);
            stored.updated_at = now_sqlite();
            let saved = save_task_git_context(&pool, &stored)
                .await
                .expect("save pending action");

            let cancelled = {
                let token = format!("{}.{}", nonce, signature);
                let mut context = fetch_task_git_context_by_id(&pool, &saved.id)
                    .await
                    .expect("fetch saved");
                let (parsed_nonce, parsed_signature) = parse_token(&token).expect("parse token");
                assert_eq!(parsed_nonce, nonce);
                assert_eq!(parsed_signature, signature);
                clear_pending_action_fields(&mut context);
                context.context_version += 1;
                context.state = TASK_GIT_STATE_MERGE_READY.to_string();
                context.updated_at = now_sqlite();
                save_task_git_context(&pool, &context)
                    .await
                    .expect("cancel save")
            };

            assert_eq!(cancelled.pending_action_type, None);
            assert_eq!(cancelled.state, TASK_GIT_STATE_MERGE_READY);

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn merge_action_merges_task_branch_into_target_branch() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let repo_root = PathBuf::from(&repo_path);
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");

            fs::write(repo_root.join("README.md"), "# demo repo\n").expect("write repo file");
            run_git_command(&repo_path, &["add", "README.md"]).expect("stage repo file");
            run_git_command(&repo_path, &["commit", "-q", "-m", "repo baseline"])
                .expect("commit repo baseline");
            run_git_command(&context.worktree_path, &["rebase", "main"]).expect("sync worktree");

            fs::write(
                PathBuf::from(&context.worktree_path).join("src/main.ts"),
                "console.log('merged from task branch');\n",
            )
            .expect("write worktree file");
            run_git_command(&context.worktree_path, &["add", "src/main.ts"])
                .expect("stage worktree change");
            run_git_command(
                &context.worktree_path,
                &["commit", "-q", "-m", "task change"],
            )
            .expect("commit worktree change");

            let main_before =
                run_git_text(&repo_path, &["rev-parse", "main"]).expect("main before");
            let task_head =
                run_git_text(&context.worktree_path, &["rev-parse", "HEAD"]).expect("task head");

            let message =
                merge_task_branch_into_target_local(&repo_path, &context, "main", "ort", true)
                    .expect("merge task branch");

            let main_after = run_git_text(&repo_path, &["rev-parse", "main"]).expect("main after");
            let current_branch =
                run_git_text(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"]).expect("branch");
            let merged_source =
                fs::read_to_string(repo_root.join("src/main.ts")).expect("read merged file");

            assert_eq!(
                message,
                format!("已将任务分支 {} 合并到目标分支 main", context.task_branch)
            );
            assert_ne!(main_before, main_after);
            assert_eq!(main_after, task_head);
            assert_eq!(current_branch, "main");
            assert!(merged_source.contains("merged from task branch"));

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn project_git_commit_file_preview_covers_root_modified_added_renamed_and_deleted() {
        let repo_path = init_git_repo();
        let repo_root = PathBuf::from(&repo_path);
        let project = build_local_preview_project(&repo_path);
        let root_commit =
            run_git_text(&repo_path, &["rev-list", "--max-parents=0", "HEAD"]).expect("root sha");

        fs::create_dir_all(repo_root.join("docs")).expect("create docs dir");
        fs::write(repo_root.join("docs/guide.txt"), "base guide\n").expect("write base guide");
        run_git_command(&repo_path, &["add", "docs/guide.txt"]).expect("stage base guide");
        run_git_command(&repo_path, &["commit", "-q", "-m", "add guide"]).expect("commit guide");

        fs::write(repo_root.join("docs/guide.txt"), "modified guide\n")
            .expect("write modified guide");
        run_git_command(&repo_path, &["add", "docs/guide.txt"]).expect("stage modified guide");
        run_git_command(&repo_path, &["commit", "-q", "-m", "modify guide"])
            .expect("commit modified guide");
        let modified_commit =
            run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("modified sha");

        fs::write(
            repo_root.join("src/feature.ts"),
            "export const feature = true;\n",
        )
        .expect("write added file");
        run_git_command(&repo_path, &["add", "src/feature.ts"]).expect("stage added file");
        run_git_command(&repo_path, &["commit", "-q", "-m", "add feature"])
            .expect("commit added file");
        let added_commit = run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("added sha");

        run_git_command(
            &repo_path,
            &["mv", "docs/guide.txt", "docs/guide-renamed.txt"],
        )
        .expect("rename guide");
        run_git_command(&repo_path, &["commit", "-q", "-m", "rename guide"])
            .expect("commit renamed file");
        let renamed_commit = run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("renamed sha");

        run_git_command(&repo_path, &["rm", "src/feature.ts"]).expect("delete feature");
        run_git_command(&repo_path, &["commit", "-q", "-m", "delete feature"])
            .expect("commit deleted file");
        let deleted_commit = run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("deleted sha");

        let root_preview = build_project_git_commit_file_preview_local(
            &project,
            &root_commit,
            "src/main.ts",
            None,
            Some("modified".to_string()),
        )
        .expect("root preview");
        assert_eq!(root_preview.before_status, "missing");
        assert_eq!(root_preview.after_status, "text");
        assert_eq!(root_preview.before_label, "父提交");
        assert!(root_preview
            .after_label
            .contains(&root_commit.chars().take(7).collect::<String>()));
        assert_eq!(
            root_preview.after_text.as_deref(),
            Some("console.log('hello');\n")
        );

        let modified_preview = build_project_git_commit_file_preview_local(
            &project,
            &modified_commit,
            "docs/guide.txt",
            None,
            Some("modified".to_string()),
        )
        .expect("modified preview");
        assert_eq!(modified_preview.before_status, "text");
        assert_eq!(modified_preview.after_status, "text");
        assert_eq!(
            modified_preview.before_text.as_deref(),
            Some("base guide\n")
        );
        assert_eq!(
            modified_preview.after_text.as_deref(),
            Some("modified guide\n")
        );

        let added_preview = build_project_git_commit_file_preview_local(
            &project,
            &added_commit,
            "src/feature.ts",
            None,
            Some("added".to_string()),
        )
        .expect("added preview");
        assert_eq!(added_preview.before_status, "missing");
        assert_eq!(added_preview.after_status, "text");
        assert_eq!(
            added_preview.after_text.as_deref(),
            Some("export const feature = true;\n")
        );

        let renamed_preview = build_project_git_commit_file_preview_local(
            &project,
            &renamed_commit,
            "docs/guide-renamed.txt",
            Some("docs/guide.txt".to_string()),
            Some("renamed".to_string()),
        )
        .expect("renamed preview");
        let expected_previous_absolute_path = repo_root.join("docs/guide.txt");
        assert_eq!(
            renamed_preview.previous_path.as_deref(),
            Some("docs/guide.txt")
        );
        assert_eq!(
            renamed_preview.previous_absolute_path.as_deref(),
            Some(expected_previous_absolute_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            renamed_preview.before_text.as_deref(),
            Some("modified guide\n")
        );
        assert_eq!(
            renamed_preview.after_text.as_deref(),
            Some("modified guide\n")
        );

        let deleted_preview = build_project_git_commit_file_preview_local(
            &project,
            &deleted_commit,
            "src/feature.ts",
            None,
            Some("deleted".to_string()),
        )
        .expect("deleted preview");
        assert_eq!(deleted_preview.before_status, "text");
        assert_eq!(deleted_preview.after_status, "missing");
        assert_eq!(
            deleted_preview.before_text.as_deref(),
            Some("export const feature = true;\n")
        );

        let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
    }

    #[test]
    fn delete_task_git_context_removes_record() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");

            delete_task_git_context(&pool, &context.id)
                .await
                .expect("delete task git context");

            let deleted = fetch_task_git_context_by_id(&pool, &context.id).await;
            assert!(deleted.is_err(), "task git context should be deleted");

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            pool.close().await;
        });
    }
