use serde_json::Value;

/// Grok Build CLI `streaming-json` 实际事件形态（官方 headless 文档 + 实测）:
///
/// - `{"type":"thought","data":"<token>"}` 思考流（token 级）
/// - `{"type":"text","data":"<token>"}` 回复流（token 级，可含 `\n`）
/// - `{"type":"tool_call",...}` / `{"type":"tool_call_update",...}` 工具开始与进度/结果
/// - `{"type":"plan","entries":[...]}` 计划
/// - `{"type":"available_commands",...}` 命令清单（噪音，默认静默）
/// - `{"type":"usage",...}` 单次模型响应用量
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
    /// toolCallId → 已输出的启动摘要（去重，避免 update 风暴）
    tool_start_lines: std::collections::HashMap<String, String>,
    /// toolCallId → 工具显示名（update 完成包常不含 toolName）
    tool_names: std::collections::HashMap<String, String>,
    /// toolCallId → 是否已输出完成摘要
    tool_done_ids: std::collections::HashSet<String>,
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

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn one_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

fn tool_payload(value: &Value) -> &Value {
    // 兼容 data 包装：{"type":"tool_call","data":{...}}
    match value.get("data") {
        Some(data) if data.is_object() => data,
        _ => value,
    }
}

fn tool_call_id(payload: &Value) -> Option<String> {
    json_first_string_field(payload, &["toolCallId", "tool_call_id", "id"]).map(ToOwned::to_owned)
}

/// Grok 把工具元数据放在 `_meta["x.ai/tool"]`（键名含 `/`，不能用未转义 JSON Pointer）。
fn grok_tool_meta(payload: &Value) -> Option<&Value> {
    payload.get("_meta")?.get("x.ai/tool")
}

fn tool_display_name(payload: &Value) -> Option<String> {
    if let Some(name) = json_first_string_field(
        payload,
        &["toolName", "tool_name", "name", "tool", "function"],
    ) {
        return Some(name.to_string());
    }
    // Grok _meta["x.ai/tool"].name
    grok_tool_meta(payload)
        .and_then(|meta| json_string_field(meta, "name"))
        .map(ToOwned::to_owned)
}

fn tool_input_value(payload: &Value) -> Option<&Value> {
    payload
        .get("rawInput")
        .or_else(|| payload.get("raw_input"))
        .or_else(|| payload.get("input"))
        .or_else(|| payload.get("arguments"))
        .or_else(|| payload.get("params"))
        .or_else(|| {
            grok_tool_meta(payload)
                .and_then(|meta| meta.get("input"))
                .filter(|item| item.is_object() || item.is_string())
        })
}

fn tool_input_object(payload: &Value) -> Option<&serde_json::Map<String, Value>> {
    tool_input_value(payload)?.as_object()
}

fn map_string<'a>(
    map: &'a serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        map.get(*key)
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
    })
}

/// Grok read_file 用 target_file；write/edit 用 file_path；list 用 target_directory。
const TOOL_PATH_KEYS: &[&str] = &[
    "target_file",
    "targetFile",
    "file_path",
    "filePath",
    "path",
    "file",
    "target_directory",
    "targetDirectory",
    "directory",
    "cwd",
    "working_directory",
    "workingDirectory",
];

fn tool_kind_or_name(payload: &Value) -> String {
    tool_display_name(payload)
        .or_else(|| {
            tool_input_object(payload)
                .and_then(|map| map_string(map, &["variant", "name"]))
                .map(ToOwned::to_owned)
        })
        .or_else(|| json_first_string_field(payload, &["kind"]).map(ToOwned::to_owned))
        .unwrap_or_else(|| "tool".to_string())
}

fn path_from_locations(payload: &Value) -> Option<String> {
    let locations = payload.get("locations")?.as_array()?;
    for item in locations {
        if let Some(path) = json_first_string_field(item, &["path", "file_path", "filePath", "uri"])
        {
            return Some(path.to_string());
        }
    }
    None
}

/// 从 title 提取反引号路径，例如 `Read `/repo/src/main.rs``
fn path_from_title(title: &str) -> Option<String> {
    let bytes = title.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let start = i + 1;
            if let Some(rel) = title[start..].find('`') {
                let inner = title[start..start + rel].trim();
                if !inner.is_empty()
                    && (inner.contains('/') || inner.contains('\\') || inner.contains('.'))
                {
                    return Some(inner.to_string());
                }
                i = start + rel + 1;
                continue;
            }
        }
        i += 1;
    }
    None
}

fn extract_tool_path(payload: &Value, input: Option<&serde_json::Map<String, Value>>) -> Option<String> {
    if let Some(input) = input {
        if let Some(path) = map_string(input, TOOL_PATH_KEYS) {
            return Some(path.to_string());
        }
    }
    if let Some(path) = path_from_locations(payload) {
        return Some(path);
    }
    if let Some(title) = json_first_string_field(payload, &["title"]) {
        if let Some(path) = path_from_title(title) {
            return Some(path);
        }
    }
    // meta.input 常把路径归一成 path
    if let Some(meta) = grok_tool_meta(payload) {
        if let Some(meta_input) = meta.get("input").and_then(|item| item.as_object()) {
            if let Some(path) = map_string(meta_input, TOOL_PATH_KEYS) {
                return Some(path.to_string());
            }
        }
    }
    None
}

fn short_path(path: &str) -> String {
    // 长绝对路径时优先展示尾部，便于终端扫读
    let normalized = path.trim().replace('\\', "/");
    if normalized.chars().count() <= 120 {
        return normalized;
    }
    let parts: Vec<&str> = normalized.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 3 {
        return truncate_chars(&normalized, 120);
    }
    format!("…/{}", parts[parts.len().saturating_sub(3)..].join("/"))
}

fn file_action_label(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("read") {
        "[读取]"
    } else if lower.contains("search_replace")
        || lower.contains("edit")
        || lower.contains("multiedit")
        || lower.contains("str_replace")
    {
        "[编辑]"
    } else if lower.contains("write") || lower == "write" {
        "[写入]"
    } else {
        "[工具]"
    }
}

fn summarize_todos(input: &serde_json::Map<String, Value>) -> Option<String> {
    let todos = input.get("todos").or_else(|| input.get("items"))?;
    let arr = todos.as_array()?;
    if arr.is_empty() {
        return Some("[待办] (空)".to_string());
    }

    let merge = input
        .get("merge")
        .and_then(|item| item.as_bool())
        .unwrap_or(false);
    let prefix = if merge { "[待办·合并]" } else { "[待办]" };

    let titles: Vec<String> = arr
        .iter()
        .filter_map(|item| {
            if let Some(s) = item.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                return Some(s.to_string());
            }
            let obj = item.as_object()?;
            let content = map_string(obj, &["content", "title", "text", "summary", "name"])?;
            let status = map_string(obj, &["status"]).unwrap_or("");
            let marker = match status {
                "completed" | "complete" | "done" => "✓",
                "in_progress" | "in-progress" | "doing" => "…",
                "cancelled" | "canceled" => "×",
                _ => "·",
            };
            Some(format!("{marker}{}", truncate_chars(content, 40)))
        })
        .take(4)
        .collect();

    if titles.is_empty() {
        return Some(format!("{prefix} {} 项", arr.len()));
    }
    let more = if arr.len() > titles.len() {
        format!(" …(+{})", arr.len() - titles.len())
    } else {
        String::new()
    };
    Some(format!("{prefix} {}{}", titles.join(" "), more))
}

/// 从工具 input 生成与 Claude/Codex 风格一致的启动摘要。
fn summarize_tool_start(payload: &Value) -> Option<String> {
    let name = tool_kind_or_name(payload);
    let lower = name.to_ascii_lowercase();
    let title = json_first_string_field(payload, &["title"]).map(str::trim);
    let input = tool_input_object(payload);

    // todo_write：展示待办内容，而不是空工具名
    if lower.contains("todo") {
        if let Some(input) = input {
            if let Some(line) = summarize_todos(input) {
                return Some(line);
            }
        }
        if let Some(title) = title {
            if !title.eq_ignore_ascii_case(&name) {
                return Some(format!("[待办] {}", truncate_chars(title, 160)));
            }
        }
        return Some("[待办] 更新任务清单".to_string());
    }

    if let Some(input) = input {
        if let Some(command) = map_string(input, &["command", "cmd"]) {
            return Some(format!("[命令] {}", truncate_chars(command, 240)));
        }
    }

    if let Some(path) = extract_tool_path(payload, input) {
        let display = short_path(&path);
        let label = file_action_label(&name);
        if label == "[工具]" {
            return Some(format!("[工具] {} {}", name, truncate_chars(&display, 200)));
        }
        return Some(format!("{label} {}", truncate_chars(&display, 220)));
    }

    if let Some(input) = input {
        if let Some(pattern) = map_string(input, &["pattern", "query", "glob"]) {
            return Some(format!(
                "[工具] {} {}",
                name,
                truncate_chars(pattern, 180)
            ));
        }

        if let Some(url) = map_string(input, &["url", "uri"]) {
            return Some(format!("[工具] {} {}", name, truncate_chars(url, 180)));
        }

        // write 只有 content 没有 path 时至少提示写入
        if lower.contains("write")
            && map_string(input, &["content", "new_string", "text"]).is_some()
        {
            return Some(format!("[写入] {name}"));
        }
    } else if let Some(input_val) = tool_input_value(payload) {
        if let Some(text) = input_val.as_str().map(str::trim).filter(|item| !item.is_empty()) {
            return Some(format!(
                "[工具] {} {}",
                name,
                truncate_chars(&one_line(text), 180)
            ));
        }
    }

    // title 含路径信息（反引号已在 extract_tool_path 处理）；其余有意义 title 也展示
    if let Some(title) = title {
        if !title.eq_ignore_ascii_case(&name) {
            return Some(format!(
                "[工具] {} {}",
                name,
                truncate_chars(title, 160)
            ));
        }
    }

    // 至少给出工具名，避免再变成盲盒
    Some(format!("[工具] {name}"))
}

fn extract_tool_result_text(payload: &Value) -> Option<String> {
    if let Some(raw) = payload
        .get("rawOutput")
        .or_else(|| payload.get("raw_output"))
        .or_else(|| payload.get("output"))
        .or_else(|| payload.get("result"))
    {
        if let Some(text) = raw.as_str().map(str::trim).filter(|item| !item.is_empty()) {
            return Some(text.to_string());
        }
        if let Some(obj) = raw.as_object() {
            if let Some(summary) = map_string(
                obj,
                &[
                    "summary_for_prompt",
                    "summary",
                    "stdout",
                    "output",
                    "text",
                    "message",
                ],
            ) {
                return Some(summary.to_string());
            }
            // Todo 完成等嵌套结构
            if let Some(nested) = obj
                .values()
                .find_map(|item| {
                    item.as_object().and_then(|inner| {
                        map_string(inner, &["summary_for_prompt", "summary", "stdout", "text"])
                            .map(ToOwned::to_owned)
                    })
                })
            {
                return Some(nested);
            }
            if let Some(lines) = obj.get("lines") {
                if let Some(n) = lines.as_u64().or_else(|| lines.as_i64().map(|v| v as u64)) {
                    return Some(format!("lines={n}"));
                }
            }
        }
        if let Some(arr) = raw.as_array() {
            let text = arr
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }

    // content: [{type:text,text:...}] 或字符串
    if let Some(content) = payload.get("content") {
        if let Some(text) = extract_text_from_content(content) {
            return Some(text);
        }
    }

    None
}

fn is_terminal_tool_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "completed" | "complete" | "success" | "succeeded" | "failed" | "error" | "cancelled" | "canceled"
    )
}

fn is_failed_tool_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "failed" | "error" | "cancelled" | "canceled"
    )
}

fn resolve_tool_name(state: &GrokJsonStreamState, call_id: &str, payload: &Value) -> String {
    tool_display_name(payload)
        .or_else(|| state.tool_names.get(call_id).cloned())
        .or_else(|| {
            json_first_string_field(payload, &["title", "kind", "variant"]).map(ToOwned::to_owned)
        })
        .or_else(|| {
            tool_input_object(payload)
                .and_then(|map| map_string(map, &["variant", "name"]))
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "tool".to_string())
}

fn summarize_tool_result(name: &str, payload: &Value) -> Option<String> {
    let status = json_first_string_field(payload, &["status"]).unwrap_or("completed");
    if !is_terminal_tool_status(status) {
        return None;
    }

    if let Some(text) = extract_tool_result_text(payload) {
        let compact = truncate_chars(&one_line(&text), 200);
        if is_failed_tool_status(status) {
            return Some(format!("[工具结果] {name} {status}: {compact}"));
        }
        return Some(format!("[工具结果] {name}: {compact}"));
    }

    if is_failed_tool_status(status) {
        return Some(format!("[工具结果] {name} {status}"));
    }
    // 成功但无输出：启动摘要已足够，避免刷 "[工具结果] xxx"
    None
}

fn remember_tool_name(state: &mut GrokJsonStreamState, call_id: &str, payload: &Value) {
    if let Some(name) = tool_display_name(payload) {
        state.tool_names.insert(call_id.to_string(), name);
        return;
    }
    if state.tool_names.contains_key(call_id) {
        return;
    }
    if let Some(name) = json_first_string_field(payload, &["title", "kind"]).map(ToOwned::to_owned)
    {
        state.tool_names.insert(call_id.to_string(), name);
    } else if let Some(name) = tool_input_object(payload)
        .and_then(|map| map_string(map, &["variant", "name"]))
        .map(ToOwned::to_owned)
    {
        state.tool_names.insert(call_id.to_string(), name);
    }
}

fn emit_tool_event(
    state: &mut GrokJsonStreamState,
    payload: &Value,
    is_update: bool,
    parsed: &mut GrokJsonParsedEvent,
) {
    let call_id = tool_call_id(payload).unwrap_or_else(|| {
        // 无 id 时用摘要 hash 近似去重
        format!("anon:{}", tool_kind_or_name(payload))
    });

    remember_tool_name(state, &call_id, payload);

    let status = json_first_string_field(payload, &["status"]).unwrap_or("");
    let terminal = !status.is_empty() && is_terminal_tool_status(status);

    // 1) 启动摘要：tool_call 必出；update 在带 input/title 时补出（Grok 常只发 update）
    let has_start_signal = tool_input_value(payload).is_some()
        || json_first_string_field(payload, &["title", "toolName", "tool_name", "name"]).is_some()
        || grok_tool_meta(payload)
            .and_then(|meta| json_string_field(meta, "name"))
            .is_some();

    if has_start_signal {
        if let Some(line) = summarize_tool_start(payload) {
            let should_emit = match state.tool_start_lines.get(&call_id) {
                Some(prev) => prev != &line,
                None => true,
            };
            if should_emit {
                state.tool_start_lines.insert(call_id.clone(), line.clone());
                parsed.lines.push(line);
            }
        }
    } else if !is_update {
        // 纯 tool_call 无字段时也给个兜底
        if let Some(line) = summarize_tool_start(payload) {
            if !state.tool_start_lines.contains_key(&call_id) {
                state.tool_start_lines.insert(call_id.clone(), line.clone());
                parsed.lines.push(line);
            }
        }
    }

    // 2) 完成/失败摘要
    if terminal {
        if state.tool_done_ids.contains(&call_id) {
            return;
        }
        let name = resolve_tool_name(state, &call_id, payload);
        if let Some(line) = summarize_tool_result(&name, payload) {
            state.tool_done_ids.insert(call_id);
            parsed.lines.push(line);
        } else {
            // 标记完成，避免后续空 update 再处理
            state.tool_done_ids.insert(call_id);
        }
    }
}

fn summarize_plan(value: &Value) -> Option<String> {
    let payload = tool_payload(value);
    let entries = payload
        .get("entries")
        .or_else(|| payload.get("todos"))
        .or_else(|| payload.get("items"))
        .or_else(|| payload.get("plan"));

    let Some(entries) = entries else {
        if let Some(text) =
            json_first_string_field(payload, &["text", "content", "message", "data"])
        {
            return Some(format!("[计划] {}", truncate_chars(&one_line(text), 200)));
        }
        return None;
    };

    if let Some(text) = entries.as_str().map(str::trim).filter(|item| !item.is_empty()) {
        return Some(format!("[计划] {}", truncate_chars(&one_line(text), 200)));
    }

    let arr = entries.as_array()?;
    if arr.is_empty() {
        return Some("[计划] (空)".to_string());
    }

    let titles: Vec<String> = arr
        .iter()
        .filter_map(|item| {
            if let Some(s) = item.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                return Some(s.to_string());
            }
            let obj = item.as_object()?;
            map_string(obj, &["content", "title", "text", "summary", "name"])
                .map(ToOwned::to_owned)
        })
        .take(4)
        .map(|s| truncate_chars(&s, 60))
        .collect();

    if titles.is_empty() {
        return Some(format!("[计划] {} 项", arr.len()));
    }

    let more = if arr.len() > titles.len() {
        format!(" …(+{})", arr.len() - titles.len())
    } else {
        String::new()
    };
    Some(format!("[计划] {}{}", titles.join(" · "), more))
}

fn usage_u64(usage: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        usage.get(*key).and_then(|item| {
            item.as_u64()
                .or_else(|| item.as_i64().map(|v| v.max(0) as u64))
                .or_else(|| {
                    item.as_f64()
                        .map(|v| if v.is_finite() && v >= 0.0 { v as u64 } else { 0 })
                })
        })
    })
}

fn summarize_usage(value: &Value) -> Option<String> {
    let payload = tool_payload(value);
    let usage = payload
        .get("usage")
        .or_else(|| payload.get("data"))
        .unwrap_or(payload);

    let input = usage_u64(
        usage,
        &["input_tokens", "inputTokens", "prompt_tokens", "input"],
    );
    let output = usage_u64(
        usage,
        &["output_tokens", "outputTokens", "completion_tokens", "output"],
    );
    let total = usage_u64(usage, &["total_tokens", "totalTokens", "total"]).or_else(|| {
        match (input, output) {
            (Some(i), Some(o)) => Some(i + o),
            _ => None,
        }
    });
    let reasoning = usage_u64(usage, &["reasoning_tokens", "reasoningTokens", "thoughtTokens"]);

    if input.is_none() && output.is_none() && total.is_none() {
        return None;
    }

    let mut parts = Vec::new();
    if let Some(v) = input {
        parts.push(format!("in={v}"));
    }
    if let Some(v) = output {
        parts.push(format!("out={v}"));
    }
    if let Some(v) = reasoning {
        if v > 0 {
            parts.push(format!("reason={v}"));
        }
    }
    if let Some(v) = total {
        parts.push(format!("total={v}"));
    }
    Some(format!("[用量] {}", parts.join(" ")))
}

fn summarize_unknown_event(event_type: &str, value: &Value) -> Option<String> {
    if let Some(text) = json_first_string_field(
        value,
        &["title", "toolName", "tool_name", "name", "message", "detail"],
    ) {
        return Some(format!("[{event_type}] {}", truncate_chars(text, 160)));
    }
    if let Some(text) =
        json_first_string_field_raw(value, &["data", "text", "content", "result"])
    {
        return Some(format!(
            "[{event_type}] {}",
            truncate_chars(&one_line(text), 160)
        ));
    }
    None
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
        // 命令清单噪音大，默认静默
        Some("available_commands") | Some("available_commands_update") => {}
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
            // end 事件本身不输出用量 JSON，避免与 usage 行重复刷屏
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
            let payload = tool_payload(&value);
            emit_tool_event(state, payload, false, &mut parsed);
        }
        Some("tool_call_update") | Some("tool_result") | Some("tool_use_result") => {
            flush_all_channels(state, &mut parsed);
            let payload = tool_payload(&value);
            emit_tool_event(state, payload, true, &mut parsed);
        }
        Some("plan") => {
            flush_all_channels(state, &mut parsed);
            if let Some(line) = summarize_plan(&value) {
                parsed.lines.push(line);
            }
        }
        Some("usage") => {
            flush_all_channels(state, &mut parsed);
            if let Some(line) = summarize_usage(&value) {
                parsed.lines.push(line);
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
            } else if let Some(line) = summarize_unknown_event(other, &value) {
                flush_all_channels(state, &mut parsed);
                parsed.lines.push(line);
            } else if parsed.session_id.is_none() {
                // 完全无可读字段：静默，避免再出现 [foo] 盲盒刷屏
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

    fn collect_lines(events: &[&str]) -> Vec<String> {
        let mut state = GrokJsonStreamState::default();
        let mut lines = Vec::new();
        for event in events {
            let parsed = parse_grok_json_event_line(event, &mut state).expect("event");
            lines.extend(parsed.lines);
        }
        let flushed = flush_grok_json_stream_state(&mut state);
        lines.extend(flushed.lines);
        lines
    }

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

    #[test]
    fn parses_official_tool_call_and_update_summary() {
        // 来自 Grok headless streaming-json 官方样例
        let lines = collect_lines(&[
            r#"{"type":"tool_call","toolCallId":"call_1","title":"Read","kind":"read","status":"in_progress","toolName":"read_file","rawInput":{"path":"src/main.rs"},"content":[],"locations":[]}"#,
            r#"{"type":"tool_call_update","toolCallId":"call_1","status":"completed","content":[],"rawOutput":{"lines":42},"locations":[]}"#,
        ]);

        assert_eq!(
            lines,
            vec![
                "[读取] src/main.rs".to_string(),
                "[工具结果] read_file: lines=42".to_string(),
            ]
        );
    }

    #[test]
    fn read_file_uses_target_file_field() {
        // 实测 Grok rawInput 使用 target_file，不是 path
        let lines = collect_lines(&[
            r#"{"type":"tool_call_update","toolCallId":"r1","title":"read_file","toolName":"read_file","rawInput":{"target_file":"/Users/wenyuan/proj/src/App.tsx"}}"#,
        ]);
        assert_eq!(lines, vec!["[读取] /Users/wenyuan/proj/src/App.tsx".to_string()]);
    }

    #[test]
    fn write_and_edit_show_file_path() {
        let lines = collect_lines(&[
            r#"{"type":"tool_call","toolCallId":"w1","toolName":"write","rawInput":{"file_path":"/repo/a.java","content":"package x;"}}"#,
            r#"{"type":"tool_call","toolCallId":"e1","toolName":"search_replace","rawInput":{"file_path":"/repo/b.java","old_string":"a","new_string":"b"}}"#,
        ]);
        assert_eq!(
            lines,
            vec![
                "[写入] /repo/a.java".to_string(),
                "[编辑] /repo/b.java".to_string(),
            ]
        );
    }

    #[test]
    fn todo_write_summarizes_todo_contents() {
        let lines = collect_lines(&[
            r#"{"type":"tool_call_update","toolCallId":"t1","title":"todo_write","toolName":"todo_write","rawInput":{"merge":false,"todos":[{"id":"1","content":"盘点认证","status":"in_progress"},{"id":"2","content":"实现 OAuth","status":"pending"},{"id":"3","content":"补测试","status":"pending"}]}}"#,
        ]);
        assert_eq!(
            lines,
            vec!["[待办] …盘点认证 ·实现 OAuth ·补测试".to_string()]
        );
    }

    #[test]
    fn path_can_come_from_title_backticks_or_locations() {
        let from_title = collect_lines(&[
            r#"{"type":"tool_call_update","toolCallId":"r2","toolName":"read_file","title":"Read `/repo/Foo.java`"}"#,
        ]);
        assert_eq!(from_title, vec!["[读取] /repo/Foo.java".to_string()]);

        let from_locations = collect_lines(&[
            r#"{"type":"tool_call_update","toolCallId":"r3","toolName":"read_file","locations":[{"path":"/repo/Bar.java"}]}"#,
        ]);
        assert_eq!(from_locations, vec!["[读取] /repo/Bar.java".to_string()]);
    }

    #[test]
    fn tool_call_update_only_stream_emits_start_and_result() {
        // 实测：Grok 常只发 tool_call_update，不发独立 tool_call
        let lines = collect_lines(&[
            r#"{"type":"tool_call_update","toolCallId":"call-abc","kind":"search","title":"StpUtil.login","rawInput":{"variant":"Grep","pattern":"StpUtil.login|AuthController","path":"/repo"},"_meta":{"x.ai/tool":{"name":"grep"}}}"#,
            r#"{"type":"tool_call_update","toolCallId":"call-abc","status":"completed","rawOutput":{"summary_for_prompt":"found 3 matches"}}"#,
            // 重复 completed 应去重
            r#"{"type":"tool_call_update","toolCallId":"call-abc","status":"completed","rawOutput":{"summary_for_prompt":"found 3 matches"}}"#,
        ]);

        assert_eq!(
            lines,
            vec![
                "[工具] grep /repo".to_string(),
                "[工具结果] grep: found 3 matches".to_string(),
            ]
        );
    }

    #[test]
    fn parses_shell_command_tool_call() {
        let lines = collect_lines(&[
            r#"{"type":"tool_call","toolCallId":"c2","toolName":"run_terminal_command","rawInput":{"command":"mvn test -Dtest=Foo"},"status":"in_progress"}"#,
            r#"{"type":"tool_call_update","toolCallId":"c2","status":"completed","rawOutput":{"stdout":"BUILD SUCCESS\nTests run: 1"}}"#,
        ]);

        assert_eq!(
            lines,
            vec![
                "[命令] mvn test -Dtest=Foo".to_string(),
                "[工具结果] run_terminal_command: BUILD SUCCESS Tests run: 1".to_string(),
            ]
        );
    }

    #[test]
    fn silences_available_commands_and_summarizes_usage_and_plan() {
        let lines = collect_lines(&[
            r#"{"type":"available_commands","tools":["read_file","bash"],"commands":["review"]}"#,
            r#"{"type":"plan","entries":[{"content":"新增 status 接口","status":"pending"},{"content":"补单元测试","status":"pending"},{"content":"跑 mvn test","status":"pending"}]}"#,
            r#"{"type":"usage","messageId":"resp_1","stopReason":"end_turn","usage":{"input_tokens":812,"output_tokens":45,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,"reasoning_tokens":12}}"#,
        ]);

        assert_eq!(
            lines,
            vec![
                "[计划] 新增 status 接口 · 补单元测试 · 跑 mvn test".to_string(),
                "[用量] in=812 out=45 reason=12 total=857".to_string(),
            ]
        );
    }

    #[test]
    fn does_not_emit_blind_type_only_markers() {
        let lines = collect_lines(&[
            r#"{"type":"tool_call_update","toolCallId":"x1","status":"in_progress"}"#,
            r#"{"type":"mystery_object","foo":1,"bar":{"a":2}}"#,
        ]);
        assert!(
            lines.iter().all(|line| !line.starts_with('[')
                || line.starts_with("[思考]")
                || line.starts_with("[工具]")
                || line.starts_with("[命令]")
                || line.starts_with("[读取]")
                || line.starts_with("[写入]")
                || line.starts_with("[编辑]")
                || line.starts_with("[工具结果]")
                || line.starts_with("[计划]")
                || line.starts_with("[用量]")
                || line.starts_with("[ERROR]")),
            "unexpected blind markers: {lines:?}"
        );
        // in_progress 且无 input/title 时静默；mystery 无字段静默
        assert!(lines.is_empty(), "expected empty, got {lines:?}");
    }
}
