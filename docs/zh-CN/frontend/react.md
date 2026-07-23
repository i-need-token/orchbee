# React

## 技术栈契约

| 关注点   | 必需选择                   |
| -------- | -------------------------- |
| 界面     | React 和 TypeScript        |
| 路由     | TanStack Router            |
| 共享状态 | Zustand                    |
| UI 原语  | 使用 Base UI 的 shadcn/ui  |
| 样式     | [`styling.md`](styling.md) |

替代路由器、共享状态库或原语系统需要显式架构决策。

## 组件契约

- 产品组件渲染准备好的展示值并绑定准备好的语义动作。
- 在 TSX 外准备文本、变体、可见性、项目、无障碍标签和动作；组件不得把它们重新解释为业务规则。
- 通过直接 Props 或功能作用域 Zustand selector Hook 提供数据；展示组件不得直接访问外部资源。
- 工作流、外部资源同步和原始事件转换保留在展示组件之外。
- 当一个具名界面区域有自己的输入子集、状态组合或变更原因时提取它；复用和行数无关。
- 当一个区域渲染有限产品状态时，在 TSX 外准备可辨识展示模型，再使用不分发工作流的聚焦渲染器。
- 通用原语可以拥有框架无障碍和视觉交互，同时公开具有产品含义的值与回调。

React 必须连接 Provider 或外部生命周期时，把集成隔离在一个窄组件中。该组件调用一个 Presenter 或生命周期 Hook，按需创建或提供作用域状态，准备直接子组件输入，并返回一个产品组件。启动、跨功能工作流、持久化、传输选择和平台实现保留在展示组件之外。

## 递归组件所有权

- 每个具名界面区域以一个组件为根，拥有私有后代、组件专属 Hook、作用域 Store 和 context、转换、样式、资产及测试。
- 递归嵌套所有权；私有后代可以拥有另一个 `components/<component-name>/` 树和更窄 Store。
- 源模块重新导出的组件是公共根；外部模块只通过模块 `index.ts` 消费它。
- 不带产品语义的可复用原语放入 `@orchbee/ui`；产品组件保留在 [`package-boundaries.md`](package-boundaries.md#ui-所有权) 指定的能力中。
- 每个产物放在完全拥有它的最窄组件子树或源模块；绝不把自有产物收集到包级技术目录。
- 私有子树获得外部消费者时，提升完整所有权单元，而不是导出一个内部文件。

## 组件 Hook 边界

- `orchbee/no-react-hooks-in-tsx`：自有生产 `.tsx` 文件绝不直接调用 React 内置 Hook。
- 把状态、Effect、记忆化、ref 和外部 Store 订阅移入按责任聚焦的自定义 Hook。
- 窄适配器可以调用一个 presenter 或 lifecycle Hook，并直接传递其准备结果，不再派生或编排工作流。
- Presenter Hook 可以把本地化、Store selector 和语义动作组合成一个可渲染契约。
- 测试、Story 和 [`docs/LINTING.md`](../LINTING.md#生成的源码) 中签入的 shadcn 源码不受此限制。

## 组件数据传递

| 情况                          | 传递方式                                          |
| ----------------------------- | ------------------------------------------------- |
| 直接父子数据                  | 直接 Props                                        |
| 在一个功能 Store 外复用的组件 | 直接 Props                                        |
| 一个所有者由多个后代共享      | 带窄后代 selector 的 Context 作用域功能 Store     |
| 兄弟分支共享 UI 状态          | 由最近共同组件根拥有的 Store                      |
| 组件输入来自不同所有者        | 组合 Props 和 Store 选择，不把 Props 镜像进 Store |

浅层所有权默认使用 Props。中间布局组件既不转发未使用值，也不代替后代订阅。Context、Store 和 selector Hook 保持子树私有；外部模块通过公共 Props 和语义动作通信。

## TSX 文件边界

- `orchbee/one-function-per-tsx`：每个自有生产 `.tsx` 文件最多有一个顶层函数。
- 该函数是唯一具名运行时导出，是 PascalCase 组件，并与文件名 stem 完全一致。
- 嵌套回调、仅类型导出、常量和查找数据不占函数名额，但必须服务于组件的单一责任。
- 子组件放入符号匹配的 PascalCase `.tsx` 文件，Presenter Hook 放入符号匹配的 `useHookName.ts` 文件。
- 框架无关辅助模块对一个可调用导出使用准确 camelCase 函数名；无运行时符号拥有模块时使用按责任聚焦的 kebab-case 文件名。
- 测试、Story 和签入的 shadcn 源码不受此限制。

## 组件目录边界

- `orchbee/component-directory`：私有自有组件位于 `components/<component-name>/<ComponentName>.tsx`，且没有 `index.ts`。
- 公共边界组件由源模块 `index.ts` 直接重新导出；其 kebab-case 源模块目录只把 `index.ts`、`<ComponentName>.tsx` 和可选 `<ComponentName>.module.css` 作为直接文件。
- 私有组件目录只把 `<ComponentName>.tsx` 和可选 `<ComponentName>.module.css` 作为直接文件。
- 在组件 `.tsx` 中定义 Props；只有它属于公共 API 时才导出并重新导出该类型。
- 不得仅为把一个组件的 Props 移出 TSX 而创建 `<component-name>-props.ts`。
- 递归重复私有结构；`Toolbar.tsx` 的子组件放在 toolbar 所有者内的 `components/action/Action.tsx`。

只有存在具体自有内容时才创建支撑目录：

| 目录          | 允许的组件自有内容                                                                 |
| ------------- | ---------------------------------------------------------------------------------- |
| `components/` | 递归私有组件目录                                                                   |
| `contexts/`   | 包含 State、动作、创建、Provider 和 selector Hook 的一个 Context 作用域 Store 模块 |
| `hooks/`      | 行为、Presenter、动作和生命周期 Hook；绝不包装 Store selector Hook                 |
| `constants/`  | 只由组件拥有的静态值和有限配置                                                     |
| `types/`      | 子树多个文件共享的具名结构、union 和 enum                                          |
| `utils/`      | 只由组件拥有的框架无关纯辅助函数                                                   |
| `__tests__/`  | [`docs/TESTING.md`](../TESTING.md) 要求的单元测试                                  |

- 兄弟树共享资源保留在最近共同所有者；跨功能工作流、外部集成和契约保留在所属源模块。
- 私有或支撑目录不得增加 barrel；另一个模块需要它时提升整个树。
- Context Store 和 `use<ComponentName>Store(selector)` 保留在 `contexts/` 下的同一模块；绝不增加转发用 `hooks/useFilterableListStore.ts`。
- 单个组件使用的 Props 保留在其 `.tsx`；只有共享或有独立含义的数据才使用 `types/`。
- 可选 CSS Module 保留在导入它的 `.tsx` 旁边；不得创建 `styles/`。
- Story 遵循 [`storybook.md`](storybook.md) 的所属工作区顶层 `stories/` 契约。
- 签入的 shadcn 源码不受此限制。

## 状态所有权

| 状态或行为                                 | 所有者                                   |
| ------------------------------------------ | ---------------------------------------- |
| 产品输入、展开、选择和焦点                 | Presenter Hook 或功能作用域 Store        |
| 仅框架无障碍或视觉交互                     | 通用 UI 原语                             |
| 一个已挂载功能子树共享的状态               | 限定在该子树的 Store 实例                |
| 领域转换和不变量                           | 所属 Hook 或 Store 使用的纯转换          |
| 网络、流、订阅、定时器和浏览器资源生命周期 | 按责任聚焦的生命周期 Hook 或平台适配器   |
| 首次渲染所需启动输入                       | `createRoot(...).render(...)` 前的组合根 |

跨工作区仍以 [`package-boundaries.md`](package-boundaries.md#状态所有权) 为权威来源。

## 派生状态和转换

- 在 selector、Presenter Hook 或纯 Presenter 函数中派生值；绝不使用 Effect 把可派生值镜像进状态。
- 通过语义动作或接收前一状态的纯转换保持多字段不变量。
- 在信任边界解析外部输入，再把已验证领域值传给转换，把准备值传给组件。

## Effect 和资源生命周期

- 展示组件不使用 Effect。
- Presenter Hook、Provider 或适配器只用一个 Effect 同步一个外部资源，并返回完整幂等清理。
- 把获取、订阅、取消和释放放在一起。
- 用按责任聚焦的生命周期 Hook 处理订阅、分阶段初始化、重试、排序或恢复；每个 Effect 只连接和释放一个外部资源。
- 跨 Store 同步保留在拥有该关系的协调器中。
- 组件接收语义动作；原语或适配器规范化原始事件，所属功能的 Hook 或 Store action 协调工作流。

## 自定义 Hook

- Presenter Hook 返回完整可渲染组件契约，不留下业务派生或事件解释。
- 只有拥有完整 React 生命周期或可复用 React 行为时才提取 Hook，绝不只为隐藏 `useState`、`useMemo`、`useRef` 或 `useEffect`。
- 框架无关计算放入纯函数，外部资源同步放入生命周期 Hook。

## Hook 文件边界

- `orchbee/one-hook-per-file`：每个自有 Hook 模块按准确 camelCase 导出命名，只包含一个顶层函数，并且只导出该 Hook。
- 使用 `useKeyboardNavigation.ts` 或 `useKeyboardNavigation.tsx` 命名；函数与文件名 stem 一致。
- 辅助函数移入聚焦 `.ts` 模块，公共契约移入类型聚焦模块。
- 嵌套 Effect 回调和适配器不占函数名额。
- 测试和生成的 `packages/ui/src/hooks/use-mobile.ts` 不受此限制。

## Zustand 和 context

- `orchbee/zustand-scope`：使用 `zustand/vanilla` 的 `createStore` 创建自有 Store；只在 Context Store 模块或共享工厂中导入 `useStore`。
- 每次所属 Provider 挂载时惰性创建一个 Store，注入完整初始状态，把稳定 `StoreApi` 放进 context，并只包裹消费子树。
- 把 Store 限定在最窄已挂载所有者；重复组件挂载使用独立 Provider。
- 后续外部输入由拥有其关系的所有者协调。
- 权威数据位于 React 外部时，所属 Provider 订阅一次并把一个投影协调进 Store；后代没有第二个面向 React 的订阅源。
- 在 Provider 旁导出 `use<ComponentName>Store(selector)`，拒绝在其外使用，并要求窄 selector。
- 消费者只选择渲染的状态和调用的语义动作；绝不读取整个 Store、发布可复用 selector 常量或让中间组件替后代订阅。
- 公开保持不变量的动作，不公开无限制 mutation。
- Store 保存功能状态和领域动作，不保存 Hook 创建的回调；通过组合提供稳定的外部能力。

## 测试边界

- 不使用 React 测试纯转换和 selector。
- 使用注入的假实现测试生命周期 Hook 和平台适配器的获取、事件转发、取消、失败和幂等清理。
- 不使用产品 Provider、Store 或外部集成的 mock 渲染直接 Props 组件。
- 使用隔离 Provider 和确定性初始状态测试 Store 连接组件。
- 通过准备后的公共接口测试 Presenter Hook；保持适配器和 Provider 集成测试狭窄。
- 绝不 patch React 内部实现或在测试中复制生产算法。
- 遵循 [`docs/TESTING.md`](../TESTING.md)。

## 正确性与安全性

| 规则                        | 必需状态                            |
| --------------------------- | ----------------------------------- |
| `react/rules-of-hooks`      | Hook 只在组件或自定义 Hook 顶层运行 |
| `react/exhaustive-deps`     | Hook 回调声明每个响应式依赖         |
| `react/jsx-key`             | 列表 key 来自稳定数据身份           |
| `react/jsx-no-target-blank` | 新标签页链接不能访问 opener         |
| `react/no-danger`           | 使用结构化 React 内容替代注入 HTML  |

生成的 shadcn `ChartStyle` 是唯一 `react/no-danger` 例外；其样式文本只能包含可信的开发者编写图表配置，绝不能包含持久化值或其他不可信值。
