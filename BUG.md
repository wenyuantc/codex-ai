保留仅实时日志在历史回填中的原始时序
Dismiss
当某个 Session 在首次打开日志弹窗前已经缓存了 session_event_id = None 的实时行时，这里的合并会先放完整历史，再把这些实时行统一追加到尾部。像启动阶段发出的 [PROMPT]、缺图警告或基线警告都会因此被排到正文末尾，严重时甚至落在 [EXIT] 之后，导致 Session transcript 的时间顺序被改写。


/Users/wenyuan/study/codex-ai/src/stores/employeeStore.ts：101-108


P2
将 SDK 可用的远程主机标记为健康通过
Dismiss
validate_remote_codex_health() 现在只用 runtime.codex_available 决定 last_check_status。如果远程机器没有全局 codex，但 Node 和 @openai/codex-sdk 已就绪，task_execution_effective_provider / one_shot_effective_provider 会是 sdk，实际任务和一次性 AI 都能跑；这里只会把 SSH 配置持久化成 failed，于是设置页里的连接测试和“当前配置状态”会把一个可用的 SDK-only 主机显示成不可用。


/Users/wenyuan/study/codex-ai/src-tauri/src/app.rs：4388-4388


P2
仅在远程 codex 可用时提示回退到 exec
Dismiss
build_remote_codex_runtime_health() 在“SDK 未启用 / Node 不可用 / SDK 未安装”这些分支里会直接返回“已回退到远程 codex exec”，但这里没有检查 codex_available。当远程主机既没有全局 codex，又不满足 SDK 条件时，健康检查和设置文案会宣称还有可用的 exec 回退路径，而 start_codex_with_manager() 和 run_ai_command_via_ssh_exec() 实际仍会执行 exec codex ... 并失败。


/Users/wenyuan/study/codex-ai/src-tauri/src/app.rs：4301-4308