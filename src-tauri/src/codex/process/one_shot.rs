use super::*;

const DEFAULT_CLAUDE_ONE_SHOT_MODEL: &str = "claude-sonnet-4-6";
const DEFAULT_CLAUDE_ONE_SHOT_REASONING_EFFORT: &str = "high";
const DEFAULT_OPENCODE_ONE_SHOT_MODEL: &str = "openai/gpt-4o";
const DEFAULT_OPENCODE_ONE_SHOT_REASONING_EFFORT: &str = "high";
const SUPPORTED_CLAUDE_ONE_SHOT_MODELS: &[&str] = &[
    "claude-opus-4-7",
    "claude-opus-4-7[1m]",
    "claude-opus-4-6[1m]",
    "claude-sonnet-4-6",
    "claude-sonnet-4-6[1m]",
    "claude-haiku-4-5",
];
const SUPPORTED_CLAUDE_ONE_SHOT_REASONING_EFFORTS: &[&str] =
    &["low", "medium", "high", "xhigh", "max", "auto"];
const SUPPORTED_OPENCODE_ONE_SHOT_REASONING_EFFORTS: &[&str] =
    &["default", "low", "medium", "high", "xhigh", "max"];

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

fn normalize_one_shot_model_for_provider(provider: &str, value: Option<&str>) -> String {
    match provider {
        "claude" => match value.map(str::trim) {
            Some(value) if SUPPORTED_CLAUDE_ONE_SHOT_MODELS.contains(&value) => value.to_string(),
            _ => DEFAULT_CLAUDE_ONE_SHOT_MODEL.to_string(),
        },
        "opencode" => value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| DEFAULT_OPENCODE_ONE_SHOT_MODEL.to_string()),
        _ => normalize_model(value).to_string(),
    }
}

fn normalize_one_shot_reasoning_for_provider(provider: &str, value: Option<&str>) -> String {
    match provider {
        "claude" => match value.map(str::trim) {
            Some(value) if SUPPORTED_CLAUDE_ONE_SHOT_REASONING_EFFORTS.contains(&value) => {
                value.to_string()
            }
            _ => DEFAULT_CLAUDE_ONE_SHOT_REASONING_EFFORT.to_string(),
        },
        "opencode" => match value.map(str::trim) {
            Some(value) if SUPPORTED_OPENCODE_ONE_SHOT_REASONING_EFFORTS.contains(&value) => {
                value.to_string()
            }
            _ => DEFAULT_OPENCODE_ONE_SHOT_REASONING_EFFORT.to_string(),
        },
        _ => normalize_reasoning_effort(value).to_string(),
    }
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

async fn run_ai_command_via_remote_sdk<R: Runtime>(
    app: &AppHandle<R>,
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

async fn run_ai_command_via_ssh_exec<R: Runtime>(
    app: &AppHandle<R>,
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

async fn run_ai_command_via_sdk<R: Runtime>(
    app: &AppHandle<R>,
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

fn build_claude_one_shot_cli_args(model: &str, effort: &str) -> Vec<String> {
    let mut args = vec!["-p".to_string(), "--model".to_string(), model.to_string()];
    if effort != "auto" {
        args.push("--effort".to_string());
        args.push(effort.to_string());
    }
    args.push("--permission-mode".to_string());
    args.push("bypassPermissions".to_string());
    args
}

fn resolve_claude_binary_path(
    settings: &crate::db::models::ClaudeSettings,
) -> Result<PathBuf, String> {
    if let Some(cli_path_override) = settings
        .cli_path_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(PathBuf::from(cli_path_override));
    }

    let install_dir = PathBuf::from(&settings.sdk_install_dir);
    let bin_name = if cfg!(target_os = "windows") {
        "claude.exe"
    } else {
        "claude"
    };
    let pkg_bin = install_dir
        .join("node_modules")
        .join("@anthropic-ai")
        .join("claude-code")
        .join("bin")
        .join(bin_name);
    if pkg_bin.exists() {
        return Ok(pkg_bin);
    }

    Ok(PathBuf::from("claude"))
}

async fn run_claude_one_shot_via_sdk<R: Runtime>(
    app: &AppHandle<R>,
    prompt: &str,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
) -> Result<String, String> {
    let claude_settings = crate::claude::load_claude_settings(app)?;
    let install_dir = PathBuf::from(&claude_settings.sdk_install_dir);
    crate::claude::ensure_claude_sdk_runtime_layout(&install_dir)?;
    let bridge_path = crate::claude::sdk_bridge_script_path(&install_dir);
    if !bridge_path.exists() {
        return Err("Claude SDK bridge 脚本不存在，请在设置中重新安装 SDK".to_string());
    }

    let mut command = new_node_command(claude_settings.node_path_override.as_deref()).await?;
    let claude_path_override = resolve_claude_binary_path(&claude_settings)
        .ok()
        .map(|path| path.to_string_lossy().to_string());
    if let Some(ref claude_path_override) = claude_path_override {
        command.env("CLAUDE_CLI_PATH", claude_path_override);
    }
    command
        .arg(&bridge_path)
        .current_dir(&install_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to spawn Claude SDK bridge: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(&serde_json::json!({
            "mode": "one_shot",
            "prompt": prompt,
            "model": model,
            "effort": reasoning_effort,
            "workingDirectory": working_dir,
            "imagePaths": image_paths,
            "claudePathOverride": claude_path_override,
        }))
        .map_err(|error| format!("Failed to serialize Claude SDK request: {error}"))?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|error| format!("Failed to write Claude SDK request: {error}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|error| format!("Failed to close Claude SDK request stdin: {error}"))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("Failed to wait for Claude SDK bridge: {error}"))?;
    parse_sdk_bridge_output(&output.stdout, &output.stderr)
}

async fn run_claude_one_shot_via_cli<R: Runtime>(
    app: &AppHandle<R>,
    prompt: String,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
) -> Result<String, String> {
    let claude_settings = crate::claude::load_claude_settings(app)?;
    let claude_bin = resolve_claude_binary_path(&claude_settings)?;
    let mut command = tokio::process::Command::new(&claude_bin);
    command
        .args(build_claude_one_shot_cli_args(model, reasoning_effort))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(run_cwd) = working_dir.map(str::trim).filter(|value| !value.is_empty()) {
        command.current_dir(run_cwd);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to spawn Claude CLI: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|error| format!("Failed to write Claude CLI prompt: {error}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|error| format!("Failed to close Claude CLI stdin: {error}"))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("Failed to wait for Claude CLI: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "Claude CLI 调用失败".to_string()
        } else {
            format!("Claude CLI 调用失败：{stderr}")
        })
    }
}

async fn run_claude_one_shot_via_remote_cli<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    prompt: String,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
) -> Result<String, String> {
    let ssh_config = fetch_ssh_config_record_by_id(&sqlite_pool(app).await?, ssh_config_id).await?;
    let run_cwd = working_dir
        .map(normalize_runtime_path_string)
        .ok_or_else(|| "SSH 一次性 AI 缺少远程工作目录".to_string())?;
    let remote_args = build_claude_one_shot_cli_args(model, reasoning_effort)
        .into_iter()
        .map(|value| crate::app::shell_escape_single_quoted(&value))
        .collect::<Vec<_>>()
        .join(" ");
    let remote_command = build_remote_shell_command(
        &format!(
            "cd {} && exec claude {}",
            remote_shell_path_expression(&run_cwd),
            remote_args
        ),
        None,
    );
    let output = crate::app::execute_ssh_command_with_input(
        app,
        &ssh_config,
        &remote_command,
        prompt.as_bytes(),
        true,
    )
    .await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "远端 Claude CLI 调用失败".to_string()
        } else {
            format!(
                "远端 Claude CLI 调用失败：{}",
                crate::app::redact_secret_text(&stderr)
            )
        })
    }
}

fn parse_opencode_one_shot_output(stdout: &[u8], stderr: &[u8]) -> Result<String, String> {
    let mut output_lines = Vec::new();
    let mut error_lines = Vec::new();

    for line in String::from_utf8_lossy(stdout).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(event) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        let event_type = event
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let data = event.get("data").cloned().unwrap_or_default();

        match event_type {
            "stdout" => {
                if let Some(raw_line) = data.get("line").and_then(|value| value.as_str()) {
                    let cleaned = raw_line
                        .strip_prefix("[OUTPUT] ")
                        .unwrap_or(raw_line)
                        .trim();
                    if !cleaned.is_empty() {
                        output_lines.push(cleaned.to_string());
                    }
                }
            }
            "error" => {
                if let Some(message) = data.get("message").and_then(|value| value.as_str()) {
                    let cleaned = message.trim();
                    if !cleaned.is_empty() {
                        error_lines.push(cleaned.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    if !error_lines.is_empty() {
        return Err(format!("OpenCode SDK 调用失败：{}", error_lines.join("；")));
    }

    if !output_lines.is_empty() {
        return Ok(output_lines.join("\n"));
    }

    let stderr_text = String::from_utf8_lossy(stderr).trim().to_string();
    if !stderr_text.is_empty() {
        return Err(format!("OpenCode SDK 调用失败：{stderr_text}"));
    }

    Err("OpenCode SDK 未返回文本内容".to_string())
}

async fn run_opencode_one_shot_via_sdk<R: Runtime>(
    app: &AppHandle<R>,
    prompt: &str,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
) -> Result<String, String> {
    let opencode_settings = crate::opencode::load_opencode_settings(app)?;
    let install_dir = PathBuf::from(&opencode_settings.sdk_install_dir);
    crate::opencode::ensure_opencode_sdk_runtime_layout(&install_dir)?;
    let bridge_path = crate::opencode::sdk_bridge_script_path(&install_dir);
    if !bridge_path.exists() {
        return Err("OpenCode SDK bridge 脚本不存在，请在设置中重新安装 SDK".to_string());
    }

    let runtime_config_backup =
        if let Some(run_cwd) = working_dir.map(str::trim).filter(|value| !value.is_empty()) {
            let provider_id = model
                .split_once('/')
                .map(|(provider_id, _)| provider_id)
                .unwrap_or("opencode-go");
            let model_id = model
                .split_once('/')
                .map(|(_, model_id)| model_id)
                .unwrap_or(model);
            let effort_to_write = reasoning_effort.trim();
            let effort_to_write = (!effort_to_write.is_empty() && effort_to_write != "default")
                .then_some(effort_to_write);
            Some(crate::opencode::write_opencode_runtime_config_file(
                run_cwd,
                provider_id,
                model_id,
                effort_to_write,
            )?)
        } else {
            None
        };

    let mut command = new_node_command(opencode_settings.node_path_override.as_deref()).await?;
    command
        .arg(&bridge_path)
        .current_dir(&install_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command.spawn().map_err(|error| {
        if let Some(ref backup) = runtime_config_backup {
            let _ = backup.restore();
        }
        format!("Failed to spawn OpenCode SDK bridge: {error}")
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(&serde_json::json!({
            "mode": "one_shot",
            "prompt": prompt,
            "model": model,
            "reasoningEffort": reasoning_effort,
            "host": opencode_settings.host,
            "port": opencode_settings.port,
            "workingDirectory": working_dir,
            "imagePaths": image_paths,
        }))
        .map_err(|error| format!("Failed to serialize OpenCode SDK request: {error}"))?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|error| format!("Failed to write OpenCode SDK request: {error}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|error| format!("Failed to close OpenCode SDK request stdin: {error}"))?;
    }

    let output = child.wait_with_output().await.map_err(|error| {
        if let Some(ref backup) = runtime_config_backup {
            let _ = backup.restore();
        }
        format!("Failed to wait for OpenCode SDK bridge: {error}")
    })?;
    let parse_result = parse_opencode_one_shot_output(&output.stdout, &output.stderr);
    if let Some(backup) = runtime_config_backup {
        if let Err(error) = backup.restore() {
            return match parse_result {
                Ok(_) => Err(error),
                Err(parse_error) => Err(format!("{parse_error}；同时{error}")),
            };
        }
    }
    parse_result
}

pub(super) async fn run_ai_command<R: Runtime>(
    app: &AppHandle<R>,
    prompt: String,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    project_id: Option<String>,
    working_dir: Option<String>,
    model_override: Option<String>,
    reasoning_effort_override: Option<String>,
) -> Result<String, String> {
    let execution_context = match task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(task_id) => resolve_task_project_execution_context(app, task_id).await?,
        None => match project_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(project_id) => resolve_project_execution_context(app, project_id).await?,
            None => ExecutionContext::local_default(),
        },
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
    let mut one_shot_provider = "codex".to_string();
    let mut one_shot_model = normalize_model(None).to_string();
    let mut one_shot_reasoning_effort = normalize_reasoning_effort(None).to_string();
    let mut one_shot_sdk_enabled = false;

    for missing_path in &missing_image_paths {
        eprintln!("[codex-sdk] one-shot 附件图片不存在，已跳过: {missing_path}");
    }

    let working_dir = resolve_one_shot_working_dir(
        app,
        task_id.as_deref(),
        project_id.as_deref(),
        working_dir.as_deref(),
    )
    .await?;

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
        one_shot_provider = settings.one_shot_preferred_provider.clone();
        one_shot_model = settings.one_shot_model.clone();
        one_shot_reasoning_effort = settings.one_shot_reasoning_effort.clone();
        one_shot_sdk_enabled = settings.one_shot_sdk_enabled;
    }

    if let Some(model_override) = model_override.as_deref() {
        one_shot_model =
            normalize_one_shot_model_for_provider(&one_shot_provider, Some(model_override));
    }
    if let Some(reasoning_effort_override) = reasoning_effort_override.as_deref() {
        one_shot_reasoning_effort = normalize_one_shot_reasoning_for_provider(
            &one_shot_provider,
            Some(reasoning_effort_override),
        );
    }

    match (
        execution_context.execution_target.as_str(),
        one_shot_provider.as_str(),
    ) {
        (EXECUTION_TARGET_LOCAL, "claude") => {
            let claude_settings = crate::claude::load_claude_settings(app)?;
            let claude_health =
                crate::claude::inspect_claude_sdk_runtime(app, &claude_settings).await;
            let mut sdk_error = None;
            if one_shot_sdk_enabled && claude_health.effective_provider == "sdk" {
                match run_claude_one_shot_via_sdk(
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
                        eprintln!("[claude-sdk] 调用失败，回退到 Claude CLI: {error}");
                        sdk_error = Some(error);
                    }
                }
            }

            match run_claude_one_shot_via_cli(
                app,
                prompt,
                &one_shot_model,
                &one_shot_reasoning_effort,
                working_dir.as_deref(),
            )
            .await
            {
                Ok(result) => Ok(result),
                Err(cli_error) => match sdk_error {
                    Some(sdk_error) => Err(format!(
                        "Claude SDK 调用失败后回退 CLI 也失败：SDK: {sdk_error}; CLI: {cli_error}"
                    )),
                    None => Err(cli_error),
                },
            }
        }
        (EXECUTION_TARGET_SSH, "claude") => {
            let ssh_config_id = execution_context
                .ssh_config_id
                .as_deref()
                .ok_or_else(|| "SSH 一次性 AI 缺少 ssh_config_id".to_string())?;
            run_claude_one_shot_via_remote_cli(
                app,
                ssh_config_id,
                prompt,
                &one_shot_model,
                &one_shot_reasoning_effort,
                working_dir.as_deref(),
            )
            .await
        }
        (EXECUTION_TARGET_LOCAL, "opencode") => {
            if !one_shot_sdk_enabled {
                return Err("一次性 AI 未启用 OpenCode SDK，当前不可用".to_string());
            }
            let opencode_settings = crate::opencode::load_opencode_settings(app)?;
            let opencode_health =
                crate::opencode::inspect_opencode_sdk_runtime(app, &opencode_settings).await;
            if opencode_health.effective_provider != "sdk" {
                return Err(opencode_health.sdk_status_message);
            }
            run_opencode_one_shot_via_sdk(
                app,
                &prompt,
                &one_shot_model,
                &one_shot_reasoning_effort,
                working_dir.as_deref(),
                &image_paths,
            )
            .await
        }
        (EXECUTION_TARGET_SSH, "opencode") => {
            Err("SSH 模式下暂不支持 OpenCode 一次性 AI".to_string())
        }
        (EXECUTION_TARGET_SSH, _) => {
            let ssh_config_id = execution_context
                .ssh_config_id
                .as_deref()
                .ok_or_else(|| "SSH 一次性 AI 缺少 ssh_config_id".to_string())?;
            if one_shot_sdk_enabled {
                if let Some(remote_settings) = settings.as_ref() {
                    let ssh_config =
                        fetch_ssh_config_record_by_id(&sqlite_pool(app).await?, ssh_config_id)
                            .await?;
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

            run_ai_command_via_ssh_exec(
                app,
                ssh_config_id,
                prompt,
                &one_shot_model,
                &one_shot_reasoning_effort,
                working_dir.as_deref(),
                &image_paths,
            )
            .await
        }
        _ => {
            let mut sdk_error = None;
            if one_shot_sdk_enabled {
                let codex_settings = match settings.as_ref() {
                    Some(settings) => settings.clone(),
                    None => load_codex_settings(app)?,
                };
                let runtime = inspect_sdk_runtime(app, &codex_settings).await;
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
                }
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
    }
}
