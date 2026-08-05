# Implement — 测试员自动化闭环

## 检查清单（实现顺序）

1. [ ] Migration：验收 run 表 / 项目 `test_command` / 所需列
2. [ ] models + 设置读写（tester 开关、allow_ai_only、默认命令）
3. [ ] `run_task_acceptance` 内核：命令执行（local/SSH）+ hard fail
4. [ ] 清单生成/编辑 command + 复用 AI tester prompt
5. [ ] 可选 AI verdict 会话与结果解析
6. [ ] `task_automation` 新 phase + session_exit 先测后审挂钩
7. [ ] 与 coordinator pipeline 插入点
8. [ ] 活动日志 + 通知 + 中文 label
9. [ ] 前端：设置、任务详情、看板徽章、手动运行
10. [ ] Rust 单测 + build + clippy
11. [ ] 手工烟测 local/SSH

## 验证命令

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml tester
cargo test --manifest-path src-tauri/Cargo.toml automation
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## 高风险文件

- `src-tauri/src/task_automation/session_exit.rs`
- `src-tauri/src/task_automation/state.rs`
- `src-tauri/src/task_automation/pipeline.rs`
- `src/components/tasks/TaskDetailDialog.tsx`
- `src-tauri/src/db/migrations.rs`

## 回滚点

- 合并前可用设置默认 `tester_automation_enabled=false`
- phase 异常 → `manual_control` / 不阻塞人工拖拽

## 开始前

- 用户批准本子任务（或父路线图含本子任务的最终摘要）后：
  `python3 ./.trellis/scripts/task.py start 08-05-tester-automation-loop`
- 加载 `trellis-before-dev`
