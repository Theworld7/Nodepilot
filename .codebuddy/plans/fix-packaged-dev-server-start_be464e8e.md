---
name: fix-packaged-dev-server-start
overview: 修复打包后启动开发服务失效的问题：在 spawn 子进程时将 nodepilot 管理的 Node bin 目录加入 PATH 环境变量，确保 npm/pnpm/yarn 可被找到。
todos:
  - id: add-nodepilot-dir-to-state
    content: 在 AppState 结构体中新增 nodepilot_dir 字段，并在 lib.rs 初始化时传入 nodepilot_dir
    status: completed
  - id: inject-path-in-start-server
    content: 在 start_dev_server 中构建 ~/.nodepilot/current/bin 路径并注入子进程 PATH 环境变量
    status: completed
    dependencies:
      - add-nodepilot-dir-to-state
  - id: build-and-verify
    content: 编译打包验证，确保打包后 dev server 正常启动
    status: completed
    dependencies:
      - inject-path-in-start-server
---

## 问题描述

打包后的 macOS 应用无法启动开发服务（Dev Server），但在开发环境下功能正常。

## 根因分析

macOS 打包应用的 PATH 环境变量仅限于 `/usr/bin:/bin:/usr/sbin:/sbin`，不包含 `~/.nodepilot/current/bin`。`start_dev_server` 命令（`commands.rs:315`）使用 `tokio::process::Command::new("npm")` 等裸命令名启动子进程，打包后系统无法找到 `npm`/`pnpm`/`yarn` 等可执行文件，导致启动失败。

## 修复范围

- `src-tauri/src/commands.rs`：`AppState` 新增 `nodepilot_dir` 字段；`start_dev_server` 中注入 `~/.nodepilot/current/bin` 到子进程 PATH
- `src-tauri/src/lib.rs`：`AppState` 初始化时传入 `nodepilot_dir`

## 技术方案

### 修改策略

在 `AppState` 中保存 `nodepilot_dir`（即 `~/.nodepilot`），在 `start_dev_server` 中构造 `{nodepilot_dir}/current/bin` 路径，通过 `.env("PATH", new_path)` 将其**前置**到子进程的 PATH 环境变量中。

### 关键实现细节

**1. AppState 新增字段（commands.rs 第14行）**

```rust
pub struct AppState {
    pub nodepilot_dir: PathBuf,  // 新增
    pub manager: crate::version::VersionManager,
    pub setup_flag: PathBuf,
    pub config_path: PathBuf,
    pub projects_path: PathBuf,
    pub servers: Mutex<HashMap<String, tokio::process::Child>>,
    pub log_buffers: Arc<Mutex<HashMap<String, Vec<String>>>>,
}
```

**2. start_dev_server PATH 注入（commands.rs 第315行附近）**

在 `tokio::process::Command::new(program)` 构建后、`.spawn()` 前，添加 PATH 注入逻辑：

```rust
// 构造扩展 PATH：nodepilot bin 目录 + 原有 PATH
let nodepilot_bin = state.nodepilot_dir.join("current").join("bin");
let existing_path = std::env::var("PATH").unwrap_or_default();
let new_path = format!("{}:{}", nodepilot_bin.display(), existing_path);

let mut child = tokio::process::Command::new(program)
    .args(&args)
    .current_dir(&project_dir)
    .env("PATH", &new_path)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .process_group(0)
    .spawn()
    .map_err(|e| AppError::Io(format!("failed to start dev server: {e}")))?;
```

**3. lib.rs 初始化适配（lib.rs 第56行）**

```rust
let state = AppState {
    nodepilot_dir: nodepilot_dir.clone(),  // 新增
    manager,
    setup_flag,
    config_path,
    projects_path,
    servers: std::sync::Mutex::new(std::collections::HashMap::new()),
    log_buffers: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
};
```

### 边界情况处理

- `~/.nodepilot/current/bin` 目录不存在时（无已激活版本），PATH 仍保留原有值，系统会尝试从默认 PATH 查找命令
- 使用 `std::env::var("PATH").unwrap_or_default()` 安全获取当前 PATH，避免 panic
- `nodepilot_bin` 使用 `PathBuf::join` 保证跨平台路径分隔符正确性

### 性能考量

- PATH 拼接仅在 `start_dev_server` 调用时执行一次，无运行时开销
- 不修改进程全局环境变量，不影响其他命令