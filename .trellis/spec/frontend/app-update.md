# App Auto-Update

> In-app check/install of GitHub Release updates via official Tauri updater plugins. No business command, no migration.

## 1. Scope / Trigger

Use this spec when changing `src/lib/appUpdate.ts`, `AboutUpdateSection`, updater/process plugin registration, `tauri.conf.json` updater keys, `scripts/build-latest-json.mjs`, or `.github/workflows/build.yml` signing / updater artifacts.

This is local desktop only. SSH remote projects do not change the card; do not try to update remote CLIs.

## 2. Signatures

```ts
// src/lib/appUpdate.ts — components import this, never the plugin modules
getAppVersion(): Promise<string>
checkForAppUpdate(): Promise<AppUpdateInfo | null> // null = already latest
downloadAndInstallUpdate(info, onProgress?): Promise<void>
relaunchApp(): Promise<void>
mapUpdaterError(error): "network" | "already_latest" | "signature" | "dev_mode" | "cancelled" | "unknown"
```

`AppUpdateInfo`: `{ version, currentVersion, notes, pubDate }`. The live plugin `Update` handle is stored in a `WeakMap`, not on the info object.

Rust: register `tauri_plugin_updater::Builder::new().build()` and `tauri_plugin_process::init()` in `lib.rs`. Capabilities: `updater:default`, `process:allow-restart`.

No new `#[tauri::command]`. Install success logs via existing `logActivity({ action: "app_update_installed", details: "<old> → <new>" })`.

## 3. Contracts

- Endpoint: `https://github.com/wenyuantc/codex-ai/releases/latest/download/latest.json`
- `plugins.updater.pubkey` is the minisign public key **string** (not a path). Private key is only `TAURI_SIGNING_PRIVATE_KEY` (+ optional password) in GitHub secrets / local env.
- `latest.json` platforms we actually build: `darwin-aarch64` (`.app.tar.gz` + `.sig`), `linux-x86_64` (`.AppImage` + `.sig`), `windows-x86_64` (`.nsis.zip` or `-setup.exe` + `.sig`).
- GitHub Release replaces spaces in uploaded asset names with `.`. `build-latest-json.mjs` must emit download URLs with that sanitized name (`Codex.AI.System.app.tar.gz`), not `%20`. Local bundle filenames may still contain spaces.
- macOS updater artifacts require `--bundles app` (or `app,dmg`). `--bundles dmg` alone does **not** emit `.app.tar.gz`.
- `tauri build --no-sign` also skips updater signatures. Tag CI must `tauri signer sign *.app.tar.gz` after the unsigned macOS build when the private key is present.
- Tag push without `TAURI_SIGNING_PRIVATE_KEY` fails. `workflow_dispatch` without the secret passes `--config '{"bundle":{"createUpdaterArtifacts":false}}'` and still ships installers.
- Dev / non-Tauri (`import.meta.env.DEV` or `!isTauri()`): `checkForAppUpdate` throws a development-mode error; do not call the plugin.
- UI: Settings → 界面与运行 → `AboutUpdateSection`. Check is manual. Install is a second click. Relaunch is a third click after success. `pubDate` uses `formatDate()`.
- Startup: packaged Tauri app silently calls `checkForAppUpdateOnStartup()` once. If a new version exists and was not dismissed (`codex-ai:dismissed-update-version`), show `StartupUpdateBanner`. Never auto-download/install. Dev/browser returns null with no throw.

## 4. Validation & Error Matrix

| Condition | Behavior |
|---|---|
| Dev / browser Vite | i18n `about.errors.devMode`; no crash |
| No network / bad latest.json | `about.errors.network` |
| Already latest / `check()` null | up-to-date copy; no activity log |
| Missing/invalid signature | `about.errors.signature`; do not install |
| User cancel | `about.errors.cancelled` |
| Other plugin errors | `about.errors.unknown` with `{{detail}}` |
| Install succeeded | `app_update_installed`; prompt restart |
| Activity log write failed | console error; still show installed + restart |

## 5. Good / Base / Bad Cases

- **Good**: signed tag Release has `latest.json` + matching platform url/sig; installed app checks, downloads with progress, logs activity, relaunches on confirm.
- **Base**: already latest; card shows 当前已是最新版本, no log.
- **Bad**: `--bundles dmg` only (no darwin updater). `--no-sign` without a later `signer sign` (latest.json drops macOS). Empty `extra[@]` under macOS bash 3.2 + `set -u`. Component `invoke` / import of `@tauri-apps/plugin-updater`. Silent auto-install on startup. `latest.json` urls with `Codex%20AI%20System` while GitHub assets are `Codex.AI.System` (download 404).

## 6. Tests Required

- `shouldPromptStartupUpdate` / dismissed version key
- `mapUpdaterError`: network, already latest, signature, dev mode, cancel, unknown
- `updaterErrorI18nKey` points at `settings:about.errors.*`
- `getActivityActionLabel("app_update_installed")` zh-CN + en
- `build-latest-json.mjs`: spaces in local artifact names become `.` in platform urls; names without spaces stay unchanged
- Do not add a fake plugin e2e in Vitest. Real download is a signed installed build only.

## 7. Wrong vs Correct

#### Wrong
```ts
import { check } from "@tauri-apps/plugin-updater"; // in a React component
await check(); // inside SettingsPage useEffect on mount
await downloadAndInstallUpdate(info); // on startup with no confirm
```

macOS CI: `tauri build --bundles dmg --no-sign` and assume `.app.tar.gz.sig` exists.

#### Correct
Wrap plugins in `appUpdate.ts`. Manual check button plus silent startup check that only shows a banner. macOS CI: `--bundles app,dmg --no-sign`, then `tauri signer sign` on `*.app.tar.gz` when the private key exists.
