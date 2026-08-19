# Implement · B1 前端对接

后端（表 / 闸门 / drain / 命令 / Rust 测试）已在 `8dbd82a`。本清单只覆盖剩余 UI 与类型适配。

## Checklist

- [x] 类型与 IPC：`StartSessionOutcome`、`TaskRunQueueItem`、`CodexSettings.max_concurrent_sessions`、`UpdateCodexSettingsInput`；`listTaskRunQueue` / `cancelQueuedTaskRun`
- [x] 四引擎 `start_*` 返回 outcome（`codex.ts` / `claude.ts` / `grok.ts` / `opencode.ts`）
- [x] `startTaskRunSession`：先启动，queued 跳过 busy / in_progress / timer，返回 outcome
- [x] 带 `taskId` 的其它执行入口与内核语义对齐（优先复用 `startTaskRunSession`）
- [x] `taskStore` 队列缓存 + `task-run-queue-changed`；`MainLayout` 注册/清理
- [x] `resolveTaskPrimaryCta` 增加 queued 锁定；`taskPrimaryCta.test.ts`
- [x] TaskCard 徽标 + 右键取消排队
- [x] RuntimeSettingsTab 本地并发上限输入；SettingsPage 读/写本地设置
- [x] KanbanPage「批量运行」+ started/queued/skipped 汇总
- [x] i18n：settings / kanban / tasks / activity（含 4 个 action key）zh-CN + en
- [x] 验证命令

## Validation

```bash
npm run test:ci
npm run format:check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml run_queue
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

手工冒烟（`npm run tauri dev`）：上限改成 1 → 连续运行 2 个任务 → 第二张卡显示排队第 1 位 → 结束第一个后自动启动 → 取消排队 → 批量运行选中任务 → 仪表盘活动流中文 → SSH 项目同样排队。

## Risky Files

- `src/lib/taskRunSession.ts`：副作用顺序错了会把排队任务标成进行中。
- `src/stores/taskStore.ts`：listener 泄漏或漏刷新。
- `src/pages/SettingsPage.tsx`：误把上限写进 remote settings。
- `TaskDetailDialog` / `CodexControls`：漏适配会双路径行为不一致。

## Rollback

只 revert 本轮前端提交。不要回退 v46 / `run_queue.rs`，除非闸门本身回归失败。
