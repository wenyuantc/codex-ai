# PRD · B1 运行队列运营面

父任务:`08-20-product-trust-ops` · 优先级 P1

## Goal

并发闸门和持久化队列已有，但运营面只有卡片「排队中第 N 位」和取消。批量运行只给 started/queued/skipped 三个数字，不知道谁被跳过、为什么。

## 证据

- 队列 API：`src/lib/backend.ts` `listTaskRunQueue` / `cancelQueuedTaskRun`
- 卡片取消：`src/components/tasks/TaskCard.tsx:752-756`
- 批量摘要：`src/pages/KanbanPage.tsx:280-356` + `kanban.json` `batchRunSummary`
- 看板无队列面板

## Requirements

1. 看板提供「运行队列」入口：列出当前排队任务、第几位、入队时间（`formatDate`）、取消。
2. 批量运行结果按任务给出跳过原因（无执行人 / 已在跑 / 已在队列 / 依赖未完成 / 已归档 / 启动失败），不只三个计数。
3. 队列为空时有空态，不假装有调度器。
4. 不引入重排、优先级插队、cron。

## Acceptance Criteria

- [ ] 不打开每张卡片也能看到并取消排队任务
- [ ] 批量运行后能指出被跳过的任务及原因
- [ ] SSH 项目任务同样出现在队列列表
- [ ] i18n zh-CN + en；已有活动 key 复用

## Out of Scope

队列重排、定时启动、改并发上限算法。
