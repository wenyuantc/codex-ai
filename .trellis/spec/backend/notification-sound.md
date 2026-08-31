# Notification Sound

> In-app alert sound for notification-center deliveries. Preference is a local JSON file, not SQLite.

## 1. Scope / Trigger

- Trigger: new `notification-center-deliver` events (`created` / `reactivated` / `updated` / `transient`) should make an audible alert even when the main window is focused.
- Do **not** use Web Audio / HTMLAudio in the Tauri webview as the primary player. WKWebView autoplay blocks `AudioContext` when the event is not a user gesture; the notification center can update while remaining silent.
- Settings live under Settings → 通知 (`/settings?section=notifications`). Local and SSH environment modes share one machine-local preference.

## 2. Signatures

Rust (`src-tauri/src/app/notification_sound.rs`):

```rust
get_notification_sound_settings(app) -> Result<NotificationSoundSettings, String>
update_notification_sound_settings(app, enabled: bool) -> Result<NotificationSoundSettings, String>
play_notification_sound_alert() -> Result<(), String>
```

```rust
struct NotificationSoundSettings { enabled: bool }
```

File: `$APPCONFIG/notification-sound.json` → `{ "enabled": true }`.

Frontend wrappers in `src/lib/backend.ts`: `getNotificationSoundSettings`, `updateNotificationSoundSettings`, `playNotificationSoundAlert`. Playback orchestration: `src/lib/notificationSound.ts`.

## 3. Contracts

- Default `enabled=true` when the file is missing, JSON is invalid, or `enabled` is absent.
- `update_*` writes the file first; activity log `notification_sound_settings_updated` only when `enabled` actually changes (`声音提醒：开启` / `声音提醒：关闭`, `project_id` null).
- `play_notification_sound_alert` is fire-from-Rust OS audio:
  - macOS: `afplay -v 1 /System/Library/Sounds/Glass.aiff` (`1` is afplay’s normal volume; do not use values above `1`)
  - Windows: `MessageBeep(MB_ICONASTERISK)` (Control Panel “Asterisk” sound)
  - Linux: `canberra-gtk-play -i message`, else `paplay`/`pw-play` of `/usr/share/sounds/freedesktop/stereo/message.oga`
- Frontend plays via the Tauri command; browser-only (`npm run dev`) falls back to a generated ~200ms WAV and stores the toggle in `localStorage['codex-ai:notification-sound-enabled']` (no activity log).
- Do **not** call `HTMLAudioElement.play()` (even muted) to unlock autoplay. WKWebView can ignore `muted`/`volume=0` and leak the fallback beep on the first click. Native playback is OS audio; browser fallback only plays from a real notification/preview.
- Sound is **not** stored in Zustand. Cache `enabled` in `notificationSound.ts`. Preview ignores the toggle.
- Do not play real deliveries until the preference JSON has been read. Default `enabled=true` applies to the cached value after that read, not to the first paint.
- Drop `notification-center-deliver` audio for the first `syncSystemNotifications` after `MainLayout` mount (timeout fallback 4s). List/badge refresh is unchanged. Later deliveries play.
- Coalesce overlapping deliveries: at most one OS/WAV play per 400ms.
- Desktop toast window inspect (`isVisible` / `isFocused`) must fail **closed** (treat as focused, do not send). Do not fail-open a system notification on first paint.

## 4. Validation & Error Matrix

| Condition | Behavior |
|-----------|----------|
| Missing / bad JSON | `enabled: true` |
| Save command fails | Tab shows error; playback cache unchanged |
| Play command fails in Tauri | Frontend tries WAV fallback, then `console.error` |
| Linux: no player / missing file | Chinese `Err(String)` from the command |
| Browser-only invoke fail | localStorage toggle + WAV play |

## 5. Good / Base / Bad Cases

- **Good**: task enters review → `notification-center-deliver` → OS Glass/Asterisk/message sound while the window is focused.
- **Base**: toggle off → real deliveries silent; 试听 still plays.
- **Bad**: `AudioContext` oscillator from the deliver listener (silent in WKWebView).

## 6. Tests Required

- Rust path roundtrip: missing file, invalid JSON, missing field, save/load (`cargo test notification_sound`).
- macOS: `Glass.aiff` exists; volume constant is `"1"`.
- Frontend: delivery key, `parseNotificationSoundEnabled`, `shouldPlayNotificationSound`, startup/preference/coalesce gates, short WAV header (`notificationSound.test.ts`).
- Activity labels zh-CN + en in `locale.test.ts` / `utils.test.ts`.

## 7. Wrong vs Correct

#### Wrong
Listen to `notification-center-changed` (list refresh) and play Web Audio from that callback.

#### Correct
Listen to `notification-center-deliver` (same reasons as desktop toasts) and call `play_notification_sound_alert`. Do not change `notifications.rs` deliver protocol.

## Related

- Settings tab: `NotificationSettingsTab`; section key `notifications`.
- i18n action: `activity:actions.notification_sound_settings_updated`.
- Do not add a SQLite table or migration for this preference.
