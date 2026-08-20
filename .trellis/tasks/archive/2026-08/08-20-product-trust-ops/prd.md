# PRD · 2026-08-20 产品下一波:可信 + 可运营 + 好找

> 来源:`docs/analysis/09-product-gap-2026-08-20.md` · 用户确认:产品向全波 8 项（2026-08-20）

## Goal

上一波（token / 队列 / 日志导出 / 自动更新 / 模板 / 审查行级定位）已落地。本波不堆新能力，把**已经有的功能**纠到用户不会被骗、找得到、管得住。

## 范围（8 个子任务）

| 子任务 | 优先级 | 一句话需求 |
|---|---|---|
| `08-20-a1-employee-status` | P0 | 员工状态改为运行时语义（空闲/运行中/异常）；禁止按 `offline` 拦截启动 |
| `08-20-a2-attachment-honesty` | P0 | 运行前告知图片附件会被跳过的引擎/SSH 场景 |
| `08-20-b1-queue-ops` | P1 | 看板队列列表 + 批量运行跳过原因 |
| `08-20-b2-session-token` | P1 | 会话页展示已落库 token（无值=未知） |
| `08-20-b3-startup-update` | P1 | 启动静默检查更新并通知，安装仍需确认 |
| `08-20-b4-onboarding-engines` | P1 | 首次引导 SDK 步骤覆盖四引擎 |
| `08-20-c1-i18n-leftovers` | P2 | TaskCard 右键 + 全局搜索类型走 i18n |
| `08-20-c2-timer-honesty` | P2 | 工时计时器可操作或空闲隐藏 |

## 明确不做

- N-P3 技术债（TaskCard/TaskDetailDialog 拆分、后端热点、组件测试网）
- hunk 级暂存、token→金额、Grok `send_input`、定时 cron
- 路线图级不做项（微服务/多端同步/第五引擎/完整 IDE/Issues 同步）
- 把 `employees.status=offline` 当成「禁用」硬拦截（会卡死空闲员工启动）

## 跨子任务验收

1. 8 个子任务各自 AC 通过。
2. 质量门禁全绿：clippy `-D warnings`、`cargo test`、`npm run lint`、`format:check`、`test:ci`、`npm run build`。
3. 仓库约定：迁移连续、活动日志中文、SSH 兼容、时间走 `formatDate()`、文案 zh-CN+en。
4. `TASK.md` 勾选本波项；分析文档与 CLAUDE/README 不因本波失真。
5. 父任务不写产品代码；实现一次只 `task.py start` 一个子任务。
