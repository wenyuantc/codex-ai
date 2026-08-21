# 对话管理补齐执行/审核/编排类型

## Goal

对话管理页能一眼区分会话是「执行」「审核」还是「编排」。编排不再被显示成执行。类型落在 `codex_sessions` 行上，可筛选。

## Background

`codex_sessions.session_kind` 只有 `execution` / `review`。协调员编排步骤也写成 `execution`。对话页把非 review 一律显示成「执行」，类型文字很小，表格没有独立类型列。

`session_kind` 是进程占用槽和前端日志 key，不能扩成第三值 `pipeline`。

## Requirements

1. `codex_sessions` 新增 `session_origin`（`direct` | `pipeline`），默认 `direct`。
2. 历史数据：`task_pipeline_steps.session_id` 指向的会话回填为 `pipeline`。
3. 新编排步骤启动成功并绑上 session 后，立刻把该会话标成 `pipeline`。
4. 展示分类：`session_kind=review` → 审核；否则 `session_origin=pipeline` → 编排；否则 → 执行。
5. 对话管理：类型用彩色 Badge；表格独立类型列；筛选增加「编排」；「执行」筛选项不含编排。
6. 同一套分类用于任务会话链、员工进行中会话、全局搜索副标题。
7. 文案：执行 / 审核 / 编排（zh-CN + en）。不用「运行」。
8. 不改编排状态机、不改各引擎 `start_*` 签名、不新增活动日志 key。看板日志仍走 `execution` 槽。
9. 本地和 SSH 会话同样打标（origin 在本地 SQLite）。

## Acceptance Criteria

- [ ] 对话管理卡片和表格能区分执行、审核、编排
- [ ] 类型筛选三种都能过滤；执行不含编排会话
- [ ] 新编排步骤产生的会话为编排
- [ ] 能从 `task_pipeline_steps.session_id` 回填的历史会话为编排
- [ ] 审核会话仍为审核；编排会话仍可查看改动文件（execution 槽）
- [ ] 看板打开编排中任务日志仍有输出
- [ ] 全局搜索会话副标题对编排显示「编排对话」
- [ ] `npm run build`、`npm run format:check`、`npm run test:ci`、`clippy -D warnings` 通过

## Out of Scope

- 把 `session_kind` 扩成 `pipeline`
- 「修复」作为第四类型
- 步骤重试时已被覆盖的旧 session_id（回填只覆盖步骤表当前指向的会话）

## Notes

用户确认：落 `session_origin`；文案用执行/审核/编排。
