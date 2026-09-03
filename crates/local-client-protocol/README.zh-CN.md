# 本机 Client 协议

这是第一条 same-user POSIX Human Client-to-Server process spine 的私有、实验性 Rust connection
contract 与具体 connecting adapter。它不是 public API、Worker protocol、terminal relay、TUI、
activation mechanism、remote protocol 或 compatibility promise。

简体中文维护译本：本文档。
英文规范：[README.md](README.md)。
冲突时以英文为准。

## Framing 与消息

每条 connection 只承载一条 Client 到 Server 的 handshake frame 和一条 Server 到 Client 的
response frame，之后保持打开仅作为 Client Session liveness attachment。frame 包含固定 `OBLC`
magic、unsigned big-endian protocol version、unsigned big-endian payload length，以及最多 256
byte payload。未知 magic、version、kind、enum value、缺少或多余 byte、oversized payload、零
generation、无效 UTF-8、空 cohort、非 ASCII cohort，或超过 64 byte 的 cohort 都会被拒绝。

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
response，然后移除 handshake timeout 并保留 liveness stream。丢弃该值只关闭这条 connection。
`wait_for_server_loss` 观察 EOF；因为本切片没有 application message，任何 post-admission byte
都会被拒绝。

adapter 不发现 endpoint、不激活 service、不 retry、不启动 process 或 thread、不渲染 TUI，
也不监督 Server lifetime。公开 `orchbee` 尚未调用它；process test 在不改变 bare、help 或
version 行为的前提下，用它连接真实私有 Server。

## Server 准入与上限

Server 为每条 accepted connection 分配独立 generation，并最多准入八个 active Client Session。
它最多保留十六个 Client candidate/session thread，因此 silent handshake 不会制造无界工作；
超过该 transport bound 的 connection 会被关闭。在 transport bound 内的 candidate 超过八个
Session admission limit 后会收到 typed capacity rejection。每条 handshake 都有两秒 read/write
timeout。

准入后的任何 byte（包括重复 hello）只会关闭该 Session。disconnect 只移除 matching
generation；延迟 cleanup 不能移除后来的 Session。Client disconnect 不停止 Server、Worker 或
任何其他 Client。Server process loss 会关闭所有 liveness stream。

## Ownership 与排除项

本 crate 只拥有 Client framing、handshake validation 与 connecting adapter。Server 拥有 listener
与 Client Session state。`orchbee-local-peer-credentials` 拥有 kernel same-user proof。
`orchbee-local-site-protocol` 单独拥有 Worker handshake。endpoint discovery、lazy activation、
公开 `orchbee` 集成、Workbench snapshot 与 command、terminal relay、controller policy、
persistence、Windows named pipe、remote authentication 与 production service lifecycle 不在本
切片内。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee-local-client-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-local-client-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions
pnpm verify
```
