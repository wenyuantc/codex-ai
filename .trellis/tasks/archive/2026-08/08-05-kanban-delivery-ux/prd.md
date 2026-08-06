# 看板交付 UX 与可发现性

## Goal

让里程碑、标签、依赖、批量改状态、筛选、归档管理在看板上 **可发现、可操作、文案中文**，消除「后端有、前端不好用」的交付能力断层。

## Background（代码确认）

- Delivery 后端已存在：`src-tauri/src/app/delivery.rs`（里程碑/标签/依赖）+ `batch_update_tasks`（`app/tasks.rs`）。
- 前端部分已有：
  - `KanbanPage`：关键词、逾期/阻塞、里程碑/标签筛选入口、批量改状态、归档管理入口。
  - `TaskDeliverySection`：详情 Overview 可维护截止/里程碑/标签/依赖。
  - `TaskCard`：已拉标签与依赖数并展示徽章；**未展示里程碑**。
  - `CreateTaskDialog`：可设标签与依赖；**无里程碑字段**。
  - `ArchiveManagementDialog`：可筛选列表与中文优先级/时间；**不可打开详情、不可编辑**。
- 已知断层（本任务必须关闭）：
  1. **标签筛选未生效**：`KanbanBoard` 将 `tagId` 记为 `_tagId` 且注释「后续再做」；`KanbanPage.filteredTaskIds` 也未按标签过滤。
  2. **批量状态 SelectValue** 可能直接露出 `todo` / `in_progress` 等 raw enum（缺中文渲染）。
  3. **归档不可操作**：列表只读；详情侧 `status === "archived"` 时状态 Select 禁用，无明确取消归档路径。
  4. **卡片缺里程碑徽章**；创建任务缺里程碑入口。
- 活动键 `tasks_batch_updated` 已在 `getActivityActionLabel` 映射为「批量更新任务」；批量路径应继续写该 activity，不新造 key 除非缺中文。
- 项目作用域：看板依赖 `currentProject` + `environmentMode`（local/SSH 项目列表已由 project store 隔离）；不在本任务重做全局项目切换。

## Requirements

### R1 筛选与批量

- 看板筛选至少覆盖：**关键词、逾期、阻塞、里程碑、标签**；可选增强 **优先级、执行人**（有则中文展示）。
- 所有枚举类筛选/批量控件的 **Trigger 与选项** 均展示中文标签（`TASK_STATUSES` / `PRIORITIES` / `getStatusLabel` / `getPriorityLabel`），不向用户露出 raw enum。
- **标签筛选必须真正过滤** 看板列与「全选筛选结果」。
- 批量改状态：成功/失败反馈中文；后端继续 `tasks_batch_updated`（或等价中文 activity）。

### R2 里程碑 / 标签 / 依赖可发现

- 任务卡：有则显示 **标签**、**里程碑**、**依赖摘要**（依赖至少保留数量徽章）。
- 任务详情 Overview：继续通过 `TaskDeliverySection` 维护标签/里程碑/依赖/截止；归档打开后关键字段仍可改。
- 创建任务：补齐 **里程碑** 选择（标签/依赖已有则保持）。

### R3 归档管理

- 归档列表行可打开 `TaskDetailDialog`（或等价详情入口）。
- 至少可编辑：**标题、描述、标签、优先级**；支持 **取消归档**（改回非 `archived` 活跃状态）并与现有 `update_task_status` / 状态机规则一致。
- 永久删除若从归档入口暴露，保持二次确认（复用 `DeleteTaskDialog`）；若当前入口无永久删除，不强行新增。

### R4 兼容与一致性

- SSH / 本地：筛选与归档列表仅基于当前 environment 可见项目（沿用 store，不跨环境泄漏）。
- 时间展示一律 `formatDate()`。
- 业务写路径仅 Tauri commands（`backend.ts` 包装）；禁止前端直写 SQL。
- 新增 activity action 必须补 `getActivityActionLabel` 中文（本任务优先复用已有 key）。

## Acceptance Criteria

- [x] AC1：看板所有筛选/批量状态下拉，选中后 Trigger **无** 英文 raw 值（如 `in_progress`）
- [x] AC2：选择某标签后，看板仅显示带该标签的任务；「全选筛选结果」与列内集合一致
- [x] AC3：任务卡在有数据时可见标签与里程碑徽章
- [x] AC4：归档管理可打开详情，编辑标题/描述/标签/优先级并保存成功；可取消归档
- [x] AC5：批量改状态成功，仪表盘/活动日志显示中文「批量更新任务」（或等价中文）
- [x] AC6：创建任务可选里程碑并落库
- [x] AC7：`npm run build` 通过；若改 Rust：`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 与相关 `cargo test` 通过

## Out of Scope

- 全新甘特图 / 跨项目依赖图可视化
- 里程碑/标签的独立管理后台大页（项目级 CRUD 若已有入口则不扩 scope）
- 批量改优先级/执行人 UI（后端 payload 可能已支持，本任务不强制做 UI）
- 前端测试网基建（属 `08-05-frontend-test-net`；本任务可不加测试，若有纯过滤函数可顺手单测）

## Key Decisions

| 决策 | 选择 |
|------|------|
| 标签过滤数据源 | 前端按当前可见任务批量 `list_task_tags` 建 `taskId → tagIds` 映射；不新增 list-by-tag 后端 API（除非性能不可接受） |
| 归档编辑 | 复用 `TaskDetailDialog`，不另做精简编辑表单 |
| 取消归档 | 详情状态 Select 在 archived 时允许选回活跃状态（`ACTIVE_TASK_STATUSES`） |
| 项目筛选 | 继续用全局当前项目 + environmentMode，不在看板再做一套项目切换 |

## Risks / Notes

- 每卡/筛选批量拉 `list_task_tags` 可能 N+1：实现时在看板层集中加载映射并下发，避免每卡重复请求放大。
- 归档任务可能不在 `taskStore.tasks` 活跃列表：打开详情后更新需刷新归档列表与（取消归档后）看板 `fetchTasks`。
