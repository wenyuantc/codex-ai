# Design — 看板交付 UX 与可发现性

## 1. 架构与边界

本任务以 **前端可发现性 / 中文 UX** 为主，**复用现有 Tauri delivery + tasks commands**，默认 **不新增表、不新增 migration**。

```text
KanbanPage / ArchiveManagementDialog / CreateTaskDialog / TaskCard / TaskDetailDialog
  → src/lib/backend.ts wrappers
  → batch_update_tasks | update_task | update_task_status
  → list_milestones | list_tags | list_task_tags | set_task_tags
  → list_task_dependencies | add/remove dependency
  → SQLite (existing schema)
```

| 层 | 是否改 | 说明 |
|----|--------|------|
| React pages/components | **是** | 筛选生效、中文 Select、卡片徽章、归档打开详情、创建里程碑 |
| `src/lib/backend.ts` / `types.ts` / `utils.ts` | **可能** | 仅当缺 wrapper 或 activity 中文；优先复用 |
| `src-tauri` delivery/tasks | **否（默认）** | 仅当标签批量查询性能必须后端化时再开 command |
| migrations | **否** | 表已有 |

## 2. 数据流

### 2.1 看板筛选

```text
KanbanPage state:
  keyword, overdueOnly, blockedOnly, milestoneFilter, tagFilter
  (+ optional priorityFilter, assigneeFilter)
  milestones[], tags[]  // listMilestones / listTags(currentProjectId)

加载 taskTagMap:
  对当前 store 中非 archived 且在可见项目内的 task ids
  → 并发 listTaskTags(id)（限流或 Promise.all 分批）
  → Map<taskId, Set<tagId>>

过滤（KanbanPage.filteredTaskIds 与 KanbanBoard.activeTasks 必须同一规则）:
  status !== archived
  ∧ overdue / blocked / milestone / keyword
  ∧ tagFilter==all ∨ taskTagMap[task].has(tagFilter)
  ∧ optional priority / assignee
```

**单一真相**：抽纯函数 `filterKanbanTasks(tasks, filters, taskTagMap)` 放在 `src/lib/`（如 `kanbanFilters.ts`），Page 全选与 Board 列共用，避免两处逻辑漂移。

### 2.2 中文枚举展示

- 批量改状态、优先级筛选等：`SelectValue` 使用渲染函数 + `TASK_STATUSES` / `PRIORITIES` / `getStatusLabel`。
- 里程碑/标签/执行人：展示 `name`，不是 id。
- 禁止在 Trigger 默认只渲染 raw `value`。

### 2.3 任务卡交付徽章

- 标签 / 依赖：已有；保留。
- 里程碑：`task.milestone_id` + 父级传入的 `milestonesById`（或板级 map），有则徽章 `里程碑·{name}`。
- **减少 N+1**：看板层加载 `taskTagMap` 后可下发 `tags: Tag[]` 给卡片，替代每卡 `listTaskTags`（渐进：先保功能，再收敛请求）。

### 2.4 创建任务里程碑

- `CreateTaskDialog` 在选定 `projectId` 后 `listMilestones`，Select 可选「无」。
- `createTask` payload 带 `milestone_id`（`taskStore.createTask` / backend 已支持字段则直接传；否则 `updateTask` 创建后补写）。

### 2.5 归档管理

```text
ArchiveManagementDialog
  listTasks({ status: "archived", projectIds: visible })
  → 行点击 / 「打开」→ selectedTask
  → TaskDetailDialog open

on TaskDetailDialog save / status change:
  → 若仍 archived：刷新归档列表
  → 若 unarchive：刷新归档列表 + useTaskStore.fetchTasks(currentProject)
```

**取消归档 UX**：

- `TaskOverviewPanel` 状态 Select：当 `status === "archived"` 时 **不再 disabled**；选项为 `ACTIVE_TASK_STATUSES`（取消归档到待办/进行中等），Trigger 显示「已归档」。
- 或提供显式「取消归档」按钮默认回 `todo`——优先复用状态 Select，减少新控件。

**编辑范围**：详情既有保存路径已覆盖标题/描述/优先级/标签（`TaskDeliverySection`）；归档打开后不额外锁死这些字段（执行类按钮可对 archived 保持禁用，沿用 `canStartPipeline` 等现有门禁）。

### 2.6 批量状态

- 继续 `batchUpdateTasks({ task_ids, status })`。
- UI 文案中文；成功后 `fetchTasks`。
- Activity：`tasks_batch_updated` 已有中文 label，无需新 key。

## 3. 组件职责

| 组件 | 变更要点 |
|------|----------|
| `src/lib/kanbanFilters.ts`（新） | 纯过滤 + 可选单元测试友好 |
| `KanbanPage.tsx` | 中文 SelectValue；加载 taskTagMap；共用过滤；可选优先级/执行人筛选 |
| `KanbanBoard.tsx` | 应用 tag（及可选）过滤；传 milestone map / tags 给卡片 |
| `TaskCard.tsx` | 里程碑徽章；优先用 props 标签减少重复请求 |
| `CreateTaskDialog.tsx` | 里程碑 Select |
| `ArchiveManagementDialog.tsx` | 打开详情 + 刷新回调 |
| `TaskOverviewPanel.tsx` | archived 可改状态取消归档 |
| `utils.ts` | 仅当缺 activity 中文时补 |

## 4. SSH / 本地

- `projects` / `environmentMode` 已限制可见项目；归档 `projectIds: projects.map(id)` 保持该边界。
- 里程碑/标签按 `project_id` 拉取，不跨 SSH/本地混用。
- 无新远程文件路径依赖。

## 5. 兼容与回滚

- **无 schema 变更** → 回滚即 git revert 前端提交。
- 过滤纯函数独立，便于回退 Board 旧逻辑。
- 若批量标签查询过慢：降级为「标签筛选仅当前项目 + 分页加载」，或后续再加 `list_task_ids_by_tag` command（本任务 out-of-band 决策点，默认前端 map）。

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| Page / Board 过滤不一致 | 共用 `filterKanbanTasks` |
| 标签 N+1 | 板级 map；卡片 props 优先 |
| 归档任务不在 store | 详情内 update 走 command；关闭时强制 reload 归档列表 |
| 取消归档后卡片不出现 | unarchive 后 `fetchTasks` |
| Select 组件 API 差异 | 对齐现有 `TaskOverviewPanel` 的 `SelectValue` 渲染模式 |

## 7. 非目标（设计层）

- 不改四引擎 / automation / git workflow。
- 不做依赖图可视化。
- 不重做归档永久删除产品流程（仅保证若有删除则二次确认）。
