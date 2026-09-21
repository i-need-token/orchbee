# Client-Server Protocol Rust Binding

## 范围

`orchbee-client-server-protocol` 是 [`schemas/client-server`](../../schemas/client-server/README.zh-CN.md)
下语言中立 OBCS version-1 contract 的独立 Rust binding。它负责 validated Rust value、严格且区分
方向的 JSON decoding、deterministic encoding 与 bounded frame I/O。它不依赖 OBLC local admission
binding、OBCN Server-Worker authorization binding、Terminal Host type 或 Workbench type。

本 crate 不连接 transport、不 authenticate Client、不 authorize request、不选择 Worker、不存储
request identity、不执行 terminal effect、不 cancel work、不 reconcile unknown outcome，也不 project
Workbench state。

## Public API

| API                                                                                                                                                 | Contract                                                                                                                                                   |
| --------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `RequestId`、`SiteId`、`WorkerId`、`TerminalId`、`ViewerId`、`HostEpoch`、`ControllerId`、`PresentationId`、`PanelId`、`TabId`、`TerminalContextId` | 解析独立 typed、nonzero opaque 128-bit 小写十六进制 identity。                                                                                             |
| `WorkingDirectory`                                                                                                                                  | 校验最多 4,096 个 encoded byte、无 NUL 的 absolute POSIX UTF-8 path。                                                                                      |
| `Geometry`                                                                                                                                          | 仅当所有 scalar 与 product bound 都成立时构造 complete geometry。cell-pixel width 与 height 必须同正（pixel 已知）或同零（pixel 未知）；混合成对值被拒绝。 |
| `TerminalCreateRequest`、`TerminalStartRequest`                                                                                                     | 构造 validated Client request；start 拒绝相同的 start/create request identity。                                                                            |
| `CreateOutcome`、`StartOutcome` 与 result type                                                                                                      | 只表示 OBCS v1 准入的稳定 Client-visible result combination。                                                                                              |
| `encode_client_frame`、`decode_client_frame`                                                                                                        | Encode 或 decode 恰好一个完整 in-memory Client-to-Server frame。                                                                                           |
| `encode_server_frame`、`decode_server_frame`                                                                                                        | Encode 或 decode 恰好一个完整 in-memory Server-to-Client frame。                                                                                           |
| `read_client_message`、`read_server_message`                                                                                                        | 从支持 partial read 的有序 stream 中精确读取一个 frame，并保留后续 frame 未读。                                                                            |
| `write_client_message`、`write_server_message`                                                                                                      | 通过 partial write 写出完整 frame，且不 flush caller。                                                                                                     |

所有 decoded object 与 enum 都是 closed。缺失显式 nullable create field、未知 field 或 tag、重复
key、BOM、无效 UTF-8、尾随 JSON value、direction mismatch、无效 identity、path 或 geometry，以及
不符合精确长度或 32 KiB payload bound 的 JSON frame 都会被拒绝。Protocol 与 geometry number 使用
schema 所有的有限 binary64 safe-integer refinement，因此等价的 integral decimal 与 exponent
spelling 会 decode 为同一个 value。Canonical encoding 使用带普通 integer token 的 compact
deterministic JSON。

## 查看附件 binding

`TerminalViewerRequest`、`TerminalViewerAction`、`TerminalViewerResult` 和 `TerminalViewerOutcome`
提供 Session scope 内只读 Attach、AttachBinding、Poll、FrameApplied、NeedSnapshot 和 Detach value。
AttachBinding 携带独立 typed 的 OBCS `SiteId`、`WorkerId` 与 `TerminalId` value；这些 domain 之间
相同的 encoded byte 仍然有效。本 crate 校验其 lexical contract，但既不导入 Workbench type，也
不判断 tuple 当前是否已暴露。该 semantic resolution 由 Server 负责。
`ViewerUnavailableReason::WorkbenchUnavailable` 仅用于 viewer result。

Poll、FrameApplied、NeedSnapshot 和 Detach 必须携带 typed `attachment_id: ViewerId`；Attach 与
AttachBinding 不能携带该字段。`TerminalViewerAction::attachment_id()` 返回既有 viewer selector，
对两种附着 action 返回 `None`。Server 校验提交 Session 内的所有权；binding 只校验身份语法。

`ViewerId` 与 `HostEpoch` 表示 Server 签发的附件 scope。反馈计数器使用规范 u64 十进制字符串；
JSON corpus 测试每个 action、outcome、reason、closed field set 与 identity boundary。记录结果保留
schema owner 定义的 OBSR 二进制 framing 和 `MAXIMUM_VIEWER_RECORD_BYTES` bound；同一组 Server
frame function 可分派 JSON control 或 opaque 二进制记录。Client 方向 reader 拒绝 OBSR。
AttachBinding 不增加 OBCN 或 OBSR type。本 crate 不解码 OBTP 或终端呈现；Server 和后续
renderer 分别负责路由与呈现验证。大记录测试覆盖字节相等、partial read、后继帧对齐、方向，
以及非法 header、length、identity 的拒绝。

### 历史视口 action（未发布）

五个追加 action kind 为原生历史视口扩展 viewer 面：`set-projection-demand`、
`set-history-demand`、`read-viewport-rows`、`rows-applied` 与 `history-observed`。它们仍是
未发布契约，要求精确开发 cohort；不定义 fallback 形态。每个 action 在自身 variant 内携带
`attachmentId`，与既有 attachment 作用域 action 一致。

`set-projection-demand` 携带 `demand`（`"active"` 或 `"paused"`）；`set-history-demand` 携带
`demand`（`"following-bottom"` 或 `"viewing-history"`）。它们的 Host 应答只回显
attachment 与 demand 对，因此调用方按 correlation 加回显 demand 匹配——没有 generation
字段可比。

`read-viewport-rows` 携带 `origin` 对象（`{"kind":"tail","rowsBefore":u32}` 或
`{"kind":"committed","delta":i32}`）、`rowCount`（`1..=4096`）、`revision`、`generation`、
`requestSequence`（正 u64 十进制字符串）与 `maximumBytes`（`112..=8,388,608`，与 `read-rows`
相同的 OBRW payload 边界）。Host 按当前视图服务：返回的 revision 成为该窗口的捕获身份，
因此 binding 不要求响应与请求 revision 相等。`rowsBefore` 与 `delta` 是有界 JSON 整数
（`u32`/`i32` 域，均在 JavaScript 安全整数范围内）；`delta` 原生有符号，允许负滚动差值。

`rows-applied` 携带 `requestSequence`、`generation` 与 `revision`——Host 将 pending 视口锚
提升为 committed 所需的精确三元组。`history-observed` 携带 `sequence`、`generation` 与
`revision`——对一条已投递 history-changed 事件的精确应答三元组。两者 sequence 与
generation 使用正 u64 十进制字符串，revision 使用规范 u64 十进制字符串。

`ViewerRejectedReason::HostRejected`（`"host-rejected"`）是新增的闭合 viewer 级 reason，
只为四个控制应答 action 的明确 Host typed failure 保留。它与 controller 面的
`ControlRejectedReason::HostRejected` 不同，也不适用于旧 action。`read-viewport-rows` 的
失败沿用既有 `RowsRejected` 九值 reason 词表；`read-rows` 逐字节不变。

## Controller binding

`TerminalControlRequest`、`TerminalControlAction`、`TerminalControlResult` 和 `TerminalControlOutcome`
携带显式 Acquire 与 grant scope 内的 Input/Resize。`ControllerId` 是 Server 签发的身份；
三个 action 都必须携带 `attachment_id: ViewerId`，通过 `TerminalControlAction::attachment_id()`
读取。这是尚未发布的 OBCS v1 必需字段变更；Client/Server/Worker 必须使用精确匹配的开发 cohort，
不提供隐式 viewer fallback。
`ControlInput` 验证 1 至 `MAXIMUM_CONTROL_INPUT_BYTES`（4,096）byte，并序列化为规范小写 hex。
共用 number adapter 验证正十进制 u64 sequence。Acquire、Input 与 Resize 使用既有 32 KiB OBCS
JSON framing。

`Paste` 是语义自动模式粘贴 action。它携带 `PasteContent`——1 至
`MAXIMUM_PASTE_CONTENT_BYTES`（262,144）byte 的不透明正文——且从不出现在 OBCS JSON 中：
client frame codec 将其编码为专用 `OBCQ` record——4-byte magic、big-endian version 1（`u16`）、
零 reserved 字段（`u16`）、big-endian body 长度（`u32`），随后是 request ID（16）、attachment
ID（16）、controller ID（16）、sequence（8）与内容。完整 `OBCQ` record 为 68 至 262,212 byte。
codec 在分配 payload 前验证完整 12-byte header 与界限，恰好消费一条 record，并保留后续 frame
边界；server 方向 reader 分发 OBCS JSON 或 `OBCQ`，client 方向 reader 拒绝 `OBCQ`。
`Pasted` outcome 回显 Host 报告的正 `accepted_bytes` 计数。除共享拒绝原因外，paste 还映射
Host 类型化失败 `host-input-invalid`、`host-input-limit-exceeded`、`host-input-backpressure`
与 `host-input-unavailable`。

Schema owner 的 corpus 检查每个 action/outcome/reason
以及畸形、最大和超限 input。本 binding 不授予 controller 权限，绝不重试未知副作用。

## Send binding

`TerminalSendRequest`、`TerminalSendPayload`、`TerminalSendResult` 与 `TerminalSendOutcome`
承载一次经 Server 授权、面向已暴露 Workbench 终端的非交互 input 提交。request 是扁平的
`requestId` + `siteId`/`workerId`/`terminalId` 绑定三元组 + 闭合 payload 联合；三元组是
Server 在准入时解析的声明目标，不是化身锁。本 binding 不授予 controller、viewer、resize
或 lease 权限，不占 attachment 槽位，且绝不跨 restart 重绑：准入后 worker 或 Host 化身已
变化的目标只能失败或报告 unknown，绝不改向。

`TerminalSendPayload` 恰好为 `text` 或 `key` 之一。`text` 携带 `SendTextInput`——已验证
非空 UTF-8 `SendText` 与 `enter` 标志的配对；回车折进同一 payload，因此一次 send 永远
恰好是一条 Host input record。`SendText` 最多接受 `MAXIMUM_SEND_TEXT_BYTES`（4,096）
byte，`SendTextInput::new` 强制合计界限：`text` + `enter` 必须落在其中——4,096-byte 的
text 仅在无 `enter` 时合法。字段私有，任何公开构造或解码路径都无法越界，wire encoder
再独立复核一次。`key` 携带闭集 15 名 `SendKey` 之一（`enter`、`escape`、`tab`、
`backspace`、`delete`、`up`、`down`、`left`、`right`、`home`、`end`、`page-up`、
`page-down`、`ctrl-c`、`ctrl-d`）；除具名 `ctrl-` 变体外不可表达修饰键，任意键序列、
多 record 原子性、raw hex 与 paste 均不在本契约内。

每个 `TerminalSendOutcome` 变体都携带正交的 `cleanup` 轴：`accepted`/`rejected`/
`unavailable`/`outcome-unknown` 各自与 `not-needed`、`confirmed`、`unconfirmed` 的附件清理
状态配对。两种配对不可能出自 Attach → input → Detach 序列，构造期与两侧 codec 边界
都拒绝：`accepted` + `not-needed`（已接受的 input 意味着真实附件）与
`outcome-unknown` + `confirmed`（confirmed 释放意味着已完成的交换）。`accepted` 回显
`acceptedBytes`——Host 编码后入队字节数（`SendAcceptedBytes`，0 至 `Self::MAXIMUM`）；
零合法，因为终端模式可能忽略某语义键。Accepted 仅表示 Host 已将该 record 接入 input
队列——绝不表示子进程读过、执行过或已完成。拒绝与不可用原因是闭集；
`outcome-unknown` 表示 record 可能已落地，绝不自动重试。

`TerminalSendResult` 另携带 `target`：Server 实际准入本 send 的 `SendTarget` 物理化身
——worker generation、Host epoch 与 Host 终端身份。`target` 字段在 wire 形态中始终存在
且可为 `null`，但 `null` 仅对 cleanup 为 `not-needed` 的 `rejected`/`unavailable`
合法——即在任何 binding 解析前决定的 pre-admission 拒绝。每个 `accepted`、
`outcome-unknown` 或 cleanup 为 `confirmed`/`unconfirmed` 的结果必须记录实际准入的
target；构造与两个 codec 边界一致执行该规则，而 cleanup 为 `not-needed` 的
`rejected`/`unavailable` 结果仍可携带已钉定的 target。消费方逐字段照录该
target，绝不重新解析。

## Launch read binding

`TerminalLaunchReadRequest`（`terminal.launch.read`）与 `TerminalLaunchReadResult`
（`terminal.launch.read.result`）承载一次针对单个终端的只读 launch-fact 查询。请求为
`requestId` + `terminalId` 加必选 `protocolVersion`；不派发任何 Worker/Host RPC、不挂
viewer、不持有 grant、绝不变更——Server 在 coordinator 锁内按其当前已提交的 Workbench
membership 解析目标，并一趟返回。

`found` outcome 携带当前路由行（`terminalId`、`workerId`、`boundTo` 的
presentation/panel/tab/context identity）、`siteId`、join 观测到的 decimal-string
`workbenchRevision` 以及一个 `launch` fact 对象。若终端经具名 Launch Profile 启动，
`found` 还携带可选成员 `launchProfile` `{profileId, catalogRevision}`——在 launch
prepare 时冻结的 provenance；其他方式启动的终端该成员整体缺席，且即便 profile 在其
选择被捕获后遭删除，该成员仍保持准确。其余 outcome 只有 `not-found` 与
`unavailable{reason:"server-busy"}`；无 Workbench 的 Site 一律回答 `not-found`，包括
仅存的 orphan retained 记录。

`launch.state` 逐字呈现 retained launch phase——`creating`、`awaiting-start`、
`starting`、`started`、`unknown` 或 `unavailable`——另加 `not-tracked`：当可见 binding
没有可唯一归属且一致的 coordinator 记录时使用（缺失、歧义或不一致的记录如实报告，绝不
修补）。`not-tracked` 是对 retained fact 的观察，不是 `unknown` phase，也绝非进程存活；
`creating` 同样只记录 phase，不证明 dispatch 已发生。`workerGeneration` 为 positive
decimal string，恰在 `not-tracked` 时为 `null`；`geometry` 恰在 `started` 时非 null；
`createOutcome` 照录 retained `CreateOutcome` 编码，在 `creating` 与 `not-tracked` 时为
`null`。三个成员始终存在：缺 key 即 malformed frame，`null` 才表示缺席。decoder 在
closed-dictionary 与 identity 校验之外一并执行这些 conditional-null 规则。任一方向上都不
出现 lease、fence、key 或 authorization 内部字段。

## Request boundary

本 crate 不建模 replay lookup、历史 duplicate detection 或 runtime idempotency ledger。
`RequestId` 仅用于 request/result correlation；Client 为每次 submitted request 生成新 value，且
不会在同一个 active Server authority 与 authenticated subject 范围内复用。
`terminalCreateRequestId` 在同一 scope 内标识 deferred create，不依赖具体 Client Session
transport。Client 对 mutating intent 至多提交一次，且不得在 timeout、EOF、Session loss 或 caller
abandonment 后自动重发。这些事件都不编码 cancellation，也不能证明 effect 未 commit；ambiguous
completion 留给未来的只读 projection 或 observation boundary。

## 验证

从仓库根目录运行：

```sh
CARGO_TARGET_DIR="$PWD/target" cargo fmt --all -- --check
CARGO_TARGET_DIR="$PWD/target" cargo check -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo test -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p orchbee-client-server-protocol --all-targets --all-features --locked -- -D warnings
```

Conformance test 消费 canonical cross-language corpus，并覆盖 frame boundary、strict payload
decoding、construction invariant、partial I/O 与 no-flush behavior。它们不证明 transport、Server
state machine、authorization policy、Worker 或 Host mapping、terminal lifecycle 或 end-to-end
execution。

## 独立终端打开 binding

`TerminalOpenRequest`、`TerminalOpenResult` 与 `OpenOutcome` 实现尚未发布的
[独立终端打开 contract](../../schemas/client-server/README.md#standalone-terminal-opening)。
请求只携带关联 ID 和完整 geometry；Opened 返回供 legacy viewer attachment 保留使用的 Server
独立生成引用。AttachBinding 不改变 Open result。Binding 拥有严格编码和校验，Server 拥有
singleton selection、creation 与 lifetime。
Canonical fixtures 覆盖所有 open outcome，并通过 JSON schema 和独立 Rust decoder
拒绝缺失字段、额外字段、畸形及方向错误的消息。

Open 与 AttachBinding 是增量、尚未发布的 OBCS v1 source contract。它们保留既有 frame version
与 limit。安装后的 exact runtime cohort 是 Client、Server、Worker 与 Host；这个 Rust source
binding 是 contract input，而不是已安装的 cohort artifact。发布前，其 source contract 与 runtime
cohort 一起 rollback，且不涉及 persisted state 或 migration。
