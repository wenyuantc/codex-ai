# 任务列表

# 下一波 · 2026-08-24 五引擎时代：设备补齐 + 信任兑现

> 依据：本文件由 PM 视角代码实读（2026-08-24，v0.6.1 基线），每条附可复核证据；参考 `docs/analysis/09-product-gap-2026-08-20.md`（已全部落地）。
> 主题一句话：**第五引擎内置 Agent（native）已跑通「模型→工具→会话」主循环，但它只有手脚没有装备（MCP / 子 Agent / 交互式权限 / Skills），而文档还停在四引擎时代。**
> Trellis 本波 P0：`.trellis/tasks/08-24-engine-ecosystem/`（3 子任务：MCP 对齐、native 高风险权限、Claude 图片）。子 Agent / plan 模式 / Skills 不进本波。

## 背景：上一波之前的增量（已落地，08-20/21）

- 08-20 「可信 + 可运营 + 好找」A1–C2 八项全部归档（员工状态语义、图片诚实、队列运营面、会话 token、启动检查更新、四引擎引导、i18n 两处、计时器去伪）。
- 08-20/21 **内置 Agent（native）** 五子任务落地：AI 渠道 CRUD + 三协议（OpenAI/Anthropic/Responses）+ 10 个工具（Read/Write/Edit/Bash/Glob/Grep/TodoRead/TodoWrite/WebFetch/WebSearch）+ 本地/SSH 双工作区 + 模型目录 + token 用量 + 会话来源类型（执行/审核/编排）+ 链路缓存用量展示。
- **主矛盾再换**：不再是「能不能跑」或「跑得起跑不起成本」，而是 **五个引擎能力是否一致、native 是否配得上「自研 Agent」的承诺、文档是否跟上**。

## P0 · 引擎生态对齐（配了要能用，不能「配了=没配」）

> Trellis：`.trellis/tasks/08-24-engine-ecosystem/` · P0 三项已实现（MCP 对齐 / native 高风险确认 / Claude CLI 本地传图）

- [x] **MCP 实际只服务 Codex ⭐⭐⭐**（`08-24-p0-mcp-align`，已改为 Codex + native 真执行；Claude/Grok/OpenCode 诚实声明不执行）：MCP 引用全在 `codex/`（`codex/mcp.rs` + `codex/process` 共 87 处）；`claude/`、`grok/`、`opencode/`、`native/` 对 MCP **零命中**。设置页 `McpSettingsTab` 与任务级三态绑定（`TaskMcpBindingSection`）生成的 `mcp-servers.json` 只被 Codex 启动路径消费。→ 用户给 Claude/Grok/OpenCode/native 员工配 MCP 是假功能，违背上波「已交付能力要说真话」。方案二选一：A) UI 明确「MCP 仅 Codex」并禁选；B) **建议** native 工具目录加 MCP 工具注入（`tools/` 分发 + SSH 通道已就绪，成本可控），外部引擎逐步跟随。

- [x] **native 缺「自研 Agent 该有的装备」⭐⭐⭐**：
  - [x] **交互式权限确认**（本波只做这一项：`08-24-p0-native-tool-permission`）：工具执行全 yolo（`native/prompt/identity.md` 只约定「先说明风险」），Write/Bash/删除/推送无用户确认——与本产品「可信」主线正面冲突。建议先做「高风险工具（删除/覆盖/推送/强制 git 操作）确认」，成本低信任收益最大。
  - [x] **子 Agent**（`08-24-native-subagent`）：会话内 `Agent` 工具（`general` / `explore`），同轮连续调用并行（上限 3），depth=1，不新建会话、不占队列。
  - [x] **自定义子智能体**（`08-25-native-custom-subagents`）：设置 Tab 配置名称/模型/工具/系统提示词/AGENTS.md；`subagent_type=<名称>` 委派。
  - [x] **plan 模式**（`08-25-native-plan-mode`）：看板右键「计划运行」（仅内置 Agent 执行人）；同一会话先只读出计划，计划轮结束自动执行。不替代协调员，不改任务详情「AI 生成计划」。
  - [x] **续聊/权限/资源收口**（2026-08-31）：transcript 真续聊、Opaque+超时+MCP 按服务器、图片 token 与子 Agent 配额、plan_content 服务层、任务会话中途纠偏。
  - [x] **Skills / Hooks / ApplyPatch / Browser**（`09-01-native-equipment`）：工作区+全局 SKILL.md 发现与 Skill 工具；`native-settings.json` hooks（Pre 退出码 2 阻断）；ApplyPatch 信封补丁（本地/SSH）；浏览器走 Playwright MCP 预设，不内置 CDP。

- [x] **图片附件引擎间仍不对齐 ⭐⭐**（`08-24-p0-claude-images`，本地 Claude CLI 已补齐 stream-json 传图；SSH 仍跳过）：native 支持 ≤8 张（`native/images.rs`）；Codex/Grok/OpenCode 本地支持、SSH 跳过（已诚实提示）；**Claude CLI 本地也跳过**（`claude/process/mod.rs:1240-1250`）。→ Claude 本地图片补齐，或在员工绑定页面明确「该引擎不支持图片」。

## P1 · 信任与运营深化

- [ ] **无定时/计划执行 ⭐⭐**：全库 `cron/schedule` 零命中。「每日验收」「下班后自动审核」只能手动。此前明确不做；native + `run_queue` + `task_automation` 就绪后建议重估，MVP 只做「定时启动任务」（cron 表达式绑定看板任务），不做复杂触发器。
- [ ] **无真实工时维度 ⭐⭐**：计时器上波去伪后已隐藏（`TaskPropertiesSidebar.tsx:123-137` 只读），员工绩效只有完成数/平均完成时间/成功率（README 员工绩效节），任务无「预估/实际工时」字段与工时报表。
- [ ] **看板无迭代与依赖可视化 ⭐⭐**：5 列固定、无 Sprint/迭代/泳道/自定义列（全库 `sprint|gantt|泳道` 零命中）；`task_dependencies` 仅列表 UI（`TaskDeliverySection` 等），无依赖图/关键路径视图。
- [ ] **通知不可外联 ⭐**：仅应用内 + 系统通知；无 Webhook/IM（企业微信/飞书/Slack/钉钉）推送，任务完成/审核待处理无法远程触达。可做成设置页可选渠道，低优先级。
- [ ] **数据导入不完整 ⭐**：任务 JSON 导入已有（`importTasksJson`）+ CSV/JSON 导出；缺任务 CSV 导入与项目/员工导入，从 Jira/Excel/其他工具迁移冷启动困难。

## P2 · 体验与可发现性

- [ ] **无内置帮助中心**：页面与组件「帮助/文档」零命中；角色说明散落在员工页。→ 设置页「帮助」tab 或扩展现有 `⌘?`（`shortcuts.ts`）为产品手册入口。
- [ ] **i18n 扫尾**：`08-10-p3-i18n/leftovers.md` 的触发条件（U1 send_input 落地）已满足；Git 对话框、设置正文、任务详情深面板仍大量硬编码中文。
- [ ] **报表缺运营指标**：仪表盘已有趋势/燃尽（R1）；缺引擎维度对比（token/会话数）、队列等待时长、审核通过率/修复轮次等指标与导出。
- [x] **文档漂移已复发**：README 对 native 零命中、设置页章节仍写「6 个标签页」（实际 7，含 channels）、引擎矩阵 4 行缺 native；CLAUDE.md 仍称四引擎 / 227 命令（native 新增 15 个）/ 26 表 / 47 迁移 / 417 测试。08-11 刚校准过又漂 → 建议把「文档=真源」检查脚本化（数据点由代码生成，接入 CI）。2026-08-31 已再校准为 258 命令 / 30 表 / 56 迁移 / 701 测试；2026-09-01 再校准为 260 命令 / 728 测试（native 技能目录 list/open）。脚本化仍待做。

## P3 · 技术债（沿用，未关闭且在涨）

- [ ] 拆分 `TaskDetailDialog`（2014→**2022** 行）/ `TaskCard`（1989 行）
- [ ] 后端热点：`app/tasks.rs` 3096 / `opencode/process/mod.rs` 2957 / `app/remote.rs` 2450（行数持续增长）
- [ ] 前端 82 测试全在纯函数；组件与交互层零回归网；无 e2e

## 建议排期（可拆子任务，一次一个）

1. P0-1 MCP 对齐：先 native MCP 工具注入 + UI 能力声明（外部引擎随后）
2. P0-2a native 高风险工具权限确认（删除/覆盖/推送）
3. P0-3 Claude 图片声明/补齐（低成本）
4. P1-4 定时启动 MVP（需用户拍板重估「定时 cron」不做项）
5. P1-6 依赖图/关键路径视图（低成本高感知）
6. P2-12 文档校准脚本化（随代码改动必跑）

## 明确不做（沿用路线图，本轮不重开）

微服务拆分 · 远程多端实时同步 · 完整 IDE（LSP/调试器）· Jira/GitHub Issues 双向同步 · token→金额换算 · hunk 级暂存 · Grok send_input（B1 豁免）· ChatGPT 网页 Codex 后端 · 定时 cron（P1-4 建议重估，需用户确认）

> 说明：原「第五个 AI 引擎」不做项已被 08-20 内置 Agent 交付，从非目标清单移除。

---

# 上一波 · 2026-08-20 可信 + 可运营 + 好找

> 依据：[docs/analysis/09-product-gap-2026-08-20.md](docs/analysis/09-product-gap-2026-08-20.md)
> 主题：**已落地的能力要说真话、找得到、管得住**
> Trellis：`.trellis/tasks/08-20-product-trust-ops/`（8 子任务）

## T-P0 · 可信

- [x] **员工状态语义纠偏（A1）**：`employees.status` 是运行时心跳，空闲写成 `offline`（`employeeStore.ts:117`）。仪表盘「在线员工」只计 `online/busy`（`database.rs:1625`），全员空闲时读成全不在线。文案改为空闲/运行中/异常。**禁止**按 offline 拦截启动（会卡死空闲员工）。原「暂时不处理·离线禁跑」关闭，不提升为拦截。
- [x] **图片附件诚实提示（A2）**：SSH 下 Codex/Grok/OpenCode 跳过本地图片只刷 WARN（`session_launch.rs:688` 等）；Claude CLI **本地也跳过**（`claude/process/mod.rs:1240`）。运行前 UI 警告，不实现传图。

## T-P1 · 可运营

- [x] **运行队列运营面（B1）**：队列已持久化，但只有卡片「第 N 位」+ 取消。看板提供队列列表；批量运行列出每条跳过原因，不只 started/queued/skipped 三个数。
- [x] **会话页 token 可见（B2）**：token 已落库且任务详情/仪表盘可见；`SessionsPage`/`SessionCard` 零展示。卡片+表格显示用量，无值=未知不假装 0。不做金额换算。
- [x] **启动时检查更新（B3）**：更新能力只在设置关于节手动点。启动静默检查 + 通知，安装仍需确认。不改发版签名。
- [x] **引导四引擎健康检查（B4）**：`OnboardingChecklist` 只调 Codex `health_check`。只用 Claude/Grok/OpenCode 时该步不应永远红。

## T-P2 · 好找 / 去伪

- [x] **看板右键与搜索 i18n（C1）**：`TaskCard` 右键硬编码「归档/合并/AI 提交/AI 解冲突」；全局搜索 `TYPE_LABELS` 写死中文。本波只收这两处，不扫全库 leftovers。
- [x] **工时计时器去伪（C2）**：侧栏展示未开始/暂停但无开始暂停按钮。空闲隐藏，或做成可操作；禁止假功能。

## T-P3 · 技术债（不进本波）

- [ ] **拆分 `TaskDetailDialog`（2014 行）/ `TaskCard`（1989 行）**
- [ ] 后端热点：`app/tasks.rs` 3096 / `opencode/process/mod.rs` 2957 / `app/remote.rs` 2460
- [ ] 前端测试仍只覆盖 `src/lib` + `src/stores` 纯函数，组件与交互层零回归网

### 本波明确不做

- **按 `offline` 拦截启动** —— 空闲就是 offline
- **hunk 级暂存 / token→金额 / Grok send_input / 定时 cron**
- 路线图级不做项照旧（微服务/多端同步/第五引擎/完整 IDE/Issues 同步）

---

# 上一波 · 2026-08-11 产品缺口（已完成，保留备查）

> 依据：[docs/analysis/08-product-gap-2026-08-11.md](docs/analysis/08-product-gap-2026-08-11.md)（代码实读，每条附 `文件:行号` 证据）
> 主题：**AI 跑起来之后，用户看不见成本、管不住并发、复用不了经验**

## N-P0 · 可度量 + 可批量

- [x] **成本可见性（A1）**：`codex_sessions` 无任何 token 消耗列（v43 的 `thinking_budget_tokens` 是预算上限不是消耗）；全库唯一 usage 解析在 `grok/process/stream.rs:812` 且只刷日志不落库。补 migration v45 加 `input/output/total/reasoning_tokens`（nullable，无值即未知不假装 0）→ 四引擎 `process/stream.rs` 解析 → 任务详情 + 仪表盘趋势。**一期只做 token 用量，不做金额换算**（各引擎计费口径不同，会误导）
- [x] **并发闸门 + 运行队列（B1）**：后端 grep `semaphore`/`max_concurrent`/`cron` 零命中，但产品支持员工多任务并发 = 同时 N 个 AI 进程裸奔。在 `engine/` 共享内核加全局并发上限（设置页可配，默认 2–3）+ 超限排队 + 看板「排队中（第 N 位）」+ 批量运行选中任务。**队列状态需持久化**，否则重启丢失（参照 `task_automation_state` 的 resume 模式）

## N-P1 · 低成本高频

- [x] **会话日志复制/导出（A2）**：`CodexTerminal.tsx` 有虚拟化/过滤/清空/输入条，但无复制无导出，跑挂了只能截图
- [x] **应用自动更新（D1）**：`Cargo.toml` 无 `tauri-plugin-updater`；CI 已在 tag 打好三平台包，但用户永远不知道有新版。发版链路最后一公里
- [x] **文档校准（D2）**：README 引擎矩阵与 `app/database.rs` 真源矛盾（Claude/OpenCode 的 send_input 与 restart 都写错）、表数/迁移数/命令数/测试数四项失真、`app/session_events_retention.rs` 整个模块未被任何文档记录 —— 2026-08-11 已修正

## N-P2 · 复用与审查深度

- [x] **任务模板（B2）**：看板「模板」+ 右键存为模板；`{{ident}}` 批量套用（≤100）；标签按名 find-or-create，子任务标题复制为待办
- [x] **审查行级定位（C1）**：`<review_findings>` 落库；任务详情列表点击打开 Monaco Diff 并跳到修改后一侧对应行；畸形输出降级为原报告

## N-P3 · 技术债

- [ ] **拆分 `TaskDetailDialog`（2014 行）/ `TaskCard`（1929 行）**：`01-domain-capability-matrix.md` §7 自认「未关闭且加剧」，是所有任务操作入口
- [ ] 后端新热点：`app/tasks.rs` 3096 行 / `opencode/process/mod.rs` 2888 行 / `app/remote.rs` 2460 行
- [ ] 前端测试仍只覆盖 `src/lib` + `src/stores` 纯函数（9 文件 104 断言），组件与交互层零回归网

### 本波明确不做（新增）

- **hunk 级暂存** —— AI 场景下整文件接受/回滚是主流，自研 patch 编辑器成本高收益低
- **token → 金额换算** —— 各引擎计费口径不同，一期只做用量

---

# 上一波 · 2026-08-10 收口（保留备查）

## P0 · 主路径可用（先让已有能力被用上）

- [x] 首次使用引导：仪表盘/空项目态提供 checklist（SDK 健康检查 → 可选配置 SSH → 创建项目 → 创建员工 → 创建并运行首个任务），每步可跳转到对应页面/弹窗
- [x] 测试员自动化可发现：设置页与看板空态提示「任务完成后自动验收」开关位置；评估提供「推荐开启」引导，避免默认关闭导致用户以为测试员无用
- [x] 任务依赖真正生效：依赖任务未完成时，拦截运行/标记完成（不仅 Overview 文案警告）；给出可理解的拦截原因
- [x] 同员工多任务运行中 CTA 文案澄清：A 任务在跑、B 任务未跑时，B 不应笼统显示「其他任务运行中」；改为可区分「本任务运行中 / 同员工其他任务运行中 / 可运行」的文案与按钮态（从「暂时不处理」提升）
- [x] 仪表盘报表加载失败可见：`getDashboardReportSummary` 失败时展示错误/重试，禁止静默吞错变成空白

## P1 · 可信与交付完整

- [x] 备份/恢复语义补全：除 SQLite 外，说明或打包 prompt 模板、MCP 配置、任务附件策略；恢复后用户不会误以为「一切已还原」
- [x] SSH 下 review / 自动 commit / diff 产物尽量对齐 local；做不到的动作在 UI 禁用并说明原因（已有降级提示基础上补齐行为边界）
- [x] 已有后端未挂 UI：任务 Git 冲突 AI 解决（`aiResolveTaskGitConflicts`）、里程碑编辑（`updateMilestone`）、标签删除（`deleteTag`）补上入口或删除死导出
- [x] Claude SSH：补远程安装/健康检查，或在设置页明确标注「Claude 仅本地」
- [x] 角色产品说明：员工页或帮助入口写清开发/审核/测试/协调员职责，以及测试员自动化、协调员编排相关开关位置
- [x] 看板续聊链路复核：任务详情/会话链「继续对话」的停止、内容落库、与 Session 管理页行为一致（原「暂时不处理·继续对话」——入口已有，改为完整性复核）

## P2 · 体验与可维护

- [x] 拆分 TaskCard / TaskDetailDialog：已抽出 TaskCardPrimaryCtaStrip；详情侧复用既有 detail panels
- [x] 看板性能：虚拟化阈值 25→15；TaskCard 已 memo
- [x] Projects / Employees / Kanban 空态加强：说明文案 + 创建 CTA
- [x] 前端测试网加深：kanbanFilters / projects / taskPrimaryCta 单测
- [x] 收紧权限面：移除 shell:allow-execute/kill/stdin-write；校准 docs/analysis/README 漂移说明

## P3 · 战略扩展（暂缓，除非单独立项）

- [x] 真·会话中 send_input（已落地：Codex/Claude/OpenCode 走 SDK bridge 保留 stdin；Grok 为 headless CLI，B1 豁免保持 false）
- [x] 任务 CSV 导出 UI（仪表盘已增加导出入口）
- [x] i18n 完整框架（zh-CN + en；主路径已抽取；深对话框/Git/设置正文见 leftovers，U1 后再扫）
- [x] 更强报表（仪表盘 R1：可配 7d/30d/8w 趋势 + 里程碑剩余序列；Issues 同步仍明确不做）
- [ ] 外部 Issues 同步（明确不做，保留条目防回潮）

### 明确不做（路线图级）

- 微服务拆分、远程多端实时同步、第五个 AI 引擎、完整 IDE（LSP/调试器）、Jira/GitHub Issues 双向实时同步

---

历史账本（上一迭代已勾选，保留备查）：

- [x] 检查运行任务，AI优化提示词、协调员计划、AI助手使用 Codex、Cluade、OpenCode、Grok是否再项目目录执行？
- [x] 看板-创建任务增加按钮立即执行，点击立即执行后，如果判断有协调员，自动在后台生成协调员执行计划，在执行。没有就直接执行.
- [x] 如果任务在已完成状态，有时候是手动移动到已完成的，但是自动质控-人工处理状态，这样没有提交代码的按钮，你觉得，如果这个任务在已完成状态直接检查是否有提交的代码，有的话就显示提交代码的按钮、还有这个情况下还要增加一个AI提交按钮，在AI提交如果有冲突自动解决冲突然后提交
- [x] 看板任务运行 协调员编排失败后重启 app，失败状态丢失且「重试失败步骤」失效
- [x] 看板任务运行 协调员编排未完成时点击转人工，编排步骤增加「手动运行」按钮
- [x] 测试员目前没有任何作用，希望任务做完，自动化测试
- [x] 看板管理-> 里程碑、标签、批量修改状态、筛选选择后显示中文不要显示英文
- [x] 看板管理->归档管理 可以查看任务详情，但是不能编辑

# 进行中

（无）

# 已完成的
- [x] 2026-09-01 native 装备补齐（`09-01-native-equipment`）：Skills / Hooks / ApplyPatch / Browser Playwright MCP 预设
- [x] 2026-08-31 native 缺陷修复 #2–#12（不含流式）：真续聊 transcript、权限硬化（Opaque/超时/MCP 按服务器）、图片 token 与子 Agent 配额、plan_content 走服务层、任务会话中途纠偏、稳定文案 i18n
- [x] 2026-08-25 native 看板计划运行（`08-25-native-plan-mode`）：右键「计划运行」，计划轮结束自动执行
- [x] 2026-08-25 native 自定义子智能体（`08-25-native-custom-subagents`）：设置 Tab + `subagent_type` 委派
- [x] 2026-08-24 native 会话内子 Agent（`08-24-native-subagent`）：`Agent` 工具 general/explore，同轮并行上限 3，depth=1
- [x] 2026-08-24 产品缺口分析：五引擎时代（本文件顶部，待排期）
- [x] 2026-08-21 会话来源类型（执行/审核/编排）与链路缓存用量（`08-21-session-origin-type`、`08-21-chain-token-usage`）
- [x] 2026-08-20 内置 Agent（native）五子任务：渠道、模型客户端三协议、工具循环、引擎接入、UI（`08-20-native-agent*`）
- [x] 2026-08-20 产品下一波：员工状态语义、图片附件诚实、队列运营面、会话 token、启动检查更新、四引擎引导、i18n 收口、计时器去伪（`08-20-product-trust-ops`）
- [x] 2026-08-12 产品缺口下一波：token 用量、并发队列、日志导出、自动更新、任务模板、审查行级定位（`08-12-product-gap-wave` 已归档）
- [x] 2026-08-10 P2/P3 收口：空态 CTA、CSV、CTA 拆分、虚拟化、权限收紧、测试网；send_input/i18n/Issues 诚实未做
- [x] 2026-08-10 P1 可信与交付完整：备份语义、SSH 产物边界、休眠 API UI、Claude SSH 声明、角色说明、续聊复核
- [x] 2026-08-10 P0 主路径可用：首次引导、测试员可发现、依赖拦截、多任务 CTA、仪表盘报表错误可见
- [x] 2026-08-10 产品下一波 backlog 写入 TASK.md（P0–P3）/ Trellis `archive/2026-08/08-10-product-next-wave-backlog`
- 看板任务运行：编排失败状态在重启后保留；失败步骤可重试；转人工后未完成步骤支持手动运行
- header上面的项目下拉框选择没效果，选择项目后 仪表板 看板 员工都要根据选择项目显示
- 检查仪表板员工绩效功能是干嘛的？
- 看板 运行任务的时候检查下停止任务是否有效果
- 支持Codex exec的同时也要支持Codex SDK,可以再设置中添加一个Codex SDK的配置，安装SDK，显示SDK版本
- 看板 任务详情 增加一个AI生成计划，要达到codex /plan的效果，生成计划后返回结果，然后显示插入详情按钮，插入详情需要弹框确认是否替换详情,现在有Codex exec和Codex SDK两种方式都要支持
- 看板任务 添加和详情需要支持上传图片，并且运行的时候可以将图片带给Codex exec和SDK模式都要支持
- AI建议指派 返回结果 只返回了员工的id，没返回员工名称
- 添加新项目后，再点击添加项目，之前添加的数据没有清除
- 看板运行任务也要 插入活动信息
- 仪表盘最近活动显示的是key而不是中文、还需要增加一个查看更多的功能可以分页查询
- 设置界面增加数据库备份、恢复功能，还要显示当前数据库数据版本
- 看板 审核中的任务 添加一个审核代码的功能
- 菜单上面标题暗夜模式下 看不见，需要优化
- 关闭窗口后记录这一次关闭的宽高，下一次打开还是这个宽度和高度
- 检查下运行任务后，增加的Codex改动文件 除了记录了文件的全路径是否记录了文件改动了新增文件的所有内容？点击文件可以查看代码diff了那些
- 看板 任务详情 执行tab Codex改动文件记录里的时间是UTC时间，不是当前系统时区时间
- 黑夜模式下点击设置会自动变成白天模式，检查下是什么原因
- 看板任务详情审核，审核Codex改动文件记录的文件代码
- 看板已完成的任务列表 不用显示运行按钮了
- 优化看板 任务详情tab 去掉滚动条
- 看板 新建任务 增加 AI优化提示词按钮的功能根据选择的项目 生成提示词，如果没选择项目需要提示选择项目后才能生成提示词，如果选择项目，生成提示词后下面显示生成的提示词，然后有替换详情按钮，继续对话功能也需要AI优化提示词功能
- Session管理 可以查看改动文件的列表，并且可以查看代码diff了那些
- 菜单增加自动化的功能，可以配置运行任务完成后，自动审核、审核有问题、自动修复问题，最终变成已完成状态
- 支持SSH功能、可以在远程主机上运行任务，可以在菜单标题下面增加一个SSH的开关，开启就打开SSH模式，SSH模式下数据库还是本地，不需要改动
  - 打开SSH模式只显示SSH项目、没开启显示本地项目、项目添加要增加一个类型 本地、和SSH,还有添加项目和修改项目的时候要判断是SSH类型项目或者本地类型的SSH，SSH类型就选择远程仓库目录
  - 看板任务也要判断项目是否SSH类型，如果开启SSH模式，只显示SSH项目的任务
  - 开启就打开SSH模式 全局项目选择的也只展示SSH项目，然后没开启SSH模型 只显示本地项目
  - 开启就打开SSH模式后，运行任务的时候要判断是否SSH模型，SSH模型远程运行
  - 开启就打开SSH模式后，设置里面安装SDK也是远程主机安装、Codex CLI验证也是验证远程主机的Codex
  - 开启就打开SSH模式后，Session管理也也只显示远程执行的Session
  - 如果点击SSH模型，未配置远程机器跳到设置配置远程SSH信息，支持秘钥登录或者账号密码登录，支持多个SSH配置
- [x] 全局搜索：缺少针对项目、任务、员工、会话的一体化统一搜索；当前只有 Session 管理页和员工启动任务弹窗内的局部搜索。
- [x] 看板任务如果是worktree模式 要显示标签并且右键按钮加一个合并的功能，可以使用项目详情->活动任务上下文->Git动作里面的合并动作，如果是通过看板任务合并按钮合并的动作类型就不能选择了，禁止选择，不要影响原来的功能
- [x] SSH模式开启时候 header右边可以选择SSH主机
- [x] SSH模式开启时候,重新启动会进入设置界面，重新打开时进入的默认页面一直都是仪表盘
- [x] 员工需要支持多任务运行，现在如果有一个任务绑定了员工运行了任务，后续第二个任务也是绑定这个员工就无法运行了这样很不好，改成员工支持多个任务，员工那里终端日志显示，改为按钮 查看运行终端、然后弹窗显示所有运行的任务终端日志，没有就不显示
- [x] 看板新建任务增加worktree模式 默认否，选择是的时候会使用worktree模式,目前是默认使用worktree
- [x] 自动质控的任务运行的时候 状态变成已审核但是 审核按钮没有变化，还是可以点击的状态，然后审核有问题，重新到进行中，运行按钮状态也是没有变化,需要修复
- [x] 设置页面增加Git相关的配置
  - 还有Git worktree目录规则或者自定义目录
  - 还有提交的时候AI生成commit的长短配置，例如 选择项 标题和详情 然后 标题，还可以选择生成Git的模型和推理强度
- [x]  4-20 项目详情 Git工作流最近提交现在只能看到5条、加一个查看更多的功能、并且点击提交详情可以浏览变动内容
- [x]  4-20 仪表盘最近活动->查看更多->全部活动 增加筛选条件 项目、活动类型、内容 、时间范围查询
- [x]  4-21 任务完成系统通知
- [x]  4-21 看板 任务右键菜单增加归档功能，新建任务按钮旁边增加一个归档管理的按钮 例如 新建任务 归档管理 按照这个顺序 。归档管理只是改为归档状态的任务，一个table，有筛选条件 项目 内容 时间范围查询
- [x]  4-21 项目详情 Git工作流增加分支管理，可以切换分支、新建分支、删除分支，将分支合并到某个分支
- [x] 项目详情 Git工作流增加回滚功能 和 选择文件，点击全局回滚
- [x] 项目详情 Git工作流增加 Git Worktree管理 显示所有Worktree可以删除，提交，点击可以查看修改文件列表展示 可以暂存和工作区文件 一样，可以将工作区文件拆分成组件复用
- [x] 设置页面 需要分成不同的tab这样后面增加新的设置不会乱
- [x] 看板任务右键菜单增加功能：标记已完成，自动移动端已完成
- [x] 对话管理切换项目的时候没有根据切换的项目隔离数据
- [x] Session管理 继续对话需要可以停止对话，继续对话后可以查看继续对话内容,继续内容要记录
- [x] 员工 测试员和协调员感觉没有用到，这个测试员和协调员有什么不，或者你觉得测试员和协调员应该干什么好？（已有验收清单/协调员计划生成与落库入口）
- [x] 设置页面增加 每个AI功能的提示词配置，如果用户配置不对，可以重置，点击重置就是原来的默认的提示词
- [x] 仪表盘 最近活动 有些key没有转换 task_review_completed task_review_started task_review_requested employee_project_membership_conflict_migrated（以及分支/回滚/回收站等缺失 key）
- [x] 全部页面支持黑夜模式
- [x] 增加菜单Mcp管理功能
- [x] 所有页面都要支持暗夜模式（与「全部页面支持黑夜模式」重复，已合并视为完成）
- [x] 支持Claude SDK
- [x] 支持Opencode SDK
- [x] 08-05 产品能力补齐路线图 9 子任务全部归档（测试员自动化闭环、看板交付 UX、信任加固、引擎能力对齐、OpenCode SSH、报表导入导出、协调员流水线可视化、MCP 任务绑定、前端最小测试网）

# 暂时不处理的任务

（无。原「离线员工不能运行」已核实为语义错误：`offline` 是空闲心跳，拦截会卡死启动。见本波 A1，不提升为硬拦截。）

（已从本区移出）

- ~~同员工多任务运行 CTA 文案~~ → 已提升至 **P0**
- ~~看板任务可以继续对话~~ → 入口已有，改为 **P1**「看板续聊链路复核」
