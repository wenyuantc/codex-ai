# 06 · 技术债与路线图

> 用户确认方向：**C · 体验打磨**（看板性能、通知、搜索、文档）  
> 同时保留 P0 边界/正确性项，避免「只抛光不管漏水」。

## 1. 系统结论

Codex AI 0.4.0 已是功能密度很高的**本地 AI 研发协作桌面应用**：模块化单体方向正确，Git + 三引擎 + 自动质控 + SSH 形成差异化闭环。主要矛盾从「能不能做」转为「**可维护性、一致性与体验摩擦**」。

## 2. 优势

1. 业务闭环完整（任务→执行→审查→修复→提交）  
2. 三引擎 + 统一会话表  
3. SSH 一等公民  
4. 通知中心模型清晰  
5. Delivery（标签/依赖/里程碑）后端已就绪  
6. 活动日志中文映射较全  

## 3. 技术债分级

### P0 — 正确性 / 边界（先堵漏）

| ID | 项 | 证据 | 建议动作 |
|----|----|------|----------|
| P0-1 | 前端 SQL 写 activity | projectStore / GlobalSearchDialog | 改为后端 API；禁 execute |
| P0-2 | capabilities 过宽 | default.json sql:allow-execute | 收紧权限 |
| P0-3 | 自动化 resume 回归清单 | task_automation.rs | 手工+单测覆盖 phase 表 |
| P0-4 | ADR 与实现一致性 | ADR vs database.ts | 更新 ADR 或改代码 |

### P1 — 可维护 / 质量网

| ID | 项 | 建议 |
|----|----|------|
| P1-1 | 拆分 `git_workflow.rs` | 按能力簇分文件，行为不变 |
| P1-2 | 拆分 `task_automation.rs` | phase handler 模块化 |
| P1-3 | 契约测试 | commands 列表 CI；关键 DTO serde roundtrip |
| P1-4 | 补 Rust 测试 | git confirm、automation、soft-delete |
| P1-5 | 前端最小测试 | store 过滤 + activity label map |

### P2 — 体验打磨（**本方向主战场 C**）

| ID | 项 | 用户价值 | 切入点 |
|----|----|----------|--------|
| P2-1 | 看板性能与卡片信息架构 | 拖拽/右键流畅 | `TaskCard.tsx` 拆分、memo、虚拟列表评估 |
| P2-2 | 任务详情操作引导 | 降低「不知道点哪」 | `TaskDetailDialog` 主路径 CTA、Tab 默认策略 |
| P2-3 | 通知跳转与已读体验 | 点开即达、少打扰 | NotificationCenter + navigation + sticky 恢复文案 |
| P2-4 | 全局搜索体验 | 更快找到实体 | GlobalSearchDialog：结果分组、键盘、去掉前端写库 |
| P2-5 | 会话/日志可读性 | 长会话不卡 | SessionsPage 日志窗口虚拟化、引擎能力徽章 |
| P2-6 | SSH 受限产物全局提示 | 避免误判 diff | 统一 banner + settings 联动 |
| P2-7 | 标签/依赖/里程碑可发现性 | 已有能力被用上 | 看板筛选、任务 Overview 入口强化 |
| P2-8 | 文档刷新 | 降低认知偏差 | 修订 README/BUG/TASK 过时段；维护 analysis 索引 |

### P3 — 产品扩展（暂缓，除非战略需要）

| ID | 项 | 备注 |
|----|----|------|
| P3-1 | MCP 管理 | TASK 未决 |
| P3-2 | 增强报表（燃尽/趋势） | FUN/BUG 方向 |
| P3-3 | JSON/CSV/跨工具导入导出 | 备份已有 SQL |
| P3-4 | 角色产品化（测试员/协调员工作流深化） | 已有 AI 命令，缺编排 UX |
| P3-5 | 引擎能力对齐（Claude/OpenCode input/restart） | 成本高 |

## 4. 推荐迭代序列（体验向 4–6 周节奏示意）

### Sprint C1 · 体验基础设施（1 周）

- P0-1 / P0-2 边界收紧（小改动、高收益）  
- P2-4 搜索体验 + 去前端写库  
- P2-3 通知跳转冒烟与文案  
- P2-8 文档：README 过时段 + analysis 索引  

### Sprint C2 · 看板与任务详情（1–2 周）

- P2-1 TaskCard 拆分与性能  
- P2-2 详情主路径（运行/审核/自动化状态）单一数据源  
- P2-7 标签/依赖入口  

### Sprint C3 · 会话与 SSH 感知（1 周）

- P2-5 日志性能  
- P2-6 SSH limited 全局提示  
- 引擎能力徽章（无能力则禁用并说明）  

### Sprint C4 · 质量加固穿插（并行）

- P1-3 契约 CI  
- P0-3 automation 回归用例  
- 不在 C 主线上强推大拆分，但为 C5 预留  

### 可选 Sprint C5 · 可维护性

- P1-1 / P1-2 大文件拆分（纯 refactor，配回归）  

## 5. 明确不做（本阶段）

- 微服务拆分  
- 远程多端同步  
- 为拆而拆的引擎重写  
- 未论证的 MCP 大功能（除非单独立项）  

## 6. 成功度量（体验 C）

| 指标 | 目标 |
|------|------|
| 看板交互 | 主路径操作 ≤3 次点击可达；无明显卡顿抱怨 |
| 搜索 | 常用实体 2 秒内定位；导航写日志不失败 |
| 通知 | sticky 可理解、跳转正确率高 |
| 回归 | P0 边界违规清零；automation 手工清单通过 |
| 文档 | BUG/TASK 与代码一致；analysis 00–06 可被新人 1 小时建立心智模型 |

## 7. 分析交付索引

| 文档 | 内容 |
|------|------|
| [00-architecture-overview.md](./00-architecture-overview.md) | 分层、边界、模块图 |
| [01-domain-capability-matrix.md](./01-domain-capability-matrix.md) | 能力 × Local/SSH × 引擎 |
| [02-data-model-audit.md](./02-data-model-audit.md) | 40 迁移、ER、备份风险 |
| [03-ipc-command-catalog.md](./03-ipc-command-catalog.md) | 149 commands 分类 |
| [04-runtime-lifecycle.md](./04-runtime-lifecycle.md) | 引擎 + 自动化生命周期 |
| [05-quality-risks.md](./05-quality-risks.md) | TOP10 风险、测试、安全 |
| [06-tech-debt-roadmap.md](./06-tech-debt-roadmap.md) | 本文件：P0–P3 + C 路线 |

## 8. 未决 / UNVERIFIED

1. AI 辅助命令在 Claude/OpenCode 员工上的实际 provider 路由细节  
2. 密钥是否进入 backup 文件  
3. OpenCode 是否完整接入 automation 全 phase  
4. 前端 `SELECT *` 是否在部分页面漏过滤 `deleted_at`（抽样已见多数带条件，需全量扫）  

---

**下一步**：若继续落地，建议从 **Sprint C1（P0 边界 + 搜索/通知/文档）** 开工。  
