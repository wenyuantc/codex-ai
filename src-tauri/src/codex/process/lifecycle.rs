use super::*;

pub(super) async fn persist_execution_change_history<R: Runtime>(
    app: &AppHandle<R>,
    session_record_id: &str,
    session_kind: CodexSessionKind,
    provider: CodexExecutionProvider,
    execution_change_baseline: Option<&ExecutionChangeBaseline>,
    sdk_file_change_store: Option<&SdkFileChangeStore>,
) {
    if session_kind != CodexSessionKind::Execution {
        return;
    }

    let result = async {
        let session = fetch_codex_session_by_id(app, session_record_id).await?;
        let (changes, artifact_capture_mode) = if session.execution_target == EXECUTION_TARGET_SSH {
            match (
                session.ssh_config_id.as_deref(),
                session.working_dir.as_deref(),
            ) {
                (Some(ssh_config_id), Some(working_dir)) => {
                    let pool = sqlite_pool(app).await?;
                    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
                    let remote_baseline = execution_change_baseline
                        .filter(|baseline| baseline.execution_target == EXECUTION_TARGET_SSH);
                    let resolved_working_dir = match remote_baseline {
                        Some(baseline) => baseline.repo_path.clone(),
                        None => resolve_remote_working_dir(app, &ssh_config, working_dir).await?,
                    };

                    match provider {
                        CodexExecutionProvider::Sdk => {
                            let empty_entries = HashMap::<String, WorkingTreeSnapshotEntry>::new();
                            let baseline_entries = remote_baseline
                                .map(|baseline| &baseline.entries)
                                .unwrap_or(&empty_entries);
                            let sdk_changes = sdk_file_change_store
                                .map(|store| {
                                    let guard = store.lock().unwrap();
                                    let mut values = guard.values().cloned().collect::<Vec<_>>();
                                    values.sort_by(|left, right| left.path.cmp(&right.path));
                                    values
                                })
                                .unwrap_or_default();

                            if !sdk_changes.is_empty() {
                                let detailed_changes = attach_remote_session_file_change_details(
                                    app,
                                    &ssh_config,
                                    &resolved_working_dir,
                                    baseline_entries,
                                    sdk_changes,
                                )
                                .await;
                                (detailed_changes, ARTIFACT_CAPTURE_MODE_SSH_FULL.to_string())
                            } else if let Some(remote_baseline) = remote_baseline {
                                match compute_remote_execution_session_file_changes(
                                    app,
                                    &ssh_config,
                                    remote_baseline,
                                )
                                .await
                                {
                                    Ok(detailed_changes) => (
                                        detailed_changes,
                                        ARTIFACT_CAPTURE_MODE_SSH_FULL.to_string(),
                                    ),
                                    Err(_) => {
                                        match capture_remote_git_status_changes(
                                            app,
                                            &ssh_config,
                                            &resolved_working_dir,
                                        )
                                        .await
                                        {
                                            Ok(changes) => (
                                                changes,
                                                ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS.to_string(),
                                            ),
                                            Err(_) => (
                                                Vec::new(),
                                                ARTIFACT_CAPTURE_MODE_SSH_NONE.to_string(),
                                            ),
                                        }
                                    }
                                }
                            } else {
                                match capture_remote_git_status_changes(
                                    app,
                                    &ssh_config,
                                    &resolved_working_dir,
                                )
                                .await
                                {
                                    Ok(changes) => {
                                        (changes, ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS.to_string())
                                    }
                                    Err(_) => {
                                        (Vec::new(), ARTIFACT_CAPTURE_MODE_SSH_NONE.to_string())
                                    }
                                }
                            }
                        }
                        CodexExecutionProvider::Cli => {
                            if let Some(remote_baseline) = remote_baseline {
                                match compute_remote_execution_session_file_changes(
                                    app,
                                    &ssh_config,
                                    remote_baseline,
                                )
                                .await
                                {
                                    Ok(detailed_changes) => (
                                        detailed_changes,
                                        ARTIFACT_CAPTURE_MODE_SSH_FULL.to_string(),
                                    ),
                                    Err(_) => {
                                        match capture_remote_git_status_changes(
                                            app,
                                            &ssh_config,
                                            &resolved_working_dir,
                                        )
                                        .await
                                        {
                                            Ok(changes) => (
                                                changes,
                                                ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS.to_string(),
                                            ),
                                            Err(_) => (
                                                Vec::new(),
                                                ARTIFACT_CAPTURE_MODE_SSH_NONE.to_string(),
                                            ),
                                        }
                                    }
                                }
                            } else {
                                match capture_remote_git_status_changes(
                                    app,
                                    &ssh_config,
                                    &resolved_working_dir,
                                )
                                .await
                                {
                                    Ok(changes) => {
                                        (changes, ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS.to_string())
                                    }
                                    Err(_) => {
                                        (Vec::new(), ARTIFACT_CAPTURE_MODE_SSH_NONE.to_string())
                                    }
                                }
                            }
                        }
                    }
                }
                _ => (Vec::new(), ARTIFACT_CAPTURE_MODE_SSH_NONE.to_string()),
            }
        } else {
            let changes = match provider {
                CodexExecutionProvider::Sdk => compute_local_sdk_execution_session_file_changes(
                    execution_change_baseline,
                    sdk_file_change_store,
                )?,
                CodexExecutionProvider::Cli => {
                    let Some(execution_change_baseline) = execution_change_baseline else {
                        return Ok(());
                    };
                    compute_execution_session_file_changes(execution_change_baseline)?
                }
            };
            (changes, ARTIFACT_CAPTURE_MODE_LOCAL_FULL.to_string())
        };

        if session.execution_target == EXECUTION_TARGET_SSH {
            if let Ok(pool) = sqlite_pool(app).await {
                match artifact_capture_mode.as_str() {
                    ARTIFACT_CAPTURE_MODE_SSH_FULL => {
                        let _ = insert_activity_log(
                            &pool,
                            "remote_session_artifact_captured",
                            "远程会话变更明细已保存",
                            session.employee_id.as_deref(),
                            session.task_id.as_deref(),
                            session.project_id.as_deref(),
                        )
                        .await;
                    }
                    ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS => {
                        let _ = insert_codex_session_event(
                            &pool,
                            session_record_id,
                            "remote_artifact_capture_limited",
                            Some("远程会话变更明细受限，仅采集到 git status 摘要"),
                        )
                        .await;
                        let _ = insert_activity_log(
                            &pool,
                            "remote_session_artifact_limited",
                            "远程会话变更明细受限，仅采集到文件摘要",
                            session.employee_id.as_deref(),
                            session.task_id.as_deref(),
                            session.project_id.as_deref(),
                        )
                        .await;
                    }
                    ARTIFACT_CAPTURE_MODE_SSH_NONE => {
                        let _ = insert_codex_session_event(
                            &pool,
                            session_record_id,
                            "remote_artifact_capture_limited",
                            Some("远程会话变更明细受限，未采集到 git status 摘要"),
                        )
                        .await;
                        let _ = insert_activity_log(
                            &pool,
                            "remote_session_artifact_limited",
                            "远程会话变更明细受限",
                            session.employee_id.as_deref(),
                            session.task_id.as_deref(),
                            session.project_id.as_deref(),
                        )
                        .await;
                    }
                    _ => {}
                }
            }
        }

        if let Ok(pool) = sqlite_pool(app).await {
            let _ =
                sqlx::query("UPDATE codex_sessions SET artifact_capture_mode = $2 WHERE id = $1")
                    .bind(session_record_id)
                    .bind(&artifact_capture_mode)
                    .execute(&pool)
                    .await;
        }
        replace_codex_session_file_changes(app, session_record_id, &changes).await
    }
    .await;

    if let Err(error) = result {
        if let Ok(pool) = sqlite_pool(app).await {
            let _ = insert_codex_session_event(
                &pool,
                session_record_id,
                "session_file_changes_failed",
                Some(&error),
            )
            .await;
        }
    }
}

async fn finalize_stale_process_slot<R: Runtime>(
    app: &AppHandle<R>,
    session_record_id: &str,
    exit_code: Option<i32>,
    error_message: Option<&str>,
    provider: CodexExecutionProvider,
    execution_change_baseline: Option<&ExecutionChangeBaseline>,
    sdk_file_change_store: Option<&SdkFileChangeStore>,
) {
    let current = fetch_codex_session_by_id(app, session_record_id).await.ok();
    let Some(current) = current else {
        return;
    };

    if matches!(current.status.as_str(), "exited" | "failed") {
        return;
    }

    let final_status =
        if current.status == "stopping" || (exit_code == Some(0) && error_message.is_none()) {
            "exited"
        } else {
            "failed"
        };
    let ended_at = now_sqlite();
    let _ = update_codex_session_record(
        app,
        session_record_id,
        Some(final_status),
        None,
        Some(exit_code),
        Some(Some(ended_at.as_str())),
    )
    .await;

    if let Ok(pool) = sqlite_pool(app).await {
        let event_type = if final_status == "exited" {
            "session_exited"
        } else {
            "session_failed"
        };
        let message = error_message
            .map(|message| format!("检测到残留进程槽位并已回收: {}", message))
            .unwrap_or_else(|| {
                format!(
                    "检测到已退出进程并已回收，exit_code={}",
                    exit_code.unwrap_or_default()
                )
            });
        let _ =
            insert_codex_session_event(&pool, session_record_id, event_type, Some(&message)).await;
    }

    persist_execution_change_history(
        app,
        session_record_id,
        normalize_session_kind(Some(current.session_kind.as_str())),
        provider,
        execution_change_baseline,
        sdk_file_change_store,
    )
    .await;
}

pub(super) fn cleanup_process_artifacts(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

async fn validate_managed_process<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    process: crate::codex::manager::ManagedCodexProcess,
) -> Result<Option<crate::codex::manager::ManagedCodexProcess>, String> {
    let status = {
        let mut child = process.child.lock().await;
        child.try_wait()
    };

    match status {
        Ok(None) => Ok(Some(process)),
        Ok(Some(exit_code)) => {
            finalize_stale_process_slot(
                app,
                &process.session_record_id,
                Some(exit_code),
                None,
                process.provider,
                process.execution_change_baseline.as_ref(),
                process.sdk_file_change_store.as_ref(),
            )
            .await;
            cleanup_process_artifacts(&process.cleanup_paths);
            let mut manager = manager_state.lock().map_err(|error| error.to_string())?;
            manager.remove_process(&process.session_record_id);
            Ok(None)
        }
        Err(error) => {
            finalize_stale_process_slot(
                app,
                &process.session_record_id,
                None,
                Some(error.as_str()),
                process.provider,
                process.execution_change_baseline.as_ref(),
                process.sdk_file_change_store.as_ref(),
            )
            .await;
            cleanup_process_artifacts(&process.cleanup_paths);
            let mut manager = manager_state.lock().map_err(|error| error.to_string())?;
            manager.remove_process(&process.session_record_id);
            Ok(None)
        }
    }
}

pub(super) async fn get_live_managed_process_with_manager<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    employee_id: &str,
) -> Result<Option<crate::codex::manager::ManagedCodexProcess>, String> {
    let processes =
        get_live_managed_processes_with_manager(app, manager_state, employee_id).await?;
    Ok(processes.into_iter().next())
}

pub(super) async fn get_live_managed_process_by_session_with_manager<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    session_record_id: &str,
) -> Result<Option<crate::codex::manager::ManagedCodexProcess>, String> {
    let process = {
        let manager = manager_state.lock().map_err(|error| error.to_string())?;
        manager.get_process(session_record_id)
    };

    let Some(process) = process else {
        return Ok(None);
    };

    validate_managed_process(app, manager_state, process).await
}

pub(super) async fn get_live_managed_processes_with_manager<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    employee_id: &str,
) -> Result<Vec<crate::codex::manager::ManagedCodexProcess>, String> {
    let processes = {
        let manager = manager_state.lock().map_err(|error| error.to_string())?;
        manager.get_employee_processes(employee_id)
    };
    let mut live_processes = Vec::with_capacity(processes.len());

    for process in processes {
        if let Some(process) = validate_managed_process(app, manager_state, process).await? {
            live_processes.push(process);
        }
    }

    Ok(live_processes)
}

pub(super) async fn get_live_task_process_by_task_with_manager<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    task_id: &str,
    session_kind: CodexSessionKind,
) -> Result<Option<crate::codex::manager::ManagedCodexProcess>, String> {
    let processes = {
        let manager = manager_state.lock().map_err(|error| error.to_string())?;
        manager.get_processes()
    };

    for process in processes {
        if process.task_id.as_deref() != Some(task_id) || process.session_kind != session_kind {
            continue;
        }

        if let Some(process) = validate_managed_process(app, manager_state, process).await? {
            return Ok(Some(process));
        }
    }

    Ok(None)
}

async fn wait_until_process_stops_with_manager<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    session_record_id: &str,
) -> Result<(), String> {
    for _ in 0..STOP_WAIT_MAX_ATTEMPTS {
        if get_live_managed_process_by_session_with_manager(app, manager_state, session_record_id)
            .await?
            .is_none()
        {
            return Ok(());
        }
        sleep(Duration::from_millis(STOP_WAIT_POLL_MS)).await;
    }

    let process = {
        let mut manager = manager_state.lock().map_err(|error| error.to_string())?;
        manager.remove_process(session_record_id)
    };

    if let Some(process) = process {
        finalize_stale_process_slot(
            app,
            &process.session_record_id,
            None,
            Some("停止等待超时，已强制回收运行槽位"),
            process.provider,
            process.execution_change_baseline.as_ref(),
            process.sdk_file_change_store.as_ref(),
        )
        .await;
        cleanup_process_artifacts(&process.cleanup_paths);
    }

    Ok(())
}

pub(super) async fn stop_managed_process_with_manager<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    session_record_id: &str,
    event_type: &str,
    message: &str,
) -> Result<bool, String> {
    let running_process =
        get_live_managed_process_by_session_with_manager(app, manager_state, session_record_id)
            .await?;

    if let Some(process) = running_process {
        let pool = sqlite_pool(app).await?;
        update_codex_session_record(
            app,
            &process.session_record_id,
            Some("stopping"),
            None,
            None,
            None,
        )
        .await?;
        insert_codex_session_event(&pool, &process.session_record_id, event_type, Some(message))
            .await?;

        let mut child = process.child.lock().await;
        if let Err(error) = child.kill_process_group() {
            eprintln!("[codex-stop] killpg failed, fallback to child.kill(): {error}");
        }
        child.kill().await?;
        drop(child);
        wait_until_process_stops_with_manager(app, manager_state, &process.session_record_id)
            .await?;
        return Ok(true);
    }

    Ok(false)
}

pub(crate) async fn stop_codex_for_automation_restart<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    expected_session_record_id: Option<&str>,
    message: &str,
) -> Result<bool, String> {
    let manager_state = app.state::<Arc<Mutex<CodexManager>>>().inner().clone();
    let Some(expected_session_record_id) = expected_session_record_id else {
        return Err("当前自动化步骤缺少会话标识，无法安全重启".to_string());
    };

    let running_process = get_live_managed_process_by_session_with_manager(
        app,
        &manager_state,
        expected_session_record_id,
    )
    .await?;

    let Some(process) = running_process else {
        return Ok(false);
    };

    if process.employee_id != employee_id {
        return Err("当前员工正在执行其他任务，无法重启这条自动化步骤".to_string());
    }

    stop_managed_process_with_manager(
        app,
        &manager_state,
        expected_session_record_id,
        "automation_restart_requested",
        message,
    )
    .await
}

pub struct CodexChild {
    child: Child,
}

impl CodexChild {
    pub(crate) fn new(child: Child) -> Self {
        Self { child }
    }

    #[cfg(unix)]
    pub fn kill_process_group(&mut self) -> Result<(), String> {
        let Some(pid) = self.child.id() else {
            return Ok(());
        };

        let result = unsafe { libc::killpg(pid as i32, libc::SIGKILL) };
        if result == 0 {
            Ok(())
        } else {
            let error = std::io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(()),
                _ => Err(error.to_string()),
            }
        }
    }

    #[cfg(not(unix))]
    pub fn kill_process_group(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub async fn kill(&mut self) -> Result<(), String> {
        match self.child.kill().await {
            Ok(()) => Ok(()),
            Err(error) => match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(()),
                _ => Err(error.to_string()),
            },
        }
    }

    pub fn try_wait(&mut self) -> Result<Option<i32>, String> {
        self.child
            .try_wait()
            .map(|status| status.and_then(|status| status.code()))
            .map_err(|e: std::io::Error| e.to_string())
    }
}

pub(super) async fn wait_for_exec_session_id(
    run_cwd: &str,
    started_at: SystemTime,
) -> Option<String> {
    for _ in 0..120 {
        if let Some(session_id) = find_latest_exec_session_id(run_cwd, started_at) {
            return Some(session_id);
        }
        sleep(Duration::from_millis(500)).await;
    }
    None
}

pub(super) fn find_latest_exec_session_id(run_cwd: &str, started_at: SystemTime) -> Option<String> {
    let sessions_root = std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".codex/sessions"))?;
    let mut latest: Option<(SystemTime, String)> = None;
    collect_session_files(&sessions_root)
        .into_iter()
        .filter_map(|path| {
            let modified = fs::metadata(&path).ok()?.modified().ok()?;
            if modified
                < started_at
                    .checked_sub(Duration::from_secs(2))
                    .unwrap_or(started_at)
            {
                return None;
            }
            let file = fs::File::open(&path).ok()?;
            let mut reader = StdBufReader::new(file);
            let mut first_line = String::new();
            reader.read_line(&mut first_line).ok()?;
            let json: serde_json::Value = serde_json::from_str(first_line.trim()).ok()?;
            if json.get("type")?.as_str()? != "session_meta" {
                return None;
            }
            let payload = json.get("payload")?;
            let is_exec_session = payload.get("source").and_then(|v| v.as_str()) == Some("exec")
                || payload.get("originator").and_then(|v| v.as_str()) == Some("codex_exec");
            if !is_exec_session {
                return None;
            }
            if payload.get("cwd").and_then(|v| v.as_str()) != Some(run_cwd) {
                return None;
            }
            let session_id = payload.get("id")?.as_str()?.to_string();
            Some((modified, session_id))
        })
        .for_each(|candidate| {
            if latest
                .as_ref()
                .map(|(modified, _)| candidate.0 > *modified)
                .unwrap_or(true)
            {
                latest = Some(candidate);
            }
        });

    latest.map(|(_, session_id)| session_id)
}

fn collect_session_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(year_dirs) = fs::read_dir(root) {
        for year_dir in year_dirs.flatten() {
            if let Ok(month_dirs) = fs::read_dir(year_dir.path()) {
                for month_dir in month_dirs.flatten() {
                    if let Ok(day_files) = fs::read_dir(month_dir.path()) {
                        for day_file in day_files.flatten() {
                            let path = day_file.path();
                            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                                files.push(path);
                            }
                        }
                    }
                }
            }
        }
    }
    files
}
