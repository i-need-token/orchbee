# 前端包边界

## 工作区放置

运行时入口点使用 `apps/*`，通过库 API 使用的稳定能力使用 `packages/*`。不要把组件、Hook、store、类型或 utility 等 UI 源码类别直接映射为工作区包。

整个仓库的 UI 目录是开发应用 `apps/storybook`。Story 文件按照 [`docs/frontend/storybook.md`](storybook.md) 继续由其前端工作区拥有；Storybook 不改变运行时包所有权。

目标依赖模型是一份放置契约，并不要求立即创建每个具名工作区。只有在同一个变更中存在具体工作区使用方时才添加包；在边界存在之前，把应用专属代码留在所属应用中。

```text
apps/desktop ---+
                 +--> @orchbee/workbench --> @orchbee/ui ----------+
apps/web --------+             |                                   |
                               +--> @orchbee/design-tokens <--------+
                               |
                               +--> @orchbee/terminal
                               |
                               +--> @orchbee/session-client --> @orchbee/session-protocol

apps/desktop --------------------> @orchbee/session-runtime --> @orchbee/session-protocol
apps/daemon ---------------------> @orchbee/session-runtime
```

应用工作区拥有运行时启动、路由根、身份验证入口点、部署或打包配置和平台专属 adapter。包绝不能从应用导入。

## 包职责

| 包                          | 运行时      | 职责                                                | 非目标                                                               | 准入点                                                          |
| --------------------------- | ----------- | --------------------------------------------------- | -------------------------------------------------------------------- | --------------------------------------------------------------- |
| `@orchbee/terminal`         | `browser`   | 终端解析、渲染、输入、选择、搜索和浏览器集成        | React UI、应用 PTY 进程、会话传输、Electron 平台副作用或产品对话框   | 现有可独立使用的库                                              |
| `@orchbee/ui`               | `browser`   | 基于 Base UI 的可复用 shadcn 组件                   | 主题、设计 Token、会话、Agent、终端窗格、路由、远程状态或平台 API    | 与首个使用它的产品 UI 一同添加                                  |
| `@orchbee/design-tokens`    | `universal` | 可枚举视觉尺度、语义主题角色和响应式阈值            | React 组件、产品交互、应用状态、任意组件值或副作用                   | shadcn 兼容样式表中已有使用方                                   |
| `@orchbee/workbench`        | `browser`   | 共享的具有产品语义的 React 界面、交互状态和展示模型 | 应用启动、路由根、身份验证、持久化、平台导入、PTY 所有权或传输层实现 | `apps/web` 中已有使用方；在跨应用契约得到证明前保持私有和实验性 |
| `@orchbee/session-protocol` | `universal` | 会话命令、事件、快照和错误的 Zod Schema 与类型      | I/O、传输选择、状态存储、PTY 进程或 UI                               | 当两个信任边界参与方使用同一传输契约时添加                      |
| `@orchbee/session-client`   | `browser`   | 无头客户端同步、命令分发、重连和状态                | React 展示、平台副作用、传输实现或服务器进程所有权                   | 当浏览器应用使用远程或跨进程会话状态时添加                      |
| `@orchbee/session-runtime`  | `node`      | 可复用 PTY、进程和会话生命周期服务                  | Electron 菜单或窗口、浏览器 UI、传输专属入口点或客户端状态           | 当 Node 会话行为在原应用树之外拥有具体使用方时提取              |

会话包系列按能力逐个引入，而不是一次全部引入。当更广泛的编排、工作节点或 Agent 协作契约的职责不再适合会话契约时，为它们建立各自具名领域；不要把 `session-*` 扩大成兜底领域。

每个新增工作区都必须按照 [`docs/PROJECTS.md`](../PROJECTS.md) 在 `project.json` 中声明准确职责和非目标。

## UI 所有权

`@orchbee/ui` 只包含不带 Orchbee 产品语义的元素，包括按钮、对话框、菜单、tooltip 和可调整大小的布局 primitive。它可以拥有可复用视觉行为，但不得知道会话、Agent、节点或终端窗格意味着什么。

所有前端工作区都遵循 [`docs/frontend/styling.md`](styling.md) 中的生成组件和样式所有权契约。

`@orchbee/design-tokens` 独立于 React 和 shadcn，拥有共享视觉词汇。`@orchbee/ui` 只使用它来映射生成的 shadcn 变量，而 Workbench 自有组件通过 CSS Modules 直接使用它。详细层级和主题契约位于 [`docs/frontend/design-tokens.md`](design-tokens.md)。

`@orchbee/workbench` 拥有具有产品语义的展示，包括应用 shell、终端窗格、标签和拆分行为、会话与 Agent View、节点状态、设置 View、命令界面，以及满足应用级终端请求的 UI。

Workbench 还拥有其产品语义消息的源描述符和发布翻译。应用工作区拥有用户 locale 选择、平台检测和偏好持久化，然后在组合时提供有效 locale。`@orchbee/ui` 保持 locale 无关，并公开通用的可见或仅无障碍文本，供具有产品语义的使用方本地化。完整契约位于 [`localization.md`](localization.md)。

在 Workbench 终端模块中保留围绕 `@orchbee/terminal` 的 React wrapper，直到它拥有独立使用方和 API。不要向 `@orchbee/terminal` 添加 React、Zustand、shadcn/ui、Base UI、TanStack Router、Electron 或 Node 依赖。

## Workbench 交互模型

Orchbee 以终端为先。默认 Workbench 界面把焦点和窗口剩余区域交给活动终端；它不得继承 IDE Workbench，也不得为聊天保留主要界面。全高左侧边栏为管理导航预留空间。在项目、worktree、Agent 或其他模型拥有具体能力边界前，让它保持为空；shell 不得把布局契约耦合到推测性模型。

让次要能力可被发现，但不要把它们变成永久 chrome。设置使用从账户或设置区域进入的专用可搜索界面，并把相关控件组织到具名类别中，而不是不断扩张一个 modal。浏览器、变更和类似检查工具属于默认隐藏的辅助右侧面板，通过紧凑的右上角控件打开，并可独立关闭。拆分和破坏性终端操作属于紧凑操作菜单。状态应靠近它所描述的实体：会话状态位于具体会话界面中，运行时和连接状态位于拥有它们的管理模型中，瞬时反馈位于通知中。不要向预留侧边栏添加占位状态 chrome。

## 产品概念

终端会话是拥有 PTY 输入、输出、身份和生命周期的权威运行进程。Workbench 布局是用于观察会话的客户端排列，包括标签、拆分、窗格、焦点和面板尺寸。Agent 或编排任务是协作实体，即使当前一个 Agent 运行在一个会话中，也不得从终端会话推断其身份和生命周期。

让 Workbench 布局状态保留在 `@orchbee/workbench` 本地，直到具体的跨设备产品需求引入单独命名的同步契约。不要把布局、Agent 协作、工作节点调度或无关应用状态放入 `@orchbee/session-protocol`；当这些职责跨越信任边界时，引入一个职责狭窄的所属领域。

## Workbench 源模块

按产品所有权组织 Workbench 源码，而不是按技术产物类型：

```text
packages/workbench/src/
  agents/
  commands/
  composition/
  nodes/
  sessions/
  settings/
  shell/
  terminals/
  index.ts
```

每个目录都是带 `index.ts` 边界的源模块。把跨功能组装放在叶级 `composition` 模块中，由它使用同级入口点，然后通过根 `index.ts` 向上提升。不要在根模块中放置会导入私有后代文件的 `App.tsx`。该结构遵循 [`docs/MODULE-BOUNDARIES.md`](../MODULE-BOUNDARIES.md#组合和循环依赖)。

把功能专属组件、Hook、状态、类型和 utility 保留在拥有它们的功能中。只有当另一个可见模块需要某项能力时，才通过功能的 `index.ts` 提升它。

## 平台集成

在使用能力的功能中定义每个平台 port。相比不断扩张的宿主接口，优先使用外部导航、目录选择、原生菜单和通知等狭窄契约。Workbench 组合模块可以聚合这些契约用于注入，而不让每个功能都依赖聚合。`apps/desktop` 通过 Electron preload 和 IPC 实现能力；`apps/web` 使用浏览器或 Web 服务 API 实现。

`@orchbee/session-client` 定义它所使用的传输行为。应用在组合时提供 Electron IPC、WebSocket 或其他传输 adapter。包不得通过检测宿主环境来选择传输。

不要为这些 adapter 创建泛化的 `bridge` 或 `platform` 包。共享的不可信消息形状属于狭窄协议包，而运行时专属副作用留在拥有该运行时的应用中。

## 状态归属

| 状态                                   | 所有者                           |
| -------------------------------------- | -------------------------------- |
| 终端缓冲区、选择、终端搜索和渲染状态   | `@orchbee/terminal`              |
| PTY 进程和权威会话生命周期             | `@orchbee/session-runtime`       |
| 客户端投影、事件排序、重连和同步状态   | `@orchbee/session-client`        |
| 活动标签、面板尺寸、焦点和命令面板状态 | `@orchbee/workbench`             |
| Workbench 产品消息和发布翻译           | `@orchbee/workbench`             |
| 用户 locale 偏好和平台 locale 检测     | 相应的 `apps/*`                  |
| Electron 或浏览器能力状态              | 相应的 `apps/*`                  |
| Workbench 布局或偏好的持久化           | 通过注入 capability 使用它的应用 |
| 原始跨进程或网络消息                   | 在接收边界使用会话 Schema 解析   |

在拥有状态的浏览器包中使用 Zustand 管理共享客户端 UI 状态。不要创建名为 `store`、`types`、`hooks`、`utils`、`common`、`shared` 或 `helpers` 的工作区包；所有权由赋予代码含义的领域决定，而不是由其 TypeScript 形状决定。

## 引入顺序

1. 一同创建使用方应用和 `@orchbee/ui`；当产品界面通过库边界共享时，也同时创建 `@orchbee/workbench`。
2. 当产品交互契约仍在变化时，针对内存会话 adapter 构建界面。
3. 连接真实跨进程或远程同步时，添加 `@orchbee/session-protocol` 和 `@orchbee/session-client`。
4. 把 Node 会话逻辑保留在所属应用中，直到另一个运行时入口对同一服务有具体需求，再提取 `@orchbee/session-runtime`。

职责转移时，在同一个变更中更新受影响的两个 `project.json` 契约、公共导出、包 README 和本文档。引入或修改包边界后，运行 [`docs/PACKAGES.md`](../PACKAGES.md#验证和审查) 要求的包检查。
