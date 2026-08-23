# Desktop 多客户端会话协调器

## 问题和范围

一个 Orchbee 应用程序运行时可以由多个独立页面观察和控制。 Electron 渲染器页面和浏览器页面具有相同的工作台客户端含义：每个页面都拥有私有 Client View 和终端视口状态，而每个页面观察一个可恢复的共享工作台文档和一组终端进程。 Electron Main、预加载、本地浏览器网关和 Terminal Host 是适配器或权限，而不是客户端。

Desktop Main 拥有一个 `DesktopClientSessionCoordinator`。它串行处理共享文档提案，持久保存一条规范修订链，为整个应用只协调一次持久终端意图，并向每个活动的 `ClientSession` 发布有序的文档与终端观察事件。该基础层提供 Electron `MessagePort` 适配器和不依赖传输方式的协调器核心。它不会打开网络监听器、认证远程对等端、持久保存 Client View、合并并发语义 Action、增加应用包依赖，也不会向普通客户端开放应用生命周期控制。

不依赖传输方式的页面载荷属于 `@orchbee/workbench-client-protocol`：会话 bootstrap、文档提案与发布、投影控制请求、附件 bootstrap，以及投影终端命令/事件流。`@orchbee/desktop-client-protocol` 只负责 Electron 的原子渲染器 bootstrap 与应用生命周期消息。先前从未发布的 Desktop 前缀页面客户端契约不提供兼容性别名。

`apps/web` 是仓库中可运行的 Web 产品入口，也是所有客户端环境共用的规范产品渲染器。缺少 runtime marker 时选择内存浏览器预览；本地 Gateway marker 则把普通浏览器页面认证为 Electron 的对等端。Electron `BrowserWindow` 通过 preload 注入的原子三端口能力包加载同一应用。`apps/web` 下平台中立的 Workbench 客户端运行时只消费窄化的客户端会话与投影端口，并负责文档协调、投影终端呈现、搜索与 Action，不依赖 Electron、`MessagePort` 或网络传输。Desktop 适配器提供 Electron 端口，并在公共运行时旁叠加应用生命周期 UI。Desktop 不拥有另一套 Workspace、Tab、Panel、终端 UI 实现或渲染器构建。共享实现归所属前端包；可运行入口组合继续位于 `apps/web`，不新建 `packages/web` 工作区。

## 输入和边界

| 输入                             | 信任和缺席语义                                                                                              |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| 已存储 `WorkbenchSharedDocument` | `@orchbee/workbench/document` 解析的不受信任的文件输入； missing 是一个独特的初始状态，invalid 表示不可用。 |
| 客户端文档提案                   | 不受信任的能力输入，包含请求标识、预期修订和完整的严格文档。                                                |
| 终端运行时结果                   | 只有与所请求的确切 Presentation 和操作标识匹配后才可信。                                                    |
| Host 退出                        | 不受信任的进程输入由 Host 协议验证，并在发布前通过 Main 绑定注册表解析。                                    |
| 客户端断开连接                   | 附件/会话生命周期事件；它绝不是最终或产品生命周期意图。                                                     |
| 应用退出政策                     | 单独的 Electron 专用功能，可以将协调器置于应用程序范围的静默状态。                                          |

协调器依赖窄端口完成文档加载/提交、终端意图执行、会话事件发布、标识符分配和附件会话处置。确定性核心不依赖 Electron、文件系统、Host、时钟、随机性、网络、React 或 DOM。

## 数据和所有权

```text
DesktopClientSessionCoordinator
├── phase: accepting | quiescing | unavailable | disposed
├── canonical document: missing | WorkbenchSharedDocument
├── event sequence: process-local canonical unsigned decimal
├── proposal queue and at most one active store operation
├── terminal operations keyed by durable operation identity
├── transient TerminalPresentation observations
└── ClientSession[ClientSessionId]
    ├── connection generation and last published sequence
    ├── pending correlated proposal responses
    └── attachment-session disposal capability
```

外围的 Desktop Main 组合负责跨边界清理与退出交付：

```text
Desktop Main lifecycle/projection owners
├── bounded TerminalExited retry queue (identity + newest revision)
├── durable pending-journal/rejection owner (queue-full, association, shutdown, unknown)
├── separate durable publication ledger (identity + newest published revision)
├── pending Host detach set for post-attach failures
├── active projection leases and resize ownership
└── retiring ClientSession records with retryable attachment disposal
```

`ClientSessionId` 和事件序列是 Main 生命周期内的标识。它们永远不会存入共享文档或恢复文件。Client View 和终端视口状态保持页面本地。`TerminalId`、绑定标识、PTY/VT 状态、历史记录和进程状态仍由 Terminal Host 或 Desktop 终端运行时掌权。Main 的瞬态 Presentation 观察只是在客户端水合时所需、限定到 Presentation 范围内的投影。

会话、提案、终端或附件都没有产品级数量上限。分配、结构化克隆、存储、Host 或内存资源耗尽属于类型化失败；绝不因此允许截断、驱逐、复用身份或终止另一个会话。

## 文档提案决策

Main 的纯决策函数是：

```ts
decideDesktopClientDocumentProposal(canonical, proposal, phase, clientAuthority)
  -> commit | converged | stale | rejected
```

| 条件                                      | 结果                             |
| ----------------------------------------- | -------------------------------- |
| 协调器不可用/已处置                       | `rejected/control-unavailable`   |
| 应用程序正在停顿并且客户端不是退出所有者  | `rejected/application-quiescing` |
| 缺少规范，预期修订为空，候选修订为 `0`    | `commit`                         |
| 预期修订等于规范，候选版本是下一个        | `commit`                         |
| 预期修订已过时，但候选内容等于规范内容    | `converged`                      |
| 预期修订已过时且候选版本不同              | `stale` 与规范文档               |
| 候选文档重复/跳过修订或违反已接受退出转换 | `rejected`                       |

提案进入 Main 的 FIFO 队列。胜出者由准入顺序决定，而不取决于传输方式、平台、焦点或墙上时钟。系统不尝试通用合并或自动重放 Action。完整文档不包含足够的语义意图，无法安全合并 Panel/Split 层级与终端删除生命周期。失败的客户端会应用返回的规范文档、协调本地 Client View、丢弃未获准的共享效果，并保持连接。

队列只有在与已接受提案匹配的持久效果发布完成后，才会接纳另一个可能取代该意图的提交。陈旧、格式错误或被拒绝的提案可以立即返回，因为它们无法改变规范状态。这一顺序可防止后续仅修改文档的修订遮蔽尚未发布的终端效果。

## 持久的意图协调

客户端通过 Workbench 创建 Presentation 和操作标识，但绝不直接调用进程效果。Main 会先发布已提交文档，再派生并执行对应的终端命令：

```text
validate proposal
  -> persist exact-next document
  -> publish canonical document
  -> reconcile provision / close / stop intent
  -> publish matching terminal result
```

在一个 Main 生命周期内，每个精确的持久操作最多调度一次。等价副本会加入现有操作或接收其缓存结果。以不同内容复用同一操作标识只会停用违规的 ClientSession。Main 重启后可以借助现有幂等绑定与 CreationId 规则重放仍然持久的操作；本契约不声称跨崩溃恰好执行一次。

`retain` 没有持久操作标识。Main 在恢复后为每个 Presentation 执行一次仅恢复协调，分配瞬态操作标识，并发布所得观察结果。Retain 绝不会创建替代终端。只有对应的持久意图仍为当前意图时，才会缓存当前操作结果；删除或取代意图时会清除缓存，不会保留无限增长的墓碑集合。

## 发布与追赶

每个已发布的变更都会获得一个进程本地序列。发布顺序为：

1. 规范文件；
2. 必要时发布“协调已开始”的观察结果；
3. 匹配生命周期结果；
4. 客户端应用该结果后生成的后续规范清理文档。

会话加入与发布过程串行化。它会收到一个原子 bootstrap，其中包含当前文档状态、文档缺失时的公共初始化种子、当前终端观察，以及该快照之后的序列。实时事件严格从该序列之后开始，因此不存在加入空档。格式错误的事件、错误的身份、被转移的端口或序列空档会使受影响的会话退役。重新连接会创建新会话和完整快照；不会恢复原会话身份或 Client View。

文档和生命周期流量是低速率控制数据。终端帧、输入、历史记录、搜索、选择和复制继续在附件范围的端口上进行，并且永远不会通过协调器。网络传输背压和重新同步是网关的职责。

## 断开连接与投影策略

任一必需会话控制能力关闭时，页面会话会退役，后续请求停止，并等待处置该会话的附件租约。处置失败会把会话留在 Main 的退役集合中，由后续退役或协调器处置重试；不能把失败转换成“已完成”。已经 Main 校验并准入的提案，即使发起者断开连接也可以完成；其持久结果会发布给其余会话。已排队但尚未准入的客户端请求会被丢弃。

会话退役绝不会改变共享文档、绑定注册表、PTY、进程或终端生命周期。显式关闭 Panel 或 Tab 仍是共享产品转换，并会协调其持久关闭意图。

对于运行中的 terminal，第一个成功打开的可写 attachment 获得 Host 的初始独占 resize lease。
后来加入的可写 attachment 获得 read/input/resize capability，但保持 resize observer。
接纳的 focus-in、key press/repeat、已提交 text 或 paste、application mouse press 或
terminal-selection start 会使该 attachment 成为最新 desired owner；attachment、background
state、viewport measurement、pointer motion、release/repair input 与 focus-out 不会。Desktop
按 presentation 合并并发 intent，一次串行一个显式 `ClaimResizeLease`，不设置固定的
Electron/Browser 优先级。Host 先发布旧 owner 的 revoke，再发布新 owner 的 grant；晋升后的
surface 随即提交其测量 geometry。Owner detach 会晋升 successful claim 最新的剩余 attachment，
从未获 grant 的 observer 以最早 admission 打破平局；若 Desktop 记录了更新的剩余 activity，
则会再次 claim。

### Host 连接附件清理

| 字段   | 契约                                                                                                                                                                                                                                                                                                                                   |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | Terminal Host 断开连接时已经销毁该连接拥有的全部附件，但通过已终止的 Desktop 传输重试 `DetachTerminal` 不可能成功，并可能无限期保留本地租约或待分离所有者。                                                                                                                                                                            |
| 输入   | 请求传输状态、活动与临时附件所有权、本地 release 或 retirement、匹配或不匹配的 detach acknowledgement、detach failure，以及 connection end。                                                                                                                                                                                           |
| 数据   | `ProjectedTerminalBroker` 清理状态，以及投影服务拥有的 active lease、provisional attachment、in-flight detach 与 pending detach。                                                                                                                                                                                                      |
| 输出   | 活动 surface 会先收到 `host-connection-unavailable`，随后其 broker 请求清理；每个受影响的本地所有者再通过匹配的 detach acknowledgement 或已确认的 connection end 完成结算。                                                                                                                                                            |
| 不变量 | 请求传输仍可用时，只有匹配的 `DetachTerminal` acknowledgement 才能清除附件所有者。传输终止后，Desktop 不再为该连接发出任何 Host 请求，并且恰好一次清除每个 connection-scoped 本地所有者。两条路径都不改变共享文档、Binding registry、PTY、process 或 retained terminal。重复或重排的 close、release、retirement 与 disposal 保持幂等。 |
| 决策   | 只有对该已终止连接范围内的附件，已确认的 connection end 才是与逐附件 detach 等价的清理证据。传输仍可用时发生的 detach failure 会保留其精确所有者以供重试。                                                                                                                                                                             |
| 模型   | `ProjectedTerminalIpcReliability` 在 release-race 状态机中加入 request-transport end 与 connection-cleanup 转换；它信任 Terminal Host 的连接 teardown，但不假设最终会重连。                                                                                                                                                            |
| 转换   | 匹配的 detach acknowledgement 移除一个所有者；传输可用时的失败保留所有者；connection end 封闭后续请求并结算该连接的所有所有者；之后重复的清理是 no-op。                                                                                                                                                                                |
| 依赖   | Terminal Host 连接 teardown 仍是远端附件移除的权威。terminal protocol 与 Host terminal lifecycle 均不改变。                                                                                                                                                                                                                            |
| 所有权 | Terminal Host 拥有远端连接 teardown，Desktop 投影服务拥有 lease 与 pending-detach 收敛，每个 broker 拥有其 renderer failure 与 port retirement。                                                                                                                                                                                       |
| 验证   | TLA+ action coverage 与 invariants 检查 detach success、available-transport failure、disconnect-before-detach、disconnect-during-detach、retry-after-failure 与 duplicate cleanup。确定性的 HostConnection、broker 与 projection-service 测试覆盖实现边界；composed-runtime 与 application 测试套件覆盖集成和处置。                    |
| 正确性 | Safety 证据只覆盖已注册的有限模型。Progress 要求已经观察到的 connection-end continuation 得到交付；不假设网络恢复或延迟上界。                                                                                                                                                                                                          |
| 排除项 | Live Host reconnect、startup deadline、Windows server authentication、lifecycle-operation replay 与 protocol change 属于独立决策。                                                                                                                                                                                                     |

## 应用生命周期

应用生命周期是一项仅限 Electron 的能力，与普通 ClientSession 控制并列分层。在准备 preserve 或 stop 前，Main 进入 quiescing 状态。已经接受的工作可以完成；来自其他所有会话的新普通提案都会被拒绝。Preserve 不接纳新的文档变更；stop 只接纳退出所有者保持结构不变的 `stop` 到 `stopped` 推进。这样可防止其他页面在应用排空期间创建终端或改变终端结构。

生命周期选择与共享文档提案使用相互独立的端口，因此传输到达顺序不代表权威顺序。渲染器会等待与请求关联的 Main 模式接受事件，收到后才刷新 preserve 状态或生成 stop 意图。只有协调器已经为确切的 ClientSession 进入 quiescing 状态，Main 才会发出该事件。如果退出所有者断开连接，新会话只能接管相同的 quiescing 模式；已有连接的会话或不同模式都不能夺走该工作流。

选择模式前取消不会进入 quiescing。一旦 stop 效果开始，失败仍留在现有的不可逆恢复工作流中；quiescing 不会回滚成表面正常的应用。普通页面关闭绝不等同于应用退出。

## 失败策略

- 无效的存储状态使协调器不可用，并且永远不会被覆盖或修复。
- 存储故障会冻结新的共享更改和终端效果，但保留最后的规范文档、现有主机进程和已打开的投影。
- 一个格式错误、过时、断开连接或被阻止的客户端永远不会淘汰另一个客户端。
- 发布失败仅会停用其目标会话。
- 到达某些会话但在另一个会话上失败的终端退出发布将保留一个序列，并仅重试尚未接受该确切事件的会话；成功的会话不会重播，并且重试不会分配序列间隙。
- 退出持久性或关联失败是类型化的可重试结果。在 Host 监听器返回前，Desktop Main 会同步把每个已接受的 `TerminalExited` 接纳到所有者专用的持久待处理日志。有界的 64 项内存重试队列只是调度器/缓存；它不是崩溃恢复的权威所有者，而且当前 Host 事件信封仍没有确认/重新交付字段。待处理日志、发布账本与持久拒绝日志各有独立容量。队列已满和关机条目可以提升为重试；等待关联或身份未知的条目会被停放，直至观察到确切身份后再提升。身份未知的条目不会进入无界定时重试循环。只有运行时结果、适配器提交、发布账本准入、协调器可见性与实时发布全部成功后，才会移除待处理所有权。待处理日志已满或写入失败时会尽可能记录持久拒绝，否则以失败关闭；绝不会只留下一条跟踪消息。bootstrap 只读取协调器已经提交且可见的观察，不读取待处理预留或尚未提交的账本尝试。
- Workbench 的终端 Presentation 发布是 Main 唯一的生命周期发布。附件的 `ProjectedTerminalBroker` 可以另行转发经过身份校验的原始 `terminal-exited` 或 `terminal-failed` 传输通知，并按附件执行单调修订过滤。该通知不是协调器序列事件，也不构成跨两个通道的恰好一次保证。Main 会先把 `terminal-failed` 记录为保留的 Presentation 结果，再广播它；失败会话只读，但仍可显式关闭。故障恢复依据存活 Host 的清单，而不是退出专用的溢出与发布账本。
- 投影清理失败由服务级租约/分离所有者保留。渲染器端口关闭、畸形控制消息、postMessage 失败、会话退役，以及附件工厂或 open 后失败，会在 Host 请求传输仍可用时通过后续 release、retire 或 dispose 重试收敛；该路径只有收到匹配的 detach acknowledgement 后才清除请求。一旦确认连接终止，Host 连接 teardown 就构成该连接附件的清理证据：Desktop 不再发送 Host 请求，并结算连接作用域内的本地所有者。
- 已删除且从未发布的单客户端 IPC 契约不提供兼容读取器、别名或迁移。版本 1 的共享文档文件格式与路径不变。

## 正确性主张和证据

| 声明                                                               | 确定性证据                        | 形式化/系统证据                                  | 限制                                            |
| ------------------------------------------------------------------ | --------------------------------- | ------------------------------------------------ | ----------------------------------------------- |
| 一条精确修订链只有一个 Main 写入者                                 | 提案决策和队列测试                | `DesktopMultiClientSession` 规范/修订不变量      | 完整文档先写者胜出策略                          |
| 最终效果遵循匹配的持久意图                                         | 意图协调器痕迹                    | TLA+ `EffectRequiresPublishedIntent`             | 主机/账本幂等性可信                             |
| 等价竞态收敛，分歧的陈旧写入保留规范状态                           | 双客户端和有状态属性测试          | TLA+ 陈旧/收敛 Action                            | 无语义合并                                      |
| 每个已连接会话都按发布顺序追赶                                     | 会话 bootstrap/广播测试           | TLA+ 发布进度                                    | 传输暂停/中断时不保证推进                       |
| 页面断开连接仅释放其附件                                           | 会话处理和并发投影测试            | TLA+ 附件/会话不变                               | OS/进程崩溃清理委托给现有边界                   |
| Quiescing 在应用排空前拒绝非所有者变更                             | 退出所有者转换测试                | TLA+ quiescing 不变式                            | 现有不可逆 stop 恢复策略                        |
| 多个查看者仍只有一个调整大小权限；符合条件的活跃查看者可发起 claim | 租约移交、资格与 claim 串行化测试 | Desktop 与 Terminal Host resize lease 模型       | 不按客户端类型设置隐式优先级                    |
| 已发布的投影请求永远不会变为活动状态                               | 客户端、网关和主要发布竞赛测试    | `ProjectedTerminalIpcReliability` 释放竞争不变量 | 一种订购的实时控制连接                          |
| 主机附加响应在稍后的入站交付之前安装其使用者                       | 主机响应/帧/关闭排序测试          | TLA+ 响应-屏障安全与进步                         | JavaScript microtask-before-`setImmediate` 细化 |
| 仅在渲染器就绪并允许绘制后才确认首帧                               | 慢启动帧/传输测试                 | TLA+ 就绪帧确认不变式                            | 确认客户端提交，不表示绘制已经完成              |
| 恢复只以已绘制的新附件替换可信冻结帧                               | 投影表面恢复测试                  | TLA+ 恢复与操作标识不变式                        | 两次尝试；不假定传输推进                        |

TLA+ 配置检查两个客户端、两个文档值、一个附件、缺失与空的可用初始化、等价与分歧的提案、重新排序的断开/提交/效果完成、quiescing 与断开连接所有者的移交、发布，以及两个有界修订（`0` 和 `1`）。`provision` 表示使终端保持活动的效果，`stop` 表示包括 close 在内的终端终结效果，`none` 表示只改变文档。每个抽象客户端身份最多连接和提案一次；依照生产契约，重新连接使用新身份。有状态属性测试使用固定种子与生成的客户端 Action。真实的 Electron 加两个 Browser 跟踪覆盖 Gateway 认证、独立页面会话、共享规范状态、附件断开/重连、Browser 效果和限定到确切附件的背压恢复。显式 resize 控制传输不在本契约范围内。不增加吞吐量验收门槛；终端帧热路径保持不变。

`ProjectedTerminalIpcReliability` 分别检查三个正交配置，而不将状态空间相乘。release-race 配置允许客户端在服务附加前、附加后或 open 事件已发布后释放；清理可能失败，失败的 Main 请求与租约仍由所有者持有，直至退役成功。迟到的 open 事件会因客户端控制能力已关闭而失败，绝不会激活其传输附件。attach-ordering 配置按 Host 线协议顺序将 attach 响应、可选首帧与 close 排入队列。响应屏障必须让 Promise 延续在后续项交付前安装 Broker 与附件监听器。渲染器初始化和首帧交付可以按任意顺序发生，但 `FrameApplied` 同时要求帧已提交且渲染器就绪。弱公平性只适用于已排队、进程内的延续与 Host 交付 Action；渲染器初始化仍是环境 Action，不假定完成期限。

恢复操作配置检查两个全新的附件身份、候选项在绘制前丢失、帧就绪但晋升前丢失、冻结帧保留、仅当候选项仍然存活时才晋升，以及预算耗尽。已经获得帧但随后不可用的候选项会被封存，不能替换冻结投影。Selection 与 Find 是两个独立的原子调用标识；它们可以任意排序，也可发生在 Host 交接、Host 完成、客户端交付、传输丢失和重试之间的任意位置。打开 Find 时只快照该附件已经收到的最新 Host 选择。因此，仍在等待中的选择会产生未预填的 Find 操作，而不会查询超时或稍后静默重新绑定。交给已断开附件的操作结果未知，会封存该附件，而且绝不会复制到重试身份；用户可以在恢复后显式创建新调用，但那不是对旧身份的重放。

该跨边界模型只假设同一个仍存活的 MessagePort、流或 WebSocket 连接内部保持有序交付。它不假设延迟上界、最终重连、断开后的交付、跨连接丢失的恰好一次执行、浏览器调度时长、Terminal Host 内部实现，也不声称每种传输实现都存在细化证明。现有的确定性客户端、Gateway、Broker、Service、HostConnection 和恢复控制器测试负责这种细化。TLC 通过只代表有界安全证据与进程内响应屏障进展，不表示故障网络最终一定恢复。

### 时钟边界和故障检测

跨界正确性源自连接本地排序、相关标识、附件标识、生成、修订和显式终端结果。经过的时间永远不会决定一个 IPC 操作是否在另一个操作之前发生，旧结果是否属于新附件，或者未知操作是否可以重播。

- 10 秒主机请求截止时间是传输切换后的故障检测器。到期会废弃整个主机连接，将每个仍待处理的结果报告为未知，并且不允许自动重播；它不会仅仅因为缓慢的相关性而产生失败的响应。
- 终端搜索看门狗只会停用当前的客户端扫描。它会推进搜索标识、中止行读取并终止 Worker，因此迟到的进度无法发布到替代搜索中。它不会推断任何有关 Host 变更顺序的信息。
- 250/500/1,000 毫秒的投影恢复会延迟安排新的请求和附件身份。它们既不会扩展断开连接的附件的权限，也不会重播其中的选择、输入、剪贴板、链接或搜索操作。
- 字体/GPU 初始化没有截止期限。真实的传输不可用事件会与启动竞态并撤销候选项；已初始化候选项会在晋升前立即重新检查。铃声、完成状态、滚动条与动画定时器只影响呈现。
- 本机浏览器输入结算是 `packages/terminal/formal/tla/TerminalNativeInputSettlement.tla` 拥有的独立本地适配器启发式；它的模型将时间视为事件排序，并且没有提出确切的浏览器调度声明。

网关控制和附件 WebSocket 引导程序当前没有连接期限。因此，既不验证也不发出 `close`/`error` 的套接字可以无限期地离开 `connecting` 中的初始表面；有界自动恢复仅在出现明确的不可用结果后才开始。选择截止日期、用户取消行为以及该截止日期是否废弃一个附件或整个控制功能是产品/协议决策，而不是当前安全改进的一部分。

终端恢复由两个独立检查的 TLA+ 状态机负责，避免一个状态空间掩盖另一个的 Action。`TerminalRecoveryBinding` 负责 Main 获取/释放、同一与跨 Host 纪元的创建恢复、渲染器文档代次、附件替换、最终修订持久化和退出发布。`TerminalExitRecovery` 负责有界易失准入、持久待处理所有权、显式的日志已满拒绝、未知身份观察/拒绝、发布预留与适配器提交、有界实时发布失败、去重，以及跨 Main 释放/重新获取的可见性恢复。其合并配置检查三个退出标识，易失限制为 `1`、待处理日志限制为 `2`，因此易失、仅持久和已拒绝的准入分支都有可达见证。

这两个模型都是比生产协调器更窄、保留所有权的抽象。它们不对会话退役、部分会话交付、序列分配、渲染器传输或文件系统原子性建模。必需的 Action 覆盖只证明所声明的有界配置执行了每个受维护语义分支；通过 TLC 仍只是有界模型证据，而非生产实现的证明。
