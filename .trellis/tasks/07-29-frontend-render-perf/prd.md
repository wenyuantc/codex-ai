# 前端列表渲染性能优化

## Goal

消除看板 TaskCard 每卡片独立 1s `setInterval` 造成的 N 倍重渲染，引入 `React.memo` 阻断无关更新，并对看板长列启用虚拟化，使任务数增长时 UI 保持可用（父优化项 #4）。

## Background / Evidence

代码实测（2026-07-29 / 2026-07-30）：

| 问题 | 证据 |
|---|---|
| 每卡片 1s 定时器 | `TaskCard.tsx`：`time_started_at` 存在时 `setInterval(..., 1000)` + `setTimerNow` |
| 详情面板同类问题 | `TaskOverviewPanel.tsx` 同样 per-instance interval |
| 无 memo | 全 `src/` `React.memo` / `memo(` 计数 = 0 |
| 无虚拟列表 | 无 `@tanstack/react-virtual` / `react-window` 等依赖；`KanbanColumn` 全量 `tasks.map` |
| 重卡片 | `TaskCard.tsx` ~1312 行，含 dnd-kit sortable、多 store 订阅、多 dialog |

父任务约束：本子任务为 C4，建议在 C3（读路径下沉）之后；当前 C3 仍为 planning。本任务**不依赖** C3 的 store 读路径改造，仅消费稳定的 `Task` props 与现有 store selector API。

## Requirements

### R1 — 统一计时驱动

- 将「每秒 now」从卡片/面板本地 `setInterval` 上提为**全局最多一个**订阅源。
- `TaskCard`、`TaskOverviewPanel`（及其他消费 `getTaskElapsedSeconds(..., now)` 的列表级 UI）不得各自 `setInterval`。
- 未在计时中的任务（无 `time_started_at`）不得强制订阅秒级更新。
- 计时展示精度仍为秒级；文案与 `formatDuration` / `getTaskElapsedSeconds` 语义不变。

### R2 — React.memo 与重渲染隔离

- 对看板热路径组件引入 `memo`：至少 `TaskCard`、`KanbanColumn`。
- 计时相关状态变更应尽量只触发**耗时标签子树**重渲染，而不是整张卡片。
- 父组件传入的回调（`onOpenLog`、`onGitActionCompleted`、`onToggleSelected` 等）在看板侧保持引用稳定（`useCallback`），避免 memo 失效。
- 不改变 TaskCard 对外 props 契约的语义（可新增可选 props，不得破坏现有调用方）。

### R3 — 长列表虚拟化

- 看板列在任务数达到阈值时启用虚拟列表，仅挂载可视区 + overscan 内的卡片。
- 阈值默认 **25**（可常量配置）；低于阈值保持现有全量渲染，避免小列表引入测量开销。
- 与 `@dnd-kit` 拖拽共存：拖拽排序/跨列移动行为在用户可感知层面保持可用（允许 overscan 内拖拽；不允许拖拽时整列崩溃或无法落点）。
- 不引入第二套 UI 框架；虚拟化库优先 `@tanstack/react-virtual`。

### R4 — 范围与非目标

**做：**

- 前端渲染路径：`TaskCard` / `KanbanColumn` / 共享 now hook / 可选小的 elapsed 子组件
- 详情概览计时同源修复（`TaskOverviewPanel`）
- 依赖：按需新增 `@tanstack/react-virtual`

**不做：**

- UI 视觉改版
- 后端 / DB / Tauri 命令变更
- 改写 taskStore 读路径（属 C3）
- Sessions 页已有分页，本任务不强制再虚拟化（除非实现中发现同等热路径）
- 拆分 `TaskCard.tsx` 巨型文件为完整重构（可抽计时/虚拟化相关小组件，不做大搬家）

## Constraints

1. 兼容本地与 SSH 项目：不改变任务执行/Git 相关行为，仅渲染层。
2. `npm run build`（tsc + Vite）必须通过。
3. 独立分支 `feat/frontend-render-perf`，独立提交合回 `main`（父任务交付约定）。
4. 时间展示继续走 `formatDate` / `formatDuration` / `getTaskElapsedSeconds`，不手写 locale。
5. 命名导出、PascalCase 组件、2 空格缩进，与现有 frontend spec 一致。

## Acceptance Criteria

- [x] 全 `src/` 中，任务耗时 UI **不再**出现「每个 TaskCard/TaskOverviewPanel 实例各自 `setInterval`」；全局秒级 now 源 ≤ 1 个活跃 interval
- [x] 至少 `TaskCard`、`KanbanColumn` 使用 `React.memo`（或 `memo()`）导出
- [x] 看板单列任务数 ≥ 阈值时启用虚拟列表；< 阈值时行为与现网一致
- [ ] 拖拽排序、跨列移动、选中高亮、打开详情/日志等现有看板交互冒烟通过
- [x] 计时中任务仍每秒更新耗时文案；停止计时后不再订阅秒级更新（实现：仅 `time_started_at` 时订阅）
- [x] `npm run build` 通过
- [x] 无后端/DB 迁移；无非必要依赖膨胀（仅允许虚拟化库）

## Dependencies / Ordering

- **建议前置**：C3 `07-29-read-path-to-command`（稳定 store 接口）。本任务在 store 接口未变前提下可独立落地。
- **无硬阻塞**：不依赖 C1/C2/C5–C7。

## Notes

- 父任务目录：`07-29-codex-ai-optimization`
- 源发现编号：#4
- 分支：`feat/frontend-render-perf`（已存在）
