---
name: fix-t-loading-webkit-tauri-rendering
overview: 修复 t-loading 组件在 Tauri/WebKit 环境下的渲染问题：1) loading 尺寸固定不随 size prop 变化; 2) 图标自转+公转双重旋转异常。
todos:
  - id: fix-webkit-css
    content: "在 style.css 中添加 .t-icon-loading 的 WebKit 兼容覆盖：transform-origin: center 修复旋转原点，强制 SVG 尺寸为 1em"
    status: completed
  - id: wrap-loading-state
    content: 在 VersionListPanel.vue 中用 .loading-state 容器包裹 t-loading 组件使其居中展示
    status: completed
  - id: build-verify
    content: 构建验证：pnpm build 确认编译通过且 CSS 输出正确
    status: completed
    dependencies:
      - fix-webkit-css
      - wrap-loading-state
---

## 问题描述

TDesign `t-loading` 组件在 Tauri（WebKit）环境下存在两个渲染缺陷：

1. `size` prop 失效，无论设置为多大值，loading 图标始终以最小尺寸显示
2. 图标出现双重旋转：绕自身中心「自转」的同时绕其左上角「公转」

## 根因

- `t-spin` 动画对 SVG 应用 `transform: rotate()`，但 WebKit 中 SVG 的默认 `transform-origin` 为 `0 0`（左上角）而非 `center`，导致公转效果
- SVG 的 `width="1em" height="1em"` 在 WebKit 的 `foreignObject` 上下文中可能无法正确继承父元素 `font-size`

## 修复目标

- 在 `style.css` 全局样式中添加 WebKit 兼容覆盖，修正旋转原点和尺寸继承
- 复用已有的 `.loading-state` CSS 类包裹 loading 组件，使其居中展示

## 技术方案

### 修复策略

通过全局 CSS 覆盖修复 `.t-icon-loading` 在 WebKit 下的两个问题：

1. **修正旋转原点**：设置 `transform-origin: center center`，使绕自身中心旋转，消除公转
2. **确保尺寸继承**：显式设置 SVG `width: 1em; height: 1em` 强制从父元素 `font-size` 继承尺寸

### 修改文件

| 文件 | 变更 |
| --- | --- |
| `src/style.css` | 新增 `.t-icon-loading` 和 `.t-loading` 的 WebKit 兼容覆盖 |
| `src/panels/VersionListPanel.vue` | 用 `.loading-state` 容器包裹 `t-loading`（复用已有 CSS） |


### 关键代码结构

```
style.css 新增覆盖:
├── .t-icon-loading { transform-origin: center center; }     // 修复旋转原点
├── .t-icon-loading svg { width: 1em !important; height: 1em !important; }  // 强制尺寸继承
└── .t-loading--center { display: flex; justify-content: center; }  // 确保居中
```