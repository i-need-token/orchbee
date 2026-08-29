# SSH Environment 连接

## 范围与已交付边界

Desktop Main 现在拥有一个显式请求的 SSH Environment 的进程内 Slice 2.2 连接基础：target
resolution、一个受监督的 OpenSSH child、瞬时 AskPass、来自 signed SSH Environment release 的 verified
Runtime projection、无下载远程 bootstrap、精确 Hello 与 heartbeat、generation-fenced retry，以及
attachment retirement。原生
`apps/ssh-environment-runtime` daemon 与 `@orchbee/ssh-environment-protocol` wire contract 是该边界的
具体 consumer。

Slice 2.3A 单独把 durable append-only Environment registry 组合进 `DesktopApplication`。Slice 2.3B
单独交付离线且仅用于 release 的双平台 Runtime bundle 与 Desktop trust composition。Slice 2.3C 现在组合了
一个按 durable Environment identity 建立的显式进程内 connection owner，并增加 same-child
installed-artifact negotiation。现在还有一个 Slice 2.4 进程内 API，可以通过已连接的 epoch 检查一个显式的
远程 Project-root candidate。Slice 2.5 将该 observation 与 canonical Workbench writer 组合，通过仅 Main
可用的进程内 API 准入一个不可用的远程 Project Directory。Slice 3.1B 原子安装 signed Runtime 与 Terminal
Host cohort，在现有 attachment 上公布一个 Terminal Host relay，并返回已协商的进程内 `HostConnection`。
Slice 3.2A 现在把同一项仅 Main 可用的 admission 扩展为在一次 canonical commit 内创建精确一个 inactive
remote Workspace 与 empty Terminal Context。Slice 3.2B 新增一个显式且仅 Main 可用的 operation，通过
Environment-scoped Terminal runtime provision 或 reconcile 该 Workspace 的第一个 terminal。Slice 3.2C 从
canonical Presentation containment 把 Electron 与 authenticated Gateway projection request 路由到该精确的
prepared Environment runtime，复用现有 interactive attachment contract，并在发布 lease 前重新检查 authority。
Slice 3.2D 现在从精确 Environment 的 current relay 观察 metadata、runtime failure 与 terminal exit，在
listener 返回前 durable journal exit，并且只有通过 Environment-scoped binding 与 canonical target 校验后
才发布。Slice 3.2E 新增一个仅限 Electron 的产品入口，用于从精确 empty remote Workspace 创建或 reconcile
该首个 singleton。它使用专用 renderer capability，由 Main 派生并重新校验全部 remote authority，并等待
canonical client-session publication。当前没有 Gateway 或 CLI creation surface、Runtime network feed、
自动 SSH 连接、directory browser、file access、Git 或 Agent authority。测试只使用本地临时目录、临时
signing key 与受控进程；不会连接真实 SSH server。

## 已接受决策 D17-D26

| 决策 | 已接受契约                                                                                                                                                                                                                           |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D17  | 拓扑固定为版本化不可变安装 → 每个 Environment 一个 detached Runtime daemon → 未来私有 service adapter。SSH 只是可替换 attachment。                                                                                                   |
| D18  | 每个 Environment generation 由一个 active OpenSSH child 通过 stdin/stdout 承载，且不分配 PTY。关闭 OpenSSH connection sharing、agent/X11 forwarding、配置中的 forwarding 及 local command。                                          |
| D19  | 产品协议直接通过 SSH stdio 承载，不存在 SOCKS 或 dynamic-forwarding 层。                                                                                                                                                             |
| D20  | `@orchbee/ssh-environment-protocol` 最初拥有精确版本 `0.0.1`、Hello、Runtime epoch、heartbeat、channel lifecycle、binary data 与 byte-credit framing。D58 只取代该精确版本与 capability set；协议仍没有通用 execute-command record。 |
| D21  | AskPass 强制启用，使用 owner-only 本地 IPC、每个 generation 一个随机 capability、瞬时且不受信的 prompt，并且 credential 不进入 state、log、telemetry 或磁盘。                                                                        |
| D22  | 永不缓存 password。一个 active child 自然复用其已鉴权 SSH connection；替换 connection 可能再次提示。优先使用 key、certificate、hardware key 或 agent-backed authentication。                                                         |
| D23  | Desktop 传输精确 artifact，远端 host 从不自行下载。准入要求 canonical Ed25519-signed manifest，以及精确 size 和 SHA-256。签名私钥位于 repository 与普通 CI 之外。                                                                    |
| D24  | 在 `~/.orchbee/ssh-runtime/versions/<version>/` 下 side-by-side 安装。执行使用精确路径；不存在可变 `current` link，也不自动删除。                                                                                                    |
| D25  | 初始远端矩阵仅为 macOS arm64 与 glibc Linux x86_64。其他远端平台全部以 `compatibility-unsupported` 失败。                                                                                                                            |
| D26  | Disconnect 只 retire attachment。幸存 daemon 保持同一个 epoch 与未来 inventory；重启 daemon 返回新 epoch。未知 terminal 或 Agent effect 永不自动 replay。                                                                            |

## Durable Environment identity

已接受决策 D27-D36 把 durable identity、release composition 与 installed-artifact negotiation 拆成可独立
回滚的切片。Slice 2.3A 只交付第一项：Desktop Main 是 `environment-registry-v1.json` 的共址 Hub owner，
具有相互独立的 Environment 与 Runtime identity、精确 SSH target uniqueness、调用方预分配的幂等 create、
严格 owner-only atomic persistence，以及 indeterminate commit 后的显式 reconciliation。

创建或加载 registry record 不会启动连接，也不会执行远程 effect。Record schema、transition table、privacy
boundary、rollback behavior、TLA+ refinement 与剩余 authority 位于
[`environment-registry.md`](environment-registry.zh-CN.md)。Slice 2.3B release composition 现在已经独立
交付；Slice 2.3C fast-path/supervisor composition 也已独立交付。

## 离线 release composition

已接受决策 D37-D46 固定一份 canonical schema-version-1 release、在 Zig `0.16.0` 下构建的精确 macOS
arm64 与 glibc Linux x86_64 stripped `ReleaseSafe` artifact、外部 Ed25519 signature、独立提供的 public
key，以及每份 Desktop package 一份精确固定 manifest 的 trust descriptor。Repository 代码绝不接受
production private key，普通 CI 只使用临时 key。

普通 package command 保持不包含 release。Release-only command 会在 Electron Packager 前验证并缓冲精确
final bundle，把 executable artifact 与 manifest 放在 `Resources` 下，并把 trust descriptor 保留在
`app.asar` 内。固定 layout、command ceremony、trust assumption、rollback、modeling record、verification
mapping 与剩余 release limit 位于
[`ssh-runtime-release.md`](ssh-runtime-release.zh-CN.md)。

Slice 3.1A 只用 schema version 2 取代当前 release packaging path：每个已准入平台各有一个独立 version
的 Runtime 与一个独立 version 的 Terminal Host，并由一个 signed canonical cohort manifest 绑定。冻结的
v1 contract 只对已经交付的 Runtime behavior 保持有效，不能满足未来 Terminal Host capability。

## 已接受决策 D47-D56

| 决策 | 已接受契约                                                                                                                                                                                                                                                               |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D47  | Desktop Main 拥有按 `EnvironmentId` 建立的进程内 connection owner。Registry 仍只表示 durable intent：load 与 create 永不连接，restart 也永不 auto-connect。                                                                                                              |
| D48  | Desktop 在 OpenSSH 前把 package 内的 trust descriptor 与 manifest 作为一个 cached release catalog 加载并完成鉴权。只有远端请求 upload 后，才会 lazy 读取并验证对应平台的 artifact bytes。                                                                                |
| D49  | Platform 既不持久化，也不通过第二个 SSH probe 解析。承载安装与 Hello 的同一个 OpenSSH child 检测一个已准入远端平台。                                                                                                                                                     |
| D50  | Negotiation 只接受一条以 newline 结尾且最大 128-byte 的 ASCII token：`OBSI/1 darwin-arm64 present`、`OBSI/1 darwin-arm64 upload`、`OBSI/1 linux-x64 present` 或 `OBSI/1 linux-x64 upload`。Unknown、duplicate、extra、non-ASCII 或 oversized bytes 全部 fail closed。    |
| D51  | Present version 必须是非 symlink 的 regular file，并具有精确 catalog digest。不存在时请求 upload。Installation 使用 no-clobber hard link；并发且相同的 installer 会 converge，任何已安装 mismatch 或 winner mismatch 都会 block 且不会替换。                             |
| D52  | Status token 只授予 flow-control authority。精确 Hello-Accepted 仍是 instance、version、digest、platform、protocol 与 daemon epoch 的最终 trust decision；不兼容的 live daemon 会 block，而不会被替换。                                                                  |
| D53  | 每个 active Environment 只存在一个 supervisor。Connect 与 disconnect 按 identity 串行化，重复 connect 幂等，active supervisor 会固定 registry intent 的副本，直至显式 disconnect。                                                                                       |
| D54  | 现有 owner-only AskPass bridge 仍是 credential boundary。Slice 2.3C 不增加 renderer prompt UI；缺少 interaction handler 时会 cancel。Windows activation 不受支持，并继续区分 release、compatibility、integrity、authentication、host-verification 与 retryable failure。 |
| D55  | Activation 是显式进程内调用。Application shutdown 会 dispose 所有 active supervisor。不进行 registry migration，不自动 activation，并保留远端 immutable version。                                                                                                        |
| D56  | TLA+ 与可执行 test 覆盖 ordering、fast path、mismatch/no-clobber behavior、owner serialization、idempotence、disposal 与 failure mapping。必须使用本地受控进程 evidence；真实 SSH server 仍在本 slice claim 之外。                                                       |

## 已接受决策 D57-D66

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                    |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D57  | 第一个具体 Runtime service 检查一个显式提供的远程 Project root。Runtime 执行 filesystem observation，Desktop Main 通过 Environment connection owner 授权并协调该操作，现有 protocol package 拥有 wire schema。不创建新 package 或 Workbench writer。                                                                          |
| D58  | 精确 `OBSE` protocol 升级为 `0.1.0`。从未 advertised 的 placeholder capability 在首次使用前移除；Runtime 只 advertise `project-root-v1`，Desktop 要求该精确 singleton，双方都不提供 dual-stack negotiation 或 downgrade。                                                                                                     |
| D59  | 一个 channel 承载一条不超过 16 KiB 的 strict JSON request。它只包含一个 encoded bytes 不超过 4,096 的 absolute POSIX UTF-8 path，不允许 NUL 或 control character，也不进行 tilde、environment-variable 或 shell expansion。Channel identity 是完整 wire correlation identity。                                                |
| D60  | Runtime 通过远端 operating system 跟随 symlink，要求最终 target 是 SSH account 可访问的 directory，并且只返回其 canonical absolute UTF-8 path。结果是可撤销 observation，不是 retained handle、Project identity、Checkout identity、Directory identity，也不保证后续 filesystem state 不变。                                  |
| D61  | Desktop 发起的 service channel 遵循 `open → opened → request → result → close → closed`。每个方向精确获得 64 KiB byte credit，单条 service message 上限为 16 KiB，heartbeat 保持独立；epoch、channel、framing、ordering 或 credit violation 会 retire attachment。                                                            |
| D62  | 一个 Environment 最多准入四个并发 inspection。每个 caller-visible inspection 必须在 30 秒内 settle；filesystem work 与 attachment loop 隔离，因此不能阻塞 heartbeat。Capacity 与 deadline exhaustion 分别返回 `busy` 与 `timed-out`。                                                                                         |
| D63  | Inspection 要求一个已连接的精确 Runtime epoch，绝不 activation connection，也不等待 connection。Cancellation 或 attachment loss 会终止 invocation；reconnect 绝不 replay。显式 retry 会分配新 channel，late worker 与旧 epoch result 被禁止 publish。                                                                         |
| D64  | 远端 outcome 被限制为 `invalid-path`、`not-found`、`not-directory`、`permission-denied`、`timed-out`、`busy` 或 `unavailable`，且不包含原始 operating-system diagnostic。有效 channel 中的 invalid service payload 只让该 channel 失败；transport、epoch、channel-table、ordering 或 credit corruption 会 retire attachment。 |
| D65  | Requested path 与 canonical path 只存在于进程内，不进入 Environment registry、log、diagnostic 或 telemetry。本 slice 不增加 renderer 或 Browser API、persistence、directory listing、file byte、watch、filesystem mutation、Git、shell、terminal、Agent、Project、Checkout、Workspace 或 automatic connection。               |
| D66  | 注册的 TLA+ service model 与可执行 evidence 覆盖精确 capability admission、channel order、credit、capacity、timeout、cancellation、attachment loss、no replay、stale-result fencing、strict codec、真实本地 Runtime behavior 与两个 native target。真实 SSH、authentication UI 与 production signing 不在 claim 内。          |

## 已接受决策 D67-D76

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                            |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D67  | Desktop Main 拥有一个进程内远程 Project admission。输入为 `EnvironmentId` 与 requested path。成功时精确创建一个 Project，其中有一个远程 Folder Directory、零个 Workspace 与零个 terminal context。                                                                                                                    |
| D68  | `WorkbenchDirectory` 继续作为承载 Checkout 的实体，并增加显式 placement：`local`，或带一个 opaque Environment 引用的 `environment`。Workspace 以后通过 primary Directory 继承 placement；不引入 Checkout package 或 store。                                                                                           |
| D69  | `WorkbenchSharedDocument` 精确升级为 version 2，文件名为 `workbench-shared-document-v2.json`。当 v2 缺失时，一个有效 v1 文件把全部 Directory 迁移为 local placement，且不修改或删除 v1。v2 一旦存在就是唯一 authority，不提供反向 migration。                                                                         |
| D70  | Admission 解析已注册 Environment，要求其已经 connected，调用精确 `project-root-v1`，从返回 canonical path 派生名称，然后进入一个 Main-owned canonical mutation。它绝不自动 connect，也不等待 connectivity。                                                                                                           |
| D71  | Workbench 存储 opaque Environment 引用；Desktop 对照 registry 校验。Client 整文档 proposal 可以原样保留或删除已有 remote placement，但不能新增、更改、伪造、reparent、改标为 local 或将其设为 available。只有 Main workflow 能引入 remote placement。                                                                 |
| D72  | `(EnvironmentId, canonicalPath)` 是 durable admission idempotency key。并发与重试的等价 admission 收敛为一个 Project；不同 canonical root 按 canonical queue 顺序 commit。新 remote Directory 获得 Main 生成的随机 128-bit identity，Project identity 使用现有 Workbench sequence。                                   |
| D73  | Inspection 完成前的 cancellation 或 attachment loss 不创建 Project。Canonical result 一旦进入 canonical mutation queue，caller cancellation 不再 rollback，后续 disconnect 也不会使该 point-in-time result 失效。可能已经越过 rename 的 outcome 为 `indeterminate`；restart 与 retry 通过 idempotency key reconcile。 |
| D74  | 已 commit 的 Environment 引用与 canonical path 持久化到 shared document，并通过已授权 Electron、Browser 与 semantic read 投影。Requested path、failure 与 raw operating-system diagnostic 不持久化、不记录日志，也不发 telemetry。它仅对已 commit canonical path 覆盖 D65。                                           |
| D75  | Remote Directory 强制为 `available: false`；它不能创建 Workspace、解析 terminal CWD 或派发 Host effect。移除 Project 只删除产品 metadata。本 slice 不增加 remote delete、listing、file byte、Git、shell、terminal、Agent、sync、renderer creation UI 或 CLI operation。                                               |
| D76  | 注册的 TLA+ admission 与 migration model、property evidence、Desktop file migration 与 authority test，以及完整 repository verification 覆盖本 slice。真实 SSH 与产品 UI 仍不在范围内。                                                                                                                               |

## 已接受决策 D77-D86

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                               |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D77  | Phase 3 分 slice 实施。Slice 3.1A 只拥有 remote-execution component 构建、签名、验证与 Desktop packaging；它不创建 remote installation、process、Workspace 或 terminal effect。                                                                                                                                                                                                                          |
| D78  | 现有 `orchbee-terminal-host` 继续作为唯一 PTY、process-tree、Ghostty-state 与 terminal-recovery authority。SSH Runtime 在本 slice 不重做 terminal behavior，也不增加 generic execution、shell 或 network listener。                                                                                                                                                                                      |
| D79  | `ssh-environment-release-v2` 与冻结的 v1 相互独立，精确包含 `ssh-environment-runtime`、随后是 `terminal-host`；每个组件精确包含 `darwin-arm64`、随后是 `linux-x64`，合计四个 artifact。                                                                                                                                                                                                                  |
| D80  | Runtime 与 Terminal Host 保持不同的 semantic component version、size 与 SHA-256 digest。Signed manifest digest 标识 compatibility cohort；它不替代 Runtime build identity 或未来 Terminal Host epoch identity。                                                                                                                                                                                          |
| D81  | 两个组件都使用 Zig `0.16.0`、`ReleaseSafe`、stripping 与显式 version。精确 target 为 Apple Silicon macOS 与 x64 glibc Linux，artifact 名为 `orchbee-ssh-environment-runtime` 与 `orchbee-terminal-host`。                                                                                                                                                                                                |
| D82  | 每个 native application 拥有自己的 build recipe。仓库级 release automation 拥有跨 application prepare/finalize 并消费 installed build output；两个 application 都不导入对方 source tree。                                                                                                                                                                                                                |
| D83  | Prepare 为离线操作，finalize 接受一个外部 raw Ed25519 signature。一个 canonical signature 覆盖每个 component role、version、platform、size 与 digest。布局必须精确，symlink 和多余 entry fail closed，每个 artifact 上限 128 MiB，manifest 上限 64 KiB。Production private key 保持在仓库 tooling 之外。                                                                                                 |
| D84  | 显式 Desktop release packaging 在暂存前独立验证 signature、manifest、四个 artifact 与 public key。它把精确 trust descriptor 放在 `app.asar` 内，并把已缓冲的 manifest/artifact 放在 `Resources` 下；普通 development packaging 继续不带 release。                                                                                                                                                        |
| D85  | 冻结的 v1 artifact 只对已交付 Runtime 与 `project-root-v1` behavior 保持有效。未来 Terminal Host capability 必须要求已验证 v2，不能使用 v1 companion、remote `PATH`、remote build、network download、mixed cohort，或在 validation failure 后 downgrade。                                                                                                                                                |
| D86  | Evidence 覆盖 strict schema 与 canonical bytes、missing/reordered/expanded/oversized/tampered input、错误 key 与 signature、symlink、固定 build arguments、真实 target compilation、prepare/finalize/package round trip、documentation pairing 与 repository gate。真实 SSH、remote upload、production signing、Workspace、terminal execution 与 UI 不在 claim 内；这个有界顺序工作流不需要 TLA+ model。 |

## 已接受决策 D87-D96

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D87  | Slice 3.1B 拥有 signed Runtime/Terminal Host cohort 的 immutable remote installation 与 reuse、一个 opaque Terminal Host relay，以及 Desktop Main `HostConnection`。它不创建 terminal、Workspace、renderer、CLI、Skill 或 Agent effect。                                                                                                  |
| D88  | 一个现有 OpenSSH stdio attachment 同时承载 `project-root-v1` 与 `terminal-host-v1`。Runtime 不获得第二个 SSH connection、forwarding、network listener、generic execute 或 shell authority。                                                                                                                                               |
| D89  | Bootstrap 精确使用 `OBSI/2`：依次处理 Runtime、Terminal Host status/upload，再通过一个 `OBSC/2` commit barrier。只有两个精确 artifact 都存在后才启动 Runtime；v1、downgrade、mixed cohort、component reorder 与 commit 后多余 bytes 都 fail closed。                                                                                      |
| D90  | Runtime 继续位于 `~/.orchbee/ssh-runtime/versions/<version>`，Terminal Host 安装到 `~/.orchbee/terminal-host/versions/<version>`，两者各有私有 `state` directory。Terminal Host endpoint identity 与 Runtime identity 采用 domain separation。                                                                                            |
| D91  | `OBSE` 精确为 `0.2.0`，capability 精确为 `[project-root-v1, terminal-host-v1]`，Hello-Accepted 绑定 Runtime build、release cohort、Terminal Host version/build、platform 与 epoch。不提供 dual stack 或 downgrade。                                                                                                                       |
| D92  | Main 显式打开至多一个 relay。Runtime 认证并连接同 UID Host endpoint，或用固定 argument 启动 signed Host 精确路径。一个 attachment 内所有 service 共用全局单调 channel identifier。                                                                                                                                                        |
| D93  | Relay 双向各有 64 KiB credit，maximum chunk 为 64 KiB。Host connection 与 I/O 在隔离 worker 中运行，不能阻塞 heartbeat 或四个 project-root worker。`service-busy`、`service-unavailable` 与 `compatibility-unsupported` 是 channel-local；outer corruption 会 retire attachment。                                                         |
| D94  | Attachment loss 只关闭 relay broker，绝不 replay outcome unknown 的 effect。带 retained terminal 的 surviving Host 继续作为 authority；reconnect 创建新 relay，并使用 Host epoch/inventory reconcile。                                                                                                                                    |
| D95  | 只 reuse 精确兼容的 Host。Runtime 永不 kill、replace、upgrade、downgrade、mix、cleanup 或安装 operating-system service；没有 mutable `current` pointer 或 linger policy。                                                                                                                                                                 |
| D96  | 已登记 TLA+ model、adversarial codec、deterministic process test 与真实本地 Runtime-to-Host process trace 覆盖 cohort activation、relay exclusivity、credit、local-failure containment、attachment loss 与 no replay。真实 SSH、production signing、terminal creation、Workspace、UI、CLI、Skill 与 Agent orchestration 不在本 slice 内。 |

## 已接受决策 D97-D106

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                          |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D97  | Slice 3.2A 为 Main-owned remote-Project admission 增加精确一个 durable Workspace placement。它不创建 terminal presentation、Terminal Host effect、renderer-owned authority、UI、CLI、Skill 或 Agent operation。                                                                                                                     |
| D98  | `WorkbenchSharedDocument` 继续精确使用 version 2。Remote Workspace 使用现有 Workspace field，引用一个 Environment placement primary Directory，并拥有一个其他方面有效的 empty Terminal Context。不改变 file name、schema shape，也不需要 migration。                                                                                |
| D99  | Admission 在进入 Main mutation queue 前，仍要求一个已登记且 connected 的 SSH Environment，以及成功的 `project-root-v1` canonical inspection。全局 idempotency key 仍为 `(EnvironmentId, canonicalPath)`。                                                                                                                           |
| D100 | 缺失的 remote Project 与其 Workspace 原子 commit。重新 admission Slice 2.5 Project 会原子补上缺失 Workspace；重新 admission 已拥有该 Workspace 的 Project 为 unchanged。Sequence-owned Workspace 与 Terminal Context identity 永不复用已删除 identity。                                                                             |
| D101 | Workspace name 从 canonical path 派生，引用 admitted Directory，保持 inactive，且不展开 Project。其 Terminal Context 不含 Tab、Panel、presentation、lifecycle intent、launch CWD 或 pending removal。                                                                                                                               |
| D102 | Environment Directory 持久保持 `available: false`；connection availability 是 shared document 之外的 observation。Remote Workspace 可以在断线时继续 durable，但本 slice 仍拒绝 terminal creation、restart、split、duplicate、primary edit、launch configuration 与 local-CWD fallback。                                             |
| D103 | 普通 Client proposal 可以精确保留或删除 canonical remote Workspace 及其 empty context。它不能引入、reparent、relabel、mutate、duplicate、convert remote placement，或向其中加入 Terminal state。只有 Main admission owner 可以引入它。                                                                                              |
| D104 | Queue admission 继续作为 cancellation commit point。此前 cancellation 或 attachment loss 不创建任何内容；此后不能 rollback queued mutation。Project、Workspace、Directory 与 empty Terminal Context 共用一次 document commit，包括 rename 前 failure 与 rename 后 indeterminate recovery。                                          |
| D105 | 删除 Project 或 Workspace 只移除 product metadata 与 empty context。它不执行 remote filesystem deletion、Host close、terminal archive、Git、shell、source transfer、synchronization 或 Environment lifecycle operation。                                                                                                            |
| D106 | 扩展后的 remote-admission TLA+ model、deterministic Workbench transition/document/runtime test、Desktop authority 与 persistence test、Client-forgery test、documentation pairing 和 repository gate 覆盖本 slice。真实 SSH、Terminal Host protocol effect、非空 remote context、UI、CLI、Skill 与 Agent orchestration 仍在范围外。 |

## 已接受决策 D107-D116

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D107 | Slice 3.2B 授权一个显式 Desktop Main 进程内 operation，用于 provision 或 reconcile 已准入 SSH Environment Workspace 的首个 terminal。它不增加 renderer creation API、semantic CLI operation、Skill、Agent authority、split、restart、close 或通用 remote command。                                                                                                                                                                               |
| D108 | 输入为一个 `EnvironmentId`、Project identity 与 Workspace identity。准入要求已注册 Environment、已经 connected 的 observation，以及 primary Directory 精确放置在该 Environment 上的 canonical Workspace。该 operation 不会启动 SSH、等待重连、接受 path hint，也不会改变 active/expanded Client View state。                                                                                                                                     |
| D109 | Remote Workspace 是有界 idempotency key，并且在本 slice 中最多拥有一个 terminal。空 canonical context 可以接收一组 Main-owned Tab、Panel 与 Presentation；精确重试返回并 reconcile 同一个 durable Presentation 与 operation identity。任何其他既有 remote-context shape 都 fail closed。                                                                                                                                                         |
| D110 | `WorkbenchSharedDocument` 保持精确 version 2。新 Presentation 使用既有 provision intent，并以 canonical remote Directory path 作为 `launchCwd`；Environment Directory 仍为 `available: false`，不允许 local、Project、system 或 caller-supplied CWD fallback。                                                                                                                                                                                   |
| D111 | Terminal effect routing 只能从 durable Presentation containment 依次通过 Terminal Context、Workspace、primary Directory 与 Environment placement 推导。Local placement 只能到达 local Host；Environment placement 只能到达该 Environment 的 remote Host。Caller hint 与相同 path text 不提供 routing authority。Client 可以保留或删除精确 canonical remote state，但不能引入、移动、修改、复制或 retarget 它。                                   |
| D112 | Main 在进入 document mutation queue 前，为每个 Environment 至多准备一个缓存的进程内 remote Terminal runtime 与 relay。其 binding ledger 与 archive namespace 按 Environment 确定性隔离。已准入 POSIX target 使用固定 `/bin/sh`、canonical remote CWD 与有界 terminal default；local executable path、environment delta、shell integration、CLI endpoint、profile 与 secret 都不会复制到 remote process。                                         |
| D113 | Canonical document commit 先于每一个 Terminal Host create effect。只有确认 durability 后，reconciliation 才能发出 Host command。Queue admission 之前取消不会创建 document 或 terminal effect；单独准备 relay 不算 terminal effect。Commit point 之后取消不能回滚 intent、binding 或 remote outcome。                                                                                                                                             |
| D114 | Disconnected 或尚未准备的 target 会延后 durable intent，绝不 fallback 到 local Host，也不发布 synthetic terminal failure。Attachment loss 或未知 Host response 会保留 Environment-scoped binding 与 Creation identity。后续显式重复调用可以打开新 relay，并 reconcile Host epoch、inventory 与同一个 Creation identity；它绝不盲目创建第二个 terminal，也不会自动激活 SSH。                                                                      |
| D115 | SSH disconnect 与 Desktop shutdown 只 retire 进程内 relay/runtime attachment。它们不会删除 Project、Workspace、Terminal Context、binding identity 或 retained remote terminal，也不会发送 terminal stop。Remote projection、input、resize、metadata、exit publication、close、restart、split、archive capture 与 application-stop draining 保留给后续 slice。                                                                                    |
| D116 | 专用 TLA+ model 检查 target authority、singleton/idempotency、commit-before-effect、cancellation、deferred readiness、unknown-outcome reconciliation，以及没有 local fallback 或 duplicate create。确定性的 Workbench、reconciler、runtime-owner、Desktop composition、persistence、anti-forgery、文档配对与仓库门禁 evidence 对其进行 refinement。真实 SSH interoperability 及所有被排除的交互与 lifecycle capability 均不在本 slice claim 内。 |

## 已接受决策 D117-D126

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D117 | Slice 3.2C 授权 Electron 与已认证 Gateway Browser Client 对既有 singleton remote terminal 进行 live interactive projection。它不增加 renderer terminal-creation API、semantic CLI operation、Skill、Agent authority、额外 remote terminal、split、restart、close 或通用 remote command。                                                                                                                                                                               |
| D118 | Main 只能从 canonical Presentation containment 依次通过 Terminal Context、Workspace、primary Directory 与 placement 派生 projection target。Projection request 只提供 `TerminalPresentationId` 与 viewport geometry；caller hint、path text、Host identity 与 Environment identity 都不提供 routing authority。Local placement 只到达 local runtime，Environment placement 只到达该精确 Environment runtime。                                                          |
| D119 | Remote projection 要求精确 Environment runtime 已经 prepared 且 connected。Projection open 不会启动 SSH、等待 reconnect、激活 Environment、准备 relay 或 fallback 到 local Host。Unavailable target 返回既有 retryable projection failure；有界 surface recovery policy 只能重试 projection request。                                                                                                                                                                  |
| D120 | Runtime open 前的精确 target resolution 是 projection admission point。若 retained terminal 仍需要 start，该 runtime 可以用请求的 viewport geometry 发出 `StartTerminal`。后续删除不能回滚已准入 Host start，但 Main 在发布 lease 前重新解析 canonical placement；target 缺失或改变时会 dispose attachment，且不发布 projection。                                                                                                                                      |
| D121 | 已准入 remote projection 直接复用既有 attachment contract，不增加 remote variant：fresh `AttachmentId`、read/input/resize capability、input capability revision、有界 command admission，以及 serialized multi-client resize-lease ownership。Renderer 不会获得 HostConnection、Environment credential、binding ledger、SSH channel 或 remote path authority。                                                                                                         |
| D122 | Key、text、paste、mouse、focus、selection、search 与 resize command 保持既有 bounded projection semantics。Host admission outcome 变为 unknown 的 input 或其他 effect 绝不 replay。Electron 保持 Desktop-owned clipboard 与 allowlisted-link effect；Gateway Browser 保持同一 client-local effect target。Transport choice 不授予额外 side effect。                                                                                                                    |
| D123 | 每个 Environment projection 只使用其 cached Environment-owned runtime、HostConnection、binding ledger 与 recovery namespace。它不能检查或 fallback 到 local runtime、local binding 或 local terminal archive。Local projection path 与 archive behavior 保持不变。                                                                                                                                                                                                     |
| D124 | 本 slice 准入 attachment-scoped frame、bell、selection、search、clipboard request、transient title/CWD/theme observation，以及向 live surface 交付 terminal outcome。Durable remote metadata 与 exit publication、close、restart、split、archive capture、application-stop draining 与 reconnect reconciliation 仍为保留决策。                                                                                                                                         |
| D125 | Client release 只 detach 该 projection。SSH disconnect、Environment runtime retirement、renderer retirement 与 Gateway session disposal 会撤销其拥有的 projection lease，但不删除 Project、Workspace、Presentation、binding identity 或 retained remote terminal，也不发送 terminal stop。Open/release operation 继续由既有 per-Presentation 与 per-Environment owner 序列化。                                                                                         |
| D126 | 专用 TLA+ model 检查 canonical target authority、prepared-runtime admission、start/open/delete race、stale-lease suppression、disconnect retirement、no SSH activation 与 no local fallback。既有 projected-terminal reliability、broker-admission、attachment-capability、确定性 runtime-owner/router、Electron/Gateway composition、文档配对与仓库门禁 evidence 完成 refinement。真实 SSH interoperability 与所有保留的 durable lifecycle capability 不在 claim 内。 |

## 已接受决策 D127-D136

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                               |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D127 | Slice 3.2D 只授权既有 singleton remote terminal 的 durable metadata、terminal-failure 与 terminal-exit observation。它不增加 terminal creation、close、restart、split、archive、通用 command、CLI、Skill 或 Agent authority。                                                                            |
| D128 | Main 只订阅来自精确 Environment 当前 connection record 的 Terminal Host envelope。Protocol epoch validation 与 current-record fence 会在记录影响 runtime 或 Client state 前拒绝 stale 或 foreign record。                                                                                                |
| D129 | 每个 remote recovery、deduplication、binding 与 publication identity 都以 `EnvironmentId` 加 `TerminalIdentity` 作为 namespace。即使 raw Host identity 相同，它也不能与 local runtime 或另一个 Environment 冲突。                                                                                        |
| D130 | Title 与 current-working-directory metadata 按 Host revision 单调接受，通过精确 Environment runtime binding 解析，并由 `DesktopClientSessionCoordinator` 为 bootstrap 保留，且不修改 shared Workbench document。                                                                                         |
| D131 | 只有在精确 remote binding 解析到其 canonical Presentation 后才发布 `TerminalFailed`。SSH disconnect、relay retirement 与 unavailable runtime state 永不合成 terminal failure。                                                                                                                           |
| D132 | Non-acknowledgement terminal-exit envelope 必须在 Host listener 返回前进入精确 Environment 的同步 durable pending 或 rejection journal。无法保留该 outcome 时会 fail closed，而不是确认后丢失。                                                                                                          |
| D133 | Remote exit recovery 的 effect 顺序是 durable binding outcome、reconciler reservation、publication ledger commit、Client broadcast，最后是 pending-journal compaction。Retry 不能越过它的 durable predecessor 发布。                                                                                     |
| D134 | Canonical deletion 或 retargeting 不能重定向已准入 outcome。Missing、changed 或 unresolved authority 会保守地保留 Environment-scoped outcome 供后续 reconciliation，且永不发布给另一个 target。                                                                                                          |
| D135 | 已准入 outcome 在 relay loss 与 Desktop restart 后仍然存在，并在下一次显式 prepare 该精确 Environment runtime 时重新考虑。Recovery 自身永不 activate 或 reconnect SSH。                                                                                                                                  |
| D136 | 专用 TLA+ crash、retry、ordering 与 Environment-isolation model，以及确定性 owner、persistence、routing、coordinator 与 composition test 完成此 slice 的 refinement。Close、restart、split、archive capture、application-stop draining、UI creation、semantic CLI、Skill 与 Agent orchestration 仍保留。 |

## 已接受决策 D137-D146

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                     |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D137 | Slice 3.2E 只授权 Electron renderer 产品 Action 为既有 remote Workspace 创建或 reconcile 第一个 singleton terminal。Environment enrollment、SSH connect/reconnect/AskPass、Project admission、额外 terminal、group、split、restart、close、CLI、Skill、Agent 与 Gateway creation 仍不在范围内。                                                                |
| D138 | 复用既有 `workbench.terminal.create` Action、New Terminal control 与 shortcut。Local 行为不变。Remote Workspace 只在精确 Terminal Context 为空时准入 default placement；singleton 存在后，所有 terminal-creation surface 都不可用。                                                                                                                            |
| D139 | Renderer input 只包含 `ProjectId` 与 `WorkspaceId`。Main 从当前 canonical document 派生 `EnvironmentId`、canonical path 与 Terminal Context，并在 commit 时重新验证。Missing、stale、mismatched 或 local target 会拒绝，且不 fallback 到 local。                                                                                                               |
| D140 | Electron bootstrap 新增一个 purpose-specific `remote-terminal-creation` `MessagePort`。Preload bridge 把它作为第十个精确 bootstrap role 传递。该 capability 不通过 preload global、client document proposal、authenticated Gateway 或 general command transport 暴露。                                                                                         |
| D141 | Main 要求派生的 Environment 已注册且当前 connected。它可以 prepare 或复用该 Environment 的 Terminal Host runtime，但此 operation 永不 start、reconnect、等待 SSH，也不触发 SSH prompt。                                                                                                                                                                        |
| D142 | 只准入 default top-level placement。Concurrent 与 retried request 收敛到 canonical created 或 existing singleton，不能创建重复 terminal、Tab、Group、Panel 或 Presentation。                                                                                                                                                                                   |
| D143 | 只有 Main 确认的 pre-commit abort 返回 `cancelled`。Commit 后的 cancellation 或 unmount 不 rollback。Dispatch 后无法确认的 durability 与 control-port loss 返回 outcome-unknown result，而不是伪造 cancellation。                                                                                                                                              |
| D144 | Renderer state 永不 optimistic mutation。它等待 canonical client-session publication，且不假设 creation port 与 document publication 的顺序。Late completion 可以更新 status，但不能 navigate、select 或 steal focus。                                                                                                                                         |
| D145 | 只有具有 Electron capability 的精确 empty remote context 才启用该 Action。Capability 缺失、context 无效、placement 不支持或 singleton 已存在时，Tab Strip control、shortcut 与 Action execution 会一致禁用。Failure 有界且已本地化，不暴露 credential、path、raw transport error 或 log content。                                                              |
| D146 | 这个 Consequential 与 Modeled slice 先更新 TLA+ abstraction，再更新 protocol、broker、client、Action、availability 与 composition code。确定性 test 覆盖 cancellation、commit boundary、concurrency、retry、port retirement、stale authority、无 local fallback 与无 automatic SSH activation。完整 `pnpm verify` 排除 real SSH 与 manual native-window test。 |

## 已接受决策 D147-D156

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D147 | Slice 3.3A 授权 POSIX Electron Desktop 的 `Connect via SSH…` enrollment 与 recovery 入口，产品文案本地化为“通过 SSH 连接…”。一次显式请求会建立或复用 SSH Environment、连接它、验证一个 remote Project root，并准入 canonical Project 与 Workspace。macOS Desktop 与 Linux Desktop 可以连接已经准入的 macOS arm64 和 glibc Linux x86_64 remote platform；Windows Desktop、Gateway、Browser Preview、CLI、Skill 与 Agent enrollment 仍不在范围内。                                                                                                                                                      |
| D148 | 现有 Workbench Project-create 与 Project-navigation experience 新增 SSH entry、remote status，以及 Connect、Retry 与 Disconnect control。多步骤 credential 与 progress ceremony 是 `@orchbee/web` 中由 Desktop 拥有的新 business dialog；它不会把共享 resource form 变成 workflow，也不会分叉 Workbench Project model。                                                                                                                                                                                                                                                                               |
| D149 | Renderer enrollment input 只包含有界 Environment display name、SSH destination 或 alias、可选绝对 SSH configuration path，以及一个绝对 POSIX Project path。Main 验证边界、运行现有固定 `ssh -G` resolver，并分配 opaque identity。Renderer input 永不包含 Environment identity、Runtime identity、resolved hostname、SSH argument vector、shell fragment、credential 或 host key。精确既有 target 复用其不可变 identity 与 authoritative name。                                                                                                                                                       |
| D150 | Electron bootstrap 新增一个 purpose-specific `ssh-project-enrollment` `MessagePort`，作为第十一个精确 role。有界 request、generation、prompt-handle、progress、status、cancellation 与 result record 通过该 port。Main 对一个 Environment 最多授予一个 interaction lease；只有发起它的 request 可以回答精确 current prompt。Port loss 会撤销 lease，prompt 永不转交，提交的 secret 只是瞬时 AskPass response，不进入 React persistence、diagnostic、telemetry、disk 或 canonical document。                                                                                                           |
| D151 | `environment-registry-v1.json` 保持不变，继续作为 append-only identity 与精确 target owner。Canonical Workbench Environment placement 是 Project 已 enrollment 的唯一 durable proof。仅存在于 registry 的 record 是可恢复 pending identity，包括 legacy record 以及 cancellation 或 failure 留下的 record。Slice 3.3A 不新增第二个 lifecycle journal、registry delete、rename、repoint、retirement、remote cleanup 或 alias merging；精确 target 始终复用已准入 identity。                                                                                                                            |
| D152 | Main 按 target resolution、durable Environment create 或 reuse、SSH/AskPass/Runtime readiness、canonical Project-root inspection、canonical Workbench commit，最后 result 与 client-session publication 的顺序执行。Workbench commit 前确认的 cancellation 会停止 request，同时保留任何 durable registry identity；一旦 commit 可能已发生，cancellation、unmount 或 port loss 不能回滚。Registry 与 Workbench write 不宣称 atomic：indeterminate result 会 fail closed，restart 或显式 reload 会 reconcile 各自独立 durable fact。                                                                    |
| D153 | Desktop restart 永不 activate SSH。每个已 enrollment 的 remote Project 从 Environment connection observation 派生 process-local status，并提供显式 Connect 或 Retry。已经证明的 live-connection loss 继续使用现有 generation-fenced automatic retry；replacement generation 在没有 live renderer lease 时若需要 interaction，会 recoverably block，直到显式请求提供 lease。Disconnect 只 retire local attachment/runtime resource，并保留 Environment、Project、Workspace 与 retained remote work。                                                                                                   |
| D154 | 新 enrollment 成功后，只在 canonical publication 之后于发起 Client View 中展开并 activate 已准入 Workspace。其他 client 收到 canonical Project，但没有 focus 或 navigation effect。Enrollment 不创建 terminal；现有 Electron New Terminal operation 仍是唯一 first-terminal 入口。                                                                                                                                                                                                                                                                                                                    |
| D155 | 普通 development 与普通 Desktop package 继续不含 release，并以已本地化 build-unavailable status fail closed。Real activation evidence 只使用现有显式 release package path，其中包含 finalized signed two-component cohort 与独立提供的 Ed25519 public key。Slice 3.3A 不新增 unsigned release、ambient artifact lookup、private signing key、download feed 或 development trust bypass。                                                                                                                                                                                                              |
| D156 | 这个 Consequential 与 Modeled slice 在 dependent production behavior 之前持久化并检查 `SshEnvironmentProjectEnrollment`。确定性 evidence 覆盖 parsing、identity reuse、owner serialization、prompt correlation 与 secrecy、cancellation 与 port retirement、status projection、reconnect、Workbench commit 与 publication ordering、localized UI，以及 restart reconciliation。受控 loopback `sshd` evidence 还覆盖 host-key confirmation、encrypted-key AskPass、signed Runtime upload、Project admission、terminal I/O 与 network interruption；完整 `pnpm verify` 排除 manual native-window test。 |

### SSH Project enrollment 模型

| 建模字段         | 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题             | Desktop 用户目前只能通过 process-local method 使用所有底层 SSH 与 remote-Project operation，却没有负责 identity establishment、authentication、progress、Project admission、failure recovery 或 reconnect 的产品路径。                                                                                                                                                                                                                                                                 |
| 输入             | Renderer 提供一个 request identity，以及 D149 中仅有的四项有界用户 intent。Main 解析每条 record，通过 OpenSSH 解析精确 target，为新 target 分配 Environment 与 Runtime identity，并派生全部 existing identity、resolved fact、canonical path 与 connection generation。                                                                                                                                                                                                                |
| 数据与 ownership | Environment registry 拥有 immutable identity 与精确 target intent。Canonical Workbench document 拥有已 enrollment Project 与 Workspace structure。Connection owner 拥有 process-local status 与每个 Environment 一个 supervisor。Enrollment coordinator 拥有 request serialization 与瞬时 interaction lease。只有发起 Client View 拥有 publication 后的 expansion 与 activation。任何 lifecycle projection 都不能成为第二个 durable authority。                                        |
| 状态             | Request 依次经历 target resolution、registry create/reuse、pending identity、connection 或 reuse、可选的精确 AskPass interaction、connected Project-root inspection、Workbench commit、settlement、canonical publication，以及 initiating-view activation。Registry 与 Workbench commit 分别允许 pre-write failure 与 post-rename uncertainty。Connection loss、renderer retirement、显式 retry/disconnect 与 application restart 可以独立排序。                                       |
| 不变量           | Remote effect 需要 durable Environment；Workbench enrollment 蕴含该 Environment；registry-only pending identity 永不发布为 enrolled Project；一个 Environment 最多有一个 active request 与一个精确 prompt lease；stale 或 foreign prompt response 不能进入 AskPass；cancellation 永不回滚可能已 committed 的 Project；restart 永不 auto-connect；只有 committed initiating request 可以 activate 自己的 Client View；enrollment 永不创建 terminal。                                    |
| Commit/recovery  | Environment commit 先于所有 remote effect，并在后续 failure 时保留。Workbench canonical commit 是 Project-enrollment commit point。已知 pre-commit failure 会根据 registry outcome 保持 pending 或 absent。Indeterminate registry 会阻止后续使用直到 reload；indeterminate Workbench write 会从 store reconcile，之后新的 request 才能声称 created 或 absent。Startup reload 两个 owner，并且只从已验证 canonical placement 派生 enrollment。                                          |
| Cancellation     | Target resolution 与 pre-commit workflow phase 可以以确认的 cancellation 停止。没有 enrolled Project 的 operation-owned connection 会被 retire；pre-existing shared connection 会保留。Workbench commit 期间或之后，cancellation 与 port loss 只移除 request delivery/interaction lease，而 Main 会完成或稍后 reconcile authoritative commit。                                                                                                                                         |
| Security/privacy | Renderer 与 Workbench 永不接收 Host connection、release byte、Runtime identity、SSH argument、resolved target、environment capability、raw diagnostic 或 secret。Prompt text 是有界 untrusted display data；response 仅针对精确 request、generation 与 handle 接受，随后直接发送到 owner-only AskPass bridge。Status 与 failure 使用有界 enum。                                                                                                                                        |
| 验证             | `SshEnvironmentProjectEnrollment` 检查 non-atomic registry/Workbench ordering、pending identity、connection 与 prompt lease exclusion、stale interaction rejection、busy request、pre/during/post-commit cancellation 与 port retirement、indeterminate reconciliation、显式 reconnect/disconnect、无 prompt lease 的 automatic retry、restart、initiating-view activation，以及无 terminal effect。现有 connection、registry、admission 与 first-terminal model 仍是独立 refinement。 |

### Renderer remote-terminal creation surface model

| 建模字段           | 契约                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题               | Electron 用户在已接纳 remote Workspace 中需要通过普通 New Terminal interaction materialize 已授权的第一个 remote terminal，同时不能获得 Environment、SSH、Project、任意 placement 或 general remote-command authority。                                                                                                                                                                                       |
| 输入               | Renderer 提供一个 request identity 以及精确 `ProjectId` 与 `WorkspaceId`。Main 读取当前 canonical document、Environment registry 与 current connection observation。Placement、Environment identity、canonical remote path 与 context shape 永远不是 renderer input。                                                                                                                                         |
| 数据与所有权       | Workbench 拥有 creation availability 与 structural singleton intent；canonical shared document 拥有 Project、Workspace 与 Terminal Context authority；Desktop Main 拥有 target derivation、connection admission、commit/reconciliation 与 dedicated request lifecycle；Client View 拥有 navigation 与 focus，且不是 creation-workflow writer。                                                                |
| 输出               | Main 以一个有界 `created`、`existing`、`cancelled`、已知 `failed` 或 outcome-unknown result 结算。Created 与 existing result 标识 canonical singleton。Renderer-visible terminal state 只通过普通 client-session canonical publication 到达。                                                                                                                                                                 |
| 状态               | Request 依次经过 client availability、dispatch、单个 Main admission、canonical target resolution、connected-runtime preparation、commit-or-existing reconciliation、settlement 与独立 canonical publication。Port retirement 可能发生在 admission 前、commit 前、不确定 commit 期间或 commit 后。                                                                                                             |
| 不变量             | Main 从当前 canonical Workspace 派生 Environment；remote creation 永不到达 local runtime；每个 Workspace 至多创建一个 canonical terminal；只有显式 SSH activation 才能增加 connectivity；renderer visibility 蕴含 canonical publication；creation completion 永不改变 active location；只有已确认的 pre-commit abort 才报告 cancellation。                                                                    |
| 决策               | Default placement 意味着不请求 Directory、Group、split 或 insertion index。Tab Strip 的 New Tab、New Group with Tab 与 Duplicate Tab projection 共享同一个 creation-availability policy，因此 remote context 不存在替代 Tab-creation path。既有 structural lifecycle proposal 继续受 Main remote-placement proposal fence 保护。                                                                              |
| Commit/publication | Canonical mutation 仍是 authoritative commit。Host reconciliation 成功不能让 renderer state 成为 authority。Commit 与 client-session publication 独立排序；late result 只影响 status，而 canonical reconciliation 会选择必要的 first-Tab validity fallback，且 workflow 不定向改变 focus。                                                                                                                    |
| Cancellation       | Canonical commit 前被观察并确认的 abort 可以结算为 `cancelled`。一旦 commit 可能已经发生，abort、unmount、broker disposal 或 port loss 都不能 rollback，而是结算或被保守观察为 outcome unknown。Fresh request 可以在 replacement port 上 reconcile 同一个 singleton。                                                                                                                                         |
| 依赖               | `@orchbee/desktop-client-protocol` 拥有有界 Electron wire contract；Desktop Main 拥有 broker 与 canonical resolver；`@orchbee/workbench` 拥有 host capability 与 Action route；`@orchbee/tab-strip` 拥有一致的 creation-control availability；既有 SSH 与 Terminal Host owner 保持不变。                                                                                                                      |
| 验证               | `SshEnvironmentRemoteTerminalCreationSurface` 检查有界 cancellation、commit、retry、concurrency、stale-target、port-retirement、publication、focus、无 local fallback 与无 auto-connect trace。确定性 protocol、broker、client、resolver、Workbench runtime、Action、Tab Strip、preload 与 composition test 形成 executable refinement chain。Real SSH、network quality 与 manual native-window test 被排除。 |

## Runtime 拓扑

```text
Desktop supervisor
  ├─ owner-only AskPass IPC → transient human interaction
  └─ one OpenSSH stdin/stdout child
       └─ fixed remote bootstrap / attach
            └─ owner-only Unix socket
                 └─ per-Environment Runtime daemon and epoch
                      ├─ bounded project-root-v1 inspection adapter
                      └─ one terminal-host-v1 opaque relay worker
                           └─ same-UID private endpoint
                                └─ retained Terminal Host daemon and Host epoch
```

Desktop connection owner 对每个 durable Environment 只准入一次显式 activation，并为这个 active identity
拥有一个 supervisor。Generation 拥有 target resolution、release-catalog admission、AskPass、OpenSSH、
installed-cohort negotiation、handshake 与 heartbeat。它不拥有任一 daemon 生命周期。Runtime 精确按顺序
公布 `project-root-v1` 与 `terminal-host-v1`；前者由 Runtime 解释，后者在不解析 Terminal Host record 的
前提下 opaque relay。

## Target resolution 与 OpenSSH transport

`SshEnvironmentTargetReference` 包含一个具体 destination 或 alias，以及至多一个由用户显式选择的绝对
configuration path。Resolution 不通过 shell 调用：

```text
/usr/bin/ssh -G [-F <absolute-config-file>] -- <destination>
```

有界结果仅准入一个有效 hostname、user、port、host-key lookup name 与
`StrictHostKeyChecking` value。接受 `yes`、`ask` 与 `accept-new`；关闭检查会 fail closed。OpenSSH 仍是
SSH configuration 与 `known_hosts` 的唯一 authority。

Transport adapter 随后用 argument array 启动同一个绝对 executable。它固定已解析的 host identity，并要求
`-T`、`RequestTTY=no`、`ClearAllForwardings=yes`、`ForwardAgent=no`、`ForwardX11=no`、
`ForkAfterAuthentication=no`、`StdinNull=no`、`Tunnel=no`、`GSSAPIDelegateCredentials=no`、
`PermitLocalCommand=no`、`ControlMaster=no`、`ControlPath=none`、`ControlPersist=no`、15 秒
server-alive probe 以及三次 missed probe。它不传入 `-D`。所选 SSH configuration 仍可拥有 identity
file、agent、ProxyJump/ProxyCommand connectivity、authentication method 与 `known_hosts` interaction。

Stderr 仅作为有界内存 diagnostic tail 保留。它永不用于协议，不作为产品 payload 返回，也不会由此子系统
持久化。在 Hello 前，固定 bootstrap exit code 用于分类 compatibility 与 integrity failure；瞬时 AskPass
prompt kind 与有界常量匹配用于区分 authentication 或 host-verification failure 和可重试 network loss，
但不会暴露 diagnostic text。

## 已验证的 bootstrap 与安装

Schema-version-2 release catalog 为两个已准入 platform 各包含按顺序排列的 Runtime 与 Terminal Host。
Desktop 在 OpenSSH 前认证精确 canonical manifest、signer、cohort digest、size 与 digest。Installed fast
path 不读取 artifact bytes；`upload` status 只会 lazy load 并重新验证对应 component。

同一个 POSIX bootstrap 检测 macOS arm64 或 glibc Linux x86_64，然后依次发送精确且最大 128-byte 的 ASCII
`OBSI/2 <platform> ssh-environment-runtime <present|upload>` 与
`OBSI/2 <platform> terminal-host <present|upload>`。Desktop 只接受这个 role order 与一致 platform。两个
精确 artifact 都存在后，bootstrap 才发送 `OBSC/2 <platform> ready`；只有此后 Desktop 才能发送 Hello。
v1、missing 或 duplicate role、reorder、mixed platform，以及 commit barrier 后的 bytes 都 fail closed。

每个 `present` file 必须是 digest 精确匹配的非 symlink regular file。每个 `upload` 会读取 signed byte count、
在远端验证 SHA-256，并通过 no-clobber hard link 发布到：

```text
~/.orchbee/ssh-runtime/versions/<version>/orchbee-ssh-environment-runtime
~/.orchbee/terminal-host/versions/<version>/orchbee-terminal-host
```

Runtime 与 Host state 分别位于 `~/.orchbee/ssh-runtime/state` 和
`~/.orchbee/terminal-host/state`。Bootstrap 绝不调用 `curl` 或 `wget`、overwrite installed artifact、选择
remote `PATH` binary 或创建 mutable `current` link。并发相同 installer 通过重新验证 winner 来 converge；
任何 mismatch 都在 Runtime 启动前阻止整个 cohort。Bootstrap 随后用 cohort 与 Host metadata 启动精确
Runtime，等待私有 socket，再在同一个 OpenSSH stdio stream 上 `exec` `attach`。

Host endpoint identifier 是 Runtime instance 与 release cohort 的 domain-separated digest，不是 credential
或 Runtime identity。Runtime 只连接其 same-UID private endpoint。Endpoint 不存在时，隔离 relay worker 只用
固定 `serve --runtime-directory ... --endpoint-id ...` ABI 启动 signed Host 精确路径；它永不 kill 或 replace
现有 Host。

Release-only Desktop package 提供已验证 cohort 与精确 manifest-pinned trust descriptor。其 private-key
exclusion、外部 signing ceremony、canonical layout、public-key injection、rollback 与 package placement 维护在
[`ssh-runtime-release.md`](ssh-runtime-release.zh-CN.md)。`DesktopApplication` 现在把该 source 与按
Environment 建立的 connection owner 组合起来；不包含 release 的 development 与 ordinary package 会以
`runtime-release-unavailable` 阻止 activation。

## AskPass 安全边界

每个 generation 中，`SshAskPassBridge` 创建新的 `0700` 临时目录、`0600` Unix socket、`0700` 固定
launcher 与随机 256-bit capability。它设置 `SSH_ASKPASS_REQUIRE=force`；helper 通过私有 socket 发送一个
有界 JSON request 并接收一个有界 answer。每次至多存在一个 active prompt，每个 generation 至多准入八次
prompt。

Prompt text 是不受信的 OpenSSH input。只有 opaque handle、prompt kind 与 transient prompt 会交给注入的
Main interaction owner。Response 被直接写给等待的 AskPass process，随后释放。Password、passphrase、OTP、
prompt 与 answer 绝不进入 `SshEnvironmentConnectionState`、diagnostics、telemetry 或 durable storage。
Cancellation 会 retire generation。已交付的安全 bridge 仅支持 POSIX；在 Windows Desktop 启用该路径前，
仍需 Windows ACL-backed implementation。在组合 Main interaction handler 前，每个 prompt 都会被 cancel，
且不会暴露给 renderer。

## 协议、identity 与 heartbeat

每个 protocol record 都有 16-byte `OBSE` header、精确 `0.2.0` version bytes、kind、big-endian payload
length 与四个 reserved zero bytes。完整 record 上限为 1 MiB。Control payload 是严格、有界的 UTF-8 JSON；
channel data 是 binary；malformed、unknown、partial 或 oversized input 都会 fail closed。

在固定 cohort commit 后，Desktop 发送一个 Hello；Hello 包含预期 Runtime instance identity、随机 client
nonce、frame bound 与 initial credit。只有 Hello-Accepted 返回精确 instance、Runtime version 与 digest、
release cohort digest、Terminal Host version 与 digest、selected platform、精确有序
`[project-root-v1, terminal-host-v1]` capability 及有效 Runtime epoch 后，才可能进入 `connected`。Bootstrap
status 本身永远不能建立 readiness。State observation 仅保留该 epoch 与不含 secret 的连接 metadata。

Desktop 立即启动 heartbeat，并采用 Runtime 公布的 15 秒 interval 与三次 miss limit。Runtime 对精确 epoch
与 sequence 进行 acknowledgement。Mismatch 属于 protocol failure；missed acknowledgement 会 retire
transport 并进入现有有界 reconnect policy。Runtime 也会限制 incomplete record 和始终未完成 Hello 的
attachment。

## Project-root inspection service

`DesktopApplication.inspectSshEnvironmentProjectRoot` 委托给按 Environment 建立的 connection owner。它既不
调用 `connect`，也不等待 retry：inactive、reconnecting、stopped 或 disposed Environment 会返回
`unavailable`。有效调用固定到当前 Runtime epoch，分配新的 channel identity，并遵循
`open → opened → request → result → close → closed`。每个 Environment 可以同时执行四个调用；第五个返回
`busy`。

Protocol codec 只准入符合 D59 边界的 `{"type":"inspect-project-root","path":"/..."}`。Runtime work 在
immutable Runtime binary 的可终止 child 中执行，path 只通过 stdin 传递。Attachment loop 独立轮询 worker
output 与 heartbeat。Worker 会跟随 symlink，将 canonical target 作为 directory 打开，并返回 canonical
UTF-8 path 或一个 D64 failure reason。过程中不使用 shell，也不进行 expansion。

`AbortSignal` 会在本地以 `cancelled` settle，并发送 channel cancellation。即使仍在等待 close
acknowledgement，caller deadline 也会在 30 秒内以 `timed-out` settle。Attachment loss 会把 outstanding
call 以 `unavailable` settle；supervisor 可以 reconnect，但绝不 replay 这些 channel。Malformed service
result 只会以 `protocol-failure` 关闭对应的有效 channel；epoch、unknown-channel、order、framing 或 credit
violation 会 retire attachment。Path 永远不会复制到 connection observation、registry record、diagnostic、
telemetry 或 log。

## Terminal Host relay service

`DesktopApplication.openSshEnvironmentTerminalHost` 要求一个已连接 Runtime epoch，绝不 activate SSH。一个
新的全局单调 channel 打开 `terminal-host-v1`；四个 project-root channel 之外至多存在一个 relay。Runtime
只有在隔离 worker 已认证并连接 same-UID Host endpoint，或启动并连接 signed Host 精确 executable 后，才发送
`channel-opened`。

打开后，Runtime 把所有 channel-data bytes 视为 opaque Terminal Host protocol。双向各有精确 64 KiB credit，
每个 chunk 非空且至多 64 KiB。Worker 接受的 bytes 会补回 Desktop input credit；Desktop `HostConnection`
消费的 bytes 会补回 Host output credit。Blocking endpoint discovery、launch 与 Host I/O 位于 attachment loop
之外，因此 heartbeat 与 project-root progress 相互独立。

Main 在 relay 上构造 `HostConnection`，并要求协商 Host version 等于 signed cohort。Busy、unavailable 与
incompatible Host outcome 只关闭对应 channel。无效 outer framing、epoch、ordering、identity、chunk 或 credit
会 retire attachment。Attachment loss 会销毁 broker stream，并使 pending Host effect 成为 outcome-unknown；
任何 effect 都不 replay。新 attachment 分配新 channel，并通过 Terminal Host 拥有的 Host epoch 与 inventory
reconcile retained terminal。

## 远程 Project 准入

`DesktopApplication.admitSshEnvironmentRemoteProject` 接收一个 `EnvironmentId` 与 requested path。它先解析
durable registry record，再要求 Environment 已有 connected observation，调用精确 `project-root-v1`
inspection，最后排入一个由 Desktop Main 拥有的 canonical Workbench mutation。它绝不启动 SSH connection，
也不等待 connection。Inspection result 排入队列前发生 cancellation 或 attachment loss，不会创建产品状态。

全局 admission key 是返回的 `(EnvironmentId, canonicalPath)`。Key 缺失时分配随机 128-bit Directory
identity 与下一个 Workbench Project identity；Project name 来自 canonical path 的最后一段。等价并发或重试
admission 返回同一 Project。已 commit 的 version-2 document 包含一个 unavailable remote Folder Directory、
一个选择该 Directory 的 inactive Workspace，以及一个 empty Terminal Context。Requested spelling 与失败
observation 不进入 durable state、diagnostic、telemetry 或 log。

Canonical result 一旦进入 mutation queue，caller cancellation 不再 rollback，后续 disconnect 也不使这一
point-in-time observation 失效。Rename 前 write failure 是安全否定结果；可能发生在 rename 后的 failure 为
`indeterminate`：coordinator 会冻结后续 mutation，restart 加 retry 通过 durable admission key reconcile。

只有 Desktop Main 可以引入 remote placement。Client proposal 可以保留或删除精确的 remote Directory、
Workspace 与 context，但不能引入、更改、reparent、relabel、duplicate 或把它们设为 available。Desktop 会对
registry 校验每个保留的 opaque Environment reference。移除 Project 只删除产品 metadata；它不会执行 remote
delete、listing、file、Git、shell、terminal stop、Agent、synchronization、renderer creation 或 CLI effect。

## 远程 terminal provision

`DesktopApplication.createSshEnvironmentRemoteTerminal` 接收一个 `EnvironmentId`、Project ID 与 Workspace
ID。它要求 Environment 已注册、有 already-connected observation，并且 Workspace 的 primary Directory 位于
该精确 Environment。它绝不调用 `connect`、等待 reconnect、接受 path hint，或更改 active/expanded Client
View state。

进入 canonical mutation queue 前，Main 为每个 Environment 至多准备一个缓存的 Terminal runtime 与一个
relay。Binding ledger 与 archive directory 使用确定性的 Environment-isolated namespace。Remote launch 固定为
`/bin/sh`，仅使用有界 terminal default 以及 `COLORTERM`、`TERM`、`TERM_PROGRAM`，并把 canonical remote
Directory path 作为 `launchCwd`；不会传输 local executable、CWD fallback、environment delta、shell
integration、CLI endpoint、profile 或 secret。

Empty remote context 会得到精确一个 Main-owned Tab、Panel 与 provision Presentation。Workspace 是
idempotency key：精确 retry 会复用同一 Presentation 与 operation identity；其他任何 pre-existing shape 都会
fail closed。Canonical version-2 commit 先于 effect reconciliation。Target selection 只能从 Presentation
containment 依次通过 Terminal Context、Workspace、primary Directory 与 placement 推导，因此 local placement
只能到达 local Host，Environment placement 只能到达该 Environment 的 Host。

Unavailable runtime 或 retryable/unknown Host outcome 会让 durable intent 保持 pending，不发布 synthetic
failure，也不缓存 settlement。后续显式重复调用会打开或复用 relay，并 reconcile 同一个
Environment-scoped binding 与 Creation identity；它绝不 fallback 到本地，也不会盲目创建第二个 terminal。
Client proposal 只能应用精确的 `provision → retain` convergence，或删除受保护 remote state。Disconnect 与
shutdown 会 dispose 进程内 runtime/relay，而不删除产品状态或发送 terminal stop。Close、restart、split、
archive capture 与 application-stop draining 仍不在本 slice 范围内。

## Electron 远程 terminal creation surface

普通 `workbench.terminal.create` Action、shortcut 与 New Terminal control 通过一个由 Workbench 拥有的
availability policy 路由。Local placement 保持既有 local command。处于精确 empty remote Workspace 的
Electron renderer 只能请求 default top-level first terminal；singleton 已存在或 capability 缺失后，New
Group with Tab、Duplicate Tab、显式 Group 或 insertion placement，以及全部 creation entry point 都不可用。

Renderer 只通过第十个专用 `remote-terminal-creation` bootstrap port 发送 correlated request identity、
`ProjectId` 与 `WorkspaceId`。Desktop Main 从当前 shared document 派生 Environment、canonical Directory
path 与 Terminal Context，要求 Environment 已登记且当前已连接，并在 canonical commit 前后重新校验
target。Broker 可以 prepare 或复用该 Environment 的 Terminal Host runtime，但绝不启动、重连、等待或提示
SSH。该 capability 不是 preload global，也不提供给 Gateway、Browser、semantic CLI、Skill 或 Agent caller。

Renderer 不执行 optimistic document 或 Client View mutation。`created` 或 `existing` settlement 只有在独立
排序的 client-session stream 发布精确 canonical Project、Workspace、Context、Tab、Panel 与 Presentation
identity 后才完成。Completion 永不 navigate、select 或 steal focus。只有 Main 确认 commit 前发生的 abort
才是 `cancelled`；dispatch 或可能 commit 后的 abort、unmount、broker retirement 或 control-port loss 都是
有界 failed/outcome-unknown result。Replacement port 上的 fresh request 会 reconcile singleton，而不会创建
duplicate 或 fallback 到 local runtime。

## 远程 terminal projection 与 durable observation

Electron 与 authenticated Gateway client 通过和 local terminal 相同的 attachment protocol project 既有
remote Presentation。Main 只从 canonical containment 派生 target，并要求该 Environment runtime 已经
prepared。Projection 不会 activate SSH、prepare relay、等待 reconnect，也不会使用 local binding/archive
fallback。Main 会在 open 后重新检查 target；若 Presentation 已删除或 retarget，则 dispose provisional
lease。

Environment runtime owner 会在打开 remote `HostConnection` 前，先加载该 Environment 的
`terminal-exits-v1.json` 与 `terminal-exit-publications-v1.json`，然后安装 current-record 与
Host-epoch-fenced envelope listener，之后才开始 runtime reconciliation。Binding ledger、pending/rejection
journal、publication ledger、retry owner 与 identity association 都位于同一个确定性的 Environment
directory 下。因此，即使 local runtime 或其他 Environment 使用相同 raw `TerminalIdentity`，它们仍彼此
隔离。

Metadata 只接受递增的 Host revision，通过 Environment runtime binding 解析，并进入
`DesktopClientSessionCoordinator` 为 bootstrap 保留的 state，而不修改 shared document。
`TerminalFailed` 通过该 binding 记录，并且只有当其 Presentation 仍解析到该精确 Environment 时才发布。
Relay loss 与 runtime unavailable 会 retire observation owner，永不合成 terminal failure。

每个 `TerminalExited` envelope 都会在 Host listener 返回前同步持久化到 Environment pending journal；若
pending admission 不可用，则进入 durable rejection journal。Reconciliation 随后依次持久化 binding
outcome、reserve Presentation outcome、commit runtime 与 Environment publication evidence、commit
reconciler event、broadcast，最后才 compact pending journal。Association 缺失、canonical deletion 或
retargeting 会停放原始 Environment-scoped event；它不能发布到另一个 target。瞬时本地
persistence/publication failure 可以 retry，但 recovery 永不连接 SSH。Relay loss 或 Desktop restart 后，
只有后续显式 operation prepare 该精确 Environment runtime 时才会重新加载 durable journal。

## 连接生命周期与恢复

| 当前 phase             | 事件                               | 下一 phase 与 effect                                                   |
| ---------------------- | ---------------------------------- | ---------------------------------------------------------------------- |
| `stopped` 或 `blocked` | 显式 start                         | 新 generation 进入 `resolving`。                                       |
| `resolving`            | 精确 target 已解析                 | 进入 `connecting`；supervisor 验证 release catalog，再打开 transport。 |
| `connecting`           | AskPass request                    | 进入 `interaction-required`；只有匹配 opaque handle 才能恢复。         |
| `connecting`           | 精确 Hello accepted                | 进入 `connected`；Runtime epoch 可观察。                               |
| `connecting`           | 初始可重试失败                     | 进入 `blocked`；第一次成功连接前不启动 unattended retry。              |
| `connected`            | transport loss 或 missed heartbeat | retire 旧 generation；新 generation 进入 `retry-wait`。                |
| `retry-wait`           | 有界 jitter elapsed                | 再次解析 target，并打开 replacement child。                            |
| 任意 active phase      | stop                               | retire 并 fence 本地 generation resource；不停止远端 daemon。          |
| 任意 phase             | stale completion                   | 不改变 state，也不授予 authority。                                     |

Retry 使用 equal jitter：首次 500-1,000 ms，倍增至 30 秒上限，并且只有一个 timer。连到幸存 daemon 的
replacement attachment 会观察到同一个 epoch。如果 daemon 已死，bootstrap 启动精确的已安装版本，Hello
报告新 epoch。这个差异是未来 reconciliation fence。Project-root inspection 是 read-only 且绝不 replay。
丢失的 Terminal Host relay 同样绝不 replay；后续显式 open 会针对 retained Host state 协商新的 broker stream。
协议仍不包含 Git、generic command 或 Agent effect。

`DesktopSshEnvironmentConnectionOwner` 按 `EnvironmentId` 串行化 connect 与 disconnect。第一次 connect 会把
registry target 与 Runtime instance identity 复制到一个 supervisor；重复 connect 会复用它。Disconnect 只在
retirement 完成后移除该 supervisor，application shutdown 会先 fence 新 activation，再停止所有剩余
supervisor。Registry load、create 与 application startup 永远不会调用 connect。

Projection request 只携带 `TerminalPresentationId` 与 viewport geometry。Desktop Main 通过 Terminal Context、
Workspace、primary Directory 与 placement 解析 Presentation containment。Local placement 只会到达 local
runtime；Environment placement 只会到达为该精确 Environment 缓存且已经 prepared 的 runtime。Main 在 runtime
open 后重复解析，并在 target 缺失或变化时 dispose provisional lease。Projection 永远不会 prepare 或 reconnect
SSH，Environment 路径还会禁用 local binding 与 terminal-archive fallback。

## 建模与验证

十一个已登记的 TLA+ model 分担职责：

- `SshEnvironmentConnection` 检查 generation exclusion、interaction、retry admission、blocking、stop
  fencing 与 retry progress。
- `SshEnvironmentRuntimeAttachment` 检查 trusted-catalog-before-transport ordering、present 时不读取
  artifact、ordered dual-component upload、atomic cohort commit、精确 installed digest、并发 no-clobber
  convergence、mismatch rejection、单个 current attachment、完整 cohort Hello/epoch readiness、保留 daemon
  identity、competing-attachment rejection、精确 service authority 以及不 replay unknown effect。
- `SshEnvironmentTerminalHostRelay` 检查单 relay、全局 channel identity、有界双向 credit、隔离 Host setup/I/O、
  channel-local failure、attachment loss、retained Host epoch、reconnect reconciliation 与 no replay。
- `SshEnvironmentProjectRootService` 检查精确 capability admission、channel order 与 credit、有界 capacity、
  timeout、cancellation、attachment loss、no replay、stale-result fencing 与 worker progress。
- `SshEnvironmentRemoteProjectAdmission` 检查 registry/connectivity/inspection/mutation ordering、
  cancellation boundary、全局 canonical-key idempotency、commit order 与 indeterminate-write reconcile。
- `SshEnvironmentProjectEnrollment` 检查 registry-before-effect 与独立 durable Workbench ordering、pending
  identity、按 Environment 的 request 与 prompt-lease exclusion、stale interaction rejection、commit point
  前后的 cancellation 与 port retirement、显式 reconnect/disconnect、无 prompt lease 的 automatic retry、
  restart、initiating-view activation，以及无 terminal effect。
- `SshEnvironmentRemoteTerminalProvision` 检查 target authority、singleton Workspace idempotency、
  commit-before-effect、cancellation、deferred readiness、unknown-outcome reconciliation，以及无 local
  fallback 或 duplicate create。
- `SshEnvironmentRemoteTerminalProjection` 检查 canonical Presentation target authority、prepared runtime
  admission、local-versus-Environment isolation、start/open/delete race、stale-lease suppression、disconnect
  retirement、无 SSH activation，以及无 local fallback。
- `SshEnvironmentRemoteTerminalOutcome` 检查 current-record subscription、按 Environment 隔离 identity、
  monotonic metadata、non-synthetic failure、同步 exit admission 或 rejection、
  binding/reservation/ledger/publication/compaction ordering、retargeting、retry、owner restart，以及不
  activate SSH 的 recovery。
- `SshEnvironmentRemoteTerminalCreationSurface` 检查 renderer admission、Main-derived target authority、
  singleton convergence、cancellation boundary、port retirement、独立排序的 canonical publication、focus
  preservation、无 local fallback，以及无 automatic SSH activation。
- `WorkbenchDocumentPlacementMigration` 检查精确 v1-to-v2 local placement migration、v1 source 保持、
  v2-present authority 与无 reverse migration。

可执行 evidence 包含 strict protocol 与 stream-decoder test；canonical release-contract、signed-manifest
mutation、wrong-signer、exact-layout、symlink 与 package-staging test；shell syntax、fixed-status decoding、
ordered dual-component installed/present/upload/commit/mutation/concurrent-install 本地 process check 与
no-download assertion；OpenSSH security-floor argument；owner-only AskPass IPC；Environment owner
serialization 与 shutdown；supervisor orchestration；全部 Desktop unit test；原生 Runtime unit 与
real-process attachment test，包括 canonical path、failure behavior、实际 Terminal Host launch、opaque Host
Hello relay 与 credit return；以及两个已准入远端 target 的 cross-compilation。Workbench 固定种子 property
覆盖重复 remote registration；Desktop test 覆盖 file migration、v2 authority、registry cross-reference
validation、proposal anti-forgery、concurrent admission、cancellation、indeterminate restart/retry、隔离的
remote runtime reuse、commit-before-effect、pending-intent retry，以及精确 provision-to-retain convergence。
这些 test 还覆盖 canonical target resolution、local/Environment routing、stale-target lease disposal、
relay-close fencing、archive-fallback suppression，以及 Electron 与 authenticated Gateway projection
composition。Environment observation test 还覆盖 listener-before-runtime initialization、Host-epoch 与
current-record fencing、同步 durable exit admission、精确 publication ordering、retry 与 restart、相同
identity 的跨 Environment 隔离、monotonic metadata、non-synthetic failure、retarget retention，以及不
fallback 到 local runtime 的精确 remote outcome composition。Electron creation-surface test 还覆盖专用
protocol 与 bootstrap role、broker concurrency 与 retirement、stale Main authority、connected-only
admission、无 automatic SSH activation、renderer request correlation 与 outcome-unknown handling、无
optimistic mutation 的 canonical-publication waiting、Action cancellation boundary，以及 shortcut 与 Tab
Strip creation control 共用一个 availability policy。Enrollment-surface evidence 还覆盖第十一个 bootstrap
role、严格 request/progress/prompt/status/result parsing、精确 interaction-lease ownership、registry 与
Workbench commit boundary、cancellation 与 port retirement、status projection、Connect、Retry、Disconnect、
仅发起 client navigation、本地化 dialog behavior，以及不会替换 identity 的 publication-timeout recovery。
显式 signed loopback system witness 针对受控 `sshd` 验证 host-key confirmation、encrypted-key AskPass、signed
Runtime 与 Terminal Host installation、canonical Project admission、无 automatic terminal creation、首个
terminal I/O、connection loss、recovery 与 durable placement retention。

从 repository root 运行聚焦边界：

```sh
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/desktop test:release-package
pnpm --filter @orchbee/desktop test:system:ssh-project-enrollment
node scripts/tla/run-models.mjs --models desktop-ssh-environment-connection,desktop-ssh-environment-project-root-service,desktop-ssh-environment-remote-project-admission,desktop-ssh-environment-remote-project-enrollment,desktop-ssh-environment-remote-terminal-creation-surface,desktop-ssh-environment-remote-terminal-outcome,desktop-ssh-environment-remote-terminal-projection,desktop-ssh-environment-remote-terminal-provision,desktop-ssh-environment-runtime-attachment,desktop-ssh-environment-terminal-host-relay,desktop-workbench-document-placement-migration
```

## 当前限制与下一项 authority

Slice 2.3A 到 2.5 授权 durable Environment identity、显式进程内 activation、immutable Runtime attachment、
一次 read-only Project-root observation，以及只由 Main 进行 remote Project admission。Slice 3.1A 与 3.1B
新增 signed v2 cohort、atomic immutable remote installation、精确 cohort negotiation，以及通过现有 SSH
attachment 提供的一个进程内 Terminal Host `HostConnection`。Slice 3.2A 通过同一 atomic admission 新增
一个 durable inactive remote Workspace 与 empty Terminal Context。Slice 3.2B 新增一个显式且仅 Main 可用的
first-terminal provision；其 durable intent 先于 Environment-routed Host effect commit，retry 会 reconcile
unknown outcome 且不 local fallback。Runtime/Host upload 与显式 provision 的 terminal 是仅有的新 remote
durable mutation。Slice 3.2C 只为 Electron 与 authenticated Gateway client 新增该现有 terminal 的 live
interactive projection。其 fresh attachment、input、selection、search、clipboard、link 与 resize 行为仍然是
attachment-scoped；disconnect 或 page release 会移除 projection，但不会删除 retained terminal 或其 Workbench
structure。
Slice 3.2D 新增精确 Environment-scoped metadata 与 failure publication，以及 crash-safe exit
admission、有序 publication evidence、restart recovery 和 target-aware deduplication。它不会 create、stop、
restart、split、archive terminal，也不会自动 reconnect。Slice 3.2E 只新增用于精确 empty remote Workspace
中 default first singleton 的 Electron Action 与产品 control。Main 派生并重新校验 target、要求既有连接，
renderer 等待 canonical publication 且不改变 navigation 或 focus。

Slice 3.3A 通过专用第十一个 bootstrap role 交付 D147-D156 的 POSIX Electron enrollment 与 recovery
surface。一项显式 request 会创建或复用 durable Environment identity、提供精确 transient interaction lease、
达到 SSH readiness、检查一项 canonical root、commit 一项 canonical Project 与 Workspace、等待
publication，随后只 activate 发起 Client View。已 enrollment 的 Project 会公开进程内 status，以及显式
Connect、Retry、Disconnect control。Restart 与 projection 仍永不 activate SSH，enrollment 仍不创建
terminal。

当前仍没有额外 remote terminal creation、remote terminal close、restart、split、archive capture、
application-stop draining、remote directory listing 或 browser、file byte、watch 或 mutation、Git operation、
Gateway 或 CLI enrollment、Agent execution、generic remote command、Environment rename/repoint/delete、
component removal、自动旧版本 cleanup 或 operating-system service/linger installation。由 CLI/Skill 控制的
产品编排属于后续 semantic-command 与 Agent slice；SSH 自身永不成为该 authority。受控 loopback witness
证明了已准入 host-key、encrypted-key、upload、Project、terminal-I/O、loss 与 recovery path；异构 production
SSH server、其他 authentication mechanism、network-quality coverage，以及 daemon 在完整 logout 或 reboot
后继续存活，仍属于未来有界工作而不是本 slice claim。
