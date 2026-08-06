# Implement: 前端测试网最小闭环

## Order & Checklist

### 0. Before code

- [ ] Load `trellis-before-dev`；精读：
  - `.trellis/spec/frontend/quality-guidelines.md`
  - `.trellis/spec/frontend/state-management.md`
  - `.trellis/spec/frontend/type-safety.md`
- [ ] 确认基线：`npm run lint` / `npm run format:check` / `npm run build` 当前均通过（ESLint 有 29 个既有 `exhaustive-deps` warning，0 error，属正常基线）

### 1. 工具链接入

- [ ] `npm i -D vitest@^3`（须与 Vite 7 兼容；不装 jsdom / @testing-library/*）
- [ ] 新增根级 `vitest.config.ts`：`environment: "node"`、`include: ["src/**/*.test.ts"]`、alias `@` → `./src`（签名见 design.md）
- [ ] `package.json` scripts 增加 `"test": "vitest"` 与 `"test:ci": "vitest run"`
- [ ] `eslint.config.js` TS 块 `files` 加入 `"vitest.config.ts"`
- [ ] 冒烟：临时建一个最小测试跑通 `npm run test:ci`，确认 runner 起得来后再写真实用例

### 2. 生产代码纯函数化（不得改变行为）

- [ ] `src/stores/taskStore.ts`：新增导出 `filterTasksByVisibleProjects`；**两个调用点都换**——`fetchTasks`（`:162`）与 `fetchTrashedTasks`（`:342`）
- [ ] `src/stores/projectStore.ts`：给 `:90` 的 `resolveSelectedSshConfigId` 加 `export`（函数体不动）
- [ ] `src/stores/dashboardStore.ts`：给 `:60-110` 的 `isInvalidDateRange` / `getKeywordMatchedActions` / `getKeywordMatchedStatuses` / `getAvailableActivityActions` / `buildActivityScopeInput` 加 `export`（函数体不动）
- [ ] grep 确认无残留内联过滤：`grep -n "visibleProjectIds.has" src/stores/taskStore.ts` 应只出现在新函数内

### 3. 首批用例（全部显式 `import { describe, it, expect } from "vitest"`）

- [ ] `src/lib/utils.test.ts`
  - `getActivityActionLabel`：抽样若干后端真实 action key 命中中文；未知 key 回退为原 key（对应 `utils.ts:271` 的 `labels[action] || action`）
  - 1–2 个工具函数（`isTaskOverdue` / `formatDuration`），含边界值
- [ ] `src/stores/taskStore.test.ts`
  - 过滤剔除不在可见集合中的任务；空集合 → 空数组；不改变入参
- [ ] `src/stores/projectStore.test.ts`
  - 当前项目 `project_type === "ssh"` 且有 `ssh_config_id` → 优先返回它
  - 已选 id 在列表中 → 保留
  - 已选 id 不在列表 → 回退首个
  - 列表为空 → `null`
- [ ] `src/stores/dashboardStore.test.ts`
  - `isInvalidDateRange`：start > end 为真；单边缺失为假；非法日期串为假
  - `getKeywordMatchedActions`：按中文标签匹配
- [ ] 断言总数 ≥ 10 且每条都有真实语义（禁止空壳断言）

### 4. 门禁与文档

- [ ] `.github/workflows/lint.yml` 的 `frontend-lint` job：在 `Prettier check` 之后、`Typecheck + production build` 之前插入
  ```yaml
      - name: Unit tests
        run: npm run test:ci
  ```
- [ ] `CLAUDE.md` Commands 区补 `npm test` / `npm run test:ci`，并在 lint gate 一行并入测试

### 5. Validation gate

- [ ] `npm run test:ci`
- [ ] `npm run build`（**关键**：验证 `tsc` 对测试文件的类型检查通过）
- [ ] `npm run lint`（0 error）
- [ ] `npm run format:check`
- [ ] 手工确认抽取无行为变化（见下）

## Validation Commands

```bash
npm run test:ci
npm run build
npm run lint
npm run format:check
```

手工冒烟（验证 R2 抽取无行为变化）：

1. `npm run tauri:dev` 启动，看板/任务列表正常加载且只显示可见项目的任务
2. 回收站页面任务列表正常
3. 本地/SSH 环境模式切换后 SSH 主机选择行为与改前一致
4. 仪表盘活动筛选：关键词搜索、日期区间非法时的提示与改前一致

## Risky Files / Rollback Points

| 风险点 | 回滚 |
|--------|------|
| `src/stores/taskStore.ts` 两处调用点 | 改回内联 `.filter(...)` |
| `src/stores/dashboardStore.ts` / `projectStore.ts` | 仅加了 `export`，删除关键字即可 |
| `.github/workflows/lint.yml` | 删除新增 step |
| `package.json` / `vitest.config.ts` | 删除脚本与配置文件，卸载 vitest |

紧急回滚：删 `vitest.config.ts` + `src/**/*.test.ts` + CI step + package.json 脚本；store 抽取无行为变化，可保留。

## Review Gates

1. 抽取的每个函数都**真的被 store 调用**，不存在「加了导出函数但原地内联逻辑还在」
2. 无 mock、无 jsdom、无 `@tauri-apps/api` 导入进入测试文件
3. 断言 ≥ 10 条且有语义；`getActivityActionLabel` 的未知 key 回退被覆盖
4. `npm run build` 通过 —— 这是本任务最容易踩的坑（测试文件在 `tsconfig` 的 `include` 内）
5. CI 步骤位置正确（format:check 之后、build 之前）
6. 未引入 jsdom / RTL / 覆盖率阈值

## Follow-ups (not this task)

- 组件渲染测试（需另配 jsdom project）与 React Testing Library
- Playwright E2E
- 覆盖率阈值门禁
- `dashboardStore` 中本次仅导出未断言的 3 个辅助函数补测
