use std::collections::{HashSet, VecDeque};

use serde_json::Value;
use tokio::sync::mpsc;

use crate::engine::UsageDelta;
use crate::native::model::client::{ChatRequest, ModelClient};
use crate::native::model::types::{Message, NativeImage, Role, ToolCall, ToolSpec};
use crate::native::model::usage_to_delta;
use crate::native::settings::DEFAULT_NATIVE_MAX_TURNS;
use crate::native::tools::{execute_tool, tool_specs, CancelFlag, LocalWorkspace, ToolCtx};

use super::compact::{compact_local, should_compact};
use super::truncate::truncate_messages;
const DEFAULT_CONTEXT_CHARS: usize = 120_000;
const REPEAT_TOOL_LIMIT: u32 = 3;
const LAST_TURN_REMINDER: &str = "工具轮次已达上限。请立即给出最终结论，不要再调用工具。";
const LAST_TURN_FALLBACK: &str = "已达到最大工具轮次，已根据已有工具结果停止。";

pub struct AgentRunner {
    pub ctx: ToolCtx,
    pub messages: Vec<Message>,
    pub max_turns: u32,
    pub context_char_limit: usize,
    pub on_event: Option<mpsc::UnboundedSender<String>>,
    pub on_usage: Option<mpsc::UnboundedSender<UsageDelta>>,
    turns: u32,
    last_tool_key: Option<String>,
    last_tool_repeat: u32,
}

enum TurnControl {
    Continue,
    Stop(String),
}

impl AgentRunner {
    pub fn new(workspace: LocalWorkspace) -> Self {
        Self {
            ctx: ToolCtx {
                workspace,
                ssh: None,
                cancel: CancelFlag::new(),
                read_files: HashSet::new(),
                todos: Vec::new(),
            },
            messages: Vec::new(),
            max_turns: DEFAULT_NATIVE_MAX_TURNS as u32,
            context_char_limit: DEFAULT_CONTEXT_CHARS,
            on_event: None,
            on_usage: None,
            turns: 0,
            last_tool_key: None,
            last_tool_repeat: 0,
        }
    }

    pub fn cancel(&self) {
        self.ctx.cancel.cancel();
    }

    fn emit(&self, line: impl Into<String>) {
        if let Some(tx) = &self.on_event {
            let _ = tx.send(line.into());
        }
    }

    fn emit_usage(&self, usage: crate::native::model::types::Usage) {
        let Some(delta) = usage_to_delta(usage) else {
            return;
        };
        if let Some(line) = delta.format_terminal_line() {
            self.emit(line);
        }
        if let Some(tx) = &self.on_usage {
            let _ = tx.send(delta);
        }
    }

    pub async fn run_with_client(
        &mut self,
        client: &ModelClient,
        user: &str,
        model: &str,
        effort: Option<&str>,
        max_output_tokens: Option<u32>,
        thinking_enabled: bool,
        images: Vec<NativeImage>,
    ) -> Result<String, String> {
        self.begin_user_turn(user, images)?;
        let tools = tool_specs();
        loop {
            let last_turn = self.prepare_model_call()?;
            let tools_now: &[ToolSpec] = if last_turn { &[] } else { &tools };
            let (assistant, usage) = client
                .chat(ChatRequest {
                    messages: &self.messages,
                    tools: tools_now,
                    model,
                    effort,
                    max_output_tokens,
                    thinking_enabled,
                })
                .await?;
            self.emit_usage(usage);
            match self.consume_assistant(assistant, last_turn).await? {
                TurnControl::Stop(text) => return Ok(text),
                TurnControl::Continue => {}
            }
        }
    }

    pub async fn run_scripted(
        &mut self,
        user: &str,
        replies: Vec<Message>,
    ) -> Result<String, String> {
        self.begin_user_turn(user, Vec::new())?;
        let mut queue = VecDeque::from(replies);
        loop {
            let last_turn = self.prepare_model_call()?;
            let assistant = queue
                .pop_front()
                .ok_or_else(|| "scripted model exhausted".to_string())?;
            match self.consume_assistant(assistant, last_turn).await? {
                TurnControl::Stop(text) => return Ok(text),
                TurnControl::Continue => {}
            }
        }
    }

    fn begin_user_turn(&mut self, user: &str, images: Vec<NativeImage>) -> Result<(), String> {
        if self.ctx.cancel.is_cancelled() {
            return Err("已取消".to_string());
        }
        self.turns = 0;
        self.last_tool_key = None;
        self.last_tool_repeat = 0;
        self.messages.push(Message::user_with_images(user, images));
        Ok(())
    }

    fn prepare_model_call(&mut self) -> Result<bool, String> {
        if self.ctx.cancel.is_cancelled() {
            return Err("已取消".to_string());
        }
        if self.max_turns > 0 && self.turns >= self.max_turns {
            return Err("达到最大模型轮次".to_string());
        }
        self.turns += 1;
        truncate_messages(&mut self.messages, self.context_char_limit);
        if should_compact(&self.messages, self.context_char_limit)
            && compact_local(&mut self.messages)
        {
            self.emit("[工具] 已压缩上下文");
        }
        let last_turn = self.max_turns > 0 && self.turns >= self.max_turns;
        if last_turn {
            self.emit(format!(
                "[工具] 第 {}/{} 轮，停止调用工具并直接作答",
                self.turns, self.max_turns
            ));
            append_last_turn_reminder(&mut self.messages);
        }
        Ok(last_turn)
    }

    async fn consume_assistant(
        &mut self,
        mut assistant: Message,
        last_turn: bool,
    ) -> Result<TurnControl, String> {
        if self.ctx.cancel.is_cancelled() {
            return Err("已取消".to_string());
        }
        assistant
            .tool_calls
            .retain(|call| !call.name.trim().is_empty());
        if last_turn {
            assistant.tool_calls.clear();
        }
        if !assistant.reasoning_content.is_empty() {
            let chars = assistant.reasoning_content.chars().count();
            self.emit(format!("[思考] 已生成 {chars} 字"));
        }
        let text = assistant.content.clone();
        let tool_calls = assistant.tool_calls.clone();
        if !text.trim().is_empty() {
            self.emit(text.clone());
        }
        self.messages.push(assistant);
        if tool_calls.is_empty() {
            let text = if text.trim().is_empty() && last_turn {
                self.emit(LAST_TURN_FALLBACK.to_string());
                LAST_TURN_FALLBACK.to_string()
            } else {
                text
            };
            return Ok(TurnControl::Stop(text));
        }
        for call in tool_calls {
            if self.ctx.cancel.is_cancelled() {
                return Err("已取消".to_string());
            }
            self.emit(tool_start_line(&call.name, &call.arguments));
            let output = self.execute_logged_tool(&call).await;
            self.emit(tool_result_line(&output));
            let mut message = Message::tool_result(&call.id, output);
            message.name = call.name.clone();
            self.messages.push(message);
        }
        Ok(TurnControl::Continue)
    }

    async fn execute_logged_tool(&mut self, call: &ToolCall) -> String {
        let key = format!("{}\n{}", call.name, call.arguments);
        if self.last_tool_key.as_deref() == Some(key.as_str()) {
            self.last_tool_repeat = self.last_tool_repeat.saturating_add(1);
        } else {
            self.last_tool_key = Some(key);
            self.last_tool_repeat = 1;
        }
        if self.last_tool_repeat >= REPEAT_TOOL_LIMIT {
            return format!(
                "重复调用被拒绝：你已用相同参数连续调用 {} {} 次。请改用其他工具或直接给出最终结论。",
                call.name, self.last_tool_repeat
            );
        }
        match execute_tool(&mut self.ctx, &call.name, &call.arguments).await {
            Ok(value) => value,
            Err(error) => error,
        }
    }
}

fn append_last_turn_reminder(messages: &mut Vec<Message>) {
    if let Some(last) = messages.last_mut() {
        if last.role == Role::User || last.role == Role::Tool {
            if !last.content.is_empty() {
                last.content.push_str("\n\n");
            }
            last.content.push_str(LAST_TURN_REMINDER);
            return;
        }
    }
    messages.push(Message::user(LAST_TURN_REMINDER.to_string()));
}

fn tool_start_line(name: &str, arguments: &str) -> String {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    match name {
        "Read" => format!("[读取] {}", json_string(&args, "file_path")),
        "Write" => format!("[写入] {}", json_string(&args, "file_path")),
        "Edit" => format!("[编辑] {}", json_string(&args, "file_path")),
        "Bash" => format!("[命令] {}", json_string(&args, "command")),
        "Glob" => format!("[工具] Glob {}", json_string(&args, "pattern")),
        "Grep" => format!("[工具] Grep {}", json_string(&args, "pattern")),
        "TodoRead" | "TodoWrite" => format!("[待办] {name}"),
        other => format!("[工具] {other}"),
    }
}

fn tool_result_line(output: &str) -> String {
    let first = output
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    format!("[工具结果] {}", truncate_chars(first.trim(), 200))
}

fn json_string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "(unknown)".to_string())
}

fn truncate_chars(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let prefix: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{prefix}…")
}

pub fn assistant_tool_call(id: &str, name: &str, arguments: &str) -> Message {
    Message {
        role: Role::Assistant,
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: arguments.to_string(),
        }],
        tool_call_id: String::new(),
        name: String::new(),
        reasoning_content: String::new(),
        images: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::model::types::Message;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_runner() -> (AgentRunner, std::path::PathBuf) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("codex-ai-agent-{stamp}-{seq}"));
        fs::create_dir_all(&root).expect("mkdir");
        fs::write(root.join("hello.txt"), "hello world\n").expect("write");
        (AgentRunner::new(LocalWorkspace::new(root.clone())), root)
    }

    fn drain_events(rx: &mut mpsc::UnboundedReceiver<String>) -> Vec<String> {
        let mut lines = Vec::new();
        while let Ok(line) = rx.try_recv() {
            lines.push(line);
        }
        lines
    }

    #[tokio::test]
    async fn reads_then_edits_file() {
        let (mut runner, root) = temp_runner();
        let replies = vec![
            assistant_tool_call("c1", "Read", r#"{"file_path":"hello.txt"}"#),
            assistant_tool_call(
                "c2",
                "Edit",
                r#"{"file_path":"hello.txt","old_string":"hello world","new_string":"goodbye world"}"#,
            ),
            Message::assistant_text("done"),
        ];
        let text = runner
            .run_scripted("fix the greeting", replies)
            .await
            .expect("run");
        assert_eq!(text, "done");
        let content = fs::read_to_string(root.join("hello.txt")).expect("read result");
        assert_eq!(content, "goodbye world\n");
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn cancel_stops_before_next_model_call() {
        let (mut runner, root) = temp_runner();
        runner.max_turns = 8;
        runner.cancel();
        let error = runner
            .run_scripted(
                "go",
                vec![assistant_tool_call(
                    "c1",
                    "Read",
                    r#"{"file_path":"hello.txt"}"#,
                )],
            )
            .await
            .unwrap_err();
        assert_eq!(error, "已取消");
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn last_turn_stops_with_fallback_instead_of_error() {
        let (mut runner, root) = temp_runner();
        runner.max_turns = 1;
        let text = runner
            .run_scripted(
                "go",
                vec![
                    assistant_tool_call("c1", "Read", r#"{"file_path":"hello.txt"}"#),
                    Message::assistant_text("should not run"),
                ],
            )
            .await
            .expect("last turn");
        assert_eq!(text, LAST_TURN_FALLBACK);
        let original = fs::read_to_string(root.join("hello.txt")).expect("read");
        assert_eq!(original, "hello world\n");
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn last_turn_keeps_model_text() {
        let (mut runner, root) = temp_runner();
        runner.max_turns = 2;
        let text = runner
            .run_scripted(
                "go",
                vec![
                    assistant_tool_call("c1", "Read", r#"{"file_path":"hello.txt"}"#),
                    Message::assistant_text("审查通过"),
                ],
            )
            .await
            .expect("run");
        assert_eq!(text, "审查通过");
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn emits_tool_progress_lines() {
        let (mut runner, root) = temp_runner();
        let (tx, mut rx) = mpsc::unbounded_channel();
        runner.on_event = Some(tx);
        let _ = runner
            .run_scripted(
                "go",
                vec![
                    assistant_tool_call("c1", "Read", r#"{"file_path":"hello.txt"}"#),
                    Message::assistant_text("done"),
                ],
            )
            .await
            .expect("run");
        let lines = drain_events(&mut rx);
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("[读取] hello.txt")),
            "missing read start: {lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.starts_with("[工具结果]")),
            "missing tool result: {lines:?}"
        );
        assert!(
            lines.iter().any(|line| line == "done"),
            "missing final: {lines:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejects_repeated_identical_tool() {
        let (mut runner, root) = temp_runner();
        let replies = vec![
            assistant_tool_call("c1", "Read", r#"{"file_path":"hello.txt"}"#),
            assistant_tool_call("c2", "Read", r#"{"file_path":"hello.txt"}"#),
            assistant_tool_call("c3", "Read", r#"{"file_path":"hello.txt"}"#),
            Message::assistant_text("ok"),
        ];
        let text = runner.run_scripted("go", replies).await.expect("run");
        assert_eq!(text, "ok");
        let refused = runner
            .messages
            .iter()
            .any(|message| message.content.contains("重复调用被拒绝"));
        assert!(refused, "expected repeat rejection in tool results");
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn empty_tool_names_do_not_continue() {
        let (mut runner, root) = temp_runner();
        let mut dummy = Message::assistant_text("hello");
        dummy.tool_calls = vec![ToolCall {
            id: "empty".to_string(),
            name: String::new(),
            arguments: "{}".to_string(),
        }];
        let text = runner.run_scripted("go", vec![dummy]).await.expect("run");
        assert_eq!(text, "hello");
        let _ = fs::remove_dir_all(root);
    }
}
