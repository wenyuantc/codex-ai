use super::*;

pub const DB_URL: &str = "sqlite:codex-ai.db";
pub(crate) const PROJECT_TYPE_LOCAL: &str = "local";
pub(crate) const PROJECT_TYPE_SSH: &str = "ssh";
pub(crate) const EXECUTION_TARGET_LOCAL: &str = "local";
pub(crate) const EXECUTION_TARGET_SSH: &str = "ssh";
pub(crate) const ARTIFACT_CAPTURE_MODE_LOCAL_FULL: &str = "local_full";
pub(crate) const ARTIFACT_CAPTURE_MODE_SSH_FULL: &str = "ssh_full";
pub(crate) const ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS: &str = "ssh_git_status";
pub(crate) const ARTIFACT_CAPTURE_MODE_SSH_NONE: &str = "ssh_none";
pub(crate) const TASK_STATUS_ARCHIVED: &str = "archived";
pub(crate) const TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1: &str = "review_fix_loop_v1";
pub(crate) const TASK_AUTOMATION_PHASE_LAUNCHING_REVIEW: &str = "launching_review";
pub(crate) const TASK_AUTOMATION_PHASE_WAITING_REVIEW: &str = "waiting_review";
pub(crate) const TASK_AUTOMATION_PHASE_LAUNCHING_FIX: &str = "launching_fix";
pub(crate) const TASK_AUTOMATION_PHASE_WAITING_EXECUTION: &str = "waiting_execution";
pub(crate) const TASK_AUTOMATION_PHASE_COMMITTING_CODE: &str = "committing_code";
pub(crate) const DB_FILE_NAME: &str = "codex-ai.db";
pub(crate) const DB_AUTO_IMPORT_BACKUP_PREFIX: &str = "codex-ai.pre-import-backup";
pub(crate) const SQLITE_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
#[cfg(test)]
pub(crate) const REVIEW_DIFF_CHAR_LIMIT: usize = 120_000;
pub(crate) const FILE_CHANGE_DIFF_CHAR_LIMIT: usize = 120_000;
#[cfg(test)]
pub(crate) const REVIEW_UNTRACKED_FILE_LIMIT: usize = 5;
#[cfg(test)]
pub(crate) const REVIEW_UNTRACKED_FILE_SIZE_LIMIT: u64 = 16 * 1024;
#[cfg(test)]
pub(crate) const REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT: usize = 48_000;
pub(crate) const SDK_BRIDGE_FILE_NAME: &str = "sdk-bridge.mjs";
pub(crate) const SDK_RUNTIME_PACKAGE_JSON: &str =
    "{\"name\":\"codex-ai-sdk-runtime\",\"private\":true,\"type\":\"module\"}";
pub(crate) const REMOTE_TASK_ATTACHMENT_ROOT_DIR: &str = ".codex-ai/img";
pub(crate) const REVIEW_VERDICT_START_TAG: &str = "<review_verdict>";
pub(crate) const REVIEW_VERDICT_END_TAG: &str = "</review_verdict>";
pub(crate) const REVIEW_REPORT_START_TAG: &str = "<review_report>";
pub(crate) const REVIEW_REPORT_END_TAG: &str = "</review_report>";
pub(crate) const GLOBAL_SEARCH_MIN_QUERY_LENGTH: usize = 2;
pub(crate) const GLOBAL_SEARCH_DEFAULT_LIMIT: usize = 24;
pub(crate) const GLOBAL_SEARCH_MAX_LIMIT: usize = 50;
pub(crate) const GLOBAL_SEARCH_TYPE_PROJECT: &str = "project";
pub(crate) const GLOBAL_SEARCH_TYPE_TASK: &str = "task";
pub(crate) const GLOBAL_SEARCH_TYPE_EMPLOYEE: &str = "employee";
pub(crate) const GLOBAL_SEARCH_TYPE_SESSION: &str = "session";

pub(crate) struct DatabaseMigrationStatus {
    pub(crate) applied_count: i64,
    pub(crate) current_version: Option<i64>,
    pub(crate) current_description: Option<String>,
}

pub(crate) struct RemoteCodexRuntimeHealth {
    pub codex_available: bool,
    pub codex_version: Option<String>,
    pub node_available: bool,
    pub node_version: Option<String>,
    pub sdk_installed: bool,
    pub sdk_version: Option<String>,
    pub task_execution_effective_provider: String,
    pub one_shot_effective_provider: String,
    pub status_message: String,
}

pub(crate) struct RemoteTaskAttachmentSyncResult {
    pub remote_paths: Vec<String>,
    pub skipped_local_paths: Vec<String>,
}

pub(crate) fn now_sqlite() -> String {
    Utc::now().format(SQLITE_DATETIME_FORMAT).to_string()
}

pub(crate) fn database_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(DB_FILE_NAME))
}

pub(crate) async fn sqlite_pool<R: Runtime>(app: &AppHandle<R>) -> Result<SqlitePool, String> {
    let instances = app.state::<DbInstances>();
    let instances = instances.0.read().await;
    let db = instances
        .get(DB_URL)
        .ok_or_else(|| format!("Database {} is not loaded", DB_URL))?;

    let DbPool::Sqlite(pool) = db;
    Ok(pool.clone())
}

pub(crate) fn resolve_user_file_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("路径不能为空".to_string());
    }

    let raw_path = PathBuf::from(trimmed);
    if raw_path.is_absolute() {
        Ok(raw_path)
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(raw_path))
            .map_err(|error| format!("无法解析当前工作目录: {}", error))
    }
}

pub(crate) fn resolve_existing_file_path(path: &str) -> Result<PathBuf, String> {
    let resolved = resolve_user_file_path(path)?;
    let canonical = resolved
        .canonicalize()
        .map_err(|error| format!("文件不存在或不可访问: {}", error))?;

    if !canonical.is_file() {
        return Err(format!("路径 {} 不是文件", canonical.display()));
    }

    Ok(canonical)
}

pub(crate) fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn ensure_git_repository(path: &Path) -> Result<(), String> {
    let git_dir = path.join(".git");
    if git_dir.exists() {
        return Ok(());
    }

    Err(format!(
        "工作目录 {} 不是 Git 仓库，缺少 .git",
        path.display()
    ))
}

fn canonicalize_existing_dir(path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("路径不能为空".to_string());
    }

    let canonical = Path::new(trimmed)
        .canonicalize()
        .map_err(|error| format!("路径不存在或不可访问: {}", error))?;

    if !canonical.is_dir() {
        return Err(format!("路径 {} 不是目录", canonical.display()));
    }

    Ok(path_to_runtime_string(&canonical))
}

pub(crate) fn validate_project_repo_path(
    repo_path: Option<&str>,
) -> Result<Option<String>, String> {
    match normalize_optional_text(repo_path) {
        Some(path) => canonicalize_existing_dir(&path).map(Some),
        None => Ok(None),
    }
}

pub(crate) fn normalize_project_type(value: Option<&str>) -> Result<String, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some(PROJECT_TYPE_LOCAL) => Ok(PROJECT_TYPE_LOCAL.to_string()),
        Some(PROJECT_TYPE_SSH) => Ok(PROJECT_TYPE_SSH.to_string()),
        Some(other) => Err(format!("不支持的项目类型: {other}")),
    }
}

pub(crate) fn validate_remote_repo_path(repo_path: Option<&str>) -> Result<Option<String>, String> {
    match normalize_optional_text(repo_path) {
        Some(path) => Ok(Some(path)),
        None => Ok(None),
    }
}

pub(crate) fn normalize_runtime_path_string(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

pub(crate) fn path_to_runtime_string(path: &Path) -> String {
    normalize_runtime_path_string(&path.to_string_lossy())
}

pub(crate) fn validate_runtime_working_dir(working_dir: Option<&str>) -> Result<String, String> {
    let resolved = match normalize_optional_text(working_dir) {
        Some(path) => canonicalize_existing_dir(&path)?,
        None => {
            let current_dir = std::env::current_dir()
                .map_err(|error| format!("无法解析当前工作目录: {}", error))?;
            let canonical = current_dir
                .canonicalize()
                .map_err(|error| format!("无法访问当前工作目录: {}", error))?;
            path_to_runtime_string(&canonical)
        }
    };

    ensure_git_repository(Path::new(&resolved))?;
    Ok(resolved)
}

pub(crate) fn parse_sqlite_datetime(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, SQLITE_DATETIME_FORMAT).ok()
}
