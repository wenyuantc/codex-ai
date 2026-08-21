# Design · 渠道 API 配置

## Schema v48

```sql
CREATE TABLE ai_channels (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  protocol TEXT NOT NULL,
  base_url TEXT NOT NULL,
  api_key_ref TEXT,
  extra_headers_json TEXT,
  models_json TEXT NOT NULL DEFAULT '[]',
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_ai_channels_enabled ON ai_channels(enabled, name);
ALTER TABLE employees ADD COLUMN ai_channel_id TEXT REFERENCES ai_channels(id);
```

更新 `latest_migration_version` 断言 47 → 48。

## Secrets

渠道 API key 存在 `ai_channels.api_key`（SQLite 配置）。DTO 回传明文供设置页编辑/显示。

v50 增加 `api_key` 列。旧行若只有 `api_key_ref`，读路径从 keyring `codex-ai-channel` 迁入列后删除钥匙串条目。新建/更新不再写钥匙串。SSH 密码仍走 `codex/secret_store.rs`，禁止混用 service。

## Commands

| command | 说明 |
|---|---|
| `list_ai_channels` | 全部，含禁用 |
| `create_ai_channel` | 校验 protocol/url/name；写 key |
| `update_ai_channel` | Option 字段；空密钥=不改 |
| `delete_ai_channel` | 引用检查 + 删 key |
| `test_ai_channel` | 用已存或请求体里的临时 key |

错误：`Result<T, String>` 中文。

## UI

`src/components/settings/AiChannelsSettingsTab.tsx`，挂到 `SettingsPage` 现有 Tab 列表。复用 Dialog/Input/Select。模型列表用逗号或每行一个 id。

## Activity

`insert_activity_log`；`src/locales/zh-CN/activity.json` + `en/activity.json`。
