# Frontend Development Guidelines

> React 19 + TypeScript + Vite + Tailwind CSS 4 + Zustand for Codex AI desktop UI.

---

## Stack Snapshot

| Piece | Local choice |
|-------|--------------|
| Framework | React 19 + React Router |
| Build | Vite (dev port `1420`), path alias `@/*` → `src/*` |
| Styling | Tailwind CSS 4 (`@tailwindcss/vite`), shadcn/ui (`base-nova`, CSS variables) |
| Icons | `lucide-react` |
| State | Zustand stores under `src/stores/` |
| IPC | `@tauri-apps/api` via thin wrappers in `src/lib/backend.ts` |
| SQL reads | `@tauri-apps/plugin-sql` select-only through `src/lib/database.ts` |
| Editor | Monaco for large text / diffs (`src/lib/monaco.ts`) |
| Tests | Vitest (`vitest.config.ts`, node env, pure functions only), colocated `*.test.ts` |

## Hard Rules

1. **Never write SQLite from the frontend.** `execute()` in `src/lib/database.ts` throws on purpose. Mutations go through Tauri commands.
2. **Do not call `invoke()` from components/stores.** Add or reuse a wrapper in `src/lib/backend.ts`.
3. **Domain types live in `src/lib/types.ts`.** Prefer snake_case fields that mirror Rust/SQLite payloads.
4. **Display dates with `formatDate()` from `src/lib/utils.ts`.** Do not hand-roll locale formatting.
5. **New user-visible activity actions need Chinese labels** in `getActivityActionLabel()`.
6. **Local and SSH modes are first-class.** UI that starts sessions, opens repos, or captures artifacts must respect `environmentMode` / `project_type`.

## Guidelines Index

| Guide | Description |
|-------|-------------|
| [Directory Structure](./directory-structure.md) | Folders, naming, where new files go |
| [Component Guidelines](./component-guidelines.md) | Components, props, composition, styling |
| [Hook Guidelines](./hook-guidelines.md) | Shared vs feature hooks |
| [State Management](./state-management.md) | Zustand stores and listener lifecycle |
| [Data Access](./data-access.md) | SQL select, backend IPC, normalization |
| [Type Safety](./type-safety.md) | Types, unions, nullable update payloads |
| [Quality Guidelines](./quality-guidelines.md) | Build checks, a11y, ErrorBoundary, smoke |

## Primary References

- Routes: `src/App.tsx`
- Shell: `src/components/layout/MainLayout.tsx`
- IPC bridge: `src/lib/backend.ts`
- Types: `src/lib/types.ts`
- Utils/labels: `src/lib/utils.ts`
- Stores: `src/stores/*Store.ts`

## Language

Spec content is English. Product UI copy in the app is primarily Chinese.
