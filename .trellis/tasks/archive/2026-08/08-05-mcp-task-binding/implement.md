# Implement — MCP 任务级深度绑定

## Preconditions

- [x] `prd.md` 收敛（D1 任务级、D2 三态、Codex 必达）
- [x] `design.md` 完成
- [ ] 用户批准本规划摘要后 → `task.py start` → Phase 2
- [ ] 写代码前加载 `trellis-before-dev`（backend + frontend）

## Ordered checklist

### Step 1 — Schema + models

1. Migration **v43**：`tasks.mcp_server_ids TEXT`（可空）
2. `Task` / 查询 SQL / `create_task` 默认 `NULL`
3. 绑定 DTO：`TaskMcpBinding` / set payload（mode + server_ids）
4. 单元或 migration 测试：列存在

### Step 2 — Resolve + binding commands

1. `resolve_effective_mcp_servers` in `codex/mcp.rs`（或 `app` 可测模块）
2. `get_task_mcp_binding` / `set_task_mcp_binding`
3. `set`：校验 task 存在、id 合法、写库、`insert_activity_log`（无 env）
4. 单测：inherit / `[]` / subset / unknown id 拒绝
5. `lib.rs` 注册 command

### Step 3 — Codex 注入

1. `append_mcp_config_args(servers) -> Vec<String>`（含 `--ignore-user-config` + 各 `-c mcp_servers…`）
2. 接入 `build_session_exec_args` / remote builder
3. 本地 launch 路径在 spawn 前解析 task_id 并附加 args
4. 终端摘要行 `[MCP] …`（无密钥）
5. SDK：探测或 CLI 回退 / 明确提示（按 design 4.3）
6. 单测：args 快照（空集 / 单 server）

### Step 4 — Frontend

1. `backend.ts` + `types.ts`
2. 任务详情 MCP 区块（三态 UI）
3. `getActivityActionLabel`：`task_mcp_binding_updated` → 中文
4. 加载绑定与全局清单、保存反馈

### Step 5 — SSH / 非 Codex 收尾

1. 确认 remote command 带上 MCP args 与 escape
2. 非 Codex：调用同一 resolve；能注入则注入，否则一行中文日志即可
3. 文档注释：远程 command 在远端执行

### Step 6 — 验证门

```bash
cargo test --manifest-path src-tauri/Cargo.toml resolve_effective_mcp
cargo test --manifest-path src-tauri/Cargo.toml append_mcp
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
```

手工冒烟（`npm run tauri:dev`）：

1. 设置页启用 2 个示例 MCP  
2. 任务 A 继承 → 日志显示全局 enabled  
3. 任务 B override 仅 1 个 → 日志与行为一致  
4. 任务 C override 空集 → 日志「空集」  
5. 改绑定 → 仪表盘/活动中文  

## Validation commands (definition of done)

| 检查 | 命令 / 方法 |
|------|-------------|
| 单测 | `cargo test --manifest-path src-tauri/Cargo.toml`（相关 filter） |
| Clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` |
| 前端 | `npm run build` |
| AC1–AC5 | 手工会话 + 活动日志 |

## Risky files / rollback points

| 风险点 | 回滚 |
|--------|------|
| `command_builders.rs` / session launch 破坏启动 | 去掉 MCP args 附加；保留列与 UI |
| `--ignore-user-config` 副作用过大 | 改为 profile 覆盖策略（design 备选） |
| SELECT * 漏列 | 全库任务查询同步加列 |

## Review gates before `task.py start`

- [x] 产品决策 D1/D2 已写入 prd
- [x] design 含 migration、解析、注入、SSH、测试
- [x] implement 步骤可执行
- [x] implement.jsonl / check.jsonl 有真实 spec 条目
- [ ] **用户明确批准本最终规划摘要**

## Follow-up（非本任务）

- 员工级绑定（在任务三态之上叠加）
- 四引擎注入对等
- MCP 定义迁入 SQL / 备份
