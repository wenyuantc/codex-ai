# Implement: OpenCode SSH 远程补齐

## Order & Checklist

### 0. Before code

- [ ] Load `trellis-before-dev`；精读：
  - `.trellis/spec/backend/ai-engines.md`
  - `.trellis/spec/backend/ssh-remote.md`
  - `.trellis/spec/guides/cross-layer-thinking-guide.md`
- [ ] 对照基线：
  - Codex SSH SDK：`app/remote.rs` `ensure_remote_sdk_runtime_layout` / `install_remote_codex_sdk` / `inspect_remote_codex_runtime`
  - Codex launch：`codex/process/session_launch.rs` + `command_builders.rs::build_remote_sdk_bridge_command`
  - OpenCode 现状：`opencode/process/mod.rs` SSH 早退（约 1408–1445 行）、`session_runtime.rs`、`settings.rs`
  - Grok 远程健康 UI 挂载点（若做设置入口）

### 1. Remote runtime plumbing（remote + models）

- [ ] `app/remote.rs`（或 opencode 内 re-export 薄封装）：
  - `remote_opencode_sdk_bridge_path(install_dir)`
  - `default_remote_opencode_sdk_install_dir(ssh_config_id)`
  - `ensure_remote_opencode_sdk_runtime_layout(app, ssh_config_id)` — mkdir + package.json + `cat > bridge`（内容 `opencode_sdk_bridge.mjs`）
  - `install_remote_opencode_sdk` Tauri command — npm install `@opencode-ai/sdk`
  - `validate_remote_opencode_health` / `inspect_remote_opencode_runtime` — node + sdk + bridge
- [ ] `db/models.rs`：`RemoteOpenCodeHealthCheck`（或复用相近结构）+ 必要 serde
- [ ] `lib.rs`：注册新 commands
- [ ] 全程 `build_ssh_command` / `execute_ssh_command*`；长会话 `allocate_tty=true`

### 2. OpenCode process SSH launch

- [ ] `opencode/process/session_runtime.rs`：
  - 抽取 bridge config JSON 序列化（local/remote 共用）
  - `launch_opencode_bridge_via_ssh(...)`：SSH spawn + stdin 写 config + `OpenCodeChild::with_stdio`
- [ ] `opencode/process/mod.rs` `start_opencode_with_manager`：
  - **删除** SSH 硬失败早退
  - SSH 分支：fetch ssh_config → ensure layout →（可选）auto-install → 远程写 `opencode.json` backup → launch remote bridge → stream
  - 本地分支逻辑保持
  - `capture_execution_change_baseline` **传入** `ssh_config`
  - 终端行：`[SSH] 运行通道: 远程 SDK`、准备/失败中文文案
  - 失败路径：`finalize_launch_failure` + restore remote config + cleanup askpass
- [ ] 远程 `opencode.json`：SSH 读-改-写 + backup/restore（解析失败不覆盖，对齐本地）
- [ ] stop 路径确认对 SSH child 有效（复用现有 manager kill）

### 3. One-shot + provider normalize

- [ ] `codex/settings.rs`：允许 remote `opencode` 作为 one_shot provider（去掉 `!is_remote` 门闩）
- [ ] `codex/process/one_shot.rs`：
  - `(SSH, opencode)` → `run_opencode_one_shot_via_remote_sdk`
  - 错误中文；restore config
- [ ] `app/remote.rs` `resolve_remote_one_shot_runtime`：opencode → sdk + 说明文案
- [ ] `codex/process/ai_commands.rs`：去掉/改写 SSH+opencode coordinator/tester 硬拒绝（改为可执行或统一错误）

### 4. Frontend / settings（最小必要）

- [ ] `src/lib/opencode.ts` 或 `backend.ts`：wrap `validate_remote_opencode_health` / `install_remote_opencode_sdk`（若注册）
- [ ] SSH/Runtime 设置区：远程 OpenCode 健康与安装入口（对照 Grok/Codex remote）
- [ ] 若新增 activity action key → `getActivityActionLabel()` 中文 + 仪表盘可见
- [ ] 确认启动错误 toast/会话事件展示后端中文字符串（无「尚未实现」）

### 5. Tests

- [ ] 远程 bridge 命令构造：install_dir 含空格/`~` 时 `remote_shell_path_expression` 正确
- [ ] one_shot normalize：`opencode` + remote 不再静默变 codex
- [ ] `resolve_remote_one_shot_runtime` opencode 非 unavailable
- [ ] 既有 OpenCode runtime config 单测不回归
- [ ] 不访问真实 SSH 网络

### 6. Validation gate

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] `npm run build`
- [ ] 手工冒烟（见下）

## Validation Commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
```

手工冒烟：

1. **本地** OpenCode 员工：启动/停止会话，行为与改前一致  
2. **SSH** 项目 + OpenCode：远端 Node+SDK 就绪时启动成功，会话表 `execution_target=ssh`、`ai_provider=opencode`  
3. 停止后状态正确（非 running）  
4. 远端未装 SDK：中文错误/可安装，**不再**「尚未实现」  
5. 本地与 SSH 项目并行不污染 cwd/会话  
6. SSH one-shot（若启用）返回文本或明确错误  
7. Claude/Grok SSH 与 Codex 本地回归无破坏  

## Risky Files / Rollback Points

| 风险点 | 回滚 |
|--------|------|
| `opencode/process/mod.rs` 启动主路径 | 恢复 SSH early return；本地分支未动 |
| `app/remote.rs` 新 helper | 独立函数，删除即可 |
| `one_shot.rs` + settings normalize | 恢复 remote opencode unavailable |
| `ai_commands.rs` 硬拒绝 | 恢复原 if |
| `lib.rs` handlers | 编译期可见 |

紧急回滚：SSH 分支 `return Err("SSH 模式下暂不支持 OpenCode…")` 即可恢复旧行为。

## Review Gates

1. PRD 验收项均可映射到代码路径  
2. SSH 启动不经「尚未实现」裸错；成功或中文受控失败  
3. 所有 SSH 长会话走 `build_ssh_command(..., true, ...)`  
4. 无前端写库；无新 session 表  
5. local / SSH 会话字段隔离  
6. clippy `-D warnings` + tests + `npm run build` 通过  

## Follow-ups (not this task)

- send_input / restart 与 Codex 对齐 → `engine-capability-parity`  
- 远程 OpenCode 一键登录/密钥注入  
- 与 Codex 共享通用「remote node sdk runtime」抽象大重构  
- UX trust：统一受限能力 banner（`ux-trust-hardening`）  
