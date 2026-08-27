# Journal - wenyuantc (Part 1)

> AI development session journal
> Started: 2026-08-03

---



## Session 1: Grok 设置页安装/重装 CLI

**Date**: 2026-08-03
**Task**: Grok 设置页安装/重装 CLI
**Branch**: `main`

### Summary

为 Grok 设置区补齐本地与 SSH 远程安装/重装 CLI（对齐 Codex），含后端命令、前端按钮与活动日志中文标签；已 build 与 clippy 通过并提交。

### Git Commits

| Hash | Message |
|------|---------|
| `36ec4c9` | (see git log) |

### Status

[OK] **Completed**


## Session 2: 协调员编排 v2 串行多员工流水线

**Date**: 2026-08-03
**Task**: 协调员编排 v2 串行多员工流水线
**Branch**: `main`

### Summary

完成协调员编排 MVP：结构化工作包、按计划串行调度、计划弹窗编排入口与步骤日志、执行中锁定与任务计时；代码已提交。

### Git Commits

| Hash | Message |
|------|---------|
| `2f9394b` | (see git log) |

### Status

[OK] **Completed**


## Session 3: P3 reports R1

**Date**: 2026-08-10
**Task**: P3 reports R1
**Branch**: `main`

### Summary

Dashboard report R1: configurable 7d/30d/8w trends + milestone remaining series; spec contract; TASK.md checked. Next: i18n ahead of send_input per user ok.

### Git Commits

| Hash | Message |
|------|---------|
| `eff5027` | (see git log) |
| `5fadede` | (see git log) |

### Status

[OK] **Completed**


## Session 4: P3 i18n I2

**Date**: 2026-08-10
**Task**: P3 i18n I2
**Branch**: `main`

### Summary

i18next zh-CN/en framework, main-path extraction, activity single-source, leftovers for deep dialogs; send_input still pending.

### Git Commits

| Hash | Message |
|------|---------|
| `a1dd67f` | (see git log) |
| `16ad2a7` | (see git log) |

### Status

[OK] **Completed**


## Session 5: P3 send_input 真会话输入

**Date**: 2026-08-11
**Task**: P3 send_input 真会话输入
**Branch**: `cursor/p3-send-input-planning`

### Summary

实现 Codex/Claude/OpenCode 真会话 send_input 与结束会话；打开终端日志时等待输入，未打开则自动退出推进任务；Grok B1 豁免；补齐 i18n 与 spec。

### Git Commits

| Hash | Message |
|------|---------|
  | `2aac559` | (see git log) |
  | `1f890b3` | (see git log) |

### Status

[OK] **Completed**


## Session 6: P3 i18n 深层对话框抽取

**Date**: 2026-08-11
**Task**: i18n deep-dialog leftovers（承接 Session 4）
**Branch**: `main`

### Summary

承接上轮未提交的深层对话框 i18n 抽取：Git 组件、项目 Git 动作/分支/提交详情/文件预览/里程碑、会话/搜索/员工角色提示全部走 locale；新增 search/tasks 命名空间；修复 EmployeeCard 缺失 `</span>` 闭合、Create/EditEmployeeDialog 角色下拉改用 labelKey、两处 useEffect 补 `t` 依赖。验证：tsc/build(Node22)/lint(仅存量警告)/format/clippy 全绿，en/zh 键对等，无残留硬编码中文。

### Git Commits

| Hash | Message |
|------|---------|
| `6fb9214` | feat(i18n): 深层对话框文案抽取，新增 search/tasks 命名空间 |

### Status

[OK] **Completed**


## Session 6: Sessions 布局重构收尾

**Date**: 2026-08-12
**Task**: Sessions 布局重构收尾
**Branch**: `main`

### Summary

对话管理页完成卡片/表格可切换布局与筛选收敛；补齐 Pi 平台脚手架并排除 lint/format；质量门 format/lint/test/build 通过后提交、归档并推送。

### Git Commits

| Hash | Message |
|------|---------|
| `d3d17ef` | (see git log) |
| `74e88d7` | (see git log) |

### Status

[OK] **Completed**


## Session 7: A1 token 用量落库与展示收尾

**Date**: 2026-08-13
**Task**: A1 token 用量落库与展示收尾
**Branch**: `main`

### Summary

四引擎 token 解析落库，任务执行 Tab 与仪表盘展示用量；检查补齐 NULL 累加与 OpenCode 解析测试，并写入 backend spec。

### Main Changes

- codex_sessions 增加可空 token 列，无上报保持 NULL
- 共享 UsageDelta 解析四引擎 CLI/SDK 用量并累加落库
- 任务详情与仪表盘按 sessions_with_usage 展示，不假装 0

### Git Commits

| Hash | Message |
|------|---------|
| `c0acc3c` | (see git log) |
| `429f772` | (see git log) |
| `bb01242` | (see git log) |

### Testing

- [OK] clippy / format:check / test:ci / build 通过；补 OpenCode 解析与 SUM 忽略未知会话测试

### Status

[OK] **Completed**

### Next Steps

- 继续 B1 并发闸门与运行队列（工作区已有后端 WIP，前端未接）


## Session 8: B1 队列后端提交与 A2 归档

**Date**: 2026-08-13
**Task**: B1 队列后端提交与 A2 归档
**Branch**: `main`

### Summary

提交全局并发闸门与持久化运行队列后端；保存产品缺口下一波 Trellis 规划；归档已落地的 A2 日志复制导出。

### Main Changes

- 四引擎 start 接闸门，超限任务写入 task_run_queue，退出后 FIFO 放行
- 提交父任务与剩余子任务 PRD/设计文档
- 归档 A2 会话日志复制与导出

### Git Commits

| Hash | Message |
|------|---------|
| `8dbd82a` | (see git log) |
| `564c89e` | (see git log) |
| `7d48973` | (see git log) |

### Status

[OK] **Completed**

### Next Steps

- 补齐 B1 设置页、看板排队徽标与批量运行 UI


## Session 9: B1 看板与设置对接运行队列

**Date**: 2026-08-19
**Task**: B1 看板与设置对接运行队列
**Branch**: `main`

### Summary

把已落地的并发闸门接到设置页、任务卡片和批量运行；排队不再误标进行中；写入 run-queue 契约并归档 B1。

### Main Changes

- 四引擎 start 返回 started|queued，startTaskRunSession 先 invoke 再写副作用
- 设置页本地并发上限；看板排队徽标、取消排队、批量运行汇总
- taskStore 监听队列事件并刷新任务列表；活动日志中英文案

### Git Commits

| Hash | Message |
|------|---------|
| `d9327cc` | (see git log) |
| `e1c20cb` | (see git log) |
| `1f5a28f` | (see git log) |

### Testing

- [OK] npm run test:ci / format:check / build 通过
- [OK] cargo test run_queue 与 clippy -D warnings 通过

### Status

[OK] **Completed**

### Next Steps

- B2 任务模板（父任务下一波未完成项）


## Session 10: B2 任务模板

**Date**: 2026-08-19
**Task**: B2 任务模板
**Branch**: `main`

### Summary

落地任务模板：看板管理/右键存为模板、{{变量}}批量套用、v47 表与套用事务；写入契约并归档 B2。

### Main Changes

- v47 task_templates + 6 条命令：CRUD、from_task、先校验再事务套用
- 看板「模板」对话框与 TaskCard 右键存为模板；标签按名 find-or-create，子任务复制为待办
- 活动日志 task_template_created/applied/deleted 中英文案；spec 与 CLAUDE 计数同步

### Git Commits

| Hash | Message |
|------|---------|
| `673c975` | (see git log) |
| `b478792` | (see git log) |
| `92afc27` | (see git log) |

### Testing

- [OK] cargo test templates（15）与 latest_migration_version 通过
- [OK] clippy -D warnings / npm run test:ci / format:check / build 通过
- [OK] 未跑 tauri:dev 手工冒烟

### Status

[OK] **Completed**

### Next Steps

- C1 审查行级定位（父任务下一未完成项；其后是 D1 自动更新）


## Session 11: C1 审查行级定位

**Date**: 2026-08-19
**Task**: C1 审查行级定位
**Branch**: `main`

### Summary

审查输出结构化 findings 并锚定 Monaco Diff；写入契约并归档 C1。

### Git Commits

| Hash | Message |
|------|---------|
| `5043c04` | (see git log) |
| `f441e09` | (see git log) |
| `e27df73` | (see git log) |

### Status

[OK] **Completed**


## Session 12: D1 应用自动更新

**Date**: 2026-08-19
**Task**: D1 应用自动更新
**Branch**: `main`

### Summary

落地应用内检查/安装更新：设置页入口、updater 签名产物与 latest.json；写入契约并归档 D1。

### Main Changes

- tauri-plugin-updater/process + 设置页 AboutUpdateSection：检查、进度、确认后重启；app_update_installed 活动
- CI：tag 必须有签名 secret；macOS --bundles app,dmg 后补签 .app.tar.gz；dispatch 无密钥只打安装包
- spec 记录插件封装、latest.json 平台键与 macOS 签名陷阱

### Git Commits

| Hash | Message |
|------|---------|
| `ff7343b` | (see git log) |
| `4e324e8` | (see git log) |
| `2e25a04` | (see git log) |

### Testing

- [OK] clippy -D warnings / npm run test:ci / format:check / build 通过
- [OK] 浏览器设置页：中英文案与开发模式错误提示可见；未跑 tauri 安装包端到端下载

### Status

[OK] **Completed**

### Next Steps

- 把 ~/.tauri/codex-ai-updater.key 配进 GitHub secret TAURI_SIGNING_PRIVATE_KEY
- 父任务 08-12-product-gap-wave 收尾：全量门禁、文档计数、归档父任务


## Session 13: 产品下一波规划：可信+可运营+好找

**Date**: 2026-08-20
**Task**: 产品下一波规划：可信+可运营+好找
**Branch**: `main`

### Summary

以产品经理视角实读代码，归档 08-12-product-gap-wave，创建 08-20-product-trust-ops 及 8 个子任务 PRD，写入 analysis/09 与 TASK.md。本回合不实现业务代码。

### Main Changes

- 归档上一波父任务并建立 8 项产品优化 backlog

### Git Commits

(No commits - planning session)

### Status

[OK] **Completed**

### Next Steps

- 用户批准后按 A1→A2→B1… 一次 start 一个子任务实现


## Session 14: 落地 08-20 产品下一波：可信+可运营+好找

**Date**: 2026-08-20
**Task**: 落地 08-20 产品下一波：可信+可运营+好找
**Branch**: `main`

### Summary

按 A1→C2 实现并归档 8 项：员工空闲语义、图片附件运行前确认、看板队列与批量跳过原因、会话 token、启动检查更新、四引擎引导、右键/搜索 i18n、空闲计时器隐藏。

### Main Changes

- 员工空闲不再叫离线；运行前诚实提示会被跳过的图片
- 看板运行队列列表+批量跳过原因；会话列表展示 token
- 启动静默检查更新横幅；引导按四引擎健康检查
- 看板右键与全局搜索 i18n；空闲任务隐藏假计时器

### Git Commits

| Hash | Message |
|------|---------|
| `dc98b23` | (see git log) |
| `b50ba28` | (see git log) |
| `3ab9c3f` | (see git log) |
| `11a07e7` | (see git log) |
| `d90947a` | (see git log) |
| `cb0dc25` | (see git log) |
| `fc105b2` | (see git log) |
| `f7d6631` | (see git log) |

### Testing

- [OK] npm run format:check；npm run test:ci 101 passed；npm run build 通过

### Status

[OK] **Completed**

### Next Steps

- 打包后冒烟：员工空闲文案、带图运行确认、看板队列、会话 token、启动更新横幅


## Session 15: 内置 Agent 引擎接入

**Date**: 2026-08-21
**Task**: 内置 Agent 引擎接入
**Branch**: `main`

### Summary

落地第五引擎 native（内置 Agent）：渠道 API、三协议客户端、工具循环、SSH、图片、全局提示词模板、项目 AGENTS.md、上下文压缩与 Web 工具；会话日志补模型与用量。

### Main Changes

- 新增 native 进程内 Agent 与 ai_channels 渠道配置
- 任务/编排/审查/运行队列接入 native，会话日志输出工具过程
- 支持本地图片、全局提示词模板、AGENTS.md/CLAUDE.md、压缩、WebFetch/WebSearch

### Git Commits

| Hash | Message |
|------|---------|
| `433f788` | (see git log) |
| `ba19873` | (see git log) |

### Testing

- [OK] cargo test --lib native:: ；clippy -D warnings ；npm run test:ci ；npm run format:check

### Status

[OK] **Completed**

### Next Steps

- 桌面端 tauri:dev 冒烟：本地与 SSH 任务、带图、Web 工具
- 未提交的 08-21-chain-token-usage 链路用量 UI 另开会话继续


## Session 16: 执行链路与对话管理展示用量

**Date**: 2026-08-21
**Task**: 执行链路与对话管理展示用量
**Branch**: `main`

### Summary

任务详情执行链路、侧栏与对话管理统一展示输入/输出/缓存/总用量/缓存率；无上报显示未知，不假装 0。复用 TokenUsageBreakdown 与 buildTokenUsageMetrics。

### Git Commits

| Hash | Message |
|------|---------|
| `b4002a6` | (see git log) |

### Status

[OK] **Completed**


## Session 17: 对话管理补齐执行/审核/编排类型

**Date**: 2026-08-21
**Task**: 对话管理补齐执行/审核/编排类型
**Branch**: `main`

### Summary

为会话增加 session_origin，对话管理、筛选和会话链能区分执行、审核与编排；不扩展 session_kind。

### Main Changes

- migration 51 新增 codex_sessions.session_origin，并从 task_pipeline_steps.session_id 回填
- 编排启动绑定 session 后打标 pipeline；列表/搜索/员工进行中会话带出 origin
- 对话管理类型 Badge 与筛选；会话链编排优先于修复启发式

### Git Commits

| Hash | Message |
|------|---------|
| `bc8433b` | (see git log) |

### Testing

- [OK] clippy -D warnings、npm run format:check、npm run test:ci、npm run build、cargo test session_origin 均通过

### Status

[OK] **Completed**

### Next Steps

- 重启桌面应用跑 migration 51，在对话管理核对三种类型筛选


## Session 18: P0 引擎生态对齐

**Date**: 2026-08-24
**Task**: P0 引擎生态对齐
**Branch**: `main`

### Summary

落地 MCP 对齐、native 高风险确认（可在设置关闭）与 Claude CLI 本地传图；配了要能用。

### Main Changes

- native 注入 MCP：本地 spawn，SSH 经 build_ssh_command 远端拉起，失败不回退本机
- 能力矩阵/设置/任务绑定按引擎声明 MCP，Claude/Grok/OpenCode 不再假装生效
- 高风险工具弹窗三选，设置「界面与运行」可关闭确认
- Claude 本地 CLI 用 stream-json 传图；SSH 仍跳过

### Git Commits

| Hash | Message |
|------|---------|
| `be6eeba` | (see git log) |

### Testing

- [OK] clippy -D warnings、format:check、test:ci、npm run build 通过
- [OK] cargo test：mcp / permission / claude_cli / missing_confirm_flag_defaults_on

### Status

[OK] **Completed**

### Next Steps

- 重启桌面应用冒烟：MCP 连接日志、高风险弹窗与设置开关、Claude CLI 本地带图
- BUG.md 两条新待办未提交：TodoWrite 日志内容、协调员终端过程日志


## Session 19: 协调员计划日志补齐工具过程

**Date**: 2026-08-24
**Task**: 协调员计划日志补齐工具过程
**Branch**: `main`

### Summary

协调员生成计划时通过 ai-command-stdout 流式展示只读工具过程；内置 Agent 走只读 AgentRunner，不注册执行会话、不进质控。

### Git Commits

| Hash | Message |
|------|---------|
| `635ddea` | (see git log) |

### Status

[OK] **Completed**


## Session 20: native 会话内子 Agent

**Date**: 2026-08-25
**Task**: native 会话内子 Agent
**Branch**: `main`

### Summary

内置 Agent 增加会话内委派与同轮并行子 Agent；设置可调并发上限和委派策略；终端前缀带类型与短标题。

### Main Changes

- 新增 Agent 工具（explore 只读 / general 可写），同一轮连续调用并行，depth=1，不占 run_queue
- 并行前置：高风险确认 FIFO 队列，MCP stdio Mutex，ModelClient Clone
- 设置「界面与运行」：同轮子 Agent 上限 1–16、策略 conservative/balanced/aggressive；Select 触发器显示中文名称
- 终端前缀 [子 Agent n(explore|general) - 短标题]

### Git Commits

| Hash | Message |
|------|---------|
| `6e19672` | (see git log) |

### Testing

- [OK] cargo test native::；clippy -D warnings
- [OK] npm run format:check、test:ci、build

### Status

[OK] **Completed**

### Next Steps

- 本地提交尚未 git push
- 观察积极策略下真实委派率；plan 模式 / Skills 仍未做


## Session 21: native 自定义子智能体

**Date**: 2026-08-25
**Task**: native 自定义子智能体
**Branch**: `main`

### Summary

设置可配置自定义子智能体；任务可选绑定且必须经 Agent 委派；作用域默认所有项目，也可限定指定项目。

### Main Changes

- 设置「子智能体」Tab：JSON 目录 CRUD，弹窗新建/编辑，旁侧编辑按钮
- 任务新建/详情可选绑定一个子智能体；执行时父循环必须 Agent(subagent_type=名称) 委派
- 作用域默认所有项目，可指定项目；选择器与运行时目录按项目过滤，已绑定任务仍会委派

### Git Commits

| Hash | Message |
|------|---------|
| `aaf593f` | (see git log) |

### Testing

- [OK] clippy -D warnings、format:check、test:ci、tsc --noEmit 通过
- [OK] cargo test native::subagents::tests 10 项通过

### Status

[OK] **Completed**

### Next Steps

- tauri dev 冒烟：设置作用域、任务绑定、父循环必须委派
- 本地提交尚未 git push


## Session 22: 任务项目文件引用

**Date**: 2026-08-25
**Task**: 任务项目文件引用
**Branch**: `main`

### Summary

新建任务和详情可引用项目仓库相对路径；git-bridge 覆盖本地与 SSH；运行/计划/自动修复提示词注入路径。未在桌面端冒烟选择器。

### Git Commits

| Hash | Message |
|------|---------|
| `3c11683` | (see git log) |

### Status

[OK] **Completed**


## Session 23: native 看板计划运行

**Date**: 2026-08-25
**Task**: native 看板计划运行
**Branch**: `main`

### Summary

内置 Agent 看板「计划运行」：只读规划，缺决策提问（可选其他自定义），无问题则同一会话自动执行。

### Main Changes

- 看板右键计划运行仅 native 执行人可见，排队透传 plan_mode
- 计划轮只读；AskQuestion 阻塞提问；可选模型选项或「其他」自定义输入
- 无提问则计划轮结束自动执行；不替代协调员、不写 plan_content

### Git Commits

| Hash | Message |
|------|---------|
| `2448bba` | (see git log) |

### Testing

- [OK] clippy -D warnings；cargo test native::；format:check；test:ci；npm run build

### Status

[OK] **Completed**

### Next Steps

- tauri dev 冒烟：无歧义任务不问直接跑，有取舍弹窗，其他可自定义
- 08-25-native-stop-no-review 仍 in_progress，确认后可归档


## Session 24: 内置 Agent 终端显示完整工具结果

**Date**: 2026-08-25
**Task**: 内置 Agent 终端显示完整工具结果
**Branch**: `main`

### Summary

内置 Agent 终端 [工具结果] 改为发出完整工具输出，不再只留第一行 200 字。

### Main Changes

- tool_result_line 发全文一条事件；TodoWrite 清单仍报项数；超长 2000 行/64KB 只截 UI
- 同步 ai-engines.md 与 native README 的终端日志约定

### Git Commits

| Hash | Message |
|------|---------|
| `4929015` | (see git log) |

### Testing

- [OK] cargo test --lib tool_result / todo_*_result / emits_tool_progress_lines
- [OK] cargo clippy --all-targets -- -D warnings；npm run format:check

### Status

[OK] **Completed**

### Next Steps

- 在桌面应用里跑一次内置 Agent Read+Grep，确认任务/会话日志能滚出完整块


## Session 25: 内置 Agent API 报错重试

**Date**: 2026-08-27
**Task**: 内置 Agent API 报错重试
**Branch**: `main`

### Summary

内置 Agent 遇到可恢复 API 错误时最多重试 10 次、间隔 3 秒，并在会话终端输出 [重试] 行；401 等鉴权错误立即失败。

### Main Changes

- RetryConfig 默认 10 次、固定 3 秒；post_stream 覆盖 5xx/限流/200 网关空响应
- 会话终端 sleep 前打出 [重试] 行；停止时 200ms 内可取消等待

### Git Commits

| Hash | Message |
|------|---------|
| `6f55f22` | (see git log) |

### Testing

- [OK] cargo test native::model；clippy -D warnings；npm run format:check

### Status

[OK] **Completed**

### Next Steps

- 08-25-native-stop-no-review 仍为 in_progress，未在本轮归档
