use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay_ms: 2_000,
            max_delay_ms: 60_000,
            jitter: true,
        }
    }
}

impl RetryConfig {
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            base_delay_ms: 1,
            max_delay_ms: 1,
            jitter: false,
        }
    }

    pub fn delay_for_attempt(self, attempt: u32) -> Duration {
        let factor = 1u64.checked_shl(attempt.min(16)).unwrap_or(u64::MAX);
        let mut delay = self.base_delay_ms.saturating_mul(factor);
        if delay > self.max_delay_ms {
            delay = self.max_delay_ms;
        }
        if self.jitter && delay > 1 {
            let jittered = delay / 2 + (delay % 7) + 1;
            delay = jittered.min(self.max_delay_ms);
        }
        Duration::from_millis(delay.max(1))
    }
}

pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 429) || status >= 500
}

pub fn redact_secrets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let lower = text.to_ascii_lowercase();
    let mut i = 0;
    while i < text.len() {
        if lower[i..].starts_with("bearer ") {
            out.push_str("[redacted]");
            i += "bearer ".len();
            i = skip_token(text, i);
            continue;
        }
        if lower[i..].starts_with("sk-") {
            out.push_str("[redacted]");
            i = skip_token(text, i);
            continue;
        }
        let next = text[i..].chars().next().unwrap_or(' ');
        out.push(next);
        i += next.len_utf8();
    }
    out
}

fn skip_token(text: &str, mut i: usize) -> usize {
    while i < text.len() {
        let Some(ch) = text[i..].chars().next() else {
            break;
        };
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    i
}

pub fn format_http_error(status: u16, url: &str, body: &str) -> String {
    let snippet: String = redact_secrets(body)
        .chars()
        .filter(|ch| *ch != '\n' && *ch != '\r')
        .take(180)
        .collect();
    let host = redact_secrets(url.split('?').next().unwrap_or(url));
    if snippet.trim().is_empty() {
        format!("模型请求失败（HTTP {status}）: {host}")
    } else {
        format!("模型请求失败（HTTP {status}）: {snippet}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_rate_limits_and_server_errors() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
    }

    #[test]
    fn http_error_does_not_echo_authorization() {
        let message = format_http_error(
            401,
            "https://api.example.com/v1/chat/completions",
            "Authorization: Bearer sk-secret-key invalid",
        );
        assert!(message.contains("HTTP 401"));
        assert!(!message.contains("sk-secret-key"));
        assert!(!message.to_ascii_lowercase().contains("bearer sk"));
    }
}
