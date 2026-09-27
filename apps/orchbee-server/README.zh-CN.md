# Rust Orchbee Server

## Windows 运行时元数据

同用户 OBLC 元数据请求读取本进程的编译版本、保留的预期 cohort、PID、终端协议、实现
特性位和实际托管／停止模式。它在准确 cohort 的 Client 接纳之前执行，不创建 Session
代次或 Workbench／viewer 状态，不申请停止记录租约，并在一次答复后关闭。既有候选及
握手上限仍适用，普通 Client 接纳正在停止时亦然。这些是诊断事实，不是操作授权，也不
证明未来可用性。[运行时状态契约](../orchbee/docs/native-runtime-status.zh-CN.md) 拥有
CLI 行为和证据；[本机 Client 协议](../../crates/local-client-protocol/README.zh-CN.md)
拥有帧格式。只有 Windows 分派此新增读取；旧接纳及 Linux／macOS 运行时行为保持不变。

## Windows 持久会话生命周期

对于原生默认/命名 scope，CLI 激活所有者在启动托管 Server 前发布精确的私有 Active 清单记录。Server 在 Worker 效果前准入并
保留该记录，与检查点存储及生命周期所有者一起持有。确认子进程退出并完成监听器清理后，关闭流程先
发布持久 Stopped，再完成运行时激活记录。发布失败保留恢复不确定性，绝不授予正常停止的成功回执。
运行时 crate 拥有清单编码、路径准入及替换；Server 拥有关闭已确认的判定时机。

## 托管 Windows Site 停止

一个停止标识拥有托管 Server 剩余的生命周期。接受它会原子关闭新应用记录准入；已准入操作及结果写入先结束再清理。
重复开始和状态请求观察保留的尝试；不同标识返回忙。只有相同失败标识的显式重试可开始下一次尝试，最多 64 次。
失败绝不重新开放普通准入，也不授权重放不确定的 Host 变更。每次尝试共用 60 秒协作截止时间，不包括不可中断的
操作系统调用。

所有者保留准确 Worker 子进程，停止 Close 维护，将 Worker carrier 冻结为仅停止流量（排队但未发送的工作结算为
不可用），并发送一次有界 Worker 停止请求。已知无 Host 或 Host 已停止结果，以及准确 Worker 退出，必须先于
Agent 观察和 Client 监听器停止、唤醒并 join 保留的 Client 处理线程、发布已停止激活记录。激活锁排除记录发布和
生命周期锁释放期间的并发重启。管道不存在或响应丢失不能证明完成。控制监听器存活时失败尝试仍可查询；监听器关闭后，
严格的激活记录仍为肯定回执。此所有者不进行进程发现、接管或路径移除，也不实现 Linux/macOS 停止。

这是实验性且尚未发布的私有 Rust Server application。它拥有内存 Workbench 导航权威与私有 projection
kernel、第一条 POSIX 本机 Server-Worker process spine 及 serialized post-admission OBCN carrier，
本机 Human Client/Client Session admission，以及内存终端协调器。产出的 executable 名为
`orchbee-server`；它参与根 Cargo verification gate，由 Linux 开发 cohort 的裸 `orchbee` 使用，但尚未被 Desktop、packaging、
installer 或 service manager 消费。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## Workbench projection store

`WorkbenchAuthority` 拥有一份内存 Site 导航文档及有界 request/result ledger。已准入本地 Client 可
通过 OBWP 读取关联 snapshot，以及创建或重命名 Project 和 Workspace。Workspace 创建同时提交
一个空 Terminal Context 及其引用。独立授权的 `workspace.primary-directory.accept` 校验本地
Worker 目录，再原子添加 Project Directory 并由该空 Workspace 选中。
[目录契约](docs/workspace-terminal-directory.zh-CN.md) 拥有校验、重放和失败语义。
`worktree-list` 读取先对已提交 snapshot 复核该 Project Directory 的 membership，再经既有
carrier 向该目录的属主 Worker 派发一个 fencing OBCN 发现 job；应答是 Worker 有界的原始
`git worktree list --porcelain -z` 字节，在 service 边界解析——membership 与 generation
检查在锁内进行，请求 I/O 绝不在锁内。
首个独立 Open 还会在启动前提交共享 Tab/Panel/Presentation
及逻辑终端 binding。本地 `tab.create` 命令追加一个简单独立或已配置 Workspace Tab，共同提交成员关系与保留的
终端意图，再派发一次启动。精确重放返回同一份待处理或最终结果。后续 Close 可以移除该
成员关系，但不改写历史 Create 结果。Client 可以使用当前 snapshot 中确切的 Site/Worker/Terminal binding，
通过 OBCS attach viewer。[权威模型](docs/workbench-authority.zh-CN.md) 定义授权、revision
冲突、语义重放、分配、停用和证据。读取在与写入相同的 gate 内克隆已提交 snapshot，响应 I/O
位于释放 gate 之后。

`tab.rename` 修改共享 Tab 标签。`tab.move` 在 standalone 或 Workspace 上下文内置换完整简单 Tab，不改变终端 binding，也不获取
Worker 注册表锁。二者均可在启动仍待完成时结算；精确 Move 重放保留原排序结果及修订。
原生 Move 交互仍是独立的 Client 集成边界。

`panel.split` 是第一项树形变换：它在派发第二个 immediate terminal launch 前，共同提交源 leaf、新 leaf、split
方向和两个 slot identity。Server 保留确切的树成员关系；现在的聚合 Close 路径会先销毁已捕获 Tab
中的每个 leaf，再发布一次移除。Linux process witness 创建两个真实 terminal、观察 Split projection、关闭 Tab，
并在 Worker 断开后验证 Host 没有剩余 terminal 或已经退出。原生 panel layout 与 Client-local focus
现已由 Linux 原生 Client 投影该规范分屏树，并在经过验证的叶子之间移动本地焦点；Client 仍一次只
附着一个 viewer，TUI 快捷键也不会提交 `panel.split`。

本地 standalone 与 Workspace 的 `tab.close`、叶子级 `panel.close`、状态 Read 和 Resume 使用保留的生命周期权威及单个进程生命周期
维护所有者。Panel close 恰好捕获一个 leaf，经同一生命周期销毁并清理它，再发布存活
Tab——被移除 leaf 的父 Split 折叠为兄弟子树；关闭 Tab 唯一 leaf 按聚合 Close 处理。已知
Host 销毁和精确 Worker/Server 清理先于规范移除；Client 退出不取消
已接受 Close。Linux 载体/进程检查覆盖三十二个活动 Client 连接、空闲控制槽释放，以及另一
shell 存活时的同连接复用。[Close 契约](docs/terminal-tab-close.zh-CN.md) 规定边界、另有证据
支持的原生 TUI 集成，以及仍待完成的生产恢复和发布义务。

`panel.move` 是无生命周期的树操作：它把同一 Tab 内一个 leaf 重定位到另一个 leaf 旁，折叠源
父 Split，并在单个修订中提交一个 fresh Split 与两个 fresh Slot；被移动 leaf 的 Panel、
Presentation 与 terminal 绑定保持精确不变。

无效、过大、语义不一致或错误 kind 的输入在协议边界失败。请求在解码前受 16 KiB 上限约束，
snapshot 上限仍为 1 MiB。OBWP 拥有请求/结果 envelope。Application library 保持未发布，仅供
自身 executable 使用。完整 TUI/Desktop/CLI Workbench 集成仍未完成。

## In-memory advancement 与 publication

权威的私有 `WorkbenchProjectionStore::advance_projection` 接受一份内部完整 snapshot，要求相同
Site 和严格递增 revision，然后按 canonical identity 顺序派生 navigation replacement 与变化的
context。在 1,024-change 和 1 MiB 编码上限内返回 `WorkbenchPublication::Delta`，否则返回完整
snapshot。没有可见 projection 变化时，empty delta 可以推进 revision。

Store 在改变状态之前校验并序列化 publication。应用 delta 必须精确重现 candidate。Parsing、
Site、revision、refinement 或 serialization 失败保持状态不变。只有权威可以推进该私有组件。
Client 不能替换 snapshot 或提交 delta。Carrier 提供写入后的按需读取；每条已提交 publication
保留在 64 条上限的 journal 中，并通过 `workbench.publication.read` 在相同读取授权下提供给
读取方，使持有基线 revision 的 Client 用有序 publication 快进，而不是重新拉取完整 snapshot。
journal 无法衔接调用方 revision 时，以一条完整 snapshot 边界 publication 应答。主动推送订阅
仍未完成。

## 托管 Linux Site 启动

私有 `--local-site <absolute-runtime-directory>` 入口消费
[共享运行时契约](../../crates/local-site-runtime/README.zh-CN.md)。
Server 先校验四产物 cohort 与运行时及不可变激活标记，再独占持有生命周期锁，
以 `setsid` 脱离 Client 的终端会话。它绑定 0600 Worker socket 并启动一个精确 cohort Worker。
Worker 注册后永久关闭 Worker listener，不准入替代代次；之后才以已完成权限设置的
原子发布开放 Client listener。启动使用 Server 自有 20 秒预算；Client 仍保有自己的总截止时间。
这里的就绪表示 Server + Worker 注册，不表示已有 Host 或终端。
托管角色的标准流为空；不会发出显式端点模式的 stdout readiness marker。

Client 退出不关闭 Site。Worker 丢失会使能力不可用，不启动替代 Worker；
Server 丢失保留启动标记和 Host 终端，需要显式恢复。本入口不实现恢复、自动清理、
服务管理器安装或重启。普通显式端点测试入口保持下述原有行为。

## 本机 Worker service

在 macOS 与 Linux 上，私有开发调用
`orchbee-server --local-endpoint <absolute-worker-path>` 会绑定一个 Unix-domain Worker socket，
且不删除既有 path。增量形式在其后追加 `--client-endpoint <absolute-client-path>`，从而绑定独立
Client socket。选中的 socket 都被限制为 `0600` mode；仅当所有选中 listener 就绪后，Server
才向 stdout 写入 machine readiness marker `ready\n`。格式错误的显式端点 invocation 保持静默，并以
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
request，submission 进入单个队列锁下的两条有界 FIFO lane——interactive 与 cleanup 走 high，
viewer 观测走 low——每条容量为 8。普通 submission 采用 try-admit：lane 已满时立即收到
`Backpressure`，high-lane submission 在已有 cleanup waiter 排队时同样退让；只有两个
session-cleanup 生产者按 FIFO 等待并优先于
后续 try-admit。low lane 非空时 owner 至多连续派发 4 个 high-lane 请求，随后必须服务一个
low-lane 请求。`Unavailable` 与 `Backpressure` 均表示 addressed request 没有跨过
complete-write boundary。[载体调度模型](docs/worker-carrier-model.zh-CN.md) 拥有 lane 分类、
准入、派放上界、排空与失败结算。Owner
会先完整写出一条 OBCN Launch、Start 或查看 request，再读取恰好一条 result。它只接受匹配的 result
direction 与 kind、operation ID、完整 authorization echo，以及 Start 的 target identity。
Malformed、mismatched、timed-out 或 disconnected exchange 会退役该 Worker generation，并使
queued work 变为 unavailable。

Close 使用仅面向生命周期的 `submit_lifecycle` 边界，将成功入队与后续结算区分开。
校验、帧编码及完成结果存储准备均在既有 `try_send` 前进行；背压不返回回执。成功时返回
一个有唯一所有者的 `PendingLifecycleResult`，即使 carrier 随即退役也不丢弃它。
`admitted_observed_at()` 在成功提交后采样，不是精确入队时刻。继续执行所有者可用此较晚
观察进行保守查询间隔计算，即使重新获取所有者 gate 时已过尝试期限，也必须计入已准入步骤。
`wait(self)` 消费回执并返回精确类型化结果或原始 carrier 错误；完成通道丢失时仍通过
既有 complete-write 标记区分写入前 Unavailable 与写入后 OutcomeUnknown。丢弃回执
不会取消已提交操作。此边界不添加队列、线程、超时、重试、步骤计数器或调度策略，也不改变
既有阻塞 `request()` 路径。暂停 owner 的队列、时间戳边界、背压、路由拒绝、完成通道丢失
及真实 carrier 结果测试覆盖准入/结算分离；Close 所有者模型和集成运行时拥有期限及继续
执行行为。

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
identity 被有意排除：该 endpoint 准入的是统一的 peer Client model。Server 最多接受三十二个 active
Session，并最多保留四十八个 candidate/session handler thread。每条 handshake 有两秒 read/write
timeout。因此，只要还有 handler slot，一个 silent candidate 就不能阻塞另一条 connection；饱和
时，多余 candidate 会被关闭，且不会分配更多 thread。Handler slot 与 admitted-Session
registration 都由 lifetime-owned guard 持有，因此正常完成、fault 与 unwind 都会释放两类上限。

经过身份验证的 candidate 会收到 protocol/cohort 不兼容、畸形或错误消息、超时、active-Session
容量超限等类型化拒绝。Server 完整写入 `ClientAccepted` 后，移除握手超时，在同一条未缓冲
stream 上读取严格 OBCS 或 OBWP。私有服务向其单一本机有效用户主体授予默认 shell 的 Create、Start
及这些终端的查看和显式获取独占 input/resize controller 权限。它读取一个请求、调用 Server 服务、完整写回关联结果，然后才读取该 Session 的下一
个请求。结果在 Client 输出前提交；写入停滞只占用自身 Session handler，不持有协调器或 Worker 锁。
读取在首个字节前没有截止时间；此后整条记录限时两秒，包括大 `OBCQ` 粘贴记录。
每次完整 Client write（包括大 OBSR
记录）也限时两秒。超时只退役对应 Session。

Create 选择当前本机 Worker，并验证显式精确 Worker 约束。Start 通过 request ID 跨同一主体的
Session 解析成功的 deferred Create。Server 保留精确 Worker 代次和完整 Host 身份。Worker
丢失或更换不能授权重新指派、重新创建或重放。EOF、畸形、错误方向、超限、截断及读写失败
仅关闭对应 Session；按代次清理保留终端状态。

私有 [`orchbee-local-client-protocol`](../../crates/local-client-protocol/README.zh-CN.md) 拥有
Client framing 和连接适配器。独立的
[`orchbee-client-server-protocol`](../../crates/client-server-protocol/README.zh-CN.md) binding
拥有 OBCS 验证及帧 I/O。Server 拥有主体策略、放置、operation/lease/fence 签发、终端绑定状态、
有界提交和关联的语义结果。Linux 裸 `orchbee` 已消费此路径；持久化和生产 ingress 仍在该切片之外。

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

在三个 generation 的探索范围内，`AdmissionUsesPreviousCursor` 与
`GenerationCursorChangesOnlyOnAdmission` 独立检查每次 Worker admission 都复制并推进
process-local cursor。Generation exhaustion 与 implementation refinement 仍由确定性 Rust 证据负责。
具名 rejection action 保留抽象分类与 authority-preservation proof；精确 typed 或 silent wire
outcome 仍由 protocol 与 process test 负责，因此模型不保留累计 rejection history。

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
转换为 successful 或 failed OBCN result。该记录所探索的单 submitted-slot 占用自此被
[载体调度模型](docs/worker-carrier-model.zh-CN.md) 中的双有界 lane 取代；wire-phase、
ambiguity-boundary 与结算义务不变。

## 本机 Client Session 与 OBCS 建模记录

`LocalClientSessions` 拥有有界准入、handler 计数、代次清理和 Server 丢失。
`LocalClientObcsSessions` 使用注入的拒绝 handler，检查同一 stream 的 framing 及顺序结果发布。
该 handler 是显式测试实例，不是服务的终端授权策略。其无终端副作用和 not-authorized 不变量
仅适用于这个实例。Runtime `application_session` 现在通过 callback 委托语义处理；单元测试保留该实例，
独立于终端副作用检查关联、重复 carrier record、部分 I/O、失败，以及完整写入后才能读取的顺序。

Session 模型探索三个 Client、两个 active Session、三个 handler 和有界代次。OBCS 的
stream-fault 和 correlation 配置包含合并的握手/请求、部分帧、输出停滞、错误方向、重复及
不同请求。这两个模型都不证明终端授权、Worker 派发、取消、重放、持久化或实时 relay。
下文的协调器模型和进程测试拥有新增副作用边界。

`LocalClientSessions` 记录 transition 前的 cursor，并检查每次 admission 的精确分配与推进；
`ActiveGenerationsPrecedeCursor` 只陈述保留的 current-state 关系。`LocalClientObcsSessions` 仅把
该 current-state 关系保留为 scaffold shape，不声明第二份 allocator proof。两项声明都是 finite 且
process-local；经过检查的 `NonZeroU64` exhaustion 仍属于 runtime evidence。具名
admission-rejection action 在不保留未读取 result label 的情况下维持 authority branch coverage；精确
wire reason 仍由确定性 protocol 与 service evidence 负责。

Carrier model configuration 仅实例化 Launch/Start。查看／控制附件 scope/outcome 校验和内部消费结果的
cleanup 复用其有界 queue 与 owner lifetime，具体 refinement 证据由 socket test 提供。Viewer cleanup
没有外部 completion receiver；经过校验的结算就是其 publication，因此成功会保持共享 carrier
存活。Controller cleanup 保留内部 completion receiver，直到精确 grant 释放或其 Worker 退役。只读投影路径另行映射到 `LocalTerminalRelay`；两个模型均不证明呈现解码或 controller
运行时行为。

## 本机 Client 查看附件转发细化

已接受的本地终端里程碑现在把 Server Client Session 接入 Worker 现有的只读查看附件循环。
暂存的原生 LocalClient adapter 消费查看协议。进程见证证明两个独立准入的 Client 经 Server 和
Worker 接收真实 Host projection。下文的 controller 细化增加 input/resize；Linux 裸入口已消费此 relay。

每个已准入 Session 在既有有序 carrier 上最多拥有八个临时 viewer。Attach 引用同一本机
subject 下已接受的终端 Create；AttachBinding 提供精确 canonical binding selector。Server
解析保留的 Host identity 与当前 Worker/generation，分配新的 request、attachment 和 cleanup
identity，并固定 Worker handle。Client 不能分配权威 runtime identity；后续每个 viewer/control
action 都必须回显一个由 Server 签发且归属发起 Session 的 attachment ID。每次 poll 移动一条
完整不透明 Host record；Server 验证 envelope、所选 attachment 与精确 feedback scope，不解码
终端呈现。每个 viewer 独立拥有 projection flight。两个 flight 的相等计数不能认证内容：显式
selector 决定反馈归属。Poll 同时排空该 viewer 的 kind-53 `HistoryChanged` event lane：Worker
在 frame record 槽旁维护一个有界的 per-viewer event FIFO，并通过显式 next-preference 在两个
来源间交替，因此 queued event 在 frame flight 未结算时仍可交付，持续就绪的来源最多越过另一
来源一次。kind-53 record 只对 `Poll` 是合法结果；任何其他 action 收到它属于严格 route
error，绝不进入 `projection.receive`。Event 交付不会把 frame flight 标记为已交付，
`HistoryObserved` 走自己的 acknowledgement flight，而不结算 `FrameApplied`。Event FIFO 或
byte-budget 饱和只关闭该 viewer；frame 与 event byte 在每条 teardown 路径上恰好释放一次；
发往已退役 viewer 的合法 kind-53 event 像迟到 frame 一样被吞掉。
Attach 在 Client publication 前提交。显式 detach 只释放所选 viewer 及其 controller；Session
断连或 publication 失败清理其全部 viewer。这些操作不终止或重新指向 retained terminal。
Unknown exchange 不重放；重新附着获得全新 attachment identity，并要求 snapshot。

Coordinator 或 registry lock 均不跨越 Worker/Client I/O。Drop cleanup 阻塞准入现有的单项
Worker request queue，不等待 Client response，也不增加无界队列。退役 handler 保留原有的
有界 Client Session/connection slot，直到 cleanup 准入完成。Wire owner 在内部观察完成。
Carrier 停止会关闭 broker 及其全部临时附件。Worker response 预算为二十秒，以容纳最多十六个
待处理 Host 查看／控制 detach、两个 Attach exchange 和 result delivery；control write 仍保持两秒边界。
一个排队请求加一个在途请求的边界要求四十六秒 launch lease。Client record read 在首字节之前
允许 idle，之后使用总计两秒 deadline；Client write 同样有总计两秒 deadline，超时只退役该
Session。

`LocalTerminalRelay` 是此前每 Session 单 viewer 的抽象，覆盖 attachment scope、单帧在途、
精确反馈、队列隔离与 detach 生命周期。其协议、确定性 feedback、carrier cleanup/backpressure
及双 Client process 证据证明该有界只读 relay refinement，不能独立证明当前多 viewer Session
registry。`SessionViewerRouting` 和下文必需的 generated/service/process 证据拥有此扩展；
其（该扩展的）最终验证仍待完成。两个模型均不证明 TUI 渲染或发布性能预算。

Viewer/control 的精确 ServerBusy 响应保证终端、查看附件与 controller effect 已知未改变。
Carrier 队列拒绝发生在 dispatch 前；Attach 的 Worker 容量拒绝可以经过 OBCN exchange，
但不会插入查看附件或向 Host 派发操作。Input/Resize 的 Client sequence 可能已被消耗，
因此已准入的 Client 背压策略必须同时使用新请求 ID 和新 effect sequence，保留原逻辑操作截止时间。
其他不可用、拒绝或未知结果不授予再次尝试权限。

## 本机终端 controller 细化

已接受里程碑为 Session 查看附件增加显式获取操作，每个保留 Host 终端只有一个输入与调整大小
共用的 controller。Server 在 Worker I/O 前预留 grant；该 grant 待定、存活或清理期间，其他
Session 不能获取。Server 新签发的 controller 身份限定 input 与 resize 共用的严格递增 sequence。
Client 提供的身份必须等于发起 Session 内所选 viewer 的 grant。非 controller 与过时 sequence 的拒绝不派发请求；
有效提交的 sequence 即使交付失败也被消费。未知副作用不重放；authority 丢失不重新选择终端。

Worker 将 grant 映射为独立 Host 附件，启用 input 和 resize、禁用 read，并申请 resize lease。
既有查看附件保持只读并保留 frame baseline。Worker 同样保证每个 Host 终端只有一个 grant，
校验精确 viewer/grant scope 与递增 sequence。只有 Host 返回的确切 capability revision 可授权
原始输入。Input 包含 1 至 4,096 byte；接受表示进入 Host 有界输入 queue，不表示 shell 已执行。
Resize 确认精确的完整 geometry。不增加 Host wire contract 或 VT owner。

Host cleanup 先释放控制附件，再分离查看附件。显式 OBCN Detach 在 Worker 内部执行此顺序；随后 Server lease Drop 发送幂等 Release 结算 registry。Session Drop 则先提交 OBCN Release 再 Detach。Server 预留持续占用，直到固定代次的
cleanup 完成或该 Worker 不可用；cleanup 经既有有界 request queue 准入，不持锁跨越 I/O。
过时 cleanup 不能清除新 grant。Host exchange 失败或结果矛盾会结束 Worker connection；完整
交付后的故障由 Server 报告 unknown control outcome。其他 viewer 不会自动提升。
显式接管现在遵循[控制权接管模型](docs/terminal-controller-takeover.zh-CN.md)：Pending 预留
封锁竞争接管和过期效果；候选者负责新旧清理，直到交接得到确认。保留 Viewer 的独立
release 仍是后续工作。

最多八个 viewer 意味着最多八个控制附件。因此，cleanup 可在下个请求前执行十六次一秒 Host
exchange。Worker response 预算为二十秒，launch lease 为四十六秒，原生 Client submission 为
五十秒。Request write 和 Client record read/write 保留既有两秒上限。这些是本机实现限制，
不是发布性能预算。

`LocalTerminalClientAuthority` 拥有 acquisition、controller input/resize、detach、loss 和过时
cleanup；`LocalTerminalRelay` 拥有递增请求和未知副作用退役。具体 reservation、cleanup、protocol
及 process test 细化这些声明，包括竞争 Session、错误 grant/sequence、输入 queue 确认、可观察
shell 输出、已接受 resize、断连和重新获取。设计质疑使用当前上下文，不属于独立审查证据。
Public cutover、持久化恢复和完整 Workbench 验收仍是独立边界。

## 本机终端 send

已准入 Client 可对其本可关闭的、已暴露 Workbench 终端提交一次非交互 input send。selector
指明 Server 在本次准入时验证的精确 site、worker、terminal 绑定；它不是从早前 list 结果
取得的隐式化身锁。send 是新增的显式编排输入权，不是只读查询，也不是权限零变化；它不新增
远程暴露，也不绕过 cohort、peer 或 Host 能力检查。

send 准入依次解析 Workbench binding 暴露、唯一保留绑定、当前 Worker placement 与绑定记录的
Worker generation、Started 终端相位、Unfenced 生命周期以及无已受理 close。未启动、已关闭、
已 fence 或 generation 过期的目标在 dispatch 前拒绝，且不消耗任何 Server 签发 identity；
解析绝不重绑陈旧化身。准入时 Server 铸造新的 operation identity 与新的 input-only
attachment identity，并把精确的 worker id、generation、Host epoch 与 Host terminal id 钉入
恰好一个 OBCN 请求；Worker 绝不按逻辑标识重解析。若被钉住的 Worker 在 dispatch 前退出或
generation 变化，该请求以 unavailable 或 unknown 失败，绝不重发。

Worker 在其既有 Host broker 上执行一次有序的 attach、input、detach；不借用任何 viewer、
controller 或 resize grant。受理仅表示输入已进入 Host 的有界 input queue；它不说明子进程
读过或执行过。Host 的类型化拒绝仍执行一次 detach。传输失败、错误 ack 或不确定 detach
之后，Worker 关闭该 Worker-Host 连接——其拆除即回收临时附件——绝不在可能失同步的流上继续
新 exchange。每个结果保留两个独立事实：input 状态（not-submitted、rejected、accepted 或
unknown）与 cleanup 状态（not-needed、confirmed 或 unconfirmed）。受理后的 cleanup 失败
保留已接受字节数，绝不把输入报告为未发送。

每次 send 提交一个 OBCN 请求，其标识只关联一次请求与响应；不新增持久 dedupe 账本，手动
重试的 send 是新请求、可能重复送达输入。send 与 controller exchange 共用单一 Worker
carrier 串行，输入顺序等于到达顺序。既有二十秒 Worker response 预算、两秒 write 上限与
五十秒 Client submission 窗口全部保持；三次一秒 Host exchange 在其内容纳，无需扩大。

## 已接受的 Terminal Client authority 与 relay 建模记录

有界 model 切片让 Rust Server 成为 relay `Client -> Server -> Worker -> Terminal Host` path 上
product-level Client Session 与 terminal authority 的 owner，并检查反向实时输出 path
`Host -> Worker -> Server -> Client`。上文所述运行中的私有 Server/Worker path 与 process trace 为
viewer/controller relay 提供 implementation evidence。Model 仍是 finite semantic evidence，不证明
wire codec 或 public UI。Viewer attachment 把一个 Client Session generation 与 retained terminal
关联，而 controller 是该 terminal 唯一、同时覆盖 input 与 resize 的 grant。

| 字段   | 已接受的 refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 多个同级 TUI、Web、Desktop 或 mobile Client Session 必须能并发读取同一个 retained terminal，同时不能产生多个 input 或 geometry writer。Slow Client 不能阻塞其他 Client 或 Worker progress；Client 退出必须 detach transient authority，而不能结束 terminal。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 输入   | Server-owned Client Session identity 与 generation；Worker、Host 与 terminal availability；viewer attach、显式 controller acquire、input、resize、detach、disconnect、reconnect 与 cleanup；Host 产生的 opaque OBTF snapshot/delta bytes；以及已有 attachment-scoped `FrameApplied`、`NeedSnapshot` 与 `ProjectionBlocked` 语义。Relay model 增加抽象 request identity 与严格递增的 attachment-local request sequence；projection record 还分别保留有界 delivery sequence、projection generation 与 content revision，但不选择其 wire field 或 encoding。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| 数据   | Imported Server authority state，加上每个 modeled Session 的 attachment generation 与 projection phase、分别用于 Host-to-Worker、Worker-to-Server、Server-to-Client delivery 的三个有界 per-attachment downstream queue、opaque frame 与 `ProjectionBlocked` payload tag、每个 projection flight 的一个精确 source-record witness、彼此独立的 frame sequence/projection generation/content revision、精确 feedback scope、单个 control request flight、used 和 effect-unknown request identity，以及独立 Worker progress counter。一个 attachment-scoped projection flight 在 Client feedback 开始返回前至多占据一个 downstream queue。history-relay 场景新增同一条三跳路径上每 Session 的 kind-53 event queue、attachment-local event sequence、一个独立 `HistoryObserved` flight、显式双源 poll preference 与有界 overtake 计数，以及覆盖 worker 持有 record 槽与排队 event 的 retained-unit 账户。Model 中不存在 Server 或 Worker 所属的 VT、screen、row 或 scrollback 内容。                                                                                                                                     |
| 输出   | 每个 live viewer 都可以收到 attachment-scoped snapshot、delta 或 `ProjectionBlocked`。`FrameApplied` 必须精确回显 attachment、delivery sequence、projection generation 与 content revision；`NeedSnapshot` 必须精确回显 attachment 与 delivery sequence。同一 flight 中不匹配的 candidate 会被拒绝，且不会改变 Host-applied baseline。只有唯一 imported controller 的 input 与 resize 才能到达 Host effect；non-controller、out-of-order 与 reused-identity request 会被拒绝。Downstream queue 满时只能 pause 或 close 该 attachment，不能改变其他 attachment 的 queue，也不能禁用独立 Worker progress。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| 不变量 | `LocalTerminalClientAuthority` 仍是唯一 authority：viewer 是其 imported viewer set，effective input 与 resize authority 是从中派生的同一个 set，cardinality 至多为一。Acquire 不会替换 owner，loss、detach 或 cleanup 都不会自动提升 viewer。每个 downstream queue 都有界且 attachment-scoped；relay hop 保留精确 source record，包括 opaque `ProjectionBlocked` payload tag；feedback 与保留的 flight identity 精确匹配；blocked recovery 的 content revision 必须严格大于 blocked revision；applied sequence 不超过 emitted sequence；effect-unknown request 会被 retire，绝不能重新进入 send phase；detach、disconnect 与 stale cleanup 保留 retained terminal。History lane 中，每个 event queue 有界且 attachment-scoped；持续等待的来源至多被越过一次；retained unit 恰好等于 worker 持有的 record 槽加排队 event；detached Session 不保留 history state；在途 `HistoryObserved` 精确回显最后交付的 event；emission、delivery 与 acknowledgement sequence 绝不超出已发出计数。                                                                                                                                 |
| 决策   | Controller 同时拥有 input 与 resize。其他 viewer 只有在显式 release 使 grant 变为 unowned 后才能 acquire；不存在 automatic promotion、implicit claim 或 modeled forced takeover。Server 选择 product authority，Worker 仍是唯一 Host broker，Host 仍是 PTY、VT、projection 与 history owner。Outcome unknown 的 input 或 resize 会消耗其抽象 request identity，绝不自动 replay。`NeedSnapshot` 放弃该 attachment 的 queued delivery 并请求新 snapshot；`ProjectionBlocked` 只阻塞该 attachment，直到 Host 发出 later-revision snapshot，而不会把 projection ownership 移入 relay。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| 模型   | [`LocalTerminalRelay`](formal/tla/LocalTerminalRelay.tla) 显式 `EXTENDS LocalTerminalClientAuthority`，并从 imported `viewers` 与 `controller` 派生 read、input、resize attachment；它不创建平行 authority machine。其 control-outcome、projection-feedback、backpressure-isolation、reattach-feedback、attachment-lifecycle、failure-interleavings 与 history-relay configuration 会拆分无关 finite history，同时检查同一个 state machine。注册的 source digest 包含 imported authority module。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 转换   | Session open 加 attach 建立 `needs-snapshot`；精确 opaque record 不经 decode 穿过三个 queue；Client apply 创建 `FrameApplied` acknowledgement；`NeedSnapshot` 清空该 attachment 的 in-flight record；`ProjectionBlocked` 可以在首个 snapshot 前或 streaming 期间发生，进入 blocked phase，并且只能通过 later-revision snapshot 恢复。Queue saturation 只能 pause 并随后 close addressed attachment。Detach 清空 delivery，并在自己持有时清除 controller grant，同时保留 terminal；re-attach 推进 attachment generation、重置 attachment-local relay state，并要求新 snapshot。Disconnect/reconnect 与 stale cleanup 仍按 generation 划分。`HistoryChanged` emission 进入 Host 侧 event queue；深度未满时 worker FIFO 接纳它，饱和 emission 只关闭该 viewer；poll 按显式 preference 取 frame record 槽或 event FIFO，每个来源还要求自己的目的 queue 有余量——目的满载令该 transfer 停顿而非把 poll 改投另一来源——每次成功后翻转 preference；client 交付开启 `HistoryObserved` flight，Host 只在 sequence/attachment 精确回显时接受，否则按 stale 丢弃；每次 attachment teardown 都随 record byte 一并释放 event lane。 |
| 依赖   | Model 把 package-owned Terminal Protocol attachment、input、resize、OBTF、projection-demand 与 projection-feedback 语义映射到已有 Server-Worker spine 和 Worker-owned Host broker。该切片不增加 codec、wire message、runtime dependency、direct Client-Worker path 或 UI behavior。未来 implementation 必须把这些 action refinement 为 concrete carrier 与 Host command，且不得让 Server 或 Worker 获得 VT/history authority。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 所有权 | Server 拥有 Client Session generation、viewer membership、combined controller selection、relay queue admission、rejection 与 stale cleanup。Worker 拥有 Host connection brokering，但不拥有 terminal presentation state。Host 拥有 retained PTY lifetime、Ghostty VT、immutable attachment-scoped OBTF production、projection/history state 与 low-level capability enforcement。现有 Host resize-owner promotion mechanism 不会成为 Server product policy。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 验证   | Authority configuration 检查三个 Client slot、两个 Session generation、有界 input/geometry revision、全部 semantic action、safety 与 cleanup progress。Relay configuration 检查两个 Client 与 attachment generation、两个 request identity、有界 delivery sequence/projection generation/content revision、两个不同 opaque payload tag、depth-one queue、每个 relay hop 的 exact source-record equality、严格更新的 blocked-projection recovery、same-or-older recovery rejection、错误 sequence/generation/revision feedback rejection、stale-feedback rejection，以及 control/output 与 Client、Server、Worker、Host loss 的交错。独立 Worker progress 保持 queue-independent，并在 weak fairness 下完成，除非 Worker 已丢失。每个 relay action 都要求非零 coverage。这些是穷尽 finite-model bound，不是 release budget；上文记录查看与控制运行时证据。Authority configuration 和七个 relay configuration 都要求完成 TLC 检查并达到 action coverage；超时不能提供进度证据。                                                                                                                                        |
| 限制   | Model 覆盖一个 retained terminal、每个 modeled Client Session 一个 attachment、一个抽象 control flight 与按 record 计数的 queue。它不定义或证明 wire schema/encoding、具体 request field、production queue count/byte budget、slow-Client threshold 或 pause-versus-close policy、production scheduler/fairness guarantee、timeout value、runtime/PTY retention、restart/durable recovery、direct attachment、forced takeover、collaborative input、controller fairness 或 public UX。history-relay 配置不把 event lane 与普通 detach、disconnect、re-attach 或 Host/Worker/Server loss 交错组合；teardown 组合依赖共享 close/reset 生产路径、core 检查与 routing model 的有界 cleanup 证据，而非本场景。                                                                                                                                                                                                                                                                                                                                                                                                            |

`LocalTerminalRelay` 为 frame 使用四个独立 identity field：attachment incarnation、delivery
sequence、projection generation 与 content revision。精确 `FrameApplied` 会匹配全部四项；精确
`NeedSnapshot` 会匹配 attachment 与 sequence，并在此 quotient 中使用值为零的 generation/revision。
`RejectWrongFrameAppliedSequence`、`RejectWrongFrameAppliedGeneration`、
`RejectWrongFrameAppliedRevision` 与 `RejectWrongNeedSnapshotSequence` 显式表示 mismatch class。
`FeedbackHasExactAttachmentScope`、`FeedbackHasExactFlightIdentity` 与
`MismatchedFeedbackIsEffectFree` 要求这些 mismatch 保留 Host-applied projection state。

`RelaySessionRefs` 仅对状态的表示做等价约简。`attachment-lifecycle` 是唯一启用 Session reconnect
的场景，并保留配置中的两个 Session generation，因此 reconnect 与 stale cleanup 仍然可达。
其余五个场景只会打开初始 Session generation；所有非 sentinel Session reference 因而都保持在
generation one，而十七个 relay-owned map 的 generation-two entry 原本也会始终保持 `RelayInit`
值。模型仅从 relay-owned map 与 quantifier 中省略这些常量 entry；两个 Client 以及全部配置的
attachment、delivery-sequence、projection-generation、content-revision、request、payload、queue
与 Worker-progress 边界均保持不变。`reattach-feedback` 仍会到达 attachment generation two；
这个独立的 attachment 轴没有被约简。已配置的不变量 `CurrentRelaySessionsAreRepresented`
检查每个打开的 Session 都仍有对应表示。

`ProjectionBlocked` 保留独立 revision 与 uninterpreted opaque payload tag；reason semantic 不在 model
范围内。`RelayedRecordsPreserveSourceRecord` 检查三个 queue 全部精确保留 record，
`SourceRecordIsUnique` 把每个 flight 绑定到一个 source record。
`RejectBlockedRecoveryWithoutNewRevision` 会拒绝 content revision 不大于 blocked revision 的 recovery
snapshot。`BlockedRecoveryRejectionIsEffectFree` 与 `BlockedRecoveryRequiresNewerRevision` 独立检查该
boundary。独立 Worker progress counter 仍是 relay-level noninterference abstraction；production
scheduler fairness 不在其 scope 内。

已检查的
[`LocalTerminalClientAuthority`](formal/tla/LocalTerminalClientAuthority.tla) model 仍是 authority 与
cleanup proof，其中显式 detach action 会保留 open Session 与 terminal。
[`LocalTerminalRelay`](formal/tla/LocalTerminalRelay.tla) 把该 state 与 transport-only queue 和
feedback 组合。Server 或 Worker loss 可以让 authority 失效，却不能转移 authority 或改变 retained
terminal；只有 Host loss 可以把 terminal 改为 `lost`。

`ViewerTransitionsAreExact` 独立对照 predecessor 检查每个 viewer transition：attach 只添加被寻址的
current Session；detach、disconnect 与 cleanup 只移除被寻址的 Session；断开未 attach Session 时会
保留该 set。因此，每个无关 viewer 都会得到保留，且无需把 predecessor set 加入 model state。

## 已接受的 Session Viewer 路由建模记录

本机已接受的实现切片将一个已准入 ClientSession 及其既有有序载体扩展为多个显式寻址的
viewer。实现使用上文所述有界 registry；下方可执行正确性论证定义其必需检查与证明边界。
本基础切片现在已与原生 Panel 分屏和聚合多终端 Tab Close 组合；Client-local focus/layout、远程路由和持久恢复仍属独立工作。
[OBCS 契约](../../schemas/client-server/README.zh-CN.md) 拥有 wire 表示。

| 字段         | 已接受契约                                                                                                                                                                                                                                                             |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题         | 显示/控制消费者需要在一个 Session 上独立路由 viewer，同时不转移权限或为每个 viewer 创建连接。                                                                                                                                                                          |
| 输入         | 严格 OBCS action、发起 Session、Server 签发的 `attachmentId`、精确保留 target、Worker generation、lifecycle proof 和受控完成。缺失/格式错误/禁止位置的 selector 字段属于 codec 错误；有效但不归属的 ID 获得语义结果。                                                  |
| 数据与 owner | `LocalTerminalSession` 独占其有界 viewer 集合；每项拥有固定 target/Worker handle、projection flight、controller lease 和精确 cleanup。Site 保留终端身份/lifecycle；controller registry 保留 target 独占 grant 所有权。                                                 |
| 输出         | 关联的既有 viewer/control outcome 与精确 Worker request；不隐式 Create、Start、Dispose、rebind 或修改 Workbench。Unknown exchange 从不授权 replay、fallback 或 retarget。                                                                                              |
| 准入         | 只回收具有精确已知 Disposed 证明的条目；在移除/Drop cleanup 前释放 proof 锁。已有八个已准入/存活条目时，在 target 解析或重复检查前返回 `server-busy`。否则解析授权的完整 runtime target；同一 Session 的重复 live target 返回 `already-attached`。不同 target 可并存。 |
| 路由         | 所有非 Attach viewer action 和所有 Control action 必须带 `attachmentId`。Attach/AttachBinding 禁止该字段。只在发起 Session 内解析，再验证该 viewer 的 controller 与 sequence 或 frame flight；从不搜索其他条目。                                                       |
| 缺失         | 不归属的 Poll/feedback 返回 `terminal-unavailable`；Control 返回 `viewer-unavailable`；Detach 返回无副作用的 `detached`。这只证明 Session 内不存在所有权，不证明终端 disposal 或其他 Session 的 cleanup。                                                              |
| 结算         | 在最终 install gate 前准备身份/storage；围绕 IPC 固定并重验当前 Worker 和未加 fence 的 target。确定拒绝的 Attach 不拥有 cleanup；候选 ownership 不确定时保留精确 cleanup。所有 gate 均不跨 IPC 或 Drop。                                                               |
| 生命周期     | Detach 只释放自身 controller，然后退役自身 viewer。Session 丢失通过既有有界 cleanup 清理全部所属条目。本地移除本身不证明下游 cleanup；pending cleanup 仍排斥竞争 grant。                                                                                               |

显式 OBCN Detach 是 Worker 复合操作：先释放 Host controller 再 Detach Host viewer；随后 Server lease Drop 通过幂等 Release 结算 registry ownership。Session Drop 则先提交 Release 再 Detach。OBCN 消息顺序本身不等于 Host 副作用顺序。

每 Session 准入上限与 Worker 既有的共享八 viewer 上限是独立计数。本地有槽位仍可能在派发后
收到 Worker capacity 拒绝，且不获得 ownership：Worker 容量拒绝经过一次 OBCN Attach，但不派发 Host Attach；Session 满时在 OBCN 派发前拒绝，包括重复 Attach。
候选预留计入 Session 准入容量，不得悄悄接纳第九个 live viewer。保持当前 queue、frame limit
和 deadline。Native 在本次迁移中仍是单 viewer 消费者，必须在 setup control、poll、feedback
或精确旧 viewer cleanup 前保留返回的 ID。

每个 controller 的 input/resize sequence 严格递增、允许跳号，与其他 grant 独立。新 grant
不能复活旧 ID。Feedback 归属显式 selector：选择 B 且计数匹配 B 当前 flight 时属于合法的 B
反馈，即使 A 的计数相同。计数不构成内容认证。入站 binary record 仍须验证精确 attachment 和
Host identity。Target-local 结构化请求保持无关 viewer；隔离前提排除 Session/carrier 退役、
共享 Worker/generation 丢失，以及同一 exact target 跨 Session 的 disposal。不新增隐式
controller 提升或 takeover。

尚未发布的 OBCS v1 source contract 修改 action 必填字段，但不改变 frame version。Schema、
corpus、Rust binding 和所有 Client producer 与精确 development Client/Server/Worker/Host
cohort 一起推进和回退。旧省略 selector 的形状被严格解码拒绝；不猜测兼容模式，不承诺混合
cohort。这是 action 形状的破坏性变化，不是此前 Open/AttachBinding 的 additive extension。

必需的可执行检查及其边界：

- `SessionViewerRouting` 建模 Session ownership、独立 Session/Worker capacity、候选及已安装
  cleanup、精确 disposal、每 viewer flight、每 grant sequence 和共享故障。同一模型还携带
  per-viewer kind-53 lane：`PollHistory` 在 frame flight 未结算时仍交付 queued event，
  `AcceptHistoryObserved` 只结算该 history flight，`RejectStaleHistoryObserved` 丢弃过期
  echo，`RejectHistoryWrongRoute` 退役在非 Poll action 上出现 kind-53 record 的 viewer。
  既有 Authority
  是单 retained terminal 抽象；Relay 每 Session 只有一个 projection/flight，sequence 只覆盖
  连续值子域；Selection 覆盖 target 解析/Worker pinning。这些模型不能独立证明多 viewer 或
  多条目 disposal cleanup。
  Routing 使用三个 viewer identity 覆盖 capacity/refusal；control 和 cleanup 使用两个。
  这些是穷举探索边界，不是八个 viewer 的 runtime registry 上限。Cleanup 按前两次准入的
  Session/target 分配与首次 controller 预留所属的 viewer 拆成全部 32 个组合。单调 viewer/grant
  cursor 使每条原始轨迹都有相容组合；操作次数不足的轨迹会出现在多个组合中。这两种操作均无
  公平性要求，每组均保留原有 cleanup/flight
  公平性、性质及 frame/sequence 边界，因此各组的并集覆盖原始行为。Routing/control 不分组；
  inventory 回归要求 cleanup 笛卡尔积完整，所有变体完成后才能接受 action coverage。
  每个配置的截止时间为十分钟。能够由当前 viewer/grant state 推导的 settled flight 与 cleanup evidence 会被规范化，
  而不是作为仅用于历史记账的 set 保留；这不会削弱 exact cleanup 或 liveness 检查。
- `viewer_session/tests/stateful.rs` 使用真实 `LocalTerminalSession` 和 service code、受控
  fake Worker，对照独立 `(session, viewer, target)` oracle。Merge corpus 使用 seed 1 至 64，
  每个最多 128 action、显式调度及带版本和 parser 的 replay。Shrinking 删除 action/chunk 并
  简化语义值，同时保留同一 failure fingerprint 和有效依赖；基础设施错误不能替代原发现。
  最小失败保留为 replay regression。检查 serialize/parse 稳定性与确定性 replay；oracle
  不得复用 production transition helper，也不能仅执行复制模型。
- Generated coverage 必须至少一次到达每个声明边界：重复 alias；满 Session 重复请求；独立
  local/Worker capacity 拒绝；错误 scope/grant；独立相等 sequence 和跳号；旧 replacement
  grant；相等 frame counter 下合法的 selected feedback；不匹配 flight；Detach/重复 Detach；
  有 live sibling 的精确 disposal；多条目 Session drop；Worker generation 变化；被拒候选
  cleanup。至少八个不同 seed 必须到达两个 live viewer 后再执行两个交错 target-local action。
  缺少覆盖即失败；固定示例单独报告，不能代替 generated interleaving。
- 确定性 service/carrier 测试检查精确 Worker request identity 及无额外副作用，包括 Native
  setup 和旧 viewer cleanup。真实 process witness 在一个已准入 stream 上持有两个终端、
  viewer 和 grant，交错 input/frame/feedback 和不同 geometry，Detach 一项后另一项继续可用，
  并证明 Session cleanup 保留两终端。第二个 Session 检查借用 ID 和 controller 竞争。两个
  独立的单 viewer 连接不能证明主要声明。Cleanup progress 依赖完成或 dependency retirement。

普通 Server unit suite 包含该 stateful test 及其 coverage/replay 检查。Protocol、model、
service/carrier 和真实 process evidence 都是 integration gate，不能互相替代。完成前仍须通过
最终验证及双语契约收敛。

## 已接受的 Session Viewer 历史读取建模记录

本已接受切片为一个已存在的 admitted viewer 增加一次性保留历史读取。它沿用同一条
Session/Server/Worker/Host 路由主干，只有一次关联 exchange，不引入任何长生命周期状态。
[OBCS 契约](../../schemas/client-server/README.md) 拥有 wire 表示；capability-node README 的
OBCN viewer 节拥有 tag-6 编码。

| 字段           | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                            |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | 一次性消费者（本机 CLI）需要有界的保留 scrollback 加可见行，不能长期占用 viewer、消费实时流或发明第二条读取路径。                                                                                                                                                                                                                                                                                     |
| Inputs         | `read-rows` action：归提交 Session 所有的 `attachmentId`、绝对 `startRow`、`rowCount` 1–4096、精确钉定的 `revision`/`generation`、正数 `requestSequence`、`maximumBytes` 112–8,388,608（界定 OBRW payload）。Client 根据一帧已验证帧计算 `startRow`/`rowCount`（`K = min(N, S, 262144 / columns)`，`start = S - K`）；Server/Worker 将该 action 视为不可信输入并逐字段重新校验。                      |
| Data and owner | `LocalTerminalSession` 仍拥有该 viewer 条目及其钉定的 Worker handle。Server 为每次 dispatch 签发一个全新的 Worker `OperationId`；该身份单独充当内层 Host correlation，绝不等同于外层 OBCS `requestId`。exchange 之外不保留任何 per-request 历史状态。                                                                                                                                                 |
| Outputs        | 恰好其一：经 OBNR/OBSR 返回的完整关联 OBTP response record、携带九种封闭 reason 之一的 `RowsRejected`，或既有明确的 `Rejected`/`Unavailable` outcome。超出字节预算是 typed failure，绝不是部分成功。`Empty` 绝不回答 `read-rows`。                                                                                                                                                                    |
| Routing        | Server 在发起 Session 内解析 selector（不归其所有 → `terminal-unavailable`），重新校验钉定的 Worker/generation/current-worker，然后 dispatch OBCN tag 6。响应 record 的 Host correlation 必须等于本次 dispatch 的 Worker `OperationId`；role、kind、encoding、Host epoch 与 attachment 必须匹配请求 scope。该 record 绕过只懂 frame 的 `projection.receive`。穿插的 Host event 仍路由到各自归属附件。 |
| Settlement     | Host `TypedFailure` 映射为封闭的 `RowsRejected` reason 集；合法的 `RowsRejected` 不会使健康 viewer 退役。非 `ReadRows` operation 上出现 `RowsRejected`，或在不合法 phase 收到 response `Record`，均为协议违例。buffer 读取不发送任何 acknowledgement（`RowsApplied`），任何失败后都没有隐式 retry、resend 或 retargeting。                                                                            |
| Lifetime       | 该 exchange 受既有一秒 Worker/Host 预算约束，处于调用方 deadline 之内。Viewer slot 释放与 detach/drop 仍归既有 Session cleanup；此处不延长 viewer 生命周期。                                                                                                                                                                                                                                          |

该 action 仅为增量的同 cohort 变化：旧 Worker 拒绝 tag 6；同 cohort 违例（内层 correlation
错误、phase 不合法的 response record、其它 action 上出现 `RowsRejected`）一律 fail closed，
而不是降级为猜测性解释。Worker 还会拒绝实际 OBRW payload 长度超过本次请求有效预算
（`min(requested, negotiated row-window, record carrier)`）的成功响应；64 字节 OBTP header 计入载体边界，
而 `maximumBytes` 只界定 OBRW payload。

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

`RejectConcurrent` 使用另一个 peer Session 的 one-outstanding-request slot 作为有界 correlation
surrogate，排入显式 `server-busy` result，并保留原 reservation。
`PublicationCopiesExactPredecessor` 要求 publication 复制并清除该精确 Session 的 queued result，同时
保留所有其他 Session。精确 OBCS request-ID echo 仍由确定性 runtime 与 codec evidence 负责。

`TerminalLaunchAuthorization` 使用 domain 包含二的独立 counter，跟踪每个 operation 的 modeled
handoff-effect occurrence。`HostEffectAtMostOncePerOperation`、
`HostEffectCountMatchesObservations`、`HandoffRecordsExactlyOneEffectOccurrence` 与
`EffectCountAdvanceRequiresHandoff` 检查其 bound、与 identity set 的对应关系、精确 increment，并要求只有
`HandoffToHost` 可以推进它。这证明
每个 operation 最多发生一次 modeled handoff-effect occurrence；具体 transport-write multiplicity 的
证据仍由 Rust duplicate-dispatch test 提供。

## 所有权

该 application 拥有本地 Workbench 权威、独立终端成员关系 workflow、私有 projection mechanics、
Worker 和 Client registry、OBCN carrier、终端意图 coordinator，以及 Session 查看/控制 relay。
现有 Desktop/Workbench 仍是已交付 Workbench writer；
[`apps/server`](../server/README.zh-CN.md) 保留 Browser ingress 权威。持久化、重启对账、
生产 service lifecycle、打包、迁移及公开切换仍未完成。

可导入的 authority boundary 是 `src/lib.rs`；`src/main.rs` 组合私有 binary。穷尽的 lateral 方向为
`local_site_service -> worker_registry`、`worker_obcn_carrier`、`local_client_service::start`
和 `local_terminal_service`；`local_client_service -> client_session_registry`、
`local_terminal_service`、`workbench_service` 及其私有 `application_session`；
`workbench_service -> local_terminal_service`；
`local_terminal_service -> site_coordinator`、`WorkbenchAuthority`、`terminal_coordinator` 值类型、`worker_registry`
和 `worker_obcn_carrier`；`site_coordinator -> WorkbenchAuthority` 和 `terminal_coordinator`；
`workbench_authority -> workbench_projection`；以及 `worker_registry -> worker_obcn_carrier`。
Service 在锁外构造 Workbench 权威。SiteCoordinator 拥有独立终端 workflow、确切 binding selection 和共享提交边界，
组合两个 core，但不接管它们各自的事实。

终端 coordinator 的私有 `outcomes` 子模块拥有 Worker observation 投影。SiteCoordinator 父模块
拥有首个终端槽位及其对应关系辅助函数。其私有 `standalone` 子模块组合首次成员关系与终端意图，
`tab_create` 则通过绑定来源、消耗所有权的令牌组合命令的共同准备与结算；`terminal_selection` 组合 canonical exposure、
保留目标解析和 attachment identity 准备。WorkbenchAuthority 的
`terminal_binding_selection` 子模块拥有只读 canonical exposure query；`terminal_tab_create`
拥有命令接纳、准备后的成员关系、预留账本字节及结算。
`workspace_directory` 子模块分别拥有 WorkbenchAuthority 的目录接纳与规范提交、Site grant
组合，以及 service 在锁外进行 Worker 观察后重新校验的流程。`terminal_tab_move` 拥有 Move 接纳与
准备后的置换；Site 在目标/无操作判断前校验组合。二者共享命令预检；Move 复用权威的
准备后已结算命令发布路径。
LocalTerminalService 父模块拥有共享熵源
获取；其 `tab_create` 子模块按 registry 再 Site 的顺序持锁固定 Worker placement，在两锁之外
派发，并先结算两个 owner，再通知首次 Open 的等待者。共享辅助函数定义在父模块中，子模块不导入兄弟模块。
Service 的 `viewer_session` 拥有附件 lifetime；
`controller_session` 拥有终端预留、Session grant、副作用 sequence 和精确释放；
`projection` 拥有路由与反馈。Client 的 `record_stream` 拥有准入后的 I/O deadline。
反向及其他 lateral import 仍禁止；测试不扩大 production visibility。

Carrier 拥有 I/O、精确 echo 验证和 generation retirement，不拥有 OBCS 或产品策略。
确定性 core 不导入 transport、clock 或 process adapter。Terminal service 操作按 registry 再 Site
的顺序获取锁；grant 操作则按 registry 再 controller map 获取锁。Workbench 访问先校验外部 OBWP policy，
读取及纯导航随后只获取 Site gate。不透明的 `WorkbenchReadOrNavigationRequest` 排除需要 Site 组合的
`tab.create`、`tab.move`、`tab.close`、`panel.split`、`panel.close`、`panel.move` 和
`workspace.primary-directory.accept`。Move 要求 navigation grant，且只获取 Site gate；
`tab.create` 还要求 terminal-create grant，并使用上述组合路径。Binding viewer selection 先持有 registry 和 Site 锁，再采样 handle 的
active state；其内部 canonical query 使用 OBCS subject admission，并保持 Workbench 优先的失败顺序。I/O 前释放全部 gate，不增加自动重试。`application_session` runner 将确切
OBCS/OBWP prefix 分派给不同语义 handler，只导入其契约及标准库 I/O。提交后协调失败会关闭混合
载体，不会伪造命令结果。尚未发布的 application library 暴露权威、Site grant 和内部
membership/exposure/preparation 的 typed result；整份 projection 写入保持私有。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-worker --locked
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --workspace apps/orchbee-server --jobs 1
pnpm verify
```

Server 模型命令每次只运行一个 JVM，避免大型状态图之间争用资源。
每个 configuration 保留其声明的 TLC worker 数量、边界、属性和 action coverage。
使用此资源配置时避免并行执行其他大型构建或模型检查；执行 deadline 仍然生效。

执行清单为 `LocalTerminalRelay` 的六个 configuration 分别设定 600 秒超时，并使用两个 TLC worker。
这是完整状态枚举与最终活性检查的执行预算；完成验证仍须通过全部已配置 property，并满足要求的 action coverage。

## 本机独立终端打开

尚未发布的 `terminal.open` 为本机单主体原生路径打开一个 Server 拥有的独立终端引用。
首个通过全部预检的请求原子提交共享 Tab/Panel/Presentation 成员关系及终端意图，再使用 Worker
默认 cwd 和请求 geometry 提交一次立即启动。后续 Open 等待或返回同一份保留结果及 Create
reference；已结算槽位在严格叶级树成员关系上重放，已提交的 `panel.split` 叶或合法重排绝不改变
返回的引用或退役共享 Workbench。此操作由 OBCS 本地终端准入授权；内部成员关系提交独立于 OBWP 导航 command ledger
及其 read/navigation grant。

共享 binding 来自 Server authorization 中的逻辑 TerminalId 和 WorkerId，不是 Host、Client
或 request 身份。它记录产品意图，在启动失败或 unknown 后仍保留，不表示 runtime 可用。
[Workbench 模型](docs/workbench-authority.zh-CN.md#独立终端成员关系模型) 定义规范聚合、预检及
共享 gate。Open 不调整既有终端大小；只有已取得权限的 controller 才能独立提交已校验 Resize。
Linux 裸启动已消费 Open 及随后基于 snapshot 的 binding attachment；后续 Workbench 操作
及完整 Client Workbench 渲染仍未完成。

| 字段     | 契约                                                                                                                                                                                                                                        |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 输入     | 已准入 OBCS Open、真实 geometry、当前 Worker placement、lease、新生成的非零随机 Create reference 及单调等待时间。Request ID 只关联响应。                                                                                                    |
| Owner    | SiteCoordinator 拥有独立槽位和跨 owner workflow；WorkbenchAuthority 拥有 projection/cursor，TerminalCoordinator 拥有终端 binding/allocation。Service 拥有 IPC、熵源和 deadline；Client 拥有自己的 request/viewer。                          |
| 模型     | Empty -> Creating -> Settled。纯终端 planning 和 Workbench 验证先于同一个 Site gate 下的提交。Dispatch 与响应发布位于 gate 外。已结算重放读取严格叶级树成员关系，并要求槽位引用的保留 binding 在全体成员中恰好出现一次。                    |
| 预检     | 缺少 Worker/Workbench、熵源失败、reference 冲突、capacity、identity、revision 或 lease 失败均保留 Empty，不留下终端或成员关系。内部 Workbench 预检/publication 失败会退役 Workbench，且不 dispatch。                                        |
| 完成     | Started 映射为 Opened；unavailable/failed 保留 Create 原因；引用冲突、熵源或 Workbench 预检拒绝映射为 server-busy。固定输入的意外拒绝映射为 terminal-create-failed。Unknown 保持 unknown。提交后失败保留成员关系，不允许再次创建。          |
| 并发     | Wait 释放 Site mutex。唯一绝对 46 秒 condition deadline 到期时，仅向该 waiter 返回 server-busy，不取消操作。Worker 交换最多 44 秒；普通 Client 提交为 50 秒，首帧前还受 startup deadline 约束。                                             |
| 生命周期 | Client 丢失或发布失败保留成员关系。Opened 要求确切的当前 Worker generation；丢失/替换不会接管或重新启动。后续导航退役不能取消已提交的启动。                                                                                                 |
| 限制     | 单 mutex 下成功进程内执行的原子性，不承诺 crash/persistence 原子性。一个独立 reference；不含 close/reset、recovery、完整 Workbench 操作、远程授权或 activation policy。Wait deadline 不限制锁获取、进程挂起或响应写入。运行时证据限 Linux。 |

### 独立终端打开正确性

| Claim                                   | 精化与证据                                                                                                                                   |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| SO-01：最多一个准备好的 create          | Reserve 映射到 gate 内 Site 提交；Singleton/NoReset 及 core 预检/冲突/并发测试。                                                             |
| SO-02：调用方共享已提交 reference       | Complete/Observe、SameReference/OpenedWasCommitted、确切 placement 测试及真实双 Session/Host trace。                                         |
| SO-03：Client 丢失不重置共享状态        | Disconnect/Reopen、DisconnectPreservesSlot、未读结果及重新打开的进程 trace。                                                                 |
| SO-04：不确定性不产生另一个副作用       | Complete(unknown)、SettledSlotIsFinal，失败/unknown/同 Worker 新 generation 的 core 和进程测试。                                             |
| SO-05：等待不依赖 Worker 公平性即可结束 | Timeout/EveryWaitSettles 及绝对截止时间 condition 测试；只有本地 timeout 观察采用弱公平性。                                                  |
| SO-06：启动必须有已提交成员关系         | Reserve/Dispatch、MembershipCommitted/DispatchHasMembership，以及 Worker 完成前的逻辑 authorization 对比。                                   |
| SO-07：导航交错时读取仍保留 binding     | Navigate/ReadWorkbench、ReadIsCommitted/MembershipIsRetained、stateful 导航 trace 及 Worker 停滞进程证据。                                   |
| SO-08：预检不消耗任何 allocator         | PreflightPreservesState 覆盖抽象成员关系/revision；确切 identity、revision 和编码 capacity 测试覆盖具体分配。                                |
| SO-09：合法多叶树上已结算重放仍成立     | `DerivedComposition` 在有序叶成员序列上的锚点唯一、`MovePreservesBootstrap`、确定性 Site 拆分/重排/关闭测试及 Linux PTY 拆分-退出-重进见证。 |

[模型](formal/tla/LocalStandaloneTerminal.tla) 区分两个 Client、两个 Create reference 和两个逻辑
binding，包含两次导航提交及一次创建尝试。其成员序列抽象叶级成员关系，追加的成员同样代表已提交的
split 叶。它抽象熵源、编码、分配 cursor、重复完成、时间数值和
调度。确定性与 stateful 测试覆盖身份不复用及忽略重复完成；进程/PTY trace 覆盖 framing、
逻辑身份到 Host 的映射、并发、真实执行和保留。

## Windows 原生 Site

Windows 托管 Site 保留现有内存权威和生命周期锁。它准入当前令牌用户的命名管道对端，并禁用句柄继承来启动精确的同级 `orchbee-worker.exe`，使 CLI 输出捕获可独立于保留的 Site 结束。运行时路径、ACL、锁、标记及 cohort 校验仍由 `orchbee-local-site-runtime` 负责。Windows 观察入口无需 Unix 半关闭即可接收一个有界完整 JSON 值；畸形连接不会中断其他客户端。此入口适配器不移植 Unix provider hooks。可用命令面及剩余限制见 [Client 快速开始](../orchbee/README.zh-CN.md#windows-原生快速开始)。

## 目录工作区启动

原生未发布的 `workspace.launch` 命令在现有项目中创建工作区、接纳绝对目录并创建首个终端，全程使用一个请求标识。必须同时具备导航、目录关联和终端创建权限。显式 argv、环境变量及启动配置复用现有有界启动契约。客户端焦点不属于此操作。

Server 检查准入，释放锁执行 Worker 只读目录探测，再按注册表到 Site 的锁顺序复查请求、修订和精确 Worker 代次。它准备六个结构标识及一份候选投影，预留保留结果与终端准入，然后在一次修订中一起提交工作区、目录、上下文及首个 Tab/Panel/Presentation。准入失败不会发布半成品工作区。终端派发仅在提交后发生；启动待定或失败时保留所有已提交标识，供查询和显式关闭。精确重放绝不再次派发。响应丢失属于结果未知，不得自动重试。

此细化复用 Tab 创建的提交与结算所有者。验证义务包括权限分离、探测期间的陈旧修订及 Worker 代次、容量与标识边界、精确重放、提交后的启动失败，以及 cwd/环境变量的 Windows 进程见证。不宣称持久化重启恢复、Desktop 支持、焦点变更或 TLA 执行。

## Windows 保留 Worktree 续执行

`workbench.worktree.operation` 接纳 Start、纯 Read 与尝试序号约束的 Resume。Workbench 权威最多保留 64 个操作，全局一个 Pending，并拥有规范 Workspace 关联；惰性维护所有者在 Client 断线后继续执行。registry 到 Site 接纳及发布复查来源和精确 Worker 实例；OBCN IPC 在两道门之外运行。Create/Open 观察 Worker 检出事实后才复用或启动 Workspace。Remove 在 Close 前检查脏状态，保留新 Workspace／目录准入，暂缓规范发布，等待终端释放和清理后才执行 Git 移除。已知 Git 失败会验证原 checkout，并在保留的 presentation 槽位恢复全新的默认 shell；独立的有界恢复计数保留启动失败和未知结果。删除结果未知时保持布局受限，不启动 shell。已知成功后释放发布，最终 Close 重试不重复 Git。Worktree 失败后，显式普通 Close 可不执行 Git 而解除保留。显式 Site 停止在 Close 维护及 Worker carrier 冻结前 join 此所有者。界限及恢复见 [CLI／续执行契约](../orchbee/docs/windows-worktrees.zh-CN.md) 与[共享协议](../../schemas/workbench/v1/workbench-worktree.schema.json)。

## 只读 Worktree 来源查询

`workbench.worktree.query` 解析显式 Workspace 或字面目录，无需 Project。它在 registry-to-Site 门内捕获来源及 Worker 实例，在门外执行一个有界发现 job，再复查并加入当前 Workspace 标签／身份。多个关联明确保留，不任取首个匹配，不改变焦点、创建 Workspace 或消耗变更回执。共享 porcelain 解析器接受 Windows 绝对驱动器路径及既有路径形式。来源变化、Worker 丢失、元数据无效及预算到期返回类型化失败。参见[所属契约](../orchbee/docs/windows-worktrees.zh-CN.md#按来源发现-worktree)。

## 保留的 Workspace 组关闭

`workbench.workspace-group-close` 保留接纳时的有界成员集合，以源最后关闭的顺序组合现有 Workspace Close 回执。已核实的 Worktree 源关联支持主目录位于子目录，并保留精确 Directory/Worker 代次证明。普通主工作区 Close 拒绝多 Workspace 组；只有精确的保留组成员令牌才能绕过保护。组与 Worktree 的新修改在 Pending 期间互相保留执行权。共享维护生命周期适配器拥有各自的唤醒/线程/加入实例；Windows Site stop 在 Close 维护冻结前加入两个提交者。[归属模型](../orchbee/docs/windows-worktrees.zh-CN.md#workspace-组关闭) 定义上限、部分失败和恢复；[线上 schema](../../schemas/workbench/v1/workbench-workspace-group-close.schema.json) 定义公开字节。组关闭不调用 Git 删除。

## Windows Client 视图路由

传输接纳仍与界面类型无关。已接纳的 Windows Session 可额外以 Server 分配的 generation 身份发布一个易失交互视图。`client_view_registry` 拥有有界路由、新鲜期及保留的投递回执；`WorkbenchClientSession` 提供认证所有权和断开清理。规范 Workbench 权威与终端载体所有权保持分离。[Client 控制所属契约](../orchbee/docs/windows-client-control.zh-CN.md) 定义线上协议、绘制后确认、生命周期及证据；`ClientViewControl` 提供有界生命周期模型.
