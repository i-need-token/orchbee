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

| API                                                                                      | Contract                                                                        |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `RequestId`、`SiteId`、`WorkerId`、`TerminalId`、`ViewerId`、`HostEpoch`、`ControllerId` | 解析独立 typed、nonzero opaque 128-bit 小写十六进制 identity。                  |
| `WorkingDirectory`                                                                       | 校验最多 4,096 个 encoded byte、无 NUL 的 absolute POSIX UTF-8 path。           |
| `Geometry`                                                                               | 仅当所有 scalar 与 product bound 都成立时构造 complete geometry。               |
| `TerminalCreateRequest`、`TerminalStartRequest`                                          | 构造 validated Client request；start 拒绝相同的 start/create request identity。 |
| `CreateOutcome`、`StartOutcome` 与 result type                                           | 只表示 OBCS v1 准入的稳定 Client-visible result combination。                   |
| `encode_client_frame`、`decode_client_frame`                                             | Encode 或 decode 恰好一个完整 in-memory Client-to-Server frame。                |
| `encode_server_frame`、`decode_server_frame`                                             | Encode 或 decode 恰好一个完整 in-memory Server-to-Client frame。                |
| `read_client_message`、`read_server_message`                                             | 从支持 partial read 的有序 stream 中精确读取一个 frame，并保留后续 frame 未读。 |
| `write_client_message`、`write_server_message`                                           | 通过 partial write 写出完整 frame，且不 flush caller。                          |

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

## Controller binding

`TerminalControlRequest`、`TerminalControlAction`、`TerminalControlResult` 和 `TerminalControlOutcome`
携带显式 Acquire 与 grant scope 内的 Input/Resize。`ControllerId` 是 Server 签发的身份；
三个 action 都必须携带 `attachment_id: ViewerId`，通过 `TerminalControlAction::attachment_id()`
读取。这是尚未发布的 OBCS v1 必需字段变更；Client/Server/Worker 必须使用精确匹配的开发 cohort，
不提供隐式 viewer fallback。
`ControlInput` 验证 1 至 `MAXIMUM_CONTROL_INPUT_BYTES`（4,096）byte，并序列化为规范小写 hex。
共用 number adapter 验证正十进制 u64 sequence。这些控制使用既有 32 KiB OBCS JSON framing；
不增加二进制 input carrier 或 Host 依赖。Schema owner 的 corpus 检查每个 action/outcome/reason
以及畸形、最大和超限 input。本 binding 不授予 controller 权限，绝不重试未知副作用。

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
