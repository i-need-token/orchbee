# Rust Orchbee Worker

这是第一条 POSIX 本机终端 vertical 使用的实验性、尚未发布私有 Rust Worker application。
产出的 executable 是 `orchbee-worker`。它参与根 Cargo gate，但不会被打包、安装、由
`orchbee` 激活，也没有被当前产品路径消费。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## 进程边界

在 macOS 与 Linux 上，确切私有开发调用为：

```text
orchbee-worker \
  --local-endpoint <absolute-server-socket> \
  --terminal-host-runtime-directory <absolute-directory> \
  --terminal-host-endpoint-id <32-lowercase-hex>
```

Worker 首先连接所提供的 Server Unix-domain socket，要求对端具有相同 effective
operating-system user，提交有界 cohort handshake，并验证 typed admission。在 Server 接受该
Worker connection 之前，它不会接触 Terminal Host。

Server admission 之后，Worker 只解析
`<runtime-directory>/<endpoint-id>.s`，连接现有 Terminal Host，独立要求相同 effective user，
并执行现有精确版本 Terminal Protocol `Hello`/`HelloAccepted` exchange。请求使用固定的
diagnostic Worker broker client identity，声明 protocol hard limit 且不声明 optional feature；
只有 registered request/response role、精确 correlation、精确 protocol version、匹配的 negotiated
limit、空 feature intersection 与非零 Host epoch 都成立时才接受。

只有两个对端均接受后，Worker 才向 stdout 写入 `ready\n`。随后它的主线程在两个自有 socket
上 poll，不创建 connection thread 或 queue。Server loss 会关闭 Host broker socket；Host loss
会关闭 Server attachment。两种结果都保持静默并以 status `1` 退出，使 Server 能观察 Worker
loss，也使没有 retained terminal 的 Host 能执行自己的 idle-shutdown rule。Invalid invocation、
connection、authentication、rejection、protocol、output 或 transport failure 也采用同样的静默
status-`1` 结果。

本切片在 handshake 之后不发送 Server message，发往 Terminal Host 的 message 也只有
`Hello`。它不执行 retry、discovery、activation、daemonization、child-process supervision、
terminal inventory、launch、attachment、input、resize、runtime filesystem inspection、shell 或
working-directory resolution、PTY operation、Agent behavior 或 durable write。它不使用
current-directory、Project、Environment、Checkout、Worktree、launch-profile、Files 或 Git
fact。Endpoint selection、service supervision、Server-issued launch authorization 的 carrier
delivery 与 runtime consumption、Host activation 以及 reconciliation 属于后续切片。

## 暂存的默认交互式 launch 解析

Application-private `default_interactive_launch` module 会编译进 Worker，但当前 process path
不会调用它。其 transport-neutral `DefaultInteractiveLaunch` output 只持有一个已解析
executable、恰好为空的 literal argument vector，以及三个固定 environment entry。后续
authorized Host adapter 可以把这些 fact 与单独解析的 cwd、Terminal Protocol terminal profile、
configuration、geometry 和 creation identity 组合起来。三个 entry 映射到 Terminal Protocol
launch environment；在已接受的首个 milestone 中，invocation-scoped
`CreateTerminal.environmentOverlay` 仍为空。

解析依次考虑 inherited `SHELL`、effective operating-system account 的 shell，再考虑
`/bin/sh`。Candidate 只有在 path 非空、为 absolute、是有效 UTF-8、不含 U+0000、不超过
4,096 byte、解析为 regular file，并且 process effective user 在解析时可执行它时才有效。Invalid
或 unusable candidate 会继续回退而不公开其 value。若连 `/bin/sh` 都不可用，解析返回 typed
`NoUsableShell` failure。

Native adapter 只读取 `SHELL`、effective-user account record，以及 executable metadata 与
access state。它既不 canonicalize 已选择 path，也不启动 process。Result 始终只设置
`TERM=xterm-256color`、`COLORTERM=truecolor` 与 `TERM_PROGRAM=ghostty`；它绝不复制
caller-transient 或 product variable，不读取 cwd 或 launch profile，不发 diagnostic，不打开
Server/OBCN/Host transport，也不构造 Host command。Pure table test 负责 candidate admission、
所有 selection combination、output fact 与 typed failure；native test 覆盖 executable probe 与
当前机器的 resolution boundary。File 在这次 observation 后仍可能变化，因此最终 Host execution
boundary 仍负责 typed launch failure。

## Worker 本地 authorized Host-operation 内核

Application-private `terminal_authorization` module 会编译进 Worker，但当前 process path
特意不构造也不调用它。它是供私有 Host launch adapter 使用的 carrier-neutral deterministic kernel，消费
[`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.zh-CN.md) 的
semantic Launch 与 Start value。它的存在不会给上述 runtime 增加 post-handshake Server message
或 Terminal Host command。

后续 composition root 必须只用当前 local-Site `WorkerId` 与 OBLS 接受的非零 Worker generation，
构造这个本身固定为 version 2 的 kernel。其 carrier 必须认证 active
authoritative Server，并且只提供 well-formed issuance：在该 Server
authority 内，每个 `OperationId` 跨 Terminal、Worker 与 Worker generation 均全局 fresh。Kernel
刻意不保留无界的跨 terminal collision ledger，也不声称能遏制违反该 producer invariant 的 Server。
后继 Server authority 只有在其 generation、lease 与 fence transition 使每份旧 authorization 不再
current 后，才能开始新的 producer scope。

一个 kernel 代表一个已接受的 Worker generation，并且最多为该 generation 保留 1,024 个不同的
terminal binding。它没有 release、TTL、LRU 或 eviction transition。Capacity 只拒绝此前未见过的
`TerminalId`；map 满时，exact cached replay 以及对现有 terminal 的有效 higher-fence replacement
仍然 eligible。该 behavior 属于 terminal-launch capability version 2；version 1 或其他 mismatched
request 会在考虑 capacity 之前以 `UnsupportedCapability` fail closed。当前 kernel 本身就是
version 2 implementation；其 constructor 不能被配置为在保留 version 2 capacity semantic 的同时
admit legacy 或 future capability。

每次 admission 或 pre-handoff decision 还会接收显式 Unix-millisecond time，Start admission 还会
接收当前 observed Host epoch。
返回的 dispatch 是准备匹配 operation 的许可，并非 Host effect 已发生的证据。Adapter 必须在
第一个可能的 Host effect 之前立即以新观察到的 Host epoch 调用 `begin_host_handoff`，然后通过
同一个私有 dispatch token 报告一个 typed Host completion、确定的 pre-handoff failure 或
post-handoff unknown outcome。Kernel 会保留观察到的最大时间，因此 caller 可见的 wall-clock
rollback 不能让任何 expired lease 复活。

### 授权内核建模记录

| 字段   | 当前契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 问题   | 后续 OBCN carrier 可能重复、延迟、针对错误 Worker generation 或在 authority 已前移后交付 Server-authorized Launch 或 Start。Worker 必须在本地决定是否可以开始恰好一次 Host operation，并保留精确 terminal result，同时不取得 Server placement authority，也不自行执行 Host effect。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 输入   | 来自经过认证且遵守全局 fresh `OperationId` producer invariant 的 authoritative Server 的 canonical OBCN `AuthorizedTerminalLaunch` 或 `AuthorizedTerminalStart`，其 authorization capability 会根据 kernel 内置的 version 2 policy 检查；caller 提供的当前 local-Site Worker ID、非零 OBLS generation 与当前 Unix-millisecond time；Start admission 与 handoff boundary 再次观察到的 Host epoch；以及 typed Host completion。缺失 result 或 occupied slot 保持为显式 state，而不推断成 success。                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 数据   | `AuthorizedHostOperationKernel` 为每个 Worker generation 最多持有 1,024 个完整 highest-fence binding，以 product `TerminalId` 为 key，并包含 operation kind、完整 authorization、Launch 或 Start payload，以及可选的精确 cached result。它另外持有一个处于 `empty`、`awaiting-handoff`、`handed-off` 或 `result-ready` 的 process-local slot，以及 maximum-observed Unix-millisecond high-water mark。私有 `AuthorizedHostDispatch` 保留完整 admitted operation，因此也保留其 generation 与 authority binding。                                                                                                                                                                                                                                                                                                                                                                                                                            |
| 输出   | Admission 返回 `Dispatch`、`ResultReady` 或 `SlotOccupied`；handoff 返回 `HandedOff` 或 `ResultReady`；`take_result` 返回一个 typed OBCN Launch 或 Start result，其中在达到 retained-binding limit 后，对有效且未见过的 terminal 返回 `CapacityExceeded`；非法 completion observation 返回 typed internal error，且不改变 slot。内核不开 carrier、不执行 Host call、不发 product event，也不写 durable state。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| 不变量 | Wrong Worker、stale generation、unsupported capability、expired new lease、lower fence、conflicting same-fence binding，以及超出 capacity 的有效 unseen terminal 绝不 dispatch。Exact binding 最多 dispatch 一次，且即使 expiry 后或处于 capacity 时也只 replay 其 cached exact result。Kernel 一旦观察到 lease expired，之后较低的时间不能使它复活。即使在 capacity 时，有效 higher fence 也会替换 prior binding 并清除其 cache。Capacity rejection 绝不插入 tombstone，也不改变任何 binding。Global slot 最多由一个 operation 或 undelivered result 占用。只有 `handed-off` 才能变成 Host outcome。                                                                                                                                                                                                                                                                                                                                      |
| 决策   | 分类顺序为 occupied slot、Worker identity、Worker generation、exact cached replay、capability、lease expiry、per-terminal fence、unseen-terminal capacity，最后是 Start target epoch。因此在 capacity 时，unsupported 或 expired new request 保留其 capability 或 lease 分类；现有 lower 与 same-fence request 保留其 fence 分类；只有有效 unseen terminal 会收到 `CapacityExceeded`。在 Start admission 与 handoff 再次检查时，target epoch mismatch 变为 `HostIdentityChanged`；对 unseen terminal，capacity 先于该检查。Handoff expiry 会阻止 effect，accepted Start identity、epoch 与 geometry 必须保持精确。                                                                                                                                                                                                                                                                                                                         |
| 模型   | 一个 deterministic state owner 将 generation-local、1,024-entry 的 per-terminal monotonic binding map 与一个 globally bounded operation/result slot 组合起来。`admit_launch` 与 `admit_start` 是 principal entry point；`begin_host_handoff` 是命名的 external-effect boundary；typed observation method 会关闭 accepted binding 并排队其 result。[`WorkerTerminalBindingCapacity`](formal/tla/WorkerTerminalBindingCapacity.tla) 将 finite threshold 加一个 unseen identity 的 cross-terminal capacity refinement 与最小化的 empty、operation-pending 和 result-ready slot abstraction 组合起来。                                                                                                                                                                                                                                                                                                                                         |
| 转换   | 下方 transition table 定义 admission、handoff、completion、exact replay、result delivery、conflict 与 stale-token behavior。接受 binding 前的 rejection 会排队一个 uncached result；接受 binding 后的每个 terminal outcome 都会在公开前先缓存。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| 依赖   | 仅依赖 Rust 标准库、canonical local-Site Worker identity，以及 OBCN semantic type 与 constructor。Time、socket liveness、decoded framing、current Host epoch 与 Host observation 均由后续 adapter 以 value 形式提供；kernel 会单调化 supplied time，但绝不读取 clock。它不读取 environment、filesystem、socket、process、profile、executable、shell 或 working directory，也不解析或编码 Host framing。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 所有权 | Server 继续拥有 placement、authorization issuance 与 reconciliation。OBCN 拥有 carrier-independent request/result type 与 wire codec。Worker 只拥有本地 validation、monotonic replay memory、单一 dispatch gate，以及 Host handoff boundary 前后的分类。Terminal Host 仍是 terminal 与 process effect 的唯一 owner。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 验证   | Table-driven unit test 对两种 operation 覆盖 wrong identity、generation、legacy 与 future capability mismatch、expiry 与 clock rollback、重叠 rejection priority、1,023/1,024/1,025 capacity boundary、无 tombstone rejection、capacity 时的 exact replay 与 higher-fence replacement、lower 与 conflicting fence、每种 accepted/rejected/failed/unknown outcome 的 exact replay、admission 后 expiry、stale 与 wrong completion、slot backpressure、initial 与 raced Start identity change、mismatched Start completion geometry、两种 Launch mode 与两种 Start success transition。OBCN test 负责 value/codec validity；注册的 TLA+ model 负责各自的 bounded abstraction。                                                                                                                                                                                                                                                               |
| 正确性 | Server-owned [`TerminalLaunchAuthorization`](../orchbee-server/formal/tla/TerminalLaunchAuthorization.tla) 的 `acceptedBinding`、`requestPhase`、cached outcome/epoch/transition 与 pending result，分别映射到一个 kernel binding、slot phase、cached typed result 与 result-ready slot。其 `FreshOperation` guard 是 Server producer premise 的 single-terminal、single-Worker projection。Worker-owned [`WorkerTerminalBindingCapacity`](formal/tla/WorkerTerminalBindingCapacity.tla) 将其 bounded fence/cache function、retained set 与 abstract global slot 映射到 Rust binding map 和 operation/result slot。它检查 capacity、occupied-slot priority、最多一个 uncached operation/result、delivery clearing、rejection preservation，以及仅在 slot empty 后才允许 full-capacity exact replay 与 higher-fence replacement。Unit test 是 implementation-refinement evidence；TLC 仍是 bounded model evidence，而不是 Rust code proof。 |
| 限制   | 当前没有 carrier 或 Host adapter 调用此内核，因此测试只建立 deterministic state behavior。Binding 与 cached result 都是 process-local，不做 crash persistence，并会保留到 kernel drop 或 higher fence 替换。1,024 limit 没有 release、TTL、LRU 或 eviction path；production 之前仍需要显式 retirement contract。在不维护无界 ledger 的前提下，kernel 无法检测 Server 跨 terminal 复用 `OperationId`。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |

| 当前 slot          | 输入或 observation                                | Guard                                                           | 下一 slot          | Binding/result effect                                                              |
| ------------------ | ------------------------------------------------- | --------------------------------------------------------------- | ------------------ | ---------------------------------------------------------------------------------- |
| `empty`            | New 或 higher-fence Launch/Start                  | Worker、generation、capability、lease 与 fence 都为 current     | `awaiting-handoff` | 存储带 empty cache 的完整 binding，并返回其私有 dispatch。                         |
| `empty`            | Invalid new、stale 或 conflicting request         | 某项 named rejection 成立                                       | `result-ready`     | 排队 exact-echo typed rejection，不接受也不替换 binding。                          |
| `empty`            | 1,024 个 binding 时有效的 unseen Launch/Start     | Worker、generation、capability 与 lease 都为 current            | `result-ready`     | 排队 exact-echo `CapacityExceeded`；不插入 tombstone，也不改变任何 binding。       |
| `empty`            | Exact accepted binding                            | 它的 exact terminal result 已缓存                               | `result-ready`     | 排队语义不变的 cached result；不 dispatch，也不重新判断 expiry。                   |
| `awaiting-handoff` | `begin_host_handoff`                              | Lease 仍 live，且 Start target epoch（如有）等于 observed epoch | `handed-off`       | 保留完整 dispatch 与 observed Host epoch；直到此时 adapter 才能执行第一个 effect。 |
| `awaiting-handoff` | Expiry、Start epoch replacement 或 definite loss  | 尚未开始 Host effect                                            | `result-ready`     | 缓存并排队 lease/identity rejection 或 `HostUnavailableBeforeHandoff`。            |
| `handed-off`       | Accepted、rejected、failed 或 unknown Host result | Dispatch 与 Host observation 一致；accepted Start geometry 精确 | `result-ready`     | 缓存并排队 exact typed result；Launch state 从其 admitted mode 派生。              |
| `result-ready`     | `take_result`                                     | 存在一个 result                                                 | `empty`            | 只交付一次 queued result，同时保留 accepted binding cache 供 replay。              |
| 任意 occupied slot | 另一个 admission                                  | 已有 operation 或 undelivered result 占用 slot                  | 不变               | 返回 `SlotOccupied`；不比较也不改变 terminal authority。                           |
| 任意 phase         | Wrong、stale 或 out-of-phase dispatch observation | Dispatch 或 phase 不匹配                                        | 不变               | 返回 typed internal error，并保留当前 operation/result。                           |

| Decision partition                      | Launch evidence                                                                  | Start evidence                                                           | Required result                                                                              |
| --------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| Worker、generation、capability 或 lease | 每种 rejection 的 table case                                                     | 相同 table case                                                          | 精确 request echo，不 dispatch。                                                             |
| 重叠 invalid condition                  | Lower 加 unsupported、lower 加 expired、same-fence changed capability 或 expiry  | 共享 classification path                                                 | Capability 先于 expiry，expiry 先于 fence，只有 exact cached binding 会绕过 expiry。         |
| Per-terminal fence                      | Lower、same-operation、lease 与 payload conflict；higher replacement             | Lower、cross-kind、target 与 geometry conflict；higher replacement       | Stale 或 conflicting rejection；有效 higher fence 会清除旧 cache。                           |
| Retained-binding capacity               | 1,023 时 admit、1,024 时已满、第 1,025 个 identity reject；full 时 replay/rebind | 相同 boundary，加 identity-mismatched unseen Start                       | 只有 unseen identity 收到 `CapacityExceeded`；没有 tombstone、binding mutation 或 eviction。 |
| Expiry 后 exact replay                  | 两种 launch mode 的 accepted、rejected、failed 与 unknown                        | `Started`、`AlreadyStarted`、rejected、failed 与 unknown                 | 原始完整 cached result；无新 Host effect。                                                   |
| Handoff boundary                        | Expiry、definite failure 与 Host epoch-bound completion                          | Admission mismatch、expiry、target epoch replacement 与 definite failure | Handoff 前没有 accepted Host observation；post-handoff loss 为 unknown。                     |
| Completion identity 与 ordering         | Wrong token、stale token、wrong phase 与 wrong epoch                             | Wrong operation kind、target identity、epoch 或 accepted geometry        | Typed error 且 state 不变；后续 valid completion 或 unknown observation 仍可将其关闭。       |
| Global result slot                      | Awaiting、handed-off 与 result-ready backpressure                                | 共享 slot behavior                                                       | 两种 kind 合计恰好一个 outstanding operation 或 undelivered result。                         |
| Time rollback                           | Expired unaccepted request 与 admission-to-handoff expiry                        | Expired unaccepted request                                               | Maximum-observed time 阻止 expired lease 再次变为 dispatchable。                             |

## Worker 本地 Terminal Host launch adapter

Application-private `terminal_host_launch_adapter` module 同样会编译，但当前 process path 不会调用它。
它是一个 pure value mapper，在一项已接纳的 `AuthorizedHostDispatch`、现有 Terminal Protocol
request/response value，以及 `terminal_authorization` 已接受的 Host observation 之间转换；它既不打开
Host socket，也不改变 kernel state。

### Launch-adapter 建模记录

| 字段   | 当前契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 已接纳的 OBCN Launch 或 Start 必须通过现有 Terminal Protocol，而不能让 carrier 获得虚构 process、directory、environment、identity、geometry 或 outcome 的 authority。                                                                                                                                                                                                                                                                                                             |
| 输入   | 一个私有 admitted dispatch；仅 Launch 还接收注入的 `ResolvedTerminalLaunchFacts`，其中包含 stable Host `CreationId`、此前已解析的 executable、literal argument、durable launch environment、cwd 省略时的 default、terminal profile 与 optional terminal configuration；或者一个 decoded Host success/`TypedFailure` 加 response envelope 的精确 Host epoch。Transport 只提供 handoff 前 definite 与 handoff 后 unknown 的区分。                                                   |
| 数据   | Mapper 不保留 state。`ResolvedTerminalLaunchFacts::new` 是 node-local default-interactive-launch resolver 与单独提供的 creation-identity/cwd/profile/configuration fact 之间的狭窄 composition seam；operating-system、account、environment 与 filesystem lookup 均留在本 module 之外。                                                                                                                                                                                           |
| 输出   | Launch 变为 `CreateTerminal`；Start 变为 `StartTerminal`；Host success 或 `TypedFailure` 变为现有 `LaunchHostCompletion` 或 `StartHostCompletion`；transport loss 变为面向 kernel 的 pre-handoff-failure 或 post-handoff-unknown observation。Caller 仍负责 framing、I/O、correlation、handoff，以及把 observation 应用于 kernel。                                                                                                                                                |
| 不变量 | Launch 会复制 independently resolved stable Host `CreationId`，而不是从 product `TerminalId` 派生它。Start 发送 OBCN target 的 Host `TerminalId` 并保留其完整 identity，不施加 product/Host byte-equality policy。Explicit cwd 会被精确复制；省略时选用注入的 Worker-user-home default。Immediate Launch 与 Start 保留全部四项 geometry dimension；deferred Launch 不携带 geometry。Caller overlay 始终为空。其他 launch fact 都会精确复制。                                      |
| 决策   | Mapper 将 Terminal Protocol encoder 作为 pure preflight validation 复用，因此 public protocol value 无法绕过 identity、text、count、palette、geometry 或 control-payload bound。Terminal Protocol `TypedFailure.retryable` 控制 retry guidance，而不是 failure kind；因此每个 generic `TypedFailure` 都变为 `HostReportedFailure`。`HostRejected` 需要未来明确的 stable rejection contract。Response kind、identity、epoch 与 geometry contradiction 仍返回 typed adapter error。 |
| 模型   | `Launch(dispatch, facts) -> CreateTerminal`；`Start(dispatch) -> StartTerminal`；`(Launch dispatch, expected CreationId, response epoch, exact response) -> Launch completion`；`(Start dispatch, response epoch, exact response) -> Start completion`；handoff 前 transport loss 映射为 definite failure，handoff 后 loss 映射为 unknown。每个 function 都是 deterministic 且 side-effect free。                                                                                 |
| 依赖   | 仅依赖 application-private OBCN authorization type，以及现有 Rust Terminal Protocol semantic value/encoder。Module 不依赖 socket、envelope codec、carrier、clock、process、filesystem、account database、environment reader、shell resolver 或 kernel orchestration。                                                                                                                                                                                                             |
| 所有权 | Default-interactive-launch resolver 拥有 node-local executable 与 durable default environment selection。Composition root 拥有 stable Host creation identity，以及 omitted-cwd、profile 与 optional configuration fact。此 adapter 只拥有 deterministic translation 与 cross-contract consistency check。Terminal Host 仍拥有 PTY/process effect；carrier 拥有精确 envelope/correlation transport；authorization kernel 拥有 replay 与 outcome state。                            |
| 验证   | Unit test 覆盖刻意不同的 product `TerminalId` 与 Host `CreationId` value、immediate 与 deferred Launch、explicit 与 omitted cwd、stable Host creation identity、empty overlay 与 accepted default environment、Start、两种 transition、每种 operation 中两个 `retryable` value 均映射为 reported failure、两种 transport outcome、wrong response kind、identity/epoch/geometry contradiction，以及 exact argument/environment 与 malformed-value bound。                          |
| 限制   | 当前 runtime 仍不发送 terminal command，因为尚无 carrier 调用此 mapper 或 authorization kernel。本切片自身不执行 default resolution，也不增加 Host framing、socket I/O、activation、retry、reconciliation 或 Terminal Protocol change。                                                                                                                                                                                                                                           |

| Authorized input | Terminal Protocol request | Successful Host response                           | Kernel completion                                                          |
| ---------------- | ------------------------- | -------------------------------------------------- | -------------------------------------------------------------------------- |
| Immediate Launch | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity；kernel 从 authorized complete geometry 派生 `Started`   |
| Deferred Launch  | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity；kernel 派生 `AwaitingStart`                             |
| Start            | `StartTerminal`           | `TerminalStartAccepted` with exact target/geometry | Accepted identity、geometry 以及 `started` 或 `already-started` transition |

## 本机 Host broker 建模记录

上文的当前 Rust process boundary 没有改变：它在 Host `HelloAccepted` 后停止 protocol activity，
发出 `ready\n`，且不发送 terminal command。扩展后的已检查 model 是下一 adapter 切片的集成前
contract，而不是此行为已经实现的证据。

| 字段   | 建模的下一切片契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | Server-first 双边 admission 完成后，未来 Worker carrier 必须把一个已经由 authorization kernel 接纳的 dispatch 转换成一次 Terminal Host effect，并返回其精确 OBCN result。它不得建立第二套 authorization decision，不得把其他 request 或 Host incarnation 的 response 混入，也不得隐去一个可能已经应用的 effect 是否可以安全 retry。                                                                                                                                                                                                                  |
| 输入   | 已接受的 Server 与 Host link；来自单独检查的 authorization kernel 的一个已接纳 OBCN Launch 或 Start dispatch；其私有 dispatch identity；在 `begin_host_handoff` 观察到的新鲜 Host epoch；一个新生成的非零 Terminal Protocol correlation；一个精确 Host response、timeout 或 link loss；以及一次有界 typed OBCN result delivery 尝试。Worker loss 后也可能观察到 retained Host terminal。                                                                                                                                                             |
| 数据   | 一个 process-local broker phase、一个 operation phase、Launch/Start kind、一次 handoff count、一个已映射 Host command 与 correlation、Host epoch、Host command 是否可能已应用、一个 completion classification、一个有界 result slot 与 delivery count、unknown outcome 的 retry barrier，以及 retained-terminal disposition。不引入无界 queue 或 durable state。                                                                                                                                                                                     |
| 输出   | Launch 只映射到 Host `CreateTerminal`；Start 只映射到 Host `StartTerminal`。每个 admitted dispatch 都会得到一个 accepted、rejected、failed 或 outcome-unknown classification。Server 仍可用时，其 OBCN result 会进入队列，并且只会交付一次，或在有界 delivery failure 后被显式 abandoned；Server loss 会立即 abandon 已分类 result。Unmatched retained Host terminal 会变为 recovery-required，绝不被静默接纳为当前 Worker state。                                                                                                                   |
| 不变量 | Server admission 先于 Host contact。Readiness 要求两条 link 都已接受。`begin_host_handoff` 先于第一个可能的 Host effect，且最多 dispatch 一个 Host command。已知 Host completion 必须具有预期 response role 与 kind、operation correlation 以及已观察的非零 Host epoch。Malformed 或 mismatched response 会 fail closed 为 unknown。Unknown 绝不 retry。Worker stop 后不持有 link。Coupled lifecycle 的唯一例外是 Host loss 后的有界 result drain。                                                                                                  |
| 决策   | Server 仍可用时，handoff 前的 failure 是确定的 `Failed(HostUnavailableBeforeHandoff)`。Host rejection 与 typed failure 分别保持为 `Rejected` 与 `Failed`。Handoff 后的 loss、timeout 或 invalid response 是 `OutcomeUnknown`，即使 model 还未观察到 Host apply step。Server loss 会立即关闭 Host，把任何未观察到的 in-flight dispatch 保守分类为 unknown，并 abandon 任何未交付 result；Host loss 会关闭 Host，但只可在一次有界 result delivery 所需时间内保留 Server。Worker loss 后的 retained state 需要显式 recovery。                           |
| 模型   | [`LocalHostBroker`](formal/tla/LocalHostBroker.tla) 保留当前 Server-first handshake 与 coupled link lifecycle，然后把来自 Server-owned [`TerminalLaunchAuthorization`](../orchbee-server/formal/tla/TerminalLaunchAuthorization.tla) abstraction 的一个已接纳 dispatch 作为输入。它对从 handoff 到 Host response 与 OBCN result delivery 的 adapter ordering 建模；既不重新实现 authorization，也不改变任一 wire contract。                                                                                                                          |
| 转换   | 下表覆盖 admission、一个 Launch/Start dispatch、pre-handoff failure、handoff、command 发出与可能 apply、严格关联的 Host completion、所有独立排序的 Server/Host loss、有界 result delivery、unknown retry barrier，以及 retained-terminal recovery classification。                                                                                                                                                                                                                                                                                   |
| 依赖   | 未来 runtime adapter 可以使用现有 authorization kernel、OBCN semantic value 与 codec、local-Site carrier、Terminal Protocol Rust binding、authenticated socket 与 bounded timer。Model 把 decoded value 与 liveness observation 当作输入。它不选择 carrier byte、deadline、queue implementation、activation、shell、working directory、PTY behavior 或 reconciliation policy。                                                                                                                                                                       |
| 所有权 | Server 继续拥有 placement、authorization issuance、Worker generation 与 reconciliation。OBCN 拥有 operation 与 result contract。Worker 拥有本机 ordering、correlation check、单 operation/result bound、loss classification 与 link cleanup。Terminal Protocol 拥有 Host wire role 与 message。Terminal Host 仍是 terminal 与 process effect 的唯一所有者。                                                                                                                                                                                          |
| 验证   | TLA inventory 注册每个具名 admission、handoff、command、response-classification、outcome、delivery、独立排序 loss、retry-block 与 recovery action，其 trigger 覆盖 Worker、OBCN、local-Site 与 Terminal Protocol implementation surface。TLC 在 weak fairness 下检查 safety 与 finite progress。现有 process test 仍只为 zero-command runtime 提供证据；后续实现必须增加 deterministic adapter/process refinement test，才能声称该 contract 已运行。                                                                                                 |
| 正确性 | Model 检查 Server-first admission、双 link readiness、stopped-link cleanup、单 dispatch 与 command、handoff-before-effect、strict response correlation、accepted-after-apply、unknown-after-handoff、closed result phase、at-most-once delivery、unknown 后 command count 不变，以及 retained-terminal 不被接纳。Progress check 要求 pending admission、operation、result 与 drain phase 最终稳定，并要求 unmatched retained state 变为 recovery-required。这些是 bounded-state claim，并非 Rust code、remote Server、Zig Host 或任一 codec 的证明。 |
| 限制   | State space 包含一个 Worker、一个已接纳 operation、一个 Host epoch、一个 semantic correlation、一个 result slot，且最多一次 delivery。它抽象 byte framing 与数值 timeout 时长。它尚未实现 OBCN carrier-to-kernel call、Host command mapping、response decoding、timer、result write、reconnect、inventory 或 recovery；这些需要后续已授权切片的 runtime code 与 test。任何 production path 都不得仅根据此 model 推断 retained terminal 属于 replacement Worker。                                                                                     |

| 当前状态                        | Observation 或 action                                 | 下一状态                 | Result 与 lifecycle 规则                                                                            |
| ------------------------------- | ----------------------------------------------------- | ------------------------ | --------------------------------------------------------------------------------------------------- |
| Server admission 前             | Host connect attempt                                  | Forbidden                | 没有启用 Host contact transition。                                                                  |
| Ready，无 operation             | 接纳一个 Launch 或 Start dispatch                     | `admitted`               | 保留精确 admitted kind；第二个 dispatch 超出单 slot bound。                                         |
| `admitted`                      | `begin_host_handoff` 前 definite failure 或 Host loss | `result-ready`           | Queue failed；不存在 Host command。                                                                 |
| `admitted`                      | 使用已观察 Host epoch 调用 `begin_host_handoff`       | `handed-off`             | 恰好一次越过具名 external-effect boundary。                                                         |
| `handed-off`                    | 使用 operation correlation 发出 mapped command        | `awaiting-host`          | 最多发送一个 Host command；在 response 可观察之前，Host 可能已经 apply。                            |
| `handed-off` 或之后             | Host loss 或 timeout                                  | `draining-result`        | Queue outcome unknown，关闭 Host，阻止 retry，且仅为有界 result delivery 保留 Server。              |
| `awaiting-host`                 | 精确 accepted、rejected 或 typed-failed Host response | `result-ready`           | 只接受预期 role/kind、operation correlation 与已观察 Host epoch；queue 相应 typed OBCN result。     |
| `awaiting-host`                 | 错误 role、kind、correlation 或 Host epoch            | `draining-result`        | Fail closed 为 outcome unknown，关闭 Host，并阻止 retry。                                           |
| 任意 live operation/result      | Server loss                                           | `stopped`                | 立即关闭两个 link；未观察到的 in-flight dispatch 为 unknown，所有未交付 result 都被显式 abandoned。 |
| `result-ready`                  | Result write 成功或其有界机会到期                     | `completed` 或 `stopped` | 恰好交付一次，或 abandon 并关闭；Host-loss drain 总会终止。                                         |
| `stopped`，有 retained terminal | Recovery classification                               | `recovery-required`      | 为显式 Server/Host reconciliation 保留 unmatched disposition；不存在 Worker-adoption transition。   |

Runtime refinement 必须保留 authorization kernel 作为唯一 dispatch gate，在任何可能到达 Host 的 send
path 之前立即调用 `begin_host_handoff`，并且只为该 private dispatch 分配 Host correlation。只有检查过
response role、预期 response kind、精确 correlation 与 handoff Host epoch 后，才能解码 completion。
该 boundary 后的任何不确定性都应视为 outcome unknown，通过 kernel 缓存，且绝不自动重发 command。
Result drain 应保持 single-slot 且有界；Server loss 仍立即关闭两条 link，而 Host loss 只允许 model
规定的 delivery drain。Worker death 后观察到的 inventory 必须进入显式 recovery 或 reconciliation，
不得在正常启动时直接接纳。

## 所有权

[`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.zh-CN.md) crate 拥有
Server-Worker framing。
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.zh-CN.md) crate 拥有两条
本机 connection 使用的 kernel same-user proof。
[`orchbee-terminal-protocol`](../../packages/terminal-protocol/references/protocol.zh-CN.md) package
拥有 Host wire；其独立 Rust binding 提供本次消费的 envelope 与 connection codec。Capability-node
protocol crate 拥有 OBCN semantic value 与 framing，不导入 local-Site 或 Terminal Host type。Worker
拥有 admission order、Host broker connection、coupled liveness、尚未集成的 deterministic
validation/replay/dispatch kernel、有界 default interactive launch resolution，以及 pure Terminal Host
launch adapter。Server 仍是唯一 placement、authorization-issuance 与 reconciliation owner。Terminal
Host 仍是唯一 PTY、process-tree、VT、history 与 attachment authority。

[`LocalSiteProcessSpine`](../orchbee-server/formal/tla/LocalSiteProcessSpine.tla) model 继续拥有
Server admission 与 Worker-generation behavior。`LocalHostBroker` 把 Server-first admission
保留为一个 broker 的 ordering precondition；它不重新定义 generation，也不增加第二个 Server
或 Host authority。Server-owned
[`TerminalLaunchAuthorization`](../orchbee-server/formal/tla/TerminalLaunchAuthorization.tla) model
仍是 abstract authorization authority；本 Worker module 是其 local admission、handoff、cache 与
result action 的 concrete deterministic refinement，而不是 competing model。
该 model 现在使用 capability version 2 表示 admitted authority，version 1 只作为 unsupported
request witness。
Worker-owned [`WorkerTerminalBindingCapacity`](formal/tla/WorkerTerminalBindingCapacity.tla)
将 generation-local cross-terminal binding bound 与 rejection priority 和 global operation/result
slot 组合起来；其 TLC configuration 使用 capacity two 加一个 unseen identity，不声称 exhaustive
检查 production value 1,024。Model authority 与其 Rust enforcement 是同一个 additive、unshipped
delivery cohort；model-only commit 不是可独立 ship 的 boundary。

## 验证

从仓库根目录运行。真实 Host process witness 使用正常 Terminal Host build artifact，所以执行聚焦
Cargo test 前要先构建它；根 merge gate 本身已经先 build 再 test。

```sh
pnpm --filter @orchbee/terminal-host build
cargo test -p orchbee-worker --all-targets --all-features --locked
cargo clippy -p orchbee-worker --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-capability-node-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-capability-node-protocol --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-local-peer-credentials --all-targets --all-features --locked
node scripts/tla/run-models.mjs --models local-site-process-spine,local-host-broker,terminal-launch-authorization,worker-terminal-binding-capacity
pnpm verify
```
