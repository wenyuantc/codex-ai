# PRD · 内置 Agent（zcli 移植）

> 父任务。持有需求全集与最终集成，不作为实现目标。实现一次只 `task.py start` 一个子任务。
> 来源：用户 2026-08-20；zcli 参考 `/Users/wenyuantc/IdeaProjects/my/zcli`。

## Goal

让 Codex AI 拥有自带编程 Agent。员工可选第五种引擎 `native`（UI：内置 Agent），流量走设置里的渠道 API，不依赖本机或远程安装 Codex / Claude / Grok / OpenCode CLI。

## 已锁定决策

| 决策 | 选择 |
|---|---|
| 第一期 | 编码核心 MVP |
| Provider | 存储 `native`，文案「内置 Agent」 |
| 渠道 | 应用级共享；员工绑定一个渠道 + 模型 |
| 协议 | `openai` Chat Completions；`anthropic` Messages；`codex` OpenAI Responses `/v1/responses` |
| 运行 | 进程内 tokio 任务，不 spawn sidecar，不硬套 `EngineChild` |
| SSH | 循环在本地；文件/shell 工具经 `build_ssh_command` |
| 权限 | yolo + workspace-only |
| 四引擎 | 保留且行为不变 |
| 密钥 | 密钥环；SQLite 只存 `api_key_ref` |
| 会话 | 继续 `codex_sessions`，不接 `~/.zcli` |

## 子任务地图

| 子任务 | 交付 |
|---|---|
| `08-20-native-agent-channels` | 渠道表、密钥、Settings CRUD、测通、活动日志 |
| `08-20-native-agent-model-client` | 三协议 SSE + tool-call 客户端 |
| `08-20-native-agent-loop-tools` | 多轮循环 + 7 个核心工具 + 本地/SSH |
| `08-20-native-agent-engine` | NativeAgentManager 接入会话/队列/编排/审核 |
| `08-20-native-agent-ui` | 员工绑定 + 全页面 provider + 能力矩阵 |

## Requirements

- R1 设置可配置 openai / anthropic / codex 渠道；密钥不进库、不进导出 SQL。
- R2 员工可选内置 Agent 并绑定渠道+模型后，任务/审核/编排走进程内循环，不拉起外部 CLI。
- R3 核心工具：`Read` `Write` `Edit` `Bash` `Glob` `Grep` `TodoRead` `TodoWrite`；workspace-only。
- R4 本地与 SSH 项目都能改工作区文件；SSH 不要求远端安装新二进制。
- R5 `start/stop/restart/resume/send_input` 对 `native` 诚实；任务执行计入 run queue。
- R6 现有四引擎回归不变。活动日志中文进仪表盘。时间展示 `formatDate()`。

## Out of Scope

- 移植 zcli TUI / ACP / Skills / 子 Agent / MCP / Hooks / Cron / Browser / plan 模式
- 交互式权限 UI、ApplyPatch、ChatGPT 网页 Codex 后端
- 删除或替换现有四引擎

## Acceptance Criteria

- [x] 五个子任务各自 AC 通过
- [x] 父任务集成：native 员工从创建 → 配渠道 → 跑本地任务 → 会话日志可见工具事件 → 可停止/续聊（代码路径已贯通；桌面冒烟待真机）
- [x] SSH 项目同等路径可用（有 SSH 配置时手工冒烟；无则单测覆盖远程命令构造）
- [x] 四引擎员工路径无回归（未知 provider 不再落入 Codex；CLI 引擎启动仍走原 commands）
- [x] `clippy -D warnings`、`npm run format:check`、`npm run test:ci`、相关 `cargo test` 通过
- [x] `.trellis/spec/backend/ai-engines.md` 与 `directory-structure.md` 已更新
