use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::codex::{new_node_command, new_npm_command};
use crate::db::models::{
    OpenCodeHealthCheck, OpenCodeSdkInstallResult, OpenCodeSettings, UpdateOpenCodeSettings,
};

const SETTINGS_FILE_NAME: &str = "opencode-settings.json";
const SDK_RUNTIME_DIR_NAME: &str = "opencode-sdk-runtime";
const SDK_BRIDGE_FILE_NAME: &str = "opencode_sdk_bridge.mjs";
const SDK_PACKAGE_NAME: &str = "@opencode-ai/sdk";
const MINIMUM_NODE_MAJOR: u32 = 18;
const DEFAULT_MODEL: &str = "openai/gpt-4o";
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 4096;

fn normalize_opencode_model(value: Option<&str>) -> String {
    match value {
        Some(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => DEFAULT_MODEL.to_string(),
    }
}

fn normalize_port(value: Option<u16>) -> u16 {
    match value {
        Some(v) if v > 0 && v <= 65535 => v,
        _ => DEFAULT_PORT,
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RawOpenCodeSettings {
    #[serde(default)]
    sdk_enabled: Option<bool>,
    #[serde(default)]
    default_model: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    node_path_override: Option<String>,
    #[serde(default)]
    sdk_install_dir: Option<String>,
}

fn opencode_sdk_runtime_ready(
    settings: &OpenCodeSettings,
    node_available: bool,
    node_supported: bool,
    sdk_installed: bool,
) -> bool {
    settings.sdk_enabled && node_available && node_supported && sdk_installed
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

fn load_raw_settings<R: Runtime>(app: &AppHandle<R>) -> Result<RawOpenCodeSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(RawOpenCodeSettings::default());
    }
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("读取 OpenCode 设置失败: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("解析 OpenCode 设置失败: {error}"))
}

fn normalize_settings<R: Runtime>(
    app: &AppHandle<R>,
    raw: &RawOpenCodeSettings,
) -> Result<OpenCodeSettings, String> {
    Ok(OpenCodeSettings {
        sdk_enabled: raw.sdk_enabled.unwrap_or(false),
        default_model: normalize_opencode_model(raw.default_model.as_deref()),
        host: raw.host.clone().unwrap_or_else(|| DEFAULT_HOST.to_string()),
        port: normalize_port(raw.port),
        sdk_install_dir: raw
            .sdk_install_dir
            .clone()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| default_sdk_install_dir(app).unwrap_or_default()),
        node_path_override: raw
            .node_path_override
            .clone()
            .filter(|v| !v.trim().is_empty()),
    })
}

pub fn load_opencode_settings<R: Runtime>(app: &AppHandle<R>) -> Result<OpenCodeSettings, String> {
    let raw = load_raw_settings(app)?;
    normalize_settings(app, &raw)
}

fn save_opencode_settings<R: Runtime>(
    app: &AppHandle<R>,
    settings: &OpenCodeSettings,
) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建 OpenCode 设置目录失败: {error}"))?;
    }

    let raw = RawOpenCodeSettings {
        sdk_enabled: Some(settings.sdk_enabled),
        default_model: Some(settings.default_model.clone()),
        host: Some(settings.host.clone()),
        port: Some(settings.port),
        node_path_override: settings.node_path_override.clone(),
        sdk_install_dir: Some(settings.sdk_install_dir.clone()),
    };

    let json = serde_json::to_string_pretty(&raw)
        .map_err(|error| format!("序列化 OpenCode 设置失败: {error}"))?;
    fs::write(&path, json).map_err(|error| format!("写入 OpenCode 设置失败: {error}"))?;
    Ok(())
}

fn merge_opencode_settings<R: Runtime>(
    app: &AppHandle<R>,
    updates: UpdateOpenCodeSettings,
) -> Result<OpenCodeSettings, String> {
    let mut current = load_opencode_settings(app)?;

    if let Some(sdk_enabled) = updates.sdk_enabled {
        current.sdk_enabled = sdk_enabled;
    }
    if let Some(default_model) = updates.default_model {
        current.default_model = normalize_opencode_model(Some(&default_model));
    }
    if let Some(host) = updates.host {
        current.host = host;
    }
    if let Some(port) = updates.port {
        current.port = normalize_port(Some(port));
    }
    if let Some(node_path_override) = updates.node_path_override {
        current.node_path_override = node_path_override.filter(|v| !v.trim().is_empty());
    }
    if let Some(sdk_install_dir) = updates.sdk_install_dir {
        if let Some(dir) = sdk_install_dir.filter(|v| !v.trim().is_empty()) {
            current.sdk_install_dir = dir;
        }
    }

    save_opencode_settings(app, &current)?;
    Ok(current)
}

pub fn sdk_bridge_script_path(install_dir: &Path) -> PathBuf {
    install_dir.join(SDK_BRIDGE_FILE_NAME)
}

pub fn ensure_opencode_sdk_runtime_layout(install_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(install_dir)
        .map_err(|error| format!("创建 OpenCode SDK 运行目录失败: {error}"))?;

    let package_json_path = install_dir.join("package.json");
    if !package_json_path.exists() {
        let package_json = serde_json::json!({
            "name": "codex-ai-opencode-sdk-runtime",
            "private": true,
            "type": "module"
        });
        fs::write(
            &package_json_path,
            serde_json::to_string_pretty(&package_json)
                .map_err(|error| format!("序列化 OpenCode SDK package.json 失败: {error}"))?,
        )
        .map_err(|error| format!("写入 OpenCode SDK package.json 失败: {error}"))?;
    }

    let bridge_path = sdk_bridge_script_path(install_dir);
    let bridge_content = include_str!("opencode_sdk_bridge.mjs");
    let should_write = match fs::read_to_string(&bridge_path) {
        Ok(existing) => existing != bridge_content,
        Err(_) => true,
    };
    if should_write {
        fs::write(&bridge_path, bridge_content)
            .map_err(|error| format!("写入 OpenCode SDK bridge 脚本失败: {error}"))?;
    }

    Ok(())
}

fn sdk_package_json_path(install_dir: &Path) -> PathBuf {
    install_dir
        .join("node_modules")
        .join(SDK_PACKAGE_NAME)
        .join("package.json")
}

pub fn read_opencode_sdk_version(install_dir: &Path) -> Result<Option<String>, String> {
    let path = sdk_package_json_path(install_dir);
    if !path.exists() {
        return Ok(None);
    }

    #[derive(Deserialize)]
    struct PackageMetadata {
        version: Option<String>,
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("读取 OpenCode SDK 版本失败: {error}"))?;
    let metadata = serde_json::from_str::<PackageMetadata>(&raw)
        .map_err(|error| format!("解析 OpenCode SDK 版本失败: {error}"))?;

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

pub async fn inspect_opencode_sdk_runtime<R: Runtime>(
    app: &AppHandle<R>,
    settings: &OpenCodeSettings,
) -> OpenCodeHealthCheck {
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
    let sdk_version = read_opencode_sdk_version(install_dir).unwrap_or(None);
    let sdk_installed = sdk_version.is_some() && sdk_bridge_script_path(install_dir).exists();
    let effective_provider = if opencode_sdk_runtime_ready(
        settings,
        node_available,
        node_supported,
        sdk_installed,
    ) {
        "sdk"
    } else {
        "unavailable"
    }
    .to_string();

    let status_message = if !settings.sdk_enabled {
        "OpenCode SDK 未启用".to_string()
    } else if let Some(error) = node_error {
        format!("Node 不可用：{error}")
    } else if let Some(version) = node_version.as_deref() {
        match parse_node_major_version(version) {
            Some(major) if major < MINIMUM_NODE_MAJOR => format!(
                "Node 版本过低（当前 {major}），OpenCode SDK 需要 Node.js {MINIMUM_NODE_MAJOR}+"
            ),
            _ if !sdk_installed => "OpenCode SDK 未安装".to_string(),
            _ => format!("OpenCode SDK 已就绪（Node {version}）"),
        }
    } else {
        "OpenCode SDK 状态未知".to_string()
    };

    let _ = app;

    OpenCodeHealthCheck {
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

pub async fn install_opencode_sdk_runtime<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<OpenCodeSdkInstallResult, String> {
    let settings = load_opencode_settings(app)?;
    let install_dir = PathBuf::from(&settings.sdk_install_dir);

    ensure_opencode_sdk_runtime_layout(&install_dir)?;
    save_opencode_settings(app, &settings)?;

    let node_version = read_node_version(settings.node_path_override.as_deref()).await?;
    if let Some(major) = parse_node_major_version(&node_version) {
        if major < MINIMUM_NODE_MAJOR {
            return Err(format!(
                "Node 版本过低（当前 {major}），OpenCode SDK 需要 Node.js {MINIMUM_NODE_MAJOR}+"
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
        .map_err(|error| format!("安装 OpenCode SDK 失败: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "安装 OpenCode SDK 失败".to_string()
        } else {
            format!("安装 OpenCode SDK 失败：{stderr}")
        });
    }

    let sdk_version = read_opencode_sdk_version(&install_dir)?
        .ok_or_else(|| "OpenCode SDK 已安装，但未能读取版本信息".to_string())?;

    Ok(OpenCodeSdkInstallResult {
        sdk_installed: true,
        sdk_version: Some(sdk_version),
        install_dir: install_dir.to_string_lossy().to_string(),
        node_version: Some(node_version),
        message: "OpenCode SDK 安装完成".to_string(),
    })
}

#[tauri::command]
pub async fn get_opencode_settings<R: Runtime>(
    app: AppHandle<R>,
) -> Result<OpenCodeSettings, String> {
    load_opencode_settings(&app)
}

#[tauri::command]
pub async fn update_opencode_settings<R: Runtime>(
    app: AppHandle<R>,
    updates: UpdateOpenCodeSettings,
) -> Result<OpenCodeSettings, String> {
    merge_opencode_settings(&app, updates)
}

#[tauri::command]
pub async fn check_opencode_sdk_health<R: Runtime>(
    app: AppHandle<R>,
) -> Result<OpenCodeHealthCheck, String> {
    let settings = load_opencode_settings(&app)?;
    Ok(inspect_opencode_sdk_runtime(&app, &settings).await)
}

#[tauri::command]
pub async fn install_opencode_sdk<R: Runtime>(
    app: AppHandle<R>,
) -> Result<OpenCodeSdkInstallResult, String> {
    install_opencode_sdk_runtime(&app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings(sdk_enabled: bool) -> OpenCodeSettings {
        OpenCodeSettings {
            sdk_enabled,
            default_model: DEFAULT_MODEL.to_string(),
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            sdk_install_dir: "/tmp/codex-ai-opencode-sdk-runtime".to_string(),
            node_path_override: None,
        }
    }

    #[test]
    fn opencode_sdk_runtime_ready_requires_all() {
        let settings = test_settings(true);
        assert!(!opencode_sdk_runtime_ready(&settings, true, true, false));
        assert!(!opencode_sdk_runtime_ready(&settings, true, false, true));
        assert!(!opencode_sdk_runtime_ready(&settings, false, true, true));
        assert!(opencode_sdk_runtime_ready(&settings, true, true, true));
    }

    #[test]
    fn disabled_opencode_sdk_never_reports_runtime_ready() {
        let settings = test_settings(false);
        assert!(!opencode_sdk_runtime_ready(&settings, true, true, true));
    }
}
