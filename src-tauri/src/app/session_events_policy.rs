use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::db::models::SessionEventsPolicy;

pub const DEFAULT_RETENTION_DAYS: i32 = 30;
pub const MIN_RETENTION_DAYS: i32 = 1;
pub const MAX_RETENTION_DAYS: i32 = 3650;

const POLICY_FILE_NAME: &str = "session-events-policy.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RawSessionEventsPolicy {
    #[serde(default)]
    retention_days: Option<i32>,
}

/// Normalize retention days to the valid range `1..=3650`.
/// Values outside the range fall back to the default (30).
pub fn normalize_retention_days(days: i32) -> i32 {
    if (MIN_RETENTION_DAYS..=MAX_RETENTION_DAYS).contains(&days) {
        days
    } else {
        DEFAULT_RETENTION_DAYS
    }
}

fn policy_file_path_for_dir(config_dir: &Path) -> PathBuf {
    config_dir.join(POLICY_FILE_NAME)
}

fn app_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法读取应用配置目录: {error}"))
}

pub fn policy_file_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(policy_file_path_for_dir(&app_config_dir(app)?))
}

fn parse_policy_from_raw(raw: &str) -> Result<SessionEventsPolicy, String> {
    let parsed: RawSessionEventsPolicy = serde_json::from_str(raw)
        .map_err(|error| format!("解析会话事件保留策略失败: {error}"))?;
    Ok(SessionEventsPolicy {
        retention_days: normalize_retention_days(
            parsed.retention_days.unwrap_or(DEFAULT_RETENTION_DAYS),
        ),
    })
}

/// Load policy from an explicit path (used by tests and runtime).
pub fn load_policy_from_path(path: &Path) -> Result<SessionEventsPolicy, String> {
    if !path.exists() {
        return Ok(SessionEventsPolicy {
            retention_days: DEFAULT_RETENTION_DAYS,
        });
    }

    let raw = fs::read_to_string(path)
        .map_err(|error| format!("读取会话事件保留策略失败: {error}"))?;
    parse_policy_from_raw(&raw)
}

/// Save policy to an explicit path (used by tests and runtime).
pub fn save_policy_to_path(path: &Path, policy: &SessionEventsPolicy) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建会话事件保留策略目录失败: {error}"))?;
    }

    let normalized = SessionEventsPolicy {
        retention_days: normalize_retention_days(policy.retention_days),
    };
    let raw = serde_json::to_string_pretty(&RawSessionEventsPolicy {
        retention_days: Some(normalized.retention_days),
    })
    .map_err(|error| format!("序列化会话事件保留策略失败: {error}"))?;

    fs::write(path, raw).map_err(|error| format!("写入会话事件保留策略失败: {error}"))
}

pub fn load_session_events_policy<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<SessionEventsPolicy, String> {
    load_policy_from_path(&policy_file_path(app)?)
}

pub fn save_session_events_policy<R: Runtime>(
    app: &AppHandle<R>,
    retention_days: i32,
) -> Result<SessionEventsPolicy, String> {
    let policy = SessionEventsPolicy {
        retention_days: normalize_retention_days(retention_days),
    };
    save_policy_to_path(&policy_file_path(app)?, &policy)?;
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn normalize_retention_days_keeps_valid_range() {
        assert_eq!(normalize_retention_days(1), 1);
        assert_eq!(normalize_retention_days(30), 30);
        assert_eq!(normalize_retention_days(3650), 3650);
        assert_eq!(normalize_retention_days(90), 90);
    }

    #[test]
    fn normalize_retention_days_rejects_out_of_range() {
        assert_eq!(normalize_retention_days(0), DEFAULT_RETENTION_DAYS);
        assert_eq!(normalize_retention_days(-1), DEFAULT_RETENTION_DAYS);
        assert_eq!(normalize_retention_days(4000), DEFAULT_RETENTION_DAYS);
        assert_eq!(normalize_retention_days(i32::MAX), DEFAULT_RETENTION_DAYS);
    }

    #[test]
    fn load_missing_policy_returns_default() {
        let dir = std::env::temp_dir().join(format!(
            "codex-ai-session-events-policy-missing-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = policy_file_path_for_dir(&dir);
        let policy = load_policy_from_path(&path).expect("load missing policy");
        assert_eq!(policy.retention_days, DEFAULT_RETENTION_DAYS);
    }

    #[test]
    fn save_and_load_policy_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "codex-ai-session-events-policy-roundtrip-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = policy_file_path_for_dir(&dir);

        let saved = SessionEventsPolicy {
            retention_days: 60,
        };
        save_policy_to_path(&path, &saved).expect("save policy");
        let loaded = load_policy_from_path(&path).expect("load policy");
        assert_eq!(loaded.retention_days, 60);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_policy_normalizes_invalid_days() {
        let dir = std::env::temp_dir().join(format!(
            "codex-ai-session-events-policy-normalize-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = policy_file_path_for_dir(&dir);

        save_policy_to_path(
            &path,
            &SessionEventsPolicy {
                retention_days: 0,
            },
        )
        .expect("save policy");
        let loaded = load_policy_from_path(&path).expect("load policy");
        assert_eq!(loaded.retention_days, DEFAULT_RETENTION_DAYS);

        let _ = fs::remove_dir_all(&dir);
    }
}
