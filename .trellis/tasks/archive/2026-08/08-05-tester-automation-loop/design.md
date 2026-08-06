# Design — 测试员自动化闭环

## 1. 目标流（先测后审）

```
执行会话成功结束
  → [若启用测试员自动化]
      launching_tester / waiting_tester
        → 命令失败或 AI 结论失败 → 回流执行（fix / in_progress）+ 通知
        → 通过 → 进入现有 launching_review / review_fix_loop
  → [若跳过测试员]
      直接现有 review 路径
```

手动「运行验收」走同一验证内核，不依赖 automation_mode 也可单次执行。

## 2. 扩展策略（不推翻 review_fix）

在 `task_automation` 中 **新增 phase**，而非替换 `review_fix_loop_v1`：

| 新 phase（建议名） | 含义 |
|--------------------|------|
| `launching_tester` | 正在启动验收（生成清单 / 起命令 / 起 AI） |
| `waiting_tester` | 等待命令或测试员会话结束 |
| `tester_failed` | 验收失败待回流 |
| `tester_launch_failed` | 启动失败 |

现有 `launching_review` … `completed` 保持语义。  
`session_exit`：执行类 session 结束时，若策略为先测后审且测试未通过，**不**直接 `start_review`。

协调员 pipeline：在实现步骤完成后、审核步骤前插入 tester 钩子（与 pipeline 优先级规则写进同一模块注释/单测）。

## 3. 数据模型（建议 migration）

### 3.1 项目/设置

- 全局 Codex/应用设置或独立 JSON：
  - `tester_automation_enabled: bool`
  - `tester_allow_ai_only: bool`（无测试命令时是否允许）
  - 默认测试命令（可选）
- 项目表或项目设置：
  - `test_command: Option<String>`（如 `npm test`）
  - 可选覆盖启用标志

### 3.2 任务验收结果

新表 `task_acceptance_runs`（名称可调）或任务列 + JSON：

- `id`, `task_id`, `status` (running/passed/failed)
- `acceptance_checklist` TEXT
- `command` TEXT NULL
- `command_exit_code` INT NULL
- `command_output_excerpt` TEXT
- `ai_verdict` TEXT NULL / JSON
- `summary` TEXT
- `created_at`, `finished_at`

任务上可冗余 `last_acceptance_status` 便于看板徽章（或 join 最新 run）。

### 3.3 验收清单编辑

清单正文存 run 或 `tasks.acceptance_checklist`；大文本前端 Monaco。

## 4. 验证内核

```
run_task_acceptance(task_id, trigger: auto|manual)
  1. resolve tester employee / skip policy
  2. ensure checklist (generate if empty & AI allowed)
  3. if test_command: spawn local or SSH command with timeout
     - nonzero → hard fail (ignore AI pass)
  4. optional AI verdict session (tester provider) with checklist + excerpt
  5. persist run + activity + notification
  6. on pass + auto: handoff to review_fix start_review
  7. on fail: set task in_progress/blocked per failure_strategy; optional start fix
```

命令执行：复用 `process_spawn` / `git_runtime` 远程 shell 模式；工作目录 = 任务 worktree 或项目 path；`validate_runtime_working_dir` 同类校验。

## 5. IPC / UI

| Command（建议） | 用途 |
|-----------------|------|
| `run_task_acceptance` | 手动/内部触发 |
| `get_task_acceptance_runs` | 历史 |
| `update_task_acceptance_checklist` | 编辑清单 |
| `update_project_test_command` / settings | 配置命令 |
| 扩展 automation state 查询 | 含 tester phase |

UI：

- 任务详情主区域：验收清单 + 最近结果 + 运行验收按钮
- 看板卡：徽章
- 设置：启用开关、allow AI only、默认命令

## 6. 活动日志 keys（示例）

- `task_tester_acceptance_started`
- `task_tester_acceptance_passed`
- `task_tester_acceptance_failed`
- `task_tester_command_failed`
- `task_tester_skipped`

均映射 `getActivityActionLabel` 中文。

## 7. 兼容与降级

| 场景 | 行为 |
|------|------|
| 无测试员 | 可配置 skip → 直接 review |
| 无命令 + allow_ai_only | 仅 AI |
| 无命令 + !allow_ai_only | skip 或 manual 提示 |
| SSH 命令失败 | 中文错误 + failed run |
| 用户手动拖到 completed | 不强制阻断（可选后续加强） |

## 8. 测试计划

- Rust：phase 转移表；hard fail 命令非 0；skip 无测试员；先测后审不提前 review
- 手工：local npm/cargo 命令；SSH echo 命令；生成清单 Monaco 编辑

## 9. 回滚

- 设置关闭 `tester_automation_enabled` 即回旧 review 路径
- 新表保留无害；phase 未知时当 idle/manual 处理
