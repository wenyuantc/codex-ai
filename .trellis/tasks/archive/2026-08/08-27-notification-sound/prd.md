# PRD · 通知中心声音提醒

## Goal

通知中心收到新通知时发出本机提示音，用户可在设置页独立「通知」Tab 开关并试听，避免只靠铃铛角标漏看审核、运行失败、SDK/SSH 异常等事件。

## Background

- 通知中心 `src/components/layout/NotificationCenter.tsx` 只有铃铛、未读角标和 Sheet，无声音。
- Store `src/stores/notificationStore.ts` 只同步列表，不播音频。仓库内无 `sound` / `audio` 实现。
- 系统桌面通知已存在：`src/lib/desktopNotifications.ts` 订阅 `notification-center-deliver`。窗口可见且聚焦时默认不弹系统通知（带 `task_id` 除外）。
- 后端只在 `created` / `reactivated` / `updated` / `transient` 发出该事件（`src-tauri/src/notifications.rs`）；`read` / `resolved` / `all_read` / `retriggered` 不投递。
- 设置页现有 Tab：界面与运行、Git 与自动质控、提示词模板、MCP、AI 渠道、子智能体、SSH、数据库维护。无通知 Tab。接入点：`SettingsTabValue`、`SETTINGS_TAB_KEYS`、`getSettingsTabFromSection` / `getSectionForSettingsTab`、`settings:tabs.*`。
- 新功能须写活动日志，仪表盘用 `getActivityActionLabel()` → `activity:actions.*`（zh-CN + en）。同类偏好走 `app_config_dir` JSON + Tauri 命令（`native/settings.rs`、`session_events_policy.rs`），不写 SQLite。
- 声音是本机 UI 能力，与当前项目本地/SSH 无关；设置不按 SSH 配置分套。

## Requirements

1. 新通知投递时播放内置短提示音，覆盖持久通知与 transient 通知。触发源与桌面投递相同：`notification-center-deliver` 的 `created` / `reactivated` / `updated` / `transient`。列表刷新、已读、解除不播放。
2. 应用前台聚焦时也播放。后台同样播放；不控制操作系统通知自带声音（可能叠音）。
3. 设置页新增独立「通知」Tab（`/settings?section=notifications`），仅含：声音开关（默认开启）+ 试听。zh-CN / en 齐全。
4. 关闭后，真实通知不再发声；试听在关闭时仍可播放，便于确认音色后再打开。
5. 保存后立即生效，重启后保持。桌面端写入应用配置目录 JSON，并写活动日志 `notification_sound_settings_updated`（`project_id` 可空）。仪表盘中文为「更新通知声音设置」。
6. 本地与 SSH 环境模式行为一致。Tab 不依赖所选 SSH 配置。
7. 浏览器-only（`npm run dev`）不得崩溃：提示音仍可播；偏好可走 localStorage 降级，不写活动日志。

## Out of scope

- 自定义上传音频、多套音色、音量滑杆、按严重级别过滤
- 免打扰时段、按项目/员工静音、按 SSH 主机分套
- 修改系统桌面通知插件声音
- 通知中心 UI 改版或新的通知类型
- 语音播报正文
- 新的 SQLite 表或迁移

## Acceptance Criteria

- [ ] 声音开启时，新通知投递可听到提示音；关闭后真实通知不发声
- [ ] 应用前台聚焦时也会发声（不依赖系统通知是否弹出）
- [ ] 设置页有独立「通知」Tab；含开关与试听；默认开启
- [ ] 试听在开关关闭时仍可发声
- [ ] 配置保存后立即生效，重启后保持
- [ ] 桌面端保存后仪表盘可见「更新通知声音设置」；en 包同步存在
- [ ] 文案走 i18n（zh-CN + en）；组件不 `invoke()`
- [ ] 本地与 SSH 环境模式行为一致
- [ ] `npm run dev`（无 Tauri）打开设置通知 Tab 不崩溃

## Key Decisions

- 设置 Tab 范围：开关 + 试听（用户确认，默认开启）。
- 不按严重级别、音量或自定义音频扩展。
- 与桌面投递共用同一事件，不另造通知总线。
- 配置文件持久化 + 活动日志；无数据库迁移。

## Risks

- 后台时系统通知可能自带系统音，与本机提示音叠加。接受为 MVP 行为，不在本任务抑制系统音。
- Web Audio 在首次用户手势前可能被挂起；需在应用内解锁 AudioContext，否则启动后第一条通知可能无声。
