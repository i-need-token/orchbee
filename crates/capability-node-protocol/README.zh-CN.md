# Capability Node Protocol

这是 Orchbee Server 与 Capability Node 之间 authorized terminal operation 及有界目录观察的
私有实验性 Rust semantic contract。首个 Capability Node 是 Worker，但本 crate 不会把 message
绑定到本机 socket 或任何其他 carrier。本机 Server adapter 现在会在 application-private
post-OBLS carrier 中消费其 codec。本机 Server coordinator 通过该 carrier 提交获授权的
Create/Start 与绑定代次的 cwd validation。它不是 public API 或 compatibility promise。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## 本地 Close 生命周期步骤

[规范终端 Tab Close](../../apps/orchbee-server/docs/terminal-tab-close.zh-CN.md) 拥有已接受的
Server 继续执行及精确 Worker/Host 清理契约。其 OBCN 扩展是窄范围获授权生命周期步骤，
携带产品绑定、Worker 代次、完整 Host 身份及操作/步骤关联。每步至多准入一个既有 Host 请求；
仅有 Viewer/controller 授权绝不允许终止。保留效果或发布 Disposed 前必须验证含目标的 Host
确认及完整 OBCN 结果关联。

Worker 保留精确生命周期观察，在返回 Disposed 前仅清理已销毁目标的 viewer/controller 及
排队 feedback/frame 条目。Server 保留精确目标撤销并清除匹配预留；确认丢失时，由已观察的
所有者保留已知/未知事实。新的 Control Acquire 与 Attach/Input/Resize/Start 一起受到栅栏
保护。Close 保留既有 1s Host exchange、2s Server 帧 I/O 及单独 20s 响应等待；Server 的
30s 尝试预算仅控制下一步准入，不能丢弃未结回执。

生命周期编解码器及 Server carrier 关联实现以下有界表示。Worker 执行及 Server 继续执行
仍待实现；编解码器不实现重放、准入、清理或完整库存遍历。协议、Server、Worker 及原生
Close 共同激活。旧私有 cohort 拒绝新增 tag；不定义混合 cohort 回退。

kind 9 `TerminalLifecycleRequest` 和 kind 10 `TerminalLifecycleResult` 保持帧版本 1 及
8 KiB payload 上限。整数均为无符号大端，ID 均为非零 16 字节值。请求 kind 后依次为：
生命周期能力版本（`u16`，当前为 1）、产品 Terminal ID（16）、Worker ID（16）、Worker
代次（正 `u64`）、保留绑定 fence（正 `u64`）、新 lease ID（16）、lease Unix 毫秒到期值
（正 `u64`）、Close 操作 ID（16）、步骤请求 ID（16）、尝试代次（正 `u64`）、步骤序号
（`u8`，1 至 32）、Host creation/epoch/terminal ID（各 16）及 action（`u8`）。生命周期
能力独立于 launch 能力版本 2。fence 精确引用当前保留的 Launch/Start 绑定，不替换它。
新生命周期 lease 独立于原 launch lease，不包含 Viewer/controller 授权。已认证 Server
签发新的步骤请求 ID；仅编解码校验不能证明授权或重放安全。

action tag 1 至 4 为 Terminate、Force、Dispose 和 InventoryPage。InventoryPage 添加
cursor 选项 tag（`0` 无，`1` 有）；有 cursor 时添加正库存代次及 offset（各 `u64`）。
每次 Host 查询固定请求最多 128 个终端。结果回显完整请求体，再添加 outcome tag：
1 TerminateAccepted、2 ForceAccepted、3 Disposed、4 InventoryPage、5 Rejected、
6 HostFailure、7 InventoryFailure、8 OutcomeUnknown 或 9 HostUnavailableBeforeHandoff
（无额外 payload）。构造器及解码拒绝 action/outcome
矛盾。Host 成功校验 ACK 的实际终端 ID 及 envelope 的 role、kind、epoch 和 correlation；
保留的完整请求 seal 提供 Host ACK 未回显的产品、creation、Worker 和尝试身份。

InventoryPage 添加正库存代次、可选 next cursor、返回行数（`u16`，0 至 128）及目标
选项（`0` NoMatch，`1` Match）。Match 添加 revision（`u64`）和 phase：1 AwaitingStart、
2 Running、3 Finishing、4 ContainingFailure、5 Exited、6 Failed。Running、Finishing 和
Exited 添加精确 `terminateRequested` 布尔值（`0` 或 `1`）；其他 phase 不携带此字段。
Match 要求返回行数为正。行数涵盖所有扫描的 Host 摘要，而非匹配数。成功页代次必须匹配
请求中提供的 cursor；next cursor 须保持同一代次，按返回行数精确前进且不溢出。NoMatch
仅表示本页未匹配，绝不表示不存在。Worker 校验完整 Host 页，发现重复或矛盾的 terminal ID
或 creation ID 匹配时加以隔离，再投影精确目标。Server 拥有跨页一致性、重复匹配、完整耗尽、
不存在判断及继续执行。Finishing 和 ContainingFailure 不是已结算结果。此固定大小投影
不携带原始 Host 库存、theme、配置或无关身份。

Rejected 添加 reason tag 1 至 8：WrongWorker、StaleWorkerGeneration、UnsupportedCapability、
LeaseExpired、BindingUnavailable、BindingFenceMismatch、HostIdentityMismatch、LifecycleConflict。
这些是交接前拒绝。LifecycleConflict 保守涵盖不同 Close 所有者、旧步骤及冲突步骤；它
绝不授权重放结果不明的变更。HostFailure 的 reason tag 1 和 2 为 HostRefused 和
CleanupIncomplete；后者仅允许用于 Dispose。关联的 Host 类型化失败是已知观察，不证明
没有效果。InventoryFailure 仅允许用于 InventoryPage，添加 tag 1 至 4：StaleCursor、
PageRejected、InvalidPage、IdentityConflict。前两者分类关联的 Host 失败，后两者分类
矛盾页观察。OutcomeUnknown 添加 tag 1 至 3：TransportLost、Deadline、InvalidResponse。
查询未知结果在 Server 工作流映射为 inventory-incomplete；变更未知结果保留受影响的
生命周期 phase。不序列化任意错误文本。

HostUnavailableBeforeHandoff 可用于每个 action，表示已知未发送任何生命周期 Host 请求。
它不会撤销已准入的 Server 到 Worker 步骤及其继续执行计数。Worker 仅在首次可能的
生命周期 Host 写入前发出此结果，包括 `HostExchange::prepare` 期间失败；后续写入边界
由 `round_trip` 拥有。Server 将此已知不可用映射为 CleanupBlocked(worker-unavailable)，
绝不映射为 HostFailure 或 OutcomeUnknown。

生命周期请求最多 182 payload 字节，结果最多 221（含帧分别为 190 和 229 字节）。手工
推导的 golden 字节、所有 outcome/reason 往返、严格前缀及畸形边界、action/page 矛盾、
精确最大长度及 Server 关联字段变异测试验证此表示。它们不证明新鲜性、Host 执行、缓存
清理、耗时预算、重放策略或异步进展；这些义务仍属于 Close 模型及运行时细化。回滚共同
替换私有 cohort，不迁移持久数据。

Launch 拒绝 tag 9 和 Start 拒绝 tag 10 添加 `LifecycleActive`。精确原始缓存 Launch/Start
重放仍可读取；不同操作（包括更高 fence）不能替换 Close 拥有的绑定。该拒绝不是旧 fence
或 Host 失败观察。Server 将其映射到既有 terminal-runtime-unavailable 结果。生命周期
变更 Unknown 在跨尝试间保持；新 lease 或更高尝试代次都不授权再次变更。库存表示本身
不创建针对 Unknown 的 Server 对账或 Resume 路径。

## Terminal cwd validation 消息

OBCN kind 11 `TerminalCwdValidationRequest` 和 kind 12 `TerminalCwdValidationResult` 在既有
串行 Server-Worker carrier 上承载一次绑定 Worker 代次的只读目录观察。请求依次包含操作 ID
（16 字节）、Worker ID（16 字节）、正 Worker 代次（`u64`）、cwd 字节长度（`u16`）以及一个
必需的 `AbsoluteCwd`。结果回显完整请求，再添加一个 outcome tag：1
`DirectoryAvailable`、2 `DirectoryUnavailable` 或 3 `WorkerUnavailable`。所有整数继续使用
既有无符号大端编码。

已接受的 [Workspace terminal directory 契约](../../apps/orchbee-server/docs/workspace-terminal-directory.zh-CN.md)
拥有消费该观察的关联操作。

Codec 校验非零身份和代次、绝对 UTF-8 cwd 的长度与原始拼写、精确 payload 长度以及封闭的
outcome 集。它不访问文件系统、不解析符号链接、不创建目录、不授权关联，也不证明 Worker
仍为当前代次。Worker 拥有时间点目录元数据观察；Server carrier 校验完整回显，并拥有既有
有界队列、deadline 和连接丢失分类。该消息不含 terminal 身份、lease、Host 命令、launch
authority 或持久 effect。Protocol、Worker 和 Server 在同一未发布本机 cohort 中共同启用
kind 11 与 12。

## Framing 与 message

一条 canonical frame 包含固定 `OBCN` magic、unsigned big-endian frame version、unsigned
big-endian payload length，以及最多 8 KiB 的 payload。Frame codec 会拒绝 unknown magic、
version、message 或 enum tag、malformed 或 incomplete value、extra byte 与 oversized payload。
Frame version 1 与 terminal-launch capability version 2 是彼此独立的 version domain。
Bounded streaming helper 会先完整读取 8-byte header，在 validation 通过后才按声明长度分配
payload，并精确读取该 payload；write 会容忍 partial progress。I/O failure 与 syntactically
invalid protocol frame 明确区分。写入一条 frame 不会 flush caller-owned buffer；该 carrier
policy 仍由 adapter 决定。

Capability version 2 在不改变 frame envelope 的情况下增加 generation-local Worker binding bound
及其 typed capacity result。Worker 只 admit 与自身精确匹配的 supported capability version；因此
version 1 与 version 2 request 会互相以 `UnsupportedCapability` 拒绝。两个 version 对该 outcome
使用相同 rejection tag，而 version 1 decoder 会把新的 capacity tag 当作 unknown 拒绝。Mixed
version 由此 fail closed，而不会静默使用 version 1 的 unbounded binding behavior。当前 Worker
authorization kernel 固定为 version 2，而不是接受带 version 2 capacity behavior 的可配置 version。
当 echoed authorization 不是 version 2 时，construction 与 decoding 也会拒绝
`CapacityExceeded`。

| Message                    | 方向                      | 闭合 semantic payload                                                                                                                                     |
| -------------------------- | ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AuthorizedTerminalLaunch` | Server -> Capability Node | 完整 operation authorization、可选的有界 node-local current directory，以及带完整 pixel geometry 的 `Immediate` 或 `Deferred`。                           |
| `TerminalLaunchResult`     | Capability Node -> Server | 确切 authorization，加 `HostAccepted`、`Rejected`、`Failed` 或 `OutcomeUnknown`；每个 post-handoff outcome 都保留 observed Host epoch。                   |
| `AuthorizedTerminalStart`  | Server -> Capability Node | 单独 fenced 且 leased 的 authorization、完整 target Host terminal identity，以及 previously deferred terminal 的完整 pixel geometry。                     |
| `TerminalStartResult`      | Capability Node -> Server | 确切 authorization 与 target Host identity，加 `HostAccepted`、`Rejected`、`Failed` 或 `OutcomeUnknown`；success 显式区分 `Started` 与 `AlreadyStarted`。 |

每条 authorization 都绑定 product terminal ID、operation ID、protocol-owned Worker ID、nonzero
Worker generation、nonzero per-terminal monotonic fence、stateful lease ID 与 nonzero absolute
Unix-millisecond expiry，以及显式 capability version。所有 protocol-owned ID 都是 nonzero opaque
128-bit value。Result 回显完整 authorization，让 Server 可以丢弃 stale generation、fence、
operation、lease 或 capability observation。

Operation identity 还有一项无法通过 decode 单条 frame 建立的 producer invariant：经过认证的权威
Server 在其 active authority 内跨所有 Terminal ID、Worker 与 Worker generation 为每个
`OperationId` 分配全局 fresh identity，且绝不会把一个 identity 分配给两份不同 authorization。后继
Server authority 只有在其 authority transition 通过 generation、lease 与 fence validation 使所有旧
authorization 不再 current 后，才能开始新的 producer scope。本 contract 不规定该 transition，也不
要求 Capability Node 保留无界的跨 terminal tombstone ledger；Worker refinement 以经过认证的
authority 正确签发为前提，并不声称能够遏制违反 invariant 的 issuer。

可选 current directory 是 1 至 4,096 encoded byte 的 absolute POSIX UTF-8 path，且不能包含 NUL。
缺省与空值不同。Geometry 包含 column、row、cell-pixel width 与 cell-pixel height。column 与 row
均在 `1..=4096` 内；cell-pixel width 与 height 均在 `0..=4096` 内，且必须同正（pixel 已知）或
同零（pixel 未知），不得混合。grid 最多 262,144 个 cell，且每条 total pixel extent 都可放入
`u16`。这些 type
是 Capability Node refinement，不 import 或 re-export Terminal Host wire payload；未来 Worker
adapter 必须显式转换并保留 validation。

## 只读查看附件消息

Worker 现在消费 OBCN kind 5 `TerminalViewerRequest` 并发出 kind 6 `TerminalViewerResult`。
Server 现在为已准入 Client Session 提交这些消息，与 Launch/Start 共用载体。
这些新增消息仍然私有且未发布。旧 decoder 拒绝新 tag 或 magic；不提供 fallback 或混合 cohort
兼容性承诺。

每个 request 包含新鲜 request `OperationId`、精确 Worker ID 与非零 generation，以及 Server
签发的 attachment `OperationId`。Server 在活动 authority 范围内分配全新的 attachment identity，
分离后也绝不复用。它们命名独立的临时查看附件，不是 product Terminal identity 或 Host creation
identity。只有已认证 Server 授予读取权限；Worker 在附着前检查已接纳的 Worker/generation 与
精确 Host epoch。此路径不授予 input 或 resize，也绝不申请 Host resize lease。

闭合 action 为携带 Host epoch 和 Host terminal ID 的 `Attach`、`Poll`、携带正数 sequence/
generation 与 revision 的 `FrameApplied`、携带正数 sequence 与现有八种 snapshot reason 的
`NeedSnapshot`，以及 `Detach`。Kind 5 按顺序包含 request ID（16 字节）、Worker ID（16）、
generation（8）、attachment ID（16）和 action tag（1）。Tag 1 到 5 按上述顺序选择 action。
Attach 追加两个 16 字节 identity；FrameApplied 追加三个 8 字节整数；NeedSnapshot 追加一个
8 字节 sequence 和 1 到 8 reason tag，依次为 `ActivityStale`、`BaseMismatch`、`DecoderRejected`、
`DimensionsMismatch`、`GenerationMismatch`、`IdentityMismatch`、`SequenceGap` 与 `StoreReset`。所有整数均为
unsigned big-endian。Kind 6 包含 request 与 attachment ID，随后是 `Attached`、`Empty`、
`FeedbackAccepted`、`Detached` 或 `Rejected`，依次使用 tag 1 到 5。Rejection tag 1 到 8 分别
表示错误 Worker、过期 Worker generation、过期 Host epoch、超出容量、附件不可用、附件已存在、
反馈不匹配和 Host 拒绝。Result 始终回显 request 与 attachment ID。轮询绝不等待未来 Host 输出。

Record result 使用独立 `OBNR` envelope：四字节 magic、big-endian version 1（`u16`）、零值
reserved field（`u16`）、big-endian payload length（`u32`），随后是 request ID（16）、attachment
ID（16）和一条未经修改的 OBTP record。Record 长度为 64 到 `64 + 36 MiB` 字节，与 Host envelope
加最大 OBTF payload 一致。Codec 在分配 payload 前校验完整的 12 字节 header 和边界；消费者
校验不透明 record 的 Host envelope、route、role、identity 和 negotiated size。这不扩大 8 KiB
OBCN control 边界，也不解码帧内容。严格的内存 decode 拒绝尾随字节；streaming decode 精确消费
一条 record，并保留下一 frame 边界。Codec 不隐含 acknowledgement、retry 或 queue policy。
查看附件 carrier 失败后退役临时 state；调用方绝不自动重放结果不明确的附着或反馈 exchange。

## Controller 消息

OBCN kind 7 `TerminalControlRequest` 和 kind 8 `TerminalControlResult` 扩展同一串行 Server-Worker
stream。实际 producer 是 Server；Worker 将获授权 controller 映射为独立、不可读取的 Host 附件。
Server 分配新的 request 和 controller 身份；viewer 与 control 身份不共用，所有请求固定已准入
Worker 代次。

Request 包含 request ID（16 byte）、Worker ID（16）、Worker generation（u64）、viewer ID（16）、
controller ID（16），然后是 action：1 Acquire、2 Input、3 Resize 或 4 Release。Input 增加一个
正 u64 sequence、u16 length 和 1 至 4,096 个原始 byte。Resize 增加一个正 u64 sequence 和四个
u16 geometry 字段，沿用 columns/rows/cell-width/cell-height 顺序。其他 action 没有附加字段。
整数使用 big-endian 编码，所有 ID 非零。Input 与 resize 共用每 grant 严格递增 sequence；
即使 Host 明确拒绝，admission 也会消费该 sequence。

`Paste` request 使用专用 `OBNQ` record 而非 kind-7 frame：4-byte magic、big-endian version 1
（`u16`）、零 reserved 字段（`u16`）、big-endian body 长度（`u32`），随后是 request ID（16）、
Worker ID（16）、Worker generation（u64）、viewer ID（16）、controller ID（16）、正 u64
sequence（8）以及 1 至 262,144 byte 的不透明内容。完整 `OBNQ` record 为 92 至 262,236 byte。
Codec 在分配 payload 前验证完整 12-byte header 与界限，恰好消费一条 record，并保留后续
frame 边界。

Result 包含 request、viewer、controller ID（各 16 byte），然后是 outcome：1 Acquired、2 Released、
3 InputAccepted、4 Resized 并带相同完整 geometry，5 Rejected 并带 reason byte：1 WrongWorker、
2 StaleWorkerGeneration、3 ViewerUnavailable、4 ControllerBusy、5 NotController、6 SequenceMismatch、
7 HostRejected、8 HostInputInvalid、9 HostInputLimitExceeded、10 HostInputBackpressure、
11 HostInputUnavailable——或 6 Pasted 并带 Host 报告的正 u32 `accepted_bytes` 计数。
必须精确匹配 ID 与 action/outcome；Resize 必须回显 geometry，且 Server carrier 还会拒绝
`accepted_bytes` 超过 `min(2 × content + 12, negotiated input limit)` 的 `Pasted` 结果。
Release 是内部
cleanup；对应 viewer 没有 controller 时幂等，存在不同 current grant 时拒绝。Host exchange
含糊时关闭 Worker carrier，不伪造接受；Server 将完整交付后无结果归类为 unknown。

8 KiB OBCN frame 限制、精确长度解码和方向规则保持不变；`OBNQ` 与 `OBNR` 一样是 frame 流旁
的原始 record。Codec test 覆盖全部 action/outcome/reason、
原始字节保留、最大 input 和非法 scope bound。Server 与 Worker 指南拥有单 controller 预留、
cleanup 和真实 PTY 证据。

## Authorization outcome

Pre-admission rejection 区分 wrong Worker、stale generation、expired lease、stale fence、
conflicting same-fence replay、unsupported capability 与 retained-binding capacity。只有在一个
Worker generation 已保留 1,024 个 binding 后，才能对此前未见过的 `TerminalId` 返回
`CapacityExceeded`；它不插入 tombstone，也不改变 binding。Host-side rejection 与 failure 会保留
observed Host epoch。`HostUnavailableBeforeHandoff` 是确定未产生 effect 的 failure；
`OutcomeUnknown` 是 dispatched request 没有可观测 result 时的保守结果，因为 Server 不能从
carrier write completion 推断 Worker 内部 handoff stage。Launch success 返回完整 Host
creation、epoch 与 terminal identity，加 awaiting-start 或 started geometry。Start result 始终
保留完整 target Host identity；accepted result 还会返回 observed identity、geometry 与
started/already-started transition。

Launch 与 Start 使用不同的 rejection 和 failure type：Launch 没有 target Host terminal，因此
不能 encode `HostIdentityChanged`。Start 的 `HostAccepted` observation 必须与 frame 中的 target
identity 完全相同。其 `HostRejected` 与 `HostReportedFailure` epoch 必须等于 target epoch；不同
的 observed epoch 只能表示为 `HostIdentityChanged`，且该 epoch 必须确实不同。经过 validation
的 result construction 与 decoding 会拒绝这些 identity 与 epoch contradiction，因此无法通过这些
constructor 发出矛盾的 canonical encoding。Worker refinement 还要求 successful Start observation
在构造 result 前保留确切 authorized geometry。

Launch payload 没有 general environment overlay、executable、argument vector、Project、
Checkout、Worktree、Agent 或 Launch Profile field。Unknown trailing byte 会被拒绝，因此本
contract 根本无法表示 nonempty environment。

## Carrier 与 ownership 边界

本 crate 只拥有闭合 semantic value 与 canonical `OBCN` frame codec。它不 open、authenticate、
admit、retain、retry、reconnect、multiplex connection，也不向 connection 应用 backpressure。
其 streaming helper 仅为一条 frame 借用 caller-owned `Read` 或 `Write`，不会保留 socket、
buffer、connection 或 phase state。
Unix-domain socket、same-user peer credential、executable cohort、`OBLS` admission handshake、
remote transport、encryption 与 connection state machine 属于 carrier adapter 及其独立
protocol。当前本机 Server adapter 在最多一条 wire request 后拥有一个 capacity-one submitted
slot，序列化完整 request/result exchange，校验确切 correlation 与 echoed authorization，并区分
完整写出 request 前后的 failure。它不映射 OBCS、不合成 semantic outcome、不 retry，也不
reconcile。Server 拥有本机终端授权与 reconciliation；Capability Node adapter 拥有
到其 Worker admission identity 与 Terminal Host protocol 的显式 mapping。本 crate 不依赖这两个
protocol。

对当前 adapter 而言，attached generation 会保持 registered，直到唯一 wire owner 已结算或保守放弃
current exchange，并 drain submitted slot。之后 lifetime guard 才移除该确切 generation，因此
replacement 不会与旧 write 或 exact-result publication 重叠。Normal return、completion receiver
消失、thread spawn failure 与 panic unwind 都遵循相同顺序。完整写出后的 abandonment 仍是 typed
carrier-level post-delivery unknown；adapter 不会制造 OBCN result。

## 已检查的 authorization model

已注册的
[`TerminalLaunchAuthorization`](../../apps/orchbee-server/formal/tla/TerminalLaunchAuthorization.tla)
model 是这个实验性 contract 的 Server-owned behavioral authority。它探索 current 与 stale
Worker generation、lease 在 admission 前、admission 与 Host handoff 之间以及 handoff 后过期、
lower/exact/conflicting fence、确定的 pre-handoff failure、post-handoff unknown outcome、Host
epoch replacement（包括已 admitted Start 的 target epoch 在 handoff 前变化）、deferred launch 后单独
authorized start、stale result suppression、每一种 connection loss 与 retained-terminal lifetime。
该 raced identity change 会作为确切 `HostIdentityChanged` result 被缓存，且没有 Host effect。Model
的 invariant 要求每个 operation 最多产生一个 Host effect，并防止旧 generation、fence、operation
或 Host epoch result 覆盖 current authority。因为本 protocol 没有 progress acknowledgement，
dispatch 后 Worker link 丢失会在每个尚未观测的 Worker stage 产生保守的 unknown observation；只有
已返回的 Worker result 才能报告确定的 pre-handoff failure。

Model 允许一个 outstanding request 与一个 pending-result slot。Worker 在 admission 前、
admission 后和 Host handoff 后断连会分别接受检查，并始终用 unknown result 占用该 empty slot。
Replacement Worker generation 只有在该 pending loss observation 已 settle 后才会 activate，
因此 reconnect 不会让 current unknown result 看起来 stale。Start acceptance 会分别探索
`Started` 与 `AlreadyStarted`。一旦 admitted operation 获得 terminal result，model 就会缓存其
确切 outcome、Host epoch 与 transition。Exact replay 不产生新的 Host effect，而是返回该
cache——lease 过期后也一样——因此 rejection、failure 或 unknown result 永远不会被制造成
success。

唯一 fairness condition 会在 Server 保持 connected 时 settle pending result。它不要求任何健康
connection 发生 failure，也不承诺 automatic reconnection、authorization 或 terminal recovery。
Model 的 bound 是两个 operation ID、两个不同的 lease identity，以及两个 Worker generation、
fence 与 Host epoch。其 deterministic fence-to-operation-and-lease mapping 是 state-space symmetry
reduction，而不是 runtime derivation rule；一个已检查的 no-revival property 会防止 expired
accepted lease 在没有替换对应 binding 时再次变为 valid。
`FreshOperation` 是 global Server producer invariant 在单个 retained terminal 上的 projection；由于
current authority 与 Host effect 会被保留，该 invariant 跨 modeled Worker reconnect 仍然生效。
Model 不枚举 cross-terminal、cross-Worker namespace 或 successor Server-authority transition。未来
runtime adapter 需要单独的 refinement evidence。

已注册的 `TerminalLaunchAuthorization` model 使用 capability version 2 作为 supported authority，
并使用 version 1 作为 unsupported-capability witness。

Worker-owned 且已注册的
[`WorkerTerminalBindingCapacity`](../../apps/orchbee-worker/formal/tla/WorkerTerminalBindingCapacity.tla)
refinement 将 cross-terminal bound 与 Worker 的 abstract global operation/result slot 组合起来，
single-terminal Server authority model 则继续保留详细 handoff phase。它的 TLC configuration 使用
capacity two 与三个 terminal identity，以 exhaustive 覆盖 below-limit admission、full threshold、
拒绝一个 new identity、occupied-slot priority、result delivery clearing、仅在 slot empty 后才允许
capacity 时的 exact cached replay 与 higher-fence replacement、rejection preservation 与
classification priority。Production constant 是每个 Worker generation 精确 1,024 个 retained
binding。没有 release、TTL、LRU 或 eviction path；production 之前仍需要 explicit retirement。
该 bounded model 不是针对 1,024 个 concrete entry 或 Rust implementation 的 proof，且其 authority
只会与对应 Rust enforcement 作为同一个 unshipped delivery cohort 一起 ship。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee-capability-node-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-capability-node-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models terminal-launch-authorization,worker-terminal-binding-capacity
pnpm test:tla:inventory
pnpm lint:documentation
pnpm lint:projects
```
