use std::collections::{HashSet, VecDeque};

use serde_json::Value;
use tokio::sync::mpsc;

use crate::engine::UsageDelta;
use crate::native::model::client::{ChatRequest, ModelClient};
use crate::native::model::types::{Message, NativeImage, Role, ToolCall, ToolSpec};
use crate::native::model::usage_to_delta;
use crate::native::settings::DEFAULT_NATIVE_MAX_TURNS;
use crate::native::tools::{
    execute_tool, tool_specs, CancelFlag, LocalWorkspace, McpSession, ToolCtx,
};

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
    extra_tools: Vec<ToolSpec>,
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
                mcp: McpSession::empty(),
                allow_all_high_risk: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                request_permission: None,
            },
            messages: Vec::new(),
            max_turns: DEFAULT_NATIVE_MAX_TURNS as u32,
            context_char_limit: DEFAULT_CONTEXT_CHARS,
            on_event: None,
            on_usage: None,
            extra_tools: Vec::new(),
            turns: 0,
            last_tool_key: None,
            last_tool_repeat: 0,
        }
    }

    pub fn cancel(&self) {
        self.ctx.cancel.cancel();
    }

    pub fn set_extra_tools(&mut self, tools: Vec<ToolSpec>) {
        self.extra_tools = tools;
    }

    fn combined_tools(&self) -> Vec<ToolSpec> {
        let mut tools = tool_specs();
        tools.extend(self.extra_tools.clone());
        tools
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
        loop {
            let last_turn = self.prepare_model_call()?;
            let tools = self.combined_tools();
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
            self.emit(tool_result_line(&call.name, &output));
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
        "TodoRead" => "[待办] 读取任务清单".to_string(),
        "TodoWrite" => format_todo_write_start(&args),
        other => format!("[工具] {other}"),
    }
}

fn format_todo_write_start(args: &Value) -> String {
    let Some(todos) = args.get("todos").and_then(Value::as_array) else {
        return "[待办] 更新任务清单".to_string();
    };
    if todos.is_empty() {
        return "[待办] (空)".to_string();
    }
    let lines: Vec<String> = todos.iter().filter_map(format_todo_item_line).collect();
    if lines.is_empty() {
        return "[待办] 更新任务清单".to_string();
    }
    format!("[待办]\n{}", lines.join("\n"))
}

fn format_todo_item_line(item: &Value) -> Option<String> {
    let content = item
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let status = item
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("pending");
    let priority = item
        .get("priority")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("medium");
    Some(format!(
        "- [{}] {} ({})",
        status,
        truncate_chars(content, 200),
        priority
    ))
}

fn tool_result_line(name: &str, output: &str) -> String {
    match name {
        "TodoWrite" if is_todo_list_output(output) => {
            format!("[工具结果] 已更新 {} 项", count_todo_item_lines(output))
        }
        "TodoRead" if is_todo_list_output(output) => {
            format!("[工具结果]\n{}", output.trim_end())
        }
        _ => {
            let first = output
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("");
            format!("[工具结果] {}", truncate_chars(first.trim(), 200))
        }
    }
}

fn is_todo_list_output(output: &str) -> bool {
    let trimmed = output.trim();
    trimmed == "(no todos)" || count_todo_item_lines(output) > 0
}

fn count_todo_item_lines(output: &str) -> usize {
    output
        .lines()
        .filter(|line| line.trim_start().starts_with("- ["))
        .count()
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
        let runner = AgentRunner::new(LocalWorkspace::new(root.clone()));
        runner
            .ctx
            .allow_all_high_risk
            .store(true, std::sync::atomic::Ordering::SeqCst);
        (runner, root)
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

    #[test]
    fn todo_write_start_line_lists_all_items() {
        let line = tool_start_line(
            "TodoWrite",
            r#"{"todos":[
                {"id":"1","content":"定位 TestController","status":"in_progress","priority":"high"},
                {"id":"2","content":"实现 ok 接口","status":"pending"},
                {"id":"3","content":"补测试","status":"pending","priority":"low"}
            ]}"#,
        );
        assert_eq!(
            line,
            "[待办]\n- [in_progress] 定位 TestController (high)\n- [pending] 实现 ok 接口 (medium)\n- [pending] 补测试 (low)"
        );
        assert!(!line.contains("TodoWrite"));
    }

    #[test]
    fn todo_write_start_line_empty_and_invalid() {
        assert_eq!(
            tool_start_line("TodoWrite", r#"{"todos":[]}"#),
            "[待办] (空)"
        );
        assert_eq!(
            tool_start_line("TodoWrite", "not-json"),
            "[待办] 更新任务清单"
        );
        assert_eq!(tool_start_line("TodoWrite", "{}"), "[待办] 更新任务清单");
    }

    #[test]
    fn todo_read_start_line_is_label() {
        assert_eq!(tool_start_line("TodoRead", "{}"), "[待办] 读取任务清单");
    }

    #[test]
    fn todo_write_result_summarizes_count() {
        let output =
            "- [in_progress] 定位 TestController (medium)\n- [pending] 实现 ok 接口 (medium)";
        assert_eq!(
            tool_result_line("TodoWrite", output),
            "[工具结果] 已更新 2 项"
        );
        assert_eq!(
            tool_result_line("TodoWrite", "(no todos)"),
            "[工具结果] 已更新 0 项"
        );
        assert_eq!(
            tool_result_line("TodoWrite", "todos 必须是数组"),
            "[工具结果] todos 必须是数组"
        );
    }

    #[test]
    fn todo_read_result_keeps_full_list() {
        let output =
            "- [completed] 定位 TestController (medium)\n- [in_progress] 实现 ok 接口 (medium)";
        assert_eq!(
            tool_result_line("TodoRead", output),
            "[工具结果]\n- [completed] 定位 TestController (medium)\n- [in_progress] 实现 ok 接口 (medium)"
        );
    }

    #[test]
    fn other_tool_result_still_uses_first_line() {
        assert_eq!(tool_result_line("Read", "line1\nline2"), "[工具结果] line1");
    }

    #[tokio::test]
    async fn emits_todo_write_list() {
        let (mut runner, root) = temp_runner();
        let (tx, mut rx) = mpsc::unbounded_channel();
        runner.on_event = Some(tx);
        let _ = runner
            .run_scripted(
                "go",
                vec![
                    assistant_tool_call(
                        "t1",
                        "TodoWrite",
                        r#"{"todos":[
                            {"content":"定位 TestController","status":"in_progress"},
                            {"content":"实现 ok 接口","status":"pending"},
                            {"content":"补测试","status":"pending"}
                        ]}"#,
                    ),
                    Message::assistant_text("done"),
                ],
            )
            .await
            .expect("run");
        let lines = drain_events(&mut rx);
        let start = lines
            .iter()
            .find(|line| line.starts_with("[待办]"))
            .expect("missing todo start");
        assert!(
            start.contains("- [in_progress] 定位 TestController (medium)")
                && start.contains("- [pending] 实现 ok 接口 (medium)")
                && start.contains("- [pending] 补测试 (medium)"),
            "todo list missing items: {start}"
        );
        assert!(
            lines.iter().any(|line| line == "[工具结果] 已更新 3 项"),
            "missing todo result count: {lines:?}"
        );
        let _ = fs::remove_dir_all(root);
    }
}
