# PRD · 2026-08-11 产品缺口下一波:可度量 + 可批量

> 来源:`TASK.md`「下一波 · 2026-08-11 产品缺口」+ `docs/analysis/08-product-gap-2026-08-11.md`
> 用户指令:创建任务,把这些功能做了(2026-08-12)

## 背景

功能闭环已通,主矛盾变为:**AI 跑起来之后,用户看不见成本、管不住并发、复用不了经验**。本波按分析文档 §3 优先级实现全部未完成功能项。

## 范围(6 个子任务)

| 子任务 | 优先级 | 一句话需求 |
|---|---|---|
| `08-12-a1-token-usage` | N-P0 | 会话 token 用量落库并在任务详情/仪表盘可见 |
| `08-12-b1-run-queue` | N-P0 | 全局并发上限 + 持久化运行队列 + 排队展示 + 批量运行 |
| `08-12-a2-log-export` | N-P1 | 会话终端日志可复制、可导出 |
| `08-12-d1-auto-update` | N-P1 | 应用内检查/安装新版本 |
| `08-12-b2-task-templates` | N-P2 | 任务模板:从任务建模板、从模板建任务 |
| `08-12-c1-review-line-anchors` | N-P2 | 审查输出行级 findings 并锚定 diff |

D2 文档校准已于 2026-08-11 完成,不在本波。

## 明确不做

- hunk 级暂存(整文件接受/回滚是 AI 场景主流)
- token → 金额换算(各引擎计费口径不同,一期只做用量)
- N-P3 技术债(TaskDetailDialog/TaskCard 拆分、后端热点拆分、组件测试网)——非功能项,不在"把这些功能做了"范围,单独立项
- 路线图级不做项照旧(微服务/多端同步/第五引擎/完整 IDE/Issues 同步)

## 跨子任务验收标准(父任务负责)

1. 全部子任务各自验收通过(见各子任务 prd.md)。
2. 质量门禁全绿:`cargo clippy --all-targets -- -D warnings`、`cargo test`、`npm run lint`、`npm run format:check`、`npm run test:ci`、`npm run build`。
3. 仓库约定遵守:数据库改动带迁移(版本连续)、新功能写活动日志且仪表盘显示中文、SSH 模式兼容、时间展示走 `formatDate()`、文案走 i18n(zh-CN + en)。
4. `TASK.md` 勾选完成项;`CLAUDE.md`/`README.md` 计数与能力描述不因本波改动而失真。
5. 按功能分提交(Conventional Commits),提交前逐项通过检查。
