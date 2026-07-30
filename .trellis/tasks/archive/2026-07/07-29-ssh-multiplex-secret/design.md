# 技术设计 — SSH 连接复用与密钥传递收敛

## 边界

**唯一改动入口**：`src-tauri/src/app/remote.rs::build_ssh_command`（第 223 行）。

约 20 个 SSH 调用点全部经此构建命令，因此复用能力在此一处注入即全局生效。调用方签名不变，无需改动任何调用点。

新增改动点仅两处：
- `src/tray.rs:51` `"quit"` 分支 —— 退出前清理 master
- `src/app/remote.rs` 新增 `#[cfg(test)]` 模块（该文件目前无测试）

## 关键约束（已验证）

| 约束 | 验证方式 | 结论 |
|---|---|---|
| Windows 不支持连接复用 | Win32-OpenSSH #1328 / #405；vscode-remote-release #96 | 依赖 Unix domain socket，`ssh.exe` 直接失败 |
| 省略标志不足以在 Windows 规避 | 同上 | 用户 `~/.ssh/config` 的 `Host *` 仍会触发；须显式 `-o ControlMaster=no -o ControlPath=none` |
| `%C` 路径长度可控 | 本机实测 `ssh -G` | 产出 40 字符 hex，`/tmp/<prefix>-<40hex>` 远低于 macOS ~104 字节上限 |
| 唯一长连接调用点 | grep `build_ssh_command` 全调用点 | 仅 `session_launch.rs:37` 的 `allocate_tty` 可变，其余硬编码 `false` |
| 退出钩子位置 | 读 `window_event.rs` | `CloseRequested` 是 close-to-tray（`prevent_close`+`hide`），**不是**退出点；真退出在 `tray.rs:51` 的 `app.exit(0)` |

## 设计

### D1 复用参数注入

在 `build_ssh_command` 内按平台与连接类型分支：

```
Unix   + 短命令(allocate_tty=false) → ControlMaster=auto
                                       ControlPath=<tmp>/codex-ai-ssh-mux/cm-%C
                                       ControlPersist=<N>s
Unix   + 长会话(allocate_tty=true)  → ControlMaster=no
Windows(任意)                        → ControlMaster=no + ControlPath=none
```

- `ControlPath` 目录复用现有惯例（`create_askpass_script` 已在 `std::env::temp_dir()` 下建专用子目录），新建 `codex-ai-ssh-mux/`，Unix 下设 0700。
- `ControlPersist` 取值需权衡：过短失去复用意义，过长残留连接多。**默认 60s**，作为常量而非配置项（PRD 未要求可配置，避免扩大范围）。

### D2 长会话隔离（R3）

`allocate_tty` 参数已存在于签名中，直接复用它区分长短连接，无需新增参数——这是本设计能保持调用点零改动的关键。

长会话显式下发 `ControlMaster=no` 而非「不下发」：与 R2 同理，需覆盖用户 ssh config 中可能存在的 `Host *` 复用配置，否则长会话会意外挂到共享 master 上，master 退出即断连。

### D3 master 生命周期（R4）

- 清理时机：`tray.rs:51` `"quit"` 分支，在 `app.exit(0)` 之前。
- 清理方式：对 `codex-ai-ssh-mux/` 下每个 socket 执行 `ssh -O exit -o ControlPath=<socket> <dummy-host>` 优雅退出；失败则直接删除 socket 文件兜底。
- 幂等：目录不存在或为空时静默返回，不阻塞退出流程（参照 `window_state::save_window_size` 的 `eprintln!` 容错惯例）。

### D4 known_hosts 与复用的交互（R4）

`known_hosts_mode = "off"` 时现有代码下发 `UserKnownHostsFile=/dev/null`。

`%C` 的哈希输入包含 host/port/user 等连接参数，**不含** `UserKnownHostsFile`。因此三种 known_hosts 模式共享同一 ControlPath——若同一主机曾以不同模式连接，第二次会直接复用首次建立的 master，**跳过** known_hosts 校验。

处理：将 `known_hosts_mode` 拼入 ControlPath 前缀（`cm-<mode>-%C`），使不同模式落到不同 socket，保持校验语义不变。这是本设计中最容易被忽略的正确性点。

## R5 权衡决策：密码传递采用「缓解」而非「消除」

### 决策

**保留 `CODEX_SSH_SECRET` 环境变量通道**，通过 D1 的连接复用间接缩短暴露窗口。FIFO 方案记录为备选，不在本任务实施。

### 论证

关键事实（`codex/secret_store.rs` 实测）：SSH 密码以**明文 JSON** 存储于 `$APPCONFIG/ssh-secrets.json`，非 OS keychain。Unix 下权限 0600；Windows 下 `tighten_secret_store_permissions` 是 `#[cfg(not(unix))] let _ = path;` ——**不做任何权限收紧**。

由此：
1. **不构成新增攻击面**。能读 `/proc/<pid>/environ` 的攻击者需同用户权限，而同样的权限可直接读取 0600 的 `ssh-secrets.json`。env 通道并未降低既有安全水位。
2. **复用带来实质缓解**。启用 ControlMaster 后，密码认证从「每次 SSH 操作」降为「每个 master 生命周期一次」，暴露窗口随操作频次线性下降——这是 R1 的免费安全收益。
3. **FIFO 的成本收益不划算**。Unix 可行（`mkfifo` + 一次性消费），但 Windows 无对应机制，需维护两套通道；同时引入阻塞、超时、清理三类新失败模式。在攻击面未实际扩大的前提下，风险收益比不成立。
4. **PRD R5 约束满足**：不引入长期落盘、不破坏 Windows、已书面记录理由与备选。

### 备选方案（不实施，供未来参考）

| 方案 | 优点 | 弃用理由 |
|---|---|---|
| FIFO（Unix） | 密码不落盘不进 env | Windows 需另一套；新增 3 类失败模式 |
| 密码内嵌 askpass 脚本 | 消除 env | 密码落盘，违反 PRD R5 约束 |
| stdin 传 helper | 通道干净 | `SSH_ASKPASS` 协议不支持，须自造 helper 二进制 |

### 衍生发现（超出本任务范围，已上报父任务）

`ssh-secrets.json` 明文存储密码、且 Windows 无权限保护——这比原发现 #7 严重。应迁移至 OS keychain（`keyring` crate）。**不在 C1 范围内处理**，作为新增候选子任务记录到父任务 PRD。

## 兼容性与回滚

- **无数据库变更**，无迁移，无 schema 影响。
- **无 IPC 契约变更**，前端零改动。
- 回滚 = 还原 `build_ssh_command` 的参数注入 + `tray.rs` 的清理调用，两处均为纯增量代码，`git revert` 单个提交即可完整回退。
- 降级路径：若复用在某环境异常，将 D1 的平台分支强制走 Windows 分支（全局 `ControlMaster=no`）即回到当前行为。

## 测试策略

`remote.rs` 当前**无测试模块**，需新建 `#[cfg(test)]`。

`build_ssh_command` 依赖 `AppHandle<R>`（为解析 secret）与 `new_ssh_command()`（解析 ssh 可执行路径），直接单测困难。**方案**：抽出纯函数 `fn multiplex_args(allocate_tty: bool, known_hosts_mode: &str) -> Vec<String>`，对其做表驱动测试，`build_ssh_command` 仅负责调用与拼接。

覆盖矩阵：Unix/Windows × 长/短连接 × 三种 known_hosts 模式 = 12 组，断言参数序列。
