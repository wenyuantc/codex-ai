# Implement: 前端列表渲染性能优化

## Checklist

### Phase A — Shared clock

- [x] 新增 `src/hooks/useSharedNow.ts`（单 interval + 订阅；`enabled` 门控）
- [x] 新增 `src/components/tasks/TaskElapsedSummary.tsx`，承接 TaskCard 耗时文案
- [x] `TaskCard.tsx`：删除 `timerNow` / interval effect；渲染 `TaskElapsedSummary`；`memo` 导出
- [x] `TaskOverviewPanel.tsx`：改用 `useSharedNow`，删除本地 interval

### Phase B — Memo + stable props

- [x] `KanbanColumn`：`memo` 包装
- [x] `KanbanBoard`：`onOpenLog` / `onGitActionCompleted` 等用 `useCallback` 稳定
- [x] 确认 `TaskCard` 调用点 props 无内联新对象导致 memo 失效（能稳则稳）

### Phase C — Virtualization

- [x] `npm install @tanstack/react-virtual`
- [x] `KanbanColumn`：阈值 ≥ 25 时 `useVirtualizer`；否则原 map
- [x] 滚动容器 `ref` + totalSize 占位 + absolute 行
- [ ] 冒烟：拖拽、跨列、大量任务列滚动（需 `npm run tauri:dev` / `npm run dev` 手工）

### Phase D — Verify

- [x] `grep -R setInterval src/components/tasks`：TaskCard / TaskOverviewPanel 无 per-instance 计时 interval
- [x] `grep -R memo src/components/tasks`：TaskCard / KanbanColumn 有导出
- [x] `npm run build`
- [ ] 手工：看板计时刷新、拖拽、打开详情

### Phase E — Spec / context (finish path)

- [x] 视情况更新 `.trellis/spec/frontend/quality-guidelines.md` 性能条目（共享 now / memo / 虚拟列）
- [x] `task.py add-context` 写入用到的 frontend specs
- [x] 提交（Conventional：`perf(frontend): ...`）

## Validation Commands

```bash
npm run build
# optional smoke
npm run dev
# or
npm run tauri:dev
```

## Review Gates

1. 无后端/DB 改动  
2. 无 UI 视觉改版  
3. 计时文案语义与改前一致  
4. 虚拟化关闭路径（列 < 25）与改前 DOM 结构等价（class/结构可微调但交互一致）

## Rollback Points

| After | Rollback |
|-------|----------|
| A only | 删 hook/leaf，还原 TaskCard/Overview interval |
| A+B | 去掉 memo/useCallback |
| C | 提高阈值或删 virtualizer 分支 + uninstall 依赖 |

## Order Notes

- 先 A 再 B 再 C：A 单独可验证「N 卡仅 1 interval」  
- C 失败时保留 A+B 仍有明确收益  
