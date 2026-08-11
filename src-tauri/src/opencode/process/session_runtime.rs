use std::path::{Path, PathBuf};
use std::process::Stdio;

use tauri::{AppHandle, Runtime};
use tokio::io::AsyncWriteExt;

use crate::app::{build_remote_opencode_sdk_bridge_command, build_ssh_command};
use crate::codex::new_node_command;
use crate::db::models::SshConfigRecord;

use super::lifecycle::OpenCodeChild;

pub struct OpenCodeBridgeConfig {
    pub mode: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub host: String,
    pub port: u16,
    pub node_path_override: Option<String>,
    pub working_directory: String,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub resume_session_id: Option<String>,
    pub image_paths: Vec<String>,
    pub install_dir: PathBuf,
    /// Interactive default true; pipeline/automation mid-flight sets false.
    pub await_followups: bool,
}

pub struct OpenCodeServerBridgeConfig {
    pub host: String,
    pub port: u16,
    pub parent_pid: u32,
    pub node_path_override: Option<String>,
    pub install_dir: PathBuf,
}

pub fn serialize_opencode_bridge_config(config: &OpenCodeBridgeConfig) -> Result<String, String> {
    let config_json = serde_json::json!({
        "mode": config.mode,
        "model": config.model,
        "reasoningEffort": config.reasoning_effort,
        "host": config.host,
        "port": config.port,
        "workingDirectory": config.working_directory,
        "prompt": config.prompt,
        "systemPrompt": config.system_prompt,
        "resumeSessionId": config.resume_session_id,
        "imagePaths": config.image_paths,
        "awaitFollowups": config.await_followups,
    });
    serde_json::to_string(&config_json).map_err(|error| format!("序列化 bridge 配置失败: {error}"))
}

pub async fn launch_opencode_bridge(
    config: &OpenCodeBridgeConfig,
    bridge_path: &Path,
) -> Result<OpenCodeChild, String> {
    let mut command = new_node_command(config.node_path_override.as_deref()).await?;

    command
        .arg(bridge_path)
        .current_dir(&config.install_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    configure_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 OpenCode SDK bridge 失败: {error}"))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法获取 OpenCode bridge stdin".to_string())?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdin = write_bridge_stdin_keep(stdin, &serialize_opencode_bridge_config(config)?).await?;

    Ok(OpenCodeChild::with_stdio_and_stdin(
        child,
        stdout,
        stderr,
        Some(stdin),
    ))
}

pub async fn launch_opencode_bridge_via_ssh<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    install_dir: &str,
    node_path_override: Option<&str>,
    config: &OpenCodeBridgeConfig,
) -> Result<(OpenCodeChild, Vec<PathBuf>), String> {
    let remote_command =
        build_remote_opencode_sdk_bridge_command(install_dir, node_path_override);
    let (mut command, askpass_path) =
        build_ssh_command(app, ssh_config, Some(&remote_command), true, false).await?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|error| format!("启动远程 OpenCode SDK bridge 失败: {error}"))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法获取远程 OpenCode bridge stdin".to_string())?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdin = match write_bridge_stdin_keep(stdin, &serialize_opencode_bridge_config(config)?)
        .await
    {
        Ok(stdin) => stdin,
        Err(error) => {
            let _ = child.kill().await;
            if let Some(path) = askpass_path.as_ref() {
                let _ = std::fs::remove_file(path);
            }
            return Err(error);
        }
    };

    Ok((
        OpenCodeChild::with_stdio_and_stdin(child, stdout, stderr, Some(stdin)),
        askpass_path.into_iter().collect(),
    ))
}

pub async fn launch_opencode_server_bridge(
    config: &OpenCodeServerBridgeConfig,
    bridge_path: &Path,
) -> Result<OpenCodeChild, String> {
    let mut command = new_node_command(config.node_path_override.as_deref()).await?;

    command
        .arg(bridge_path)
        .current_dir(&config.install_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    configure_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 OpenCode SDK server bridge 失败: {error}"))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法获取 OpenCode server bridge stdin".to_string())?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let config_json = serde_json::json!({
        "mode": "server",
        "host": config.host,
        "port": config.port,
        "parentPid": config.parent_pid,
    });

    let config_str = serde_json::to_string(&config_json)
        .map_err(|error| format!("序列化 server bridge 配置失败: {error}"))?;

    write_bridge_stdin(stdin, &config_str).await?;

    Ok(OpenCodeChild::with_stdio(child, stdout, stderr))
}

async fn write_bridge_stdin_keep(
    mut stdin: tokio::process::ChildStdin,
    config_str: &str,
) -> Result<tokio::process::ChildStdin, String> {
    let mut payload = config_str.as_bytes().to_vec();
    if !payload.ends_with(b"\n") {
        payload.push(b'\n');
    }
    crate::engine::write_stdin_bytes(&mut stdin, &payload).await?;
    Ok(stdin)
}

async fn write_bridge_stdin(
    mut stdin: tokio::process::ChildStdin,
    config_str: &str,
) -> Result<(), String> {
    stdin
        .write_all(config_str.as_bytes())
        .await
        .map_err(|error| format!("写入 OpenCode bridge stdin 失败: {error}"))?;
    stdin
        .flush()
        .await
        .map_err(|error| format!("刷新 OpenCode bridge stdin 失败: {error}"))?;
    drop(stdin);
    Ok(())
}

#[cfg(unix)]
fn configure_process_group(command: &mut tokio::process::Command) {
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut tokio::process::Command) {}
