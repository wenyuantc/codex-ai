# Implement · native 看板计划运行

1. `catalog`/`loop.rs`：read_only 过滤 extra tools + 单测
2. `session.rs`：plan_mode 两轮、活动日志、启动参数；`pipeline`/`fix_loop`/`review`/`restart`/`resume` 补参
3. `run_queue.rs`：QueuedTaskRun.plan_mode + drain + serde 默认测
4. `prompt/mod.rs` + `identity.md`：plan 环境块
5. 前端穿线 + TaskCard 右键 + `[PLAN]` 配色 + i18n/activity/utils.test
6. spec：ai-engines / run-queue / i18n
7. clippy、format:check、test:ci、cargo test native+run_queue、npm run build
