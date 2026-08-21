# PRD · 员工与前端全链路

> 父任务：`08-20-native-agent`。依赖渠道 API 与引擎 commands。

## Goal

用户能在员工对话框选择「内置 Agent」、绑定渠道和模型，并在看板/任务/会话/设置里正确显示与操作。

## Requirements

- R1 `AiProvider` 增加 `native`；`AI_PROVIDER_OPTIONS` 文案「内置 Agent」。
- R2 创建/编辑员工：选 native 后渠道下拉（仅启用）+ 该渠道模型列表 + 推理强度。无渠道时禁用保存并中文提示。
- R3 `normalizeAiProvider` / 后端 `normalize_employee_ai_provider` 识别 native。
- R4 启动路径 `taskRunSession`、看板、CodexControls、SessionInputBar、运行中会话、能力对比识别 native 与 send_input。
- R5 设置渠道 Tab 已在 channels 子任务；本任务接员工绑定与会话控件。
- R6 i18n zh-CN + en；时间 `formatDate()`。
- R7 native 无渠道或渠道禁用：启动按钮失败原因明确。

## Out of Scope

- 新做独立 Chat 页面
- Skills UI

## Acceptance Criteria

- [x] 创建 native 员工可选渠道+模型并保存 `ai_channel_id`
- [x] 会话页过滤「内置 Agent」
- [x] 运行中会话可发送跟进（矩阵 true）
- [x] 四引擎 UI 无回归
- [x] format:check / test:ci / build 通过
