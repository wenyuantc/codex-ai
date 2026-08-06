# Design — 引擎能力对齐与会话体验

## 1. 目标与原则

让用户 **看清** 四引擎真实能力，并在 CLI/SDK 允许范围内 **尽力补齐**；不可补齐处用统一能力矩阵 + 禁用说明，禁止「点了没反应」。

原则：

1. **诚实边界优先**：能力矩阵不得声称已支持但实际总是失败的能力。
2. **后端真源**：`get_ai_provider_capabilities` 为唯一 UI 门禁真源；前端只缓存/展示/门禁，不硬编码第二套布尔表。
3. **共享内核优先**：restart 类能力若可复用 stop→start，按引擎各自 command 暴露，不引入 `dyn AiEngine` 大一统分发。
4. **local + SSH 一等**：restart 路径必须沿用各引擎既有 start 的 SSH/工作目录解析，不写 local-only 捷径。

## 2. 现状结论（代码证据）

| 能力 | Codex | Claude | Grok | OpenCode | 备注 |
|------|-------|--------|------|----------|------|
| start / stop / resume | ✅ | ✅ | ✅ | ✅ | 前端 `taskRunSession` / Sessions 续聊已分流 |
| restart command | ✅ `restart_codex` = stop live + `start_codex` | ❌ 无独立 command | ❌ | ❌ | 模式可复制 |
| send_input command | ⚠️ `send_codex_input` **始终失败**（non-interactive） | ❌ | ❌ | ❌ | 矩阵写 `send_input: true` 不诚实 |
| 能力 UI | `EngineCapabilityBadges` 挂在设置 **MCP** 子页，文案写「三引擎」 | 部分 | | | 无运行态按钮门禁 helper |
| 会话日志 | `get_codex_session_log_lines` LIMIT **2000**；`CodexTerminal` **全量 map DOM**，无虚拟化/搜索 | | | | `@tanstack/react-virtual` 已依赖（看板在用） |

前端 **未调用** `restartCodex` / `sendCodexInput`；假操作风险主要来自：矩阵广告与真实行为不一致、设置页位置/文案误导、未来或隐蔽入口未按矩阵门禁。

## 3. 能力对齐决策

### 3.1 `restart` — 补齐（四引擎）

**定义（产品语义）**：停止该员工当前相关 live 会话后，以相同 prompt/参数重新启动一次执行会话（与现 `restart_codex` 一致：不 resume 旧 CLI session，而是新 start）。

实现策略：

- Codex：保留 `restart_codex`；修正 notes。
- Claude / Grok / OpenCode：各增 `restart_*` command（或共享内部 helper + 薄 command）：
  1. 解析 employee 的 live managed processes
  2. 按 session 调用既有 `stop_*_session` / stop managed process
  3. 调用既有 `start_*`（参数与对应 start 对齐：model / working_dir / task_id / image_paths 等）
- 前端：`restartClaude` / `restartGrok` / `restartOpenCode`（或统一 `restartEngine(provider, …)` 薄封装）
- **矩阵**：四引擎 `restart: true`（实现落地后）

兼容：

- SSH：走各引擎 start 已有 remote path；不在 restart 层另写 SSH。
- 无 live 进程：允许直接 start（与「重启」语义等价于重新开跑）；或返回中文错误「当前无运行会话，请使用启动」——推荐 **无 live 时直接 start**，与用户预期「再跑一遍」一致。文档写进 notes。

### 3.2 `send_input` — 不伪造

**结论**：当前四引擎会话均为 **非交互批处理**（prompt 启动时经 stdin 一次写入后关闭，或 CLI 参数；stdio 不保留可写会话 stdin）。

| 动作 | 决策 |
|------|------|
| 矩阵 `send_input` | **四引擎均为 `false`**（含 Codex：从 `true` 纠正） |
| `send_codex_input` command | 保留但返回稳定中文错误；或标记 deprecated 仍注册以免破坏 invoke。不向用户暴露可点按钮。 |
| UI | 无「会话中发送输入」入口；徽章显示「输入（不支持）」+ notes 说明「非交互模式」 |
| 未来 | 若某引擎支持真正 interactive session，再改矩阵 + 实现，禁止先亮灯 |

### 3.3 能力矩阵结构

保持现有字段，不扩表、无 migration：

```text
AiProviderCapabilities {
  provider, label,
  start, stop, restart, send_input, resume,
  notes  // 中文，给 tooltip / 设置页
}
```

可选增强（实现时二选一，优先 A）：

- **A（最小）**：继续硬编码 `vec![...]`，但 notes/布尔与真实 command 一致；加 Rust 单测锁矩阵快照。
- **B（可选）**：内部 `const CAPS: [...]` + 测试断言 command 注册名存在；仍不查运行时 CLI 版本。

不引入 DB 持久化能力位。

## 4. 前端能力门禁

### 4.1 数据获取

- 复用 `getAiProviderCapabilities()`。
- 新增轻量 hook 或模块级 cache，例如 `useAiProviderCapabilities()`：
  - 一次加载、会话内复用
  - `getCap(provider)` / `can(provider, 'restart' | 'send_input' | …)`
  - 失败时 fail-closed：未知能力视为不支持（按钮禁用 + 「能力信息加载失败」）

### 4.2 展示组件

| 组件 | 职责 |
|------|------|
| `EngineCapabilityBadges` | 单引擎或列表徽章；强化 `title`/tooltip 用 `notes`；compact 模式用于卡片 |
| 设置页 **对照表** | 从 MCP 子页迁出/复制到更可见位置（建议 Runtime 或独立「AI 引擎」区块）；标题改为 **四引擎**；表格列 = 启动/停止/重启/输入/续聊 |
| 运行入口 | 若存在或新增 restart 按钮：按矩阵 `disabled` + 中文 `title`/`Tooltip`；无能力不 silent no-op |

入口盘点（实现时 `rg` 确认后逐一门禁）：

- `CodexControls` / `EmployeeCard`（若加重启）
- `SessionsPage` / `TaskSessionChainPanel` 停止/续聊（续聊已有 resume_status；停止按 provider 已分流）
- 设置页能力文案

不强制本任务给所有启动路径加徽章；**最低要求**：设置页完整对照 + 任何 restart/send_input 控件必须门禁。

### 4.3 文案

- 禁用原因示例：`当前引擎不支持会话中输入（非交互模式）`
- 重启支持：`停止当前运行后重新启动任务（非恢复旧 CLI 会话）`
- 活动日志：若新增 restart 成功/失败 activity，action key 需 `getActivityActionLabel` 中文映射；**仅在实际写入 activity 时加**。

## 5. 会话日志体验

### 5.1 渲染虚拟化

- 改造 `CodexTerminal`：当行数 ≥ 阈值（建议 80–100，对齐 `KanbanColumn` 模式）使用 `@tanstack/react-virtual`。
- 行高：等宽字体 `text-xs` + `whitespace-pre-wrap` → 采用 **动态 measure** 或保守固定估算 + overscan；优先动态 measure 避免截断。
- 自动滚底：虚拟列表 `scrollToIndex(last)`；用户上滚时暂停 stick-to-bottom（可选，MVP 可始终 stick）。
- `SessionLogDialog` 高度已放大（28rem）：虚拟化主要收益在 dialog；员工卡片内 h-36 仍受益。

### 5.2 搜索 / 过滤

- 在 `SessionLogDialog`（非整页终端小窗）增加：
  - 关键字过滤（客户端 filter `line.includes`）
  - 可选 event 类型过滤若 UI 能拿到 type（`CodexSessionLogLine` 若仅有 line，则仅关键字）
- 清空过滤 / 匹配计数展示
- 过滤后仍走虚拟列表

### 5.3 后端窗口

- 保持 `LIMIT 2000` 除非手工标准不达标；若需扩展，可参数化 `limit` 并文档化，**非本任务必须**。
- 不重写 stream 协议；不改事件落库 schema。

## 6. 数据流

```text
UI 按钮(restart?)
  → can(provider, 'restart')? 否 → disabled + tooltip
  → invoke restart_* 
  → stop managed processes (local/SSH 同既有 stop)
  → start_* (validate_runtime_working_dir / remote 同 start)
  → events → employeeStore sessionLogs
  → CodexTerminal 虚拟列表渲染

UI 设置页
  → get_ai_provider_capabilities
  → 表格 + EngineCapabilityBadges
```

无新 SQLite 表；无 migration。

## 7. 与相邻子任务边界

| 子任务 | 边界 |
|--------|------|
| `opencode-ssh-bridge` | SSH 启动/产物深度归该任务；本任务 restart 仅复用 start，不修 OpenCode 远程缺口 |
| `ux-trust-hardening` | 全局 SSH 提示不归本任务；本任务只保证能力/禁用说明 |
| 父任务 | 矩阵更新后父验收「非 Codex 边界可见」由本任务交付 |

## 8. 风险与回滚

| 风险 | 缓解 |
|------|------|
| restart 参数不齐导致行为与 start 不一致 | restart 直接转发 start 同参；单测/烟测按引擎至少 1 条 stop+start 路径 |
| 矩阵纠正 Codex `send_input:false` 被认为「降级」 | notes 写明「历来为非交互；UI 未暴露」；诚实优先 |
| 虚拟列表动态行高抖动 | overscan + measure；必要时固定 min height |
| 与并行 worktree 冲突 | 本分支 `feat/engine-capability-parity`；少动 automation/看板核心 |

回滚：

- 删/关新 restart commands 注册；矩阵回退
- `CodexTerminal` 可 feature-flag 或行数阈值回退全量渲染
- 无 migration，回滚零 schema 成本

## 9. 测试策略

### Rust

- `get_ai_provider_capabilities` 快照：四 provider 存在；`send_input` 全 false；实现后 `restart` 全 true；notes 非空。
- restart 内部：对 manager 的「无进程 → start」/「有进程 → stop 再 start」可用现有 test harness 或纯函数拆分测（避免真 CLI）。

### 前端

- 纯函数：`can(provider, capability)` / filter log lines（若抽出）— 若 `frontend-test-net` 已就绪则补测；否则至少 `npm run build`。
- 手工：设置页对照表；千级日志滚动；Claude/Grok/OpenCode 重启一次（有 CLI 环境时）。

### 质量门

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml capabilities
# 或相关模块测试名
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## 10. 非目标

- 重写 stream 协议 / 统一 `dyn AiEngine`
- 真正的 interactive PTY 会话中输入
- 保证四引擎 100% 行为一致
- OpenCode SSH 远程补齐（独立子任务）
- 会话事件表 schema 大改
