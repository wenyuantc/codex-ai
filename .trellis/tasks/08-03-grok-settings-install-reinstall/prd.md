# Grok 设置页支持安装与重装 CLI（对齐 Codex）

## Goal

在「设置 → 运行时」的 Grok 配置区提供与 Codex 同类的 **安装 / 重装** 操作：用户可在应用内安装或升级 **本地** 与 **当前 SSH 目标远端** 的 xAI Grok Build CLI，成功后刷新健康状态；无需离开应用手动跑安装命令。

## Background

### 产品现状（代码已验证）

1. **Codex**：`RuntimeSettingsTab`「安装 SDK / 重装 SDK」；本地 `install_codex_sdk`，SSH `install_remote_codex_sdk`（`SettingsPage.handleInstallSdk`）。
2. **Claude / OpenCode**：仅本地安装；Grok 此前连本地安装都没有。
3. **Grok 缺口**：
   - UI：`grokActionLoading: "save" | null`，无 install 按钮与 handler。
   - 后端：`src-tauri/src/grok/settings.rs` 有 settings/health/models；**无** install 命令。
   - 远端：`validate_remote_grok_health` 已存在（`app/remote.rs`），安装后可复用探测。
4. Grok 依赖 **官方 CLI 二进制**（非 npm SDK）；路径：`cli_path_override` → `GROK_CLI_PATH`/`GROK_PATH` → `~/.grok/bin` → PATH。
5. 官方安装：Unix `curl -fsSL https://x.ai/cli/install.sh | bash`；Windows `irm https://x.ai/cli/install.ps1 | iex`。

### 已锁定产品决策

| ID | 决策 | 选择 |
|----|------|------|
| Q1 | 安装范围 | **B：本地 + 当前 SSH 目标远程**（对齐 Codex） |
| Q2 | 按钮文案 | **安装 CLI / 重装 CLI**（Grok 实际为 CLI，非 SDK） |

## Requirements

### R1 — UI（设置页 Grok 区）

- 在「刷新检测」旁增加安装按钮：
  - 当前上下文未安装：**安装 CLI**
  - 当前上下文已安装：**重装 CLI**
- 判定：
  - 本地模式：`grokHealth?.cli_available`
  - SSH 模式：`remoteGrokHealth?.available`（需已选 SSH 配置）
- SSH 模式未选配置：禁用安装并提示先选 SSH（对齐 Codex）。
- `grokActionLoading` 扩展为 `"save" | "install" | null`；安装中禁用冲突操作并显示 loading。
- 成功/失败中文消息；成功后刷新 local/remote health 与模型列表（与现有 Grok 刷新路径一致）。

### R2 — 本地安装 / 重装

- 新增 Tauri 命令 `install_grok_cli`（本地）。
- 按主机 OS 调用官方安装脚本；安装与重装共用同一入口（脚本幂等升级）。
- 安装成功后探测 `grok --version`（经现有 resolve 路径），返回版本与路径。
- **不**自动 `grok login`；登录态仍由 health 展示。
- `cli_path_override` 策略：安装始终写入官方默认位置；若 override 指向无效路径，返回成功消息中附带提示「已安装到默认位置，但当前覆盖路径不可用，请清空或修正 CLI 路径覆盖」。

### R3 — 远程安装 / 重装（SSH）

- 新增 Tauri 命令 `install_remote_grok_cli(ssh_config_id)`。
- 通过既有 `execute_ssh_command` + `build_remote_shell_command` 在远端执行官方 Unix 安装脚本（远程目标按 Linux/macOS shell 假设，与现有 SSH 脚本一致）。
- 安装后在远端执行 `grok --version`（或等价探测）确认可用；失败返回中文错误（stderr 做脱敏，对齐 remote Codex）。
- 成功写入 activity log（见 R4）。

### R4 — 活动日志与中文标签

| action key | 中文标签 |
|------------|----------|
| `grok_cli_installed` | 安装 Grok CLI |
| `remote_grok_cli_installed` | 远程安装 Grok CLI |

- 写入 `insert_activity_log`；前端 `getActivityActionLabel()` 补齐。

### R5 — 契约与注册

- `db/models.rs` + 前端 `types` 增加 `GrokCliInstallResult`（字段对齐 Codex 结果形态中的可复用部分：`execution_target`、`ssh_config_id`、`target_host_label`、`cli_available`/`cli_version`/`cli_path`、`message`）。
- `src/lib/backend.ts` 封装 invoke；`lib.rs` 注册两个 command。
- 不改会话启动 / stream 协议。

## Acceptance Criteria

- [ ] AC1：本地未装 CLI 时显示「安装 CLI」，安装成功后 health 为可用且展示版本（若可读）。
- [ ] AC2：本地已装时显示「重装 CLI」，重装后 health 刷新成功。
- [ ] AC3：SSH 模式选中配置后可远程安装/重装；成功后 `remoteGrokHealth.available === true`（或版本可显示）。
- [ ] AC4：SSH 未选配置时安装被阻断并有中文提示。
- [ ] AC5：安装失败 UI 显示中文错误，loading 正确结束，应用不崩溃。
- [ ] AC6：本地/远程安装成功均写入最近活动，仪表盘中文标签正确。
- [ ] AC7：安装不自动登录；未登录时 health 仍可提示执行 `grok login`。
- [ ] AC8：`npm run build` 与 `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 通过。

## Out of Scope

- 自动 `grok login` / API Key 托管。
- 卸载 CLI。
- 把 Grok 改成 npm SDK 执行模式。
- 修改 Grok 会话/stream/任务自动化逻辑。
- 远端 Windows SSH 专用安装脚本（远端按 Unix shell；本地 Windows 用官方 ps1）。

## Risks

- `curl | bash` 需网络与非交互环境；沙箱/企业代理可能导致失败，错误需透传。
- 安装后 shell PATH 与 GUI 应用环境可能不一致；依赖现有 `~/.grok/bin` / resolve 逻辑。
- 官方脚本变更时可能破坏解析；安装后以 `grok --version` 成功与否为验收真值。
