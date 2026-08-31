# Native Agent 缺陷修复

## Goal

按 2026-08-31 Native Agent 缺陷复核报告，修复已确认项（1–4、6–8、11）以及确认要改的部分确认项（5、9）。本地与 SSH 共用同一套分类器、预算和 transcript。

## Requirements

1. 有限 rollout 预算下，未配置 `max_output_tokens` 时也必须向 Provider 传递不超过剩余预算的输出上限；预留与 cap 对齐。Provider 不返回 usage 时按响应文本估算结算。
2. Bash 高风险分类能识别包装器、赋值前缀、转义命令、Git 全局选项、解释器 `-c/-e`、`find -exec`、`xargs`；无法可靠解析时默认确认。
3. 工具结果截断后的 Read continuation `offset` 指向第一个未完整展示的行，不跳过半行剩余内容。
4. 模型摘要请求不再只吃 320 字 / 每类 8 条的 `local_summary`；handoff 保留更多工具观察（尤其错误堆栈）。本地 fallback 也略放宽。
5. 请求带了 tools 且模型返回合法 tool_calls 时，settle 后预算耗尽不得静默丢弃这批调用；planned last_turn（请求未带 tools）仍清空。
6. 模型摘要「有效但不可用」时做一次纠偏，再降级本地摘要；不可用结果不得 `mark_compacted`。
7. Provider 连续报告更小 max_tokens 上限时允许最多 3 次下调重试。
8. 模型列表在 500 条或 20 页截断时，IPC 与设置页能看到 truncated，活动日志沿用 `ai_channel_models_fetched`。
9. 同一批子 Agent 共享一份 `remaining * share%` 的 ChildQuota 池，合计不超过该池。
10. Transcript 按指纹跳过无变化写入，结束时不重复写；落库失败打日志。

## Out of Scope

- 第 10 项图片 token 估算（报告判不成立）
- 第 12 项 try_lock / WaitFollowup 竞争（不成立）
- 完整 POSIX shell AST / transcript 分片表 / 数据库 migration

## Acceptance Criteria

- [x] AC1：有限预算 + `max_output_tokens=None` 时 `reserve_model_call` 返回 cap；usage=0 时按文本结算不低于预留
- [x] AC2：`env rm`、`VAR=x rm`、`\rm`、`python -c`、`find -exec`、`git -c ... push --force` 为 High；`echo hello` / `git status` 为 Low
- [x] AC3：超长单行截断 hint 的 offset 指向该行
- [x] AC4：`compaction_prompt` 含超过 320 字的错误堆栈片段
- [x] AC5：带 tools 的响应在 settle 耗尽后仍执行 tool_calls；planned last_turn 仍丢弃
- [x] AC6：无标题/过短摘要不 compact_with_summary，走本地降级
- [x] AC7：连续两档更小 max_tokens 上限后 chat 成功
- [x] AC8：`ListAiChannelModelsResult.truncated` 为 true 时设置页提示列表不完整
- [x] AC9：三 child 共享同一 ChildQuota，合计上限为 remaining*share%
- [x] AC10：相同 transcript 指纹不二次 upsert；失败有日志
- [x] AC11：clippy / 相关 cargo test / format:check / test:ci / build 通过
