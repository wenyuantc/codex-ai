# Design: lint-toolchain

## Overview

在不大改业务代码结构的前提下，建立前后端统一的 lint/format 门禁：

```text
开发者本地                          CI (lint.yml)
─────────────                      ─────────────
npm run lint / format:check   →    Node 20 + npm ci + lint + format:check
cargo clippy -D warnings      →    rust-toolchain stable + clippy -D warnings
npm run build (既有)          →    可选同 job 或仅本地
cargo test (既有)             →    可选；本任务默认不把完整 test 塞进 lint job（耗时长）
```

## Tool choices

| 层 | 工具 | 版本策略 | 理由 |
|----|------|----------|------|
| Rust lint | `clippy` (stable) | 与 CI `dtolnay/rust-toolchain@stable` 一致 | 官方、零新依赖 |
| Rust format | `rustfmt` | 可选门禁 | 若 `cargo fmt --check` 存量 diff 过大，可本任务只引入 clippy，rustfmt 列为 follow-up；优先评估一次 `cargo fmt` 是否可接受 |
| TS lint | ESLint 9 flat config + `typescript-eslint` | 当前 major | 与 Vite/TS 生态匹配；flat config 为官方方向 |
| TS format | Prettier 3 | 当前 major | 与 ESLint 通过 `eslint-config-prettier` 解耦 |
| React | `eslint-plugin-react-hooks` | 匹配 React 19 | 捕获 hooks 误用 |

不选 Biome：一次换栈收益有限，团队与周边文档更熟悉 ESLint/Prettier。

## Config layout

```text
/
├── eslint.config.js          # flat config，@/* 对齐 tsconfig paths
├── .prettierrc               # 2 空格、单引号或双引号与现有代码对齐（先扫描再定）
├── .prettierignore           # dist, src-tauri/target, node_modules, lockfiles
├── package.json              # scripts: lint, lint:fix, format, format:check
├── src-tauri/
│   └── src/lib.rs            # #![allow(clippy::too_many_arguments)] + 注释
└── .github/workflows/
    └── lint.yml              # PR + push 门禁
```

### ESLint 范围

- Include: `src/**/*.{ts,tsx}`，以及根部 `*.ts` / `vite.config.ts` / `scripts/**`（若存在）
- Ignore: `dist`、`src-tauri`、`node_modules`、生成物
- 规则基线：`typescript-eslint` recommended + react-hooks recommended；**不**开 `type-checked` 全量规则（避免首轮海量噪音）；strict type-aware 可作为后续迭代

### Clippy 范围

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

- Crate 级 `#![allow(clippy::too_many_arguments)]` 放在 `src-tauri/src/lib.rs`（或 `main`/`lib` 入口），注释引用本任务 PRD 策略。
- 其余 20 条：优先 `cargo clippy --fix`，不能 auto 的手工改。

### Prettier 风格探测

在引入前用抽样确认现有习惯（双引号 / 分号 / trailing comma）。目标：**format 后 diff 可接受**，不为「风格辩论」改语义。

若全量 format 产生过大 diff，可：

1. 本任务仍跑 format 并提交（推荐，一次痛）；或
2. 仅 `format:check` 新文件 + 后续渐进（**违反父任务清零策略，不采用**）。

## CI design

新建 `.github/workflows/lint.yml`：

```yaml
on:
  pull_request:
  push:
    branches: [main]
```

Jobs:

1. **frontend-lint**
   - setup-node 20, cache npm
   - `npm ci`
   - `npm run lint`
   - `npm run format:check`
   - `npm run build`（保证 tsc + vite 仍绿）

2. **rust-lint**
   - rust-toolchain stable + clippy (+ rustfmt if enabled)
   - rust-cache workspaces `./src-tauri`
   - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
   - 可选 `cargo fmt --all -- --check`

不修改 `build.yml` 的 tag 打包语义。

## Conflict surface with sibling tasks

| 兄弟任务 | 冲突 | 缓解 |
|----------|------|------|
| C5 engine trait | 引擎签名大改 → clippy 再响 | 本任务 allow `too_many_arguments`；C5 后重跑 |
| C6 split large modules | 文件移动 → 路径/import | ESLint 路径别名不变；合并后重跑 format/lint |
| C3/C4 前端 | store/组件改动 | 并行时可能 format 冲突；合入时以 main 为准再 format |

## Rollout / rollback

- **Rollout**: 单分支 `feat/lint-toolchain` 一次合入配置 + 清零修复 + CI。
- **Rollback**: 删除/禁用 `lint.yml`；保留本地 scripts 无害。业务代码机械修复一般可保留。
- **不引入** runtime 依赖；仅 devDependencies + CI。

## Risks

1. ESLint 首次扫描告警量未知 → 实现阶段先装工具出 baseline 再清零。
2. Prettier 全量 format  diff 大 → 接受为独立 commit 或同 commit 清晰分区。
3. Clippy 版本漂移（本地 vs CI）→ 文档写明 stable；不 pin 具体 nightly。
