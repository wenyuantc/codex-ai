# Hook Guidelines

> When and how to write hooks in this codebase.

---

## Two Hook Homes

| Location | Use for | Examples |
|----------|---------|----------|
| `src/hooks/` | Cross-feature reusable behavior | `useAiOptimizePrompt.ts` |
| `src/components/<domain>/hooks/` | Feature orchestration used by one domain | `useTaskExecutionActions.ts`, `useTaskReviewActions.ts`, `useTaskAiActions.ts` |

There is no large shared hooks library. Prefer colocated feature hooks over premature global hooks.

## When To Create A Hook

Create a hook when one of these is true:
- Multiple components share the same multi-step action flow (start session, review, optimize prompt).
- A component needs listener setup/teardown that is not store-global.
- Dialog + async command + toast/error handling is too heavy to keep inline.

Do **not** create a hook for:
- A single `useState` pair.
- A thin one-line store selector.
- Logic that belongs on a Zustand store action (shared cache mutation).

## Local Patterns

### Feature action hooks

`src/components/tasks/hooks/useTaskExecutionActions.ts` and `useTaskReviewActions.ts` encapsulate:
- reading project/employee context
- calling `backend.ts` / engine helpers
- updating related store slices after success

Use the same style for new task-side multi-step actions.

### Cross-feature hooks

`src/hooks/useAiOptimizePrompt.ts` is shared by create/edit flows that need AI prompt optimization. Put new cross-page AI helpers here only if a second domain needs them.

### Store-owned listeners

Long-lived event listeners (Codex session/output/exit, notifications) are initialized from stores and wired in `MainLayout`:
- `useEmployeeStore.initCodexListeners`
- `useTaskStore.initCodexSessionListeners`
- `useNotificationStore.initNotificationListeners`

Do not re-subscribe the same global events inside random components.

## Naming

- Always start with `use`.
- Name after the capability, not the UI widget: `useTaskReviewActions`, not `useTaskCardStuff`.
- Return a stable object/functions map; keep side effects inside returned actions or setup effects.

## Anti-Patterns

- Subscribing to Tauri events in five components without cleanup.
- Putting SQL `select` queries in hooks when the store already owns that collection.
- Importing page-level components into hooks (direction should be page/component → hook, not reverse).
