# Thinking Guides

> Expand thinking before coding so cross-layer and duplication bugs are caught early.

---

## Why These Guides Exist

Most regressions in this repo come from boundary mistakes, not syntax:
- Frontend writes that should be Rust commands
- Local-only assumptions breaking SSH projects
- New activity actions without Chinese dashboard labels
- Schema field added in one layer but not migrations/types/UI
- Duplicated status/label maps drifting apart

## Available Guides

| Guide | Purpose | When to Use |
|-------|---------|-------------|
| [Cross-Layer Thinking Guide](./cross-layer-thinking-guide.md) | Trace UI ↔ IPC ↔ Rust ↔ SQLite ↔ engines/SSH | Feature spans 2+ layers or changes a payload/field |
| [Code Reuse Thinking Guide](./code-reuse-thinking-guide.md) | Find existing helpers/stores/commands before inventing new ones | Adding utils, labels, hooks, wrappers, or similar UI |

## Quick Triggers

### Cross-layer

- [ ] New Tauri command or invoke wrapper
- [ ] New DB column / migration
- [ ] New activity `action` key
- [ ] Task/session/git flow touching local + SSH
- [ ] Nullable update field (omit vs clear)
- [ ] Engine event shape change
- [ ] `start_*` return value or run-queue / concurrency cap
- [ ] Task template apply (batch create + tags/subtasks, no `create_task` attachments)

→ Read [Cross-Layer Thinking Guide](./cross-layer-thinking-guide.md)

### Reuse

- [ ] About to copy a SQL query or label map
- [ ] New date formatting / status color / priority helper
- [ ] New dialog that mutates tasks/projects/employees
- [ ] New normalize/parse for backend payload fields

→ Read [Code Reuse Thinking Guide](./code-reuse-thinking-guide.md)

## Pre-Modification Rule

Before changing any constant, action key, status string, or path format:

```bash
rg -n "value_to_change" src src-tauri
```

Update every layer that embeds it (Rust constants, TS unions, label maps, migrations if persisted).

## Core Principle

30 minutes of boundary mapping saves hours of desktop debugging across SQLite, IPC, and SSH.
