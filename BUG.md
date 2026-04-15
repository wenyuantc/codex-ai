保留仅实时日志在历史回填中的原始时序
Dismiss
当某个 Session 在首次打开日志弹窗前已经缓存了 session_event_id = None 的实时行时，这里的合并会先放完整历史，再把这些实时行统一追加到尾部。像启动阶段发出的 [PROMPT]、缺图警告或基线警告都会因此被排到正文末尾，严重时甚至落在 [EXIT] 之后，导致 Session transcript 的时间顺序被改写。


/Users/wenyuan/study/codex-ai/src/stores/employeeStore.ts：101-108