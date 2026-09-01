# Design · 协调员计划按意见修改

## Boundaries

- 扩展现有 `ai_generate_coordinator_task_plan` payload，不新增 IPC。
- 规划仍 one-shot + 只读工具；不注册 NativeAgentManager；不进 run-queue；不调 `handle_session_exit`。
- 无新表、无迁移、不新增 `session_kind`。

## Data flow

```
弹窗输入条
  → generateCoordinatorPlanForTask({ revisionInstruction, currentMarkdown })
  → ai_generate_coordinator_task_plan({ revision_instruction, current_markdown, request_id })
  → 修订 prompt（coordinator_plan_revise）+ 当前 steps
  → native: load 最近 coordinator transcript（可空）+ 只读 one-shot + save_transcript
    CLI: emit「未恢复原会话」+ 现网 one-shot
  → parse + save_task_plan_content + replace_task_pipeline_steps_from_plan
  → task_plan_revised + task_pipeline_plan_saved
  → 前端 refresh tasks + pipeline steps
```

## Contracts

### Payload

```
revision_instruction?: string | null
current_markdown?: string | null
```

- instruction trim 非空 → 修订模式
- markdown：payload 优先，否则 `tasks.plan_content`；仍空则中文错误
- steps：读 `task_pipeline_steps` 编进 prompt

### Prompt scene

`coordinator_plan_revise`：同一 JSON schema；未要求改的步骤尽量保持；只读工具；employee_id 必须来自可用员工。

### Native transcript

- `run_native_read_only_one_shot` 成功后 `save_transcript`
- 修订：查该任务最近一次成功 coordinator session 且有 transcript
- `AiCommandOptions.resume_session_id` 传入；找不到则注入兜底，不失败

### Frontend

- `CoordinatorPlanDialog`：`hasPlan` 时输入条 + `onRevise`
- 重新生成 `window.confirm`
- 生成与修订共用 `runExclusiveCoordinatorPlanGenerate`
- 新文案走 `tasks` i18n；活动 key 走 `activity.json`

## Compatibility

- 首次/重新生成不传 revision，路径不变
- 保存计划仍只写 Markdown；弹窗注明不会改工作包
- SSH：transcript 本机 DB；远端只读工具与生成相同
