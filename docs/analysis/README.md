# Codex AI 项目分析文档

基线分析：2026-07-16 · 增量：2026-07-26 · **代码现状校准：2026-08-10（应用约 0.5.6）**

本目录为**完整项目分析**产出，证据来自仓库只读审计（代码路径、迁移、IPC 注册表、LOC、文档对照）。

> **漂移提示（2026-08-10）**：下列结论中「前端 SQL 写库 P0」「依赖无环」「`shell:allow-execute`」等条目已过时——前端 SQL 已硬失败 stub；依赖环检测与拦截已落地；capabilities 已移除 `shell:allow-execute` / kill / stdin-write。请以 `01-domain-capability-matrix.md`（08-05 收尾版）与当前 `TASK.md` 为准。

## 阅读顺序

1. [00 · 架构全景](./00-architecture-overview.md)  
2. [01 · 领域能力矩阵](./01-domain-capability-matrix.md)  
3. [02 · 数据模型与迁移](./02-data-model-audit.md)  
4. [03 · IPC 命令目录](./03-ipc-command-catalog.md)  
5. [04 · 运行时生命周期](./04-runtime-lifecycle.md)  
6. [05 · 质量与风险](./05-quality-risks.md)  
7. [06 · 技术债与路线图](./06-tech-debt-roadmap.md)（含体验打磨方向 C）  
8. [07 · 增量分析 Delta](./07-delta-2026-07-16.md)（相对 00–06 的落地复核与新风险）

## 一句话结论

模块化单体 + Rust 服务层方向正确，能力闭环强。**2026-08 后主矛盾**转为：主路径组件体量、SSH 产物等价、备份语义完整性、以及引擎交互（`send_input`）尚未开放。P0/P1 产品体验债已在 `TASK.md` 收口；完整 IDE / 第五引擎 / 双向 Issues 同步仍明确不做。
