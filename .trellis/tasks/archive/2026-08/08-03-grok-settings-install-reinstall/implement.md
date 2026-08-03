# Implement: Grok CLI 安装/重装

## Ordered Checklist

### 1. 类型与契约

- [x] `src-tauri/src/db/models.rs`：新增 `GrokCliInstallResult`
- [x] `src/lib/types.ts`：镜像 `GrokCliInstallResult`

### 2. 本地安装后端

- [x] `src-tauri/src/grok/settings.rs`：
  - `install_grok_cli_runtime` + `#[tauri::command] install_grok_cli`
  - OS 分支执行官方脚本
  - 安装后 resolve + version
  - `insert_activity_log("grok_cli_installed", ...)`
  - override 冲突时 message 提示
- [x] `src-tauri/src/lib.rs`：注册 `install_grok_cli`

### 3. 远程安装后端

- [x] `src-tauri/src/app/remote.rs`：
  - `install_remote_grok_cli`（紧邻 `validate_remote_grok_health` / `install_remote_codex_sdk`）
  - 远端 curl 安装 + CLI_VERSION/CLI_PATH 输出
  - activity `remote_grok_cli_installed`
- [x] `lib.rs`：注册 `install_remote_grok_cli`

### 4. 前端 IPC 与 UI

- [x] `src/lib/backend.ts`：`installGrokCli` / `installRemoteGrokCli`
- [x] `src/lib/utils.ts`：activity 中文标签
- [x] `RuntimeSettingsTab.tsx`：Grok 安装按钮 + props
- [x] `SettingsPage.tsx`：`handleInstallGrokCli`、loading 类型扩展、接线

### 5. 验证

- [x] `npm run build`
- [x] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] 手工冒烟：本地安装文案切换；SSH 无配置阻断；有配置时安装路径（环境允许时）

## Validation Commands

```bash
npm run build
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## Risky Files / Rollback Points

| 文件 | 风险 | 回滚点 |
|------|------|--------|
| `grok/settings.rs` | 脚本执行与 PATH | 删除 install 函数与 command |
| `app/remote.rs` | SSH 脚本注入/引号 | 删除 remote install 命令 |
| `RuntimeSettingsTab.tsx` / `SettingsPage.tsx` | props 接线遗漏 | 去掉按钮与 handler |
| `lib.rs` | 注册遗漏导致前端 invoke 失败 | 补注册 |

## Notes for implementer

- 禁止前端 `invoke` 直调：只经 `backend.ts`。
- 错误字符串中文；远程 stderr 用现有 `redact_secret_text`。
- 安装 ≠ 登录；不要改 health 的 auth 判定逻辑。
- 参考实现：`install_codex_sdk_runtime`、`install_remote_codex_sdk`、`handleInstallSdk`。
- 跨层：activity key 前后端一致；先 `rg` 再改。
