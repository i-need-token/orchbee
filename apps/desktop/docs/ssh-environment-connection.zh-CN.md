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
才发布。Slice 3.2E 与 D194 提供一个仅限 Electron 的产品入口，用于创建和关闭 default top-level remote
Terminal Tab。它使用专用 renderer capability，由 Main 派生并重新校验全部 remote authority，并等待
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
回滚的切片。Slice 2.3A 最初通过 `environment-registry-v1.json` 交付第一项；D192 把当前 Desktop Main
owner 扩展到 `environment-registry-v2.json`，并从未修改的版本 1 文件进行只读 fallback migration。
相互独立的 Environment 与 Runtime identity、精确 SSH target uniqueness、调用方预分配的幂等 create、
严格 owner-only atomic persistence，以及 indeterminate commit 后的显式 reconciliation 均保持不变。

创建或加载 registry record 不会启动连接，也不会执行远程 effect。Record schema、transition table、privacy
boundary、rollback behavior、TLA+ refinement 与剩余 authority 位于
[`environment-registry.md`](environment-registry.zh-CN.md)。Slice 2.3B release composition 现在已经独立
交付；Slice 2.3C fast-path/supervisor composition 也已独立交付。

## 离线 release composition

已接受决策 D37-D46 固定一份 canonical schema-version-1 release、在 Zig `0.16.0` 下构建的精确 macOS
arm64 与 glibc Linux x86_64 stripped `ReleaseSafe` artifact、外部 Ed25519 signature、独立提供的 public
key，以及每份 Desktop package 一份精确固定 manifest 的 trust descriptor。Repository 代码绝不接受
production private key，普通 CI 只使用临时 key。

Development startup 与 Dev packaging 会在 Electron 启动前准备并验证 checkout-scoped development-signed
cohort。Local packaging 继续不包含 release。Release-only command 仍会在 Electron Packager 前验证并缓冲
精确 externally signed final bundle，把 executable artifact 与 manifest 放在 `Resources` 下，并把 trust
descriptor 保留在 `app.asar` 内。固定 layout、command ceremony、trust assumption、rollback、modeling
record、verification mapping 与剩余 release limit 位于
[`ssh-runtime-release.md`](ssh-runtime-release.zh-CN.md)。

Slice 3.1A 只用 schema version 2 取代当前 release packaging path：每个已准入平台各有一个独立 version
的 Runtime 与一个独立 version 的 Terminal Host，并由一个 signed canonical cohort manifest 绑定。冻结的
v1 contract 只对已经交付的 Runtime behavior 保持有效，不能满足未来 Terminal Host capability。

## 已接受决策 D47-D56

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                     |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D47  | Desktop Main 拥有按 `EnvironmentId` 建立的进程内 connection owner。Registry 仍只表示 durable intent：load 与 create 永不连接，restart 也永不 auto-connect。                                                                                                                                                                    |
| D48  | Desktop 在 OpenSSH 前把 package 内的 trust descriptor 与 manifest 作为一个 cached release catalog 加载并完成鉴权。只有远端请求 upload 后，才会 lazy 读取并验证对应平台的 artifact bytes。                                                                                                                                      |
| D49  | Platform 既不持久化，也不通过第二个 SSH probe 解析。承载安装与 Hello 的同一个 OpenSSH child 检测一个已准入远端平台。                                                                                                                                                                                                           |
| D50  | 历史 v1 negotiation 只接受一条以 newline 结尾且最大 128-byte 的 ASCII token：`OBSI/1 darwin-arm64 present`、`OBSI/1 darwin-arm64 upload`、`OBSI/1 linux-x64 present` 或 `OBSI/1 linux-x64 upload`。Unknown、duplicate、extra、non-ASCII 或 oversized bytes 全部 fail closed。D89 已针对当前 activation 取代此 token contract。 |
| D51  | Present version 必须是非 symlink 的 regular file，并具有精确 catalog digest。不存在时请求 upload。Installation 使用 no-clobber hard link；并发且相同的 installer 会 converge，任何已安装 mismatch 或 winner mismatch 都会 block 且不会替换。                                                                                   |
| D52  | Status token 只授予 flow-control authority。精确 Hello-Accepted 仍是 instance、version、digest、platform、protocol 与 daemon epoch 的最终 trust decision；不兼容的 live daemon 会 block，而不会被替换。                                                                                                                        |
| D53  | 每个 active Environment 只存在一个 supervisor。Connect 与 disconnect 按 identity 串行化，重复 connect 幂等，active supervisor 会固定 registry intent 的副本，直至显式 disconnect。                                                                                                                                             |
| D54  | 现有 owner-only AskPass bridge 仍是 credential boundary。Slice 2.3C 不增加 renderer prompt UI；缺少 interaction handler 时会 cancel。Windows activation 不受支持，并继续区分 release、compatibility、integrity、authentication、host-verification 与 retryable failure。                                                       |
| D55  | Activation 是显式进程内调用。Application shutdown 会 dispose 所有 active supervisor。不进行 registry migration，不自动 activation，并保留远端 immutable version。                                                                                                                                                              |
| D56  | TLA+ 与可执行 test 覆盖 ordering、fast path、mismatch/no-clobber behavior、owner serialization、idempotence、disposal 与 failure mapping。必须使用本地受控进程 evidence；真实 SSH server 仍在本 slice claim 之外。                                                                                                             |

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
| D84  | 显式 Desktop release packaging 在暂存前独立验证 signature、manifest、四个 artifact 与 public key。它把精确 trust descriptor 放在 `app.asar` 内，并把已缓冲的 manifest/artifact 放在 `Resources` 下；Dev 与 Local packaging 继续不带 release。                                                                                                                                                            |
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

## 已接受决策 D157-D167

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D157 | POSIX Electron SSH Project enrollment 流程改为先连接、后选目录。初始界面只询问一个 SSH 地址，通过既有有界 OpenSSH owner 与精确 AskPass interaction 建立连接；只有在 connected Runtime 存在后，才展示只读 remote-directory chooser。这个扩展延续 D147-D156，且不新增 Windows Desktop、Gateway、Browser Preview、CLI、Skill 或 Agent enrollment。                                                                                                                                                                                                                                                                                          |
| D158 | Renderer enrollment input 缩减为一个有界 SSH destination。它接受普通 OpenSSH destination 形式 `alias` 与 `[user@]host`，并为粘贴兼容接受可选且精确的前缀 `ssh `；它不接受 option、shell syntax 或 argument vector。Main 移除可选前缀、验证单一 destination、派生有界 Environment display name，并保留 OpenSSH 原生 configuration discovery。当 Desktop application home 与 OS account home 不同且其中存在 `.ssh/config` 时，Main 会把该文件作为用户级 configuration 传入，以保持应用 filesystem context 一致。它取代 D149 对新 enrollment 的约束；此前存储的精确 target、name 与可选 configuration path 无需 migration，仍可 reconnect。 |
| D159 | 精确 `OBSE` protocol 升级为 `0.3.0`，Runtime cohort 升级为 `0.2.0`，capability 精确变为 `[project-root-v2, terminal-host-v1]`。`project-root-v2` 保留 canonical inspection 并新增有界 directory listing；不提供 dual stack、downgrade、mixed cohort、remote build 或 ambient artifact selection。旧 signed cohort 仍可供兼容的旧 Desktop build 使用。它取代 D91 对新 cohort 的约束。                                                                                                                                                                                                                                                     |
| D160 | 第一个 directory snapshot 解析 Runtime process 已验证的绝对 `HOME`。每次 listing 在有界 Project-root message 与 timeout budget 内返回 canonical absolute path、可选 canonical parent、最多 256 个按确定顺序排列且可访问的 UTF-8 directory name，以及 `truncated` 标记。File、file content、无效名称、不可访问 entry 与 mutation 均被排除；directory symlink 只有在 target 能作为 directory 打开时才可出现。可编辑 absolute-path navigation field 是 truncated 或超大 directory 的完整 fallback。                                                                                                                                         |
| D161 | Main 为发起 request 精确拥有一个瞬时 directory-selection interaction。每个发布 snapshot 携带精确 request identity 与单调递增 revision；只有精确 current response 才能 navigate 或 select。Navigation failure 保留上一份 valid snapshot，并返回有界 localized failure。Select 只接受 current snapshot 的 canonical path，之后 Main 在 Workbench commit 前执行既有且独立的 canonical inspection；listing 永不成为 durable path authority。                                                                                                                                                                                                 |
| D162 | Enrollment 顺序是 target resolution、durable Environment create 或 reuse、SSH/AskPass/Runtime readiness、initial home listing、零次或多次 directory navigation、精确 selection、canonical Project-root inspection，以及 canonical Workbench commit。Initial-listing failure 会让 enrollment 失败；directory choice 期间 cancellation 或 port loss 属于 pre-commit，并且只 retire 不被 enrolled Project 引用的 operation-owned connection。既有 cancellation、indeterminate-write recovery、publication、activation 与 no-terminal rule 保持不变。                                                                                        |
| D163 | Directory snapshot 只是发起 renderer 的瞬时数据，永不进入 registry、Workbench document、diagnostic、telemetry 或 log。新 service 不能读取 file byte、枚举 file、执行 command、打开 terminal、变更 remote host，或越过 connected Environment 既有 same-UID authority。Credential 仍是精确关联的 AskPass response，且永不进入 controller state。                                                                                                                                                                                                                                                                                           |
| D164 | Rollout 与 rollback 以 cohort 为单位。要求 `project-root-v2` 的 Desktop build 只安装并准入精确 signed `0.2.0` Runtime cohort，同时保留的旧 cohort 继续服务兼容的旧 build。该变化不执行 Environment-registry 或 Workbench-document migration，也不重写现有 SSH target。                                                                                                                                                                                                                                                                                                                                                                   |
| D165 | 扩展既有 canonical `desktop.ssh-project-enrollment-dialog` experience，而不创建 fork。Address 与 path navigation control 都使用 native form，Enter 键执行当前 primary step；两个 stage、progress、prompt、failure、empty/truncated listing、cancellation 与 selection 均提供等价 `en-US`、`zh-CN` product copy 和 keyboard-accessible semantics。                                                                                                                                                                                                                                                                                        |
| D166 | 这个 Consequential 与 Modeled extension 在 production behavior 之前更新并检查 enrollment 与 Project-root TLA+ abstraction。确定性 evidence 覆盖 input normalization、精确 protocol codec、listing bound 与 failure、worker lifecycle、Main interaction correlation、stale revision、cancellation、controller secrecy、Enter submission、localized rendering，以及 final re-inspection。受控 loopback evidence 覆盖 authentication 后的 home listing、navigation、selection、Project admission 与 terminal I/O；完整 `pnpm verify` 继续排除 manual native-window test。                                                                   |
| D167 | App-owned canonical `web.remote-directory-browser` experience 是由有界 directory snapshot 控制、与数据源无关的只读 view。SSH enrollment dialog 通过 SSH-owned adapter 组合它；未来 Gateway 或跨设备 client 只有在其所属 protocol 单独授权、关联并限制 directory listing 后，才能组合同一 experience。组件自身不授予 transport 或 filesystem authority，也不包含 file listing、file byte、mutation、generic command 或 terminal behavior。                                                                                                                                                                                                |

## 已接受决策 D168-D176

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D168 | Electron 项目管理扩展规范 `workbench.project-manager-dialog`，不新增平行的管理器。当 SSH capability 存在时，最终的“添加文件夹”行会明确提供“本地文件夹”和“SSH 文件夹”两种选择。本地获取继续使用原生选择器；SSH 获取打开既有的单 SSH 地址对话框，并在认证后组合规范远程目录浏览器。Browser Preview 与 Gateway 保留当前手动本地路径适配，且不获得 SSH 挂载权限。                                                                                                                                                                                                                                                                         |
| D169 | SSH 文件夹挂载请求只携带精确的当前 Project identity 与有界 SSH destination。Main 派生或复用 Environment，通过既有 AskPass lease 连接、枚举并选择远程目录，独立重新检查所选 canonical path，并在规范 Workbench mutation 前再次验证目标 Project。Renderer 永不提供 Environment identity、Directory identity、已解析 target fact 或 canonical-path authority。                                                                                                                                                                                                                                                                           |
| D170 | 挂载提交只向目标 Project 追加一个 unavailable、Environment-placed Folder。它不重命名 Project、不改变主 Directory、不创建或修改 Workspace、不改变 active Client View location、不创建 terminal，也不改变既有 terminal context。Project 名称 Save 仍是独立 action；Folder 挂载只通过普通规范文档发布变得可见。                                                                                                                                                                                                                                                                                                                          |
| D171 | 本 slice 中一个 Project 最多引用一个 SSH Environment identity，但可以同时包含该 Environment 下任意多个不同 canonical directory 与本地 Directory。复用同一精确 SSH target 会保留 Environment identity。若 Project 已有 Environment，另一个不同 Environment 会在 Workbench commit 前失败，从而保留既有 Project 级 Connect、Retry 与 Disconnect 语义。                                                                                                                                                                                                                                                                                   |
| D172 | 目标 Project 内精确 `(EnvironmentId, canonical path)` 相等是幂等操作，返回 existing 且不写文档。同一远程位置已由其他 Project 拥有，以及任何 identity 或 location collision 都会 fail closed。已删除的目标 Project、迟到的重复项与 Environment conflict 都在规范 mutation 内重新检查，不能把 Folder 重定向到另一个 Project。                                                                                                                                                                                                                                                                                                           |
| D173 | 既有专用 SSH renderer port 增加相关联的 Folder-attachment request、progress、directory-selection、prompt、cancellation 与有界 result record；它不获得通用 filesystem 或 Workbench mutation RPC。Enrollment、attachment、connect 与 disconnect 共用一个 active-request 边界。Port 丢失会撤销瞬时 prompt 与 directory-selection authority；可能已经发生的 commit 永不回滚。                                                                                                                                                                                                                                                             |
| D174 | 规范 Project manager 为每个 Environment-placed Folder 呈现明确且本地化的 SSH 标识，包括其 accessible identity；本地 Folder 不显示标识。完整 canonical path 继续通过既有 Folder tooltip 提供。标识只由规范 placement 派生，不暴露 Environment identifier、已解析 hostname、credential 或 transport diagnostic。                                                                                                                                                                                                                                                                                                                        |
| D175 | 只有当发起 renderer 在已发布 Workbench document 中观察到精确 Project、返回的 Directory identity、canonical path 与 Environment placement 后，成功挂载才关闭 SSH overlay；其下方 Project manager 随即显示新行。取消或已知 pre-commit failure 不改变 Folder membership。Registry identity 继续 append-only；不确定的 registry 或 Workbench write 继续采用既有 reconciliation 与 outcome-unknown policy。                                                                                                                                                                                                                                |
| D176 | 这个 Consequential 与 Modeled 扩展在 dependent production behavior 前持久化并检查 `SshEnvironmentProjectFolderAttachment`。确定性 evidence 覆盖向既有本地 Project 挂载目录、纯 remote-directory transition、protocol parsing、request serialization、target 与 Environment revalidation、duplicate 与 cross-Project rejection、prompt 与 directory correlation、cancellation、publication waiting、本地化 menu 与 marker rendering，以及 D170 中不变的 Project 字段。受控 loopback evidence 扩展既有 SSH enrollment system test，以覆盖向 existing Project 挂载第二个 Folder；完整 `pnpm verify` 继续排除 manual native-window test。 |

## 已接受决策 D177-D184

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D177 | Electron Desktop 设置新增一个专门的 **SSH 连接** 分类，用于可复用的 Environment 级连接意图。它扩展规范 Terminal Settings surface，不新建第二套设置外壳、Project manager，也不向 Browser/Gateway 授予 SSH authority。Desktop capability 缺失时，Browser Preview 与 Gateway 不显示该分类。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| D178 | 页面明确分为 **你的连接** 与 **来自 SSH 配置**。已管理行只来自 append-only Environment registry，并显示有界名称、SSH destination、Project 引用数和当前 process-local 状态。建议行只来自自动只读发现；只有明确执行“添加”成功后，它才成为已管理连接。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| D179 | 发现从 Desktop 应用用户的 `~/.ssh/config` 开始，按有界 user-config `Include` 展开，最多返回 1,024 个具体 literal `Host` alias。Wildcard、negation、option-like token、malformed value、duplicate alias、configuration body、absolute configuration path、resolved hostname、user、port、identity file、Proxy command 与 credential 都不会跨越 renderer boundary。根配置缺失表示成功发现空列表；不可读、无效或截断会成为有界可见 issue。                                                                                                                                                                                                                                                                                                                                                                                   |
| D180 | 打开设置或刷新发现，只能读取有界本地配置图。它不启动 OpenSSH child，不执行 DNS 或网络访问，不提示，不写配置，也不改变 registry 或 Workbench 状态。发现结果在显式刷新之间缓存；registry 与 connection observation 仍会向每个已挂载 Desktop renderer 发布当前已管理行。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| D181 | 手动添加与建议 Host 添加都只提供一个 D158 SSH destination。Main 先应用既有固定 `ssh -G` security-floor resolver，再执行既有 append-only create-or-reuse。成功添加只记录可复用 intent，不连接、不提示、不选择目录、不创建 Project、不挂载 Folder、不创建 Workspace，也不创建 terminal。精确已管理 target 会从建议中消失；不同精确 target 不做启发式合并。                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| D182 | Connect、Retry 与 Disconnect 都是显式 Environment 级 action。它们复用既有 connection owner、每个 Environment 一个 supervisor 的生命周期、瞬时 interaction lease、AskPass prompt UI、retry fencing 与 disconnect retirement。Project 级与设置级 action 观察同一连接；被动发现与 Desktop restart 永不激活它。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| D183 | 专用 renderer contract 只携带相关联 request、opaque Environment identity、有界 catalog row、有界 discovery issue、connection state、Project 引用数、prompt handle/text 与有界 settlement。Main 仍是 SSH configuration 的唯一 reader 与 Environment intent 的唯一 writer。一个 renderer 同时最多一个请求；stale response、port loss 与 competing prompt lease 都 fail closed；secret 不进入 React snapshot、catalog、storage、diagnostic 或 canonical document。                                                                                                                                                                                                                                                                                                                                                           |
| D184 | 这个 Consequential 与 Modeled 扩展在 production integration 前记录 catalog 与 transition contract。已登记的双场景 `SshEnvironmentConnectionManagement` 模型检查 stale-refresh fencing、append-only Add ordering、被动操作不激活连接、显式 connection ownership、精确 prompt lease、port retirement，以及不存在 Project、Directory、Workspace 与 terminal effect。确定性 evidence 还覆盖有界递归发现、concrete-host filtering、缺失与 partial configuration、精确 suggestion suppression、resolver-before-record ordering、幂等 Add、共享 status projection、显式 connect/disconnect、prompt correlation 与 secrecy、multi-renderer publication、localization、search 与 capability absence。受控 loopback evidence 继续负责真实 OpenSSH/AskPass/runtime behavior；完整 `pnpm verify` 继续排除 manual native-window test。 |

## 已接受决策 D185-D192

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D185 | 在受支持 POSIX host 上，development startup 与 Dev packaging 自动准备 development-signed schema-v2 Runtime/Terminal Host cohort。这只在 Dev profile 上取代 D43、D84 与 D155。Local 与 Windows 继续不含 release，而 Release packaging 继续要求独立提供的 production cohort 与 public key。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| D186 | Development component version 由带版本 recipe、canonical checkout identity、精确 native source 与 build input，以及 active Zig toolchain identity 做内容寻址。Wall time、randomness、Git commit identity、generated output、credential 与 remote state 不影响版本。因此，任何 admitted input 变化都会产生不同的 immutable remote install path。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| D187 | Development Ed25519 private key 只在一次 preparation attempt 的内存中存在。Checkout cache 只可保留 public key、signed final cohort、精确 trust descriptor 与 validation marker。Renderer bootstrap 与 diagnostic 均不得暴露 signer、private key、release path 或 artifact byte。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| D188 | Development cache publication 采用 absent-target atomic。Private candidate 必须通过与 release package 相同的 signature、canonical-manifest、trust-descriptor 与四个 artifact 验证后才能 rename。并发相同 attempt 会重新验证 winner；任何无效 existing target 都 fail closed，且不得静默修复或替换。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| D189 | Electron bootstrap 只发布一个静态 `sshProjectEnrollmentAvailable` capability，它仅由 POSIX support 与已准入 verified cohort 派生。Renderer 只在该 bit 为 true 时组合 enrollment、Folder attachment 与 connection-management UI。既有固定 bootstrap port role 始终存在，因此 capability absence 不改变 role 顺序或 general transport authority。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| D190 | 受控 signed-loopback witness 会 package 真实 Dev distribution，从 packaged manifest 派生预期 component version，并覆盖 AskPass、immutable install、Project admission、terminal I/O、connection loss、retry 与 disconnect。它取代 D146-D184 中较早的 real-SSH verification exclusion，在 required repository gate 中于 workspace test 后串行运行，并继续与被禁止的 manual native-window test 相互独立。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| D191 | 已接受的 operational cost 是每个新 content identity 首次 preparation 较慢，以及 remote host 会保留 immutable development component version；本 slice 不增加 remote cleanup。Capability absence 就是 rollback 与 release-free mode。Real-host validation 只作为补充 evidence，其 credential 保持 transient interaction input。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| D192 | SSH enrollment 字段为粘贴兼容接受一个有界 connection-command string：可选的精确 `ssh` token、一个 destination、`-4`、`-6`、`-A`、`-a`、`-C`、`-F`、`-i`、`-J`、`-l`、`-p`，以及只允许 `AddressFamily`、`Compression`、`ForwardAgent`、`IdentityFile`、`Port`、`ProxyJump` 与 `User` 形式的 `-o`。Main 不经过 shell 进行 tokenization，拒绝未引用的 shell-control syntax、port forwarding、PTY、remote-command、local-command 与未知 option，然后在 registry 版本 2 中持久化规范化 structured option。等价 singleton-option 输入顺序复用一个精确 target；identity-file 顺序仍有意义。Resolution 与 transport 只通过 argument array 重放该结构。除非用户显式提供 `-A` 或 `ForwardAgent=yes`，agent forwarding 仍关闭；其他固定 transport 与 host-key floor 均保持。只有版本 2 缺失时才迁移版本 1，且永不重写版本 1。本决策对新 enrollment 与 Add input 取代 D158 与 D181，只在当前 registry filename 与 target shape 上取代 D151。 |

## 已接受决策 D193-D195

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D193 | 精确 `OBSE` protocol version `0.4.0` 在 Hello acceptance 中增加远端账户声明并验证过的绝对 login-Shell path。Desktop 使用该 executable 的登录模式（`-l`）、canonical remote CWD 与既有有界 terminal environment 启动每个新 remote Terminal；它不复制 local profile、argument vector、environment delta、shell integration 或 secret。仅当账户未公开有效 absolute `SHELL` 时，Runtime 才以 `/bin/sh` 作为 fail-closed fallback。本决策只在 protocol version 与 remote executable selection 上取代 D112 与 D159。                                                                                                                                                                                                                                                                            |
| D194 | 已连接的 remote Workspace 可以创建和关闭多个 default top-level Terminal Tab。每个不同的 correlated creation request 获得一个确定性 Presentation identity；精确 replay 保持幂等，而后续不同的用户操作会创建另一个 Tab。Workbench 继续拒绝 Group、split、duplicate、explicit-directory 与 insertion-index placement。Renderer proposal 可以更新 title、custom title 与 pinned state 等非权威 Tab 展示 metadata，并且只能引入精确的整 Tab close intent；Presentation identity、launch CWD、Workspace placement 与 panel topology 保持不可变。Main 仍拥有 effect 与 canonical completion。已发布的 creation 会成为选中且可见的 Tab，并获得 Terminal focus。本决策只对 Electron top-level Tab creation 与 close 取代 D137-D145，以及 D117、D127、D145 中 additional-terminal/close exclusion。 |
| D195 | Bootstrap 将每个签名发布 cohort 的 Runtime lock 与 socket 隔离在私有 `~/.orchbee/runtime-state/<cohort-namespace>` 目录下。该 namespace 只是有界路由前缀；Hello 仍会认证完整签名 cohort digest、精确 Runtime identity、version 与 build digest。兼容 cohort 会复用其存活 daemon，而新接纳的 cohort 无法附着到或替换较旧的不兼容 daemon。旧状态与进程继续按既有不清理策略保留。本决策细化 D52、D90、D95、D164 与 D191，但不改变持久 Environment 或 Runtime identity。                                                                                                                                                                                                                                                                                                                      |

### SSH 连接管理模型

| 建模字段                 | 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 问题                     | Desktop 用户无法在一个地方看到可复用 SSH Environment，无法判断连接是否活跃或被 Project 引用，也无法在不了解 Orchbee Project-enrollment 流程时采用既有 OpenSSH Host alias。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 输入                     | Untrusted input 包括一个有界 renderer request identity、一个 D192 SSH connection-command draft、一个 opaque registered Environment identity、瞬时相关联 AskPass response，以及以 `~/.ssh/config` 为根的有界本地 user-configuration graph。根配置缺失表示成功发现空列表；registry 或配置读取不可用必须成为显式 issue，不能退化成空 authority。                                                                                                                                                                                                                                                                                                                                                                                              |
| 数据与 ownership         | Environment registry 拥有 immutable managed target intent。OpenSSH configuration 仍是外部用户自有输入，永不复制进 Orchbee。Connection owner 拥有 supervisor 与 observation。Canonical Workbench document 拥有 Project reference。Desktop Main 派生 catalog；每个 renderer controller 只拥有可撤销 projection 与瞬时 request state。                                                                                                                                                                                                                                                                                                                                                                                                        |
| 输出                     | Catalog publication 包含带有界 canonical command projection 的 managed row、concrete Host suggestion 与最多一个有界 discovery issue。Add 返回 added、existing、known failure、cancellation 或 durability-unknown。Connect 与 Disconnect 返回 connected、disconnected、known failure 或 cancellation。任何输出都不包含 secret、raw registry document 或 raw configuration。                                                                                                                                                                                                                                                                                                                                                                 |
| 状态与 transition        | Discovery 从 idle → reading → ready 或 issue，且无 remote effect。Add 从 validating → resolving → recording → added/existing 或 failure，且不激活连接。Connect 从 claimed → optional prompt → resolving/connecting → connected、blocked、cancelled 或 failed。Disconnect 从 requested → retiring → disconnected 或 failed。Registry create 与 connection observation 各自独立触发 catalog publication。                                                                                                                                                                                                                                                                                                                                    |
| 不变量                   | 每个 managed row 对应一个当前 registry record；每个 suggestion 都具体、有界，且按 exact target 不在 managed rows 中；discovery 没有 process、network、prompt 或 write effect；Add 在 durable create 前 resolve，且永不连接；每个 Environment 最多一个 supervisor 与一个 prompt lease；只有显式 action 连接或断开；restart 永不 auto-connect。                                                                                                                                                                                                                                                                                                                                                                                              |
| 决策                     | Suggestion 保留 literal Host alias 作为 reusable destination input。Main 在精确 target 比较前解析并规范化 D192 option；语义等价的 singleton-option 顺序复用一个 target，不同 option value 仍是不同 immutable intent。Project-reference count 只供参考，永不成为 lifecycle authority。Append-only registry 保留 D151 的 lifecycle boundary：本 slice 不新增 remove、rename、repoint、retirement、configuration editor、wildcard materialization 或 semantic alias merging。                                                                                                                                                                                                                                                                 |
| 依赖                     | 有界注入式 OpenSSH-config discovery capability 拥有 filesystem 与 glob access。既有 resolver 拥有 `ssh -G`；Environment registry 拥有 create/list；connection owner 拥有 activation 与 observation；Workbench document 只读提供 reference count；Desktop renderer protocol 拥有 validation；规范 Settings 与 SSH prompt component 拥有 presentation。                                                                                                                                                                                                                                                                                                                                                                                      |
| Cancellation 与 recovery | Refresh 可以被更新的 refresh supersede。Add 在 registry dispatch 前取消不会产生 durable effect；indeterminate registry commit 保留既有 reconciliation 要求。Connect cancellation 只撤销 initiating lease，并遵循既有 supervisor retirement rule。Port loss 会取消瞬时 authority，但绝不改写配置、删除 Environment 或回滚可能 durable 的 create。                                                                                                                                                                                                                                                                                                                                                                                           |
| Security/privacy         | Discovery 只读取寻找 alias 所需的有界配置文件，不发布 file content 或 path。Canonical command projection、alias string 与 prompt text 是 untrusted display data。Main 永不调用 shell 或授予任意 argument-vector authority，Add 不能绕过既有 host-key 或 transport floor。显式 option metadata 本身也可能敏感，除该显式 UI 展示的有界 command projection 外都保持 owner-only。Password 与 passphrase 只存在于精确 current AskPass response，永不保留。                                                                                                                                                                                                                                                                                      |
| 验证                     | `SshEnvironmentConnectionManagement` 将 catalog/Add ordering 与显式 connection/prompt/port lifecycle 隔离成两个场景，并用两个对称 renderer 检查二者。它证明 stale catalog completion 不能发布、discovery 与 Add 不能激活 SSH 或创建 Workbench resource、registry growth 必须来自 resolved Add、竞争操作不能窃取 ownership，且 prompt 必须拥有精确 live lease。Pure parser 与 catalog test 覆盖 traversal bound、cycle、Include order、malformed input、alias filtering、deduplication、issue mapping、exact target suppression 与 reference count。Runtime、broker、client、controller、component、localization、bootstrap 与 composition test 会把每个 renderer action 追踪到其 Main owner，并证明 capability 缺失不会产生 UI 或 effect。 |

### SSH Project enrollment 模型

| 建模字段         | 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题             | Desktop 用户目前只能通过 process-local method 使用所有底层 SSH 与 remote-Project operation，却没有负责 identity establishment、authentication、progress、Project admission、failure recovery 或 reconnect 的产品路径。                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| 输入             | Renderer 提供一个 request identity，以及 D192 中唯一的有界 SSH connection-command draft。Main 解析、派生并连接精确 structured target 后，renderer response 只能 navigate 到一个有界 absolute path，或选择精确 current canonical directory snapshot。Main 派生 name、identity、resolved fact、canonical path 与 connection generation。                                                                                                                                                                                                                                                                                                                          |
| 数据与 ownership | Environment registry 拥有 immutable identity 与精确 target intent。Canonical Workbench document 拥有已 enrollment Project 与 Workspace structure。Connection owner 拥有 process-local status 与每个 Environment 一个 supervisor。Enrollment coordinator 拥有 request serialization，以及瞬时 AskPass 与 directory-selection lease。只有发起 Client View 拥有瞬时 snapshot 和 publication 后的 expansion 与 activation。任何 snapshot 或 lifecycle projection 都不能成为第二个 durable authority。                                                                                                                                                               |
| 状态             | Request 依次经历 target resolution、registry create/reuse、pending identity、connection 或 reuse、可选精确 AskPass interaction、initial home listing、零次或多次成功或失败的 directory navigation、精确 directory selection、connected Project-root inspection、Workbench commit、settlement、canonical publication，以及 initiating-view activation。Registry 与 Workbench commit 分别允许 pre-write failure 与 post-rename uncertainty。                                                                                                                                                                                                                      |
| 不变量           | Remote effect 需要 durable Environment；Workbench enrollment 蕴含该 Environment；registry-only pending identity 永不发布为 enrolled Project；一个 Environment 最多有一个 active request 与一个精确 interaction lease；stale 或 foreign prompt 和 directory response 不能进入各自 owner；inspection 与 commit 需要精确选择 current snapshot；cancellation 永不回滚可能 committed 的 Project；restart 永不 auto-connect；enrollment 永不创建 terminal。                                                                                                                                                                                                           |
| Commit/recovery  | Environment commit 先于所有 remote effect，并在后续 failure 时保留。Workbench canonical commit 是 Project-enrollment commit point。已知 pre-commit failure 会根据 registry outcome 保持 pending 或 absent。Indeterminate registry 会阻止后续使用直到 reload；indeterminate Workbench write 会从 store reconcile，之后新的 request 才能声称 created 或 absent。Startup reload 两个 owner，并且只从已验证 canonical placement 派生 enrollment。                                                                                                                                                                                                                   |
| Cancellation     | Target resolution 与 pre-commit workflow phase 可以以确认的 cancellation 停止。在 renderer 发送 request cancellation 之前，它会先使该 request 的 pending prompt 或 directory response 失效；之后到达的本地 completion 会被丢弃，而不会成为 stale channel response。Main settlement 会释放 Client serialization，之后的 re-entry 使用新的 request identity；真正并发的 live request 仍返回 busy。没有 enrolled Project 的 operation-owned connection 会被 retire；pre-existing shared connection 会保留。Workbench commit 期间或之后，cancellation 与 port loss 只移除 request delivery/interaction lease，而 Main 会完成或稍后 reconcile authoritative commit。 |
| Security/privacy | Renderer 与 Workbench 永不接收 Host connection、release byte、Runtime identity、已解析 SSH argument vector、resolved target、environment capability、raw diagnostic 或 secret。Renderer 只能提交或选择有界 D192 command string；Main 拥有 parsing 与 execution。Prompt text 与 directory metadata 是有界 untrusted display data，并且只通过精确 current correlation 接受或回答。Listing 暴露 directory name 与 canonical path，但不暴露 file byte、file entry、mutation、command execution 或 terminal authority。                                                                                                                                              |
| 验证             | `SshEnvironmentProjectEnrollment` 检查 non-atomic registry/Workbench ordering、pending identity、connection 与 interaction-lease exclusion、stale prompt 和 directory rejection、cancel-before-prompt-completion ordering 与 re-entry、initial listing failure、保留 authority 的 navigation failure、inspection 前的精确 selection、busy request、cancellation 与 port retirement、reconciliation、reconnect/disconnect、restart、initiating-view activation，以及无 terminal effect。现有 connection、registry、admission 与 first-terminal model 仍是独立 refinement。                                                                                       |

### 既有 Project 的 SSH 文件夹挂载模型

| 建模字段         | 契约                                                                                                                                                                                                                                                                                                                                                                                     |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题             | 管理既有 Project 的用户无法通过 SSH 发现并挂载远程 Folder；迟到或含糊的结果不得修改已删除的 Project、另一个 Project，或已绑定不同 Environment 的 Project。                                                                                                                                                                                                                               |
| 输入             | Renderer 提供一个精确 Project identity、一个有界 D192 SSH connection-command draft，以及相关联的 prompt 与 directory-selection response。Main 解析 structured target、派生 Environment identity 与 canonical path，并在 commit 时读取当前规范 Project。                                                                                                                                  |
| 数据与 ownership | Environment registry 拥有不可变 SSH target identity。规范 Workbench document 拥有 Project 与 Directory membership。Desktop Main 拥有 connection、inspection、request serialization、canonical mutation 与 publication observation。Workbench 拥有 provider-neutral Environment placement，并且只通过注入的 Desktop capability 呈现 SSH 专属 action。                                     |
| 状态             | Request 可以解析并记录 Environment，连接或复用 connection，提示、浏览、选择、检查、重新验证 target，拒绝 missing target、duplicate、cross-Project location 或 Environment conflict，提交或 reconcile 一个 Folder，发布它，取消、失败或失去 renderer port。                                                                                                                               |
| 不变量           | 已提交 remote Folder 只属于精确请求且仍存活的 Project；其 Environment 已持久化且是该 Project 唯一的 Environment identity；重复完成不会增加第二个 Folder；挂载不改变 Project name、primary Directory、Workspace structure、active location 或 terminal count；stale 或 foreign interaction 不能提交；publication 永不早于 canonical commit。                                              |
| Commit/recovery  | Append-only Environment write 先于远程 effect。规范 Workbench mutation 是 Folder-attachment commit point，并原子重新检查所有 Project 与 location guard。已知 pre-commit failure 保持 Folder membership 不变。可能已提交的 write 会从 document store reconcile，并可幂等重试。                                                                                                            |
| Cancellation     | Commit 前取消或 port 丢失会撤销 request 与瞬时 interaction；operation-owned 且未被引用的 connection 可以被 retire。可能 commit 期间或之后的取消不能回滚 Folder。Project manager 只观察规范 publication。                                                                                                                                                                                 |
| 安全与隐私       | Renderer 接收有界 SSH draft/canonical candidate projection，以及有界 prompt text 与 directory metadata。Credential 保持为瞬时 AskPass response。Environment identifier、已解析 SSH argument vector、resolved target、file entry、file byte、mutation、generic command 或 terminal authority 都不会跨越 presentation boundary。                                                           |
| 验证             | `SshEnvironmentProjectFolderAttachment` 在声明的有限边界内检查 exact-target commit、每 Project 单一 Environment、幂等 duplicate settlement、cross-Project 与 stale-target rejection、serialization、interaction ownership、registry-before-effect ordering、commit 前后 cancellation 与 port retirement、reconciliation、publication ordering，以及零无关 Project 或 terminal mutation。 |

### Renderer remote-terminal creation surface model

| 建模字段           | 契约                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题               | Electron 用户在已接纳 remote Workspace 中需要普通 New Terminal 与整 Tab 关闭交互，同时不能获得 Environment、SSH、Project、任意 placement 或 general remote-command authority。                                                                                                                                                                                                                                |
| 输入               | Renderer 提供一个 request identity 以及精确 `ProjectId` 与 `WorkspaceId`。Main 读取当前 canonical document、Environment registry 与 current connection observation。Placement、Environment identity、canonical remote path 与 context shape 永远不是 renderer input。                                                                                                                                         |
| 数据与所有权       | Workbench 拥有 creation availability 与默认 top-level Tab intent；canonical shared document 拥有 Project、Workspace 与 Terminal Context authority；Desktop Main 拥有 target derivation、connection admission、commit/reconciliation 与每个 correlated request lifecycle；Client View 拥有 navigation 与 focus，且不是 creation-workflow writer。                                                              |
| 输出               | Main 以一个有界 `created`、`existing`、`cancelled`、已知 `failed` 或 outcome-unknown result 结算。Created 与 existing result 标识属于该精确 request 的 Presentation。Renderer-visible terminal state 只通过普通 client-session canonical publication 到达。                                                                                                                                                   |
| 状态               | Request 依次经过 client availability、dispatch、单个 Main admission、canonical target resolution、connected-runtime preparation、commit-or-existing reconciliation、settlement 与独立 canonical publication。Port retirement 可能发生在 admission 前、commit 前、不确定 commit 期间或 commit 后。                                                                                                             |
| 不变量             | Main 从当前 canonical Workspace 派生 Environment；remote creation 永不到达 local runtime；一个 request identity 至多拥有一个 canonical Presentation；不同 request 可以追加一个新 Tab；只有显式 SSH activation 才能增加 connectivity；renderer visibility 蕴含 canonical publication；只有已确认的 pre-commit abort 才报告 cancellation。                                                                      |
| 决策               | Default placement 意味着不请求 Directory、Group、split 或 insertion index。New Group with Tab 与 Duplicate Tab 在 remote Workspace 中仍不可用。普通 New Tab control 保持可用；renderer close proposal 只能增加精确整 Tab removal，且 Presentation identity 与 immutable target metadata 必须匹配 canonical state。                                                                                            |
| Commit/publication | Canonical mutation 仍是 authoritative commit。Host reconciliation 成功不能让 renderer state 成为 authority。Commit 与 client-session publication 独立排序；创建的 Tab 在 publication 后被选中并获得 Terminal focus。Canonical close completion 只移除所请求的 Tab。                                                                                                                                           |
| Cancellation       | Canonical commit 前被观察并确认的 abort 可以结算为 `cancelled`。一旦 commit 可能已经发生，abort、unmount、broker disposal 或 port loss 都不能 rollback，而是结算或被保守观察为 outcome unknown。精确 replay 会 reconcile 同一 request-owned Presentation；之后的用户操作使用不同 request identity。                                                                                                           |
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
                      ├─ bounded project-root-v2 inspection/listing adapter
                      └─ one terminal-host-v1 opaque relay worker
                           └─ same-UID private endpoint
                                └─ retained Terminal Host daemon and Host epoch
```

Desktop connection owner 对每个 durable Environment 只准入一次显式 activation，并为这个 active identity
拥有一个 supervisor。Generation 拥有 target resolution、release-catalog admission、AskPass、OpenSSH、
installed-cohort negotiation、handshake 与 heartbeat。它不拥有任一 daemon 生命周期。Runtime 精确按顺序
公布 `project-root-v2` 与 `terminal-host-v1`；前者由 Runtime 解释，后者在不解析 Terminal Host record 的
前提下 opaque relay。

## Target resolution 与 OpenSSH transport

`SshEnvironmentTargetReference` 包含一个具体 destination 或 alias、至多一个由用户显式选择的绝对
configuration path，以及 D192 准入的规范化 structured option list。Renderer 只发送有界 command draft。
Main 不经过 shell evaluation 执行有界 tokenizer，拒绝未引用的 control operator 与任何第二个 positional token，
并把经过评审的 option 转为该结构。Resolution 调用：

```text
/usr/bin/ssh -G [-F <absolute-config-file>] [reviewed structured options] -- <destination>
```

有界结果仅准入一个有效 hostname、user、port、host-key lookup name 与
`StrictHostKeyChecking` value。接受 `yes`、`ask` 与 `accept-new`；关闭检查会 fail closed。OpenSSH 仍是
SSH configuration 与 `known_hosts` 的唯一 authority。

Transport adapter 随后重放相同结构，并用 argument array 启动同一个绝对 executable。它固定已解析的
host identity，并要求 `-T`、`RequestTTY=no`、`ClearAllForwardings=yes`、`ForwardX11=no`、
`ForkAfterAuthentication=no`、`StdinNull=no`、`Tunnel=no`、`GSSAPIDelegateCredentials=no`、
`PermitLocalCommand=no`、`ControlMaster=no`、`ControlPath=none`、`ControlPersist=no`、15 秒
server-alive probe 以及三次 missed probe。它不传入 `-D`、`-L` 或 `-R`。除非 structured target 包含
用户显式提供的 `-A` 或 `ForwardAgent=yes`，`ForwardAgent=no` 仍是 floor；该 opt-in 会以
`ForwardAgent=yes` 重放。所选 SSH configuration 仍可拥有 identity file、agent、
ProxyJump/ProxyCommand connectivity、authentication method 与 `known_hosts` interaction，而显式 D192
identity-file、ProxyJump、user、port、address-family 与 compression 选择通过普通 OpenSSH argument
semantics 覆盖。

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

Runtime state 位于 `~/.orchbee/runtime-state` 下的私有发布-cohort namespace 中，而 Host state 仍位于
`~/.orchbee/terminal-host/state`。Runtime namespace 只是一层紧凑 routing boundary；Hello 会在 ready 前
验证完整签名 cohort digest。Bootstrap 绝不调用 `curl` 或 `wget`、overwrite installed artifact、选择
remote `PATH` binary 或创建 mutable `current` link。并发相同 installer 通过重新验证 winner 来 converge；
任何 mismatch 都在 Runtime 启动前阻止整个 cohort。Bootstrap 随后用 cohort 与 Host metadata 启动精确
Runtime，等待私有 socket，再在同一个 OpenSSH stdio stream 上 `exec` `attach`。

Host endpoint identifier 是 Runtime instance 与 release cohort 的 domain-separated digest，不是 credential
或 Runtime identity。Runtime 只连接其 same-UID private endpoint。Endpoint 不存在时，隔离 relay worker 只用
固定 `serve --runtime-directory ... --endpoint-id ...` ABI 启动 signed Host 精确路径；它永不 kill 或 replace
现有 Host。

Dev 与 Release Desktop profile 提供已验证 cohort 与精确 manifest-pinned trust descriptor。Dev 使用
checkout-scoped transient-signing preparation；Release 保留维护在
[`ssh-runtime-release.md`](ssh-runtime-release.zh-CN.md) 中的 private-key exclusion、外部 signing ceremony、
canonical layout、public-key injection、rollback 与 package placement。`DesktopApplication` 把任一已准入
source 与按 Environment 建立的 connection owner 组合起来。Release-free Local profile 不公布 SSH
Project-enrollment capability，任何直接 activation attempt 都以 `runtime-release-unavailable` 失败。

## AskPass 安全边界

每个 generation 中，`SshAskPassBridge` 创建新的 `0700` 临时目录、`0600` Unix socket、`0700` 固定
launcher 与随机 256-bit capability。它设置 `SSH_ASKPASS_REQUIRE=force`；helper 通过私有 socket 发送一个
有界 JSON request 并接收一个有界 answer。每次至多存在一个 active prompt，每个 generation 至多准入八次
prompt。

Prompt text 是不受信的 OpenSSH input。只有 opaque handle、prompt kind 与 transient prompt 会交给注入的
Main interaction owner。Response 被直接写给等待的 AskPass process，随后释放。Password、passphrase、OTP、
prompt 与 answer 绝不进入 `SshEnvironmentConnectionState`、diagnostics、telemetry 或 durable storage。
Cancellation 会 retire generation。部分平台 OpenSSH build 在 host verification 时不会提供
`SSH_ASKPASS_PROMPT`；此时 helper 只识别 OpenSSH 的 canonical host-confirmation suffix，其他缺失 kind
一律保持为 `unknown`。已交付的安全 bridge 仅支持 POSIX；在 Windows Desktop 启用该路径前，仍需 Windows
ACL-backed implementation。在组合 Main interaction handler 前，每个 prompt 都会被 cancel，且不会暴露给
renderer。

## 协议、identity 与 heartbeat

每个 protocol record 都有 16-byte `OBSE` header、精确 `0.4.0` version bytes、kind、big-endian payload
length 与四个 reserved zero bytes。完整 record 上限为 1 MiB。Control payload 是严格、有界的 UTF-8 JSON；
channel data 是 binary；malformed、unknown、partial 或 oversized input 都会 fail closed。

在固定 cohort commit 后，Desktop 发送一个 Hello；Hello 包含预期 Runtime instance identity、随机 client
nonce、frame bound 与 initial credit。只有 Hello-Accepted 返回精确 instance、Runtime version 与 digest、
release cohort digest、Terminal Host version 与 digest、selected platform、精确有序
`[project-root-v2, terminal-host-v1]` capability 及有效 Runtime epoch 后，才可能进入 `connected`。Bootstrap
status 本身永远不能建立 readiness。State observation 仅保留该 epoch 与不含 secret 的连接 metadata。

Desktop 立即启动 heartbeat，并采用 Runtime 公布的 15 秒 interval 与三次 miss limit。Runtime 对精确 epoch
与 sequence 进行 acknowledgement。Mismatch 属于 protocol failure；missed acknowledgement 会 retire
transport 并进入现有有界 reconnect policy。Runtime 也会限制 incomplete record 和始终未完成 Hello 的
attachment。

## Project-root inspection 与 directory-listing service

`DesktopApplication.inspectSshEnvironmentProjectRoot` 委托给按 Environment 建立的 connection owner。它既不
调用 `connect`，也不等待 retry：inactive、reconnecting、stopped 或 disposed Environment 会返回
`unavailable`。有效调用固定到当前 Runtime epoch，分配新的 channel identity，并遵循
`open → opened → request → result → close → closed`。每个 Environment 可以同时执行四个调用；第五个返回
`busy`。

Protocol codec 只准入符合 D160 边界的精确 inspection 或 directory-listing request。Null listing path 解析
Runtime process 已验证的绝对 `HOME`；其他 request 各携带一个 absolute POSIX path。Runtime work 在 immutable
Runtime binary 的可终止 child 中执行，path 只通过 stdin 传递。Attachment loop 独立轮询 worker output 与
heartbeat。Worker 会跟随 symlink，将 canonical target 作为 directory 打开，并返回其 canonical UTF-8 path，
或最多 256 个按顺序排列且可访问的 UTF-8 child-directory name、可选 canonical parent 与 truncation metadata。
过程中不涉及 file entry、file byte、shell、expansion、retained handle 或 mutation。

`AbortSignal` 会在本地以 `cancelled` settle，并发送 channel cancellation。即使仍在等待 close
acknowledgement，caller deadline 也会在 30 秒内以 `timed-out` settle。Attachment loss 会把 outstanding
call 以 `unavailable` settle；supervisor 可以 reconnect，但绝不 replay 这些 channel。Cancellation 结算时
已经在途的单个有效 result 会在匹配的 cancelled close 前被消费并丢弃；它既不改变 caller settlement，也不
使 attachment 失效。Malformed service result 只会以 `protocol-failure` 关闭对应的有效 channel；epoch、
unknown-channel、order、framing 或 credit violation 会 retire attachment。Path 永远不会复制到 connection
observation、registry record、diagnostic、telemetry 或 log。

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
durable registry record，再要求 Environment 已有 connected observation，调用精确 `project-root-v2`
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

`DesktopApplication.createSshEnvironmentRemoteTerminal` 接收一个 `EnvironmentId`、Project ID、Workspace
ID 与 correlated request identity。它要求 Environment 已注册、有 already-connected observation，并且 Workspace 的 primary Directory 位于
该精确 Environment。它绝不调用 `connect`、等待 reconnect、接受 path hint，或更改 active/expanded Client
View state。

进入 canonical mutation queue 前，Main 为每个 Environment 至多准备一个缓存的 Terminal runtime 与一个
relay。Binding ledger 与 archive directory 使用确定性的 Environment-isolated namespace。Remote launch 使用
该 Runtime 报告并验证过的 absolute login Shell 的登录模式（`-l`），仅使用有界 terminal default 以及 `COLORTERM`、`TERM`、
`TERM_PROGRAM`，并把 canonical remote Directory path 作为 `launchCwd`；不会传输 local executable、CWD
fallback、environment delta、shell integration、CLI endpoint、profile 或 secret。

每个不同 request 都会追加精确一个 Main-owned top-level Tab、Panel 与 provision Presentation。从有界 request
identity 派生的确定性 Presentation identity 是 idempotency key：精确 replay 会复用该 Presentation，另一个
request 则创建另一个 Tab。既有 remote state 必须保持为 canonical remote CWD 下无 Group、单 Panel 的 Tab
collection；任何其他 shape 都 fail closed。Canonical version-2 commit 先于 effect reconciliation。Target
selection 只能从 Presentation containment 依次通过 Terminal Context、Workspace、primary Directory 与
placement 推导，因此 local placement 只能到达 local Host，Environment placement 只能到达该 Environment 的
Host。

Unavailable runtime 或 retryable/unknown Host outcome 会让 durable intent 保持 pending，不发布 synthetic
failure，也不缓存 settlement。精确 replay 会打开或复用 relay，并 reconcile 同一个 Environment-scoped
binding 与 Creation identity；它绝不 fallback 到本地。Client proposal 只能应用精确的 `provision → retain`
convergence、精确的整 Tab close intent，或删除受保护 remote state。Disconnect 与 shutdown 会 dispose
进程内 runtime/relay，而不删除产品状态或发送 terminal stop。Restart、split、archive capture 与
application-stop draining 仍不在本 slice 范围内。

## Electron 远程 terminal creation surface

普通 `workbench.terminal.create` Action、shortcut 与 New Terminal control 通过一个由 Workbench 拥有的
availability policy 路由。Local placement 保持既有 local command。Remote Workspace 中的 Electron renderer
只要具有 capability，就可以请求另一个 default top-level terminal。New Group with Tab、Duplicate Tab、显式
Directory、Group 或 insertion placement 仍不可用。

Renderer 只通过第十个专用 `remote-terminal-creation` bootstrap port 发送 correlated request identity、
`ProjectId` 与 `WorkspaceId`。Desktop Main 从当前 shared document 派生 Environment、canonical Directory
path 与 Terminal Context，要求 Environment 已登记且当前已连接，并在 canonical commit 前后重新校验
target。Broker 可以 prepare 或复用该 Environment 的 Terminal Host runtime，但绝不启动、重连、等待或提示
SSH。该 capability 不是 preload global，也不提供给 Gateway、Browser、semantic CLI、Skill 或 Agent caller。

Renderer 不执行 optimistic shared-document mutation。`created` 或 `existing` settlement 只有在独立排序的
client-session stream 发布精确 canonical Project、Workspace、Context、Tab、Panel 与 Presentation identity
后才完成。发起 renderer 随后会把该已发布 Tab 纳入其 window scope、选中它并请求 Terminal focus，但不改变
Project navigation。只有 Main 确认 commit 前发生的 abort 才是 `cancelled`；dispatch 或可能 commit 后的
abort、unmount、broker retirement 或 control-port loss 都是有界 failed/outcome-unknown result。精确 request
replay 会 reconcile 其 Presentation；新 request 代表新的用户操作，并创建另一个 Tab，且不 fallback 到 local
runtime。

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

十二个已登记的 TLA+ model 分担职责：

- `SshEnvironmentConnection` 检查 generation exclusion、interaction、retry admission、blocking、stop
  fencing 与 retry progress。
- `SshEnvironmentConnectionManagement` 检查 stale catalog fencing、append-only Add ordering、被动操作不激活连接、
  显式 connection ownership、精确 prompt lease、renderer retirement 与 replacement，以及不存在 Project、
  Directory、Workspace 与 terminal effect。
- `SshEnvironmentRuntimeAttachment` 检查 trusted-catalog-before-transport ordering、present 时不读取
  artifact、ordered dual-component upload、atomic cohort commit、精确 installed digest、并发 no-clobber
  convergence、mismatch rejection、单个 current attachment、完整 cohort Hello/epoch readiness、保留 daemon
  identity、competing-attachment rejection、精确 service authority 以及不 replay unknown effect。
- `SshEnvironmentTerminalHostRelay` 检查单 relay、全局 channel identity、有界双向 credit、隔离 Host setup/I/O、
  channel-local failure、attachment loss、retained Host epoch、reconnect reconciliation 与 no replay。
- `SshEnvironmentProjectRootService` 检查精确 capability admission、channel order 与 credit、有界 capacity、
  timeout、cancellation/result crossover、attachment loss、no replay、stale-result fencing 与 worker progress。
- `SshEnvironmentRemoteProjectAdmission` 检查 registry/connectivity/inspection/mutation ordering、
  cancellation boundary、全局 canonical-key idempotency、commit order 与 indeterminate-write reconcile。
- `SshEnvironmentProjectEnrollment` 检查 registry-before-effect 与独立 durable Workbench ordering、pending
  identity、按 Environment 的 request 与 prompt-lease exclusion、stale interaction rejection、commit point
  前后的 cancellation 与 port retirement、显式 reconnect/disconnect、无 prompt lease 的 automatic retry、
  restart、initiating-view activation，以及无 terminal effect。
- `SshEnvironmentRemoteTerminalProvision` 检查首个 remote binding 的 target authority、
  commit-before-effect、cancellation、deferred readiness、unknown-outcome reconciliation，以及无 local
  fallback 或 blind duplicate Host create。
- `SshEnvironmentRemoteTerminalProjection` 检查 canonical Presentation target authority、prepared runtime
  admission、local-versus-Environment isolation、start/open/delete race、stale-lease suppression、disconnect
  retirement、无 SSH activation，以及无 local fallback。
- `SshEnvironmentRemoteTerminalOutcome` 检查 current-record subscription、按 Environment 隔离 identity、
  monotonic metadata、non-synthetic failure、同步 exit admission 或 rejection、
  binding/reservation/ledger/publication/compaction ordering、retargeting、retry、owner restart，以及不
  activate SSH 的 recovery。
- `SshEnvironmentRemoteTerminalCreationSurface` 检查 renderer admission、Main-derived target authority、
  per-request convergence、distinct-request growth、cancellation boundary、port retirement、独立排序的
  canonical publication、focus preservation、无 local fallback，以及无 automatic SSH activation。
- 确定性的 remote-placement validator 与 intent-reconciler test 检查非权威 Tab 展示 metadata、精确整
  Tab close admission，并拒绝 forged path、identity、partial removal、Group 与 split。
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
Required signed loopback system witness 会 package 真实 Dev distribution，从 packaged manifest 派生预期
component version，并针对受控 `sshd` 验证 host-key confirmation、encrypted-key AskPass、signed Runtime 与
Terminal Host installation、canonical Project admission、无 automatic terminal creation、首个 terminal I/O、
connection loss、recovery 与 durable placement retention。

从 repository root 运行聚焦边界：

```sh
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/desktop test:release-package
pnpm --filter @orchbee/desktop test:system:ssh-project-enrollment
node scripts/tla/run-models.mjs --models desktop-ssh-environment-connection,desktop-ssh-environment-connection-management,desktop-ssh-environment-project-root-service,desktop-ssh-environment-remote-project-admission,desktop-ssh-environment-remote-project-enrollment,desktop-ssh-environment-remote-terminal-creation-surface,desktop-ssh-environment-remote-terminal-outcome,desktop-ssh-environment-remote-terminal-projection,desktop-ssh-environment-remote-terminal-provision,desktop-ssh-environment-runtime-attachment,desktop-ssh-environment-terminal-host-relay,desktop-workbench-document-placement-migration
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
中的 Electron Action 与产品 control；D194 将该 Action 扩展为多个 default top-level Tab 与精确整 Tab close。
Main 派生并重新校验每个 target、要求既有连接，renderer 等待 canonical publication 后再选中并聚焦新 Tab。

Slice 3.3A 与 D157-D167 通过专用第十一个 bootstrap role 交付 POSIX Electron enrollment 与 recovery
surface。一项显式 request 会创建或复用 durable Environment identity、提供精确 transient interaction lease、
达到 SSH readiness、检查 remote home、让发起 client 导航有界 directory snapshot、独立检查精确选中的
canonical root、commit 一项 canonical Project 与 Workspace、等待
publication，随后只 activate 发起 Client View。已 enrollment 的 Project 会公开进程内 status，以及显式
Connect、Retry、Disconnect control。Restart 与 projection 仍永不 activate SSH，enrollment 仍不创建
terminal。

当前仍没有 remote restart、split、Group、duplicate-Tab placement、archive capture、application-stop draining、
通用或 retained remote filesystem browser、file listing 或 byte、watch 或 mutation、Git operation、
Gateway 或 CLI enrollment、Agent execution、generic remote command、Environment rename/repoint/delete、
component removal、自动旧版本 cleanup 或 operating-system service/linger installation。由 CLI/Skill 控制的
产品编排属于后续 semantic-command 与 Agent slice；SSH 自身永不成为该 authority。受控 loopback witness
证明了已准入 host-key、encrypted-key、upload、Project、terminal-I/O、loss 与 recovery path；异构 production
SSH server、其他 authentication mechanism、network-quality coverage，以及 daemon 在完整 logout 或 reboot
后继续存活，仍属于未来有界工作而不是本 slice claim。
