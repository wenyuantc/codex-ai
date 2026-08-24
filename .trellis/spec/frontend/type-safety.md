# Type Safety

> Strict TypeScript with shared domain types mirroring the Rust/SQLite layer.

---

## Baseline

- `tsconfig.json`: `strict`, `noUnusedLocals`, `noUnusedParameters`
- Path alias: `@/*` → `src/*`
- Domain types: **`src/lib/types.ts`** is the source of truth on the frontend
- Build gate: `npm run build` runs `tsc && vite build`

## Domain Type Rules

1. Entity field names are **snake_case** to match SQLite/Rust serde payloads (`project_id`, `created_at`, `use_worktree`).
2. Prefer string union aliases for closed sets:
   - `TaskStatus = "todo" | "in_progress" | "review" | "completed" | "blocked" | "archived"`
   - `ProjectType = "local" | "ssh"`
   - `AiProvider = "codex" | "claude" | "opencode" | "grok" | "native"` — `normalizeAiProvider` must keep `"native"`. Git/one-shot lists use `CLI_AI_PROVIDER_OPTIONS` / `normalizeCliAiProvider` (native → codex). `normalizeReasoningEffortForProvider("native")` must use `NATIVE_THINKING_LEVELS` (includes `xhigh`/`max`/`minimal`); do not reuse Grok's three-level set.
   - `ArtifactCaptureMode`, `GitActionType`, etc.
3. Nullable DB columns are `T | null`, not optional `?`, when the row always includes the key.
4. Constants such as `PRIORITIES` live next to the types when UI enumerations need them.

## Update Payload Pattern

Rust update DTOs distinguish “field omitted” vs “set null” via `Option<Option<T>>` and `deserialize_explicit_nullable`.

Frontend wrappers should preserve that intent:
- omit a field when not editing it
- pass `null` only when clearing the value is requested

See `UpdateTask` / `UpdateProject` in Rust (`src-tauri/src/db/models.rs`) and the corresponding backend wrapper input types.

## Typing invoke Wrappers

```ts
export async function createTask(input: CreateTaskInput): Promise<Task> {
  return invoke("create_task", { payload: input });
}
```

- Put input interfaces near the wrapper in `backend.ts` when they are command-specific.
- Return shared entity types from `types.ts` whenever the command returns a table model.
- If the raw payload is wider/looser, normalize before returning.

## Component Props

- Explicit `interface Props` per component.
- Import entity types from `@/lib/types`, not inline duplicated shapes.
- Event handlers typed with concrete args (`taskId: string`, `status: TaskStatus`).

## Utility Typing

Helpers in `utils.ts` should take `Pick<Task, ...>` (or similar) when they only need a subset:
- `isTaskOverdue(task: Pick<Task, "due_date" | "status">)`
- `getTaskElapsedSeconds(task: Pick<Task, "time_started_at" | "time_spent_seconds">)`

This avoids over-coupling helpers to full entities.

## Escape Hatches

- Avoid `any`. If a temporary bridge type is needed, prefer a local `type RawX = T & { ... }` then normalize (as in `backend.ts`).
- `@ts-expect-error` appears rarely (e.g. Vite config host env). Do not use it to silence domain model issues.

## Anti-Patterns

- Re-declaring `interface Task { ... }` inside a component file.
- Camel-casing DB fields only on the frontend (`projectId` in stored entities) — breaks select mapping.
- Casting opaque `as Task` without checking required fields after partial updates.
- Widening status/priority to plain `string` in new APIs when a union already exists.
