use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

use super::*;

pub(super) struct SessionLaunch {
    pub(super) provider: CodexExecutionProvider,
    pub(super) command: tokio::process::Command,
    pub(super) cleanup_paths: Vec<PathBuf>,
    pub(super) cli_json_output_flag: Option<CliJsonOutputFlag>,
    pub(super) session_lookup_started_at: Option<SystemTime>,
    pub(super) sdk_codex_path_override: Option<String>,
    pub(super) ssh_config_name: Option<String>,
    pub(super) ssh_host: Option<String>,
    pub(super) ssh_config_for_artifact_capture: Option<SshConfigRecord>,
    pub(super) remote_sdk_fallback_error: Option<String>,
    pub(super) remote_sdk_fallback_logged: bool,
}

fn configure_command_pipes(command: &mut tokio::process::Command) {
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
}

async fn build_remote_exec_command(
    app: &AppHandle,
    ssh_config: &SshConfigRecord,
    remote_command: String,
    allocate_tty: bool,
) -> Result<(tokio::process::Command, Vec<PathBuf>), String> {
    let (mut command, askpass_path) =
        build_ssh_command(app, ssh_config, Some(&remote_command), true, allocate_tty).await?;
    configure_command_pipes(&mut command);
    Ok((command, askpass_path.into_iter().collect()))
}

async fn build_remote_cli_fallback_command(
    app: &AppHandle,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: CodexSessionKind,
    ssh_config: &SshConfigRecord,
    node_path_override: Option<&str>,
    model: &str,
    reasoning_effort: &str,
    run_cwd: &str,
    image_paths: &[String],
    resume_session_id: Option<&str>,
    remote_exec_json_flag: Option<CliJsonOutputFlag>,
    fallback_message: Option<String>,
) -> Result<(tokio::process::Command, Vec<PathBuf>), String> {
    if let Some(fallback_message) = fallback_message {
        emit_session_terminal_line(
            app,
            pool,
            session_record_id,
            employee_id,
            task_id,
            session_kind,
            fallback_message,
        )
        .await;
        emit_session_terminal_line(
            app,
            pool,
            session_record_id,
            employee_id,
            task_id,
            session_kind,
            "[SSH] 正在改用远程 codex exec 建立会话...".to_string(),
        )
        .await;
    } else {
        emit_session_terminal_line(
            app,
            pool,
            session_record_id,
            employee_id,
            task_id,
            session_kind,
            "[SSH] 当前远程任务执行未启用 SDK，正在通过远程 codex exec 建立会话...".to_string(),
        )
        .await;
    }

    let remote_command = build_remote_codex_session_command(
        model,
        reasoning_effort,
        run_cwd,
        image_paths,
        resume_session_id,
        remote_exec_json_flag,
        node_path_override,
    );
    build_remote_exec_command(
        app,
        ssh_config,
        remote_command,
        REMOTE_EXEC_SSH_ALLOCATE_TTY,
    )
    .await
}

async fn prepare_remote_session_launch(
    app: &AppHandle,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: CodexSessionKind,
    execution_context: &ExecutionContext,
    model: &str,
    reasoning_effort: &str,
    run_cwd: &str,
    image_paths: &[String],
    resume_session_id: Option<&str>,
) -> Result<SessionLaunch, String> {
    let ssh_config_id = execution_context
        .ssh_config_id
        .as_deref()
        .ok_or_else(|| "SSH 项目缺少 ssh_config_id，无法启动 Codex。".to_string())?;
    let ssh_config = fetch_ssh_config_record_by_id(&sqlite_pool(app).await?, ssh_config_id).await?;
    let remote_settings = load_remote_codex_settings(app, ssh_config_id).ok();
    let remote_exec_json_flag = match probe_remote_exec_json_support(
        app,
        &ssh_config,
        remote_settings
            .as_ref()
            .and_then(|settings| settings.node_path_override.as_deref()),
    )
    .await
    {
        Ok(flag) => flag,
        Err(error) => {
            eprintln!("[codex-cli] 探测远程 codex exec --json 支持失败: {error}");
            None
        }
    };

    emit_session_terminal_line(
        app,
        pool,
        session_record_id,
        employee_id,
        task_id,
        session_kind,
        format!(
            "[SSH] 正在连接 {}（{}@{}:{}）...",
            ssh_config.name, ssh_config.username, ssh_config.host, ssh_config.port
        ),
    )
    .await;

    let ssh_config_name = Some(ssh_config.name.clone());
    let ssh_host = Some(format!("{}:{}", ssh_config.host, ssh_config.port));
    let ssh_config_for_artifact_capture = Some(ssh_config.clone());
    let mut remote_sdk_fallback_error = None;
    let mut remote_sdk_fallback_logged = false;

    if remote_settings
        .as_ref()
        .map(|settings| settings.task_sdk_enabled)
        .unwrap_or(false)
    {
        if let Some(remote_settings) = remote_settings.as_ref() {
            emit_session_terminal_line(
                app,
                pool,
                session_record_id,
                employee_id,
                task_id,
                session_kind,
                "[SSH] 正在检查远程 Codex / Node / SDK 运行环境...".to_string(),
            )
            .await;
            match inspect_remote_codex_runtime(app, &ssh_config, remote_settings).await {
                Ok(runtime) if runtime.task_execution_effective_provider == "sdk" => {
                    emit_session_terminal_line(
                        app,
                        pool,
                        session_record_id,
                        employee_id,
                        task_id,
                        session_kind,
                        format!("[SSH] {}", runtime.status_message),
                    )
                    .await;
                    emit_session_terminal_line(
                        app,
                        pool,
                        session_record_id,
                        employee_id,
                        task_id,
                        session_kind,
                        "[SSH] 正在准备远程 SDK 运行目录与 bridge...".to_string(),
                    )
                    .await;
                    match ensure_remote_sdk_runtime_layout(app, ssh_config_id).await {
                        Ok(remote_runtime_settings) => {
                            emit_session_terminal_line(
                                app,
                                pool,
                                session_record_id,
                                employee_id,
                                task_id,
                                session_kind,
                                "[SSH] 远程 SDK 运行目录已就绪，正在建立远程会话...".to_string(),
                            )
                            .await;
                            let remote_command = build_remote_sdk_bridge_command(
                                &remote_runtime_settings.sdk_install_dir,
                                remote_runtime_settings.node_path_override.as_deref(),
                            );
                            match build_ssh_command(
                                app,
                                &ssh_config,
                                Some(&remote_command),
                                true,
                                false,
                            )
                            .await
                            {
                                Ok((mut command, askpass_path)) => {
                                    configure_command_pipes(&mut command);
                                    return Ok(SessionLaunch {
                                        provider: CodexExecutionProvider::Sdk,
                                        command,
                                        cleanup_paths: askpass_path.into_iter().collect(),
                                        cli_json_output_flag: None,
                                        session_lookup_started_at: None,
                                        sdk_codex_path_override: None,
                                        ssh_config_name,
                                        ssh_host,
                                        ssh_config_for_artifact_capture,
                                        remote_sdk_fallback_error,
                                        remote_sdk_fallback_logged,
                                    });
                                }
                                Err(error) => {
                                    remote_sdk_fallback_error = Some(error.clone());
                                    remote_sdk_fallback_logged = true;
                                    let (command, cleanup_paths) = build_remote_cli_fallback_command(
                                        app,
                                        pool,
                                        session_record_id,
                                        employee_id,
                                        task_id,
                                        session_kind,
                                        &ssh_config,
                                        remote_settings.node_path_override.as_deref(),
                                        model,
                                        reasoning_effort,
                                        run_cwd,
                                        image_paths,
                                        resume_session_id,
                                        remote_exec_json_flag,
                                        Some(format!(
                                            "[WARN] [SSH] 远程 SDK 启动失败，已回退到远程 codex exec：{}",
                                            error
                                        )),
                                    )
                                    .await?;
                                    return Ok(SessionLaunch {
                                        provider: CodexExecutionProvider::Cli,
                                        command,
                                        cleanup_paths,
                                        cli_json_output_flag: remote_exec_json_flag,
                                        session_lookup_started_at: None,
                                        sdk_codex_path_override: None,
                                        ssh_config_name,
                                        ssh_host,
                                        ssh_config_for_artifact_capture,
                                        remote_sdk_fallback_error,
                                        remote_sdk_fallback_logged,
                                    });
                                }
                            }
                        }
                        Err(error) => {
                            remote_sdk_fallback_error = Some(error.clone());
                            remote_sdk_fallback_logged = true;
                            let (command, cleanup_paths) = build_remote_cli_fallback_command(
                                app,
                                pool,
                                session_record_id,
                                employee_id,
                                task_id,
                                session_kind,
                                &ssh_config,
                                remote_settings.node_path_override.as_deref(),
                                model,
                                reasoning_effort,
                                run_cwd,
                                image_paths,
                                resume_session_id,
                                remote_exec_json_flag,
                                Some(format!(
                                    "[WARN] [SSH] 远程 SDK 准备失败，已回退到远程 codex exec：{}",
                                    error
                                )),
                            )
                            .await?;
                            return Ok(SessionLaunch {
                                provider: CodexExecutionProvider::Cli,
                                command,
                                cleanup_paths,
                                cli_json_output_flag: remote_exec_json_flag,
                                session_lookup_started_at: None,
                                sdk_codex_path_override: None,
                                ssh_config_name,
                                ssh_host,
                                ssh_config_for_artifact_capture,
                                remote_sdk_fallback_error,
                                remote_sdk_fallback_logged,
                            });
                        }
                    }
                }
                Ok(runtime) => {
                    remote_sdk_fallback_error = Some(runtime.status_message.clone());
                    remote_sdk_fallback_logged = true;
                    let (command, cleanup_paths) = build_remote_cli_fallback_command(
                        app,
                        pool,
                        session_record_id,
                        employee_id,
                        task_id,
                        session_kind,
                        &ssh_config,
                        remote_settings.node_path_override.as_deref(),
                        model,
                        reasoning_effort,
                        run_cwd,
                        image_paths,
                        resume_session_id,
                        remote_exec_json_flag,
                        Some(format!("[WARN] [SSH] {}", runtime.status_message)),
                    )
                    .await?;
                    return Ok(SessionLaunch {
                        provider: CodexExecutionProvider::Cli,
                        command,
                        cleanup_paths,
                        cli_json_output_flag: remote_exec_json_flag,
                        session_lookup_started_at: None,
                        sdk_codex_path_override: None,
                        ssh_config_name,
                        ssh_host,
                        ssh_config_for_artifact_capture,
                        remote_sdk_fallback_error,
                        remote_sdk_fallback_logged,
                    });
                }
                Err(error) => {
                    remote_sdk_fallback_error = Some(error.clone());
                    remote_sdk_fallback_logged = true;
                    let (command, cleanup_paths) = build_remote_cli_fallback_command(
                        app,
                        pool,
                        session_record_id,
                        employee_id,
                        task_id,
                        session_kind,
                        &ssh_config,
                        remote_settings.node_path_override.as_deref(),
                        model,
                        reasoning_effort,
                        run_cwd,
                        image_paths,
                        resume_session_id,
                        remote_exec_json_flag,
                        Some(format!(
                            "[WARN] [SSH] 远程运行环境检查失败，已回退到远程 codex exec：{}",
                            error
                        )),
                    )
                    .await?;
                    return Ok(SessionLaunch {
                        provider: CodexExecutionProvider::Cli,
                        command,
                        cleanup_paths,
                        cli_json_output_flag: remote_exec_json_flag,
                        session_lookup_started_at: None,
                        sdk_codex_path_override: None,
                        ssh_config_name,
                        ssh_host,
                        ssh_config_for_artifact_capture,
                        remote_sdk_fallback_error,
                        remote_sdk_fallback_logged,
                    });
                }
            }
        }
    }

    let (command, cleanup_paths) = build_remote_cli_fallback_command(
        app,
        pool,
        session_record_id,
        employee_id,
        task_id,
        session_kind,
        &ssh_config,
        remote_settings
            .as_ref()
            .and_then(|settings| settings.node_path_override.as_deref()),
        model,
        reasoning_effort,
        run_cwd,
        image_paths,
        resume_session_id,
        remote_exec_json_flag,
        None,
    )
    .await?;

    Ok(SessionLaunch {
        provider: CodexExecutionProvider::Cli,
        command,
        cleanup_paths,
        cli_json_output_flag: remote_exec_json_flag,
        session_lookup_started_at: None,
        sdk_codex_path_override: None,
        ssh_config_name,
        ssh_host,
        ssh_config_for_artifact_capture,
        remote_sdk_fallback_error,
        remote_sdk_fallback_logged,
    })
}

async fn prepare_local_session_launch(
    app: &AppHandle,
    run_cwd: &str,
) -> Result<SessionLaunch, String> {
    if should_use_sdk_for_session(app).await {
        match load_codex_settings(app) {
            Ok(settings) => {
                let install_dir = PathBuf::from(&settings.sdk_install_dir);
                if let Err(error) = ensure_sdk_runtime_layout(&install_dir) {
                    eprintln!("[codex-sdk] 刷新 SDK bridge 失败，回退 CLI: {error}");
                } else {
                    let bridge_path = sdk_bridge_script_path(Path::new(&settings.sdk_install_dir));
                    match new_node_command(settings.node_path_override.as_deref()).await {
                        Ok(mut command) => {
                            let sdk_codex_path_override = resolve_codex_executable_path()
                                .await
                                .ok()
                                .and_then(|path| sdk_codex_path_override_from_resolved_path(&path));
                            if let Some(ref codex_path_override) = sdk_codex_path_override {
                                command.env("CODEX_CLI_PATH", codex_path_override);
                            }
                            configure_command_pipes(&mut command);
                            command.arg(&bridge_path).current_dir(run_cwd);
                            return Ok(SessionLaunch {
                                provider: CodexExecutionProvider::Sdk,
                                command,
                                cleanup_paths: Vec::new(),
                                cli_json_output_flag: None,
                                session_lookup_started_at: None,
                                sdk_codex_path_override,
                                ssh_config_name: None,
                                ssh_host: None,
                                ssh_config_for_artifact_capture: None,
                                remote_sdk_fallback_error: None,
                                remote_sdk_fallback_logged: false,
                            });
                        }
                        Err(error) => {
                            eprintln!("[codex-sdk] SDK 任务启动失败，回退 CLI: {error}");
                        }
                    }
                }
            }
            Err(error) => {
                eprintln!("[codex-sdk] 读取配置失败，回退 CLI: {error}");
            }
        }
    }

    let command = new_codex_command()
        .await
        .map_err(|error| format!("Failed to spawn codex: {error}"))?;
    let cli_json_output_flag = probe_local_exec_json_support().await.unwrap_or(None);
    Ok(SessionLaunch {
        provider: CodexExecutionProvider::Cli,
        command,
        cleanup_paths: Vec::new(),
        cli_json_output_flag,
        session_lookup_started_at: Some(SystemTime::now()),
        sdk_codex_path_override: None,
        ssh_config_name: None,
        ssh_host: None,
        ssh_config_for_artifact_capture: None,
        remote_sdk_fallback_error: None,
        remote_sdk_fallback_logged: false,
    })
}

pub(super) async fn prepare_session_launch(
    app: &AppHandle,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: CodexSessionKind,
    execution_context: &ExecutionContext,
    model: &str,
    reasoning_effort: &str,
    run_cwd: &str,
    image_paths: &[String],
    resume_session_id: Option<&str>,
) -> Result<SessionLaunch, String> {
    if execution_context.execution_target == EXECUTION_TARGET_SSH {
        prepare_remote_session_launch(
            app,
            pool,
            session_record_id,
            employee_id,
            task_id,
            session_kind,
            execution_context,
            model,
            reasoning_effort,
            run_cwd,
            image_paths,
            resume_session_id,
        )
        .await
    } else {
        prepare_local_session_launch(app, run_cwd).await
    }
}

pub(super) async fn capture_session_execution_change_baseline(
    app: &AppHandle,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: CodexSessionKind,
    execution_context: &ExecutionContext,
    run_cwd: &str,
    ssh_config_for_artifact_capture: Option<&SshConfigRecord>,
) -> Result<Option<ExecutionChangeBaseline>, String> {
    if !should_capture_execution_change_baseline(session_kind, &execution_context.execution_target)
    {
        return Ok(None);
    }

    if execution_context.execution_target == EXECUTION_TARGET_SSH {
        emit_session_terminal_line(
            app,
            pool,
            session_record_id,
            employee_id,
            task_id,
            session_kind,
            "[SSH] 正在采集远程仓库基线，用于展示本次会话改动...".to_string(),
        )
        .await;
    }

    let baseline_result = if execution_context.execution_target == EXECUTION_TARGET_SSH {
        let ssh_config = ssh_config_for_artifact_capture
            .ok_or_else(|| "SSH 会话缺少 SSH 配置，无法采集远程文件基线".to_string());
        match ssh_config {
            Ok(ssh_config) => {
                capture_remote_execution_change_baseline(app, ssh_config, run_cwd).await
            }
            Err(error) => Err(error),
        }
    } else {
        capture_execution_change_baseline(run_cwd)
    };

    match baseline_result {
        Ok(baseline) => {
            if execution_context.execution_target == EXECUTION_TARGET_SSH {
                emit_session_terminal_line(
                    app,
                    pool,
                    session_record_id,
                    employee_id,
                    task_id,
                    session_kind,
                    "[SSH] 远程仓库基线采集完成。".to_string(),
                )
                .await;
            }
            Ok(Some(baseline))
        }
        Err(error) => {
            insert_codex_session_event(
                pool,
                session_record_id,
                "session_file_changes_baseline_failed",
                Some(&error),
            )
            .await?;
            let _ = app.emit(
                "codex-stdout",
                CodexOutput {
                    employee_id: employee_id.to_string(),
                    task_id: task_id.map(str::to_string),
                    session_kind: session_kind.as_str().to_string(),
                    session_record_id: session_record_id.to_string(),
                    session_event_id: None,
                    line: format!(
                        "[WARN] 执行会话文件基线采集失败，文件详情将退化为最佳努力快照: {error}"
                    ),
                },
            );
            Ok(None)
        }
    }
}

pub(super) async fn emit_session_launch_diagnostics(
    app: &AppHandle,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: CodexSessionKind,
    provider: CodexExecutionProvider,
    execution_context: &ExecutionContext,
    run_cwd: &str,
    model: &str,
    reasoning_effort: &str,
    prompt: &str,
    image_paths: &[String],
    missing_image_paths: &[String],
    ignored_remote_image_count: usize,
    ssh_config_name: Option<&str>,
    ssh_host: Option<&str>,
    remote_sdk_fallback_error: Option<&str>,
    remote_sdk_fallback_logged: bool,
    cli_json_output_flag: Option<CliJsonOutputFlag>,
) {
    for missing_path in missing_image_paths {
        let _ = app.emit(
            "codex-stdout",
            CodexOutput {
                employee_id: employee_id.to_string(),
                task_id: task_id.map(str::to_string),
                session_kind: session_kind.as_str().to_string(),
                session_record_id: session_record_id.to_string(),
                session_event_id: None,
                line: format!("[WARN] 附件图片不存在，已跳过: {}", missing_path),
            },
        );
    }

    if ignored_remote_image_count > 0 {
        let _ = app.emit(
            "codex-stdout",
            CodexOutput {
                employee_id: employee_id.to_string(),
                task_id: task_id.map(str::to_string),
                session_kind: session_kind.as_str().to_string(),
                session_record_id: session_record_id.to_string(),
                session_event_id: None,
                line: format!(
                    "[WARN] SSH 远程运行暂不传输本地图片附件，已忽略 {} 张图片。",
                    ignored_remote_image_count
                ),
            },
        );
    }

    if let Some(error) = remote_sdk_fallback_error.filter(|_| !remote_sdk_fallback_logged) {
        let _ = app.emit(
            "codex-stdout",
            CodexOutput {
                employee_id: employee_id.to_string(),
                task_id: task_id.map(str::to_string),
                session_kind: session_kind.as_str().to_string(),
                session_record_id: session_record_id.to_string(),
                session_event_id: None,
                line: format!("[WARN] 远程 SDK 启动失败，已回退到远程 codex exec: {error}"),
            },
        );
    }

    if execution_context.execution_target == EXECUTION_TARGET_SSH {
        if provider == CodexExecutionProvider::Cli {
            emit_session_terminal_line(
                app,
                pool,
                session_record_id,
                employee_id,
                task_id,
                session_kind,
                if cli_json_output_flag.is_some() {
                    "[SSH] 远程 codex exec 已启用 JSON 事件流，将实时回传执行日志。"
                        .to_string()
                } else {
                    "[WARN] [SSH] 远程 codex exec 不支持 JSON 事件流，终端可能只能显示最终输出；建议升级远程 Codex CLI 或启用远程 SDK。".to_string()
                },
            )
            .await;
        }
        emit_session_terminal_line(
            app,
            pool,
            session_record_id,
            employee_id,
            task_id,
            session_kind,
            "[SSH] 远程命令已准备完成，正在启动 Codex 会话...".to_string(),
        )
        .await;
    }

    let _ = app.emit(
        "codex-stdout",
        CodexOutput {
            employee_id: employee_id.to_string(),
            task_id: task_id.map(str::to_string),
            session_kind: session_kind.as_str().to_string(),
            session_record_id: session_record_id.to_string(),
            session_event_id: None,
            line: format_session_prompt_log(
                provider,
                model,
                reasoning_effort,
                &execution_context.execution_target,
                ssh_config_name,
                ssh_host,
                execution_context.target_host_label.as_deref(),
                run_cwd,
                prompt,
                image_paths,
            ),
        },
    );
}
