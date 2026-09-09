# Orchbee Server、连接客户端与 Worker

| 字段     | 内容                                                                                                                                                                                                                                                                                                                                                                                              |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | 方向已接受；Linux 本地原生 Tab 开发链路已接通且尚未打包发布：裸命令、精确 cohort、按需启动、终端保留、共享 Tab 创建、独立选择和显式刷新已有实现证据。当前 Browser/Desktop/TypeScript CLI 继续作为已交付权威；远程暂缓，完整 Workbench、安装更新恢复、跨平台验收和迁移仍待后续交付。                                                                                                               |
| Owner    | 产品架构、Server 协调、Workbench、网络入口、Worker placement、远程 connection adapter、CLI、Agent 与未来 capability-node 维护者                                                                                                                                                                                                                                                                   |
| 当前权威 | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) 以及 Desktop、Gateway、Web、Terminal Host 的所属指南                                                                                                                                                                                                                                                                                         |
| 决策权威 | 产品决策人已经接受统一同级 Client 模型；彼此独立的公共 Rust `orchbee` 与私有 Server/Worker/Terminal Host 加可选 Rust Agent Bridge role；第一条经 Server relay 的 terminal path；有界本机懒激活；native topology 中已接受的本机 lifecycle 与 capability refinement；以及下文目标处置。确切 remote enrollment、production identity、TLS/Internet、multi-user 与 mobile-background contract 继续保留 |
| 退出条件 | 接受并实现边界清晰的 owner-specific 切片，把维护事实迁入所属指南和测试，然后删除本提案                                                                                                                                                                                                                                                                                                            |

英文 [connected-client-capability-service.md](connected-client-capability-service.md) 是 canonical
版本；本文用于简体中文评审。

## 产品方向

Orchbee Site 是由一个活跃 Server 权威拥有的产品状态与协调边界，可以服务多台机器和多个 Client
实例。仅安装 Client 的机器连接已有 Site，不会因此创建另一个 Site 或成为 Worker。产品最终必须支持：

- Desktop、Browser、TUI、手机与 CLI 客户端只能观察或控制其获得授权的产品 session；
- 性能相对较弱的工作电脑只作为远程站点的产品客户端；
- 另一台已登记的电脑提供有类型的算力或设备 capability；
- 协调电脑在协调其他 node 的同时提供本地 capability。

因此，对外可访问的服务是应用后端，而不只是静态文件服务器或反向代理。Web 静态资源交付、TLS 终止与
请求转发可以是部署职责，但不能拥有 session、授权、设备登记、capability lease、调度或产品 state。

这个方向不会把当前仅用于本地开发的 `apps/gateway`，或 Terminal Host 的同用户私有 endpoint，直接
变成网络服务。在分别接受替代实现或 adapter 之前，这些现有边界继续有效。目标形态在 Server 接管
受支持 Browser session、terminal relay 与 backpressure 行为后，删除 `apps/gateway` 与
`@orchbee/gateway-protocol`；其 framing 与进程拆分不是兼容目标。

## 产品结果与验收边界

用户应当能把工作留在性能足够的机器上持续运行，并通过已授权 Client 回到同一份工作。
增加执行机器与远程访问一台机器是两个独立结果；两者都不要求所有 Client 形态同时发布。

| 用户场景                     | 可观察的验收结果                                                                                                              | 边界                                                                           |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| 用弱电脑访问主力机           | 已鉴权 Client 打开所选 Site 已有的 Workbench 与保留终端，收到真实输出，并可请求控制权；本机不需要 Worker 或源码 Checkout。    | 属于连接客户端的产品结果；生产网络身份与传输仍需各自已接受的切片。             |
| 在多个 Client 打开同一份工作 | Client 看到同一份已授权共享结构与相同终端身份，同时保留独立焦点和视口；多个观察者不会创建另一个终端或另一个输入与尺寸控制者。 | 本机终端证据先覆盖同级 TUI Session；Web/Desktop 迁移验证跨 Client 共享的结果。 |
| 关闭 Client 或丢失连接       | Host 健康时，保留的执行继续运行；后续已授权连接回到同一个终端；不重放结果未知的输入。                                         | Client／传输连续性不承诺 Host 故障或操作系统重启后的恢复。                     |
| 不打开 Electron 窗口也能运行 | 本机 Site 与保留终端的生命周期独立于任意 Desktop 或 TUI Client。                                                              | 属于已接受的本机原生方向；仅进程握手成功不足以证明这一点。                     |
| 让另一台机器贡献能力         | 单独登记的 Worker 执行一种明确准入的工作，准入、结果、失败与撤销行为均可观察。                                                | 属于后续分布式能力纵向链路；仅完成登记不足以证明有效调度。                     |

远程查看与控制需要所选 Site 的 Server 及执行 Worker，不需要远程 Worker 登记、自动放置、
源码同步或通用 Agent 调度器。共享 Project 身份也不意味着文件已在另一 Worker 上可用，
或活跃终端可以在机器间迁移。

## 里程碑与发布边界

最初接受的工程里程碑是
[本机终端纵向链路](native-runtime-topology.zh-CN.md#已接受的本机终端纵向里程碑)：裸 `orchbee` 通过 Client、Server、Worker、
Host 到达真实的保留 shell；第二次调用加入同一 Site；输出、输入、尺寸调整、断开与重新连接
通过这条路径工作。进程、故障与性能证据必须覆盖组合后的运行时。模型、编解码器、握手和
尚待接线的 UI 模块是支撑证据，不等于这个用户结果已经完成。

已接通的本地开发链路还支持原生 Client 之间的共享 Tab 创建、独立选择和显式刷新。
当前交互及进程证据边界由[原生 Tab 指南](../../apps/orchbee/README.zh-CN.md#原生-tab-交互)维护。
Canonical Move 也已接通并具有 Linux cohort 进程证据。该指南还记录原生 Close 到 Empty、手动
Create 及保留 shell 重入的有界 Linux 进程证据；其余 Workbench 操作仍须通过各自的交付切片完成。

[Workspace 目录切片](../../apps/orchbee-server/docs/workspace-terminal-directory.zh-CN.md)
增加 Project 拥有的 Directory、经 Worker 校验后的显式首次关联，以及使用已接受目录和确切 Worker
的 Workspace Tab 创建。[原生上下文选择](../../apps/orchbee/docs/native-context-selection.zh-CN.md)
将 Linux TUI 接到已有 Workspace 及其 Tab 操作。
[原生 Workspace 设置](../../apps/orchbee/docs/native-workspace-setup.zh-CN.md) 增加分步 Project 和
Workspace 创建及显式首目录配置；终端启动仍为独立显式动作。Desktop
迁移，仍属独立工作。

该里程碑是尚未发布的增量实现，不证明生产重启恢复、远程访问或现有 Desktop 迁移已经完成。
其中，对无法匹配的保留 Host inventory 返回 `RecoveryRequired` 是开发阶段边界，不是生产连续性
体验。公开切换仍须满足原生拓扑中规定的恢复、迁移、打包与性能要求。

建议后续先验证一个人通过远程 Web 或 Desktop Client 使用一个 Site 及其本地 Worker，
在增加已登记远程 Worker 或调度之前验证连接工作台的产品价值。首个远程 Client、支持的网络与
身份契约，以及它相对其他迁移的顺序，仍属于产品决策；这个建议不会接受这些契约，
也不会扩大当前本机里程碑。

## 已接受的分布式工作台拓扑

已接受的逻辑拓扑包含三个独立授权的角色：

| 角色   | Authority                                                                                                                                                         |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Client | 渲染一个已授权产品 projection、提交语义 command，并拥有客户端本地 view 与平台 effect；不拥有站点级 durable 产品 state                                             |
| Server | 拥有 canonical Workbench state、已授权语义 command 的提交、Agent 编排、Worker registry、capability placement 与 durable recovery                                  |
| Worker | 只拥有已安装的 node-local capability 与 effect；terminal 基线拥有 launch admission 与 Host brokering，而 Files、Checkout、Git 与 Agent effect 都是可选 capability |

一台机器可以共址运行 Client、Server 与 Worker 角色，但共址不会合并它们的 identity、authority、state 或
lifecycle。性能较弱的工作电脑可以只运行 Client。中心机器可以运行 Server 与一个 Worker，同时协调其他
Worker。**Terminal Host** 继续表示 node 本地的 PTY 服务，绝不表示中心 Server。

TUI、Web、Desktop、mobile、不同 Browser tab、不同 Desktop window 与重复 native invocation 都是同级
Client 实现。每个活跃实例拥有一个 Client Session 及其本地 focus、active location、viewport、scroll
与 presentation state。共享 Workspace、Tab、Panel 与 Terminal fact 来自 Server。Renderer 形态绝不
授予额外 authority。

Project 是不依赖主机的产品目标。Checkout 是位于一个确切 Worker 上的可选具体源码树；在兼容路径
保留期间，它也可以位于一个 SSH Environment adapter 之后。Workspace 可以选择 Primary Checkout
或 Directory 作为 feature-owned 初始目录建议，但 Project 或 Workspace identity 都不是 terminal
创建的前置条件，也不能独立决定 Worker placement。已接受的关系尚未定义 durable schema、migration、
跨机器源码传输策略，也未决定在内部重命名还是适配 `Directory`；这些决策需要一个边界清晰的
Workbench proposal。

已接受的本地[目录契约](../../apps/orchbee-server/docs/workspace-terminal-directory.zh-CN.md)
现为尚未发布的内存原生 cohort 选定 Project 拥有 Directory、Workspace 持有 nullable primary 引用。
这关闭了本地表示及首次关联决策；持久化 schema migration、通用 Directory CRUD 和跨机器传输仍不在其中。

[`native-runtime-topology.md`](native-runtime-topology.zh-CN.md) 中已经接受的原生 refinement 取代了
“每个 execution effect 都必须解析通用 Environment”的旧规则。Future native terminal creation
直接解析到一个 Server 授权的 Worker；Project、Checkout、Worktree 与 Environment identity 都留在
Terminal Host 边界之外。已经交付的 SSH Environment record 与 remote Workspace 行为在另行接受
migration 或产品下线前继续作为现行权威。当前 `apps/ssh-environment-runtime` 与
`@orchbee/ssh-environment-protocol` 是已接受的退役候选项，而不是目标架构资产。未来 SSH adapter
是与 Server/Worker 边界一致的新设计，可以在旧路径有意删除之后再交付；它不需要保留当前 executable、
package、wire、service、release cohort 或 Desktop-owned composition。在 cutover 前，当前
unmanaged-SSH 路径仍可以在到达 Runtime 前解析其确切旧 Environment adapter，但不能让
Environment 恢复为通用产品领域。

Workbench 产品领域是共享的，renderer 不是。Server 拥有 canonical Workbench transition，并发布
framework-neutral、版本化 semantic snapshot 与 delta。Browser 与 Desktop 使用 `apps/web` 拥有的
唯一 React 实现；Rust TUI 与未来 mobile Client 各自拥有 presentation 实现。CLI 可以返回有界的
结构化 semantic projection snapshot，但绝不是 HTML、React tree、pixel 或另一 Client 的本地 view。
当前混合的 `@orchbee/workbench` package 按这个边界拆解，并在迁移后删除。

## 已接受的语义控制方向

人类控件、Orchbee CLI、Skill 与已批准 automation 必须到达同一组 owner-defined 语义操作，不能实现平行
mutation。CLI 是 versioned invocation surface，不是产品 state Store 或 authority 边界；替换实验性
Client 时可以重新设计 grammar，而不改变 semantic owner contract。Skill 组合结构化 CLI 操作；当存在
符合条件的语义操作时，Skill 绝不解析面向人的展示文本，也不模拟 pointer、DOM 或 keyboard input。

只读 CLI operation 可以为 automation 暴露一份已授权且版本化的 Workbench semantic projection
snapshot。该 snapshot 是 Server 产品 projection，不是任何 Web、TUI、mobile 或 Desktop renderer
的序列化结果。

已经接受的 Rust 替换方向及其刻意重新设计的公共 grammar 由
[`native-command-surface.md`](../cli/native-command-surface.zh-CN.md) 拥有。

当前进程内的 Action registration 与 prepared invocation 继续作为本地 runtime capability。它们不是可序列化
的网络请求，也不会因为能够枚举就获得远程调用能力。每个允许 CLI 或远程调用的操作都需要明确且版本化的
input 与安全 output schema、caller authorization、非交互 confirmation policy、compatibility lifecycle、
idempotency boundary 以及所属 domain handler。focus、窗口 geometry、native dialog 与 clipboard access 等
客户端本地操作需要一个精确且已授权的 Client Session；它们绝不能隐式改变所有连接客户端的 view。

Durable ownership 拆分如下：

| State 或 effect                                                               | Owner                              |
| ----------------------------------------------------------------------------- | ---------------------------------- |
| Project、Workspace、共享 Tab/Panel 结构、Agent task 与 scheduling state       | Server product coordinator         |
| Worker registration、capability placement、operation authorization 与 fencing | Server product coordinator         |
| Terminal launch admission                                                     | 精确 Worker                        |
| PTY、process tree 与 terminal state                                           | 精确 Terminal Host                 |
| 可选 Checkout、file、Git 与 Agent-executor effect                             | 精确且单独准入的 capability owner  |
| Active location、selection、focus、maximization、窗口 geometry 与 clipboard   | 精确的 Client Session 或客户端平台 |

## 已接受的命名与 contract 时机

目标后端角色名为 **Server**，其私有原生可执行程序是 `orchbee-server`，由
`apps/orchbee-server` 拥有。现有 Node `apps/server` / `@orchbee/server` workspace 是已经交付的
Browser-ingress preview，不是目标原生进程；它只在受支持行为迁移前继续作为现行权威。`Server`
表示完整产品后端角色，但不会把网络入口、鉴权、协调、设备 identity、调度和执行混成一个没有
内部边界的模块。

Human Client 与 Worker 通信边界保持分离，并且只能随具体 consumer 落地：

| Package                                                    | 创建时机与职责                                                                                                                                                                   |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `packages/remote-client-protocol`                          | 已由接受的人类 Browser 鉴权切片创建。当前负责个人访问令牌交换与签名 Cookie session 建立契约；更广泛的生产 transport、协商与授权仍为保留决策。                                    |
| `schemas/client-server` 与 `crates/client-server-protocol` | 已有的实验性 OBCS Create/Start 意图与结果契约，服务于本机终端纵向链路。本机准入独立存在；它不是完整的实时终端或生产远程 Client 协议。                                            |
| `crates/capability-node-protocol`                          | 已有的实验性 OBCN 已授权终端 Launch/Start 值与编解码器，用于 Server-Worker 集成。不提供登记或通用调度器；确切现行权威由所属指南维护。                                            |
| 未来远程扩展                                               | 只增加已接受远程 Client 或具体远程 Worker 能力所需的契约。生产身份、登记、能力声明、进度、取消、数据传输与兼容性必须针对该 consumer 定义；已有本机编解码器不会隐含提供这些能力。 |

Client 与 Worker 边界必须分开，因为人类产品客户端与 Worker 具有不同的 identity、credential、
authority、撤销范围和兼容性生命周期。两个边界都不是 Server 实现，不拥有产品 state，
也不能授予通用远程 shell。当前本机契约由
[Client-Server schema](../../schemas/client-server/README.zh-CN.md)、
[Client-Server binding](../../crates/client-server-protocol/README.zh-CN.md) 与
[Capability Node 指南](../../crates/capability-node-protocol/README.zh-CN.md) 维护。

当前 `apps/gateway` 与 `@orchbee/gateway-protocol` 继续明确作为仅限预览的 Browser transport，直到
生产 external-client 切片迁移并删除它们。它们可以绑定 loopback 或用户明确选择的 trusted-LAN interface。
`apps/server` 使用个人访问令牌换取签名 session Cookie，Gateway 使用同一个 session 准入 exact-Origin
control connection；访问 URL 不含 credential。它们不得改名为 `server`，也不得被视为生产
external-client 契约。生产迁移不能长期维护两套相互竞争的外部客户端网络契约。

## 已接受的连接策略

Orchbee 支持两个互补的连接族：

| 连接族                | 职责                                                                                                                                        |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| 原生站点              | HTTPS 建立已鉴权的人类或设备 session；WSS 通过 role-specific contract 传输版本化 control、event、progress 与有界 binary projection message  |
| SSH bootstrap/adapter | 连接未受 Orchbee 管理的机器、验证其 host identity、Bootstrap 或修复 Orchbee runtime，并可以在一个明确有界的阶段通过 tunnel 承载原生协议流量 |

这个连接族描述的是产品 capability，并不承诺保留当前 SSH Environment Runtime 或协议。若现有
产品路径已显式下线，已经接受的退役可以有意先于替代 adapter。

原生站点连接是已鉴权 Client 与已登记 Worker 的长期主路径。已接受 Connect-via-SSH path 继续作为
首次使用、兼容、管理与 temporary-connection path；它不是 Workbench、Agent、调度或 state
synchronization protocol。已登记 Worker 通常使用非 Cookie device credential 主动连接 Server；
SSH-attached temporary Worker 是 connection-scoped，绝不隐式提升为 enrollment。

用户明确授权的 SSH Project 可以在该 SSH 账号范围内提供 Terminal 与 filesystem effect。这不会自动登记
该机器，不会把 capability-node contract 变成通用远程 shell，也不会向 scheduled work 授予超出已接受 typed
capability 的 authority。

Desktop 与 Browser client 通过 remote-client 边界消费同一个已授权产品模型。即使两种连接角色使用同一个
TLS listener 与 WebSocket 实现，capability node 仍然使用独立的 node 边界。HTTP 与 WebSocket framing 不定义
产品 identity、authority、retry 或 compatibility 语义。应用 contract 保持 transport-neutral，使已接受的 SSH
tunnel、relay 或后续 transport adapter 无法创建第二套产品模型。

生产网络流量使用成熟 TLS 与 SSH 实现提供的 authenticated encryption；Orchbee 不发明 transport
cryptography。Plain HTTP 继续仅限于已经交付且明确警告风险的 trusted-LAN 预览。由测量触发的
local-native Client-to-Worker path 只在下文 scoped Server-capability contract 内获接受；remote direct
path、QUIC/WebTransport adapter、relay service 与 Internet ingress 仍是保留的优化或部署决策。
它们都不能绕过 Server 签发的 authority 或 recovery fencing。

## 已接受的初始入口基础

已交付 Node 预览的 HTTP 路由、Fastify 组合、session 交换、listener 默认值与静态资源行为归
[Server 指南](../../apps/server/README.zh-CN.md) 所有。Desktop 所属的启用流程、精确 Origin
Browser 准入、终端转发与背压归 [Gateway 指南](../../apps/gateway/README.zh-CN.md) 所有。
这些是迁移输入，不是要求在原生 Server 旁重新建设第二套 Node 底座。

可信局域网 HTTP 预览仍是生产加密传输方向之外、必须明确启用并展示风险提示的例外。
它不证明生产 Internet 部署或独立于 Desktop 的 Site 生命周期。单独启动 Server entry 不会启用共享。

## 人类用户的 Web 访问方向

网络暴露默认关闭。用户启用共享后，已接受的站点入口才在明确的 interface 与端口上开始监听；可分发的入口是
类似 `https://192.0.2.1:8443/` 的短 origin，后面只跟普通产品路由。credential、bootstrap token 与
session capability 绝不能出现在被分享的 query string 或 fragment 中。

未鉴权的 Browser 请求保留其原本要访问的产品 path，同时进入登录流程。用户输入单独签发的 access token；
交换成功后，系统创建有界且可撤销的人类 session，通过范围适当的 Cookie 表示，然后恢复原目标路由。这个
人类登录流程绝不能登记 capability node，也不能授予设备 credential。

已交付预览的 token 与 Cookie 实现继续由上面链接的 Server 和 Gateway 指南拥有。生产 session
时长与撤销存储、TLS 部署、多用户授权和账号恢复仍为保留决策；不能把预览鉴权描述为
这些生产契约已经获得接受。

## 已接受的连接连续性模型

网络连接是可替换的 transport attachment，不是 Project、Workspace、Checkout、retained terminal、Agent task
或 durable 产品 session 的生命周期 owner。SSH、WebSocket 或底层网络失败会让受影响 attachment 进入明确
的 disconnected、reconnecting 与 recovering lifecycle；它不会请求删除、取消或终止进程。

重连会建立一条新的已鉴权 transport，并对账稳定的 site、Client Session、Node Session、Workbench
revision、terminal inventory、Agent execution 与 event sequence identity。当增量恢复不安全或不可用时，
authority 返回有界的缺失 event 范围或一份新 snapshot。安全的语义 command 使用 owner-defined operation
identity 与 idempotency rule。Terminal input 或其他 admission outcome 未知的 effect 绝不能自动重放。

Client 与 node 使用有界的随机重试和递增 backoff，展示 offline 与 recovery state，并在 identity、
authorization、host verification、compatibility 或 policy 需要用户或管理员处理时停止重试。鉴权便利性不能
削弱 host-key 或 TLS identity validation。Key-backed 或 agent-backed SSH 与已登记设备 credential 是首选的
无人值守恢复机制；password-only SSH 在 transport 丢失后可能需要用户重新鉴权。

Transport 丢失、远程 runtime 失败与远程机器重启是不同的失败类型。第一种保留远程执行；第二种在可用时与
独立 retained owner 对账；第三种可能需要明确重启任务，因为操作系统重启无法保留普通 PTY 或进程。

这些是目标连续性要求。首个本机原生里程碑没有持久化 Server 接管能力，当前预览也有各自更窄的
生命周期。保留执行、恢复观察、恢复控制必须分别在正在交付的边界得到证明；只保留 Host 进程
不足以证明后继 Server 能安全恢复控制。

## 已接受的原生运行时细化

[`native-runtime-topology.md`](native-runtime-topology.zh-CN.md) 是本逻辑设计的 canonical physical-process
与 local-lifecycle refinement。产品审阅还接受了以下方向：

| 区域                    | 已接受方向                                                                                                                                                                                                                                                                                                                                                                               |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Client 与 terminal use  | 每次 TUI invocation 都是共享 Workbench document 上的独立 peer Client View。Display attachment 是 transient 的，多个 Client 可以查看一个 retained terminal。一个 combined input-and-resize controller 由 Server fencing；显式 request、disconnect grace 与二次确认 forced take 定义 transfer，不做 automatic promotion 或 collaborative input。                                           |
| 可选 capability         | 私有 Rust Agent Bridge 通过 Worker 向 Server-owned Agent semantics 提供有界 local observation；managed Worktree Git effect 与 Archive content 保持 Worker-local；Server 拥有其 canonical reference/metadata 与 revisioned Launch Profile catalog；每个 Client 各自渲染 projection。Terminal-only Site 不需要这些 capability。                                                            |
| 通过 SSH 连接           | 目标中删除 generic Environment。Server 拥有 `SshTarget`/`SshConnection` intent，所选 local Worker 使用 system OpenSSH、本机 host verification/credential 与 ephemeral AskPass 启动 connection-scoped temporary remote Worker/Host。它不同于 enrolled Worker，首批从 local TUI/Desktop 创建；只有 migration evidence 完成后才替换旧 SSH Runtime/protocol。                                |
| Terminal path           | Server relay 是首条和 fallback route。只有实测需要才可以在 short Server-minted、绑定 identity/generation 的 capability 下启用 local native TUI/Desktop Client-to-Worker route。Browser/mobile 保持 relay，任何 Client 不直连 Host，Server 保留 control authority，fallback 不 replay unknown input。                                                                                     |
| Local Site lifecycle    | 默认 lazy activation。显式 `orchbee site enable` 可以安装 same-cohort per-user LaunchAgent/systemd-user Server 与 Worker unit；Host 保持 lazy，不增加 root daemon 或 supervisor。Restart 保留 retained Host terminal；stop 报告它们，不等同 terminate-all。Signed side-by-side cohort 原子更新，绝不替换 live Host。                                                                     |
| Persistence 与 recovery | Production Server 对 canonical intent、topology、ledger、profile 与 reference 使用 transactional SQLite/WAL，而不保存 Client View、live attachment、terminal frame、PTY/scrollback、transient secret 或 raw hook。Intent 先于 effect；restart 对账 exact identity 且不 replay。Unknown Host inventory 被 quarantine，corrupt/incompatible storage 进入 read-only recovery 而不是 reset。 |

这些是已接受的未来边界，并不表示已交付 Node Browser preview 或 Desktop-co-located runtime 已经具备
对应行为。Exact remote Worker enrollment、production human/device identity、TLS/Internet ingress、
multi-user tenancy 与 mobile background behavior 仍未决定。

## 候选职责拆分

这些是已接受角色内部的语义职责；每一行不代表需要额外进程、workspace、框架或独立部署的服务。
可执行程序拆分由原生拓扑拥有，进一步的模块边界由具体 consumer 及其行为决定。

| 职责                                          | 候选 owner                                                                                                                |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| HTTP/WebSocket 入口与 Web UI                  | 网络入口服务；拥有 TLS、Origin policy、路由 fallback、限流与 renderer 静态资源交付                                        |
| 人类用户鉴权                                  | 站点 identity/session 服务；拥有 bootstrap token 换取、安全 Cookie session、过期、撤销与审计                              |
| 产品协调                                      | Server 应用 coordinator；拥有已授权 Workbench session、canonical state、语义提交与恢复                                    |
| Worker placement 与可选 source-tree reference | Server product coordinator；确切 Worker identity 是原生 placement，旧 SSH Environment identity 只存在到当前路径迁移或下线 |
| Capability node 登记                          | 设备 identity 服务；拥有配对、设备 credential、attestation 输入、轮换、撤销与在线状态                                     |
| 工作调度                                      | 有类型的 capability scheduler；拥有准入、放置、lease、取消、重试、配额与结果 ownership                                    |
| 本地终端执行                                  | 位于现有受信本地 broker 边界之后的 Terminal Host                                                                          |
| 远程执行                                      | 单独鉴权的 node runtime，只暴露已接受且版本化的 capability contract                                                       |
| CLI 与 Skill 调用                             | 狭窄的语义 command client；提供结构化调用和观察，但不拥有产品 state                                                       |
| Desktop、Browser 与手机端 projection          | 消费已授权应用 API 的产品客户端；终端形态本身不会自动获得后端 authority                                                   |

逻辑 Client-Server-Worker 拓扑与原生物理 baseline 已经接受：公共 Rust `orchbee`、私有 Rust Server、
私有 Rust Worker、独立 Zig Terminal Host 与可选私有 Rust Agent Bridge。其增量底座与第一条本机
Server-Worker process spine 已经交付但尚未发布，其余Linux 本地开发链路已接通；当前与
Desktop 共置的 runtime 在有边界 migration slice 替换前保持权威。第一条完整本机 terminal path
经 Server relay，裸 `orchbee` 可以懒激活确切本机 cohort，但不监督其生命周期。共置绝不能合并
trust、state 或 lifecycle 职责。User-service installation 与 direct-path contract 已在上文接受，
但仍是 gated implementation work；remote enrollment 继续保留。

## 连接角色

每个连接都具有一个明确且已经鉴权的角色。设备类型、User-Agent、IP 地址和局域网位置都不能授予角色。

| 角色            | Authority                                                                |
| --------------- | ------------------------------------------------------------------------ |
| 人类产品客户端  | 对选定产品资源进行已授权的读取和语义 command 操作                        |
| Capability node | 声明已接受的 capability 版本、接收有范围的 lease、报告进度并发布有界结果 |
| 站点管理员      | 登记和撤销设备、批准 capability、设置策略并查看安全相关审计记录          |

一个设备可以通过不同 credential 和有范围的 session 同时承担多个角色。Browser Cookie 用于鉴权人类 session；
capability node 必须使用可轮换、可撤销的非 Cookie 设备 credential。Browser token 绝不能成为 node
credential。

## Capability 执行模型

远程算力以有类型的 capability 暴露，而不是通用远程命令执行。
每个已接受的远程 capability 契约必须定义：

- 版本化的 input、progress、result、failure、cancellation 与 compatibility schema；
- 资源需求和 scheduler 可见的容量，同时不暴露无关设备数据；
- lease identity、过期、heartbeat、重复投递行为与幂等边界；
- artifact、源码数据、secret、日志与诊断的传输策略；
- 断连、重新分配、部分结果与迟到结果的行为；
- 授权工作并提交 durable outcome 的产品 owner。

本机终端基线需要其具体启动与实时终端契约。这个清单不要求在基线完成或人类 Client 远程使用 Site
之前增加通用任务队列、调度器、artifact 服务或远程登记。新能力必须先明确实际工作、所需的
节点本地数据与凭据，以及完成证据，再准入通用基础设施。已有终端保留语义绝不意味着实时迁移，
也不意味着自动把终端输入重新分配给另一 Worker。

站点 coordinator 拥有 placement policy；capability node 拥有已接受 lease 的执行。node 与网络 adapter
都不能成为 Workbench shared state 的 writer。Product Action 继续是 owner-internal 语义契约。单独评审
的 CLI 或远程 API 只能通过可序列化边界暴露明确允许 automation 的操作；它绝不能序列化 prepared
invocation，也不能授予 registry-wide authority。

## 部署形态

| 形态                 | 必需行为                                                                                                                |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| 单台电脑             | Client、Server、本地 Worker capability 与可选 Web 入口可以共址                                                          |
| 弱性能远程工作电脑   | Desktop、Browser、手机或 CLI Client 渲染并控制一个已授权远程 Server，而不成为 Worker                                    |
| 当前 SSH Environment | 在迁移或下线前，当前产品 coordinator Bootstrap 并附着一个未受 Orchbee 管理的远程 Project Environment，而不虚构一个 Site |
| 分布式站点           | 一个 Server 通过明确 identity 与 capability 协调本地 Worker 和其他已登记 Worker                                         |
| Coordinator 暂时离线 | Client 与 node fail closed，或进入明确建模的降级模式；双方都不能凭空产生 authority                                      |

LAN 发现、Internet relay、NAT 穿透、公共 DNS 与云端 rendezvous 是传输和部署选择，不是产品授权机制。

## 不变量

- 用户明确启用并配置网络暴露之前，网络入口默认关闭。
- 在每个信任边界解析所有外部消息，并在鉴权后执行授权；知道 IP 地址和端口不会获得任何 authority。
- 人类 session、设备 identity、capability lease、terminal attachment 与 Workbench client session 使用
  不同 identity 和撤销范围。
- Server、Worker 与 Client Session state 分别只有一个已接受的 authority；transport adapter 与共址不能创建
  另一个 writer。
- Project 不依赖机器 identity。Native terminal effect 直接解析到一个确切且由 Server 授权的
  Worker，不要求 Project、Checkout、Worktree 或 Environment identity。具体 Checkout 解析到
  其确切 Worker；保留的 unmanaged-SSH 路径可以先解析其显式 Environment adapter。
- 在当前契约下，Terminal Host 保持私有本地传输，绝不直接接受 Internet 或 LAN peer。
- SSH 与原生站点连接是可替换的 transport attachment。连接丢失不表示删除 Project 或 Workspace、终止
  retained remote work，或取消 Agent task。
- 恢复过程对账稳定 identity 与 revision。它绝不能重放 admission outcome 未知的 Terminal input 或其他
  effect。
- 某个已接受 capability 只需要较窄 effect 时，远程执行不能因此获得通用 Desktop、文件系统、shell、
  secret 或 Action capability。
- CLI、Skill 与远程客户端只能调用明确准入的语义操作，不能绕过 feature-owned validation、authorization、
  confirmation、commit 或 concurrency policy。
- 没有明确建模的 fencing 机制时，断连和重试不能重复提交一个逻辑结果，也不能把一个 lease 交给两个
  活动 owner。
- Browser 与 Electron 消费 `apps/web` React renderer，TUI 与 mobile 基于同一套已授权产品语义
  各自实现 presentation；transport 选择绝不能创建第二个 canonical Workbench authority。
- 审计记录默认只包含 identity、决策与有界 metadata，绝不包含 terminal 内容、credential、
  capability token 或 secret payload。

## 保留决策

已接受的本机 terminal milestone 不会顺带决定以下后续事项：

1. 已接受 observation topology 之外的 Agent launch/orchestration、provider-native response、
   durable content/history、automatic remote integration installation 与 mobile notification delivery。
2. 超出已接受首批 migration 的 destructive managed-Worktree operation、Archive
   export/synchronization 与长期 retention、更广 Launch Profile capability，以及 remote SSH
   credential UX。
3. Production human/device session、Worker enrollment、remote capability contract、scheduling、
   lease、fencing、cancellation、quota 与 data-locality policy。
4. Production HTTPS/WSS、TLS 与 Internet ingress、relay 或 NAT traversal、mobile backgrounding、
   multi-user tenancy、authorization hierarchy、audit、secret 与 artifact transfer。
5. 已接受 local lifecycle contract 内的 exact service unit、activation/grace deadline、performance
   threshold、numeric limit、database schema/migration、backup UI 与 package-manager adapter mechanic。
6. 已接受 command architecture 内的 remote CLI targeting、exact machine schema 与 numeric exit、
   详细 TUI presentation、installed Skill catalog 与 exact first public command cohort。
7. Node Server preview、Gateway/protocol、SSH Runtime/protocol、混合 Workbench package、旧 CLI 或
   已迁移 Desktop authority 的每项 deletion。删除需要各自的 consumer、installed-artifact、behavior、
   migration 或 sunset、documentation、evidence 与 rollback 审计。

没有具体且已接受的纵向 consumer 时，不得创建空的 remote Worker protocol workspace，也不得扩展
现有 preview protocol。当前 Browser ingress 与 SSH 路径只对已交付行为保持权威，直到完整替代或
显式 sunset 获得接受并验证。

## 交付顺序

现行已交付行为属于其 application guide 与可执行测试，而不再通过本提案中的阶段流水账维护。

| Milestone                       | 可独立审阅的 outcome                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 现有现行路径                    | 在增量原生工作仍未发布期间，让 Node Browser-ingress preview、当前 TypeScript CLI、Desktop-colocated coordinator 与已交付 SSH 路径继续对其受支持行为保持权威。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 原生底座                        | **已经完成且尚未发布：**根 Rust 支持、framework-neutral Workbench projection contract、原生 `orchbee` 入口底座、Rust Server kernel 与 Rust Terminal Host binding。[`native-runtime-topology.md`](native-runtime-topology.zh-CN.md) 链接其现行 owner。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| 本机终端纵向链路                | **Linux 开发链路已接通，尚未打包发布：**裸命令校验继承 TTY、精确四产物 cohort 和私有运行时，先连接、必要时独占提交一次 Server 启动。Server 持有生命周期锁并只启动一个 Worker；Worker 仅在首个 Launch 授权通过后启动 Host。Open、viewer、controller 和首帧共用 Client 启动截止时间。真实 PTY 进程测试覆盖双 Client 冷启动、控制/只读、关闭后原 shell PID 和变量保留、失效 cohort、失败标记、锁竞争截止时间，以及 Server/Worker/Host 丢失后不启动替代进程。只支持已校验的 Linux ext4/tmpfs 开发路径与真实 cell/pixel geometry；macOS/Windows 公共启动、完整 Workbench、终端交互模式对齐、自动恢复、安装更新和发布性能预算尚未交付。握手就绪不等于终端首帧就绪；此链路不替换当前已交付入口。当前契约见 [Client](../../apps/orchbee/README.zh-CN.md) 和 [共享运行时](../../crates/local-site-runtime/README.zh-CN.md)。 |
| 共享 Client 与 Server migration | 把 canonical Workbench coordination 和受支持 Browser session/relay 行为迁入 Rust Server，把 React presentation 迁入 `apps/web`，把 Desktop 缩减为 Client/platform adapter，并且只迁移显式准入的 CLI capability。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 已接受的可选 capability         | 通过各自已接受的 owner-specific vertical migration 实现 Agent observation、managed Worktree、Archive、Launch Profile 与 Connect via SSH。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 远程连接 Client                 | 验证另一台机器上的人类 Client 使用同一 Site 及其已有执行 Worker。在独立切片中选择身份、网络暴露、Client 范围与重连证据；远程 Worker 登记和调度不是前置条件。优先级遵循上文所述产品决策。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| 分布式 Worker capability        | 审阅登记与一种具体远程工作，覆盖设备身份、数据／secret 可用性、放置、取消与失败行为。Mobile background、多用户策略与更广调度仅通过各自已接受范围增加。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| 旧实现退役                      | 只有每个替代或 sunset cohort 证明 compatibility、installed-artifact handling 与 rollback 后，才删除 Node Server preview、Gateway/protocol、SSH Runtime/protocol、混合 Workbench package、旧 CLI 与已移除 Desktop authority。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

每个 implementation slice 都必须保留已接受 role boundary，并证明自身 security、privacy、
compatibility、observability、rollback 与 recovery claim。历史交付细节留在所属现行指南；本提案
记录尚未解决的目标决策，而不是任务 chronology。
