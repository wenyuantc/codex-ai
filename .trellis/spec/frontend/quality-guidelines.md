# Quality Guidelines

> Practical quality bar for frontend changes in this repo.

---

## Verification Commands

Minimum checks before considering a frontend change done:

```bash
# Lint + format (CI enforces these on PR/push to main)
npm run lint
npm run format:check

# Unit tests (CI enforces this on PR/push to main)
npm run test:ci

# Typecheck + production bundle
npm run build

# Frontend-only dev server
npm run dev

# Full desktop app smoke
npm run tauri:dev
```

For backend-impacting UI flows, also run:

```bash
npm run lint:rust
cargo test --manifest-path src-tauri/Cargo.toml
```

Tooling notes:
- ESLint flat config: `eslint.config.js` (scope: `src/**`, `vite.config.ts`, `vitest.config.ts`)
- Vitest config: `vitest.config.ts` (separate from `vite.config.ts` on purpose — the production build config stays untouched)
- Prettier: `.prettierrc` / `.prettierignore`
- React Hooks: enforce `rules-of-hooks`; `exhaustive-deps` is warn-only to avoid cascading false positives on existing effects
- CI: `.github/workflows/lint.yml`

## Required Product Behaviors

When a UI change introduces or touches these areas, verify:

| Area | Expectation |
|------|-------------|
| Dates | Render through `formatDate()` |
| Activity feed | New actions have Chinese labels in `getActivityActionLabel` |
| SSH mode | Banner/limits understood; no “local only” assumptions for execution |
| Soft delete | Lists hide `deleted_at` rows; trash pages use trash APIs |
| Monaco | Large text edit/preview uses Monaco helpers, not a random textarea, when the feature already does |
| Mutations | Go through stores/`backend.ts`, never SQL execute |

These come from project docs (`AGENTS.md`) and current code.

## Error Handling UX

- Store fetch failures: `console.error` + safe fallback state (existing pattern).
- Command failures: surface to the user via dialog text / inline error / notification depending on the surrounding flow.
- Render crashes: wrap risky subtrees with `ErrorBoundary` (`src/components/ErrorBoundary.tsx`) showing Chinese fallback copy + stack.
- Do not empty-catch. If an error is optional telemetry (e.g. activity log for mode switch), log it and continue only when failure is non-critical.

## Accessibility & Safety

- Prefer shadcn controls for focus management in dialogs.
- Destructive actions use confirmation dialogs (delete task, git high-risk actions, permanent delete).
- Git high-risk operations must keep the request/confirm/cancel flow already modeled in backend + UI.
- Do not auto-run merge/push/rebase from UI without the existing confirmation path.

## Performance Habits

- Use store selectors narrowly.
- Avoid refetching entire collections on every keystroke; debounce search UIs (global search pattern).
- Keep heavy Monaco editors mounted only while needed.
- Listener init belongs in layout/store once, not per card.
- **Never put `setInterval` on list-item components** for clocks/elapsed time. Use the shared second clock (`src/hooks/useSharedNow.ts`) so the app keeps at most one interval; isolate live labels in a small memoized leaf (e.g. `TaskElapsedSummary`).
- Memoize hot list rows and columns (`React.memo` on `TaskCard` / `KanbanColumn`); keep callbacks from board parents stable with `useCallback`.
- Virtualize long scroll lists when item count can grow large. Kanban columns use `@tanstack/react-virtual` at ≥ 15 tasks; keep full id lists for `@dnd-kit` SortableContext.
- Variable-height virtual rows **must** attach `ref={virtualizer.measureElement}` (and `data-index`). Overscan only renders extra items; it does not absorb height variance. `CodexTerminal` is the reference. Underestimated `estimateSize` without measuring stacks absolute rows on top of each other.

## Testing Expectations

Runner: **Vitest**, `environment: "node"`, config in `vitest.config.ts`. `npm test` watches, `npm run test:ci` runs once and is a CI hard gate.

Scope rules:

- Tests are colocated: `src/lib/utils.test.ts`, `src/stores/taskStore.test.ts`. `include` is `src/**/*.test.ts` (`.ts` only — there are no component tests).
- **Pure functions only.** No mocks, no jsdom, no `@tauri-apps/api` imports in a test file. If testing something requires stubbing `invoke`, extract the logic into an exported pure function instead and test that (see below).
- Prefer backend Rust tests for business invariants; use UI smoke for wiring.

### Test files live inside `tsconfig`'s `include`

`tsconfig.json` has `include: ["src"]`, so `npm run build` (`tsc && vite build`) typechecks test files too. That is intentional — a broken test type fails the build gate rather than rotting silently. Two consequences:

- Test files must satisfy `strict` / `noUnusedLocals` / `noUnusedParameters`.
- Use **explicit imports** (`import { describe, it, expect } from "vitest"`), not `globals: true`. Globals would require adding `"types": ["vitest/globals"]` to `tsconfig.json` and would leak test types into app code; explicit imports need no tsconfig change at all.

### Making store logic testable

Store actions are `async` and call `invoke` through `@/lib/backend`, so they are not directly testable under these rules. When a store holds logic worth locking down, extract it as an exported pure function and have the action call it:

```ts
// src/stores/taskStore.ts
export function filterTasksByVisibleProjects<T extends { project_id: string }>(
  rows: T[],
  visibleProjectIds: Set<string>,
): T[] {
  return rows.filter((row) => visibleProjectIds.has(row.project_id));
}
```

Rules for such an extraction:

- The action **must** call the extracted function. Leaving the original inline copy in place is the failure mode to check for (`rg` the predicate afterwards).
- Extraction must be behavior-preserving — no signature widening, no added normalization.
- A module-private helper that is already pure (e.g. `resolveSelectedSshConfigId` in `projectStore.ts`) only needs `export` added; do not restructure it.

### Assertion quality

- Assert real semantics, not shape. `expect(true).toBe(true)` and snapshot-only tests do not count.
- Cover the fallback/degenerate branch, not just the happy path — e.g. `getActivityActionLabel` returning the raw key for an unmapped action is the guard for the historical "dashboard shows snake_case" bug.
- For helpers that take an injectable clock (`isTaskOverdue(task, today)`, `getTaskElapsedSeconds(task, nowMs)`), always pass it so the test is deterministic.
- Verify the label-vs-key distinction where matching is done on Chinese labels (`getKeywordMatchedActions` matches 新增SSH配置, not `ssh_config_created`).

## Forbidden Patterns

1. Frontend SQL writes / schema changes.
2. Direct `invoke` sprawl outside the IPC bridge modules.
3. Silent `catch {}`.
4. `@ts-ignore` to force incorrect payload shapes.
5. Introducing a second CSS or state framework without an explicit project decision.
6. Checking in secrets, SSH passwords, or raw private keys into frontend code/localStorage beyond existing secure ref patterns managed by backend.

## Review Checklist (frontend)

- [ ] Types updated in `types.ts` if entities changed
- [ ] `backend.ts` wrapper added/updated for new commands
- [ ] Store read/write split preserved
- [ ] Labels/dates/SSH compatibility handled
- [ ] `npm run test:ci` passes; new pure logic has assertions
- [ ] `npm run build` passes
