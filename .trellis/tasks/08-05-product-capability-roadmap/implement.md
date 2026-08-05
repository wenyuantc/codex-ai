# Implement — 产品能力补齐路线图（父任务）

## 执行原则

1. **一次只 `task.py start` 一个子任务**（可验证交付）。
2. 父任务保持 `planning` 或仅作集成跟踪；不在父任务目录写产品代码。
3. 每子任务：`before-dev` → 实现 → `check` → commit → archive 子任务。
4. 全部子任务完成后更新 analysis/README，再 archive 父任务。

## 推荐顺序与门禁

| 序 | 子任务 | 进入条件 | 完成门禁 |
|----|--------|----------|----------|
| 1 | `08-05-tester-automation-loop` | 本规划获用户批准 | build + clippy + 自动化相关 cargo test；手工：先测后审路径 |
| 2 | `08-05-kanban-delivery-ux` | #1 完成或可并行若无人改同一文件 | build；筛选中文；归档可编辑 |
| 3 | `08-05-ux-trust-hardening` | 可与 #2 串行 | 主 CTA + SSH banner |
| 4 | `08-05-frontend-test-net` | 建议在 #2 后插入首版 | `npm test` 绿 |
| 5 | `08-05-engine-capability-parity` | — | 能力徽章 + 尽力补齐有测 |
| 6 | `08-05-opencode-ssh-bridge` | 可与 #5 紧邻 | SSH 启动 OpenCode 路径 |
| 7 | `08-05-insights-export` | — | 趋势 + 任务 JSON 往返 |
| 8 | `08-05-coordinator-pipeline-viz` | 协调员数据已有 | 阶段 UI |
| 9 | `08-05-mcp-task-binding` | MCP 清单已有 | 任务绑定生效于会话 |

> 顺序可在实现中按风险微调，但 **tester 优先** 与用户「全部要做」的 P0 一致。

## 每子任务标准 Definition of Done

- [ ] 子任务 `prd` AC 勾选
- [ ] `npm run build`
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] 相关 `cargo test`（后端改动时）
- [ ] 新 activity key 中文
- [ ] SSH 兼容说明（适用时）
- [ ] Conventional Commit + 子任务 archive

## 父级收尾清单

- [ ] 9/9 子任务 archived（或裁剪记录）
- [ ] `docs/analysis/01-domain-capability-matrix.md` 更新
- [ ] 父任务 archive
