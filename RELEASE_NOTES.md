## v0.3.0

### 变更

- **应用更新检查**：启动时从 GitHub Releases 检测新版本，主面板顶部出现「有新版本 vX.Y.Z」提示条，点击跳转到下载页（ADR 0007）。debug 构建不检查；release-only 功能，验证需 `pnpm tauri build` 后跑真机
- **PATH 按项目版本注入**：`start_dev_server` 不再用全局 `current` 符号链接，而是从 `projects.json` 读出项目绑定的 Node 版本，把 `~/.nodepilot/versions/<version>/` 注入子进程 PATH。修复 v14 项目误跑 v24 `npm install` 时 `deasync` 报 `spawn EINVAL` 的问题
- **PATH 日志换行展示**：`[nodepilot] PATH:` 日志条目按 PATH 分隔符切分，每条路径一行，便于在日志窗口阅读
- **配置抽屉 UI 调整**：把「该项目无可用脚本，请在下方输入自定义命令名」提示放到「默认执行命令」选择框下方（用 `#help` 槽）；「自定义命令名」与「默认执行命令」v-model 分离，键入不再回写到下拉框；「命令前缀」「自定义启动命令」按用户规则显示
- **配置抽屉保存容错**：t-select 清空会把 v-model 设为 `undefined`，saveSettings 加 `?? ""` 兜底避免 `.trim()` 抛 `TypeError`；catch 改为 toast 让用户能看到错误而不是只 console.error
- **图标圆角修复**：Windows 托盘图标改为透明圆角，`build.rs` 监听 `icon.ico` 变化触发重新编译
- **dev server 进程树终止**：停止时递归终止整棵进程树，避免 PTY 让内层进程 `setsid` 到新进程组后遗留孤儿服务
- **dev server 秒退修复（Windows）**：Windows 下用 `cmd /C` 包装命令以正确找到 `.cmd`，PATH 分隔符与目录按平台修正
- **日志窗口**：新增清理功能、可点击链接、运行状态同步

### 文档

- 新增 `CLAUDE.md` / `AGENTS.md`：架构总览、依赖注入约定、dev server 子进程坑点、Windows 差异、版本号单一来源约定
- `docs/adr/` 文档修订

### 安装

Windows 用户下载安装包（二选一）：

- `nodepilot_0.3.0_x64-setup.exe`（NSIS 安装程序）
- `nodepilot_0.3.0_x64_zh-CN.msi`（MSI 安装程序）

双击运行，按提示完成安装即可。安装后从系统托盘启动 nodepilot。

---

## v0.2.7

### 变更

- **修复主题显示冲突**：修复 Windows 系统深色模式与应用内浅色设置（或反之）不一致时界面颜色错乱的问题。深色模式现在完全由应用内切换开关控制，不再受系统 `prefers-color-scheme` 媒体查询干扰（首次启动仍跟随系统主题作为默认值）
- **组件库主题同步**：tdesign-vue-next 组件（按钮、对话框、表单等）的主题模式与应用设置保持同步，不再各自跟随系统主题，避免自定义样式与组件库样式不一致
- **自定义启动命令**：项目设置抽屉新增「自定义启动命令」，可配置任意启动命令（如 `dsh web`、`pnpm dsh web`），▶ 按钮在项目目录下原样执行并接管日志与停止，无需依赖 npm script；设置后优先于「默认执行命令 + 命令前缀」，未配置时行为不变

### 安装

Windows 用户下载安装包（二选一）：

- `nodepilot_0.2.7_x64-setup.exe`（NSIS 安装程序）
- `nodepilot_0.2.7_x64_zh-CN.msi`（MSI 安装程序）

双击运行，按提示完成安装即可。安装后从系统托盘启动 nodepilot。
