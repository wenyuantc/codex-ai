# Implement · 渠道 API 配置

1. 迁移 v48 + `db/models.rs` `AiChannel` DTO（无明文 key）。
2. 渠道密钥环 + channels CRUD + 测通最小 HTTP。
3. `lib.rs` 注册 command；`native/mod.rs` 接入。
4. 单测：protocol 校验、删除引用拦截、DTO 无 key 字段、迁移 48。
5. Settings Tab + i18n + activity 中英。
6. `backend.ts` wrappers。
7. clippy / format:check / cargo test 相关 / test:ci。

依赖：无。后续 model-client 替换测通实现细节，命令签名保持。
