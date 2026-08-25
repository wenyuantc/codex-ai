# Implement · 自定义子智能体

## Order

1. `native/subagents.rs`：JSON 读写、normalize/validate、CRUD 命令、活动日志、单元测试。注册 `lib.rs`。
2. `agent/subagent.rs` + `prompt/mod.rs` + `loop.rs`：Custom kind、动态工具描述、spawn 工具/prompt/渠道模型。扩展现有测试。
3. 设置 Tab + `native.ts` + i18n + locale/utils 测试。`shared.ts` tab key。
4. `identity.md`、policy hint、spec（ai-engines / directory-structure / i18n）。
5. clippy / format:check / test:ci / build。

## Validation

- `cargo test --manifest-path src-tauri/Cargo.toml native::`
- `npm run test:ci`
- 设置页：新建自定义类型 → 勾工具 → 指定渠道 → 保存再开仍在

## Rollback

去掉 JSON 文件与 Tab；`Agent` 恢复只认 general/explore。
