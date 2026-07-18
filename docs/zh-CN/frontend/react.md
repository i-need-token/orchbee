# React

## 技术栈契约

- 使用 React 和 TypeScript 构建产品界面。
- 使用 TanStack Router 进行客户端路由。
- 使用 Zustand 管理共享客户端状态。
- 使用基于 Base UI primitives 的 shadcn/ui 构建组件系统。
- 未经显式架构决策，不要引入其他路由器、共享状态库或 UI primitive 系统。
- 按照 [`docs/frontend/styling.md`](styling.md) 编写组件样式和运行时主题覆盖。

## 组件契约

- 把产品 View 视为从已准备接口到 React 节点的纯投影：`ViewProps -> ReactNode`。
- 在文本、variant、可见性、列表项、无障碍标签和语义回调到达 View 前准备好它们。View 只把这些值绑定到元素和 UI primitive，不解释它们。
- View 不订阅 store、不调用 service 或 controller、不本地化消息、不推导产品状态、不拥有产品交互状态、不运行 Effect，也不把原始 DOM 事件转换为业务操作。
- 通过 props 明确表达 View 依赖。只有当多个后代共享一个所有者时，才在 React adapter 中引入 context 或作用域 store；产品 View 不读取环境应用状态。
- 组件大小不是架构边界。只有存在另一个所有者、生命周期或可复用行为时才提取代码，不要为了满足任意行数或 Hook 数量而提取。
- 通用 UI primitive 可以封装框架级无障碍和视觉交互行为。它们公开具有产品含义的值和回调，而不是把这些机制泄漏到产品 View 中。

React 仍然要求由函数组件调用 Hook。让该组件保持为一个狭窄的 adapter，并与 View 分离：

```text
Controller 或 Store -> Presenter Hook -> ViewProps -> View
```

adapter 订阅 React 专属来源、调用 presenter Hook，并返回一个 View。它不包含产品标记或业务工作流。应用启动、跨功能工作流、持久化、传输选择和平台能力实现都留在 adapter 和 View 之外。

## 状态归属

| 状态或行为                                   | 所有者                                            |
| -------------------------------------------- | ------------------------------------------------- |
| 产品输入、展开、选择和焦点状态               | presenter Hook 或功能作用域 store                 |
| 仅框架使用的无障碍或视觉交互状态             | 封装该状态的通用 UI primitive                     |
| 一个已挂载功能子树共享的 UI 状态             | 作用域限定到该子树的 store 实例                   |
| 领域转换和不变量                             | 负责该领域的 store 或 service 所封装的纯转换函数  |
| 网络、终端、订阅、计时器和浏览器资源生命周期 | controller、service 或狭窄平台 adapter            |
| 首次渲染所需的应用启动输入                   | 调用 `createRoot(...).render(...)` 前的应用组合根 |

工作区级所有权仍以 [`docs/frontend/package-boundaries.md`](package-boundaries.md#状态归属) 为权威来源。不要仅仅因为 React 渲染某个状态的投影就把该状态移入 React。

## 派生状态和转换

- 在构造 `ViewProps` 前，通过 selector、presenter Hook 或纯 presenter 函数派生值；不要用 Effect 把可派生值镜像到状态中。
- 使用接收前一状态并返回下一状态的语义操作或纯转换来建模多字段不变量。
- 在外部输入的信任边界解析它，然后把已验证的领域值传给转换，把准备好的显示值传给 View。

```ts
// ❌ 镜像状态可能过期。
React.useEffect(() => setIsEmpty(sessions.length === 0), [sessions]);

// ✅ 调用 View 前准备投影。
const viewProps = { isEmpty: sessions.length === 0 };
```

## Effect 和资源生命周期

- 产品 View 不使用 Effect。presenter Hook 或 adapter 只有在同步 React 与外部系统时才使用 Effect。
- 每个 Effect 只拥有一个外部资源，并返回该资源完整且幂等的清理逻辑。
- 把获取、订阅、取消和释放放在一起，使部分初始化不会泄漏资源。
- 当生命周期协调多个订阅、分阶段异步初始化、重试、排序或恢复时，把它移入 controller 或 service。此时 Effect 只创建或连接该所有者并释放它。
- 把跨 store 同步放在拥有这层关系的协调器中，而不是 presenter Hook 或 View 中。
- View 接收语义回调。原始事件规范化属于通用 UI primitive 或 adapter；多步骤工作流属于 controller 或 service。

## 自定义 Hook

- presenter Hook 是准备完整 View 接口的 React 专属 adapter。其结果无需进一步本地化、派生或事件解释即可直接供 View 使用。
- 当另一个自定义 Hook 拥有完整的 React 专属生命周期或组合了可复用 React 行为时才提取它；不要仅仅为了隐藏 `useState`、`useMemo`、`useRef` 或 `useEffect` 而提取 Hook。
- 把与框架无关的计算移入纯函数，把外部资源编排移入 controller 或 service，不要把它们嵌入更大的 Hook。

## Zustand 和 context

- 把 Zustand store 限定到需要共享状态的最小所有者；不要为仅在一次挂载中使用的功能引入进程级单例。
- 在 presenter Hook 或 adapter 中通过 selector 订阅，使其只观察准备 View 接口所需的状态。
- 公开保持 store 不变量的语义操作；不要向 adapter 或 View 公开不受限的状态修改。
- 存储持久状态和领域操作，不要存储由 React Hook 创建的回调。当后代需要共享行为时，通过组合传递稳定的 controller 或 capability。
- React context 用于稳定 capability 和作用域 store 实例。高频可变数据属于外部 store，并通过 selector 读取。

## 测试边界

- 在没有 React 的情况下测试纯转换和 selector。
- 使用 fake port 测试 controller，并验证获取、事件转发、取消、失败和幂等清理。
- 直接从导出接口渲染 View，不使用产品 provider、store、service 或 controller mock。覆盖有意义的接口 variant 和无障碍输出。
- 针对 presenter Hook 的公共准备接口测试它，然后让 adapter 集成覆盖保持狭窄。
- 不要 patch React 私有内部实现，也不要在测试中复制生产算法。
- 遵循 [`docs/TESTING.md`](../TESTING.md) 中仓库级可发现性和回归要求。

## 正确性与安全性

- `react/rules-of-hooks`：只在组件或自定义 Hook 顶层调用 Hook。
- `react/exhaustive-deps`：包含 Hook 回调使用的每个响应式依赖。
- `react/jsx-key`：为列表元素提供来自数据身份的稳定 key。
- `react/jsx-no-target-blank`：防止新标签页链接访问 opener。
- `react/no-danger`：渲染结构化 React 内容，不要注入 HTML。

生成的 shadcn `ChartStyle` 实现是唯一的 `react/no-danger` 例外。其 style 文本只能包含可信、由开发者编写的图表配置；绝不能把持久化用户值或其他不可信字符串传入该配置。
