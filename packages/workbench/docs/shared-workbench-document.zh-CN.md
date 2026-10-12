# 共享工作台文档和 Client View

## 范围和所有权

`@orchbee/workbench/document` 拥有两个不同权限的版本化合约：

- `WorkbenchSharedDocument` 是连接到一个应用程序运行时的每个浏览器页面和 Electron 渲染器页面共享的可恢复、修订的产品结构。
- `WorkbenchClientView` 是一页一次性导航和布局投影。它从不授权产品或终端生命周期变更。

该包验证并转换这些合约，但不执行存储、IPC、流程、网络、React 或 DOM 效果。应用程序拥有这些适配器并在边界解析不受信任的值。浏览器页面和 Electron 渲染器页面具有相同的客户端语义；两个合约均不包含平台鉴别器。

## 共享文档

```text
WorkbenchSharedDocument
├── version: 3
├── revision: canonical unsigned decimal text
├── projectWorkspace
│   ├── identity sequences, ordered optional Projects and independent launch-resource scopes
│   └── ordered Workspaces reference a nullable Project, stable resource scope and one Terminal Context
└── terminalContexts[] (canonical identity order)
    ├── Tab and Group identity, metadata, membership, and order
    └── Panel workspace
        ├── per-Tab Panel/Split hierarchy, orientation, slot identity, and order
        ├── terminal presentations: identity, title, launch CWD, and durable intent
        └── pending removals and allocation sequences
```

Presentation intent（呈现意图）恰好有五种形式：

| 意图        | 共同的持久意义                                             |
| ----------- | ---------------------------------------------------------- |
| `provision` | 指定的 provision 操作仍需要协调。                          |
| `retain`    | 预期存在既有技术绑定；绑定缺失时不得（也无法）新建替代品。 |
| `close`     | 指定的关闭操作和结构拆除仍悬而未决。                       |
| `stop`      | 应用程序退出正在停止绑定，同时保留产品布局/放置。          |
| `stopped`   | 产品布局/放置仍然存在，但预期不会有任何绑定或进程。        |

投影把 provision 失败映射到 `provision`；把 running、restoring、reconciling、restore-failed 或 exited 观测映射到 `retain`；把关闭状态映射到 `close`；把停止状态映射到 `stop`；把一次已完成的应用停止映射到 `stopped`。 Hydration 重建挂起的生命周期状态，但不分派任何进程命令。显式 reconciliation 稍后重放 provision、close 与 stop，并仅对 `retain` 使用恢复。

效果命令会显式区分破坏性处置：`terminal-close-requested` 在普通关闭和结构移除时携带 `remove`，在应用停止及其持久重试时携带 `stop`。这一仅属于命令的区别允许应用归档已停止终端，而不会改变五种共享意图，也不会把普通关闭视为应保留的产品历史。

公开停止重试是对当前 document 的一次批量操作：它使用新的 operation 标识重新启动每项可重试的
`stop-failed` 或 `close-failed` Presentation，同时保留已经停止与仍在处理的 peer。durability commit
失败会在任何 close command 前回滚暂存 stop；启动已停止 placement 会分配新的 Presentation 标识，
而不会恢复已经退役的 binding。

Terminal Context 可以在结构上为空：分配序列从 `1` 开始，Tab、Group、Panel、Presentation 和待处理
删除集合均为空。这是一种文档形状，不代表消费应用已提供远程 Workspace 准入或 provision。

Schema 为兼容文档保留 Environment placement。Placement 标识资源命名空间，不授予传输或进程能力。
应用拥有效果准入和提案来源检查；[Desktop 存储契约](../../../apps/desktop/docs/workbench-shared-document-storage.zh-CN.md)
说明其旧 placement 保护及仅支持本地的终端效果。

每个共享选项卡都存储一个持久回退 `title` 和可为 null 的持久 `customTitle`。产品创建的后备值是本地化且不带编号的“终端”标签；自定义标题是显式用户覆盖。主机派生的自然终端标题和当前的 CWD 是瞬态运行时观察结果，永远不会进入共享文档。显示层可以从持久启动 CWD 派生紧凑的目录标签，因此水合可以在新投影到达前恢复目录上下文，同时保持存储的后备标题和自定义标题不变。

## Client View

```text
WorkbenchClientView
├── version: 2
├── active Workspace-only location and expanded Projects
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
- 每个 Workspace 的 primary Directory 都能解析，可以使用 local 或 Environment placement。
  普通 Workbench 执行变更要求本地 placement；组织变更保留任一 placement。结构解析不会授权创建或
  重定向远程效果；消费应用负责自己的提案与运行时边界。
- 选项卡和面板工作区选项卡成员身份和顺序匹配；固定的选项卡形成一个前缀，并且每个组都是连续的。
- 每个 Split 恰好有两个子级及唯一插槽。子 Split 可以重复父级方向，因为该祖先关系决定局部关闭与调整大小行为。
- 每个 Presentation 只出现一次。待处理移除会解析为精确非空目标，彼此绝不重叠，并且只与具有相同移除标识的 `close` 意图匹配。
- `stop` 意图永远不属于挂起的删除。 `stopped` 从不隐式执行 provision 或 restore。
- 协调后的 Client View 每个共享终端上下文和选项卡只有一个条目，仅包含解析引用，并且为每个分割赋予每个槽一个非负权重，总计为 `100`。
- 两项契约都不规定数量上限。分配或适配器资源耗尽属于类型化失败，绝不会授权截断、驱逐或复用身份。
- 严格解析拒绝未知字段。共享文档损坏不可用且永远无法修复；无效 Client View 输入被拒绝；仅有效但陈旧的引用使用确定性协调回退。

## 兼容性和应用程序集成

`WorkbenchSharedDocument` version `3` 把有序 Project 分组与有序 Workspace、不可变启动资源命名空间分离。严格历史 v2 DTO 先验证完整旧图，包括 scope 内 Directory/配置引用、重复 ID、上下文归属和终端拓扑。迁移使用各旧 Project ID 作为资源命名空间，复制全部 Directory/配置（含未使用条目），保留 Workspace/context 身份、顺序、修订和持久意图，然后再次验证完整 v3 图。原始资源 ID 可以在不同命名空间合法重复。分配跳过已占用 ID，包括与新序列命名相似的历史 ID。

Version `1` 先通过严格历史 DTO 增加本地 Directory placement，再执行已验证的 v2 迁移。历史上省略的 Tab `customTitle` 仍默认为 `null`。部分、无效、未知版本或开发期不受支持的拓扑绝不变成空白替代状态。应用拥有迁移持久化。

Client View version `2` 使用不含 Project 身份的 `workspaceId`。严格 v1 迁移要求历史 `projectId` 字段，验证视图图结构，然后只移除该依赖分组的选择字段。针对 v3 的协调会在移动或解散后保留相同 Workspace。仅分组改变的共享发布保留 live context、Tab model 和 Panel store 对象、本地焦点/放大/子终端选择及终端 attachment。

只有共享更改才会推进应用拥有的 revision 并进入其 durability barrier。`subscribeDocument(...)` 与
`subscribeClientView(...)` 比较各自拥有的 projection 并独立通知，因此仅 client 的更改不能调度 shared
commit。Workbench 不仲裁 client；Desktop session coordinator 拥有单一 canonical writer、proposal
ordering 与 broadcast。

## 正确性证据

| 声明                                                       | 证据                                                                                                                                                                                                                                                       | 限制                                       |
| ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| 严格的共享文档保留每个二叉关系                             | 有效/格式错误示例、JSON 往返及生成的同轴层次结构                                                                                                                                                                                                           | 生成的边界：八个选项卡，每个选项卡四个面板 |
| 历史 v1/v2 migration 精确且单向                            | 严格 parser、字段保持与 Desktop file-authority test                                                                                                                                                                                                        | 仅严格历史 version 1 与 2                  |
| Client View 是隔离的，协调是幂等的                         | 双视图示例、陈旧参考回归和固定种子属性                                                                                                                                                                                                                     | 仅内存中查看合约                           |
| 水合会保留持久的终端意图                                   | provision/retain/close/stop/stopped 运行时往返                                                                                                                                                                                                             | Host 绑定是否存在仍由应用负责              |
| 自定义标题会持久保留，自然元数据保持瞬态                   | 标题覆盖/reset 与共享文档水合场景                                                                                                                                                                                                                          | 重命名 Presentation 由适配器负责           |
| 应用共享文档不产生终端效果（terminal effect）              | 具有结构去除和保留运行观察的运行时跟踪                                                                                                                                                                                                                     | 一个渲染器运行时                           |
| 空 Terminal Context 不携带 Presentation 意图               | [文档 schema 测试](../src/workbench/document/__tests__/workbench-document.unit.test.ts)                                                                                                                                                                    | 纯结构，不授予远程运行时准入               |
| 保留的 Environment placement 不能授予 Desktop 远程效果权限 | [Desktop 目标测试](../../../apps/desktop/src/main/client-session/__tests__/desktop-terminal-effect-target.unit.test.ts)和[提案保护测试](../../../apps/desktop/src/main/client-session/__tests__/validateDesktopClientRemotePlacementProposal.unit.test.ts) | 应用边界，不是共享 schema 执行能力         |
| 仅查看更改从不需要共享写入                                 | 投影特定观察员和 Desktop coordinator 指纹测试                                                                                                                                                                                                              | Shared ordering 由 Desktop session 负责    |
| 预览状态无法声明共享持久性                                 | 确定性拒绝测试                                                                                                                                                                                                                                             | 预览仍然是内存中的合成                     |
| 应用停止先持久提交再关闭，且可批量重试                     | [Stop 生命周期测试](../src/workbench/model/__tests__/terminal-session-stop.unit.test.ts)和[文档运行时测试](../src/workbench/model/__tests__/workbench-document-runtime.unit.test.ts)                                                                       | 受控运行时夹具；不声明 OS 终止保证         |

纯文档/视图投影及严格单向 v1/v2 到 v3 迁移使用确定性/属性测试。受控生命周期测试覆盖提交先于关闭、可重试失败、无 binding 的停止态、新替换身份以及位置、标题和 launch-CWD 的保留。Desktop 文档存储 CAS 保留独立有界协议模型；该模型不证明整个多 Client 或生命周期组合。
