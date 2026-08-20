# PRD · A1 员工状态语义纠偏

父任务:`08-20-product-trust-ops` · 优先级 P0

## Goal

员工徽章「离线」实际是**空闲**（会话结束后写 `offline`）。仪表盘「在线员工」只计 `online/busy`，空闲时几乎恒为 0。用户会以为员工挂了或被禁用。

## 证据

- 空闲回写 `offline`：`src/stores/employeeStore.ts:117-119`、会话退出 `:392`
- 启动路径丢弃 status：`CodexControls.tsx:44` `employeeStatus: _employeeStatus`
- 仪表盘口径：`src-tauri/src/app/database.rs:1625` `status IN ('online', 'busy')`
- 文案：`src/locales/zh-CN/common.json` `online=在线` `offline=离线`
- 编辑员工对话框**没有**状态开关（`EditEmployeeDialog.tsx`）

## Requirements

1. 用户可见文案改为运行时语义：空闲 / 运行中 / 异常（en: Idle / Running / Error）。保留内部枚举值以免大迁移。
2. 仪表盘指标改为「运行中员工」或「空闲+运行中」，禁止再把空闲当成不在线。
3. 员工列表筛选项与徽章同步新文案。
4. **禁止**按 `status === offline` 拦截启动。若以后要「禁用员工」，必须新字段（如 `enabled`），本任务不做。
5. 关闭 `TASK.md`「暂时不处理·离线禁跑」：标注为语义错误，不提升为拦截。

## Acceptance Criteria

- [x] 中英文界面不再把空闲员工叫「离线/Offline」
- [x] 仪表盘数字在全员空闲时不为「全员不在线」的误导读法
- [x] 空闲员工仍可从看板/员工卡片启动任务
- [x] 无新的 `offline` 启动硬拦截
- [x] i18n zh-CN + en；活动日志如有新 key 需中文映射

## Out of Scope

禁用/启用开关、workload 条 `MAX_TASKS=5` 限流。
