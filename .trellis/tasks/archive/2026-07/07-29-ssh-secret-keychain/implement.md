# 执行计划 — SSH 密钥迁移至 OS keychain

前置：`prd.md` / `design.md` 已评审，`task.py start` 后 status = `in_progress`。

分支：`feat/ssh-secret-keychain`（父任务：独立分支 + 单独提交）

基线：实现前记录 `cargo test` 通过数（C1 后约 **262**），完成后不得低于基线。

## 步骤

### S1 分支与基线 `[无回滚点]`

```bash
git checkout main
git pull  # 若适用；本地可跳过
git checkout -b feat/ssh-secret-keychain
cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -8
```

### S2 依赖 `keyring` `[回滚点 R1]`

`src-tauri/Cargo.toml` 增加：

```toml
keyring = "3"
```

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

### S3 抽象 `SecretBackend` + Memory 实现 `[回滚点 R2]`

文件：`src-tauri/src/codex/secret_store.rs`

- `trait SecretBackend`（set/get/delete）
- `struct MemoryBackend`
- `struct KeyringBackend { service: &'static str }` → `"codex-ai-ssh"`
- 错误信息中文；keyring `NoEntry` / not found → get 返回 `Ok(None)` 或 delete 幂等

**验证**：可编译（允许未接线）。

### S4 索引 v2 + 迁移 `[回滚点 R3]`

- 常量：`SECRET_INDEX_FILE_NAME = "ssh-secret-index.json"`，保留 `SECRET_STORE_FILE_NAME = "ssh-secrets.json"` 仅迁移源
- `SecretIndexDocument { version, entries: HashMap<ref, Meta> }` — Meta **无 value**
- `ensure_migrated(app, backend)`：读旧 JSON → set keyring → 写索引 → 删旧文件；失败保留旧文件
- 索引读写 + Unix 0600

### S5 重写公开 API 走 backend+index `[回滚点 R4]`

保持 `store_secret_value` / `resolve_secret_value` / `delete_secret_value` / `sweep_orphan_secret_refs` 签名。

入口统一 `ensure_migrated`。生产路径构造 `KeyringBackend`。

**验证**：

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

确认 `remote.rs` 无需改调用签名。

### S6 单元测试 `[回滚点 R5]`

`#[cfg(test)]` 使用 `MemoryBackend` + 临时目录模拟 config 路径（若 API 强依赖 `AppHandle`，则：

- 优先把纯逻辑抽到 `store_secret_value_with` / `migrate_document` 等不依赖 Tauri 的函数；或
- 仅测 `MemoryBackend` + `migrate_from_legacy_document` + index serde 无 value

最低覆盖：

1. 往返 store/resolve  
2. delete  
3. replace_ref 删除旧值  
4. sweep 孤儿  
5. 迁移后 legacy 条目在 backend 可读、序列化索引 JSON 不含明文密码  

```bash
cargo test --manifest-path src-tauri/Cargo.toml secret
```

### S7 全量验证

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml
npm run build
```

### S8 实机（审查门，可用户执行）

```bash
npm run tauri:dev
```

1. 新建密码认证 SSH 配置 → 确认 **无** `$APPCONFIG/ssh-secrets.json` 或其中无 `value`；存在 `ssh-secret-index.json` 且无密码明文  
2. macOS「钥匙串访问」或等价处可见 `codex-ai-ssh` 条目（若系统允许查看）  
3. 远程密码认证会话/git 仍可用  
4. 若本地仍有旧 `ssh-secrets.json`：启动一次相关操作后文件应消失，密码仍可用  

### S9 收尾

提交并合回 `main`。更新 `.trellis/spec/backend/ssh-remote.md`（或 secret 专节）记录 at-rest keychain 合约。

## 回滚点

| 标记 | 状态 | 回退 |
|---|---|---|
| R1 | 仅依赖 | 还原 Cargo.toml / Cargo.lock |
| R2 | trait+memory | 还原 secret_store 增量 |
| R3 | 迁移逻辑 | 同上 |
| R4 | API 已切 keyring | 整文件回退；用户已迁则需重录密码 |
| R5 | 测试 | 保留实现删测试或整回退 |

全任务：`git branch -D feat/ssh-secret-keychain`（未合并前）。

## 验证命令汇总

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml secret
cargo build --manifest-path src-tauri/Cargo.toml
npm run build
npm run tauri:dev   # S8
```

## 完成后

- 父任务地图 C1b 勾选完成；journal 记 keychain 落地  
- Spec：ssh-remote / error-handling 补充 at-rest 与禁止明文回退  
