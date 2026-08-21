# Design · 内置 Agent（zcli 移植）

## Boundaries

- 新模块 `src-tauri/src/native/` 拥有渠道、模型客户端、agent 循环、工具、进程内 manager。
- 不引入 `dyn AiEngine`。各引擎仍有自己的 `start_*`。
- 不把 native 会话登记进 `engine::ProcessManager`（那是 `EngineChild`）。`NativeAgentManager` 独立 HashMap。
- 复用：`ExecutionContext`、`validate_runtime_working_dir`、`codex_sessions`、`apply_codex_session_usage`、`insert_activity_log`、`build_ssh_command`、run queue 计数接口。

## Data flow

```
Settings CRUD → ai_channels + keyring (service: codex-ai-channel)
Employee native + ai_channel_id + model
  → start_native_session
  → NativeAgentManager (cancel + followup mpsc + JoinHandle)
  → agent::Runner ↔ model::Client (openai | anthropic | responses)
  → tools LocalFs | SshFs
  → codex_sessions / events / usage / 现有 Session 日志 UI
```

## Schema (v48)

`ai_channels`: id, name, protocol (`openai|anthropic|codex`), base_url, api_key_ref, extra_headers_json, models_json, enabled, created_at, updated_at.

`employees.ai_channel_id` TEXT NULL。仅 `ai_provider=native` 时必填。

DTO 对外永不返回明文 key；`api_key_configured: bool`。

## Protocols

| id | HTTP |
|---|---|
| openai | `POST {base}/v1/chat/completions` SSE |
| anthropic | `POST {base}/v1/messages` SSE，`anthropic-version: 2023-06-01` |
| codex | `POST {base}/v1/responses` SSE；input items + function_call / function_call_output |

`base_url` 去尾 `/`。默认 path 由协议拼接。

## Process-in-session vs EngineChild

Native live count 必须纳入 `run_queue`（与四引擎进程表相加）。冲突检测与 `get_employee_runtime_status` 增加 native 分支，禁止 `else → codex`。

send_input：mpsc 送到 live runner。任务会话 `await_followups=false`（turn 结束退出）；自由会话可等待。

## SSH tools

同一 Tool trait。Local：std fs / tokio process。SSH：`build_ssh_command` 跑远程 `cat` / 写入 / `rg` / `bash`。workspace-only 用远程工作目录做前缀检查。

## Compatibility

- `normalize_employee_ai_provider` 必须认识 `native`，否则静默变 Codex。
- 能力矩阵测试从 4 个改为 5 个。
- 导出 SQL 不含密钥环。
- 前端 `AiProvider` 联合类型扩展；未知值不得把 native 显示成 Codex。
