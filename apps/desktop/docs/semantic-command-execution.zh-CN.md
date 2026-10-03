# 语义命令执行

## 边界

Desktop 拥有一个进程生命周期内的 semantic-command coordinator generation、显式 operation catalog、
有界内存 execution ledger，以及向已认证 current-user client 暴露该 service 的 private local endpoint。
生产 catalog 精确包含十三个 operation：`agent.session.list`、`agent.session.get`、
`agent.session.explain`、`agent.session.wait` 与 `agent.session.changes`，以及 `workbench.project.list`、
`workbench.directory.list`、`workbench.workspace.list`、`workbench.tab.list`、
`workbench.panel.list`、`workbench.terminal.list`、`workbench.terminal.output.read` 与
`workbench.terminal.create`。它不提供任意 Action、filesystem、shell、renderer 或 Terminal Host escape hatch。

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
| Catalog                | 不可变的 `DesktopSemanticCommandCatalog`；只存在五项 Agent query 与八项 Workbench entry               |
| Coordinator generation | 随 service 创建一次，复制到 locator，并出现在每个 public snapshot 中                                  |
| Endpoint identity      | 仅用于 locator 与 socket 或 named-pipe 名称的随机进程局部 identifier                                  |
| Locator                | Native adapter 在 listener 创建后发布一个 private bounded file，并在 teardown 时移除                  |
| Request receipt        | Ledger 在保留期内拥有 request identity、canonical SHA-256 operation fingerprint 与 execution identity |
| Active execution       | Ledger 拥有 identity、commit marker 与 cancellation-forward count；service 拥有一个 `AbortController` |
| Terminal execution     | Ledger 拥有 safe result 与测量后的 byte weight，直到按最旧优先淘汰                                    |
| HTTP response          | 一条不超过 1 MiB 的严格 JSON response；framing error 不带 payload，且绝不序列化 exception             |
| Observation output     | 保留的 snapshot，或者精确的 request/execution receipt-unavailable result                              |
| Terminal text output   | 一个不超过 1,000 行与 128 KiB 的 exact-frame 纯文本 tail；只通过有界 ledger 保留                      |

Ledger 不存储 canonical input。异步 handler closure 只能在 execution active 期间持有 canonical input。
Diagnostics 不接收 operation input 或 result payload。Query result 可以在其有界 response 与 receipt 中显式包含
调用方请求的 name 与 path。

只有 `workbench.directory.list` 使用 operation schema version `2` 与 output schema version `2`；每项
result 都包含 Directory placement。其他七项 Workbench operation 保持 version `1`。Remote Directory
result 只包含已 commit 的 opaque Environment identity 与 canonical path。Requested alias、失败
observation 与原始 operating-system diagnostic 绝不进入 semantic output。

Tab 与 Panel list 只投影规范共享文档成员关系。Tab 顺序是共享 Tab collection 顺序；精确目标 Panel
顺序是深度优先且从左到右。两种 query 都不读取或返回 Client View 的 active、selected、focused、
maximized、active-child、split-weight 或 geometry 状态。签名 cursor 会绑定到精确 location，Panel cursor
还会绑定到精确 Tab identity。

`workbench.terminal.output.read` 固定一个精确的 canonical Terminal presentation identity，并且只打开
capture read attachment。它可以从 live local Terminal、已连接的 SSH Terminal 或保留的 local archive 读取
当前 tail，而不会启动或重连 runtime。默认值为 80 行，owner 上限为 1,000 行，public text 上限为 128 KiB。
Reader 针对一个精确 Host generation 与 revision 请求纯文本，拼接 soft-wrapped row，并且不公开 ANSI
style、graphic、hyperlink、raw VT byte 或未来 output。Input、paste、key、resize、focus、close 与 stop authority
保持缺失。Frame 变化、target retired、projection unavailable、cancellation、cleanup failure，或用于 lease
acquisition 与 frame/row collection 的五秒 deadline 到期时，都会选择一个不含旧文本且不暗中重试的显式
result。Lease 一旦 acquired，reader 会在 publication 前等待一次 cleanup attempt。Cleanup 成功后才能发布
普通 result；dispose rejection 会发布 `cleanup-failed`。Cleanup 没有单独 deadline，因此 publication 可能超过
五秒。文本不会自动脱敏，可能包含 secret；它不会进入 diagnostic、telemetry 或 durable history，但现有
有界、易失的 execution ledger 可以保留 safe result，直到淘汰或 restart。

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
   Execution identity 在一个 process lifetime 中最多准入一次，即使其 receipt 已被淘汰也一样。
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
9. 成功的 Terminal output read 会命名请求的 live target 及一个精确 Host generation 与 revision，只返回有界
   纯文本，并且只有在 capture lease 已 dispose 后才结束。每个 acquired read 都会在 publication 前尝试
   cleanup；dispose rejection 会发布 `cleanup-failed`。每个 stale、retired、unavailable、cancelled 或
   cleanup-failed outcome 都不包含 Terminal text。
10. Discovery 与 description 精确暴露十三项已组合 operation。Catalog registration 或 UI Action availability
    本身不授予 process authority。
11. Native adapter 既不跟随 POSIX symlink，也不跟随 Windows reparse point，并且只接受 current-user
    private runtime directory、locator、endpoint 与双向认证 peer。
    Windows 创建时显式指定当前 process 的 `TokenUser` 为 owner，并提供 protected private DACL，
    不依赖 token 的 default owner。已有 runtime directory 以不跟随 reparse point 的方式打开，
    先验证 current-user ownership，再通过同一个 handle 收紧 DACL。已有同用户 broad 或 null DACL
    仍可修复；foreign ownership 或 reparse point 会被拒绝，且不会修改 ownership 或 permission。
12. 一条 connection 承载一条有界 request 和一条有界 response。未知 framing、method、path、content
    type、trailing byte 与 capacity 都会 fail closed。
13. Locator publication 发生在 listener 创建后。Shutdown 会先 retire locator 与 listener，再 dispose
    service。Client disconnect 不对应 service transition，也不会引发 replay。

## Transition

| 状态             | 事件                    | Guard                                                                                          | 结果                                                                |
| ---------------- | ----------------------- | ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| Endpoint 缺失    | Start                   | Trusted runtime directory 与 native listener 验证通过                                          | 创建 listener、发布匹配 locator，然后报告 ready                     |
| Listening        | Accept                  | Peer identity 与当前 effective OS user 匹配                                                    | 精确读取一条有界 request                                            |
| Listening        | Reject peer             | Peer identity 缺失或不匹配                                                                     | 关闭该 connection；不分配 execution                                 |
| Connected        | Reject request          | Framing、route、content type、timeout 或 capacity guard 失败                                   | 尽可能返回一条有界 local error；service 与 ledger 不变              |
| Request 缺失     | Admit                   | Catalog entry 可见，且 version/input/authorization/confirmation 通过，并仍有 active capacity   | 分配一个 execution 并启动一个 handler                               |
| Request 已保留   | Repeat 或 conflict      | Fingerprint 相同或不同                                                                         | 返回已有 snapshot，或返回 conflict 且不启动第二个 handler           |
| Active execution | Cancel                  | Execution 尚未提交且未转发 cancellation                                                        | 转发一次 cooperative abort                                          |
| Active execution | 标记 committed          | Canonical Workbench revision 完成 durable storage                                              | 在 post-commit reconciliation 前记录 commit；拒绝后续 cancellation  |
| Terminal read    | 观察 exact frame        | 精确 target 打开 capture-only lease，且 generation/revision 保持 current                       | 读取有界纯文本 tail、尝试 lease cleanup，然后发布                   |
| Terminal read    | Abort 或失效            | 发生 cancellation、deadline、stale frame、target retirement、unavailability 或 cleanup failure | 选择不带文本的 result；dispose rejection 会替换为 `cleanup-failed`  |
| Active execution | Settle                  | Settlement 与 commit-marker consistency guard 成立                                             | 保留一个经过验证的 terminal result                                  |
| Connected        | Client disconnect       | Request 可能已经到达 service                                                                   | 只丢弃 response transport；不 cancel 或 replay 已接受 execution     |
| 任意状态         | 进程 dispose 或 restart | Coordinator 生命周期结束                                                                       | Retire locator/listener、abort active signal，并丢弃内存 generation |

## 依赖与所有权

`DesktopSemanticCommandService` 编排 catalog admission、fingerprinting、handler execution、safe-result
capacity 与 cancellation。`DesktopSemanticCommandExecutionLedger` 是唯一可变 receipt writer，不依赖
network、filesystem、clock、Workbench、Electron 或 process。`createDesktopWorkbenchReadOperations`
通过 Workbench owner 的确定性 projection 适配 canonical shared document，并把 page cursor 绑定到
operation、target、generation 与 revision。`createDesktopWorkbenchTerminalCreateOperation` 验证
Workbench 所属的严格 input，把 owner operation 应用到隔离且 hydrated 的 runtime，只通过
`DesktopClientSessionCoordinator` 提交 projected next revision，并在 coordinator 的 storage 后即时 callback
中把 execution 标记为 committed。Coordinator 会隔离 callback failure，使其无法抑制 canonical publication
或 reconciliation；service 会在 validation 前根据 handler 的 committed settlement 再次确认 marker。隔离
runtime 绝不 dispatch provision command；正常 canonical reconciliation 只会在持久化成功后启动 PTY。

`createDesktopTerminalOutputReadOperation` 委托给 `DesktopTerminalOutputReader`，后者通过
`DesktopTerminalProjectionRouter` 打开精确 presentation。Local live 与 archived target 来自 Desktop
Terminal runtime；已连接 SSH target 来自现有 environment runtime owner，不会进行 preparation 或
reconnection。Reader 拥有五秒 deadline、exact-frame row paging、public text projection、cancellation 与
lease cleanup；该 deadline 覆盖 lease acquisition 与 frame/row collection，并在 reader 等待 cleanup 前清除。
Workbench 拥有严格 operation input 与 output contract；Desktop 拥有 routing 与 execution，Terminal Host
则继续对 captured screen row 负责。

`createDesktopAgentReadOperations` 通过 Agent-owned projection 与 contract 适配 application-lifetime
`AgentSnapshotSource` 和 `AgentChangeSource`。Desktop 拥有 signed cursor binding、asynchronous wait 与
change subscription、timeout scheduling、cooperative abort 与 coordinator-lifetime failure boundary；
它不重新解释 Agent status，也不成为另一名 Agent state owner。

`DesktopSemanticCommandLocalEndpoint` 拥有 Worker lifetime。Worker 在 Electron Main 之外执行有界 blocking
local I/O，只转发完整 request byte。Native adapter 拥有 private runtime、locator、Unix-domain socket 或
Windows named-pipe creation，以及双向 current-user validation。
`@orchbee/semantic-command-protocol/local-http` 拥有 framing 与 route；CLI 拥有 trusted enumeration 与
response validation。该 endpoint 与 Terminal Host、Browser access 分离，且不打开 TCP listener。

Windows client 可能在发现 listening pipe 后或打开 pipe 后暂停。Worker 的一次 poll 必须保留这两种
操作，避免把健康 coordinator 错误报告为 unavailable。
`acceptBefore(server, pollMilliseconds, connectionMilliseconds)` 在 poll 尚未完成时返回 `null`，
认证成功后返回正数的剩余 request budget。Worker 传入既有的 100 ms poll 与 5,000 ms connection
budget；native 不另设连接时长。该 budget 负责 admission 和 request reading；既有 parent-response
和 write budget 仍独立。POSIX 在 peer validation 后返回该 request budget。

Windows server 独占一个位于 heap 的 overlapped operation、event、私有首 byte 和 monotonic
connection deadline；只有串行 Worker call 可修改它们。Idle listening 没有 connection expiry。
第一次观察到 connection 时冻结传入 budget；后续 poll 和认证不续期。一次 poll budget 同时覆盖
connect completion 和 preread。

| 状态                                    | 事件与条件                                                    | 转移与可观察结果                                                                                          |
| --------------------------------------- | ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Idle                                    | Accept call                                                   | 启动唯一 owned ConnectNamedPipe operation；进入 Listening                                                 |
| Listening                               | Poll 到期                                                     | 保留同一 pending operation；返回 null                                                                     |
| Listening                               | 观察到 connection                                             | 冻结 connection deadline；进入 Awaiting identity byte                                                     |
| Awaiting identity byte                  | Poll 到期且 connection deadline 未到                          | 保留 pending read 和私有 byte；返回 null                                                                  |
| Awaiting identity byte                  | Byte 完成、精确 SID 匹配、RevertToSelf 成功且 budget 尚有剩余 | 进入 Authenticated；仅返回剩余 budget                                                                     |
| Awaiting identity byte 或 Authenticated | 原 connection deadline 到期、peer 失败或 identity 被拒绝      | 等待精确 operation 静止、断连并清除 prefix；进入 Closed，返回 terminal error                              |
| Authenticated                           | 原 deadline 前读取                                            | 在 stream tail 前仅返回一次 prefix；较短的 read poll 可返回 null                                          |
| Retired                                 | 任意 accept、read 或 write                                    | 返回 unavailable；永不重用 retired pipe                                                                   |
| Closed                                  | 再次 accept 或 read                                           | 返回 terminal error；不隐式接纳其他 peer                                                                  |
| 任意 live state                         | 显式 disconnect 或 Worker stop/close                          | 取消精确 pending operation 并观察完成后才能重置或释放 event、buffer、pipe；disconnect 允许新的 Idle owner |

认证读取一个 byte 来建立 Windows client security context。在精确 current-user SID 验证和
`RevertToSelf` 成功之前，不发布 byte、request parse 或 service effect。Native 在发布认证前、
返回 held prefix 前和 body I/O 完成后检查原 deadline。它始终是 Windows deadline authority；
Worker 的 remaining-budget receipt 只限制本地 loop 和 250 ms read poll。Receipt 后的调度不会
延长 native read authority。Terminal expiry 会断连，不发布 byte 或 service request；既有 HTTP
408 response 仍仅在可行时发送。即使 `CancelIoEx` 报告 completion 已先发生，取消也必须通过
`GetOverlappedResult` 观察完成，之后才能重用或释放 operation storage。OS cleanup 不保证硬实时。
Quiescence 后若 disconnect 出现未知错误，server 会关闭并永久退役该 pipe；旧 peer 无法获得新
budget。后续 call 在访问 prefix 前拒绝，destroy 不会再次关闭 retired handle。
该生命周期不会重放 request 或 response。

Windows server write 只有在 client 已消费所写 pipe byte 后才报告成功。
Helper 在 flush 成功后对同一个 overlapped pipe 调用零字节 `PeekNamedPipe`。
如果 client 在 flush dispatch 前关闭，Windows 可能丢弃未读 response 却仍返回 flush 成功；
pipe 检查会拒绝该断开状态，同时不消费 request byte。检查失败沿用既有 closed/system-error
处理，不发布成功 byte count。
Write、helper creation 与 flush wait 共用从 write-call 入口计时的 timeout；expiration 不会发布
部分成功 byte count。Flush helper 将阻塞 pipe I/O 与 deadline owner 分离。Timeout 先断开同一个
owned pipe，再 join helper，包括 helper 尚未进入 flush call 的情况。在 join 完成前，listener 与
helper context 始终保留所有权，accept、destruction 与 handle reuse 因而不会和 pending I/O 竞争。
当 disconnect 成功或 pipe 已断开时，deadline 限制 peer waiting；它不保证 OS cleanup 的硬实时完成。
未知 disconnect failure 会在 helper retirement 后返回 system error，随后关闭并永久退役该 listener。
OS cleanup failure 不保证完成时间。不增加 response acknowledgement、replay 或 wire-format change。

## 正确性证据

| 声明                                             | 契约           | 形式化或生成式证据                                                                                                                                             | 确定性或真实 refinement                                                                                                                                   | 边界                                                                                                                  |
| ------------------------------------------------ | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| 只允许显式且已授权的 catalog                     | 不变量 3、10   | N/A：execution-ledger abstraction 从 catalog 与 service admission 之后开始                                                                                     | Catalog/service test 覆盖负向 admission 与精确十三项 composition；HTTP integration 调用全部 Workbench read 及精确 target 的 Terminal 创建                 | 仅十三项已接受 operation                                                                                              |
| Request/execution 映射与 identity 保持唯一       | 不变量 1-4     | `RequestAndExecutionMappingsAgree`、`RetainedRequestHasOneHandlerStart`、`ExecutionIdentityIsProcessUnique` 与 `ActiveExecutionCapacity`                       | Ledger boundary test、淘汰后 duplicate execution identity test 与固定 seed 的 stateful command property                                                   | Request replay 在淘汰后结束；execution identity uniqueness 在 process restart 后结束                                  |
| Cancellation 与 retention 安全                   | 不变量 5-8     | `CancellationForwardedAtMostOnce`、`CancelCommittedDoesNotForward`、`CommittedExecutionNeverCancels` 与两个 TLC variant 的 count/weight retention invariant    | Deferred-handler、commit 前后 cancellation、duplicate request、receipt eviction 及精确 capacity test                                                      | 不声明 handler liveness 或 durable storage 成功后的 rollback                                                          |
| Agent wait 匹配 canonical state                  | 不变量 3、10   | `AgentSemanticWait.tla`：exact subscription、eligibility、freshness、output 与 revision invariant                                                              | Agent domain 与 Desktop operation test 覆盖 pagination、exact read、status explanation、race-safe recheck、timeout、cancellation 与 future baseline       | 仅有界 volatile runtime；不声明 restart continuity 或 provider truth                                                  |
| Agent change 暴露每个 retained revision 或 reset | 不变量 3、10   | `AgentSemanticChanges.tla`：ordered suffix、batch、subscription、`WaitingCursorIsCurrent`、gap、reset、timeout、cancellation 与 retirement invariant           | Agent journal 与 Desktop operation test 覆盖 applied-only record、page order、signed cursor、synchronous recheck、overflow/reset、timeout 与 cancellation | Retention 为内存中 512 条；不声明 durable replay、cryptographic proof 或 restart continuity                           |
| Terminal output 精确、有界且仅 capture           | 不变量 3、9-10 | `TerminalSemanticRead.tla`：capture-only lease、exact success fence、无文本 failure、cleanup attempt/disposal 区分、cleanup failure 保留与单一 terminal result | Reader 与 operation test 覆盖 live/archived routing、HTTP invocation、UTF-8 capacity、wrapping、staleness、retirement、deadline、cancellation 与 cleanup  | 仅 safety：不声明逐 read settlement progress、cleanup deadline/retry、脱敏、raw VT、wait、remote reconnect 或 history |
| Locator 与 peer 保持 current-user                | 不变量 11、13  | Native source 会为维护中的 POSIX 与 Windows family 编译                                                                                                        | Native Node 与 Electron test 会创建、认证、读取、拒绝不安全 POSIX ownership 并 retire endpoint；维护中的 Windows CI 会执行 DACL/SID path                  | 信任受损 same-user process；cross-user 负向执行需要 platform account fixture                                          |
| Framing 与 capacity fail closed                  | 不变量 8、12   | 严格 protocol framing 与 route schema                                                                                                                          | Unit rejection matrix 与真实 Node/Electron local-transport test 覆盖 trailing input、64 KiB request、1 MiB response、malformed input 与恢复               | Kernel queue scheduling 与 process kill 不属于该声明                                                                  |
| Disconnect 不会 cancel 或 replay                 | 不变量 13      | N/A：encoded transport 与 client connection state 不属于 execution-ledger abstraction                                                                          | `verify-semantic-command-local-http-runtime.mjs` disconnect proof；`SemanticCommandClient` 的 read-only polling 与 generation-mismatch test               | 排除跨 machine 或 coordinator restart 的 durable recovery                                                             |

注册的 TLC model 使用两个 request identity、两个 fingerprint、三个 execution identity、一个 active
slot、两个 coordinator generation，以及分别限制 count 和 weight 的 retention variant。这些边界会暴露
replay、conflict、cancellation、completion-order eviction 与 restart interleaving，包括 post-commit
cancellation 与淘汰后的 execution-identity reuse；它们不证明无界状态。Native test 会在当前平台的 Node
与 pinned Electron runtime 下运行，而维护中的 macOS 与 Windows CI matrix 是平台行为 authority。不会运行
任何 manual native-window test。

`verify-semantic-command-endpoint-windows-security.mjs` 将 production Windows adapter 按 warning
即 error 编译。有限 case matrix 覆盖创建、已有 private/broad/null DACL、foreign ownership 与
reparse 拒绝。真实 NTFS 操作验证 current-user directory 和 locator 创建、DACL 修复，以及拒绝
真实 directory junction 且不修改其 target。API boundary fixture 分别覆盖不同于 `TokenUser` 的
default owner 与 foreign-owner security query，验证创建时显式指定 owner，以及拒绝时没有
security write。这些 fixture 不改变 token 或 account，也不声明真实 elevated 或 cross-user 执行；
此类执行仍需要适当的 Windows account fixture。

同一 verifier 还针对真实 named pipe 运行 `tests/windows_accept.c`。受控 connect-before-first-write
case 覆盖 default client flag 与 bundled client 的 identification flag、精确 binary prefix/tail
交付、已认证 read polling timeout 后的延迟 fragment、peer EOF、disconnect 与 successor reuse。
Clock fixture 覆盖剩余 poll budget、发布 admission 前的 connection expiration，以及既有 infinite-timeout
sentinel；identity failure fixture 覆盖 missing/mismatched token 与 impersonation failure。Admission
失败时 byte 不可观察，accept thread 返回时不残留 impersonation。这些有限 cut 细化不变量 11-13，
不增加 trusted identity，也不替代真实 cross-user test。
首字节之后过期的 fixture 仅在精确 preread operation 成功完成一个字节后推进时钟；
发起 pending read 不能在受控 write 之前就使连接过期。

同一 verifier 还运行 `tests/windows_write.c`，以真实 named pipe 验证延迟及分段 response read、
flush 前后两种 deadline 顺序、flush 期间及 dispatch 前的 peer close、helper creation failure、原始 write budget 耗尽、
未知 disconnect failure 后的 listener retirement，以及下一次已认证 connection。受控 OS failure
仅验证错误分类与资源保留，不证明 OS failure 下 cleanup 的活性。

`WindowsSemanticEndpointAccept.tla` model 负责 poll/completion/expiry/cancellation 的顺序声明。
它探索两个后继 peer、0 到 3 的 tick、两 tick 的 connection budget、一个私有 prefix 和一次 body
publication。Native Idle/Connecting/Identity/Authenticated/Closed/Retired 与精确 overlapped completion
映射到模型的 state 和 operation owner；显式 disconnect 映射到 generation change。每个维护 action
都参与 authentication、publication、byte order、closed ownership、storage reuse invariant，以及
poll ownership 和 budget 不续期 property 检查。
Cleanup progress 假设 OS cancellation completion 与 quiescence 公平发生；peer connection 和首 byte
交付没有 fairness 假设。模型不证明 Win32 identity API、OS failure 下 cleanup liveness、无界时间或
端到端 service completion。

`tests/windows_persistent_accept.c` 使用真实 named pipe、受控 clock 和精确 completion event
细化这些 cut：跨 poll 到期的 availability/open、跨多次 preread poll 的首次 write、idle lifetime、
remaining-budget handoff、prefix/body expiry、closed-state refusal、connect/preread 期间 stop、
completion 先发生的 cancellation、successor byte isolation，以及旧 peer handle 仍打开时的
unknown-disconnect retirement。Worker unit test 检查 100/5,000 ms
参数、receipt 限制的 read、terminal admission cleanup 和 stop。真实 Node/Electron runtime 与 CLI
verifier 覆盖 C/addon/TypeScript projection 和成功 status exchange。这些确定性 trace 保留两个 native
failure cut，不依赖没有 gate 的反复运行直至变绿。

Agent-wait model 还包含两个 Session、两个 semantic status、三个 health value、两次 Agent mutation
（revision 0 到 2）与两个 coordinator generation。其 action coverage 包含 begin、missing、ahead、update、removal、timeout、
cancellation 与 retirement path。这些边界可以暴露 subscription 与 decisive-recheck race，但不声明无界
waiter count、time、revision 或 restart recovery。

Agent-changes model 使用 revision 0 到 4、retention 2、page bound 2 与两个 coordinator generation。
Required action coverage 包含 baseline、current follow、immediate batch、overflow reset、invalid cursor、
update、timeout、cancellation 与 retirement。这些边界会暴露 silent-gap、ordering、batching 与
subscribe-before-recheck 错误；`WaitingCursorIsCurrent` 会独立拒绝落后于新 revision 的 waiting reader。
该模型不声明无界 retention、reader、revision、cryptographic cursor security 或 restart recovery。

Terminal-read model 使用一个 target、两个 target epoch、两个 frame generation 与 revision，以及一个 capture
lease。Required action coverage 包含 opening、unavailable 与 cleanup-failed open、deadline expiry、frame
observation、exact row match、stale row、cancellation、target retirement、成功与 rejected cleanup，以及
result publication。这些边界会暴露 interactive-lease leakage、stale-success、带文本或被覆盖的 cleanup
failure、cleanup attempt 前 settlement 与 duplicate-result 错误。该模型只检查 safety：它没有 fairness 或
temporal settlement property，也不声明 bounded cleanup、无界 row、time、target、Host behavior 或 process
recovery。
