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

- 领域决策与不变量保留在语义所有者处。组件可以从 Props 和局部状态派生简单展示值；不得在渲染中重复领域策略。
- 通过直接 Props 或功能作用域 Zustand selector Hook 提供数据；展示组件不得直接访问外部资源。
- 跨功能工作流、持久化、传输选择和平台实现保留在展示组件之外。简单的事件到值转换可以与处理器放在一起。
- 界面区域具有独立变化的职责或生命周期时再提取。内聚的私有渲染辅助函数可以放在一起；函数数量和行数不决定所有权。
- 当预先准备的可辨识展示模型能让复杂产品状态策略离开渲染时，使用该模型。
- 通用原语可以拥有框架无障碍和视觉交互，同时公开具有产品含义的值与回调。

Provider 与外部生命周期集成保持窄化、显式。组件可以组合其职责需要的 Hook；不要求只能调用一个 Hook 或只能返回一个子组件。

## 递归组件所有权

- 每个具名界面区域以一个组件为根，拥有私有后代、组件专属 Hook、作用域 Store 和 context、转换、样式、资产及测试。
- 递归嵌套所有权；私有后代可以拥有另一个 `components/<component-name>/` 树和更窄 Store。
- 源模块重新导出的组件是公共根；外部模块只通过模块 `index.ts` 消费它。
- 不带产品语义的可复用视觉和无障碍原语放入 `@orchbee/ui`。按照 [`docs/PACKAGES.md`](../PACKAGES.md#foundation-边界)，已准入的领域中立 React 编程原语放入 `@orchbee/foundation/react`；产品组件保留在 [`package-boundaries.md`](package-boundaries.md#ui-所有权) 指定的能力中。
- 每个产物放在完全拥有它的最窄组件子树或源模块；绝不把自有产物收集到包级技术目录。
- 私有子树获得外部消费者时，提升完整所有权单元，而不是导出一个内部文件。

## 组件 Hook 边界

- 组件可以为自有局部状态、ref、标识符和展示计算直接调用 React 内置 Hook，同时遵循[回调标识](#回调标识)及 [Effect 和资源生命周期](#effect-和资源生命周期)。
- 为内聚的可复用行为、复杂状态策略或外部资源生命周期提取自定义 Hook。不得仅为把内置调用移出 TSX 而增加转发 Hook。
- 当准备工作具有独立职责时，Presenter Hook 可以组合本地化、Store selector 和语义动作。
- 无论文件放在哪里，Hook 调用顺序、依赖、状态所有权与清理要求都继续适用。

## 回调标识

- `orchbee/no-react-use-callback`：仓库源码绝不导入、调用、重新导出、设置别名或以其他方式访问 React 的 `useCallback`；检入的生成源码也没有例外。
- 当事件处理器、命令式适配器以及通过组件或 Hook 契约公开的回调需要稳定标识，并且实现必须读取最新渲染值时，使用 `@orchbee/foundation/react` 的 `useMemoizedFn`。
- 当使用方需要响应语义变化时，使用显式数据或版本 Token 表达，绝不通过发布新的函数标识表达。
- 在 Effect 依赖中直接列出具有语义的订阅和资源生命周期输入；稳定回调标识不得抑制必需的清理、重新注册、取消或重启行为。

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

- `orchbee/component-export-boundary`：自有生产组件模块只有一个具名 PascalCase 运行时导出，与文件名匹配。
- 内聚的私有函数、渲染辅助函数、常量、静态查找数据及相关类型声明可以与组件放在一起。相关类型属于组件 API 时才导出。
- 只为独立职责或消费者提取组件或辅助模块，不为满足函数或声明数量而拆分。
- 模块级值不得引入共享可变功能状态、资源申请或副作用。这些语义需要审查；导出规则检查公共形状，不检查纯度。
- 提取后的组件使用符号匹配的 PascalCase `.tsx` 文件。专属 Hook 使用 `useHookName.ts`；非组件 TSX 入口模块保持按职责命名的 kebab-case 文件名。
- 测试、Story 和签入的 shadcn 源码不受组件导出规则限制。

## 组件目录边界

- `orchbee/component-directory`：私有自有组件位于 `components/<component-name>/<ComponentName>.tsx`，且没有 `index.ts`。
- 公共边界组件由源模块 `index.ts` 直接重新导出；其 kebab-case 目录名与组件名匹配。
- 公共和私有组件目录都可以把自有 Hook、辅助函数、常量及类型与 `<ComponentName>.tsx` 和可选 `<ComponentName>.module.css` 就近放置。私有目录仍然没有 `index.ts`。
- 在组件 `.tsx` 中定义 Props；只有它属于公共 API 时才导出并重新导出该类型。
- 不得仅为把一个组件的 Props 移出 TSX 而创建 `<component-name>-props.ts`。
- 递归重复私有结构；`Toolbar.tsx` 的子组件放在 toolbar 所有者内的 `components/action/Action.tsx`。

支撑目录应有具体自有内容，并能改善导航；不要求每类文件都创建目录：

| 目录          | 允许的组件自有内容                                                                                     |
| ------------- | ------------------------------------------------------------------------------------------------------ |
| `components/` | 递归私有组件目录                                                                                       |
| `contexts/`   | 只读能力 Context，或包含 State、动作、创建、Provider 和 selector Hook 的一个 Context 作用域 Store 模块 |
| `hooks/`      | 行为、Presenter、动作和生命周期 Hook；绝不包装 Store selector Hook                                     |
| `constants/`  | 组件拥有的会生成 JavaScript 的静态值、`as const` 类枚举对象、查找数据和有限配置                        |
| `types/`      | 子树多个文件共享且会被擦除的 TypeScript interface、alias、结构和 union                                 |
| `utils/`      | 只由组件拥有的框架无关纯辅助函数                                                                       |
| `__tests__/`  | 按 [`docs/TESTING.md`](../TESTING.md#单元测试准入与删除) 准入的测试                                    |

- 兄弟树共享资源保留在最近共同所有者；跨功能工作流、外部集成和契约保留在所属源模块。
- 私有或支撑目录不得增加 barrel；另一个模块需要它时提升整个树。
- 当公共根组件本身是一项完整能力时，可以直接拥有语义子系统目录，前提是每个目录包含多个内聚文件、只承担一个领域责任，并且没有独立外部消费者。名称应描述能力侧面，例如 `model/`、`layout/`、`dragging/` 或 `accessibility/`；禁止 `common/`、`shared/`、`style/` 和 `misc/` 等通用收集名称。
- 语义子系统目录继续作为公共组件源模块的结构部分。它们没有 `index.ts`，直接导入同级实现文件，并且只通过公共组件 `index.ts` 公开选定 API。
- 只读能力 Context 传递稳定的不可变数据或函数，例如准备好的消息。它不拥有变化中的 UI 状态或无限制 mutation；当后代共享会变化的功能状态时，使用 Context 作用域 Store 契约。
- Context Store 和 `use<ComponentName>Store(selector)` 保留在 `contexts/` 下的同一模块；绝不增加转发用 `hooks/useFilterableListStore.ts`。
- 单个组件使用的 Props 保留在其 `.tsx`；只有共享或有独立含义的数据才使用 `types/`。
- 所有 TypeScript 擦除后仍存在的值都不得放入 `types/`；组件自有的运行时常量可与消费者相邻，或在分组有帮助时放入 `constants/`。原生 TypeScript enum 继续受 `erasableSyntaxOnly` 禁止。
- 可选 CSS Module 保留在导入它的 `.tsx` 旁边；不得创建 `styles/`。
- Story 遵循 [`storybook.md`](storybook.md) 的所属工作区顶层 `stories/` 契约。
- 签入的 shadcn 源码不受此限制。

## 状态所有权

| 状态或行为                                 | 所有者                                   |
| ------------------------------------------ | ---------------------------------------- |
| 局部输入、展开、选择和焦点                 | 所属组件或内聚 Hook                      |
| 仅框架无障碍或视觉交互                     | 通用 UI 原语                             |
| 一个已挂载功能子树共享的状态               | 限定在该子树的 Store 实例                |
| 领域转换和不变量                           | 所属 Hook 或 Store 使用的纯转换          |
| 网络、流、订阅、定时器和浏览器资源生命周期 | 按责任聚焦的生命周期 Hook 或平台适配器   |
| 首次渲染所需启动输入                       | `createRoot(...).render(...)` 前的组合根 |

跨工作区仍以 [`package-boundaries.md`](package-boundaries.md#状态所有权) 为权威来源。

## 派生状态和转换

- 在渲染、selector 或内聚的 presenter 逻辑中派生值；绝不使用 Effect 把可派生值镜像进状态。
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

- `orchbee/hook-export-boundary`：自有专属 Hook 模块只有一个具名 Hook 运行时导出，与 camelCase 文件名匹配；允许相关类型导出。
- 使用 `useKeyboardNavigation.ts` 或 `useKeyboardNavigation.tsx` 命名。
- 私有辅助函数、常量和直接相关类型保留在 Hook 文件中。只为独立职责或消费者提取它们。
- 测试、Story、Context Store 模块和生成的 `packages/ui/src/hooks/use-mobile.ts` 遵循各自所属契约。

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

应用以下边界前，先按 [`docs/TESTING.md`](../TESTING.md#单元测试准入与删除) 选择测试。
组件、Hook 和 selector 不需要各自配套一份测试。初始 ref、普通状态 setter 和准备好的静态标记
本身不足以支持新增专用测试。

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
