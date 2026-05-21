<div align="center">
  <img src="./public/favicon.svg" width="80" height="80" alt="nodepilot logo" />
  <h1>nodepilot</h1>
  <p><strong>Node.js 版本管理器 · 图形界面</strong></p>
  <p>
    <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue" alt="Platform" />
    <img src="https://img.shields.io/badge/built%20with-Tauri%202%20%7C%20Vue%203%20%7C%20Rust-green" alt="Built with" />
    <img src="https://img.shields.io/badge/license-MIT-yellow" alt="License" />
  </p>
</div>

---

**nodepilot** 是一款基于 Tauri 构建的桌面端 Node.js 版本管理工具。它常驻在系统托盘，以直观的图形界面替代繁琐的命令行操作——查看当前版本、安装新版本、切换版本、删除旧版本，一切皆可点击完成。

## ✨ 特性

- **🖥️ 系统托盘常驻** — 后台静默运行，托盘图标实时显示当前激活的 Node.js 主版本号
- **📱 弹出式面板** — 点击托盘图标弹出手机-sized 面板，版本列表一目了然
- **🔍 搜索过滤** — 按主版本号快速筛选，支持仅显示 LTS 版本
- **⬇️ 一键安装** — 点击安装按钮，自动下载、解压、放置到 `~/.nodepilot/versions/`
- **🔄 一键切换** — 点击激活按钮，自动更新 `~/.nodepilot/current` 符号链接，支持全局包迁移
- **🗑️ 一键删除** — 删除已安装版本释放磁盘空间，带确认保护
- **📦 全局包迁移** — 切换版本时自动检测并引导迁移全局 npm 包
- **🔄 自动启动** — 支持开机自启，始终可用
- **🛠️ 自定义镜像** — 支持配置自定义 Node.js 下载镜像源
- **🌐 离线可用** — 缓存版本列表，离线时仍可查看和切换已安装版本

## 🚀 快速开始

### 安装

从 [Releases 页面](https://github.com/Theworld7/Nodepilot/releases) 下载对应平台的安装包即可。

### 首次使用

安装后启动应用，会弹出设置引导：

1. 将以下命令添加到你的 shell 配置文件（`.zshrc` / `.bashrc`）中：

   ```bash
   export PATH="$HOME/.nodepilot/current/bin:$PATH"
   ```

2. 重新加载配置：

   ```bash
   source ~/.zshrc
   ```

3. 点击"我已设置完成"，然后就可以通过托盘图标管理 Node.js 版本了。

## 🎯 使用场景

| 场景 | 操作 |
|------|------|
| 查看当前 Node.js 版本 | 瞥一眼托盘图标即可 |
| 安装最新 LTS 版本 | 打开面板 → 点击「安装」|
| 项目需要 Node 18 | 搜索 "18" → 筛选 → 点击「激活」|
| 清理旧版本 | 点击「删除」→ 确认 |
| 切换后保留全局工具 | 自动弹出迁移提示 |

## 🧱 项目结构

```
nodepilot/
├── src/                      # Vue 3 前端
│   ├── App.vue               # 根组件（入口）
│   ├── panels/
│   │   ├── VersionListPanel.vue  # 版本列表面板
│   │   └── SetupGuide.vue       # 首次设置引导
│   ├── components/
│   │   └── CodeBlock.vue        # 代码块组件（带复制）
│   ├── types/
│   │   └── index.ts             # TypeScript 类型定义
│   └── main.ts
├── src-tauri/                # Rust 后端
│   └── src/
│       ├── main.rs           # 入口
│       ├── lib.rs            # Tauri 应用启动
│       ├── commands.rs       # IPC 命令
│       ├── tray.rs           # 托盘图标生成（动态绘制版本号）
│       └── version/
│           ├── mod.rs
│           ├── types.rs      # 数据结构
│           ├── fetcher.rs    # 远程版本列表获取与缓存
│           ├── installer.rs  # 下载与安装
│           ├── activator.rs  # 符号链接切换
│           └── deleter.rs    # 删除版本
├── docs/
│   ├── prd.md                # 产品需求文档
│   └── adr/                  # 架构决策记录
└── package.json
```

## 🏗️ 技术栈

| 层 | 技术 |
|----|------|
| 桌面框架 | [Tauri 2](https://v2.tauri.app/) |
| 后端语言 | Rust 2021 edition |
| 前端框架 | [Vue 3](https://vuejs.org/) + TypeScript |
| UI 组件库 | [tdesign-mobile-vue](https://tdesign.tencent.com/mobile-vue/) |
| 构建工具 | [Vite](https://vitejs.dev/) |
| 包管理器 | pnpm |

## 🛠️ 本地开发

### 前置要求

- [Rust](https://www.rust-lang.org/)（推荐使用 rustup 安装）
- [Node.js](https://nodejs.org/) ≥ 18
- [pnpm](https://pnpm.io/)

### 启动开发环境

```bash
# 安装前端依赖
pnpm install

# 启动 Tauri 开发模式（自动启动 Vite + 桌面应用）
pnpm tauri dev
```

### 构建

```bash
pnpm tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 🗺️ 数据目录

```
~/.nodepilot/
├── current -> versions/v24.1.2/    符号链接，指向当前激活的版本
├── versions/
│   ├── v18.20.0/
│   │   └── bin/node
│   └── v24.1.2/
│       └── bin/node
└── cache/
    └── versions.json               远程版本列表缓存
```

## 📄 架构决策

关键架构决策记录在 `docs/adr/` 中：

- [ADR-0001](docs/adr/0001-rust-owns-version-management.md) — Rust 后端自主管理所有版本逻辑
- [ADR-0002](docs/adr/0002-popup-window-panel.md) — 弹出式面板而非原生菜单
- [ADR-0003](docs/adr/0003-dynamic-tray-icon.md) — 动态托盘图标显示版本号

## 📝 待实现

- [ ] Windows 平台全面测试与优化
- [ ] 自动更新（Tauri updater 插件已集成，待配置签名）
- [ ] 更多镜像源预设
- [ ] 版本下载进度动画优化
- [ ] 国际化支持

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

[MIT](./LICENSE)
