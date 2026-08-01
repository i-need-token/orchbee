# 前端体验组件

## 目的与范围

本规范让经过评审的产品界面知识在实现前可被发现，并约束高复杂度 UI 工作采用经过考虑、可重复的
组合。它治理各应用与包中面向浏览器和桌面端的 React 界面，但不取代 Primitive、React、包边界、
Storybook、本地化、Action 或产品领域权威。

| 术语      | 契约                                                                                 |
| --------- | ------------------------------------------------------------------------------------ |
| Primitive | 领域中立的框架组件，例如 Dialog、Menu、Input 或 Button；拥有无障碍与底层交互         |
| 体验组件  | 经过评审的组合；拥有可复用的交互结构、生命周期、层级或表单模式，但不拥有完整业务任务 |
| 业务组件  | 经过评审的产品语义组合；拥有一个内聚业务任务、准备好的状态与语义 Action              |
| Manifest  | 包本地的发现记录；概述一个体验组件或业务组件，并指向其证据与实现                     |
| 规范组件  | 在声明的 Intent 与受支持范围内，默认采用的已评审组件                                 |

目录是选择性的。不要登记每一个 Primitive、叶子组件、布局 Wrapper 或私有实现碎片。只有当组件经过评审的
结构能帮助未来的界面决策，而且意外重组很可能造成不一致时，才登记它。

## 准入信号

当拟议界面具有以下任一信号时，在实现前执行[发现工作流](#发现工作流)：

- 把多个 Primitive 组合成一个用户可见任务或有状态 Surface；
- 打开 Dialog、Menu、Context Menu、Popover、Drawer、Command Surface 或嵌套 Overlay；
- 拥有确认、取消、校验、加载、失败、键盘、焦点恢复或打开状态行为；
- 重复已有的 Row、资源编辑器、导航层级、搜索、Picker、Inspector 或管理工作流；
- 跨多个 Control 协调领域状态或语义 Action；或
- 引入足够多的可见元素，以至于其层级、顺序、间距或响应式行为需要设计判断。

由当前权威完全决定文案、且没有额外生命周期的单个 Primitive 可以保留在本地，不要求 Manifest。
只要符合任一信号，看起来实现很小也不能跳过发现。

## 所有权与放置

分类绝不覆盖语义所有权。

| 产物                     | 必需所有者与位置                                                                                                                                         |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 领域中立 Primitive       | `@orchbee/ui`，遵循 [`package-boundaries.md`](package-boundaries.md)                                                                                     |
| 领域中立体验模块         | 只有真实消费者和独立有意义的 API 证明应当提升时，才放入 `@orchbee/ui`                                                                                    |
| 产品体验组件             | 拥有其语义上下文的最窄应用或产品能力包                                                                                                                   |
| 业务组件                 | 拥有该业务概念、准备状态和 Action 的应用或产品能力包                                                                                                     |
| Manifest                 | `<owner>/docs/experience/<component-name>.json`                                                                                                          |
| Story                    | 所属工作区顶层 `stories/`，遵循 [`storybook.md`](storybook.md)                                                                                           |
| 跨仓库 Schema 与查询工具 | [`schemas/experience-component.schema.json`](../../../schemas/experience-component.schema.json) 与 `scripts/experience-catalog.mjs`；它们不拥有运行时 UI |

不要创建仓库级 `experience`、`business-components`、`shared-components` 或同类运行时桶。Manifest
让知识可以集中查询，却不把源码移出所有者。私有可见性继续保持私有：另一个工作区绝不能因为能发现 Manifest
就深层导入私有组件。当另一个所有者需要相同语义能力时，先按照正常的提取与公共边界决策移动完整所有权单元。

包本地 Manifest 记录是英文当前权威引用。它们跟随所属包，因此不进入仓库级中文规范镜像。

## 发现工作流

组合符合准入条件的界面前：

1. 描述用户 Intent 与语义所有者，不先指定实现。
2. 用 `pnpm experience:list` 获取紧凑目录，或用 `pnpm experience:find` 确定性缩小范围。优先使用
   精确的 `--intent`、`--kind`、`--owner` 和 `--status` Filter；自由文本使用词法匹配，绝不使用
   向量相似度。
3. 只对可能匹配项运行 `pnpm experience:show <id>`，然后检查引用的 Story、使用与避免条件、
   Invariant、依赖、源码和所属权威。
4. 只选择一个处理方式：**复用**、**扩展**、**替换**或**新建**。
5. 通过其受支持边界应用选中的组件。不得从截图重新拼出视觉结果，也不得把私有碎片复制到新所有者。

| 处理方式 | 必需处理                                                                                                     |
| -------- | ------------------------------------------------------------------------------------------------------------ |
| 复用     | 使用规范组件及其语义 API，不增加本地视觉分叉                                                                 |
| 扩展     | 在现有所有者中增加最小内聚语义能力；更新其 Manifest、Story、测试、引用与消费者                               |
| 替换     | 就变化后的交互获得决策所有者接受，迁移消费者，用 `deprecatedBy` 把旧 Manifest 标为废弃，并在没有消费者后移除 |
| 新建     | 确认没有准入组件满足 Intent，获得新可见结构的接受，在最窄所有者中实现，并在同一变更中登记                    |

发现采用渐进式披露，而不是 RAG。紧凑摘要让 Agent 能够推理已有词汇；只有可能匹配的组件才展开详细上下文。
选择候选项之前，不要加载所有源码或所有 Story。

## 组合约束

- 当规范体验组件或业务组件的 `useWhen` 条件匹配，且没有 `avoidWhen` 条件适用时，优先于新组合使用它。
- 在完成发现以及复用/扩展/替换/新建选择之前，不得直接用原始 Dialog、Menu、Form、Overlay 或导航
  Primitive 拼装符合准入条件的工作流。
- 新的准入组合在具备 Manifest 与证据前并不完整。不得用一个泛化的本地组件名掩盖未经评审的产品模式。
- 组件 API 表达产品 Intent 与受控 Variant。避免任意 Children 拓扑、Render Prop 逃生口、一次性 Slot
  Graph 或不受约束的样式覆盖，让每个消费者都能重新设计组件。
- 领域 Transition 与 Action 在视觉 Primitive 之外保持权威。体验组件协调展示和生命周期，但不创建
  平行业务写入者。
- 在所属组件内部复用 Design Token 与共享 Primitive。Manifest 不允许本地复制 Primitive 的几何、
  焦点行为、本地化或无障碍契约。
- 当现有规范设计不足时，先挑战现有所有者。更好的通用设计应改善所有受影响消费者；真正只属于一个
  所有者的差异保留在更窄所有者中。

## 直接排序交互词汇

- 可直接排序的有序集合在排序轴上使用挤位动效：被拖动项跟随指针；越过兄弟项中线时，位于两者之间的
  所有兄弟项通过动画进入空出的槽位；反向越过中线时恢复位移。
- 垂直集合把排序运动限制在块轴，水平集合把排序运动限制在行内轴。排序不得在交叉轴上引入滚动或溢出。
- 指针取消和 Escape 恢复拖动前的顺序与展开状态。松手通过所属语义 Transition 或 Action 完成排序；
  完成后的布局不得再次播放之前的位移 Transform。
- 同一容器内的线性排序不得用插入标志替代挤位动效。只有位移无法表达目标时（例如跨容器转移或树形
  嵌套位置），才能经过评审后例外使用插入标志。
- 规范排序体验必须在其 Manifest、Story 证据和测试中记录跟手、越过中线、兄弟项位移、反向恢复、
  完成与取消。

## Manifest 契约

每个 Manifest 都遵循
[`schemas/experience-component.schema.json`](../../../schemas/experience-component.schema.json)。

| 字段             | 含义                                                              |
| ---------------- | ----------------------------------------------------------------- |
| `$schema`        | 指向仓库 Manifest Schema 的相对路径                               |
| `id`             | 按能力命名空间划分、仓库级稳定的标识                              |
| `kind`           | `experience` 或 `business`                                        |
| `status`         | 下文定义的生命周期状态                                            |
| `name`           | 简短的人类可读名称                                                |
| `description`    | 解释所属交互或任务的紧凑检索摘要                                  |
| `owner`          | 所属工作区包名                                                    |
| `visibility`     | `private` 或 `public`；发现绝不扩大源码可见性                     |
| `intents`        | 用于确定性查找、采用 kebab-case 的稳定用户或实现 Intent           |
| `implementation` | 拥有组件的现有源码根                                              |
| `stories`        | Story 文件、命名导出、稳定生成的 Story ID，以及要检查的状态或交互 |
| `uses`           | 本组件组合的其他已登记组件                                        |
| `useWhen`        | 本组件是预期选择的条件                                            |
| `avoidWhen`      | 要求使用另一个组件或重新决策的条件                                |
| `invariants`     | 复用时必须保持的交互、状态、无障碍、所有权或副作用规则            |
| `references`     | 深入理解所需的当前权威包引用                                      |
| `deprecatedBy`   | 废弃 Manifest 必需的替代 ID；其他状态禁止使用                     |

路径采用仓库相对路径，并留在所属工作区中。组件 ID 全局唯一。每个 `uses` 与 `deprecatedBy` ID 都必须在
目录中解析，且 `uses` 组合图保持无环。规范组件始终引用至少一个确定性 Story。

## 生命周期与评审

| 状态         | 含义与允许用途                                                                 |
| ------------ | ------------------------------------------------------------------------------ |
| `candidate`  | 有明确所有者的已实现探索；可被发现用于评估，但绝不是新消费者的默认选择         |
| `reviewed`   | 已就声明的使用条件获得接受，但尚未成为该 Intent 的仓库默认词汇                 |
| `canonical`  | 匹配 Intent 与范围时经过评审的默认项；新消费者必须复用，或显式论证其他处理方式 |
| `deprecated` | 仅在消费者迁移期间保留；`deprecatedBy` 标识受支持的替代项                      |

决策所有者按照 [`../DEVELOPMENT.md`](../DEVELOPMENT.md) 接受用户可见结构、受支持 Variant、Invariant
和规范状态。实现可以选择可逆的私有名称与 Helper 位置。状态变化、实质上扩大的使用条件、新逃生口或变化的
Invariant 都属于受治理的行为或边界变化，并在同一变更中更新所有受影响证据与消费者。

## Story 与证据契约

- 规范 Manifest 引用由同一工作区拥有的确定性 Story。其 `states` 解释哪个状态或交互提供相关证据。
- 私有组件可以通过其公共组合根展示；不得只为了 Storybook 导出私有组件。
- Story 是检查证据，不是唯一正确性模型。领域 Transition、取消、异步行为、Action 和 Invariant
  继续保留由所属方负责的单元、集成、模型或属性测试。
- 原子更新 Story ID 与 Manifest 引用。Story ID 按 [`storybook.md`](storybook.md) 属于自动化接口。
- 凭据、实时服务、当前时间、随机性和机器专属状态绝不能成为目录的使用前提。

## 验证

从仓库根目录运行：

```sh
pnpm experience:list
pnpm experience:find --intent manage-resource
pnpm experience:show workbench.project-manager-dialog
pnpm lint:experience
pnpm test:experience-contract
pnpm storybook:build
pnpm verify
```

`lint:experience` 校验 Schema 新鲜度、包所有权、路径存在、命名 Story 导出及其生成 ID、组件 ID
唯一性和目录关系。查询命令按 ID 排序并返回确定性结果。根级 `lint`、`test` 和 `verify` Gate 包含
目录检查。
