# Review Findings

> Structured line-level review issues: prompt tag, session event, latest-review payload, Monaco reveal.

## 1. Scope / Trigger

Use this spec when changing review output, `build_task_review_prompt`, `persist_review_session_events`, `get_task_latest_review`, `TaskReviewPanel`, or `TaskExecutionChangeDetailDialog` line anchoring.

No new table. Findings live in `codex_session_events` (`event_type='review_findings'`). Automation still keys only on `review_verdict` + `review_report`.

## 2. Signatures

```rust
// tags in app/shared.rs
REVIEW_FINDINGS_START_TAG = "<review_findings>"
REVIEW_FINDINGS_END_TAG   = "</review_findings>"

fn extract_review_findings(raw: &str) -> Option<String> // same extract_tagged_block as verdict
fn parse_review_findings_json(value: &str) -> Result<Vec<ReviewFinding>, String>
async fn persist_review_session_events(pool, session_id, raw) -> Result<(), String>
async fn get_task_latest_review(app, task_id) -> Result<Option<TaskLatestReview>, String>
```

`ReviewFinding`: `{ file: String, line: Option<i64>, severity: String, message: String }`.

`TaskLatestReview` adds `findings: Vec<ReviewFinding>` and `has_findings_event: bool`.

## 3. Contracts

- Prompt (`build_task_review_prompt` + default `prompt_templates` scene=`review`) requires a third tagged JSON array. `file` is repo-relative; `line` is **new-file 1-based**; `severity` is `blocker|warning|info`; no issues → `[]`.
- Persist is shared: Codex / Claude / Grok Review session-end call `persist_review_session_events` with captured stream text. Native Review session-end calls `persist_review_session_events_from_session_logs` (stdout/stderr already in `codex_session_events`) **before** `handle_session_exit`. OpenCode has no Review kind — do not invent one here.
- Automation `fetch_session_exit_facts` must recover `<review_verdict>` from stdout/stderr when the dedicated `review_verdict` event is missing. Missing recovery treats a valid native/log-only review as “结构化输出无效” and hands Auto QC to manual.
- Write verdict / report / findings independently, only when that block parses. Missing or malformed findings → no `review_findings` event (command still Ok).
- `get_task_latest_review` reads the latest review session’s latest findings event. Parse failure → `has_findings_event=false`, `findings=[]`.
- `format_session_log_line` returns `None` for `review_report` / `review_verdict` / `review_findings`.
- Anchoring uses stored `codex_session_file_change_details` snapshots (local and SSH). No live SSH file fetch.
- Do **not** add a findings activity action. Review still logs `task_review_completed` / `task_review_failed`.
- Do **not** inject findings into the automation fix prompt.

## 4. Validation & Error Matrix

| Condition | Behavior |
|---|---|
| Missing tags / non-JSON / non-array | No event; UI has no findings block |
| Item missing `file` or `message` | Skip item |
| Unknown / empty severity | `info` |
| `line` missing, 0, negative, non-integer | `line=None`; row still listed |
| Filter yields `[]` | Store `[]`; `has_findings_event=true`; UI “无行级问题” |
| File not in execution history | Inline cannot-locate; do not crash |
| Line out of after-text / deleted file / unified-diff only | Open dialog; show cannot-locate; no fake reveal |

## 5. Good / Base / Bad Cases

- **Good**: valid array persists; latest review returns findings; click opens DiffEditor and reveals the modified-side line.
- **Base**: `[]` stored; list empty-state, not an error.
- **Bad**: wrap `onOpenChangeDetail` as `(change) => handler(change)` and drop `{line,message}` — click opens the file but never reveals. New table for findings. Feeding findings into fix-loop verdict.

## 6. Tests Required

- extract: tagged array, `[]`, escaped `<\/review_findings>`, no tags → None
- parse: valid, empty, non-array, skip incomplete items, unknown severity → info
- persist: malformed writes no findings event; `[]` writes event; `get_task_latest_review` returns findings / degrades
- persist from native-style stdout logs (prompt mentions tags + final tagged JSON) writes `review_verdict` / `review_findings`
- session exit facts recover verdict from stdout when the dedicated event is missing; stored `review_verdict` event wins over stdout
- prompt tests assert the findings tags
- frontend: `matchReviewFindingToChange` newest session, `previous_path`, suffix, miss → null

## 7. Wrong vs Correct

#### Wrong
Duplicate verdict/report/findings inserts in each engine `session_runtime`. Native Review exit skipping persist (stdout-only) so Auto QC treats a valid tagged answer as invalid. Parse findings into `ReviewVerdict`. Match only the latest execution session. Call `revealLineInCenter` without passing `options` through the review-panel callback.

#### Correct
One `persist_review_session_events`. Third tagged block, independent degrade. Match all execution history (newest first). Parent must forward `onOpenChangeDetail(change, options)` so Monaco can reveal the modified editor line.
