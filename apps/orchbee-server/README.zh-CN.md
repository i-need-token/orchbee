# Rust Orchbee Server

这是实验性且尚未发布的私有 Rust Server application。它拥有内存 Workbench projection
kernel、第一条 POSIX 本机 Server-Worker process spine 及 serialized post-admission OBCN carrier，
以及增量的本机 Human Client/Client Session admission spine。产出的 executable 名为
`orchbee-server`；它参与根 Cargo verification gate，但不被公开 `orchbee`、Desktop、packaging、
installer 或 service manager 消费。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## Workbench projection store

`WorkbenchProjectionStore` 通过 `orchbee-workbench-protocol` 载入一个 encoded version-1
`workbench.snapshot`。一次 read 解析一个 encoded `workbench.snapshot.read` intent，并返回
in-memory `SnapshotReadResult`，其中包含确切 request ID 与完整已校验 snapshot 的 clone。

无效、过大、semantic 不一致或 document kind 错误的输入通过现有 `ContractError` 边界失败。
Read boundary 在 JSON decode 前执行 16 KiB intent cap；snapshot parser 同样先执行 1 MiB
projection cap。Read 永远不会改变 retained state。该 result 是尚未发布的 application type，
而不是新的 wire response 或稳定 cross-application API。

## In-memory advancement 与 publication

`advance_projection` 接受一份内部提供的完整 encoded snapshot。它要求 Site 相同且 revision
严格递增，然后派生 deterministic aggregate diff：可选 navigation replacement 在前，changed
Terminal Context 按 canonical identity 排序。当 1,024-change 与 1 MiB encoded limit 均满足时，
它返回 typed `WorkbenchPublication::Delta`。如果任一 delta limit 会被超过，则改为返回完整的
已校验 snapshot。没有可见 projection change 时，可以用 empty delta 推进 revision。

Store 会在改变 retained state 前，把 selected publication 序列化到 contract bound 内。Selected
delta 还必须能够应用到 previous snapshot，并准确重现 candidate。Parsing、Site、revision、
refinement 或 serialization failure 会保留 previous snapshot 不变。Store 不接受 Client delta 或
semantic mutation intent，也没有 external caller 或 publication adapter。

## 本机 Worker service

在 macOS 与 Linux 上，私有开发调用
`orchbee-server --local-endpoint <absolute-worker-path>` 会绑定一个 Unix-domain Worker socket，
且不删除既有 path。增量形式在其后追加 `--client-endpoint <absolute-client-path>`，从而绑定独立
Client socket。选中的 socket 都被限制为 `0600` mode；仅当所有选中 listener 就绪后，Server
才向 stdout 写入 machine readiness marker `ready\n`。任何其他 invocation 继续保持静默，并以
status `1` 退出。这是内部增量进程 contract，不是 public CLI grammar 或 production
service-installation interface。

Server 与 Worker 通过 kernel Unix-socket credential 相互要求 peer 的 effective operating-system
user。Credential mismatch 或 handshake transport-setup failure 会关闭 candidate，不返回 protocol
response。一个 listener 同时最多准入一个 compatible Worker，并分配严格递增的 process-local
generation。Duplicate、malformed、timed-out、wrong-message、protocol-incompatible 或
cohort-incompatible attempt 都不会改变 active Worker。虽然 OBLS 把 Worker identity decode 为 opaque
16-byte value，Server 仍会在 registry admission 或 `WorkerAccepted` 之前把全零值拒绝为
`MalformedFrame`：该值无法映射到 OBCN 的 nonzero Worker identity，且 rejection 不消耗 generation。
Handshake 在有限 timeout 下同步读取。Server 完整写出 `WorkerAccepted` 后，会把同一个 unbuffered
stream 交给恰好一个 generation-scoped OBCN wire owner，并通过 Worker registry 暴露 cloneable
request handle。由该 owner 而非 caller 或 registry 序列化每次 write/read exchange；wire I/O 期间
不会持有 registry lock。

Handle 会在触碰 wire 前拒绝指向其他 Worker 或 generation 的 request。除了可能正在 in-flight 的一条
request，它最多再准入一条 submitted request；更多 submission 会立即收到 backpressure。
`Unavailable` 与 `Backpressure` 均表示 addressed request 没有跨过 complete-write boundary。Owner
会先完整写出一条 OBCN Launch 或 Start request，再读取恰好一条 result。它只接受匹配的 result
direction 与 kind、operation ID、完整 authorization echo，以及 Start 的 target identity。
Malformed、mismatched、timed-out 或 disconnected exchange 会退役该 Worker generation，并使
queued work 变为 unavailable。

Attached carrier 会在 wire owner 的整个 lifetime 内保持为 active registry entry。唯一可调用的
admission cancellation 会拒绝 attached carrier；request handle 与 registry 都不暴露独立的 active-
generation deactivation。因此，只要旧 owner 仍可能 write、read、validate 或 publish exact result，
就不能准入 replacement。Owner settlement 会 gate retirement：无论 normal exit、completion receiver
消失、thread spawn failure 还是 panic unwind，owner destruction 都会先把 handle 标为 inactive，根据
complete-write marker 保守结算任何 in-flight request，并 drain submitted slot；之后，其 RAII
registration guard 才移除 matching generation。该 removal 是允许准入 replacement 的 retirement
linearization point。Stale guard 无法移除 replacement。

每次完整 request write 和完整 result read 都有各自两秒的 monotonic deadline；partial progress
不会重置 deadline。Request frame 完整写出前的 failure 会被报告为 `NotDelivered`，且为 known
unchanged：strict Worker decoding 无法 dispatch incomplete frame，owner 还会退役该 connection。
完整写出后，任何 failure 都是不同类型的 `PostDeliveryOutcomeUnknown` carrier error：Server 不会
伪造 semantic OBCN `OutcomeUnknown` value，也不会 retry request。来自 Worker 的确切 result
（包括 Worker 报告的 semantic unknown outcome）会原样保留；carrier 不会依据 capability-version
或 outcome variant 分支。没有 outstanding request 时收到 byte 属于 invalid，会退役 connection。
Drop 整个 registry 会请求私有 shutdown，但无法准入 replacement；owner 会在 25-millisecond idle
poll 或 bounded current exchange 内观察到它，后者最多包含剩余 write deadline 加一段 result-read
deadline。不存在无界 connection thread 或 message queue。

Endpoint path 由受控开发 witness 提供。Discovery、stale-endpoint recovery、default runtime
directory、跨 candidate path 的 singleton activation、daemonization、Windows named pipe、production
service supervision 与 shutdown signaling 都不属于本切片。进程被突然终止或 bind 后 setup
失败时，可能留下由外部 owner 负责移除的 socket path。

选中 Client listener 时，bind 或 listener-thread spawn failure 会在 readiness 前发生，并使 Server
startup 失败。Readiness 后发生的意外 Client listener accept failure 会结束该 listener，但尚不与
Worker loop 协调；production fault supervision 与 restart 仍属保留事项。

私有 [`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.zh-CN.md) crate 拥有
Worker framing、handshake、cohort rejection 与 bound；
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.zh-CN.md) 拥有 kernel
same-user proof。私有
[`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.zh-CN.md) crate 拥有
OBCN semantic value 与 frame encoding。Server 拥有 listener lifetime、Worker admission state、
generation、bounded submission、serialized carrier validation 与 generation-matched cleanup。
[`orchbee-worker`](../orchbee-worker/README.zh-CN.md) application 拥有连接进程 lifecycle；其当前
runtime 尚未 end-to-end 消费 post-admission request。

## 本机 Client Session service

选择私有 Client endpoint 后，每条 same-user compatible Human Client connection 都会创建一个独立
process-local Client Session generation。TUI、Web、Desktop、mobile、tab、window 与 renderer
identity 被有意排除：该 endpoint 准入的是统一的 peer Client model。Server 最多接受八个 active
Session，并最多保留十六个 candidate/session handler thread。每条 handshake 有两秒 read/write
timeout。因此，只要还有 handler slot，一个 silent candidate 就不能阻塞另一条 connection；饱和
时，多余 candidate 会被关闭，且不会分配更多 thread。Handler slot 与 admitted-Session
registration 都由 lifetime-owned guard 持有，因此正常完成、fault 与 unwind 都会释放两类上限。

经过认证的 candidate 会因 protocol/cohort 不兼容、malformed 或 wrong message、timeout 或
active-Session capacity 收到 typed rejection。Server 完整写出 `ClientAccepted` 后才移除 handshake
timeout，并在同一条 unbuffered stream 上读取 strict OBCS。它每次读取一条 request，构造并尝试完整
写出相关联的 denial；只有写出完成才读取下一条。对于每个成功 decode 的 Create-immediate、
Create-deferred 与 Start request，构造出的 result 都具有精确 result kind 与 `requestId`，并使用
`rejected/not-authorized`；request field 与 Worker presence 不会改变该决策。EOF、malformed、
wrong-direction、oversized、truncated、read 或 write failure 只会关闭该 Session。Disconnect cleanup
匹配确切 generation，不能移除另一个或 replacement Session。它不会停止 Server、干扰独立 Worker
registry，也不意味着任何 terminal action。Server process loss 会关闭每条 Client liveness
attachment。

私有 [`orchbee-local-client-protocol`](../../crates/local-client-protocol/README.zh-CN.md) crate
拥有 Client framing 与真实 Rust connecting adapter。独立
[`orchbee-client-server-protocol`](../../crates/client-server-protocol/README.zh-CN.md) binding 拥有
OBCS validation 与 frame I/O。Server 拥有 listener、handler-slot、admission、sequential denial、
generation 与 cleanup state。公开 `orchbee` 尚未调用 adapter，因此 bare、help 与 version 行为
保持不变。本切片不增加 Workbench command、successful terminal result、terminal relay、TUI、
activation、endpoint discovery、persistence 或 remote ingress。

## 本机 Worker process-spine 建模记录

| 字段   | 现行 contract                                                                                                                                                                                                                                                                                            |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 已验证原生 artifact 之间没有真实进程关系。本 Worker 子骨架必须证明独立 Server 可以准入并保留一个独立、same-user 且 compatible 的 Worker，同时不创建 terminal、activation policy 或 durable state。                                                                                                       |
| 输入   | 不可信私有 argument vector、开发 owner 提供的 absolute endpoint path、kernel peer credential 与一条有界 framed Worker handshake。Absence、malformed byte、timeout、wrong message、wrong user、version mismatch、cohort mismatch、duplicate Worker 与 disconnect 均显式表示。                             |
| 数据   | Server phase、可选 active Worker identity 加 nonzero process-local generation、next generation 与一条 candidate connection。Server 是唯一 writer。所有 value 都不跨 process exit 保留。                                                                                                                  |
| 输出   | Bind 成功发出一条 `ready\n` marker。Worker admission 返回其 generation；当 stream 允许时，经过认证的 protocol-level rejection 返回 typed bounded reason。Peer-authentication 与 transport-setup failure 会静默关闭。Process setup 或 listener failure 以 status `1` 退出，不产生 human diagnostic。      |
| 不变量 | 一个 endpoint 最多一个 listener；一个 Server 最多一个 active Worker；active generation 非零且严格递增；rejection 不能替换 active authority；generation-scoped cleanup 不能移除 replacement；Worker disconnect 不停止 Server；Server loss 关闭 Worker connection。                                        |
| 决策   | Invocation validity、peer-user equality、frame validity、protocol/cohort equality、预期 Worker message kind、active-Worker absence 与 matching cleanup generation 完全决定结果。Current directory、Project、Environment、Worktree、Agent 与 Terminal fact 均不参与。                                     |
| 模型   | Deterministic Worker registry 拥有 admission 与 generation-scoped disconnect。薄同步 Unix-socket workflow 负责 authenticate、decode、admit 或 reject、acknowledge，并 monitor accepted connection。                                                                                                      |
| 转换   | `absent -> listening`；candidate `none -> pending -> admitted/rejected/timed-out`；Worker `none -> active(generation) -> none`；Server `listening -> stopped`。第二次 bind 被拒绝，stale cleanup 对其他 generation 是幂等 no-op。                                                                        |
| 依赖   | Rust standard-library process、filesystem、Unix socket、timeout、thread 与 I/O，加私有 local-site protocol 和 kernel peer-credential crate。没有 terminal、child process、network socket、async runtime、persistence、environment lookup 或 randomness。                                                 |
| 验证   | Pure codec 与 registry test；macOS/Linux 原生 same-user socket-pair test；覆盖 bind、readiness、admission、duplicate/incompatible rejection、disconnect 后 replacement、Server loss、malformed peer、确切 output 与 cleanup bound 的真实 Server/Worker process test；注册的 TLA+ safety/progress check。 |
| 限制   | 本 Worker evidence 覆盖同一 effective user 下的 macOS 与 Linux。它不证明 Windows behavior、cross-user negative execution、production endpoint discovery、crash cleanup、service activation、Client Session semantic、terminal retention 或 remote security。                                             |

已检查的 [`LocalSiteProcessSpine`](formal/tla/LocalSiteProcessSpine.tla) model 探索两个 Worker
identity 与三个 generation。其 abstraction 把 candidate action 映射到 handshake admission，
把 `activeWorker` 与 `activeGeneration` 映射到 registry，把 stale cleanup 映射到
generation-scoped disconnect，并把 Server stop 映射到 transport loss。它只证明 bounded model；
deterministic 与 process test 负责 implementation refinement。

## 本机 Worker OBCN carrier 模型与 runtime refinement 记录

这份 model-first 记录固定了完整写出 accepted OBLS response 后最小的 Server-owned carrier
contract。该 model 是已接受的 refinement target；上面的 service implementation 提供对应 runtime
evidence，但不规定 OBCS business policy。

| 字段   | 已接受的 refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | OBLS admission 后，Server 必须在有界 OBCN Launch/Start exchange 中复用该精确 Worker generation 的 stream，且不能让 registry、request submitter 或其他 task 成为第二个 wire owner。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| 输入   | 一个 current Worker identity 与 process-local generation；三条合法 request candidate，其中两条是 operation ID 不同但 kind 相同的 Launch，另一条是 Start；active 或 inactive 时的 stale-handle submission；complete-frame write 或未完成 write；有限 monotonic per-I/O deadline tick；disconnect；以及一个真实 incoming response value。Response domain 携带 direction、result kind、Terminal ID、operation ID、Worker ID/generation、fence、lease ID/expiry、capability、Start target 和精确 outcome payload，包括两个不同的 definite payload，以及 Launch `OutcomeUnknown` 中的 Host epoch。                                                                                                                        |
| 数据   | Model 保留一个 `activeRef`、一个 generation-scoped `requestHandleRef`、一个 scalar `wireOwnerRef`、一个 capacity-one `submittedSlot`、最多一个 `wireRequest`、其 `wirePhase`、`now` 与 `wireExpiry`、environment-owned `incomingResponse`，以及 attempt/completion/publication audit state。已检查 configuration 使用两个 Worker identity、两个 generation、两条 Launch operation 与一条 Start operation。Submitted slot 可以与一条 wire request 共存，但只有后者能处于 in-flight。                                                                                                                                                                                                                               |
| 输出   | Complete request-frame write 前的 rejection 为 `known-unchanged`；exact OBCN result 保留经过验证的真实 incoming value 及 payload，包括 Worker `OutcomeUnknown`；complete write 后的 timeout、disconnect 或 invalidity 变成 carrier-level `effect-unknown`。后者不是伪造的 OBCN result。Retirement 把 queued slot drain 为 known unchanged/unavailable；inactive stale handle 无 write 即 unavailable；任何 class 都不会引发 automatic retry。                                                                                                                                                                                                                                                                     |
| 不变量 | 最多存在一个 current generation 与一个 stream owner；registry 只存 request handle，绝不存 stream；submitted slot 与 wire slot 各自以一为上界；每条 operation 最多有一次 complete write；deadline clock 单调；exact publication 必须等于此前 observed response，且其 direction、kind、operation、每个 authorization field、Start target 与 outcome payload 都匹配；每种 mismatch class 与 invalid unknown payload 都不能 exact publish；unknown effect 不可 retry；replacement 等待旧 completion 与 publication settle；stale activation 与 generation-matched cleanup 不能替换或移除 current generation。                                                                                                         |
| 决策   | Wrong-Worker 与 wrong-generation routing 在 enqueue 前 reject，且不 retire owner。Stale handle（包括没有 active generation 时）返回 known unchanged/unavailable。Capacity 是一条 submitted request 加一条 wire request；新鲜的第三条合法 request 记录 backpressure 且不改变两个 occupied slot。`CompleteRequestWrite` 是 ambiguity boundary，并在 request-write deadline 后安装新的 result-read deadline。Local time advancement 与 expiry 让 silent peer settle；response 与 disconnect arrival 没有 fairness assumption。`RetireCarrier` 关闭 generation gate：之后不能继续 write 或新观察 exact response；若 exact result 先被观察则仍可 publish。Replacement 要求每个旧 completion 与 publication 已 settle。 |
| 模型   | [`LocalWorkerObcnCarrier`](formal/tla/LocalWorkerObcnCarrier.tla) 分离 registry identity、cloneable request-handle authority、queue occupancy、sole wire ownership、write completion、actual response arrival、local validation、deadline expiry、publication、retirement 与 cleanup。`ReceiveExactWorkerResult` 验证并保留 `incomingResponse`，不会构造一个 matching result。该模型细化 post-OBLS boundary 与 OBCN semantic contract，而不重新实现任一 byte codec。                                                                                                                                                                                                                                              |
| 转换   | `ActivateCarrier` 发布一个 generation；`SubmitFirstLaunch`、`SubmitSecondLaunch` 与 `SubmitStart` 覆盖不同 candidate；`DequeueRequest` 将一个 value 移给 owner；`CompleteRequestWrite` 跨过 ambiguity boundary；`AdvanceMonotonicTime` 使 write 前后 expiry 生效；exact arrival 与针对每种 mismatch class 的具名 arrival action 填充 `incomingResponse`；`ReceiveExactWorkerResult` 或 `RejectMismatchedWorkerResult` 验证它；disconnect action 覆盖 idle、before-write、after-write、queued-after-write 与 after-result ordering；publication、late-result discard、replacement、stale rejection 与 generation-matched cleanup 完成 retained state。                                                             |
| 依赖   | Target 将现有 Server Worker registry 和 post-`WorkerAccepted` stream 与私有 local-site/capability-node protocol contract 组合起来。Local-site protocol 拥有 OBLS framing，capability-node protocol 拥有 OBCN value 与 strict framing，carrier 负责组合二者。`AllAttemptsSettled` 是 carrier 对 `TerminalLaunchAuthorization` 中 pending-result replacement guard 的对应表达。OBCS mapping、authorization decision、Host dispatch、terminal retention、persistence、endpoint discovery 与 remote ingress 均不参与。                                                                                                                                                                                                |
| 所有权 | Server registry 拥有 current Worker generation 与 request-handle publication。一个等价于 `WorkerWireOwner` 的 owner 排他拥有 stream read/write 与 queue draining；request-handle clone 只能 submit。Codec 拥有 byte validity，Worker 拥有 semantic OBCN result，后续 Server policy owner（而非本 carrier）拥有 authorization、reconciliation，或对另一项已证明 known-unchanged operation 的任何 permitted retry。                                                                                                                                                                                                                                                                                                 |
| 验证   | 已注册的 `local-worker-obcn-carrier` model 在 `response-validation`、`disconnect-order`、`stale-routing` 与 `capacity` variant 中要求 47 个 semantic action 都具有非零 TLC coverage。这些 variant 覆盖两条 same-kind correlation、每个 response field/payload mismatch、write 两侧的 deadline expiry、每种 disconnect ordering、active 与 inactive stale handle、replacement gating、cleanup，以及 submitted slot 与 in-flight wire request 都占用时的一条新鲜第三 request。它检查 safety invariant 以及 admitted attempt、pending publication 与 cleanup 的 eventual settlement；weak fairness 只施加于 local dequeue、time、expiry、validation、publication 与 cleanup action。                                 |
| 限制   | 这是 bounded formal evidence，而不是 runtime-refinement evidence。它把 complete-frame write 抽象成一个 boundary，并使用 one-tick finite write/read deadline 而非 concrete duration；它不证明 partial-I/O code、system clock implementation、thread scheduling、panic cleanup、OS behavior、codec correctness、OBCS-to-OBCN policy、Host effect、durable recovery，或其他 authorized owner 是否应 retry 一条 known-unchanged request。两个串行 phase deadline 对应最多两倍 configured per-I/O timeout。                                                                                                                                                                                                            |

已实现的 runtime refinement 把 `WorkerRequestHandle` 映射到 `requestHandleRef` 加
`submittedSlot`，并把唯一 `WorkerWireOwner` 映射到 `wireOwnerRef`、`wireRequest`、`wirePhase`、
`now` 与 `wireExpiry`。这个 scalar expiry 按 phase 解释：`DequeueRequest` 启动 total write
deadline，`CompleteRequestWrite` 用 total read deadline 替换它。`WrongWorker` 与
`WrongGeneration` 绝不进入 `submittedSlot`；`Backpressure` 保留两个 occupied slot 不变；
`Unavailable`、`NotDelivered` 与 queued-owner
shutdown 映射为 `known-unchanged`；`PostDeliveryOutcomeUnknown` 仅在
`CompleteRequestWrite` 后映射为 carrier `effect-unknown`。Deactivation 必须与 owner
linearize：complete write 前为 known unchanged，complete write 后为 unknown，而且只有
`ReceiveExactWorkerResult` 在 `RetireCarrier` 前获胜时才能接受 exact result。仅在 exchange
开始前检查 active flag 不足以防止 retirement 后 write 或新接受 exact result。已被观察的 exact
result 仍可在 replacement 前 publish；replacement activation 必须等待该 publication，与
authorization model 的 pending-result guard 一致。Carrier 不得把任何 transport classification
转换为 successful 或 failed OBCN result。

## 本机 Client Session 与 OBCS 建模记录

Rust handler 现在细化下述已检查的 deny-only phase。它只在完整写出 accepted OBLC response 后消费
strict OBCS，但不会因此获得 terminal capability，也不产生 terminal 或 Worker effect。

| 字段   | 现行 runtime 与已检查的 refinement                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 保留已实现的 same-user OBLC admission boundary，并证明最小的下一个 same-stream phase：只有 OBLC accepted frame 已 queued 且 written 后，该精确 generation 才能读取 OBCS。OBLC admission 不等于 terminal capability。                                                                                                                                                                                                                                                                                                         |
| 输入   | Runtime 与 model 消费可选 absolute endpoint、kernel peer credential、在当前每次 I/O 两秒 handshake timeout 下 decode 的有界 OBLC frame，以及 strictly decoded 的完整 Create-immediate、Create-deferred 与 Start request class。Model 还抽象 carrier 已缓冲的一帧或两帧 typed frame；idle、partial-header、partial-payload 与 result-write stall；strict malformed、wrong-direction、oversized、truncated、clean EOF、I/O failure 与 Server loss。OBCS schema、corpus 与 binding 拥有 byte 和 field validity。                |
| 数据   | Runtime 最多拥有八个 active Session generation 与十六个 candidate/session handler。有界模型使用三个 Client slot、最多两个 Session、三个 handler、四个 globally fresh generation，以及两个 request-ID equivalence class。Active Session 与 slow candidate 各占一个 handler。OBCS refinement 为每个 Session 最多保留一个 outstanding request；最多两帧 carrier-buffered frame 是 environment fact，而不是 Server request queue。                                                                                               |
| 输出   | Runtime 与 model 把合法 Create 映射为 `terminal.create.result`，把合法 Start 映射为 `terminal.start.result`，始终使用请求的精确 `requestId` 与 `rejected/not-authorized`；actual write 仍取决于 carrier writability。任何输出都不暴露或创建 Terminal、Worker、Host、generation、operation、lease、fence、shell、executable、environment 或 Workbench authority。                                                                                                                                                             |
| 不变量 | OBCS decode 之前必须已写出 OBLC accepted；每个 active generation 恰好拥有一个 handler；Session 与 handler bound 相互独立；每个 Session 最多有一个 outstanding request/result；malformed input 或 transport loss 只关闭 addressed current generation 并释放其 handler；stale EOF 或 cleanup 不能移除 replacement；Server stop 清除全部 Session；stalled Client work 保留另一个 Client 与 Worker registry；每个 modeled terminal-effect 和 Worker-request effect counter 始终为零。                                            |
| 决策   | 合法 Create 与 Start input 在不查询 Worker presence、exact Worker constraint、`cwd`、Create start mode、geometry 或 Start `terminalCreateRequestId` 的情况下被拒绝。Start correlation `requestId` 与该 reference 不同，并且不会 lookup 该 reference。重复 request ID 会依据 current state 再次决定；不存在 replay lookup、historical conflict detection、cancellation、retry 或 idempotency ledger。                                                                                                                         |
| 模型   | [`LocalClientSessions`](formal/tla/LocalClientSessions.tla) 拥有 OBLC admission、generation 与彼此独立的 Session/handler bound。[`LocalClientObcsSessions`](formal/tla/LocalClientObcsSessions.tla) 把每个 non-closed phase refinement 到一个 active generation 与 handler，从唯一 outstanding correlation record 直接派生 deny result，并保留 generation-scoped cleanup。其 `stream-faults` 与 `correlation` configuration 共享同一个 state machine，但避免将无关的 carrier-pair history 与每种 fault/stall ordering 相乘。 |
| 转换   | Candidate `none -> slow -> released`，或 Session `closed -> OBLC-accepted-queued -> OBCS-idle`；read 可以 stall 或经过 partial header/payload；一条完整 request 变成一个 ready 或 write-stalled result；成功写出 result 后先清除该 request，再重新进入 read。Fault、EOF、I/O failure 或 result-write failure 会关闭 current generation。后续 admission 获得 fresh generation，而 stale event 保留它。Server loss 关闭全部 generation。                                                                                       |
| 依赖   | Runtime 把 Server-owned local Client admission workflow 与 OBLC 及独立 OBCS Rust binding 组合起来；model 组合这些 contract。两者都不执行 Worker lookup、OBCN mapping、Host dispatch、PTY operation、terminal retention、shell selection、product projection、persistence 或 network ingress。                                                                                                                                                                                                                                |
| 所有权 | Server 拥有 Client Session generation、handler accounting、same-stream ordering、deny result construction 与 generation-scoped cleanup。Carrier 拥有 byte arrival、buffering、EOF 与 writability。OBCS 拥有 strict framing 与 message validity。Worker/Host authority 与未来 OBCS-to-OBCN mapping 仍在本 slice 范围外。                                                                                                                                                                                                      |
| 验证   | Deterministic session test 覆盖每种 request class、exact correlation、partial I/O、双帧 ordering、duplicate ID、strict failure class、deterministic write failure 与 unwind 期间的 RAII cleanup。真实 process test 覆盖 coalesced OBLC 加 OBCS、sequential Create/Start、capacity cleanup、post-admission stall recovery 与 Worker non-interference。两个已注册 model 都要求其 semantic action 具有非零 TLC coverage，并检查下述 bounded admission 与 OBCS state space。                                                     |
| 限制   | 这是 bounded safety 与 native process evidence，而不是 successful terminal effect 或 codec reimplementation。Runtime 不增加 OBCS timeout、queue、retry、cancellation、ledger 或 fairness promise；stalled Session 可以永久 stalled。它既不证明 eventual response，也不证明 terminal creation。Client protocol 仍要求在 active Server authority 与 authenticated subject 下为每次 submitted request 使用 fresh `requestId`；repeated ID 会被 statelessly 处理，且不会产生 Server history obligation。                         |

Admission model 使用 `MaxSessions = 2` 与 `MaxHandlers = 3`，因此一个 slow candidate 可以独立于
两个 active Session 占用 spare handler。OBCS model 使用两个不同 request ID；Start 始终以其中一个作为
result-correlation ID，并以另一个作为与其不同的 Create reference。Carrier-pair annotation 只用于证明
duplicate 与 distinct ordering，Server denial decision 从不查询它。Buffered Start 会保留其独立的
Create reference 直至 decode，而 denial 与 result correlation 会忽略该 reference。两个模型都没有
temporal fairness clause。

## 已接受的 Terminal Client authority 建模记录

下一个已接受的 implementation slice 让 Rust Server 成为 relay
`Client -> Server -> Worker -> Terminal Host` path 上 product-level Client Session 与 terminal
authority 的 owner。本记录及其 formal model 描述已接受的 refinement target；它们不声称当前
runtime 或 wire contract 已实现该目标。Client Session 是 Server transport/session identity，viewer
attachment 把该 Session 的某一 generation 与 retained terminal 关联，而 controller 是该 terminal
唯一、同时覆盖 input 与 resize 的 grant。

| 字段   | 已接受的 refinement target                                                                                                                                                                                                                                                                                                                                                                                          |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 多个同级 TUI、Web、Desktop 或 mobile Client Session 必须能够并发查看同一个 retained terminal，同时不能产生多个 input 或 geometry writer。关闭 Client 必须释放其临时 authority，但不能结束 terminal。                                                                                                                                                                                                                |
| 输入   | Server availability；独立生成的 Client Session identity；Worker 与 Host availability；retained 或 lost terminal state；viewer attach、controller acquire、input、resize、Client disconnect 与 generation-scoped cleanup event。建立 Client Session 只依赖存活的 Server。Worker、Host 或 terminal 不可用会阻止 terminal attach 与 control，但不会阻止建立 Server session。                                           |
| 数据   | Server-owned last-known session registry 与每个 Client 的 generation、generation-scoped viewer attachment、每个 terminal 一个可选 combined controller reference，以及 pending cleanup reference。Server 丢失后，registry entry 只是最后一次 Server snapshot，不是 Client transport 仍存活的证据。Effective Client transport 还要求 Server 存活。                                                                    |
| 输出   | Viewer attachment 只在通向 retained terminal 的 path 存活时成功。只有同一个 current controller reference 才能推进 input 与 resize；其他 attached viewer 的尝试会被拒绝，且不改变 terminal input 或 geometry。Disconnected Client 会从 current attachment 中移除；disconnected controller 会立即释放 grant。                                                                                                         |
| 不变量 | 任意数量的 modeled current session 都可以成为 viewer，但 effective input 与 resize authority 必须是同一个 set，并且最多包含一个 generation-scoped controller。Controller 必须是 current viewer。Acquire operation 只能填充 unowned grant，绝不替换另一个 controller。Disconnect 与 stale cleanup 不能终止 retained terminal；旧 generation 的 cleanup 不能撤销 reconnected generation 的新 grant。                  |
| 决策   | Client Session creation 与 reconnection 只要求 Server 可用。Viewer attach 与 controller 使用要求 Server、Worker、Host 以及 retained terminal 均可用。Controller disconnect 会释放 authority；其他 viewer 只能在 grant unowned 后 acquire。确切 acquire/takeover interaction 刻意保持未决。Server 或 Worker loss 可以释放 temporary grant 或冻结其 registry snapshot，但不能转移 grant 或结束 terminal。             |
| 模型   | Server 把每个 Client generation 投影为 modeled terminal 的零或一个 viewer reference，并保存零或一个 combined controller reference。Effective input 与 resize controller 由该 reference 与 path availability 派生。现有 Host resize lease 仍是 lower-level mechanism，并不是 Server product policy。                                                                                                                 |
| 转换   | Session `closed -> open(generation)`，且不依赖 terminal availability；viewer `detached -> attached`；controller `unowned -> owned(current viewer) -> unowned`；Client `open -> disconnected -> reconnected(next generation)`，old-generation cleanup 可发生在 reconnect 前或后；process `up -> lost`；只有 Host loss 能让 terminal `retained -> lost`。不存在 restart 或 automatic controller transfer transition。 |
| 依赖   | 未来 runtime refinement 将依赖 Server-Worker spine、Worker-owned Host broker、Terminal Host attachment/input/resize protocol 与 Client transport。它必须为 viewer 投影 read-only Host attachment，并为 controller 投影严格的 combined input-plus-resize authority，或者证明等价 refinement。本 modeling slice 不引入新的 runtime 或 wire dependency。                                                               |
| 所有权 | Server 拥有 Client Session generation、viewer membership、product-level controller selection、rejection 与 stale cleanup。Worker 仍是唯一 Terminal Host broker。Host 拥有 retained PTY lifetime，并执行其 lower-level attachment capability 与 resize lease。Host 侧 resize owner 在 detach 后的 promotion 只是现有 mechanism；本模型不会把它提升为 Server-level automatic takeover policy。                        |
| 验证   | 注册的 TLA+ model 检查三个 Client slot、每个 slot 两个 generation、有界 input 与 geometry revision、每个 semantic transition 的非零 coverage、safety invariant，以及 Server 保持可用期间的 cleanup progress。因为本 slice 不改变 runtime，所以尚无 runtime refinement test。                                                                                                                                        |
| 限制   | Model 覆盖一个 terminal 与每个 Client slot 最多一个 attachment。它不证明 runtime 或 protocol refinement、byte/queue/backpressure behavior、PTY retention implementation、direct Client-Worker path、restart、durable recovery、automatic 或 forced takeover、collaborative input、controller fairness 或 takeover UX。                                                                                              |

已检查的
[`LocalTerminalClientAuthority`](formal/tla/LocalTerminalClientAuthority.tla) model 把 Client
transport liveness 与 terminal path availability 分离。Server 或 Worker loss 会让 terminal authority
失效，并可以释放 temporary controller reference 或留下 frozen registry snapshot；两者都不会改变
retained terminal。只有 Host loss 会把 terminal 改为 `lost`。其 progress property 保证 Server
存活期间 pending generation cleanup 会被清空，而不保证 authority 会自动重新分配。

## 所有权

该 application 拥有其注入 in-memory snapshot copy 的 projection mechanics、实验性本机 Worker 与
Client Session registry、application-private OBCN Worker carrier，以及 deny-only OBCS phase。它
不是第二个 current product writer：每个成功 decode 的 Client terminal intent 都会获得不产生
effect 的 denial decision，OBCS 不会调用 Worker request handle，没有 Client 或当前 runtime 能提交
projection candidate，也没有 publication 离开该 application。当前 Desktop/Workbench composition
继续作为已交付 Workbench writer，Node
[`apps/server`](../server/README.zh-CN.md) 继续作为既有 Browser ingress 行为的权威。

OBCS-to-OBCN mapping、semantic business transition、authorized 或 successful terminal
processing、terminal relay、product authorization、persistence、recovery、external publication、
endpoint discovery 与 activation、production service lifecycle、Terminal Host authority/runtime
integration、retry 或 reconciliation policy、packaging、migration 与 cutover 都需要后续
implementation slice。

原生 source tree 有两个 root：`src/lib.rs` 是可导入的 projection root，`src/main.rs` 是私有 binary
composition root。Binary 的穷尽 lateral import 方向是 `main -> local_site_service`、
`worker_registry` 与 `worker_obcn_carrier`；`local_site_service -> worker_registry`、
`worker_obcn_carrier` 与 `local_client_service::start`；`worker_registry -> worker_obcn_carrier`；以及
`local_client_service -> client_session_registry` 加其私有 `obcs_session`。Reverse import、cousin
import 与其他所有 lateral 方向均被禁止。Test 可以访问 owner-private state，但不得扩大 production
方向。

`worker_obcn_carrier` 不会被 re-export。其 cloneable request handle 只能通过私有 Worker registry
取得，其 registered wire owner 接收 already admitted stream。只有该 owner 的 lifetime guard 可以在
wire settlement 与 queue drain 后退役 attached generation；registry 只能取消尚未 attached carrier 的
admission。它不得 import OBCS、Client Session、Host、projection、product authorization、effect、
retry 或 recovery policy。把 OBCS request 映射到该 handle 需要后续已接受切片。

`obcs_session` 不会被 re-export。它唯一的入口是一个接收已准入 stream 的 `pub(super)` runner，
唯一的非标准库依赖是 `orchbee-client-server-protocol`。它不得导入 Worker registry、Host、
projection、effect 或 dispatch authority。任何更宽的 visibility 或 dependency（包括 OBCS-to-OBCN
mapping）都需要后续已接受切片。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions,local-client-sessions-obcs,local-site-process-spine,local-terminal-client-authority,local-worker-obcn-carrier
pnpm verify
```
