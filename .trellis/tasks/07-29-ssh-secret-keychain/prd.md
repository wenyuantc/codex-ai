# SSH 密钥迁移至 OS keychain

父任务：`.trellis/tasks/07-29-codex-ai-optimization`  
来源：C1 规划期衍生发现（高于原源发现 #7）

## Goal

将 SSH 密码从 `$APPCONFIG/ssh-secrets.json` 明文 JSON 存储，迁移至操作系统凭据库（OS keychain / Credential Manager / Secret Service），消除明文落盘与 Windows 上无文件权限收紧的安全缺陷。

## Background

`src-tauri/src/codex/secret_store.rs` 当前以明文 JSON 持久化 SSH 密码：

- 文件名：`ssh-secrets.json`（`SECRET_STORE_FILE_NAME`）
- Unix：`tighten_secret_store_permissions` 设 0600
- Windows：`#[cfg(not(unix))] let _ = path;` —— **不做任何权限收紧**

能读取该文件的进程即获得全部已保存的 SSH 密码。C1 已判定：在此威胁模型下，运行时 `CODEX_SSH_SECRET` 环境变量通道不构成**新增**攻击面；真正的安全水位问题是**持久化明文存储**，由本任务处理。

## 前置条件

- **建议在 C1（`07-29-ssh-multiplex-secret`）合回 `main` 之后再实现**，避免与 `remote.rs` / secret 读取路径的并发冲突。
- 不依赖 C2–C7。

## Requirements

### R1 — 凭据读写走 OS 凭据库

SSH 密码的 save / load / delete 必须写入 OS 凭据库（macOS Keychain、Windows Credential Manager、Linux Secret Service 等），不再以明文写入 `ssh-secrets.json`。

### R2 — 一次性迁移

升级后首次访问 secret store 时，若仍存在旧版 `ssh-secrets.json`：

1. 将其中条目迁入 OS 凭据库；
2. 迁移成功后删除或清空旧文件，避免明文残留；
3. 迁移失败不得静默丢弃用户密码——须可诊断、可重试。

### R3 — 调用面兼容

现有通过 `secret_store` 读写密码的路径（含 `remote.rs` 密码认证、`sweep_ssh_secret_store` 等）行为不变：密码仍可正确注入 `SSH_ASKPASS` / `CODEX_SSH_SECRET` 通道（C1 已决定保留该运行时通道）。

### R4 — 跨平台

macOS / Linux / Windows 三条路径均可工作。Linux 无可用 Secret Service 时须有明确降级或错误提示策略（在 `design.md` 定夺，不得静默回退到明文 JSON）。

### R5 — 测试

对 secret store 抽象层提供可测接口（mock 或内存后端），覆盖：写入、读取、删除、从 JSON 迁移、迁移后旧文件不再含明文密码。

## Acceptance Criteria

- [ ] 新保存的 SSH 密码不以明文出现在 `$APPCONFIG/ssh-secrets.json`（文件不存在或为空/无 secrets 字段）
- [ ] 已有 `ssh-secrets.json` 用户升级后可自动迁移，迁移后旧文件无明文密码残留
- [ ] 密码认证的 SSH 远程项目在 macOS（必测）与文档声明支持的平台上功能不回归
- [ ] Windows 上不再依赖「仅靠文件 ACL」保护明文密码文件
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` 全绿，测试数不低于基线
- [ ] 独立分支 + 单独提交合回 `main`（父任务交付约束）

## Out of Scope

- 不替换 `ssh` 子进程模型 / 不引入 `russh`
- 不改动 C1 已定的运行时 `CODEX_SSH_SECRET` 传递策略（消除 env 属另一议题）
- 不迁移非 SSH 类密钥（若未来有其他 secret 类型，另开任务）
- 不做 UI 视觉改版；若需用户授权 keychain 访问提示，仅做最小必要文案

## Notes

- 交付方式：独立分支 + 单独提交
- 本任务为 complex：启动实现前须补齐 `design.md` + `implement.md`
- 执行顺序：父任务地图中排在 C1 之后（见父 `prd.md`）
