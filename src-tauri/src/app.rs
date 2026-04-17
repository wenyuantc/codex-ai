use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use chrono::{Duration, NaiveDateTime, Utc};
use sqlx::{
    migrate::{Migration as SqlxMigration, MigrationType as SqlxMigrationType, Migrator},
    QueryBuilder, Sqlite, SqlitePool,
};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_sql::{DbInstances, DbPool};
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use uuid::Uuid;

use crate::codex::{
    delete_secret_value, determine_effective_provider, ensure_supported_node_version,
    inspect_sdk_runtime, load_codex_settings, load_remote_codex_settings, new_codex_command,
    new_ssh_command, resolve_secret_value, store_secret_value, sweep_orphan_secret_refs,
    CodexManager,
};
use crate::db::models::{
    CodexHealthCheck, CodexOutput, CodexRuntimeStatus, CodexSessionFileChange,
    CodexSessionFileChangeDetail, CodexSessionFileChangeDetailRecord, CodexSessionFileChangeInput,
    CodexSessionListItem, CodexSessionLogLine, CodexSessionRecord, CodexSessionResumePreview,
    CodexSettings, Comment, CreateComment, CreateEmployee, CreateProject, CreateSshConfig,
    CreateSubtask, CreateTask, DatabaseBackupResult, DatabaseRestoreResult, Employee,
    EmployeeMetric, GlobalSearchItem, GlobalSearchResponse, PasswordAuthProbeResult, Project,
    ReviewVerdict, SearchGlobalPayload, SetTaskAutomationModePayload, SshConfig, SshConfigRecord,
    Subtask, Task, TaskAttachment, TaskAutomationState, TaskAutomationStateRecord,
    TaskExecutionChangeHistoryItem, TaskLatestReview, UpdateEmployee, UpdateProject,
    UpdateSshConfig, UpdateTask,
};
use crate::process_spawn::configure_std_command;

pub const DB_URL: &str = "sqlite:codex-ai.db";
pub(crate) const PROJECT_TYPE_LOCAL: &str = "local";
pub(crate) const PROJECT_TYPE_SSH: &str = "ssh";
pub(crate) const EXECUTION_TARGET_LOCAL: &str = "local";
pub(crate) const EXECUTION_TARGET_SSH: &str = "ssh";
pub(crate) const ARTIFACT_CAPTURE_MODE_LOCAL_FULL: &str = "local_full";
pub(crate) const ARTIFACT_CAPTURE_MODE_SSH_FULL: &str = "ssh_full";
pub(crate) const ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS: &str = "ssh_git_status";
pub(crate) const ARTIFACT_CAPTURE_MODE_SSH_NONE: &str = "ssh_none";
const DB_FILE_NAME: &str = "codex-ai.db";
const DB_AUTO_IMPORT_BACKUP_PREFIX: &str = "codex-ai.pre-import-backup";
const SQLITE_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
const REVIEW_DIFF_CHAR_LIMIT: usize = 120_000;
const FILE_CHANGE_DIFF_CHAR_LIMIT: usize = 120_000;
const REVIEW_UNTRACKED_FILE_LIMIT: usize = 5;
const REVIEW_UNTRACKED_FILE_SIZE_LIMIT: u64 = 16 * 1024;
const REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT: usize = 48_000;
const SDK_BRIDGE_FILE_NAME: &str = "sdk-bridge.mjs";
const SDK_RUNTIME_PACKAGE_JSON: &str =
    "{\"name\":\"codex-ai-sdk-runtime\",\"private\":true,\"type\":\"module\"}";
const REMOTE_TASK_ATTACHMENT_ROOT_DIR: &str = ".codex-ai/img";
pub(crate) const REVIEW_VERDICT_START_TAG: &str = "<review_verdict>";
pub(crate) const REVIEW_VERDICT_END_TAG: &str = "</review_verdict>";
const REVIEW_REPORT_START_TAG: &str = "<review_report>";
const REVIEW_REPORT_END_TAG: &str = "</review_report>";
const GLOBAL_SEARCH_MIN_QUERY_LENGTH: usize = 2;
const GLOBAL_SEARCH_DEFAULT_LIMIT: usize = 24;
const GLOBAL_SEARCH_MAX_LIMIT: usize = 50;
const GLOBAL_SEARCH_TYPE_PROJECT: &str = "project";
const GLOBAL_SEARCH_TYPE_TASK: &str = "task";
const GLOBAL_SEARCH_TYPE_EMPLOYEE: &str = "employee";
const GLOBAL_SEARCH_TYPE_SESSION: &str = "session";

struct DatabaseMigrationStatus {
    applied_count: i64,
    current_version: Option<i64>,
    current_description: Option<String>,
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

async fn fetch_database_migration_status(
    pool: &SqlitePool,
) -> Result<DatabaseMigrationStatus, String> {
    let (applied_count, current_version, current_description) =
        sqlx::query_as::<_, (i64, Option<i64>, Option<String>)>(
            r#"
            SELECT
                COUNT(*) AS applied_count,
                MAX(version) AS current_version,
                (
                    SELECT description
                    FROM _sqlx_migrations
                    WHERE success = 1
                    ORDER BY version DESC
                    LIMIT 1
                ) AS latest_description
            FROM _sqlx_migrations
            WHERE success = 1
            "#,
        )
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Failed to fetch migration status: {}", error))?;

    Ok(DatabaseMigrationStatus {
        applied_count,
        current_version,
        current_description,
    })
}

fn filesystem_safe_timestamp() -> String {
    Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

fn auto_import_backup_sql_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法解析应用配置目录: {}", error))?;

    Ok(dir.join(format!(
        "{}-{}.sql",
        DB_AUTO_IMPORT_BACKUP_PREFIX,
        filesystem_safe_timestamp()
    )))
}

fn resolve_user_file_path(path: &str) -> Result<PathBuf, String> {
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

fn resolve_existing_file_path(path: &str) -> Result<PathBuf, String> {
    let resolved = resolve_user_file_path(path)?;
    let canonical = resolved
        .canonicalize()
        .map_err(|error| format!("文件不存在或不可访问: {}", error))?;

    if !canonical.is_file() {
        return Err(format!("路径 {} 不是文件", canonical.display()));
    }

    Ok(canonical)
}

fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sql_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn ensure_statement_terminated(sql: &str) -> String {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        String::new()
    } else if trimmed.ends_with(';') {
        trimmed.to_string()
    } else {
        format!("{trimmed};")
    }
}

fn append_sql_statement(script: &mut String, sql: &str) {
    let statement = ensure_statement_terminated(sql);
    if !statement.is_empty() {
        script.push_str(&statement);
        script.push_str("\n\n");
    }
}

pub(crate) fn build_current_migrator() -> Migrator {
    let migrations = crate::db::migrations::get_all_migrations()
        .into_iter()
        .filter_map(|migration| match migration.kind {
            tauri_plugin_sql::MigrationKind::Up => Some(SqlxMigration::new(
                migration.version,
                Cow::Borrowed(migration.description),
                SqlxMigrationType::ReversibleUp,
                Cow::Borrowed(migration.sql),
                false,
            )),
            tauri_plugin_sql::MigrationKind::Down => None,
        })
        .collect::<Vec<_>>();

    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn fetch_schema_names(pool: &SqlitePool, object_type: &str) -> Result<Vec<String>, String> {
    let query = if object_type == "table" {
        "SELECT name FROM sqlite_master WHERE type = $1 AND name NOT LIKE 'sqlite_%' ORDER BY CASE WHEN name = '_sqlx_migrations' THEN 0 ELSE 1 END, name"
    } else {
        "SELECT name FROM sqlite_master WHERE type = $1 AND name NOT LIKE 'sqlite_%' ORDER BY name"
    };

    sqlx::query_scalar::<_, String>(query)
        .bind(object_type)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("读取数据库对象列表失败（{}）: {}", object_type, error))
}

async fn fetch_schema_sql(pool: &SqlitePool, object_type: &str) -> Result<Vec<String>, String> {
    let query = if object_type == "table" {
        "SELECT sql FROM sqlite_master WHERE type = $1 AND sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY CASE WHEN name = '_sqlx_migrations' THEN 0 ELSE 1 END, name"
    } else {
        "SELECT sql FROM sqlite_master WHERE type = $1 AND sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY name"
    };

    sqlx::query_scalar::<_, String>(query)
        .bind(object_type)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("读取数据库对象定义失败（{}）: {}", object_type, error))
}

async fn build_table_insert_statements(
    pool: &SqlitePool,
    table_name: &str,
) -> Result<Vec<String>, String> {
    let column_query = format!(
        "SELECT name FROM pragma_table_info({}) ORDER BY cid",
        sql_string_literal(table_name)
    );
    let columns = sqlx::query_scalar::<_, String>(&column_query)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("读取表 {} 的列信息失败: {}", table_name, error))?;

    if columns.is_empty() {
        return Ok(Vec::new());
    }

    let table_ident = sql_identifier(table_name);
    let column_list = columns
        .iter()
        .map(|column| sql_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let insert_prefix = format!("INSERT INTO {} ({}) VALUES (", table_ident, column_list);
    let values_expr = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let quoted_column = sql_identifier(column);
            if index == 0 {
                format!("quote({quoted_column})")
            } else {
                format!(" || ',' || quote({quoted_column})")
            }
        })
        .collect::<String>();
    let row_query = format!(
        "SELECT {} || {} || ');' FROM {}",
        sql_string_literal(&insert_prefix),
        values_expr,
        table_ident
    );

    sqlx::query_scalar::<_, String>(&row_query)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("导出表 {} 的数据失败: {}", table_name, error))
}

async fn build_sql_backup_script(pool: SqlitePool) -> Result<String, String> {
    let migration_status = fetch_database_migration_status(&pool).await.ok();
    let mut script = String::new();

    writeln!(&mut script, "-- Codex AI SQL backup").ok();
    writeln!(&mut script, "-- created_at: {}", now_sqlite()).ok();
    if let Some(version) = migration_status
        .as_ref()
        .and_then(|status| status.current_version)
    {
        writeln!(&mut script, "-- database_version: {}", version).ok();
    }
    script.push('\n');

    for sql in fetch_schema_sql(&pool, "table").await? {
        append_sql_statement(&mut script, &sql);
    }

    for table_name in fetch_schema_names(&pool, "table").await? {
        let row_statements = build_table_insert_statements(&pool, &table_name).await?;
        if !row_statements.is_empty() {
            for statement in row_statements {
                script.push_str(&statement);
                script.push('\n');
            }
            script.push('\n');
        }
    }

    for sql in fetch_schema_sql(&pool, "index").await? {
        append_sql_statement(&mut script, &sql);
    }

    for sql in fetch_schema_sql(&pool, "view").await? {
        append_sql_statement(&mut script, &sql);
    }

    for sql in fetch_schema_sql(&pool, "trigger").await? {
        append_sql_statement(&mut script, &sql);
    }

    Ok(script)
}

fn write_sql_backup_file(path: &Path, script: &str) -> Result<(), String> {
    if script.trim().is_empty() {
        return Err("SQL 备份内容为空，已中止写入".to_string());
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析目标目录: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("无法创建目录 {}: {}", parent.display(), error))?;
    fs::write(path, script)
        .map_err(|error| format!("写入 SQL 备份文件失败 {}: {}", path.display(), error))
}

fn sanitize_sql_backup_script(script: &str) -> String {
    let script = script.trim_start_matches('\u{feff}');
    let mut normalized = String::new();

    for line in script.lines() {
        let trimmed = line.trim();
        let upper = trimmed.trim_end_matches(';').trim().to_ascii_uppercase();
        let skip = matches!(
            upper.as_str(),
            "BEGIN TRANSACTION"
                | "BEGIN IMMEDIATE"
                | "BEGIN EXCLUSIVE"
                | "COMMIT"
                | "ROLLBACK"
                | "PRAGMA FOREIGN_KEYS=OFF"
                | "PRAGMA FOREIGN_KEYS = OFF"
                | "PRAGMA FOREIGN_KEYS=ON"
                | "PRAGMA FOREIGN_KEYS = ON"
        );

        if !skip {
            normalized.push_str(line);
            normalized.push('\n');
        }
    }

    normalized
}

async fn ensure_integrity_on_pool(pool: SqlitePool) -> Result<(), String> {
    let integrity_result = sqlx::query_scalar::<_, String>("PRAGMA integrity_check(1)")
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("数据库完整性校验失败: {}", error))?;

    if integrity_result.is_empty()
        || integrity_result
            .iter()
            .any(|item| !item.eq_ignore_ascii_case("ok"))
    {
        return Err(format!(
            "数据库完整性校验未通过: {}",
            integrity_result.join("; ")
        ));
    }

    let foreign_key_violations =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pragma_foreign_key_check")
            .fetch_one(&pool)
            .await
            .map_err(|error| format!("数据库外键校验失败: {}", error))?;

    if foreign_key_violations > 0 {
        return Err(format!(
            "数据库外键校验未通过，发现 {} 条约束问题",
            foreign_key_violations
        ));
    }

    Ok(())
}

async fn validate_sql_backup_script(
    script: String,
    latest_registered_version: i64,
) -> Result<(String, DatabaseMigrationStatus), String> {
    let sanitized = sanitize_sql_backup_script(&script);
    if sanitized.trim().is_empty() {
        return Err("SQL 备份文件为空或不包含可执行语句".to_string());
    }

    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .map_err(|error| format!("无法创建临时校验数据库: {}", error))?;

    sqlx::raw_sql(&sanitized)
        .execute(&pool)
        .await
        .map_err(|error| format!("SQL 备份文件执行失败: {}", error))?;

    ensure_integrity_on_pool(pool.clone()).await?;

    let migration_status = fetch_database_migration_status(&pool).await?;
    let source_version = migration_status
        .current_version
        .ok_or_else(|| "SQL 备份不包含已应用迁移记录，无法导入".to_string())?;

    if source_version > latest_registered_version {
        pool.close().await;
        return Err(format!(
            "SQL 备份版本 v{} 高于当前应用支持的最新版本 v{}，请先升级应用后再导入",
            source_version, latest_registered_version
        ));
    }

    let mut connection = pool
        .acquire()
        .await
        .map_err(|error| format!("无法获取临时校验数据库连接: {}", error))?;
    let migrator = build_current_migrator();
    migrator
        .run_direct(&mut *connection)
        .await
        .map_err(|error| format!("SQL 备份与当前应用迁移不兼容: {}", error))?;

    ensure_integrity_on_pool(pool.clone()).await?;

    let final_status = fetch_database_migration_status(&pool).await?;
    pool.close().await;

    Ok((sanitized, final_status))
}

async fn build_clear_database_script(pool: SqlitePool) -> Result<String, String> {
    let mut script = String::new();

    for trigger in fetch_schema_names(&pool, "trigger").await? {
        writeln!(
            &mut script,
            "DROP TRIGGER IF EXISTS {};",
            sql_identifier(&trigger)
        )
        .ok();
    }

    for view in fetch_schema_names(&pool, "view").await? {
        writeln!(
            &mut script,
            "DROP VIEW IF EXISTS {};",
            sql_identifier(&view)
        )
        .ok();
    }

    for table in fetch_schema_names(&pool, "table").await? {
        writeln!(
            &mut script,
            "DROP TABLE IF EXISTS {};",
            sql_identifier(&table)
        )
        .ok();
    }

    Ok(script)
}

async fn replace_database_from_sql(pool: SqlitePool, sanitized_sql: String) -> Result<(), String> {
    let clear_script = build_clear_database_script(pool.clone()).await?;
    let mut connection = pool
        .acquire()
        .await
        .map_err(|error| format!("无法获取数据库连接: {}", error))?;

    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .map_err(|error| format!("无法关闭外键检查: {}", error))?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await
        .map_err(|error| format!("无法开始 SQL 导入事务: {}", error))?;

    if !clear_script.trim().is_empty() {
        if let Err(error) = sqlx::raw_sql(&clear_script).execute(&mut *connection).await {
            let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
            let _ = sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&mut *connection)
                .await;
            return Err(format!("清空当前数据库失败: {}", error));
        }
    }

    if let Err(error) = sqlx::raw_sql(&sanitized_sql)
        .execute(&mut *connection)
        .await
    {
        let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
        let _ = sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await;
        return Err(format!("执行 SQL 导入失败: {}", error));
    }

    if let Err(error) = sqlx::query("COMMIT").execute(&mut *connection).await {
        let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
        let _ = sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await;
        return Err(format!("提交 SQL 导入事务失败: {}", error));
    }

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .map_err(|error| format!("无法恢复外键检查: {}", error))?;

    Ok(())
}

async fn run_current_migrations(pool: SqlitePool) -> Result<(), String> {
    let mut connection = pool
        .acquire()
        .await
        .map_err(|error| format!("无法获取迁移数据库连接: {}", error))?;
    let migrator = build_current_migrator();
    migrator
        .run_direct(&mut *connection)
        .await
        .map_err(|error| format!("补齐数据库迁移失败: {}", error))
}

pub(crate) async fn log_database_startup_status<R: Runtime>(app: &AppHandle<R>) {
    let path = database_path(app)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<unknown>".to_string());
    let latest_registered_version = crate::db::migrations::latest_migration_version();

    match sqlite_pool(app).await {
        Ok(pool) => {
            let migration_summary = fetch_database_migration_status(&pool).await;

            match migration_summary {
                Ok(DatabaseMigrationStatus {
                    applied_count,
                    current_version,
                    current_description,
                }) => {
                    let current_version = current_version.unwrap_or_default();
                    let pending_migrations =
                        latest_registered_version.saturating_sub(current_version);

                    println!("[db] SQLite 已加载: path={path}");
                    println!(
                        "[db] 迁移检查完成: applied_count={applied_count}, current_version={current_version}, latest_registered_version={latest_registered_version}, pending_migrations={pending_migrations}, latest_description={}",
                        current_description.as_deref().unwrap_or("none")
                    );
                }
                Err(error) => {
                    eprintln!("[db] SQLite 已加载，但读取迁移状态失败: path={path}, error={error}");
                }
            }
        }
        Err(error) => {
            eprintln!("[db] SQLite 未加载: path={path}, error={error}");
        }
    }
}

pub(crate) fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn new_id() -> String {
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

pub(crate) fn validate_remote_repo_path(
    remote_repo_path: Option<&str>,
) -> Result<Option<String>, String> {
    match normalize_optional_text(remote_repo_path) {
        Some(path) => Ok(Some(path)),
        None => Ok(None),
    }
}

async fn ensure_ssh_config_exists(pool: &SqlitePool, ssh_config_id: &str) -> Result<(), String> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ssh_configs WHERE id = $1")
        .bind(ssh_config_id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Failed to verify ssh config: {}", error))?;

    if exists > 0 {
        Ok(())
    } else {
        Err(format!("SSH 配置 {} 不存在", ssh_config_id))
    }
}

async fn validate_project_storage_fields(
    pool: &SqlitePool,
    project_type: &str,
    repo_path: Option<&str>,
    ssh_config_id: Option<&str>,
    remote_repo_path: Option<&str>,
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    match project_type {
        PROJECT_TYPE_LOCAL => Ok((validate_project_repo_path(repo_path)?, None, None)),
        PROJECT_TYPE_SSH => {
            let ssh_config_id = normalize_optional_text(ssh_config_id)
                .ok_or_else(|| "SSH 项目必须绑定 SSH 配置".to_string())?;
            ensure_ssh_config_exists(pool, &ssh_config_id).await?;
            let remote_repo_path = validate_remote_repo_path(remote_repo_path)?
                .ok_or_else(|| "SSH 项目必须提供远程仓库目录".to_string())?;
            Ok((None, Some(ssh_config_id), Some(remote_repo_path)))
        }
        other => Err(format!("不支持的项目类型: {other}")),
    }
}

pub(crate) fn normalize_ssh_auth_type(value: Option<&str>) -> Result<String, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("key") => Ok("key".to_string()),
        Some("password") => Ok("password".to_string()),
        Some(other) => Err(format!("不支持的 SSH 认证类型: {other}")),
    }
}

fn normalize_known_hosts_mode(value: Option<&str>) -> String {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("strict") => "strict".to_string(),
        Some("off") => "off".to_string(),
        _ => "accept-new".to_string(),
    }
}

pub(crate) fn ssh_config_target_host_label(config: &SshConfigRecord) -> String {
    format!("{}@{}:{}", config.username, config.host, config.port)
}

pub(crate) async fn fetch_ssh_config_record_by_id(
    pool: &SqlitePool,
    ssh_config_id: &str,
) -> Result<SshConfigRecord, String> {
    sqlx::query_as::<_, SshConfigRecord>("SELECT * FROM ssh_configs WHERE id = $1 LIMIT 1")
        .bind(ssh_config_id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("SSH 配置 {} 不存在: {}", ssh_config_id, error))
}

async fn fetch_ssh_config_by_id(
    pool: &SqlitePool,
    ssh_config_id: &str,
) -> Result<SshConfig, String> {
    Ok(fetch_ssh_config_record_by_id(pool, ssh_config_id)
        .await?
        .into())
}

async fn collect_all_ssh_secret_refs(pool: &SqlitePool) -> Result<HashSet<String>, String> {
    let rows = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT password_ref, passphrase_ref FROM ssh_configs",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to load ssh secret refs: {}", error))?;

    let mut refs = HashSet::new();
    for (password_ref, passphrase_ref) in rows {
        if let Some(password_ref) = password_ref {
            refs.insert(password_ref);
        }
        if let Some(passphrase_ref) = passphrase_ref {
            refs.insert(passphrase_ref);
        }
    }
    Ok(refs)
}

async fn sweep_ssh_secret_store<R: Runtime>(app: &AppHandle<R>) -> Result<usize, String> {
    let pool = sqlite_pool(app).await?;
    let active_refs = collect_all_ssh_secret_refs(&pool).await?;
    sweep_orphan_secret_refs(app, &active_refs)
}

fn redact_secret_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        "[REDACTED]".to_string()
    }
}

fn shell_escape_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn shell_escape_double_quoted(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`")
    )
}

pub(crate) fn remote_shell_path_expression(path: &str) -> String {
    let normalized = path.trim();
    if normalized.is_empty() {
        return "\"$HOME\"".to_string();
    }
    if matches!(normalized, "~" | "$HOME" | "${HOME}") {
        return "\"$HOME\"".to_string();
    }
    if let Some(rest) = normalized.strip_prefix("~/") {
        return format!(
            "\"$HOME/{}\"",
            shell_escape_double_quoted(rest).trim_matches('"')
        );
    }
    if let Some(rest) = normalized.strip_prefix("$HOME/") {
        return format!(
            "\"$HOME/{}\"",
            shell_escape_double_quoted(rest).trim_matches('"')
        );
    }
    if let Some(rest) = normalized.strip_prefix("${HOME}/") {
        return format!(
            "\"$HOME/{}\"",
            shell_escape_double_quoted(rest).trim_matches('"')
        );
    }
    shell_escape_double_quoted(normalized)
}

fn remote_node_bin_dir_expression(node_path_override: Option<&str>) -> Option<String> {
    let node_path = normalize_optional_text(node_path_override)?;
    let separator_index = node_path.rfind('/')?;
    if separator_index == 0 {
        Some("\"/\"".to_string())
    } else {
        Some(remote_shell_path_expression(&node_path[..separator_index]))
    }
}

fn remote_shell_bootstrap(node_path_override: Option<&str>) -> String {
    let mut statements = vec![
        "PATH=\"/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:$PATH\"".to_string(),
        "for dir in \"$HOME/.local/bin\" \"$HOME/bin\" \"$HOME/.npm-global/bin\" \"$HOME/.local/share/pnpm\" \"$HOME/Library/pnpm\" \"$HOME/.volta/bin\" \"$HOME/.yarn/bin\" \"$HOME/.bun/bin\" \"$HOME/.asdf/shims\"; do [ -d \"$dir\" ] && PATH=\"$dir:$PATH\"; done".to_string(),
        "if [ -s \"$HOME/.nvm/nvm.sh\" ]; then . \"$HOME/.nvm/nvm.sh\" >/dev/null 2>&1; nvm_default=$(nvm which default 2>/dev/null || true); if [ -n \"$nvm_default\" ] && [ -x \"$nvm_default\" ]; then PATH=\"$(dirname \"$nvm_default\"):$PATH\"; fi; fi".to_string(),
        "for dir in \"$HOME\"/.nvm/versions/node/*/bin; do [ -d \"$dir\" ] && PATH=\"$PATH:$dir\"; done".to_string(),
    ];

    if let Some(node_dir) = remote_node_bin_dir_expression(node_path_override) {
        statements.push(format!("PATH={node_dir}:$PATH"));
    }

    statements.push("export PATH".to_string());
    statements.push("hash -r 2>/dev/null || true".to_string());
    format!("{}; ", statements.join("; "))
}

pub(crate) fn build_remote_shell_command(script: &str, node_path_override: Option<&str>) -> String {
    format!(
        "sh -lc {}",
        shell_escape_single_quoted(&format!(
            "{}{}",
            remote_shell_bootstrap(node_path_override),
            script
        ))
    )
}

fn create_askpass_script(secret: &str) -> Result<PathBuf, String> {
    let base_dir = std::env::temp_dir().join("codex-ai-ssh-askpass");
    fs::create_dir_all(&base_dir).map_err(|error| format!("创建 askpass 目录失败: {error}"))?;
    let path = if cfg!(target_os = "windows") {
        base_dir.join(format!("askpass-{}.cmd", Uuid::new_v4()))
    } else {
        base_dir.join(format!("askpass-{}", Uuid::new_v4()))
    };
    let contents = if cfg!(target_os = "windows") {
        "@echo off\r\nsetlocal\r\n<nul set /p =%CODEX_SSH_SECRET%\r\n"
    } else {
        "#!/bin/sh\nprintf '%s' \"$CODEX_SSH_SECRET\"\n"
    };
    fs::write(&path, contents).map_err(|error| format!("写入 askpass 脚本失败: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&path, permissions)
            .map_err(|error| format!("设置 askpass 权限失败: {error}"))?;
    }
    let _ = secret;
    Ok(path)
}

pub(crate) async fn build_ssh_command<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    remote_command: Option<&str>,
    require_password_probe: bool,
    allocate_tty: bool,
) -> Result<(TokioCommand, Option<PathBuf>), String> {
    let mut command = new_ssh_command().await?;
    let mut askpass_path = None;

    command
        .arg("-p")
        .arg(ssh_config.port.to_string())
        .arg("-o")
        .arg("BatchMode=no")
        .arg("-o")
        .arg("ConnectTimeout=15");
    if allocate_tty {
        command.arg("-tt");
    }

    match ssh_config.known_hosts_mode.as_str() {
        "off" => {
            command.arg("-o").arg("StrictHostKeyChecking=no");
            command.arg("-o").arg("UserKnownHostsFile=/dev/null");
        }
        "strict" => {
            command.arg("-o").arg("StrictHostKeyChecking=yes");
        }
        _ => {
            command.arg("-o").arg("StrictHostKeyChecking=accept-new");
        }
    }

    if let Some(private_key_path) = ssh_config
        .private_key_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.arg("-i").arg(private_key_path);
    }

    if ssh_config.auth_type == "password" {
        if require_password_probe
            && !matches!(
                ssh_config.password_probe_status.as_deref(),
                Some("passed" | "available")
            )
        {
            return Err("当前 SSH 配置的密码认证尚未通过 probe，已阻止执行入口".to_string());
        }

        let secret = resolve_secret_value(app, ssh_config.password_ref.as_deref())?
            .ok_or_else(|| "当前 SSH 配置缺少可用密码引用".to_string())?;
        let askpass_script = create_askpass_script(&secret)?;
        command.env("DISPLAY", "codex-ai-ssh");
        command.env("SSH_ASKPASS_REQUIRE", "force");
        command.env("SSH_ASKPASS", &askpass_script);
        command.env("CODEX_SSH_SECRET", secret);
        askpass_path = Some(askpass_script);
    }

    command.arg(format!("{}@{}", ssh_config.username, ssh_config.host));
    if let Some(remote_command) = remote_command {
        command.arg(remote_command);
    }

    Ok((command, askpass_path))
}

pub(crate) async fn execute_ssh_command<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    remote_command: &str,
    require_password_probe: bool,
) -> Result<std::process::Output, String> {
    let (mut command, askpass_path) = build_ssh_command(
        app,
        ssh_config,
        Some(remote_command),
        require_password_probe,
        false,
    )
    .await?;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command
        .output()
        .await
        .map_err(|error| format!("执行远程 SSH 命令失败: {error}"))?;
    if let Some(path) = askpass_path {
        let _ = fs::remove_file(path);
    }
    Ok(output)
}

pub(crate) async fn execute_ssh_command_with_input<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    remote_command: &str,
    stdin_bytes: &[u8],
    require_password_probe: bool,
) -> Result<std::process::Output, String> {
    let (mut command, askpass_path) = build_ssh_command(
        app,
        ssh_config,
        Some(remote_command),
        require_password_probe,
        false,
    )
    .await?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("执行远程 SSH 命令失败: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(stdin_bytes)
            .await
            .map_err(|error| format!("写入远程 SSH 标准输入失败: {error}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|error| format!("关闭远程 SSH 标准输入失败: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("等待远程 SSH 命令完成失败: {error}"))?;
    if let Some(path) = askpass_path {
        let _ = fs::remove_file(path);
    }
    Ok(output)
}

fn remote_path_join(base: &str, leaf: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed.is_empty() {
        leaf.to_string()
    } else {
        format!("{trimmed}/{leaf}")
    }
}

pub(crate) fn remote_sdk_bridge_path(install_dir: &str) -> String {
    remote_path_join(install_dir, SDK_BRIDGE_FILE_NAME)
}

pub(crate) async fn ensure_remote_sdk_runtime_layout<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
) -> Result<crate::db::models::CodexSettings, String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let remote_settings = load_remote_codex_settings(app, ssh_config_id)?;
    let install_dir = remote_settings.sdk_install_dir.clone();
    let bridge_path = remote_sdk_bridge_path(&install_dir);
    let init_script = format!(
        "install_dir={}; mkdir -p \"$install_dir\"; if [ ! -f \"$install_dir/package.json\" ]; then printf '%s' {} > \"$install_dir/package.json\"; fi",
        remote_shell_path_expression(&install_dir),
        shell_escape_single_quoted(SDK_RUNTIME_PACKAGE_JSON),
    );
    let init_output = execute_ssh_command(
        app,
        &ssh_config,
        &build_remote_shell_command(&init_script, remote_settings.node_path_override.as_deref()),
        true,
    )
    .await?;
    if !init_output.status.success() {
        let stderr = String::from_utf8_lossy(&init_output.stderr)
            .trim()
            .to_string();
        return Err(if stderr.is_empty() {
            "初始化远程 SDK 运行目录失败".to_string()
        } else {
            format!(
                "初始化远程 SDK 运行目录失败：{}",
                redact_secret_text(&stderr)
            )
        });
    }

    let bridge_output = execute_ssh_command_with_input(
        app,
        &ssh_config,
        &build_remote_shell_command(
            &format!("cat > {}", remote_shell_path_expression(&bridge_path)),
            remote_settings.node_path_override.as_deref(),
        ),
        include_str!("codex/sdk_bridge.mjs").as_bytes(),
        true,
    )
    .await?;
    if !bridge_output.status.success() {
        let stderr = String::from_utf8_lossy(&bridge_output.stderr)
            .trim()
            .to_string();
        return Err(if stderr.is_empty() {
            "写入远程 SDK bridge 脚本失败".to_string()
        } else {
            format!(
                "写入远程 SDK bridge 脚本失败：{}",
                redact_secret_text(&stderr)
            )
        });
    }

    Ok(remote_settings)
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

fn truncate_review_text(value: &str, limit: usize) -> (String, bool) {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        return (trimmed.to_string(), false);
    }

    let truncated = trimmed.chars().take(limit).collect::<String>();
    (truncated, true)
}

fn is_supported_review_text_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some(
            "ts" | "tsx"
                | "js"
                | "jsx"
                | "json"
                | "rs"
                | "md"
                | "css"
                | "scss"
                | "html"
                | "yml"
                | "yaml"
                | "toml"
                | "sql"
                | "sh"
                | "txt"
        )
    )
}

fn run_git_text(repo_path: &str, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    configure_std_command(&mut command);
    let output = command
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map_err(|error| format!("执行 git {:?} 失败: {}", args, error))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn read_untracked_review_snippets(repo_path: &str, untracked_files: &[String]) -> String {
    let mut snippets = Vec::new();
    let mut consumed_chars = 0usize;

    for relative_path in untracked_files.iter().take(REVIEW_UNTRACKED_FILE_LIMIT) {
        if consumed_chars >= REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT {
            break;
        }

        let full_path = Path::new(repo_path).join(relative_path);
        let metadata = match fs::metadata(&full_path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        if !metadata.is_file()
            || metadata.len() > REVIEW_UNTRACKED_FILE_SIZE_LIMIT
            || !is_supported_review_text_extension(&full_path)
        {
            continue;
        }

        let content = match fs::read_to_string(&full_path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let remaining = REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT.saturating_sub(consumed_chars);
        if remaining == 0 {
            break;
        }

        let (snippet, truncated) = truncate_review_text(&content, remaining.min(12_000));
        if snippet.is_empty() {
            continue;
        }

        consumed_chars += snippet.chars().count();
        snippets.push(format!(
            "### {}\n```text\n{}\n```\n{}",
            relative_path,
            snippet,
            if truncated {
                "（内容已截断）"
            } else {
                ""
            }
        ));
    }

    if snippets.is_empty() {
        "（无可直接读取的未跟踪文本文件内容）".to_string()
    } else {
        snippets.join("\n\n")
    }
}

fn build_untracked_review_section(untracked_files: &[String], snippets: &str) -> String {
    if untracked_files.is_empty() {
        "（无未跟踪文件）".to_string()
    } else {
        format!(
            "未跟踪文件列表：\n{}\n\n未跟踪文本文件摘录：\n{}",
            untracked_files
                .iter()
                .map(|path| format!("- {}", path))
                .collect::<Vec<_>>()
                .join("\n"),
            snippets,
        )
    }
}

fn build_task_review_context_from_git_outputs(
    status_output: &str,
    unstaged_stat: &str,
    unstaged_diff: &str,
    staged_stat: &str,
    staged_diff: &str,
    untracked_files: &[String],
    untracked_section: &str,
) -> Result<String, String> {
    let status_trimmed = status_output.trim();
    if status_trimmed.is_empty() {
        return Err("当前工作区没有可审核的代码改动".to_string());
    }

    let combined_diff = [staged_diff.trim(), unstaged_diff.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if combined_diff.trim().is_empty() && untracked_files.is_empty() {
        return Err("当前工作区没有可审核的代码 diff".to_string());
    }

    let combined_stat = [staged_stat.trim(), unstaged_stat.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let (diff_body, diff_truncated) = truncate_review_text(&combined_diff, REVIEW_DIFF_CHAR_LIMIT);

    Ok(format!(
        "## Git 状态\n{}\n\n## Diff 概览\n{}\n\n## 完整 Diff\n{}\n{}\n\n## 未跟踪文件\n{}",
        status_trimmed,
        if combined_stat.trim().is_empty() {
            "（无 diff 统计）"
        } else {
            combined_stat.trim()
        },
        if diff_body.trim().is_empty() {
            "（无已跟踪文件 diff）"
        } else {
            diff_body.trim()
        },
        if diff_truncated {
            "\n（完整 diff 已截断）"
        } else {
            ""
        },
        untracked_section
    ))
}

pub(crate) fn collect_task_review_context(repo_path: &str) -> Result<String, String> {
    let status_output = run_git_text(repo_path, &["status", "--short"])?;
    let unstaged_stat = run_git_text(repo_path, &["diff", "--no-ext-diff", "--stat"])?;
    let unstaged_diff = run_git_text(repo_path, &["diff", "--no-ext-diff"])?;
    let staged_stat = run_git_text(repo_path, &["diff", "--no-ext-diff", "--stat", "--cached"])?;
    let staged_diff = run_git_text(repo_path, &["diff", "--no-ext-diff", "--cached"])?;
    let untracked_output =
        run_git_text(repo_path, &["ls-files", "--others", "--exclude-standard"])?;
    let untracked_files = untracked_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let untracked_section = build_untracked_review_section(
        &untracked_files,
        &read_untracked_review_snippets(repo_path, &untracked_files),
    );

    build_task_review_context_from_git_outputs(
        &status_output,
        &unstaged_stat,
        &unstaged_diff,
        &staged_stat,
        &staged_diff,
        &untracked_files,
        &untracked_section,
    )
}

fn shell_join_single_quoted(args: &[&str]) -> String {
    args.iter()
        .map(|arg| shell_escape_single_quoted(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

async fn run_remote_git_text<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    repo_path: &str,
    args: &[&str],
) -> Result<String, String> {
    let remote_command = build_remote_shell_command(
        &format!(
            "git -C {} {}",
            remote_shell_path_expression(repo_path),
            shell_join_single_quoted(args)
        ),
        None,
    );
    let output = execute_ssh_command(app, ssh_config, &remote_command, true).await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("远程执行 git {:?} 失败", args)
        } else {
            format!(
                "远程执行 git {:?} 失败: {}",
                args,
                redact_secret_text(&stderr)
            )
        })
    }
}

pub(crate) async fn collect_remote_task_review_context<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    repo_path: &str,
) -> Result<String, String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let status_output =
        run_remote_git_text(app, &ssh_config, repo_path, &["status", "--short"]).await?;
    let unstaged_stat = run_remote_git_text(
        app,
        &ssh_config,
        repo_path,
        &["diff", "--no-ext-diff", "--stat"],
    )
    .await?;
    let unstaged_diff =
        run_remote_git_text(app, &ssh_config, repo_path, &["diff", "--no-ext-diff"]).await?;
    let staged_stat = run_remote_git_text(
        app,
        &ssh_config,
        repo_path,
        &["diff", "--no-ext-diff", "--stat", "--cached"],
    )
    .await?;
    let staged_diff = run_remote_git_text(
        app,
        &ssh_config,
        repo_path,
        &["diff", "--no-ext-diff", "--cached"],
    )
    .await?;
    let untracked_output = run_remote_git_text(
        app,
        &ssh_config,
        repo_path,
        &["ls-files", "--others", "--exclude-standard"],
    )
    .await?;
    let untracked_files = untracked_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let untracked_section = build_untracked_review_section(
        &untracked_files,
        "（SSH 模式暂不采集远程未跟踪文件内容摘录，请结合未跟踪文件列表人工确认）",
    );

    build_task_review_context_from_git_outputs(
        &status_output,
        &unstaged_stat,
        &unstaged_diff,
        &staged_stat,
        &staged_diff,
        &untracked_files,
        &untracked_section,
    )
}

pub(crate) fn build_task_review_prompt(
    task: &Task,
    project: &Project,
    review_working_dir: &str,
    review_context: &str,
) -> String {
    format!(
        "你正在执行一次只读代码审查。\n\
要求：\n\
- 只允许阅读和分析代码，禁止修改任何文件，禁止执行 git commit/reset/checkout/merge/rebase 等写操作\n\
- 审核范围仅限下方提供的任务信息和当前工作区改动\n\
- 最终结构化判定必须且只能输出在 {verdict_start_tag} 和 {verdict_end_tag} 之间，内容必须是 JSON，对应字段：passed(boolean)、needs_human(boolean)、blocking_issue_count(number)、summary(string)\n\
- 最终人类可读报告必须且只能输出在 {start_tag} 和 {end_tag} 之间\n\
- 报告必须使用中文 Markdown，包含以下小节：## 结论、## 阻断问题、## 风险提醒、## 改进建议、## 验证缺口\n\
- 如果没有阻断问题，明确写“无阻断问题”\n\
- 如果 diff 信息被截断，要把这件事写进“验证缺口”\n\n\
任务标题：{title}\n\
任务状态：{status}\n\
任务优先级：{priority}\n\
项目名称：{project_name}\n\
仓库路径：{repo_path}\n\
执行目标：{execution_target}\n\
任务描述：{description}\n\n\
{review_context}",
        verdict_start_tag = REVIEW_VERDICT_START_TAG,
        verdict_end_tag = REVIEW_VERDICT_END_TAG,
        start_tag = REVIEW_REPORT_START_TAG,
        end_tag = REVIEW_REPORT_END_TAG,
        title = task.title.trim(),
        status = task.status.trim(),
        priority = task.priority.trim(),
        project_name = project.name.trim(),
        repo_path = review_working_dir,
        execution_target = if project.project_type == PROJECT_TYPE_SSH {
            "SSH 远程工作区"
        } else {
            "本地工作区"
        },
        description = task.description.as_deref().unwrap_or("（未填写）"),
        review_context = review_context,
    )
}

pub(crate) fn parse_review_verdict_json(value: &str) -> Result<ReviewVerdict, String> {
    let verdict = serde_json::from_str::<ReviewVerdict>(value)
        .map_err(|error| format!("Failed to parse review verdict JSON: {}", error))?;

    if verdict.summary.trim().is_empty() {
        return Err("Review verdict summary cannot be empty".to_string());
    }

    if verdict.blocking_issue_count < 0 {
        return Err("Review verdict blocking_issue_count cannot be negative".to_string());
    }

    Ok(verdict)
}

fn task_attachments_root_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .or_else(|_| app.path().app_config_dir())
        .map(|dir| dir.join("task-attachments"))
        .map_err(|error| format!("无法解析附件存储目录: {}", error))
}

fn task_attachment_dir<R: Runtime>(app: &AppHandle<R>, task_id: &str) -> Result<PathBuf, String> {
    Ok(task_attachments_root_dir(app)?.join(task_id))
}

fn task_attachment_mime_type(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn validate_task_attachment_source_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("附件路径不能为空".to_string());
    }

    let canonical = Path::new(trimmed)
        .canonicalize()
        .map_err(|error| format!("附件路径不存在或不可访问: {}", error))?;

    if !canonical.is_file() {
        return Err(format!("附件路径 {} 不是文件", canonical.display()));
    }

    if task_attachment_mime_type(&canonical).is_none() {
        return Err(format!(
            "附件 {} 不是支持的图片格式，仅支持 png/jpg/jpeg/gif/webp/bmp/svg",
            canonical.display()
        ));
    }

    Ok(canonical)
}

fn validate_managed_task_attachment_path<R: Runtime>(
    app: &AppHandle<R>,
    path: &str,
) -> Result<PathBuf, String> {
    let canonical = validate_task_attachment_source_path(path)?;
    let root = task_attachments_root_dir(app)?;
    let root = root.canonicalize().unwrap_or(root);

    if !canonical.starts_with(&root) {
        return Err(format!(
            "附件路径不在应用托管目录内: {}",
            canonical.display()
        ));
    }

    Ok(canonical)
}

fn cleanup_task_attachment_files(paths: &[String]) {
    for path in paths {
        let target = Path::new(path);
        if target.exists() {
            if let Err(error) = fs::remove_file(target) {
                eprintln!(
                    "[task-attachments] 清理附件文件失败: path={}, error={}",
                    target.display(),
                    error
                );
            }
        }
    }
}

fn cleanup_empty_attachment_dir<R: Runtime>(app: &AppHandle<R>, task_id: &str) {
    let Ok(dir) = task_attachment_dir(app, task_id) else {
        return;
    };

    let is_empty = fs::read_dir(&dir)
        .ok()
        .and_then(|mut entries| entries.next().transpose().ok())
        .flatten()
        .is_none();

    if is_empty {
        let _ = fs::remove_dir(&dir);
    }
}

fn build_task_attachment_from_source<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
    source_path: &str,
    sort_order: i32,
) -> Result<TaskAttachment, String> {
    let source = validate_task_attachment_source_path(source_path)?;
    let attachment_id = new_id();
    let original_name = source
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("无法解析附件文件名: {}", source.display()))?;
    let mime_type = task_attachment_mime_type(&source)
        .ok_or_else(|| format!("无法识别图片类型: {}", source.display()))?
        .to_string();
    let extension = source
        .extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase());
    let target_dir = task_attachment_dir(app, task_id)?;
    fs::create_dir_all(&target_dir).map_err(|error| format!("创建任务附件目录失败: {}", error))?;
    let target_path = match extension {
        Some(extension) if !extension.is_empty() => {
            target_dir.join(format!("{attachment_id}.{extension}"))
        }
        _ => target_dir.join(&attachment_id),
    };
    fs::copy(&source, &target_path).map_err(|error| {
        format!(
            "复制附件失败: {} -> {}: {}",
            source.display(),
            target_path.display(),
            error
        )
    })?;
    let file_size = fs::metadata(&target_path)
        .map_err(|error| format!("读取附件信息失败: {}", error))?
        .len() as i64;

    Ok(TaskAttachment {
        id: attachment_id,
        task_id: task_id.to_string(),
        original_name,
        stored_path: target_path.to_string_lossy().to_string(),
        mime_type,
        file_size,
        sort_order,
        created_at: now_sqlite(),
    })
}

fn build_task_attachments_from_sources<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
    source_paths: &[String],
    start_sort_order: i32,
) -> Result<Vec<TaskAttachment>, String> {
    let mut attachments = Vec::new();

    for (index, source_path) in source_paths.iter().enumerate() {
        match build_task_attachment_from_source(
            app,
            task_id,
            source_path,
            start_sort_order + index as i32,
        ) {
            Ok(attachment) => attachments.push(attachment),
            Err(error) => {
                let copied_paths = attachments
                    .iter()
                    .map(|attachment| attachment.stored_path.clone())
                    .collect::<Vec<_>>();
                cleanup_task_attachment_files(&copied_paths);
                cleanup_empty_attachment_dir(app, task_id);
                return Err(error);
            }
        }
    }

    Ok(attachments)
}

fn task_attachment_file_name(attachment: &TaskAttachment) -> Result<String, String> {
    Path::new(&attachment.stored_path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("无法解析附件文件名: {}", attachment.stored_path))
}

fn remote_task_attachment_dir(home_dir: &str, task_id: &str) -> String {
    remote_path_join(
        &remote_path_join(
            home_dir.trim_end_matches('/'),
            REMOTE_TASK_ATTACHMENT_ROOT_DIR,
        ),
        task_id,
    )
}

fn remote_task_attachment_path(
    home_dir: &str,
    attachment: &TaskAttachment,
) -> Result<String, String> {
    Ok(remote_path_join(
        &remote_task_attachment_dir(home_dir, &attachment.task_id),
        &task_attachment_file_name(attachment)?,
    ))
}

async fn resolve_remote_home_dir_with_config<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
) -> Result<String, String> {
    let output = execute_ssh_command(
        app,
        ssh_config,
        &build_remote_shell_command("printf '%s' \"$HOME\"", None),
        true,
    )
    .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "无法解析远程 HOME 目录".to_string()
        } else {
            format!("无法解析远程 HOME 目录：{}", redact_secret_text(&stderr))
        });
    }

    let home_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if home_dir.is_empty() {
        return Err("远程 HOME 目录为空".to_string());
    }

    Ok(home_dir)
}

async fn upload_task_attachment_to_remote<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    home_dir: &str,
    attachment: &TaskAttachment,
    skip_missing_local_source: bool,
) -> Result<Option<String>, String> {
    let source = match validate_managed_task_attachment_path(app, &attachment.stored_path) {
        Ok(source) => source,
        Err(error) if skip_missing_local_source => {
            let _ = error;
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let bytes = match fs::read(&source) {
        Ok(bytes) => bytes,
        Err(_) if skip_missing_local_source => return Ok(None),
        Err(error) => {
            return Err(format!("读取本地附件失败: {}: {}", source.display(), error));
        }
    };
    let remote_dir = remote_task_attachment_dir(home_dir, &attachment.task_id);
    let remote_path = remote_task_attachment_path(home_dir, attachment)?;
    let remote_command = build_remote_shell_command(
        &format!(
            "mkdir -p {} && cat > {}",
            remote_shell_path_expression(&remote_dir),
            remote_shell_path_expression(&remote_path),
        ),
        None,
    );
    let output =
        execute_ssh_command_with_input(app, ssh_config, &remote_command, &bytes, true).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("上传附件到远程失败：{}", attachment.original_name)
        } else {
            format!(
                "上传附件到远程失败：{}：{}",
                attachment.original_name,
                redact_secret_text(&stderr)
            )
        });
    }

    Ok(Some(remote_path))
}

async fn remove_remote_task_attachment_by_path<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    remote_path: &str,
) -> Result<(), String> {
    let output = execute_ssh_command(
        app,
        ssh_config,
        &build_remote_shell_command(
            &format!("rm -f {}", remote_shell_path_expression(remote_path)),
            None,
        ),
        true,
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("删除远程附件失败：{}", remote_path)
        } else {
            format!(
                "删除远程附件失败：{}：{}",
                remote_path,
                redact_secret_text(&stderr)
            )
        });
    }
    Ok(())
}

async fn sync_task_attachment_records_to_remote<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    attachments: &[TaskAttachment],
    skip_missing_local_source: bool,
) -> Result<RemoteTaskAttachmentSyncResult, String> {
    if attachments.is_empty() {
        return Ok(RemoteTaskAttachmentSyncResult {
            remote_paths: Vec::new(),
            skipped_local_paths: Vec::new(),
        });
    }

    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let home_dir = resolve_remote_home_dir_with_config(app, &ssh_config).await?;
    let mut remote_paths = Vec::with_capacity(attachments.len());
    let mut skipped_local_paths = Vec::new();

    for attachment in attachments {
        match upload_task_attachment_to_remote(
            app,
            &ssh_config,
            &home_dir,
            attachment,
            skip_missing_local_source,
        )
        .await
        {
            Ok(Some(remote_path)) => remote_paths.push(remote_path),
            Ok(None) => skipped_local_paths.push(attachment.stored_path.clone()),
            Err(error) => {
                for remote_path in &remote_paths {
                    if let Err(cleanup_error) =
                        remove_remote_task_attachment_by_path(app, &ssh_config, remote_path).await
                    {
                        eprintln!(
                            "[task-attachments] 清理远程附件失败: path={}, error={}",
                            remote_path, cleanup_error
                        );
                    }
                }
                return Err(error);
            }
        }
    }

    Ok(RemoteTaskAttachmentSyncResult {
        remote_paths,
        skipped_local_paths,
    })
}

pub(crate) async fn sync_task_attachments_to_remote<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    task_id: &str,
) -> Result<RemoteTaskAttachmentSyncResult, String> {
    let pool = sqlite_pool(app).await?;
    let attachments = fetch_task_attachments(&pool, task_id).await?;
    sync_task_attachment_records_to_remote(app, ssh_config_id, &attachments, true).await
}

async fn cleanup_remote_task_attachment_paths<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    remote_paths: &[String],
) {
    if remote_paths.is_empty() {
        return;
    }

    let pool = match sqlite_pool(app).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!(
                "[task-attachments] 获取数据库连接失败，无法清理远程附件: {}",
                error
            );
            return;
        }
    };
    let ssh_config = match fetch_ssh_config_record_by_id(&pool, ssh_config_id).await {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "[task-attachments] 读取 SSH 配置失败，无法清理远程附件: {}",
                error
            );
            return;
        }
    };

    for remote_path in remote_paths {
        if let Err(error) =
            remove_remote_task_attachment_by_path(app, &ssh_config, remote_path).await
        {
            eprintln!(
                "[task-attachments] 清理远程附件失败: path={}, error={}",
                remote_path, error
            );
        }
    }
}

async fn cleanup_remote_task_attachments_for_task<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    task_id: &str,
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let home_dir = resolve_remote_home_dir_with_config(app, &ssh_config).await?;
    let remote_dir = remote_task_attachment_dir(&home_dir, task_id);
    let output = execute_ssh_command(
        app,
        &ssh_config,
        &build_remote_shell_command(
            &format!("rm -rf {}", remote_shell_path_expression(&remote_dir)),
            None,
        ),
        true,
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("删除远程任务附件目录失败：{}", remote_dir)
        } else {
            format!(
                "删除远程任务附件目录失败：{}：{}",
                remote_dir,
                redact_secret_text(&stderr)
            )
        });
    }
    Ok(())
}

async fn cleanup_remote_task_attachment<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    attachment: &TaskAttachment,
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let home_dir = resolve_remote_home_dir_with_config(app, &ssh_config).await?;
    let remote_path = remote_task_attachment_path(&home_dir, attachment)?;
    remove_remote_task_attachment_by_path(app, &ssh_config, &remote_path).await
}

#[tauri::command]
pub async fn read_image_file(path: String) -> Result<Vec<u8>, String> {
    let source = validate_task_attachment_source_path(&path)?;
    fs::read(&source).map_err(|error| format!("读取图片文件失败: {}", error))
}

#[tauri::command]
pub async fn open_task_attachment<R: Runtime>(
    app: AppHandle<R>,
    path: String,
) -> Result<(), String> {
    let source = validate_managed_task_attachment_path(&app, &path)?;
    app.opener()
        .open_path(source.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| format!("打开附件失败: {}", error))
}

fn parse_sqlite_datetime(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, SQLITE_DATETIME_FORMAT).ok()
}

pub(crate) async fn insert_activity_log(
    pool: &SqlitePool,
    action: &str,
    details: &str,
    employee_id: Option<&str>,
    task_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO activity_logs (id, employee_id, action, details, task_id, project_id) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(new_id())
    .bind(employee_id)
    .bind(action)
    .bind(details)
    .bind(task_id)
    .bind(project_id)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to insert activity log: {}", error))?;

    Ok(())
}

pub(crate) async fn insert_codex_session_event_with_id(
    pool: &SqlitePool,
    session_id: &str,
    event_type: &str,
    message: Option<&str>,
) -> Result<String, String> {
    let event_id = new_id();

    sqlx::query(
        "INSERT INTO codex_session_events (id, session_id, event_type, message) VALUES ($1, $2, $3, $4)",
    )
    .bind(&event_id)
    .bind(session_id)
    .bind(event_type)
    .bind(message)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to insert session event: {}", error))?;

    Ok(event_id)
}

pub(crate) async fn insert_codex_session_event(
    pool: &SqlitePool,
    session_id: &str,
    event_type: &str,
    message: Option<&str>,
) -> Result<(), String> {
    insert_codex_session_event_with_id(pool, session_id, event_type, message)
        .await
        .map(|_| ())
}

fn emit_task_preflight_log<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    task_id: &str,
    session_kind: &str,
    line: impl Into<String>,
) {
    let _ = app.emit(
        "codex-stdout",
        CodexOutput {
            employee_id: employee_id.to_string(),
            task_id: Some(task_id.to_string()),
            session_kind: session_kind.to_string(),
            session_record_id: format!("preflight:{}:{}", session_kind, task_id),
            session_event_id: None,
            line: line.into(),
        },
    );
}

#[tauri::command]
pub async fn get_codex_session_log_lines<R: Runtime>(
    app: AppHandle<R>,
    session_id: String,
) -> Result<Vec<CodexSessionLogLine>, String> {
    let pool = sqlite_pool(&app).await?;
    let resolved_session_record_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM codex_sessions
        WHERE id = $1 OR cli_session_id = $1
        ORDER BY CASE WHEN id = $1 THEN 0 ELSE 1 END, started_at DESC
        LIMIT 1
        "#,
    )
    .bind(&session_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to resolve session log target: {}", error))?;

    let Some(resolved_session_record_id) = resolved_session_record_id else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        r#"
        WITH recent AS (
            SELECT rowid AS event_rowid, id, event_type, message
            FROM codex_session_events
            WHERE session_id = $1
              AND message IS NOT NULL
            ORDER BY event_rowid DESC
            LIMIT 2000
        )
        SELECT id, event_type, message
        FROM recent
        ORDER BY event_rowid ASC
        "#,
    )
    .bind(&resolved_session_record_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to fetch session log lines: {}", error))?;

    Ok(rows
        .into_iter()
        .filter_map(|(event_id, event_type, message)| {
            message.and_then(|value| {
                format_session_log_line(&event_type, &value)
                    .map(|line| CodexSessionLogLine { event_id, line })
            })
        })
        .collect())
}

pub(crate) async fn replace_codex_session_file_changes<R: Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
    changes: &[CodexSessionFileChangeInput],
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;

    sqlx::query("DELETE FROM codex_session_file_changes WHERE session_id = $1")
        .bind(session_id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to clear session file changes: {}", error))?;

    for change in changes {
        let change_id = new_id();
        sqlx::query(
            "INSERT INTO codex_session_file_changes (id, session_id, path, change_type, capture_mode, previous_path) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&change_id)
        .bind(session_id)
        .bind(&change.path)
        .bind(&change.change_type)
        .bind(&change.capture_mode)
        .bind(&change.previous_path)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to insert session file change: {}", error))?;

        if let Some(detail) = &change.detail {
            sqlx::query(
                "INSERT INTO codex_session_file_change_details (id, change_id, absolute_path, previous_absolute_path, before_status, before_text, before_truncated, after_status, after_text, after_truncated) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(new_id())
            .bind(&change_id)
            .bind(&detail.absolute_path)
            .bind(&detail.previous_absolute_path)
            .bind(&detail.before_status)
            .bind(&detail.before_text)
            .bind(if detail.before_truncated { 1 } else { 0 })
            .bind(&detail.after_status)
            .bind(&detail.after_text)
            .bind(if detail.after_truncated { 1 } else { 0 })
            .execute(&pool)
            .await
            .map_err(|error| format!("Failed to insert session file change detail: {}", error))?;
        }
    }

    Ok(())
}

pub(crate) async fn insert_codex_session_record<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: Option<&str>,
    task_id: Option<&str>,
    task_git_context_id: Option<&str>,
    working_dir: Option<&str>,
    resume_session_id: Option<&str>,
    session_kind: &str,
    status: &str,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    target_host_label: Option<&str>,
    artifact_capture_mode: &str,
) -> Result<CodexSessionRecord, String> {
    let pool = sqlite_pool(app).await?;
    let project_id = match task_id {
        Some(task_id) => sqlx::query_scalar::<_, Option<String>>(
            "SELECT project_id FROM tasks WHERE id = $1 LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(&pool)
        .await
        .map_err(|error| format!("Failed to resolve session project: {}", error))?
        .flatten(),
        None => None,
    };

    let record = CodexSessionRecord {
        id: new_id(),
        employee_id: employee_id.map(ToOwned::to_owned),
        task_id: task_id.map(ToOwned::to_owned),
        project_id,
        task_git_context_id: task_git_context_id.map(ToOwned::to_owned),
        cli_session_id: None,
        working_dir: working_dir.map(ToOwned::to_owned),
        execution_target: execution_target.to_string(),
        ssh_config_id: ssh_config_id.map(ToOwned::to_owned),
        target_host_label: target_host_label.map(ToOwned::to_owned),
        artifact_capture_mode: artifact_capture_mode.to_string(),
        session_kind: session_kind.to_string(),
        status: status.to_string(),
        started_at: now_sqlite(),
        ended_at: None,
        exit_code: None,
        resume_session_id: resume_session_id.map(ToOwned::to_owned),
        created_at: now_sqlite(),
    };

    sqlx::query(
        "INSERT INTO codex_sessions (id, employee_id, task_id, project_id, task_git_context_id, cli_session_id, working_dir, execution_target, ssh_config_id, target_host_label, artifact_capture_mode, session_kind, status, started_at, ended_at, exit_code, resume_session_id, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
    )
    .bind(&record.id)
    .bind(&record.employee_id)
    .bind(&record.task_id)
    .bind(&record.project_id)
    .bind(&record.task_git_context_id)
    .bind(&record.cli_session_id)
    .bind(&record.working_dir)
    .bind(&record.execution_target)
    .bind(&record.ssh_config_id)
    .bind(&record.target_host_label)
    .bind(&record.artifact_capture_mode)
    .bind(&record.session_kind)
    .bind(&record.status)
    .bind(&record.started_at)
    .bind(&record.ended_at)
    .bind(record.exit_code)
    .bind(&record.resume_session_id)
    .bind(&record.created_at)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to insert session record: {}", error))?;

    Ok(record)
}

pub(crate) async fn update_codex_session_record<R: Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
    status: Option<&str>,
    cli_session_id: Option<Option<&str>>,
    exit_code: Option<Option<i32>>,
    ended_at: Option<Option<&str>>,
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE codex_sessions SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(status) = status {
        separated.push("status = ").push_bind_unseparated(status);
        touched = true;
    }
    if let Some(cli_session_id) = cli_session_id {
        separated
            .push("cli_session_id = ")
            .push_bind_unseparated(cli_session_id.map(ToOwned::to_owned));
        touched = true;
    }
    if let Some(exit_code) = exit_code {
        separated
            .push("exit_code = ")
            .push_bind_unseparated(exit_code);
        touched = true;
    }
    if let Some(ended_at) = ended_at {
        separated
            .push("ended_at = ")
            .push_bind_unseparated(ended_at.map(ToOwned::to_owned));
        touched = true;
    }

    if !touched {
        return Ok(());
    }

    builder.push(" WHERE id = ").push_bind(session_id);
    builder
        .build()
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update session record: {}", error))?;

    Ok(())
}

pub(crate) async fn fetch_codex_session_by_id<R: Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
) -> Result<CodexSessionRecord, String> {
    let pool = sqlite_pool(app).await?;
    sqlx::query_as::<_, CodexSessionRecord>("SELECT * FROM codex_sessions WHERE id = $1 LIMIT 1")
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch session record: {}", error))
}

pub(crate) async fn fetch_project_by_id(pool: &SqlitePool, id: &str) -> Result<Project, String> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Project {} not found: {}", id, error))
}

pub(crate) async fn fetch_employee_by_id(pool: &SqlitePool, id: &str) -> Result<Employee, String> {
    sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Employee {} not found: {}", id, error))
}

pub(crate) async fn fetch_task_by_id(pool: &SqlitePool, id: &str) -> Result<Task, String> {
    sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Task {} not found: {}", id, error))
}

async fn fetch_task_attachment_by_id(
    pool: &SqlitePool,
    id: &str,
) -> Result<TaskAttachment, String> {
    sqlx::query_as::<_, TaskAttachment>("SELECT * FROM task_attachments WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Task attachment {} not found: {}", id, error))
}

pub(crate) async fn fetch_task_attachments(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<TaskAttachment>, String> {
    sqlx::query_as::<_, TaskAttachment>(
        "SELECT * FROM task_attachments WHERE task_id = $1 ORDER BY sort_order, created_at",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch task attachments: {}", error))
}

async fn ensure_project_exists(pool: &SqlitePool, project_id: &str) -> Result<(), String> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Failed to verify project: {}", error))
        .and_then(|count| {
            if count > 0 {
                Ok(())
            } else {
                Err(format!("Project {} does not exist", project_id))
            }
        })
}

async fn validate_assignee_for_project(
    pool: &SqlitePool,
    assignee_id: Option<&str>,
    _project_id: &str,
) -> Result<(), String> {
    let Some(assignee_id) = assignee_id else {
        return Ok(());
    };

    fetch_employee_by_id(pool, assignee_id).await?;
    Ok(())
}

pub(crate) async fn validate_reviewer_for_project(
    pool: &SqlitePool,
    reviewer_id: Option<&str>,
    _project_id: &str,
) -> Result<(), String> {
    let Some(reviewer_id) = reviewer_id else {
        return Ok(());
    };

    let reviewer = fetch_employee_by_id(pool, reviewer_id).await?;
    if reviewer.role != "reviewer" {
        return Err(format!("员工 {} 不是审查员角色", reviewer.name));
    }

    Ok(())
}

async fn insert_task_record(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    task: &Task,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO tasks (id, title, description, status, priority, project_id, assignee_id, reviewer_id, automation_mode, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(&task.id)
    .bind(&task.title)
    .bind(&task.description)
    .bind(&task.status)
    .bind(&task.priority)
    .bind(&task.project_id)
    .bind(&task.assignee_id)
    .bind(&task.reviewer_id)
    .bind(&task.automation_mode)
    .bind(&task.created_at)
    .bind(&task.updated_at)
    .execute(&mut **tx)
    .await
    .map_err(|error| format!("Failed to create task: {}", error))?;

    Ok(())
}

pub(crate) async fn fetch_task_automation_state_record(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Option<TaskAutomationStateRecord>, String> {
    sqlx::query_as::<_, TaskAutomationStateRecord>(
        "SELECT * FROM task_automation_state WHERE task_id = $1 LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch task automation state: {}", error))
}

pub(crate) fn decode_task_automation_state(
    record: TaskAutomationStateRecord,
) -> Result<TaskAutomationState, String> {
    let last_verdict = match record.last_verdict_json.as_deref() {
        Some(raw) => Some(parse_review_verdict_json(raw)?),
        None => None,
    };

    Ok(TaskAutomationState {
        task_id: record.task_id,
        phase: record.phase,
        round_count: record.round_count,
        consumed_session_id: record.consumed_session_id,
        last_trigger_session_id: record.last_trigger_session_id,
        pending_action: record.pending_action,
        pending_round_count: record.pending_round_count,
        last_error: record.last_error,
        last_verdict,
        updated_at: record.updated_at,
    })
}

async fn resolve_next_task_attachment_sort_order(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<i32, String> {
    let next = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM task_attachments WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to resolve attachment order: {}", error))?
    .flatten()
    .unwrap_or(1);

    Ok(next as i32)
}

async fn insert_task_attachments(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    attachments: &[TaskAttachment],
) -> Result<(), String> {
    for attachment in attachments {
        sqlx::query(
            "INSERT INTO task_attachments (id, task_id, original_name, stored_path, mime_type, file_size, sort_order, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&attachment.id)
        .bind(&attachment.task_id)
        .bind(&attachment.original_name)
        .bind(&attachment.stored_path)
        .bind(&attachment.mime_type)
        .bind(attachment.file_size)
        .bind(attachment.sort_order)
        .bind(&attachment.created_at)
        .execute(&mut **tx)
        .await
        .map_err(|error| format!("Failed to insert task attachment: {}", error))?;
    }

    Ok(())
}

pub(crate) async fn record_completion_metric(pool: &SqlitePool, task: &Task) -> Result<(), String> {
    let Some(employee_id) = task.assignee_id.as_deref() else {
        return Ok(());
    };

    let task_created_at = parse_sqlite_datetime(&task.created_at)
        .ok_or_else(|| format!("Invalid task created_at: {}", task.created_at))?;
    let now = Utc::now().naive_utc();
    let duration_secs = (now - task_created_at).num_seconds().max(0) as f64;

    let day_start = now
        .date()
        .and_hms_opt(0, 0, 0)
        .expect("valid day start")
        .format(SQLITE_DATETIME_FORMAT)
        .to_string();
    let day_end = (now + Duration::days(1))
        .date()
        .and_hms_opt(0, 0, 0)
        .expect("valid day end")
        .format(SQLITE_DATETIME_FORMAT)
        .to_string();

    let existing = sqlx::query_as::<_, EmployeeMetric>(
        "SELECT * FROM employee_metrics WHERE employee_id = $1 AND period_start = $2 AND period_end = $3 LIMIT 1",
    )
    .bind(employee_id)
    .bind(&day_start)
    .bind(&day_end)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch employee metrics: {}", error))?;

    if let Some(existing) = existing {
        let previous_count = existing.tasks_completed.max(0) as f64;
        let new_count = existing.tasks_completed + 1;
        let avg_completion_time = if previous_count == 0.0 {
            duration_secs
        } else {
            ((existing.average_completion_time.unwrap_or(duration_secs) * previous_count)
                + duration_secs)
                / (previous_count + 1.0)
        };
        let success_rate = if previous_count == 0.0 {
            100.0
        } else {
            ((existing.success_rate.unwrap_or(100.0) * previous_count) + 100.0)
                / (previous_count + 1.0)
        };

        sqlx::query(
            "UPDATE employee_metrics SET tasks_completed = $1, average_completion_time = $2, success_rate = $3 WHERE id = $4",
        )
        .bind(new_count)
        .bind(avg_completion_time)
        .bind(success_rate)
        .bind(existing.id)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to update employee metrics: {}", error))?;
    } else {
        sqlx::query(
            "INSERT INTO employee_metrics (id, employee_id, tasks_completed, average_completion_time, success_rate, period_start, period_end) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(new_id())
        .bind(employee_id)
        .bind(1_i64)
        .bind(duration_secs)
        .bind(100.0_f64)
        .bind(day_start)
        .bind(day_end)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to insert employee metrics: {}", error))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn health_check<R: Runtime>(app: AppHandle<R>) -> Result<CodexHealthCheck, String> {
    let latest_registered_version = crate::db::migrations::latest_migration_version();
    let pool = sqlite_pool(&app).await.ok();
    let database_loaded = pool.is_some();
    let migration_status = if let Some(pool) = pool.as_ref() {
        match fetch_database_migration_status(pool).await {
            Ok(status) => Some(status),
            Err(error) => {
                eprintln!("[db] health_check 读取迁移状态失败: {error}");
                None
            }
        }
    } else {
        None
    };
    let codex_settings = load_codex_settings(&app)?;
    let last_session_error = if let Some(pool) = pool.as_ref() {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT message FROM codex_session_events WHERE event_type IN ('validation_failed', 'spawn_failed', 'session_failed') ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .map_err(|error| format!("Failed to query last session error: {}", error))?
        .flatten()
    } else {
        None
    };

    let (codex_available, codex_version) = match new_codex_command().await {
        Ok(mut command) => match command
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
        {
            Ok(output) if output.status.success() => (
                true,
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string()),
            ),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                (false, (!stderr.is_empty()).then_some(stderr))
            }
            Err(error) => (
                false,
                Some(format!("Failed to run codex --version: {}", error)),
            ),
        },
        Err(error) => (false, Some(error)),
    };
    let sdk_health = inspect_sdk_runtime(&app, &codex_settings).await;

    Ok(CodexHealthCheck {
        execution_target: EXECUTION_TARGET_LOCAL.to_string(),
        ssh_config_id: None,
        target_host_label: None,
        codex_available,
        codex_version,
        node_available: sdk_health.node_available,
        node_version: sdk_health.node_version,
        task_sdk_enabled: codex_settings.task_sdk_enabled,
        one_shot_sdk_enabled: codex_settings.one_shot_sdk_enabled,
        sdk_installed: sdk_health.sdk_installed,
        sdk_version: sdk_health.sdk_version,
        sdk_install_dir: codex_settings.sdk_install_dir.clone(),
        task_execution_effective_provider: sdk_health.task_execution_effective_provider,
        one_shot_effective_provider: sdk_health.one_shot_effective_provider,
        sdk_status_message: sdk_health.status_message,
        database_loaded,
        database_path: database_path(&app).map(|path| path.to_string_lossy().to_string()),
        database_current_version: migration_status
            .as_ref()
            .and_then(|status| status.current_version),
        database_current_description: migration_status
            .as_ref()
            .and_then(|status| status.current_description.clone()),
        database_latest_version: latest_registered_version,
        shell_available: true,
        password_auth_available: false,
        password_probe_status: None,
        last_session_error,
        checked_at: now_sqlite(),
    })
}

#[tauri::command]
pub async fn backup_database<R: Runtime>(
    app: AppHandle<R>,
    destination_path: String,
) -> Result<DatabaseBackupResult, String> {
    let pool = sqlite_pool(&app).await?;
    let live_path = database_path(&app).ok_or_else(|| "无法解析数据库路径".to_string())?;
    let destination = resolve_user_file_path(&destination_path)?;

    if destination == live_path {
        return Err("备份目标不能与当前数据库文件相同".to_string());
    }

    let parent = destination
        .parent()
        .ok_or_else(|| format!("无法解析备份目录: {}", destination.display()))?;
    if !parent.exists() {
        return Err(format!("备份目录不存在: {}", parent.display()));
    }
    if destination.exists() && !destination.is_file() {
        return Err(format!("备份目标不是文件: {}", destination.display()));
    }
    if destination.exists() {
        fs::remove_file(&destination)
            .map_err(|error| format!("无法覆盖已有备份文件: {}", error))?;
    }

    let backup_script = build_sql_backup_script(pool.clone())
        .await
        .map_err(|error| format!("生成 SQL 备份失败: {}", error))?;
    write_sql_backup_file(&destination, &backup_script)
        .map_err(|error| format!("写入 SQL 备份失败: {}", error))?;

    let migration_status = fetch_database_migration_status(&pool).await.ok();
    let created_at = now_sqlite();

    Ok(DatabaseBackupResult {
        source_path: live_path.to_string_lossy().to_string(),
        destination_path: destination.to_string_lossy().to_string(),
        database_version: migration_status.and_then(|status| status.current_version),
        created_at: created_at.clone(),
        message: format!("SQL 备份已导出到 {}", destination.display()),
    })
}

#[tauri::command]
pub fn restore_database<R: Runtime>(
    app: AppHandle<R>,
    source_path: String,
) -> Result<DatabaseRestoreResult, String> {
    tauri::async_runtime::block_on(async move {
        let source = resolve_existing_file_path(&source_path)?;
        let source_sql = fs::read_to_string(&source)
            .map_err(|error| format!("读取 SQL 备份文件失败 {}: {}", source.display(), error))?;
        let latest_registered_version = crate::db::migrations::latest_migration_version();
        let (sanitized_sql, migration_status) =
            validate_sql_backup_script(source_sql, latest_registered_version).await?;
        let source_version = migration_status
            .current_version
            .ok_or_else(|| "SQL 备份不包含已应用迁移记录，无法导入".to_string())?;
        let pool = sqlite_pool(&app).await?;
        let current_backup_script = build_sql_backup_script(pool.clone())
            .await
            .map_err(|error| format!("生成导入前自动备份失败: {}", error))?;
        let backup_path = auto_import_backup_sql_path(&app)?;
        write_sql_backup_file(&backup_path, &current_backup_script)
            .map_err(|error| format!("写入导入前自动备份失败: {}", error))?;

        if let Err(error) = replace_database_from_sql(pool.clone(), sanitized_sql.clone()).await {
            return Err(format!("导入 SQL 失败，原数据库未改动。错误：{}", error));
        }

        if let Err(error) = run_current_migrations(pool.clone()).await {
            let restore_error = match replace_database_from_sql(
                pool.clone(),
                current_backup_script.clone(),
            )
            .await
            {
                Ok(()) => run_current_migrations(pool.clone()).await,
                Err(restore_error) => Err(restore_error),
            };

            return match restore_error {
                Ok(()) => Err(format!(
                    "SQL 导入后补齐迁移失败，已恢复导入前数据库。错误：{}",
                    error
                )),
                Err(recovery_error) => Err(format!(
                    "SQL 导入后补齐迁移失败，且恢复导入前数据库失败：{}。原始错误：{}。自动备份位于 {}",
                    recovery_error,
                    error,
                    backup_path.display()
                )),
            };
        }

        ensure_integrity_on_pool(pool.clone()).await?;
        let final_status = fetch_database_migration_status(&pool).await?;

        let restored_at = now_sqlite();
        Ok(DatabaseRestoreResult {
            source_path: source.to_string_lossy().to_string(),
            backup_path: backup_path.to_string_lossy().to_string(),
            database_version: final_status.current_version.or(Some(source_version)),
            restored_at,
            message: format!(
                "SQL 导入完成，当前数据库已更新到 v{}",
                final_status.current_version.unwrap_or(source_version)
            ),
        })
    })
}

#[tauri::command]
pub fn open_database_folder<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let database_file = database_path(&app).ok_or_else(|| "无法解析当前数据库路径".to_string())?;
    let directory = database_file
        .parent()
        .ok_or_else(|| format!("无法解析数据库所在目录: {}", database_file.display()))?;

    if !directory.exists() {
        return Err(format!("数据库目录不存在: {}", directory.display()));
    }

    if !directory.is_dir() {
        return Err(format!("数据库目录不是文件夹: {}", directory.display()));
    }

    app.opener()
        .open_path(directory.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| format!("打开数据库文件夹失败: {}", error))
}

#[tauri::command]
pub async fn get_codex_session_status<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    employee_id: String,
) -> Result<CodexRuntimeStatus, String> {
    let running_process = {
        let manager = state.lock().map_err(|error| error.to_string())?;
        manager.get_process(&employee_id)
    };

    let running_process = if let Some(process) = running_process {
        let process_state = {
            let mut child = process.child.lock().await;
            child.try_wait()
        };

        match process_state {
            Ok(None) => Some(process),
            Ok(Some(exit_code)) => {
                let current = fetch_codex_session_by_id(&app, &process.session_record_id)
                    .await
                    .ok();
                if let Some(current) = current {
                    if !matches!(current.status.as_str(), "exited" | "failed") {
                        let final_status = if current.status == "stopping" || exit_code == 0 {
                            "exited"
                        } else {
                            "failed"
                        };
                        let ended_at = now_sqlite();
                        let _ = update_codex_session_record(
                            &app,
                            &process.session_record_id,
                            Some(final_status),
                            None,
                            Some(Some(exit_code)),
                            Some(Some(ended_at.as_str())),
                        )
                        .await;
                    }
                }

                let mut manager = state.lock().map_err(|error| error.to_string())?;
                manager.remove_process(&employee_id);
                None
            }
            Err(error) => {
                let current = fetch_codex_session_by_id(&app, &process.session_record_id)
                    .await
                    .ok();
                if let Some(current) = current {
                    if !matches!(current.status.as_str(), "exited" | "failed") {
                        let ended_at = now_sqlite();
                        let _ = update_codex_session_record(
                            &app,
                            &process.session_record_id,
                            Some("failed"),
                            None,
                            Some(None),
                            Some(Some(ended_at.as_str())),
                        )
                        .await;
                        if let Ok(pool) = sqlite_pool(&app).await {
                            let _ = insert_codex_session_event(
                                &pool,
                                &process.session_record_id,
                                "session_failed",
                                Some(&format!("运行态检查失败: {}", error)),
                            )
                            .await;
                        }
                    }
                }

                let mut manager = state.lock().map_err(|error| error.to_string())?;
                manager.remove_process(&employee_id);
                None
            }
        }
    } else {
        None
    };

    let session = if let Some(process) = running_process.as_ref() {
        Some(fetch_codex_session_by_id(&app, &process.session_record_id).await?)
    } else {
        let pool = sqlite_pool(&app).await?;
        sqlx::query_as::<_, CodexSessionRecord>(
            "SELECT * FROM codex_sessions WHERE employee_id = $1 ORDER BY started_at DESC LIMIT 1",
        )
        .bind(&employee_id)
        .fetch_optional(&pool)
        .await
        .map_err(|error| format!("Failed to fetch runtime status: {}", error))?
    };

    Ok(CodexRuntimeStatus {
        running: running_process.is_some(),
        session,
    })
}

fn resolve_session_resume_state(
    cli_session_id: Option<&str>,
    employee_id: Option<&str>,
    employee_name: Option<&str>,
    status: &str,
    employee_is_running: bool,
) -> (String, Option<String>, bool) {
    if cli_session_id.is_none() {
        return (
            "missing_cli_session".to_string(),
            Some("该会话缺少可恢复的 CLI session id，只能查看，不能继续对话。".to_string()),
            false,
        );
    }

    if employee_id.is_none() || employee_name.is_none() {
        return (
            "missing_employee".to_string(),
            Some("该会话缺少有效的关联员工，暂时无法恢复。".to_string()),
            false,
        );
    }

    if status == "stopping" {
        return (
            "stopping".to_string(),
            Some("该会话正在停止，请稍后再试。".to_string()),
            false,
        );
    }

    if employee_is_running {
        return (
            "running".to_string(),
            Some("关联员工当前已有运行中的会话，请先停止后再继续对话。".to_string()),
            false,
        );
    }

    ("ready".to_string(), None, true)
}

fn format_session_log_line(event_type: &str, message: &str) -> Option<String> {
    let preserved = message.trim_end_matches(['\r', '\n']);
    if preserved.trim().is_empty() {
        return None;
    }

    match event_type {
        "stdout" => Some(preserved.to_string()),
        "stderr" => Some(if preserved.starts_with('[') {
            preserved.to_string()
        } else {
            format!("[ERROR] {}", preserved)
        }),
        "session_failed"
        | "spawn_failed"
        | "validation_failed"
        | "activity_log_failed"
        | "session_file_changes_failed" => Some(format!("[ERROR] {}", preserved.trim())),
        "session_exited" => Some(format!("[EXIT] {}", preserved.trim())),
        "review_report" => None,
        _ => Some(format!("[SYSTEM] {}", preserved.trim())),
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct TaskSearchRow {
    id: String,
    title: String,
    description: Option<String>,
    status: String,
    priority: String,
    project_id: String,
    project_name: String,
    updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct EmployeeSearchRow {
    id: String,
    name: String,
    role: String,
    specialization: Option<String>,
    status: String,
    project_id: Option<String>,
    project_name: Option<String>,
    updated_at: String,
}

fn normalize_search_query(value: &str) -> String {
    value.trim().to_lowercase()
}

fn normalize_global_search_types(raw_types: Option<Vec<String>>) -> HashSet<String> {
    let mut kinds = HashSet::new();

    if let Some(raw_types) = raw_types {
        for item in raw_types {
            match item.trim().to_lowercase().as_str() {
                GLOBAL_SEARCH_TYPE_PROJECT => {
                    kinds.insert(GLOBAL_SEARCH_TYPE_PROJECT.to_string());
                }
                GLOBAL_SEARCH_TYPE_TASK => {
                    kinds.insert(GLOBAL_SEARCH_TYPE_TASK.to_string());
                }
                GLOBAL_SEARCH_TYPE_EMPLOYEE => {
                    kinds.insert(GLOBAL_SEARCH_TYPE_EMPLOYEE.to_string());
                }
                GLOBAL_SEARCH_TYPE_SESSION => {
                    kinds.insert(GLOBAL_SEARCH_TYPE_SESSION.to_string());
                }
                _ => {}
            }
        }
    }

    if kinds.is_empty() {
        kinds.extend([
            GLOBAL_SEARCH_TYPE_PROJECT.to_string(),
            GLOBAL_SEARCH_TYPE_TASK.to_string(),
            GLOBAL_SEARCH_TYPE_EMPLOYEE.to_string(),
            GLOBAL_SEARCH_TYPE_SESSION.to_string(),
        ]);
    }

    kinds
}

fn text_match_score(
    normalized_query: &str,
    value: Option<&str>,
    exact_score: i64,
    prefix_score: i64,
    contains_score: i64,
) -> i64 {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return 0;
    };
    let normalized_value = value.to_lowercase();

    if normalized_value == normalized_query {
        exact_score
    } else if normalized_value.starts_with(normalized_query) {
        prefix_score
    } else if normalized_value.contains(normalized_query) {
        contains_score
    } else {
        0
    }
}

fn best_match_score(
    normalized_query: &str,
    fields: &[Option<&str>],
    exact_score: i64,
    prefix_score: i64,
    contains_score: i64,
) -> i64 {
    fields
        .iter()
        .map(|field| {
            text_match_score(
                normalized_query,
                *field,
                exact_score,
                prefix_score,
                contains_score,
            )
        })
        .max()
        .unwrap_or(0)
}

fn search_recency_bonus(value: Option<&str>) -> i64 {
    let Some(value) = value else {
        return 0;
    };
    let Some(updated_at) = parse_sqlite_datetime(value) else {
        return 0;
    };
    let age = Utc::now().naive_utc() - updated_at;

    if age <= Duration::days(3) {
        40
    } else if age <= Duration::days(14) {
        24
    } else if age <= Duration::days(30) {
        12
    } else {
        0
    }
}

fn compact_search_text(value: Option<&str>, max_chars: usize) -> Option<String> {
    let normalized = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\r', " ").replace('\n', " "))?;

    if normalized.chars().count() <= max_chars {
        return Some(normalized);
    }

    Some(
        normalized
            .chars()
            .take(max_chars)
            .collect::<String>()
            .trim_end()
            .to_string()
            + "…",
    )
}

fn search_status_label(status: &str) -> &str {
    match status {
        "todo" => "待办",
        "in_progress" => "进行中",
        "review" => "审核中",
        "completed" => "已完成",
        "blocked" => "已阻塞",
        "online" => "在线",
        "busy" => "忙碌",
        "offline" => "离线",
        "error" => "错误",
        "active" => "活跃",
        "archived" => "已归档",
        "pending" => "待启动",
        "running" => "运行中",
        "stopping" => "停止中",
        "exited" => "已结束",
        "failed" => "失败",
        _ => status,
    }
}

fn search_priority_label(priority: &str) -> &str {
    match priority {
        "low" => "低",
        "medium" => "中",
        "high" => "高",
        "urgent" => "紧急",
        _ => priority,
    }
}

fn search_employee_role_label(role: &str) -> &str {
    match role {
        "developer" => "开发者",
        "reviewer" => "审查员",
        "tester" => "测试员",
        "coordinator" => "协调员",
        _ => role,
    }
}

fn search_project_type_label(project_type: &str) -> &str {
    if project_type == PROJECT_TYPE_SSH {
        "SSH 项目"
    } else {
        "本地项目"
    }
}

fn search_session_kind_label(session_kind: &str) -> &str {
    if session_kind == "review" {
        "审核会话"
    } else {
        "执行会话"
    }
}

fn compare_global_search_items(
    left: &GlobalSearchItem,
    right: &GlobalSearchItem,
) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.title.cmp(&right.title))
}

fn build_project_search_item(project: Project, normalized_query: &str) -> Option<GlobalSearchItem> {
    let primary_score = best_match_score(
        normalized_query,
        &[Some(project.name.as_str())],
        1400,
        1100,
        860,
    );
    let secondary_score = best_match_score(
        normalized_query,
        &[
            project.description.as_deref(),
            project.repo_path.as_deref(),
            project.remote_repo_path.as_deref(),
        ],
        720,
        560,
        320,
    );
    let score = primary_score.max(secondary_score)
        + search_recency_bonus(Some(project.updated_at.as_str()));

    if score <= 0 {
        return None;
    }

    Some(GlobalSearchItem {
        item_type: GLOBAL_SEARCH_TYPE_PROJECT.to_string(),
        item_id: project.id.clone(),
        title: project.name.clone(),
        subtitle: Some(format!(
            "{} · {}",
            search_project_type_label(&project.project_type),
            search_status_label(&project.status)
        )),
        summary: compact_search_text(
            project
                .description
                .as_deref()
                .or(project.remote_repo_path.as_deref())
                .or(project.repo_path.as_deref()),
            96,
        ),
        navigation_path: format!("/projects/{}", project.id),
        score,
        updated_at: Some(project.updated_at.clone()),
        project_id: Some(project.id.clone()),
        task_id: None,
        employee_id: None,
        session_id: None,
    })
}

fn build_task_search_item(task: TaskSearchRow, normalized_query: &str) -> Option<GlobalSearchItem> {
    let primary_score = best_match_score(
        normalized_query,
        &[Some(task.title.as_str())],
        1450,
        1180,
        900,
    );
    let alias_score = best_match_score(
        normalized_query,
        &[
            task.description.as_deref(),
            Some(task.project_name.as_str()),
        ],
        760,
        580,
        340,
    );
    let score =
        primary_score.max(alias_score) + search_recency_bonus(Some(task.updated_at.as_str()));

    if score <= 0 {
        return None;
    }

    Some(GlobalSearchItem {
        item_type: GLOBAL_SEARCH_TYPE_TASK.to_string(),
        item_id: task.id.clone(),
        title: task.title.clone(),
        subtitle: Some(format!(
            "{} · {} · {}",
            task.project_name,
            search_status_label(&task.status),
            search_priority_label(&task.priority)
        )),
        summary: compact_search_text(task.description.as_deref(), 110),
        navigation_path: format!("/kanban?taskId={}", task.id),
        score,
        updated_at: Some(task.updated_at.clone()),
        project_id: Some(task.project_id.clone()),
        task_id: Some(task.id.clone()),
        employee_id: None,
        session_id: None,
    })
}

fn build_employee_search_item(
    employee: EmployeeSearchRow,
    normalized_query: &str,
) -> Option<GlobalSearchItem> {
    let primary_score = best_match_score(
        normalized_query,
        &[Some(employee.name.as_str())],
        1380,
        1080,
        820,
    );
    let alias_score = best_match_score(
        normalized_query,
        &[
            employee.specialization.as_deref(),
            Some(employee.role.as_str()),
            employee.project_name.as_deref(),
        ],
        700,
        520,
        300,
    );
    let score =
        primary_score.max(alias_score) + search_recency_bonus(Some(employee.updated_at.as_str()));

    if score <= 0 {
        return None;
    }

    let project_label = employee
        .project_name
        .clone()
        .unwrap_or_else(|| "未分配项目".to_string());

    Some(GlobalSearchItem {
        item_type: GLOBAL_SEARCH_TYPE_EMPLOYEE.to_string(),
        item_id: employee.id.clone(),
        title: employee.name.clone(),
        subtitle: Some(format!(
            "{} · {} · {}",
            search_employee_role_label(&employee.role),
            project_label,
            search_status_label(&employee.status)
        )),
        summary: compact_search_text(employee.specialization.as_deref(), 96),
        navigation_path: format!("/employees?employeeId={}", employee.id),
        score,
        updated_at: Some(employee.updated_at.clone()),
        project_id: employee.project_id.clone(),
        task_id: None,
        employee_id: Some(employee.id.clone()),
        session_id: None,
    })
}

fn build_session_search_item(
    session: CodexSessionListItem,
    normalized_query: &str,
) -> Option<GlobalSearchItem> {
    let primary_score = best_match_score(
        normalized_query,
        &[
            Some(session.display_name.as_str()),
            Some(session.session_id.as_str()),
            session.cli_session_id.as_deref(),
        ],
        1500,
        1200,
        960,
    );
    let secondary_score = best_match_score(
        normalized_query,
        &[
            session.summary.as_deref(),
            session.content_preview.as_deref(),
            session.task_title.as_deref(),
            session.project_name.as_deref(),
            session.employee_name.as_deref(),
            session.working_dir.as_deref(),
        ],
        760,
        600,
        360,
    );
    let score = primary_score.max(secondary_score)
        + search_recency_bonus(Some(session.last_updated_at.as_str()));

    if score <= 0 {
        return None;
    }

    Some(GlobalSearchItem {
        item_type: GLOBAL_SEARCH_TYPE_SESSION.to_string(),
        item_id: session.session_record_id.clone(),
        title: session.display_name.clone(),
        subtitle: Some(format!(
            "{} · {} · {}",
            search_session_kind_label(&session.session_kind),
            search_status_label(&session.status),
            session
                .project_name
                .clone()
                .unwrap_or_else(|| "无关联项目".to_string()),
        )),
        summary: compact_search_text(
            session
                .content_preview
                .as_deref()
                .or(session.summary.as_deref())
                .or(session.task_title.as_deref()),
            110,
        ),
        navigation_path: format!("/sessions?sessionId={}", session.session_id),
        score,
        updated_at: Some(session.last_updated_at.clone()),
        project_id: session.project_id.clone(),
        task_id: session.task_id.clone(),
        employee_id: session.employee_id.clone(),
        session_id: Some(session.session_id.clone()),
    })
}

#[tauri::command]
pub async fn search_global<R: Runtime>(
    app: AppHandle<R>,
    payload: SearchGlobalPayload,
) -> Result<GlobalSearchResponse, String> {
    let normalized_query = normalize_search_query(&payload.query);
    if normalized_query.is_empty() {
        return Ok(GlobalSearchResponse {
            query: payload.query,
            normalized_query,
            state: "empty_query".to_string(),
            message: Some("输入关键词后开始搜索。".to_string()),
            min_query_length: GLOBAL_SEARCH_MIN_QUERY_LENGTH,
            total: 0,
            items: Vec::new(),
        });
    }

    if normalized_query.chars().count() < GLOBAL_SEARCH_MIN_QUERY_LENGTH {
        return Ok(GlobalSearchResponse {
            query: payload.query,
            normalized_query,
            state: "query_too_short".to_string(),
            message: Some(format!(
                "至少输入 {} 个字符后再搜索。",
                GLOBAL_SEARCH_MIN_QUERY_LENGTH
            )),
            min_query_length: GLOBAL_SEARCH_MIN_QUERY_LENGTH,
            total: 0,
            items: Vec::new(),
        });
    }

    let selected_types = normalize_global_search_types(payload.types);
    let environment_mode = match payload.environment_mode.as_deref() {
        Some(EXECUTION_TARGET_SSH) => PROJECT_TYPE_SSH,
        _ => PROJECT_TYPE_LOCAL,
    };
    let limit = payload
        .limit
        .unwrap_or(GLOBAL_SEARCH_DEFAULT_LIMIT)
        .clamp(1, GLOBAL_SEARCH_MAX_LIMIT);
    let offset = payload.offset.unwrap_or(0);
    let pool = sqlite_pool(&app).await?;
    let mut items = Vec::new();

    if selected_types.contains(GLOBAL_SEARCH_TYPE_PROJECT) {
        let projects = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE project_type = $1 ORDER BY updated_at DESC, created_at DESC",
        )
        .bind(environment_mode)
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("Failed to fetch searchable projects: {}", error))?;

        items.extend(
            projects
                .into_iter()
                .filter_map(|project| build_project_search_item(project, &normalized_query)),
        );
    }

    if selected_types.contains(GLOBAL_SEARCH_TYPE_TASK) {
        let tasks = sqlx::query_as::<_, TaskSearchRow>(
            r#"
            SELECT
                t.id AS id,
                t.title AS title,
                t.description AS description,
                t.status AS status,
                t.priority AS priority,
                t.project_id AS project_id,
                p.name AS project_name,
                t.updated_at AS updated_at
            FROM tasks t
            INNER JOIN projects p ON p.id = t.project_id
            WHERE p.project_type = $1
            ORDER BY t.updated_at DESC, t.created_at DESC
            "#,
        )
        .bind(environment_mode)
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("Failed to fetch searchable tasks: {}", error))?;

        items.extend(
            tasks
                .into_iter()
                .filter_map(|task| build_task_search_item(task, &normalized_query)),
        );
    }

    if selected_types.contains(GLOBAL_SEARCH_TYPE_EMPLOYEE) {
        let employees = sqlx::query_as::<_, EmployeeSearchRow>(
            r#"
            SELECT
                e.id AS id,
                e.name AS name,
                e.role AS role,
                e.specialization AS specialization,
                e.status AS status,
                e.project_id AS project_id,
                p.name AS project_name,
                e.updated_at AS updated_at
            FROM employees e
            LEFT JOIN projects p ON p.id = e.project_id
            WHERE e.project_id IS NULL OR p.project_type = $1
            ORDER BY e.updated_at DESC, e.created_at DESC
            "#,
        )
        .bind(environment_mode)
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("Failed to fetch searchable employees: {}", error))?;

        items.extend(
            employees
                .into_iter()
                .filter_map(|employee| build_employee_search_item(employee, &normalized_query)),
        );
    }

    if selected_types.contains(GLOBAL_SEARCH_TYPE_SESSION) {
        let sessions = query_codex_session_list(&app).await?;
        items.extend(
            sessions
                .into_iter()
                .filter(|session| session.execution_target == environment_mode)
                .filter_map(|session| build_session_search_item(session, &normalized_query)),
        );
    }

    items.sort_by(compare_global_search_items);
    let total = items.len();
    let items = items.into_iter().skip(offset).take(limit).collect();

    Ok(GlobalSearchResponse {
        query: payload.query,
        normalized_query,
        state: "ok".to_string(),
        message: None,
        min_query_length: GLOBAL_SEARCH_MIN_QUERY_LENGTH,
        total,
        items,
    })
}

async fn query_codex_session_list<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<CodexSessionListItem>, String> {
    let pool = sqlite_pool(app).await?;
    sqlx::query_as::<_, CodexSessionListItem>(
        r#"
        SELECT
            s.id AS session_record_id,
            COALESCE(s.cli_session_id, s.id) AS session_id,
            s.cli_session_id AS cli_session_id,
            s.session_kind AS session_kind,
            s.status AS status,
            COALESCE(
                (
                    SELECT MAX(e.created_at)
                    FROM codex_session_events e
                    WHERE e.session_id = s.id
                ),
                s.ended_at,
                s.started_at,
                s.created_at
            ) AS last_updated_at,
            COALESCE(
                t.title,
                CASE
                    WHEN s.session_kind = 'review' THEN '代码审核会话'
                    ELSE 'Codex 执行会话'
                END
            ) AS display_name,
            CASE
                WHEN t.title IS NOT NULL AND p.name IS NOT NULL THEN p.name || ' · ' || t.title
                WHEN p.name IS NOT NULL THEN p.name
                WHEN s.working_dir IS NOT NULL THEN s.working_dir
                ELSE NULL
            END AS summary,
            SUBSTR(
                (
                    SELECT GROUP_CONCAT(message, ' ')
                    FROM (
                        SELECT TRIM(REPLACE(REPLACE(e.message, char(10), ' '), char(13), ' ')) AS message
                        FROM codex_session_events e
                        WHERE e.session_id = s.id
                          AND e.message IS NOT NULL
                          AND TRIM(e.message) <> ''
                        ORDER BY e.created_at DESC
                        LIMIT 5
                    )
                ),
                1,
                600
            ) AS content_preview,
            s.employee_id AS employee_id,
            e.name AS employee_name,
            s.task_id AS task_id,
            t.title AS task_title,
            t.status AS task_status,
            s.project_id AS project_id,
            p.name AS project_name,
            s.working_dir AS working_dir,
            s.execution_target AS execution_target,
            s.ssh_config_id AS ssh_config_id,
            s.target_host_label AS target_host_label,
            s.artifact_capture_mode AS artifact_capture_mode,
            '' AS resume_status,
            NULL AS resume_message,
            0 AS can_resume
        FROM codex_sessions s
        LEFT JOIN employees e ON e.id = s.employee_id
        LEFT JOIN tasks t ON t.id = s.task_id
        LEFT JOIN projects p ON p.id = s.project_id
        ORDER BY last_updated_at DESC, s.created_at DESC
        "#,
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to fetch codex sessions: {}", error))
}

#[tauri::command]
pub async fn list_codex_sessions<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
) -> Result<Vec<CodexSessionListItem>, String> {
    let mut items = query_codex_session_list(&app).await?;
    let running_by_employee = {
        let manager = state.lock().map_err(|error| error.to_string())?;
        let mut running = HashMap::new();
        for item in &items {
            if let Some(employee_id) = item.employee_id.as_ref() {
                running
                    .entry(employee_id.clone())
                    .or_insert_with(|| manager.is_running(employee_id));
            }
        }
        running
    };

    for item in &mut items {
        let employee_is_running = item
            .employee_id
            .as_ref()
            .and_then(|employee_id| running_by_employee.get(employee_id))
            .copied()
            .unwrap_or(false);
        let (resume_status, resume_message, can_resume) = resolve_session_resume_state(
            item.cli_session_id.as_deref(),
            item.employee_id.as_deref(),
            item.employee_name.as_deref(),
            &item.status,
            employee_is_running,
        );
        item.resume_status = resume_status;
        item.resume_message = resume_message;
        item.can_resume = can_resume;
    }
    Ok(items)
}

#[tauri::command]
pub async fn prepare_codex_session_resume<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    session_id: String,
) -> Result<CodexSessionResumePreview, String> {
    let mut items = query_codex_session_list(&app).await?;
    let item = items.drain(..).find(|current| {
        current.session_id == session_id || current.session_record_id == session_id
    });

    let Some(item) = item else {
        return Ok(CodexSessionResumePreview {
            requested_session_id: session_id,
            resolved_session_id: None,
            session_record_id: None,
            session_kind: None,
            session_status: None,
            display_name: None,
            summary: None,
            employee_id: None,
            employee_name: None,
            task_id: None,
            task_title: None,
            project_id: None,
            project_name: None,
            working_dir: None,
            execution_target: None,
            ssh_config_id: None,
            target_host_label: None,
            artifact_capture_mode: None,
            resume_status: "invalid".to_string(),
            resume_message: Some("无效 session id，未找到对应会话。".to_string()),
            can_resume: false,
        });
    };

    let employee_is_running = item
        .employee_id
        .as_ref()
        .map(|employee_id| {
            let manager = state.lock().map_err(|error| error.to_string())?;
            Ok::<bool, String>(manager.is_running(employee_id))
        })
        .transpose()?
        .unwrap_or(false);
    let (resume_status, resume_message, can_resume) = resolve_session_resume_state(
        item.cli_session_id.as_deref(),
        item.employee_id.as_deref(),
        item.employee_name.as_deref(),
        &item.status,
        employee_is_running,
    );

    Ok(CodexSessionResumePreview {
        requested_session_id: session_id,
        resolved_session_id: item.cli_session_id.clone(),
        session_record_id: Some(item.session_record_id),
        session_kind: Some(item.session_kind),
        session_status: Some(item.status),
        display_name: Some(item.display_name),
        summary: item.summary,
        employee_id: item.employee_id,
        employee_name: item.employee_name,
        task_id: item.task_id,
        task_title: item.task_title,
        project_id: item.project_id,
        project_name: item.project_name,
        working_dir: item.working_dir,
        execution_target: Some(item.execution_target),
        ssh_config_id: item.ssh_config_id,
        target_host_label: item.target_host_label,
        artifact_capture_mode: Some(item.artifact_capture_mode),
        resume_status,
        resume_message,
        can_resume,
    })
}

#[tauri::command]
pub async fn get_task_latest_review<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Option<TaskLatestReview>, String> {
    let pool = sqlite_pool(&app).await?;
    let session = sqlx::query_as::<_, CodexSessionRecord>(
        "SELECT * FROM codex_sessions WHERE task_id = $1 AND session_kind = 'review' ORDER BY started_at DESC LIMIT 1",
    )
    .bind(&task_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch task latest review session: {}", error))?;

    let Some(session) = session else {
        return Ok(None);
    };

    let report = sqlx::query_scalar::<_, Option<String>>(
        "SELECT message FROM codex_session_events WHERE session_id = $1 AND event_type = 'review_report' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&session.id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch task latest review report: {}", error))?
    .flatten();

    let reviewer_name = match session.employee_id.as_deref() {
        Some(employee_id) => sqlx::query_scalar::<_, Option<String>>(
            "SELECT name FROM employees WHERE id = $1 LIMIT 1",
        )
        .bind(employee_id)
        .fetch_optional(&pool)
        .await
        .map_err(|error| format!("Failed to fetch task reviewer name: {}", error))?
        .flatten(),
        None => None,
    };

    Ok(Some(TaskLatestReview {
        session,
        report,
        reviewer_name,
    }))
}

async fn resolve_execution_session_capture_mode(
    pool: &SqlitePool,
    session_id: &str,
    changes: &[CodexSessionFileChange],
) -> Result<String, String> {
    if let Some(change) = changes.first() {
        return Ok(change.capture_mode.clone());
    }

    let session_started_message = sqlx::query_scalar::<_, Option<String>>(
        "SELECT message FROM codex_session_events WHERE session_id = $1 AND event_type = 'session_started' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch session provider info: {}", error))?
    .flatten()
    .unwrap_or_default();

    if session_started_message.contains("通过 SDK 启动") {
        Ok("sdk_event".to_string())
    } else {
        Ok("git_fallback".to_string())
    }
}

async fn build_execution_change_history_item(
    pool: &SqlitePool,
    session: CodexSessionRecord,
) -> Result<TaskExecutionChangeHistoryItem, String> {
    let changes = sqlx::query_as::<_, CodexSessionFileChange>(
        "SELECT * FROM codex_session_file_changes WHERE session_id = $1 ORDER BY path ASC, created_at ASC",
    )
    .bind(&session.id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch task execution file changes: {}", error))?;

    let capture_mode = resolve_execution_session_capture_mode(pool, &session.id, &changes).await?;

    Ok(TaskExecutionChangeHistoryItem {
        session,
        capture_mode,
        changes,
    })
}

async fn fetch_execution_change_history_item_by_session_id(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<TaskExecutionChangeHistoryItem, String> {
    let session = sqlx::query_as::<_, CodexSessionRecord>(
        r#"
        SELECT *
        FROM codex_sessions
        WHERE id = $1 OR cli_session_id = $1
        ORDER BY CASE WHEN id = $1 THEN 0 ELSE 1 END, started_at DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch execution session: {}", error))?
    .ok_or_else(|| "找不到对应的 Session 记录".to_string())?;

    if session.session_kind != "execution" {
        return Err("只有 execution 会话支持查看改动文件".to_string());
    }

    build_execution_change_history_item(pool, session).await
}

#[tauri::command]
pub async fn get_task_execution_change_history<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Vec<TaskExecutionChangeHistoryItem>, String> {
    let pool = sqlite_pool(&app).await?;
    let sessions = sqlx::query_as::<_, CodexSessionRecord>(
        "SELECT * FROM codex_sessions WHERE task_id = $1 AND session_kind = 'execution' ORDER BY started_at DESC",
    )
    .bind(&task_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to fetch task execution sessions: {}", error))?;

    let mut items = Vec::with_capacity(sessions.len());
    for session in sessions {
        items.push(build_execution_change_history_item(&pool, session).await?);
    }

    Ok(items)
}

#[tauri::command]
pub async fn get_codex_session_execution_change_history<R: Runtime>(
    app: AppHandle<R>,
    session_id: String,
) -> Result<TaskExecutionChangeHistoryItem, String> {
    let pool = sqlite_pool(&app).await?;
    fetch_execution_change_history_item_by_session_id(&pool, &session_id).await
}

fn build_file_change_diff_preview(
    before_label: &str,
    before_text: Option<&str>,
    after_label: &str,
    after_text: Option<&str>,
) -> Result<(Option<String>, bool), String> {
    if before_text.is_none() && after_text.is_none() {
        return Ok((None, false));
    }

    let temp_dir = std::env::temp_dir().join(format!("codex-ai-diff-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).map_err(|error| format!("创建 diff 临时目录失败: {}", error))?;
    let before_file = temp_dir.join("before.txt");
    let after_file = temp_dir.join("after.txt");

    let write_result = (|| -> Result<(), String> {
        fs::write(&before_file, before_text.unwrap_or(""))
            .map_err(|error| format!("写入 diff 前镜像失败: {}", error))?;
        fs::write(&after_file, after_text.unwrap_or(""))
            .map_err(|error| format!("写入 diff 后镜像失败: {}", error))?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(error);
    }

    let mut command = Command::new("git");
    configure_std_command(&mut command);
    let output = command
        .current_dir(&temp_dir)
        .args([
            "diff",
            "--no-index",
            "--no-ext-diff",
            "--unified=3",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--",
            "before.txt",
            "after.txt",
        ])
        .output()
        .map_err(|error| format!("生成文件 diff 失败: {}", error))?;
    let _ = fs::remove_dir_all(&temp_dir);

    if !output.status.success() && output.status.code() != Some(1) {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let diff = rewrite_file_change_diff_labels(
        &String::from_utf8_lossy(&output.stdout),
        before_label,
        after_label,
    );
    let trimmed = diff.trim();
    if trimmed.is_empty() {
        return Ok((None, false));
    }

    let (diff_text, diff_truncated) = truncate_review_text(trimmed, FILE_CHANGE_DIFF_CHAR_LIMIT);
    Ok((Some(diff_text), diff_truncated))
}

fn file_change_diff_display_label(prefix: &str, label: &str) -> String {
    if label == "/dev/null" {
        label.to_string()
    } else {
        format!("{prefix}/{label}")
    }
}

fn rewrite_file_change_diff_labels(diff: &str, before_label: &str, after_label: &str) -> String {
    let before_display = file_change_diff_display_label("a", before_label);
    let after_display = file_change_diff_display_label("b", after_label);

    diff.lines()
        .map(|line| {
            if line == "diff --git a/before.txt b/after.txt" {
                format!("diff --git {} {}", before_display, after_display)
            } else if line == "--- a/before.txt" {
                format!("--- {}", before_display)
            } else if line == "+++ b/after.txt" {
                format!("+++ {}", after_display)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tauri::command]
pub async fn get_codex_session_file_change_detail<R: Runtime>(
    app: AppHandle<R>,
    change_id: String,
) -> Result<CodexSessionFileChangeDetail, String> {
    let pool = sqlite_pool(&app).await?;
    let change = sqlx::query_as::<_, CodexSessionFileChange>(
        "SELECT * FROM codex_session_file_changes WHERE id = $1",
    )
    .bind(&change_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch session file change: {}", error))?
    .ok_or_else(|| "找不到对应的文件变更记录".to_string())?;
    let session =
        sqlx::query_as::<_, CodexSessionRecord>("SELECT * FROM codex_sessions WHERE id = $1")
            .bind(&change.session_id)
            .fetch_optional(&pool)
            .await
            .map_err(|error| {
                format!(
                    "Failed to fetch session record for change detail: {}",
                    error
                )
            })?
            .ok_or_else(|| "找不到对应的执行会话".to_string())?;
    let detail = sqlx::query_as::<_, CodexSessionFileChangeDetailRecord>(
        "SELECT * FROM codex_session_file_change_details WHERE change_id = $1",
    )
    .bind(&change.id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch session file change detail: {}", error))?;

    let fallback_absolute_path = session
        .working_dir
        .as_ref()
        .map(|dir| path_to_runtime_string(&Path::new(dir).join(&change.path)));
    let fallback_previous_absolute_path = session
        .working_dir
        .as_ref()
        .zip(change.previous_path.as_ref())
        .map(|(dir, path)| path_to_runtime_string(&Path::new(dir).join(path)));

    let Some(detail) = detail else {
        return Ok(CodexSessionFileChangeDetail {
            change,
            working_dir: session.working_dir,
            absolute_path: fallback_absolute_path,
            previous_absolute_path: fallback_previous_absolute_path,
            before_status: "missing".to_string(),
            before_text: None,
            before_truncated: false,
            after_status: "missing".to_string(),
            after_text: None,
            after_truncated: false,
            diff_text: None,
            diff_truncated: false,
            snapshot_status: "unavailable".to_string(),
            snapshot_message: Some(
                "该执行记录生成于旧版本，只保留了文件级变更，没有保存可预览的文本快照。"
                    .to_string(),
            ),
        });
    };

    let before_label = if change.change_type == "added" {
        "/dev/null"
    } else {
        change
            .previous_path
            .as_deref()
            .unwrap_or(change.path.as_str())
    };
    let after_label = if change.change_type == "deleted" {
        "/dev/null"
    } else {
        change.path.as_str()
    };
    let can_build_diff = (detail.before_status == "text" && detail.before_text.is_some())
        || (detail.after_status == "text" && detail.after_text.is_some());
    let (diff_text, raw_diff_truncated) = if can_build_diff {
        build_file_change_diff_preview(
            before_label,
            detail.before_text.as_deref(),
            after_label,
            detail.after_text.as_deref(),
        )?
    } else {
        (None, false)
    };
    let diff_truncated =
        raw_diff_truncated || detail.before_truncated != 0 || detail.after_truncated != 0;

    Ok(CodexSessionFileChangeDetail {
        change,
        working_dir: session.working_dir,
        absolute_path: detail.absolute_path.or(fallback_absolute_path),
        previous_absolute_path: detail
            .previous_absolute_path
            .or(fallback_previous_absolute_path),
        before_status: detail.before_status,
        before_text: detail.before_text,
        before_truncated: detail.before_truncated != 0,
        after_status: detail.after_status,
        after_text: detail.after_text,
        after_truncated: detail.after_truncated != 0,
        diff_text,
        diff_truncated,
        snapshot_status: "ready".to_string(),
        snapshot_message: None,
    })
}

pub(crate) async fn start_task_code_review_internal(
    app: AppHandle,
    manager_state: Arc<Mutex<CodexManager>>,
    task_id: &str,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, task_id).await?;
    if task.status != "review" {
        return Err("只有“审核中”的任务才能发起代码审核".to_string());
    }

    let reviewer_id = task
        .reviewer_id
        .as_deref()
        .ok_or_else(|| "请先为任务指定审查员".to_string())?;
    let reviewer = fetch_employee_by_id(&pool, reviewer_id).await?;
    if reviewer.role != "reviewer" {
        return Err(format!("员工 {} 不是审查员角色", reviewer.name));
    }

    let project = fetch_project_by_id(&pool, &task.project_id).await?;
    let (review_working_dir, review_context) = if project.project_type == PROJECT_TYPE_SSH {
        let ssh_config_id = project
            .ssh_config_id
            .as_deref()
            .ok_or_else(|| "当前 SSH 项目未绑定 SSH 配置，无法审核代码".to_string())?;
        let remote_repo_path = project
            .remote_repo_path
            .as_deref()
            .ok_or_else(|| "当前 SSH 项目未配置远程仓库目录，无法审核代码".to_string())?;
        let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
        emit_task_preflight_log(
            &app,
            &reviewer.id,
            &task.id,
            "review",
            format!(
                "[SSH] 正在连接 {}（{}@{}:{}）...",
                ssh_config.name, ssh_config.username, ssh_config.host, ssh_config.port
            ),
        );
        emit_task_preflight_log(
            &app,
            &reviewer.id,
            &task.id,
            "review",
            format!(
                "[SSH] 正在准备远程审核上下文，仓库目录：{}",
                remote_repo_path
            ),
        );
        emit_task_preflight_log(
            &app,
            &reviewer.id,
            &task.id,
            "review",
            "[SSH] 正在通过 SSH 采集远程 git status / diff，用于生成审核上下文...".to_string(),
        );
        let review_context =
            collect_remote_task_review_context(&app, ssh_config_id, remote_repo_path).await?;
        emit_task_preflight_log(
            &app,
            &reviewer.id,
            &task.id,
            "review",
            "[SSH] 远程审核上下文采集完成，正在启动审核会话...".to_string(),
        );
        (remote_repo_path.to_string(), review_context)
    } else {
        let repo_path = project
            .repo_path
            .clone()
            .ok_or_else(|| "当前项目未配置仓库路径，无法审核代码".to_string())?;
        let review_context = collect_task_review_context(&repo_path)?;
        (repo_path, review_context)
    };
    let review_prompt =
        build_task_review_prompt(&task, &project, &review_working_dir, &review_context);

    crate::codex::start_codex_with_manager(
        app.clone(),
        manager_state,
        reviewer.id.clone(),
        review_prompt,
        Some(reviewer.model.clone()),
        Some(reviewer.reasoning_effort.clone()),
        reviewer.system_prompt.clone(),
        Some(review_working_dir),
        Some(task.id.clone()),
        None,
        None,
        None,
        Some("review".to_string()),
    )
    .await?;

    insert_activity_log(
        &pool,
        "task_review_requested",
        &format!("{} 发起代码审核", reviewer.name),
        Some(reviewer.id.as_str()),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn start_task_code_review(
    app: AppHandle,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    task_id: String,
) -> Result<(), String> {
    start_task_code_review_internal(app, state.inner().clone(), &task_id).await
}

#[tauri::command]
pub async fn set_task_automation_mode<R: Runtime>(
    app: AppHandle<R>,
    payload: SetTaskAutomationModePayload,
) -> Result<Task, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    let normalized_mode = payload
        .automation_mode
        .and_then(|value| value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(mode) = normalized_mode.as_deref() {
        if mode != "review_fix_loop_v1" {
            return Err(format!("不支持的自动质控模式: {}", mode));
        }
    }

    sqlx::query("UPDATE tasks SET automation_mode = $1 WHERE id = $2")
        .bind(&normalized_mode)
        .bind(&payload.task_id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update task automation mode: {}", error))?;

    if normalized_mode.is_some() {
        sqlx::query(
            r#"
            INSERT INTO task_automation_state (
                task_id,
                phase,
                round_count,
                consumed_session_id,
                last_trigger_session_id,
                pending_action,
                pending_round_count,
                last_error,
                last_verdict_json,
                updated_at
            ) VALUES ($1, 'idle', 0, NULL, NULL, NULL, NULL, NULL, NULL, $2)
            ON CONFLICT(task_id) DO UPDATE SET
                phase = 'idle',
                round_count = 0,
                consumed_session_id = NULL,
                last_trigger_session_id = NULL,
                pending_action = NULL,
                pending_round_count = NULL,
                last_error = NULL,
                last_verdict_json = NULL,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&payload.task_id)
        .bind(now_sqlite())
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to upsert task automation state: {}", error))?;
    } else {
        sqlx::query(
            r#"
            UPDATE task_automation_state
            SET pending_action = NULL,
                pending_round_count = NULL,
                last_verdict_json = NULL,
                phase = CASE
                    WHEN phase IN ('review_launch_failed', 'fix_launch_failed') THEN 'idle'
                    ELSE phase
                END,
                updated_at = $2
            WHERE task_id = $1
            "#,
        )
        .bind(&payload.task_id)
        .bind(now_sqlite())
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to clear pending automation state: {}", error))?;
    }

    insert_activity_log(
        &pool,
        if normalized_mode.is_some() {
            "task_automation_enabled"
        } else {
            "task_automation_disabled"
        },
        &task.title,
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    fetch_task_by_id(&pool, &payload.task_id).await
}

#[tauri::command]
pub async fn get_task_automation_state<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Option<TaskAutomationState>, String> {
    let pool = sqlite_pool(&app).await?;
    let Some(record) = fetch_task_automation_state_record(&pool, &task_id).await? else {
        return Ok(None);
    };

    Ok(Some(decode_task_automation_state(record)?))
}

#[tauri::command]
pub async fn create_project<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateProject,
) -> Result<Project, String> {
    let pool = sqlite_pool(&app).await?;
    let project_type = normalize_project_type(payload.project_type.as_deref())?;
    let (repo_path, ssh_config_id, remote_repo_path) = validate_project_storage_fields(
        &pool,
        &project_type,
        payload.repo_path.as_deref(),
        payload.ssh_config_id.as_deref(),
        payload.remote_repo_path.as_deref(),
    )
    .await?;
    let project = Project {
        id: new_id(),
        name: payload.name.trim().to_string(),
        description: normalize_optional_text(payload.description.as_deref()),
        status: "active".to_string(),
        repo_path,
        project_type,
        ssh_config_id,
        remote_repo_path,
        created_at: now_sqlite(),
        updated_at: now_sqlite(),
    };

    if project.name.is_empty() {
        return Err("项目名称不能为空".to_string());
    }

    sqlx::query(
        "INSERT INTO projects (id, name, description, status, repo_path, project_type, ssh_config_id, remote_repo_path, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(&project.id)
    .bind(&project.name)
    .bind(&project.description)
    .bind(&project.status)
    .bind(&project.repo_path)
    .bind(&project.project_type)
    .bind(&project.ssh_config_id)
    .bind(&project.remote_repo_path)
    .bind(&project.created_at)
    .bind(&project.updated_at)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to create project: {}", error))?;

    fetch_project_by_id(&pool, &project.id).await
}

#[tauri::command]
pub async fn update_project<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateProject,
) -> Result<Project, String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_project_by_id(&pool, &id).await?;
    let resolved_project_type = normalize_project_type(
        updates
            .project_type
            .as_deref()
            .or(Some(&current.project_type)),
    )?;
    let resolved_repo_path = match updates.repo_path.as_ref() {
        Some(Some(value)) => Some(value.as_str()),
        Some(None) => None,
        None => current.repo_path.as_deref(),
    };
    let resolved_ssh_config_id = match updates.ssh_config_id.as_ref() {
        Some(Some(value)) => Some(value.as_str()),
        Some(None) => None,
        None => current.ssh_config_id.as_deref(),
    };
    let resolved_remote_repo_path = match updates.remote_repo_path.as_ref() {
        Some(Some(value)) => Some(value.as_str()),
        Some(None) => None,
        None => current.remote_repo_path.as_deref(),
    };
    let (validated_repo_path, validated_ssh_config_id, validated_remote_repo_path) =
        validate_project_storage_fields(
            &pool,
            &resolved_project_type,
            resolved_repo_path,
            resolved_ssh_config_id,
            resolved_remote_repo_path,
        )
        .await?;
    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE projects SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(name) = updates.name {
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return Err("项目名称不能为空".to_string());
        }
        separated.push("name = ").push_bind_unseparated(trimmed);
        touched = true;
    }
    if let Some(description) = updates.description {
        separated.push("description = ").push_bind_unseparated(
            description.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(status) = updates.status {
        separated.push("status = ").push_bind_unseparated(status);
        touched = true;
    }
    if updates.project_type.is_some() {
        separated
            .push("project_type = ")
            .push_bind_unseparated(resolved_project_type.clone());
        touched = true;
    }
    if let Some(repo_path) = updates.repo_path {
        separated
            .push("repo_path = ")
            .push_bind_unseparated(match repo_path {
                Some(_) => validated_repo_path.clone(),
                None => None,
            });
        touched = true;
    }
    if updates.ssh_config_id.is_some() || updates.project_type.is_some() {
        separated
            .push("ssh_config_id = ")
            .push_bind_unseparated(validated_ssh_config_id.clone());
        touched = true;
    }
    if updates.remote_repo_path.is_some() || updates.project_type.is_some() {
        separated
            .push("remote_repo_path = ")
            .push_bind_unseparated(validated_remote_repo_path.clone());
        touched = true;
    }

    if !touched {
        return Ok(current);
    }

    builder.push(" WHERE id = ").push_bind(&id);
    builder
        .build()
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update project: {}", error))?;

    fetch_project_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn delete_project<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start project transaction: {}", error))?;

    sqlx::query(
        "DELETE FROM activity_logs WHERE project_id = $1 OR task_id IN (SELECT id FROM tasks WHERE project_id = $1)",
    )
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(|error| format!("Failed to delete project activity logs: {}", error))?;
    sqlx::query("UPDATE employees SET project_id = NULL WHERE project_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to clear employee project ownership: {}", error))?;
    sqlx::query("DELETE FROM tasks WHERE project_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete project tasks: {}", error))?;
    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete project: {}", error))?;

    tx.commit()
        .await
        .map_err(|error| format!("Failed to commit project delete: {}", error))?;

    Ok(())
}

#[tauri::command]
pub async fn list_ssh_configs<R: Runtime>(app: AppHandle<R>) -> Result<Vec<SshConfig>, String> {
    let pool = sqlite_pool(&app).await?;
    let records = sqlx::query_as::<_, SshConfigRecord>(
        "SELECT * FROM ssh_configs ORDER BY updated_at DESC, created_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to list ssh configs: {}", error))?;

    Ok(records.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn get_ssh_config<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<SshConfig, String> {
    let pool = sqlite_pool(&app).await?;
    fetch_ssh_config_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn create_ssh_config<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateSshConfig,
) -> Result<SshConfig, String> {
    let pool = sqlite_pool(&app).await?;
    let id = new_id();
    let name = payload.name.trim().to_string();
    let host = payload.host.trim().to_string();
    let username = payload.username.trim().to_string();
    let auth_type = normalize_ssh_auth_type(Some(&payload.auth_type))?;
    let private_key_path = normalize_optional_text(payload.private_key_path.as_deref());
    let known_hosts_mode = normalize_known_hosts_mode(payload.known_hosts_mode.as_deref());
    let port = payload.port.unwrap_or(22).clamp(1, 65535);

    if name.is_empty() || host.is_empty() || username.is_empty() {
        return Err("SSH 配置名称、主机和用户名不能为空".to_string());
    }

    if auth_type == "key" && private_key_path.is_none() {
        return Err("密钥认证必须提供 private_key_path".to_string());
    }

    let password_ref = store_secret_value(&app, payload.password.as_deref(), None)?;
    let passphrase_ref = store_secret_value(&app, payload.passphrase.as_deref(), None)?;

    let insert_result = sqlx::query(
        r#"
        INSERT INTO ssh_configs (
            id,
            name,
            host,
            port,
            username,
            auth_type,
            private_key_path,
            password_ref,
            passphrase_ref,
            known_hosts_mode,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(&id)
    .bind(&name)
    .bind(&host)
    .bind(port)
    .bind(&username)
    .bind(&auth_type)
    .bind(&private_key_path)
    .bind(&password_ref)
    .bind(&passphrase_ref)
    .bind(&known_hosts_mode)
    .bind(now_sqlite())
    .bind(now_sqlite())
    .execute(&pool)
    .await;

    if let Err(error) = insert_result {
        let _ = delete_secret_value(&app, password_ref.as_deref());
        let _ = delete_secret_value(&app, passphrase_ref.as_deref());
        return Err(format!("Failed to create ssh config: {}", error));
    }

    let _ = insert_activity_log(&pool, "ssh_config_created", &name, None, None, None).await;
    let _ = sweep_ssh_secret_store(&app).await;

    fetch_ssh_config_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn update_ssh_config<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateSshConfig,
) -> Result<SshConfig, String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_ssh_config_record_by_id(&pool, &id).await?;

    let host_changed = updates.host.is_some();
    let port_changed = updates.port.is_some();
    let username_changed = updates.username.is_some();

    let name = updates
        .name
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| current.name.clone());
    let host = updates
        .host
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| current.host.clone());
    let username = updates
        .username
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| current.username.clone());
    let auth_type =
        normalize_ssh_auth_type(updates.auth_type.as_deref().or(Some(&current.auth_type)))?;
    let private_key_path = match updates.private_key_path {
        Some(Some(value)) => normalize_optional_text(Some(&value)),
        Some(None) => None,
        None => current.private_key_path.clone(),
    };
    let known_hosts_mode = updates
        .known_hosts_mode
        .map(|value| normalize_known_hosts_mode(Some(&value)))
        .unwrap_or_else(|| current.known_hosts_mode.clone());
    let port = updates.port.unwrap_or(current.port).clamp(1, 65535);

    if name.is_empty() || host.is_empty() || username.is_empty() {
        return Err("SSH 配置名称、主机和用户名不能为空".to_string());
    }
    if auth_type == "key" && private_key_path.is_none() {
        return Err("密钥认证必须提供 private_key_path".to_string());
    }

    let mut created_password_ref: Option<String> = None;
    let mut created_passphrase_ref: Option<String> = None;

    let password_ref = if auth_type == "password" {
        match updates.password {
            Some(Some(ref value)) => {
                let next = store_secret_value(&app, Some(value), None)?;
                created_password_ref = next.clone();
                next
            }
            Some(None) => None,
            None => current.password_ref.clone(),
        }
    } else {
        None
    };
    let passphrase_ref = if auth_type == "key" {
        match updates.passphrase {
            Some(Some(ref value)) => {
                let next = store_secret_value(&app, Some(value), None)?;
                created_passphrase_ref = next.clone();
                next
            }
            Some(None) => None,
            None => current.passphrase_ref.clone(),
        }
    } else {
        None
    };

    let password_probe_needs_reset = auth_type != "password"
        || current.auth_type != "password"
        || updates.password.is_some()
        || host_changed
        || port_changed
        || username_changed;
    let password_probe_status = if auth_type == "password" && !password_probe_needs_reset {
        current.password_probe_status.clone()
    } else {
        None
    };
    let password_probe_checked_at = if auth_type == "password" && !password_probe_needs_reset {
        current.password_probe_checked_at.clone()
    } else {
        None
    };
    let password_probe_message = if auth_type == "password" && !password_probe_needs_reset {
        current.password_probe_message.clone()
    } else {
        None
    };

    let update_result = sqlx::query(
        r#"
        UPDATE ssh_configs
        SET name = $2,
            host = $3,
            port = $4,
            username = $5,
            auth_type = $6,
            private_key_path = $7,
            password_ref = $8,
            passphrase_ref = $9,
            known_hosts_mode = $10,
            password_probe_checked_at = $11,
            password_probe_status = $12,
            password_probe_message = $13,
            updated_at = $14
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .bind(&name)
    .bind(&host)
    .bind(port)
    .bind(&username)
    .bind(&auth_type)
    .bind(&private_key_path)
    .bind(&password_ref)
    .bind(&passphrase_ref)
    .bind(&known_hosts_mode)
    .bind(&password_probe_checked_at)
    .bind(&password_probe_status)
    .bind(&password_probe_message)
    .bind(now_sqlite())
    .execute(&pool)
    .await;

    if let Err(error) = update_result {
        if created_password_ref.is_some() {
            let _ = delete_secret_value(&app, password_ref.as_deref());
        }
        if created_passphrase_ref.is_some() {
            let _ = delete_secret_value(&app, passphrase_ref.as_deref());
        }
        return Err(format!("Failed to update ssh config: {}", error));
    }

    if current.password_ref != password_ref {
        delete_secret_value(&app, current.password_ref.as_deref())?;
    }
    if current.passphrase_ref != passphrase_ref {
        delete_secret_value(&app, current.passphrase_ref.as_deref())?;
    }

    let _ = insert_activity_log(&pool, "ssh_config_updated", &name, None, None, None).await;
    sweep_ssh_secret_store(&app).await?;

    fetch_ssh_config_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn delete_ssh_config<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_ssh_config_record_by_id(&pool, &id).await?;
    let usage_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE ssh_config_id = $1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .map_err(|error| format!("Failed to check ssh config usage: {}", error))?;
    if usage_count > 0 {
        return Err("当前 SSH 配置仍被项目引用，不能删除".to_string());
    }

    sqlx::query("DELETE FROM ssh_configs WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to delete ssh config: {}", error))?;

    delete_secret_value(&app, current.password_ref.as_deref())?;
    delete_secret_value(&app, current.passphrase_ref.as_deref())?;
    sweep_ssh_secret_store(&app).await?;
    let _ = insert_activity_log(&pool, "ssh_config_deleted", &current.name, None, None, None).await;

    Ok(())
}

#[tauri::command]
pub async fn probe_ssh_password_auth<R: Runtime>(
    app: AppHandle<R>,
    ssh_config_id: String,
) -> Result<PasswordAuthProbeResult, String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
    if current.auth_type != "password" {
        return Err("当前 SSH 配置不是密码认证，无需执行 password probe".to_string());
    }

    let checked_at = now_sqlite();
    let remote_command = format!(
        "sh -lc {}",
        shell_escape_single_quoted("printf 'codex-ai-password-probe' >/dev/null")
    );
    let output = execute_ssh_command(&app, &current, &remote_command, false).await;
    let (supported, status, message) = match output {
        Ok(output) if output.status.success() => (
            true,
            "passed".to_string(),
            "密码认证 probe 通过".to_string(),
        ),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = if stderr.is_empty() {
                "密码认证 probe 失败".to_string()
            } else {
                format!("密码认证 probe 失败：{}", redact_secret_text(&stderr))
            };
            (false, "failed".to_string(), message)
        }
        Err(error) => (false, "failed".to_string(), error),
    };

    sqlx::query(
        r#"
        UPDATE ssh_configs
        SET password_probe_checked_at = $2,
            password_probe_status = $3,
            password_probe_message = $4,
            updated_at = $5
        WHERE id = $1
        "#,
    )
    .bind(&ssh_config_id)
    .bind(&checked_at)
    .bind(&status)
    .bind(&message)
    .bind(now_sqlite())
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to persist password probe: {}", error))?;

    Ok(PasswordAuthProbeResult {
        ssh_config_id,
        target_host_label: ssh_config_target_host_label(&current),
        supported,
        status,
        message,
        checked_at,
    })
}

fn parse_remote_key_value_output(raw: &str) -> HashMap<String, String> {
    raw.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

fn build_remote_codex_runtime_health(
    remote_settings: &CodexSettings,
    values: &HashMap<String, String>,
    redacted_stderr: &str,
) -> RemoteCodexRuntimeHealth {
    let codex_available = values
        .get("CODEX_STATUS")
        .map(|value| value == "0")
        .unwrap_or(false);
    let node_available = values
        .get("NODE_STATUS")
        .map(|value| value == "0")
        .unwrap_or(false);
    let sdk_installed = values
        .get("SDK_INSTALLED")
        .map(|value| value == "1")
        .unwrap_or(false);
    let codex_version = values
        .get("CODEX_VERSION")
        .cloned()
        .filter(|value| !value.is_empty());
    let node_version = values
        .get("NODE_VERSION")
        .cloned()
        .filter(|value| !value.is_empty());
    let sdk_version = values
        .get("SDK_VERSION")
        .cloned()
        .filter(|value| !value.is_empty());
    let node_support_error = if node_available {
        match node_version.as_deref() {
            Some(version) => ensure_supported_node_version(version).err(),
            None => Some("无法解析远程 Node 版本".to_string()),
        }
    } else {
        None
    };
    let node_ready_for_sdk = node_available && node_support_error.is_none();
    let task_execution_effective_provider = determine_effective_provider(
        remote_settings.task_sdk_enabled,
        node_ready_for_sdk,
        sdk_installed,
    )
    .to_string();
    let one_shot_effective_provider = determine_effective_provider(
        remote_settings.one_shot_sdk_enabled,
        node_ready_for_sdk,
        sdk_installed,
    )
    .to_string();
    let status_message = if !redacted_stderr.is_empty() {
        redacted_stderr.to_string()
    } else if !remote_settings.task_sdk_enabled && !remote_settings.one_shot_sdk_enabled {
        "远程 Codex SDK 未启用，任务运行与一次性 AI 将使用远程 codex exec。".to_string()
    } else if !node_available {
        "远程 Node 不可用，已回退到远程 codex exec。".to_string()
    } else if let Some(error) = node_support_error {
        format!("{error}，已回退到远程 codex exec。")
    } else if !sdk_installed {
        "远程 Codex SDK 未安装，已回退到远程 codex exec。".to_string()
    } else if task_execution_effective_provider == "sdk" || one_shot_effective_provider == "sdk" {
        "远程 Codex SDK 已就绪，任务运行与一次性 AI 将优先使用远程 SDK，失败时自动回退到远程 codex exec。"
            .to_string()
    } else if codex_available {
        "远程 Codex 健康检查完成；当前将使用远程 codex exec。".to_string()
    } else {
        "远程 Codex 不可用".to_string()
    };

    RemoteCodexRuntimeHealth {
        codex_available,
        codex_version,
        node_available,
        node_version,
        sdk_installed,
        sdk_version,
        task_execution_effective_provider,
        one_shot_effective_provider,
        status_message,
    }
}

pub(crate) async fn inspect_remote_codex_runtime<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    remote_settings: &CodexSettings,
) -> Result<RemoteCodexRuntimeHealth, String> {
    let sdk_install_dir = remote_shell_path_expression(&remote_settings.sdk_install_dir);
    let remote_script = format!(
        "sdk_install_dir={sdk_install_dir}; \
codex_output=$(codex --version 2>/dev/null); codex_status=$?; \
node_output=$(node --version 2>/dev/null); node_status=$?; \
sdk_pkg=\"$sdk_install_dir\"/node_modules/@openai/codex-sdk/package.json; \
sdk_cli_pkg=\"$sdk_install_dir\"/node_modules/@openai/codex/package.json; \
if [ -f \"$sdk_pkg\" ] && [ -f \"$sdk_cli_pkg\" ]; then \
  sdk_installed=1; \
  sdk_version=$(sed -n 's/.*\"version\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p' \"$sdk_pkg\" | head -n 1); \
else \
  sdk_installed=0; sdk_version=''; \
fi; \
printf 'CODEX_STATUS=%s\\nCODEX_VERSION=%s\\nNODE_STATUS=%s\\nNODE_VERSION=%s\\nSDK_INSTALLED=%s\\nSDK_VERSION=%s\\n' \"$codex_status\" \"$codex_output\" \"$node_status\" \"$node_output\" \"$sdk_installed\" \"$sdk_version\""
    );
    let output = execute_ssh_command(
        app,
        ssh_config,
        &build_remote_shell_command(
            &remote_script,
            remote_settings.node_path_override.as_deref(),
        ),
        true,
    )
    .await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let redacted_stderr = redact_secret_text(&stderr);
    let values = parse_remote_key_value_output(&stdout);

    Ok(build_remote_codex_runtime_health(
        remote_settings,
        &values,
        &redacted_stderr,
    ))
}

#[tauri::command]
pub async fn validate_remote_codex_health<R: Runtime>(
    app: AppHandle<R>,
    ssh_config_id: String,
) -> Result<CodexHealthCheck, String> {
    let pool = sqlite_pool(&app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
    let remote_settings = load_remote_codex_settings(&app, &ssh_config_id)?;
    let runtime = inspect_remote_codex_runtime(&app, &ssh_config, &remote_settings).await?;
    let checked_at = now_sqlite();
    sqlx::query(
        "UPDATE ssh_configs SET last_checked_at = $2, last_check_status = $3, last_check_message = $4, updated_at = $5 WHERE id = $1",
    )
    .bind(&ssh_config_id)
    .bind(&checked_at)
    .bind(if runtime.codex_available { "passed" } else { "failed" })
    .bind(&runtime.status_message)
    .bind(now_sqlite())
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to persist remote health check: {}", error))?;
    let _ = insert_activity_log(
        &pool,
        "remote_codex_validated",
        &ssh_config_target_host_label(&ssh_config),
        None,
        None,
        None,
    )
    .await;

    let latest_registered_version = crate::db::migrations::latest_migration_version();
    let migration_status = fetch_database_migration_status(&pool).await.ok();
    Ok(CodexHealthCheck {
        execution_target: EXECUTION_TARGET_SSH.to_string(),
        ssh_config_id: Some(ssh_config_id),
        target_host_label: Some(ssh_config_target_host_label(&ssh_config)),
        codex_available: runtime.codex_available,
        codex_version: runtime.codex_version,
        node_available: runtime.node_available,
        node_version: runtime.node_version,
        task_sdk_enabled: remote_settings.task_sdk_enabled,
        one_shot_sdk_enabled: remote_settings.one_shot_sdk_enabled,
        sdk_installed: runtime.sdk_installed,
        sdk_version: runtime.sdk_version,
        sdk_install_dir: remote_settings.sdk_install_dir,
        task_execution_effective_provider: runtime.task_execution_effective_provider,
        one_shot_effective_provider: runtime.one_shot_effective_provider,
        sdk_status_message: runtime.status_message,
        database_loaded: true,
        database_path: database_path(&app).map(|path| path.to_string_lossy().to_string()),
        database_current_version: migration_status
            .as_ref()
            .and_then(|status| status.current_version),
        database_current_description: migration_status
            .as_ref()
            .and_then(|status| status.current_description.clone()),
        database_latest_version: latest_registered_version,
        shell_available: true,
        password_auth_available: matches!(
            ssh_config.password_probe_status.as_deref(),
            Some("passed" | "available")
        ),
        password_probe_status: ssh_config.password_probe_status,
        last_session_error: None,
        checked_at,
    })
}

#[tauri::command]
pub async fn install_remote_codex_sdk<R: Runtime>(
    app: AppHandle<R>,
    ssh_config_id: String,
) -> Result<crate::db::models::CodexSdkInstallResult, String> {
    let pool = sqlite_pool(&app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
    let remote_settings = ensure_remote_sdk_runtime_layout(&app, &ssh_config_id).await?;
    let install_dir = remote_shell_path_expression(&remote_settings.sdk_install_dir);
    let package_json = shell_escape_single_quoted(
        "{\"name\":\"codex-ai-sdk-runtime\",\"private\":true,\"type\":\"module\"}",
    );
    let remote_script = format!(
        "install_dir={install_dir}; mkdir -p \"$install_dir\" && cd \"$install_dir\" && \
if [ ! -f package.json ]; then printf '%s' {package_json} > package.json; fi && \
npm install --no-audit --no-fund --include=optional @openai/codex-sdk @openai/codex && \
sdk_pkg=\"$install_dir\"/node_modules/@openai/codex-sdk/package.json && \
sdk_version=$(sed -n 's/.*\"version\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p' \"$sdk_pkg\" | head -n 1) && \
node_version=$(node --version 2>/dev/null || true) && \
printf 'SDK_VERSION=%s\\nNODE_VERSION=%s\\n' \"$sdk_version\" \"$node_version\""
    );
    let output = execute_ssh_command(
        &app,
        &ssh_config,
        &build_remote_shell_command(
            &remote_script,
            remote_settings.node_path_override.as_deref(),
        ),
        true,
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "远程安装 Codex SDK 失败".to_string()
        } else {
            format!("远程安装 Codex SDK 失败：{}", redact_secret_text(&stderr))
        });
    }

    let values = parse_remote_key_value_output(&String::from_utf8_lossy(&output.stdout));
    let result = crate::db::models::CodexSdkInstallResult {
        execution_target: EXECUTION_TARGET_SSH.to_string(),
        ssh_config_id: Some(ssh_config_id.clone()),
        target_host_label: Some(ssh_config_target_host_label(&ssh_config)),
        sdk_installed: true,
        sdk_version: values
            .get("SDK_VERSION")
            .cloned()
            .filter(|value| !value.is_empty()),
        install_dir: remote_settings.sdk_install_dir,
        node_version: values
            .get("NODE_VERSION")
            .cloned()
            .filter(|value| !value.is_empty()),
        message: "远程 Codex SDK 安装完成".to_string(),
    };
    let _ = insert_activity_log(
        &pool,
        "remote_sdk_installed",
        &ssh_config_target_host_label(&ssh_config),
        None,
        None,
        None,
    )
    .await;
    Ok(result)
}

#[tauri::command]
pub async fn create_employee<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateEmployee,
) -> Result<Employee, String> {
    let pool = sqlite_pool(&app).await?;
    let project_id = normalize_optional_text(payload.project_id.as_deref());
    if let Some(project_id) = project_id.as_deref() {
        ensure_project_exists(&pool, project_id).await?;
    }

    let employee = Employee {
        id: new_id(),
        name: payload.name.trim().to_string(),
        role: payload.role,
        model: payload.model.unwrap_or_else(|| "gpt-5.4".to_string()),
        reasoning_effort: payload
            .reasoning_effort
            .unwrap_or_else(|| "high".to_string()),
        status: "offline".to_string(),
        specialization: normalize_optional_text(payload.specialization.as_deref()),
        system_prompt: normalize_optional_text(payload.system_prompt.as_deref()),
        project_id,
        created_at: now_sqlite(),
        updated_at: now_sqlite(),
    };

    if employee.name.is_empty() {
        return Err("员工名称不能为空".to_string());
    }

    sqlx::query(
        "INSERT INTO employees (id, name, role, model, reasoning_effort, status, specialization, system_prompt, project_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(&employee.id)
    .bind(&employee.name)
    .bind(&employee.role)
    .bind(&employee.model)
    .bind(&employee.reasoning_effort)
    .bind(&employee.status)
    .bind(&employee.specialization)
    .bind(&employee.system_prompt)
    .bind(&employee.project_id)
    .bind(&employee.created_at)
    .bind(&employee.updated_at)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to create employee: {}", error))?;

    fetch_employee_by_id(&pool, &employee.id).await
}

#[tauri::command]
pub async fn update_employee<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateEmployee,
) -> Result<Employee, String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_employee_by_id(&pool, &id).await?;
    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE employees SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(name) = updates.name {
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return Err("员工名称不能为空".to_string());
        }
        separated.push("name = ").push_bind_unseparated(trimmed);
        touched = true;
    }
    if let Some(role) = updates.role {
        separated.push("role = ").push_bind_unseparated(role);
        touched = true;
    }
    if let Some(model) = updates.model {
        separated.push("model = ").push_bind_unseparated(model);
        touched = true;
    }
    if let Some(reasoning_effort) = updates.reasoning_effort {
        separated
            .push("reasoning_effort = ")
            .push_bind_unseparated(reasoning_effort);
        touched = true;
    }
    if let Some(status) = updates.status {
        separated.push("status = ").push_bind_unseparated(status);
        touched = true;
    }
    if let Some(specialization) = updates.specialization {
        separated.push("specialization = ").push_bind_unseparated(
            specialization.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(system_prompt) = updates.system_prompt {
        separated.push("system_prompt = ").push_bind_unseparated(
            system_prompt.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(project_id) = updates.project_id {
        let project_id = match project_id {
            Some(project_id) => {
                let project_id = normalize_optional_text(Some(&project_id));
                if let Some(project_id) = project_id.as_deref() {
                    ensure_project_exists(&pool, project_id).await?;
                }
                project_id
            }
            None => None,
        };
        separated
            .push("project_id = ")
            .push_bind_unseparated(project_id);
        touched = true;
    }

    if !touched {
        return Ok(current);
    }

    builder.push(" WHERE id = ").push_bind(&id);
    builder
        .build()
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update employee: {}", error))?;

    fetch_employee_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn delete_employee<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    id: String,
) -> Result<(), String> {
    {
        let manager = state.lock().map_err(|error| error.to_string())?;
        if manager.is_running(&id) {
            return Err("员工仍有运行中的 Codex 会话，不能删除".to_string());
        }
    }

    let pool = sqlite_pool(&app).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start employee transaction: {}", error))?;

    sqlx::query("UPDATE tasks SET assignee_id = NULL WHERE assignee_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to clear employee assignments: {}", error))?;
    sqlx::query("UPDATE activity_logs SET employee_id = NULL WHERE employee_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to preserve employee activity logs: {}", error))?;
    sqlx::query("DELETE FROM employee_metrics WHERE employee_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete employee metrics: {}", error))?;
    sqlx::query("DELETE FROM employees WHERE id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete employee: {}", error))?;

    tx.commit()
        .await
        .map_err(|error| format!("Failed to commit employee delete: {}", error))?;

    Ok(())
}

#[tauri::command]
pub async fn update_employee_status<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    status: String,
) -> Result<Employee, String> {
    let pool = sqlite_pool(&app).await?;
    sqlx::query("UPDATE employees SET status = $1 WHERE id = $2")
        .bind(&status)
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update employee status: {}", error))?;

    fetch_employee_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn create_task<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateTask,
) -> Result<Task, String> {
    let pool = sqlite_pool(&app).await?;
    ensure_project_exists(&pool, &payload.project_id).await?;
    validate_assignee_for_project(&pool, payload.assignee_id.as_deref(), &payload.project_id)
        .await?;
    validate_reviewer_for_project(&pool, payload.reviewer_id.as_deref(), &payload.project_id)
        .await?;
    let project = fetch_project_by_id(&pool, &payload.project_id).await?;
    let settings = load_codex_settings(&app).ok();
    let automation_mode = settings
        .as_ref()
        .filter(|settings| settings.task_automation_default_enabled)
        .map(|_| "review_fix_loop_v1".to_string());

    if automation_mode.is_some()
        && normalize_optional_text(payload.reviewer_id.as_deref()).is_none()
    {
        return Err("当前已开启“新建任务默认自动质控”，请先指定审查员。".to_string());
    }

    let task = Task {
        id: new_id(),
        title: payload.title.trim().to_string(),
        description: normalize_optional_text(payload.description.as_deref()),
        status: "todo".to_string(),
        priority: payload.priority.unwrap_or_else(|| "medium".to_string()),
        project_id: payload.project_id,
        assignee_id: normalize_optional_text(payload.assignee_id.as_deref()),
        reviewer_id: normalize_optional_text(payload.reviewer_id.as_deref()),
        complexity: None,
        ai_suggestion: None,
        automation_mode,
        last_codex_session_id: None,
        last_review_session_id: None,
        created_at: now_sqlite(),
        updated_at: now_sqlite(),
    };

    if task.title.is_empty() {
        return Err("任务标题不能为空".to_string());
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start task transaction: {}", error))?;

    insert_task_record(&mut tx, &task).await?;

    if task.automation_mode.is_some() {
        sqlx::query(
            r#"
            INSERT INTO task_automation_state (
                task_id,
                phase,
                round_count,
                consumed_session_id,
                last_trigger_session_id,
                pending_action,
                pending_round_count,
                last_error,
                last_verdict_json,
                updated_at
            ) VALUES ($1, 'idle', 0, NULL, NULL, NULL, NULL, NULL, NULL, $2)
            "#,
        )
        .bind(&task.id)
        .bind(now_sqlite())
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to initialize task automation state: {}", error))?;
    }

    let mut uploaded_remote_paths = Vec::new();
    let attachments = if let Some(source_paths) = payload.attachment_source_paths.as_ref() {
        let attachments = build_task_attachments_from_sources(&app, &task.id, source_paths, 1)?;
        if project.project_type == PROJECT_TYPE_SSH && !attachments.is_empty() {
            let ssh_config_id = project
                .ssh_config_id
                .as_deref()
                .ok_or_else(|| "当前 SSH 项目未绑定 SSH 配置，无法同步图片到远程".to_string())?;
            match sync_task_attachment_records_to_remote(&app, ssh_config_id, &attachments, false)
                .await
            {
                Ok(sync_result) => {
                    uploaded_remote_paths = sync_result.remote_paths;
                }
                Err(error) => {
                    cleanup_task_attachment_files(
                        &attachments
                            .iter()
                            .map(|attachment| attachment.stored_path.clone())
                            .collect::<Vec<_>>(),
                    );
                    cleanup_empty_attachment_dir(&app, &task.id);
                    tx.rollback().await.ok();
                    return Err(error);
                }
            }
        }
        if let Err(error) = insert_task_attachments(&mut tx, &attachments).await {
            cleanup_task_attachment_files(
                &attachments
                    .iter()
                    .map(|attachment| attachment.stored_path.clone())
                    .collect::<Vec<_>>(),
            );
            cleanup_empty_attachment_dir(&app, &task.id);
            if project.project_type == PROJECT_TYPE_SSH {
                if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
                    cleanup_remote_task_attachment_paths(
                        &app,
                        ssh_config_id,
                        &uploaded_remote_paths,
                    )
                    .await;
                }
            }
            tx.rollback().await.ok();
            return Err(error);
        }
        attachments
    } else {
        Vec::new()
    };

    if let Err(error) = tx.commit().await {
        cleanup_task_attachment_files(
            &attachments
                .iter()
                .map(|attachment| attachment.stored_path.clone())
                .collect::<Vec<_>>(),
        );
        cleanup_empty_attachment_dir(&app, &task.id);
        if project.project_type == PROJECT_TYPE_SSH {
            if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
                cleanup_remote_task_attachment_paths(&app, ssh_config_id, &uploaded_remote_paths)
                    .await;
            }
        }
        return Err(format!("Failed to commit task create: {}", error));
    }

    insert_activity_log(
        &pool,
        "task_created",
        &format!(
            "{}{}",
            task.title,
            if attachments.is_empty() {
                "".to_string()
            } else {
                format!("（含 {} 张图片附件）", attachments.len())
            }
        ),
        None,
        Some(&task.id),
        Some(&task.project_id),
    )
    .await?;

    if project.project_type == PROJECT_TYPE_SSH && !attachments.is_empty() {
        insert_activity_log(
            &pool,
            "remote_task_attachments_synced",
            &format!(
                "{}（已同步 {} 张图片到远程）",
                task.title,
                attachments.len()
            ),
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    if task.automation_mode.is_some() {
        insert_activity_log(
            &pool,
            "task_automation_enabled",
            &format!("{}（新建任务默认开启）", task.title),
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    fetch_task_by_id(&pool, &task.id).await
}

#[tauri::command]
pub async fn add_task_attachments<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    source_paths: Vec<String>,
) -> Result<Vec<TaskAttachment>, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &task_id).await?;
    let project = fetch_project_by_id(&pool, &task.project_id).await?;

    if source_paths.is_empty() {
        return Ok(Vec::new());
    }

    let start_sort_order = resolve_next_task_attachment_sort_order(&pool, &task_id).await?;
    let attachments =
        build_task_attachments_from_sources(&app, &task_id, &source_paths, start_sort_order)?;
    let mut uploaded_remote_paths = Vec::new();

    if project.project_type == PROJECT_TYPE_SSH && !attachments.is_empty() {
        let ssh_config_id = project
            .ssh_config_id
            .as_deref()
            .ok_or_else(|| "当前 SSH 项目未绑定 SSH 配置，无法同步图片到远程".to_string())?;
        match sync_task_attachment_records_to_remote(&app, ssh_config_id, &attachments, false).await
        {
            Ok(sync_result) => {
                uploaded_remote_paths = sync_result.remote_paths;
            }
            Err(error) => {
                cleanup_task_attachment_files(
                    &attachments
                        .iter()
                        .map(|attachment| attachment.stored_path.clone())
                        .collect::<Vec<_>>(),
                );
                cleanup_empty_attachment_dir(&app, &task_id);
                return Err(error);
            }
        }
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start attachment transaction: {}", error))?;

    if let Err(error) = insert_task_attachments(&mut tx, &attachments).await {
        cleanup_task_attachment_files(
            &attachments
                .iter()
                .map(|attachment| attachment.stored_path.clone())
                .collect::<Vec<_>>(),
        );
        cleanup_empty_attachment_dir(&app, &task_id);
        if project.project_type == PROJECT_TYPE_SSH {
            if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
                cleanup_remote_task_attachment_paths(&app, ssh_config_id, &uploaded_remote_paths)
                    .await;
            }
        }
        tx.rollback().await.ok();
        return Err(error);
    }

    if let Err(error) = tx.commit().await {
        cleanup_task_attachment_files(
            &attachments
                .iter()
                .map(|attachment| attachment.stored_path.clone())
                .collect::<Vec<_>>(),
        );
        cleanup_empty_attachment_dir(&app, &task_id);
        if project.project_type == PROJECT_TYPE_SSH {
            if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
                cleanup_remote_task_attachment_paths(&app, ssh_config_id, &uploaded_remote_paths)
                    .await;
            }
        }
        return Err(format!("Failed to commit attachment create: {}", error));
    }

    if project.project_type == PROJECT_TYPE_SSH && !attachments.is_empty() {
        insert_activity_log(
            &pool,
            "remote_task_attachments_synced",
            &format!(
                "{}（追加同步 {} 张图片到远程）",
                task.title,
                attachments.len()
            ),
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    fetch_task_attachments(&pool, &task_id).await
}

#[tauri::command]
pub async fn delete_task_attachment<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let attachment = fetch_task_attachment_by_id(&pool, &id).await?;
    let task = fetch_task_by_id(&pool, &attachment.task_id).await?;
    let project = fetch_project_by_id(&pool, &task.project_id).await?;
    let stored_path = Path::new(&attachment.stored_path);

    if stored_path.exists() {
        fs::remove_file(stored_path)
            .map_err(|error| format!("删除附件文件失败: {}: {}", stored_path.display(), error))?;
    }

    sqlx::query("DELETE FROM task_attachments WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to delete task attachment: {}", error))?;

    cleanup_empty_attachment_dir(&app, &attachment.task_id);
    if project.project_type == PROJECT_TYPE_SSH {
        if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
            if let Err(error) =
                cleanup_remote_task_attachment(&app, ssh_config_id, &attachment).await
            {
                eprintln!("[task-attachments] 删除远程附件失败: {}", error);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn update_task<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateTask,
) -> Result<Task, String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_task_by_id(&pool, &id).await?;
    let next_status = updates
        .status
        .clone()
        .unwrap_or_else(|| current.status.clone());

    if let Some(assignee_id) = updates.assignee_id.as_ref() {
        validate_assignee_for_project(&pool, assignee_id.as_deref(), &current.project_id).await?;
    }
    if let Some(reviewer_id) = updates.reviewer_id.as_ref() {
        validate_reviewer_for_project(&pool, reviewer_id.as_deref(), &current.project_id).await?;
    }

    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE tasks SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(title) = updates.title {
        let trimmed = title.trim().to_string();
        if trimmed.is_empty() {
            return Err("任务标题不能为空".to_string());
        }
        separated.push("title = ").push_bind_unseparated(trimmed);
        touched = true;
    }
    if let Some(description) = updates.description {
        separated.push("description = ").push_bind_unseparated(
            description.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(status) = updates.status.clone() {
        separated.push("status = ").push_bind_unseparated(status);
        touched = true;
    }
    if let Some(priority) = updates.priority {
        separated
            .push("priority = ")
            .push_bind_unseparated(priority);
        touched = true;
    }
    if let Some(assignee_id) = updates.assignee_id {
        separated
            .push("assignee_id = ")
            .push_bind_unseparated(assignee_id);
        touched = true;
    }
    if let Some(reviewer_id) = updates.reviewer_id {
        separated
            .push("reviewer_id = ")
            .push_bind_unseparated(reviewer_id);
        touched = true;
    }
    if let Some(complexity) = updates.complexity {
        separated
            .push("complexity = ")
            .push_bind_unseparated(complexity);
        touched = true;
    }
    if let Some(ai_suggestion) = updates.ai_suggestion {
        separated
            .push("ai_suggestion = ")
            .push_bind_unseparated(ai_suggestion);
        touched = true;
    }
    if let Some(last_codex_session_id) = updates.last_codex_session_id {
        separated
            .push("last_codex_session_id = ")
            .push_bind_unseparated(last_codex_session_id);
        touched = true;
    }
    if let Some(last_review_session_id) = updates.last_review_session_id {
        separated
            .push("last_review_session_id = ")
            .push_bind_unseparated(last_review_session_id);
        touched = true;
    }

    if !touched {
        return Ok(current);
    }

    builder.push(" WHERE id = ").push_bind(&id);
    builder
        .build()
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update task: {}", error))?;

    if next_status != current.status {
        insert_activity_log(
            &pool,
            "task_status_changed",
            &format!("{} -> {}", current.title, next_status),
            None,
            Some(&id),
            Some(&current.project_id),
        )
        .await?;

        if current.status != "completed" && next_status == "completed" {
            let updated_task = fetch_task_by_id(&pool, &id).await?;
            record_completion_metric(&pool, &updated_task).await?;
        }
    }

    fetch_task_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn update_task_status<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    status: String,
) -> Result<Task, String> {
    update_task(
        app,
        id,
        UpdateTask {
            title: None,
            description: None,
            status: Some(status),
            priority: None,
            assignee_id: None,
            reviewer_id: None,
            complexity: None,
            ai_suggestion: None,
            last_codex_session_id: None,
            last_review_session_id: None,
        },
    )
    .await
}

#[tauri::command]
pub async fn delete_task<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &id).await?;
    let project = fetch_project_by_id(&pool, &task.project_id).await?;
    let attachment_dir = task_attachment_dir(&app, &id).ok();
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start task transaction: {}", error))?;

    sqlx::query("DELETE FROM activity_logs WHERE task_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete task activity logs: {}", error))?;
    sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to delete task: {}", error))?;

    tx.commit()
        .await
        .map_err(|error| format!("Failed to commit task delete: {}", error))?;

    if let Some(attachment_dir) = attachment_dir.filter(|path| path.exists()) {
        if let Err(error) = fs::remove_dir_all(&attachment_dir) {
            eprintln!(
                "[task-attachments] 删除任务附件目录失败: path={}, error={}",
                attachment_dir.display(),
                error
            );
        }
    }

    if project.project_type == PROJECT_TYPE_SSH {
        if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
            if let Err(error) =
                cleanup_remote_task_attachments_for_task(&app, ssh_config_id, &task.id).await
            {
                eprintln!("[task-attachments] 删除远程任务附件目录失败: {}", error);
            }
        }
    }

    insert_activity_log(
        &pool,
        "task_deleted",
        &task.title,
        None,
        None,
        Some(&task.project_id),
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn create_subtask<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateSubtask,
) -> Result<Subtask, String> {
    let pool = sqlite_pool(&app).await?;
    let title = payload.title.trim().to_string();
    if title.is_empty() {
        return Err("子任务标题不能为空".to_string());
    }

    let sort_order = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM subtasks WHERE task_id = $1",
    )
    .bind(&payload.task_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to resolve subtask order: {}", error))?
    .flatten()
    .unwrap_or(1);

    let id = new_id();
    sqlx::query("INSERT INTO subtasks (id, task_id, title, sort_order) VALUES ($1, $2, $3, $4)")
        .bind(&id)
        .bind(&payload.task_id)
        .bind(title)
        .bind(sort_order)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to create subtask: {}", error))?;

    sqlx::query_as::<_, Subtask>("SELECT * FROM subtasks WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch created subtask: {}", error))
}

#[tauri::command]
pub async fn update_subtask_status<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    status: String,
) -> Result<Subtask, String> {
    let pool = sqlite_pool(&app).await?;
    sqlx::query("UPDATE subtasks SET status = $1 WHERE id = $2")
        .bind(&status)
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update subtask status: {}", error))?;

    sqlx::query_as::<_, Subtask>("SELECT * FROM subtasks WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch subtask: {}", error))
}

#[tauri::command]
pub async fn delete_subtask<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    sqlx::query("DELETE FROM subtasks WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to delete subtask: {}", error))?;

    Ok(())
}

#[tauri::command]
pub async fn create_comment<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateComment,
) -> Result<Comment, String> {
    let pool = sqlite_pool(&app).await?;
    let content = payload.content.trim().to_string();
    if content.is_empty() {
        return Err("评论内容不能为空".to_string());
    }

    let id = new_id();
    sqlx::query(
        "INSERT INTO comments (id, task_id, employee_id, content, is_ai_generated) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(&payload.task_id)
    .bind(payload.employee_id)
    .bind(content)
    .bind(if payload.is_ai_generated.unwrap_or(false) {
        1_i64
    } else {
        0_i64
    })
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to create comment: {}", error))?;

    sqlx::query_as::<_, Comment>("SELECT * FROM comments WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch created comment: {}", error))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::process::Command;

    use sqlx::SqlitePool;

    use super::{
        build_current_migrator, build_remote_codex_runtime_health, build_remote_shell_command,
        build_task_review_context_from_git_outputs, build_task_review_prompt,
        ensure_statement_terminated, fetch_execution_change_history_item_by_session_id,
        fetch_task_by_id, insert_task_record, normalize_runtime_path_string,
        remote_shell_path_expression, remote_task_attachment_dir, remote_task_attachment_path,
        resolve_session_resume_state, rewrite_file_change_diff_labels, sanitize_sql_backup_script,
        validate_project_repo_path, validate_runtime_working_dir, CodexSettings, Project, Task,
        TaskAttachment, PROJECT_TYPE_LOCAL, PROJECT_TYPE_SSH,
    };

    async fn setup_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");
        let migrator = build_current_migrator();
        let mut connection = pool.acquire().await.expect("acquire sqlite connection");
        migrator
            .run_direct(&mut *connection)
            .await
            .expect("run migrations");
        drop(connection);
        pool
    }

    async fn insert_session(
        pool: &SqlitePool,
        session_id: &str,
        cli_session_id: Option<&str>,
        session_kind: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO codex_sessions (
                id,
                cli_session_id,
                session_kind,
                status,
                started_at,
                created_at
            ) VALUES ($1, $2, $3, 'exited', '2026-04-16 10:00:00', '2026-04-16 10:00:00')
            "#,
        )
        .bind(session_id)
        .bind(cli_session_id)
        .bind(session_kind)
        .execute(pool)
        .await
        .expect("insert session");
    }

    async fn insert_session_started_event(pool: &SqlitePool, session_id: &str, message: &str) {
        sqlx::query(
            r#"
            INSERT INTO codex_session_events (
                id,
                session_id,
                event_type,
                message,
                created_at
            ) VALUES ($1, $2, 'session_started', $3, '2026-04-16 10:00:01')
            "#,
        )
        .bind(format!("event-{session_id}"))
        .bind(session_id)
        .bind(message)
        .execute(pool)
        .await
        .expect("insert session started event");
    }

    async fn insert_file_change(
        pool: &SqlitePool,
        change_id: &str,
        session_id: &str,
        path: &str,
        capture_mode: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO codex_session_file_changes (
                id,
                session_id,
                path,
                change_type,
                capture_mode,
                previous_path,
                created_at
            ) VALUES ($1, $2, $3, 'modified', $4, NULL, '2026-04-16 10:00:02')
            "#,
        )
        .bind(change_id)
        .bind(session_id)
        .bind(path)
        .bind(capture_mode)
        .execute(pool)
        .await
        .expect("insert file change");
    }

    async fn insert_project(pool: &SqlitePool, project_id: &str) {
        sqlx::query(
            r#"
            INSERT INTO projects (
                id,
                name,
                description,
                status,
                repo_path,
                created_at,
                updated_at
            ) VALUES ($1, $2, NULL, 'active', NULL, '2026-04-16 10:00:00', '2026-04-16 10:00:00')
            "#,
        )
        .bind(project_id)
        .bind(format!("Project {project_id}"))
        .execute(pool)
        .await
        .expect("insert project");
    }

    async fn insert_employee(pool: &SqlitePool, employee_id: &str, name: &str, role: &str) {
        sqlx::query(
            r#"
            INSERT INTO employees (
                id,
                name,
                role,
                model,
                reasoning_effort,
                status,
                specialization,
                system_prompt,
                project_id,
                created_at,
                updated_at
            ) VALUES ($1, $2, $3, 'gpt-5.4', 'high', 'offline', NULL, NULL, NULL, '2026-04-16 10:00:00', '2026-04-16 10:00:00')
            "#,
        )
        .bind(employee_id)
        .bind(name)
        .bind(role)
        .execute(pool)
        .await
        .expect("insert employee");
    }

    #[test]
    fn rejects_missing_project_repo_path() {
        let path = format!(
            "{}/codex-ai-test-missing-{}",
            std::env::temp_dir().display(),
            std::process::id()
        );

        assert!(validate_project_repo_path(Some(&path)).is_err());
    }

    #[test]
    fn validates_git_runtime_directory() {
        let root = std::env::temp_dir().join(format!(
            "codex-ai-runtime-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let git_dir = root.join(".git");
        fs::create_dir_all(&git_dir).expect("create .git dir");

        let validated = validate_runtime_working_dir(Some(root.to_string_lossy().as_ref()))
            .expect("valid git working dir");
        assert!(validated.contains("codex-ai-runtime"));

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn strips_windows_verbatim_path_prefixes() {
        assert_eq!(
            normalize_runtime_path_string(r"\\?\D:\repo\demo"),
            r"D:\repo\demo"
        );
        assert_eq!(
            normalize_runtime_path_string(r"\\?\UNC\server\share\demo"),
            r"\\server\share\demo"
        );
        assert_eq!(
            normalize_runtime_path_string(r"/tmp/codex-ai"),
            "/tmp/codex-ai"
        );
    }

    #[test]
    fn remote_shell_path_expression_expands_home_prefix() {
        assert_eq!(
            remote_shell_path_expression("~/codex sdk"),
            "\"$HOME/codex sdk\""
        );
        assert_eq!(
            remote_shell_path_expression("${HOME}/runtime"),
            "\"$HOME/runtime\""
        );
    }

    #[test]
    fn remote_shell_command_bootstraps_common_node_paths() {
        let command = build_remote_shell_command(
            "codex --version",
            Some("~/.nvm/versions/node/v22.0.0/bin/node"),
        );

        assert!(command.starts_with("sh -lc "));
        assert!(command.contains(".nvm/versions/node/*/bin"));
        assert!(command.contains(". \"$HOME/.nvm/nvm.sh\""));
        assert!(command.contains(".local/share/pnpm"));
        assert!(command.contains("$HOME/.nvm/versions/node/v22.0.0/bin"));
    }

    #[test]
    fn remote_shell_bootstrap_prefers_nvm_default_alias_before_fallback_versions() {
        let root = std::env::temp_dir().join(format!("codex-nvm-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join(".nvm/versions/node/v18.20.0/bin")).expect("create v18 dir");
        fs::create_dir_all(root.join(".nvm/versions/node/v9.11.2/bin")).expect("create v9 dir");
        fs::write(
            root.join(".nvm/versions/node/v18.20.0/bin/node"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("write fake v18 node");
        fs::write(
            root.join(".nvm/versions/node/v9.11.2/bin/node"),
            "#!/bin/sh\nexit 0\n",
        )
        .expect("write fake v9 node");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(
                root.join(".nvm/versions/node/v18.20.0/bin/node"),
                fs::Permissions::from_mode(0o755),
            )
            .expect("chmod fake v18 node");
            fs::set_permissions(
                root.join(".nvm/versions/node/v9.11.2/bin/node"),
                fs::Permissions::from_mode(0o755),
            )
            .expect("chmod fake v9 node");
        }
        fs::write(
            root.join(".nvm/nvm.sh"),
            r#"nvm() {
  if [ "$1" = "which" ] && [ "$2" = "default" ]; then
    printf '%s\n' "$HOME/.nvm/versions/node/v18.20.0/bin/node"
    return 0
  fi
  return 1
}
"#,
        )
        .expect("write fake nvm script");

        let output = Command::new("sh")
            .env("HOME", &root)
            .arg("-lc")
            .arg(build_remote_shell_command("printf '%s' \"$PATH\"", None))
            .output()
            .expect("run bootstrap command");
        assert!(output.status.success(), "bootstrap shell should succeed");

        let path = String::from_utf8_lossy(&output.stdout).to_string();
        let expected_v18 = root
            .join(".nvm/versions/node/v18.20.0/bin")
            .to_string_lossy()
            .to_string();
        let expected_v9 = root
            .join(".nvm/versions/node/v9.11.2/bin")
            .to_string_lossy()
            .to_string();

        assert!(path.starts_with(&format!("{expected_v18}:")));
        assert!(path.contains(&expected_v9));
        assert!(
            path.find(&expected_v18).unwrap_or(usize::MAX)
                < path.find(&expected_v9).unwrap_or(usize::MAX)
        );

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn remote_runtime_health_falls_back_to_exec_when_sdk_is_missing() {
        let settings = CodexSettings {
            task_sdk_enabled: true,
            one_shot_sdk_enabled: true,
            one_shot_model: "gpt-5.4".to_string(),
            one_shot_reasoning_effort: "high".to_string(),
            task_automation_default_enabled: false,
            task_automation_max_fix_rounds: 3,
            task_automation_failure_strategy: "blocked".to_string(),
            node_path_override: None,
            sdk_install_dir: "~/.codex-ai/codex-sdk-runtime/ssh-1".to_string(),
            one_shot_preferred_provider: "sdk".to_string(),
        };
        let values = HashMap::from([
            ("CODEX_STATUS".to_string(), "0".to_string()),
            ("CODEX_VERSION".to_string(), "0.34.0".to_string()),
            ("NODE_STATUS".to_string(), "0".to_string()),
            ("NODE_VERSION".to_string(), "v22.15.0".to_string()),
            ("SDK_INSTALLED".to_string(), "0".to_string()),
            ("SDK_VERSION".to_string(), "".to_string()),
        ]);

        let runtime = build_remote_codex_runtime_health(&settings, &values, "");

        assert_eq!(runtime.task_execution_effective_provider, "exec");
        assert_eq!(runtime.one_shot_effective_provider, "exec");
        assert!(runtime.status_message.contains("未安装"));
    }

    #[test]
    fn remote_runtime_health_rejects_unsupported_node_versions() {
        let settings = CodexSettings {
            task_sdk_enabled: true,
            one_shot_sdk_enabled: true,
            one_shot_model: "gpt-5.4".to_string(),
            one_shot_reasoning_effort: "high".to_string(),
            task_automation_default_enabled: false,
            task_automation_max_fix_rounds: 3,
            task_automation_failure_strategy: "blocked".to_string(),
            node_path_override: None,
            sdk_install_dir: "~/.codex-ai/codex-sdk-runtime/ssh-1".to_string(),
            one_shot_preferred_provider: "sdk".to_string(),
        };
        let values = HashMap::from([
            ("CODEX_STATUS".to_string(), "0".to_string()),
            ("CODEX_VERSION".to_string(), "0.34.0".to_string()),
            ("NODE_STATUS".to_string(), "0".to_string()),
            ("NODE_VERSION".to_string(), "v9.11.2".to_string()),
            ("SDK_INSTALLED".to_string(), "1".to_string()),
            ("SDK_VERSION".to_string(), "0.12.0".to_string()),
        ]);

        let runtime = build_remote_codex_runtime_health(&settings, &values, "");

        assert_eq!(runtime.task_execution_effective_provider, "exec");
        assert_eq!(runtime.one_shot_effective_provider, "exec");
        assert!(runtime.status_message.contains("Node 版本过低"));
    }

    #[test]
    fn remote_task_attachment_dir_uses_home_scoped_task_folder() {
        assert_eq!(
            remote_task_attachment_dir("/home/demo", "task-1"),
            "/home/demo/.codex-ai/img/task-1"
        );
    }

    #[test]
    fn remote_task_attachment_path_reuses_managed_file_name() {
        let attachment = TaskAttachment {
            id: "att-1".to_string(),
            task_id: "task-1".to_string(),
            original_name: "ui.png".to_string(),
            stored_path: "/tmp/task-attachments/task-1/att-1.png".to_string(),
            mime_type: "image/png".to_string(),
            file_size: 123,
            sort_order: 1,
            created_at: "2026-04-16 10:00:00".to_string(),
        };

        assert_eq!(
            remote_task_attachment_path("/home/demo", &attachment).expect("remote attachment path"),
            "/home/demo/.codex-ai/img/task-1/att-1.png"
        );
    }

    #[test]
    fn review_context_builder_accepts_remote_untracked_summary_without_snippets() {
        let context = build_task_review_context_from_git_outputs(
            " M src/main.rs\n?? notes.txt\n",
            " src/main.rs | 2 ++\n 1 file changed, 2 insertions(+)\n",
            "diff --git a/src/main.rs b/src/main.rs\n+println!(\"hi\");\n",
            "",
            "",
            &["notes.txt".to_string()],
            "未跟踪文件列表：\n- notes.txt\n\n未跟踪文本文件摘录：\n（SSH 模式暂不采集远程未跟踪文件内容摘录，请结合未跟踪文件列表人工确认）",
        )
        .expect("build review context");

        assert!(context.contains("## Git 状态"));
        assert!(context.contains("notes.txt"));
        assert!(context.contains("SSH 模式暂不采集远程未跟踪文件内容摘录"));
    }

    #[test]
    fn review_prompt_uses_explicit_remote_working_dir_for_ssh_projects() {
        let task = Task {
            id: "task-1".to_string(),
            title: "审核远程改动".to_string(),
            description: Some("检查 SSH 项目的改动".to_string()),
            status: "review".to_string(),
            priority: "high".to_string(),
            project_id: "project-1".to_string(),
            assignee_id: None,
            reviewer_id: Some("reviewer-1".to_string()),
            complexity: None,
            ai_suggestion: None,
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        };
        let project = Project {
            id: "project-1".to_string(),
            name: "SSH 项目".to_string(),
            description: None,
            status: "active".to_string(),
            repo_path: None,
            project_type: PROJECT_TYPE_SSH.to_string(),
            ssh_config_id: Some("ssh-1".to_string()),
            remote_repo_path: Some("/srv/demo".to_string()),
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        };

        let prompt =
            build_task_review_prompt(&task, &project, "/srv/demo", "## Git 状态\n M src/main.rs");

        assert!(prompt.contains("仓库路径：/srv/demo"));
        assert!(prompt.contains("执行目标：SSH 远程工作区"));
        assert!(!prompt.contains("仓库路径：（未配置）"));
    }

    #[test]
    fn review_prompt_marks_local_projects_as_local_workspace() {
        let task = Task {
            id: "task-2".to_string(),
            title: "审核本地改动".to_string(),
            description: None,
            status: "review".to_string(),
            priority: "medium".to_string(),
            project_id: "project-2".to_string(),
            assignee_id: None,
            reviewer_id: Some("reviewer-2".to_string()),
            complexity: None,
            ai_suggestion: None,
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        };
        let project = Project {
            id: "project-2".to_string(),
            name: "本地项目".to_string(),
            description: None,
            status: "active".to_string(),
            repo_path: Some("/tmp/demo".to_string()),
            project_type: PROJECT_TYPE_LOCAL.to_string(),
            ssh_config_id: None,
            remote_repo_path: None,
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        };

        let prompt =
            build_task_review_prompt(&task, &project, "/tmp/demo", "## Git 状态\n M src/main.rs");

        assert!(prompt.contains("执行目标：本地工作区"));
    }

    #[test]
    fn sanitizes_sql_control_statements() {
        let source = "\u{feff}BEGIN TRANSACTION;\nCREATE TABLE demo(id INTEGER);\nCOMMIT;\nPRAGMA foreign_keys=OFF;\nINSERT INTO demo VALUES (1);\n";
        let sanitized = sanitize_sql_backup_script(source);

        assert!(sanitized.contains("CREATE TABLE demo(id INTEGER);"));
        assert!(sanitized.contains("INSERT INTO demo VALUES (1);"));
        assert!(!sanitized.contains("BEGIN TRANSACTION"));
        assert!(!sanitized.contains("COMMIT"));
        assert!(!sanitized.contains("PRAGMA foreign_keys=OFF"));
    }

    #[test]
    fn terminates_sql_statement_once() {
        assert_eq!(
            ensure_statement_terminated("CREATE TABLE demo(id INTEGER)"),
            "CREATE TABLE demo(id INTEGER);"
        );
        assert_eq!(
            ensure_statement_terminated("CREATE TABLE demo(id INTEGER);"),
            "CREATE TABLE demo(id INTEGER);"
        );
        assert_eq!(ensure_statement_terminated("   "), "");
    }

    #[test]
    fn rewrite_file_change_diff_labels_only_updates_headers() {
        let raw = concat!(
            "diff --git a/before.txt b/after.txt\n",
            "index 1111111..2222222 100644\n",
            "--- a/before.txt\n",
            "+++ b/after.txt\n",
            "@@ -1 +1 @@\n",
            "-const path = \"a/before.txt\";\n",
            "+const path = \"b/after.txt\";\n",
        );

        let rewritten = rewrite_file_change_diff_labels(raw, "src/old.ts", "src/new.ts");

        assert!(rewritten.contains("diff --git a/src/old.ts b/src/new.ts"));
        assert!(rewritten.contains("--- a/src/old.ts"));
        assert!(rewritten.contains("+++ b/src/new.ts"));
        assert!(rewritten.contains("-const path = \"a/before.txt\";"));
        assert!(rewritten.contains("+const path = \"b/after.txt\";"));
    }

    #[test]
    fn rewrite_file_change_diff_labels_keeps_dev_null_unprefixed() {
        let raw = concat!(
            "diff --git a/before.txt b/after.txt\n",
            "--- a/before.txt\n",
            "+++ b/after.txt\n",
        );

        let rewritten = rewrite_file_change_diff_labels(raw, "/dev/null", "src/new.ts");

        assert!(rewritten.contains("diff --git /dev/null b/src/new.ts"));
        assert!(rewritten.contains("--- /dev/null"));
        assert!(rewritten.contains("+++ b/src/new.ts"));
        assert!(!rewritten.contains("a//dev/null"));
        assert!(!rewritten.contains("b//dev/null"));
    }

    #[test]
    fn session_resume_state_requires_cli_session_id() {
        let (status, message, can_resume) =
            resolve_session_resume_state(None, Some("emp-1"), Some("Alice"), "exited", false);

        assert_eq!(status, "missing_cli_session");
        assert!(!can_resume);
        assert!(message.unwrap_or_default().contains("CLI session id"));
    }

    #[test]
    fn session_resume_state_blocks_when_employee_missing() {
        let (status, _, can_resume) =
            resolve_session_resume_state(Some("sess-1"), None, None, "exited", false);

        assert_eq!(status, "missing_employee");
        assert!(!can_resume);
    }

    #[test]
    fn session_resume_state_allows_resumable_exited_session() {
        let (status, message, can_resume) = resolve_session_resume_state(
            Some("sess-1"),
            Some("emp-1"),
            Some("Alice"),
            "exited",
            false,
        );

        assert_eq!(status, "ready");
        assert!(can_resume);
        assert!(message.is_none());
    }

    #[test]
    fn fetch_execution_change_history_item_returns_existing_changes() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            insert_session(&pool, "sess-1", Some("cli-sess-1"), "execution").await;
            insert_file_change(
                &pool,
                "change-1",
                "sess-1",
                "src/pages/SessionsPage.tsx",
                "sdk_event",
            )
            .await;

            let item = fetch_execution_change_history_item_by_session_id(&pool, "sess-1")
                .await
                .expect("fetch execution change history item");

            assert_eq!(item.session.id, "sess-1");
            assert_eq!(item.capture_mode, "sdk_event");
            assert_eq!(item.changes.len(), 1);
            assert_eq!(item.changes[0].path, "src/pages/SessionsPage.tsx");

            pool.close().await;
        });
    }

    #[test]
    fn fetch_execution_change_history_item_returns_empty_changes_when_missing() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            insert_session(&pool, "sess-2", Some("cli-sess-2"), "execution").await;

            let item = fetch_execution_change_history_item_by_session_id(&pool, "sess-2")
                .await
                .expect("fetch empty execution change history item");

            assert_eq!(item.session.id, "sess-2");
            assert!(item.changes.is_empty());
            assert_eq!(item.capture_mode, "git_fallback");

            pool.close().await;
        });
    }

    #[test]
    fn fetch_execution_change_history_item_falls_back_to_session_started_provider() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            insert_session(&pool, "sess-3", Some("cli-sess-3"), "execution").await;
            insert_session_started_event(
                &pool,
                "sess-3",
                "通过 SDK 启动，使用模型 gpt-5.4 / 推理强度 high / 图片 0 张",
            )
            .await;

            let item = fetch_execution_change_history_item_by_session_id(&pool, "sess-3")
                .await
                .expect("fetch provider fallback execution change history item");

            assert!(item.changes.is_empty());
            assert_eq!(item.capture_mode, "sdk_event");

            pool.close().await;
        });
    }

    #[test]
    fn insert_task_record_persists_reviewer_id() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            insert_project(&pool, "proj-1").await;
            insert_employee(&pool, "reviewer-1", "Reviewer", "reviewer").await;

            let task = Task {
                id: "task-1".to_string(),
                title: "测试任务".to_string(),
                description: Some("验证审核员持久化".to_string()),
                status: "todo".to_string(),
                priority: "medium".to_string(),
                project_id: "proj-1".to_string(),
                assignee_id: None,
                reviewer_id: Some("reviewer-1".to_string()),
                complexity: None,
                ai_suggestion: None,
                automation_mode: None,
                last_codex_session_id: None,
                last_review_session_id: None,
                created_at: "2026-04-16 10:00:00".to_string(),
                updated_at: "2026-04-16 10:00:00".to_string(),
            };

            let mut tx = pool.begin().await.expect("begin task transaction");
            insert_task_record(&mut tx, &task)
                .await
                .expect("insert task record");
            tx.commit().await.expect("commit task transaction");

            let saved_task = fetch_task_by_id(&pool, &task.id)
                .await
                .expect("fetch inserted task");
            assert_eq!(saved_task.reviewer_id.as_deref(), Some("reviewer-1"));

            pool.close().await;
        });
    }
}
