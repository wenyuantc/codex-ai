# Design · C1 审查行级定位

## Boundaries

- 不新表、不新命令。复用 `codex_session_events` + 扩展 `get_task_latest_review` 返回体。
- 解析与落库进 `app/review.rs`（已有 `parse_review_verdict_json`）。三引擎会话结束改调同一 helper，禁止再复制第三份提取块。
- 前端只经 `backend.ts`。`TaskLatestReview.findings` 为缓存字段，不进 Zustand。
- 变更详情弹窗升级影响任务详情与会话页（共用 `TaskExecutionChangeDetailDialog`）；会话页不传行号，行为仅编辑器升级。

## Data flow

```
审查会话 stdout
  → extract <review_findings>
  → parse_review_findings_json
  → 合法则 insert event_type=review_findings
  → get_task_latest_review 读最新 review 会话的最新 findings 事件
  → TaskReviewPanel 列表
  → 纯函数匹配 executionChangeHistory
  → get_codex_session_file_change_detail
  → Monaco DiffEditor reveal 修改后一侧
```

畸形 / 缺块：不写事件，命令仍成功，`findings=[]`。

## Contracts

**Prompt**（`build_task_review_prompt` + `prompt_templates` scene=`review`）增加：

- 必须且只能把 findings JSON 放在 `<review_findings>` … `</review_findings>`
- schema：`[{file:string, line:number, severity:"blocker"|"warning"|"info", message:string}]`
- `file` 相对仓库根；`line` 为修改后文件行号；无问题输出 `[]`
- 标签常量放 `app/shared.rs`（与 verdict/report 并列）

**解析**（`app/review.rs`）

| 函数 | 行为 |
|---|---|
| `extract_review_findings(raw)` | 复用 `extract_tagged_block`（建议把它从 `codex/process/mod.rs` 提到 crate 可见，或 findings 提取放在已有 `extract_review_verdict` 旁并 re-export） |
| `parse_review_findings_json` | 顶层必须是数组。元素缺 `file`/`message` 或二者 trim 空 → 跳过该条。`severity` 小写归一；未知 → `info`。`line` 缺省/非正整数 → `None`（仍保留 finding，点击只开文件不 reveal）。整段非 JSON / 非数组 → `Err`，调用方当缺失。合法后即使过滤后为空数组也算成功（存 `[]`）。 |
| `persist_review_session_events(pool, session_id, raw)` | 写 verdict（parse 成功才写）、report（有块才写）、findings（parse 成功才写）。Codex/Claude/Grok 的 Review 结束处改为调它。 |

**读取**：`TaskLatestReview` 加 `findings: Vec<ReviewFinding>`。查询：`event_type='review_findings' ORDER BY created_at DESC LIMIT 1`，parse 失败当空。

**终端**：`format_session_log_line` 对 `review_verdict` / `review_findings` 返回 `None`（与 `review_report` 一致）。

**自动化**：`session_exit` / `fix_loop` 仍只读 verdict + report。不要把 findings 接进修复 prompt。

## Path matching（前端纯函数，Vitest）

`matchReviewFindingToChange(finding, history) -> CodexSessionFileChange | null`

- 规范化：`\`→`/`、去前导 `./`、trim。
- `history` 按会话 `started_at` 新→旧。
- 先精确匹配 `change.path`，再 `previous_path`，再 suffix（finding 或 path 一方是另一方后缀，避免绝对/相对混用）。
- 多命中取最新会话中的第一条。

## UI

- `TaskReviewPanel`：findings 在报告 ScrollArea 上方。空数组且无事件不占位；有事件但 `[]` 显示「无行级问题」。
- 点击：父组件现有 `handleOpenExecutionChangeDetail` 增加可选 `{line, message}`；弹窗 `revealLine?: number`。
- `TaskExecutionChangeDetailDialog`：有 before/after 文本时用 `createDiffEditor`（抄 `ProjectGitFilePreviewDialog`：只读、并排、`loadMonaco`）。reveal 用 `getModifiedEditor()`。无 after 文本或 line 空：不 decoration，顶部提示无法定位到行。
- 三 Tab 纯文本预览可去掉；只保留 DiffEditor + 原有 snapshot 元信息。若 before/after 都不可用但有 `diff_text`：单栏只读 Monaco 显示 unified diff，不声称已定位。

## Compatibility

- SSH：审查上下文采集不变；锚定读本地 SQLite snapshot。
- OpenCode：无 Review kind，不改 `opencode/`。
- 审查员 `ai_provider==opencode` 仍走现有 Codex start（本波不修）。
- 无迁移。`review_findings` 随会话事件保留策略过期删除，可接受。

## Tradeoffs

- 第三标签而不是塞进 verdict：verdict 解析保持严格字段；findings 可单独降级。
- 匹配全部执行历史而不是「仅最新会话」：同一验收「点到行」，少误报无法定位。
- 抽 `persist_review_session_events`：C1 若只在三处粘贴，三引擎必漂移。
- 共用弹窗升级：会话页也换成 Monaco，符合「大文本用 Monaco」；不改其打开路径。

## Rollback

单提交 revert。无迁移。已写入的 `review_findings` 事件残留无害（旧 UI 不当终端行即可；若已藏类型则只是多几行 JSON）。
