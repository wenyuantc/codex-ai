use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::codex::{new_node_command, new_npm_command};
use crate::db::models::{
    ClaudeHealthCheck, ClaudeSdkInstallResult, ClaudeSettings, UpdateClaudeSettings,
};

const SETTINGS_FILE_NAME: &str = "claude-settings.json";
const SDK_RUNTIME_DIR_NAME: &str = "claude-sdk-runtime";
const SDK_BRIDGE_FILE_NAME: &str = "claude-sdk-bridge.mjs";
const SDK_PACKAGE_NAME: &str = "@anthropic-ai/claude-agent-sdk";
const MINIMUM_NODE_MAJOR: u32 = 18;
const DEFAULT_MODEL: &str = "claude-sonnet-4-6";
const DEFAULT_THINKING_BUDGET: i32 = 10000;
const CLAUDE_PROVIDER_SDK: &str = "sdk";
const CLAUDE_PROVIDER_CLI: &str = "cli";
const CLAUDE_PROVIDER_UNAVAILABLE: &str = "unavailable";

pub const SUPPORTED_CLAUDE_MODELS: &[&str] = &[
    "claude-opus-4-7",
    "claude-opus-4-7[1m]",
    "claude-opus-4-6[1m]",
    "claude-sonnet-4-6",
    "claude-sonnet-4-6[1m]",
    "claude-haiku-4-5",
];

#[derive(Debug, Default, Deserialize, Serialize)]
struct RawClaudeSettings {
    #[serde(default)]
    sdk_enabled: Option<bool>,
    #[serde(default)]
    default_model: Option<String>,
    #[serde(default)]
    default_thinking_budget: Option<i32>,
    #[serde(default)]
    node_path_override: Option<String>,
    #[serde(default)]
    cli_path_override: Option<String>,
    #[serde(default)]
    sdk_install_dir: Option<String>,
}

fn normalize_claude_model(value: Option<&str>) -> String {
    match value {
        Some(v) if SUPPORTED_CLAUDE_MODELS.contains(&v) => v.to_string(),
        _ => DEFAULT_MODEL.to_string(),
    }
}

fn normalize_thinking_budget(value: Option<i32>) -> i32 {
    match value {
        Some(v) if v >= 1024 && v <= 128000 => v,
        _ => DEFAULT_THINKING_BUDGET,
    }
}

fn claude_sdk_runtime_ready(
    settings: &ClaudeSettings,
    node_available: bool,
    node_supported: bool,
    sdk_installed: bool,
    cli_available: bool,
) -> bool {
    settings.sdk_enabled && node_available && node_supported && sdk_installed && cli_available
}

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("获取配置目录失败: {error}"))?;
    Ok(config_dir.join(SETTINGS_FILE_NAME))
}

fn default_sdk_install_dir<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("获取配置目录失败: {error}"))?;
    Ok(config_dir
        .join(SDK_RUNTIME_DIR_NAME)
        .to_string_lossy()
        .to_string())
}

fn load_raw_settings<R: Runtime>(app: &AppHandle<R>) -> Result<RawClaudeSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(RawClaudeSettings::default());
    }
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("读取 Claude 设置失败: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("解析 Claude 设置失败: {error}"))
}

fn normalize_settings<R: Runtime>(
    app: &AppHandle<R>,
    raw: &RawClaudeSettings,
) -> Result<ClaudeSettings, String> {
    Ok(ClaudeSettings {
        sdk_enabled: raw.sdk_enabled.unwrap_or(false),
        default_model: normalize_claude_model(raw.default_model.as_deref()),
        default_thinking_budget: normalize_thinking_budget(raw.default_thinking_budget),
        sdk_install_dir: raw
            .sdk_install_dir
            .clone()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| default_sdk_install_dir(app).unwrap_or_default()),
        node_path_override: raw
            .node_path_override
            .clone()
            .filter(|v| !v.trim().is_empty()),
        cli_path_override: raw
            .cli_path_override
            .clone()
            .filter(|v| !v.trim().is_empty()),
    })
}

pub fn load_claude_settings<R: Runtime>(app: &AppHandle<R>) -> Result<ClaudeSettings, String> {
    let raw = load_raw_settings(app)?;
    normalize_settings(app, &raw)
}

fn save_claude_settings<R: Runtime>(
    app: &AppHandle<R>,
    settings: &ClaudeSettings,
) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 Claude 设置目录失败: {error}"))?;
    }

    let raw = RawClaudeSettings {
        sdk_enabled: Some(settings.sdk_enabled),
        default_model: Some(settings.default_model.clone()),
        default_thinking_budget: Some(settings.default_thinking_budget),
        node_path_override: settings.node_path_override.clone(),
        cli_path_override: settings.cli_path_override.clone(),
        sdk_install_dir: Some(settings.sdk_install_dir.clone()),
    };

    let json = serde_json::to_string_pretty(&raw)
        .map_err(|error| format!("序列化 Claude 设置失败: {error}"))?;
    fs::write(&path, json).map_err(|error| format!("写入 Claude 设置失败: {error}"))?;
    Ok(())
}

fn merge_claude_settings<R: Runtime>(
    app: &AppHandle<R>,
    updates: UpdateClaudeSettings,
) -> Result<ClaudeSettings, String> {
    let mut current = load_claude_settings(app)?;

    if let Some(sdk_enabled) = updates.sdk_enabled {
        current.sdk_enabled = sdk_enabled;
    }
    if let Some(default_model) = updates.default_model {
        current.default_model = normalize_claude_model(Some(&default_model));
    }
    if let Some(default_thinking_budget) = updates.default_thinking_budget {
        current.default_thinking_budget = normalize_thinking_budget(Some(default_thinking_budget));
    }
    if let Some(node_path_override) = updates.node_path_override {
        current.node_path_override = node_path_override.filter(|v| !v.trim().is_empty());
    }
    if let Some(cli_path_override) = updates.cli_path_override {
        current.cli_path_override = cli_path_override.filter(|v| !v.trim().is_empty());
    }
    if let Some(sdk_install_dir) = updates.sdk_install_dir {
        if let Some(dir) = sdk_install_dir.filter(|v| !v.trim().is_empty()) {
            current.sdk_install_dir = dir;
        }
    }

    save_claude_settings(app, &current)?;
    Ok(current)
}

pub fn sdk_bridge_script_path(install_dir: &Path) -> PathBuf {
    install_dir.join(SDK_BRIDGE_FILE_NAME)
}

pub fn ensure_claude_sdk_runtime_layout(install_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(install_dir)
        .map_err(|error| format!("创建 Claude SDK 运行目录失败: {error}"))?;

    let package_json_path = install_dir.join("package.json");
    if !package_json_path.exists() {
        let package_json = serde_json::json!({
            "name": "codex-ai-claude-sdk-runtime",
            "private": true,
            "type": "module"
        });
        fs::write(
            &package_json_path,
            serde_json::to_string_pretty(&package_json)
                .map_err(|error| format!("序列化 Claude SDK package.json 失败: {error}"))?,
        )
        .map_err(|error| format!("写入 Claude SDK package.json 失败: {error}"))?;
    }

    let bridge_path = sdk_bridge_script_path(install_dir);
    let bridge_content = include_str!("claude_sdk_bridge.mjs");
    let should_write = match fs::read_to_string(&bridge_path) {
        Ok(existing) => existing != bridge_content,
        Err(_) => true,
    };
    if should_write {
        fs::write(&bridge_path, bridge_content)
            .map_err(|error| format!("写入 Claude SDK bridge 脚本失败: {error}"))?;
    }

    Ok(())
}

fn sdk_package_json_path(install_dir: &Path) -> PathBuf {
    install_dir
        .join("node_modules")
        .join(SDK_PACKAGE_NAME)
        .join("package.json")
}

pub fn read_claude_sdk_version(install_dir: &Path) -> Result<Option<String>, String> {
    let path = sdk_package_json_path(install_dir);
    if !path.exists() {
        return Ok(None);
    }

    #[derive(Deserialize)]
    struct PackageMetadata {
        version: Option<String>,
    }

    let raw =
        fs::read_to_string(&path).map_err(|error| format!("读取 Claude SDK 版本失败: {error}"))?;
    let metadata = serde_json::from_str::<PackageMetadata>(&raw)
        .map_err(|error| format!("解析 Claude SDK 版本失败: {error}"))?;

    Ok(metadata.version.filter(|v| !v.trim().is_empty()))
}

fn parse_node_major_version(version: &str) -> Option<u32> {
    let normalized = version.trim().trim_start_matches('v');
    normalized
        .split('.')
        .next()
        .and_then(|segment| segment.parse::<u32>().ok())
}

async fn read_node_version(node_path_override: Option<&str>) -> Result<String, String> {
    let mut command = new_node_command(node_path_override).await?;
    let output = command
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("检测 Node 版本失败: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Node 版本检测失败".to_string()
        } else {
            stderr
        });
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return Err("Node 版本输出为空".to_string());
    }

    Ok(version)
}

pub async fn read_claude_cli_version(cli_path_override: Option<&str>) -> Result<String, String> {
    let cli_path = cli_path_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("claude");
    let output = tokio::process::Command::new(cli_path)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("检测 Claude CLI 版本失败: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Claude CLI 版本检测失败".to_string()
        } else {
            stderr
        });
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return Err("Claude CLI 版本输出为空".to_string());
    }

    Ok(version)
}

pub async fn inspect_claude_sdk_runtime<R: Runtime>(
    app: &AppHandle<R>,
    settings: &ClaudeSettings,
) -> ClaudeHealthCheck {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let node_result = read_node_version(settings.node_path_override.as_deref()).await;
    let node_available = node_result.is_ok();
    let node_version = node_result.as_ref().ok().cloned();
    let node_error = node_result.err();
    let node_supported = node_version
        .as_deref()
        .and_then(parse_node_major_version)
        .map(|major| major >= MINIMUM_NODE_MAJOR)
        .unwrap_or(false);

    let install_dir = Path::new(&settings.sdk_install_dir);
    let sdk_version = read_claude_sdk_version(install_dir).unwrap_or(None);
    let sdk_installed = sdk_version.is_some() && sdk_bridge_script_path(install_dir).exists();
    let cli_result = read_claude_cli_version(settings.cli_path_override.as_deref()).await;
    let cli_available = cli_result.is_ok();
    let cli_version = cli_result.as_ref().ok().cloned();
    let effective_provider = if claude_sdk_runtime_ready(
        settings,
        node_available,
        node_supported,
        sdk_installed,
        cli_available,
    ) {
        CLAUDE_PROVIDER_SDK
    } else if cli_available {
        CLAUDE_PROVIDER_CLI
    } else {
        CLAUDE_PROVIDER_UNAVAILABLE
    }
    .to_string();

    let status_message = if !settings.sdk_enabled {
        if cli_available {
            "Claude SDK 未启用，将使用 Claude CLI".to_string()
        } else {
            "Claude SDK 未启用，且 Claude CLI 不可用".to_string()
        }
    } else if let Some(error) = node_error {
        if cli_available {
            format!("Node 不可用，已回退到 Claude CLI：{error}")
        } else {
            format!("Node 不可用，且 Claude CLI 不可用：{error}")
        }
    } else if let Some(version) = node_version.as_deref() {
        match parse_node_major_version(version) {
            Some(major) if major < MINIMUM_NODE_MAJOR => {
                if cli_available {
                    format!("Node 版本过低（当前 {major}），已回退到 Claude CLI")
                } else {
                    format!(
                        "Node 版本过低（当前 {major}），Claude SDK 需要 Node.js {MINIMUM_NODE_MAJOR}+，且 Claude CLI 不可用"
                    )
                }
            }
            _ if !sdk_installed => {
                if cli_available {
                    "Claude SDK 未安装，已回退到 Claude CLI".to_string()
                } else {
                    "Claude SDK 未安装，且 Claude CLI 不可用".to_string()
                }
            }
            _ if cli_available => format!("Claude SDK 已就绪（Node {version}）"),
            _ => "Claude SDK 已安装，但 Claude Code CLI 不可用；Agent SDK 需要先安装并登录 Claude CLI".to_string(),
        }
    } else {
        "Claude SDK 状态未知".to_string()
    };

    let _ = app;

    ClaudeHealthCheck {
        cli_available,
        cli_version,
        sdk_installed,
        sdk_version,
        node_available,
        node_version,
        sdk_install_dir: settings.sdk_install_dir.clone(),
        effective_provider,
        sdk_status_message: status_message,
        checked_at: now,
    }
}

pub async fn install_claude_sdk_runtime<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<ClaudeSdkInstallResult, String> {
    let settings = load_claude_settings(app)?;
    let install_dir = PathBuf::from(&settings.sdk_install_dir);

    ensure_claude_sdk_runtime_layout(&install_dir)?;
    save_claude_settings(app, &settings)?;

    let node_version = read_node_version(settings.node_path_override.as_deref()).await?;
    if let Some(major) = parse_node_major_version(&node_version) {
        if major < MINIMUM_NODE_MAJOR {
            return Err(format!(
                "Node 版本过低（当前 {major}），Claude SDK 需要 Node.js {MINIMUM_NODE_MAJOR}+"
            ));
        }
    }

    let mut npm_command = new_npm_command(settings.node_path_override.as_deref()).await?;
    let output = npm_command
        .current_dir(&install_dir)
        .arg("install")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg(format!("{SDK_PACKAGE_NAME}@latest"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("安装 Claude SDK 失败: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "安装 Claude SDK 失败".to_string()
        } else {
            format!("安装 Claude SDK 失败：{stderr}")
        });
    }

    let sdk_version = read_claude_sdk_version(&install_dir)?
        .ok_or_else(|| "Claude SDK 已安装，但未能读取版本信息".to_string())?;

    Ok(ClaudeSdkInstallResult {
        sdk_installed: true,
        sdk_version: Some(sdk_version),
        install_dir: install_dir.to_string_lossy().to_string(),
        node_version: Some(node_version),
        message: "Claude SDK 安装完成".to_string(),
    })
}

#[tauri::command]
pub async fn get_claude_settings<R: Runtime>(app: AppHandle<R>) -> Result<ClaudeSettings, String> {
    load_claude_settings(&app)
}

#[tauri::command]
pub async fn update_claude_settings<R: Runtime>(
    app: AppHandle<R>,
    updates: UpdateClaudeSettings,
) -> Result<ClaudeSettings, String> {
    merge_claude_settings(&app, updates)
}

#[tauri::command]
pub async fn check_claude_sdk_health<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ClaudeHealthCheck, String> {
    let settings = load_claude_settings(&app)?;
    Ok(inspect_claude_sdk_runtime(&app, &settings).await)
}

#[tauri::command]
pub async fn install_claude_sdk<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ClaudeSdkInstallResult, String> {
    install_claude_sdk_runtime(&app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings(sdk_enabled: bool) -> ClaudeSettings {
        ClaudeSettings {
            sdk_enabled,
            default_model: DEFAULT_MODEL.to_string(),
            default_thinking_budget: DEFAULT_THINKING_BUDGET,
            sdk_install_dir: "/tmp/codex-ai-claude-sdk-runtime".to_string(),
            node_path_override: None,
            cli_path_override: None,
        }
    }

    #[test]
    fn claude_sdk_runtime_requires_cli_runtime() {
        let settings = test_settings(true);

        assert!(!claude_sdk_runtime_ready(
            &settings, true, true, true, false
        ));
        assert!(claude_sdk_runtime_ready(&settings, true, true, true, true));
    }

    #[test]
    fn disabled_claude_sdk_never_reports_runtime_ready() {
        let settings = test_settings(false);

        assert!(!claude_sdk_runtime_ready(&settings, true, true, true, true));
    }
}
