use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use super::{insert_activity_log, sqlite_pool};
use crate::db::models::NotificationSoundSettings;
use crate::process_spawn::configure_std_command;

const SETTINGS_FILE_NAME: &str = "notification-sound.json";
const DEFAULT_ENABLED: bool = true;
#[cfg(target_os = "macos")]
const MACOS_NOTIFICATION_SOUND_PATH: &str = "/System/Library/Sounds/Glass.aiff";
#[cfg(target_os = "macos")]
const MACOS_NOTIFICATION_SOUND_VOLUME: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RawNotificationSoundSettings {
    #[serde(default)]
    enabled: Option<bool>,
}

fn settings_file_path_for_dir(config_dir: &Path) -> PathBuf {
    config_dir.join(SETTINGS_FILE_NAME)
}

fn app_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法读取应用配置目录: {error}"))
}

fn settings_file_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(settings_file_path_for_dir(&app_config_dir(app)?))
}

fn default_settings() -> NotificationSoundSettings {
    NotificationSoundSettings {
        enabled: DEFAULT_ENABLED,
    }
}

fn parse_settings_from_raw(raw: &str) -> NotificationSoundSettings {
    let parsed: RawNotificationSoundSettings = serde_json::from_str(raw).unwrap_or_default();
    NotificationSoundSettings {
        enabled: parsed.enabled.unwrap_or(DEFAULT_ENABLED),
    }
}

pub fn load_settings_from_path(path: &Path) -> Result<NotificationSoundSettings, String> {
    if !path.exists() {
        return Ok(default_settings());
    }

    let raw = fs::read_to_string(path).map_err(|error| format!("读取通知声音设置失败: {error}"))?;
    Ok(parse_settings_from_raw(&raw))
}

pub fn save_settings_to_path(
    path: &Path,
    settings: &NotificationSoundSettings,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建通知声音设置目录失败: {error}"))?;
    }

    let raw = serde_json::to_string_pretty(&RawNotificationSoundSettings {
        enabled: Some(settings.enabled),
    })
    .map_err(|error| format!("序列化通知声音设置失败: {error}"))?;

    fs::write(path, raw).map_err(|error| format!("写入通知声音设置失败: {error}"))
}

fn notification_sound_activity_details(enabled: bool) -> String {
    if enabled {
        "声音提醒：开启".to_string()
    } else {
        "声音提醒：关闭".to_string()
    }
}

pub fn load_notification_sound_settings<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<NotificationSoundSettings, String> {
    load_settings_from_path(&settings_file_path(app)?)
}

async fn persist_notification_sound_settings<R: Runtime>(
    app: &AppHandle<R>,
    enabled: bool,
) -> Result<NotificationSoundSettings, String> {
    let previous = load_notification_sound_settings(app)?;
    let next = NotificationSoundSettings { enabled };
    save_settings_to_path(&settings_file_path(app)?, &next)?;

    if previous.enabled != next.enabled {
        if let Ok(pool) = sqlite_pool(app).await {
            let _ = insert_activity_log(
                &pool,
                "notification_sound_settings_updated",
                &notification_sound_activity_details(next.enabled),
                None,
                None,
                None,
            )
            .await;
        }
    }

    Ok(next)
}

fn spawn_alert_command(program: &str, args: &[&str]) -> Result<(), String> {
    let mut command = Command::new(program);
    command.args(args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    configure_std_command(&mut command);
    let status = command
        .status()
        .map_err(|error| format!("播放通知提示音失败: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("播放通知提示音失败: {status}"))
    }
}

#[cfg(target_os = "macos")]
fn play_platform_notification_sound() -> Result<(), String> {
    spawn_alert_command(
        "afplay",
        &[
            "-v",
            MACOS_NOTIFICATION_SOUND_VOLUME,
            MACOS_NOTIFICATION_SOUND_PATH,
        ],
    )
}

#[cfg(target_os = "windows")]
fn play_platform_notification_sound() -> Result<(), String> {
    const MB_ICONASTERISK: u32 = 0x0000_0040;
    // SAFETY: MessageBeep is a documented user32 API and takes only a flag.
    let ok = unsafe { MessageBeep(MB_ICONASTERISK) };
    if ok == 0 {
        Err("播放系统提示音失败".to_string())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn MessageBeep(u_type: u32) -> i32;
}

#[cfg(target_os = "linux")]
fn play_platform_notification_sound() -> Result<(), String> {
    let candidates: &[(&str, &[&str])] = &[
        ("canberra-gtk-play", &["-i", "message"]),
        (
            "paplay",
            &["/usr/share/sounds/freedesktop/stereo/message.oga"],
        ),
        (
            "pw-play",
            &["/usr/share/sounds/freedesktop/stereo/message.oga"],
        ),
    ];
    let mut last_error = "未找到可用的系统提示音播放器".to_string();
    for (program, args) in candidates {
        match spawn_alert_command(program, args) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn play_platform_notification_sound() -> Result<(), String> {
    Err("当前平台不支持通知提示音".to_string())
}

#[tauri::command]
pub async fn play_notification_sound_alert() -> Result<(), String> {
    tokio::task::spawn_blocking(play_platform_notification_sound)
        .await
        .map_err(|error| format!("播放通知提示音失败: {error}"))?
}

#[tauri::command]
pub async fn get_notification_sound_settings<R: Runtime>(
    app: AppHandle<R>,
) -> Result<NotificationSoundSettings, String> {
    load_notification_sound_settings(&app)
}

#[tauri::command]
pub async fn update_notification_sound_settings<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
) -> Result<NotificationSoundSettings, String> {
    persist_notification_sound_settings(&app, enabled).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_settings_path(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "codex-ai-notification-sound-{}-{}",
            label,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        settings_file_path_for_dir(&dir)
    }

    #[test]
    fn load_missing_file_returns_enabled_default() {
        let path = temp_settings_path("missing");
        let settings = load_settings_from_path(&path).expect("load missing settings");
        assert!(settings.enabled);
    }

    #[test]
    fn load_invalid_json_returns_enabled_default() {
        let path = temp_settings_path("invalid");
        fs::write(&path, "{not json").expect("write invalid json");
        let settings = load_settings_from_path(&path).expect("load invalid settings");
        assert!(settings.enabled);
    }

    #[test]
    fn load_missing_enabled_field_returns_default() {
        let path = temp_settings_path("missing-field");
        fs::write(&path, "{}").expect("write empty object");
        let settings = load_settings_from_path(&path).expect("load empty object");
        assert!(settings.enabled);
    }

    #[test]
    fn save_and_load_settings_roundtrip() {
        let path = temp_settings_path("roundtrip");
        save_settings_to_path(&path, &NotificationSoundSettings { enabled: false })
            .expect("save settings");
        let loaded = load_settings_from_path(&path).expect("load settings");
        assert!(!loaded.enabled);

        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn activity_details_use_chinese_labels() {
        assert_eq!(notification_sound_activity_details(true), "声音提醒：开启");
        assert_eq!(notification_sound_activity_details(false), "声音提醒：关闭");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_uses_system_glass_sound() {
        assert!(std::path::Path::new(MACOS_NOTIFICATION_SOUND_PATH).exists());
        assert_eq!(MACOS_NOTIFICATION_SOUND_VOLUME, "1");
    }
}
