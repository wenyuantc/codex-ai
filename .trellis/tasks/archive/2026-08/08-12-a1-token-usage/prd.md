# PRD · A1 成本可见性:token 用量落库与展示

父任务:`08-12-product-gap-wave` · 优先级 N-P0

## 需求

1. `codex_sessions` 增加 token 消耗列:`input_tokens` / `output_tokens` / `total_tokens` / `reasoning_tokens`,全部 nullable——**无值即未知,不假装 0**。
2. 四个引擎(codex / claude / grok / opencode)的流解析各自提取 usage 事件并在会话结束前落库;解析不到留 NULL。Grok 已有 `summarize_usage()` / `usage_u64()` 取值逻辑可复用。
3. 任务详情「执行」Tab 显示本任务累计 token 用量(有数据才显示)。
4. 仪表盘报表扩展:token 用量趋势(近 7/30 天)+ 按引擎分组汇总。
5. 一期只做 token 用量,不做金额换算。

## 约束

- 迁移版本连续(当前 44 → 新增 v45)。
- SSH 会话与本地会话同样落库(解析发生在流层,与执行目标无关,需验证)。
- 兼容 CLI 与 SDK bridge 两种流格式(codex/claude/opencode 各有两条流路径)。

## 验收标准

- [ ] migration v45 存在且 `migration_versions_are_contiguous` 测试通过。
- [ ] 任一引擎跑完一个会话,`codex_sessions` 对应行有 token 数;解析不到的引擎/模式留 NULL。
- [ ] 任务详情执行 Tab 可见累计 token;无数据时不显示「0」。
- [ ] 仪表盘报表含 token 趋势与按引擎汇总,i18n zh/en 齐全。
- [ ] 各引擎 stream 解析有单元测试(喂样例 JSON 行,断言解析出的 usage)。
