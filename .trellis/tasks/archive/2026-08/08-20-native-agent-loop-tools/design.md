# Design · Agent 循环与核心工具

参考 zcli `internal/agent/loop.go`、`internal/tools/*`。

## Runner

`native/agent/loop.rs`：加载 messages → chat_stream → 收集 tool_calls → 并行或串行执行（第一期串行更简单）→ 追加 tool 消息 → 重复。MaxTurns 默认 40，0=不限制。

事件：text / thinking / tool_start / tool_end / error / done / usage。

## Tools

`native/tools/`：每个工具一个文件 + `WorkspaceFs` trait。

```rust
#[async_trait]
trait WorkspaceFs {
    async fn read(&self, path: &Path) -> Result<String, String>;
    async fn write(&self, path: &Path, contents: &str) -> Result<(), String>;
    async fn glob(&self, pattern: &str) -> Result<Vec<String>, String>;
    async fn grep(&self, pattern: &str, path: &str) -> Result<String, String>;
    async fn bash(&self, cmd: &str, timeout: Duration) -> Result<String, String>;
}
```

`LocalFs` / `SshFs`。SshFs 持有 AppHandle + SshConfig + remote cwd。Grep 远程用 `rg` 若无则 `grep -R`。

路径：相对工作区 normalize + `starts_with(workspace_root)`。
