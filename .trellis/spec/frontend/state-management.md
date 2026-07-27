# State Management

> Zustand is the only global client state system.

---

## State Categories

| Category | Tool | Examples |
|----------|------|----------|
| Domain cache | Zustand store | tasks, projects, employees, notifications, dashboard metrics |
| Server/DB truth | SQLite via Rust commands (writes) + select (reads) | `tasks`, `projects`, `activity_logs` |
| Ephemeral UI | `useState` / dialog open flags | create dialog open, selected card ids |
| Environment preference | Zustand + `localStorage` | `environmentMode`, selected SSH config, last route |
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
3. `fetchX` methods that **read** via `select()` from `@/lib/database` and/or command wrappers
4. Mutation methods that call `@/lib/backend` then refresh local cache (`fetchX` or surgical `set`)
5. Optional listener init methods returning an unsubscribe function

Example (task create path):
- UI → `useTaskStore.createTask`
- store → `createTaskCommand` in `backend.ts`
- backend → Rust `create_task`
- store → `fetchTasks` with select SQL

## Read vs Write Split (Critical)

```text
READ  : select() SQL in stores (allowed by capabilities sql:allow-select)
WRITE : Tauri commands only (capabilities block sql execute)
```

`src/lib/database.ts`:
- `select()` — allowed
- `execute()` — always throws with a Chinese error directing developers to commands

Never reintroduce frontend writes even if a plugin permission looks convenient.

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

## Derived / Display Helpers

Keep pure display derivation in `src/lib/utils.ts` or small lib modules, not duplicated inside multiple components:
- `getStatusLabel`, `getPriorityLabel`, `getActivityActionLabel`
- `formatDate`, `formatDuration`, `isTaskOverdue`
- automation/runtime display helpers used by kanban cards

## Anti-Patterns

- Writing SQL `INSERT`/`UPDATE`/`DELETE` from stores.
- Fetching the same collection independently in every dialog instead of using the store.
- Storing full Monaco models or DOM nodes in Zustand.
- Cloning backend response types into a second store-specific interface without need.
