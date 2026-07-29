# SSH 连接复用与密钥传递收敛

父任务：`.trellis/tasks/07-29-codex-ai-optimization`
覆盖父任务源发现：**#1（SSH 无连接复用）**、**#7（密码经环境变量传递）**

## Goal

消除远程项目每次 SSH 操作都要重新 TCP 握手 + 认证的开销，并收敛 SSH 密码的传递通道。

## Background

`src-tauri/src/app/remote.rs:223` 的 `build_ssh_command` 是全仓库**唯一**的 SSH 命令构建入口——约 20 个调用点（`git_runtime.rs`、`app/review.rs`、`codex/process/*`、`claude/`、`grok/`）全部经它构建。改这一个函数即可全局生效，这是本任务可控性的基础。

当前该函数不设置任何连接复用参数，因此每个 git status / branch / diff / 文件传输都是一次完整的 TCP 握手 + 密钥或密码认证。

## 前置条件

无。本任务不依赖父任务下的其他子任务，可作为第一个执行的子任务。

## Requirements

### R1 — 短命令启用连接复用（Unix）

在 macOS / Linux 上为 SSH 命令启用 `ControlMaster` / `ControlPath` / `ControlPersist`，使短命令复用同一条底层连接。

### R2 — Windows 显式禁用复用

Win32-OpenSSH **不支持**连接复用：多路复用依赖 Unix domain socket，原生 Windows OpenSSH 无法创建该类型 socket，客户端会直接失败（`muxclient socket(): Unknown error` / `getsockname failed: Not a socket`）。

关键点：**省略我方标志并不足够**。若用户 `~/.ssh/config` 中存在 `Host *` 的 ControlMaster 配置，`ssh.exe` 仍会尝试复用并失败。因此 Windows 上必须显式下发 `-o ControlMaster=no -o ControlPath=none` 覆盖用户配置（命令行 `-o` 优先级高于配置文件）。

这一项不是「Windows 无收益的兼容处理」——它修复了一个既有隐患：当前代码在用户 ssh config 开启复用时，Windows 远程功能会失败。

### R3 — 长连接会话不参与复用

`codex/process/session_launch.rs:37` 是唯一 `allocate_tty` 可变的调用点，承载长期运行的 AI CLI 交互会话；其余调用点均传 `false`。

长会话必须走独立连接（`ControlMaster=no`）：若长会话依附于共享 master，master 退出会连带断开所有会话；若长会话自身成为 master，其超长生命周期又会把短命令绑死在一条连接上。

### R4 — master 连接生命周期管理

`ControlPersist` 会留下后台 master 进程。需要：
- `ControlPath` 使用 `%C`（连接参数哈希，实测产出 40 字符 hex）而非 `%r@%h:%p`，规避 Unix domain socket 路径长度上限（macOS ~104 字节）；
- 应用退出时清理残留 master（现有钩子：`lib.rs:62` `on_window_event`）；
- `known_hosts_mode = "off"` 时当前会下发 `UserKnownHostsFile=/dev/null`，需验证与复用共存无副作用。

### R5 — 密码传递通道处置

**现状更正**：`CODEX_SSH_SECRET` 环境变量**不是冗余，无法直接删除**。`create_askpass_script`（`remote.rs:198`）第 219 行为 `let _ = secret;`——显式丢弃传入的密码参数，脚本体是 `printf '%s' "$CODEX_SSH_SECRET"`，只能从环境变量取值。

处置方案见 `design.md` 的权衡决策。本 PRD 约束结论必须满足：
- 不得引入密码**长期落盘**；
- 不得因该项改动破坏 Windows（无 `mkfifo`）；
- 若最终判定为「缓解而非消除」，必须在 `design.md` 中写明威胁模型与理由，并记录备选方案。

## Acceptance Criteria

- [ ] Unix 平台下，同一 SSH 配置的连续短命令复用同一底层连接（验证：`ControlPath` socket 文件存在，且第二条命令耗时显著低于首条）
- [ ] Windows 平台下显式下发 `ControlMaster=no` + `ControlPath=none`，且在用户 ssh config 含 `Host *` 复用配置时远程功能仍正常
- [ ] `session_launch.rs` 的 `allocate_tty=true` 长会话走独立连接，不受 master 生命周期影响
- [ ] 应用退出后无残留 master 进程与 socket 文件
- [ ] `known_hosts_mode` 三种取值（`off` / `strict` / 默认 `accept-new`）在复用开启后行为不变
- [ ] 密码认证路径（`auth_type == "password"`）功能不回归；`password_probe_status` 校验逻辑不变
- [ ] 新增单元测试覆盖 `build_ssh_command` 的参数生成：Unix/Windows 分支 × 长短连接 × 三种 known_hosts 模式
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` 全绿，测试数不低于基线
- [ ] 实机验证：SSH 远程项目可正常创建会话并执行任务

## Out of Scope

- 不改 `ssh_configs` 表的既有列语义（如需新列走新迁移）
- 不引入第三方 SSH 库（`russh` 等）替换 `ssh` 子进程模型
- 不改动 `build_remote_shell_command` 的远程命令拼装逻辑
- 不处理独立 scp 路径（当前实现走 `execute_ssh_command_with_input` 的 stdin 通道，无独立 scp 调用）

## Notes

- 交付方式：独立分支 + 单独提交（父任务约束）
- 本任务改动集中于 `src-tauri/src/app/remote.rs`，预期不触及前端
