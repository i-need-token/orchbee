# Orchbee Server、连接客户端与能力节点

| 字段     | 内容                                                                                                                                                                                                                                                                                                                                |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | 产品方向、Client-Hub-Worker 拓扑、原生与 SSH 双连接策略及连续性不变量已接受；初始入口、有界的个人访问令牌换 Cookie 预览、面向 Agent 的本地 CLI、Slice 2.2 SSH Runtime spine、Slice 2.3A-C identity/release/activation composition 与 Slice 2.4 bounded Project-root inspection 已交付；Project placement 与其余实现契约仍是保留决策 |
| Owner    | 产品架构、Server 协调、Workbench、网络入口、远程 Environment、CLI、Agent 与未来 capability-node 维护者                                                                                                                                                                                                                              |
| 当前权威 | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) 以及 Desktop、Gateway、Web、Terminal Host 的所属指南                                                                                                                                                                                                                           |
| 退出条件 | 接受并实现边界清晰的 owner-specific 切片，把维护事实迁入所属指南和测试，然后删除本提案                                                                                                                                                                                                                                              |

英文 [connected-client-capability-service.md](connected-client-capability-service.md) 是 canonical
版本；本文用于简体中文评审。

## 产品方向

一个 Orchbee 安装实例是一个站点，将来服务的范围可以超出本地 Electron 窗口。站点最终必须支持：

- Desktop、Browser、手机与 CLI 客户端只能观察或控制其获得授权的产品 session；
- 性能相对较弱的工作电脑只作为远程站点的产品客户端；
- 另一台已登记的电脑提供有类型的算力或设备 capability；
- 协调电脑在协调其他 node 的同时提供本地 capability。

因此，对外可访问的服务是应用后端，而不只是静态文件服务器或反向代理。Web 静态资源交付、TLS 终止与
请求转发可以是部署职责，但不能拥有 session、授权、设备登记、capability lease、调度或产品 state。

这个方向不会把当前仅用于本地开发的 `apps/gateway`，或 Terminal Host 的同用户私有 endpoint，直接
变成网络服务。在分别接受替代实现或 adapter 之前，这些现有边界继续有效。

## 已接受的分布式工作台拓扑

已接受的逻辑拓扑包含三个独立授权的角色：

| 角色   | Authority                                                                                                                             |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| Client | 渲染一个已授权产品 projection、提交语义 command，并拥有客户端本地 view 与平台 effect；不拥有站点级 durable 产品 state                 |
| Hub    | 拥有 canonical Workbench state、已授权语义 command 的提交、Agent 编排、Environment registry、capability placement 与 durable recovery |
| Worker | 拥有本地 Checkout、文件系统、Git、PTY、进程与有界 Agent executor effect；不能直接提交 canonical Workbench state                       |

一台机器可以共址运行 Client、Hub 与 Worker 角色，但共址不会合并它们的 identity、authority、state 或
lifecycle。性能较弱的工作电脑可以只运行 Client。中心服务器可以运行 Hub 与一个 Worker，同时协调其他
Worker。**Terminal Host** 继续表示 node 本地的 PTY 服务，绝不表示中心 Hub。

Project 是不依赖主机的产品目标。Checkout 是某个执行 Environment 中的一份具体源码树。Workspace 选择一个
Primary Checkout，从而派生其 Terminal 与 Executor Agent 的默认 placement。已接受的关系尚未定义 durable
schema、migration、跨机器源码传输策略，也未决定在内部重命名还是适配 `Directory`；这些决策需要一个边界
清晰的 Workbench proposal。

## 已接受的语义控制方向

人类控件、Orchbee CLI、Skill 与已批准 automation 必须到达同一组 owner-defined 语义操作，不能实现平行
mutation。CLI 是稳定的调用界面，不是产品 state Store 或 authority 边界。Skill 组合结构化 CLI 操作；当
存在符合条件的语义操作时，Skill 绝不解析面向人的展示文本，也不模拟 pointer、DOM 或 keyboard input。

当前进程内的 Action registration 与 prepared invocation 继续作为本地 runtime capability。它们不是可序列化
的网络请求，也不会因为能够枚举就获得远程调用能力。每个允许 CLI 或远程调用的操作都需要明确且版本化的
input 与安全 output schema、caller authorization、非交互 confirmation policy、compatibility lifecycle、
idempotency boundary 以及所属 domain handler。focus、窗口 geometry、native dialog 与 clipboard access 等
客户端本地操作需要一个精确且已授权的 Client Session；它们绝不能隐式改变所有连接客户端的 view。

Durable ownership 拆分如下：

| State 或 effect                                                             | Owner                              |
| --------------------------------------------------------------------------- | ---------------------------------- |
| Project、Workspace、共享 Tab/Panel 结构、Agent task 与 scheduling state     | Hub product coordinator            |
| Checkout、文件、Git、PTY、进程与本地 Agent executor state                   | 精确的 Worker capability owner     |
| Active location、selection、focus、maximization、窗口 geometry 与 clipboard | 精确的 Client Session 或客户端平台 |

## 已接受的命名与建包时机

正式后端可执行进程命名为 **Orchbee Server**。其可运行 workspace 位于 `apps/server`，package identity
使用 `@orchbee/server`；已接受的初始入口切片创建了这个 workspace。`Server` 表示完整的产品后端进程，
但不会把网络入口、鉴权、协调、设备 identity、调度和执行混成一个没有内部边界的模块。

两个协议边界的名字已经接受，并且只能随具体 consumer 创建：

| Package                             | 创建时机与职责                                                                                                                                                                  |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `packages/remote-client-protocol`   | 已由接受的人类 Browser 鉴权切片创建。当前负责个人访问令牌交换与签名 Cookie session 建立契约；更广泛的生产 transport、协商与授权仍为保留决策。                                   |
| `packages/capability-node-protocol` | 随第一个已接受的具体远程 capability 创建。负责 node 登记与 identity 交换、capability/版本声明、lease、heartbeat、progress、result、failure、cancellation、fencing、限制与错误。 |

两个协议必须分开，因为人类产品客户端与 capability node 具有不同的 identity、credential、authority、撤销范围和
兼容性生命周期。两个 package 都不是 Server 实现，不拥有产品 state，也不能授予通用远程 shell。

当前 `apps/gateway` 与 `@orchbee/gateway-protocol` 继续明确作为仅限预览的 Browser transport，直到
生产 external-client 切片迁移或淘汰它们。它们可以绑定 loopback 或用户明确选择的 trusted-LAN interface。
`apps/server` 使用个人访问令牌换取签名 session Cookie，Gateway 使用同一个 session 准入 exact-Origin
control connection；访问 URL 不含 credential。它们不得改名为 `server`，也不得被视为生产
external-client 契约。生产迁移不能长期维护两套相互竞争的外部客户端网络契约。

## 已接受的连接策略

Orchbee 支持两个互补的连接族：

| 连接族          | 职责                                                                                                                                        |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| 原生站点        | HTTPS 建立已鉴权的人类或设备 session；WSS 通过 role-specific contract 传输版本化 control、event、progress 与有界 binary projection message  |
| SSH Environment | 连接未受 Orchbee 管理的机器、验证其 host identity、Bootstrap 或修复 Orchbee runtime，并可以在一个明确有界的阶段通过 tunnel 承载原生协议流量 |

原生站点连接是已鉴权 Client 与已登记 Worker 的长期主路径。SSH 继续作为首次使用、兼容、管理和恢复路径；它不是
Workbench、Agent、调度或 state synchronization protocol。登记后，Worker 通常应使用非 Cookie 设备
credential 主动建立到 Hub 的原生出站连接，而不要求 Hub 持续维护一条入站 SSH 登录。

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
它们不能绕过 Hub 签发的 authority 或 recovery fencing。

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

| 职责                                 | 候选 owner                                                                                            |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| HTTP/WebSocket 入口与 Web UI         | 网络入口服务；拥有 TLS、Origin policy、路由 fallback、限流与 renderer 静态资源交付                    |
| 人类用户鉴权                         | 站点 identity/session 服务；拥有 bootstrap token 换取、安全 Cookie session、过期、撤销与审计          |
| 产品协调                             | Hub 应用 coordinator；拥有已授权 Workbench session、canonical state、语义提交与恢复                   |
| Environment 与 Checkout registry     | 产品 Environment owner；拥有稳定 placement identity、观测到的 availability 与有界 provider adaptation |
| Capability node 登记                 | 设备 identity 服务；拥有配对、设备 credential、attestation 输入、轮换、撤销与在线状态                 |
| 工作调度                             | 有类型的 capability scheduler；拥有准入、放置、lease、取消、重试、配额与结果 ownership                |
| 本地终端执行                         | 位于现有受信本地 broker 边界之后的 Terminal Host                                                      |
| 远程执行                             | 单独鉴权的 node runtime，只暴露已接受且版本化的 capability contract                                   |
| CLI 与 Skill 调用                    | 狭窄的语义 command client；提供结构化调用和观察，但不拥有产品 state                                   |
| Desktop、Browser 与手机端 projection | 消费已授权应用 API 的产品客户端；终端形态本身不会自动获得后端 authority                               |

逻辑 Client-Hub-Worker 拓扑已经接受；最终物理进程拓扑仍是保留决策。第一版可以让入口与 coordinator 和
Desktop 共址，但进程共址不能合并各自的信任、state 或生命周期职责。后续的常驻后台服务必须能够保持相同
契约，且不需要实现另一套 Workbench renderer。

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

| 形态                 | 必需行为                                                                                                |
| -------------------- | ------------------------------------------------------------------------------------------------------- |
| 单台电脑             | Client、Hub、本地 Worker capability 与可选 Web 入口可以共址                                             |
| 弱性能远程工作电脑   | Desktop、Browser、手机或 CLI Client 渲染并控制一个已授权远程 Hub，而不成为 Worker                       |
| SSH Environment      | 当前产品 coordinator Bootstrap 并附着一个未受 Orchbee 管理的远程 Project Environment，而不虚构一个 Site |
| 分布式站点           | 一个 Hub 通过明确 identity 与 capability 协调本地 Worker 和其他已登记 Worker                            |
| Coordinator 暂时离线 | Client 与 node fail closed，或进入明确建模的降级模式；双方都不能凭空产生 authority                      |

LAN 发现、Internet relay、NAT 穿透、公共 DNS 与云端 rendezvous 是传输和部署选择，不是产品授权机制。

## 不变量

- 用户明确启用并配置网络暴露之前，网络入口默认关闭。
- 在每个信任边界解析所有外部消息，并在鉴权后执行授权；知道 IP 地址和端口不会获得任何 authority。
- 人类 session、设备 identity、capability lease、terminal attachment 与 Workbench client session 使用
  不同 identity 和撤销范围。
- Hub、Worker 与 Client Session state 分别只有一个已接受的 authority；transport adapter 与共址不能创建
  另一个 writer。
- Project 不依赖机器 identity；每个具体 Checkout 与执行 effect 必须先通过一个明确的 Environment 解析，
  然后才能到达 Worker。
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
- Browser、Electron 与手机客户端观察同一个已授权产品模型和路由语义；传输选择不能产生另一套
  Workbench 实现。
- 审计记录默认只包含 identity、决策与有界 metadata，绝不包含 terminal 内容、credential、
  capability token 或 secret payload。

## 保留决策

实现前仍需分别接受：

- 站点 coordinator 是 Electron-owned child、独立监督的本地服务，还是可部署 server；
- 把已交付的 append-only Environment registry 扩展到 Checkout 与 Workspace placement、edit 或 removal、
  本地数据 migration、provider discovery、源码传输与跨机器 Git policy；
- 扩展已交付的 Agent-ready CLI catalog，包括每项新增 operation 的结构化合约、authorization、
  compatibility、confirmation 与 client-view targeting policy；
- Production SSH Runtime signer operation 与 artifact publication 或 network feed；Windows AskPass；
  operating-system service 或 linger integration；version repair、removal 与旧版本 cleanup。Slice 2.2 已经固定 target resolution、host-key
  admission、POSIX AskPass、受监督 OpenSSH stdio attachment、signed no-download bootstrap、双平台矩阵、
  immutable installation、精确 Hello/epoch/heartbeat protocol、generation fencing 与 reconnect policy；
  Slice 2.3A 已经固定相互独立的 Environment 与 Runtime identity、精确 SSH target uniqueness、append-only
  幂等 create、严格本地 persistence 与 indeterminate-commit reconciliation。Slice 2.3B 已经固定 canonical
  双平台 release、外部 Ed25519 signing boundary、精确固定 manifest 的 Desktop trust descriptor、
  release-only package command 与 private staging。Slice 2.3C 已经固定显式 Environment-owned activation 与
  same-child installed-artifact negotiation。Slice 2.4 已经固定精确 `project-root-v1` admission 与有界
  canonical-directory observation，但不授予 Project placement 或通用 filesystem authority；
- 登记与恢复 ceremony、设备 identity 技术、credential 存储、轮换、撤销与管理员策略；
- 生产 HTTPS/WSS endpoint、session-resumption credential、协议协商、heartbeat、backoff、snapshot/delta、
  binary attachment、backpressure 与 replay bounds；
- LAN-only、Internet 可达、relay 与 NAT 穿透的部署支持；
- 是否允许任何已授权的高流量数据路径让 Client 直连 Worker；如果允许，所需 Hub-issued ticket、revocation、
  accounting 与 fallback；
- 第一个具体远程 capability，以及其数据能否离开主电脑；
- 调度策略、配额、公平性、优先级、抢占、能耗感知与用户同意；
- 多用户 tenancy、授权层级、共享范围与审计保留；
- 协议版本协商、滚动更新、兼容性与 node quarantine；
- artifact 与 secret 传输、静态加密、删除、恢复与失败诊断；
- Client、Hub、Worker 或 transport 离线期间的精确降级行为与保留时长；
- 服务安装、自动启动、更新连续性、备份与卸载行为。

在另一个边界清晰的切片拥有具体 consumer 和已接受 contract 之前，不得创建空的
`capability-node-protocol` workspace，不得把 `@orchbee/remote-client-protocol` 扩展到已接受的人类
Browser 鉴权契约之外，也不得把 `@orchbee/gateway-protocol` 扩展成生产契约。已交付的 Server 与 Browser
鉴权切片不授权 capability-node 或生产远程执行协议。

## 交付顺序

初始的 default-loopback Server 入口、明确启用的 trusted-LAN 预览，以及有界的人类个人访问令牌换 Cookie
鉴权，已经在完整路线之前交付。它们是复用的基础，不能证明后续阶段已经完成。

| 阶段 | 可独立交付的结果                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0    | 本提案已接受：逻辑 Client-Hub-Worker 角色、authority 拆分、原生与 SSH 双策略、连接连续性不变量、有序阶段与明确的保留决策                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 1    | 已通过当前[面向 Agent 的本地 CLI](../../apps/cli/README.zh-CN.md) 交付，为一组已接受的 owner-defined semantic operation 提供结构化 input/output、execution observation、cancellation、authorization 与非交互 policy；它不增加 remote execution                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 2    | 进行中：[Desktop SSH 连接基础](../../apps/desktop/docs/ssh-environment-connection.zh-CN.md)已交付 target resolution、host-key 准入、瞬时 POSIX AskPass、一个受监督 OpenSSH stdio transport、signed no-download Runtime bootstrap、精确 Hello/epoch/heartbeat、连接复用、retained-daemon attachment recovery、installed-artifact negotiation 与一个有界 Project-root observation service。[durable Environment registry](../../apps/desktop/docs/environment-registry.zh-CN.md)交付稳定 provider-neutral identity 与严格 append-only persistence，而[离线 Runtime release composition](../../apps/desktop/docs/ssh-runtime-release.zh-CN.md)交付外部 signing、精确 Desktop trust 与 release-only package placement。其余工作将增加 Project、Checkout 与 Workspace placement，以及任何需单独接受的远程 filesystem 或 Terminal Host service |
| 3    | 让 Project 不依赖主机，并增加明确的 Environment 与 Checkout placement，使一个 Project 可以包含本地与远程 Workspace；从手工 placement 开始，不自动进行跨机器源码同步                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| 4    | 建立 headless Hub authority 与生产 remote Client session，使弱性能 Desktop 或 Browser client 可以重连到同一个 Workbench、Terminal 与 durable Agent work；在接纳不受信 writer 之前，把受信的 whole-document proposal 迁移为已授权的语义 command                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 5    | 增加 durable Hub Agent 编排、Worker-local executor Agent，以及组合已授权 CLI 调用面的 Skill；一个目标可以协调明确选择的 Worker，但不自动进行资源调度                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| 6    | 增加 node 登记与只读 capability inventory，然后增加一个已建模的远程 capability，再为其增加基于 lease 的 scheduler；在接纳第二种 capability 前，为 heartbeat、fencing、cancellation、retry、data locality、fault injection 与 security 提供 evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| 7    | 使用已接受的 TLS 与 ingress policy、按需的多用户授权、按需的 relay 或 NAT 穿透、secret 与 artifact policy、quota、audit、rolling compatibility、backup、service supervision 与 Hub recovery 强化生产部署                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

每个阶段都必须在编辑受治理实现之前拥有自己的已接受 bounded 或 consequential authorization。Security、privacy、
compatibility、observability、rollback 与 recovery 是每个阶段的累积要求，不能全部推迟到阶段 7。
