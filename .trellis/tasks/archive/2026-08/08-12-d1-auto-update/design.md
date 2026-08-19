# Design · D1 应用自动更新

## Boundaries

- 无新表、无新业务 command。更新走官方插件 JS API；活动走已有 `logActivity`。
- 插件调用收在 `src/lib/appUpdate.ts`，组件不直接 import `@tauri-apps/plugin-updater`。
- UI 放设置「界面与运行」顶部新卡片 `AboutUpdateSection`，不新增 Settings tab，不把逻辑堆进 `RuntimeSettingsTab` 的 props。
- SSH：更新的是本机桌面端，与远程项目无关；设置页本地/远程模式都显示同一块。

## Data flow

```
设置页 AboutUpdateSection
  → getVersion() 展示当前版本
  → appUpdate.checkForAppUpdate()
       → plugin check() 读 tauri.conf endpoints + pubkey
       → GET https://github.com/wenyuantc/codex-ai/releases/latest/download/latest.json
  → 无更新 / 错误：只更新本卡片状态
  → 有更新：展示 version + notes；用户点「更新」
  → downloadAndInstall(progress)
  → logActivity({ action: "app_update_installed", details })
  → 提示重启；用户确认 → process.relaunch()
```

开发态（`tauri dev`）或插件拒绝：映射为「开发模式无法检查更新」，不抛未捕获异常。

## Contracts

**配置**（`tauri.conf.json`）

- `bundle.createUpdaterArtifacts: true`
- `plugins.updater.pubkey`: minisign 公钥字符串
- `plugins.updater.endpoints`: `["https://github.com/wenyuantc/codex-ai/releases/latest/download/latest.json"]`

**插件注册**：`lib.rs` desktop 注册 `tauri_plugin_updater` + `tauri_plugin_process`。capabilities：`updater:default`、`process:allow-restart`。

**latest.json**（`scripts/build-latest-json.mjs` 由 CI 在三端产物齐后生成）

| 字段 | 来源 |
|---|---|
| `version` | tag 去 `v` 前缀，与 `tauri.conf.json` 版本对齐 |
| `notes` | Release 说明摘录或 tag 名 |
| `pub_date` | ISO-8601 |
| `platforms` | 只收录本轮实际打出的包 |

平台键与现有 CI 对齐：`darwin-aarch64`（`.app.tar.gz` + `.sig`）、`linux-x86_64`（AppImage 更新包 + `.sig`）、`windows-x86_64`（NSIS `.nsis.zip` + `.sig`）。不编造未构建的 `darwin-x86_64`。

**CI**

- 构建步注入 `TAURI_SIGNING_PRIVATE_KEY` / 可选 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。
- Windows 改为 `npm run tauri:windows`（NSIS+MSI）：MSI 继续给人装，NSIS 给 updater。
- 上传 `.sig`、`.app.tar.gz`、`.nsis.zip`；`publish-release` 挂上它们和 `latest.json`。
- tag 且缺签名 secret → 失败。`workflow_dispatch` 且无 secret → `--config` 关掉 `createUpdaterArtifacts`，行为与现在「只打安装包」一致。

**前端错误映射**（纯函数，Vitest）：网络、已最新、签名缺失/校验失败、开发模式、用户取消。`pub_date` 若展示则走 `formatDate()`。

**活动**：仅安装成功写 `app_update_installed`；details 含旧版本→新版本。检查/失败不写日志。

## Tradeoffs

- 手动检查而非启动自动检查：避免开发态和无 secret 的包反复弹错；符合 PRD。
- 插件 API 而非自研 command：签名校验与下载进度已由官方实现。
- 独立 `AboutUpdateSection`：避免继续膨胀 `RuntimeSettingsTab` 的 props。
- GitHub `/releases/latest`：正式用户拿不到 prerelease，与现有 `prerelease:` 标记一致。

## Rollback

revert 功能提交。已发出的 `latest.json` 不会回写旧客户端；去掉 endpoint/pubkey 后旧客户端检查失败（有提示）。私钥轮换必须同步换公钥并重签。
