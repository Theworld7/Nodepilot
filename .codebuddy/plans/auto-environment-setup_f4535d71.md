---
name: auto-environment-setup
overview: 彻底移除 SetupGuide 引导步骤，在应用首次启动时自动且透明地将 nodepilot 管理的 Node.js 注入系统 PATH，包含竞争管理器检测/禁用、全平台支持（macOS launchd + Windows 注册表）、失败自动回滚与原生对话框提示。
todos:
  - id: add-deps
    content: 在 Cargo.toml 中添加 macOS 平台 plist crate 和 Windows 平台 winreg crate 依赖
    status: completed
  - id: create-env-setup
    content: 新建 src-tauri/src/env_setup.rs，实现 setup/rollback 主逻辑、竞争管理器检测、macOS launchd plist 生成与卸载、Windows 注册表 PATH 注入与还原、shell 配置文件修改与恢复
    status: completed
    dependencies:
      - add-deps
  - id: update-lib-rs
    content: 修改 lib.rs：注册 env_setup 模块，setup 闭包中调用 env_setup::setup，setup_flag 重命名为 auto_setup_flag，失败时通过 tauri_plugin_dialog 展示重试/跳过对话框
    status: completed
    dependencies:
      - create-env-setup
  - id: update-commands
    content: 修改 commands.rs：删除 is_first_run 和 mark_setup_done 命令，AppState.setup_flag 重命名，新增 auto_setup 命令供前端手动重试
    status: completed
    dependencies:
      - create-env-setup
  - id: update-app-vue-delete-setup-guide
    content: 修改 App.vue 移除 SetupGuide 导入、showSetup 状态、is_first_run 调用，直接渲染 VersionListPanel；删除 src/panels/SetupGuide.vue 文件
    status: completed
    dependencies:
      - update-commands
---

## 用户需求

彻底移除 nodepilot 的初始引导步骤（SetupGuide.vue），解决其对非程序员用户不友好的问题。安装完成后，应用必须自动且无缝地接管操作系统的 Node.js 环境，无需用户进行任何手动配置。

## 产品概述

将当前"首次启动弹引导页 → 用户手动编辑 shell 配置 → 点击完成"的流程，替换为"应用首次启动时静默自动接管系统 PATH → 失败时原生对话框回滚"的零交互体验。

## 核心功能

- **自动 PATH 注入**：macOS 通过 launchd agent (`~/Library/LaunchAgents/com.nodepilot.env.plist`) 注入 `~/.nodepilot/current/bin` 到终端环境 PATH；Windows 通过 HKCU 注册表 `Environment\PATH` 追加 nodepilot bin 路径。全程无需 root 权限。
- **竞争管理器禁用**：检测并临时禁用 nvm、fnm、volta、Homebrew Node 等已有版本管理器，注释其在 `.zshrc`/`.bashrc`/PowerShell Profile 中的初始化钩子，确保 nodepilot 版本优先。
- **自动回滚**：写入成功但验证失败时，自动删除 launchd plist/注册表项，恢复被注释的 shell 配置行，保持系统原样。
- **失败提示**：接管失败时通过 Tauri 原生对话框展示错误信息，提供"重试"和"跳过"两个操作选项。
- **幂等性保障**：`~/.nodepilot/.auto-setup-done` 标记文件防止重复尝试。

## 技术栈选型

| 层级 | 技术 | 说明 |
| --- | --- | --- |
| 框架 | Tauri 2 (Rust + Vue 3) | 沿用现有架构 |
| macOS PATH | launchd plist (`plist` crate) | 用户级 Agent，避免 root |
| macOS shell | 直接读写 `.zshrc`/`.bashrc` | 注释竞争管理器行 |
| Windows PATH | `winreg` crate (HKCU) | 用户级注册表，免 admin |
| Windows shell | PowerShell Profile 文件操作 | 注释 fnm/nvm 行 |
| 失败提示 | tauri-plugin-dialog (已集成) | 原生系统对话框 |
| 容错 | `#[cfg(target_os)]` 条件编译 | macOS/Windows 代码分离 |


## 实现方案

### 整体策略

在 `lib.rs` 的 `setup` 闭包中，于窗口/托盘创建**之前**插入环境自动设置步骤。因为是首次启动前的同步操作，使用 `std::fs`（非 tokio）避免阻塞事件循环。设置成功则创建 `.auto-setup-done` 标记文件，失败则展示系统对话框让用户选择重试或跳过。

### 新模块 `env_setup.rs` 设计

```
env_setup
├── pub fn setup(nodepilot_dir: &Path) -> Result<(), EnvSetupError>
│   1. 检测竞争管理器（nvm/fnm/volta/brew node）
│   2. 平台特定 PATH 注入
│   3. 注释 shell 配置中的竞争管理器钩子
│   4. 写 .auto-setup-done 标记
├── fn rollback(nodepilot_dir: &Path) 
│   1. 移除 launchd plist / 注册表项
│   2. 恢复 shell 配置中被注释的行
│   3. 删除 .auto-setup-done 标记
├── mod macos
│   ├── install_launchd_agent()  - 写 ~/Library/LaunchAgents/com.nodepilot.env.plist
│   ├── uninstall_launchd_agent()- 删除 plist + launchctl unload
│   ├── modify_shell_rc()        - 注释 nvm/fnm/volta 行
│   └── restore_shell_rc()       - 取消注释恢复原样
├── mod windows
│   ├── modify_registry_path()   - 读写 HKCU\Environment\PATH
│   ├── rollback_registry_path() - 还原注册表
│   ├── modify_ps_profile()      - 注释 PowerShell Profile 中的竞争管理器行
│   └── restore_ps_profile()     - 取消注释
└── CompetingManager 检测
    ├── 检测 .nvm/ 目录 → nvm
    ├── 检测 fnm 二进制/目录
    ├── 检测 ~/.volta/ 目录
    └── 检测 /usr/local/bin/node 为 brew 安装
```

### 关键技术决策

1. **launchd plist 而非 /etc/paths.d**：免 root 权限，对非程序员用户完全透明
2. **注释而非删除竞争管理器行**：回滚时精确恢复，不破坏用户原有配置
3. **同步执行**：setup 闭包中 `std::fs`，避免 tokio 嵌套运行时问题
4. **条件编译**：`#[cfg(target_os = "macos")]` / `#[cfg(target_os = "windows")]` 隔离平台代码
5. **`setup_flag` → `auto_setup_flag` 重命名**：语义对齐新行为

### 异常处理与回滚

```
setup() 失败路径：
  ├── 竞争管理器检测失败 → 跳过禁用步骤，仅做 PATH 注入
  ├── PATH 注入失败 → rollback() → 展示错误对话框（重试/跳过）
  ├── shell 配置修改失败 → rollback() → 展示错误对话框
  └── .auto-setup-done 写入失败 → rollback() → 展示错误对话框
```

对话框通过 `tauri_plugin_dialog::blocking::MessageDialog` 在 setup 闭包中同步调用，提供两个按钮："重试"（再次调用 setup）和"跳过"（写入 .auto-setup-done 标记为失败状态）。

## 目录结构

```
nodepilot/
├── src-tauri/
│   ├── Cargo.toml                          # [MODIFY] 添加 plist crate（macOS）和 winreg crate（Windows）
│   ├── src/
│   │   ├── lib.rs                          # [MODIFY] 添加 env_setup 模块声明；setup 闭包中调用 env_setup::setup；setup_flag 重命名为 auto_setup_flag；移除 is_first_run/mark_setup_done 的 handler 注册
│   │   ├── commands.rs                     # [MODIFY] AppState.setup_flag → auto_setup_flag；删除 is_first_run() 和 mark_setup_done() 两个 #[tauri::command]；新增 auto_setup() 命令供前端手动重试
│   │   └── env_setup.rs                    # [NEW] 核心环境设置模块，包含：
│   │                                       #   - detect_competing_managers() 检测 nvm/fnm/volta/brew node
│   │                                       #   - pub fn setup(nodepilot_dir) → Result 主入口
│   │                                       #   - pub fn rollback(nodepilot_dir) → 回滚所有修改
│   │                                       #   - #[cfg(target_os="macos")] macos 子模块：launchd plist 创建/删除、.zshrc/.bashrc 修改/恢复
│   │                                       #   - #[cfg(target_os="windows")] windows 子模块：注册表读写、PowerShell Profile 修改/恢复
│   │                                       #   - EnvSetupError 错误类型：Permission(平台权限)、Io(文件操作)、Parse(配置解析)
│   │                                       #   - CompetingManager 结构体：name, config_path, line_pattern 用于检测和恢复
│   └── ...
├── src/
│   ├── App.vue                             # [MODIFY] 移除 SetupGuide 组件导入和引用；移除 showSetup/loading 状态；移除 onMounted 中的 is_first_run 调用；直接渲染 VersionListPanel
│   └── panels/
│       └── SetupGuide.vue                  # [DELETE] 完全删除此文件，不再需要
└── CONTEXT.md                              # [MODIFY 已完成] 术语已更新：Current Symlink、Version Activation、新增 Automatic Environment Setup / Competing Version Manager / Environment Rollback
```

## 关键代码结构

```rust
// env_setup.rs - 核心类型定义

#[derive(Debug)]
pub enum EnvSetupError {
    Io(String),
    Plist(String),
    Registry(String),
    ShellConfig(String),
    Permission(String),
}

#[derive(Debug)]
pub struct CompetingManager {
    pub name: &'static str,           // "nvm", "fnm", "volta", "brew"
    pub config_paths: Vec<PathBuf>,   // 可能的 shell 配置文件路径
    pub line_matcher: fn(&str) -> bool, // 判断某行是否为该管理器的初始化钩子
}

pub fn setup(nodepilot_dir: &Path) -> Result<(), EnvSetupError> {
    let managers = detect_competing_managers();
    inject_path_platform(nodepilot_dir)?;       // 平台特定 PATH 注入
    disable_competing_managers(&managers)?;     // 注释竞争管理器钩子
    write_setup_flag(nodepilot_dir)?;           // 写 .auto-setup-done
    Ok(())
}

pub fn rollback(nodepilot_dir: &Path, managers: &[CompetingManager]) {
    remove_path_platform(nodepilot_dir);        // 删除 launchd plist / 注册表项
    restore_competing_managers(managers);       // 取消注释恢复
    remove_setup_flag(nodepilot_dir);           // 删 .auto-setup-done
}
```

```rust
// commands.rs - 修改后的 AppState
pub struct AppState {
    pub nodepilot_dir: PathBuf,
    pub manager: crate::version::VersionManager,
    pub auto_setup_flag: PathBuf,   // 原 setup_flag 重命名
    pub config_path: PathBuf,
    pub projects_path: PathBuf,
    pub servers: Mutex<HashMap<String, tokio::process::Child>>,
    pub log_buffers: Arc<Mutex<HashMap<String, Vec<String>>>>,
}
```

## 使用的 Agent 扩展

### Skill

- **grill-with-docs**
- 用途：在访谈过程中挑战计划设计，对照项目领域模型（CONTEXT.md）和架构决策（docs/adr/）打磨术语，确保"自动 PATH 注入"、"竞争管理器覆盖"、"环境回滚"等概念与现有领域语言一致
- 预期成果：CONTEXT.md 已更新为新术语，计划中的 7 项关键决策全部经过代码验证

### SubAgent

- **code-explorer**
- 用途：探索 env_setup.rs 需要交互的全部 Rust 依赖链（commands.rs 的 AppState 结构、lib.rs 的 setup 闭包、error.rs 的错误类型模式、version/ 模块的 trait 抽象风格），以及前端 App.vue 的完整依赖关系
- 预期成果：确认所有修改目标的精确行号和上下文，确保计划中的文件路径和 API 引用准确无误