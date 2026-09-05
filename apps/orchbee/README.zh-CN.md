# Orchbee 原生入口

这是已接受 native-command foundation 与 local-terminal milestone 的 experimental、尚未发布 Rust
process entry、application-private 本机 Client transport 与 rendezvous primitive，以及已检查的
Client-side 本机 Site activation refinement，以及已暂存但尚未通过入口调用、带已连接单终端 adapter 的 TUI shell。产出的
executable 名为 `orchbee`。它参与根 Cargo verification gate，但不被 Desktop、packaging、installer
或 package manager 消费。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## 命令面

确切 development surface 是：

```text
orchbee
orchbee --help
orchbee --version
```

裸调用选择未来 TUI route，并报告本地化的 development-unavailable result。它不检查 terminal
device、不进入 terminal mode、不注册 signal，也不渲染已暂存的 shell。`--help` 与 `--version`
保持完全一致的输出，同样无需初始化 TUI 或 backend 就会完成。其他、额外或非 Unicode argument
都是 usage failure。不存在 short flag、alias、abbreviation、业务命令、target discovery、
current-directory inference 或 compatibility behavior。

Help 只写 stdout。Unavailable 与 usage result 只写 stderr。Version 向 stdout 写一行与 locale
无关的 `orchbee <version>`。Development exit status 仍为：help 与 version 为 `0`，不可用的裸 TUI
route 为 `1`，usage failure 为 `2`。这些值不是未来 stable public exit table。

## 私有本机 Client transport

在 Unix 上，application-private `LocalClient` 只从 internal caller 接收 endpoint 与 executable
cohort。它把 admission 委托给 `orchbee-local-client-protocol`：该 crate 在连接前验证 cohort，证明
Server 与当前进程具有相同 effective operating-system user，以两秒 read/write timeout 执行确切
`OBLC` handshake，并返回 accepted generation 与同一条无 buffer stream。这些既有 handshake
timeout 仍是独立的 OBLC boundary；本切片不为 connect 加 handshake 增加单一 total deadline。
application 在进入 `OBCS` phase 前把 admitted stream 切换为 nonblocking I/O。

`submit_terminal_create`、`submit_terminal_start`、`submit_terminal_viewer` 与 `submit_terminal_control` 各自取得一条已经验证的 typed request 的
ownership，并在提交前 encode 完整 OBCS frame。Encoding failure 是 definite failure，并保留
Session。Encoding 成功后启动一个五十秒 monotonic deadline，覆盖完整 frame write 与一次 Server-frame
read；每次 readiness wait 都重新计算剩余 budget，因此 partial progress 不会续期。第一次 write
syscall 之前的 failure 是 definite failure。该 syscall 一旦开始，任何 write 或 read failure、
deadline expiry、malformed 或 incompatible result、错误 result kind 或错误 correlation，都会使
effect outcome 成为 typed `OutcomeUnknown` 并让 connection 退役。transport 绝不 retry，也不返回
request 供 replay。Server 返回的正确 correlated `outcome-unknown` 是有效 OBCS result，而不是
transport ambiguity。

当前 invocation 不会到达该 transport。私有已连接 TUI 现已消费已准入 `LocalClient`、分配新的
request identity 并提交 Create、查看和控制意图。Endpoint selection/discovery、activation、
reconnection、reconciliation 与公开入口接线仍是独立边界。仅允许关闭的 `LocalClientShutdown`
handle 让 TUI 可以停止阻塞的 exchange，而不获得额外 stream reader 或 writer。

五十秒提交上限容纳 Server 一个排队及一个在途 Worker exchange（各自最多两秒 write 加二十秒
read）和 Client 结果交付。Handshake 限制保持独立。查看提交验证 action/outcome 兼容性与精确
correlation，并保留 opaque OBSR byte 供已连接的 projection consumer 使用；它不授予 controller 权限，
也不执行终端呈现解码。控制提交针对其 action 验证精确 grant 获取、input acknowledgement 和
完整 resize geometry。Server 报告的 unknown control outcome 原样返回且不重试；不匹配的 outcome
让 transport 退役。Caller 拥有新的 request identity、绑定 grant 的递增 sequence 和后续显式获取。
Session 丢失后的 cleanup 可超出单次提交 deadline。

### 提交正确性记录

| 字段   | 契约                                                                                                                                                                                                                                                                                                                                    |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 持有 already admitted same-user Session 的 internal Client 必须提交一条 typed Create、Start、查看或控制 request，并在 transport completion 含糊时避免重复 effect。Endpoint acquisition、process activation 与 presentation 不在范围内。                                                                                                 |
| 输入   | Trusted typed `TerminalCreateRequest`、`TerminalStartRequest`、`TerminalViewerRequest` 或 `TerminalControlRequest`、admitted `ClientSessionConnection`、accepted generation、finite request budget 与 monotonic time。Server byte 与 readiness outcome 不可信。connection 缺失表示 `SessionUnavailable`。                               |
| 数据   | 一次 invocation 拥有一个 `requestId`、encoded frame、absolute deadline、connection state、possible-handoff boundary、expected result kind 与 decoded result class。`LocalClient` 独占其 Session。                                                                                                                                       |
| 输出   | Exact correlated typed result；definite `RequestEncoding` 或 `RequestNotSubmitted`；带 submitted identity 的 typed `OutcomeUnknown`；或 `SessionUnavailable`。不存在 cancellation、retry、replay return 或 optimistic terminal mutation。                                                                                               |
| 不变量 | Encoding 先于 deadline 与 transport；一次 invocation 至多开始一次 frame handoff；只有 exact kind 与 identity 才完成；possible handoff 后的每种 ambiguity 都让 Session 退役；retired Session 不发送 byte。                                                                                                                               |
| 决策   | 第一次 possible handoff 前或后；write 未完成或完成；result 在 deadline 前或等于/晚于 deadline；exact 或错误 kind/identity；有效或 malformed frame；known 或 Server-reported unknown outcome。若 acceptance 尚未完成，deadline observation 胜出。                                                                                        |
| 模型   | `LocalClientSubmission` 是 bounded one-invocation state refinement。`LocalClient::exchange` 是 implementation workflow；其 deadline-aware nonblocking stream wrapper 是 time 与 I/O adapter。                                                                                                                                           |
| 转换   | 下方 transition record 定义 serializable abstract phase 与 guard。Runtime phase 是 call-local；只有 live-or-retired connection state 跨 call 保留。                                                                                                                                                                                     |
| 依赖   | 一条 already admitted Unix stream、monotonic time、readiness polling、OBCS encoding/decoding 与 same-stream OBLC admission。OBCS workflow 没有 filesystem、process、endpoint-selection、randomness、localization 或 UI capability。                                                                                                     |
| 所有权 | `LocalClient` 是唯一 stream 与 retirement owner。OBCS 拥有 frame 与 result semantics；OBLC 拥有 admission、credential、cohort、generation 与既有 handshake timeout。                                                                                                                                                                    |
| 验证   | Fake-Server test 覆盖 exact Create/Start、大 OBSR record、不兼容查看 outcome、精确控制 outcome 和 resize geometry、correlation、protocol 与 EOF failure、silent timeout、deterministic partial read/write deadline exhaustion、pre-write expiry、retirement 与无 duplicate send。TLC 要求每个 registered semantic action 都有 witness。 |
| 正确性 | 下方 LC-01 至 LC-05 把 implementation branch 与 deterministic test refine 到 registered model。TLC 对一条 request 与一个 Session 穷举 49 个 bounded state；它不证明 OS scheduling、codec correctness、OBLC connect timing、activation、reconnection、reconciliation 或未来 TUI behavior。                                               |

### 提交转换记录

| 当前 phase                         | Event 与 guard                                                 | 下一 phase         | Result 与 connection effect                                          |
| ---------------------------------- | -------------------------------------------------------------- | ------------------ | -------------------------------------------------------------------- |
| `ready`                            | Session 存在时提交                                             | `encoding`         | 无 transport effect                                                  |
| `encoding`                         | Frame encoding 失败                                            | `definite-failure` | `RequestEncoding`；Session retained                                  |
| `encoding`                         | Frame encoding 成功并创建一个 absolute deadline                | `write-ready`      | Encoded frame 仅属于本次 invocation                                  |
| `write-ready`                      | 第一次 write syscall 前 deadline 或 I/O setup 失败             | `definite-failure` | `RequestNotSubmitted`；Session retained                              |
| `write-ready`                      | 第一次 write syscall 可能向 kernel 移交 byte                   | `write-begun`      | Handoff count 变为一                                                 |
| `write-begun`                      | 唯一完整 frame 在 expiry 前写完                                | `awaiting-result`  | 在同一 deadline 下等待恰好一条 Server frame                          |
| `write-begun` 或 `awaiting-result` | Deadline、I/O、timeout 或 EOF 胜出                             | `retired`          | `OutcomeUnknown`；connection closed                                  |
| `awaiting-result`                  | Decoded frame 在 expiry 前为 malformed、wrong-kind 或 wrong-ID | `retired`          | `OutcomeUnknown`；connection closed                                  |
| `awaiting-result`                  | Exact correlated result 已 decode 且 deadline 仍 open          | `completed`        | 返回 typed known 或 Server-reported-unknown result；Session retained |
| `retired`                          | 后续任何 submit call                                           | `retired`          | `SessionUnavailable`；不 write 且不 retry                            |

### 正确性可追溯性

| Claim                                                                                                           | Implementation 与 deterministic witness                                                                                                                                   | TLA+ evidence                                                                                                   |
| --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| LC-01：encoding 与第一次 write 前的每种 failure 都是 definite。                                                 | `exchange` 在启动 `SubmissionDeadline` 前 encode；`deadline_expiry_before_the_first_write_is_a_definite_failure` 观察到 Server 收到零 byte 且 generation 保留。           | `EncodingFails`、`PreWriteDeadline`、`PreWriteIoFails`；`DefiniteFailurePrecedesHandoff`。                      |
| LC-02：一次 invocation 至多有一次 request-frame handoff，且绝不 retry。                                         | 一个 encoded buffer 只传给 `write_all` 一次；slow-write 与 silent-timeout test 拒绝第二条 request，后续 call 返回 `SessionUnavailable`。                                  | `WriteBegins`；`AtMostOneHandoff`；`NoSendAfterRetirement`。                                                    |
| LC-03：possible handoff 后的每种 failure 都是 unknown，并让 Session 退役。                                      | Write/read、EOF、incompatible frame、wrong kind 与 wrong identity branch 都调用 `retire_with_unknown`；focused test 断言 generation 消失。                                | `WriteFails`、`WriteDeadlineExpires`、`ReadFails`、全部 retire action；`UnknownRequiresHandoffAndRetirement`。  |
| LC-04：只有 exact kind 与 identity 才完成；correlated Server `outcome-unknown` 是数据而非 transport ambiguity。 | Exact Create/Start、wrong-kind、wrong-ID 与 correlated-unknown fake-Server test 覆盖 result classifier。                                                                  | Matching 与 wrong-result arrival action；`AcceptedResultIsExactAndBeforeDeadline`。                             |
| LC-05：一个 monotonic deadline 覆盖完整 write 与 read，progress 不会续期。                                      | `SubmissionDeadline` 只创建一次；每个 readiness boundary 重新计算余量。Stepping-clock test 在 read 与 write 两侧制造 sub-deadline byte progress，直到 total budget 用尽。 | `WriteDeadlineExpires`、`ResultDeadlineExpires`、`RetireTimedOutResult`；`TimeoutCannotProduceAcceptedResult`。 |

## 已暂存的 TUI shell 底座

以下 application-private module 会编译和测试，但当前 invocation 不会进入它们。未连接的 `run`
adapter 保留下面的 unavailable shell 行为；`run_connected` 组合下一节已接受的终端细化：

| 关注点      | 当前契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 准入        | 已暂存的 Unix adapter 要求继承的 stdin 与 stdout 都是 TTY，且具有相同 terminal-device identity；它只复制这些已准入 descriptor，并从复制后的 stdout descriptor 读取 cell 与 pixel dimension。它没有 `/dev/tty`、Crossterm global size 或 process fallback。                                                                                                                                                                                                                                                                                                                                                       |
| 生命周期    | 一个已暂存 guard 拥有 raw mode、alternate screen 与 cursor visibility。它会在 operation 可能部分成功前标记 cleanup responsibility，在 inverse 成功前保留每个 flag，并为每个失败的 inverse 最多尝试两次，随后返回一个 aggregate error。Process-lifetime singleton panic hook 只在发生 panic 的 thread 是 active guard owner 时派发，因此其他 thread 无法在保存 terminal state 前清除 setup lease；guard drop 不会移除后来安装的 hook。正常返回、initialization/event/draw failure、owner-thread panic，以及 `SIGHUP`/`SIGINT`/`SIGQUIT`/`SIGTERM` 中的每一种，都有确定性 restoration 或 signal-observation 证据。 |
| Client View | 每个 invocation 拥有 prefix mode、viewport class、sidebar projection、active overlay 与 transient notice。它不拥有任何共享 Tab、Panel、Workspace、Site 或 Terminal fact，也不持久化任何内容。                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 布局        | 96 列及以上时，30 列 Orchbee business sidebar 停靠在 Workbench region 左侧。更窄 viewport 让 Workbench 占满宽度，sidebar 只作为左侧 overlay 显示。Workbench projection 包含 Tab strip 与 Panel/Terminal region，但会将其标记为不可用，而不会伪造共享数据。                                                                                                                                                                                                                                                                                                                                                       |
| 输入        | 已暂存 adapter 每次最多从已准入 stdin descriptor poll 并读取 4,096 个 raw byte。每次完整 read 都会先依序 dispatch，之后才允许后续 signal、viewport query 或 resize decision 结束本轮 loop。`Ctrl+B` 进入可见 prefix mode。`b`、`w`、`?`、`q` 与 `d` 只驱动本地 shell state 或退出。Typed Tab 与 Panel action 保持不可用。在 prefix mode 外，所有 byte——包括 `Ctrl+C`、`Ctrl+D`、escape sequence、UTF-8，以及 overlay 可见时的输入——都会依序到达可注入 terminal-input port；prefix 后的 `Ctrl+B` 会 forwarding 一个 literal byte。                                                                                |
| 结果        | 当前没有 command 进入此 shell。此 seam 不宣称 Server、Client Session、真实 terminal、detach、attachment、retained-terminal、activation、Workbench mutation 或 Site-lifetime behavior。                                                                                                                                                                                                                                                                                                                                                                                                                           |

确定性 Client View transition model 如下：

| 状态与事件                                     | 下一状态、命令与可见结果                                                                                                                                                                                             |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Normal + `Ctrl+B`                              | 进入 Prefix；不发出 terminal input；渲染 prefix indicator。                                                                                                                                                          |
| Normal + 其他 byte，无论 overlay 是否存在      | 保留本地 view state，并将确切、有序的 byte 提供给 terminal-input port。Unavailable port 会在无副作用的情况下拒绝，已暂存 renderer 会报告 input 未发送。                                                              |
| Prefix + `b`                                   | 离开 Prefix，并为此 Client View 切换停靠的宽屏 sidebar 或窄屏 sidebar overlay。                                                                                                                                      |
| Prefix + `w` 或 `?`                            | 离开 Prefix，并在本地打开 Workbench navigator 或 shortcut-help overlay。                                                                                                                                             |
| Prefix + `c`、`n`、`p`、`1` … `9` 或 `h/j/k/l` | 离开 Prefix，保持共享状态不变，并把映射后的 backend-dependent action 显示为不可用；不提交 command。                                                                                                                  |
| Prefix + `Ctrl+B`                              | 离开 Prefix，并向 terminal-input port 提供一个 literal `Ctrl+B` event。                                                                                                                                              |
| Prefix + `q` 或 `d`                            | 离开 Prefix，并关闭这个 unavailable shell。                                                                                                                                                                          |
| Prefix + `Esc`                                 | 取消 Prefix，且不改变 overlay。在 Prefix 外，`Esc` 及其后续 byte 都是普通 terminal input。                                                                                                                           |
| Resize                                         | 查询已准入 stdout descriptor，更新 fixed backend viewport，并且只重新计算 Client-local viewport class。进入 narrow mode 会隐藏停靠 sidebar；回到 wide 会关闭 sidebar overlay，并恢复此前的 wide-sidebar preference。 |
| Event/draw error、panic 或 termination signal  | 停止 loop，并尝试恢复 cursor、alternate-screen 与 raw-mode，不启动或停止其他 process。                                                                                                                               |

不变量是：每个 prefix sequence 至多产生一个 typed command；非 prefix byte 保持顺序；未连接 adapter 绝不改变 shared Workbench 与 terminal state；每个可能取得的 terminal mode 都会
保留一个有界 restoration owner。Unit test 覆盖 state、input-port、fixed-backend、layout、renderer、
真实 PTY 的 same-terminal admission、实际 panic hook、全部四种 signal、cross-thread setup
interleaving、post-read viewport/resize failure 与 lifecycle fault injection。Native PTY process test
则分别证明当前 bare/help/version path 不改变 terminal mode、Linux PTY master 在最后一个 slave
关闭后的 `EIO` 是正常 hangup，并且 linked public binary 不包含 `/dev/tty` 或 `tput` fallback token。
已暂存 runtime 及其全部 third-party dependency 仅支持 Unix；Windows target dependency tree 让当前
one-shot CLI 不包含这些 dependency，且不宣称 Windows TUI runtime 或 signal-restoration 支持。

## 已连接单终端 TUI 细化

已接受的本地终端里程碑让私有 TUI 成为 `LocalClient` 的实际消费者：`run_connected` 接收已准入
Client 和显式创建或附着意图。它不选择 endpoint、激活服务、改变公开语法或发布共享 Workbench
事实。真实进程／PTY 见证已经经此 adapter 呈现 Server/Worker/Host 终端；裸入口 rendezvous 和
lazy activation 仍是后续缺失的集成边界。

| 字段   | 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | Shell 已有输入和生命周期端口，但不能显示或控制保留终端。接入一个 Session，同时保留及时退出、有界队列、精确反馈和终端生命周期。                                                                                                                                                                                                                                                                                                                                                                            |
| 输入   | 已准入 LocalClient、可选默认 Shell cwd 或已接受 create 引用、继承终端的 geometry 和原始输入、不可信 OBCS/OBSR 响应，以及终止信号。                                                                                                                                                                                                                                                                                                                                                                        |
| 数据   | 一个拥有连接的 task、八条排队 command（每条 input 最多 4,096 byte）、一个 response slot、一个待发 frame feedback、一个有界文本投影、新 request ID 和一个 controller sequence。只有 Host 拥有 VT 和 history。获取已有终端也提交初始 panel geometry；input 与后续 resize 共用同一递增 sequence。                                                                                                                                                                                                            |
| 输出   | Create/attach 和显式 acquire、按序准入的原始输入和 geometry、文本／style／cursor 呈现、精确 applied feedback 或 NeedSnapshot、本地化连接中／只读／blocked／失败状态，以及仅针对 Session 的清理。                                                                                                                                                                                                                                                                                                          |
| 不变量 | UI 线程不执行 Server I/O。Task 串行交换且不重试副作用。队列已满的输入被明确判定为未提交。Unknown outcome 关闭 task 并丢弃未发送 command。没有 task 超过其 TUI owner 的生命周期。                                                                                                                                                                                                                                                                                                                          |
| 模型   | Opening -> 已附着 controller 或只读 viewer -> polling -> 收到 record -> 原子文本投影提交 -> draw -> feedback -> polling。控制意图与单一 projection flight 分开排队。关闭会 shutdown 已准入 socket 并 join task；Server 拥有按代次划分的 detach cleanup。                                                                                                                                                                                                                                                  |
| 决策   | Snapshot 替换全部行且不回退已提交 revision（包括 projection reset 后）；delta 要求精确 attachment、epoch、sequence、generation、base revision、screen 和 geometry。无效内容请求新 snapshot 且不应用任何行。错误 envelope 或 scope 让 Session 退役。ProjectionBlocked 清除呈现，直到更高 revision 的 snapshot。没有自动提升 controller、reconnect 或 unknown input 重放。                                                                                                                                  |
| 依赖   | 既有 OBCS 和 Rust Terminal Protocol binding、有界 channel、POSIX socket shutdown、已准入 descriptor、现有 Ratatui renderer 和本地化文案。不增加三方依赖、直连 Host 路径或 VT 模拟器。                                                                                                                                                                                                                                                                                                                     |
| 所有权 | `terminal_session` 拥有连接 task 和输入准入；`terminal_projection` 拥有 Client 文本状态和反馈；Terminal Protocol binding 拥有有界 OBTF 文本解码；shell 拥有 mode 与 draw 顺序。                                                                                                                                                                                                                                                                                                                           |
| 验证   | Canonical OBTF fixture、畸形／边界分区、确定性的 snapshot/delta 和队列／退役轨迹、render buffer 断言，以及真实进程／PTY 见证。既有 LocalClientSubmission 和 LocalTerminalRelay 映射 transport 退役及 projection/control 顺序；其有限边界不是性能保证。                                                                                                                                                                                                                                                    |
| 限制   | 投影文本 cell 和基本 style/cursor；排除 graphics、hyperlink 激活、history、selection、terminal interaction-mode negotiation、cursor shape 对齐和完整 Workbench 行为。不支持的 cell width 使用替代字形。缺失或无效的继承 cell/pixel geometry 会在 Create 或初始 attach 前拒绝，或经恢复边界结束活动 TUI；绝不编造 pixel size。Optional extension 内容保持 opaque，文本 decoder 不保留它们。投影按共享文本分配计数，在提交前拒绝超过 36 MiB 的保留文本。显示 cell 中的控制字符绝不成为终端 escape command。 |
| 审查   | 当前上下文内的挑战检查覆盖内存放大、至多一次副作用、draw 失败反馈和 join 取消；不构成独立审查证据。完整里程碑验收及此前报告的 authority model 超时仍未关闭。                                                                                                                                                                                                                                                                                                                                              |

## 本地化与权威

Authored help 与 diagnostic 同时提供 `en-US` 和 `zh-CN`。Locale selection 依次检查非空
`LC_ALL`、`LC_MESSAGES` 与 `LANG`；以 `zh` 开头的 language prefix 选择 `zh-CN`，其他 value
均 fallback 到 canonical `en-US`。Locale data 只影响 presentation，绝不会持久化或用作 authority。

三个已交付 invocation route 只使用 Rust standard-library argument、environment、stdout 与
stderr capability。它们不会选择任一 private module、不打开 file 或 socket、不启动 process 或
thread、不初始化 terminal mode，也不读取 product state。local Client transport 与 rendezvous
primitive 会在 Unix 上编译，但 public dispatch 无法到达它们。在另行接受的 packaged cutover 前，
已交付 TypeScript [`apps/cli`](../cli/README.zh-CN.md) 继续作为当前 CLI 权威。

## POSIX 本机 Site rendezvous 底座

application-private 的 `local_site_rendezvous` module 为后续已接受 activation workflow 暂存了一个
经过 mode 检查的 macOS/Linux rendezvous primitive。未来 installation composition 必须提供稳定的
`SiteScope`；cohort version 与 digest 不属于该 scope，因此 update 会保留同一个 Site identity，
release、local 与 checkout-scoped development installation 则可以彼此隔离。`SiteScope` 接受
`[a-z][a-z0-9]*(?:-[a-z0-9]+)*` canonical form 的 1 到 63 个 ASCII byte，因此不能加入 path
separator、dot component 或依赖平台的 encoding。

对于一个可信、已存在且为 exactly normalized absolute 的 per-user runtime base，确切 derived
path 是：

| 角色               | 稳定路径                                         |
| ------------------ | ------------------------------------------------ |
| Scope directory    | `<runtime-base>/<site-scope>`                    |
| Activation lock    | `<runtime-base>/<site-scope>/activation.lock`    |
| Site lifetime lock | `<runtime-base>/<site-scope>/site-lifetime.lock` |
| Client socket      | `<runtime-base>/<site-scope>/client.sock`        |
| Worker socket      | `<runtime-base>/<site-scope>/worker.sock`        |

`ModeCheckedLocalSiteRendezvous` 刻意在名称中标明尚未完整的 security boundary。Preparation 只会在
scope directory 不存在时创建它。Preparation 与 validation 把 `/` 作为 trusted root 打开，然后用
`O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC` 逐个 descriptor-relative 地打开 runtime-base component 与
受控 scope。它们既不 canonicalize，也不让 operating system 对传入 base 执行 normalization。除 `/`
本身外，可接受 base 只含一个 leading slash、由单个 slash 分隔的一个或多个 nonempty normal
component，不含 trailing slash、`.` 或 `..` component，也不含 NUL byte。Malformed base 在 trusted
root 之外的 filesystem access 发生前被拒绝；symbolic-link 或 non-directory component 会在该
component 处被拒绝。保留的 scope 必须是 effective UID 拥有且 exact mode 为 `0700` 的 directory。
新 scope 会在不 follow leaf 的情况下接受检查，验证其 directory type 与 effective-UID ownership，在
依赖 permission 的 open 之前以 descriptor-relative 方式改为 `0700`，然后仅在打开 descriptor 的
device 与 inode 和 chmod 前检查的 entry 一致时才 admission。即使 process umask 移除了全部请求的
permission，这也能在不改变 process umask 的情况下达到 exact `0700`。已有的错误 mode 会被拒绝且
不会修复。

每项 lock operation 都通过保留的 directory descriptor，以
`O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` 只打开或创建固定 leaf。Leaf 必须是 effective UID 拥有且
exact mode 为 `0600` 且 hard link 数恰好为一的 regular file。已有且有效的 stale file 会被复用，
不会被 truncate 或 replace。`try_acquire_activation_lock` 与 `try_acquire_site_lifetime_lock` 各自只执行一次
nonblocking exclusive kernel lock attempt，并返回 owned guard 或 `Contended`；它们不会 wait、
retry 或推断 process health。File creation 与 advisory-lock acquisition 是分开的：返回 `Contended`
的 caller 可能先创建并规范化了此前缺失的 durable lock inode，之后另一个 descriptor 才赢得 lock。
Contended attempt 绝不会 truncate、replace、unlink 或改变 existing inode 的 ownership，也不会修复
pre-existing invalid entry。Drop guard 只释放其 advisory lock。该 module 绝不移除 runtime
directory、durable lock file、socket path 或这些 path 上的任何 entry。

在 inherited 或 extended ACL 可以授予 mode bit 之外访问权限的 filesystem 上，这个底座尚未建立
production-private runtime boundary：它既不检查也不移除 ACL。在具备 ACL 能力的 boundary 补齐
这一缺口之前，public activation 必须保持 unwired。它也不选择 runtime base 或 scope，不验证
Unix-socket path length 或 socket metadata，不 bind、connect、认证 peer、启动 process、等待
readiness、unlink endpoint，也不声称 stale-process recovery。

该 primitive 仍由 `apps/orchbee` 拥有，因为本切片中只有 public Client-side activation boundary
具备已接纳的需求；不存在第二个 source consumer。
`apps/orchbee-server` 不能导入另一个 application，而且尚未获取 `site-lifetime.lock`；后续 Server
lifecycle slice 必须先接纳一个真实 shared owner 或 explicit composition seam，两个 application
才能使用同一 contract。

### 建模与证据

| 字段         | 当前暂存 contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | 为后续 local Client activation workflow 提供一个 path-component-safe、same-effective-UID、经过 mode 检查的 rendezvous identity，同时不提前实现 process activation、readiness、cleanup 或 recovery。                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Inputs       | 一个可信、已存在且为 exactly normalized absolute 的 runtime base；在 63-byte ASCII boundary 解析的 installation-owned scope string；effective UID；以及 adversarial directory 和 lock entry。Relative 与 non-normal base、scope 缺失、link、错误 type/owner/mode、stale file 与已持有 lock 都是显式 case。                                                                                                                                                                                                                                                                                                                                                       |
| Data         | Canonical `SiteScope`、确切 derived path、一个保留的 scope-directory descriptor，以及每个 acquired advisory lock 的一个 owned file descriptor。不包含 PID、readiness marker、process identity、durable record payload 或 socket state。                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Outputs      | Preparation 或 validation 返回保留的 rendezvous 或 typed failure。每次 lock attempt 返回 `Acquired`、`Contended` 或 typed filesystem failure。Client 与 Worker socket method 返回确切 stable path且不执行 I/O。                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Invariants   | 每个 runtime-base component 与被打开的受控 directory 或 lock entry 都使用 no-follow；新初始化 scope 的 opened identity 与 chmod 前检查的 entry 一致；已接受 entry 在 admission 时具有 exact type、effective-UID ownership、mode 与 lock link count；已接受的 activation 与 lifetime leaf 在 admission 时无法 alias 同一个 inode；contended lock attempt 绝不 truncate、replace、unlink、改变 existing inode 的 ownership 或修复 pre-existing invalid entry，但可以创建并 mode-normalize 缺失 lock inode；drop resource 不移除任何内容；scope identity 独立于 executable cohort。                                                                                 |
| Decisions    | Exact normalized-base syntax、每个 descriptor-relative base-component result、scope syntax 与 length、create-versus-validate policy、descriptor metadata、lock kind、create-versus-existing lock file，以及一次 kernel lock outcome 完全决定结果。                                                                                                                                                                                                                                                                                                                                                                                                               |
| Model        | 一个纯粹且有界的 scope/path resolver 向一个从 trusted root descriptor 开始且不执行 canonicalization 的 POSIX adapter 提供输入，随后保留 validated scope descriptor。该 adapter 不包含 retry loop、clock、process observer 或 recovery branch。                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Transitions  | 即使 umask 完全 restrictive，缺失 scope 也可以变成一个 identity-matched validated directory。缺失 lock leaf 可以变成一个 durable validated regular file，包括其 creator 最终返回 `Contended` 的情况；creation winner 与 advisory-lock winner 不必是同一个 caller。Unlocked leaf 可以变成 guard-held，并在 guard drop 时回到 unlocked。已有 invalid entry 保持不变。                                                                                                                                                                                                                                                                                              |
| Dependencies | Rust path/value operation、用于 effective UID 与 descriptor-relative POSIX call 的 safe `nix` wrapper，以及用于一次 advisory exclusive attempt 的 `File::try_lock`。不存在 network、socket、child-process、thread、time、randomness、persistence payload 或 Server dependency。                                                                                                                                                                                                                                                                                                                                                                                  |
| Ownership    | Typed scope 来自未来 installation composition；本 application 拥有 derivation、directory validation、lock-file validation 与 guard lifetime。未来 activation workflow 将拥有 deadline 与 process sequencing；Server 必须另外获得已接受的 lifetime-lock seam。                                                                                                                                                                                                                                                                                                                                                                                                    |
| Verification | Owner-local deterministic test 覆盖 valid/invalid scope name、确切 path、relative 与 malformed lexical base、trailing slash 与 dot/parent component、intermediate/final base link、scope link、restrictive-umask initialization、concurrent permissive-umask creation、non-directory、错误 owner/mode、lock symbolic/hard link、directory、FIFO、错误 metadata、独立 contention、stale inode/content preservation、reacquisition 与 cleanup non-ownership。                                                                                                                                                                                                      |
| Limits       | Test 在一个 effective user 下运行，并信任 kernel metadata 与 advisory-lock semantics。它们注入不同的 expected UID，而不是执行 cross-user。Concurrent creation 在 permissive umask 下执行；如果另一个 process 的 umask 限制 user bit，本 primitive 不会在 separate caller 观察到其短暂 pre-chmod window 时 retry。Durable 表示该 owner 绝不 unlink 或 truncate lock file，并不表示对其执行 fsync；kernel filesystem call 不具有 wall-clock guarantee。此 atomic primitive 不选择 TLA+ model；`LocalSiteActivation` 拥有已接受 workflow 的 bounded singleton/process-order 与 abstract total-deadline claim，但不 refine 这个 POSIX adapter，也不实现 activation。 |

## 已接受的本机 Site activation 建模记录

[`LocalSiteActivation`](formal/tla/LocalSiteActivation.tla) 是已接受 connect-first 与
lazy-activation workflow 的 maintained design evidence。它不改变 runtime behavior：bare invocation
仍返回 development-unavailable TUI 结果，public activation 继续保持 unwired。

| 字段   | 已接受的 refinement target                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 问题   | 两个并发的本机 `orchbee` TUI invocation 必须通过同一次 Site activation attempt 协调，且二者都不能成为 Server 或 Worker 的 lifetime supervisor。已存在的 compatible Site 不应产生 launch；Site 不存在时至多启动一个 exact installed cohort；每个非成功 outcome 都必须显式。                                                                                                                                                           |
| 输入   | 两个 Client invocation slot；TUI、help 与 version intent；absent、compatible-ready、incompatible 或 partial 的 initial Site observation；一次 activation-lock result；exact installed Server-plus-Worker cohort capability；typed readiness、incompatibility、partial-cohort 与 launcher-failure observation；以及 abstract monotonic total-deadline expiry。External process outcome 不受信，且可以永远不抵达。                     |
| 数据   | 每个 Client 的 phase、typed result 与 connection path；一个可选 activation-lock owner；zero-or-one launch count 与 exact-installed cohort tag；Site 与 typed-readiness state；以及 Site-owned ready-service lifetime marker。不包含 PID、path、runtime-base、scope、ACL、process tree、cleanup record、recovery identity、service-manager unit 或 numeric time value。                                                               |
| 输出   | Compatible present Site 直接连接。Site 不存在时，一个 lock owner 启动 exact installed cohort，contender 等待 typed readiness。成功时连接并释放 caller 的 activation resource。Incompatible Site、partial cohort、launcher failure 与 deadline expiry 是不同的 failure。Help 与 version 完成时不 discovery 或 activate。Client exit 只结束该 Client。                                                                                 |
| 不变量 | 每条 TUI path 都先尝试 connection 再 activation；activation owner 唯一；launch count 至多为一，且每次 launch 都是在 lock 下启动 exact installed cohort；contender 成为 readiness waiter；只有 ready compatible Site 能产生 connection；settled Client 不拥有 activation resource；ready-service lifetime 属于 Site；failure 与 Client exit 都不停止或改写 Site state；help/version 保留全部 activation fact。                        |
| 决策   | Invocation kind 决定是否允许执行 Site work。Connection observation 区分 direct success、absent activation、incompatible failure 与 partial-cohort failure。发现 absent 后，lock ownership 区分 launcher 与 waiter；lock winner 在 launch 前重新检查 Site state。Typed readiness、typed failure 或 Client-owned total deadline 结算每个 waiter。在 connection 完成之前，deadline observation 可以胜出。                               |
| 模型   | 对恰好两个 Client slot 与四种受支持 initial Site class 建模的 bounded workflow。一个 scalar `activationOwner` 对 `activation.lock` 建模；`launchCount` 与 `launchedCohort` 暴露 singleton exact-cohort launch；`serviceLifetimeOwner` 把 ready Site 与每个 Client 区分。External launcher/readiness action 是 nondeterministic，且没有 fairness assumption。                                                                         |
| 转换   | Present compatible Site 下为 `idle -> connecting -> connected`；present incompatible 或 partial state 下为 `connecting -> failed`；launch winner 为 `connecting -> lock-ready -> lock-held -> waiting-readiness`；contention 为 `lock-ready -> waiting-readiness`；waiting 或 recheck state 会结算为 connected 或一种 typed failure；`connected -> exited` 保留 Site state。Help/version 从 `idle` 直接进入 information completion。 |
| 依赖   | Model 仅在具名 abstraction 上组合已接受 command contract、当前 private `LocalClient` admission boundary 与 `ModeCheckedLocalSiteRendezvous` lock semantics。未来 runtime refinement 必须提供 installation-owned cohort selection、typed readiness 与一个 monotonic total deadline。本 slice 不增加 process、socket、filesystem、localization 或 public-command implementation。                                                      |
| 所有权 | `apps/orchbee` 拥有未来 Client bootstrap workflow、deadline、activation guard 与 typed Client result。Installation composition 拥有 exact cohort capability。Server 与 Worker 在成功后拥有各自 readiness 与 service lifetime。Model 不向 Client 授予 supervise、stop、replace、downgrade、clean up 或 recover Site 的 authority。                                                                                                    |
| 验证   | 注册的 TLC configuration 探索四种 initial Site class 与恰好两个 Client identity，覆盖每个 maintained semantic action，并检查 safety 与 Client-local deadline progress。它检查 850 个 distinct state，depth 为 13。Symmetry reduction 被刻意禁用，因为 TLC 在使用 symmetry 时进行 liveness checking 可能漏报 violation。这个 model-only slice 的 runtime refinement evidence 为 N/A，并且在接线前仍为必需。                           |
| 正确性 | 下方 LA-01 至 LA-05 把已接受 claim 映射到 action、invariant 与 progress property。`WF_vars(DeadlineExpires(client))` 是唯一 fairness clause：它表示 contract-guaranteed local total deadline。`CohortBecomesReady`、typed cohort failure 与 `LauncherFails` 被刻意设为 unfair，因此永远静默的 external process 仍产生 bounded Client failure，而不是被假设会响应。                                                                   |
| 限制   | 该 abstraction 只证明 checked two-Client、one-Site state space。它不证明 runtime refinement、operating-system scheduling、process creation 或 detachment、daemonization、service-manager behavior、runtime base 或 scope、ACL、socket metadata、cleanup、recovery、retry、update、concrete deadline 或 progress threshold、localization、TUI behavior、packaging、public wiring 或 external process progress。                       |

| Transition owner     | Event 与 guard                                                   | Result                                                                                                            |
| -------------------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Invocation           | Help 或 version                                                  | Information completion；Site、lock、launch、readiness 与 service-lifetime fact 不变                               |
| Client connection    | Compatible Site 已 ready                                         | Direct Client connection；无 activation resource 或 launch                                                        |
| Client connection    | Site incompatible 或 partial                                     | 对应 typed failure；无 activation 或 Site mutation                                                                |
| Activation contender | Initial connection 找不到 ready Site 且 `activation.lock` 已持有 | 在 caller 的 total deadline 下等待同一个 typed readiness result                                                   |
| Activation winner    | Lock 已获得、recheck 仍发现 absent 且不存在 launch               | 启动恰好一个 installed Server-plus-Worker cohort，并在持有 guard 时等待                                           |
| Activation winner    | Recheck 发现 ready、in-progress、incompatible、partial 或 failed | 根据 exact observation 连接、释放并等待或失败；绝不启动第二个 cohort                                              |
| External cohort      | Launch pending                                                   | 可以发布 ready、incompatible、partial、launcher failure，也可以完全不返回；external completion 没有 fairness      |
| Pending Client       | 观察到 typed result 或 total deadline                            | 连接或返回 `incompatible`、`partial-cohort`、`launcher-failure` 或 `timeout`；释放 caller 拥有的 activation guard |
| Connected Client     | Client exits                                                     | 只结束 Client Session；保留 Site、cohort、readiness、launch 与 service-lifetime state                             |

因为没有 invocation 能到达 activation，这个 slice 的 runtime refinement 被刻意标为 N/A。未来接线必须先
加入 deterministic process test，才能声称 model 映射到 implementation。

| Claim                                                                                         | TLA+ evidence                                                                                                                                                                                                                                | Bound                                                                                                                           |
| --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| LA-01：每个 activating TUI 都先连接；present compatible、help 与 version path 从不 launch。   | `ConnectFindsCompatible`、全部 connection-observation action、`CompleteHelp`、`CompleteVersion`；`ConnectionProbeNeverActivates`、`InformationInvocationHasNoActivationEffect`。                                                             | 对 command intent 与 abstract observation 建模，不覆盖 parser、transport 或 process refinement。                                |
| LA-02：并发 absent 在一个 lock 下至多产生一次 exact installed-cohort launch。                 | `AcquireActivationLock`、`ActivationLockContended`、`LaunchExactInstalledCohort`；`ActivationOwnerIsUniqueAndPending`、`LaunchIsSingletonAndExact`、`LaunchStartsOnlyUnderTheActivationLock`、`ContenderWaitsForTypedReadiness`。            | 恰好两个 Client slot 与一个 activation round；不声称 runtime-base、lock-file 或 process implementation。                        |
| LA-03：incompatibility、partial cohort、launcher failure 与 timeout 均显式，且不会停止 Site。 | Direct、recheck 与 readiness failure action，加上 `DeadlineExpires`；`ClientShapeIsExact`、`ExplicitFailureHasNoServiceStopEffect`。                                                                                                         | Failure presentation、cleanup、recovery、retry 与 stale-artifact policy 仍不在范围内。                                          |
| LA-04：connection 释放 Client activation ownership；Client exit 不 supervise 或 stop Site。   | `RecheckCompatible`、`ObserveReady`、`ExitClient`；`SettledClientReleasesItsActivationResource`、`ReadySiteOwnsItsServiceLifetime`、`NoClientSupervisesServiceLifetime`、`ClientExitPreservesSite`、`ActivationResourceEventuallyReleases`。 | 只对 ready-service ownership 建模；不选择 process 如何 detach 或 service lifetime 如何实现。                                    |
| LA-05：每个 started TUI 都会结算，且不假设 external process 响应。                            | `EveryPendingTuiEventuallySettles`，且只有每个 Client 的 `DeadlineExpires` 具备 weak fairness；external cohort 与 launcher completion 不含 fairness clause。                                                                                 | 假设未来 adapter 确实保证观察到一个 finite monotonic total deadline；不选择 numeric value、OS scheduling 或 external liveness。 |

当前 one-shot process path 只使用 Rust standard-library argument、environment、stdout 与 stderr
capability。它不打开 file 或 socket，不启动 process 或 thread，不读取 product state，也绝不会
进入已暂存 shell。已暂存 module 使用 Ratatui 完成 stateful rendering，只使用 Crossterm queue
ANSI output，只让 Nix/Rustix 操作继承的 descriptor，并使用 signal-hook 设置 atomic flag。它们不
依赖 Server、Worker 或 Terminal Host，且所有这些 third-party dependency 都只针对 Unix target
声明。在另行接受的 packaged cutover 前，已交付 TypeScript [`apps/cli`](../cli/README.zh-CN.md)
继续作为当前 CLI 权威。

## 测量

从仓库根目录收集 reference release baseline：

```sh
node apps/orchbee/scripts/measure-release-baseline.mjs
```

该脚本构建确切 locked release package，warm 后重复调用 `orchbee --version`，报告 artifact byte
与 warm latency percentile，并在 `/usr/bin/time` 可以提供时报告 peak resident memory。结果只描述
输出中注明的 host、toolchain、profile 与 invocation。它不是 acceptance budget、cold-cache
guarantee，也不是未来 TUI performance 的证据。

已暂存 shell、当前 public path 的 reference run 于 2026-09-03 在 Apple M1 arm64、Darwin
23.6.0、Rust 1.97.0、默认 Cargo release profile 上完成，包含 5 次 warmup 与 50 次 measured
invocation：

| 测量项               | Reference observation |
| -------------------- | --------------------- |
| Executable size      | 432,144 bytes         |
| Warm startup minimum | 1.216 ms              |
| Warm startup p50     | 1.564 ms              |
| Warm startup p95     | 1.936 ms              |
| Warm startup maximum | 2.151 ms              |
| Peak resident memory | 1,343,488 bytes       |

后续已接受切片只要改变 executable 的 reachable dependency graph 或 invoked startup path，就重新运行
并替换这份 reference。这个 staged module 无法从任何 invocation route 到达。受支持平台的 acceptance budget
仍为保留决策。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-server -p orchbee-worker --locked
cargo test -p orchbee --all-targets --all-features --locked
cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings
node scripts/tla/check-inventory.mjs
node scripts/tla/run-models.mjs --models local-client-submission,local-site-activation
cargo tree -p orchbee --target x86_64-pc-windows-msvc --locked
pnpm verify
```
