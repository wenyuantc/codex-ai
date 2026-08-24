# Implement · 协调员计划过程日志

## Checklist

1. Event + `AiCommandOptions` on `run_ai_command`; coordinator passes `request_id` + `read_only_tools`.
2. Frontend: `onAiCommandOutput`, hook around `generateCoordinatorPlan`, Dialog 配色。
3. Native: `set_allowed_tools` + `ToolCtx.read_only`; `run_native_read_only_one_shot` with cwd/SSH; tests reject Write and emit `[读取]`.
4. Codex/Claude SDK stream on coordinator path only.
5. Codex CLI `--json` when progress + probe succeeds.
6. Update `coordinator_plan` template and `.trellis/spec/backend/ai-engines.md`.
7. Lint/test/build.

## Validation

```bash
cargo test --manifest-path src-tauri/Cargo.toml native::agent::loop
cargo test --manifest-path src-tauri/Cargo.toml native::tools
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run format:check
npm run test:ci
npm run build
```

## Risky files

- `src-tauri/src/codex/process/one_shot.rs` — shared by tester/commit; default options must preserve old behavior
- `src-tauri/src/codex/sdk_bridge.mjs` / `claude_sdk_bridge.mjs` — session mode return shape
- `src-tauri/src/native/tools/dispatch.rs` — `read_only` must default false for live sessions
- `TaskCard.tsx` / `TaskDetailDialog.tsx` — duplicated generate flow; extract listen only

## Rollback

Revert the coordinator command options, event, native allowlist, and Dialog coloring. Other one-shots must keep working if coordinator path is reverted.
