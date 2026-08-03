# Design: 协调员编排 v2

## 1. Summary

在 **不引入 Meta-Agent CLI** 的前提下，增加「结构化工作包 + 串行调度」能力：

1. 协调员 one-shot 输出 **Markdown（`plan_content`）+ 机读 steps**  
2. 用户显式点 **「按计划编排」** 启动流水线  
3. 扩展 `task_automation` 会话退出钩子：流水线中的 execution 成功 → 下一步；**最后一步**成功且开启质控 → 既有 `launching_review`  
4. **「立即执行」** 仍走单员工整包，行为与现网一致  

## 2. Boundaries

| 在内 | 在外 |
|------|------|
| 计划结构化持久化、步骤指派编辑、串行启动、失败重试/转人工、恢复、活动日志、双入口 UI | 并行、测试员门禁、跳过步骤、引擎协议对齐、独立编排 daemon |

**模块归属**

| 层 | 位置 | 职责 |
|----|------|------|
| DB | `db/migrations.rs` + models | steps / pipeline 游标字段 |
| 计划生成 | `codex/process/ai_commands.rs` + prompts + templates | 结构化生成与校验 |
| 编排运行时 | `task_automation/*`（新 slice 可 `include!`） | 启步、退出推进、恢复、与质控衔接 |
| 任务 API | `app/tasks.rs` 等 | 列表/更新步骤、start/retry/abort 命令 |
| 前端 | `taskCreateAndRun`、看板/任务详情、activity labels | 双入口、步骤 UI |

## 3. Data Model

### 3.1 推荐：独立步骤表（优于只塞 JSON 列）

便于逐步绑 `session_id`、状态查询、迁移清晰。

```text
task_pipeline_steps
  id TEXT PK
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE
  step_index INTEGER NOT NULL          -- 0-based 或 1-based，实现统一
  title TEXT NOT NULL
  goal TEXT
  success_criteria TEXT
  employee_id TEXT REFERENCES employees(id) ON DELETE SET NULL  -- 建议值，可空
  status TEXT NOT NULL                -- pending | launching | running | succeeded | failed | skipped | cancelled
  session_id TEXT                     -- 最近一次 execution session
  handoff_summary TEXT                -- 本步结束后的交接摘要
  last_error TEXT
  started_at / ended_at / created_at / updated_at
  UNIQUE(task_id, step_index)
```

`tasks.plan_content`：**保留**人类可读 Markdown（兼容现网展示）。

可选：`tasks.plan_structured_version` 或仅以 steps 表为准；**不以 subtasks 表充当编排源**（避免与手工子任务语义混淆）。生成后可选同步标题到 subtasks（非必须，MVP 可不做）。

### 3.2 编排游标（挂在 `task_automation_state`）

避免第二套生命周期表，与 resume 逻辑同源。

新增列（迁移）：

| 列 | 用途 |
|----|------|
| `pipeline_active` INTEGER/bool | 是否处于协调员流水线（中间步禁止直接进审核） |
| `pipeline_step_index` INTEGER | 当前步骤 index |
| 或复用 `pending_action` | 如 `pipeline_run_step` / `pipeline_start_review` |

**新 phase（字符串常量）**

```text
pipeline_launching_step
pipeline_waiting_step
pipeline_step_failed
pipeline_completed          -- 瞬时/日志用，随即转 idle 或 launching_review
```

质控 phase 保持不变：`launching_review` / `waiting_review` / `launching_fix` / `waiting_execution` / …

### 3.3 与 `automation_mode` 关系

- `automation_mode` **仍只表示**是否启用 `review_fix_loop_v1`（质控开关）。  
- 编排由 `pipeline_active` + phases 表达，**不**新增第二种 `automation_mode` 值（避免 `set_task_automation_mode` 校验爆炸）。  
- D4：流水线正常结束后，若 `automation_mode == review_fix_loop_v1` → 走现有 `reserve_pending_action(..., start_review)` 路径。

## 4. Runtime Flow

### 4.1 生成结构化计划

```text
UI / 创建流程
  → ai_generate_coordinator_task_plan (升级)
  → one-shot：员工列表 + 任务 + 附件
  → 模型输出：markdown + JSON steps（推荐 fenced JSON 或双通道约定）
  → 校验 steps（非空 title、index 连续）
  → 写 plan_content + 替换/upsert task_pipeline_steps（status=pending）
  → activity: task_plan_generated / task_pipeline_plan_saved
```

解析失败：返回中文错误，不写半残 steps（或事务回滚）；允许用户重试生成。

### 4.2 按计划编排（start）

```text
start_task_pipeline(task_id)
  → 校验：存在 steps；每步可 resolve employee；项目/SSH/worktree 可启动
  → pipeline_active=true, pipeline_step_index=first pending
  → launch_step(task, step)
       resolve employee → build step prompt → start_*_with_manager(execution)
       step.status=running, phase=pipeline_waiting_step
       last_trigger_session_id=session
```

### 4.3 会话退出（关键分叉）

在 `handle_session_exit` / `handle_execution_exit` **最前**增加：

```text
if pipeline_active && phase in (pipeline_waiting_step, pipeline_launching_step):
    if stop requested → pipeline 转人工, pipeline_active=false
    if fail → step.failed, phase=pipeline_step_failed, 不启动 review
    if success:
        写 handoff_summary（可从 session 事件截取末段/固定提示要求模型输出）
        step.succeeded
        if has next step → launch next
        else → pipeline_active=false
              if automation enabled → launching_review（现网）
              else → idle + 任务状态与现网无质控成功对齐
else:
    现有 review_fix_loop 逻辑不变
```

**AC9 保障**：`pipeline_active==true` 且非最后一步时，禁止进入 `PHASE_LAUNCHING_REVIEW`。

质控修复轮的 `waiting_execution` 时 `pipeline_active` 必须为 false，避免与流水线混淆。

### 4.4 立即执行（不变）

`continueCreatedTaskRun` / 看板立即执行：

- 仍可生成 **Markdown 计划**（可同时写 steps 供以后编排，但**不**自动 start pipeline）  
- 仍单 assignee 一次 execution  
- 成功后若开质控 → 现网 `handle_execution_exit`（此时 `pipeline_active=false`）

### 4.5 恢复

扩展 `resume_pending_automation` / pending 查询：

- phase ∈ `pipeline_launching_step` | `pipeline_step_failed` → 重试当前步  
- phase ∈ `pipeline_waiting_step` + 未消费的 terminal session → replay `handle_session_exit`  
- 与现网 orphan running session 回收共用  

### 4.6 重试 / 转人工

| 动作 | 行为 |
|------|------|
| 重试当前步 | `pipeline_step_failed` → 再 `launch_step` 同 index |
| 转人工 | `pipeline_active=false`，phase=`manual_control`，停 live session（若有） |

MVP 不做跳过。

## 5. Prompt / 交接

**步骤执行 prompt 结构（示意）**

```text
任务标题/描述摘要
本步目标 / 成功标准
上一步交接摘要（第一步可无）
约束：完成后用简短中文总结「交接摘要」供下一步使用（实现可用事件截取或约定标签）
```

**不要**把完整多步 plan 无差别塞给每一步（可附「总步骤列表一行标题」作地图，但执行焦点在本步）。

协调员生成模板：新增/改 `coordinator_plan` 或增加 `coordinator_pipeline_plan` scene，要求 JSON schema 字段稳定；设置页重置默认模板需同步。

## 6. API（Tauri commands 草案）

| Command | 作用 |
|---------|------|
| `ai_generate_coordinator_task_plan` | 升级：返回/落库 steps |
| `list_task_pipeline_steps` | 详情展示 |
| `update_task_pipeline_step` | 改 employee_id / 标题等 |
| `start_task_pipeline` | 按计划编排入口 |
| `retry_task_pipeline_step` | 失败重试 |
| `abort_task_pipeline` | 转人工 |
| （可选）`get_task_pipeline_status` | 聚合 phase + steps |

均走 `lib.rs` 注册；活动日志中文 key 在 `src/lib/utils.ts`（或既有 activity map）注册。

## 7. Frontend

- 任务详情：步骤表（顺序、标题、员工下拉、状态、session 链接、错误）。
- 看板/详情操作：
  - **立即执行**（现网）
  - **按计划编排**（无 steps 时可「先生成结构化计划再编排」或报错提示先生成——推荐：无 steps 时自动走一次结构化生成再启动，失败则停）。
- 创建任务弹窗：不强制改「立即执行」；可选后续加「创建并编排」。
- 运行中 badge：复用/扩展 `taskBackgroundRunStore` 或 automation phase 展示。

## 8. Compatibility

| 场景 | 行为 |
|------|------|
| 本地 / SSH | 每步 `validate_runtime_working_dir` / remote 路径；OpenCode SSH 限制原样报错 |
| 多引擎 | 每步 `start_*_with_manager` 与质控 fix 启动分支对齐 |
| 旧任务无 steps | 仅可立即执行；编排按钮禁用或引导生成 |
| 归档守卫 | `is_task_automation_active_for_archival` 纳入 pipeline phases |

## 9. Trade-offs

| 选择 | 原因 | 代价 |
|------|------|------|
| 步骤表而非只 JSON | 可绑 session、可查询 | 多一次迁移与 CRUD |
| pipeline_active 而非新 automation_mode | 质控开关语义不脏 | 退出钩子分支变复杂，需单测钉死 |
| 共用任务 worktree | 复用 git 上下文 | 多员工可能互相干扰；靠串行 + 交接缓解 |
| 双入口 | 不破坏现网 | UI 稍多 |

## 10. Rollback

- 功能开关：无 steps / 不点编排则零行为变化。  
- 迁移向前兼容：新表空、新列默认 `pipeline_active=0`。  
- 若严重问题：可隐藏「按计划编排」按钮；数据可保留。  

## 11. Test Focus（设计级）

- 解析 steps 合法/非法  
- `pipeline_active` 中间步成功 **不** `launching_review`  
- 最后一步 + automation on → review  
- 最后一步 + automation off → 无 review  
- 立即执行 + automation on → 仍 review（回归）  
- resume pending pipeline  
- 指派回落 `assignee_id` / 拒绝启动  
