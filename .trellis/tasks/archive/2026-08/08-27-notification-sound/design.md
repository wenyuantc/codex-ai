# Design · 通知中心声音提醒

## Architecture

本机提示音是前端能力；开关是本机偏好。不改通知投递协议，不新增 SQLite 表。

```text
Rust publish_one_time / sticky / transient
  → emit notification-center-deliver
    → desktopNotifications.ts（系统通知，聚焦时可能抑制）
    → notificationSound.ts（本机 Web Audio，开关开启则播）
设置 → NotificationSettingsTab
  → backend.ts get/update_notification_sound_settings
    → app/notification_sound.rs
      → $APPCONFIG/notification-sound.json
      → activity_logs.notification_sound_settings_updated
```

对标实现：`session_events_policy.rs`（配置文件 roundtrip）+ `native/settings.rs`（变更写活动日志）+ `DatabaseSettingsTab`（Tab 自管加载/保存）。

## Boundaries

| 层 | 职责 | 不做什么 |
|---|---|---|
| `NotificationSettingsTab` | 开关、试听、保存反馈 | 不 `invoke`、不播系统通知 |
| `src/lib/notificationSound.ts` | Web Audio、投递去重、偏好缓存、AudioContext 解锁 | 不进 Zustand |
| `src/lib/backend.ts` | 仅桌面端 get/update 封装 | 不写 localStorage |
| `app/notification_sound.rs` | JSON 读写、normalize、活动日志 | 不碰 `notifications` 表 |
| `notifications.rs` | 保持现有 deliver 事件 | 不嵌入声音逻辑 |

## Contracts

### 配置文件 `$APPCONFIG/notification-sound.json`

```json
{ "enabled": true }
```

缺文件、坏 JSON、缺字段 → `enabled: true`。

### Tauri 命令

- `get_notification_sound_settings` → `{ enabled: bool }`
- `update_notification_sound_settings { enabled: bool }` → 写入后返回最新值；`enabled` 变化时 `insert_activity_log("notification_sound_settings_updated", "声音提醒：开启|关闭", None, None, None)`

模型放 `db/models.rs` 的 `NotificationSoundSettings`，与 `SessionEventsPolicy` 同级。类型同步 `src/lib/types.ts`。

### 活动日志

| action | zh-CN | en |
|---|---|---|
| `notification_sound_settings_updated` | 更新通知声音设置 | Updated notification sound settings |

须写入 `src/locales/{zh-CN,en}/activity.json`，并在 `locale.test.ts` / `utils.test.ts` 各加一条断言。

### 设置 Tab

- `SettingsTabValue` 增加 `"notifications"`
- `getSectionForSettingsTab` / `getSettingsTabFromSection`：section = `"notifications"`
- `SETTINGS_TAB_KEYS` + `settings:tabs.notifications`（「通知」/ `Notifications`）
- 新文件 `src/components/settings/NotificationSettingsTab.tsx`，自管 load/save，避免继续膨胀 `SettingsPage.tsx`
- URL：`/settings?section=notifications`

### 播放

- 监听已有 `onDesktopNotificationDeliver`
- 去重键与桌面通知相同：`buildDesktopNotificationKey` 逻辑抽到可测纯函数或在 sound 模块复用同等规则
- `enabled === false` 时真实投递不播；试听忽略开关
- 内置 Web Audio 短 beep（双音、约 200ms），不引入音频文件
- `MainLayout` 与桌面通知桥并列 `initNotificationSoundBridge()`
- 在 window `pointerdown` / `keydown` 上 `AudioContext.resume()`，解决自动播放策略

### 非 Tauri

与 `DatabaseSettingsTab` 的 `isTauriRuntime` 一致：命令失败或不在 Tauri 时，读写 `localStorage['codex-ai:notification-sound-enabled']`（`"true"` / `"false"`），不写活动日志。播放仍走 Web Audio。

## Data flow

**成功保存（桌面）**

1. Tab 切换开关并保存
2. `updateNotificationSoundSettings({ enabled })`
3. Rust 写 JSON；若变化则写活动日志
4. 派发 `codex-ai:notification-sound-change`（或调用 `setNotificationSoundEnabled`）立即更新播放模块缓存

**成功播放**

1. 后端 emit `notification-center-deliver`
2. sound 模块按 delivery key 去重
3. 读缓存 `enabled`；true 则 `playNotificationSound()`

**失败**

- 保存失败：Tab 显示错误，不改播放缓存
- AudioContext 仍 suspended：本次静默，不弹错误（试听按钮可提示用户再点一次）

## Compatibility

- SSH：设置与播放均为本机，不读 `selectedSshConfigId`
- 无迁移、无 capability 变更（沿用现有 command 权限）
- 不改变系统桌面通知是否弹出

## Trade-offs

- 后台可能与系统通知音叠加 → 换「仅前台播放」可避免，但用户选择了前台也播；后台一并播放更简单、也不依赖系统通知权限。
- 配置走 JSON 而非 localStorage → 为了活动日志与其它桌面偏好一致；浏览器-only 才降级 localStorage。
- 不把开关放进 `notificationStore` → 避免通知列表重渲染绑定音频副作用。

## Rollback

删除新命令/模块/Tab/i18n key，去掉 `MainLayout` 初始化即可。遗留 `notification-sound.json` 无害；活动日志行可保留。
