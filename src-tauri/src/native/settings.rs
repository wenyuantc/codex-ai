use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{insert_activity_log, sqlite_pool};
use crate::db::models::{NativeSettings, UpdateNativeSettings};

const SETTINGS_FILE_NAME: &str = "native-settings.json";
pub const DEFAULT_NATIVE_MAX_TURNS: i32 = 40;
const MAX_NATIVE_MAX_TURNS: i32 = 500;
pub const DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS: i32 = 1;
const MAX_NATIVE_MAX_CONCURRENT_SUBAGENTS: i32 = 16;
/// Keep normal coding turns well below the provider's advertised context
/// window. The runner can still compact and continue when this threshold is
/// reached.
pub const DEFAULT_NATIVE_CONTEXT_WINDOW_TOKENS: i32 = 128_000;
const MIN_NATIVE_CONTEXT_WINDOW_TOKENS: i32 = 8_000;
const MAX_NATIVE_CONTEXT_WINDOW_TOKENS: i32 = 1_000_000;
/// A rollout budget is shared by the parent and all child agents. Zero keeps
/// the legacy unlimited behavior for users who explicitly opt out.
pub const DEFAULT_NATIVE_ROLLOUT_TOKEN_BUDGET: i64 = 10_000_000;
const MAX_NATIVE_ROLLOUT_TOKEN_BUDGET: i64 = 100_000_000;
pub const DEFAULT_NATIVE_MAX_TOOL_OUTPUT_TOKENS: i32 = 4_096;
const MIN_NATIVE_MAX_TOOL_OUTPUT_TOKENS: i32 = 256;
const MAX_NATIVE_MAX_TOOL_OUTPUT_TOKENS: i32 = 65_536;
pub const DEFAULT_NATIVE_PERMISSION_TIMEOUT_SECS: i32 = 300;
const MAX_NATIVE_PERMISSION_TIMEOUT_SECS: i32 = 86_400;
pub const SUBAGENT_POLICY_CONSERVATIVE: &str = "conservative";
pub const SUBAGENT_POLICY_BALANCED: &str = "balanced";
pub const SUBAGENT_POLICY_AGGRESSIVE: &str = "aggressive";
pub const DEFAULT_NATIVE_SUBAGENT_POLICY: &str = SUBAGENT_POLICY_CONSERVATIVE;

#[derive(Debug, Default, Deserialize, Serialize)]
struct RawNativeSettings {
    #[serde(default)]
    max_turns: Option<i32>,
    #[serde(default)]
    confirm_high_risk: Option<bool>,
    #[serde(default)]
    max_concurrent_subagents: Option<i32>,
    #[serde(default)]
    subagent_policy: Option<String>,
    #[serde(default)]
    context_window_tokens: Option<i32>,
    #[serde(default)]
    rollout_token_budget: Option<i64>,
    #[serde(default)]
    max_tool_output_tokens: Option<i32>,
    #[serde(default)]
    permission_timeout_secs: Option<i32>,
}

pub fn normalize_native_max_turns(value: Option<i32>) -> i32 {
    match value {
        Some(value) if (0..=MAX_NATIVE_MAX_TURNS).contains(&value) => value,
        _ => DEFAULT_NATIVE_MAX_TURNS,
    }
}

pub fn normalize_native_max_concurrent_subagents(value: Option<i32>) -> i32 {
    match value {
        Some(value) if (1..=MAX_NATIVE_MAX_CONCURRENT_SUBAGENTS).contains(&value) => value,
        _ => DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS,
    }
}

pub fn normalize_native_context_window_tokens(value: Option<i32>) -> i32 {
    match value {
        Some(value)
            if (MIN_NATIVE_CONTEXT_WINDOW_TOKENS..=MAX_NATIVE_CONTEXT_WINDOW_TOKENS)
                .contains(&value) =>
        {
            value
        }
        _ => DEFAULT_NATIVE_CONTEXT_WINDOW_TOKENS,
    }
}

pub fn normalize_native_rollout_token_budget(value: Option<i64>) -> i64 {
    match value {
        Some(value) if (0..=MAX_NATIVE_ROLLOUT_TOKEN_BUDGET).contains(&value) => value,
        _ => DEFAULT_NATIVE_ROLLOUT_TOKEN_BUDGET,
    }
}

pub fn normalize_native_permission_timeout_secs(value: Option<i32>) -> i32 {
    match value {
        Some(value) if (0..=MAX_NATIVE_PERMISSION_TIMEOUT_SECS).contains(&value) => value,
        _ => DEFAULT_NATIVE_PERMISSION_TIMEOUT_SECS,
    }
}

pub fn normalize_native_max_tool_output_tokens(value: Option<i32>) -> i32 {
    match value {
        Some(value)
            if (MIN_NATIVE_MAX_TOOL_OUTPUT_TOKENS..=MAX_NATIVE_MAX_TOOL_OUTPUT_TOKENS)
                .contains(&value) =>
        {
            value
        }
        _ => DEFAULT_NATIVE_MAX_TOOL_OUTPUT_TOKENS,
    }
}

pub fn normalize_subagent_policy(value: Option<&str>) -> String {
    match value.map(str::trim).unwrap_or("") {
        SUBAGENT_POLICY_CONSERVATIVE => SUBAGENT_POLICY_CONSERVATIVE.to_string(),
        SUBAGENT_POLICY_AGGRESSIVE => SUBAGENT_POLICY_AGGRESSIVE.to_string(),
        SUBAGENT_POLICY_BALANCED => SUBAGENT_POLICY_BALANCED.to_string(),
        _ => DEFAULT_NATIVE_SUBAGENT_POLICY.to_string(),
    }
}

pub fn subagent_policy_label_zh(policy: &str) -> &'static str {
    match policy {
        SUBAGENT_POLICY_CONSERVATIVE => "保守",
        SUBAGENT_POLICY_AGGRESSIVE => "积极",
        _ => "均衡",
    }
}

fn app_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法读取应用配置目录: {error}"))
}

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join(SETTINGS_FILE_NAME))
}

fn default_settings() -> NativeSettings {
    NativeSettings {
        max_turns: DEFAULT_NATIVE_MAX_TURNS,
        confirm_high_risk: true,
        max_concurrent_subagents: DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS,
        subagent_policy: DEFAULT_NATIVE_SUBAGENT_POLICY.to_string(),
        context_window_tokens: DEFAULT_NATIVE_CONTEXT_WINDOW_TOKENS,
        rollout_token_budget: DEFAULT_NATIVE_ROLLOUT_TOKEN_BUDGET,
        max_tool_output_tokens: DEFAULT_NATIVE_MAX_TOOL_OUTPUT_TOKENS,
        permission_timeout_secs: DEFAULT_NATIVE_PERMISSION_TIMEOUT_SECS,
    }
}

fn normalize_settings(raw: RawNativeSettings) -> NativeSettings {
    NativeSettings {
        max_turns: normalize_native_max_turns(raw.max_turns),
        confirm_high_risk: raw.confirm_high_risk.unwrap_or(true),
        max_concurrent_subagents: normalize_native_max_concurrent_subagents(
            raw.max_concurrent_subagents,
        ),
        subagent_policy: normalize_subagent_policy(raw.subagent_policy.as_deref()),
        context_window_tokens: normalize_native_context_window_tokens(raw.context_window_tokens),
        rollout_token_budget: normalize_native_rollout_token_budget(raw.rollout_token_budget),
        max_tool_output_tokens: normalize_native_max_tool_output_tokens(raw.max_tool_output_tokens),
        permission_timeout_secs: normalize_native_permission_timeout_secs(
            raw.permission_timeout_secs,
        ),
    }
}

pub fn load_native_settings<R: Runtime>(app: &AppHandle<R>) -> Result<NativeSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(default_settings());
    }
    let text =
        fs::read_to_string(&path).map_err(|error| format!("读取内置 Agent 设置失败: {error}"))?;
    let raw = serde_json::from_str::<RawNativeSettings>(&text).unwrap_or_default();
    Ok(normalize_settings(raw))
}

pub fn effective_max_turns<R: Runtime>(app: &AppHandle<R>) -> u32 {
    load_native_settings(app)
        .map(|settings| settings.max_turns.max(0) as u32)
        .unwrap_or(DEFAULT_NATIVE_MAX_TURNS as u32)
}

fn save_native_settings<R: Runtime>(
    app: &AppHandle<R>,
    settings: &NativeSettings,
) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建内置 Agent 设置目录失败: {error}"))?;
    }
    let raw = RawNativeSettings {
        max_turns: Some(normalize_native_max_turns(Some(settings.max_turns))),
        confirm_high_risk: Some(settings.confirm_high_risk),
        max_concurrent_subagents: Some(normalize_native_max_concurrent_subagents(Some(
            settings.max_concurrent_subagents,
        ))),
        subagent_policy: Some(normalize_subagent_policy(Some(
            settings.subagent_policy.as_str(),
        ))),
        context_window_tokens: Some(normalize_native_context_window_tokens(Some(
            settings.context_window_tokens,
        ))),
        rollout_token_budget: Some(normalize_native_rollout_token_budget(Some(
            settings.rollout_token_budget,
        ))),
        max_tool_output_tokens: Some(normalize_native_max_tool_output_tokens(Some(
            settings.max_tool_output_tokens,
        ))),
        permission_timeout_secs: Some(normalize_native_permission_timeout_secs(Some(
            settings.permission_timeout_secs,
        ))),
    };
    let json = serde_json::to_string_pretty(&raw)
        .map_err(|error| format!("序列化内置 Agent 设置失败: {error}"))?;
    fs::write(&path, json).map_err(|error| format!("写入内置 Agent 设置失败: {error}"))
}

fn max_turns_activity_details(max_turns: i32) -> String {
    if max_turns == 0 {
        "内置 Agent 最大工具轮次：不限制".to_string()
    } else {
        format!("内置 Agent 最大工具轮次：{max_turns}")
    }
}

async fn merge_native_settings<R: Runtime>(
    app: &AppHandle<R>,
    updates: UpdateNativeSettings,
) -> Result<NativeSettings, String> {
    let previous = load_native_settings(app)?;
    let mut next = previous.clone();
    if let Some(max_turns) = updates.max_turns {
        next.max_turns = normalize_native_max_turns(Some(max_turns));
    }
    if let Some(confirm_high_risk) = updates.confirm_high_risk {
        next.confirm_high_risk = confirm_high_risk;
    }
    if let Some(max_concurrent_subagents) = updates.max_concurrent_subagents {
        next.max_concurrent_subagents =
            normalize_native_max_concurrent_subagents(Some(max_concurrent_subagents));
    }
    if let Some(subagent_policy) = updates.subagent_policy {
        next.subagent_policy = normalize_subagent_policy(Some(subagent_policy.as_str()));
    }
    if let Some(context_window_tokens) = updates.context_window_tokens {
        next.context_window_tokens =
            normalize_native_context_window_tokens(Some(context_window_tokens));
    }
    if let Some(rollout_token_budget) = updates.rollout_token_budget {
        next.rollout_token_budget =
            normalize_native_rollout_token_budget(Some(rollout_token_budget));
    }
    if let Some(max_tool_output_tokens) = updates.max_tool_output_tokens {
        next.max_tool_output_tokens =
            normalize_native_max_tool_output_tokens(Some(max_tool_output_tokens));
    }
    if let Some(permission_timeout_secs) = updates.permission_timeout_secs {
        next.permission_timeout_secs =
            normalize_native_permission_timeout_secs(Some(permission_timeout_secs));
    }
    save_native_settings(app, &next)?;
    if previous.max_turns != next.max_turns
        || previous.confirm_high_risk != next.confirm_high_risk
        || previous.max_concurrent_subagents != next.max_concurrent_subagents
        || previous.subagent_policy != next.subagent_policy
        || previous.context_window_tokens != next.context_window_tokens
        || previous.rollout_token_budget != next.rollout_token_budget
        || previous.max_tool_output_tokens != next.max_tool_output_tokens
        || previous.permission_timeout_secs != next.permission_timeout_secs
    {
        if let Ok(pool) = sqlite_pool(app).await {
            let _ = insert_activity_log(
                &pool,
                "native_settings_updated",
                &native_settings_activity_details(&next),
                None,
                None,
                None,
            )
            .await;
        }
    }
    Ok(next)
}

fn native_settings_activity_details(settings: &NativeSettings) -> String {
    format!(
        "{}；高风险确认：{}；确认超时：{}；同轮子 Agent 上限：{}；子 Agent 策略：{}；上下文窗口：{} token；会话预算：{}；单条工具结果：{} token",
        max_turns_activity_details(settings.max_turns),
        if settings.confirm_high_risk {
            "开启"
        } else {
            "关闭"
        },
        if settings.permission_timeout_secs == 0 {
            "不超时".to_string()
        } else {
            format!("{} 秒", settings.permission_timeout_secs)
        },
        settings.max_concurrent_subagents,
        subagent_policy_label_zh(&settings.subagent_policy),
        settings.context_window_tokens,
        if settings.rollout_token_budget == 0 {
            "不限制".to_string()
        } else {
            format!("{} token", settings.rollout_token_budget)
        },
        settings.max_tool_output_tokens,
    )
}

pub fn effective_subagent_policy<R: Runtime>(app: &AppHandle<R>) -> String {
    load_native_settings(app)
        .map(|settings| normalize_subagent_policy(Some(settings.subagent_policy.as_str())))
        .unwrap_or_else(|_| DEFAULT_NATIVE_SUBAGENT_POLICY.to_string())
}

pub fn effective_max_concurrent_subagents<R: Runtime>(app: &AppHandle<R>) -> u32 {
    load_native_settings(app)
        .map(|settings| settings.max_concurrent_subagents.max(1) as u32)
        .unwrap_or(DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS as u32)
}

pub fn effective_context_window_tokens<R: Runtime>(app: &AppHandle<R>) -> u32 {
    load_native_settings(app)
        .map(|settings| {
            settings
                .context_window_tokens
                .max(MIN_NATIVE_CONTEXT_WINDOW_TOKENS) as u32
        })
        .unwrap_or(DEFAULT_NATIVE_CONTEXT_WINDOW_TOKENS as u32)
}

pub fn effective_rollout_token_budget<R: Runtime>(app: &AppHandle<R>) -> u64 {
    load_native_settings(app)
        .map(|settings| settings.rollout_token_budget.max(0) as u64)
        .unwrap_or(DEFAULT_NATIVE_ROLLOUT_TOKEN_BUDGET as u64)
}

pub fn effective_max_tool_output_tokens<R: Runtime>(app: &AppHandle<R>) -> u32 {
    load_native_settings(app)
        .map(|settings| {
            settings
                .max_tool_output_tokens
                .max(MIN_NATIVE_MAX_TOOL_OUTPUT_TOKENS) as u32
        })
        .unwrap_or(DEFAULT_NATIVE_MAX_TOOL_OUTPUT_TOKENS as u32)
}

pub fn effective_permission_timeout_secs<R: Runtime>(app: &AppHandle<R>) -> u64 {
    load_native_settings(app)
        .map(|settings| settings.permission_timeout_secs.max(0) as u64)
        .unwrap_or(DEFAULT_NATIVE_PERMISSION_TIMEOUT_SECS as u64)
}

pub fn confirm_high_risk_enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    load_native_settings(app)
        .map(|settings| settings.confirm_high_risk)
        .unwrap_or(true)
}

#[tauri::command]
pub async fn get_native_settings<R: Runtime>(app: AppHandle<R>) -> Result<NativeSettings, String> {
    load_native_settings(&app)
}

#[tauri::command]
pub async fn update_native_settings<R: Runtime>(
    app: AppHandle<R>,
    updates: UpdateNativeSettings,
) -> Result<NativeSettings, String> {
    merge_native_settings(&app, updates).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_zero_as_unlimited() {
        assert_eq!(normalize_native_max_turns(Some(0)), 0);
    }

    #[test]
    fn normalize_keeps_default_range() {
        assert_eq!(normalize_native_max_turns(Some(40)), 40);
        assert_eq!(normalize_native_max_turns(Some(500)), 500);
    }

    #[test]
    fn missing_confirm_flag_defaults_on() {
        let settings = normalize_settings(RawNativeSettings {
            max_turns: Some(40),
            confirm_high_risk: None,
            max_concurrent_subagents: None,
            subagent_policy: None,
            context_window_tokens: None,
            rollout_token_budget: None,
            max_tool_output_tokens: None,
            permission_timeout_secs: None,
        });
        assert!(settings.confirm_high_risk);
        assert_eq!(
            settings.max_concurrent_subagents,
            DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS
        );
        assert_eq!(settings.subagent_policy, DEFAULT_NATIVE_SUBAGENT_POLICY);
        assert_eq!(
            settings.context_window_tokens,
            DEFAULT_NATIVE_CONTEXT_WINDOW_TOKENS
        );
        assert_eq!(
            settings.rollout_token_budget,
            DEFAULT_NATIVE_ROLLOUT_TOKEN_BUDGET
        );
        assert_eq!(
            settings.max_tool_output_tokens,
            DEFAULT_NATIVE_MAX_TOOL_OUTPUT_TOKENS
        );
        assert_eq!(
            settings.permission_timeout_secs,
            DEFAULT_NATIVE_PERMISSION_TIMEOUT_SECS
        );
    }

    #[test]
    fn normalize_subagent_policy_values() {
        assert_eq!(
            normalize_subagent_policy(None),
            DEFAULT_NATIVE_SUBAGENT_POLICY
        );
        assert_eq!(
            normalize_subagent_policy(Some("balanced")),
            SUBAGENT_POLICY_BALANCED
        );
        assert_eq!(
            normalize_subagent_policy(Some("aggressive")),
            SUBAGENT_POLICY_AGGRESSIVE
        );
        assert_eq!(
            normalize_subagent_policy(Some("conservative")),
            SUBAGENT_POLICY_CONSERVATIVE
        );
        assert_eq!(
            normalize_subagent_policy(Some("yolo")),
            DEFAULT_NATIVE_SUBAGENT_POLICY
        );
    }

    #[test]
    fn normalize_concurrent_subagents_range() {
        assert_eq!(
            normalize_native_max_concurrent_subagents(None),
            DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS
        );
        assert_eq!(normalize_native_max_concurrent_subagents(Some(3)), 3);
        assert_eq!(normalize_native_max_concurrent_subagents(Some(1)), 1);
        assert_eq!(normalize_native_max_concurrent_subagents(Some(16)), 16);
        assert_eq!(
            normalize_native_max_concurrent_subagents(None),
            DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS
        );
        assert_eq!(
            normalize_native_max_concurrent_subagents(Some(0)),
            DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS
        );
        assert_eq!(
            normalize_native_max_concurrent_subagents(Some(17)),
            DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS
        );
    }

    #[test]
    fn normalize_falls_back_outside_range() {
        assert_eq!(normalize_native_max_turns(None), DEFAULT_NATIVE_MAX_TURNS);
        assert_eq!(
            normalize_native_max_turns(Some(-1)),
            DEFAULT_NATIVE_MAX_TURNS
        );
        assert_eq!(
            normalize_native_max_turns(Some(501)),
            DEFAULT_NATIVE_MAX_TURNS
        );
    }

    #[test]
    fn normalize_context_window_and_tool_limits() {
        assert_eq!(
            normalize_native_context_window_tokens(Some(DEFAULT_NATIVE_CONTEXT_WINDOW_TOKENS)),
            DEFAULT_NATIVE_CONTEXT_WINDOW_TOKENS
        );
        assert_eq!(
            normalize_native_context_window_tokens(Some(MIN_NATIVE_CONTEXT_WINDOW_TOKENS)),
            MIN_NATIVE_CONTEXT_WINDOW_TOKENS
        );
        assert_eq!(
            normalize_native_context_window_tokens(Some(MIN_NATIVE_CONTEXT_WINDOW_TOKENS - 1)),
            DEFAULT_NATIVE_CONTEXT_WINDOW_TOKENS
        );
        assert_eq!(
            normalize_native_context_window_tokens(Some(MAX_NATIVE_CONTEXT_WINDOW_TOKENS + 1)),
            DEFAULT_NATIVE_CONTEXT_WINDOW_TOKENS
        );

        assert_eq!(normalize_native_rollout_token_budget(Some(0)), 0);
        assert_eq!(
            normalize_native_rollout_token_budget(Some(DEFAULT_NATIVE_ROLLOUT_TOKEN_BUDGET)),
            DEFAULT_NATIVE_ROLLOUT_TOKEN_BUDGET
        );
        assert_eq!(
            normalize_native_rollout_token_budget(Some(-1)),
            DEFAULT_NATIVE_ROLLOUT_TOKEN_BUDGET
        );
        assert_eq!(
            normalize_native_rollout_token_budget(Some(MAX_NATIVE_ROLLOUT_TOKEN_BUDGET + 1)),
            DEFAULT_NATIVE_ROLLOUT_TOKEN_BUDGET
        );

        assert_eq!(
            normalize_native_max_tool_output_tokens(Some(DEFAULT_NATIVE_MAX_TOOL_OUTPUT_TOKENS)),
            DEFAULT_NATIVE_MAX_TOOL_OUTPUT_TOKENS
        );
        assert_eq!(
            normalize_native_max_tool_output_tokens(Some(MIN_NATIVE_MAX_TOOL_OUTPUT_TOKENS - 1)),
            DEFAULT_NATIVE_MAX_TOOL_OUTPUT_TOKENS
        );
        assert_eq!(
            normalize_native_max_tool_output_tokens(Some(MAX_NATIVE_MAX_TOOL_OUTPUT_TOKENS + 1)),
            DEFAULT_NATIVE_MAX_TOOL_OUTPUT_TOKENS
        );

        assert_eq!(normalize_native_permission_timeout_secs(Some(0)), 0);
        assert_eq!(
            normalize_native_permission_timeout_secs(Some(DEFAULT_NATIVE_PERMISSION_TIMEOUT_SECS)),
            DEFAULT_NATIVE_PERMISSION_TIMEOUT_SECS
        );
        assert_eq!(
            normalize_native_permission_timeout_secs(Some(-1)),
            DEFAULT_NATIVE_PERMISSION_TIMEOUT_SECS
        );
    }
}
