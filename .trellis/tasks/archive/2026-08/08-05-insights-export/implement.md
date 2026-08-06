# Implement — 报表洞察与任务 JSON 导入导出

## 检查清单（实现顺序）

### A. 报表 hardening

1. [ ] 扩展 `GetDashboardReportPayload` / `DashboardReportSummary`（`selected_ssh_config_id`、`aging_*`、`weekly_completed_series`）
2. [ ] 重构 `get_dashboard_report_summary` 作用域：与 `get_dashboard_stats` 同 scoped project ids
3. [ ] 实现 aging 计数 + 近 8 周完成序列；保留近 7 日序列字段兼容
4. [ ] `backend.ts` + `DashboardPage` 传 SSH 过滤并展示新指标/周图

### B. JSON 导出

5. [ ] models：`ExportTasksJsonPayload` / `Result` + envelope serde 类型（可内联 private struct）
6. [ ] `export_tasks_json`：scoped 查询 + tags/subtasks/deps 组装 + activity `tasks_json_exported`
7. [ ] 注册 `lib.rs`；前端 `exportTasksJson`；仪表盘按钮改 JSON 下载

### C. JSON 导入

8. [ ] `import_tasks_json`：schema 校验、白名单、预校验、单事务写入 tags/subtasks/deps
9. [ ] conflict：`create_new`（默认）/ `skip_existing`
10. [ ] activity `tasks_json_imported` + 中文 label（export/import 两个 key）
11. [ ] UI：导入文件选择 + 结果摘要（依赖当前项目）

### D. 质量

12. [ ] Rust 单测：export 字段安全、import 往返/skip/非法 format
13. [ ] `npm run build` + clippy `-D warnings`
14. [ ] 手工烟测：local 过滤、SSH 主机过滤、导出→导入关键字段

## 验证命令

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml export_tasks
cargo test --manifest-path src-tauri/Cargo.toml import_tasks
cargo test --manifest-path src-tauri/Cargo.toml dashboard_report
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

（测试名以实际 `#[test] fn` 为准；可用模块过滤。）

## 主要改动文件

| 区域 | 路径 |
|------|------|
| Report command | `src-tauri/src/app/database.rs` |
| Export/Import | `src-tauri/src/app/tasks.rs` |
| DTOs | `src-tauri/src/db/models.rs` |
| 注册 | `src-tauri/src/lib.rs` |
| IPC | `src/lib/backend.ts` |
| Labels | `src/lib/utils.ts` |
| UI | `src/pages/DashboardPage.tsx` |
| 可选组件拆分 | `src/components/dashboard/*`（趋势图过长时抽出） |

## 高风险点

- 作用域 helper 与 stats 不一致 → SSH 串数据（对齐 `resolve` 逻辑）
- 导入事务半成功 → 坚持预校验 + 单事务
- 误导出员工/SSH 字段 → serde 白名单 struct，禁止 `Task` 全量 Serialize 直出

## 回滚点

- 无 migration；禁用 UI 按钮 + 取消 command 注册即可
- 保留 `export_tasks_csv` 作为暗命令直至确认无调用

## 开始前

用户明确批准本任务最终规划摘要后：

```bash
python3 ./.trellis/scripts/task.py start 08-05-insights-export
```

然后加载 `trellis-before-dev`，Phase 2 用 `trellis-implement` / 本会话实现。
