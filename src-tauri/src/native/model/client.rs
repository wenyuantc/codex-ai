use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::native::protocol::{
    channel_chat_url, channel_models_url, model_list_next_page, parse_model_list_json,
    PROTOCOL_ANTHROPIC, PROTOCOL_CODEX, PROTOCOL_OPENAI,
};
use crate::native::tools::CancelFlag;

use super::anthropic::{build_anthropic_body, parse_anthropic_json, parse_anthropic_sse};
use super::openai::{
    build_openai_body, parse_max_output_token_limit, parse_openai_json, parse_openai_sse,
};
use super::responses::{build_responses_body, parse_responses_json, parse_responses_sse};
use super::retry::{
    format_http_error, format_retry_line, is_retryable_error, redact_secrets, RetryConfig,
};
use super::sse::parse_sse;
use super::types::{Message, StreamEvent, ToolSpec, Usage};

#[derive(Debug, Clone)]
pub struct ModelClientConfig {
    pub protocol: String,
    pub base_url: String,
    pub api_key: String,
    pub extra_headers: HashMap<String, String>,
    pub retry: RetryConfig,
    pub timeout: Duration,
}

pub struct ChatRequest<'a> {
    pub messages: &'a [Message],
    pub tools: &'a [ToolSpec],
    pub model: &'a str,
    pub effort: Option<&'a str>,
    pub max_output_tokens: Option<u32>,
    pub thinking_enabled: bool,
}

pub type RetryHook = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone)]
pub struct ModelClient {
    http: reqwest::Client,
    config: ModelClientConfig,
    on_retry: Option<RetryHook>,
    cancel: Option<CancelFlag>,
}

impl ModelClient {
    pub fn new(config: ModelClientConfig) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|error| format!("创建 HTTP 客户端失败: {error}"))?;
        Ok(Self {
            http,
            config,
            on_retry: None,
            cancel: None,
        })
    }

    pub fn with_retry_hook(mut self, hook: RetryHook) -> Self {
        self.on_retry = Some(hook);
        self
    }

    pub fn with_cancel(mut self, cancel: CancelFlag) -> Self {
        self.cancel = Some(cancel);
        self
    }

    pub fn build_body(&self, request: &ChatRequest<'_>, stream: bool) -> Result<Value, String> {
        match self.config.protocol.as_str() {
            PROTOCOL_ANTHROPIC => Ok(build_anthropic_body(
                request.messages,
                request.tools,
                request.model,
                request.effort,
                request.max_output_tokens,
                request.thinking_enabled,
                stream,
            )),
            PROTOCOL_CODEX => Ok(build_responses_body(
                request.messages,
                request.tools,
                request.model,
                request.effort,
                request.max_output_tokens,
                request.thinking_enabled,
                stream,
            )),
            PROTOCOL_OPENAI => Ok(build_openai_body(
                request.messages,
                request.tools,
                request.model,
                request.effort,
                request.max_output_tokens,
                request.thinking_enabled,
                stream,
            )),
            other => Err(format!("不支持的渠道协议: {other}")),
        }
    }

    pub fn parse_sse(&self, text: &str) -> Result<(Message, Usage), String> {
        match self.config.protocol.as_str() {
            PROTOCOL_ANTHROPIC => parse_anthropic_sse(text),
            PROTOCOL_CODEX => parse_responses_sse(text),
            PROTOCOL_OPENAI => parse_openai_sse(text),
            other => Err(format!("不支持的渠道协议: {other}")),
        }
    }

    pub async fn chat(&self, request: ChatRequest<'_>) -> Result<(Message, Usage), String> {
        let url = channel_chat_url(&self.config.base_url, &self.config.protocol)?;
        let mut max_output_tokens = request.max_output_tokens;
        let mut retried_limit = false;
        loop {
            let adjusted = ChatRequest {
                messages: request.messages,
                tools: request.tools,
                model: request.model,
                effort: request.effort,
                max_output_tokens,
                thinking_enabled: request.thinking_enabled,
            };
            let body = self.build_body(&adjusted, true)?;
            match self.post_stream(&url, &body).await {
                Ok(result) => return Ok(result),
                Err(error) if !retried_limit => {
                    let Some(limit) = parse_max_output_token_limit(&error) else {
                        return Err(error);
                    };
                    let current = max_output_tokens.unwrap_or(u32::MAX);
                    if limit == 0 || limit >= current {
                        return Err(error);
                    }
                    max_output_tokens = Some(limit);
                    retried_limit = true;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub async fn list_models(&self) -> Result<Vec<String>, String> {
        let url = channel_models_url(&self.config.base_url)?;
        let mut collected = Vec::new();
        let mut after_id: Option<String> = None;
        for _ in 0..20 {
            let mut query: Vec<(&str, String)> = Vec::new();
            if self.config.protocol == PROTOCOL_ANTHROPIC {
                query.push(("limit", "100".to_string()));
            }
            if let Some(id) = after_id.as_deref() {
                query.push(("after_id", id.to_string()));
            }
            let query_refs: Vec<(&str, &str)> = query
                .iter()
                .map(|(key, value)| (*key, value.as_str()))
                .collect();
            let (status, text) = self.get_raw(&url, &query_refs).await?;
            if !(200..300).contains(&status) {
                return Err(format_http_error(status, &url, &text));
            }
            let page = parse_model_list_json(&text)?;
            for model in page {
                if collected.len() >= 500 {
                    break;
                }
                if !collected.iter().any(|existing| existing == &model) {
                    collected.push(model);
                }
            }
            if collected.len() >= 500 {
                break;
            }
            after_id = model_list_next_page(&text);
            if after_id.is_none() {
                break;
            }
        }
        collected.sort_by_key(|model| model.to_ascii_lowercase());
        Ok(collected)
    }

    pub async fn probe(&self, model: &str) -> Result<(), String> {
        let url = channel_chat_url(&self.config.base_url, &self.config.protocol)?;
        let request = ChatRequest {
            messages: &[Message::user("ping")],
            tools: &[],
            model,
            effort: None,
            max_output_tokens: Some(16),
            thinking_enabled: false,
        };
        let body = self.build_body(&request, false)?;
        let (status, text) = self.post_raw(&url, &body).await?;
        if (200..300).contains(&status) {
            return Ok(());
        }
        Err(format_http_error(status, &url, &text))
    }

    pub async fn chat_stream(
        &self,
        request: ChatRequest<'_>,
    ) -> Result<mpsc::Receiver<StreamEvent>, String> {
        let (tx, rx) = mpsc::channel(32);
        let (message, usage) = self.chat(request).await?;
        for event in events_from_message(message, usage) {
            let _ = tx.send(event).await;
        }
        Ok(rx)
    }

    async fn post_stream(&self, url: &str, body: &Value) -> Result<(Message, Usage), String> {
        let attempts = self.config.retry.max_retries.saturating_add(1);
        let mut last_error = "模型请求失败".to_string();
        for attempt in 0..attempts {
            if self.is_cancelled() {
                return Err("已取消".to_string());
            }
            match self.post_raw(url, body).await {
                Ok((status, text)) if (200..300).contains(&status) => {
                    match self.parse_success_body(&text) {
                        Ok(result) => return Ok(result),
                        Err(error) => last_error = error,
                    }
                    if self.should_stop_retry(&last_error, Some(status), attempt, attempts) {
                        return Err(last_error);
                    }
                }
                Ok((status, text)) => {
                    last_error = format_http_error(status, url, &text);
                    if self.should_stop_retry(&last_error, Some(status), attempt, attempts) {
                        return Err(last_error);
                    }
                }
                Err(error) => {
                    last_error = error;
                    if self.is_cancelled() {
                        return Err("已取消".to_string());
                    }
                    if self.should_stop_retry(&last_error, None, attempt, attempts) {
                        return Err(last_error);
                    }
                }
            }
            let delay = self.config.retry.delay_for_attempt(attempt);
            self.emit_retry(&last_error, attempt.saturating_add(1), delay);
            self.wait_before_retry(delay).await?;
        }
        Err(last_error)
    }

    fn should_stop_retry(
        &self,
        error: &str,
        status: Option<u16>,
        attempt: u32,
        attempts: u32,
    ) -> bool {
        parse_max_output_token_limit(error).is_some()
            || !is_retryable_error(status, error)
            || attempt + 1 >= attempts
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(CancelFlag::is_cancelled)
    }

    fn emit_retry(&self, error: &str, attempt: u32, delay: Duration) {
        let Some(hook) = &self.on_retry else {
            return;
        };
        hook(&format_retry_line(
            &redact_secrets(error),
            attempt,
            self.config.retry.max_retries,
            delay,
        ));
    }

    async fn wait_before_retry(&self, delay: Duration) -> Result<(), String> {
        let Some(cancel) = &self.cancel else {
            tokio::time::sleep(delay).await;
            return Ok(());
        };
        let mut remaining = delay;
        while remaining > Duration::ZERO {
            if cancel.is_cancelled() {
                return Err("已取消".to_string());
            }
            let slice = remaining.min(Duration::from_millis(200));
            tokio::time::sleep(slice).await;
            remaining = remaining.saturating_sub(slice);
        }
        if cancel.is_cancelled() {
            return Err("已取消".to_string());
        }
        Ok(())
    }

    fn apply_auth(&self, mut request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.config.protocol == PROTOCOL_ANTHROPIC {
            request = request
                .header("x-api-key", &self.config.api_key)
                .header("anthropic-version", "2023-06-01");
        } else {
            request = request.header("authorization", format!("Bearer {}", self.config.api_key));
        }
        for (name, value) in &self.config.extra_headers {
            if name.eq_ignore_ascii_case("authorization") || name.eq_ignore_ascii_case("x-api-key")
            {
                continue;
            }
            request = request.header(name, value);
        }
        request
    }

    async fn get_raw(&self, url: &str, query: &[(&str, &str)]) -> Result<(u16, String), String> {
        let mut request = self.http.get(url).header("accept", "application/json");
        if !query.is_empty() {
            request = request.query(query);
        }
        let response = self
            .apply_auth(request)
            .send()
            .await
            .map_err(|error| format!("拉取模型列表失败: {error}"))?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        Ok((status, text))
    }

    async fn post_raw(&self, url: &str, body: &Value) -> Result<(u16, String), String> {
        let request = self
            .http
            .post(url)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream, application/json");
        let response = self
            .apply_auth(request)
            .json(body)
            .send()
            .await
            .map_err(|error| format!("模型请求失败: {error}"))?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        Ok((status, text))
    }

    fn parse_success_body(&self, text: &str) -> Result<(Message, Usage), String> {
        let trimmed = text.trim_start_matches('\u{feff}').trim();
        if trimmed.is_empty() {
            return Err("模型返回空响应：正文为空".to_string());
        }
        if let Some(error) = extract_gateway_error(trimmed) {
            return Err(format_gateway_error(&error));
        }
        if let Ok(parsed) = self.parse_sse(trimmed) {
            return Ok(parsed);
        }
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            let payload = unwrap_gateway_payload(&value);
            if let Some(error) = json_error_message(payload) {
                return Err(format_gateway_error(&error));
            }
            if let Ok(parsed) = self.parse_complete_json(payload) {
                return Ok(parsed);
            }
        }
        Err(empty_response_error(trimmed))
    }

    fn parse_complete_json(&self, value: &Value) -> Result<(Message, Usage), String> {
        match self.config.protocol.as_str() {
            PROTOCOL_ANTHROPIC => parse_anthropic_json(value),
            PROTOCOL_CODEX => parse_responses_json(value),
            PROTOCOL_OPENAI => parse_openai_json(value),
            other => Err(format!("不支持的渠道协议: {other}")),
        }
    }
}

fn extract_gateway_error(text: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return json_error_message(&value)
            .or_else(|| json_error_message(unwrap_gateway_payload(&value)));
    }
    for event in parse_sse(text) {
        let Ok(value) = serde_json::from_str::<Value>(&event.data) else {
            continue;
        };
        if let Some(error) = json_error_message(&value) {
            return Some(error);
        }
    }
    None
}

fn json_error_message(value: &Value) -> Option<String> {
    let error = value.get("error")?;
    if error.is_null() || error.as_object().is_some_and(serde_json::Map::is_empty) {
        return None;
    }
    if let Some(text) = error.as_str() {
        let text = text.trim();
        return if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        };
    }
    if let Some(text) = error
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        return Some(text.to_string());
    }
    if let Some(text) = error
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        return Some(text.to_string());
    }
    Some("未知错误".to_string())
}

fn unwrap_gateway_payload(value: &Value) -> &Value {
    for key in ["data", "result"] {
        if let Some(inner) = value.get(key) {
            if inner.get("choices").is_some()
                || inner.get("content").is_some()
                || inner.get("output").is_some()
                || inner.get("error").is_some()
            {
                return inner;
            }
        }
    }
    value
}

fn format_gateway_error(message: &str) -> String {
    let snippet = snippet_for_error(message);
    format!("模型返回错误：{snippet}")
}

fn empty_response_error(text: &str) -> String {
    let snippet = snippet_for_error(text);
    if snippet.is_empty() {
        "模型返回空响应".to_string()
    } else {
        format!("模型返回空响应：{snippet}")
    }
}

fn snippet_for_error(text: &str) -> String {
    redact_secrets(text)
        .chars()
        .filter(|ch| *ch != '\n' && *ch != '\r')
        .take(180)
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn events_from_message(message: Message, usage: Usage) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    if !message.reasoning_content.is_empty() {
        events.push(StreamEvent::ReasoningDelta(message.reasoning_content));
    }
    if !message.content.is_empty() {
        events.push(StreamEvent::TextDelta(message.content));
    }
    for call in message.tool_calls {
        events.push(StreamEvent::ToolCall(call));
    }
    events.push(StreamEvent::Usage(usage));
    events.push(StreamEvent::Done);
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const OK_SSE: &str =
        "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";

    async fn serve_once(status: u16, body: &str) -> String {
        serve_sequence(vec![(status, body.to_string())]).await
    }

    async fn serve_sequence(responses: Vec<(u16, String)>) -> String {
        serve_sequence_counted(responses, None).await
    }

    async fn serve_sequence_counted(
        responses: Vec<(u16, String)>,
        counter: Option<Arc<AtomicU32>>,
    ) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.expect("accept");
                if let Some(counter) = &counter {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                let mut buf = vec![0u8; 16384];
                let _ = stream.read(&mut buf).await;
                let header = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(body.as_bytes()).await;
            }
        });
        format!("http://{addr}")
    }

    fn client(base_url: String) -> ModelClient {
        client_with_protocol(base_url, PROTOCOL_OPENAI)
    }

    fn client_with_protocol(base_url: String, protocol: &str) -> ModelClient {
        client_with_retry_protocol(base_url, protocol, RetryConfig::none())
    }

    fn fast_retry() -> RetryConfig {
        RetryConfig {
            max_retries: 10,
            base_delay_ms: 1,
            max_delay_ms: 1,
            jitter: false,
        }
    }

    fn client_with_retry(base_url: String, retry: RetryConfig) -> ModelClient {
        client_with_retry_protocol(base_url, PROTOCOL_OPENAI, retry)
    }

    fn client_with_retry_protocol(
        base_url: String,
        protocol: &str,
        retry: RetryConfig,
    ) -> ModelClient {
        ModelClient::new(ModelClientConfig {
            protocol: protocol.to_string(),
            base_url,
            api_key: "sk-secret-key".to_string(),
            extra_headers: HashMap::new(),
            retry,
            timeout: Duration::from_secs(5),
        })
        .expect("client")
    }

    async fn chat_hi_on(client: ModelClient) -> Result<(Message, Usage), String> {
        client
            .chat(ChatRequest {
                messages: &[Message::user("hi")],
                tools: &[],
                model: "gpt-4o",
                effort: None,
                max_output_tokens: None,
                thinking_enabled: false,
            })
            .await
    }

    #[tokio::test]
    async fn chat_parses_mock_openai_sse() {
        let base = serve_once(200, OK_SSE).await;
        let (message, _) = chat_hi_on(client(base)).await.expect("chat");
        assert_eq!(message.content, "ok");
    }

    #[tokio::test]
    async fn list_models_reads_openai_payload() {
        let body = r#"{"data":[{"id":"gpt-4o"},{"id":"o4-mini"}]}"#;
        let base = serve_once(200, body).await;
        let models = client(base).list_models().await.expect("list models");
        assert_eq!(models, vec!["gpt-4o".to_string(), "o4-mini".to_string()]);
    }

    #[tokio::test]
    async fn probe_error_hides_api_key() {
        let base = serve_once(401, "Authorization Bearer sk-secret-key denied").await;
        let error = client(base)
            .probe("gpt-4o")
            .await
            .expect_err("probe should fail");
        assert!(error.contains("HTTP 401"));
        assert!(!error.contains("sk-secret-key"));
    }

    #[tokio::test]
    async fn chat_stream_ends_with_done() {
        let base = serve_once(200, OK_SSE).await;
        let mut rx = client(base)
            .chat_stream(ChatRequest {
                messages: &[Message::user("hi")],
                tools: &[],
                model: "gpt-4o",
                effort: None,
                max_output_tokens: None,
                thinking_enabled: false,
            })
            .await
            .expect("stream");
        let mut saw_done = false;
        while let Some(event) = rx.recv().await {
            if event == StreamEvent::Done {
                saw_done = true;
            }
        }
        assert!(saw_done);
    }

    #[tokio::test]
    async fn chat_retries_when_max_tokens_exceeds_gateway_limit() {
        let error = r#"{"error":{"message":"max_tokens is too large: 384000. This model supports at most 131072 completion tokens."}}"#;
        let base = serve_sequence(vec![(400, error.to_string()), (200, OK_SSE.to_string())]).await;
        let (message, _) = client(base)
            .chat(ChatRequest {
                messages: &[Message::user("hi")],
                tools: &[],
                model: "deepseek-v4-flash",
                effort: None,
                max_output_tokens: Some(384000),
                thinking_enabled: true,
            })
            .await
            .expect("chat should retry with gateway limit");
        assert_eq!(message.content, "ok");
    }

    async fn chat_hi(base: String) -> Result<(Message, Usage), String> {
        chat_hi_on(client(base)).await
    }

    #[tokio::test]
    async fn chat_parses_non_stream_json() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"json ok"}}],"usage":{"prompt_tokens":1,"completion_tokens":2}}"#;
        let (message, usage) = chat_hi(serve_once(200, body).await)
            .await
            .expect("json chat");
        assert_eq!(message.content, "json ok");
        assert_eq!(usage.prompt_tokens, 1);
    }

    #[tokio::test]
    async fn chat_parses_wrapped_gateway_json() {
        let body = r#"{"data":{"choices":[{"message":{"content":"wrapped"}}]}}"#;
        let (message, _) = chat_hi(serve_once(200, body).await)
            .await
            .expect("wrapped json");
        assert_eq!(message.content, "wrapped");
    }

    #[tokio::test]
    async fn chat_surfaces_json_error_on_http_200() {
        let body = r#"{"error":{"message":"insufficient quota for sk-secret-key"}}"#;
        let error = chat_hi(serve_once(200, body).await)
            .await
            .expect_err("json error should fail");
        assert!(error.contains("模型返回错误"));
        assert!(error.contains("insufficient quota"));
        assert!(!error.contains("空响应"));
        assert!(!error.contains("sk-secret-key"));
    }

    #[tokio::test]
    async fn chat_empty_body_includes_empty_hint() {
        let error = chat_hi(serve_once(200, "").await)
            .await
            .expect_err("empty body");
        assert!(error.contains("模型返回空响应"));
        assert!(error.contains("正文为空"));
    }

    #[tokio::test]
    async fn chat_parses_responses_completed_json() {
        let body = r#"{"output":[{"type":"message","content":[{"type":"output_text","text":"done plan"}]}],"usage":{"input_tokens":2,"output_tokens":1}}"#;
        let (message, _) = client_with_protocol(serve_once(200, body).await, PROTOCOL_CODEX)
            .chat(ChatRequest {
                messages: &[Message::user("hi")],
                tools: &[],
                model: "gpt-5.4",
                effort: None,
                max_output_tokens: None,
                thinking_enabled: false,
            })
            .await
            .expect("responses json");
        assert_eq!(message.content, "done plan");
    }

    #[tokio::test]
    async fn chat_retries_http_503_then_succeeds() {
        let counter = Arc::new(AtomicU32::new(0));
        let base = serve_sequence_counted(
            vec![(503, "busy".to_string()), (200, OK_SSE.to_string())],
            Some(counter.clone()),
        )
        .await;
        let lines = Arc::new(Mutex::new(Vec::new()));
        let captured = lines.clone();
        let (message, _) = chat_hi_on(client_with_retry(base, fast_retry()).with_retry_hook(
            Arc::new(move |line: &str| {
                captured.lock().expect("retry lines").push(line.to_string());
            }),
        ))
        .await
        .expect("503 then success");
        assert_eq!(message.content, "ok");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        let lines = lines.lock().expect("retry lines");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("[重试]"));
        assert!(lines[0].contains("1/10"));
        assert!(lines[0].contains("HTTP 503"));
    }

    #[tokio::test]
    async fn chat_retries_http_200_gateway_error_then_succeeds() {
        let error = r#"{"error":{"message":"overloaded"}}"#;
        let base = serve_sequence(vec![(200, error.to_string()), (200, OK_SSE.to_string())]).await;
        let (message, _) = chat_hi_on(client_with_retry(base, fast_retry()))
            .await
            .expect("gateway error then success");
        assert_eq!(message.content, "ok");
    }

    #[tokio::test]
    async fn chat_retries_http_200_empty_then_succeeds() {
        let base = serve_sequence(vec![(200, String::new()), (200, OK_SSE.to_string())]).await;
        let (message, _) = chat_hi_on(client_with_retry(base, fast_retry()))
            .await
            .expect("empty then success");
        assert_eq!(message.content, "ok");
    }

    #[tokio::test]
    async fn chat_retry_none_does_not_retry() {
        let counter = Arc::new(AtomicU32::new(0));
        let base =
            serve_sequence_counted(vec![(503, "busy".to_string())], Some(counter.clone())).await;
        let error = chat_hi_on(client_with_retry(base, RetryConfig::none()))
            .await
            .expect_err("none should fail immediately");
        assert!(error.contains("HTTP 503"));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn chat_retry_line_redacts_api_key() {
        let body = "Authorization: Bearer sk-secret-key gateway busy";
        let base = serve_sequence(vec![(503, body.to_string()), (200, OK_SSE.to_string())]).await;
        let lines = Arc::new(Mutex::new(Vec::new()));
        let captured = lines.clone();
        chat_hi_on(
            client_with_retry(base, fast_retry()).with_retry_hook(Arc::new(move |line: &str| {
                captured.lock().expect("retry lines").push(line.to_string());
            })),
        )
        .await
        .expect("retry then success");
        let lines = lines.lock().expect("retry lines");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("[重试]"));
        assert!(!lines[0].contains("sk-secret-key"));
        assert!(!lines[0].to_ascii_lowercase().contains("bearer sk"));
    }

    #[tokio::test]
    async fn chat_does_not_retry_unauthorized() {
        let counter = Arc::new(AtomicU32::new(0));
        let base =
            serve_sequence_counted(vec![(401, "denied".to_string())], Some(counter.clone())).await;
        let lines = Arc::new(Mutex::new(Vec::new()));
        let captured = lines.clone();
        let error = chat_hi_on(
            client_with_retry(base, fast_retry()).with_retry_hook(Arc::new(move |line: &str| {
                captured.lock().expect("retry lines").push(line.to_string());
            })),
        )
        .await
        .expect_err("401 should fail");
        assert!(error.contains("HTTP 401"));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(lines.lock().expect("retry lines").is_empty());
    }

    #[tokio::test]
    async fn chat_max_tokens_limit_skips_http_retry_budget() {
        let error = r#"{"error":{"message":"max_tokens is too large: 384000. This model supports at most 131072 completion tokens."}}"#;
        let counter = Arc::new(AtomicU32::new(0));
        let base = serve_sequence_counted(
            vec![(400, error.to_string()), (200, OK_SSE.to_string())],
            Some(counter.clone()),
        )
        .await;
        let (message, _) = client_with_retry(base, fast_retry())
            .chat(ChatRequest {
                messages: &[Message::user("hi")],
                tools: &[],
                model: "deepseek-v4-flash",
                effort: None,
                max_output_tokens: Some(384000),
                thinking_enabled: true,
            })
            .await
            .expect("max_tokens one-shot retry");
        assert_eq!(message.content, "ok");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn chat_cancels_during_retry_wait() {
        let base = serve_once(503, "busy").await;
        let cancel = CancelFlag::new();
        let client = client_with_retry(
            base,
            RetryConfig {
                max_retries: 10,
                base_delay_ms: 2_000,
                max_delay_ms: 2_000,
                jitter: false,
            },
        )
        .with_cancel(cancel.clone());
        let task = tokio::spawn(async move { chat_hi_on(client).await });
        tokio::time::sleep(Duration::from_millis(80)).await;
        cancel.cancel();
        let error = task
            .await
            .expect("join")
            .expect_err("retry wait should cancel");
        assert_eq!(error, "已取消");
    }
}
