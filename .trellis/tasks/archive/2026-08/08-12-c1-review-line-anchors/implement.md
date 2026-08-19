# Implement · C1 审查行级定位

依赖：main 已含 v47 / B2。本任务无迁移。

## Checklist

- [ ] `shared.rs`：`REVIEW_FINDINGS_START_TAG` / `END_TAG`
- [ ] `extract_tagged_block` 可被 findings 复用；`extract_review_findings` + 转义闭合标签与 verdict 同行为
- [ ] `app/review.rs`：`ReviewFinding` + `parse_review_findings_json` + `persist_review_session_events`
- [ ] Codex / Claude / Grok Review 结束处改为调 persist helper（删三份重复写 verdict/report）
- [ ] `build_task_review_prompt` 加第三块；`prompt_templates` scene=`review` 同步
- [ ] `get_task_latest_review` + `TaskLatestReview`（Rust + `types.ts`）加 `findings`
- [ ] `format_session_log_line` 隐藏 `review_verdict` / `review_findings`
- [ ] Rust 单测：extract/parse 矩阵；prompt 含标签；persist 畸形不写事件；latest review 带回 findings
- [ ] `src/lib/reviewFindings.ts`：`matchReviewFindingToChange` + Vitest
- [ ] `TaskReviewPanel` 列表 + 点击回调；`TaskDetailDialog` 传 `revealLine`
- [ ] `TaskExecutionChangeDetailDialog` → Monaco DiffEditor + reveal/高亮；无法定位提示
- [ ] i18n：`src/locales/{zh-CN,en}/tasks.json`（列表/级别/空态/无法定位/变更详情）
- [ ] 不改自动化 fix prompt；不新增活动键

## Validation

```bash
cargo test --manifest-path src-tauri/Cargo.toml review
cargo test --manifest-path src-tauri/Cargo.toml extract_review
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run test:ci
npm run format:check
npm run build
```

手工（`npm run tauri:dev`）：

1. 本地任务跑一轮执行再审查；有 findings 时列表在报告上方；点一条打开 Diff，修改后一侧跳到行并高亮。
2. 人为制造缺 `<review_findings>` 或坏 JSON：审查完成、报告仍在、无列表、无报错。
3. 点一个不在变更历史里的路径：提示无法定位。
4. SSH 项目同样点到行（不现场拉远程文件）。
5. 会话页打开同一变更详情：Monaco Diff，无高亮。
6. 终端日志无 findings JSON。切换 en 文案齐全。

## Risky files

- 三引擎 `session_runtime.rs`：只替换 Review 落库，勿改 usage/file-change。
- `TaskExecutionChangeDetailDialog`：会话页共用，reveal 必须可选。
- `extract_tagged_block`：上移时保持转义 `<\/tag>` 行为（已有 verdict 测试）。
- `TaskReviewPanel` / `TaskDetailDialog`：只加列表与回调参数，勿拆大文件。

## Rollback

revert 本功能提交。无迁移。开发库里残留 `review_findings` 事件可留。
