// Domain slice: acceptance / tester automation (included into task_automation)

const PHASE_LAUNCHING_TESTER: &str = "launching_tester";
const PHASE_WAITING_TESTER: &str = "waiting_tester";
const PHASE_TESTER_FAILED: &str = "tester_failed";
#[allow(dead_code)]
const PHASE_TESTER_LAUNCH_FAILED: &str = "tester_launch_failed";

const ACCEPTANCE_STATUS_RUNNING: &str = "running";
const ACCEPTANCE_STATUS_PASSED: &str = "passed";
const ACCEPTANCE_STATUS_FAILED: &str = "failed";
const ACCEPTANCE_STATUS_SKIPPED: &str = "skipped";

const ACCEPTANCE_TRIGGER_MANUAL: &str = "manual";
const ACCEPTANCE_TRIGGER_AUTO: &str = "auto";

const COMMAND_OUTPUT_EXCERPT_LIMIT: usize = 8_000;
const TEST_COMMAND_TIMEOUT_SECS: u64 = 900;

#[derive(Clone, Debug)]
struct TesterAutomationPolicy {
    enabled: bool,
    allow_ai_only: bool,
    default_test_command: Option<String>,
}

#[derive(Clone, Debug)]
struct AcceptanceCommandResult {
    command: String,
    exit_code: i32,
    output_excerpt: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AcceptanceDecision {
    Passed { summary: String },
    Failed { summary: String, hard_fail: bool },
    Skipped { summary: String },
}

fn load_tester_automation_policy<R: Runtime>(app: &AppHandle<R>) -> TesterAutomationPolicy {
    let defaults = TesterAutomationPolicy {
        enabled: false,
        allow_ai_only: false,
        default_test_command: None,
    };
    let Ok(settings) = load_codex_settings(app) else {
        return defaults;
    };
    TesterAutomationPolicy {
        enabled: settings.tester_automation_enabled,
        allow_ai_only: settings.tester_allow_ai_only,
        default_test_command: settings
            .default_test_command
            .as_deref()
            .and_then(|value| normalize_optional_text(Some(value))),
    }
}

fn resolve_effective_test_command(
    project: &Project,
    default_test_command: Option<&str>,
) -> Option<String> {
    project
        .test_command
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)))
        .or_else(|| {
            default_test_command.and_then(|value| normalize_optional_text(Some(value)))
        })
}

fn truncate_command_output(stdout: &str, stderr: &str) -> String {
    let mut combined = String::new();
    if !stdout.trim().is_empty() {
        combined.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push_str("\n--- stderr ---\n");
        }
        combined.push_str(stderr.trim_end());
    }
    if combined.chars().count() <= COMMAND_OUTPUT_EXCERPT_LIMIT {
        return combined;
    }
    let truncated: String = combined.chars().take(COMMAND_OUTPUT_EXCERPT_LIMIT).collect();
    format!("{truncated}\n…(输出已截断)")
}

fn decide_acceptance_result(
    command_result: Option<&AcceptanceCommandResult>,
    allow_ai_only: bool,
    checklist: Option<&str>,
    is_manual: bool,
) -> AcceptanceDecision {
    if let Some(result) = command_result {
        if result.exit_code != 0 {
            return AcceptanceDecision::Failed {
                summary: format!(
                    "测试命令失败（exit {}）：{}",
                    result.exit_code, result.command
                ),
                hard_fail: true,
            };
        }
        let checklist_note = if checklist.map(str::trim).filter(|v| !v.is_empty()).is_some() {
            "；已记录验收清单"
        } else {
            ""
        };
        return AcceptanceDecision::Passed {
            summary: format!("测试命令通过：{}{}", result.command, checklist_note),
        };
    }

    if allow_ai_only {
        let has_checklist = checklist.map(str::trim).filter(|v| !v.is_empty()).is_some();
        if has_checklist {
            return AcceptanceDecision::Passed {
                summary: "未配置客观测试命令，已按验收清单主观通过（结果偏主观）".to_string(),
            };
        }
        if is_manual {
            return AcceptanceDecision::Failed {
                summary: "未配置测试命令且验收清单为空，无法完成仅 AI 验收".to_string(),
                hard_fail: false,
            };
        }
        return AcceptanceDecision::Skipped {
            summary: "未配置测试命令且无验收清单，已跳过测试员阶段".to_string(),
        };
    }

    if is_manual {
        AcceptanceDecision::Failed {
            summary: "未配置测试命令，且不允许仅 AI 验收。请在项目或设置中配置测试命令。"
                .to_string(),
            hard_fail: false,
        }
    } else {
        AcceptanceDecision::Skipped {
            summary: "未配置测试命令且不允许仅 AI 验收，已跳过测试员阶段".to_string(),
        }
    }
}

fn shell_escape_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

async fn run_local_test_command(
    working_dir: &str,
    command: &str,
) -> Result<AcceptanceCommandResult, String> {
    use crate::process_spawn::configure_tokio_command;
    use std::time::Duration;
    use tokio::process::Command as TokioCommand;

    let mut child = TokioCommand::new("sh");
    configure_tokio_command(&mut child);
    child
        .arg("-lc")
        .arg(command)
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let output = tokio::time::timeout(Duration::from_secs(TEST_COMMAND_TIMEOUT_SECS), child.output())
        .await
        .map_err(|_| format!("测试命令超时（{} 秒）: {}", TEST_COMMAND_TIMEOUT_SECS, command))?
        .map_err(|error| format!("执行测试命令失败: {}", error))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(AcceptanceCommandResult {
        command: command.to_string(),
        exit_code: output.status.code().unwrap_or(1),
        output_excerpt: truncate_command_output(&stdout, &stderr),
    })
}

async fn run_ssh_test_command<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    project: &Project,
    working_dir: &str,
    command: &str,
) -> Result<AcceptanceCommandResult, String> {
    use crate::app::{build_remote_shell_command, execute_ssh_command, fetch_ssh_config_record_by_id};

    let ssh_config_id = project
        .ssh_config_id
        .as_deref()
        .ok_or_else(|| "SSH 项目缺少 ssh_config_id，无法执行远程测试命令".to_string())?;
    let ssh_config = fetch_ssh_config_record_by_id(pool, ssh_config_id).await?;
    let remote_script = format!(
        "cd {} && {}",
        shell_escape_single_quoted(working_dir),
        command
    );
    let remote_command = build_remote_shell_command(&remote_script, None);
    let output = execute_ssh_command(app, &ssh_config, &remote_command, true).await?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(AcceptanceCommandResult {
        command: command.to_string(),
        exit_code: output.status.code().unwrap_or(1),
        output_excerpt: truncate_command_output(&stdout, &stderr),
    })
}

async fn resolve_acceptance_working_dir(
    pool: &SqlitePool,
    task: &Task,
    project: &Project,
) -> Result<String, String> {
    match resolve_automation_execution_context(pool, task, project).await {
        Ok(context) => Ok(context.working_dir),
        Err(_) => {
            if project.project_type == "ssh" {
                project
                    .remote_repo_path
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| "SSH 项目缺少远程仓库路径，无法执行测试命令".to_string())
            } else {
                project
                    .repo_path
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| "项目缺少本地仓库路径，无法执行测试命令".to_string())
            }
        }
    }
}

async fn insert_acceptance_run(
    pool: &SqlitePool,
    run: &TaskAcceptanceRun,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO task_acceptance_runs (
            id, task_id, status, trigger, acceptance_checklist, command,
            command_exit_code, command_output_excerpt, ai_verdict, summary,
            created_at, finished_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
        "#,
    )
    .bind(&run.id)
    .bind(&run.task_id)
    .bind(&run.status)
    .bind(&run.trigger)
    .bind(&run.acceptance_checklist)
    .bind(&run.command)
    .bind(run.command_exit_code)
    .bind(&run.command_output_excerpt)
    .bind(&run.ai_verdict)
    .bind(&run.summary)
    .bind(&run.created_at)
    .bind(&run.finished_at)
    .execute(pool)
    .await
    .map_err(|error| format!("写入验收记录失败: {}", error))?;
    Ok(())
}

async fn update_acceptance_run_finished(
    pool: &SqlitePool,
    run: &TaskAcceptanceRun,
) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE task_acceptance_runs
        SET status = $2,
            acceptance_checklist = $3,
            command = $4,
            command_exit_code = $5,
            command_output_excerpt = $6,
            ai_verdict = $7,
            summary = $8,
            finished_at = $9
        WHERE id = $1
        "#,
    )
    .bind(&run.id)
    .bind(&run.status)
    .bind(&run.acceptance_checklist)
    .bind(&run.command)
    .bind(run.command_exit_code)
    .bind(&run.command_output_excerpt)
    .bind(&run.ai_verdict)
    .bind(&run.summary)
    .bind(&run.finished_at)
    .execute(pool)
    .await
    .map_err(|error| format!("更新验收记录失败: {}", error))?;
    Ok(())
}

async fn set_task_last_acceptance_status(
    pool: &SqlitePool,
    task_id: &str,
    status: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE tasks SET last_acceptance_status = $2, updated_at = $3 WHERE id = $1",
    )
    .bind(task_id)
    .bind(status)
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("更新任务验收状态失败: {}", error))?;
    Ok(())
}

async fn project_has_tester(pool: &SqlitePool, project_id: &str) -> Result<bool, String> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM employees
        WHERE project_id = $1
          AND role = 'tester'
        "#,
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(|error| format!("查询测试员失败: {}", error))?;
    Ok(count > 0)
}

/// Core acceptance runner. Returns the finished run.
pub async fn run_task_acceptance_internal<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task: &Task,
    trigger: &str,
) -> Result<TaskAcceptanceRun, String> {
    let project = fetch_project_by_id(pool, &task.project_id).await?;
    let policy = load_tester_automation_policy(app);
    let is_manual = trigger == ACCEPTANCE_TRIGGER_MANUAL;
    let effective_command =
        resolve_effective_test_command(&project, policy.default_test_command.as_deref());
    let checklist = task.acceptance_checklist.clone();

    let now = now_sqlite();
    let mut run = TaskAcceptanceRun {
        id: new_id(),
        task_id: task.id.clone(),
        status: ACCEPTANCE_STATUS_RUNNING.to_string(),
        trigger: trigger.to_string(),
        acceptance_checklist: checklist.clone(),
        command: effective_command.clone(),
        command_exit_code: None,
        command_output_excerpt: None,
        ai_verdict: None,
        summary: Some("验收运行中".to_string()),
        created_at: now.clone(),
        finished_at: None,
    };
    insert_acceptance_run(pool, &run).await?;
    set_task_last_acceptance_status(pool, &task.id, ACCEPTANCE_STATUS_RUNNING).await?;
    insert_activity_log(
        pool,
        "task_tester_acceptance_started",
        &format!(
            "{}（触发：{}）",
            task.title,
            if is_manual { "手动" } else { "自动" }
        ),
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    let command_result = if let Some(command) = effective_command.as_deref() {
        let working_dir = match resolve_acceptance_working_dir(pool, task, &project).await {
            Ok(dir) => dir,
            Err(error) => {
                run.status = ACCEPTANCE_STATUS_FAILED.to_string();
                run.summary = Some(error.clone());
                run.finished_at = Some(now_sqlite());
                update_acceptance_run_finished(pool, &run).await?;
                set_task_last_acceptance_status(pool, &task.id, ACCEPTANCE_STATUS_FAILED).await?;
                insert_activity_log(
                    pool,
                    "task_tester_acceptance_failed",
                    &error,
                    None,
                    Some(task.id.as_str()),
                    Some(task.project_id.as_str()),
                )
                .await?;
                return Ok(run);
            }
        };
        let result = if project.project_type == "ssh" {
            run_ssh_test_command(app, pool, &project, &working_dir, command).await
        } else {
            run_local_test_command(&working_dir, command).await
        };
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                run.status = ACCEPTANCE_STATUS_FAILED.to_string();
                run.summary = Some(error.clone());
                run.finished_at = Some(now_sqlite());
                update_acceptance_run_finished(pool, &run).await?;
                set_task_last_acceptance_status(pool, &task.id, ACCEPTANCE_STATUS_FAILED).await?;
                insert_activity_log(
                    pool,
                    "task_tester_command_failed",
                    &error,
                    None,
                    Some(task.id.as_str()),
                    Some(task.project_id.as_str()),
                )
                .await?;
                insert_activity_log(
                    pool,
                    "task_tester_acceptance_failed",
                    &error,
                    None,
                    Some(task.id.as_str()),
                    Some(task.project_id.as_str()),
                )
                .await?;
                return Ok(run);
            }
        }
    } else {
        None
    };

    if let Some(result) = command_result.as_ref() {
        run.command = Some(result.command.clone());
        run.command_exit_code = Some(result.exit_code);
        run.command_output_excerpt = Some(result.output_excerpt.clone());
    }

    let decision = decide_acceptance_result(
        command_result.as_ref(),
        policy.allow_ai_only,
        checklist.as_deref(),
        is_manual,
    );

    match decision {
        AcceptanceDecision::Passed { summary } => {
            run.status = ACCEPTANCE_STATUS_PASSED.to_string();
            run.summary = Some(summary.clone());
            run.ai_verdict = if command_result.is_none() {
                Some("ai_only_subjective_pass".to_string())
            } else {
                Some("command_pass".to_string())
            };
            run.finished_at = Some(now_sqlite());
            update_acceptance_run_finished(pool, &run).await?;
            set_task_last_acceptance_status(pool, &task.id, ACCEPTANCE_STATUS_PASSED).await?;
            insert_activity_log(
                pool,
                "task_tester_acceptance_passed",
                &summary,
                None,
                Some(task.id.as_str()),
                Some(task.project_id.as_str()),
            )
            .await?;
        }
        AcceptanceDecision::Failed { summary, hard_fail } => {
            run.status = ACCEPTANCE_STATUS_FAILED.to_string();
            run.summary = Some(summary.clone());
            run.ai_verdict = if hard_fail {
                Some("command_hard_fail".to_string())
            } else {
                Some("policy_fail".to_string())
            };
            run.finished_at = Some(now_sqlite());
            update_acceptance_run_finished(pool, &run).await?;
            set_task_last_acceptance_status(pool, &task.id, ACCEPTANCE_STATUS_FAILED).await?;
            let action = if hard_fail {
                "task_tester_command_failed"
            } else {
                "task_tester_acceptance_failed"
            };
            insert_activity_log(
                pool,
                action,
                &summary,
                None,
                Some(task.id.as_str()),
                Some(task.project_id.as_str()),
            )
            .await?;
            if hard_fail {
                insert_activity_log(
                    pool,
                    "task_tester_acceptance_failed",
                    &summary,
                    None,
                    Some(task.id.as_str()),
                    Some(task.project_id.as_str()),
                )
                .await?;
            }
        }
        AcceptanceDecision::Skipped { summary } => {
            run.status = ACCEPTANCE_STATUS_SKIPPED.to_string();
            run.summary = Some(summary.clone());
            run.ai_verdict = Some("skipped".to_string());
            run.finished_at = Some(now_sqlite());
            update_acceptance_run_finished(pool, &run).await?;
            set_task_last_acceptance_status(pool, &task.id, ACCEPTANCE_STATUS_SKIPPED).await?;
            insert_activity_log(
                pool,
                "task_tester_skipped",
                &summary,
                None,
                Some(task.id.as_str()),
                Some(task.project_id.as_str()),
            )
            .await?;
        }
    }

    Ok(run)
}

async fn launch_review_after_tester(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: Option<&TaskAutomationStateRecord>,
    consumed_session_id: Option<&str>,
) -> Result<(), String> {
    let next_round_count = state_record.map(|item| item.round_count).unwrap_or(0);
    let next_last_error = state_record.and_then(|item| item.last_error.clone());
    let next_last_verdict_json = state_record.and_then(|item| item.last_verdict_json.clone());

    reserve_pending_action(
        pool,
        &task.id,
        consumed_session_id,
        PHASE_LAUNCHING_REVIEW,
        Some(PENDING_ACTION_START_REVIEW),
        None,
        next_round_count,
        next_last_error.as_deref(),
        next_last_verdict_json.as_deref(),
    )
    .await?;

    let reserved_state = fetch_task_automation_state_record(pool, &task.id)
        .await?
        .ok_or_else(|| "自动质控状态写入后丢失，无法发起审核".to_string())?;
    let review_launched = retry_pending_review(app, pool, &task.id, &reserved_state).await?;
    if review_launched {
        insert_activity_log(
            pool,
            "task_automation_review_started",
            "测试验收通过，已自动发起代码审核",
            None,
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
    }
    Ok(())
}

async fn handle_tester_failure_for_automation(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: Option<&TaskAutomationStateRecord>,
    facts: &SessionExitFacts,
    summary: &str,
) -> Result<(), String> {
    // 测试失败回流执行人：保留 tester_failed phase，任务回到 in_progress 便于修复。
    upsert_state_terminal(
        pool,
        &task.id,
        Some(&facts.session_id),
        PHASE_TESTER_FAILED,
        state_record.and_then(|item| item.last_verdict_json.as_deref()),
        None,
        Some(summary),
        state_record,
    )
    .await?;
    update_task_status_internal(app, pool, task, "in_progress").await?;
    stop_task_timer_internal(pool, &task.id, "测试验收失败").await?;
    insert_activity_log(
        pool,
        "task_automation_manual_control",
        &format!("测试验收未通过，已回流执行：{}", summary),
        facts.employee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    emit_task_automation_state_changed(app, task, PHASE_TESTER_FAILED);
    Ok(())
}

/// Called after successful execution exit when tester automation may run.
async fn maybe_run_tester_then_review(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: Option<&TaskAutomationStateRecord>,
    facts: &SessionExitFacts,
) -> Result<bool, String> {
    let policy = load_tester_automation_policy(app);
    if !policy.enabled {
        return Ok(false);
    }

    if !project_has_tester(pool, &task.project_id).await? {
        insert_activity_log(
            pool,
            "task_tester_skipped",
            "项目无测试员，已跳过测试员阶段直接进入审核",
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        return Ok(false);
    }

    let project = fetch_project_by_id(pool, &task.project_id).await?;
    let effective_command =
        resolve_effective_test_command(&project, policy.default_test_command.as_deref());
    if effective_command.is_none() && !policy.allow_ai_only {
        insert_activity_log(
            pool,
            "task_tester_skipped",
            "未配置测试命令且不允许仅 AI 验收，已跳过测试员阶段",
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        return Ok(false);
    }

    reserve_pending_action(
        pool,
        &task.id,
        Some(&facts.session_id),
        PHASE_LAUNCHING_TESTER,
        None,
        None,
        state_record.map(|item| item.round_count).unwrap_or(0),
        None,
        state_record.and_then(|item| item.last_verdict_json.as_deref()),
    )
    .await?;
    emit_task_automation_state_changed(app, task, PHASE_LAUNCHING_TESTER);

    // Synchronous command-based acceptance (no long-lived tester session in MVP).
    upsert_state_terminal(
        pool,
        &task.id,
        Some(&facts.session_id),
        PHASE_WAITING_TESTER,
        state_record.and_then(|item| item.last_verdict_json.as_deref()),
        None,
        None,
        state_record,
    )
    .await?;
    emit_task_automation_state_changed(app, task, PHASE_WAITING_TESTER);

    let latest_task = fetch_task_by_id(pool, &task.id).await?;
    let run =
        run_task_acceptance_internal(app, pool, &latest_task, ACCEPTANCE_TRIGGER_AUTO).await?;

    match run.status.as_str() {
        ACCEPTANCE_STATUS_PASSED | ACCEPTANCE_STATUS_SKIPPED => {
            launch_review_after_tester(
                app,
                pool,
                task,
                state_record,
                Some(&facts.session_id),
            )
            .await?;
        }
        _ => {
            let summary = run
                .summary
                .clone()
                .unwrap_or_else(|| "测试验收失败".to_string());
            handle_tester_failure_for_automation(
                app,
                pool,
                task,
                state_record,
                facts,
                &summary,
            )
            .await?;
        }
    }

    Ok(true)
}

#[tauri::command]
pub async fn run_task_acceptance(
    app: AppHandle,
    payload: RunTaskAcceptancePayload,
) -> Result<TaskAcceptanceRun, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    let trigger = payload
        .trigger
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)))
        .unwrap_or_else(|| ACCEPTANCE_TRIGGER_MANUAL.to_string());
    if trigger != ACCEPTANCE_TRIGGER_MANUAL && trigger != ACCEPTANCE_TRIGGER_AUTO {
        return Err("无效的验收触发类型".to_string());
    }
    run_task_acceptance_internal(&app, &pool, &task, &trigger).await
}

#[tauri::command]
pub async fn get_task_acceptance_runs(
    app: AppHandle,
    task_id: String,
) -> Result<Vec<TaskAcceptanceRun>, String> {
    let pool = sqlite_pool(&app).await?;
    let _task = fetch_task_by_id(&pool, &task_id).await?;
    sqlx::query_as::<_, TaskAcceptanceRun>(
        r#"
        SELECT *
        FROM task_acceptance_runs
        WHERE task_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(&task_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("查询验收历史失败: {}", error))
}

#[tauri::command]
pub async fn update_task_acceptance_checklist(
    app: AppHandle,
    payload: UpdateTaskAcceptanceChecklistPayload,
) -> Result<Task, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    let checklist = normalize_optional_text(Some(&payload.acceptance_checklist));
    sqlx::query(
        "UPDATE tasks SET acceptance_checklist = $2, updated_at = $3 WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(&task.id)
    .bind(&checklist)
    .bind(now_sqlite())
    .execute(&pool)
    .await
    .map_err(|error| format!("更新验收清单失败: {}", error))?;
    insert_activity_log(
        &pool,
        "task_tester_checklist_updated",
        &format!(
            "{}（验收清单 {} 字）",
            task.title,
            checklist.as_deref().map(str::len).unwrap_or(0)
        ),
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    fetch_task_by_id(&pool, &task.id).await
}

#[cfg(test)]
mod acceptance_unit_tests {
    use super::*;

    #[test]
    fn hard_fail_when_command_nonzero_even_if_ai_only_allowed() {
        let result = AcceptanceCommandResult {
            command: "npm test".into(),
            exit_code: 1,
            output_excerpt: "fail".into(),
        };
        let decision = decide_acceptance_result(Some(&result), true, Some("checklist"), true);
        match decision {
            AcceptanceDecision::Failed { hard_fail, .. } => assert!(hard_fail),
            other => panic!("expected hard fail, got {other:?}"),
        }
    }

    #[test]
    fn pass_when_command_zero() {
        let result = AcceptanceCommandResult {
            command: "cargo test".into(),
            exit_code: 0,
            output_excerpt: "ok".into(),
        };
        let decision = decide_acceptance_result(Some(&result), false, None, true);
        assert!(matches!(decision, AcceptanceDecision::Passed { .. }));
    }

    #[test]
    fn ai_only_pass_with_checklist() {
        let decision = decide_acceptance_result(None, true, Some("- [ ] item"), true);
        assert!(matches!(decision, AcceptanceDecision::Passed { .. }));
    }

    #[test]
    fn skip_auto_without_command_when_ai_only_disallowed() {
        let decision = decide_acceptance_result(None, false, None, false);
        assert!(matches!(decision, AcceptanceDecision::Skipped { .. }));
    }

    #[test]
    fn resolve_project_command_over_default() {
        let project = Project {
            id: "p".into(),
            name: "n".into(),
            description: None,
            status: "active".into(),
            repo_path: None,
            project_type: "local".into(),
            ssh_config_id: None,
            remote_repo_path: None,
            test_command: Some("  npm test  ".into()),
            deleted_at: None,
            created_at: "t".into(),
            updated_at: "t".into(),
        };
        assert_eq!(
            resolve_effective_test_command(&project, Some("cargo test")).as_deref(),
            Some("npm test")
        );
    }
}
