use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{insert_activity_log, new_id, now_sqlite, sqlite_pool};
use crate::db::models::{
    McpEnvVar, McpServerConfig, McpServersDocument, SetTaskMcpBindingPayload, TaskMcpBindingMode,
    TaskMcpBindingView, UpdateMcpServersPayload,
};

const MCP_SERVERS_FILE_NAME: &str = "mcp-servers.json";

fn app_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法读取应用配置目录: {error}"))
}

fn mcp_servers_file_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join(MCP_SERVERS_FILE_NAME))
}

pub fn default_mcp_servers() -> McpServersDocument {
    McpServersDocument {
        servers: vec![McpServerConfig {
            id: "example-filesystem".to_string(),
            name: "示例：Filesystem（请按需启用）".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                "/tmp".to_string(),
            ],
            env: vec![],
            enabled: false,
            notes: Some(
                "这是占位示例。启用前请确认本机已安装 Node/npx，并按实际仓库路径修改 args。"
                    .to_string(),
            ),
        }],
    }
}

fn normalize_server(mut server: McpServerConfig) -> Result<McpServerConfig, String> {
    server.name = server.name.trim().to_string();
    server.command = server.command.trim().to_string();
    if server.name.is_empty() {
        return Err("MCP 服务器名称不能为空".to_string());
    }
    if server.command.is_empty() {
        return Err("MCP 启动命令不能为空".to_string());
    }
    if server.id.trim().is_empty() {
        server.id = new_id();
    }
    server.args = server
        .args
        .into_iter()
        .map(|arg| arg.trim().to_string())
        .filter(|arg| !arg.is_empty())
        .collect();
    server.env = server
        .env
        .into_iter()
        .filter_map(|env| {
            let key = env.key.trim().to_string();
            if key.is_empty() {
                None
            } else {
                Some(McpEnvVar {
                    key,
                    value: env.value,
                })
            }
        })
        .collect();
    if let Some(notes) = server.notes.as_mut() {
        let trimmed = notes.trim();
        if trimmed.is_empty() {
            server.notes = None;
        } else {
            *notes = trimmed.to_string();
        }
    }
    Ok(server)
}

fn load_document_from_disk(path: &PathBuf) -> Result<McpServersDocument, String> {
    if !path.exists() {
        return Ok(default_mcp_servers());
    }
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("读取 MCP 配置失败 {}: {error}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(default_mcp_servers());
    }
    serde_json::from_str(&raw).map_err(|error| format!("解析 MCP 配置失败: {error}"))
}

fn write_document(path: &PathBuf, document: &McpServersDocument) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败: {error}"))?;
    }
    let json = serde_json::to_string_pretty(document)
        .map_err(|error| format!("序列化 MCP 配置失败: {error}"))?;
    fs::write(path, json).map_err(|error| format!("写入 MCP 配置失败: {error}"))
}

pub async fn load_mcp_document<R: Runtime>(app: &AppHandle<R>) -> Result<McpServersDocument, String> {
    let path = mcp_servers_file_path(app)?;
    load_document_from_disk(&path)
}

/// Resolved effective MCP set for a session.
#[derive(Debug, Clone)]
pub struct ResolvedMcp {
    pub mode: TaskMcpBindingMode,
    /// Resolved id list (same order as `servers` for override; enabled ids for inherit).
    #[allow(dead_code)]
    pub server_ids: Vec<String>,
    pub servers: Vec<McpServerConfig>,
}

/// Pure resolver: `binding_json` is the raw `tasks.mcp_server_ids` column.
/// - `None` / missing → inherit global `enabled=true`
/// - `Some("[]")` / JSON array → override (may be empty)
pub fn resolve_effective_mcp_servers(
    document: &McpServersDocument,
    binding_json: Option<&str>,
) -> Result<ResolvedMcp, String> {
    let by_id: HashMap<&str, &McpServerConfig> = document
        .servers
        .iter()
        .map(|server| (server.id.as_str(), server))
        .collect();

    match binding_json.map(str::trim).filter(|value| !value.is_empty()) {
        None => {
            let servers: Vec<McpServerConfig> = document
                .servers
                .iter()
                .filter(|server| server.enabled)
                .cloned()
                .collect();
            let server_ids = servers.iter().map(|server| server.id.clone()).collect();
            Ok(ResolvedMcp {
                mode: TaskMcpBindingMode::Inherit,
                server_ids,
                servers,
            })
        }
        Some(raw) => {
            let ids: Vec<String> = serde_json::from_str(raw)
                .map_err(|error| format!("任务 MCP 绑定 JSON 无效: {error}"))?;
            let mut servers = Vec::with_capacity(ids.len());
            let mut server_ids = Vec::with_capacity(ids.len());
            let mut missing = Vec::new();
            for id in ids {
                let id = id.trim().to_string();
                if id.is_empty() {
                    continue;
                }
                match by_id.get(id.as_str()) {
                    Some(server) => {
                        server_ids.push(id);
                        servers.push((*server).clone());
                    }
                    None => missing.push(id),
                }
            }
            if !missing.is_empty() {
                return Err(format!(
                    "任务 MCP 绑定包含未知服务器 id: {}",
                    missing.join(", ")
                ));
            }
            Ok(ResolvedMcp {
                mode: TaskMcpBindingMode::Override,
                server_ids,
                servers,
            })
        }
    }
}

/// Soft resolve for session launch: unknown ids are skipped with a note, never fail the session.
pub fn resolve_effective_mcp_servers_lenient(
    document: &McpServersDocument,
    binding_json: Option<&str>,
) -> ResolvedMcp {
    match resolve_effective_mcp_servers(document, binding_json) {
        Ok(resolved) => resolved,
        Err(_) => {
            // Re-parse override and skip missing ids instead of failing.
            let by_id: HashMap<&str, &McpServerConfig> = document
                .servers
                .iter()
                .map(|server| (server.id.as_str(), server))
                .collect();
            let ids: Vec<String> = binding_json
                .and_then(|raw| serde_json::from_str(raw).ok())
                .unwrap_or_default();
            let mut servers = Vec::new();
            let mut server_ids = Vec::new();
            for id in ids {
                let id = id.trim().to_string();
                if let Some(server) = by_id.get(id.as_str()) {
                    server_ids.push(id);
                    servers.push((*server).clone());
                }
            }
            ResolvedMcp {
                mode: TaskMcpBindingMode::Override,
                server_ids,
                servers,
            }
        }
    }
}

pub async fn load_task_mcp_binding_json(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Option<String>, String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT mcp_server_ids FROM tasks WHERE id = $1 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("读取任务 MCP 绑定失败: {error}"))?
    .ok_or_else(|| format!("任务不存在: {task_id}"))
}

pub async fn resolve_effective_mcp_for_task<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task_id: Option<&str>,
) -> Result<ResolvedMcp, String> {
    let document = load_mcp_document(app).await?;
    let binding = match task_id {
        Some(task_id) => load_task_mcp_binding_json(pool, task_id).await?,
        None => None,
    };
    Ok(resolve_effective_mcp_servers_lenient(
        &document,
        binding.as_deref(),
    ))
}

pub fn mcp_summary_line(resolved: &ResolvedMcp) -> String {
    let mode_label = match resolved.mode {
        TaskMcpBindingMode::Inherit => "继承全局",
        TaskMcpBindingMode::Override => "任务覆盖",
    };
    if resolved.servers.is_empty() {
        return format!("[MCP] {mode_label} · 空集（本会话不启用应用 MCP）");
    }
    let names: Vec<&str> = resolved
        .servers
        .iter()
        .map(|server| server.name.as_str())
        .collect();
    format!(
        "[MCP] {mode_label} · {} 个服务器：{}",
        resolved.servers.len(),
        names.join("、")
    )
}

/// Sanitize a stable TOML key for `mcp_servers.<key>`.
pub fn mcp_config_key(server: &McpServerConfig) -> String {
    let source = if server.id.trim().is_empty() {
        server.name.as_str()
    } else {
        server.id.as_str()
    };
    let mut key = String::with_capacity(source.len());
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            key.push(ch);
        } else {
            key.push('_');
        }
    }
    if key.is_empty() {
        key.push_str("mcp");
    }
    if key.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        key.insert(0, '_');
    }
    key
}

fn toml_quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Append Codex CLI args so this process uses only `servers` as MCP set.
pub fn append_mcp_config_args(args: &mut Vec<String>, servers: &[McpServerConfig]) {
    // Prevent merging user ~/.codex mcp_servers into the session set.
    args.push("--ignore-user-config".to_string());

    for server in servers {
        let key = mcp_config_key(server);
        args.push("-c".to_string());
        args.push(format!(
            "mcp_servers.{key}.command={}",
            toml_quoted(&server.command)
        ));
        if !server.args.is_empty() {
            let rendered = server
                .args
                .iter()
                .map(|arg| toml_quoted(arg))
                .collect::<Vec<_>>()
                .join(", ");
            args.push("-c".to_string());
            args.push(format!("mcp_servers.{key}.args=[{rendered}]"));
        }
        for env in &server.env {
            if env.key.trim().is_empty() {
                continue;
            }
            let env_key = env
                .key
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            args.push("-c".to_string());
            args.push(format!(
                "mcp_servers.{key}.env.{env_key}={}",
                toml_quoted(&env.value)
            ));
        }
    }
}

#[tauri::command]
pub async fn get_mcp_servers<R: Runtime>(app: AppHandle<R>) -> Result<McpServersDocument, String> {
    load_mcp_document(&app).await
}

#[tauri::command]
pub async fn update_mcp_servers<R: Runtime>(
    app: AppHandle<R>,
    payload: UpdateMcpServersPayload,
) -> Result<McpServersDocument, String> {
    if payload.servers.len() > 50 {
        return Err("MCP 服务器数量过多（最多 50）".to_string());
    }

    let mut servers = Vec::with_capacity(payload.servers.len());
    let mut seen_ids = std::collections::HashSet::new();
    for server in payload.servers {
        let normalized = normalize_server(server)?;
        if !seen_ids.insert(normalized.id.clone()) {
            return Err(format!("重复的 MCP 服务器 id: {}", normalized.id));
        }
        servers.push(normalized);
    }

    let document = McpServersDocument { servers };
    let path = mcp_servers_file_path(&app)?;
    write_document(&path, &document)?;

    if let Ok(pool) = sqlite_pool(&app).await {
        let _ = insert_activity_log(
            &pool,
            "mcp_servers_updated",
            &format!("更新 MCP 配置（{} 个服务器）", document.servers.len()),
            None,
            None,
            None,
        )
        .await;
    }

    Ok(document)
}

#[tauri::command]
pub async fn reset_mcp_servers<R: Runtime>(app: AppHandle<R>) -> Result<McpServersDocument, String> {
    let document = default_mcp_servers();
    let path = mcp_servers_file_path(&app)?;
    write_document(&path, &document)?;

    if let Ok(pool) = sqlite_pool(&app).await {
        let _ = insert_activity_log(
            &pool,
            "mcp_servers_reset",
            "重置 MCP 配置为默认示例",
            None,
            None,
            None,
        )
        .await;
    }

    Ok(document)
}

/// Export MCP servers in a Codex-compatible shape for user to copy into config.
#[tauri::command]
pub async fn export_mcp_servers_snippet<R: Runtime>(app: AppHandle<R>) -> Result<String, String> {
    let document = get_mcp_servers(app).await?;
    let mut lines = vec![
        "# 可将以下 JSON 片段合并到 Codex/Claude MCP 配置中".to_string(),
        "# 仅导出 enabled = true 的服务器".to_string(),
        "{".to_string(),
        "  \"mcpServers\": {".to_string(),
    ];

    let enabled: Vec<_> = document.servers.iter().filter(|s| s.enabled).collect();
    for (index, server) in enabled.iter().enumerate() {
        let args = server
            .args
            .iter()
            .map(|arg| format!("\"{}\"", arg.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ");
        let env_pairs = server
            .env
            .iter()
            .map(|env| {
                format!(
                    "\"{}\": \"{}\"",
                    env.key.replace('"', "\\\""),
                    env.value.replace('"', "\\\"")
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "    \"{}\": {{\n      \"command\": \"{}\",\n      \"args\": [{}],\n      \"env\": {{{}}}\n    }}{}",
            server.name.replace('"', "\\\""),
            server.command.replace('"', "\\\""),
            args,
            env_pairs,
            if index + 1 == enabled.len() {
                ""
            } else {
                ","
            }
        ));
    }
    lines.push("  }".to_string());
    lines.push("}".to_string());
    Ok(lines.join("\n"))
}

#[tauri::command]
pub async fn get_task_mcp_binding<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<TaskMcpBindingView, String> {
    let pool = sqlite_pool(&app).await?;
    let document = load_mcp_document(&app).await?;
    let binding = load_task_mcp_binding_json(&pool, &task_id).await?;
    // Lenient: unknown catalog ids are skipped so UI still loads after server deletion.
    let resolved = match binding.as_deref() {
        None => resolve_effective_mcp_servers(&document, None)?,
        Some(raw) => {
            // Corrupt JSON should surface; valid JSON with stale ids is lenient.
            let _: Vec<String> = serde_json::from_str(raw)
                .map_err(|error| format!("任务 MCP 绑定 JSON 无效: {error}"))?;
            resolve_effective_mcp_servers_lenient(&document, Some(raw))
        }
    };
    Ok(TaskMcpBindingView {
        task_id,
        mode: resolved.mode,
        server_ids: match binding {
            None => Vec::new(),
            Some(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        },
        effective: resolved.servers,
    })
}

#[tauri::command]
pub async fn set_task_mcp_binding<R: Runtime>(
    app: AppHandle<R>,
    payload: SetTaskMcpBindingPayload,
) -> Result<TaskMcpBindingView, String> {
    let pool = sqlite_pool(&app).await?;
    let task_id = payload.task_id.trim();
    if task_id.is_empty() {
        return Err("任务 id 不能为空".to_string());
    }

    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT project_id, title FROM tasks WHERE id = $1 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("读取任务失败: {error}"))?
    .ok_or_else(|| format!("任务不存在: {task_id}"))?;
    let (project_id, title) = row;

    let document = load_mcp_document(&app).await?;
    let known: std::collections::HashSet<&str> = document
        .servers
        .iter()
        .map(|server| server.id.as_str())
        .collect();

    let (column_value, mode, server_ids): (Option<String>, TaskMcpBindingMode, Vec<String>) =
        match payload.mode {
            TaskMcpBindingMode::Inherit => (None, TaskMcpBindingMode::Inherit, Vec::new()),
            TaskMcpBindingMode::Override => {
                let mut ids = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for id in payload.server_ids {
                    let id = id.trim().to_string();
                    if id.is_empty() {
                        continue;
                    }
                    if !known.contains(id.as_str()) {
                        return Err(format!("未知的 MCP 服务器 id: {id}"));
                    }
                    if seen.insert(id.clone()) {
                        ids.push(id);
                    }
                }
                let json = serde_json::to_string(&ids)
                    .map_err(|error| format!("序列化 MCP 绑定失败: {error}"))?;
                (Some(json), TaskMcpBindingMode::Override, ids)
            }
        };

    let updated_at = now_sqlite();
    sqlx::query("UPDATE tasks SET mcp_server_ids = $1, updated_at = $2 WHERE id = $3")
        .bind(&column_value)
        .bind(&updated_at)
        .bind(task_id)
        .execute(&pool)
        .await
        .map_err(|error| format!("更新任务 MCP 绑定失败: {error}"))?;

    let resolved = resolve_effective_mcp_servers(&document, column_value.as_deref())?;
    let details = match mode {
        TaskMcpBindingMode::Inherit => format!("任务「{title}」MCP 绑定改为继承全局默认"),
        TaskMcpBindingMode::Override if server_ids.is_empty() => {
            format!("任务「{title}」MCP 绑定改为显式空集")
        }
        TaskMcpBindingMode::Override => {
            let names: Vec<String> = resolved
                .servers
                .iter()
                .map(|server| server.name.clone())
                .collect();
            format!(
                "任务「{title}」MCP 绑定为 {} 个服务器：{}",
                names.len(),
                names.join("、")
            )
        }
    };

    let _ = insert_activity_log(
        &pool,
        "task_mcp_binding_updated",
        &details,
        None,
        Some(task_id),
        Some(&project_id),
    )
    .await;

    Ok(TaskMcpBindingView {
        task_id: task_id.to_string(),
        mode,
        server_ids,
        effective: resolved.servers,
    })
}

// Keep Serialize/Deserialize available for tests without unused warnings on re-exports.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
struct McpFileMeta {
    version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> McpServersDocument {
        McpServersDocument {
            servers: vec![
                McpServerConfig {
                    id: "a".to_string(),
                    name: "Alpha".to_string(),
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "pkg-a".to_string()],
                    env: vec![McpEnvVar {
                        key: "TOKEN".to_string(),
                        value: "secret".to_string(),
                    }],
                    enabled: true,
                    notes: None,
                },
                McpServerConfig {
                    id: "b".to_string(),
                    name: "Beta".to_string(),
                    command: "uvx".to_string(),
                    args: vec![],
                    env: vec![],
                    enabled: false,
                    notes: None,
                },
                McpServerConfig {
                    id: "c".to_string(),
                    name: "Gamma".to_string(),
                    command: "node".to_string(),
                    args: vec!["server.js".to_string()],
                    env: vec![],
                    enabled: true,
                    notes: None,
                },
            ],
        }
    }

    #[test]
    fn resolve_inherit_uses_enabled_only() {
        let resolved = resolve_effective_mcp_servers(&sample_doc(), None).unwrap();
        assert!(matches!(resolved.mode, TaskMcpBindingMode::Inherit));
        assert_eq!(resolved.server_ids, vec!["a", "c"]);
    }

    #[test]
    fn resolve_override_empty() {
        let resolved = resolve_effective_mcp_servers(&sample_doc(), Some("[]")).unwrap();
        assert!(matches!(resolved.mode, TaskMcpBindingMode::Override));
        assert!(resolved.servers.is_empty());
    }

    #[test]
    fn resolve_override_subset_ignores_global_enabled() {
        let resolved = resolve_effective_mcp_servers(&sample_doc(), Some(r#"["b"]"#)).unwrap();
        assert_eq!(resolved.server_ids, vec!["b"]);
        assert_eq!(resolved.servers[0].name, "Beta");
    }

    #[test]
    fn resolve_override_unknown_id_errors() {
        let err = resolve_effective_mcp_servers(&sample_doc(), Some(r#"["missing"]"#)).unwrap_err();
        assert!(err.contains("未知"));
    }

    #[test]
    fn append_mcp_config_args_empty_still_ignores_user_config() {
        let mut args = vec!["exec".to_string()];
        append_mcp_config_args(&mut args, &[]);
        assert_eq!(args, vec!["exec", "--ignore-user-config"]);
    }

    #[test]
    fn append_mcp_config_args_renders_server() {
        let mut args = Vec::new();
        let servers = vec![McpServerConfig {
            id: "a".to_string(),
            name: "Alpha".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "pkg".to_string()],
            env: vec![McpEnvVar {
                key: "TOKEN".to_string(),
                value: "secret".to_string(),
            }],
            enabled: true,
            notes: None,
        }];
        append_mcp_config_args(&mut args, &servers);
        assert!(args.contains(&"--ignore-user-config".to_string()));
        assert!(args.iter().any(|arg| arg.contains("mcp_servers.a.command=")));
        assert!(args.iter().any(|arg| arg.contains("mcp_servers.a.args=")));
        assert!(args
            .iter()
            .any(|arg| arg.contains("mcp_servers.a.env.TOKEN=")));
        // Ensure secret is in argv (needed for runtime) but summary never logs it.
        assert!(args.iter().any(|arg| arg.contains("secret")));
        let summary = mcp_summary_line(&ResolvedMcp {
            mode: TaskMcpBindingMode::Override,
            server_ids: vec!["a".to_string()],
            servers,
        });
        assert!(!summary.contains("secret"));
        assert!(summary.contains("Alpha"));
    }
}
