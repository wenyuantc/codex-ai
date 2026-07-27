use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{insert_activity_log, new_id, sqlite_pool};
use crate::db::models::{McpEnvVar, McpServerConfig, McpServersDocument, UpdateMcpServersPayload};

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
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建配置目录失败: {error}"))?;
    }
    let json = serde_json::to_string_pretty(document)
        .map_err(|error| format!("序列化 MCP 配置失败: {error}"))?;
    fs::write(path, json).map_err(|error| format!("写入 MCP 配置失败: {error}"))
}

#[tauri::command]
pub async fn get_mcp_servers<R: Runtime>(app: AppHandle<R>) -> Result<McpServersDocument, String> {
    let path = mcp_servers_file_path(&app)?;
    load_document_from_disk(&path)
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
pub async fn export_mcp_servers_snippet<R: Runtime>(
    app: AppHandle<R>,
) -> Result<String, String> {
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
            if index + 1 == enabled.len() { "" } else { "," }
        ));
    }
    lines.push("  }".to_string());
    lines.push("}".to_string());
    Ok(lines.join("\n"))
}

// Keep Serialize/Deserialize available for tests without unused warnings on re-exports.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
struct McpFileMeta {
    version: u32,
}
