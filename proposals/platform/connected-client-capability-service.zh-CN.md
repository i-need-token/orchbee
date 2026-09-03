# Orchbee Server、连接客户端与 Worker

| 字段     | 内容                                                                                                                                                                                                                                                                                                                        |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | Client-Server-Worker 产品方向已经接受；现有 Browser ingress 与 Desktop SSH 底座继续作为已交付现行权威，第一条原生本机 Server-Worker process spine 已完成且尚未发布，其余本机 terminal vertical 正在推进，remote/distributed operation、consumer migration、cutover 与 deletion 继续作为有边界的未来工作                     |
| Owner    | 产品架构、Server 协调、Workbench、网络入口、Worker placement、远程 connection adapter、CLI、Agent 与未来 capability-node 维护者                                                                                                                                                                                             |
| 当前权威 | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) 以及 Desktop、Gateway、Web、Terminal Host 的所属指南                                                                                                                                                                                                                   |
| 决策权威 | 产品决策人已经接受统一的同级 Client 模型、相互独立的 Rust `orchbee`/Server/Worker 加 Zig Terminal Host 进程、第一条经 Server relay 的本机 terminal path、有界本机懒激活、第一个 milestone 中每个 terminal 单一 controller，以及下文引用的目标处置；确切 remote、production-service 与 optional-capability contract 继续保留 |
| 退出条件 | 接受并实现边界清晰的 owner-specific 切片，把维护事实迁入所属指南和测试，然后删除本提案                                                                                                                                                                                                                                      |

英文 [connected-client-capability-service.md](connected-client-capability-service.md) 是 canonical
版本；本文用于简体中文评审。

## 产品方向

一个 Orchbee 安装实例是一个站点，将来服务的范围可以超出本地 Electron 窗口。站点最终必须支持：

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

| Package                           | 创建时机与职责                                                                                                                                                                                                                            |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `packages/remote-client-protocol` | 已由接受的人类 Browser 鉴权切片创建。当前负责个人访问令牌交换与签名 Cookie session 建立契约；更广泛的生产 transport、协商与授权仍为保留决策。                                                                                             |
| 未来 Server-Worker contract       | 只随第一个已接受的具体远程 capability 创建 language-neutral authority 与真实 consumer 所需 binding。负责 node 登记与 identity 交换、capability/版本声明、lease、heartbeat、progress、result、failure、cancellation、fencing、限制与错误。 |

两个 contract 必须分开，因为人类产品客户端与 Worker 具有不同的 identity、credential、authority、撤销范围和
兼容性生命周期。两个 contract 都不是 Server 实现，不拥有产品 state，也不能授予通用远程 shell。

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

原生站点连接是已鉴权 Client 与已登记 Worker 的长期主路径。如果选择保留 SSH，它继续作为首次使用、
兼容、管理和恢复路径；它不是 Workbench、Agent、调度或 state synchronization protocol。登记后，
Worker 通常应使用非 Cookie 设备 credential 主动建立到 Server 的原生出站连接，而不要求 Server 持续维护
一条入站 SSH 登录。

用户明确授权的 SSH Project 可以在该 SSH 账号范围内提供 Terminal 与 filesystem effect。这不会自动登记
该机器，不会把 capability-node contract 变成通用远程 shell，也不会向 scheduled work 授予超出已接受 typed
capability 的 authority。

Desktop 与 Browser client 通过 remote-client 边界消费同一个已授权产品模型。即使两种连接角色使用同一个
TLS listener 与 WebSocket 实现，capability node 仍然使用独立的 node 边界。HTTP 与 WebSocket framing 不定义
产品 identity、authority、retry 或 compatibility 语义。应用 contract 保持 transport-neutral，使已接受的 SSH
tunnel、relay 或后续 transport adapter 无法创建第二套产品模型。

生产网络流量使用成熟 TLS 与 SSH 实现提供的 authenticated encryption；Orchbee 不发明 transport
cryptography。Plain HTTP 继续仅限于已经交付且明确警告风险的 trusted-LAN 预览。Client 到 Worker 的直连
数据路径、QUIC 或 WebTransport adapter、relay service 与 Internet ingress 仍是保留的优化或部署决策。
它们不能绕过 Server 签发的 authority 或 recovery fencing。

## 已接受的初始入口基础

第一个边界清晰的 Server 切片创建 `apps/server`，作为基于 Fastify 的 Node 应用。一个 HTTP/1.1 listener
在同一个 origin 和端口上同时拥有普通 HTTP 请求与 WebSocket Upgrade。初始路由 namespace 预留如下：

| Path          | 初始职责                                                                                             |
| ------------- | ---------------------------------------------------------------------------------------------------- |
| `/api/*`      | 应用 HTTP API namespace，初始版本位于 `/api/v1`；未实现的 endpoint 返回 JSON，绝不返回 renderer HTML |
| `/ws/*`       | 应用 WebSocket namespace，初始版本位于 `/ws/v1`；普通 HTTP 请求或未知 Upgrade fail closed            |
| `/static/*`   | 不可变 renderer asset；缺失 asset 返回 `404`，绝不落入 Browser Router fallback                       |
| 其他产品 path | 已鉴权的 `GET` 与 `HEAD` HTML navigation 返回 renderer entry；未鉴权 navigation 重定向到 `/login`    |

Fastify 拥有 HTTP lifecycle、路由与 plugin composition。其官方 WebSocket 和 static plugin 分别处理
Upgrade 与 renderer 交付；应用 schema 继续在各自信任边界由 Zod 拥有。初始基础不定义外部 API 或
WebSocket message contract。后续有界的人类 Browser 鉴权切片创建了
`@orchbee/remote-client-protocol`，但没有创建通用产品 API。

loopback 仍是独立 Server 的默认值。已接受的 Desktop 所属 trusted-LAN 预览是唯一的生产前例外：用户必须
在 UI 中明确操作，Server 与 Gateway 才绑定 `0.0.0.0`；产品只发布枚举得到的 RFC 1918 IPv4 Origin；
系统要求交换个人访问令牌，并使用得到的签名 Cookie 准入每个 exact-Origin Gateway control connection；
UI 明确警告 HTTP 可能暴露 credential 与终端流量；停止、切换模式、重置令牌或退出进程都会撤销 session。
这个例外不是生产 Internet 部署或多用户账号设计。单独启动 Server entry 不等于启用共享。

## 人类用户的 Web 访问方向

网络暴露默认关闭。用户启用共享后，已接受的站点入口才在明确的 interface 与端口上开始监听；可分发的入口是
类似 `https://192.0.2.1:8443/` 的短 origin，后面只跟普通产品路由。credential、bootstrap token 与
session capability 绝不能出现在被分享的 query string 或 fragment 中。

未鉴权的 Browser 请求保留其原本要访问的产品 path，同时进入登录流程。用户输入单独签发的 access token；
交换成功后，系统创建有界且可撤销的人类 session，通过范围适当的 Cookie 表示，然后恢复原目标路由。这个
人类登录流程绝不能登记 capability node，也不能授予设备 credential。

已交付预览使用一个安装范围的 256 位个人访问令牌，只持久化其 verifier，只在创建或重置后展示明文，并在
内存中按来源限制失败尝试后交换令牌。Session Cookie 包含签名随机 identity，使用 `HttpOnly`、
`SameSite=Strict`、`Path=/`，并持续到 Browser session 结束。每次 Browser 访问 activation 使用新签名密钥；
停止、切换模式、重置和退出都会使之前的 session 失效。令牌交换与 Gateway control 准入都要求精确 Origin。
TLS 要求、生产 session duration 与 revocation storage、多用户授权和账号恢复仍为保留决策。

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

## 候选职责拆分

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
私有 Rust Worker 与独立 Zig Terminal Host。其增量底座与第一条本机 Server-Worker process spine
已经交付但尚未发布，其余本机 terminal vertical 正在推进；当前与 Desktop 共置的 runtime 在有边界
migration slice 替换前保持权威。第一条完整本机 terminal path 经 Server relay，裸 `orchbee` 可以
懒激活确切本机 cohort，但不监督其生命周期。共置绝不能合并 trust、state 或 lifecycle 职责。
Production service installation、remote enrollment 与任何由测量证明必要的 direct Worker
attachment 继续保留。

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

远程算力以有类型的 capability 暴露，而不是通用远程命令执行。每个已接受的 capability contract 必须定义：

- 版本化的 input、progress、result、failure、cancellation 与 compatibility schema；
- 资源需求和 scheduler 可见的容量，同时不暴露无关设备数据；
- lease identity、过期、heartbeat、重复投递行为与幂等边界；
- artifact、源码数据、secret、日志与诊断的传输策略；
- 断连、重新分配、部分结果与迟到结果的行为；
- 授权工作并提交 durable outcome 的产品 owner。

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

1. Agent integration，包括 orchestration 语义、可选 hook helper、Worker-local execution、
   association、installation cohort 与 failure policy。
2. Managed Worktree、Archive、Launch Profiles、SSH 专属 Desktop 行为，以及 split、restart、
   close、export 与长期 history 等更广 terminal lifecycle。
3. 当前 SSH 路径的 migration 或 sunset，包括每个 consumer 与 installed artifact；与此独立，
   任何未来 unmanaged-SSH adapter 的产品范围、时机、transport、authentication 与 lifecycle。
4. Production human/device session、Worker enrollment、remote capability contract、scheduling、
   lease、fencing、cancellation、quota 与 data-locality policy。
5. Production HTTPS/WSS、TLS 与 Internet ingress、relay 或 NAT traversal、mobile backgrounding、
   multi-user tenancy、authorization hierarchy、audit、secret 与 artifact transfer。
6. Client 到 Worker 的 direct terminal path。只有 Server relay 测量证明有必要，并且 scoped
   capability、expiry、revocation、accounting、reconnect 与 fallback 单独获接受后才能准入。
7. 已接受有界本机懒激活之外的 production service installation、login-time start、update cohort、
   compatibility、durable Server recovery、backup、rollback 与 uninstall。
8. 裸 TUI、`--help` 与 `--version` 之外的 public command grammar；稳定 automation output 与 exit
   contract；Skill 与 completion；package-manager distribution；public cutover 与 compatibility policy。
9. Node Server preview、Gateway/protocol、SSH Runtime/protocol、混合 Workbench package、旧 CLI 或
   已迁移 Desktop authority 的每项 deletion。删除需要各自的 consumer、installed-artifact、behavior、
   migration 或 sunset、documentation、evidence 与 rollback 审计。

没有具体且已接受的纵向 consumer 时，不得创建空的 remote Worker protocol workspace，也不得扩展
现有 preview protocol。当前 Browser ingress 与 SSH 路径只对已交付行为保持权威，直到完整替代或
显式 sunset 获得接受并验证。

## 交付顺序

现行已交付行为属于其 application guide 与可执行测试，而不再通过本提案中的阶段流水账维护。

| Milestone                       | 可独立审阅的 outcome                                                                                                                                                                                                                                  |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 现有现行路径                    | 在增量原生工作仍未发布期间，让 Node Browser-ingress preview、当前 TypeScript CLI、Desktop-colocated coordinator 与已交付 SSH 路径继续对其受支持行为保持权威。                                                                                         |
| 原生底座                        | **已经完成且尚未发布：**根 Rust 支持、framework-neutral Workbench projection contract、原生 `orchbee` 入口底座、Rust Server kernel 与 Rust Terminal Host binding。[`native-runtime-topology.md`](native-runtime-topology.zh-CN.md) 链接其现行 owner。 |
| 本机 terminal 纵向链路          | **正在推进且尚未发布：**真实本机 Server/Worker process spine 已完成；下一步交付最小 Host broker、经 Server relay 的 terminal stream 与真实 TUI Client，随后完成 fault 与 performance 收敛。该路径不改变当前 consumer，也不进行 public cutover。       |
| 共享 Client 与 Server migration | 把 canonical Workbench coordination 和受支持 Browser session/relay 行为迁入 Rust Server，把 React presentation 迁入 `apps/web`，把 Desktop 缩减为 Client/platform adapter，并且只迁移显式准入的 CLI capability。                                      |
| 可选与分布式 capability         | 通过单独纵向 capability 审阅 Agent、Worktree、Archive、Launch Profiles、未来 SSH、已登记 remote Worker、mobile/remote Client、production authentication 与 scheduling。                                                                               |
| 旧实现退役                      | 只有每个替代或 sunset cohort 证明 compatibility、installed-artifact handling 与 rollback 后，才删除 Node Server preview、Gateway/protocol、SSH Runtime/protocol、混合 Workbench package、旧 CLI 与已移除 Desktop authority。                          |

每个 implementation slice 都必须保留已接受 role boundary，并证明自身 security、privacy、
compatibility、observability、rollback 与 recovery claim。历史交付细节留在所属现行指南；本提案
记录尚未解决的目标决策，而不是任务 chronology。
