# 支持 Grok Build 作为 AI Provider 并启用 SSH

## Goal

在 Codex AI 中新增 xAI **Grok Build**（`grok` CLI）作为一等 AI Provider：员工可选用 Grok 跑任务会话；**本地与 SSH 项目**均可执行；并将 Grok 纳入 **one-shot / AI commit 偏好** 与运行时设置面（范围 C）。

## Background

- 现有 provider：`codex` | `claude` | `opencode`；会话共用 `codex_sessions*`，靠 `ai_provider` 区分。
- Claude 已支持本地 + SSH CLI 会话，是 Grok 会话实现模板；OpenCode SSH 受限，不作 SSH 模板。
- One-shot / AI commit 集中在 `codex/settings.rs`、`codex/process/one_shot.rs` 与设置页 provider 下拉。
- 本机 Grok CLI `0.2.112`：默认模型 `grok-4.5`；effort `high|medium|low`；headless：`grok -p ... --output-format streaming-json`。
- 认证：远端自备 `grok login` 或远端环境自有凭据；应用不注入密钥（对齐 Claude SSH）。

## Requirements

### R1 — Provider 基础

1. 新增 `AiProvider = "grok"`，展示名 `Grok`。
2. 员工创建/编辑可选 Grok；模型默认 `grok-4.5`；effort 默认 `high`（`high|medium|low`）。
3. 全量 UI 标签/过滤/徽章识别 Grok（员工、任务指派、Sessions、TaskSessionChain 等）。
4. `get_ai_provider_capabilities`：start/stop/resume = true；restart/send_input = false（对齐 Claude）。

### R2 — 任务会话（本地 + SSH）

1. 本地：Grok 员工 start/stop 任务会话，输出进入现有 session 事件流。
2. SSH：用项目 `ssh_config` + `remote_repo_path`，经 `build_ssh_command` 远程执行 headless `grok`。
3. 会话行 `ai_provider = "grok"`，不新建平行表。
4. `task_automation` / review 启动：assignee 为 grok 时走 Grok start。
5. 错误可读：找不到 `grok`、未认证、SSH 失败、工作目录无效。
6. 远程凭据：**不由应用注入**；依赖远端已登录/已配置。

### R3 — One-shot 与 AI Commit

1. Runtime 设置：`one_shot_preferred_provider` 可选 Grok + 模型/effort。
2. Git 自动化：`ai_commit_preferred_provider` 可选 Grok（custom 源）。
3. 后端 normalize + one_shot 分发支持 grok：
   - 本地：Grok headless CLI
   - SSH：远程 Grok headless CLI（**不**像 OpenCode 那样禁用 SSH）
4. 相关前端文案/状态识别 Grok。

### R4 — 设置与健康探测

1. Grok 轻量设置：可选 `cli_path_override`、默认模型、默认 effort（并入 Runtime 设置区，非独立大页）。
2. 本地健康：是否找到 `grok`、版本、简要可用状态。
3. 远程健康（轻量）：对选定 SSH 配置探测 `grok --version` / 可执行性（不做远程安装）。

### R5 — 兼容性

1. 不破坏 Codex/Claude/OpenCode 既有主路径（含 Claude SSH）。
2. 未知 `ai_provider` 经 normalize 安全回落（现行为回落 codex）。
3. 不引入插件化 provider 框架。

## Acceptance Criteria

- [ ] 员工可设为 Grok 并持久化模型/effort；各处标签显示 Grok。
- [ ] 本地 Grok 任务会话可启动、见输出、可停止。
- [ ] SSH Grok 任务会话可启动；远端无 CLI/未认证时错误明确。
- [ ] automation 下 Grok assignee 走 Grok，不误启动 Codex。
- [ ] Sessions 可按 Grok 过滤。
- [ ] one_shot / AI commit 设置可选 Grok；本地与 SSH one-shot 偏好 Grok 时调用 Grok（非静默变 Codex）。
- [ ] 本地/远程轻量健康信息可在设置或健康检查路径展示。
- [ ] capabilities 与实现一致；`npm run build` + 相关 `cargo test` 通过；Codex/Claude/OpenCode 冒烟不回归。

## Out of Scope

- Provider 插件框架重构；OpenCode SSH 补齐。
- Grok ACP / `agent stdio` 深度集成（首期 headless `-p`）。
- 应用内 Grok 浏览器登录 UI；远程自动安装 Grok。
- 应用侧存储/注入 `XAI_API_KEY`（二期可选）。
- 修复 Grok CLI 上游隐私问题（必要时仅提示）。

## Key Decisions

| 决策 | 结论 |
|------|------|
| 指代 | xAI Grok Build CLI（`grok`） |
| 范围 | C：会话（本地+SSH）+ one-shot + AI commit + 轻量健康 |
| 会话模板 | Claude process（含 SSH） |
| Headless | `grok -p` + `streaming-json` + `bypassPermissions`/`--always-approve` |
| SSH 认证 | A：远端自备登录；不注入密钥 |
| 远程健康 | 轻量 version/可用性探测；不安装 |

## Notes

- 前端大量三元 provider 分支需全量扫改。
- `normalize_one_shot_provider*` 不识 grok 会回落 codex —— 范围 C 必改。
- 复杂任务：以本 PRD + `design.md` + `implement.md` 为实施依据。
