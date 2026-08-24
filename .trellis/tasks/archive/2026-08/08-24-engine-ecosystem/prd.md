# PRD · P0 引擎生态对齐

> 父任务。持有需求全集、子任务地图与最终集成验收，不作为实现目标。
> 实现一次只 `task.py start` 一个子任务。
> 来源：`TASK.md`「下一波 · 2026-08-24 五引擎时代」P0；用户 2026-08-24 确认创建本波任务。

## Goal

第五引擎内置 Agent（native）已跑通「模型 → 工具 → 会话」，但装备不齐；MCP / 权限 / 图片仍按四引擎时代的半真半假能力对外展示。本波把**已经配得上的入口说真话，并把 native 补到配得上「自研 Agent」的最低信任线**。

一句话：**配了要能用，不能「配了=没配」。**

## 已确认事实（代码实读 2026-08-24）

- MCP 配置与任务三态绑定是全产品入口：`McpSettingsTab`、`TaskMcpBindingSection`、`mcp-servers.json`、`tasks.mcp_server_ids`。
- 启动路径真正消费 MCP 的只有 Codex：`codex/mcp.rs` + `codex/process/{mod,session_launch,command_builders}.rs`（`append_mcp_config_args`）。`claude/`、`grok/`、`opencode/`、`native/` 对 MCP **零命中**。
- 设置文案仍暗示可「合并到 Codex、Claude 等引擎配置」（`src/locales/zh-CN/settings.json:625`），任务绑定文案写「控制本任务会话启用的 MCP 服务器」（`src/locales/zh-CN/tasks.json:282`），未按员工引擎分流。
- 能力矩阵 `get_ai_provider_capabilities` 只有 start/stop/restart/send_input/resume，**没有 MCP / 图片字段**（`src-tauri/src/db/models.rs:1193-1202`）。
- native 工具目录 10 个（Read/Write/Edit/Bash/Glob/Grep/TodoRead/TodoWrite/WebFetch/WebSearch）；权限模式写死 yolo（`native/prompt/identity.md:14`、`native/prompt/mod.rs:63`）。
- native 图片：本地读文件转 base64，最多 8 张（`native/images.rs`）；SSH 工作区文件走 SSH，图片仍读**本机路径**。
- Claude SDK 本地会把 `imagePaths` 交给 `claude_sdk_bridge.mjs`；Claude CLI 本地目前跳过（`claude/process/mod.rs:1240-1250`），stdin 只写纯文本；本机 CLI 2.1.221 无 `--image`，有 `--input-format stream-json`。运行前 UI 已有 Claude CLI / SSH 跳过确认（P0-3 将取消本地 CLI 预警告）。

## 子任务地图

| 子任务 | 优先级 | 一句话需求 |
|---|---|---|
| `08-24-p0-mcp-align` | P0 | native 注入 MCP 工具 + UI/能力矩阵按引擎说真话；Claude/Grok/OpenCode 本波不执行 MCP |
| `08-24-p0-native-tool-permission` | P0 | native 高风险工具（删除/覆盖/推送/强制 git）执行前用户确认；低风险保持 yolo |
| `08-24-p0-claude-images` | P0 | 补齐 Claude CLI **本地**传图（stream-json）；SSH 仍跳过 |

建议实现顺序：P0-1 → P0-2a → P0-3。P0-2a 应把 MCP 工具默认视为高风险（或按工具注解分类），因此排在 MCP 注入之后。

## 跨子任务验收

1. 三个子任务各自 AC 通过。
2. 用户给 **Claude / Grok / OpenCode** 员工配 MCP 时，界面不再暗示「本会话会带上这些服务器」；Codex 行为不变；native **本地与 SSH** 在承诺范围内真的能调到 MCP 工具（SSH 必须远端执行，不得打到开发机）。
3. native 删除/覆盖/推送/强制 git / MCP 不再静默 yolo；确认框三选：**本会话全部允许** / **仅允许一次** / **不允许**。不允许或关闭对话框后该次工具失败，会话可继续。全部允许只活在当前会话内存，重启/新会话重新问，不写设置。
4. Claude **本地 CLI** 与 SDK 都能附带图片；SSH Claude 继续跳过并保持运行前警告。SDK 本地不回退。失败不得标成已看图。
5. 质量门禁：`clippy -D warnings`、`cargo test`（相关模块）、`npm run lint`、`format:check`、`test:ci`、`npm run build`。
6. 仓库约定：迁移连续、新活动日志 key 进仪表盘中文、SSH 兼容、时间走 `formatDate()`、文案 zh-CN+en。
7. `TASK.md` 勾选本波 P0 三项；`ai-engines.md` 与能力矩阵文档不因本波失真。
8. 父任务不写产品代码。

## 明确不做（本波）

- native 子 Agent、plan 模式工作流、Skills / Hooks / ApplyPatch / Browser
- 给 Claude / Grok / OpenCode **实现** MCP 执行（只允许诚实声明/禁选）
- Grok `send_input`、token→金额、定时 cron、hunk 级暂存
- P1/P2（工时、依赖图、帮助中心、文档脚本化）与 N-P3 技术债拆文件
- 路线图级：微服务、多端同步、完整 IDE、Issues 双向同步

## 已锁定决策

| 决策 | 推荐 | 备选 |
|---|---|---|
| native MCP 的 SSH | **远程也要做**：SSH native 会话在远端拉起 MCP stdio，JSON-RPC 经 `build_ssh_command` 长连接转发。禁止把本机 MCP 伪装成远程工具。远端 spawn 失败则该服务器跳过并可见警告，会话其余工具继续。 | 仅本地注入（已否决） |
| 高风险确认 UX | 会话中阻塞对话框，三选：**本会话全部允许** / **仅允许一次** / **不允许**。关闭对话框、停止会话、超时 = 不允许。不自动放行。 | 仅终端打印（已否决） |
| 同一会话内重复高风险调用 | **由用户当场选**：全部允许后本会话后续高风险不再问；仅允许一次则下次再问。范围=该会话全部高风险种类（覆盖/删除/推送/MCP），内存态，不写设置。 | 按风险类型分别记住 / 跨会话白名单（本波不做） |
| Claude CLI 图片 | **补齐本地 CLI 传图**（2.1.221 无 `--image`，走 `--input-format stream-json` image block，对齐 SDK bridge）。SSH 仍跳过 + A2 警告。失败可见，不假装已看图。 | 只声明不支持（已否决） |

## 开放问题

无。产品决策已锁定，规划可进入最终确认（仍须用户批准本摘要后才 `task.py start`）。
