# Directory Structure

> How frontend code is organized in this repository.

---

## Layout

```text
src/
├── App.tsx                 # BrowserRouter + page routes + route persistence
├── main.tsx                # React root
├── index.css               # Tailwind / design tokens
├── pages/                  # Route-level screens (PascalCase *Page.tsx)
├── components/
│   ├── ui/                 # shadcn primitives (button, dialog, select, ...)
│   ├── layout/             # Shell: Sidebar, Header, MainLayout, notifications
│   ├── tasks/              # Domain UI + detail/ + hooks/
│   ├── projects/
│   ├── sessions/
│   ├── employees/
│   ├── dashboard/
│   ├── settings/           # Settings tabs
│   ├── git/
│   ├── search/
│   ├── codex/
│   ├── ai/
│   ├── trash/
│   ├── keyboard/
│   └── ErrorBoundary.tsx
├── stores/                 # Zustand domain stores
├── lib/                    # Types, IPC, SQL, utils, engine clients
├── hooks/                  # Cross-feature hooks only
└── assets/
```

## Where New Code Goes

| Kind of change | Put it here |
|----------------|-------------|
| New route screen | `src/pages/<Name>Page.tsx`, register in `src/App.tsx` |
| Domain widget / dialog | `src/components/<domain>/` (`ProjectFileRefPicker` for task project file refs; kanban `TaskCard` right-click **计划运行** is native-assignee-only) |
| Reusable primitive | Prefer `src/components/ui/` (shadcn style) before inventing a new base |
| Feature-only hook | `src/components/<domain>/hooks/useX.ts` |
| Cross-page hook | `src/hooks/useX.ts` |
| Shared type | `src/lib/types.ts` |
| Tauri command wrapper | `src/lib/backend.ts` (CRUD/shared, including `listAiChannels`) or engine client `src/lib/codex.ts` / `claude.ts` / `opencode.ts` / `grok.ts` / `native.ts` |
| Unified engine start/stop | `src/lib/aiEngine.ts` (`startByProvider` / `stopSessionByProvider` / `restartByProvider`) — unknown provider throws, never Codex |
| Settings subsection | `src/components/settings/<Name>SettingsTab.tsx` (`AiChannelsSettingsTab` for native channels, `SubagentsSettingsTab` for custom native sub-agents, `NotificationSettingsTab` for in-app alert sound) |
| Domain helper (non-UI) | `src/lib/<topic>.ts` (`projects.ts`, `taskPrompt.ts`, ...) |
| Global cache / mutations orchestration | `src/stores/<domain>Store.ts` |


## Naming

| Artifact | Convention | Example |
|----------|------------|---------|
| Pages / dialogs / cards | PascalCase | `CreateTaskDialog.tsx`, `KanbanPage.tsx` |
| Stores / libs / utils | camelCase file | `taskStore.ts`, `backend.ts` |
| Hooks | `use` + PascalCase | `useTaskExecutionActions.ts` |
| UI primitives | kebab-case file, named export component | `button.tsx` → `Button` |

## Module Boundaries

- **Pages** compose domain components and call stores. Avoid deep business SQL in pages.
- **Components** may call stores and `backend.ts`, but keep long orchestration in hooks or stores.
- **Stores** own fetch/mutate orchestration for a domain.
- **`lib/`** is framework-light: types, pure helpers, IPC wrappers, engine event helpers.
- **Do not** create parallel type systems outside `src/lib/types.ts` for domain entities.

## Route Map (current)

| Path | Page |
|------|------|
| `/` | `DashboardPage` |
| `/projects` | `ProjectsPage` |
| `/projects/:id` | `ProjectDetailPage` |
| `/kanban` | `KanbanPage` |
| `/sessions` | `SessionsPage` |
| `/employees` | `EmployeesPage` |
| `/settings` | `SettingsPage` |
| `/trash` | `TrashPage` |

Routes live under `MainLayout` and are restored from local storage via `RoutePersistence` in `src/App.tsx`.

## Anti-Patterns

- Dumping new feature files at `src/components/` root instead of a domain folder.
- Putting feature-only hooks in `src/hooks/` when only one domain uses them.
- Duplicating IPC wrappers next to a single dialog instead of `backend.ts`.
- Creating a new store for ephemeral dialog form state.
