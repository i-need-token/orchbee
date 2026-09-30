# Rust Orchbee Worker

这是第一条 POSIX 本机终端 vertical 使用的实验性、尚未发布私有 Rust Worker application。
产出的 executable 是 `orchbee-worker`。它参与根 Cargo gate，由 Linux 裸入口的本地 Server 激活；尚未打包或安装。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## Windows 子终端 CLI 路由

托管 Windows 启动从准入后的 Site 运行时目录覆盖 `ORCHBEE_RUNTIME_BASE` 和
`ORCHBEE_RUNTIME_SCOPE`，并保留现有终端与 Worker 身份。覆盖值优先于继承值和启动参数中的值。
延迟和立即启动使用相同路由；路径文本不可用或无效时拒绝准备，不静默将内部 CLI 命令路由到其他 Site。
不带托管运行时的显式端点启动保持现有行为。

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
运行串行 OBCN Launch/Start、lifecycle、查看、控制及 cwd-validation request loop，不创建
connection thread 或 operation queue；每个查看附件拥有一个有界交付槽。Server loss 关闭 Host
broker socket；Host loss 停止准入，只允许对已分类 result 做有界交付尝试，然后关闭 Server
attachment。任一 peer 丢失均静默以 status `1` 退出；retained terminal 继续由 Host 拥有。Invalid
invocation、connection、authentication、handshake rejection、framing、output 或 transport failure
也采用同样的静默 status-`1` 结果。合法的 operation-level rejection 返回 typed OBCN result，并让
loop 继续接收下一请求。

Worker 只消费已准入 same-user Server 的 authorization，并用 OBLS generation 构造授权 kernel。
只有 local admission 与新的 handoff check 通过后才发送 Create/Start。显式端点模式不启动 Host；
托管 Linux 模式使用下面的按需启动流程。不提供连接重试、重启、inventory 接管、PTY/VT 操作、
Agent 行为、服务安装或恢复。

## 托管 Windows 显式停止

托管 Windows Worker 保留其创建的准确 Host 子进程句柄直到完成。代次匹配的停止请求永久关闭普通操作准入；
非托管或代次不匹配的请求在产生效果前拒绝。停止先取消并等待发现任务结束，再在单次请求截止时间内（最长 15 秒）
使用现有 Host 清单、强制终止和释放契约。每次清单请求选择首个剩余终端；单次尝试对选中的活动终端只发送一次
强制终止，释放要求观察到可释放阶段并收到准确确认。预先准备的目标清理仅在确认后回收本地查看器和控制器所有权。
完整空清单允许断开代理连接，然后等待保留的 Host 句柄。管道消失不能单独证明停止。Server 必须另行确认 Worker
退出后才能发布重启权限。

未完成的尝试保留失败阶段、原因及本次已知释放确认计数。之后 Worker 仅接受显式停止请求。后续尝试可继续健康连接，
或等待已经断开且清单为空的 Host；协议或传输不确定会退役 Host 连接，绝不授权重放变更、重新连接或接管。
发现任务子进程的清理未知时不能报告成功。无 Host 或确认 Host 退出的结果在有界结果写入尝试后结束 Worker；
普通 Server 断开保留现有语义。此 Windows 生命周期不增加 Linux 或 macOS 停止路径。

## 托管 Linux 延迟启动 Host

私有 `--local-site <absolute-runtime-directory>` 入口校验共享运行时及四产物 cohort，
先完成 Server 注册，此时不创建 Host 目录也不启动 Host。
[共享建模记录](../../crates/local-site-runtime/README.zh-CN.md) 定义进程与失败边界。

首个 Launch 先由现有授权内核准入并解析启动事实。无效 lease/generation 的类型化拒绝
不产生 Host。绑定代次的 cwd validation 和 worktree-job 请求都可以在该 Launch 前完成
而不启动 Host——发现槽位跨交接保留，已准入的 job 在交接后仍由其属主持有；其他非 Launch
流量让连接退役。准入的 dispatch 和同一个内核跨 Host 启动保留，交接前再次校验授权。
Host 目录为运行时下的 `host`，固定 endpoint ID 为 `00000000000000000000000000000001`。
该目录下端点的绝对地址在创建目录或 Host 进程之前先按平台 socket 地址 ABI 检查；
无法容纳的地址以同一交接前失败结算，且不产生这两种副作用。
任何已有端点立即要求恢复；只为本次新启动的 Host 等待最终 0600 socket。
Host 启动、唯一连接和严格 OBTP 握手共用五秒预算；不续期或重试连接。
准备/启动失败在可交付时返回交接前失败结果并退役 Worker；已交接的不明确结果不重放。
后续操作使用原串行循环和 replay cache。

此 managed 路径上的每个 Launch 还会通过现有 Terminal Protocol `environmentOverlay`
传播本地 Agent 观察通道。对每个准入的 Launch，launch adapter 将跨属主约定的文件名
`agent-observation-v3.sock` 拼接到已验证的 `--local-site` 运行时目录上，仅当 `lstat`
证明该路径是存在且归本 Worker effective user 所有的 socket 时才予以采纳。被采纳的
socket 在现有 16 条 / 4 KiB overlay 上限内把 `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3`
设为其绝对路径；候选缺失、非 socket、属主不符或非 UTF-8 时则不注入变量，也绝不因此
使 Launch 失败。门禁在每次 Launch 重新求值，不缓存准入结果。显式端点路径不携带 Site
运行时目录，不携带观察通道 overlay——远端与非 managed 启动不会获得观察通道，也不为此引入
CNP 字段或 Server 私有代码。

回收线程只等待退出，不杀死 Host。Server/Worker 连接关闭后 Host 仍拥有保留终端。
不支持重启后接管或替代 Host。显式端点模式继续采用其既有双 peer readiness 语义。

## Worktree 发现 job

串行请求循环在首个 Launch 之前和 Host 准入之后都接纳 `worktree list` Server 流程的
OBCN worktree-job 请求（start/poll/cancel）。一个进程级槽位同时只运行一个发现 job——先在 Server 提供的
目录内执行 `git rev-parse --path-format=absolute --git-common-dir` 探测，再执行
`git worktree list --porcelain -z`——处于从准入起算的固定 5 秒 TTL 与一份跨两个
stage 共享的 64 KiB stdout 预算之下；越过合并预算的 stage 承担 `too-large` 归因。
每个请求按精确的 `{workerId, generation, jobId}` 三元组 fencing 并应答类型化结果帧；
`rejected-fencing`、`busy`、`dead` 都是诚实 outcome，绝不伪造完成。job 只上报有界
原始字节——porcelain 解析与畸形 listing 拒绝是 Server 的权威，不属于 Worker。
属主自治回收其子进程——死亡或被逐出的 job 只保留一份有界 receipt，直到下一次
start 将其替换；取消后终态仍可通过 `poll` 观察。

## 实时查看附件转发细化

已接受的本地终端里程碑现在将 `LocalTerminalRelay` 细化到 Worker 的单一 Host broker。
Server 仍是查看附件的唯一签发方。每个绑定代际的查看附件只有读取能力、独立的有界交付槽，
并且最多只有一帧等待 Client 精确反馈。Worker 校验信封和固定路由元数据，同时保留完整的
不透明 Host 记录；不解码终端行、VT 或历史。附着、轮询、反馈和分离与 Launch/Start 一起在
已接纳的 Server 连接上串行执行。轮询交付已就绪记录，不等待终端输出；下游交付不能占住
Worker 操作槽。

在 frame record 槽旁，每个查看附件还维护一个有界、深度为四的 FIFO，保存完整不透明
kind-53 `HistoryChanged` event record，并计入同一个 retained-byte 预算。Poll 在 frame/
projection-blocked record 槽与 event FIFO 两个来源之间按显式 next-preference 择一交付，
初始偏好 record 槽，每次成功交付后翻转；未结算的 frame flight 绝不阻塞 queued event，
持续就绪的来源至多越过另一来源一次。Event 交付绝不触碰 `flight.delivered`、
`last_sequence` 或 frame acknowledgement state；`HistoryObserved` 走既有 feedback lane
回传 Host，不结算 frame flight。FIFO 或聚合字节饱和只关闭该查看附件；close、remove、
cleanup、Host 丢失和 target cleanup 都恰好释放一次 frame 与 event byte。发往已退役查看
附件的合法 kind-53 event 像迟到 frame record 一样被消费，而不是退役 broker。

OBCN 查看附件载体和 Worker 附件转换现已服务于 Server 已准入 Client Session 转发，并有真实
Host 进程轨迹。Server 拥有 OBCS 查看消息与 Session 清理；Linux 裸入口现已消费该 relay。二十四个存活查看附件、每附件最多一条保留 Host 记录，以及 128 MiB 聚合保留记录预算属于
实现边界，不是发布性能预算；owning 模型见
[查看附件容量与保留记录预算参考](docs/viewer-capacity-and-record-budget.zh-CN.md)。
缺失或矛盾的反馈绝不确认 Host 帧。`NeedSnapshot` 保留原因和序号；
`ProjectionBlocked` 仍是附件事件。
交付槽已满时，只关闭该查看附件并释放排队字节；会把聚合预算推过上限的合法记录同样只退役
其寻址查看附件。它的 Host detach 在下一个普通 Server operation
之前执行，lifecycle step 跳过该清理。关闭中附件的迟到 event 被排空，不再保留额外帧。绝不静默替换 blocked event 或帧。
Broker 的 `host_connection` module 拥有 event 路由、查看附件槽、controller grant 和共享 deadline exchange；
`terminal_viewer_operation` 映射已准入的只读 command，并校验精确 Host response。
查看附件 state borrow 不跨越 Host I/O。Attach 依次执行两个有界 exchange（capabilities，随后
激活 projection demand）；cleanup 可在请求前执行最多三十二个有界查看／控制 detach exchange。查看附件 carrier 现为这些 exchange 和 OBNR record delivery 提供二十秒预算。Lifecycle/metadata notification 经校验后排空；
bell、clipboard 与 application-signal effect 不经此只读投影路径对外发布。分离和 broker 丢失释放临时附件并保留终端。
重新附着必须使用 Server 新签发的附件身份；此只读查看路径不授予输入或调整大小权限。

证据将现有权限／转发模型映射到确定性路由、精确反馈、过期作用域和容量测试，以及包含
两个查看附件、停滞查看附件、快照反馈、分离、重新附着和终端保留的真实 Worker/Host 轨迹。
线协议测试检查有界分帧和不透明字节保留。当前上下文内的挑战检查不构成独立审查证据。

## Controller grant 与副作用

`terminal_control_operation` 消费 Server 签发的 OBCN 获取、输入、调整尺寸、粘贴和内部释放
请求；粘贴以专用 `OBNQ` record 在同一流上到达。
`host_connection::controllers` 拥有有界纯 grant 状态：每个 Host terminal 一个 controller、精确
viewer 和 controller 身份、待定或已接受的 Host capability revision，以及已消耗的最高 sequence。
Grant state borrow 不跨 Host I/O。查看和控制 adapter 均使用 `host_connection` 所属的精确关联、
有 deadline 的 exchange。

获取要求存活的只读 viewer，并创建独立 Host 附件，禁用 read、启用 input 和 resize，并申请
resize lease。它保留该 viewer 的 projection baseline。只有精确 capability acknowledgement 才
提交 grant。Input 保留 1 至 4,096 个原始字节并使用已接受的 capability revision；Host 必须确认
完整 byte count。Resize 必须确认完整 geometry。Paste 以 kind-39 `PasteAutoInput` 保留 1 至
262,144 个不透明字节；Host 必须确认正的 `accepted_bytes`，且不超过
`min(2 × content + 12, negotiated input limit)`；其类型化失败映射为四个 `HostInput*` 拒绝。
这些 action 均在 Host I/O 前拒绝错误的
viewer/grant 或非递增 sequence。已知 Host rejection 消耗 sequence；含糊或矛盾的副作用结果
关闭 broker 且不重放。

Viewer detach 或关闭先释放其 controller，再 detach viewer。仅当该 viewer 没有 controller 时，
内部 release 才是幂等的；它绝不清除不同的当前 grant。存活 grant 的意外 resize lease 撤销让 broker 退役。
已知 Dispose 后，精确 retired-target event 经校验后排空。
Release 或 loss 均不提升其他 viewer，也不终止保留执行。controller grant 上限保持八个，
与查看附件数量无关；cleanup 与普通操作共用单一 Host writer。

确定性 grant 测试覆盖互斥、过期作用域、待定获取、sequence 耗尽和显式重新获取。故障进程测试
覆盖 Host 输入确认静默或矛盾且不重放。Server 的真实双 Client 进程见证验证 Shell 输入、实际
PTY geometry、非 controller 拒绝、detach，以及另一 Client 获取全新 grant。这些运行时见证细化
现有 authority/relay 模型；它们不证明公开 TUI 行为、独立审查或实测发布性能。

### 语义粘贴准入建模记录

[`SemanticPasteAdmission`](formal/tla/SemanticPasteAdmission.tla) 是既有 carrier 请求链上的
小型有界抽象，不是通用连接或重连模型。一个请求在已建立的 carrier 上沿
`idle -> record-open -> request-ready -> admitted -> handed -> prepared -> responded ->
settled` 推进；partial 或 malformed record abandon 后进入终止 `failed` phase，对应 carrier
对连接 fail-closed 的行为——模型不声称同流 realign；完整 record 之后的字节游标对齐仍由
codec 测试承担。丢失、迟到或误分类的 Host response 把该请求结算为 `unknown` 并进入
`failed`（真实 broker 退役）；broker 替换、化身清理与跨请求后续仍归 `LocalWorkerObcnCarrier`、
`LocalHostBroker` 及其测试所有。

模型把 Server 的每 controller sequence 与 Worker 的每 grant sequence 折叠为单一
`grantSequence`，grant 集合保持静态，并把 child exit、writer stop、failure fencing 与
client input closure 折叠进 `terminalAlive`。Preparation 在 terminal mutex 下快照
bracketed-paste mode；commit 在 Core 二次授权后于独立的 input-admission 锁下发布——模型
从不声称一把互斥锁覆盖两段。模型不假设公平性：`ReservationReleasedOnce` 是转换安全性，
`SettledRequestHasOutcome` 只陈述 `settled => outcome # "none"`；sequence 恰好一次消费
由单向 phase 图得出。字节预算、身份宽度与四个类型化拒绝码仍属 codec/schema/进程测试
证据，不是模型声明。

## 默认交互式 launch 解析

当被准入的 OBCN Launch 携带显式 launch selection 时，request loop 采用其 executable
与 argument vector，仅应用下文 Windows 交互式 PowerShell 的特定调整：不查 `PATH`、不做 shell 解析、不静默回退到默认 shell——spawn 失败即
Host 的 typed launch failure。该 selection 共享同样的默认 environment 与下述 cwd/profile 组合。

Application-private `default_interactive_launch` module 仅在没有显式 selection 时为运行中的
request loop 提供 launch-fact composition。其 transport-neutral `DefaultInteractiveLaunch`
output 只持有一个已解析 executable、恰好为空的 literal argument vector，以及三个固定
environment entry。Loop 将它们与 Worker-local cwd、Terminal Protocol profile、geometry 和独立
creation identity 组合。三个 entry 映射到 Terminal Protocol launch environment。
invocation-scoped `CreateTerminal.environmentOverlay` 在显式端点路径上仍为空；在 managed
local-site 路径上承载下文所述的单个 Agent 观察变量。

单次启动的 `cwd` 由 Server 选择后传入：显式覆盖优先，其次已接纳的 Workspace 目录，
均未提供时使用 Worker 主目录。Host 负责实际目录检查和启动失败。
无论显式程序还是默认 shell，loop 都把已校验的 OBCN 环境追加到三个默认 launch entry 后。
Host 继承自身环境并应用这些逐字条目；不会修改 Worker 进程环境。
调用者不能覆盖 Orchbee 或终端身份变量。这属于 launch 数据，与上文仅用于观察的
`CreateTerminal.environmentOverlay` 分离。

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
| 限制   | 运行中的 private Worker request loop 会调用此 kernel 及其 Host adapter。Binding 与 cached result 都是 process-local，不做 crash persistence，并会保留到 kernel drop 或 higher fence 替换。1,024 limit 没有 release、TTL、LRU 或 eviction path；production 之前仍需要显式 retirement contract。在不维护无界 ledger 的前提下，kernel 无法检测 Server 跨 terminal 复用 `OperationId`。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

`TerminalLaunchAuthorization` 使用 domain 包含二的独立 counter，跟踪每个 operation 的 modeled
handoff-effect occurrence。`HostEffectAtMostOncePerOperation`、
`HostEffectCountMatchesObservations`、`HandoffRecordsExactlyOneEffectOccurrence` 与
`EffectCountAdvanceRequiresHandoff` 检查其 bound、与 identity set 的对应关系、精确 increment，并要求只有
`HandoffToHost` 可以推进它。这证明
每个 operation 最多发生一次 modeled handoff-effect occurrence；具体 transport-write multiplicity 的
证据仍由 Rust duplicate-dispatch test 提供。

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

`LocalHostBroker` 保持已接受 `HostBrokerConnection` epoch 不变。Handoff 前 Host loss 会执行
`LoseHostBeforeHandoff` 并 fail closed；替换该 Host 必须 teardown connection，并在这个单连接
lifecycle 之外重新进行 Worker/Host handshake。`ActiveHostConnectionKeepsEpoch` 与
`HostEpochInstallationRequiresHandshake` 检查这些精确 boundary。Authorization kernel 的防御性
`HostIdentityChanged` result 仍由其 Rust test 覆盖，但此 broker model 不存在 in-place
hot-replacement transition。

| 字段   | 建模的 broker 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | Server-first 双边 admission 完成后，Worker carrier 必须把一个已经由 authorization kernel 接纳的 dispatch 转换成最多一次 Terminal Host effect，并返回其精确 OBCN result。它必须保留 `begin_host_handoff` 期间 kernel 给出的 `ResultReady` decision，验证实际 Host response 而不是 diagnostic label，并且绝不能虚构 wire 未携带的 Host-rejection classification。                                                                                                                                                                                                                                                                                                                                               |
| 输入   | 已接受的 Server 与 Host link 及其不可变 handshake Host epoch；一个已经通过 kernel identity、generation、intrinsic capability、lease、fence、1,024-binding capacity 与 global-slot gate 的 OBCN Launch 或 Start `Dispatch`；Start target Host epoch；传给 `begin_host_handoff` 的 retained connection epoch；一个非零 command correlation；实际 Host response role、kind、correlation 与 epoch；timeout 或 link loss；以及一次有界 OBCN result-delivery opportunity。Worker loss 后也可能观察到 retained Host terminal。                                                                                                                                                                                       |
| 数据   | 一个 process-local broker phase；映射到 kernel 唯一全局 `empty`/`awaiting-handoff`/`handed-off`/`result-ready` slot 的一个 operation phase；Launch/Start kind；handoff-attempt 与 effect-boundary count；Start target 与 retained handoff Host epoch；mapped Host command 与 correlation；保留的 response role、kind、correlation 与 epoch；Host 是否应用 command；精确 completion source 与 cached-result fact；一个有界 delivery count；unknown retry barrier；以及 retained-terminal disposition。不引入 adapter queue 或 durable state。                                                                                                                                                                  |
| 输出   | Launch 只映射到 Host `CreateTerminal`；Start 只映射到 Host `StartTerminal`。`begin_host_handoff` 期间的 lease expiry 返回 cached OBCN `Rejected(LeaseExpired)`，且不越过 effect boundary。调用前的确定 transport failure 或 Host loss 仍是 cached `Failed(HostUnavailableBeforeHandoff)`。精确 success 变为 `HostAccepted`；精确 generic Terminal Protocol `TypedFailure` 变为 `Failed(HostReportedFailure)`。Unknown 与 retained-state outcome 沿用之前的有界 lifecycle。                                                                                                                                                                                                                                    |
| 不变量 | Server admission 先于 Host contact。Readiness 要求两条 link 都已接受且 Host epoch 非零。Active connection 的 Host epoch 不能改变。最多存在一个 kernel-global operation/result slot 与一个 Host command。成功的 `begin_host_handoff` 先于第一个可能的 Host effect；其 modeled `ResultReady` branch 会保留一次 attempted call，但 crossed boundary、command 与 effect 都为零。已知 Host completion 必须具有实际 `Response` role、request-specific expected kind、精确 command correlation 与精确保留的 handoff Host epoch。精确 generic `TypedFailure` 只能产生 `HostReportedFailure`；mismatch 会 fail closed 为 cached unknown，且两者都不能产生 `HostRejected`。Unknown 绝不 retry。                         |
| 决策   | Dispatch 前的 kernel result（包括 `CapacityExceeded`、`SlotOccupied` 与防御性 Start `HostIdentityChanged`）保持不同且绝不进入本 adapter model。Modeled handoff 期间，lease expiry 是精确 cached rejection，而不是 transport failure。Host loss 会 fail closed，且只能由全新的 connection lifecycle 安装不同 Host epoch。当前 Terminal Protocol 不存在 authoritative Host-rejection discriminator，因此相同的精确 generic `TypedFailure` input 会保守映射到 OBCN `Failed(HostReportedFailure)`。OBCN `HostRejected` 留给未来显式 classified owner contract。Post-handoff loss、timeout 或 invalid response 为 `OutcomeUnknown`。                                                                               |
| 模型   | [`LocalHostBroker`](formal/tla/LocalHostBroker.tla) 保留 Server-first handshake 与 coupled lifecycle，接收来自 Server-owned [`TerminalLaunchAuthorization`](../orchbee-server/formal/tla/TerminalLaunchAuthorization.tla) 的一个 admitted dispatch，并把详细 `awaiting-handoff`/`handed-off` phase 映射到 `WorkerTerminalBindingCapacity` 的单一 `operation-pending` global kernel slot。其 Host-side effect boundary 与 lost-reply classification refine [`TerminalHostRequestOutcome`](../terminal-host/formal/tla/TerminalHostRequestOutcome.tla) 和 [Terminal Host runtime model](../terminal-host/docs/runtime-model.zh-CN.md)；它不重新实现任何 authority，也不改变 wire contract。                     |
| 转换   | 下表覆盖 admission、lease-expiry `begin_host_handoff -> ResultReady` branch、确定 pre-call transport failure、成功 handoff、command 发出与可能 apply、concrete response validation、保守 generic failure、独立排序的 Server/Host loss、有界 exact-result delivery、unknown retry barrier，以及 retained-terminal recovery。                                                                                                                                                                                                                                                                                                                                                                                   |
| 依赖   | Adapter 依赖 authorization kernel 以及单独检查的 `WorkerTerminalBindingCapacity` bound 与 global slot、OBCN semantic value 与 codec、local-Site carrier、Terminal Protocol binding、Terminal Host runtime outcome contract、authenticated socket 与 bounded timer。Model 把 kernel 的 post-capacity `Dispatch`、decoded Host record 与 liveness 当作输入。它不选择 carrier byte、deadline value、queue implementation、activation、shell、working directory、PTY behavior 或 reconciliation policy。                                                                                                                                                                                                          |
| 所有权 | Server 拥有 placement、issuance、generation 与 reconciliation。Worker authorization kernel 拥有 binding capacity、admission/replay、精确 cached result 与唯一 global operation/result slot。Adapter 只能消费 `Dispatch`，不得增加 parallel slot 或 capacity counter。`WorkerTerminalBindingCapacity` 检查该 bound，并把本 model 的 pre-result adapter phase 折叠到其 `operation-pending` phase。OBCN 拥有 result variant。Terminal Protocol 拥有 response role、kind、correlation、epoch 与 generic `TypedFailure`，但不拥有 generic rejected/failed discriminator。Terminal Host 拥有 request commit/effect/reply ordering。Worker 只拥有 mapping、strict observation、loss classification 与 link cleanup。 |
| 验证   | TLA inventory 要求每个 admission、lease-expiry handoff-result branch、command/effect、concrete response mismatch、accepted/generic-failure outcome、delivery、ordered loss、retry-block、recovery action 与 immutable-epoch check。Trigger 覆盖 kernel 与 carrier surface、两种 Terminal Protocol binding、Terminal Host source 与 runtime model、`TerminalHostRequestOutcome`，以及已接受 topology/capacity authority。TLC 在 weak fairness 下检查 safety 与 finite progress。Host exchange 与集成 request loop 具有下文描述的 socket 和 real-process refinement witness。                                                                                                                                   |
| 正确性 | 已检查 invariant 把 operation phase 映射到 kernel-global slot；区分 attempted handoff 与 crossed effect boundary；要求精确 cached lease rejection 与 pre-call failure classification；保持 active Host epoch 不变且只允许 handshake 安装 epoch；在有界 delivery 或 abandonment 中保留完整 handoff-result projection；验证已保留 response field；把精确 generic `TypedFailure` 分类为 `HostReportedFailure`、把 mismatched generic failure 分类为 unknown，且不把两者分类为 rejected；并保留此前 one-command、unknown/no-retry、link-cleanup、result-delivery 与 recovery property。这些是 bounded model claim，而不是 Rust kernel、carrier、Host、codec 或 capacity implementation 的证明。                   |
| 限制   | State space 包含一个 Worker、一个 admitted operation、一个已安装的 nonzero Host epoch 加一个不同的 mismatched response epoch、一个 operation correlation 加一个 mismatched correlation、一个 kernel-global result slot，且最多一次 delivery。Model 从 dispatch 后开始，因此 pre-admission rejection/replay 与 1,024-binding bound 的枚举留在 kernel 和 `WorkerTerminalBindingCapacity`；显式 slot projection 阻止 adapter 虚构另一个 outstanding operation。Byte framing、数值 timeout duration、failure-code taxonomy、carrier integration、inventory 与 recovery 不在本切片范围内。该 bounded abstraction 本身不能证明 Rust implementation 或 production adoption。                                         |

| 当前状态                        | Observation 或 action                                                  | 下一状态                 | 精确 result 与 lifecycle 规则                                                                                        |
| ------------------------------- | ---------------------------------------------------------------------- | ------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Server admission 前             | Host connect attempt                                                   | Forbidden                | 没有启用 Host contact transition。                                                                                   |
| Ready，kernel slot empty        | 消费一个 kernel `Dispatch`                                             | `admitted`               | 把 global slot 映射到 `awaiting-handoff`；capacity、replay、rejection 与 occupied-slot result 不会 dispatch。        |
| `admitted`                      | `begin_host_handoff` 观察到 expired lease                              | `result-ready`           | Cache 并 queue 精确 `Rejected(LeaseExpired)`；记录一次 attempted call，不越过 effect boundary，也没有 Host command。 |
| `admitted`                      | Handoff call 前的确定 transport failure 或 Host loss                   | `result-ready`           | Cache 并 queue `Failed(HostUnavailableBeforeHandoff)`，与 lease rejection 保持不同。                                 |
| `admitted`                      | 使用保留的不可变 Host epoch 成功调用 `begin_host_handoff`              | `handed-off`             | 恰好一次越过具名 external-effect boundary，并把 kernel slot 映射到 `handed-off`。                                    |
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
`begin_host_handoff`，并在发送任何 Host message 前检查其返回：原样转发精确 `ResultReady` lease rejection，
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
| Handoff          | 使用保留的不可变 handshake epoch 与新采样的 lease clock 调用一次 `begin_host_handoff`。它的 `ResultReady` branch 不发送内容，保留精确 cached lease rejection。只有 `HandedOff` 才允许第一次 write。                                                                                            |
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

## Agent drain 队列与 request loop

每个已接纳 Worker 到 Server connection 拥有一个 `AgentDrain` 队列（`src/agent_drain.rs`），
在任何 Host 启动之前创建。共享的 `Rc<RefCell<AgentDrain>>` 句柄是
`HostBrokerConnection` 两条 connection 路径上的必需构造参数——eager 路径在 Server
admission 与 Host connect 之间建立它，lazy 路径在 pre-Host loop 顶部建立它，并把同一句柄
穿过 `start_host` 传入 admitted loop。不存在可选 owner、空 fallback，也没有 per-viewer
或 per-request 分配；队列生命周期恰好等于已接纳 Server connection 的 epoch。
`Rc<RefCell>` 是合理的，因为 Worker event loop 是单线程的，且 borrow 始终在任何 Server
I/O 之前释放：`server_transport::send` 在 POLLOUT 等待期间可能消费 Host event，而这些
event 会追加到同一队列。

`HostBrokerConnection::receive_event` 是唯一生产者。在 envelope 与 Host-epoch 校验之后，
它解码每条 kind-58 application signal，通过既有 launch adapter 转换 Terminal Host
identity，并在借用 viewer 之前追加到共享 drain——没有附着 viewer 的 terminal 同样入队。
携带外部 Host epoch 的 signal 仍是致命的；到达 viewer 路径的 `ApplicationSignal` 现在是
非预期路由，而非被静默丢弃。Bell 与 Clipboard 路由不变。

队列至多保留 1,024 个 entry 与 4 MiB 编码字节。溢出逐出最旧 sequence，由合并出的头部
`Gap` 覆盖——该 GAP 自身消耗一个槽位与字节。Reply 同时受 480 个 entry 与 8,165 编码
entry 字节的分页约束；一次成功 send 提交请求时捕获的水位，`Ok(false)` 与 `Ok(true)`
两种写结果同样提交。超过 offered 水位的 acknowledgement 是致命的；stale 或零
acknowledgement 是 no-op，未确认 sequence 继续处于 offered；回显 token 与请求的完全
一致。

Pre-Host loop 用与 admitted loop 相同的共享 dispatch 应答 drain request，因此 Host 存在
之前的请求返回真实的空页而不启动 Host，且同一 owner、token 与 acknowledgement 状态
穿越首次 launch 保持连续。

## Terminal cwd validation

集成 request loop 在 terminal authorization 或 viewer/controller 清理之前处理
`TerminalCwdValidationRequest`。Linux lazy loop 会在解析 launch fact 或启动 Terminal Host 之前
处理它。Worker 首先将请求中的精确逻辑 Worker ID 与代次同自身已准入 OBLS 身份比较。不匹配
时不观察路径，直接返回 `WorkerUnavailable`。匹配时执行一次同步 `std::fs::metadata` 查询，
跟随符号链接并保留输入路径拼写；仅当现有对象为目录时返回 `DirectoryAvailable`。路径缺失、
普通文件及所有元数据错误均返回 `DirectoryUnavailable`；OBCN 不传输任意 OS 错误细节。

Validation 是一个时间点只读观察。它不创建目录、terminal、Host、attachment、controller、
lease、launch binding 或 replay cache entry，也不发送 Terminal Protocol record。响应复用既有
有界 OBCN 读写路径。Server 仍拥有 association admission、当前 Worker revalidation 及 canonical
state。文件系统元数据调用为同步调用，carrier deadline 不会使其可中断；当前 runtime claim
仅限受支持的本机 Linux filesystem cohort。Unit test 覆盖真实目录、保留拼写的 symlink 查询、
普通文件和缺失路径，以及不依赖 Host 的 Worker 身份／代次拒绝。

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

## Close 生命周期单步模型

已接受的 [Close 契约](../orchbee-server/docs/terminal-tab-close.zh-CN.md) 由
[`WorkerTerminalLifecycleStep`](formal/tla/WorkerTerminalLifecycleStep.tla) 细化。这是尚未发布 cohort
的有界模型证据。Worker 实现及其确定性／进程见证映射如下；Server 编排与 reservation 所有权保持独立。模型拥有一个已授权生命周期
请求或 inventory page 及其精确目标缓存结算，不拥有 Server 的完整 inventory walk 或 Close attempt。

`identity` configuration 探索 Terminate、Force、Dispose 和一个 inventory page，以及一个当前
product/Worker/generation/Host epoch/creation/terminal tuple 与每个字段的独立不匹配值。
`target-cleanup` configuration 固定 Dispose，探索空、目标、无关以及满（两个 slot）的 viewer
cache，目标／无关 controller reservation，不同的 old/new controller ID、已排队的目标 frame/feedback、
旧 lease 和已接纳的 Acquire。这些有限边界不证明实际八个 slot 的边界。初始状态假设一个已经认证接纳、
epoch/generation 不变的 broker；没有替换或接管转换。Configuration 不使用 symmetry、state constraint
或 fairness 假设，也不承诺清理最终成功。

| 所有的 claim                                      | 模型动作和属性                                                                                                                                                                                                                   | 必需的实现细化                                                                                                                                                                              |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 精确的新鲜授权，以及每个 step/page 一个 Host 请求 | `RejectAuthorization`、`RejectFreshHandoff`、`SubmitHostRequest`；`HandoffRequiresFreshExactAuthorization`、`OneHostRequestPerStep`                                                                                              | 在副作用前拒绝每个错误的 product/Worker/Host 字段、grant 和过期 handoff；统计每种 step 的实际 Host 请求。                                                                                   |
| 已知确认与不确定交付保持区分                      | `ReceiveExactAck`、`ReceiveTypedFailure`、`ReceiveInvalidAck`、timeout/loss 动作；`KnownAckRequiresFullCorrelation`、`KnownDisposeNeverRegresses`、`UnknownNeverDispatchesAnotherLifecycleRequest`                               | 检查实际成功 ACK 的 terminal ID 和 envelope role/kind/epoch/correlation；typed failure 保守分类，不虚构 terminal 回显。另行检查 OBCN result 的完整保留 identity 和 operation/attempt/step。 |
| Dispose 成功等待精确的 Worker 和 Server 清理      | `CommitWorkerTargetCleanup`、`ObserveExactWorkerResult`、`CommitServerTargetCleanup`；`WorkerDisposedResultRequiresTargetCleanup`、`TargetFrameAndFeedbackFreed`、`ServerCompletionRequiresExactReservationRevocation`           | Dispose 前准备有界精确清理；无分配地移除目标前保留已知 ACK；覆盖全部八个 slot、排队 frame/feedback、无关项保留、Worker result 丢失和 Server 清理失败。                                      |
| Closing 阻止新 grant，旧清理保持精确              | `RejectAcquireAfterClose`、`DropOldLeaseExactMatch`、`DropOldLeaseMismatchedReservation`、`DropOldViewer`；`LateAcquireCannotRestoreClosingTarget`、`OldLeaseCannotDeleteDifferentReservation`、`OldLeaseAfterRevocationIsLocal` | Acquire 确认后重新校验；撤销后的旧 lease Drop 不发送过期 Host 调用，也不能移除较新的 controller ID。Unknown Close 仍允许合法的精确 lease 清理。                                             |
| Attempt 到期保留已接纳 receipt                    | `ExpireAttempt`、`ReceiveLateExactAck`；`ExpiryCannotDiscardOutstandingOrKnownReceipt`                                                                                                                                           | 超过 30s attempt 接纳期限仍保留在途 result 和精确迟到 ACK；Resume 或接纳下一步前先结算。                                                                                                    |

`prepared` 包括已经接纳到 carrier 的 step 和清理 seal；`sent` 是首次可能发生 Host 副作用的位置。
它们映射到 `LocalHostBroker` 的 admitted/awaiting-host phase，随后对应其 result-ready 和
completed/abandoned 边界。模型不驱逐 `WorkerTerminalBindingCapacity` 所有的 launch binding。
Generic typed failure 检查相关 envelope，即使此前发生副作用仍归类为已知失败。
Inventory-page success 只是一个不透明的精确 page 观察，绝不表示不存在或允许 Dispose。
Host containment 和完整 walk 校验仍分别属于 Host 和 Server owner。

独立的到期动作抽象现有 **1s Worker HostExchange**、**2s Server frame I/O**、**20s Server response
wait** 和 **30s Close attempt 接纳**边界。它们不模拟数值时间，也不保证返回／join 时间。
即使 Server 只能报告 Unknown，Worker 的精确 Dispose 知识仍在丢失后保留。Server 本地清理可以无限期
保持 pending；含糊的生命周期副作用后没有自动 retry。ACK 后不分配的 claim 需要代码内有界移除证据；
此模型只证明排序，不证明 allocator 行为。

生命周期 adapter、carrier result、viewer/controller 清理或 Server reservation fence 改变时，同步
更新此模型和注册的 action coverage。当受维护的替代模型拥有这些 claim 及其可执行细化映射时，
一起移除此模型和注册。

### 保留的生命周期 receipt

已接受的 Worker 细化在每个现有保留终端 binding 内保存一个可选 Close cell，不驱逐 binding，也不增加
第二个全局 operation/result slot。Cell 拥有固定的 Close operation/target、最高 attempt/step、最后一个
精确 step receipt、最后一个精确 mutation receipt，以及不可回退的 mutation-Unknown/KnownDisposed。
[WorkerTerminalLifecycleReceipts](formal/tla/WorkerTerminalLifecycleReceipts.tla) 在单步模型之外
单独检查此有界序列。[Lifecycle kernel](src/terminal_authorization/lifecycle.rs)
在现有 binding 和全局 slot 内实现该 receipt 状态。

校验当前 Worker/generation/target association 和 lifecycle capability 后，精确 last-step replay 返回
缓存结果；否则精确 last-mutation replay 在 query 后仍可读取。复用这两个保留 request ID 却修改字段返回
`LifecycleConflict`。其他过期／冲突 step 无副作用拒绝；没有无界 receipt ledger。新 dispatch 需要新鲜
lifecycle lease 和 handoff 再校验；精确缓存 replay 不要求续租。当前 attempt 要求严格更大的 step ordinal；
更晚 attempt 从 ordinal one 开始。Wire domain 是 ordinal **1..32** 和非零 u64 attempt generation；
两者均不回绕，耗尽时仍可读取精确保留的 replay。

Inventory 观察不能抹去 mutation Unknown 或 KnownDisposed。Mutation 一旦 Unknown，就拒绝所有后续
mutation，包括更高 attempt。当前 Server 将此状态停在不可恢复位置，不授予新的 query/reconciliation
预算。已知 blocked 状态可以 Resume；Worker 允许观察不意味着新增 Server recovery 路径。
KnownDisposed 后 query 失败也不能抹去 disposal；仅精确保留的 Dispose receipt 能 replay Disposed，
并且必须已经完成单步清理边界。新的 termination 或 Dispose 均被拒绝。

原始精确缓存 Launch/Start replay 优先于 `LifecycleActive` fence。同一 product binding 上的不同操作或
更高 fence replacement 不能重置 Close owner 或启动新的 execution。现有全局 slot 是单 slot 的结构
前提，不单独证明并发正确性。

| 序列 claim                | 必需的有限观察                                                                                            | 必需的确定性细化                                                                              |
| ------------------------- | --------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| 保留 ID 冲突不能 dispatch | Candidate 保留旧 request ID 却修改 attempt/ordinal；实际 handoff 计数允许错误值 two，并独立要求最多 one。 | 相同 ID 的更高 ordering/action 冲突；query 后精确旧 mutation replay；替换后拒绝更旧 receipt。 |
| Query 保留生命周期事实    | 观察 mutation Unknown 后的实际 handoff history，以及 query Unknown 后保留的 KnownDisposed。               | Unknown 后 query，再拒绝更高 attempt mutation；Dispose 后 query failure 和精确 replay。       |
| 耗尽不会抹去 replay       | 完成 maximum-attempt/maximum-ordinal receipt，再到达其精确 replay。                                       | Ordinal 31/32/耗尽和 attempt MAX-1/MAX/耗尽，无回绕或缓存结果丢失。                           |

新增 receipt 模型固定一个现有 association、两个 attempt、两个 ordinal、两个新鲜 ID variant 和一个
冲突 Close owner。派生的新鲜 ID 允许多个 query；显式保留 ID adversary 在冲突场景中使 identity 独立于
ordering。它不包含 transport、cache product、fairness 或成功 recovery；单步模型拥有 handoff/cleanup
细化。Runtime adapter 必须在可能发送额外 Host 请求的普通 `cleanup_closed` 前分支处理 lifecycle。
Dispose 使用准备好的固定目标 ID 和现有无分配移除。当前 Host Dispose 移除 attachment 而不发出 false
resize-lease event；本 slice 保持现有 event rule，不增加推测性的 pre-ACK revocation policy。

### Worker runtime 细化

[Lifecycle adapter](src/terminal_lifecycle_operation.rs) 消费一个已接纳请求，并复用现有一秒
`HostExchange`。其 dispatch 分支先于 `cleanup_closed`，因此无关的已关闭 viewer 不能为 lifecycle
step 增加 Host 请求。Preflight Host 丢失是已知的 `HostUnavailableBeforeHandoff`；handoff 后丢失
是 Unknown。精确 ACK 知识在 result 交付前保留，包括观察 ACK 后 Host 或 Server 丢失的情况。
Inventory 请求每次只查询一页，最多 128 个 terminal，同时受 Host control-record byte bound 限制。
投影目标前检查每个解码 row、identity collision、cursor generation 和 offset。

[Target cleanup](src/host_connection/target_cleanup.rs) 在 Host I/O 前捕获最多八个精确 viewer ID 和
一个精确 controller ID，并预留一个 generation-local retired-target cell，受现有 1,024 个 binding
限制。精确 Dispose ACK 后，提交已准备的 cell 并移除现有 map entry，不分配内存，也不增加 Host
exchange。丢弃 viewer entry 会释放排队 record 和 feedback flight。精确 retired ID 吸收经校验的
迟到 event，使旧 viewer Detach 本地完成；拒绝向已 disposed 目标新建 attachment。没有 ACK 的
prepared cleanup 既不移除 owner，也不建立 disposal 事实。

| Runtime 边界                                       | 可执行证据                                                                                                                                                                                                      |
| -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Receipt 排序、identity、到期与耗尽                 | [Kernel 测试](src/terminal_authorization/lifecycle/tests.rs) 覆盖多次 query 后的旧 mutation replay、修改保留 ID、Unknown 后全部后续 mutation、原始 Launch/Start replay 和 `LifecycleActive` fence。             |
| 一个请求、ACK correlation、page 校验与 result 丢失 | [Adapter 测试](src/terminal_lifecycle_operation/tests.rs) 覆盖带无关已关闭 viewer 的实际 dispatch loop、所有 mutation ACK kind、一个固定上限 page、每种目标 phase、identity conflict 和 pre/post-handoff 丢失。 |
| 精确有界无分配清理                                 | [Cleanup 测试](src/host_connection/target_cleanup/tests.rs) 覆盖零／一／七／八个 viewer、第九个拒绝、排队 record/flight 移除、七个 controller 保留、不同的后续 ID 和 1,024-cell 边界。                          |
| 真实 Worker 与 Host                                | [进程见证](tests/process/terminal_operations/lifecycle.rs) 保留八个目标 viewer entry，执行 Force 并观察结算，Dispose 目标，inventory 后 replay，本地 detach 旧 ID，并向 sibling attach 八个新 viewer。          |

这些 Worker 见证不证明 Server 的八个存活 Client reservation、延迟 Acquire 再校验、旧 Client lease
Drop 或最终 Closed 发布 gate。这些仍是 Server 所有的细化义务；Worker receipt 不允许自动恢复
mutation Unknown。

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
node scripts/tla/run-models.mjs --models worker-terminal-lifecycle-step --jobs 1
node scripts/tla/run-models.mjs --models worker-terminal-lifecycle-receipts --jobs 1
pnpm verify
```

## Windows 原生执行

### Git 进程归属

Windows Git 发现为每个命令持有一个在暂停状态下分配的 Job。私有适配器先轮询内部
根进程，随后终止剩余 Job 成员，并轮询根进程退出与准确的全进程退出完成事件。
结算在保留的发现线程上拥有一次五秒协作式清理预算。只有成功结算才允许终态回执、
下一个命令或复用槽位；失败将槽位置为 `PhysicalUnknown`，不能重试并当作成功证明。

状态为运行、结算、已结算或物理状态未知。结算保留 Job 和线程所有者。操作系统调用
可能超过协作式期限；`poll` 保持 pending，`stop_before` 有界返回失败而不声称已清理。
Drop 只请求终止，不阻塞也不证明退出。Worker 突然死亡和操作系统终止失败不属于成功
清理声明。根进程退出或 Job 成员列表为空，都不能证明所有后代进程句柄已经发出退出信号。

与原生 Client 共享已有的 `process-wrap` 10.0.1 `std`、`job-object`、`creation-flags`
特性和 `windows` 0.62.2 Threading 绑定。经过审查的
[依赖修正](../../third-party/process-wrap/README.zh-CN.md) 修复完成通知的判定，
不新增自写 unsafe 或依赖版本。Client 的
[依赖审查](../orchbee/docs/windows-agent-integrations.zh-CN.md) 仍约束已有绑定。
原生测试使用已有固定版本 `winsafe` 0.0.29 `kernel` 作为仅 Windows 的开发依赖：
在放行根进程或取消前保留存活后代的句柄，要求终态返回时句柄已发出退出信号。
现有发现测试覆盖二进制输出、取消、边界、失败清理与真实 Git 执行。

Windows 托管原生路径沿用 Worker 授权、回执、重放栅栏及 Host 协议，以私有命名管道替换 Unix Socket。精确的同级 `orchbee-terminal-host.exe` 仅在授权 Launch 后惰性启动并使用 ConPTY。默认启动使用配置的 Shell，或绝对 PATH 中首个兼容的 PowerShell 7，再回退到系统 Windows PowerShell，初始目录为 `USERPROFILE`。工作树发现通过自有子进程输出管道运行 Windows Git，具备输出边界、取消、截止时间及 kill/wait 清理。原生测试覆盖 broker 断开后的真实 Host 保留和真实 Git 发现。暂存和限制见 [Client 快速开始](../orchbee/README.zh-CN.md#windows-原生快速开始)。

## Windows 子进程调用者上下文

每次已接纳的 Windows Launch 都向 Terminal Protocol 环境 overlay 添加
`ORCHBEE_TERMINAL_ID` 和 `ORCHBEE_WORKER_ID`。Worker 从产品启动授权中派生完整
小写十六进制值；Host 在继承环境与每次启动的环境值之后应用它们。它们是原生 CLI
的查询提示，绝非凭证。现有 overlay 上限不变。显式端点 Windows 启动同样适用；
观察通道策略独立。Panel 后续可能移动，因此不储存 Panel 或 Workspace 位置。
原生 `panel current` 查询从一次 Server 快照解析位置。不会修改已有子进程，
本次不引入 Mac/Linux 上下文注入。

Windows 还会在连接 Server 之前，使用已有的 OS 随机源生成一个非零 128 位 Worker ID。
`WorkerHello`、保留的 `ServerConnection` 和操作内核共享该精确身份。不同连接不再
复用原先固定的本地诊断 ID，否则不同 Site 的终端计数器可能误匹配旧调用者。随机源
失败或返回零值时，在连接前失败。抵抗身份碰撞依赖 OS 随机源。此修改不新增
OBLS/OBCN 字段，也不启用重连、inventory 接管或旧 Worker 绑定迁移。
Mac/Linux 保留原有身份行为。两次独立 Windows Site 进程轨迹覆盖 Terminal ID 复用
时对旧 Worker 上下文的拒绝。

## Windows Worktree 变更所有者

[私有变更契约](../../crates/capability-node-protocol/README.zh-CN.md#worktree-变更作业契约)
拥有受检意图与 wire 词汇。`worktree_mutation_job` 拥有接纳、保留意图身份、单个物理线程
和完成回执；Windows Git 适配器拥有字面量 argv、有界输出及精确进程收敛。Server
续执行仍负责 Workspace 关联与端到端 CLI 恢复。

Start 在接触槽位前校验 Worker 身份／generation。相同保留 ID／意图观测 Existing；
变更意图冲突。单个运行作业排斥其他作业，生命周期 64 次接纳在不让变更身份过期的
前提下限制回执。channel 仅在意图及 join handle 已保留后释放所有者。Poll 只 join
已结束线程。panic 或未证实的进程清理退役新接纳，并保持 PhysicalUnknown。取消只是
请求，不是回滚。Site 关闭在报告后台工作停止前取消并 join 此所有者；从无 Host 到有
Host 的交接保留同一槽位。正常传输退出也在协作式六秒清理窗口内取消并 join。
Drop 只请求取消。

Git 执行具有 30 秒工作预算、64 KiB stdout 上限及 2 KiB 诊断保留。这些是协作式
接纳预算，不会抢占文件系统调用。受跟踪的原生创建边界区分已知无子进程失败与创建后
包装失败；后者退役 discovery 和 mutation 所有者的新接纳。每次读取均非阻塞
且按量限界；取消或超限使用独立的五秒清理预算收敛既有 Job。在变更可能影响文件系统
之前，变更交付将失败观测改为 MayHaveChanged。只接纳现有 Windows 绝对盘符路径
契约；拒绝 UNC／device 及盘符相对路径。继承的 `GIT_*` 变量不能重定向仓库、
index 或配置。显式信任只作用于本命令的源仓库，绝不修改全局配置。

Create 检查已有本地分支，再选择检出或使用 `-b` 与 base；已有分支忽略 base，与
Herdr 一致。Open 按路径或分支选择一个已登记 checkout。InspectRemove 观测登记及
脏状态。Remove 复查公共目录身份、排除主 checkout，并调用 Git 的 dirty／force 策略，
不删除分支，也不使用递归文件系统回退删除。Checkout 和公共目录事实是规范化观测，
不是对其他 Git 进程或文件系统写入者的事务。变更后观测失败保留可能发生的副作用，
不宣称回滚。

原生测试拥有一次性 Git 夹具，直到最后一个子进程、管道及仓库消费者结束。转换测试
覆盖重复／冲突 Start、容量、取消、收敛失败及 generation fencing。真实 Git 测试覆盖
分支创建／复用、打开选择器、脏目录拒绝、强制删除及分支保留。不证明外部写入者排斥、
Worker 突然死亡恢复或非 Windows 执行。不会仅凭此 Worker 边界推导 CLI 对齐。

InspectSource 解析包含源目录的检出；Open 允许主检出。Create 在 Git 完成后重新观察分支，因此成功 hook 改变 HEAD 时返回 RepositoryChanged 与可能的副作用，不会误报关联成功。Server 通过其保留续执行消费这些观测；物理所有权和清理仍留在 Worker。

## Windows 按来源读取 Worktree

`StartSource` 复用发现所有者及物理槽位。准入仅校验驱动器语法。所属任务规范化目录并准备信任，在启动前复查取消／deadline，再观察 Git 公共目录和检出根，再在同一个五秒 job 预算内列举 porcelain 条目。共享 `git_repository_windows` 清除继承的 `GIT_*` 仓库／配置注入，仅对本命令选定的准确规范仓库应用可选信任；变更也使用相同命令准备。原 Start 字节与行为不变。真实 Git 测试覆盖主／关联检出的子目录、不改动脏文件／配置、准确信任范围及所有者关闭。参见[来源查询契约](../orchbee/docs/windows-worktrees.zh-CN.md#按来源发现-worktree)。

## Windows 交互式 PowerShell 目录模型

启动事实所有者仅在 Windows 识别 `powershell[.exe]` 和 `pwsh[.exe]`，
参数为空或最多三个完整且忽略大小写的 `-NoLogo`、`-NoProfile`、`-NoExit` 开关。
保留这些开关，通过 `-NoExit -Command` 添加固定的进程内 prompt 包装。
未知开关、脚本、命令字符串、其他程序和其他平台均保留原始启动参数。

包装捕获 profile 定义的 prompt，首先调用它，使依赖状态的 prompt 能看到上条命令的成功状态，
随后将 FileSystem provider 位置同步到 `[Environment]::CurrentDirectory`。
非文件系统 provider 或赋值失败保留进程目录。每进程只安装一次，不写用户 profile，
不插入启动数据，输出原 prompt 结果。原生 CLI 创建因而可通过根进程采样观察目录变化，
无需获取终端输入权。

这属于既有 Host 交接之前的启动准备，不是第二个 Shell／进程，也不表示终端已启动。
现有终端不会补装。准入单测和实际 Windows PowerShell 5.1／7 prompt 测试覆盖自定义 prompt、
失败命令、Unicode 目录、非文件系统 provider 和幂等。实际 ConPTY 创建检查 Set-Location 后的
根进程 cwd。任意自定义启动命令不在集成范围内，不声称其 Shell 私有状态的目录准确性。

## 显式控制权接管

Takeover 在任何 Host 效果前验证预期的已提交旧控制者及目标。单操作循环释放精确旧
controller，等待 DetachTerminalAccepted，然后 attach 新 input/resize controller。迟到的
旧效果无法通过 token 准入；迟到旧清理不能释放替代授权。detach 后 Host 拒绝新授权时，
旧授权保持撤销。缺失 detach 证明会以 broker error 结束操作，绝不尝试新 Attach。
Windows 可运行的本地管道测试覆盖真实协议交换，不代表打包 Worker/Host 进程验收。
[Server 交接模型](../orchbee-server/docs/terminal-controller-takeover.zh-CN.md)拥有联合契约。

## Windows 默认 Shell 配置

Worker 在匹配版本的托管启动中接收自有 `WindowsShellSettings` 快照，或在显式端点接入前读取一次共享配置。延迟 Host 前置循环与接入后循环借用该快照作为默认启动准备的回退值。显式可执行文件/Profile 绕过它，精确重放不重复解析。[Windows 配置契约](../orchbee/docs/windows-cli-configuration.zh-CN.md#windows-配置-shell) 负责可执行文件语法、PATH 解析、模式参数、启动生命周期和证据边界。

新的 Server 授权默认启动可用冻结的 Windows Shell 策略覆盖启动回退。显式程序始终优先。准备时校验策略并仅解析一次 Worker 本地事实；缓存重放和延迟 Start 保留这些事实。无效语法或不可用程序仅让该次交接前操作失败，延迟和已接入 Worker 循环均继续接收新的修正请求。参见[在线默认值](../orchbee/docs/windows-cli-configuration.zh-CN.md#后续-windows-终端的在线默认值)。

## Windows 插件进程归属

受理请求循环在 Host 创建前后处理有界插件任务 start/list/get/cancel 记录。`plugin_jobs` 保留最多 32 个命令 owner 和 200 条日志；共享 `orchbee-local-site-runtime` Windows 命令所有者 复用 Git 的 Job 清理方式。CLI 断开后已受理任务继续运行。显式停止 Site 时，必须先通知、清理并等待所有插件 owner 结束，才能报告成功。[原生插件行为](../orchbee/docs/windows-plugins.zh-CN.md) 管理调用、分页、输出上限及剩余集成。

## 插件终端启动环境

显式终端启动可携带独立的 `PluginTerminalEnvironment`（OBCS 字段为 `pluginEnvironment`）。只允许插件 ID/root/entrypoint/config/state/context、原生二进制路径及可选 Workspace/Tab/Pane 展示 ID：十个唯一名称，七项必填，合计最多 24 KiB UTF-8。这些是展示数据，不是权限或插件登记。普通调用者环境保持原有上限及保留名称拒绝规则。runtime base/scope 和 Worker/终端身份仍由 Worker 适配器负责。插件环境必须配合显式可执行文件。OBCN 扩展 3 传递显式启动选择、普通环境和插件环境；旧编码及 40 KiB 帧上限不变。Server 在预留物理操作前校验完整插件启动编码。Worker 先清除继承的已知插件展示字段，再应用本次值。Workbench 插件解析、Pane 归属和 Client 布局仍是独立的集成职责。定向边界及启动事实测试覆盖此载荷，不代表插件 Pane 已完整接通。

## Windows 物理标题投影

已接纳的 Host 连接在 viewer 消费者之前保留精确的
`TerminalMetadataChanged` 标题观察。其 `Titles` 投影拥有至多
1024 个身份和 64 MiB 标题文本，拒绝版本倒退和同版本变化，
并在确认物理处置或连接结束时释放观察。
原始标题是展示数据，独立于 Agent 报告和输入。

只读标题批次使用现有低优先级通道。Windows 懒启动循环返回不可用，
不会启动 Host。已接纳循环每次消费至多 32 个排队事件，无需繁忙流整体静止，
返回完整精确身份、版本与可空
标题；不创建附着或新线程。无效/损坏的 Host 事件使连接退出，
不会提升旧标题。详见[标题协议](../../crates/capability-node-protocol/README.zh-CN.md#windows-物理标题观察)
及[Client 模型](../orchbee/docs/machines.zh-CN.md#实时终端标题观察模型)，
了解帧、Site 栅栏、渲染及证据边界。

## Workspace Git 状态读取

`StartStatus` 与来源查询共用自有发现槽及五秒预算。
字面 `git status --porcelain=v2 --branch --untracked-files=no
--ignore-submodules=all -- :(top,exclude)**` 仅输出分支头。
命令局部 `core.fsmonitor=false`、`core.untrackedCache=false`、关闭可选锁、隔离 Git 环境及不扩大信任，
防止意外 hook／缓存／配置写入；不 fetch。终态回执要求子进程／管道结算，保留精确 status/stderr 边界。
真实 Windows Git 测试覆盖 ahead/behind、无 upstream、游离／未出生／关联／非仓库状态及数据／配置不变。
参见[状态协议](../../crates/capability-node-protocol/README.zh-CN.md#windows-workspace-git-状态观测)
及 [Client 模型](../orchbee/docs/machines.zh-CN.md#工作区侧栏模型)。
