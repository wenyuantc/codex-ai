# Implement · 执行计划

顺序:A1 → A2 → B1 → B2 → C1 → D1(D1 涉及依赖安装与密钥生成放最后)。每步完成:clippy + format:check + 相关测试 → 按功能提交 → 勾选。

## 1. A1 成本可见性(feat(sessions): token 用量落库与展示)

- [x] v45 迁移 + `latest_migration_version` 测试 44→45
- [x] `CodexSessionRecord`(Rust+TS)加 4 token 字段;INSERT 列清单(app/sessions.rs + 测试夹具 4 处)
- [x] `engine/usage.rs`:UsageDelta + parse_usage_value(+单测,移植 grok usage_u64)
- [x] `apply_codex_session_usage` 累加 UPDATE
- [x] Grok stream usage 分支产出 delta → runtime 落库
- [x] Claude CLI result 分支 + SDK bridge 标记行 → 落库
- [x] Codex CLI turn.completed + SDK bridge 标记行 → 落库
- [x] OpenCode bridge usage 事件 → mod.rs 落库
- [x] `get_task_token_usage` 命令 + TaskExecutionPanel 展示 + i18n
- [x] Dashboard summary 加 token_usage/series/by_provider + DashboardPage 卡片 + i18n
- [x] 各引擎 stream 单测喂样例行;验证:cargo test + clippy + format:check

## 2. A2 日志复制/导出(feat(sessions): 终端日志复制与导出)

- [x] CodexTerminal 头部复制/导出按钮 + Blob 下载 + i18n(zh/en)
- [x] 验证:npm run build + format:check

## 3. B1 并发闸门+队列(feat(engine): 全局并发闸门与运行队列)

- [x] v46 task_run_queue 表 + 版本测试 45→46
- [x] CodexSettings.max_concurrent_sessions(默认3,0不限)+ RuntimeSettingsTab 输入 + i18n
- [x] run_queue.rs:count_running(四 manager)+ in-flight 预约 + gate_or_enqueue + drain + resume + cancel/list 命令
- [x] 四个 start_*_with_manager 接闸门,返回 started|queued;lib.rs 注册新命令 + setup spawn_resume
- [x] 会话退出汇合点接 spawn_drain
- [x] 前端:start 返回类型适配、taskRunSession queued 分支、taskStore 队列状态+事件、TaskCard 排队徽标+取消、KanbanPage 批量运行
- [x] 活动日志 3 键 + activity:actions zh/en
- [x] Rust 测试:入队/出队/取消/重启恢复/闸门计数;验证三件套

## 4. B2 任务模板(feat(tasks): 任务模板)

- [x] v47 task_templates 表 + 版本测试 46→47
- [x] app/templates.rs CRUD + from_task + apply(变量替换,≤100)+ lib.rs 注册
- [x] 变量提取/替换单测 + apply roundtrip 测试
- [x] 前端:KanbanPage「模板」入口 + 管理/套用 Dialog + TaskCard「存为模板」+ backend.ts + i18n
- [x] 活动日志 2 键 + zh/en;验证三件套

## 5. C1 审查行级定位(feat(review): 行级 findings 与 diff 锚定)

- [x] prompt 契约 + prompt_templates 默认文案同步
- [x] shared 标签 + extract/parse findings(+畸形降级单测)
- [x] 各引擎会话结束提取写 review_findings 事件
- [x] get_task_latest_review 返回 findings;TaskReviewPanel 列表 + 点击定位
- [x] TaskExecutionChangeDetailDialog 升级 Monaco Diff + revealLine + 高亮
- [x] i18n zh/en;验证三件套

## 6. D1 自动更新(feat(app): 应用内检查更新)

- [x] cargo add tauri-plugin-updater/process;npm i @tauri-apps/plugin-updater/process
- [x] 生成签名密钥(私钥存本地不入库),pubkey + endpoints 入 tauri.conf.json,createUpdaterArtifacts
- [x] lib.rs 注册插件;capabilities 权限
- [x] RuntimeSettingsTab 关于与更新节(版本/检查/进度/重启)+ i18n
- [x] build.yml 签名 env + scripts/build-latest-json.mjs + release 挂 latest.json
- [x] README 发版 secrets 说明;验证:npm run build + clippy

## 7. 收尾

- [x] 全量门禁:clippy / cargo test / npm run lint / format:check / test:ci / build
- [x] TASK.md 勾选;CLAUDE.md 计数校准(迁移数 44→47、命令数、测试数);README 引擎/表数如受影响
- [x] Trellis:spec 更新(如有沉淀)、子任务已归档
- [ ] journal 记录（`/trellis-finish-work`）

## 回滚点

每功能独立提交;任一功能失败可 revert 单个提交,迁移版本连续性需同时回退对应 vN。
