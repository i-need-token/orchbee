# Orchbee Server、连接客户端与能力节点

| 字段     | 内容                                                                                                      |
| -------- | --------------------------------------------------------------------------------------------------------- |
| 状态     | 产品方向、初始入口与有界的个人访问令牌换 Cookie 预览已接受并交付；生产信任模型与远程执行契约仍是保留决策  |
| Owner    | 产品架构、Desktop 服务、网络入口与未来 capability-node 维护者                                             |
| 当前权威 | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) 以及 Desktop、Gateway、Web、Terminal Host 的所属指南 |
| 退出条件 | 接受并实现边界清晰的 owner-specific 切片，把维护事实迁入所属指南和测试，然后删除本提案                    |

英文 [connected-client-capability-service.md](connected-client-capability-service.md) 是 canonical
版本；本文用于简体中文评审。

## 产品方向

一个 Orchbee 安装实例是一个站点，将来服务的范围可以超出本地 Electron 窗口。站点最终必须支持：

- Browser 与手机客户端只能观察和控制其获得授权的产品 session；
- 另一台已登记的电脑提供有类型的算力或设备 capability；
- 主电脑在协调站点的同时提供本地 capability。

因此，对外可访问的服务是应用后端，而不只是静态文件服务器或反向代理。Web 静态资源交付、TLS 终止与
请求转发可以是部署职责，但不能拥有 session、授权、设备登记、capability lease、调度或产品 state。

这个方向不会把当前仅用于本地开发的 `apps/gateway`，或 Terminal Host 的同用户私有 endpoint，直接
变成网络服务。在分别接受替代实现或 adapter 之前，这些现有边界继续有效。

## 已接受的命名与建包时机

正式后端可执行进程命名为 **Orchbee Server**。当第一个已接受的生产切片确实需要该进程时，其可运行
workspace 使用 `apps/server`，package identity 使用 `@orchbee/server`。`Server` 表示完整的产品后端
进程，但不会把网络入口、鉴权、协调、设备 identity、调度和执行混成一个没有内部边界的模块。

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

## 候选职责拆分

| 职责                         | 候选 owner                                                                                             |
| ---------------------------- | ------------------------------------------------------------------------------------------------------ |
| HTTP/WebSocket 入口与 Web UI | 网络入口服务；拥有 TLS、Origin policy、路由 fallback、限流与 renderer 静态资源交付                     |
| 人类用户鉴权                 | 站点 identity/session 服务；拥有 bootstrap token 换取、安全 Cookie session、过期、撤销与审计           |
| 产品协调                     | 站点应用 coordinator；拥有已授权的 Workbench session、shared-document authority 与 capability registry |
| Capability node 登记         | 设备 identity 服务；拥有配对、设备 credential、attestation 输入、轮换、撤销与在线状态                  |
| 工作调度                     | 有类型的 capability scheduler；拥有准入、放置、lease、取消、重试、配额与结果 ownership                 |
| 本地终端执行                 | 位于现有受信本地 broker 边界之后的 Terminal Host                                                       |
| 远程执行                     | 单独鉴权的 node runtime，只暴露已接受且版本化的 capability contract                                    |
| Browser 与手机端展示         | 消费已授权应用 API 的产品客户端；终端形态本身不会自动获得后端 authority                                |

最终进程拓扑仍是保留决策。第一版可以让入口与 coordinator 和 Desktop 共址，但进程共址不能合并各自的信任、state
或生命周期职责。后续的常驻后台服务必须能够保持相同契约，且不需要实现另一套 Workbench renderer。

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
都不能成为 Workbench shared state 的 writer。Product Action 继续是本地语义契约，除非单独评审的 API
明确暴露某一项操作。

## 部署形态

| 形态                  | 必需行为                                                                          |
| --------------------- | --------------------------------------------------------------------------------- |
| 单台电脑              | Electron、站点协调、本地 capability 与可选 Web 入口可以共址                       |
| 共享 Browser 或手机端 | 站点交付 renderer，并在暴露产品 API 前鉴权人类 session                            |
| 额外算力电脑          | 已登记 node 声明有类型的容量，并且只接收明确调度的 lease                          |
| Coordinator 暂时离线  | 客户端与 node fail closed，或进入明确建模的降级模式；双方都不能凭空产生 authority |

LAN 发现、Internet relay、NAT 穿透、公共 DNS 与云端 rendezvous 是传输和部署选择，不是产品授权机制。

## 不变量

- 用户明确启用并配置网络暴露之前，网络入口默认关闭。
- 在每个信任边界解析所有外部消息，并在鉴权后执行授权；知道 IP 地址和端口不会获得任何 authority。
- 人类 session、设备 identity、capability lease、terminal attachment 与 Workbench client session 使用
  不同 identity 和撤销范围。
- 在当前契约下，Terminal Host 保持私有本地传输，绝不直接接受 Internet 或 LAN peer。
- 某个已接受 capability 只需要较窄 effect 时，远程执行不能因此获得通用 Desktop、文件系统、shell、
  secret 或 Action capability。
- 没有明确建模的 fencing 机制时，断连和重试不能重复提交一个逻辑结果，也不能把一个 lease 交给两个
  活动 owner。
- Browser、Electron 与手机客户端观察同一个已授权产品模型和路由语义；传输选择不能产生另一套
  Workbench 实现。
- 审计记录默认只包含 identity、决策与有界 metadata，绝不包含 terminal 内容、credential、
  capability token 或 secret payload。

## 保留决策

实现前仍需分别接受：

- 站点 coordinator 是 Electron-owned child、独立监督的本地服务，还是可部署 server；
- 登记与恢复 ceremony、设备 identity 技术、credential 存储、轮换、撤销与管理员策略；
- LAN-only、Internet 可达、relay 与 NAT 穿透的部署支持；
- 第一个具体远程 capability，以及其数据能否离开主电脑；
- 调度策略、配额、公平性、优先级、抢占、能耗感知与用户同意；
- 多用户 tenancy、授权层级、共享范围与审计保留；
- 协议版本协商、滚动更新、兼容性与 node quarantine；
- artifact 与 secret 传输、静态加密、删除、恢复与失败诊断；
- 客户端、coordinator 与 node 的离线行为；
- 服务安装、自动启动、更新连续性、备份与卸载行为。

在另一个边界清晰的切片拥有具体 consumer 和已接受 contract 之前，不得创建空的
`capability-node-protocol` workspace，不得把 `@orchbee/remote-client-protocol` 扩展到已接受的人类
Browser 鉴权契约之外，也不得把 `@orchbee/gateway-protocol` 扩展成生产契约。已交付的 Server 与 Browser
鉴权切片不授权 capability-node 或生产远程执行协议。

## 交付顺序

1. 已交付：建立默认绑定 loopback 的 Orchbee Server 入口基础，以及上文单独明确启用的 trusted-LAN 预览。
2. 已交付：把 opt-in Web 入口和人类个人访问令牌换 Cookie 鉴权作为一个边界清晰的切片实现。
3. 实现设备登记与只读 typed capability inventory，不执行远程工作。
4. 选择一个具体 compute capability，并建模它的 lease、取消、失败、数据与 fencing 语义。
5. 在引入第二种 capability 之前，为第一种 capability 的调度和远程执行提供 deterministic、
   fault-injected 与 security evidence。
