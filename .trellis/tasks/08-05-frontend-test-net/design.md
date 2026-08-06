# Design: 前端测试网最小闭环

## Architecture Overview

新增一条与生产构建**并行且不相交**的测试链路：

```text
npm run test:ci
  → vitest run
  → vitest.config.ts (environment: node, alias @ → ./src)
  → src/**/*.test.ts
       ├─ src/lib/utils.test.ts          → 直接测已导出纯函数
       └─ src/stores/*.test.ts           → 测本任务从 store 抽出的导出纯函数
  → 不加载 @tauri-apps/api、不触碰 window、不发网络
```

生产链路 `npm run build`（`tsc && vite build`）保持不变，唯一交集是 `tsc` 会顺带类型检查 `src/**/*.test.ts` —— 这是刻意保留的，让坏掉的测试类型在构建阶段就暴露。

## Boundaries

| 层 | 职责 | 不负责 |
|----|------|--------|
| `vitest.config.ts` | 测试环境、alias、include 范围 | 生产构建配置 |
| `vite.config.ts` | 生产/开发构建 | 测试配置（**本任务不改动此文件**） |
| `src/lib/utils.ts` | 已有纯函数 | 本任务不改动，只被测试引用 |
| `src/stores/*.ts` | 抽出导出的纯函数 + action 调用它们 | 行为变化、签名扩张 |
| `src/**/*.test.ts` | 断言纯函数契约 | mock、DOM、异步 store action |
| `.github/workflows/lint.yml` | 门禁编排 | 测试实现 |

## Contracts

### 抽取的纯函数签名

```ts
// src/stores/taskStore.ts
export function filterTasksByVisibleProjects<T extends { project_id: string }>(
  tasks: T[],
  visibleProjectIds: Set<string>,
): T[];

// src/stores/projectStore.ts —— 已存在于 :90，仅新增 export
export function resolveSelectedSshConfigId(
  sshConfigs: SshConfig[],
  selectedSshConfigId: string | null,
  currentProject: Project | null,
): string | null;

// src/stores/dashboardStore.ts —— 已存在于 :60-110，仅新增 export
export function isInvalidDateRange(filters: ActivityFilters): boolean;
export function getKeywordMatchedActions(keyword: string, availableActions: string[]): string[];
export function getKeywordMatchedStatuses(keyword: string): string[];
export function getAvailableActivityActions(actions: string[]): string[];
export function buildActivityScopeInput(
  environmentMode: EnvironmentMode,
  selectedSshConfigId?: string | null,
  filters?: ActivityFilters,
): ListActivityLogsInput;
```

`filterTasksByVisibleProjects` 用泛型是因为 `fetchTasks`（`Task[]`）与 `fetchTrashedTasks`（`TrashedTask[]`）共用同一逻辑；两处调用点必须都换成它，否则抽取只是加了个未使用的函数。

`dashboardStore` 的五个函数中，测试只覆盖 `isInvalidDateRange` 与 `getKeywordMatchedActions`；其余仅加 `export` 以保持该组辅助函数的可见性一致，不为凑数写断言。

### 配置契约

```ts
// vitest.config.ts
import { defineConfig } from "vitest/config";
import path from "path";

export default defineConfig({
  resolve: { alias: { "@": path.resolve(__dirname, "./src") } },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
```

- 不设 `globals: true`。测试文件显式 `import { describe, it, expect } from "vitest"`，因此**无需改 `tsconfig.json` 的 `types`**，`tsc` 从 `node_modules/vitest` 解析类型。
- `include` 限定 `.ts`（无 `.tsx`）：本批无组件测试，收窄范围避免误扫。

```jsonc
// package.json scripts
"test": "vitest",
"test:ci": "vitest run"
```

```js
// eslint.config.js — TS 块的 files 需加入新配置文件
files: ["src/**/*.{ts,tsx}", "vite.config.ts", "vitest.config.ts"]
```

## Data Flow

测试不涉及运行时数据流。抽取后的 store 内部流向：

```text
fetchTasks
  → listTasksCommand(invoke)            [不变]
  → visibleProjectIds = Set(projects)   [不变]
  → filterTasksByVisibleProjects(...)   [由内联 .filter 改为函数调用]
  → set({ tasks })                       [不变]
```

## Compatibility & Migration

- 无 DB / IPC / 类型契约变化，纯前端内部重构 + 新增测试。
- 抽取全部是「内联表达式 → 具名导出函数」的等价改写，无行为变化，无需迁移。
- 新增 devDeps 只有 `vitest`（及其自带传递依赖），不引入 jsdom / RTL / happy-dom。
- CI 新增一个步骤；失败语义与既有 lint 步骤一致。

## Trade-offs

| 选择 | 理由 | 代价 |
|------|------|------|
| 独立 `vitest.config.ts` 而非合并进 `vite.config.ts` | 生产构建配置零改动，最小化破坏 `npm run build` 的风险 | alias 重复 1 条，未来改 alias 需同步两处 |
| 抽纯函数而非 mock `invoke` | 零 mock、零 jsdom，测试不与后端命令签名耦合 | 需改 `src/stores/` 生产代码 |
| 显式 vitest import 而非 `globals: true` | 不改 `tsconfig.json`，不污染全局类型 | 每个测试文件多一行 import |
| 测试文件留在 `tsconfig` 的 `include` 内 | 坏掉的测试类型在 `npm run build` 阶段即暴露 | 测试文件受 `noUnusedLocals` 等严格规则约束 |
| node 环境而非 jsdom | 快、依赖少 | 后续要测组件时需另配一个 jsdom project |

## Risks & Mitigations

| 风险 | 缓解 |
|------|------|
| 测试文件被 `tsc` 检查后报错，破坏 `npm run build` | 显式 vitest import；实现后必跑 `npm run build` 验证 |
| `vitest.config.ts` 不在任何 tsconfig `include` 内，编辑器/lint 行为不一致 | 无 type-aware linting，ESLint 只需在 files 列表登记；不强行塞进 tsconfig.node.json 以免影响 `composite` 构建 |
| 抽取时手滑改变行为（如漏改第二个调用点） | R2 明确要求两个调用点都换；check 阶段 grep 确认无残留内联 `.filter(...visibleProjectIds...)` |
| Prettier 对新文件报格式差异导致 CI 红 | 实现后跑 `npm run format:check`（CI 同款命令） |
| CI 步骤顺序放错导致构建先失败掩盖测试结果 | 固定插在 format:check 之后、build 之前 |
| vitest 与 Vite 7 版本不匹配 | 安装时选与 Vite 7 兼容的 vitest 3.x；装完立即跑 `test:ci` 验证 |

## Rollout / Rollback

- 无 feature flag。合并即生效。
- 回滚：撤销 `vitest.config.ts` + 测试文件 + package.json 脚本 + CI 步骤即可；store 抽取可保留（无行为变化）或一并回退。
- 验证顺序见 `implement.md` 的 Validation Commands。
