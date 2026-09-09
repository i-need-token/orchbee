# Orchbee 原生入口

实验性、尚未打包发布的原生 Rust Client。在 Linux 上，裸命令从已校验的开发 cohort
连接或启动本地 Site，取得第一个已校验的终端 projection frame 后进入 TUI。原生 Client 可以创建、
选择、移动和刷新共享 Tab，同时保持独立选择与保留的终端进程。远程和完整 Workbench 操作尚未接入。
产物名为 `orchbee`，参与根 Cargo 验证；Desktop、安装器和包管理器尚未消费它。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## 命令面

确切 development surface 是：

```text
orchbee
orchbee tui
orchbee --help
orchbee --version
```

Linux 裸调用和显式 `tui` 调用先准入继承的同一输入/输出 TTY 和有效 cell/pixel geometry，再运行下面的
本地启动流程。两者使用同一 Client 路径；`tui` 不选择另一种模式。其他平台仍返回本地化不可用结果。
`--help` 与 `--version` 不初始化 TUI 或服务。其他、额外或非 Unicode 参数都是 usage failure；不提供
缩写、业务命令或远程选择。

Help 只写 stdout，失败只写 stderr；Version 写入一行与 locale 无关的 `orchbee <version>`。
正常关闭 TUI、help、version 的开发退出状态为 `0`；启动/运行失败为 `1`；usage failure 为 `2`。
这些不是未来稳定公开退出状态表。

## Linux 本地启动

从仓库根目录构建一个新的开发 cohort：

```sh
node scripts/stage-native-site.mjs --output /tmp/orchbee-native-dev
# Windows 主机使用 WSL Linux：
node scripts/stage-native-site.mjs --wsl Ubuntu --output /tmp/orchbee-native-dev
```

随后在交互式 Linux 终端执行 `/tmp/orchbee-native-dev/orchbee`。
脚本从锁定输入重新构建三个 Rust release 二进制和 Zig ReleaseSafe Host；
已有输出目录会被拒绝。可用 `--cargo-target-dir <absolute-directory>` 复用 Cargo 构建缓存。
这不是系统安装，也不替换现有 TypeScript CLI。

默认 Site 位于 `/tmp/orchbee-native-<effective-uid>`；开发和测试可将
`ORCHBEE_RUNTIME_BASE` 指向另一个已有绝对目录来隔离 Site。
[共享运行时契约](../../crates/local-site-runtime/README.zh-CN.md) 定义 0700 目录、
0600 锁/socket/不可变启动标记、四产物精确 cohort、20 秒共用截止时间和 500 毫秒延迟提示。
Server + Worker 就绪与终端首帧就绪是不同阶段。初始 Open、snapshot read、attach、acquire、
获权后的 resize 以及首帧 Poll 都使用原启动截止时间。原生 Ready 要求完整帧已绘制、其精确反馈已获接受，
并满足当前控制模式尺寸。Worker 消费 Ready reply 后才恢复常规请求预算。
未绘制的 FrameApplied 不会提前发送；NeedSnapshot 恢复反馈可以继续推进首帧获取。

Client 不重启或接管异常 Site。失败/残留运行时明确要求恢复，不自动删除。
`Ctrl+B q` 或 `Ctrl+B d` 只关闭 Client；再次打开连接同一个保留 shell。
同时打开的其他 Client 只读，除非它取得独立 controller grant。
TTY 只准入一次，所有复制描述符均带 CLOEXEC，准备好的 Session 和同一 TTY 交给界面。
当前要求终端报告真实且有效的 cell/pixel 尺寸；不推测缺失的像素值。
Linux 是唯一已有公共启动进程证据的平台；尚不承诺发布性能预算。

## 原生 Tab 交互

原生 Session 呈现当前上下文的有序 Tab 和 canonical 标签。每个 Client 独立拥有活动选择、
navigator 与 viewport；另一 Client 新建的 Tab 在显式刷新后可见。以下操作沿用 `Ctrl+B` 前缀：

| 按键                  | 已连接原生行为                                                      |
| --------------------- | ------------------------------------------------------------------- |
| `c`                   | 请求一次 Server 拥有的 Tab 创建，再选择已验证的提交成员。           |
| `n` / `p`             | 在当前 Client snapshot 中选择下一个 / 上一个 Tab。                  |
| `1` … `9`             | 该位置存在时选择它。                                                |
| `[` / `]`             | 按 Server 导航顺序选择上一个 / 下一个独立上下文或 Workspace。       |
| `a`                   | 通过分步显式提交，在已有或新建 Project 中创建 Workspace。           |
| `e`                   | 配置当前尚未配置且为空的 Workspace 的首个目录。                     |
| `r` / `R`             | 重命名当前 Workspace / 其所属 Project；standalone 没有这两种目标。  |
| `<` / `>`             | 将活动 Tab 移到相邻项之前 / 之后，在两端循环。                      |
| `,`                   | 编辑所选 Tab 的共享 label；首次完整绘制后 Enter 提交，Escape 取消。 |
| `x`                   | 确认关闭所选 Tab 对所有已连接 Client 的执行，或取消。               |
| `w`                   | 打开 navigator 并刷新共享 Tab，保留活动终端身份。                   |
| `h` / `j` / `k` / `l` | 当前 Tab 有分屏树时，将本地焦点移到相邻 Panel。                     |
| `q` / `d`             | 关闭此 Client，保留的执行继续运行。                                 |

[上下文选择](docs/native-context-selection.zh-CN.md) 在切换位置时保留各终端的执行。
导航器和 Workbench 标题显示当前 Project/Workspace。可以进入和离开空 Workspace；只有显式 `c`
才使用 Workspace 已接受的目录创建终端。[Workspace 设置](docs/native-workspace-setup.zh-CN.md)
提供创建及字面绝对路径输入。每一步要求首次完整绘制；取消保留更早已提交的对象。
目录接受绝不启动终端。
[位置标签](docs/native-location-labels.zh-CN.md) 使用预填编辑器与共享 Server 命令；
重命名保留运行中终端，peer 通过显式刷新看到变化。

创建会排空此前已接受的工作，并在截止时间或 Client 退役时保留已知提交事实。
切换先释放旧 viewer，再附着所选 binding；不会停止或重建任一终端。
移动改变共享顺序，同时保留活动终端及其 viewer/controller。如果 Move 已提交且快照返回明确的 denied/unavailable 结果，
Client 保留终端交互，并通过 `w` 显式恢复快照后才允许下一次结构操作。

Rename 精确保留最多 512 个 Unicode 标量。Bracketed paste 原子追加，Backspace 删除一个标量，
Ctrl+U 清空草稿。已知 Rename 提交但投影不可用时，同样通过 `w` 显式恢复，绝不重发命令。
[原生 Session 契约](docs/native-tab-session.zh-CN.md) 定义准入、尺寸、反馈、失败、恢复与证据边界。
Close 要求独立生命周期授权，只读 Client 也适用。关闭最后一个 Tab 后留下 Empty；显式 `c`
创建新终端，普通 Client 退出则保留该终端供后续裸 Open 使用。终端被关闭的 peer 通过显式 `w`
恢复当前成员关系。Linux 进程证据覆盖此流程。未分组、未固定 Tab 的规范分屏投影与本地面板焦点
已经接入；生产重启恢复仍未完成。

## 私有本机 Client transport

在 Unix 上，application-private `LocalClient` 只从 internal caller 接收 endpoint 与 executable
cohort。它把 admission 委托给 `orchbee-local-client-protocol`：该 crate 在连接前验证 cohort，证明
Server 与当前进程具有相同 effective operating-system user，以两秒 read/write timeout 执行确切
`OBLC` handshake，并返回 accepted generation 与同一条无 buffer stream。这些既有 handshake
timeout 仍是独立的 OBLC boundary。在 Linux 上，私有 `LocalClient::connect_before` 将调用方的
绝对单调时钟截止时间贯穿连接、peer proof、hello、响应与最终准入。
[所属 adapter 与正确性记录](../../crates/local-client-protocol/README.zh-CN.md#linux-有截止时间的连接准入)
定义失败清理与截止时间不续期保证。两条路径都在 `OBCS` 前将 admitted stream 切换为
nonblocking I/O。常规请求有独立预算；本地初始请求受原启动截止时间约束。macOS 截止时间准入尚未完成。

`submit_terminal_open`、`submit_terminal_create`、`submit_terminal_start`、`submit_terminal_viewer` 与 `submit_terminal_control` 各自取得一条已经验证的 typed request 的
ownership，并在提交前 encode 完整 OBCS frame。Encoding failure 是 definite failure，并保留
Session。Encoding 成功后启动一个五十秒 monotonic deadline，覆盖完整 frame write 与一次 Server-frame
read；每次 readiness wait 都重新计算剩余 budget，因此 partial progress 不会续期。第一次 write
syscall 之前的 failure 是 definite failure。该 syscall 一旦开始，任何 write 或 read failure、
deadline expiry、malformed 或 incompatible result、错误 result kind 或错误 correlation，都会使
effect outcome 成为 typed `OutcomeUnknown` 并让 connection 退役。transport 绝不 retry，也不返回
request 供 replay。Server 返回的正确 correlated `outcome-unknown` 是有效 OBCS result，而不是
transport ambiguity。

Linux 裸入口通过该 transport 提交独立 Open、查看和控制意图。仅允许关闭的 `LocalClientShutdown`
handle 停止阻塞 exchange，不增加 stream reader 或 writer。重连和结果协调仍未提供。

五十秒提交上限容纳 Server 一个排队及一个在途 Worker exchange（各自最多两秒 write 加二十秒
read）和 Client 结果交付。Handshake 限制保持独立。查看提交验证 action/outcome 兼容性与精确
correlation，并保留 opaque OBSR byte 供已连接的 projection consumer 使用；它不授予 controller 权限，
也不执行终端呈现解码。控制提交针对其 action 验证精确 grant 获取、input acknowledgement 和
完整 resize geometry。Server 报告的 unknown control outcome 原样返回且不重试；不匹配的 outcome
让 transport 退役。Caller 拥有新的 request identity、绑定 grant 的递增 sequence 和后续显式获取。
Session 丢失后的 cleanup 可超出单次提交 deadline。

所有操作既有 viewer 的 action 和所有 controller action 都携带 Server 签发的精确 `attachmentId`。
原生 executor 在 Attach 成功后立即保留该身份，早于任何可能失败或被取代的 setup。
Poll、控制、projection feedback 和 cleanup 都使用这个已捕获的身份；有界 busy 重试保留它，
只更新契约允许刷新的 request identity 和 control sequence。提交前验证 projection feedback
属于其 attachment，绝不重新指定目标。`LocalClient` 不保存隐式的当前 viewer。
原生 UI 仍一次附着一个 viewer；分屏 Tab 保留每个经过验证的叶子 binding，并在本地通过
`h`/`j`/`k`/`l` 沿规范树移动焦点，当前叶子显示实时帧。非活动叶子保留布局和身份，但不会
保留第二个实时 viewer 帧。因此 Server 的多 viewer 路由基础尚未在一个原生 Client 内提供同时
组合多个 viewer 的能力。

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

## TUI shell 底座

Application-private shell adapter 拥有以下终端与呈现边界。裸 Linux 启动使用准备好的 Session；
未连接的 `run` 与显式 `run_connected` 仍是测试与组合接缝。

下面的输入与转换表描述未连接时的 fallback。已连接的上下文和 Tab 操作由上文的原生 Session 处理。

| 关注点      | 当前契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 准入        | 已暂存的 Unix adapter 要求继承的 stdin 与 stdout 都是 TTY，且具有相同 terminal-device identity；它只复制这些已准入 descriptor，并从复制后的 stdout descriptor 读取 cell 与 pixel dimension。它没有 `/dev/tty`、Crossterm global size 或 process fallback。                                                                                                                                                                                                                                                                                                                                                       |
| 生命周期    | 一个已暂存 guard 拥有 raw mode、alternate screen 与 cursor visibility。它会在 operation 可能部分成功前标记 cleanup responsibility，在 inverse 成功前保留每个 flag，并为每个失败的 inverse 最多尝试两次，随后返回一个 aggregate error。Process-lifetime singleton panic hook 只在发生 panic 的 thread 是 active guard owner 时派发，因此其他 thread 无法在保存 terminal state 前清除 setup lease；guard drop 不会移除后来安装的 hook。正常返回、initialization/event/draw failure、owner-thread panic，以及 `SIGHUP`/`SIGINT`/`SIGQUIT`/`SIGTERM` 中的每一种，都有确定性 restoration 或 signal-observation 证据。 |
| Client View | 每个 invocation 拥有 prefix mode、viewport class、sidebar projection、active overlay 与 transient notice。它不拥有任何共享 Tab、Panel、Workspace、Site 或 Terminal fact，也不持久化任何内容。                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 布局        | 96 列及以上时，30 列 Orchbee business sidebar 停靠在 Workbench region 左侧。更窄 viewport 让 Workbench 占满宽度，sidebar 只作为左侧 overlay 显示。已连接的 Workbench projection 包含 Tab strip 和规范 Panel 树；分屏叶子保留身份与布局，当前叶子接收终端帧。                                                                                                                                                                                                                                                                                                                                                     |
| 输入        | 已暂存 adapter 每次最多从已准入 stdin descriptor poll 并读取 4,096 个 raw byte。每次完整 read 都会先依序 dispatch，之后才允许后续 signal、viewport query 或 resize decision 结束本轮 loop。`Ctrl+B` 进入可见 prefix mode。`b`、`w`、`?`、`q` 与 `d` 驱动本地 shell state 或退出；已连接的 `c`、Tab 选择和 `h`/`j`/`k`/`l` action 由 Native Session 处理。在 prefix mode 外，所有 byte——包括 `Ctrl+C`、`Ctrl+D`、escape sequence、UTF-8，以及 overlay 可见时的输入——都会依序到达可注入 terminal-input port；prefix 后的 `Ctrl+B` 会 forwarding 一个 literal byte。                                                |
| 结果        | Shell 消费准备好的 Session。Bootstrap 拥有激活，Server 拥有终端身份，Host 拥有终端生命周期。Shell 不改变 Workbench 状态。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

确定性 Client View transition model 如下：

| 状态与事件                                     | 下一状态、命令与可见结果                                                                                                                                                                                             |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Normal + `Ctrl+B`                              | 进入 Prefix；不发出 terminal input；渲染 prefix indicator。                                                                                                                                                          |
| Normal + 其他 byte，无论 overlay 是否存在      | 保留本地 view state，并将确切、有序的 byte 提供给 terminal-input port。Unavailable port 会在无副作用的情况下拒绝，已暂存 renderer 会报告 input 未发送。                                                              |
| Prefix + `b`                                   | 离开 Prefix，并为此 Client View 切换停靠的宽屏 sidebar 或窄屏 sidebar overlay。                                                                                                                                      |
| Prefix + `w` 或 `?`                            | 离开 Prefix，并在本地打开 Workbench navigator 或 shortcut-help overlay。                                                                                                                                             |
| Prefix + `c`、`n`、`p`、`1` … `9` 或 `h/j/k/l` | 离开 Prefix；已连接 Session 执行对应的 Tab 或本地 Panel 焦点操作，未连接 fallback 保持共享状态不变并显示不可用；不由 shell fallback 提交 command。                                                                   |
| Prefix + `Ctrl+B`                              | 离开 Prefix，并向 terminal-input port 提供一个 literal `Ctrl+B` event。                                                                                                                                              |
| Prefix + `q` 或 `d`                            | 离开 Prefix，并关闭此 Client shell。                                                                                                                                                                                 |
| Prefix + `Esc`                                 | 取消 Prefix，且不改变 overlay。在 Prefix 外，`Esc` 及其后续 byte 都是普通 terminal input。                                                                                                                           |
| Resize                                         | 查询已准入 stdout descriptor，更新 fixed backend viewport，并且只重新计算 Client-local viewport class。进入 narrow mode 会隐藏停靠 sidebar；回到 wide 会关闭 sidebar overlay，并恢复此前的 wide-sidebar preference。 |
| Event/draw error、panic 或 termination signal  | 停止 loop，并尝试恢复 cursor、alternate-screen 与 raw-mode，不启动或停止其他 process。                                                                                                                               |

不变量是：每个 prefix sequence 至多产生一个 typed command；非 prefix byte 保持顺序；未连接 adapter 绝不改变 shared Workbench 与 terminal state；每个可能取得的 terminal mode 都会
保留一个有界 restoration owner。Unit test 覆盖 state、input-port、fixed-backend、layout、renderer、
真实 PTY 的 same-terminal admission、实际 panic hook、全部四种 signal、cross-thread setup
interleaving、post-read viewport/resize failure 与 lifecycle fault injection。Native PTY process test
则分别证明缺失 cohort 的 bare 与 help/version path 不改变 terminal mode、Linux PTY master 在最后一个 slave
关闭后的 `EIO` 是正常 hangup，并且 linked public binary 不包含 `/dev/tty` 或 `tput` fallback token。
已暂存 runtime 及其全部 third-party dependency 仅支持 Unix；Windows target dependency tree 让当前
one-shot CLI 不包含这些 dependency，且不宣称 Windows TUI runtime 或 signal-restoration 支持。

## 已连接单终端 TUI 细化

已接受的本地终端里程碑让私有 TUI 成为 `LocalClient` 的实际消费者：`run_connected` 接收已准入
Client 和显式创建或附着意图。它不选择 endpoint、激活服务、改变公开语法或发布共享 Workbench
事实。真实进程／PTY 见证已经经此 adapter 呈现 Server/Worker/Host 终端；裸 Linux 启动在完成有界 Site 激活后，进入同一个已准备好的 shell。

下面的记录保留显式私有 Create/Attach adapter 及其回归测试。Standalone 启动使用原生 Tab Session，
不使用该 adapter 的单终端队列。

| 字段   | 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | Shell 已有输入和生命周期端口，但不能显示或控制保留终端。接入一个 Session，同时保留及时退出、有界队列、精确反馈和终端生命周期。                                                                                                                                                                                                                                                                                                                                                                            |
| 输入   | 已准入 LocalClient、可选默认 Shell cwd 或已接受 create 引用、继承终端的 geometry 和原始输入、不可信 OBCS/OBSR 响应，以及终止信号。                                                                                                                                                                                                                                                                                                                                                                        |
| 数据   | 一个拥有连接的 task、八条排队 command（每条 input 最多 4,096 byte）、一个 response slot、一个待发 frame feedback、一个有界文本投影、新 request ID 和一个 controller sequence。只有 Host 拥有 VT 和 history。获取已有终端也提交初始 panel geometry；input 与后续 resize 共用同一递增 sequence。                                                                                                                                                                                                            |
| 输出   | Create/attach 和显式 acquire、按序准入的原始输入和 geometry、文本／style／cursor 呈现、精确 applied feedback 或 NeedSnapshot、本地化连接中／只读／blocked／失败状态，以及仅针对 Session 的清理。                                                                                                                                                                                                                                                                                                          |
| 不变量 | UI 线程不执行 Server I/O。Task 串行交换且不重试副作用。队列已满的输入被明确判定为未提交。Unknown outcome 关闭 task 并丢弃未发送 command。没有 task 超过其 TUI owner 的生命周期。                                                                                                                                                                                                                                                                                                                          |
| 模型   | Opening -> 按需执行 standalone snapshot selection -> 已附着 controller 或只读 viewer -> polling -> 收到 record -> 原子文本投影提交 -> draw -> feedback -> polling。控制意图与单一 projection flight 分开排队。关闭会 shutdown 已准入 socket 并 join task；Server 拥有按代次划分的 detach cleanup。                                                                                                                                                                                                        |
| 决策   | Snapshot 替换全部行且不回退已提交 revision（包括 projection reset 后）；delta 要求精确 attachment、epoch、sequence、generation、base revision、screen 和 geometry。无效内容请求新 snapshot 且不应用任何行。错误 envelope 或 scope 让 Session 退役。ProjectionBlocked 清除呈现，直到更高 revision 的 snapshot。没有自动提升 controller、reconnect 或 unknown input 重放。                                                                                                                                  |
| 依赖   | 既有 OBCS 和 Rust Terminal Protocol binding、有界 channel、POSIX socket shutdown、已准入 descriptor、现有 Ratatui renderer 和本地化文案。不增加三方依赖、直连 Host 路径或 VT 模拟器。                                                                                                                                                                                                                                                                                                                     |
| 所有权 | `terminal_session` 拥有连接 task 和输入准入；`terminal_projection` 拥有 Client 文本状态和反馈；Terminal Protocol binding 拥有有界 OBTF 文本解码；shell 拥有 mode 与 draw 顺序。                                                                                                                                                                                                                                                                                                                           |
| 验证   | Canonical OBTF fixture、畸形／边界分区、确定性的 snapshot/delta 和队列／退役轨迹、render buffer 断言，以及真实进程／PTY 见证。既有 LocalClientSubmission 和 LocalTerminalRelay 映射 transport 退役及 projection/control 顺序；其有限边界不是性能保证。                                                                                                                                                                                                                                                    |
| 限制   | 投影文本 cell 和基本 style/cursor；排除 graphics、hyperlink 激活、history、selection、terminal interaction-mode negotiation、cursor shape 对齐和完整 Workbench 行为。不支持的 cell width 使用替代字形。缺失或无效的继承 cell/pixel geometry 会在 Create 或初始 attach 前拒绝，或经恢复边界结束活动 TUI；绝不编造 pixel size。Optional extension 内容保持 opaque，文本 decoder 不保留它们。投影按共享文本分配计数，在提交前拒绝超过 36 MiB 的保留文本。显示 cell 中的控制字符绝不成为终端 escape command。 |
| 审查   | 检查覆盖内存放大、至多一次副作用、draw 失败反馈和 join 取消。原 authority-model 超时问题已解决；启动链路另有独立审查与 Linux 进程证据。                                                                                                                                                                                                                                                                                                                                                                   |

`terminal_text` 拥有禁止显示的 Unicode 标量规则：Unicode 控制字符及 U+2028/U+2029。
终端单元格渲染将该规则与既有宽度检查一起使用，后端最终的 `Print` 还会独立地把禁止标量替换为
U+FFFD。安全文本通过借用使用，不产生分配；规范字符串保持不变。捕获输出测试验证单元格载荷
不能注入终端命令，同时保留后端自身的光标与样式命令。

## Client 已知未改变结果的背压处理

Carrier 保留一个排队请求和一个 wire owner；即使 wire 空闲，队列也可能短暂已满。
Terminal Session 为精确的 viewer/control ServerBusy 结果提供有界处理：
每次都是已完成、已关联且没有派发 Host action 的请求。等待 10 毫秒后，以新 ID 发起新请求。
Input 和 Resize 还使用新的递增 controller sequence，因为 Server 在队列准入前已消耗前一个序号。
Carrier Backpressure 在 OBCN dispatch 前拒绝；Attach 的 Worker 容量拒绝可能已完成 OBCN exchange，
但在创建 Host attachment 前拒绝。逻辑操作只保留一个绝对截止时间；首帧前受原启动截止时间约束，之后使用常规 50 秒预算。
每次底层 LocalClient 提交仍只写入一次。

Create、Open 和 Start 绝不重新提交。普通拒绝、controller 竞争、运行时不可用、错误响应、
transport 失败和未知结果均为最终结果。Feedback 在接受前保持同一个精确的待确认内容；
此策略不能确认未绘制帧。Client 取消关闭 stream、打断 exchange，在正常调度下至多等待一次
10 毫秒退避后 join。到期或身份耗尽会结束操作。

| 声明                                 | 细化映射和证据                                                                                                                                  |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| 只有已知未改变的容量结果允许再次尝试 | Server viewer/control 的 ServerBusy 分支证明 Host effect 未发生；Client 仅匹配这个精确结果。Fake Server 分区检查 busy、拒绝、不可用和未知结果。 |
| 不重放任何可能已发生的副作用         | LocalClientSubmission 仍拥有每个只写一次的 wire exchange；LocalClientBackpressure 仅在已知未改变响应后允许再次提交。                            |
| 截止时间与身份不重置                 | 同一操作截止时间覆盖全部尝试；请求 ID 与 controller sequence 递增；有界 fake Server 轨迹覆盖到期和 busy Resize。                                |
| 保留反馈与取消仍有界                 | Session 只有一个活跃 command，并沿用八槽队列；fake Server 测试与真实并发 Client 轨迹覆盖保留及 join 后释放。                                    |

LocalClientBackpressure 抽象一个 viewer/control 操作、最多三个新请求身份和有限截止时间。
它信任 ServerBusy 到无 dispatch 的分类、精确响应校验及已有 transport 模型；
不证明操作系统调度，也不向其他 caller 或协议授予新的重试权限。

## 本地化与权威

Authored help 与 diagnostic 同时提供 `en-US` 和 `zh-CN`。Locale selection 依次检查非空
`LC_ALL`、`LC_MESSAGES` 与 `LANG`；以 `zh` 开头的 language prefix 选择 `zh-CN`，其他 value
均 fallback 到 canonical `en-US`。Locale data 只影响 presentation，绝不会持久化或用作 authority。

只有 help/version/usage 的 one-shot 路径仅使用标准库参数、环境与标准流。
Linux 裸入口组合共享运行时、私有 Client transport 和 TUI；第三方终端依赖仍只针对 Unix。
在另行接受的打包切换前，TypeScript [`apps/cli`](../cli/README.zh-CN.md) 仍是已交付 CLI 权威。

## POSIX 本机 Site rendezvous 底座

共享所有权与完整原语建模记录现由
[local-site-runtime](../../crates/local-site-runtime/README.zh-CN.md) 维护；应用不再保留重复实现。

## 已接受的本机 Site activation 建模记录

[`LocalSiteActivation`](formal/tla/LocalSiteActivation.tla) 是已接受 connect-first 与
lazy-activation workflow 的 maintained design evidence。此抽象模型独立于后续运行时细化；具体启动标记、进程、生命周期锁和首帧截止时间由共享运行时记录维护。

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
| 所有权 | `apps/orchbee` 拥有 Client bootstrap workflow、deadline、activation guard 与 typed Client result。Installation composition 拥有 exact cohort capability。Server 与 Worker 在成功后拥有各自 readiness 与 service lifetime。Model 不向 Client 授予 supervise、stop、replace、downgrade、clean up 或 recover Site 的 authority。                                                                                                        |
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

该模型抽象了运行时细化；`LocalSiteStartup` 提供具体的启动安全论证。在声称该模型映射到实现
之前，未来接线必须补充确定性进程测试。

| Claim                                                                                         | TLA+ evidence                                                                                                                                                                                                                                | Bound                                                                                                                           |
| --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| LA-01：每个 activating TUI 都先连接；present compatible、help 与 version path 从不 launch。   | `ConnectFindsCompatible`、全部 connection-observation action、`CompleteHelp`、`CompleteVersion`；`ConnectionProbeNeverActivates`、`InformationInvocationHasNoActivationEffect`。                                                             | 对 command intent 与 abstract observation 建模，不覆盖 parser、transport 或 process refinement。                                |
| LA-02：并发 absent 在一个 lock 下至多产生一次 exact installed-cohort launch。                 | `AcquireActivationLock`、`ActivationLockContended`、`LaunchExactInstalledCohort`；`ActivationOwnerIsUniqueAndPending`、`LaunchIsSingletonAndExact`、`LaunchStartsOnlyUnderTheActivationLock`、`ContenderWaitsForTypedReadiness`。            | 恰好两个 Client slot 与一个 activation round；不声称 runtime-base、lock-file 或 process implementation。                        |
| LA-03：incompatibility、partial cohort、launcher failure 与 timeout 均显式，且不会停止 Site。 | Direct、recheck 与 readiness failure action，加上 `DeadlineExpires`；`ClientShapeIsExact`、`ExplicitFailureHasNoServiceStopEffect`。                                                                                                         | Failure presentation、cleanup、recovery、retry 与 stale-artifact policy 仍不在范围内。                                          |
| LA-04：connection 释放 Client activation ownership；Client exit 不 supervise 或 stop Site。   | `RecheckCompatible`、`ObserveReady`、`ExitClient`；`SettledClientReleasesItsActivationResource`、`ReadySiteOwnsItsServiceLifetime`、`NoClientSupervisesServiceLifetime`、`ClientExitPreservesSite`、`ActivationResourceEventuallyReleases`。 | 只对 ready-service ownership 建模；不选择 process 如何 detach 或 service lifetime 如何实现。                                    |
| LA-05：每个 started TUI 都会结算，且不假设 external process 响应。                            | `EveryPendingTuiEventuallySettles`，且只有每个 Client 的 `DeadlineExpires` 具备 weak fairness；external cohort 与 launcher completion 不含 fairness clause。                                                                                 | 假设未来 adapter 确实保证观察到一个 finite monotonic total deadline；不选择 numeric value、OS scheduling 或 external liveness。 |

## 测量

从仓库根目录收集 reference release baseline：

```sh
node apps/orchbee/scripts/measure-release-baseline.mjs
```

该脚本构建确切 locked release package，warm 后重复调用 `orchbee --version`，报告 artifact byte
与 warm latency percentile，并在 `/usr/bin/time` 可以提供时报告 peak resident memory。结果只描述
输出中注明的 host、toolchain、profile 与 invocation。它不是 acceptance budget、cold-cache
guarantee，也不是未来 TUI performance 的证据。

2026-09-07 的当前 Linux reference 使用源码提交 `ac620e35`、x86_64 WSL2、
Linux 6.18.33.2、Rust 1.97.0、默认 Cargo release profile 和同 cohort Zig ReleaseSafe
Host。Python 3.14.4 驱动测量：version 先预热 5 次再测量 50 次；PTY 为 120×32 cell、
960×640 pixel，测量 5 个全新 Site 和每个 Site 3 次重连。隔离调用环境仅包含账号的
HOME/USER/LOGNAME、Cargo 和标准可执行文件路径、仓库根目录 PWD、
`LANG=LC_ALL=en_US.UTF-8`、`SHELL=/usr/bin/zsh`、`TERM=xterm-256color` 和唯一的
`ORCHBEE_RUNTIME_BASE`。文件系统缓存已预热；新 Site 不代表冷 OS 缓存。

界面计时从 Client spawn 到一次完整绘制，其中包含 `[*1 1]` 和底行的
`Tab 1 · Ready · controlling` footer。该节点位于完整帧绘制、精确 FrameApplied /
FeedbackAccepted exchange 及最新尺寸的 UI Ready 发布之后；不证明 Worker 已消费 Ready reply
或清除启动上限。Client VmHWM 在该节点采样：它衡量截至 Ready 的驻留内存峰值，
不代表整个 TUI 生命周期的峰值。每次关闭均验证 terminal mode 精确恢复并退出
alternate screen。每次热重连之前，在计时区间之外按条件等待，确认先前 Server Session task
和 socket 已退出。排队中的 viewer Detach 没有公开完成见证；残余 detach 工作可能计入下次
attachment 的测量延迟。此前单终端 reference 使用了不同的就绪见证，因此这些观察不构成严格的前后对照。

| 测量                                                | 参考观察                         |
| --------------------------------------------------- | -------------------------------- |
| Client 可执行文件大小                               | 2,989,768 bytes                  |
| Version 启动 min / p50 / p95 / max                  | 0.791 / 0.834 / 1.176 / 1.406 ms |
| Version 峰值驻留内存                                | 2,818,048 bytes                  |
| 新 Site 完整 Ready 帧 p50 / p95（5 次）             | 485.526 / 501.903 ms             |
| 热 Site 完整 Ready 帧 p50 / p95（15 次）            | 439.502 / 486.007 ms             |
| Client 截至 Ready 的峰值驻留内存 p50 / max（20 次） | 4,792,320 / 4,894,720 bytes      |

这些是该主机的实际观察，不是性能承诺，也不适合直接和其他 OS、driver 或旧 one-shot 产物比较。
后续改变可达依赖或启动路径时重新测量并替换 reference；支持平台的发布验收预算仍待确定。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-server -p orchbee-worker --locked
cargo test -p orchbee --all-targets --all-features --locked
cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings
node scripts/tla/check-inventory.mjs
node scripts/tla/run-models.mjs --models local-client-admission,local-client-submission,local-client-backpressure,local-site-activation,local-site-startup,workbench-terminal-selection
cargo tree -p orchbee --target x86_64-pc-windows-msvc --locked
pnpm verify
```

使用 `ORCHBEE_TEST_NATIVE_COHORT` 指向已暂存的绝对路径，可对实际 Release 产物运行同一套
真实进程验收。Fixture 先校验该 cohort，再复制到各自隔离目录；测试不改变源产物或默认 Site。

```sh
ORCHBEE_TEST_NATIVE_COHORT=/tmp/orchbee-native-dev cargo test -p orchbee --test local_startup_process --locked
```

## 独立终端意图

私有的已连接 TUI 接受 `TerminalIntent::Standalone`。它提交一次
`submit_terminal_open` 并要求 Opened，然后在同一条已准入连接上读取相关联的 OBWP snapshot。
它使用 snapshot 中确切的 Site/Worker/Server-terminal binding 提交 OBCS `attach-binding`；
Open create reference 仍是显式旧路径的值，不用于选择此 viewer。Linux 裸入口通过已校验的
本地 Site 激活与 Worker 按需启动 Host 消费此路径。显式 Create 与 Attach intent
保留现有 Create-reference attachment 行为。

纯投影器要求恰好一个 context 匹配 standalone context ID，没有 group，并包含零到 256 个按序、
未分组、未固定的 Tab。每个 root 都必须是 Panel 或递归二叉 Panel 分屏树，没有 child Presentation，
且每个叶子的 primary Presentation 都具有 binding。它精确保留两种标签、全部成员身份、split/slot
拓扑与各自 binding；任一不受支持的成员都会使整个投影被拒绝。冷启动另外要求存在第一个有序 Tab，
并使用其 binding。Canonical OBWP validation 拥有全局唯一性和引用完整性。纯刷新与已创建成员验证器
校验修订号、成员不可变事实及关联创建事实。交互式创建、切换、刷新、分屏树渲染及本地 Panel 焦点
已经接入；原生 Session 保持当前叶子作为唯一已附着 viewer。

终端投影适配器还为原生 Session 提供控制模式下的完整帧尺寸门。传入期望尺寸时，四个字段必须
全部匹配；不匹配会清除可见帧并请求快照，但不会重置序号、修订号或阻塞修订号栅栏。原生控制模式
建立流程使用该尺寸门；保留的显式 Create/Attach adapter 不传入期望尺寸。针对性测试覆盖每个字段，
以及通过新的匹配完整帧恢复。

Open、snapshot read、attach、controller 获取、首次获权 Resize 和第一个已校验的终端
projection frame 共用现有 startup ceiling。Observer 不调整大小。Snapshot exchange
保持单一 carrier 及现有非阻塞 deadline I/O owner；Workbench request ID 与 OBCS correlation
及逻辑终端 ID 保持独立。

| 条件                                                                                 | Carrier 状态及 standalone 行为                                                         |
| ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| 本地编码或确定的首次写入前失败                                                       | 保留 carrier，返回明确的 not-submitted 失败；workflow 以 unavailable 退出，不重试。    |
| 可能 handoff 后发生 I/O 错误；畸形、超限、截断、错误 kind 或错误关联的 snapshot 响应 | 退役混合 carrier，报告 snapshot-read 失败；已经成功的 Open 不变成未知 launch。         |
| 有效的 denied 或 unavailable snapshot 结果                                           | 返回有类型的结果，不执行协议 retirement；workflow 以 unavailable 退出并丢弃 Session。  |
| 有效 snapshot 的 standalone 结构不受支持                                             | 以 unavailable 退出，不再次选择，也不退回 Open reference。                             |
| 已知未改变状态的 viewer server-busy 结果                                             | 现有 retry 保留确切 binding 三元组，使用新 OBCS request ID，并保留同一个逻辑截止时间。 |
| Unknown Open/submission 或不明确的 attachment 结果                                   | 停止，不重放 Open、不改为 Create，也不退回 Create-reference attachment。               |

Snapshot read 不重试。Attachment 后仍须完成现有 controller 与 projection 准入，
持有 binding 不授予 controller 权限。独立 Site/Worker/Terminal 身份域中的相同编码字节是
有效的；每个字段独立拥有类型并接受校验。

[Server 所有的记录](../orchbee-server/docs/workbench-authority.zh-CN.md#从-workbench-绑定选择终端)
定义确切 membership lookup、失败优先级、保留 Worker generation 与 attachment 线性化。
此 Client 不拥有独立终端，也不写入 Workbench 成员关系。Session trace 覆盖混合请求顺序、
确切选择三元组、结构拒绝、有界 I/O 和不存在 fallback；新构建 release 的 PTY 测试覆盖
首次打开及返回保留 shell。Server 的 WorkbenchTerminalSelection 模型证明有界 selection safety；
现有 carrier 和 Client deadline 模型及运行时测试拥有 progress。远程访问、重启后接管、
持久化及完整 Workbench Client 不属于此切片。

已接受的[原生 Tab 会话契约](docs/native-tab-session.zh-CN.md)负责定义多 Tab 创建、本地选择、
显式刷新及其共享载体上的操作屏障，并规定相应的实现与进程验证要求。纯投影与分类 TabCreate
适配器已经实现。适配器区分明确提交前、已提交及未知结果；调用方截止时间方法从同一个注入的
共享时钟计算普通、调用方与启动上限，采用其中最早者。确定性的会话状态与有界邮箱实现原子的
选择准入、Create/刷新屏障、精确事件结算、尺寸门及载体退役。退役保留首个原因，并区分明确的
未知结果与确定失败。这些核心实现已经与原生连接执行器及 UI 消费方集成。屏障生成测试、
脚本化载体轨迹及 Linux 上真实的双 Client Tab/刷新进程见证提供实现证据；契约的
[声明与证据矩阵](docs/native-tab-session.zh-CN.md#声明证据矩阵)记录已检查的声明及其边界。
这些进程证据不证明其他操作系统、安装、重启恢复或远程访问。
