# 引入 lint 工具链

## Goal

为 codex-ai 接入可重复执行的 lint/format 工具链（`cargo clippy` + ESLint + Prettier），清零当前基线告警，并挂到 CI，使后续提交无法再无约束地引入同类问题。

覆盖父任务源发现 **#9**（无任何 lint）。

## Background

- 实测基线（2026-07-30，`src-tauri`）：`cargo clippy` **62** 条 warning；其中 **42** 条为 `clippy::too_many_arguments`（多集中在引擎 session 启动与 git/Tauri 命令签名）。
- 前端：`src/` 约 149 个 TS/TSX 文件、~35k 行；无 ESLint / Prettier；`tsc` 已在 `npm run build` 中强制。
- CI：仅有 `.github/workflows/build.yml`（tag / 手动触发打安装包），**无** PR/push 质量门禁。
- 父任务用户确认约束：**存量告警一次性清零**，再挂 CI 阻断；本子任务在执行顺序上应排在 C5/C6 之后，避免重构推翻已修告警。

## Requirements

### R1 — Rust：Clippy 清零并脚本化

1. 提供可本地/CI 复用的 Clippy 检查命令（warnings = errors）。
2. 当前 62 条基线必须在本任务结束时归零（见「告警处理策略」）。
3. 不改变业务语义；允许为工具链引入的极小机械修复（borrow/strip/`if_same_then_else` 等）。

### R2 — 前端：ESLint + Prettier

1. 接入 ESLint（flat config，覆盖 `src/**/*.{ts,tsx}`）与 Prettier。
2. 提供 `lint` / `lint:fix` / `format` / `format:check` 等 npm scripts。
3. ESLint + Prettier 冲突规则用 `eslint-config-prettier` 关闭。
4. 存量问题一次性清零；允许通过安全 auto-fix 与必要手工修复完成。

### R3 — CI 门禁

1. 新增独立 workflow（建议 `lint.yml`），在 `pull_request` 与 `push`（`main` / `feat/**` 或至少 `main` + PR）上运行：
   - 前端：`npm ci` → typecheck/lint/format check
   - 后端：`cargo clippy`（warnings denied）+ 可选 `cargo fmt --check`（若引入 rustfmt 门禁）
2. 现有 `build.yml` 安装包构建流程保持不变。

### R4 — 文档

1. 更新 `Claude.md` / `Agents.md` 中「无 lint」描述与推荐命令。
2. 更新 `.trellis/spec/frontend/quality-guidelines.md` 与 backend 相关 quality 指引，写入验证命令。

## 告警处理策略（对齐父任务）

| 类别 | 策略 |
|------|------|
| 机械可修（`needless_borrow`、`manual_strip`、`if_same_then_else`、`manual_unwrap_or_default` 等，约 20 条） | **实际修复代码** |
| `clippy::too_many_arguments`（42 条） | **crate / 模块级 allow + 注释说明理由**（引擎启动/Tauri 命令签名参数多；真正收敛参数应在 C5 trait 抽象完成，本任务不借 lint 做大规模签名重构） |
| ESLint 可 auto-fix | 优先 auto-fix |
| ESLint 需手工 | 最小 diff 修复；禁止 `@ts-ignore` 掩盖类型错误 |

> 说明：allow `too_many_arguments` 后 Clippy 仍视为「零 warning」（该 lint 不再报告）。这是一次性清零与避免和 C5/C6 大范围冲突的平衡点。

## 前置条件 / 顺序约束

- 父任务强制：C7（本任务）应在 C5（引擎 trait）与 C6（巨型文件拆分）之后。
- 当前仓库状态（2026-07-30）：C1/C1b/C2 已归档；C3–C6 仍为 `planning`；用户在本 worktree 上主动启动本任务。
- **风险接受**：若本任务先于 C5/C6 合入，后续重构可能再引入 lint 噪声，需在 C5/C6 合并后重跑 lint 并补齐。

## Acceptance Criteria

- [x] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 通过（零 warning）
- [x] `npm run lint` 与 `npm run format:check` 通过（ESLint errors=0；`exhaustive-deps` 为 warn）
- [x] `npm run build` 通过
- [x] `cargo test --manifest-path src-tauri/Cargo.toml` 全绿，测试总数 ≥ 基线（284）
- [x] CI 存在独立 lint workflow，在 PR/push 路径可触发且检查上述门禁
- [x] 项目文档与 Trellis quality 指引已更新 lint 命令
- [x] 无业务行为变更（纯工具链 + 机械修复 + 有文档的 allow）

## Out of Scope

- 不借本任务做引擎 trait 抽象或巨型文件拆分
- 不引入 Biome 替代 ESLint/Prettier（保持生态常见组合，便于贡献者）
- 不改业务功能、数据库 schema、UI 视觉
- 不强制 IDE 插件；只保证 CLI/CI

## Notes

- 本任务为复杂任务：需 `design.md` + `implement.md`。
- 分支：`feat/lint-toolchain`；base：`main`。
