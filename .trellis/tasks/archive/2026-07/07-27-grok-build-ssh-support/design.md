# Design: Grok Build Provider + SSH

## Architecture Overview

新增第四个 AI 引擎模块 `src-tauri/src/grok/`，**镜像 Claude 模块形状**，复用：

- 共享 session 表（`codex_sessions` / events / file changes）
- SSH 基建（`build_ssh_command`、`build_remote_shell_command`、`fetch_ssh_config_record_by_id`）
- 执行上下文解析模式（local cwd vs remote_repo_path）
- 前端 `startX` / event listen 薄封装

```text
UI (employee.ai_provider === "grok")
  → startGrok / stopGrokSession (src/lib/grok.ts)
  → Tauri start_grok / stop_grok_session
  → grok::process (local spawn | SSH remote headless)
  → stream events → codex_session_events + frontend events
  → task_automation on exit (reuse existing hooks)
```

One-shot / AI commit 不新建会话长生命周期，而是在 `codex/process/one_shot.rs` 增加 `grok` 分支，调用同一套 CLI 参数构造 + 本地/远程执行 helper。

## Boundaries

| 层 | 职责 | 不负责 |
|----|------|--------|
| `src/lib/types.ts` | `AiProvider` 联合类型、模型/effort 选项、normalize | 进程细节 |
| `src/lib/grok.ts` | invoke + event 订阅 | 业务状态机 |
| UI 分支 | 启动分发、标签、设置下拉 | 直接 spawn |
| `src-tauri/src/grok/` | 设置、健康、会话生命周期、流解析 | DB schema 重构 |
| `codex/settings` + `one_shot` | provider normalize、一次性调用路由 | Grok 长会话 |
| `app/remote` | 可选 `validate_remote_grok_health` | 远程安装 Grok |
| `task_automation` | assignee=grok 时 start_grok | Grok 专属自动化策略 |

## Module Layout (new)

```text
src-tauri/src/grok/
  mod.rs
  manager.rs          # 对齐 ClaudeManager
  settings.rs         # grok-settings.json：cli_path_override, default_model, default_reasoning_effort
  process/
    mod.rs            # start_grok / stop / CLI args / remote command
    context.rs        # resolve_session_execution_context（可复制 Claude 并改文案）
    lifecycle.rs      # GrokChild
    session_runtime.rs
    stream.rs         # streaming-json 宽松解析 → 终端行 + session_id
```

`lib.rs`：`mod grok;`、manage `GrokManager`、注册 commands。

## Contracts

### Provider ID

- 存储值：`"grok"`
- 展示：`Grok`
- normalize：仅精确匹配 `"grok"`；未知回落现有默认（codex）

### Models / Effort

| 项 | 值 |
|----|-----|
| 默认模型 | `grok-4.5` |
| 模型选项 | 首期静态列表：`[{ value: "grok-4.5", label: "Grok 4.5" }]`（后续可接 `grok models`） |
| Effort | `high` \| `medium` \| `low`，默认 `high` |
| capabilities | start/stop/resume true；restart/send_input false |

### Headless CLI（会话与 one-shot 共用参数构造）

```bash
grok -p <prompt> \
  --cwd <run_cwd> \          # 或由 spawn cwd / remote cd 提供
  -m <model> \
  --reasoning-effort <effort> \
  --output-format streaming-json \
  --permission-mode bypassPermissions \
  [--system-prompt-override <sp>] \
  [--resume <session_id>]
```

备选：`--always-approve` 与 permission-mode 二选一，实现时以「非交互自动批准工具」为准，固定一种并在注释说明。

远程：

```bash
cd <remote_repo_path> && exec grok <same-args without conflicting cwd if already cd>
```

经 `build_remote_shell_command` + `build_ssh_command(..., remote_command, true, false)`。

### CLI 解析

- 可执行名：`grok`
- 覆盖：`GROK_CLI_PATH` / 设置 `cli_path_override`
- 搜索：复用 `codex/cli.rs` 的 resolve 模式（env → known paths → shell `command -v`），路径含 `~/.grok/bin`

### 事件

- 前端：`grok-stdout` / `grok-stderr` / `grok-exit` / `grok-session`（payload 字段对齐 Claude）
- DB：`insert_codex_session_record(..., ai_provider: "grok")` + session events
- stream：对 `streaming-json` 做**宽松**解析（提取可读文本行 + session id）；未知 JSON 类型降级为原始行，避免解析失败卡死

### One-shot

在 `normalize_one_shot_provider*` / `SUPPORTED_ONE_SHOT_PROVIDERS` 加入 `"grok"`。

分发：

| target | provider | 行为 |
|--------|----------|------|
| local | grok | `run_grok_one_shot_via_cli` |
| ssh | grok | `run_grok_one_shot_via_remote_cli` |
| ssh | opencode | 仍禁用（不变） |

One-shot 输出：优先 plain/`json` 最终文本；若用 streaming-json，聚合 assistant 文本后返回字符串。

### 健康检查

**本地** `inspect_grok_runtime` / `get_grok_health`：

- `cli_available`, `cli_version`, `cli_path`, `status_message`, `checked_at`
- 不强制解析 auth.json；若 `grok models` 或 version 可跑即视为 CLI 可用（认证失败在真正调用时暴露）

**远程** `validate_remote_grok_health(ssh_config_id)`：

- SSH 执行 `grok --version`（或 `command -v grok && grok --version`）
- 返回 available/version/message；**不**安装、**不**登录

### 设置存储

- 文件：`app_config_dir/grok-settings.json`
- 字段：`cli_path_override?`, `default_model`, `default_reasoning_effort`
- one_shot / git 的 provider 选择仍写在现有 `codex-settings.json`（与 Claude/OpenCode 一致）

## Data Flow

### 任务会话启动

1. UI 按 `employee.ai_provider` 调 `startGrok`
2. `start_grok`：防重入 + cross-provider conflict（对齐 Claude）
3. resolve execution context（task→project→local/ssh）
4. insert session pending（ai_provider=grok）
5. 构造 CLI args；local spawn 或 SSH
6. 挂 stream task；更新 running；emit 输出
7. exit → finalize session → automation hook

### One-shot

1. 读 settings.one_shot_preferred_provider（或 override）
2. normalize 为 grok 后走 grok CLI helper
3. 返回文本给现有 AI 命令（optimize prompt / plan / commit message 等）

## Compatibility & Migration

- **无 DB migration**：`ai_provider` 已是字符串；员工表无需改 schema。
- 旧设置缺 grok 字段：normalize 默认仍 codex。
- 前端 `normalizeAiProvider` 增加 grok；未知仍回落 codex。
- Capabilities 数组追加 grok 项。

## Trade-offs

| 选择 | 理由 | 代价 |
|------|------|------|
| 复制 Claude 模块而非抽象插件 | 与仓库现风格一致、改动可审 | 第四份相似 process 代码 |
| headless `-p` 而非 ACP stdio | 与文档 scripting 示例一致、易 SSH | 无双向 tool 协议；resume/流式字段需宽松解析 |
| 远端自备登录 | 对齐 Claude、无密钥托管 | 运维需在远程 `grok login` |
| 静态模型列表 | 当前 CLI 仅稳定暴露 grok-4.5 | 新模型需改代码或二期动态拉取 |
| SSH one-shot 支持 grok | 范围 C 且 Claude 已有先例 | one_shot.rs 分支变多 |

## Risks & Mitigations

| 风险 | 缓解 |
|------|------|
| streaming-json schema 不稳定 | 宽松解析 + plain 回退；单测样例事件 |
| PATH 不含 `~/.grok/bin` | resolve 时加入 known dirs；设置可覆盖路径 |
| 远程未登录错误难读 | 捕获 stderr，映射中文提示「请在远程执行 grok login」 |
| 前端漏改 provider 三元表达式 | implement 清单全量文件列表 + 搜索验收 |
| one_shot normalize 漏改导致静默 Codex | 单测 normalize_one_shot_provider("grok") |
| 与 Codex 进程冲突 | 复用 `ensure_no_cross_provider_conflict` 模式 |

## Rollout / Rollback

- 功能开关：无强制 flag；用户不选 grok 即不触发。
- 回滚：移除/禁用 UI 选项 + 停止注册 start_grok（或 normalize 拒绝 grok）；会话历史中已有 grok 行只读展示即可。
- 发布验证：本地 grok 会话；SSH 会话（有/无 grok）；one_shot 选 grok；Claude SSH 回归。

## Open Implementation Notes（非产品决策）

- system prompt：优先 `--system-prompt-override`；若与员工 prompt 组合，对齐 Claude `compose_*_prompt` 策略。
- 图片附件：首期 SSH/Grok 可对齐 Claude「远程忽略图片计数提示」；本地若 CLI 支持再接，不阻塞 MVP。
- resume：CLI 支持 `--resume`；会话表已有 session_id 字段时接上，解析不到则降级新会话。
