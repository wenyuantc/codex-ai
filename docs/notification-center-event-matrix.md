# 通知中心事件矩阵

## 最小模型

通知中心当前统一使用如下字段：

- 类型：`review_pending`、`run_failed`、`run_completed`、`task_completed`、`sdk_unavailable`、`database_error`、`ssh_config_error`
- 严重级别：`info`、`success`、`warning`、`error`、`critical`
- 展示形态：`one_time`、`sticky`
- 状态：`active`、`resolved`
- 文案字段：`title`、`message`、`recommendation`
- 来源字段：`source_module`
- 关联对象：`related_object_type`、`related_object_id`、`project_id`、`task_id`、`ssh_config_id`
- 行为字段：`action_label`、`action_route`
- 时间字段：`first_triggered_at`、`last_triggered_at`、`read_at`、`resolved_at`、`created_at`、`updated_at`
- 去重字段：`dedupe_key`、`occurrence_count`

数据库持久化表为 `notifications`。当数据库本身不可用时，系统退回 `TransientNotification` 事件流，仅做前端临时提醒，不依赖数据库写入。

## 生命周期

1. 创建：一次性通知用 `publish_one_time_notification`，持续告警用 `ensure_sticky_notification`。
2. 展示：前端通过 `list_notifications` 拉取持久化通知，并监听 `notification-center-changed` / `notification-center-transient`。
3. 已读：用户可单条已读或全部已读，对应 `mark_notification_read`、`mark_all_notifications_read`。
4. 重新激活：同一 `dedupe_key` 的 sticky 告警再次出现时复用原记录，重置为未读并累加 `occurrence_count`。
5. 关闭：`resolve_sticky_notification` 将 sticky 告警改为 `resolved`，必要时补发一条恢复类 `one_time` 通知。

## 事件矩阵

| 类型 | 谁触发 | 何时触发 | 用户看到什么 | 严重级别 / 形态 | 去重策略 | 点开后去哪里 | 恢复策略 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 审核待处理 | 任务状态更新 | 任务状态进入 `review` | “有任务进入待审核”，含任务标题与处理提示 | `warning` / `one_time` | 不做 sticky 去重，依赖状态切换边界触发 | `/kanban?taskId={taskId}` | 无 |
| 运行失败 | 会话退出处理 | 执行类会话异常退出且状态不是正常 `exited` | “任务运行失败”，含任务标题与失败摘要 | `error` / `one_time` | 不做 sticky 去重，依赖单次失败事件 | `/kanban?taskId={taskId}` | 无 |
| 运行完成 | 会话退出处理 | 普通任务的执行类会话成功退出，且任务未开启自动质控 | “任务运行完成”，含任务标题与查看建议 | `success` / `one_time` | 不做 sticky 去重，依赖单次成功退出事件 | `/kanban?taskId={taskId}` | 无 |
| 任务完成 | 任务状态更新 | 任务从非 `completed` 进入 `completed` | “任务已完成”，含任务标题与后续查看建议 | `success` / `one_time` | 不做 sticky 去重，依赖完成状态跃迁 | `/kanban?taskId={taskId}` | 无 |
| SDK 不可用（本地） | 本地健康检查 / 系统同步 | 本地启用 SDK 但实际 provider 退回非 `sdk` | “本地 SDK 当前不可用”，含修复建议 | `warning` 或 `error` / `sticky` | `sdk_unavailable:local` | `/settings?section=sdk` | 恢复后发“本地 SDK 已恢复可用” |
| SDK 不可用（远程） | 远程健康检查 / 系统同步 | 远程配置启用 SDK 但远程 provider 退回非 `sdk` | “远程主机的 SDK 当前不可用”，含主机名与修复建议 | `warning` 或 `error` / `sticky` | `sdk_unavailable:ssh:{sshConfigId}` | `/settings?section=sdk&sshConfigId={sshConfigId}` | 恢复后发“远程 SDK 已恢复” |
| 数据库异常 | 数据库连接 / 迁移状态检查 | 连接失败、迁移状态读取失败 | “数据库连接异常”或“数据库迁移状态异常” | `critical` / `sticky`；连接失败时退回 `transient` | `database_error:local` | `/settings?section=database` | 恢复后发“数据库连接已恢复” |
| SSH 配置缺失 | 系统同步 | 当前处于 SSH 模式，但没有选中或可用 SSH 配置 | “SSH 配置缺失” | `warning` / `sticky` | `ssh_config_error:missing_selection` | `/settings?section=ssh` | 退出 SSH 模式或补齐配置后关闭 |
| SSH 配置不可读 | 系统同步 | 当前选中 SSH 配置无法加载 | “选中的 SSH 配置不可用” | `error` / `sticky` | `ssh_config_error:selected:{sshConfigId}` | `/settings?section=ssh&sshConfigId={sshConfigId}` | 配置恢复可读后关闭 |
| SSH 认证异常 | SSH 密码探测 / 系统同步 | 密码认证探测失败或不支持 | “SSH 配置认证异常” | `warning` 或 `error` / `sticky` | `ssh_config_error:{sshConfigId}` 或 `ssh_config_error:password_probe:{sshConfigId}` | `/settings?section=ssh&sshConfigId={sshConfigId}` | 探测恢复后发“SSH 密码认证校验已恢复” |
| SSH 连通性异常 | 远程健康检查 / 系统同步 | 远程主机校验失败、连接失败、环境异常 | “SSH 连接校验失败”或“SSH 主机连接异常” | `error` / `sticky` | `ssh_config_error:health:{sshConfigId}` | `/settings?section=ssh&sshConfigId={sshConfigId}` | 恢复后发“SSH 连接已恢复”或“SSH 主机已恢复正常” |

## 扩展约束

- 时间展示统一走前端 `formatDate()`。
- 新增/恢复通知会写入最近活动日志，动作 key 为 `notification_created`、`notification_resolved`。
- 仪表盘当前额外展示 `未读通知` 与 `高优先级告警`，中文 key 已落地。
- 后续新增通知类型时，优先复用 `NotificationDraft`、`ensure_sticky_notification`、`publish_one_time_notification`，不要在业务模块直接写表。
