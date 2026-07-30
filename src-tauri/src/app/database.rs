use super::*;

pub(crate) async fn fetch_database_migration_status(
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

fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sql_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub(crate) fn ensure_statement_terminated(sql: &str) -> String {
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

pub(crate) fn sanitize_sql_backup_script(script: &str) -> String {
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

async fn sync_local_sdk_notification<R: Runtime>(
    app: &AppHandle<R>,
    sdk_health: &crate::codex::SdkRuntimeHealth,
    task_sdk_enabled: bool,
    one_shot_sdk_enabled: bool,
    one_shot_preferred_provider: &str,
) {
    let sdk_expected = sdk_notification_expected(
        task_sdk_enabled,
        one_shot_sdk_enabled,
        one_shot_preferred_provider,
    );
    let sdk_unavailable = sdk_notification_unavailable(
        task_sdk_enabled,
        one_shot_sdk_enabled,
        one_shot_preferred_provider,
        &sdk_health.task_execution_effective_provider,
        &sdk_health.one_shot_effective_provider,
    );
    let dedupe_key = sdk_unavailable_dedupe_key("local");

    if sdk_unavailable {
        let mut draft = NotificationDraft::sticky(
            NOTIFICATION_TYPE_SDK_UNAVAILABLE,
            if sdk_health.node_available {
                NOTIFICATION_SEVERITY_WARNING
            } else {
                NOTIFICATION_SEVERITY_ERROR
            },
            "sdk_health",
            "本地 SDK 当前不可用",
            sdk_health.status_message.clone(),
        );
        draft.recommendation =
            Some("请前往设置页检查 Node、SDK 安装状态以及执行 provider 配置。".to_string());
        draft.action_label = Some("打开设置".to_string());
        draft.action_route = Some(settings_route("sdk", None));
        draft.related_object_type = Some("environment".to_string());
        draft.related_object_id = Some("local".to_string());
        draft.dedupe_key = Some(dedupe_key);

        let _ = ensure_sticky_notification(app, draft).await;
    } else if sdk_expected
        && resolve_sticky_notification(app, &dedupe_key, None)
            .await
            .ok()
            .flatten()
            .is_some()
    {
        let mut recovery = NotificationDraft::one_time(
            NOTIFICATION_TYPE_SDK_UNAVAILABLE,
            NOTIFICATION_SEVERITY_SUCCESS,
            "sdk_health",
            "本地 SDK 已恢复可用",
            "本地 SDK 健康检查恢复正常，任务执行将按当前设置继续使用 SDK。",
        );
        recovery.action_label = Some("查看设置".to_string());
        recovery.action_route = Some(settings_route("sdk", None));
        recovery.related_object_type = Some("environment".to_string());
        recovery.related_object_id = Some("local".to_string());
        let _ = publish_one_time_notification(app, recovery).await;
    } else if !sdk_expected {
        let _ = resolve_sticky_notification(app, &dedupe_key, None).await;
    }
}

fn sdk_notification_expected(
    task_sdk_enabled: bool,
    one_shot_sdk_enabled: bool,
    one_shot_preferred_provider: &str,
) -> bool {
    task_sdk_enabled || (one_shot_sdk_enabled && one_shot_preferred_provider == "codex")
}

fn sdk_notification_unavailable(
    task_sdk_enabled: bool,
    one_shot_sdk_enabled: bool,
    one_shot_preferred_provider: &str,
    task_execution_effective_provider: &str,
    one_shot_effective_provider: &str,
) -> bool {
    (task_sdk_enabled && task_execution_effective_provider != "sdk")
        || (one_shot_sdk_enabled
            && one_shot_preferred_provider == "codex"
            && one_shot_effective_provider != "sdk")
}

async fn resolve_local_one_shot_runtime<R: Runtime>(
    app: &AppHandle<R>,
    codex_settings: &crate::db::models::CodexSettings,
    codex_sdk_health: &crate::codex::SdkRuntimeHealth,
) -> (String, String) {
    match codex_settings.one_shot_preferred_provider.as_str() {
        "claude" => {
            let claude_settings = match crate::claude::load_claude_settings(app) {
                Ok(settings) => settings,
                Err(error) => {
                    return (
                        "unavailable".to_string(),
                        format!("读取 Claude 设置失败：{error}"),
                    );
                }
            };
            let claude_health =
                crate::claude::inspect_claude_sdk_runtime(app, &claude_settings).await;
            if !codex_settings.one_shot_sdk_enabled {
                if claude_health.cli_available {
                    (
                        "cli".to_string(),
                        "一次性 AI 未启用 Claude SDK，将使用 Claude CLI".to_string(),
                    )
                } else {
                    (
                        "unavailable".to_string(),
                        "一次性 AI 未启用 Claude SDK，且 Claude CLI 不可用".to_string(),
                    )
                }
            } else {
                let channel = if claude_health.effective_provider == "sdk" {
                    "sdk"
                } else if claude_health.cli_available {
                    "cli"
                } else {
                    "unavailable"
                };
                (channel.to_string(), claude_health.sdk_status_message)
            }
        }
        "opencode" => {
            if !codex_settings.one_shot_sdk_enabled {
                return (
                    "unavailable".to_string(),
                    "一次性 AI 未启用 OpenCode SDK，当前不可用".to_string(),
                );
            }
            let opencode_settings = match crate::opencode::load_opencode_settings(app) {
                Ok(settings) => settings,
                Err(error) => {
                    return (
                        "unavailable".to_string(),
                        format!("读取 OpenCode 设置失败：{error}"),
                    );
                }
            };
            let opencode_health =
                crate::opencode::inspect_opencode_sdk_runtime(app, &opencode_settings).await;
            let channel = if opencode_health.effective_provider == "sdk" {
                "sdk"
            } else {
                "unavailable"
            };
            (channel.to_string(), opencode_health.sdk_status_message)
        }
        "grok" => {
            let grok_settings = match crate::grok::load_grok_settings(app) {
                Ok(settings) => settings,
                Err(error) => {
                    return (
                        "unavailable".to_string(),
                        format!("读取 Grok 设置失败：{error}"),
                    );
                }
            };
            let grok_health = crate::grok::inspect_grok_runtime(app, &grok_settings).await;
            if grok_health.cli_available {
                ("cli".to_string(), grok_health.status_message)
            } else {
                ("unavailable".to_string(), grok_health.status_message)
            }
        }
        _ => (
            codex_sdk_health.one_shot_effective_provider.clone(),
            codex_sdk_health.status_message.clone(),
        ),
    }
}

fn emit_local_database_unavailable_notification<R: Runtime>(app: &AppHandle<R>, message: String) {
    let now = now_sqlite();
    emit_transient_notification(
        app,
        TransientNotification {
            id: transient_notification_id(&database_error_dedupe_key("local")),
            notification_type: NOTIFICATION_TYPE_DATABASE_ERROR.to_string(),
            severity: NOTIFICATION_SEVERITY_CRITICAL.to_string(),
            source_module: "database".to_string(),
            title: "数据库当前不可用".to_string(),
            message,
            recommendation: Some("请前往设置页检查数据库文件、迁移状态和读写权限。".to_string()),
            action_label: Some("打开设置".to_string()),
            action_route: Some(settings_route("database", None)),
            related_object_type: Some("environment".to_string()),
            related_object_id: Some("local".to_string()),
            project_id: None,
            task_id: None,
            ssh_config_id: None,
            delivery_mode: "sticky".to_string(),
            occurrence_count: 1,
            first_triggered_at: now.clone(),
            last_triggered_at: now,
            is_read: false,
            is_transient: true,
        },
    );
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
    sync_local_sdk_notification(
        &app,
        &sdk_health,
        codex_settings.task_sdk_enabled,
        codex_settings.one_shot_sdk_enabled,
        &codex_settings.one_shot_preferred_provider,
    )
    .await;

    let (one_shot_effective_channel, one_shot_status_message) =
        resolve_local_one_shot_runtime(&app, &codex_settings, &sdk_health).await;

    if !database_loaded {
        emit_local_database_unavailable_notification(
            &app,
            "当前数据库未能正常加载，通知中心会先以临时提醒模式提示异常。".to_string(),
        );
    }

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
        one_shot_preferred_provider: codex_settings.one_shot_preferred_provider.clone(),
        sdk_installed: sdk_health.sdk_installed,
        sdk_version: sdk_health.sdk_version,
        sdk_install_dir: codex_settings.sdk_install_dir.clone(),
        task_execution_effective_provider: sdk_health.task_execution_effective_provider,
        one_shot_effective_provider: codex_settings.one_shot_preferred_provider.clone(),
        one_shot_effective_channel,
        one_shot_status_message,
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
pub fn get_database_backup_scope() -> crate::db::models::DatabaseBackupScope {
    crate::db::models::DatabaseBackupScope {
        includes: vec![
            "SQLite 全库数据（项目/任务/员工/会话/活动日志/交付字段等）".to_string(),
            "已应用的数据库迁移版本记录".to_string(),
        ],
        excludes: vec![
            "任务附件文件目录（图片/文档本体）".to_string(),
            "密钥环中的 SSH 密码/私钥口令等敏感密钥".to_string(),
            "应用配置目录中的 AI Prompt 模板 JSON".to_string(),
            "应用配置目录中的 MCP 服务器 JSON".to_string(),
            "窗口尺寸等本地 UI 状态".to_string(),
        ],
        note: "当前「导出 SQL」仅覆盖数据库本体，不等于完整灾备。若需完整恢复，请额外备份配置目录与附件目录。".to_string(),
    }
}

#[tauri::command]
pub fn get_ai_provider_capabilities() -> Vec<crate::db::models::AiProviderCapabilities> {
    vec![
        crate::db::models::AiProviderCapabilities {
            provider: "codex".to_string(),
            label: "Codex".to_string(),
            start: true,
            stop: true,
            restart: true,
            send_input: true,
            resume: true,
            notes: "完整支持启动/停止/重启/发送输入/续聊。".to_string(),
        },
        crate::db::models::AiProviderCapabilities {
            provider: "claude".to_string(),
            label: "Claude".to_string(),
            start: true,
            stop: true,
            restart: false,
            send_input: false,
            resume: true,
            notes: "支持启动/停止/续聊；不支持独立 restart 与会话中 send_input。".to_string(),
        },
        crate::db::models::AiProviderCapabilities {
            provider: "opencode".to_string(),
            label: "OpenCode".to_string(),
            start: true,
            stop: true,
            restart: false,
            send_input: false,
            resume: true,
            notes: "支持启动/停止/续聊；不支持独立 restart 与会话中 send_input。".to_string(),
        },
        crate::db::models::AiProviderCapabilities {
            provider: "grok".to_string(),
            label: "Grok".to_string(),
            start: true,
            stop: true,
            restart: false,
            send_input: false,
            resume: true,
            notes: "支持启动/停止/续聊；不支持独立 restart 与会话中 send_input。".to_string(),
        },
    ]
}

async fn count_tasks_with_scope(
    pool: &SqlitePool,
    project_id: Option<&str>,
    environment_mode: Option<&str>,
    extra_predicate: &str,
) -> Result<i64, String> {
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT COUNT(*) FROM tasks t INNER JOIN projects p ON p.id = t.project_id WHERE t.deleted_at IS NULL AND p.deleted_at IS NULL",
    );
    if let Some(pid) = project_id.map(str::trim).filter(|v| !v.is_empty()) {
        builder.push(" AND t.project_id = ");
        builder.push_bind(pid);
    }
    match environment_mode {
        Some("ssh") => {
            builder.push(" AND p.project_type = ");
            builder.push_bind("ssh");
        }
        Some("local") => {
            builder.push(" AND p.project_type = ");
            builder.push_bind("local");
        }
        _ => {}
    }
    if !extra_predicate.is_empty() {
        builder.push(" ");
        builder.push(extra_predicate);
    }
    builder
        .build_query_scalar::<i64>()
        .fetch_one(pool)
        .await
        .map_err(|e| format!("统计任务失败: {e}"))
}

// ========== Activity log list + homepage dashboard stats ==========

const LIST_ACTIVITY_DEFAULT_LIMIT: i64 = 50;
const LIST_ACTIVITY_MAX_LIMIT: i64 = 500;

const GLOBAL_ACTIVITY_PREFIXES: &[&str] = &[
    "environment_",
    "global_",
    "notification_",
    "opencode_",
    "remote_",
    "ssh_",
];

const GLOBAL_ACTIVITY_ACTIONS: &[&str] = &[
    "employee_project_membership_conflict_migrated",
    "git_runtime_installed",
    "git_runtime_install_failed",
];

fn escape_sql_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn parse_activity_date_bound(date: &str, end_of_day: bool) -> Option<i64> {
    let trimmed = date.trim();
    if trimmed.is_empty() {
        return None;
    }
    let date_part = trimmed.split('T').next().unwrap_or(trimmed).trim();
    let naive_date = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()?;
    let naive_dt = if end_of_day {
        naive_date.and_hms_milli_opt(23, 59, 59, 999)?
    } else {
        naive_date.and_hms_opt(0, 0, 0)?
    };
    Some(naive_dt.and_utc().timestamp())
}

async fn resolve_visible_project_ids(
    pool: &SqlitePool,
    environment_mode: Option<&str>,
    selected_ssh_config_id: Option<&str>,
    explicit_project_ids: Option<&[String]>,
) -> Result<Vec<String>, String> {
    if let Some(ids) = explicit_project_ids {
        return Ok(ids.to_vec());
    }

    let mode = environment_mode.unwrap_or("local");
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT id FROM projects WHERE deleted_at IS NULL AND project_type = ",
    );
    builder.push_bind(mode);

    if mode == "ssh" {
        let Some(ssh_id) = selected_ssh_config_id
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            return Ok(Vec::new());
        };
        builder.push(" AND ssh_config_id = ");
        builder.push_bind(ssh_id);
    }

    builder
        .build_query_scalar::<String>()
        .fetch_all(pool)
        .await
        .map_err(|e| format!("解析可见项目失败: {e}"))
}

/// Push activity scope predicates onto a QueryBuilder that already has a WHERE clause started
/// (or is about to use AND). Returns false when the result set is intentionally empty.
fn push_activity_scope_conditions(
    builder: &mut QueryBuilder<'_, Sqlite>,
    visible_project_ids: &[String],
    environment_mode: Option<&str>,
    project_id: Option<&str>,
    has_prior_where: bool,
) -> bool {
    let join = if has_prior_where { " AND " } else { " WHERE " };

    if let Some(pid) = project_id.map(str::trim).filter(|v| !v.is_empty()) {
        if !visible_project_ids.iter().any(|id| id == pid) {
            builder.push(join);
            builder.push("1 = 0");
            return false;
        }
        builder.push(join);
        builder.push("a.project_id = ");
        builder.push_bind(pid.to_string());
        return true;
    }

    // No environment scoping requested → no scope filter (logStore simple path).
    if environment_mode.is_none() && visible_project_ids.is_empty() {
        return true;
    }

    let mode = environment_mode.unwrap_or("local");
    builder.push(join);
    builder.push("(");

    let mut wrote_any = false;
    if !visible_project_ids.is_empty() {
        builder.push("a.project_id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in visible_project_ids {
                separated.push_bind(id.clone());
            }
        }
        builder.push(")");
        wrote_any = true;
    }

    if wrote_any {
        builder.push(" OR ");
    }

    if mode == "ssh" {
        builder.push("(a.project_id IS NULL AND (");
        let mut first = true;
        for prefix in GLOBAL_ACTIVITY_PREFIXES {
            if !first {
                builder.push(" OR ");
            }
            first = false;
            builder.push("a.action LIKE ");
            builder.push_bind(format!("{}%", escape_sql_like(prefix)));
            builder.push(" ESCAPE '\\'");
        }
        if !GLOBAL_ACTIVITY_ACTIONS.is_empty() {
            builder.push(" OR a.action IN (");
            {
                let mut separated = builder.separated(", ");
                for action in GLOBAL_ACTIVITY_ACTIONS {
                    separated.push_bind(*action);
                }
            }
            builder.push(")");
        }
        builder.push("))");
    } else {
        builder.push("a.project_id IS NULL");
    }

    builder.push(")");
    true
}

fn push_activity_filter_conditions(
    builder: &mut QueryBuilder<'_, Sqlite>,
    payload: &crate::db::models::ListActivityLogsPayload,
    has_prior_where: bool,
) -> bool {
    let mut has_where = has_prior_where;

    let push_and = |builder: &mut QueryBuilder<'_, Sqlite>, has_where: &mut bool| {
        if *has_where {
            builder.push(" AND ");
        } else {
            builder.push(" WHERE ");
            *has_where = true;
        }
    };

    if let Some(task_id) = payload
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        push_and(builder, &mut has_where);
        builder.push("a.task_id = ");
        builder.push_bind(task_id.to_string());
    }

    if let Some(action) = payload
        .action
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        push_and(builder, &mut has_where);
        builder.push("a.action = ");
        builder.push_bind(action.to_string());
    }

    let keyword = payload
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_lowercase());

    if let Some(keyword) = keyword {
        let pattern = format!("%{}%", escape_sql_like(&keyword));
        push_and(builder, &mut has_where);
        builder.push("(");
        builder.push("LOWER(a.action) LIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" ESCAPE '\\'");
        builder.push(" OR LOWER(COALESCE(a.details, '')) LIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" ESCAPE '\\'");
        builder.push(" OR LOWER(COALESCE(p.name, '')) LIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" ESCAPE '\\'");
        builder.push(" OR LOWER(COALESCE(e.name, '')) LIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" ESCAPE '\\'");

        if let Some(matched) = payload.matched_actions.as_ref() {
            if !matched.is_empty() {
                builder.push(" OR a.action IN (");
                {
                    let mut separated = builder.separated(", ");
                    for action in matched {
                        separated.push_bind(action.clone());
                    }
                }
                builder.push(")");
            }
        }

        if let Some(statuses) = payload.matched_statuses.as_ref() {
            for status in statuses {
                builder.push(" OR a.details LIKE ");
                builder.push_bind(format!("%{}%", escape_sql_like(status)));
                builder.push(" ESCAPE '\\'");
            }
        }

        builder.push(")");
    }

    if let Some(start) = payload
        .start_date
        .as_deref()
        .and_then(|d| parse_activity_date_bound(d, false))
    {
        push_and(builder, &mut has_where);
        builder.push("CAST(strftime('%s', a.created_at) AS INTEGER) >= ");
        builder.push_bind(start);
    }

    if let Some(end) = payload
        .end_date
        .as_deref()
        .and_then(|d| parse_activity_date_bound(d, true))
    {
        push_and(builder, &mut has_where);
        builder.push("CAST(strftime('%s', a.created_at) AS INTEGER) <= ");
        builder.push_bind(end);
    }

    has_where
}

fn activity_date_range_invalid(payload: &crate::db::models::ListActivityLogsPayload) -> bool {
    let start = payload
        .start_date
        .as_deref()
        .and_then(|d| parse_activity_date_bound(d, false));
    let end = payload
        .end_date
        .as_deref()
        .and_then(|d| parse_activity_date_bound(d, true));
    matches!((start, end), (Some(s), Some(e)) if s > e)
}

pub(crate) async fn list_activity_logs_with_pool(
    pool: &SqlitePool,
    payload: &crate::db::models::ListActivityLogsPayload,
) -> Result<crate::db::models::ActivityLogPage, String> {
    use crate::db::models::{ActivityLogPage, ActivityLogRow};

    let limit = payload
        .limit
        .filter(|v| *v > 0)
        .unwrap_or(LIST_ACTIVITY_DEFAULT_LIMIT)
        .clamp(1, LIST_ACTIVITY_MAX_LIMIT);
    let offset = payload.offset.unwrap_or(0).max(0);
    let include_total = payload.include_total.unwrap_or(false);

    let environment_mode = payload
        .environment_mode
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let selected_ssh = payload
        .selected_ssh_config_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let project_id = payload
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    // Resolve visible projects when environment or explicit ids are provided,
    // or when a single project_id filter needs membership check.
    let needs_scope = environment_mode.is_some()
        || payload.project_ids.as_ref().is_some_and(|ids| !ids.is_empty())
        || project_id.is_some();

    let visible_project_ids = if needs_scope {
        resolve_visible_project_ids(
            pool,
            environment_mode,
            selected_ssh,
            payload.project_ids.as_deref(),
        )
        .await?
    } else {
        Vec::new()
    };

    let mut available_actions = Vec::new();
    if include_total {
        let mut actions_builder = QueryBuilder::<Sqlite>::new(
            "SELECT DISTINCT a.action FROM activity_logs a",
        );
        // Scope-only for available actions (ignore keyword/action/date filters).
        let env_for_scope = if needs_scope { environment_mode } else { None };
        let _ = push_activity_scope_conditions(
            &mut actions_builder,
            &visible_project_ids,
            env_for_scope,
            project_id,
            false,
        );
        let mut actions: Vec<String> = actions_builder
            .build_query_scalar::<String>()
            .fetch_all(pool)
            .await
            .map_err(|e| format!("获取活动类型失败: {e}"))?;
        actions.sort();
        available_actions = actions;
    }

    if activity_date_range_invalid(payload) {
        return Ok(ActivityLogPage {
            items: Vec::new(),
            total: 0,
            available_actions,
        });
    }

    let items_select = "SELECT a.id, a.employee_id, a.action, a.details, a.task_id, a.project_id, a.created_at, \
         e.name AS employee_name, p.name AS project_name \
         FROM activity_logs a \
         LEFT JOIN employees e ON a.employee_id = e.id \
         LEFT JOIN projects p ON a.project_id = p.id";
    let count_select = "SELECT COUNT(*) FROM activity_logs a \
         LEFT JOIN employees e ON a.employee_id = e.id \
         LEFT JOIN projects p ON a.project_id = p.id";

    let mut items_q = QueryBuilder::<Sqlite>::new(items_select);
    let mut has_where = false;
    if needs_scope {
        let _ = push_activity_scope_conditions(
            &mut items_q,
            &visible_project_ids,
            environment_mode,
            project_id,
            has_where,
        );
        has_where = true;
    }
    let _ = push_activity_filter_conditions(&mut items_q, payload, has_where);
    items_q.push(" ORDER BY a.created_at DESC, a.id DESC LIMIT ");
    items_q.push_bind(limit);
    items_q.push(" OFFSET ");
    items_q.push_bind(offset);

    let items: Vec<ActivityLogRow> = items_q
        .build_query_as::<ActivityLogRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| format!("获取活动日志失败: {e}"))?;

    let total = if include_total {
        let mut count_q = QueryBuilder::<Sqlite>::new(count_select);
        let mut has_where = false;
        if needs_scope {
            let _ = push_activity_scope_conditions(
                &mut count_q,
                &visible_project_ids,
                environment_mode,
                project_id,
                has_where,
            );
            has_where = true;
        }
        let _ = push_activity_filter_conditions(&mut count_q, payload, has_where);
        count_q
            .build_query_scalar::<i64>()
            .fetch_one(pool)
            .await
            .map_err(|e| format!("统计活动日志失败: {e}"))?
    } else {
        0
    };

    Ok(ActivityLogPage {
        items,
        total,
        available_actions,
    })
}

#[tauri::command]
pub async fn list_activity_logs<R: Runtime>(
    app: AppHandle<R>,
    payload: Option<crate::db::models::ListActivityLogsPayload>,
) -> Result<crate::db::models::ActivityLogPage, String> {
    let pool = sqlite_pool(&app).await?;
    let payload = payload.unwrap_or_default();
    list_activity_logs_with_pool(&pool, &payload).await
}

async fn resolve_scoped_project_ids_for_stats(
    pool: &SqlitePool,
    environment_mode: Option<&str>,
    selected_ssh_config_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<Vec<String>, String> {
    let mode = environment_mode.unwrap_or("local");
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT id FROM projects WHERE deleted_at IS NULL AND project_type = ",
    );
    builder.push_bind(mode);
    if mode == "ssh" {
        let Some(ssh_id) = selected_ssh_config_id
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            return Ok(Vec::new());
        };
        builder.push(" AND ssh_config_id = ");
        builder.push_bind(ssh_id);
    }
    let visible: Vec<String> = builder
        .build_query_scalar::<String>()
        .fetch_all(pool)
        .await
        .map_err(|e| format!("解析可见项目失败: {e}"))?;

    if let Some(pid) = project_id.map(str::trim).filter(|v| !v.is_empty()) {
        if visible.iter().any(|id| id == pid) {
            return Ok(vec![pid.to_string()]);
        }
        return Ok(Vec::new());
    }
    Ok(visible)
}

pub(crate) async fn get_dashboard_stats_with_pool(
    pool: &SqlitePool,
    payload: &crate::db::models::GetDashboardStatsPayload,
) -> Result<crate::db::models::DashboardStats, String> {
    use crate::db::models::DashboardStats;
    use std::collections::HashMap;

    let environment_mode = payload
        .environment_mode
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let selected_ssh = payload
        .selected_ssh_config_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let project_id = payload
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    let scoped_ids = resolve_scoped_project_ids_for_stats(
        pool,
        environment_mode,
        selected_ssh,
        project_id,
    )
    .await?;

    // Projects
    let (total_projects, active_projects) = if scoped_ids.is_empty() {
        (0_i64, 0_i64)
    } else {
        let mut count_builder = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END), 0) \
             FROM projects WHERE deleted_at IS NULL AND id IN (",
        );
        {
            let mut separated = count_builder.separated(", ");
            for id in &scoped_ids {
                separated.push_bind(id);
            }
        }
        count_builder.push(")");
        count_builder
            .build_query_as::<(i64, i64)>()
            .fetch_one(pool)
            .await
            .map_err(|e| format!("统计项目失败: {e}"))?
    };

    // Tasks by status
    let mut tasks_by_status: HashMap<String, i64> = HashMap::new();
    let (total_tasks, completed) = if scoped_ids.is_empty() {
        (0_i64, 0_i64)
    } else {
        let mut status_builder = QueryBuilder::<Sqlite>::new(
            "SELECT status, COUNT(*) FROM tasks WHERE deleted_at IS NULL AND project_id IN (",
        );
        {
            let mut separated = status_builder.separated(", ");
            for id in &scoped_ids {
                separated.push_bind(id);
            }
        }
        status_builder.push(") GROUP BY status");
        let rows: Vec<(String, i64)> = status_builder
            .build_query_as::<(String, i64)>()
            .fetch_all(pool)
            .await
            .map_err(|e| format!("统计任务失败: {e}"))?;
        let mut total = 0_i64;
        let mut completed = 0_i64;
        for (status, count) in rows {
            total += count;
            if status == "completed" {
                completed = count;
            }
            tasks_by_status.insert(status, count);
        }
        (total, completed)
    };

    let completion_rate = if total_tasks > 0 {
        ((completed as f64 / total_tasks as f64) * 100.0).round() as i64
    } else {
        0
    };

    // Employees: project_id in scoped OR (no project filter and employee unbound)
    let (total_employees, online_employees) = if scoped_ids.is_empty() && project_id.is_some() {
        (0_i64, 0_i64)
    } else if scoped_ids.is_empty() {
        // No visible projects and no single project filter — still count unbound employees only
        // when not filtering by project (matches frontend: employee.project_id ? scoped : !projectId)
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN status IN ('online', 'busy') THEN 1 ELSE 0 END), 0)
             FROM employees WHERE project_id IS NULL",
        )
        .fetch_one(pool)
        .await
        .map_err(|e| format!("统计员工失败: {e}"))?
    } else {
        let mut emp_builder = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN status IN ('online', 'busy') THEN 1 ELSE 0 END), 0)
             FROM employees WHERE ",
        );
        if project_id.is_some() {
            emp_builder.push("project_id IN (");
            {
                let mut separated = emp_builder.separated(", ");
                for id in &scoped_ids {
                    separated.push_bind(id);
                }
            }
            emp_builder.push(")");
        } else {
            emp_builder.push("(project_id IN (");
            {
                let mut separated = emp_builder.separated(", ");
                for id in &scoped_ids {
                    separated.push_bind(id);
                }
            }
            emp_builder.push(") OR project_id IS NULL)");
        }
        emp_builder
            .build_query_as::<(i64, i64)>()
            .fetch_one(pool)
            .await
            .map_err(|e| format!("统计员工失败: {e}"))?
    };

    // Notifications: all active (frontend did not scope by project)
    let (unread_notifications, high_severity_notifications) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT
            COALESCE(SUM(CASE WHEN is_read = 0 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN severity IN ('error', 'critical') THEN 1 ELSE 0 END), 0)
         FROM notifications WHERE state = 'active'",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("统计通知失败: {e}"))?;

    Ok(DashboardStats {
        total_projects,
        active_projects,
        total_tasks,
        tasks_by_status,
        total_employees,
        online_employees,
        completion_rate,
        unread_notifications,
        high_severity_notifications,
    })
}

#[tauri::command]
pub async fn get_dashboard_stats<R: Runtime>(
    app: AppHandle<R>,
    payload: Option<crate::db::models::GetDashboardStatsPayload>,
) -> Result<crate::db::models::DashboardStats, String> {
    let pool = sqlite_pool(&app).await?;
    let payload = payload.unwrap_or_default();
    get_dashboard_stats_with_pool(&pool, &payload).await
}

#[tauri::command]
pub async fn get_dashboard_report_summary<R: Runtime>(
    app: AppHandle<R>,
    project_id: Option<String>,
    environment_mode: Option<String>,
) -> Result<crate::db::models::DashboardReportSummary, String> {
    use crate::db::models::{
        DashboardReportSummary, DashboardTrendPoint, DashboardWorkloadItem,
    };

    let pool = sqlite_pool(&app).await?;
    let project_id = project_id.as_deref();
    let environment_mode = environment_mode.as_deref();

    let total_tasks =
        count_tasks_with_scope(&pool, project_id, environment_mode, "").await?;
    let completed_tasks = count_tasks_with_scope(
        &pool,
        project_id,
        environment_mode,
        "AND t.status = 'completed'",
    )
    .await?;
    let blocked_tasks = count_tasks_with_scope(
        &pool,
        project_id,
        environment_mode,
        "AND t.status = 'blocked'",
    )
    .await?;
    let in_progress_tasks = count_tasks_with_scope(
        &pool,
        project_id,
        environment_mode,
        "AND t.status = 'in_progress'",
    )
    .await?;
    let overdue_tasks = count_tasks_with_scope(
        &pool,
        project_id,
        environment_mode,
        "AND t.due_date IS NOT NULL AND t.due_date < date('now') AND t.status NOT IN ('completed', 'archived')",
    )
    .await?;

    let completion_rate = if total_tasks > 0 {
        (completed_tasks as f64 / total_tasks as f64) * 100.0
    } else {
        0.0
    };

    let mut weekly_completed = Vec::new();
    for days_ago in (0..7).rev() {
        let label: String = sqlx::query_scalar(&format!("SELECT date('now', '-{days_ago} day')"))
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|_| format!("d-{days_ago}"));
        let count = count_tasks_with_scope(
            &pool,
            project_id,
            environment_mode,
            &format!(
                "AND t.completed_at IS NOT NULL AND date(t.completed_at) = date('now', '-{days_ago} day')"
            ),
        )
        .await
        .unwrap_or(0);
        weekly_completed.push(DashboardTrendPoint {
            label: label.chars().skip(5).collect::<String>(),
            count,
        });
    }

    let workload_rows = sqlx::query_as::<_, (String, String, i64, i64)>(
        r#"
        SELECT e.id, e.name,
          COALESCE(SUM(CASE WHEN t.status IN ('todo','in_progress','review','blocked') THEN 1 ELSE 0 END), 0) AS active_tasks,
          COALESCE(SUM(CASE WHEN t.status = 'completed' THEN 1 ELSE 0 END), 0) AS completed_tasks
        FROM employees e
        LEFT JOIN tasks t ON t.assignee_id = e.id AND t.deleted_at IS NULL
        LEFT JOIN projects p ON p.id = t.project_id AND p.deleted_at IS NULL
        GROUP BY e.id, e.name
        ORDER BY active_tasks DESC, completed_tasks DESC
        LIMIT 12
        "#,
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("统计员工负载失败: {e}"))?;

    let employee_workload = workload_rows
        .into_iter()
        .map(|(employee_id, employee_name, active_tasks, completed_tasks)| DashboardWorkloadItem {
            employee_id,
            employee_name,
            active_tasks,
            completed_tasks,
        })
        .collect();

    Ok(DashboardReportSummary {
        total_tasks,
        completed_tasks,
        overdue_tasks,
        blocked_tasks,
        in_progress_tasks,
        completion_rate,
        weekly_completed,
        employee_workload,
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
