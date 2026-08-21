//! Shared token usage parsing for all engines.
//!
//! Engines report usage in slightly different JSON shapes; this module
//! normalizes them into [`UsageDelta`] with multi-key fallbacks. Values that
//! cannot be parsed stay `None` (unknown is never coerced to zero).

use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UsageDelta {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
}

impl UsageDelta {
    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.total_tokens.is_none()
            && self.reasoning_tokens.is_none()
            && self.cached_tokens.is_none()
    }

    /// 终端展示行，例如 `[用量] in=812 out=45 reason=12 total=857`。
    pub fn format_terminal_line(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }

        let mut parts = Vec::new();
        if let Some(value) = self.input_tokens {
            parts.push(format!("in={value}"));
        }
        if let Some(value) = self.output_tokens {
            parts.push(format!("out={value}"));
        }
        if let Some(value) = self.reasoning_tokens {
            if value > 0 {
                parts.push(format!("reason={value}"));
            }
        }
        if let Some(value) = self.cached_tokens {
            if value > 0 {
                parts.push(format!("cache={value}"));
            }
        }
        if let Some(value) = self.total_tokens {
            parts.push(format!("total={value}"));
        }
        if parts.is_empty() {
            return None;
        }
        Some(format!("[用量] {}", parts.join(" ")))
    }
}

pub fn usage_u64(usage: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        usage.get(*key).and_then(|item| {
            item.as_u64()
                .or_else(|| item.as_i64().map(|v| v.max(0) as u64))
                .or_else(|| {
                    item.as_f64().map(|v| {
                        if v.is_finite() && v >= 0.0 {
                            v as u64
                        } else {
                            0
                        }
                    })
                })
        })
    })
}

/// 从 JSON 值提取 token 用量。`value` 既可以是包含 `usage`/`data` 字段的事件，
/// 也可以直接是 usage 对象本身。全部字段都解析不到时返回 `None`。
pub fn parse_usage_value(value: &Value) -> Option<UsageDelta> {
    let usage = value
        .get("usage")
        .or_else(|| value.get("data"))
        .unwrap_or(value);

    let input = usage_u64(
        usage,
        &["input_tokens", "inputTokens", "prompt_tokens", "input"],
    );
    let output = usage_u64(
        usage,
        &[
            "output_tokens",
            "outputTokens",
            "completion_tokens",
            "output",
        ],
    );
    let total = usage_u64(usage, &["total_tokens", "totalTokens", "total"]).or_else(|| {
        match (input, output) {
            (Some(i), Some(o)) => Some(i + o),
            _ => None,
        }
    });
    let reasoning = usage_u64(
        usage,
        &[
            "reasoning_tokens",
            "reasoningTokens",
            "thoughtTokens",
            "reasoning",
        ],
    );
    let cached = usage_u64(
        usage,
        &[
            "cached_tokens",
            "cachedTokens",
            "prompt_cache_hit_tokens",
            "cache_read_input_tokens",
            "cache_read_tokens",
        ],
    )
    .or_else(|| {
        ["prompt_tokens_details", "input_tokens_details"]
            .iter()
            .find_map(|key| {
                usage
                    .get(*key)
                    .and_then(|details| usage_u64(details, &["cached_tokens", "cachedTokens"]))
            })
    });

    let delta = UsageDelta {
        input_tokens: input,
        output_tokens: output,
        total_tokens: total,
        reasoning_tokens: reasoning,
        cached_tokens: cached,
    };

    if delta.is_empty() {
        None
    } else {
        Some(delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_nested_usage_object() {
        let value = json!({
            "type": "usage",
            "usage": {
                "input_tokens": 812,
                "output_tokens": 45,
                "reasoning_tokens": 12
            }
        });

        let delta = parse_usage_value(&value).expect("usage");
        assert_eq!(delta.input_tokens, Some(812));
        assert_eq!(delta.output_tokens, Some(45));
        assert_eq!(delta.total_tokens, Some(857));
        assert_eq!(delta.reasoning_tokens, Some(12));
        assert_eq!(delta.cached_tokens, None);
    }

    #[test]
    fn parses_bare_usage_object_with_camel_case_keys() {
        let value = json!({ "inputTokens": 10, "outputTokens": 20 });

        let delta = parse_usage_value(&value).expect("usage");
        assert_eq!(delta.input_tokens, Some(10));
        assert_eq!(delta.output_tokens, Some(20));
        assert_eq!(delta.total_tokens, Some(30));
        assert_eq!(delta.reasoning_tokens, None);
        assert_eq!(delta.cached_tokens, None);
    }

    #[test]
    fn returns_none_when_no_token_fields_present() {
        let value = json!({ "type": "usage", "usage": { "note": "n/a" } });
        assert!(parse_usage_value(&value).is_none());
    }

    #[test]
    fn keeps_partial_fields_without_faking_zero() {
        let value = json!({ "usage": { "output_tokens": 7 } });
        let delta = parse_usage_value(&value).expect("usage");
        assert_eq!(delta.input_tokens, None);
        assert_eq!(delta.output_tokens, Some(7));
        assert_eq!(delta.total_tokens, None);
    }

    #[test]
    fn parses_opencode_native_input_output_keys() {
        let value = json!({ "input": 40, "output": 12, "reasoning": 3 });
        let delta = parse_usage_value(&value).expect("usage");
        assert_eq!(delta.input_tokens, Some(40));
        assert_eq!(delta.output_tokens, Some(12));
        assert_eq!(delta.reasoning_tokens, Some(3));
        assert_eq!(delta.total_tokens, Some(52));
        assert_eq!(delta.cached_tokens, None);
    }

    #[test]
    fn formats_terminal_line() {
        let delta = UsageDelta {
            input_tokens: Some(100),
            output_tokens: Some(50),
            total_tokens: Some(150),
            reasoning_tokens: Some(0),
            cached_tokens: Some(0),
        };
        assert_eq!(
            delta.format_terminal_line().as_deref(),
            Some("[用量] in=100 out=50 total=150")
        );
    }

    #[test]
    fn parses_cache_hit_keys_and_nested_details() {
        let grok = parse_usage_value(&json!({
            "usage": {
                "input_tokens": 812,
                "output_tokens": 45,
                "cache_read_input_tokens": 200,
                "cache_creation_input_tokens": 50
            }
        }))
        .expect("grok usage");
        assert_eq!(grok.cached_tokens, Some(200));

        let openai = parse_usage_value(&json!({
            "prompt_tokens": 10,
            "completion_tokens": 4,
            "prompt_tokens_details": { "cached_tokens": 3 }
        }))
        .expect("openai usage");
        assert_eq!(openai.input_tokens, Some(10));
        assert_eq!(openai.cached_tokens, Some(3));
        assert_eq!(
            openai.format_terminal_line().as_deref(),
            Some("[用量] in=10 out=4 cache=3 total=14")
        );
    }
}
