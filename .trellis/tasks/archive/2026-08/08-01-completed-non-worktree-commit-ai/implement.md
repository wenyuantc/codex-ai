# Implement: 完成态/非Worktree提交与AI提交(含自动解冲突)

## Checklist (ordered)

### Phase A — 目标解析与脏/冲突探测（Backend）

- [ ] A1. 在 `git_workflow` 增加 `resolve_task_commit_target(task_id)`（Worktree | ProjectRepo）
- [ ] A2. `git_runtime`：`list_unmerged_paths` / dirty 摘要 helper（local + SSH）
- [ ] A3. 新 command：`get_task_commit_action_state(task_id)` → `{ mode, has_stageable, has_staged, has_unmerged, can_commit, can_merge, warnings[] }`
- [ ] A4. 注册 `lib.rs` invoke；`backend.ts` + `types.ts` 包装
- [ ] A5. 单元测试：target 解析、unmerged 解析 fixture

### Phase B — 手动提交双路径（Backend + UI）

- [ ] B1. 扩展项目提交日志支持可选 `task_id`（或新 `commit_task_project_git_changes`）
- [ ] B2. 扩展 `TaskGitCommitDialog`（或新建 `TaskCommitDialog`）支持 `mode: worktree | project_repo`
- [ ] B3. 非 Worktree 风险提示文案
- [ ] B4. `TaskCard`：用 action state 驱动「提交代码」显示；去掉对「仅 gitContext + automation 白名单」的硬依赖（对齐 PRD R2）
- [ ] B5. 完成态 / manual_control 场景验证

### Phase C — AI 提交编排（Backend + UI）

- [ ] C1. `ai_commit_task_changes(task_id)` orchestrator：unmerged → resolve → stage → message → commit
- [ ] C2. 活动日志 keys + 中文 label（utils + 后端 allowlist）
- [ ] C3. `TaskCard` 菜单「AI 提交」按钮 + loading/错误
- [ ] C4. Worktree 成功后 `merge_ready` / automation recover 与现 commit 一致
- [ ] C5. 非 Worktree 成功仅主仓库 commit

### Phase D — AI 解冲突（Backend + Merge 集成）

- [ ] D1. `ai_resolve_task_git_conflicts(task_id, phase)`：列冲突、启 AI 可写会话、复查 unmerged、git add
- [ ] D2. Prompt 模板（只解冲突、不擅自 commit）
- [ ] D3. 超时 / 1 次重试 / 失败不 abort（保留人工）
- [ ] D4. 接入 AI 提交 T1 路径
- [ ] D5. 接入 Worktree 合并 T2 路径（冲突发生在主仓库时 cwd=repo_path）
- [ ] D6. 失败 `last_error` + 活动日志

### Phase E — 看板集成与一致性

- [ ] E1. `KanbanBoard` 在需要时刷新 commit action state（与 gitContext 刷新协同）
- [ ] E2. 非 Worktree **永不**显示合并
- [ ] E3. 任务详情若有 Git 菜单，复用同一 helper
- [ ] E4. SSH 冒烟：dirty 探测 + 提交；有条件时解冲突 cwd

### Phase F — 质量门

- [ ] F1. `npm run build`
- [ ] F2. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] F3. `cargo test --manifest-path src-tauri/Cargo.toml`（相关模块）
- [ ] F4. 手工：AC1–AC6 核心路径

## Validation Commands

```bash
npm run build
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml git_workflow
# 或更全：
cargo test --manifest-path src-tauri/Cargo.toml
```

## Risky Files / Rollback Points

| 区域 | 风险 | 回滚点 |
|---|---|---|
| `TaskCard.tsx` 菜单条件 | 误显示/误隐藏按钮 | 恢复 `canCommitTaskCode` 旧逻辑 |
| `pending_action` merge | 合并状态机破坏 | 解冲突调用前后 feature 隔离 |
| `git_runtime` SSH | 远程 status 解析错误 | 仅 local 先合，SSH 跟进 |
| AI 会话写文件 | 误改无关文件 | prompt 收紧 + 仅 unmerged 列表内文件优先 |
| 项目 commit 绑 task_id | 日志兼容 | optional 字段 |

## Suggested Commit Slices

1. `feat(git): task commit target + dirty/unmerged state`
2. `feat(tasks): manual commit for non-worktree tasks`
3. `feat(tasks): AI commit orchestrator`
4. `feat(git): AI conflict resolve for commit and merge`

## Pre-start Notes

- 实现前加载 `trellis-before-dev` 与 backend/frontend 相关 spec。
- 子 agent 派发时 prompt 首行：`Active task: .trellis/tasks/08-01-completed-non-worktree-commit-ai`
- 用户批准本规划摘要后才 `task.py start`。
