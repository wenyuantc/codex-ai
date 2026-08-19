# Component Guidelines

> How React components are written in this project.

---

## Patterns In Use

1. **Function components** with TypeScript props interfaces.
2. **Named exports** for business components (`export function TaskCard...`).
3. **Domain folders** under `src/components/<domain>/`.
4. **shadcn/ui primitives** from `src/components/ui/*` built on `@base-ui/react` + Tailwind.
5. **lucide-react** icons.
6. **Dialog-heavy flows** for create/edit/confirm rather than separate routes.
7. **ErrorBoundary** around fragile subtrees (see `TaskCard` wrapping risky dialogs).

## Typical Structure

```tsx
import { useState } from "react";
import { Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useTaskStore } from "@/stores/taskStore";
import { formatDate } from "@/lib/utils";
import type { Task } from "@/lib/types";

interface ExampleProps {
  task: Task;
  onClose?: () => void;
}

export function Example({ task, onClose }: ExampleProps) {
  const updateTask = useTaskStore((s) => s.updateTask);
  // local UI state only
  return (
    <div className="space-y-2">
      <p>{task.title}</p>
      <p className="text-xs text-muted-foreground">{formatDate(task.updated_at)}</p>
      <Button onClick={onClose}>关闭</Button>
    </div>
  );
}
```

Reference implementations:
- Card + actions: `src/components/tasks/TaskCard.tsx`
- Form dialog: `src/components/tasks/CreateTaskDialog.tsx`
- Layout shell: `src/components/layout/MainLayout.tsx`
- Primitive: `src/components/ui/button.tsx`, `src/components/ui/dialog.tsx`

## Props Conventions

- Define an explicit `interface XxxProps` above the component.
- Domain entities use shared types from `@/lib/types` (`Task`, `Project`, ...).
- Optional callbacks for parent orchestration (`onGitActionCompleted`, `onOpenLog`).
- Prefer passing ids + store lookups over deeply nested derived trees when the store already has the data.
- Keep props serializable/plain; do not pass SQLite handles or Tauri plugin instances.

## Styling

- Tailwind utility classes; merge with `cn()` from `@/lib/utils`.
- Prefer design tokens already used in the app: `bg-background`, `text-foreground`, `text-muted-foreground`, `border-border`, semantic status colors via helpers (`getStatusColor`, `getPriorityColor`).
- shadcn components use `data-slot="..."` attributes; keep that pattern when extending primitives.
- Do not introduce CSS modules, styled-components, or a second icon library.

## Composition

- Extract dialogs/sections when a page file grows large (Project/Sessions/Settings are already large; prefer new section components over more page bloat when touching them).
- Feature hooks live beside the feature (`src/components/tasks/hooks/*`) when they encapsulate multi-step actions.
- Use portals only when needed (e.g. drag overlay / menus in `TaskCard`).

## Live session input (`SessionInputBar`)

- Single shared bar: `src/components/sessions/SessionInputBar.tsx` — wire it under live terminals in TaskLog / SessionLog / EmployeeRunningSessions (and review panel if showing a live session). Do not fork per-host composers.
- Gate with `can(provider, "send_input")` + live session presence; fail-closed while capabilities are loading (no false "unsupported" flash).
- Copy lives in `sessions` i18n namespace (`zh-CN` + `en`); dispatch via `src/lib/{codex,claude,opencode,grok}.ts` send helpers — never invent a resume/new-session path from the bar.

## Task primary path (trust UX)

- **One primary CTA per task surface**: resolve with pure `resolveTaskPrimaryCta` in `src/lib/taskPrimaryCta.ts` — do not fork if/else tables in `TaskCard` vs `TaskDetailDialog`.
- Inputs must share automation/runtime sources: `getTaskAutomationDisplayState` + `getTaskActionRuntimeState` (never a second phase map in JSX).
- Detail dialog sticky bar: `TaskPrimaryActionBar`; secondary actions go in overflow, not competing primary color.
- **SSH trust**: global mode uses `SshTrustBanner` in `MainLayout`; session artifact limits still use `SshArtifactLimitedNotice` — keep both messages consistent (审查依据可能不完整).

## Accessibility Baseline

- Use semantic controls from shadcn (`Button`, `Dialog`, `Select`) rather than clickable `div`s.
- Banner/status regions use roles where already established (`SshTrustBanner` / SSH mode banner uses `role="status"`).
- Icon-only buttons should include accessible text (`aria-label` / visually hidden label) when adding new ones.
- Keep keyboard shortcuts out of text inputs; global shortcuts are coordinated via existing shortcut helpers (`src/lib/shortcuts.ts`).

## Localization / Copy

- User-facing strings are primarily Chinese.
- Status/priority/activity labels come from `src/lib/utils.ts` helpers — reuse them instead of hardcoding divergent wording.

## Anti-Patterns

- Raw `invoke("some_command")` inside JSX event handlers.
- Calling `Database.execute` / reintroducing frontend writes.
- Hand-formatting timestamps with `new Date(...).toLocaleString` instead of `formatDate`.
- Rebuilding Select/Dialog primitives instead of using `src/components/ui`.
- Giant inline forms that reimplement store mutation logic already on the store.
- Wrapping `onOpenChangeDetail` as `(change) => handler(change)` and dropping the optional `{line, message}` — review findings then open the file but never reveal/highlight. Forward `options` through. See [review-findings.md](../backend/review-findings.md).
