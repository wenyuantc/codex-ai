# Implement · P0-3 Claude CLI 本地图片补齐

1. 用本机 `claude` 探测 `--input-format stream-json` 图片帧最小合法形状；把样例放进测试/注释
2. CLI 启动：有图则 stream-json 写入 image blocks；无图回归保持可用
3. 去掉 CLI「直接跳过图片」WARN；读失败逐张 WARN
4. prompt 日志列出 CLI 实际附带的文件名
5. `imageAttachmentSkip` + 单测：本地 Claude CLI 不再 skip；SSH Claude 仍 skip
6. 能力矩阵 notes / 员工绑定说明：本地能看图、SSH 不能
7. clippy / 相关 cargo test / format:check / test:ci / build

风险文件：`claude/process/mod.rs`、`src/lib/imageAttachmentSkip.ts`、`imageAttachmentSkip.test.ts`、Settings/员工能力文案

不要加 `--image`。不要动 SSH 传图。不要把本机图当远程路径塞进 SSH CLI。
