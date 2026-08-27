# Git AI 提交信息支持内置 Agent（本地 Agent）

## Goal

Git AI 生成提交内容（项目 / 任务 / Worktree 提交对话框的「AI 生成提交信息」）目前无法选择本地 agent（内置 Agent / native）。需要支持：在 Git AI 设置中选择「内置 Agent」作为提供商，并绑定一个 AI 渠道；生成提交信息时通过该渠道调用本地 agent 执行。实现方式参考一次性 AI（一次性 AI）的 native 渠道模式。

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| 「本地 agent」含义 | 应用内「内置 Agent」（native），通过 AI 渠道（OpenAI 兼容 HTTP）执行 | 2026-08-27 |
| 渠道绑定 | 自定义模式使用独立字段 `ai_commit_native_channel_id`（镜像 `one_shot_native_channel_id`），不复用一次性 AI 的渠道 | 2026-08-27 |
| 跟随一次性 AI | 行为不变：一次性 AI 为内置 Agent 时，Git 提交信息生成继续走一次性 AI 绑定的渠道 | 2026-08-27 |
| 执行方式 | 复用 `run_native_one_shot_via_channel`（本地 HTTP、无工具循环），提交信息生成无需工作目录/图片 | 2026-08-27 |
| SSH 项目 | remote profile 的 git_preferences 独立保存渠道；native 执行始终在本机（与一次性 AI SSH 语义一致） | 2026-08-27 |
| 存储 | 设置存 JSON 文件（非 SQLite）→ 无需数据库迁移，新字段 serde default 兼容旧文件 | 2026-08-27 |

## Background

- 前端 `GitAutomationSettingsTab` 提供商下拉使用 `CLI_AI_PROVIDER_OPTIONS`（`src/lib/types.ts` 明确过滤掉 `native`）；`SettingsPage` 加载时 `normalizeCliAiProvider` 把 native 钳回 codex。
- 后端 `run_ai_command_with_options`（`src-tauri/src/codex/process/one_shot.rs`）中 `provider_override="native"` 且无 employee 时直接报错「内置 Agent 一次性调用需要绑定员工」；「跟随一次性 AI」模式 one-shot=native 时已能走 `run_native_one_shot_via_channel`。
- 一次性 AI 已有完整参考实现：`RuntimeSettingsTab` 渠道下拉 + `one_shot_native_channel_id` + `run_native_one_shot_via_channel` + `selectNativeModel` / `resolveNativeThinking`。

## Requirements

### R1 设置：Git AI 支持内置 Agent

- 「单独指定」模式提供商下拉出现「内置 Agent」（native）。
- 选中内置 Agent 后出现「AI 渠道」下拉（复用现有 AI 渠道列表），模型 / 推理强度随渠道联动。
- 未选渠道时保存报错，提示先选择 AI 渠道。
- 本地与 SSH 远程 profile 均可配置。

### R2 生成：调用本地 agent

- 生成提交信息时，内置 Agent 提供商通过所选 AI 渠道执行（本地 HTTP 调用）。
- 渠道缺失 / 停用 / 删除时有明确中文报错。
- 「跟随一次性 AI」模式行为不回归。

### R3 可观测性

- 活动日志 provider 显示「内置 Agent」。
- 设置变更活动日志纳入新字段。

## Acceptance Criteria

- [ ] 设置 → Git AI「单独指定」：提供商可选「内置 Agent」，选中后出现 AI 渠道下拉，模型/推理强度随渠道联动
- [ ] 未选渠道保存被拦截并提示「请先选择 AI 渠道」
- [ ] 保存后 `ai_commit_native_channel_id` 持久化（本地 + SSH remote profile）
- [ ] 项目/任务/Worktree 提交对话框「AI 生成提交信息」在内置 Agent + 渠道配置下成功返回提交信息
- [ ] 渠道停用/删除时生成报错清晰
- [ ] 「跟随一次性 AI」+ 一次性 AI=内置 Agent 场景不回归
- [ ] 活动日志显示「内置 Agent」
- [ ] Rust 单测覆盖：settings 归一化/合并/校验 + ai_commands 选择解析
- [ ] `cargo test`、`cargo clippy --all-targets -- -D warnings`、`npm run test:ci`、`npm run build`、`npm run format:check` 全绿

## Out of Scope

- 改变 CLI 提供商（codex/claude/grok/opencode）的现有行为
- 改变 `run_ai_command` 其他调用方（计划生成/验收清单等仍走员工绑定 native 路径）
- 为 Git AI 增加一次性 AI 之外的运行时状态展示
