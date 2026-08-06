# Design: 任务详情主路径与 SSH 可信提示

## Summary

用**纯函数主 CTA 解析器**统一任务卡片与详情的主操作语义；在 `TaskDetailDialog` 增加**粘性主操作条**，次要操作收进菜单/既有 Tab；在现有 SSH 顶栏提示上**增强可发现性与跳转**，并在详情侧对「会话 artifact 受限」复用同一文案组件，避免「SSH 模式全量提示」与「具体会话受限」两套口径漂移。

## Boundaries

| Layer | In scope | Out of scope |
|-------|----------|--------------|
| Frontend utils | `resolveTaskPrimaryCta` + 展示标签 | 改后端 phase 枚举 |
| TaskCard | 用解析器驱动现有主按钮文案/禁用 | 重做卡片布局 |
| TaskDetailDialog | sticky 主 CTA 条 + 次要菜单 | 完整信息架构重写 |
| Layout | 增强 `MainLayout` SSH banner | 新通知子系统 |
| Backend / DB | 无（复用现有 automation_state / sessions） | 新 migration / 新 command |
| Activity labels | 仅当新增 action 时补中文 | 无新 action 则不动 |

## Architecture

```text
task + automationDisplay + runtime (run/review) + git/commit capability
        │
        ▼
 resolveTaskPrimaryCta()   ← pure, unit-testable (TS 逻辑先放 utils，可后续抽单测)
        │
        ├─► TaskCard 主按钮区（视觉保持小按钮，语义同源）
        └─► TaskDetailDialog sticky PrimaryActionBar
                 ├─ primary button → 已有 handlers（run/stop/review/commit/acceptance）
                 └─ overflow menu → 其余操作（不新增业务能力）
```

SSH 提示：

```text
environmentMode === "ssh"
  → MainLayout sticky banner（增强文案 + 设置链接）

session.artifact_capture_mode in {ssh_git_status, ssh_none}
  → 既有 SshArtifactLimitedNotice（详情/会话/变更史，不删除）

可选：当前 project.project_type === "ssh" 且全局仍为 local
  → 不强制全局 banner（避免误报）；仅在打开该项目任务详情且历史会话有 limited mode 时，在详情顶部嵌 compact notice
```

## Primary CTA State Table

输入（最小集合）：

- `task.status`
- `TaskAutomationDisplayState`（`getTaskAutomationDisplayState`）
- `getTaskActionRuntimeState` → `executionActive` / `reviewActive`
- 指派：`assignee_id` / `reviewer_id`
- 能力：`canCommit`（与 TaskCard 现有 `canCommitTaskCode` 同语义）、`canGenerateAcceptance`（有 tester 等现有条件）
- 后台启动：`backgroundPlanning` / `backgroundStarting`（可选细化 label）

**优先级从上到下，命中即停（唯一主 CTA）：**

| # | 条件 | kind | 标签 | 可点？ | 备注 |
|---|------|------|------|--------|------|
| 1 | `executionActive` 且进程可停 | `stop` | 停止 | yes | 对齐 TaskCard 可 stop 分支 |
| 2 | `executionActive` 但仅 automation/pipeline 占用 | `running_locked` | 运行中 / 修复中… | no | 文案：`launching_fix`/`waiting_execution`/`committing_code` →「修复中…」；pipeline →「编排中」；else「运行中」 |
| 3 | background planning/starting | `starting` | 生成计划中 / 启动中 | no | 对齐后台启动 |
| 4 | `reviewActive` 或 `status===review` | `review` | 审核 / 审核中 | need reviewer | 无 reviewer → disabled + hint「请先指定审查员」 |
| 5 | `status===blocked` | `blocked` | 查看阻塞原因 | soft | 主按钮滚动/聚焦 overview 阻塞字段；不启动会话 |
| 6 | `status===completed` 且 `canCommit` | `commit` | 提交代码 | yes | 打开既有提交对话框（卡片已有逻辑） |
| 7 | `status===completed` 且可生成验收 | `acceptance` | 生成验收清单 | yes | 详情已有 handler；卡片可菜单保留 |
| 8 | `status===archived` | `none` | — | — | 无主 CTA，仅提示已归档 |
| 9 | 默认（todo / in_progress / 等） | `run` | 运行 | need assignee | 无 assignee → disabled +「未指派」 |

**次要操作**（不进主 CTA，详情 overflow / 卡片菜单）：停止以外的运行、手动审核、AI 提交、合并、生成验收（当主 CTA 不是它时）、自动化开关、删除等——**行为保持现状，只改入口层级**。

## Data Sources (single source)

| 展示面 | automation | runtime |
|--------|------------|---------|
| TaskCard | `getTaskAutomationDisplayState(task, store.automationStates[id])` | hooks + `getTaskActionRuntimeState` |
| TaskDetailDialog | 同上；props `automationState` 优先，否则 store | 同上 |

禁止：详情本地再算一套 phase 文案或「按钮可点但 automation 锁定」分叉。  
`resolvedAutomationState` 已存在于详情；主 CTA 必须基于同一 `runtimeState`。

## UI Contracts

### `resolveTaskPrimaryCta(input): TaskPrimaryCta`

```ts
type TaskPrimaryCtaKind =
  | "stop"
  | "running_locked"
  | "starting"
  | "review"
  | "blocked"
  | "commit"
  | "acceptance"
  | "run"
  | "none";

interface TaskPrimaryCta {
  kind: TaskPrimaryCtaKind;
  label: string;
  disabled: boolean;
  reason?: string; // tooltip / aria
  tone: "danger" | "primary" | "warning" | "muted";
}
```

放置：`src/lib/taskPrimaryCta.ts`（或 `src/lib/utils.ts` 旁独立模块，避免 utils 继续膨胀）。  
**禁止**在组件内复制 if/else 表。

### Detail sticky bar

- 位置：`TaskDetailDialog` 标题区下方或 `DialogContent` 底部 sticky（推荐**底部 sticky**，不挡标题与 Tab）。
- 左：automation 状态 chip（复用 `getTaskAutomationStatusLabel`，已有）。
- 右：主按钮 + `⋯` 菜单（次要）。
- 点击主按钮：调用现有 handler，不新建 command。
- 默认 Tab 不强制跳转；若 kind 与当前 Tab 无关（例如在 overview 点「审核」），可 `setDetailTab("review")` 后再触发——推荐「先切 Tab 再执行」仅当 kind 为 `review`/`run`/`stop`。

### SSH Banner 增强

现有 `MainLayout`（`environmentMode === "ssh"`）已有 amber 条。本任务：

1. 抽成 `SshTrustBanner`（或扩展 `SshArtifactLimitedNotice` 的 `variant="app-bar"`），文案统一。
2. 增加「打开设置」链接 → `/settings`（已有无配置自动跳转逻辑，链接仍有助于有配置用户看说明）。
3. 可选 dismiss（sessionStorage），默认不 dismiss 以免漏看；若实现 dismiss，刷新会话后可再出现一次。
4. **不**在 local 模式对全部项目刷全局 banner；SSH 项目在 local 全局模式下依赖会话级 notice。

## Compatibility

- **SSH**：banner + limited notice 均需可用；不依赖新 IPC。
- **自动化**：`getTaskActionRuntimeState` 已把 review/execution/pipeline 锁住；主 CTA 表第 1–2 行必须与之对齐，避免「显示运行但实际 locked」。
- **多引擎**：CTA 只调用现有 `useTaskAiActions` / execution / review hooks，不碰 engine 差异。

## Trade-offs

| 选择 | 利 | 弊 |
|------|----|----|
| 纯函数 CTA vs 组件内逻辑 | 卡片/详情一致、可测 | 需把 commit/acceptance 能力作为输入传入 |
| 详情 sticky 主条 vs 只改卡片 | 解决「详情不知点哪」 | Dialog 布局多一行，需注意小屏 |
| 增强全局 SSH banner vs 仅会话 notice | 满足 AC「无需点进深层」 | 全 SSH 模式持续提示（已存在，可接受） |
| 无后端改动 | 风险低、可快速交付 | 无法服务端定义 CTA；前端表需与 phase 同步维护 |

## Rollback

- 删除 sticky bar + 解析器引用，恢复 TaskCard 内联 if/else 与 MainLayout 内联 banner。
- 无 DB/migration，回滚为纯前端 revert。

## Risks

1. TaskCard 与详情 handler 入参不完全同构（commit 对话框开在卡片、详情未必有）→ **详情 sticky 的 commit/acceptance 仅绑定详情已有能力；缺能力时降级为 `setDetailTab` + 提示，或隐藏该 kind**。  
   **决策**：`commit` 若详情尚无打开提交对话框的路径，主 CTA 改为引导到卡片同款流程：在详情内接线现有 `get_task_commit_action_state` / commit dialog（若已有共享 hook 则复用；否则本任务最小接线，不新造提交算法）。
2. status 与 runtime 竞态（status 仍是 todo 但 session running）→ **runtime 优先级高于 status**（表 #1–3）。
3. 验收与提交在 completed 并存 → **提交优先于验收**（表 #6 > #7），因误提交风险低于漏验收清单。
