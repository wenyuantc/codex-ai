# Design: OpenCode SSH 远程补齐

## Architecture Overview

OpenCode 当前是 **本地 Node + `@opencode-ai/sdk` + `opencode_sdk_bridge.mjs`** 路径；`start_opencode` 在 `execution_target == ssh` 时会插入 pending 会话后硬失败（「尚未实现」）。本任务对齐 **Codex 远程 SDK** 模式（而非 Claude/Grok 的远程 CLI），在远程主机上跑同一套 bridge，使 SSH 项目可真实启动/停止/落库，并可选支持 one-shot。

```text
UI (employee.ai_provider === "opencode", SSH project)
  → startOpenCode / stopOpenCodeSession (src/lib/opencode.ts)
  → start_opencode
  → resolve_opencode_session_context (local | ssh)
  → [SSH]
       ensure_remote_opencode_sdk_runtime_layout
       inspect / install remote node + @opencode-ai/sdk + bridge
       build_ssh_command(remote: cd install_dir && node bridge)
       stdin JSON (mode/session|resume_session|one_shot, workingDirectory=remote cwd)
       stream stdout/stderr → codex_session_events + opencode-* events
  → stop：kill 本地 SSH child process group（复用现有 stop 路径）
```

**本地路径保持不变**；仅拆除 SSH 早退，并补齐远程 SDK 启动、健康检查、基线采集传参、one-shot。

## Boundaries

| 层 | 职责 | 不负责 |
|----|------|--------|
| `opencode/process/mod.rs` | SSH 分支启动、失败 finalize、基线传 `ssh_config`、活动日志 | 自建 SSH 参数 |
| `opencode/process/session_runtime.rs` | 本地/远程共用 bridge stdin 配置序列化；远程 spawn helper | ControlMaster 细节 |
| `opencode/settings.rs` | 本地设置；可选远程 install 目录默认值 helper | 改 UI 设置文件 schema（除非必要） |
| `app/remote.rs` | `build_ssh_command` / `execute_ssh_command*` / 远程健康与安装命令 | OpenCode stream 解析 |
| `codex/process/one_shot.rs` | SSH+opencode 分支改为远程 bridge one_shot | 长会话生命周期 |
| Frontend | 展示中文错误/SSH 终端行；设置页远程健康入口（若加 command） | 直接 spawn / 写库 |
| `engine/*` | 复用 `ExecutionContext` / `EngineChild` / stop 内核 | 不新增 dyn 引擎 trait |

## Key Decisions

| 决策 | 结论 | 理由 |
|------|------|------|
| 远程通道 | **远程 SDK bridge**（Node + `@opencode-ai/sdk` + 现有 mjs） | 产品内 OpenCode 无独立 headless CLI 路径；与本地协议一致 |
| SSH 入口 | 必须经 `build_ssh_command(..., allocate_tty=true, ...)` | 对齐 ssh-remote 规范；长会话禁用 ControlMaster 共享 |
| 远程 runtime 布局 | 每 `ssh_config_id` 独立目录（镜像 Codex `default_remote_sdk_install_dir` 风格，前缀 `opencode-sdk-runtime`） | 避免与 Codex remote SDK 目录/bridge 名冲突 |
| Bridge 脚本 | 上传当前 `include_str!("opencode_sdk_bridge.mjs")` 到远端 | 与 Codex `ensure_remote_sdk_runtime_layout` 同模式 |
| `opencode.json` 运行时配置 | 远程会话通过 **SSH 写入/恢复** 远端 `run_cwd/opencode.json`（备份语义对齐本地） | 本地依赖写 cwd；远端无本地 FS |
| 图片附件 | 复用 `prepare_execution_image_paths`；远程缺失/不可传时跳过并中文 WARN | 与 Claude/Grok 远程一致，不阻塞 MVP |
| one-shot | SSH OpenCode 启用远程 bridge `mode=one_shot` | PRD R1 可干活；与本地 one_shot 同源 |
| 失败文案 | 去掉「尚未实现」；中文原因（未装 Node/SDK、缺 ssh_config、健康检查失败） | 验收：受控失败 |
| UI 模式 | 终端行标明 `[SSH] 运行通道: 远程 SDK` | PRD R1「标明模式」 |
| 不做 | OpenCode 云多租户；send_input 与 Codex 对齐 | 归 engine-capability-parity |

## Contracts

### Session launch (SSH)

前置（与现有一致）：

1. 防重入 + `ensure_no_cross_provider_conflict`
2. `resolve_opencode_session_context` → `execution_target=ssh`，`working_dir=remote_repo_path`，`ssh_config_id`
3. `insert_codex_session_record(..., execution_target=ssh, ai_provider=opencode)`
4. 可选 `mark_task_git_context_running`

SSH 专属：

1. 加载 `SshConfigRecord`；缺失 → finalize + 中文错误
2. `validate_remote_opencode_health` / inspect：Node 可用、bridge 文件、sdk package（可按 Codex 粒度）
3. `ensure_remote_opencode_sdk_runtime_layout`：mkdir、package.json、写入 bridge；若 SDK 未装则引导/调用 `install_remote_opencode_sdk`（或启动前自动 npm install，行为在 implement 固定一种并单测文案）
4. 远程写入 `opencode.json` 运行配置（backup 对象记录远端路径与原内容，退出时 SSH 恢复）
5. `build_remote_opencode_sdk_bridge_command(install_dir, node_override)` →  
   `cd "$install_dir" && exec node "$bridge_path"`（`remote_shell_path_expression` + `build_remote_shell_command`）
6. `build_ssh_command(app, ssh_config, Some(remote_cmd), true, false)` + piped stdio + process group
7. stdin 写入与本地相同的 bridge JSON（`workingDirectory` = 远程 `run_cwd`；`mode` = session / resume_session）
8. 注册 manager；`stream_opencode_output` 复用；基线用 `capture_execution_change_baseline(..., Some(&ssh_config))`

### Stop / Resume

| 操作 | 行为 |
|------|------|
| stop | 现有 `stop_opencode_process_with_manager` 杀本地 SSH 子进程组 → 远端 node/bridge 随会话结束 |
| resume | `resume_session_id` 传入 bridge；依赖远端 OpenCode session 存储（与本地一致）；解析不到则新会话/错误中文提示 |
| 状态 | pending → running → stopping/completed/failed；`ai_provider=opencode` 不改表结构 |

### Health & install

| API | 行为 |
|-----|------|
| `validate_remote_opencode_health(ssh_config_id)` | SSH 探测 `node -v`、sdk package 路径、bridge 存在；返回 available / message / checked_at（模型字段对齐 `RemoteGrokHealthCheck` 或新增 `RemoteOpenCodeHealthCheck`） |
| `install_remote_opencode_sdk(ssh_config_id)` | 确保 layout + 远端 `npm install @opencode-ai/sdk`（镜像 `install_remote_codex_sdk`） |
| 本地 `check_opencode_sdk_health` | 不变 |

### One-shot

| target | provider | 行为 |
|--------|----------|------|
| local | opencode | 现有 `run_opencode_one_shot_via_sdk` |
| ssh | opencode | **新** `run_opencode_one_shot_via_remote_sdk`：ensure layout → 远程写 config（可选）→ SSH 跑 bridge `mode=one_shot` → 聚合 stdout 文本 |
| ssh | 其它 | 不变 |

同步改：

- `resolve_remote_one_shot_runtime` 中 opencode 分支：由 `unavailable` 改为 `sdk` + 中文说明
- `normalize_one_shot_provider*` 允许 remote opencode（当前 `if !is_remote` 会挡掉）
- `ai_commands` 中 coordinator/tester 对 SSH+opencode 的硬拒绝改为可走 one-shot 或明确会话路径

### Events / activity

- 继续 `opencode-stdout|stderr|exit|session` + `codex_session_events`
- 活动 key 已有 `remote_task_session_started` 等；新增远程健康/安装失败时用现有 activity 风格；**若新增 action key**，前端 `getActivityActionLabel()` 必须加中文
- 仪表盘：仅当新增 key 时补中文

### Artifact / baseline

- SSH：`capture_external_remote_execution_change_baseline`
- 会话启动时 **必须传入** `ssh_config`（修当前本地分支 `None` 的 SSH 死路径）
- 受限失败：事件 `session_file_changes_baseline_failed` + 终端中文行，不静默

## Data Flow

### 任务会话（SSH）

```text
start_opencode
  → insert pending session (ai_provider=opencode, execution_target=ssh)
  → load ssh_config
  → ensure remote runtime + optional install
  → write remote opencode.json (backup)
  → spawn ssh + node bridge, stdin config
  → status=running, stream events
  → exit → restore remote config, finalize session, automation hook
```

### 与本地隔离

- 本地：本地 install_dir + 本地 cwd
- SSH：远端 install_dir + 远端 cwd + ssh_config_id 绑定会话
- 不把本地 path 当远程 cwd；不跨项目复用错误的 ssh_config

## Compatibility & Migration

- **无 DB migration**（会话表已有 `execution_target` / `ssh_config_id` / `ai_provider`）
- 旧会话「启动失败/尚未实现」历史行只读
- Capabilities：start/stop/resume 保持；不宣称 restart/send_input
- Spec：`ai-engines.md` 中「OpenCode SSH limited」在实现后更新（Phase 3.3）

## Trade-offs

| 选择 | 理由 | 代价 |
|------|------|------|
| 远程 SDK 而非远程 CLI | 与现有 OpenCode 实现一致 | 远端需 Node + npm 安装；运维成本高于 pure CLI |
| 镜像 Codex remote layout | 已有 scp/cat bridge、mux 规范 | 与 Codex 有平行代码；可抽小 helper 但不做大重构 |
| 启动时写远端 opencode.json | 保证模型/effort 与本地一致 | 需可靠 backup/restore；失败要 finalize |
| one-shot 一并打通 | 减少「会话能开、一键 AI 不能」 | one_shot.rs 多分支 |

## Risks & Mitigations

| 风险 | 缓解 |
|------|------|
| 远端无 Node / 旧 Node | 健康检查明确中文；安装指引 |
| `@opencode-ai/sdk` npm 失败 | install command 返回 stderr 摘要；会话失败不卡死 manager |
| 远程 opencode.json 写坏用户文件 | 解析失败不覆盖（对齐本地）；backup restore 在 exit/失败路径 |
| SSH 密码/askpass | 只走 `build_ssh_command`；不新开密钥路径 |
| 长会话与 mux 互相杀 | `allocate_tty=true` → ControlMaster=no（规范） |
| stop 只杀本地 ssh、远端残留 | process group + bridge 随 stdin/父进程退出；文档化 best-effort |
| 与 Codex remote bridge 文件名冲突 | 独立 install_dir 与 `opencode_sdk_bridge.mjs` 文件名 |

## Rollout / Rollback

- 无 feature flag：SSH 选 OpenCode 员工即走新路径；失败有中文原因
- 回滚：恢复 SSH 早退 `Err(...)` 即可；历史会话不受影响
- 验证：本地 OpenCode 回归；SSH 有/无 SDK；stop 状态；local/SSH 互不污染；clippy + tests

## Open Implementation Notes

- 远程 npm 是否「首次会话自动装」vs「仅设置页安装」：优先 **ensure layout + 检测未装则自动 install 一次**（对齐 Codex task SDK 体验），设置页保留显式安装按钮。
- host/port：远端 bridge 自启临时服务时优先让 mjs 使用 127.0.0.1 + 动态端口（已有 `findFreePort`）；远程 settings 中的 host/port 仅作兼容字段，避免绑到用户本机地址。
- 前端：若仅后端错误文案改进，UI 改动可最小；若加 `validate_remote_opencode_health`，在 SSH/Runtime 设置区挂入口（对照 Grok remote health）。
