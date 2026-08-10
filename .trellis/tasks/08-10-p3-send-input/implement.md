# send_input — Implement

## Depends on

`08-10-p3-reports` archived/checked (O1). Strings stay Chinese until i18n child.

## Checklist

1. [ ] Research each engine CLI/SDK for mid-session input; record exemption evidence if impossible
2. [ ] Kernel: retain stdin on interactive sessions; tests for handle lifecycle
3. [ ] Codex path first → matrix true + working `send_codex_input`
4. [ ] Claude / OpenCode / Grok adapters; flip matrix or document B1 exemption
5. [ ] Frontend `SessionInputBar` + wire three terminal hosts; capability gating
6. [ ] Update `docs/analysis/01-domain-capability-matrix.md` honesty notes
7. [ ] format:check + clippy + tests

## Validation

```bash
npm run format:check
npm run test:ci
cargo test --manifest-path src-tauri/Cargo.toml send_
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## Acceptance gate

- Codex `send_input=true` with verifiable send
- At least one more engine true **or** B1 evidence filed for each remaining false
- Total true ≥ 2 including Codex
