# Implement: Grok Build Provider + SSH

## Order & Checklist

### 0. Before code

- [ ] `trellis-before-dev` / 读 `.trellis/spec/backend/ai-engines.md`、frontend type-safety
- [ ] 对照 Claude：`src-tauri/src/claude/**`、`src/lib/claude.ts` 作为复制基线

### 1. Frontend types & labels

- [ ] `src/lib/types.ts`
  - `AiProvider` 加 `"grok"`
  - `AI_PROVIDER_OPTIONS`、`GROK_MODEL_OPTIONS`、`GROK_EFFORT_OPTIONS`
  - `normalizeAiProvider` / `getDefaultModelForProvider` / `normalizeModelForProvider` / `normalizeReasoningEffortForProvider` / `getModelOptionsForProvider`
- [ ] 全量标签分支改为含 grok（搜索 `opencode` / `Claude` 三元）：
  - `CreateEmployeeDialog.tsx` / `EditEmployeeDialog.tsx` / `EmployeeCard.tsx` / `EmployeeRunningSessionsDialog.tsx`
  - `CreateTaskDialog.tsx` / `TaskOverviewPanel.tsx`
  - `SessionsPage.tsx` / `TaskSessionChainPanel.tsx`
  - `useTaskAiActions.ts` 等文案

### 2. Backend grok engine module

- [ ] 新建 `src-tauri/src/grok/`（manager / settings / process/*）
- [ ] CLI resolve：`grok` + `GROK_CLI_PATH` + `~/.grok/bin`
- [ ] `start_grok` / `stop_grok` / `stop_grok_session`
- [ ] local + SSH launch（复制 Claude context/SSH 路径，改命令为 grok）
- [ ] streaming-json 宽松解析 + session_id 提取
- [ ] events：`grok-stdout|stderr|exit|session`
- [ ] `lib.rs`：mod、State、invoke_handler 注册
- [ ] models：`GrokSettings` / `GrokHealthCheck`（及更新结构体若需要）

### 3. Wire session entry points

- [ ] `src/lib/grok.ts`：start/stop + listeners
- [ ] `CodexControls.tsx`、`useTaskExecutionActions.ts`、`SessionsPage.tsx`、`TaskSessionChainPanel.tsx`、`TaskDetailDialog.tsx`（若有）
- [ ] `task_automation.rs`：assignee.ai_provider == "grok"
- [ ] `app/review.rs` / employees runtime status：若按 provider 分支则补 grok
- [ ] `get_ai_provider_capabilities` 追加 grok

### 4. One-shot + AI commit + settings

- [ ] `codex/settings.rs`：`SUPPORTED_ONE_SHOT_PROVIDERS` + normalize model/effort/provider（含 remote）
- [ ] `codex/process/one_shot.rs`：local/ssh grok 分支 + helpers
- [ ] `RuntimeSettingsTab.tsx` / `GitAutomationSettingsTab.tsx` / `SettingsPage.tsx`：Grok 选项与模型/effort UI
- [ ] 本地 grok 健康展示；远程 `validate_remote_grok_health` + 前端调用（可挂 SSH/Runtime 区）

### 5. Tests & validation

- [ ] Rust 单测：normalize provider/model/effort；remote command 构造转义；stream 样例行解析
- [ ] 若有 runtime 测试夹具，扩展 grok 默认不破坏现有
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] `npm run build`
- [ ] 手工冒烟（见下）

## Validation Commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
# 手工：
# 1) 创建 Grok 员工 → 本地任务启动/停止
# 2) SSH 项目 Grok 启动（远端已 grok login）
# 3) 设置 one_shot=Grok → 优化提示/生成类 one-shot
# 4) AI commit custom provider=Grok
# 5) Claude SSH + Codex 本地回归
```

## Risky Files / Rollback Points

| 风险点 | 回滚 |
|--------|------|
| `one_shot.rs` 大 match | 保持未知 provider 回落 codex；grok 分支可整段删除 |
| `task_automation.rs` 启动 | 仅加 elif grok，失败时不影响其他分支 |
| 前端全量标签 | 漏改只影响展示；启动漏改会导致误走 Codex——用搜索验收 |
| `lib.rs` handler 列表 | 编译期可见 |

Rollback：不选 grok 即无路径；紧急可 `normalizeAiProvider` 忽略 grok + UI 隐藏选项。

## Review Gates

1. PRD 验收项可映射到代码路径  
2. SSH 与 local 均有启动路径  
3. one_shot 选 grok 不会静默变 codex  
4. 无新 session 表 / 无前端写库  
5. capabilities 与真实能力一致  

## Follow-ups (post-MVP, not this task)

- 动态 `grok models`
- `XAI_API_KEY` 注入
- ACP stdio
- 远程一键安装
