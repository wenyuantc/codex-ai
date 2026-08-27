use super::*;

fn sdk_notification_expected(
    task_sdk_enabled: bool,
    one_shot_sdk_enabled: bool,
    one_shot_preferred_provider: &str,
) -> bool {
    task_sdk_enabled || (one_shot_sdk_enabled && one_shot_preferred_provider == "codex")
}

pub(crate) fn sdk_notification_unavailable(
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

pub(crate) async fn ensure_ssh_config_exists(
    pool: &SqlitePool,
    ssh_config_id: &str,
) -> Result<(), String> {
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

pub(crate) fn redact_secret_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        "[REDACTED]".to_string()
    }
}

pub(crate) fn shell_escape_single_quoted(value: &str) -> String {
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

const SSH_MUX_DIR_NAME: &str = "codex-ai-ssh-mux";
const SSH_MUX_CONTROL_PERSIST: &str = "60s";

/// Sanitize known_hosts_mode for use inside ControlPath filenames.
fn sanitize_mux_mode(known_hosts_mode: &str) -> String {
    let trimmed = known_hosts_mode.trim();
    match trimmed {
        "off" | "strict" | "accept-new" => trimmed.to_string(),
        _ => {
            let sanitized: String = trimmed
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            if sanitized.is_empty() {
                "accept-new".to_string()
            } else {
                sanitized
            }
        }
    }
}

/// Control socket directory.
///
/// Unix domain sockets are limited to ~104 bytes on macOS (`sun_path`).
/// `std::env::temp_dir()` is often a long `/var/folders/.../T` path, so
/// `tmpdir/codex-ai-ssh-mux/cm-accept-new-%C` can exceed the limit (110+).
/// Prefer `/tmp` on Unix when the temp-dir candidate would be too long.
fn ssh_mux_dir() -> PathBuf {
    let from_temp = std::env::temp_dir().join(SSH_MUX_DIR_NAME);
    if cfg!(windows) {
        return from_temp;
    }

    // Worst-case expanded socket name under this dir (mode + 40-char %C hash).
    let worst_case = from_temp.join(format!("cm-accept-new-{}", "a".repeat(40)));
    if worst_case.to_string_lossy().len() <= 100 {
        from_temp
    } else {
        PathBuf::from("/tmp").join(SSH_MUX_DIR_NAME)
    }
}

/// Ensure the ControlPath directory exists (Unix short-connection path only).
fn ensure_ssh_mux_dir() -> Result<PathBuf, String> {
    let base_dir = ssh_mux_dir();
    fs::create_dir_all(&base_dir).map_err(|error| format!("创建 SSH 复用目录失败: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&base_dir, permissions)
            .map_err(|error| format!("设置 SSH 复用目录权限失败: {error}"))?;
    }
    Ok(base_dir)
}

/// Build SSH ControlMaster / ControlPath args for the given connection shape.
///
/// Platform is parameterized so table-driven tests can cover Windows and Unix
/// on a single host. Production callers use [`multiplex_args`].
fn multiplex_args_impl(
    is_windows: bool,
    allocate_tty: bool,
    known_hosts_mode: &str,
) -> Vec<String> {
    if is_windows {
        return vec![
            "-o".to_string(),
            "ControlMaster=no".to_string(),
            "-o".to_string(),
            "ControlPath=none".to_string(),
        ];
    }

    if allocate_tty {
        return vec!["-o".to_string(), "ControlMaster=no".to_string()];
    }

    let mode = sanitize_mux_mode(known_hosts_mode);
    let control_path = ssh_mux_dir().join(format!("cm-{mode}-%C"));
    vec![
        "-o".to_string(),
        "ControlMaster=auto".to_string(),
        "-o".to_string(),
        format!("ControlPath={}", control_path.display()),
        "-o".to_string(),
        format!("ControlPersist={SSH_MUX_CONTROL_PERSIST}"),
    ]
}

fn multiplex_args(allocate_tty: bool, known_hosts_mode: &str) -> Vec<String> {
    multiplex_args_impl(cfg!(windows), allocate_tty, known_hosts_mode)
}

/// Gracefully shut down residual SSH ControlMaster processes and remove sockets.
/// Best-effort and non-blocking for app quit — failures are logged, never fatal.
pub(crate) fn cleanup_ssh_mux_masters() {
    if cfg!(windows) {
        return;
    }

    let mux_dir = ssh_mux_dir();
    if !mux_dir.is_dir() {
        return;
    }

    let entries = match fs::read_dir(&mux_dir) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("读取 SSH 复用目录失败: {error}");
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        // Control sockets are neither regular files nor directories; skip only dirs.
        if path.is_dir() {
            continue;
        }

        let path_str = path.to_string_lossy();
        // Best-effort master shutdown. BatchMode/ConnectTimeout avoid hang on quit.
        let _ = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=2",
                "-o",
                &format!("ControlPath={path_str}"),
                "-O",
                "exit",
                "dummy",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        // ssh -O exit usually removes the socket; delete any leftover as fallback.
        if path.exists() {
            if let Err(error) = fs::remove_file(&path) {
                eprintln!("清理 SSH 复用 socket 失败: {}: {error}", path.display());
            }
        }
    }
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

    // Unix short connections need a 0700 ControlPath directory before multiplex args.
    if !cfg!(windows) && !allocate_tty {
        ensure_ssh_mux_dir()?;
    }
    for arg in multiplex_args(allocate_tty, ssh_config.known_hosts_mode.as_str()) {
        command.arg(arg);
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

/// Long-lived SSH stdio pipe for protocols such as MCP JSON-RPC.
///
/// Uses `allocate_tty=true` so Unix ControlMaster is disabled — a mux master
/// dying must not take the MCP session with it (see ssh-remote.md).
pub(crate) struct SshStdioProcess {
    pub child: tokio::process::Child,
    askpass_path: Option<PathBuf>,
}

impl SshStdioProcess {
    pub fn cleanup_askpass(&mut self) {
        if let Some(path) = self.askpass_path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

impl Drop for SshStdioProcess {
    fn drop(&mut self) {
        self.cleanup_askpass();
        let _ = self.child.start_kill();
    }
}

pub(crate) async fn spawn_ssh_stdio_command<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    remote_command: &str,
    require_password_probe: bool,
) -> Result<SshStdioProcess, String> {
    let (mut command, askpass_path) = build_ssh_command(
        app,
        ssh_config,
        Some(remote_command),
        require_password_probe,
        true,
    )
    .await?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::process_spawn::configure_tokio_command(&mut command);
    let child = command
        .spawn()
        .map_err(|error| format!("启动远程 SSH 长连接失败: {error}"))?;
    Ok(SshStdioProcess {
        child,
        askpass_path,
    })
}

pub(crate) fn remote_path_join(base: &str, leaf: &str) -> String {
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
        include_str!("../codex/sdk_bridge.mjs").as_bytes(),
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

fn build_settings_section_route(section: &str, ssh_config_id: Option<&str>) -> String {
    match ssh_config_id {
        Some(ssh_config_id) => format!("/settings?section={section}&sshConfigId={ssh_config_id}"),
        None => format!("/settings?section={section}"),
    }
}

async fn sync_database_notifications<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let pool = match sqlite_pool(app).await {
        Ok(pool) => pool,
        Err(error) => {
            let now = now_sqlite();
            emit_transient_notification(
                app,
                TransientNotification {
                    id: transient_notification_id(&database_error_dedupe_key("local")),
                    notification_type: NOTIFICATION_TYPE_DATABASE_ERROR.to_string(),
                    severity: NOTIFICATION_SEVERITY_CRITICAL.to_string(),
                    source_module: "database".to_string(),
                    title: "数据库连接异常".to_string(),
                    message: format!("应用数据库当前不可用：{error}"),
                    recommendation: Some(
                        "请前往系统设置检查数据库文件、权限或恢复备份。".to_string(),
                    ),
                    action_label: Some("打开数据库维护".to_string()),
                    action_route: Some(build_settings_section_route("database", None)),
                    related_object_type: Some("system".to_string()),
                    related_object_id: Some("database".to_string()),
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
            return Ok(());
        }
    };

    match fetch_database_migration_status(&pool).await {
        Ok(_) => {
            let _ = resolve_sticky_notification(
                app,
                "database_error:local",
                Some(
                    NotificationDraft::one_time(
                        "database_error",
                        "success",
                        "数据库",
                        "数据库连接已恢复",
                        "数据库连接与迁移状态已恢复正常。",
                    )
                    .with_recommendation("如之前执行过导入或恢复，请再做一次关键路径冒烟验证。")
                    .with_action(
                        "打开数据库维护",
                        build_settings_section_route("database", None),
                    )
                    .with_related_object("system", "database"),
                ),
            )
            .await?;
        }
        Err(error) => {
            let _ = ensure_sticky_notification(
                app,
                NotificationDraft::sticky(
                    "database_error",
                    "critical",
                    "数据库",
                    "数据库迁移状态异常",
                    format!("数据库已连接，但迁移状态读取失败：{error}"),
                )
                .with_dedupe_key("database_error:local")
                .with_recommendation("请先检查数据库完整性与迁移状态，再继续执行任务。")
                .with_action(
                    "打开数据库维护",
                    build_settings_section_route("database", None),
                )
                .with_related_object("system", "database"),
            )
            .await?;
        }
    }

    Ok(())
}

async fn sync_local_sdk_notifications<R: Runtime>(
    app: &AppHandle<R>,
    _pool: &SqlitePool,
) -> Result<(), String> {
    let settings = load_codex_settings(app)?;
    let sdk_health = inspect_sdk_runtime(app, &settings).await;
    let sdk_expected = sdk_notification_expected(
        settings.task_sdk_enabled,
        settings.one_shot_sdk_enabled,
        &settings.one_shot_preferred_provider,
    );
    let sdk_unavailable = sdk_notification_unavailable(
        settings.task_sdk_enabled,
        settings.one_shot_sdk_enabled,
        &settings.one_shot_preferred_provider,
        &sdk_health.task_execution_effective_provider,
        &sdk_health.one_shot_effective_provider,
    );
    let dedupe_key = sdk_unavailable_dedupe_key("local");

    if !sdk_expected {
        let _ = resolve_sticky_notification(app, &dedupe_key, None).await?;
        return Ok(());
    }

    if !sdk_unavailable {
        let _ = resolve_sticky_notification(
            app,
            &dedupe_key,
            Some(
                NotificationDraft::one_time(
                    "sdk_unavailable",
                    "success",
                    "SDK 健康检查",
                    "本地 SDK 已恢复",
                    "本地任务执行链路已恢复 SDK 可用状态。",
                )
                .with_recommendation("可以重新尝试任务执行或一次性 AI 调用。")
                .with_action("打开 SDK 设置", build_settings_section_route("sdk", None))
                .with_related_object("system", "local-sdk"),
            ),
        )
        .await?;
        return Ok(());
    }

    let _ = ensure_sticky_notification(
        app,
        NotificationDraft::sticky(
            "sdk_unavailable",
            "error",
            "SDK 健康检查",
            "本地 SDK 不可用",
            format!("本地 SDK 当前不可用：{}", sdk_health.status_message),
        )
        .with_dedupe_key(dedupe_key)
        .with_recommendation("请前往系统设置安装或修复 SDK，必要时临时关闭 SDK 优先开关。")
        .with_action("打开 SDK 设置", build_settings_section_route("sdk", None))
        .with_related_object("system", "local-sdk"),
    )
    .await?;

    Ok(())
}

async fn sync_ssh_notifications<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    ssh_config_id: Option<&str>,
) -> Result<(), String> {
    let Some(ssh_config_id) = ssh_config_id else {
        let _ = ensure_sticky_notification(
            app,
            NotificationDraft::sticky(
                "ssh_config_error",
                "warning",
                "SSH 配置",
                "SSH 配置缺失",
                "当前处于 SSH 模式，但还没有可用的 SSH 配置。",
            )
            .with_dedupe_key(ssh_missing_selection_dedupe_key())
            .with_recommendation("请先创建并选择 SSH 配置，再继续远程执行。")
            .with_action("打开 SSH 配置", build_settings_section_route("ssh", None))
            .with_related_object("system", "ssh"),
        )
        .await?;
        return Ok(());
    };

    let _ = resolve_sticky_notification(app, ssh_missing_selection_dedupe_key(), None).await?;

    let ssh_config = match fetch_ssh_config_record_by_id(pool, ssh_config_id).await {
        Ok(config) => config,
        Err(error) => {
            let dedupe_key = ssh_selected_config_dedupe_key(ssh_config_id);
            let _ = ensure_sticky_notification(
                app,
                NotificationDraft::sticky(
                    "ssh_config_error",
                    "error",
                    "SSH 配置",
                    "选中的 SSH 配置不可用",
                    format!("当前选中的 SSH 配置无法读取：{error}"),
                )
                .with_dedupe_key(dedupe_key)
                .with_recommendation("请重新选择或重建 SSH 配置。")
                .with_action(
                    "打开 SSH 配置",
                    build_settings_section_route("ssh", Some(ssh_config_id)),
                )
                .with_related_object("ssh_config", ssh_config_id),
            )
            .await?;
            return Ok(());
        }
    };

    let selected_config_dedupe_key = ssh_selected_config_dedupe_key(ssh_config_id);
    let _ = resolve_sticky_notification(app, &selected_config_dedupe_key, None).await?;

    let host_label = ssh_config_target_host_label(&ssh_config);
    let ssh_route = build_settings_section_route("ssh", Some(ssh_config_id));
    let probe_dedupe_key = ssh_password_probe_dedupe_key(ssh_config_id);
    let health_dedupe_key = ssh_health_check_dedupe_key(ssh_config_id);

    let password_probe_ok = ssh_config.auth_type != "password"
        || matches!(
            ssh_config.password_probe_status.as_deref(),
            Some("passed" | "available")
        );

    if password_probe_ok {
        let _ = resolve_sticky_notification(
            app,
            &probe_dedupe_key,
            Some(
                NotificationDraft::one_time(
                    "ssh_config_error",
                    "success",
                    "SSH 配置",
                    "SSH 密码认证校验已恢复",
                    format!("{host_label} 的密码认证校验已通过。"),
                )
                .with_recommendation("可以继续执行远程健康检查或远程任务。")
                .with_action("打开 SSH 配置", ssh_route.clone())
                .with_related_object("ssh_config", ssh_config_id),
            ),
        )
        .await?;
    } else {
        let _ = ensure_sticky_notification(
            app,
            NotificationDraft::sticky(
                "ssh_config_error",
                "warning",
                "SSH 配置",
                "SSH 配置认证异常",
                format!(
                    "{} 的密码认证尚未通过测试：{}",
                    host_label,
                    ssh_config
                        .password_probe_message
                        .clone()
                        .unwrap_or_else(|| "当前平台无法安全执行密码认证".to_string())
                ),
            )
            .with_dedupe_key(probe_dedupe_key)
            .with_recommendation("请先修复认证方式或改用密钥登录，再执行远程任务。")
            .with_action("打开 SSH 配置", ssh_route.clone())
            .with_related_object("ssh_config", ssh_config_id)
            .with_ssh_config_id(ssh_config_id),
        )
        .await?;
    }

    match inspect_remote_codex_runtime(
        app,
        &ssh_config,
        &load_remote_codex_settings(app, ssh_config_id)?,
    )
    .await
    {
        Ok(runtime) => {
            let _ = resolve_sticky_notification(
                app,
                &health_dedupe_key,
                Some(
                    NotificationDraft::one_time(
                        "ssh_config_error",
                        "success",
                        "SSH 配置",
                        "SSH 连接已恢复",
                        format!("{host_label} 的远程连通性与运行时校验已恢复正常。"),
                    )
                    .with_recommendation("可以重新尝试远程执行、校验和 SDK 安装。")
                    .with_action("打开 SSH 配置", ssh_route.clone())
                    .with_related_object("ssh_config", ssh_config_id)
                    .with_ssh_config_id(ssh_config_id),
                ),
            )
            .await?;

            let remote_settings = load_remote_codex_settings(app, ssh_config_id)?;
            let remote_sdk_expected = sdk_notification_expected(
                remote_settings.task_sdk_enabled,
                remote_settings.one_shot_sdk_enabled,
                &remote_settings.one_shot_preferred_provider,
            );
            let remote_sdk_unavailable = sdk_notification_unavailable(
                remote_settings.task_sdk_enabled,
                remote_settings.one_shot_sdk_enabled,
                &remote_settings.one_shot_preferred_provider,
                &runtime.task_execution_effective_provider,
                &runtime.one_shot_effective_provider,
            );
            let sdk_dedupe_key = sdk_unavailable_dedupe_key(&format!("ssh:{ssh_config_id}"));
            let sdk_route = build_settings_section_route("sdk", Some(ssh_config_id));

            if !remote_sdk_expected {
                let _ = resolve_sticky_notification(app, &sdk_dedupe_key, None).await?;
                return Ok(());
            }

            if !remote_sdk_unavailable {
                let _ = resolve_sticky_notification(
                    app,
                    &sdk_dedupe_key,
                    Some(
                        NotificationDraft::one_time(
                            "sdk_unavailable",
                            "success",
                            "SDK 健康检查",
                            "远程 SDK 已恢复",
                            format!("{host_label} 的远程 SDK 已恢复可用。"),
                        )
                        .with_recommendation("可以重新尝试远程任务执行或一次性 AI。")
                        .with_action("打开 SDK 设置", sdk_route)
                        .with_related_object("ssh_config", ssh_config_id)
                        .with_ssh_config_id(ssh_config_id),
                    ),
                )
                .await?;
            } else {
                let _ = ensure_sticky_notification(
                    app,
                    NotificationDraft::sticky(
                        "sdk_unavailable",
                        "error",
                        "SDK 健康检查",
                        "远程 SDK 不可用",
                        format!(
                            "{host_label} 的远程 SDK 当前不可用：{}",
                            runtime.status_message
                        ),
                    )
                    .with_dedupe_key(sdk_dedupe_key)
                    .with_recommendation("请在 SSH 目标上安装或修复 SDK，必要时暂时切回 exec。")
                    .with_action("打开 SDK 设置", sdk_route)
                    .with_related_object("ssh_config", ssh_config_id)
                    .with_ssh_config_id(ssh_config_id),
                )
                .await?;
            }
        }
        Err(error) => {
            let sdk_dedupe_key = sdk_unavailable_dedupe_key(&format!("ssh:{ssh_config_id}"));
            let _ = resolve_sticky_notification(app, &sdk_dedupe_key, None).await?;
            let _ = ensure_sticky_notification(
                app,
                NotificationDraft::sticky(
                    "ssh_config_error",
                    "error",
                    "SSH 配置",
                    "SSH 连接校验失败",
                    format!("{host_label} 的远程校验失败：{error}"),
                )
                .with_dedupe_key(health_dedupe_key)
                .with_recommendation("请检查主机地址、密钥/密码、known_hosts 和远程环境后重试。")
                .with_action("打开 SSH 配置", ssh_route)
                .with_related_object("ssh_config", ssh_config_id)
                .with_ssh_config_id(ssh_config_id),
            )
            .await?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn sync_system_notifications<R: Runtime>(
    app: AppHandle<R>,
    environment_mode: Option<String>,
    ssh_config_id: Option<String>,
) -> Result<(), String> {
    sync_database_notifications(&app).await?;

    let pool = match sqlite_pool(&app).await {
        Ok(pool) => pool,
        Err(_) => return Ok(()),
    };

    sync_local_sdk_notifications(&app, &pool).await?;

    if environment_mode.as_deref() == Some(EXECUTION_TARGET_SSH) {
        sync_ssh_notifications(&app, &pool, ssh_config_id.as_deref()).await?;
    } else {
        let _ = resolve_sticky_notification(&app, ssh_missing_selection_dedupe_key(), None).await?;
    }

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

    let dedupe_key = ssh_password_probe_dedupe_key(&ssh_config_id);
    if supported {
        if resolve_sticky_notification(&app, &dedupe_key, None)
            .await
            .ok()
            .flatten()
            .is_some()
        {
            let mut recovery = NotificationDraft::one_time(
                NOTIFICATION_TYPE_SSH_CONFIG_ERROR,
                NOTIFICATION_SEVERITY_SUCCESS,
                "ssh_config",
                format!("SSH 配置 {} 已恢复正常", current.name),
                "密码认证探测已通过，可以继续使用该 SSH 配置执行远程任务。",
            );
            recovery.action_label = Some("查看设置".to_string());
            recovery.action_route = Some(settings_route("ssh", Some(&ssh_config_id)));
            recovery.related_object_type = Some("ssh_config".to_string());
            recovery.related_object_id = Some(ssh_config_id.clone());
            recovery.ssh_config_id = Some(ssh_config_id.clone());
            let _ = publish_one_time_notification(&app, recovery).await;
        }
    } else {
        let mut draft = NotificationDraft::sticky(
            NOTIFICATION_TYPE_SSH_CONFIG_ERROR,
            NOTIFICATION_SEVERITY_ERROR,
            "ssh_config",
            format!("SSH 配置 {} 认证失败", current.name),
            message.clone(),
        );
        draft.recommendation =
            Some("请检查密码、用户名和主机连通性，然后在设置页重新探测。".to_string());
        draft.action_label = Some("查看 SSH 设置".to_string());
        draft.action_route = Some(settings_route("ssh", Some(&ssh_config_id)));
        draft.related_object_type = Some("ssh_config".to_string());
        draft.related_object_id = Some(ssh_config_id.clone());
        draft.ssh_config_id = Some(ssh_config_id.clone());
        draft.dedupe_key = Some(dedupe_key);
        let _ = ensure_sticky_notification(&app, draft).await;
    }

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

pub(crate) fn build_remote_codex_runtime_health(
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

fn resolve_remote_one_shot_runtime(
    remote_settings: &CodexSettings,
    runtime: &RemoteCodexRuntimeHealth,
) -> (String, String) {
    match remote_settings.one_shot_preferred_provider.as_str() {
        "claude" => {
            if remote_settings.one_shot_sdk_enabled {
                (
                    "cli".to_string(),
                    "SSH 模式下 Claude 一次性 AI 使用远端 Claude CLI".to_string(),
                )
            } else {
                (
                    "cli".to_string(),
                    "一次性 AI 未启用 Claude SDK，将使用远端 Claude CLI".to_string(),
                )
            }
        }
        "opencode" => (
            "sdk".to_string(),
            "SSH 模式下 OpenCode 一次性 AI 使用远端 Node + OpenCode SDK bridge".to_string(),
        ),
        "grok" => (
            "cli".to_string(),
            "SSH 模式下 Grok 一次性 AI 使用远端 Grok CLI（远端需已安装并 `grok login`）"
                .to_string(),
        ),
        "native" => (
            "channel".to_string(),
            "SSH 模式下内置 Agent 一次性 AI 通过本地 AI 渠道执行".to_string(),
        ),
        _ => (
            runtime.one_shot_effective_provider.clone(),
            runtime.status_message.clone(),
        ),
    }
}

#[tauri::command]
pub async fn validate_remote_codex_health<R: Runtime>(
    app: AppHandle<R>,
    ssh_config_id: String,
) -> Result<CodexHealthCheck, String> {
    let pool = sqlite_pool(&app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
    let remote_settings = load_remote_codex_settings(&app, &ssh_config_id)?;
    let ssh_target_label = ssh_config_target_host_label(&ssh_config);
    let ssh_issue_key = ssh_health_check_dedupe_key(&ssh_config_id);
    let runtime = match inspect_remote_codex_runtime(&app, &ssh_config, &remote_settings).await {
        Ok(runtime) => runtime,
        Err(error) => {
            let checked_at = now_sqlite();
            let _ = sqlx::query(
                "UPDATE ssh_configs SET last_checked_at = $2, last_check_status = 'failed', last_check_message = $3, updated_at = $4 WHERE id = $1",
            )
            .bind(&ssh_config_id)
            .bind(&checked_at)
            .bind(&error)
            .bind(now_sqlite())
            .execute(&pool)
            .await;

            let mut draft = NotificationDraft::sticky(
                NOTIFICATION_TYPE_SSH_CONFIG_ERROR,
                NOTIFICATION_SEVERITY_ERROR,
                "ssh_config",
                format!("SSH 主机 {} 连接异常", ssh_config.name),
                error.clone(),
            );
            draft.recommendation =
                Some("请检查主机地址、认证信息、网络连通性以及远端运行环境。".to_string());
            draft.action_label = Some("查看 SSH 设置".to_string());
            draft.action_route = Some(settings_route("ssh", Some(&ssh_config_id)));
            draft.related_object_type = Some("ssh_config".to_string());
            draft.related_object_id = Some(ssh_config_id.clone());
            draft.ssh_config_id = Some(ssh_config_id.clone());
            draft.dedupe_key = Some(ssh_issue_key);
            let _ = ensure_sticky_notification(&app, draft).await;
            return Err(error);
        }
    };
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

    if resolve_sticky_notification(&app, &ssh_issue_key, None)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        let mut recovery = NotificationDraft::one_time(
            NOTIFICATION_TYPE_SSH_CONFIG_ERROR,
            NOTIFICATION_SEVERITY_SUCCESS,
            "ssh_config",
            format!("SSH 主机 {} 已恢复正常", ssh_config.name),
            "远程健康检查已恢复成功，可以继续使用该 SSH 主机。",
        );
        recovery.action_label = Some("查看 SSH 设置".to_string());
        recovery.action_route = Some(settings_route("ssh", Some(&ssh_config_id)));
        recovery.related_object_type = Some("ssh_config".to_string());
        recovery.related_object_id = Some(ssh_config_id.clone());
        recovery.ssh_config_id = Some(ssh_config_id.clone());
        let _ = publish_one_time_notification(&app, recovery).await;
    }

    let sdk_issue_key = sdk_unavailable_dedupe_key(&format!("ssh:{ssh_config_id}"));
    let sdk_unavailable = sdk_notification_unavailable(
        remote_settings.task_sdk_enabled,
        remote_settings.one_shot_sdk_enabled,
        &remote_settings.one_shot_preferred_provider,
        &runtime.task_execution_effective_provider,
        &runtime.one_shot_effective_provider,
    );
    if sdk_unavailable {
        let mut draft = NotificationDraft::sticky(
            NOTIFICATION_TYPE_SDK_UNAVAILABLE,
            if runtime.codex_available {
                NOTIFICATION_SEVERITY_WARNING
            } else {
                NOTIFICATION_SEVERITY_ERROR
            },
            "sdk_health",
            format!("远程主机 {} 的 SDK 当前不可用", ssh_config.name),
            runtime.status_message.clone(),
        );
        draft.recommendation =
            Some("请检查远程 Node、SDK 安装状态以及该主机的执行 provider 配置。".to_string());
        draft.action_label = Some("查看远程设置".to_string());
        draft.action_route = Some(settings_route("sdk", Some(&ssh_config_id)));
        draft.related_object_type = Some("ssh_config".to_string());
        draft.related_object_id = Some(ssh_config_id.clone());
        draft.ssh_config_id = Some(ssh_config_id.clone());
        draft.dedupe_key = Some(sdk_issue_key.clone());
        let _ = ensure_sticky_notification(&app, draft).await;
    } else if resolve_sticky_notification(&app, &sdk_issue_key, None)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        let mut recovery = NotificationDraft::one_time(
            NOTIFICATION_TYPE_SDK_UNAVAILABLE,
            NOTIFICATION_SEVERITY_SUCCESS,
            "sdk_health",
            format!("远程主机 {} 的 SDK 已恢复可用", ssh_config.name),
            "远程 SDK 健康检查恢复正常，后续任务将按当前设置继续使用 SDK。",
        );
        recovery.action_label = Some("查看远程设置".to_string());
        recovery.action_route = Some(settings_route("sdk", Some(&ssh_config_id)));
        recovery.related_object_type = Some("ssh_config".to_string());
        recovery.related_object_id = Some(ssh_config_id.clone());
        recovery.ssh_config_id = Some(ssh_config_id.clone());
        let _ = publish_one_time_notification(&app, recovery).await;
    }

    let latest_registered_version = crate::db::migrations::latest_migration_version();
    let migration_status = fetch_database_migration_status(&pool).await.ok();
    let (one_shot_effective_channel, one_shot_status_message) =
        resolve_remote_one_shot_runtime(&remote_settings, &runtime);
    Ok(CodexHealthCheck {
        execution_target: EXECUTION_TARGET_SSH.to_string(),
        ssh_config_id: Some(ssh_config_id),
        target_host_label: Some(ssh_target_label),
        codex_available: runtime.codex_available,
        codex_version: runtime.codex_version,
        node_available: runtime.node_available,
        node_version: runtime.node_version,
        task_sdk_enabled: remote_settings.task_sdk_enabled,
        one_shot_sdk_enabled: remote_settings.one_shot_sdk_enabled,
        one_shot_preferred_provider: remote_settings.one_shot_preferred_provider.clone(),
        sdk_installed: runtime.sdk_installed,
        sdk_version: runtime.sdk_version,
        sdk_install_dir: remote_settings.sdk_install_dir,
        task_execution_effective_provider: runtime.task_execution_effective_provider,
        one_shot_effective_provider: remote_settings.one_shot_preferred_provider.clone(),
        one_shot_effective_channel,
        one_shot_status_message,
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
    let install_packages = SDK_INSTALL_PACKAGE_SPECS.join(" ");
    let remote_script = format!(
        "install_dir={install_dir}; mkdir -p \"$install_dir\" && cd \"$install_dir\" && \
if [ ! -f package.json ]; then printf '%s' {package_json} > package.json; fi && \
npm install --no-audit --no-fund --include=optional {install_packages} && \
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
pub async fn install_remote_grok_cli<R: Runtime>(
    app: AppHandle<R>,
    ssh_config_id: String,
) -> Result<crate::db::models::GrokCliInstallResult, String> {
    let pool = sqlite_pool(&app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
    let host_label = ssh_config_target_host_label(&ssh_config);
    let remote_script = "set -e; \
curl -fsSL https://x.ai/cli/install.sh | bash; \
if command -v grok >/dev/null 2>&1; then \
  printf 'CLI_VERSION=%s\\n' \"$(grok --version 2>/dev/null | head -n 1)\"; \
  printf 'CLI_PATH=%s\\n' \"$(command -v grok)\"; \
  exit 0; \
fi; \
for p in \"$HOME/.grok/bin/grok\" /usr/local/bin/grok; do \
  if [ -x \"$p\" ]; then \
    printf 'CLI_VERSION=%s\\n' \"$(\"$p\" --version 2>/dev/null | head -n 1)\"; \
    printf 'CLI_PATH=%s\\n' \"$p\"; \
    exit 0; \
  fi; \
done; \
echo 'CLI_MISSING=1'; exit 1";
    let output = execute_ssh_command(
        &app,
        &ssh_config,
        &build_remote_shell_command(remote_script, None),
        true,
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = [stderr.as_str(), stdout.as_str()]
            .into_iter()
            .find(|value| !value.is_empty())
            .unwrap_or("远程安装脚本执行失败");
        return Err(format!(
            "远程安装 Grok CLI 失败：{}",
            redact_secret_text(detail)
        ));
    }

    let values = parse_remote_key_value_output(&String::from_utf8_lossy(&output.stdout));
    if values.get("CLI_MISSING").is_some_and(|value| value == "1") {
        return Err("远程安装 Grok CLI 完成，但未找到 grok 可执行文件".to_string());
    }

    let cli_version = values
        .get("CLI_VERSION")
        .cloned()
        .filter(|value| !value.is_empty());
    let cli_path = values
        .get("CLI_PATH")
        .cloned()
        .filter(|value| !value.is_empty());
    let message = match cli_version.as_deref() {
        Some(version) => format!("远程 Grok CLI 安装完成，版本 {version}"),
        None => "远程 Grok CLI 安装完成".to_string(),
    };

    let _ = insert_activity_log(
        &pool,
        "remote_grok_cli_installed",
        &host_label,
        None,
        None,
        None,
    )
    .await;

    Ok(crate::db::models::GrokCliInstallResult {
        execution_target: EXECUTION_TARGET_SSH.to_string(),
        ssh_config_id: Some(ssh_config_id),
        target_host_label: Some(host_label),
        cli_available: true,
        cli_version,
        cli_path,
        message,
    })
}

#[tauri::command]
pub async fn validate_remote_grok_health<R: Runtime>(
    app: AppHandle<R>,
    ssh_config_id: String,
) -> Result<crate::db::models::RemoteGrokHealthCheck, String> {
    let pool = sqlite_pool(&app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
    let checked_at = now_sqlite();
    // 一次探测：version + models（用于登录态）。失败时 models 段落可能为空。
    let remote_command = build_remote_shell_command(
        "if command -v grok >/dev/null 2>&1; then \
            echo '__GROK_VERSION__'; grok --version; \
            echo '__GROK_MODELS__'; grok models 2>&1 || true; \
         else echo '__GROK_MISSING__'; exit 127; fi",
        None,
    );

    match execute_ssh_command(&app, &ssh_config, &remote_command, true).await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if output.status.success() && !stdout.contains("__GROK_MISSING__") {
                let version = stdout
                    .lines()
                    .skip_while(|line| line.trim() != "__GROK_VERSION__")
                    .skip(1)
                    .take_while(|line| line.trim() != "__GROK_MODELS__")
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        stdout
                            .lines()
                            .map(str::trim)
                            .find(|line| {
                                !line.is_empty()
                                    && *line != "__GROK_VERSION__"
                                    && *line != "__GROK_MODELS__"
                            })
                            .map(ToOwned::to_owned)
                    });

                let models_section = stdout
                    .lines()
                    .skip_while(|line| line.trim() != "__GROK_MODELS__")
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join("\n");
                let (_, auth_ok, _) = crate::grok::parse_grok_models_output(&models_section);
                // models 成功输出且未明确失败时，至少视为可能已登录；无 models 段落则未知。
                let auth_ok = if models_section.trim().is_empty() {
                    None
                } else {
                    auth_ok.or(Some(true))
                };

                let message = match (&version, auth_ok) {
                    (Some(version), Some(true)) => {
                        format!("远程 Grok CLI 可用（{version}），已登录")
                    }
                    (Some(version), Some(false)) => {
                        format!(
                            "远程 Grok CLI 可用（{version}），但未登录或登录失效。请在远程执行 `grok login`。"
                        )
                    }
                    (Some(version), None) => {
                        format!("远程 Grok CLI 可用（{version}）；登录态未知")
                    }
                    (None, Some(true)) => "远程 Grok CLI 可用，已登录".to_string(),
                    (None, Some(false)) => {
                        "远程 Grok CLI 可用，但未登录。请在远程执行 `grok login`。".to_string()
                    }
                    (None, None) => "远程 Grok CLI 可用；登录态未知".to_string(),
                };

                Ok(crate::db::models::RemoteGrokHealthCheck {
                    available: true,
                    version,
                    auth_ok,
                    message,
                    checked_at,
                })
            } else {
                let detail = if !stderr.is_empty() {
                    redact_secret_text(&stderr)
                } else if !stdout.is_empty() {
                    redact_secret_text(&stdout)
                } else {
                    "未找到 grok 可执行文件".to_string()
                };
                Ok(crate::db::models::RemoteGrokHealthCheck {
                    available: false,
                    version: None,
                    auth_ok: None,
                    message: format!(
                        "远程 Grok CLI 不可用：{detail}。请在远程安装 Grok Build CLI 并执行 `grok login`。"
                    ),
                    checked_at,
                })
            }
        }
        Err(error) => Err(format!("探测远程 Grok 健康状态失败: {error}")),
    }
}

// ========== Remote OpenCode SDK runtime ==========

const OPENCODE_SDK_RUNTIME_DIR_NAME: &str = "opencode-sdk-runtime";
const OPENCODE_SDK_BRIDGE_FILE_NAME: &str = "opencode_sdk_bridge.mjs";
const OPENCODE_SDK_PACKAGE_NAME: &str = "@opencode-ai/sdk";
const OPENCODE_SDK_RUNTIME_PACKAGE_JSON: &str =
    "{\"name\":\"codex-ai-opencode-sdk-runtime\",\"private\":true,\"type\":\"module\"}";
const OPENCODE_MINIMUM_NODE_MAJOR: u32 = 18;

pub(crate) fn default_remote_opencode_sdk_install_dir(ssh_config_id: &str) -> String {
    format!("~/.codex-ai/{OPENCODE_SDK_RUNTIME_DIR_NAME}/{ssh_config_id}")
}

pub(crate) fn remote_opencode_sdk_bridge_path(install_dir: &str) -> String {
    remote_path_join(install_dir, OPENCODE_SDK_BRIDGE_FILE_NAME)
}

pub(crate) fn build_remote_opencode_sdk_bridge_command(
    install_dir: &str,
    node_path_override: Option<&str>,
) -> String {
    let bridge_path = remote_opencode_sdk_bridge_path(install_dir);
    build_remote_shell_command(
        &format!(
            "install_dir={}; bridge_path={}; cd \"$install_dir\" && exec node \"$bridge_path\"",
            remote_shell_path_expression(install_dir),
            remote_shell_path_expression(&bridge_path),
        ),
        node_path_override,
    )
}

fn parse_node_major_version_for_remote(version: &str) -> Option<u32> {
    let normalized = version.trim().trim_start_matches('v');
    normalized
        .split('.')
        .next()
        .and_then(|segment| segment.parse::<u32>().ok())
}

pub(crate) async fn ensure_remote_opencode_sdk_runtime_layout<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
) -> Result<String, String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let install_dir = default_remote_opencode_sdk_install_dir(ssh_config_id);
    let bridge_path = remote_opencode_sdk_bridge_path(&install_dir);
    let init_script = format!(
        "install_dir={}; mkdir -p \"$install_dir\"; if [ ! -f \"$install_dir/package.json\" ]; then printf '%s' {} > \"$install_dir/package.json\"; fi",
        remote_shell_path_expression(&install_dir),
        shell_escape_single_quoted(OPENCODE_SDK_RUNTIME_PACKAGE_JSON),
    );
    let init_output = execute_ssh_command(
        app,
        &ssh_config,
        &build_remote_shell_command(&init_script, None),
        true,
    )
    .await?;
    if !init_output.status.success() {
        let stderr = String::from_utf8_lossy(&init_output.stderr)
            .trim()
            .to_string();
        return Err(if stderr.is_empty() {
            "初始化远程 OpenCode SDK 运行目录失败".to_string()
        } else {
            format!(
                "初始化远程 OpenCode SDK 运行目录失败：{}",
                redact_secret_text(&stderr)
            )
        });
    }

    let bridge_output = execute_ssh_command_with_input(
        app,
        &ssh_config,
        &build_remote_shell_command(
            &format!("cat > {}", remote_shell_path_expression(&bridge_path)),
            None,
        ),
        include_str!("../opencode/opencode_sdk_bridge.mjs").as_bytes(),
        true,
    )
    .await?;
    if !bridge_output.status.success() {
        let stderr = String::from_utf8_lossy(&bridge_output.stderr)
            .trim()
            .to_string();
        return Err(if stderr.is_empty() {
            "写入远程 OpenCode SDK bridge 脚本失败".to_string()
        } else {
            format!(
                "写入远程 OpenCode SDK bridge 脚本失败：{}",
                redact_secret_text(&stderr)
            )
        });
    }

    Ok(install_dir)
}

pub(crate) async fn inspect_remote_opencode_runtime<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config: &SshConfigRecord,
    install_dir: &str,
    node_path_override: Option<&str>,
) -> Result<crate::db::models::RemoteOpenCodeHealthCheck, String> {
    let sdk_install_dir_expr = remote_shell_path_expression(install_dir);
    let bridge_path = remote_opencode_sdk_bridge_path(install_dir);
    let bridge_path_expr = remote_shell_path_expression(&bridge_path);
    let remote_script = format!(
        "sdk_install_dir={sdk_install_dir_expr}; \
bridge_path={bridge_path_expr}; \
node_output=$(node --version 2>/dev/null); node_status=$?; \
sdk_pkg=\"$sdk_install_dir\"/node_modules/{OPENCODE_SDK_PACKAGE_NAME}/package.json; \
if [ -f \"$sdk_pkg\" ] && [ -f \"$bridge_path\" ]; then \
  sdk_installed=1; \
  sdk_version=$(sed -n 's/.*\"version\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p' \"$sdk_pkg\" | head -n 1); \
else \
  sdk_installed=0; sdk_version=''; \
fi; \
printf 'NODE_STATUS=%s\\nNODE_VERSION=%s\\nSDK_INSTALLED=%s\\nSDK_VERSION=%s\\n' \"$node_status\" \"$node_output\" \"$sdk_installed\" \"$sdk_version\""
    );
    let output = execute_ssh_command(
        app,
        ssh_config,
        &build_remote_shell_command(&remote_script, node_path_override),
        true,
    )
    .await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let redacted_stderr = redact_secret_text(&stderr);
    let values = parse_remote_key_value_output(&stdout);

    let node_available = values
        .get("NODE_STATUS")
        .map(|value| value == "0")
        .unwrap_or(false);
    let node_version = values
        .get("NODE_VERSION")
        .cloned()
        .filter(|value| !value.trim().is_empty());
    let sdk_installed = values
        .get("SDK_INSTALLED")
        .map(|value| value == "1")
        .unwrap_or(false);
    let sdk_version = values
        .get("SDK_VERSION")
        .cloned()
        .filter(|value| !value.trim().is_empty());
    let node_supported = node_version
        .as_deref()
        .and_then(parse_node_major_version_for_remote)
        .map(|major| major >= OPENCODE_MINIMUM_NODE_MAJOR)
        .unwrap_or(false);
    let available = node_available && node_supported && sdk_installed;

    let message = if !node_available {
        if redacted_stderr.is_empty() {
            "远程 Node 不可用，请先在远端安装 Node.js 18+".to_string()
        } else {
            format!("远程 Node 不可用：{redacted_stderr}")
        }
    } else if let Some(version) = node_version.as_deref() {
        match parse_node_major_version_for_remote(version) {
            Some(major) if major < OPENCODE_MINIMUM_NODE_MAJOR => format!(
                "远程 Node 版本过低（当前 {major}），OpenCode SDK 需要 Node.js {OPENCODE_MINIMUM_NODE_MAJOR}+"
            ),
            _ if !sdk_installed => {
                "远程 OpenCode SDK 未安装，请先安装远程 OpenCode SDK".to_string()
            }
            _ => format!("远程 OpenCode SDK 已就绪（Node {version}）"),
        }
    } else {
        "远程 OpenCode SDK 状态未知".to_string()
    };

    Ok(crate::db::models::RemoteOpenCodeHealthCheck {
        available,
        node_available,
        node_version,
        sdk_installed,
        sdk_version,
        sdk_install_dir: install_dir.to_string(),
        message,
        checked_at: now_sqlite(),
    })
}

#[tauri::command]
pub async fn validate_remote_opencode_health<R: Runtime>(
    app: AppHandle<R>,
    ssh_config_id: String,
) -> Result<crate::db::models::RemoteOpenCodeHealthCheck, String> {
    let pool = sqlite_pool(&app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
    let install_dir = default_remote_opencode_sdk_install_dir(&ssh_config_id);
    let health = inspect_remote_opencode_runtime(&app, &ssh_config, &install_dir, None).await?;
    let _ = insert_activity_log(
        &pool,
        "remote_opencode_validated",
        &ssh_config_target_host_label(&ssh_config),
        None,
        None,
        None,
    )
    .await;
    Ok(health)
}

#[tauri::command]
pub async fn install_remote_opencode_sdk<R: Runtime>(
    app: AppHandle<R>,
    ssh_config_id: String,
) -> Result<crate::db::models::RemoteOpenCodeSdkInstallResult, String> {
    let pool = sqlite_pool(&app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, &ssh_config_id).await?;
    let install_dir = ensure_remote_opencode_sdk_runtime_layout(&app, &ssh_config_id).await?;
    let install_dir_expr = remote_shell_path_expression(&install_dir);
    let package_json = shell_escape_single_quoted(OPENCODE_SDK_RUNTIME_PACKAGE_JSON);
    let remote_script = format!(
        "install_dir={install_dir_expr}; mkdir -p \"$install_dir\" && cd \"$install_dir\" && \
if [ ! -f package.json ]; then printf '%s' {package_json} > package.json; fi && \
npm install --no-audit --no-fund {OPENCODE_SDK_PACKAGE_NAME}@latest && \
sdk_pkg=\"$install_dir\"/node_modules/{OPENCODE_SDK_PACKAGE_NAME}/package.json && \
sdk_version=$(sed -n 's/.*\"version\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p' \"$sdk_pkg\" | head -n 1) && \
node_version=$(node --version 2>/dev/null || true) && \
printf 'SDK_VERSION=%s\\nNODE_VERSION=%s\\n' \"$sdk_version\" \"$node_version\""
    );
    let output = execute_ssh_command(
        &app,
        &ssh_config,
        &build_remote_shell_command(&remote_script, None),
        true,
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "远程安装 OpenCode SDK 失败".to_string()
        } else {
            format!(
                "远程安装 OpenCode SDK 失败：{}",
                redact_secret_text(&stderr)
            )
        });
    }

    let values = parse_remote_key_value_output(&String::from_utf8_lossy(&output.stdout));
    let result = crate::db::models::RemoteOpenCodeSdkInstallResult {
        execution_target: EXECUTION_TARGET_SSH.to_string(),
        ssh_config_id: Some(ssh_config_id.clone()),
        target_host_label: Some(ssh_config_target_host_label(&ssh_config)),
        sdk_installed: true,
        sdk_version: values
            .get("SDK_VERSION")
            .cloned()
            .filter(|value| !value.is_empty()),
        install_dir,
        node_version: values
            .get("NODE_VERSION")
            .cloned()
            .filter(|value| !value.is_empty()),
        message: "远程 OpenCode SDK 安装完成".to_string(),
    };
    let _ = insert_activity_log(
        &pool,
        "remote_opencode_sdk_installed",
        &ssh_config_target_host_label(&ssh_config),
        None,
        None,
        None,
    )
    .await;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{
        build_remote_opencode_sdk_bridge_command, default_remote_opencode_sdk_install_dir,
        multiplex_args_impl, remote_opencode_sdk_bridge_path, remote_shell_path_expression,
        sanitize_mux_mode, ssh_mux_dir, SSH_MUX_CONTROL_PERSIST,
    };

    #[test]
    fn sanitize_mux_mode_keeps_known_values() {
        assert_eq!(sanitize_mux_mode("off"), "off");
        assert_eq!(sanitize_mux_mode("strict"), "strict");
        assert_eq!(sanitize_mux_mode("accept-new"), "accept-new");
        assert_eq!(sanitize_mux_mode("  off  "), "off");
    }

    #[test]
    fn sanitize_mux_mode_replaces_unsafe_chars() {
        assert_eq!(sanitize_mux_mode("weird/mode"), "weird_mode");
        assert_eq!(sanitize_mux_mode(""), "accept-new");
        assert_eq!(sanitize_mux_mode("   "), "accept-new");
    }

    fn assert_windows_args(args: &[String]) {
        assert_eq!(
            args,
            &[
                "-o".to_string(),
                "ControlMaster=no".to_string(),
                "-o".to_string(),
                "ControlPath=none".to_string(),
            ]
        );
    }

    fn assert_unix_long_args(args: &[String]) {
        assert_eq!(args, &["-o".to_string(), "ControlMaster=no".to_string(),]);
    }

    fn assert_unix_short_args(args: &[String], mode: &str) {
        let expected_path = ssh_mux_dir().join(format!("cm-{mode}-%C"));
        assert_eq!(args.len(), 6);
        assert_eq!(args[0], "-o");
        assert_eq!(args[1], "ControlMaster=auto");
        assert_eq!(args[2], "-o");
        assert_eq!(args[3], format!("ControlPath={}", expected_path.display()));
        assert_eq!(args[4], "-o");
        assert_eq!(args[5], format!("ControlPersist={SSH_MUX_CONTROL_PERSIST}"));
        assert!(
            args[3].contains(&format!("cm-{mode}-%C")),
            "ControlPath should include mode prefix cm-{mode}-%C, got {}",
            args[3]
        );
    }

    // 12-case matrix: platform × connection length × known_hosts_mode
    // Uses multiplex_args_impl so both Windows and Unix paths are testable on one host.

    #[test]
    fn multiplex_windows_short_off() {
        assert_windows_args(&multiplex_args_impl(true, false, "off"));
    }

    #[test]
    fn multiplex_windows_short_strict() {
        assert_windows_args(&multiplex_args_impl(true, false, "strict"));
    }

    #[test]
    fn multiplex_windows_short_accept_new() {
        assert_windows_args(&multiplex_args_impl(true, false, "accept-new"));
    }

    #[test]
    fn multiplex_windows_long_off() {
        assert_windows_args(&multiplex_args_impl(true, true, "off"));
    }

    #[test]
    fn multiplex_windows_long_strict() {
        assert_windows_args(&multiplex_args_impl(true, true, "strict"));
    }

    #[test]
    fn multiplex_windows_long_accept_new() {
        assert_windows_args(&multiplex_args_impl(true, true, "accept-new"));
    }

    #[test]
    fn multiplex_unix_long_off() {
        assert_unix_long_args(&multiplex_args_impl(false, true, "off"));
    }

    #[test]
    fn multiplex_unix_long_strict() {
        assert_unix_long_args(&multiplex_args_impl(false, true, "strict"));
    }

    #[test]
    fn multiplex_unix_long_accept_new() {
        assert_unix_long_args(&multiplex_args_impl(false, true, "accept-new"));
    }

    #[test]
    fn multiplex_unix_short_off() {
        assert_unix_short_args(&multiplex_args_impl(false, false, "off"), "off");
    }

    #[test]
    fn multiplex_unix_short_strict() {
        assert_unix_short_args(&multiplex_args_impl(false, false, "strict"), "strict");
    }

    #[test]
    fn multiplex_unix_short_accept_new() {
        assert_unix_short_args(
            &multiplex_args_impl(false, false, "accept-new"),
            "accept-new",
        );
    }

    #[test]
    fn multiplex_unix_short_modes_use_distinct_control_paths() {
        let off = multiplex_args_impl(false, false, "off");
        let strict = multiplex_args_impl(false, false, "strict");
        let accept = multiplex_args_impl(false, false, "accept-new");
        let off_path = &off[3];
        let strict_path = &strict[3];
        let accept_path = &accept[3];
        assert_ne!(off_path, strict_path);
        assert_ne!(off_path, accept_path);
        assert_ne!(strict_path, accept_path);
    }

    #[test]
    fn unix_control_path_stays_within_mac_sun_path_limit() {
        // macOS AF_UNIX sun_path is typically 104 bytes. Keep margin for safety.
        let args = multiplex_args_impl(false, false, "accept-new");
        let control_path = args[3]
            .strip_prefix("ControlPath=")
            .expect("ControlPath option");
        // %C expands to 40 hex chars — substitute before measuring.
        let expanded = control_path.replace("%C", &"a".repeat(40));
        assert!(
            expanded.len() <= 100,
            "ControlPath too long for macOS sockets ({}): {expanded}",
            expanded.len()
        );
    }

    #[test]
    fn remote_opencode_install_dir_is_independent_of_codex() {
        let install_dir = default_remote_opencode_sdk_install_dir("ssh-1");
        assert_eq!(install_dir, "~/.codex-ai/opencode-sdk-runtime/ssh-1");
        assert!(!install_dir.contains("codex-sdk-runtime"));
        assert_eq!(
            remote_opencode_sdk_bridge_path(&install_dir),
            "~/.codex-ai/opencode-sdk-runtime/ssh-1/opencode_sdk_bridge.mjs"
        );
    }

    #[test]
    fn remote_opencode_sdk_bridge_command_expands_home_and_spaces() {
        let with_home = build_remote_opencode_sdk_bridge_command(
            "~/.codex-ai/opencode-sdk-runtime/ssh-1",
            Some("~/.nvm/versions/node/v22.0.0/bin/node"),
        );
        assert!(with_home.contains("install_dir=\"$HOME/.codex-ai/opencode-sdk-runtime/ssh-1\""));
        assert!(with_home.contains(
            "bridge_path=\"$HOME/.codex-ai/opencode-sdk-runtime/ssh-1/opencode_sdk_bridge.mjs\""
        ));
        assert!(with_home.contains("cd \"$install_dir\" && exec node \"$bridge_path\""));

        let with_spaces =
            build_remote_opencode_sdk_bridge_command("~/Open Code Runtime/ssh id", None);
        assert!(with_spaces
            .contains(remote_shell_path_expression("~/Open Code Runtime/ssh id").as_str()));
        assert!(with_spaces.contains(
            remote_shell_path_expression("~/Open Code Runtime/ssh id/opencode_sdk_bridge.mjs")
                .as_str()
        ));
        assert!(with_spaces.contains("cd \"$install_dir\" && exec node \"$bridge_path\""));
    }
}
