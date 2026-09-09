# Terminal Host 模块边界

## 权威与范围

本参考把 [`runtime-model.md`](runtime-model.md#semantic-ownership) 中已接受的语义所有者映射到 Terminal Host 原生源码树。仓库范围的语义模块设计遵循 [`docs/MODULE-BOUNDARIES.md`](../../../docs/MODULE-BOUNDARIES.md#semantic-module-design)；Zig 命名、资源、安全与测试规则遵循 [`docs/ZIG.md`](../../../docs/ZIG.md)。本参考只负责 Terminal Host 源模块职责、可见性及允许的依赖方向。

## 模块图

每个跨模块导入都使用此表中的命名导入。直接相对导入保留在导入模块目录中。一行只能导入 `Allowed imports` 中列出的行；标准库、编译器提供的、生成的构建选项和固定的外部模块仅在影响边界时才会列出。

| 源边界                            | 命名导入或根                            | 责任                                                                                          | 允许的导入（imports）                                                                                                                                                           |
| --------------------------------- | --------------------------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/protocol/semantic_input.zig` | `terminal-input`                        | 编解码器和引擎共享的规范语义输入值类型                                                        | 无                                                                                                                                                                              |
| `src/protocol/`                   | `terminal-protocol` 至 `root.zig`       | 共享线路合约和二进制编解码器的本机镜像                                                        | `terminal-input`                                                                                                                                                                |
| `src/model/`                      | `terminal-host-model` 至 `root.zig`     | 规范主机标识符、配置、运行时主题、几何、生命周期、启动/配置文件、附件、投影、历史和选择命令值 | `terminal-protocol`                                                                                                                                                             |
| `src/platform/`                   | `terminal-host-platform` 至 `root.zig`  | PTY，进程遏制和操作系统适配                                                                   | 支持的 POSIX 目标上的 `pty-c`                                                                                                                                                   |
| `src/engine/`                     | `terminal-host-engine` 至 `root.zig`    | 一个终端引擎、Ghostty 状态、启动、运行时主题应用（application）、输入、投影、历史与编码       | `terminal-input`、`terminal-protocol`、`terminal-host-model`、`terminal-host-platform`、`ghostty-vt`                                                                            |
| `src/core/`                       | `terminal-host-core` 至 `root.zig`      | 纯主机、注册表、终端会话配置/主题、代理会话、附件和交付状态所有者                             | `terminal-host-model`                                                                                                                                                           |
| `src/control/`                    | `terminal-host-control` 至 `root.zig`   | 控制记录与键入的主机命令、回复、事件和完整主题状态之间的无损适配                              | `terminal-protocol`, `terminal-host-model`                                                                                                                                      |
| `src/service/`                    | `terminal-host-service` 至 `root.zig`   | 请求工作流程、运行时/主题协调、出口事务、维护和主机服务生命周期                               | `terminal-input`、`terminal-protocol`、`terminal-host-model`、`terminal-host-platform`、`terminal-host-engine`、`terminal-host-core`、`terminal-host-control`、 `build-options` |
| `src/transport/`                  | `terminal-host-transport` 至 `root.zig` | 本地 IPC 端点、连接成帧、对等验证和传输生命周期                                               | `terminal-protocol`, `terminal-host-service`                                                                                                                                    |
| `src/cli/`                        | `terminal-host-cli` 至 `root.zig`       | 启动器语法和经过验证的可执行配置                                                              | `terminal-host-model`, `terminal-host-transport`                                                                                                                                |
| `src/terminal_host.zig`           | `terminal-host`                         | 私有可执行外观和规范主机版本暴露                                                              | `terminal-host-cli`、`terminal-host-service`、`terminal-host-transport`                                                                                                         |
| `src/main.zig`                    | 可执行根目录                            | 流程组成和顶级诊断                                                                            | `terminal-host`                                                                                                                                                                 |
| `src/benchmark/`                  | `terminal-host-benchmark` 至 `main.zig` | 受控 PTY 吞吐量线束                                                                           | `terminal-host-cli`、`terminal-host-model`、`terminal-host-engine`                                                                                                              |

依赖关系仅从较低的行指向明确列出的较早的语义功能。模块永远不会通过相对路径导入另一个目录，不会通过另一个模块的根到达实现文件，也不会公开可变的内部结构，以便调用者可以绕过拥有操作。

## 内部审查接缝

上面的目录边界仍然是构建图权限。其中，以下有状态或依赖于平台的接缝是持久的审查单元，而不是偶然的文件分割：

| 边界          | 内部所有者                                                                                                           | 协调员或门面责任                                                                     |
| ------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 协议          | `message_kind.zig` 拥有准确的数字类型注册表；编解码器和格式模块拥有其有界线形状                                      | `contract.zig` 拥有版本/绑定常量和角色/编码/成功映射； `root.zig` 公开协议命名空间   |
| 模型          | `lifecycle/`，以及附件、交互、启动、配置与运行时主题值模块                                                           | `root.zig` 公开不可变的跨层值，无需导入有状态运行时边界                              |
| Engine        | `input_queue.zig`、`input_ownership.zig`、`history/`、`projection/` 和选择所有者                                     | `runtime/` 在规范终端锁与输入锁下安排跨所有者提交                                    |
| Engine 运行时 | `runtime/` 状态、句柄适配器、生命周期、I/O、主题、终端变更、投影、交互和测试接缝                                     | `runtime/root.zig` 保留一个公共命名空间，各事务文件拥有可独立审查的运行时操作        |
| 核心          | `state/` 连接、终端/主题、附件和投影交易及其专用状态所有者                                                           | `state/root.zig` 保留 Core 命名空间； `host_core.zig` 保留唯一的 Core 具体实现所有者 |
| 控制          | `decode_*` 和 `encode_*` 主机、终端、主题、投影、交互适配器                                                          | `decode.zig` 和 `encode.zig` 保留一个显式公共协议面                                  |
| 平台          | `process_session.zig` 拥有稳定的 POSIX 遏制标识；`process_posix.zig` 和 `process_windows.zig` 拥有各目标的进程适配器 | `process.zig` 选择目标实现而不重新实现进程行为                                       |
| 服务接入      | `runtime_aggregate/`、`runtime_maintenance/`、保留终端、主题、输入准入、历史记录、选择访问                           | `runtime_aggregate/host_aggregate.zig` 保留单一所有者；它的根保留现有的服务名称空间  |
| 业务出口      | `interaction.zig`、`projection.zig` 和 `outgoing.zig`                                                                | `event_delivery.zig` 拥有公平调度与整批（complete-batch）移交（handoff）             |
| 服务          | `terminal_requests/`、以功能命名的请求工作流、`connection_workflow.zig` 和仅测试组合观察器                           | `service.zig`拥有生命周期和组成；工作流程使用聚合而不是可变的核心内部结构            |
| 传输          | `connection.zig`、`local_server_posix.zig` 和 `local_server_windows.zig`                                             | `root.zig` 选择本地服务器实现并公开稳定的传输边界                                    |

一个语义所有者超出了一个独立可审阅的文件的范围，就成为一个目录。它的根源是一个狭窄的命名空间或机械外观；状态声明和分支事务逻辑存在于以职责命名的文件中。序列化提交协议在一个事务模块或类型化 prepare/commit 移交中保持显式，而不是将外观变成一个包罗万象的文件。

`src/` 下的每个手写生产 Zig 文件必须保持在 500 物理行或以下。测试是豁免的，因为它们可能是场景目录；生成的来源（如果引入）仍归其生成者所有，并通过该来源进行审查。接近限制是在文件越过限制之前提示按语义事务进行拆分，而不是压缩代码、删除有用名称或隐藏非类型化帮助程序后面的逻辑的原因。

所有者本地事务文件可能仅出于其记录的责任而改变为该目录声明的私有运行时状态。它不得创建第二个所有者、通过公共根公开状态、依赖于反向模块边缘或调用外观中的非类型化回调。

## 切入点和测试

- `root.zig` 文件是显式名称空间清单或当边界具有一个运行时标识时的实现条目。它们仅公开声明的消费者所需的操作和类型。
- `src/terminal_host.zig`组成产品运行模块；它不会成为第二个域所有者或广泛的测试外观。
- 单元测试位于所属边界的 `__tests__/` 目录，并通过具名模块导入边界。大型测试套件按语义所有者分区，只共享该测试目录中的领域辅助工具；不得仅为组织夹具而暴露生产内部实现。跨边界一致性测试继续位于 `src/__tests__/`，且只使用具名导入。
- `src/__tests__/host_black_box_test.zig` 是外部可执行边界细化。其相邻的传输助手仅充当本机客户端；构建的主机仍然是一个单独的子进程，测试仅通过启动器 ABI 和本地 IPC 协议对其进行观察。
- 生产根不会发布用于白盒测试的编解码器实现名称空间。当边界需要这样的接缝时，其根仅在 `builtin.is_test` 保护的 `testing` 命名空间下公开它。
- `src/service/__tests__/composition_observation.zig` 是仅用于测试的复制投影，由所有者提供的 Core 与 Engine 观察结果组装而成。它不保留领域状态，并提供在稳定请求与轮询边界后使用的共享跨所有者不变式校验器。相邻的 `composition_trace.zig` 词汇表和 `composition_reference*.zig` 可执行 reducer 只保留测试证据与由动作确定的抽象事实；它们不能授权或提交产品工作。它们与 `src/service/__tests__/` 下生成的状态机驱动器、重放编解码器和收缩器配合使用，并且只使用所有者提供的狭窄测试接缝，不会成为另一个 Service 协调器或进入生产命名空间。提交接缝证据和确定性调度也仍然仅用于测试：稳定矩阵将每个可能失败的提交边界映射到具名证据，而脚本化调度约束测试事件顺序，不会创建生产协调器或其他领域状态来源。
- 构建图是可执行的可见性权限。添加允许的边缘需要在相同的可接受的结构更改中更新此参考和 `build.zig`。
- 本机 C 和头文件与其所属适配器一起存在：`src/platform/` 下的进程遏制，以及 POSIX 端点安全性或 Windows 下的 IPC 本地 `src/transport/`。仅 Windows 黑盒客户端调用可以重用该传输 C 单元中的现有测试支持功能，而无需通过 Zig 产品模块公开它们。

## 审查门

当结构更改引入反向边缘、相对跨目录导入、循环、广泛重新导出、重复状态所有者或允许较低模块调用其协调器的回调时，拒绝结构更改。如果有效操作需要这样的边，请返回运行时模型并在更改图之前修改所有者或提交边界。
