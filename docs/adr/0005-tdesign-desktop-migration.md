# ADR 0005: 从 tdesign-mobile-vue 迁移到 tdesign-vue-next

## Status
Accepted

## Context
应用原本使用 `tdesign-mobile-vue`，因为最初设计为手机尺寸的 popup 面板（ADR 0002）。后来 ADR 0004 将窗口改为常规桌面窗口（有标题栏、不自动隐藏），但 UI 组件库仍为移动端风格——触摸优化的尺寸、手势交互（如下拉刷新）、缺少桌面原生交互反馈（hover 状态等）。这种桌面壳 + 移动组件的组合造成了交互模式矛盾。

## Decision
将前端 UI 框架从 `tdesign-mobile-vue` 替换为 `tdesign-vue-next`（TDesign 桌面端组件库），同时将窗口尺寸从 375×667 适度放宽至 500×700。

### 组件映射
- 10 个同名组件（Button、Input、Collapse、Divider、Loading、Progress、Radio、RadioGroup、Switch、Tag）直接映射
- `<t-pull-down-refresh>` → 顶部刷新按钮（桌面端无下拉刷新概念）
- `<t-search>` → `<t-input>` + SearchIcon 前缀（桌面端无独立 Search 组件）
- `<t-popup>` → `<t-drawer placement="bottom">`（项目设置面板）

### 视觉策略
拥抱 tdesign-vue-next 桌面默认尺寸，不刻意缩小组件。通过 Design Tokens 控制全局风格，必要时用 `:deep()` 做精确覆盖。

## Consequences
Positive:
- 组件交互符合桌面用户预期（hover 状态、更大的点击区域、非触摸手势）
- tdesign-vue-next 的 Design Token 系统更完善，主题定制更灵活
- 消除了移动端下拉刷新等不协调的手势交互

Negative:
- 500×700 窗口内可展示的版本行数略少于原 375×667（组件间距更大）
- `<t-search>` 替换为 `<t-input>` 后无内置搜索图标行为，需手动实现 prefix-icon

## Supersedes
- ADR 0002 中 "tdesign-mobile fits the phone form factor better" 的假设（窗口已不再是手机尺寸）

## Rejected Alternatives
- **保留 tdesign-mobile-vue 但不放宽窗口**：交互模式矛盾持续存在，hover 状态缺失
- **渐进式迁移（两库共存）**：增加依赖体积，双份样式可能冲突，一次性替换风险可控
