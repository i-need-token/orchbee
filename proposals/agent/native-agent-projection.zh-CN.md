# 原生 Agent 观测投影

| 字段       | 内容                                                                                                                                                                                                                      |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态       | 设计输入，评审中。本文记录已核实的现状事实、精确定位原生 wire 缺口，并为原生 TUI 与 CLI 提出一个可验收的投影边界。在产品与架构 decision owner 接受之前，本文任何内容都不是已接受权威；下文每个选项都只是提案而非决策      |
| 决策 Owner | 以 Orchbee 产品与架构 decision owner 身份行事的 active requester                                                                                                                                                          |
| 语义 Owner | Agent 观测域、Server 语义所有权与原生 Client 投影边界                                                                                                                                                                     |
| 当前权威   | [`agent-runtime-observation.md`](agent-runtime-observation.zh-CN.md)、[`native-runtime-topology.md`](../platform/native-runtime-topology.zh-CN.md)、[`native-command-surface.md`](../cli/native-command-surface.zh-CN.md) |
| 退出条件   | 投影边界被接受，所属切片实现并附证据，持续维护的契约迁入所属 package 与 application 指南及测试，然后删除本提案及其译本                                                                                                    |

英文 [native-agent-projection.md](native-agent-projection.md) 是 canonical 版本；本文用于简体中文评审。

## 变更授权

| 字段     | 建议授权                                                                                                                                                                                                                                                                                                 |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 分级     | 该边界拥有有序状态、cursor 语义、保留溢出与多 Client 投影，因此属于 Modeled；它在 Server 与原生 Client 之间引入新的跨进程契约，因此同时属于 Consequential                                                                                                                                                |
| 问题     | 原生 TUI 与 CLI 完全无法观测 Agent session。canonical observed-session 模型及其易失变化流只存在于 TypeScript 运行时中，位于 Desktop 拥有的 ingress、Desktop 拥有的 renderer 协议与 Desktop 拥有的本地 semantic-command endpoint 之后。Rust Server 没有任何 Agent 模块，原生 wire 族不携带任何 Agent 文档 |
| 阶段结果 | 一条已授权、只读、content-free 的 Agent 观测投影从 Server 拥有的 canonical 模型抵达原生 TUI 与 CLI：先取 baseline snapshot，再以 opaque cursor 跟随有界有序变化页，保留溢出时显式返回 reset-required，并保持 truthful freshness；TUI 通过自己的呈现渲染它，CLI 通过已接受的命令面暴露查询族              |
| 非目标   | Agent 启动、停止、resume、调度、编排、prompt 提交、问题应答、批准或任何 mutation；终端内容或进程抓取；持久历史；remote relay；改变 Workbench publication 语义；在本切片内迁移 Server 侧 canonical 状态；决定 wire schema 名称、数值边界或产品词汇                                                        |
| 兼容性   | Agent 观测缺失、不支持、过期或被拒绝时，终端、Workbench snapshot 与 publication、现有 Client session 及 CLI 命令行为完全一致。不请求该族的 Client 看不到任何变化                                                                                                                                         |
| 交付     | wire 契约与 schema 先于 TUI 和 CLI 消费者。每个消费者切片保有自己的呈现所有权与证据边界                                                                                                                                                                                                                  |
| 恢复     | 可在不触碰 Workbench 投影的情况下撤回该投影族；缺失时所有 Agent 指示降级为隐藏或 `unknown`，绝不凭空产生 working、attention、success 或 failure                                                                                                                                                          |

## 已核实现状

以下陈述是基准提交上的已核实事实，不是提案。

### Canonical 模型与变化流（TypeScript）

- `packages/agent` 拥有 canonical observed-session 模型（`AgentSessionSnapshot`、`AgentRuntimeSnapshot`、`AgentTurnState`、`AgentAttentionRequest`、`AgentTurnOutcome`、`AgentObservation`）、语义投影（`needs-attention`、`failed`、`succeeded`、`canceled`、`interrupted`、`working`、`idle`、`unknown`、`ended`）以及 `AgentRuntime` 内的易失变化 journal（`AGENT_RUNTIME_CHANGE_RETENTION` 为 512 条已应用迁移，`MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE` 为每次读取至多 100 条有序记录）。
- 该 journal 按契约 content-free：不含 prompt、transcript、命令、问题文本、工具内容、文件系统路径、环境值、provider 私有 payload 或屏幕证据。保留溢出返回显式 reset-required 结果，并指引消费者重新获取 canonical snapshot。
- Desktop 拥有 ingress 与 brokering：`apps/desktop/src/main/agent-observation/` 承载 `ORCHBEE_AGENT_OBSERVATION_SOCKET` Unix-socket 监听器，将其与经 `orchbee.agent.v1` 通道以 `TerminalApplicationSignal` 转发的私有 APC correlation 半边汇合，并把汇合后的迁移 dispatch 进运行时。`DesktopAgentObservationBroker` 通过 `desktop-client-protocol` 向 renderer 发布 snapshot-change 与 semantic-event 通知。
- Web Workbench 保留通过 `useAgentSnapshot`、Agent navigation 与 notification hook 消费 `AgentSnapshotSource` 的能力。旧 Desktop semantic endpoint 与 TypeScript CLI consumer 已退役；原生 Agent 行为由其原生所有者维护。

### 原生路径（Rust）

- `apps/orchbee-server/src` 不含任何 Agent 模块、schema 或所有权接缝；`apps/orchbee-server/src/lib.rs` 仅导出 Workbench authority 与 projection 类型。
- `crates/workbench-protocol` 的 `Snapshot`、`Delta`、`Presentation`、`DeltaChange`、`WorkbenchProtocolDocument` 与 `WorkbenchPublicationDocument` 只携带 Workbench navigation 与 terminal-context 数据。`WorkbenchRequest` 恰好准入 `workbench.snapshot.read`、`workbench.command`、`workbench.tab-close.read`、`workbench.tab-close.resume` 与 `workbench.publication.read`。`apps/orchbee-server/src/workbench_projection.rs` 中的 Server publication journal 至多保留 `MAXIMUM_RESULT_PUBLICATIONS`（64）条已提交 publication。
- `apps/orchbee-server/src/local_client_service/application_connection.rs` 按四字节前缀对本地 Client 连接解复用：`OBCS`（终端控制帧）、`OBCQ`（paste 记录）与 `OBWP`（Workbench 请求）。任何其他前缀以 `UnknownProtocol` 终止会话。不存在 Agent 族分支。
- `apps/orchbee-worker` 在 `host_connection/viewers.rs` 中解码 `orchbee.agent.v1` 终端 application signal，仅用于按终端 epoch fencing 后丢弃；不存在任何 Agent 运行时、汇合或转发。`apps/orchbee-server`、`apps/orchbee-worker`、`crates/capability-node-protocol` 与 `crates/client-server-protocol` 中没有任何 Agent 引用。
- `apps/orchbee`（原生 TUI/CLI 可执行文件）通过 `snapshot.read` 基线加 `publication.read` cursor 读取轮询 Workbench 状态，并在普通轮询上搭 250 ms publication probe；提示转化为 `PublicationDebt` 刷新需求。`tui_render.rs` 绘制一条静态本地化 `Agents` 侧栏行，不消费任何状态。原生 CLI 面无 Agent 命令。

## 精确 wire 缺口

原生 Client 在当前 wire 上无法表达以下任何一项：

1. 读取 Agent 观测 snapshot——不存在相应请求 kind；`Snapshot` 既没有 Agent 集合，也没有 per-presentation Agent 字段。
2. 跟随有界变化流——`PublicationReadIntent`/`WorkbenchPublicationOutcome` 是 Workbench-revision journal，而非 Agent journal；其 64 条保留与 Workbench revision 空间无法表达已接受的 512 条 Agent journal、≤100 页界、签名 opaque cursor 或 reset-required 结果。
3. 接收 Agent 状态的 reset-required 信号——`WorkbenchPublicationOutcome` 只有 `Current`、`Published`、`Denied` 与 `Unavailable`。
4. 把 Agent presentation attachment 映射到原生 presentation——canonical 模型把 `presentationAttachments[].presentationId` 存为 1–256 字符 opaque 字符串，而原生 Workbench `TerminalPresentationId` 恰好是 32 位小写十六进制字符。两侧都不存在映射契约。
5. 命名该观测族——四字节解复用没有 Agent magic，且 `ClientMessage`/`ServerMessage`（OBCS）与 `WorkbenchRequest`（OBWP）都不准入 `agent.*` kind。

## 投影边界选项

| 选项                                                                | 形态                                                                                                       | 后果                                                                                                                                                                                                                          |
| ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A. 在 Workbench `Snapshot`/`Delta` 中内嵌 Agent 字段                | 向 `crates/workbench-protocol` 文档加入 session 集合或 per-presentation 标记                               | 融合两个权威与两个 revision 空间：Workbench journal（64 条已提交 publication、Workbench revision）将携带无关的 Agent 变化（512 条迁移 journal、自有 cursor 与 reset 语义），且每个 Workbench 消费者都要解析它不拥有的域。拒绝 |
| B. 在现有 OBWP 请求枚举中准入 `agent.observation.*` 请求与结果 kind | 在同一带帧连接上扩展 `WorkbenchRequest` 与响应枚举加入 Agent intent                                        | 复用带帧连接、请求标识、版本与 dispatch，无需新解复用分支；但该枚举与 crate 以 Workbench 投影协议命名和归属，Agent kind 要么误名该族，要么迫使重命名/泛化。仅当 owner 接受命名空间张力时可行                                  |
| C. 独立 Agent-observation 协议族                                    | 在同一已认证本地 Client 连接上新增四字节 magic 与 crate，拥有自己的请求/结果文档、journal 与 revision 空间 | 与仓库的一域一族解复用（`OBCS`、`OBCQ`、`OBWP`、`OBCN`、`OBLW`）一致，所有权无歧义，且不触碰 Workbench；代价是一个新解复用分支、一个 crate 及其自身的符合性证据                                                               |
| D. 在 `ClientMessage`/`ServerMessage`（OBCS）内做 Agent 读取        | 扩展终端控制帧族                                                                                           | OBCS 当前承载终端 attachment 与控制消息，而非投影读取；把域投影混入控制族会在第二个载体上重演选项 B 的所有权问题。拒绝                                                                                                        |

### 推荐

优先选项 C，选项 B 为可接受的较轻备选。决定性属性不是字节级载体而是语义边界：一个 Agent-observation 族，拥有自己的版本化文档、自己的 revision 与 cursor 空间、自己的有界 journal，且与 Workbench 不共享序列。选项 C 最诚实地表达了该边界；若 owner 偏好更少协议族并接受 `agent.*` 命名空间与 `workbench.*` kind 并存，选项 B 可达同等语义。无论选择哪个载体，该族不得把 Agent 状态内嵌进 `Snapshot`、`Delta` 或 `Presentation`，也不得复用 Workbench publication journal、revision 或 cursor 空间。

确切 wire 形态——新 magic 还是 OBWP 上的新 kind、schema 名称、数值边界、兼容性协商与协议版本阶梯——仍属 wire-契约阶段的保留决策。本文仅断言所选形态必须满足的语义要求。

## 必需的消费契约

无论选择哪个载体，投影必须保持已接受的变化流语义：

1. Client 先在被记录 revision 上读取 canonical snapshot，然后以 opaque cursor 跟随有界变化页。每页至多返回 100 条有序记录。
2. 保留溢出绝不呈现为连续流。结果是显式 reset-required，携带当前 revision 并指引 Client 重新获取 snapshot。
3. Cursor 按 coordinator generation、operation 与目标 fencing；伪造、跨 generation、跨 operation 或畸形 cursor 一律拒绝。跨 Server 重启不承诺任何连续性。
4. 记录保持 content-free：仅含 session 标识、provider、execution location、presentation attachment、生命周期、turn 状态、attention request、outcome 与观测健康（fidelity、freshness、generation、sequence、source）。
5. 该族只读。任何请求不得启动、停止、resume、prompt、应答、批准、调度或编排。
6. 长轮询是 Server 侧准入选择，而非 wire 要求；有界的 `wait`/`changes` 调用既可映射到 Server 长轮询，也可映射到有界 Client 重轮询。

## TUI 投影位点

TUI 作为对等 renderer 消费该投影；它只拥有呈现。

- 侧栏：把静态 `Agents` 行替换为取自投影的聚合条目——session 计数、attention 计数与最高优先级语义状态。当该族缺失或被拒绝时，必须降级为当前的静态呈现。
- Tab 条：仅当存在 attachment 映射时，per-presentation 语义指示才可准入；它属于 presentation 槽位而非 Tab 标签，未映射时不渲染。
- Navigator：更丰富的呈现面——按 provider、execution location、语义状态与 freshness 分组的 session 列表——更适合 navigator 覆盖层，而不是永久扩展紧凑 Tab 行。
- 刷新机制：Agent 变化提示追踪自己的债务与刷新 generation，与 `PublicationDebt` 分离。probe 可搭现有轮询节奏，但它发起 Agent 族读取而非 `publication.read`；Workbench publication 仍是另一个域的 journal。
- 状态词汇（`blocked`、`working`、`done`、图标、颜色）属产品语言，保持保留；wire 携带九个语义状态与 freshness，由 renderer 负责映射。

## CLI 查询族

`native-command-surface.md` 把 Agent 列在 later domains 中，仅由其所属已接受域契约加入；本文即是该域关于读取的输入。建议准入五个读取操作作为 resource-first 命令——`agent session list`、`agent session get`、`agent session explain`、`agent session wait` 与 `agent session changes`——形态为：

- 无状态调用：在被记录 revision 上获取 baseline 并打印一个有界页；`wait` 与 `changes` 是有界单次调用，而非驻留订阅；
- stdout 输出版本化 JSON 或 NDJSON，stderr 输出本地化诊断，并使用已接受的稳定 exit 类别，其中 reset-required 使用独立类别而非静默缺页；
- 不初始化 TUI、不依赖 Desktop、不直接读取产品状态文件；
- 不含 mutation 动词；任何后续 Agent 控制族是单独决策。

## 关联与迁移说明

- canonical 模型迁入 Rust 时必须调和 presentation attachment 标识：TypeScript 模型允许至多 256 字符的 opaque 字符串，而原生 `TerminalPresentationId` 为 32 位小写十六进制。Server 拥有的投影应发出原生 presentation 标识，使每个 Client 无需 per-renderer 关联规则即可映射 attachment。
- 终端 effect wire 上的 `orchbee.agent.v1` 通道名与 `orchbee.agent.v1;` APC 前缀是 ingress 传输细节，不是面向 Client 的观测契约；Client 族命名不得继承其中任何一个。
- 当前 TypeScript package（`@orchbee/agent` 契约）在已接受迁移逐个切换消费者之前仍是其现有消费者的当前权威；它们不会静默变成 Rust wire 契约。首个 Rust 生产者与消费者之前必须有语言中立 schema 与跨语言符合性证据。
- Server 重启 fencing、remote-Client 可达性与 Server 侧 journal 由 Server 迁移切片拥有；本提案消费其结果而不替其决策。

## 保留决策

- 确切 wire 形态：新 magic 族还是现有载体上的新 kind、schema 名称、数值边界、兼容性协商与版本阶梯。
- 产品呈现词汇、图标、颜色，以及聚合条目在侧栏排序中的位置。
- Agent 是否进入 mutation 域；本文仅提出读取。
- canonical attachment 到原生 presentation 的标识契约，以及任何已存标识格式的迁移。
- 跨 Server 重启的 journal 保留、cursor 签名与 reset 语义；512/100 边界是 TypeScript journal 的已接受值，是参考点而非自动的 wire 承诺。
- 该族的 remote Client 交付与 Server 网络准入。

## 任何已接受切片的证据义务

- 每种请求与结果 kind 的 schema fixture，包括 reset-required、denied、unavailable 与畸形 cursor 拒绝。
- Journal 符合性证据：有序批量、连续 revision 页、保留溢出、cursor fencing 与 coordinator 退休行为，对齐已接受的 TypeScript journal 测试与有界模型。
- TUI 证据：缺失、拒绝与 reset-required 下真实降级、不捏造状态；CLI 证据：机器输出、exit 类别与有界调用。
- 该切片触及的每个仓库文档都要完成双语文档配对。

<!-- documentation-pairing: english-sha256=fb53b7c07daa3b31560dac672c2a5b5565fb766bcbcc05bdbafafa4e74aac50f zh-cn-sha256=722e4dc537abe8313c26b5e7995db10dd6067f1013a88619e4c6250b4b7c1e93 -->
