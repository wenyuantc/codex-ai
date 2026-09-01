# Code Reuse Thinking Guide

> Search the existing Codex AI patterns before creating new abstractions.

---

## The Problem Here

This codebase already has concentrated “do it here” modules. Duplication usually means:
- a second date formatter
- a second status/priority label map
- a one-off `invoke` next to a dialog
- a new store method that reimplements another domain’s mutation flow
- copy-pasted SQL with slightly different `deleted_at` filters

---

## Search First

```bash
# symbols / labels / actions
rg -n "functionName|action_key|getStatusLabel" src src-tauri

# similar UI
rg -n "DialogTitle|createPortal|useSortable" src/components

# similar commands
rg -n "pub async fn create_|#\[tauri::command\]" src-tauri/src/app
```

Also open the closest existing feature and mirror it.

---

## Reuse Map (Prefer These)

| Need | Reuse |
|------|-------|
| Class name merge | `cn()` in `src/lib/utils.ts` |
| Date display | `formatDate`, `parseDateValue`, `getDateOnly` |
| Status/priority labels & colors | `getStatusLabel`, `getStatusColor`, `getPriorityLabel`, `getPriorityColor` |
| Activity Chinese labels | `getActivityActionLabel`, `getActivityDetailsLabel` |
| Domain types | `src/lib/types.ts` |
| IPC | `src/lib/backend.ts` wrappers |
| SQL read helper | `select()` in `src/lib/database.ts` |
| Project scope / env mode | `src/lib/projects.ts` + `projectStore` |
| Task execution/review actions | `src/components/tasks/hooks/*` |
| Coordinator plan generate + logs | `generateCoordinatorPlanForTask` / `generateAndPersistCoordinatorPlan` — never raw `aiGenerateCoordinatorTaskPlan` |
| UI primitives | `src/components/ui/*` |
| Select whose value is a machine key | `SelectValue` render fn → name (`KanbanPage`, `NativeChannelFields`, Settings 子 Agent 策略). Empty `<SelectValue />` shows `aggressive` / UUID |
| Icons | `lucide-react` only |
| Error UI shell | `ErrorBoundary` |
| IDs / timestamps / path validation (Rust) | `app/shared.rs` |
| Activity insert (Rust) | `insert_activity_log` |
| Soft-delete fetch style | `fetch_project_by_id`, `fetch_task_by_id` |
| SSH remote exec helpers | `app/remote.rs` |
| Working dir validation | `validate_runtime_working_dir` |
| Notifications | `notifications.rs` helpers |
| In-memory DB tests | `app/tests/mod.rs` `setup_test_pool` |

---

## Decision Table

| Question | If yes |
|----------|--------|
| Does a store already own this collection? | Add/extend store action, don’t fetch ad hoc in a leaf card |
| Does `backend.ts` already wrap the command? | Import it; don’t invoke raw |
| Is this a display-only derivation? | Put pure helper in `utils.ts` / domain lib |
| Is this multi-step task orchestration? | Prefer `components/tasks/hooks` |
| Is this a new shadcn-looking control? | Extend `components/ui` patterns |
| Is this a business invariant? | Pure Rust helper + unit test |
| Is this the 2nd copy of the same SQL? | Centralize in store or Rust query helper |

---

## Patterns Worth Copying (Not Rewriting)

1. **Mutation flow**: UI → store → backend wrapper → Rust command → `fetchX` refresh  
   Example: `taskStore.createTask`
2. **Listener flow**: store `init*Listeners` + cleanup, called from `MainLayout`
3. **Normalize at boundary**: `backend.ts` normalizers for SSH/health/session fields
4. **Explicit nullable updates**: Rust `deserialize_explicit_nullable`
5. **Compensating cleanup**: `create_task` attachments/SSH
6. **High-risk git confirm**: request/confirm/cancel commands
7. **Label registry growth**: append to existing maps instead of parallel dictionaries

---

## When Creating Something New Is OK

- New domain with no owner module yet (still place it using directory specs)
- Genuine new engine capability not covered by managers
- New migration for new persisted data
- New pure helper when no existing helper fits without distorting its meaning

Creating a new helper is fine; creating a second competing helper for the same concept is not.

---

## Anti-Patterns

- Copying `getStatusLabel` maps into a component.
- New `formatDateToLocal` helper.
- `invoke("update_task")` inside a dialog when `updateTask` wrapper exists.
- New CSS framework or icon pack.
- New global state library.
- Re-implementing soft delete as a boolean `is_deleted` column.

---

## Quick Self-Check

- [ ] I searched for an existing helper/command/store action
- [ ] I matched naming and snake_case entity fields
- [ ] I extended label/type unions in the canonical files
- [ ] I did not add a parallel abstraction for the same job
