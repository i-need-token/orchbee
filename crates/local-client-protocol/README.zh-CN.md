# 本机 Client 协议

这是第一条 same-user POSIX Human Client-to-Server process spine 的私有、实验性 Rust connection
contract 与具体 connecting adapter。它不是 public API、Worker protocol、terminal relay、TUI、
activation mechanism、remote protocol 或 compatibility promise。

简体中文维护译本：本文档。
英文规范：[README.md](README.md)。
冲突时以英文为准。

## Framing 与消息

每条 connection 只承载一条 Client 到 Server 的 handshake frame 和一条 Server 到 Client 的
response frame。准入会结束 `OBLC` 阶段，并且不会消费下一协议阶段的 byte；之后同一 stream
可以只作为 Client Session liveness attachment 保持打开，也可以由调用方用于另一个单独拥有的
semantic protocol。`OBLC` frame 包含固定 magic、unsigned big-endian protocol version、unsigned
big-endian payload length，以及最多 256 byte payload。未知 magic、version、kind、enum value、
缺少或多余 byte、oversized payload、零 generation、无效 UTF-8、空 cohort、非 ASCII cohort，
或超过 64 byte 的 cohort 都会被拒绝。

| 消息             | 字段与含义                                                                                                                  |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `ClientHello`    | 确切 executable cohort。kernel peer credential 提供本机 human identity；renderer form 与 Client-local view state 不在其中。 |
| `ClientAccepted` | Server 为该确切 transport 分配的非零 process-local Client Session generation。它不授予 terminal 或 Workbench capability。   |
| `Rejected`       | protocol/cohort 不兼容、超过容量、错误消息、malformed frame 或 timeout。拒绝不会改变任何已准入 Session。                    |

Human Client 与 Worker contract 保持分离。本 crate 不包含 Worker identity、capability
advertisement、lease、fence、Host、terminal、renderer、Workspace、Tab、Panel 或 semantic-command
字段。

## Connecting adapter

`ClientSessionConnection::connect` 是真实 Rust Client-side consumer。它先在 effect 前验证 cohort，
连接一条由调用方选择的 Unix socket，通过 `orchbee-local-peer-credentials` 证明 Server 具有相同
effective user，应用两秒 read/write handshake timeout，只发送一条 hello，要求一条 accepted
response，然后移除 handshake timeout 并保留 stream。
accepted connection 实现 `Read` 与 `Write`，因此调用方可以成为下一协议阶段的唯一 owner，
不需要打开第二条 socket 或创建第二个 reader。精确、无 buffer 的 handshake read 会保留下一
阶段的 byte，即使 Server 把它们与 `ClientAccepted` 一起写入。丢弃该值只关闭这条 connection。
`set_io_timeout` 允许唯一 caller 为其拥有的下一 phase 应用同一 read/write timeout，而不改变
handshake policy。
对于只做 admission 的调用方，`wait_for_server_loss` 观察 EOF，并拒绝任何 post-admission byte；
使用 semantic phase 的调用方则直接读取该 phase。

adapter 不发现 endpoint、不激活 service、不 retry、不启动 process 或 thread、不渲染 TUI，
也不监督 Server lifetime。公开 `orchbee` 尚未调用它；process test 在不改变 bare、help 或
version 行为的前提下，用它连接真实私有 Server。

## Server 准入与上限

Server 为每条 accepted connection 分配独立 generation，并最多准入八个 active Client Session。
它最多保留十六个 Client candidate/session thread，因此 silent handshake 不会制造无界工作；
超过该 transport bound 的 connection 会被关闭。在 transport bound 内的 candidate 超过八个
Session admission limit 后会收到 typed capacity rejection。每条 handshake 都有两秒 read/write
timeout。

已交付的私有 Server 把 accepted unbuffered stream 保留为 Session carrier，并把 next-phase byte
交给由 [`orchbee-client-server-protocol`](../client-server-protocol/README.zh-CN.md) 单独拥有的 OBCS
decoder。本 crate 既不定义也不解码该 phase。Server 的 deny-only runtime、fault handling 与
generation-scoped cleanup 由其
[`apps/orchbee-server` 指南](../../apps/orchbee-server/README.zh-CN.md) 拥有。

## Ownership 与排除项

本 crate 只拥有 Client framing、handshake validation、connecting adapter，以及把 accepted byte
stream 交给调用方；它既不定义也不解码下一 semantic phase。Server 拥有 listener 与 Client
Session state。`orchbee-local-peer-credentials` 拥有 kernel same-user proof。
`orchbee-local-site-protocol` 单独拥有 Worker handshake。endpoint discovery、lazy activation、
公开 `orchbee` 集成、Workbench snapshot 与 command、terminal relay、controller policy、
persistence、Windows named pipe、remote authentication 与 production service lifecycle 不在本
切片内。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee-local-client-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-local-client-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions,local-client-sessions-obcs
pnpm verify
```
