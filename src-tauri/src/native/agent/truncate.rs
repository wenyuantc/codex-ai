use crate::native::model::types::{Message, Role};

pub fn message_chars(message: &Message) -> usize {
    message.content.chars().count()
        + message.reasoning_content.chars().count()
        + message
            .tool_calls
            .iter()
            .map(|call| call.arguments.chars().count() + call.name.len())
            .sum::<usize>()
}

pub fn truncate_messages(messages: &mut [Message], limit: usize) {
    if limit == 0 {
        return;
    }
    let total: usize = messages.iter().map(message_chars).sum();
    if total <= limit {
        return;
    }
    for index in 0..messages.len() {
        if messages[index].role != Role::Tool {
            continue;
        }
        if messages[index].content.chars().count() > 240 {
            let prefix: String = messages[index].content.chars().take(200).collect();
            messages[index].content = format!("{prefix}…[truncated]");
        }
        let current: usize = messages.iter().map(message_chars).sum();
        if current <= limit {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::model::types::Message;

    #[test]
    fn shrinks_old_tool_results() {
        let mut messages = vec![
            Message::user("hi"),
            Message::tool_result("1", "x".repeat(800)),
            Message::user("next"),
        ];
        truncate_messages(&mut messages, 400);
        assert!(messages[1].content.contains("[truncated]"));
        assert!(message_chars(&messages[1]) < 800);
    }
}
