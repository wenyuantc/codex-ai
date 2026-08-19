# PRD · C1 审查行级定位

父任务:`08-12-product-gap-wave` · 优先级 N-P2

## Goal

审查结束时，用户能在任务详情看到逐条 findings（文件 + 行号 + 级别 + 说明），点击后打开对应文件的 diff 并跳到那一行。不必再对着「3 个阻断问题」自己翻 diff。

## Background

现状：`build_task_review_prompt` 只要 `<review_verdict>` + `<review_report>`。`get_task_latest_review` 只回报告正文。`TaskReviewPanel` 是整篇 Markdown。`TaskExecutionChangeDetailDialog` 仍是纯文本三 Tab，不能 `revealLine`。痛点见 `docs/analysis/08-product-gap-2026-08-11.md` §C1。本波顺序 A1 → A2 → B1 → B2 → **C1** → D1；前四项已归档。

已有基础：`codex_session_events` 已存 `review_verdict` / `review_report`；执行会话已有 `codex_session_file_change_details`（含 before/after 文本）；`ProjectGitFilePreviewDialog` 已是只读 Monaco DiffEditor。

## Requirements

1. **输出契约**：审查 prompt 增加第三块 `<review_findings>`，内容为 JSON 数组，元素 `{file, line, severity, message}`。`file` 为仓库相对路径；`line` 为**新文件** 1-based 行号；`severity` 仅 `blocker` / `warning` / `info`。无问题输出 `[]`。`codex/prompt_templates.rs` 的 `review` 默认文案同步。
2. **同一解析**：手动审查与 `task_automation` 审查会话走同一提取/解析。判定仍只看 `review_verdict`；findings 不影响通过/转人工/修复循环。
3. **落库**：会话结束时把合法 findings JSON 写成 `codex_session_events.event_type='review_findings'`。新审查是新会话，`get_task_latest_review` 只读最新审查会话，因此重复审查在展示上覆盖旧 findings。畸形或缺失则不写该事件，UI 退回现状（只有整体结论/报告），不报错、不失败会话。
4. **读取**：`get_task_latest_review` 增加 `findings: ReviewFinding[]`（无事件则为空数组，不把命令变成失败）。
5. **UI**：审查结果区在 Markdown 报告**上方**列出 findings（级别徽标 + `file:line` + message）。点击后在该任务**执行会话变更历史**里匹配文件（新会话优先；也匹配 `previous_path`），打开已有变更详情弹窗，用 Monaco DiffEditor 在**修改后一侧** `revealLineInCenter` 并高亮。匹配不到、删除文件无 after 文本、行号越界或缺失：弹窗仍可开（若匹配到文件）或 toast/行内提示无法定位，不得崩溃。
6. **终端日志**：`review_findings`（以及已有的 `review_verdict`）与 `review_report` 一样不进会话终端可见行。
7. **文案**：findings 列表、级别、无法定位、变更详情相关文案走 i18n zh-CN + en。活动日志沿用既有 `task_review_completed` / `task_review_failed`，不另增 findings 专用动作键。
8. **兼容**：本地与 SSH 同一路径——锚定只读已落库的 snapshot，不现场 SSH 拉文件。OpenCode 没有 `Review` session kind，本波不给它补审查会话。

## Acceptance Criteria

- [ ] 审查 prompt 与 review 模板都要求 `<review_findings>`；解析有单测：合法数组、`[]`、缺标签、非 JSON、缺 file/message、未知 severity、转义闭合标签。
- [ ] 最新审查的 findings 可经 `get_task_latest_review` 读到；任务详情审查区展示列表。
- [ ] 点击能匹配到的 finding 打开 diff，修改后一侧跳到对应行且高亮可见。
- [ ] 无 findings / 畸形输出时仍只显示原报告，不报错；匹配失败有明确提示。
- [ ] 终端日志看不到 findings JSON；i18n zh-CN + en 齐全。
- [ ] 不新增迁移；手动审查与自动审查行为一致。

## Out of Scope

- 把 findings 注入自动修复 prompt 或改变 verdict 语义。
- 新表或改 `codex_session_file_change_details` 结构。
- hunk 级暂存、行内评论编辑、对未改动文件现场打开完整文件。
- OpenCode 审查会话、审查员 provider 误路由（opencode 审查员当前会走 Codex start）。
- 复制审查结果改为复制 findings JSON（仍复制 Markdown 报告）。
- D1 自动更新。
