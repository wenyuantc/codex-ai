# Quality Guidelines

> Practical quality bar for frontend changes in this repo.

---

## Verification Commands

There is **no ESLint/Prettier CI config** in this project today. Minimum checks:

```bash
# Typecheck + production bundle
npm run build

# Frontend-only dev server
npm run dev

# Full desktop app smoke
npm run tauri:dev
```

For backend-impacting UI flows, also run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

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

## Testing Expectations

- No established frontend unit test runner yet.
- New automation, if added, should live near the feature or under `src/__tests__` and must not require rewriting the architecture.
- Prefer backend Rust tests for business invariants; use UI smoke for wiring.

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
- [ ] `npm run build` passes
