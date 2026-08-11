# 产品缺口分析与下一波 backlog

## Goal

以产品经理视角，基于**代码实读**（非文档陈述、非命名推断）判断 Codex AI v0.5.6 还缺什么功能，产出可排期的下一波 backlog，并顺手校准已与代码矛盾的项目文档。

**本任务是分析型任务，不实现任何业务代码**（`src/` 与 `src-tauri/src/` 零改动）。

## Requirements

### R1 · 缺口分析必须有代码证据

每条结论都要能定位到 `文件:行号`，并可被独立复核。禁止根据命名或文档描述推断能力有无。

### R2 · 结论落到项目既有文档体系

- `docs/analysis/` 已有 00–07 编号体系与 delta 文档范式 → 续编为 08
- `TASK.md` 是 backlog 账本 → 新波次写入，保留历史勾选不动

### R3 · 校准文档与代码的矛盾

分析过程中发现的文档失真必须当场修掉，不能只记录不修——本项目供 AI agent 使用，文档撒谎的代价被放大。

### R4 · 不推翻既有路线图级决策

`TASK.md` 已明确不做的（微服务拆分、多端实时同步、第五引擎、完整 IDE、Issues 双向同步）保持不做。

## 核心结论

**主矛盾已从「能不能做」换成：AI 跑起来之后，用户看不见成本、管不住并发、复用不了经验。**

| 编号 | 缺口 | 关键证据 | 级别 |
|---|---|---|---|
| A1 | Token/成本零可见性 | `codex_sessions` 无任何 token 消耗列（`migrations.rs:229` + 8 处 ALTER）；唯一 usage 解析在 `grok/process/stream.rs:812`，只刷日志不落库；其余三引擎连解析都没有 | P0 |
| B1 | 无并发闸门/队列/调度 | 后端 grep `semaphore`/`max_concurrent`/`cron` 零命中，但产品支持员工多任务并发 | P0 |
| A2 | 会话日志不可复制/导出 | `CodexTerminal.tsx` 有虚拟化(:13)/过滤/清空/输入条，grep `clipboard`/`复制`/`导出` 零命中 | P1 |
| D1 | 无应用自动更新 | `Cargo.toml:17-30` 无 `tauri-plugin-updater`，但 CI 已打三平台包 | P1 |
| D2 | 文档与代码矛盾 | README 引擎矩阵 vs `app/database.rs:833/843`；四项计数失真；`app/session_events_retention.rs` 整模块未被记录 | P1 |
| B2 | 无任务模板 | `prompt_templates.rs` 是 AI 提示词模板，非任务模板；全库无 `task_template` | P2 |
| C1 | 审查无行级定位 | `app/review.rs` 只解析 verdict + 阻塞数 + 摘要；grep `line_comment` 零命中 | P2 |
| E1 | 主路径组件体量 | `TaskDetailDialog` 2014 行 / `TaskCard` 1929 行 | P3 |

**建议不做**：hunk 级暂存（AI 场景下整文件接受是主流）、token→金额换算（各引擎计费口径不同，会误导）。

## Acceptance Criteria

- [x] 完成缺口分析，每条结论附可复核的 `文件:行号` 证据
- [x] 新增 `docs/analysis/08-product-gap-2026-08-11.md`，风格对齐既有 07-delta
- [x] 更新 `docs/analysis/README.md`：阅读顺序加第 9 条、一句话结论改写、补计数校准提示
- [x] 校准 `README.md`：引擎能力矩阵（Claude/OpenCode 的 `send_input` 与四引擎的 `restart`）、表数 22→24、迁移数 80→44、补漏掉的两张表、Rust 测试数 339→366
- [x] 校准 `CLAUDE.md`：命令数 207→218 及全部分项、Rust 测试 339→366、前端测试 32 断言 4 文件→104 断言 9 文件、补记 `app/session_events_retention.rs`
- [x] `TASK.md` 写入下一波 backlog（N-P0～N-P3 + 明确不做），并修正已被 `2aac559` 推翻的 send_input 条目
- [x] `src/` 与 `src-tauri/src/` 零改动（可用 `git diff --stat` 验证）
- [x] `npm run format:check` 通过

## 验证方式

```bash
# 1. 确认无业务代码改动
git diff --stat -- src src-tauri

# 2. 复核计数（应为 218 / 24 / 44 / 366 / 104）
awk '/generate_handler!/,/^\s*\]\)/' src-tauri/src/lib.rs | grep -cE "^\s+[a-z_0-9:]+,\s*$"
grep -oE "CREATE TABLE (IF NOT EXISTS )?[a-z_]+" src-tauri/src/db/migrations.rs | awk '{print $NF}' | sort -u | wc -l
grep -cE "version: [0-9]+" src-tauri/src/db/migrations.rs
grep -rn "#\[test\]\|#\[tokio::test\]" --include='*.rs' src-tauri/src | wc -l
grep -rho "expect(" $(find src -name '*.test.ts') | wc -l

# 3. 能力矩阵与真源逐字段比对
grep -n "get_ai_provider_capabilities" -A 45 src-tauri/src/app/database.rs

# 4. 格式门禁
npm run format:check
```

## Notes

- 下一波的具体技术方案见 `docs/analysis/08-product-gap-2026-08-11.md` §4，**本任务不实现**。真正动手时应另开任务，届时补 `design.md` + `implement.md`。
- 分析过程中的意外发现：`app/session_events_retention.rs`（4 个命令，会话事件按天保留 + VACUUM + 统计）是一个完整功能模块，但 README 与 CLAUDE.md 都没记录——说明文档漂移不只是数字偏差，还包括「以为不存在的能力其实已经有了」。
