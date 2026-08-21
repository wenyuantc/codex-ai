# Design · 第五引擎接入

## Manager

```rust
struct NativeLiveSession {
    employee_id: String,
    task_id: Option<String>,
    session_record_id: String,
    cancel: CancellationToken,
    followup_tx: mpsc::Sender<String>,
    join: JoinHandle<()>,
}
```

事件：复用现有前端监听或新增 `native-output` 与 Codex 同形状，前端 ui 子任务再接。本任务至少 emit 与 `onCodexOutput` 类似的 payload，避免 ui 子任务发明新协议。

## 禁止

漏改 `normalize_employee_ai_provider`、`start_employee_execution_session`、`review.rs` 里 `else { start_codex }`。

## SSH

`ExecutionContext` 已分辨 local/ssh。Runner 选 LocalFs/SshFs。
