# PRD · A2 图片附件诚实提示

父任务:`08-20-product-trust-ops` · 优先级 P0

## Goal

用户给任务挂了图片再点运行，会以为模型看见了图。实际：SSH 跳过本地图片；Claude CLI **本地也跳过**。现在只在终端刷 `[WARN]`。

## 证据

- Codex SSH：`src-tauri/src/codex/process/session_launch.rs:688`
- Grok SSH：`src-tauri/src/grok/process/mod.rs:878`
- OpenCode SSH：`src-tauri/src/opencode/process/mod.rs:1942`
- Claude CLI 本地：`src-tauri/src/claude/process/mod.rs:1240-1250`
- 运行 CTA / CreateTask 无预检 UI

## Requirements

1. 点击运行 / 立即执行 / 批量运行前，若本次执行会丢图片，必须在 UI 可见处提示（对话框或内联警告），写清原因（SSH 不传本地图 / Claude CLI 不附带图片）。
2. 提示出现在动作之前，不依赖用户去翻终端。
3. 不传图的引擎/模式下，任务仍可运行（警告非硬拦截），除非用户取消。
4. SSH 与本地都要覆盖；四引擎各自真实能力，禁止一刀切文案。
5. 详情执行 Tab / 创建任务挂图处可有静态说明，但不能替代运行前警告。

## Acceptance Criteria

- [x] SSH 项目 + 图片附件：运行前看到「图片不会传给远程」类文案
- [x] Claude CLI 员工 + 图片：本地运行前也能看到跳过说明
- [x] Codex/OpenCode 本地 SDK 路径不误报「会跳过」（按其真实行为）
- [x] 文案 zh-CN + en；终端 WARN 可保留作补充

## Out of Scope

实现 SSH 传图、让 Claude CLI 吃本地图片、改附件存储。
