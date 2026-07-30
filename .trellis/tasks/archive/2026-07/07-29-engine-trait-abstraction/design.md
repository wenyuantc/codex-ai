# Design: AI 引擎共享内核

## 1. 目标边界

| 纳入 | 不纳入 |
|------|--------|
| `manager` 进程表 | 四引擎 `start_*` 统一调度 |
| `lifecycle` Child 封装 | `stream.rs` 协议合并 |
| `context` 执行目录解析 | `process/mod.rs` 全量启动逻辑合并 |
| final-status 纯函数（可选） | C6 巨型文件拆分 |
| 共享内核 + manager 缺口测试 | 前端 / DB schema |

## 2. 模块布局

```text
src-tauri/src/
├── engine/                      # 新增共享内核
│   ├── mod.rs
│   ├── context.rs               # ExecutionContext + resolve_*
│   ├── child.rs                 # EngineChild
│   ├── manager.rs               # ProcessManager<SessionKind, Extra>
│   ├── status.rs                # resolve_final_session_status（可选）
│   └── tests（cfg 于各文件或 tests.rs）
├── claude|grok|codex|opencode/
│   ├── manager.rs               # 薄 re-export / 类型别名 + 引擎特有扩展
│   └── process/
│       ├── lifecycle.rs         # re-export EngineChild 或 newtype
│       ├── context.rs           # re-export + 引擎 label 常量
│       ├── stream.rs            # 保留各自实现
│       ├── session_runtime.rs   # 保留；可改用共享 status helper
│       └── mod.rs               # 启动路径保留
└── lib.rs                       # `mod engine;`
```

命名采用 `engine` 而非 `ai_engine`，与现有 `codex/claude/...` 短模块风格一致。

## 3. 核心类型与 trait

### 3.1 执行上下文（R1）

```rust
// engine/context.rs
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionContext {
    pub execution_target: String,
    pub working_dir: Option<String>,
    pub ssh_config_id: Option<String>,
    pub target_host_label: Option<String>,
    pub artifact_capture_mode: String,
}

pub async fn resolve_session_execution_context<R: Runtime>(
    app: &AppHandle<R>,
    task_id: Option<&str>,
    working_dir: Option<&str>,
    engine_label: &str, // "Claude" | "Grok" | "Codex" | "OpenCode"
) -> Result<ExecutionContext, String>;
```

- SQL 与分支逻辑只保留一份（现 claude/grok/codex 同构）。
- 错误串：`format!("当前 SSH 项目缺少 ssh_config_id，无法启动 {engine_label}。")`。
- Codex 额外的 `resolve_project_execution_context` / `resolve_one_shot_working_dir` 迁入 `engine/context.rs`（`pub(crate)`）或保留在 `codex/process/context.rs` 并调用共享基础函数；优先「共享基础 + codex 扩展函数同文件」。

### 3.2 子进程（R2）

```rust
// engine/child.rs
pub struct EngineChild {
    child: tokio::process::Child,
}

impl EngineChild {
    pub fn new(child: tokio::process::Child) -> Self;
    pub fn kill_process_group(&mut self) -> Result<(), String>; // unix: killpg
    pub async fn kill(&mut self) -> Result<(), String>;
    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, String>;
    pub fn take_stdout(&mut self) -> Option<ChildStdout>;
    pub fn take_stderr(&mut self) -> Option<ChildStderr>;
}
```

- 错误文案使用中性「AI 引擎进程」或接受 `label: &str` 参数；为少改调用点，默认中性文案，引擎 newtype 可映射。
- **OpenCode 适配**：当前 `OpenCodeChild` 在 `new` 时同时持有已 take 的 stdout/stderr。两种兼容策略（实现时选改动更小者）：
  1. `EngineChild` 增加可选预置 handles；或
  2. OpenCode 在 spawn 后立即 `take_stdout/stderr` 存本地，内部仍用 `EngineChild` 管 kill/wait。
- **Codex `try_wait` 返回 `Option<i32>`**：在共享层统一为 `ExitStatus`，Codex 调用处 `.code()` 适配（行为不变）。

### 3.3 进程 Manager（R3）

```rust
// engine/manager.rs
#[derive(Clone)]
pub struct ManagedProcess<SessionKind, Extra = ()> {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_kind: SessionKind,
    pub child: Arc<Mutex<EngineChild>>,
    pub session_record_id: String,
    pub cleanup_paths: Vec<PathBuf>,
    pub extra: Extra,
}

pub struct ProcessManager<SessionKind, Extra = ()> {
    processes: HashMap<String, ManagedProcess<SessionKind, Extra>>,
}

impl<SessionKind: Copy + Eq, Extra> ProcessManager<SessionKind, Extra> {
    pub fn new() -> Self;
    pub fn add_process(...);
    pub fn remove_process(...);
    pub fn get_process(...);
    pub fn get_employee_processes(...);
    pub fn has_employee_processes(...);
    pub fn get_task_process_any(...); // task_id + session_kind
}
```

引擎适配：

| 引擎 | SessionKind | Extra | 额外 API |
|------|-------------|-------|----------|
| Claude | `ClaudeSessionKind` | `()` | 无 |
| Grok | `GrokSessionKind` | `()` | 无 |
| Codex | `CodexSessionKind` | `CodexProcessExtra { provider, execution_change_baseline, sdk_file_change_store }` | `get_processes`；测试用 `get_task_process(employee, task, kind)` |
| OpenCode | `OpenCodeSessionKind` | `()` | `sdk_server` 字段挂在 `OpenCodeManager` 包装结构，不塞进 generic Extra 强扭 |

公开类型保持兼容：

```rust
// claude/manager.rs
pub type ClaudeManager = ProcessManager<ClaudeSessionKind>;
pub type ManagedClaudeProcess = ManagedProcess<ClaudeSessionKind>;
// 或 newtype + Deref，若需在 impl 块挂引擎方法
```

若 `add_process` 签名因 Extra 变化，仅在各引擎内部调用点改；Tauri State 类型名不变。

### 3.4 Trait 契约（R4）

推荐**小而真**的 trait，避免假大空 `AiEngine`：

```rust
/// 引擎进程注册表最小契约（非 dyn 调度）
pub trait EngineProcessRegistry {
    type SessionKind: Copy + Eq;
    fn has_employee_processes(&self, employee_id: &str) -> bool;
    // … 与 ProcessManager 方法对齐的关联类型实现
}

impl<SK: Copy + Eq, E> EngineProcessRegistry for ProcessManager<SK, E> { ... }
```

可选第二 trait（若值得文档化 Child）：

```rust
pub trait EngineProcessHandle {
    fn kill_process_group(&mut self) -> Result<(), String>;
    // ...
}
impl EngineProcessHandle for EngineChild { ... }
```

**不**引入 `async trait AiEngine { async fn start(...) }`——`task_automation` 继续按 `ai_provider` 分支调用 `start_claude_with_manager` 等，避免本任务扩散到 3k 行自动化状态机。

「AiEngine」在本任务中的含义：**共享进程/上下文内核 + 显式 trait 契约**，而非统一启动 facade。

### 3.5 final status（R6）

```rust
// engine/status.rs
pub fn resolve_final_session_status(
    current_status: Option<&str>,
    exit_code: Option<i32>,
) -> &'static str {
    match (current_status, exit_code) {
        (Some("stopping"), _) => "exited",
        (_, Some(0)) => "exited",
        _ => "failed",
    }
}
```

Claude / Grok session_runtime 删除本地副本并调用之；OpenCode 已有类似逻辑可对齐（注意 bridge_error 特例保留在 OpenCode）。

## 4. 迁移顺序（降低破窗风险）

```text
1. 新增 engine/ + 单测（先绿）
2. claude context/lifecycle/manager → 共享（行为不变，最易 diff）
3. grok 同上（应接近空 diff）
4. codex context 合并；CodexChild → EngineChild；Manager Extra
5. opencode context 删除薄封装；Child/Manager 适配
6. 可选 status helper
7. 补 Claude/Grok manager 测试；跑全量 cargo test
8. 更新 spec
```

每步可独立 `cargo test`；任一步失败回滚该步。

## 5. 兼容性

| 面 | 策略 |
|----|------|
| Tauri 命令 / 事件名 | 不变 |
| `ai_provider` | 不变 |
| Manager 类型名 | 保留 public 名 |
| SSH / local | context 单源，禁止弱化校验 |
| OpenCode SDK server | 仍在 OpenCodeManager |
| Codex file-change baseline | `CodexProcessExtra` |

## 6. 风险与回滚

| 风险 | 缓解 |
|------|------|
| OpenCode Child 语义分叉 | 适配层而非强改 spawn |
| Codex Extra 字段漏传 | 编译期强制 Extra 构造 |
| 大 diff 难审 | 按迁移顺序提交（可选 1–2 个逻辑 commit） |
| 误合并 stream | 明确 out of scope + review checklist |

回滚：整分支 revert；共享模块无迁移，不改 schema。

## 7. 与父任务 / 兄弟任务关系

- **C6 split-large-modules**：不碰 `git_workflow` / `task_automation` 拆分；本任务只动引擎目录 + `lib.rs` + spec。
- **C7 lint**：本任务不追求 clippy 全仓清零；新增代码保持可维护即可。
- 父任务 AC：合并后更新 CLAUDE.md 中「零 trait」描述。

## 8. 测试设计

| 用例 | 位置 |
|------|------|
| Manager 同员工多 task 会话 | `engine/manager.rs` 或引擎 re-export 测试 |
| `has_employee_processes` / `get_task_process_any` | 同上 |
| context：local repo / SSH 缺 config / SSH 正常 / 显式 working_dir 覆盖 SSH | `engine/context.rs`（mock pool 若成本过高则抽纯函数测分支；否则复用 app test pool） |
| final status 三分支 | `engine/status.rs` |
| Claude/Grok manager 对等用例 | 各 `manager.rs` 或共享参数化 |
| 现有 stream/CLI args 测试 | 适配 import 后全保留 |

优先纯函数；避免 spawn 真实 CLI。现有 manager 测试用 `sleep 10` 子进程的模式可复用。
