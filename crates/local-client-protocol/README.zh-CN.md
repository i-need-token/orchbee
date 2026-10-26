# 本机 Client 协议

## 只读 Windows Server 元数据

OBLC v1 类型 6/7 在不变的 256 字节载荷上限内承载一次元数据请求／答复。请求只有类型 6。
类型 7 依次为：版本长度（`u8`）、1–64 字节 ASCII 可见版本、cohort 长度（`u8`）、既有
已验证 ASCII cohort 字节、非零 PID（大端 `u32`）、非零 OBCS 终端版本（`u8`）、模式
（`0` 非托管、`1` 托管就绪、`2` 托管停止中），以及诊断特性位（大端 `u32`）。已知位为
render 行读取（1）、Client view 控制（2）、Agent 观察（4）和 live handoff（8）；未知位
仍是不解释的数据。无效长度／值／模式和尾部字节均解码失败。旧消息编码保持不变。

Windows 时限适配器通过既有命名管道传输验证对端，写入一次请求，读取一个准确答复后
丢弃候选连接，不发送 Client hello，也不进入应用阶段。调用者拥有五秒总预算和取消仲裁；
部分进展不延长时限。只有连接失败保留连接前不存在的来源；连接后 EOF 绝不表示不存在。
Windows Server 在同用户证明之后、cohort 相等及 Client Connection 接纳之前响应，沿用既有候选
及握手上限。该观察不授予操作能力、不创建 Client Connection，也不改变停止状态。不增加 Linux／
macOS 运行时适配器。[CLI 状态契约](../../apps/orchbee/docs/native-runtime-status.zh-CN.md)
拥有公共比较／输出语义和证据，包括旧 Server 不兼容的处理。

## 托管 会话 停止控制

私有 OBLC v1 在现有 256 字节上限内增加类型 4/5，承载同用户、准确 cohort 的 会话 停止请求和状态响应。
请求包含已验证 cohort、非零 128 位停止标识以及封闭的开始、状态或显式重试动作。响应回显该标识，区分未记录、
处理中、失败、另一活动标识和非托管模式。处理中和失败状态保留正数尝试序号（最多 64）和封闭阶段；失败还保留
原因及本次已知释放数量。响应绝不声明进程最终退出：托管所有者回收资源后的已准入 Windows 激活记录才是完成回执。
重复开始不会重试效果；只有同一失败标识的显式重试可以推进尝试。停止中的新 Client hello 收到类型化的停止拒绝。
未知标签、零标识、非法尝试序号和多余载荷字节均解码失败。编解码器自身不启动或停止进程，也不引入 Linux 或 macOS
停止运行时。

这是第一条 same-user POSIX Human Client-to-Server process spine 的私有、实验性 Rust connection
contract 与具体 connecting adapter。它不是 public API、Worker protocol、terminal relay、TUI、
activation mechanism、remote protocol 或 compatibility promise。

简体中文维护译本：本文档。
英文规范：[README.md](README.md)。
冲突时以英文为准。

## Framing 与消息

每条 connection 只承载一条 Client 到 Server 的 handshake frame 和一条 Server 到 Client 的
response frame。准入会结束 `OBLC` 阶段，并且不会消费下一协议阶段的 byte；之后同一 stream
可以只作为 Client Connection liveness attachment 保持打开，也可以由调用方用于另一个单独拥有的
semantic protocol。`OBLC` frame 包含固定 magic、unsigned big-endian protocol version、unsigned
big-endian payload length，以及最多 256 byte payload。未知 magic、version、kind、enum value、
缺少或多余 byte、oversized payload、零 generation、无效 UTF-8、空 cohort、非 ASCII cohort，
或超过 64 byte 的 cohort 都会被拒绝。

| 消息             | 字段与含义                                                                                                                   |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `ClientHello`    | 确切 executable cohort。kernel peer credential 提供本机 human identity；renderer form 与 Client-local view state 不在其中。  |
| `ClientAccepted` | Server 为该确切 transport 分配的非零 process-local Client Connection generation。它不授予 terminal 或 Workbench capability。 |
| `Rejected`       | protocol/cohort 不兼容、超过容量、错误消息、malformed frame 或 timeout。拒绝不会改变任何已准入 Client Connection。           |

Human Client 与 Worker contract 保持分离。本 crate 不包含 Worker identity、Worker capability
advertisement、lease、fence、Host、terminal、renderer、Workspace、Tab、Panel 或 semantic-command
字段。

## Connecting adapter

`ClientConnection::connect` 是真实 Rust Client-side consumer。它先在 effect 前验证 cohort，
连接一条由调用方选择的 Unix socket，通过 `orchbee-local-peer-credentials` 证明 Server 具有相同
effective user，应用两秒 read/write handshake timeout，只发送一条 hello，要求一条 accepted
response，然后移除 handshake timeout 并保留 stream。
accepted connection 实现 `Read` 与 `Write`，因此调用方可以成为下一协议阶段的唯一 owner，
不需要打开第二条 socket 或创建第二个 reader。精确、无 buffer 的 handshake read 会保留下一
阶段的 byte，即使 Server 把它们与 `ClientAccepted` 一起写入。丢弃该值只关闭这条 connection。
`set_nonblocking` 与标准 `AsFd` view 允许同一唯一 owner 实现 absolute readiness deadline，
而不提取、clone 或替换 admitted stream。
对于只做 admission 的调用方，`wait_for_server_loss` 观察 EOF，并拒绝任何 post-admission byte；
使用 semantic phase 的调用方则直接读取该 phase。

adapter 不发现 endpoint、不激活 service、不 retry、不启动 process 或 thread、不渲染 TUI，
也不监督 Server lifetime。Linux 与 macOS 裸 `orchbee` 已消费带截止时间的 adapter；help 与 version 仍无副作用。

## POSIX 有截止时间的连接准入

已接受的懒激活流程需要调用方拥有一个覆盖连接与握手的截止时间。POSIX
ClientConnection::connect_before(endpoint, cohort, deadline) 在 Linux 与 macOS 提供该边界，
apps/orchbee 通过私有 LocalClient::connect_before 消费它。现有 `connect` 保留独立的两秒握手超时。
两个入口都不发现或激活 会话。macOS 原生验证覆盖同一 descriptor、poll、peer proof 与 handshake 路径；
既有 `connect` 入口保持不变。

只有 `ConnectFailed` 报告连接完成前的失败，从而保留拨号错误来源。
托管启动可将其中的 NotFound/ConnectionRefused 当作不存在；连接后相同错误编号属于 `Io`，
不能授权激活。Peer、握手、拒绝和协议失败均不表示不存在。两个变体都不允许 adapter 自行重试。

### 准入建模记录

| 字段         | 契约                                                                                                                                                                                                             |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | 静默或逐步响应的本机 Server 不能延长 Client 启动准入预算。本切片是一次连接尝试，不是完整 会话 激活。                                                                                                             |
| Inputs       | 调用方选定的 endpoint 与绝对单调时钟 Instant；effect 前验证 cohort；不可信的内核连接、就绪、peer 观察与一条 OBLC 响应。                                                                                          |
| Data         | 一个独占的 nonblocking socket、一个不变的截止时间、一条 hello、解码进度与有类型结果。没有 socket clone、进程、锁、重试账本或产品 state。                                                                         |
| Outputs      | 精确的已接受 generation 及同一条 blocking、无缓冲 stream；现有有类型 IO、protocol 或 rejection error。过期携带 TimedOut。失败仅关闭候选连接。                                                                    |
| Invariants   | 一次 connect syscall；同 effective user 证明后才发送 hello；部分进展不续期；接受必须满足精确响应、成功恢复 blocking 与最后的 now < deadline；失败释放候选连接。                                                  |
| Decisions    | 无效 cohort 在 effect 前拒绝。已过期的有效请求不打开 socket。只有 EINPROGRESS 等待连接完成；就绪还须验证 SO_ERROR 为零。Backlog EAGAIN、其他连接错误、错误 peer、IO 错误与无效响应都直接失败且不重新连接。       |
| Model        | 一次带期限的准入尝试依次经过连接、对端证明、部分 hello/response I/O 与阻塞模式恢复。受控实现测试在每个边界注入完成与过期。                                                                                       |
| Transitions  | 已验证请求 → connecting → peer proof → writing → reading → decoded response → blocking restoration → accepted。无效输入、IO、peer 或响应失败，或观察到过期，都进入 failed 并释放候选连接。                       |
| Dependencies | POSIX 原子 SOCK_CLOEXEC/SOCK_NONBLOCK、已有固定版本 Nix socket/poll wrapper、内核 peer credential、单调时钟与不变的 OBLC codec。                                                                                 |
| Ownership    | 本 crate 拥有准入与候选连接清理；调用方选择截止时间并拥有已接受 stream。Server 拥有自身 Client Connection，并独立观察连接丢失。                                                                                  |
| Verification | 确定性部分 I/O 与期限边界测试、Linux 与 macOS Unix-socket 接受/拒绝/静默/EOF/backlog 案例、下一阶段字节保留及既有连接回归。                                                                                      |
| Correctness  | AD-01–AD-04 映射到下方具体适配器证据。本地期限观察提供有界失败，不假设外部连接/就绪/响应推进。不保留独立准入 TLA 模型。                                                                                          |
| Limits       | 仅一次 POSIX 准入，不证明 service readiness、激活、OS 进程调度或 Server 清理时长。Adapter 无法抢占内核调用或调度，只能在可控制的边界及返回成功前检查截止时间。Windows 使用命名管道 adapter；不选择生产超时数值。 |

| Claim                                                           | 实现与可执行证据                                                                        |
| --------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| AD-01：无效或已过期准入没有 socket effect；一次尝试不重新连接。 | connect_before；无效、已过期及 Linux/macOS backlog 测试。                               |
| AD-02：peer proof 先于一条 hello 与一条精确、无缓冲响应。       | 现有 credential 边界与 codec；同 stream OBCS 与合并下一阶段字节测试。                   |
| AD-03：一个总截止时间覆盖部分进展与最终接受。                   | 带截止时间的 poll/read/write 与最终检查；步进时钟写、读及最终边界测试。                 |
| AD-04：失败仅关闭候选连接；成功返回已接受的 blocking stream。   | 静默 Server EOF、有类型负面响应与 blocking mode 测试；现有调用方保留 stream ownership。 |

### 可取消准入边界

`connect_before_cancellable(endpoint, cohort, deadline, wake)` 是同一准入外加一个可选的
borrowed wake descriptor。`None` 与 `connect_before` 完全一致，absolute deadline 永不续期。
该 descriptor 只是唤醒能力：本 crate 从不 own、duplicate、read、drain 或 close 它，从不改动
signal mask 或 disposition，也从不解释它为何可读。`POLLIN` 只是 wake candidate；
无可读 byte 时的 `POLLERR`、`POLLHUP` 或 `POLLNVAL` 表示 wake channel 已损坏，并且 wake
event 先于任何同时就绪的 socket readiness 判定。私有观察在边界上映射为
`ConnectClientError::CancellationReady` 与 `ConnectClientError::CancellationSourceFault(error)`。
两者都不是取消结论，也都不得按 marker 文本读取；由调用方自己的 signal owner 执行唯一的
仲裁读并决定结果，包括唤醒源不可达
或故障的情形。`connect` 与 `connect_before` 不会产生这两个变体。

## Server 准入与上限

Server 为每条 accepted connection 分配独立 generation，并最多准入三十二个 active Client Connection。
它最多保留四十八个 Client candidate/connection thread，因此 silent handshake 不会制造无界工作；
超过该 transport bound 的 connection 会被关闭。在 transport bound 内的 candidate 超过三十二个
Client Connection admission limit 后会收到 typed capacity rejection。每条 handshake 都有两秒 read/write
timeout。

已交付的私有 Server 把 accepted unbuffered stream 保留为 Client Connection carrier，并把 next-phase byte
交给由 [`orchbee-client-server-protocol`](../client-server-protocol/README.zh-CN.md) 单独拥有的 OBCS
decoder。本 crate 既不定义也不解码该 phase。Server 的本机主体策略、fault handling 与
generation-scoped cleanup 由其
[`apps/orchbee-server` 指南](../../apps/orchbee-server/README.zh-CN.md) 拥有。

## Ownership 与排除项

本 crate 只拥有 Client framing、handshake validation、connecting adapter，以及把 accepted byte
stream 交给调用方；它既不定义也不解码下一 semantic phase。Server 拥有 listener 与 Client
Client Connection state。`orchbee-local-peer-credentials` 拥有 kernel same-user proof。
`orchbee-local-worker-protocol` 单独拥有 Worker handshake。endpoint discovery、lazy activation、
公开 `orchbee` 集成、Workbench snapshot 与 command、terminal relay、controller policy、
persistence、Windows named pipe、remote authentication 与 production service lifecycle 不在本
切片内。

## 验证

从仓库根目录运行：

```sh
cargo test -p orchbee-local-client-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-local-client-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --workspace apps/orchbee
node scripts/tla/run-models.mjs --models local-session-startup
pnpm verify
```

## Windows 准入

Windows 使用 `orchbee-local-ipc` 本地命名管道承载相同的 OBLC 消息。适配器在发送 hello 前证明对端令牌用户，连接、读取和写入共用调用方的绝对截止时间，并通过借用的就绪源观察取消。它不启动或重试 会话。管道名称和权限归运行时与传输所有者负责；OBCS/OBWP 分帧不变。

10 毫秒的取消检查节奏只限制尚未连接时的内核等待，不是已认证连接的截止时间。
管道在该切片后、调用方原始期限前完成打开和对端证明，仍会继续发送唯一一条 hello。
只有传输层有类型的连接前等待结果允许再等待。拨号错误保留为 `ConnectFailed`；
打开管道后的错误保留为 `Io`，因此认证失败和已连接 EOF 都不能被重新解释为连接前缺席。
认证后遇到取消或总期限耗尽，会在 hello 前释放候选，不再次拨号、不延长总预算。
元数据和 会话-stop 使用相同连接边界。不重试任何请求或效果。

`src/connection/windows_admission/tests.rs` 控制完成顺序与时钟推进，覆盖跨切片、等待时
取消、连接后失败、迟到完成和候选清理。原生元数据／停止测试覆盖真实管道适配器。
这些具名轨迹补充既有协议模型，不建模或限制 Windows 内核调度。

<!-- documentation-pairing: english-sha256=3d11c3c701166230b7c6b47f39bc26fdae0abdaa67833fe4e64156c7cbc87a33 zh-cn-sha256=db1a27f469c7570b12d475882ba0270af55337b51bb4870ded46e4c46f5ace1e -->
