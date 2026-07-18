# React

## 技术栈契约

- 使用 React 和 TypeScript 构建产品界面。
- 使用 TanStack Router 进行客户端路由。
- 使用 Zustand 管理共享客户端状态。
- 使用基于 Base UI primitives 的 shadcn/ui 构建组件系统。
- 未经显式架构决策，不要引入其他路由器、共享状态库或 UI primitive 系统。
- 按照 [`docs/frontend/styling.md`](styling.md) 编写组件样式和运行时主题覆盖。

## 组件契约

- 产品函数组件渲染准备好的显示值，并把准备好的语义操作绑定到元素或 UI primitive。
- 在渲染文本、variant、可见性、列表项、无障碍标签和语义操作前准备好它们。组件不把这些值解释成业务规则。
- 组件可以通过直接 props 或功能作用域 Zustand selector Hook 接收准备好的数据。两者都是明确的组件依赖；两者都不允许组件调用 service 或 controller。
- 展示组件不拥有业务工作流、不用 Effect 同步外部资源，也不把原始 DOM 事件转换成业务操作。
- 当一个可独立命名的界面区域拥有独立数据子集、状态组合或变化原因时，为它提供独立的展示组件和输入契约。提取并不要求复用。
- 当一个区域渲染有限的产品状态或模型种类时，在 TSX 外准备可辨识的显示模型，并让一个聚焦的 renderer 选择匹配的叶子组件。工作流分发留在该 renderer 之外。
- 不要按任意行数或 Hook 数量拆分；应在维护者能够独立命名和测试的职责处拆分。
- 通用 UI primitive 可以封装框架级无障碍和视觉交互行为。它们公开具有产品含义的值和回调，而不是把这些机制泄漏到产品组件中。

React 仍然要求在 Provider 和外部生命周期边界使用函数组件。让这些 adapter 组件保持狭窄：

```text
Controller 或 Service -> Presenter Hook 或 Scoped Store -> Presentational Component
```

adapter 连接 React 与外部生命周期、创建或提供作用域 Store、准备直接子组件输入，并返回一个产品组件。应用启动、跨功能工作流、持久化、传输选择和平台能力实现仍然留在展示组件之外。

## 组件 Hook 边界

- `orchbee/no-react-hooks-in-tsx`：自有生产 `.tsx` 文件不直接调用 React 内置 Hook。把状态、Effect、记忆化、引用和外部 Store 订阅移入按职责聚焦的自定义 Hook。
- 狭窄的 adapter 组件可以调用一个具名 presenter 或生命周期 Hook，然后把准备好的结果传给显示组件。Hook 返回后，它不再增加派生或工作流步骤。
- presenter Hook 可以把本地化、功能 Store selector 和语义操作组合成一个完整、可直接渲染的契约。展示后代消费该契约，不调用 service，也不重新构造工作流。
- 单元测试、Storybook story，以及 [`docs/LINTING.md`](../LINTING.md#生成的源码) 中说明的检入 shadcn 源码不受这条自有生产代码规则约束。

## 组件数据传递

- 当数据属于直接父子关系，或者组件需要在某个功能 Store 之外复用时，使用直接 props。
- 当一个功能所有者向多个后代提供状态或操作，而且直接 props 会让中间组件转发自己不使用的值时，使用功能作用域 Store。
- 不要仅仅为了避免一个有意义的父子 props 契约而引入 Store。对于浅层所有权，props 仍然是默认选择。
- 中间布局组件既不订阅仅供后代使用的状态，也不转发它。消费数据的后代通过功能 Hook 自行选择窄状态和语义操作。
- 当输入拥有不同所有者时，组件可以组合自己的 props 与功能 Store selector。不要仅仅为了让所有输入都通过同一通道到达，就把 props 镜像进 Store。

## TSX 文件边界

- `orchbee/one-function-per-tsx`：每个自有生产 `.tsx` 文件最多包含一个顶层函数。把其他组件、Hook、回调工厂和辅助函数全部移入其所属模块。
- 当一个函数声明、函数表达式或箭头函数不被其他函数包含时，它就是顶层函数。嵌套在文件唯一组件内部的回调不占用另一个文件级函数名额。
- 把子展示组件放入自己的 `.tsx` 文件，把 presenter Hook 放入自己的 `use-*.ts` 文件，把与框架无关的辅助逻辑放入按职责聚焦的 `.ts` 文件。
- 类型声明、常量和准备好的查找数据不占用函数名额，但只有在支撑组件单一职责时才保留在该文件中。
- 单元测试、Storybook story，以及 [`docs/LINTING.md`](../LINTING.md#生成的源码) 中说明的检入 shadcn 源码不受这条自有生产代码规则约束。

## 组件目录边界

- `orchbee/component-directory`：未由所属源模块的 `index.ts` 直接重新导出的自有生产组件属于私有子组件，并放在 `components/<component-name>/<component-name>.tsx`。
- 由所属 `index.ts` 直接重新导出的组件是该模块的边界组件，可以继续与 index 同级放置。只负责挂载边界组件的应用入口文件不是组件，仍留在组合边界。
- 对更深层提取出的子组件重复这一分组；`toolbar.tsx` 的私有子组件应放在 `toolbar` 分组内的 `components/action/action.tsx`，而不是作为松散的同级文件。
- 把组件拥有的 CSS Modules 和其他组件资源放在同一个组件分组中。组件从自身目录导入 CSS Module，而不跨到祖先目录或其他组件分组中获取。
- 组件分组是结构归属目录，不是源模块边界。不要在私有组件分组中增加 `index.ts` barrel；在所属模块内部直接导入实现文件。
- 不要把组件分组当作混杂的功能目录。模块级 Hook、controller、service 和共享契约仍放在拥有对应职责的源模块中。
- 当组件拥有有意义的可选测试时，按照 [`docs/TESTING.md`](../TESTING.md#必需的单元测试) 把它放在组件分组内同级的 `__tests__` 目录中。
- [`docs/LINTING.md`](../LINTING.md#生成的源码) 中说明的检入 shadcn 源码不受这条自有组件目录规则约束。

```text
feature/
├── index.ts
├── feature.tsx
└── components/
    └── toolbar/
        ├── toolbar.tsx
        ├── toolbar.module.css
        └── components/
            └── action/
                ├── action.tsx
                └── action.module.css
```

## 状态归属

| 状态或行为                                   | 所有者                                            |
| -------------------------------------------- | ------------------------------------------------- |
| 产品输入、展开、选择和焦点状态               | presenter Hook 或功能作用域 Store                 |
| 仅框架使用的无障碍或视觉交互状态             | 封装该状态的通用 UI primitive                     |
| 一个已挂载功能子树共享的 UI 状态             | 作用域限定到该子树的 store 实例                   |
| 领域转换和不变量                             | 负责该领域的 store 或 service 所封装的纯转换函数  |
| 网络、终端、订阅、计时器和浏览器资源生命周期 | controller、service 或狭窄平台 adapter            |
| 首次渲染所需的应用启动输入                   | 调用 `createRoot(...).render(...)` 前的应用组合根 |

工作区级所有权仍以 [`docs/frontend/package-boundaries.md`](package-boundaries.md#状态归属) 为权威来源。不要仅仅因为 React 渲染某个状态的投影就把该状态移入 React。

## 派生状态和转换

- 在组件渲染值之前，通过 selector、presenter Hook 或纯 presenter 函数派生它们；不要用 Effect 把可派生值镜像到状态中。
- 使用接收前一状态并返回下一状态的语义操作或纯转换来建模多字段不变量。
- 在外部输入的信任边界解析它，然后把已验证的领域值传给转换，把准备好的显示值传给组件。

```ts
// ❌ 镜像状态可能过期。
React.useEffect(() => setIsEmpty(sessions.length === 0), [sessions]);

// ✅ 渲染组件前准备投影。
const viewProps = { isEmpty: sessions.length === 0 };
```

## Effect 和资源生命周期

- 展示组件不使用 Effect。presenter Hook、Provider 或 adapter 只有在同步 React 与外部系统时才使用 Effect。
- 每个 Effect 只拥有一个外部资源，并返回该资源完整且幂等的清理逻辑。
- 把获取、订阅、取消和释放放在一起，使部分初始化不会泄漏资源。
- 当生命周期协调多个订阅、分阶段异步初始化、重试、排序或恢复时，把它移入 controller 或 service。此时 Effect 只创建或连接该所有者并释放它。
- 把跨 Store 同步放在拥有这层关系的协调器中，而不是展示组件中。
- 组件接收语义操作。原始事件规范化属于通用 UI primitive 或 adapter；多步骤工作流属于 controller 或 service。

## 自定义 Hook

- presenter Hook 是准备完整组件输入契约的 React 专属 adapter。其结果无需进一步业务派生或事件解释即可直接渲染。
- 当另一个自定义 Hook 拥有完整的 React 专属生命周期或组合了可复用 React 行为时才提取它；不要仅仅为了隐藏 `useState`、`useMemo`、`useRef` 或 `useEffect` 而提取 Hook。
- 把与框架无关的计算移入纯函数，把外部资源编排移入 controller 或 service，不要把它们嵌入更大的 Hook。

## Hook 文件边界

- `orchbee/one-hook-per-file`：每个命名为 `use-*.ts` 或 `use-*.tsx` 的自有生产 Hook 模块只能包含一个顶层函数和一个导出符号。
- 唯一的顶层函数就是具名 Hook 导出。把支撑函数移入按职责聚焦的 `.ts` 模块，把公共契约移入专门存放类型的模块。
- 嵌套在 Hook 内的函数，包括 Effect 回调和事件 adapter，不占用另一个文件级函数名额，也不是额外导出。
- 单元测试以及 [`docs/LINTING.md`](../LINTING.md#生成的源码) 中说明的生成源码 `packages/ui/src/hooks/use-mobile.ts` 不受这条自有生产代码规则约束。

## Zustand 和 context

- `orchbee/zustand-scope`：使用来自 `zustand/vanilla` 的 `createStore` 创建自有 Zustand Store，在功能所有者内部为每次挂载创建实例，并且只在提供显式 selector 的 `use-*.ts` 或 `use-*.tsx` selector Hook 中导入 `useStore`。
- 把 Store 限定到需要共享状态的最小功能所有者；不要为仅在一次挂载中使用的功能引入进程级单例。
- 把稳定的 `StoreApi` 实例放进 React context。不要把不断变化的状态快照放进 Provider value。
- 每次 Provider 挂载时惰性创建一次 Store，注入完整初始状态，并由理解该关系的所有者协调后续外部输入。
- 当外部 Service 是功能数据的权威来源时，由它的 Provider 统一订阅一次，并把最新投影协调进作用域 Store。后代通过 Zustand 选择该投影；不要再把 Service snapshot 暴露成第二个面向 React 的订阅源。
- 公开一个功能 Hook，例如 `useWorkbenchStore(selector)`。它从 context 读取作用域 Store、拒绝在 Provider 外调用，并使用调用方的窄 selector 订阅。
- 消费者只选择自己渲染或调用的状态和语义操作。不要读取整个 Store、提取可复用 selector 常量，也不要让中间组件为更深后代订阅。
- 公开保持 Store 不变量的语义操作；不要向组件公开不受限的状态修改。
- 存储持久功能状态和领域操作，不要存储由 React Hook 创建的回调。当后代需要共享行为时，通过组合传递稳定的 controller 或 capability。
- 同一功能挂载多次时使用独立 Provider 实例；测试通过同一 Provider 边界创建隔离 Store。

## 测试边界

- 在没有 React 的情况下测试纯转换和 selector。
- 使用 fake port 测试 controller，并验证获取、事件转发、取消、失败和幂等清理。
- 直接从导出接口渲染使用直接 props 的展示组件，不使用产品 Provider、Store、service 或 controller mock。覆盖有意义的接口 variant 和无障碍输出。
- 使用隔离的功能 Provider 和确定的初始状态测试 Store 连接组件；不要在测试间复用进程级 Store。
- 针对 presenter Hook 的公共准备接口测试它，然后让 adapter 和 Provider 集成覆盖保持狭窄。
- 不要 patch React 私有内部实现，也不要在测试中复制生产算法。
- 遵循 [`docs/TESTING.md`](../TESTING.md) 中仓库级可发现性和回归要求。

## 正确性与安全性

- `react/rules-of-hooks`：只在组件或自定义 Hook 顶层调用 Hook。
- `react/exhaustive-deps`：包含 Hook 回调使用的每个响应式依赖。
- `react/jsx-key`：为列表元素提供来自数据身份的稳定 key。
- `react/jsx-no-target-blank`：防止新标签页链接访问 opener。
- `react/no-danger`：渲染结构化 React 内容，不要注入 HTML。

生成的 shadcn `ChartStyle` 实现是唯一的 `react/no-danger` 例外。其 style 文本只能包含可信、由开发者编写的图表配置；绝不能把持久化用户值或其他不可信字符串传入该配置。
