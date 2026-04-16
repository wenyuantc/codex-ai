use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{insert_activity_log, normalize_optional_text, sqlite_pool};
use crate::codex::{new_node_command, new_npm_command};
use crate::db::models::{
    CodexSdkInstallResult, CodexSettings, CodexSettingsDocument, RemoteCodexSettingsPayload,
    UpdateCodexSettings,
};

const SETTINGS_FILE_NAME: &str = "codex-settings.json";
const SDK_RUNTIME_DIR_NAME: &str = "codex-sdk-runtime";
const SDK_BRIDGE_FILE_NAME: &str = "sdk-bridge.mjs";
const SDK_PACKAGE_NAME: &str = "@openai/codex-sdk";
const SDK_CLI_PACKAGE_NAME: &str = "@openai/codex";
const ONE_SHOT_PROVIDER_SDK: &str = "sdk";
const ONE_SHOT_PROVIDER_EXEC: &str = "exec";
const MINIMUM_NODE_MAJOR: u32 = 18;
const DEFAULT_ONE_SHOT_MODEL: &str = "gpt-5.4";
const DEFAULT_ONE_SHOT_REASONING_EFFORT: &str = "high";
const DEFAULT_TASK_AUTOMATION_MAX_FIX_ROUNDS: i32 = 3;
const DEFAULT_TASK_AUTOMATION_FAILURE_STRATEGY: &str = "blocked";
const SUPPORTED_MODELS: &[&str] = &["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex", "gpt-5.2"];
const SUPPORTED_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];
const SUPPORTED_TASK_AUTOMATION_FAILURE_STRATEGIES: &[&str] = &["blocked", "manual_control"];

#[derive(Debug, Clone)]
pub struct SdkRuntimeHealth {
    pub node_available: bool,
    pub node_version: Option<String>,
    pub sdk_installed: bool,
    pub sdk_version: Option<String>,
    pub task_execution_effective_provider: String,
    pub one_shot_effective_provider: String,
    pub status_message: String,
}

#[derive(Debug, Deserialize)]
struct PackageMetadata {
    version: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RawCodexSettings {
    #[serde(default)]
    sdk_enabled: Option<bool>,
    #[serde(default)]
    task_sdk_enabled: Option<bool>,
    #[serde(default)]
    one_shot_sdk_enabled: Option<bool>,
    #[serde(default)]
    one_shot_model: Option<String>,
    #[serde(default)]
    one_shot_reasoning_effort: Option<String>,
    #[serde(default)]
    task_automation_default_enabled: Option<bool>,
    #[serde(default)]
    task_automation_max_fix_rounds: Option<i32>,
    #[serde(default)]
    task_automation_failure_strategy: Option<String>,
    #[serde(default)]
    node_path_override: Option<String>,
    #[serde(default)]
    sdk_install_dir: Option<String>,
    #[serde(default)]
    one_shot_preferred_provider: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RawCodexSettingsDocument {
    #[serde(default)]
    local: Option<RawCodexSettings>,
    #[serde(default)]
    remote_profiles: HashMap<String, RawCodexSettings>,
}

fn normalize_one_shot_model(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(value) if SUPPORTED_MODELS.contains(&value) => value.to_string(),
        _ => DEFAULT_ONE_SHOT_MODEL.to_string(),
    }
}

fn normalize_one_shot_reasoning_effort(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(value) if SUPPORTED_REASONING_EFFORTS.contains(&value) => value.to_string(),
        _ => DEFAULT_ONE_SHOT_REASONING_EFFORT.to_string(),
    }
}

fn normalize_task_automation_max_fix_rounds(value: Option<i32>) -> i32 {
    match value {
        Some(value) if (1..=10).contains(&value) => value,
        _ => DEFAULT_TASK_AUTOMATION_MAX_FIX_ROUNDS,
    }
}

fn normalize_task_automation_failure_strategy(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(value) if SUPPORTED_TASK_AUTOMATION_FAILURE_STRATEGIES.contains(&value) => {
            value.to_string()
        }
        _ => DEFAULT_TASK_AUTOMATION_FAILURE_STRATEGY.to_string(),
    }
}

fn app_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法读取应用配置目录: {error}"))
}

fn settings_file_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join(SETTINGS_FILE_NAME))
}

fn default_sdk_install_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join(SDK_RUNTIME_DIR_NAME))
}

fn default_remote_sdk_install_dir<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
) -> Result<PathBuf, String> {
    Ok(default_sdk_install_dir(app)?
        .join("remote")
        .join(ssh_config_id))
}

fn default_codex_settings<R: Runtime>(app: &AppHandle<R>) -> Result<CodexSettings, String> {
    let default_install_dir = default_sdk_install_dir(app)?;
    Ok(CodexSettings {
        task_sdk_enabled: false,
        one_shot_sdk_enabled: false,
        one_shot_model: DEFAULT_ONE_SHOT_MODEL.to_string(),
        one_shot_reasoning_effort: DEFAULT_ONE_SHOT_REASONING_EFFORT.to_string(),
        task_automation_default_enabled: false,
        task_automation_max_fix_rounds: DEFAULT_TASK_AUTOMATION_MAX_FIX_ROUNDS,
        task_automation_failure_strategy: DEFAULT_TASK_AUTOMATION_FAILURE_STRATEGY.to_string(),
        node_path_override: None,
        sdk_install_dir: default_install_dir.to_string_lossy().to_string(),
        one_shot_preferred_provider: ONE_SHOT_PROVIDER_SDK.to_string(),
    })
}

fn normalize_settings(settings: CodexSettings, default_install_dir: &Path) -> CodexSettings {
    CodexSettings {
        task_sdk_enabled: settings.task_sdk_enabled,
        one_shot_sdk_enabled: settings.one_shot_sdk_enabled,
        one_shot_model: normalize_one_shot_model(Some(&settings.one_shot_model)),
        one_shot_reasoning_effort: normalize_one_shot_reasoning_effort(Some(
            &settings.one_shot_reasoning_effort,
        )),
        task_automation_default_enabled: settings.task_automation_default_enabled,
        task_automation_max_fix_rounds: normalize_task_automation_max_fix_rounds(Some(
            settings.task_automation_max_fix_rounds,
        )),
        task_automation_failure_strategy: normalize_task_automation_failure_strategy(Some(
            &settings.task_automation_failure_strategy,
        )),
        node_path_override: normalize_optional_text(settings.node_path_override.as_deref()),
        sdk_install_dir: normalize_optional_text(Some(&settings.sdk_install_dir))
            .unwrap_or_else(|| default_install_dir.to_string_lossy().to_string()),
        one_shot_preferred_provider: ONE_SHOT_PROVIDER_SDK.to_string(),
    }
}

fn normalize_raw_settings(raw: RawCodexSettings, default_install_dir: &Path) -> CodexSettings {
    let legacy_sdk_enabled = raw.sdk_enabled.unwrap_or(false);

    CodexSettings {
        task_sdk_enabled: raw.task_sdk_enabled.unwrap_or(legacy_sdk_enabled),
        one_shot_sdk_enabled: raw.one_shot_sdk_enabled.unwrap_or(legacy_sdk_enabled),
        one_shot_model: normalize_one_shot_model(raw.one_shot_model.as_deref()),
        one_shot_reasoning_effort: normalize_one_shot_reasoning_effort(
            raw.one_shot_reasoning_effort.as_deref(),
        ),
        task_automation_default_enabled: raw.task_automation_default_enabled.unwrap_or(false),
        task_automation_max_fix_rounds: normalize_task_automation_max_fix_rounds(
            raw.task_automation_max_fix_rounds,
        ),
        task_automation_failure_strategy: normalize_task_automation_failure_strategy(
            raw.task_automation_failure_strategy.as_deref(),
        ),
        node_path_override: normalize_optional_text(raw.node_path_override.as_deref()),
        sdk_install_dir: normalize_optional_text(raw.sdk_install_dir.as_deref())
            .unwrap_or_else(|| default_install_dir.to_string_lossy().to_string()),
        one_shot_preferred_provider: raw
            .one_shot_preferred_provider
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| ONE_SHOT_PROVIDER_SDK.to_string()),
    }
}

fn normalize_settings_document<R: Runtime>(
    app: &AppHandle<R>,
    raw: RawCodexSettingsDocument,
) -> Result<CodexSettingsDocument, String> {
    let default_install_dir = default_sdk_install_dir(app)?;
    let local = normalize_raw_settings(raw.local.unwrap_or_default(), &default_install_dir);
    let mut remote_profiles = HashMap::new();
    for (ssh_config_id, profile) in raw.remote_profiles {
        let install_dir = default_remote_sdk_install_dir(app, &ssh_config_id)?;
        remote_profiles.insert(ssh_config_id, normalize_raw_settings(profile, &install_dir));
    }
    Ok(CodexSettingsDocument {
        local,
        remote_profiles,
    })
}

pub fn load_codex_settings_document<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<CodexSettingsDocument, String> {
    let path = settings_file_path(app)?;

    if !path.exists() {
        return Ok(CodexSettingsDocument {
            local: default_codex_settings(app)?,
            remote_profiles: HashMap::new(),
        });
    }

    let raw = fs::read_to_string(&path).map_err(|error| format!("读取 Codex 设置失败: {error}"))?;
    let json_value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("解析 Codex 设置失败: {error}"))?;
    if json_value.get("local").is_some() || json_value.get("remote_profiles").is_some() {
        let parsed_document = serde_json::from_value::<RawCodexSettingsDocument>(json_value)
            .map_err(|error| format!("解析 Codex 设置文档失败: {error}"))?;
        return normalize_settings_document(app, parsed_document);
    }
    let legacy = serde_json::from_value::<RawCodexSettings>(json_value)
        .map_err(|error| format!("解析 Codex 旧版设置失败: {error}"))?;
    let default_install_dir = default_sdk_install_dir(app)?;
    Ok(CodexSettingsDocument {
        local: normalize_raw_settings(legacy, &default_install_dir),
        remote_profiles: HashMap::new(),
    })
}

fn save_codex_settings_document<R: Runtime>(
    app: &AppHandle<R>,
    document: &CodexSettingsDocument,
) -> Result<(), String> {
    let config_dir = app_config_dir(app)?;
    fs::create_dir_all(&config_dir).map_err(|error| format!("创建应用配置目录失败: {error}"))?;

    let default_install_dir = default_sdk_install_dir(app)?;
    let mut remote_profiles = HashMap::new();
    for (ssh_config_id, settings) in &document.remote_profiles {
        let install_dir = default_remote_sdk_install_dir(app, ssh_config_id)?;
        remote_profiles.insert(
            ssh_config_id.clone(),
            normalize_settings(settings.clone(), &install_dir),
        );
    }

    let normalized = CodexSettingsDocument {
        local: normalize_settings(document.local.clone(), &default_install_dir),
        remote_profiles,
    };
    let raw = serde_json::to_string_pretty(&normalized)
        .map_err(|error| format!("序列化 Codex 设置失败: {error}"))?;
    fs::write(settings_file_path(app)?, raw)
        .map_err(|error| format!("写入 Codex 设置失败: {error}"))
}

pub fn load_codex_settings<R: Runtime>(app: &AppHandle<R>) -> Result<CodexSettings, String> {
    Ok(load_codex_settings_document(app)?.local)
}

pub fn load_remote_codex_settings<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
) -> Result<CodexSettings, String> {
    let mut document = load_codex_settings_document(app)?;
    if let Some(settings) = document.remote_profiles.remove(ssh_config_id) {
        return Ok(settings);
    }

    let install_dir = default_remote_sdk_install_dir(app, ssh_config_id)?;
    Ok(normalize_settings(
        default_codex_settings(app)?,
        &install_dir,
    ))
}

pub fn save_codex_settings<R: Runtime>(
    app: &AppHandle<R>,
    settings: &CodexSettings,
) -> Result<(), String> {
    let mut document = load_codex_settings_document(app)?;
    document.local = settings.clone();
    save_codex_settings_document(app, &document)
}

pub fn merge_codex_settings<R: Runtime>(
    app: &AppHandle<R>,
    updates: UpdateCodexSettings,
) -> Result<CodexSettings, String> {
    let mut settings = load_codex_settings(app)?;

    if let Some(task_sdk_enabled) = updates.task_sdk_enabled {
        settings.task_sdk_enabled = task_sdk_enabled;
    }

    if let Some(one_shot_sdk_enabled) = updates.one_shot_sdk_enabled {
        settings.one_shot_sdk_enabled = one_shot_sdk_enabled;
    }

    if let Some(one_shot_model) = updates.one_shot_model {
        settings.one_shot_model = normalize_one_shot_model(Some(&one_shot_model));
    }

    if let Some(one_shot_reasoning_effort) = updates.one_shot_reasoning_effort {
        settings.one_shot_reasoning_effort =
            normalize_one_shot_reasoning_effort(Some(&one_shot_reasoning_effort));
    }

    if let Some(task_automation_default_enabled) = updates.task_automation_default_enabled {
        settings.task_automation_default_enabled = task_automation_default_enabled;
    }

    if let Some(task_automation_max_fix_rounds) = updates.task_automation_max_fix_rounds {
        settings.task_automation_max_fix_rounds =
            normalize_task_automation_max_fix_rounds(Some(task_automation_max_fix_rounds));
    }

    if let Some(task_automation_failure_strategy) = updates.task_automation_failure_strategy {
        settings.task_automation_failure_strategy =
            normalize_task_automation_failure_strategy(Some(&task_automation_failure_strategy));
    }

    if let Some(node_path_override) = updates.node_path_override {
        settings.node_path_override = normalize_optional_text(node_path_override.as_deref());
    }

    if let Some(sdk_install_dir) = updates.sdk_install_dir {
        settings.sdk_install_dir = normalize_optional_text(sdk_install_dir.as_deref())
            .unwrap_or_else(|| {
                default_sdk_install_dir(app)
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|_| String::new())
            });
    }

    save_codex_settings(app, &settings)?;
    load_codex_settings(app)
}

pub fn merge_remote_codex_settings<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    updates: UpdateCodexSettings,
) -> Result<CodexSettings, String> {
    let mut document = load_codex_settings_document(app)?;
    let mut settings = document
        .remote_profiles
        .remove(ssh_config_id)
        .unwrap_or(load_remote_codex_settings(app, ssh_config_id)?);

    if let Some(task_sdk_enabled) = updates.task_sdk_enabled {
        settings.task_sdk_enabled = task_sdk_enabled;
    }
    if let Some(one_shot_sdk_enabled) = updates.one_shot_sdk_enabled {
        settings.one_shot_sdk_enabled = one_shot_sdk_enabled;
    }
    if let Some(one_shot_model) = updates.one_shot_model {
        settings.one_shot_model = normalize_one_shot_model(Some(&one_shot_model));
    }
    if let Some(one_shot_reasoning_effort) = updates.one_shot_reasoning_effort {
        settings.one_shot_reasoning_effort =
            normalize_one_shot_reasoning_effort(Some(&one_shot_reasoning_effort));
    }
    if let Some(task_automation_default_enabled) = updates.task_automation_default_enabled {
        settings.task_automation_default_enabled = task_automation_default_enabled;
    }
    if let Some(task_automation_max_fix_rounds) = updates.task_automation_max_fix_rounds {
        settings.task_automation_max_fix_rounds =
            normalize_task_automation_max_fix_rounds(Some(task_automation_max_fix_rounds));
    }
    if let Some(task_automation_failure_strategy) = updates.task_automation_failure_strategy {
        settings.task_automation_failure_strategy =
            normalize_task_automation_failure_strategy(Some(&task_automation_failure_strategy));
    }
    if let Some(node_path_override) = updates.node_path_override {
        settings.node_path_override = normalize_optional_text(node_path_override.as_deref());
    }
    if let Some(sdk_install_dir) = updates.sdk_install_dir {
        settings.sdk_install_dir = normalize_optional_text(sdk_install_dir.as_deref())
            .unwrap_or_else(|| {
                default_remote_sdk_install_dir(app, ssh_config_id)
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|_| String::new())
            });
    }

    document
        .remote_profiles
        .insert(ssh_config_id.to_string(), settings.clone());
    save_codex_settings_document(app, &document)?;
    load_remote_codex_settings(app, ssh_config_id)
}

fn npm_package_dir(install_dir: &Path, package_name: &str) -> PathBuf {
    package_name
        .split('/')
        .fold(install_dir.join("node_modules"), |dir, segment| {
            dir.join(segment)
        })
}

fn npm_package_json_path(install_dir: &Path, package_name: &str) -> PathBuf {
    npm_package_dir(install_dir, package_name).join("package.json")
}

fn sdk_package_json_path(install_dir: &Path) -> PathBuf {
    npm_package_json_path(install_dir, SDK_PACKAGE_NAME)
}

fn sdk_cli_package_json_path(install_dir: &Path) -> PathBuf {
    npm_package_json_path(install_dir, SDK_CLI_PACKAGE_NAME)
}

fn sdk_platform_package_for_target(
    target_os: &str,
    target_arch: &str,
) -> Option<(&'static str, &'static str, &'static str)> {
    match (target_os, target_arch) {
        ("windows", "x86_64") => Some((
            "@openai/codex-win32-x64",
            "x86_64-pc-windows-msvc",
            "codex.exe",
        )),
        ("windows", "aarch64") => Some((
            "@openai/codex-win32-arm64",
            "aarch64-pc-windows-msvc",
            "codex.exe",
        )),
        ("macos", "x86_64") => Some(("@openai/codex-darwin-x64", "x86_64-apple-darwin", "codex")),
        ("macos", "aarch64") => Some((
            "@openai/codex-darwin-arm64",
            "aarch64-apple-darwin",
            "codex",
        )),
        ("linux", "x86_64") => Some((
            "@openai/codex-linux-x64",
            "x86_64-unknown-linux-musl",
            "codex",
        )),
        ("linux", "aarch64") => Some((
            "@openai/codex-linux-arm64",
            "aarch64-unknown-linux-musl",
            "codex",
        )),
        _ => None,
    }
}

fn current_sdk_platform_package() -> Option<(&'static str, &'static str, &'static str)> {
    sdk_platform_package_for_target(std::env::consts::OS, std::env::consts::ARCH)
}

fn sdk_platform_binary_path(install_dir: &Path) -> Option<PathBuf> {
    let (package_name, target_triple, binary_name) = current_sdk_platform_package()?;
    Some(
        npm_package_dir(install_dir, package_name)
            .join("vendor")
            .join(target_triple)
            .join("codex")
            .join(binary_name),
    )
}

fn sdk_cli_binaries_available(install_dir: &Path) -> bool {
    sdk_cli_package_json_path(install_dir).exists()
        && sdk_platform_binary_path(install_dir)
            .map(|path| path.exists())
            .unwrap_or(false)
}

pub fn sdk_bridge_script_path(install_dir: &Path) -> PathBuf {
    install_dir.join(SDK_BRIDGE_FILE_NAME)
}

pub fn ensure_sdk_runtime_layout(install_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(install_dir).map_err(|error| format!("创建 SDK 运行目录失败: {error}"))?;

    let package_json_path = install_dir.join("package.json");
    if !package_json_path.exists() {
        let package_json = serde_json::json!({
            "name": "codex-ai-sdk-runtime",
            "private": true,
            "type": "module"
        });
        fs::write(
            &package_json_path,
            serde_json::to_string_pretty(&package_json)
                .map_err(|error| format!("序列化 SDK package.json 失败: {error}"))?,
        )
        .map_err(|error| format!("写入 SDK package.json 失败: {error}"))?;
    }

    let bridge_path = sdk_bridge_script_path(install_dir);
    let bridge_content = include_str!("sdk_bridge.mjs");
    let should_write_bridge = match fs::read_to_string(&bridge_path) {
        Ok(existing) => existing != bridge_content,
        Err(_) => true,
    };
    if should_write_bridge {
        fs::write(&bridge_path, bridge_content)
            .map_err(|error| format!("写入 SDK bridge 脚本失败: {error}"))?;
    }

    Ok(())
}

pub fn read_sdk_version_from_dir(install_dir: &Path) -> Result<Option<String>, String> {
    let package_json_path = sdk_package_json_path(install_dir);
    if !package_json_path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&package_json_path)
        .map_err(|error| format!("读取 Codex SDK 版本失败: {error}"))?;
    let metadata = serde_json::from_str::<PackageMetadata>(&raw)
        .map_err(|error| format!("解析 Codex SDK 版本失败: {error}"))?;

    Ok(metadata.version.filter(|value| !value.trim().is_empty()))
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

fn ensure_supported_node_version(version: &str) -> Result<(), String> {
    match parse_node_major_version(version) {
        Some(major) if major >= MINIMUM_NODE_MAJOR => Ok(()),
        Some(major) => Err(format!(
            "Node 版本过低（当前 {major}），Codex SDK 需要 Node.js {MINIMUM_NODE_MAJOR}+"
        )),
        None => Err(format!("无法解析 Node 版本：{version}")),
    }
}

pub fn determine_effective_provider(
    sdk_enabled: bool,
    node_available: bool,
    sdk_installed: bool,
) -> &'static str {
    if sdk_enabled && node_available && sdk_installed {
        ONE_SHOT_PROVIDER_SDK
    } else {
        ONE_SHOT_PROVIDER_EXEC
    }
}

pub async fn inspect_sdk_runtime<R: Runtime>(
    app: &AppHandle<R>,
    settings: &CodexSettings,
) -> SdkRuntimeHealth {
    let node_result = read_node_version(settings.node_path_override.as_deref()).await;
    let node_available = node_result.is_ok();
    let node_version = node_result.as_ref().ok().cloned();
    let node_error = node_result.err();
    let node_supported = node_version
        .as_deref()
        .map(ensure_supported_node_version)
        .transpose()
        .is_ok();

    let install_dir = Path::new(&settings.sdk_install_dir);
    let sdk_result = read_sdk_version_from_dir(install_dir);
    let sdk_version = sdk_result.as_ref().ok().and_then(Clone::clone);
    let sdk_installed = sdk_version.is_some();
    let sdk_error = sdk_result.err();
    let cli_binaries_available = sdk_cli_binaries_available(install_dir);

    let task_execution_effective_provider = determine_effective_provider(
        settings.task_sdk_enabled,
        node_available && node_supported,
        sdk_installed && cli_binaries_available,
    )
    .to_string();

    let one_shot_effective_provider = determine_effective_provider(
        settings.one_shot_sdk_enabled,
        node_available && node_supported,
        sdk_installed && cli_binaries_available,
    )
    .to_string();

    let status_message = if !settings.task_sdk_enabled && !settings.one_shot_sdk_enabled {
        "Codex SDK 未启用，任务运行与一次性 AI 将使用 codex exec".to_string()
    } else if let Some(error) = node_error {
        format!("Node 不可用，已回退到 codex exec：{error}")
    } else if let Some(version) = node_version.as_deref() {
        if let Err(error) = ensure_supported_node_version(version) {
            format!("{error}，已回退到 codex exec")
        } else if let Some(error) = sdk_error {
            format!("Codex SDK 状态异常，已回退到 codex exec：{error}")
        } else if !sdk_installed {
            "Codex SDK 未安装，已回退到 codex exec".to_string()
        } else if !cli_binaries_available {
            match current_sdk_platform_package() {
                Some((package_name, _, _)) => format!(
                    "Codex SDK 缺少当前平台 CLI 二进制（{package_name}），已回退到 codex exec"
                ),
                None => {
                    "当前系统架构缺少受支持的 Codex CLI 平台包，已回退到 codex exec".to_string()
                }
            }
        } else {
            format!("Codex SDK 已就绪，任务运行与一次性 AI 将优先使用 SDK（Node {version}）")
        }
    } else {
        "Codex SDK 状态未知，已回退到 codex exec".to_string()
    };

    let _ = app;

    SdkRuntimeHealth {
        node_available,
        node_version,
        sdk_installed,
        sdk_version,
        task_execution_effective_provider,
        one_shot_effective_provider,
        status_message,
    }
}

pub async fn install_codex_sdk_runtime<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<CodexSdkInstallResult, String> {
    let settings = load_codex_settings(app)?;
    let install_dir = PathBuf::from(&settings.sdk_install_dir);

    ensure_sdk_runtime_layout(&install_dir)?;
    save_codex_settings(app, &settings)?;

    let node_version = read_node_version(settings.node_path_override.as_deref()).await?;
    ensure_supported_node_version(&node_version)?;

    let mut npm_command = new_npm_command(settings.node_path_override.as_deref()).await?;
    let output = npm_command
        .current_dir(&install_dir)
        .arg("install")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg("--include=optional")
        .arg(SDK_PACKAGE_NAME)
        .arg(SDK_CLI_PACKAGE_NAME)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("安装 Codex SDK 失败: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "安装 Codex SDK 失败".to_string()
        } else {
            format!("安装 Codex SDK 失败：{stderr}")
        });
    }

    let sdk_version = read_sdk_version_from_dir(&install_dir)?
        .ok_or_else(|| "Codex SDK 已安装，但未能读取版本信息".to_string())?;
    if !sdk_cli_binaries_available(&install_dir) {
        let message = match current_sdk_platform_package() {
            Some((package_name, _, _)) => {
                format!("Codex SDK 安装不完整，缺少当前平台 CLI 二进制包：{package_name}")
            }
            None => "Codex SDK 安装完成，但当前系统架构没有匹配的 Codex CLI 平台包".to_string(),
        };
        return Err(message);
    }

    Ok(CodexSdkInstallResult {
        execution_target: "local".to_string(),
        ssh_config_id: None,
        target_host_label: None,
        sdk_installed: true,
        sdk_version: Some(sdk_version),
        install_dir: install_dir.to_string_lossy().to_string(),
        node_version: Some(node_version),
        message: "Codex SDK 安装完成".to_string(),
    })
}

#[tauri::command]
pub async fn get_codex_settings<R: Runtime>(app: AppHandle<R>) -> Result<CodexSettings, String> {
    load_codex_settings(&app)
}

#[tauri::command]
pub async fn get_remote_codex_settings<R: Runtime>(
    app: AppHandle<R>,
    ssh_config_id: String,
) -> Result<CodexSettings, String> {
    load_remote_codex_settings(&app, &ssh_config_id)
}

#[tauri::command]
pub async fn update_codex_settings<R: Runtime>(
    app: AppHandle<R>,
    updates: UpdateCodexSettings,
) -> Result<CodexSettings, String> {
    let previous = load_codex_settings(&app)?;
    let next = merge_codex_settings(&app, updates)?;

    if previous.task_automation_default_enabled != next.task_automation_default_enabled
        || previous.task_automation_max_fix_rounds != next.task_automation_max_fix_rounds
        || previous.task_automation_failure_strategy != next.task_automation_failure_strategy
    {
        if let Ok(pool) = sqlite_pool(&app).await {
            let _ = insert_activity_log(
                &pool,
                "task_automation_settings_updated",
                &format!(
                    "新建任务默认自动质控：{}；最大自动修复轮次：{}；失败后：{}",
                    if next.task_automation_default_enabled {
                        "开启"
                    } else {
                        "关闭"
                    },
                    next.task_automation_max_fix_rounds,
                    if next.task_automation_failure_strategy == "manual_control" {
                        "转人工"
                    } else {
                        "转阻塞"
                    }
                ),
                None,
                None,
                None,
            )
            .await;
        }
    }

    Ok(next)
}

#[tauri::command]
pub async fn update_remote_codex_settings<R: Runtime>(
    app: AppHandle<R>,
    payload: RemoteCodexSettingsPayload,
) -> Result<CodexSettings, String> {
    merge_remote_codex_settings(&app, &payload.ssh_config_id, payload.updates)
}

#[tauri::command]
pub async fn install_codex_sdk<R: Runtime>(
    app: AppHandle<R>,
) -> Result<CodexSdkInstallResult, String> {
    install_codex_sdk_runtime(&app).await
}

#[cfg(test)]
mod tests {
    use super::{
        determine_effective_provider, normalize_raw_settings,
        normalize_task_automation_failure_strategy, normalize_task_automation_max_fix_rounds,
        parse_node_major_version, read_sdk_version_from_dir, sdk_platform_package_for_target,
        RawCodexSettings,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_dir() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "codex-ai-sdk-settings-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time drift")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    #[test]
    fn parses_sdk_version_from_package_json() {
        let base = create_temp_dir();
        let package_json = base.join("node_modules").join("@openai").join("codex-sdk");
        fs::create_dir_all(&package_json).expect("create sdk package dir");
        fs::write(
            package_json.join("package.json"),
            r#"{"name":"@openai/codex-sdk","version":"1.2.3"}"#,
        )
        .expect("write package json");

        let version = read_sdk_version_from_dir(&base).expect("read sdk version");
        assert_eq!(version.as_deref(), Some("1.2.3"));

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn resolves_platform_package_for_supported_targets() {
        assert_eq!(
            sdk_platform_package_for_target("windows", "x86_64"),
            Some((
                "@openai/codex-win32-x64",
                "x86_64-pc-windows-msvc",
                "codex.exe"
            ))
        );
        assert_eq!(
            sdk_platform_package_for_target("macos", "aarch64"),
            Some((
                "@openai/codex-darwin-arm64",
                "aarch64-apple-darwin",
                "codex"
            ))
        );
    }

    #[test]
    fn rejects_unsupported_platform_targets() {
        assert_eq!(sdk_platform_package_for_target("windows", "x86"), None);
        assert_eq!(sdk_platform_package_for_target("freebsd", "x86_64"), None);
    }

    #[test]
    fn sdk_provider_requires_enabled_and_healthy_runtime() {
        assert_eq!(determine_effective_provider(true, true, true), "sdk");
        assert_eq!(determine_effective_provider(true, false, true), "exec");
        assert_eq!(determine_effective_provider(false, true, true), "exec");
    }

    #[test]
    fn parses_node_major_version_from_standard_output() {
        assert_eq!(parse_node_major_version("v20.11.1"), Some(20));
        assert_eq!(parse_node_major_version("18.19.0"), Some(18));
        assert_eq!(parse_node_major_version("invalid"), None);
    }

    #[test]
    fn legacy_sdk_enabled_is_mapped_to_both_new_switches() {
        let base = create_temp_dir();
        let normalized = normalize_raw_settings(
            RawCodexSettings {
                sdk_enabled: Some(true),
                ..RawCodexSettings::default()
            },
            &base,
        );

        assert!(normalized.task_sdk_enabled);
        assert!(normalized.one_shot_sdk_enabled);
        assert_eq!(normalized.one_shot_model, "gpt-5.4");
        assert_eq!(normalized.one_shot_reasoning_effort, "high");

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn invalid_one_shot_model_and_reasoning_fall_back_to_defaults() {
        let base = create_temp_dir();
        let normalized = normalize_raw_settings(
            RawCodexSettings {
                one_shot_model: Some("unknown-model".to_string()),
                one_shot_reasoning_effort: Some("extreme".to_string()),
                ..RawCodexSettings::default()
            },
            &base,
        );

        assert_eq!(normalized.one_shot_model, "gpt-5.4");
        assert_eq!(normalized.one_shot_reasoning_effort, "high");

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn invalid_task_automation_settings_fall_back_to_defaults() {
        let base = create_temp_dir();
        let normalized = normalize_raw_settings(
            RawCodexSettings {
                task_automation_default_enabled: Some(true),
                task_automation_max_fix_rounds: Some(0),
                task_automation_failure_strategy: Some("something-else".to_string()),
                ..RawCodexSettings::default()
            },
            &base,
        );

        assert!(normalized.task_automation_default_enabled);
        assert_eq!(normalized.task_automation_max_fix_rounds, 3);
        assert_eq!(normalized.task_automation_failure_strategy, "blocked");

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn task_automation_settings_are_normalized_within_supported_range() {
        assert_eq!(normalize_task_automation_max_fix_rounds(Some(1)), 1);
        assert_eq!(normalize_task_automation_max_fix_rounds(Some(10)), 10);
        assert_eq!(normalize_task_automation_max_fix_rounds(Some(11)), 3);
        assert_eq!(
            normalize_task_automation_failure_strategy(Some("manual_control")),
            "manual_control"
        );
    }
}
