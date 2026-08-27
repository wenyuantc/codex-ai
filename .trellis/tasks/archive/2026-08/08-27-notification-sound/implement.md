# Implement · 通知中心声音提醒

## Checklist

1. **后端配置模块**
   - 新增 `src-tauri/src/app/notification_sound.rs`（对标 `session_events_policy.rs`）
   - `db/models.rs` 增加 `NotificationSoundSettings { enabled: bool }`
   - `app/mod.rs` 声明模块；`lib.rs` 注册 `get_notification_sound_settings` / `update_notification_sound_settings`
   - 缺文件默认 `enabled: true`；变更时 `insert_activity_log`
   - 纯函数/路径 roundtrip 测试（缺文件、坏 JSON、开关往返）

2. **前端 IPC 与类型**
   - `src/lib/types.ts` 增加类型
   - `src/lib/backend.ts` 增加 get/update 封装，组件不 `invoke`

3. **播放模块**
   - 新增 `src/lib/notificationSound.ts`：Web Audio beep、delivery 去重、enabled 缓存、AudioContext 解锁
   - 可测纯函数（去重 key、解析 localStorage 偏好）放同文件并加 `notificationSound.test.ts`
   - `MainLayout` 初始化 bridge（与 `initDesktopNotificationBridge` 并列）

4. **设置 Tab**
   - `shared.ts`：`SettingsTabValue` + section 映射
   - `SettingsPage.tsx`：`SETTINGS_TAB_KEYS` + `TabsContent`
   - 新增 `NotificationSettingsTab.tsx`：开关、试听、保存；非 Tauri 走 localStorage
   - 保存成功后同步播放模块缓存

5. **i18n 与活动日志**
   - `settings:tabs.notifications` 及 Tab 内文案（zh-CN + en）
   - `activity:actions.notification_sound_settings_updated`
   - `locale.test.ts`、`utils.test.ts` 增加 action 断言

6. **验证**
   - 见下方命令；桌面端手工：投递通知听声、关开关、试听、重启、SSH 模式打开 Tab、仪表盘活动

## Validation

```bash
cargo test --manifest-path src-tauri/Cargo.toml notification_sound
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run test:ci
npm run format:check
npm run build
```

手工（`npm run tauri:dev`）：

- 设置 → 通知：默认开；试听有声
- 触发一条真实通知（如任务进审核）：前台有声
- 关闭开关：真实通知无声；试听仍有声
- 重启后开关状态保持
- 仪表盘出现「更新通知声音设置」
- SSH 环境模式下 Tab 仍可用
- `npm run dev` 打开该 Tab 不崩溃

## Risky files

- `src/pages/SettingsPage.tsx` — 只加 Tab 项与 `TabsContent`，不把开关状态抬到页面级
- `src/components/layout/MainLayout.tsx` — 只加 sound bridge + 手势解锁，不改通知 fetch
- `src-tauri/src/lib.rs` — 只注册两条命令
- `src-tauri/src/notifications.rs` — **不要改** deliver 协议

## Do not

- 不要新迁移
- 不要改 `desktopNotifications.ts` 的聚焦抑制逻辑
- 不要为声音新增 Zustand store
- 不要引入音频二进制资源
- 不要按 SSH 配置分套

## Ready for start

- `prd.md` / `design.md` / `implement.md` 已齐
- `implement.jsonl` / `check.jsonl` 需含真实 spec 条目
- 等用户批准本规划后再 `task.py start`
