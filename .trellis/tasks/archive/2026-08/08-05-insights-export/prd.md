# 报表洞察与导入导出

## Goal

补齐「做完了看不清趋势」与「数据迁不走」：仪表盘展示可过滤的吞吐/趋势洞察；提供任务域 JSON 导入导出作为 SQL 整库备份之外的业务迁移路径。

## Background（已确认）

- 仪表盘已有：统计卡（`get_dashboard_stats`）、任务分布、活动流、员工绩效。
- **已有半成品（本任务要收敛/补齐，而非重写）**：
  - `get_dashboard_report_summary`：完成率、逾期/阻塞/进行中、**近 7 日按日完成趋势**、员工负载列表；UI 在 `DashboardPage`「增强报表」卡片。
  - `export_tasks_csv`：扁平 CSV（id/title/status…），**无** description/标签/子任务/依赖，**无** JSON 导入。
  - SQL 整库：`backup_database` / `restore_database`（设置页路径，不在本任务范围）。
- **缺口**：
  - 报表未对齐 `selected_ssh_config_id` 作用域（stats 有，report/export 无）。
  - 无按周序列（PRD 要求日/周至少一种；日已有，周为增强）。
  - 进行中老化（可选）未做。
  - 无任务域 JSON 导出/导入往返；产品决策为 **仅 JSON**（非 CSV 为主路径）。
- 约束继承父任务：写库仅经 Rust command；activity 中文 label；local/SSH 双模式；时间展示 `formatDate()`。

## Requirements

### R1 洞察

- 仪表盘至少展示一种完成趋势序列（现有近 7 日柱状可保留并 hardening）。
- 支持按**当前项目**或**全局**过滤；过滤必须与环境模式一致，并与 stats 一样尊重 **SSH 主机**（`selected_ssh_config_id`）。
- 增强（纳入 MVP）：
  - `aging_in_progress`：进行中超过 N 天的任务数（默认 N=7，只读指标，不落库）。
  - 可选第二序列：近 8 周按周完成数（或与日序列切换；实现优先扩展同一 command 返回字段）。
- 数据全部经 Rust command 聚合，前端只展示。

### R2 导出（任务域 JSON）

- 导出范围：当前过滤下的任务（项目 / 环境模式 / SSH 主机），上限与 CSV 同级（如 5000）并在结果中标明截断。
- 格式：**JSON only** 为 UI 主路径；版本化 envelope（见 design）。
- 任务字段至少：`source_id`、title、status、priority、description、due_date、blocked_reason、completed_at、**标签名列表**、**子任务**（title/status/sort_order）、**依赖**（导出集内的 source_id 边，可选）。
- **不得包含**：员工实体、SSH 配置、密钥、附件二进制、assignee/reviewer/coordinator 外键、会话/自动化内部状态。
- 既有 `export_tasks_csv`：UI 不再作为主入口（可保留 command 以免破坏调用方，或实现阶段一并删除——优先 UI 换 JSON）。

### R3 导入

- 导入任务 JSON 到**指定目标项目**。
- 冲突策略（design 默认，见 Key Decisions）：
  - 默认 `create_new`：一律新 UUID，包内依赖按 source_id 重映射。
  - 可选 `skip_existing`：若 `source_id` 已在库中存在则跳过该条。
- 标签：按**目标项目内名称**匹配，不存在则创建。
- 校验错误可读（中文）；写 activity；不执行任意 SQL；附件 v1 忽略。

### R4 安全

- 导入路径禁止执行用户 SQL / shell；仅解析受控 JSON schema。
- 导出文件经人工打开不得暴露密钥字段（结构层就不写入）。

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| 导入导出范围 | 仅任务域 JSON | 2026-08-05 |
| 冲突默认策略 | `create_new`（安全优先，可往返烟测） | 2026-08-05 |
| CSV | 非 v1 主路径；UI 切到 JSON | 2026-08-05 |
| 老化指标 | MVP 纳入（默认 7 天进行中） | 2026-08-05 |
| 周趋势 | MVP 纳入（扩展 report DTO） | 2026-08-05 |

## Acceptance Criteria

- [ ] 仪表盘可见完成趋势（日和/或周）及进行中老化指标
- [ ] 报表过滤与项目/local·SSH 主机一致（与 stats 同范围）
- [ ] 任务 JSON 导出后再导入可还原关键字段（title/status/priority/description/tags/subtasks；依赖在包内可重映射）
- [ ] 导出文件不含员工/SSH 密钥字段
- [ ] 导入写 activity 且仪表盘中文 label 可见
- [ ] `npm run build` + `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 通过

## Out of Scope

- 项目/员工整包迁移（仍用 SQL 备份）
- CSV 作为产品承诺格式
- Jira/Linear OAuth 双向同步
- 完整 BI、自定义报表设计器
- 附件二进制迁移
- 导入时恢复 assignee 等员工外键

## Open Questions

（无阻塞项）
