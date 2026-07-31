# 修复 Claude stream-json 缺少 --verbose 导致任务/审核失败

## Goal

修复 Claude CLI 启动参数不完整，使「运行任务」和「审核任务」在新版 Claude CLI 下可正常建立会话并完成流式输出。

## Background

新版 Claude CLI 约束：使用 `--print`（`-p`）且 `--output-format=stream-json` 时，**必须**同时传入 `--verbose`，否则立即失败：

```text
[STDERR] Error: When using --print, --output-format=stream-json requires --verbose
[ERROR] Claude 会话已结束，退出码: 1
```

应用侧 `build_claude_cli_args` 已使用 `-p` + `stream-json`，但未加 `--verbose`，导致 Claude 引擎的任务运行与审核会话均无法启动。

## Requirements

1. Claude CLI 会话启动参数在 `-p` + `--output-format stream-json` 组合下必须包含 `--verbose`。
2. 本地与 SSH 远程启动路径共用同一套 `build_claude_cli_args` 结果，两处行为一致。
3. 现有 effort / system-prompt / resume 等参数行为不变。
4. 补充或更新单元测试，断言 CLI args 含 `--verbose`。
5. 不改变前端协议、数据库 schema、任务状态机或其他引擎（Codex/Grok/OpenCode）参数。

## Out of Scope

- Claude SDK bridge 路径的行为调整（仅当 CLI 回退路径受影响时一并覆盖）
- 升级/锁定用户本机 Claude CLI 版本
- 其他引擎的 stream/json 输出格式改动

## Acceptance Criteria

- [x] `build_claude_cli_args` 生成的参数包含 `--verbose`
- [x] 相关单元测试通过（含 verbose、effort、resume、remote command 等现有断言）
- [ ] 使用 Claude 引擎「运行任务」时，不再出现 `stream-json requires --verbose` 错误（需本机手工冒烟）
- [ ] 使用 Claude 引擎「审核任务」时，不再出现同上错误（需本机手工冒烟）
- [x] 本地与 SSH 项目启动 Claude 会话时参数一致包含 `--verbose`（共用 `build_claude_cli_args` + remote command 测试）

## Implementation Notes

- 改动文件：`src-tauri/src/claude/process/mod.rs`
- 在 `build_claude_cli_args` 的 `stream-json` 后追加 `--verbose`
- 单元测试：`claude_cli_args_*`、`remote_claude_session_command_uses_shell_bootstrap` 已断言 `--verbose`
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml claude_cli_args` 与 `remote_claude_session` 均通过

## Notes

- 轻量任务：PRD-only，无需 `design.md` / `implement.md`
