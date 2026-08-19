# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

nodepilot — Tauri 2 桌面端 Node.js 版本管理器（Rust 后端 + Vue 3 前端）。管理 `~/.nodepilot/` 下的 Node.js 版本，为项目绑定版本、启动 dev server、常驻系统托盘。产品功能与目录结构见 `README.md`，领域术语见 `CONTEXT.md`，架构决策见 `docs/adr/`（新增架构决策需写 ADR）。

## 常用命令

```bash
pnpm dev                    # 仅前端（Vite dev server，端口 5199）
pnpm tauri dev              # 完整开发模式（Vite + 桌面应用）
pnpm build                  # 前端类型检查 + 构建（vue-tsc -b && vite build）
pnpm tauri build            # 打包（产物在 src-tauri/target/release/bundle/）
cd src-tauri && cargo check # Rust 类型检查
cd src-tauri && cargo test  # Rust 单元测试
cd src-tauri && cargo test <test_name>  # 跑单个测试
```

改动后验证：Rust 用 `cargo check`，前端用 `pnpm exec vue-tsc -b`（不需要跑完整 `pnpm build`）。

## 架构要点

### 分层与数据流

- **Rust 后端拥有全部版本管理逻辑**（ADR 0001），前端只通过 IPC 调用和展示。`commands.rs` 里的 `#[tauri::command]` 是唯一入口，全部在 `lib.rs` 的 `generate_handler!` 中注册。
- `version/` 模块：`VersionManager` 是外观（facade），内部拆成 fetcher（远程列表+缓存）/ installer（流式下载解压）/ activator（符号链接切换）/ deleter。
- **依赖注入**：`client.rs` 的 `HttpClient` trait 和 `fs.rs` 的 `FileSystem` trait 有生产实现（`HttpClientProd`/`FsProd`）和 `#[cfg(test)]` mock，单测用 mock。新增需要网络/文件的代码应沿用这两个 trait（如 `commands.rs` 的 `check_app_update` 复用 `HttpClientProd`）。
- 错误统一走 `error.rs` 的 `AppError`（serde tag="kind" 序列化），前端可拿到结构化错误。

### 双窗口与托盘

- 主面板 375×667 不可缩放；LogView 是同一个 webview 带 `?view=log` 参数的另一窗口（`App.vue` 里按 query 分支）。
- 关闭窗口 = 隐藏到托盘（`lib.rs` 的 `CloseRequested` 拦截），应用常驻。托盘图标动态显示当前 Node 主版本号（`tray.rs`），左键显示/聚焦主窗口。
- 主窗口每次打开都应刷新数据；版本列表有本地缓存，先展示缓存再后台刷新。

### 子进程与 Windows 特殊处理

- dev server / git 都是 `tokio::process::Command` shell out。**Windows 下所有子进程必须加 `creation_flags(0x0800_0000)`**（CREATE_NO_WINDOW，隐藏控制台窗口）——现有 `start_dev_server`、`list_git_branches`、`checkout_branch` 都这么做，新增的也要。
- dev server stdout/stderr 通过 Tauri 事件流式推送，按项目路径缓冲（上限 1000 行）。

### 环境配置（env_setup.rs）

首次启动静默执行：注入 PATH（macOS launchd + shell rc；Windows HKCU 注册表）、禁用竞品版本管理器（nvm/fnm/volta 的 shell hook）、失败自动回滚。状态标志在 `~/.nodepilot/.auto-setup-done` / `.auto-setup-error`。前端 `App.vue` onMounted 里处理重试/跳过对话框。

### 数据与配置

- `~/.nodepilot/config.json`：`AppConfig { mirror_url }`（Node.js 下载镜像，默认 nodejs.org）。
- `~/.nodepilot/projects.json`：项目绑定（version / path / name / 自定义启动命令等）。
- `~/.nodepilot/cache/versions.json`：远程版本列表缓存。

### 更新检查（ADR 0007）

`check_app_update` 查询 GitHub Releases API（不走 tauri-plugin-updater——releases 不发布 latest.json，插件是死配置）。**release-only**：debug 构建直接返回 None，`lib.rs` 里 updater 插件注册也是 `#[cfg(not(debug_assertions))]`。要在真机验证此类功能必须 `pnpm tauri build`。

## 约定

- **版本号两处**：`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 必须同步改；`package.json` 保持 `0.0.0` 不动。
- IPC 返回字段用 snake_case，前端 `src/types/index.ts` 的接口原样镜像。
- tdesign 插件 API（`MessagePlugin`、`DialogPlugin`）**显式 import**；组件通过 `TDesignResolver` 自动导入，不用手动 import。
- 注释、commit message、UI 文案用中文。
- 发布流程见记忆：打包产物只传 NSIS + MSI，不打 latest.json；本机无 gh CLI，用 GCM token 走 API；先改版本号再打包。
