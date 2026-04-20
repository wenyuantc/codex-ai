- [ ] 看板自动化刷新了状态但是 运行按钮状态和审核按钮没有刷新
a


自动化审核 报错

审核结果结构化输出无效，自动质控已停止，需人工接管

<review_verdict>
{"passed":false,"needs_human":false,"blocking_issue_count":3,"summary":"发现 3 个阻断问题：1) SDK 不可用判定在健康检查与周期同步之间不一致，会把真实故障错误地恢复；2) sticky 告警在定时/焦点同步下会反复变回未读并累加次数，违背去重目标；3) 数据库断连走 transient 分支后没有恢复通知，恢复时只会被前端静默清掉。"}
</review_verdict>
<review_report>
## 结论
本次改动**不建议通过**。通知中心的主体结构已经落下来了，但在“系统异常同步”和“sticky 生命周期”两条主链路上存在行为级缺陷，会直接影响 6 类目标事件里的 SDK 异常、数据库异常以及去重体验。
## 阻断问题
- [src-tauri/src/app.rs](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src-tauri/src/app.rs:636)、[src-tauri/src/app.rs](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src-tauri/src/app.rs:3085)、[src-tauri/src/app.rs](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src-tauri/src/app.rs:5892)、[src-tauri/src/app.rs](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src-tauri/src/app.rs:3276)：本地/远程 SDK 异常的判定标准前后不一致。`health_check` 与 `validate_remote_codex_health` 按“启用了 SDK 但 effective provider 退回非 sdk”判故障；而 `sync_system_notifications` 的本地/SSH 同步只看 `node_available && sdk_installed`。结果是 provider 已回退但 SDK 仍安装的场景下，健康检查会创建“SDK 不可用”通知，下一次定时同步又会把它误判为已恢复并关闭，导致真实异常被错误消音。
- [src-tauri/src/notifications.rs](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src-tauri/src/notifications.rs:358)、[src-tauri/src/notifications.rs](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src-tauri/src/notifications.rs:397)、[src/components/layout/MainLayout.tsx](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src/components/layout/MainLayout.tsx:34)：sticky 去重实现会在每次重复命中时把同一条告警重新置为未读、清空 `read_at`、递增 `occurrence_count`。与此同时，主布局每次进入页面、每 60 秒轮询、窗口重新聚焦都会触发 `sync_system_notifications`。这意味着同一个持续故障即使内容完全没变，用户手动“已读”后也会很快再次变成未读，形成持续打扰，和任务要求里的“避免同一故障高频刷屏”相冲突。
- [src-tauri/src/app.rs](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src-tauri/src/app.rs:3001)、[src-tauri/src/app.rs](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src-tauri/src/app.rs:3039)、[src/stores/notificationStore.ts](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src/stores/notificationStore.ts:114)、[docs/notification-center-event-matrix.md](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/docs/notification-center-event-matrix.md:37)：数据库断连分支只发 `TransientNotification`，恢复分支却只会尝试 `resolve_sticky_notification("database_error:local")`。由于断连时根本没有持久化 sticky 记录，恢复后不会生成“数据库连接已恢复”通知；前端下一次成功 `fetchNotifications()` 时还会把 transient 列表直接清空，用户看到的结果是告警被静默消失，和事件矩阵里承诺的恢复反馈不一致。
## 风险提醒
- [src/stores/dashboardStore.ts](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src/stores/dashboardStore.ts:115)：仪表盘的“未读通知/高优先级告警”直接统计全量 `notifications` 表，没有按当前环境或项目范围过滤。切到某个 SSH 主机或单项目时，卡片数字可能混入别的环境/项目的通知，和页面其余统计口径不一致。
- [src/components/layout/NotificationCenter.tsx](/Users/wenyuan/study/.codex-ai-worktrees-codex-ai/33af1f51-ea08-4a9b-8af9-79ecdce4c301/src/components/layout/NotificationCenter.tsx:123)：`source_module` 既有中文值也有英文值，当前直接原样展示 badge，后续如果继续扩展通知来源，列表标签风格会不统一，且不利于多语言收口。
## 改进建议
- 把“SDK 是否不可用”的判定提成单一后端 helper，统一给 `health_check`、`sync_system_notifications`、`validate_remote_codex_health` 复用，避免本地与 SSH 两套逻辑分叉。
- 调整 `ensure_sticky_notification` 的刷新策略：仅在“从 resolved 重新激活”或“告警内容/严重级别发生实质变化”时重置未读并增加次数；纯轮询命中应保持当前已读状态。
- 为数据库断连的 transient 分支补一条显式恢复事件，或者在前端保留 transient 直到收到恢复信号，而不是在成功拉取持久化列表时直接丢弃。
- 给通知统计补明确口径：如果要做全局统计，就在文案上标明“全局”；如果要跟随当前环境/项目，就需要同步加过滤条件。
## 验证缺口
- 本次结论基于静态代码审查，**未执行** `npm run build`、`cargo test --manifest-path src-tauri/Cargo.toml`、`npm run tauri dev` 手工冒烟。
- 目前没有看到覆盖以下关键链路的自动化验证：sticky 告警轮询去重、数据库 transient 告警的恢复通知、SDK provider 回退场景下的本地/SSH 系统同步。
</review_report>

审核结果
";
    74	const REVIEW_REPORT_END_TAG: &str = "