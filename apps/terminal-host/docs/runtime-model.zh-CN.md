# Terminal Host 运行时契约和正确性模型

## 状态与权威

本文档是`terminal-host`的权威运行时模型。接受的合同足够稳定，可以管理实施和审查，而应用程序仍处于实验版本状态。接受模型本身并不意味着当前的实现符合每个不变量；一致性需要下面定义的证据。

权限被刻意划分：

| 关注                                                 | 权威                                                                                       |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 主机运行时所有权、生命周期、交付、重试和并发语义     | 本文件                                                                                     |
| 面向操作员的用途、启动器使用和开发命令               | `../README.md`                                                                             |
| 传输记录、消息模式、编解码器、格式版本和协议安全限制 | [`@orchbee/terminal-protocol`](../../../packages/terminal-protocol/references/protocol.md) |
| 产品拓扑、恢复策略、更新策略、渲染、平台效果         | 桌面                                                                                       |
| 平台进程、PTY 和端点行为                             | 拥有的本机适配器及其本机测试                                                               |

实现注释、源布局描述或协议编解码器不得定义此处拥有的事实的第二版本。

## 目的

Terminal Host 是 Orchbee 的本地权威终端执行服务。对于每个端点的一个活动可信本地 Broker，它拥有：

- 保留终端身份及其生命周期；
- 每个终端的PTY和进程树；
- 规范的 Ghostty 终端状态；
- 终端配置和有序输入准入；
- 附件范围的投影、历史记录、选择和导出状态；和
- 活动 Broker 通过其访问这些能力的私有本地端点。

Host 的存在是为了使终端真相和终端资源不会移动到 Electron、renderer 或产品布局模型中。Broker 断开连接或重启会破坏连接范围的能力，但不会处置 retained terminal。

## 边界和非目标

一个端点最多允许一个活动代理会话。端点的操作系统所有权检查定义了信任边界：作为同一有效用户运行的进程位于同一信任域中。端点标识符不是秘密，并且主机不会针对同一用户应用程序对另一应用程序进行身份验证。这是单用户本地服务，而不是多租户隔离边界。

主机不拥有：

- 项目、工作区、工作台、选项卡、面板、代理或产品会话模型；
- 并发独立代理、同一用户应用程序隔离或授权策略；
- TCP、HTTP、WebSocket、TLS、远程认证或共享网关；
- 产品恢复选择、更新时间、UI、渲染、平台剪贴板内容或效果、外部导航或本地化产品副本；
- 搜索查询或匹配排名；客户端搜索有界文本详细信息行；
- 持久的 PTY 迁移、主机进程崩溃恢复或操作系统崩溃连续性；或
- 用于另一个工作区的可重用本机库 API。

桌面可能会重新连接到幸存的主机进程。如果主机或操作系统崩溃，合约不承诺其 PTYs 或终端进程可以恢复。

当前 Desktop adapter 仍是一种 Broker 实现。尚未发布的 Rust Worker 是相同单 Broker role 的新增
实现：它只在 Server admission 之后执行 same-user authentication 与精确版本 Host handshake，保留
已接受的 Host epoch 与 negotiated limit，并耦合 Server 和 Host connection liveness。这个仅连接
切片不发送 terminal command、不创建 PTY，也不增加 launch authority 或并发 Broker session。

拥有的进程树是在启动时建立的操作系统遏制域：Linux 和 macOS 上的专用会话、FreeBSD 上保留的直接子进程 reaper 子树，或 Windows 的 kill-on-close Job。在提交清理前，遏制必须观察到该域没有活动成员。在 Linux 和 macOS 上，故意逃逸 POSIX 会话的同用户子进程也会逃逸此契约；防止敌对的同用户代码分离不属于已接受的信任边界。FreeBSD 遏制跟随 reaper 子树而不是会话成员关系。

运行时还为每个已投影的实时终端派生一项非权威关闭活动观察。在 POSIX 上，`active` 表示另一个进程组拥有 PTY 前台；在 Windows 上，它表示 kill-on-close Job 除保留的 shell 外还包含其他进程。平台查询失败时为 `unknown`。平台查询成功且观察为不活动后，当前 OSC 133 状态仍可把光标位于提示/输入区域之外分类为 `active`，以覆盖在 shell 进程内执行的命令；光标位于提示/输入区域时为 `idle`。没有语义集成时，成功的不活动平台查询回退为 `idle`。已接受的影响活动的输入会立即使旧采样失效，但后续有界采样可以再次分类当前进程与语义提示状态。该值只为投影而采样：它绝不会改变生命周期、遏制或进程所有权，消费者必须保守处理 `unknown`。

拥有保留终端的 Host 即使没有 Broker 仍保持可用。既没有 Broker 也没有保留终端的 Host 可以退出。线协议中的 `ClientId` 是诊断元数据，而非连接权限；只有当前物理 Broker 会话能够授予附件能力。

该协议是实验性的，并作为一个完全兼容的版本进行协商。主机不携带兼容性转换层。虽然此合约仍处于试验阶段，但破坏性（不兼容）的 wire 变更可能会重置协议，但其 schema、TypeScript 和本机编解码器、所有生产者和消费者、一致性装置和受影响的模型必须原子地同步变更（atomically）。不兼容性仅关闭尝试的代理会话。

## 语义所有权

每一个可变事实都有一个权威所有者。仅当派生索引和调度程序队列可以在不更改拥有的事实的情况下进行验证、重建或丢弃时，才允许使用派生索引和调度程序队列。

| 业主                         | 权威事实                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `HostState`                  | 主机纪元和进程默认值                                                                                                                                                                                                                                                                                                                                                      |
| `TerminalRegistry`           | 终端成员资格、创建-身份绑定索引、清单生成以及每个保留会话的稳定引用                                                                                                                                                                                                                                                                                                       |
| `RetainedTerminal`           | 将一个 `TerminalSession` 分配并一对一绑定到一个延迟的 `RuntimePreparation` 或一个启动的 `TerminalEngine`；稳定的保留状态从不拥有任何一个或两者                                                                                                                                                                                                                            |
| 本地 IPC 连接                | `awaiting_hello`/`active`/`closing`、协商的协议限制、传输生命周期和物理连接                                                                                                                                                                                                                                                                                               |
| `BrokerSession`              | 活动连接标识符、诊断 `ClientId` 和附件成员身份                                                                                                                                                                                                                                                                                                                            |
| `TerminalSession`            | 终端标识、生命周期、提交的有效配置及其修订、提交的带有操作/修订标识的完整运行时主题、启动后不可变的初始启动几何图形以及可重建的上次同步引擎观察                                                                                                                                                                                                                           |
| `TerminalEngine`             | PTY/进程资源、Ghostty 规范状态、内容修订、生成、几何、终端范围的视口恢复目标、当前和默认动态颜色、配色方案、响铃序列、当前进度、一个有界挂起的剪贴板写入意图、一个有界 pending application signal FIFO、物理配置/主题应用、输入队列、附件范围的持有输入与焦点声明账本、终端焦点聚合、由投影互斥锁拥有的关闭活动缓存与新鲜度 epoch、有界关闭活动采样截止时间，以及故障观察 |
| `AttachmentSession`          | 终端绑定、不可变准入能力、当前输入能力及单调能力修订、需求、在途交付、响铃交付游标、历史锚点、选择及导出进度                                                                                                                                                                                                                                                              |
| 本地 IPC                     | 端点安全、接受/读/写/超时行为和记录帧                                                                                                                                                                                                                                                                                                                                     |
| 本机协议适配器               | 传输记录与键入的主机命令、回复和事件之间的无损转换                                                                                                                                                                                                                                                                                                                        |
| `@orchbee/terminal-protocol` | 线路模式、编解码器、消息类型、格式版本和协议安全限制                                                                                                                                                                                                                                                                                                                      |
| 桌面                         | 产品绑定、恢复决策、升级时间、平台效果、演示和更新                                                                                                                                                                                                                                                                                                                        |

`TerminalEngine` 仍然是一个语义所有者，但它不是一个实现文件。它的运行时源是一个审查单元目录：一个私有状态记录，一个窄公共句柄，以及用于创建、生命周期、遏制、故障防护、输入准入和写入、输出解析、终端控制、调整大小、投影、附件以及历史或选择交互的事务模块。每个交易模块都是其拥有的事实的唯一实现编写者；跨事务操作命名其锁定顺序并通过类型化操作调用其他所有者。将所有者移动到目录中不会创建另一个终端引擎、另一个 Ghostty 状态或另一个进程所有者。

同样的规则也适用于主机端所有者。 `state/` 保留一个具体的 `HostCore`，而连接、终端生命周期、附件和投影事务则位于单独的所有者本地文件中。 `runtime_aggregate/` 保留一个具体的 `HostRuntimeAggregate`，同时保留所有权、创建、附件生命周期、输入准入、配置、运行时主题、几何、终端控制、交互访问、投影、终止和维护编排是单独的审核单元。 `runtime_maintenance/` 将调度与不可变的运行时观察转换分开。它们的根保留了现有的命名空间；这些目录都不会引入另一个注册表、代理、保留终端映射或清理所有者。

特别是，终端状态和终端运行时不能存在于独立的权威映射中。主机运行时协调器分配一个`RetainedTerminal`，其相邻字段是完整的语义会话及其拥有的延迟启动准备或启动的引擎。 `TerminalRegistry` 索引对该嵌入式会话的稳定引用；它不复制会话或维护并行引擎查找。运行时访问通过该引用解析保留的记录，因此注册表成员身份和运行时所有权不能独立变化。附件由其代理会话拥有并引用一个终端；终端不得保留第二个权威附件注册表。源自主动附件需求的终端范围缓存是一次性加速器，而不是附件事实的第二个来源。

Engine 的输入清理账本不是附件注册表：它不包含能力或终端绑定，不能授权操作。具备 Core 输入能力的附件获准时，会在同一提交中创建对应空槽。分离或输入能力降级会先删除该槽，再由 Core 移除输入权限；升级则先准备并提交新的空槽，再公开新权限。账本只负责该附件已接受的精确按键与应用鼠标事实、附件焦点声明、这些声明在终端范围内的 OR 聚合，以及为释放它们预留的有界修复额度。Core 的投影需求仍是决定焦点声明能否存在的权威可见性事实。

`EngineObservation` 同样不是终极真理。它是在显式同步边界复制到主机所有者线程上的单调快照，因此 Core 可以验证交付并序列化一致的库存摘要，而无需通过非类型化运行时映射。它可以从引擎重建，并且绝不能超越它记录的引擎事实。提交的配置不同：`TerminalSession` 拥有其协议可见值和修订版。本机更新准备所有核心先决条件，将值应用于引擎，然后执行无误的会话提交；失败的物理应用程序会使提交的配置保持不变。运行时主题使用相同的所有权方向：`TerminalSession` 拥有完整的协议可见主题状态，而启动的引擎或延迟的 `RuntimePreparation` 拥有其物理应用程序。聚合验证所有者协议，应用完整的终端主题，然后才无误地提交下一步操作 ID 和主题修订。被拒绝或失败的效果会使双方所有者处于先前的完整状态。

公平后台同步器可以处理每个轮询的保留终端的有界子集。因此，`ListTerminals` 请求会同步它将在游标验证之后和对其进行编码之前序列化的确切有界页面；一个响应绝不会仅仅因为全局注册表大于一个轮询预算而将已观察到的页面前缀与不同步的后缀混合在一起。

### 空闲回滚历史压缩

固定版本的 Ghostty 拥有页面表示、不可解释的压缩活动 token 与增量压缩游标。`TerminalEngine` 拥有最近观察到的 Ghostty token、由终端互斥锁保护的交互 serial、250 毫秒单调空闲截止时间，以及最近一次 pending 或 dormant 调度结果。每次成功准备非空客户端输入，以及每个已提交的 PTY reader batch 或可见 Kitty 动画 tick，都会在终端互斥锁下推进交互 serial。Service maintenance 拥有可丢弃的公平扫描游标和一个进程范围的 20 毫秒机会 cadence。

每次机会按轮询顺序至多选择一个已启动的保留运行时，并只尝试终端互斥锁而不等待。锁竞争只会停顿，不会推迟已有截止时间。观察到 Ghostty token 变化会开始新的 250 毫秒空闲间隔；观察到交互 serial 变化只会在压缩处于 waiting 或 pending 时重启该间隔，complete 与 unsupported 工作只吸收这次观察而不会唤醒。到期且未变化的 token 与 serial 恰好允许一次 Ghostty `compress(.incremental)` 调用。Pending 工作只能在后续公平机会继续；完成后会保持 dormant，直到出现与压缩相关的 Ghostty 活动；延迟轮询绝不会突发补跑。Ghostty 报告 `unsupported` 后，不支持的目标也进入 dormant。

Ghostty 当前只压缩不与可编辑 viewport 相交的完整冷 primary-screen 回滚页面。在受支持的 64 位 Darwin 与 Linux 目标上，压缩保留每个虚拟映射和编码字节，同时回收原始物理 backing；测试构建会模拟这种回收，其他生产目标不执行压缩。这只改变存储表示：终端 revision、generation、geometry、逻辑行、projection、history、selection 与 delivery identity 都不改变。该调度不依赖 Broker，因此保留终端仍可参与；页面 backing 的节省不等同于整个进程的 RSS 保证。

### 组合观察与不变式校验器

仅测试组合观察是所有者提供的事实的复制投影，而不是另一个语义所有者。核心和引擎公开来自其自身状态的狭窄测试观察结果，而服务则通过连接和保留的运行时成员资格来组装它们。Engine 事实按规范的 projection→terminal→input 锁序复制；观察不拥有可变域状态，并且无法授权或提交操作。

组合校验器先运行 Core 与 Engine 各自所有者本地的不变式校验器，再检查跨所有者关系。保留运行时成员与 Core 终端成员一一对应；活动连接与 Core Broker 一致；已提交配置与终端主题完全一致；Core 观察到的修订绝不领先于 Engine，修订相等时几何也一致，固定状态的终端则要求精确匹配修订。每个附件保留一个正数能力修订，以及不可变的已准入读取/调整大小值；当前输入能力与 Engine 输入账本成员关系一致。每项 Engine 焦点声明都属于具有输入能力且 Core 投影需求为活动状态的附件，Engine 的终端焦点聚合也必须与该声明集是否为空一致。历史账本成员关系与投影游标同样要与各自所有者一致，且绝不领先于 Engine 的 generation 或 revision。显式的仅模型夹具可以省略原生运行时，但不能拥有或模拟运行时事实。

服务测试会在共享的成功请求和轮询边界后调用该校验器。有状态、生成式、重放和故障注入跟踪也必须在每项稳定操作或每次注入故障后调用它。

## 核心数据模型

以下 pseudo-Zig 描述了语义形状，而不是公共源名称：

```zig
const HostState = struct {
    epoch: HostEpoch,
    defaults: TerminalDefaults,
    terminals: TerminalRegistry,
    broker: ?BrokerSession,
};

const ConnectionState = union(enum) {
    awaiting_hello,
    active: struct {
        connection_id: BrokerSessionId,
        limits: NegotiatedLimits,
    },
    closing,
};

const TerminalRegistry = struct {
    // Each value is a stable reference to RetainedTerminal.session.
    by_id: Map(TerminalId, *TerminalSession),
    creation_index: Map(CreationId, CreationBinding),
    inventory_generation: InventoryGeneration,
};

const RetainedTerminal = struct {
    session: TerminalSession,
    runtime: union(enum) {
        awaiting_start: RuntimePreparation,
        started: TerminalEngine,
    },
};

const TerminalSession = struct {
    id: TerminalId,
    lifecycle: TerminalLifecycle,
    environment_overlay_digest: EnvironmentOverlayDigest,
    configuration: CommittedTerminalConfiguration,
    theme: TerminalThemeState,
    start_geometry: ?TerminalGeometry,
    observed_engine: ?EngineObservation, // derived, rebuildable after start
};

const BrokerSession = struct {
    id: BrokerSessionId,
    client_id: ClientId,
    attachments: Map(AttachmentId, AttachmentSession),
};

const AttachmentSession = struct {
    terminal_id: TerminalId,
    admitted_capabilities: AttachmentCapabilities,
    capabilities: AttachmentCapabilities,
    capability_revision: CapabilityRevision,
    bell_cursor: DeliverySequence,
    projection: ProjectionState,
    interaction: InteractionState,
};
```

主机、代理、终端、创作、附件、主题操作、请求、修订和交付序列的标识符是不同的类型。线路边界处的转换是显式的。零或缺失值不会默默地重新解释为另一种状态。

生产 `RetainedTerminal` 没有无主或空的运行时变体。立即创建在提交嵌入式会话之前构造其本机引擎。相反，延迟创建会深度复制并拥有其完整的准备启动，而无需构造子项、PTY、Ghostty 状态或几何图形，然后在一个免分配步骤中提交嵌入的 `awaiting_start` 会话、启动基线主题状态和两个身份索引。在开始更新之前承认的主题事务将拥有的准备和相邻会话一起更新。 `StartTerminal` 根据请求的精确几何形状从最新拥有的准备构造引擎，释放准备，并提交 `running` 加上相同的初始几何形状。启动失败会保留准备工作和未更改的 `awaiting_start` 会话，以便安全重试。处置或主机关闭会在销毁准备或引擎以及完整保留的记录之前删除稳定的会话引用。

Durable launch fingerprint 与 volatile environment-overlay digest 被有意分离。Overlay 使用目标规则规范化且与顺序无关，只有 digest 随精确 creation generation 保留；其值只在准备和持有 runtime process environment 时存在，绝不进入 inventory 或 diagnostic data。非空 create replay 必须匹配 retained digest。空重放只对已经保留的 creation 充当 wildcard，使 Desktop 可以在不重建已退役 capability 的情况下校验 durable recovery identity；不存在的 creation 携带空 overlay 时，只会创建一个没有 overlay 的 runtime。

确定性核心测试通过仅测试构建的存储工具分配纯独立会话；他们不会将模型运行时变体添加到 `TerminalSession`、生产代码或线可见状态。

### 终端生命周期

```zig
const TerminalLifecycle = union(enum) {
    awaiting_start,
    running: struct {
        termination: TerminationIntent,
    },
    finishing: struct {
        first: union(enum) {
            child_exited: ProcessResult,
            output_closed: OutputClose,
        },
        termination: TerminationIntent,
    },
    containing_failure: struct {
        failure: RuntimeFailure,
        containment: FailureContainment,
    },
    settled: TerminalOutcome,
};

const FailureContainment = union(enum) {
    stopping: struct {
        process: ?ProcessObservation,
    },
    blocked: struct {
        process: ?ProcessObservation,
        cleanup_failure: RuntimeCleanupFailure,
        retry_at_ns: MonotonicTime,
    },
};

const ProcessObservation = union(enum) {
    observed: ProcessResult,
    unavailable: ProcessObservationFailure,
};

const RuntimeCleanupFailure = enum {
    contain_process,
    release_output,
    release_process_identity,
};
const ProcessObservationFailure = enum { observe_process };

const TerminalOutcome = union(enum) {
    exited: struct {
        final_revision: Revision,
        result: ProcessResult,
    },
    failed: struct {
        failure: RuntimeFailure,
        final_revision: Revision,
        process: ProcessObservation,
    },
};

const TerminationIntent = enum {
    none,
    graceful,
    force,
};

const EngineFailure = enum {
    process_observation,
    pty_read,
    pty_write,
    resize,
    terminal_reply,
    worker_integrity,
};

const EngineCompletion = struct {
    child_exited: bool,
    exit_result: ?ProcessResult,
    final_revision: Revision,
    output_drained: bool,
    writer_stopped: bool,
    classification: union(enum) {
        pending,
        normal,
        failed: EngineFailure,
    },
};
```

`RuntimeFailure` 是一种语义操作阶段变体。其稳定的粗粒度 wire kind 派生自该变体，因此无法保留无效的种类/阶段组合。这不是英文产品消息。至少，不可恢复的 PTY 读取、PTY 写入、终端回复准入、resize 围栏（fencing）、进程观察与 worker-integrity 故障仍须可区分。主要终端故障类型不包括所有清理阶段。 `contain_process`、`release_output` 和 `release_process_identity` 仅在已保留主故障后存在于 `RuntimeCleanupFailure` 中。

`EngineCompletion.classification` 是唯一的完成权威。`normal` 与 `failed` 互斥，且 `failed` 会精确保留在线性化点首先发布的 Engine 故障。各 Worker 的诊断标志不能形成第二套完成模型。只有子进程完成、正常输出耗尽、写入器静止且结果已处理后，正常分类才合法。Host 只会把 Engine 故障映射到对应的语义 `RuntimeFailure` 阶段一次。

结果发布使用语义结果类型（`exited` 或 `failed`）以及终端标识和最终修订。它不会将生命周期重新编码为独立的故障布尔值。向任一固定变体的过渡都捕获了 `TerminalOutcome` 中的确切最终修订版。它是结果数据，而不是根据后来的观察重新计算的值。

`ProcessObservation.unavailable` 携带的故障始终是 `observe_process`。阻塞清理恰好携带一个 `RuntimeCleanupFailure`：进程树遏制、PTY 输出释放或进程身份释放。专用的上下文类型使主要/清理混乱以及无效的种类/阶段组合在主机模型中无法表示。协议适配器发出相应的固定线 `kind`/`stage` 对象。

若启动忽略几何信息，则在 `awaiting_start` 中原子创建保留终端；若显式几何信息允许立即启动，则在 `running` 中原子创建。构建到一半的终端不可观察。`awaiting_start` 拥有标识、有效配置和完整的已准备启动，但没有子进程、PTY、Engine 观察、几何、附件、终止意图或可见帧。此时只允许精确启动、清单读取、创建重放和直接处置。启动成功后，所提供的完整几何信息会成为初始 PTY、Ghostty、Core 和投影帧几何。生成失败会保留同一延迟启动所有者，使其可安全重试；成功后精确重放启动是幂等的，不同几何则是类型化冲突。

`finishing` 准确记录了第一个正常完成的事实。此状态下的 `OutputClose` 表示仅正常 PTY 排水。仅在等待工作线程对完成路径进行分类并且 PTY 写入器完全停顿之后，补充事实才会生成 `settled.exited`。因此，在提交正常退出之前，正在进行的写入仍然有机会成为终端的类型故障。进程观察或回收（reap）失败具有相同的优先级。正常和失败的分类是最终的选择；以后的处置诊断无法改写保留的正常结果。所有者请求的输出中止和自然 PTY 通过一个发布边界的耗尽竞争：读者在发布耗尽之前立即重新检查中止请求，同时保持终端互斥体。首先线性化的中止会抑制漏极发布；首先线性化的自然消耗仍然是有效的正常完成事实。

结算是终端拥有的变更屏障。一旦提交 `settled.exited` 或 `settled.failed`，规范内容、生成、几何、有效配置、配置修订、完整运行时主题与主题修订，以及已观察的内容修订，都会冻结在所捕获的最终修订。输入、调整大小、配置和主题更新均被拒绝。投影、历史、选择、导出、清单及结果发布只能读取该冻结最终状态后继续；Broker 作用域内的附件和交付状态仍可独立推进。待处理结果只发布一次，不会因后续读取或确认而重建。显式处置会被可重试地拒绝，直至精确的已结算结果交接提交发布；该拒绝会保留完整的保留终端记录、结果令牌与附件。

在 POSIX 上，即使每个 PTY 句柄都已关闭，所拥有的后代进程仍可能留在进程遏制域中。此时，根子进程结束且 PTY 正常排空仍属于终端的正常结果；保留的进程域会在显式处置时被强制遏制。在此之前不回收 POSIX 会话首领标识，可以防止会话标识符复用削弱遏制。因此，退出观察必须使用不会回收进程的平台观察。在 macOS 上，等待 Worker 先等待 `EVFILT_PROC/NOTE_EXIT`，再通过有界的非回收式 `waitid(WNOHANG)` 轮询读取退出状态；`EVFILT_PROC` 注册返回 `ESRCH` 时也进入同一轮询，因为 XNU 可能在子进程可等待前便耗尽进程引用。在等待一次成功注册时，Worker 还会按有界间隔执行非回收式观察，从而避免 XNU 在进程引用耗尽竞态中丢失 `NOTE_EXIT` 后使其永久等待。审计超时会先关闭该 kqueue 再执行观察，使其持有的进程引用能够耗尽；若子进程仍在运行，则重新注册。瞬态 `CLD_STOPPED` 绝不能成为子进程完成。观察失败属于类型化运行时失败，绝不能退化为在完成遏制前回收会话首领。最终标识释放使用 `waitpid(WNOHANG)`。若此前没有精确退出观察，`EAGAIN` 会立即返回并保留标识围栏。若已有精确的非回收式退出观察，process adapter 会在持有该围栏期间，仅对 `EAGAIN` 执行固定上限的重试，以跨过 macOS 中 `waitid` 已报告退出、但 `waitpid` 暂时仍无法回收的狭窄区间；重试耗尽时仍会保留围栏，等待以后再次尝试遏制、观察和回收。

### 稳定的 POSIX 进程标识（THCR-006）

数字 PID 绝不是效果权限。在 Linux 和 macOS 上，会话成员扫描会先为候选 PID 当前绑定的进程取得一个 `SessionMemberRef`，再执行权威的数字会话成员关系观察，并且只使用该保留引用判断存活与发送信号。成员操作结束后释放引用。macOS 只能把可复用 PID 上的 `getsid` 用作稳定引用获取前的负向预筛选；可能属于会话的成员在获取引用后始终要再次接受权威检查。如果无法准备引用获取，或无法可靠完成观察，平台操作失败，生命周期继续保有清理所有权。macOS 初始可复用 PID 预筛选返回 `ESRCH` 时可以跳过已消失的候选者；预筛选一旦匹配，后续引用丢失及所有非 `ESRCH` 观察错误都保守失败。不存在原始 PID 信号或存活后备路径。权威判定为非成员时可以跳过；已经匹配的引用若在信号期间失效，扫描必须失败并重试，而不能报告成功。继承同一数字 PID 的替换进程既不会成为目标，也不能通过已失效引用被当作空域证据；同一会话内的替换进程只能由新扫描与新稳定引用观察到。

- Linux 在 `getsid` 前调用 `pidfd_open`，并通过 `pidfd_send_signal` 发信号；实际信号返回 `ESRCH` 表示引用失效并须重试，绝不是静默成功。存活检查先轮询同一 pidfd，且不执行等待。没有就绪事件即证明该精确对象仍存活；`POLLHUP`、无效事件及轮询错误都保守失败。可读的非首领成员会通过同一 pidfd 发送信号 0 作为线性化点：成功证明精确的未回收僵尸进程，因此视为不存活；`ESRCH` 表示该 PID 可能已指向替换进程，须失败并重新扫描。包括 `EPERM` 在内的其他探测错误均保守失败。保留的会话首领是唯一可读特例：Host 拥有并延迟其 wait，因此它的 PID 无法复用；即使发生凭据转换，也可在不执行信号 0 权限检查的情况下安全判定为不存活。
- macOS 枚举所有进程 PID，而不是按真实或有效 UID 筛选，因为会话成员关系会跨凭据转换保留。它先以可复用 PID 上的 `getsid` 排除确定的非成员，然后在权威 `getsid` 与精确 PID 版本复检前取得 Mach task-name 权限及其 `TASK_AUDIT_TOKEN`。预筛选一旦匹配，引用获取丢失、`ESRCH`、已终止或无效的 task 权限、token 变化或精确 ID 丢失都会使整个扫描失败并重试；因此 exec 或同会话 PID ABA 绝不会形成伪空证据。信号通过 `proc_signal_with_audittoken` 发送，存活状态通过 task 权限和精确 PID 版本查询校验，成员操作后释放 task-name 权限。
- FreeBSD 在 fork 任何终端首领前验证 Terminal Host 是 process reaper。`PROC_REAP_KILL` 配合 `REAPER_KILL_SUBTREE`，以及 `PROC_REAP_GETPIDS`，都以保留的直接子进程首领作为子树标识；内核在持有进程标识时解析成员。遏制只回收该子树中已成为 Host 直接子进程的僵尸后代，绝不回收保留的首领、非子进程僵尸或其他终端子树的成员。所有终端的后代 wait 与保留首领 wait 在同一互斥锁下串行化。该互斥锁下，遏制执行有界的快照、定向回收与新快照轮次：新鲜活动成员返回未遏制；新鲜定向僵尸会强制下一轮；只有既无前者也无后者的新鲜快照才能返回零。进程抖动、截断、轮次耗尽或 wait/查询错误都保守失败并保留所有权。

每终端标识围栏串行化优雅 HUP、强制 STOP/KILL/CONT、遏制观察及最终首领回收。只有保留首领标识时才准入这些操作。回收前必须确认遏制；回收后绝不再查询数字会话或子树键。平台准备、枚举、稳定引用、信号、存活与回收失败都保留为类型化失败，并保存所有权状态以供重试。稳定成员信号扫描会记录首个成员错误，但仍尝试并释放此后每一项已捕获引用。同轮扫描中的永久错误优先于引用失效重试，避免后续扫描掩盖权限、资源或未知失败。在 Linux 上，native 信号适配器保留明确的 `EAGAIN` 失效结果，并在原标识围栏内最多执行八次全新会话扫描。每次失败扫描都先释放所有捕获引用及迭代器，再由下一轮重新绑定成员并检查成员关系。八轮上限约束瞬态退出或标识抖动中的重试，不增加 sleep、deadline、线程或所有权状态。仅完整成功的扫描返回成功；失效重试耗尽仍失败，旧引用及其复用的数字 PID 都不能授权效果。其他 native 错误、macOS/FreeBSD 行为以及遏制查询保留既有失败语义。部分成功的强制 KILL 在重试仍失败后继续进入完整的 CONT 补偿扫描；在 FreeBSD 上，`PROC_REAP_KILL` 调用成功但设置了 `rk_fpid` 也属于部分失败。

THCR-006 Host 终止屏障会在分派前捕获 pending runtime，随后完成该 runtime 的 shutdown，才恢复已准入的优雅或强制终止请求。仅有正常完成并不构成身份释放屏障：没有 PTY 句柄的后代仍可能需要所有者完成遏制，THCR-028 覆盖了这一情况。该屏障检查实际身份释放，并验证恢复的请求不会发出额外信号、会提交正常退出，且重放保持幂等。

Windows 仅在其kill-on-close 作业没有活动进程后才声明补充耗尽事实。在后代仍然存在时关闭伪控制台可以向该后代发出信号，并且可以在受支持的 Windows 版本上无限期地阻塞，因此作业空是 Windows 适配器安全耗尽条件的一部分。当共享生命周期可能离开 `finishing` 时，这种情况会发生变化；它不会创建第二个生命周期或允许不安全的正常分类。适配器按带有等待间隔的单调时间节奏观察 Job 统计，直到活动进程数归零；进程自然退出不会使 Job 句柄变为有信号状态。强制终止后有信号的句柄不能绕过该统计条件，也不能使重试变为忙循环。

Windows Process owner 保留一个成功提交根进程终止的凭据，初始为空。每个 Process 的短互斥锁
串行化终止系统调用及凭据发布；阻塞等待、join 或 PTY 关闭期间不持有该锁，锁内也不获取
其他生命周期锁。遏制可以持有既有的外层互斥锁调用强制终止；锁序为遏制锁再到终止锁，
从不反向。
优雅终止根进程成功或强制终止 Job 成功后，在返回前发布凭据。此后的优雅请求成为幂等空操作，
即使 Windows 仍在完成退出。强制请求始终仍作用于 Job，包括已有优雅终止凭据时，因为后代
可能仍然存在。失败的系统调用不会创建凭据。没有凭据时，优雅终止根进程的失败仅在对
保留进程句柄的零时间观察证明退出后才能忽略。强制终止 Job 的失败始终保留为错误，
即使根进程已经退出。退出状态 259 不用于判断存活。凭据仅证明终止提交已被接受：实际
子进程退出、Job 为空、PTY EOF 与遏制仍由各自既有的独立观察确定。

不可恢复的运行时故障会阻断新的客户端输入与终端生成输入，停止继续摄取 PTY 数据，保留最后一个已知一致修订，把在线性化点首先发布的故障记录为类型化主因，并且只启动一次强制遏制。整个遏制期间，清单与故障状态仍然可见。遏制尚未确认时，由运行时支持的投影、历史和选择都会暂停。在栅栏前捕获的投影帧仍须经过串行化读取提交保护，因此未确认的故障不能借助陈旧的可读性采样完成新的帧交接。遏制后，已有读取附件可以检查保留的一致状态；resize 损坏是例外，因为模拟器无法保证失败的变更仍留下可读屏幕。当确认所拥有的进程树已经停止且运行时资源已被遏制后，终端进入 `settled.failed`，发布其类型化结果，并在显式处置前保持可观察。进程观察失败后可以被标记为不可用；这与伪装成子进程正常退出不同。

主要的失败值是语义阶段本身；其较粗的线`kind`是由协议适配器导出的。独立的种类/阶段字段和无主平台代码占位符不是主机模型的一部分，因此不能保留矛盾或永久为空的诊断。

随后的清理或观察失败将被保留作为遏制诊断；它们永远不会取代终端的主要运行时原因。并发工作人员会竞相通过一个原子首次失败发布点，因此报告不依赖于布尔标志之间的硬编码优先级。

Engine 故障栅栏与第一次有界遏制尝试不依赖已连接的 Broker。Worker 只执行 Engine 自有的安全操作；它不会改变 Host 生命周期或连接状态。Host 在下一次转换过程中把不可变故障观察线性化到 `TerminalLifecycle`，并在该处完成或重试所有者清理。本地 IPC 即使在等待已接受连接完成握手或部分记录时，也会继续执行该维护；传输进度不是清理调度器的门槛。

### 输入准入和终端回复

客户端输入与终端生成的协议回复共用一个 PTY 写入器，但不共用准入预算。已接受的客户端输入彼此保持 FIFO 顺序；终端回复彼此也保持 FIFO 顺序，并在有界回复突发内获得优先级：写入器最多交付 `max_terminal_reply_burst_slices` 个终端回复切片，随后必须让给一个等待中的已接受客户端条目。每个切片至多为 `max_pty_write_slice_bytes`，因此突发会限制客户端条目需要等待的回复工作量。这样，当子进程需要回复才能推进时，回复仍排在大段客户端粘贴之前；但持续的回复工作不能无限期饿死已接受的客户端输入。突发耗尽后，写入器会让给等待中的客户端条目，并在该客户端切片之后重新开始计算突发；没有客户端条目等待时，回复可以持续推进。`InputAccepted` 仍只表示队列准入，绝不承诺相对于终端回复的交付顺序。

ANSI 换行模式 `20` 作用于 Host 所有的 PTY 写入准备边界。启用时，客户端输入或终端生成回复中的每个字面回车都会在队列预留计费前恰好展开一次为回车加换行。展开后的长度而非源长度必须满足协商的语义输入限制与对应的有界队列预算。准备过程在持有 `terminal_mutex` 时观察规范模式；后续模式变化不会改写已经准备好的 FIFO 条目。

语义按键和应用程序鼠标输入会在输入互斥锁保护下，于发布其客户端 FIFO 条目的同一线性化点提交由附件拥有的按住状态变更。准备、取消、拒绝以及分离后的迟到提交都不会改变账本或队列。按键事实保留实际接受的文本和修饰键；鼠标事实保留按下顺序和最近一次接受的采样。滚轮报告的已按按钮集合为空，并且永远不会被保留。语义按键点击由有序的按下与释放组成，因此没有稳态所有权；但如果附件在二者之间丢失，仍然可以修复。

语义鼠标准备还会在持有 `terminal_mutex` 时捕获一个由终端拥有的正数 mouse-input epoch。每次已接受的鼠标事件模式或格式模式动作、完整重置，以及确认回到本地 shell 的提示符恢复，都会按 `terminal -> input` 顺序推进该 epoch。影响活动状态的鼠标输入在最终提交时按 `projection -> terminal -> input` 加锁；如果 epoch 不同，则以 `input-mode-changed` 拒绝，即使该次编码为零字节也是如此。推进 epoch 绝不改写已经接受的有序输入条目。epoch 耗尽属于不可恢复的 worker-integrity 故障，不允许通过回绕制造 ABA 匹配。

只有无按钮的语义鼠标移动条目会被分类为瞬态 hover。已接受的字节会保留其客户端队列位置，直到写入器选中它。在写入开始线性化点，如果一个尚未开始的 hover 已不属于当前 epoch，则将其移除、释放，并完整退还计费，然后写入器重新选择。写入器会按 `terminal -> input` 顺序完成这次检查，因此 Ghostty 模式突变及其 epoch 推进之间不会暴露中间写入起点。按键、文本、粘贴、焦点、鼠标按下/释放、拖拽、滚轮、附件修复、原始客户端输入及终端回复都不会被这条规则退役。一旦某个条目的任意字节获准开始，即使 epoch 随后变化，该条目的每次部分写入仍会按原字节完成，因此 Host 不会截断控制序列。所有幸存客户端条目保持相对 FIFO 顺序。

OSC 133 新提示符标记（`A` / `fresh_line_new_prompt` 与 `P` / `prompt_start`）为子程序未关闭鼠标跟踪便消失的情况提供一条狭窄恢复缝，例如 SSH 客户端断线。Host 会同步采样保留的平台进程，但不会等待。只有成功观察到已经没有前台命令时，才可以清除全部规范鼠标事件模式、把鼠标格式恢复为 X10，并推进 epoch。前台命令仍活动时会保留模式，因此 Zellij、SSH 或其他嵌套 shell 内发出的提示符不能重置其所有者；观察失败时同样保留。解析器顺序仍然具有权威性：同一 reader batch 中更晚的鼠标模式动作最终获胜。没有 OSC 133 集成时，提示符恢复不可用，只有子程序正常发出的模式重置能自动恢复。

原始输入与语义输入会在二进制权限头中携带附件的正数能力修订。Core 在 Engine 准备前验证确切的当前修订与输入位，已准备输入在提交队列前还会再次验证二者。能力更新是按附件执行的单调 compare-and-set：当前修订和值构成精确重放；确切的下一修订必须改变输入位；旧、冲突或有缺口的修订会在不变更状态的情况下失败。降级先只把该附件按住的所有权转换为有序修复并删除其账本，再提交禁用输入后的新 Core 修订。已接受的队列条目保持原有的唯一 FIFO 位置；该转换不会复制或重新入队。升级会在提交新 Core 修订前预留一个全新空账本，不能恢复以前按住的所有权，也不能重放以前的输入。最初准入的能力值仍然是不可变的 `AttachTerminal` 重放标识，因此实时更新无法把精确 attach 重试变成冲突，也不能让该重试覆盖当前权限。

仅当附件具有活动投影需求和输入能力时，带精确修订号的焦点输入才会改变该附件的焦点声明。Engine 对终端的所有合格声明执行逻辑或聚合：第一个声明发出一个语义 `FocusIn`，后续声明不发出 PTY 字节；非最后一项声明的释放不发出 PTY 字节，最后一项声明的释放发出一个语义 `FocusOut`。重放附件的当前声明不会产生任何效果。暂停、禁用、已分离或修订号陈旧的输入无法创建声明。激活需求或升级输入能力不会创建声明，也不会重放焦点；必须提交新的、带精确修订号的焦点输入。启用 DEC 模式 `1004` 还会通过终端回复预算立即准入一条当前聚合状态报告。模式更新、聚合观察与回复预留按 `terminal -> input` 排序；已准备焦点输入会记录其观察到的报告模式，若最终提交时模式冲突则以 `input-mode-changed` 拒绝，因此启用动作无法越过一个未报告的焦点转换。

每个新按下的键或鼠标按钮都会预充电其最坏情况的释放字节，并且第一个终端焦点声明会预充电最坏情况的最终 `FocusOut` 字节。保持状态和非空焦点聚合各自根据有限的客户端输入预算保留一个未来修复条目。普通客户端输入无法消耗该信用。投影需求暂停修复并仅清除该附件的保持状态和焦点声明，同时保留其空输入分类帐；分离、代理丢失或输入能力降级执行相同的修复，然后删除分类帐。仅当删除目标声明使聚合为空时，修复才会对接受的密钥释放进行编码，然后对鼠标释放进行编码，然后对 `FocusOut` 进行编码。仅在修复提交后，Core 才会发布暂停的需求或删除输入权限。当运行时仍然存在时未能实现或提交修复是完整性故障，它会隔离运行时并进入其遏制路径；自然退出或已隔离的运行时不需要 PTY 修复。另一个附件的分类帐永远不会被释放、删除或允许产生中间聚合转换。

粘贴准备携带审查该投影的客户端（the reviewing projected client）所观察到的括号粘贴模式。引擎保存规范的终端互斥体，同时将该期望与 Ghostty 模式进行比较并对粘贴进行编码。不匹配是 `input-mode-changed` 拒绝并且不保留 FIFO 仓位；它是不可重试的，因为自动重播会绕过新的检查决定。

客户端字节与客户端条目限制不能消耗有界的终端回复字节与条目预算。客户端预算耗尽属于普通的可重试背压，绝不会变成终端故障。独立回复预算耗尽意味着子进程生成回复的速度超过有界写入器的承载能力；这才是实际的 `terminal_reply` 完整性故障，并走正常的栅栏与遏制路径。已准备的客户端输入会在准备回复前预留其 FIFO 位置与预算；取消输入会释放二者，且不接纳任何字节。

Ghostty 的终端处理程序仍然是唯一的终端状态变异器。Host 所有的流适配器把每个普通解析器动作与效果委托给该处理程序，并拦截有界的 `DCS $ q` 与 `DCS + q` 查询族，因为固定的无分配 `ghostty_vt.TerminalStream` 会有意忽略 DCS 动作。Ghostty 的 DCS 处理程序负责分类载荷。Host 从同一规范 SGR、光标和边距状态编码 DECRPSS，并从宣称的 `TERM=xterm-256color` 身份中经过审查的固定子集回答 XTGETTCAP：终端名称、颜色数量、直接颜色位宽，以及基线导航键与 F1-F12 键字符串。一个请求只产生一个有序回复，且每个名称与值都使用大写十六进制。首个名称无效或不支持时返回 `DCS 0 + r ST`；后续名称无效时终止列表并保留此前以分号分隔的 `DCS 1 + r ... ST` 成功前缀，与 xterm 的多名称行为一致。适配器不启用 XTSETTCAP、tmux 控制模式或第二个 VT 所有者。它的 APC 处理程序禁用 Glyph Protocol 识别：在端到端投影渲染器与资源合同获接受前，support、query、registration 与 clear 请求既不回复，也不改变 glossary。保留的来源独立语料库固定 DSR、主要/次要/第三 DA、XTWINOPS、配色方案、XTVERSION、ENQ、DECRQSS、XTGETTCAP、已禁用 Glyph 请求、畸形边界及解析器分片的精确有序字节。

### 固定 Ghostty termio/effect 清单

本清单针对固定的 Ghostty 提交 `9c3ec931d64561a8407dde7ac984ce156ae91539` 维护。每次更新 Ghostty pin，都必须重新审计 `src/termio/stream_handler.zig` 与嵌入的 `TerminalStream.Handler` effect 接口，Host 才能继续宣称兼容。“Host effect”表示结果由本机运行时拥有并受边界约束；“renderer/application effect”表示规范状态先跨协议，再由产品所有者执行；其余处置都不会被宣称为现有能力。

| 上游 effect 族                                                                                          | 处置                                         | Orchbee 合同                                                                                                                                                                                               |
| ------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 普通 VT 变更；mode 与 Kitty keyboard 查询；DA、DSR、XTWINOPS、配色/颜色、XTVERSION 与 ENQ 回复          | Host effect                                  | 固定的终端处理程序仍是唯一状态变异器。Profile、规范状态及完整的网格与单元像素几何进入有界 terminal-reply 队列；回复准入失败走类型化 `terminal_reply` 遏制。                                                |
| BEL 以及 OSC title/current-directory metadata                                                           | Host effect                                  | BEL 推进一个终端序列。Title 与原始 current-directory metadata 保持为规范终端事实并随修订投影；产品呈现不会成为第二所有者。                                                                                 |
| OSC 133 语义提示与 OSC 9;4 进度                                                                         | Host effect                                  | 语义提示状态细化关闭活动与有界完成反馈；进度只是一个有界的当前投影样本，不是通用通知权威。                                                                                                                 |
| 模式 `20`、`40`、`1004`、`2026`、`2048`，以及 DEC 80/132 列模式 `3`                                     | Host effect                                  | 输入展开、即时聚合焦点报告、同步投影释放、精确尺寸报告、DECCOLM 与视口恢复，都按 `TH-022` 与规范状态、PTY effect 和回复准入串行化。                                                                        |
| DECRQSS 与经过审查的 `xterm-256color` XTGETTCAP 子集                                                    | Host effect                                  | Host 所有的 DCS 适配器提供有界、有序、来源独立的回复，绝不安装子进程提供的 capability。                                                                                                                    |
| OSC 8 hyperlink 与规范 color/palette 变更                                                               | Renderer/application effect                  | Hyperlink 与颜色事实从规范状态投影。链接激活和浏览器绘制仍是限定范围的应用 effect；子进程文本不会授予本地文件权限。                                                                                        |
| 有界 Kitty graphics                                                                                     | Renderer/application effect（有界）          | Ghostty 拥有 direct/file/temporary/shared-memory 媒体、animation、virtual/relative 放置、裁剪、查询/删除与有符号 z 状态。Host 解析这些关系，只投影自有解码像素与绝对放置；源 capability 绝不跨越本机边界。 |
| OSC 52 selection kind `c` 纯文本写入                                                                    | Host effect 加 application effect            | Host 最多发布一个有界写入意图，由一个选定的可读附件应用。Query/read、其他 selection kind、二进制数据与无界内容属于明确的产品和安全边界。                                                                   |
| Private APC prefix `orchbee.agent.v3;`                                                                  | Host transport 加 application interpretation | Host 以事务方式发布一个有界 decoded UTF-8 application signal FIFO。它不认证 emitter，不推断 Agent/provider 语义，也不请求 notification；application 校验任何更窄的 payload 含义。                          |
| OSC 52 读取、公开 notification channel 与 OSC 22 指针形状                                               | 明确的产品边界                               | OSC 9、OSC 777、OSC 99、OSC 1337 等 channel 需要权限、限速、焦点、优先级与多客户端 effect owner 决策。它们不会从规范状态静默升级，也不会作为当前 Host 能力对外宣称。                                       |
| Glyph Protocol、XTSETTCAP 与 tmux control mode                                                          | 不支持的 capability                          | Glyph 在解析前被禁用，不能回复或保留 glossary 资源。XTSETTCAP 与 tmux control mode 均未启用；tmux 作为普通终端应用仍受支持。                                                                               |
| Title stack push/pop，以及 Ghostty 原生 shell、renderer、window、notification、updater 或 remote helper | 不支持或不属于此边界                         | 固定 Ghostty 的处理程序本身也未实现 title push/pop。Ghostty 原生应用集成不是终端模拟能力；若要采用，必须拥有各自的 Orchbee 产品合同。                                                                      |

### 有界真实应用维护矩阵

每次本地维护运行都会替换而不是累积 observation 列。2026-08-23 的运行在 macOS 14.8.7 上使用对外宣称的 `TERM=xterm-256color` 身份和一个已分配 PTY。`PASS` 表示程序能进入并退出预期终端表面，且未发生启动、terminfo 或几何故障；它是调查证据，不是可移植 CI gate，也不宣称浏览器像素一致。任何 `TERM`、XTGETTCAP、parser、mode effect 或 Ghostty pin 变更，都必须重跑适用行，并同时运行 Host 的来源独立语料与 real-PTY integration test。

| 表面          | 有界探针                                                                         | 2026-08-23 观察结果                                            | 证据边界                                                   |
| ------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------------------- |
| Terminfo      | `infocmp -1 xterm-256color`；`tput colors`；`tput pairs`                         | `PASS`：系统条目可解析；`256` 色与 `32767` color pair          | 确认对外宣称的本地身份，不代表每个可选 escape sequence     |
| VT diagnostic | `vttest -V`；在 PTY 中打开主菜单并选择 Exit                                      | `PASS`：vttest 2.7（20251205）成功进入并退出                   | 仅为启动 smoke；保留的兼容语料拥有确定性查询字节           |
| Shell         | 启动干净的 Zsh 与 Bash 交互会话，打印 `$TERM` 后退出                             | `PASS`：Zsh 5.9 与 Bash 3.2.57 均保留 `xterm-256color`         | 不宣称自动注入 shell integration                           |
| Multiplexer   | 启动分离的 90x30 tmux 会话，捕获 pane geometry 与子进程 `$TERM`，随后停止 server | `PASS`：tmux 3.7b 报告 `90x30`，并向子进程提供 `tmux-256color` | 仅覆盖普通终端应用路径；tmux control mode 仍不支持         |
| Editor        | 在 PTY 中打开 Vim 与 Neovim，查询 `term` 后退出                                  | `PASS`：Vim 9.1 与 Neovim 0.12.4 均观察到 `xterm-256color`     | 启动/全屏 capability smoke；编辑行为仍由应用拥有           |
| Pager         | 用 `less` 打开 `/etc/services` 后退出                                            | `PASS`：less 581.2 成功进入并退出                              | 有界 alternate-screen/input smoke                          |
| Modern TUI    | 在 PTY 中启动 btop 后退出                                                        | `PASS`：btop 1.4.7 成功进入并退出                              | 有界 modern-TUI smoke；不得据此推断 graphics protocol 支持 |

`TerminationIntent` 从 `none` 单调移动到 `graceful` 到 `force`；force 涵盖 graceful。接受的意图最多执行一次其平台效应。部分或失败的终止效果进入类型化失败遏制，而不是记录成功的意图。当平台报告进程遏制、PTY 输出释放或进程身份释放未完成时，可以独立重试故障清除。转换到失败永远不会丢弃 `finishing` 已保留的子退出观察。

如果所有者清理无法完成，终端将保留在 `containing_failure.blocked` 中。强制请求可能会重试清理。在类型化的失败清理阶段中拒绝处置；它不能默默地孤立进程树或释放工作人员仍可访问的内存。主机保留终端，直到进程遏制、工作进程静止和身份释放全部成功。主机按照有限的时间表独立于代理连接进行重试，并限制每次轮询的尝试次数； `retry_at_ns` 是内部单调调度状态，不是线路摘要的一部分。

### 致命拆卸保留清理 (THDD-001)

致命的拆卸是 `serveUntilIdle` 因可恢复类（如 `PeerIdentityMismatch` 之外的错误）失败后主机的最后一次关闭（例如侦听器描述符耗尽）。拆卸尝试停止并加入每个保留的运行时。当无法确认包含一个保留的 POSIX 进程树时（会话观察或信令在整个有界包含窗口中失败，或者活动成员在八次尝试限制之前从未产生两个连续的空观察），单次失败的传递必须 NOT 放弃该树。 THDD-001 明确所有权：

- **保留清理所有权**：未解决的故障终端继续使用 `containing_failure.blocked`。正常或失败结果已经解决的终端会保留该不可变结果，并在独立终端拥有的 `retained_cleanup.blocked` 状态中记录类型化的清理失败。两种形式都带有一个有界重试期限，该期限源自独立于代理的调度程序使用的同一主机生命周期单调清理时钟。聚合、运行时和 POSIX 会话身份仍然完全保留；遏制未得到确认，因此会话领导者身份永远不会释放（来自 THCR-006 的会话身份栅栏保持）。
- **有界退避节奏**：主机进入保留清理窗口，该窗口重用现有的有界清理重试调度程序。每次传递仅重试其 `retry_at_ns` 截止日期到期的清理，因此以相同的一秒有界回退节奏重试而不是旋转。该窗口被限制为固定数量的通过。
- **端点和生命周期锁定行为**：在窗口期间，本地端点保持绑定状态并且生命周期锁定保持不变（服务器仅在窗口解析后才取消初始化），因此在仍拥有未确认的树的情况下，没有第二个主机可以为同一端点启动。当生命周期关闭时，主机不接受新的代理会话。
- **可观察性**：保留清理状态通过可观察状态（`active`、`attempts`、`exhausted`）和完成/耗尽日志来显示，因此操作员可以看到清理正在重试，而不是默默地删除。
- **永久失败策略**：在有界预算耗尽且树仍未确认的情况下，主机不会默默地孤立或释放它。阻塞的清理变成稳定的显式耗尽状态，可观察的状态记录`exhausted`，该失败会被醒目地上报，拆卸以关闭失败退出，而不是假装干净释放。耗尽只是清理结果；它永远不会重写已经解决的正常或失败的最终结果。

### 附件传送

投影排序与交付构成同一个状态所有者。下一个序列和最高的废弃序列不是可以独立于标记的传递状态而前进的附件元数据：

```zig
const ProjectionState = struct {
    next_sequence: DeliverySequence,
    last_abandoned_sequence: DeliverySequence,
    delivery: ProjectionDelivery,
};

const ProjectionDelivery = union(enum) {
    paused,
    ready: union(enum) {
        snapshot: ?FrameBase,
        delta: FrameBase,
    },
    in_flight: struct {
        base_before: ?FrameBase,
        frame: FrameFlight,
        dirty_after_capture: bool,
    },
    blocked: struct {
        base: ?FrameBase,
        observed_revision: Revision,
        reason: ProjectionBlock,
    },
};
```

`FrameBase` 包含精确已确认的生成、修订与序列。嵌套的 ready 联合使“没有基准的增量”无法表示；快照可以保留旧基准，但只用于识别重复确认，不会用它捕获增量。被阻塞的交付会记录在 Engine 终端状态互斥锁下捕获或尝试的精确修订，即使编码失败时调度器已观察到更新修订。Core 会拒绝比当前终端观察还新的失败修订。只有当前观察不比失败修订更新时，阻塞才保持稳定；因此，惰性附件刷新可以让已观察到的更新修订进入快照就绪状态，无需另一次 PTY 变更或需求切换。尤其是，缺少已确认基准本身不能解除初始快照失败的阻塞。`base_before` 允许否定确认使当前在途交付失效，而不用虚构已确认基准。帧提交会在同一事务中推进 `next_sequence` 并进入 `in_flight`；回滚则同时恢复二者。暂停在途交付会先把其序列记为已放弃，再进入 `paused`，因此迟到确认是幂等空操作，而不具备支配更新在途交付的权限。

在批量切换之前，失败的保留或编码可能会回滚并重用未发布的序列，因为没有客户端可以观察到它。将精确的编码记录提交到已交付的批次中，会将 `(HostEpoch, AttachmentId, sequence)` 发布为不可变的交付标识：主机永远不会为该范围序列重新编码不同的字节或不同的提交元数据。如果切换后传输失败，代理会话及其附件将被销毁，而不是重用或重建已发布的序列。这个不变量允许能力范围的本地客户端从其固定标头中识别重复项，而无需再次对有效负载进行散列或解码。

投影帧编码并捕获后仍是临时状态。出口先预留记录与字节容量并接管唯一的编码分配，随后 Engine 在完成发布与失败发布共用的终端状态互斥锁上取得读提交守卫。守卫重新校验调整大小围栏、完成分类、捕获的代次以及一次性故障围栏 epoch，并一直保持到无分配的传出记录与 Core 在途交付提交完成。代次变化或故障发布会使此前所有临时帧失效。取消此类接管会释放容量与分配，但不推进在途交付、序列、投影节奏、调度器位置或可见传出批次。若守卫先胜出，则代次变更或故障发布者取得互斥锁前恰好提交一帧。确认遏制后可以重新准入新准备的保留状态帧，但不能复活围栏前的临时帧；调整大小失败绝不会准入保留读取。

DEC 同步输出模式 `2026` 抑制实时投影捕获，而不抑制规范解析、修订推进、历史变更或终端回复。启用或再次启用该模式会在 Engine 的单调时钟域内设置一秒截止时间。显式 reset、RIS 或应用 reset、成功几何变化、或截止时间到期都会同时清除规范模式与截止时间；下一个合格调度轮次只能发布最新一致修订。若无法取得单调时间采样或截止时间相加失败，Host 会立即清除该模式，而不会允许无界的呈现冻结。附件不拥有单独的同步区域，暂停或附加客户端也不会延长子进程拥有的截止时间。

历史行交付与纯文本选择导出遵循相同原则：每项在途交付都有一个正数且严格递增的序列；只有对它的精确确认才能提交。控制边界会在访问运行时前拒绝 HTML 和原始 VT 导出值。过时或重复的确认无法推进更新的在途交付。精确重放行确认，以及因需求、屏幕或代次转换放弃行在途交付后，序列不超过该附件已签发高水位的过时行确认，都是幂等空操作；两者都不能清除更新的待处理窗口或提升锚点。错误标识当前待处理窗口的元组，以及超过已签发高水位的序列，仍会被拒绝。暂停投影或销毁附件会放弃其在途交付并使其增量基准失效，但不会改变规范终端状态。

维护的 Ghostty 表面装置在本机/浏览器边界上细化了 `TH-006`。它们的字节仅通过启用模式 `2027`、通过一个固定的 Ghostty VT 馈送固定 UTF-8 流并使用一个真实的主机投影编码器来生成。主机验证重新生成 OBTF 快照，并将它们与协议拥有的装置进行逐字节比较；终端包通过其解码器、打包存储和两个渲染器使用主快照，然后按顺序提交配对的主/备用快照。这保留了字素边界、单元宽度、间隔拓扑、环绕布局和终端元数据的一个权限，而无需引入浏览器 VT。主语料库还通过软包装携带一个 OSC 8 范围，并在明确的硬线之前将其关闭；投影（projected）的搜索、链接和可访问性测试会消耗这些确切的行标志和单元格，而不是重建等效的合成行。它的最后一行设置动态默认颜色并使用显式RGB、反色、淡色、隐形、闪烁、上划线、删除线、每种Ghostty下划线类型、独立下划线颜色、粗体和斜体。相同的捕获样式记录由两个浏览器后端解码和渲染，因此主机一致性不依赖于单独手工构建的 TypeScript 样式装置。第二个快照进入备用屏幕，并作为一次投影提交更改标题、当前目录、光标、交互模式、生成、修订和行；运行时阅读器测试单独证明了1049代过渡。

同一语料库在其软包装中携带 OSC 8 目标，并在其最后一行中跨越 OSC 133 提示、输入和输出边界。主机编码发布 Ghostty 拥有的超链接、单元格语义内容和行提示类，而无需在附件中解析或推断它们。终端包使用这些字段来进行有界链接和语义工作流程；应用策略仍然是唯一可以执行外部开放效果的权限。

每个实时 OBTF 帧在其类型化可选扩展中携带运行时缓存的 `idle` 或 `active` 关闭活动观察；省略表示 `unknown`。这一三值线协议结构不公开 Host 的内部新鲜度 token。客户端可以用新鲜的 `idle` 省略破坏性关闭确认，但 `active`、`unknown` 或本地过时证据始终需要确认。该值不能授权进程控制，也不能推断遏制。历史 OBPB 行窗口不携带这一实时事实，因此应用历史窗口不能刷新或清除最新实时帧的活动决策。

关闭活动缓存、活动 epoch 与耗尽位都由 `projection_mutex` 拥有。成功的原始输入或非焦点语义输入（包括编码为零字节的输入）会在同一个 `projection -> input` 事务中提交输入准入和活动失效：缓存变为 `unknown`，活动 epoch 前进。遭拒输入和仅焦点输入不会使缓存失效。Engine 提交后，Host 所有者会使该终端的每个读取呈现失效：就绪 reader 要求快照，在途 reader 记录其精确在途帧解决后强制快照，暂停 reader 则保持暂停。

Writer 完成与 PTY 输出不会直接刷新缓存，也不能证明 shell 已消费获准输入。后续符合条件的采样会改为分类当前规范状态。平台成功观察到活动前台进程时始终发布 `active`；平台查询失败时发布 `unknown`。平台成功观察为不活动后，Ghostty 的当前语义提示状态会在光标位于提示/输入区域之外时发布 `active`，位于该区域时发布 `idle`。尚未观察到语义集成时，现有平台后备会发布 `idle`。因此，同一终端生命周期内的采样可以恢复 `idle`，包括物理输入交付仍在等待时，因为它描述的是当前进程与 VT 状态，而不是 shell 对某项特定输入的消费。活动 epoch 耗尽仍会保守失败：token 饱和，耗尽位阻止 ABA 匹配，缓存永久保持 `unknown`。

这一重新分类遵循固定 Ghostty revision `9c3ec931d64561a8407dde7ac984ce156ae91539` 的默认关闭确认决策：`Surface.needsConfirmQuit` 读取当前 `Terminal.cursorIsAtPrompt()` 结果，而不会永久记忆此前输入。Orchbee 采纳这一当前语义提示行为，但保留独立平台进程观察的优先级，将平台查询失败映射为 `unknown`，并通过 Host 协议传输采样得到的三态观察。Ghostty 的 `always`/`false` 配置模式、只读特例、直接同步调用时序、原生窗口组合与生命周期策略均不在该参考范围内。没有 shell 集成时，Orchbee 保留其平台后备，而不采用 Ghostty 的“始终不在提示符”结果。

采样按 `projection -> terminal` 加锁；影响活动的输入按 `projection -> input` 加锁；reader 提交获取 `terminal`；writer 只在 `input` 下记录完成。没有任何路径从 `input` 获取 `projection`，因此缓存失效既保留运行时锁顺序，也不虚构跨所有者交付证据。

Host 投影调度器在检查所扫描附件是否就绪前，按每个单调投影 cadence 对每个合格终端至多采样一次。因此，即使终端修订没有变化，或该附件已有帧在途，采样仍会继续。独立观察到的前台进程变化（包括没有 PTY 字节的静默进程组变化）会使每个 reader 失效，并产生同修订的强制快照。查询一经尝试，活动采样截止时间便会前进，因此传出批次已满或重试不能在同一 cadence 重复查询；普通临时帧及 Core 在途帧回滚仍保留精确的序列与重试状态。

每个编码帧都会在 Host 本地元数据中捕获活动 epoch 与耗尽位。读提交守卫先获取 `projection_mutex`，再获取 `terminal_mutex`；它会连同代次与故障围栏 epoch 一起重新校验该 token，并持有两把锁直到完成无分配的传出记录与 Core 在途帧提交。因此，编码与提交之间发生的输入或采样失效会拒绝旧帧且不消费其序列，包括计数器最大值的 ABA 情况。新的同修订快照会编码当前保守值。

输入回复与反向传输的帧之间没有排序屏障。投影客户端在本地发布影响活动的输入后，会将关闭活动保持为 `unknown`。如果此后第一帧仍携带 idle，客户端会以 `NeedSnapshot(S, activity-stale)` 替代 `FrameApplied(S)`。有序的客户端到 Host 数据流会把该请求排在输入之后；Host 只接受精确的在途序列，放弃它并强制生成快照 `S+1`。发生 Host 侧失效时，`S+1` 必须在其后通过活动 token 守卫。此后，无论 Host 是否接受了此前输入，它都可以携带后续当前状态采样得到的 `active`、`idle` 或 `unknown`。该屏障只证明排序与缓存新鲜度，不证明 Host 接受或 shell 消费；即使输入前的 idle 帧在本地失效后才到达，它仍然不安全。

### Kitty 图形投影模型

Kitty 图形保留在同一个终端状态与投影事务中。本模型要关闭的旧投影缺口是：固定版本的 Ghostty 已经接受文件媒体、临时文件媒体、共享内存、Unicode 占位符与相对放置、负 z 合成、动画及历史渲染图像，而原投影会丢弃这些信息。输入包括不受信任的 PTY 字节流、活动屏幕与回滚缓冲区几何、Host 单调时间戳、可选的历史区间请求，以及已声明的资源限制。参考范围是 Ghostty revision `9c3ec931d64561a8407dde7ac984ce156ae91539` 已实现的完整 Kitty 图形功能集，而不是 Ghostty 未指定的未来行为。

Ghostty 图像存储是解码图像、动画帧、放置关系和协议回复的唯一规范所有者。Host 拥有 Wuffs 解码器安装、临时目录生命周期、单调动画截止时间以及每份不可变的已解析捕获。它启用 direct、普通文件、临时文件以及平台可用时的 POSIX 共享内存加载；原始与 PNG 数据、zlib 压缩、分片、传输/显示/查询/删除、帧合成、动画控制、虚拟放置和相对放置仍是 Ghostty 原生转换。加载器与 Ghostty 一致的 400 MiB 采集上限和 Host 每屏幕 32 MiB 解码存储上限保持有限且彼此独立。Host 从不序列化源路径、打开的文件、临时文件句柄、共享内存名称或映射、PNG 容器或压缩流。只有 Host 拥有的已解码像素字节与完全解析的放置值可以跨越原生边界。

捕获执行 `advance -> resolve -> encode -> commit`。持有终端状态锁时，Host 首先推进任何到期动画，再把 pinned、virtual 与 relative 放置解析为裁剪后的像素矩形及绝对单元格/像素几何，最后把图像与放置排序成确定性的线协议顺序。OBKG 保留有符号 z 值，并把合成精确分为三层：`z < -1073741824` 位于单元格背景下方，`-1073741824 <= z < 0` 位于单元格背景上方但文本下方，`z >= 0` 位于文本上方。Canvas2D 与 WebGPU 消费者都在对应的渲染接缝绘制这些层；两种后端都不会重新解析 Kitty 命令或解析 Ghostty pin。

实时 OBTF 图形扩展类型 `2` 携带完整的图像资源快照与当前已解析放置，或仅放置替换并命名精确的先前已提交图像修订。终端快照以及每次图像 generation 变化都要求完整形式。只有当增量的已确认基线不早于安装相同可见图像集的捕获 revision 时，增量才能仅含放置；否则整个终端帧会提升为快照。render-detail OBRW 携带一份独立的完整图形快照，其中包含与选定行相交的放置，坐标相对该窗口重新定位并在请求时捕获；text-detail OBRW 不携带图形。历史是渲染快照，而不是对更早 VT 或动画状态的恢复。图像重传、可见性变化、删除、重置、屏幕切换、历史淘汰及重连都不能保留或复活过时的客户端资源。

投影调度器在终端状态锁下使用同一个单调时钟域调用 Ghostty `animationTick`，并为每个终端保留其返回的下一延迟截止时间。它比较 tick 前后的 Ghostty 图形 generation。一次 tick 若改变一个或多个已显示帧，终端 revision 恰好推进一次并使合格 reader 失效；没有显示变化的 tick 不推进 revision。未来截止时间会在没有 PTY 字节时安排唤醒。已提交的 PTY 批次只在活动屏幕或 Kitty 资源 generation 变化时唤醒该调度器；没有运行中动画则不安排唤醒。计数器耗尽或无法表示的截止时间进入既有类型化运行时失败或饱和有界时间路径，不能产生 ABA 有效帧或无界忙循环。

| 转换                                        | 规范效果                                                                       | 投影与失败规则                                                                   |
| ------------------------------------------- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| PTY 传输、查询、显示、帧、动画或删除 action | Ghostty 校验并原子改变其有界图形存储，或发出协议回复                           | 拒绝会保留此前规范图像/放置状态；客户端 payload 不会部分提交                     |
| Direct、文件、临时文件或共享内存加载        | Ghostty 获取有界输入并存储自有的已解码像素；临时文件与共享内存清理留在原生边界 | 路径、句柄、映射、编码容器与压缩字节绝不进入 OBKG 或 OBRW                        |
| 到期动画 tick                               | Ghostty 选择应显示的帧并返回下一延迟                                           | 图形 generation 变化会推进一次终端 revision；无变化是投影停滞步                  |
| 滚动、reflow、resize、屏幕切换或放置变更    | Ghostty 保持 pin 与放置关系的规范所有权                                        | Host 在捕获时解析当前绝对裁剪与层；未解析 pin 不跨越边界                         |
| 实时捕获与串行化交接                        | Host 针对一个 generation/revision 暂存确定性的资源与放置                       | 精确提交会一同发布全部字节与基线元数据；分配、限制或过时守卫失败时不发布任何部分 |
| Render-detail 行窗口捕获                    | Host 选择与所请求历史行相交的当前放置并重新定位                                | OBRW 原子提交行及其独立图形快照；text detail 排除图形                            |
| 精确确认、重置、删除、淘汰、分离或重连      | 交付所有权推进或使附件范围基线失效                                             | 过时确认不能推进资源；失效会强制完整的后继快照                                   |

macOS 与受支持、链接 libc 的 Linux 目标启用全部媒体。Windows 启用 direct、普通文件和临时文件媒体，但按照固定上游实现报告不支持 POSIX 共享内存；这一平台排除是显式的，不会用路径或复制侧信道模拟。这里的完整性指有界且固定 revision 的 Kitty 图形功能集，而不是无限图像尺寸或内存、每个 Kitty keyboard keysym、OSC 66、OSC 99，或 Ghostty 的应用程序/窗口集成。

一致性要求 [Kitty 图形投影 TLA+ 模型](../formal/tla/TerminalGraphicsProjection.tla)、有状态的放置/动画属性测试、与精确 Ghostty pin 做差分检查的来源独立语料、严格一致的 Zig/TypeScript OBKG 与 render-detail OBRW、能在 Canvas2D 与 WebGPU 观察全部三个绘制接缝的渲染器测试，以及每个所宣称平台上的原生执行或交叉构建证据。聚合 36 MiB 帧、33 MiB 图形扩展、32 MiB 解码资源、256 个可见图像、1024 个可见放置及 44 MiB 行窗口限制仍是彼此独立的准入检查；超过任一限制都会拒绝当前捕获，而不会削弱规范终端状态或最后提交的客户端基线。

启动配置文件种子 Ghostty 的默认前景、背景、光标颜色、完整调色板、光标形状和光标闪烁模式。缺少光标默认字段会标准化为块、非闪烁光标的产品默认值；当前配置文件明确保留这些值。 DECSCUSR 和等效的终端转义序列可能会覆盖活动形状和闪烁模式。 Ghostty RIS 和应用程序 `reset` 恢复保留的配置文件默认值，而投影仍然是渲染器的唯一光标输入。 OSC 4、10、11 和 12 更改会改变规范颜色状态：投影将调色板单元解析为 RGB 并携带当前动态默认值，而查询则从相同状态生成终端回复字节。维护的表面语料库涵盖突变和投影，运行时测试涵盖精确的 pinned-Ghostty 颜色查询回复。单独的 real-PTY 跟踪涵盖主机终端回复路由和计费，而不依赖于固定长度的颜色查询读取。没有附件或渲染器拥有第二个终端调色板或光标默认值。

持有规范终端状态互斥锁时，一次 Ghostty 响铃效果会让终端拥有的正数单调 `bell_sequence` 恰好推进一次。新准入且可读取的附件以当前序列初始化其 `bell_cursor`，因此附加、重连和保留终端恢复都不会重放先前的声音意图。附件存在期间，后续每个序列对应一项附件作用域的 `TerminalBell` 事件。调度器在一次轮询中至多交付下一个序列，且只有精确事件记录已附加到成功的传出批次后才推进游标。批次已满或分配失败时，该序列仍可重试；分离或 Broker 丢失会销毁游标，并丢弃尚未交付的响铃意图。响铃交付无需客户端确认，也不会改变投影、历史或规范屏幕内容。

Ghostty 仅在 selection kind 为 `c` 时准入 OSC 52：标准 base64 纯文本、有效 UTF-8、解码后至多 16 KiB。剪贴板读取 (`?`)、主要选择、无效的 base64、无效的 UTF-8、NUL、不允许的 C0 控件、DEL 和超大文本不会产生任何事件，也不会产生终端回复。读取器事务暂存已解码的分配，并仅使用完整的 PTY 读取批次来发布它。每个终端最多保留一个待处理请求；随后写入的 OSC 52 将被丢弃，直到该请求被移交或运行时被破坏。交互调度程序为其准备一个当前扫描的可读附件，如果编码或传出批次准入失败，则回滚准备工作，并且仅在确切的 `TerminalClipboardWriteRequest` 记录位于移交批次中后才将其删除。运行时范围内的正在进行的所有权可防止同一轮询或稍后轮询中的第二个附件接收请求。新的附件或重新连接永远不会收到已移交的请求。主机从不读取平台剪贴板、执行写入、存储信任或提交策略或接收应用程序结果；这些仍然是应用效果。

Ghostty unknown APC input 只在 content 以准确 private prefix `orchbee.agent.v3;` 开始，且剩余 unpadded base64url 能解码成最多 4 KiB、有效且不含 NUL 的 UTF-8 时准入。Truncated、empty、malformed 或 oversized input 不产生 signal。Reader transaction 随对应 PTY read batch 暂存 decoded signal；rollback 不发布任何内容。Commit 分配一个 terminal-owned 正数 sequence，并把 signal 追加到一个 64-entry FIFO；FIFO 已满时会保守丢弃新 signal，而不扰动已经准入的顺序。Egress 把准确 queue-head terminal identity、sequence 与 decoded body 编码为 `TerminalApplicationSignal`；encoding 或 batch full 会保留该 head，准确 append 只会消费它。Record 的 literal channel `orchbee.agent.v1` 是内部历史 Host-to-Desktop discriminator，不是 child wire protocol。OSC 9、OSC 777 与其他公开 notification input 不产生 application signal。该路径是不可信 application transport：它不认证 child，不派生 Agent/provider state，也不拥有产品或平台 notification policy。

Ghostty 进度报告使用其 PTY 读取批次以事务方式更新终端拥有的当前样本。状态为 `remove`、`set`、`error`、`indeterminate` 或 `pause`；存在（present）百分比时限定为`0..100`。投影在可选的 OBTF 进度扩展中携带样本，因此新附件接收当前活动状态，中间值可能合并，并且 `remove` 防止旧任务重新出现。进度永远不会改变生命周期、进程所有权、调度优先级、通知策略或终端内容修订。

Ghostty OSC 133 `C` 和 `D` 语义提示操作使用其 PTY 读取批处理以事务方式更新一个终端拥有的命令间隔。主机仅保留最新的完成序号、持续时间、挂钟完成时间和标准化退出代码。它从不保留或投影命令文本、参数、环境或输出。失败的读取器批次既不会发布开始，也不会发布完成。可选的 OBTF 命令完成扩展可让新连接的客户端看到最新的事实；Host 不决定新鲜度、焦点、通知权限、速率限制或客户端是否应该产生效果。

每个 `ReadRows` 请求都声明修订策略。`exact` 为绝对搜索、buffer 访问及其他确定性读取保留捕获修订标识。`current-view` 只允许用于来源为 `tail` 或 `committed` 的 `viewport` 请求；请求修订是观察游标，而不是准入围栏。Host 持有终端互斥锁时，根据处理当下的屏幕或附件拥有的跟踪锚点解析来源，并编码当前修订与行数。两种策略仍要求精确匹配当前投影代次。因此，持续 PTY 输出可以推进修订而不会饿死交互式视口读取；resize、屏幕替换和锚点淘汰仍是硬屏障。

每次 `HistoryChanged` 事件切换都会以事务方式分配一个正数、附件本地且单调递增的事件序列；需求变化绝不会复用或重置该序列，因此在终端没有发生变更时重新进入历史查看会话，仍会发布不同标识。`HistoryObserved` 携带被确认事件自身的序列。Host 只会在序列与修订/生成均匹配时精确清除当前在途事件；早于当前在途交付的确认——无论已观察、因需求变化或屏幕/生成失效而放弃，还是被后续事件取代——都是幂等空操作，绝不会清除更新的在途交付。

成功调整单元格网格大小可以推进内容修订与投影代次。它会放弃固定到代次的待处理行捕获，并取消已经编码的选择导出字节。如果 Ghostty 跟踪固定点在回流后仍然有效，已提交的历史锚点与活动选择会继续连接这些固定点；Host 会重新发布选择经过回流后的端点。屏幕替换、保留行驱逐或陈旧跟踪固定点会释放选择并报告 `evicted`，因此回流不会与内容丢失混为一谈。

每个附件还拥有一个固定的 Ghostty 选择手势。指针动作携带一个已捕获代次及视口相对的数值端点，但不携带修订标识。Host 在终端互斥锁内于处理时解析每个端点：查看历史时以附件已提交的跟踪锚点为基准，否则以当前活动视口为基准。因此，修订推进既不会拒绝动作，也不会在当前视图之外重新解释动作；代次不匹配、屏幕失效和锚点淘汰会取消动作。仅在可以安装点击行为和选择之后，开始才会替换先前手势；更新会依据有界表面几何应用 Ghostty 的单元内、矩形、跨单词、行或语义输出拖动规则；普通结束释放指针活动，同时只保留点击序列锚点。取消不携带点，并幂等重置手势、释放其跟踪固定点，同时保留最新选择快照；它绝不替代普通结束。精确修订选择命令、分离、代次或屏幕失效，以及状态拆卸执行相同重置。协议适配器会在运行时突变边界前拒绝无效的动作专属手势字段。

选择命令保留 Ghostty 的规范内容语义。全选会修剪周围的空白单元格，排除未使用的视口行，并保留未选择的空屏幕，而不是创建几何范围的选择。

## 命令和结果契约

主机边界在发送前验证每个信封。控制负载解析和每个控制回复或事件编码都经过本机协议适配器；表示验证和特定于命令的语义验证都在调用域转换之前完成。核心、运行时、投影和交互所有者永远不会解析原始 JSON 或手动组装控件 JSON。

协议拥有的消息契约确定注册类型作为请求、响应或事件是否合法，该角色需要哪种有效负载编码，以及哪种成功类型应答请求。主机调度和出口在域工作或切换之前咨询本机镜像；跨语言一致性语料库将该镜像与 TypeScript 注册表进行比较。业务调用者不提供第二个预期类型或编码关联。

每个操作都声明其重试类：

| 操作类             | 所需的语义                                                                                                                                                                                                                                                                                                                                               |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 只读库存或状态查询 | 可能会重复；每个回复都描述了一个连贯的观察结果                                                                                                                                                                                                                                                                                                           |
| 创建               | 当 terminal 保留在一个 Host epoch 中时，`CreationId` 加 durable launch fingerprint 是幂等的。非空 environment-overlay replay 还必须匹配 retained digest；空 overlay 只在校验该 retained creation 时充当 wildcard。精确重放返回同一 terminal，launch 或非空 overlay 冲突则返回 typed conflict。显式 disposal 释放 identity，而不是保留无界 Host tombstone |
| 推迟开始           | 第一个有效的几何图形恰好启动一个子进程。精确重放返回 `already-started` 以及提交的几何图形；不同几何体的重用是不可重试的 `start-conflict`。类型化的生成失败使 `awaiting_start` 拥有并可重试。                                                                                                                                                         |
| 绝对期望状态更新   | 重新应用相同的有效值没有额外的语义效果                                                                                                                                                                                                                                                                                                                   |
| 附加               | 附件标识符是会话范围的；精确的重播解析为相同的绑定，并且冲突的重用被拒绝                                                                                                                                                                                                                                                                                 |
| 分离、暂停和确认   | 完全重复是不需要操作的；陈旧或交叉附件值永远不会影响当前状态                                                                                                                                                                                                                                                                                             |
| 优雅终止和强制终止 | 幂等意图；升级时间属于桌面版                                                                                                                                                                                                                                                                                                                             |
| 输入和相关交互     | 未知结果后已排序且不可重播；自动重试可能会重复用户操作                                                                                                                                                                                                                                                                                                   |
| 处置               | 仅删除已发布确切结果的包含的已确定终端；挂起的发布或阻止的遏制会返回可重试的类型化失败；出现未知结果后 Desktop 通过清单进行协调                                                                                                                                                                                                                          |

对于每个请求，这些具有外部意义的结果之一适用：

- **拒绝**：没有发生语义提交；输入的错误表明重试是否有意义；
- **已应用**：发生了一次语义提交并交付了回复；
- **重放**：幂等性身份与较早的提交相匹配，并且没有发生重复效果；
- **结果未知**：在提交边界之后、代理确认回复之前传输失败。

传输丢失无法回滚已执行的 PTY 或流程效果。桌面可能会盲目地仅重试其重试类使其安全的操作。它必须协调或询问用户不可重播的操作，例如输入。

失败的处置会保留完整的保留终端。待结果发布返回 `outcome-publication-pending`；阻塞的运行时清理返回从确切所有者阶段派生的一个可重试类型化代码：`cleanup-contain-process`、`cleanup-release-output` 或 `cleanup-release-process-identity`。这两种障碍都不会演变成永久性阶段（phase）故障或一般终止故障。

## 提交纪律

每个容易出错的操作都有一个明确的 `prepare -> effect -> commit` 边界：

1. **准备**验证权限和状态，保留所需的内存和容量，并对提交前必须存在的任何响应数据进行编码。它不会改变权威状态。
2. **效果** 执行最小必要的 PTY、进程或平台操作。当效果可以部分成功时，首先定义其补偿或失败遏制转换。
3. **提交**在域分配方面是绝对正确的，并且一次更改权威状态。以后的域故障不会导致跨所有者转换的一半可见。
4. **传输切换**仍可能失败。这会产生终端范围效应的未知结果并关闭代理会话；它不会假装域提交已回滚。

运行时主题请求命名一个 `TerminalThemeOperationId`、一个预期的主题修订版以及完整的终端颜色状态：配色方案、默认前景/背景、可为空的光标颜色以及正好 256 个调色板条目。Host 在应用主题前保留完整结果记录。在序列化准入下，它会在比较和设置之前检查确切的重播：具有相同主题的当前操作返回 `already-applied`，将该操作标识与不同主题重用是不可重试的冲突，并且基于旧版本的不同操作返回 `stale` 以及完整的当前主题状态。这些路径都不会改变核心、延迟的准备或引擎。

在核心一次推进操作标识和主题修订之前，已批准的操作将每个终端主题字段应用于一个延迟的准备或启动的引擎。因此，延迟启动根据最新提交的主题构建其较晚的引擎。启动的引擎会在规范的 Ghostty 突变锁下自动替换动态颜色默认值及其当前活动值，清除早期的 OSC 4/10/11/12 覆盖，并将调色板投影标记为脏。如果改变活动终端颜色，则内容修订会前进一次，而生成保持不变；如果完整的默认值和活动值已经匹配，则即使独立主机主题修订提交了新操作，内容修订也不需要提前。所有附件都会投影相同的结果内容修订和颜色状态。

每次成功提交语义主题，都会使每个可读附件的呈现交付失效。因此，即使终端内容修订未变，就绪附件也会收到携带新 `themeRevision` 的全新完整快照。已经交给传输层的帧保持不变，并完成其精确确认路径；随后，该附件会被强制发送带修订标记的快照。被阻塞的交付以快照形式重新开放；暂停附件则通过常规恢复转换取得最新完整快照。主题修订是呈现屏障，独立于内容修订、投影生成、附件序列和历史锚点。

替换后，以后的 OSC 4/10/11/12 操作可能会正常覆盖新的运行时主题。它们的重置表单、终端 RIS 和主机拥有的应用程序重置恢复最新的运行时主题，而不是启动时颜色或早期的 OSC 值。光标形状和闪烁保持独立的保留配置文件默认值。任何验证、生命周期、修订溢出、运行时一致性或物理预检失败都发生在语义主题提交之前，并保留完整的先前状态。恢复清单始终包括当前操作 ID、主题修订版和完整的终端主题，包括带有空操作 ID 的修订版零启动基线。

仅当引擎恢复了最后提交的 PTY 几何图形后，平台调整大小错误才可重试。如果无法确认补偿，则引擎将被屏蔽并进入类型调整大小失败收容；主机状态从不假设失败的平台调用没有副作用。

Geometry 有两类已准入发起者和一个本机提交所有者。终端范围的视口恢复目标以完整 launch geometry 初始化。当前 resize-lease owner 的每个成功请求都会替换该目标：改变几何的请求只在 PTY 与 Ghostty commit 后替换；已经等于 canonical geometry 的请求仍可替换旧目标，但不产生 platform effect、generation 变化或 geometry revision。Lease transfer 本身绝不合成 measurement，也不改变目标。

PTY 输出也可以请求 Ghostty 启用的 80/132 列模式。Host 流适配器识别已解析 mode action，而不是重新解析字节；它与 attachment resize 串行化，应用新的 OS PTY grid 与 pixel extent，再在 reader batch 仍拥有提交时调用固定 Ghostty 的 DECCOLM。Cell pixel metric 保持不变，DECCOLM 也绝不覆盖 viewport target。每个显式 DEC mode `40` set、reset 或 saved-mode restore 都通过同一 transaction 恢复已保存的完整 viewport grid 与 cell-pixel geometry。目标已相等时，不产生 platform effect 或 geometry revision。改变几何的恢复会释放 synchronized-output suppression，并在 mode `2048` 启用时为已提交 geometry 准入精确报告。同一 batch 内的多个 column-mode 与 mode-40 action 按 parser 顺序执行，只有最终一致的 geometry 可被观察。

附件 resize 提交修订 `r` 后，Host 同步只在 `r' >= r` 时接受 `r'` 的 Engine observation。`r` 的 observation 必须包含请求的完整 geometry。PTY 输出可以在 resize 与 observation 之间提交；当它推进到 `r' > r` 时，较新的完整 geometry 与 revision 是规范值，Core 会直接复制，既不重写也不中止 Host。Resize acknowledgement 仍标识在 `r` 接受的请求提交，后续 projection 则携带较新的 observation。

对于每个运行中的 terminal，broker 最多授予一个可变 resize lease。Attachment admission
把不可变 `requestResizeLease` fact 与 current ownership 分开保留，因此 observer 在后来 transfer
后仍可精确幂等 attach。具备 resize 能力的 attachment 可以显式 claim ownership。Claim 当前
ownership 会返回无 event 的幂等 response；transfer 在 mutation 前预留 response 与两条 ownership
event record，原子移动 lease 与 successful-claim rank，发布前 owner 的 attachment-scoped
`ResizeLeaseChanged(false)` fact，再发布 claimant 的 `ResizeLeaseChanged(true)` fact，最后发布
完整 capability response。拒绝或预留失败会保留此前 owner 与 rank。

Detach 当前 owner 会原子选择 successful claim rank 最新的 eligible observer；从未获 grant 的
observer 以最早 broker admission sequence 打破平局。Host 在 mutation 前预留 ownership event
与 detach response，发布晋升 owner 的 event 后才发布 detach acknowledgement。若没有 eligible
observer，terminal 不再有 attachment resize owner，直到后来 attach 或显式 claim 请求 lease。
Settled terminal 拒绝 claim，也不会晋升新的 resize owner。

读取器提交只能围绕该平台效果释放其终端状态互斥体。受保护的读取器事务可防止关闭或不相关的故障栅栏将临时解锁视为已完成的批次，而 `resize_effect_in_flight` 则可防止附件大小调整超过它。失败的平台效果会跳过 Ghostty 模式突变和批次的其余部分，然后进入类型化调整大小失败遏制，因为子级无法接收普通请求拒绝。在成功的平台效果之后，Ghostty 调整大小失败首先尝试恢复之前的 PTY 几何图形，但仍然会阻止引擎，因为规范屏幕内存可能不安全。一旦成功，Core 会在投影或生命周期结算之前从一个引擎观察中复制列、行、单元格像素和修订；世代更改快照是客户端可见的调整大小通知。接收该快照本身并不产生反向附件调整大小请求。

应用程序请求的 `clear` 和 `reset` 是语义主机操作，而不是 PTY 输入。两者都需要一个能够输入的附件到正在运行的终端，并在保持与 PTY 输出相同的规范 Ghostty 突变边界的同时执行。它们从不向子级写入转义字节，进入客户端输入或终端回复队列，或者通过可能已经保存了部分子级序列的解析器提供合成字节。 `clear` 调用 Ghostty 的完整活动屏幕擦除并将光标归位，同时保留活动屏幕、终端模式、标题、当前目录、几何图形和单元格像素。 `reset` 调用 Ghostty 的完全重置：它返回主屏幕，放弃备用屏幕，清除屏幕和保留的历史记录，并将模式、标题、当前目录、光标形状和闪烁恢复为其保留的配置文件默认值，将动态颜色恢复为最新的运行时主题、制表位、与调色板相关的终端状态以及将交互所有权恢复为其重置值；几何形状和单元格像素保持不变。

每个接受的操作都会将终端生成和修订精确地推进一次，刷新历史模型，使投影基础和生成固定行或导出工作无效，并释放不再命名保留的 Ghostty 状态的选择或历史锚点。因此，下一个投影帧是每个附件共享的完整规范快照。发起客户端还修复 `reset` 之前浏览器拥有的瞬态：取消 IME 组合，通过普通语义输入释放按键和鼠标状态，通过普通选择命令清除其选择，清除搜索和悬停呈现，并恢复跟随底部。主机操作本身绝不会丢弃已经接受的输入。

启用 DEC 带内尺寸报告模式 `2048` 时，同一 reader 事务会立即从已提交规范几何排入 `CSI 48 ; rows ; columns ; height-pixels ; width-pixels t`。之后每次成功的附件或终端驱动几何提交都会先清除同步输出，再从该次精确提交的几何排入一条报告；无操作与失败的 resize 尝试不发出报告。报告准入使用独立的终端回复预算。无法准入必需报告属于类型化 `terminal_reply` 完整性故障，而不是几何已成功但未报告的 resize。

授权、运行生命周期验证和生成/修订溢出检查在 Ghostty 突变之前完成。一旦突变开始，它的域提交就不会出错。丢失的响应会使结果未知，并且不得自动重播：重复清除或重置可能会收敛到相同的像素，但仍会推进修订和交付身份。投影 `store-reset` 保留对无效编码帧基础的客户端恢复，并且永远不会解释为任一终端操作。

创建在提交任一保留形式之前保留注册和回复能力。入场前即刻生成；如果spawn成功但admission无法完成，则在操作返回之前强制包含进程树。延迟创建承认准备好的启动和身份，没有过程效果。 Start 在生成之前保留其接受的回复和所需的 `TerminalStarted` 事件，然后仅在完整的引擎存在后提交确切的请求几何图形。如果生成失败，这些记录将回滚，并且保留的准备工作仍可启动。引擎工作人员只能在引擎同步下改变引擎拥有的队列或规范的 VT 状态。它们发布不可变的生命周期观察结果，并且不会改变注册表、生命周期、代理或附件状态。主机在一个转换边界处线性化命令、运行时观察、计时器和确认。

只有 Host 同时没有 Broker 和保留终端时，正常自行退出才合法。在致命进程拆卸期间，停止某个终端时发生的失败会被记录，但不会阻止 Host 尝试停止并 join 其他所有终端。每个保留运行时均已尝试后再聚合错误。失败的拆卸不得被描述为干净释放资源。若失败的传输留下尚未确认的进程树，Host 不会跨越该边界退出：它保留清理所有权，并在有界时间窗中按照有界退避计划重试，直至进入永久失败状态，因此未确认进程树绝不会被静默遗弃（见“致命拆卸保留清理（THDD-001）”）。

## 出口事务

传出记录构建为连接范围传递状态上的事务。

- 请求分派在命令之前同步运行时观察，但仅发出由该请求引起的记录。生命周期发布、投影和交互调度在稍后的轮询中运行，因此异步编码或调度程序故障无法丢弃已提交的命令回复。
- 首先对记录进行编码并根据记录和字节限制进行检查。大型二进制生产者要么直接写入响应保留，要么返回一个拥有传输标头余量的分配。出口采用该分配而不进行复制。附加记录后，立即采用会转移所有权；临时采用会通过容量保留来转移它，取消容量保留会释放确切的分配。拒绝或批次满则由生产商放行。
- 仅当该确切记录包含在传递给本地 IPC 的批次中时，其飞行或确认状态才会提交。
- 即使无需客户端确认，瞬态响铃游标也遵循相同的“先附加、后提交”规则：每附加一项事件，附件游标就向前推进一个序列。
- 待处理的剪贴板写入意图有一个运行时范围的准备所有者，而不是每个附件一个光标。追加失败则回滚；一个成功的附加会消耗它，因此多个可读附件和重新连接不能增加特权应用程序的效果。
- Pending application signal 遵循相同的 prepare、append 与 commit discipline，但不选择 attachment。新的已准入 private-APC input 会在 predecessor 之后进入有界 FIFO；encoding 或 append 失败会保留 queue head，只有精确附加的 `TerminalApplicationSignal` 才会消费该 head。
- 如果下一条记录不适合，则正常返回已准备好的前缀，并且下一条记录仍符合后续批次的条件。完整批次是调度程序背压，而不是连接错误。
- 如果单个记录永远无法容纳，则拥有能力会进入类型化阻塞状态，而不会消耗序列或破坏基数。
- 在切换之前，分配或编码失败会回滚所有临时传送转换并释放一个所有者持有的有效负载分配。不会仅仅为了跨越引擎到出口所有权边界而重建大的有效负载。
- 在终端读取提交守卫被保留之前，投影采用仍然是临时的。门卫拒绝取消预订；保护成功使得传出记录和核心飞行提交免分配并且串行失败发布。
- 交接后，任何短写或写入失败都会关闭 Broker 会话，销毁其全部附件与传输中的在途交付；保留终端及其规范状态不受影响。

任何调用者都不能提交帧、行飞行、导出块或事件，然后丢弃应该携带它的批次。

### 语义跟踪词汇

可执行跟踪、引用减速器、收缩输出、重放输入和正式反例使用这些精确的稳定操作名称：

- `Connect`、`Disconnect`、`Create`、`Attach`、`Detach` 和 `Dispose` 更改组合成员资格或权限；
- `SetProjectionDemand`、`AcceptInput`、`Resize`、`UpdateConfiguration`、`ApplyTerminalControl` 和 `RequestTermination` 练习终端范围的命令；
- `ObserveChildExit`、`ObservePtyDrain`、`ObserveRuntimeFailure`、`ConfirmFailureContainment` 和 `PublishOutcome` 线性化异步生命周期事实；
- `Poll` 推进计划观察和交付，`BeginShutdown` 更改服务寿命，`Reconcile` 是结果未知操作后所需的观察边界。

每个已记录步骤都恰好包含 `applied`、`replayed`、`rejected` 或 `unknown` 之一。`rejected` 保持参考状态不变。`unknown` 会刻意禁用精确细化，直至已记录的 `Reconcile` 用一致的组合观察取代不确定性。可执行参考状态只包含 Action 所确定的抽象事实；所有者队列、运行时资源、锁及单调实现计数器位于其外部，并由各自所有者本地校验器检查。

一个外部原子调用可以线性化多个命名动作。减速器在比较其单个稳定的调用后观察之前应用完整的有序操作批次。例如，成功的`Poll`也可能包含`PublishOutcome`；对这些不可分割的提交进行比较会发明一种从未存在过的产品状态。

## 状态转换

### Broker 会话

| 目前状态         | 输入                                 | 下一个状态       | 所需效果                       |
| ---------------- | ------------------------------------ | ---------------- | ------------------------------ |
| 无 Broker        | 已接受本地传输连接                   | `awaiting_hello` | 分配新的物理会话标识           |
| `awaiting_hello` | 有效兼容你好                         | `active`         | 提交协商的限制和代理元数据     |
| `awaiting_hello` | 超时、畸形或不兼容的 hello、断开连接 | 无 Broker        | 只关闭该传输；终端不变         |
| `active`         | 断开连接或传输失败                   | 无 Broker        | 销毁全部会话附件；终端不变     |
| 任意 Broker 会话 | 另一次连接尝试                       | 不变             | 绝不同时允许第二个 Broker 会话 |

### 终端会话

| 目前状态                 | 输入                                           | 下一个状态                                | 所需效果                                                                                         |
| ------------------------ | ---------------------------------------------- | ----------------------------------------- | ------------------------------------------------------------------------------------------------ |
| 缺席                     | 使用显式几何体进行有效创建                     | `running`                                 | 准备容量，分配一条保留记录，在该几何上生成一个运行时，然后在回复之前自动承认其嵌入会话和两个索引 |
| 缺席                     | 使用延迟几何体进行有效创建                     | `awaiting_start`                          | 准备容量，分配一个保留的记录，保留完整的准备启动，并以原子方式承认其嵌入会话和两个索引而不产生   |
| 任意 retained state      | 携带空 overlay 的精确 launch replay            | 不变                                      | 返回 retained identity，不重建或延长原 overlay capability                                        |
| 任意 retained state      | 携带匹配非空 overlay 的精确 launch replay      | 不变                                      | 匹配经目标规则规范化的 overlay digest 后返回 retained identity                                   |
| 任意 retained state      | launch conflict 或非空 overlay-digest conflict | 不变                                      | 返回 typed creation conflict，不改变 identity、runtime 或 process environment                    |
| `awaiting_start`         | 从有效的几何形状开始；产卵成功                 | `running`                                 | 生成唯一的运行时，并在回复之前以原子方式将相同的初始几何体提交到生命周期和引擎观察               |
| `awaiting_start`         | 启动生成失败                                   | 不变                                      | 返回一个类型化的可重试失败；保留身份并准备启动，并且不暴露任何几何体、子项、PTY、附件或框架      |
| `running` 或更高版本     | 精确的重复起始几何形状                         | 不变                                      | 返回`already-started`，无其他进程影响；冲突的几何体返回不可重试的 `start-conflict`               |
| `awaiting_start`         | 有效的运行时主题比较和设置                     | 生命周期不变                              | 替换完整的拥有的准备主题，然后将其操作 ID 和下一个主题修订提交到相邻会话                         |
| `running` 或 `finishing` | 有效的运行时主题比较和设置                     | 生命周期不变                              | 在一个 Ghostty 突变边界下应用完整的引擎默认/当前颜色，然后无误地提交相同的完整语义主题状态       |
| 任何保留状态             | 准确回放当前主题操作和数据                     | 不变                                      | 返回`already-applied`;重播检查先于生命周期和过时检查，并且不执行物理或语义突变                   |
| 主题可变状态             | 陈旧的主题修订                                 | 不变                                      | 返回`stale`加上完整的当前主题状态；当前操作身份冲突或任何失败的预检也会保留两个所有者            |
| `running`                | 子进程退出或输出正常关闭                       | `finishing(first)`                        | 准确保留第一个完成事实并拒绝新的客户端输入                                                       |
| `finishing`              | 作者静止后补充完成的事实                       | `settled.exited`                          | 提交一个不可变的正常结果                                                                         |
| `running` 或 `finishing` | 不可恢复的运行时故障                           | `containing_failure.stopping`             | Fence I/O，记录故障，并发起一次强制围堵；尚未公布最终结果                                        |
| `containing_failure`     | 确认包含进程树和运行时资源                     | `settled.failed`                          | 保留类型化的故障和观察到的或显式不可用的过程结果                                                 |
| `containing_failure`     | 所有者清理阶段失败                             | `containing_failure.blocked`              | 保留确切的清理失败并安排独立于代理的重试                                                         |
| `settled`                | 致命的拆卸无法确认保留的清理                   | 相同的结果 + `retained_cleanup.blocked`   | 独立保留清理并安排独立于代理的重试；永远不要改写结果                                             |
| 沉淀+保留清理            | 清理重试成功                                   | 相同的结果 + `retained_cleanup.complete`  | 确认清理，同时保留已确定的结果和最终修订                                                         |
| 沉淀+保留清理            | 有界重试预算已用完                             | 相同的结果 + `retained_cleanup.exhausted` | 保留所有权和确切的清理失败信息；拒绝处置并且绝不报告干净排放                                     |
| `running` 或 `finishing` | 优雅或强制的请求                               | 具有更新意图的相同生命周期                | 每个意图最多执行一次；绝不隐式处置                                                               |
| `settled`                | 输入、调整大小、配置、主题或修订进度           | 不变                                      | 拒绝突变；捕获的最终修订版和终端拥有的可见状态保持不变                                           |
| `settled`                | 在结果发布等待期间处置                         | 不变                                      | 在运行时关闭或任何删除副作用之前可重试拒绝                                                       |
| `settled`                | 结果公布后处置                                 | 缺席                                      | 完成资源清理，删除会话引用和索引，然后销毁保留的记录                                             |
| `awaiting_start`         | 处置                                           | 缺席                                      | 删除会话引用和索引，释放准备好的启动，并销毁保留的记录而不终止                                   |
| 任意保留状态             | Broker 断开连接                                | 不变                                      | 只移除附件能力                                                                                   |

### 附件会话

附件准入通过Core和Engine准备传递一个规范的`Capabilities`值。引擎从 `input || resize` 获取实时运行时准入，从 `read` 获取保留可读准入，仅从 `input` 获取输入账本容量，仅从 `read` 获取历史状态。调整大小没有附件本地引擎分配。最终的引擎检查在保留终端互斥体的同时运行，因此核心验证后的完成或失败无法接受过时的输入或调整权限大小。

Core 保留该准入值作为不可变的附件重放标识，并将当前能力修订初始化为 `1`。协议 `0.0.23` 只允许输入位实时变化；读取和调整大小仍等于准入值。每个成功的附件或更新回复都会返回完整的当前能力与修订。

| 目前状态     | 输入                                                            | 下一个状态                    | 所需效果                                                                                   |
| ------------ | --------------------------------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------ |
| 缺席         | 具有至少一种能力的授权附加                                      | 投影 `paused`                 | 创建一个代理范围的能力绑定；调整租约需要调整大小功能                                       |
| 任何实时状态 | 精确的当前输入能力修订和值                                      | 相同状态                      | 返回完整的当前绑定作为幂等重播                                                             |
| 任何实时状态 | 确切的下一个修订版禁用输入                                      | 相同的投影状态                | 仅修复并删除该附件的输入账本，然后撤销核心输入权限                                         |
| 任何实时状态 | 确切的下一个修订版启用输入                                      | 相同的投影状态                | 准备一个新的空输入账本，然后暴露Core输入权限；不重放之前的输入                             |
| 任何实时状态 | 旧的、有间隙的、冲突的或未更改的下一个修订版输入更新            | 不变                          | 因类型化的不可重试的功能修订失败而拒绝                                                     |
| 任何实时状态 | 当前 resize owner claim 其具备 resize 能力的 attachment         | 相同的投影状态                | 返回完整 current capability binding，不发布 ownership event                                |
| 任何实时状态 | 另一个具备 resize 能力的 attachment claim ownership             | 相同的投影状态                | 原子 revoke 后 grant ownership，推进 successful-claim rank，并在 response 前发布两条 event |
| `paused`     | 激活需求                                                        | `ready.snapshot(base = null)` | 根据需要延迟获取终端渲染资源                                                               |
| `ready`      | 临时帧赢得了序列化读提交保护                                    | `in_flight`                   | 将其保留的传出记录、精确帧、基数和序列一起提交                                             |
| `in_flight`  | 确切的应用确认                                                  | `ready`                       | 仅推广公认的基础                                                                           |
| `ready`      | 捕获无法在其语义范围内表示                                      | `blocked`                     | 记录准确捕获或尝试的修订，该修订不能超出当前观察结果                                       |
| `blocked`    | 核心已经观察到比失败更新的修订版                                | `ready.snapshot`              | 惰性地使失败的捕获无效并重试，而不需要另一个终端突变                                       |
| 活跃状态     | 暂停                                                            | `paused`                      | 修复此附件的保留输入和焦点声明，然后放弃飞行并使其三角洲基地失效                           |
| 任何实时状态 | owner detach 选择这个 successful claim 最新的 eligible observer | 相同的投影状态                | 授予当前 resize ownership，并在 detach acknowledgement 前发布其作用域 event                |
| 任意状态     | 分离或 Broker 丢失                                              | 不存在                        | 将附件拥有的按键/鼠标/最终焦点修复入队，然后释放 pin、在途交付、选择、导出及权限           |

## 正确性不变量

测试、模型和审查结果使用稳定的声明标识符。

| 声明     | 不变式                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TH-001` | 每个保留的 `TerminalId` 都命名一个注册表成员，其稳定会话引用恰好属于一个 `RetainedTerminal`；该记录恰好包含一个 `TerminalSession`，并且在稳定状态下，恰好包含一个延迟启动准备或已启动的 Engine。任何 Engine 都不能存在于创建/启动事务或保留记录之外。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-002` | 每个可变语义事实都有一个权威所有者；派生状态不能比该所有者存在得更久，也不能与该所有者相矛盾。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `TH-003` | 每个 endpoint 最多存在一个 broker session，每项 attachment operation 均由该 physical session 及其 `AttachmentId` 授权。运行中的 terminal 最多有一个 current resize lease；显式 transfer 先 revoke 旧 owner 再 grant 新 owner，owner detach 晋升 successful claim 最新的 eligible observer，仅在从未获 grant 的 observer 间以最早 admission 打破平局，且不改变 immutable attach-replay identity。已接受的 held input 只属于该 attachment；detach 在 revoke authority 前只修复并移除其 ledger。                                                                                                                                                                                                                                                                                 |
| `TH-004` | 代理损失仅修复和破坏代理范围内的附件状态和功能；它永远不会交叉释放另一个附件或处置健康的保留终端。不可恢复的修复完整性故障使用显式运行时故障遏制路径。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-005` | 当 terminal 被保留时，`HostEpoch + CreationId + durable launch fingerprint` 创建是原子、幂等的；显式 disposal 释放该 identity。单独、经目标规则规范化且与顺序无关的 environment-overlay digest 属于该 retained creation generation。非空 replay 必须匹配它，空 replay 只对已经保留的 creation 充当 wildcard，且绝不重建 capability。Deferred creation 不公开 runtime geometry 或 process。Start 只提交一次精确提供的 initial geometry；精确 replay 幂等，冲突 replay 被拒绝，spawn failure 保留可启动 owner 且不产生 orphan process。                                                                                                                                                                                                                                         |
| `TH-006` | PTY 输出、输入编码、投影、历史、选择和调整大小同步使用一个规范 Ghostty 状态及一致的修订/代次对；失败的投影会精确命名其捕获或尝试的修订。调整大小后的观察绝不回退已接受修订；同一修订要求精确几何，并采用更新的规范观察。`projection_mutex` 独占关闭活动缓存及其活动 epoch/耗尽位。成功的原始输入或非焦点语义输入（包括零字节编码）会使缓存失效并推进 epoch；遭拒输入和仅焦点输入不会。后续成功采样可以根据当前平台进程与 Ghostty 提示状态恢复 idle。平台 active 始终发布 active，查询失败发布 unknown，活动 epoch 耗尽则永久保持 unknown。关闭活动绝不会成为生命周期或遏制权威。                                                                                                                                                                                              |
| `TH-007` | 接受的客户端输入正好占据一个 client-FIFO 位置，并原子地提交任何保留的输入清理所有权；被拒绝的输入都不占据。无按钮的语义 hover 只有在完全尚未开始且相对终端拥有的 mouse-input epoch 已过时时才可以退役；其他所有已接受客户端条目仍然有序，已经开始的条目会完整结束而不截断控制序列，所有幸存客户端条目保持相对 FIFO 顺序。保持输入预充电有限的分离修复能力。终端回复在有界回复突发内具有独立的有界准入和优先级，之后写入者屈服于等待接受的客户端条目；未知的客户端输入结果永远不会自动重播。                                                                                                                                                                                                                                                                                   |
| `TH-008` | 无论观察顺序如何，只有在子进程完成、正常输出关闭且写入器静止后才发布正常退出。结算会确定一个最终修订；此后，终端拥有的内容、代次、几何、配置和修订都无法再推进。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `TH-009` | 不可恢复的运行时故障是显式的，会围栏不安全 I/O，且不能报告为正常排空或正常退出。未确认的遏制不准入新的帧交接。清理仍由 Host 拥有，并可在无 Broker 时重试；已结算结果不能被后续清理症状改写，也不能在其精确发布提交前删除；处置绝不会静默遗弃未遏制的进程树。每个 POSIX 成员信号与存活决策都在权威成员关系观察前绑定到一项稳定内核标识。已匹配但失效的引用不能报告信号成功或空遏制，必须重新扫描。Linux 使用同一引用的信号 0 区分精确的未回收僵尸与已回收/可复用 pidfd；唯一例外是保留首领，其 Host 所有的延迟 wait 会阻止复用。复用的数字 PID 绝不能通过过时引用命中或统计替换进程；同会话替换进程需要新的稳定引用。                                                                                                                                                          |
| `TH-010` | 当且仅当精确记录位于已交接批次中时，传出交付转换才提交；批次已满会返回已提交前缀且不丢弃它。Engine 读提交守卫持有规范的 `projection -> terminal` 锁并校验代次、故障围栏 epoch、捕获的关闭活动 epoch 与耗尽位时，投影记录与在途提交均无分配。输入或采样失效会拒绝更早的临时帧且不推进其序列。`NeedSnapshot(S, activity-stale)` 只放弃精确在途帧 `S`，并强制生成更高序列的受守卫快照。已交接记录对其限定序列不可变，绝不以不同字节重建。                                                                                                                                                                                                                                                                                                                                        |
| `TH-011` | 每个附件最多有一个帧、行请求或每种类型的导出块正在运行；只有明确的范围内的确认才能推进它。发布的帧序列标识该主机纪元和附件的一个不可变记录。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `TH-012` | 边界与背压会拒绝或推迟当前操作，而不破坏保留的注册表、规范终端状态、交付基础或序列号。每个合格终端在每个单调 cadence 至多查询一次关闭活动，包括同修订及已有帧在途的状态；传出批次已满时保留强制快照重试，且不会重复该 cadence 的查询。活动 epoch 溢出会饱和为永久 unknown，不能创建 ABA 有效的 idle 帧。                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `TH-013` | 每项成功取得的资源（包括 POSIX 稳定成员引用）都有一个释放所有者，且至多释放一次；即使一次尝试失败，拆卸轮次也会尝试每个保留运行时，绝不把清理失败报告为干净关闭。FreeBSD 后代僵尸回收仅限于保留首领的 reaper 子树，不能消费该首领的 wait 结果或其他终端子树。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TH-014` | 所有控制 JSON 跨越一个本机协议适配器，所有二进制数据跨越其所属的编解码器，并且每个注册的角色/编码/成功类型/调度操作关联都来自协议注册表；格式错误、未知、不兼容、不匹配或未处理的记录无法到达域转换。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-015` | 每个 Ghostty 铃声只向前推进一个终端拥有的序列一次。具有读取能力的附件最多接收每个后续序列一次，仅以精确的传出批次附加方式前进，并且从不重播早于其接纳或在其销毁后幸存的序列。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TH-016` | 输入授权的应用程序清除或重置仅改变主机拥有的 Ghostty 状态，一次推进生成和修订，使附件之间的相关投影和交互状态一致无效，永远不会进入 PTY 输入/回复队列，并且在未知结果后永远不会自动重播。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `TH-017` | OSC 52 只能为 selection kind `c` 发布一个有界纯文本 write intent，绝不读取 platform clipboard data。Reader-batch rollback 不发布内容，outgoing-batch rollback 保留同一 request 以便重试，一次成功 handoff 会跨全部 attachment 消费它，reconnect 不重放。准确 private APC prefix 可以按 terminal sequence 顺序以事务方式把一份 decoded、有界 UTF-8 payload 追加到一个 64-entry FIFO；公开 notification channel、无关 APC prefix、malformed payload 与 reader rollback 都不发布，outgoing rollback 保留准确 queue head，append 只消费该 head。该 signal 与 terminal progress 都不会成为 Agent、provider、lifecycle 或 notification 权威。                                                                                                                                                |
| `TH-018` | 一种可接受的运行时主题操作会自动替换一个独立主机主题修订版中的配色方案、默认前景/背景、可为空的光标颜色以及所有 256 个调色板条目。重播、过时、冲突、验证、溢出、生命周期和运行时一致性故障保留了完整的先前核心和物理状态。 Recovery 发布完整的当前状态；所有附件都遵循一个一致的内容修订，并且 OSC/reset/RIS 根据最新提交的运行时主题进行解析。                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `TH-019` | 每个附件都有一个正单调能力修订版，初始化为 `1`。输入准备和最终队列提交需要其准确的当前修订版和启用的输入位。精确的能力更新重播是惰性的；陈旧的、有缺陷的、冲突的和不变的下一次修订不能改变权威。降级仅在撤销之前修复并删除目标附件的保留输入分类帐，保留已接受的 FIFO 条目而不重复，而升级则创建一个新的空分类帐。不可变的承认功能继续定义附加重放。                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `TH-020` | `TerminalEngine` 对已附加、具备输入能力且 Core 投影需求为活动状态的附件焦点声明做 OR 聚合。只有聚合从零变为非零或从非零变为零时，才发出终端 `FocusIn` 或 `FocusOut`。分离、Broker 丢失、需求暂停和能力降级只会在完成其按键/鼠标修复后清除目标声明；最终焦点修复会预留额度并最后排序。过时、禁用、暂停或已分离的输入无法创建声明；激活和能力升级也绝不重放焦点。                                                                                                                                                                                                                                                                                                                                                                                                               |
| `TH-021` | Host 只宣称拥有有界且可观察实现的终端能力。Glyph Protocol 被禁用，既不能回复也不能改变 glossary。XTGETTCAP 以一条有界有序回复回答固定的 `xterm-256color` 子集并使用精确十六进制编码；首个名称无效时返回否定回复，后续名称无效时终止成功前缀。它绝不安装 capability 数据，也不引入另一个 VT 所有者。                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `TH-022` | 模式 `20`、`40`、`1004`、`2026`、`2048` 以及 DEC 列模式 `3` 的外部 effect 都细化同一个规范 mode transition。模式 `20` 在 PTY 写入准入前计费 CR 到 CRLF 的展开。模式 `1004` 把即时聚合报告与已准备的焦点提交排序。模式 `2026` 只抑制 projection，并受显式 reset、完整 reset、成功 geometry change 或一秒单调 deadline 限制。模式 `3` 串行化 PTY 与 Ghostty 的 80/132 列 geometry，且不替换终端范围的 viewport target。每个显式 mode-`40` set、reset 或 saved restore 都恢复最近一次成功 resize-owner 请求的完整 geometry；目标已相等时不增加 geometry revision。模式 `2048` 在启用和每次成功 geometry commit 时报告精确 geometry。Resize 或必需回复失败进入各自的类型化 containment，且不会回滚已提交的一致 geometry；任何 effect 都不创建 attachment-local terminal state。   |
| `TH-023` | 固定版本的 Ghostty 是唯一 Kitty 图形状态所有者。每个被接受的媒体、帧、动画、放置、查询和删除转换都受有限的加载、解码存储、数量、线协议与历史边界约束。实时或渲染历史捕获命名一个精确的终端 revision 与图形 generation；每个放置都引用一项已包含的自有图像，具有范围内裁剪，不含未解析的 pin/路径/句柄，并且只属于一个有符号 z 绘制层。到期动画 tick 若改变显示像素，终端 revision 恰好推进一次；停滞 tick 不推进。实时 OBKG 与 render-detail OBRW 原子提交资源和放置；text-detail OBRW 不含图形。重置、删除、淘汰、屏幕变化、分离、重连、过时确认或准入失败都不能保留、部分发布或复活无效的客户端资源基线。                                                                                                                                                                   |
| `TH-024` | 冷 primary-screen 回滚压缩只在一次非阻塞终端锁尝试内观察 Ghostty 的不可解释活动 token 与终端拥有的交互 serial。每次成功准备非空客户端输入，以及每个已提交的 PTY reader batch 或可见 Kitty 动画 tick，都会在同一把锁下推进该 serial。Ghostty token 变化，或 waiting/pending 工作期间的交互 serial 变化，都必须经历至少 250 毫秒单调空闲时间，才允许一个有界增量步骤；仅有交互活动绝不会唤醒 complete 或 unsupported 工作。Host maintenance 每 20 毫秒至多提供一个公平运行时机会，且绝不突发补跑；锁竞争只停顿而不等待，也不推迟已有截止时间。Pending 工作只在后续公平轮次继续；完成或不支持的工作保持 dormant，直到出现与压缩相关的 Ghostty 活动。压缩只改变页面表示：revision、generation、geometry、逻辑行、projection、history、selection 与 delivery identity 都保持不变。 |

## 情景决策

| 场景                                     | 合同决定                                                                                         |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------ |
| 终端运行期间 Desktop 重启                | Host 保留终端；新 Broker 完成握手、读取清单并创建新附件。                                        |
| 同一用户进程知道端点                     | 位于当前信任边界内；不承诺应用程序级隔离。                                                       |
| 一个 Broker 活动期间第二个 Broker 连接   | 第二个 Broker 不能同时激活。                                                                     |
| 协议协商失败                             | 关闭该 Broker 的传输连接；保留终端继续存在。                                                     |
| 输入允许后响应丢失                       | 结果未知；桌面不得自动重播输入。                                                                 |
| 鼠标跟踪在 hover 准入后发生变化          | 只退役尚未开始且已过时的无按钮 hover；完整写完已开始的序列，并保留每一个有序输入条目。           |
| OSC 133 在子程序丢失后报告本地提示符     | 只有成功观察到无活动命令时才清除孤立鼠标跟踪；嵌套或未知的进程状态会保留它。                     |
| 主题响应已过时、丢失或重播               | 返回或恢复完整的权威主题状态；精确重放是幂等的，而未知结果必须在变基之前得到协调。               |
| 延迟终端启动无法生成                     | 将保留的身份和准备启动保留在`awaiting_start`中；不发布几何图形或启动事件，并允许显式重试或处置。 |
| 调度程序批量填充                         | 正常返回准备好的前缀并推迟下一条记录。                                                           |
| 客户端观察到重复的帧序列                 | 已提交的不可变帧从固定标头元数据中幂等地得到确认；它不会被再次解码或提交。                       |
| PTY 读取、写入、回复或调整大小完整性失败 | 记录类型化的故障、隔离 I/O、确认进程树遏制，然后发布并保留故障终端以供观察和处置。               |
| 主机关闭期间一个运行时失败               | 继续关闭所有其他运行时，然后报告聚合故障。                                                       |
| 主机或操作系统崩溃                       | PTY 不承诺连续性和恢复性。平台清理机制仍然适用于 OS 提供的地方。                                 |

## 所需的正确性证据

在声明映射到可执行证据之前，该实现不被认为是一致的。仅文件覆盖是不够的。

| 证据                                                                                   | 所需覆盖范围                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Host 注册表 TLA+ 模型](../formal/tla/TerminalHostRegistry.tla)                        | `TH-001`、`TH-004`、`TH-005`，双 terminal identity 唯一性、新鲜成功创建时内部无冲突的 terminal 选择、分离的 durable-launch 与 generation-overlay fingerprint、精确 non-empty overlay conflict、empty-overlay retained replay、atomic creation failure、broker-independent retention、settlement-gated disposal 与 disposal 后 identity reuse；检查边界位于相邻 `.cfg`                                                                                                                          |
| [终端生命周期 TLA+ 模型](../formal/tla/TerminalHostLifecycle.tla)                      | `TH-005`、`TH-008`、`TH-009`、精确挂载几何的启动、启动失败停滞、写入器静止、两种正常完成顺序、失败遏制、清理重试、几何与有界单调 canonical revision 分离、介入内容变更后的 resize target 合并、精确结算的 geometry/revision、变更冻结，以及从等待启动或结算状态处置；检查边界位于相邻 `.cfg` 中                                                                                                                                                                                                |
| [Host 关闭 TLA+ 模型](../formal/tla/TerminalHostShutdown.tla)                          | `TH-013`，一次双运行时拆卸轮次、一次失败后不短路、失败结果保留全部所有权、干净结果释放全部所有权，以及仅在尝试每个保留运行时后才完成该轮次；检查边界位于相邻 `.cfg`                                                                                                                                                                                                                                                                                                                            |
| [交付 TLA+ 模型](../formal/tla/TerminalHostDelivery.tla)                               | `TH-003`、`TH-010`、`TH-011`、当前物理 connection 与 `AttachmentId` 授权后的转换、作为有界交付 scope 而非 wire epoch 的 `brokerActive`、批次前缀、确认竞态、暂停/分离、重连和传输失败；协调器在进入此模型边界前拒绝先前 connection 的请求，检查边界位于相邻 `.cfg`                                                                                                                                                                                                                             |
| [请求结果TLA+模型](../formal/tla/TerminalHostRequestOutcome.tla)                       | `TH-005`、`TH-007`、`TH-010`、`TH-016`、准备/提交/切换、可重试拒绝、已送达答复、未知结果、幂等重播、禁止的不可重播重试，以及将应用程序清除/重置绑定到不可重播类别的受检映射                                                                                                                                                                                                                                                                                                                    |
| [运行时主题TLA+模型](../../../packages/workbench/formal/tla/TerminalThemeRevision.tla) | `TH-018`，两次操作主机比较和设置，原子默认值/调色板/光标发布，精确重播，冲突，陈旧和失败的无突变路径，未知结果协调以及后来的客户端呈现障碍                                                                                                                                                                                                                                                                                                                                                     |
| [清单 TLA+ 模型](../formal/tla/TerminalInventoryPagination.tla)                        | `TH-002`、`TH-012`、具有显式单成员页边界的代次范围分页、跨多页精确成员关系、防止重复和过时游标拒绝；按 payload 大小截短的多成员打包及按实际发出数量推进游标仍由 `connection_request_test.zig` 负责                                                                                                                                                                                                                                                                                             |
| [帧交付 TLA+ 模型](../formal/tla/TerminalFrameBackpressure.tla)                        | `TH-006`、`TH-009`、`TH-010`、`TH-011`、`TH-012`、`TH-016`、双附件投影变更/捕获分离、改变代次的清除/重置、精确失败 revision、临时帧提交、失败发布和遏制确认、禁止未确认失败交接、每个附件一个 flight、按不可变 sequence/revision/generation 分类原始 `FrameApplied`、按 sequence 分类原始 `NeedSnapshot`、精确旧值重放及已放弃/错误/未来输入保留更新的 flight、确认基线、暂停/放弃和受阻快照恢复；不假定客户端确认到达具有公平性                                                               |
| [关闭活动新鲜度 TLA+ 模型](../formal/tla/TerminalCloseActivityFreshness.tla)           | `TH-006`、`TH-010`、`TH-012`，Host 端对含字节及零字节输入的立即失效、当前 Ghostty 提示状态重新分类、平台 active 与失败优先级、静默的同修订进程或提示变化、包括 active-to-idle 再进入的两个有界 cadence 轮次、临时 epoch/耗尽守卫、批次已满回滚、精确的本地 `FrameApplied` sequence/revision 与 `NeedSnapshot` sequence、独立的精确输入或 flight 保留检查、当前 token 与缓存一致，以及为已失效在途帧强制后继快照；消费者本地呈现与关闭决策不属于此 Host 模型，由终端客户端和 Workbench 测试验证 |
| [输入背压 TLA+ 模型](../formal/tla/TerminalInputBackpressure.tla)                      | `TH-003`、`TH-004`、`TH-007`、`TH-012`、`TH-019`、`TH-020`、两个附件的持有输入所有权与焦点聚合、可见性丢失/恢复时不重放、单调输入能力降级/升级/重放/拒绝、过时输入拒绝、排队输入保留、无交叉发布的预留额度修复、作为代表性标量的独立客户端/回复准入预算、有界双位置客户端 FIFO 监视器、写入器关闭清理、精确 `MaxReplyBurst` 耗尽让步、回复优先级及有界客户端推进；具体 byte-plus-entry 容量由队列与所有权测试负责                                                                              |
| [鼠标输入边界 TLA+ 模型](../formal/tla/TerminalMouseInputBoundary.tla)                 | `TH-007`、已准备鼠标 epoch、过时准备拒绝、可表达 active-to-active 模式变化的双组件派生鼠标活动状态、模式/重置/本地提示符推进、活动与未知提示符保留、按解析器顺序重新启用、尚未开始的过时 hover 退役、已开始条目完成、幸存 FIFO 顺序、有界写入器推进，以及在已有准备、排队或已开始工作时由因果推进触发的 epoch 耗尽故障关闭                                                                                                                                                                     |
| [终端模式效果 TLA+ 模型](../formal/tla/TerminalModeEffects.tla)                        | `TH-006`、`TH-007`、`TH-020`、`TH-022`、一个已配置投影事务内的同步投影抑制及有界释放、模式 20 准入计费、即时焦点报告与已准备焦点的排序、viewport 与 canonical geometry、模式 3 DECCOLM、模式 40 set/reset 恢复、模式 2048 启用/geometry 报告、reset 与 resize 交互，以及 resize/reply 失败遏制；重复 synchronized-output 再进入仍由确定性运行时测试负责                                                                                                                                        |
| [Kitty 图形投影 TLA+ 模型](../formal/tla/TerminalGraphicsProjection.tla)               | `TH-006`、`TH-010`、`TH-011`、`TH-012`、`TH-016`、`TH-023`、规范图形变更、到期/停滞动画 tick、精确图形 generation 与终端 revision、已解析层/资源一致、原子实时与渲染历史捕获、文本历史排除、失效、重试，以及 parser 测试和维护的图形语料分类具体 byte/dimension/decoded-size 及 placement/frame-count 限制后的原子有界拒绝；`Acknowledge` 是帧交付模型已验证的精确结果，本模型配置的 reader 是逐点抽象，跨 reader 失效/flight 交错仍由共享双 reader 帧模型负责                                 |
| [回滚压缩 TLA+ 模型](../formal/tla/TerminalScrollbackCompression.tla)                  | `TH-024`、分别观察 Ghostty 压缩活动与“自上次观察后发生变化”的终端交互活动、非阻塞锁竞争、逻辑空闲重启、dormant 交互吸收、每个抽象维护机会一个有界增量步骤、pending 继续、轮询推进，以及不变的 revision/generation；精确 250 毫秒空闲截止时间、20 毫秒 cadence 与无突发延迟轮询由确定性源码测试负责，回绕 serial refinement 假定两次成功观察之间少于 `2^64` 次交互                                                                                                                              |
| [进程标识 TLA+ 模型](../formal/tla/TerminalHostProcessIdentity.tla)                    | `TH-008`、`TH-009`、`TH-013` 的有限双场景证明族：PID 竞态场景跨数值 PID 复用获取稳定成员引用，且不能由单个候选确认整个会话已遏制；双成员扫描保留首个错误、继续访问所有候选并释放每个已获取引用，仅在完整且无错误的 count 未发现存活成员后确认遏制；后续 SID 重绑定继续受围栏保护，检查边界位于相邻 `.cfg`                                                                                                                                                                                      |
| [历史交互 TLA+ 模型](../formal/tla/TerminalHistoryInteraction.tla)                     | `TH-002`、`TH-006`、`TH-010`、`TH-011`、`TH-012`、`TH-016`、具有精确 sequence/revision/generation 确认标识的行 flight、pending 与 committed anchor 标识、代次变化时 clear/reset 失效、历史观察、选择淘汰、精确的导出/chunk 确认标识、单调 search request sequence 加 revision/generation、过时行/搜索输入保留及 detach 清理；不假定消费者确认到达具有公平性                                                                                                                                    |
| 组合不变式校验器                                                                       | `TH-001` 至 `TH-013` 及 `TH-016` 至 `TH-024` 跨越所有者边界：保留运行时成员关系、Broker 标识、生命周期与遏制、配置、终端主题、几何、能力修订、输入账本/焦点资格协议、图形 generation/基线协议、仅表示变化的压缩协议，以及每个稳定 Service 步骤后的有界投影观察                                                                                                                                                                                                                                 |
| 有状态属性测试                                                                         | `TH-001` 到 `TH-024` 适用于每个所有者：注册表/索引一致性、生命周期、终端控制和运行时主题命令序列、附件能力修订和焦点声明序列、图形放置/动画序列、压缩调度、投影序列、历史/导出序列以及每个操作后的不变检查                                                                                                                                                                                                                                                                                     |
| 确定性故障注入测试                                                                     | 每个提交接缝上的分配、编码、立即创建与延迟启动 spawn、队列准入、运行时主题预检、调整大小、PTY I/O、进程观察、保守失败的成员关系错误、在优雅 HUP、强制 STOP/KILL/CONT 及存活判断前立即按脚本复用 PID、回复交接、部分批次及多运行时关闭失败                                                                                                                                                                                                                                                      |
| 本机集成测试                                                                           | 稳定 POSIX 成员标识与会话/reaper 子树进程行为、Windows Job/ConPTY 行为、端点所有权、对等身份、描述符/句柄清理、Worker join，以及每个原生声明平台上的真实可执行文件/本地 IPC/PTY 跟踪                                                                                                                                                                                                                                                                                                           |
| 跨语言协议一致性                                                                       | 每种角色、编码、成功响应；每个本机命令、回复、事件、标识符类型以及声明的联合或错误变体；每种传输和二进制格式；每个谈判或硬性约束的双方；以及每个声明的格式错误记录拒绝类别                                                                                                                                                                                                                                                                                                                     |
| 针对性的性能证据                                                                       | 有界内存、无需求行为、公平性、大输入背压、投影节奏和历史/导出边界，且不会削弱语义不变量                                                                                                                                                                                                                                                                                                                                                                                                        |

每个 TLA+ 模块标头和有状态属性测试套件都必须识别其改进的转换词汇及其涵盖的稳定声明 IDs。跟踪操作名称必须使用本文档中的转换词汇。改变状态、所有者、提交边界、重试类或失败结果的生产变更必须在同一变更中更新此模型及其证据。

### 本地证据索引

以下条目是所需的可执行细化，而不是声明未执行的测试当前通过。仅当包验证命令成功运行这些条目后，切换才可以报告一致性。

以下稳定兼容性见证是现有 Host 不变量的别名，不是并行规范。常规 Host `test` 门禁执行具名原生套件和跨语言语料；单独选择的 `test:tla` 门禁执行链接模型。这些见证控制 Host 适配器、故障注入和确定性调度，但除下文具名原生集成外，不声明应用 UI 或操作系统效果。

| 稳定见证                                                                  | Host 不变量与可执行细化                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal-parity/PR-10-replies`                                           | `TH-006`、`TH-007` 与 `TH-021`；[`../src/engine/__tests__/terminal_compatibility_test.zig`](../src/engine/__tests__/terminal_compatibility_test.zig)、[`../src/engine/__tests__/terminal_stream_test.zig`](../src/engine/__tests__/terminal_stream_test.zig)、[`../src/engine/__tests__/runtime_input_test.zig`](../src/engine/__tests__/runtime_input_test.zig)、协议一致性及 real-PTY 回复路由执行有界 pinned-Ghostty 查询语料与有序准入。                                                    |
| `terminal-parity/PR-12-deccolm`                                           | `TH-006` 与 `TH-022`；[`../src/engine/__tests__/runtime_mode_effects_test.zig`](../src/engine/__tests__/runtime_mode_effects_test.zig)、投影/Core 同步测试及 [`../formal/tla/TerminalModeEffects.tla`](../formal/tla/TerminalModeEffects.tla) 执行模式 3 几何、模式 40 viewport 恢复、解析器顺序、串行化与遏制。                                                                                                                                                                                |
| `terminal-parity/PR-13-clear-reset`                                       | `TH-016`；[`../src/engine/__tests__/runtime_state_test.zig`](../src/engine/__tests__/runtime_state_test.zig)、[`../src/service/__tests__/runtime_aggregate_lifecycle_test.zig`](../src/service/__tests__/runtime_aggregate_lifecycle_test.zig)、协议一致性及请求结果/帧交付模型执行授权、一次规范突变、完整快照失效和未知结果不重放。                                                                                                                                                           |
| `terminal-parity/IN-15-detach-repair`、`terminal-parity/IN-16-input-fifo` | `TH-003`、`TH-004` 与 `TH-007`；[`../src/engine/__tests__/input_ownership_test.zig`](../src/engine/__tests__/input_ownership_test.zig)、[`../src/engine/__tests__/input_queue_test.zig`](../src/engine/__tests__/input_queue_test.zig)、[`../src/engine/__tests__/runtime_input_test.zig`](../src/engine/__tests__/runtime_input_test.zig)、Service 输入测试及 [`../formal/tla/TerminalInputBackpressure.tla`](../formal/tla/TerminalInputBackpressure.tla) 执行仅附件修复和有界终端全局 FIFO。 |

`src/service/__tests__/composition_observation_test.zig` 覆盖原生运行时与显式的仅模型组合；共享的检查请求与轮询辅助工具则在整个服务集成测试套件中应用同一校验器。Core 测试辅助工具委托给组合检查使用的同一所有者本地校验器，因此两个证据层无法静默定义不同的 Core 不变式。

`src/service/__tests__/composition_refinement_test.zig` 针对本机运行时执行稳定的语义词汇，包括已应用、重播、拒绝和结果未知/协调路径。相邻的 `composition_reference*.zig` 文件将状态形状、转换缩减、观察比较和编排保留为测试目录内的单独审查单元。

`src/service/__tests__/composition_state_machine_test.zig` 生成确定性多所有者 HostService 命令序列，重播新服务中的每个候选者，并在每个稳定操作或原子操作批次后检查细化。通过范围删除和命令简化对故障进行增量调试，然后将其作为 `ORCHBEE_TERMINAL_HOST_PROPERTY_REPLAY` 的精确值发出；还可以使用 `ORCHBEE_TERMINAL_HOST_PROPERTY_SEED` 再现单独生成的流。

`src/__tests__/host_black_box_test.zig` 关闭剩余的进程中假设。构建将实际的 `orchbee-terminal-host` 工件传递给它；运行程序创建一个专用端点，将主机作为子进程启动，并仅使用真实的本地 IPC 记录。其确切的观察顺序是：进程启动、端点连接和 Hello 接受、终端创建/启动和连接、物理断开、在同一主机纪元中重新连接、具有相同身份的幂等创建重放和重新连接、PTY 输入接受、成功的子出口（其最终版本证明 PTY 输出已进入规范状态）、显式处置和空闲主机进程退出。看门狗限制主机关闭，而记录和端点截止时间限制每个外部等待。

常规包 `test` 目标会运行该跟踪。因此，仓库 CI 提供 Linux、macOS 和 Windows 的原生执行证据；`test:cross` 会为 FreeBSD 编译同一运行器，但不会把该编译结果视为 PTY、IPC 或 ABI 运行时证据。

命名的应用程序系统改进关闭了受影响的跨所有者边界，而不替换下面的本地证据。 `terminal-parity/SU-13-cursor-defaults` 观察主机拥有的显式块/非闪烁配置文件、DECSCUSR 覆盖以及 real-PTY 投影元数据中的 RIS 恢复；渲染器几何形状仍然是所有者本地证据。 POSIX-only `terminal-parity/SU-16-startup-geometry` 在其 shell 启动之前记录子进程的第一个 `stty size`，并要求它等于独立测量的安装网格和唯一的第一个主机/客户端框架；它不声明 Windows ConPTY 或更高版本的调整大小。受控的 Canvas `terminal-parity/SU-09-theme`、`terminal-parity/PR-11-bell`、`terminal-parity/IN-10-read-only` 和 `terminal-parity/IN-14-shared-focus` 见证人通过应用程序边界完善了主机主题修订、铃声序列、功能修订和聚合焦点合约，同时捕获而不是声明本机主题、声音、通知或多窗口焦点效果。 `terminal-parity/HS-07-selection-export` 添加真实的 PTY-to-Electron 普通剪贴板结果；生成的 TypeScript/Zig 一致性仍然是在运行时访问之前拒绝 HTML 和原始 VT 格式的权威。

| 声明               | 所需原生证据                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TH-001`, `TH-002` | `src/core/__tests__/registry_broker_test.zig`、`lifecycle_observation_test.zig` 和 `projection_state_test.zig`：注册表/索引一致性和所有三个有状态转换套件； `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`：立即/延迟所有权、创建/启动失败和重播身份                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TH-003`, `TH-004` | `src/engine/__tests__/input_ownership_test.zig` 与 `runtime_input_test.zig`：held-input ownership、late prepared-input rejection、precharged repair 及 repair failure containment；`src/core/__tests__/registry_broker_test.zig`：单 broker attachment ownership、显式 resize transfer、幂等、denial、rank overflow 与 recency promotion；`src/service/__tests__/connection_request_test.zig`：revoke-before-grant event ordering、replay、broker replacement、disconnect 与 detach acknowledgement；`src/transport/__tests__/local_endpoint_test.zig`：failed-handoff preservation                                                                                                                                                                                                                                                                                                                                                                |
| `TH-005`           | `src/model/__tests__/launch_test.zig`：全数值或全空 launch geometry、durable launch digest identity、target-normalized overlay validation 与 order-independent overlay digest；`src/service/__tests__/runtime_aggregate_lifecycle_test.zig`：deferred ownership、failed-start retry、精确 initial geometry、empty-overlay retained replay、匹配与冲突 non-empty overlay replay、creation replay 与 immediate creation failure；`src/service/__tests__/terminal_integration_test.zig`：real PTY disconnect/replay 与隔离 immediate spawn failure；`src/engine/__tests__/runtime_launch_test.zig`、`runtime_startup_test.zig` 与 `runtime_state_test.zig`：canonical launch geometry、overlay application、one-shot startup 与 partial-start cleanup                                                                                                                                                                                                 |
| `TH-006`           | `src/platform/__tests__/process_test.zig`、`src/engine/__tests__/runtime_state_test.zig` 和 `runtime_input_test.zig`：活动命令分类、规范解析、终端回复、OSC 颜色、投影元数据及终端驱动的调整大小排序；`src/engine/__tests__/history_rows_test.zig`、`history_selection_test.zig` 和 `history_export_test.zig`：行、reflow、选择和导出；`src/core/__tests__/lifecycle_observation_test.zig` 与 `projection_state_test.zig`：观察和交付规则；`src/service/__tests__/projection_test.zig`：立即输入失效、物理交付前后的新鲜不活动恢复、active 到 idle 转换、含字节及零字节输入、当前 OSC 133 提示/命令重新分类、平台 active 优先级，以及查询失败；`runtime_aggregate_lifecycle_test.zig`：串行化投影竞态与完整几何同步；`runtime_aggregate_attachment_test.zig`：排除遭拒/焦点输入、已接受原始/非焦点输入失效、调整大小提交与 Host 同步之间确定推进运行时修订，以及单调采用；仍要求 Engine stream/投影/选择单元测试与配对的 pinned-Ghostty 表面语料库 |
| `TH-007`           | `src/engine/__tests__/input_queue_test.zig`、`input_ownership_test.zig`、`runtime_input_test.zig`：精确 byte 与 entry 容量、有界入场、优先、FIFO、结算、脱离修复； `src/service/__tests__/runtime_aggregate_attachment_test.zig`：准备回滚； `src/service/__tests__/input_request_test.zig`：原始/语义失败等效和仅队列准入确认                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `TH-008`, `TH-009` | `src/model/__tests__/lifecycle_test.zig`；`src/core/__tests__/lifecycle_observation_test.zig` 和 `registry_broker_test.zig`：生命周期、不可变结果与发布门控删除；`src/platform/__tests__/process_test.zig`：保留首领/SID 围栏、macOS stop 与 exit 观察区分、注册前已可等待的退出观察、保守失败的成员关系错误，以及在优雅 HUP、强制 STOP/KILL/CONT 和通过生产稳定成员算法统计存活前立即确定性复用成员 PID；Engine 完成/遏制测试以及 `runtime_state_test.zig` 和 `runtime_failure_test.zig`：状态冻结与不依赖 Broker 的遏制；`src/service/__tests__/event_delivery_test.zig`、`lifecycle_request_test.zig`、`runtime_aggregate_lifecycle_test.zig` 和 `runtime_aggregate_cleanup_test.zig`：交接、处置、同步、精确清理阶段和公平重试；`src/transport/__tests__/transport_maintenance_test.zig`：无 Broker 维护                                                                                                                                       |
| `TH-010`, `TH-011` | `src/service/__tests__/outgoing_test.zig`、`projection_test.zig`、`interaction_transaction_test.zig` 和 `event_delivery_test.zig`：前缀、预留、回滚、串行化提交、调度器公平性、活动 epoch 与耗尽位导致的临时帧拒绝、同修订强制快照、精确的 `activity-stale` 在途帧放弃，以及 S+1 恢复；`src/engine/__tests__/selection_export_test.zig`、`history_rows_test.zig` 和 `history_export_test.zig`：行/导出标识、回滚与 ABA 拒绝                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-012`           | 协议限制测试；`src/engine/__tests__/input_queue_test.zig`、`runtime_input_test.zig`、`runtime_state_test.zig`、`projection_test.zig` 和 `history_rows_test.zig`；`src/service/__tests__/projection_test.zig`：终端修订不变时每 cadence 至多一次采样、在途与批次已满重试、查询失败及活动 epoch 耗尽；`src/service/__tests__/connection_request_test.zig`：payload 截短的清单页、按实际发出数量精确推进游标及回复预留失败；`src/__tests__/protocol_conformance_test.zig`：完整 TypeScript/原生边界与 `activity-stale` 原因相等                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `TH-013`           | Engine 启动/遏制单元测试以及 `runtime_state_test.zig` 和 `runtime_failure_test.zig`：所有权发布、部分启动与受遏制销毁；平台进程测试：exec 失败、POSIX 稳定引用释放、确定性 PID 复用遏制及 Windows 遏制；`src/service/__tests__/runtime_aggregate_cleanup_test.zig`：关闭所有运行时与保留清理；`src/transport/__tests__/local_endpoint_test.zig`：端点和连接清理                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TH-014`           | 控制解码/连线测试、协议注册表测试和`src/__tests__/protocol_conformance_test.zig`； `src/service/__tests__/lifecycle_request_test.zig`：格式错误、过时的时代、未知的可选以及调度前的角色/编码拒绝                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `TH-015`           | `src/engine/__tests__/runtime_state_test.zig`：Ghostty BEL 效果计数；`src/service/__tests__/interaction_delivery_test.zig`：每个附件的基线、有序交付、批次已满重试和销毁行为；Desktop 与 Web 的投影终端测试：精确的限定范围转发和客户端去重                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-016`           | `src/engine/__tests__/runtime_state_test.zig`：应用清除/重置语义和反穷举拒绝； `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`：输入能力授权和精确的核心观测同步； `src/__tests__/protocol_conformance_test.zig`：控制操作和确切接受的回复                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `TH-017`           | `src/engine/__tests__/runtime_state_test.zig`：OSC 52 read/selection/base64/UTF-8/control rejection、准确 16 KiB boundary、first-pending ownership、retry identity、准确 private-APC admission、公开 OSC/unrelated-prefix rejection、same-batch signal FIFO order 与 bound，以及 progress set/remove projection；`src/service/__tests__/event_delivery_test.zig`：真实 reader-batch private-APC application signal 经 control-event commit 至 consumption；`src/service/__tests__/interaction_delivery_test.zig`：clipboard outgoing-full rollback、one-recipient routing、consumption 与 reconnect non-replay；protocol 与 Desktop test 覆盖 bounded signal event、strict v3 launch/correlation interpretation、clipboard event 与准确 automatic application decision                                                                                                                                                                             |
| `TH-018`           | `src/model/__tests__/theme_test.zig` 和 `src/core/__tests__/theme_revision_test.zig`：完整的状态有效性、比较设置、精确重播、陈旧/冲突、溢出和无突变拒绝； `src/engine/__tests__/runtime_theme_test.zig`：原子物理替换、OSC覆盖/重置、RIS/应用重置优先级、失败保存、两附件修订协议； `src/service/__tests__/runtime_aggregate_theme_test.zig` 和 `connection_request_test.zig`：延迟/运行跨所有者提交、回复预分配、恢复库存、重放/过时/冲突/失败结果； `src/__tests__/protocol_conformance_test.zig`：确切的 TypeScript/本机主题请求、结果、状态和注册表协议                                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-019`           | `src/core/__tests__/registry_broker_test.zig`：修订初始化、精确更新/重播、过时/间隙/冲突拒绝、不可变附加重播和不变量； `src/service/__tests__/input_request_test.zig`：两个附件持有所有权、仅目标降级修复、准备输入最终重新授权、排队输入保存、禁用/陈旧线路拒绝、新升级分类帐和当前修订接受；组合状态机跟踪和协议一致性涵盖修订可见性和精确的跨语言字节                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `TH-020`           | `src/engine/__tests__/runtime_input_test.zig`：两个附件的聚合焦点转换、重复声明、保留账本的可见性 reset、有序按键/鼠标/焦点修复、预留额度最终发布、过时已准备输入拒绝及修复失败遏制；`src/service/__tests__/input_request_test.zig` 加组合跟踪：过时/暂停拒绝、需求丢失修复、分离与降级排序、激活/升级不重放及不变式协议；`src/service/__tests__/terminal_integration_test.zig`：确定性的 real-PTY 双附件焦点跟踪；有界面 `terminal-parity/IN-14-shared-focus`：两个生产投影表面及应用命令跟踪                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `TH-021`           | `src/engine/__tests__/terminal_compatibility_test.zig`：禁用 Glyph support/registration 且无回复或 glossary 变更；依据固定兼容语料验证有界、分片、多名称、不支持、畸形及有序的 XTGETTCAP/DECRQSS 行为                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `TH-022`           | `src/engine/__tests__/runtime_input_test.zig`、`runtime_state_test.zig`、`runtime_mode_effects_test.zig` 与 `projection_test.zig`：模式 20 展开及展开后限制、模式 1004 即时聚合报告及过期准备、模式 2026 跨批抑制/reset/resize/timeout、launch 与 same-canonical viewport 替换、模式 3 DECCOLM 后的模式 40 set/reset/saved 恢复、parser-order 收敛、串行化 PTY failure、模式 2048 精确报告、回复预算遏制，以及完整 grid/cell-pixel geometry；`src/service/__tests__/runtime_aggregate_lifecycle_test.zig`：Core 采用较新的 terminal-driven viewport restore，且不覆盖该结果                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-023`           | `src/engine/__tests__/projection_test.zig` 与 `history_rows_test.zig`：固定 Ghostty 的 direct/file/temporary/shared-memory、raw/PNG/zlib/chunk、virtual/relative/crop/layer、animation generation、实时/历史捕获、删除/reset/淘汰、边界及来源独立差分用例；`src/service/__tests__/projection_test.zig`：单调到期唤醒、每次变化 tick 一个 revision、停滞、在途失效、重试与双附件协议；协议一致性加 `src/protocol/__tests__/graphics_extension_test.zig`：精确有符号 z OBKG 与 render-detail OBRW 字节；terminal store 与两个 renderer 后端：原子缓存替换与三个绘制接缝；[`../formal/tla/TerminalGraphicsProjection.tla`](../formal/tla/TerminalGraphicsProjection.tla)：转换/refinement 检查                                                                                                                                                                                                                                                        |
| `TH-024`           | `src/engine/__tests__/runtime_state_test.zig`：非阻塞锁竞争、精确空闲截止时间、每步至多一页、pending 到 complete 收敛、冷页面 backing 节省、同一行输出与已准备客户端输入重启空闲期、dormant 交互吸收，以及 revision/generation 不变；`src/service/__tests__/runtime_aggregate_maintenance_test.zig`：全局 cadence、轮询公平与无突发补跑；[`../formal/tla/TerminalScrollbackCompression.tla`](../formal/tla/TerminalScrollbackCompression.tla)：调度与仅表示变化 refinement 检查                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |

确定性的提交接缝集是：创建准备/生成/准入；延迟启动生成；控制-回复保留；具有最终能力重新授权的原始和语义队列预留/发布；附件输入账本准入、焦点聚合、可见性修复、能力降级/升级以及分离修复物化/发布/故障遏制；应用程序终端控制验证和规范突变；调整大小效果和不安全回滚；模式效果回复准入、焦点模式重新校验与同步输出截止释放；图形媒体获取/清理、动画推进、放置解析、实时/历史临时捕获及原子资源-放置提交；投影捕获/编码、临时有效负载采用和序列化读取提交；行、历史、选择和导出临时突变；批量前缀/完整行为；生命周期结果交接；本地IPC部分写入；过程观察和遏制；多运行时关闭；鼠标输入 epoch 重新校验与尚未开始的 hover 退役；以及语义提交之前的运行时主题物理应用程序。必须将新的易错接缝添加到此集合中，并在该接缝之后的实现可能依赖于 `catch unreachable` 之前获得一项具名（named）故障测试。

### 提交接缝故障矩阵

每个稳定的接缝都有一个所需的故障处理和一个调度类。 `rollback` 恢复转换前的观察，`reject_without_commit` 在突变前保留或验证，`preserve_for_retry` 为以后的切换保留拥有的值，`publish_prefix` 仅公开完全提交的前缀，`contain_and_retain` 隔离不安全工作同时保留生命周期所有权。分配索引计划使选定的分配失败，确定性交错计划承认显式命名的事件顺序，本机边界计划使用失败的平台或传输适配器，单所有者计划执行有序的本地事务。

| ID       | 接缝稳定                               | 所需的故障处理 | 调度类     |
| -------- | -------------------------------------- | -------------- | ---------- |
| `CS-001` | `CreationPreparation`                  | 回滚           | 分配指数   |
| `CS-002` | `CreationSpawn`                        | 包含并保留     | 原生边界   |
| `CS-003` | `CreationAdmission`                    | 回滚           | 分配指数   |
| `CS-004` | `ControlReplyReservation`              | 拒绝而不提交   | 分配指数   |
| `CS-005` | `RawInputReservation`                  | 拒绝而不提交   | 单一业主   |
| `CS-006` | `SemanticInputReservation`             | 回滚           | 分配指数   |
| `CS-007` | `InputPublication`                     | 回滚           | 确定性交织 |
| `CS-008` | `AttachmentRuntimePreparation`         | 回滚           | 单一业主   |
| `CS-009` | `AttachmentCoreCommit`                 | 回滚           | 分配指数   |
| `CS-010` | `AttachmentInputRepairMaterialization` | 包含并保留     | 分配指数   |
| `CS-011` | `AttachmentInputRepairPublication`     | 保留以供重试   | 确定性交织 |
| `CS-012` | `TerminalControlMutation`              | 拒绝而不提交   | 单一业主   |
| `CS-013` | `ResizeEffect`                         | 包含并保留     | 原生边界   |
| `CS-014` | `ResizeRollback`                       | 包含并保留     | 确定性交织 |
| `CS-015` | `ProjectionCaptureEncoding`            | 保留以供重试   | 分配指数   |
| `CS-016` | `ProjectionPayloadAdoption`            | 回滚           | 分配指数   |
| `CS-017` | `ProjectionReadCommit`                 | 保留以供重试   | 确定性交织 |
| `CS-018` | `InteractionProvisionalMutation`       | 回滚           | 分配指数   |
| `CS-019` | `BatchPrefixFull`                      | 发布前缀       | 单一业主   |
| `CS-020` | `LifecycleOutcomeHandoff`              | 保留以供重试   | 单一业主   |
| `CS-021` | `LocalIpcPartialWrite`                 | 保留以供重试   | 原生边界   |
| `CS-022` | `ProcessObservationContainment`        | 包含并保留     | 原生边界   |
| `CS-023` | `MultiRuntimeShutdown`                 | 包含并保留     | 确定性交织 |
| `CS-024` | `DeferredStartSpawn`                   | 包含并保留     | 原生边界   |
| `CS-025` | `RuntimeThemeCommit`                   | 拒绝而不提交   | 单一业主   |
| `CS-026` | `ModeEffectReplyAdmission`             | 包含并保留     | 单一业主   |
| `CS-027` | `FocusModeRevalidation`                | 拒绝而不提交   | 确定性交织 |
| `CS-028` | `SynchronizedOutputDeadlineRelease`    | 保留以供重试   | 确定性交织 |
| `CS-029` | `MouseInputEpochRetirement`            | 拒绝而不提交   | 确定性交织 |

`src/service/__tests__/commit_seam_matrix_test.zig` 是可执行目录。它需要上面每一行都有一个一对一的枚举/表条目，并找到提供其失败证据的确切命名测试。共享脚本调度程序仅通过声明的事件序列前进；并发证据必须使用门或调度程序，并且永远不要将计时睡眠作为排序证明。

## 审核规则

审查遵循语义边界而不是文件顺序。对于每次转换，审核者都会验证：

1. 权威所有者与合法源状态；
2. 生效前的验证和预留容量；
3. 外部影响及其补偿或遏制路径；
4. 一项无误的领域提交；
5. 回复/事件切换和未知结果行为；
6. 每个故障边缘的资源清理；和
7. 每项受影响声明的证据 ID。

即使一个语义所有者必须保持完整，文件大小也是一个强制性的审查信号。当所有者不再适合一个可独立审查的文件时，将其提升到一个目录，并将其状态声明、公共外观和事务实现划分为指定的审查单元。保留一位权威所有者并明确跨部门提交；仅仅在复制权限、允许非法状态组合或隐藏跨所有者提交的同时移动代码并不是边界改进。
