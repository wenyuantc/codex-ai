# 01 · 领域能力矩阵

> 功能 × 本地/SSH × 引擎 覆盖表（代码事实 + 部分 UI 推断）
>
> **最后校准**：2026-08-11，随 `08-10-p3-send-input`（真会话 send_input）更新 §4 能力矩阵与诚实边界。
> 上一版基线为 2026-08-07（`08-05-product-capability-roadmap` 收尾）。

## 图例

| 符号 | 含义 |
|------|------|
| ✅ | 已实现且有 command/UI 证据 |
| ⚠️ | 部分实现 / 能力受限 / 模式差异大 |
| ❌ | 未实现或引擎不支持 |
| — | 不适用 |

## 1. 项目管理

| 能力 | Local | SSH | 关键路径 |
|------|-------|-----|----------|
| 项目 CRUD / 软删 / 回收站 | ✅ | ✅ | `app/projects.rs` |
| 项目类型 local/ssh | ✅ | ✅ | migrations v23+ |
| Git overview / 提交历史 / 文件预览 | ✅ | ⚠️（依赖远程 git） | `git_workflow` |
| 分支切换/创建/删除/合并 | ✅ | ⚠️ | 同上 |
| Worktree 管理 | ✅ | ⚠️ | 同上 + `ProjectWorktreeSection` |
| AI 生成 commit message | ✅ | ⚠️ | `ai_generate_commit_message` / worktree 变体 |
| 高风险 Git 确认流 | ✅ | ✅ | `request/confirm/cancel_git_action` |
| 项目级测试命令 `test_command` | ✅ | ✅ | migration v43（供 §3 测试员使用） |

## 2. 任务与看板

| 能力 | Local | SSH | 说明 |
|------|-------|-----|------|
| 状态机 todo/in_progress/review/completed/blocked/archived | ✅ | ✅ | `update_task_status` |
| 看板拖拽 | ✅ | ✅ | `KanbanBoard` |
| 优先级 / 复杂度 | ✅ | ✅ | |
| 执行人/审核人/协调员 | ✅ | ✅ | migrations v18/v34 |
| Worktree 模式任务 | ✅ | ⚠️ | `worktree` 偏好 v27 |
| 子任务 / 评论 / 附件 | ✅ | ⚠️ 附件需远程同步 | `review` sync helpers |
| 计时 time tracking | ✅ | ✅ | migrations v36 |
| 标签 tags | ✅ | ✅ | delivery v40 + UI |
| 依赖 dependencies | ✅ | ✅ | delivery v40 + UI |
| 里程碑 milestones | ✅ | ✅ | delivery v40 + `ProjectMilestonesSection` |
| **看板按里程碑/标签筛选** | ✅ | ✅ | `lib/kanbanFilters.ts` → `KanbanBoard` |
| 归档管理 / 回收站 | ✅ | ✅ | `ArchiveManagementDialog` |
| 任务主路径 CTA | ✅ | ✅ | `TaskPrimaryActionBar` |
| **任务域 JSON 导出 / 导入** | ✅ | ✅ | `export_tasks_json` / `import_tasks_json` |
| 任务 CSV 导出（旧入口） | ✅ | ✅ | `export_tasks_csv`，UI 主路径已换 JSON |

> **更正（保留）**：`BUG.md` 中「无标签/依赖/导入」部分已过时——标签、依赖、里程碑在 migration 40 + `app/delivery.rs` 落地；任务导入导出在本轮补齐。

## 3. AI 员工

| 能力 | 状态 | 说明 |
|------|------|------|
| CRUD + 角色 | ✅ | developer / reviewer / tester / coordinator |
| 绑定 AI 提供商 | ✅ | `ai_provider` 列 v31，四引擎可选 |
| 模型 / 推理强度 | ✅ | |
| 多任务并发运行 | ✅ | runtime 按 session 聚合 |
| 离线禁止运行 | ✅ | 业务规则存在 |
| 测试员生成验收清单 | ✅ | `ai_generate_tester_acceptance` |
| **测试员自动化闭环** | ✅（默认关闭） | 见 §5「验收阶段」；开关 `tester_automation_enabled` 默认 `false`，需在设置中启用 |
| 协调员生成计划 | ✅ | `ai_generate_coordinator_task_plan` |
| 协调员结构化流水线 | ✅ | `task_pipeline_steps` + 阶段视图（§5） |
| 角色产品定位文档 | ⚠️ | 四角色均已进入主链路，但产品文案/定位说明仍偏薄 |

## 4. 四引擎会话

> 能力真源：`app/database.rs::get_ai_provider_capabilities`（前端 `src/lib/aiCapabilities.ts`），并有 `capability_matrix_is_honest_and_complete` 单测守护。

| 能力 | Codex | Claude | OpenCode | Grok |
|------|-------|--------|----------|------|
| 设置 CRUD | ✅ | ✅ | ✅ | ✅ |
| 健康检查 / 安装 SDK | ✅ | ✅ | ✅ | ✅ |
| start / stop / stop_session | ✅ | ✅ | ✅ | ✅ |
| restart\* | ✅ | ✅ | ✅ | ✅ |
| send input（会话中 stdin） | ✅ SDK | ✅ SDK | ✅ SDK bridge | ❌ B1（headless `-p` + `Stdio::null`） |
| resume 会话 | ✅ | ✅ | ✅ | ✅ |
| AI 辅助命令簇\*\* | ✅（命令注册在 codex 命名空间） | — | — | — |
| 会话落库统一表 | ✅ | ✅ | ✅ | ✅ |
| 文件变更/diff | ✅ | ✅ | ✅ | ✅ |
| SSH 远程执行 | ✅ | ✅ | ✅ SDK bridge over SSH | ✅ |
| 启动时服务 | — | — | ✅ SDK server | — |
| UI 能力徽章 | ✅ | ✅ | ✅ | ✅ `EngineCapabilityBadges` |

\* `restart_*` = 停止该员工在跑的进程后重新 `start_*`，**不是** CLI 层面 resume 旧 session id。

\*\* AI 辅助（指派、复杂度、计划、拆分、评论、优化 prompt、commit message、协调员计划、测试验收）command 注册在 `codex::` 命名空间，由当前员工/设置决定实际 provider 行为——**能力入口在 Codex 模块，不等于仅 Codex 引擎**（运行时路由见 `determine_effective_provider`）。

**诚实边界（2026-08-11）**：`send_input` 在 Codex / Claude / OpenCode 为 `true`（SDK bridge 保留可写 stdin；交互会话默认 `awaitFollowups`：首轮结束后进程保持存活并等待后续输入；编排/流水线 mid-flight 设 `awaitFollowups:false` 以便 drain 后立即退出推进自动化。CLI 批处理通道无 stdin 时命令返回明确错误）。进程退出后需 resume/新会话，不用假 send_input。Grok 为 B1 豁免 `false`（headless CLI，`Stdio::null`，证据见 `.trellis/tasks/08-10-p3-send-input/notes-b1-grok.md`）。UI 控件必须走 `can(provider, cap)` 门控；矩阵加载失败时 fail-closed。

## 5. 验收、代码审查与自动质控

主链路顺序为 **先测后审**：开发会话结束 → （若启用）测试员验收 → code review / review_fix_loop。

| 能力 | Local | SSH | 关键路径 |
|------|-------|-----|----------|
| 测试命令执行（exit code 硬判定） | ✅ | ✅ | `task_automation/acceptance.rs`，超时 900s，输出摘要上限 8000 字符 |
| 验收清单生成 / 编辑 | ✅ | ✅ | `update_task_acceptance_checklist`（Monaco 编辑） |
| 验收历史 | ✅ | ✅ | `get_task_acceptance_runs`；migration v43 |
| 手动触发验收 | ✅ | ✅ | `run_task_acceptance`（trigger=manual/auto） |
| 收集 git diff / untracked 上下文 | ✅ | ⚠️ 捕获模式受限 | |
| `start_task_code_review` | ✅ | ✅ | |
| verdict 解析与状态回写 | ✅ | ✅ | |
| review_fix_loop_v1 | ✅ | ⚠️ | |
| 最大修复轮次 / 失败策略 | ✅ | ✅ | |
| 启动 resume pending automation | ✅ | ✅ | |
| 审核通过自动 commit | ✅ | ⚠️ | |
| 附件远程同步 | — | ✅ helpers | |

验收阶段 phase：`launching_tester` / `waiting_tester` / `tester_failed`；结论 `passed` / `failed` / `skipped`。**测试命令非 0 即硬失败**，AI 不能覆盖。

协调员流水线操作：

| 能力 | 状态 | Command |
|------|------|---------|
| 步骤列表 | ✅ | `list_task_pipeline_steps` |
| 启动 / 中止 | ✅ | `start_task_pipeline` / `abort_task_pipeline` |
| 失败步骤重试 | ✅ | `retry_task_pipeline_step` |
| 转人工后手动运行单步 | ✅ | `run_task_pipeline_step_manual` |
| 步骤编辑 | ✅ | `update_task_pipeline_step` |
| 详情页阶段视图 / 看板轻提示 | ✅ | 有 steps 才渲染，无 steps 不出空壳 |

Artifact 捕获模式（`shared` 常量）：

- `local_full`
- `ssh_full`
- `ssh_git_status`
- `ssh_none`

SSH 下 diff 明细可能降级 → UI 有 `SshArtifactLimitedNotice`（会话/详情内联）+ `SshTrustBanner`（`MainLayout` 全局）。

## 6. 横切

| 能力 | 状态 | 说明 |
|------|------|------|
| 通知中心 sticky/one-time | ✅ | 文档齐全 |
| 桌面系统通知 | ✅ | |
| 全局搜索 project/task/employee/session | ✅ | 写 activity 已改为 Rust command，前端 SQL 路径消除 |
| 仪表盘统计 + 活动筛选分页 | ✅ | |
| **报表洞察** | ✅ | `get_dashboard_report_summary`：完成率、逾期/阻塞/进行中、近 7 日完成序列、**近 8 周完成序列**、**`aging_in_progress`**、员工负载；尊重项目 + `selected_ssh_config_id` 作用域 |
| 活动日志中文 key | ✅ | 映射在 `getActivityActionLabel`，有单测覆盖未知 key 回退 |
| 暗色主题 | ✅ | |
| 托盘 + 关窗隐藏 + 尺寸恢复 | ✅ | |
| 数据库备份/恢复 | ✅ | SQL 级，`backup_database` / `restore_database` |
| **MCP 管理** | ✅ | 全局清单（`mcp-servers.json`）+ **任务级三态绑定** + 会话启动注入 |
| **跨工具导入导出** | ✅ 任务域 JSON | 版本化 envelope；不含员工/SSH/密钥/附件二进制。Jira/GitHub 同步仍不做 |
| **前端自动化测试** | ✅ | Vitest（node 环境），`src/stores/*.test.ts` + `src/lib/utils.test.ts`；CI `frontend-lint` 硬门禁 |
| 前端直写 SQL | ❌（已封堵） | `src/lib/database.ts` 为 hard-fail stub；`capabilities/default.json` 不授予 `sql:allow-select` / `sql:allow-execute` |

MCP 任务绑定三态（migration v44，`tasks.mcp_server_ids`）：

| 状态 | 存储 | 解析结果 |
|------|------|----------|
| 继承全局（默认） | `NULL` | 全局 `enabled=true` 的 server |
| 显式空集 | `'[]'` | 无 MCP |
| 指定集合 | `'["id1",…]'` | 全局定义中匹配且仍存在的 server |

Command：`get_task_mcp_binding` / `set_task_mcp_binding` / `get_mcp_servers` / `update_mcp_servers` / `reset_mcp_servers` / `export_mcp_servers_snippet`。Codex 本地注入必达；其他引擎复用同一解析器，不可注入时给中文提示而非静默。

## 7. 剩余缺口（体验向）

上一版 5 条中 4 条已在本轮路线图关闭，重估如下：

| 上一版缺口 | 现状 |
|------------|------|
| SSH 产物捕获降级提示不清晰 | ✅ 已关闭：`SshTrustBanner` 全局 + `SshArtifactLimitedNotice` 内联 |
| 引擎能力不对称、缺统一徽章 | ✅ 已关闭：能力矩阵单测守护 + `EngineCapabilityBadges` |
| 全局搜索写 activity 走前端 SQL | ✅ 已关闭：前端 SQL 全面封堵 |
| 标签/依赖/里程碑 UI 发现性不足 | ✅ 已关闭：看板筛选 + 里程碑区块 |
| 看板/详情组件过大 | ❌ 未关闭且加剧：`TaskCard` 1799 行、`TaskDetailDialog` 1975 行 |

当前仍开放：

1. **`TaskCard` / `TaskDetailDialog` 体量**（1.8k / 2.0k 行）——本轮多个子任务继续往这两个文件加功能，交互延迟与可维护性风险上升，建议单开拆分任务。
2. **测试员自动化默认关闭**——`tester_automation_enabled` 默认 `false`，能力已具备但用户不主动开就等于没有；缺引导性 onboarding。
3. **SSH 下 review/自动 commit 仍为 ⚠️**——artifact 捕获降级路径的行为边界已有提示，但未做到与 local 等价。
4. **前端测试覆盖仅到 store 纯函数层**——4 个测试文件、32 条断言，组件与交互层无回归网。
5. **角色产品定位文档偏薄**——四角色技术闭环已通，产品侧说明未同步。

明确不做（路线图级）：微服务拆分、远程多端实时同步、第五个引擎、完整 IDE 体验、Jira/GitHub Issues 双向实时同步。
