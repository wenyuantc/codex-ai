# PRD · Agent 循环与核心工具

> 父任务：`08-20-native-agent`。依赖模型客户端。不接入 Tauri 会话生命周期（下一子任务）。

## Goal

可取消的多轮 tool loop：模型要工具则执行，结果回灌，直到纯文本结束或取消。工具在本地与 SSH 工作区都能跑。

## Requirements

- R1 工具名：`Read` `Write` `Edit` `Bash` `Glob` `Grep` `TodoRead` `TodoWrite`（对齐 zcli）。
- R2 workspace-only：禁止读写工作区外路径。
- R3 Bash 默认超时 120s、上限 600s；模型可见输出约 30KB。
- R4 Write 覆盖已存在文件前应先 Read（对齐 zcli 语义，可警告而非硬拦若实现过重）。
- R5 Edit 精确字符串替换；找不到则错误。
- R6 SSH：同一工具接口，经 `build_ssh_command`；不部署远端二进制。
- R7 取消 token 能停循环与正在跑的 Bash。
- R8 超窗口时截断早期 tool 结果（简单策略，非 zcli compact 全套）。
- R9 单测：本地临时目录跑 Read/Write/Edit/Glob/Grep；路径逃逸被拒。SSH 至少测命令构造或 mock。

## Out of Scope

- Skills、子 Agent、ApplyPatch、MCP、权限 UI、plan 模式
- `start_native_session` 与前端

## Acceptance Criteria

- [x] 给定 mock 模型「先 Read 再 Edit」，循环在临时 git 目录改对文件
- [x] `../` 逃逸失败
- [x] 取消后不再发下一轮模型请求
- [x] clippy / cargo test 通过
