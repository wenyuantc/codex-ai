use crate::native::model::types::{Message, Role};

use super::truncate::message_chars;

const COMPACT_THRESHOLD_PERCENT: usize = 85;
const PREVIEW_CHARS: usize = 160;

pub fn total_chars(messages: &[Message]) -> usize {
    messages.iter().map(message_chars).sum()
}

pub fn should_compact(messages: &[Message], limit: usize) -> bool {
    if limit == 0 {
        return false;
    }
    total_chars(messages).saturating_mul(100) >= limit.saturating_mul(COMPACT_THRESHOLD_PERCENT)
}

pub fn compact_local(messages: &mut Vec<Message>) -> bool {
    if messages.len() < 4 {
        return false;
    }
    let sys_len = messages
        .iter()
        .take_while(|message| message.role == Role::System)
        .count();
    let rest = &messages[sys_len..];
    let groups = group_user_turns(rest);
    if groups.len() < 2 {
        return false;
    }
    let (old, recent) = groups.split_at(groups.len() - 1);
    let to_summarize: Vec<Message> = old.iter().flatten().cloned().collect();
    let preserved: Vec<Message> = recent.iter().flatten().cloned().collect();
    if to_summarize.is_empty() || preserved.is_empty() {
        return false;
    }
    let summary = local_summary(&to_summarize);
    let mut next = messages[..sys_len].to_vec();
    next.push(Message::user(format!(
        "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\n{summary}\n\nRecent messages are preserved verbatim."
    )));
    next.extend(preserved);
    *messages = next;
    true
}

fn group_user_turns(messages: &[Message]) -> Vec<Vec<Message>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    for message in messages {
        if message.role == Role::User && !current.is_empty() {
            groups.push(std::mem::take(&mut current));
        }
        current.push(message.clone());
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn local_summary(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|message| {
            let preview = if message.content.trim().is_empty() && !message.tool_calls.is_empty() {
                let names = message
                    .tool_calls
                    .iter()
                    .map(|call| call.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("tool_calls: {names}")
            } else {
                let text = message.content.replace('\n', " ");
                if text.chars().count() > PREVIEW_CHARS {
                    let prefix: String = text.chars().take(PREVIEW_CHARS).collect();
                    format!("{prefix}…")
                } else {
                    text
                }
            };
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
                Role::System => "system",
            };
            if message.role == Role::Tool && !message.name.is_empty() {
                format!("- tool({}): {preview}", message.name)
            } else {
                format!("- {role}: {preview}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_system_and_latest_user_turn() {
        let mut messages = vec![
            Message::system("sys"),
            Message::user("first"),
            Message::assistant_text("looked"),
            Message::tool_result("c1", "fn a() {}"),
            Message::user("second"),
            Message::assistant_text("done"),
        ];
        assert!(compact_local(&mut messages));
        assert_eq!(messages[0].role, Role::System);
        assert!(messages[1].content.contains("first"));
        assert!(messages.iter().any(|item| item.content == "second"));
        assert!(messages.iter().any(|item| item.content == "done"));
        assert!(!messages.iter().any(|item| item.content == "first"));
    }

    #[test]
    fn skips_when_only_one_user_turn() {
        let mut messages = vec![
            Message::system("sys"),
            Message::user("only"),
            Message::assistant_text("ok"),
        ];
        assert!(!compact_local(&mut messages));
        assert_eq!(messages.len(), 3);
    }
}
