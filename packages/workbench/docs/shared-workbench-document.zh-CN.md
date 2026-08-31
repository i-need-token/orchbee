# 共享工作台文档和 Client View

## 范围和所有权

`@orchbee/workbench/document` 拥有两个不同权限的版本化合约：

- `WorkbenchSharedDocument` 是连接到一个应用程序运行时的每个浏览器页面和 Electron 渲染器页面共享的可恢复、修订的产品结构。
- `WorkbenchClientView` 是一页一次性导航和布局投影。它从不授权产品或终端生命周期变更。

该包验证并转换这些合约，但不执行存储、IPC、流程、网络、React 或 DOM 效果。应用程序拥有这些适配器并在边界解析不受信任的值。浏览器页面和 Electron 渲染器页面具有相同的客户端语义；两个合约均不包含平台鉴别器。

## 共享文档

```text
WorkbenchSharedDocument
├── version: 2
├── revision: canonical unsigned decimal text
├── projectWorkspace
│   ├── identity sequences and ordered Projects with placed Directories
│   └── each Workspace references exactly one Terminal Context
└── terminalContexts[] (canonical identity order)
    ├── Tab and Group identity, metadata, membership, and order
    └── Panel workspace
        ├── per-Tab Panel/Split hierarchy, orientation, slot identity, and order
        ├── terminal presentations: identity, title, launch CWD, and durable intent
        └── pending removals and allocation sequences
```

演示意图恰好有五种形式：

| 意图        | 共同的持久意义                                   |
| ----------- | ------------------------------------------------ |
| `provision` | 指定的表示操作仍需要协调。                       |
| `retain`    | 预计现有技术约束力；缺席无法替代。               |
| `close`     | 指定的关闭操作和结构拆除仍悬而未决。             |
| `stop`      | 应用程序退出正在停止绑定，同时保留植入式广告。   |
| `stopped`   | 植入式广告仍然存在，但预计不会有任何约束或流程。 |

投影将配置失败映射到 `provision`；对 `retain` 运行、恢复、协调、恢复失败或退出观察；关闭状态为 `close`；停止状态为 `stop`；并填写完整的申请表至 `stopped`。 Hydration 重建挂起的生命周期状态，但不分派任何进程命令。显式协调稍后会重播配置、关闭和停止，并仅对 `retain` 使用恢复。

效果命令会显式区分破坏性处置：`terminal-close-requested` 在普通关闭和结构移除时携带 `remove`，在应用停止及其持久重试时携带 `stop`。这一仅属于命令的区别允许应用归档已停止终端，而不会改变五种共享意图，也不会把普通关闭视为应保留的产品历史。

Terminal Context 可以在结构上为空：allocation sequence 从 `1` 开始，而 Tab、Group、Panel、Presentation
与 pending-removal collection 均为空。Slice 3.2A 为 Main 准入的 remote Workspace 使用这一已有的有效
shape，使 durable placement 可以先于任何已授权 remote terminal effect，且不改变 document version 或
schema shape。

Slice 3.2B 使用同一 version-2 schema 承载一次显式的 first-terminal provision。Main-owned mutation 可以把
该精确 empty remote context 替换为一个顶层 Tab、一个 primary Panel 与一个 `provision` Presentation，
其 durable launch CWD 是已准入 remote Directory path。对应 process effect 不是 document authority：
Desktop 从 canonical Project/Workspace containment 派生 local 或精确 Environment target，先提交匹配
intent，再请求目标 runtime reconcile。Client 可以保留或删除完整 remote placement，也只能为已准入
Presentation 报告精确 `provision` 到 `retain` convergence；它不能创建、retarget 或以其他方式改变 remote
terminal structure。

每个共享选项卡都存储一个持久回退 `title` 和可为 null 的持久 `customTitle`。产品创建的后备值是本地化且不带编号的“终端”标签；自定义标题是显式用户覆盖。主机派生的自然终端标题和当前的 CWD 是瞬态运行时观察结果，永远不会进入共享文档。显示层可以从持久启动 CWD 派生紧凑的目录标签，因此水合可以在新投影到达前恢复目录上下文，同时保持存储的后备标题和自定义标题不变。

## Client View

```text
WorkbenchClientView
├── version: 1
├── active Project/Workspace location and expanded Projects
└── terminalContexts[]
    ├── active/selected Tabs, selection anchor, recent Group, and collapsed Groups
    └── per-Tab focused/maximized Panel, active child presentation, and Split weights
```

Client View 没有共享修订版。切换工作区或选项卡、聚焦或最大化面板、显示一个子终端、折叠组或调整拆分大小不会更改共享文档。共享分割层次结构和方向；权重是本地的，因此具有不同视口大小的客户端可以独立渲染相同的结构。

每个 Tab 的实时 Panel MRU 焦点历史是仅存在于 runtime 的状态，并从每项 Client View 的聚焦 Panel 初始化。它不会序列化进任一契约，因此初始水合不会恢复此前的焦点访问记录。

终端视口状态位于两个合约之外。每个投影终端客户端都拥有其 `AttachmentId`、尺寸、滚动位置、搜索输入和结果、选择、复制状态、GPU 资源和渲染器缓存。 `TerminalId`，绑定标识，PTY 字节，VT 状态、输出和历史记录保留终端运行时权限。

## 协调

`createDefaultWorkbenchClientView(document)` 派生完整且确定性的视图。`reconcileWorkbenchClientView(document, candidate)` 保留每项有效本地引用，移除未知上下文与引用，派生确定性后备，恢复缺失的均等 Split 权重，并按共享文档顺序排列结果。该操作幂等，绝不会改变共享文档，也不会创建/删除 Project、Tab、Panel、Presentation 或进程。

当共享操作还意味着本地导航时，共享结构结果和发起客户端的视图结果是单独的投影。例如，创建选项卡会将其添加到共享文档中，并仅在启动 Client View 中选择它。删除共享选项卡会导致每个客户端独立协调。

`WorkbenchProjectWorkspaceRuntime#applySharedDocument(...)` 会应用不同的共享结构、协调当前 Client View、不发出终端命令，并为持久记录未变的 Presentation 保留自然标题、当前 CWD 及丰富生命周期观察。若唯一差异是所提供的修订，运行时返回 `unchanged`，因为修订所有权属于应用协调器。

Client View 位于该基础的内存中。关闭页面会销毁它；除非应用提供一个由其单独拥有、供协调使用的候选，否则新页面接收确定性默认值。Client View 本身不会写入浏览器键值存储，因为它的引用集没有产品计数上限，并且普通的同源 `localStorage` 会意外耦合独立的浏览器页面。当前 Desktop Web 组合只存储一个有界、会话范围的焦点 `TerminalPresentationId` 提示，并把它投影到其余部分保持默认的候选中；它不会序列化 Client View。完整或跨重启 Client View 恢复需要单独获准的所有者和契约。

## 不变量

- 项目、工作区、终端上下文、选项卡、组、面板、拆分、面板插槽、删除和 `TerminalPresentationId` 标识在其声明的范围内是唯一的。
- 终端上下文使用规范的身份顺序，并且等于独立和工作空间上下文引用的确切集合。
- 目录、primary-Directory、启动配置和终端上下文引用解析。
- 每个 Directory 精确拥有一种 placement：`local`，或带一个非空 opaque Environment 引用的
  `environment`。Environment-placed Directory 始终 unavailable。
- 每个 Workspace 的 primary Directory 都会解析，且可以是 local 或 Environment placement。普通
  Workbench mutation 要求 local placement；只有 Desktop Main 可以准入精确 remote Workspace，并向其
  Terminal Context 追加 default top-level Terminal。Client mutation 不能创建或 retarget remote terminal
  structure；只接受精确保留、删除、已准入的 provision-to-retain convergence，以及精确的整 Tab close
  intent。
- 选项卡和面板工作区选项卡成员身份和顺序匹配；固定的选项卡形成一个前缀，并且每个组都是连续的。
- 每个 Split 恰好有两个子级及唯一插槽。子 Split 可以重复父级方向，因为该祖先关系决定局部关闭与调整大小行为。
- 每个 Presentation 只出现一次。待处理移除会解析为精确非空目标，彼此绝不重叠，并且只与具有相同移除标识的 `close` 意图匹配。
- `stop` 意图永远不属于挂起的删除。 `stopped` 从不隐式配置或恢复。
- 协调后的 Client View 每个共享终端上下文和选项卡只有一个条目，仅包含解析引用，并且为每个分割赋予每个槽一个非负权重，总计为 `100`。
- 两项契约都不规定数量上限。分配或适配器资源耗尽属于类型化失败，绝不会授权截断、驱逐或复用身份。
- 严格解析拒绝未知字段。共享文档损坏不可用且永远无法修复；无效的一次性 Client View 输入通过确定性协调回退。

## 兼容性和应用程序集成

`WorkbenchSharedDocument` version `2` 增加显式 Directory placement。其导出的单向 migration 只接受
一个有效的历史 version-1 document，为每个 Directory 增加 `placement: { kind: 'local' }`，且不改变
identity、order、revision、lifecycle intent 或任何其他字段，然后校验完整 version-2 result。它绝不接受
局部或 malformed v1 value，也绝不输出 v1。在历史 v1 parser 内，缺失的 Tab `customTitle` 会
canonicalize 为 `null`，从而保留此前 v1 contract。Desktop 决定何时持久化 migration，并保持 source
file 不变。开发阶段的 flattened 或 n-ary topology 继续不受支持，也不会被重新解释为 v1。

只有共享更改才会推进应用拥有的 revision 并进入其 durability barrier。`subscribeDocument(...)` 与
`subscribeClientView(...)` 比较各自拥有的 projection 并独立通知，因此仅 client 的更改不能调度 shared
commit。Workbench 不仲裁 client；Desktop session coordinator 拥有单一 canonical writer、proposal
ordering 与 broadcast。

## 正确性证据

| 声明                                                 | 证据                                                              | 限制                                                              |
| ---------------------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------- |
| 严格的共享文档保留每个二叉关系                       | 有效/格式错误示例、JSON 往返及生成的同轴层次结构                  | 生成的边界：八个选项卡，每个选项卡四个面板                        |
| 历史 v1 placement migration 精确且单向               | 严格 parser、字段保持与 Desktop file-authority test               | 仅精确历史 version 1                                              |
| Client View 是隔离的，协调是幂等的                   | 双视图示例、陈旧参考回归和固定种子属性                            | 仅内存中查看合约                                                  |
| 水合会保留持久的终端意图                             | provision/retain/close/stop/stopped 运行时往返                    | Host 绑定是否存在仍由应用负责                                     |
| 自定义标题会持久保留，自然元数据保持瞬态             | 标题覆盖/reset 与共享文档水合场景                                 | 重命名 Presentation 由适配器负责                                  |
| 应用共享文档不会产生最终效果                         | 具有结构去除和保留运行观察的运行时跟踪                            | 一个渲染器运行时                                                  |
| Empty remote Workspace placement 不携带 Presentation | Empty-context schema/runtime 与 Desktop admission/forgery test    | 仅 Slice 3.2A admission                                           |
| Remote provision 保持 target-exact 且 commit-first   | Workbench context test 加 Desktop routing、replay 与 forgery test | Default top-level terminal；projection 与 input 不属于本 contract |
| 仅查看更改从不需要共享写入                           | 投影特定观察员和 Desktop coordinator 指纹测试                     | Shared ordering 由 Desktop session 负责                           |
| 预览状态无法声明共享持久性                           | 确定性拒绝测试                                                    | 预览仍然是内存中的合成                                            |

一般的纯 document/view projection 不选择 TLA+，因为它不包含异步 completion、cancellation 或独立排序
event。单向 v1-to-v2 placement migration 由 `WorkbenchDocumentPlacementMigration` 单独覆盖；Desktop 的
commit/lifecycle 与 multi-client session model 拥有各自的异步 ordering contract。
