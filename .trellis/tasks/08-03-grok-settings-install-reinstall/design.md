# Design: Grok CLI 本地 + 远程安装/重装

## Architecture / Boundaries

```text
SettingsPage (handleInstallGrokCli)
  → isRemoteMode && sshConfigId
      ? installRemoteGrokCli(sshConfigId)   // backend.ts
      : installGrokCli()
  → Tauri commands
      install_grok_cli            // grok/settings.rs
      install_remote_grok_cli     // app/remote.rs（与 install_remote_codex_sdk 并列）
  → activity_logs + 返回 GrokCliInstallResult
  → 前端 loadGrok/runtime health 刷新
```

**边界**

| 层 | 负责 | 不负责 |
|----|------|--------|
| UI | 按钮文案、loading、错误展示、模式切换 | 直接执行 shell |
| `backend.ts` | invoke 包装 | 业务逻辑 |
| `grok/settings.rs` | 本地安装脚本、安装后 resolve+version | SSH |
| `app/remote.rs` | SSH 上跑官方安装 + 结果解析 + 活动日志 | 本地进程 |
| 会话/process | 无变更 | — |

## Contracts

### `GrokCliInstallResult`（Rust + TS 镜像）

```rust
struct GrokCliInstallResult {
    execution_target: String,          // "local" | "ssh"
    ssh_config_id: Option<String>,
    target_host_label: Option<String>,
    cli_available: bool,
    cli_version: Option<String>,
    cli_path: Option<String>,
    message: String,
}
```

与 `CodexSdkInstallResult` 同构思想，字段语义改为 CLI。

### Commands

| Command | 入参 | 成功条件 |
|---------|------|----------|
| `install_grok_cli` | 无 | 官方安装脚本 exit 0，且 resolve 后 `grok --version` 成功（或脚本成功但 override 冲突时 `cli_available` 仍反映 resolve 结果，message 说明） |
| `install_remote_grok_cli` | `ssh_config_id: String` | 远端脚本成功且 stdout 可解析版本（或 `command -v grok` 成功） |

### 本地安装伪流程

1. 根据 `std::env::consts::OS`：
   - `windows`：`powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://x.ai/cli/install.ps1 | iex"`
   - 其他：`bash -lc 'curl -fsSL https://x.ai/cli/install.sh | bash'`（或等价非交互调用）
2. 捕获 stdout/stderr；失败 → `Err(中文 + 脱敏详情)`
3. `load_grok_settings` → `resolve_grok_executable_path` → `read_grok_cli_version`
4. `insert_activity_log(..., "grok_cli_installed", ...)`
5. 返回 `GrokCliInstallResult { execution_target: local, ... }`

### 远程安装伪流程（对齐 `install_remote_codex_sdk`）

1. `fetch_ssh_config_record_by_id`
2. remote_script 近似：

```sh
curl -fsSL https://x.ai/cli/install.sh | bash
if command -v grok >/dev/null 2>&1; then
  printf 'CLI_VERSION=%s\n' "$(grok --version 2>/dev/null | head -n 1)"
  printf 'CLI_PATH=%s\n' "$(command -v grok)"
else
  # 常见默认路径兜底
  for p in "$HOME/.grok/bin/grok" /usr/local/bin/grok; do
    if [ -x "$p" ]; then
      printf 'CLI_VERSION=%s\n' "$($p --version 2>/dev/null | head -n 1)"
      printf 'CLI_PATH=%s\n' "$p"
      exit 0
    fi
  done
  echo 'CLI_MISSING=1'; exit 1
fi
```

3. `execute_ssh_command(..., true)`；失败错误用 `redact_secret_text`
4. `parse_remote_key_value_output` 取 `CLI_VERSION` / `CLI_PATH`
5. `insert_activity_log(..., "remote_grok_cli_installed", host_label, ...)`
6. 返回 result

## Frontend Changes

| 文件 | 改动 |
|------|------|
| `RuntimeSettingsTab.tsx` | Grok 区增加 install 按钮；文案随 local/remote health；props：`onGrokInstall`，loading 含 `"install"` |
| `SettingsPage.tsx` | `handleInstallGrokCli` 镜像 `handleInstallSdk` 的 remote/local 分支；loading/message/error state |
| `backend.ts` | `installGrokCli` / `installRemoteGrokCli` |
| `types.ts` | `GrokCliInstallResult` |
| `utils.ts` | activity 中文标签 |

**按钮可用性**

- 本地：`!healthLoading && grokActionLoading === null`
- SSH：还需 `selectedSshConfigId`；无配置时 disabled + 错误提示

## Compatibility

- **SSH 模型**：远程安装必做；远程 health 复用 `validate_remote_grok_health`。
- **DB migration**：无 schema 变更；仅 activity action 字符串。
- **Windows 本地**：官方 ps1；**远程** 仅 Unix shell（与现有 remote 假设一致）。
- **登录**：安装后不登录；UI 文案可轻提示「安装后需 `grok login`」。

## Trade-offs

| 方案 | 取舍 |
|------|------|
| 官方 install 脚本 vs 自托管二进制 | 选官方：与文档一致、维护成本低；依赖外网与脚本稳定性 |
| 远程写在 `remote.rs` vs `grok/settings.rs` | 选 `remote.rs`：与 `install_remote_codex_sdk` / `validate_remote_grok_health` 同层，SSH 工具已在此 |
| 结果类型复用 CodexSdkInstallResult | 不复用：避免语义混淆；新建 `GrokCliInstallResult` |

## Rollback

- 功能纯增量：删除 command 注册 + UI 按钮即可回退。
- 已安装的 CLI 不由本功能卸载；回滚不移除用户机器上的 `grok`。

## Test Plan (design-level)

- 单元：无强制；可选解析远端 `CLI_VERSION=` 行（若抽出纯函数）。
- 手工：本地安装/重装；SSH 目标安装；失败断网提示；未选 SSH 阻断。
- 门禁：`npm run build` + clippy `-D warnings`。
