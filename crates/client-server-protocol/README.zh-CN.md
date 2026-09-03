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

| API                                             | Contract                                                                        |
| ----------------------------------------------- | ------------------------------------------------------------------------------- |
| `RequestId`、`WorkerId`                         | 解析 nonzero opaque 128-bit 小写十六进制 identity。                             |
| `WorkingDirectory`                              | 校验最多 4,096 个 encoded byte、无 NUL 的 absolute POSIX UTF-8 path。           |
| `Geometry`                                      | 仅当所有 scalar 与 product bound 都成立时构造 complete geometry。               |
| `TerminalCreateRequest`、`TerminalStartRequest` | 构造 validated Client request；start 拒绝相同的 start/create request identity。 |
| `CreateOutcome`、`StartOutcome` 与 result type  | 只表示 OBCS v1 准入的稳定 Client-visible result combination。                   |
| `encode_client_frame`、`decode_client_frame`    | Encode 或 decode 恰好一个完整 in-memory Client-to-Server frame。                |
| `encode_server_frame`、`decode_server_frame`    | Encode 或 decode 恰好一个完整 in-memory Server-to-Client frame。                |
| `read_client_message`、`read_server_message`    | 从支持 partial read 的有序 stream 中精确读取一个 frame，并保留后续 frame 未读。 |
| `write_client_message`、`write_server_message`  | 通过 partial write 写出完整 frame，且不 flush caller。                          |

所有 decoded object 与 enum 都是 closed。缺失显式 nullable create field、未知 field 或 tag、重复
key、BOM、无效 UTF-8、尾随 JSON value、direction mismatch、无效 identity、path 或 geometry，以及
不符合精确长度或 32 KiB payload bound 的 frame 都会被拒绝。Protocol 与 geometry number 使用
schema 所有的有限 binary64 safe-integer refinement，因此等价的 integral decimal 与 exponent
spelling 会 decode 为同一个 value。Canonical encoding 使用带普通 integer token 的 compact
deterministic JSON。

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
