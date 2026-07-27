# 01 · 领域能力矩阵

> 功能 × 本地/SSH × 引擎 覆盖表（代码事实 + 部分 UI 推断）

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
| 归档管理 / 回收站 | ✅ | ✅ | |

> **更正**：`BUG.md` 中「无标签/依赖/导入」部分已过时——标签、依赖、里程碑已在 migration 40 + `app/delivery.rs` 落地。

## 3. AI 员工

| 能力 | 状态 | 说明 |
|------|------|------|
| CRUD + 角色 | ✅ | developer / reviewer / tester / coordinator |
| 绑定 AI 提供商 | ✅ | `ai_provider` 列 v31 |
| 模型 / 推理强度 | ✅ | |
| 多任务并发运行 | ✅ | runtime 按 session 聚合（TASK 已完成） |
| 离线禁止运行 | ✅ | 业务规则存在 |
| 测试员生成验收清单 | ✅ | `ai_generate_tester_acceptance` |
| 协调员生成计划 | ✅ | `ai_generate_coordinator_task_plan` |
| 角色产品定位文档 | ⚠️ | TASK 仍有「角色价值」讨论项 |

## 4. 三引擎会话

| 能力 | Codex | Claude | OpenCode |
|------|-------|--------|----------|
| 设置 CRUD | ✅ | ✅ | ✅ |
| 健康检查 / 安装 SDK | ✅ | ✅ | ✅ |
| start / stop / stop_session | ✅ | ✅ | ✅ |
| restart | ✅ | ❌（未见 command） | ❌ |
| send input | ✅ `send_codex_input` | ❌ | ❌ |
| resume 会话 | ✅（prepare + start 参数） | ✅（start 参数） | ✅（start 参数） |
| AI 辅助命令簇* | ✅（集中在 codex 模块） | — | — |
| 会话落库统一表 | ✅ | ✅ | ✅（`ai_provider`） |
| 文件变更/diff | ✅ | ✅ | ✅ |
| SSH 远程执行 | ✅ | ⚠️ | ⚠️ |
| 启动时服务 | — | — | ✅ SDK server |

\* AI 辅助（指派、复杂度、计划、拆分、评论、优化 prompt、commit message、协调员计划、测试验收）command 注册在 `codex::` 命名空间，由当前员工/设置决定实际 provider 行为——**能力入口在 Codex 模块，不等于仅 Codex 引擎**（需结合 `determine_effective_provider` 再确认运行时路由，标 `UNVERIFIED` 细节）。

## 5. 代码审查与自动质控

| 能力 | Local | SSH |
|------|-------|-----|
| 收集 git diff / untracked 上下文 | ✅ | ⚠️ 捕获模式受限 |
| `start_task_code_review` | ✅ | ✅ |
| verdict 解析与状态回写 | ✅ | ✅ |
| review_fix_loop_v1 | ✅ | ⚠️ |
| 最大修复轮次 / 失败策略 | ✅ | ✅ |
| 启动 resume pending automation | ✅ | ✅ |
| 审核通过自动 commit | ✅ | ⚠️ |
| 附件远程同步 | — | ✅ helpers |

Artifact 捕获模式（`shared` 常量）：

- `local_full`
- `ssh_full`
- `ssh_git_status`
- `ssh_none`

SSH 下 diff 明细可能降级 → UI 有 `SshArtifactLimitedNotice`。

## 6. 横切

| 能力 | 状态 |
|------|------|
| 通知中心 sticky/one-time | ✅ 文档齐全 |
| 桌面系统通知 | ✅ |
| 全局搜索 project/task/employee/session | ✅ |
| 仪表盘统计 + 活动筛选分页 | ✅ |
| 活动日志中文 key | ✅ 大量映射在 `getActivityActionLabel` |
| 暗色主题 | ✅ |
| 托盘 + 关窗隐藏 + 尺寸恢复 | ✅ |
| 数据库备份/恢复 | ✅ SQL 级 |
| MCP 管理 | ❌ TASK 未完成 |
| 报表燃尽/趋势 | ❌ 基础仪表盘 only |
| 跨工具导入导出 | ⚠️ 仅 DB SQL backup，无 JSON/CSV/Jira |

## 7. 主路径能力缺口（体验向）

结合路线图选项 **C · 体验打磨**，能力已齐但体验摩擦点：

1. 看板 / 任务详情组件过大（`TaskCard` 1.3k、`TaskDetailDialog` 1.3k）→ 交互延迟风险  
2. SSH 产物捕获降级需更清晰的全局状态提示  
3. 三引擎能力不对称（input/restart 仅 Codex 完整）→ 设置/会话页需统一能力徽章  
4. 全局搜索写 activity 走前端 SQL，与其他日志路径不一致  
5. 标签/依赖/里程碑后端已有，UI 发现性可能不足（需产品验证）
