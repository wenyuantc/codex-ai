# Design: session_origin

## Boundaries

```
编排启动（pipeline.rs）
  → start_* 仍写 session_kind=execution（不改引擎签名）
  → latest_execution_session_id 绑定步骤
  → UPDATE codex_sessions SET session_origin='pipeline'
列表 / 搜索 / 员工进行中
  → 读 session_kind + session_origin
  → 前端 sessionDisplayKind() 分类
UI
  → Badge + 筛选 + 表格列
```

失败：启动失败不写 origin；session_id 为空则不更新（与现有步骤绑定行为一致）。

## Schema

Migration 51:

```sql
ALTER TABLE codex_sessions
  ADD COLUMN session_origin TEXT NOT NULL DEFAULT 'direct';

UPDATE codex_sessions
SET session_origin = 'pipeline'
WHERE id IN (
  SELECT session_id FROM task_pipeline_steps
  WHERE session_id IS NOT NULL AND TRIM(session_id) <> ''
);
```

`INSERT` 可不列该列（默认 `direct`）。`CodexSessionRecord` / list item / running session DTO 都带上该字段。

合法值：`direct` | `pipeline`。未知值按 `direct` 展示。

## Classification

单一规则（Rust 搜索文案 + TS `sessionDisplayKind`）：

1. `session_kind == "review"` → review / 审核对话
2. `session_origin == "pipeline"` → pipeline / 编排对话
3. else → execution / 执行对话

审核优先，避免脏 origin。

## Write path

在 `update_pipeline_step_status(..., session_id)` 之后（启动成功、已有 id）调用：

```sql
UPDATE codex_sessions SET session_origin = 'pipeline' WHERE id = $1
```

不改 `insert_codex_session_record` 签名。OpenCode 仍只跑 Execution 槽。

## Display

- `formatSessionKind` 改为按 display kind 映射 i18n
- Badge class：执行绿 / 审核蓝 / 编排紫（对齐 `TaskSessionChainPanel` role badge）
- 表格新增类型列；状态列不再重复类型小字
- `kindFilter` 值：`all` | `execution` | `review` | `pipeline`，用 display kind 比较
- 会话链：review → 审核；pipeline origin → 编排（优先于 fix 启发式）；其余保持 execute/fix
- 员工进行中对话框改用共享 `formatSessionKind`，去掉硬编码中文
- 全局搜索 `search_session_kind_label(kind, origin)`

## Compatibility

- `session_kind` 不变：占用槽、日志 key、改动文件、latest_execution_session_id
- SSH：origin 在本地库，与 `execution_target` 无关
- 无新 activity key

## Tests

- Migration：列存在、默认 `direct`、按 pipeline step 回填
- Pipeline 绑定后 origin 为 `pipeline`
- Vitest：`sessionDisplayKind` / `formatSessionKind` 三分类 + review 优先
