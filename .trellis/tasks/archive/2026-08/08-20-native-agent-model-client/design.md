# Design · 三协议模型客户端

参考 zcli `internal/model/{client,stream,retry}.go`。新增 Responses 适配。

## Trait

```rust
#[async_trait]
trait ModelClient {
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
        model: &str,
    ) -> Result<mpsc::Receiver<StreamEvent>, String>;
}
```

`StreamEvent`: TextDelta, ReasoningDelta, ToolCall, Usage, Done, Error.

## 请求构造

- openai: `messages[]` + `tools[]` function；`stream: true` `stream_options.include_usage`
- anthropic: system 抽出；`tools` 用 anthropic 形状；`stream: true`
- responses: `input` 为 item 列表；`instructions` 放 system；tools 为 function；上一轮 function_call_output 追加

Effort：openai `reasoning_effort`；anthropic thinking budget 若渠道需要可先忽略未知字段；responses `reasoning.effort`。第一期尽力发送，网关忽略未知字段可接受。

## 文件

`src-tauri/src/native/model/{mod,types,openai,anthropic,responses,retry}.rs`
