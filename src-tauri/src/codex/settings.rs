use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{insert_activity_log, normalize_optional_text, sqlite_pool};
use crate::codex::{new_node_command, new_npm_command};
use crate::db::models::{
    CodexSdkInstallResult, CodexSettings, CodexSettingsDocument, GitPreferences,
    RemoteCodexSettingsPayload, UpdateCodexSettings, UpdateGitPreferences,
};

const SETTINGS_FILE_NAME: &str = "codex-settings.json";
const SDK_RUNTIME_DIR_NAME: &str = "codex-sdk-runtime";
const SDK_BRIDGE_FILE_NAME: &str = "sdk-bridge.mjs";
const SDK_PACKAGE_NAME: &str = "@openai/codex-sdk";
const SDK_CLI_PACKAGE_NAME: &str = "@openai/codex";
pub const SDK_INSTALL_PACKAGE_SPECS: &[&str] =
    &["@openai/codex-sdk@latest", "@openai/codex@latest"];
const ONE_SHOT_PROVIDER_SDK: &str = "sdk";
const ONE_SHOT_PROVIDER_EXEC: &str = "exec";
const MINIMUM_NODE_MAJOR: u32 = 18;
const DEFAULT_ONE_SHOT_MODEL: &str = "gpt-5.4";
const DEFAULT_ONE_SHOT_REASONING_EFFORT: &str = "high";
const DEFAULT_TASK_AUTOMATION_MAX_FIX_ROUNDS: i32 = 3;
const DEFAULT_TASK_AUTOMATION_FAILURE_STRATEGY: &str = "blocked";
const DEFAULT_WORKTREE_LOCATION_MODE: &str = "repo_sibling_hidden";
const DEFAULT_AI_COMMIT_MESSAGE_LENGTH: &str = "title_with_body";
const DEFAULT_AI_COMMIT_MODEL_SOURCE: &str = "inherit_one_shot";
const SUPPORTED_MODELS: &[&str] = &[
    "gpt-5.4",
    "gpt-5.2-codex",
    "gpt-5.1-codex-max",
    "gpt-5.4-mini",
    "gpt-5.3-codex",
    "gpt-5.3-codex-spark",
    "gpt-5.2",
    "gpt-5.1-codex-mini",
];
const SUPPORTED_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];
const SUPPORTED_TASK_AUTOMATION_FAILURE_STRATEGIES: &[&str] = &["blocked", "manual_control"];
const SUPPORTED_WORKTREE_LOCATION_MODES: &[&str] =
    &["repo_sibling_hidden", "repo_child_hidden", "custom_root"];
const SUPPORTED_AI_COMMIT_MESSAGE_LENGTHS: &[&str] = &["title_only", "title_with_body"];
const SUPPORTED_AI_COMMIT_MODEL_SOURCES: &[&str] = &["inherit_one_shot", "custom"];

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
struct RawGitPreferences {
    #[serde(default)]
    default_task_use_worktree: Option<bool>,
    #[serde(default)]
    worktree_location_mode: Option<String>,
    #[serde(default)]
    worktree_custom_root: Option<String>,
    #[serde(default)]
    ai_commit_message_length: Option<String>,
    #[serde(default)]
    ai_commit_model_source: Option<String>,
    #[serde(default)]
    ai_commit_model: Option<String>,
    #[serde(default)]
    ai_commit_reasoning_effort: Option<String>,
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
    git_preferences: Option<RawGitPreferences>,
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

fn normalize_worktree_location_mode(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(value) if SUPPORTED_WORKTREE_LOCATION_MODES.contains(&value) => value.to_string(),
        _ => DEFAULT_WORKTREE_LOCATION_MODE.to_string(),
    }
}

fn normalize_ai_commit_message_length(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(value) if SUPPORTED_AI_COMMIT_MESSAGE_LENGTHS.contains(&value) => value.to_string(),
        _ => DEFAULT_AI_COMMIT_MESSAGE_LENGTH.to_string(),
    }
}

fn normalize_ai_commit_model_source(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(value) if SUPPORTED_AI_COMMIT_MODEL_SOURCES.contains(&value) => value.to_string(),
        _ => DEFAULT_AI_COMMIT_MODEL_SOURCE.to_string(),
    }
}

fn validate_supported_model(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if SUPPORTED_MODELS.contains(&trimmed) {
        Ok(trimmed.to_string())
    } else {
        Err(format!("不支持的模型：{}", value))
    }
}

fn validate_supported_reasoning_effort(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if SUPPORTED_REASONING_EFFORTS.contains(&trimmed) {
        Ok(trimmed.to_string())
    } else {
        Err(format!("不支持的推理强度：{}", value))
    }
}

fn local_worktree_custom_root_is_valid(value: &str) -> bool {
    Path::new(value).is_absolute()
}

fn remote_worktree_custom_root_is_valid(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed == "~" || trimmed.starts_with("~/") || Path::new(trimmed).is_absolute()
}

fn normalize_git_worktree_custom_root(value: Option<&str>, is_remote: bool) -> Option<String> {
    let normalized = normalize_optional_text(value)?;
    let is_valid = if is_remote {
        remote_worktree_custom_root_is_valid(&normalized)
    } else {
        local_worktree_custom_root_is_valid(&normalized)
    };
    is_valid.then_some(normalized)
}

fn default_git_preferences() -> GitPreferences {
    GitPreferences {
        default_task_use_worktree: false,
        worktree_location_mode: DEFAULT_WORKTREE_LOCATION_MODE.to_string(),
        worktree_custom_root: None,
        ai_commit_message_length: DEFAULT_AI_COMMIT_MESSAGE_LENGTH.to_string(),
        ai_commit_model_source: DEFAULT_AI_COMMIT_MODEL_SOURCE.to_string(),
        ai_commit_model: DEFAULT_ONE_SHOT_MODEL.to_string(),
        ai_commit_reasoning_effort: DEFAULT_ONE_SHOT_REASONING_EFFORT.to_string(),
    }
}

fn normalize_git_preferences(preferences: GitPreferences, is_remote: bool) -> GitPreferences {
    let default_preferences = default_git_preferences();
    let worktree_location_mode =
        normalize_worktree_location_mode(Some(&preferences.worktree_location_mode));
    let worktree_custom_root =
        normalize_git_worktree_custom_root(preferences.worktree_custom_root.as_deref(), is_remote);
    let worktree_location_mode =
        if worktree_location_mode == "custom_root" && worktree_custom_root.is_none() {
            default_preferences.worktree_location_mode
        } else {
            worktree_location_mode
        };

    GitPreferences {
        default_task_use_worktree: preferences.default_task_use_worktree,
        worktree_location_mode,
        worktree_custom_root,
        ai_commit_message_length: normalize_ai_commit_message_length(Some(
            &preferences.ai_commit_message_length,
        )),
        ai_commit_model_source: normalize_ai_commit_model_source(Some(
            &preferences.ai_commit_model_source,
        )),
        ai_commit_model: normalize_one_shot_model(Some(&preferences.ai_commit_model)),
        ai_commit_reasoning_effort: normalize_one_shot_reasoning_effort(Some(
            &preferences.ai_commit_reasoning_effort,
        )),
    }
}

fn normalize_raw_git_preferences(raw: RawGitPreferences, is_remote: bool) -> GitPreferences {
    let default_preferences = default_git_preferences();
    let worktree_location_mode =
        normalize_worktree_location_mode(raw.worktree_location_mode.as_deref());
    let worktree_custom_root =
        normalize_git_worktree_custom_root(raw.worktree_custom_root.as_deref(), is_remote);
    let worktree_location_mode =
        if worktree_location_mode == "custom_root" && worktree_custom_root.is_none() {
            default_preferences.worktree_location_mode
        } else {
            worktree_location_mode
        };

    GitPreferences {
        default_task_use_worktree: raw
            .default_task_use_worktree
            .unwrap_or(default_preferences.default_task_use_worktree),
        worktree_location_mode,
        worktree_custom_root,
        ai_commit_message_length: normalize_ai_commit_message_length(
            raw.ai_commit_message_length.as_deref(),
        ),
        ai_commit_model_source: normalize_ai_commit_model_source(
            raw.ai_commit_model_source.as_deref(),
        ),
        ai_commit_model: normalize_one_shot_model(raw.ai_commit_model.as_deref()),
        ai_commit_reasoning_effort: normalize_one_shot_reasoning_effort(
            raw.ai_commit_reasoning_effort.as_deref(),
        ),
    }
}

fn validate_git_preferences(preferences: &GitPreferences, is_remote: bool) -> Result<(), String> {
    if preferences.worktree_location_mode == "custom_root" {
        let root = preferences
            .worktree_custom_root
            .as_deref()
            .ok_or_else(|| "自定义 Worktree 根目录不能为空".to_string())?;
        let is_valid = if is_remote {
            remote_worktree_custom_root_is_valid(root)
        } else {
            local_worktree_custom_root_is_valid(root)
        };
        if !is_valid {
            return if is_remote {
                Err("SSH 配置下的自定义 Worktree 根目录必须是绝对路径或 ~/ 开头".to_string())
            } else {
                Err("本地配置下的自定义 Worktree 根目录必须是绝对路径".to_string())
            };
        }
    }

    if preferences.ai_commit_model_source == "custom" {
        validate_supported_model(&preferences.ai_commit_model)?;
        validate_supported_reasoning_effort(&preferences.ai_commit_reasoning_effort)?;
    }

    Ok(())
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

fn default_remote_sdk_install_dir(ssh_config_id: &str) -> String {
    format!("~/.codex-ai/{SDK_RUNTIME_DIR_NAME}/{ssh_config_id}")
}

fn default_codex_settings_with_install_dir(install_dir: String) -> CodexSettings {
    CodexSettings {
        task_sdk_enabled: false,
        one_shot_sdk_enabled: false,
        one_shot_model: DEFAULT_ONE_SHOT_MODEL.to_string(),
        one_shot_reasoning_effort: DEFAULT_ONE_SHOT_REASONING_EFFORT.to_string(),
        task_automation_default_enabled: false,
        task_automation_max_fix_rounds: DEFAULT_TASK_AUTOMATION_MAX_FIX_ROUNDS,
        task_automation_failure_strategy: DEFAULT_TASK_AUTOMATION_FAILURE_STRATEGY.to_string(),
        git_preferences: default_git_preferences(),
        node_path_override: None,
        sdk_install_dir: install_dir,
        one_shot_preferred_provider: ONE_SHOT_PROVIDER_SDK.to_string(),
    }
}

fn default_remote_codex_settings(ssh_config_id: &str) -> CodexSettings {
    default_codex_settings_with_install_dir(default_remote_sdk_install_dir(ssh_config_id))
}

fn remote_sdk_install_dir_should_be_repaired(
    sdk_install_dir: &str,
    local_default_install_dir: &str,
) -> bool {
    let normalized_value = sdk_install_dir.trim();
    let normalized_local = local_default_install_dir.trim();
    if normalized_value.is_empty() || normalized_local.is_empty() {
        return normalized_value.is_empty();
    }

    normalized_value == normalized_local
        || normalized_value.starts_with(&format!("{normalized_local}/"))
        || normalized_value.starts_with(&format!("{normalized_local}\\"))
}

fn normalize_remote_settings(
    raw: RawCodexSettings,
    ssh_config_id: &str,
    local_default_install_dir: &str,
) -> CodexSettings {
    let remote_default_install_dir = default_remote_sdk_install_dir(ssh_config_id);
    let mut settings = normalize_raw_settings_with_scope(raw, &remote_default_install_dir, true);
    if remote_sdk_install_dir_should_be_repaired(
        &settings.sdk_install_dir,
        local_default_install_dir,
    ) {
        settings.sdk_install_dir = remote_default_install_dir;
    }
    settings
}

fn normalize_remote_profile_settings(
    settings: CodexSettings,
    ssh_config_id: &str,
    local_default_install_dir: &str,
) -> CodexSettings {
    let remote_default_install_dir = default_remote_sdk_install_dir(ssh_config_id);
    let mut settings = normalize_settings_with_scope(settings, &remote_default_install_dir, true);
    if remote_sdk_install_dir_should_be_repaired(
        &settings.sdk_install_dir,
        local_default_install_dir,
    ) {
        settings.sdk_install_dir = remote_default_install_dir;
    }
    settings
}

fn default_codex_settings<R: Runtime>(app: &AppHandle<R>) -> Result<CodexSettings, String> {
    let default_install_dir = default_sdk_install_dir(app)?;
    Ok(default_codex_settings_with_install_dir(
        default_install_dir.to_string_lossy().to_string(),
    ))
}

fn normalize_settings_with_scope(
    settings: CodexSettings,
    default_install_dir: &str,
    is_remote: bool,
) -> CodexSettings {
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
        git_preferences: normalize_git_preferences(settings.git_preferences, is_remote),
        node_path_override: normalize_optional_text(settings.node_path_override.as_deref()),
        sdk_install_dir: normalize_optional_text(Some(&settings.sdk_install_dir))
            .unwrap_or_else(|| default_install_dir.to_string()),
        one_shot_preferred_provider: ONE_SHOT_PROVIDER_SDK.to_string(),
    }
}

fn normalize_settings(settings: CodexSettings, default_install_dir: &str) -> CodexSettings {
    normalize_settings_with_scope(settings, default_install_dir, false)
}

fn normalize_raw_settings_with_scope(
    raw: RawCodexSettings,
    default_install_dir: &str,
    is_remote: bool,
) -> CodexSettings {
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
        git_preferences: normalize_raw_git_preferences(
            raw.git_preferences.unwrap_or_default(),
            is_remote,
        ),
        node_path_override: normalize_optional_text(raw.node_path_override.as_deref()),
        sdk_install_dir: normalize_optional_text(raw.sdk_install_dir.as_deref())
            .unwrap_or_else(|| default_install_dir.to_string()),
        one_shot_preferred_provider: raw
            .one_shot_preferred_provider
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| ONE_SHOT_PROVIDER_SDK.to_string()),
    }
}

fn normalize_raw_settings(raw: RawCodexSettings, default_install_dir: &str) -> CodexSettings {
    normalize_raw_settings_with_scope(raw, default_install_dir, false)
}

fn normalize_settings_document<R: Runtime>(
    app: &AppHandle<R>,
    raw: RawCodexSettingsDocument,
) -> Result<CodexSettingsDocument, String> {
    let default_install_dir = default_sdk_install_dir(app)?.to_string_lossy().to_string();
    let local = normalize_raw_settings(raw.local.unwrap_or_default(), &default_install_dir);
    let mut remote_profiles = HashMap::new();
    for (ssh_config_id, profile) in raw.remote_profiles {
        remote_profiles.insert(
            ssh_config_id.clone(),
            normalize_remote_settings(profile, &ssh_config_id, &default_install_dir),
        );
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
    let default_install_dir = default_sdk_install_dir(app)?.to_string_lossy().to_string();
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

    let default_install_dir = default_sdk_install_dir(app)?.to_string_lossy().to_string();
    let mut remote_profiles = HashMap::new();
    for (ssh_config_id, settings) in &document.remote_profiles {
        remote_profiles.insert(
            ssh_config_id.clone(),
            normalize_remote_profile_settings(
                settings.clone(),
                ssh_config_id,
                &default_install_dir,
            ),
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

    Ok(default_remote_codex_settings(ssh_config_id))
}

pub fn save_codex_settings<R: Runtime>(
    app: &AppHandle<R>,
    settings: &CodexSettings,
) -> Result<(), String> {
    let mut document = load_codex_settings_document(app)?;
    document.local = settings.clone();
    save_codex_settings_document(app, &document)
}

fn merge_git_preferences(
    current: &mut GitPreferences,
    updates: UpdateGitPreferences,
    is_remote: bool,
) -> Result<(), String> {
    if let Some(default_task_use_worktree) = updates.default_task_use_worktree {
        current.default_task_use_worktree = default_task_use_worktree;
    }
    if let Some(worktree_location_mode) = updates.worktree_location_mode {
        current.worktree_location_mode =
            normalize_worktree_location_mode(Some(&worktree_location_mode));
    }
    if let Some(worktree_custom_root) = updates.worktree_custom_root {
        current.worktree_custom_root = normalize_optional_text(worktree_custom_root.as_deref());
    }
    if let Some(ai_commit_message_length) = updates.ai_commit_message_length {
        current.ai_commit_message_length =
            normalize_ai_commit_message_length(Some(&ai_commit_message_length));
    }
    if let Some(ai_commit_model_source) = updates.ai_commit_model_source {
        current.ai_commit_model_source =
            normalize_ai_commit_model_source(Some(&ai_commit_model_source));
    }
    if let Some(ai_commit_model) = updates.ai_commit_model {
        current.ai_commit_model = validate_supported_model(&ai_commit_model)?;
    }
    if let Some(ai_commit_reasoning_effort) = updates.ai_commit_reasoning_effort {
        current.ai_commit_reasoning_effort =
            validate_supported_reasoning_effort(&ai_commit_reasoning_effort)?;
    }
    validate_git_preferences(current, is_remote)
}

fn git_preferences_changed(previous: &GitPreferences, next: &GitPreferences) -> bool {
    previous.default_task_use_worktree != next.default_task_use_worktree
        || previous.worktree_location_mode != next.worktree_location_mode
        || previous.worktree_custom_root != next.worktree_custom_root
        || previous.ai_commit_message_length != next.ai_commit_message_length
        || previous.ai_commit_model_source != next.ai_commit_model_source
        || previous.ai_commit_model != next.ai_commit_model
        || previous.ai_commit_reasoning_effort != next.ai_commit_reasoning_effort
}

fn format_worktree_location_mode_label(value: &str) -> &str {
    match value {
        "repo_child_hidden" => "仓库 .git 目录",
        "custom_root" => "自定义根目录",
        _ => "仓库同级隐藏目录",
    }
}

fn format_ai_commit_message_length_label(value: &str) -> &str {
    match value {
        "title_only" => "仅标题",
        _ => "标题+详情",
    }
}

fn format_ai_commit_model_source_label(value: &str) -> &str {
    match value {
        "custom" => "单独指定",
        _ => "跟随一次性 AI",
    }
}

fn format_git_preferences_activity_details(
    profile_label: &str,
    preferences: &GitPreferences,
) -> String {
    let custom_root = preferences
        .worktree_custom_root
        .as_deref()
        .unwrap_or("未设置");
    let model_details = if preferences.ai_commit_model_source == "custom" {
        format!(
            "{} / 推理 {}",
            preferences.ai_commit_model, preferences.ai_commit_reasoning_effort
        )
    } else {
        "跟随一次性 AI".to_string()
    };

    format!(
        "{}：新建任务默认 Worktree {}；目录规则 {}；自定义根目录 {}；提交信息默认 {}；Git AI {}",
        profile_label,
        if preferences.default_task_use_worktree {
            "开启"
        } else {
            "关闭"
        },
        format_worktree_location_mode_label(&preferences.worktree_location_mode),
        custom_root,
        format_ai_commit_message_length_label(&preferences.ai_commit_message_length),
        if preferences.ai_commit_model_source == "custom" {
            format!(
                "{}（{}）",
                format_ai_commit_model_source_label(&preferences.ai_commit_model_source),
                model_details
            )
        } else {
            format_ai_commit_model_source_label(&preferences.ai_commit_model_source).to_string()
        }
    )
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

    if let Some(git_preferences) = updates.git_preferences {
        merge_git_preferences(&mut settings.git_preferences, git_preferences, false)?;
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
    if let Some(git_preferences) = updates.git_preferences {
        merge_git_preferences(&mut settings.git_preferences, git_preferences, true)?;
    }
    if let Some(node_path_override) = updates.node_path_override {
        settings.node_path_override = normalize_optional_text(node_path_override.as_deref());
    }
    if let Some(sdk_install_dir) = updates.sdk_install_dir {
        settings.sdk_install_dir = normalize_optional_text(sdk_install_dir.as_deref())
            .unwrap_or_else(|| default_remote_sdk_install_dir(ssh_config_id));
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

pub(crate) fn ensure_supported_node_version(version: &str) -> Result<(), String> {
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
        .args(SDK_INSTALL_PACKAGE_SPECS)
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

    if git_preferences_changed(&previous.git_preferences, &next.git_preferences) {
        if let Ok(pool) = sqlite_pool(&app).await {
            let _ = insert_activity_log(
                &pool,
                "git_preferences_updated",
                &format_git_preferences_activity_details("本地 Git 偏好", &next.git_preferences),
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
    let previous = load_remote_codex_settings(&app, &payload.ssh_config_id)?;
    let next = merge_remote_codex_settings(&app, &payload.ssh_config_id, payload.updates)?;

    if git_preferences_changed(&previous.git_preferences, &next.git_preferences) {
        if let Ok(pool) = sqlite_pool(&app).await {
            let _ = insert_activity_log(
                &pool,
                "git_preferences_updated",
                &format_git_preferences_activity_details(
                    &format!("SSH Git 偏好（{}）", payload.ssh_config_id),
                    &next.git_preferences,
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
pub async fn install_codex_sdk<R: Runtime>(
    app: AppHandle<R>,
) -> Result<CodexSdkInstallResult, String> {
    install_codex_sdk_runtime(&app).await
}

#[cfg(test)]
mod tests {
    use super::{
        default_git_preferences, default_remote_codex_settings, determine_effective_provider,
        merge_git_preferences, normalize_raw_settings, normalize_remote_profile_settings,
        normalize_remote_settings, normalize_task_automation_failure_strategy,
        normalize_task_automation_max_fix_rounds, parse_node_major_version,
        read_sdk_version_from_dir, sdk_platform_package_for_target, RawCodexSettings,
        RawGitPreferences, SDK_INSTALL_PACKAGE_SPECS,
    };
    use crate::db::models::{CodexSettings, GitPreferences, UpdateGitPreferences};
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
    fn sdk_reinstall_targets_latest_packages() {
        assert_eq!(
            SDK_INSTALL_PACKAGE_SPECS,
            &["@openai/codex-sdk@latest", "@openai/codex@latest"]
        );
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
            base.to_string_lossy().as_ref(),
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
            base.to_string_lossy().as_ref(),
        );

        assert_eq!(normalized.one_shot_model, "gpt-5.4");
        assert_eq!(normalized.one_shot_reasoning_effort, "high");

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn supported_new_one_shot_models_are_preserved() {
        let base = create_temp_dir();
        let normalized = normalize_raw_settings(
            RawCodexSettings {
                one_shot_model: Some("gpt-5.3-codex-spark".to_string()),
                ..RawCodexSettings::default()
            },
            base.to_string_lossy().as_ref(),
        );

        assert_eq!(normalized.one_shot_model, "gpt-5.3-codex-spark");

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
            base.to_string_lossy().as_ref(),
        );

        assert!(normalized.task_automation_default_enabled);
        assert_eq!(normalized.task_automation_max_fix_rounds, 3);
        assert_eq!(normalized.task_automation_failure_strategy, "blocked");

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn missing_git_preferences_fall_back_to_defaults() {
        let base = create_temp_dir();
        let normalized =
            normalize_raw_settings(RawCodexSettings::default(), base.to_string_lossy().as_ref());

        assert!(!normalized.git_preferences.default_task_use_worktree);
        assert_eq!(
            normalized.git_preferences.worktree_location_mode,
            "repo_sibling_hidden"
        );
        assert_eq!(
            normalized.git_preferences.ai_commit_message_length,
            "title_with_body"
        );
        assert_eq!(
            normalized.git_preferences.ai_commit_model_source,
            "inherit_one_shot"
        );

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn invalid_custom_root_falls_back_to_default_worktree_mode() {
        let base = create_temp_dir();
        let normalized = normalize_raw_settings(
            RawCodexSettings {
                git_preferences: Some(RawGitPreferences {
                    worktree_location_mode: Some("custom_root".to_string()),
                    worktree_custom_root: Some("relative/path".to_string()),
                    ..RawGitPreferences::default()
                }),
                ..RawCodexSettings::default()
            },
            base.to_string_lossy().as_ref(),
        );

        assert_eq!(
            normalized.git_preferences.worktree_location_mode,
            "repo_sibling_hidden"
        );
        assert_eq!(normalized.git_preferences.worktree_custom_root, None);

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn remote_settings_preserve_home_custom_root() {
        let local_default =
            "/Users/wenyuan/Library/Application Support/com.wenyuan.codex-ai/codex-sdk-runtime";
        let normalized = normalize_remote_settings(
            RawCodexSettings {
                git_preferences: Some(RawGitPreferences {
                    worktree_location_mode: Some("custom_root".to_string()),
                    worktree_custom_root: Some("~/codex-worktrees".to_string()),
                    ..RawGitPreferences::default()
                }),
                ..RawCodexSettings::default()
            },
            "ssh-1",
            local_default,
        );

        assert_eq!(
            normalized.git_preferences.worktree_location_mode,
            "custom_root"
        );
        assert_eq!(
            normalized.git_preferences.worktree_custom_root.as_deref(),
            Some("~/codex-worktrees")
        );
    }

    #[test]
    fn remote_profile_settings_preserve_home_custom_root() {
        let local_default =
            "/Users/wenyuan/Library/Application Support/com.wenyuan.codex-ai/codex-sdk-runtime";
        let normalized = normalize_remote_profile_settings(
            CodexSettings {
                git_preferences: GitPreferences {
                    worktree_location_mode: "custom_root".to_string(),
                    worktree_custom_root: Some("~/codex-worktrees".to_string()),
                    ..default_git_preferences()
                },
                ..default_remote_codex_settings("ssh-1")
            },
            "ssh-1",
            local_default,
        );

        assert_eq!(
            normalized.git_preferences.worktree_location_mode,
            "custom_root"
        );
        assert_eq!(
            normalized.git_preferences.worktree_custom_root.as_deref(),
            Some("~/codex-worktrees")
        );
    }

    #[test]
    fn merge_git_preferences_rejects_invalid_local_custom_root() {
        let mut current = default_git_preferences();
        let error = merge_git_preferences(
            &mut current,
            UpdateGitPreferences {
                worktree_location_mode: Some("custom_root".to_string()),
                worktree_custom_root: Some(Some("relative/path".to_string())),
                default_task_use_worktree: None,
                ai_commit_message_length: None,
                ai_commit_model_source: None,
                ai_commit_model: None,
                ai_commit_reasoning_effort: None,
            },
            false,
        )
        .expect_err("invalid local custom root should fail");

        assert!(error.contains("绝对路径"));
    }

    #[test]
    fn merge_git_preferences_accepts_remote_home_path() {
        let mut current = default_git_preferences();
        merge_git_preferences(
            &mut current,
            UpdateGitPreferences {
                worktree_location_mode: Some("custom_root".to_string()),
                worktree_custom_root: Some(Some("~/worktrees".to_string())),
                default_task_use_worktree: Some(true),
                ai_commit_message_length: Some("title_only".to_string()),
                ai_commit_model_source: Some("custom".to_string()),
                ai_commit_model: Some("gpt-5.4-mini".to_string()),
                ai_commit_reasoning_effort: Some("medium".to_string()),
            },
            true,
        )
        .expect("remote custom root should be valid");

        assert!(current.default_task_use_worktree);
        assert_eq!(current.worktree_location_mode, "custom_root");
        assert_eq!(current.worktree_custom_root.as_deref(), Some("~/worktrees"));
        assert_eq!(current.ai_commit_message_length, "title_only");
        assert_eq!(current.ai_commit_model_source, "custom");
        assert_eq!(current.ai_commit_model, "gpt-5.4-mini");
        assert_eq!(current.ai_commit_reasoning_effort, "medium");
    }

    #[test]
    fn merge_git_preferences_rejects_unsupported_custom_model() {
        let mut current = default_git_preferences();
        let error = merge_git_preferences(
            &mut current,
            UpdateGitPreferences {
                worktree_location_mode: None,
                worktree_custom_root: None,
                default_task_use_worktree: None,
                ai_commit_message_length: None,
                ai_commit_model_source: Some("custom".to_string()),
                ai_commit_model: Some("gpt-unknown".to_string()),
                ai_commit_reasoning_effort: Some("medium".to_string()),
            },
            false,
        )
        .expect_err("unsupported custom model should fail");

        assert!(error.contains("不支持的模型"));
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

    #[test]
    fn default_remote_codex_settings_use_remote_home_directory() {
        let settings = default_remote_codex_settings("ssh-1");
        assert_eq!(
            settings.sdk_install_dir,
            "~/.codex-ai/codex-sdk-runtime/ssh-1"
        );
    }

    #[test]
    fn remote_settings_repair_legacy_local_sdk_directory() {
        let local_default =
            "/Users/wenyuan/Library/Application Support/com.wenyuan.codex-ai/codex-sdk-runtime";
        let normalized = normalize_remote_settings(
            RawCodexSettings {
                sdk_install_dir: Some(local_default.to_string()),
                ..RawCodexSettings::default()
            },
            "ssh-1",
            local_default,
        );

        assert_eq!(
            normalized.sdk_install_dir,
            "~/.codex-ai/codex-sdk-runtime/ssh-1"
        );
    }
}
