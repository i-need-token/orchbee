# 术语表

## 权威与使用方式

跨工作区的产品概念、代码名称、序列化契约、文档与本地化文案使用本词表。功能所有者定义自身操作
和更细的术语，但必须保持这些实体边界。[架构](ARCHITECTURE.md) 负责依赖与运行时边界；
[跨端界面设计](INTERFACE-DESIGN.md) 负责它们的呈现。

下文拼写是规范名称，不代表现有标识符已全部完成迁移。所属源码、schema 和应用参考说明当前已实现的
表面。分阶段改名的具体映射和剩余消费方保留在对应 issue 中；更新这些消费方后才能声称改名完成。
本标准不会仅因命名一个概念就新增命令、字段、别名、类型或能力。

英文为权威。中文列选用现有本地化词汇；当当前中文文案使用英文角色名时，有意保留该名称。
产品说明通过其本地化所有者翻译，不翻译标识符或用户提供的名称。只有界面能明确指代对象时，
短标签才可省略限定词。

## 产品作用域

| 英文             | 中文       | 含义与区别                                                                                                                                                                                      |
| ---------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Session          | 会话       | 由 Server 服务的选定 Orchbee 运行时作用域，包含 Workbench 状态和协调的执行活动。客户端断开后它仍可继续存在。命名会话停止时可保留存储状态；运行中的进程或连接并不是它的定义。`Site` 是退役名称。 |
| Workbench        | Workbench  | 包含项目、工作区、终端上下文、标签页和面板，以及这些模型的客户端投影的产品组织与操作模型。它既不是 Server 进程，也不是终端模拟器。共享状态与客户端本地视图状态各有所有者。                      |
| Project          | 项目       | 工作区的命名组织分组。成员关系变化不改变工作区身份、目录或终端。项目不是 Git 仓库、检出、目录或运行时会话。仓库工具的工作区 `project.json` 是另一种工程层面的“项目”用法。                       |
| Workspace        | 工作区     | 具有稳定身份的工作单元，拥有自己的终端上下文、可选项目归属和可选主目录。移出项目或移动分组不会产生另一个工作区。关闭它和删除磁盘检出是不同效果。                                                |
| Directory        | 目录       | 与执行 Worker 关联、经过准入的文件系统位置，其身份独立于路径拼写。工作区可将其引用为主目录；项目分组不代表文件系统所有权。观测到的终端工作目录是另一项事实。                                    |
| Worktree         | 工作树     | 磁盘上的 Git 工作树，具有自己的检出和 Git 元数据关系。工作区可以使用它。关闭工作区不代表移除工作树或删除分支。不要由工作区 ID 推造工作树身份。                                                  |
| Terminal Context | 终端上下文 | 工作区内拥有有序标签页集合的 Workbench 作用域。它不是终端进程、选择状态或连接。                                                                                                                 |
| Tab              | 标签页     | 终端上下文中的成员；终端标签页拥有面板/分割树。其标题、分组和显示顺序不定义身份。它不是操作系统窗口或终端进程。                                                                                 |
| Tab Group        | 标签页组   | 终端上下文内的有序标签页分组。它不同于项目对工作区的分组。                                                                                                                                      |
| Panel            | 面板       | 标签页布局中的稳定叶节点，拥有主终端呈现及获准子呈现的位置。移动面板、更换显示的终端和替换终端附着是不同操作。                                                                                  |
| Pane             | 窗格       | 客户端为面板提供的渲染/附着表面，或带明确限定的插件表面。TUI 的 `PaneKey` 标识逻辑窗格，`PaneRef` 增加附着代次。它们不是第二套共享面板身份。共享布局操作使用 Panel，本地表面生命周期使用 Pane。 |
| Window           | 窗口       | 外层应用窗口或宿主终端窗口，必要时说明所属者。它既不是标签页，也不是面板。`Window` 是产品名词；`Windows` 是操作系统。                                                                           |

## 运行时角色

| 英文                  | 中文                  | 含义与区别                                                                                                                                                                                            |
| --------------------- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Machine               | 机器                  | 本地执行目标或已保存的远程连接配置。保存的机器选择目标和会话；标签或启用状态不证明 Server 正在运行。它不是 Worker 或 Terminal Host 进程。                                                             |
| Server                | Server                | 拥有原生会话的 Workbench 权威、准入、协调和发布职责的进程。`server` 命令操作选定会话中的该服务。不要把命令族改名为 `session`。                                                                        |
| Worker                | Worker                | Server 后方获准的执行协调者，将终端工作路由到 Terminal Host，并拥有执行侧能力。Worker 准入属于 Worker 协议，不是会话或客户端握手。                                                                    |
| Terminal Host (Host)  | Terminal Host（Host） | 拥有 PTY/进程生命周期、终端模拟、屏幕和历史的进程。只有在终端运行时上下文中才可简称 Host。SSH 主机名、承载 UI 的操作系统和外层终端模拟器是不同用法，必须加以限定。                                    |
| Client                | 客户端                | 消费某个权威公开边界的参与者。GUI 或 TUI 客户端投影状态并调用操作；无界面的 CLI 调用也是客户端。客户端本地选择和焦点不会成为共享 Workbench 权威。                                                     |
| Client Connection     | 客户端连接            | 一条已接受的原生客户端–Server 传输连接及其按 generation 限定的登记。连接结束时登记随之结束。一个 Workbench 客户端可持有多条通道/连接；连接 generation 不是会话 ID 或 Workbench 客户端 ID。            |
| Workbench Client      | Workbench 客户端      | 拥有自身 Workbench 投影状态、视图、通知、输入派发和通信通道的 TUI 运行时。它不同于 Server 的逐连接处理器，也不同于所观察的会话。对该运行时使用完整限定名，不使用 Terminal Session 或 Native Session。 |
| Agent                 | 智能体                | 执行工作并通过集成暴露观测信息的 AI 程序/provider 参与者。它既不是 Orchbee Worker，也不是运行它的终端。                                                                                               |
| Agent Session         | 智能体会话            | 智能体自身被跟踪的运行/对话及观测生命周期。`AgentSession` 保持为独立领域术语；provider 引用、进程观测和终端位置不会把它变成 Orchbee 会话。可能同时指代两者时，必须限定“会话”。                        |
| Terminal              | 终端                  | 运行时终端执行身份及其由 PTY 支撑的状态，通过 Terminal Host 和协调协议提供。其身份独立于显示它的面板、标题、智能体会话和附着。当需要区分执行与渲染表面时，使用 Terminal Runtime（终端运行时）。       |
| Terminal Presentation | 终端呈现              | 用于显示终端的 Workbench 位置/生命周期引用，包括主呈现和子呈现。呈现身份与运行时终端身份相互独立。                                                                                                    |
| Viewer / Attachment   | 查看器 / 附着         | 查看关系及其对某个终端代次的物理附着。使用所属协议的 `ViewerId` 或 `AttachmentId`；两者都不是会话 ID 或永久面板身份。控制权授权与查看权限相互独立。                                                   |
| Cohort                | cohort                | 一同准入、相互匹配的可执行文件和配套产物集合。其身份不同于会话身份、发布标签或已接受连接的 generation。                                                                                               |

## 标识符与外部拼写

先按含义命名，再应用语言大小写规则。Rust 类型使用 `UpperCamelCase`，模块和值使用 `snake_case`；
TypeScript 类型使用 `UpperCamelCase`，值使用 `camelCase`。crate/package 路径使用 `kebab-case`。
序列化字段遵循所属 schema，不执行全局大小写转换。下表给出规范词干及已有或已接受的表面拼写；
省略某个表面不代表允许新增它。

| 概念                | 类型 / 模块拼写                                                                                               | 序列化或外部拼写                                                                                                     |
| ------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| 会话运行时身份      | `SessionId`、`session_id`                                                                                     | `sessionId`；替换产品 `siteId`，包括 schema 定义和引用                                                               |
| 会话选择名称        | `SessionName`、`session_name`                                                                                 | `--session`、`--remote-session`、`ORCHBEE_SESSION`；这些选择名称，不是运行时 ID                                      |
| 会话运行时命名空间  | `SessionScope`、`session_scope`、`session_catalog`                                                            | 现有 `ORCHBEE_RUNTIME_BASE` 和 `ORCHBEE_RUNTIME_SCOPE` 仍是运行时位置输入，遵循所属优先级；scope 不是名称或运行时 ID |
| 会话记录            | `NamedSession*`、`RetainedSession*`、`SessionStore`                                                           | 保留会话含义；存储记录不证明 Server 存活                                                                             |
| Workbench           | `Workbench*`、`workbench`                                                                                     | `workbench.*` 操作命名空间；`orchbee.workbench.*` 文档保留原所有者                                                   |
| 项目                | `WorkbenchProjectId`、`project_id` / `projectId`                                                              | `projectId`、`project`、声明了的 `--project`                                                                         |
| 工作区              | `WorkbenchWorkspaceId`、`workspace_id` / `workspaceId`                                                        | `workspaceId`、`workspace`、声明了的 `--workspace`                                                                   |
| 目录                | `WorkbenchDirectoryId`、`directory_id` / `directoryId`                                                        | `directoryId`；`cwd` 是工作目录值，不是该身份                                                                        |
| 工作树              | `Worktree*`、`worktree`                                                                                       | `worktree` 命令/操作拼写；保留所属路径/来源选择器                                                                    |
| 终端上下文          | `WorkbenchTerminalContextId`、`terminal_context_id`                                                           | `terminalContextId`                                                                                                  |
| 标签页 / 标签页组   | `WorkbenchTabId`、`WorkbenchTabGroupId`、`tab` / `tab_group`                                                  | `tabId`、`groupId`；`tab` 命令族                                                                                     |
| 面板 / 窗格         | `WorkbenchPanelId`、`panel`；`PaneKey`、`PaneRef`、`pane`                                                     | `panelId`、`panel`、声明了的 `--panel`；已有插件 `focusedPaneId` 属于插件上下文，不是新的共享实体                    |
| 窗口                | `Window*`、`window`                                                                                           | 使用所属窗口适配器的契约；不推造会话或面板标识符                                                                     |
| 机器                | `Machine*`、`machine`                                                                                         | 所属名录的 `id`；`machine`、声明了的 `--machine`                                                                     |
| Server              | `Server*`、`server`                                                                                           | `server` 命令族和 `orchbee.server.*` 机器文档                                                                        |
| Worker              | `WorkerId`、`worker_id`、`local_worker_service`                                                               | `workerId`、`ORCHBEE_WORKER_ID`；本地 Worker 准入 magic 为 `OBLW`，版本为 `2`                                        |
| Terminal Host       | `TerminalHost*`、`terminal_host`                                                                              | 现有 `hostEpoch`、`hostId` 和终端协议身份保持各自作用域                                                              |
| 客户端连接          | `ClientConnection*`、`client_connection_registry`                                                             | OBLC `ClientAccepted { generation }` 表示客户端连接 generation；不新增 `clientConnectionId` 线字段                   |
| Workbench 客户端    | `WorkbenchClient`、`WorkbenchClientState`、`WorkbenchClientView`、`WorkbenchClientNotice`、`workbench_client` | 现有 `WorkbenchClientId`、`clientId` 和 `--client` 标识客户端视图/控制参与者，不是 OBLC 连接 generation              |
| 智能体 / 智能体会话 | `Agent*`、`AgentSession*`、`agent`、`agent_session`                                                           | 保留智能体 schema 的 `sessionId` 和现有 provider/会话选择器；字段拼写相同不代表智能体与 Orchbee 的身份域相同         |
| 终端 / 呈现 / 附着  | `TerminalId`、`TerminalPresentationId`、`AttachmentId`、`terminal`                                            | `terminalId`、`presentationId`、`attachmentId`、`terminal`、声明了的 `--terminal`、`ORCHBEE_TERMINAL_ID`             |
| Cohort              | `Cohort`、`CohortArtifact`、`CohortError`、`cohort`                                                           | 现有 cohort 字段保留产物准入含义                                                                                     |

五个服务文档标识符为 `orchbee.server.start`、`orchbee.server.stop`、`orchbee.server.diagnose`、
`orchbee.server.configuration` 和 `orchbee.server.agent-manifests`。其生产者是 `server` 命令族；
将 `orchbee.site.*` 替换成 `orchbee.session.*` 会错误命名该边界。文档版本遵循
[架构](ARCHITECTURE.md#数据边界)，独立于术语命名。

CLI 语法仍由 [CLI 命令表面](cli/command-surface.md) 管理。仅做代码改名不会更改命令、参数或环境变量。
已接受的会话选择器是 `ORCHBEE_SESSION`，不保留 `ORCHBEE_SITE` 回退或别名。Server 和 Worker
内部入口使用 `--runtime-directory <absolute-runtime-directory>` 替换 `--local-site`，不保留别名。
其值是绝对运行时目录；会话名称、scope 和运行时 ID 仍是不同概念。其他运行时输入保留现有契约。
现有 GUI 渲染器协调、浏览器认证和交互生命周期必须在各自边界分类，不能仅因标识符包含 `Session`
就机械地当成原生客户端连接。

## 退役名称与平台边界

| 禁用的用法                                                           | 规范含义                                                                                                                                 |
| -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| 产品 `Site`、`site`、`site_id`、`siteId`、“站点”                     | Session 及对应拼写；普通词语“website”“call site”和第三方名称不是产品别名                                                                 |
| 原生客户端–Server 准入的 `ClientSession*`                            | `ClientConnection*`；将 `ClientSessionConnection` 简化成 `ClientConnection`，不是 `ClientConnectionConnection`                           |
| Server 的 `WorkbenchClientSession`                                   | `WorkbenchClientConnection`；它不是 TUI 的 `WorkbenchClient`                                                                             |
| TUI 运行时的 `terminal_session`、`TerminalSession`、`NativeSession*` | `workbench_client`、`WorkbenchClient*`；视图可以保留各自状态/UI 模块所有权，不另造会话概念                                               |
| Worker 准入的 `local-site-protocol`、`LocalSiteMessage`、`OBLS`      | `local-worker-protocol` / `orchbee-local-worker-protocol`、`LocalWorkerMessage`、`OBLW`                                                  |
| `local-site-runtime` / `orchbee-local-site-runtime`                  | `local-runtime` / `orchbee-local-runtime`；库拥有经过准入的共享本地运行时能力，不是第二个会话权威                                        |
| 产品类型上的 `Windows*` / `Native*` 前缀                             | 按语义角色命名：`SessionName`、`RetainedSessionRecord`、`UserSettings`、`NotificationSettings`、`CohortArtifact`；不保留旧名重新导出别名 |

平台名属于操作系统/能力适配器，遵循 [Rust](RUST.md#平台特定代码)。平台专属实现可以位于
`session_store/windows.rs`，同时暴露 `SessionStore`；中立名称不代表所有平台都已实现它。
真实操作系统 API 和适配器自有表示保留准确的平台名称。不要从产品概念中删除 `Window`，也不要改名
上游 API。

不带限定的 Session 专指 Orchbee 运行时；保留带限定的 Agent Session。临时读取器、控制循环、连接、
投影或 UI 生命周期应使用对应角色名，不再引入另一种 Session 同义词。通过更细的角色名或模块限定
解决重名；不要仅为复用名字而合并不同的状态和 UI 投影。

## 变更与验证

术语变更须沿生产者、消费方、schema、CLI/文档投影及两套 locale 目录追踪身份与生命周期。
批量替换名称族前，审查相邻概念和明确例外。序列化契约及其绑定/夹具同步变更；除非任务另行授权
行为变化，否则保留所属身份宽度、权威和生命周期。

任务专属清单保留在 issue 中。已接受的概念或边界发生变化时维护本术语表；链接所属参考，不复制
实现清单。语义审查必须检查中英文文档对和有源码依据的映射：文档配对验证结构与已审阅内容，
不能证明翻译或改名正确。按 [文档](DOCUMENTATION.md#本地化) 确认变更的文档对，再运行
[验证](VERIFICATION.md#合并门禁) 规定的适用门禁。

<!-- documentation-pairing: english-sha256=c244bfb5babe682c5484617d32ffc03947c7b0fe15aecc97ee18cf54b24ddf80 zh-cn-sha256=8e419967a25270c064ba48c9f988c02b1df9ad506721397ba796733375458efe -->
