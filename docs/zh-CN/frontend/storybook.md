# Storybook

## 仓库模型

`apps/storybook` 是 Orchbee 在整个仓库中唯一的 Storybook 宿主。它拥有 Storybook 框架版本、Rsbuild 集成、addon、全局 preview 配置、静态构建和浏览器入口点。它不拥有组件或产品界面行为。

前端工作区在 `src/` 旁的顶层 `stories/` 目录中拥有自己的 Story 文件：

```text
apps/storybook/.storybook/
packages/ui/stories/
packages/<frontend-package>/stories/
apps/<visual-application>/stories/
```

不要把仓库 Storybook 配置放在 `packages/ui` 中，也不要默认每个工作区创建一套 Storybook。只有当视觉行为无法放入可复用前端包时，应用才提供 Story。

## Story 所有权

每个 Story 都由拥有所渲染行为的工作区维护。修改公共视觉组件时，在同一个变更中更新它所拥有的 Story。

每个公共视觉组件模块都至少有一个覆盖默认渲染状态的 Story。按需添加 variant，以及禁用、加载、空、错误、打开、选中和响应式状态。纯类型、非视觉 Hook 和内部 provider 连接逻辑不需要独立 Story。

Story 只能通过已声明的包导入来导入其他工作区。绝不能导入另一个工作区的 `src/*` 或 `dist/*` 路径。fixture 和 mock 保持确定性，并由 Story 所属工作区拥有；不要依赖实时网络、当前时间、随机数据、凭据或机器专属状态。

## 导航契约

顶层 Story 标题标识所属界面：

| 前缀                  | 所有者                         |
| --------------------- | ------------------------------ |
| `UI`                  | `@orchbee/ui` 可复用 primitive |
| `Packages/<name>`     | 其他共享前端包                 |
| `Applications/<name>` | 应用专属视觉行为               |

把 Story 标题、导出名称和生成 ID 视为浏览器自动化接口。有意识地重命名它们，并在同一个变更中更新每个已记录链接或浏览器测试。

使用 Storybook tag 进行跨领域状态或所有权筛选。保持全局 preview decorator 通用。只有一个工作区需要的 provider、mock 或布局应与该工作区的 Story 放在一起，不要在全局 preview 中通过所有者专属分支提供。

## 默认单实例

Story 数量本身不是拆分 Storybook 的理由。只要 Story 共享兼容的 renderer、浏览器环境、依赖集和全局 preview 契约，就保持一个物理实例。

只有当测量到的构建或测试成本不可接受、renderer 或所需运行时不兼容、依赖无法共存，或者所有权和部署确实需要隔离时，才拆分独立构建的 Storybook。通过 Storybook Composition 组合拆分后的 Storybook，保留一个发现入口。

不要在没有仓库测量证明阈值能预测真实开发或 CI 问题时设置数值拆分阈值。

## 编写约束

- 使用命名为 `<component>.stories.tsx` 的 Component Story Format 文件。
- 每个 Story 文件只保留一个主要组件模块，使覆盖可以机械发现。
- 优先使用显式、可读的 fixture 值和 args，不要使用生成数据。
- 需要交互的行为使用 `play` 函数。
- 添加仅 Story 使用的 CSS 前，先使用 Storybook 布局和 viewport 参数。
- 仅 Story 使用的布局不得重新设计被检查组件的样式。
- 通过所属包的公共资产导出导入其样式表。

## 命令和验证

从仓库根目录运行仓库 Storybook：

```sh
pnpm storybook
pnpm storybook:build
```

`pnpm storybook` 启动一个 Storybook 进程。其 Rsbuild source-build 集成把 Story 使用的已声明公共源码入口纳入原生 HMR 图，因此包变更无需并行包 watcher 即可进入 preview。

静态 Storybook 构建是应用工作区 `build` 任务的一部分，因此也是 `pnpm verify` 的一部分。Storybook 工作区测试会检查每个公共 `@orchbee/ui` 组件模块都有对应 Story 文件。

浏览器 Agent 打开开发服务器，通过侧边栏发现 Story，并使用隔离的 `iframe.html?id=<story-id>&viewMode=story` 路由进行 DOM、交互、无障碍、计算样式和截图检查。

## 外部参考

验证日期：2026-07-17。

- [Story 加载配置](https://storybook.js.org/docs/configure/index)定义多目录 Story 发现和标题前缀。
- [Story tag](https://storybook.js.org/docs/writing-stories/tags)定义侧边栏和测试筛选。
- [Storybook Composition](https://storybook.js.org/docs/sharing/storybook-composition)定义如何聚合独立构建的 Storybook 和 renderer。
- [Storybook Rsbuild](https://storybook.rsbuild.dev/guide/)定义仓库宿主使用的 Storybook 10 和 Rsbuild 集成。
