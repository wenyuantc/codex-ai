use keyring::Entry;

const KEYRING_SERVICE: &str = "codex-ai-channel";
const SECRET_SERVICE_UNAVAILABLE_MSG: &str =
    "系统凭据服务不可用，无法保存渠道 API 密钥。请安装/启用 Secret Service（如 gnome-keyring）后重试";

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
    match error {
        keyring::Error::NoStorageAccess(err) => {
            let detail = err.to_string();
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

fn entry(secret_ref: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, secret_ref).map_err(map_keyring_error)
}

pub fn resolve_channel_api_key(secret_ref: &str) -> Result<Option<String>, String> {
    match entry(secret_ref)?.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(map_keyring_error(error)),
    }
}

pub fn delete_channel_api_key(secret_ref: &str) -> Result<(), String> {
    match entry(secret_ref)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(map_keyring_error(error)),
    }
}
