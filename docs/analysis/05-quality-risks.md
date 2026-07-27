# 05 · 质量与风险清单

## 1. 风险 TOP 10

| # | 风险 | 影响 | 概率 | 证据 |
|---|------|------|------|------|
| 1 | 前端可直接 `execute` SQL | 绕过业务规则/审计不完整 | 中 | `database.ts` + `sql:allow-execute` + 3 处 INSERT |
| 2 | `git_workflow.rs` ~5.5k 单文件 | 回归难、冲突多、评审失效 | 高 | LOC |
| 3 | `task_automation` 状态机复杂 | 错误提交/双开会话/卡死 phase | 中 | ~2.9k + resume 逻辑 |
| 4 | 无前端自动化测试 | UI 状态不同步类 bug 易复发 | 高 | package.json 无 test script |
| 5 | 引擎能力不对称 | 用户以为三引擎等价 | 中 | restart/input 仅 Codex command |
| 6 | SSH artifact 降级 | 审查/diff 不完整导致误判 | 中 | capture modes + notice 组件 |
| 7 | DTO 双端手工 | 字段静默丢失 | 中 | models.rs / types.ts |
| 8 | 备份不含附件/密钥语义不清 | 用户误以为可完整灾备 | 中 | backup_database 范围 |
| 9 | 超大 React 组件 | 卡顿、难测、状态局部错误 | 中 | TaskCard/Detail/ProjectDetail >1k 行 |
| 10 | capabilities `shell:allow-execute` 宽 | 攻击面依赖命令构造正确性 | 低–中 | default.json |

## 2. 测试现状

### 2.1 有

`src-tauri/src/app/tests/`（约 1.6k LOC）：

| 文件 | 方向 |
|------|------|
| `runtime_and_paths.rs` | 路径/运行时 |
| `sql_and_session.rs` | SQL/会话 |
| `task_lifecycle.rs` | 任务生命周期 |
| `review_and_attachments.rs` | 审查与附件 |

另：`git_workflow.rs` 内含部分 `#[cfg(test)]` 单元测试（worktree 路径等）。

全库 `#[test]` 量级约 **200+** 标注行（含模块内测），但**不覆盖**前端与多数 IPC 端到端。

### 2.2 无

- 前端 unit / component / e2e  
- ESLint / Prettier / Clippy 强制门禁  
- Command 契约快照测试  
- 自动化状态机属性测试  

### 2.3 建议最小测试网（质量向，非本阶段实施）

1. 任务状态迁移 + 软删守卫  
2. automation phase 转换表驱动  
3. git high-risk confirm token  
4. 前端：store 过滤 local/ssh + 项目作用域  
5. `getActivityActionLabel` 覆盖后端新 action key  

## 3. 安全

| 主题 | 观察 |
|------|------|
| SSH 密钥 | `secret_store` + redact helpers 存在 |
| SQL 注入 | 后端 SQLx 绑定；前端拼接 LIKE 有 escape（dashboard）但仍危险若滥用 execute |
| 命令注入 | remote `shell_escape_single_quoted`；需审计所有 SSH 拼接点 |
| 路径穿越 | validate_*_path 系列 |
| 备份脚本 | `sanitize_sql_backup_script` |
| CSP/IPC | Tauri 2 capabilities 白名单；sql execute 过宽是主要前端面 |

## 4. 性能与可维护性

### 4.1 后端热点

| 文件 | 行数 | 建议 |
|------|------|------|
| git_workflow.rs | 5515 | 按 overview/stage/branch/worktree/task-context/confirm 拆模块 |
| task_automation.rs | 2943 | policy / resume / phase handlers 拆分 |
| opencode/process/mod.rs | 2187 | 对齐 codex process 子模块结构 |
| app/tasks.rs | 1794 | 附件/归档/计时分包 |

### 4.2 前端热点

| 文件 | 行数 |
|------|------|
| ProjectDetailPage.tsx | 1425 |
| TaskDetailDialog.tsx | 1339 |
| TaskCard.tsx | 1290 |
| SettingsPage.tsx | 1081 |
| backend.ts / types.ts | ~1100 |

### 4.3 运行时性能关注

- Monaco 多实例同时打开  
- 会话日志无限追加  
- Dashboard 多路 `select` 无请求合并（除 pendingProjectsLoad）  
- Git overview 频繁全量刷新  

## 5. 构建与 CI

| 命令 | 作用 |
|------|------|
| `npm run build` | tsc + vite |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Rust 测试 |
| `npm run tauri:dev` | 全栈冒烟 |
| `.github/workflows/build.yml` | 多 OS 打包（tag/manual） |

缺口：PR 级强制 test；无前端 typecheck-only CI 分轨说明（build 含 tsc）。

## 6. 产品文档债

| 文档 | 状态 |
|------|------|
| README | 较完整，与代码大体一致 |
| ADR-0001 | 有效；部分「前端不写 SQL」已被现实突破 |
| notification matrix | 较新、可用 |
| BUG.md / FUN.md | **部分过时**（标签/依赖已实现；备份已有 SQL） |
| TASK.md | 仍有 MCP、角色讨论未决 |
| OpenWolf `.wolf/` | 仓库内不存在（指令引用但目录缺失） |

## 7. 边界违规清单（可勾选修复）

- [ ] `src/stores/projectStore.ts` 移除 `execute(INSERT activity_logs)`  
- [ ] `src/components/search/GlobalSearchDialog.tsx` 同上  
- [ ] capabilities 去掉 `sql:allow-execute`（或仅 debug）  
- [ ] 评估 store 的 `SELECT` 是否迁到 query commands  
- [ ] 更新 ADR 或落地 ADR（二选一，避免文档撒谎）  

## 8. UX 风险（服务路线图 C）

1. 看板卡片信息密度 + 右键菜单过重  
2. 任务详情 Tab 过多，首屏不知从何操作  
3. 设置四 Tab 信息架构可，但 Runtime 单 Tab 仍 ~970 行组件  
4. 全局搜索、通知跳转 deep-link 与路由持久化可能冲突（需手工回归）  
5. 暗色模式历史 bug 需持续冒烟  
