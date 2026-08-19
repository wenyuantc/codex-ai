# Implement · B2 任务模板

依赖：v46 已在 main。本任务只加 v47 + 模板命令 + 看板 UI。

## Checklist

- [ ] v47 `task_templates` + 索引；`latest_migration_version` 断言 46→47
- [ ] `db/models.rs`：`TaskTemplate` / Create / Update / Apply payload；`src/lib/types.ts` 对齐
- [ ] `app/templates.rs`：变量纯函数 + CRUD + from_task + apply 事务；`app/mod.rs` 挂模块
- [ ] `lib.rs` 注册 6 个命令
- [ ] 套用复用 `resolve_project_task_default_settings`、审查员校验、`insert_task_record`、质控 state 初始化
- [ ] 活动日志 3 键；`src/locales/{zh-CN,en}/activity.json`
- [ ] Rust 测试：extract/render、缺变量、超限、软删不出现在 list、apply 替换 + 标签 find-or-create + 子任务
- [ ] `backend.ts` 包装；不要在组件里 `invoke`
- [ ] `TaskTemplateManagerDialog`：列表/编辑/删除/套用表格
- [ ] `KanbanPage`「模板」按钮；套用后 `fetchTasks` + 刷新标签图
- [ ] `TaskCard` 右键「存为模板」
- [ ] i18n：`kanban.json` + `tasks.json` zh-CN / en

## Validation

```bash
cargo test --manifest-path src-tauri/Cargo.toml templates
cargo test --manifest-path src-tauri/Cargo.toml latest_migration_version
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run test:ci
npm run format:check
npm run build
```

手工（`npm run tauri:dev`）：从任务存模板 → 把标题改成 `给 {{module}} 补 i18n` → 套用两行 → 看板上两条待办、标签与子任务正确 → 缺变量被拒且无新任务 → 软删除后列表消失 → SSH 项目同样套用 → 仪表盘活动中文。

## Risky files

- `create_task` / `insert_task_record`：套用不要改对外契约，不要走附件/SSH 上传。
- `TaskCard.tsx`：右键菜单已很长，只加一项，勿顺手重构。
- `KanbanPage.tsx`：入口放在新建与归档之间，勿打乱批量运行条。

## Rollback

只 revert 本功能提交。不要回退 v45/v46。若迁移已应用到本机开发库，空 `task_templates` 表可保留。
