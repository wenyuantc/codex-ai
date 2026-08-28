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
- Copy lives in `sessions` i18n namespace (`zh-CN` + `en`); dispatch via `src/lib/{codex,claude,opencode,grok,native}.ts` send helpers — never invent a resume/new-session path from the bar. Unknown provider throws; `native` finish is `finish_native_input` (stop). Do not call it from `useSessionLogAwaitFollowups`.

## Task primary path (trust UX)

- **One primary CTA per task surface**: resolve with pure `resolveTaskPrimaryCta` in `src/lib/taskPrimaryCta.ts` — do not fork if/else tables in `TaskCard` vs `TaskDetailDialog`.
- Inputs must share automation/runtime sources: `getTaskAutomationDisplayState` + `getTaskActionRuntimeState` (never a second phase map in JSX).
- **Auto QC badge ≠ task running.** The kanban/detail badge is `task_automation_state.phase`, not `tasks.status` or a live session. First-run execution leaves phase `idle` until the session exits and review starts. Overlay copy only: `getTaskAutomationStatusLabel(status, { executionRunning })` when `status === "idle"` and `executionActions.isRunning` (real `session_kind === "execution"`). zh-CN `执行中` / en `Executing`. Do not write a new phase, do not reuse `waiting_execution` (that means auto-fix), do not pass `runtimeState.executionActive` (includes pipeline / auto-fix). CTA, restart, and archival still treat `idle` as idle.
- Detail dialog sticky bar: `TaskPrimaryActionBar`; secondary actions go in overflow, not competing primary color.
- Detail right rail (`TaskPropertiesSidebar`): always show the 用量 group below 耗时 (unknown copy when no session reported usage). Do not leave task-level token totals only on the Execution tab. Field order is 输入 / 输出 / 缓存 / 总用量 / 缓存率 via `TokenUsageBreakdown`.
- Execution chain (`TaskSessionChainPanel`): each session card shows the same five metrics from `listCodexSessions` token fields. Missing values stay 未知; do not fake 0. Reuse `buildTokenUsageMetrics` / `TokenUsageBreakdown` — do not fork a third formatter.
- Sessions page (`SessionCard` + table): same five metrics, including cache tokens and cache rate. Compact `formatSessionTokenUsage` must include cache + rate, not only in/out/total.
- **SSH trust**: global mode uses `SshTrustBanner` in `MainLayout`; session artifact limits still use `SshArtifactLimitedNotice` — keep both messages consistent (审查依据可能不完整).

## Accessibility Baseline

- Use semantic controls from shadcn (`Button`, `Dialog`, `Select`) rather than clickable `div`s.
- Banner/status regions use roles where already established (`SshTrustBanner` / SSH mode banner uses `role="status"`).
- Icon-only buttons should include accessible text (`aria-label` / visually hidden label) when adding new ones.
- Keep keyboard shortcuts out of text inputs; global shortcuts are coordinated via existing shortcut helpers (`src/lib/shortcuts.ts`).

## Localization / Copy

- User-facing strings are primarily Chinese.
- Status/priority/activity labels come from `src/lib/utils.ts` helpers — reuse them instead of hardcoding divergent wording.

## Scenario: Select trigger shows name, not stored key

### 1. Scope / Trigger

`src/components/ui/select.tsx` is **Base UI** (`@base-ui/react/select`), not Radix. `SelectItem` children go to `ItemText` (dropdown list only). The **closed trigger** is `SelectValue`. An empty `<SelectValue />` renders the stored `value` string.

Trigger whenever `Select` `value` is a machine key that is not what the user should read: `aggressive`, `conservative`, `high`, UUID, protocol id, `FILTER_ALL`, etc.

Evidence: Settings 界面与运行「子 Agent 策略」— list showed 保守/均衡/积极, trigger showed `aggressive`. Same contract as `KanbanPage` filters and `NativeChannelFields`.

### 2. Signatures

| Piece | Contract |
|-------|----------|
| Stored `value` | Machine key persisted / sent to Rust (`aggressive`, channel id, …) |
| `SelectItem value` | That same key |
| `SelectItem` children | Display **name** (`t(...)`, `getPriorityLabel`, `channel.name`) |
| `SelectValue` children | `(value) => name` render function — **required** when key ≠ name |
| Locales | zh-CN + en for every visible name |

### 3. Contracts

- Dropdown **and** closed trigger show the name in the active locale.
- Changing locale remaps the trigger through `t` / label helpers; do not snapshot Chinese into state.
- Keep storing the key. Do not persist 积极 / Aggressive as `subagent_policy`.
- If `value` is not a string (placeholder / empty), return a placeholder name, not `String(value)`.

### 4. Validation & Error Matrix

| Case | Behavior |
|------|----------|
| key ≠ display name, empty `<SelectValue />` | Trigger shows `aggressive` / UUID — **invalid** |
| key ≠ display name, `SelectValue` maps via `t` / helper | Trigger shows 积极 / Aggressive |
| key already equals name (rare model ids) | Empty `SelectValue` is acceptable only if users should see that id |
| Unknown key | Fallback name or placeholder, never dump the raw key if a label exists |

### 5. Good / Base / Bad Cases

- Good: `nativeSubagentPolicy` stays `"aggressive"`; trigger uses `t("settings:nativeAgent.subagentPolicyAggressive")` → 积极
- Good: kanban priority filter `value="high"` + `getPriorityLabel` in `SelectValue`
- Base: model id select where the visible text is the id
- Bad: `<SelectValue />` for `subagent_policy` / status / channel id / priority

### 6. Tests Required

- i18n: both locales have the name keys (`locale.test.ts` pattern for `subagentPolicyBalanced`).
- No component test required unless adding a pure mapper; then assert `aggressive` → 积极 and not `"aggressive"`.

### 7. Wrong vs Correct

#### Wrong

```tsx
<SelectTrigger>
  <SelectValue />
</SelectTrigger>
<SelectContent>
  <SelectItem value="aggressive">{t("settings:nativeAgent.subagentPolicyAggressive")}</SelectItem>
</SelectContent>
```

List is localized; closed trigger still shows `aggressive`.

#### Correct

```tsx
<SelectTrigger>
  <SelectValue>
    {(value) => {
      if (value === "conservative") return t("settings:nativeAgent.subagentPolicyConservative");
      if (value === "aggressive") return t("settings:nativeAgent.subagentPolicyAggressive");
      return t("settings:nativeAgent.subagentPolicyBalanced");
    }}
  </SelectValue>
</SelectTrigger>
```

References: `src/pages/KanbanPage.tsx`, `src/components/employees/NativeChannelFields.tsx`, `src/components/settings/RuntimeSettingsTab.tsx` (子 Agent 策略).

## Anti-Patterns

- Raw `invoke("some_command")` inside JSX event handlers.
- Calling `Database.execute` / reintroducing frontend writes.
- Hand-formatting timestamps with `new Date(...).toLocaleString` instead of `formatDate`.
- Rebuilding Select/Dialog primitives instead of using `src/components/ui`.
- Empty `<SelectValue />` when `value` is a machine key (`aggressive`, UUID, `high`) — closed trigger shows the key. Map to a name; see **Select trigger shows name, not stored key** above.
- Giant inline forms that reimplement store mutation logic already on the store.
- Wrapping `onOpenChangeDetail` as `(change) => handler(change)` and dropping the optional `{line, message}` — review findings then open the file but never reveal/highlight. Forward `options` through. See [review-findings.md](../backend/review-findings.md).
