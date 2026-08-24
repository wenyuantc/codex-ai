# Design · 协调员计划过程日志

## Boundaries

- Keep `ai_generate_coordinator_task_plan` as the only IPC for plan generation.
- Do **not** call `start_*`, insert a live `codex_sessions` row for this run, or touch run-queue / `handle_session_exit`.
- New event `ai-command-stdout` is isolated from engine session stdout.

## Data flow

```
UI request_id + listen
  → ai_generate_coordinator_task_plan({ ..., request_id })
  → run_ai_command(..., AiCommandOptions { progress, read_only_tools })
      native: AgentRunner allowlist + ToolCtx.read_only
      CLI/SDK: stream stdout, emit parsed tool lines, keep final text
  → emit ai-command-stdout { request_id, task_id, line }
  → parse_coordinator_structured_plan + existing DB writes
  → return { markdown, usage_line }
```

## Contracts

### Event

```
ai-command-stdout
{ request_id: string, task_id: string | null, line: string }
```

Frontend matches `request_id`. Missing/blank `request_id` → no emit.

### Native allowlist

Advertise + execute: Read, Glob, Grep, TodoRead, TodoWrite, WebFetch, WebSearch.  
Reject Write, Edit, Bash, MCP (`只读规划模式禁止调用工具 {name}`).

Workspace: `resolve_one_shot_working_dir` + `ExecutionContext`. SSH uses existing `SshToolRuntime`. No MCP connect. No permission UI (write tools never offered).

Tester path stays `run_native_one_shot` (HTTP, no tools).

### CLI/SDK

Coordinator-only `streamProgress` / line reader:

- Codex SDK: `runStreamed` (reuse session emitters) then final `{ok,text}` JSON line.
- Claude SDK: emit tool summaries from `query()` messages; keep result text.
- Codex CLI: if `--json` probe succeeds, parse `parse_cli_json_event_line` and collect agent text; else keep `wait_with_output`.
- Prefer engine read-only sandbox **only** on this path when a known flag exists.
- Grok/OpenCode: emit parsed lines if cheap; else final text only.

### Frontend

- `GenerateCoordinatorTaskPlanInput.request_id?`
- Feature hook listens for the invoke duration (TaskCard + TaskDetail).
- Dialog: `formatTerminalLine` + `getLineColor`; slightly taller log.

### Prompt

`coordinator_plan` scene: may read repo with read-only tools; must not modify files.

## Compatibility

- No migration.
- No new activity key (`task_plan_generated` already exists).
- SSH: native tools on remote; CLI remote stdout streamed the same way as local.
