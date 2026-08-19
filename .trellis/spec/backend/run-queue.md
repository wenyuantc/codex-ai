# Run Queue & Concurrency Gate

> Global session cap + persistent FIFO queue for **task execution** starts.

## 1. Scope / Trigger

Use this spec when changing `start_*`, session exit drain, `CodexSettings.max_concurrent_sessions`, `task_run_queue`, or frontend start side effects.

Module: `src-tauri/src/run_queue.rs`. Table: migration **v46** `task_run_queue`.

## 2. Signatures

```rust
// serde(tag = "status", rename_all = "snake_case")
enum StartSessionOutcome { Started, Queued { position: i64 } }

fn should_gate_task_run(task_id, resume_session_id, session_kind) -> bool
// true only when task_id is Some, resume is None, session_kind is None | "execution"

async fn list_task_run_queue(app) -> Result<Vec<TaskRunQueueItem>, String>
async fn cancel_queued_task_run(app, task_id: String) -> Result<bool, String>
// event: "task-run-queue-changed" (empty payload)
```

`TaskRunQueueItem`: `id`, `task_id`, `provider`, `employee_id`, `enqueued_at`, `position` (1-based).

Settings: local `CodexSettings.max_concurrent_sessions: i32` (default **3**, **0** = unlimited). Gate calls `load_codex_settings` (local only). Remote settings merge must **not** apply this field.

## 3. Contracts

- Count: four engine process tables + in-flight reservations (`RunQueueGate`).
- Enqueue: unique `task_id`; payload is `QueuedTaskRun` JSON (provider, employee, prompt, model, working_dir, git context, images). Resume is always `None` on replay.
- Enqueue does **not** set employee busy, task `in_progress`, or timer.
- Drain (`spawn_drain` after session exit / `spawn_resume` on app start): replay via `*_with_manager` (bypasses gate). Success → `in_progress` + busy + `start_task_timer_internal`. Failure → drop row + `task_run_dequeue_failed`.
- Activity: `task_run_queued` / `task_run_dequeued` / `task_run_queue_cancelled` / `task_run_dequeue_failed`.
- SSH executions use the same `start_*` path and the same local cap.

## 4. Validation & Error Matrix

| Condition | Behavior |
|---|---|
| Limit 0 / negative | No enqueue; proceed |
| Running + reserved >= limit, gated start | `Queued { position }` + emit |
| Review / fix / pipeline / resume / no task_id | Count toward cap; never enqueue |
| Cancel unknown `task_id` | `Ok(false)`, no log |
| Corrupt queue payload | Skip row and continue drain |
| Replay start fails | Row already claimed; log `task_run_dequeue_failed` |

## 5. Good / Base / Bad Cases

- **Good**: cap=1, second execution start returns `queued`; first exit starts the queued task and emits the event.
- **Base**: cap=3, three starts return `started`; fourth queues at position 1.
- **Bad**: frontend marks busy / `in_progress` / timer **before** reading outcome; SSH settings save overwrites the local cap.

## 6. Tests Required

- `run_queue.rs`: enqueue position, cancel, claim FIFO, skip bad payload, `should_gate_task_run`, `effective_session_limit` / `has_capacity`.
- Frontend: `resolveTaskPrimaryCta` queued lock; activity keys resolve via `activity:actions.*`.
- Gates: `cargo test run_queue`, `npm run test:ci`.

## 7. Wrong vs Correct

#### Wrong
`startTaskRunSession` sets employee busy and `updateTaskStatus(..., "in_progress")` then `invoke("start_*")`. Queued tasks look running and occupy the assignee.

#### Correct
Invoke first. On `{ status: "queued" }` only refresh `runQueue`. On `{ status: "started" }` then busy / `in_progress` / timer. If `runQueue` already has that `task_id`, return queued without worktree prep or a second invoke.

Frontend refresh: `task-run-queue-changed` must `fetchRunQueue` **and** `fetchTasks` — drain writes status in Rust; exit listeners alone miss the start.
