# P0 主路径可用

## Goal

交付 TASK.md P0 五项，使已有能力在主路径上可发现、可拦截、文案可信。

## Children

1. `08-10-p0-onboarding-checklist` — 首次使用引导
2. `08-10-p0-tester-discoverability` — 测试员自动化可发现
3. `08-10-p0-dependency-enforcement` — 依赖真正拦截
4. `08-10-p0-multitask-cta-copy` — 同员工多任务 CTA
5. `08-10-p0-dashboard-report-errors` — 仪表盘报表错误可见

## Acceptance

- [ ] 五项子任务均完成并勾选 TASK.md P0
- [ ] `npm run format:check` + clippy `-D warnings` 通过
- [ ] 兼容 local/SSH 模式（引导与依赖拦截不假设仅本地）

## Out of scope

P1–P3 条目；不实现 `send_input`。
