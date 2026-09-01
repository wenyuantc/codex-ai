# Implement · 协调员计划按意见修改

## Order

1. Prompt 场景 `coordinator_plan_revise` + `build_ai_revise_plan_prompt` + payload 字段 + 修订分支落库/活动日志 + Rust 单测。
2. native 规划 one-shot：`save_transcript`；`AiCommandOptions.resume_session_id`；修订时查找并 load。
3. `generateCoordinatorPlanForTask` 增加 revision 参数；Dialog 输入条；TaskCard / TaskDetailDialog 接线；重新生成确认。
4. `activity.json` zh-CN/en + `tasks.json` 新文案 + `locale.test.ts`。
5. 门禁。

## Risky files

- `src-tauri/src/codex/process/ai_commands.rs` — 生成与修订共用，勿打坏首次生成
- `src-tauri/src/native/session.rs` — 只改 read-only one-shot，勿碰 live session
- `CoordinatorPlanDialog.tsx` — 按钮已挤，输入条放 footer 上方

## Validation

```
npm run test:ci
npm run format:check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml ai_commands
cargo test --manifest-path src-tauri/Cargo.toml native::session
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

桌面冒烟：看板打开协调员计划 → 修改计划 → 工作包更新；重新生成需确认；创建并运行不走修订。

## Rollback

去掉 payload 新字段即可回退（serde default）。transcript 多写无害。
