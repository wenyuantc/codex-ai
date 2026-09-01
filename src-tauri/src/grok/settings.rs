use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{insert_activity_log, sqlite_pool, EXECUTION_TARGET_LOCAL};
use crate::db::models::{
    GrokCliInstallResult, GrokHealthCheck, GrokModelInfo, GrokSettings, UpdateGrokSettings,
};

const SETTINGS_FILE_NAME: &str = "grok-settings.json";
const DEFAULT_MODEL: &str = "grok-4.5";
const DEFAULT_REASONING_EFFORT: &str = "high";
const GROK_PATH_ENV_VARS: &[&str] = &["GROK_CLI_PATH", "GROK_PATH"];
/// 静态兜底列表；动态列表优先由 `list_grok_models` / `grok models` 提供。
pub const SUPPORTED_GROK_MODELS: &[&str] = &["grok-4.5"];
/// 应用侧 Grok 推理强度仅暴露 low / medium / high。
pub const SUPPORTED_GROK_REASONING_EFFORTS: &[&str] = &["low", "medium", "high"];

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

    // 接受 CLI/自定义模型 ID；空值才回落默认。动态 `grok models` 结果不再被静态白名单截断。
    value.to_string()
}

pub fn normalize_grok_reasoning_effort(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(value) if SUPPORTED_GROK_REASONING_EFFORTS.contains(&value) => value.to_string(),
        // 兼容旧 UI 的 auto：映射到默认 high
        Some("auto") => DEFAULT_REASONING_EFFORT.to_string(),
        _ => DEFAULT_REASONING_EFFORT.to_string(),
    }
}

pub fn fallback_grok_model_list() -> Vec<GrokModelInfo> {
    SUPPORTED_GROK_MODELS
        .iter()
        .map(|value| GrokModelInfo {
            value: (*value).to_string(),
            label: format_grok_model_label(value),
            is_default: *value == DEFAULT_MODEL,
        })
        .collect()
}

fn format_grok_model_label(model_id: &str) -> String {
    match model_id {
        "grok-4.5" => "Grok 4.5".to_string(),
        other => other.to_string(),
    }
}

/// 解析 `grok models` 文本输出。
pub fn parse_grok_models_output(
    output: &str,
) -> (Vec<GrokModelInfo>, Option<bool>, Option<String>) {
    let mut models = Vec::new();
    let mut auth_ok: Option<bool> = None;
    let mut default_model: Option<String> = None;
    let lower = output.to_ascii_lowercase();

    if lower.contains("logged in") || lower.contains("you are logged in") {
        auth_ok = Some(true);
    } else if lower.contains("not logged")
        || lower.contains("please login")
        || lower.contains("please log in")
        || lower.contains("grok login")
        || lower.contains("unauthor")
        || lower.contains("authentication required")
    {
        auth_ok = Some(false);
    }

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed
            .strip_prefix("Default model:")
            .or_else(|| trimmed.strip_prefix("default model:"))
        {
            let model = rest.trim();
            if !model.is_empty() {
                default_model = Some(model.to_string());
            }
            continue;
        }

        // 形如: "* grok-4.5 (default)" / "- grok-4.5"
        let candidate = trimmed.trim_start_matches(['*', '-', '•']).trim();
        if candidate.is_empty() {
            continue;
        }

        let (id_part, is_default_marker) = if let Some((id, rest)) = candidate.split_once('(') {
            (id.trim(), rest.to_ascii_lowercase().contains("default"))
        } else {
            (candidate, false)
        };

        if id_part.is_empty() || id_part.contains(' ') {
            continue;
        }
        // 模型 ID 通常是 grok-* 或含 / 的自定义 ID
        if !(id_part.starts_with("grok")
            || id_part.contains('/')
            || id_part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':')))
        {
            continue;
        }

        let is_default = is_default_marker
            || default_model
                .as_deref()
                .is_some_and(|value| value == id_part);
        if models
            .iter()
            .any(|item: &GrokModelInfo| item.value == id_part)
        {
            continue;
        }
        models.push(GrokModelInfo {
            value: id_part.to_string(),
            label: format_grok_model_label(id_part),
            is_default,
        });
    }

    if models.is_empty() {
        models = fallback_grok_model_list();
    } else if !models.iter().any(|item| item.is_default) {
        if let Some(default_model) = default_model.as_deref() {
            if let Some(item) = models.iter_mut().find(|item| item.value == default_model) {
                item.is_default = true;
            }
        } else if let Some(first) = models.first_mut() {
            first.is_default = true;
        }
    }

    (models, auth_ok, default_model)
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

fn save_grok_settings<R: Runtime>(
    app: &AppHandle<R>,
    settings: &GrokSettings,
) -> Result<(), String> {
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
        let output = crate::process_spawn::tokio_command(shell)
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

pub async fn resolve_grok_executable_path(settings: &GrokSettings) -> Result<PathBuf, String> {
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
        return Err(format!("配置的 grok 路径无效：{}", path.to_string_lossy()));
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
    let output = crate::process_spawn::tokio_command(cli_path)
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

async fn run_grok_models_command(cli_path: &Path) -> Result<String, String> {
    let output = crate::process_spawn::tokio_command(cli_path)
        .arg("models")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("执行 grok models 失败: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        let detail = [stderr.trim(), stdout.trim()]
            .into_iter()
            .find(|value| !value.is_empty())
            .unwrap_or("grok models 失败");
        return Err(detail.to_string());
    }

    Ok(if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    })
}

fn compose_local_health_message(
    version: &str,
    auth_ok: Option<bool>,
    models_error: Option<&str>,
) -> String {
    match auth_ok {
        Some(true) => format!("Grok CLI 可用（{version}），已登录"),
        Some(false) => format!(
            "Grok CLI 可用（{version}），但未登录或登录失效。请执行 `grok login`。{extra}",
            extra = models_error
                .map(|error| format!(" 详情：{error}"))
                .unwrap_or_default()
        ),
        None => format!(
            "Grok CLI 可用（{version}）{}",
            models_error
                .map(|error| format!("；登录态探测失败：{error}"))
                .unwrap_or_else(|| "；登录态未知".to_string())
        ),
    }
}

pub async fn inspect_grok_runtime<R: Runtime>(
    app: &AppHandle<R>,
    settings: &GrokSettings,
) -> GrokHealthCheck {
    let _ = app;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    match resolve_grok_executable_path(settings).await {
        Ok(cli_path) => match read_grok_cli_version(&cli_path).await {
            Ok(version) => {
                let (auth_ok, models_error) = match run_grok_models_command(&cli_path).await {
                    Ok(output) => {
                        let (_, auth_ok, _) = parse_grok_models_output(&output);
                        (auth_ok.or(Some(true)), None)
                    }
                    Err(error) => {
                        let lower = error.to_ascii_lowercase();
                        let auth_ok = if lower.contains("login")
                            || lower.contains("auth")
                            || lower.contains("unauthor")
                        {
                            Some(false)
                        } else {
                            None
                        };
                        (auth_ok, Some(error))
                    }
                };

                GrokHealthCheck {
                    cli_available: true,
                    cli_version: Some(version.clone()),
                    cli_path: Some(cli_path.to_string_lossy().to_string()),
                    auth_ok,
                    status_message: compose_local_health_message(
                        &version,
                        auth_ok,
                        models_error.as_deref(),
                    ),
                    checked_at: now,
                }
            }
            Err(error) => GrokHealthCheck {
                cli_available: false,
                cli_version: None,
                cli_path: Some(cli_path.to_string_lossy().to_string()),
                auth_ok: None,
                status_message: format!("找到 Grok CLI，但版本检测失败：{error}"),
                checked_at: now,
            },
        },
        Err(error) => GrokHealthCheck {
            cli_available: false,
            cli_version: None,
            cli_path: None,
            auth_ok: None,
            status_message: error,
            checked_at: now,
        },
    }
}

async fn run_official_grok_cli_install() -> Result<(), String> {
    let output = if cfg!(target_os = "windows") {
        crate::process_spawn::tokio_command("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "irm https://x.ai/cli/install.ps1 | iex",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|error| format!("启动 Grok CLI 安装失败: {error}"))?
    } else {
        crate::process_spawn::tokio_command("/bin/bash")
            .args(["-lc", "curl -fsSL https://x.ai/cli/install.sh | bash"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|error| format!("启动 Grok CLI 安装失败: {error}"))?
    };

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = [stderr.as_str(), stdout.as_str()]
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or("安装脚本执行失败");
    Err(format!("安装 Grok CLI 失败：{detail}"))
}

/// 在忽略 `cli_path_override` 的前提下解析官方默认路径（用于 override 冲突时的提示）。
async fn resolve_grok_default_executable_path() -> Option<PathBuf> {
    if let Some(path) = resolve_from_env_override(GROK_PATH_ENV_VARS) {
        return Some(path);
    }
    if let Some(path) = resolve_from_known_paths("grok") {
        return Some(path);
    }
    resolve_from_shell("grok").await
}

pub async fn install_grok_cli_runtime<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<GrokCliInstallResult, String> {
    let settings = load_grok_settings(app)?;
    run_official_grok_cli_install().await?;

    let override_invalid = settings
        .cli_path_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| !is_executable_file(Path::new(value)))
        .unwrap_or(false);

    let (cli_path, cli_version, cli_available, message) = match resolve_grok_executable_path(
        &settings,
    )
    .await
    {
        Ok(path) => match read_grok_cli_version(&path).await {
            Ok(version) => (
                Some(path.to_string_lossy().to_string()),
                Some(version.clone()),
                true,
                format!("Grok CLI 安装完成，版本 {version}"),
            ),
            Err(error) => (
                Some(path.to_string_lossy().to_string()),
                None,
                false,
                format!("Grok CLI 安装完成，但版本检测失败：{error}"),
            ),
        },
        Err(resolve_error) if override_invalid => {
            match resolve_grok_default_executable_path().await {
                    Some(default_path) => {
                        let version = read_grok_cli_version(&default_path).await.ok();
                        let version_text = version
                            .as_deref()
                            .map(|value| format!("，默认路径版本 {value}"))
                            .unwrap_or_default();
                        (
                            Some(default_path.to_string_lossy().to_string()),
                            version,
                            true,
                            format!(
                                "Grok CLI 已安装到默认位置{version_text}，但当前覆盖路径不可用，请清空或修正 CLI 路径覆盖。原始错误：{resolve_error}"
                            ),
                        )
                    }
                    None => (
                        None,
                        None,
                        false,
                        format!(
                            "Grok CLI 安装脚本已完成，但未能解析可执行文件。请检查 PATH 或 CLI 路径覆盖。{resolve_error}"
                        ),
                    ),
                }
        }
        Err(error) => (
            None,
            None,
            false,
            format!("Grok CLI 安装脚本已完成，但未能解析可执行文件：{error}"),
        ),
    };

    if cli_available {
        if let Ok(pool) = sqlite_pool(app).await {
            let detail = cli_path
                .as_deref()
                .or(cli_version.as_deref())
                .unwrap_or("local");
            let _ =
                insert_activity_log(&pool, "grok_cli_installed", detail, None, None, None).await;
        }
    }

    if !cli_available {
        return Err(message);
    }

    Ok(GrokCliInstallResult {
        execution_target: EXECUTION_TARGET_LOCAL.to_string(),
        ssh_config_id: None,
        target_host_label: None,
        cli_available: true,
        cli_version,
        cli_path,
        message,
    })
}

#[tauri::command]
pub async fn install_grok_cli<R: Runtime>(
    app: AppHandle<R>,
) -> Result<GrokCliInstallResult, String> {
    install_grok_cli_runtime(&app).await
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
pub async fn check_grok_health<R: Runtime>(app: AppHandle<R>) -> Result<GrokHealthCheck, String> {
    let settings = load_grok_settings(&app)?;
    Ok(inspect_grok_runtime(&app, &settings).await)
}

#[tauri::command]
pub async fn list_grok_models<R: Runtime>(app: AppHandle<R>) -> Result<Vec<GrokModelInfo>, String> {
    let settings = load_grok_settings(&app)?;
    let cli_path = match resolve_grok_executable_path(&settings).await {
        Ok(path) => path,
        Err(_) => return Ok(fallback_grok_model_list()),
    };

    match run_grok_models_command(&cli_path).await {
        Ok(output) => {
            let (models, _, _) = parse_grok_models_output(&output);
            Ok(models)
        }
        Err(_) => Ok(fallback_grok_model_list()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grok_model_normalization_defaults_to_grok_4_5() {
        assert_eq!(normalize_grok_model(Some("grok-4.5")), "grok-4.5");
        assert_eq!(normalize_grok_model(Some("custom/model")), "custom/model");
        assert_eq!(normalize_grok_model(None), "grok-4.5");
        assert_eq!(normalize_grok_model(Some("  ")), "grok-4.5");
    }

    #[test]
    fn grok_effort_normalization_accepts_supported_values() {
        assert_eq!(normalize_grok_reasoning_effort(Some("low")), "low");
        assert_eq!(normalize_grok_reasoning_effort(Some("medium")), "medium");
        assert_eq!(normalize_grok_reasoning_effort(Some("high")), "high");
        assert_eq!(normalize_grok_reasoning_effort(Some("xhigh")), "high");
        assert_eq!(normalize_grok_reasoning_effort(Some("auto")), "high");
        assert_eq!(normalize_grok_reasoning_effort(None), "high");
    }

    #[test]
    fn parse_grok_models_output_extracts_login_and_models() {
        let output = r#"
You are logged in with grok.com.

Default model: grok-4.5

Available models:
  * grok-4.5 (default)
  * grok-code-fast
"#;
        let (models, auth_ok, default_model) = parse_grok_models_output(output);
        assert_eq!(auth_ok, Some(true));
        assert_eq!(default_model.as_deref(), Some("grok-4.5"));
        assert!(models
            .iter()
            .any(|item| item.value == "grok-4.5" && item.is_default));
        assert!(models.iter().any(|item| item.value == "grok-code-fast"));
    }
}
