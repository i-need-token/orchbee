# Capability Node Protocol

这是 Orchbee Server 与 Capability Node 之间 authorized terminal operation 的私有实验性 Rust
semantic contract。首个 Capability Node 是 Worker，但本 crate 不会把 message 绑定到本机 socket
或任何其他 carrier。本机 Server adapter 现在会在 application-private post-OBLS carrier 中消费其
codec，但尚无 Server business coordinator 或当前 Worker runtime end-to-end 消费 request。它不是
public API 或 compatibility promise。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

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
缺省与空值不同。Geometry 包含 column、row、cell-pixel width 与 cell-pixel height。每个值均在
`1..=4096` 内，grid 最多 262,144 个 cell，且每条 total pixel extent 都可放入 `u16`。这些 type
是 Capability Node refinement，不 import 或 re-export Terminal Host wire payload；未来 Worker
adapter 必须显式转换并保留 validation。

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
reconcile。Server 拥有未来 product authorization 与 reconciliation；Capability Node adapter 拥有
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
model 是这个尚未 end-to-end 消费 contract 的 Server-owned behavioral authority。它探索 current 与 stale
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
