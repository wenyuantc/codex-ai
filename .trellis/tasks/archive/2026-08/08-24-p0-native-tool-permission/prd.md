# PRD · P0-2a native 高风险工具权限确认

父任务：`08-24-engine-ecosystem` · 优先级 P0

建议在 P0-1 之后实现：MCP 动态工具默认按高风险（或「未分类=高风险」）走同一确认通道。

## Goal

内置 Agent 不再对删除 / 覆盖 / 推送 / 强制 git **静默 yolo**。高风险动作执行前必须用户可见确认；拒绝则该次工具失败，会话继续。低风险读写保持直接执行。

## 证据

- 身份约定只要求「先说明风险」，同时写明 yolo：`src-tauri/src/native/prompt/identity.md:9,14`
- 环境块写死 `Permission mode: yolo`：`src-tauri/src/native/prompt/mod.rs:63`
- `execute_tool` 无确认钩子：`src-tauri/src/native/tools/dispatch.rs:29-49`
- Write 仅「先 Read 再写」，不是用户确认：`dispatch.rs:76-101`
- Bash 可跑 `rm -rf` / `git push --force`，无分类
- 产品主线「可信」：上波 A2 已要求运行前对跳过图片说真话；工具侧仍是反例

## Requirements

1. **高风险定义（MVP）**
   - 覆盖已存在文件（Write 目标已存在；Edit 也算覆盖）
   - 删除文件/目录（Bash 检测 `rm` / `rmdir` / `git rm` 等；若有未来 Delete 工具同样适用）
   - 推送与强制 git（`git push`、`git push --force`、`git reset --hard`、`git checkout --` 丢改动）
   - MCP 工具：默认高风险，除非后续白名单（本波可全部确认）
2. **确认 UX**：会话运行中弹出阻塞对话框（任务终端/会话 UI 可见），展示引擎、工具名、路径或命令摘要、风险类型、本地/远程。三个按钮，不可省略：
   - **本会话全部允许**：执行本次，并在该 `session_record_id` 内存里记住，后续高风险不再问
   - **仅允许一次**：只执行本次，下一次高风险再弹
   - **不允许**：本次不执行，返回工具错误给模型，不记住放行
3. **默认不放行**：点不允许、关闭对话框、停止会话、超时（若做超时）= 不允许。禁止自动允许。新会话 / 重启进程后记忆清空。
4. **记忆范围**：全部允许作用于**当前会话全部高风险种类**（覆盖/删除/推送/强制 git/MCP），不按路径 glob 细分，不写 SQLite/设置文件。本波不做跨会话白名单。
5. **本地与 SSH 同一套确认**；SSH 文案标明「将在远程工作区执行」（含远端 MCP 工具，不因 MCP 在 SSH 管道上而跳过确认）。
6. **低风险**（Read/Glob/Grep/Todo*/WebFetch/WebSearch、创建新文件的 Write）不弹窗。
7. 系统提示词改为「高风险需用户确认，低风险直接执行」，去掉「当前权限为 yolo」或把它改成与真实行为一致。
8. 确认事件写入会话日志（允许/拒绝）。若新活动 key，进仪表盘中文。
9. 单测：分类器覆盖典型命令；拒绝路径不写文件；允许路径写文件。可用 mock 确认通道，不必起 UI。

## Acceptance Criteria

- [x] native 覆盖已有文件前 UI 出现三选确认；不允许后文件内容不变
- [x] `git push --force` / `rm` 类 Bash 前出现确认
- [x] 「仅允许一次」后，下一次高风险再次弹窗
- [x] 「本会话全部允许」后，同会话后续高风险不再弹窗并直接执行
- [x] 新会话不会继承上一会话的「全部允许」
- [x] Read / Grep / 新建文件不弹窗
- [x] 用户不允许后模型收到失败原因，循环可继续（不整段崩）
- [x] SSH 项目同样阻塞确认，文案含远程
- [x] 身份/环境提示与真实权限模式一致（不再写全 yolo）
- [x] zh-CN + en；clippy / 相关 cargo test / format:check / test:ci / build 通过

## Out of Scope

- 完整权限矩阵（ask/allow/deny 三级 + 路径 glob 规则编辑器）
- 跨会话/设置页白名单；按风险类型分别「本会话允许覆盖但不允许推送」（本波全部允许=所有高风险）
- 子 Agent、plan 模式、Skills
- 对 Codex/Claude/Grok/OpenCode CLI 的 `bypassPermissions` 动手（Claude CLI 仍是 `--permission-mode bypassPermissions`，本任务只改 native）
