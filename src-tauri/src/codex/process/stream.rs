use super::*;

pub(super) fn parse_sdk_file_change_event(line: &str) -> Option<SdkFileChangeEvent> {
    let payload = line.strip_prefix(SDK_FILE_CHANGE_EVENT_PREFIX)?;
    serde_json::from_str::<SdkFileChangeEvent>(payload.trim()).ok()
}

pub(super) fn detect_exec_json_output_flag(help_output: &str) -> Option<CliJsonOutputFlag> {
    if help_output.contains("--json") {
        Some(CliJsonOutputFlag::Json)
    } else if help_output.contains("--experimental-json") {
        Some(CliJsonOutputFlag::ExperimentalJson)
    } else {
        None
    }
}

fn json_string_field_raw<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn json_first_string_field_raw<'a>(
    value: &'a serde_json::Value,
    candidates: &[&str],
) -> Option<&'a str> {
    candidates.iter().find_map(|candidate| {
        json_string_field_raw(value, candidate).filter(|item| !item.trim().is_empty())
    })
}

fn json_string_field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    json_string_field_raw(value, key)
        .map(str::trim)
        .filter(|item| !item.is_empty())
}

fn json_first_string_field<'a>(
    value: &'a serde_json::Value,
    candidates: &[&str],
) -> Option<&'a str> {
    candidates
        .iter()
        .find_map(|candidate| json_string_field(value, candidate))
}

fn cli_json_delta(previous: &str, next: &str) -> String {
    if next.starts_with(previous) {
        next[previous.len()..].to_string()
    } else {
        next.to_string()
    }
}

fn cli_json_multiline(text: &str) -> Vec<String> {
    text.replace('\r', "")
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn summarize_cli_json_file_change(item: &serde_json::Value) -> Option<String> {
    let changes = item.get("changes")?.as_array()?;
    let files = changes
        .iter()
        .filter_map(|change| {
            let path = json_first_string_field(
                change,
                &["path", "file_path", "filePath", "new_path", "newPath"],
            )?;
            let kind = normalize_session_file_change_kind(json_first_string_field(
                change,
                &["kind", "type", "action"],
            ))?;
            Some(format!("{}:{}", kind.as_str(), path))
        })
        .take(5)
        .collect::<Vec<_>>();

    if files.is_empty() {
        None
    } else {
        Some(format!(
            "[文件变更] {}{}",
            files.join(", "),
            if changes.len() > 5 { " ..." } else { "" }
        ))
    }
}

fn summarize_cli_json_todo_list(item: &serde_json::Value) -> Option<String> {
    let items = item.get("items").and_then(|value| value.as_array());
    let entries = items
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let text = json_first_string_field(entry, &["text", "title", "content"])?;
            let completed = entry
                .get("completed")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            Some(format!(
                "{} {}",
                if completed { "[x]" } else { "[ ]" },
                text
            ))
        })
        .collect::<Vec<_>>();

    if !entries.is_empty() {
        return Some(format!("[计划] {}", entries.join(" | ")));
    }

    json_first_string_field(item, &["summary", "text"]).map(|summary| format!("[计划] {summary}"))
}

pub(super) fn parse_cli_json_event_line(
    line: &str,
    state: &mut CliJsonStreamState,
) -> Option<CliJsonParsedEvent> {
    let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    let event_type = json_string_field(&value, "type")?;
    let mut parsed = CliJsonParsedEvent::default();

    match event_type {
        "thread.started" => {
            parsed.session_id = json_first_string_field(&value, &["thread_id", "session_id"])
                .map(ToOwned::to_owned);
        }
        "turn.failed" => {
            parsed
                .lines
                .push("[ERROR] Codex 执行失败，请查看后续退出信息。".to_string());
        }
        "error" => {
            if let Some(message) = json_first_string_field(&value, &["message"]) {
                parsed.lines.push(format!("[ERROR] {message}"));
            } else if let Some(message) = value
                .get("error")
                .and_then(|item| json_first_string_field(item, &["message"]))
            {
                parsed.lines.push(format!("[ERROR] {message}"));
            }
        }
        item_event if item_event.starts_with("item.") => {
            let item = value.get("item")?;
            let item_type = json_string_field(item, "type")?;
            let item_id = json_string_field(item, "id").unwrap_or(item_type);

            match item_type {
                "command_execution" => {
                    if item_event == "item.started" {
                        if let Some(command) = json_first_string_field(item, &["command"]) {
                            parsed.lines.push(format!("[命令] {command}"));
                        }
                    }

                    let next = json_first_string_field_raw(
                        item,
                        &["aggregated_output", "aggregatedOutput", "output"],
                    )
                    .unwrap_or("");
                    let previous = state
                        .command_outputs
                        .get(item_id)
                        .cloned()
                        .unwrap_or_default();
                    let delta = cli_json_delta(&previous, next);
                    state
                        .command_outputs
                        .insert(item_id.to_string(), next.to_string());
                    parsed.lines.extend(cli_json_multiline(&delta));

                    if json_string_field(item, "status") == Some("failed") {
                        if let Some(code) = item
                            .get("exit_code")
                            .or_else(|| item.get("exitCode"))
                            .and_then(|value| value.as_i64())
                        {
                            parsed.lines.push(format!("[ERROR] 命令退出，code={code}"));
                        }
                    }
                }
                "agent_message" => {
                    let next = json_first_string_field_raw(item, &["text"]).unwrap_or("");
                    let previous = state
                        .agent_messages
                        .get(item_id)
                        .cloned()
                        .unwrap_or_default();
                    let delta = cli_json_delta(&previous, next);
                    state
                        .agent_messages
                        .insert(item_id.to_string(), next.to_string());
                    parsed.lines.extend(cli_json_multiline(&delta));
                }
                "reasoning" => {
                    if let Some(text) = json_first_string_field_raw(item, &["text"]) {
                        let previous = state
                            .reasoning_messages
                            .get(item_id)
                            .cloned()
                            .unwrap_or_default();
                        if previous.is_empty() {
                            parsed.lines.push(format!("[思考] {text}"));
                        }
                        state
                            .reasoning_messages
                            .insert(item_id.to_string(), text.to_string());
                    }
                }
                "file_change" => {
                    if let Some(summary) = summarize_cli_json_file_change(item) {
                        parsed.lines.push(summary);
                    }
                }
                "error" => {
                    if let Some(message) = json_first_string_field(item, &["message"]) {
                        parsed.lines.push(format!("[ERROR] {message}"));
                    }
                }
                "todo_list" | "task_plan" | "plan_update" => {
                    if let Some(summary) = summarize_cli_json_todo_list(item) {
                        if state.last_todo_summary.as_deref() != Some(summary.as_str()) {
                            state.last_todo_summary = Some(summary.clone());
                            parsed.lines.push(summary);
                        }
                    }
                }
                "mcp_tool_call" => {
                    if item_event == "item.started" {
                        let server =
                            json_first_string_field(item, &["server"]).unwrap_or("unknown");
                        let tool = json_first_string_field(item, &["tool"]).unwrap_or("unknown");
                        parsed.lines.push(format!("[MCP] {server}/{tool}"));
                    }
                }
                "web_search" => {
                    if item_event == "item.started" {
                        if let Some(query) = json_first_string_field(item, &["query"]) {
                            parsed.lines.push(format!("[搜索] {query}"));
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }

    Some(parsed)
}

pub(super) fn extract_session_id_from_output(line: &str) -> Option<String> {
    let normalized = line.trim();
    if !normalized
        .to_ascii_lowercase()
        .starts_with(SESSION_ID_PREFIX)
    {
        return None;
    }

    normalized
        .split_once(':')
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
