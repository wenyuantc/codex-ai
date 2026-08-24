use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::native::protocol::{
    channel_chat_url, channel_models_url, model_list_next_page, parse_model_list_json,
    PROTOCOL_ANTHROPIC, PROTOCOL_CODEX, PROTOCOL_OPENAI,
};

use super::anthropic::{build_anthropic_body, parse_anthropic_json, parse_anthropic_sse};
use super::openai::{
    build_openai_body, parse_max_output_token_limit, parse_openai_json, parse_openai_sse,
};
use super::responses::{build_responses_body, parse_responses_json, parse_responses_sse};
use super::retry::{format_http_error, is_retryable_status, redact_secrets, RetryConfig};
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

pub struct ModelClient {
    http: reqwest::Client,
    config: ModelClientConfig,
}

impl ModelClient {
    pub fn new(config: ModelClientConfig) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|error| format!("创建 HTTP 客户端失败: {error}"))?;
        Ok(Self { http, config })
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
            match self.post_raw(url, body).await {
                Ok((status, text)) if (200..300).contains(&status) => {
                    return self.parse_success_body(&text);
                }
                Ok((status, text)) => {
                    last_error = format_http_error(status, url, &text);
                    if !is_retryable_status(status) || attempt + 1 >= attempts {
                        return Err(last_error);
                    }
                }
                Err(error) => {
                    last_error = error;
                    if attempt + 1 >= attempts {
                        return Err(last_error);
                    }
                }
            }
            tokio::time::sleep(self.config.retry.delay_for_attempt(attempt)).await;
        }
        Err(last_error)
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn serve_once(status: u16, body: &str) -> String {
        serve_sequence(vec![(status, body.to_string())]).await
    }

    async fn serve_sequence(responses: Vec<(u16, String)>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.expect("accept");
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
        ModelClient::new(ModelClientConfig {
            protocol: protocol.to_string(),
            base_url,
            api_key: "sk-secret-key".to_string(),
            extra_headers: HashMap::new(),
            retry: RetryConfig::none(),
            timeout: Duration::from_secs(5),
        })
        .expect("client")
    }

    #[tokio::test]
    async fn chat_parses_mock_openai_sse() {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
        let base = serve_once(200, sse).await;
        let (message, _) = client(base)
            .chat(ChatRequest {
                messages: &[Message::user("hi")],
                tools: &[],
                model: "gpt-4o",
                effort: None,
                max_output_tokens: None,
                thinking_enabled: false,
            })
            .await
            .expect("chat");
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
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
        let base = serve_once(200, sse).await;
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
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
        let base = serve_sequence(vec![(400, error.to_string()), (200, sse.to_string())]).await;
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
        client(base)
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
}
