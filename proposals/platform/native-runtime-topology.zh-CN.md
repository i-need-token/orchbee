# 原生运行时拓扑与职责迁移

| 字段     | 值                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 状态     | 已接受的重大目标；增量原生底座、本机 Server-Worker 与 Client Session process spine、已检查的 terminal Client authority model，以及 Worker-Host Broker connection spine 已经完成且尚未发布，其余本机终端纵向实施正在推进，后续 migration、cutover 与删除继续受本设计边界约束                                                                                                                                                                                                                                                                                                          |
| 负责人   | 产品架构，以及 Client、Server、Desktop、Worker、Terminal Host 与 Agent 集成边界的维护者                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| 现行权威 | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md)、各 app 与 package 的归属指南，以及 [`connected-client-capability-service.md`](connected-client-capability-service.zh-CN.md) 中已接受的逻辑拓扑                                                                                                                                                                                                                                                                                                                                                                                 |
| 决策权威 | 产品决策人已经接受 Client-Server-Worker 拓扑，彼此独立的 Rust `orchbee`、Rust Server、Rust Worker 与 Zig Terminal Host executable，下文汇总的已交付原生底座、代码无关 launch 边界、统一多 Client 模型、不让 Client 拥有生命周期的本机懒激活、第一条经 Server relay 的本机 terminal 路径、本 milestone 中每个 terminal 一个 input-and-resize controller，以及下文 CLI、Gateway、SSH Runtime/protocol、Workbench package 与非 SSH Desktop 处置。Agent、Worktree、Archive、Launch Profiles、SSH 专属 Desktop 行为、direct Worker attachment、collaborative input 与后续生产事项仍然保留 |
| 退出条件 | 接受边界清晰的交付切片，以映射好的证据完成实施，把需要维护的事实迁入现行权威，解决每个 workspace 的处置问题，并在本提案不再承载未决未来设计证据后删除它                                                                                                                                                                                                                                                                                                                                                                                                                              |

英文规范原文：
[native-runtime-topology.md](native-runtime-topology.md)。
如有冲突，以英文为准。

## 决策边界

本提案细化父提案的物理进程拓扑，并显式收窄其用于原生终端启动的逻辑 placement 模型。
当前产品 runtime 尚未形成这一目标形态。下文通过当前 owner 汇总已完成的增量底座、本机
Server-Worker、Client Session 与 Worker-Host connection spine，以及已检查的 terminal Client
authority model。其余本机终端纵向里程碑正在推进；其他 runtime
implementation、migration、cutover 与 deletion 继续受下文边界与保留决策约束。

已接受方向如下：

- 本机与分布式运行共用一套逻辑 Client-Server-Worker 拓扑；
- 强制提供一个原生 Rust `orchbee` 入口，同时包含 CLI 和 TUI 界面；
- 提供彼此独立的私有 Rust `orchbee-server`、Rust `orchbee-worker` 与 Zig
  `orchbee-terminal-host` executable；可选 Agent helper 的 topology、name 与 language 随 Agent
  integration 继续保留；
- Web、Desktop 和 mobile 是可选 Client 界面，消费同一个 Site 权威；
- Terminal Host 是独立的节点本地进程，也是唯一的 PTY/VT 权威；以及
- Worker 基线仅负责与代码无关的 Terminal 启动；Agent 运行时效果只能作为可选 capability
  extension 准入，而不能变成通用 Files、Git、远程 shell 或任意命令服务。

创建终端不依赖 Project、repository、Checkout 或 Worktree。基础启动契约选择一个 Worker，
并携带一个可选的节点本地初始工作目录；省略时使用该节点已接受的默认值。managed Worktree
功能可以产出这样的目录，但它只是可选的上游 capability，绝不会成为 Terminal 依赖。
因此目标核心不包含通用 `Environment` registry 或 `EnvironmentId` 解析。这项决策取代父提案
要求 future native terminal launch 必须先经过 Environment 再到 Worker 的通用不变量：Server
直接授权一个确切 Worker，不会让 Environment identity 到达 Terminal Host。父提案在同一次
变更中同步收窄。已经交付的 SSH Environment record 与行为在另行接受 migration 前继续作为
现行权威；unmanaged-SSH adapter 可以继续把 `EnvironmentId` 作为 connection identity，
但不能让它重新成为 Terminal 核心概念。

当前 `apps/ssh-environment-runtime` 与 `@orchbee/ssh-environment-protocol` 不是目标架构资产。
产品决策人已经接受在一个有边界的切片把每条受支持用法迁移到原生 Worker 或新的 SSH 设计，
或者显式下线该用法并处理已安装 artifact 后，将二者退役。未来 unmanaged-SSH capability 可以
在旧实现删除后重新设计；它不需要保留当前 executable、源码、package 名、framing、service、
release cohort 或 Desktop-owned topology。这项决策把 SSH 产品 capability 与当前实现分开，
本身并不授权删除代码。

产品决策人还接受替换并最终删除当前 TypeScript `apps/cli` 实现。Rust `orchbee` 入口是一套
新的 Client 实现，而不是 source-compatible port：它的公共 command、argument、output、exit
meaning、discovery 与 activation 行为将在所属的
[`native-command-surface.md`](../cli/native-command-surface.zh-CN.md) 提案中重新设计。当前 CLI
行为在已接受的 cutover 前继续作为现行权威；这项决策本身不会删除代码。

产品决策人已经接受在 Server 拥有生产 Browser session、relay 与 backpressure contract，且每个
受支持 consumer 都迁移后，删除 `apps/gateway` 与 `@orchbee/gateway-protocol`。Gateway 是当前
实现边界，不是目标进程或兼容契约。

当前 `@orchbee/workbench` package 也不是目标 package。它的 React shell、browser interaction
state、layout persistence 与 presentation logic 移入 `apps/web`；后者继续作为 Browser 与 Desktop
通过构建产物消费的唯一 renderer 实现。共享 Workbench 业务词汇、semantic intent、snapshot 与
delta contract 成为 framework-neutral 的 Server/Client contract，并拥有跨语言 source of truth；
它们不会继续作为 browser package 中的 canonical TypeScript reducer。Rust TUI 与未来 mobile
Client 在这些 contract 上各自实现 presentation。CLI 可以请求有界且版本化的 Workbench projection
snapshot，但绝不会序列化 React tree、DOM state、pixel 或另一 Client 的本地 focus 与 window state。
在 contract 与 Web 行为迁移后，删除这个混合 package，而不是把它的 browser-only identity
改造成新的跨语言权威。

下文非 SSH Desktop Main 处置已经接受为目标边界。Agent-owned 区域、managed Worktree、Terminal
Archive、Launch Profiles 与 SSH 专属 Desktop 区域仍有意保留；本提案不会通过相邻决策暗示其结论。

已接受拓扑和 Phase 1 仓库底座不隐含接受未来 command grammar、Agent design、未来 SSH design、
persistence migration、Client 到 Worker 的直连数据通道，也不隐含在每条受影响现行路径具备
已接受的 migration 或下线方案及 rollback boundary 前授权删除。

## 问题

当前运行时组合把规范 Workbench 协调、本机启动位置及 SSH connection 所有权、Terminal Host
代理、Agent 观察、语义命令服务、Browser 访问和 Desktop 平台效果集中在
`apps/desktop`。当前 `apps/server` 是一个有边界的 Node Browser 入口预览，
`apps/cli` 是经由 Electron Node 模式启动的 Node 客户端，Agent hooks 使用打包脚本。
已接受的 Client-Server-Worker 物理拓扑尚未成为已交付 runtime。

对于具备下列要求的产品，这种形态并不充分：

- CLI 和 TUI 是每个完整本机安装中必备的界面；
- 并发 TUI、Web、Desktop 与 mobile Client 实例可以观察同一个 Site；
- 一个 Site 可以协调本机与远程执行节点；
- 关闭或重启 Client 或 Server 不得隐式终止应保留的终端；
- 安装 Agent hooks 的场景需要一个小巧、快速、有严格边界的原生 helper；以及
- 面向用户的 `orchbee` 进程需要比完整后端更小、启动更快。

架构需要分别为产品意图、节点执行、终端事实和客户端本地呈现指定唯一所有者，
还需要一条迁移路径：它可以识别废弃 workspace 和模块，但不能在替代行为被接受并验证前删除它们。

## 范围

本提案覆盖：

- 逻辑角色、进程边界、可执行程序语言、可见性和生命周期；
- terminal-only 安装 cohort 与可选 Agent、managed-Worktree capability 的分离；
- 全本机 CLI/TUI、Web 和 Desktop 部署形态；
- 分布式 Worker 节点及远程或 mobile Client；
- 控制面与终端数据面路由；
- 状态、身份、协议、信任、连续性和故障的所有权；
- 以 app 和 Desktop Main 模块为粒度的当前职责到目标职责迁移；
- package 与 workspace 的保留、缩减、替换或退役候选项；
- 交付顺序、回滚边界与正确性证据；以及
- 实施前必须由产品决策人接受的问题。

## 非目标

本提案不会：

- 仅因增量底座存在，就声称目标 runtime、已接受本机 milestone、migration 或 deletion 已交付；
- 把当前 Terminal Host wire 当作 Client-Server 或 Server-Worker contract，或定义持久存储
  schema、完整公共命令语法或完整 UI 设计；
- 授权通用 Files、Git、文件系统、远程 shell 或任意命令 Worker capability；
- 要求创建终端前必须存在 Project、repository、Checkout 或 Worktree；
- 要求 terminal-only cohort 必须包含 Agent Bridge、Agent orchestration 或 managed Worktree；
- 选择生产 TLS、Internet relay、NAT 穿透、多用户租户、高可用或云部署策略；
- 承诺 Terminal Host 故障或操作系统重启后的 PTY 恢复；
- 要求改写 Desktop 技术栈，或把当前 React renderer 改成原生实现；
- 把 Web 或 Desktop 变成必备 CLI 和 TUI 界面的可选替代品；
- 选择本 milestone 单一活跃 controller 之外的最终 takeover 或 collaborative-input 策略；或
- 把较小的公共可执行程序解释成总安装体积必须下降的承诺。

## 逻辑拓扑

一个 Site 恰好拥有一个活跃的规范 Server 权威、零个或多个 Worker，以及零个或多个
Client。一台机器可以共置多个角色，但不能合并它们的身份、状态或生命周期。

```text
Client role
|- orchbee CLI
|- orchbee TUI
|- Desktop
|- Web
`- Mobile
        |
        | authentication, shared state, semantic commands
        v
Server role
`- orchbee-server
        |
        | placement, scoped lease, capability authorization
        v
Worker role x N
`- orchbee-worker
   |- Terminal launch validation
   |- Terminal Host broker --------> orchbee-terminal-host --> PTY / process tree / Ghostty VT
   `- optional Agent capability (process topology reserved)
```

Terminal Host 是物理 Worker 节点边界内的专用进程，不会形成额外逻辑角色，并属于 terminal
baseline。Agent integration 后续可以增加专用 helper，但其 process boundary 未被本提案接受。

## 原生可执行程序集合

| 可执行程序              | 语言 | 可见性 | 生命周期                | 已接受职责                                                                               |
| ----------------------- | ---- | ------ | ----------------------- | ---------------------------------------------------------------------------------------- |
| `orchbee`               | Rust | 公共   | 一次性 CLI 或交互式 TUI | 人类命令入口、TUI Client、本机 Site 发现和显式生命周期命令                               |
| `orchbee-server`        | Rust | 私有   | 长期运行的 Site 服务    | Server 权威、Client Session、规范产品状态、placement、可选 Agent 编排和远程 Client relay |
| `orchbee-worker`        | Rust | 私有   | 每个执行节点长期运行    | 节点身份、限定范围且与代码无关的启动、Host 代理和可选 Agent 运行时效果                   |
| `orchbee-terminal-host` | Zig  | 私有   | 按需保留的节点服务      | PTY、进程树、规范 Ghostty VT、终端历史、输入和 attachment 权威                           |

Agent hook helper 是否存在、executable name、language、lifetime 及其与 Worker 的关系都随 Agent
capability 保留。它不属于已接受 executable set。

建议的安装布局只保留一个面向用户的名称：

```text
bin/
`- orchbee

libexec/
|- orchbee-server
|- orchbee-worker
`- orchbee-terminal-host
```

不同安装 cohort 不会产生协议变体：

| Cohort                     | 必需可执行程序                                  |
| -------------------------- | ----------------------------------------------- |
| 仅原生 Client              | `orchbee`                                       |
| 本机 terminal Site         | `orchbee`、Server、Worker 与 Terminal Host      |
| Terminal-only 远程执行节点 | Worker 与 Terminal Host                         |
| 启用 Agent 的执行节点      | 保留；尚未接受 helper executable set            |
| Browser 或 mobile Client   | 不需要本机原生可执行程序；连接一个已授权 Server |

各平台的确切安装路径，以及完整发行版是否捆绑可选 cohort，仍然保留。Agent integration 永远不是
普通 terminal 创建的前置条件。

## 角色职责

### Client

Client 渲染一份经过授权的 projection，并提交语义意图。它为自己的确切 Client Session
拥有 active location、focus、terminal viewport、search presentation、selection presentation、
window geometry 和平台 clipboard effect。它不拥有任何规范 Site 状态、Worker lease、PTY
或 terminal lifecycle。

每个活跃 TUI invocation、browser tab，以及独立活跃的 Desktop 或 mobile surface 都是一个
Client Session。Renderer 类型不授予 authority，也不创建另一套产品模型。不同 Client Session
可以选择不同 active location 与 view state，同时消费相同的已授权共享 Workspace、Tab、Panel
与 Terminal fact。

`orchbee` 同时包含 CLI 和 TUI Client 界面。一次 CLI 调用是瞬时的；TUI 拥有更长的
Client Session。CLI 路径不得初始化 TUI 渲染，也不得加载 Server、Worker 或 Host 引擎。
单入口交互方向已经接受；确切 command grammar 与 parameter contract 由
`native-command-surface.md` 拥有，而不是从 `apps/cli` 继承。Browser、Desktop、TUI、mobile 与 CLI
可以消费相同的版本化 semantic projection snapshot，而每个交互 Client 拥有自己的 renderer 与
Client-local view state。

### Server

Server 是 Site 唯一的控制面。它拥有：

- 人类认证、授权、Client Session 生命周期和远程入口；
- 规范 Workspace、共享 Tab/Panel、Terminal intent、Agent task，以及可选 Project 或 managed
  Worktree 产品引用，但不拥有节点本地 Git effect 或 recovery journal；
- Worker registry、placement、限定范围的终端启动授权、lease 创建和 fencing 决策；
- 语义命令 admission、规范 commit、publication 与持久恢复；
- 版本化 Workbench snapshot 与 delta，暴露已授权产品 projection，但不序列化 framework-specific
  UI tree 或另一 Client 的本地 view；
- 产品级终端创建、关闭、重启、保留与 outcome reconciliation；以及
- 为无法访问本机 Worker attachment endpoint 的 Client 提供有边界的 terminal relay。

Server 不直接访问节点的 PTY、Terminal Host endpoint、Agent hook endpoint、节点本地文件或
目录、Git 进程或不受限 shell。它的 terminal relay 永远不会成为 VT、history 或
terminal-content 权威。

### Worker

Worker 向一个 Site 代表一个执行节点。它拥有：

- 已 enrollment 的 Node identity、device credential 使用、connection generation、
  capability advertisement、compatibility state 以及到 Server 的出站连接；
- 对确切 Server-issued operation、operation identity、lease、cancellation、heartbeat、
  progress、result 和 fencing 的校验与执行；
- 准入一份 `AuthorizedTerminalLaunch`，其中确切 Worker、operation identity、lease、
  generation 与 fence 均由 Server 绑定，不能由 Client field 提供权威值；
- 解析省略的节点本地默认值，并把经过校验的确切启动事实交给 Terminal Host；
- 与节点本地 Terminal Host 的唯一 broker 关系；
- 安装对应 capability 时，可选提供有边界的 Agent executor 生命周期和单独接受的本地 hook
  ingress；以及
- 重连后对 Host inventory 与活跃执行观察进行 reconciliation。

接受一个初始 `cwd` 并不等于提供 filesystem API。Worker 可以校验这项确切的启动输入，
但不会枚举目录、代理普通文件读取，也不会从路径推断 Git 含义。shell 或 Agent 进程启动后
使用普通操作系统访问。Worker 不解析用户的 `git` 命令，不修改规范 Workbench 状态，
不授权人类 Client，不调度其他 Worker，也不提供通用远程命令逃生口。

### Terminal Host

Terminal Host 仍是节点本地终端权威。它拥有 retained Terminal identity、PTY、process tree、
规范 Ghostty state、有序 input、geometry、revision、generation、history、selection、export
和 attachment-scoped capability。它只暴露私有、同用户的本机 endpoint，并在目标拓扑中把
Worker 作为常规 broker。

Terminal Host 不拥有 Site、Client Session、Project、Workspace、Tab、Panel、Worker placement、
Agent semantics、远程认证、网络 listener 或更新策略。Server 或 Client 重启不拥有它的生命周期。
Host 或操作系统故障不承诺恢复 PTY。

### Agent integration helper（保留）

如果后续接受，Agent hook helper 应当是最小化、有严格边界的同步原生进程。它会接收一次有边界的调用，只解析
本地 Worker endpoint 与调用范围内的 context，在严格 end-to-end deadline 内尝试一次
delivery，不向 stdout 或 stderr 输出产品 payload；Worker 不可用时以零状态退出且不改变
Agent 结果，也不能激活 service。它既不启动 Server 或 Worker，也不拥有持久状态。Agent
提案拥有确切 deadline 与 provider installation contract。本段不会接受该 helper 的存在、name、
language 或 process topology。

只有在明确接受这项保留语言决策，并在同一实施切片更新 Agent 提案后，选择 Rust 才会取代
现有 Agent 提案对 Zig bridge 的偏好。

## 与代码无关的终端启动边界

Terminal 创建在每个信任与所有权边界使用不同 contract。以下名称表示语义角色；确切 wire
表示仍为保留决策。

| Contract                   | 边界             | 拥有的内容                                                                                                                                  |
| -------------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `TerminalCreateIntent`     | Client 到 Server | 请求的 terminal semantics、可选 Worker constraint、cwd intent，以及 immediate 或 deferred start intent；绝不包含 lease、generation 或 fence |
| `AuthorizedTerminalLaunch` | Server 到 Worker | 确切 Terminal 与 Worker placement、operation identity、lease、generation、fence、已授权 cwd intent 与 start mode                            |
| `WorkerLaunchAdmission`    | Worker 本地      | Validation result、已接受 fence、execution phase、Host observation、retry status 与 reconciliation outcome                                  |
| `HostLaunch`               | Worker 到 Host   | 稳定 creation identity 与 fingerprint、解析后的 node-local cwd、确切默认交互 shell 启动事实，以及完整 geometry 或 deferred start            |

Server 是 placement authorization、lease generation 与 fence 的唯一 writer。Worker 只拥有这项已
签发 authorization 的节点本地 admission 与 execution state。未经 Server 校验，不得把 Client field
复制进 authorization envelope；Terminal Host 不会收到 Site、Client Session、Workspace、Project、
Checkout、Worktree、Environment、Worker lease 或 placement semantics。

Terminal 基线启动 Worker 已接受的默认交互 shell。它没有 command profile、raw shell text、
arbitrary executable 或 caller-supplied argument-vector variant。后续 profile 或
noninteractive-command capability 必须单独定义 authorization、catalog/default owner、versioned
representation，并以负向证据证明仅有 terminal-create authority 无法调用它。

### 初始目录语义

初始 `cwd` 可以是用户 HOME、临时目录、普通非 Git 目录、repository 目录或生成的 Worktree。
通用 create intent 可以携带一个显式 node-local path，也可以省略；省略表示请求所选 Worker
解析已接受的默认值。Worker 校验解析后的启动事实，Terminal Host 启动 PTY。shell 随后可以在
普通操作系统权限下切换目录，而不改变规范产品状态。

Workspace 或 Panel 可以呈现生成的 terminal，可选功能也可以在提交 intent 前解析初始目录，
但关联本身不会推导 `cwd`。保留现有 split 与 child-terminal 语义行为：它引用确切 source
Terminal，在可用时使用最新、已接受且由 Host 报告的 `currentCwd`，并把新 terminal 固定到
source Worker。不得在另一 Worker 上复用 node-local path。如果没有已接受的 source
observation，所属 feature 在提交前应用其文档规定的 fallback；Server 与 Worker 不重建
Project 或 Workspace policy。任何未来 cross-Worker path translation 都是独立 filesystem
capability，不能根据字符串相等推断。

managed Worktree 流程严格位于上游：

```text
optional managed Worktree operation --> node-local directory --> TerminalCreateIntent.cwd
```

Server 可以拥有规范 Worktree 产品引用，但不拥有 node-local Git effect、pending-operation
journal、serialization、compensation、recovery 或 cleanup。如果继续支持 managed Worktree，
一个单独接受的 node-local capability owner 必须拥有这些事实。managed Worktree 不属于
Worker 基线 capability；即使没有这项功能，也能完整使用 terminal 创建。

### 启动 geometry

Start mode 是一个封闭选择：

- immediate creation 通过已授权 operation 与 `HostLaunch` 携带一份完整 geometry；
- deferred creation 不携带部分 geometry 或 fallback geometry，让 Host 停在 `awaiting-start`，
  并要求之后经确切 Worker 提交一项 Server 授权且携带完整 geometry 的 `StartTerminal` operation。

Host 保留现有幂等 exact-replay 与 conflicting-geometry 行为。任何层都不能虚构 timeout 或
80x24 fallback。哪些 Client operation 选择 immediate 或 deferred creation，以及由哪个已授权
Client 提供第一份 geometry，仍属于 command 与 attachment policy 决策。

### 环境变量 overlay

环境变量 overlay 是 invocation-scoped capability，不是规范 Terminal intent 或持久 recovery
state。它受到边界约束并独立授权，只为确切 Worker admission 与 Host creation generation
交付，不进入 audit 或 diagnostic，也绝不在丢失或过期后重放。Server 只能保留非秘密 outcome
metadata；Host 只保留当前幂等 contract 所需的 target-normalized digest。仍需要 overlay value
的 retry 必须取得新 capability，不能重建或持久化这些值。

## Server、Worker 与 Terminal Host 为何保持分离

- Server 是与位置无关的 Site 权威，而 Terminal Host 必须与 PTY 共置。把 Host 合入
  Server，要么会阻止远程执行节点，要么会在每个节点复制一个不完整的 Server。
- Worker 是 node identity、capability、lease 与 fencing 边界。显式保留 Worker，可以让
  本机节点与远程节点使用相同产品协议，而不是维护一条隐藏的仅本机 execution path。
- Terminal Host 具有不同的 retention 边界。Worker 重连、升级或崩溃时，可以通过
  inventory reconciliation 恢复，而不终止仍由 Host 持有的 PTY 与 process tree。
- Zig 让 Ghostty integration 与 terminal resource model 留在其原生所有权域中。只有实测
  maintenance、packaging 或 runtime 证据充分时，才有理由用 Rust 改写 Host；仅减少一个
  process name 并不是理由。
- 这种分离增加一个私有、节点本地的 Worker-Host IPC 边界。Capability Node Protocol
  不是另一个 process hop；它是 Server-Worker 语义 contract，由本机 IPC 或远程连接承载。
- 对性能敏感的 frame routing 可以使用有界 batching、shared buffer，或后续获准的 direct
  attachment 设计，而不合并 authority。process count 不是首要优化目标；需要测量 startup
  work、copy、queue bound 与 failure coupling。

## 状态所有权

| 状态或效果                                                                 | 唯一权威                               | 副本或 projection                                                    |
| -------------------------------------------------------------------------- | -------------------------------------- | -------------------------------------------------------------------- |
| Workspace、共享 Tab/Panel 及可选 Project/Worktree 产品引用                 | Server                                 | 已授权且 framework-neutral 的 Client snapshot 与 delta               |
| Terminal 产品意图、placement、retention 和共享 outcome                     | Server                                 | Worker operation 与 Client presentation projection                   |
| Worker enrollment、placement authorization、lease generation 与 fence      | Server                                 | 确切 Worker 接收一项 scoped operation                                |
| Node credential 使用、connection generation、launch admission 与 execution | 确切 Worker                            | Server registry 与 reconciliation observation                        |
| 节点本地启动校验与默认值解析                                               | 确切 Worker                            | Server 保存经过授权的 intent 与非秘密 outcome                        |
| PTY、process tree、VT、terminal history、input order 和 geometry           | 确切 Terminal Host                     | attachment-scoped Client projection                                  |
| Invocation-scoped 环境变量 overlay                                         | 确切 Worker launch admission           | Host 只接收一次 value，并只保留 generation-scoped digest             |
| 可选 managed-Worktree Git effect、journal 与 recovery                      | 单独接受的 node-local capability owner | Server 只能保留规范 product intent、reference 与 reconciled outcome  |
| 可选 Agent task、orchestration 与共享 semantic observation                 | Server                                 | Worker executor observation 与 Client presentation projection        |
| Active Tab、focus、viewport、search、selection view 与 window state        | 确切 Client Session 或 client platform | 无，除非由明确接受的 client-local persistence 功能拥有               |
| Platform clipboard 和 native window effect                                 | 确切 client platform                   | 不做隐式跨 Client replication                                        |
| 可选 Agent hook delivery（若接受）                                         | 确切选定 helper invocation             | Worker 与 Server 可以观察一条已接受的有界事件；helper 不保留任何内容 |

任何共置路径都不得引入第二个 writer。尤其是，Server 记录 terminal intent 与 outcome，
但绝不保存第二份 VT；Worker 记录 Host identity 与 execution observation，但绝不保存第二个
terminal engine；Client 可以暂存 UI preview，但绝不能用 optimistic 方式提交共享 Workbench 状态。

## 进程生命周期与监督

| 进程                  | 启动策略                                               | 停止与重启的后果                                                                            |
| --------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------- |
| `orchbee` CLI         | 每次调用                                               | 只结束自身的 request 或 observation                                                         |
| `orchbee` TUI         | 人类显式启动；可以发起有界本机 Site 懒激活             | 释放自己的 Client Session 和 attachment；不会停止 Site 或 terminal                          |
| Server                | 确切 cohort 的本机懒激活或可选 user-service policy     | Client 与 Worker 重连；不能据此推断取消已保留的节点执行                                     |
| Worker                | 确切 cohort 本机激活或 enrolled-node service policy    | Host 独立保留；替代 Worker 对 Host inventory 进行 reconciliation                            |
| Terminal Host         | 首次使用 terminal 时，由 Worker 启动或发现确切兼容版本 | 只能依照已接受的无 broker/无 retained-terminal policy 退出；Host 丢失需要报告 terminal loss |
| Agent helper（保留）  | 仅在单独接受后由 Provider hook 调用                    | 候选：一次有边界尝试后退出；失败时对发起调用的 Agent fail-open                              |
| Web 或 Desktop Client | 人类显式启动                                           | 只释放该 Client Session，除非人类另行请求共享产品或 Site lifecycle command                  |

裸 `orchbee` 可以发起 Server 与本机 Worker activation，并等待有界 ready。Ready 后，每个私有
进程拥有自己的生命周期；发起 Client 不是其 supervisor，Client 退出也不是 stop signal。操作系统
用户服务仍是可选的后续交付机制。确切 launchd、systemd、Windows Service、login-time start、
update 与 uninstall policy 继续保留。

## 部署形态

### 本机 terminal Site

```text
orchbee CLI -----.
                  +--> local Server --> local Worker --> Terminal Host
orchbee TUI -----'
```

此形态不需要 Browser listener、Web renderer distribution 或 Desktop shell。Server 与
Worker 使用本机 IPC，同时保留跨节点时使用的相同身份和协议语义。

如果 Agent integration 后续接受 helper，一条候选独立 hook 路径是：

```text
Agent hook --> selected helper --> local Worker
```

省略这个尚未接受的 helper 不会改变任何 terminal-launch contract。

### Web 加 CLI 与 TUI

```text
orchbee CLI/TUI -- native local IPC --.
                                        +--> Server --> Worker --> Terminal Host
Browser -- loopback HTTP/WebSocket -----'
```

Web 形态为 Server 增加 renderer assets、Browser authentication 和 Browser ingress。
它不会增加另一个规范 coordinator。当前 Gateway 是迁移输入，不是第六个永久目标进程。

### Desktop 加 CLI 与 TUI

```text
orchbee CLI/TUI -- local IPC -----------.
                                        +--> same local Server --> Worker --> Host
Desktop shell / renderer -- local IPC --'
```

Desktop 仍是 Client shell 与 platform-effect adapter。它可以复用 Web renderer 而不启用
Browser ingress。关闭 Desktop window 不代表停止 Site，也不代表终止 retained terminal。

### 分布式 Site

```text
Machine A: Site
|- Server
|- optional local Worker
|  `- Terminal Host
`- optional local Clients

Machine B: execution node
|- Worker
|- Terminal Host
`- optional Agent helper (reserved)

Machine C: client only
`- Desktop / TUI / Web / Mobile
```

unmanaged-SSH 产品 capability 后续可以重新设计为独立的 bootstrap、recovery 或临时远程路径。
当前 SSH Environment Runtime 与协议不属于目标 executable 集；未来 adapter 也不会被悄然
重新归类为 enrolled Worker 或通用 `Environment` 领域。

## 控制面

每个共享产品 mutation 都以显式、版本化的语义命令到达 Server：

```text
Client
  |
  | semantic command
  v
Server
  |- authenticate and authorize
  |- validate identity and expected revision
  |- commit or reject canonical intent
  |- select Worker and issue AuthorizedTerminalLaunch
  |- issue exact operation and lease when a node effect is required
  `- publish the canonical result to authorized Clients
          |
          v
       Worker --> Terminal Host when required
```

创建、重命名、移动或关闭 Workspace、Tab 或 Panel 始终使用这条路径。Terminal create、
close、restart、retention 和 Agent-task operation 也使用它。Server 接收语义意图，
而不是 DOM event、pointer event 或供人阅读的 CLI output。

Client 本地的 Tab activation、focus、window geometry 和 presentation state 不会变成
Server mutation。分离 Client view 与关闭共享 Panel 或终止共享 terminal 是不同操作。

## 终端数据面

Server 首先授权每一个 terminal attachment。所有 Client renderer 的第一条已接受完整路径相同：

```text
Terminal Host <--> Worker <--> Server terminal relay <--> Client
```

Server 与 Worker 只检查有界 routing、identity、role、authorization、queue 与 recovery fact。
它们保留 terminal payload byte，绝不模拟 VT 或成为 terminal-content authority。Terminal frame、
input、history、selection 与 export 保持在规范 Workbench commit queue 之外。

后续由实测驱动的优化可以让共置的原生 Client 使用
`Terminal Host <--> Worker <--> Client`。该路径需要一份确切的 Server-issued attachment
capability，并与 Client Session、Terminal、Worker、connection/Host epoch、允许的 operation
以及 expiry 或 revocation 绑定。Client 直连 Worker 不属于已接受的本机终端 milestone。

Server terminal relay 可以认证、路由、执行边界限制、隔离 backpressure，并请求新
snapshot。它不解析或模拟 VT，不拥有 history，不持久保存 terminal content，也不在
input admission outcome 未知时自动重放输入。

## 主要工作流

### 本机 Site 激活

1. TUI、会修改状态的 CLI 命令、Web opener 或 Desktop launch 发现确切的本机 Site。
2. 裸 `orchbee`，以及 contract 明确允许的其他 command，可以发起确切兼容私有 Server 与本机
   Worker cohort 的有界 activation。
3. Launcher 等待 ready 或 typed unavailable/incompatible result，随后释放 activation resource；
   它绝不继续作为私有进程的生命周期 supervisor。
4. 本机 Server 通过同用户平台本地边界认证 Client，本机 Worker 独立认证为执行节点。
5. Help、version、observation-only 与 hook 路径不会隐式激活不可用 service，除非其确切且已接受的
   contract 另有规定。

### 共享结构 mutation

1. Client 提交一项 Workspace、Tab 或 Panel 语义 operation，并携带 request identity 与
   expected revision。
2. Server 授权并序列化规范 transition。
3. Server 向所有已授权 Client 发布 committed revision。
4. 如果 transition 需要 terminal 或 Agent effect，Server 通过确切 Worker operation
   对其进行 reconciliation，并发布 typed outcome，但不声称具有跨进程原子性。

### 终端创建

1. Client 发送一项 `TerminalCreateIntent`；它可以请求 Worker constraint、cwd semantics 与
   start mode，但不携带权威 operation、lease、generation 或 fence。
2. Server 认证 Client，解析确切 Worker placement，授权 cwd 与 start semantics，并按建模
   顺序提交 terminal 产品意图。通用 Workspace 或 Panel 关联不会推导 `cwd`。
3. 对 split 或 child-terminal 创建，Server 解析 source Terminal 已接受的 `currentCwd`，固定
   source Worker，或遵循所属 feature 文档规定的 absence fallback；绝不在另一 Worker 复用
   node-local path。
4. Server 向确切 Worker 签发一项幂等、受 fencing 约束的 `AuthorizedTerminalLaunch`。
5. Worker 校验 authorization 与 fence，解析省略的 node-local default，记录一项
   `WorkerLaunchAdmission`，并用一项 `HostLaunch` 调用 Terminal Host。
6. Host 按现有 geometry 与 idempotency contract，以原子方式准备或创建 retained terminal，
   并返回 typed result；它不会收到 Project、repository、Checkout、Worktree、Environment、
   Site 或 Worker lease 语义。
7. Worker 报告确切 observation；Server 进行 reconciliation 并发布产品 outcome。
8. 各 Client 按需打开各自独立、经过授权的 attachment。Deferred creation 要求之后经同一
   Worker 提交一项携带完整 geometry 且由 Server 授权的 `StartTerminal`。

如果可选 managed Worktree operation 先于终端创建执行，它对本流程的唯一输入就是作为 intent
`cwd` 使用的节点本地目录。

### 可选 Agent 观察

这条候选 flow 在 Agent 审阅选择其 topology 前不属于已接受设计：

1. 安装 Agent capability 后，显式安装的 provider hook 可以用有边界的 event data 与确切
   execution context 调用选定 helper。
2. Helper 尝试一次本机 Worker delivery，然后 fail-open 退出。
3. Worker 校验事件并把它关联到一项经过授权的本机 execution，再转发有边界的 observation。
4. Server 拥有 Agent semantic reduction、持久 orchestration state 及向 Client 的 publication。
5. Terminal Host 永远不会从 hook data 推导 Agent semantics。

## 多客户端终端语义

一个 retained terminal 可以有多个 attachment-scoped projection。

| 共享终端事实                                         | attachment 或 Client 本地事实                         |
| ---------------------------------------------------- | ----------------------------------------------------- |
| PTY、shell、current process、terminal output 与 exit | Active Tab 与 focus                                   |
| 规范 VT content、mode、title 与 retained history     | Viewport 与 scroll position                           |
| 已接受的有序 terminal input 及其 effect              | Search query、result cursor 与 highlighting           |
| 规范 terminal geometry 与 terminal-generated resize  | Selection presentation 与 platform copy effect        |
| Clear、reset、close 与 process termination outcome   | Projection demand、pause、recovery 与 local animation |

Host attachment capability 决定一个 attachment 是否可以 read、input 或 resize。不同 Client
Session 可以控制不同 terminal。多个 attachment 可以 read 同一个 terminal，但已接受的本机
milestone 对该 terminal 最多只给一个 active controller 授予 input 与 resize lease。其他
attachment 不具有控制权。

显式 takeover、分离 input 与 resize controller，或 opt-in collaborative input 继续保留。任何
策略都不得把多条 input stream 表现成相互独立的 shell。只要 selecting、searching、copying
与 scrolling history 不会变成 application mouse 或其他 PTY input，它们就保持相互独立。

## 协议与传输边界

| 边界                           | 语义目的                                                                                                              | 本机传输候选项                   | 远程传输候选项                               |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------- | -------------------------------- | -------------------------------------------- |
| Client 到 Server               | Human session、snapshot、delta、包含 `TerminalCreateIntent` 的 semantic command、attachment bootstrap 与 remote relay | Unix-domain socket 或 named pipe | HTTPS 加有边界的 binary WebSocket 或同等方案 |
| Server 到 Worker               | Node enrollment、capability version、`AuthorizedTerminalLaunch`、lease、heartbeat、fencing、progress 与 result        | Unix-domain socket 或 named pipe | 经过认证和加密的出站节点连接                 |
| Worker 到 Terminal Host        | `HostLaunch`、确切 terminal lifecycle、input、projection、history、selection 与 Host inventory                        | 现有私有 Host IPC                | 无                                           |
| Agent helper 到 Worker（保留） | 仅在单独接受后交付一条可选、有边界的本机 Agent hook observation                                                       | 候选私有本机 IPC                 | 无                                           |
| 本机原生 Client 到 Worker      | 在获准后使用 Server 授权的高吞吐量 terminal attachment                                                                | 限定范围的私有本机 IPC           | 保留；初始拓扑不需要                         |

`Capability Node Protocol` 指的是 Server-Worker 语义，不是额外进程或固定传输。角色共置时，
它运行在本机 IPC 上；Worker 远程运行时，它运行在网络连接上。

当前 TypeScript protocol package 不能静默变成 Rust contract。获准实施时必须选择一个
规范 schema 与行为来源，通过生成 codec 或独立实现 codec 并提供跨语言 conformance 证据，
避免出现两个可修改的协议权威。

## 身份与 fencing

目标形态至少区分：

- Site、human identity 与 Client Session；
- 稳定 Node identity、Node Session 或 connection generation，以及 device credential；
- 可选 Project 或 managed-Worktree 产品 identity、Workspace presentation、确切 Worker placement
  与 terminal-launch operation；
- Terminal、Host epoch、attachment 与 capability revision；
- Agent task、Agent execution 与 hook correlation；
- semantic request、operation、lease 与 result identity；以及
- transport connection 与 reconnect generation。

transport identity 永远不能替代 product identity。重连会建立新的已认证 transport，
并对稳定 identity 进行 reconciliation。过期 Server generation、Worker lease、Host epoch、
attachment 和 capability revision 都必须 fail closed。结果未知的 terminal input 永远不会自动重放。

## 信任与安全约束

- Human Client credential 与 Worker device credential 保持不同，且不能互换。
- Worker 通常主动连接 Server；Terminal Host 不暴露 LAN 或 Internet listener，任何未来 Agent
  helper 也必须保留这项约束。
- Terminal Host 保持同用户本机信任边界，不获得任何 Site authorization 逻辑。
- Server 授权的本机数据路径使用范围很窄的 attachment capability，而不是把同用户 ambient
  access 当作产品 authorization。
- 每一条不受信任的消息都在首个可信边界做大小限制、解析、认证、授权和身份校验。
- Worker 只接受显式版本化的 capability。launch operation 可以携带一个初始 `cwd` 和一份
  单独授权的 transient environment overlay，但 Worker 不暴露通用目录枚举、文件读写、
  不受限的环境变量注入、command dispatch、raw shell text、arbitrary argument vector 或隐式
  Files/Git API。
- 默认情况下，terminal content、credential、hook payload 和 secret 不得进入 diagnostics 或 audit。
- 同用户进程隔离限制职责与故障耦合，但不能宣称它是抵御恶意多租户的安全 sandbox。

## 连续性与故障模型

| 事件                                    | 必需或候选行为                                                                                                         |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| CLI invocation 退出                     | 只结束自己的 request 或 observation                                                                                    |
| TUI、Web、Desktop 或 mobile Client 退出 | 释放该 Client Session 与 attachment；共享状态和 retained terminal 保留                                                 |
| Client transport 丢失                   | 把 attachment 标记为 disconnected；不做删除，也不重放 terminal input                                                   |
| Server 不可用                           | Worker 与 Host 保留独立 retained execution；确切的 Client 与 Worker 降级行为仍然保留                                   |
| Server 重启                             | Client 与 Worker 重连，并 reconciliation Site revision、node generation、execution、Host inventory 和 attachment state |
| Server-Worker 链路丢失                  | Worker 不接受凭空生成的 authority；retained Host terminal 继续运行；有界 observation 的保留或丢失策略仍然保留          |
| Worker 重启                             | Host 尽可能独立存活；替代 Worker 建立新的 generation，并对确切 Host inventory 进行 reconciliation                      |
| Terminal Host 崩溃                      | Worker 报告 terminal loss；不承诺 PTY recovery                                                                         |
| Node 或操作系统重启                     | 普通 PTY 和进程可能丢失；Site 报告单独的 machine-restart outcome                                                       |
| 选定 Agent helper 不可用或被拒绝        | 若该 capability 已接受，hook fail-open 退出，不生成虚假 observation                                                    |
| 缓慢的远程 Client                       | 暂停或关闭其有界 relay 或 attachment，不阻塞其他 Client 或 terminal                                                    |
| Terminal-input acknowledgement 丢失     | outcome 未知，永不自动重放                                                                                             |
| Version 或 protocol 不匹配              | 拒绝或隔离不兼容连接，不能替换仍然拥有 retained terminal 的 Host                                                       |

## App 职责迁移

上文所述的增量原生底座与本机终端纵向里程碑已经获得授权。本节没有任何一行会单独授权后续
consumer migration、public cutover 或 deletion。

| 当前或候选 app                 | 审阅状态与目标处置                                                                                                                                                                                                                                                                                                                                |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/cli`                     | **已接受：**把 Rust `orchbee` CLI/TUI 构建为新 Client，只迁移显式选择的 semantic capability，原子切换已打包公共入口，然后删除 Node workspace 及其 launch artifact；不要求兼容旧 command 与 parameter                                                                                                                                              |
| `apps/orchbee-server`          | **已交付增量边界：**Rust Server 拥有其内存 kernel，以及彼此分离、有界且 same-user 的本机 Worker 与 Human Client listener、admission registry、generation 和 liveness attachment。Client wire 当前止于独立 Session 准入；下一步只按已接受 terminal vertical 扩展，之后再通过单独接受的 migration 吸收生产 Browser ingress 与 remote terminal relay |
| `apps/server`                  | **已接受替换：**在其受支持行为迁到 `apps/orchbee-server` 前，让已交付 Node Browser-ingress preview 继续保持权威；随后在同一个审计 cohort 中删除 Node app 与过时 package identity                                                                                                                                                                  |
| `apps/orchbee-worker`          | **已交付增量边界：**独立 Rust Worker 拥有有界 same-user Server registration、保留的 Worker generation、经认证且精确版本的 Host Broker connection、保留的 Host epoch 与 limit，以及耦合的 peer-loss cleanup，且不执行 terminal effect；下一步只增加精确 Server-authorized Host launch，remote enrollment 仍是后续工作                              |
| `apps/terminal-host`           | **已交付增量连接边界：**保留独立 Zig app 与 Ghostty 权威；尚未发布的 Worker 现在通过其现有 same-user endpoint 与 handshake 形成常规 Broker connection，且不修改 Zig、不迁移 Desktop/SSH adapter、不发送 terminal command，也不合并进程                                                                                                            |
| `apps/agent-bridge`            | **随 Agent 保留：**在 Agent integration、absence/failure、language 与 cohort 决策获接受前，不创建目标                                                                                                                                                                                                                                             |
| `apps/desktop`                 | **除保留区域外已接受：**保留 Desktop Client shell、renderer bootstrap、packaging、platform effect 与 app-local lifecycle；应用下文非 SSH 模块处置，并继续保留 Agent、Worktree、Archive、Launch Profiles 与 SSH 专属区域                                                                                                                           |
| `apps/web`                     | **已接受：**保留唯一 Browser/Desktop renderer app，并从 `@orchbee/workbench` 吸收 React Workbench shell、browser interaction state、layout persistence 与 presentation 实现                                                                                                                                                                       |
| `apps/gateway`                 | **已接受退役：**Server 拥有 authenticated Browser session、terminal relay 与 backpressure 行为，且每个受支持 Desktop/Web consumer 都迁移后删除                                                                                                                                                                                                    |
| `apps/ssh-environment-runtime` | **已接受退役：**它不是目标 Worker 或 compatibility layer。仅在其仍为现行权威期间保留；一个已接受切片迁移或显式下线每条受支持路径并处理已安装 artifact 后将其删除；后续 unmanaged-SSH 实现是无需 executable compatibility 的新设计                                                                                                                 |
| `apps/storybook`               | **保留：**仅用于开发的视觉证据 host；application-owned Web Story 随 React Workbench 实现一起移动                                                                                                                                                                                                                                                  |

## Desktop Main 目标处置

产品决策人把非 SSH Desktop 边界交由本文收口，并已接受这份目标处置。这里描述的是职责 outcome，
不是机械目录移动：已接受行只有在目标行为与 recovery evidence 存在后才移除当前 Desktop owner。
标记为保留的行仍有意保持未决。

| 当前 `apps/desktop/src/main` 区域                                                                                    | 已接受或保留的目标处置                                                                                                                                                                                                    |
| -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `client-session` 与 `workbench-document`                                                                             | **迁到 Server，再从 Desktop 删除：**Server 拥有已鉴权 Client session、canonical Workbench state、commit、snapshot/delta publication、persistence 与 reconciliation                                                        |
| `terminal-document-control` 与 `remote-terminal-creation` 中的共享文档部分                                           | **迁到 Server，再从 Desktop 删除：**Client 提交 terminal intent；Server 拥有 canonical mutation 与已授权 Worker placement。Desktop 不保留 terminal-create authority                                                       |
| `semantic-command` coordinator、execution ledger、publication 与 native local endpoint                               | **迁到 Server 与 Rust `orchbee`，再从 Desktop 删除：**Server 拥有 admission/commit/observation；公共 Client 拥有 invocation。存续的 Desktop platform effect 只能通过狭窄 Desktop capability adapter 暴露                  |
| `host`、`terminal-binding-registry`，以及 `terminal-lifetime` 与 `terminal-output-read` 中面向 Host 的部分           | **迁到 Worker，再从 Desktop 删除：**Worker 拥有 Host discovery、brokering、binding、lifecycle effect dispatch、output-read routing 与 inventory reconciliation；Terminal Host 仍是 terminal authority                     |
| `terminal-projection-control` 与 `broker` 中面向 Host 的行为                                                         | **拆分，再缩减 Desktop：**Server 授权 attachment，Worker 路由 Host data。Desktop 只能保留 sandboxed renderer 所需的有界 Electron transport handoff；它不拥有 attachment policy、terminal state 或 node routing            |
| `browser-access` service lifecycle、authentication material 与 relay composition，以及全部 `gateway`                 | **把 ingress/session/relay ownership 迁到 Server 并删除旧 composition：**Desktop 可以保留显式 Client action，用于请求访问、打开已批准 URL 或复制它，但不监督 Gateway、不签发 Browser session，也不 relay terminal         |
| `environment` 中的通用产品 placement 行为                                                                            | **从目标核心删除：**原生 terminal placement 直接解析到 Worker。当前 SSH 路径仍需要的 registry code 只保留到保留的 SSH 专属处置完成                                                                                        |
| `project-folder-acquisition`                                                                                         | **只保留 Desktop platform adapter：**可以展示 native picker，并把用户选择的本地 path 返回给已授权 Client workflow；它不创建 canonical Project state，也不声称 client-only 机器就是所选 Worker                             |
| `terminal-default-settings`                                                                                          | **拆分：**把 node-local default interactive-shell resolution 迁到 Worker。任何产品 preference 或 named-profile 行为与 `terminal-launch-profiles` 一起保留                                                                 |
| `application-lifetime`                                                                                               | **拆分并只保留 Desktop lifetime：**window/app quit 与 renderer teardown 留在 Desktop；Site shutdown、共享 terminal intent、Worker drain 与 Host retention 分别属于 Server、Worker 与 Host contract                        |
| `desktop-clipboard`、`window-actions`、`window-state`、`application-menu` 与 `secure-keyboard-entry`                 | **留在 Desktop：**确切 Client-local 或 operating-system effect                                                                                                                                                            |
| `desktop-renderer-protocol`、preload bootstrap 与其余 `broker` adapter                                               | **缩减并保留：**只暴露最小 sandboxed renderer bootstrap 与确切 Desktop platform capability；所属切片迁移后，删除 Server、Worker、terminal-authority、Gateway、Agent-owner 与旧 Environment message                        |
| `composition` 与 `recovery`                                                                                          | **围绕存续 Desktop 职责重写：**只保留 Electron Client/platform composition 与 Client-local recovery；Server 拥有 Site recovery，Worker 拥有 node/Host reconciliation，任何替代 composition 都不能重新创建已移除 authority |
| `agent-observation` 中的共享 Agent state 与 `agent-integration-management`                                           | **随 `@orchbee/agent` 保留：**本轮不决定 package、Server、Worker、Bridge 或 Desktop presentation ownership                                                                                                                |
| `project-worktree`                                                                                                   | **保留：**它仍是 terminal 基线之外的可选能力；本轮不决定 retention、owner、Git authorization、journal、cleanup 或 deletion                                                                                                |
| `terminal-archive`                                                                                                   | **保留：**不能只因 live-terminal routing 移动就迁移或删除                                                                                                                                                                 |
| `terminal-launch-profiles` 与 `terminal-default-settings` 的非基线部分                                               | **保留：**本轮不决定 named profile、raw command、argument vector、preference scope 或 compatibility                                                                                                                       |
| `ssh-environment`、`ssh-project-enrollment`、`remote-project`、`remote-terminal` 的 SSH 部分及其余 SSH registry 行为 | **保留：**当前 SSH Runtime/protocol 的退役已经接受，但确切 Desktop 用户体验迁移或删除清单留给单独审阅                                                                                                                     |

每个已接受的迁移切片都必须同时更新来源与目标的 `project.json`。只有当一个模块的 state、
behavior、recovery、test 与 documentation 都有一个经过验证的存续 owner 后，才能删除该模块。

## Package 与 workspace 缩减审计

任何 workspace 都不会仅因为名字出现在本表中而被删除。每次删除都需要 consumer scan、
public-export decision、migration/compatibility policy、replacement 或显式 product-sunset
evidence，并在同一个已接受的切片中删除过时 documentation 与 test。

| Workspace 或 package                                                             | 候选审阅结果                                                                                                                                                                                                                                                             |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `@orchbee/gateway-protocol` 与 `apps/gateway`                                    | **已接受联动退役：**Server 提供唯一生产 Browser/session/relay contract，且每个受支持 Desktop/Web consumer 都迁移后删除；旧 framing 与进程拓扑都不是兼容目标                                                                                                              |
| `@orchbee/desktop-client-protocol`                                               | **已接受缩减：**只保留 sandboxed Desktop bootstrap 与确切 platform-local capability；迁移后删除 Server、Worker、terminal-authority 与 Gateway message。Agent 与 SSH 专属 message 服从各自保留审阅，不会在这里获得永久 ownership                                          |
| `@orchbee/remote-client-protocol`                                                | **已接受保留：**保留 human Client 边界；只能通过已接受的生产 session 与 relay 切片扩展，绝不能混入 Worker device 语义                                                                                                                                                    |
| `@orchbee/semantic-command-protocol`                                             | **已接受拆解：**只要仍有 consumer，就只把刻意选择的 semantic operation 保留为 framework-neutral contract/TS binding；cutover 时替换或退役当前 CLI-specific HTTP/locator transport，不兼容旧 syntax，Rust 使用已接受的跨语言 schema authority                             |
| `@orchbee/workbench-client-protocol`                                             | **已接受有界保留：**只要仍有真实 TS consumer，就仅作为 transport-neutral Client session、semantic-intent、snapshot 与 delta 边界的 TypeScript binding 保留；canonical 跨语言 schema 与 framework 无关，Rust/mobile binding 不依赖该 package                              |
| `@orchbee/terminal-protocol`                                                     | **已接受保留：**保留为 Host 边界并添加 Rust conformance，但不能创建第二个 protocol authority                                                                                                                                                                             |
| `@orchbee/ssh-environment-protocol` 与 `apps/ssh-environment-runtime`            | 已接受且必须联动退役的候选项；其源码、wire、service、名称与 release cohort 都不是目标 compatibility contract。只有在当前 consumer、受支持路径的迁移或下线、installed-artifact 处理与 rollback 都获接受并验证后，才一起删除                                               |
| `@orchbee/agent`                                                                 | **保留：**本轮不作 package 或 ownership 处置                                                                                                                                                                                                                             |
| `@orchbee/workbench`                                                             | **已接受拆解并删除：**把 React shell、browser-local interaction state、layout persistence、style 与 Story 移入 `apps/web`；把共享词汇及 snapshot/delta contract 抽到 framework-neutral 跨语言权威；把 canonical transition 迁到 Server；然后删除这个混合 browser package |
| `@orchbee/actions`                                                               | **已接受保留：**只要 Web renderer 仍在消费，就保留与框架无关的 action discovery 与 Client-side execution lifecycle；不得把它变成第二个 Server command authority                                                                                                          |
| `@orchbee/foundation`                                                            | **已接受保留并清理：**保留仍有跨 workspace consumer、依赖轻量且 domain-neutral 的 primitive；每次 owner 迁移后审计孤立 export，不能把 backend behavior 移入其中                                                                                                          |
| `@orchbee/keyboard-shortcuts`                                                    | **已接受保留：**当 Web 或 Desktop 仍在消费时，为 TypeScript Client 保留 shortcut normalization；Rust TUI 拥有自己的 platform input adapter，只能通过已接受的 contract 共享 identifier                                                                                    |
| `@orchbee/terminal`                                                              | **已接受保留：**保留 Browser/Desktop terminal renderer；Rust TUI 通过自己的 renderer 消费 Terminal Protocol，并不会让这个 browser package 过时                                                                                                                           |
| `@orchbee/ui`、`@orchbee/tab-strip`、`@orchbee/i18n` 与 `@orchbee/design-tokens` | **已接受保留：**只要 Web/Desktop renderer 仍在消费就保留；仅后端迁移并不是删除证据                                                                                                                                                                                       |
| 当前 Desktop Main 模块                                                           | 应用上文已接受的非 SSH 目标处置表；Agent、Worktree、Archive、Launch Profiles 与 SSH 专属行继续保留；没有已发布 compatibility 需求时不保留 compatibility wrapper                                                                                                          |

首要缩减目标是重复所有权，而不是 package 数量。一个 owner 明确且有多个真实 consumer 的小
package 可以保留；一个已无存续 semantic owner 的大型 compatibility package 不能仅为了避免迁移而保留。

## Rust 仓库结构决策

Phase 1 会在创建任何 Rust product runtime 前解决仓库缺口：

- 仓库根拥有唯一 virtual Cargo workspace 和一个精确的 `rust-toolchain.toml`；
- 第一个真实 member 创建唯一且已跟踪的根 `Cargo.lock`，必需 Cargo 工作使用 `--locked`；
- Rust executable crate 直接放在 `apps/*`；具备真实 consumer 的可复用 Rust library 直接放在
  `crates/*`；`packages/*` 继续作为 JavaScript 与 TypeScript library root；
- Cargo application 直接参与根门禁，不添加 `package.json`、占位 TypeScript 或重复的应用级
  Cargo orchestration；
- `project.json.runtime` 继续使用 `native`，因为它描述执行形态；`Cargo.toml` 才是 Rust 身份权威；
- [`docs/RUST.md`](../../docs/zh-CN/RUST.md) 负责 toolchain、dependency、module、safety、
  concurrency、testing、release input 与跨语言 conformance 规则；以及
- application crate 可以依赖已声明可复用 crate，但绝不能依赖另一个应用的 source 或 Cargo target。

每个 serialized 跨语言切片在实施前仍必须指定确切 language-neutral schema 或 encoding authority
与 canonical fixture。Phase 1 建立了这条规则，但没有选择 wire schema；下文另行接受的第一个
Phase 2 切片只选择只读 Workbench version 1 projection，仍未选择任何 Terminal wire schema。
这项底座不会创建 product process，也不会改变当前 runtime behavior。

## 已交付的原生底座

除非所属指南另有说明，下列底座均为增量、尚未发布且没有生产 consumer。在一个已接受的
migration 切换完整 consumer 路径前，当前 TypeScript、Electron、Node 与 Zig runtime 行为
继续保持权威。

| 底座                            | 已交付边界与当前 owner                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust 仓库契约                   | 根 Cargo workspace、固定 toolchain、Rust 标准、project-contract 支持与根 verification 已成为现行仓库基础设施，由 [`docs/RUST.md`](../../docs/zh-CN/RUST.md) 与根 build 文件拥有。                                                                                                                                                                                                                                                                                                                                                                                         |
| Workbench projection contract   | JSON Schema version 1 以及独立 TypeScript/Rust binding 提供有界只读 snapshot 与原子 delta。Schema、package guide、crate contract 与 conformance corpus 拥有维护事实；当前 writer 或 consumer 均未迁移。                                                                                                                                                                                                                                                                                                                                                                   |
| 原生 `orchbee` 入口             | [`apps/orchbee`](../../apps/orchbee/README.zh-CN.md) 拥有尚未发布的 empty、`--help`、`--version` 底座及其启动参考测量。裸调用仍报告 TUI 不可用。                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Rust Server kernel 与 service   | [`apps/orchbee-server`](../../apps/orchbee-server/README.zh-CN.md) 拥有有界 Workbench read、failure-atomic 的内存 projection publication selection，以及彼此分离、尚未发布、私有且 same-user 的本机 Worker 和 Human Client service。Worker service 准入一个 compatible Worker；Client service 通过有界 candidate handling 准入最多八个相互独立的 compatible Client Session。两者都拥有非零 generation-scoped liveness 与 cleanup。Client wire 不承载准入后的产品或 terminal command，Server 也没有 terminal、activation、persistence、packaging 或 production lifecycle。 |
| Terminal Client authority model | Server 所属的 [`LocalTerminalClientAuthority`](../../apps/orchbee-server/formal/tla/LocalTerminalClientAuthority.tla) refinement model 检查同级 Client Session、多个 viewer、每个 terminal 一个合并的 input-and-resize controller、generation-scoped cleanup，以及独立排序的 Client、Server、Worker、Host 和 terminal loss。它记录已接受的目标 authority，但不增加 runtime message，也不实现 attach、control、takeover 或自动 controller promotion。                                                                                                                      |
| Rust Worker process spine       | [`apps/orchbee-worker`](../../apps/orchbee-worker/README.zh-CN.md)、[`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.zh-CN.md) 与 [`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.zh-CN.md) 拥有尚未发布的 Server registration 加常规 Worker-to-Host Broker connection、有界私有 framing、两条 link 上的 POSIX peer proof、精确 Host handshake、保留的 admission identity 与 limit，以及耦合的 connection liveness。它们不执行 terminal effect，也没有 production consumer。                                         |
| Rust Terminal Host binding      | [`crates/terminal-protocol/project.json`](../../crates/terminal-protocol/project.json) 与 package 所属的 [Terminal Protocol 参考](../../packages/terminal-protocol/references/protocol.zh-CN.md) 拥有已经完成到 kind 70 至 73 terminal-configuration 的独立 Rust envelope 与 payload binding。Worker 只为尚未发布的 Host handshake 消费 envelope 与 connection-control 子集；只有具体纵向 consumer 才能准入更多 message codec。                                                                                                                                           |

本文不再维护逐切片历史 authorization 与命令结果。当前行为属于上述 owner 与可执行证据；
本提案只保留尚未解决的目标设计和下一个已接受 milestone。

## 已接受的本机终端纵向里程碑

产品决策人于 2026-09-02 接受了这个 consequential milestone，并在接受下列 observable
behavior、authority boundary、排除项、证据与 rollback 后，委托其依赖有序的实现切片。
只有发现实质不同的产品 outcome、owner、public contract、risk 或 exclusion 时，切片才必须
返回评审。

| 字段            | 已接受决策                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题            | 原生入口、Server kernel 与 Rust Host binding 各自经过验证，却没有形成可运行产品路径。继续逐 message 追求 parity 会增加未使用 protocol surface，同时仍不能验证单命令体验、Worker 边界、进程生命周期、真实终端渲染与 IPC 性能。                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Outcome         | 在一台受支持本机上，裸 `orchbee` 发现或懒激活确切兼容的私有 Server 与本机 Worker cohort，进入真实 TUI Client Session，并能创建或附加一个默认交互式 Shell terminal；其真实 output、input 与 resize 经过保留的 Zig Terminal Host。关闭任一 Client 不会隐式终止 Site、私有 service 或 retained terminal。后续调用以另一个独立 Client Session 加入同一个 Site。                                                                                                                                                                                                                                                                                                                           |
| 范围            | 交付私有 Rust Server service boundary、一个私有 Rust 本机 Worker、现有 Zig Host discovery 或 activation、有界同用户本机 IPC、带可选 node-local `cwd` 的最小默认 Shell terminal intent、Host launch mapping、create/attach/output/input/resize flow、真实单 terminal TUI surface、有界 relay queue、version-cohort rejection，以及该路径需要的 process/fault/performance 证据。                                                                                                                                                                                                                                                                                                        |
| Client 模型     | TUI、Web、Desktop、mobile、不同 browser tab、不同 Desktop window 与重复 `orchbee` invocation 都属于同一个 Client 模型中的同级对象。每个活跃实例拥有自己的 Client Session，以及本地 focus、active location、viewport、scroll 与 presentation state。共享 Workspace、Tab、Panel 与 Terminal fact 来自 Server。Terminal Host 只看 typed attachment 与 capability，绝不识别 renderer 类型。                                                                                                                                                                                                                                                                                               |
| Terminal 控制   | 不同 Client 可以控制不同 terminal。多个 Client Session 可以通过不同 attachment 查看同一个 terminal，但本 milestone 对该 terminal 只准入一个活跃 input-and-resize controller。其他 attachment 不具有控制权；显式 takeover 与 collaborative input 仍是后续决策。                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 激活与生命周期  | 裸 `orchbee` 可以发起确切 installed private cohort 的有界懒激活并等待 ready，但 ready 后不再作为 Server 或 Worker 的生命周期 supervisor。`--help` 与 `--version` 保持无副作用。Client 退出只释放其 Client Session 与 attachment。确切 daemonization 或平台 service 机制属于委托的实现选择，但必须保留 singleton、version、cleanup 与 failure claim。                                                                                                                                                                                                                                                                                                                                  |
| Launch 边界     | Client intent 绝不提供权威 Worker placement、lease、generation、fence、executable、arbitrary argument、Environment、Project、Checkout、Worktree 或 Agent identity。Server 选择唯一的本机 Worker；Worker 解析已接受的默认交互式 Shell 与省略的本机 default；Host 只接收其确切 launch fact。第一个 milestone 发送空的 transient environment overlay。                                                                                                                                                                                                                                                                                                                                   |
| 数据路径        | 第一条完整路径是 `Terminal Host <-> Worker <-> Server <-> Client`。Worker 与 Server 校验 routing、identity、role、size、queue 与 authorization fact，但不模拟 VT 或重新解释 frame content。Client 直连本机 Worker 不属于本 milestone；它需要实测需求以及另行接受的 scoped-capability 设计。                                                                                                                                                                                                                                                                                                                                                                                           |
| 信任            | 本 milestone 只暴露同一操作系统用户可访问的私有本机 endpoint。它不增加 LAN/Internet listener、Cookie 到 device credential 的转换、ambient direct Host access 或 multi-user claim。错误 peer role、过时 identity、畸形数据与不兼容 version 均失败关闭。                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Claims          | 一个本机 Site 最多拥有一个活跃 Server authority 与一个准入的本机 Worker identity。Help/version 不启动 service。Mutating intent 最多提交一次，unknown terminal input 绝不 replay。慢 Client 不能阻塞 Worker progress 或另一 Client。Client disconnect 不意味着 terminal termination。Server 绝不成为 VT/history authority，Worker 绝不成为 generic command/filesystem service，baseline terminal 不需要 Project、repository、Worktree、Environment 或 Agent state。                                                                                                                                                                                                                    |
| 非目标          | 不准入完整 Workbench UI、额外 public CLI grammar、selection/export/history/theme/clear/reset feature parity、direct Worker data path、最终 collaborative-input 或 takeover UX、Agent helper、managed Worktree、Archive、Launch Profile、SSH redesign、remote Worker、Web/mobile migration、Gateway/SSH/Workbench/旧 CLI 删除、production TLS、multi-user tenancy、durable Server recovery、操作系统重启后的 PTY recovery、login-time service installation、package-manager release、public cutover 或 compatibility promise。                                                                                                                                                         |
| 证据            | 在现有 topology model 不足时，对 singleton activation、独立有序的 Client/Server/Worker/Host loss、controller ownership、有界 queue 与 unknown input outcome 建模。增加 absent/present/incompatible cohort、两个同时 Client Session、Client 退出而 terminal 保留、畸形 peer、Host loss 与有界 shutdown 的 process test。增加 create、attach、frame、input、resize、detach、reattach 与 exit 的端到端真实 PTY trace。在提出 direct path 或数值 release budget 前，测量 cold/warm activation、input latency、frame throughput、copy、queue memory 与 slow-client isolation。仍需通过适用 focused check 与根 `pnpm verify`；除非明确授权，否则继续排除会抢焦点的手动 native-window 测试。 |
| 交付与 rollback | 首先收敛三份所属提案，然后依次交付无 terminal effect 的 process spine、最小 launch/Host broker、经 Server relay 的 terminal stream、TUI adapter，最后完成 loss/performance 收敛。每个切片保持增量和尚未发布，不含 durable migration，并让当前 Desktop 行为继续保持权威。Rollback 删除新的未使用切片或回到上一个已验证的增量 spine；绝不删除或切换现有 consumer。                                                                                                                                                                                                                                                                                                                      |

## 交付与迁移顺序

| Milestone               | 可独立审阅的 outcome                                                                                                                                                                                                                                                                                                                                                                                               |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 原生底座                | **已经完成且尚未发布：**仓库/Cargo 支持、只读 Workbench projection contract、原生入口底座、内存 Server projection kernel 与独立 Rust Terminal Host binding。所属指南与可执行测试维护其当前事实。                                                                                                                                                                                                                   |
| 本机终端纵向链路        | **正在推进且尚未发布：**真实本机 Server-Worker 与 Client Session spine、已检查的 terminal Client authority model，以及 Worker-Host Broker connection、handshake 与 coupled-loss boundary 已经完成；下一步交付精确 Server-authorized Host launch、经 Server relay 的 terminal stream、真实 TUI adapter 与剩余 fault/performance 证据。Protocol 工作继续由这些 consumer 的需求驱动，不按 message registry 顺序推进。 |
| 共享 Web/Desktop Client | 本机权威路径建立后，把 React Workbench presentation 移入 `apps/web`，把共享 semantic transition 迁给 Server，保留 Desktop-only platform effect；只有完整职责审计通过后才删除 `@orchbee/workbench`。                                                                                                                                                                                                                |
| Browser 入口退役        | 把受支持 Browser session、terminal relay 与 backpressure 行为移入 Server，迁移 Web/Desktop consumer，然后把 `apps/gateway`、`@orchbee/gateway-protocol` 与被取代 Desktop composition 作为一个可回滚 cohort 删除。                                                                                                                                                                                                  |
| 可选 capability         | Agent、managed Worktree、Archive、Launch Profiles、SSH 专属 Desktop 行为与任何未来 unmanaged-SSH 设计由各自 owner 单独审阅；terminal baseline 不需要它们。                                                                                                                                                                                                                                                         |
| 分布式运行              | 每次接受一个 capability，逐步增加 enrolled remote Worker、mobile 或 remote Client、production authentication、recovery、compatibility 与优化后的 direct attachment。                                                                                                                                                                                                                                               |
| 旧实现退役              | 只有各自接受的替代或下线切片证明 consumer、artifact、migration 与 rollback coverage 后，才删除当前 SSH Runtime/protocol、旧 CLI、Gateway、Workbench package 与缩减后的 Desktop authority。                                                                                                                                                                                                                         |

Rollback 保留唯一 canonical writer。每个本机终端切片都是增量且尚未发布，因此 rollback
只删除未使用 artifact 或回到上一个已验证 process spine。后续 consumer 或 durable-schema
migration 需要各自的 cohort rollback，绝不能让旧 writer 读取未知新 state。仍拥有 live
terminal 的兼容 retained Host 不会被替换。

## 正确性主张与证据计划

| 主张                                                                                    | 完成前所需证据                                                                                                                                     |
| --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| 即使发生共置与重连，一个 mutable fact 仍只有一个 authority                              | Ownership assertion、project contract、deterministic transition test 与跨进程 topology model                                                       |
| 一条共享 semantic command 最多 commit 一次，并从 canonical state 发布                   | Server state-machine test、property test、persistence fault injection 与建模后的 commit/publication ordering                                       |
| Placement 与 fencing 永远不会把 operation 或 attachment 路由到错误 Worker 或 Host       | Identity/epoch property test、TLA+ lease 与 reconnect model、malformed-message test 与 multi-node deterministic trace                              |
| Client、Server 或 Worker 断开不会隐含 terminal termination                              | 覆盖每种独立进程丢失的 Host retention test 与 system trace                                                                                         |
| admission 未知的 input 永不重放                                                         | Protocol test、disconnect fault injection 与 end-to-end input trace                                                                                |
| 选择本机 data path 时，其 capability 不会授予超出 Server 授权的权限                     | Capability decision test、expiry/revocation race、peer-identity integration test 与 security review                                                |
| 缓慢 Client 不会阻塞另一 Client、Worker heartbeat 或 terminal progress                  | Bounded queue test、adversarial backpressure trace 与实测 relay limit                                                                              |
| Rust、TypeScript 与 Zig implementation 在每个共享 wire contract 上一致                  | Canonical fixture、适用时的 generated-file stale check、跨语言 conformance 与 differential test                                                    |
| 在有显式 replacement decision 前，migration 保留已接受的当前行为                        | Old/new differential scenario、durable-data migration/rollback test 与 feature-specific acceptance evidence                                        |
| 创建终端不需要 Project、repository、Checkout、Worktree、Environment 或 Agent capability | Terminal-only cohort test，覆盖省略 `cwd`、普通非 Git 目录、repository 目录与可选上游生成 path                                                     |
| Client 不能提供有权威性的 Worker lease、generation 或 fence                             | Schema rejection、Server binding test、stale-fence property test 与 reconnect trace                                                                |
| Split 与 child terminal 保留已接受的 current CWD，且不跨 Worker                         | Same-Worker inheritance test、absent-observation fallback test，以及 two-Worker rejection 或单独接受的 translation trace                           |
| Transient environment overlay 永远不会成为 durable intent 或可重放 recovery data        | Persistence redaction test、Host digest replay test、capability-expiry test 与 lost-delivery trace                                                 |
| Immediate 与 deferred creation 保留 Host geometry/start contract                        | Complete-geometry、awaiting-start、exact-replay、conflicting-start 与 no-fallback test                                                             |
| Worker 不暴露隐式通用 Files、Git、filesystem、profile 或 remote-command authority       | Capability allowlist test、project non-goal、protocol schema review 与 negative integration test                                                   |
| 退役当前 SSH Runtime 与协议既不会遗留受支持路径，也不会信任陈旧 artifact                | 当前 consumer 与 packaged-artifact inventory、迁移或下线证据、uninstall/ignore policy、cutover test 与 rollback exercise                           |
| 每个 Client 获得相同的已授权 Workbench 语义，但不共享 renderer                          | 跨语言 snapshot/delta fixture、Server projection test、Web/TUI/mobile conformance、CLI structured-output test 与 client-local-state exclusion test |
| 删除 Gateway 仍保留 Browser admission、relay isolation 与 backpressure                  | 旧/新 Browser session 与 terminal-relay differential test、slow-client test、consumer scan、packaged-process inventory 与 rollback exercise        |
| Desktop 迁移后不保留第二个 Server、Worker 或 terminal authority                         | Module dependency scan、process-loss trace、authority-negative test、project-contract update 与被取代 composition root 删除                        |

process/session/lease/reconnect model 选择 TLA+，因为 Client、Server、Worker、Host、transport
与 persistence 中相互独立排序的 event 可能破坏 authority、continuity 或 at-most-once 主张。
Property test 覆盖 identifier、stale generation、topology cardinality 和 serializable
transition。Deterministic system test 至少覆盖一个全本机 Site，以及一个具备两个 Worker
和三种并发 Client 形态的 Site。性能证据必须测量 input latency、frame copy、relay memory、
slow-client isolation 与 startup cost；仅统计 process hop 不是性能证据。在接受 Rust CLI
交付切片前，其证据计划必须明确被测 command 与完成点、冷启动与热启动条件、artifact-size
范围、峰值内存指标、参考平台、baseline 与 acceptance-budget owner。具体数值可以在有边界的
prototype 后确定，但 measurement contract 不能推迟到实施以后。

## 产品审阅已接受的决策

1. Terminal launch 独立于 Project、repository、Checkout、Worktree 与通用 Environment
   identity。Future native placement 直接解析到一个 Server 授权的 Worker；当前 SSH
   Environment 行为在接受 migration 前保持不变。
2. 一个公共 Rust `orchbee` executable 同时拥有 CLI 与 TUI Client 界面。当前 TypeScript
   `apps/cli` 在 cutover 后被替换并删除；其源码、command、parameter、output、exit meaning
   与 Desktop-specific launcher contract 都不是兼容目标。
3. Terminal-only cohort 不要求 Agent Bridge、Agent orchestration 或 managed Worktree。
4. 新 launch boundary 保留当前 Host geometry/start rule、non-persistent overlay semantics 与
   same-Worker split/child `currentCwd` inheritance，除非后续显式产品决策替换可观察行为。
5. 当前 `apps/ssh-environment-runtime` 与 `@orchbee/ssh-environment-protocol` 不属于目标架构，
   并在已接受的迁移或产品下线后联动删除。未来 unmanaged-SSH capability 是新设计，不承担源码、
   executable、package 名、wire、service、release cohort 或 Desktop composition 兼容义务。
6. `apps/gateway` 与 `@orchbee/gateway-protocol` 在 Server 接管受支持 Browser session、terminal
   relay 与 backpressure 行为且 consumer 迁移后联动删除。其进程拆分与 framing 不是兼容目标。
7. 当前混合的 `@orchbee/workbench` browser package 被拆解并删除。React UI、browser-local
   interaction state、layout persistence、style 与 Story 移入 `apps/web`；Server 拥有 canonical
   Workbench transition；framework-neutral semantic-intent、snapshot 与 delta contract 使用跨语言
   权威。TUI 与 mobile 拥有各自 renderer，CLI 可以返回有界且版本化的 semantic projection
   snapshot，而不是 Web UI state。
8. 非 SSH Desktop Main 目标处置表已经接受。Agent、managed Worktree、Terminal Archive、Launch
   Profiles 与 SSH 专属 Desktop 区域继续保留，不会被相邻迁移顺带决定。
9. 目标 baseline 使用四个独立 executable：公共 Rust `orchbee`、私有 Rust `orchbee-server`、
   私有 Rust `orchbee-worker` 与私有 Zig `orchbee-terminal-host`。共置不会合并它们的 role 或
   lifecycle。尚未接受任何 Agent helper executable。
10. Phase 1 Rust 仓库决策已经接受：一个固定版本的根 Cargo workspace、`apps/*` 下的 executable
    crate、`crates/*` 下的可复用 Rust library、直接接入根门禁、以 `native` 表示 runtime，并以
    `Cargo.toml` 作为实现语言权威。
11. 第一个增量 Phase 2 切片使用 `schemas/workbench/v1` 下严格的 JSON Schema 2020-12 作为语言
    中立权威。它只准入只读 snapshot intent、完整有界 projection snapshot、原子且 revision-chained
    的 aggregate delta，以及消费同一 canonical conformance corpus 的独立 TypeScript/Rust binding。
    它不改变任何当前 writer、consumer、transport、persistence 或 runtime behavior。
12. 增量 Phase 3A 切片创建尚未发布的 Rust `apps/orchbee` application，并且只准入确切 empty、
    `--help` 与 `--version` dispatch。Empty dispatch 报告显式 development-unavailable TUI state；
    authored help 与 diagnostic 同时提供 `en-US` 和 `zh-CN`；不准入 Server、Worker、Host、TUI、
    业务命令、packaging、当前 CLI migration 或 deletion。它的 release-profile measurement 是
    reference baseline，而非 acceptance budget。
13. TUI、Web、Desktop、mobile、browser tab、Desktop window 与重复 native invocation 使用同一套
    Client/Client Session 模型。Renderer 形式绝不改变 authority；共享产品 state 来自 Server，
    active location、focus、viewport、scroll 与 presentation 保持为各 Client Session 本地状态。
14. 第一条本机 terminal 路径经 Server relay。裸 `orchbee` 可以懒激活确切私有 Server 与本机
    Worker cohort，但不监督其生命周期；help 与 version 保持无副作用。Client 退出不会停止 Site
    或 retained terminal。
15. 在已接受的本机 milestone 中，多个 attachment 可以查看一个 terminal，但最多只有一个 active
    controller 拥有 input 与 resize authority。Direct Worker attachment、takeover 与 collaborative
    input 需要后续接受。
16. 已完成底座之后的 Rust Terminal Host protocol 工作由 consumer 驱动。下一个纵向 runtime
    切片不需要的 message codec 不会独立推进。

## 留待审阅的决策

1. 决定 Agent integration 是否需要 hook helper；若需要，再决定其 executable、language、
   lifetime、installation cohort、failure contract、Worker 关系与 association 模型。
2. 定义已接受裸 TUI、`--help` 与 `--version` 之外的公共 command：grammar、stable output
   与 exit contract、automation、completion、compatibility 与 migration。
3. 决定 managed Worktree、Archive、Launch Profiles 与所有 SSH 专属 Desktop 区域的 owner
   和行为。Terminal baseline 不隐含其中任何一项。
4. 只有 relay 测量证明有必要，且 scoped capability、expiry、revocation、reconnect、
   accounting 与 fallback 获得接受后，才准入本机 Client 到 Worker 的 direct data path。
5. 定义已接受 single-controller milestone policy 之外的显式 controller takeover、分离
   input/resize ownership 或 collaborative input。
6. 独立于即将退役的实现，定义任何未来 unmanaged-SSH capability 的产品范围、时机、
   transport、authentication、installation 与 removal model。
7. 在已接受本机懒激活和 Client-independent lifetime 之外，定义 production service
   installation、login-time start、update cohort、compatibility、durable Server recovery、
   backup、rollback 与 uninstall。
8. 在本机终端 milestone 之外，定义后续 Workbench schema、更广 mutating intent、
   persistence 与 migration、authorization 与 redaction，以及其余 projection domain。
9. 独立于 Desktop 的 Client-only authority，决定其长期 shell 技术。
10. 定义 remote Worker enrollment、production human/device session、mobile backgrounding、
    TLS、discovery、Internet reachability、multi-user tenancy 与 compatibility。
11. 在已接受 milestone 产生测量后，设置受支持平台的 startup、artifact-size、latency、
    throughput、queue-memory 与 peak-memory budget；在 release 时定义 package-manager
    distribution、public cutover 与确切 `apps/cli` deletion。
12. 只有 Gateway、SSH Runtime/protocol、Workbench package、旧 CLI 与 Desktop authority
    各自通过 consumer、installed-artifact、behavior、migration 或 sunset、documentation、
    evidence 与 rollback 审计后，才接受对应 deletion。

## 审阅清单

- 每个 product、node、terminal 和 Client-local fact 是否都有一个令人信服的 owner？
- 一个 Server 能否在不引入本机专用业务路径的前提下协调本机 Worker 与远程 Worker？
- Server、Worker、Desktop 与 TUI 能否独立重启，且不会捏造 terminal continuity？
- Worker 是否仍足够狭窄，不会成为第二个通用 backend？
- Client intent、Server authorization、Worker admission 与 Host launch fact 是否保持分离？
- terminal 能否在省略 `cwd` 或普通非 Git 目录时启动，且不依赖 Project、Checkout 或
  Worktree、Environment 或 Agent state？
- Split 与 child terminal 是否只在 source Worker 上继承一份已接受的 `currentCwd`？
- Transient environment overlay 是否不会进入 canonical intent、persistence、diagnostic 与 replay？
- managed Worktree 是否始终只是可选的路径生产者，而不是 Terminal 依赖？
- 第一条经 Server relay 的 terminal path 是否满足实测 latency、throughput、queue-memory 与
  slow-client-isolation 需求；任何后续 direct path 是否由证据证明有必要？
- mobile 与 remote Client 能否使用 Server relay，同时不让 Server 成为 VT 或 history owner？
- 当前 Desktop 哪些职责真正属于 platform-local？
- 已接受的 Desktop 表是否让每个非保留 Main module 都恰好具有一个目标 owner 或 deletion outcome？
- 迁移后哪些 workspace 仍有长期 consumer？允许每项候选 deletion 的确切证据是什么？
- Rust 能否成为权威而不保留相互竞争的 TypeScript domain transition？
- 新 `orchbee` 能否替换并删除 `apps/cli`，同时不意外保留旧 grammar，也不丢失刻意选择的
  semantic capability？
- 当前 SSH Runtime 与协议能否作为一条完整旧链路删除，同时不暗示它们的实现契约或通用
  Environment 领域继续存在？
- Web、TUI、mobile 与 CLI 能否消费一套 semantic Workbench projection contract，而不共享 React、
  DOM、pixel 或 Client-local focus state？
- migration、compatibility、rollback 与 evidence obligation 是否足够小，可以切成可独立审阅的交付？
