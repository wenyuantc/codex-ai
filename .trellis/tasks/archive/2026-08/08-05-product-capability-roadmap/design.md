# Design — 产品能力补齐路线图（父任务）

## 1. 架构立场

本父任务是 **产品交付树**，不引入新运行时架构。所有子任务继续遵守：

```
React UI → Tauri IPC → Rust service → SQLite
(+ engine managers / git_runtime / SSH)
```

- 禁止前端直写 SQL
- 新持久化必须 migration
- 活动 action 双端同步中文 label
- local + SSH 一等公民

## 2. 子任务边界

| 子任务 | 主边界层 | 关键模块（预期） |
|--------|----------|------------------|
| tester-automation-loop | backend automation + UI 任务 | `task_automation/*`, `app/tasks`, `app/projects` settings, TaskDetail/Kanban |
| kanban-delivery-ux | frontend + delivery commands | `KanbanPage`, `app/delivery`, Archive UI |
| ux-trust-hardening | frontend 为主 | TaskDetailDialog, Notification/banner, SSH artifact flags |
| engine-capability-parity | engines + UI | `get_ai_provider_capabilities`, sessions UI, engine process |
| opencode-ssh-bridge | opencode + remote | `opencode/process`, remote spawn |
| insights-export | app commands + dashboard | `app/*` aggregate queries, DashboardPage |
| coordinator-pipeline-viz | UI + pipeline state | `task_automation/pipeline`, TaskDetail |
| mcp-task-binding | settings + session start | MCP settings, session launch env |
| frontend-test-net | tooling | Vitest, pure utils/stores |

## 3. 跨子任务契约

1. **自动化扩展点**：测试员 phase 插入在 review 之前；协调员 pipeline 在「实现完成」后接 tester 再 review。
2. **能力矩阵**：`get_ai_provider_capabilities` 为唯一 UI 门禁真源；engine-parity 与 opencode-ssh 更新后必须刷新该矩阵。
3. **SSH 可信**：artifact 受限标志可被 ux-trust、review、tester 共用（同一 notice 语义）。
4. **导入导出**：仅任务 JSON，不与 DB SQL 备份互相替代。
5. **前端测试网**：从 Sprint B 起，每个改动子任务至少补与本域相关的纯函数/过滤测（若适用）。

## 4. 风险

| 风险 | 缓解 |
|------|------|
| 自动化状态机复杂度爆炸 | 先测后审以 phase 扩展，不重写 review_fix；单测锁 phase 表 |
| 9 子任务并行冲突 | 串行 start 子任务；共享文件（TaskDetail、automation）由 implement 顺序约束 |
| 引擎补齐不可行 | 「诚实边界」允许 UI 禁用；不阻塞其它子任务 |
| OpenCode SSH 技术债 | 独立子任务；失败时明确错误，不拖死 tester/看板 |

## 5. 回滚

- 每子任务独立 commit（Conventional Commits）
- 功能开关优先：自动化测试、导入、MCP 绑定可用设置关闭
- migration 只增不改破坏性列；新列可空或默认关闭

## 6. 父任务不实现代码

父任务归档条件 = 子任务全部完成（或用户裁剪后的剩余集合完成）+ 可选 README/analysis 能力矩阵更新。
