# Rust Orchbee Server

## Windows 语义事件

会话 日志记录真实已接受的拓扑、元数据、worktree、Agent 与 Client 变化。
已认证连接持有一个有界订阅；read／wait 保留 cursor 与探测，close／EOF 释放所有者。
终端输出与退出依据已准入 Host 观察；Client 滚动／visible 输出读取准确的只读 Client viewport。
历史断档、来源不可用与观察器回收不确定均明确报告。[事件所有权与验收](docs/windows-events.zh-CN.md)
定义生命周期、上限及 Windows 证据；本项不增加 Desktop 消费者。

## Windows 运行时元数据

同用户 OBLC 元数据请求读取本进程的编译版本、保留的预期 cohort、PID、终端协议、实现
特性位和实际托管／停止模式。它在准确 cohort 的 Client 接纳之前执行，不创建 Connection
代次或 Workbench／viewer 状态，不申请停止记录租约，并在一次答复后关闭。既有候选及
握手上限仍适用，普通 Client 接纳正在停止时亦然。这些是诊断事实，不是操作授权，也不
证明未来可用性。[运行时状态契约](../orchbee/docs/native-runtime-status.zh-CN.md) 拥有
CLI 行为和证据；[本机 Client 协议](../../crates/local-client-protocol/README.zh-CN.md)
拥有帧格式。只有 Windows 分派此新增读取；旧接纳及 Linux／macOS 运行时行为保持不变。

## Windows 持久会话生命周期

对于原生默认/命名 scope，CLI 激活所有者在启动托管 Server 前发布精确的私有 Active 清单记录。Server 在 Worker 效果前准入并
保留该记录，与检查点存储及生命周期所有者一起持有。确认子进程退出并完成监听器清理后，关闭流程先
发布持久 Stopped，再完成运行时激活记录。发布失败保留恢复不确定性，绝不授予正常停止的成功回执。
运行时 crate 拥有清单编码、路径准入及替换；Server 拥有关闭已确认的判定时机。

## 托管 Windows 会话 停止

托管 Server 在 Workbench 已提交变化后以 500ms 防抖自动保存，每 30 秒补存已缓存目录。保留三份有效存档，不保存终端屏幕。启动时恢复最新有效存档；不可读副本保留并向
Client 提醒，全部副本无效时明确提示空工作区。[checkpoint 契约](docs/workbench-checkpoint.zh-CN.md)
拥有捕获、确切物理目录围栏及恢复；会话 owner 的启动准入仍由 runtime 判定。

一个停止标识拥有托管 Server 剩余的生命周期。接受它会原子关闭新应用记录准入；已准入操作及结果写入先结束再清理。
重复开始和状态请求观察保留的尝试；不同标识返回忙。只有相同失败标识的显式重试可开始下一次尝试，最多 64 次。
失败绝不重新开放普通准入，也不授权重放不确定的 Host 变更。每次尝试共用 60 秒协作截止时间，不包括不可中断的
操作系统调用。

所有者保留准确 Worker 子进程，先结算并 join 唯一 checkpoint 保存者、周期进程采样器，以及保留的
Worktree／Workspace-group owner，再停止 Close 与 graphics 维护。上述写者及采样器结算后才捕获最新目录
并发布最终存档；随后将 Worker carrier 冻结为仅停止流量（排队但未发送的工作结算为不可用），并发送一次
有界 Worker 停止请求。采样 viewer 的不确定清理责任保留到匹配 Worker／Host 停止及 Worker 实际退出。已知无 Host 或 Host 已停止结果，以及准确 Worker 退出，必须先于
Agent 观察和 Client 监听器停止、唤醒并 join 保留的 Client 处理线程、发布已停止激活记录。激活锁排除记录发布和
生命周期锁释放期间的并发重启。管道不存在或响应丢失不能证明完成。控制监听器存活时失败尝试仍可查询；监听器关闭后，
严格的激活记录仍为肯定回执。此所有者不进行进程发现、接管或路径移除，也不实现 Linux/macOS 停止。

这是实验性且尚未发布的私有 Rust Server application。它拥有内存 Workbench 导航权威与私有 projection
kernel、第一条 POSIX 本机 Server-Worker process spine 及 serialized post-admission OBCN carrier，
本机 Human Client/Client Connection admission，以及内存终端协调器。产出的 executable 名为
`orchbee-server`；它参与根 Cargo verification gate，由 Linux 开发 cohort 的裸 `orchbee` 使用，但尚未被 Desktop、packaging、
installer 或 service manager 消费。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

## Workbench projection store

侧栏删除可提交带独立完整指纹的 `workspace.close-if-idle` 或 `project.dissolve-if-idle`。
[条件准入所有者](docs/terminal-tab-close.zh-CN.md#条件式空闲准入)在 会话 gate 内证明整个捕获
范围，包括待处理预留和子项绑定。已有运行时或未解析的范围绝不会因 Client 观察而成为空范围。
条件 Workspace Close 安装普通保留栅栏；条件 Project 解散只移除组织，保留成员资源。
确定的 `confirmation-required` 拒绝允许重新审核决定，绝不自动无条件重发。既有显式确认动作
及回执保持原语义。

`WorkbenchAuthority` 拥有一份内存 会话 导航文档及有界 request/result ledger。导航保存按身份排序的
全局 `directories`、`projects`、`workspaces` 数组，以及独立的 `projectOrder` 与 `workspaceOrder` 排列。
Project 仅含身份和标签；每个 Workspace 都必须包含可为 null 的 `projectId`，未分组 Workspace
也是同一文档的普通成员。Directory 是独立于 Project 成员关系的全局资源。
对象图最多容纳 65,536 个 Directory、1,024 个 Workspace，每个 Project 最多 256 个 Workspace，
同时受 snapshot 字节上限约束。

已准入本地 Client 可通过 OBWP 创建或重命名 Project 和 Workspace。
`workspace.create` 接受可为 null 的 Project，并提交一个空 Terminal Context 及其引用。
`workspace.set-project` 将既有 Workspace 加入分组或取消分组；`workspace.create-project`
原子创建 Project 并将该 Workspace 移入其中；`project.dissolve` 删除 Project 元数据并取消
其成员的分组。这些操作保留 Workspace 身份、目录引用、上下文、Tab、Panel、终端绑定及
工作目录，不访问文件系统、不创建终端。Workspace 重命名、Close 及主目录接纳通过稳定
Workspace 身份定位，无需 Project 参数。独立授权的 `workspace.primary-directory.accept`
校验本地 Worker 目录，再原子添加全局 Directory 并由该空 Workspace 选中。
[目录契约](docs/workspace-terminal-directory.zh-CN.md) 拥有校验、重放和失败语义。
`worktree-list` 读取先对已提交 snapshot 复核稳定 Directory 身份，再经既有
carrier 向该目录的属主 Worker 派发一个 fencing OBCN 发现 job；应答是 Worker 有界的原始
`git worktree list --porcelain -z` 字节，在 service 边界解析——membership 与 generation
检查在锁内进行，请求 I/O 绝不在锁内。
本地 `tab.create` 命令追加一个已配置 Workspace Tab，共同提交成员关系与保留的
终端意图，再派发一次启动。精确重放返回同一份待处理或最终结果。后续 Close 可以移除该
成员关系，但不改写历史 Create 结果。Client 可以使用当前 snapshot 中确切的 会话/Worker/Terminal binding，
通过 OBCS attach viewer。[权威模型](docs/workbench-authority.zh-CN.md) 定义授权、revision
冲突、语义重放、分配、停用和证据。读取在与写入相同的 gate 内克隆已提交 snapshot，响应 I/O
位于释放 gate 之后。

`tab.rename` 修改共享 Tab 标签。`tab.move` 在 Workspace 上下文内置换完整简单 Tab，不改变终端 binding，也不获取
Worker 注册表锁。二者均可在启动仍待完成时结算；精确 Move 重放保留原排序结果及修订。
原生 Move 交互仍是独立的 Client 集成边界。

`navigation.configure` 原子调整 Project 顺序，或在 Project 与未分组列表内／之间移动一个
Workspace。协议的纯 `configure_navigation` 校验稳定 ID 锚点、容量和无变化语义；权威层拥有
revision CAS、精确重放、一次聚合发布及检查点顺序保留。它不分配身份、不访问文件系统、不启动
终端。创建 Project 时追加到显式项目顺序，解散时只移除对应顺序成员。

`tabs.configure` 通过同一权威门原子改变分组／固定元数据与顺序，仅分配新分组身份，
拒绝 context 内尚未结束的 Close 操作，且不产生 Worker 副作用。
[原生分组契约](../orchbee/docs/native-tab-groups.zh-CN.md)定义准确转换与回执校验；
共享一致性语料对照 Rust 与 GUI 的排序。

`panel.split` 是第一项树形变换：它在派发第二个 immediate terminal launch 前，共同提交源 leaf、新 leaf、split
方向和两个 slot identity。Server 保留确切的树成员关系；现在的聚合 Close 路径会先销毁已捕获 Tab
中的每个 leaf，再发布一次移除。Linux process witness 创建两个真实 terminal、观察 Split projection、关闭 Tab，
并在 Worker 断开后验证 Host 没有剩余 terminal 或已经退出。原生 panel layout 与 Client-local focus
现已由 Linux 原生 Client 投影该规范分屏树，并在经过验证的叶子之间移动本地焦点；Client 仍一次只
附着一个 viewer，TUI 快捷键也不会提交 `panel.split`。

本地 Workspace 的 `tab.close`、叶子级 `panel.close`、状态 Read 和 Resume 使用保留的生命周期权威及单个进程生命周期
维护所有者。Panel close 恰好捕获一个 leaf，经同一生命周期销毁并清理它，再发布存活
Tab——被移除 leaf 的父 Split 折叠为兄弟子树；关闭 Tab 唯一 leaf 按聚合 Close 处理。已知
Host 销毁和精确 Worker/Server 清理先于规范移除；Client 退出不取消
已接受 Close。Linux 载体/进程检查覆盖三十二个活动 Client 连接、空闲控制槽释放，以及另一
shell 存活时的同连接复用。[Close 契约](docs/terminal-tab-close.zh-CN.md) 规定边界、另有证据
支持的原生 TUI 集成，以及仍待完成的生产恢复和发布义务。

`panel.move` 是无生命周期的树操作：它把同一 Tab 内一个 leaf 重定位到另一个 leaf 旁，折叠源
父 Split，并在单个修订中提交一个 fresh Split 与两个 fresh Slot；被移动 leaf 的 Panel、
Presentation 与 terminal 绑定保持精确不变。

无效、过大、语义不一致或错误 kind 的输入在协议边界失败。请求在解码前受 16 KiB 上限约束，
snapshot 上限仍为 1 MiB。OBWP 拥有请求/结果 envelope。Application library 保持未发布，仅供
自身 executable 使用。完整 TUI/Desktop/CLI Workbench 集成仍未完成。

## In-memory advancement 与 publication

权威的私有 `WorkbenchProjectionStore::advance_projection` 接受一份内部完整 snapshot，要求相同
会话 和严格递增 revision，然后按 canonical identity 顺序派生 navigation replacement 与变化的
context。在 1,024-change 和 1 MiB 编码上限内返回 `WorkbenchPublication::Delta`，否则返回完整
snapshot。没有可见 projection 变化时，empty delta 可以推进 revision。

Store 在改变状态之前校验并序列化 publication。应用 delta 必须精确重现 candidate。Parsing、
会话、revision、refinement 或 serialization 失败保持状态不变。只有权威可以推进该私有组件。
Client 不能替换 snapshot 或提交 delta。Carrier 提供写入后的按需读取；每条已提交 publication
保留在 64 条上限的 journal 中，并通过 `workbench.publication.read` 在相同读取授权下提供给
读取方，使持有基线 revision 的 Client 用有序 publication 快进，而不是重新拉取完整 snapshot。
journal 无法衔接调用方 revision 时，以一条完整 snapshot 边界 publication 应答。主动推送订阅
仍未完成。

## 托管 Linux 与 macOS 会话 启动

私有 `--runtime-directory <absolute-runtime-directory>` 入口消费
[共享运行时契约](../../crates/local-runtime/README.zh-CN.md)。
Server 先校验四产物 cohort 与运行时及不可变激活标记，再独占持有生命周期锁，
以 `setsid` 脱离 Client 的终端会话。Linux 使用 no-replace 方式绑定并发布 0600 Worker socket；
macOS 在私有运行时目录中直接绑定最终 socket 路径，设置 `0600`，并验证路径是由当前
effective user 所有且只有一个硬链接的 socket。随后启动一个精确 cohort Worker。
Worker 注册后永久关闭 Worker listener，不准入替代代次；之后才以已完成权限设置的
原子发布开放 Client listener。启动使用 Server 自有 20 秒预算；Client 仍保有自己的总截止时间。
这里的就绪表示 Server + Worker 注册，不表示已有 Host 或终端。
托管角色的标准流为空；不会发出显式端点模式的 stdout readiness marker。

Client 退出不关闭 会话。Worker 丢失会使能力不可用，不启动替代 Worker；
Server 丢失保留启动标记和 Host 终端，需要显式恢复。本入口不实现恢复、自动清理、
服务管理器安装或重启。普通显式端点测试入口保持下述原有行为。

Linux 与 macOS 编译与 Windows 相同的 Workbench 分发。仅 Windows 支持的能力会一直执行到
其平台适配器，或执行到本启动从不初始化的 owner，由它以自己的类型化结果拒绝：插件存储和
Worker 插件作业返回 `Unsupported`，依赖配置的请求返回 `Unavailable`。启动时会请求启动进程
观察器；这些 Host 无法采样进程，所以观察器在读取设置前就以 `Unsupported` 拒绝，Agent manifest
请求因此返回 `Unavailable`。启动时还会配置 Panel 图形文件帧，其私有帧目录以 `Unsupported`
拒绝预留。Agent 名称与 Windows 走同一条受保护路径；终端名称归属只来自 Agent 报告与进程观察，
因此这里不会出现。终端编辑器、Client view、Panel Agent、已配置命令、Panel 图形、语义事件、
Agent view、Workspace 与 Panel 元数据、窗口标题与通知请求在准入处保持原有拒绝，直到确定它们在
Windows 之外的行为。检查点、停止与交接仍只在 Windows 上，因为只有 Windows 启动会构造它们的 owner。

## 本机 Worker service

在 macOS 与 Linux 上，私有开发调用
`orchbee-server --local-endpoint <absolute-worker-path>` 会绑定一个 Unix-domain Worker socket，
且不删除既有 path。增量形式在其后追加 `--client-endpoint <absolute-client-path>`，从而绑定独立
Client socket。选中的 socket 都被限制为 `0600` mode；仅当所有选中 listener 就绪后，Server
才向 stdout 写入 machine readiness marker `ready\n`。格式错误的显式端点 invocation 保持静默，并以
status `1` 退出。这是内部增量进程 contract，不是 public CLI grammar 或 production
service-installation interface。

Server 与 Worker 通过 kernel Unix-socket credential 相互要求 peer 的 effective operating-system
user。Credential mismatch 或 handshake transport-setup failure 会关闭 candidate，不返回 protocol
response。一个 listener 同时最多准入一个 compatible Worker，并分配严格递增的 process-local
generation。Duplicate、malformed、timed-out、wrong-message、protocol-incompatible 或
cohort-incompatible attempt 都不会改变 active Worker。虽然 OBLW 把 Worker identity decode 为 opaque
16-byte value，Server 仍会在 registry admission 或 `WorkerAccepted` 之前把全零值拒绝为
`MalformedFrame`：该值无法映射到 OBCN 的 nonzero Worker identity，且 rejection 不消耗 generation。
Handshake 在有限 timeout 下同步读取。Server 完整写出 `WorkerAccepted` 后，会把同一个 unbuffered
stream 交给恰好一个 generation-scoped OBCN wire owner，并通过 Worker registry 暴露 cloneable
request handle。由该 owner 而非 caller 或 registry 序列化每次 write/read exchange；wire I/O 期间
不会持有 registry lock。

Handle 会在触碰 wire 前拒绝指向其他 Worker 或 generation 的 request。除了可能正在 in-flight 的一条
request，submission 进入单个队列锁下的两条有界 FIFO lane——interactive 与 cleanup 走 high，
viewer 观测走 low——每条容量为 8。普通 submission 采用 try-admit：lane 已满时立即收到
`Backpressure`，high-lane submission 在已有 cleanup waiter 排队时同样退让；只有两个
connection-cleanup 生产者按 FIFO 等待并优先于
后续 try-admit。low lane 非空时 owner 至多连续派发 4 个 high-lane 请求，随后必须服务一个
low-lane 请求。`Unavailable` 与 `Backpressure` 均表示 addressed request 没有跨过
complete-write boundary。[载体调度模型](docs/worker-carrier-model.zh-CN.md) 拥有 lane 分类、
准入、派放上界、排空与失败结算。Owner
会先完整写出一条 OBCN Launch、Start 或查看 request，再读取恰好一条 result。它只接受匹配的 result
direction 与 kind、operation ID、完整 authorization echo，以及 Start 的 target identity。
Malformed、mismatched、timed-out 或 disconnected exchange 会退役该 Worker generation，并使
queued work 变为 unavailable。

Close 使用仅面向生命周期的 `submit_lifecycle` 边界，将成功入队与后续结算区分开。
校验、帧编码及完成结果存储准备均在既有 `try_send` 前进行；背压不返回回执。成功时返回
一个有唯一所有者的 `PendingLifecycleResult`，即使 carrier 随即退役也不丢弃它。
`admitted_observed_at()` 在成功提交后采样，不是精确入队时刻。继续执行所有者可用此较晚
观察进行保守查询间隔计算，即使重新获取所有者 gate 时已过尝试期限，也必须计入已准入步骤。
`wait(self)` 消费回执并返回精确类型化结果或原始 carrier 错误；完成通道丢失时仍通过
既有 complete-write 标记区分写入前 Unavailable 与写入后 OutcomeUnknown。丢弃回执
不会取消已提交操作。此边界不添加队列、线程、超时、重试、步骤计数器或调度策略，也不改变
既有阻塞 `request()` 路径。暂停 owner 的队列、时间戳边界、背压、路由拒绝、完成通道丢失
及真实 carrier 结果测试覆盖准入/结算分离；Close 所有者模型和集成运行时拥有期限及继续
执行行为。

Attached carrier 会在 wire owner 的整个 lifetime 内保持为 active registry entry。唯一可调用的
admission cancellation 会拒绝 attached carrier；request handle 与 registry 都不暴露独立的 active-
generation deactivation。因此，只要旧 owner 仍可能 write、read、validate 或 publish exact result，
就不能准入 replacement。Owner settlement 会 gate retirement：无论 normal exit、completion receiver
消失、thread spawn failure 还是 panic unwind，owner destruction 都会先把 handle 标为 inactive，根据
complete-write marker 保守结算任何 in-flight request，并 drain submitted slot；之后，其 RAII
registration guard 才移除 matching generation。该 removal 是允许准入 replacement 的 retirement
linearization point。Stale guard 无法移除 replacement。

每次完整 request write 有两秒、完整 result read 有二十秒的 monotonic deadline；partial progress
不会重置 deadline。Request frame 完整写出前的 failure 会被报告为 `NotDelivered`，且为 known
unchanged：strict Worker decoding 无法 dispatch incomplete frame，owner 还会退役该 connection。
完整写出后，任何 failure 都是不同类型的 `PostDeliveryOutcomeUnknown` carrier error：Server 不会
伪造 semantic OBCN `OutcomeUnknown` value，也不会 retry request。来自 Worker 的确切 result
（包括 Worker 报告的 semantic unknown outcome）会原样保留；carrier 不解释 launch capability-version
或 lifecycle outcome variant。查看结果还必须匹配附件身份与 action/outcome kind。没有 outstanding request 时收到 byte 属于 invalid，会退役 connection。
Drop 整个 registry 会请求私有 shutdown，但无法准入 replacement；owner 会在 25-millisecond idle
poll 或 bounded current exchange 内观察到它，后者最多包含剩余 write deadline 加一段 result-read
deadline。不存在无界 connection thread 或 message queue。

Endpoint path 由受控开发 witness 提供。Discovery、stale-endpoint recovery、default runtime
directory、跨 candidate path 的 singleton activation、daemonization、Windows named pipe、production
service supervision 与 shutdown signaling 都不属于本切片。进程被突然终止或 bind 后 setup
失败时，可能留下由外部 owner 负责移除的 socket path。

选中 Client listener 时，bind 或 listener-thread spawn failure 会在 readiness 前发生，并使 Server
startup 失败。Readiness 后发生的意外 Client listener accept failure 会结束该 listener，但尚不与
Worker loop 协调；production fault supervision 与 restart 仍属保留事项。

私有 [`orchbee-local-worker-protocol`](../../crates/local-worker-protocol/README.zh-CN.md) crate 拥有
Worker framing、handshake、cohort rejection 与 bound；
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.zh-CN.md) 拥有 kernel
same-user proof。私有
[`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.zh-CN.md) crate 拥有
OBCN semantic value 与 frame encoding。Server 拥有 listener lifetime、Worker admission state、
generation、bounded submission、serialized carrier validation 与 generation-matched cleanup。
[`orchbee-worker`](../orchbee-worker/README.zh-CN.md) application 拥有连接进程 lifecycle；其当前
runtime 已通过 Host broker 消费获授权的 Launch/Start 和只读查看 request。

## 本机 Client Connection service

选择私有 Client endpoint 后，每条 same-user compatible Human Client connection 都会创建一个独立
process-local Client Connection generation。TUI、Web、Desktop、mobile、tab、window 与 renderer
identity 被有意排除：该 endpoint 准入的是统一的 peer Client model。Server 最多接受三十二个 active
Connection，并最多保留四十八个 candidate/connection handler thread。每条 handshake 有两秒 read/write
timeout。因此，只要还有 handler slot，一个 silent candidate 就不能阻塞另一条 connection；饱和
时，多余 candidate 会被关闭，且不会分配更多 thread。Handler slot 与 admitted-Connection
registration 都由 lifetime-owned guard 持有，因此正常完成、fault 与 unwind 都会释放两类上限。

经过身份验证的 candidate 会收到 protocol/cohort 不兼容、畸形或错误消息、超时、active-Connection
容量超限等类型化拒绝。Server 完整写入 `ClientAccepted` 后，移除握手超时，在同一条未缓冲
stream 上读取严格 OBCS 或 OBWP。私有服务向其单一本机有效用户主体授予默认 shell 的 Create、Start
及这些终端的查看和显式获取独占 input/resize controller 权限。它读取一个请求、调用 Server 服务、完整写回关联结果，然后才读取该 Connection 的下一
个请求。结果在 Client 输出前提交；写入停滞只占用自身 Connection handler，不持有协调器或 Worker 锁。
读取在首个字节前没有截止时间；此后整条记录限时两秒，包括大 `OBCQ` 粘贴记录。
每次完整 Client write（包括大 OBSR
记录）也限时两秒。超时只退役对应 Connection。

Create 选择当前本机 Worker，并验证显式精确 Worker 约束。Start 通过 request ID 跨同一主体的
Connection 解析成功的 deferred Create。Server 保留精确 Worker 代次和完整 Host 身份。Worker
丢失或更换不能授权重新指派、重新创建或重放。EOF、畸形、错误方向、超限、截断及读写失败
仅关闭对应 Connection；按代次清理保留终端状态。

没有对端能接收的失败——处理线程无法创建、握手传输设置失败或对端凭据被拒、registry 中毒、
代次耗尽、停止结果无法送达，以及因故障结束的 Connection——经该服务私有的 `diagnostic` 出口
输出：一行与语言区域无关的标准错误输出，写明 `operation`、`stage`、I/O 错误类别或 `protocol`
形式的 `cause`，存在时再附 Connection 代次。多路复用处理器的失败保留其自身阶段。会话停止和
对端离开（流结束、重置、中止、管道断开）属于正常结束，不报告。该行不含记录字节、终端内容、
路径、凭据、环境值或嵌套错误文本，也不改变对端可见的结果。托管启动把 Server 的标准错误绑定到
空设备，因此只有持有它的测试或开发 harness 能读到这些行。

私有 [`orchbee-local-client-protocol`](../../crates/local-client-protocol/README.zh-CN.md) 拥有
Client framing 和连接适配器。独立的
[`orchbee-client-server-protocol`](../../crates/client-server-protocol/README.zh-CN.md) binding
拥有 OBCS 验证及帧 I/O。Server 拥有主体策略、放置、operation/lease/fence 签发、终端绑定状态、
有界提交和关联的语义结果。Linux 裸 `orchbee` 已消费此路径；持久化和生产 ingress 仍在该切片之外。

## 本机 Worker process-spine 建模记录

| 字段   | 现行 contract                                                                                                                                                                                                                                                                                                      |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 问题   | 已验证原生 artifact 之间没有真实进程关系。本 Worker 子骨架必须证明独立 Server 可以准入并保留一个独立、same-user 且 compatible 的 Worker，同时不创建 terminal、activation policy 或 durable state。                                                                                                                 |
| 输入   | 不可信私有 argument vector、开发 owner 提供的 absolute endpoint path、kernel peer credential 与一条有界 framed Worker handshake。Absence、malformed byte、timeout、wrong message、wrong user、version mismatch、cohort mismatch、duplicate Worker 与 disconnect 均显式表示。                                       |
| 数据   | Server phase、可选 active Worker identity 加 nonzero process-local generation、next generation 与一条 candidate connection。Server 是唯一 writer。所有 value 都不跨 process exit 保留。                                                                                                                            |
| 输出   | Bind 成功发出一条 `ready\n` marker。Worker admission 返回其 generation；当 stream 允许时，经过认证的 protocol-level rejection 返回 typed bounded reason。Peer-authentication 与 transport-setup failure 会静默关闭。Process setup 或 listener failure 以 status `1` 退出，不产生 human diagnostic。                |
| 不变量 | 一个 endpoint 最多一个 listener；一个 Server 最多一个 active Worker；active generation 非零且严格递增；rejection 不能替换 active authority；generation-scoped cleanup 不能移除 replacement；Worker disconnect 不停止 Server；Server loss 关闭 Worker connection。                                                  |
| 决策   | Invocation validity、peer-user equality、frame validity、protocol/cohort equality、预期 Worker message kind、active-Worker absence 与 matching cleanup generation 完全决定结果。Current directory、Project、Environment、Worktree、Agent 与 Terminal fact 均不参与。                                               |
| 模型   | Deterministic Worker registry 拥有 admission 与 generation-scoped disconnect。薄同步 Unix-socket workflow 负责 authenticate、decode、admit 或 reject、acknowledge，并 monitor accepted connection。                                                                                                                |
| 转换   | `absent -> listening`；candidate `none -> pending -> admitted/rejected/timed-out`；Worker `none -> active(generation) -> none`；Server `listening -> stopped`。第二次 bind 被拒绝，stale cleanup 对其他 generation 是幂等 no-op。                                                                                  |
| 依赖   | Rust standard-library process、filesystem、Unix socket、timeout、thread 与 I/O，加私有 local-worker protocol 和 kernel peer-credential crate。没有 terminal、child process、network socket、async runtime、persistence、environment lookup 或 randomness。                                                         |
| 验证   | Pure codec 与 registry test；macOS/Linux 原生 same-user socket-pair test；覆盖 bind、readiness、admission、duplicate/incompatible rejection、disconnect 后 replacement、Server loss、malformed peer、确切 output 与 cleanup bound 的真实 Server/Worker process test。这些实现轨迹不构成穷尽的 admission 协议证明。 |
| 限制   | 本 Worker evidence 覆盖同一 effective user 下的 macOS 与 Linux。它不证明 Windows behavior、cross-user negative execution、production endpoint discovery、crash cleanup、service activation、Client Connection semantic、terminal retention 或 remote security。                                                    |

在三个 generation 的探索范围内，`AdmissionUsesPreviousCursor` 与
`GenerationCursorChangesOnlyOnAdmission` 独立检查每次 Worker admission 都复制并推进
process-local cursor。Generation exhaustion 与 implementation refinement 仍由确定性 Rust 证据负责。
具名 rejection action 保留抽象分类与 authority-preservation proof；精确 typed 或 silent wire
outcome 仍由 protocol 与 process test 负责，因此模型不保留累计 rejection history。

## 本机 Worker OBCN carrier 模型与 runtime refinement 记录

这份 model-first 记录固定了完整写出 accepted OBLW response 后最小的 Server-owned carrier
contract。该 model 是已接受的 refinement target；上面的 service implementation 提供对应 runtime
evidence，但不规定 OBCS business policy。

| 字段   | 已接受的 refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | OBLW admission 后，Server 必须在有界 OBCN Launch/Start exchange 中复用该精确 Worker generation 的 stream，且不能让 registry、request submitter 或其他 task 成为第二个 wire owner。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| 输入   | 一个 current Worker identity 与 process-local generation；三条合法 request candidate，其中两条是 operation ID 不同但 kind 相同的 Launch，另一条是 Start；active 或 inactive 时的 stale-handle submission；complete-frame write 或未完成 write；有限 monotonic per-I/O deadline tick；disconnect；以及一个真实 incoming response value。Response domain 携带 direction、result kind、Terminal ID、operation ID、Worker ID/generation、fence、lease ID/expiry、capability、Start target 和精确 outcome payload，包括两个不同的 definite payload，以及 Launch `OutcomeUnknown` 中的 Host epoch。                                                                                                                     |
| 数据   | Model 保留一个 `activeRef`、一个 generation-scoped `requestHandleRef`、一个 scalar `wireOwnerRef`、一个 capacity-one `submittedSlot`、最多一个 `wireRequest`、其 `wirePhase`、`now` 与 `wireExpiry`、environment-owned `incomingResponse`，以及 attempt/completion/publication audit state。已检查 configuration 使用两个 Worker identity、两个 generation、两条 Launch operation 与一条 Start operation。Submitted slot 可以与一条 wire request 共存，但只有后者能处于 in-flight。                                                                                                                                                                                                                               |
| 输出   | Complete request-frame write 前的 rejection 为 `known-unchanged`；exact OBCN result 保留经过验证的真实 incoming value 及 payload，包括 Worker `OutcomeUnknown`；complete write 后的 timeout、disconnect 或 invalidity 变成 carrier-level `effect-unknown`。后者不是伪造的 OBCN result。Retirement 把 queued slot drain 为 known unchanged/unavailable；inactive stale handle 无 write 即 unavailable；任何 class 都不会引发 automatic retry。                                                                                                                                                                                                                                                                     |
| 不变量 | 最多存在一个 current generation 与一个 stream owner；registry 只存 request handle，绝不存 stream；submitted slot 与 wire slot 各自以一为上界；每条 operation 最多有一次 complete write；deadline clock 单调；exact publication 必须等于此前 observed response，且其 direction、kind、operation、每个 authorization field、Start target 与 outcome payload 都匹配；每种 mismatch class 与 invalid unknown payload 都不能 exact publish；unknown effect 不可 retry；replacement 等待旧 completion 与 publication settle；stale activation 与 generation-matched cleanup 不能替换或移除 current generation。                                                                                                         |
| 决策   | Wrong-Worker 与 wrong-generation routing 在 enqueue 前 reject，且不 retire owner。Stale handle（包括没有 active generation 时）返回 known unchanged/unavailable。Capacity 是一条 submitted request 加一条 wire request；新鲜的第三条合法 request 记录 backpressure 且不改变两个 occupied slot。`CompleteRequestWrite` 是 ambiguity boundary，并在 request-write deadline 后安装新的 result-read deadline。Local time advancement 与 expiry 让 silent peer settle；response 与 disconnect arrival 没有 fairness assumption。`RetireCarrier` 关闭 generation gate：之后不能继续 write 或新观察 exact response；若 exact result 先被观察则仍可 publish。Replacement 要求每个旧 completion 与 publication 已 settle。 |
| 模型   | carrier 状态所有者分离 registry identity、cloneable request-handle authority、queue occupancy、sole wire ownership、write completion、actual response arrival、local validation、deadline expiry、publication、retirement 与 cleanup。`ReceiveExactWorkerResult` 验证并保留 `incomingResponse`，不会构造一个 matching result。该模型细化 post-OBLW boundary 与 OBCN semantic contract，而不重新实现任一 byte codec。                                                                                                                                                                                                                                                                                              |
| 转换   | `ActivateCarrier` 发布一个 generation；`SubmitFirstLaunch`、`SubmitSecondLaunch` 与 `SubmitStart` 覆盖不同 candidate；`DequeueRequest` 将一个 value 移给 owner；`CompleteRequestWrite` 跨过 ambiguity boundary；`AdvanceMonotonicTime` 使 write 前后 expiry 生效；exact arrival 与针对每种 mismatch class 的具名 arrival action 填充 `incomingResponse`；`ReceiveExactWorkerResult` 或 `RejectMismatchedWorkerResult` 验证它；disconnect action 覆盖 idle、before-write、after-write、queued-after-write 与 after-result ordering；publication、late-result discard、replacement、stale rejection 与 generation-matched cleanup 完成 retained state。                                                             |
| 依赖   | Target 将现有 Server Worker registry 和 post-`WorkerAccepted` stream 与私有 local-worker/capability-node protocol contract 组合起来。Local-worker protocol 拥有 OBLW framing，capability-node protocol 拥有 OBCN value 与 strict framing，carrier 负责组合二者。OBCS mapping、authorization decision、Host dispatch、terminal retention、persistence、endpoint discovery 与 remote ingress 均不参与。                                                                                                                                                                                                                                                                                                             |
| 所有权 | Server registry 拥有 current Worker generation 与 request-handle publication。一个等价于 `WorkerWireOwner` 的 owner 排他拥有 stream read/write 与 queue draining；request-handle clone 只能 submit。Codec 拥有 byte validity，Worker 拥有 semantic OBCN result，后续 Server policy owner（而非本 carrier）拥有 authorization、reconciliation，或对另一项已证明 known-unchanged operation 的任何 permitted retry。                                                                                                                                                                                                                                                                                                 |
| 验证   | carrier 测试覆盖准确关联、响应字段/payload 不匹配、各写入侧过期、断开顺序、过期 handle、替换门控、清理及已占队列容量。确定性时钟检查本地结算；不保留独立 carrier TLA 证明。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 限制   | 这是 bounded formal evidence，而不是 runtime-refinement evidence。它把 complete-frame write 抽象成一个 boundary，并使用 one-tick finite write/read deadline 而非 concrete duration；它不证明 partial-I/O code、system clock implementation、thread scheduling、panic cleanup、OS behavior、codec correctness、OBCS-to-OBCN policy、Host effect、durable recovery，或其他 authorized owner 是否应 retry 一条 known-unchanged request。两个串行 phase deadline 对应 configured write timeout 加 response timeout。                                                                                                                                                                                                  |

已实现的 runtime refinement 把 `WorkerRequestHandle` 映射到 `requestHandleRef` 加
`submittedSlot`，并把唯一 `WorkerWireOwner` 映射到 `wireOwnerRef`、`wireRequest`、`wirePhase`、
`now` 与 `wireExpiry`。这个 scalar expiry 按 phase 解释：`DequeueRequest` 启动 total write
deadline，`CompleteRequestWrite` 用 total read deadline 替换它。`WrongWorker` 与
`WrongGeneration` 绝不进入 `submittedSlot`；`Backpressure` 保留两个 occupied slot 不变；
`Unavailable`、`NotDelivered` 与 queued-owner
shutdown 映射为 `known-unchanged`；`PostDeliveryOutcomeUnknown` 仅在
`CompleteRequestWrite` 后映射为 carrier `effect-unknown`。Deactivation 必须与 owner
linearize：complete write 前为 known unchanged，complete write 后为 unknown，而且只有
`ReceiveExactWorkerResult` 在 `RetireCarrier` 前获胜时才能接受 exact result。仅在 exchange
开始前检查 active flag 不足以防止 retirement 后 write 或新接受 exact result。已被观察的 exact
result 仍可在 replacement 前 publish；replacement activation 必须等待该 publication，与
authorization model 的 pending-result guard 一致。Carrier 不得把任何 transport classification
转换为 successful 或 failed OBCN result。该记录所探索的单 submitted-slot 占用自此被
[载体调度模型](docs/worker-carrier-model.zh-CN.md) 中的双有界 lane 取代；wire-phase、
ambiguity-boundary 与结算义务不变。

## 本机 Client Connection 与 OBCS 建模记录

Connection 模型探索三个 Client、两个 active Connection、三个 handler 和有界代次。OBCS 的
stream-fault 和 correlation 配置包含合并的握手/请求、部分帧、输出停滞、错误方向、重复及
不同请求。这两个模型都不证明终端授权、Worker 派发、取消、重放、持久化或实时 relay。
下文的协调器模型和进程测试拥有新增副作用边界。

## 本机 Client 查看附件转发细化

已接受的本地终端里程碑现在把 Server Client Connection 接入 Worker 现有的只读查看附件循环。
暂存的原生 LocalClient adapter 消费查看协议。进程见证证明两个独立准入的 Client 经 Server 和
Worker 接收真实 Host projection。下文的 controller 细化增加 input/resize；Linux 裸入口已消费此 relay。

每个已准入 Connection 在既有有序 carrier 上最多拥有八个临时 viewer。Attach 引用同一本机
subject 下已接受的终端 Create；AttachBinding 提供精确 canonical binding selector。Server
解析保留的 Host identity 与当前 Worker/generation，分配新的 request、attachment 和 cleanup
identity，并固定 Worker handle。Client 不能分配权威 runtime identity；后续每个 viewer/control
action 都必须回显一个由 Server 签发且归属发起 Connection 的 attachment ID。每次 poll 移动一条
完整不透明 Host record；Server 验证 envelope、所选 attachment 与精确 feedback scope，不解码
终端呈现。每个 viewer 独立拥有 projection flight。两个 flight 的相等计数不能认证内容：显式
selector 决定反馈归属。Poll 同时排空该 viewer 的 kind-53 `HistoryChanged` event lane：Worker
在 frame record 槽旁维护一个有界的 per-viewer event FIFO，并通过显式 next-preference 在两个
来源间交替，因此 queued event 在 frame flight 未结算时仍可交付，持续就绪的来源最多越过另一
来源一次。kind-53 record 只对 `Poll` 是合法结果；任何其他 action 收到它属于严格 route
error，绝不进入 `projection.receive`。Event 交付不会把 frame flight 标记为已交付，
`HistoryObserved` 走自己的 acknowledgement flight，而不结算 `FrameApplied`。Event FIFO 或
byte-budget 饱和只关闭该 viewer；frame 与 event byte 在每条 teardown 路径上恰好释放一次；
发往已退役 viewer 的合法 kind-53 event 像迟到 frame 一样被吞掉。
Attach 在 Client publication 前提交。显式 detach 只释放所选 viewer 及其 controller；Connection
断连或 publication 失败清理其全部 viewer。这些操作不终止或重新指向 retained terminal。
Unknown exchange 不重放；重新附着获得全新 attachment identity，并要求 snapshot。

Poll 还接纳精确 Host epoch、terminal 和 creation 身份对应的 Host 终端结束记录（kind 15 和
19）。它们保留 projection flight，不需要帧确认。Worker 从保留的 inventory 为重新附着的
viewer 初始化结束状态；Client 按[终端结束状态](../orchbee/docs/native-panel-lifecycle.zh-CN.md#终端结束状态)
保留最终输出并降级控制权。

Coordinator 或 registry lock 均不跨越 Worker/Client I/O。Drop cleanup 阻塞准入现有的单项
Worker request queue，不等待 Client response，也不增加无界队列。退役 handler 保留原有的
有界 Client Connection slot，直到 cleanup 准入完成。Wire owner 在内部观察完成。
Carrier 停止会关闭 broker 及其全部临时附件。Worker response 预算为二十秒，以容纳最多十六个
待处理 Host 查看／控制 detach、两个 Attach exchange 和 result delivery；control write 仍保持两秒边界。
一个排队请求加一个在途请求的边界要求四十六秒 launch lease。Client record read 在首字节之前
允许 idle，之后使用总计两秒 deadline；Client write 同样有总计两秒 deadline，超时只退役该
Connection。

Viewer/control 的精确 ServerBusy 响应保证终端、查看附件与 controller effect 已知未改变。
Carrier 队列拒绝发生在 dispatch 前；Attach 的 Worker 容量拒绝可以经过 OBCN exchange，
但不会插入查看附件或向 Host 派发操作。Input/Resize 的 Client sequence 可能已被消耗，
因此已准入的 Client 背压策略必须同时使用新请求 ID 和新 effect sequence，保留原逻辑操作截止时间。
其他不可用、拒绝或未知结果不授予再次尝试权限。

## 本机终端 controller 细化

已接受里程碑为 Connection 查看附件增加显式获取操作，每个保留 Host 终端只有一个输入与调整大小
共用的 controller。Server 在 Worker I/O 前预留 grant；该 grant 待定、存活或清理期间，其他
Connection 不能获取。Server 新签发的 controller 身份限定 input 与 resize 共用的严格递增 sequence。
Client 提供的身份必须等于发起 Connection 内所选 viewer 的 grant。非 controller 与过时 sequence 的拒绝不派发请求；
有效提交的 sequence 即使交付失败也被消费。未知副作用不重放；authority 丢失不重新选择终端。

Worker 将 grant 映射为独立 Host 附件，启用 input 和 resize、禁用 read，并申请 resize lease。
既有查看附件保持只读并保留 frame baseline。Worker 同样保证每个 Host 终端只有一个 grant，
校验精确 viewer/grant scope 与递增 sequence。只有 Host 返回的确切 capability revision 可授权
原始输入。Input 包含 1 至 4,096 byte；接受表示进入 Host 有界输入 queue，不表示 shell 已执行。
Resize 确认精确的完整 geometry。不增加 Host wire contract 或 VT owner。

Host cleanup 先释放控制附件，再分离查看附件。显式 OBCN Detach 在 Worker 内部执行此顺序；随后 Server lease Drop 发送幂等 Release 结算 registry。Connection Drop 则先提交 OBCN Release 再 Detach。Server 预留持续占用，直到固定代次的
cleanup 完成或该 Worker 不可用；cleanup 经既有有界 request queue 准入，不持锁跨越 I/O。
过时 cleanup 不能清除新 grant。Host exchange 失败或结果矛盾会结束 Worker connection；完整
交付后的故障由 Server 报告 unknown control outcome。其他 viewer 不会自动提升。
显式接管现在遵循[控制权接管模型](docs/terminal-controller-takeover.zh-CN.md)：Pending 预留
封锁竞争接管和过期效果；候选者负责新旧清理，直到交接得到确认。保留 Viewer 的独立
release 仍是后续工作。

最多八个 viewer 意味着最多八个控制附件。因此，cleanup 可在下个请求前执行十六次一秒 Host
exchange。Worker response 预算为二十秒，launch lease 为四十六秒，原生 Client submission 为
五十秒。Request write 和 Client record read/write 保留既有两秒上限。这些是本机实现限制，
不是发布性能预算。

## 本机终端 send

已准入 Client 可对其本可关闭的、已暴露 Workbench 终端提交一次非交互 input send。selector
指明 Server 在本次准入时验证的精确 session、worker、terminal 绑定；它不是从早前 list 结果
取得的隐式化身锁。send 是新增的显式编排输入权，不是只读查询，也不是权限零变化；它不新增
远程暴露，也不绕过 cohort、peer 或 Host 能力检查。

send 准入依次解析 Workbench binding 暴露、唯一保留绑定、当前 Worker placement 与绑定记录的
Worker generation、Started 终端相位、Unfenced 生命周期以及无已受理 close。未启动、已关闭、
已 fence 或 generation 过期的目标在 dispatch 前拒绝，且不消耗任何 Server 签发 identity；
解析绝不重绑陈旧化身。准入时 Server 铸造新的 operation identity 与新的 input-only
attachment identity，并把精确的 worker id、generation、Host epoch 与 Host terminal id 钉入
恰好一个 OBCN 请求；Worker 绝不按逻辑标识重解析。若被钉住的 Worker 在 dispatch 前退出或
generation 变化，该请求以 unavailable 或 unknown 失败，绝不重发。

Worker 在其既有 Host broker 上执行一次有序的 attach、input、detach；不借用任何 viewer、
controller 或 resize grant。受理仅表示输入已进入 Host 的有界 input queue；它不说明子进程
读过或执行过。Host 的类型化拒绝仍执行一次 detach。传输失败、错误 ack 或不确定 detach
之后，Worker 关闭该 Worker-Host 连接——其拆除即回收临时附件——绝不在可能失同步的流上继续
新 exchange。每个结果保留两个独立事实：input 状态（not-submitted、rejected、accepted 或
unknown）与 cleanup 状态（not-needed、confirmed 或 unconfirmed）。受理后的 cleanup 失败
保留已接受字节数，绝不把输入报告为未发送。

每次 send 提交一个 OBCN 请求，其标识只关联一次请求与响应；不新增持久 dedupe 账本，手动
重试的 send 是新请求、可能重复送达输入。send 与 controller exchange 共用单一 Worker
carrier 串行，输入顺序等于到达顺序。既有二十秒 Worker response 预算、两秒 write 上限与
五十秒 Client submission 窗口全部保持；三次一秒 Host exchange 在其内容纳，无需扩大。

## 已接受的 Terminal Client authority 与 relay 建模记录

`RelayDeliveryIsolation.tla/.cfg` 负责传递隔离。两个 viewer 各有独立 frame/history 队列，每条通道只有一个在途记录。两个源记录与容量为一的队列同时覆盖正常排空和溢出。溢出的 viewer 关闭时不清空对端工作。反馈必须指向准确的已传递记录，history 传递保留 frame flight，停滞的 viewer 不得禁止 Worker 前进。这是安全性模型，不假定网络传递公平。

`Frame`/`History` 对应 relay 入队，`DeliverFrame`/`DeliverHistory` 对应 viewer 传递，反馈动作对应确认，`Close` 对应 viewer 局部清理。实际字节、生产容量、请求 ID、attachment 代际与编解码由 `local_terminal_service/viewer_connection` 和 Worker carrier 测试检查。权限与清理顺序归 `ConnectionViewerOwnership` 所有；两个独立模型不证明整个产品组合。

变异套件允许错误 flight 的反馈并要求得到反例。运行 `node scripts/tla/run-models.mjs --models local-terminal-relay` 及 Server/Worker 实现测试。

## 已接受的 Connection Viewer 路由建模记录

`ConnectionViewerOwnership.tla/.cfg` 检查两个不可变 viewer 身份、两个 Connection、两个目标及两个 Worker epoch。Attach 探索全部 Connection/目标分配，包括同目标竞争。控制权 grant 在所有者关闭期间保持预留；控制权释放先于 viewer 退役，过期清理不能释放后继 grant。控制接纳同时校验 Connection、目标、viewer 和 Worker epoch。目标销毁或 Worker 替换会关闭受影响绑定。控制权/viewer 释放的弱公平保证最终退役。

`Attach`、`Acquire`、`Control`、`Close` 及释放动作映射到 `local_terminal_service/viewer_connection`、`controller_connection` 和 Worker carrier 所有权。模型内 viewer 身份永不复用。容量算术、候选 attach 失败、frame/history 路由、线协议校验及复合操作结果由既有服务单元测试和 `tests/client_process` 轨迹负责，不属于身份核心的穷举声明。relay 模型独立检查通道隔离。

维护中的迟到清理变异必须违反 `StaleCleanupPreservesSuccessor`。运行 `node scripts/tla/run-models.mjs --models connection-viewer-routing` 及所属 Server 测试。

## 已接受的 Connection Viewer 历史读取建模记录

本已接受切片为一个已存在的 admitted viewer 增加一次性保留历史读取。它沿用同一条
Connection/Server/Worker/Host 路由主干，只有一次关联 exchange，不引入任何长生命周期状态。
[OBCS 契约](../../schemas/client-server/README.md) 拥有 wire 表示；capability-node README 的
OBCN viewer 节拥有 tag-6 编码。

| 字段           | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                               |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | 一次性消费者（本机 CLI）需要有界的保留 scrollback 加可见行，不能长期占用 viewer、消费实时流或发明第二条读取路径。                                                                                                                                                                                                                                                                                        |
| Inputs         | `read-rows` action：归提交 Connection 所有的 `attachmentId`、绝对 `startRow`、`rowCount` 1–4096、精确钉定的 `revision`/`generation`、正数 `requestSequence`、`maximumBytes` 112–8,388,608（界定 OBRW payload）。Client 根据一帧已验证帧计算 `startRow`/`rowCount`（`K = min(N, S, 262144 / columns)`，`start = S - K`）；Server/Worker 将该 action 视为不可信输入并逐字段重新校验。                      |
| Data and owner | `LocalTerminalConnection` 仍拥有该 viewer 条目及其钉定的 Worker handle。Server 为每次 dispatch 签发一个全新的 Worker `OperationId`；该身份单独充当内层 Host correlation，绝不等同于外层 OBCS `requestId`。exchange 之外不保留任何 per-request 历史状态。                                                                                                                                                 |
| Outputs        | 恰好其一：经 OBNR/OBSR 返回的完整关联 OBTP response record、携带九种封闭 reason 之一的 `RowsRejected`，或既有明确的 `Rejected`/`Unavailable` outcome。超出字节预算是 typed failure，绝不是部分成功。`Empty` 绝不回答 `read-rows`。                                                                                                                                                                       |
| Routing        | Server 在发起 Connection 内解析 selector（不归其所有 → `terminal-unavailable`），重新校验钉定的 Worker/generation/current-worker，然后 dispatch OBCN tag 6。响应 record 的 Host correlation 必须等于本次 dispatch 的 Worker `OperationId`；role、kind、encoding、Host epoch 与 attachment 必须匹配请求 scope。该 record 绕过只懂 frame 的 `projection.receive`。穿插的 Host event 仍路由到各自归属附件。 |
| Settlement     | Host `TypedFailure` 映射为封闭的 `RowsRejected` reason 集；合法的 `RowsRejected` 不会使健康 viewer 退役。非 `ReadRows` operation 上出现 `RowsRejected`，或在不合法 phase 收到 response `Record`，均为协议违例。buffer 读取不发送任何 acknowledgement（`RowsApplied`），任何失败后都没有隐式 retry、resend 或 retargeting。                                                                               |
| Lifetime       | 该 exchange 受既有一秒 Worker/Host 预算约束，处于调用方 deadline 之内。Viewer slot 释放与 detach/drop 仍归既有 Connection cleanup；此处不延长 viewer 生命周期。                                                                                                                                                                                                                                          |

该 action 仅为增量的同 cohort 变化：旧 Worker 拒绝 tag 6；同 cohort 违例（内层 correlation
错误、phase 不合法的 response record、其它 action 上出现 `RowsRejected`）一律 fail closed，
而不是降级为猜测性解释。Worker 还会拒绝实际 OBRW payload 长度超过本次请求有效预算
（`min(requested, negotiated row-window, record carrier)`）的成功响应；64 字节 OBTP header 计入载体边界，
而 `maximumBytes` 只界定 OBRW payload。

## 本机终端协调器建模记录

已接受的本机里程碑授权 Server 的单一有效操作系统用户主体，通过获准接入的本机 Human
Client Connection 执行默认 shell 的 Create 和 Start。OBLC 本身仍只负责传输准入；此私有服务
在验证 peer 和 cohort 后显式选择该策略。其他端点、远程身份、Worker peer 或渲染器类型
不会因此获得权限。

| 字段   | 契约                                                                                                                                                                                                                                                                                            |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 将获准 Client 的 Create/Start 意图接入现有 Worker carrier，同时不转移放置权限、不重放副作用，也不在 Client 断连时丢失保留绑定。                                                                                                                                                                 |
| 输入   | 严格 OBCS 意图、当前获准 Worker 的身份/代次，以及适配器提供的绝对租约到期时间。Client 字段不能成为 operation、terminal、lease、generation 或 fence。                                                                                                                                            |
| 数据   | Server 为单一本机主体维护至多 1,024 个 create 引用的内存映射；每项保留精确放置、最后授权、启动模式，以及 pending、awaiting-start、started、unknown 或 unavailable 状态。不淘汰，也不提供持久恢复。                                                                                              |
| 输出   | 关联的 OBCS 结果或一个精确 OBCN 命令。缺少 Worker 和约束不满足均无副作用。Start 可跨 Connection 解析成功的 deferred Create；未知或不存在的 create 不得派发。                                                                                                                                    |
| 不变量 | 检查溢出的单调 128 位分配器跨终端和 Worker 代次不复用 operation 身份。每次派发 Start 都递增终端 fence 并使用新 lease。每个终端至多有一个 pending 操作。同步锁不跨 Worker I/O 或 Client 输出。                                                                                                   |
| 决策   | 拒绝已知 create 引用冲突；保留容量已满和并发操作返回 server-busy。已确认 deferred Start 后，相同几何的 Start 直接返回 already-started，不再发送命令；不同几何被拒绝。Immediate create 拒绝 Start。Worker 更换不能重新指派保留绑定。                                                             |
| 模型   | 纯 prepare/complete 状态机在副作用前预留绑定，通过精确结果或明确的交付失败提交。独立的 Connection 输出不阻塞另一 Connection 的下一次操作。                                                                                                                                                      |
| 转移   | Prepare 在提交 carrier 前预留状态。精确且当前的成功结果在向 Client 发布前提交 Host 身份和几何。明确未交付的 Start 失败恢复 awaiting-start；交付不明确或成功结果矛盾使绑定冻结为 unknown。此前已确认的 Host 身份在 Start 失败或结果未知时仍保留。Client 丢失不能取消已提交操作或移除绑定。       |
| 依赖   | 纯协调器消费协议值和显式时间/放置事实。私有服务负责短 mutex 临界区、墙钟采样、Worker handle 选择及一次 carrier 调用。现有 carrier 负责线协议串行化和完整写入分类。                                                                                                                              |
| 归属   | Server 拥有主体策略、create 引用、放置、operation/lease/fence 签发及结果提交。Worker 拥有启动默认值、准入和 Host 副作用。Host 拥有 PTY/VT 和保留执行。不新增公开或跨 workspace 契约。                                                                                                           |
| 验证   | 确定性协调器轨迹检查分配、pending 互斥、精确结果提交、unknown 隔离、容量、几何和更换。Socket/进程轨迹检查跨 Connection Start、串行化、畸形 peer 和 Client 丢失。已登记的启动授权及协调器模型检查各自有限的安全性/进展性质。                                                                     |
| 限制   | 仅限附加、未发布的 POSIX 本机权限。1,024 条记录上限沿用已接受的 Worker 代次限制，生产前需要显式退役机制。租约有效期为四十六秒，来自 carrier 的一个排队及一个在途交换，每次均有两秒 write 和二十秒 result 截止时间；到期可拒绝且不重试。无重放 ledger、细粒度权限、UI 激活、重启协调或生产恢复。 |

协调器的实际消费者是准入后的 OBCS handler。其集成见证是 Client 通过真实 Server、Worker
和 Host 进程发起 Create 和 Start。下一个缺失的用户可见边界是 TUI 激活。

[`LocalTerminalCoordinator`](formal/tla/LocalTerminalCoordinator.tla) 检查一个 deferred 绑定、
两个 peer Connection、三个 operation 身份和两个 Worker 代次。Prepare 对应 IssueCreate/IssueStart，
complete 对应精确/未交付/拒绝/未知/过时代次结果转移；每个 Connection 的独立发布可与另一 Connection 的
操作交错。进展依赖有界 carrier 完成及发布/放弃步骤的弱公平性，不保证已断连 Client 的结果交付。
几何、容量、类型映射与真实 PTY 行为由确定性测试和进程见证负责。

`RejectConcurrent` 使用另一个 peer Connection 的 one-outstanding-request slot 作为有界 correlation
surrogate，排入显式 `server-busy` result，并保留原 reservation。
`PublicationCopiesExactPredecessor` 要求 publication 复制并清除该精确 Connection 的 queued result，同时
保留所有其他 Connection。精确 OBCS request-ID echo 仍由确定性 runtime 与 codec evidence 负责。

## 所有权

该应用负责原生本地 Workbench 权威、Workspace membership、private projection、Worker 与 Client registry、OBCN carrier、terminal-intent coordination 及 Connection viewer/controller relay。保留的本地 Desktop composition 有独立 Workbench writer。生产 Browser ingress 仍是未来能力，本次旧链路退役不引入替代服务。

可导入的 authority boundary 是 `src/lib.rs`；`src/main.rs` 组合私有 binary。穷尽的 lateral 方向为
`local_worker_service -> worker_registry`、`worker_obcn_carrier`、`local_client_service::start`
和 `local_terminal_service`；`local_client_service -> client_connection_registry`、
`local_terminal_service`、`workbench_service` 及其私有 `application_connection`；
`workbench_service -> local_terminal_service`；
`local_terminal_service -> session_coordinator`、`WorkbenchAuthority`、`terminal_coordinator` 值类型、`worker_registry`
和 `worker_obcn_carrier`；`session_coordinator -> WorkbenchAuthority` 和 `terminal_coordinator`；
`workbench_authority -> workbench_projection`；以及 `worker_registry -> worker_obcn_carrier`。
Service 在锁外构造 Workbench 权威。SessionCoordinator 拥有 Workspace 终端创建流程、确切 binding selection 和共享提交边界，
组合两个 core，但不接管它们各自的事实。

终端 coordinator 的私有 `outcomes` 子模块拥有 Worker observation 投影。SessionCoordinator 私有 `tab_create` 子模块通过绑定来源、消耗所有权的令牌组合命令的共同准备与结算；`terminal_selection` 组合 canonical exposure、
保留目标解析和 attachment identity 准备。WorkbenchAuthority 的
`terminal_binding_selection` 子模块拥有只读 canonical exposure query；`terminal_tab_create`
拥有命令接纳、准备后的成员关系、预留账本字节及结算。
`workspace_directory` 子模块分别拥有 WorkbenchAuthority 的目录接纳与规范提交、会话 grant
组合，以及 service 在锁外进行 Worker 观察后重新校验的流程。`terminal_tab_move` 拥有 Move 接纳与
准备后的置换；会话 在目标/无操作判断前校验组合。二者共享命令预检；Move 复用权威的
准备后已结算命令发布路径。
LocalTerminalService 父模块拥有共享熵源
获取；其 `tab_create` 子模块按 registry 再 会话 的顺序持锁固定 Worker placement，在两锁之外
派发，并先结算两个 owner，再返回保留命令结果。共享辅助函数定义在父模块中，子模块不导入兄弟模块。
Service 的 `viewer_connection` 拥有附件 lifetime；
`controller_connection` 拥有终端预留、Connection grant、副作用 sequence 和精确释放；
`projection` 拥有路由与反馈。Client 的 `record_stream` 拥有准入后的 I/O deadline。
反向及其他 lateral import 仍禁止；测试不扩大 production visibility。

Carrier 拥有 I/O、精确 echo 验证和 generation retirement，不拥有 OBCS 或产品策略。
确定性 core 不导入 transport、clock 或 process adapter。Terminal service 操作按 registry 再 会话
的顺序获取锁；grant 操作则按 registry 再 controller map 获取锁。Workbench 访问先校验外部 OBWP policy，
读取及纯导航随后只获取 会话 gate。不透明的 `WorkbenchReadOrNavigationRequest` 排除需要 会话 组合的
`tab.create`、`tab.move`、`tab.close`、`panel.split`、`panel.close`、`panel.move` 和
`workspace.primary-directory.accept`。Move 要求 navigation grant，且只获取 会话 gate；
`tab.create` 还要求 terminal-create grant，并使用上述组合路径。Binding viewer selection 先持有 registry 和 会话 锁，再采样 handle 的
active state；其内部 canonical query 使用 OBCS subject admission，并保持 Workbench 优先的失败顺序。I/O 前释放全部 gate，不增加自动重试。`application_connection` runner 将确切
OBCS/OBWP prefix 分派给不同语义 handler，只导入其契约及标准库 I/O。提交后协调失败会关闭混合
载体，不会伪造命令结果。尚未发布的 application library 暴露权威、会话 grant 和内部
membership/exposure/preparation 的 typed result；整份 projection 写入保持私有。

## Client 多路复用交付

TUI 将已准入的 OBLC v1 连接升级为一条 OBWP v1 carrier。请求和订阅 stream ID 非零、
仅在连接内有效且不复用，与 graphics producer ID 独立。逻辑 lane ID 表示资源作用域，
不是另一个已认证 Client。终端 viewer 与 popup owner 共用 `(connection generation, lane ID)`；
关闭 lane 只释放其 attachment／popup。最后一个 lane owner 释放与显式取消使用同一清理路径。
已执行的完成可能先于取消返回，但排队的 lane 关闭随后回收准确 owner，不能回收兄弟 lane。
首次成功 ClientView Synchronize 绑定发布 lane。只有该 lane 可以同步，关闭时退休视图并结算
待完成控制回执。Synchronize 不能作为可重建订阅；稳定失效提示唤醒 domain 拥有的一次性交换。

每条 carrier 有一个标准库 reader 和一个 writer／dispatcher。reader 最多接纳 64 条排队记录；
dispatcher 执行有限请求，并在 mailbox 和业务锁外写入。最多 64 条活跃 stream 和 64 个显式
lane，另有根 lane 1。保留请求／响应的 reservation 共用 32 MiB 预算，保守计入编码副本。
每条内部记录最多 24 MiB 加 12 字节 header。decoder 存储和当前解码记录另有有界临时分配；
reservation 计数不是精确进程堆上限。增量 decoder 输入上限为 32 MiB，几何容量增长也受该上限
约束；大 frame 出队后，空或最多 16 KiB 的后缀会替换超过 64 KiB 的多余容量，不把大 buffer
作为空闲 scratch 保留。Client 命令队列和输出队列也有固定上限。
预算耗尽返回类型化 Capacity 失败，不会无界缓存。

在该总量内，Client 为应用进展验证预留一条 stream 和 256 KiB；普通消费者最多接纳 63 条
stream，保留记录上限为 32 MiB 减 256 KiB。五秒未收到获接纳的应用进展后，唯一 actor 排队
一个既有只读 `TerminalLaunchRead`。其合法非零目标可以不存在；即便返回 Found，响应也受
OBCS 的 63 KiB payload 加八字节 header 上限约束。Server 按既有三倍记录长度预留预算，
此结果最多计 193560 字节。decoder 和临时解析值仍有独立上界；此 reservation 不是整堆测量。
stream 分配与 FIFO 入队共用用户命令锁，保留递增 open ID。两秒总期限从 probe 创建时开始，
包含排队和写入时间。只有匹配 stream ID、请求身份、实际 handoff generation 且完整合法的
Result 能结算它。其他 stream 和已退休 probe 回复不能续期。尚无在途 probe 时，获接纳的
应用 frame 推迟下一次空闲探测。失败只表示无法确认进展：撤回投影，沿普通连接恢复路径处理，
不重放已提交 mutation。Server 容量耗尽仍返回类型化 Capacity；控制预留不保证穿透过载 peer。
SSH keepalive 本身不代表应用进展。这是每连接一个存活性采样，不是每功能的业务状态轮询。

提交信号先于快照捕获注册。捕获期间并发提交会留下更高的信号 revision，dispatcher 在休眠
前再次求值。状态 stream 先发快照，再发送变化后的完整值；Client 将待消费状态合并为一个值。
失效提示 stream 先发语义快照，再发送合并后的八字节提交提示。事件 stream 在 Client 每条
最多保留 32 条待消费记录。语义 journal 订阅分别保留 Subscribe 定义与 Read continuation，
立即续读 `more` 页，并使用既有 journal epoch／cursor。活跃 stream 独占该语义 cursor；
竞争 stream 或一次性 Read／Close 不能消费它。物理 Host 探测共用每连接唯一 capture permit，
并保留到取消及 join 完成；增加订阅不会增加并发 Host reader job。每个订阅读取失败出口
（包括 capture 采样前的来源解析）均在返回前取消及 join 保留任务，使兄弟订阅可以取得 permit。journal 丢失或 Client inbox 溢出
都会先发送 Gap，再捕获新快照；不把丢失事件冒充为完整历史。连续超限之间间隔小于 30 秒时，
第三次消费者 resnapshot 以 SlowConsumer 终止该连接。每条输出记录写入上限为两秒。
无响应 socket 会关闭；仅在 socket 还能接收完整 frame 时才能交付类型化 disconnect。
其他 Client 和提交发布不等待该 socket。多路复用 dispatcher 不支持长语义 Wait；
直接 CLI Wait 保留原有有界契约。

Workspace metadata 过期为同一 dispatcher 提供准确的 owner 下次期限。维护先取得 Session 锁，
再采样单调时间、提交已到期状态并返回下一期限。dispatcher 在等待前求值相应失效提示；
socket I/O 和等待期间不持业务锁。即便仅 TTL 改变、token 值相等，Report 的通用提交通知
也会重新计算期限。没有 Workbench 状态时没有期限；退休或维护失败会关闭 carrier，
不会不断重试已过去的期限。因此纯 metadata 消费者无需用周期读取驱动过期。

共享偏好快照订阅单独保留一秒的绝对文件采样期限。Desktop 和其他 Session 使用共享
存储的 OS 锁，但没有跨进程变更信号。同一 Server 内的提交仍立即通知 dispatcher；普通载体
frame 和 Host 采样不能提前触发或推迟文件期限。dispatcher 等待 Host 采样、metadata
过期和文件采样中最早的期限；取消订阅会移除其期限。期限表示采样已到期；dispatcher
已有工作和调度可能延后实际读取与交付，因此不保证严格在一秒内观察到变化。Invalidations 模式只传递 Server
提交提示；外部文件采样属于交付语义快照的订阅。不增加 Client 轮询定时器或连接。

每次创建在准入前、于 Session 锁之外读取同一个冻结存储一次，其计划把该
[启动偏好快照](../../crates/capability-node-protocol/README.zh-CN.md#启动偏好快照)附加到 Worker
请求：滚动历史大小、终端默认色，以及默认启动时所选配置的命令。存储缺失、读取被拒或程序超出选择预算时
捕获 `Unavailable`，只拒绝该次创建。快照无法放入一个 OBCN 帧的启动在规划其终端时失败，早于任何
Worker 派发；Workbench 创建此时报告运行时不可用 `ServerBusy`，与插件环境过大时一致。Presentation 在其绑定自身的
修订中以 `launchProgram` 发布该绑定的启动所请求的程序：显式可执行文件、自定义默认程序，或准入它的
Worker 这一代在 hello 中报告的系统 Shell 文件名。该代的 carrier 注册时，Worker registry 把这个名称附加到
该代的请求 handle 上；创建从它规划所针对的同一个 handle 取得 Worker 位置和该名称，因此启动不会用上另一代的
Shell。保存的修改只影响之后的创建，运行中的终端保留其快照。
会话组合根在任何终端启动之前，根据 Server 启动时唯一一次配置读取冻结存储范围，因此从 checkpoint
恢复的终端与之后的创建一样捕获偏好，并共用一次读取。

Client actor 同时 poll carrier 与本地命令唤醒 stream，拥有物理重连、取消、ID 和订阅重建。
交接前取消的请求为未提交；一旦写入可能开始，传输丢失即为 OutcomeUnknown，绝不重放请求。
回执按实际开始写入时的代次关联，包含首次离线排队的请求。重连丢弃旧 decoder／队列，只重建订阅；stream 取消会隔离排队和在途交付。
类型化超限断连为终态，不能通过重连规避上限。逻辑终端 attachment 不通过重放 Attach 重建。
一次性 CLI 连接继续使用直接协议路径。

`ClientSubscriptionDelivery.tla/.cfg` 是新增边界的最小并发模型：两个 Client、队列容量二、
两个发布 revision 和两代连接。独立在途 frame 跨 Cancel／Reconnect 保留，在交付处检查。
模型检查先快照后 update、先 gap 后替代快照、代次／取消约束、队列长度上限，以及发布是否
独立于 Client 队列占用仍可执行。最后一项以非阻塞发布为假设，不证明 OS 调度、socket 进展、
字节／stream 预算、codec、终端所有权或端到端活性。`Overflow` 是必需的正向可达证据。
维护中的负对照将 FenceGeneration 或 FenceCancellation 置 FALSE，并要求对应不变量反例。
既有 relay／所有权模型没有覆盖这个连接／订阅新边界。

`local_client_service/multiplex/tests.rs` 的受控实现轨迹检查捕获期间提交、不读取的 peer 与
仍进展的 peer 并存、事件 cursor 排他、迟到 lane 清理保留兄弟 owner、同值提前 TTL 上报后
的空闲到期，以及维护失败不形成已过期限重试循环。偏好载体轨迹在不发送提交通知的
情况下写入真实共享存储，检查无关请求流量既不加速也不饿死文件采样，然后在 peer
不再发送请求时观察第二次外部写入。Client 传输轨迹检查取消后的迟到
回复、真实事件溢出／gap／快照交付和总 reservation 拒绝。受控空闲 probe 轨迹持续发送
无关推送及旧 probe 回复直至固定期限，随后验证只恢复订阅、不重放未结算 Reload。
配置恢复检查不重放结果未知的
reload。控制轨迹让 UI 样本变化与投递竞态，消费准确 completion 后接纳下一命令。
来源丢失轨迹让已完成 capture 经真实订阅读取失败出口回收，再验证兄弟任务可以启动；
它使用受控 job，不是 OS Host 集成见证。视图生命周期轨迹区分兄弟与发布者清理。协议 codec 测试拒绝不可信长度、版本、身份和 tag。修改这些声明时运行
`node scripts/tla/run-models.mjs --models client-subscription-delivery --jobs 1`、两个
`client-subscription-delivery` 变异用例及受影响的 Rust host gate。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-worker --locked
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --workspace apps/orchbee-server --jobs 1
pnpm verify
```

Viewer 路由测试运行全部 64 个生成种子，并要求覆盖声明的边界。
每条轨迹都经过重放编解码往返，其中一个种子还在新的 IPC fixture 中重放。
其余成功轨迹只执行一次；失败时保留原始种子和轨迹，并按相同失败指纹进行缩减。

Server 模型命令支持共享运行器的有界并发。每个保留配置保持声明的边界、性质及动作覆盖。产品行为变化时仍必须运行实现测试。

relay 核心只有一个配置，期限为 20 秒。它必须穷举完整可达图并满足每个声明的性质与动作证据。完整无缓存 `pnpm test:tla` 门禁（包括工具证据）的预算为 60 秒。

## Workspace 终端创建

会话 从空 Workbench 开始。每个承载终端的上下文都有一个真实 Workspace 所有者，Project 成员关系可为 null。显式 `workspace.launch` 或 `tab.create` 联合准备规范成员与运行时预留，一次提交，并使用该 Workspace 已准入的 Worker／目录启动。精确重放返回保留的待完成／最终结果，不再次启动。

Client 读取规范成员并附着其准确绑定。不存在 `terminal.open` 端点、单例首终端引用或无 Workspace 的回退。空 Workspace 仍可导航；空 Workbench 不隐式创建终端。参见 [Workspace 成员关系](docs/workbench-authority.zh-CN.md#workspace-终端成员模型)、[Tab 创建](docs/workbench-authority.zh-CN.md#workspace-tab-创建模型)及[目录准入](docs/workspace-terminal-directory.zh-CN.md)。

## Windows 原生 会话

Windows 托管 会话 保留现有内存权威和生命周期锁。它准入当前令牌用户的命名管道对端，并禁用句柄继承来启动精确的同级 `orchbee-worker.exe`，使 CLI 输出捕获可独立于保留的 会话 结束。运行时路径、ACL、锁、标记及 cohort 校验仍由 `orchbee-local-runtime` 负责。Windows 观察入口无需 Unix 半关闭即可接收一个有界完整 JSON 值；畸形连接不会中断其他客户端。此入口适配器不移植 Unix provider hooks。可用命令面及剩余限制见 [Client 快速开始](../orchbee/README.zh-CN.md#windows-原生快速开始)。

## 目录工作区启动

原生未发布的 `workspace.launch` 命令创建 Workspace、接纳绝对目录并创建首个终端，全程使用一个请求标识。放置方式为 `ungrouped`、既有 `project`、`new-project`，或位于来源 Workspace 旁的 `alongside`。Alongside 在最终准入时采用来源当前的 Project；来源未分组时，仅将该来源与新 Workspace 放入新建 Project。来源已关闭或正在关闭时拒绝。必须同时具备导航、目录关联和终端创建权限。显式 argv、环境变量及 Launch Profile 复用现有有界启动契约。Client 焦点不属于此操作。

Server 检查准入，释放锁执行 Worker 只读目录探测，再按注册表到 会话 的锁顺序复查请求、修订、来源成员关系和精确 Worker 代次。它在一份候选投影中准备六个结构标识及所需的 Project 标识，预留保留结果与终端准入，然后在一次修订中一起提交 Project 放置、Workspace、Directory、Context 及首个 Tab/Panel/Presentation。准入失败不会发布半成品 Workspace 或 Project，也不会改变来源的分组。终端派发仅在提交后发生；启动待定或失败时保留所有已提交标识，供查询和显式 Close。回执携带最终可为 null 的 Project。精确重放绝不再次派发。响应丢失属于结果未知，不得自动重试。

此细化复用 Tab 创建的提交与结算所有者。验证义务包括权限分离、探测期间的陈旧修订及 Worker 代次、容量与标识边界、精确重放、提交后的启动失败，以及 cwd/环境变量的 Windows 进程见证。不宣称持久化重启恢复、Desktop 支持、焦点变更或 TLA 执行。

## Windows 保留 Worktree 续执行

`workbench.worktree.operation` 接纳 Start、纯 Read 与尝试序号约束的 Resume。Workbench 权威最多保留 64 个操作，全局一个 Pending，并拥有规范 Workspace 关联；惰性维护所有者在 Client 断线后继续执行。registry 到 会话 接纳及发布复查来源和精确 Worker 实例；OBCN IPC 在两道门之外运行。Create/Open 观察 Worker 检出事实后才复用或启动 Workspace。Remove 在 Close 前检查脏状态，保留新 Workspace／目录准入，暂缓规范发布，等待终端释放和清理后才执行 Git 移除。已知 Git 失败会验证原 checkout，并在保留的 presentation 槽位恢复全新的默认 shell；独立的有界恢复计数保留启动失败和未知结果。删除结果未知时保持布局受限，不启动 shell。已知成功后释放发布，最终 Close 重试不重复 Git。Worktree 失败后，显式普通 Close 可不执行 Git 而解除保留。显式 会话 停止在 Close 维护及 Worker carrier 冻结前 join 此所有者。界限及恢复见 [CLI／续执行契约](../orchbee/docs/windows-worktrees.zh-CN.md) 与[共享协议](../../schemas/workbench/v1/workbench-worktree.schema.json)。

Worktree Start 必须包含可为 null 的 `projectId`：仅 Directory 来源可指定既有 Project；
Workspace 来源和 Remove 必须为 `null`。保留结果保留原请求元数据。基于 Workspace 的
Create/Open 在异步准备后需要新建 Workspace 时，按来源当前成员关系采用 Alongside 放置，并拒绝已关闭或
正在关闭的来源。已验证 Git 关联使用独立于 Project 成员关系的稳定 Workspace／Directory
身份，因此重新分组不会丢弃保留的检出事实。

## 只读 Worktree 来源查询

`workbench.worktree.query` 解析显式 Workspace 或字面目录，无需 Project。它在 registry-to-会话 门内捕获来源及 Worker 实例，在门外执行一个有界发现 job，再复查并加入当前 Workspace 标签／身份。多个关联明确保留，不任取首个匹配，不改变焦点、创建 Workspace 或消耗变更回执。共享 porcelain 解析器接受 Windows 绝对驱动器路径及既有路径形式。来源变化、Worker 丢失、元数据无效及预算到期返回类型化失败。参见[所属契约](../orchbee/docs/windows-worktrees.zh-CN.md#按来源发现-worktree)。

## 保留的 Workspace 组关闭

普通 `workspace.close` 仅移除所选 Workspace 及其终端上下文，即使其他 Workspace 与它属于同一 Project 或 Git 仓库，也不会连带关闭物理 Git 同伴或删除 Project 元数据。

`workbench.workspace-group-close` 显式关闭一个 `projectId` 的所有成员。准入按当前 Workspace 显示顺序捕获最多 256 个成员，允许跨仓库，并组合其普通 Workspace Close 回执。其他 Project 的 Workspace 即使共享仓库也不包含在内；后加入的成员不会追加到保留的尝试中。空 Project 也是有效目标。结果保留 Project 身份、尝试序号、状态，以及每个已捕获 Workspace 的身份、标签和 Close 结果。Project 元数据保留到单独执行 `project.dissolve`。组与 Worktree 的新修改在 Pending 期间互相保留执行权。共享维护生命周期适配器拥有各自的唤醒/线程/加入实例；Windows 会话 stop 在 Close 维护冻结前加入两个提交者。[Project 全部关闭契约](../orchbee/docs/windows-worktrees.zh-CN.md#project-全部关闭) 定义上限、部分失败和恢复；[线上 schema](../../schemas/workbench/v1/workbench-workspace-group-close.schema.json) 定义公开字节。组关闭不调用 Git 删除。

## Windows Client 视图路由

传输接纳仍与界面类型无关。已接纳的 Windows Connection 可额外以 Server 分配的 generation 身份发布一个易失交互视图。`client_view_registry` 拥有有界路由、新鲜期及保留的投递回执；`WorkbenchClientConnection` 提供认证所有权和断开清理。规范 Workbench 权威与终端载体所有权保持分离。[Client 控制所属契约](../orchbee/docs/windows-client-control.zh-CN.md) 定义线上协议、绘制后确认、生命周期及证据；确定性 Client-view 生命周期测试覆盖实现。

## Workspace 元数据上报

[Workspace 元数据模型](../orchbee/docs/windows-workspace-metadata.zh-CN.md) 管理既有 会话 锁内的临时 token 值、各来源序号栅栏及单调 TTL。独立的 OBWP 31/32 RPC 以读取权限准入查询、以导航管理权限准入上报，不创建拓扑账本项、不改变权威修订，也不要求终端 Worker。上报在 token／来源／资源容量限制下保持原子性；旧序号不刷新过期时间。重启后状态为空，已关闭 Workspace 的身份在下一次元数据操作时清理。上报和过期发布到语义 journal；已准入 dispatcher 在 owner 最早 TTL 期限唤醒。

Panel 展示上报通过 OBWP 33/34 在 会话 锁内运行，使用读取／导航权限及精确主绑定守卫。[状态模型](docs/workbench-authority.zh-CN.md#panel-展示元数据) 负责按终端保存的临时来源、字段优先级、tokens 和过期。

## 原生插件 action 受理

Workbench 插件执行在解析源文件前要求读取、终端创建和生命周期权限。本地终端服务检查 会话/Worker 身份、已启用清单可用性、action 唯一性和 Windows 平台支持，固定 argv/cwd/context 后仅向 Worker 提交一次。它不拥有子进程。读取页面固定 Worker generation；修改回复丢失时保留未知状态。[原生插件行为](../orchbee/docs/windows-plugins.zh-CN.md) 管理完整契约及尚未接通的生命周期。

插件 Pane 的 Tab／分屏受理在权限、修订号和重放检查后解析目录选择器，再冻结显式程序、插件根目录后备 cwd 及受保护的展示环境。终端规划与完成阶段均将环境与冻结的程序、cwd、普通环境和 Worker 代次进行关联核对。解析拒绝先于规范预留；保留请求重放时不重新读取已变化的清单。在有明确的 Windows shell 包装字面参数契约前，拒绝 batch 入口。

插件 Pane 成员记录针对候选规范快照准备，与终端预留同时提交。有界临时映射跟随精确 Presentation／绑定标识；只读 PaneGet 清除丢失绑定，返回带修订号的事实，Worker 不可用时仍可读取。已退役的 authority 拒绝读取。

## Agent 集成受理

`workbench.agent-integration` 的检查（`list`、`explain`）、poll 与 cancel 需要读取授权。启动
`enable` 或 `disable` 还需要终端创建与生命周期授权，与变更型插件 action 相同。非变更请求不会
触发通用 Workbench 失效。未授权请求回答 `denied`。

已授权请求必须指向当前 Session，否则回答 `session-mismatch`。Server 以新的操作 ID 把请求作为 OBCN
kind 31 中继给当前 Worker，并按操作 ID、job ID 和 action 配对 kind 32 结果。Start 的 job ID 就是它自己的
操作 ID，因此 Workbench job `{workerId, generation, jobId}` 恰好对应一个 Worker job。poll 或 cancel 的
job 指向其他 Worker 或代次，或者 Worker 拒绝 fencing 时，回答 `worker-changed`。Worker `Busy` 和载体背压
回答 `busy`；没有 Worker、`SpawnThreadFailed` 或 `Dead` 回答 `worker-unavailable`。已投递的 enable
或 disable Start 丢失结果时回答 `abandoned`，因为其效果未知。Server 先按契约解码已完成的报告再回答；
无法解码的报告回答 `malformed`。

## Workspace Git 观测

`workbench.workspace-git` 按读取授权查询当前 Worker 实例上的规范 Workspace。
它读取 Workspace 首个终端最新的精确物理目录采样，否则读取已提交目录；此读取绝不附加 viewer。
会话、已提交目录、规范修订和 Worker 代际在发现 job 前后检查；不修改拓扑或元数据报告。
状态头提供分支及本地 upstream 计数，明确保留缺失语义；Git 退出码 128 视为不含分支事实的非仓库观测。
[工作区侧栏模型](../orchbee/docs/machines.zh-CN.md#工作区侧栏模型) 拥有完整消费者和证据边界。

发现交换采用非阻塞高优先级队列接纳及绝对截止时间等待。
WorktreeJob 与 AgentDrain 回复仍由唯一 wire owner 受检，
但放弃的读取完成结果不会因此退役健康 carrier；有副作用的操作仍要求送达结果。
状态读取器不重发超时启动，只为已知 job 身份排队一次非阻塞取消；
无法接纳清理时，物理 TTL、reap 及保留回执仍归 Worker 所有。
读取预算不抢占 mutex 获取或进程挂起，取消接纳不代表已确认物理清理。

### 发现准入与回执归属

`local_terminal_service::worktree_list::DiscoveryGate` 协调三种只读启动：
已提交 Directory 列表（`Start`）、Workspace／字面目录查询（带有 `trust_repository` 的 `Start`）
及侧栏 Git 观测（`StartStatus`）。Worker 仍拥有唯一物理发现 job 和唯一保留回执。
新接受的启动会替换回执，因此 Server 准入覆盖完整 job 生命周期，
包括两次轮询之间的间隔与终态结果消费。

Gate 保留一个活动读取器和至多一个等待的前台读取器，分别代表物理所有者及其
唯一后继；不增加 Worker 容量，也不建立后台队列。侧栏观测只尝试即时准入，
被占用时返回既有类型化 `Unavailable`。已登记的前台列表／查询拥有下一轮；
之后的前台竞争者在派发前收到 `ServerBusy`。

| 状态／事件                     | 转移与归属                                                                        |
| ------------------------------ | --------------------------------------------------------------------------------- |
| 空闲；有效状态／列表／查询     | 在任何发现 Start 前获取唯一 RAII 许可。                                           |
| 活动；首个前台竞争者           | 登记唯一后继，在原绝对截止时间内通过条件变量等待。                                |
| 活动或前台等待；状态竞争者     | 立即拒绝；没有 Worker 请求或保留的状态等待者。                                    |
| 已有前台等待；另一个前台竞争者 | 拒绝且不替换已登记后继。                                                          |
| 等待读取器截止时间到期         | 仅移除该等待者；保留活动 job 及其许可。                                           |
| 活动读取器消费终态结果         | 释放许可；已登记前台后继优先于新状态读取。                                        |
| 活动读取器放弃已准入 job       | 不等待地排队一次精确 job Cancel，然后释放许可；准入失败时由 Worker TTL 拥有清理。 |

会话、registry 与 gate mutex 均不跨越 IPC、轮询延迟或准入等待。
来源、会话 与 Worker 代际仍在观测前后设防；发现不改变组织或终端资源。
Gate 是临时状态，不跨 Server 重启保留。

所有读取器保留原五秒截止时间，包含准入与回复等待。唯一 wire owner 在读取器
返回后仍消费并校验迟到回复。已准入 Start 保留精确 operation／job 身份用于
Cancel，即使接受回复迟到也如此。前台读取器只可在放弃的物理 job 结算期间，
针对明确的 `Busy` Start 拒绝使用新 operation ID 重试；间隔至多 50 ms，
不更新截止时间。Start 和 Poll 共用 101 次请求上限，位于预留的 128 身份块内，
并为取消保留空间。接受的启动、未知结果与 Git 变更操作绝不重放。
物理清理和进展仍依赖既有 Worker TTL、carrier 及进程调度；本策略不承诺
Worker 丢失或前一 job 耗尽截止时间后读取仍能成功。

`worktree_list::tests::discovery_*` 检查有界准入、前台优先和等待者过期。
Windows 真实 LocalStream 的 `worktree_query::tests` 轨迹覆盖：六个侧栏读取器
在轮询之间与两种来源竞争、已提交 Directory 列表／查询共享、活动状态移交、
准入超时、迟到 Start／Poll 回复、精确 Cancel 和明确 Busy 后恢复。
这些受控实现轨迹覆盖本 gate；既有 coordinator／relay／viewer TLA 模型
不建模发现回执归属，也不声称证明此修复。真实终端负载与已部署配套版本行为
需要单独观测。

## Windows scrollback 编辑器 popup 模型

私有配套版本 `workbench.terminal-editor` 请求／结果（OBWP51／52，version1）
支持连接拥有的独立终端 Open、Read、Close。Open 携带准确的规范
context／Tab／Panel／Presentation／binding 来源、同连接拥有的只读 viewer、
十进制完整 u64 内容 generation／revision 及可绘制尺寸，不携带可执行程序、
环境、文档路径或终端内容。当前 Client 焦点准入归 Client view；Server 在创建前、
只读 Worker 编辑器选择之后，重复核对规范成员、连接 viewer／frame、Worker
代际及物理目标。读取权限不授予输入 controller 或 resize 权限。

独立 popup 所有者与 plugin popup 共用；编辑器不经过已安装 plugin 解析，也不进入
规范 Pane 成员。准入在交接前登记所有权。Host 延迟 Create 固定不可变文本，
确认创建后才 Start 独占写入文档、启动编辑器。创建／启动未知时保留且不重复提交。
状态为 Opening、Active、Queued、Waiting、Dispose、Unknown、Closed。
已知延迟目标在 Start 前放弃可直接 Dispose；inventory AwaitingStart 也可处置，
不宣称子进程退出。EOF 将已接纳清理移交现有 会话 维护。来源只读 attachment
保留到最后捕获消费者结束后 detach；popup 进程／文档清理仍归 Host。

串行应用分发器通过本地 RefCell map 在 OBCS 与编辑器 OBWP 请求间共用每条逻辑 lane 的
viewer owner，每次请求一次借用，不存在重入回调或跨线程访问。授权／会话／来源／编辑器
拒绝均在 Create 前发生。相关协议及原生 Windows 管道轨迹验证来源约束、延迟
顺序、字面参数、未知结果、旧 plugin 行为、规范状态保持及物理清理；不构成部署后
GUI、TUI／模拟器或完整 cohort 验收。Client overlay 和公开 CLI 入口需单独验证，
之后才能报告编辑器完整能力。

延迟 Start 准入会更新终端授权／fence。popup 所有者在该准入前锁定，交接前同步
准确的已准备 Start 授权；失败／未知回执及后续生命周期清理保留当前所有者，
不会继续使用过时的 Create 权限。

## Windows Panel 图片所有权

已接受模型和当前实现边界由 [Windows Panel 图形](docs/windows-graphics.zh-CN.md) 维护。规范状态提交立即清理无效图层；已认证连接拥有生产者，共用临时 viewer 观察者保留不确定的 PNG 读取清理责任。UI 像素、能力观察、CLI 生产者和最终组合验收仍各自需要完成。

规范分割比例由原生 Client 共享并保存在 checkpoint v6，catalog v5 负责准入。[检查点契约](docs/workbench-checkpoint.zh-CN.md) 负责版本 1–5 的严格迁移；[共享比例契约](docs/shared-split-ratios.zh-CN.md) 负责比例修改、投影与兼容性。

<!-- documentation-pairing: english-sha256=53a75dbb0cfa52c347337b945f82320c4ebc1780725fd42a183b4c10feb9b1f7 zh-cn-sha256=27b32cde162958be124bb954aacc036c01a9ea1610c23d5628127fc982670b79 -->
