# Implement: 巨型文件模块化拆分

## Preconditions

- [x] 用户选择交付方案 **A**
- [ ] 用户批准本最终规划（`prd` + `design` + `implement`）
- [ ] `task.py start 07-29-split-large-modules`（status → `in_progress`）
- [ ] Phase 2 前加载 `trellis-before-dev` / 或 dispatch `trellis-implement`（Grok：`spawn_subagent`，prompt 首行 `Active task: .trellis/tasks/07-29-split-large-modules`）

## Validation commands (repeated)

```bash
# 编译 + 全量 Rust 测试（主门禁）
cargo test --manifest-path src-tauri/Cargo.toml

# 可选：更快反馈
cargo check --manifest-path src-tauri/Cargo.toml

# 前端回归（本任务通常无 TS 改动）
npm run build
```

基线：记录开始时 `cargo test` 通过用例数，结束时 **≥ 该数**。

---

## Commit 1 — `git_workflow` 目录化拆分

### Step G0 — 基线

1. 运行 `cargo test --manifest-path src-tauri/Cargo.toml`，记下通过数
2. `wc -l src-tauri/src/git_workflow.rs`

### Step G1 — 整文件迁入目录（行为零 diff）

1. `mkdir -p src-tauri/src/git_workflow`
2. `git mv src-tauri/src/git_workflow.rs src-tauri/src/git_workflow/mod.rs`
3. `cargo test --manifest-path src-tauri/Cargo.toml`  
   - 预期：通过（Rust 将 `mod git_workflow` 解析为目录）

### Step G2 — 抽出 `types.rs`

1. 将 DTO / 常量 / 小 struct / `TaskGitAutoCommitOutcome` 移入 `types.rs`
2. `mod types;` + 必要 `pub use`
3. 修复其它仍在 `mod.rs` 的引用（`use crate::git_workflow::types::…` 或 `super::types::`）
4. `cargo check` 或 `cargo test`

### Step G3 — 抽出 `runtime.rs`

1. 迁入 runtime 解析、`run_git_*`、ref/branch 探测、通用 normalize/hash 工具
2. domain 文件通过 `super::runtime` 调用
3. 编译门禁

### Step G4 — 抽出 `context.rs`

1. task git context CRUD + prepare/refresh/reconcile commands
2. 引擎钩子 + `auto_commit_task_worktree` + task 侧 stage/commit overview
3. 保持 `pub(crate)` API 名称；`mod.rs` re-export
4. 编译门禁

### Step G5 — 抽出 `worktree.rs`

1. worktree 路径/解析/list 与全部 worktree commands（含 remove/merge）
2. 编译门禁

### Step G6 — 抽出 `project_ops.rs`

1. overview / commits / preview / open / 主工作树 stage-commit 系列
2. 编译门禁

### Step G7 — 抽出 `branch.rs` + `pending_action.rs`

1. branch 六命令 + normalize
2. request/confirm/cancel + `execute_normalized_action` 族
3. 编译门禁

### Step G8 — 抽出 `tests.rs`

1. 将 `#[cfg(test)] mod tests { … }` 迁入 `tests.rs`
2. 修正 `use super::…` 路径
3. **全量** `cargo test --manifest-path src-tauri/Cargo.toml`

### Step G9 — 体量验收

```bash
wc -l src-tauri/src/git_workflow/*.rs
# 每个非 tests 文件 < 1500；目标 < 1000
```

### Step G10 — Commit 1

```text
refactor(git_workflow): split monolithic module into domain files
```

仅包含 `git_workflow/` 相关与为编译所必需的 import 调整。

---

## Commit 2 — `task_automation` 续抽 + 文档

### Step A0 — 现状确认

```bash
wc -l src-tauri/src/task_automation.rs src-tauri/src/task_automation/*.rs
```

保持 **`task_automation.rs` + `task_automation/` 混合布局**（与 `prompt.rs` 一致），不强制改成纯目录。

### Step A1 — `state.rs`

1. 常量、policy、emit、upsert、reserve/finalize、status update helpers
2. 根文件 `mod state;`
3. 编译门禁

### Step A2 — `session_exit.rs`

1. resume/orphan/replay + session/execution/review exit 处理链
2. 对外 `pub` 函数在根 re-export 或保持 `task_automation::handle_session_exit_blocking` 可访问
3. 编译门禁

### Step A3 — `fix_loop.rs` + `restart.rs` + `review_data.rs`

1. 按 design 迁入
2. 迁移伴随的 `#[cfg(test)]` 模块
3. 全量 `cargo test`

### Step A4 — 体量验收

- 根 `task_automation.rs` **< 1500**
- `prompt.rs` 之外 **≥ 2** 个新文件（本计划为 5 个，可接受）

### Step A5 — 文档

更新：

1. `.trellis/spec/backend/directory-structure.md`
2. `.trellis/spec/backend/index.md`（Git/Automation 描述）
3. `CLAUDE.md` 中巨型文件/布局相关句子

### Step A6 — 全量回归

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
```

### Step A7 — Commit 2

```text
refactor(task_automation): extract domain submodules from monolith

Also update backend directory docs after module split.
```

---

## Review gates

在宣称完成前：

- [ ] `git diff` 以 move 为主；无故意业务逻辑改动
- [ ] `lib.rs` handler 列表符号仍解析
- [ ] 引擎侧 `use crate::git_workflow::…` / `task_automation::…` 无新增错误
- [ ] 行数门槛满足
- [ ] 测试数 ≥ 基线
- [ ] 文档已更新

## Rollback points

| 点 | 动作 |
|---|---|
| G1 后 | `git mv` 回单文件 |
| G2–G8 任一步 | 恢复该步涉及文件，或 reset 到上一个绿测 commit |
| Commit 1 已提交、Commit 2 失败 | 修复 A* 或 revert Commit 2 文件，保留 git_workflow 成果 |

## Out of scope during implement

- 解循环依赖
- 重命名 command
- clippy 全库治理
- 改 `git_runtime.rs` 结构
