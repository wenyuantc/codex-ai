# Implement · 内置 Agent（父任务）

父任务不写产品代码。按序 `task.py start` 子任务。

## 顺序

1. `08-20-native-agent-channels` — 表/密钥/Settings。后续子任务依赖渠道存在。
2. `08-20-native-agent-model-client` — 可与 UI 并行，但测通命令会调用客户端，优先紧跟 1 或在 1 里用最小 HTTP 测通、2 再换成正式客户端。
3. `08-20-native-agent-loop-tools` — 依赖 2。
4. `08-20-native-agent-engine` — 依赖 1–3；接入 session/queue/pipeline/review。
5. `08-20-native-agent-ui` — 依赖 1 与 4；员工绑定与全页面。
6. 父任务集成验收 + spec 更新后 archive。

集成记录（2026-08-20）：五个子任务代码已合入工作区。`clippy -D warnings`、`npm run format:check`、`npm run test:ci`（102）、`cargo test`（450）通过。SSH 覆盖为 `native/tools/ssh.rs` 命令构造单测。桌面 `tauri:dev` 冒烟未跑。spec：`backend/ai-engines.md`、`directory-structure.md`，并同步 frontend / run-queue / 跨层 guide。

## 集成检查（父任务最后）

- native 员工创建 → 选渠道 → 启动任务 → 日志有文本/工具 → 停止 → resume
- SSH：工具走 `build_ssh_command`，无远端 agent 二进制
- 四引擎回归
- 更新 `.trellis/spec/backend/ai-engines.md`、`directory-structure.md`
- 质量门禁：clippy / format:check / test:ci / cargo test

## Rollback

渠道表为加表+加列，可留空不用。`ai_provider=native` 员工在引擎未完成前禁止启动（中文错误）。不要回改已发布的 v48 SQL。
