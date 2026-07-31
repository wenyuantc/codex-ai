# Implement: 看板新建任务支持创建并执行

## Checklist

### 1. 抽取执行启动内核（可回归验证）

- [ ] 从 `useTaskExecutionActions` 抽出 `startTaskRunSession`（或等价命名）纯异步函数，参数覆盖：task、assignee、projectRepoPath、prepared input / planContent、store side-effects 所需依赖。
- [ ] Hook 改为调用该内核；卡片/详情「运行」行为不变。
- [ ] 错误仍写入任务执行日志并回调 `onError`。

### 2. 协调员计划 helper

- [ ] 封装 `generateAndPersistCoordinatorPlan`：调用 `aiGenerateCoordinatorTaskPlan` → 校验非空 → `updateTask({ plan_content })` → 返回 plan 字符串。
- [ ] 不打开 `CoordinatorPlanDialog`。

### 3. CreateTaskDialog UI + 编排

- [ ] 底部按钮：取消 | 创建 | **创建并执行**（主按钮样式）。
- [ ] 共享 create 主体（标签/依赖/附件）避免两套 create 逻辑分叉。
- [ ] `handleCreate`：仅创建后关弹窗。
- [ ] `handleCreateAndRun`：
  1. R4 校验（含 assignee 必填）
  2. phase=creating → create + tags/deps
  3. 有 coordinator → phase=planning → generateAndPersist
  4. phase=starting → startTaskRunSession（带 plan 若有）
  5. 成功 → `onOpenLog?.(id, "execution")` + 关弹窗
  6. 失败 → `createError`，任务已创建则不删除
- [ ] 阶段文案与按钮 disabled 状态。

### 4. 接线 onOpenLog

- [ ] `KanbanPage` / 若创建入口在 Board 内：把 `handleOpenLog` 传入 `CreateTaskDialog`。
- [ ] 确认其他打开 `CreateTaskDialog` 的入口兼容可选 prop。

### 5. 活动日志中文

- [ ] 核对 create / plan save / 执行启动相关 action 在 `src/lib/utils.ts` 中文映射；缺失则补。
- [ ] 不引入无中文的新 key。

### 6. 回归与构建

- [ ] 手工：仅创建；无协调员创建并执行；有协调员创建并执行；无 assignee 拦截；plan 失败保留任务；卡片有协调员仍弹确认。
- [ ] SSH / worktree 各至少一条冒烟（若环境可用）。
- [ ] `npm run build`
- [ ] 若动到 Rust：`cargo test --manifest-path src-tauri/Cargo.toml` 相关模块；否则可跳过。

## Validation Commands

```bash
npm run build
# 可选
npm run lint
npm run tauri:dev   # 手工冒烟
```

## Risky Files / Rollback Points

| 文件 | 风险 |
|---|---|
| `src/components/tasks/hooks/useTaskExecutionActions.ts` | 抽内核回归卡片运行/停止 |
| `src/components/tasks/CreateTaskDialog.tsx` | 创建路径分叉、双提交竞态 |
| `src/components/tasks/TaskCard.tsx` / `TaskDetailDialog.tsx` | 若改 hook 签名需同步 |
| `src/pages/KanbanPage.tsx` / `KanbanBoard.tsx` | onOpenLog 接线 |

回滚：还原上述文件；无 DB migration。

## Before `task.py start`

- [x] `prd.md` 收敛，Open Questions 已清空
- [x] `design.md` / `implement.md` 已写
- [ ] 用户批准最终规划摘要
- [ ] 再 `task.py start`（本文件批准前不执行）

## Notes for implementer

- Agents.md：活动日志中文；兼容 SSH；不直接写 SQLite。
- 不新增 toast 依赖。
- 创建并执行成功前保持弹窗打开以便展示错误。
