# Design · P0-2a native 高风险工具权限确认

## 边界

只改 **native 工具执行路径**。不改四引擎 CLI 的 permission-mode。确认通道是产品 UI，不是终端 stdin。

用户已锁定对话框三选：本会话全部允许 / 仅允许一次 / 不允许。

## 数据流

```
execute_tool(name, args)
  → classify_risk → Low | High { kind, summary }
  → Low: 直接执行
  → High + session.allow_all_high_risk: 直接执行（仍写会话日志「沿用本会话全部允许」）
  → High: emit native-permission-request
       ← UI: allow_session | allow_once | deny
  → allow_session: 置内存标志后执行
  → allow_once: 执行，不置标志
  → deny / 关闭 / stop: 返回「用户不允许该高风险操作：…」
```

## 合约

### 分类

纯函数 `classify_native_tool_risk(name, args) -> Risk`。

- Write 已存在 → High(overwrite)
- Edit → High(overwrite)
- Bash：按管道/分号切分后看 token
  - High(delete): rm, rmdir, git rm
  - High(push): git push
  - High(force_git): --force / reset --hard / checkout -- / restore --worktree
- MCP 动态工具：High(mcp)
- 其余内置：Low

宁可误报，不可漏报删除/推送。

### 会话记忆

`NativeLiveSession` 增加 `allow_all_high_risk: bool`（默认 false）。

- 只活在该 `session_record_id` 的进程内条目
- `remove_session` / 停止 / 进程退出即丢
- 不写 SQLite、不写 settings JSON
- 「全部允许」覆盖所有 High kind，本波不做 per-kind 开关

### 确认通道

- 后端 oneshot + `request_id`
- 前端 Dialog 三个按钮 + 关闭=deny
- 未决请求在 stop 时全部 deny
- 命令建议：`resolve_native_tool_permission { session_id, request_id, decision }`，`decision` = `allow_session | allow_once | deny`

### 提示词

去掉 yolo。改为：高风险会弹确认，用户可选本次、本会话全部、或不允许；被拒后换方案。

`Permission mode: confirm-high-risk`。

### SSH

同一 Dialog；文案前缀「远程工作区」。MCP 在 `tools/call` 前确认。全部允许对远端 MCP 同样生效（仍只限本会话）。

## 回滚

分类全 Low；去掉 Dialog。提示词一并回滚。
