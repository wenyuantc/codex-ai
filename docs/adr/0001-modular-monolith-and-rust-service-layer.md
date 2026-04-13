# ADR 0001: 模块化单体与 Rust 应用服务层

## 状态

已采纳

## 背景

项目最初更接近“可运行 demo”：

- 前端 store 直接写 SQLite
- 员工归属同时存在 `employees.project_id` 和 `project_employees`
- Codex 运行时缺少结构化会话记录和健康检查
- 设置页无法反映真实运行状态

这会导致业务规则散落、事务不完整、运行失败难以归因。

## 决策

采用桌面端模块化单体，不拆分远程服务。关键边界如下：

1. 业务写路径统一收口到 Rust/Tauri commands。
2. 前端 store 只负责缓存、交互和调用命令。
3. 员工与项目关系统一为 `employees.project_id`。
4. Codex 运行时会话统一记录到 `codex_sessions` 与 `codex_session_events`。
5. 运行前必须校验工作目录是否存在、可访问且是 Git 仓库。

## 结果

正向影响：

- 项目、任务、员工的写操作有了统一入口
- 任务状态变更可同步写活动日志和绩效指标
- Codex 的 start/stop/restart/status 有了会话状态支撑
- 设置页可以展示真实健康状态和最近错误

代价：

- `src-tauri` 侧承担更多业务逻辑
- 前端与 Rust 命令之间需要维护 DTO 契约
- 迁移逻辑需要兼容旧数据

## 迁移策略

- 停止从 `project_employees` 读取团队成员
- 将历史归属回填到 `employees.project_id`
- 对多项目历史数据记录迁移日志
- 保留旧表作为兼容痕迹，但不再新增业务写入

## 暂不做

- 微服务拆分
- 远程同步
- 复杂 agent 编排平台
- 交互式 stdin 会话模型
