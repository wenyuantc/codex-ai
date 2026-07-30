# Implement: engine-trait-abstraction

## Preconditions

- [x] 分支 `feat/engine-trait-abstraction`
- [x] 父任务顺序：C5 可独立进行（不依赖 C6/C7）
- [ ] 用户批准本规划摘要后再改产品代码
- [ ] 实现前加载 `trellis-before-dev`（backend specs）

## Baseline（开始实现前记录）

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --list 2>/dev/null | tail -5
# 记下 test 数量作为「不得净减少」底线
```

## Checklist

### Phase A — 共享内核骨架

1. [x] `src-tauri/src/engine/mod.rs` + `lib.rs` 声明 `mod engine`
2. [x] `engine/context.rs`：迁入统一 `ExecutionContext` + resolve_*（`engine_label`）
3. [x] `engine/child.rs`：`EngineChild`（对齐 claude/grok 行为）
4. [x] `engine/manager.rs`：`ProcessManager` / `ManagedProcess` 泛型
5. [x] `engine/status.rs`：`resolve_final_session_status`
6. [x] 为上述模块写单元测试并先绿

### Phase B — Claude / Grok 切换

7. [x] `claude/process/context.rs` → re-export / 固定 label `"Claude"`
8. [x] `claude/process/lifecycle.rs` → re-export `EngineChild` as `ClaudeChild` 或全局替换类型
9. [x] `claude/manager.rs` → 共享 Manager
10. [x] Grok 同步 7–9（label `"Grok"`）
11. [x] session_runtime final-status 改调共享 helper
12. [x] `cargo test` 过滤 claude/grok 相关

### Phase C — Codex / OpenCode

13. [x] Codex context 改用共享；保留 one-shot/project 扩展
14. [x] CodexChild / CodexManager 接入 `Extra`
15. [x] OpenCode context 删除重复，调用共享
16. [x] OpenCodeChild / OpenCodeManager 适配（sdk_server 保留）
17. [x] 修复因 `try_wait` / kill 签名引起的调用点

### Phase D — 测试与文档

18. [x] Claude/Grok manager 缺口测试
19. [x] 全量 `cargo test --manifest-path src-tauri/Cargo.toml`
20. [x] 测试数量 ≥ baseline（284 → 295）
21. [x] 更新 `.trellis/spec/backend/ai-engines.md`
22. [x] 更新 `.trellis/spec/backend/directory-structure.md`（`engine/` 布局）
23. [x] 视需要更新 `CLAUDE.md`「零 trait」描述

### Phase E — 质量门

24. [x] 自检：无 stream 误合并、无 SSH 校验弱化、无 State 类型名破坏
25. [ ] 提交（Conventional Commit，例如 `refactor(engine): share process manager and execution context`） — 待用户确认

## Validation Commands

```bash
cargo test --manifest-path src-tauri/Cargo.toml
# 可选冒烟（手工）：npm run tauri:dev
# 本地 + 若有环境则 SSH 项目各起一次会话（父任务集成项，子任务至少不破坏编译测试）
```

## Risky files

| 文件 | 风险 |
|------|------|
| `codex/process/lifecycle.rs` | 与 Child 混杂大量 stop 逻辑，改类型时勿挪逻辑位置出错 |
| `opencode/process/lifecycle.rs` | stdout/stderr 预取语义 |
| `task_automation.rs` | 尽量不改；仅当 Manager 方法签名外溢时最小改动 |
| `lib.rs` | 仅加 `mod engine` |

## Rollback points

- Phase A 失败：删除 `engine/` 与 `mod engine`
- Phase B 失败：还原 claude/grok 三文件
- Phase C 失败：还原 codex/opencode；共享模块可保留若测试自洽

## Definition of done

满足 `prd.md` Acceptance Criteria 全部勾选；`implement.md` checklist 完成；用户可见行为无回归。
