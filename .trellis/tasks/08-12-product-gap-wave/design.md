# Design · 2026-08-11 产品缺口下一波

> 依据四路代码探索(2026-08-12),证据见各子任务 PRD 与本文引用。迁移版本规划:v45 token 列、v46 运行队列表、v47 任务模板表。`latest_migration_version` 测试随最后一个迁移更新。

## A1 · 成本可见性

**数据层**
- v45:`ALTER TABLE codex_sessions ADD COLUMN input_tokens INTEGER / output_tokens INTEGER / total_tokens INTEGER / reasoning_tokens INTEGER`(全 nullable)。
- `models.rs::CodexSessionRecord` 加 4 字段;`app/sessions.rs` 的 INSERT 列清单同步;测试夹具中显式 INSERT 列清单同步(app/tests/mod.rs、review_and_attachments.rs、task_automation/tests_modules.rs、session_events_retention.rs)。
- 新增 `app/sessions.rs::apply_codex_session_usage(pool, session_record_id, delta)`:累加式 UPDATE(`COALESCE(col,0)+delta`),无 usage 事件则保持 NULL(不假装 0)。

**共享解析(engine/usage.rs 新增)**
- `UsageDelta { input, output, total, reasoning: Option<u64> }` + `parse_usage_value(&Value)`:移植 Grok `usage_u64` 的多 key 兜底(input_tokens/inputTokens/prompt_tokens…);total 缺失时 in+out 推导。

**各引擎提取点**(流解析纯函数产出 `Option<UsageDelta>`,runtime 持 pool 落库):
- Grok:`parse_grok_json_event_line` 的 `Some("usage")` 分支已有 `summarize_usage`,追加结构化 delta 输出。
- Claude CLI:`parse_claude_cli_json_event_line` 的 `Some("result")` 分支读顶层 `usage`。
- Claude SDK bridge:`claude_sdk_bridge.mjs` `case "result"` 输出结构化 usage 标记行,Rust 侧解析(仿 file-change 标记机制)。
- Codex CLI(--json):`parse_cli_json_event_line` 加 `turn.completed` 分支读 `usage`。
- Codex SDK bridge:`sdk_bridge.mjs` `turn.completed` 透传 usage 标记行。
- OpenCode bridge:`opencode_sdk_bridge.mjs` 按 messageID 聚合 tokens,emit `usage` 事件;`stream_opencode_output` match 加分支。
- 标记行格式统一 `[[codex-ai:usage]]{json}`,终端不显示。

**展示**
- 任务详情执行 Tab:新命令 `get_task_token_usage(task_id)`(SUM 全部会话),`TaskExecutionPanel` 头部显示「Token 用量」(全 NULL 则不显示)。
- 仪表盘:`DashboardReportSummary` 加 `token_usage`(总量)/ `token_usage_series`(按 trend_range 日/周桶)/ `token_usage_by_provider`;`DashboardPage` 加卡片。按 `codex_sessions.started_at` 聚合,受项目 scope 过滤。

## B1 · 并发闸门 + 运行队列

**语义边界(豁免声明)**:闸门与队列作用于「任务执行会话」(session_kind=execution、有 task_id、非 resume)。review/fix/planning 等内部会话**计入并发数但不排队**——排队会打断 task_automation 状态机,且它们是已获准工作流的后续。写入 PRD 豁免条款。

**配置**:`CodexSettings.max_concurrent_sessions: Option<u32>`(serde default 3,0=不限),`codex-settings.json` 持久化;RuntimeSettingsTab 加数字输入。

**表(v46)**:`task_run_queue(id PK, task_id UNIQUE NOT NULL, provider NOT NULL, payload TEXT NOT NULL, status 'queued', enqueued_at, created_at)`。payload=引擎 start 参数 JSON,重启可重放。

**闸门(crate 根新模块 run_queue.rs,仿 task_automation 布局)**
- `count_running(app)`:四个 manager 进程表求和 + in-flight 预约计数(Mutex,防 check→add_process 窗口竞态)。
- 四个 `start_*_with_manager` 冲突检查后调 `run_queue::gate_or_enqueue(...)`:可排队会话且超限 → 写队列行 + emit `task-run-queue-changed` + 返回 `StartOutcome::Queued{position}`;否则预约 slot 继续。启动失败/add_process 完成后释放预约。
- 四个 start 命令返回值改为 tagged 结构(started|queued),前端 `taskRunSession.ts` 适配:queued 时提示排队并跳过员工/计时副作用。

**放行**:四引擎会话退出汇合点(`handle_session_exit_blocking` 之后/各 runtime remove_process 后)调 `run_queue::spawn_drain(app)`:容量富余时按 enqueued_at 取队首,按 provider 重放 start(带 bypass 标记防再入队)。启动失败 → 移除该行 + 活动日志记失败。
- `lib.rs` setup 加 `run_queue::spawn_resume(app)`(仿 spawn_resume_pending_automation)。

**前端**
- taskStore 监听 `task-run-queue-changed` + `list_task_run_queue` 命令;TaskCard 徽标「排队中(第 N 位)」+ 右键取消排队(`cancel_queued_task_run`);排队中主 CTA 锁定。
- 批量运行:KanbanPage 已有多选批量条,加「批量运行」按钮,顺序对选中可运行任务走既有 `startTaskRunSession`(闸门在 Rust,天然前 N 个启动、其余排队)。
- 「排队中」不新增 kanban status 列,只做徽标。

**活动日志**:task_run_queued / task_run_dequeued / task_run_queue_cancelled(zh/en activity:actions)。

## A2 · 日志复制/导出

`CodexTerminal.tsx` 头部(Eraser 旁)加复制/导出按钮,数据源 `output`(组件内已聚合 lines/sessionLogs/taskLogs):
- 复制:`navigator.clipboard.writeText(output.map(o=>o.line).join("\n"))`(仓库既有模式),成功切换 Check 图标。
- 导出:Blob + `<a download>`(CSV 既有模式),文件名 `session-log-{taskId|sessionRecordId}-{yyyyMMdd-HHmmss}.log`。
- 空输出禁用;i18n 键加 `sessions.json`(zh/en)。所有引用方自动获得。

## D1 · 自动更新

- Rust:`tauri-plugin-updater`+`tauri-plugin-process`(relaunch);lib.rs 注册(desktop);capabilities 加 `updater:default`、`process:allow-restart`。
- 配置:`tauri.conf.json` `bundle.createUpdaterArtifacts: true` + `plugins.updater.{pubkey, endpoints:[GitHub latest.json]}`。密钥:本地 `tauri signer generate` 生成,公钥入库,私钥不入库(用户配 CI secret `TAURI_SIGNING_PRIVATE_KEY`)。
- 前端:`@tauri-apps/plugin-updater`/`plugin-process`;RuntimeSettingsTab 新「关于与更新」节:`getVersion()` 当前版本 + 检查更新 + 下载进度 + 安装后重启;错误可见。
- CI:`build.yml` 三平台构建注入签名 env;发版 job 汇总 `.sig` 生成 `latest.json`(新 `scripts/build-latest-json.mjs`)挂到 release。

## B2 · 任务模板

- 表(v47):`task_templates(id PK, name NOT NULL, description, project_id NULL, title_template NOT NULL, description_template, priority, use_worktree INTEGER DEFAULT 0, tags_json, subtasks_json, created_at, updated_at, deleted_at)`(软删,查询过滤 deleted_at)。
- 新 `app/templates.rs`:list/create/update/delete(软删)/`create_task_template_from_task`/`apply_task_template(template_id, project_id, variable_sets[], assignee_id?)`。变量 `{{name}}`,`extract_template_variables` + 替换;缺变量报中文错;单次 ≤100 组(对齐 batch 限制)。套用循环走 `create_task` 同等内部逻辑(含 tags/subtasks),活动日志 task_template_applied(含数量)。
- 前端:KanbanPage 头部「模板」按钮(新建任务/归档管理旁)→ TaskTemplateManagerDialog(列表/删除/套用:变量多行表格批量生成);TaskCard 右键「存为模板」。i18n kanban/tasks 命名空间。

## C1 · 审查行级定位

- 契约:`build_task_review_prompt` 增加第三个输出块 `<review_findings>`:JSON 数组 `{file(相对路径), line(新文件行号), severity(blocker|warning|info), message}`;无问题输出 `[]`。`codex/prompt_templates.rs` review 场景默认文案同步。
- 解析:shared.rs 加标签常量;`extract_review_findings` + `parse_review_findings_json`(severity 归一化,畸形→None 降级),各引擎会话结束提取处与 review_verdict 并列写 `codex_session_events(event_type='review_findings')`。
- 读取:`get_task_latest_review` 返回体加 `findings`;前端 `TaskReviewPanel` 列表展示(severity 徽标+file:line+message),点击 → 在该任务最新执行会话文件变更中匹配文件 → 打开变更详情弹窗定位到行。
- 变更详情弹窗 `TaskExecutionChangeDetailDialog` 升级为 Monaco DiffEditor(仿 ProjectGitFilePreviewDialog)+ `revealLineInCenter` + 行高亮 decoration;匹配不到文件时仅提示无法定位。

## 通用约束

- 每功能:活动日志(如适用)+ `activity:actions.*` zh/en、i18n 文案 zh/en、时间显示 formatDate、SSH 模式同路径生效(A1 解析在流层/B1 闸门在 start 层/其余与目标无关)。
- 每功能完成跑:clippy -D warnings、`npm run format:check`;波次收尾跑全量门禁。
- 提交:按功能 Conventional Commits。
