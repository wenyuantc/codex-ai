# P3 Strategic — Parent Implement Plan

## Order (O1)

1. Start + finish `08-10-p3-reports`
2. Start + finish `08-10-p3-send-input`
3. Start + finish `08-10-p3-i18n`
4. Parent integration: `TASK.md` / `docs/analysis` / archive parent

## Gates

- Do **not** `task.py start` parent for coding; start the owning child
- Each child: implement → trellis-check → archive
- After all children: parent check of acceptance + commit reminder

## Validation (parent closeout)

```bash
npm run format:check
npm run test:ci
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```
