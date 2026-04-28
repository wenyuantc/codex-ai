use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::AsyncWriteExt;

use crate::codex::new_node_command;

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
}

pub struct OpenCodeServerBridgeConfig {
    pub host: String,
    pub port: u16,
    pub parent_pid: u32,
    pub node_path_override: Option<String>,
    pub install_dir: PathBuf,
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
    });

    let config_str = serde_json::to_string(&config_json)
        .map_err(|error| format!("序列化 bridge 配置失败: {error}"))?;

    let mut stdin_writer = stdin;
    stdin_writer
        .write_all(config_str.as_bytes())
        .await
        .map_err(|error| format!("写入 OpenCode bridge stdin 失败: {error}"))?;
    stdin_writer
        .flush()
        .await
        .map_err(|error| format!("刷新 OpenCode bridge stdin 失败: {error}"))?;
    drop(stdin_writer);

    Ok(OpenCodeChild::new(child, None, stdout, stderr))
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

    let mut stdin_writer = stdin;
    stdin_writer
        .write_all(config_str.as_bytes())
        .await
        .map_err(|error| format!("写入 OpenCode server bridge stdin 失败: {error}"))?;
    stdin_writer
        .flush()
        .await
        .map_err(|error| format!("刷新 OpenCode server bridge stdin 失败: {error}"))?;
    drop(stdin_writer);

    Ok(OpenCodeChild::new(child, None, stdout, stderr))
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
