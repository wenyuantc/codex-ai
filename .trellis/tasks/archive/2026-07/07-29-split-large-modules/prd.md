# 巨型文件模块化拆分

## Goal

将后端两个巨型模块做**纯结构拆分**（行为不变），降低维护成本与变更冲突面：

1. `git_workflow.rs`（5515 行 / 43 个 Tauri command / 16 个测试）按领域拆到目录模块
2. `task_automation.rs`（2976 行 / 6 个对外 pub API / 15 个测试）延续已有 `task_automation/prompt.rs` 的抽取方式

用户价值：后续改 Git 工作流或自动质控时，不必在 3000–5500 行单文件中定位与冲突；为父任务 C7 lint 清零提供更清晰的模块边界。

## Background

### 来源与约束（父任务）

- 父任务：`07-29-codex-ai-optimization` 源发现 #6
- 优先级：P3；建议序在 C5 之后、C7 之前（用户可独立启动本分支）
- 交付：独立分支 `feat/split-large-modules`，合回 `main`
- 明确排除：不改用户可见行为、不改表语义、不做 UI 改版

### 代码基线（2026-07-30）

| 文件 | 行数 | 公开面 | 测试 |
|---|---|---|---|
| `src-tauri/src/git_workflow.rs` | 5515 | 43× `#[tauri::command]` + 4× 引擎/automation `pub(crate)` helper | 16（末尾 `mod tests` ≈ L4345–5515） |
| `src-tauri/src/task_automation.rs` | 2976 | `spawn_resume_pending_automation` / `handle_session_exit*` / `resume_pending_automation` / `restart_task_automation*` / `mark_task_automation_commit_completed` | 15（含内嵌 test modules） |
| `src-tauri/src/task_automation/prompt.rs` | 188 | 已抽取 prompt 构建 | — |
| `src-tauri/src/git_runtime.rs` | 1159 | 底层 git 执行 | **本任务不拆** |

### 外部调用面（必须保持路径兼容）

**git_workflow（引擎 + automation）**

- `validate_task_git_context_launch` / `mark_task_git_context_running` / `mark_task_git_context_session_finished` — 四引擎 `process/mod.rs` / `session_runtime.rs`
- `auto_commit_task_worktree` / `TaskGitAutoCommitOutcome` — `task_automation`
- 43 个 command — `lib.rs` `generate_handler!`

**task_automation**

- `handle_session_exit_blocking` — 四引擎 session 退出
- `spawn_resume_pending_automation` / `restart_task_automation` — `lib.rs`
- `mark_task_automation_commit_completed` — `git_workflow` commit 路径

**双向依赖（保留，本任务不解环）**

- `task_automation` → `git_workflow::{auto_commit_task_worktree, mark_task_git_context_session_finished, TaskGitAutoCommitOutcome}`
- `git_workflow` → `task_automation::mark_task_automation_commit_completed`

## Requirements

### 必须

1. **行为零变更**：Tauri command 名、参数、返回 JSON、错误文案、DB 读写、事件名均不变；前端与引擎调用方无需改业务逻辑。
2. **稳定 crate 路径**：继续使用 `crate::git_workflow::…` / `crate::task_automation::…`；允许 `git_workflow.rs` → `git_workflow/mod.rs` 目录化。
3. **拆分 `git_workflow`**：改为目录模块，按实测内聚域分子文件（见 design）；**禁止**硬套父任务原文四文件名若与内聚冲突。
4. **拆分 `task_automation`**：在 `prompt.rs` 之外至少新增 2 个领域子模块；主文件变为编排/re-export + 必要 glue。
5. **测试保全**：`cargo test --manifest-path src-tauri/Cargo.toml` 全绿；测试数不净减（父任务基线 ≥246；本模块 16+15 用例必须保留）。
6. **SSH/本地双路径**：经 `git_runtime` / execution target 的分支只搬迁不改语义。
7. **文档同步**：更新 `CLAUDE.md` 与 `.trellis/spec/backend/directory-structure.md`（及 backend index 中单文件描述）。
8. **交付切分（决策 A）**：同一分支内 **2 个逻辑 commit**——① `git_workflow` 目录化拆分；② `task_automation` 续抽子模块 + 文档。

### 明确不做

- 不改 Git 业务算法、automation 状态机语义、prompt 策略
- 不消除 `git_workflow` ↔ `task_automation` 循环依赖
- 不做 C5 引擎 trait、C7 lint 清零
- 不拆 `git_runtime.rs`
- 不把 command 迁入 `app/`
- 不引入新 Git 库

## Acceptance Criteria

- [x] 存在 `src-tauri/src/git_workflow/` 目录模块；仓库中不再有 5515 行级的 `git_workflow.rs` 单文件
- [x] `git_workflow` 下任一实现文件（不含聚合 tests）**< 1500 行**，目标 **< 1000 行**
- [x] `task_automation` 在 `prompt.rs` 外至少 **2** 个新子模块；主文件 **< 1500 行**
- [x] 原有 43 个 git command + automation 对外 API 仍通过原 crate 路径可链接；`lib.rs` 注册名不变
- [x] `cargo test --manifest-path src-tauri/Cargo.toml` 通过，测试数 ≥ 基线（284）
- [x] `npm run build` 通过
- [x] `directory-structure.md` / `CLAUDE.md`（及相关 backend index 措辞）已反映新布局
- [x] 无行为变更意图：仅结构移动（实现采用 `include!` 保命名空间，见 design）

## Out of Scope

- 功能增强、API 重命名、错误类型统一、clippy 全量修复
- 前端 Git UI 改版
- 解循环依赖 / 抽 shared git-automation bridge 模块

## Key Decisions

| 决策 | 选择 | 日期 |
|---|---|---|
| 交付切分与粒度 | **A**：单任务做完两文件；git_workflow 按实测 6–8 子文件；automation 续 prompt 模式；同分支 2 commit | 2026-07-30 |
| 循环依赖 | 保留，不解环 | 2026-07-30 |
| 父任务「worktree/branch/merge/review」四名 | 作参考标签，**不**作为强制文件名 | 2026-07-30 |

## Open Questions

（无阻塞项）

## Notes

- 状态：`planning`。实现须在用户批准本最终规划后执行 `task.py start`。
- 曾误触 degraded `task.py start`，已恢复 `planning`。
