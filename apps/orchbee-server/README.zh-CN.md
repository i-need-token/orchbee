# Rust Orchbee Server

这是实验性且尚未发布的私有 Rust Server application。它拥有内存 Workbench projection
kernel、第一条 POSIX 本机 Server-Worker process spine 及 serialized post-admission OBCN carrier，
本机 Human Client/Client Session admission，以及内存终端协调器。产出的 executable 名为
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
会先完整写出一条 OBCN Launch、Start 或查看 request，再读取恰好一条 result。它只接受匹配的 result
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

每次完整 request write 有两秒、完整 result read 有二十秒的 monotonic deadline；partial progress
不会重置 deadline。Request frame 完整写出前的 failure 会被报告为 `NotDelivered`，且为 known
unchanged：strict Worker decoding 无法 dispatch incomplete frame，owner 还会退役该 connection。
完整写出后，任何 failure 都是不同类型的 `PostDeliveryOutcomeUnknown` carrier error：Server 不会
伪造 semantic OBCN `OutcomeUnknown` value，也不会 retry request。来自 Worker 的确切 result
（包括 Worker 报告的 semantic unknown outcome）会原样保留；carrier 不解释 launch capability-version
或 lifecycle outcome variant。查看结果还必须匹配附件身份与 action/outcome kind。没有 outstanding request 时收到 byte 属于 invalid，会退役 connection。
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
runtime 已通过 Host broker 消费获授权的 Launch/Start 和只读查看 request。

## 本机 Client Session service

选择私有 Client endpoint 后，每条 same-user compatible Human Client connection 都会创建一个独立
process-local Client Session generation。TUI、Web、Desktop、mobile、tab、window 与 renderer
identity 被有意排除：该 endpoint 准入的是统一的 peer Client model。Server 最多接受八个 active
Session，并最多保留十六个 candidate/session handler thread。每条 handshake 有两秒 read/write
timeout。因此，只要还有 handler slot，一个 silent candidate 就不能阻塞另一条 connection；饱和
时，多余 candidate 会被关闭，且不会分配更多 thread。Handler slot 与 admitted-Session
registration 都由 lifetime-owned guard 持有，因此正常完成、fault 与 unwind 都会释放两类上限。

经过身份验证的 candidate 会收到 protocol/cohort 不兼容、畸形或错误消息、超时、active-Session
容量超限等类型化拒绝。Server 完整写入 `ClientAccepted` 后，移除握手超时，在同一条未缓冲
stream 上读取严格 OBCS。私有服务向其单一本机有效用户主体授予默认 shell 的 Create、Start
及这些终端的查看和显式获取独占 input/resize controller 权限。它读取一个请求、调用 Server 服务、完整写回关联结果，然后才读取该 Session 的下一
个请求。结果在 Client 输出前提交；写入停滞只占用自身 Session handler，不持有协调器或 Worker 锁。
读取在首个字节前没有截止时间；此后整条记录限时两秒。每次完整 Client write（包括大 OBSR
记录）也限时两秒。超时只退役对应 Session。

Create 选择当前本机 Worker，并验证显式精确 Worker 约束。Start 通过 request ID 跨同一主体的
Session 解析成功的 deferred Create。Server 保留精确 Worker 代次和完整 Host 身份。Worker
丢失或更换不能授权重新指派、重新创建或重放。EOF、畸形、错误方向、超限、截断及读写失败
仅关闭对应 Session；按代次清理保留终端状态。

私有 [`orchbee-local-client-protocol`](../../crates/local-client-protocol/README.zh-CN.md) 拥有
Client framing 和连接适配器。独立的
[`orchbee-client-server-protocol`](../../crates/client-server-protocol/README.zh-CN.md) binding
拥有 OBCS 验证及帧 I/O。Server 拥有主体策略、放置、operation/lease/fence 签发、终端绑定状态、
有界提交和关联的语义结果。公开 `orchbee` 尚未调用此路径；input/resize control、TUI 激活、持久化和
生产 ingress 仍在该切片之外。

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
| 输入   | 一个 current Worker identity 与 process-local generation；三条合法 request candidate，其中两条是 operation ID 不同但 kind 相同的 Launch，另一条是 Start；active 或 inactive 时的 stale-handle submission；complete-frame write 或未完成 write；有限 monotonic per-I/O deadline tick；disconnect；以及一个真实 incoming response value。Response domain 携带 direction、result kind、Terminal ID、operation ID、Worker ID/generation、fence、lease ID/expiry、capability、Start target 和精确 outcome payload，包括两个不同的 definite payload，以及 Launch `OutcomeUnknown` 中的 Host epoch。                                                                                                                     |
| 数据   | Model 保留一个 `activeRef`、一个 generation-scoped `requestHandleRef`、一个 scalar `wireOwnerRef`、一个 capacity-one `submittedSlot`、最多一个 `wireRequest`、其 `wirePhase`、`now` 与 `wireExpiry`、environment-owned `incomingResponse`，以及 attempt/completion/publication audit state。已检查 configuration 使用两个 Worker identity、两个 generation、两条 Launch operation 与一条 Start operation。Submitted slot 可以与一条 wire request 共存，但只有后者能处于 in-flight。                                                                                                                                                                                                                               |
| 输出   | Complete request-frame write 前的 rejection 为 `known-unchanged`；exact OBCN result 保留经过验证的真实 incoming value 及 payload，包括 Worker `OutcomeUnknown`；complete write 后的 timeout、disconnect 或 invalidity 变成 carrier-level `effect-unknown`。后者不是伪造的 OBCN result。Retirement 把 queued slot drain 为 known unchanged/unavailable；inactive stale handle 无 write 即 unavailable；任何 class 都不会引发 automatic retry。                                                                                                                                                                                                                                                                     |
| 不变量 | 最多存在一个 current generation 与一个 stream owner；registry 只存 request handle，绝不存 stream；submitted slot 与 wire slot 各自以一为上界；每条 operation 最多有一次 complete write；deadline clock 单调；exact publication 必须等于此前 observed response，且其 direction、kind、operation、每个 authorization field、Start target 与 outcome payload 都匹配；每种 mismatch class 与 invalid unknown payload 都不能 exact publish；unknown effect 不可 retry；replacement 等待旧 completion 与 publication settle；stale activation 与 generation-matched cleanup 不能替换或移除 current generation。                                                                                                         |
| 决策   | Wrong-Worker 与 wrong-generation routing 在 enqueue 前 reject，且不 retire owner。Stale handle（包括没有 active generation 时）返回 known unchanged/unavailable。Capacity 是一条 submitted request 加一条 wire request；新鲜的第三条合法 request 记录 backpressure 且不改变两个 occupied slot。`CompleteRequestWrite` 是 ambiguity boundary，并在 request-write deadline 后安装新的 result-read deadline。Local time advancement 与 expiry 让 silent peer settle；response 与 disconnect arrival 没有 fairness assumption。`RetireCarrier` 关闭 generation gate：之后不能继续 write 或新观察 exact response；若 exact result 先被观察则仍可 publish。Replacement 要求每个旧 completion 与 publication 已 settle。 |
| 模型   | [`LocalWorkerObcnCarrier`](formal/tla/LocalWorkerObcnCarrier.tla) 分离 registry identity、cloneable request-handle authority、queue occupancy、sole wire ownership、write completion、actual response arrival、local validation、deadline expiry、publication、retirement 与 cleanup。`ReceiveExactWorkerResult` 验证并保留 `incomingResponse`，不会构造一个 matching result。该模型细化 post-OBLS boundary 与 OBCN semantic contract，而不重新实现任一 byte codec。                                                                                                                                                                                                                                              |
| 转换   | `ActivateCarrier` 发布一个 generation；`SubmitFirstLaunch`、`SubmitSecondLaunch` 与 `SubmitStart` 覆盖不同 candidate；`DequeueRequest` 将一个 value 移给 owner；`CompleteRequestWrite` 跨过 ambiguity boundary；`AdvanceMonotonicTime` 使 write 前后 expiry 生效；exact arrival 与针对每种 mismatch class 的具名 arrival action 填充 `incomingResponse`；`ReceiveExactWorkerResult` 或 `RejectMismatchedWorkerResult` 验证它；disconnect action 覆盖 idle、before-write、after-write、queued-after-write 与 after-result ordering；publication、late-result discard、replacement、stale rejection 与 generation-matched cleanup 完成 retained state。                                                             |
| 依赖   | Target 将现有 Server Worker registry 和 post-`WorkerAccepted` stream 与私有 local-site/capability-node protocol contract 组合起来。Local-site protocol 拥有 OBLS framing，capability-node protocol 拥有 OBCN value 与 strict framing，carrier 负责组合二者。`AllAttemptsSettled` 是 carrier 对 `TerminalLaunchAuthorization` 中 pending-result replacement guard 的对应表达。OBCS mapping、authorization decision、Host dispatch、terminal retention、persistence、endpoint discovery 与 remote ingress 均不参与。                                                                                                                                                                                                |
| 所有权 | Server registry 拥有 current Worker generation 与 request-handle publication。一个等价于 `WorkerWireOwner` 的 owner 排他拥有 stream read/write 与 queue draining；request-handle clone 只能 submit。Codec 拥有 byte validity，Worker 拥有 semantic OBCN result，后续 Server policy owner（而非本 carrier）拥有 authorization、reconciliation，或对另一项已证明 known-unchanged operation 的任何 permitted retry。                                                                                                                                                                                                                                                                                                 |
| 验证   | 已注册的 `local-worker-obcn-carrier` model 在 `response-validation`、`disconnect-order`、`stale-routing` 与 `capacity` variant 中要求 47 个 semantic action 都具有非零 TLC coverage。这些 variant 覆盖两条 same-kind correlation、每个 response field/payload mismatch、write 两侧的 deadline expiry、每种 disconnect ordering、active 与 inactive stale handle、replacement gating、cleanup，以及 submitted slot 与 in-flight wire request 都占用时的一条新鲜第三 request。它检查 safety invariant 以及 admitted attempt、pending publication 与 cleanup 的 eventual settlement；weak fairness 只施加于 local dequeue、time、expiry、validation、publication 与 cleanup action。                                 |
| 限制   | 这是 bounded formal evidence，而不是 runtime-refinement evidence。它把 complete-frame write 抽象成一个 boundary，并使用 one-tick finite write/read deadline 而非 concrete duration；它不证明 partial-I/O code、system clock implementation、thread scheduling、panic cleanup、OS behavior、codec correctness、OBCS-to-OBCN policy、Host effect、durable recovery，或其他 authorized owner 是否应 retry 一条 known-unchanged request。两个串行 phase deadline 对应 configured write timeout 加 response timeout。                                                                                                                                                                                                  |

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

`LocalClientSessions` 拥有有界准入、handler 计数、代次清理和 Server 丢失。
`LocalClientObcsSessions` 使用注入的拒绝 handler，检查同一 stream 的 framing 及顺序结果发布。
该 handler 是显式测试实例，不是服务的终端授权策略。其无终端副作用和 not-authorized 不变量
仅适用于这个实例。Runtime `obcs_session` 现在通过 callback 委托语义处理；单元测试保留该实例，
独立于终端副作用检查关联、重复 carrier record、部分 I/O、失败，以及完整写入后才能读取的顺序。

Session 模型探索三个 Client、两个 active Session、三个 handler 和有界代次。OBCS 的
stream-fault 和 correlation 配置包含合并的握手/请求、部分帧、输出停滞、错误方向、重复及
不同请求。这两个模型都不证明终端授权、Worker 派发、取消、重放、持久化或实时 relay。
下文的协调器模型和进程测试拥有新增副作用边界。

Carrier model configuration 仅实例化 Launch/Start。查看／控制附件 scope/outcome 校验和内部消费结果的
cleanup 复用其有界 queue 与 owner lifetime，具体 refinement 证据由 socket test 提供。Viewer cleanup
没有外部 completion receiver；经过校验的结算就是其 publication，因此成功会保持共享 carrier
存活。Controller cleanup 保留内部 completion receiver，直到精确 grant 释放或其 Worker 退役。只读投影路径另行映射到 `LocalTerminalRelay`；两个模型均不证明呈现解码或 controller
运行时行为。

## 本机 Client 查看附件转发细化

已接受的本地终端里程碑现在把 Server Client Session 接入 Worker 现有的只读查看附件循环。
暂存的原生 LocalClient adapter 消费查看协议。进程见证证明两个独立准入的 Client 经 Server 和
Worker 接收真实 Host projection。下文的 controller 细化增加 input/resize；TUI 激活仍是独立边界。

每个已准入 Session 最多拥有一个临时查看附件。Attach 引用同一本机 subject 下已接受的终端
Create；Server 解析其保留的 Host identity 与精确当前 Worker/generation，分配全新的 request、
attachment 和 cleanup identity，并固定该 Worker handle。Client 绝不提供权威 Worker、Host 或
attachment identity。每次 poll 移动一条完整不透明 Host record；Server 校验 envelope、route 与
精确 feedback scope，不解码终端呈现。Frame delivery 与 feedback 不能跨 Session slot。
Attach 在 Client publication 前提交。Disconnect、publication 失败或显式 detach 释放查看
附件及其拥有的 controller，绝不终止或重新指向保留终端。不重放 unknown exchange。重新附着获得全新 attachment
identity，并要求 snapshot。

Coordinator 或 registry lock 均不跨越 Worker/Client I/O。Drop cleanup 阻塞准入现有的单项
Worker request queue，不等待 Client response，也不增加无界队列。退役 handler 保留原有的
有界 Client Session/connection slot，直到 cleanup 准入完成。Wire owner 在内部观察完成。
Carrier 停止会关闭 broker 及其全部临时附件。Worker response 预算为二十秒，以容纳最多十六个
待处理 Host 查看／控制 detach、两个 Attach exchange 和 result delivery；control write 仍保持两秒边界。
一个排队请求加一个在途请求的边界要求四十六秒 launch lease。Client record read 在首字节之前
允许 idle，之后使用总计两秒 deadline；Client write 同样有总计两秒 deadline，超时只退役该
Session。

现有 `LocalTerminalRelay` 仍是 attachment scope、单帧在途、精确反馈、队列隔离与 detach
生命周期的权威模型。本切片将这些转换映射到 OBCS viewer control、OBSR 不透明 record、
现有 OBCN/OBNR carrier 与 Session 所有的 cleanup。证据包括严格协议 fixture、确定性的过期
作用域／反馈测试、carrier cleanup/backpressure 测试，以及真实双 Client/Server/Worker/Host
进程轨迹。它们证明私有只读路径，不证明 TUI 渲染、controller authority、发布性能预算或独立
审查证据。

## 本机终端 controller 细化

已接受里程碑为 Session 查看附件增加显式获取操作，每个保留 Host 终端只有一个输入与调整大小
共用的 controller。Server 在 Worker I/O 前预留 grant；该 grant 待定、存活或清理期间，其他
Session 不能获取。Server 新签发的 controller 身份限定 input 与 resize 共用的严格递增 sequence。
Client 提供的身份必须等于其 Session grant。非 controller 与过时 sequence 的拒绝不派发请求；
有效提交的 sequence 即使交付失败也被消费。未知副作用不重放；authority 丢失不重新选择终端。

Worker 将 grant 映射为独立 Host 附件，启用 input 和 resize、禁用 read，并申请 resize lease。
既有查看附件保持只读并保留 frame baseline。Worker 同样保证每个 Host 终端只有一个 grant，
校验精确 viewer/grant scope 与递增 sequence。只有 Host 返回的确切 capability revision 可授权
原始输入。Input 包含 1 至 4,096 byte；接受表示进入 Host 有界输入 queue，不表示 shell 已执行。
Resize 确认精确的完整 geometry。不增加 Host wire contract 或 VT owner。

Detach 与 Session 丢失先释放控制附件，再分离查看附件。Server 预留持续占用，直到固定代次的
cleanup 完成或该 Worker 不可用；cleanup 经既有有界 request queue 准入，不持锁跨越 I/O。
过时 cleanup 不能清除新 grant。Host exchange 失败或结果矛盾会结束 Worker connection；完整
交付后的故障由 Server 报告 unknown control outcome。其他 viewer 不会自动提升。
保留查看附件的独立 release 和强制接管仍是后续工作。

最多八个 viewer 意味着最多八个控制附件。因此，cleanup 可在下个请求前执行十六次一秒 Host
exchange。Worker response 预算为二十秒，launch lease 为四十六秒，原生 Client submission 为
五十秒。Request write 和 Client record read/write 保留既有两秒上限。这些是本机实现限制，
不是发布性能预算。

`LocalTerminalClientAuthority` 拥有 acquisition、controller input/resize、detach、loss 和过时
cleanup；`LocalTerminalRelay` 拥有递增请求和未知副作用退役。具体 reservation、cleanup、protocol
及 process test 细化这些声明，包括竞争 Session、错误 grant/sequence、输入 queue 确认、可观察
shell 输出、已接受 resize、断连和重新获取。设计质疑使用当前上下文，不属于独立审查证据。
TUI 激活、public cutover、持久化恢复和完整里程碑验收仍是独立边界。

## 已接受的 Terminal Client authority 与 relay 建模记录

已接受的纯模型切片让 Rust Server 成为 relay
`Client -> Server -> Worker -> Terminal Host` path 上 product-level Client Session 与 terminal
authority 的 owner，并进一步检查反向实时输出 path `Host -> Worker -> Server -> Client`。这些
model 描述已接受的 refinement target；它们不声称当前 runtime、wire contract 或 public UI 已实现
该目标。Viewer attachment 把一个 Client Session generation 与 retained terminal 关联，而
controller 是该 terminal 唯一、同时覆盖 input 与 resize 的 grant。

| 字段   | 已接受的 refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 多个同级 TUI、Web、Desktop 或 mobile Client Session 必须能并发读取同一个 retained terminal，同时不能产生多个 input 或 geometry writer。Slow Client 不能阻塞其他 Client 或 Worker progress；Client 退出必须 detach transient authority，而不能结束 terminal。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| 输入   | Server-owned Client Session identity 与 generation；Worker、Host 与 terminal availability；viewer attach、显式 controller acquire、input、resize、detach、disconnect、reconnect 与 cleanup；Host 产生的 opaque OBTF snapshot/delta bytes；以及已有 attachment-scoped `FrameApplied`、`NeedSnapshot` 与 `ProjectionBlocked` 语义。Relay model 增加抽象 request identity 与严格递增的 attachment-local request sequence，但不选择其 wire field 或 encoding。                                                                                                                                                                                                                                                                                                                                                                                |
| 数据   | Imported Server authority state，加上每个 modeled Session 的 attachment generation 与 projection phase、分别用于 Host-to-Worker、Worker-to-Server、Server-to-Client delivery 的三个有界 per-attachment downstream queue、opaque frame value、精确 feedback scope、单个 control request flight、used 和 effect-unknown request identity，以及独立 Worker progress counter。一个 attachment-scoped projection flight 在 Client feedback 开始返回前至多占据一个 downstream queue。Model 中不存在 Server 或 Worker 所属的 VT、screen、row、scrollback 或 history state。                                                                                                                                                                                                                                                                      |
| 输出   | 每个 live viewer 都可以收到 attachment-scoped snapshot、delta 或 `ProjectionBlocked`；applied-frame acknowledgement 与 `NeedSnapshot` 在相同 attachment scope 上返回。只有唯一 imported controller 的 input 与 resize 才能到达 Host effect；non-controller、out-of-order 与 reused-identity request 会被拒绝。Downstream queue 满时只能 pause 或 close 该 attachment，不能改变其他 attachment 的 queue，也不能禁用独立 Worker progress。                                                                                                                                                                                                                                                                                                                                                                                                  |
| 不变量 | `LocalTerminalClientAuthority` 仍是唯一 authority：viewer 是其 imported viewer set，effective input 与 resize authority 是从中派生的同一个 set，cardinality 至多为一。Acquire 不会替换 owner，loss、detach 或 cleanup 都不会自动提升 viewer。每个 downstream queue 都有界且 attachment-scoped；relay hop 保留完整 opaque record；feedback 按 attachment 划分；applied sequence 不超过 emitted sequence；effect-unknown request 会被 retire，绝不能重新进入 send phase；detach、disconnect 与 stale cleanup 保留 retained terminal。                                                                                                                                                                                                                                                                                                       |
| 决策   | Controller 同时拥有 input 与 resize。其他 viewer 只有在显式 release 使 grant 变为 unowned 后才能 acquire；不存在 automatic promotion、implicit claim 或 modeled forced takeover。Server 选择 product authority，Worker 仍是唯一 Host broker，Host 仍是 PTY、VT、projection 与 history owner。Outcome unknown 的 input 或 resize 会消耗其抽象 request identity，绝不自动 replay。`NeedSnapshot` 放弃该 attachment 的 queued delivery 并请求新 snapshot；`ProjectionBlocked` 只阻塞该 attachment，直到 Host 发出 later-revision snapshot，而不会把 projection ownership 移入 relay。                                                                                                                                                                                                                                                        |
| 模型   | [`LocalTerminalRelay`](formal/tla/LocalTerminalRelay.tla) 显式 `EXTENDS LocalTerminalClientAuthority`，并从 imported `viewers` 与 `controller` 派生 read、input、resize attachment；它不创建平行 authority machine。其 control-outcome、projection-feedback、backpressure-isolation、reattach-feedback、attachment-lifecycle 与 failure-interleavings configuration 会拆分无关 finite history，同时检查同一个 state machine。注册的 source digest 包含 imported authority module。                                                                                                                                                                                                                                                                                                                                                        |
| 转换   | Session open 加 attach 建立 `needs-snapshot`；精确 opaque record 不经 decode 穿过三个 queue；Client apply 创建 `FrameApplied` acknowledgement；`NeedSnapshot` 清空该 attachment 的 in-flight record；`ProjectionBlocked` 可以在首个 snapshot 前或 streaming 期间发生，进入 blocked phase，并且只能通过 later-revision snapshot 恢复。Queue saturation 只能 pause 并随后 close addressed attachment。Detach 清空 delivery，并在自己持有时清除 controller grant，同时保留 terminal；re-attach 推进 attachment generation、重置 attachment-local relay state，并要求新 snapshot。Disconnect/reconnect 与 stale cleanup 仍按 generation 划分。                                                                                                                                                                                                |
| 依赖   | Model 把 package-owned Terminal Protocol attachment、input、resize、OBTF、projection-demand 与 projection-feedback 语义映射到已有 Server-Worker spine 和 Worker-owned Host broker。该切片不增加 codec、wire message、runtime dependency、direct Client-Worker path 或 UI behavior。未来 implementation 必须把这些 action refinement 为 concrete carrier 与 Host command，且不得让 Server 或 Worker 获得 VT/history authority。                                                                                                                                                                                                                                                                                                                                                                                                            |
| 所有权 | Server 拥有 Client Session generation、viewer membership、combined controller selection、relay queue admission、rejection 与 stale cleanup。Worker 拥有 Host connection brokering，但不拥有 terminal presentation state。Host 拥有 retained PTY lifetime、Ghostty VT、immutable attachment-scoped OBTF production、projection/history state 与 low-level capability enforcement。现有 Host resize-owner promotion mechanism 不会成为 Server product policy。                                                                                                                                                                                                                                                                                                                                                                              |
| 验证   | Authority configuration 检查三个 Client slot、两个 Session generation、有界 input/geometry revision、全部 semantic action、safety 与 cleanup progress。Relay configuration 检查两个 Client 与 attachment generation、两个 request identity 与 request/frame sequence、两个不同 opaque record、depth-one queue、每个 relay hop 的 exact source-record equality、blocked-projection recovery、stale-feedback rejection，以及 control/output 与 Client、Server、Worker、Host loss 的交错。独立 Worker progress 保持 queue-independent，并在 weak fairness 下完成，除非 Worker 已丢失。每个 relay action 都要求非零 coverage。这些是穷尽 finite-model bound，不是 release budget；上文记录查看与控制运行时证据。当前验收证据中 authority configuration 仍有未解决的 300 秒超时；六个 relay configuration 均完成且达到要求的 action coverage。 |
| 限制   | Model 覆盖一个 retained terminal、每个 modeled Client Session 一个 attachment、一个抽象 control flight 与按 record 计数的 queue。它不定义或证明 wire schema/encoding、具体 request field、production queue count/byte budget、slow-Client threshold 或 pause-versus-close policy、production scheduler/fairness guarantee、timeout value、runtime/PTY retention、restart/durable recovery、direct attachment、forced takeover、collaborative input、controller fairness 或 public UX。                                                                                                                                                                                                                                                                                                                                                    |

已检查的
[`LocalTerminalClientAuthority`](formal/tla/LocalTerminalClientAuthority.tla) model 仍是 authority 与
cleanup proof，其中显式 detach action 会保留 open Session 与 terminal。
[`LocalTerminalRelay`](formal/tla/LocalTerminalRelay.tla) 把该 state 与 transport-only queue 和
feedback 组合。Server 或 Worker loss 可以让 authority 失效，却不能转移 authority 或改变 retained
terminal；只有 Host loss 可以把 terminal 改为 `lost`。

## 本机终端协调器建模记录

已接受的本机里程碑授权 Server 的单一有效操作系统用户主体，通过获准接入的本机 Human
Client Session 执行默认 shell 的 Create 和 Start。OBLC 本身仍只负责传输准入；此私有服务
在验证 peer 和 cohort 后显式选择该策略。其他端点、远程身份、Worker peer 或渲染器类型
不会因此获得权限。

| 字段   | 契约                                                                                                                                                                                                                                                                                            |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 将获准 Client 的 Create/Start 意图接入现有 Worker carrier，同时不转移放置权限、不重放副作用，也不在 Client 断连时丢失保留绑定。                                                                                                                                                                 |
| 输入   | 严格 OBCS 意图、当前获准 Worker 的身份/代次，以及适配器提供的绝对租约到期时间。Client 字段不能成为 operation、terminal、lease、generation 或 fence。                                                                                                                                            |
| 数据   | Server 为单一本机主体维护至多 1,024 个 create 引用的内存映射；每项保留精确放置、最后授权、启动模式，以及 pending、awaiting-start、started、unknown 或 unavailable 状态。不淘汰，也不提供持久恢复。                                                                                              |
| 输出   | 关联的 OBCS 结果或一个精确 OBCN 命令。缺少 Worker 和约束不满足均无副作用。Start 可跨 Session 解析成功的 deferred Create；未知或不存在的 create 不得派发。                                                                                                                                       |
| 不变量 | 检查溢出的单调 128 位分配器跨终端和 Worker 代次不复用 operation 身份。每次派发 Start 都递增终端 fence 并使用新 lease。每个终端至多有一个 pending 操作。同步锁不跨 Worker I/O 或 Client 输出。                                                                                                   |
| 决策   | 拒绝已知 create 引用冲突；保留容量已满和并发操作返回 server-busy。已确认 deferred Start 后，相同几何的 Start 直接返回 already-started，不再发送命令；不同几何被拒绝。Immediate create 拒绝 Start。Worker 更换不能重新指派保留绑定。                                                             |
| 模型   | 纯 prepare/complete 状态机在副作用前预留绑定，通过精确结果或明确的交付失败提交。独立的 Session 输出不阻塞另一 Session 的下一次操作。                                                                                                                                                            |
| 转移   | Prepare 在提交 carrier 前预留状态。精确且当前的成功结果在向 Client 发布前提交 Host 身份和几何。明确未交付的 Start 失败恢复 awaiting-start；交付不明确或成功结果矛盾使绑定冻结为 unknown。此前已确认的 Host 身份在 Start 失败或结果未知时仍保留。Client 丢失不能取消已提交操作或移除绑定。       |
| 依赖   | 纯协调器消费协议值和显式时间/放置事实。私有服务负责短 mutex 临界区、墙钟采样、Worker handle 选择及一次 carrier 调用。现有 carrier 负责线协议串行化和完整写入分类。                                                                                                                              |
| 归属   | Server 拥有主体策略、create 引用、放置、operation/lease/fence 签发及结果提交。Worker 拥有启动默认值、准入和 Host 副作用。Host 拥有 PTY/VT 和保留执行。不新增公开或跨 workspace 契约。                                                                                                           |
| 验证   | 确定性协调器轨迹检查分配、pending 互斥、精确结果提交、unknown 隔离、容量、几何和更换。Socket/进程轨迹检查跨 Session Start、串行化、畸形 peer 和 Client 丢失。已登记的启动授权及协调器模型检查各自有限的安全性/进展性质。                                                                        |
| 限制   | 仅限附加、未发布的 POSIX 本机权限。1,024 条记录上限沿用已接受的 Worker 代次限制，生产前需要显式退役机制。租约有效期为四十六秒，来自 carrier 的一个排队及一个在途交换，每次均有两秒 write 和二十秒 result 截止时间；到期可拒绝且不重试。无重放 ledger、细粒度权限、UI 激活、重启协调或生产恢复。 |

协调器的实际消费者是准入后的 OBCS handler。其集成见证是 Client 通过真实 Server、Worker
和 Host 进程发起 Create 和 Start。下一个缺失的用户可见边界是 TUI 激活。

[`LocalTerminalCoordinator`](formal/tla/LocalTerminalCoordinator.tla) 检查一个 deferred 绑定、
两个 peer Session、三个 operation 身份和两个 Worker 代次。Prepare 对应 IssueCreate/IssueStart，
complete 对应精确/未交付/拒绝/未知/过时代次结果转移；每个 Session 的独立发布可与另一 Session 的
操作交错。进展依赖有界 carrier 完成及发布/放弃步骤的弱公平性，不保证已断连 Client 的结果交付。
几何、容量、类型映射与真实 PTY 行为由确定性测试和进程见证负责。

## 所有权

该 application 拥有注入 snapshot 的 projection mechanics、本机 Worker 和 Client registry、私有
OBCN carrier、本机终端协调器以及Session 查看／控制 relay。它不发布 Workbench mutation，也不改变现有消费者。
Desktop/Workbench 仍是已交付 Workbench writer，
[`apps/server`](../server/README.zh-CN.md) 保留既有 Browser ingress 权威。input/resize control、持久化、
重启协调、激活、生产服务生命周期、打包、迁移及公开切换仍属于后续切片。

源码树包含可导入的 projection root（`src/lib.rs`）和私有 binary composition root
（`src/main.rs`）。穷尽的 lateral 方向为 `local_site_service -> worker_registry`、
`worker_obcn_carrier`、`local_client_service::start` 和 `local_terminal_service`；
`local_client_service -> client_session_registry`、`local_terminal_service` 及其私有
`obcs_session`；`local_terminal_service -> terminal_coordinator`、`worker_registry` 和
`worker_obcn_carrier`；以及 `worker_registry -> worker_obcn_carrier`。协调器的私有 `outcomes`
子模块拥有 Worker observation 投影。服务的 `viewer_session` 子模块拥有附件 lifetime，
`controller_session` 拥有按 terminal 预留、Session grant、递增副作用 sequence 和精确释放结算；
`projection` 拥有路由/反馈状态。Client 服务的 `record_stream` 子模块拥有准入后 I/O deadline。
反向及其他 lateral import 仍禁止；测试可访问 owner-private
状态而不扩大生产可见性。

Carrier 拥有串行 I/O、精确 echo 验证及代次退役，不导入 OBCS 或协调器策略。纯协调器只导入
协议值和标准库，不导入 transport、clock 或 process adapter。服务在 terminal 预留和提交时按 registry
再 coordinator 的顺序获取锁；grant 预留则按 registry 再 controller map 获取锁。I/O 前释放
全部锁，且不重试。私有 `obcs_session` runner 仅导入
OBCS 和标准库 I/O，并调用注入的语义 handler。任何模块都不通过 projection library 重导出。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-worker --locked
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions,local-client-sessions-obcs,local-site-process-spine,local-terminal-client-authority,local-terminal-relay,local-worker-obcn-carrier,local-terminal-coordinator,terminal-launch-authorization
pnpm verify
```
