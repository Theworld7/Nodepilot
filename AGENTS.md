# AGENTS.md

nodepilot 是基于 Tauri 2 的桌面端 Node.js 版本管理器（macOS + Windows）。Rust 后端拥有全部版本管理逻辑，Vue 3 前端仅负责展示。代码注释、提交信息均使用中文。

详细产品功能见 `README.md`，领域术语见 `CONTEXT.md`，架构决策见 `docs/adr/`（新增架构决策需写 ADR）。已存在 Claude/Cursor 专用指引见 `CLAUDE.md` 与 `.codebuddy/`，可与本文件互为补充。

## 改动后验证（不需要全量构建）

```bash
pnpm exec vue-tsc -b          # 前端类型检查（比 pnpm build 快得多）
cd src-tauri && cargo check   # Rust 类型检查
cd src-tauri && cargo test    # 跑所有测试（前端无测试）
cd src-tauri && cargo test <name>   # 跑单个测试
cd src-tauri && cargo clippy  # Rust lint
```

只有需要验证发布路径（如 `check_app_update`）时再 `pnpm tauri build`。

## 架构关键点

- **Rust 后端独占版本管理逻辑**（ADR 0001）。前端通过 `#[tauri::command]` 调用，所有 handler 在 `src-tauri/src/lib.rs` 的 `generate_handler!` 中注册。
- **`version/` 模块用 Command 模式**：`VersionManager::execute(VersionCommand, &dyn EventSink)` 分派到 fetcher/installer/activator/deleter，每个操作完成后再 `enrich` 标记 installed/active，经 `VersionEvent` 推回 UI。
- **依赖注入贯穿全后端**：`lib.rs` 把 `HttpClientProd`（reqwest）和 `FsProd` 包装成 `Arc<dyn HttpClient>` / `Arc<dyn FileSystem>` 注入 `VersionManager`。**新引入网络或文件操作的代码必须沿用这两个 trait**（参考 `commands.rs::check_app_update`），测试里替换为 `HttpClientMock`/`FsMock`。
- **错误**统一走 `src-tauri/src/error.rs` 的 `AppError`，serde `tag="kind"`，前端拿到的是结构化错误。
- **双窗口** + 托盘：主窗口 375×667 不可缩放；日志窗口是同一 webview 带 `?view=log` 参数的另一实例（`App.vue` 按 query 路由）。窗口关闭 = `prevent_close` + hide，仅托盘菜单可退出；退出时清理所有 dev server 子进程。

## 这些坑不读代码很容易踩

### Dev server 子进程（`start_dev_server`）
- 必须把项目绑定的 Node 版本的 bin 注入子进程 PATH（如 `~/.nodepilot/versions/v14.21.3/`），不能用全局 `current` 符号链接 —— current 是用户激活的版本，可能与项目不匹配。
- macOS 用 `/usr/bin/script` PTY 包装获取行缓冲输出；关掉 stdin 会导致 Vite 退出。**停止时必须杀整棵进程树**（PTY 让内层进程 `setsid` 到新进程组，`kill -- -pid` 杀不到）。
- 调试 PTY 问题可设 `NODEPILOT_NO_PTY=1`。

### Windows 差异
- **所有子进程必须加 `creation_flags(0x0800_0000)`**（CREATE_NO_WINDOW），否则会闪控制台窗口。`start_dev_server`/`list_git_branches`/`checkout_branch` 都这么写，新增的也要。
- 符号链接需要管理员权限。
- `cfg(windows)` 门控的依赖（`winreg`、`taskkill`）单独放在 `src-tauri/Cargo.toml` 的 `[target.'cfg(target_os = "windows")'.dependencies]`，改动时不要破坏 macOS 编译。

### 更新检查（ADR 0007）
- `check_app_update` 直接查 GitHub Releases API，**不用 tauri-plugin-updater** —— releases 不发布 `latest.json`，插件配了也没用。
- **debug 构建直接返回 `None`**；`lib.rs` 里 updater 插件注册也是 `#[cfg(not(debug_assertions))]`。要真机验证必须 `pnpm tauri build` 后跑 release 包。

### 配置一致性
- `src-tauri/tauri.conf.json` 的 `build.devUrl`（`http://localhost:5199`）与 `vite.config.ts` 的 `server.port`/`strictPort: 5199` 必须保持一致。
- 新增窗口/能力要同步在 `src-tauri/capabilities/default.json` 声明（`windows` 数组 + 需要的 `core:*` 权限）。

## 约定

- **版本号单一来源**：`src-tauri/Cargo.toml`。`./scripts/release.sh` 会同步到 `tauri.conf.json`，`package.json` 始终保持 `0.0.0`。`RELEASE_NOTES.md` 里的 `{version}` 占位符会被脚本替换。
- **镜像源**：默认 `https://nodejs.org/dist/index.json`；`VersionUrls::derive_dist_url` 从 index URL 推导 dist 前缀，改镜像时两条 URL 同步更新。
- **IPC 字段命名**：Rust 返回 `snake_case`，前端 `src/types/index.ts` 原样镜像。
- **tdesign-vue-next**：组件与图标通过 `unplugin-vue-components` + `TDesignResolver`（`vite.config.ts`）自动导入，无需手动 import；**插件 API**（`MessagePlugin`、`DialogPlugin` 等）必须显式 import。
- **首次环境配置**：首次启动在 `lib.rs::setup` 里静默尝试 `env_setup::setup`，失败时写 `~/.nodepilot/.auto-setup-error`，前端 `App.vue` 挂载时检测后弹重试/跳过框；不要在前端手动调用 PATH 修改。
- **ADR 新增规范**：架构级改动（窗口/UI 库/更新方案等）写到 `docs/adr/NNNN-kebab-title.md`，README 的 ADR 列表也要同步更新。

## 发布流程

`./scripts/release.sh [--dry-run] [VERSION]`：递增版本号、生成/校验 `~/.nodepilot-keys/nodepilot.key` 签名对、构建 DMG + updater tarball + `latest.json`、通过 `gh` 创建 Draft Release。脚本会先把版本号写入 `Cargo.toml` 和 `tauri.conf.json`，再开始打包 —— 顺序不要颠倒。