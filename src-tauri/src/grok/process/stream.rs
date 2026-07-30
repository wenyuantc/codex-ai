use serde_json::Value;

/// Grok Build CLI `streaming-json` 实际事件形态（实测）:
///
/// - `{"type":"thought","data":"<token>"}` 思考流（token 级）
/// - `{"type":"text","data":"<token>"}` 回复流（token 级，可含 `\n`）
/// - `{"type":"end","sessionId":"...","stopReason":"..."}` 回合结束
///
/// 另兼容旧的 assistant/message/tool 结构。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct GrokJsonStreamState {
    assistant_messages: std::collections::HashMap<String, String>,
    /// 当前活跃流通道：`text` / `thought`
    active_channel: Option<String>,
    /// 按通道聚合的未完成行缓冲
    stream_buffers: std::collections::HashMap<String, String>,
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

/// 流式 token 允许空白（如单独的 `\n`），否则无法按行聚合。
fn json_first_string_field_allow_ws<'a>(value: &'a Value, candidates: &[&str]) -> Option<&'a str> {
    candidates
        .iter()
        .find_map(|candidate| json_string_field_raw(value, candidate))
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
    match next.strip_prefix(previous) {
        Some(rest) => rest.to_string(),
        None => next.to_string(),
    }
}

fn channel_line_prefix(channel: &str) -> Option<&'static str> {
    match channel {
        "thought" => Some("[思考] "),
        _ => None,
    }
}

fn format_channel_line(channel: &str, line: &str) -> String {
    match channel_line_prefix(channel) {
        Some(prefix) => format!("{prefix}{line}"),
        None => line.to_string(),
    }
}

/// 将通道缓冲中已完成的行写入 parsed；保留未完成尾部。
fn drain_complete_lines(
    state: &mut GrokJsonStreamState,
    channel: &str,
    parsed: &mut GrokJsonParsedEvent,
) {
    let Some(buffer) = state.stream_buffers.get_mut(channel) else {
        return;
    };

    // 统一换行，按行切分
    let normalized = buffer.replace('\r', "");
    if !normalized.contains('\n') {
        *buffer = normalized;
        return;
    }

    let mut parts = normalized.split('\n').map(str::to_owned).collect::<Vec<_>>();
    let remainder = parts.pop().unwrap_or_default();
    *buffer = remainder;

    for part in parts {
        let line = part.trim_end();
        if line.is_empty() {
            continue;
        }
        parsed.lines.push(format_channel_line(channel, line));
    }
}

fn flush_channel(
    state: &mut GrokJsonStreamState,
    channel: &str,
    parsed: &mut GrokJsonParsedEvent,
) {
    drain_complete_lines(state, channel, parsed);
    if let Some(buffer) = state.stream_buffers.get_mut(channel) {
        let remaining = std::mem::take(buffer);
        let line = remaining.trim_end();
        if !line.is_empty() {
            parsed.lines.push(format_channel_line(channel, line));
        }
    }
}

fn flush_all_channels(state: &mut GrokJsonStreamState, parsed: &mut GrokJsonParsedEvent) {
    let channels = state.stream_buffers.keys().cloned().collect::<Vec<_>>();
    for channel in channels {
        flush_channel(state, &channel, parsed);
    }
    state.active_channel = None;
}

fn append_stream_delta(
    state: &mut GrokJsonStreamState,
    channel: &str,
    delta: &str,
    parsed: &mut GrokJsonParsedEvent,
) {
    // 切换通道时先刷掉上一个通道的残留
    if let Some(active) = state.active_channel.clone() {
        if active != channel {
            flush_channel(state, &active, parsed);
        }
    }
    state.active_channel = Some(channel.to_string());

    let buffer = state
        .stream_buffers
        .entry(channel.to_string())
        .or_default();
    buffer.push_str(delta);
    drain_complete_lines(state, channel, parsed);
}

fn extract_text_from_content(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    let array = content.as_array()?;
    let text = array
        .iter()
        .filter_map(|item| {
            json_first_string_field_raw(item, &["text", "data"]).map(ToOwned::to_owned)
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

    if let Some(text) = json_first_string_field_raw(message, &["text", "content", "result", "data"])
    {
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

fn extract_session_id(value: &Value) -> Option<String> {
    // 优先 session* 字段，避免把 message id / requestId 误当会话 ID
    json_first_string_field(
        value,
        &[
            "session_id",
            "sessionId",
            "conversation_id",
            "conversationId",
        ],
    )
    .map(ToOwned::to_owned)
}

/// 进程结束或聚合结束时刷出残留缓冲。
pub(super) fn flush_grok_json_stream_state(state: &mut GrokJsonStreamState) -> GrokJsonParsedEvent {
    let mut parsed = GrokJsonParsedEvent::default();
    flush_all_channels(state, &mut parsed);
    parsed
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

    let mut parsed = GrokJsonParsedEvent {
        session_id: extract_session_id(&value),
        ..Default::default()
    };

    match json_string_field(&value, "type") {
        Some("system") | Some("status") | Some("heartbeat") => {}
        // Grok Build 实际 streaming-json：token 级 thought/text
        Some("thought") => {
            // 允许空白 token（如 " "），否则思考词之间会丢空格
            if let Some(delta) = json_string_field_raw(&value, "data") {
                append_stream_delta(state, "thought", delta, &mut parsed);
            }
        }
        Some("text") | Some("delta") | Some("content_delta") | Some("response_delta") => {
            let delta =
                json_first_string_field_allow_ws(&value, &["data", "text", "content", "delta"]);
            if let Some(delta) = delta {
                append_stream_delta(state, "text", delta, &mut parsed);
            }
        }
        Some("end") | Some("done") | Some("turn_end") => {
            flush_all_channels(state, &mut parsed);
            // end 事件本身不输出用量 JSON，避免刷屏
        }
        Some("assistant") | Some("message") | Some("response") => {
            flush_all_channels(state, &mut parsed);
            if let Some(message) = value.get("message") {
                emit_assistant_content(message, state, &mut parsed);
            } else {
                emit_assistant_content(&value, state, &mut parsed);
            }
        }
        Some("result") | Some("final") | Some("completion") => {
            flush_all_channels(state, &mut parsed);
            if let Some(result) =
                json_first_string_field_raw(&value, &["result", "text", "content", "output", "data"])
            {
                parsed.lines.extend(json_text_lines(result));
            } else if let Some(message) = value.get("message") {
                emit_assistant_content(message, state, &mut parsed);
            }
        }
        Some("error") => {
            flush_all_channels(state, &mut parsed);
            if let Some(message) =
                json_first_string_field(&value, &["message", "error", "detail", "data"])
            {
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
        Some("tool_use") | Some("tool_call") | Some("tool") | Some("function_call") => {
            flush_all_channels(state, &mut parsed);
            // 兼容 data 包装：{"type":"tool_call","data":{...}}
            let payload = value.get("data").unwrap_or(&value);
            if let Some(name) =
                json_first_string_field(payload, &["name", "tool", "tool_name", "function"])
            {
                let input = payload
                    .get("input")
                    .or_else(|| payload.get("arguments"))
                    .or_else(|| payload.get("params"));

                if let Some(input) = input.and_then(|item| item.as_object()) {
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
            } else if let Some(data) = json_string_field_raw(payload, "data") {
                parsed.lines.extend(json_text_lines(data));
            }
        }
        Some(other) => {
            // 未知事件：优先取可读字段；有 session 且无文本则静默
            if let Some(text) = json_first_string_field_raw(
                &value,
                &["data", "text", "content", "message", "result"],
            ) {
                // 若 data 是短 token 形态，按 text 通道聚合更友好
                if text.chars().count() <= 32 && !text.contains('\n') {
                    append_stream_delta(state, "text", text, &mut parsed);
                } else {
                    flush_all_channels(state, &mut parsed);
                    parsed.lines.extend(json_text_lines(text));
                }
            } else if parsed.session_id.is_none() {
                // 未知对象结构：避免整行 JSON 刷屏，仅标记类型
                if value.get("data").is_some() || value.as_object().map(|o| o.len() > 1).unwrap_or(false)
                {
                    parsed.lines.push(format!("[{other}]"));
                } else {
                    parsed.lines.push(format!("[{other}] {trimmed}"));
                }
            }
        }
        None => {
            if let Some(text) = json_first_string_field_raw(
                &value,
                &["data", "text", "content", "message", "result"],
            ) {
                append_stream_delta(state, "text", text, &mut parsed);
            } else if parsed.session_id.is_none() {
                parsed.lines.push(trimmed.to_string());
            }
        }
    }

    Some(parsed)
}

/// 聚合 one-shot 输出：
/// 1) 优先解析完整 `--output-format json` 对象的 `text`
/// 2) 再尝试 streaming-json 行事件
/// 3) 最后回落原始 stdout
pub fn aggregate_grok_one_shot_output(stdout: &str) -> String {
    let trimmed_all = stdout.trim();
    if trimmed_all.is_empty() {
        return String::new();
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed_all) {
        if let Some(text) = value
            .get("text")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            return text.to_string();
        }
        if let Some(message) = value
            .get("message")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            // error object: {"type":"error","message":"..."}
            return message.to_string();
        }
    }

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

    let flushed = flush_grok_json_stream_state(&mut state);
    if !flushed.lines.is_empty() {
        saw_json = true;
        lines.extend(flushed.lines);
    }

    if saw_json && !lines.is_empty() {
        lines.join("\n")
    } else {
        trimmed_all.to_string()
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
        // delta 走 text 通道缓冲，需 flush 才完整输出
        let flushed = flush_grok_json_stream_state(&mut state);
        let mut lines = parsed.lines;
        lines.extend(flushed.lines);
        assert_eq!(lines, vec!["partial".to_string()]);
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

    #[test]
    fn aggregates_one_shot_final_json_object() {
        let stdout = r#"{
  "text": "最终结果",
  "stopReason": "EndTurn",
  "sessionId": "sess-1"
}"#;
        assert_eq!(aggregate_grok_one_shot_output(stdout), "最终结果");
    }

    #[test]
    fn aggregates_token_level_thought_and_text_stream() {
        // 注意：JSON 里的换行必须是真正的 \n 转义，不能用 raw string 字面 \\n
        let events = [
            r#"{"type":"thought","data":"I"}"#,
            r#"{"type":"thought","data":" will"}"#,
            r#"{"type":"thought","data":" help"}"#,
            r#"{"type":"text","data":"你好"}"#,
            r#"{"type":"text","data":"！"}"#,
            // serde_json 解析后 data 为真实换行符
            "{\"type\":\"text\",\"data\":\"\\n\"}",
            r#"{"type":"text","data":"完成"}"#,
            r#"{"type":"text","data":"了"}"#,
            r#"{"type":"end","stopReason":"EndTurn","sessionId":"sess-xyz"}"#,
        ];
        let mut state = GrokJsonStreamState::default();
        let mut lines = Vec::new();
        let mut session_id = None;
        for event in events {
            let parsed = parse_grok_json_event_line(event, &mut state).expect("event");
            if parsed.session_id.is_some() {
                session_id = parsed.session_id;
            }
            lines.extend(parsed.lines);
        }

        assert_eq!(session_id.as_deref(), Some("sess-xyz"));
        assert_eq!(
            lines,
            vec![
                "[思考] I will help".to_string(),
                "你好！".to_string(),
                "完成了".to_string(),
            ]
        );
    }

    #[test]
    fn aggregate_one_shot_handles_streaming_json_tokens() {
        let stdout = r#"
{"type":"thought","data":"plan"}
{"type":"text","data":"hello"}
{"type":"text","data":" world"}
{"type":"end","sessionId":"s1"}
"#;
        assert_eq!(
            aggregate_grok_one_shot_output(stdout),
            "[思考] plan\nhello world"
        );
    }

    #[test]
    fn does_not_emit_raw_json_for_thought_tokens() {
        let mut state = GrokJsonStreamState::default();
        let parsed = parse_grok_json_event_line(
            r#"{"type":"thought","data":","}"#,
            &mut state,
        )
        .expect("event");
        assert!(parsed.lines.is_empty());
        assert!(!format!("{:?}", parsed).contains("type"));

        let flushed = flush_grok_json_stream_state(&mut state);
        assert_eq!(flushed.lines, vec!["[思考] ,".to_string()]);
    }
}
