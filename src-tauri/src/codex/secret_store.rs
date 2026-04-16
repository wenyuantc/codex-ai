use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

use crate::app::now_sqlite;

const SECRET_STORE_FILE_NAME: &str = "ssh-secrets.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SecretEntry {
    value: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct SecretStoreDocument {
    #[serde(default)]
    entries: HashMap<String, SecretEntry>,
}

fn app_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法读取应用配置目录: {error}"))
}

fn secret_store_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join(SECRET_STORE_FILE_NAME))
}

fn tighten_secret_store_permissions(path: &PathBuf) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("设置 secret store 权限失败: {error}"))?;
    }

    #[cfg(not(unix))]
    let _ = path;

    Ok(())
}

fn load_secret_store_document<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<SecretStoreDocument, String> {
    let path = secret_store_path(app)?;
    if !path.exists() {
        return Ok(SecretStoreDocument::default());
    }

    let raw =
        fs::read_to_string(&path).map_err(|error| format!("读取 secret store 失败: {error}"))?;
    serde_json::from_str::<SecretStoreDocument>(&raw)
        .map_err(|error| format!("解析 secret store 失败: {error}"))
}

fn save_secret_store_document<R: Runtime>(
    app: &AppHandle<R>,
    document: &SecretStoreDocument,
) -> Result<(), String> {
    let path = secret_store_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析 secret store 目录: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("创建 secret store 目录失败: {error}"))?;
    let raw = serde_json::to_string_pretty(document)
        .map_err(|error| format!("序列化 secret store 失败: {error}"))?;
    fs::write(&path, raw).map_err(|error| format!("写入 secret store 失败: {error}"))?;
    tighten_secret_store_permissions(&path)?;
    Ok(())
}

pub fn store_secret_value<R: Runtime>(
    app: &AppHandle<R>,
    value: Option<&str>,
    replace_ref: Option<&str>,
) -> Result<Option<String>, String> {
    let normalized = value.map(str::trim).filter(|value| !value.is_empty());
    let mut document = load_secret_store_document(app)?;

    if let Some(replace_ref) = replace_ref {
        document.entries.remove(replace_ref);
    }

    let Some(value) = normalized else {
        save_secret_store_document(app, &document)?;
        return Ok(None);
    };

    let secret_ref = format!("ssh-secret-{}", Uuid::new_v4());
    let now = now_sqlite();
    document.entries.insert(
        secret_ref.clone(),
        SecretEntry {
            value: value.to_string(),
            created_at: now.clone(),
            updated_at: now,
        },
    );
    save_secret_store_document(app, &document)?;
    Ok(Some(secret_ref))
}

pub fn resolve_secret_value<R: Runtime>(
    app: &AppHandle<R>,
    secret_ref: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(secret_ref) = secret_ref.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let document = load_secret_store_document(app)?;
    Ok(document
        .entries
        .get(secret_ref)
        .map(|entry| entry.value.clone()))
}

pub fn delete_secret_value<R: Runtime>(
    app: &AppHandle<R>,
    secret_ref: Option<&str>,
) -> Result<(), String> {
    let Some(secret_ref) = secret_ref.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut document = load_secret_store_document(app)?;
    document.entries.remove(secret_ref);
    save_secret_store_document(app, &document)
}

pub fn sweep_orphan_secret_refs<R: Runtime>(
    app: &AppHandle<R>,
    active_refs: &HashSet<String>,
) -> Result<usize, String> {
    let mut document = load_secret_store_document(app)?;
    let before = document.entries.len();
    document
        .entries
        .retain(|secret_ref, _| active_refs.contains(secret_ref));
    let removed = before.saturating_sub(document.entries.len());
    if removed > 0 {
        save_secret_store_document(app, &document)?;
    }
    Ok(removed)
}
