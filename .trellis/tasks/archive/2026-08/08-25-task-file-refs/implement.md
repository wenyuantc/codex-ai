# Implement · 任务项目文件引用

## Checklist

1. Migration 53 + `TaskFileRef` + `CreateTask.file_ref_paths` in `db/models.rs`; bump `latest_migration_version` tests if they hardcode 52.
2. Path normalize/validate helper + unit tests in `app/tasks.rs` (or small helper module next to tasks).
3. `git_bridge.mjs` `list_files`; `git_runtime` wrapper; `git_workflow::list_project_files`; register in `lib.rs`.
4. `app/tasks.rs`: insert on create; `list_task_file_refs` / `add_task_file_refs` / `delete_task_file_ref`; activity logs; register in `lib.rs`.
5. Coordinator plan + automation fix prompt include file refs; extend their tests.
6. Frontend types, `backend.ts`, `taskStore`, i18n (`tasks` + `activity`), `locale.test.ts` + `utils.test.ts`.
7. `ProjectFileRefPicker` + wire `CreateTaskDialog` and `TaskCollaborationPanel` / `TaskDetailDialog`.
8. `taskPrompt.ts` + every `buildTaskExecutionInput` call site (`CreateTaskAndRun`, detail, card, kanban, CodexControls).
9. Vitest for `taskPrompt` file-ref block.
10. `clippy --all-targets -- -D warnings`, `npm run format:check`, `npm run test:ci`, targeted `cargo test`.

## Validation

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run format:check
npm run test:ci
cargo test --manifest-path src-tauri/Cargo.toml normalize_task_file_ref
cargo test --manifest-path src-tauri/Cargo.toml build_automation_fix_prompt
```

Desktop smoke (manual): create dialog without project → button disabled; pick project → search files → create; open detail add/remove; run task and confirm prompt contains paths. SSH project: picker still lists remote git files.

## Risky files

- `src/lib/taskPrompt.ts` call sites (easy to miss one)
- `git_bridge.mjs` (local + SSH)
- `create_task` transaction (insert refs before commit)
- activity keys must exist in both locale packs

## Rollback points

- Before migration 53 ships in a release, revert the commit.
- After users run v53, table stays; hide UI if needed.
