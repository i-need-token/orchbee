# Rust Orchbee Server

这是实验性且尚未发布的私有 Rust Server application。它拥有内存 Workbench projection
kernel、第一条 POSIX 本机 Server-Worker process spine，以及增量的本机 Human Client/Client
Session admission spine。产出的 executable 名为 `orchbee-server`；它参与根 Cargo verification
gate，但不被公开 `orchbee`、Desktop、packaging、installer 或 service manager 消费。

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
cohort-incompatible attempt 都不会改变 active Worker。Disconnect 只移除 matching generation，
因此 delayed cleanup 无法退役后续 Worker。Handshake 在有限 timeout 下同步读取；只有已接受
Worker 获得一个 monitor thread。不存在无界 connection thread 或 message queue。

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
same-user proof。Server 拥有 listener lifetime、Worker admission state、generation 与 cleanup。
[`orchbee-worker`](../orchbee-worker/README.zh-CN.md) application 拥有连接进程 lifecycle。

## 本机 Client Session service

选择私有 Client endpoint 后，每条 same-user compatible Human Client connection 都会创建一个独立
process-local Client Session generation。TUI、Web、Desktop、mobile、tab、window 与 renderer
identity 被有意排除：该 endpoint 准入的是统一的 peer Client model。Server 最多接受八个 active
Session，并最多保留十六个 candidate/session handler thread。每条 handshake 有两秒 read/write
timeout。因此，只要还有 handler slot，一个 silent candidate 就不能阻塞另一条 connection；饱和
时，多余 candidate 会被关闭，且不会分配更多 thread。

经过认证的 candidate 会因 protocol/cohort 不兼容、malformed 或 wrong message、timeout 或
active-Session capacity 收到 typed rejection。准入后的任何 byte（包括重复 hello）只会关闭该
Session。Disconnect cleanup 匹配确切 generation，不能移除另一个或 replacement Session。它不会
停止 Server、干扰独立 Worker registry，也不意味着任何 terminal action。Server process loss 会
关闭每条 Client liveness attachment。

私有 [`orchbee-local-client-protocol`](../../crates/local-client-protocol/README.zh-CN.md) crate
拥有 Client framing 与真实 Rust connecting adapter。Server 拥有 listener、handler-slot、
admission、generation 与 cleanup state。公开 `orchbee` 尚未调用 adapter，因此 bare、help 与
version 行为保持不变。本切片不增加 Workbench command、terminal relay、TUI、activation、
endpoint discovery、persistence 或 remote ingress。

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

## 本机 Client Session 建模记录

| 字段   | 现行 contract                                                                                                                                                                                                                                                                                                                                              |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 已接受的 peer Client model 没有原生 Server connection boundary。本切片必须证明多个独立 same-user 本机 Client Session，且不改变公开 `orchbee`、不增加 terminal message，也不把 Client disconnect 与 Server、Worker、其他 Client 或 terminal lifetime 耦合。                                                                                                 |
| 输入   | 开发 owner 提供的可选 absolute Client endpoint、kernel peer credential 与有界 framed handshake。缺少 endpoint 表示不启动 Client listener。Slow、malformed、wrong-message、wrong-user、incompatible protocol/cohort、capacity、repeated post-admission input、disconnect、stale cleanup 与 Server loss 均显式表示。                                         |
| 数据   | 最多八个 active nonzero process-local Client Session generation、一个 next generation、最多十六个 candidate/session handler slot，以及每条 accepted liveness stream。Server 是唯一 writer。所有 value 都不跨 process exit 保留。                                                                                                                           |
| 输出   | Admission 返回一个 generation；当 stream 与 handler bound 允许时，经过认证的 protocol-level failure 返回 typed bounded rejection。Credential failure 与 handler saturation 会静默关闭。Client disconnect 不产生 product mutation output。                                                                                                                  |
| 不变量 | 每条 accepted connection 有一个 unique generation；active Session 不超过八个；handler thread 不超过十六个；cleanup 只移除 matching generation；rejection 不改变 admitted Session；一个 Client loss 保留 Server、Worker 与所有其他 Session；Server stop 释放每个 Session。                                                                                  |
| 决策   | Listener selection、handler-slot availability、peer-user equality、frame validity、protocol/cohort equality、expected Client message kind、active-Session capacity、next-generation availability 与 matching cleanup generation 完全决定结果。Renderer kind 与 local view state 从不参与。                                                                 |
| 模型   | Deterministic Client Session registry 拥有 bounded admission 与 generation-scoped disconnect。有界同步 Unix-socket handler 负责 authenticate、decode、admit 或 reject、acknowledge，并保留恰好一条 liveness attachment。固定 listener thread 仅通过 sixteen-slot guard 准入工作。                                                                          |
| 转换   | Server `absent -> listening -> stopped`；candidate `none -> slow/rejected/admitted`；Session `none -> active(generation) -> none`；Worker marker 独立 `absent -> registered`。Duplicate post-admission hello 是 scoped disconnect。因为 generation 永不复用，stale cleanup 对每个 current Session 都是 no-op。                                             |
| 依赖   | Rust standard-library process、filesystem、Unix socket、timeout、thread、lock 与 I/O；私有 local Client framing/adapter；以及安全 kernel peer-credential crate。没有 async runtime、network listener、terminal、Workbench command、child process、discovery、activation、persistence、environment lookup 或 randomness。                                   |
| 验证   | Pure codec 与 registry case；原生 same-user socket-pair proof；使用具体 Rust Client adapter 的真实 Server process test，覆盖 concurrent admission、capacity、disconnect/replacement、Server loss、Worker preservation、slow-handshake isolation、duplicate input、malformed/version/cohort rejection、exact readiness、permission 与 cleanup；TLA+ check。 |
| 限制   | Runtime evidence 覆盖同一 effective user 下的 macOS 与 Linux。它不证明 cross-user execution、Windows behavior、公开 Client integration、endpoint discovery、activation、terminal retention、renderer behavior、remote authentication、hostile isolation、graceful Server shutdown 或 production service lifecycle。                                        |

已检查的 [`LocalClientSessions`](formal/tla/LocalClientSessions.tla) model 探索三个 abstract
Client、最多两个 simultaneous Session 与四个 generation。它把 `sessions` 映射到 registry，把
slow candidate 映射到 bounded handshake thread，把 `workerRegistered` 映射到独立 Worker registry
presence，并把 stale cleanup 映射到 generation-scoped disconnect。它检查 safety，不检查 handshake
time progress 或 implementation refinement；deterministic 与 process test 负责这些 mapping。

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

该 application 拥有其注入 in-memory snapshot copy 的 projection mechanics，以及实验性本机
Worker 与 Client Session registry。它不是第二个 current product writer：已准入 Client transport
不承载 product message，没有 Client 或当前 runtime 能提交 projection candidate，也没有
publication 离开该 application。当前 Desktop/Workbench composition 继续作为
已交付 Workbench writer，Node [`apps/server`](../server/README.zh-CN.md) 继续作为既有 Browser
ingress 行为的权威。

Semantic business transition、post-handshake product Client transport、product authorization、
persistence、recovery、external publication、endpoint discovery 与 activation、production service
lifecycle、Terminal Host authority/runtime integration、packaging、migration 与 cutover 都需要后续
implementation slice。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions,local-site-process-spine,local-terminal-client-authority
pnpm verify
```
