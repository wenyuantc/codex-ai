# PRD · C1 看板右键与搜索 i18n 收口

父任务:`08-20-product-trust-ops` · 优先级 P2

## Goal

主路径已 i18n，但看板右键和全局搜索类型标签仍硬编码中文。切到 English 这两处会漏出来。

## 证据

- `src/components/tasks/TaskCard.tsx:1715-1787`：「归档」「合并到目标分支」「AI 提交」「AI 解冲突」及 title
- `src/components/search/GlobalSearchDialog.tsx:26-31` `TYPE_LABELS` 写死中文
- leftovers 跟踪：`.trellis/tasks/archive/2026-08/08-10-p3-i18n/leftovers.md`

## Requirements

1. TaskCard 右键用户可见字符串（含 disabled title）进 `tasks` namespace，zh-CN + en。
2. 全局搜索类型标签/导航活动 details 走 i18n，禁止硬编码「项目/任务/员工/会话」。
3. 不在本任务清扫整个 leftovers 清单（Git 深对话框、设置大段正文仍可留）。
4. 活动 key `global_search_navigated` 的 details 随 locale。

## Acceptance Criteria

- [ ] 界面切 en 后，看板右键与 ⌘K 类型筛选不再出现这批中文
- [ ] zh-CN 体验与现网一致
- [ ] `npm run build` + `test:ci` 通过

## Out of Scope

Git 对话框、设置 Tab 正文、ShortcutsHelpDialog 全量抽取。
