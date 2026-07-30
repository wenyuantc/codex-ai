# Design: 巨型文件模块化拆分

## 1. Goals & Non-Goals

**Goals**

- 将 `git_workflow` / `task_automation` 从「难导航的超大单文件」变为「可定位的领域子模块」
- 保持所有对外符号路径与运行时行为不变

**Non-Goals**

- 业务逻辑重写、循环依赖消除、API 重命名、lint 清零

## 2. Architecture

### 2.1 原则

1. **机械搬迁优先**：先移动再修可见性；禁止顺手改算法/文案/SQL
2. **对外稳定**：引擎与 `lib.rs` 仍只依赖 crate 根路径 `crate::git_workflow::*` / `crate::task_automation::*`
3. **已有模式复用**：
   - `task_automation.rs` + `task_automation/*.rs`（已有 `prompt.rs` 真实 submodule）
   - 新建 `git_workflow/mod.rs` + 兄弟文件
4. **测试跟随领域或集中 tests 文件**：优先保持现有断言原文，避免重写
5. **实现选择（2026-07-30）**：领域切片用 `include!` 拼回同一模块命名空间，而不是真实 sibling `mod` + `pub use`。原因：Tauri `#[tauri::command]` 的 inventory/`__cmd__*` 不能通过 `pub use` 稳定 re-export；超大文件内 private helper 交叉调用若改真实子模块需海量 `pub(super)` 与 import 图。`include!` 在保持可导航文件边界的同时零行为/零可见性风险。`prompt.rs` 仍为真实 `mod prompt`。

### 2.2 Target layout

```text
src-tauri/src/
├── git_workflow/                    # 原 git_workflow.rs
│   ├── mod.rs                       # mod 声明 + 对外 pub use 聚合
│   ├── types.rs                     # DTO / 状态常量 / TaskGitAutoCommitOutcome
│   ├── runtime.rs                   # runtime 解析、run_git_*、ref/branch 探测、小工具
│   ├── context.rs                   # task_git_contexts CRUD + prepare/refresh/reconcile
│   │                                # + validate/mark running/finished + auto_commit + task commit cmds
│   ├── worktree.rs                  # worktree 路径/解析/list/stage/commit/remove/merge
│   ├── project_ops.rs               # overview / commits / file preview / open
│   │                                # + 主工作树 stage/unstage/rollback/commit
│   ├── branch.rs                    # push/pull/checkout/create/delete/merge branches
│   ├── pending_action.rs            # request/confirm/cancel + payload normalize + execute
│   └── tests.rs                     # 原 mod tests 整体搬迁（#[cfg(test)]）
├── git_runtime.rs                   # 不变
├── task_automation.rs               # 根：mod 声明、re-export、少量 glue
└── task_automation/
    ├── prompt.rs                    # 已有
    ├── state.rs                     # phase/常量、policy、emit、upsert、reserve/finalize pending
    ├── session_exit.rs              # resume/orphan/replay + handle_session_exit* + execution/review exit
    ├── fix_loop.rs                  # fix/review 重试、start_fix_round、auto-commit 衔接、无改动收尾
    ├── restart.rs                   # restart_task_automation* + review/fix step restart
    └── review_data.rs               # review report/raw/verdict 恢复 + subtasks/attachments 查询
```

> 文件名以实现期可微调（例如 `project_ops` → `project.rs`），但**职责边界**以上表为准。若某文件仍 >1500 行，允许在同域内再切一刀，不得把无关域塞回去。

### 2.3 模块职责与迁入内容

#### `git_workflow::types`

- 全部 `pub struct` DTO（`TaskGitContextRecord/Summary`、`ProjectGitOverview`、…）
- 状态常量：`TASK_GIT_STATE_*`、`PENDING_ACTION_TTL_MINUTES`、history limit 等
- `TaskGitAutoCommitOutcome`（`pub(crate)` 保持）
- 内部小 struct：`RawWorktreeEntry`、`TaskGitContextWorktreeRow`、`RequestGitActionInput` 等

#### `git_workflow::runtime`

- `GitProjectRuntimeContext` 及 `resolve_project_runtime*`
- `run_git_text` / `run_git_command` / `ensure_git_repository`
- branch/ref 存在性、current branch、sanitize fragment、hash/signature 小工具
- 与 SSH/local 无关的纯 normalize 辅助（path/limit/change_type 等）

#### `git_workflow::context`

- DB：insert/save/delete/fetch task git context
- prepare / refresh / reconcile / list / get / delete command
- 引擎钩子：`validate_task_git_context_launch`、`mark_*_running`、`mark_*_session_finished`
- `auto_commit_task_worktree`、`commit_task_git_changes*`、`stage_all_task_git_files`、`get_task_git_commit_overview`
- worktree ensure/branch ensure 中与 context 生命周期强耦合的部分（若与 `worktree` 交叉，**以调用方清晰为准**：ensure 放 worktree，context 调用之）

#### `git_workflow::worktree`

- path 构建、porcelain 解析、list/enrich
- 全部 `*_project_worktree_*` command
- `remove_project_git_worktree` / `merge_project_git_worktree`

#### `git_workflow::project_ops`

- `get_project_git_overview` / commits / commit detail / file preview / open file
- 主工作树 stage/unstage/rollback/commit 系列
- 共享 `file_preview_in_dir` / `collect_working_tree_changes` 等（若 worktree 也用，则放 `runtime` 或 `project_ops` 并 `pub(super)`）

#### `git_workflow::branch`

- push/pull/checkout/create/delete/merge_project_git_branches
- 相关 normalize（fast-forward/strategy/force/pull mode）

#### `git_workflow::pending_action`

- pending 字段 clear/reject、payload normalize、`execute_normalized_action`
- `request_git_action` / `confirm_git_action` / `cancel_git_action`

#### `git_workflow::mod`

```rust
mod types;
mod runtime;
mod context;
mod worktree;
mod project_ops;
mod branch;
mod pending_action;
#[cfg(test)]
mod tests;

pub use types::*;          // 或显式列表，避免 * 若 clippy 抱怨则显式
pub use context::{
    validate_task_git_context_launch,
    mark_task_git_context_running,
    mark_task_git_context_session_finished,
    auto_commit_task_worktree,
    // …全部 command 与 pub API
};
// 其余子模块 pub use 其对外 command
```

`lib.rs` **继续**写 `git_workflow::get_project_git_overview` 等，无需改路径（若 `pub use` 完整）。

#### `task_automation` 子模块

| 文件 | 迁入 |
|---|---|
| `state.rs` | 常量、`TaskAutomationPolicy`、`load_*`、`emit_*`、`upsert_state_terminal`、`reserve_pending_action`、`finalize_launched_action`、`update_task_status_internal` |
| `session_exit.rs` | `spawn_resume_*`、`resume_pending_*`、orphan/replay、`handle_session_exit*`、`handle_execution_exit`、`handle_review_exit`、`fetch_session_exit_facts` |
| `fix_loop.rs` | `retry_pending_{review,fix,commit}`、`start_automation_fix_round`、`complete_automation_without_auto_commit`、`finalize_no_reviewable_changes`、`handle_disabled_mode_exit`、`mark_launch_failure`、`resolve_automation_execution_context` |
| `restart.rs` | `restart_task_automation*`、`restart_{review,fix}_step`、`resolve_restart_target`、`stop_running_session_for_automation_restart` |
| `review_data.rs` | `review_*_for_session`、`recover_review_*`、`fetch_task_subtasks/attachments` |
| `prompt.rs` | 不变 |
| 根 `task_automation.rs` | `mod …;` + 必要 `pub use`；内嵌 `#[cfg(test)]` 可暂留或迁 `tests` 子模块 |

内嵌测试（`automation_working_dir_tests` 等）优先随被测函数迁入对应文件底部的 `#[cfg(test)]`，或集中到根文件 tests，**禁止丢测试**。

### 2.4 可见性与循环依赖

```text
                    ┌──────────────────┐
   engines ────────►│  git_workflow    │◄──── lib.rs (commands)
                    │  (multi-file)    │
                    └────────┬─────────┘
                             │ auto_commit / mark_finished
                             ▼
                    ┌──────────────────┐
   engines ────────►│ task_automation  │◄──── lib.rs (restart/resume)
                    │  (+prompt/…)     │
                    └────────┬─────────┘
                             │ mark_task_automation_commit_completed
                             └──────────► back to git_workflow (commit path)
```

- 循环保留在 **crate 根模块** 之间，与现状一致。
- 子模块之间单向优先：`types` ← 全员；`runtime` ← domain；`pending_action` 可依赖 `context`/`runtime`；避免 `types` 依赖 domain。
- 若 Rust 抱怨子模块私有项：优先 `pub(super)`，引擎需要的保持 `pub(crate)` 并在 `mod.rs` re-export。

### 2.5 兼容性

| 层 | 策略 |
|---|---|
| Tauri IPC | command 函数名/签名不变 |
| 前端 | 零改动 |
| 引擎 | 继续 `crate::git_workflow::{validate_…, mark_…}` |
| DB | 无 migration |
| SSH | runtime 路径整块搬迁 |

### 2.6 风险与缓解

| 风险 | 缓解 |
|---|---|
| 可见性/use 路径导致大编译错误风暴 | 先整文件改目录（`git_workflow.rs`→`mod.rs`），再按文件抽出；每抽 1–2 文件 `cargo test` |
| 误改行为 | 禁止编辑函数体逻辑；review diff 以 move 为主 |
| 测试找不到 `super::` | 调整 tests 的 `use` 到新模块路径；保持断言 |
| 循环依赖编译失败 | 不把回调改成更深子模块互引；回调仍走 `crate::task_automation::` / `crate::git_workflow::` |
| 单文件仍过大 | 允许同域二次拆分；硬上限 1500 行 |

### 2.7 Rollback

- git 分支 `feat/split-large-modules`；任一步 `cargo test` 失败则该步内修复或 `git checkout --` 回退该文件
- 两个 commit 分段：① 仅 git_workflow；② task_automation + docs。可只 revert 后者

## 3. Trade-offs

| 选项 | 结论 |
|---|---|
| 严格 4 文件 vs 内聚 7+ 文件 | **内聚**（决策 A） |
| 解循环依赖 | **不做**（扩 scope） |
| `git_workflow` 保持「.rs + 子目录」混合 | **否**，直接目录模块（更干净）；`task_automation` 保持现有混合以少动入口 |
| 一次 PR 两文件 | **是**，两 commit 可审 |

## 4. Docs touchpoints

- `.trellis/spec/backend/directory-structure.md` — 布局树与 ownership 表
- `.trellis/spec/backend/index.md` — Git / Automation 一行描述
- `CLAUDE.md` — 巨型文件行数/结构描述
