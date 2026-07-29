# Codex AI Desktop

基于 `Tauri 2 + React 19 + SQLite` 的本地桌面协作应用，管理项目、任务、AI 员工以及多引擎 AI 会话（Codex/Claude/Grok/OpenCode）。提供看板任务流转、Git 工作区管理、代码审查、任务自动化和远程 SSH 执行能力。

## 当前架构

项目采用桌面端模块化单体：

- `src/` 负责页面、交互和 Zustand 状态缓存
- `src-tauri/src/app/` 提供应用服务层，统一处理写操作、运行时健康检查和业务校验
  （`projects` / `employees` / `tasks` / `delivery` / `sessions` / `review` / `remote` / `database`）
- `src-tauri/src/codex` 负责 Codex CLI 进程控制和会话事件
- `src-tauri/src/claude` 负责 Claude CLI 集成与会话管理
- `src-tauri/src/grok` 负责 Grok CLI 集成与会话管理
- `src-tauri/src/opencode` 负责 OpenCode SDK 集成与会话管理
- `src-tauri/src/db` 负责迁移与数据模型
- `src-tauri/src/git_workflow.rs` / `git_runtime.rs` 负责完整 Git 工作流与底层 git 进程执行
- `src-tauri/src/task_automation.rs` 负责任务审核-修复自动化循环
- `src-tauri/src/process_spawn.rs` 统一进程拉起；AI 引擎一律由 Rust 侧启动，不经前端 shell 插件

当前核心边界：

- 前端不能写 SQL：`src/lib/database.ts` 的 `execute()` 直接抛错，`capabilities/default.json` 只授予 `sql:default` + `sql:allow-select`（不含 `sql:allow-execute`）
- 前端仍有部分只读 `select()` 查询直连 SQLite；新增读路径优先走 Tauri command
- 任务、项目、员工的写路径统一通过 Tauri commands（共 168 个，集中注册在 `lib.rs`）
- 员工归属以 `employees.project_id` 为唯一来源
- 四个引擎的会话统一写入 `codex_sessions` / `codex_session_events`（表名是历史遗留，非仅 Codex 专用）
- 数据行为软删除，查询需过滤 `deleted_at IS NULL`

## 页面与路由

| 路径 | 页面 | 说明 |
|------|------|------|
| `/` | 仪表盘 | 统计指标卡、任务分布图、活动动态、员工绩效 |
| `/projects` | 项目列表 | 项目管理（本地/SSH）、SSH 配置管理 |
| `/projects/:id` | 项目详情 | Git 概览、分支/提交/worktree 管理、任务统计 |
| `/kanban` | 看板 | 5 列任务看板（待办/进行中/审核中/已完成/已阻塞）、拖拽状态流转、任务 CRUD |
| `/sessions` | 会话管理 | AI 会话列表、日志查看、变更历史、继续对话 |
| `/employees` | 员工管理 | AI 员工 CRUD、角色与提供商绑定、运行时状态 |
| `/settings` | 设置 | 运行时/SDK、Git 自动质控、提示词模板、MCP、SSH、数据库维护 |
| `/trash` | 回收站 | 软删除数据的查看与恢复 |

快捷键统一定义在 `src/lib/shortcuts.ts`（macOS 为 `⌘`，其他平台为 `Ctrl`）：

- **导航**：`⌘1` 仪表盘 · `⌘2` 项目 · `⌘3` 看板 · `⌘4` 会话 · `⌘5` 员工 · `⌘6` 设置 · `⌘7` 回收站
- **全局**：`⌘K` 全局搜索 · `⌘T` 切换主题 · `⌘B` 切换侧边栏 · `?` 快捷键帮助
- **页面级**：`N` 新建（看板/项目/员工）· `A` 归档管理（看板）· `R` 刷新（会话）

## 主要能力

### 项目管理

- 本地项目与 SSH 远程项目双模式
- Git 概览面板：默认分支、当前分支、HEAD commit、工作区变更文件列表
- 完整 Git 暂存工作流：单个/批量/全部文件暂存与取消暂存
- 分支管理：切换、创建、删除、合并分支（支持多种合并策略）
- Worktree 管理：创建、查看、提交、合并、删除 worktree
- 提交管理：普通提交 + AI 生成 Conventional Commit 格式提交信息
- 推送/拉取、回滚文件、分支冲突合并
- 文件内容预览与差异对比（集成 Monaco Editor 语法高亮）
- 高风险 Git 操作安全确认机制（合并/推送/变基/cherry-pick/stash）
- 项目级活动日志与通知

### 任务管理

- 完整状态机：`todo → in_progress → review → completed / blocked / archived`
- 看板 5 列视图（`archived` 不在看板展示，走归档管理），支持拖拽卡片切换状态
- 优先级（低/中/高/紧急）与复杂度标签
- 执行人、审核人、协调员三角色分配
- **AI 辅助**：推荐执行人、分析复杂度、生成执行计划、拆分子任务、生成评论、优化 Prompt
- 子任务列表：批量添加（去重）、勾选完成、删除
- 评论系统：普通评论与 AI 生成评论
- 附件管理：文件上传、图片预览、排序
- 归档管理：查看已归档任务、取消归档、永久删除
- 任务执行变更历史追踪
- Worktree 绑定：任务可选在独立 worktree 中执行

### AI 员工管理

- 员工 CRUD，支持角色：执行人、审核人、协调员、观察者
- 每位员工独立绑定 AI 提供商（Codex/Claude/Grok/OpenCode）
- 模型与推理强度单独配置
- 专业领域与自定义系统提示词
- 运行时状态实时同步（在线/忙碌/离线/异常）
- 查看员工当前正在运行的会话

### AI 会话管理

- 四引擎支持：Codex (OpenAI)、Claude (Anthropic)、Grok (xAI)、OpenCode (开源)
- 会话生命周期：启动、停止、重启、继续对话、发送输入。各引擎能力不同，以 `get_ai_provider_capabilities` 为准：

  | 引擎 | 启动 | 停止 | 重启 | 会话中发送输入 | 续聊 |
  |------|:---:|:---:|:---:|:---:|:---:|
  | Codex | ✅ | ✅ | ✅ | ✅ | ✅ |
  | Claude | ✅ | ✅ | ❌ | ❌ | ✅ |
  | Grok | ✅ | ✅ | ❌ | ❌ | ✅ |
  | OpenCode | ✅ | ✅ | ❌ | ❌ | ✅ |
- 会话列表查询与全文搜索过滤（ID、任务、内容、项目、执行目标）
- 会话日志实时流式展示
- 会话文件变更记录与 diff 查看
- 会话续聊状态检测
- AI 提供商标签展示
- 会话关联任务与员工

### 代码审查

- 自动收集任务上下文（Git diff、未跟踪文件等）发送给 AI 审查
- 审查结论自动解析：通过/不通过、阻塞问题数量、摘要
- 任务审查状态追踪与附件同步（SSH 场景）

### 任务自动化

- `review_fix_loop_v1` 模式：审核 → 修复 → 执行的自动循环
- 可配置最大修复轮次（默认 3 轮）
- 自动化失败策略：转阻塞 / 转人工
- AI 自动生成修复 Prompt
- 审核通过后自动提交代码
- 应用启动时自动恢复未完成的自动化流程

### 仪表盘

- 统计指标卡：项目总数、活跃项目、任务总数、员工总数、在线员工数、完成率、未读通知数
- 任务分布柱状图：按状态展示数量对比
- 活动动态时间线：支持按项目/动作/关键词/日期过滤和分页查询
- 员工绩效图表：完成任务数、平均完成时间、成功率

### 通知系统

- 通知类型：审核待处理、运行失败/完成、任务完成、SDK 不可用、数据库错误、SSH 配置错误
- 严重级别：信息/成功/警告/错误/严重
- 通知生命周期：活跃 → 已读 → 已解决
- 粘性通知持续显示直到问题解决
- 去重机制（dedupe_key 合并）
- 桌面通知弹窗（Tauri 系统通知 + 应用内通知中心）
- 通知动作跳转链接

### 系统托盘

- 系统托盘图标，左键单击显示主窗口
- 关闭窗口隐藏到托盘（不退出）
- 窗口尺寸持久化

### 设置页（6 个标签页）

**界面与运行**：主题切换（亮色/暗色）、Codex/Claude/Grok/OpenCode 引擎配置与健康检查、模型选择、推理强度、Node 路径覆盖

**Git 与自动质控**：任务自动化开关、最大修复轮次、失败策略、Worktree 模式、AI 提交信息格式与模型配置

**提示词模板**：可配置的 AI 提示词模板管理

**MCP 管理**：MCP server 配置的查看、编辑、重置与配置片段导出

**SSH 配置**：SSH 配置 CRUD、密码/密钥认证、连接测试、密码认证探测、远程 SDK 安装与健康验证

**数据库维护**：版本信息、SQL 备份导出、SQL 备份导入（自动保护性备份）、打开数据文件夹

### 全局搜索

`⌘K` 打开全局搜索：跨项目、任务、员工、会话的文本搜索。

## 数据模型摘要

共 22 张表，由 `src-tauri/src/db/migrations.rs` 内联的 80 个版本化迁移维护。

**核心业务**

- `projects` — 项目信息（名称、描述、状态、类型、仓库路径）
- `employees` — AI 员工（名称、角色、提供商、模型、系统提示词、项目归属）
- `tasks` — 任务（标题、描述、状态、优先级、执行人/审核人/协调员、自动化模式）
- `subtasks` — 子任务
- `comments` — 评论
- `activity_logs` — 活动日志
- `employee_metrics` — 员工绩效指标

**交付管理**

- `milestones` — 里程碑
- `tags` / `task_tags` — 标签与任务标签关联
- `task_dependencies` — 任务依赖关系
- `task_attachments` — 任务附件

**会话**

- `codex_sessions` — AI 会话记录（类型、状态、工作目录、退出码、提供商等）
- `codex_session_events` — 会话事件
- `codex_session_file_changes` / `codex_session_file_change_details` — 会话文件变更与差异明细

**运行支撑**

- `ssh_configs` — SSH 连接配置
- `notifications` — 通知记录
- `task_automation_state` — 任务自动化状态机持久化
- `task_git_contexts` — 任务 Git 上下文绑定

说明：

- `employees.project_id` 是员工与项目的唯一归属关系
- `project_employees`、`codex_sessions_new` 已停止作为业务读写来源，仅保留迁移兼容痕迹
- `codex_sessions` 记录一次运行会话的状态、引擎类型、工作目录、退出码和 CLI session id；四个引擎共用此表
- 数据行为软删除（`deleted_at`），回收站页面可查看与恢复

## 本地运行要求

- Node.js / npm
- Rust toolchain（安装：`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`）
- Tauri 2 运行环境
- 至少安装一个 AI 引擎 CLI 并可在 shell 中执行：`codex` / `claude` / `grok` / `opencode`（按员工绑定的提供商选装；设置页可逐个健康检查）

## 开发命令

```bash
npm run dev                            # 前端开发服务器
npm run build                          # TypeScript 检查 + Vite 构建
npm run preview                        # 预览构建产物
npm run tauri:dev                      # 完整 Tauri 开发环境
npm run tauri                          # 透传 tauri CLI（如 tauri build）
npm run bump-version                   # 同步升级 package.json / Cargo.toml / tauri.conf.json 版本号
npm run tauri:dmg:no-sign              # macOS 未签名 DMG
npm run tauri:linux                    # Linux 打包（AppImage/deb/rpm）
npm run tauri:windows                  # Windows 打包（NSIS/MSI）
cargo test --manifest-path src-tauri/Cargo.toml               # Rust 测试（项目唯一测试套件，246 个用例）
cargo test --manifest-path src-tauri/Cargo.toml <test_name>   # 运行单个测试
```

## 技术栈

- **前端**：React 19 + TypeScript + Vite + TailwindCSS 4 + shadcn/ui + Monaco Editor
- **状态管理**：Zustand（6 个 store：project、task、employee、dashboard、notification、log）
- **后端**：Rust 2021 + Tokio async + SQLx 0.8（编译期查询检查）+ Tauri 2
- **数据库**：SQLite
- **AI 引擎**：Codex CLI (OpenAI)、Claude CLI (Anthropic)、Grok CLI (xAI)、OpenCode (开源)
- **桌面能力**：系统托盘、桌面通知、窗口状态持久化、路由持久化

## 运行时校验

设置页会执行健康检查，返回：

- `codex` / `claude` / `grok` / `opencode` 是否可执行
- 版本信息
- SQLite 数据库是否已加载、当前版本与最新可用迁移版本
- 本地数据库路径
- 最近一次会话失败原因

四个引擎启动会话前都会经过同一道校验（`app/shared.rs::validate_runtime_working_dir`）：

- 工作目录存在
- 工作目录是目录
- 工作目录可访问
- 工作目录包含 `.git`

## 验证基线

提交前至少执行：

- `npm run build`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `npm run tauri dev` 手工冒烟一轮：创建项目、创建员工、创建任务、拖拽看板、启动/停止/继续 AI 会话

## 设计决策与项目文档

- ADR: [docs/adr/0001-modular-monolith-and-rust-service-layer.md](docs/adr/0001-modular-monolith-and-rust-service-layer.md)
- 深度分析: [docs/analysis/](docs/analysis/README.md) — 架构全景、能力矩阵、数据模型、IPC 目录、运行时生命周期、质量风险、技术债路线图
- 通知事件矩阵: [docs/notification-center-event-matrix.md](docs/notification-center-event-matrix.md)
- 编码规约: `.trellis/spec/{backend,frontend,guides}/`
