use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

use crate::app::now_sqlite;

/// Legacy plaintext store — migration source only.
const SECRET_STORE_FILE_NAME: &str = "ssh-secrets.json";
/// Index of secret refs (no password values).
const SECRET_INDEX_FILE_NAME: &str = "ssh-secret-index.json";
const SECRET_INDEX_VERSION: u32 = 2;
const KEYRING_SERVICE: &str = "codex-ai-ssh";

// ---------------------------------------------------------------------------
// Backend abstraction
// ---------------------------------------------------------------------------

trait SecretBackend {
    fn set(&self, secret_ref: &str, value: &str) -> Result<(), String>;
    fn get(&self, secret_ref: &str) -> Result<Option<String>, String>;
    fn delete(&self, secret_ref: &str) -> Result<(), String>;
}

/// In-memory backend for unit tests (does not touch OS keychain).
#[cfg(test)]
struct MemoryBackend {
    store: Mutex<HashMap<String, String>>,
}

#[cfg(test)]
impl MemoryBackend {
    fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

#[cfg(test)]
impl SecretBackend for MemoryBackend {
    fn set(&self, secret_ref: &str, value: &str) -> Result<(), String> {
        let mut guard = self
            .store
            .lock()
            .map_err(|_| "内存密钥后端锁定失败".to_string())?;
        guard.insert(secret_ref.to_string(), value.to_string());
        Ok(())
    }

    fn get(&self, secret_ref: &str) -> Result<Option<String>, String> {
        let guard = self
            .store
            .lock()
            .map_err(|_| "内存密钥后端锁定失败".to_string())?;
        Ok(guard.get(secret_ref).cloned())
    }

    fn delete(&self, secret_ref: &str) -> Result<(), String> {
        let mut guard = self
            .store
            .lock()
            .map_err(|_| "内存密钥后端锁定失败".to_string())?;
        guard.remove(secret_ref);
        Ok(())
    }
}

/// Production backend backed by the OS credential store via `keyring`.
struct KeyringBackend {
    service: &'static str,
}

impl KeyringBackend {
    fn new() -> Self {
        Self {
            service: KEYRING_SERVICE,
        }
    }

    fn entry(&self, secret_ref: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(self.service, secret_ref).map_err(map_keyring_error)
    }
}

impl SecretBackend for KeyringBackend {
    fn set(&self, secret_ref: &str, value: &str) -> Result<(), String> {
        self.entry(secret_ref)?
            .set_password(value)
            .map_err(map_keyring_error)
    }

    fn get(&self, secret_ref: &str) -> Result<Option<String>, String> {
        match self.entry(secret_ref)?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(map_keyring_error(error)),
        }
    }

    fn delete(&self, secret_ref: &str) -> Result<(), String> {
        match self.entry(secret_ref)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_error(error)),
        }
    }
}

const SECRET_SERVICE_UNAVAILABLE_MSG: &str =
    "系统凭据服务不可用，无法保存 SSH 密码。请安装/启用 Secret Service（如 gnome-keyring）后重试";

fn looks_like_missing_secret_service(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("secret service")
        || lower.contains("no secret service")
        || lower.contains("org.freedesktop.secrets")
        || (lower.contains("dbus")
            && (lower.contains("connect")
                || lower.contains("unavailable")
                || lower.contains("failed")))
        || lower.contains("platform secure storage failure")
        || lower.contains("couldn't access platform secure storage")
}

fn map_keyring_error(error: keyring::Error) -> String {
    // Never fall back to plaintext. Map unavailable/locked store to a clear Chinese message.
    match error {
        keyring::Error::NoStorageAccess(err) => {
            let detail = err.to_string();
            // Linux locked/missing Secret Service typically surfaces here.
            if cfg!(target_os = "linux") || looks_like_missing_secret_service(&detail) {
                SECRET_SERVICE_UNAVAILABLE_MSG.to_string()
            } else {
                format!("系统凭据库操作失败: 无法访问凭据存储: {detail}")
            }
        }
        keyring::Error::PlatformFailure(err) => {
            let detail = err.to_string();
            if looks_like_missing_secret_service(&detail) {
                SECRET_SERVICE_UNAVAILABLE_MSG.to_string()
            } else {
                format!("系统凭据库操作失败: {detail}")
            }
        }
        other => {
            let message = other.to_string();
            if looks_like_missing_secret_service(&message) {
                SECRET_SERVICE_UNAVAILABLE_MSG.to_string()
            } else {
                format!("系统凭据库操作失败: {message}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Index document (v2, no password values)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct SecretMeta {
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SecretIndexDocument {
    #[serde(default = "default_index_version")]
    version: u32,
    #[serde(default)]
    entries: HashMap<String, SecretMeta>,
}

fn default_index_version() -> u32 {
    SECRET_INDEX_VERSION
}

impl Default for SecretIndexDocument {
    fn default() -> Self {
        Self {
            version: SECRET_INDEX_VERSION,
            entries: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Legacy plaintext document (migration source)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct LegacySecretEntry {
    value: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LegacySecretStoreDocument {
    #[serde(default)]
    entries: HashMap<String, LegacySecretEntry>,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn app_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法读取应用配置目录: {error}"))
}

fn legacy_store_path(config_dir: &Path) -> PathBuf {
    config_dir.join(SECRET_STORE_FILE_NAME)
}

fn index_path(config_dir: &Path) -> PathBuf {
    config_dir.join(SECRET_INDEX_FILE_NAME)
}

fn tighten_secret_store_permissions(path: &Path) -> Result<(), String> {
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

fn load_index_document(config_dir: &Path) -> Result<SecretIndexDocument, String> {
    let path = index_path(config_dir);
    if !path.exists() {
        return Ok(SecretIndexDocument::default());
    }

    let raw =
        fs::read_to_string(&path).map_err(|error| format!("读取密钥索引失败: {error}"))?;
    let mut document: SecretIndexDocument = serde_json::from_str(&raw)
        .map_err(|error| format!("解析密钥索引失败: {error}"))?;
    if document.version == 0 {
        document.version = SECRET_INDEX_VERSION;
    }
    Ok(document)
}

fn save_index_document(config_dir: &Path, document: &SecretIndexDocument) -> Result<(), String> {
    let path = index_path(config_dir);
    fs::create_dir_all(config_dir)
        .map_err(|error| format!("创建密钥索引目录失败: {error}"))?;
    let raw = serde_json::to_string_pretty(document)
        .map_err(|error| format!("序列化密钥索引失败: {error}"))?;

    // Write via temp + rename so a crash mid-write cannot leave a truncated index.
    let tmp_path = config_dir.join(format!(
        ".{SECRET_INDEX_FILE_NAME}.{}.tmp",
        std::process::id()
    ));
    fs::write(&tmp_path, raw.as_bytes())
        .map_err(|error| format!("写入密钥索引失败: {error}"))?;
    // Best-effort: if rename fails because target exists (Windows), remove then retry.
    if let Err(error) = fs::rename(&tmp_path, &path) {
        let _ = fs::remove_file(&path);
        fs::rename(&tmp_path, &path).map_err(|rename_error| {
            let _ = fs::remove_file(&tmp_path);
            format!("写入密钥索引失败: {error}; 重试: {rename_error}")
        })?;
    }
    tighten_secret_store_permissions(&path)?;
    Ok(())
}

fn load_legacy_document(config_dir: &Path) -> Result<Option<LegacySecretStoreDocument>, String> {
    let path = legacy_store_path(config_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("读取旧版密钥文件失败: {error}"))?;
    let document = serde_json::from_str::<LegacySecretStoreDocument>(&raw)
        .map_err(|error| format!("解析旧版密钥文件失败: {error}"))?;
    Ok(Some(document))
}

/// Migrate legacy plaintext JSON into backend + index.
///
/// On success the legacy file is deleted. On failure the legacy file is kept
/// so the operation can be retried (no silent plaintext fallback).
fn ensure_migrated(config_dir: &Path, backend: &dyn SecretBackend) -> Result<(), String> {
    let legacy = match load_legacy_document(config_dir) {
        Ok(None) => return Ok(()),
        Ok(Some(doc)) => doc,
        Err(error) => {
            return Err(format!("迁移 SSH 密钥到系统凭据库失败: {error}"));
        }
    };

    migrate_from_legacy_document(config_dir, backend, &legacy).map_err(|error| {
        format!("迁移 SSH 密钥到系统凭据库失败: {error}")
    })?;

    let legacy_path = legacy_store_path(config_dir);
    if legacy_path.exists() {
        fs::remove_file(&legacy_path).map_err(|error| {
            format!("迁移 SSH 密钥到系统凭据库失败: 删除旧版明文文件失败: {error}")
        })?;
    }

    Ok(())
}

/// Pure migration core: write values to backend, meta to index (no password in index).
fn migrate_from_legacy_document(
    config_dir: &Path,
    backend: &dyn SecretBackend,
    legacy: &LegacySecretStoreDocument,
) -> Result<(), String> {
    let mut index = load_index_document(config_dir)?;

    for (secret_ref, entry) in &legacy.entries {
        backend.set(secret_ref, &entry.value)?;
        index.entries.insert(
            secret_ref.clone(),
            SecretMeta {
                created_at: entry.created_at.clone(),
                updated_at: entry.updated_at.clone(),
            },
        );
    }

    index.version = SECRET_INDEX_VERSION;
    save_index_document(config_dir, &index)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Pure API cores (path + backend; unit-testable without AppHandle)
// ---------------------------------------------------------------------------

fn store_secret_value_with(
    config_dir: &Path,
    backend: &dyn SecretBackend,
    value: Option<&str>,
    replace_ref: Option<&str>,
) -> Result<Option<String>, String> {
    ensure_migrated(config_dir, backend)?;
    let normalized = value.map(str::trim).filter(|value| !value.is_empty());
    let mut index = load_index_document(config_dir)?;

    // Drop replace target from the in-memory index first; only delete the old
    // keyring entry after the new index is durable, so a failed store cannot
    // leave the user with neither old nor new password.
    let replace_ref = replace_ref
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(ref replace_ref) = replace_ref {
        index.entries.remove(replace_ref);
    }

    let Some(value) = normalized else {
        save_index_document(config_dir, &index)?;
        if let Some(replace_ref) = replace_ref {
            let _ = backend.delete(&replace_ref);
        }
        return Ok(None);
    };

    let secret_ref = format!("ssh-secret-{}", Uuid::new_v4());
    let now = now_sqlite();
    backend.set(&secret_ref, value)?;
    index.entries.insert(
        secret_ref.clone(),
        SecretMeta {
            created_at: now.clone(),
            updated_at: now,
        },
    );
    index.version = SECRET_INDEX_VERSION;
    if let Err(error) = save_index_document(config_dir, &index) {
        // Avoid orphan credentials when index persistence fails.
        let _ = backend.delete(&secret_ref);
        return Err(error);
    }
    if let Some(replace_ref) = replace_ref {
        let _ = backend.delete(&replace_ref);
    }
    Ok(Some(secret_ref))
}

fn resolve_secret_value_with(
    config_dir: &Path,
    backend: &dyn SecretBackend,
    secret_ref: Option<&str>,
) -> Result<Option<String>, String> {
    ensure_migrated(config_dir, backend)?;
    let Some(secret_ref) = secret_ref.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let index = load_index_document(config_dir)?;
    if !index.entries.contains_key(secret_ref) {
        return Ok(None);
    }

    match backend.get(secret_ref)? {
        Some(value) => Ok(Some(value)),
        None => Err(format!(
            "密钥索引存在但系统凭据库中缺少对应条目（可能已损坏）: {secret_ref}"
        )),
    }
}

fn delete_secret_value_with(
    config_dir: &Path,
    backend: &dyn SecretBackend,
    secret_ref: Option<&str>,
) -> Result<(), String> {
    ensure_migrated(config_dir, backend)?;
    let Some(secret_ref) = secret_ref.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    backend.delete(secret_ref)?;
    let mut index = load_index_document(config_dir)?;
    if index.entries.remove(secret_ref).is_some() {
        save_index_document(config_dir, &index)?;
    }
    Ok(())
}

fn sweep_orphan_secret_refs_with(
    config_dir: &Path,
    backend: &dyn SecretBackend,
    active_refs: &HashSet<String>,
) -> Result<usize, String> {
    ensure_migrated(config_dir, backend)?;
    let mut index = load_index_document(config_dir)?;
    let orphans: Vec<String> = index
        .entries
        .keys()
        .filter(|secret_ref| !active_refs.contains(*secret_ref))
        .cloned()
        .collect();

    let removed = orphans.len();
    for secret_ref in &orphans {
        backend.delete(secret_ref)?;
        index.entries.remove(secret_ref);
    }

    if removed > 0 {
        save_index_document(config_dir, &index)?;
    }
    Ok(removed)
}

// ---------------------------------------------------------------------------
// Public API (signatures unchanged)
// ---------------------------------------------------------------------------

pub fn store_secret_value<R: Runtime>(
    app: &AppHandle<R>,
    value: Option<&str>,
    replace_ref: Option<&str>,
) -> Result<Option<String>, String> {
    let config_dir = app_config_dir(app)?;
    let backend = KeyringBackend::new();
    store_secret_value_with(&config_dir, &backend, value, replace_ref)
}

pub fn resolve_secret_value<R: Runtime>(
    app: &AppHandle<R>,
    secret_ref: Option<&str>,
) -> Result<Option<String>, String> {
    let config_dir = app_config_dir(app)?;
    let backend = KeyringBackend::new();
    resolve_secret_value_with(&config_dir, &backend, secret_ref)
}

pub fn delete_secret_value<R: Runtime>(
    app: &AppHandle<R>,
    secret_ref: Option<&str>,
) -> Result<(), String> {
    let config_dir = app_config_dir(app)?;
    let backend = KeyringBackend::new();
    delete_secret_value_with(&config_dir, &backend, secret_ref)
}

pub fn sweep_orphan_secret_refs<R: Runtime>(
    app: &AppHandle<R>,
    active_refs: &HashSet<String>,
) -> Result<usize, String> {
    let config_dir = app_config_dir(app)?;
    let backend = KeyringBackend::new();
    sweep_orphan_secret_refs_with(&config_dir, &backend, active_refs)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_config_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("codex-ai-secret-test-{nanos}"));
        fs::create_dir_all(&dir).expect("create temp config dir");
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn store_and_resolve_roundtrip() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();

        let secret_ref = store_secret_value_with(&dir, &backend, Some("p@ssw0rd"), None)
            .expect("store")
            .expect("ref");
        assert!(secret_ref.starts_with("ssh-secret-"));

        let resolved = resolve_secret_value_with(&dir, &backend, Some(&secret_ref))
            .expect("resolve")
            .expect("value");
        assert_eq!(resolved, "p@ssw0rd");

        // Index must not contain plaintext password
        let raw = fs::read_to_string(index_path(&dir)).expect("read index");
        assert!(!raw.contains("p@ssw0rd"));
        assert!(raw.contains(&secret_ref));
        assert!(!raw.contains("\"value\""));

        cleanup(&dir);
    }

    #[test]
    fn delete_removes_from_backend_and_index() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();

        let secret_ref = store_secret_value_with(&dir, &backend, Some("to-delete"), None)
            .expect("store")
            .expect("ref");

        delete_secret_value_with(&dir, &backend, Some(&secret_ref)).expect("delete");

        let resolved =
            resolve_secret_value_with(&dir, &backend, Some(&secret_ref)).expect("resolve");
        assert!(resolved.is_none());
        assert!(backend.get(&secret_ref).expect("get").is_none());

        cleanup(&dir);
    }

    #[test]
    fn replace_ref_deletes_old_value() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();

        let old_ref = store_secret_value_with(&dir, &backend, Some("old-secret"), None)
            .expect("store old")
            .expect("old ref");

        let new_ref =
            store_secret_value_with(&dir, &backend, Some("new-secret"), Some(&old_ref))
                .expect("store new")
                .expect("new ref");

        assert_ne!(old_ref, new_ref);
        assert!(backend.get(&old_ref).expect("old get").is_none());
        assert_eq!(
            backend.get(&new_ref).expect("new get").as_deref(),
            Some("new-secret")
        );
        assert!(
            resolve_secret_value_with(&dir, &backend, Some(&old_ref))
                .expect("resolve old")
                .is_none()
        );
        assert_eq!(
            resolve_secret_value_with(&dir, &backend, Some(&new_ref))
                .expect("resolve new")
                .as_deref(),
            Some("new-secret")
        );

        cleanup(&dir);
    }

    #[test]
    fn store_none_with_replace_clears_password() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();

        let old_ref = store_secret_value_with(&dir, &backend, Some("clear-me"), None)
            .expect("store")
            .expect("ref");

        let result = store_secret_value_with(&dir, &backend, None, Some(&old_ref)).expect("clear");
        assert!(result.is_none());
        assert!(backend.get(&old_ref).expect("get").is_none());

        cleanup(&dir);
    }

    #[test]
    fn sweep_only_removes_orphans() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();

        let keep = store_secret_value_with(&dir, &backend, Some("keep"), None)
            .expect("store keep")
            .expect("keep ref");
        let drop_ref = store_secret_value_with(&dir, &backend, Some("drop"), None)
            .expect("store drop")
            .expect("drop ref");

        let mut active = HashSet::new();
        active.insert(keep.clone());

        let removed =
            sweep_orphan_secret_refs_with(&dir, &backend, &active).expect("sweep");
        assert_eq!(removed, 1);
        assert!(backend.get(&drop_ref).expect("drop get").is_none());
        assert_eq!(
            resolve_secret_value_with(&dir, &backend, Some(&keep))
                .expect("resolve keep")
                .as_deref(),
            Some("keep")
        );

        cleanup(&dir);
    }

    #[test]
    fn migrate_legacy_json_to_backend_and_index_without_plaintext() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();

        let legacy_ref = "ssh-secret-legacy-001";
        let legacy_password = "legacy-plain-password-xyz";

        let legacy_path = legacy_store_path(&dir);
        let raw = serde_json::to_string_pretty(&serde_json::json!({
            "entries": {
                legacy_ref: {
                    "value": legacy_password,
                    "created_at": "2026-01-01 00:00:00",
                    "updated_at": "2026-01-02 00:00:00"
                }
            }
        }))
        .expect("serialize legacy");
        fs::write(&legacy_path, raw).expect("write legacy");
        assert!(legacy_path.exists());

        // Touch migration via public pure entry
        ensure_migrated(&dir, &backend).expect("migrate");

        // Legacy file removed
        assert!(
            !legacy_path.exists(),
            "legacy ssh-secrets.json must be deleted after successful migration"
        );

        // Backend holds password
        assert_eq!(
            backend.get(legacy_ref).expect("backend get").as_deref(),
            Some(legacy_password)
        );

        // Resolve works
        assert_eq!(
            resolve_secret_value_with(&dir, &backend, Some(legacy_ref))
                .expect("resolve")
                .as_deref(),
            Some(legacy_password)
        );

        // Index exists, has meta, no plaintext password / no value field
        let index_raw = fs::read_to_string(index_path(&dir)).expect("read index");
        assert!(index_raw.contains(legacy_ref));
        assert!(index_raw.contains("created_at"));
        assert!(!index_raw.contains(legacy_password));
        assert!(
            !index_raw.contains("\"value\""),
            "index JSON must not contain value field: {index_raw}"
        );

        // Index meta timestamps preserved
        let index = load_index_document(&dir).expect("load index");
        let meta = index.entries.get(legacy_ref).expect("meta");
        assert_eq!(meta.created_at, "2026-01-01 00:00:00");
        assert_eq!(meta.updated_at, "2026-01-02 00:00:00");
        assert_eq!(index.version, SECRET_INDEX_VERSION);

        // Idempotent: no legacy → ensure_migrated ok
        ensure_migrated(&dir, &backend).expect("idempotent migrate");

        cleanup(&dir);
    }

    #[test]
    fn resolve_missing_index_returns_none() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();
        let resolved =
            resolve_secret_value_with(&dir, &backend, Some("ssh-secret-nope")).expect("resolve");
        assert!(resolved.is_none());
        cleanup(&dir);
    }

    #[test]
    fn resolve_index_present_backend_missing_is_error() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();

        let secret_ref = store_secret_value_with(&dir, &backend, Some("sync-me"), None)
            .expect("store")
            .expect("ref");

        // Corrupt: drop from backend only
        backend.delete(&secret_ref).expect("delete backend");

        let err = resolve_secret_value_with(&dir, &backend, Some(&secret_ref))
            .expect_err("should error on corrupted state");
        assert!(
            err.contains("损坏") || err.contains("缺少"),
            "unexpected error: {err}"
        );

        cleanup(&dir);
    }

    #[test]
    fn index_serialization_has_no_value_field() {
        let doc = SecretIndexDocument {
            version: SECRET_INDEX_VERSION,
            entries: HashMap::from([(
                "ssh-secret-abc".to_string(),
                SecretMeta {
                    created_at: "t1".to_string(),
                    updated_at: "t2".to_string(),
                },
            )]),
        };
        let raw = serde_json::to_string(&doc).expect("serialize");
        assert!(!raw.contains("\"value\""));
        assert!(raw.contains("\"version\":2") || raw.contains("\"version\": 2"));
    }

    #[test]
    fn migrate_failure_keeps_legacy_file() {
        let dir = temp_config_dir();
        // Backend that always fails set
        struct FailBackend;
        impl SecretBackend for FailBackend {
            fn set(&self, _: &str, _: &str) -> Result<(), String> {
                Err("模拟凭据库写入失败".to_string())
            }
            fn get(&self, _: &str) -> Result<Option<String>, String> {
                Ok(None)
            }
            fn delete(&self, _: &str) -> Result<(), String> {
                Ok(())
            }
        }

        let legacy_path = legacy_store_path(&dir);
        fs::write(
            &legacy_path,
            r#"{"entries":{"ssh-secret-x":{"value":"secret","created_at":"a","updated_at":"b"}}}"#,
        )
        .expect("write legacy");

        let err = ensure_migrated(&dir, &FailBackend).expect_err("migrate should fail");
        assert!(
            err.contains("迁移 SSH 密钥到系统凭据库失败"),
            "unexpected: {err}"
        );
        assert!(
            legacy_path.exists(),
            "legacy file must remain after failed migration"
        );

        cleanup(&dir);
    }

    #[test]
    fn migrate_corrupt_legacy_keeps_file_and_prefixes_error() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();
        let legacy_path = legacy_store_path(&dir);
        fs::write(&legacy_path, "{not-json").expect("write corrupt legacy");

        let err = ensure_migrated(&dir, &backend).expect_err("corrupt legacy should fail");
        assert!(
            err.contains("迁移 SSH 密钥到系统凭据库失败"),
            "unexpected: {err}"
        );
        assert!(legacy_path.exists(), "corrupt legacy must be kept for retry");

        cleanup(&dir);
    }

    #[test]
    fn migrate_merges_with_existing_index_entries() {
        let dir = temp_config_dir();
        let backend = MemoryBackend::new();

        let existing_ref = store_secret_value_with(&dir, &backend, Some("keep-me"), None)
            .expect("store existing")
            .expect("ref");

        let legacy_ref = "ssh-secret-legacy-merge";
        let legacy_password = "merged-legacy-password";
        fs::write(
            legacy_store_path(&dir),
            serde_json::to_string(&serde_json::json!({
                "entries": {
                    legacy_ref: {
                        "value": legacy_password,
                        "created_at": "2026-01-01 00:00:00",
                        "updated_at": "2026-01-01 00:00:00"
                    }
                }
            }))
            .expect("serialize"),
        )
        .expect("write legacy");

        ensure_migrated(&dir, &backend).expect("migrate");

        assert_eq!(
            resolve_secret_value_with(&dir, &backend, Some(&existing_ref))
                .expect("resolve existing")
                .as_deref(),
            Some("keep-me")
        );
        assert_eq!(
            resolve_secret_value_with(&dir, &backend, Some(legacy_ref))
                .expect("resolve legacy")
                .as_deref(),
            Some(legacy_password)
        );
        assert!(!legacy_store_path(&dir).exists());

        let index_raw = fs::read_to_string(index_path(&dir)).expect("read index");
        assert!(!index_raw.contains(legacy_password));
        assert!(!index_raw.contains("\"value\""));

        cleanup(&dir);
    }

    #[test]
    fn looks_like_missing_secret_service_detects_common_messages() {
        assert!(looks_like_missing_secret_service(
            "Platform secure storage failure: dbus connect failed"
        ));
        assert!(looks_like_missing_secret_service(
            "Couldn't access platform secure storage: no secret service"
        ));
        assert!(!looks_like_missing_secret_service("No matching entry found"));
    }
}
