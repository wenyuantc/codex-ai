# Codex AI 功能与架构优化专项

## Goal

对 codex-ai 代码库中 9 项已确认的优化点做统一治理。9 项经代码实测确认（非推测），收敛为 7 个可独立验证的子任务；规划期衍生发现再新增 1 个子任务（OS keychain），合计 **8** 个子任务。

本任务为**父任务**：拥有源需求集合、子任务地图、执行顺序约束、跨子任务验收标准与最终集成评审。父任务自身**不承载直接实现工作**。

## Source Findings

以下 9 项均在 2026-07-29 的代码扫描中实测确认，附证据位置：

| # | 发现 | 证据 |
|---|---|---|
| 1 | SSH 无连接复用，每次操作重新握手认证 | `src-tauri/src/app/remote.rs:223` `build_ssh_command` 无 `ControlMaster`/`ControlPath`/`ControlPersist` |
| 2 | `codex_session_events` 只增不删 | 全仓库无 `DELETE FROM codex_session_events`、无保留策略、无 `VACUUM` |
| 3 | 四引擎零抽象，`pub trait` 数为 0 | 归一化引擎名后 diff：`manager.rs`=0 行、`process/lifecycle.rs`=0 行、`process/context.rs`=7 行 |
| 4 | 前端列表渲染无优化 | `TaskCard.tsx:404` 每卡片 1s `setInterval`；全仓库 `React.memo` 计数 = 0；无虚拟列表库 |
| 5 | 前端直读 SQL 且关键查询无分页 | 8 处 `import { select } from "@/lib/database"`；`taskStore.ts:112`、`dashboardStore.ts:356` 全表 `SELECT *` 无 LIMIT |
| 6 | 巨型文件 | `git_workflow.rs` 5515 行/153 函数；`task_automation.rs` 2976 行/46 函数 |
| 7 | SSH 密码经环境变量传子进程 | `remote.rs:282` `command.env("CODEX_SSH_SECRET", secret)`；**该 env 是必需项，非冗余**——`create_askpass_script` 第 219 行 `let _ = secret;` 显式丢弃入参，脚本内容 `printf '%s' "$CODEX_SSH_SECRET"` 只能从环境变量取值 |
| 8 | 引擎测试覆盖倾斜 | codex 48 例，claude/grok/opencode 各 3–11 例，而后三者恰是复制粘贴代码 |
| 9 | 无任何 lint | 无 ESLint、无 Clippy 配置、无 Prettier |

## Requirements

### 收敛决策

9 项按「共享改动面」收敛为 7 个子任务：

- **1 + 7 合并** → 二者都修改 `remote.rs::build_ssh_command` 同一函数，分开做必然冲突。
  - 注：第 7 项**不能靠简单删除 env 解决**（见上表证据），需替换传递通道（如 FIFO / stdin），或经评估后确认在单用户桌面应用威胁模型下风险可接受而降级处理。此判断在 C1 规划阶段定夺。
- **3 + 8 合并** → trait 抽象会重写引擎结构，测试须随之重写；分开做等于写两遍测试。
- 其余 5 项保持独立。

### 用户确认的约束

1. **Lint 存量告警：一次性清零**。所有 clippy / ESLint 告警修到零再挂 CI。
   - 风险已告知：改动面横跨全仓库，与子任务 C3、C6 冲突面大。
   - **缓解措施（强制）**：lint 子任务 **必须排在执行顺序最后**，对已完成重构后的最终代码清零，避免修完被重构推翻。
2. **会话事件保留：默认 30 天 + 设置项可调**。在 `SettingsPage` 暴露天数配置与「立即清理」入口。
3. **交付方式：每个子任务独立分支 + 单独提交**，逐个合回 `main`。

## 规划期衍生发现

以下为各子任务规划阶段新发现、**不在原 9 项之内**的问题：

| 来源 | 发现 | 严重性 | 状态 |
|---|---|---|---|
| C1 规划 | SSH 密码以**明文 JSON** 存于 `$APPCONFIG/ssh-secrets.json`（`codex/secret_store.rs`），非 OS keychain；且 `tighten_secret_store_permissions` 在非 Unix 平台是空操作（`#[cfg(not(unix))] let _ = path;`），**Windows 上无任何权限收紧** | 高于原发现 #7 | **已决策（2026-07-29）**：选项 A — 新增子任务 `07-29-ssh-secret-keychain`，排在 C1 之后；**不纳入 C1 范围** |

## 子任务地图

| 序 | 子任务 | 目录 | 覆盖 | 优先级 |
|---|---|---|---|---|
| 1 | SSH 连接复用与密钥传递收敛 | `07-29-ssh-multiplex-secret` | #1 #7 | P1 |
| 2 | SSH 密钥迁移至 OS keychain | `07-29-ssh-secret-keychain` | 衍生（明文落盘） | P1 |
| 3 | 会话事件保留与清理策略 | `07-29-session-events-retention` | #2 | P1 |
| 4 | 前端读路径下沉与查询分页 | `07-29-read-path-to-command` | #5 | P2 |
| 5 | 前端列表渲染性能优化 | `07-29-frontend-render-perf` | #4 | P2 |
| 6 | AI 引擎 trait 抽象与测试补齐 | `07-29-engine-trait-abstraction` | #3 #8 | P2 |
| 7 | 巨型文件模块化拆分 | `07-29-split-large-modules` | #6 | P3 |
| 8 | 引入 lint 工具链 | `07-29-lint-toolchain` | #9 | P3 |

## 执行顺序约束

父子结构不是依赖系统，以下顺序为**强制约束**，各子任务须在自己的 `prd.md` / `implement.md` 中复述其前置条件：

```
C1 SSH 复用/密钥传递
              ↓
C1b OS keychain  ← 【排在 C1 之后】避免与 secret 读取路径并发冲突；真正消除明文落盘
C2 保留策略 ─┐
C3 读路径 ───┼─ 与 C1b 彼此独立，可与 C1b 任意穿插（低风险、用户可感知收益）
              ↓
C4 前端渲染  ← 建议在 C3 之后：C3 会改动 taskStore 读路径，C4 依赖稳定的 store 接口
              ↓
C5 引擎抽象  ← 高风险重构，独立分支
              ↓
C6 文件拆分  ← 高风险重构，独立分支；与 C5 目标文件不重叠但同为大规模移动
              ↓
C7 Lint 清零 ← 【强制最后】对最终代码清零，否则 C5/C6 会推翻已修告警
```

顺序理由：
- C1 先落地连接复用（含 env 暴露窗口缓解）；C1b 紧随其后解决更高严重性的明文落盘问题。
- C2/C3 改动面小、风险低、收益直接可感知 → 可与 C1b 穿插落地建立信心。
- C4 排在 C3 之后，因为 C3 改 `taskStore` 读路径，C4 要基于稳定接口做 memo 与虚拟化。
- C5/C6 是纯内部重构，无用户可见收益但风险最高 → 放在验证充分之后，各自独立分支。
- C7 必须最后（见上文缓解措施）。

## Acceptance Criteria

### 跨子任务验收（父任务负责）

- [ ] 8 个子任务全部完成并各自合回 `main`，每个有独立分支与提交记录
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` 全绿，且测试总数 ≥ 246（现有基线，不允许净减少）
- [ ] `npm run build`（TypeScript 检查 + Vite 打包）通过
- [ ] 9 项源发现 + 1 项衍生发现（明文 keychain）逐条可追溯到某个已完成子任务，无遗漏
- [ ] `CLAUDE.md` 中因本次重构而失效的描述已更新（尤其：引擎 trait 数量、文件行数、`invoke_handler!` 命令数、前端直读 SQL 的说明）
- [ ] 最终集成评审：在全部子任务合并后，完整跑一次应用（`npm run tauri:dev`），确认本地项目与 SSH 远程项目两条路径均可正常创建会话并执行任务

### 不做的事（明确排除）

- 不引入新的 AI 引擎
- 不改变现有数据库表结构语义（`codex_sessions` 等历史表名保持不变）
- 不做 UI 视觉改版，前端改动仅限性能与数据流

## Notes

- 各子任务的技术设计与执行清单在**各自目录**的 `design.md` / `implement.md` 中，本文件不承载技术方案。
- 子任务须逐个规划→实现，不提前为全部 8 个写完设计——前序子任务的结果会影响后续（尤其 C5 的 trait 形态直接决定 C6 的拆分边界）。
- 会话开始时 git 快照显示的 21 个未提交改动已过期，实际工作区在本任务创建时是干净的。
