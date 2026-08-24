#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

pub fn parse_sse(text: &str) -> Vec<SseEvent> {
    let text = text.trim_start_matches('\u{feff}');
    let mut events = Vec::new();
    let mut event_name = String::new();
    let mut data_lines: Vec<String> = Vec::new();

    for raw in text.split('\n') {
        let line = raw.trim_end_matches('\r').trim_start();
        if line.is_empty() {
            flush_sse(&mut events, &mut event_name, &mut data_lines);
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            event_name = value.trim().to_string();
            continue;
        }
        if let Some(value) = line.strip_prefix("data:") {
            let data = value.trim_start().to_string();
            if is_standalone_data_line(&data) {
                if !data_lines.is_empty() {
                    flush_sse(&mut events, &mut event_name, &mut data_lines);
                }
                data_lines.push(data);
                flush_sse(&mut events, &mut event_name, &mut data_lines);
                continue;
            }
            data_lines.push(data);
        }
    }
    flush_sse(&mut events, &mut event_name, &mut data_lines);
    events
}

fn is_standalone_data_line(data: &str) -> bool {
    let trimmed = data.trim();
    if trimmed == "[DONE]" {
        return true;
    }
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
}

fn flush_sse(events: &mut Vec<SseEvent>, event_name: &mut String, data_lines: &mut Vec<String>) {
    if data_lines.is_empty() {
        event_name.clear();
        return;
    }
    let data = data_lines.join("\n");
    data_lines.clear();
    if data == "[DONE]" {
        event_name.clear();
        events.push(SseEvent {
            event: "done".to_string(),
            data: "[DONE]".to_string(),
        });
        return;
    }
    events.push(SseEvent {
        event: std::mem::take(event_name),
        data,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_and_data_only_events() {
        let events = parse_sse("event: ping\ndata: {\"ok\":true}\n\ndata: [DONE]\n");
        assert_eq!(events[0].event, "ping");
        assert_eq!(events[0].data, "{\"ok\":true}");
        assert_eq!(events[1].event, "done");
    }

    #[test]
    fn splits_complete_json_data_lines_without_blank_separators() {
        let events = parse_sse(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\ndata: {\"choices\":[{\"delta\":{\"content\":\" there\"}}]}\ndata: [DONE]\n",
        );
        assert_eq!(events.len(), 3);
        assert!(events[0].data.contains("hi"));
        assert!(events[1].data.contains("there"));
        assert_eq!(events[2].data, "[DONE]");
    }

    #[test]
    fn strips_bom_and_leading_whitespace() {
        let events = parse_sse("\u{feff}  data: {\"ok\":true}\n");
        assert_eq!(events[0].data, "{\"ok\":true}");
    }
}
