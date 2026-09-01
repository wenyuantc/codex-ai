# Implement · native 装备补齐

## Checklist

1. ApplyPatch：`tools/patch.rs` 解析/应用 + 单测（增/改/删/移动/失配）。
2. catalog + dispatch + permission + ssh delete + CUSTOM_TOOL_NAMES。
3. Skills：`skills.rs` 发现 + Skill 工具 + prompt 注入 + Tauri list/open。
4. Hooks：settings 模型 + bash_with_status + dispatch pre/post + 归一化单测。
5. 前端：Runtime 技能/钩子卡片 + MCP Playwright 预设 + i18n。
6. identity.md / native README / CLAUDE.md / TASK.md。
7. 门禁：clippy、cargo test、format:check、test:ci、build。

## Validation

```bash
cargo test --manifest-path src-tauri/Cargo.toml patch::
cargo test --manifest-path src-tauri/Cargo.toml skills::
cargo test --manifest-path src-tauri/Cargo.toml native::settings
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run format:check
npm run test:ci
npm run build
```

## Review gates

- SSH 路径：ApplyPatch / Skill 发现 / hooks 均不假设本机 fs。
- plan 模式：Skill 可用，ApplyPatch 不可。
- 权限：ApplyPatch 删除走 Delete。
- 无新 SQLite 迁移。
