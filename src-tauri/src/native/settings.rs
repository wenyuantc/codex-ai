use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{insert_activity_log, sqlite_pool};
use crate::db::models::{NativeSettings, UpdateNativeSettings};

const SETTINGS_FILE_NAME: &str = "native-settings.json";
pub const DEFAULT_NATIVE_MAX_TURNS: i32 = 40;
const MAX_NATIVE_MAX_TURNS: i32 = 500;

#[derive(Debug, Default, Deserialize, Serialize)]
struct RawNativeSettings {
    #[serde(default)]
    max_turns: Option<i32>,
    #[serde(default)]
    confirm_high_risk: Option<bool>,
}

pub fn normalize_native_max_turns(value: Option<i32>) -> i32 {
    match value {
        Some(value) if (0..=MAX_NATIVE_MAX_TURNS).contains(&value) => value,
        _ => DEFAULT_NATIVE_MAX_TURNS,
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
    }
}

fn normalize_settings(raw: RawNativeSettings) -> NativeSettings {
    NativeSettings {
        max_turns: normalize_native_max_turns(raw.max_turns),
        confirm_high_risk: raw.confirm_high_risk.unwrap_or(true),
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
    save_native_settings(app, &next)?;
    if previous.max_turns != next.max_turns || previous.confirm_high_risk != next.confirm_high_risk
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
        "{}；高风险确认：{}",
        max_turns_activity_details(settings.max_turns),
        if settings.confirm_high_risk {
            "开启"
        } else {
            "关闭"
        }
    )
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
        });
        assert!(settings.confirm_high_risk);
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
