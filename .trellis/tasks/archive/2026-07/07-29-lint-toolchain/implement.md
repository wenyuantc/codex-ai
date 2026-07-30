# Implement: lint-toolchain

## Ordered checklist

### Phase A — 工具接入（不改业务）

1. [x] 前端 devDeps：`eslint`、`@eslint/js`、`typescript-eslint`、`eslint-plugin-react-hooks`、`eslint-config-prettier`、`prettier`、`globals`
2. [x] 新增 `eslint.config.js`、`.prettierrc`、`.prettierignore`
3. [x] `package.json` scripts：
   - `lint`: `eslint .`
   - `lint:fix`: `eslint . --fix`
   - `format`: `prettier --write .`
   - `format:check`: `prettier --check .`
   - `lint:rust`: 包装 clippy 命令
4. [x] 跑一遍 baseline：Clippy 62；ESLint 首轮 128（含 Compiler-style hooks 规则）

### Phase B — Rust 清零

5. [x] `src-tauri/src/lib.rs` 增加 `#![allow(clippy::too_many_arguments)]` + 说明注释
6. [x] 对其余 lint：`cargo clippy --fix` + 手工修不能 auto 的项
7. [x] 验证：`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
8. [x] 验证：`cargo test --manifest-path src-tauri/Cargo.toml`（284 passed）

### Phase C — 前端清零

9. [x] `npm run lint:fix`（安全规则）
10. [x] 手工修剩余 ESLint **errors**；Compiler-style hooks 规则未启用；`exhaustive-deps` 保持 warn
11. [x] `npm run format`
12. [x] 验证：`npm run lint` && `npm run format:check` && `npm run build`

### Phase D — CI 与文档

13. [x] 新增 `.github/workflows/lint.yml`
14. [x] 更新 `Claude.md`、`Agents.md`（去掉「无 lint」）
15. [x] 更新 `.trellis/spec/frontend/quality-guidelines.md`（写入 lint 命令）
16. [x] 更新 `.trellis/spec/backend/testing.md` clippy 说明

### Phase E — 收尾验证

17. [x] 全量命令复跑（见 Validation）
18. [x] 对照 acceptance criteria
19. [ ] 提交（待用户确认后 Conventional Commit）

## Validation commands

```bash
# Rust
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml

# Frontend
npm run lint
npm run format:check
npm run build
```

## Review gates

- [ ] 无业务逻辑语义变更（review 时重点看非 format 的逻辑 diff）
- [ ] allow 仅限文档化的 `too_many_arguments`（或实现期发现的同类「结构性」lint，须写进 PRD/Notes）
- [ ] CI workflow 不破坏 `build.yml`
- [ ] 测试数量不净减少

## Rollback points

| 点 | 动作 |
|----|------|
| A 后 | 删除 config + 还原 package.json |
| B 失败 | 还原 Rust 修复，保留 allow 或整段回退 |
| C 失败 | `git checkout -- src` 后收紧规则或分批 |
| D 失败 | 删除 lint.yml |

## Notes for implementer

- Active task: `.trellis/tasks/07-29-lint-toolchain`
- Branch: `feat/lint-toolchain`
- 大文件（`git_workflow.rs` / `task_automation.rs`）只做机械 clippy 修复，不做拆分。
- 四引擎重复代码的同类 warning 用同一修法，避免只修 codex。
