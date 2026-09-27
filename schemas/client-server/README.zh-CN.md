# Client-Server Terminal Control Contract

## 权威与边界

[`v2/client-server.schema.json`](v2/client-server.schema.json) 与本文档是 Orchbee Client-Server
terminal-control protocol version 2（`OBCS`）的语言中立权威。OBCS 通过任意有序 byte stream，
在已授权 Client Session 与 Server 之间承载 terminal semantic control。Rust crate
`orchbee-client-server-protocol` 是独立 binding，不得增加 value、省略 invariant 或重新定义 field
含义。

OBCS 有以下相邻边界：

| 边界                 | 职责                                                                                             |
| -------------------- | ------------------------------------------------------------------------------------------------ |
| `OBLC`               | 仅负责同一用户本地 Client connection admission 与 liveness；它不授予 terminal operation。        |
| `OBCS`               | 负责 carrier-neutral Client-to-Server terminal semantic request 与稳定的 Client-visible result。 |
| `OBCN`               | 负责 Server-to-Worker placement authorization、operation、generation、lease 与 fence fact。      |
| Workbench projection | 只读 authorized product projection；它不是 terminal-control writer。                             |

Version 2 准入 `terminal.create`、`terminal.create.result`、`terminal.start`、
`terminal.start.result`、`terminal.viewer`、`terminal.viewer.result`、`terminal.control` 和
`terminal.control.result`，以及尚未发布的 `terminal.open` / `terminal.open.result` 和 viewer
`attach-binding` 扩展、`terminal.launch.read` / result 以及 `terminal.send.result`。Send request 使用 OBSN binary record。它不定义 close、cancel、retry transport、authentication、authorization
policy、Server state store、Worker operation、Host launch 或 Workbench mutation。

## Frame

| Field          | Encoding                                              |
| -------------- | ----------------------------------------------------- |
| Magic          | 四个 byte `OBCS`                                      |
| Frame version  | unsigned 16-bit big-endian integer `1`                |
| Payload length | unsigned 16-bit big-endian byte length，至多 48 KiB   |
| Payload        | 带有 `protocolVersion: 2` 的 strict UTF-8 JSON object |

Decoder 在分配 payload storage 之前校验完整的八 byte header，精确读取声明的 payload，并拒绝
无效 magic、不支持的 frame version、超过 48 KiB 的 payload、truncation，或在解码单个完整
in-memory frame 时出现的额外 byte。Stream reader 每次消费一个 frame，因此后面可以跟随另一个
完整 frame。Reader 与 writer 必须支持 partial I/O；frame writer 不会 flush caller 的 stream。

Payload 拒绝 UTF-8 BOM、无效 UTF-8、任意深度的重复 object key、尾随的第二个 JSON value、
非 object root、未知 field、未知 message 或 enum tag，以及方向错误的 message。Whitespace、
object-key order 与等价 JSON string escape 不具有语义。Numeric `protocolVersion` 与 geometry
value 会先 refine 为有限的 IEEE 754 binary64 safe integer，再应用其精确 bound。因此 `2.0`、
`2e0` 与 `8e1` 等 integral decimal 或 exponent spelling 在 refined value 位于范围内时有效；
refinement 后仍为 fractional 的 value、non-finite value、unsafe integer 与越界 value 无效。
Canonical encoding 是确定性的 compact JSON，并使用普通 integer token。

## Identity、path 与 geometry

`requestId`、`terminalCreateRequestId`、`siteId`、`workerId` 与 `terminalId` 是 opaque、nonzero
128-bit value，编码为恰好 32 个小写十六进制字符。Site、Worker、Terminal 与 request identity
属于不同语义域，但其编码 byte 无需彼此不同；每个 field 都独立校验。`requestId` 用于关联一次
已提交 request 及其 result；它不是 replay 或 Server-ledger identity。Client 为每次 submitted
request 生成新的 `requestId`，且不会在同一个 active Server authority 与 authenticated subject
范围内复用。这是 Client producer obligation，不是 Server 保留或搜索 request history 的承诺。
独立的 `terminal.start` invariant 要求 `requestId` 与 `terminalCreateRequestId` 彼此不同。它的
`terminalCreateRequestId` 在同一个 authority 与 subject 范围内标识 deferred create，不依赖具体
Client Session transport。

显式 `cwd` 是 absolute POSIX UTF-8 path，其 encoded path value 包含 1 到 4,096 个 byte 且不含
NUL。`/` 有效。OBCS 不 normalize path、不 resolve `.` 或 `..`、不检查 filesystem、不推断 Git
含义，也不证明所选 Worker 会接受它。`cwd: null` 请求由所选 Worker boundary resolve 其已授权
default；省略该 field 无效。

每个 geometry 都包含全部四个 integer field，并满足所有 invariant：

| Field 或 product                    | 范围                                                                     |
| ----------------------------------- | ------------------------------------------------------------------------ |
| `columns`、`rows`                   | 1 到 4,096，含边界                                                       |
| `cellPixelWidth`、`cellPixelHeight` | 0 到 4,096，含边界；必须同正（pixel 已知）或同零（pixel 未知），不得混合 |
| `columns * rows`                    | 至多 262,144                                                             |
| `columns * cellPixelWidth`          | 至多 65,535                                                              |
| `rows * cellPixelHeight`            | 至多 65,535                                                              |

## Client request

`terminal.create` 包含 `protocolVersion`、`requestId`、显式 `workerConstraint`、显式 `cwd` 与
`startMode`。

| Field              | 准入 value                                                                  |
| ------------------ | --------------------------------------------------------------------------- |
| `workerConstraint` | `null`，或 `{ "kind": "exact", "workerId": ... }`                           |
| `cwd`              | `null`，或一个有效 absolute local-platform path                             |
| `startMode`        | `{ "kind": "immediate", "geometry": ... }`，或恰好 `{ "kind": "deferred" }` |

Deferred creation 不携带 geometry 或 fallback。Immediate creation 携带一个完整有效 geometry。
`terminal.create` request 绝不提交 Terminal identity、Worker generation、operation、lease、fence、
Host identity 或 epoch、Project、Workspace、Panel、Worktree、Client generation、profile 或 configuration fact。可选 literal launch/environment value 遵循下方 version-2 契约。

`terminal.start` 包含 `protocolVersion`、新的 `requestId`、用于标识 deferred create request 且与
前者不同的 `terminalCreateRequestId`，以及一个完整有效 `geometry`。它不携带 terminal identity
或 fallback geometry。

## Server result

每个 JSON result 都包含 `protocolVersion`、确切 request 的 `requestId`，以及一个 closed `outcome`。
Create/Start result 不暴露内部 Terminal、Worker、operation、lease、fence、Host 或 placement fact。

| Create outcome    | Closed detail                                                                   |
| ----------------- | ------------------------------------------------------------------------------- |
| `accepted`        | `state` 为 `awaiting-start`，或为带 accepted complete `geometry` 的 `started`。 |
| `rejected`        | `not-authorized`、`worker-constraint-unsatisfied` 或 `cwd-not-accepted`。       |
| `unavailable`     | `no-compatible-worker`、`terminal-runtime-unavailable` 或 `server-busy`。       |
| `failed`          | `terminal-create-failed`。                                                      |
| `outcome-unknown` | 没有额外 field。Client 必须 reconcile，不能推断 success 或作为新 effect retry。 |

| Start outcome     | Closed detail                                                                                           |
| ----------------- | ------------------------------------------------------------------------------------------------------- |
| `accepted`        | Accepted complete `geometry`，以及 `started` 或 `already-started` 的 `transition`。                     |
| `rejected`        | `not-authorized`、`terminal-create-unavailable`、`terminal-not-awaiting-start` 或 `geometry-conflict`。 |
| `unavailable`     | `terminal-runtime-unavailable` 或 `server-busy`。                                                       |
| `failed`          | `terminal-start-failed`。                                                                               |
| `outcome-unknown` | 没有额外 field。Client 必须 reconcile，不能推断 transition。                                            |

## Session 查看附件控制与二进制记录

`terminal.viewer` 包含 `protocolVersion`、新的 `requestId` 和一个封闭 `action`。每个已准入
Client Session 可以在既有有序 carrier 上拥有多个只读查看附件。`attach` 只提供 `terminalCreateRequestId`；Server
解析同一 authority 与主体下已接受的终端，固定其既有 Worker 代次和 Host epoch，并签发新的
附件身份。`attach-binding` 则仅提供从 canonical Workbench snapshot 获得的 `siteId`、`workerId`
与逻辑 `terminalId`。它不携带 Workbench revision、Worker generation、Host identity、operation、
lease 或 Create reference。

Binding triple 选择既有 terminal，且不授予 authorization。Server 必须先确认当前 active Site，
且 canonical Workbench presentation 仍暴露精确 Worker/Terminal pair，之后才解析精确保留终端
及其固定的 Worker generation。Selection 不产生 Open、Create、Start、launch、membership、rebind、
navigation、input、resize 或 controller effect。经过 Server 拥有的 Session capacity guard 后，target validation
遵循以下优先级：

1. registry 或 Site ownership poisoned 返回 `server-busy`；
2. Workbench 缺失或已 retired 返回 `workbench-unavailable`；
3. Site 错误或精确 pair 未暴露返回 `terminal-unavailable`；
4. current Worker 缺失或 logical Worker 不同返回 `terminal-runtime-unavailable`；
5. 精确保留记录缺失返回 `terminal-unavailable`；
6. 保留的 Worker generation 不同返回 `terminal-runtime-unavailable`；
7. 保留 phase 无法 attach 返回 `terminal-unavailable`；
8. attachment identity 耗尽或已知未改变的 capacity 不足返回 `server-busy`。

Workbench exposure 与 retained-target resolution 一起 linearize。之后 Workbench retired 不会取消
已固定的 operation。Attachment 成功在既有 post-IPC current-Worker check 处线性化；
检查后的 Worker loss 可能与响应交付竞争，由下一次 operation 的 fence 拦截。`poll`、`frame-applied`、
`need-snapshot`、`detach`、`read-rows`、`set-projection-demand`、`set-history-demand`、`read-viewport-rows`、
`rows-applied` 与 `history-observed` 必须在 action 内携带 `attachmentId`，采用非零 32 字符小写十六进制身份格式。
它只选择 Server 已签发且归提交 Session 所有的 viewer，不能创建或转移所有权。
`attach` 与 `attach-binding` 禁止该字段。Session 丢失会分离临时查看状态，绝不
终止保留的终端。该 action 只改变 OBCS JSON；既有 OBCN relay、OBSR framing 与 byte limit 不变。

`frame-applied` 还携带 `sequence`、`generation` 和 `revision`；`need-snapshot` 还携带 `sequence`
和 `reason`。计数器使用 unsigned 64-bit 范围内的规范十进制字符串，无前导零或符号；sequence
和 generation 必须为正。Reason 为 `activity-stale`、`base-mismatch`、`decoder-rejected`、
`dimensions-mismatch`、`generation-mismatch`、`identity-mismatch`、`sequence-gap` 和 `store-reset`。
只有所选 viewer 待确认帧的精确反馈才能到达其 Worker 附件。不同 viewer 的计数器可以相等；
有效性由 selector 与该 viewer 的 frame scope 决定。该 viewer 等待反馈期间 Poll 返回 `empty`，
不会交付另一帧，也不阻塞兄弟 viewer。
新附件需要新 baseline；查看附件不授予 input 或 resize 权限。

`read-rows` 对已固定的 `generation` 与 `revision` 执行一次 buffer 读取，覆盖保留历史加可见行，
以绝对 `startRow` 为锚点。除 `attachmentId` 外，它携带规范十进制字符串 `startRow` 与 `revision`、
正数规范十进制字符串 `generation` 与 `requestSequence`、整数 `rowCount`（1 到 4096）和整数
`maximumBytes`（112 到 8,388,608，界定 OBRW payload 而非载体 record）。它是一次关联的 Host
exchange，不含 acknowledgement、replay 或 retry：超出字节预算是 typed failure，绝不是部分成功。
穿插的 Host event 仍路由到各自归属附件，绝不会被误认为该响应。成功应答是经既有 OBSR 二进制
载体返回的完整关联 OBTP response record——与回答 Poll 的 `Record` 形状相同，但内容是 response
而非 event。

`set-projection-demand` 与 `set-history-demand` 各携带 `attachmentId` 和一个 `demand`：projection 为
`active` 或 `paused`，history 为 `following-bottom` 或 `viewing-history`。`read-viewport-rows`
对保留历史加可见行执行一次 buffer 读取，锚点相对 Client 历史位置而非绝对行：`origin` 为
`{"kind":"tail","rowsBefore":N}`（`rowsBefore` 为 0 至 4,294,967,295 的整数）或
`{"kind":"committed","delta":N}`（`delta` 为 -2,147,483,648 至 2,147,483,647 的整数）。除
`attachmentId` 与 `origin` 外，它携带整数 `rowCount`（1 到 4096）、整数 `maximumBytes`（112 到
8,388,608，界定 OBRW payload 而非载体 record）、可过期的规范十进制字符串 `revision`，以及正数
规范十进制字符串 `generation` 与 `requestSequence`。它是一次关联的 Host exchange，不含
acknowledgement、replay 或 retry：服务应答的 revision 标识本次捕获的 window，不是对请求
revision 的全等回显，Host 可交付 clamp 后的窗口。`rows-applied` 以正数规范 `requestSequence` 与
`generation` 加规范 `revision` 确认 Client 已应用的窗口；`history-observed` 以 `sequence`、
`generation` 与 `revision`（同一计数器形状）确认一条历史事件。穿插的 Host event 仍路由到各自
归属附件，绝不会被误认为该响应。

未知或不归当前 Session 所有的 selector，在 Poll、反馈或 ReadRows 时返回 `terminal-unavailable`，Detach 时
返回无副作用的 `detached`，Control 时返回 `viewer-unavailable`。同一 Session 重复附着同一个
已解析的存活 runtime target 返回 `already-attached`；不同 target 可以共存。
[Server 归属指南](../../apps/orchbee-server/README.md#accepted-session-viewer-routing-modeling-record)
定义容量优先级、disposal、generation fence 和 cleanup。Codec 校验语法，不判断运行时所有权。

必需 selector 改变了尚未发布的 OBCS v2 请求形状。必须一起构建、部署和回滚精确匹配的
Client/Server/Worker 开发 cohort；不支持混合 cohort 或隐式 current viewer fallback。
OBSR 载体格式不变；`read-rows`/`rows-rejected` 对匹配的 OBCN cohort 做增量扩展（action tag 6
与 result tag 6），五个 viewport 与 history action 同样新增 OBCN action tag 7 至 11——旧 Worker
会拒绝新 tag 而非静默忽略。

JSON 查看附件 outcome 包括：带 `attachmentId` 和 `hostEpoch`（非零 32 字符小写 hex）的
`attached`；无其他字段的 `empty`、`feedback-accepted`、`detached`；reason 为 `already-attached`、
`terminal-unavailable`、`feedback-mismatch` 或 `host-rejected` 的 `rejected`；reason 为 `stale-revision`、
`stale-generation`、`history-evicted`、`missing-resource`、`invalid-field`、
`invalid-acknowledgement`、`limit-exceeded`、`resource-exhausted` 或 `invalid-phase` 的
`rows-rejected`；reason 为 `server-busy` 或
`terminal-runtime-unavailable`，或仅用于 viewer 的 `workbench-unavailable` 的 `unavailable`。
Attached 回答 Attach 或 AttachBinding；Empty 或二进制记录回答 Poll；FeedbackAccepted 回答两种
反馈或四个控制确认 action 之一（`set-projection-demand`、`set-history-demand`、`rows-applied`、
`history-observed`）；Detached 回答 Detach，包括附件已不存在的情况；二进制 record 或
RowsRejected 回答 ReadRows 或 ReadViewportRows，二者绝不回答 Empty。RowsRejected 只回答
ReadRows 或 ReadViewportRows——出现在其它 action 上即协议
违例——且合法的 RowsRejected 不会使该 viewer 退役。`host-rejected` 仅报告四个控制确认 action
之一的 Host typed failure；健康 viewer 在其后保持附着且可用。新增 reason 扩的是封闭 enum——不
扩 consumer 可产生的配对集合。Rejected 和 Unavailable 可回答任意
action。错误 kind 或 correlation 会让 Client carrier 退役。只有已知未 commit 任何 effect 的
correlated `server-busy` result 才可使用既有 bounded retry policy：Client 保留完全相同的 action
value，使用新的 `requestId`，并处于同一 logical deadline 内。Ambiguous failure 绝不授权 replay、
fallback 或 retargeting。

记录结果使用二进制 `OBSR`，不使用 JSON：四个 magic byte、big-endian u16 version 1、两个为零
的保留字节，以及 big-endian u32 payload length。Payload 是以 16 个 big-endian byte 表示的确切
request ID，后接一条长度为 64 至 `64 + 36 * 1024 * 1024` byte 的 opaque OBTP record——对 Poll
是 Host event record，对 ReadRows 或 ReadViewportRows 是关联的 Host response record。内层 Host correlation 是
Server 签发的 Worker operation identity；Client `requestId` 只存在于该外层载体中，绝不与内层
身份等同。
分配 payload 前校验 12-byte header 和 bound；stream reader 消费一帧，内存 decoder 拒绝截断或
多余字节。只有 Server-to-Client reader 接受 OBSR。Server 校验 Host epoch、附件、role、frame
路由元数据和反馈；呈现解码仍由 Client 负责。OBSR 没有 JSON `record` outcome，也不扩大 48 KiB
OBCS JSON 限制。其字节契约由本节定义；JSON corpus 拥有控制消息，binding test 覆盖二进制边界
和 opaque byte 保留。

## Session controller

`terminal.control` 包含 `protocolVersion`、新的 `requestId` 和一个封闭 `action`。所有 control action
必须在 action 内携带 `attachmentId`。`acquire` 没有其他字段，并要求该精确 viewer 已附着且归
提交 Session 所有。Server 为每个终端显式授予一个输入与调整大小共用的
controller，不自动提升。普通 Acquire 拒绝已占用终端；显式 `takeover` 同样仅携带
`attachmentId`，替换已激活的旧控制者，拒绝仍在获取/交接中的授权。Server 分配新授权
身份并在交接前封锁旧授权；Worker 收到精确旧 Host detach 确认后才获取新授权。接管失败
可能撤销旧权限而未授予新权限，绝不静默恢复权威。Rejected、Unavailable 和 OutcomeUnknown
保持区分；清理结果未知时保留封锁。见[接管模型](../../apps/orchbee-server/docs/terminal-controller-takeover.zh-CN.md)。
此 v2 新 action 要求同一开发版本整体部署，旧 peer 会拒绝它。`acquired` 结果携带新的 `controllerId`，采用非零 32 字符小写
hex 身份格式。Grant 绑定该 Session、viewer、Worker 代次与 Host 终端。

`input` 还包含 `controllerId`、`sequence` 和 `bytes`；`resize` 还包含 `controllerId`、`sequence`
及完整 `geometry`。输入为 1 至 4,096 个原始 byte，以无前缀、偶数长度的小写十六进制字符串
编码；不是 UTF-8 文本、Base64 或 executable metadata。两个 action 共用严格递增的正 u64
sequence，以规范十进制字符串编码。允许跳号；有效提交即消费 sequence，即使下游交付失败或
拒绝。Grant 不匹配或过时 sequence 不产生终端副作用。新 grant 没有已消费的 sequence；旧 grant
不能通过重置 sequence 重新获得权限。

`paste` action 是语义级整 envelope 操作：在同一 attachment、controller 与严格递增
sequence 下提交 1 至 262,144 个不透明内容 byte，由 Host 按其当前 bracketed-paste mode 编码
一次。它绝不由 OBCS JSON 承载：`terminal.control` 内的 `"kind": "paste"` action 在两个方向
均非法。唯一的请求编码是 Client→Server 流上的二进制 `OBCQ` record：四个 magic byte、大端
u16 version 1、两个零保留 byte，再大端 u32 body 长度。Body 依次为 16 个大端 byte 的精确
request ID、16 byte 的 attachment ID、16 byte 的 controller ID、大端 u64 正 sequence，然后
是原始内容 byte——总计 57 至 262,200 byte。分配 body 前先校验 12-byte header 与界限；流
reader 每次消费一个 record，内存 decoder 拒绝截断或尾随 byte。OBCQ 没有对应的 JSON `paste`
action，也不扩大 48 KiB OBCS JSON 上限；小粘贴同样使用该 record 编码。

`terminal.control.result` 使用普通关联 JSON envelope。Outcome 包括：带 `controllerId` 的
`acquired`、无其他字段的 `input-accepted`、带精确接受 `geometry` 的 `resized`、带
`acceptedBytes`（1 至 1,048,576 的整数，表示 Host 接纳进有界输入 queue 的编码后 byte 数）的
`pasted`、无其他字段的 `outcome-unknown`；reason 为 `viewer-unavailable`、`controller-busy`、
`not-controller`、`sequence-mismatch`、`host-rejected`、`host-input-invalid`、
`host-input-limit-exceeded`、`host-input-backpressure` 或 `host-input-unavailable` 的
`rejected`；reason 为 `server-busy` 或 `terminal-runtime-unavailable` 的 `unavailable`。
InputAccepted 与 Pasted 表示完整进入 Host 有界输入 queue，不表示 shell 已执行。Rejected、
Unavailable 和 OutcomeUnknown 可回答任意 action；其他 outcome 必须匹配所提交 action，Resize
还必须匹配其 geometry。`pasted` 结果绝不回答非 paste action，`input-accepted` 也绝不回答
`paste`。

Detach 或 Session 丢失在退役 viewer 前释放共用 grant。本切片没有独立 Client release action。
待定 acquisition 与 cleanup 排斥竞争 grant；cleanup 可以超过 Client submission deadline。
未知 input/resize 绝不自动重放，包括 reconnect 或新 grant 后。明确拒绝也不授予重试策略。
Conformance corpus 覆盖全部 control action、outcome、reason、byte bound 和非法 scope 字段。
Runtime 所有权与 Host admission 仍由 Server 和 Worker 归属指南负责。

## Submission、reference 与 cancellation obligation

Client 对每个 mutating intent 至多提交一次。OBCS version 2 不定义 replay lookup、历史 duplicate
detection 或 idempotency ledger。Consumer 可在其当前 accepted state 暴露 ID collision 或以其他
方式使 request 无效时拒绝该 request，但这种 state validation 不是保留、识别或 replay 历史
request 的承诺。

Version 2 没有 cancel message。Timeout、transport loss、Client Session loss 或 caller abandonment
既不编码 cancellation，也不能证明 terminal effect 未 commit。Client 不得在这些事件后自动重发
mutating intent；ambiguous result 应改由未来的只读 projection 或 observation boundary reconcile。

## Conformance 与限制

[`v2/client-server-conformance-v2.json`](v2/client-server-conformance-v2.json) 是确定性的跨语言
oracle。其 JSON fixture 与独立编写的 binary hex record 覆盖每个 message、result category、reason、accepted-state variant 与 accepted-start
transition，以及 malformed 和 semantically invalid payload。Repeated-value recipe 以紧凑形式
materialize 精确 byte-boundary、multibyte 与 worst-case JSON path input。Binding 消费该 corpus 时
不能从自己的 codec 派生 expected result。AttachBinding fixture 覆盖精确 field name、required 与
additional field、lexical identity failure、zero sentinel，以及错误 message/result kind。其独立
typed identity domain 之间相同的有效 byte 是 canonical。Schema validation 无法判断 well-formed
tuple 是否指向当前暴露的 terminal；该 semantic refusal 由 Server test 负责。

从仓库根目录运行：

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p orchbee-client-server-protocol --all-targets --all-features --locked -- -D warnings
```

这些检查建立 schema-to-binding corpus agreement 与 deterministic frame behavior。它们不建立
authentication、authorization decision、replay lookup、duplicate-history 或 idempotency storage、
cancellation、terminal execution、persistence、recovery 或任何 live transport integration。

## 独立终端打开

尚未发布的 version-2 扩展 `terminal.open` 携带 `protocolVersion`、Client 新生成的
`requestId` 和完整 `geometry`。它打开本机单主体 Server 的独立终端引用：首次准备使用
Worker 默认 cwd 创建立即启动的终端；后续打开返回同一个引用。Open 自身不 dispatch 或提交 Resize；Client 可随后 attach、获取 controller 权限，并提交独立的已校验 Resize。
此终端线协议不增加 Workbench payload。原生 Server 拥有启动前首次共享独立终端成员关系的提交，
具体定义见其
[Workbench 权威](../../apps/orchbee-server/docs/workbench-authority.zh-CN.md#独立终端成员关系模型)；
Client 通过独立的 Workbench 协议观察该结构。

`terminal.open.result` 关联确切的调用请求，并返回一个封闭 outcome：
带 `terminalCreateRequestId` 的 `opened`；带 Create unavailable 原因的 `unavailable`；
带 `terminal-create-failed` 的 `failed`；或仅有标签的 `outcome-unknown`。
返回的引用可以标识 Server 内部生成的 create。它独立于调用者的关联 ID，不授予 viewer 或
controller 权限，仍须经过普通 Session attach 和 controller 准入。AttachBinding 不改变该 result
shape；原生 standalone 流程观察独立 Workbench snapshot，而不会把这个 Create reference 当作
canonical selection state。熵源失败或引用冲突在准备前返回 server-busy，并保持独立终端槽位
为空。Dispatch 后的失败或不确定性不允许再次创建。Client 不重试未知效果；另一次打开只观察
已保留的独立终端结果。

Schema、Rust binding 与 canonical corpus 共同定义增量 Open 和 AttachBinding contract。它们仍
未发布，且不改变 frame version、byte limit、既有 action encoding、admission 或 unknown-effect
replay rule；legacy Attach 仍然可用。安装后的 exact runtime cohort 是 Client、Server、Worker 与
Host。Schema 与 Rust source 是 contract input，不是已安装的 cohort artifact。发布前，未发布的
source contract 与 runtime cohort 一起 rollback。该扩展不增加 persisted state 或 migration。

## 原生 Windows 路径细化

线协议中的绝对路径接受 POSIX 前导斜杠，或 ASCII 盘符后跟 `:` 及 `/` 或 `\`；Windows 逐字盘符路径可以带 `\\?\` 前缀。词法规则不依赖解码器所在平台；字节保持原样，不展开、不规范化。本次 Windows 增量不接受反斜杠根路径、盘符相对路径、UNC 或设备路径。既有字节上限和 NUL／Unicode 校验不变。执行方仍须在产生效果前依据本地文件系统验证路径。

## 原生版本 2

原生 cohort 使用 JSON `protocolVersion: 2`、OBCS frame version 1 和 48 KiB JSON payload 上限。[version-1 schema](v1/client-server.schema.json) 与 [corpus](v1/client-server-conformance-v1.json) 保留为冻结参考；当前 [version-2 schema](v2/client-server.schema.json) 与 [corpus](v2/client-server-conformance-v2.json) 约束原生 binding。Version 2 拒绝 version-1 body 和旧版 JSON `terminal.send` 请求。专用 OBSN binary request 保持 encoding version 1；其四个非零 16-byte identity、有序 record sequence 和 progress-result 语义属于原生 v2 契约。Binary framing、聚合 content 上限与词法验证独立于 JSON schema。

v2 Create 额外接受可选 `launch`（null 或 literal `{exec,args}`）和 `environment`（有序 `[name,value]` pair，空时省略）。Executable 必须绝对，argv 不含 argv0，最多 128 个 argument，合计 2,048 UTF-8 bytes，禁止 NUL。Environment 最多 32 项，name/value 合计最多 1,024 UTF-8 bytes。ASCII identifier name 长 1–128 bytes，忽略 ASCII 大小写唯一，并忽略大小写拒绝 `ORCHBEE_*`、`TERM`、`COLORTERM` 和 `TERM_PROGRAM`。Value 为 literal，可以为空，禁止 NUL。Byte sum 与忽略大小写唯一性由 binding 在 JSON Schema 之外细化。Found launch read 的可选同级 `launchProfile` 携带 opaque `profileId`（1–128 UTF-8 bytes）和规范 unsigned-64 decimal `catalogRevision`；它记录捕获的来源，不授予 launch authority。

Found launch read 还可携带非 null 的 `launchSelection:{exec,args}`，复用上述程序／参数边界，表示创建时经解析后冻结的选择；默认 shell 与 `not-tracked` 时省略。出现 null、非法选择或 `not-tracked` 携带选择均拒绝。旧消息可省略该成员；不导出 environment，不读取当前 profile 目录，也不声明跨重启保存。该未发布的增量字段要求同 cohort 部署／回滚，旧 reader 可能拒绝。

Send result 以 `progress` 替代 `acceptedBytes`：数值 `recordsAccepted`、`firstStoppedRecord`（0–64）和 `acceptedEncodedBytes`（0–67,108,864）。Accepted records 不超过 stopped index。成功 outcome 要求非零且相等的 count/index，以及非 not-needed 的 cleanup；任何非零 accepted count 都禁止 not-needed。Unknown outcome 禁止 confirmed cleanup。Accepted/unknown 或任何非 not-needed cleanup 都要求 physical target。跨字段关系是独立测试的 binding 细化。Unknown frontier 不授权重发。`input-budget-exhausted` 是新增 unavailable reason。仍须按精确 cohort 发布和回滚；不意味着持久化迁移、Desktop 支持或 Workbench schema 等价。

OBSN header 为 magic `OBSN`、big-endian u16 encoding version `1`、零 u16 reserved field 和 u32 body length。Body 依次为 request/Site/Worker/Terminal identity（各 16 bytes）、u8 count（1–64）及有序 record。Body 上限为 `64 + 1 + 64 * 5 + 1,048,576` bytes；分配前拒绝错误 header。每个 record 以 u8 kind 开始：

| Kind    | 剩余 record bytes                                                                                                              |
| ------- | ------------------------------------------------------------------------------------------------------------------------------ |
| 1 text  | u16 UTF-8 byte length、text、u8 enter（0 或 1）；非空 text 加可选 CR <=4,096 bytes                                             |
| 2 key   | u8 code：enter、escape、tab、backspace、delete、up、down、left、right、home、end、page-up、page-down、ctrl-c、ctrl-d 对应 1–15 |
| 3 combo | u8 非零 modifier flags（Control=1、Alt=2、Shift=4）、u8 base kind、u8 base value                                               |
| 4 bytes | u32 length、非空 opaque bytes <=1,048,576                                                                                      |
| 5 paste | u32 length、非空 opaque content <=262,144；Host 拥有 bracket encoding                                                          |

Combo base 为 named key `(1,1..13)`、Insert `(2,0)`、F1..F12 `(3,1..12)`、Space `(4,0)`、ASCII 小写字母 `(5,97..122)` 或数字 `(6,48..57)`。额外 flag bit、无效 base、未知 kind、truncation 与 trailing byte 均拒绝。Decoded text（含折叠 CR）、bytes 与 paste content 聚合最多 1,048,576 bytes；key 不携带 decoded payload。请求不具备原子性：result 保留已确认前缀和首个停止 record。v2 corpus 独立于 Rust encoder 提供 canonical binary bytes、每个 key 和 base kind、混合 record 与 negative control。

## 带样式的 buffer 读取

`read-rows` 动作可带 `detail:"text"|"render"`。省略时为 text，原生 text 编码省略默认成员，
保留既有字节。未知 detail 明确拒绝。原有边界、精确 revision/generation/start-row 固定、
关联及单次生命周期全部保留。Server 按请求验证返回的 OBRW detail。render 携带 Host 既有的
完整投影及图形快照，8 MiB 负载预算不变。render 被拒绝时不得退回 text 或静默丢弃图形。
viewport 读取仍为 text。

这是增量的未发布 OBCS v2 契约，保留现有帧版本。Client、Server、Worker 与 Host 开发产物
必须一起部署或回退。旧读取端拒绝新成员，不新增持久状态或迁移。Windows ANSI CLI 输出是
具体消费者，wire 保持平台无关。JSON schema 及独立 Rust 用例覆盖默认 text 兼容、render
选择和非法 detail；运行时 peer 验证精确响应固定。
