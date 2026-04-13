# Codex AI Desktop

一个基于 `Tauri 2 + React 19 + SQLite` 的本地桌面协作应用，用来管理项目、任务、AI 员工以及 Codex CLI 运行会话。

## 当前架构

项目采用桌面端模块化单体：

- `src/` 负责页面、交互和 Zustand 状态缓存
- `src-tauri/src/app.rs` 提供应用服务层，统一处理写操作、运行时健康检查和业务校验
- `src-tauri/src/codex` 负责 Codex CLI 进程控制和会话事件
- `src-tauri/src/db` 负责迁移与数据模型

当前核心边界：

- 前端 `store` 不再直接写 SQL
- 任务、项目、员工的写路径统一通过 Tauri commands
- 员工归属以 `employees.project_id` 为唯一来源
- Codex 会话写入 `codex_sessions` / `codex_session_events`

## 主要能力

- 项目、任务、员工的本地管理
- 任务状态流转、评论、子任务维护
- Codex CLI 会话启动、停止、重启
- 设置页健康检查：Codex CLI、数据库路径、最近一次运行错误
- 员工绩效表的最小指标写入

## 数据模型摘要

主要表：

- `projects`
- `employees`
- `tasks`
- `subtasks`
- `comments`
- `activity_logs`
- `employee_metrics`
- `codex_sessions`
- `codex_session_events`

说明：

- `employees.project_id` 是员工与项目的唯一归属关系
- `project_employees` 已停止作为业务读写来源，仅保留迁移兼容痕迹
- `codex_sessions` 记录一次运行会话的状态、工作目录、退出码和 CLI session id

## 本地运行要求

- Node.js / npm
- Rust toolchain
- Tauri 2 运行环境
- 本机已安装 `codex` 命令，并可在 shell 中执行

## 开发命令

- `npm run dev`
- `npm run build`
- `npm run preview`
- `npm run tauri dev`
- `npm run tauri build`
- `cargo test --manifest-path src-tauri/Cargo.toml`

## 运行时校验

设置页会执行健康检查，返回：

- `codex` 是否可执行
- `codex --version` 输出
- SQLite 数据库是否已加载
- 本地数据库路径
- 最近一次会话失败原因

Codex 运行前会校验：

- 工作目录存在
- 工作目录是目录
- 工作目录可访问
- 工作目录包含 `.git`

## 验证基线

提交前至少执行：

- `npm run build`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `npm run tauri dev` 手工冒烟一轮：创建项目、创建员工、创建任务、启动/停止 Codex

## 设计决策

- ADR: [docs/adr/0001-modular-monolith-and-rust-service-layer.md](docs/adr/0001-modular-monolith-and-rust-service-layer.md)
