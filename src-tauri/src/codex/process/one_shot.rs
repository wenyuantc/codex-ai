use super::*;

pub(super) fn build_sdk_input_items(
    prompt: &str,
    image_paths: &[String],
) -> Vec<serde_json::Value> {
    let mut items = vec![serde_json::json!({
        "type": "text",
        "text": prompt,
    })];

    for path in image_paths {
        items.push(serde_json::json!({
            "type": "local_image",
            "path": path,
        }));
    }

    items
}

async fn run_ai_command_via_exec(
    prompt: String,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
) -> Result<String, String> {
    let mut cmd = new_codex_command()
        .await
        .map_err(|error| format!("Failed to spawn codex exec: {}", error))?;
    let mut child = cmd
        .args(build_one_shot_exec_args(
            model,
            reasoning_effort,
            working_dir,
            image_paths,
        ))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e: std::io::Error| format!("Failed to spawn codex exec: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|error| format!("Failed to write codex exec prompt: {}", error))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("Failed to wait for codex exec: {}", error))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("codex exec failed: {}", stderr.trim()))
    }
}

async fn run_ai_command_via_remote_sdk(
    app: &AppHandle,
    ssh_config_id: &str,
    prompt: &str,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
) -> Result<String, String> {
    let ssh_config = fetch_ssh_config_record_by_id(&sqlite_pool(app).await?, ssh_config_id).await?;
    let remote_settings = ensure_remote_sdk_runtime_layout(app, ssh_config_id).await?;
    let remote_command = build_remote_sdk_bridge_command(
        &remote_settings.sdk_install_dir,
        remote_settings.node_path_override.as_deref(),
    );
    let (mut command, askpass_path) =
        build_ssh_command(app, &ssh_config, Some(&remote_command), true, false).await?;
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to spawn remote Codex SDK bridge: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(&serde_json::json!({
            "prompt": prompt,
            "input": build_sdk_input_items(prompt, image_paths),
            "model": model,
            "modelReasoningEffort": reasoning_effort,
            "workingDirectory": working_dir,
        }))
        .map_err(|error| format!("Failed to serialize remote SDK request: {}", error))?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|error| format!("Failed to write remote SDK request: {}", error))?;
        stdin
            .shutdown()
            .await
            .map_err(|error| format!("Failed to close remote SDK request stdin: {}", error))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("Failed to wait for remote Codex SDK bridge: {}", error))?;
    if let Some(path) = askpass_path {
        let _ = fs::remove_file(path);
    }

    parse_sdk_bridge_output(&output.stdout, &output.stderr)
}

async fn run_ai_command_via_ssh_exec(
    app: &AppHandle,
    ssh_config_id: &str,
    prompt: String,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
) -> Result<String, String> {
    let ssh_config = fetch_ssh_config_record_by_id(&sqlite_pool(app).await?, ssh_config_id).await?;
    let remote_settings = load_remote_codex_settings(app, ssh_config_id).ok();
    let run_cwd = working_dir
        .map(normalize_runtime_path_string)
        .ok_or_else(|| "SSH 一次性 AI 缺少远程工作目录".to_string())?;
    let remote_command =
        build_one_shot_exec_args(model, reasoning_effort, Some(&run_cwd), image_paths)
            .into_iter()
            .map(|value| shell_escape_arg(&value))
            .collect::<Vec<_>>()
            .join(" ");
    let remote_command = build_remote_shell_command(
        &format!("exec codex {remote_command}"),
        remote_settings
            .as_ref()
            .and_then(|settings| settings.node_path_override.as_deref()),
    );
    let (mut command, askpass_path) =
        build_ssh_command(app, &ssh_config, Some(&remote_command), true, false).await?;
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to spawn remote codex exec: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|error| format!("Failed to write remote codex exec prompt: {error}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|error| format!("Failed to close remote codex exec stdin: {error}"))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("Failed to wait for remote codex exec: {error}"))?;
    if let Some(path) = askpass_path {
        let _ = fs::remove_file(path);
    }

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("remote codex exec failed: {}", stderr.trim()))
    }
}

async fn run_ai_command_via_sdk(
    app: &AppHandle,
    prompt: &str,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
) -> Result<String, String> {
    let settings = load_codex_settings(app)?;
    let install_dir = PathBuf::from(&settings.sdk_install_dir);
    ensure_sdk_runtime_layout(&install_dir)?;
    let bridge_path = sdk_bridge_script_path(&install_dir);
    if !bridge_path.exists() {
        return Err("Codex SDK bridge 脚本不存在，请在设置中重新安装 SDK".to_string());
    }

    let mut command = new_node_command(settings.node_path_override.as_deref()).await?;
    let codex_path_override = resolve_codex_executable_path()
        .await
        .ok()
        .and_then(|path| sdk_codex_path_override_from_resolved_path(&path));
    if let Some(ref codex_path_override) = codex_path_override {
        command.env("CODEX_CLI_PATH", codex_path_override);
    }
    command
        .arg(&bridge_path)
        .current_dir(&install_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to spawn Codex SDK bridge: {}", error))?;
    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(&serde_json::json!({
            "prompt": prompt,
            "input": build_sdk_input_items(prompt, image_paths),
            "model": model,
            "modelReasoningEffort": reasoning_effort,
            "workingDirectory": working_dir,
            "codexPathOverride": codex_path_override,
        }))
        .map_err(|error| format!("Failed to serialize SDK request: {}", error))?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|error| format!("Failed to write SDK request: {}", error))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("Failed to wait for Codex SDK bridge: {}", error))?;
    parse_sdk_bridge_output(&output.stdout, &output.stderr)
}

pub(super) async fn run_ai_command(
    app: &AppHandle,
    prompt: String,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    working_dir: Option<String>,
) -> Result<String, String> {
    let execution_context = match task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(task_id) => resolve_task_project_execution_context(app, task_id).await?,
        None => ExecutionContext::local_default(),
    };
    let (image_paths, missing_image_paths, _ignored_remote_image_count) =
        prepare_execution_image_paths(
            app,
            task_id.as_deref(),
            &execution_context.execution_target,
            execution_context.ssh_config_id.as_deref(),
            image_paths,
        )
        .await?;
    let mut one_shot_model = normalize_model(None).to_string();
    let mut one_shot_reasoning_effort = normalize_reasoning_effort(None).to_string();
    let mut sdk_error = None;

    for missing_path in &missing_image_paths {
        eprintln!("[codex-sdk] one-shot 附件图片不存在，已跳过: {missing_path}");
    }

    let working_dir =
        resolve_one_shot_working_dir(app, task_id.as_deref(), working_dir.as_deref()).await?;

    let settings = if execution_context.execution_target == EXECUTION_TARGET_SSH {
        execution_context
            .ssh_config_id
            .as_deref()
            .map(|ssh_config_id| load_remote_codex_settings(app, ssh_config_id))
            .transpose()?
            .or_else(|| load_codex_settings(app).ok())
    } else {
        load_codex_settings(app).ok()
    };

    if let Some(ref settings) = settings {
        one_shot_model = normalize_model(Some(&settings.one_shot_model)).to_string();
        one_shot_reasoning_effort =
            normalize_reasoning_effort(Some(&settings.one_shot_reasoning_effort)).to_string();
        if execution_context.execution_target == EXECUTION_TARGET_LOCAL
            && settings.one_shot_sdk_enabled
        {
            let runtime = inspect_sdk_runtime(app, &settings).await;
            if runtime.one_shot_effective_provider == "sdk" {
                match run_ai_command_via_sdk(
                    app,
                    &prompt,
                    &one_shot_model,
                    &one_shot_reasoning_effort,
                    working_dir.as_deref(),
                    &image_paths,
                )
                .await
                {
                    Ok(result) => return Ok(result),
                    Err(error) => {
                        eprintln!("[codex-sdk] 调用失败，回退到 codex exec: {error}");
                        sdk_error = Some(error);
                    }
                }
            } else {
                eprintln!("[codex-sdk] {}", runtime.status_message);
            }
        }
    }

    if execution_context.execution_target == EXECUTION_TARGET_SSH {
        let ssh_config_id = execution_context
            .ssh_config_id
            .as_deref()
            .ok_or_else(|| "SSH 一次性 AI 缺少 ssh_config_id".to_string())?;
        if settings
            .as_ref()
            .map(|settings| settings.one_shot_sdk_enabled)
            .unwrap_or(false)
        {
            if let Some(remote_settings) = settings.as_ref() {
                let ssh_config =
                    fetch_ssh_config_record_by_id(&sqlite_pool(app).await?, ssh_config_id).await?;
                match inspect_remote_codex_runtime(app, &ssh_config, remote_settings).await {
                    Ok(runtime) if runtime.one_shot_effective_provider == "sdk" => {
                        match run_ai_command_via_remote_sdk(
                            app,
                            ssh_config_id,
                            &prompt,
                            &one_shot_model,
                            &one_shot_reasoning_effort,
                            working_dir.as_deref(),
                            &image_paths,
                        )
                        .await
                        {
                            Ok(result) => return Ok(result),
                            Err(error) => {
                                eprintln!(
                                    "[codex-sdk] 远程 SDK 调用失败，回退到 remote codex exec: {error}"
                                );
                            }
                        }
                    }
                    Ok(runtime) => {
                        eprintln!("[codex-sdk] {}", runtime.status_message);
                    }
                    Err(error) => {
                        eprintln!(
                            "[codex-sdk] 远程 SDK 预检失败，回退到 remote codex exec: {error}"
                        );
                    }
                }
            }
        }

        return run_ai_command_via_ssh_exec(
            app,
            ssh_config_id,
            prompt,
            &one_shot_model,
            &one_shot_reasoning_effort,
            working_dir.as_deref(),
            &image_paths,
        )
        .await;
    }

    match run_ai_command_via_exec(
        prompt,
        &one_shot_model,
        &one_shot_reasoning_effort,
        working_dir.as_deref(),
        &image_paths,
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(exec_error) => match sdk_error {
            Some(sdk_error) => Err(format!(
                "Codex SDK 调用失败后回退 exec 也失败：SDK: {sdk_error}; exec: {exec_error}"
            )),
            None => Err(exec_error),
        },
    }
}
