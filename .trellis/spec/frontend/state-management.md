# State Management

> Zustand is the only global client state system.

---

## State Categories

| Category | Tool | Examples |
|----------|------|----------|
| Domain cache | Zustand store | tasks, projects, employees, notifications, dashboard metrics |
| Server/DB truth | SQLite via Rust Tauri commands (reads + writes) | `tasks`, `projects`, `activity_logs` |
| Ephemeral UI | `useState` / dialog open flags | create dialog open, selected card ids |
| Environment preference | Zustand + `localStorage` | `environmentMode`, selected SSH config, last route, **`codex-ai:locale`** (via `src/lib/i18n/locale.ts`) |
| Live engine streams | Zustand + Tauri event listeners | Codex output buffers, session status |

There is **no** Redux, React Query, or SWR layer.

## Existing Stores

| Store | File | Owns |
|-------|------|------|
| Project | `src/stores/projectStore.ts` | projects, SSH configs, environment mode |
| Task | `src/stores/taskStore.ts` | tasks, subtasks, comments, attachments, automation states |
| Employee | `src/stores/employeeStore.ts` | employees, runtime/session listeners, output buffers |
| Dashboard | `src/stores/dashboardStore.ts` | dashboard aggregates + activity feed queries |
| Notification | `src/stores/notificationStore.ts` | sticky/system notifications |
| Log | `src/stores/logStore.ts` | lightweight log UI state |

## Standard Store Shape

Follow the existing pattern:

1. `create<StoreState>((set, get) => ({ ... }))`
2. Data arrays/maps + `loading` flags
3. `fetchX` methods that **read** via `@/lib/backend` command wrappers
4. Mutation methods that call `@/lib/backend` then refresh local cache (`fetchX` or surgical `set`)
5. Optional listener init methods returning an unsubscribe function

Example (task create path):
- UI → `useTaskStore.createTask`
- store → `createTaskCommand` in `backend.ts`
- backend → Rust `create_task`
- store → `fetchTasks` via `listTasks` command

## Read vs Write (Critical)

```text
READ  : Tauri commands only (list_*/get_*) via backend.ts
WRITE : Tauri commands only (capabilities block sql execute)
```

`src/lib/database.ts` is a hard-fail stub (`select` / `execute` / `getDb` throw).
Never reintroduce frontend SQL access even if a plugin permission looks convenient.

## Selector Usage

Prefer narrow selectors to avoid extra renders:

```ts
const fetchTasks = useTaskStore((s) => s.fetchTasks);
const environmentMode = useProjectStore((s) => s.environmentMode);
```

For imperative work inside effects/event handlers, `useXStore.getState()` is acceptable (see `MainLayout` notification open handler).

## Listener Lifecycle

- Initialize once from `MainLayout` effects.
- Listener methods must return cleanup that unsubscribes all Tauri listeners.
- On session exit events, stores refresh related collections (tasks/employees) rather than requiring every page to know about engine events.
- **All four engines** (`codex` / `claude` / `grok` / `opencode`) must drive the same task-list refresh path on exit. Codex-only listeners leave the kanban board stale after Claude/Grok/OpenCode sessions.
- Also refresh on `task-automation-state-changed` (automation phase + often task `status`). Exit events alone race automation write-back; the emit is the post-commit signal.
- Do **not** optimistically force `task.status` from `session_kind` (e.g. execution → `in_progress`, review → `review`). Session id fields may update locally; status is owned by backend commands / automation and must come from `fetchTasks` / `updateTaskStatus` / returned task rows.
- After manual **stop**, refresh `employeeRuntime` **and** `automationStates` (and tasks if status/timer fields change). Cards treat active automation phases (`waiting_execution`, `launching_fix`, …) as “running”; a stale phase shows permanent Loader UI even when the process is gone.
- `task-run-queue-changed`: `taskStore.initRunQueueListener` in `MainLayout` (same ref-count cleanup as session listeners). Handler must `fetchRunQueue` **and** `fetchTasks`. Drain starts the next session in Rust (`in_progress` + timer); refreshing only the queue leaves cards on the old kanban column. Do not derive `task.status` from a queue row.

## LocalStorage Persistence

### Convention: Guard browser-only storage with `typeof window`

**What**: Any exported helper that reads/writes `localStorage` must no-op (or return the documented default) when `typeof window === "undefined"`.

**Why**: Vitest runs in the node env — unguarded `window.localStorage` access crashes the pure-function tests that Quality Guidelines require for new lib logic. Canonical reference: `src/lib/i18n/locale.ts` (`getStoredLocale`); persisted keys use the `codex-ai:*` prefix (e.g. `codex-ai:locale`, `codex-ai:sessions-view-mode`).

**Example**:

```ts
export function getStoredSessionsViewMode(): SessionsViewMode {
  if (typeof window === "undefined") {
    return "card"; // documented default
  }
  return window.localStorage.getItem(SESSIONS_VIEW_MODE_STORAGE_KEY) === "table" ? "table" : "card";
}
```

Stored values must fail closed to the default when corrupted (only an exact known value opts out of the default).

## Derived / Display Helpers

Keep pure display derivation in `src/lib/utils.ts` or small lib modules, not duplicated inside multiple components:
- `getStatusLabel`, `getPriorityLabel`, `getActivityActionLabel`
- `formatDate`, `formatDuration`, `isTaskOverdue`
- automation/runtime display helpers used by kanban cards

Store-owned scoping/derivation logic belongs in **exported pure functions in the store module**, called by the action — not inlined inside the `async` action body. This is what makes it testable (see [Quality Guidelines](./quality-guidelines.md) → Testing Expectations):
- `filterTasksByVisibleProjects` (`taskStore.ts`) — project-scope filter shared by `fetchTasks` and `fetchTrashedTasks`
- `resolveSelectedSshConfigId` (`projectStore.ts`) — SSH host selection precedence
- `isInvalidDateRange` / `getKeywordMatchedActions` / `buildActivityScopeInput` (`dashboardStore.ts`) — activity feed filtering

When the same scoping rule is applied in two actions, extract once and call it twice; two inline copies of a `visibleProjectIds.has(...)` predicate is exactly how the active list and the trash list drift apart.

## Anti-Patterns

- Writing SQL `INSERT`/`UPDATE`/`DELETE` from stores.
- Fetching the same collection independently in every dialog instead of using the store.
- Storing full Monaco models or DOM nodes in Zustand.
- Cloning backend response types into a second store-specific interface without need.
