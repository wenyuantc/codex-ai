# B1 Exemption — Grok send_input

Date: 2026-08-11  
Task: `08-10-p3-send-input`

## Verdict

**Grok `send_input` remains `false`** (B1). Codex / Claude / OpenCode are `true`.

## Evidence

1. **Launch uses `Stdio::null()` for stdin** in both local and SSH session paths (`src-tauri/src/grok/process/mod.rs`). Prompt is passed via `-p` / headless CLI flags, not a retained pipe.
2. **No SDK / Node bridge** exists for Grok — unlike Codex/Claude/OpenCode, there is no long-lived process that can accept NDJSON follow-ups on stdin while keeping the same OS process.
3. **`send_grok_input` is an honest failure command**: if a live process exists it returns a Chinese error explaining headless CLI / null stdin; if none, it reports no live session. It does **not** call resume or start a new session.

## Alternatives considered (rejected)

| Option | Why rejected |
|--------|----------------|
| Keep stdin open on `grok -p` | Headless `-p` batch mode does not implement mid-session conversational stdin; writing after launch is undefined |
| Fake via `resume` / new `start_grok` | Explicitly out of scope in PRD |
| Interactive TTY without `-p` | Would change product contract (permissions, stream JSON, automation exit) beyond this task |

## Matrix notes

`get_ai_provider_capabilities` Grok entry documents B1 / `Stdio::null` / no mid-session stdin. UI gates with `can(provider, "send_input")` and shows capability notes / i18n disabled reason.

## Bridge exit after drain (Codex / Claude / OpenCode)

Date: 2026-08-11

`send_input` keeps Node stdin open with `data`/`end`/`error` listeners. After the turn finishes, bridges drain queued follow-ups via `tryNextLine()` then **must `exit(0)`**. Returning from `main()` alone leaves the event loop alive → Rust never sees process exit → orchestration stays `[执行中]` and `SessionInputBar` looks live. Do not switch to infinite `nextLine()` wait (breaks automation). OpenCode already forced `process.exit(0)`; Codex/Claude session success paths now call `exit(0)` the same way.
