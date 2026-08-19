# Implement · D1 应用自动更新

依赖：main 已含 C1。无迁移。实施前先本地生成 updater 密钥，公钥写入配置，私钥只交给维护者。

## Checklist

- [ ] `tauri signer generate`（或等价 CLI）：公钥写入 `tauri.conf.json`；私钥不入库
- [ ] `cargo add tauri-plugin-updater tauri-plugin-process`；`npm i @tauri-apps/plugin-updater @tauri-apps/plugin-process`
- [ ] `tauri.conf.json`：`createUpdaterArtifacts` + `plugins.updater.{pubkey,endpoints}`
- [ ] `lib.rs` 注册两插件；`capabilities/default.json` 加 `updater:default`、`process:allow-restart`
- [ ] `src/lib/appUpdate.ts`：`getAppVersion` / `checkForAppUpdate` / `downloadAndInstallUpdate` / `relaunchApp` + `mapUpdaterError`；Vitest 覆盖错误映射
- [ ] `scripts/build-latest-json.mjs`：从三端产物扫 `.sig` 与更新包，写出 `latest.json`；缺平台则省略该 key
- [ ] `build.yml`：签名 env；Windows 改 `tauri:windows`；上传 updater 产物；tag 汇总 `latest.json` 并挂 Release；dispatch 无 secret 时关闭 updater artifacts
- [ ] `AboutUpdateSection`：当前版本、检查、新版说明、进度、安装后确认重启；挂到 `RuntimeSettingsTab` 顶部
- [ ] i18n：`settings`（关于与更新）+ `activity.actions.app_update_installed`（zh-CN / en）
- [ ] README：发版 secrets 与「设置里检查更新」一句；`TASK.md` 勾选 D1
- [ ] 安装成功调 `logActivity`；不新增 Rust command / 表

## Validation

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run test:ci
npm run format:check
npm run build
```

手工（`npm run tauri:dev`）：

1. 设置 → 界面与运行顶部能看到当前版本；点检查：开发模式有明确提示，应用不崩。
2. 切 en：关于/更新与活动键文案在。
3. 无法在 dev 里走完真实下载。签名 secret 配好后，用比当前版本更高的 tag 打一版，再在已安装包上检查→下载进度→安装→确认重启。
4. 断网再检查：网络错误提示。
5. SSH 远程模式下同一设置页仍显示该卡片。

## Risky files

- `build.yml`：保留「dispatch 只构建、tag 才发版」；不要把 MSI 从 Release 拿掉。
- `RuntimeSettingsTab.tsx`：只嵌入新组件，不改引擎表单 props。
- `tauri.conf.json` pubkey：贴字符串，不要写路径。
- 密钥：生成输出里的私钥绝不能进 commit / 对话记录之外的仓库文件。

## Rollback

revert 本功能提交。已发布 Release 上的 `latest.json` 可手动删。轮换密钥必须同时换公钥。
