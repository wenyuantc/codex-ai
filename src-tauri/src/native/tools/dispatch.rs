use std::collections::HashSet;

use serde::Deserialize;
use serde_json::Value;

use super::cancel::CancelFlag;
use super::local::{apply_edit, format_read, LocalWorkspace};
use super::paths::resolve_under_workspace;
use super::ssh::SshToolRuntime;

#[derive(Debug, Clone, Deserialize)]
pub struct TodoItem {
    #[serde(default)]
    pub id: String,
    pub content: String,
    pub status: String,
    #[serde(default)]
    pub priority: String,
}

pub struct ToolCtx {
    pub workspace: LocalWorkspace,
    pub ssh: Option<SshToolRuntime>,
    pub cancel: CancelFlag,
    pub read_files: HashSet<String>,
    pub todos: Vec<TodoItem>,
}

pub async fn execute_tool(
    ctx: &mut ToolCtx,
    name: &str,
    arguments: &str,
) -> Result<String, String> {
    if ctx.cancel.is_cancelled() {
        return Err("已取消".to_string());
    }
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
        other => Err(format!("unknown tool: {other}")),
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
