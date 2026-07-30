# Design: 前端列表渲染性能优化

## Summary

三层收敛，从高 ROI / 低风险到中等复杂度：

1. **共享 now 时钟** — 模块级单 interval + 订阅式 hook；计时展示拆为叶子组件，避免整卡每秒 setState  
2. **memo** — `TaskCard` / `KanbanColumn` 包装 + 父级稳定回调  
3. **列虚拟化** — `@tanstack/react-virtual`，列内任务数 ≥ 阈值时启用，与 dnd-kit 共存

不改 store / IPC / 后端。

## Architecture

```
KanbanBoard
  └─ KanbanColumn (memo)
       └─ SortableContext(items=allIds)
            └─ [virtual? useVirtualizer : map]
                 └─ TaskCard (memo)
                      └─ TaskElapsedSummary  ← useSharedNow(running)
                           (only this leaf re-renders each second)

TaskOverviewPanel
  └─ useSharedNow(Boolean(timeStartedAt))  // same clock
```

### Shared clock

**File:** `src/hooks/useSharedNow.ts`（新；项目尚无 `src/hooks` 目录惯例时放此处，与 feature hooks `components/*/hooks` 区分：这是跨域通用 hook）

**Module singleton：**

- `listeners: Set<(now: number) => void>`
- `intervalId: number | null`
- 首个订阅者启动 `setInterval(1000)`，最后一个取消时 `clearInterval`
- `useSharedNow(enabled: boolean): number`  
  - `enabled === false`：返回稳定的初始/`Date.now()` 快照，**不**加入 listeners  
  - `enabled === true`：订阅，每秒 `setState(now)`  

**保证：** 无论多少卡片/面板启用，浏览器内最多 1 个 interval。

### Elapsed leaf

**File:** `src/components/tasks/TaskElapsedSummary.tsx`（新）

- Props：`task: Pick<Task, "status" | "time_started_at" | "time_spent_seconds" | "completed_at" | "updated_at" | "created_at">` + 可选 `className`
- 内部：`useSharedNow(Boolean(task.time_started_at))` + 现有 `getTaskElapsedSeconds` / `formatDuration` / `formatDate` 文案逻辑（从 TaskCard 抽出 `taskTimeSummary` 等价输出）
- TaskCard 删除本地 `timerNow` state 与 interval effect

`TaskOverviewPanel` 删除本地 interval，改为 `useSharedNow`；可内联或复用 leaf（详情布局不同，优先 hook 复用，不必强行共用 leaf UI）。

### React.memo

```tsx
// TaskCard.tsx
function TaskCardInner(props: TaskCardProps) { ... }
export const TaskCard = memo(TaskCardInner);
// 保持具名：部分调试器显示 memo 内名称；可 TaskCardInner.displayName

// KanbanColumn.tsx
export const KanbanColumn = memo(function KanbanColumn(...) { ... });
```

**回调稳定（KanbanBoard）：**

- `onOpenLog` / `onGitActionCompleted` 已有则包 `useCallback`
- `onToggleTaskSelection` 由页面传入；Board 透传时不包一层新闭包

**Store 选择：** TaskCard 已用窄 selector（`useTaskStore((s) => s.xxx)`）；不改为订阅整个 store。`automationStates[task.id]` 仍会按任务触发重渲染，属正确行为。

### Virtualization

**Dependency:** `@tanstack/react-virtual`

**Site:** `KanbanColumn` 内滚动容器

```
const VIRTUALIZE_THRESHOLD = 25;
const ESTIMATED_CARD_HEIGHT = 140; // px, overscan 缓冲
const OVERSCAN = 4;

const parentRef = useRef<HTMLDivElement>(null);
const shouldVirtualize = tasks.length >= VIRTUALIZE_THRESHOLD;
const virtualizer = useVirtualizer({
  count: shouldVirtualize ? tasks.length : 0,
  getScrollElement: () => parentRef.current,
  estimateSize: () => ESTIMATED_CARD_HEIGHT,
  overscan: OVERSCAN,
  enabled: shouldVirtualize,
});
```

- `enabled: false` 或 count=0 时走现有 `tasks.map`（零行为变化）
- 虚拟模式：外层 `height: virtualizer.getTotalSize()`，绝对定位行 + `transform: translateY`
- `SortableContext items={tasks.map(t => t.id)}` **始终传全量 id**（碰撞检测需要）
- 卡片实际高度可变 → 依赖 estimate + overscan；不强制 `measureElement` 第一版（可后续加）

**Drag 注意：**

- DragOverlay 已在 `KanbanBoard` 使用，虚拟化不卸载 overlay
- 拖出可视区时 overscan 提供缓冲；跨列 drop 依赖 droppable 列本身，不依赖源列虚拟项

## Data / Control Flow

| 输入 | 来源 | 输出 |
|------|------|------|
| `task.time_started_at` | props / store | 是否 `useSharedNow(true)` |
| `tasks[]` | KanbanColumn props | 是否虚拟化、render 窗口 |
| drag events | dnd-kit | 不变，仍由 Board 处理 |

无 IPC、无 DB。

## Tradeoffs

| 选项 | 取舍 |
|------|------|
| 模块单例 vs Context Provider | 单例无需改 Layout 树；Context 更 React 惯用但要挂 Provider。选单例 + hook，测试时可 reset listeners。 |
| 整卡 memo vs 只拆 elapsed | 两者都做：memo 挡父级噪声，elapsed 挡秒级刷新。 |
| 虚拟化阈值 25 | 小列零开销；大列才 window。可调常量。 |
| dnd + virtual 完整测高 | v1 固定 estimate，接受轻微滚动空隙；避免 measure 复杂度。 |
| Sessions 虚拟化 | 已分页，排除本任务。 |

## Compatibility

- 本地 / SSH：无差异（纯 UI）
- React 19：`memo` / hooks 兼容
- `@dnd-kit`：SortableContext 全量 ids + 局部 mount 是社区常见模式

## Rollout / Rollback

- 单分支单 PR；回滚即 revert 提交 / 卸依赖
- 无迁移、无 feature flag
- 风险缓解：阈值开关，必要时将 `VIRTUALIZE_THRESHOLD` 提到极大值即禁用虚拟化而保留 clock+memo

## Files Touched (expected)

| Path | Change |
|------|--------|
| `src/hooks/useSharedNow.ts` | 新增 |
| `src/components/tasks/TaskElapsedSummary.tsx` | 新增 |
| `src/components/tasks/TaskCard.tsx` | 去 interval；用 leaf；memo |
| `src/components/tasks/detail/TaskOverviewPanel.tsx` | useSharedNow |
| `src/components/tasks/KanbanColumn.tsx` | memo + virtualizer |
| `src/components/tasks/KanbanBoard.tsx` | 稳定回调（如需） |
| `package.json` / lock | `@tanstack/react-virtual` |
| `.trellis/spec/frontend/*` | 可选：性能约定写入 quality/component guidelines |

## Out of Scope

- TaskCard 大文件拆分重构
- taskStore 分页 / 读路径（C3）
- 全局 list 基础设施抽象层
