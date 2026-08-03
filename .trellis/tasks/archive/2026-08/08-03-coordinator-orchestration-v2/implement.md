# Implement: 协调员编排 v2

## Preconditions

- [x] PRD 决策 D1–D5 已锁定  
- [x] design.md 已写  
- [ ] 用户批准最终规划摘要  
- [ ] `task.py start` 后方可改产品代码  

## Validation commands（每阶段末）

```bash
npm run build
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
# 相关单测可收窄：
cargo test --manifest-path src-tauri/Cargo.toml task_automation
cargo test --manifest-path src-tauri/Cargo.toml coordinator
cargo test --manifest-path src-tauri/Cargo.toml pipeline
```

手工冒烟：`npm run tauri dev`（本地一条编排 + 开/关质控各一次）。

## Ordered checklist

### Phase A — 数据与模型

1. [x] `db/migrations.rs`：新增 `task_pipeline_steps` 表 + `task_automation_state.pipeline_active` + `pipeline_step_index`（v42）
2. [x] `db/models.rs` + `src/lib/types.ts`：步骤/状态 DTO
3. [x] 归档守卫纳入 pipeline phases

**Verify**：迁移可空库升级；旧任务无 steps 可读。

### Phase B — 结构化计划生成

4. [x] 升级 `coordinator_plan` 默认模板：Markdown + JSON steps  
5. [x] `ai_generate_coordinator_task_plan`：注入员工列表；解析 JSON；写 `plan_content` + steps  
6. [x] 解析失败 fallback + 活动日志中文 key  
7. [x] 单测：prompt 文案 / pipeline unit  

**Verify**：生成后 DB 有多条 steps；`plan_content` 仍有可读正文。

### Phase C — 编排运行时（核心）

8. [x] `task_automation/pipeline.rs` include  
9. [x] `resolve_pipeline_step_employee`  
10. [x] `launch_pipeline_step` 四引擎  
11. [x] `handle_session_exit` pipeline 分叉（中间不审 / 末步质控）  
12. [x] start/retry/abort + `lib.rs` 注册  
13. [x] resume + unconsumed pipeline exits  
14. [x] emit phase 变更  
15. [x] pipeline unit tests；全量 lib 测试 312 通过  

**Verify**：单元测试全绿。

### Phase D — 读写 API 与活动

16. [x] list/update steps commands  
17. [x] 活动中文 `utils.ts`  
18. [x] `backend.ts` 封装  

### Phase E — 前端

19. [x] 任务详情：工作包列表 + 改执行人  
20. [x] **按计划编排** / 重试 / 转人工  
21. [x] **立即执行** 未改为自动 pipeline  
22. [ ] 看板卡片级 phase 徽标（可后续增强）  
23. [x] SSH 错误走现有引擎中文路径  

### Phase F — 收口

24. [x] `npm run build` + clippy `-D warnings` + `cargo test --lib`  
25. [ ] 手工 tauri 冒烟（用户环境）  
26. [ ] 可选 `trellis-update-spec`  

## Risky files

| 文件 | 风险 |
|------|------|
| `task_automation/session_exit.rs` | 误伤现网质控入口 |
| `task_automation/fix_loop.rs` | 与 pipeline 启动引擎分支持久分叉 |
| `codex/process/ai_commands.rs` | 计划生成兼容旧调用方 |
| `src/lib/taskCreateAndRun.ts` | 误把立即执行改成编排 |
| `db/migrations.rs` | 版本冲突 |

## Rollback points

- Phase A 仅迁移：可前进保留空表  
- Phase C 前：无运行时入口则用户无感  
- 上线后紧急：隐藏「按计划编排」；`pipeline_active` 任务转人工命令  

## Out of implement scope（勿顺手做）

- 测试员自动验收、并行步骤、跳过、send_input 对齐、Meta-Agent  

## Suggested commit slices

1. `feat(db): task pipeline steps + automation pipeline cursor`  
2. `feat(coordinator): structured plan generation`  
3. `feat(automation): serial pipeline runtime + review handoff`  
4. `feat(ui): pipeline steps panel and run entry`  
