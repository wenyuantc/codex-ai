# 执行清单：Git AI 提交信息支持内置 Agent（本地 Agent）

## 顺序

1. 后端 `db/models.rs`：`GitPreferences` + `UpdateGitPreferences` 新字段
2. 后端 `codex/settings.rs`：Raw/默认/归一化/合并/校验/活动日志
3. 后端 `codex/process/ai_commands.rs`：`CommitMessageAiSelection.native_channel_id` + `run_commit_message_native`
4. 后端测试 fixture 更新 + 新增单测
5. 前端 `src/lib/types.ts`
6. 前端 `src/pages/SettingsPage.tsx`
7. 前端 `src/components/settings/GitAutomationSettingsTab.tsx`
8. 语言包 `src/locales/zh-CN/settings.json` + `en/settings.json`
9. 验证（见下）

## 验证命令

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run test:ci
npm run build
npm run format:check
```

## 手工冒烟（npm run tauri dev）

- [ ] 设置 → Git AI「单独指定」→ 提供商选「内置 Agent」→ 出现 AI 渠道下拉 → 选渠道后模型/推理联动
- [ ] 不选渠道直接保存 → 报错「请先选择 AI 渠道」
- [ ] 保存成功 → 重新进入设置，渠道回显
- [ ] 项目提交对话框「AI 生成提交信息」→ 成功返回提交信息，活动日志显示「内置 Agent」
- [ ] 停用渠道 → 生成报错「渠道「X」已停用」
- [ ] SSH 项目 remote profile 同样可配置并生效
- [ ] 「跟随一次性 AI」+ 一次性 AI=内置 Agent 场景不回归

## 回滚点

- 每步完成后可独立编译（Rust 侧 `cargo check`，前端 `npm run build`）。
- 纯增量字段，回滚 = 移除新字段与 UI 分支，无数据迁移。
