# 前端测试网最小闭环

## Goal

建立 **可在 CI/本地一键运行** 的前端最小测试网，优先锁住易回归逻辑：store 过滤（local/SSH、项目作用域）、`getActivityActionLabel` 映射完整性、关键纯函数。不为追求覆盖率而大上 E2E。

## Background

- 仓库几乎无前端测试 runner；回归依赖 `npm run build` + 手工 tauri。
- 历史 bug：看板状态不同步、活动 key 中文缺失等。

## Requirements

### R1 工具链

- 选定并接入 Vitest（或与 Vite 一致的方案）+ 必要的 React Testing Library（若测组件）。
- `package.json` 增加 `test` / `test:ci` 脚本。

### R2 首批用例

- activity label：后端已知 action 抽样映射存在。
- project/task store 过滤：SSH 开关与项目作用域。
- 1–2 个纯工具函数（如状态文案）。

### R3 门禁建议

- 文档说明本地必跑；可选接入现有 lint workflow（子任务实现时评估耗时）。

## Acceptance Criteria

- [ ] `npm test`（或约定脚本）本地可绿
- [ ] 至少 10 条有意义断言（非空壳）
- [ ] 不破坏 `npm run build`
- [ ] 在 README 或 Claude.md 记录如何跑测试

## Out of Scope

- 全页面 E2E Playwright 套件（可后续）
- 视觉回归
