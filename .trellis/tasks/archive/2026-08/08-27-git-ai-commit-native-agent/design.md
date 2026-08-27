# 设计：Git AI 提交信息支持内置 Agent（本地 Agent）

## 数据流

```
GitAutomationSettingsTab（内置 Agent + AI 渠道下拉）
  → SettingsPage 保存 git_preferences.ai_commit_native_channel_id
  → codex/settings.rs merge_git_preferences（JSON 设置文件，本地 + remote profile）
  → ai_generate_commit_message（提交对话框）
  → generate_commit_message_for_project（ai_commands.rs）
      effective_provider == "native"
  → run_commit_message_native（新私有 helper）
  → crate::native::run_native_one_shot_via_channel（本地 HTTP，无工具循环）
  → 返回提交信息 → 活动日志（provider=内置 Agent）
```

## 后端

### 1. `db/models.rs`

- `GitPreferences` 增加 `ai_commit_native_channel_id: Option<String>`。
- `UpdateGitPreferences` 增加 `ai_commit_native_channel_id: Option<Option<String>>`（`deserialize_explicit_nullable`，镜像 `one_shot_native_channel_id`）。

### 2. `codex/settings.rs`

- `RawGitPreferences` 增加同名字段（`#[serde(default)]`）→ 旧设置文件兼容。
- `default_git_preferences()` → `None`。
- `normalize_git_preferences` / `normalize_raw_git_preferences` → `normalize_optional_text`（trim，空串→None）。
- `merge_git_preferences` → 应用更新（`normalize_optional_text`）。
- `git_preferences_changed` → 纳入比较（触发设置变更活动日志）。
- `validate_git_preferences` → `ai_commit_model_source == "custom" && provider == "native"` 且渠道为空 → `Err("Git AI 使用内置 Agent 时请先选择 AI 渠道")`。
- `format_ai_preferred_provider_label` → `"native" => "内置 Agent"`。
- `format_git_preferences_activity_details` → native 时附带渠道 id。

### 3. `codex/process/ai_commands.rs`

- `CommitMessageAiSelection` 增加 `native_channel_id: Option<String>`：
  - custom → `git_preferences.ai_commit_native_channel_id`
  - inherit_one_shot → `settings.one_shot_native_channel_id`
- `generate_commit_message_for_project`：`effective_provider == "native"` 时，首次与重试都走新私有 helper：

```rust
async fn run_commit_message_native<R: Runtime>(
    app: &AppHandle<R>,
    channel_id: &str,
    model: &str,
    reasoning_effort: &str,
    prompt: String,
) -> Result<String, String> {
    let shot = crate::native::run_native_one_shot_via_channel(
        app, channel_id, model, reasoning_effort, prompt, None,
    )
    .await?;
    Ok(shot.text)
}
```

- 渠道缺失 → `Err("Git AI 使用内置 Agent 时请先选择 AI 渠道")`。
- CLI 提供商保持走 `run_ai_command` 不变；`one_shot.rs` 的 `resolve_native_one_shot_employee_id` 报错逻辑保留（其他调用方仍要求员工绑定）。

### 4. 测试

- fixture 更新（编译期）：`git_workflow/tests.rs`、`app/tests/runtime_and_paths.rs`、`settings.rs` 测试、`ai_commands.rs` 的 `test_settings` 中所有 `GitPreferences { ... }` 字面量。
- 新增单测：
  - settings：`ai_commit_native_channel_id` 归一化（trim/保留）、merge、custom+native 无渠道校验失败。
  - ai_commands：`resolve_commit_message_ai_selection` custom native / inherit native 的 channel 解析。

## 前端

### 1. `src/lib/types.ts`

- `GitPreferences` 增加 `ai_commit_native_channel_id: string | null`。
- 删除 `CLI_AI_PROVIDER_OPTIONS`（仅 `GitAutomationSettingsTab` 使用）及注释；改用 `AI_PROVIDER_OPTIONS`。
- `normalizeCliAiProvider` 若不再被引用则删除（`SettingsPage` 改用 `normalizeAiProvider`）。

### 2. `src/pages/SettingsPage.tsx`

- 新增 `aiCommitNativeChannelId` state。
- `applySettingsToFormState`：`setAiCommitNativeChannelId(gitPreferences.ai_commit_native_channel_id ?? "")`；git 提供商改用 `normalizeAiProvider`（不再钳制 native）。
- 保存 payload `git_preferences` 增加 `ai_commit_native_channel_id: aiCommitNativeChannelId.trim() || null`。
- 向 `GitAutomationSettingsTab` 传 `aiCommitNativeChannelId` / `nativeChannels`（已加载）/ `onAiCommitNativeChannelIdChange`。

### 3. `src/components/settings/GitAutomationSettingsTab.tsx`

- `providerOptions` 改用 `AI_PROVIDER_OPTIONS`。
- 新 props：`aiCommitNativeChannelId: string`、`nativeChannels: AiChannel[]`、`onAiCommitNativeChannelIdChange: (value: string) => void`。
- `isGitNativeProvider = isGitAiCustom && gitAiProvider === "native"`。
- custom + native 时显示渠道 Select（镜像 `RuntimeSettingsTab` 899-938 行）：
  - `onValueChange` → 渠道 + `selectNativeModel` 联动模型 + `resolveNativeThinking` 联动推理强度。
  - 无渠道时显示 `noChannelHint`。
- 模型下拉：native 时用选中渠道的 models（`nativeModelOptions`）。
- 推理下拉：native 时用 `resolveNativeThinking(selectedChannel, model).levels`。

### 4. 语言包

- `git.options.providers.native`：`内置 Agent` / `Built-in Agent`。
- `git.gitAi` 增加 `channelLabel` / `selectChannel` / `noChannelHint`（镜像 `runtime.oneShot` 文案）。

## 边界与失败模式

| 场景 | 行为 |
|------|------|
| 渠道被删除/停用 | `load_native_client_from_channel`（`native/session.rs:343`）报「渠道「X」已停用」/不存在，提交对话框展示错误 |
| 旧设置文件无新字段 | serde default `None`，行为不变 |
| 一次性 AI 未选渠道 + Git 跟随一次性 AI | 现有错误「一次性 AI 使用内置 Agent 时请先选择 AI 渠道」保持 |
| SSH 项目 | remote profile 的 git_preferences 独立保存渠道；native 执行在本机（与一次性 AI SSH 语义一致） |
| 其他 run_ai_command 调用方 | 不变（计划生成/验收清单仍走员工绑定 native 路径） |

## 兼容性 / 回滚

- 纯增量：新字段可选，旧设置文件与旧前端均不受影响。
- 回滚：移除新字段与 UI 分支即可，无数据迁移。
