# PRD · B2 会话页 token 可见

父任务:`08-20-product-trust-ops` · 优先级 P1

## Goal

token 已落库，任务详情和仪表盘能看。会话管理页看不到，查「哪次会话在烧」必须绕路。

## 证据

- 会话记录已有字段：`src/lib/types.ts` `CodexSessionRecord` `input_tokens` 等
- 任务详情：`TaskDetailDialog.tsx` `getTaskTokenUsage`
- 仪表盘：`DashboardPage.tsx` `token_usage` / `token_usage_series`
- `SessionsPage.tsx` / `SessionCard.tsx` 对 token 字段零展示

## Requirements

1. 卡片视图和表格视图都展示该会话 `total_tokens`（可附 input/output）。
2. 无值显示「未知」，禁止显示 0 冒充没消耗。
3. 筛选/排序不做 token 区间（可后续）；本任务只展示。
4. 不新增金额换算。按员工聚合若仪表盘已有 by_provider 即可，本任务不强制新报表。

## Acceptance Criteria

- [ ] `/sessions` 两种视图能看到 token 或「未知」
- [ ] 与任务详情同一会话数字一致（同源字段）
- [ ] i18n zh-CN + en；`formatDate` 不受影响

## Out of Scope

token→金额、按员工新报表、改四引擎解析。
