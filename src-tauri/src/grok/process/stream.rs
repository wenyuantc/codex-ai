use serde_json::Value;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct GrokJsonStreamState {
    assistant_messages: std::collections::HashMap<String, String>,
}

#[derive(Debug, Default)]
pub(super) struct GrokJsonParsedEvent {
    pub(super) session_id: Option<String>,
    pub(super) lines: Vec<String>,
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

fn extract_text_from_content(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    let array = content.as_array()?;
    let text = array
        .iter()
        .filter_map(|item| {
            if json_string_field(item, "type") == Some("text") {
                json_first_string_field_raw(item, &["text"]).map(ToOwned::to_owned)
            } else {
                json_first_string_field_raw(item, &["text"]).map(ToOwned::to_owned)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    (!text.trim().is_empty()).then_some(text)
}

fn emit_assistant_content(
    message: &Value,
    state: &mut GrokJsonStreamState,
    parsed: &mut GrokJsonParsedEvent,
) {
    let message_id = json_string_field(message, "id").unwrap_or("assistant");
    if let Some(content) = message.get("content") {
        if let Some(text) = extract_text_from_content(content) {
            let key = format!("{message_id}:text");
            let previous = state
                .assistant_messages
                .get(&key)
                .cloned()
                .unwrap_or_default();
            let delta = json_text_delta(&previous, &text);
            state.assistant_messages.insert(key, text);
            parsed.lines.extend(json_text_lines(&delta));
            return;
        }
    }

    if let Some(text) = json_first_string_field_raw(message, &["text", "content", "result"]) {
        let key = format!("{message_id}:text");
        let previous = state
            .assistant_messages
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let delta = json_text_delta(&previous, text);
        state
            .assistant_messages
            .insert(key, text.to_string());
        parsed.lines.extend(json_text_lines(&delta));
    }
}

/// 宽松解析 Grok streaming-json 行：提取可读文本与 session_id；未知结构降级为原始行。
pub(super) fn parse_grok_json_event_line(
    line: &str,
    state: &mut GrokJsonStreamState,
) -> Option<GrokJsonParsedEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let value = match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => value,
        Err(_) => {
            return Some(GrokJsonParsedEvent {
                session_id: None,
                lines: vec![trimmed.to_string()],
            });
        }
    };

    let mut parsed = GrokJsonParsedEvent::default();
    parsed.session_id = json_first_string_field(
        &value,
        &["session_id", "sessionId", "id", "conversation_id", "conversationId"],
    )
    .map(ToOwned::to_owned);

    match json_string_field(&value, "type") {
        Some("system") | Some("status") | Some("heartbeat") => {}
        Some("assistant") | Some("message") | Some("response") => {
            if let Some(message) = value.get("message") {
                emit_assistant_content(message, state, &mut parsed);
            } else {
                emit_assistant_content(&value, state, &mut parsed);
            }
        }
        Some("result") | Some("final") | Some("completion") => {
            if let Some(result) =
                json_first_string_field_raw(&value, &["result", "text", "content", "output"])
            {
                parsed.lines.extend(json_text_lines(result));
            } else if let Some(message) = value.get("message") {
                emit_assistant_content(message, state, &mut parsed);
            }
        }
        Some("error") => {
            if let Some(message) = json_first_string_field(&value, &["message", "error", "detail"]) {
                parsed.lines.push(format!("[ERROR] {message}"));
            } else if let Some(message) = value
                .get("error")
                .and_then(|item| json_first_string_field(item, &["message", "detail"]))
            {
                parsed.lines.push(format!("[ERROR] {message}"));
            } else {
                parsed.lines.push("[ERROR] Grok 执行失败".to_string());
            }
        }
        Some("tool_use") | Some("tool_call") | Some("tool") => {
            if let Some(name) = json_first_string_field(&value, &["name", "tool", "tool_name"]) {
                if let Some(input) = value
                    .get("input")
                    .or_else(|| value.get("arguments"))
                    .and_then(|item| item.as_object())
                {
                    if let Some(command) = input
                        .get("command")
                        .and_then(|item| item.as_str())
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                    {
                        parsed.lines.push(format!("[命令] {command}"));
                    } else if let Some(path) = input
                        .get("path")
                        .or_else(|| input.get("file_path"))
                        .and_then(|item| item.as_str())
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                    {
                        parsed.lines.push(format!("[工具] {name} {path}"));
                    } else {
                        parsed.lines.push(format!("[工具] {name}"));
                    }
                } else {
                    parsed.lines.push(format!("[工具] {name}"));
                }
            }
        }
        Some(other) => {
            if let Some(text) =
                json_first_string_field_raw(&value, &["text", "content", "message", "result"])
            {
                parsed.lines.extend(json_text_lines(text));
            } else if parsed.session_id.is_none() {
                // 未知事件且无文本时降级为原始行，避免丢信息
                parsed.lines.push(format!("[{other}] {trimmed}"));
            }
        }
        None => {
            if let Some(text) =
                json_first_string_field_raw(&value, &["text", "content", "message", "result"])
            {
                parsed.lines.extend(json_text_lines(text));
            } else if parsed.session_id.is_none() {
                parsed.lines.push(trimmed.to_string());
            }
        }
    }

    Some(parsed)
}

/// 聚合 one-shot 输出：优先提取 assistant 文本，否则返回原始 stdout。
pub fn aggregate_grok_one_shot_output(stdout: &str) -> String {
    let mut state = GrokJsonStreamState::default();
    let mut lines = Vec::new();
    let mut saw_json = false;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(parsed) = parse_grok_json_event_line(trimmed, &mut state) {
            saw_json = true;
            lines.extend(parsed.lines);
        }
    }

    if saw_json && !lines.is_empty() {
        lines.join("\n")
    } else {
        stdout.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_id_from_system_like_event() {
        let mut state = GrokJsonStreamState::default();
        let parsed = parse_grok_json_event_line(
            r#"{"type":"system","session_id":"sess-123"}"#,
            &mut state,
        )
        .expect("json event");

        assert_eq!(parsed.session_id.as_deref(), Some("sess-123"));
        assert!(parsed.lines.is_empty());
    }

    #[test]
    fn parses_assistant_text_without_raw_json() {
        let mut state = GrokJsonStreamState::default();
        let parsed = parse_grok_json_event_line(
            r#"{"type":"assistant","message":{"id":"msg-1","content":[{"type":"text","text":"hello\nworld"}]}}"#,
            &mut state,
        )
        .expect("json event");

        assert_eq!(parsed.lines, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn unknown_json_falls_back_to_raw_or_text_fields() {
        let mut state = GrokJsonStreamState::default();
        let parsed = parse_grok_json_event_line(
            r#"{"type":"delta","text":"partial"}"#,
            &mut state,
        )
        .expect("json event");
        assert_eq!(parsed.lines, vec!["partial".to_string()]);
    }

    #[test]
    fn non_json_line_is_kept() {
        let mut state = GrokJsonStreamState::default();
        let parsed = parse_grok_json_event_line("plain log line", &mut state).expect("line");
        assert_eq!(parsed.lines, vec!["plain log line".to_string()]);
    }

    #[test]
    fn aggregates_one_shot_assistant_text() {
        let stdout = r#"
{"type":"system","session_id":"abc"}
{"type":"assistant","message":{"id":"1","content":[{"type":"text","text":"done"}]}}
"#;
        assert_eq!(aggregate_grok_one_shot_output(stdout), "done");
    }
}
