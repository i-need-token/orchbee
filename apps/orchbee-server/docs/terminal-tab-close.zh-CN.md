# 规范终端 Tab Close

状态：已接受的高影响本地 Close 契约，已包含严格 schema 和 Rust/TypeScript binding。
Server/Worker 执行、保留维护、精确会话清理及规范发布已集成，并有确定性载体及 Linux
协议客户端进程证据。原生 Close 控制及成员安装已集成；Server 侧 PanelSplit 与聚合 Close
已有权威单测、有界模型和 Linux 真实终端 witness；
[原生 Session 契约](../../orchbee/docs/native-tab-session.zh-CN.md) 记录 Linux TUI
Close 到 Empty、手动 Create 及保留 shell 重入的进程用例。这些有界检查不证明发布就绪或生产
重启恢复。这份归 Server 所有的参考文档为 Server、Worker、协议及原生 Client 维护者规定生命周期
工作流、跨所有者结算及细化义务。它随这些实现及证据维护。
[Workbench 权威](workbench-authority.zh-CN.md) 继续规定当前规范投影及严格独立成员关系。

## 用户结果与生命周期授权

Worktree Remove 可以暂缓规范 Workspace Close 发布，同时由已有续执行释放所有捕获的终端并完成本地清理。Worktree 所有者在 Close 准入的同一把锁内记录精确操作／Close 关联。调度和直接发布入口均拒绝绕过保留；只有精确的可发布证据才允许 Git 删除。已知 Git 失败保留 Workspace 结构及 Close 栅栏。Worktree 失败后，显式接纳的普通 Workspace Close 可以不执行 Git 而释放保留；Worktree pending 期间的竞争 Close 返回 busy。读取和失败的 Close 准入从不释放保留。Git 删除成功后，先保留单调完成标记再释放发布，因此后续 Worktree Resume 仅完成规范 Close。该边界为恢复保留布局，但不重启 shell。[CLI Worktree 模型](../../orchbee/docs/windows-worktrees.zh-CN.md) 拥有用户可见的部分结果和恢复契约。

### 未绑定及混合范围

Close 先从规范树解析精确 Context／Tab／Panel，再收集已绑定成员。只有当 Panel 是 Tab 的唯一
布局叶子时，才将其视为整个 Tab 范围。已绑定成员数量不会扩大范围而覆盖未绑定兄弟项，未绑定
主项也不会借用已绑定子项。即使没有物理终端，保留命令仍拥有范围地址。此类 Close 无需 Worker
或运行时所有者：保留正常 ledger／revision 额度，沿用排队发布、重放／加入、状态、发布失败及
Resume 流程，不发送 Dispose。

重叠的 Tab／Panel／Workspace 范围是否冲突不依赖已绑定成员数量。迁移对正在关闭的指定 Panel
设置栅栏，包括未绑定 Panel；兄弟结构操作仍遵循既有规则。发布重新证明有序精确已绑定成员及
规范范围，只移除该范围，并根据全部存续绑定协调 standalone 引用。移除绑定集合为空时，保留
既有 Open 引用或 Empty 槽位。范围缺失保留既有类型化拒绝行为。定向权威与 Site 用例覆盖混合
成员、仅子项绑定、纯发布关闭、重放、范围栅栏以及无关绑定的保留。

### Workspace 生命周期扩展

本地 Server 接受规范 Workspace 所有的简单 Terminal Context 中的 Close，沿用 standalone 的
授权、精确绑定/运行时身份、确认、保留 Read/Resume、续行预算以及先清理后移除契约。不增加
线协议字段或 Worker 操作。原生 Workspace UI 不在此扩展范围内。Directory 可用性既不授权
也不阻塞 Close；新的主操作仍要求精确的活动运行时身份。

`WorkbenchTerminalMembership` 和 `WorkbenchTerminalTab` 为既有操作描述严格单 leaf 成员，
为已接受的 PanelSplit/聚合 Close 路径描述两类上下文中的确切树成员。成员关系携带从规范导航派生的
不可变上下文类型，是读取结果而非另一份权威。规范分组、固定／分组 Tab 及子项均可接受。未绑定成员保留在规范树中，运行时成员集合
仅包含已绑定成员，并显式记录主成员身份。除调用方条件分支外，standalone 组合 helper 自身也拒绝 Workspace 成员关系，使未发布库
使用通用类型名后仍保留所有者边界。

Workspace 准入和发布不读取、重定向或结算 standalone 引导槽位。已知 Dispose 和精确目标清理
后，发布重新提取当前目标上下文成员关系，仅移除保留的 Tab/Panel/Presentation/绑定元组。
同级 Create、Rename、Move 及其他上下文变化均保留。待完成的同级 Create 通过原 token 结算并
保留原结果 revision。Workspace 发布不返回 standalone-change 通知。移除最后一个 Tab 保留
Workspace 及 Directory 引用，留下空上下文，不自动启动替补。后续显式 Workspace Create
遵循既有目录校验并分配新身份。

Close 主操作容量和 revision credit 作用于整个 Site：每个非 Closed 主操作在共享 ledger 中
保留一个未来 revision；发布恰好消耗该 credit 并增加一次 revision。准入、别名和精确重放
不推进 revision 或重复清理。Closed 墓碑和不可变确认在后续上下文变化后仍保留。失败或未知
Dispose 不能发布移除；已知 Dispose 但清理阻塞时保留状态，沿用既有显式 Resume 路径。
不增加持久化、远程执行、跨上下文转移或未确立运行时身份的恢复。

`Ctrl+B x` 关闭选中的规范独立 Tab，先以本地化确认明确其终端执行将对**所有已挂接 Client**结束。
取消不改变任何状态。独立的受信本地生命周期授权允许此效果；仅有导航权限或 Input controller
所有权并不足够。只读 Viewer 所属 Client 若有生命周期授权，也可 Close。目标是精确保留绑定，
从不使用序号。Creating/Starting 目标在取得已结算身份前，明确返回效果前 `terminal-busy`。
无法确定活动 Worker 代次或精确 Host 身份时，不允许终止或规范移除。

Closed 表示已知 Host Dispose、精确 Worker/Server 目标缓存清理及规范移除均已完成；不是隐藏
Tab 并留下无法寻址的进程。没有自动替代终端。移除最后一个 Tab 后，原生 Workbench 保持可用的
**Empty**：无选择、Viewer、controller 或来源记录，仍可 Refresh 及手动 `c`。后续裸 Open
是独立操作，可引导新终端。远程授权、未知身份接管及改变 epoch 后的恢复不在范围内。Server
重启持久化属于单独的持久化工作。

本地传输集成将 `manage_terminal_lifecycle` 授权独立于读取、导航、Create、Viewer 和
controller 权限；仅受信本地组合显式设置它。Close 和 Resume 需要此授权；状态 Read 需要
读取授权。已授权请求进入保留的 Site/Workbench Close 权威。Read/Resume 在查找操作前
检查保留 Site 身份：错误 Site 返回 Denied。匹配的 Read 返回保留操作或 NotClosing；
权威不可用时仍返回不可用。此身份检查在发布退役后仍可用，且不生成快照。维护可用性在
保留重放/别名/回执判定之后约束新主操作接受及全新 Resume，不清除旧确认或状态。既有 Create/Move
adapter 将 Close 专属原因、已接受结果及响应信封拒绝为 action 不兼容。

### Workspace 范围 Close

`workspace.close {workspaceId}` 在与成员级 Close 相同的生命周期授权、指纹、
重放/加入优先级及修订额度规则下，对指定 Workspace 的 Terminal Context 准入一次范围级
Close。准入解析独立于 Project 归属的当前 `(workspace, terminalContext)` 二元组，冻结该上下文完整的
有序叶子成员关系——零成员的 Empty 上下文也是有效范围——并预留一份额度及有界保留记录。
开启中的成员级 Close、已准入但未提交的结构写入，或覆盖该上下文的另一个范围级 Close
返回 `terminal-busy`；随后保留记录在发布或终态前隔离该上下文上的后续成员级 close 与
结构写入。相同范围的新命令携自己的固定账本确认加入该操作；完全相同的保留指纹重放其
原确认。

接受返回 `workspace-close-accepted {closeRequestId, workspaceId,
terminalContextId, acceptedRevision}`：主命令请求 ID 即 `closeRequestId`，
`acceptedRevision` 为准入基础修订，与成员级 Close 完全一致。无副作用的
`workbench.workspace-close.read {protocolVersion, requestId, siteId, workspaceId}`
不分配账本条目，返回 `not-closing`、`denied`、`unavailable {reason:server-busy}` 或
`available {closeRequestId, workspaceId, terminalContextId,
attemptGeneration, status, counts, members}`：`members` 按冻结的准入顺序保留，仅携带
每成员生命周期事实——close 修订只属于范围级 `status`——`counts` 对相同的行聚合。
范围查询包含 closed 墓碑，因此已被发布移除的范围仍返回 `available`。
`workbench.workspace-close.resume {protocolVersion, requestId, siteId,
workspaceId, closeRequestId, expectedAttemptGeneration}` 应用相同的代次纪律与预留控制
存储，每个结果均回显范围字段。

当每个被捕获成员均已知销毁且精确清理完成后，发布在一次规范提交中同时移除该
Workspace 及其 Terminal Context，并记录 Closed 墓碑。Project 元数据、全局 Directory 池、
其他 Workspace 的目录关联及 Git checkout 均保留，且任何成员行都不携带 close 修订。

## 条件式空闲准入

`workspace.close-if-idle {workspaceId}` 使用独立命令指纹，沿用普通生命周期授权、重放／冲突、
容量、Site／revision 及完整 Context 成员检查，并共用待处理 create／split 与重叠 Close 守卫。
Site 在持有 coordinator gate 时解析**每个已绑定 Presentation**，包括未绑定 primary 下的子项。
只有绑定范围为空，或每个成员均有精确 `Uncreated` 证据，才准入条件动作。
`TerminalCoordinator::is_definitively_uncreated` 要求唯一准确绑定，其 Create 在 Host 提交前
结算为 failed／unavailable，运行时身份为 `Unavailable(None)`。缺失、歧义、待处理、unknown
或已有运行时的记录均不能证明该事实。保留的 Started 结果、观察到进程 Exited 或 Client snapshot
都不是这项证明。

已解析运行时返回保留的 `rejected {reason:confirmation-required, revision}`，不产生 Close
额度、发布或运行时效果。权威缺失及 busy／unavailable 关联保留既有准确分类。条件 Close 接受后
使用与普通 Close **相同**的 prepared reservation、完整指纹、continuation、maintenance 和
publication。释放 gate 前已安装隔离栅栏；后续 create／split 不能插入空闲预读与无条件 Close
之间。精确重放观察原回执；同一请求 ID 在条件动作与已确认动作间切换会冲突。
普通 `workspace.close` 保持既有语义。

`project.dissolve-if-idle {projectId}` 要求导航授权，在同一 gate 内冻结目标 Project 的全部成员
Context。借用式准入先执行普通授权、重放／容量、Site／revision、Project 和组织冲突检查，
即使 revision 尚未增加，也拒绝**任一**成员 Context 的待处理终端预留，并在接受逐绑定精确
证据前准备有界组织计划。它使用相同的从未创建证明；已有运行时要求确认。接受只删除 Project
元数据并将 Workspace 取消分组，保留全部成员 ID、目录、Context 与运行时，绝不组合 Close
操作。显式 `project.dissolve` 保持兼容。

`ConfirmationRequired` 表示确定无效果，不授权重发。Client 可提供新的、完整且已检查来源的
确认；只有之后的显式确认才以新请求 ID 发送普通动作。revision 漂移、权威不可用及传输不确定
保持独立，不能被转换为空闲或自动重试。有限权威转换和
[条件准入测试](../src/workbench_authority/terminal_tab_close/tests/context/idle.rs)覆盖 revision
不变的待处理创建、接受后的创建栅栏、整个 Project 与仅子项范围、保留拒绝／重放、授权拒绝和
已确认动作兼容性。Site 测试另行负责真实终端记录到这些证据的映射；解析器和权威测试本身
不能证明当前 Host 进程状态或真实 TUI 行为。

这是既有保留 Close 状态机的准入谓词细化，不引入第二个生命周期所有者：已接受的条件 Close
进入相同的预留与发布转换。新 wire 判别值细化完整请求指纹；明确拒绝没有效果，之后仍需另行
审阅无条件请求。已注册的重放与 Close 模型保留各自抽象范围；执行它们本身不能证明具体的
从未创建谓词、完整 primary／child／Project 成员提取、revision 不变的预提交预留、跨语言
解码，或新的 Client 绘制与请求 ID 升级。上述实现与 Site 见证分别承担这些义务，
不声称已知当前进程退出状态。

## Close 命令与保留状态

既有 `workbench.command` 信封及完整指纹
`(requestId, siteId, expectedRevision, action)` 准入
`{kind:"tab.close", terminalContextId, tabId}`、
`{kind:"panel.close", terminalContextId, tabId, panelId}` 及
`{kind:"workspace.close", workspaceId}`。优先级依次为：生命周期授权；精确重放/冲突；
新变更的退役检查；账本条目容量；Site/基础修订/上下文/严格成员关系；精确目标及终端关联；
Busy/运行时身份不可用；资源预留；共同接受。已接受 Close 的重放在 Workbench 退役后仍可读取。
重放没有副作用。

Panel close 在指定 Tab 内解析一个精确叶子成员。当该叶子是 Tab 唯一成员时，命令按覆盖全体
成员的普通 Tab close 准入；否则按仅覆盖该成员的叶子级 close 准入。存活 Tab 上的未知
`panelId` 返回 `PanelUnavailable`；缺失 Tab 返回 `TabUnavailable`。每种限定范围的 close
仍在准入时冻结其捕获的叶子成员关系，经同一套 continuation 销毁并清理每个被捕获叶子，并按
覆盖关系加入或拒绝重叠 close：同 action 且成员集合相同的重放携固定确认加入；已有开启中的
Close 以不同 action 或不同成员覆盖所请求成员时返回 `terminal-busy`。已完成的 Close 仅保留
墓碑：仅当其捕获成员仍留在投影中才与后续 close 相交，该情形属于既有不变量违例并退役
authority；因此幸存 Tab 上已完成的叶子记录不会仅凭同 Tab 地址阻挡后续整 Tab 关闭。因此同一
Tab 上互不相交的叶子 close 可以各自保留主操作。开启中的聚合 Close 同时隔离同一 Tab 的 `panel.split` 与
`panel.move` 准入：两者均以瞬态 `terminal-busy` 被拒绝，而不允许使发布时必须重新证明的完整
有序叶子成员关系失效。叶子级 Close 仅重新证明其自身捕获的叶子元组，因此对两者均不隔离；其他
Tab 上的操作不受影响。

接受不消耗规范修订。其固定结果为
`tab-close-accepted {closeRequestId, terminalContextId, tabId, acceptedRevision}` 或
`panel-close-accepted {closeRequestId, terminalContextId, tabId, panelId, acceptedRevision}`，
回显所准入 action 的字段。主命令
请求 ID 即 `closeRequestId`；`acceptedRevision` 是该主命令的准入基础修订，不是移除修订。
每个别名确认均保留规范操作原始 `closeRequestId` 和 `acceptedRevision`，即使别名使用更新的
expectedRevision；仍保留别名自身的完整请求指纹。Client 验证目标/规范操作及
`acceptedRevision <=` 自己的准入基础修订，不要求别名修订相等。
主命令确认要求与准入基础修订相等。每份规范确认必须满足 `acceptedRevision < u64::MAX`：
主命令接受按照 `r + c + 1 <= M` 预留移除修订。准入基础修订为 `u64::MAX` 的别名仍可确认更早的接受修订。
以当前基础
修订对同一仍可见 closing 目标发起新 Close，会加入主操作并保留自己的固定账本确认，但绝不
恢复尝试，也不预留第二个生命周期所有者或修订额度。新 Close 对已移除目标返回
`TabUnavailable`；保留的重放/状态仍描述旧操作。既有类型化 Denied/Rejected/Unavailable
适用；精确新增的效果前 Unavailable 原因为 `terminal-busy` 与 `terminal-runtime-unavailable`。

`workbench.tab-close.read {protocolVersion, requestId, siteId, terminalContextId, tabId}`
无副作用，使用 Site 读取授权，发布退役后仍可使用。不分配命令账本条目。
`workbench.tab-close.result` 回显读取身份，返回 `not-closing`、`denied`、
`unavailable {reason:server-busy}` 或
`available {closeRequestId, terminalContextId, tabId, attemptGeneration, status}`。
查询包含 closed 墓碑；单调 Tab ID 不会与它们发生别名。读取授权不允许 Resume。

| 状态                                               | 保留含义及允许的后续工作                                                                                           |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `closing {phase}`                                  | `queued`、`waiting-grace`、`waiting-outcome` 或 `disposing`；Server 拥有继续执行责任。信号确认不等于生命周期完成。 |
| `cleanup-blocked {reason,resumable}`               | 已知状态停驻；只有显式 Resume 可为相同身份及阶段重启符合条件的尝试。                                               |
| `outcome-unknown {phase,reason}`                   | Terminate、Force 或 Dispose 可能已跨越 Host 边界。保留成员关系及已知事实；不盲目重发、删除或替换身份。             |
| `disposed-awaiting-publication {reason,resumable}` | Dispose 已知；仅可执行剩余精确本地清理/发布，不再终止。                                                            |
| `closed {revision}`                                | 精确销毁、目标所有者清理及规范移除已提交；此后不可变。                                                             |

封闭原因集合为：

- CleanupBlocked：`worker-unavailable`、`host-refused`、`cleanup-incomplete`、`inventory-incomplete`、
  `identity-conflict`、`continuation-budget-exhausted`、`resume-generation-exhausted`。
- OutcomeUnknown：`transport-lost`、`deadline`、`invalid-response`；phase 标识受影响的
  步骤，精确为 `terminate`、`force` 或 `dispose`；仅查询失败不表示新的未知效果。
- 待发布：`target-cleanup-pending`、`preparing`、`preparation-failed`、`authority-retired`。

不序列化任意错误文本。已关联的 Host 类型化失败保守映射为 host-refused/cleanup-incomplete，
不得声称是无效果拒绝。原生状态轮询在 blocked/unknown 后保留可寻址目标，提供状态 Refresh。
Retry Close 要求已显示的可恢复状态及重新确认。接受回复丢失后通过状态或原命令重放恢复；
别名、读取及命令重放从不恢复执行。

读取结果精确包含 `kind`、`protocolVersion`、`requestId` 和 `outcome`；只有 Available
包含上述目标字段。Read 或 Resume 的 Site 不匹配在查询目标前返回 Denied。
这不改变 Close 命令既有 SiteMismatch 优先级。

## 使用预留控制存储的 Resume

主操作准入预留一个有界控制单元及最后一次已接受 Resume 回执，包含最大编码响应存储。
恢复无需普通账本条目或别名分配。Close 专属控制为
`workbench.tab-close.resume {protocolVersion,requestId,siteId,terminalContextId,tabId,closeRequestId,expectedAttemptGeneration}`；
Workspace 范围控制 `workbench.workspace-close.resume
{protocolVersion,requestId,siteId,workspaceId,closeRequestId,expectedAttemptGeneration}`
遵循相同规则并回显其范围字段。
它使用既有请求 ID 类型、生命周期授权及再次面向所有 Client 的确认。
`workbench.tab-close.resume.result` 回显请求 ID 及规范目标，返回
`resumed {attemptGeneration}`、`stale {attemptGeneration}`、`denied`、
`unavailable {reason:server-busy}` 或 `rejected {reason}`。Rejected 原因为
`operation-unavailable`、`operation-mismatch`、`request-identity-conflict`、`generation-mismatch`、
`not-resumable` 及 `generation-exhausted`。

每个 Resume 结果精确包含 `kind`、`protocolVersion`、`requestId`、`siteId`、
`terminalContextId`、`tabId`、`closeRequestId` 和 `outcome`。即使 Denied 也回显提交请求的
目标字段：失败时不披露或确认既有操作。Resumed 和 Stale 对应已验证操作，不回显预期代次。
[线协议契约](../../../schemas/workbench/README.zh-CN.md#已接受的-tab-close-线协议契约)
拥有封闭 schema、帧 kind 和跨语言一致性；解析器不授权效果。

`attemptGeneration` 与 `expectedAttemptGeneration` 共用一个 Close 尝试代次值域，独立于
Workbench 修订：非零 u64 值，在线上编码为
`1..=u64::MAX` 范围内的规范十进制字符串。初始代次为 1。在所有者门锁内，先检查授权及精确
操作/目标，再应用以下规则：

1. 保留的已接受指纹重放原 Resumed 确认，不改变状态或时钟；同一保留请求 ID 携带不同字段时
   返回 request-identity-conflict。
2. 否则，较旧预期代次返回含当前代次的 Stale；未来代次返回 generation-mismatch。相等要求
   已知可恢复停驻阶段、无已派发且待结算步骤，以及精确目标依赖。不可恢复阶段返回 not-resumable。
3. 经过检查的递增、截止时间准备、运行状态及回执替换组成一次预备提交。资源/时钟准备失败
   返回 Unavailable，控制状态不变。不分配普通账本条目、修订额度或新 nonce 身份。
4. 递增绝不回绕。耗尽保留已知事实，并以 generation-exhausted 拒绝 Resume；blocked 操作
   暴露不可恢复的 resume-generation-exhausted。已 disposed 操作保留其销毁分类及
   generation-exhausted 拒绝。

一个代次、阶段及最后一次已接受回执足以防止延迟重试恢复后续尝试。不承诺所有被拒控制或早于
保留回执的已接受请求均能不可变重放：被替换后的旧已接受请求成为无效果 Stale。原生不会自动
重试明确拒绝。回复丢失可重放同一控制；Resumed 确认该次尝试，当前阶段由状态读取确定。

## Server 继续执行与精确 Host 生命周期

每个可见 Tab 至多一个 Close 所有者，每个上下文最多 256 个 Tab。整个 Site 的主操作与别名
共用 4096 条目、8 MiB 的命令 ledger 上限。一个维护所有者服务当前单个
`LocalTerminalService`/Site。每轮至多准入一个 Worker 步骤，遇背压让出，等待时不持有
registry/Site 锁。轮转选择不保证 OS 公平性。阻塞的回执等待将该 Site 的 Close 步骤
串行化；其他 Client 线程仍可写 Site 状态，并共享既有有界 Worker 载体。Client 断连不取消
Server 所有的尝试。

两个生产 Site 构造路径均在 Client 准入前启动唯一命名维护线程；线程创建失败即阻止启动。
服务保留线程句柄，线程在轮次之间仅持有服务弱引用。既有进程生命周期所有权不新增生产
Drop join 或关闭预算。接受先于生命周期 IPC 提交，返回其确认不等待维护回复；回复交付
不是派发门槛。所有者可用性仅在精确重放/别名/回执判定之后限制新主操作接受和新 Resume。
即使该所有者停止，读取及保留确认仍然可用。

唤醒标志在扫描保留规范操作之前清除，通知在所有者门释放后设置该标志。等待重新检查标志
及最早保留到期时间，因此扫描与等待之间的唤醒不会丢失。轮转游标仅保存请求 ID；全部操作
状态仍属于主 authority 记录。背压使用核心的 250 ms 最早重试时间，不更新截止时间。
token 形成前的身份/租约候选失败使用相同 250 ms 唤醒节奏，每次重试前检查核心截止时间。
`submit_lifecycle` 前保留精确预备步骤 seal；成功入队使用 `admitted_observed_at` 计数，
即使此时已到期也如此。始终等待并结算其回执，任何所有者锁均不跨越传输等待。
KnownDisposed 的 Resume 验证保留旧目标及本地依赖，不要求存活 Worker。

一次尝试有 **30 秒**、**32 次已提交 Host 步骤**，且**查询提交间隔至少 250 ms**。
在主操作原子接受或 Resume 成功提交时采样 Server 单调时间；提交前经检查计算截止时间。
排队、背压、宽限及所有分页消耗同一时间预算。下一步骤前要求 `now < deadline` 且仍有步骤
额度；准入载体时计数。背压只消耗时间，不产生 Host 步骤、不重置时钟、不忙循环。每个
生命周期步骤至多包含一个 Host 请求，不隐藏分页/重试循环。

保留既有独立预算：Worker HostExchange 为 **1s**；Server 载体帧 I/O 为 **2s**，另有
**20s 响应等待**；提交等待 `result.recv()`。30s 尝试预算规定下一步骤准入，不规定 Host join、
调度或总返回时限。到期不丢弃已派发回执，也不跨越它 Resume：先结算其精确已知确认或交付后
Unknown，再于预算耗尽时停驻。Close 不重写载体超时。

Worker 生命周期步骤边界携带产品绑定、Worker 代次、完整 Host 身份、操作/步骤关联及新鲜
授权；Viewer ID 不是生命周期授权。复用 Host Terminate、Force 和 Dispose。Host ACK 携带
terminal ID，其信封提供 epoch 及关联；预备 seal 及完整 OBCN 结果保留 creation/Worker 代次
身份。验证这些实际字段，不要求不存在的 ACK 回显。保留精确生命周期观察，或通过 inventory
获取；仅验证并丢弃事件并不足够。

Inventory 遍历从无 cursor 开始，保持同一 Host epoch/inventory generation。验证有界页面
大小、连续前进的 cursor/代次，并保留有界目标匹配事实。每页、stale-cursor 响应及重新开始的
首页均消耗同一步骤/截止时间/查询预算。过期 cursor 丢弃未完成遍历，仅在剩余额度内重新开始。
变化频繁、耗尽或超大摘要拒绝时停驻为 inventory-incomplete。只有同代次完整遍历耗尽才证明
不存在；部分页面从不证明。身份不匹配属于受栅栏保护的冲突。即使证明原已知同 epoch 目标
不存在，清理/移除前仍须其精确幂等 Dispose 确认。Inventory 成员稳定不证明生命周期新鲜性；
Host 重新验证每次变更。不接管未知身份或新 epoch。

运行中目标：精确 Terminate 确认 → 固定**五秒宽限** → 若无已结算结果则 Force → 精确
Exited/Failed 结果 → Dispose。宽限及已知信号事实跨尝试到期/Resume 保留：不新增宽限，
不重复已确认 Force。AwaitingStart 直接 Dispose；Exited/Failed 仍遵守 Host 结果发布及清理
屏障。类型化 blocked Dispose 可在 Resume 后通过幂等已知状态路径重新观察/重试。模糊交付
按既有失败关闭载体规则停驻 Unknown，不盲目重发或替换。

## 准入栅栏与各所有者清理

Close 接受为目标的新 Attach、**Control Acquire**、Input、Resize 及 Start 准入建立栅栏。
Acquire 创建新的 Host 输入/尺寸挂接：Server 在预留前及安装返回授权前再次检查生命周期门。
Close 前已准入但确认延迟的 Acquire 不得恢复目标租约。已准入流量按既有因果/Worker 顺序
结算；不存在虚构的全局排空或成功 Detach。其他终端保持可用。目标范围的 Close 错误与可能
退役共享载体的既有协议损坏明确区分。

仅 Host Dispose 不会清除 Worker `HostViewers`/`HostControllers` 或 Server controller
预留，各自当前都有八槽上限。Closed 必须释放目标槽位，无需空闲 Client 轮询或 drop，并保留
无关条目。

1. Dispose 前准备有界精确目标清理存储/seal。Worker 验证 Host 确认并先保留 KnownDispose，
   再以不分配路径仅移除精确 Host epoch/Worker 代次下目标的 viewer、controller 及排队
   frame/feedback 条目。仅在清理后返回类型化 Disposed。迟到事件/旧 Detach/Release 幂等结算。
2. Server 验证完整结果身份/关联，保留 KnownDisposed，只移除匹配的
   `ControlTarget {worker,generation,hostEpoch,terminal}` 及精确 controller ID。保留目标撤销
   事实，使旧 `ControllerLease`/`ViewerAttachment` 结算成为幂等本地清理；不得发送过期
   Host 操作、保留已占槽位或删除更新授权。
   Controller lease 的 Drop 首先在短暂的 controller gate 内读取当前预留。若可读预留已不存在，
   或属于其他 controller ID，旧 lease 只在本地结算，不发送 carrier 请求，也不修改映射。
   精确所有者的清理继续在 gate 外走现有传输路径；无法读取 gate 不代表已经撤销。
   该检查不会取消撤销前已经接纳的清理。
3. 保留 registry → Site 锁顺序；controller 门锁仅涵盖短暂精确变更。不跨传输或租约 Drop
   持有所有者锁。本地清理失败保留 DisposedAwaitingPublication(target-cleanup-pending)，
   发布等待。清理完成后发布失败，仅重试发布。

Host 确认丢失时，将受栅栏保护的受影响事实保留为 Unknown。Worker 本地清理后交付丢失，通过
结果/退出交接保留已知效果；Server 仅报告其已观察事实。这些切点均不允许全映射清空、新代次
清理、条目重建或虚构成功 Detach。

Server 会话细化已实现，并由下述载体/进程见证覆盖。Viewer/controller 所有者保留产品
Worker/Terminal ID、Worker 代次以及完整 Host 创建/epoch/终端身份。唯一的 Workbench Close
主记录（含 Closed 墓碑）提供新效果栅栏。精确匹配的 DisposedAwaitingPublication/Closed
提供附件本地销毁证明；Viewer 发出时的 Start fence 属于历史，而维护清理验证主记录包含
fence 的完整已接受运行时。Close 准入解析恰好一个保留绑定：Creating/Starting 为 TerminalBusy；
未知、保留 Host identity 的不可用、缺失或重复关联为 TerminalRuntimeUnavailable。已确认未创建的 binding 遵循下方启动失败移除契约。已知身份还要求当前 Worker
代次。历史 Started 不代表 Host 仍在 Running。

保留既有会话/请求/序号/当前 Worker 检查。新增栅栏拒绝中，Attach 使用 TerminalUnavailable，
Control 使用 ViewerUnavailable，Start 使用 TerminalCreateUnavailable。Start 在既有
AlreadyStarted/GeometryConflict 判定之后、新 fence/身份/派发准备之前应用栅栏。
Registry → Site → controllers 保护 Acquire 预留及返回授权安装；Attach 也在重新验证后于
Site 门锁内安装。失败时保留候选所有权，直到所有门锁释放。已接纳的 Input/Resize 保留真实
结果。精确已销毁 Viewer 的 Detach 将本地不存在结算为 Detached，Poll/反馈变为
TerminalRuntimeUnavailable，且不发送 Worker 清理请求。仅 Closing/Unknown 不允许这种
基于销毁的结算。

Server 清理以保留的规范请求 ID 及完整运行时作为 Dispose 前身份 seal。在同一个不间断
Site 门锁内验证 KnownDisposed，在一个 controller 门锁内捕获并仅移除当前精确 controller
ID，释放该门锁后再标记 authority 清理完成。预留不存在即已清空。该本地切点不分配目标/映射
存储、不执行传输或租约 Drop，也不要求活动 Worker。证明或 controller 访问失败保留
target-cleanup-pending。旧租约结算保留无关/较新 ID，绝不持有 controllers 再获取 Site。
声称交付前，实际八个空闲 Client 的容量释放以及晚到 Attach/Acquire 进程/载体轨迹必须覆盖
维护消费者。

对于新的 Attach/AttachBinding，已有且精确已知销毁的 Viewer 可先本地结算再建立新附件。
缺少完整旧目标证明时保留 AlreadyAttached。先按新请求既有的语法、权限、目标、Worker 和
Close 栅栏检查准备新附件；失败保留旧所有者。仅成功的无效果准备允许取走旧 Viewer，基于
单调销毁证明清除其 Detach 意图并解除 controller 的 Host Release 意图。在所有所有者门锁
之外 Drop 旧值，保留精确 controller ID 移除，然后派发已准备的新 Attach 并执行通常的晚到
安装检查。不发送旧 Host 清理；新附件失败不能恢复已销毁 Viewer。这无需 Poll、显式 Detach
或 Client Drop 即可复用八个 Client ReadySlot 之一。该进程见证与实际 service/carrier 的
八所有者观测配对：Closed 后、任何旧会话动作之前恰好保留七个预留；隐式本地结算本身不能证明
维护所有者已释放该槽。

Linux Server 进程见证保持八个原始已准入 Client 连接，关闭一个目标，观察其精确规范移除，
并在同一连接上创建/挂接/取得全新目标控制权，不对旧目标 Poll、Detach 或 Drop Client。
另一个原始 controller 仍能执行 shell 命令。独立的实际 service/载体见证在 Closed 后、
任何旧会话动作之前观察到七个原始无关预留。载体测试还将 Attach/Acquire 及已经准入的
Input/Resize 持有至跨过 Close，并检查后续受栅栏拒绝的效果不消耗序列或身份。按 Server
测试装置要求准备精确 Host/Worker 产物后，运行 `cargo test --locked -p orchbee-server`。
这些检查覆盖 Server 协议客户端及生命周期组合，不证明原生 TUI 确认、Empty 交互、重启
恢复或已安装 cohort 验证。

## 修订额度与基于当前状态的发布

令当前规范修订为 `r`、已接受且未移除的 Close 数为 `c`、最大修订为 `M`，经检查维持
**`r + c <= M`**。主 Close 要求 `r + c + 1 <= M`，预留一个额度而不推进修订。
每次普通规范写入，包括初始 bootstrap/Create，都要求 `r + 1 + c <= M`。重放、无变化及
Create 终端结果结算不消耗修订。最终 Close 以 `(r,c) -> (r+1,c-1)` 消耗自己的额度；
别名/Resume 不消耗第二份。Unknown 保留额度及可寻址所有者。其他写入可继续，但不得占用它。

Host 效果前，预留主命令及最大编码 command/ack/status/tombstone 费用和 Resume 单元/回执，
涵盖最大修订表示及每个有界原因。预先构造固定确认/分类存储。别名预留自己的固定账本条目；
所有写入计算已预留条目/字节。读取不消耗，最终完成无需后续账本插入。序列化字节预留不证明
分配、panic 或崩溃原子性。

保留预留量按实测计算，绝不使用统一费率：以一次经检查的加法合计最大合法 wire 单元
——主命令加确认、最坏合法状态文档（范围级 Close 的文档还须按每个被捕获成员各一行
最坏拼写加分隔符计费）、最坏 Resume 请求及最坏 Resume 结果——加上账本条目的已知单元
（`WorkbenchRequestId` 键头及其自有缓冲区、容纳记录的 `RetainedRequest` 内联枚举单元
——各计恰好一次）、已提交记录的实际受控堆（装箱记录本体、各自有身份缓冲区、
continuation/target 两个 `Vec` 按元素大小计的容量，以及每个被捕获成员一份有界生命周期
请求负载），以及整个记录生命周期持有的两个同形态最大回执负载槽——这是覆盖已存回执与
新候选共存窗口的刻意容量代价，仅随记录离开账本时释放。只有已知类型负载计入：`Arc`
引用计数头、`BTreeMap` 节点结构、分配器元数据及 RSS 按约定留在受控负载预算之外；上述
键头与枚举单元正是节点内被计费的已知类型单元。叶子级或聚合成员级 Close 对自身捕获
成员应用同一公式；Empty 范围 Close 仅计固定份额、零成员负载。有界向量与记录先于账本
容量检查构造并实测，因此在双重失败角落——账本已满且 continuation 准备中经检查的截止
时间溢出同时发生——报告的是准备失败而非账本裁决；该角落实际不可达，不为其设专门机制。
Resume 在任何状态变更前实测真实候选回执：它必须装入其预留负载槽，且保留记录——含既
有回执——加候选必须仍在原预留量内；超预算返回 busy，绝不视为格式错误。该总量含 wire
计费，比较上比纯堆界更宽——实际恰好：每个候选装入其槽，每个已存回执在自身准入时亦装
入同一槽，其余堆单元在提交后不可增长。

先保留 KnownDisposed 并完成精确本地清理。在 Site 门锁内验证当前适用的成员关系及精确目标
元组，再从**当前** snapshot 准备：聚合 Close 将整棵捕获树的每个 leaf 作为一次 Tab 发布移除，
保留其他 Tab、顺序及标签。叶子级 Close 恰好移除其捕获的叶子：该叶子的父 Split 被兄弟子树
替换，兄弟自身的 slot 与 Split 身份保留，存活 Tab 保持其 ID、标签及剩余成员；当被关闭叶子
已是 Tab 根时，与聚合 Close 一样移除整个 Tab。释放门锁要求重新验证/准备。旧准入 snapshot
不替换后续写入，不跨宽限冻结整个 Site，额度也不预先指定最终修订。

检查实际编码投影限制及 delta/full-snapshot 回退；不假定已预分配未来完整缓冲区。准备失败
保留销毁、成员关系、额度及可寻址状态，既不是效果前拒绝，也不允许再次终止。不变量引发的
Workbench 退役仍允许读取清理状态。成功准备后，成员关系、活动 Site 关联、额度消耗及 Closed
共同提交，不再执行可能失败的工作。响应丢失不撤销该提交。

## 跟随当前引用的 Site 完成

本节引用重定向仅适用于 standalone 成员关系。Workspace 发布提交规范移除，不进入此槽位结算路径。

保留 `Empty | Creating(reference) | Settled {reference,outcome}`，保留历史 Terminal
创建结果/已关闭身份，不复用 ID。Create 完成时，为每个引用保留精确独立结算值，不从当前有损
终端阶段重构。Closing/Disposed 事实与原完成分类分离。既有原生规范 Create 使用立即 Start。

若活动引用在移除后存活，保留它。否则选择**当前移除后顺序**首个成员：无成员 → Empty；
Creating B → Creating(B.reference)；已结算 B → Settled(B.reference, retained B outcome)。
Closing B 在自身移除前仍是可见候选引用。树形组合要求每个剩余 leaf 都匹配保留 binding；
当前引用仍只为 standalone 重定向选择一个 anchor。

B 的原 Create seal、请求、元数据、绑定、历史 `is_first` 及重放结果保持不可变。在共同门锁
内验证其精确 Workbench token 及 Terminal 关联，并结算两者记录；**仅当 Site 当前 Creating
引用为 B.reference 时**同时结算 Site，否则保持不变。历史 `settles_first` 不拥有当前结算
或通知权威。向 service 返回显式 `standalone_changed`，释放锁后通知。这覆盖 B 先完成再移除
A，以及先移除 A 再完成 B。Close 期间其他 Create 继续；关闭 Creating/Starting B 仍为 Busy。
Empty 后新 C 使用新 ID；旧完成同时受 seal 及当前引用检查保护，无法覆盖 C。

Site 细化在每个 Terminal 绑定上保留 `Option<CreateOutcome>`：精确 Create 完成只写入一次，
后续 Start/生命周期阶段不覆盖。None 仅在 Creating 中表示待完成；其他阶段缺少原结果属于
协调失败。共同 Create 完成返回 `CompletedTerminalTabCreate {result, standalone_changed}`，
同时保留历史首成员关联检查。预备 Close 重定向 helper 仅借用 Site 槽位，消费权威计划的当前
移除前成员关系、预备移除后成员关系及精确移除成员。准备下一槽位前，验证移除前组合、唯一
精确移除元组、严格移除视图及所选绑定唯一保留引用。两个所有者提交在同一 Site 门锁内保持
不可失败，通知在释放门锁后发生。维护 finalizer 已消费该 helper。实际载体轨迹覆盖 B
分别在 A 的共同 Close 发布之前及之后完成，且两种顺序下裸 Open 都选择存活的 B。

## 原生移除与屏障组合

相同修订 snapshot 仍须完全一致。更高修订可移除 ID；存活 ID 保留不可变元组/绑定/
Presentation 标签，Tab 标签/顺序可变。叶子 close 同样表现为更高修订的收缩：存活 Tab 保持
其身份并恰好减少一个叶子，被移除叶子的父 Split 由兄弟子树替换。原生刷新仅在候选是冻结
基线树的精确兄弟替换时才接受该收缩；任何其他树形变化、存活成员变更或多叶丢失均属分歧。
若被收缩的叶子正是已挂接选择，原生会话退役；存活的已挂接叶子保留其挂接与视口。可用的
更高规范 snapshot 证明成员移除，无需为每个
消失 Tab 查询状态。对自身已知 Closed，从 `Closed.revision` 起的每个 snapshot 都必须缺少目标。

| 观察                                    | 必需结算                                                                                                                                                           |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 非活动成员移除                          | 仅丢弃其来源记录，保留活动 Viewer/controller/代次。                                                                                                                |
| 活动/期望成员移除                       | 发布另一选择前，结算或明确失效旧挂接。选择旧序号处存活成员，否则最后一个存活成员，否则 Empty。新 Attach 失败为可重试 selected-unavailable，绝不隐式 Start/Create。 |
| Closed 后续 Snapshot 不可用             | 通过 worker 交接保留已知 Close，并保留可恢复投影/发布状态。                                                                                                        |
| 最后移除及手动 Create                   | Empty 没有 Ready 证明。`c` 以 Empty 来源捕获尺寸；提交前失败恢复 Empty，提交不 Detach 虚构 Viewer，类型化创建投影/结果规定选择/Attach。                            |
| Empty Refresh 看到其他 Client 的 Create | 通过新 Attach/重试选择首个规范成员，不重复 bootstrap/Create。                                                                                                      |
| 精确 Create 修订 Snapshot               | 要求原基线加末尾新成员；缺少旧/新成员属于分歧。                                                                                                                    |
| 更晚 CreateSnapshot                     | 旧/新成员可能已关闭。保留已知 Create；新成员存在则选它，否则按其原追加序号及相邻/Empty 规则结算为 superseded-by-removal。旧 Ready 锚是历史清理数据。               |
| Switch 目标/旧成员移除                  | 保持挂接代次清理顺序，在未结租约期间冻结缓存。明确 already-gone 可结算清理，模糊状态不可。用新 key 选择当前存活成员，绝不恢复旧 ID。                               |
| Move 结果/恢复观察移除                  | 精确 Move 提交修订必须有目标；更高修订可移除。保留 Move 提交并结算移除。提交前 TabUnavailable 同样要求权威协调。                                                   |
| Refresh/其他在途操作早于移除发布        | Closing/DisposedAwaitingPublication 仍可能是挂接不可用的成员。保留身份/状态，直到有效 snapshot 确认移除。提交前恢复不能撤销已观察移除。                            |

移除跨越活动 Create/Switch 屏障时，既有已捕获的前向 **45s**、总计 **50s** 截止时间不变。
已完成 Refresh/Move/自身 Closed 传输放置或显式恢复后，若验证过的候选需要新挂接协调，
在安装候选时恰好捕获一次新 **50s 总计** setup 预算：**45s 前向**，最后 **5s 预留精确清理**。
它涵盖新 Attach 前的精确旧清理，遵循既有 Started 恢复策略。进展或 Busy 从不续期。仅非活动
成员移除不改变挂接，不需要 setup 预算。及时类型化 UI 放置结束传输截止时间；UI 消费没有
合成截止时间。确定性测试时钟建立顺序，不证明 OS 时序。

结构在途操作保留精确消费/清理栅栏。等待一个 Client 的 Close 不冻结其他 Client 的规范写入。
不声称跨读取新鲜性或无界屏障定理；重复准入及迟到确认需要组合实现轨迹。

## 模型与实现依赖图

上文 Close 行为仍是合约。其 Server、Worker 和原生流程模型已退役；以下所有者测试套件提供具体实现证据。它们不穷尽全部交错顺序，也不证明 Host 清理最终成功。

| 所有者 | 实现证据                                                                                                                                                   | 必需主张                                                                                                                    |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Server | `src/workbench_authority/terminal_tab_close/tests`、`reservation/tests.rs`、`continuation/tests.rs`；`src/local_terminal_service/tab_close/tests`          | 预留 revision/ledger 额度、准确别名和 Resume 代次、保留迟到回执、基于当前状态移除、准确 Create 引用结算、发布失败及目标围栏 |
| Worker | `src/terminal_lifecycle_operation/tests.rs`、`src/host_connection/target_cleanup/tests.rs`、`src/terminal_authorization/lifecycle/tests.rs`                | 准确请求/目标/epoch、模糊与已知效果、缓存和预留边界、迟到 Acquire 与准确旧租约清理                                          |
| Native | `src/terminal_session/state/tests/tab_close`、`src/terminal_session/mailbox/tests/tab_close.rs`、`src/local_client/tests/workbench/tab_close/transport.rs` | Empty 与剩余选择、attachment 代次围栏、排队结果、结构屏障、别名、固定截止时间及畸形/部分传输交接                            |

路径相对于各自所属应用；Server 的 reservation 与 continuation 路径位于同一 `terminal_tab_close` 所有者下。从仓库根目录运行：

```sh
cargo test --locked -p orchbee-server -p orchbee-worker -p orchbee --all-targets --all-features
```

集成仍需组合轨迹：两种 A-close/B-create 顺序；Closing/Unknown 幸存引用重定向；Empty 后新建 C；拒绝 C 之后的旧完成；移除后的 pending/final 重放；屏障期间跨 Client 移除；以及结果/UI/退役交接。授权检查、完整身份与指纹、准确编码边界、zero/noncanonical/max/overflow 代次和封闭失败原因仍是实现义务。

单次 continuation 提交不能证明完整 inventory 或 Host 生命周期。Known-blocked 与 Unknown 仍须区分；准确的迟到 Disposed 回执在过期后保留。Unknown 禁止基于 disposal 的清理或重试，但允许合法的准确租约清理。发布前必须完成 Server 清理。读取/重放保持无效果。初始准入在代次一捕获成员；只有符合条件的 Resume 可增加一个代次且不改变该成员集合。

测试必须覆盖满 ledger 的 Resume、过期/重复/耗尽控制、revision 预留、inventory 变动与完整末页缺失、全部八个 idle-Client 槽、保留的 standalone 结果以及锁后的 `standalone_changed` 通知。真实进程证据必须覆盖其他终端存活时的优雅后强制后代清理、最后 Close 后手工 Create 或裸 Open，以及实测 30s/32-step/250ms 计量。确定性时钟建立顺序而非 OS 计时。未执行的平台证据仍未验证。

纯原生传输验证可先于结构 flight 集成；它不授权任何生命周期效果、重放、重连或恢复。未发布的本地组件组共同启用及回滚。回滚不能复活已终止执行或丢弃保留的清理事实。Server 重启持久化、永久 Host/平台恢复、变更 epoch 接管、远程操作、打包及无条件最终清理仍在范围外。

## Worktree 移除失败后保留 Workspace

Worktree 所有者只能取消自身精确持有且已静止的 Workspace Close 的规范移除。
全部捕获的终端目标必须已经完成销毁和本地清理。取消不会撤销这些效果：保留的 Close
以不可恢复的 `workspace-retained` 原因报告 `disposed-awaiting-publication`，
保留全部旧绑定墓碑，并使规范 Workspace 留在原处。它不能发布移除或恢复执行，
也不再限制 Context 结构或参与新的 Workspace/Tab/Panel Close 查找。
后续普通 Close 捕获当前成员，并拥有独立身份。

Close 成员关系包含全部已绑定的主、子 Presentation，也包含分组／固定 Tab。恢复保留捕获时的顺序成员及其精确身份。

持有期间，已领取的 Worktree 尝试可以为一个精确捕获的 presentation 准备替换绑定。
原 Context、Tab、Panel、顺序、标签和分割比例保持不变。每次替换同时生成全新的
Presentation 身份和终端绑定，存活 Presentation 绝不改变其绑定。
重新绑定要求当前旧绑定完全一致、新绑定位于同一 Worker、Close 已静止，
且保留的操作和尝试匹配。准备阶段在提交前验证完整的新投影。
过期尝试、已替换的槽位、重复终端绑定或无关 Close 均不能修改投影。
Site 组合在派发默认 shell 启动之前，将此投影与新的 TerminalCoordinator Create 预留一起提交。

集成恢复所有者在已知 Git 失败后，先验证原 checkout，再启动任何 shell。
它使用已验证的 checkout 目录、默认 shell、空自定义环境和初始 80×24 几何尺寸；
后续 Client 调整尺寸仍为权威。它不会重跑旧 Agent 或程序。
已返回的 Git 失败、验证失败、每个新启动结果和部分恢复是独立事实。
删除结果未知时不启动恢复；启动结果未知时保留该事实，绝不自动重复启动。
取消旧 Close 不代表每个替换 shell 都已启动，这一区别由 Worktree 回执负责表达。

核心证据覆盖空与非空静止范围、清理未完成、过期或外部所有权、重复绑定、
重复替换与取消、布局身份不变、旧绑定限制、直接发布和 Resume 拒绝，
以及取消后的全新普通 Close。Site 和 Windows 驱动证据进一步验证预留先于派发、
代际限制、已知失败后的验证、部分启动和重试行为。这些是有界测试；
完整平台验收仍是独立义务。此流程不另设 TLA 模型。

## 启动失败后的移除

包含已确认 Create 失败成员的 Tab、Panel 或 Workspace 仍可关闭。TerminalCoordinator 在 Site gate 内、Close 准入前解析全部捕获的 binding。恰好一个 binding 处于 `Unavailable(None)` 且保留 failed/unavailable Create outcome，才能证明没有提交 Host terminal；仅缺少 identity 不足以证明。Host Create failure 发生在不可失败的 terminal commit 之前。Start 和 lifecycle failure 不构成该事实。

`WorkbenchTabCloseMemberRuntime` 携带精确 runtime observation 或精确的 `Uncreated` 产品 binding。每个捕获成员仍保留在有序 Close record 与 reservation 计量中。Uncreated 成员没有 runtime identity，直接进入 publication；它不发送 lifecycle 命令，也不进行 controller cleanup，因为从未有成功 identity 可用于准入 Viewer/controller。此处 `disposed-awaiting-publication` 表示已证明的不存在，不声称收到 Host Dispose acknowledgment。混合 scope 仍须在任何准入效果前解析全部 runtime 成员。maintenance owner 仍为必需，但全部为 Uncreated 的 scope 无需连接 Worker。

Creating/Starting 仍为 busy。Unknown、保留 identity 的 unavailable、缺失或重复 binding、矛盾的 accepted Create facts 仍为 unavailable。既有 membership fence、request replay、publication revision、失败停驻与显式 resume 规则保持适用。Publication 重新验证完整有序 scope，包括 Uncreated 成员。测试覆盖真实 failed-Create settlement、无 Worker 移除、混合 scope 与 unknown-outcome 拒绝。

## Workspace 组接纳

普通 Workspace Close 对有关联组成员的已知主 checkout Workspace 返回 `workspace-group-close-required`，沿用相同的拒绝/重放规则。独立的[Project 全部关闭 owner](../../orchbee/docs/windows-worktrees.zh-CN.md#project-全部关闭) 冻结作用域，再将各子操作交给此生命周期。内部接纳令牌必须匹配保留操作、已领取的尝试、精确规范成员和前序已关闭成员前缀；线上 action 不包含绕过标志。子 Close 仍独自负责终端处置、代次围栏恢复及规范移除。组聚合读取携带精简子操作观测，现有 Workspace Close 读取继续保留完整终端成员。

## 完整 Presentation 成员关系

Panel、Tab 和 Workspace Close 按规范 Tab／树顺序捕获全部已绑定的 Presentation，每个 Panel 先主 Presentation、再按顺序排列的子项。分组／固定 Tab 同样可关闭；这些元数据不改变终端生命周期权限。Panel 范围同时捕获其主项与全部子项。只有该 Tab 没有其他不同 Panel 时，才升级为整个 Tab 范围，与目标 Panel 包含多少 Presentation 无关。

每条捕获的 binding 独立经过现有的运行时解析、预留、销毁与清理续作。全部目标结束前禁止发布移除。未知效果保留既有隔离／恢复语义，不能遗漏尚未处理的子项。发布前，所有者再次将完整、有序的现存范围与捕获的身份、binding 比较。新增、删除、重排或替换子项均导致组合失败，保留已知销毁事实且不移除规范范围。其他 Panel／Tab 及其元数据原样保留。完整 Tab 发布仅在移除后源组变空时删除该组；所有无关组均保留，包括其他 Client 新建的空组。仅移除 Panel 时不清理分组。这有意不同于会清理所有空组的 GUI `TabModel.closeTabs`：生命周期发布不能扫描最新 Snapshot 并删除无关数据。Standalone 协调同时移除所有捕获 binding，再选择精确的存续引用或空状态。

完整成员遍历与只读注册共用，发布时在预备候选上调用。存在未绑定 Presentation 时仍拒绝整个捕获，不能过滤丢弃。其他结构操作和原生 TUI 导航保留各自准入契约。不增加命令、线路字段、生命周期回执或权限。定向证据覆盖多 Presentation 的 Panel／Tab 范围区别、完整 Tab／Workspace 捕获、部分目标完成、未知效果、成员篡改、存续元数据和 Standalone 引用组合。完整 Windows CLI／TUI 验收仍属于后续组合门禁。
