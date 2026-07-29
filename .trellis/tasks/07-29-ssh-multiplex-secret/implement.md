# 执行计划 — SSH 连接复用与密钥传递收敛

前置：`prd.md` / `design.md` 已评审通过，`task.py start` 已执行（status = in_progress）。

分支：`feat/ssh-multiplex`（父任务约束：每子任务独立分支）

## 步骤

### S1 建立分支与基线 `[无回滚点]`

```bash
git checkout -b feat/ssh-multiplex
cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5
```

记录基线测试数（预期 246）。后续任何步骤不得使该数字下降。

### S2 抽出纯函数 `multiplex_args` `[回滚点 R1]`

文件：`src-tauri/src/app/remote.rs`

新增纯函数，签名见 `design.md` 测试策略节：

```rust
fn multiplex_args(allocate_tty: bool, known_hosts_mode: &str) -> Vec<String>
```

规则（对应 design D1/D2/D4）：
- `cfg!(windows)` → `["-o", "ControlMaster=no", "-o", "ControlPath=none"]`
- `allocate_tty == true` → `["-o", "ControlMaster=no"]`
- 否则 → `ControlMaster=auto` + `ControlPath=<tmpdir>/codex-ai-ssh-mux/cm-<mode>-%C` + `ControlPersist=60s`

注意 D4：`known_hosts_mode` 必须拼进 ControlPath 前缀，否则不同校验模式会复用同一 master 而跳过 known_hosts 校验。

此步骤**只加函数，不接线**，编译应通过（允许 dead_code 警告）。

**验证**：`cargo build --manifest-path src-tauri/Cargo.toml`

### S3 表驱动测试 `[回滚点 R2]`

在 `remote.rs` 新建 `#[cfg(test)] mod tests`（该文件当前无测试模块）。

覆盖 12 组：Unix/Windows × 长/短连接 × `off`/`strict`/`accept-new`。

平台分支用 `cfg!(windows)` 而非 `#[cfg]`，使两分支逻辑在单平台下均可断言；若实现用了 `#[cfg]`，测试相应加 `#[cfg_attr]` 分流。

**验证**：`cargo test --manifest-path src-tauri/Cargo.toml multiplex`

**审查门**：此处测试必须先于接线通过。若 12 组断言写不出来，说明 S2 的规则定义有歧义，退回 S2。

### S4 接线到 `build_ssh_command` `[回滚点 R3]`

在 `remote.rs:233` 现有 `-p` / `BatchMode` / `ConnectTimeout` 参数之后、`known_hosts_mode` 匹配之前，插入 `multiplex_args` 的结果。

确认：`build_ssh_command` 签名不变，20 个调用点零改动。

**验证**：
```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml
```

### S5 创建 ControlPath 目录 `[回滚点 R4]`

参照 `create_askpass_script`（`remote.rs:198`）的既有惯例：`fs::create_dir_all` + Unix 下 0700。

仅在 Unix 且短连接分支需要；Windows 分支不建目录。

### S6 退出清理 `[回滚点 R5]`

文件：`src-tauri/src/tray.rs:51`

在 `"quit"` 分支的 `app.exit(0)` **之前**插入清理调用。

实现按 design D3：遍历 `codex-ai-ssh-mux/`，逐个 `ssh -O exit -o ControlPath=<socket>` 优雅退出，失败则删文件兜底。全程容错，不阻塞退出——参照 `window_event.rs` 中 `save_window_size` 失败仅 `eprintln!` 的惯例。

**注意**：不要挂到 `window_event.rs` 的 `CloseRequested`——那是 close-to-tray，不是退出。

**验证**：`cargo build`

### S7 实机验证 `[审查门]`

```bash
npm run tauri:dev
```

必须逐项确认（对应 PRD 验收标准）：

1. SSH 远程项目连续执行 git 操作 → `ls $TMPDIR/codex-ai-ssh-mux/` 出现 socket 文件
2. 首条命令与后续命令耗时对比 → 后续显著更快
3. `allocate_tty` 长会话（远程 AI 会话）正常启动并持续运行
4. 三种 `known_hosts_mode` 各连一次 → 产生**不同** socket 文件（验证 D4）
5. 托盘「退出」→ socket 文件与 master 进程均清空（`ps aux | grep "ssh.*ControlMaster"` 为空）
6. 密码认证配置的远程项目功能正常

任一项不通过 → 回到对应回滚点。

### S8 收尾

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # 全绿且 ≥ 基线
npm run build                                      # 确认前端未受影响
```

提交（父任务约束：单独提交），合回 `main`。

## 回滚点

| 标记 | 状态 | 回退方式 |
|---|---|---|
| R1 | 纯函数已加未接线 | `git checkout -- remote.rs` |
| R2 | 测试已通过 | 保留 S2+S3，重做 S4 |
| R3 | 已接线 | 移除 `multiplex_args` 调用一行 |
| R4 | 目录创建 | 同 R3 |
| R5 | 清理已加 | 移除 `tray.rs` 调用一行 |

全任务回滚：`git branch -D feat/ssh-multiplex`（未合并前）。

## 验证命令汇总

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml
npm run build
npm run tauri:dev            # S7 实机
```

## 完成后

在父任务 `prd.md` 的衍生发现处登记：`ssh-secrets.json` 明文存密码 + Windows 无权限收紧，建议新增子任务迁移至 OS keychain（见 `design.md` R5 衍生发现节）。
