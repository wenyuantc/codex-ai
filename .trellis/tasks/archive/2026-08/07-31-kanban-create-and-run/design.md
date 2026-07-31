# Design: 看板新建任务支持创建并执行

## Architecture / Boundaries

| 层 | 职责 | 本任务变更预期 |
|---|---|---|
| UI `CreateTaskDialog` | 双提交、校验、阶段文案、错误展示 | **改** |
| 编排 helper（新建） | 创建后可选 plan → 启动执行的可 await 流程 | **增**（优先纯函数/模块，避免 dialog 内堆逻辑） |
| `useTaskExecutionActions` | 引擎启动、状态/计时/日志 | **小改或抽内核**，供 dialog 与卡片复用 |
| `KanbanBoard` / `KanbanPage` | `onOpenLog` 向下传递 | **小改** |
| Backend create / plan / run commands | 已有 | **默认不改**；仅在活动 key 中文缺失时补前端映射或后端 log |
| DB schema | `coordinator_id` / `plan_content` 已有 | **不改** |

原则：业务写仍走现有 Tauri 命令；前端只做编排。

## Data Flow

```
用户点击「创建并执行」
  │
  ├─ 前端校验（标题/项目/assignee/审查员/defaults）
  │     失败 → 弹窗错误，不创建
  │
  ├─ createTask(+ tags/deps/attachments)   [已有]
  │
  ├─ 若 coordinator_id 有值：
  │     aiGenerateCoordinatorTaskPlan
  │     updateTask({ plan_content })
  │     失败 → 保留任务，弹窗错误，不 run
  │
  ├─ startTaskRun(task, { planContent? })  [复用执行内核]
  │     prepareExecutionInput / worktree / start* engine
  │
  └─ 成功 → onOpenLog(taskId, "execution") + 关闭弹窗 + refresh 列表
```

「创建」按钮：只走到 createTask 分支后关闭，与现状一致。

## Contracts

### CreateTaskDialog props（增量）

```ts
interface CreateTaskDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  projectId?: string;
  /** 创建并执行启动成功后打开任务日志 */
  onOpenLog?: (taskId: string, sessionKind?: "execution" | "review") => void;
}
```

### 编排 API（建议形状）

```ts
// e.g. src/lib/taskCreateAndRun.ts 或 hooks 旁的 pure module
type CreateAndRunPhase = "idle" | "creating" | "planning" | "starting";

async function runCreatedTaskExecution(params: {
  task: Task;
  assignee: Employee;
  projectRepoPath: string | null;
  planContent?: string | null;
  // stores / engine start deps injected or imported from existing modules
}): Promise<void>;

async function generateAndPersistCoordinatorPlan(params: {
  task: Task;
  coordinatorId: string;
  workingDir: string | null;
}): Promise<string>; // trimmed plan, throws on empty/failure
```

执行内核应与 `useTaskExecutionActions.startExecution("run", …)` 行为一致：

- `buildTaskExecutionInput`（含 plan / 附件图片）
- `prepareTaskGitExecution` when `use_worktree`
- provider 分支：claude / opencode / grok / codex
- `updateTaskStatus(in_progress)`、`startTaskTimer`、`updateEmployeeStatus(busy)`、runtime refresh
- 错误写入 task 执行日志（`addCodexOutput`）

**推荐实现顺序**：先抽出 `startTaskRunSession(...)` 非 hook 函数，hook 改为薄封装调用它；`CreateTaskDialog` 直接调用同一函数。避免 dialog 复制启动代码。

### 阶段与按钮文案

| phase | 主按钮文案示例 |
|---|---|
| idle | 创建并执行 |
| creating | 创建中… |
| planning | 生成协调员计划… |
| starting | 启动执行… |

「创建」在任意非 idle 提交中禁用。

### 错误策略

| 阶段 | 任务是否已创建 | UI |
|---|---|---|
| 校验失败 | 否 | `createError`，不关弹窗 |
| create 失败 | 否 | `createError` |
| plan 失败 | 是 | `createError` 说明可稍后从看板重试；不关弹窗（用户可手动关） |
| run 启动失败 | 是（可能已 in_progress 部分状态） | `createError` + 任务日志 ERROR；遵循现有 hook 错误处理 |

不引入 toast 依赖（仓库当前无 toast）。

## Compatibility

- **SSH**：`project_type === "ssh"` 时 `projectRepoPath` / 引擎 start 路径与卡片一致（workingDir 来自项目远程路径；worktree 走 `prepareTaskGitExecution`）。
- **附件**：创建时 `attachment_source_paths` 已由 create 命令托管；run 时 `fetchAttachments` + `buildTaskExecutionInput` 取图片。
- **自动质控默认**：仅创建时按现有逻辑可能写入 automation；创建并执行不额外改 automation 语义。
- **多任务员工**：沿用现有 runtime 多 session 能力，不新增队列。

## Trade-offs

| 选择 | 理由 | 代价 |
|---|---|---|
| 固定自动 plan，无开关 | 范围可控，复用卡片确认流作人工路径 | 复杂任务无法在创建口审计划 |
| 成功前不关弹窗 | 无 toast 时错误仍可见；避免卸载中断 | 计划生成期间弹窗占用 |
| 抽执行内核而非 dialog 内联 | 与卡片一致，防漂移 | 需小心改 hook 回归 |

## Rollback

- 功能隔离在创建弹窗 + 可选抽取的 run helper；回滚可隐藏「创建并执行」按钮并恢复 hook 内联（若抽取引起回归）。
- 无 migration，无 schema 回滚。

## Non-goals (design)

- 后端新「create_and_run」单体命令（前后端耦合过重，且 plan 使用 one-shot AI，适合前端编排）。
- 改 `CoordinatorPlanDialog` 行为。
