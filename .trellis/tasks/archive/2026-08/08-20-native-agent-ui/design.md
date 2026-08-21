# Design · 员工与前端全链路

## 类型

`src/lib/types.ts`：`AiProvider` 加 `"native"`。`Employee` 加 `ai_channel_id: string | null`。

## 员工对话框

与 OpenCode 拉模型类似：选 native 时 `list_ai_channels` 过滤 enabled，模型来自 `models_json`。

## 启动

`src/lib/taskRunSession.ts` 增加 native 分支，调用 `start_native_session`。禁止落入默认 Codex。

## 能力

`get_ai_provider_capabilities` 已含 native（engine 子任务）。UI `can(provider, cap)` 自然生效。
