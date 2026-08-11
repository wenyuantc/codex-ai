//! Shared mid-session stdin framing for interactive AI engine sessions.

use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;

/// Encode a bootstrap JSON object as a single NDJSON line (no EOF).
pub fn encode_ndjson_line(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| format!("序列化会话 stdin 载荷失败: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Encode a follow-up user input as an NDJSON line for live session bridges.
pub fn encode_session_followup_input(input: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("输入内容不能为空".to_string());
    }
    encode_ndjson_line(&serde_json::json!({
        "type": "input",
        "prompt": trimmed,
    }))
}

/// Ask a session bridge to wait for follow-ups after the current turn (log UI open).
pub fn encode_await_followups_control(enabled: bool) -> Result<Vec<u8>, String> {
    encode_ndjson_line(&serde_json::json!({
        "type": "await_followups",
        "enabled": enabled,
    }))
}

/// Write bytes to a live session stdin and flush (keep the pipe open).
pub async fn write_stdin_bytes(stdin: &mut ChildStdin, bytes: &[u8]) -> Result<(), String> {
    stdin
        .write_all(bytes)
        .await
        .map_err(|error| format!("写入会话 stdin 失败: {error}"))?;
    stdin
        .flush()
        .await
        .map_err(|error| format!("刷新会话 stdin 失败: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_input_followup_is_single_ndjson_line() {
        let bytes = encode_session_followup_input("  hello world  ").expect("encode");
        assert!(bytes.ends_with(b"\n"));
        let line = std::str::from_utf8(&bytes[..bytes.len() - 1]).expect("utf8");
        let value: serde_json::Value = serde_json::from_str(line).expect("json");
        assert_eq!(value["type"], "input");
        assert_eq!(value["prompt"], "hello world");
        assert!(!line.contains('\n'));
    }

    #[test]
    fn send_input_followup_rejects_blank() {
        let error = encode_session_followup_input("   ").expect_err("blank");
        assert!(error.contains("不能为空"));
    }

    #[test]
    fn bootstrap_line_is_compact_json_plus_newline() {
        let bytes = encode_ndjson_line(&serde_json::json!({"mode":"session","prompt":"x"}))
            .expect("encode");
        assert_eq!(bytes.last().copied(), Some(b'\n'));
        let parsed: serde_json::Value =
            serde_json::from_slice(&bytes[..bytes.len() - 1]).expect("json");
        assert_eq!(parsed["mode"], "session");
    }

    #[test]
    fn await_followups_control_line() {
        let bytes = encode_await_followups_control(true).expect("encode");
        let line = std::str::from_utf8(&bytes[..bytes.len() - 1]).expect("utf8");
        let value: serde_json::Value = serde_json::from_str(line).expect("json");
        assert_eq!(value["type"], "await_followups");
        assert_eq!(value["enabled"], true);
    }
}
