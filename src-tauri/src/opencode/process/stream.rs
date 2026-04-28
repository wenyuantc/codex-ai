use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use crate::db::models::CodexSessionFileChangeInput;

pub type SdkFileChangeStore = Arc<StdMutex<HashMap<String, CodexSessionFileChangeInput>>>;

const OPENCODE_FILE_CHANGE_EVENT_PREFIX: &str = "[OPENCODE_FILE_CHANGE]";

pub fn parse_opencode_sdk_file_change_event(line: &str) -> Option<SdkFileChangeEvent> {
    if !line.starts_with(OPENCODE_FILE_CHANGE_EVENT_PREFIX) {
        return None;
    }

    let json_str = line.trim_start_matches(OPENCODE_FILE_CHANGE_EVENT_PREFIX);
    serde_json::from_str::<SdkFileChangeEvent>(json_str).ok()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SdkFileChangeEvent {
    pub changes: Vec<SdkFileChangeItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SdkFileChangeItem {
    pub path: Option<String>,
    pub kind: Option<String>,
    pub previous_path: Option<String>,
}

pub fn normalize_file_change_kind(value: Option<&str>) -> Option<&'static str> {
    match value.map(|v| v.trim().to_ascii_lowercase()) {
        Some(v) if matches!(v.as_str(), "add" | "added" | "create" | "created") => Some("added"),
        Some(v)
            if matches!(
                v.as_str(),
                "modify"
                    | "modified"
                    | "update"
                    | "updated"
                    | "change"
                    | "changed"
                    | "edit"
                    | "edited"
            ) =>
        {
            Some("modified")
        }
        Some(v) if matches!(v.as_str(), "delete" | "deleted" | "remove" | "removed") => {
            Some("deleted")
        }
        Some(v) if matches!(v.as_str(), "rename" | "renamed" | "move" | "moved") => Some("renamed"),
        _ => None,
    }
}

pub fn upsert_sdk_file_change_event(store: &SdkFileChangeStore, event: SdkFileChangeEvent) {
    let mut guard = store.lock().unwrap();
    for change in event.changes {
        let path = change.path.unwrap_or_default().trim().to_string();
        if path.is_empty() {
            continue;
        }
        let Some(change_kind) = normalize_file_change_kind(change.kind.as_deref()) else {
            continue;
        };
        guard.insert(
            path.clone(),
            CodexSessionFileChangeInput {
                path,
                change_type: change_kind.to_string(),
                capture_mode: "sdk_event".to_string(),
                previous_path: change
                    .previous_path
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty()),
                detail: None,
            },
        );
    }
}

pub fn extract_session_id_from_opencode_output(line: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
        if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
            if !session_id.is_empty() {
                return Some(session_id.to_string());
            }
        }
        if let Some(session_id) = value.get("id").and_then(|v| v.as_str()) {
            if !session_id.is_empty() {
                return Some(session_id.to_string());
            }
        }
    }
    None
}
