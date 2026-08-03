# 协调员编排 v2：多员工流水线

## Goal

把「协调员」从一次性生成 Markdown 计划的辅助角色，升级为**任务级编排能力**：在现有员工 / 会话 / 自动质控模型上，把计划拆成可调度的工作包，按序派给（可不同引擎的）员工执行，并保留失败可恢复与人工介入。

**用户价值**：用户可以在看板任务上显式启动一条可观察的多步骤流水线（不同步骤可用不同 AI 员工/引擎），同时保留现网「立即执行 = 单员工整包一次会话」习惯；不引入自由调度四引擎的 Meta-Agent CLI。

## Key Decisions（全部已锁定）

| ID | 决策 | 选择 |
|----|------|------|
| D1 | 架构路径 | **方案 B**：扩展协调员 + `task_automation`；不新增 Meta-Agent CLI |
| D2 | MVP 边界 | **A**：结构化计划 + **串行**多员工派工 + 失败可恢复 + 衔接质控；**不含**测试员自动验收 |
| D3 | 工作包指派 | **A**：协调员建议 `employee_id`；用户可改；开跑解析：步骤员工 → 任务 `assignee_id` → 否则拒绝 |
| D4 | 质控衔接 | **A**：任务已开 `review_fix_loop_v1` 时，**全部编排步骤成功后**自动进入既有审核→修复→提交；未开则与现网无质控成功出口一致 |
| D5 | 入口语义 | **A**：**双入口**——「立即执行」保持现网单员工整包；新增「按计划编排」走串行工作包 |

## Background / Confirmed Facts

1. 四引擎：员工绑定单一 `ai_provider`；Codex 能力最全；Claude/OpenCode/Grok 支持 start/stop/resume。
2. 协调员 v1：`ai_generate_coordinator_task_plan` → Markdown → `tasks.plan_content`；`continueCreatedTaskRun` 把整份计划塞进**单个** assignee 会话。
3. `coordinator_plan` 模板当前要求纯 Markdown，非机读工作包。
4. `subtasks` / `ai_split_subtasks` 存在但不驱动自动派工。
5. 测试员仅生成验收评论；本期不做自动验收。
6. 自动质控唯一模式 `review_fix_loop_v1`；执行会话成功退出（`session_kind=execution`）且开启质控时，现网会**直接** `launching_review`（见 `handle_execution_exit`）。编排中途步骤**不得**误触发审核，仅最后一步成功后才可进入。
7. 会话退出钩子：各引擎进程结束 → `task_automation::handle_session_exit`；启动时有 `resume_pending_automation`。

## Requirements

### R1 — 结构化工作包计划

- 协调员生成须产出：
  - 人类可读 Markdown（可继续写入 `plan_content`）
  - **结构化工作包列表**（持久化，刷新详情仍可见）
- 工作包字段至少：`index`、`title`、`goal`、`employee_id`（可空）、`success_criteria`、串行顺序（MVP 无并行边）。
- 生成 one-shot 输入须含本项目员工池（id / 名称 / 角色 / ai_provider）。
- 用户可在任务详情修改任一步执行员工。
- 开跑解析执行者：合法的 `步骤.employee_id` → 否则任务 `assignee_id` → 仍无效则**拒绝启动**并中文报错。
- 生成/解析失败可重试，不得卡死不可恢复。

### R2 — 串行编排运行

- 独立入口 **「按计划编排」**（D5）；不改变「立即执行」现网语义。
- 同一时刻仅一个编排步骤在执行。
- 每步用解析后员工的 `ai_provider` 启动既有引擎 `execution` 会话；worktree / SSH / 工作目录校验复用现有路径。
- 每步 prompt：任务上下文 + 本步目标/成功标准 + **上一步交接摘要**（非整份大 plan 糊弄下一步）。
- 步骤成功 → 自动下一步；任一步失败/非 0 退出 → 暂停在可识别状态，支持**重试当前步**或**转人工**（MVP 可不做「跳过」）。
- 应用重启后可恢复未完成编排（对齐 pending resume 模式）。

### R3 — 可观察性

- 任务级 UI：步骤列表、当前步、状态、关联 session、执行员工。
- 关键节点活动日志；仪表盘 key **中文**映射。
- 编排相关状态变更需能推到前端（可复用或扩展 `task-automation-state-changed` 一类事件）。

### R4 — 与自动质控衔接（D4）

- 编排**全部步骤成功**后：
  - 若 `automation_mode == review_fix_loop_v1` → 进入既有审核→修复→提交（复用 `start_task_code_review_internal` 等，不新造质控）。
  - 若未开启 → 与现网「执行成功且无质控」一致。
- **中间步骤**成功的 execution 退出**不得**触发审核。
- 「立即执行」单会话路径 + 既有质控行为**回归不变**。
- 不新增第二套「编排后是否质控」开关。

### R5 — 兼容性与约束

- 本地 + SSH 主路径；OpenCode 等既有限制须中文错误，禁止静默跳过。
- 前端不直写 SQLite；业务写经 Tauri commands。
- 活动日志 + 中文 key；大文本编辑预览用 Monaco（若需编辑计划正文）。
- 最小改动：扩展 `task_automation` / 关联表，不平行再造生命周期内核。

## Acceptance Criteria

- [ ] AC1：协调员生成后持久化结构化工作包 + 可读计划；刷新详情仍可查看/改步骤执行人。
- [ ] AC2：「按计划编排」按序启动各步员工会话；同时仅一步执行中。
- [ ] AC3：步成功自动下一步；全成功且已开质控 → 自动审核链路；未开质控 → 现网无质控出口；有活动日志。
- [ ] AC4：步失败时编排暂停，支持重试或转人工；UI 有明确状态。
- [ ] AC5：重启后未完成编排可恢复（或明确需确认后继续且状态不丢）。
- [ ] AC6：「立即执行」与无编排的 `review_fix_loop_v1` 路径回归通过。
- [ ] AC7：SSH 上至少一种非受限引擎可作步骤执行者跑通；OpenCode 受限场景中文错误。
- [ ] AC8：新活动类型仪表盘显示中文。
- [ ] AC9：中间步骤成功**不会**误开审核会话（自动化开启时）。

## Out of Scope

- Meta-Agent / 第五 CLI、并行 fan-out、动态重规划
- 测试员自动验收门禁
- 四引擎 `send_input` / restart 对齐
- 跳过步骤（可选后续）、费用熔断、跨任务队列
- 通用多智能体平台化

## Risks

- 现网 `handle_execution_exit` 在质控开启时对**任意**成功 execution 会进审核；编排必须用 phase/标记区分「流水线中」vs「质控修复执行」vs「单次立即执行」。
- 多员工串行默认**共用任务 worktree**（MVP）有冲突风险；交接摘要要写清上步改了什么。
- 结构化输出脆弱 → 设计采用 JSON（或 fence）+ Markdown 双写 / 校验重试。
- 工作包 vs `subtasks` 双轨 → 设计选定主存，避免两套真相。

## Planning Status

- 状态：`planning`
- 产物：`prd.md`（已收敛）+ `design.md` + `implement.md`
- 下一步：用户批准本最终规划摘要后，方可 `task.py start` 进入实现
