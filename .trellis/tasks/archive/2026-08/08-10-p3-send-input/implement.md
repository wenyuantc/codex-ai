# send_input — Implement

## Depends on

`08-10-p3-reports` archived/checked (O1). Strings stay Chinese until i18n child.

## Checklist

1. [x] Research each engine CLI/SDK for mid-session input; record exemption evidence if impossible
2. [x] Kernel: retain stdin on interactive sessions; tests for handle lifecycle
3. [x] Codex path first → matrix true + working `send_codex_input`
4. [x] Claude / OpenCode / Grok adapters; flip matrix or document B1 exemption
5. [x] Frontend `SessionInputBar` + wire three terminal hosts; capability gating
6. [x] Update `docs/analysis/01-domain-capability-matrix.md` honesty notes
7. [x] format:check + clippy + tests
8. [x] Interactive `awaitFollowups` (default) vs pipeline `false` so live terminals stay input-capable

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
