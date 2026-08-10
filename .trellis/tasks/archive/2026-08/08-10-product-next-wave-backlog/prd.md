# 产品下一波能力与体验规划

## Goal

在 `08-05-product-capability-roadmap`（9 子任务已归档）之上，把下一波真实 backlog 写入仓库根目录 `TASK.md`，使后续实现可按 P0→P3 直接拆子任务。本任务**只交付规划文档**，不实现产品功能代码。

## Background

- 主闭环已通：任务 → 四引擎执行 → 审核/修复 → Git/worktree → SSH → 测试员自动化（默认关）→ 协调员流水线。
- 下一波矛盾：能力有了但发现性不足、部分语义不完整（依赖/备份/SSH 产物）、主路径组件过重、质量网偏薄。
- 范围确认（用户 2026-08-10）：建 Trellis 任务跟踪；清单覆盖 **产品 + 体验 + 可维护性** 均衡（非整包 `send_input` 等全量战略项优先）。

## Requirements

### R1 · 更新 TASK.md

- 保留历史「已完成的」与任务列表中已勾选 `[x]` 账本，不删历史。
- 用 P0–P3 结构化待办替换空的开放项；文风对齐现有中文可勾选条目。
- 「进行中」记录本规划交付本身。
- 「暂时不处理」保留原意图并做合理提升/标注：
  - 同员工多任务 CTA 文案 → 提升到 P0
  - 看板继续对话 → 能力已有入口，改为 P1「链路复核」
  - 离线员工禁跑 → 仍暂缓，标注需复核前后端是否一致硬拦截

### R2 · 优先级内容（写入 TASK 的契约）

- **P0 主路径可用**：首次引导、测试员自动化可发现、依赖真正拦截、多任务 CTA、仪表盘报表错误可见
- **P1 可信与交付**：备份语义补全、SSH review/commit 对齐或禁用、休眠 API 补 UI、Claude SSH 声明或补齐、角色说明、续聊复核
- **P2 体验与可维护**：拆分 TaskCard/TaskDetailDialog、看板性能、空态 CTA、前端测试加深、权限面/文档漂移
- **P3 战略扩展**：真·`send_input`、CSV UI、i18n/a11y、更强报表、外部 Issues 同步

### R3 · 明确不做（路线图级备注）

微服务拆分、远程多端实时同步、第五引擎、完整 IDE、Jira/GitHub Issues 双向实时同步。

### R4 · 本任务边界

- 不实现 backlog 内功能；不 bump 版本；不批量重写 `docs/analysis/*`。
- 功能实现须另开子任务，并按序一次 `task.py start` 一个。

## Acceptance Criteria

- [x] Trellis 任务 `08-10-product-next-wave-backlog` 存在，且本 `prd.md` 与 `TASK.md` 优先级一致
- [x] `TASK.md` 无空开放项；含 P0–P3 可拆实现的条目
- [x] 历史已完成账本保留；暂时不处理三项已提升或标注
- [x] 本回合无产品业务代码改动（仅 `TASK.md` + 本 Trellis 规划产物）

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| 清单范围 | 产品 + 体验 + 可维护性均衡 | 2026-08-10 |
| 同员工多任务 CTA | 从暂缓提升到 P0 | 2026-08-10 |
| 看板继续对话 | 能力已落地，P1 复核完整性 | 2026-08-10 |
| 离线禁跑 | 暂缓；需复核前后端硬拦截 | 2026-08-10 |

## Notes

- 上游归档：`.trellis/tasks/archive/2026-08/08-05-product-capability-roadmap/`
- 证据来源：`docs/analysis/01-domain-capability-matrix.md` §7、`TASK.md` 暂缓项、backend 未挂 UI 的命令面
