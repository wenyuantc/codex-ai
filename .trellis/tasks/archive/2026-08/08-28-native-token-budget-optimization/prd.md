# 按 Codex 上下文窗口机制优化内置 Agent Token 消耗

## Goal

降低内置 Agent 在简单编码任务中的累计 token，重点消除每轮重复发送完整历史和大工具输出造成的输入放大，同时保留复杂任务通过上下文压缩继续执行的能力。

## Requirements

- Agent 会话必须按上下文窗口管理历史；达到安全阈值时自动压缩或开启新窗口，不得无限携带完整旧历史。
- 自动压缩优先使用当前模型生成结构化摘要；摘要失败或预算不足时使用本地结构化窗口重置。
- 工具结果进入模型上下文前必须按 token 预算截断，保留可继续读取的路径/偏移提示；UI/会话事件仍可保留完整结果。
- 父 Agent 和子 Agent 必须共享累计 rollout token 预算，子 Agent 不得绕过预算；达到预算时停止新的工具轮次并收尾。
- OpenAI Responses/Codex 协议优先复用服务端 continuation；Chat/Anthropic 协议使用本地窗口压缩。
- 本地和 SSH 工作区行为一致；旧设置文件和旧会话数据必须兼容。
- 记录窗口、压缩、截断、父子 Agent 和预算停止原因，便于解释异常高用量。

## Acceptance Criteria

- [ ] 简单任务的每轮 input token 不再随历史线性增长，累计 token 相比当前实现至少降低一个数量级。
- [ ] 上下文压缩后保留系统约束、用户目标、已完成修改、验证结果和待办事项。
- [ ] 工具大输出不会再次触发完整重复读取；模型能按提示用路径/offset 继续读取。
- [ ] 父/子 Agent 共享预算和取消语义在本地、SSH、只读计划三类流程中都成立。
- [ ] Responses、OpenAI Chat、Anthropic 三种协议均有对应测试或明确的兼容降级路径。
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`、`npm run format:check`、`npm run build` 通过。
