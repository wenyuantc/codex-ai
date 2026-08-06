# Design — MCP 任务级深度绑定

## 1. 边界与职责

| 层 | 职责 | 非职责 |
|----|------|--------|
| 设置页 / `mcp-servers.json` | Server **定义** + 全局 `enabled` | 不存 per-task 绑定 |
| SQLite `tasks` | 任务绑定三态 | 不存 command/env 副本 |
| `codex/mcp.rs` | 加载定义、解析有效集、导出、绑定 CRUD 辅助 | 不直接 spawn 进程 |
| Codex session launch | 将有效集注入 **本会话** 进程 | 不持久改 `~/.codex/config.toml` |
| 其它引擎 | 复用解析；注入 best-effort | 不阻塞 Codex 验收 |

数据流：

```text
Settings UI → update_mcp_servers → mcp-servers.json
Task UI → set_task_mcp_binding → tasks.mcp_server_ids
Session start(task_id)
  → load document + read task.mcp_server_ids
  → resolve_effective_mcp_servers
  → materialize Codex -c / profile args
  → spawn CLI or SDK (fallback rules below)
```

## 2. 数据模型

### 2.1 Migration（version = latest+1，当前基线 42 → **43**）

```sql
ALTER TABLE tasks ADD COLUMN mcp_server_ids TEXT;
-- NULL  = 继承全局 enabled
-- '[]'  = 显式空集
-- '["uuid",…]' = 指定 id 列表（JSON array of strings）
```

### 2.2 模型字段

- `Task.mcp_server_ids: Option<String>`（原始 JSON 文本，与 SQLite 一致）  
  或反序列化为 `Option<Vec<String>>` 时用自定义：  
  - 列 NULL → `None`（继承）  
  - 列 `'[]'` → `Some(vec![])`  
  - 列有 id → `Some(ids)`  
- 前端 `Task` 类型同步；更新用 **显式可空**（`deserialize_explicit_nullable` / `Option<Option<Vec<String>>>`）以免「省略字段」与「清回继承」混淆。

推荐专用 command（比塞进巨大 `UpdateTask` 更清晰）：

| Command | 入参 | 出参 |
|---------|------|------|
| `get_task_mcp_binding` | `task_id` | `{ mode: "inherit"\|"override", server_ids: string[], effective: McpServerConfig[] }` |
| `set_task_mcp_binding` | `task_id`, `mode`, `server_ids?` | 同上 + 写库 + activity |

`mode=inherit` → 写 `NULL`；`mode=override` → 写 JSON 数组（可空数组）。

校验：override 时 id 必须在当前全局清单中存在（未知 id 拒绝或过滤；**推荐拒绝整次保存并中文报错**，避免静默丢绑定）。

## 3. 解析契约

```text
fn resolve_effective_mcp_servers(app, task_id: Option<&str>) -> Result<ResolvedMcp, String>

ResolvedMcp {
  mode: inherit | override,
  server_ids: Vec<String>,      // 解析后引用 id
  servers: Vec<McpServerConfig>, // 完整定义，用于注入
}
```

算法：

1. `doc = load mcp-servers.json`
2. 若无 task 或 `mcp_server_ids IS NULL` → `servers = doc.servers.filter(|s| s.enabled)`，mode=inherit
3. 若 `mcp_server_ids` 解析为数组 → 按 id 顺序从 doc 取定义（**不要求** server.enabled，覆盖优先于全局开关），缺 id 跳过并记 warn；mode=override
4. 返回 servers（可为空）

单元测试覆盖：inherit / empty override / subset / unknown id / 无 task_id。

## 4. Codex 注入设计

### 4.1 已验证 CLI 能力

- `-c key=value`：TOML 覆盖，支持 nested dotted path  
- 用户配置使用 `[mcp_servers.<name>]` + `command` / `args` / `env`  
- `--ignore-user-config`：不加载 user `config.toml`（auth 仍用 `CODEX_HOME`）  
- 现有 `build_session_exec_args` 已使用 `-c model_reasoning_effort=...`

### 4.2 推荐策略（本地 CLI）

目标：**本会话 MCP 集 = 解析结果**，不被 `~/.codex` 里其它 MCP 污染（否则 AC3 空集无法保证）。

实现步骤：

1. `ResolvedMcp.servers` → 生成一系列 `-c` 参数：  
   - 对每个 server，name 规范化为合法 TOML key（优先用稳定 `id` 的 slug，或 name 净化）  
   - `-c mcp_servers.<key>.command="..."`  
   - `-c 'mcp_servers.<key>.args=[...]'`  
   - env 逐项 `-c mcp_servers.<key>.env.KEY="..."`（注意 shell/arg 转义；本地 `Command` 直接传 argv 不经 shell）
2. **替换语义**：在注入前加 `--ignore-user-config`，避免用户 config 中已有 MCP 合并进来。  
   - 副作用：用户 config 里的其它默认项不加载 → 可接受，因为应用已显式传 model / reasoning_effort / cwd。  
   - 若后续发现缺关键默认，再按需补 `-c`。
3. 将上述 args 并入 `build_session_exec_args`（或包装函数 `append_mcp_config_args(args, &servers)`），**本地 + 远程 CLI** 共用生成器。
4. 会话启动日志打一行摘要：`[MCP] 继承全局 · 2 个服务器：…` / `[MCP] 任务覆盖 · 空集`（**不打印 env 值**）。

### 4.3 SDK 路径

`sdk_bridge.mjs` 的 `threadOptions` 目前无 MCP 字段。策略：

1. 实现期探测 Codex SDK 是否支持 config overrides / mcp；支持则传入。  
2. 不支持：**任务会话**优先走已可注入的 CLI 路径，或启动时 emit `[MCP] SDK 路径暂无法注入，已回退 CLI` / `未注入`。  
3. 不得在 SDK 路径静默忽略绑定。

### 4.4 SSH

- 远程 `codex exec` 命令字符串由 `build_remote_codex_session_command` 生成 → 复用同一 `append_mcp_config_args`，经 `shell_escape_arg`。  
- 远程 MCP **command 在远程主机执行**：args 中的本机路径可能无效 → 日志提示「MCP 在远程执行，请确认远程已安装对应 command」；不在本任务做自动 path rewrite。  
- 若远程 Codex 过旧不支持 `-c`/`mcp_servers`：捕获/探测失败时终端中文降级，会话仍可启动。

### 4.5 无 task_id

one-shot / 无任务会话：resolve(None) → 全局 enabled，同样注入，保持应用清单一致性。

## 5. 前端

- `TaskOverviewPanel` 或 `TaskExecutionPanel` 增加「MCP 工具」区块：  
  - Switch：使用全局默认  
  - 关闭后：Checkbox 列表（`getMcpServers`）  
  - 保存调用 `setTaskMcpBinding`  
  - 展示 effective 预览（从 get 接口）
- `backend.ts` 包装新 command；`types.ts` 扩展 Task。
- 活动 label：`task_mcp_binding_updated` → `更新任务 MCP 绑定`。

## 6. 兼容 / 滚动 / 回滚

| 项 | 策略 |
|----|------|
| 旧任务 | `NULL` 继承，行为 = 全局 enabled 注入（相对今天「不注入应用清单」会有变化：开始按全局 enabled 注入）。**产品接受点**：有应用 MCP 且 enabled 的用户会话会开始带上这些工具。若需更保守，可增加「仅当任务有 override 或设置项开启注入」开关——**默认采用始终注入解析结果**（与 D4 一致）。 |
| 回滚 | 停止注入代码；列可留（可空无害）；UI 隐藏 |
| 密钥 | activity / 终端摘要不输出 env value |

## 7. 测试

- Rust 单测：`resolve_effective_mcp_servers` 三态 + 未知 id  
- Rust 单测：`append_mcp_config_args` 生成的 argv 含预期 `-c` 且空集时无 mcp_servers key（或仅 ignore-user-config）  
- 可选：migration 43 列存在  
- 手工：设置 2 个 MCP → 任务 override 其一 → 开 Codex 会话看 `[MCP]` 日志与工具是否可用

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| `--ignore-user-config` 丢掉用户偏好 | 应用已传 model/effort；文档化；必要时补关键 `-c` |
| SDK 无法注入 | CLI 回退或明确提示 |
| Server name 非法 TOML key | 使用 id slug 化 |
| env 泄露进日志 | 日志只打 name/id 列表 |
| SSH 远程无二进制 | 提示，不阻断会话主流程 |

## 9. 文件触点（预期）

- `src-tauri/src/db/migrations.rs`（v43）
- `src-tauri/src/db/models.rs`（Task / 绑定 DTO）
- `src-tauri/src/codex/mcp.rs`（resolve + binding commands）
- `src-tauri/src/codex/process/command_builders.rs` / `session_launch.rs`
- `src-tauri/src/app/tasks.rs`（list/get SELECT 列）
- `src-tauri/src/lib.rs`（register commands）
- `src/lib/backend.ts`, `src/lib/types.ts`, `src/lib/utils.ts`
- `src/components/tasks/detail/*`（MCP 绑定 UI）
