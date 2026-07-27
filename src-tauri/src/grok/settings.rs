use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::db::models::{GrokHealthCheck, GrokSettings, UpdateGrokSettings};

const SETTINGS_FILE_NAME: &str = "grok-settings.json";
const DEFAULT_MODEL: &str = "grok-4.5";
const DEFAULT_REASONING_EFFORT: &str = "high";
const GROK_PATH_ENV_VARS: &[&str] = &["GROK_CLI_PATH", "GROK_PATH"];
pub const SUPPORTED_GROK_MODELS: &[&str] = &["grok-4.5"];
pub const SUPPORTED_GROK_REASONING_EFFORTS: &[&str] = &["high", "medium", "low"];

#[derive(Debug, Default, Deserialize, Serialize)]
struct RawGrokSettings {
    #[serde(default)]
    default_model: Option<String>,
    #[serde(default)]
    default_reasoning_effort: Option<String>,
    #[serde(default)]
    cli_path_override: Option<String>,
}

pub fn normalize_grok_model(value: Option<&str>) -> String {
    let value = value.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return DEFAULT_MODEL.to_string();
    }

    if SUPPORTED_GROK_MODELS.contains(&value) {
        return value.to_string();
    }

    if value.starts_with("grok-") {
        return DEFAULT_MODEL.to_string();
    }

    DEFAULT_MODEL.to_string()
}

pub fn normalize_grok_reasoning_effort(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(value) if SUPPORTED_GROK_REASONING_EFFORTS.contains(&value) => value.to_string(),
        _ => DEFAULT_REASONING_EFFORT.to_string(),
    }
}

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("获取配置目录失败: {error}"))?;
    Ok(config_dir.join(SETTINGS_FILE_NAME))
}

fn load_raw_settings<R: Runtime>(app: &AppHandle<R>) -> Result<RawGrokSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(RawGrokSettings::default());
    }
    let raw = fs::read_to_string(&path).map_err(|error| format!("读取 Grok 设置失败: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("解析 Grok 设置失败: {error}"))
}

fn normalize_settings(raw: &RawGrokSettings) -> GrokSettings {
    GrokSettings {
        default_model: normalize_grok_model(raw.default_model.as_deref()),
        default_reasoning_effort: normalize_grok_reasoning_effort(
            raw.default_reasoning_effort.as_deref(),
        ),
        cli_path_override: raw
            .cli_path_override
            .clone()
            .filter(|value| !value.trim().is_empty()),
    }
}

pub fn load_grok_settings<R: Runtime>(app: &AppHandle<R>) -> Result<GrokSettings, String> {
    let raw = load_raw_settings(app)?;
    Ok(normalize_settings(&raw))
}

fn save_grok_settings<R: Runtime>(app: &AppHandle<R>, settings: &GrokSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 Grok 设置目录失败: {error}"))?;
    }

    let raw = RawGrokSettings {
        default_model: Some(settings.default_model.clone()),
        default_reasoning_effort: Some(settings.default_reasoning_effort.clone()),
        cli_path_override: settings.cli_path_override.clone(),
    };

    let json = serde_json::to_string_pretty(&raw)
        .map_err(|error| format!("序列化 Grok 设置失败: {error}"))?;
    fs::write(&path, json).map_err(|error| format!("写入 Grok 设置失败: {error}"))?;
    Ok(())
}

fn merge_grok_settings<R: Runtime>(
    app: &AppHandle<R>,
    updates: UpdateGrokSettings,
) -> Result<GrokSettings, String> {
    let mut current = load_grok_settings(app)?;

    if let Some(default_model) = updates.default_model {
        current.default_model = normalize_grok_model(Some(&default_model));
    }
    if let Some(default_reasoning_effort) = updates.default_reasoning_effort {
        current.default_reasoning_effort =
            normalize_grok_reasoning_effort(Some(&default_reasoning_effort));
    }
    if let Some(cli_path_override) = updates.cli_path_override {
        current.cli_path_override = cli_path_override.filter(|value| !value.trim().is_empty());
    }

    save_grok_settings(app, &current)?;
    Ok(current)
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        true
    }
}

fn candidate_binary_names(binary_name: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![
            format!("{binary_name}.exe"),
            format!("{binary_name}.cmd"),
            format!("{binary_name}.bat"),
            binary_name.to_string(),
        ]
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![binary_name.to_string()]
    }
}

fn resolve_from_env_override(env_vars: &[&str]) -> Option<PathBuf> {
    env_vars.iter().find_map(|key| {
        env::var_os(key)
            .map(PathBuf::from)
            .filter(|path| is_executable_file(path))
    })
}

fn resolve_from_known_paths(binary_name: &str) -> Option<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            if !dir.as_os_str().is_empty() && !dirs.iter().any(|existing| existing == &dir) {
                dirs.push(dir);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
            for dir in [
                ".grok/bin",
                ".local/bin",
                "bin",
                ".npm-global/bin",
                ".volta/bin",
                ".yarn/bin",
                ".bun/bin",
            ] {
                let candidate = home.join(dir);
                if !candidate.as_os_str().is_empty()
                    && !dirs.iter().any(|existing| existing == &candidate)
                {
                    dirs.push(candidate);
                }
            }
        }

        for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
            let candidate = PathBuf::from(dir);
            if !dirs.iter().any(|existing| existing == &candidate) {
                dirs.push(candidate);
            }
        }
    }

    dirs.into_iter().find_map(|dir| {
        candidate_binary_names(binary_name)
            .into_iter()
            .map(|name| dir.join(name))
            .find(|path| is_executable_file(path))
    })
}

async fn resolve_from_shell(binary_name: &str) -> Option<PathBuf> {
    let lookups = [
        (
            "/bin/zsh",
            vec!["-lc".to_string(), format!("command -v {binary_name}")],
        ),
        (
            "/bin/bash",
            vec!["-lc".to_string(), format!("command -v {binary_name}")],
        ),
    ];

    for (shell, args) in lookups {
        let output = tokio::process::Command::new(shell)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await
            .ok()?;
        if !output.status.success() {
            continue;
        }
        let path = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)?;
        if is_executable_file(&path) {
            return Some(path);
        }
    }

    None
}

pub async fn resolve_grok_executable_path(
    settings: &GrokSettings,
) -> Result<PathBuf, String> {
    if let Some(cli_path_override) = settings
        .cli_path_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let path = PathBuf::from(cli_path_override);
        if is_executable_file(&path) {
            return Ok(path);
        }
        return Err(format!(
            "配置的 grok 路径无效：{}",
            path.to_string_lossy()
        ));
    }

    if let Some(path) = resolve_from_env_override(GROK_PATH_ENV_VARS) {
        return Ok(path);
    }

    if let Some(path) = resolve_from_known_paths("grok") {
        return Ok(path);
    }

    if let Some(path) = resolve_from_shell("grok").await {
        return Ok(path);
    }

    Err(
        "未找到 grok 可执行文件。请确认已安装 Grok Build CLI，并在终端执行 `grok --version` 可以成功。"
            .to_string(),
    )
}

pub async fn read_grok_cli_version(cli_path: &Path) -> Result<String, String> {
    let output = tokio::process::Command::new(cli_path)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("检测 Grok CLI 版本失败: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Grok CLI 版本检测失败".to_string()
        } else {
            stderr
        });
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return Err("Grok CLI 版本输出为空".to_string());
    }

    Ok(version)
}

pub async fn inspect_grok_runtime<R: Runtime>(
    app: &AppHandle<R>,
    settings: &GrokSettings,
) -> GrokHealthCheck {
    let _ = app;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    match resolve_grok_executable_path(settings).await {
        Ok(cli_path) => match read_grok_cli_version(&cli_path).await {
            Ok(version) => GrokHealthCheck {
                cli_available: true,
                cli_version: Some(version.clone()),
                cli_path: Some(cli_path.to_string_lossy().to_string()),
                status_message: format!("Grok CLI 可用（{version}）"),
                checked_at: now,
            },
            Err(error) => GrokHealthCheck {
                cli_available: false,
                cli_version: None,
                cli_path: Some(cli_path.to_string_lossy().to_string()),
                status_message: format!("找到 Grok CLI，但版本检测失败：{error}"),
                checked_at: now,
            },
        },
        Err(error) => GrokHealthCheck {
            cli_available: false,
            cli_version: None,
            cli_path: None,
            status_message: error,
            checked_at: now,
        },
    }
}

#[tauri::command]
pub async fn get_grok_settings<R: Runtime>(app: AppHandle<R>) -> Result<GrokSettings, String> {
    load_grok_settings(&app)
}

#[tauri::command]
pub async fn update_grok_settings<R: Runtime>(
    app: AppHandle<R>,
    updates: UpdateGrokSettings,
) -> Result<GrokSettings, String> {
    merge_grok_settings(&app, updates)
}

#[tauri::command]
pub async fn check_grok_health<R: Runtime>(
    app: AppHandle<R>,
) -> Result<GrokHealthCheck, String> {
    let settings = load_grok_settings(&app)?;
    Ok(inspect_grok_runtime(&app, &settings).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grok_model_normalization_defaults_to_grok_4_5() {
        assert_eq!(normalize_grok_model(Some("grok-4.5")), "grok-4.5");
        assert_eq!(normalize_grok_model(Some("unknown")), "grok-4.5");
        assert_eq!(normalize_grok_model(None), "grok-4.5");
    }

    #[test]
    fn grok_effort_normalization_accepts_supported_values() {
        assert_eq!(normalize_grok_reasoning_effort(Some("low")), "low");
        assert_eq!(normalize_grok_reasoning_effort(Some("medium")), "medium");
        assert_eq!(normalize_grok_reasoning_effort(Some("high")), "high");
        assert_eq!(normalize_grok_reasoning_effort(Some("auto")), "high");
        assert_eq!(normalize_grok_reasoning_effort(None), "high");
    }
}
