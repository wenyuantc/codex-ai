# Code Review Report

## Overview

| Item | Value |
|------|-------|
| Skill | code-review-level |
| Level | **max** |
| Scope mode | git_diff (untracked-only) |
| Files | 9 |
| Languages | markdown, json |
| Total findings | 34 |
| Critical | 3 |
| High | 11 |
| Medium | 11 |
| Low | 6 |
| Info | 3 |
| Generated | 2026-07-25T16:45:24.382486+00:00 |

## Level Execution Summary

| Item | Value |
|------|-------|
| Level | **max** |
| Depth | exhaustive |
| Dimensions | correctness, readability, performance, security, testing, architecture |
| Files reviewed | 9 |
| Truncated | no |
| Agents | Explore×2 + general-purpose×2（trace-deps / security / adversarial / architecture） |
| Cross-file | yes（对照 live codebase） |
| Adversarial | yes |
| Dependency trace | yes |

### Covered
- 六维全开：correctness / security / performance / readability / testing / architecture
- 对照代码核验：IPC 149、迁移 v40、LOC、capabilities、automation resume、git confirm hash、Settings 5 Tab
- 多 Agent 并行：事实溯源、安全、生产失败对抗、架构深挖

### Not covered at this level
- 无（exhaustive mode）；范围仅限 **未跟踪** 的 `docs/analysis/*` 与 `.openclaude/settings.local.json`
- **未**审查已提交且 clean 的业务源码作为独立 diff（但为验证文档声明做了必要代码抽检）
- 未跑完整 `cargo test` / `npm run build` 作为门禁（文档审查为主）

### Scope note
Working tree **无 staged/unstaged 已跟踪改动**；仅有 untracked 分析文档。本次 max 审查目标是：分析文档的事实正确性、风险分级是否充分、以及依赖这些文档做生产决策时的对抗失败模式。

## Risk Hotspots

- `docs/analysis/05-quality-risks.md` (security, config)
- `docs/analysis/03-ipc-command-catalog.md` (security, data)
- `docs/analysis/02-data-model-audit.md` (data, security)
- `docs/analysis/04-runtime-lifecycle.md` (concurrency, config)
- `.openclaude/settings.local.json` (config)

## Files Reviewed

- `docs/analysis/05-quality-risks.md`
- `docs/analysis/03-ipc-command-catalog.md`
- `docs/analysis/02-data-model-audit.md`
- `docs/analysis/04-runtime-lifecycle.md`
- `docs/analysis/00-architecture-overview.md`
- `docs/analysis/01-domain-capability-matrix.md`
- `docs/analysis/06-tech-debt-roadmap.md`
- `docs/analysis/README.md`
- `.openclaude/settings.local.json`

## Critical (3)

### F-001 [Critical] 文档未披露：冷启动 orphan recovery 无差别将 running 会话标 failed，可能毒化自动化

- **Dimension**: correctness
- **Location**: `docs/analysis/04-runtime-lifecycle.md`:125
- **Evidence**: 04 仅写「orphan recovery 防双开」。代码 task_automation.rs:361-386 recover_orphaned_running_sessions 将所有 status='running' AND ended_at IS NULL 的会话 UPDATE 为 failed/exit_code=1，不区分进程是否仍存活；随后 replay 可能把 waiting_review/waiting_execution 推向 terminal failure，误阻塞任务。
- **Suggestion**: 文档写清冷启动有损 reconcile；实现改为仅 Manager 无 live process 时标记 interrupted；区分 app_restart_interrupted 与真实失败，避免误 finalize_terminal_failure。
- **Source**: adversarial

### F-002 [Critical] 文档未披露：resume_pending 单任务失败 fail-stop 且未过滤软删任务

- **Dimension**: correctness
- **Location**: `docs/analysis/04-runtime-lifecycle.md`:119
- **Evidence**: fetch_pending_automation_task_ids（task_automation.rs:307-328）只过滤 automation_mode/status!=archived/phase，无 deleted_at IS NULL。resume 循环对 retry_* 使用 ?，单任务 Err 中止后续全部恢复。软删后 phase 仍可能 pending，与 trash 交叉。
- **Suggestion**: SQL 加 deleted_at 过滤；软删时停 automation；resume 改为 per-task try/continue；文档声明启动恢复非全量保证。
- **Source**: adversarial

### F-003 [Critical] Git 高风险 confirm 被文档标为能力完整，实则 DefaultHasher token + 非原子 claim

- **Dimension**: security
- **Location**: `docs/analysis/01-domain-capability-matrix.md`:24
- **Evidence**: 01 将「高风险 Git 确认流」标 ✅。git_workflow.rs:539-555 用 std DefaultHasher 签 token；confirm 校验后先执行再清 pending。叠加 sql:allow-execute 可读 pending nonce/hash。05 仅建议「补测试」，未暴露可双执行/弱签名风险。
- **Suggestion**: HMAC 密钥签 token；条件 UPDATE 原子 claim 后再执行；收紧 sql execute。文档勿把 request/confirm 等同安全二次确认。
- **Source**: adversarial

## High (11)

### F-004 [High] sql:allow-execute + csp:null 组合风险在文档中被低估为「中」

- **Dimension**: security
- **Location**: `docs/analysis/05-quality-risks.md`:7
- **Evidence**: default.json:11 授予 sql:allow-execute；tauri.conf.json:24 csp 为 null。05 TOP1 影响标「中」且未点名 csp:null。任意 WebView 脚本可写库绕过 Rust 服务层。3 处 activity INSERT 证据正确但非主因。
- **Suggestion**: 风险表将 sql-execute/csp:null 升为 High/Critical；区分「当前调用点」与「权限面」；生产移除 allow-execute 并配置严格 CSP。
- **Source**: security-pass

### F-005 [High] shell:allow-execute 无白名单，文档标「低–中」严重低估

- **Dimension**: security
- **Location**: `docs/analysis/05-quality-risks.md`:16
- **Evidence**: default.json:27-29 授予无限制 shell:allow-execute/kill/stdin-write；仅 spawn 有 codex 白名单。前端未直接用 plugin-shell，但权限对 main 窗口开放；csp:null 下构成潜在本地 RCE 面。
- **Suggestion**: 删除未使用的 shell:allow-execute/kill/stdin-write；仅保留严格 spawn 白名单；文档将此项升为 High。
- **Source**: security-pass

### F-006 [High] sanitize_sql_backup_script 被文档列作安全观察，实际几乎不防恶意 SQL

- **Dimension**: security
- **Location**: `docs/analysis/05-quality-risks.md`:58
- **Evidence**: database.rs sanitize 仅剥 BEGIN/COMMIT 与部分 PRAGMA，不拦截 ATTACH/DROP 等。恢复路径对 live pool 再执行 sanitized_sql。文档易被读成「已充分消毒」。
- **Suggestion**: 仅接受本应用签名/白名单导出格式；或 VACUUM INTO 二进制快照。文档明确 sanitize 非安全边界。
- **Source**: security-pass

### F-007 [High] 路线图将体验 C 置于正确性之前，可能误配工程资源

- **Dimension**: architecture
- **Location**: `docs/analysis/06-tech-debt-roadmap.md`:3
- **Evidence**: README/06 主推体验打磨 C，P0 仅 activity 写与 capabilities；git confirm/resume 毒化/软删交叉排在测试或体验旁路。对抗审查显示冷启动与 Git confirm 可导致误完成/误阻塞/双执行。
- **Suggestion**: 重排：resume 守卫 + soft-delete 运行时清理 + git confirm 原子化 + 禁用 sql/shell execute 高于看板 UX；更新一句话结论。
- **Source**: adversarial

### F-008 [High] AI 多引擎路由错误指向 determine_effective_provider

- **Dimension**: correctness
- **Location**: `docs/analysis/01-domain-capability-matrix.md`:73
- **Evidence**: 脚注要求结合 determine_effective_provider 确认多引擎路由。该函数（codex/settings.rs）仅返回 sdk|exec 传输层，非 ai_provider→claude/opencode 路由。真实分支在 ai_commands/one_shot 与 task_automation assignee.ai_provider。
- **Suggestion**: 脚注改为：传输层看 determine_effective_provider(sdk|exec)；跨引擎看 employee/settings 的 ai_provider 与 automation 启动分支。
- **Source**: trace-deps

### F-009 [High] 自动质控审核路径硬编码 Codex，与「三引擎统一」叙述矛盾

- **Dimension**: architecture
- **Location**: `docs/analysis/04-runtime-lifecycle.md`:127
- **Evidence**: retry_pending_review（task_automation.rs:904-905）固定 CodexManager + start_task_code_review_internal；fix 路径才按 ai_provider 分 Claude/OpenCode/Codex。01/04 未写死「automation review = Codex-only」。
- **Suggestion**: 能力矩阵拆分 review vs fix 引擎列；文档写死 review 仅 Codex 或实现 reviewer provider 路由。
- **Source**: adversarial

### F-010 [High] 软删/回收站被标 ✅，未描述进程与 automation 泄漏及 restore 过宽

- **Dimension**: correctness
- **Location**: `docs/analysis/01-domain-capability-matrix.md`:40
- **Evidence**: delete_task 仅 SET deleted_at，不 stop 会话、不改 automation。restore_project 可复活项目删除前已独立进回收站的任务。永久删项目不清理 worktree。
- **Suggestion**: 能力矩阵对 soft-delete 改为 ⚠️；补充删除矩阵（DB/进程/worktree/附件）；实现软删时 stop session + disable automation。
- **Source**: adversarial

### F-011 [High] 读路径是结构性双轨，文档弱化为「仍混用」

- **Dimension**: architecture
- **Location**: `docs/analysis/00-architecture-overview.md`:71
- **Evidence**: ADR 要求 store 只缓存并调 command；核心领域读主路径是 plugin-sql SELECT（stores + 多个组件）。00 把写标「少量泄漏」、读标「仍混用」，低估默认架构形态。
- **Suggestion**: 边界状态拆成：写路径 P0 泄漏 vs 读路径 CQRS 双轨；更新 ADR 明确是否允许只读 plugin-sql。
- **Source**: architecture

### F-012 [High] Claude.md 过时架构数字未列入产品文档债，易与 analysis 对撞

- **Dimension**: correctness
- **Location**: `docs/analysis/05-quality-risks.md`:100
- **Evidence**: Claude.md 仍写「45 commands / 7 pages / 前端从不直写库」。analysis 证明 149 commands、8 页、前端 SELECT+INSERT。05 未点名 Claude.md/Agents.md。
- **Suggestion**: P0-4/P2-8 扩展同步修订 Claude.md；README 声明权威优先级：analysis/ADR > 过时 agent 指南数字。
- **Source**: architecture

### F-013 [High] 设置页 Tab 数量与路由描述过时（四→五）

- **Dimension**: correctness
- **Location**: `docs/analysis/05-quality-risks.md`:123
- **Evidence**: 文档写「设置四 Tab」。SettingsPage.tsx:80-86 为 5 项：runtime/git/prompts/ssh/database。00:119 路由说明同样漏「提示词模板」。
- **Suggestion**: 统一改为 5 Tab，并在 00 路由表补上 prompts。
- **Source**: trace-deps

### F-014 [High] 审核通过后先标 completed 再 commit 的状态机分叉未文档化

- **Dimension**: correctness
- **Location**: `docs/analysis/04-runtime-lifecycle.md`:110
- **Evidence**: automation 在 verdict.passed 且 auto-commit 时先 update_task_status completed，再 phase=committing_code；commit 失败则 phase=commit_failed 但任务已 completed。04 阶段表未描述该不变量破坏。
- **Suggestion**: commit 成功后再 completed；文档增加 status×phase 不变量与故障表。
- **Source**: adversarial

## Medium (11)

### F-015 [Medium] SSH 密钥与 SQL 备份关系被标 UNVERIFIED，实为已可证实

- **Dimension**: security
- **Location**: `docs/analysis/02-data-model-audit.md`:123
- **Evidence**: 密码/口令经 secret_store 明文写 ssh-secrets.json；backup_database 仅 SQL 导出，不含 secret_store。private_key_path 会进 SQL。文档「不一定包含」过弱。
- **Suggestion**: 改为明确：SQL 仅含 ref/路径；灾备必须另备 secret_store 与私钥文件。
- **Source**: security-pass

### F-016 [Medium] secret_store 明文 JSON 与 SSH_ASKPASS 环境变量风险未写

- **Dimension**: security
- **Location**: `docs/analysis/05-quality-risks.md`:54
- **Evidence**: secret_store 以 pretty JSON 落盘（Unix 0o600）；remote build_ssh_command 将密码注入 CODEX_SSH_SECRET 环境。05 仅写「secret_store + redact helpers 存在」。
- **Suggestion**: 文档明确威胁模型；评估 keyring；限制 env 传密生命周期。
- **Source**: security-pass

### F-017 [Medium] OpenCode 已接入 automation fix 启动，仍标 UNVERIFIED 过保守

- **Dimension**: architecture
- **Location**: `docs/analysis/04-runtime-lifecycle.md`:127
- **Evidence**: task_automation 已 import OpenCodeManager 并在 ai_provider==opencode 启动修复会话。未验证的是全 phase 对称性，不是「是否接入」。
- **Suggestion**: 用 phase×provider 矩阵：fix launch ✅；review Codex-only；resume/SSH/commit 待验证。
- **Source**: trace-deps

### F-018 [Medium] IPC「基本对齐」掩盖 3 个前端零引用的注册命令

- **Dimension**: correctness
- **Location**: `docs/analysis/03-ipc-command-catalog.md`:12
- **Evidence**: 149 注册与 149 注解属实；前端 invoke 字面量 146。orphan commands：get_task_git_context、open_project_git_file、get_ssh_config 前端无引用。
- **Suggestion**: 统计表增加「仅后端注册」列；CI 检查 invoke ⊆ handler 与 handler 未使用清单。
- **Source**: trace-deps

### F-019 [Medium] 「Rust 服务层」夸大：实为胖 command 处理器 + 平级写库模块

- **Dimension**: architecture
- **Location**: `docs/analysis/00-architecture-overview.md`:35
- **Evidence**: app/* 无独立 repository/domain；git_workflow/task_automation/engines 平级直写 SQLite。对外边界是 Tauri commands，对内非分层服务。
- **Suggestion**: 00 明确：服务边界=IPC；内部为模块化单体胖 handler。拆分目标为 domain 模块而非仅切文件。
- **Source**: architecture

### F-020 [Medium] 共库并发/WAL/restore 一致性模型完全缺失

- **Dimension**: architecture
- **Location**: `docs/analysis/00-architecture-overview.md`:47
- **Evidence**: 前端 plugin-sql 与后端 SQLx 共享同库；未文档化连接、busy、写事务所有者、restore 时前端缓存失效。
- **Suggestion**: 00/04 增加 SQLite 访问模型：共享 pool、写事务归属、re-fetch 策略、restore 互斥。
- **Source**: architecture

### F-021 [Medium] 事件总线契约未作为一等架构面

- **Dimension**: architecture
- **Location**: `docs/analysis/03-ipc-command-catalog.md`:136
- **Evidence**: 03 仅一句事件靠 listen；实际有 automation/session/notification 等多事件与 live 合成日志，无清单与 payload schema。
- **Suggestion**: 新增 event-catalog：事件名、发射点、订阅点、payload、去重语义。
- **Source**: architecture

### F-022 [Medium] 遗留前端启发式 AI（lib/ai.ts）双轨未记录

- **Dimension**: architecture
- **Location**: `docs/analysis/01-domain-capability-matrix.md`:67
- **Evidence**: AISuggestDialog 等走 lib/ai.ts 本地规则+SQL，与 IPC ai_* 并行；组件疑似死代码但路径存在。
- **Suggestion**: 能力矩阵标注启发式 vs 真 AI command；清理死代码或统一入口。
- **Source**: trace-deps

### F-023 [Medium] 前端 SQL 读消费者清单不完整

- **Dimension**: correctness
- **Location**: `docs/analysis/00-architecture-overview.md`:76
- **Evidence**: 除 dashboard/task/employee/project stores 外，logStore、ArchiveManagementDialog、TaskSessionChainPanel、EmployeePerformanceChart、lib/ai.ts 也 import database。
- **Suggestion**: 全量枚举 plugin-sql 调用点并分类；评估收紧权限时覆盖组件 ad-hoc SELECT。
- **Source**: architecture

### F-024 [Medium] spawn_resume 与用户操作无同步屏障未文档化

- **Dimension**: architecture
- **Location**: `docs/analysis/04-runtime-lifecycle.md`:13
- **Evidence**: lib.rs fire-and-forget resume，窗口可交互；可与手动 start/set_automation 并发写 phase。
- **Suggestion**: 恢复完成前 automation gate 或 per-task lock；UI 显示 resuming。
- **Source**: adversarial

### F-030 [Medium] 关键正确性路径缺测试网，文档建议正确但未绑定 P0

- **Dimension**: testing
- **Location**: `docs/analysis/05-quality-risks.md`:42
- **Evidence**: 无前端测试、无 automation 属性测试、无 git confirm 契约测试；package.json 无 test script。P0-3 仅「手工+单测」，git confirm 在 P1-4。
- **Suggestion**: 将 automation resume 矩阵、git confirm 原子 claim、soft-delete 守卫升为 P0 测试闸门。
- **Source**: primary

## Low (6)

### F-025 [Low] known_hosts_mode=off 禁用主机密钥校验未覆盖

- **Dimension**: security
- **Location**: `docs/analysis/05-quality-risks.md`:52
- **Evidence**: remote.rs 支持 StrictHostKeyChecking=no，MITM 风险。安全表未列。
- **Suggestion**: 默认 accept-new/strict；off 需危险确认；文档补充。
- **Source**: security-pass

### F-026 [Low] 索引「约 58」是 CREATE 次数非稳态唯一索引数

- **Dimension**: correctness
- **Location**: `docs/analysis/02-data-model-audit.md`:102
- **Evidence**: 迁移含索引重建，稳态唯一索引 < 58。
- **Suggestion**: 改为「迁移约 58 次 CREATE INDEX（含重建）」并建议 schema 枚举稳态数。
- **Source**: trace-deps

### F-027 [Low] project_employees 停写可收口为已验证

- **Dimension**: correctness
- **Location**: `docs/analysis/02-data-model-audit.md`:95
- **Evidence**: 全库 project_employees 仅 migrations.rs；业务无读写。
- **Suggestion**: 状态改为 ✅ 业务层无读写；是否 DROP 为可选清理。
- **Source**: trace-deps

### F-028 [Low] 00 将测试规模近似为 app/tests ~1.6k，易被读成全库

- **Dimension**: correctness
- **Location**: `docs/analysis/00-architecture-overview.md`:160
- **Evidence**: app/tests ~1.6k 属实；全库 #[test] 约 221；05 已写 200+。00 措辞偏窄。
- **Suggestion**: 与 05 对齐：集成测试 + 模块内单测；仍无前端测试。
- **Source**: trace-deps

### F-029 [Low] Manager 锁模型不对称未评估架构风险

- **Dimension**: architecture
- **Location**: `docs/analysis/04-runtime-lifecycle.md`:5
- **Evidence**: Codex=std Mutex，Claude/OpenCode=tokio Mutex；跨引擎编排存在阻塞风险未入债项。
- **Suggestion**: 05/P1 增加统一 async Mutex 债项。
- **Source**: architecture

### F-031 [Low] 分析系列整体可读性良好，但跨文档重复与矛盾点缺索引

- **Dimension**: readability
- **Location**: `docs/analysis/README.md`:17
- **Evidence**: 阅读顺序清晰；但 00/05/06 对边界风险分级不完全一致，无「已知矛盾/更新日志」。
- **Suggestion**: README 增加变更日志与 severity 统一表；交叉引用 UNVERIFIED 清单单一来源。
- **Source**: primary

## Info (3)

### F-032 [Info] 性能热点 LOC 数字抽样核验通过

- **Dimension**: performance
- **Location**: `docs/analysis/05-quality-risks.md`:67
- **Evidence**: git_workflow 5515、task_automation 2943、ProjectDetail 1425、TaskDetail 1339、TaskCard 1290、Settings 1081 与现码一致。
- **Suggestion**: 保留基线；大拆分后更新 LOC 表。
- **Source**: trace-deps

### F-033 [Info] 核心量化声明与现码大体一致（149/40/0.4.0/6 stores/8 pages）

- **Dimension**: correctness
- **Location**: `docs/analysis/03-ipc-command-catalog.md`:7
- **Evidence**: generate_handler 149；#[tauri::command] 149；invoke 字面量 146；迁移 max 40；version 0.4.0；stores 6；pages 8；边界违规位点一致。
- **Suggestion**: CI 冻结 149/40/dead-command 列表防漂移。
- **Source**: trace-deps

### F-034 [Info] 未跟踪文件未发现真实密钥材料

- **Dimension**: security
- **Location**: `.openclaude/settings.local.json`:1
- **Evidence**: settings.local.json 为 {}；docs/analysis 无 API key/私钥。
- **Suggestion**: 保持零密钥；settings.local 勿提交凭据。
- **Source**: security-pass


## Recommendations (Priority Order)

1. **文档未披露：冷启动 orphan recovery 无差别将 running 会话标 failed，可能毒化自动化** (`docs/analysis/04-runtime-lifecycle.md`) — 文档写清冷启动有损 reconcile；实现改为仅 Manager 无 live process 时标记 interrupted；区分 app_restart_interrupted 与真实失败，避免误 finalize_terminal_failure。
2. **文档未披露：resume_pending 单任务失败 fail-stop 且未过滤软删任务** (`docs/analysis/04-runtime-lifecycle.md`) — SQL 加 deleted_at 过滤；软删时停 automation；resume 改为 per-task try/continue；文档声明启动恢复非全量保证。
3. **Git 高风险 confirm 被文档标为能力完整，实则 DefaultHasher token + 非原子 claim** (`docs/analysis/01-domain-capability-matrix.md`) — HMAC 密钥签 token；条件 UPDATE 原子 claim 后再执行；收紧 sql execute。文档勿把 request/confirm 等同安全二次确认。
4. **sql:allow-execute + csp:null 组合风险在文档中被低估为「中」** (`docs/analysis/05-quality-risks.md`) — 风险表将 sql-execute/csp:null 升为 High/Critical；区分「当前调用点」与「权限面」；生产移除 allow-execute 并配置严格 CSP。
5. **shell:allow-execute 无白名单，文档标「低–中」严重低估** (`docs/analysis/05-quality-risks.md`) — 删除未使用的 shell:allow-execute/kill/stdin-write；仅保留严格 spawn 白名单；文档将此项升为 High。
6. **sanitize_sql_backup_script 被文档列作安全观察，实际几乎不防恶意 SQL** (`docs/analysis/05-quality-risks.md`) — 仅接受本应用签名/白名单导出格式；或 VACUUM INTO 二进制快照。文档明确 sanitize 非安全边界。
7. **路线图将体验 C 置于正确性之前，可能误配工程资源** (`docs/analysis/06-tech-debt-roadmap.md`) — 重排：resume 守卫 + soft-delete 运行时清理 + git confirm 原子化 + 禁用 sql/shell execute 高于看板 UX；更新一句话结论。
8. **AI 多引擎路由错误指向 determine_effective_provider** (`docs/analysis/01-domain-capability-matrix.md`) — 脚注改为：传输层看 determine_effective_provider(sdk|exec)；跨引擎看 employee/settings 的 ai_provider 与 automation 启动分支。
9. **自动质控审核路径硬编码 Codex，与「三引擎统一」叙述矛盾** (`docs/analysis/04-runtime-lifecycle.md`) — 能力矩阵拆分 review vs fix 引擎列；文档写死 review 仅 Codex 或实现 reviewer provider 路由。
10. **软删/回收站被标 ✅，未描述进程与 automation 泄漏及 restore 过宽** (`docs/analysis/01-domain-capability-matrix.md`) — 能力矩阵对 soft-delete 改为 ⚠️；补充删除矩阵（DB/进程/worktree/附件）；实现软删时 stop session + disable automation。
11. **读路径是结构性双轨，文档弱化为「仍混用」** (`docs/analysis/00-architecture-overview.md`) — 边界状态拆成：写路径 P0 泄漏 vs 读路径 CQRS 双轨；更新 ADR 明确是否允许只读 plugin-sql。
12. **Claude.md 过时架构数字未列入产品文档债，易与 analysis 对撞** (`docs/analysis/05-quality-risks.md`) — P0-4/P2-8 扩展同步修订 Claude.md；README 声明权威优先级：analysis/ADR > 过时 agent 指南数字。

## Merge Gate Hint

**BLOCK（就生产决策依赖而言）**: 存在 Critical 文档缺口/系统风险——若仅合并文档可合并，但不得把当前 analysis 当作 resume/Git confirm/权限模型的生产真相源，须先修订。

**文档合入**: 可作为分析草稿合入，但建议同步修订 Critical/High 相关章节。

## 核心结论（中文摘要）

1. **文档基线质量高**：版本 0.4.0、149 commands、40 migrations、超大文件 LOC、前端 3 处 activity INSERT、`sql:allow-execute` 等关键量化声明与现码一致，分析系列可作为新人心智模型入口。
2. **Critical 缺口集中在「未写清的生产失败模式」**：冷启动 orphan→failed 毒化自动化、resume fail-stop+软删交叉、Git confirm 弱 token——这些会让「按文档排期体验优先」变成错误工程决策。
3. **安全权限面被系统性低估**：`sql:allow-execute` + `shell:allow-execute` + `csp:null` + 弱 backup sanitize；05 表应升级 severity。
4. **架构叙事需校准**：读路径是结构性双轨而非「少量混用」；automation review 实际 Codex-only；「服务层」=IPC 边界而非内部分层。
5. **优先修订建议**：先改 04/05/06 的 Critical 叙述与 P0 排序，再考虑把 docs/analysis 正式入库；同步刷新 Claude.md 过时数字以免 agent 对撞。

## Artifacts

- workDir: `.workflow/.scratchpad/code-review-level-20260725163827`
- `level-scope.json` / `context.json` / `findings.json` / `completion.json`
- `findings/*.json`（按维度拆分）
