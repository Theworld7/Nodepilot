---
name: tdesign-mobile-to-desktop-migration
overview: 将 UI 框架从 tdesign-mobile-vue 一次性替换为 tdesign-vue-next，窗口尺寸放宽至 500×700，采用桌面原生组件风格，创建 ADR 0005 记录架构决策。
todos:
  - id: update-configs
    content: 替换依赖配置：package.json 添加 tdesign-vue-next 移除 tdesign-mobile-vue；vite.config.ts 将 TDesignResolver library 改为 vue-next；main.ts CSS 导入路径改为 vue-next；tauri.conf.json 窗口尺寸改为 500×700
    status: completed
  - id: install-deps
    content: 执行 pnpm install 安装新依赖并自动更新 lockfile
    status: completed
    dependencies:
      - update-configs
  - id: replace-components
    content: 替换组件：VersionListPanel.vue 的 PullDownRefresh→刷新按钮、Search→Input+prefixIcon、DialogPlugin 导入源；ProjectRow.vue 的 Popup→Drawer(bottom) 并适配插槽结构
    status: completed
    dependencies:
      - install-deps
  - id: fix-css-overrides
    content: 修复 CSS 覆盖：在 style.css 中桥接 --brand CSS 变量；对照桌面版 BEM 类名修复 4 个 Vue 文件中的 :deep() 选择器；删除 PullDownRefresh 和 Search 相关的旧覆盖
    status: completed
    dependencies:
      - replace-components
  - id: update-docs
    content: 使用 [skill:grill-with-docs] 更新文档：CONTEXT.md、CODEBUDDY.md、README.md、docs/prd.md 中的 UI 库引用；创建 docs/adr/0005-tdesign-desktop-migration.md
    status: completed
    dependencies:
      - fix-css-overrides
  - id: build-verify
    content: 构建验证：pnpm tauri build 确认编译通过、窗口尺寸正确、组件渲染正常
    status: completed
    dependencies:
      - update-docs
---

## 用户需求

将 nodepilot 前端的 UI 框架从 `tdesign-mobile-vue` 替换为 `tdesign-vue-next`，同时将窗口尺寸从 375×667 放宽至 500×700。

## 核心动机

当前应用已是传统桌面窗口（ADR 0004），但使用触摸优化的移动端组件库，交互模式不协调。切换到桌面端组件库后，组件交互更符合桌面用户预期（如正常的 hover 状态、更大的点击区域、非触摸手势操作）。

## 核心变更

### 依赖替换

- `tdesign-mobile-vue@^1.14.1` → `tdesign-vue-next`（同大版本系列，API 高度相似）
- `@tdesign-vue-next/auto-import-resolver` 的 `library` 参数从 `mobile-vue` 改为 `vue-next`

### 组件替换

- `<t-pull-down-refresh>` 删除，替换为顶部搜索栏旁边的刷新按钮（`<t-button>` + RefreshIcon）
- `<t-search>` 替换为 `<t-input>` + prefixIcon（SearchIcon）+ clearable，保持圆角搜索框外观
- `<t-popup placement="bottom">` 替换为 `<t-drawer placement="bottom">`，从底部展开项目设置面板
- 其余 10 个组件（Button、Input、Collapse、CollapsePanel、Divider、Loading、Progress、Radio、RadioGroup、Switch、Tag）保持同名，自动导入机制无需改动
- `DialogPlugin` 从新库导入，API 一致

### CSS 适配

- 全局样式导入路径变更：`tdesign-mobile-vue/es/style/index.css` → `tdesign-vue-next/es/style/index.css`
- 替换 `tdesign-mobile-vue` 提供的 CSS 变量（如 `--brand`、`--brand-active`、`--gray-color-*` 等）为 `tdesign-vue-next` 的 Design Tokens（`--td-brand-color` 系列）
- 修复所有 `:deep()` 覆盖以匹配桌面版 BEM 类名

### 窗口尺寸

- 宽度 375 → 500，高度 667 → 700

## 技术栈

- **前端框架**: Vue 3 + TypeScript
- **新 UI 库**: tdesign-vue-next (桌面端)
- **图标库**: tdesign-icons-vue-next (保持不变)
- **自动导入**: unplugin-vue-components + unplugin-auto-import + @tdesign-vue-next/auto-import-resolver
- **构建工具**: Vite 8
- **桌面壳**: Tauri 2

## 实现方案

### 总体策略

一次性全换：在同一批次中完成 npm 依赖替换、Vite 配置修改、所有组件替换、CSS 适配和文档更新。由于 tdesign-mobile-vue 和 tdesign-vue-next 共享底层设计体系（TDesign），组件 API 高度一致，一次性替换的回归风险可控。

### 关键技术决策

1. **自动导入机制不变**: `unplugin-vue-components` 的 `TDesignResolver` 同时支持 `mobile-vue` 和 `vue-next` 两种 library，仅需修改参数即可切换。组件在模板中通过 `T` 前缀自动识别（如 `<t-button>`），无需逐个手动导入。

2. **CSS 变量迁移策略**: `tdesign-mobile-vue` 的 CSS 变量命名规则（如 `--brand`、`--brand-active`、`--gray-color-1`）与 `tdesign-vue-next` 的 Design Tokens（`--td-brand-color`、`--td-brand-color-active`、`--td-gray-color-1`）不同。项目中 `src/components/ProjectRow.vue` 使用了 `var(--brand)`，需要：

- 在 `src/style.css` 的 `:root` 中定义 `--brand: var(--td-brand-color)` 作为桥接变量
- 确保 tdesign-vue-next 的主题 CSS 变量在全局生效

3. **`:deep()` 类名对照**: 两个库的 BEM 类名有差异，常见映射：

- `.t-collapse-panel` → 相同（同名）
- `.t-collapse-panel__header-icon` → 可能变为 `.t-collapse-panel__header--icon` 或相同，需实际构建后验证
- `.t-collapse-panel__content` → 相同
- `.t-button__icon` → 相同
- `.t-button__content` → 相同
- `.t-divider--vertical` → 相同
- `.t-pull-down-refresh` → 删除（组件不再存在）
- `.t-search__input-box` → `.t-input__wrap` 或类似（Search→Input 替换）
- `.t-radio-group` → 相同

4. **Drawer 替换 Popup**: `t-drawer` 通过 `placement="bottom"` 实现底部滑出效果，其内容结构不同于 `<t-popup>` 的 `#content` 插槽。需要将设置面板内容直接放在 `<t-drawer>` 的默认插槽中（而非命名插槽），底部按钮区域可使用 `#footer` 插槽。

### 性能考虑

- 自动导入不变，按需加载机制不变，bundle 体积不会显著增加（tdesign-vue-next 组件可能略大，但仍在合理范围）
- 全局 CSS 从 mobile 版切到桌面版，CSS 文件大小相当
- 无新增运行时开销

## 实现要点

### 执行顺序（防止中间损坏状态）

1. 先改配置文件（package.json、vite.config.ts、tauri.conf.json、main.ts）
2. 安装新依赖 `pnpm install`
3. 逐个替换组件
4. 修复 CSS 覆盖
5. 更新文档
6. 构建验证

### 回归风险控制

- `DialogPlugin.confirm` API 在 v1.14.x mobile 和 latest vue-next 之间基本一致（`title`、`content`、`confirmBtn`、`cancelBtn`、`onConfirm`、`onClose`），可直接从新库导入
- `DialogPlugin` 的样式（按钮颜色、间距）在桌面版可能略有差异，属于预期行为
- 12 个 `tdesign-icons-vue-next` 图标零改动

### CSS 变量桥接

在 `src/style.css` 的 `:root` 和 `.dark` 块中添加 `tdesign-vue-next` 提供的品牌色映射：

```css
:root {
  --brand: var(--td-brand-color, #0052d9);
  --brand-active: var(--td-brand-color-active, #0034b5);
}
:root.dark {
  --brand: var(--td-brand-color, #4582e6);
  --brand-active: var(--td-brand-color-active, #699ef5);
}
```

这样项目中已有的 `var(--brand)` 引用无需逐个修改。

## Agent Extensions

### Skill

- **grill-with-docs**
- 用途: 已将访谈结论固化到文档中——更新 CONTEXT.md 的 Frontend 定义，创建 ADR 0005 记录迁移决策
- 预期结果: CONTEXT.md 第 68 行更新为 tdesign-vue-next；新建 `docs/adr/0005-tdesign-desktop-migration.md` 记录动机、决策和后果