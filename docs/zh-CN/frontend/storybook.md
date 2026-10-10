# Storybook

## 仓库模型

| 位置                                   | 所有权                                                                       |
| -------------------------------------- | ---------------------------------------------------------------------------- |
| `apps/storybook/.storybook/`           | 唯一仓库 Storybook 宿主：框架、Rsbuild、addon、preview、静态构建和浏览器入口 |
| `packages/<frontend-package>/stories/` | 共享前端包拥有的 Story                                                       |
| `apps/<visual-application>/stories/`   | 无法归属可复用包的应用专属视觉行为                                           |

`apps/storybook` 不拥有组件或产品行为。不得把其配置移入 `packages/ui`，也不得默认每个工作区创建一个 Storybook。

## Story 所有权

- 拥有渲染行为的工作区拥有顶层 `stories/` 中与 `src/` 同级的 `<component>.stories.tsx`。
- 每个公共视觉组件都有默认 Story；增加适用的禁用、加载、空、错误、打开、选中、变体和响应式状态。
- 纯类型、非视觉 Hook 和内部 Provider 接线不需要独立 Story。
- 每次公共视觉行为变更都同步更新所属 Story。
- 规范体验组件 Manifest 引用由所属方拥有的确定性 Story 及待检查状态；按照 [`experience-components.md`](experience-components.md) 原子更新其 Story ID 与 Manifest。
- 只通过包导出导入其他工作区，绝不导入 `src/*` 或 `dist/*`。
- 夹具必须确定，且不依赖实时网络、当前时间、随机性、凭据或机器状态。

## 导航契约

| 标题前缀              | 所有者                   |
| --------------------- | ------------------------ |
| `UI`                  | `@orchbee/ui` 可复用原语 |
| `Packages/<name>`     | 其他共享前端包           |
| `Applications/<name>` | 应用专属视觉行为         |

Story 标题、导出名称和生成 ID 是浏览器自动化接口；重命名时更新全部链接和浏览器测试。

- 使用 tag 进行跨领域状态或所有权筛选。
- 全局 decorator 保持通用；所有者专属 Provider、mock 和布局保留在所属 Story 中。

## 宿主决策

当 Story 共享渲染器、浏览器环境、依赖集合和 preview 契约时，保留一个物理 Storybook。Story 数量本身绝不能证明应拆分。

只有构建或测试成本经测量不可接受、渲染器或运行时不兼容、依赖无法共存，或所有权与部署独立时才拆分。通过 Storybook Composition 保留一个发现入口，并在设置数值阈值前记录测量结果。

## 编写和验证

- 每个 Component Story Format 文件只包含一个主要组件模块。
- 优先使用显式 args 和夹具；必需交互使用 `play`。
- 在增加 Story 专属 CSS 前使用 Storybook 布局和 viewport 参数。
- Story 专属布局不得重设被检查对象的样式。
- 通过所属包的公共资产导出导入样式表。

从仓库根目录运行：

```sh
pnpm storybook
pnpm storybook:build
```

`pnpm storybook` 启动一个进程；Rsbuild 把声明的公共源码入口纳入原生 HMR，不需要并行包 watcher。验证静态包时运行 `pnpm storybook:build`；`pnpm build:full` 也包含该工作区构建。普通验证运行工作区测试，要求每个公共 `@orchbee/ui` 组件模块都有 Story，不构建静态包。

`pnpm storybook:build` 和 `pnpm build:full` 都会先完成共享包构建，再启动静态宿主构建。发现的应用 Story 可能导入宿主直接依赖列表之外的包；全新静态构建需要在 Storybook 启动前准备好这些包的导出。

浏览器 Agent 先在侧边栏发现 Story，再使用 `iframe.html?id=<story-id>&viewMode=story` 进行 DOM、交互、无障碍、计算样式和截图检查。

主要参考：[Story 加载](https://storybook.js.org/docs/configure/index)、[tag](https://storybook.js.org/docs/writing-stories/tags)、[Composition](https://storybook.js.org/docs/sharing/storybook-composition) 和 [Rsbuild 集成](https://storybook.rsbuild.dev/guide/)。
