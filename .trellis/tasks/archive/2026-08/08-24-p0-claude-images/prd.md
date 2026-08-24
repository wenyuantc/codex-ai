# PRD · P0-3 Claude 图片声明/补齐

父任务：`08-24-engine-ecosystem` · 优先级 P0

建议放在 P0-1 / P0-2a 之后。用户已锁定：**补齐 Claude CLI 本地传图**（方案 I），不是只声明。

## Goal

Claude **本地 CLI** 带图任务要真正把图片交给模型，和 SDK 本地通道对齐。SSH Claude 继续跳过并保持运行前警告。能力矩阵与员工绑定必须和真实通道一致，禁止「界面说能看图、CLI 实际丢掉」。

## 证据

- CLI 跳过：`src-tauri/src/claude/process/mod.rs:1240-1250`；stdin 只写纯文本 prompt（`:1256-1275`）
- CLI args 无图片：`build_claude_cli_args`（`claude/process/mod.rs:130-163`）只有 `-p --model --output-format stream-json --verbose --permission-mode bypassPermissions`
- 本机 Claude CLI **2.1.221** `--help` **没有** `--image`（与社区 issue「--image flag does not exist」一致）。有 `--input-format text|stream-json`（仅 `--print`）
- SDK 已传图：`imagePaths` → `claude_sdk_bridge.mjs:139-166`（text + `type:image` base64 source）
- 运行前警告：`src/lib/imageAttachmentSkip.ts` 对本地 `claude` 且非 sdk 返回 `claude_cli`；测试 `:45-68`
- 能力矩阵无图片字段：`AiProviderCapabilities`
- Codex 本地用 `--image`；Grok 本地用 ACP JSON image block；native ≤8 张 base64

## Requirements

1. **本地 Claude CLI 必须附带图片**：对已 prepare 的本地 `image_paths`，把图片作为模型可见输入发出。推荐路径：`--input-format stream-json` 写入与 SDK 同构的 user message（text + image base64 content blocks）。禁止发明不存在的 `--image`。实现阶段用当前 CLI 做一次最小探测；协议若变，以探测结果为准并更新本 PRD 技术注记。
2. **失败可见**：读文件失败、CLI 拒收、400 空图等不得假装已看图。会话 `[WARN]` 写清跳过/失败张数与原因；能附带的继续附带。
3. **SDK 本地不回退**：现有 bridge 路径保持。
4. **SSH Claude 仍跳过**（CLI 与 SDK 远程通道都不做上传）。A2 运行前 `ssh_claude` 确认保留。
5. **`imageAttachmentSkip`**：本地 Claude CLI 成功路径不再预警告「会跳过」。SSH Claude / Grok SSH 等其他原因不变。
6. **能力矩阵 / 员工绑定**：Claude 注明「本地 SDK+CLI 支持图片；SSH 跳过」。设置徽章与创建/编辑员工可见。
7. 提示词日志：CLI 成功附带时 `format_claude_session_prompt_log` 应列出图片名（今天 CLI 分支传空切片，`mod.rs:1438-1441`）。
8. 文案 zh-CN + en。相关单测更新（args、skip resolver、如有 JSON 帧构造）。

## Acceptance Criteria

- [x] 本地 Claude **CLI** 员工 + 图片附件：运行前不再弹出「CLI 会跳过图片」；会话中图片进入 CLI 输入（stream-json image block 或等价已核实通道）
- [x] 本地 Claude **SDK** 行为不变
- [x] 某张图读失败：WARN + 其余图仍可送；不把失败标成已附带
- [x] SSH Claude + 图片：运行前仍确认会跳过；不在本任务做远程传图
- [x] 员工选 Claude 时能看到「本地能看图 / SSH 不能」口径
- [x] Codex/OpenCode/native 本地不误报跳过
- [x] `imageAttachmentSkip.test.ts` 不再把「本地 Claude CLI」当作 skip；SSH Claude 仍 skip
- [x] format:check / test:ci / 相关 cargo test / clippy / build 通过

## Out of Scope

- SSH 传图（任何引擎，含 Claude SDK 远程）
- 给 Claude CLI 加 MCP 执行（P0-1 明确不做外部引擎 MCP）
- Grok send_input、改附件存储、native 8 张上限
