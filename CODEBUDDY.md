# CODEBUDDY.md

为 CodeBuddy 在本仓库中工作时提供指引。
领域术语定义见 `CONTEXT.md`（版本、激活、离线策略等概念）。

## 常用命令

```bash
# 前端开发服务器
pnpm dev

# Tauri 开发模式（Vite + 桌面应用）
pnpm tauri dev

# 构建生产版本
pnpm tauri build

# 前端 TypeScript 检查
pnpm vue-tsc --noEmit

# 构建前端
pnpm build

# Rust 后端编译（仅检查）
cd src-tauri && cargo build

# Rust 单元测试
cd src-tauri && cargo test

# Rust lint
cd src-tauri && cargo clippy
```

> **注意**：前端使用 pnpm 管理依赖。Rust 后端位于 `src-tauri/`，通过 `tauri::process::Command` 管理的 Dev Server 子进程路径 PATH 取决于当前激活的 nodepilot Node 版本。

## 项目架构

### 整体架构

nodepilot 是一个桌面端 Node.js 版本管理器（GUI），基于 **Tauri 2** 构建。Rust 后端直接管理所有版本逻辑（获取列表、下载安装、符号链接切换、删除），Vue 3 前端通过 Tauri IPC（`invoke` + event listen）与之通信。UI 使用 tdesign-vue-next 组件库，窗口为手机尺寸 (~375×667)，常驻系统托盘。

### 数据目录

所有数据存储在 `~/.nodepilot/`：
- `versions/{version}/` — 已安装的 Node.js 发行版
- `current` — 指向当前激活版本的符号链接
- `cache/versions.json` — 远程版本列表的本地缓存
- `config.json` — 用户配置（镜像源 URL）
- `projects.json` — 项目绑定列表

### Rust 后端 (`src-tauri/src/`)

以 **Command 模式 + trait 抽象**组织：

- **`lib.rs`** — Tauri 应用入口，初始化 `AppState`（含 manager、servers、log_buffers 等），注册 IPC handler，设置系统托盘和窗口行为
- **`main.rs`** — 仅调用 `run()`，处理 Windows 子系统标志
- **`commands.rs`** — 所有 `#[tauri::command]` 处理器，是前后端的唯一接口层：
  - `AppState`：全局共享状态，使用 `Mutex` 保护 servers map 和 log_buffers
  - 版本管理：`get_versions`、`refresh_versions`、`install_version`、`activate_version`、`delete_version`
  - 项目绑定：`bind_project`、`unbind_project`、`get_project_bindings`
  - Dev Server：`start_dev_server`、`stop_dev_server`、`get_dev_server_logs`（启动子进程，逐行读取 stdout/stderr 并通过 Tauri event 推送到前端缓冲）
  - 配置：`get_config`、`set_config`、`is_first_run`、`mark_setup_done`
  - 初始检测：`read_package_json`、`detect_pm`
- **`version/`** — 版本管理的领域层：
  - `manager.rs`：`VersionManager` 封装所有版本操作，通过 `VersionCommand` 枚举分派到下层组件
  - `types.rs`：`NodeVersion`（前端展示）和 `RemoteVersion`（API 响应），含 LTS 反序列化逻辑
  - `fetcher.rs`：远程 version list 获取 + JSON 缓存读写，支持离线回退
  - `installer.rs`：下载 + 流式进度回调 + tar.gz/zip 解压，支持架构回退（arm64 → x64）
  - `activator.rs`：管理符号链接（`~/.nodepilot/current`），获取已安装/当前版本列表
  - `deleter.rs`：删除版本目录（阻止删除当前激活版本）
  - `event.rs`：进度事件类型定义 + `EventSink` trait
  - `error.rs`：领域错误类型（使用 thiserror）
- **`client.rs`** — HTTP 客户端抽象层：
  - `HttpClient` trait，`HttpClientProd`（reqwest 流式下载），`HttpClientMock`（测试 mock）
- **`fs.rs`** — 文件系统抽象层：
  - `FileSystem` trait，`FsProd`（真实文件系统），`FsMock`（内存测试 mock，支持符号链接模拟）
- **`tray.rs`** — 托盘图标生成（32×32 绿色圆角图标，使用 imageproc 渲染版本号）
- **`error.rs`** — 全局 `AppError`，统一 `VersionManagerError → AppError` 转换

**依赖注入**：`lib.rs` 实例化 `HttpClientProd` / `FsProd` 后以 `Arc<dyn Trait>` 注入 `VersionManager`。测试时替换为 mock 实现。

### Vue 3 前端 (`src/`)

- **`App.vue`** — 根组件，根据 URL 参数路由（`?view=log` → 日志窗口，`is_first_run` → 首次引导或版本列表）
- **`panels/`**
  - `VersionListPanel.vue` — 主面板：版本列表、搜索过滤、LTS 筛选、安装/激活/删除操作
  - `SetupGuide.vue` — 首次设置引导（添加 `~/.nodepilot/current/bin` 到 PATH 的指南）
  - `LogView.vue` — Dev Server 日志独立窗口
- **`components/`**
  - `VersionRow.vue` — 单个版本的折叠条目，展开后显示绑定的项目列表
  - `ProjectRow.vue` — 项目行，含启动/停止 Dev Server、查看日志按钮
  - `CodeBlock.vue` — 代码块组件（含复制按钮）
- **`composables/`**
  - `useVersionManager.ts` — 封装所有 Tauri IPC 调用（`invoke`）和 event 监听，提供响应式状态
  - `useTheme.ts` — 明暗主题切换
- **`types/index.ts`** — `NodeVersion`、`ProjectInfo` 接口定义
- **`style.css`** — 全局 CSS 变量（明暗双主题），tdesign-vue-next 样式覆盖

### 架构决策记录

关键决策见 `docs/adr/`：
- ADR-0001：Rust 后端自主管理所有版本逻辑
- ADR-0004：使用传统桌面窗口替代面板（后续选择，取代 ADR-0002）
- ADR-0003：动态托盘图标显示版本号

### 关于 Dev Server 子进程

`start_dev_server` 使用 `tokio::process::Command` 启动 npm/pnpm/yarn 命令，并将 `~/.nodepilot/current/bin` 注入子进程 PATH，确保打包应用中也能找到上述命令。子进程的 stdout/stderr 被异步读取并通过 Tauri event (`dev_server_log`) 推送到前端缓冲（上限 1000 行）。
