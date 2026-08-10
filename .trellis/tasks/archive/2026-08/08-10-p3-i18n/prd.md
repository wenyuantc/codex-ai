# i18n 完整框架

## Goal

引入抽取式多语言基础设施，并按 **I2** 在本波尽量覆盖用户可见字符串（zh-CN + en）。

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| 语言与深度 | **I2**：zh-CN + en；本波尽量全量抽取用户可见文案（含错误提示、活动 key→文案） | 2026-08-10 |

## Background（证据）

- 无 `i18n` / `react-i18next` 依赖；UI 文案以中文硬编码为主
- `formatDate` / `toLocaleString("zh-CN")` 等为格式化 locale，不是翻译框架
- 活动日志中文映射已在 `getActivityActionLabel` 等处；需纳入 i18n 或单一真源，避免双写混乱
- 上一波仅加强空态 CTA，明确未做抽取式多语言

## Requirements

- 引入前端 i18n 框架；设置页可切换语言并持久化（重启后保持）
- 默认语言保持现有中文体验（zh-CN）
- 本波抽取范围：导航、各主页面、对话框主路径、设置、看板/仪表盘/员工/会话、能力禁用理由、常见错误/toast、活动 action 文案
- Rust 返回的用户可见错误：能在前端映射的映射；无法映射的允许保留原文但需清单跟踪
- 日期/数字格式随 locale 走现有 `formatDate` 策略对齐

## Acceptance Criteria

- [x] zh-CN / en 可切换且持久化
- [x] 主路径与约定范围内用户可见文案在 en 下可读（无大片中文硬编码残留；允许极少数清单内遗漏并跟踪）
- [x] 活动 key 文案不出现「中英双源冲突」
- [x] 构建/类型检查通过；不破坏暗色主题
- [x] `TASK.md` i18n 项可勾选

## Out of Scope

- 社区翻译平台、运行时远程下发文案
- 自动机器翻译整库（人工/受控英文化）
- 第三语言（ja 等）本波不加

## Notes

I2 工期长。原父 **O1** 为 reports → send_input → i18n；用户已确认 **先做 i18n**（reports 已完成）。`send_input`/U1 落地后需再抽一轮新增文案。

**Depends on**: `08-10-p3-reports` archived.

**Check**：2026-08-10 PASS（format / test:ci / build；深对话框见 leftovers.md）。
