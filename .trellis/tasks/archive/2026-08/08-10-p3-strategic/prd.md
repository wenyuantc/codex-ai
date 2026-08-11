# P3 战略扩展三项

## Goal

在 `TASK.md` P3「战略扩展」剩余项上单独立项落地：**真·会话中 `send_input`**、**i18n 完整框架**、**更强报表**。外部 Issues / Jira / GitHub 双向同步仍属路线图级明确不做。

## Task Map

| 子任务 | 目录 | 交付 |
|--------|------|------|
| 真会话 send_input | `08-10-p3-send-input` | **四引擎同波**真实可写交互路径 + 能力矩阵诚实翻转 + UI 门控开放 |
| i18n 完整框架 | `08-10-p3-i18n` | 抽取式多语言基础设施 + 关键面落库 |
| 更强报表 | `08-10-p3-reports` | 在现有仪表盘报表之上增强（燃尽/趋势等）；不含 Issues 同步 |

父任务负责跨子任务验收与集成复核，不直接实现业务代码（除非收口缺口）。

## Confirmed Facts（仓库证据）

- 四引擎 `send_input` 能力矩阵均为 `false`；`send_codex_input` 恒返回「非交互批处理」错误；共享 spawn 路径大量 `Stdio::null()`。
- UI 已用 `can(provider, "send_input")` fail-closed；历史曾错误宣称 Codex 支持，已纠正。
- 无 i18n 框架：文案硬编码中文；无 `react-i18next` / locale 包。
- 报表已有：`get_dashboard_report_summary`（完成率、逾期/阻塞、近 7 日、近 8 周、aging、员工负载）+ JSON/CSV 导出；路线图曾提「燃尽/趋势」增强。
- Issues 同步：文档与 `TASK.md` 明确不做。

## Constraints

- 所有业务写走 Rust Tauri commands；前端不直写 SQLite。
- 新增能力兼容 Local + SSH。
- 不得在无真实交互路径时把矩阵标为 `true` 或暴露可点假入口。
- `format:check` + clippy `-D warnings` + 相关单测为硬门禁。
- P0–P2 未提交改动按 C1 先单独 commit，再作为 P3 实现基线。

## Out of Scope（父级）

- Jira / GitHub Issues 双向实时同步
- 微服务拆分、远程多端同步、第五引擎、完整 IDE
- 用「停止后重启 / 新开会话续聊」冒充真·`send_input`

## Key Decisions

| 决策 | 结论 | 日期 |
|------|------|------|
| `send_input` MVP 引擎范围 | **B · 四引擎同波**：Codex / Claude / Grok / OpenCode 本波均需评估并尽力实现真·mid-session 写入；不得用续聊/新会话冒充 | 2026-08-10 |
| `send_input` 不可行引擎验收 | **B1 · 尽力齐推 + 诚实豁免**：有可复核证据（文档/实测）证明无法 mid-session 写入时，该引擎矩阵保持 `false` + UI 禁用说明；父验收要求至少 **≥2 引擎真实 `true` 且必须含 Codex** | 2026-08-10 |
| 更强报表形态 | **R1 · 仪表盘增强包**：可配置日期范围 + 按里程碑燃尽/剩余 + 现有趋势可切换范围；不新开独立报表路由；不含 Issues | 2026-08-10 |
| i18n 范围 | **I2 · zh-CN + en · 本波尽量全量抽取**：用户可见字符串尽量进入文案表（含错误提示、活动 key 映射）；设置可切换并持久化 | 2026-08-10 |
| `send_input` UI | **U1 · 活终端内联输入条**：运行中任务日志 / Session 日志 / 员工运行终端的 `CodexTerminal` 下方共用输入组件；`can(send_input)` 且会话存活才可发送 | 2026-08-10 |
| 实施顺序 | **O1→调整**：`reports`（已完成）→ **`i18n`（进行中）** → `send_input`（暂缓）；用户 2026-08-10 确认先做 i18n | 2026-08-10 |
| 工作区基线 | **C1**：先单独 commit 现有 P0–P2 未提交改动，再开 P3 实现；P3 规划产物不混入该 commit | 2026-08-10 |

## Open Decisions（待用户）

（无 — 产品决策已收敛；待最终 planning summary 审批后，按 O1 对子任务逐个 `task.py start`）

## Acceptance Criteria（父级集成）

- [ ] 三子任务各自验收通过并可独立归档
- [ ] `TASK.md` P3：`send_input` / i18n / 更强报表勾选；Issues 仍明确不做
- [ ] 能力矩阵与 UI 对 `send_input` 诚实一致
- [ ] 文档（`docs/analysis` 相关段落）与实现一致
- [ ] 硬门禁通过

## Notes

- 同意建任务 ≠ 同意实现；须 PRD/design/implement 齐备并获 start review 后再 `task.py start` 具体子任务。
- CSV 导出已在上一波完成，不计入本父任务工作量。
- C1 已完成：`25d8c33` 收口 P0–P2；工作区仅剩本父/子规划产物未提交。
