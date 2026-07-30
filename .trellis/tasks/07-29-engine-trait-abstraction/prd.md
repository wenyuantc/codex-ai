# AI 引擎 trait 抽象与测试补齐

## Goal

消除 codex / claude / grok / opencode 在 **进程注册表（manager）**、**子进程封装（lifecycle）**、**执行上下文解析（context）** 上的复制粘贴，抽出共享内核；各引擎只保留协议差异（以 `stream.rs` 与启动参数组装为主）。同步把共享内核与非 codex 引擎的关键缺口测试补齐，使同类 bug 只需修一处。

用户价值：后续修会话生命周期 / SSH 工作目录 / 进程停止逻辑时不再四份漏改；新增第 5 个 CLI 引擎时有明确复用面。

## Background（实测）

父任务源发现 #3 + #8。2026-07-30 本分支实测：

| 对比 | manager | lifecycle | context | session_runtime | stream |
|------|---------|-----------|---------|-----------------|--------|
| claude vs grok | **100%** | **100%** | **97.9%** | 86.6% | 25.2% |
| claude vs opencode | 54% | 41% | 12%* | 2% | 9.5% |
| claude vs codex | 63% | 11%** | 66% | 12% | 15% |

\* OpenCode 的 `context.rs` 已是对 codex context 的薄封装。  
\*\* Codex `lifecycle.rs` 还承载 stop/validate/file-change 等大量 Codex 专有逻辑，与 Child 封装混在同一文件。

| 引擎 | 现有 `#[test]` 约数 | 主要覆盖 |
|------|---------------------|----------|
| codex | ~95 | manager / stream / one-shot / settings / secrets… |
| claude | ~15 | CLI args / stream / final status / settings |
| grok | ~19 | CLI args / stream / final status / settings |
| opencode | ~15 | manager / bridge parse / final status / settings |

全仓库 `pub trait` 计数仍为 **0**。

父任务顺序约束：本任务为 **C5**，高风险内部重构，独立分支 `feat/engine-trait-abstraction`；不依赖未完成的 C6 巨型文件拆分。

## Requirements

### R1 — 共享执行上下文

- 单一 `ExecutionContext` 与 `resolve_task_project_execution_context` / `resolve_session_execution_context` 实现，供四引擎使用。
- 错误文案中的引擎显示名通过参数注入（「无法启动 Claude/Grok/Codex…」），行为与现网一致。
- OpenCode 删除重复薄封装，直接复用共享 API。
- Codex 专有扩展（如 `resolve_project_execution_context` / `resolve_one_shot_working_dir`）保留在 codex 侧或迁入共享模块但仅 codex 调用。

### R2 — 共享子进程封装（lifecycle 内核）

- 抽出统一的 `EngineChild`（或等价命名），提供：`new`、`kill_process_group`、`kill`、`try_wait`、`take_stdout` / `take_stderr`。
- Claude / Grok 删除各自 100% 同构的 `*Child` 实现，改为类型别名或 newtype。
- Codex / OpenCode 在**不改变对外行为**前提下接入：OpenCode 若需构造时预取 stdout/stderr，用共享封装的等价能力实现，不在本任务重写 OpenCode 启动协议。

### R3 — 共享进程注册表（manager 内核）

- 抽出泛型或参数化的进程 Manager：`add/remove/get_process`、`get_employee_processes`、`has_employee_processes`、按 task+session_kind 查询。
- Claude / Grok Manager 收敛为共享实现的薄类型别名。
- Codex 额外字段（`provider`、`execution_change_baseline`、`sdk_file_change_store`）与 OpenCode 的 `sdk_server` 状态通过**组合/扩展字段**保留，禁止用 `else if` 特判链削弱类型安全。
- 对外仍导出 `ClaudeManager` / `GrokManager` / `CodexManager` / `OpenCodeManager` 名称，避免一次性改遍 `task_automation` / `lib.rs` / `sessions` 的 State 类型（允许内部 re-export）。

### R4 — trait 边界（本任务定义的 “AiEngine”）

- 至少存在一个文档化的共享契约（`trait` 或等价的泛型约束 + 模块级 API），覆盖：进程注册、子进程生命周期、执行上下文解析。
- **不要求**本任务把 `start_*` / `stop_*` / stream 解析做成统一 `dyn AiEngine` 并改写 `task_automation` 调度；启动与协议仍按引擎命令分发（与现网一致）。

### R5 — stream 与启动路径边界

- 各引擎 `process/stream.rs` **保持各自实现**（协议不同：Claude CLI JSON / Grok streaming-json / OpenCode bridge / Codex CLI-SDK）。
- 各引擎 `process/mod.rs` 中的 CLI/SDK 参数组装、SSH 远程 shell 启动 **默认不合并**（相似度不足且与 C6 边界重叠）；仅允许抽取已证明纯重复且无行为分叉的小 helper。

### R6 — 可选共享：final status 解析

- claude / grok `session_runtime` 中同构的 `resolve_final_*_status` 允许抽到共享 helper（已有对称单测）；**不强制**整文件合并 `session_runtime.rs`。

### R7 — 测试补齐

- 共享内核具备单元测试：Manager 多会话、unbound 查询（若共享）、context 在 local/SSH/显式 working_dir 下的解析、错误文案含引擎名。
- Claude / Grok 至少具备与 OpenCode/Codex 对等的 Manager 注册表测试（当前 Claude/Grok manager 无测试）。
- 现有引擎测试全部迁移/适配后仍通过；测试总数 **不得净减少**（父任务基线 ≥246；本分支以当前全量 `cargo test` 通过数为底线，只增不减）。
- 不引入真实网络 / 真实 SSH / 真实 CLI 二进制依赖。

### R8 — 文档与规格

- 更新 `.trellis/spec/backend/ai-engines.md` 与 `directory-structure.md`：记录 `engine/`（或最终模块名）共享内核、各引擎保留差异、trait/API 契约。
- 若 `CLAUDE.md` / `Agents.md` 中「零 trait」描述因本任务失效，在收尾阶段更新（可与提交同批）。

## Acceptance Criteria

- [x] 新增共享模块（建议 `src-tauri/src/engine/`），包含 context + child + process manager 内核，且存在可文档化的 trait/契约
- [x] `claude` / `grok` 的 manager / lifecycle / context **不再各维护一份完整拷贝**（通过 re-export / 类型别名 / 薄包装）
- [x] `opencode` context 与 `codex` context 不再三份重复实现同一 SQL 解析
- [x] 四引擎 `stream.rs` 仍为独立文件，协议解析未强行统一
- [x] 对外 Tauri 命令名、事件名、`ai_provider` 取值、会话表语义不变
- [x] `cargo test --manifest-path src-tauri/Cargo.toml` 全绿，测试数量 ≥ 任务开始前基线（284 → 295+）
- [x] 共享内核 + Claude/Grok manager 缺口测试已落地
- [x] spec `ai-engines.md`（及目录结构说明）已反映新布局

## Out of Scope

- 不统一四引擎 `start_*` 为单一 IPC 或 `dyn` 调度
- 不合并四引擎 `stream.rs` / 不统一事件 payload 形状
- 不拆分 `git_workflow.rs` / `task_automation.rs`（属 C6）
- 不改数据库表结构或历史表名 `codex_sessions*`
- 不新增引擎、不改前端 UI
- 不做 lint 清零（属 C7）
- 不强行把 Codex 的 stop/validate/file-change 长逻辑与 CLI 引擎生命周期揉成一个大文件

## Constraints

- 分支：`feat/engine-trait-abstraction`（已绑定）；基线 `main`
- 兼容 SSH 与 local 执行目标；`validate_runtime_working_dir` 等既有校验路径不得绕过
- 用户可见错误文案（中文）语义保持；允许引擎名参数化
- 独立提交合回；父任务要求每子任务独立分支 + 单独提交

## Open Questions

（无阻塞项；技术形态见 `design.md`。）
