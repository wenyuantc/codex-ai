use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct SdkFileChangeEvent {
    pub(super) changes: Vec<SdkFileChangePayload>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SdkFileChangePayload {
    pub(super) kind: Option<String>,
    pub(super) path: Option<String>,
    #[serde(
        default,
        alias = "previousPath",
        alias = "oldPath",
        alias = "old_path",
        alias = "from",
        alias = "previous_path"
    )]
    pub(super) previous_path: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ClaudeCliJsonStreamState {
    assistant_messages: std::collections::HashMap<String, String>,
    tool_results: std::collections::HashMap<String, String>,
}

#[derive(Debug, Default)]
pub(super) struct ClaudeCliJsonParsedEvent {
    pub(super) session_id: Option<String>,
    pub(super) lines: Vec<String>,
    pub(super) file_change_events: Vec<SdkFileChangeEvent>,
}

pub(super) fn parse_claude_file_change_event(
    line: &str,
    prefix: &str,
) -> Option<SdkFileChangeEvent> {
    let json_str = line.strip_prefix(prefix)?.trim();
    serde_json::from_str(json_str).ok()
}

pub(super) fn extract_session_id(line: &str, prefix: &str) -> Option<String> {
    let value = line.strip_prefix(prefix)?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn json_string_field_raw<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn json_first_string_field_raw<'a>(value: &'a Value, candidates: &[&str]) -> Option<&'a str> {
    candidates.iter().find_map(|candidate| {
        json_string_field_raw(value, candidate).filter(|item| !item.trim().is_empty())
    })
}

fn json_string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    json_string_field_raw(value, key)
        .map(str::trim)
        .filter(|item| !item.is_empty())
}

fn json_first_string_field<'a>(value: &'a Value, candidates: &[&str]) -> Option<&'a str> {
    candidates
        .iter()
        .find_map(|candidate| json_string_field(value, candidate))
}

fn json_text_lines(text: &str) -> Vec<String> {
    text.replace('\r', "")
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn json_text_delta(previous: &str, next: &str) -> String {
    if next.starts_with(previous) {
        next[previous.len()..].to_string()
    } else {
        next.to_string()
    }
}

fn tool_input_path(input: &Value) -> Option<String> {
    json_first_string_field(input, &["file_path", "path", "filePath"]).map(ToOwned::to_owned)
}

fn file_change_from_tool_use(block: &Value) -> Option<SdkFileChangeEvent> {
    let name = json_string_field(block, "name")?;
    let input = block.get("input")?;
    let path = tool_input_path(input)?;
    let kind = match name {
        "Write" => "added",
        "Edit" | "MultiEdit" | "NotebookEdit" => "modified",
        _ => return None,
    };

    Some(SdkFileChangeEvent {
        changes: vec![SdkFileChangePayload {
            kind: Some(kind.to_string()),
            path: Some(path),
            previous_path: None,
        }],
    })
}

fn tool_summary_line(block: &Value) -> Option<String> {
    let name = json_string_field(block, "name")?;
    let input = block.get("input");
    match name {
        "Bash" => input
            .and_then(|value| json_first_string_field(value, &["command"]))
            .map(|command| format!("[命令] {command}"))
            .or_else(|| Some("[命令] (unknown)".to_string())),
        "Read" => input
            .and_then(tool_input_path)
            .map(|path| format!("[读取] {path}"))
            .or_else(|| Some("[读取] (unknown)".to_string())),
        "Write" => input
            .and_then(tool_input_path)
            .map(|path| format!("[写入] {path}"))
            .or_else(|| Some("[写入] (unknown)".to_string())),
        "Edit" | "MultiEdit" | "NotebookEdit" => input
            .and_then(tool_input_path)
            .map(|path| format!("[编辑] {path}"))
            .or_else(|| Some("[编辑] (unknown)".to_string())),
        "Glob" | "Grep" | "LS" => Some(format!("[工具] {name}")),
        _ => None,
    }
}

fn tool_result_text(block: &Value) -> Option<String> {
    let content = block.get("content")?;
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    let text = content
        .as_array()?
        .iter()
        .filter_map(|item| match json_string_field(item, "type") {
            Some("text") => json_string_field_raw(item, "text").map(ToOwned::to_owned),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    (!text.trim().is_empty()).then_some(text)
}

fn emit_assistant_content(
    message: &Value,
    state: &mut ClaudeCliJsonStreamState,
    parsed: &mut ClaudeCliJsonParsedEvent,
) {
    let message_id = json_string_field(message, "id").unwrap_or("assistant");
    let Some(content) = message.get("content").and_then(|value| value.as_array()) else {
        return;
    };

    for block in content {
        match json_string_field(block, "type") {
            Some("text") => {
                let next = json_first_string_field_raw(block, &["text"]).unwrap_or("");
                let key = format!("{message_id}:text");
                let previous = state
                    .assistant_messages
                    .get(&key)
                    .cloned()
                    .unwrap_or_default();
                let delta = json_text_delta(&previous, next);
                state.assistant_messages.insert(key, next.to_string());
                parsed.lines.extend(json_text_lines(&delta));
            }
            Some("thinking") => {
                if let Some(text) = json_first_string_field_raw(block, &["thinking", "text"]) {
                    let key = format!("{message_id}:thinking");
                    if !state.assistant_messages.contains_key(&key) {
                        parsed.lines.push(format!(
                            "[思考] {}",
                            text.chars().take(200).collect::<String>()
                        ));
                    }
                    state.assistant_messages.insert(key, text.to_string());
                }
            }
            Some("tool_use") => {
                if let Some(event) = file_change_from_tool_use(block) {
                    parsed.file_change_events.push(event);
                }
                if let Some(line) = tool_summary_line(block) {
                    parsed.lines.push(line);
                }
            }
            _ => {}
        }
    }
}

fn emit_user_tool_results(
    message: &Value,
    state: &mut ClaudeCliJsonStreamState,
    parsed: &mut ClaudeCliJsonParsedEvent,
) {
    let Some(content) = message.get("content").and_then(|value| value.as_array()) else {
        return;
    };

    for block in content {
        if json_string_field(block, "type") != Some("tool_result") {
            continue;
        }
        let tool_use_id = json_string_field(block, "tool_use_id").unwrap_or("tool_result");
        let Some(text) = tool_result_text(block) else {
            continue;
        };
        let previous = state
            .tool_results
            .get(tool_use_id)
            .cloned()
            .unwrap_or_default();
        let delta = json_text_delta(&previous, &text);
        state.tool_results.insert(tool_use_id.to_string(), text);
        parsed
            .lines
            .extend(json_text_lines(&delta).into_iter().take(20));
    }
}

pub(super) fn parse_claude_cli_json_event_line(
    line: &str,
    state: &mut ClaudeCliJsonStreamState,
) -> Option<ClaudeCliJsonParsedEvent> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    let mut parsed = ClaudeCliJsonParsedEvent::default();

    parsed.session_id =
        json_first_string_field(&value, &["session_id", "sessionId"]).map(ToOwned::to_owned);

    match json_string_field(&value, "type") {
        Some("system") => {}
        Some("assistant") => {
            if let Some(message) = value.get("message") {
                emit_assistant_content(message, state, &mut parsed);
            }
        }
        Some("user") => {
            if let Some(message) = value.get("message") {
                emit_user_tool_results(message, state, &mut parsed);
            }
        }
        Some("result") => match json_string_field(&value, "subtype") {
            Some("success") => {
                if let Some(result) = json_first_string_field_raw(&value, &["result"]) {
                    parsed.lines.extend(json_text_lines(result));
                }
            }
            Some("error_max_turns") => {
                parsed.lines.push("[Claude] 已达最大轮次限制".to_string());
            }
            Some("error_during_execution") => {
                parsed
                    .lines
                    .push("[ERROR] Claude 执行过程中出错".to_string());
            }
            Some(other) if other.starts_with("error") => {
                parsed
                    .lines
                    .push(format!("[ERROR] Claude 执行失败: {other}"));
            }
            _ => {}
        },
        Some("error") => {
            if let Some(message) = json_first_string_field(&value, &["message"]) {
                parsed.lines.push(format!("[ERROR] {message}"));
            } else if let Some(message) = value
                .get("error")
                .and_then(|item| json_first_string_field(item, &["message"]))
            {
                parsed.lines.push(format!("[ERROR] {message}"));
            }
        }
        _ => {}
    }

    Some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_cli_session_id_from_system_event() {
        let mut state = ClaudeCliJsonStreamState::default();
        let parsed = parse_claude_cli_json_event_line(
            r#"{"type":"system","subtype":"init","session_id":"abc-123"}"#,
            &mut state,
        )
        .expect("json event");

        assert_eq!(parsed.session_id.as_deref(), Some("abc-123"));
        assert!(parsed.lines.is_empty());
    }

    #[test]
    fn parses_claude_cli_assistant_text_without_raw_json() {
        let mut state = ClaudeCliJsonStreamState::default();
        let parsed = parse_claude_cli_json_event_line(
            r#"{"type":"assistant","message":{"id":"msg-1","content":[{"type":"text","text":"hello\nworld"}]}}"#,
            &mut state,
        )
        .expect("json event");

        assert_eq!(parsed.lines, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn parses_claude_cli_write_tool_as_file_change() {
        let mut state = ClaudeCliJsonStreamState::default();
        let parsed = parse_claude_cli_json_event_line(
            r#"{"type":"assistant","message":{"id":"msg-1","content":[{"type":"tool_use","name":"Write","input":{"file_path":"src/main.rs"}}]}}"#,
            &mut state,
        )
        .expect("json event");

        assert_eq!(parsed.lines, vec!["[写入] src/main.rs".to_string()]);
        assert_eq!(parsed.file_change_events.len(), 1);
        assert_eq!(
            parsed.file_change_events[0].changes[0].path.as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(
            parsed.file_change_events[0].changes[0].kind.as_deref(),
            Some("added")
        );
    }
}
