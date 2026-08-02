# 完成态/非Worktree提交与AI提交(含自动解冲突)

## Goal

让看板任务在「已完成 / 人工处理 / 非 Worktree」等收尾场景下，仍能在任务菜单完成代码提交；提供一键 **AI 提交**，并在约定 Git 冲突场景由 AI **自动解冲突后继续**，减少切到项目页手工收尾。

## Background

### 用户决策

| # | 决策 |
|---|---|
| D1 | 非 Worktree **不提供合并**，只提供「提交代码」+「AI 提交」 |
| D2 | **必须**实现 AI 自动解冲突（不可仅报错转人工作为终态能力） |
| D3 | 进入 Trellis 规划本任务 |
| D4 | 显示条件以「可提交变更」为主；完成态 / `manual_control` 不得单独隐藏提交 |
| D5 | 解冲突范围 **方案 A**：① AI 提交前已有 unmerged/冲突 → 解完再 stage+commit；② Worktree「合并到目标分支」冲突 → AI 解完再完成合并 |

### 仓库现状（证据）

- 看板「提交代码」=`canCommitTaskCode`，强依赖 `gitContext`（Worktree）：`src/components/tasks/TaskCard.tsx` ~289-302
- 完成列只隐藏运行：`KanbanColumn.tsx` `hideRunAction={status === "completed"}`
- `manual_control` 已在 automation 放行列表中；非 Worktree 仍常无按钮，因无 `gitContext`
- 非 Worktree 自动质控跳过自动提交：`task_automation/fix_loop.rs` `should_auto_commit_task_worktree`
- 任务提交绑 `task_git_context` + worktree path：`commit_task_git_changes` 等
- 项目主仓库提交已有：`commit_project_git_changes`
- AI 生成 commit message 已有：`generate_commit_message_for_project` / `ai_generate_commit_message`
- Worktree 合并已有：`git_workflow` pending_action merge / `merge_task_branch_into_target_*`
- **尚无** AI 解 Git 冲突闭环

## Requirements

### R1. 任务菜单统一 Git 收尾入口

| 按钮 | Worktree | 非 Worktree |
|---|---|---|
| 提交代码 | 任务 worktree | 项目主仓库 |
| AI 提交 | 同上 + AI message + 冲突处理 | 同上 |
| 合并到目标分支 | 有可合并上下文时 | **永不显示** |

入口位置：看板 `TaskCard` 菜单为主；任务详情若已有同类 Git 操作则保持同一套可见性规则。

### R2. 显示条件

**提交代码 / AI 提交** 同时满足时显示：

1. 项目 Git 运行时可用（local / SSH）。
2. 无本任务活跃执行/审核会话。
3. 目标工作区有可提交变更，**或**可重试的提交失败态 / 已有 unmerged 需先解冲突再提交。
4. 不因 `task.status === "completed"` 或 automation ∈ `{manual_control, commit_failed, blocked}` 单独隐藏。
5. Worktree：上下文健康；若 `merge_ready` 且无脏文件且无 unmerged → 隐藏提交，保留合并。
6. 非 Worktree：不要求 `use_worktree` / `gitContext`。

**合并到目标分支**（仅 Worktree）：沿用并修正现有 `canTriggerMergeAction`（健康上下文且非终态失败/漂移等）。

### R3. 手动「提交代码」

- 对话框：变更摘要、编辑 message、可选用 AI 生成 message 后人工确认提交。
- 非 Worktree：文案明确「将提交项目主仓库当前工作区改动（可能含其他任务文件）」。
- 成功后活动日志 + 刷新看板相关状态。

### R4. 「AI 提交」一键流

1. 解析目标目录（worktree path 或 project repo path）。
2. 若存在 unmerged/冲突 → **R5**，成功后继续。
3. Stage-all（与现有 task/project 策略一致）。
4. AI 生成 commit message（复用现有生成与校验）。
5. Commit。
6. Worktree：成功后进入 `merge_ready`（沿用现语义）。
7. 非 Worktree：仅主仓库 commit，不进入任务合并态。
8. 中文活动日志 + UI 刷新。

### R5. AI 自动解冲突（方案 A）

**触发：**

- T1：AI 提交路径发现 unmerged paths / 冲突标记文件。
- T2：Worktree「合并到目标分支」执行后发生冲突。

**不触发（本任务 Out of Scope）：** 自动 pull/rebase 远端；cherry-pick/rebase 菜单动作的 AI 解冲突（可后续扩展）。

**行为：**

1. 列出冲突文件。
2. 用可写文件能力的 AI 执行（优先任务 assignee 引擎会话；若策略改为 one-shot+后端写回须在 design 固定）在正确 cwd（兼容 SSH）解决冲突。
3. `git add` 已解决文件。
4. 继续原操作：T1 → stage+commit；T2 → 完成 merge（`merge --continue` 或等价）。
5. 有限次重试；失败则明确错误、保留人工入口，**不得静默丢改动**。
6. 操作过程写活动日志。

### R6. 活动日志与仪表盘中文

新增 action 的中文 label 必须进 `getActivityActionLabel()`（及后端允许列表若有）。

### R7. SSH

dirty 探测、stage、commit、merge、冲突读写、AI cwd 一律走现有 `git_runtime` / project runtime / 引擎远程执行路径。

## Acceptance Criteria

- [ ] AC1：非 Worktree + 主仓库有可提交改动 → 菜单有「提交代码」「AI 提交」，无「合并到目标分支」。
- [ ] AC2：Worktree + worktree 有可提交改动 → 有提交/AI 提交；`merge_ready` 且干净 → 有合并无提交。
- [ ] AC3：`status=completed` + automation `manual_control` + 仍有可提交改动 → 提交/AI 提交可见且可成功。
- [ ] AC4：手动提交可完成并写中文活动日志。
- [ ] AC5：AI 提交在无冲突时可一键生成 message 并提交成功。
- [ ] AC6：T1/T2 冲突场景下 AI 自动解冲突并继续；失败有明确错误与人工路径，不无故清空工作区。
- [ ] AC7：local 与 SSH 均可完成核心路径（SSH 至少 dirty+提交；解冲突 cwd 正确）。
- [ ] AC8：无脏文件且无 unmerged/不可重试时不误显示提交；完成列仍隐藏「运行」。

## Out of Scope

- 非 Worktree 合并 / 主仓库任意目标分支 merge
- 智能挑选「仅本任务文件」提交（MVP 全量 stage + 警告）
- 本任务必做 push
- 自动 pull/rebase 远端（方案 C）
- 重做自动质控状态机全貌
- cherry-pick/rebase 动作的 AI 解冲突

## Risks

- 非 Worktree 多任务共享主仓库，全量 stage 可能提交他人改动 → UI 强提示。
- AI 解冲突直接改业务代码 → 需日志、失败可恢复、重试上限。
- 解冲突实现若仅 one-shot 无写文件能力会假完成 → design 必须选可写路径。

## Decisions Log

| # | 决策 | 来源 |
|---|---|---|
| D1 | 非 Worktree 无合并 | 用户 |
| D2 | 必须 AI 解冲突 | 用户 |
| D3 | Trellis 规划 | 用户 |
| D4 | 完成态/人工处理不挡提交 | 规划+用户意图 |
| D5 | 解冲突范围方案 A | 用户 |
