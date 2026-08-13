# PRD · C1 审查行级定位

父任务:`08-12-product-gap-wave` · 优先级 N-P2

## 需求

1. AI 代码审查的输出契约扩展为结构化 findings:`{file, line, severity, message}` 列表(在现有 verdict/阻塞数/摘要之上)。
2. findings 落库,与审查记录关联;重复审查覆盖旧 findings。
3. 审查结果 UI 展示 findings 列表(文件+行号+级别+说明),点击可打开对应文件 diff 并定位到行(Monaco 锚定/高亮)。
4. AI 输出不含结构化 findings 时优雅降级为现状(只有整体结论),不报错。

## 约束

- 手动审查与自动化审查(task_automation)走同一解析,行为一致。
- diff 数据复用 `codex_session_file_change_details` 已存明细。

## 验收标准

- [ ] 审查 prompt 要求结构化输出;解析函数有单测(含畸形输出降级)。
- [ ] findings 落库可查;任务详情审查区展示列表。
- [ ] 点击 finding 打开 diff 并跳到对应行(高亮可见)。
- [ ] i18n zh/en 齐全;活动日志照写。
