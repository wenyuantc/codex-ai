# PRD · B3 启动时检查更新

父任务:`08-20-product-trust-ops` · 优先级 P1

## Goal

自动更新只在设置 → 关于里手动点。用户不进设置就不知道有新版。检查/安装能力已有，缺启动触达。

## 证据

- 手动检查：`src/components/settings/AboutUpdateSection.tsx:57-77`
- `src/lib/appUpdate.ts` `checkForAppUpdate` 包装 `check()`
- 无启动调用（`App.tsx` / `MainLayout` 无 updater）
- 发版 404 已修（`0fa8491`），本任务不改签名链路

## Requirements

1. 应用启动后（桌面 Tauri，非纯浏览器 dev）静默检查一次；失败不打断主路径。
2. 有新版本：通知中心或非模态条提示版本号，提供「去更新」跳到设置关于节。
3. 下载/安装仍需用户确认，禁止静默安装重启。
4. 开发模式沿用现有 `dev_mode` 错误，启动检查应跳过或静默忽略。
5. 同一版本不重复刷通知（去重）。

## Acceptance Criteria

- [ ] 打包运行启动后，有新版则无需打开设置也能看见
- [ ] 点击可到达现有检查/安装 UI
- [ ] 不自动安装；检查失败无阻塞 toast 轰炸
- [ ] i18n zh-CN + en；如写活动日志需中文 key

## Out of Scope

改 GitHub Release 产物命名、签名密钥、强制更新策略。
