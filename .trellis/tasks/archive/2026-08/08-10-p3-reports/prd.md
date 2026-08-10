# 更强报表

## Goal

在现有仪表盘报表洞察之上做 **R1 增强包**（燃尽/可配趋势），**不含**外部 Issues / Jira / GitHub 同步，**不**新开独立报表路由。

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| 形态 | **R1**：仪表盘内增强；可配置日期范围 + 按里程碑燃尽/剩余 + 现有趋势可切换范围 | 2026-08-10 |

## Background（证据）

- 已有：`get_dashboard_report_summary`（完成率、逾期/阻塞/进行中、近 7 日、近 8 周、aging、员工负载）+ 项目/SSH 作用域
- 已有：任务 JSON/CSV 导出、JSON 导入
- 里程碑数据与 UI 已存在（项目/看板侧）
- Issues 同步属明确不做

## Requirements

- 仪表盘报表支持用户可配置的日期/趋势范围（替换或扩展固定近 7 日 / 近 8 周）
- 按**当前作用域内里程碑**展示燃尽或剩余任务趋势（无里程碑时有空态说明，不报错空白）
- 继续尊重当前项目 + SSH 作用域
- 加载失败可见（错误 + 重试）
- 后端扩展现有 report command 或新增专用 command；前端不直写库

## Acceptance Criteria

- [x] 用户可在仪表盘切换/配置趋势日期范围并看到数据变化
- [x] 有里程碑时可见燃尽或剩余视图；无里程碑时空态可读
- [x] Local / SSH 作用域行为正确
- [x] 无 Issues 同步入口或暗示已支持
- [x] 相关 Rust 单测覆盖聚合边界；前端纯函数可测则补测

## Out of Scope

- 独立 `/reports` 页
- Jira / GitHub Issues 双向同步
- 外部 BI 对接

## Notes

CSV/JSON 导出已在上一波完成，本任务不重复验收导出本身。

**依赖顺序（父 O1）**：本子任务最先实施并验收；不依赖 send_input / i18n。

**Check**：2026-08-10 PASS（format / test:ci / dashboard_report / clippy / build）。
