# 产品能力补齐路线图（全量）

## Goal

在现有「任务 → AI 执行 → 审核/修复 → Git/SSH」闭环之上，**系统性补齐产品缺口**，使 Codex AI 从「功能齐全的桌面壳」升级为：

1. **角色闭环完整**（测试员真正进入自动化主链路）
2. **交付管理可用**（看板里程碑/标签/筛选/归档）
3. **多引擎体验可信**（能力对齐 + OpenCode SSH + 会话可读性）
4. **主路径清晰、产物可信**（详情 CTA、SSH 降级提示）
5. **可观测与可迁移**（报表 + 导入导出）
6. **编排可见、MCP 可用、前端可回归**（协调员流水线 / MCP 绑定 / 最小测试网）

本父任务只负责：范围地图、验收总表、交付顺序、跨子任务约束。**不直接实现代码**；实现落在 9 个独立可验收的子任务中。

## Background（已确认事实）

- 应用已具备：四引擎会话、看板状态机、review_fix_loop、协调员编排 v2、Git/worktree、SSH 一等、MCP 配置清单、DB SQL 备份、全局搜索、通知中心。
- 开放产品缺口（来自 `TASK.md` + 产品讨论 + 代码证据）：
  - 测试员仅有验收清单生成，无「任务完成后自动测 → 失败回流」
  - 看板交付 UX：筛选文案、归档可编辑等仍糙
  - 引擎能力不对称：restart / send_input 基本仅 Codex
  - OpenCode SDK bridge 远程明确未实现
  - 报表/导入导出/前端测试网/MCP 任务级绑定/流水线可视化缺失或偏浅
- 边界约束（全子任务继承）：
  - 前端禁止直写 SQL；业务写经 Rust Tauri commands
  - 新功能兼容 SSH；活动 `action` 需中文仪表盘映射
  - 大文本编辑/预览用 Monaco；时间展示走 `formatDate()`
  - 涉及库表必须有 migration

## Task Map（子任务）

| 序 | 子任务目录 | 交付物 | 建议阶段 |
|----|------------|--------|----------|
| 1 | `08-05-tester-automation-loop` | 测试员自动化闭环 | Sprint A |
| 2 | `08-05-kanban-delivery-ux` | 看板交付 UX + 标签/依赖/里程碑可发现 | Sprint B |
| 3 | `08-05-ux-trust-hardening` | 详情主路径 CTA + SSH 产物全局提示 | Sprint B/C |
| 4 | `08-05-engine-capability-parity` | 引擎能力对齐 + 能力徽章 + 会话日志体验 | Sprint C |
| 5 | `08-05-opencode-ssh-bridge` | OpenCode SSH 远程补齐 | Sprint C |
| 6 | `08-05-insights-export` | 报表洞察 + 任务导入导出 | Sprint D |
| 7 | `08-05-coordinator-pipeline-viz` | 协调员编排可视化 | Sprint D |
| 8 | `08-05-mcp-task-binding` | MCP 任务/员工级深度绑定 | Sprint D |
| 9 | `08-05-frontend-test-net` | 前端最小测试网（与实现穿插） | 并行穿插 |

推荐依赖关系（非硬阻塞，仅顺序建议）：

```
tester-automation-loop
  → kanban-delivery-ux ∥ ux-trust-hardening
  → engine-capability-parity ∥ opencode-ssh-bridge
  → insights-export ∥ coordinator-pipeline-viz ∥ mcp-task-binding
frontend-test-net：从 Sprint B 起随改动补测
```

## Parent Requirements

### R-P1 范围与治理

- 每个子任务独立 `prd.md` /（复杂）`design.md` / `implement.md`，独立验收与归档。
- 父任务归档条件：全部 9 子任务 `completed`，或用户书面裁剪后剩余子任务全部完成。
- 子任务不得破坏现有 local/SSH 双模式与四引擎会话落库约定。

### R-P2 跨子任务验收（集成）

- [x] 典型本地路径：创建任务 → 执行 →（可选自动质控）→ **测试员阶段** → 完成/提交，全程状态与活动日志中文可读。（`task_automation/acceptance.rs`，phase `launching_tester`/`waiting_tester`/`tester_failed`；开关默认关闭）
- [x] 典型 SSH 路径：同流程可在 SSH 项目上跑通，或明确降级提示且不静默失败。（四引擎均有 SSH 路径；降级由 `SshTrustBanner` + `SshArtifactLimitedNotice` 提示）
- [x] 看板可按里程碑/标签筛选，筛选与批量状态 UI 为中文。（`lib/kanbanFilters.ts` → `KanbanBoard`）
- [x] 非 Codex 引擎在 UI 上能力边界可见；无能力操作禁用并说明。（`get_ai_provider_capabilities` + `EngineCapabilityBadges`，含 `capability_matrix_is_honest_and_complete` 单测）
- [x] 仪表盘具备基础趋势/吞吐类洞察；任务可 JSON（或 CSV）导出/导入。（`get_dashboard_report_summary` 日/周序列 + `aging_in_progress`；`export_tasks_json` / `import_tasks_json`）
- [x] 前端至少有 store 过滤 + activity label 的最小自动化测试可跑。（Vitest 4 文件 32 断言，CI 硬门禁）

### R-P3 明确不做（全路线图级）

- 微服务拆分、远程多端实时同步
- 第五个 AI 引擎
- 完整 IDE 体验（内嵌调试器/全量 LSP）
- Jira/GitHub Issues 双向实时同步（导入导出若做，仅文件级，不含 OAuth 集成，除非子任务 PRD 另开）

## Acceptance Criteria（父任务）

- [x] 9 个子任务均已规划（含可测 AC），并按序或按用户批准顺序完成实现与归档（全部在 `.trellis/tasks/archive/2026-08/`）
- [x] 无子任务将业务写回退到前端 SQL（`src/lib/database.ts` hard-fail stub；capabilities 不授予 `sql:allow-select` / `sql:allow-execute`）
- [x] README 或 analysis 文档与最终能力矩阵一致（commit `e422a9f`：能力矩阵重写 + 架构/风险文档更正 + CLAUDE.md/README 计数校准）
- [x] 父任务 journal/notes 记录交付顺序与任何范围裁剪决策（见下方 Notes；无范围裁剪，9/9 全交付）

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| 测试员 MVP 形态 | 混合模式（测试命令 + AI 验收清单） | 2026-08-05 |
| 测试员流水线位置 | 先测后审 | 2026-08-05 |
| 导入导出范围 | 仅任务域 JSON | 2026-08-05 |
| 引擎对齐目标 | 诚实边界 + 尽力补齐 | 2026-08-05 |
| 交付顺序 | 见 Task Map；一次 start 一个子任务 | 2026-08-05 |

## Open Questions

（无阻塞项）

---

## Notes

- 用户 2026-08-05 明确：「说的全部要做」→ 本树覆盖讨论中的 P0–P2 全部条目。
- 复杂父任务产物：`prd.md` + `design.md` + `implement.md`；首个子任务 `tester-automation-loop` 已具备 design/implement。
- **实现批准**：需用户在本最终规划摘要之后另发明确批准，再 `task.py start 08-05-tester-automation-loop`（父任务不直接写产品代码）。

### 收尾结论（2026-08-07）

**实际交付顺序**（与 `implement.md` 建议顺序基本一致，无范围裁剪）：

| 序 | 子任务 | 归档位置 |
|----|--------|----------|
| 1 | tester-automation-loop | `archive/2026-08/` |
| 2 | kanban-delivery-ux | 同上 |
| 3 | ux-trust-hardening | 同上 |
| 4 | engine-capability-parity | 同上 |
| 5 | insights-export | 同上 |
| 6 | coordinator-pipeline-viz | 同上 |
| 7 | mcp-task-binding | 同上 |
| 8 | opencode-ssh-bridge | 同上 |
| 9 | frontend-test-net | 同上 |

**父级收尾门禁**：`cargo clippy -D warnings` 通过；`cargo test` 339 passed；`npm run test:ci` 32 passed；`npm run format:check` 通过。

**遗留（未纳入本路线图，建议单开任务）**：

1. `TaskCard` 1799 行 / `TaskDetailDialog` 1975 行——本轮多个子任务持续往这两个文件加功能，体量比路线图启动时更大。
2. 测试员自动化 `tester_automation_enabled` 默认 `false`，缺开箱引导。
3. SSH 下 review / 自动 commit 仍为 ⚠️，未与 local 等价。
4. 前端测试仅覆盖 store 纯函数层，无组件/e2e。
