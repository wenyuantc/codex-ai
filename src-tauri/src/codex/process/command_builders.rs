use super::*;

/// Run a one-shot AI command using `codex exec`.
pub(super) async fn probe_local_exec_json_support() -> Result<Option<CliJsonOutputFlag>, String> {
    let mut command = new_codex_command().await?;
    let output = command
        .arg("exec")
        .arg("--help")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("Failed to probe local codex exec --json support: {error}"))?;

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(detect_exec_json_output_flag(&combined))
}

pub(super) async fn probe_remote_exec_json_support<R: tauri::Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    node_path_override: Option<&str>,
) -> Result<Option<CliJsonOutputFlag>, String> {
    let output = execute_ssh_command(
        app,
        ssh_config,
        &build_remote_shell_command("codex exec --help 2>/dev/null || true", node_path_override),
        true,
    )
    .await?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(detect_exec_json_output_flag(&combined))
}

pub(super) fn build_session_exec_args(
    model: &str,
    reasoning_effort: &str,
    run_cwd: &str,
    image_paths: &[String],
    resume_session_id: Option<&str>,
    json_output_flag: Option<CliJsonOutputFlag>,
) -> Vec<String> {
    let mut args = vec![
        "exec".to_string(),
        "--model".to_string(),
        model.to_string(),
        "-c".to_string(),
        format!("model_reasoning_effort=\"{}\"", reasoning_effort),
        "-C".to_string(),
        run_cwd.to_string(),
    ];
    if let Some(json_output_flag) = json_output_flag {
        args.push(json_output_flag.as_arg().to_string());
    }
    if let Some(session_id) = resume_session_id {
        args.push("resume".to_string());
        args.push(session_id.to_string());
    }
    for image_path in image_paths {
        args.push("--image".to_string());
        args.push(image_path.clone());
    }
    args.push("-".to_string());
    args
}

pub(super) fn shell_escape_arg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn build_remote_codex_session_command(
    model: &str,
    reasoning_effort: &str,
    run_cwd: &str,
    image_paths: &[String],
    resume_session_id: Option<&str>,
    json_output_flag: Option<CliJsonOutputFlag>,
    node_path_override: Option<&str>,
) -> String {
    let args = build_session_exec_args(
        model,
        reasoning_effort,
        run_cwd,
        image_paths,
        resume_session_id,
        json_output_flag,
    )
    .into_iter()
    .map(|value| shell_escape_arg(&value))
    .collect::<Vec<_>>()
    .join(" ");
    build_remote_shell_command(
        &format!("cd {} && exec codex {}", shell_escape_arg(run_cwd), args),
        node_path_override,
    )
}

pub(super) fn build_remote_sdk_bridge_command(
    install_dir: &str,
    node_path_override: Option<&str>,
) -> String {
    let bridge_path = remote_sdk_bridge_path(install_dir);
    build_remote_shell_command(
        &format!(
            "install_dir={}; bridge_path={}; cd \"$install_dir\" && exec node \"$bridge_path\"",
            remote_shell_path_expression(install_dir),
            remote_shell_path_expression(&bridge_path),
        ),
        node_path_override,
    )
}

pub(super) fn build_one_shot_exec_args(
    model: &str,
    reasoning_effort: &str,
    working_dir: Option<&str>,
    image_paths: &[String],
) -> Vec<String> {
    let mut args = vec![
        "exec".to_string(),
        "--skip-git-repo-check".to_string(),
        "--model".to_string(),
        model.to_string(),
        "-c".to_string(),
        format!("model_reasoning_effort=\"{}\"", reasoning_effort),
    ];
    if let Some(working_dir) = working_dir {
        args.push("-C".to_string());
        args.push(working_dir.to_string());
    }
    for image_path in image_paths {
        args.push("--image".to_string());
        args.push(image_path.clone());
    }
    args
}

pub(super) fn parse_sdk_bridge_output(stdout: &[u8], stderr: &[u8]) -> Result<String, String> {
    let raw_stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let raw_stderr = String::from_utf8_lossy(stderr).trim().to_string();

    if !raw_stdout.is_empty() {
        match serde_json::from_str::<SdkBridgeResponse>(&raw_stdout) {
            Ok(response) if response.ok => {
                return Ok(response.text.unwrap_or_default().trim().to_string())
            }
            Ok(response) => {
                return Err(response
                    .error
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "Codex SDK 返回了失败响应".to_string()))
            }
            Err(_) => {}
        }
    }

    if !raw_stderr.is_empty() {
        return Err(raw_stderr);
    }

    if !raw_stdout.is_empty() {
        return Err(raw_stdout);
    }

    Err("Codex SDK 返回空响应".to_string())
}
