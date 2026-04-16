use std::borrow::Cow;
use std::collections::HashMap;
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
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_sql::{DbInstances, DbPool};
use uuid::Uuid;

use crate::codex::{inspect_sdk_runtime, load_codex_settings, new_codex_command, CodexManager};
use crate::db::models::{
    CodexHealthCheck, CodexRuntimeStatus, CodexSessionFileChange, CodexSessionFileChangeInput,
    CodexSessionListItem, CodexSessionLogLine, CodexSessionRecord, CodexSessionResumePreview,
    Comment, CreateComment, CreateEmployee, CreateProject, CreateSubtask, CreateTask,
    DatabaseBackupResult, DatabaseRestoreResult, Employee, EmployeeMetric, Project, Subtask, Task,
    TaskAttachment, TaskExecutionChangeHistoryItem, TaskLatestReview, UpdateEmployee,
    UpdateProject, UpdateTask,
};

pub const DB_URL: &str = "sqlite:codex-ai.db";
const DB_FILE_NAME: &str = "codex-ai.db";
const DB_AUTO_IMPORT_BACKUP_PREFIX: &str = "codex-ai.pre-import-backup";
const SQLITE_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
const REVIEW_DIFF_CHAR_LIMIT: usize = 120_000;
const REVIEW_UNTRACKED_FILE_LIMIT: usize = 5;
const REVIEW_UNTRACKED_FILE_SIZE_LIMIT: u64 = 16 * 1024;
const REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT: usize = 48_000;
const REVIEW_REPORT_START_TAG: &str = "<review_report>";
const REVIEW_REPORT_END_TAG: &str = "</review_report>";

struct DatabaseMigrationStatus {
    applied_count: i64,
    current_version: Option<i64>,
    current_description: Option<String>,
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

fn build_current_migrator() -> Migrator {
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
    let output = Command::new("git")
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

fn collect_task_review_context(repo_path: &str) -> Result<String, String> {
    let status_output = run_git_text(repo_path, &["status", "--short"])?;
    let status_trimmed = status_output.trim();
    if status_trimmed.is_empty() {
        return Err("当前工作区没有可审核的代码改动".to_string());
    }

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
    let untracked_section = if untracked_files.is_empty() {
        "（无未跟踪文件）".to_string()
    } else {
        format!(
            "未跟踪文件列表：\n{}\n\n未跟踪文本文件摘录：\n{}",
            untracked_files
                .iter()
                .map(|path| format!("- {}", path))
                .collect::<Vec<_>>()
                .join("\n"),
            read_untracked_review_snippets(repo_path, &untracked_files),
        )
    };

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

fn build_task_review_prompt(task: &Task, project: &Project, review_context: &str) -> String {
    format!(
        "你正在执行一次只读代码审查。\n\
要求：\n\
- 只允许阅读和分析代码，禁止修改任何文件，禁止执行 git commit/reset/checkout/merge/rebase 等写操作\n\
- 审核范围仅限下方提供的任务信息和当前工作区改动\n\
- 最终结论必须且只能输出在 {start_tag} 和 {end_tag} 之间\n\
- 报告必须使用中文 Markdown，包含以下小节：## 结论、## 阻断问题、## 风险提醒、## 改进建议、## 验证缺口\n\
- 如果没有阻断问题，明确写“无阻断问题”\n\
- 如果 diff 信息被截断，要把这件事写进“验证缺口”\n\n\
任务标题：{title}\n\
任务状态：{status}\n\
任务优先级：{priority}\n\
项目名称：{project_name}\n\
仓库路径：{repo_path}\n\
任务描述：{description}\n\n\
{review_context}",
        start_tag = REVIEW_REPORT_START_TAG,
        end_tag = REVIEW_REPORT_END_TAG,
        title = task.title.trim(),
        status = task.status.trim(),
        priority = task.priority.trim(),
        project_name = project.name.trim(),
        repo_path = project.repo_path.as_deref().unwrap_or("（未配置）"),
        description = task.description.as_deref().unwrap_or("（未填写）"),
        review_context = review_context,
    )
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
        sqlx::query(
            "INSERT INTO codex_session_file_changes (id, session_id, path, change_type, capture_mode, previous_path) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(new_id())
        .bind(session_id)
        .bind(&change.path)
        .bind(&change.change_type)
        .bind(&change.capture_mode)
        .bind(&change.previous_path)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to insert session file change: {}", error))?;
    }

    Ok(())
}

pub(crate) async fn insert_codex_session_record<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: Option<&str>,
    task_id: Option<&str>,
    working_dir: Option<&str>,
    resume_session_id: Option<&str>,
    session_kind: &str,
    status: &str,
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
        cli_session_id: None,
        working_dir: working_dir.map(ToOwned::to_owned),
        session_kind: session_kind.to_string(),
        status: status.to_string(),
        started_at: now_sqlite(),
        ended_at: None,
        exit_code: None,
        resume_session_id: resume_session_id.map(ToOwned::to_owned),
        created_at: now_sqlite(),
    };

    sqlx::query(
        "INSERT INTO codex_sessions (id, employee_id, task_id, project_id, cli_session_id, working_dir, session_kind, status, started_at, ended_at, exit_code, resume_session_id, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(&record.id)
    .bind(&record.employee_id)
    .bind(&record.task_id)
    .bind(&record.project_id)
    .bind(&record.cli_session_id)
    .bind(&record.working_dir)
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

async fn fetch_project_by_id(pool: &SqlitePool, id: &str) -> Result<Project, String> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Project {} not found: {}", id, error))
}

async fn fetch_employee_by_id(pool: &SqlitePool, id: &str) -> Result<Employee, String> {
    sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Employee {} not found: {}", id, error))
}

async fn fetch_task_by_id(pool: &SqlitePool, id: &str) -> Result<Task, String> {
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

async fn fetch_task_attachments(
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

async fn validate_reviewer_for_project(
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

async fn record_completion_metric(pool: &SqlitePool, task: &Task) -> Result<(), String> {
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
        let changes = sqlx::query_as::<_, CodexSessionFileChange>(
            "SELECT * FROM codex_session_file_changes WHERE session_id = $1 ORDER BY path ASC, created_at ASC",
        )
        .bind(&session.id)
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("Failed to fetch task execution file changes: {}", error))?;

        let capture_mode = if let Some(change) = changes.first() {
            change.capture_mode.clone()
        } else {
            let session_started_message = sqlx::query_scalar::<_, Option<String>>(
                "SELECT message FROM codex_session_events WHERE session_id = $1 AND event_type = 'session_started' ORDER BY created_at DESC LIMIT 1",
            )
            .bind(&session.id)
            .fetch_optional(&pool)
            .await
            .map_err(|error| format!("Failed to fetch session provider info: {}", error))?
            .flatten()
            .unwrap_or_default();

            if session_started_message.contains("通过 SDK 启动") {
                "sdk_event".to_string()
            } else {
                "git_fallback".to_string()
            }
        };

        items.push(TaskExecutionChangeHistoryItem {
            session,
            capture_mode,
            changes,
        });
    }

    Ok(items)
}

#[tauri::command]
pub async fn start_task_code_review(
    app: AppHandle,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    task_id: String,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &task_id).await?;
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
    let repo_path = project
        .repo_path
        .clone()
        .ok_or_else(|| "当前项目未配置仓库路径，无法审核代码".to_string())?;
    let review_context = collect_task_review_context(&repo_path)?;
    let review_prompt = build_task_review_prompt(&task, &project, &review_context);

    crate::codex::start_codex(
        app.clone(),
        state,
        reviewer.id.clone(),
        review_prompt,
        Some(reviewer.model.clone()),
        Some(reviewer.reasoning_effort.clone()),
        reviewer.system_prompt.clone(),
        Some(repo_path),
        Some(task.id.clone()),
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
pub async fn create_project<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateProject,
) -> Result<Project, String> {
    let pool = sqlite_pool(&app).await?;
    let project = Project {
        id: new_id(),
        name: payload.name.trim().to_string(),
        description: normalize_optional_text(payload.description.as_deref()),
        status: "active".to_string(),
        repo_path: validate_project_repo_path(payload.repo_path.as_deref())?,
        created_at: now_sqlite(),
        updated_at: now_sqlite(),
    };

    if project.name.is_empty() {
        return Err("项目名称不能为空".to_string());
    }

    sqlx::query(
        "INSERT INTO projects (id, name, description, status, repo_path, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&project.id)
    .bind(&project.name)
    .bind(&project.description)
    .bind(&project.status)
    .bind(&project.repo_path)
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
    if let Some(repo_path) = updates.repo_path {
        separated
            .push("repo_path = ")
            .push_bind_unseparated(match repo_path {
                Some(repo_path) => validate_project_repo_path(Some(&repo_path))?,
                None => None,
            });
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

    let task = Task {
        id: new_id(),
        title: payload.title.trim().to_string(),
        description: normalize_optional_text(payload.description.as_deref()),
        status: "todo".to_string(),
        priority: payload.priority.unwrap_or_else(|| "medium".to_string()),
        project_id: payload.project_id,
        assignee_id: normalize_optional_text(payload.assignee_id.as_deref()),
        reviewer_id: None,
        complexity: None,
        ai_suggestion: None,
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

    sqlx::query(
        "INSERT INTO tasks (id, title, description, status, priority, project_id, assignee_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(&task.id)
    .bind(&task.title)
    .bind(&task.description)
    .bind(&task.status)
    .bind(&task.priority)
    .bind(&task.project_id)
    .bind(&task.assignee_id)
    .bind(&task.created_at)
    .bind(&task.updated_at)
    .execute(&mut *tx)
    .await
    .map_err(|error| format!("Failed to create task: {}", error))?;

    let attachments = if let Some(source_paths) = payload.attachment_source_paths.as_ref() {
        let attachments = build_task_attachments_from_sources(&app, &task.id, source_paths, 1)?;
        if let Err(error) = insert_task_attachments(&mut tx, &attachments).await {
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

    fetch_task_by_id(&pool, &task.id).await
}

#[tauri::command]
pub async fn add_task_attachments<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    source_paths: Vec<String>,
) -> Result<Vec<TaskAttachment>, String> {
    let pool = sqlite_pool(&app).await?;
    fetch_task_by_id(&pool, &task_id).await?;

    if source_paths.is_empty() {
        return Ok(Vec::new());
    }

    let start_sort_order = resolve_next_task_attachment_sort_order(&pool, &task_id).await?;
    let attachments =
        build_task_attachments_from_sources(&app, &task_id, &source_paths, start_sort_order)?;

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
        tx.rollback().await.ok();
        return Err(error);
    }

    tx.commit()
        .await
        .map_err(|error| format!("Failed to commit attachment create: {}", error))?;

    fetch_task_attachments(&pool, &task_id).await
}

#[tauri::command]
pub async fn delete_task_attachment<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let attachment = fetch_task_attachment_by_id(&pool, &id).await?;
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
    use std::fs;

    use super::{
        ensure_statement_terminated, normalize_runtime_path_string, resolve_session_resume_state,
        sanitize_sql_backup_script, validate_project_repo_path, validate_runtime_working_dir,
    };

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
}
