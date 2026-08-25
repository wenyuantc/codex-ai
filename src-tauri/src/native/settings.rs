use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{insert_activity_log, sqlite_pool};
use crate::db::models::{NativeSettings, UpdateNativeSettings};

const SETTINGS_FILE_NAME: &str = "native-settings.json";
pub const DEFAULT_NATIVE_MAX_TURNS: i32 = 40;
const MAX_NATIVE_MAX_TURNS: i32 = 500;
pub const DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS: i32 = 3;
const MAX_NATIVE_MAX_CONCURRENT_SUBAGENTS: i32 = 16;
pub const SUBAGENT_POLICY_CONSERVATIVE: &str = "conservative";
pub const SUBAGENT_POLICY_BALANCED: &str = "balanced";
pub const SUBAGENT_POLICY_AGGRESSIVE: &str = "aggressive";
pub const DEFAULT_NATIVE_SUBAGENT_POLICY: &str = SUBAGENT_POLICY_BALANCED;

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
    save_native_settings(app, &next)?;
    if previous.max_turns != next.max_turns
        || previous.confirm_high_risk != next.confirm_high_risk
        || previous.max_concurrent_subagents != next.max_concurrent_subagents
        || previous.subagent_policy != next.subagent_policy
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
        "{}；高风险确认：{}；同轮子 Agent 上限：{}；子 Agent 策略：{}",
        max_turns_activity_details(settings.max_turns),
        if settings.confirm_high_risk {
            "开启"
        } else {
            "关闭"
        },
        settings.max_concurrent_subagents,
        subagent_policy_label_zh(&settings.subagent_policy)
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
        });
        assert!(settings.confirm_high_risk);
        assert_eq!(
            settings.max_concurrent_subagents,
            DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS
        );
        assert_eq!(settings.subagent_policy, DEFAULT_NATIVE_SUBAGENT_POLICY);
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
            normalize_native_max_concurrent_subagents(Some(3)),
            DEFAULT_NATIVE_MAX_CONCURRENT_SUBAGENTS
        );
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
}
