# 03 · IPC 命令与契约清单

## 1. 统计

| 指标 | 数值 | 来源 |
|------|------|------|
| `lib.rs` 注册 commands | **149** | `generate_handler!` |
| `#[tauri::command]` 注解 | 149 | 全库扫描 |
| 前端 `invoke("...")` 字面量 | ~146 | `src/**` |
| 主封装文件 | `src/lib/backend.ts` (~1066) | + `codex.ts` / `claude.ts` / `opencode.ts` |

结论：前后端 command 名称基本对齐；未发现「前端 invoke 不在 handler」的明显死调用。部分 command 仅封装未在业务页深层使用的情况需按功能再验证。

## 2. 按域分类

### 2.1 Git（43）— `git_workflow::*`

**查询类**

- `get_project_git_overview`
- `list_project_git_commits` / `get_project_git_commit_detail`
- `open_project_git_file` / `get_project_git_file_preview` / `get_project_git_commit_file_preview`
- `list_project_git_worktrees` / `get_project_worktree_file_preview`
- `list_task_git_contexts` / `get_task_git_context` / `get_task_git_commit_overview`

**暂存 / 回滚 / 提交**

- stage/unstage 单文件与全部（项目工作区 + worktree + task）
- rollback 选中/全部（项目 + worktree）
- `commit_project_git_changes` / `commit_project_worktree_changes` / `commit_task_git_changes`
- `generate_project_worktree_commit_message`

**分支**

- push / pull / checkout / create / delete / merge branches

**Worktree / 任务上下文**

- `prepare_task_git_execution` / `refresh_task_git_context` / `reconcile_task_git_context`
- `remove_project_git_worktree` / `merge_project_git_worktree`
- `delete_task_git_context_record`

**高风险确认**

- `request_git_action` / `confirm_git_action` / `cancel_git_action`

### 2.2 Codex 引擎与 AI 命令（22）— `codex::*`

**设置 / 模板 / SDK**

- `get_codex_settings` / `update_codex_settings`
- `get_remote_codex_settings` / `update_remote_codex_settings`
- `get_ai_prompt_templates` / `update_ai_prompt_templates` / `reset_ai_prompt_templates`
- `install_codex_sdk`

**会话**

- `start_codex` / `stop_codex` / `stop_codex_session` / `restart_codex` / `send_codex_input`

**AI 辅助**

- `ai_suggest_assignee` / `ai_analyze_complexity` / `ai_generate_comment`
- `ai_generate_commit_message` / `ai_optimize_prompt` / `ai_generate_plan`
- `ai_generate_coordinator_task_plan` / `ai_generate_tester_acceptance` / `ai_split_subtasks`

### 2.3 任务（16）— `app::tasks::*`

- CRUD：`create_task` / `update_task` / `update_task_status` / `delete_task`
- 回收站：`permanently_delete_task` / `restore_task` / `list_trashed_tasks`
- 附件：`add_task_attachments` / `delete_task_attachment`
- 子任务/评论：`create_subtask` / `update_subtask_status` / `delete_subtask` / `create_comment`
- 自动化：`set_task_automation_mode` / `get_task_automation_state`
- 计时：`start_task_timer`

### 2.4 交付管理（12）— `app::delivery::*`

- milestones：list/create/update/delete  
- tags：list/create/delete + `list_task_tags` / `set_task_tags`  
- dependencies：list/add/remove  

### 2.5 Remote / SSH（9）— `app::remote::*`

- `list_ssh_configs` / `get_ssh_config` / CRUD create/update/delete  
- `probe_ssh_password_auth`  
- `validate_remote_codex_health` / `install_remote_codex_sdk`  
- `sync_system_notifications`  

### 2.6 Sessions / 搜索 / 审查读模型（8）— `app::sessions::*`

- `search_global` / `list_codex_sessions`  
- `prepare_codex_session_resume` / `get_codex_session_log_lines`  
- `get_task_latest_review`  
- `get_task_execution_change_history`  
- `get_codex_session_execution_change_history`  
- `get_codex_session_file_change_detail`  

### 2.7 OpenCode（8） / Claude（7）

| OpenCode | Claude |
|----------|--------|
| get/update settings | get/update settings |
| check/install SDK | check/install SDK |
| start / stop / stop_session | start / stop / stop_session |
| get_opencode_models | — |

### 2.8 Employees（6） / Projects（6）

- employees：create/update/delete/status + `get_employee_runtime_status` / `get_codex_session_status`  
- projects：create/update/delete + trash permanently/restore/list  

### 2.9 Database（4） / Review 附件（3） / Notifications（3） / Tray（1） / Automation（1）

- DB：`health_check` / `backup_database` / `restore_database` / `open_database_folder`  
- Review：`start_task_code_review` / `read_image_file` / `open_task_attachment`  
- Notif：`list_notifications` / `mark_notification_read` / `mark_all_notifications_read`  
- `show_main_window`  
- `restart_task_automation`  

## 3. 前端封装分层

```
UI / Stores
  → backend.ts     业务 + git + settings + delivery + notif
  → codex.ts       会话生命周期 + AI 辅助 invoke
  → claude.ts      Claude 会话
  → opencode.ts    OpenCode 会话/设置/模型
  → database.ts    ⚠️ 直接 SQL（非 IPC）
```

## 4. 契约风险

| 风险 | 说明 |
|------|------|
| 参数 camelCase | Tauri 默认 serde rename；`backend.ts` 混用 payload 对象 |
| Raw 归一化 | SSH/health 等在 `backend.ts` 做字段兼容（`password_auth_available` 等） |
| 事件流 | 会话 output/exit、notification-center-changed、task-automation-state-changed 等靠 `listen`，不在 command 列表 |
| 无 OpenAPI | 契约靠人工 diff models/types/backend |

## 5. 非 IPC 写路径（Boundary）

| 位置 | 操作 | 应改为 |
|------|------|--------|
| `projectStore.ts` | `execute(INSERT activity_logs)` | `insert_activity_log` 类 command 或已有后端日志 API |
| `GlobalSearchDialog.tsx` | 同上 | 搜索导航日志走后端 |
| capabilities | `sql:allow-execute` | 收紧为仅 select 或移除前端 SQL |

## 6. 引擎能力不对称（契约视角）

| Command 能力 | Codex | Claude | OpenCode |
|--------------|-------|--------|----------|
| restart | ✅ | ❌ | ❌ |
| send_input | ✅ | ❌ | ❌ |
| models 列表 | 设置内 | 设置内 | ✅ `get_opencode_models` |
| AI 辅助簇 | ✅（共享入口） | 经由 provider 路由 `UNVERIFIED` | 同左 |

## 7. 维护建议

1. 从 `lib.rs` 自动生成 command 清单（CI 检查前端 invoke ⊆ 注册表）  
2. 事件名集中常量文件（前后端各一份或共享 JSON）  
3. delivery / automation 变更时同步 `types.ts` 与 activity 中文 map  
