# 前端测试网最小闭环

## Goal

给前端建立一张**可在本地和 CI 一键运行**的最小测试网，锁住易回归的纯逻辑：activity action 中文映射完整性、store 的过滤/解析规则、关键工具函数。目标是让这类回归在 PR 阶段被挡住，而不是靠 `npm run build` + 手工 tauri 冒烟发现。

不追求覆盖率，不上 E2E。

## Background

- 前端**零测试**：`src/` 下无 `*.test.*` / `*.spec.*` / `__tests__`；devDeps 无 vitest / jsdom / @testing-library/*；`package.json` 无 `test` 脚本。
- 回归目前只能靠 `npm run build` 与手工 tauri 冒烟。
- 已发生过的对应 bug：看板状态不同步、活动 action key 缺中文映射（渲染出裸 snake_case）。

## Confirmed Facts（代码调研，2026-08-06）

**工具链约束**

| 事实 | 影响 |
|------|------|
| Vite 7 + TS 5.8 + React 19；`vite.config.ts` 已配 `@` → `./src` alias | Vitest 是同链路自然选择 |
| `npm run build` = `tsc && vite build`；`tsconfig.json` `include: ["src"]` | **`src/` 下的测试文件会被 `tsc` 一起类型检查**；vitest 类型必须可解析，否则直接违反「不破坏 `npm run build`」 |
| `tsconfig.json` 开启 `strict` / `noUnusedLocals` / `noUnusedParameters` | 测试文件不能留未使用变量 |
| `tsconfig.node.json` 仅 `include: ["vite.config.ts"]` | 根级新配置文件不在任一 tsconfig 的 include 内 |
| `eslint.config.js` 的 TS 块 `files: ["src/**/*.{ts,tsx}", "vite.config.ts"]`；无 type-aware linting | 新增根级配置文件需挂进该 files 列表 |
| `.prettierignore` 忽略 `*.json`（放行 `package.json`/`tsconfig*.json`/`vite.config.ts`），不忽略 `.ts` | 新增 `.ts` 配置文件受 `format:check` 约束 |
| CI `.github/workflows/lint.yml` 的 `frontend-lint` job：ESLint → format:check → `npm run build`，已跑 `npm ci` | 接测试门禁的天然位置，增量成本仅测试本身耗时 |

**R2 三个目标的可测性差异**

| 目标 | 位置 | 现状 |
|------|------|------|
| `getActivityActionLabel` | `src/lib/utils.ts:121`（~150 个 action → 中文），兜底 `labels[action] \|\| action` | 纯函数，可直接测 |
| 其它工具函数 | `src/lib/utils.ts` 共 ~25 个导出（`isTaskOverdue`、`formatDuration`、`getTaskElapsedSeconds`、`isArtifactCaptureLimited`、`getStatusLabel` …） | 纯函数，可直接测 |
| task 项目作用域过滤 | `src/stores/taskStore.ts:162`、`:342` | 过滤内嵌在调用 `invoke` 的 async action 中 |
| SSH 配置解析 | `src/stores/projectStore.ts:90` `resolveSelectedSshConfigId(sshConfigs, selectedSshConfigId, currentProject)` | **已是纯函数，仅未导出** |
| dashboard 过滤辅助 | `src/stores/dashboardStore.ts:60-110`（`isInvalidDateRange`、`getKeywordMatchedActions`、`getKeywordMatchedStatuses`、`getAvailableActivityActions`、`buildActivityScopeInput`） | 纯函数，模块私有未导出 |
| 环境模式读写 | `src/stores/projectStore.ts:37,56-62` | 直接读写 `window.localStorage`，纯函数化需要传参而非读全局 |

推论：R2 三项中只有「store 过滤」需要额外处理；其余两项零成本。R1 原文提到的 React Testing Library **无对应测试目标**（三项均非组件）。

## Key Decisions

| 决策 | 结论 | 理由 |
|------|------|------|
| 测试运行器 | Vitest | 与 Vite 7 同转换链，复用 alias，无额外构建配置 |
| 测试环境 | `environment: "node"` | 首批用例全为纯函数，不需要 DOM |
| React Testing Library / jsdom | **不引入** | 无组件测试目标；为零收益引入依赖属过度设计 |
| store 过滤测法 | **抽纯函数再测**：把内嵌过滤与私有辅助抽成导出的纯函数，store action 改为调用它们 | 完整覆盖 R2 且零 mock、零 jsdom；顺带降低 store 复杂度。代价是小幅改动 `src/stores/`（无行为变化） |
| 测试全局 API | **显式 `import { describe, it, expect } from "vitest"`**，不开 `globals: true` | 无需改 `tsconfig.json` 的 `types`，`tsc` 直接从包解析类型，不污染全局 |
| 配置文件位置 | 独立 `vitest.config.ts`，不改 `vite.config.ts` | 生产构建配置零改动，最小化「破坏 `npm run build`」风险；alias 仅 1 条，重复成本可忽略 |
| CI 门禁 | **接入 `lint.yml` 的 `frontend-lint` job，作为硬门禁**（在 format:check 之后、`npm run build` 之前） | 纯函数 + node 环境，耗时约 1 秒，job 已跑 `npm ci`，增量成本可忽略；不接则测试会逐渐失效 |

## Requirements

### R1 工具链

- 接入 Vitest：新增 `vitest.config.ts`（`environment: "node"`，`@` → `./src` alias）。
- `package.json` 新增 `test`（watch）与 `test:ci`（单次运行）脚本。
- `eslint.config.js` 的 TS 块 `files` 加入 `vitest.config.ts`。
- 不引入 jsdom / React Testing Library。
- 测试文件使用显式 `vitest` 导入，保证 `npm run build` 的 `tsc` 阶段通过。

### R2 生产代码纯函数化

以下抽取**不得改变运行时行为**，store action 改为调用抽出的函数：

- `taskStore`：导出项目作用域过滤函数，`fetchTasks`（`:162`）与 `fetchTrashedTasks`（`:342`）共用。
- `projectStore`：导出既有的 `resolveSelectedSshConfigId`（`:90`，已是纯函数，仅加 `export`）。
- `dashboardStore`：导出过滤辅助函数（`:60-110`）。

### R3 首批用例

全部为纯函数测试，零 mock、零 jsdom：

- **activity label**：后端已知 action 抽样命中中文映射；未知 key 按 `labels[action] || action` 回退为原 key。
- **task 项目作用域过滤**：不在可见项目集合中的任务被剔除；空集合返回空。
- **SSH 配置解析**：当前项目为 ssh 且带 `ssh_config_id` 时优先返回它；已选 id 存在于列表时保留；否则回退首个；列表为空返回 `null`。
- **dashboard 过滤辅助**：日期区间非法判定；关键词匹配基于中文标签。
- **工具函数**：`src/lib/utils.ts` 中 1–2 个（如 `isTaskOverdue` / `formatDuration`）。

### R4 门禁与文档

- `.github/workflows/lint.yml` 的 `frontend-lint` job 在 `Prettier check` 之后、`Typecheck + production build` 之前插入 `npm run test:ci`。
- `CLAUDE.md` 的 Commands 区补测试脚本，并在 lint gate 一行中并入测试。
- `.trellis/spec/frontend/quality-guidelines.md` 的 Verification Commands 补测试命令（Phase 3.3 执行）。

## Acceptance Criteria

- [ ] `npm run test:ci` 本地绿；`npm test` 可进入 watch
- [ ] 至少 10 条有意义断言，覆盖 R3 全部 5 类目标（非空壳、非 `expect(true).toBe(true)`）
- [ ] `npm run build` 通过（含 `tsc` 对测试文件的类型检查）
- [ ] `npm run lint` 0 error、`npm run format:check` 通过
- [ ] R2 抽取无行为变化：`fetchTasks` / `fetchTrashedTasks` / dashboard 过滤 / SSH 解析的外部行为与改前一致
- [ ] CI `frontend-lint` job 含测试步骤，且测试失败时 job 失败
- [ ] `CLAUDE.md` 记录如何跑测试

## Out of Scope

- 全页面 E2E（Playwright）套件
- 视觉回归
- 组件渲染测试与 React Testing Library
- 覆盖率阈值门禁
- 后端 Rust 测试改动（已有 334 用例，不在本任务范围）
