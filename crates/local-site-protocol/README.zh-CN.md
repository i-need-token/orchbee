# 本机 Site Protocol

这是第一条 same-user POSIX Server-Worker process spine 的私有实验性 Rust connection contract。
`orchbee-server` 与 `orchbee-worker` 是其具体 consumer。它不是 public API、remote protocol、
terminal protocol 或 compatibility promise。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## Framing 与 message

每个 connection 的 OBLS 部分恰好承载一条从 Worker 到 Server 的 handshake frame 和一条从 Server
到 Worker 的 response frame。接受后，本机 Server adapter 可以把同一个 stream 保留为单独拥有的
OBCN carrier；之后不再允许 OBLS frame。OBLS frame 包含固定 `OBLS` magic、unsigned big-endian
protocol version、unsigned big-endian payload length，以及最多 256 byte 的 payload。Unknown magic、
version、kind、enum value、missing byte、extra byte、oversized payload、zero generation、invalid
UTF-8、empty cohort、non-ASCII cohort 或超过 64 byte 的 cohort 均被拒绝。

| Message          | 字段与含义                                                                                                                                       |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `WorkerHello`    | 确切 executable cohort 加 16-byte opaque development Worker identity。Codec 接受所有 bit pattern；它不请求 work 或 capability。                  |
| `WorkerAccepted` | 一条 nonzero process-local Server generation。只确认这条确切 connection。                                                                        |
| `Rejected`       | 一项 typed reason：incompatible protocol、incompatible cohort、duplicate Worker、wrong message、malformed frame 或 timeout。它不授予 authority。 |

Protocol crate 编码和解码完整 frame，并提供有界 blocking I/O adapter。它不 retry、不保留
connection、不选择 endpoint、不绑定 listener、不启动 thread 或 process，也不拥有 Server
admission state。

## 信任与边界

任一进程交换 frame 前，每个 consumer 都使用
[`orchbee-local-peer-credentials`](../local-peer-credentials/README.zh-CN.md) 把 kernel Unix-socket
peer 的 effective user 与自身比较。两个方向都会检查。Runtime evidence 覆盖 macOS 与 Linux。
Directory 与 socket mode 继续作为纵深防御；持有 path 或有效 byte 绝不能替代 peer
authentication。

Read 与 write 使用有限 frame bound，并拒绝 partial EOF。Server 还提供 finite handshake timeout。
`WorkerAccepted` 之后，OBLS 不再准入 local-site frame。当前 Server adapter 只在一条 request
outstanding 时允许其 serialized OBCN request/result exchange；unsolicited byte 或其他 protocol 会
违反该 carrier 并关闭 Worker attachment。本 crate 不解析或拥有 post-OBLS phase。当前 protocol
version 是私有的，可以随尚未发布 cohort 原子替换。

当前 Server adapter 必须把 OBLS Worker identity 映射为 OBCN 的 nonzero Worker identity。因此，它会
在 admission 或 `WorkerAccepted` 前，把全零 `WorkerHello` identity 拒绝为 `MalformedFrame`，且不消耗
Server generation。这是一项 adapter compatibility precondition，而不是新的 OBLS codec restriction。

## Ownership 与排除项

本 crate 只拥有 encoded local Worker handshake。Server 拥有 Worker admission、generation 与其
post-acceptance carrier；Worker 拥有 connection order；`orchbee-local-peer-credentials` 拥有
same-user proof；Terminal Host 与 Human Client 保持独立 protocol。Client Session、Workbench
semantic、terminal creation/streaming、activation、endpoint discovery、remote transport、
encryption、enrollment、persistence、Windows named pipe 与 compatibility negotiation 均不属于本
crate。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee-local-site-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-local-site-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-site-startup
pnpm verify
```
