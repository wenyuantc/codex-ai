# Implement — 引擎能力对齐与会话体验

## 检查清单（实现顺序）

1. [x] **能力矩阵诚实化**  
   - 更新 `src-tauri/src/app/database.rs::get_ai_provider_capabilities`  
   - Codex `send_input: false`；notes 标明非交互、restart/start/stop/resume 语义  
   - 四引擎 `restart: true`（与 step 2 同步落地）

2. [x] **Restart 补齐（后端）**  
   - 以 `restart_codex` 为模板：stop live processes → `start_*`  
   - 新增并注册：`restart_claude` / `restart_grok` / `restart_opencode`（参数对齐各自 start）  
   - 完成后矩阵四引擎 `restart: true` + 更新 notes

3. [x] **send_input 诚实处理**  
   - `send_codex_input` 返回稳定中文错误（非英文 stub 文案）  
   - 不新增其它引擎 send_input command  
   - 不向 UI 暴露输入框

4. [x] **前端 engine API**  
   - `src/lib/claude.ts` / `grok.ts` / `opencode.ts` / `codex.ts`：restart 包装  
   - 统一 `src/lib/aiEngine.ts`：`restartByProvider(provider, …)`

5. [x] **能力门禁 hook + 徽章**  
   - `useAiProviderCapabilities` + `src/lib/aiCapabilities.ts`（cache + `can`）  
   - 增强 `EngineCapabilityBadges`（tooltip/`notes`、compact）  
   - 当前 UI 无 restart 按钮入口；API + `can()` 已就绪供后续接入

6. [x] **设置页四引擎对照**  
   - 从 MCP 子页迁到 Runtime 区块  
   - 标题/文案改为四引擎；完整徽章列表  
   - 删除误导性「仅 Codex 完整支持」过时句

7. [x] **会话日志虚拟化**  
   - `CodexTerminal`：`@tanstack/react-virtual`，≥80 行启用  
   - 保持清空日志；stick-to-bottom MVP  
   - `SessionLogDialog` 高度 `h-[28rem]`

8. [x] **日志搜索/过滤**  
   - `SessionLogDialog`：关键字过滤 + 匹配计数  
   - 过滤结果走 `CodexTerminal lines=` 虚拟渲染

9. [x] **活动日志（若有）**  
   - 无新增 activity action（N/A）

10. [x] **测试与质量门**  
    - Rust：`capability_matrix_is_honest_and_complete` 通过  
    - clippy `-D warnings` 通过  
    - `npm run build`（tsc + vite）通过

11. [ ] **手工烟测**（待用户/本机 UI）  
    - 设置页：四引擎徽章/表与矩阵一致  
    - 有 CLI 时：Claude 或 Grok 启动 → 重启 → 停止  
    - 会话日志：加载接近 2000 行或 mock 千级 → 滚动流畅；关键字过滤可用  
    - 确认无 send_input 可点入口

## 验证命令

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml capabilities
cargo test --manifest-path src-tauri/Cargo.toml get_ai_provider
# 若测试名不同，按模块搜：
# cargo test --manifest-path src-tauri/Cargo.toml ai_provider
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## 高风险 / 高频改动文件

| 路径 | 原因 |
|------|------|
| `src-tauri/src/app/database.rs` | 能力矩阵真源 |
| `src-tauri/src/codex/process/mod.rs` | restart_codex / send_codex_input |
| `src-tauri/src/claude/process/mod.rs`（及 grok/opencode 对等） | 新增 restart |
| `src-tauri/src/lib.rs` | command 注册 |
| `src/components/codex/CodexTerminal.tsx` | 虚拟化 |
| `src/components/sessions/SessionLogDialog.tsx` | 过滤 UI |
| `src/components/ai/EngineCapabilityBadges.tsx` | 徽章 |
| `src/pages/SettingsPage.tsx` | 对照表位置/文案 |
| `src/lib/backend.ts` | 类型已存在，核对一致 |

少动：`task_automation/*`、看板、delivery（非本任务）。

## 回滚点

| 步骤后 | 回滚方式 |
|--------|----------|
| 仅矩阵诚实化 | 恢复布尔位（不推荐，会恢复虚假广告） |
| 新增 restart commands | 取消 `lib.rs` 注册 + 删 FE 包装；矩阵 `restart` 改回 false |
| 虚拟化 | 阈值调到极大或回退全量 map |
| 设置页搬迁 | 徽章移回原 TabsContent |

无 migration → 无需 DB 回滚。

## 实现约束（编码时）

- 加载 `trellis-before-dev`；读 backend `ai-engines` / frontend `data-access` + cross-layer guide
- 业务写只经 Rust commands；FE 不碰 SQL
- 时间展示走 `formatDate()`（本任务若新增时间列）
- 大文本仍用 Monaco（本任务日志用终端组件，不改 Monaco）
- SSH 路径不绕过 `validate_runtime_working_dir` / 既有 remote start

## 开始前

用户批准本规划后：

```bash
python3 ./.trellis/scripts/task.py start 08-05-engine-capability-parity
```

然后加载 `trellis-before-dev`，再按清单实现（Grok：可 `spawn_subagent` `trellis-implement`，首行 `Active task: .trellis/tasks/08-05-engine-capability-parity`）。

## 验收对照（PRD AC）

| AC | 对应步骤 |
|----|----------|
| 无能力操作不可点 + 中文说明 | 3, 5, 6 |
| 设置页四引擎对照 | 6 |
| 已补齐能力有测 | 2, 10（restart） |
| 长会话滚动无明显卡顿 | 7, 11 |
| build + clippy | 10 |
