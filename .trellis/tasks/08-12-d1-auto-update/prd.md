# PRD · D1 应用自动更新

父任务:`08-12-product-gap-wave` · 优先级 N-P1

## 需求

1. 接入 `tauri-plugin-updater`:应用可检查 GitHub Releases 上的新版本并下载安装。
2. 设置页新增「检查更新」入口:显示当前版本、检查按钮、有新版时显示版本号/说明并可一键更新(下载进度可见),更新完成提示重启。
3. CI 发版产物补齐 updater 所需签名与 `latest.json`(构建工作流调整 + 所需 secrets 说明)。
4. 无网络/无新版/未配置签名时给出可理解的失败提示,不崩、不静默。

## 约束

- 签名私钥不入库;公钥进 `tauri.conf.json`。私钥生成后交用户保管并配置到 CI secrets。

## 验收标准

- [ ] `Cargo.toml` 含 `tauri-plugin-updater`,capabilities 授权,前端可调 check()。
- [ ] 设置页可见当前版本 + 检查更新按钮;文案 i18n zh/en。
- [ ] 构建工作流含 updater artifacts 与 latest.json 生成路径(tag 发版时)。
- [ ] README 或设置页说明发版所需 secrets(TAURI_SIGNING_PRIVATE_KEY 等)。
