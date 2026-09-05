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

只有两个对端均接受后，Worker 才向 stdout 写入 `ready\n`。随后主线程在两个自有 socket 上
运行串行 OBCN Launch/Start、查看与控制附件 request loop，不创建 connection thread 或 operation queue；
每个查看附件拥有一个有界交付槽。Server loss 关闭
Host broker socket；Host loss 停止准入，只允许对已分类 result 做有界交付尝试，然后关闭 Server
attachment。任一 peer 丢失均静默以 status `1` 退出；retained terminal 继续由 Host 拥有。Invalid
invocation、connection、authentication、handshake rejection、framing、output 或 transport failure
也采用同样的静默 status-`1` 结果。合法的 operation-level rejection 返回 typed OBCN result，并让
loop 继续接收下一请求。

Worker 只消费已准入 same-user Server 的 authorization，并用 OBLS generation 构造授权 kernel。
只有 local admission 与新的 handoff check 通过后才发送 Create/Start。它不执行 retry、discovery、
activation、daemonization、child-process supervision、terminal inventory、PTY
operation、Agent behavior 或 durable write。Server Client Create/Start authorization 已集成。
Endpoint selection、service supervision、Host activation、Server Client Session 查看附件路由、
裸入口 TUI 激活与 reconciliation 仍属于后续边界；裸 `orchbee` 尚不是可用的 terminal Client。

## 实时查看附件转发细化

已接受的本地终端里程碑现在将 `LocalTerminalRelay` 细化到 Worker 的单一 Host broker。
Server 仍是查看附件的唯一签发方。每个绑定代际的查看附件只有读取能力、独立的有界交付槽，
并且最多只有一帧等待 Client 精确反馈。Worker 校验信封和固定路由元数据，同时保留完整的
不透明 Host 记录；不解码终端行、VT 或历史。附着、轮询、反馈和分离与 Launch/Start 一起在
已接纳的 Server 连接上串行执行。轮询交付已就绪记录，不等待终端输出；下游交付不能占住
Worker 操作槽。

OBCN 查看附件载体和 Worker 附件转换现已服务于 Server 已准入 Client Session 转发，并有真实
Host 进程轨迹。Server 拥有 OBCS 查看消息与 Session 清理；裸入口 TUI 激活仍是后续集成边界。八个存活查看附件和每附件一帧保留记录属于实现边界，不是发布性能
预算。缺失或矛盾的反馈绝不确认 Host 帧。`NeedSnapshot` 保留原因和序号；
`ProjectionBlocked` 仍是附件事件。
交付槽已满时，只关闭该查看附件并释放排队字节；它的 Host detach 在下一个 Server operation
之前执行，关闭中附件的迟到 event 被排空，不再保留额外帧。绝不静默替换 blocked event 或帧。
Broker 的 `host_connection` module 拥有 event 路由、查看附件槽、controller grant 和共享 deadline exchange；
`terminal_viewer_operation` 映射已准入的只读 command，并校验精确 Host response。
查看附件 state borrow 不跨越 Host I/O。Attach 依次执行两个有界 exchange（capabilities，随后
激活 projection demand）；cleanup 可在请求前执行最多十六个有界查看／控制 detach exchange。查看附件 carrier 现为这些 exchange 和 OBNR record delivery 提供二十秒预算。Lifecycle/metadata notification 经校验后排空；
bell、clipboard 与 application-signal effect 不经此只读投影路径对外发布。分离和 broker 丢失释放临时附件并保留终端。
重新附着必须使用 Server 新签发的附件身份；此只读查看路径不授予输入或调整大小权限。

证据将现有权限／转发模型映射到确定性路由、精确反馈、过期作用域和容量测试，以及包含
两个查看附件、停滞查看附件、快照反馈、分离、重新附着和终端保留的真实 Worker/Host 轨迹。
线协议测试检查有界分帧和不透明字节保留。当前上下文内的挑战检查不构成独立审查证据。

## Controller grant 与副作用

`terminal_control_operation` 消费 Server 签发的 OBCN 获取、输入、调整尺寸和内部释放请求。
`host_connection::controllers` 拥有有界纯 grant 状态：每个 Host terminal 一个 controller、精确
viewer 和 controller 身份、待定或已接受的 Host capability revision，以及已消耗的最高 sequence。
Grant state borrow 不跨 Host I/O。查看和控制 adapter 均使用 `host_connection` 所属的精确关联、
有 deadline 的 exchange。

获取要求存活的只读 viewer，并创建独立 Host 附件，禁用 read、启用 input 和 resize，并申请
resize lease。它保留该 viewer 的 projection baseline。只有精确 capability acknowledgement 才
提交 grant。Input 保留 1 至 4,096 个原始字节并使用已接受的 capability revision；Host 必须确认
完整 byte count。Resize 必须确认完整 geometry。两个 action 均在 Host I/O 前拒绝错误的
viewer/grant 或非递增 sequence。已知 Host rejection 消耗 sequence；含糊或矛盾的副作用结果
关闭 broker 且不重放。

Viewer detach 或关闭先释放其 controller，再 detach viewer。仅当该 viewer 没有 controller 时，
内部 release 才是幂等的；它绝不清除不同的当前 grant。意外的 resize lease 撤销让 broker 退役。
Release 或 loss 均不提升其他 viewer，也不终止保留执行。八个 viewer 最多允许八个额外控制
附件；cleanup 与普通操作共用单一 Host writer。

确定性 grant 测试覆盖互斥、过期作用域、待定获取、sequence 耗尽和显式重新获取。故障进程测试
覆盖 Host 输入确认静默或矛盾且不重放。Server 的真实双 Client 进程见证验证 Shell 输入、实际
PTY geometry、非 controller 拒绝、detach，以及另一 Client 获取全新 grant。这些运行时见证细化
现有 authority/relay 模型；它们不证明公开 TUI 行为、独立审查或实测发布性能。

## 默认交互式 launch 解析

Application-private `default_interactive_launch` module 为运行中的 request loop 提供 launch-fact
composition。其 transport-neutral `DefaultInteractiveLaunch` output 只持有一个已解析 executable、
恰好为空的 literal argument vector，以及三个固定 environment entry。Loop 将它们与 Worker-local
cwd、Terminal Protocol profile、geometry 和独立 creation identity 组合。三个 entry 映射到 Terminal
Protocol launch environment；已接受首个 milestone 的 invocation-scoped
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

Application-private `terminal_authorization` module 是运行中 request loop 使用的、供私有 Host
launch adapter 使用的 carrier-neutral deterministic kernel，消费
[`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.zh-CN.md) 的
semantic Launch 与 Start value。Loop 为每个已准入 Server attachment 构造一个 kernel。

Composition root 只用当前 local-Site `WorkerId` 与 OBLS 接受的非零 Worker generation，
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
| 问题   | OBCN carrier 可能重复、延迟、针对错误 Worker generation 或在 authority 已前移后交付 Server-authorized Launch 或 Start。Worker 必须在本地决定是否可以开始恰好一次 Host operation，并保留精确 terminal result，同时不取得 Server placement authority，也不自行执行 Host effect。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| 输入   | 来自经过认证且遵守全局 fresh `OperationId` producer invariant 的 authoritative Server 的 canonical OBCN `AuthorizedTerminalLaunch` 或 `AuthorizedTerminalStart`，其 authorization capability 会根据 kernel 内置的 version 2 policy 检查；caller 提供的当前 local-Site Worker ID、非零 OBLS generation 与当前 Unix-millisecond time；Start admission 与 handoff boundary 再次观察到的 Host epoch；以及 typed Host completion。缺失 result 或 occupied slot 保持为显式 state，而不推断成 success。                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 数据   | `AuthorizedHostOperationKernel` 为每个 Worker generation 最多持有 1,024 个完整 highest-fence binding，以 product `TerminalId` 为 key，并包含 operation kind、完整 authorization、Launch 或 Start payload，以及可选的精确 cached result。它另外持有一个处于 `empty`、`awaiting-handoff`、`handed-off` 或 `result-ready` 的 process-local slot，以及 maximum-observed Unix-millisecond high-water mark。私有 `AuthorizedHostDispatch` 保留完整 admitted operation，因此也保留其 generation 与 authority binding。                                                                                                                                                                                                                                                                                                                                                                                                                            |
| 输出   | Admission 返回 `Dispatch`、`ResultReady` 或 `SlotOccupied`；handoff 返回 `HandedOff` 或 `ResultReady`；`take_result` 返回一个 typed OBCN Launch 或 Start result，其中在达到 retained-binding limit 后，对有效且未见过的 terminal 返回 `CapacityExceeded`；非法 completion observation 返回 typed internal error，且不改变 slot。内核不开 carrier、不执行 Host call、不发 product event，也不写 durable state。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| 不变量 | Wrong Worker、stale generation、unsupported capability、expired new lease、lower fence、conflicting same-fence binding，以及超出 capacity 的有效 unseen terminal 绝不 dispatch。Exact binding 最多 dispatch 一次，且即使 expiry 后或处于 capacity 时也只 replay 其 cached exact result。Kernel 一旦观察到 lease expired，之后较低的时间不能使它复活。即使在 capacity 时，有效 higher fence 也会替换 prior binding 并清除其 cache。Capacity rejection 绝不插入 tombstone，也不改变任何 binding。Global slot 最多由一个 operation 或 undelivered result 占用。只有 `handed-off` 才能变成 Host outcome。                                                                                                                                                                                                                                                                                                                                      |
| 决策   | 分类顺序为 occupied slot、Worker identity、Worker generation、exact cached replay、capability、lease expiry、per-terminal fence、unseen-terminal capacity，最后是 Start target epoch。因此在 capacity 时，unsupported 或 expired new request 保留其 capability 或 lease 分类；现有 lower 与 same-fence request 保留其 fence 分类；只有有效 unseen terminal 会收到 `CapacityExceeded`。在 Start admission 与 handoff 再次检查时，target epoch mismatch 变为 `HostIdentityChanged`；对 unseen terminal，capacity 先于该检查。Handoff expiry 会阻止 effect，accepted Start identity、epoch 与 geometry 必须保持精确。                                                                                                                                                                                                                                                                                                                         |
| 模型   | 一个 deterministic state owner 将 generation-local、1,024-entry 的 per-terminal monotonic binding map 与一个 globally bounded operation/result slot 组合起来。`admit_launch` 与 `admit_start` 是 principal entry point；`begin_host_handoff` 是命名的 external-effect boundary；typed observation method 会关闭 accepted binding 并排队其 result。[`WorkerTerminalBindingCapacity`](formal/tla/WorkerTerminalBindingCapacity.tla) 将 finite threshold 加一个 unseen identity 的 cross-terminal capacity refinement 与最小化的 empty、operation-pending 和 result-ready slot abstraction 组合起来。                                                                                                                                                                                                                                                                                                                                         |
| 转换   | 下方 transition table 定义 admission、handoff、completion、exact replay、result delivery、conflict 与 stale-token behavior。接受 binding 前的 rejection 会排队一个 uncached result；接受 binding 后的每个 terminal outcome 都会在公开前先缓存。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| 依赖   | 仅依赖 Rust 标准库、canonical local-Site Worker identity，以及 OBCN semantic type 与 constructor。Time、socket liveness、decoded framing、current Host epoch 与 Host observation 均由 operation adapter 以 value 形式提供；kernel 会单调化 supplied time，但绝不读取 clock。它不读取 environment、filesystem、socket、process、profile、executable、shell 或 working directory，也不解析或编码 Host framing。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
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

Application-private `terminal_host_launch_adapter` module 由运行中的单次 Host exchange 消费。
它是一个 pure value mapper，在一项已接纳的 `AuthorizedHostDispatch`、现有 Terminal Protocol
request/response value，以及 `terminal_authorization` 已接受的 Host observation 之间转换；它既不打开
Host socket，也不改变 kernel state。

### Launch-adapter 建模记录

| 字段   | 当前契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 已接纳的 OBCN Launch 或 Start 必须通过现有 Terminal Protocol，而不能让 carrier 获得虚构 process、directory、environment、identity、geometry 或 outcome 的 authority。                                                                                                                                                                                                                                                                                                             |
| 输入   | 一个私有 admitted dispatch；仅 Launch 还接收注入的 `ResolvedTerminalLaunchFacts`，其中包含 stable Host `CreationId`、此前已解析的 executable、literal argument、durable launch environment、cwd 省略时的 default、terminal profile 与 optional terminal configuration；或者一个 decoded Host success/`TypedFailure` 加 response envelope 的精确 Host epoch。                                                                                                                      |
| 数据   | Mapper 不保留 state。`ResolvedTerminalLaunchFacts::new` 是 node-local default-interactive-launch resolver 与单独提供的 creation-identity/cwd/profile/configuration fact 之间的狭窄 composition seam；operating-system、account、environment 与 filesystem lookup 均留在本 module 之外。                                                                                                                                                                                           |
| 输出   | Launch 变为 `CreateTerminal`；Start 变为 `StartTerminal`；Host success 或 `TypedFailure` 变为现有 `LaunchHostCompletion` 或 `StartHostCompletion`。Caller 仍负责 framing、I/O、correlation、handoff，以及把 observation 应用于 kernel。                                                                                                                                                                                                                                           |
| 不变量 | Launch 会复制 independently resolved stable Host `CreationId`，而不是从 product `TerminalId` 派生它。Start 发送 OBCN target 的 Host `TerminalId` 并保留其完整 identity，不施加 product/Host byte-equality policy。Explicit cwd 会被精确复制；省略时选用注入的 Worker-user-home default。Immediate Launch 与 Start 保留全部四项 geometry dimension；deferred Launch 不携带 geometry。Caller overlay 始终为空。其他 launch fact 都会精确复制。                                      |
| 决策   | Mapper 将 Terminal Protocol encoder 作为 pure preflight validation 复用，因此 public protocol value 无法绕过 identity、text、count、palette、geometry 或 control-payload bound。Terminal Protocol `TypedFailure.retryable` 控制 retry guidance，而不是 failure kind；因此每个 generic `TypedFailure` 都变为 `HostReportedFailure`。`HostRejected` 需要未来明确的 stable rejection contract。Response kind、identity、epoch 与 geometry contradiction 仍返回 typed adapter error。 |
| 模型   | `Launch(dispatch, facts) -> CreateTerminal`；`Start(dispatch) -> StartTerminal`；`(Launch dispatch, expected CreationId, response epoch, exact response) -> Launch completion`；`(Start dispatch, response epoch, exact response) -> Start completion`。Operation exchange 拥有 transport handoff distinction。每个 function 都是 deterministic 且 side-effect free。                                                                                                             |
| 依赖   | 仅依赖 application-private OBCN authorization type，以及现有 Rust Terminal Protocol semantic value/encoder。Module 不依赖 socket、envelope codec、carrier、clock、process、filesystem、account database、environment reader、shell resolver 或 kernel orchestration。                                                                                                                                                                                                             |
| 所有权 | Default-interactive-launch resolver 拥有 node-local executable 与 durable default environment selection。Composition root 拥有 stable Host creation identity，以及 omitted-cwd、profile 与 optional configuration fact。此 adapter 只拥有 deterministic translation 与 cross-contract consistency check。Terminal Host 仍拥有 PTY/process effect；carrier 拥有精确 envelope/correlation transport；authorization kernel 拥有 replay 与 outcome state。                            |
| 验证   | Unit test 覆盖刻意不同的 product `TerminalId` 与 Host `CreationId` value、immediate 与 deferred Launch、explicit 与 omitted cwd、stable Host creation identity、empty overlay 与 accepted default environment、Start、两种 transition、每种 operation 中两个 `retryable` value 均映射为 reported failure、wrong response kind、identity/epoch/geometry contradiction，以及 exact argument/environment 与 malformed-value bound。                                                  |
| 限制   | Mapper 自身不执行 default resolution，也不拥有 Host framing、socket I/O、activation、retry、reconciliation 或 Terminal Protocol change。                                                                                                                                                                                                                                                                                                                                          |

| Authorized input | Terminal Protocol request | Successful Host response                           | Kernel completion                                                          |
| ---------------- | ------------------------- | -------------------------------------------------- | -------------------------------------------------------------------------- |
| Immediate Launch | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity；kernel 从 authorized complete geometry 派生 `Started`   |
| Deferred Launch  | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity；kernel 派生 `AwaitingStart`                             |
| Start            | `StartTerminal`           | `TerminalStartAccepted` with exact target/geometry | Accepted identity、geometry 以及 `started` 或 `already-started` transition |

## 本机 Host broker 建模记录

已检查 model 管理完整 broker lifecycle。下文的单次 operation exchange 与集成 request loop
细化其 Host effect 和 bounded result-delivery boundary。Executable 现在在两个 handshake 后
调用它们；restart recovery 仍未实现。

| 字段   | 建模的 broker 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | Server-first 双边 admission 完成后，Worker carrier 必须把一个已经由 authorization kernel 接纳的 dispatch 转换成最多一次 Terminal Host effect，并返回其精确 OBCN result。它必须保留 `begin_host_handoff` 期间 kernel 给出的 `ResultReady` decision，验证实际 Host response 而不是 diagnostic label，并且绝不能虚构 wire 未携带的 Host-rejection classification。                                                                                                                                                                                                                                                                                                                                               |
| 输入   | 已接受的 Server 与 Host link 及 handshake Host epoch；一个已经通过 kernel identity、generation、intrinsic capability、lease、fence、1,024-binding capacity 与 global-slot gate 的 OBCN Launch 或 Start `Dispatch`；Start target Host epoch；传给 `begin_host_handoff` 的当前 Host epoch；一个非零 command correlation；实际 Host response role、kind、correlation 与 epoch；timeout 或 link loss；以及一次有界 OBCN result-delivery opportunity。Worker loss 后也可能观察到 retained Host terminal。                                                                                                                                                                                                          |
| 数据   | 一个 process-local broker phase；映射到 kernel 唯一全局 `empty`/`awaiting-handoff`/`handed-off`/`result-ready` slot 的一个 operation phase；Launch/Start kind；handoff-attempt 与 effect-boundary count；Start target 与 observed handoff Host epoch；mapped Host command 与 correlation；保留的 response role、kind、correlation 与 epoch；Host 是否应用 command；精确 completion source 与 cached-result fact；一个有界 delivery count；unknown retry barrier；以及 retained-terminal disposition。不引入 adapter queue 或 durable state。                                                                                                                                                                  |
| 输出   | Launch 只映射到 Host `CreateTerminal`；Start 只映射到 Host `StartTerminal`。`begin_host_handoff` 期间的 lease expiry 返回 cached OBCN `Rejected(LeaseExpired)`；变化的 Start Host epoch 返回带 observed epoch 的 cached `Rejected(HostIdentityChanged)`；两者都不越过 effect boundary。调用前的确定 transport failure 仍是 cached `Failed(HostUnavailableBeforeHandoff)`。精确 success 变为 `HostAccepted`；精确 generic Terminal Protocol `TypedFailure` 变为 `Failed(HostReportedFailure)`。Unknown 与 retained-state outcome 沿用之前的有界 lifecycle。                                                                                                                                                    |
| 不变量 | Server admission 先于 Host contact。Readiness 要求两条 link 都已接受且 Host epoch 非零。最多存在一个 kernel-global operation/result slot 与一个 Host command。成功的 `begin_host_handoff` 先于第一个可能的 Host effect；其 `ResultReady` branch 会保留一次 attempted call，但 crossed boundary、command 与 effect 都为零。已知 Host completion 必须具有实际 `Response` role、request-specific expected kind、精确 command correlation 与精确保留的 handoff Host epoch。精确 generic `TypedFailure` 只能产生 `HostReportedFailure`；mismatch 会 fail closed 为 cached unknown，且两者都不能产生 `HostRejected`。Unknown 绝不 retry。                                                                           |
| 决策   | Dispatch 前的 kernel result（包括 `CapacityExceeded` 与 `SlotOccupied`）保持不同且绝不进入本 adapter model。Handoff 期间，lease expiry 与 Start identity replacement 是精确 cached rejection，而不是 transport failure。当前 Terminal Protocol 不存在 authoritative Host-rejection discriminator，因此相同的精确 generic `TypedFailure` input 绝不被非确定性拆分：它保守映射到 OBCN `Failed(HostReportedFailure)`。OBCN `HostRejected` 留给未来显式 classified owner contract。Post-handoff loss、timeout 或 invalid response 为 `OutcomeUnknown`。                                                                                                                                                           |
| 模型   | [`LocalHostBroker`](formal/tla/LocalHostBroker.tla) 保留 Server-first handshake 与 coupled lifecycle，接收来自 Server-owned [`TerminalLaunchAuthorization`](../orchbee-server/formal/tla/TerminalLaunchAuthorization.tla) 的一个 admitted dispatch，并把详细 `awaiting-handoff`/`handed-off` phase 映射到 `WorkerTerminalBindingCapacity` 的单一 `operation-pending` global kernel slot。其 Host-side effect boundary 与 lost-reply classification refine [`TerminalHostRequestOutcome`](../terminal-host/formal/tla/TerminalHostRequestOutcome.tla) 和 [Terminal Host runtime model](../terminal-host/docs/runtime-model.zh-CN.md)；它不重新实现任何 authority，也不改变 wire contract。                     |
| 转换   | 下表覆盖 admission、两个 `begin_host_handoff -> ResultReady` branch、确定 pre-call transport failure、成功 handoff、command 发出与可能 apply、concrete response validation、保守 generic failure、独立排序的 Server/Host loss、有界 exact-result delivery、unknown retry barrier，以及 retained-terminal recovery。                                                                                                                                                                                                                                                                                                                                                                                           |
| 依赖   | Adapter 依赖 authorization kernel 以及单独检查的 `WorkerTerminalBindingCapacity` bound 与 global slot、OBCN semantic value 与 codec、local-Site carrier、Terminal Protocol binding、Terminal Host runtime outcome contract、authenticated socket 与 bounded timer。Model 把 kernel 的 post-capacity `Dispatch`、decoded Host record 与 liveness 当作输入。它不选择 carrier byte、deadline value、queue implementation、activation、shell、working directory、PTY behavior 或 reconciliation policy。                                                                                                                                                                                                          |
| 所有权 | Server 拥有 placement、issuance、generation 与 reconciliation。Worker authorization kernel 拥有 binding capacity、admission/replay、精确 cached result 与唯一 global operation/result slot。Adapter 只能消费 `Dispatch`，不得增加 parallel slot 或 capacity counter。`WorkerTerminalBindingCapacity` 检查该 bound，并把本 model 的 pre-result adapter phase 折叠到其 `operation-pending` phase。OBCN 拥有 result variant。Terminal Protocol 拥有 response role、kind、correlation、epoch 与 generic `TypedFailure`，但不拥有 generic rejected/failed discriminator。Terminal Host 拥有 request commit/effect/reply ordering。Worker 只拥有 mapping、strict observation、loss classification 与 link cleanup。 |
| 验证   | TLA inventory 要求每个 admission、两个 handoff-result branch、command/effect、concrete response mismatch、accepted/generic-failure outcome、delivery、ordered loss、retry-block 与 recovery action。Trigger 覆盖 kernel 与 carrier surface、两种 Terminal Protocol binding、Terminal Host source 与 runtime model、`TerminalHostRequestOutcome`，以及已接受 topology/capacity authority。TLC 在 weak fairness 下检查 safety 与 finite progress。Host exchange 与集成 request loop 具有下文描述的 socket 和 real-process refinement witness。                                                                                                                                                                  |
| 正确性 | 已检查 invariant 把 operation phase 映射到 kernel-global slot；区分 attempted handoff 与 crossed effect boundary；要求精确 cached lease/identity rejection 与 pre-call failure classification；在有界 delivery 或 abandonment 中保留完整 handoff-result projection；验证已保留 response field；把精确 generic `TypedFailure` 分类为 `HostReportedFailure`、把 mismatched generic failure 分类为 unknown，且不把两者分类为 rejected；并保留此前 one-command、unknown/no-retry、link-cleanup、result-delivery 与 recovery property。这些是 bounded model claim，而不是 Rust kernel、carrier、Host、codec 或 capacity implementation 的证明。                                                                    |
| 限制   | State space 包含一个 Worker、一个 admitted operation、两个 symbolic nonzero Host epoch、一个 operation correlation 加一个 mismatched correlation、一个 kernel-global result slot，且最多一次 delivery。Model 从 dispatch 后开始，因此 pre-admission rejection/replay 与 1,024-binding bound 的枚举留在 kernel 和 `WorkerTerminalBindingCapacity`；显式 slot projection 阻止 adapter 虚构另一个 outstanding operation。Byte framing、数值 timeout duration、failure-code taxonomy、carrier integration、inventory 与 recovery 不在本切片范围内。当前 runtime behavior 或 production adoption 不能由该 abstraction 推出。                                                                                       |

| 当前状态                        | Observation 或 action                                                  | 下一状态                 | 精确 result 与 lifecycle 规则                                                                                        |
| ------------------------------- | ---------------------------------------------------------------------- | ------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Server admission 前             | Host connect attempt                                                   | Forbidden                | 没有启用 Host contact transition。                                                                                   |
| Ready，kernel slot empty        | 消费一个 kernel `Dispatch`                                             | `admitted`               | 把 global slot 映射到 `awaiting-handoff`；capacity、replay、rejection 与 occupied-slot result 不会 dispatch。        |
| `admitted`                      | `begin_host_handoff` 观察到 expired lease                              | `result-ready`           | Cache 并 queue 精确 `Rejected(LeaseExpired)`；记录一次 attempted call，不越过 effect boundary，也没有 Host command。 |
| `admitted` Start                | `begin_host_handoff` 观察到与 target 不同的 Host epoch                 | `result-ready`           | Cache 并 queue 带 target 与 observed epoch 的精确 `Rejected(HostIdentityChanged)`；不执行 Host effect。              |
| `admitted`                      | Handoff call 前的确定 transport failure 或 Host loss                   | `result-ready`           | Cache 并 queue `Failed(HostUnavailableBeforeHandoff)`，与两个 handoff rejection 保持不同。                           |
| `admitted`                      | 使用保留的当前 Host epoch 成功调用 `begin_host_handoff`                | `handed-off`             | 恰好一次越过具名 external-effect boundary，并把 kernel slot 映射到 `handed-off`。                                    |
| `handed-off`                    | 使用一个 operation correlation 发出 mapped command                     | `awaiting-host`          | 最多发送一个 Host command；在 response 可观察之前，Host 可能已经 apply。                                             |
| `awaiting-host`                 | `Response` + expected success kind + 精确 correlation + handoff epoch  | `result-ready`           | Queue cached OBCN `HostAccepted`。                                                                                   |
| `awaiting-host`                 | `Response` + generic `TypedFailure` + 精确 correlation + handoff epoch | `result-ready`           | Queue cached `Failed(HostReportedFailure)`；绝不从同一精确 record 推导 `HostRejected`。                              |
| `awaiting-host`                 | 错误 role、kind、correlation 或 Host epoch                             | `draining-result`        | 保留 concrete mismatched observation，fail closed 为 cached outcome unknown，关闭 Host 并阻止 retry。                |
| `handed-off` 或 `awaiting-host` | Host loss 或 timeout                                                   | `draining-result`        | Queue cached outcome unknown，关闭 Host、阻止 retry，并且只为有界 result delivery 保留 Server。                      |
| 任意 live operation/result      | Server loss                                                            | `stopped`                | 立即关闭两个 link；未观察到的 in-flight dispatch 为 unknown，任何未交付 exact result 都会原样 abandoned。            |
| `result-ready`                  | Result write 成功或其有界机会到期                                      | `completed` 或 `stopped` | 清空 global slot；恰好一次交付 cached result，或在不改变其 classification 的情况下 abandon。                         |
| `stopped`，有 retained terminal | Recovery classification                                                | `recovery-required`      | 为显式 reconciliation 保留 unmatched disposition；不存在 Worker-adoption transition。                                |

Runtime refinement 必须把 capacity、`SlotOccupied`、pre-admission rejection、cached replay 与一个
global slot 留在 `AuthorizedHostOperationKernel`；adapter 只能消费其 post-capacity `Dispatch`，
不得引入 adapter-local operation queue。调用一次
`begin_host_handoff`，并在发送任何 Host message 前检查其返回：原样转发精确 `ResultReady` rejection，
或为一个 command 保留其返回的 handoff epoch。为该 dispatch 分配一个非零 correlation。已知 completion
要求实际 `Response` role、request-specific success kind 或 generic `TypedFailure`、精确 correlation
与精确 handoff Host epoch。在 authoritative classified Host-rejection contract 出现前，generic
`TypedFailure` 只能映射到 `HostReportedFailure`。Effect boundary 后的任何不确定性都应视为 outcome
unknown，通过 kernel 缓存且绝不重发。Delivery 应保持 single-slot 且有界；Worker death 后的 inventory
必须 quarantine 以便显式 recovery，不得在启动时直接接纳。

### 单次 Host 操作 exchange 细化

已接受的本机 terminal milestone 将 `terminal_host_operation` adapter 委派给 Worker owner。
其消费者是运行中的 Worker OBCN request loop：kernel admission 之后，该 loop 提供已解析的
launch fact 与新的非零 Host correlation，执行一个 prepared command，然后交付 kernel 的精确
result，才可准入下一个请求。Server request consumption、默认 cwd/profile/creation-identity
composition 与 result delivery 属于该 loop，而不是此 adapter。

`PreparedHostOperation` 将一个 kernel dispatch 绑定到已验证的 Create 或 Start payload 及其
correlation。Preparation 没有 effect，也不能改变 kernel。`execute` 使用已准入的 Host
connection、已准入的 Server stream、注入的 lease clock，以及调用方提供的正数 I/O budget。
一个 monotonic deadline 为完整 Host write 与 response read（包括 fragmented record）设定上限；
它不选择 release timeout。Kernel state 是唯一的 operation/result slot。不增加 queue、result
cache、retry、identity allocator、shell resolver、terminal inventory 或 VT state。有效 lifecycle 与
metadata event 按下文规则排空，包括 handoff 之前以及 command write 与精确 response 之间；
它们不能完成 operation，并消费同一个 Host deadline。

| 边界             | Runtime 细化与证据义务                                                                                                                                                                                                                                                                         |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Handoff 之前     | 不消费 Server byte 地检查两个 peer。观察到 Server loss 或意外 Server data 时关闭两条 link 并放弃 pending operation。Host loss 或 invalid unsolicited Host data 会关闭 Host，并记录 kernel 的精确 pre-handoff failure（或 expired-lease rejection）。Preparation 与 invalid budget 不发送内容。 |
| Handoff          | 使用保留的 handshake epoch 与新采样的 lease clock 调用一次 `begin_host_handoff`。它的 `ResultReady` 分支不发送内容，保留精确缓存的 lease 或 Host-identity rejection。只有 `HandedOff` 才允许第一次 write。                                                                                     |
| Response         | 在解码 operation-specific success 或 generic `TypedFailure` 之前，要求精确 protocol、mandatory control encoding、Response role、command correlation 与 handoff epoch。现有 mapper 验证 payload identity 与 geometry。Generic failure 只映射为 `HostReportedFailure`。                          |
| 不确定性         | Handoff 之后的任何 write/read/timeout/invalid-response failure 都记录 kernel 的精确 unknown result，并关闭 Host。观察到 Server loss 时关闭两条 link；绝不重发 command。                                                                                                                        |
| Result ownership | Success 与每种已分类 failure 都保留在 kernel slot。调用方必须交付或放弃 result，并遵守 `HostClosed`/`ServerClosed`；此 adapter 从不清空 slot，也不声称已交付 OBCN。                                                                                                                            |

此 module 只依赖 `host_connection`、`terminal_authorization`、
`terminal_host_launch_adapter` 及其 protocol value。它的私有 `transport` child 拥有 nonblocking
byte I/O，并在每次 Host wait 时同时 poll Server；不导入 product policy。这些依赖方向都是单向。
运行中的 request loop 调用此 module。

Deterministic socket witness 将上表映射到现有 `LocalHostBroker` 的 handoff、exact completion、
mismatch、ordered-loss 与 unknown/no-retry action。使用真实已构建 Zig Host 的 witness 检查
deferred Create 与 Start、稳定 Host identity，以及 broker loss 之后保留的 inventory。
此 exchange 将 delivery 留给 request loop；recovery 仍在 runtime refinement 之外。这里没有需要
Lean 证明的新 pure transformation：映射仍由现有已检查 mapper 拥有；有限 protocol partition 与
受控 socket timeline 提供 adapter 证据。真实 Host witness 只证明其运行所在的 native operating
system，不证明完整 Client path。

| Claim                                  | TLA+ property                                                                       | Runtime witness（`terminal_host_operation::tests`）                                                                                                                                                   |
| -------------------------------------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Handoff rejection 不发送内容           | `HandoffResultReadyIsExact`, `BeginHandoffPrecedesEveryHostEffect`                  | `handoff_rejections_and_invalid_preparation_never_send_a_command`                                                                                                                                     |
| 精确 completion 与唯一 result slot     | `HostCompletionIsStrictlyCorrelated`, `KernelGlobalSlotRefinesOperation`            | `exact_launch_success_remains_in_the_kernel_slot_and_replays_without_an_effect`, `start_responses_preserve_transitions_and_reject_contradictory_payloads`                                             |
| Generic Host failure 保持 failed       | `GenericTypedFailureIsNeverRejected`                                                | `exact_generic_failure_is_failed_even_when_its_diagnostics_say_rejected_and_retryable`                                                                                                                |
| Invalid response 为 unknown 且不 retry | `MismatchedHostResponseCannotProduceKnownOutcome`, `UnknownOutcomeNeverRetriesHost` | `malformed_response_partitions_close_host_cache_unknown_and_never_resend`                                                                                                                             |
| 有序 loss 关闭对应 link                | `StoppedWorkerOwnsNoLink`, `ObservableUnknownBeginsAfterHandoff`                    | `observed_host_loss_before_handoff_is_a_definite_failure`, `server_loss_before_handoff_abandons_without_sending_or_inventing_a_result`, `server_loss_interrupts_a_waiting_host_and_closes_both_links` |
| Deadline 与保留执行                    | `PendingOperationEventuallySettles`, `RetainedHostIsNeverSilentlyAdopted`           | `a_silent_host_is_bounded_by_the_operation_deadline`, `real_zig_host_creates_starts_and_retains_the_same_terminal_after_broker_loss`                                                                  |

## 已集成的 authorized request loop

`terminal_operations` 在私有 Worker 内组合已接受的 local-terminal milestone。输入为已准入
Server stream、已准入 Host broker、local Worker identity 与 OBLS generation。它拥有串行 operation
lifetime；`terminal_authorization` 仍是唯一 launch binding/cache/operation authority。查看／控制请求使用 Host broker
附件状态及各自 operation adapter。私有
`server_transport` child 将 canonical OBCN `Read`/`Write` codec 适配为 nonblocking socket 与 peer
observation；私有 `launch_facts` child 解析 native launch input。两个 child 都不获得 Server placement
authority 或 Host process state。

Refinement 为 `idle -> bounded request read -> kernel admission -> optional preparation and
Host exchange -> bounded exact result delivery -> idle`。Cached replay 或 rejection 跳过 preparation
与 Host I/O。Result delivery 完成前不准入下一请求。Malformed、wrong-direction、oversized、
partial-timeout 与已观察到的 pipelined request 会关闭 attachment；codec 将 OBCN payload allocation
限制为 8 KiB。Idle wait 没有 timeout；一旦可观察到首个 record，其全部 fragment 共享两秒 monotonic
budget。Result write 也共享一个两秒 budget。两种 wait 均观察 Host loss；result delivery 中的 Host
loss 保留这次有界交付尝试，随后结束 Worker；Server loss 放弃 delivery 并关闭 Host。

每次实际 Host exchange 的完整 write/read budget 为一秒，为 Server carrier 二十秒 result wait
内的结果交付留出时间。这是实验性本机上限，不是经测量的 release performance claim。本机
account/filesystem resolution 是同步调用，不在 socket deadline 内，因此缓慢或失败的解析仍可能
使 Server outcome unknown。Preparation failure 在任何 Host command 之前关闭 attachment，不虚构
Host-reported failure。不可用或不可表示的 wall clock 使 lease 过期；kernel 仍阻止 time rollback
恢复已过期 authority。

对于已准入 Launch，explicit absolute cwd 按字面保留，不要求查询 account home。省略时读取
effective user 的 account home，忽略 inherited `HOME` 与 Worker startup cwd。已有 shell resolver
提供 executable、空 arguments 与三个固定 launch environment entry。Configuration 缺省，transient
overlay 为空。Baseline Terminal Protocol profile 与已有 Orchbee default 一致：light scheme、
`0xfcfcfc` background、`0x141414` foreground、非闪烁 block cursor 与 `0x57bef8` cursor color，
16 个 ANSI color 后接 216-color cube 和 24 个 gray，conformance level 62 与 feature 22，以及
`Orchbee` version response。此私有 native default 不导入 Desktop source，不增加 product Launch
Profile 或 appearance preference。

Host 还会发送 unsolicited `TerminalStarted`、`TerminalExited`、`TerminalFailed` 与
`TerminalMetadataChanged` event。私有 `host_connection::events` adapter 校验其 Event role、
缺省 correlation、精确 protocol/epoch、payload codec 与匹配的 identity epoch，随后排空这些
notification，不维护 lifecycle/metadata projection。Broker 将查看附件帧和 blocked event 路由
到独立槽，也处理与 Launch/Start response 交错到达的 event。Event 绝不确认 operation 或接纳
retained terminal。Response-like、unknown、malformed 或 wrong-epoch event 均 fail closed。
每个 event 的 read budget 为一秒，并受 active OBCN deadline 限制。Allocation 上限为 OBTP
header 加 36 MiB frame limit；envelope 仍执行 64 KiB control limit。有效但被丢弃的 lifecycle
event 仍是 symbolic operation model 中不改变 state 的步骤。

Creation 与 command correlation identity 分别独立从 POSIX OS random source 读取 128 bit。
零值或读取失败使 preparation fail closed；绝不以 product Terminal identity 作为 Host creation
identity。Resolved fact 仅存于一个 prepared operation；replay 使用 kernel 的精确 cached result，
不重新解析 launch，也不新建 Host terminal。Start 不解析 shell 或 cwd，保留精确授权 Host target
与完整 geometry。Worker 重连后不会接纳 retained Host inventory。

`tests/process/terminal_operations.rs` 的 process witness 覆盖真实 Worker 与 Zig Host 的 deferred
Create、Start、immediate Create、exact replay，以及 Server/Worker loss 后保留的两个 terminal；
fake Host 检查省略/显式 cwd、launch environment、profile 与独立 identity。其他 partition 覆盖
无 command 的 authorization rejection、wrong-direction/oversized/partial/pipelined OBCN，以及
silent Host 返回一次精确 unknown result 后断开，以及 Host 在 response 后立即关闭时仍交付精确
result。这些 witness 细化已有 `LocalHostBroker` 的
admission、delivery、loss 与 no-retry claim；framing 与 OS fact 仍是 symbolic state space 之外的
具体假设。它们不证明 Server Client Session 查看附件路由、activation、restart reconciliation 或其他 OS
platform。查看附件 process trace 单独证明真实 Worker 与 Host 上的两个读取附件、停滞查看
附件、精确反馈、快照恢复、分离／重新附着，以及终端保留。

## 所有权

[`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.zh-CN.md) crate 拥有
Server-Worker framing。
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.zh-CN.md) crate 拥有两条
本机 connection 使用的 kernel same-user proof。
[`orchbee-terminal-protocol`](../../packages/terminal-protocol/references/protocol.zh-CN.md) package
拥有 Host wire；其独立 Rust binding 提供本次消费的 control、envelope 与 routing codec。Capability-node
protocol crate 拥有 OBCN semantic value 与 framing，不导入 local-Site 或 Terminal Host type。Worker
拥有 admission order、Host broker connection、coupled liveness、已集成的 deterministic
validation/replay/dispatch、有界 default interactive launch resolution、Host operation mapping，
以及临时查看附件 relay state 和绑定 grant 的控制副作用。Server 仍是唯一 placement、authorization-issuance 与 reconciliation owner。Terminal
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
node scripts/tla/run-models.mjs --models local-site-process-spine,terminal-launch-authorization,worker-terminal-binding-capacity,local-host-broker,terminal-host-request-outcome
pnpm verify
```
