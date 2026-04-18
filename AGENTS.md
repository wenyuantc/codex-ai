# 仓库指南

## 项目结构与模块组织
该应用由 Vite/React 前端和 Tauri/Rust 桌面壳组成。前端源码位于 `src/`：页面级路由在 `src/pages`，可复用 UI 在 `src/components`，共享辅助函数在 `src/lib`，Zustand 状态在 `src/stores`。静态资源应放在 `public/` 或 `src/assets/`。原生桌面端代码位于 `src-tauri/src`，其中数据库代码在 `src-tauri/src/db`，Codex 进程管理位于 `src-tauri/src/codex`。将 `.omx/` 和 `src-tauri/target/` 视为生成物或运行时状态，不要手工修改。

## 开发指南
- 每次修改代码，如果涉及数据库必须确保数据库升级脚本。
- 时间字段展示改为走 formatDate()
- 每次新增功能，要增加最近活动日志、并且key要再仪表盘显示中文
- 新增功能都要兼容SSH模型

## 构建、测试与开发命令
- `npm run dev`：启动仅用于浏览器开发的 Vite 前端。
- `npm run build`：执行 TypeScript 编译并生成前端产物。
- `npm run preview`：在本地预览生产构建后的前端包。
- `npm run tauri dev`：启动带 Rust 后端且前端支持热更新的桌面应用。
- `npm run tauri build`：构建可分发的桌面安装包。
- `cargo test --manifest-path src-tauri/Cargo.toml`：运行 Tauri 层的 Rust 测试与编译检查。

## 编码风格与命名约定
TypeScript 使用 2 空格缩进，Rust 使用默认 `rustfmt` 格式化。React 组件、页面和对话框文件使用 PascalCase 命名，例如 `CreateTaskDialog.tsx`；store、工具函数和模块辅助文件使用 camelCase 命名，例如 `taskStore.ts` 和 `database.ts`。共享 React 组件优先使用具名导出，业务相关 UI 尽量按领域放在类似 `src/components/tasks` 的目录下。新增抽象前，优先复用 `src/components/ui` 和 `src/lib/utils.ts` 中现有的工具与基础能力。

## 测试指南
当前仓库尚未定义前端测试运行器，因此每次修改至少要通过 `npm run build`，并在 `npm run tauri dev` 中完成一次手工冒烟测试。新增自动化测试时，前端测试应放在对应功能附近或 `src/__tests__` 下；Rust 测试应保留在相关的 `src-tauri/src/*` 模块中，并使用 `#[cfg(test)]`。优先覆盖 store、数据库访问以及 Codex 进程管理相关逻辑。

## 提交与 Pull Request 规范
近期提交历史使用简洁的 Conventional Commit 前缀，例如 `feat:` 和 `fix(codex):`；请保持这一模式，并让提交主题聚焦于“为什么改”。Pull Request 应总结用户可见的行为变化，列出验证步骤，并关联对应的 issue 或任务。涉及 UI 改动时附上截图或短录屏；涉及数据库、能力边界或进程管理的改动时要明确说明。

## 版本号修改

更新版本号命令 npm run bump-version -- 0.2.0  