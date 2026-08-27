use super::*;

const DEFAULT_CLAUDE_ONE_SHOT_REASONING_EFFORT: &str = "high";
const DEFAULT_GROK_ONE_SHOT_REASONING_EFFORT: &str = "high";
const DEFAULT_OPENCODE_ONE_SHOT_MODEL: &str = "openai/gpt-4o";
const DEFAULT_OPENCODE_ONE_SHOT_REASONING_EFFORT: &str = "high";
const SUPPORTED_CLAUDE_ONE_SHOT_REASONING_EFFORTS: &[&str] =
    &["low", "medium", "high", "xhigh", "max", "auto"];
const SUPPORTED_GROK_ONE_SHOT_REASONING_EFFORTS: &[&str] = &["low", "medium", "high"];
const SUPPORTED_OPENCODE_ONE_SHOT_REASONING_EFFORTS: &[&str] = &["low", "medium", "high"];

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
        "claude" => crate::claude::normalize_claude_model(value),
        "grok" => crate::grok::normalize_grok_model(value),
        "opencode" => value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| DEFAULT_OPENCODE_ONE_SHOT_MODEL.to_string()),
        "native" => value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "default".to_string()),
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
        "grok" => match value.map(str::trim) {
            Some(value) if SUPPORTED_GROK_ONE_SHOT_REASONING_EFFORTS.contains(&value) => {
                value.to_string()
            }
            _ => DEFAULT_GROK_ONE_SHOT_REASONING_EFFORT.to_string(),
        },
        "opencode" => match value.map(str::trim) {
            Some(value) if SUPPORTED_OPENCODE_ONE_SHOT_REASONING_EFFORTS.contains(&value) => {
                value.to_string()
            }
            _ => DEFAULT_OPENCODE_ONE_SHOT_REASONING_EFFORT.to_string(),
        },
        "native" => match value.map(str::trim) {
            Some(value)
                if crate::codex::settings::SUPPORTED_NATIVE_REASONING_EFFORTS.contains(&value) =>
            {
                value.to_string()
            }
            _ => "high".to_string(),
        },
        _ => normalize_reasoning_effort(value).to_string(),
    }
}

fn normalize_one_shot_provider_for_target(value: Option<&str>, _execution_target: &str) -> String {
    match value.map(str::trim) {
        Some("claude") => "claude".to_string(),
        Some("grok") => "grok".to_string(),
        Some("opencode") => "opencode".to_string(),
        Some("native") => "native".to_string(),
        Some("codex") => "codex".to_string(),
        _ => "codex".to_string(),
    }
}

pub(crate) struct AiCommandResult {
    pub text: String,
    pub usage_line: Option<String>,
}

impl AiCommandResult {
    fn text_only(text: String) -> Self {
        Self {
            text,
            usage_line: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AiCommandOptions {
    pub progress_request_id: Option<String>,
    pub task_id_for_progress: Option<String>,
    pub read_only_tools: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
struct AiCommandOutput {
    request_id: String,
    task_id: Option<String>,
    line: String,
}

pub(crate) fn emit_ai_command_line<R: Runtime>(
    app: &AppHandle<R>,
    options: &AiCommandOptions,
    line: impl Into<String>,
) {
    let Some(request_id) = options
        .progress_request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let line = line.into();
    if line.trim().is_empty() {
        return;
    }
    let _ = app.emit(
        "ai-command-stdout",
        AiCommandOutput {
            request_id: request_id.to_string(),
            task_id: options.task_id_for_progress.clone(),
            line,
        },
    );
}

fn ai_command_options_streaming(options: &AiCommandOptions) -> bool {
    options
        .progress_request_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

async fn wait_sdk_bridge_output<R: Runtime>(
    mut child: tokio::process::Child,
    app: &AppHandle<R>,
    options: &AiCommandOptions,
) -> Result<String, String> {
    if !ai_command_options_streaming(options) {
        let output = child
            .wait_with_output()
            .await
            .map_err(|error| format!("Failed to wait for SDK bridge: {error}"))?;
        return parse_sdk_bridge_output(&output.stdout, &output.stderr);
    }

    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "SDK bridge 缺少 stdout".to_string())?;
    let mut stderr = child.stderr.take();
    let mut lines = BufReader::new(stdout).lines();
    let mut result_text = None;
    let mut result_error = None;
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|error| format!("Failed to read SDK bridge stdout: {error}"))?
    {
        if let Ok(parsed) = serde_json::from_str::<SdkBridgeResponse>(&line) {
            if parsed.ok {
                result_text = Some(parsed.text.unwrap_or_default());
            } else {
                result_error = Some(
                    parsed
                        .error
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "SDK 返回了失败响应".to_string()),
                );
            }
            continue;
        }
        if let Some(usage) = parse_sdk_usage_event(&line) {
            if let Some(usage_line) = usage.format_terminal_line() {
                emit_ai_command_line(app, options, usage_line);
            }
            continue;
        }
        if parse_sdk_file_change_event(&line).is_some() {
            continue;
        }
        emit_ai_command_line(app, options, line);
    }
    let status = child
        .wait()
        .await
        .map_err(|error| format!("Failed to wait for SDK bridge: {error}"))?;
    let mut stderr_bytes = Vec::new();
    if let Some(mut err) = stderr.take() {
        let _ = err.read_to_end(&mut stderr_bytes).await;
    }
    if let Some(error) = result_error {
        return Err(error);
    }
    if let Some(text) = result_text {
        return Ok(text.trim().to_string());
    }
    if !status.success() {
        let stderr_text = String::from_utf8_lossy(&stderr_bytes);
        return Err(if stderr_text.trim().is_empty() {
            "SDK bridge 失败".to_string()
        } else {
            stderr_text.trim().to_string()
        });
    }
    Err("Codex SDK 返回空响应".to_string())
}

fn resolve_native_one_shot_employee_id(
    provider_override: Option<&str>,
    employee_id: Option<&str>,
) -> Result<Option<String>, String> {
    if provider_override.map(str::trim) != Some("native") {
        return Ok(None);
    }
    let Some(employee_id) = employee_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err("内置 Agent 一次性调用需要绑定员工，不能回退到 Codex SDK".to_string());
    };
    Ok(Some(employee_id.to_string()))
}

async fn run_ai_command_via_exec<R: Runtime>(
    app: &AppHandle<R>,
    prompt: String,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
    options: &AiCommandOptions,
) -> Result<String, String> {
    let mut args = build_one_shot_exec_args(model, reasoning_effort, working_dir, image_paths);
    let json_flag = if ai_command_options_streaming(options) {
        probe_local_exec_json_support().await.ok().flatten()
    } else {
        None
    };
    if let Some(flag) = json_flag {
        args.push(flag.as_arg().to_string());
    }
    let mut cmd = new_codex_command()
        .await
        .map_err(|error| format!("Failed to spawn codex exec: {}", error))?;
    let mut child = cmd
        .args(args)
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

    if json_flag.is_some() {
        return wait_exec_json_output(child, app, options).await;
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

async fn wait_exec_json_output<R: Runtime>(
    mut child: tokio::process::Child,
    app: &AppHandle<R>,
    options: &AiCommandOptions,
) -> Result<String, String> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "codex exec 缺少 stdout".to_string())?;
    let mut stderr = child.stderr.take();
    let mut lines = BufReader::new(stdout).lines();
    let mut state = CliJsonStreamState::default();
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|error| format!("Failed to read codex exec stdout: {error}"))?
    {
        if let Some(parsed) = parse_cli_json_event_line(&line, &mut state) {
            for item in parsed.lines {
                emit_ai_command_line(app, options, item);
            }
            continue;
        }
        if !line.trim().is_empty() && !line.trim_start().starts_with('{') {
            emit_ai_command_line(app, options, line);
        }
    }
    let status = child
        .wait()
        .await
        .map_err(|error| format!("Failed to wait for codex exec: {error}"))?;
    let mut stderr_bytes = Vec::new();
    if let Some(mut err) = stderr.take() {
        let _ = err.read_to_end(&mut stderr_bytes).await;
    }
    let text = state
        .agent_messages
        .values()
        .max_by_key(|value| value.len())
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    if !status.success() && text.is_empty() {
        let stderr_text = String::from_utf8_lossy(&stderr_bytes);
        return Err(format!("codex exec failed: {}", stderr_text.trim()));
    }
    if text.is_empty() {
        return Err("codex exec 未返回可用内容".to_string());
    }
    Ok(text)
}

async fn run_ai_command_via_remote_sdk<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    prompt: &str,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
    options: &AiCommandOptions,
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
            "streamProgress": ai_command_options_streaming(options),
            "readOnly": options.read_only_tools,
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

    let result = wait_sdk_bridge_output(child, app, options).await;
    if let Some(path) = askpass_path {
        let _ = fs::remove_file(path);
    }
    result
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
    options: &AiCommandOptions,
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
            "streamProgress": ai_command_options_streaming(options),
            "readOnly": options.read_only_tools,
        }))
        .map_err(|error| format!("Failed to serialize SDK request: {}", error))?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|error| format!("Failed to write SDK request: {}", error))?;
    }

    wait_sdk_bridge_output(child, app, options).await
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

async fn resolve_claude_binary_path(
    settings: &crate::db::models::ClaudeSettings,
) -> Result<PathBuf, String> {
    crate::claude::resolve_claude_cli_executable(
        settings.cli_path_override.as_deref(),
        Some(settings.sdk_install_dir.as_str()),
    )
    .await
}

async fn run_claude_one_shot_via_sdk<R: Runtime>(
    app: &AppHandle<R>,
    prompt: &str,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
    options: &AiCommandOptions,
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
        .await
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
            "streamProgress": ai_command_options_streaming(options),
            "readOnly": options.read_only_tools,
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

    wait_sdk_bridge_output(child, app, options).await
}

async fn run_claude_one_shot_via_cli<R: Runtime>(
    app: &AppHandle<R>,
    prompt: String,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
) -> Result<String, String> {
    let claude_settings = crate::claude::load_claude_settings(app)?;
    let claude_bin = resolve_claude_binary_path(&claude_settings).await?;
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

async fn run_grok_one_shot_via_cli<R: Runtime>(
    app: &AppHandle<R>,
    prompt: String,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
) -> Result<String, String> {
    let grok_settings = crate::grok::load_grok_settings(app)?;
    let grok_bin = crate::grok::resolve_grok_executable_path(&grok_settings).await?;
    let mut command = tokio::process::Command::new(&grok_bin);
    let mut args = crate::grok::build_grok_one_shot_cli_args(model, reasoning_effort);
    // prompt 通过 -p 传入，避免依赖 stdin 的 headless 行为差异
    if let Some(p_index) = args.iter().position(|arg| arg == "-p") {
        args.insert(p_index + 1, prompt.clone());
    } else {
        args.insert(0, "-p".to_string());
        args.insert(1, prompt.clone());
    }
    command
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(run_cwd) = working_dir.map(str::trim).filter(|value| !value.is_empty()) {
        command.current_dir(run_cwd);
    }

    let output = command
        .output()
        .await
        .map_err(|error| format!("启动 Grok CLI 失败: {error}"))?;
    if output.status.success() {
        Ok(crate::grok::aggregate_grok_one_shot_output(
            &String::from_utf8_lossy(&output.stdout),
        ))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "Grok CLI 调用失败。请确认已安装 grok 并完成登录（grok login）。".to_string()
        } else if stderr.to_ascii_lowercase().contains("auth")
            || stderr.to_ascii_lowercase().contains("login")
            || stderr.to_ascii_lowercase().contains("unauthor")
        {
            format!("Grok CLI 未认证或登录失效：{stderr}。请执行 `grok login`。")
        } else {
            format!("Grok CLI 调用失败：{stderr}")
        })
    }
}

async fn run_grok_one_shot_via_remote_cli<R: Runtime>(
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
    let mut remote_args = crate::grok::build_grok_one_shot_cli_args(model, reasoning_effort);
    if let Some(p_index) = remote_args.iter().position(|arg| arg == "-p") {
        remote_args.insert(p_index + 1, prompt.clone());
    } else {
        remote_args.insert(0, "-p".to_string());
        remote_args.insert(1, prompt.clone());
    }
    let remote_args = remote_args
        .into_iter()
        .map(|value| crate::app::shell_escape_single_quoted(&value))
        .collect::<Vec<_>>()
        .join(" ");
    let remote_command = build_remote_shell_command(
        &format!(
            "cd {} && exec grok {}",
            remote_shell_path_expression(&run_cwd),
            remote_args
        ),
        None,
    );
    let output = crate::app::execute_ssh_command(app, &ssh_config, &remote_command, true).await?;

    if output.status.success() {
        Ok(crate::grok::aggregate_grok_one_shot_output(
            &String::from_utf8_lossy(&output.stdout),
        ))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "远端 Grok CLI 调用失败。请确认远端已安装 grok 并完成 `grok login`。".to_string()
        } else if stderr.to_ascii_lowercase().contains("auth")
            || stderr.to_ascii_lowercase().contains("login")
            || stderr.to_ascii_lowercase().contains("unauthor")
            || stderr.contains("not found")
            || stderr.contains("No such file")
        {
            format!(
                "远端 Grok CLI 调用失败：{}。请在远程执行 `grok login` 或确认已安装 grok。",
                crate::app::redact_secret_text(&stderr)
            )
        } else {
            format!(
                "远端 Grok CLI 调用失败：{}",
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

async fn run_opencode_one_shot_via_remote_sdk<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    prompt: &str,
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
) -> Result<String, String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let install_dir = default_remote_opencode_sdk_install_dir(ssh_config_id);

    let mut runtime = inspect_remote_opencode_runtime(app, &ssh_config, &install_dir, None).await?;
    if !runtime.available {
        if runtime.node_available && !runtime.sdk_installed {
            crate::app::remote::install_remote_opencode_sdk(app.clone(), ssh_config_id.to_string())
                .await?;
            runtime = inspect_remote_opencode_runtime(app, &ssh_config, &install_dir, None).await?;
        }
        if !runtime.available {
            return Err(runtime.message);
        }
    }

    let install_dir = ensure_remote_opencode_sdk_runtime_layout(app, ssh_config_id).await?;

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
            Some(
                crate::opencode::write_remote_opencode_runtime_config_file(
                    app,
                    &ssh_config,
                    run_cwd,
                    provider_id,
                    model_id,
                    effort_to_write,
                )
                .await?,
            )
        } else {
            None
        };

    let remote_command = build_remote_opencode_sdk_bridge_command(&install_dir, None);
    let (mut command, askpass_path) =
        build_ssh_command(app, &ssh_config, Some(&remote_command), true, false).await?;
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command.spawn().map_err(|error| {
        if let Some(path) = askpass_path.as_ref() {
            let _ = fs::remove_file(path);
        }
        format!("启动远程 OpenCode SDK bridge 失败: {error}")
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(&serde_json::json!({
            "mode": "one_shot",
            "prompt": prompt,
            "model": model,
            "reasoningEffort": reasoning_effort,
            "host": "127.0.0.1",
            "port": 4096,
            "workingDirectory": working_dir,
            "imagePaths": image_paths,
        }))
        .map_err(|error| format!("序列化远程 OpenCode SDK 请求失败: {error}"))?;
        if let Err(error) = stdin.write_all(&payload).await {
            if let Some(path) = askpass_path.as_ref() {
                let _ = fs::remove_file(path);
            }
            if let Some(ref backup) = runtime_config_backup {
                let _ = backup.restore_async(app).await;
            }
            return Err(format!("写入远程 OpenCode SDK 请求失败: {error}"));
        }
        if let Err(error) = stdin.shutdown().await {
            if let Some(path) = askpass_path.as_ref() {
                let _ = fs::remove_file(path);
            }
            if let Some(ref backup) = runtime_config_backup {
                let _ = backup.restore_async(app).await;
            }
            return Err(format!("关闭远程 OpenCode SDK stdin 失败: {error}"));
        }
    }

    let wait_result = child.wait_with_output().await;
    if let Some(path) = askpass_path {
        let _ = fs::remove_file(path);
    }
    let output = match wait_result {
        Ok(output) => output,
        Err(error) => {
            if let Some(ref backup) = runtime_config_backup {
                let _ = backup.restore_async(app).await;
            }
            return Err(format!("等待远程 OpenCode SDK bridge 完成失败: {error}"));
        }
    };

    let parse_result = parse_opencode_one_shot_output(&output.stdout, &output.stderr);
    if let Some(backup) = runtime_config_backup {
        if let Err(error) = backup.restore_async(app).await {
            return match parse_result {
                Ok(_) => Err(error),
                Err(parse_error) => Err(format!("{parse_error}；同时{error}")),
            };
        }
    }
    parse_result
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_native_ai_command(
    app: &AppHandle,
    employee_id: String,
    prompt: String,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    project_id: Option<String>,
    working_dir: Option<String>,
    model_override: Option<String>,
    reasoning_effort_override: Option<String>,
    options: &AiCommandOptions,
) -> Result<AiCommandResult, String> {
    if !options.read_only_tools {
        let shot = crate::native::run_native_one_shot(
            app,
            &employee_id,
            prompt,
            image_paths,
            model_override,
            reasoning_effort_override,
        )
        .await?;
        return Ok(AiCommandResult {
            text: shot.text,
            usage_line: shot.usage_line,
        });
    }

    let working_dir = resolve_one_shot_working_dir(
        app,
        task_id.as_deref(),
        project_id.as_deref(),
        working_dir.as_deref(),
    )
    .await?;
    let Some(run_cwd) = working_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
    else {
        emit_ai_command_line(
            app,
            options,
            "[WARN] 未配置工作目录，规划将不读取仓库。".to_string(),
        );
        let shot = crate::native::run_native_one_shot(
            app,
            &employee_id,
            prompt,
            image_paths,
            model_override,
            reasoning_effort_override,
        )
        .await?;
        return Ok(AiCommandResult {
            text: shot.text,
            usage_line: shot.usage_line,
        });
    };

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

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let app_for_emit = app.clone();
    let options_for_emit = options.clone();
    let drain = tokio::spawn(async move {
        while let Some(line) = event_rx.recv().await {
            emit_ai_command_line(&app_for_emit, &options_for_emit, line);
        }
    });
    let shot_result = crate::native::run_native_read_only_one_shot(
        app,
        &employee_id,
        prompt,
        image_paths,
        model_override,
        reasoning_effort_override,
        &run_cwd,
        &execution_context.execution_target,
        execution_context.ssh_config_id.as_deref(),
        event_tx,
    )
    .await;
    let _ = drain.await;
    let shot = shot_result?;
    Ok(AiCommandResult {
        text: shot.text,
        usage_line: shot.usage_line,
    })
}

pub(crate) async fn run_ai_command<R: Runtime>(
    app: &AppHandle<R>,
    prompt: String,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    project_id: Option<String>,
    working_dir: Option<String>,
    provider_override: Option<String>,
    model_override: Option<String>,
    reasoning_effort_override: Option<String>,
    employee_id: Option<String>,
) -> Result<AiCommandResult, String> {
    run_ai_command_with_options(
        app,
        prompt,
        image_paths,
        task_id,
        project_id,
        working_dir,
        provider_override,
        model_override,
        reasoning_effort_override,
        employee_id,
        &AiCommandOptions::default(),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_ai_command_with_options<R: Runtime>(
    app: &AppHandle<R>,
    prompt: String,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    project_id: Option<String>,
    working_dir: Option<String>,
    provider_override: Option<String>,
    model_override: Option<String>,
    reasoning_effort_override: Option<String>,
    employee_id: Option<String>,
    options: &AiCommandOptions,
) -> Result<AiCommandResult, String> {
    if let Some(employee_id) =
        resolve_native_one_shot_employee_id(provider_override.as_deref(), employee_id.as_deref())?
    {
        let shot = crate::native::run_native_one_shot(
            app,
            &employee_id,
            prompt,
            image_paths,
            model_override,
            reasoning_effort_override,
        )
        .await?;
        return Ok(AiCommandResult {
            text: shot.text,
            usage_line: shot.usage_line,
        });
    }

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

    if let Some(provider_override) = provider_override.as_deref() {
        one_shot_provider = normalize_one_shot_provider_for_target(
            Some(provider_override),
            execution_context.execution_target.as_str(),
        );
        one_shot_model = normalize_one_shot_model_for_provider(&one_shot_provider, None);
        one_shot_reasoning_effort =
            normalize_one_shot_reasoning_for_provider(&one_shot_provider, None);
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

    if one_shot_provider == "native" {
        let channel_id = settings
            .as_ref()
            .and_then(|settings| settings.one_shot_native_channel_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "一次性 AI 使用内置 Agent 时请先选择 AI 渠道".to_string())?;
        let shot = crate::native::run_native_one_shot_via_channel(
            app,
            channel_id,
            &one_shot_model,
            &one_shot_reasoning_effort,
            prompt,
            Some(image_paths),
        )
        .await?;
        return Ok(AiCommandResult {
            text: shot.text,
            usage_line: shot.usage_line,
        });
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
                    options,
                )
                .await
                {
                    Ok(result) => return Ok(AiCommandResult::text_only(result)),
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
        (EXECUTION_TARGET_LOCAL, "grok") => {
            run_grok_one_shot_via_cli(
                app,
                prompt,
                &one_shot_model,
                &one_shot_reasoning_effort,
                working_dir.as_deref(),
            )
            .await
        }
        (EXECUTION_TARGET_SSH, "grok") => {
            let ssh_config_id = execution_context
                .ssh_config_id
                .as_deref()
                .ok_or_else(|| "SSH 一次性 AI 缺少 ssh_config_id".to_string())?;
            run_grok_one_shot_via_remote_cli(
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
            if !one_shot_sdk_enabled {
                return Err("一次性 AI 未启用 OpenCode SDK，当前不可用".to_string());
            }
            let ssh_config_id = execution_context
                .ssh_config_id
                .as_deref()
                .ok_or_else(|| "SSH 一次性 AI 缺少 ssh_config_id".to_string())?;
            run_opencode_one_shot_via_remote_sdk(
                app,
                ssh_config_id,
                &prompt,
                &one_shot_model,
                &one_shot_reasoning_effort,
                working_dir.as_deref(),
                &image_paths,
            )
            .await
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
                                options,
                            )
                            .await
                            {
                                Ok(result) => return Ok(AiCommandResult::text_only(result)),
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
        (_, "native") => Err("内置 Agent 一次性调用需要绑定员工，不能回退到 Codex SDK".to_string()),
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
                        options,
                    )
                    .await
                    {
                        Ok(result) => return Ok(AiCommandResult::text_only(result)),
                        Err(error) => {
                            eprintln!("[codex-sdk] 调用失败，回退到 codex exec: {error}");
                            sdk_error = Some(error);
                        }
                    }
                }
            }

            match run_ai_command_via_exec(
                app,
                prompt,
                &one_shot_model,
                &one_shot_reasoning_effort,
                working_dir.as_deref(),
                &image_paths,
                options,
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
    .map(AiCommandResult::text_only)
}

#[cfg(test)]
mod tests {
    use super::{normalize_one_shot_provider_for_target, resolve_native_one_shot_employee_id};

    #[test]
    fn one_shot_provider_keeps_native_instead_of_remapping_to_codex() {
        assert_eq!(
            normalize_one_shot_provider_for_target(Some("native"), "local"),
            "native"
        );
        assert_eq!(
            normalize_one_shot_provider_for_target(Some("native"), "ssh"),
            "native"
        );
        assert_eq!(
            normalize_one_shot_provider_for_target(Some("unknown"), "local"),
            "codex"
        );
        assert_eq!(
            normalize_one_shot_provider_for_target(None, "local"),
            "codex"
        );
    }

    #[test]
    fn native_one_shot_requires_employee_and_does_not_fall_back_to_codex() {
        assert_eq!(
            resolve_native_one_shot_employee_id(Some("claude"), None).unwrap(),
            None
        );
        assert_eq!(
            resolve_native_one_shot_employee_id(Some("native"), Some(" emp-1 "))
                .unwrap()
                .as_deref(),
            Some("emp-1")
        );
        assert_eq!(
            resolve_native_one_shot_employee_id(Some("native"), None).unwrap_err(),
            "内置 Agent 一次性调用需要绑定员工，不能回退到 Codex SDK"
        );
        assert_eq!(
            resolve_native_one_shot_employee_id(Some("native"), Some("  ")).unwrap_err(),
            "内置 Agent 一次性调用需要绑定员工，不能回退到 Codex SDK"
        );
    }
}
