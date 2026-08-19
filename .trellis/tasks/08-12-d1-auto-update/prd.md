# PRD · D1 应用自动更新

父任务:`08-12-product-gap-wave` · 优先级 N-P1

## Goal

已安装的桌面端能在应用内检查 GitHub Releases 上的新版本，用户确认后下载安装并提示重启，不必自己去 Releases 翻包。

## Background

- 仓库公开：`wenyuantc/codex-ai`。tag `v*` 已由 `.github/workflows/build.yml` 打三端安装包并挂 Release（当前最新 `v0.5.6`）。
- `Cargo.toml` 仅有 opener/sql/shell/dialog/notification；无 `tauri-plugin-updater` / `tauri-plugin-process`。`tauri.conf.json` 无 updater 段、无 `createUpdaterArtifacts`。
- 设置页「界面与运行」是系统级入口，尚无当前版本展示与检查更新。前端也未使用 `getVersion()`。
- 版本号由 `npm run bump-version` 同步 `package.json` / `Cargo.toml` / `tauri.conf.json`。
- `activity_logs.project_id` 可空；仪表盘只通过 `activity:actions.*` 显示中文。
- 当前 CI：Windows 只上传 MSI（脚本是 `tauri:windows:msi`）；macOS 只上传未签名 DMG；Linux 上传 AppImage/deb/rpm。均无 `.sig` / `latest.json`。
- `macos-latest` 现为 Apple Silicon；本任务不另开 Intel macOS 或 ARM Linux 矩阵。

## Requirements

1. 应用检查公开 Releases 上的 `latest.json`，校验更新签名后下载并安装当前平台包。
2. 设置 → 界面与运行 展示当前版本与「检查更新」。有新版时显示版本号和说明，用户确认后一键更新，下载进度可见；完成后提示重启，用户确认再 relaunch。
3. 无网络 / 已是最新 / 未配置签名 / 开发模式：给出可理解提示，不崩、不静默。
4. tag 发版在签名 secret 就绪时产出 updater 产物与 `latest.json` 并挂到 Release。README 说明所需 secrets。
5. 安装成功写活动日志 `app_update_installed`（`project_id` 可空），仪表盘 zh-CN / en 文案可见。

## Out of scope

- 启动时自动检查、后台静默下载、强制更新
- macOS Apple 代码签名 / 公证、Windows Authenticode
- Intel macOS、ARM Linux 额外构建
- 私有仓库或带鉴权的更新 endpoint
- 把预发布（`-alpha` / `-beta` / `-rc`）推给跟踪 GitHub `latest` 的正式用户
- 新表、新业务 Tauri 命令、更新 SSH 远端机器上的 CLI

## Acceptance

- [ ] 设置页可见当前版本 +「检查更新」；文案 i18n zh-CN / en
- [ ] 有新版时显示版本与说明；确认后可下载（进度可见）并提示重启
- [ ] 无网络 / 已最新 / 未配置签名 / 开发模式 有明确提示且应用不崩
- [ ] `Cargo.toml` 含 `tauri-plugin-updater` 与 `tauri-plugin-process`；capabilities 授权；组件不 raw `invoke`
- [ ] tag 工作流在签名 secret 存在时产出各平台 updater 产物与 `latest.json` 并挂到 Release
- [ ] README 写明 `TAURI_SIGNING_PRIVATE_KEY` 及可选 password
- [ ] 安装成功后仪表盘能看到「应用已更新」类中文活动

## Constraints

- 签名私钥不入库；公钥写入 `tauri.conf.json`（不能是文件路径）。
- 实施时生成新密钥对：公钥提交；私钥只交给维护者写入 GitHub secrets，不进 git。
