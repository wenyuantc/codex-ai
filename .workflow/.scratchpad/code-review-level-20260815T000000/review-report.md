# Code Review Report

## Overview

| Item | Value |
|------|-------|
| Skill | code-review-level |
| Level | **max** |
| Scope mode | branch_diff (`HEAD~10..HEAD`，最近 10 次提交) |
| Files | 46 个优先源码/契约文件（全量 82，含 Trellis 文档/日志） |
| Languages | rust, typescript, javascript |
| Total findings | 18 |
| Critical | 0 |
| High | 8 |
| Medium | 8 |
| Low | 2 |
| Info | 0 |
| Generated | 2026-08-15T08:40:00Z |

## Level Execution Summary

| Item | Value |
|------|-------|
| Level | **max** |
| Depth | exhaustive |
| Dimensions | correctness, readability, performance, security, testing, architecture |
| Files reviewed | 46 |
| Truncated | no |
| Agents | Explore ×2（依赖链 / 安全）、general-purpose ×2（对抗 / 架构） |
| Cross-file | yes |
| Adversarial | yes |

### Covered

- 六维全开
- 跨文件调用链：token 解析 → 落库 → 仪表盘/任务花费；闸门 acquire/release → 四引擎 start → drain replay
- 对抗场景：崩溃丢队、并发 drain、软删后回放、SSH payload 过期、前端把 Queued 当 Started
- 架构：共享内核 vs 四引擎复制、IPC 契约、活动日志中文 key、设置分层

### Not covered at this level

- 仅受文件上限约束（本次未截断）
- 未在桌面端做手工冒烟（`npm run tauri:dev`）；闸门竞态为代码审阅，非动态复现
- Trellis 规划/journal 提交只作背景，不作为缺陷源

## 审查范围（最近 10 次提交）

| Hash | Subject | 性质 |
|------|---------|------|
| `c0acc3c` | feat(sessions): 四引擎 token 用量解析落库与任务/仪表盘成本展示 | 代码 |
| `7d48973` | feat(sessions): 终端日志一键复制与导出为文件 | 代码 |
| `429f772` | test(sessions): 补齐 token 用量解析与 NULL 累加断言 | 测试 |
| `bb01242` | docs(spec): 记录会话 token 用量契约与空值语义 | 文档 |
| `1828409` / `7c63fde` | archive token-usage + journal | 杂务 |
| `8dbd82a` | feat(engine): 增加全局并发闸门与持久化运行队列 | 代码 |
| `564c89e` | docs(task): 产品缺口下一波 Trellis 规划 | 规划 |
| `45c2d80` / `40befb3` | archive log-export + journal | 杂务 |

**统计**：82 files, +3232 / −477。真正改变运行时行为的是 **token 用量**、**运行队列（默认上限 3）**、**终端复制导出** 三块。

## Risk Hotspots

- `src-tauri/src/run_queue.rs` — concurrency, data, config
- `src-tauri/src/engine/usage.rs` — data, 空值语义
- `src-tauri/src/app/database.rs` — SQL, 仪表盘聚合
- `src-tauri/src/app/sessions.rs` — 会话落库 / 日志导出
- `src-tauri/src/db/migrations.rs` — v45/v46
- 四引擎 `process/{mod,session_runtime,stream}.rs` — 闸门接线与用量事件
- `src/lib/taskRunSession.ts` / `src/components/codex/CodexTerminal.tsx` — 前端契约

## Files Reviewed

优先源码见 `context.json`。重点精读：`run_queue.rs`、`engine/usage.rs`、`app/{database,sessions}.rs`、`db/{migrations,models}.rs`、四引擎 start/stream、`taskRunSession.ts`、`CodexTerminal.tsx`、`DashboardPage.tsx`、`backend.ts`、`activity.json`。

## High (8)

### F-001 [High] 前端丢弃 StartSessionOutcome，排队被当成已启动

- **Dimension**: correctness
- **Location**: `src/lib/taskRunSession.ts`:65
- **Evidence**: Rust `start_*` 已返回 `Started | Queued{position}`，四个 TS wrapper 仍是 `Promise<void>`。`startTaskRunSession` 在 invoke 前把员工设为 `busy`、任务设为 `in_progress`，返回后无条件 `startTaskTimer` + `refreshEmployeeRuntimeStatus`。排队时没有存活进程，`deriveEmployeeRuntimeStatus` 会把 `busy && !running` 改成 `offline`。
- **Suggestion**: TS 贯通 `StartSessionOutcome`。仅 `Started` 时才 busy / in_progress / startTimer；`Queued` 时展示位次并订阅 `task-run-queue-changed`。
- **Source**: explore

### F-002 [High] 队列命令未接线，停止/删除也无法取消排队

- **Dimension**: correctness
- **Location**: `src-tauri/src/lib.rs`:300
- **Evidence**: `list_task_run_queue` / `cancel_queued_task_run` 已注册，`src/` 零引用。`stopTask` 在 `!runningSession` 时直接 return。`delete_task` / `permanently_delete_task` 从不清理 `task_run_queue`。用户点停止或丢进回收站后，drain 仍会拉起任务。
- **Suggestion**: `backend.ts` 封装 list/cancel；stop / 归档 / 软删 / 永久删统一 `remove_queued_task_run`；排队态 CTA 改为「取消排队」。
- **Source**: explore

### F-003 [High] claim_next_task_run 非原子，并发 drain 可重复认领

- **Dimension**: correctness
- **Location**: `src-tauri/src/run_queue.rs`:409
- **Evidence**: `SELECT ... LIMIT 1` 再 `DELETE WHERE id=?`，不检查 `rows_affected`。每个会话退出都会 `spawn_drain`，无单飞。两个 drain 可读到同一行并各自 `replay`。`「已在运行」` 检查与进程注册之间同样无锁。
- **Suggestion**: `DELETE ... RETURNING` 或 `BEGIN IMMEDIATE` + `rows_affected==1`；全局只允许一个 drain worker；补并发测试。
- **Source**: adversarial

### F-004 [High] 先删队列再启动，崩溃或 replay 失败会永久丢队

- **Dimension**: correctness
- **Location**: `src-tauri/src/run_queue.rs`:213
- **Evidence**: drain 先 claim（删除行）再 `replay_task_run`。失败只写 `task_run_dequeue_failed`，不回队。进程在 DELETE 之后、start 成功之前被杀，任务从队列蒸发。replay 还会提前把员工标 busy、任务标 in_progress。
- **Suggestion**: `queued → claimed` 状态机：claim 只改 status/lease，启动成功再删行；失败或超时回 queued，并回滚状态。
- **Source**: adversarial

### F-005 [High] 有余量时 Proceed 不删除已有队列行，重试后会被再跑一遍

- **Dimension**: correctness
- **Location**: `src-tauri/src/run_queue.rs`:150
- **Evidence**: `has_capacity` 为真时直接 `Proceed`，从不 `DELETE` 该 `task_id` 的 queued 行。任务已在队列中时用户再点执行（前端 `isRunning=false`），会一边直接 start，一边留下队列行。会话结束后 `spawn_drain` 用旧 payload 再启动一次。
- **Suggestion**: Proceed 前按 `task_id` 作废队列行；已 queued 的重复 start 应返回现有 position；前端在 queued 态禁用 Run。
- **Source**: adversarial

### F-006 [High] 软删/永久删除任务不取消排队，已删任务仍会被 drain 拉起

- **Dimension**: correctness
- **Location**: `src-tauri/src/app/tasks.rs`:1686
- **Evidence**: `delete_task` / `permanently_delete_task` 都不调用 `remove_queued_task_run`。`replay_task_run` 在 `fetch_task_by_id`（含 `deleted_at IS NULL`）失败后只跳过改状态，仍把员工标 busy 并 `start_*_with_manager`。
- **Suggestion**: 删除/归档时同步删队列行；replay 前若任务不可运行则丢弃 claim。
- **Source**: adversarial

### F-007 [High] 启动失败释放预约后不 drain，队列会饿死

- **Dimension**: correctness
- **Location**: `src-tauri/src/codex/process/mod.rs`:687
- **Evidence**: 四引擎 `start_*` 在 `with_manager` 返回后 `drop(reservation)`，失败路径不 `spawn_drain`。limit=2、1 个在跑、1 个 start 失败时，reserved 归零，队首要等到下一次会话退出或重启。Codex 停止超时摘槽同样不 drain。
- **Suggestion**: 释放预约且队列非空时（start 失败、超时摘槽、清僵尸）统一 `spawn_drain`。
- **Source**: explore

### F-008 [High] 默认并发上限 3 已生效，但设置页/类型/看板均未接入

- **Dimension**: architecture
- **Location**: `src-tauri/src/codex/settings.rs`:31
- **Evidence**: `DEFAULT_MAX_CONCURRENT_SESSIONS = 3`，旧配置缺字段会被 normalize 成 3。闸门对所有任务执行立即生效。前端 `CodexSettings` 没有该字段，SettingsPage 无控件，看板无「排队中」。用户无法把上限调成 0（不限制）。与 B1 PRD（设置页可配、看板可见、可取消、可批量入队）差距很大。
- **Suggestion**: 补齐 app 级设置与看板 UI；或在 UI 未就绪前默认 0，避免半截功能改变线上行为。
- **Source**: architecture

## Medium (8)

### F-009 [Medium] 闸门用原始 session_kind 判断，normalize 后仍按 execution 启动

- **Dimension**: security
- **Location**: `src-tauri/src/run_queue.rs`:80
- **Evidence**: `resume_session_id` 非空或 `session_kind` 不是 `None`/`execution` 即跳过闸门。`normalize_session_kind` 把除 `review` 外的任意值归一成 Execution。本机桌面 IPC，主要是闸门完整性。
- **Suggestion**: 闸门应对 normalize 之后的 kind 判断；未知 kind 拒绝或强制入闸；resume 必须校验会话归属并计入额度。
- **Source**: security-pass

### F-010 [Medium] 运行队列活动日志缺少仪表盘中文 key

- **Dimension**: architecture
- **Location**: `src/locales/zh-CN/activity.json`:152
- **Evidence**: 写入了 `task_run_queued` / `task_run_dequeued` / `task_run_dequeue_failed` / `task_run_queue_cancelled`，中英文 `activity.json` 都没有这些 key。仪表盘会显示原始蛇形 key。
- **Suggestion**: 补齐中英文案，并在 locale/utils 测试加断言。
- **Source**: primary

### F-011 [Medium] 并发闸门复制进四引擎 start 包装，未收口到共享内核

- **Dimension**: architecture
- **Location**: `src-tauri/src/codex/process/mod.rs`:646
- **Evidence**: spec/PRD 要求闸门挂 `engine/`。实际四个 `start_*` 各复制约 40 行；`count_running_sessions` 分别锁四种 Manager；`replay_task_run` 是 crate 根的四引擎 match。
- **Suggestion**: 计数走 `EngineProcessRegistry`；start_* 只调一次共享 helper。
- **Source**: architecture

### F-012 [Medium] 非法 token 被打成 0，破坏「未知保持 NULL」契约

- **Dimension**: correctness
- **Location**: `src-tauri/src/engine/usage.rs`:53
- **Evidence**: 注释写 unknown 永不打成 0。`usage_u64` 却把负 i64 和 NaN/负 float 映射为 `Some(0)`，随后 `apply` 把 NULL 列写成 0，会话进入 `sessions_with_usage`。
- **Suggestion**: 非法值返回 `None`；单测覆盖负值/NaN/缺字段。
- **Source**: primary

### F-013 [Medium] 缺 total 时用 input+output 合成后再按列累加，合计会不一致

- **Dimension**: correctness
- **Location**: `src-tauri/src/engine/usage.rs`:92
- **Evidence**: 先到 `output=7`（total 仍 NULL），再到 `in=10/out=20/total=30`，会变成 `input=10、output=27、total=30`。仪表盘 `SUM(total_tokens)` 与 input+output 对不上。累计快照会被再加一次。
- **Suggestion**: 区分 snapshot 与 delta；不要把合成 total 再独立累加。
- **Source**: adversarial

### F-014 [Medium] SSH/全局仪表盘 token 会串入无项目 ad-hoc 会话

- **Dimension**: correctness
- **Location**: `src-tauri/src/app/database.rs`:2214
- **Evidence**: `include_null_project_sessions = project_id.is_none()`。SSH 按主机筛选时仍 `OR project_id IS NULL`，本地/其他主机 ad-hoc 会话会算进当前主机报表。
- **Suggestion**: 按 `execution_target` / `ssh_config_id` 过滤；仅真正的全局视图才计入 NULL project。
- **Source**: explore

### F-015 [Medium] 队列并发认领、失败回队、前端 Queued 分支缺少测试

- **Dimension**: testing
- **Location**: `src-tauri/src/run_queue.rs`:519
- **Evidence**: 现有测试只覆盖 FIFO、去重、损坏 payload、容量算术。token 侧的 NULL 累加与仪表盘分引擎聚合覆盖较好。
- **Suggestion**: 补并发 drain、软删跳过、start 失败回队、TS 侧 Queued vs Started。
- **Source**: primary

### F-016 [Medium] 排队 payload 在长时间等待后会过期，失败即丢任务

- **Dimension**: correctness
- **Location**: `src-tauri/src/run_queue.rs`:49
- **Evidence**: payload 只存 `working_dir` / `image_paths` / `task_git_context_id`。排队期间图片、SSH、worktree 都可能失效。失败即删行。`ON CONFLICT DO NOTHING` 还会锁死旧 prompt。
- **Suggestion**: replay 时按当前任务重解析路径；失败回队；重复入队更新 payload 或拒绝。
- **Source**: adversarial

## Low (2)

### F-017 [Low] 终端复制失败无用户反馈，导出立刻 revoke URL

- **Dimension**: readability
- **Location**: `src/components/codex/CodexTerminal.tsx`:96
- **Evidence**: catch 只有 `console.error`。`click()` 后立刻 `revokeObjectURL`。过滤态导出的是可见子集。`get_codex_session_log_lines` LIMIT 2000 无提示。任务详情主终端（`TaskExecutionPanel`）没有复制/导出。
- **Suggestion**: toast + 延后 revoke；过滤态标明范围；超 2000 行警告。
- **Source**: primary

### F-018 [Low] token 趋势按桶循环查库

- **Dimension**: performance
- **Location**: `src-tauri/src/app/database.rs`:1988
- **Evidence**: 30d 就是 30 次独立 `SUM`。完成数趋势是同一模式，token 又复制了一套。
- **Suggestion**: 一次 `GROUP BY` 取回再在内存对齐坐标轴。
- **Source**: primary

## 做得好的地方

- **token 空值语义总体正确**：列可空、未知不假装 0、`sessions_with_usage` 与 empty UI 对齐；`apply_codex_session_usage` 有明确 SQL 契约和单测。
- **共享解析器**：`engine/usage.rs` 收口四引擎 JSON 形态，stream 仍按引擎协议分治，符合 ai-engines spec。
- **用量与活动日志分离**：token 不写 activity（避免刷屏），spec 已写明。
- **闸门范围有意识**：只拦 fresh execution，review/pipeline 走 `*_with_manager` 以免打断状态机——理由写在模块头，虽然与 B1 PRD「内部会话同样受闸」不完全一致。
- **队列 CRUD 单测扎实**：FIFO、task_id 去重、损坏 payload 跳过都有覆盖。
- **迁移版本连续**：v45/v46，`latest_migration_version==46`，`migration_versions_are_contiguous` 仍成立。

## Recommendations (Priority Order)

1. **先补前端契约（F-001 / F-002 / F-008）** — 在 UI 未就绪时把默认并发改回 0，或立刻接上 Queued 态 / 设置项 / 取消排队，否则默认上限 3 会在用户不知情时改变执行行为。
2. **把 claim 做成原子操作并加单飞 drain（F-003 / F-004 / F-005）** — 这是队列正确性的根。
3. **删除/归档/停止必须清队列（F-006）** — 否则回收站任务会被半夜拉起。
4. **失败路径统一 spawn_drain（F-007）** — 避免队列饿死。
5. **补活动中文 key（F-010）和非法 token→None（F-012）** — 成本低、契约清晰。
6. **再收敛架构（F-011）** — 闸门进 `engine/`，避免四引擎再复制一份。

## Merge Gate Hint

**CAUTION**：无 Critical，但有 **8 条 High**。默认并发上限 3 已经生效，而看板/设置/停止/删除都还不知道队列的存在。建议修完 F-001～F-008 再合入发布分支；若必须先合，至少把 `DEFAULT_MAX_CONCURRENT_SESSIONS` 改为 `0`（不限制），把闸门变成暗开关。

## Artifacts

- workDir: `.workflow/.scratchpad/code-review-level-20260815T000000`
- `level-scope.json` / `context.json` / `findings.json` / `completion.json`
- Agent 来源：依赖链 Explore、安全 Explore、对抗 general-purpose、架构 general-purpose
