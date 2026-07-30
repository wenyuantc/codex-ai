# 技术设计 — SSH 密钥迁移至 OS keychain

## 边界

**唯一实现面**：`src-tauri/src/codex/secret_store.rs`（及 `Cargo.toml` 增加依赖）。

公开 API **签名不变**（调用方零改动）：

```rust
pub fn store_secret_value<R: Runtime>(app, value, replace_ref) -> Result<Option<String>, String>
pub fn resolve_secret_value<R: Runtime>(app, secret_ref) -> Result<Option<String>, String>
pub fn delete_secret_value<R: Runtime>(app, secret_ref) -> Result<(), String>
pub fn sweep_orphan_secret_refs<R: Runtime>(app, active_refs) -> Result<usize, String>
```

调用方：`app/remote.rs`（create/update/delete SSH config、`build_ssh_command` 密码注入、`sweep_ssh_secret_store`）。运行时仍通过 `CODEX_SSH_SECRET` 注入（C1 R5，本任务不改）。

## 现状问题

| 点 | 行为 |
|---|---|
| 落盘 | `$APPCONFIG/ssh-secrets.json` 明文 `entries[ref].value` |
| Windows | `tighten_secret_store_permissions` 空操作 |
| 索引+密文同文件 | 无法只靠 0600 在跨平台上对齐 OS 凭据库水位 |

## 设计

### D1 双层存储

| 层 | 存什么 | 位置 |
|---|---|---|
| **凭据层** | 密码明文 | OS keychain：`keyring` crate，`Entry::new(service, user)` |
| **索引层** | ref 元数据（**无 value**） | `$APPCONFIG/ssh-secret-index.json` |

**keyring 约定**：

- `service` = `"codex-ai-ssh"`（固定常量，全平台一致）
- `user` / account = `secret_ref`（形如 `ssh-secret-<uuid>`，与 SQLite `password_ref` / `passphrase_ref` 一致）

**索引文档**（替换旧 `SecretStoreDocument` 语义）：

```json
{
  "version": 2,
  "entries": {
    "ssh-secret-...": { "created_at": "...", "updated_at": "..." }
  }
}
```

- 新写入**禁止**出现 `value` 字段。
- 旧文件名 `ssh-secrets.json` 仅作迁移源，迁移成功后删除。

为何保留索引：

- `sweep_orphan_secret_refs` 需要「已存 ref 全集」；`keyring` 在各 OS 上**不能可靠 list 全部 entry**。
- 删除/替换需知道 ref 是否曾存在，避免无意义的 keyring 调用。

### D2 后端抽象（可测）

```rust
trait SecretBackend {
    fn set(&self, secret_ref: &str, value: &str) -> Result<(), String>;
    fn get(&self, secret_ref: &str) -> Result<Option<String>, String>;
    fn delete(&self, secret_ref: &str) -> Result<(), String>;
}
```

- 生产：`KeyringBackend`（`keyring::Entry`）
- 测试：`MemoryBackend`（`Mutex<HashMap>`），不触真实 keychain

`store/resolve/delete/sweep` 走 `with_backend` 或模块内 `fn backend() -> impl SecretBackend`；单测注入 memory。

### D3 API 行为映射

| API | 行为 |
|---|---|
| `store_secret_value(Some(v), replace)` | 若 replace：删旧 keyring+索引；生成新 ref；keyring set；索引写元数据 |
| `store_secret_value(None, replace)` | 仅删除 replace（清密码） |
| `resolve_secret_value(ref)` | 索引无此 ref → `Ok(None)`；有则 keyring get；keychain 缺项但索引有 → 中文错误（损坏态） |
| `delete_secret_value(ref)` | keyring delete（NoEntry 忽略）+ 索引移除 |
| `sweep_orphan_secret_refs` | 索引中不在 `active_refs` 的：delete keyring + 去索引 |

### D4 一次性迁移

触发：任意 `store` / `resolve` / `delete` / `sweep` 入口，在操作前调用 `ensure_migrated(app)`（幂等）。

```
if ssh-secrets.json exists:
  load entries (含 value)
  for each (ref, entry):
    backend.set(ref, value)
    index.entries[ref] = {created_at, updated_at}  // 丢弃 value
  save index
  delete ssh-secrets.json   // 或 rename 后删，失败则 Err 可重试
if only index exists: done
if neither: empty index
```

失败策略：

- 中途失败：**不删除**旧 JSON，返回 `Err("迁移 SSH 密钥到系统凭据库失败: …")`，可重试。
- **禁止**静默回退到继续读写明文 JSON 作为生产路径。

### D5 跨平台与 Linux 降级

| 平台 | keyring 后端 | 无服务时 |
|---|---|---|
| macOS | Security framework | 罕见；报错中文 |
| Windows | Credential Manager | 报错中文 |
| Linux | Secret Service (libsecret) | **禁止**回退明文；`Err("系统凭据服务不可用，无法保存 SSH 密码。请安装/启用 Secret Service（如 gnome-keyring）后重试")` |

依赖：`keyring = "3"`（Cargo.toml）。不引入 `russh` / 不改 SSH 子进程模型。

### D6 权限与旧路径

- 索引文件 Unix 仍 0600；Windows 索引无密钥，风险可接受。
- `tighten_secret_store_permissions` 仅作用于索引路径。
- 迁移后若磁盘仍出现带 `value` 的 `ssh-secrets.json` 视为 bug。

## 兼容与回滚

- **无 DB 迁移**，`password_ref` 字符串格式不变。
- **无 IPC / 前端改动**。
- 回滚：还原 `secret_store.rs` + 去掉 `keyring` 依赖；已迁用户需手动重录密码（写进 release note / 任务 Notes）。

## 测试策略

| 用例 | 方式 |
|---|---|
| store → resolve 往返 | MemoryBackend |
| delete / replace_ref | MemoryBackend |
| sweep 只删孤儿 | MemoryBackend |
| 迁移：JSON+value → index 无 value + backend 有值；源文件标记删除 | 临时目录 fixture + MemoryBackend |
| 生产 keyring | 可选 `#[ignore]` 集成测，默认 CI 不跑 |

不在单测中访问真实 macOS Keychain（避免交互弹窗 / CI 失败）。

## 非目标

- 不消除 `CODEX_SSH_SECRET` 运行时 env（C1 已定）。
- 不迁移非 SSH secret 类型。
- 不做 UI 改版；keychain 系统弹窗沿用 OS 默认。
