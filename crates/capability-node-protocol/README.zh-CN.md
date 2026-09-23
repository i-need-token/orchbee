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

kind 9 `TerminalLifecycleRequest` 和 kind 10 `TerminalLifecycleResult` 使用帧版本 2 及
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

| Message                    | 方向                      | 闭合 semantic payload                                                                                                                                            |
| -------------------------- | ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AuthorizedTerminalLaunch` | Server -> Capability Node | 完整 operation authorization、可选的有界 node-local current directory、带完整 pixel geometry 的 `Immediate` 或 `Deferred`，以及可选的有界显式 launch selection。 |
| `TerminalLaunchResult`     | Capability Node -> Server | 确切 authorization，加 `HostAccepted`、`Rejected`、`Failed` 或 `OutcomeUnknown`；每个 post-handoff outcome 都保留 observed Host epoch。                          |
| `AuthorizedTerminalStart`  | Server -> Capability Node | 单独 fenced 且 leased 的 authorization、完整 target Host terminal identity，以及 previously deferred terminal 的完整 pixel geometry。                            |
| `TerminalStartResult`      | Capability Node -> Server | 确切 authorization 与 target Host identity，加 `HostAccepted`、`Rejected`、`Failed` 或 `OutcomeUnknown`；success 显式区分 `Started` 与 `AlreadyStarted`。        |

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

可选 launch selection 是 launch mode 之后的闭合尾块：`u16` executable 长度、executable 字节、
`u16` argument 计数，然后是若干 `u16` 长度前缀的 argument 字符串。Executable 是不含 NUL 的
非空 absolute UTF-8 path；argument 是不含 NUL 的 UTF-8 字符串（允许空值与 dash 前导值），最多
128 项，且 executable 加全部 argument 合计最多 2,048 UTF-8 byte。在 mode block 处结束的
payload 解码为默认 launch；存在的尾块按完整消费校验，部分字节或多余尾随字节一律拒绝。尾块
只承载 `argv[1..]`——`argv[0]` 由 Host 从 executable path 合成——且缺省与显式空 argument
列表不同。

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
`NeedSnapshot`、`Detach`，以及 `ReadRows`。Kind 5 按顺序包含 request ID（16 字节）、Worker ID（16）、
generation（8）、attachment ID（16）和 action tag（1）。Tag 1 到 6 按上述顺序选择 action。
Attach 追加两个 16 字节 identity；FrameApplied 追加三个 8 字节整数；NeedSnapshot 追加一个
8 字节 sequence 和 1 到 8 reason tag，依次为 `ActivityStale`、`BaseMismatch`、`DecoderRejected`、
`DimensionsMismatch`、`GenerationMismatch`、`IdentityMismatch`、`SequenceGap` 与 `StoreReset`。
`ReadRows` 追加 big-endian `startRow`（u64）、`rowCount`（u16，1 到 4096）、`revision`（u64）、
正数 `generation`（u64）、正数 `requestSequence`（u64）与 `maximumBytes`（u32，112 到
8,388,608——是 OBRW payload 边界，不是 record 边界）；其 attachment 留在 request envelope
上，不重复携带。Tag 1 到 5 逐字节不变。所有整数均为
unsigned big-endian。Kind 6 包含 request 与 attachment ID，随后是 `Attached`、`Empty`、
`FeedbackAccepted`、`Detached`、`Rejected` 或 `RowsRejected`，依次使用 tag 1 到 6。Rejection
tag 1 到 8 分别
表示错误 Worker、过期 Worker generation、过期 Host epoch、超出容量、附件不可用、附件已存在、
反馈不匹配和 Host 拒绝。`RowsRejected` 追加一个 1 到 9 的 reason tag：`StaleRevision`、
`StaleGeneration`、`HistoryEvicted`、`MissingResource`、`InvalidField`、`InvalidAcknowledgement`、
`LimitExceeded`、`ResourceExhausted` 与 `InvalidPhase`；它只应答 `ReadRows`，而 `ReadRows`
请求只接受 `Record`、`RowsRejected` 或既有明确的不可用/拒绝 outcome。其它 action 上出现的
`RowsRejected`，或应答不能产出 record 的 action 的 `Record`，均为协议违例。
Result 始终回显 request 与 attachment ID。轮询绝不等待未来 Host 输出。

Record result 使用独立 `OBNR` envelope：四字节 magic、big-endian version 1（`u16`）、零值
reserved field（`u16`）、big-endian payload length（`u32`），随后是 request ID（16）、attachment
ID（16）和一条未经修改的 OBTP record。对 `Poll` 该 record 是 Host event；对 `ReadRows` 则是
关联的 Host response。Record 长度为 64 到 `64 + 36 MiB` 字节，与 Host envelope
加最大 OBTF payload 一致。Codec 在分配 payload 前校验完整的 12 字节 header 和边界；消费者
校验不透明 record 的 Host envelope、route、role、identity 和 negotiated size。这不扩大 8 KiB
OBCN control 边界，也不解码帧内容。严格的内存 decode 拒绝尾随字节；streaming decode 精确消费
一条 record，并保留下一 frame 边界。Codec 不隐含 acknowledgement、retry 或 queue policy。
查看附件 carrier 失败后退役临时 state；调用方绝不自动重放结果不明确的附着或反馈 exchange。

五个追加 action tag 7 到 11 服务历史视口，仍然私有且未发布；tag 1 到 6 逐字节不变。
所有 action 的 attachment 仅由 request envelope 携带。所有整数为 big-endian；`i32` 是唯一的
有符号字段。未知 tag、未知 origin 或 demand 值、零值正整数、越界边界与尾随字节一律拒绝。

- Tag 7 `set-projection-demand` 追加 `demand`（`u8`）：1 active、2 paused。
- Tag 8 `set-history-demand` 追加 `demand`（`u8`）：1 following-bottom、2 viewing-history。
- Tag 9 `read-viewport-rows` 追加 `origin`（`u8`：1 tail、2 committed），tail 后随
  `rowsBefore`（`u32`）、committed 后随 `delta`（`i32`），然后依次是 `rowCount`（`u16`，
  1 到 4096）、`revision`（`u64`）、正数 `generation`（`u64`）、正数 `requestSequence`
  （`u64`）与 `maximumBytes`（`u32`，112 到 8,388,608——是 OBRW payload 边界，不是 record
  边界）。Purpose、detail 与 revision policy 由调用方 adapter 固定为
  viewport/text/current-view，不由本 payload 表达。
- Tag 10 `rows-applied` 追加正数 `requestSequence`（`u64`）、正数 `generation`（`u64`）与
  `revision`（`u64`）——与 `FrameApplied` 相同的字段顺序。
- Tag 11 `history-observed` 追加正数 `sequence`（`u64`）、正数 `generation`（`u64`）与
  `revision`（`u64`）。

四个控制应答 action 以 `FeedbackAccepted` 应答，或当 Host 返回 typed failure 时以携带既有
Host-rejected reason tag 8 的 `Rejected` 应答。`read-viewport-rows` 以承载相关 kind-46
行窗口 response 的 `Record` 应答，或以既有九值 reason 词表的 `RowsRejected` 应答。其它
outcome 配对仍为协议违例。

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

## Send 消息

OBCN kind 13 `TerminalSendRequest` 与 kind 14 `TerminalSendResult` 承载一次经 Server 授权的
非交互 input 提交。Server 分配新 request 身份与 input-only attachment 身份；Worker 绝不自行
铸造二者、绝不查询其 viewer 或 controller 映射、绝不按逻辑 terminal ID 重解析目标——request
已钉住精确的授权化身。

Request 含 request ID（16 byte）、Worker ID（16）、Worker generation（u64）、精确的
Host 终端身份（48：creation ID + Host epoch + Host terminal ID），随后 Server 分配的
attachment ID（16），随后 payload tag：1 text 或 2 key。
Text 附加 u16 长度与 1 至 4,096 UTF-8 byte 及 `enter` 标志 byte（0 或 1）；回车声明在
同一 payload 内，因此一个 request 永远编码恰好一条 Host input record，且 `text` + `enter`
合计必须落在 4,096-byte 界内。Key 附加闭集 u8 码：1 Enter、2 Escape、3 Tab、4 Backspace、
5 Delete、6 Up、7 Down、8 Left、9 Right、10 Home、11 End、12 PageUp、13 PageDown、14 CtrlC、
15 CtrlD。所有整数为 big-endian，所有 ID 非零。generation、epoch 与 terminal 身份是 fence：
Worker 拒绝钉在不同 generation 或 epoch 的 request 而非重绑，本契约不存在 restart-rebind
行为。

Result 含 request ID（16），随后 input 轴——1 Unavailable 带 reason byte（1 HostUnavailable、
2 HostProtocolFault），2 Rejected 带 reason byte（1 WrongWorker、2 StaleWorkerGeneration、
3 StaleHostEpoch、4 TerminalUnavailable、5 HostRejected、6 HostInputInvalid、
7 HostInputLimitExceeded、8 HostInputBackpressure、9 HostInputUnavailable），3 Accepted 带
u32 `accepted_bytes`——Host 编码进 input 队列的字节数（模式忽略某语义键时零合法；绝非
request 字节长或键名长），或 4 Unknown——随后正交 cleanup 轴 byte：1 NotNeeded、
2 Confirmed、3 Unconfirmed。两种配对不可能出自 Attach → input → Detach 序列，构造期与
两侧 codec 边界都拒绝：Accepted + NotNeeded（已接受的 input 意味着真实附件）与
Unknown + Confirmed（confirmed 释放意味着已完成的 exchange）。Unknown
与 Unconfirmed 表示 input 可能已落地或 attachment 可能残留；二者都绝不自动重试。明确的
Host 拒绝仍释放临时 attachment，而失同步的 exchange 以关闭 Worker-Host carrier 应答而非
继续请求——降级的 cleanup 状态如实记录而非冒称已释放。

8 KiB OBCN frame 限制、精确长度解码、方向规则与非零身份规则不变。本消息不授予 controller、
viewer、resize 或 lease 权限，也不占两个映射中的任何槽位；授权 attachment 仅存在于一个
顺序 send 内。Codec test 覆盖每个 payload/outcome/reason 组合、严格 golden、未知 tag、
trailing byte 与边界长度；Server 与 Worker 指南拥有准入与真实 Host 证据。

## Worktree job 消息

OBCN kind 15 `WorktreeJobRequest` 与 kind 16 `WorktreeJobResult` 为 `worktree list`
Server 流程承载一个有界 `git worktree list` 发现 job。请求依次为 operation ID（16 字节）、
Worker ID（16）、Worker generation（u64），然后是 action tag：1 `Start` 带 u16 长度与
1 至 4,096 字节目录（绝对、非空、无 NUL 的 POSIX 路径——构造与解码双侧校验），
2 `Poll` 带 job ID（16），或 3 `Cancel` 带 job ID（16）。job ID 即 Start 操作自身的
operation ID：Server 为每个 job 铸造一个身份并在 Poll/Cancel 上复用。

结果回显 operation ID（16）、action tag 与 job ID（16）——action 回显使每个 outcome
码不歧义——然后是一个按 action 划分的 outcome 字节。Start outcome：1 Accepted、
2 Busy、3 InvalidDirectory、4 SpawnThreadFailed（属主线程，绝非子进程）、5 Dead
（永久 tombstone 槽位）、12 RejectedFencing。Poll outcome：1 Pending；2 Completed 带
三个 u16 长度前缀字节字段——probe `common_dir`、`listing` 与 `stderr`——其长度之和
在构造与解码时都必须满足 7,936 字节字段预算；3 GitFailed 带 stage（1 probe / 2 list）、
status 存在字节、可选 i32 退出码及同一预算下的一个长度前缀 `stderr` 字段；
4 SpawnUnavailable{stage}；5 SpawnFailed{stage}；6 TooLarge 带可选 stage（wire 序列化
预算自身溢出时为 0）；7 Timeout；8 Cancelled；9 OwnerIo 带可选 stage；10 Unknown；
11 PhysicalUnknown；或 12 RejectedFencing。Cancel outcome 仅摘要：1 Registered 或
2 AlreadyTerminal——终态在有界 receipt 过期前仍可由 Poll 重读——另有 10 Unknown、
11 PhysicalUnknown 与 12 RejectedFencing。`RejectedFencing` 结果是类型化帧而非断连：
钉在另一 Worker 或 generation 上的请求被拒绝且不退役 carrier。

8 KiB OBCN 帧上限不变——7,936 是保守 policy 界，使每个 arm 都低于载荷上限并留有
明示余量。Codec test 覆盖每个 action 与 outcome arm、严格 golden、字段预算边界、
未知 tag、stage 码与 trailing byte；Server 与 Worker 指南拥有准入、fencing 与 job 证据。

## Agent drain 消息

OBCN kind 17 `AgentDrainRequest` 与 kind 18 `AgentDrainReply` 承载 Server 发起的、对 Worker
留存 terminal application signal 的 drain。kind 之后的 request body 依次为：protocol 所有的
Worker ID（`16` 字节）、非零 Worker generation（`u64`）、非零 request token（`u64`）、
已确认 sequence 水位（`u64`）——41 payload 字节，含 frame 共 49 字节。Worker 原样回显
token，从不自行铸造或重铸。超过 Worker offered 水位的 acknowledgement 是致命协议错误；
stale 或零 acknowledgement 是合法 no-op，绝不移除任何未确认 sequence。

Reply 携带回显的 token 与 acknowledgement、queue tail 水位、有界 entry 数量，以及一段连续
且不重叠的 `Event`/`Gap` entry 页。每个 event entry 是完整的 CNP 侧 `HostTerminalIdentity`
加至多 4,096 字节的 UTF-8 payload；每个 gap entry 覆盖一段已从留存窗口逐出的闭 sequence
区间。Decoder 在任何分配之前拒绝超过 480 个 entry、未知 tag、非正 sequence 或 gap 界、
反向 gap 区间、超大 payload、不连续或重叠的页、超出已声明 queue tail 的 entry、trailing
byte 与整数溢出。首条 entry 不要求紧跟回显的 acknowledgement，因为 stale
acknowledgement 是合法的。

这些类型是 Capability Node refinement：event identity 复用本 crate 自己的
`HostTerminalIdentity`，而非 Terminal Host wire identity。Wire kind 15 与 16 仍为 Worktree
功能保留，此处不占用。Server 侧 drain 生产、registry 与 held-listener 行为不在本 crate
范围内。

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

Launch payload 除可选 launch selection 外，还接受有界、逐字的 `LaunchEnvironment`。
环境为空时沿用旧字节编码；非空时，selection 尾部替换为 `u16(0)`、扩展版本 `u8(1)`、
selection-present `u8(0|1)`、可选的既有 selection，再跟 `u16` 条目数及由 `u16` 长度前缀
标记的 UTF-8 名称和值。零是无效的旧 executable 长度，因此旧 decoder 拒绝该扩展。
未知版本、字段截断、空扩展及多余尾部字节均被拒绝。

环境至多 32 项，名称和值合计不超过 1,024 UTF-8 字节。名称匹配
`[A-Za-z_][A-Za-z0-9_]*`，至多 128 字节，忽略 ASCII 大小写后必须唯一。拒绝含 NUL 的值、
`ORCHBEE_*`、`TERM`、`COLORTERM` 和 `TERM_PROGRAM`；空值逐字保留。
准入前完成校验；Debug 不输出值。cwd、selection、参数数量与环境均取最大值时仍满足
8 KiB 帧上限。不含 Project、Checkout、Worktree、Agent 或 Launch Profile 字段。
Cohort 校验要求部署匹配的二进制；本扩展不承诺混合版本运行。

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

## 托管 Worker 停止

Windows 托管 Site 生命周期在帧版本 2 和既有 8 KiB 上限下增加 kind 19/20。
请求依次包含操作 ID、Worker ID（各 16 字节，整体非零）、正 Worker generation（u64）
和协作式预算（u32，1..15000 毫秒），整数均为大端。仅精确准入的托管 Worker 可执行；
非托管或身份不匹配在效果前拒绝。既有 20 秒 carrier 回应窗口不变，响应回显完整请求。

结果 tag 为 1 NoHost、2 Stopped（Host epoch、u32 已释放计数）、3 Refused
（1 Worker 不符、2 非托管）、4 Incomplete（可选 Host epoch、u32 已释放计数、阶段及
失败原因）。NoHost 表示从未启动自有 Host 且后台任务已结束；Stopped 要求收敛／释放后
确认自有 Host 退出。二者都不宣称 Worker 已退出，Server 须等待创建时保留的进程句柄。
计数只描述本次尝试。阶段 1..5 为后台任务、库存、强制终止、释放及 Host 退出；失败
1..5 为期限、传输、协议无效、Host 拒绝及进程等待。可选 epoch 使用 tag 0/1，存在时
后接非零 ID。非法 tag、零值、预算越界和尾随字节均拒绝。编解码有效不产生终端权限，
未知提交后不自动重放。正常断线不代表停止。此类型化交换向 Server 生命周期提供证据，
绝不发布本地激活完成记录。

## 原生 Windows 路径细化

线协议中的绝对路径接受 POSIX 前导斜杠，或 ASCII 盘符后跟 `:` 及 `/` 或 `\`；Windows 逐字盘符路径可以带 `\\?\` 前缀。词法规则不依赖解码器所在平台；字节保持原样，不展开、不规范化。本次 Windows 增量不接受反斜杠根路径、盘符相对路径、UNC 或设备路径。既有字节上限和 NUL／Unicode 校验不变。执行方仍须在产生效果前依据本地文件系统验证路径。

## 终端进程读取（未发布）

Viewer action tag 12 为 `ReadProcessInfo`，tag 后没有额外字节，使用请求中既有的 attachment ID。Worker 要求确切的保留 viewer，并以新的关联身份发送一次 Host `ReadProcessInfo`（OBTP kind 79）。响应限制为 64 KiB 加信封，以 `Record` 原样返回，其中也允许格式正确且相关联的 Host `TypedFailure`。成功观察必须回显确切的 attachment 与物理终端。Worker/generation 不匹配、viewer 缺失、Host epoch 错误或结果无效均通过既有拒绝或 carrier 失败返回。Worker 不枚举进程，Host 仍为事实所有者。进程读取既不确认帧，也不创建输入权限。现有 action 编码保持不变，开发版本必须整体更新。
