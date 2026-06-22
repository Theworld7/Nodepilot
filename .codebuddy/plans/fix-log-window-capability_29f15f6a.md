---
name: fix-log-window-capability
overview: 修复 Tauri 2 安全能力限制导致日志窗口无法监听事件的问题：扩展 capabilities 的 windows 列表 + LogView.vue 增加 try-catch 防御
todos:
  - id: fix-capability-windows
    content: 修改 capabilities/default.json，将 windows 从 ["main"] 扩展为 ["main", "log-*"]，允许日志窗口使用 listen 权限
    status: completed
  - id: fix-logview-listen-fallback
    content: 修改 LogView.vue，将 listen() 包入 try-catch，失败时降级为 setInterval 轮询 get_dev_server_logs
    status: completed
---

## 问题诊断

打包后日志窗口持续显示「等待日志...」且无法输出实时日志，从四个维度排查如下：

### 1. 连接/权限（根本原因）

Tauri 2 的 `capabilities/default.json` 中 `"windows": ["main"]` 将所有 IPC 权限（包括 `listen`）限定在主窗口。日志窗口通过 `VersionRow.vue` 中使用 `new WebviewWindow(label)` 创建，label 格式为动态生成的 `log-<hash>`（如 `log-d18klt`）。当 `LogView.vue` 调用 `listen("dev_server_log")` 时，Tauri 安全策略因窗口 label 不在白名单中直接拦截，抛出 `event.listen not allowed on window "log-d18klt"`。

### 2. 日志流推送机制（正常）

Rust 侧使用 `app_handle.emit("dev_server_log", ...)` 全局广播事件，日志缓冲 `log_buffers` 正常写入，`get_dev_server_logs` 命令正常读取。之前接入的 PTY 方案（`script -q /dev/null`）确保子进程输出实时可达。

### 3. 前端渲染逻辑（受权限问题连累）

`listen()` 在 `onMounted` 中位于 try-catch 外部，权限拦截导致未经捕获的 Promise 拒绝直接中断 `onMounted` 执行流，后续 `get_dev_server_logs` 永不调用。`logs` 保持空数组 `[]`，模板因此持续渲染「等待日志...」状态。

### 4. 进程/权限（正常）

PTY 子进程以用户身份运行，`~/.nodepilot/` 读写权限完好，非问题所在。

## 修复方案

| 文件 | 修改 | 目的 |
| --- | --- | --- |
| `capabilities/default.json` | `windows` 从 `["main"]` 改为 `["main", "log-*"]` | 允许日志窗口使用 `listen` 等 IPC 权限 |
| `LogView.vue` | `listen()` 包入 try-catch，失败时降级为轮询 `get_dev_server_logs` | 防止 listen 失败阻断整个日志功能 |


## 技术方案

### 修复 1：扩展 Tauri Capability 白名单

Tauri 2 的 capability 系统通过 `windows` 字段按窗口 label 控制权限范围，支持 glob 通配符。

**修改文件**：`src-tauri/capabilities/default.json`

**变更**：将 `"windows": ["main"]` 改为 `"windows": ["main", "log-*"]`

`log-*` 通配符会匹配所有以 `log-` 为前缀的动态 label（`safeLabel()` 生成格式为 `log-{hash36}`），使日志窗口继承 `default` capability 中的所有权限（`core:default` 包含 `event:allow-listen`、`event:default` 等）。

**注意**：Tauri 在读取 capability 后会将其缓存。修改 `default.json` 后需执行 `cargo clean` 或删除 `target/` 中的生成 schema 确保生效，最稳妥的方式是完整重新打包。

### 修复 2：前端降级处理

**修改文件**：`src/panels/LogView.vue`

**策略**：

1. 将 `listen()` 包入 try-catch，catch 中设置 `unlisten = null` 并输出错误日志
2. 无论 listen 成功与否，都执行 `get_dev_server_logs` 获取缓冲日志
3. 若 listen 失败，启动一个 `setInterval` 轮询 `get_dev_server_logs`（间隔 2s）作为降级方案
4. `onUnmounted` 中同时清理 `unlisten` 和 `clearInterval`

这样即使未来 capability 配置再次出问题，用户仍能看到日志（通过轮询，稍有延迟但不会白屏）。

### 安全性评估

- `log-*` 通配符仅扩大日志窗口的权限，主窗口权限不变
- 日志窗口由代码内部控制创建（非用户触发），不存在恶意窗口注入风险
- 日志窗口获取的权限与主窗口一致（命令调用、事件监听），属于正常功能需求