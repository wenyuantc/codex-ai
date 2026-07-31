# Implement: 看板状态与停止按钮 UI 同步

## Checklist

### 1. Backend — 补齐 automation emit

- [ ] `session_exit.rs`：人工停止分支（execution / review 的 `has_stopping_requested`）在写 `manual_control` 后 `emit_task_automation_state_changed(..., PHASE_MANUAL_CONTROL)`
- [ ] `fix_loop.rs` / `retry_pending_review`（及对称的 fix 启动成功路径）：`finalize_launched_action` 成功后 emit 对应 phase（`waiting_review` / `waiting_execution`）
- [ ] 检查 `reserve_pending_action` → launch 失败路径是否需 emit（`review_launch_failed` / `fix_launch_failed`）以便 UI 退出假 running
- [ ] 可选：`update_task_status_internal` 成功后 emit（若 checklist 前几项手工验证仍有漏刷再做）
- [ ] 在 `task_automation` 相关测试中增加“关键路径会 emit”的断言（若现有测试 harness 可 mock `AppHandle::emit`；否则至少保证编译 + 手工步骤）

### 2. Frontend — taskStore 监听

- [ ] 扩展 `initCodexSessionListeners`：订阅 Claude/Grok/OpenCode exit，与 Codex 相同触发 `fetchTaskAutomationState` + `fetchTasks`
- [ ] 评估 `setTaskLastSessionId`：去掉或收窄对 `status` 的乐观覆盖，避免把已是 `review` 的任务打回 `in_progress`
- [ ] 确认 `onTaskAutomationStateChanged` 仍 `fetchTaskAutomationState` + `fetchTasks`（已有则保持）
- [ ] 若 exit 与 emit 之间仍有竞态，对同一 `task_id` 做 300–500ms 去抖二次 `fetchTasks`（仅 fallback）

### 3. Frontend — stop 路径

- [ ] `useTaskExecutionActions.stopTask`：停止成功后 `fetchTaskAutomationState(task.id)`；必要时刷新任务计时字段
- [ ] 确认 `loading` 在 `finally` 清理；不依赖 loading 表示“运行中”

### 4. 验证

- [ ] 手工：Codex 自动质控 进行中→审核，看板列即时变化
- [ ] 手工：Claude 同上（若环境有 Claude）
- [ ] 手工：进行中停止 → 按钮恢复，无永久 Loader2
- [ ] 手工：拖拽改列仍正常
- [ ] `npm run build`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`（或聚焦 task_automation 模块测试）

## Risky files

| 文件 | 风险 |
|------|------|
| `src-tauri/src/task_automation/session_exit.rs` | 漏 emit / 重复 emit |
| `src-tauri/src/task_automation/fix_loop.rs` | 启动成功路径遗漏 |
| `src/stores/taskStore.ts` | 多引擎监听泄漏、过度 fetch |
| `src/components/tasks/hooks/useTaskExecutionActions.ts` | stop 失败路径状态 |

## Validation commands

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml task_automation
# 或全量：
cargo test --manifest-path src-tauri/Cargo.toml
```

## Rollback points

1. 仅 backend emit 补丁可独立回退
2. 仅 frontend listener 可独立回退
3. 去掉 status 乐观更新若引起回归，可只保留 session id 写入、status 仍走 fetch
