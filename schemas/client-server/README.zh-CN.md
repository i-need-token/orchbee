# Client-Server Terminal Control Contract

## 权威与边界

[`v1/client-server.schema.json`](v1/client-server.schema.json) 与本文档是 Orchbee Client-Server
terminal-control protocol version 1（`OBCS`）的语言中立权威。OBCS 通过任意有序 byte stream，
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

Version 1 只准入 `terminal.create`、`terminal.create.result`、`terminal.start` 与
`terminal.start.result`。它不定义 attach、input、start 后 resize、close、cancel、retry transport、
authentication、authorization policy、Server state store、Worker operation、Host launch 或
Workbench mutation。

## Frame

| Field          | Encoding                                              |
| -------------- | ----------------------------------------------------- |
| Magic          | 四个 byte `OBCS`                                      |
| Frame version  | unsigned 16-bit big-endian integer `1`                |
| Payload length | unsigned 16-bit big-endian byte length，至多 32 KiB   |
| Payload        | 带有 `protocolVersion: 1` 的 strict UTF-8 JSON object |

Decoder 在分配 payload storage 之前校验完整的八 byte header，精确读取声明的 payload，并拒绝
无效 magic、不支持的 frame version、超过 32 KiB 的 payload、truncation，或在解码单个完整
in-memory frame 时出现的额外 byte。Stream reader 每次消费一个 frame，因此后面可以跟随另一个
完整 frame。Reader 与 writer 必须支持 partial I/O；frame writer 不会 flush caller 的 stream。

Payload 拒绝 UTF-8 BOM、无效 UTF-8、任意深度的重复 object key、尾随的第二个 JSON value、
非 object root、未知 field、未知 message 或 enum tag，以及方向错误的 message。Whitespace、
object-key order 与等价 JSON string escape 不具有语义。Canonical encoding 是确定性的 compact
JSON；equality 与 request replay 比较 decoded value，而不是 raw payload byte。

## Identity、path 与 geometry

`requestId`、`terminalCreateRequestId` 与 `workerId` 是 opaque、nonzero 128-bit value，编码为
恰好 32 个小写十六进制字符。`terminal.start` request 使用彼此不同的 `requestId` 与
`terminalCreateRequestId`。

显式 `cwd` 是 absolute POSIX UTF-8 path，其 encoded path value 包含 1 到 4,096 个 byte 且不含
NUL。`/` 有效。OBCS 不 normalize path、不 resolve `.` 或 `..`、不检查 filesystem、不推断 Git
含义，也不证明所选 Worker 会接受它。`cwd: null` 请求由所选 Worker boundary resolve 其已授权
default；省略该 field 无效。

每个 geometry 都包含全部四个 integer field，并满足所有 invariant：

| Field 或 product                                       | 范围               |
| ------------------------------------------------------ | ------------------ |
| `columns`、`rows`、`cellPixelWidth`、`cellPixelHeight` | 1 到 4,096，含边界 |
| `columns * rows`                                       | 至多 262,144       |
| `columns * cellPixelWidth`                             | 至多 65,535        |
| `rows * cellPixelHeight`                               | 至多 65,535        |

## Client request

`terminal.create` 包含 `protocolVersion`、`requestId`、显式 `workerConstraint`、显式 `cwd` 与
`startMode`。

| Field              | 准入 value                                                                  |
| ------------------ | --------------------------------------------------------------------------- |
| `workerConstraint` | `null`，或 `{ "kind": "exact", "workerId": ... }`                           |
| `cwd`              | `null`，或一个有效 absolute POSIX path                                      |
| `startMode`        | `{ "kind": "immediate", "geometry": ... }`，或恰好 `{ "kind": "deferred" }` |

Deferred creation 不携带 geometry 或 fallback。Immediate creation 携带一个完整有效 geometry。
Client 绝不提交 Terminal identity、Worker generation、operation、lease、fence、Host identity 或
epoch、executable、argument vector、environment 或 overlay、Project、Workspace、Panel、Worktree、
Client generation、profile 或 configuration fact。

`terminal.start` 包含 `protocolVersion`、新的 `requestId`、用于标识 deferred create request 且与
前者不同的 `terminalCreateRequestId`，以及一个完整有效 `geometry`。它不携带 terminal identity
或 fallback geometry。

## Server result

每个 result 都包含 `protocolVersion`、确切 request 的 `requestId`，以及一个 closed `outcome`。
Result 不暴露内部 Terminal、Worker、operation、lease、fence、Host 或 placement fact。

| Create outcome    | Closed detail                                                                                    |
| ----------------- | ------------------------------------------------------------------------------------------------ |
| `accepted`        | `state` 为 `awaiting-start`，或为带 accepted complete `geometry` 的 `started`。                  |
| `rejected`        | `not-authorized`、`worker-constraint-unsatisfied`、`cwd-not-accepted` 或 `request-id-conflict`。 |
| `unavailable`     | `no-compatible-worker`、`terminal-runtime-unavailable` 或 `server-busy`。                        |
| `failed`          | `terminal-create-failed`。                                                                       |
| `outcome-unknown` | 没有额外 field。Client 必须 reconcile，不能推断 success 或作为新 effect retry。                  |

| Start outcome     | Closed detail                                                                                                                  |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `accepted`        | Accepted complete `geometry`，以及 `started` 或 `already-started` 的 `transition`。                                            |
| `rejected`        | `not-authorized`、`terminal-create-unavailable`、`terminal-not-awaiting-start`、`geometry-conflict` 或 `request-id-conflict`。 |
| `unavailable`     | `terminal-runtime-unavailable` 或 `server-busy`。                                                                              |
| `failed`          | `terminal-start-failed`。                                                                                                      |
| `outcome-unknown` | 没有额外 field。Client 必须 reconcile，不能推断 transition。                                                                   |

## Request 与 cancellation obligation

在同一个 active Server authority 中，一个 request identity 命名一个 semantic request。用相同
identity 重复 semantically equal 的 decoded request 时，必须与该 request reconcile，而不能授权
另一个 effect；将该 identity 用于不同 decoded request 时返回 `request-id-conflict`。本 contract
定义 boundary obligation 与 result vocabulary，不实现 idempotency ledger 或 Server。

Version 1 没有 cancel message。Transport loss、Client Session loss 或 caller abandonment 都不编码
cancellation，也不得被解释为 terminal effect 未 commit 的证明。

## Conformance 与限制

[`v1/client-server-conformance-v1.json`](v1/client-server-conformance-v1.json) 是确定性的跨语言
oracle。它包含每个 message、result category、reason、accepted-state variant 与 accepted-start
transition，以及 malformed 和 semantically invalid payload。Repeated-value recipe 以紧凑形式
materialize 精确 byte-boundary、multibyte 与 worst-case JSON path input。Binding 消费该 corpus 时
不能从自己的 codec 派生 expected result。

从仓库根目录运行：

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p orchbee-client-server-protocol --all-targets --all-features --locked -- -D warnings
```

这些检查建立 schema-to-binding corpus agreement 与 deterministic frame behavior。它们不建立
authentication、authorization decision、same-active-Server idempotency storage、cancellation、
terminal execution、persistence、recovery 或任何 live transport integration。
