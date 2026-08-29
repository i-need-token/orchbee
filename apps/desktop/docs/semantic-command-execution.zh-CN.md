# 语义命令执行

## 边界

Desktop 拥有一个进程生命周期内的 semantic-command coordinator generation、显式 operation catalog、
有界内存 execution ledger，以及向已认证 current-user client 暴露该 service 的 private local endpoint。
生产 catalog 精确包含十二个 operation：`agent.session.list`、`agent.session.get`、
`agent.session.explain`、`agent.session.wait` 与 `agent.session.changes`，以及 `workbench.project.list`、
`workbench.directory.list`、`workbench.workspace.list`、`workbench.tab.list`、
`workbench.panel.list`、`workbench.terminal.list` 与 `workbench.terminal.create`。它不提供任意
Action、filesystem、shell、renderer 或 Terminal Host escape hatch。

Feature owner 保留 input 与 output 含义。Protocol package 拥有严格 envelope 与 local HTTP mapping。
Desktop 拥有 catalog admission、execution、locator publication、endpoint lifecycle 和 OS peer
authentication。它不拥有 Workbench resource 语义、CLI presentation、remote authentication 或持久
semantic-execution recovery。

| 输入               | 验证与缺失语义                                                                                                                                                |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Locator            | 只包含严格 version、generation 与 32 字符 endpoint identity；不含 credential、path、address 或产品数据                                                        |
| Local peer         | 双向验证 POSIX effective UID 或 Windows current-user process SID；identity 缺失或不匹配会拒绝 connection                                                      |
| HTTP request       | 一条 close-delimited HTTP/1.1 request、严格 method/path、16 KiB header、64 KiB body，且 `POST` 仅接受 JSON；malformed 或 over-capacity input 不会到达 service |
| Invocation request | 使用 `SemanticCommandInvocationRequestSchema`；格式错误的值不分配 execution                                                                                   |
| Catalog entry      | 构造时验证 identity 与 version 唯一，并提供 public descriptor、canonical input parser、authorization 与 confirmation policy、safe-output validator 和 handler |
| Handler settlement | 使用严格 `SemanticCommandTerminalResult`；throw、无效 output 和超过单项 safe-output bound 的 result 会变成不带 payload 的所属 failure                         |

## 数据与输出

| 状态或输出             | Owner 与生命周期                                                                                      |
| ---------------------- | ----------------------------------------------------------------------------------------------------- |
| Catalog                | 不可变的 `DesktopSemanticCommandCatalog`；只存在五项 Agent query 与七项 Workbench entry               |
| Coordinator generation | 随 service 创建一次，复制到 locator，并出现在每个 public snapshot 中                                  |
| Endpoint identity      | 仅用于 locator 与 socket 或 named-pipe 名称的随机进程局部 identifier                                  |
| Locator                | Native adapter 在 listener 创建后发布一个 private bounded file，并在 teardown 时移除                  |
| Request receipt        | Ledger 在保留期内拥有 request identity、canonical SHA-256 operation fingerprint 与 execution identity |
| Active execution       | Ledger 拥有 identity、commit marker 与 cancellation-forward count；service 拥有一个 `AbortController` |
| Terminal execution     | Ledger 拥有 safe result 与测量后的 byte weight，直到按最旧优先淘汰                                    |
| HTTP response          | 一条不超过 1 MiB 的严格 JSON response；framing error 不带 payload，且绝不序列化 exception             |
| Observation output     | 保留的 snapshot，或者精确的 request/execution receipt-unavailable result                              |

Ledger 不存储 canonical input。异步 handler closure 只能在 execution active 期间持有 canonical input。
Diagnostics 不接收 operation input 或 result payload。Query result 可以在其有界 response 与 receipt 中显式包含
调用方请求的 name 与 path。

只有 `workbench.directory.list` 使用 operation schema version `2` 与 output schema version `2`；每项
result 都包含 Directory placement。其他六项 Workbench operation 保持 version `1`。Remote Directory
result 只包含已 commit 的 opaque Environment identity 与 canonical path。Requested alias、失败
observation 与原始 operating-system diagnostic 绝不进入 semantic output。

Tab 与 Panel list 只投影规范共享文档成员关系。Tab 顺序是共享 Tab collection 顺序；精确目标 Panel
顺序是深度优先且从左到右。两种 query 都不读取或返回 Client View 的 active、selected、focused、
maximized、active-child、split-weight 或 geometry 状态。签名 cursor 会绑定到精确 location，Panel cursor
还会绑定到精确 Tab identity。

五个 Agent operation 都使用 schema version `1`。List、get 与 explain 从 application-lifetime
`AgentSnapshotSource` 获取一个 canonical snapshot。List page 经过签名并受 revision fencing；get 与
explain 精确指向一个保留的 Session。Wait 是 server-owned asynchronous query：先 subscribe 再进行
decisive recheck，重新评估 canonical source，并在一个精确 match、Session missing、future revision
baseline、timeout、cooperative cancellation 或 coordinator retirement 上结束。Changes 会取得立即返回的
空 baseline，或沿签名 cursor 跟随最近 512 个 applied runtime transition。它最多返回 100 条有序、
content-free record，会先 subscribe 再 recheck current cursor，把 timeout 作为 `current` 完成，并在跨过
retention 时返回显式 `reset-required`。Semantic-status condition 要求 fresh observation。Agent query
不读取 renderer、terminal screen、provider transport、prompt、transcript、tool content、filesystem path、
environment value、provider error 或 adapter-private correlation data。

## 不变量

1. 一个保留中的 request identity 精确映射到一个保留中的 execution 和一个 canonical fingerprint。
2. 使用相同 fingerprint 重复该 request 会返回已有 snapshot；使用不同 fingerprint 会返回
   `request-identity-conflict`；两者都不会启动另一个 handler。
3. 无效、不可用、未授权、需要 confirmation、版本不兼容或超容量的 request 都不会分配 execution。
4. 最多有 32 个 active execution，且 active record 永不被淘汰。
5. 对一个 active 且未提交的 execution 最多转发一次 cooperative cancellation。标记 durable commit 后不再
   转发 cancellation。Cancellation 不会自行制造 terminal result，也不会覆盖 handler settlement。
6. 标记为 `committed` 的 handler settlement 只能包含 `completed`；未提交 settlement 可以包含任意
   terminal result kind。
7. Terminal retention 最多包含 256 条 record，编码后的 safe result 总量最多为 32 MiB。Completion
   会淘汰最旧的 terminal record，直到两个限制都满足。
8. 单个 execution result 被限制在 local response capacity 以下。Oversize 会变成不带 owner payload 的
   `failed/output-capacity-exceeded`。
9. Discovery 与 description 精确暴露十二项已组合 operation。Catalog registration 或 UI Action availability
   本身不授予 process authority。
10. Native adapter 既不跟随 POSIX symlink，也不跟随 Windows reparse point，并且只接受 current-user
    private runtime directory、locator、endpoint 与双向认证 peer。
11. 一条 connection 承载一条有界 request 和一条有界 response。未知 framing、method、path、content
    type、trailing byte 与 capacity 都会 fail closed。
12. Locator publication 发生在 listener 创建后。Shutdown 会先 retire locator 与 listener，再 dispose
    service。Client disconnect 不对应 service transition，也不会引发 replay。

## Transition

| 状态             | 事件                    | Guard                                                                                        | 结果                                                                |
| ---------------- | ----------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| Endpoint 缺失    | Start                   | Trusted runtime directory 与 native listener 验证通过                                        | 创建 listener、发布匹配 locator，然后报告 ready                     |
| Listening        | Accept                  | Peer identity 与当前 effective OS user 匹配                                                  | 精确读取一条有界 request                                            |
| Listening        | Reject peer             | Peer identity 缺失或不匹配                                                                   | 关闭该 connection；不分配 execution                                 |
| Connected        | Reject request          | Framing、route、content type、timeout 或 capacity guard 失败                                 | 尽可能返回一条有界 local error；service 与 ledger 不变              |
| Request 缺失     | Admit                   | Catalog entry 可见，且 version/input/authorization/confirmation 通过，并仍有 active capacity | 分配一个 execution 并启动一个 handler                               |
| Request 已保留   | Repeat 或 conflict      | Fingerprint 相同或不同                                                                       | 返回已有 snapshot，或返回 conflict 且不启动第二个 handler           |
| Active execution | Cancel                  | Execution 尚未提交且未转发 cancellation                                                      | 转发一次 cooperative abort                                          |
| Active execution | 标记 committed          | Canonical Workbench revision 完成 durable storage                                            | 在 post-commit reconciliation 前记录 commit；拒绝后续 cancellation  |
| Active execution | Settle                  | Settlement 与 commit-marker consistency guard 成立                                           | 保留一个经过验证的 terminal result                                  |
| Connected        | Client disconnect       | Request 可能已经到达 service                                                                 | 只丢弃 response transport；不 cancel 或 replay 已接受 execution     |
| 任意状态         | 进程 dispose 或 restart | Coordinator 生命周期结束                                                                     | Retire locator/listener、abort active signal，并丢弃内存 generation |

## 依赖与所有权

`DesktopSemanticCommandService` 编排 catalog admission、fingerprinting、handler execution、safe-result
capacity 与 cancellation。`DesktopSemanticCommandExecutionLedger` 是唯一可变 receipt writer，不依赖
network、filesystem、clock、Workbench、Electron 或 process。`createDesktopWorkbenchReadOperations`
通过 Workbench owner 的确定性 projection 适配 canonical shared document，并把 page cursor 绑定到
operation、target、generation 与 revision。`createDesktopWorkbenchTerminalCreateOperation` 验证
Workbench 所属的严格 input，把 owner operation 应用到隔离且 hydrated 的 runtime，只通过
`DesktopClientSessionCoordinator` 提交 projected next revision，并在 coordinator 的 storage 后即时 callback
中把 execution 标记为 committed。隔离 runtime 绝不 dispatch provision command；正常 canonical
reconciliation 只会在持久化成功后启动 PTY。

`createDesktopAgentReadOperations` 通过 Agent-owned projection 与 contract 适配 application-lifetime
`AgentSnapshotSource` 和 `AgentChangeSource`。Desktop 拥有 signed cursor binding、asynchronous wait 与
change subscription、timeout scheduling、cooperative abort 与 coordinator-lifetime failure boundary；
它不重新解释 Agent status，也不成为另一名 Agent state owner。

`DesktopSemanticCommandLocalEndpoint` 拥有 Worker lifetime。Worker 在 Electron Main 之外执行有界 blocking
local I/O，只转发完整 request byte。Native adapter 拥有 private runtime、locator、Unix-domain socket 或
Windows named-pipe creation，以及双向 current-user validation。
`@orchbee/semantic-command-protocol/local-http` 拥有 framing 与 route；CLI 拥有 trusted enumeration 与
response validation。该 endpoint 与 Terminal Host、Browser access 分离，且不打开 TCP listener。

## 正确性证据

| 声明                                             | 契约          | 形式化或生成式证据                                                                                                         | 确定性或真实 refinement                                                                                                                                   | 边界                                                                                        |
| ------------------------------------------------ | ------------- | -------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| 只允许显式且已授权的 catalog                     | 不变量 1-3、9 | `SemanticCommandExecutionLedger.tla`：`RequestAndExecutionMappingsAgree`                                                   | Catalog/service test 覆盖负向 admission 与精确十二项 composition；HTTP integration 调用全部 Workbench read 及精确 target 的 Terminal 创建                 | 仅十二项已接受 operation                                                                    |
| 保留期间至多一个 execution                       | 不变量 1-4    | `RetainedRequestHasOneHandlerStart`、`ActiveExecutionCapacity`                                                             | Ledger boundary test 与固定 seed 的 stateful command property                                                                                             | 淘汰或 restart 后不作声明                                                                   |
| Cancellation 与 retention 安全                   | 不变量 5-8    | `CommittedExecutionNeverCancels`，以及两个 TLC variant 中的 cancellation、count 与 weight invariant                        | Deferred-handler、commit 前后 cancellation、duplicate request、receipt eviction 及精确 capacity test                                                      | 不声明 handler liveness 或 durable storage 成功后的 rollback                                |
| Agent wait 匹配 canonical state                  | 不变量 3、9   | `AgentSemanticWait.tla`：exact subscription、eligibility、freshness、output 与 revision invariant                          | Agent domain 与 Desktop operation test 覆盖 pagination、exact read、status explanation、race-safe recheck、timeout、cancellation 与 future baseline       | 仅有界 volatile runtime；不声明 restart continuity 或 provider truth                        |
| Agent change 暴露每个 retained revision 或 reset | 不变量 3、9   | `AgentSemanticChanges.tla`：ordered suffix、batch、subscription、gap、reset、timeout、cancellation 与 retirement invariant | Agent journal 与 Desktop operation test 覆盖 applied-only record、page order、signed cursor、synchronous recheck、overflow/reset、timeout 与 cancellation | Retention 为内存中 512 条；不声明 durable replay、cryptographic proof 或 restart continuity |
| Locator 与 peer 保持 current-user                | 不变量 10、12 | Native source 会为维护中的 POSIX 与 Windows family 编译                                                                    | Native Node 与 Electron test 会创建、认证、读取、拒绝不安全 POSIX ownership 并 retire endpoint；维护中的 Windows CI 会执行 DACL/SID path                  | 信任受损 same-user process；cross-user 负向执行需要 platform account fixture                |
| Framing 与 capacity fail closed                  | 不变量 8、11  | 严格 protocol framing 与 route schema                                                                                      | Unit rejection matrix 与真实 Node/Electron local-transport test 覆盖 trailing input、64 KiB request、1 MiB response、malformed input 与恢复               | Kernel queue scheduling 与 process kill 不属于该声明                                        |
| Disconnect 不会 cancel 或 replay                 | 不变量 12     | `Disconnect` 与 `Reconnect` 保持 ledger variable 不变                                                                      | 真实 client timeout 证明只交付一次；CLI test 会恢复同一保留 request，并在 generation 变化后报告 unknown 且不重新 invocation                               | 排除跨 machine 或 coordinator restart 的 durable recovery                                   |

注册的 TLC model 使用两个 request identity、两个 fingerprint、三个 execution identity、一个 active
slot、两个 coordinator generation，以及分别限制 count 和 weight 的 retention variant。这些边界会暴露
replay、conflict、cancellation、completion-order eviction 与 restart interleaving；它们不证明无界状态。
Native test 会在当前平台的 Node 与 pinned Electron runtime 下运行，而维护中的 macOS 与 Windows CI matrix
是平台行为 authority。不会运行任何 manual native-window test。

Agent-wait model 还包含两个 Session、两个 semantic status、三个 health value、两次 Agent mutation
（revision 0 到 2）与两个 coordinator generation。其 action coverage 包含 begin、missing、ahead、update、removal、timeout、
cancellation 与 retirement path。这些边界可以暴露 subscription 与 decisive-recheck race，但不声明无界
waiter count、time、revision 或 restart recovery。

Agent-changes model 使用 revision 0 到 4、retention 2、page bound 2 与两个 coordinator generation。
Required action coverage 包含 baseline、current follow、immediate batch、overflow reset、invalid cursor、
update、timeout、cancellation 与 retirement。这些边界会暴露 silent-gap、ordering、batching 与
subscribe-before-recheck 错误，但不声明无界 retention、reader、revision、cryptographic cursor security
或 restart recovery。
