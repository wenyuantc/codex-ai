# Design · P0-3 Claude CLI 本地图片补齐

## 已锁定

方案 I：补齐本地 CLI 传图。SSH 仍跳过。

## 通道选择（2026-08-24 本机 Claude CLI 2.1.221）

| 候选 | 结论 |
|---|---|
| `--image` | **不存在**，禁止使用 |
| prompt 里写本地路径 | 弱；模型可能 Read 文件但不等于 vision 输入 |
| `--input-format stream-json` + Anthropic image content block | **推荐**：与 `claude_sdk_bridge.mjs` 的 `type:image / source.base64` 同构；CLI 已支持 stream-json stdin（`--print`） |

当前 CLI 启动：`-p --output-format stream-json --verbose --permission-mode bypassPermissions`，stdin 写纯文本后 shutdown。补齐后：加 `--input-format stream-json`，stdin 写一帧（或最少必要帧）user message，再关闭 stdin（批处理）或按现有 follow-up 策略保留。

探测（实现第一步，写入测试夹具/注释）：

```text
claude -p --output-format stream-json --input-format stream-json --verbose ...
stdin: {"type":"user","message":{"role":"user","content":[{"type":"text","text":"..."},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"..."}}]}}
```

若官方帧字段名不同（例如必须 `stream_event` 包装），以探测为准，不要硬套 SDK bridge 的 generator 形状。

## 启动路径改动

`claude/process/mod.rs` CLI 分支：

1. 去掉「有图就 WARN 并当 0 张」的短路径
2. 读本地文件 → base64 + mime（可抽与 native/Grok 类似的小函数，避免复制三份 mime）
3. 构造 stream-json 帧；无图时保持今日文本 stdin（或统一走 stream-json 纯 text，需回归无图会话）
4. `format_claude_session_prompt_log` CLI 分支传入真实 `image_paths`
5. SSH CLI：`prepare_execution_image_paths` 已忽略远程图；保持 skip + 现有 WARN/A2

不要为了图片去改 SDK bridge，除非发现帧可完全复用——CLI 是另一个进程协议。

## 前端

`resolveImageAttachmentSkip`：删除（或收窄）`provider===claude && claudeEffectiveProvider!=='sdk'` → `claude_cli`。仅 SSH 仍 `ssh_claude`。

能力矩阵：Claude notes「本地 CLI/SDK 可附带图片；SSH 跳过」。若加 `images` bool，Claude 应为 true 并靠 notes 说明 SSH。更细可用 notes-only，避免 `images=true` 被理解成 SSH 也能传。

## 失败

单张 `read` 失败：该张 missing，其余继续。CLI 立刻因图片 400 退出：会话失败原因要含「图片输入」，不要只说 spawn_failed。

## 回滚

恢复文本 stdin + WARN skip；前端恢复 `claude_cli` skip。SDK 不受影响。
