# PRD · 渠道 API 配置

> 父任务：`08-20-native-agent`。本子任务可独立验收 Settings 渠道 CRUD。不实现 agent 循环。

## Goal

用户能在设置里配置多个模型渠道（协议 + 地址 + 密钥 + 模型列表），密钥与渠道配置一并写入 SQLite，设置页可显示明文。

## Requirements

- R1 迁移 v48：`ai_channels` 表 + `employees.ai_channel_id`（可空，本任务不强制员工 UI）。
- R2 Tauri commands：list / get / create / update / delete / test。前端经 `backend.ts`。
- R3 protocol ∈ `openai | anthropic | codex`。base_url 必填，去尾 `/`。
- R4 API key 写入 `ai_channels.api_key`；读接口返回 `api_key` 与 `api_key_configured`。旧 `api_key_ref` / 钥匙串条目仅作一次性迁移。设置页默认掩码，点眼睛图标显示明文。
- R5 测通：按协议发最小请求（缺 key / 非 2xx 返回中文原因）。测通实现可先用最小 HTTP，正式三协议客户端在下一子任务替换。
- R6 设置页新 Tab「渠道」：列表、新建/编辑对话框、启用开关、测通按钮。zh-CN + en。
- R7 活动日志：`ai_channel_created` `ai_channel_updated` `ai_channel_deleted` `ai_channel_tested`，中文+英文 activity 包。
- R8 删除渠道：若仍有员工 `ai_channel_id` 指向它，拒绝并中文说明。

## Out of Scope

- 员工选择 native / 渠道绑定 UI（子任务 ui）
- 完整 SSE tool-call 客户端（子任务 model-client）
- 启动会话

## Acceptance Criteria

- [x] 新库可创建三种协议渠道；重启后列表仍在（设置页 CRUD + v48 表）
- [x] SQLite 渠道配置保存 API key；设置页可掩码/明文切换；旧钥匙串条目可一次性迁入列
- [x] 更新时留空密钥表示不改；可单独改名称/模型/启用
- [x] 测通失败有中文错误，不泄漏 key
- [x] 仪表盘活动显示「新增 AI 渠道」等中文
- [x] 被员工引用的渠道不能删
- [x] clippy / format:check / 相关 cargo test 通过
