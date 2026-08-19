# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

nodepilot 是一个基于 Tauri 2 的桌面端 Node.js 版本管理器（macOS + Windows）。Rust 后端拥有全部版本管理逻辑（ADR 0001），Vue 3 前端仅负责展示与交互。产品功能与目录结构见 `README.md`，领域术语（版本、激活、离线策略等）见 `CONTEXT.md`，架构决策见 `docs/adr/`（新增架构决策需写 ADR）。代码注释、提交信息均使用中文，请保持这一惯例。

## 常用命令

```bash
pnpm dev                              # 仅前端（Vite dev server，端口 5199 strictPort）
pnpm install                          # 安装前端依赖
pnpm tauri dev                        # 开发模式（自动先跑 Vite）
pnpm tauri build                      # 生产构建，产物在 src-tauri/target/release/bundle/
pnpm build                            # vue-tsc -b && vite build（前端类型检查 + 构建）
pnpm vue-tsc --noEmit                 # 仅前端 TypeScript 检查
cd src-tauri && cargo check           # Rust 类型检查
cd src-tauri && cargo test            # Rust 单元测试（含 mock，无集成测试）
cd src-tauri && cargo test <name>     # 跑单个测试
cd src-tauri && cargo clippy          # Rust lint
./scripts/release.sh [--dry-run] [VERSION]   # 发布：递增版本号、构建 DMG、生成 latest.json、创建 GitHub Draft Release
```

改动后验证：Rust 用 `cargo check`，前端用 `pnpm exec vue-tsc -b`（不需要跑完整 `pnpm build`）。前端没有测试套件（package.json 无 test script），所有测试都在 Rust 侧。

## 架构总览

### 分层与数据流

- **Rust 后端拥有全部版本管理逻辑**（ADR 0001），前端只通过 IPC 调用和展示。`commands.rs` 里的 `#[tauri::command]` 是唯一入口，全部在 `lib.rs` 的 `generate_handler!` 中注册。
- `version/` 模块：`VersionManager` 是外观（facade），内部拆成 fetcher（远程列表+缓存）/ installer（流式下载解压）/ activator（符号链接切换）/ deleter。
- **依赖注入贯穿全后端**：`lib.rs` 实例化 `HttpClientProd`（reqwest）和 `FsProd`（真实文件系统）后以 `Arc<dyn Trait>` 注入 `VersionManager`；测试中替换为 `HttpClientMock` / `FsMock`（内存实现，模拟符号链接）。新增需要网络/文件的代码应沿用这两个 trait（如 `commands.rs` 的 `check_app_update` 复用 `HttpClientProd`），而不是直接 new。
- 错误统一走 `error.rs` 的 `AppError`（serde tag="kind" 序列化），前端可拿到结构化错误。

### 数据目录 `~/.nodepilot/`

- `versions/{vX.Y.Z}/` — 已安装的 Node.js 发行版
- `current` — 指向当前激活版本的符号链接；`current/bin` 会被注入系统 PATH（首次启动时自动完成）
- `cache/versions.json` — 远程版本列表缓存（离线时先展示缓存再后台刷新）
- `config.json` — 用户配置（`mirror_url` 镜像源）
- `projects.json` — 项目绑定列表（版本、路径、别名、默认脚本、命令前缀、自定义启动命令）
- `.auto-setup-done` / `.auto-setup-error` — 环境配置完成标志 / 失败原因（供前端弹窗重试）

### Rust 后端 `src-tauri/src/`

- **`lib.rs`** — 应用入口：构建 `AppState`、注册 IPC handler、托盘图标（用 imageproc 渲染当前主版本号）、首次启动静默尝试 `env_setup::setup`、窗口关闭 → `prevent_close` + hide（常驻托盘，仅托盘菜单可退出）、退出时清理所有 dev server 进程
- **`commands.rs`** — 所有 `#[tauri::command]`，前后端唯一接口层。`AppState` 含 `servers: Arc<Mutex<HashMap<path, pid>>>` 和 `log_buffers: Arc<Mutex<HashMap<path, Vec<String>>>>`（每条日志上限 1000 行）。分组：版本管理（`get/refresh/install/activate/delete_version`）、项目绑定、Dev Server（`start/stop_dev_server`、`get_dev_server_logs`、`get_running_servers`）、Git 分支（`list_git_branches`、`checkout_branch`）、环境配置（`auto_setup`/`rollback_setup` 等）、配置（`get/set_config`）、更新检查（`check_app_update`）
- **`version/`** — 版本领域层，**Command 模式**：`VersionManager::execute(VersionCommand, &dyn EventSink)` 分派到 fetcher、installer（流式下载进度回调 + tar.gz/zip 解压 + 架构回退 arm64→x64）、activator（符号链接切换）、deleter（拒绝删除当前激活版本）。每个操作完成后重取版本列表并 `enrich`（标记 installed/active），通过 `VersionEvent` 返回给 UI
- **`env_setup.rs`** — 自动环境配置：macOS 用 LaunchAgent + `.zshrc`/`.bashrc` 注入 PATH，Windows 改 HKCU 注册表；检测并禁用 nvm/fnm/volta 的 Shell Hook（注释掉配置行，记录行号以便回滚）；失败时完整回滚并写入 `.auto-setup-error`
- **`client.rs` / `fs.rs`** — `HttpClient` / `FileSystem` trait 抽象（见上文 DI）
- **`tray.rs`** — 托盘图标生成（左键显示/聚焦主窗口）；**`error.rs`** — 全局 `AppError`

### 双窗口与托盘

- 主面板 375×667 不可缩放；LogView 是同一个 webview 带 `?view=log` 参数的另一窗口（`App.vue` 里按 query 分支）。
- 关闭窗口 = 隐藏到托盘（`lib.rs` 的 `CloseRequested` 拦截），应用常驻，仅托盘菜单可退出。
- 主窗口每次打开都应刷新数据；版本列表有本地缓存，先展示缓存再后台刷新。

### Vue 3 前端 `src/`

- **`App.vue`** — 按 URL 参数路由：`?view=log` → `LogView`（独立日志窗口），否则 `VersionListPanel`；挂载时检查首次环境配置并弹确认框（重试/跳过）
- **`panels/VersionListPanel.vue`** — 主面板：版本列表、搜索、LTS 筛选、安装/激活/删除
- **`components/VersionRow.vue`** — 版本折叠条目，展开显示绑定项目；**`ProjectRow.vue`** — 项目行（启动/停止 dev server、日志、Git 分支切换、设置抽屉）
- **`composables/useVersionManager.ts`** — 封装所有 Tauri `invoke` 调用 + event 监听，提供响应式状态；**`useTheme.ts`** — 明暗主题
- **tdesign-vue-next 自动导入**：`vite.config.ts` 配置了 `unplugin-auto-import` + `unplugin-vue-components`（TDesignResolver），组件和图标无需手动 import

### 前后端通信

- 命令：`invoke`（见 commands.rs 的 handler 列表）
- 事件（Rust `app.emit` → 前端 `listen`）：`versions_updated`、`version_activated`、`dev_server_log`、`dev_server_status`

### Dev Server 子进程（关键实现细节）

`start_dev_server` 用 `tokio::process::Command` 启动，**必须把 `~/.nodepilot/current/bin` 注入子进程 PATH**（打包应用中系统 node 不在 PATH）。macOS 上通过 `/usr/bin/script` PTY 包装获取行缓冲输出（stdin 被关闭会导致 Vite 退出）；设环境变量 `NODEPILOT_NO_PTY=1` 可禁用 PTY 做诊断。stdout/stderr 逐行读取、strip ANSI 后经 `dev_server_log` 推送前端。停止时需终止**整棵进程树**（PTY 会让内层进程 `setsid` 到新进程组，`kill -- -pid` 杀不到），避免退出后残留孤儿服务。

### 更新检查（ADR 0007）

`check_app_update` 查询 GitHub Releases API（不走 tauri-plugin-updater——releases 不发布 latest.json，插件是死配置）。**release-only**：debug 构建直接返回 None，`lib.rs` 里 updater 插件注册也是 `#[cfg(not(debug_assertions))]`。要在真机验证此类功能必须 `pnpm tauri build`。

## 关键约定

- **版本号**：唯一事实来源是 `src-tauri/Cargo.toml`，`release.sh` 会同步到 `tauri.conf.json`（package.json 保持 0.0.0）。`RELEASE_NOTES.md` 中 `{version}` 占位符会被替换
- **镜像源**：默认 `https://nodejs.org/dist/index.json`；`VersionUrls::derive_dist_url` 从 index URL 推导 dist 前缀，改镜像时两条 URL 都会更新
- **Windows 差异**：符号链接需管理员权限；**所有子进程必须加 `creation_flags(0x0800_0000)`**（CREATE_NO_WINDOW，隐藏控制台窗口）——现有 `start_dev_server`、`list_git_branches`、`checkout_branch` 都这么做，新增的也要；`cfg(windows)` 门控的平台代码（winreg、taskkill）不要破坏 macOS 编译
- **IPC 返回字段用 snake_case**，前端 `src/types/index.ts` 的接口原样镜像
- **tdesign 插件 API**（`MessagePlugin`、`DialogPlugin`）显式 import；组件通过 `TDesignResolver` 自动导入，不用手动 import
- **改动范围提示**：`tauri.conf.json` 的 `devUrl` 与 vite `strictPort: 5199` 必须一致；新增窗口/能力要在 `src-tauri/capabilities/default.json` 中声明
- **发布流程**见记忆：打包产物只传 NSIS + MSI，不打 latest.json；本机无 gh CLI，用 GCM token 走 API；先改版本号再打包
