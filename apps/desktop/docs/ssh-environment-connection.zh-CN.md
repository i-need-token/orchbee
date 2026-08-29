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

可执行 evidence 包含 strict protocol 与 stream-decoder test；canonical release-contract、signed-manifest
mutation、wrong-signer、exact-layout、symlink 与 package-staging test；shell syntax、fixed-status decoding、
installed/present/upload/mutation/concurrent-install 本地 process check 与 no-download assertion；OpenSSH
security-floor argument；owner-only AskPass IPC；Environment owner serialization 与 shutdown；supervisor
orchestration；全部 Desktop unit test；原生 Runtime unit 与 real-process attachment test，包括 canonical
path 与 failure behavior；以及两个已准入远端 target 的 cross-compilation。

从 repository root 运行聚焦边界：

```sh
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/desktop test:release-package
node scripts/tla/run-models.mjs --models desktop-ssh-environment-connection,desktop-ssh-environment-project-root-service,desktop-ssh-environment-runtime-attachment
```

## 当前限制与下一项 authority

Slice 2.3A、2.3B、2.3C 与 2.4 现在授权 durable Environment identity、offline release composition、显式
进程内 activation、immutable Runtime installation 或 attachment，以及一次 read-only Project-root candidate
observation。唯一远程 mutation 是同一 bootstrap child 请求的精确 versioned Runtime upload。当前仍没有
renderer API 或 prompt UI、automatic activation、Project/Workspace model、remote directory listing 或
browser、file bytes、watch 或 mutation、terminal channel、Git operation、Agent execution、generic remote
command、Runtime removal、自动旧版本 cleanup 或 operating-system service installation。Windows activation
仍不受支持，也不承诺 daemon 在完整 logout 或 reboot 后继续存活。

Directory-listing、file 或 Terminal Host service 仍需要后续显式 authorization、channel contract、reconnect
inventory 与 no-replay 语义。由 CLI/Skill 控制的产品编排属于后续 semantic-command 与 Agent slice；SSH
自身永不成为该 authority。Real-SSH interoperability、authentication UI 与 network-failure evidence 也仍
属于未来有界工作，而不是本 slice 的 claim。
