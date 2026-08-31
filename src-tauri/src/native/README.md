# 内置 Agent（native）

`src-tauri/src/native` 是应用内置的编程 Agent 运行时：它不依赖外部 CLI（Codex/Claude/Grok/OpenCode），而是直接通过用户配置的 **AI 渠道**（OpenAI 兼容 / Anthropic / OpenAI Responses 三种协议）发起 HTTP 流式请求，并在 Rust 进程内完成「模型调用 → 工具执行 → 上下文维护 → 会话记录」的完整循环，与本项目其他引擎（`src-tauri/src/{codex,claude,grok,opencode}`）复用同一套 `codex_sessions` 会话表、活动日志、文件变更基线与任务自动化体系。

## 目录结构

```
src-tauri/src/native/
├── mod.rs                    # 模块声明与对外重导出（NativeAgentManager 等）
├── manager.rs                # 内存态会话注册表 NativeAgentManager
├── session.rs                # 会话生命周期（启动/停止/重启/恢复/输入/一次性调用）
├── transcript.rs             # native 会话模型历史落库（续聊/崩溃后续跑）
├── settings.rs               # 内置 Agent 设置（轮次/上下文/Token 预算/权限超时/子预算）
├── channels.rs               # AI 渠道 CRUD、测通、拉取模型列表
├── protocol.rs               # 协议归一化、URL 构造、模型列表解析、渠道 DTO
├── secret_store.rs           # 系统密钥库（keyring）读写渠道 API Key
├── images.rs                 # 图片附件加载（base64 + mime 识别）
├── model_catalog.rs          # 内置模型目录的加载、查找与默认值回填
├── model_catalog.json        # 模型目录数据（编译期内嵌）
├── prompt/
│   ├── mod.rs                # 系统提示词组装（身份/环境/Git/模板/项目指令/员工设定）
│   └── identity.md           # 内置 Agent 身份与工作方式设定
├── agent/
│   ├── mod.rs                # 模块声明
│   ├── loop.rs               # AgentRunner 主循环（模型调用/工具执行/轮次控制/子 Agent 批次）
│   ├── subagent.rs           # Agent 工具参数、类型、并发/轮次上限、子循环提示词
│   ├── compact.rs            # 上下文压缩、RolloutBudget、ChildQuota
│   └── truncate.rs           # 超长工具结果截断与图片 token 估算
├── model/
│   ├── mod.rs                # 模块声明与重导出
│   ├── client.rs             # ModelClient：请求/SSE 解析/重试/模型列表/探活/鉴权
│   ├── types.rs              # 统一消息类型（Message/Role/ToolCall/Usage/ToolSpec 等）
│   ├── sse.rs                # SSE 事件解析
│   ├── retry.rs              # 重试策略、可重试状态码、错误信息脱敏
│   ├── usage.rs              # 各协议 token 用量解析 → engine::UsageDelta
│   ├── openai.rs             # OpenAI chat/completions 协议（body 构造 + SSE 解析）
│   ├── anthropic.rs          # Anthropic messages 协议（body 构造 + SSE 解析）
│   └── responses.rs          # OpenAI Responses（codex）协议（body 构造 + SSE 解析）
└── tools/
    ├── mod.rs                # 模块声明与重导出
    ├── catalog.rs            # 工具清单（ToolSpec 定义，随请求声明给模型）
    ├── dispatch.rs           # execute_tool 分发 + ToolCtx（工作区/SSH/MCP/取消/待办/已读文件）
    ├── permission.rs         # 高风险/不透明命令分类与权限决策
    ├── question.rs           # 计划模式 AskQuestion
    ├── cancel.rs             # CancelFlag 原子取消标志
    ├── local.rs              # 本地工作区：读/写/编辑/glob/grep/bash
    ├── mcp.rs                # stdio MCP 客户端（本地 spawn / SSH 远端 spawn，失败跳过不回退）
    ├── ssh.rs                # SSH 工作区：通过 SSH 命令执行 read/write/glob/grep/bash
    ├── web.rs                # WebFetch / WebSearch（DuckDuckGo / Exa）
    ├── glob.rs               # glob 匹配实现（**、*、?）
    └── paths.rs              # 工作区路径解析与越界防护（本地 + POSIX/SSH）
```

## 模块职责

### 顶层

#### `mod.rs`
模块入口。声明 `agent`、`channels`、`images`、`manager`、`model`、`model_catalog`、`prompt`、`protocol`、`secret_store`、`session`、`settings`、`tools` 子模块，并重导出 `NativeAgentManager` 与 `session` 中供外部（`lib.rs`、任务自动化、run_queue）使用的函数：`list_live_native_employee_processes`、`run_native_one_shot`、`start_native_with_manager`、`stop_native_for_automation_restart`。

#### `manager.rs` — 内存态会话注册表
`NativeAgentManager`：以 `session_record_id` 为键的 `HashMap`，保存每个活跃会话的 `NativeLiveSession`（会话信息 + `CancelFlag` 取消标志 + `followup_tx` 输入通道 + 任务 JoinHandle）。提供按员工/任务维度查询（`has_employee_processes`、`get_employee_processes`、`get_task_process_any`）与增删会话的接口，用于去重启动、发送补充输入、停止会话。会话结束后由 `run_native_loop` 调用 `remove_session` 清理。

#### `session.rs` — 会话生命周期（最大文件，约 1000 行）
内置 Agent 与外部引擎对齐的核心接线层，对外暴露 Tauri 命令：

- `start_native_session`：启动会话。先经 `crate::run_queue` 做任务级门控（并发任务排队/插槽），再调用 `start_native_with_manager`。
- `start_native_with_manager`：核心启动流程 —— 校验同员工/同任务会话不重复 → 解析执行上下文（`engine::context::resolve_session_execution_context`，本地或 SSH）→ 校验工作目录 → 加载员工配置并组装 `ModelClient`（`load_native_client`）→ 插入 `codex_sessions` 记录 → 采集执行文件变更基线（`capture_native_execution_change_baseline`，本地/SSH 均支持）→ spawn `run_native_loop` → 注册到 manager → 发 `native-session` 事件、写活动日志。
- `run_native_loop`：会话主循环 —— 组装系统提示词（`prompt::compose_system`，含 Git 上下文、AGENTS.md、员工设定）、创建 `AgentRunner`、起两个转发任务（事件行 → `native-stdout`/session 事件表；用量 → `apply_codex_session_usage`）、循环处理首条提示词与 `followup_rx` 后续输入；结束时落库会话状态、持久化文件变更历史、发 `native-exit`、清理 manager、触发任务自动化（`handle_session_exit_blocking`）与 run_queue 排空。
- `run_native_one_shot`：员工级一次性调用（协调器计划、测试验收等场景）。仅 HTTP，无工具循环。思考模型若只回 `reasoning_content`、正文为空，则在思考内容像 JSON/Markdown 计划时采用，否则关思考再打一次。DeepSeek V4 必须显式 `thinking.type=disabled` 才能关掉默认思考。
- `stop_native_session` / `stop_native` / `finish_native_input`：停止单个会话、按员工停止全部、结束当前输入轮。
- `restart_native_session` / `resume_native_session`：停止后重新启动 / 以 `resume_session_id` 续接会话记录。
- `send_native_input`：向运行中的会话发送后续输入（`NativeFollowup::Input`）。
- `stop_native_for_automation_restart`：供任务自动化「重启步骤」使用，校验员工身份与会话标识后才停止。
- 辅助函数：`load_native_client`（员工 → 渠道 → 模型配置 → `ModelClient`）、`emit_native_line`（写 `codex_session_events` 并广播 `native-stdout`）、`resolve_run_model_config`（渠道模型配置缺失时回填模型目录）。运行时思考等级经 `resolve_runtime_reasoning_effort` 限制到渠道模型允许集合。

#### `transcript.rs` — 会话模型历史
`native_session_transcripts` 表保存每个 native 会话的模型消息快照（剥图片、sanitize tool 对）。`save_transcript` / `load_transcript` 只吃 `&SqlitePool`。续聊时用当前 `compose_system` 重建 system 提示词，再接上历史。崩溃后会话被标 failed，只要 transcript 仍在即可继续。

#### `settings.rs` — 设置持久化
将内置 Agent 设置保存到 `$APPCONFIG/native-settings.json`（结构体 `RawNativeSettings`）。字段包括最大模型工具轮次 `max_turns`（默认 40，0 表示不限制，上限 500）、高风险确认、确认超时 `permission_timeout_secs`（默认 300，0 表示不超时）、同轮子 Agent 并发上限（默认 1，范围 1–16）、子 Agent 策略（默认 conservative）、子 Agent 预算占比 `subagent_budget_share_percent`（默认 40，范围 5–100），上下文窗口上限 `context_window_tokens`（默认 128,000，范围 8,000–1,000,000）、会话 rollout 预算 `rollout_token_budget`（默认 10,000,000，0 表示不限制，上限 100,000,000）和单条工具结果上限 `max_tool_output_tokens`（默认 4,096，范围 256–65,536）。缺少新增字段的旧 JSON 会按保守默认值归一化。提供 Tauri 命令 `get_native_settings` / `update_native_settings`，修改时写活动日志（`native_settings_updated`）。

#### `channels.rs` — AI 渠道管理
管理 `ai_channels` 表（内置 Agent 的模型来源）。Tauri 命令：`list_ai_channels`、`create_ai_channel`、`update_ai_channel`、`delete_ai_channel`（被员工引用时拒绝删除）、`test_ai_channel`（发一条 probe 请求测通）、`list_ai_channel_models`（调用 `/v1/models` 拉取模型列表）。包含 API Key 的迁移逻辑：旧字段 `api_key_ref`（keyring 引用）自动迁移到 `api_key` 列（`hydrate_channel_record`、`require_channel_api_key`），模型配置写入时经 `normalize_channel_model_config` 回填目录默认值并校验思考等级。

#### `protocol.rs` — 协议与 URL 工具
协议归一化：`openai`（chat/completions）、`anthropic`（messages）、`codex`（OpenAI Responses）三套及别名。提供 `channel_chat_url` / `channel_models_url` 构造端点、`normalize_base_url`、`normalize_extra_headers_json`（额外请求头）、模型列表 JSON 解析与分页（`parse_model_list_json`、`model_list_next_page`，兼容 OpenAI/Anthropic/网关形态）、`ChannelModelConfig` 的解析/序列化（`parse_channel_models_json` / `serialize_channel_models`，去重、回填目录），以及 `record_to_channel` 将数据库记录转为 UI 用的 `AiChannel` DTO（含 `api_key_configured`，不回传密钥引用）。

#### `secret_store.rs` — 系统凭据库
通过 `keyring` 在系统凭据服务（服务名 `codex-ai-channel`）中读写渠道 API Key：`resolve_channel_api_key`、`delete_channel_api_key`，并把「Secret Service 不可用」等平台错误映射为面向用户的中文提示。

#### `images.rs` — 图片附件
`load_native_images`：把本地图片路径列表加载为 `NativeImage`（base64 + 按扩展名猜 mime）；限制最多 8 张、单张 ≤ 8MB，缺失/超限分别记录到 `missing` / `skipped`；`image_log_lines` 生成会话日志行。

#### `model_catalog.rs` — 内置模型目录
以 `include_str!` 内嵌 `model_catalog.json`（编译期校验可解析）。`lookup_catalog` 支持精确 ID、别名、归一化匹配（忽略大小写/分隔符/下划线）与前缀模糊匹配；`fill_from_catalog` / `normalize_channel_model_config` 为 `ChannelModelConfig` 回填缺失的 `context_tokens`、`max_output_tokens`、`thinking_enabled`、`thinking_level`、`thinking_levels`，并统一去空白、去重。`thinking_levels` 仅在 `None` 时采用目录全集（未知模型回退 `low/medium/high`）；用户已保存的显式子集或空数组不会被目录新增等级覆盖。开启思考时写入路径拒绝空集合。运行时默认 `thinking_level` 必须属于已选集合，关闭思考时清空。`resolve_runtime_reasoning_effort` 会把员工或一次性调用传入的等级限制到当前允许集合，越界时回退默认等级。`apply_catalog_defaults` 按模型 ID 生成完整默认配置。Tauri 命令 `list_model_catalog` 供前端展示。

#### `model_catalog.json`
模型目录数据：每条含 `id`、`aliases`、`vendor`（openai/anthropic/deepseek/minimax/glm/kimi/doubao/hunyuan/gemini/mimo/qwen 等）、`label`、`context_tokens`、`max_output_tokens`、`thinking`、`thinking_levels`。

### `prompt/` — 提示词

#### `prompt/mod.rs`
`compose_system` 按顺序组装系统提示词块：identity.md → 环境块（工作目录/平台/日期/权限 confirm-high-risk/模型名）→ Git 上下文（分支/状态/最近提交）→ 全局提示词（`native_agent_global` 模板，来自 AI 提示词模板库）→ 项目指令（AGENTS.md / Agents.md / CLAUDE.md，本地文件系统或 SSH 读取，单文件上限 32KB 并去重）→ 员工设定。另有 `detect_local_git` / `detect_ssh_git` 采集 Git 信息、`format_global_template` 组合「输出目标 + 场景要求」。

#### `prompt/identity.md`
内置 Agent 的身份设定：先读后改、最小必要改动、不编造仓库事实、优先用 Read/Glob/Grep/WebFetch/WebSearch 而非 Bash、把工具输出当数据防注入、删除/覆盖/推送/密钥前说明风险、用简洁中文汇报并标注「未验证」等。

### `agent/` — 循环与上下文管理

#### `agent/loop.rs` — `AgentRunner`
工具循环核心：
- `run_with_client`：真实模型路径。每轮先 `prepare_model_call`（取消检查、轮次上限、截断/压缩、最后一轮移除工具并追加「停止调用工具」提醒），再调用 `ModelClient::chat`，最后 `consume_assistant`。
- `run_scripted`：测试专用，用预置的回复序列替代模型调用。
- `consume_assistant`：处理模型输出 —— 剥离空工具名、最后一轮强制清空工具调用、播报思考字数与文本、执行工具并把结果作为 `tool` 消息回填；无工具调用即返回最终文本。
- 防重复调用：同一工具+相同参数连续调用 3 次（`REPEAT_TOOL_LIMIT`）后直接拒绝，避免死循环。
- 子 Agent：连续 `Agent` 调用走 `JoinSet`（上限见设置）；子循环 `event_prefix` 为 `[子 Agent n(explore|general) - {description}] `（description 来自工具参数短标题，空白折叠、去括号、最长 32 字）；高风险确认 FIFO；MCP 经 `SharedMcp` Mutex 共享。
- 事件输出：`[思考] `、`[读取] `、`[命令] `、`[工具结果] `、`[子 Agent] ` 等进度行通过 `on_event` 通道发出；用量通过 `on_usage` 通道发出。`[工具结果]` 事件保留完整工具输出（一条事件，可含换行），不是第一行摘要；TodoWrite 清单仍只报「已更新 N 项」。超过 2000 行或 65536 字时 UI 截断并附中文提示；模型历史使用按 token 限制的头尾片段。

#### `agent/compact.rs`
上下文窗口与预算管理：`ContextWindow` 按序列化消息 Token 估算，在达到 85% 阈值时优先用当前模型发起无工具结构化摘要，失败或预算不足时回退本地摘要/窗口重置；`RolloutBudget` 由父 Agent 与所有子 Agent 共享并原子预留、结算，达到预算后停止新的工具轮次。每次会话结束写入 `native_token_diagnostics` 诊断事件，包含压缩、重置、截断、子 Agent 和预算停止计数。

#### `agent/truncate.rs`
消息 Token 估算（正文、思考、工具调用、图片和工具 schema）与超长工具结果截断。模型历史中的每条工具结果默认限制 4,096 token，保留头尾并附路径/offset 继续读取提示；`truncate_messages_tokens` 在发送前再执行总窗口保护。旧的字符预算 `truncate_messages` API 保留给兼容调用者，UI/session 事件仍走展示截断而不改变模型消息。

### `model/` — 模型客户端与协议

#### `model/mod.rs`
模块声明，重导出 `ModelClient`、`ModelClientConfig`、`RetryConfig`、`usage_to_delta`。

#### `model/client.rs` — `ModelClient`
统一的 HTTP 模型客户端：
- `chat`：按协议构造请求体 → `post_stream` 流式拉取 → `parse_success_body`（SSE，失败再解析完整 JSON）解析为统一 `Message` + `Usage`；Responses/Codex 协议在历史前缀未变化时携带 `previous_response_id`，只发送新增 input，压缩或网关拒绝时自动失效并回退完整历史。若网关报「max_tokens 超限」，自动从错误信息解析上限并降级重试一次（`parse_max_output_token_limit`）。HTTP 200 的 `error` 对象要报「模型返回错误」，不得叫空响应。
- `chat_stream`：把一次 chat 结果拆成 `StreamEvent` 序列（文本/思考/工具调用/用量/Done），供流式消费。
- `list_models`：`/v1/models` 分页拉取（最多 500，去重、排序）。
- `probe`：发送最小请求测通渠道。
- `post_raw` / `get_raw` / `apply_auth`：鉴权（Anthropic 用 `x-api-key` + `anthropic-version`，其他用 `Authorization: Bearer`），支持额外请求头（禁止覆盖 authorization/x-api-key），重试由 `RetryConfig` 驱动（默认最多 10 次、固定 3s、无抖动；`none` 用于探测）。HTTP 2xx 网关错误/空响应也可重试；401/4xx/配额不重试。重试前经 `on_retry` 打 `[重试]` 行，等待期可响应 `CancelFlag`。
- `parse_success_body`：SSE（含缺空行的完整 JSON `data:` 行）失败时解析非流式 JSON（可剥一层 `data`/`result`）；仍空则错误带脱敏正文摘要。

#### `model/types.rs`
统一的内部消息模型：`Role`（system/user/assistant/tool）、`Message`（content、tool_calls、tool_call_id、name、reasoning_content、images）、`ToolCall`、`NativeImage`（data_url 生成）、`Usage`（prompt/completion/cached tokens）、`StreamEvent`、`ToolSpec`，及常用构造器（`Message::system/user/user_with_images/assistant_text/tool_result`）。

#### `model/sse.rs`
SSE 文本解析：把 `event:` / `data:` 块解析为 `SseEvent` 列表，`[DONE]` 归一为 `done` 事件，忽略注释行；完整 JSON / `[DONE]` 的 `data:` 行即使中间没有空行也单独成事件（兼容不按 spec 分帧的中转）。

#### `model/retry.rs`
重试配置 `RetryConfig`（默认最多 10 次、固定 3s、无抖动；`none` 用于探测请求）、`delay_for_attempt`（`max_delay_ms` 封顶后为固定间隔）、可重试状态码（408/409/429/5xx）与 `is_retryable_error`（网络/空响应/网关抖动可重试，401/配额/其它 4xx 不重试）、`format_retry_line`（`[重试] …第 n/10 次重试`）、错误信息脱敏（`redact_secrets` 抹掉 `Bearer` 与 `sk-` 令牌）与 `format_http_error` 友好错误文案。

#### `model/usage.rs`
`parse_usage`：兼容 OpenAI（prompt_tokens/completion_tokens/prompt_tokens_details.cached_tokens）、Anthropic（input_tokens/output_tokens/cache_read_input_tokens）与 Responses 的用量字段；`usage_to_delta` 转换为引擎通用的 `engine::UsageDelta`。

#### `model/openai.rs`
OpenAI chat/completions 协议：`build_openai_body`（stream + stream_options.include_usage、tools/tool_choice=auto、reasoning_effort、max_tokens 与 thinking 时的 max_completion_tokens、多模态 image_url 内容）、`openai_messages` / `openai_tools`、`parse_openai_sse` / `parse_openai_json`（增量或完整 `choices[].message`，含 reasoning 对象与 content 数组）、`parse_max_output_token_limit`（从网关错误正文解析支持的最大输出 token）、`normalize_effort`（minimal/low/medium/high/xhigh/max）。

#### `model/anthropic.rs`
Anthropic messages 协议：`build_anthropic_body`（system 提取、thinking 开启时按 effort 计算 budget 并抬高 max_tokens、tools 用 input_schema、图片用 base64 source）、`anthropic_messages` / `anthropic_tools`、`parse_anthropic_sse` / `parse_anthropic_json`（content_block_start/delta、非流式 content[]，含 thinking）、`thinking_budget_tokens`（effort → token 预算）。

#### `model/responses.rs`
OpenAI Responses（codex）协议：`build_responses_body`（instructions/input、function 工具、reasoning.effort、max_output_tokens、input_image 内容）、`responses_input` / `responses_tools`、`parse_responses_sse_with_id` / `parse_responses_json_with_id`（读取 response id，支持 `previous_response_id` 续接；同时兼容 output_text.delta/done、function_call、completed.output 正文）。

### `tools/` — 工具运行时

#### `tools/mod.rs`
模块声明，重导出 `CancelFlag`、`tool_specs`、`execute_tool`、`ToolCtx`、`LocalWorkspace`。

#### `tools/catalog.rs`
工具声明 `tool_specs()`：Read、Write、Edit、Bash、Glob、Grep、TodoRead、TodoWrite、WebFetch、WebSearch、Agent。`Agent` 仅父循环（depth=0 且非只读）注入，用于会话内委派；`explore` 只读，`general` 可写。同一轮连续 `Agent` 调用并行，上限为 `native-settings.json` 的 `max_concurrent_subagents`（默认 1）。委派勤快程度由 `subagent_policy`（conservative / balanced / aggressive，默认 conservative）写入系统提示，不强制调工具。子 Agent 类型仍只有 explore / general。子循环共享父 Agent 的 `RolloutBudget`，不占 run_queue、不新建 `codex_sessions`。

#### `tools/dispatch.rs` — `execute_tool` 分发
`ToolCtx` 保存会话工具状态：工作区（本地或 SSH）、取消标志、已读文件集合（先读后写校验）、待办列表。`execute_tool` 按名称分发到具体实现；每个工具都先判取消、解析 JSON 参数（`parse_args` / `string_arg`），并区分本地（`LocalWorkspace`）与 SSH（`SshToolRuntime`）两条执行路径；写/编辑类工具要求目标文件已先被 Read。

#### `tools/cancel.rs`
`CancelFlag`：基于 `Arc<AtomicBool>` 的原子取消标志，`cancel` / `is_cancelled`，在循环与 Bash 执行中协作检查。

#### `tools/local.rs` — 本地工作区
`LocalWorkspace`（root 目录）：
- `read_file`：读取 + 行号输出（`format_read`，默认 2000 行、offset/limit），拒绝目录。
- `write_file`：自动创建父目录后写入。
- `glob_files`：递归遍历（跳过 .git）后用 `glob_match` 过滤，最多返回 100 条。
- `grep_files`：逐行子串匹配（限制 1..1000 条），可按 glob 过滤文件。
- `bash`：`bash -lc` 在工作区执行（默认 120s、上限 600s、支持取消与 kill_on_drop），stdout+stderr 合并后截断到 3 万字符（`cap_model_output` 保留尾部）。
- `apply_edit`：精确字符串替换，old_string 非唯一时报错（除非 replace_all）。

#### `tools/ssh.rs` — SSH 工作区
`SshToolRuntime`：基于既有 SSH 执行通道（`app::execute_ssh_command`）把工具映射为远程命令 —— `cat`（read）、`mkdir -p && cat >`（write）、`find . -type f | head -n 500`（glob，配合本地 `glob_match` 过滤）、`rg -n || grep -R -n`（grep）、`cd <root> && bash -lc`（bash）。所有路径先经 `resolve_under_workspace_posix` 防越界，命令参数经 shell 转义。

#### `tools/web.rs`
`web_fetch`：只允许 http/https，GET 拉取（30s 超时、512KB 上限、最多 5 次重定向），去 HTML 标签后截断到 8 万字符；`web_search`：优先使用 Exa（环境变量 `EXA_API_KEY`），否则解析 DuckDuckGo HTML 结果，最多 10 条、截断到 2 万字符。

#### `tools/glob.rs`
`glob_match`：支持 `**`（跨目录）、`*`、`?`（不匹配 `/`）的 glob 匹配，`/` 与 `\` 归一化处理。

#### `tools/paths.rs` — 工作区路径边界
`resolve_under_workspace`：把用户/模型给的路径解析到工作区下（绝对路径或相对路径），归一化 `.` / `..` 并拒绝逃逸出工作区的路径；`resolve_under_workspace_posix` 提供 SSH/POSIX 版本。

## 数据流

```
前端 (Tauri command)
  │
  ├─ session.rs  start/stop/restart/resume/send_input …
  ├─ channels.rs create/update/delete/test/list_models
  └─ settings.rs get/update_native_settings
        │
        ▼
  session.rs ──► manager.rs（注册表）
        │             │
        └─► run_native_loop ──► prompt::compose_system
                                   │
                                   ▼
                              AgentRunner（agent/loop.rs）
                                   │ 每轮
                                   ├─ model/client.rs → 协议适配（openai/anthropic/responses）→ HTTP 渠道
                                   └─ tools/dispatch.rs → local / ssh / web 工具执行
                                   │
                                   ▼
                        事件/用量 → codex_session_events + native-stdout/native-exit 事件
                                   ▼
                        codex_sessions 状态、文件变更历史、任务自动化、run_queue
```

## 依赖关系说明

- 复用 `crate::engine`（`UsageDelta`）、`crate::codex`（会话记录/事件/文件变更基线/提示词模板库）、`crate::task_automation`、`crate::run_queue` 等既有基础设施，本模块不重复造轮子。
- Tauri 命令统一注册在 `src-tauri/src/lib.rs` 的 `invoke_handler!`：渠道 5 个、模型目录 1 个、会话 7 个、设置 2 个。
- 渠道 API Key 优先存 `ai_channels.api_key` 列；历史 `api_key_ref`（keyring）在读取时自动迁移。
- 工作区安全由 `tools/paths.rs`（本地）与 `tools/ssh.rs`（远程）双层保障：所有文件工具都禁止逃逸工作区，写/编辑前强制先 Read。
