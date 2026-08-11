# Codex AI 项目分析文档

基线分析：2026-07-16 · 增量：2026-07-26 · **代码现状校准：2026-08-11（应用 v0.5.6）**

本目录为**完整项目分析**产出，证据来自仓库只读审计（代码路径、迁移、IPC 注册表、LOC、文档对照）。

> **漂移提示（2026-08-10）**：下列结论中「前端 SQL 写库 P0」「依赖无环」「`shell:allow-execute`」等条目已过时——前端 SQL 已硬失败 stub；依赖环检测与拦截已落地；capabilities 已移除 `shell:allow-execute` / kill / stdin-write。请以 `01-domain-capability-matrix.md`（08-05 收尾版）与当前 `TASK.md` 为准。

> **计数校准（2026-08-11，实测）**：00–07 各文档中的规模数字多已过时。当前实测为 **218 IPC 命令 / 24 表 / 44 迁移 / 366 Rust 测试 / 前端 9 测试文件 104 断言 / 前端 44.6k 行 / Rust 60.8k 行**。复现命令见 `08-product-gap-2026-08-11.md` §1。

## 阅读顺序

1. [00 · 架构全景](./00-architecture-overview.md)  
2. [01 · 领域能力矩阵](./01-domain-capability-matrix.md)  
3. [02 · 数据模型与迁移](./02-data-model-audit.md)  
4. [03 · IPC 命令目录](./03-ipc-command-catalog.md)  
5. [04 · 运行时生命周期](./04-runtime-lifecycle.md)  
6. [05 · 质量与风险](./05-quality-risks.md)  
7. [06 · 技术债与路线图](./06-tech-debt-roadmap.md)（含体验打磨方向 C）  
8. [07 · 增量分析 Delta](./07-delta-2026-07-16.md)（相对 00–06 的落地复核与新风险）  
9. [08 · 产品缺口分析](./08-product-gap-2026-08-11.md)（产品视角：还缺什么 + 下一波 backlog）

## 一句话结论

模块化单体 + Rust 服务层方向正确，能力闭环已通、功能密度高。**2026-08-11 后主矛盾**转为 —— **AI 跑起来之后，用户看不见成本、管不住并发、复用不了经验**：`codex_sessions` 无 token 消耗列（成本零可见性）、后端无并发闸门与运行队列（多任务裸奔）、无任务模板（重复劳动）。`send_input` 已在 Codex/Claude/OpenCode 开放（Grok B1 豁免），组件体量债（`TaskDetailDialog` 2014 行 / `TaskCard` 1929 行）仍未关闭。完整 IDE / 第五引擎 / 双向 Issues 同步 / hunk 级暂存明确不做。
