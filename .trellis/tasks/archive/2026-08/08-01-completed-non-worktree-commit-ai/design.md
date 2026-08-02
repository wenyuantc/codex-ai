# Design: 完成态/非Worktree提交与AI提交(含自动解冲突)

## Architecture Overview

```text
TaskCard / TaskDetail menu
  ├─ 提交代码  → open commit dialog (mode: worktree | project_repo)
  ├─ AI 提交   → invoke task_ai_commit (Rust orchestrator)
  └─ 合并      → existing merge dialog (worktree only)
                    └─ on conflict → task_ai_resolve_git_conflicts → continue merge

UI (React)  ──Tauri──►  git_workflow + codex one-shot/session
                           │
                           ├─ dirty / unmerged detect (git_runtime)
                           ├─ stage / commit (existing)
                           ├─ merge (existing pending_action)
                           └─ AI resolve conflicts (new orchestration)
```

原则：

- **所有写库 / 写仓库** 只在 Rust command。
- **双后端路径** 收敛到同一套前端入口与可见性函数。
- **解冲突** 与 **提交** 解耦为可组合步骤，供 AI 提交与合并复用。

## Boundaries

| Layer | Owns | Must not |
|---|---|---|
| Frontend `TaskCard` | 菜单可见性、打开对话框、触发 AI 提交、展示 loading/error | 直接 shell git / 写 SQLite |
| `src/lib/backend.ts` | invoke 包装 | 业务规则 |
| `git_workflow` | dirty/unmerged、stage、commit、merge continue、冲突文件列表 | 选模型策略细节可调用 codex helpers |
| `codex/process` (or engine) | AI message 生成、冲突解决会话/one-shot | 不知道看板 UI |
| `task_automation` | 提交成功后可选恢复 automation 完成态（复用现有 mark） | 强制非 Worktree 自动提交（保持现 skip 策略，除非本任务明确扩展） |

本任务 **不** 把非 Worktree 自动质控改为默认 auto-commit；只保证**手动/AI 入口**可用。

## Target Resolution

新增统一概念（Rust 内部，可不必新表）：

```text
TaskCommitTarget {
  mode: Worktree | ProjectRepo,
  task_id,
  project_id,
  working_dir,          // worktree_path or project.repo_path / remote path
  task_git_context_id?, // only Worktree
  execution_target,     // local | ssh
  ssh_config_id?,
}
```

解析规则：

1. `task.use_worktree == true` 且存在健康 `task_git_context` → `Worktree`
2. 否则 → `ProjectRepo`（要求项目 runtime 可解析）
3. Worktree 标记 `worktree_missing` / unhealthy → 提交类按钮不可用（合并/修复另论）

## Visibility (Frontend)

抽出纯函数（建议 `src/lib/taskGitActions.ts` 或扩展现有 helpers）：

```ts
canShowTaskCommitActions({ task, gitContext, automation, hasActiveSession, dirtyHint })
canShowTaskMergeAction({ task, gitContext, hasActiveSession })
```

规则对齐 PRD R2；`dirtyHint` 来源：

- Worktree：看板已有 `taskGitContextMap` 时，可增加轻量字段或按需 `get_task_git_commit_overview` / 新 `get_task_git_dirty_state`
- 非 Worktree：新 command `get_task_commit_target_overview(task_id)` 返回 `{ mode, has_stageable, has_staged, has_unmerged, branch, warning? }`

**性能**：看板 N 任务避免同步 N 次全量 overview。策略：

1. MVP：仅对 `completed` / `manual_control` / `commit_failed` / 已有 gitContext 的任务懒探测；或
2. 后端 `list_task_commit_action_states(project_id | task_ids)` 批量 `git status --porcelain`（优先，SSH 注意并发上限）

推荐实现顺序：先单任务 command + 菜单打开时/卡片挂载时对「可能需要提交」的任务请求；若卡顿再批量。

## Manual Commit Dialog

复用 `GitCommitDialogContent`：

| Mode | Overview API | Stage | Commit |
|---|---|---|---|
| Worktree | 现有 `get_task_git_commit_overview` | `stage_all_task_git_files` | `commit_task_git_changes` |
| ProjectRepo | 扩展/复用 `get_project_git_overview` 子集 | `stage_all_project_git_files` | `commit_project_git_changes`（日志补 task_id） |

非 Worktree 对话框 description 固定风险提示文案。

`commit_project_git_changes` 今日 activity 未绑 `task_id`：实现时应 **可选传入 task_id** 写日志，便于任务维度审计（向后兼容旧签名：新增 command 或给现有 command 加 optional task_id）。

## AI Commit Orchestrator

新 command（建议名）：

```text
ai_commit_task_changes(task_id) -> AiCommitTaskResult
```

伪流程：

```text
target = resolve_task_commit_target(task_id)
if has_unmerged(target):
  resolve = ai_resolve_git_conflicts(task_id, target, reason="pre_commit")
  if resolve.failed: return error
stage_all(target)
if no staged: return error/no changes
message = generate_commit_message_for_project(...)  // existing
commit(target, message)
if target.Worktree: mark merge_ready + optional automation recover
log task_ai_commit_completed
return summary
```

前端：菜单点「AI 提交」→ loading → toast/错误；成功刷新 git map + tasks。

## AI Conflict Resolution (D5 / 方案 A)

新 command（可内嵌也可独立）：

```text
ai_resolve_task_git_conflicts(task_id, phase: pre_commit | post_merge) -> ResolveResult
```

### 冲突检测

在 `git_runtime` 增加（local + SSH）：

- `list_unmerged_paths(working_dir)` → `git ls-files -u` 或 porcelain 中 unmerged 状态
- `read_working_tree_file` 已有则复用；需支持冲突文件内容读取

### 解决策略（选定）

**采用「可写工作区的引擎会话 / agent」为主路径**，不用纯文本 one-shot 假装改文件：

1. 冲突文件列表 + 简短 status 写入 prompt。
2. `working_dir` = target.working_dir（SSH 走引擎远程 cwd）。
3. 优先 `task.assignee_id` 对应引擎启动**短生命周期**执行（参考现有 task run / one-shot 带 tools 的能力）；若 assignee 为空：回退 settings `one_shot_preferred_provider`，但仍须 **cwd 内可编辑文件**（Codex/Claude/Grok 以各自 exec/session 写文件为准）。
4. Prompt 约束：只解决冲突标记、不重构无关代码、解决后不要自行 commit（由 Rust 继续）。
5. 轮询/等待会话结束（超时可配置，建议 10–20min 上限与现有 session 一致量级，可更短如 5–10min）。
6. 再次 `list_unmerged_paths`：
   - 空 → `git add -A`（或 add 列表）→ 成功
   - 非空 → 最多 **1 次** 追加 prompt 重试；仍失败 → 返回错误，保留冲突文件

### Merge 路径集成（T2）

在 Worktree 合并执行点（`pending_action` merge 失败或检测到冲突）：

1. 识别冲突错误 / unmerged。
2. 调用 `ai_resolve_task_git_conflicts(..., post_merge)`。
3. 成功后 `git merge --continue`（若 git 停在 MERGING）或等价完成步骤；清理 state → 现有 completed/merge success 路径。
4. 失败：context 保持可人工合并，UI 显示 last_error。

注意：`merge_task_branch_into_target_local` 今日在主工作区 checkout 目标分支再 merge；冲突发生在 **主仓库 cwd**，不是 task worktree。解冲突 `working_dir` 必须用 **实际产生冲突的目录**（主 repo_path），并在 design 实现时用 runtime 探测 `MERGE_HEAD` 所在仓库。

### Commit 路径集成（T1）

AI 提交前若 unmerged（例如上次合并中断残留）：

1. 先 resolve
2. 再 stage + commit  
若仍处于 MERGING 且用户只想提交：优先 `merge --continue` 完成合并再视 dirty 决定是否额外 commit；实现时以 `git status` 状态机分支处理，避免错误 `git commit` 嵌进半成品 merge。

## Data / Schema

- **MVP 不强制新表**。
- 若需可观测性：activity_logs 足够。
- 不改 `tasks` 列。

## Activity Log Keys（建议）

| action | 中文 |
|---|---|
| `task_commit_opened` / 可省略 | — |
| `task_git_committed` | 已有任务 worktree 提交则复用 |
| `task_project_git_committed` | 任务维度主仓库提交 |
| `task_ai_commit_started` | 任务 AI 提交开始 |
| `task_ai_commit_completed` | 任务 AI 提交完成 |
| `task_ai_commit_failed` | 任务 AI 提交失败 |
| `task_ai_conflict_resolve_started` | AI 解冲突开始 |
| `task_ai_conflict_resolve_completed` | AI 解冲突完成 |
| `task_ai_conflict_resolve_failed` | AI 解冲突失败 |

后端 activity allowlist（若 `database.rs` 有枚举/校验）与 `src/lib/utils.ts` 同步。

## Compatibility

| 场景 | 行为 |
|---|---|
| Local worktree | 现路径 + AI |
| Local non-worktree | 主仓库提交 + AI |
| SSH worktree | 现 remote git + AI cwd remote |
| SSH non-worktree | remote 主仓库 status/stage/commit |
| 无 assignee | AI 提交/解冲突回退 one_shot 设置；失败文案要求指派或配置 one-shot |
| automation enabled + manual_control | 允许手动/AI 提交；成功可 mark automation completed（与现 commit 成功恢复一致） |

## Trade-offs

| 选择 | 取舍 |
|---|---|
| 非 Worktree 全量 stage | 简单、与现项目页一致；有误提交风险 → UI 警告 |
| 解冲突用 agent 写文件 | 成功率高；耗时长、要会话编排 |
| 看板脏探测 | 精确按钮 vs 性能；先条件探测再批量优化 |
| 不改 auto-commit 非 Worktree 策略 | 范围可控；自动闭环仍跳过非 Worktree |

## Rollback

- 功能开关：若需紧急回退，可用 settings flag `task_ai_commit_enabled`（可选；MVP 可用代码回退）。
- Git：解冲突失败不 `merge --abort` 除非用户明确操作；默认保留冲突便于人工。
- DB：无迁移则无需 DB 回滚。

## Test Plan (design-level)

Rust：

- `resolve_task_commit_target` worktree vs project
- unmerged detect parsing
- visibility pure functions（若放 TS 则前端 build；逻辑尽量 Rust 测 target）
- commit_project with task_id log
- merge conflict detect → resolve hook dry-run（mock AI）

手工：

- 完成列 + manual_control + worktree dirty
- 非 Worktree dirty 提交 / AI 提交
- 制造 merge 冲突后 AI 合并路径
- SSH 冒烟
