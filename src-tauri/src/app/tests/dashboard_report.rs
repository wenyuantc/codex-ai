use super::*;

use crate::app::database::get_dashboard_report_summary_with_pool;
use crate::db::models::GetDashboardReportPayload;

#[test]
fn dashboard_report_empty_scope_has_readable_milestone_empty_state() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        let summary = get_dashboard_report_summary_with_pool(
            &pool,
            &GetDashboardReportPayload {
                environment_mode: Some("local".into()),
                ..Default::default()
            },
        )
        .await
        .expect("report summary");

        assert_eq!(summary.trend_range, "7d");
        assert_eq!(summary.trend_series.len(), 7);
        assert_eq!(summary.weekly_completed.len(), 7);
        assert_eq!(summary.weekly_completed_series.len(), 8);
        assert!(summary.milestones.is_empty());
        assert!(summary.milestone_burndown.is_empty());
        assert!(summary
            .milestone_burndown_empty_reason
            .as_deref()
            .unwrap_or("")
            .contains("暂无里程碑"));
        assert_eq!(summary.token_usage.sessions_with_usage, 0);
        assert_eq!(summary.token_usage.session_count, 0);
        assert_eq!(summary.token_usage_series.len(), 7);
        assert!(summary.token_usage_by_provider.is_empty());
    });
}

#[test]
fn dashboard_report_trend_range_and_milestone_remaining_series() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-report").await;

        sqlx::query(
            r#"
            INSERT INTO milestones (id, project_id, name, due_date, description, created_at, updated_at)
            VALUES ('ms-1', 'proj-report', 'R1', '2026-08-10', NULL, '2026-08-01 10:00:00', '2026-08-01 10:00:00')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert milestone");

        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, title, status, priority, project_id, use_worktree,
                completed_at, milestone_id, created_at, updated_at
            ) VALUES
            (
                't-open', 'Open', 'todo', 'medium', 'proj-report', 0,
                NULL, 'ms-1', '2026-08-01 12:00:00', '2026-08-01 12:00:00'
            ),
            (
                't-done', 'Done', 'completed', 'medium', 'proj-report', 0,
                '2026-08-05 12:00:00', 'ms-1', '2026-08-02 12:00:00', '2026-08-05 12:00:00'
            ),
            (
                't-deleted', 'Gone', 'todo', 'medium', 'proj-report', 0,
                NULL, 'ms-1', '2026-08-01 12:00:00', '2026-08-01 12:00:00'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert tasks");

        sqlx::query("UPDATE tasks SET deleted_at = '2026-08-03 00:00:00' WHERE id = 't-deleted'")
            .execute(&pool)
            .await
            .expect("soft delete");

        let summary_30d = get_dashboard_report_summary_with_pool(
            &pool,
            &GetDashboardReportPayload {
                project_id: Some("proj-report".into()),
                environment_mode: Some("local".into()),
                trend_range: Some("30d".into()),
                milestone_id: Some("ms-1".into()),
                ..Default::default()
            },
        )
        .await
        .expect("30d report");

        assert_eq!(summary_30d.trend_range, "30d");
        assert_eq!(summary_30d.trend_series.len(), 30);
        assert_eq!(summary_30d.selected_milestone_id.as_deref(), Some("ms-1"));
        assert_eq!(summary_30d.milestones.len(), 1);
        assert!(summary_30d.milestone_burndown_empty_reason.is_none());
        assert!(!summary_30d.milestone_burndown.is_empty());
        // Soft-deleted task must not inflate remaining.
        assert!(
            summary_30d
                .milestone_burndown
                .iter()
                .all(|p| p.remaining <= 2),
            "remaining should ignore soft-deleted tasks"
        );

        let summary_8w = get_dashboard_report_summary_with_pool(
            &pool,
            &GetDashboardReportPayload {
                project_id: Some("proj-report".into()),
                environment_mode: Some("local".into()),
                trend_range: Some("8w".into()),
                ..Default::default()
            },
        )
        .await
        .expect("8w report");
        assert_eq!(summary_8w.trend_range, "8w");
        assert_eq!(summary_8w.trend_series.len(), 8);
        // Auto-picks the only milestone when none requested.
        assert_eq!(summary_8w.selected_milestone_id.as_deref(), Some("ms-1"));

        let last = summary_30d
            .milestone_burndown
            .last()
            .expect("burndown points");
        // Due day 2026-08-10: only the still-open task remains (soft-deleted excluded).
        assert_eq!(last.remaining, 1);
    });
}

#[test]
fn dashboard_report_respects_project_and_ssh_scope_for_milestones() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;

        sqlx::query(
            r#"
            INSERT INTO ssh_configs (
                id, name, host, port, username, auth_type, known_hosts_mode, created_at, updated_at
            ) VALUES
            ('ssh-a', 'Host A', 'a.example', 22, 'u', 'key', 'accept-new', '2026-08-01 10:00:00', '2026-08-01 10:00:00'),
            ('ssh-b', 'Host B', 'b.example', 22, 'u', 'key', 'accept-new', '2026-08-01 10:00:00', '2026-08-01 10:00:00')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert ssh configs");

        insert_project(&pool, "proj-local").await;
        sqlx::query(
            r#"
            INSERT INTO projects (
                id, name, description, status, repo_path, project_type, ssh_config_id,
                created_at, updated_at
            ) VALUES
            (
                'proj-ssh-a', 'SSH A', NULL, 'active', NULL, 'ssh', 'ssh-a',
                '2026-08-01 10:00:00', '2026-08-01 10:00:00'
            ),
            (
                'proj-ssh-b', 'SSH B', NULL, 'active', NULL, 'ssh', 'ssh-b',
                '2026-08-01 10:00:00', '2026-08-01 10:00:00'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert ssh projects");

        sqlx::query(
            r#"
            INSERT INTO milestones (id, project_id, name, due_date, description, created_at, updated_at)
            VALUES
            ('ms-local', 'proj-local', 'Local MS', '2026-08-20', NULL, '2026-08-01 10:00:00', '2026-08-01 10:00:00'),
            ('ms-ssh-a', 'proj-ssh-a', 'SSH A MS', '2026-08-20', NULL, '2026-08-01 10:00:00', '2026-08-01 10:00:00'),
            ('ms-ssh-b', 'proj-ssh-b', 'SSH B MS', '2026-08-20', NULL, '2026-08-01 10:00:00', '2026-08-01 10:00:00')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert milestones");

        let local = get_dashboard_report_summary_with_pool(
            &pool,
            &GetDashboardReportPayload {
                environment_mode: Some("local".into()),
                ..Default::default()
            },
        )
        .await
        .expect("local report");
        let local_ids: Vec<_> = local.milestones.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(local_ids, vec!["ms-local"]);
        assert_eq!(local.selected_milestone_id.as_deref(), Some("ms-local"));

        let ssh_a = get_dashboard_report_summary_with_pool(
            &pool,
            &GetDashboardReportPayload {
                environment_mode: Some("ssh".into()),
                selected_ssh_config_id: Some("ssh-a".into()),
                ..Default::default()
            },
        )
        .await
        .expect("ssh-a report");
        let ssh_a_ids: Vec<_> = ssh_a.milestones.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ssh_a_ids, vec!["ms-ssh-a"]);
        assert_eq!(ssh_a.selected_milestone_id.as_deref(), Some("ms-ssh-a"));

        let ssh_b_empty_host = get_dashboard_report_summary_with_pool(
            &pool,
            &GetDashboardReportPayload {
                environment_mode: Some("ssh".into()),
                selected_ssh_config_id: None,
                ..Default::default()
            },
        )
        .await
        .expect("ssh without host");
        assert!(ssh_b_empty_host.milestones.is_empty());
        assert!(ssh_b_empty_host
            .milestone_burndown_empty_reason
            .as_deref()
            .unwrap_or("")
            .contains("暂无里程碑"));
        assert_eq!(ssh_b_empty_host.token_usage.sessions_with_usage, 0);
        assert!(ssh_b_empty_host.token_usage_by_provider.is_empty());
    });
}

#[test]
fn dashboard_report_token_usage_groups_by_provider_and_omits_unknown_sessions() {
    tauri::async_runtime::block_on(async {
        let pool = setup_test_pool().await;
        insert_project(&pool, "proj-token").await;
        insert_project(&pool, "proj-other").await;

        sqlx::query(
            r#"
            INSERT INTO codex_sessions (
                id, project_id, session_kind, status, ai_provider,
                input_tokens, output_tokens, total_tokens, reasoning_tokens,
                started_at, created_at
            ) VALUES
            (
                'sess-codex', 'proj-token', 'execution', 'exited', 'codex',
                80, 20, 100, NULL,
                datetime('now'), datetime('now')
            ),
            (
                'sess-claude', 'proj-token', 'execution', 'exited', 'claude',
                10, 5, 15, 2,
                datetime('now'), datetime('now')
            ),
            (
                'sess-unknown', 'proj-token', 'execution', 'exited', 'grok',
                NULL, NULL, NULL, NULL,
                datetime('now'), datetime('now')
            ),
            (
                'sess-other-project', 'proj-other', 'execution', 'exited', 'codex',
                999, 1, 1000, NULL,
                datetime('now'), datetime('now')
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert token sessions");

        let summary = get_dashboard_report_summary_with_pool(
            &pool,
            &GetDashboardReportPayload {
                project_id: Some("proj-token".into()),
                environment_mode: Some("local".into()),
                trend_range: Some("7d".into()),
                ..Default::default()
            },
        )
        .await
        .expect("token report");

        assert_eq!(summary.token_usage.input_tokens, 90);
        assert_eq!(summary.token_usage.output_tokens, 25);
        assert_eq!(summary.token_usage.total_tokens, 115);
        assert_eq!(summary.token_usage.reasoning_tokens, 2);
        assert_eq!(summary.token_usage.sessions_with_usage, 2);
        assert_eq!(summary.token_usage.session_count, 3);
        assert_eq!(summary.token_usage_series.len(), 7);
        assert_eq!(
            summary.token_usage_series.last().map(|p| p.count),
            Some(115)
        );

        let providers: Vec<_> = summary
            .token_usage_by_provider
            .iter()
            .map(|item| {
                (
                    item.provider.as_str(),
                    item.total_tokens,
                    item.sessions_with_usage,
                )
            })
            .collect();
        assert_eq!(providers, vec![("codex", 100, 1), ("claude", 15, 1)]);

        pool.close().await;
    });
}
