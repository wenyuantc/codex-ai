# Implement — 看板交付 UX 与可发现性

## 前置

- [ ] 用户批准本规划摘要后执行：`python3 ./.trellis/scripts/task.py start 08-05-kanban-delivery-ux`
- [ ] Phase 2 前加载 `trellis-before-dev`（frontend + 相关 guides）
- [ ] 分支：当前 `feat/kanban-delivery-ux`（或与任务 `set-branch` 对齐）

## 有序清单

### Step 1 — 共用过滤纯函数

- [ ] 新增 `src/lib/kanbanFilters.ts`：`filterKanbanTasks(tasks, filters, taskTagMap)`  
  覆盖：非 archived、keyword、overdue、blocked、milestone、tag、（可选）priority/assignee。
- [ ] 类型：`KanbanFilterState`、`TaskTagMap = Map<string, string[]>`（或 `Set`）。
- [ ] 逾期语义与 `isTaskOverdue` 对齐（Board 已用；Page 当前仅 `Boolean(due_date)` — **统一为 isTaskOverdue**）。

### Step 2 — 看板筛选生效 + 中文

- [ ] `KanbanPage`：加载 `taskTagMap`（当前可见非归档任务）；`filteredTaskIds` 改用纯函数。
- [ ] `KanbanBoard`：去掉 `_tagId` 死参；`activeTasks` 用同一纯函数。
- [ ] 批量状态 `SelectValue` 中文（`TASK_STATUSES`）。
- [ ] 可选：优先级、执行人筛选 Select（中文）；执行人列表来自 `employeeStore` 且与当前项目相关。
- [ ] 校验：全选筛选结果 id 集合 ⊆ Board 可见任务。

### Step 3 — 任务卡里程碑 + 标签展示收敛

- [ ] `TaskCard`：有 `milestone_id` 时展示里程碑名徽章（板级 `milestonesById` prop 或父级 map）。
- [ ] 优先从板级传入 tags，避免每卡重复 `listTaskTags`（依赖 count 可保留轻量请求或一并板级缓存）。
- [ ] 保持标签色点与依赖·N 徽章。

### Step 4 — 创建任务里程碑

- [ ] `CreateTaskDialog`：项目变更时 `listMilestones`；Select「无里程碑」+ 列表。
- [ ] 创建时写入 `milestone_id`（create payload 或 create 后 `updateTask`）。

### Step 5 — 归档打开详情与取消归档

- [ ] `ArchiveManagementDialog`：行可点 / 操作列「打开」→ `TaskDetailDialog`。
- [ ] 详情关闭或保存后刷新归档 `listTasks`；若状态离开 archived，调用 `fetchTasks`。
- [ ] `TaskOverviewPanel`：archived 时状态 Select 可改为 `ACTIVE_TASK_STATUSES` 之一（取消归档）。
- [ ] 确认标题/描述/优先级/标签在 archived 详情可保存（不误 disabled）。

### Step 6 — Activity / 中文收尾

- [ ] 确认 `tasks_batch_updated` 中文存在（已有则跳过）。
- [ ] 若实现中新增 action key，补 `getActivityActionLabel`。
- [ ] 时间字段仅 `formatDate()`。

### Step 7 — 验证

- [ ] `npm run build`
- [ ] 若改 Rust：`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` + 相关 `cargo test`
- [ ] 手工冒烟（`npm run tauri:dev` 或等价）：
  1. 标签筛选生效 + 全选一致
  2. 批量改状态中文 Trigger + 活动中文
  3. 卡片见标签/里程碑
  4. 归档打开编辑 + 取消归档回看板
  5. 创建任务带里程碑
  6. environmentMode 切换后列表不混项目

### Step 8 — 收尾（Phase 3）

- [ ] `trellis-check` / 质量门
- [ ] Conventional Commit（如 `feat(kanban): 交付筛选与归档可编辑 UX`）
- [ ] 勾选 `prd.md` AC；`task.py archive` 子任务（父任务保持）

## 关键文件（预期）

| 路径 | 动作 |
|------|------|
| `src/lib/kanbanFilters.ts` | 新增 |
| `src/pages/KanbanPage.tsx` | 改 |
| `src/components/tasks/KanbanBoard.tsx` | 改 |
| `src/components/tasks/TaskCard.tsx` | 改 |
| `src/components/tasks/CreateTaskDialog.tsx` | 改 |
| `src/components/tasks/ArchiveManagementDialog.tsx` | 改 |
| `src/components/tasks/detail/TaskOverviewPanel.tsx` | 改 |
| `src/lib/utils.ts` | 按需 |
| `src-tauri/**` | 默认不动 |

## 回滚点

1. Step 1–2 可独立 revert（筛选）。
2. Step 5 归档详情可独立 revert。
3. 无 DB migration → 无数据迁移回滚。

## 不做（执行时勿扩张）

- 甘特 / 依赖图
- 批量改执行人/优先级 UI
- 新 delivery 管理整页
- 为本任务强上完整 Vitest 基建（纯函数可本地轻测，测试网归另一子任务）
