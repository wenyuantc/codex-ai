# Implement: 任务详情主路径与 SSH 可信提示

## Preconditions

- [x] `prd.md` 已收敛
- [x] `design.md` 已写
- [ ] 用户批准本规划摘要后执行 `task.py start`
- [ ] 实现前 load `trellis-before-dev`（读 frontend component / state / quality + cross-layer 如触及 activity）

## Ordered Checklist

### 1. 主 CTA 解析器（纯函数）

- [x] 新增 `src/lib/taskPrimaryCta.ts`：实现 `resolveTaskPrimaryCta` + 类型，严格按 `design.md` 优先级表
- [x] 从 `TaskCard` 抽出与表相关的条件输入（runtime、automation phase、assignee/reviewer、canCommit、background phase）
- [x] 导出稳定 `kind`/`label`/`disabled`/`reason`/`tone`，组件只负责渲染

### 2. TaskCard 接解析器

- [x] 主操作条（运行/停止/审核/锁定）改为消费 `resolveTaskPrimaryCta`
- [x] 菜单内主操作文案与条上一致
- [x] 行为不变：仍调用现有 `handleRun` / `handleStop` / `handleReviewCode` 等
- [x] 自动化锁定、未指派、无审查员提示不回归

### 3. TaskDetailDialog sticky 主路径

- [x] 在 Dialog 底部（或标题下）增加 `TaskPrimaryActionBar`
- [x] 输入与卡片同源：`resolvedAutomationState` + `getTaskActionRuntimeState` + 任务字段
- [x] 绑定已有 handler：`handleRunCodex` / `handleStopCodex` / `handleStartCodeReview` / 验收生成；`commit` 若缺路径则最小接线或降级（见 design Risks）
- [x] 次要操作放 overflow，不与主按钮并列抢主色
- [x] `review`/`run`/`stop` 时可同步切换到对应 Tab（execution / review）

### 4. SSH 全局可信提示增强

- [x] 将 `MainLayout` 内联 SSH 条抽为可复用组件（建议 `src/components/layout/SshTrustBanner.tsx` 或扩展 sessions notice 的 app-bar variant）
- [x] 保留 amber sticky 样式；补充「审查依据可能不完整」语义 + 链到 `/settings`
- [x] 确认 `environmentMode === "ssh"` 时在看板/详情页无需进入会话深层即可看见
- [x] 会话/详情内既有 `SshArtifactLimitedNotice` 保留；文案不互相矛盾

### 5. 活动日志（按需）

- [x] 本任务默认不新增 action；若实现中新增，补 `getActivityActionLabel` 中文

### 6. 验证

- [ ] 手工矩阵（至少 5 态）：
  1. todo + 已指派 → 主 CTA「运行」
  2. 执行中 session → 「停止」
  3. status=review 或 review 进行中 → 「审核/审核中」
  4. automation 修复占用且无本地 stop → 「修复中…」disabled
  5. completed + 可提交 → 「提交代码」；或 completed + 验收 → 「生成验收清单」
  6. （附加）blocked → 「查看阻塞原因」类 soft CTA
- [ ] SSH 模式：顶栏 banner 可见，设置链接可用
- [ ] 自动化按钮：开启闭环任务的锁定逻辑与改前一致
- [x] `tsc --noEmit` 通过
- [x] `npm run build:web` 通过（~7.5 min）
- [x] `eslint` 目标文件 0 error（仅既有 hooks warning）
- [x] 无 Rust 变更，跳过 clippy

## Risky Files

| File | Risk |
|------|------|
| `src/components/tasks/TaskCard.tsx` (~1.6k) | 回归主按钮与菜单 |
| `src/components/tasks/TaskDetailDialog.tsx` (~1.5k) | Dialog 布局/handler 接线 |
| `src/components/layout/MainLayout.tsx` | 全局 banner |
| `src/lib/utils.ts` | 避免继续堆逻辑；优先新文件 |

## Rollback Points

1. 解析器落地后、接 UI 前：可单独保留文件不引用  
2. 仅 TaskCard 接入后：可单独回退详情 bar  
3. Banner 与 CTA 解耦：可独立 revert

## Definition of Done

- [ ] AC 全部满足（见 `prd.md`）
- [ ] `trellis-check` 通过视角：无双源 automation 展示、无 frontend 直写 DB
- [ ] 无未说明的 scope creep（不重写 Dialog IA）
