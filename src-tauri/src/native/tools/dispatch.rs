use std::collections::HashSet;

use serde::Deserialize;
use serde_json::Value;

use super::cancel::CancelFlag;
use super::local::{apply_edit, format_read, LocalWorkspace};
use super::mcp::McpSession;
use super::paths::resolve_under_workspace;
use super::permission::{
    classify_native_tool_risk, NativePermissionDecision, NativeToolRisk, NativeToolRiskKind,
};
use super::ssh::SshToolRuntime;
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Deserialize)]
pub struct TodoItem {
    #[serde(default)]
    pub id: String,
    pub content: String,
    pub status: String,
    #[serde(default)]
    pub priority: String,
}

#[derive(Debug, Clone)]
pub struct PermissionPrompt {
    pub tool_name: String,
    pub kind: NativeToolRiskKind,
    pub summary: String,
    pub remote: bool,
}

pub type PermissionRequester =
    Arc<dyn Fn(PermissionPrompt, oneshot::Sender<NativePermissionDecision>) + Send + Sync>;

pub struct ToolCtx {
    pub workspace: LocalWorkspace,
    pub ssh: Option<SshToolRuntime>,
    pub cancel: CancelFlag,
    pub read_files: HashSet<String>,
    pub todos: Vec<TodoItem>,
    pub mcp: McpSession,
    pub allow_all_high_risk: Arc<std::sync::atomic::AtomicBool>,
    pub request_permission: Option<PermissionRequester>,
}

pub async fn execute_tool(
    ctx: &mut ToolCtx,
    name: &str,
    arguments: &str,
) -> Result<String, String> {
    if ctx.cancel.is_cancelled() {
        return Err("已取消".to_string());
    }
    confirm_if_high_risk(ctx, name, arguments).await?;
    match name {
        "Read" => call_read(ctx, arguments).await,
        "Write" => call_write(ctx, arguments).await,
        "Edit" => call_edit(ctx, arguments).await,
        "Glob" => call_glob(ctx, arguments).await,
        "Grep" => call_grep(ctx, arguments).await,
        "Bash" => call_bash(ctx, arguments).await,
        "TodoRead" => Ok(format_todos(&ctx.todos)),
        "TodoWrite" => call_todo_write(ctx, arguments),
        "WebFetch" => super::web::web_fetch(arguments).await,
        "WebSearch" => super::web::web_search(arguments).await,
        other if ctx.mcp.has_tool(other) => ctx.mcp.call(other, arguments).await,
        other => Err(format!("unknown tool: {other}")),
    }
}

async fn confirm_if_high_risk(
    ctx: &mut ToolCtx,
    name: &str,
    arguments: &str,
) -> Result<(), String> {
    let exists = match name {
        "Write" => write_target_exists(ctx, arguments).await,
        _ => None,
    };
    let is_mcp = ctx.mcp.has_tool(name) || name.starts_with("mcp_");
    match classify_native_tool_risk(name, arguments, exists, is_mcp) {
        NativeToolRisk::Low => Ok(()),
        NativeToolRisk::High { kind, summary } => {
            request_permission(ctx, name, kind, summary).await
        }
    }
}

async fn write_target_exists(ctx: &ToolCtx, arguments: &str) -> Option<bool> {
    let path = serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| {
            value
                .get("file_path")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
        })?;
    if ctx.ssh.is_some() {
        return Some(ctx.read_files.contains(&path));
    }
    ctx.workspace
        .resolve(&path)
        .ok()
        .map(|resolved| resolved.exists())
}

async fn request_permission(
    ctx: &mut ToolCtx,
    name: &str,
    kind: NativeToolRiskKind,
    summary: String,
) -> Result<(), String> {
    if ctx
        .allow_all_high_risk
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Ok(());
    }
    let Some(requester) = ctx.request_permission.clone() else {
        // Setting off, or tests that skip the UI channel.
        return Ok(());
    };
    let prompt = PermissionPrompt {
        tool_name: name.to_string(),
        kind,
        summary: if ctx.ssh.is_some() {
            format!("远程工作区 · {summary}")
        } else {
            summary
        },
        remote: ctx.ssh.is_some(),
    };
    let (tx, rx) = oneshot::channel();
    requester(prompt, tx);
    let decision = tokio::select! {
        biased;
        _ = async {
            loop {
                if ctx.cancel.is_cancelled() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        } => NativePermissionDecision::Deny,
        result = rx => result.unwrap_or(NativePermissionDecision::Deny),
    };
    match decision {
        NativePermissionDecision::AllowSession => {
            ctx.allow_all_high_risk
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
        NativePermissionDecision::AllowOnce => Ok(()),
        NativePermissionDecision::Deny => Err("用户不允许该高风险操作".to_string()),
    }
}

fn parse_args(arguments: &str) -> Result<Value, String> {
    if arguments.trim().is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    serde_json::from_str(arguments).map_err(|error| format!("工具参数不是合法 JSON: {error}"))
}

async fn call_read(ctx: &mut ToolCtx, arguments: &str) -> Result<String, String> {
    let args = parse_args(arguments)?;
    let path = string_arg(&args, "file_path")?;
    let offset = args.get("offset").and_then(Value::as_i64);
    let limit = args.get("limit").and_then(Value::as_i64);
    if let Some(ssh) = ctx.ssh.as_ref() {
        let raw = ssh.read(&path).await?;
        ctx.read_files.insert(path.clone());
        return Ok(format_read(&raw, offset, limit));
    }
    let resolved = resolve_under_workspace(&ctx.workspace.root, &path)?;
    let output = ctx.workspace.read_file(&path, offset, limit)?;
    ctx.read_files
        .insert(resolved.to_string_lossy().into_owned());
    Ok(output)
}

async fn call_write(ctx: &mut ToolCtx, arguments: &str) -> Result<String, String> {
    let args = parse_args(arguments)?;
    let path = string_arg(&args, "file_path")?;
    let content = string_arg(&args, "content")?;
    if let Some(ssh) = ctx.ssh.as_ref() {
        if !ctx.read_files.contains(&path) {
            return Err(
                "File has not been read yet. Read it first before writing to it.".to_string(),
            );
        }
        let output = ssh.write(&path, &content).await?;
        ctx.read_files.insert(path);
        return Ok(output);
    }
    let resolved = resolve_under_workspace(&ctx.workspace.root, &path)?;
    if resolved.exists()
        && !ctx
            .read_files
            .contains(&resolved.to_string_lossy().into_owned())
    {
        return Err("File has not been read yet. Read it first before writing to it.".to_string());
    }
    let output = ctx.workspace.write_file(&path, &content)?;
    ctx.read_files
        .insert(resolved.to_string_lossy().into_owned());
    Ok(output)
}

async fn call_edit(ctx: &mut ToolCtx, arguments: &str) -> Result<String, String> {
    let args = parse_args(arguments)?;
    let path = string_arg(&args, "file_path")?;
    let old = string_arg(&args, "old_string")?;
    let new = string_arg(&args, "new_string")?;
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if let Some(ssh) = ctx.ssh.as_ref() {
        if !ctx.read_files.contains(&path) {
            return Err("File has not been read yet. Read it first before editing.".to_string());
        }
        let original = ssh.read(&path).await?;
        let updated = apply_edit(&original, &old, &new, replace_all)?;
        ssh.write(&path, &updated).await?;
        return Ok(format!("Edited {path}"));
    }
    let resolved = resolve_under_workspace(&ctx.workspace.root, &path)?;
    if !ctx
        .read_files
        .contains(&resolved.to_string_lossy().into_owned())
    {
        return Err("File has not been read yet. Read it first before editing.".to_string());
    }
    let original =
        std::fs::read_to_string(&resolved).map_err(|error| format!("读取失败: {error}"))?;
    let updated = apply_edit(&original, &old, &new, replace_all)?;
    ctx.workspace.write_file(&path, &updated)?;
    Ok(format!("Edited {}", resolved.display()))
}

async fn call_glob(ctx: &ToolCtx, arguments: &str) -> Result<String, String> {
    let args = parse_args(arguments)?;
    let pattern = string_arg(&args, "pattern")?;
    let path = args.get("path").and_then(Value::as_str);
    if let Some(ssh) = ctx.ssh.as_ref() {
        let listing = ssh.glob().await?;
        let hits: Vec<_> = listing
            .lines()
            .filter(|line| super::glob::glob_match(&pattern, line.trim()))
            .take(100)
            .map(ToOwned::to_owned)
            .collect();
        return Ok(if hits.is_empty() {
            "No files found".to_string()
        } else {
            hits.join("\n")
        });
    }
    ctx.workspace.glob_files(&pattern, path)
}

async fn call_grep(ctx: &ToolCtx, arguments: &str) -> Result<String, String> {
    let args = parse_args(arguments)?;
    let pattern = string_arg(&args, "pattern")?;
    let path = args.get("path").and_then(Value::as_str);
    let glob = args.get("glob").and_then(Value::as_str);
    let head_limit = args.get("head_limit").and_then(Value::as_i64);
    if let Some(ssh) = ctx.ssh.as_ref() {
        return ssh.grep(&pattern, path).await;
    }
    ctx.workspace.grep_files(&pattern, path, glob, head_limit)
}

async fn call_bash(ctx: &ToolCtx, arguments: &str) -> Result<String, String> {
    let args = parse_args(arguments)?;
    let command = string_arg(&args, "command")?;
    let timeout = args.get("timeout").and_then(Value::as_i64);
    if let Some(ssh) = ctx.ssh.as_ref() {
        return ssh.bash(&command).await;
    }
    ctx.workspace.bash(&command, timeout, &ctx.cancel).await
}

fn call_todo_write(ctx: &mut ToolCtx, arguments: &str) -> Result<String, String> {
    let args = parse_args(arguments)?;
    let todos: Vec<TodoItem> =
        serde_json::from_value(args.get("todos").cloned().unwrap_or(Value::Null))
            .map_err(|_| "todos 必须是数组".to_string())?;
    ctx.todos = todos
        .into_iter()
        .enumerate()
        .map(|(index, mut item)| {
            if item.id.trim().is_empty() {
                item.id = format!("{}", index + 1);
            }
            if item.priority.trim().is_empty() {
                item.priority = "medium".to_string();
            }
            item
        })
        .collect();
    Ok(format_todos(&ctx.todos))
}

fn format_todos(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return "(no todos)".to_string();
    }
    todos
        .iter()
        .map(|item| format!("- [{}] {} ({})", item.status, item.content, item.priority))
        .collect::<Vec<_>>()
        .join("\n")
}

fn string_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{key} 不能为空"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn deny_keeps_existing_file() {
        let root = std::env::temp_dir().join(format!(
            "codex-ai-perm-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let path = root.join("keep.txt");
        std::fs::write(&path, "original").expect("write");
        let mut ctx = ToolCtx {
            workspace: LocalWorkspace::new(root.clone()),
            ssh: None,
            cancel: CancelFlag::new(),
            read_files: HashSet::new(),
            todos: Vec::new(),
            mcp: McpSession::empty(),
            allow_all_high_risk: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            request_permission: Some(Arc::new(
                |_prompt, tx: oneshot::Sender<NativePermissionDecision>| {
                    let _ = tx.send(NativePermissionDecision::Deny);
                },
            )),
        };
        let err = execute_tool(
            &mut ctx,
            "Write",
            r#"{"file_path":"keep.txt","content":"changed"}"#,
        )
        .await
        .expect_err("denied");
        assert!(err.contains("不允许"));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "original");
        let _ = std::fs::remove_dir_all(root);
    }
}
