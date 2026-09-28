# 原生 Workbench 权威

[重启检查点契约](workbench-checkpoint.zh-CN.md) 拥有保留配置和新权威重建；它本身不启用磁盘
持久化，也不恢复终端进程。

## Close 权威与本地集成

[规范终端 Tab Close](terminal-tab-close.zh-CN.md) 拥有已接受本地生命周期契约：规范移除前须有
已知 Host 销毁及精确目标所有者清理，预留修订和账本/Resume 容量，基于当前成员关系发布，
并跟随当前引用结算 Create 而不修改历史 token。清理期间其他规范写入可继续。最后移除使
Site 成为 Empty；原生 Empty 仍可用，后续裸 Open 可引导新身份。不增加显式空 Open 响应。

Server 库实现了下述保留权威和续接内核。Site、精确 controller/Worker 清理及进程生命周期
维护所有者将其与本地 Close/Read/Resume 入口组合。确定性及 Linux 协议客户端进程测试覆盖
credit、保留控制、当前快照删除、空闲预留释放以及八个活动 Client 连接。Close 记录另有
原生确认及成员关系/Empty 运行时的 Linux TUI 进程证据；仅凭这些 Server 检查不能证明它们。
已安装 cohort 的收敛仍未完成。

## Close 权威运行时细化

已接受的实现切片在现有命令账本中用单个 `CloseRecord` 保留每个主 Close。它拥有不可变命令、
固定确认、完整独立成员及已验证的 Worker generation/Host 身份/fence、尝试 generation、最后一次
已接受 Resume 回执和预留序列化计费。别名只保留自己的命令及主操作的固定确认。嵌套的
`CloseContinuation` 独占阶段、尝试 deadline、已提交步骤、查询间隔、已确认信号/grace 事实、
有界 inventory 遍历，以及已准备/已接纳的未结算步骤 seal。Service 代码只携带临时 token 和调度
机制；不保留另一份可变 Close 状态或 generation。
无副作用 binding 查询包括已关闭 tombstone，并隔离新的目标效果。该布尔值不是精确清理权威：
清理还须比较保留的 Worker generation、完整 Host 身份和 binding fence。原始 launch/Start
重放保留既有优先级。

Close 接纳依次检查生命周期权限、精确保留重放/冲突、退役、账本条目容量、Site/base/context
和适用的严格或树形成员关系。Site 在目标解析前验证组合。可见的既有主操作在任何新运行时查询前被加入，
包括 Worker 现已不可用的 Unknown 或已 Dispose 主操作。新主操作需要精确运行时关联，并在联合
接受前完成全部预留。Close 重放在退役后仍可读取；普通命令保留既有退役优先预检。状态读取和
Resume 使用保留的主操作，包括已关闭 tombstone，不插入普通账本。内联 Resume cell 只有在
generation、可续接性、未结算步骤、精确依赖和 checked deadline 准备成功后，才替换最后一次
已接受指纹/响应。

Credit 通过扫描账本中非 Closed 主记录派生，扫描最多 4096 个条目；没有第二个可变 credit
计数器。每次实际普通 revision 推进，包括 bootstrap、Create 和 Move，都检查
`r + 1 + c <= u64::MAX`；主操作检查相同不等式但不推进 revision。No-op 和终端结果结算不消耗
credit。主记录存储预留命令/确认、状态/tombstone 以及 Resume 请求/响应的最大编码计费，覆盖
最大十进制 generation/revision 和每种有界分类。Tombstone 保留该计费。这些字节限制不证明
分配、panic 或崩溃原子性。

已知 Dispose 和精确本地清理后，借用式发布计划根据当前适用的成员关系验证完整保留目标，并准备
当前投影、编码 delta/完整快照回退及最终 Closed revision/状态。对于聚合树形 Close，它捕获有序的
`exact_removed_members`，并向 Site 的分离 slot 专属重定向准备暴露移除前后的成员关系。两个计划
在同一 Site gate 下提交，不再执行可失败工作。丢弃准备保持成员关系和 credit 不变；失败保留
已 Dispose 分类，只允许剩余清理/发布。续接内核跨过期保留未结算回执，显式 Resume 保留信号、
grace 和查询事实。

Authority 测试负责具体账本/编码边界、别名/重放优先级、最大 revision 和 generation、回执
替换、准备回滚及当前快照删除。续接测试负责时钟/步骤/查询边界和精确回执结算。声明 Close
交付前，Site 和进程测试必须证明联合关联、清理及实际 carrier/Host 边界。既有有限 Close 模型
仍只是这些运行时主张的有界抽象。`cargo test -p orchbee-server --lib --locked` 运行这些
权威/续接测试；`cargo clippy -p orchbee-server --lib --locked -- -D warnings` 检查库实现。
这些聚焦检查不替代根门禁或所属跨进程证据。

## 已接受的 Panel 分割与聚合 Tab Close 细化

`panel.split` 是第一项已接受的树形变换。Workbench 验证确切的上下文、Tab 和 leaf Panel，限制
变换后的树大小，并在派发第二个 terminal launch 前提交源 leaf、新 leaf、split 方向、两个 slot
identity 和 presentation identity。Server 与 Site 将所有 leaf binding 作为一份确切的树成员关系保留；
任何缺失或 Unknown 的 leaf 结果都会隔离待处理操作，不能发布部分树。

聚合 Close 捕获 Tab 的完整有序 leaf 成员关系。它在保留的生命周期权威下处置并清理每个 leaf，
只有全部 leaf 都有已知 proof 后才发布一次 Tab 移除。`panel.close` 是叶子级对应操作：它恰好
捕获一个 leaf 成员，复用同一 continuation，并发布存活 Tab——被移除 leaf 的父 Split 由兄弟
子树替换；关闭 Tab 的唯一 leaf 按普通聚合 Close 准入。Standalone 重定向会以剩余树校验确切移除集合，
保留仍存在的当前引用；没有剩余成员时发布 Empty。当聚合 Close 在某 Tab 上处于开启状态时，指向
同一上下文/Tab 的 `panel.split` 与 `panel.move` 准入均以既有的瞬态 `terminal-busy` 拒绝：该
Close 发布时需重新证明其捕获的完整有序 leaf 成员关系，任何同 Tab 成员变更都会使其失效。隔离
检查位于授权、重放/冲突、退役、账本容量、Site、基础修订与上下文校验之后；它不留存结果也不派发
terminal，Close 发布后同一请求可按原样重试。叶子级 Close 仅重新证明自身 leaf 元组，对两者均不
隔离；其他 Tab 的写入照常进行。权威单测、Linux process witness
`real_workbench_panel_split_and_aggregate_close_releases_both_terminals` 和
`real_workspace_panel_split_and_aggregate_close_releases_both_terminals`，以及有界
`WorkbenchPanelSplit` TLA+ 模型覆盖这一细化。Client-local panel layout 和 focus 已由原生 TUI
通过严格 projection 和本地焦点消费这棵树；它把对端 leaf close 接受为对其基线树的精确兄弟
替换收缩；若该收缩在 Tab 存活期间移除了本 Client 已挂接的 leaf，会话进入通用的
membership 清理/安装路径以附着同一 Tab 的存活 leaf，而非直接退役：附着成功则正常恢复，
附着不可用时可驻车于 `RetryableAttachUnavailable` 并保留已知 commit，协议或 deadline
错误仍按共享的 fail-closed 规则退役。TUI 现以临时 `Ctrl+B` 快捷键提交
`panel.split`、`panel.close` 与 `panel.move`，待输入设计裁决后可能重绑。

`panel.move` 是纯 projection 对应操作：它在 Tab 内重定位一个 leaf Panel，不涉及任何生命周期。
准入复用 navigation 变更授权、确切上下文/Tab 校验，以及树成员关系——Split 树所需的 leaf 级成员
关系——随后校验两个 leaf 都是指定 Tab 的直接成员。若指定 Tab 上存在开启中的聚合 Close，本可
提交的移动以瞬态 `terminal-busy` 拒绝，而拒绝与无操作 `Unchanged` 结果保持原有优先级；叶子级
Close 不设隔离。源 leaf 与目标同名，或这对叶子已是匹配方向
Split 按隐含次序排列的直接孩子（布局已满足）时，提交 `Unchanged`，不消耗任何身份。实际移动的
提交会摘除源 leaf——其父 Split 折叠为兄弟子树——并将其移植到目标 leaf 旁的一个 fresh Split 下，
消耗一个 fresh Split 身份和两个 fresh Slot 身份；源 leaf 保留完整 Panel 节点、Presentation 与绑定，
所有未触及子树保留全部身份。该操作分配三个结构身份和一个修订，随后与其他已提交 projection 一样
经基线/发表路径发布 `panel-moved`。重放、陈旧修订与请求身份规则遵循共享命令规则。Client
projection 只在已提交修订处把已提交的移动接受为这一确切重构——Panel 集合不变、恰有一个
fresh graft、化简后两树相等；更晚的规范修订按普通规范规则验证，若其在 Tab 存活期间移除了
选中 leaf 则修复 membership；此验证步骤中只有真正分歧才以 `AuthorityDivergence` 退役，
传输或协议错误仍按各自的 carrier 规则退役。有界 `WorkbenchPanelMove` TLA+ 模型在三 leaf
树域上覆盖 leaf 摘除、父级折叠、fresh graft 身份、方向次序/朝向及重放；Linux cohort witness
`native_standalone_panel_move_relocates_a_leaf_without_touching_either_shell` 建立共享修订、leaf
次序交换、shell 存活及已挂接 Client 刷新；Workspace 进程见证
`real_workspace_panel_move_relocates_a_leaf_without_touching_either_terminal` 经 Workspace
树成员关系路径覆盖同一提交，并证明两个保留 shell 均存活。

## 已接受的 Panel 迁移模型

`panel.relocate` 将现有终端 Panel 移到其他 Tab、现有上下文的新 Tab，或现有 Project 的新 Workspace。
参考为 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `pane.move`（H066/R049）。
Workbench 拥有拓扑，Site 拥有 standalone Open 引用，Client 视图权威拥有比例、缩放和焦点。
迁移不启动、停止或重新绑定终端，也不删除目录或 Git 树。

源元组和预期修订必须精确。现有目标形式保留默认源上下文；显式目标上下文支持跨上下文迁移，
可选目标 Panel 指定嫁接叶子，缺省保留现有根部嫁接契约。新 Workspace 指定 Project、可选的
Project 所属目录及标签。新增 `primaryDirectoryPath` 可在同次提交中创建 Directory，与已有目录 ID 互斥。本地服务通过当前 Worker 探测路径、复核其代次，并要求目录关联权限；已验证 Worker 必须拥有保留的来源终端。缺少证明、容量不足或路径失败均不能发布拓扑。新目录多分配一个身份，并由 `transfer.createdDirectoryId` 返回；仅分配新结构与目录身份。移动叶子保留 Panel、Presentation、绑定、自定义标签和
输入策略，包括全部有序子 Presentation。未绑定成员的生命周期仍是单独跟踪的限制。

| 守卫或转换                                     | 可观察结果                                                             |
| ---------------------------------------------- | ---------------------------------------------------------------------- |
| 源／目标缺失、目录归属错误、修订过期或容量耗尽 | 拓扑和身份分配发布前拒绝。                                             |
| 目标是源 Tab                                   | 保留带类型的 same-Tab 拒绝；CLI 工作流负责参考实现的无变化投影。       |
| 源仍有其他叶子                                 | 脱离叶子并折叠其父 Split。                                             |
| 源只有该叶子                                   | 移除源 Tab，但不释放终端。                                             |
| 跨上下文移动使源 Workspace 变空                | 移除 Workspace／上下文及显示顺序项，保留 Project／目录和全部终端绑定。 |
| 同上下文移动或源为变空的 standalone            | 保留上下文；达到集合上限时，新 Tab 可以替换刚移除的最后叶子 Tab。      |
| 目标为现有 Tab                                 | 在指定叶子或根部嫁接新 Split 和两个 slot，检查深度及结果节点上限。     |
| 目标为新 Tab 或 Workspace                      | 追加新 Tab；新 Workspace 追加到显式显示顺序，归属明确。                |
| 生命周期操作与源／目标或待结算的移动终端重叠   | 临时 terminal-busy，不保留成功或部分移动。                             |

一个克隆候选先通过校验和编码，再准入账本；一次提交发布快照、结构身份高水位和精确结果。
保留重放返回原结果，请求冲突和发布失败不能分配第二次移动。迁移事实标明目标上下文、移除的源
Tab／Workspace 和新建 Workspace。standalone 前后成员关系与 Site 引用在同一守卫内准备：
移出时重新定位或清空 Open，移入时将 Open 绑定到已保留终端。组合失败在发布前退役。
整个 Site 的绑定多重集合保持不变。

公开 Windows 集成还必须解析所选 Client 的目标叶子、处理缩放／无变化，并通过该 Client 应用比例／焦点。
这些阶段分别确认，后续失败保留已知拓扑提交。结构测试覆盖决策表、守恒、重放和有界交换，
共享线上夹具覆盖两种语言绑定。完整用户验收要求真实 Windows 多 Client 移动、shell 存活、刷新和重启。
通用语义事件继续由独立事件契约负责。

## 已接受的 Workspace 排序模型

用户重排整个 Site 的 Workspace 列表，不移动 Project、目录、终端上下文或运行中的终端。
Navigation 保留按身份排序的成员关系，以及可选的完整 `workspaceOrder` 排列；缺省／空列表沿用
原来的 Project 优先遍历。这将显示顺序与归属分开，支持跨多个 Project 的块移动。

| 判定                                            | 结果                                                            |
| ----------------------------------------------- | --------------------------------------------------------------- |
| 索引源不存在，或原始插入边界超过 Workspace 数量 | 修改前拒绝。                                                    |
| 源位置 S，原始插入边界 N                        | S < N 时目标为 N - 1，否则为 N；S 和 S + 1 都是无变化。         |
| 空块、重复／缺失源或缺失目标                    | 拒绝整个块。                                                    |
| before 目标属于被移动块                         | 拒绝，不进行部分排序。                                          |
| 有效块及可选 before 目标                        | 移除所有源身份，按调用者顺序插入目标前或列表末尾。              |
| 有效顺序不变                                    | 保留 Unchanged，不增加 revision、不分配身份、不写冗余排序字段。 |
| 有效顺序改变                                    | 经现有纯命令提交发布完整排列及一个 revision。                   |

现有修改 grant、Site／revision 检查、请求重放／冲突及有界 ledger 拥有准入和发布；结果丢失不能
授权用新请求身份重放。所有可失败的排列／投影准备发生在权威提交前。排序保持 Workspace／Project
成员关系、Directory 引用、终端绑定、worktree 关联和 Client 本地选中身份不变。新建 Workspace
追加到显式顺序末尾；Close 移除已提交关闭的身份。检查点捕获／恢复保留并验证顺序。

参考行为为 Herdr `e0507237` R020/R021：原始插入边界、按调用者顺序收集不连续块、原子验证和逻辑选择保留。
原生公开操作为 `workspace move <target> --insert-index N` 及 `workspace move-block <targets...> [--before target]`。
Windows CLI 与 TUI 消费该共享顺序，筛选列表保持对应子序列。通用 moved／reordered 事件交付属于
独立事件流所有者，仍是明确的集成义务；排序回执不是事件送达证据。

证据义务包括 Rust／TypeScript 共享排列及 delta 一致性，跨 Project 的索引／块／无变化／无效／重放／
过期用例，新建／关闭／检查点延续，完整 Workspace 容量边界，每 Client 选择保留，以及真实 Windows
列表／TUI／终端存活和重启。定向证据不证明完整对齐、Desktop UI 行为或其他平台交付。排序提交本身
没有进程、Git 或文件系统效果。

## 已接受的规范 Tab 移动模型

### 插入边界扩展

`tab.move-index` 携带精确 `terminalContextId`、`tabId` 和 0..256 的整数 `insertIndex`。
现有限定上下文的 Move owner 处理两个严格 action 变体；`tab.move` 保留原方向线协议。
原始零基源位置为 S、原始 Tab 数为 L 时，接受 0..L 内的 N；S < N 时目标为 N - 1，否则为 N。
边界 S 和 S + 1 均无变化；0 移到首位，L 追加到末尾。禁止截断和重复相对移动命令。
当前边界越界在目标/组合验证之后、no-op/revision 分配之前产生保留的
`tab-move-index-out-of-bounds`，顺序和身份游标不变。精确请求重放、冲突、容量、待完成创建
结算及上下文关闭围栏沿用现有 owner。完整记录及其嵌套 Panel 树和输入策略原样移动。
此转换不包含 Client 焦点、终端附着、控制器、Host 输入或 Worker 动作。

`WorkspaceTabMove` 覆盖四种方向、三个 Tab 的每个边界及一个越界边界。覆盖项区分索引移动、
索引 no-op 和边界拒绝；不提交的转换保留顺序/revision，保留结果不可变。本次 Windows 对齐
维护此有限模型但不执行 TLA。Rust 置换参考算法、跨语言严格语料、单效果命名管道及实际双 TUI
检查提供可执行证据；不声明模型检查器证明。原生 CLI 消费方及未知结果规则归属
[原生 CLI 参考](../../orchbee/docs/native-cli-query.zh-CN.md#tab-插入边界)。实验协议、Server
和原生二进制作为匹配批次一同启用。回退也使用匹配批次，不补偿已经提交的顺序，不迁移持久数据。

### Workspace 上下文扩展

本地 Move owner 也接受规范 Workspace 引用的 Terminal Context，沿用下方四种方向或插入边界、精确请求
指纹、共享 revision、no-op 和保留结果规则。上下文准入接受 standalone 或已有 Workspace 的
上下文；未知上下文返回 `TerminalContextUnavailable`，属于其他上下文的 Tab 返回
`TabUnavailable`。Move 不要求主 Directory、文件系统探测或在线 Worker。

两类上下文均接受规范拆分树、分组／固定 Tab 以及全部已绑定主／子 Presentation。缺失绑定仍为单独跟踪的生命周期限制。规范元数据不是组合失败。Tab Move 仅改变顺序，不改变分组／固定元数据，也不将单 Tab 扩展为整组。精确请求位置若破坏固定前缀或打散连续分组，则在分配修订之前返回保留的 `tab-move-order-conflict`，状态不变且权威仍可用。合法位置及精确 no-op 沿用原结果和重放规则，不截断位置或隐式重新分组。

只有 standalone Move 准入向 Site 提供 `WorkbenchTerminalMembership`，供其执行既有引用派生组合
校验。Workspace 准入不读取或修改 standalone 槽位及其保留授权。在同一 Site 门锁下，借用式
准入在已验证上下文中定位目标，预备完整 Tab 置换。规范变化仅为目标上下文 Tab 顺序和一次
共享 revision 增量；其他上下文、导航、Directory、绑定、结构身份分配、终端操作和通知均保持
不变。丢弃预备无效果。其他上下文推进共享 revision 后，重放仍保留原结果；同请求 ID 改变
上下文产生冲突。

`WorkspaceTabMove` 建模两个上下文、各三个不可变完整 Tab、有界共享请求 ledger 和 revision，
以及独立的 Empty/Creating/Settled standalone 槽位。安全性声明包括完整 Tab 保持、其他上下文
隔离、每次实际移动一次 revision、历史重放固定及跨上下文请求冲突。上下文内 Tab 索引以
`(owner, index)` 抽象全局唯一身份；错误上下文目标拒绝仍由 Rust 测试验证。必需覆盖见证三种
standalone 槽位状态各自实际发生 Workspace 移动。既有
`WorkbenchNavigationCommit` 保留容量/退役证据；具体布局、授权、分配和字节边界由 Rust 测试
覆盖。Site 测试 `workspace_pending_create_move_completion_preserves_origin_and_standalone_states`
证明 pending Workspace Create 后 Move 和精确完成；进程测试
`real_server_worker_workspace_directory_association_and_tab_lifecycle` 证明 Worker 丢失及目录删除后的
共享快照顺序与重放。这些不声明原生 Workspace UI、Workspace Close、跨上下文转移、持久化
或远程交付。

本地、未发布的 `tab.move` 线协议及 Server 操作已实现，并包含严格成员关系/Site 组合。
原生 standalone 身份投影器及 Move 在途操作已集成，其确定性及 Linux cohort 进程证据见
[原生 Session 契约](../../orchbee/docs/native-tab-session.zh-CN.md)。协议、Server 和 Client 一同激活及回退：旧位置式 Site 检查
不能与重排状态并存。回退恢复最后已验证的内存批次，不删除或终止保留进程。

问题是共享规范 Tab 顺序：已准入 Client 移动完整独立 Tab，已有 Client 保留活动 Tab 身份及终端。
上方 Workspace 扩展使用同一置换；下方引用派生的 Site 组合仅适用于 standalone。
Workbench 拥有顺序、修订、严格成员关系和请求/结果保留。Site 仅保留
`Empty | Creating(reference) | Settled { reference, outcome }`；TerminalCoordinator 拥有保留
授权和执行。Move 不增加存储的锚身份/绑定、每 Tab 的 Site 映射、Open 字段、结构身份、终端计划、
Worker IPC、查看器或控制器效果。Close/Stop、归档/重启、固定／分组属性修改、创建拆分、持久化、远程操作、
Web 迁移和推送仍从 Move 排除。Close 已有上方描述的独立本地 Server 集成，原生集成另有
Linux 进程证据；发布收敛仍待完成。不增加显式空 Open 结果。

`tree_standalone_membership()` 返回的 `WorkbenchTerminalMembership` 是包含全部已绑定独立成员的
有界只读向量，保持规范顺序，暴露
精确 Tab/Panel/Presentation 身份及绑定。Workbench 验证完整上下文并构造它；Site 从不缓存。
空成员关系有效当且仅当 Site 槽位为 Empty。非空成员关系要求 Creating 或 Settled 含引用 R、
`TerminalCoordinator.retained_authorization(R)` 可用、成功精确转换成 `PresentationBinding`，
并且向量任意位置恰有一个匹配绑定。位置从不标识引导或完成。零/多个匹配、授权缺失/无法转换、
重复上下文或已绑定成员缺少保留授权属于内部权威分歧，从不修复或返回 `TabUnavailable`。

`standalone_membership()` 仍为要求每 Tab 一个成员的调用方提供严格简单投影。
Create、完成、Move 和 Open 使用 `tree_standalone_membership()`：在相同上下文验证下，按规范
顺序返回独立 Tab 树的每个已绑定主项及子项。Site 对这些成员应用同一
引用派生锚点关系，因此合法的多叶形状绝不是组合分歧；仅凭"全体成员有 retained binding"不能证明
槽位锚点，授权缺失、零/重复锚点匹配及任何结构性违规仍是失败关闭的分歧。

在 Site 门锁内，在先行准入决策之后，对新的首次/后续 create 和 Move，以及共同精确已提交 create
完成前应用此关系；Open 在严格树成员关系上应用它。它不是无条件入口检查：授权、退役、
活动态精确 pending/final 重放或冲突及更早预检保留优先级。提交前分歧退役 Workbench，并返回顶层
`unavailable/service-unavailable`；Open 保留 `server-busy`。提交后令牌/组合失败按现有失败关闭
规则关闭载体，绝非提交前拒绝。已结算 Open 在无关 Workbench 退役后仍可读取。

语法有效 Move 的精确优先级为：

1. navigation-mutation 授权/action 授权；
2. 已退役 Workbench；
3. 保留的精确重放或全指纹请求 ID 冲突；
4. 请求账本条目容量；
5. 精确 Site，否则 `SiteMismatch`；
6. 预期修订，否则 `RevisionMismatch`；
7. 精确 standalone 或 Workspace 上下文，否则 `TerminalContextUnavailable`；
8. 已打开的上下文关闭围栏，否则暂时 `unavailable/terminal-busy`；
9. 完整严格目标上下文布局；
10. 仅 standalone 要求从引用导出的唯一绑定 Site 组合；
11. 精确目标 Tab，否则 `TabUnavailable`；
12. 插入边界不超过当前 Tab 数，否则 `TabMoveIndexOutOfBounds`；
13. 语义无变化，产生现有 `Unchanged { revision }`；
14. 保留固定前缀及分组连续性，否则 `TabMoveOrderConflict`；
15. 实际置换时的修订耗尽；
16. 预构建/验证完整候选投影、发布和固定结果；
17. 预留精确保留请求/结果字节；
18. 不会失败地提交投影、修订及已结算账本条目。

因此不支持布局优先于目标缺失/无变化；上下文缺失保留类型化拒绝。无变化优先于修订耗尽。
类型化拒绝/无变化仅在字节策略准入时保留。条目/字节耗尽为暂时 unavailable，不插入账本、不改变
顺序/修订/游标/Site/终端，也无效果。精确重放在后续提交后保留原结果修订。指纹包含 Site、基础
修订、上下文、目标和方向或插入边界，以及 action 类型。Move 既不读取也不推进结构身份游标。

公共预检和完整成员关系产生不透明借用准入；Site 验证组合后，预备 Move 才拥有完整发布及固定
账本插入。丢弃计划无效果。可恢复验证、序列化和发布失败先于修改。OOM、panic 及崩溃原子性仍排除。
Move 仅获取 Site 门锁，从不获取 Worker 注册表/句柄。Create/Open 保留放置提交的注册表到 Site
锁顺序，在 IPC 前释放两者；启动暂停时 Move/读取仍可结算。响应写入在释放 Site 门锁之后。

必需组合轨迹先让首次 create A Pending，再实际提交 B，并移动成 `[B,A]`。无关内部 Workbench
退役保留成员关系/令牌。精确 A 完成仍先验证两个来源 seal、wrapper 关联、保留授权、唯一绑定和
槽位引用，再一次性结算 Terminal 阶段、Workbench 结果及 Site outcome，并通知 Open 等待者。
后续外部读取、新 Move、pending/final 重放及同 ID 冲突被授权后退役的优先级阻挡。已准入原响应
在途操作仍可投递最终结果。Move/完成保留固定 create 结果修订、元组、创建标签和派发次数。
要求覆盖两种完成顺序。

| 声明 / 转换                                                    | 所属证据与细化                                                                     | 限制                                                      |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------- | --------------------------------------------------------- |
| 四方向、目标缺失、无变化、修订、重放/丢失/冲突、两种容量和退役 | `WorkbenchNavigationCommit`；严格语料及确定性/状态生成的预备 Move 测试             | 三成员及单成员有限模型；精确字符串/字节和发布仍由测试负责 |
| 首次/后续 create 待处理、置换、退役、精确完成及两种顺序        | `WorkbenchTabCreate`；Site 令牌/组合/状态轨迹                                      | 内部退役是 Site 证明切点，不是受支持线上触发器            |
| 槽位引用/outcome、唯一导出绑定、两个 Open 加入/等待/超时       | `LocalStandaloneTerminal`；确定性 Open/Site 测试                                   | 具体转换及零/重复匹配要求测试                             |
| 暂停启动允许 Move/读取/重放                                    | 真实 Server 进程的顺序/修订/派发见证                                               | 仅支持接口；不制造退役钩子                                |
| 已有 Client 身份及新 Client 规范首项启动                       | [原生 Session 契约](../../orchbee/docs/native-tab-session.zh-CN.md)、原生/进程轨迹 | 不声明推送、回退选择、重新挂接或全域置换定理              |

所选证据为有界 TLA 安全性及适用现有进展、可缩减/重放的状态生成、确定性核心/适配器与受控进程/
PTY 轨迹。不作无界变换/细化声明；一次性收窄 TLC 不是所属证据。记录/模型先于运行时；宣告集成前，
测试建立具体细化。

`WorkbenchTabCreate` 的 Move 组合变体从空状态执行首次和后续创建、最多两次交换、退役及退役后
精确完成。它使用三个请求身份、两个指纹、两个成员/账本条目、六个抽象账本字节、一个可用 Worker
和 Started 完成。现有容量/并发变体保留其他 outcome、Worker 更换及预检分支；rename 变体保持
独立。`firstSlot` 抽象保留的引导引用，`BindingFor` 抽象精确保留授权转换；不存储位置锚。具体
缺失/重复/无法转换授权仍属 Site 测试证据。Move 保留终端阶段、派发次数、两个令牌 seal 及固定
create 结果修订；按创建时间排序的完成顺序见证使用引导引用，而非当前顺序。

`LocalStandaloneTerminal` 保留两个 Client、两个引用/绑定、一次引导派发以及最多两次其他投影
提交。这些提交可实际追加后续成员并交换两者。追加的成员抽象任意后续叶——`tab.create` 成员或已
提交的 `panel.split` 叶——因此 `membership` 是 Open 读取的有序叶序列，而非每个 Tab 一个成员；
待处理的 split 叶已经是成员，绝不延迟或重定向已结算引用的重放。`DerivedComposition` 是合法形状
边界：成员为空当且仅当槽位为 Empty，否则锚点 binding 恰好出现一次；其不可达违例代表生产端退役的
分歧。`authorization` 属于 Terminal 所有者，有序
`membership` 属于 Workbench，Site 槽位仅保留其引用/outcome。`member` 从该授权派生。读取
使用按修订索引的规范首项历史；Open 仍观察原引用，合法叶重排（`MoveMembership`）保留它。叶移除、
锚点移除后的槽位重定向及最终 Empty 转移属于 `WorkbenchTabCloseCreateReference` 和
`WorkbenchPanelSplit`；与重定向相关的逐成员 pending/settled 阶段归它们而非本模型。现有两个
Open 的等待/超时公平性仍必须满足。
这些有界模型是成员关系/Site 细化的前提，不建立后续 Move 线上、导航账本或原生传输/投影器细化。

`WorkbenchNavigationCommit` 保留两个现有导航/rename 变体，并增加独立 Move 顺序、单成员、
字节边界和修订边界变体。Move 使用三个已有成员 ID 或单成员、两个目标 ID 加一个缺失目标、全部
四方向、基础修订 0..2，以及独立的错误 Site/上下文指纹。完整命令记录相等性覆盖每个指纹字段。
有限账本保留两个记录，每个占两个抽象字节；字节上限为二/四，变更修订上限为一/二。新语义提交在
权威处串行化，类型化响应在途操作可以投递或丢失。消费后从模型状态移除其临时载荷；不可变账本及
精确丢失的已提交请求 ID 保留，因此后续提交后重放仍保留原结果修订。原有变体保留显式双 Client
竞争提交；Move 输入的基础修订抽象各自独立捕获的过时修订。

Move 检查置换/完整成员事实保留、产品游标及无关事实不变、精确提交修订、不可变账本、两种容量、
修订耗尽前的无变化、已提交读取历史和退役/重放优先级。具体 JSON/字符串/字节记账、不支持布局
优先级、丢弃准备、发布拒绝及门锁/传输组合仍属确定性/状态及进程义务；模型不声明并行传输在途
操作或无界置换定理。

## 已接受的多叶成员关系模型

一个 Tab 合法地拥有有序的叶成员序列：`panel.split` 在同一 Tab 内产生第二叶，
`panel.close`/聚合 Close 再将其移除。因此 `tab.create` 准入、`tab.move` 准入与独立 Create
完成所消费的成员关系视图必须是树感知的有序成员列表，而不是每 Tab 恰一叶的形状断言。
对合法分割树沿用旧形状会把共享权威错误退役；已接受的修正只更换这些门禁处的成员关系视图，
并保留全部结构校验。

| 字段   | 契约                                                                                                                                                                                                                                                                   |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | TabCreate、TabMove 与 Create 完成经由「每 Tab 恰一叶」谓词读取终端成员关系，任何合法带分割树都会在准入或完成时被视为腐坏而失败。                                                                                                                                       |
| 输入   | 已准入命令指纹、精确 Site/上下文、预期修订，以及当前规范树——其每个 Tab 各拥有有序叶序列。                                                                                                                                                                              |
| 数据   | `tree_standalone_membership()`/`tree_terminal_membership()` 的有序成员、Site 持有的 `Empty`/`Creating`/`Settled` 槽位、保留的终端授权与逐 owner 的 presentation binding，以及不可变的已保留 Create 记录。                                                              |
| 输出   | 原有 mutation publication 与已保留的 Create 结算；不引入新结果种类、账本记录、修订规则或 effect。                                                                                                                                                                      |
| 不变式 | 树成员关系只替换一叶断言；独立槽位仍以「引用派生唯一 binding」校验与成员组合；Workspace 准入绝不触碰独立槽位；完成重读**当前**槽位与成员；被移除成员绝不复活，其账本恰结算一次。                                                                                       |
| 模型   | `WorkbenchMultileafMembership` 建模两个上下文（携带槽位的独立上下文、无槽位的 Workspace）、各两个 Tab、每 Tab 至多两个有序叶、anchor 与非 anchor 的 pending Create、准入/提交、交错的分割或重排、Close 捕获/发布重定向、完成与重放，并有有界成员腐坏与运行时丢失注入。 |
| 所有者 | SiteCoordinator 在既有 Site 门下组合；WorkbenchAuthority 拥有成员关系视图与投影；TerminalCoordinator 拥有保留授权、阶段与完成封记。所有者或锁归属不变。                                                                                                                |
| 依赖   | 既有树成员关系读取器、共享命令账本、修订与 publication 机制；不新增协议、依赖、crate 或调度器。                                                                                                                                                                        |
| 验证   | 带必需覆盖的有界 TLA 安全性与活性、确定性 Site/SiteCoordinator 见证，以及在真实 PTY 上覆盖分割/创建/移动与 pending-Close fencing 的 Linux 进程组；交错完成见证在受控 Worker seam 驻留 Launch 结果（已标注为合成）。                                                    |
| 边界   | 每次至多一处成员腐坏与一处运行时丢失、一次至多一个被建模聚合 Close，Tab/叶/移动均有界，Close 清理被抽象；具体字节/布局边界、open 重入的 Empty-only intent 策略与传输公平性仍归其各自所有者。                                                                           |

| 声明                                                                       | 所有者证据与细化                                                                                                                                               | 边界                                                                |
| -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| 合法多叶树可准入 create/move 并完成 pending Create，不发生形状驱动的退役   | `NoShapeRetirement`、`AnchorExactInValidStates`、`MembersRetained`；两上下文上的确定性多叶见证                                                                 | 腐坏注入是有界的、仅限独立域的示例，不是穷尽故障模型                |
| 独立 anchor 保持精确：Empty 当且仅当无成员，否则槽位引用恰匹配一次         | `AnchorExactInValidStates`、`StrongComposition` 门禁；组合校验绝不弱化到「仅成员 binding」形式                                                                 | 具体重复/不可解析授权仍属 Site 测试证据                             |
| Workspace 准入与 mutation 绝不读写独立槽位                                 | `WorkspaceIsolation`；Workspace 成员关系仅需可导出的树成员关系                                                                                                 | 共享请求账本与修订仍串行化两个上下文                                |
| 完成保持投影、恰结算一次，且仅在仍为当前 anchor 时更新槽位                 | `CompletionPreservesProjection`、`SlotTransitionsExact`、`LedgerResultStable`、`NoRejectionAfterDispatch`、`UnknownIsNotSuccess`；retarget 与 Empty 的见证轨迹 | 提交后门禁失败产出诚实 unknown；绝不转化为拒绝或退役                |
| Close 重定向跟随幸存有序成员，聚合 fence 保持精确                          | `RetargetLegal`、`AggregateFenceStable`；`split-fenced`/`unfenced-during-close` 覆盖见证                                                                       | 叶级 close 保持其自有边界；同 Tab panel mutation 仍被 fence         |
| Close 准入要求每个被捕获成员拥有已知可用运行时                             | `RefuseCloseBusy`/`RefuseCloseUnavailable` 覆盖见证；pending 目标拒绝为 Busy，未知/不可用运行时拒绝为 Unavailable                                              | 其他幸存 Tab 上的 pending 成员不阻塞目标，且可在重定向后成为 anchor |
| 每个已准入 pending Create 在公平派发/完成下达到保留 settled 或诚实 unknown | `PendingCompletionProgress`；移除公平性的 withheld-completion 对照使其违反                                                                                     | 义务在腐坏与退役后以诚实 unknown 存续；任意传输失败未建模           |

两个谓词按设计保持区分。`composition_matches` 解析当前槽位引用的保留授权，并要求其
binding 在有序成员中恰出现一次；`tree_composition_matches` 仅要求 Empty 等价加全部成员
binding 被保留。较强形式用于一切写入或校验独立 anchor 关联之处：Create 准入（可能开启
槽位）、Move 准入（重排整 Tab 但必须保持既有槽位）与 Create 完成（重读**当前**槽位与
成员）。分割准入/完成与 Close 准入/发布保持较弱形式，因为它们从不经组合解析 anchor
binding：Split 只在既有 Tab 下追加一叶，Close 发布直接从幸存有序成员推导新 anchor，而非
经组合解析。

因此 Close 不经组合即改变 anchor，且其可达轨迹是精确的：pending 成员绝不能成为 Close
目标（Busy 拒绝），故没有任何 pending anchor 会被 Close 移除或重定向走，也没有 pending
完成会在最后一次 Close 清空槽位后到达。目标成员全部已 settled 的 Close 仍可重定向到幸存
的 pending 成员——后者以 Creating 成为 anchor 并在其完成到达时结算**当前**槽位；从不当前
的 pending Create 在另一个幸存 pending 成员担任 anchor 时仅结算其账本；全部已 settled
成员的最后一次 Close 留下 Empty，其已 final 的账本仍可重放。建模的单 Close 记录是有界
检查抽象，不是生产并发限制；驻留完成的进程见证证明的是受控 Worker seam 下的顺序，而非
独立 Worker 时序。

## 范围与所有权

原生 Server 为每个本地 Site 持有一份规范 Workbench 状态。首组导航转换负责创建和重命名
Project 与 Workspace。创建 Workspace 时只创建一个空 Terminal Context，不启动终端、
创建目录或选择 Worker。首个独立 Open 还会在启动前提交共享 Tab/Panel/Presentation 成员关系
及逻辑终端 binding，具体定义见下文。[Workspace 目录契约](workspace-terminal-directory.zh-CN.md)
增加经 Worker 校验后、具有独立授权的显式目录关联。本地 `tab.create` 在同一个成员关系/终端意图共同提交边界内
追加简单独立或已配置 Workspace Tab，并支持精确的待处理/最终结果重放。纯 `tab.rename` 改变规范 Tab 标签并关闭自动命名，且可在
该完成仍 pending 时运行。`tab.move` 改变规范位置，也可在该完成仍 pending 时运行。
其他 Tab/Panel 转换、完整 Client Workbench 渲染、订阅、持久化、
重启恢复及旧实现迁移仍未完成。

Projection store 是该权威的私有实现组件，没有公开的整份 snapshot 写入口，只有此权威可以
推进它。每条已提交 publication 保留在 64 条上限的 journal 中，并通过
`workbench.publication.read` 在相同读取授权下提供，使持有基线的 Client 按序快进。主动推送
订阅及完整 Client Workbench 集成仍未完成。

跨语言请求和结果的权威属于 [Workbench 契约](../../../schemas/workbench/README.zh-CN.md)。
Workbench 在同一条已准入的本地连接上使用独立 OBWP carrier，OBCS 继续只负责终端契约。
两种协议之前都须完成 same-user OBLC 准入。Server 随后选择显式的 Site-scoped Workbench
grant。开发 composition 允许读取 snapshot 与 publication，以及纯 Workbench 操作；`tab.create` 还要求显式的
terminal-create grant。目录关联同时要求 navigation 和独立的 directory-association grant。对于外部提交的 OBWP 读取及命令，
权威在读取任何状态或 ledger 之前检查 grant。只读及拒绝策略均可表达。缺失或错误 Site 的 grant 返回 typed denied，不泄露
状态、revision、identity 或 ledger；grant 绝不来自 wire input。Client 提交语义操作与预期
revision，不能指定新产品身份、替换共享文档或发布 Client delta。

## 导航模型

`panel.rename` 为纯元数据 owner 增加精确 context／Tab／叶子主 Presentation 的覆盖项。
其 [Panel 标签契约](../../orchbee/docs/native-panel-labels.zh-CN.md) 保持创建时回退标签不可变，
并定义有界规范化设置／清除、无操作及投影语义。`WorkbenchNavigationCommit` 区分同一 Tab
的 Tab 标签与主 Presentation 覆盖项，也包含不存在的 Panel 目标；`SendPanelSet` 与
`SendPanelClear` 在既有保留 ledger 规则下覆盖两种更新方向。具体规范化以及不可变绑定／
回退标签的修改范围仍由细化测试证明。

| 字段       | 契约                                                                                                                                                                                                                                                                                                                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题       | 独立准入的 Client 必须修改和观察同一份共享导航结构，避免丢失更新或重复逻辑效果。                                                                                                                                                                                                                                           |
| 输入       | 严格 snapshot 或 publication 读取 intent，或包含 request identity、确切 Site、预期 revision 及已准入的纯 action的严格命令。创建标签与 Tab 标签保留原始非空文本，最多 512 个 Unicode code point；窗格自定义标题按上文规范化有界可空更新。Tab rename fingerprint 包含 context、Tab、label 和 base revision。                 |
| 数据       | Active 或 retired 权威、一份已校验 Snapshot、Server 拥有且不复用的 identity cursor，以及有界 request/result ledger。初始 revision 为零，standalone Terminal Context 为空。Adapter 从操作系统随机源取得非零 Site identity。Cursor 不复用导航聚合内的身份；无关 Terminal/operation 身份域保留现有 owner。                    |
| 输出       | 读取返回相关联的 available { snapshot }、denied {} 或 unavailable { reason }。Publication 读取返回 current { revision }、published { publications }、denied {} 或 unavailable { reason }。命令返回 project-created、workspace-created、renamed、unchanged、rejected、denied 或 unavailable。新身份仅出现于对应成功变体中。 |
| 不变量     | 只有 Server 提交共享结构。每个 Workspace 恰好引用一个 context。拒绝保留结构、revision 和分配状态。发生变更时 revision 恰好加一。固定名称的等值重命名保留 revision。Tab rename 只改变目标 `Tab.label` 和 `Tab.automaticLabel`；非目标标签及所有其他 projection 事实保持精确。一个 request identity 不能提交两次。           |
| 决策       | 授权先于 retired 状态及 ledger 检查。Active 精确重放先于预期 revision 检查。保留的 request identity 对应内容改变时是冲突。拒绝命令没有结构效果。意外内部 invariant 失败会原子停用权威；后续所有已授权读取和命令（包括重放）都不可用。                                                                                      |
| 模型       | 对完整 candidate 执行确定性串行转换；网络、时钟、文件系统、随机源及进程效果留在 adapter。                                                                                                                                                                                                                                  |
| 依赖       | 既有 Workbench snapshot 校验及上限、严格 OBWP framing、已准入本地 Session。这些导航操作不需要 Worker 或 Host。                                                                                                                                                                                                             |
| 所有权     | 权威一起持有 snapshot/cursor/ledger。Service 拥有初始化及调用串行化。Application-session adapter 拥有协议选择及释放锁之后的有界响应交付。                                                                                                                                                                                  |
| 验证       | 依据 schema 编写的 Rust/TypeScript conformance、可缩减的确定性有状态命令轨迹、有界 TLA 并发/重放检查，以及真实双 Client Server 进程证据。                                                                                                                                                                                  |
| 正确性限制 | 模型抽象 label、identity encoding、OS 调度及 JSON。进程证据为 Linux；本组转换不声明持久化、transport retry、重启后恢复 Site identity 或公开 Client 集成。                                                                                                                                                                  |

| Action           | Guard                                  | 提交结果                                                          |
| ---------------- | -------------------------------------- | ----------------------------------------------------------------- |
| project.create   | Label 和 projection 容量被接受         | 一个没有 Workspace 的新 Project                                   |
| project.rename   | 确切 Project 存在                      | 修改其 label；等值文本为 unchanged                                |
| workspace.create | 确切 Project 存在且 context 容量被接受 | 一个新 Workspace 及其唯一空 Terminal Context                      |
| workspace.rename | 确切 Workspace 属于指定 Project        | 修改其 label；等值文本为 unchanged                                |
| tab.rename       | 确切 context 包含确切 Tab              | 修改其 Tab label 并关闭自动命名；仅固定模式的等值文本为 unchanged |
| panel.rename     | 精确 context/Tab 包含目标 Panel        | 主 Presentation 自定义标题设置或清除；规范化后相等则不变          |

仅窗格自定义标题更新裁剪空白。这些操作不做乐观 Client 提交、自动冲突重试、焦点改变或终端效果。
读取不占用 command ledger 条目。

## 提交与重放

授权在线性化的 Server policy 检查处发生，先于任何状态或 ledger 读取。读取在线性化的
snapshot 克隆处发生，并与写入共用 commit gate。发生变更的命令在 snapshot/cursor/ledger
原子提交处线性化；已结算拒绝或 unchanged 命令在 ledger 插入处线性化。同身份的相同或
冲突请求竞争遵守同一个 gate。

对于已授权有效命令，owner 先解析精确重放或 request-identity conflict。新命令对当前 Site
和 revision 求值，先准备完整 candidate、identity cursor、typed result 与 ledger 条目，
再一起提交。Candidate 校验、编码和 projection publication 预检位于 ledger byte reservation
之前。`CommitPlan` 保留可选 `PreparedProjectionAdvance`；reservation 之后的 commit 只执行
`commit_prepared` 与 cursor/ledger move，第一次修改之后没有可恢复检查。无效或不可发布的
candidate 不能消耗 identity 或 ledger 容量。Transport 在释放状态所有权后写入关联响应。响应丢失不回滚已提交状态。

决策顺序是授权、retired 状态、保留的重放/冲突、ledger 数量、Site、预期 revision、确切
target context 后再到 target member、等值重命名 no-op、collection 容量、identity 分配、发生变更的 revision，以及编码
candidate 容量。`u64::MAX` 上的等值重命名仍为 unchanged。Workspace 创建同时预检两个
identity。最后，在任何新结算前，canonical ledger 字节计数必须接纳暂存的 request/result；
字节耗尽会把预期结果改为 unavailable，并保持全部状态不变。该最终检查也覆盖预期拒绝及
no-op。既有重放不受两项容量检查限制。

Service 在与读写相同的 gate 内串行处理停用。初始化随机源失败只让 Workbench 不可用；
无关终端准入和执行继续可用。Retired 权威不会在此进程内自动重置或接受新工作。

重放等价比较已校验的语义命令值，与 JSON 空白、key 顺序及等价 escape 无关。重复 JSON key
在 typed admission 之前是协议失败。Ledger 保留已结算的有效请求，包括拒绝和 unchanged
结果。相同 identity 的不同内容不覆盖既有记录。上限为 4,096 个 request identity，以及
canonical request/result JSON 合计 8 MiB。这是序列化计数上限，不是精确 heap 大小声明。
超过任一上限的新工作无效果且不可用，已保留的精确重放仍可用。没有会把旧请求变成新效果
的自动 eviction。

在持久化/恢复集成完成前，ledger 与 Site identity 仅存于当前进程。Client 断开不会销毁 Site，
替代 Client 可在同一个 Site 进程内对账精确请求。这不授权盲目重放输入或跨重启重放请求。
新的 Site identity 拒绝寻址旧 Site 的请求。

| 条件                                                            | 结果                                                                           |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| 缺失或错误 Site 的 grant，或只读 grant 下的 mutation            | denied；不泄露当前 revision 或状态                                             |
| 命令寻址另一 Site                                               | rejected: site-mismatch                                                        |
| 预期 revision 不同                                              | rejected: revision-mismatch                                                    |
| Project 缺失                                                    | rejected: project-unavailable                                                  |
| Workspace 缺失或不属于指定 Project                              | rejected: workspace-unavailable                                                |
| Terminal Context 缺失                                           | rejected: terminal-context-unavailable                                         |
| Tab 不在指定 Terminal Context 中                                | rejected: tab-unavailable                                                      |
| 保留 identity 对应不同语义命令内容                              | rejected: request-identity-conflict                                            |
| Project、Workspace、context 数量或编码 snapshot 超限            | rejected: resource-limit                                                       |
| 导航 identity cursor 或发生变更的 revision 无法推进             | rejected: identity-exhausted 或 revision-exhausted                             |
| 新请求超过任一 ledger 上限                                      | unavailable: request-ledger-full                                               |
| Service 已停用或意外内部 candidate/serialization invariant 失败 | unavailable: service-unavailable；停用 service，不把程序错误表示为 Client 拒绝 |

## Launch Profile 目录

authority 还拥有一个按 Site 划分、易失的 Launch Profile 目录：为 `tab.create` 与 `panel.split`
命名可复用自定义可执行文件启动选择的私有 Server 状态。目录仅存在于内存，随 Site 进程消亡；
它从不进入公开 snapshot，因此导航提交既看不到也改不了它，`deny_unknown_fields` 也把旧读取方
与新形状完全隔开。

| 字段   | 约定                                                                                                                                                                                                                                                                                                                                         |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 已接纳 Client 必须能列出、解析、创建、删除命名的自定义可执行文件选择，并在创建终端时消费它们，而不引入第二个目录写方或持久化注册表。                                                                                                                                                                                                         |
| 输入   | 严格的 `workbench.launch-profile-catalog.read` 意图（请求 identity + 可选 selector），或严格的 `launch-profile.create`/`launch-profile.delete` 命令。selector 是 1–512 字节不透明文本；name 复用 label 信封；selection 是封闭的 launch 契约。                                                                                                |
| 数据   | 有界条目向量（64 条、序列化总量 256 KiB）、私有单调递增的目录 revision，以及与导航组共用的同一请求/结果 ledger。Profile 由 Site identity 与既有 Server identity cursor 铸造不透明 id `<siteHex>:<counterHex>`。                                                                                                                              |
| 输出   | 读返回 catalog { entries }、profile { entry }、not-found、ambiguous { candidateIds }、denied 或 unavailable。命令返回 launch-profile-created/deleted { revision, catalogRevision, profileId }、落账的 selector 解析结果 launch-profile-unavailable 与 launch-profile-ambiguous { candidateIds }，以及常规 rejected/denied/unavailable 集合。 |
| 不变量 | 只有 Server 提交目录状态。已提交的 create/delete 同时推进 Workbench revision 与私有目录 revision，并与 projection、ledger、保留请求更新原子完成。精确 profile-id 匹配先于任何 name 查找；否则零个 name 匹配为 not-found，一个为 found，多于一个为 ambiguous 并携带同一次解析捕获的有界 id 列表。                                             |
| 决策   | 读与 snapshot 读使用相同的 site-identity grant 语义，从不进入命令 ledger。selector 失败与其他已决结果一样落账，因此相同请求重放相同应答。字节容量与 revision 耗尽拒绝先于任何目录变更。已消亡 Site 的 id 内嵌该 Site identity，构造上不可能别名到另一个 Site 的条目。                                                                        |
| 模型   | 同一个确定性串行迁移：candidate 状态在提交闸内暂存，首次变更之后没有可恢复的检查。                                                                                                                                                                                                                                                           |
| 属主   | authority 拥有目录条目、私有目录 revision 与铸造。service 拥有策略映射（读走 read grant、变更走 navigation）与调用序列化。终端创建消费已解析的 `LaunchProfileUse` —— 绝不是 selector 文本 —— 因此 Worker 启动载荷仍只携带 executable 与 arguments。                                                                                          |
| 验证   | authority 有状态测试覆盖 id-first/基数解析、重放与冲突、数量与序列化边界、零变更的 revision 耗尽、跨 Site id 不可别名；协调器测试覆盖 prepare 期冻结与 provenance 发布。                                                                                                                                                                     |

`tab.create` 与 `panel.split` 接受既有显式 `launch` 选择或 `profile` selector，二者绝不同现
—— 线形在解析与序列化两个方向都拒绝该组合。`profile` selector 在命令接纳内、于拥有目录的
同一个协调器锁下解析：解析出的选择与 `{profileId, catalogRevision}` provenance 对在异步
Worker 间隙之前冻结，因此派发后才被删除的 profile 仍按捕获载荷启动，终端的 launch read 也
仍发布其 provenance。解析失败落账为类型化结果，歧义载荷只携带同一次解析的候选 id ——
绝不携带来自其他解析时刻的可执行文件文本。

## 证据与 refinement

| 主张                                              | 必需证据                                                       | 边界                                                       |
| ------------------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------------------- |
| 竞争预期 revision 只提交一次                      | WorkbenchNavigationCommit 模型与双 Client 进程轨迹             | 两个抽象 Client；进程内串行权威                            |
| 读取观察一份已提交状态；拒绝调用不泄露状态        | 携带 revision/value history 的原子读取模型；已准入只读策略测试 | 读取在 commit gate 下克隆时线性化                          |
| 停用阻止后续读取、写入及重放                      | Retire 和 Unavailable 模型 action；确定性 service 轨迹         | 进程内停用，不声明恢复                                     |
| 慢 snapshot writer 不阻塞其他 Client              | 阻塞响应 writer 与并发读写证据                                 | 响应 I/O 位于克隆及释放锁之后                              |
| 响应丢失后精确重放不产生新效果                    | 模型 replay/conflict 转换与重连进程轨迹                        | 仅保留的 Site 进程 ledger                                  |
| Tab rename 只改变目标标签，后续提交后仍重放原修订 | 双目标/缺失目标模型 variant 及有状态轨迹                       | 两种标签；确切字符串与规范字节仍由确定性证据负责           |
| 导航保持有效且拒绝无效果                          | 有状态命令轨迹及 snapshot validator                            | 既有 snapshot collection、identity 和字节上限              |
| 跨语言契约含义一致                                | Rust 与 TypeScript 消费共享导航命令 corpus                     | 严格已解码 JSON 及编码 carrier 上限                        |
| 大 snapshot 保持 framing 与方向                   | OBWP codec 和进程轨迹                                          | 请求/命令结果 16 KiB；snapshot 结果最多 1 MiB + 1,024 字节 |

从仓库根目录运行：

```sh
cargo test -p orchbee-server -p orchbee-workbench-protocol --locked
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol typecheck
node scripts/tla/run-models.mjs --models workbench-navigation-commit
pnpm verify
```

并发模型使用两个 Client、三个 request identity，最多提交三项 operation。Core variant 保留原有
授权、退役、容量、并发、读取历史、响应丢失与重放覆盖。Target-domain variant 使用两个既有
context/Tab pair、一个跨 context 缺失 pair 和两种抽象标签，使非目标标签稳定性不是空真。
Ledger entry 和响应 flight 保留原始 result revision，包括后续 mutation 后的精确重放。
Context 先于 Tab 的解析、确切 Unicode 标签文本、identity 编码、canonical byte 及分离的条目/
字节预检边界仍由确定性实现证据负责；模型容量谓词抽象两种 ledger 上限。
仅通过抽象模型或 binding 不代表 Server 进程证据或其余 Workbench 交付工作完成。

## 独立终端成员关系模型

空独立终端上下文中的首个本地 `terminal.open` 在通过全部预检后，先提交一个 Tab、一个 Panel
及其主 Presentation，再提交 Worker 启动。这项内部语义操作不进入 OBWP 导航 command ledger。
现有独立 Open 状态负责合并请求及禁止重放。之后成功读取的导航 revision 可以不同，但保留同一份
已提交的独立终端成员关系和 binding。

| 字段   | 契约                                                                                                                                                                                                                 |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 保留的原生终端必须属于共享 Workbench，不能产生未记录的启动或第二个成员关系 writer。                                                                                                                                  |
| 输入   | 已准入的本地 Open、真实 geometry、Server 新生成的 Create reference、确切的当前 Worker placement 和 lease。                                                                                                           |
| 数据   | 同一个 Site gate 串行化 Workbench 权威、终端意图 coordinator 及 SiteCoordinator 拥有的 Empty/Creating/Settled 独立状态。Client identity、Create reference、聚合身份和逻辑 Terminal/Worker binding 属于不同身份领域。 |
| 输出   | 现有相关联的 Open result，以及包含首个独立 Tab、Panel 和 Presentation 的一个规范 snapshot revision。                                                                                                                 |
| 不变量 | 没有已提交成员关系和终端意图就不能启动；成员关系必须对应确切的计划终端意图；预检失败不消耗任何 allocator；后续导航保留 binding；提交后不自动回滚、重置或重新启动。                                                   |
| 模型   | 纯终端预检、已验证的 Workbench candidate、一次受 gate 保护的跨组件提交、释放锁后 dispatch、相关联的终端 settlement，最后发布响应。已结算重放读取严格树成员关系，并要求保留的锚点 binding 恰好出现一次。              |
| Owner  | binary-private SiteCoordinator 在现有 service gate 下组合两个确定性 owner。只有 WorkbenchAuthority 写 projection；只有 TerminalCoordinator 拥有 launch/start authorization 和执行 phase。                            |
| 依赖   | 现有 OBCS Open、OBCN authorization、OBWP read、Workbench projection 和本地 Worker registry。不增加协议、依赖、package 或网络权威。                                                                                   |
| 验证   | core 边界及序列测试，区分身份领域并覆盖导航/读取交错的 LocalStandaloneTerminal，真实 Server/Worker carrier trace，以及保留 shell 的 PTY 证据。                                                                       |
| 限制   | 一个进程内的独立终端。后续 Tab/Panel mutation、完整 Client Workbench 集成、订阅、持久化和恢复仍未完成。                                                                                                              |

终端 coordinator 首先生成完整且不可 Clone 的 Create plan，不修改状态。随后 Workbench 预检空
独立上下文、三个全新的聚合身份、下一个 revision、完整编码 snapshot 及 publication。上下文没有 group；唯一 Tab 不属于 group、未 pinned，
包含一个 Panel，且没有 child Presentation。首个 Tab 和主 Presentation 使用与 locale 无关的序号标签 `1`。binding 使用计划 OBCN authorization 中的 Server
逻辑 `TerminalId` 和所选 `WorkerId`，绝不写入 Host 终端身份、Worker generation、operation/lease
identity、Client request identity 或内部 Create reference。

非空 binding 表示保留的产品意图，不承诺 Worker 或 Host 当前可用，也不表示启动成功。执行可用性
由终端 owner 解析。缺失 binding 仍不表示任何生命周期原因。启动完成只改变此 projection 未编码的
执行 phase，因此不再增加第二个 Workbench revision。

Workbench 验证并提交后，同一个 gate 通过不会失败的所有权转移提交准备好的终端 binding、allocator
及独立 Creating reference。读取和其他 command 无法观察到这些步骤之间的状态。原子性覆盖 gate 内
成功的进程内执行，不承诺 crash/persistence recovery。Clock 采样及 deadline 保留在 adapter；
此 gate 内不执行 randomness、filesystem、Worker IPC、condition wait 或响应写入。WorkbenchService 应用现有
显式 read/navigation policy 后委托给同一个 Site 权威，不再拥有第二份 snapshot 或 mutex。
Site 随机身份只在 service 构造时于锁外读取一次。

提交前缺少 Worker、reference 错误、identity/revision/capacity 耗尽或 Workbench 不可用时，返回现有
typed unavailable result，不启动也不创建成员关系。Workbench 预检/publication 的内部不变量失败
会在 dispatch 前退役 Workbench。之后其他导航引发的退役不会取消已经提交的启动。直接实验性 Create
和 Start 仍独立于 Workbench 初始化，也不创建 Tab 成员关系。

任何提交后失败都保留共享 Tab 和逻辑 binding，并结算现有 typed failed、unavailable 或 unknown Open
result。Client 丢失及响应未读都不能撤销提交。确切 Worker generation 校验仍约束成功；替换 Worker
不会接管或重新启动该终端。现有 condition wait 释放 gate，并沿用同一个绝对截止时间。

Creating 与 Settled 重放读取 `tree_standalone_membership()`——按规范顺序返回独立 Tab 树的每个叶
成员——因此合法拆分或重排的树绝不是不变量失败，也不会退役权威。锚点关系不变：树无成员时槽位恰为
Empty；否则槽位引用的保留授权必须转换为 `PresentationBinding`，并在全体成员中恰好出现一次。已提交
的 split 叶在其终端 outcome 可知前已是成员：待处理兄弟叶既不重定向也不阻塞已结算引用的重放，合法
叶重排保留该引用。只有既有 Close settlement 改变引用——指向保留的幸存成员——最后一次移除回到
Empty；Open 绝不另选替代终端。授权缺失、零/重复锚点出现次数及任何树形状或 binding 违规仍是失败
关闭的分歧，会退役权威并返回 `server-busy`。

形式模型使用两个 Client、两个独立 Create reference、两个不透明逻辑 binding，以及最多两次交错的
导航提交。Reserve 表示完整的 gate 内提交；Dispatch 是独立动作，必须携带该 binding。其有序
`membership` 抽象叶级严格树成员关系：追加的成员既代表 `tab.create` 成员也代表已提交的 split 叶，
因此两成员上的已结算槽位正是多叶重放形状。
ReadWorkbench 观察一份已提交 snapshot。抽象 capacity 和 Workbench retirement 不能代替确切编码、
身份分配及真实进程测试。

## 从 Workbench 绑定选择终端

本机 Client 可以使用 canonical Workbench snapshot 中确切的 `siteId`、`workerId`
和逻辑 `terminalId` 请求只读 viewer attach。OBCS 的 `attach-binding` action 用于选择终端；
它不提交 Workbench command，也不修改成员关系。原生 Client 在 standalone Open 后，
通过同一个已准入连接执行相关联的 OBWP snapshot read，再使用此 action。现有显式
Create-reference attach 继续供实验性 Create/Attach 路径使用。

| 字段   | 契约                                                                                                                                                                                                 |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | Client 读取共享产品状态后，应能选择其保留终端，而不依赖内部 Create reference，也不静默跟随另一个执行身份。                                                                                           |
| 输入   | 已准入 Session，以及确切的 Site/Worker/Server-terminal 三元组。三者是独立的不透明身份域；都不是 Host identity、generation、operation、lease 或 Create reference。                                    |
| 数据   | Canonical 暴露的 binding、保留终端的 authorization/phase、当前 Worker placement 和 Session 本地 attachment state。                                                                                   |
| 输出   | 现有 Attached scope，或有类型的拒绝/不可用结果。选择不产生终端 create、start、input、resize、controller grant 或 Workbench effect。                                                                  |
| 不变量 | 只解析当前活跃 Site 暴露的 binding；固定保留的确切 Worker generation；绝不替换终端或 Worker；target validation 失败时，在分配 attachment identity 前拒绝。                                           |
| 所有权 | WorkbenchAuthority 检查 canonical membership；TerminalCoordinator 解析保留执行；SiteCoordinator 在现有 registry 后 Site 的锁顺序下组合这些检查。ViewerSession 在释放锁后拥有 attachment IPC 和清理。 |
| 模型   | 解析有类型的 target，准备 attachment，通过固定的 handle dispatch，并在现有 current-Worker 检查处提交 Session attachment。                                                                            |
| 依赖   | 尚未发布的 OBCS v1 值、保持不变的 OBCN/OBSR relay、现有 OBWP snapshot read，以及原生 Client 现有的绝对提交截止时间。                                                                                 |
| 限制   | 同用户本机 target resolution，不接管控制权。Selection 模型不证明多个 viewer 路由、远程授权、持久化、重启后接管、选择 UI 或 Worker 持续可用。                                                         |

[Session 路由所有者](../README.md#accepted-session-viewer-routing-modeling-record) 先回收精确
已知 Disposed 的条目，再执行容量限制。有空余容量时，同一 Session 重复解析到同一个存活 target，
会在 Worker IPC 前返回 `already-attached`；不同 target 可以共存。在 target resolution 内，
Registry 或 Site 锁中毒时返回 `server-busy`。
Workbench 缺失或 retired 时返回 `workbench-unavailable`；Site 错误，或当前暴露的
Presentation binding 中不存在该三元组时，返回 `terminal-unavailable`。当前 Worker 缺失或 Worker identity 不匹配时，为 `terminal-runtime-unavailable`。
接下来，缺失确切 retained record 时为 `terminal-unavailable`；保留 generation 不匹配时
为 `terminal-runtime-unavailable`；最后，其 phase 不允许 attach 时为 `terminal-unavailable`。这些 target 拒绝均发生在
attachment identity 分配和 Worker IPC 前。Identity capacity，以及已知没有改变状态的队列/
attachment capacity 拒绝，保留现有 `server-busy` 语义。现有已知未改变状态的 `server-busy` viewer retry 可以在同一个逻辑截止时间内，
使用新 OBCS request identity 提交相同 binding 三元组。不重试 snapshot read，也不 fallback、
创建终端、重新选择 target 或重放未知 effect。

Membership validation 与 target resolution 在持有 Worker registry 时，于 Site gate 内共同
线性化。选择不要求 snapshot revision 仍是当前版本：只要确切 binding 仍暴露，就允许无关导航先提交。
准备好的 target 固定保留的 Worker generation 与 Host identity；Client 不提供这两个字段。
随后 Workbench retirement 不取消已经解析的 attachment。成功 attach 在线性化时执行现有的
IPC 后 current-Worker 检查。此检查后 Worker 丢失可能与响应交付竞态；下一个 viewer/control
操作仍应用现有 generation fencing。Attached 不承诺 Worker 会持续可用。

OBCS 保留现有同用户终端准入。OBWP read/navigation policy 继续仅拥有其各自的请求；
知道三元组不会形成新的授权来源。生产 Site 组合根据确切的 planned terminal authorization 提交成员关系。因此，
通过该构造不会出现已暴露三元组没有匹配 retained record 的情况，包括终端存储在另一个 Worker 下。
此查询对这样的不一致 fixture 仍返回 `terminal-unavailable`，不引入 retirement 或修复。
Workbench lookup 是内部读取，不是第二个 snapshot writer，
也不是 command-ledger entry。

| 声明                                                        | 必需证据                                                                                                                                                                                                                                                                       | 边界                                                                                             |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------ |
| 解析确切暴露三元组，不改变 membership 或 terminal execution | ExactSelection、SelectionWasAuthorized、DispatchWasResolved 和 SelectionPreservesTerminal；`exact_exposed_binding_prepares_the_retained_host_without_changing_workbench` 与 `workbench_refusals_precede_runtime_interpretation_and_consume_no_identity`                        | 现有有界 canonical snapshot 与 retained terminal map                                             |
| 替换 Worker 不会重新指向或新授权过期选择                    | InstallationWasCurrent 和 NoRetarget；`worker_loss_before_post_ipc_current_check_prevents_attachment_publication`、`worker_loss_after_post_ipc_current_check_does_not_revoke_attachment` 与 `standalone_unknown_after_worker_loss_retains_membership_across_worker_generation` | 模型：两个 Client，一个 logical Worker 及一次 generation 替换；core 测试覆盖其他 Worker identity |
| Client 从其已准入连接上的共享 snapshot 选择终端             | `standalone_selects_the_snapshot_binding_and_never_replays_unknown_open`、`canonical_snapshot_binding_attaches_exact_host_without_launch_and_is_session_scoped` 与新构建 release 的 PTY startup                                                                                | 一个 standalone Tab，包含一个 Panel 及有 binding 的 primary Presentation                         |
| 混合 OBWP/OBCS 响应保持关联和截止时间边界                   | `snapshot_read_supports_partial_io_and_preserves_the_same_mixed_carrier`、`snapshot_read_retires_on_wrong_kind_or_correlation` 与 `standalone_snapshot_does_not_complete_or_reset_the_startup_deadline`                                                                        | 现有 startup ceiling；不重试 snapshot read 或未知结果                                            |
| 线协议语义符合其权威                                        | `schema_and_corpus_are_well_formed_and_every_canonical_message_round_trips` 与 `consumes_every_invalid_and_forbidden_corpus_case_as_a_rejection`                                                                                                                               | 尚未发布的 v1 增量，现有 framing 和 byte bound                                                   |

`WorkbenchTerminalSelection` 将确切 lookup 与异步 Worker accept、Session installation 分开，
允许 retirement、Worker loss/replacement 和 launch completion 在步骤间交错。其 Worker 替换保持相同 logical Worker identity，并改变一次 generation。
它抽象其他 Worker identity、详细拒绝优先级、JSON、identity encoding、clock、allocation、
OS scheduling、Host cleanup 和 Client rendering。Core 测试覆盖 identity 和 reason 区别。
受控 service trace 覆盖 IPC 后检查两侧的 loss；进程测试不声明调度到这个确切指令边界。
现有 navigation、standalone 和 relay 模型继续拥有各自的 state family；
确定性与真实进程测试建立 selection refinement。

Client read-error 与结构 selector 规则属于其
[独立终端意图](../../orchbee/README.zh-CN.md#独立终端意图)。身份域不要求编码字节两两不同。
每个字段须独立满足非零身份契约；不同域中的相同文本是有效的。Selection 模型只检查 safety，
没有 fairness 或 settlement property。现有 LocalWorkerObcnCarrier 与
LocalClientSubmission/Backpressure 边界及运行时 deadline trace 拥有 progress 和失败 settlement。

## 独立 Tab 创建模型

本地 `tab.create` 细化将一次共享成员提交与一次保留终端意图组合起来。
其[线协议](../../../schemas/workbench/README.zh-CN.md#独立-tab-创建契约)
属于尚未发布的 OBWP v1 扩展。Server 命令、混合载体分派及原生 Client 开发交互已经实现。
[原生 Tab 指南](../../orchbee/README.zh-CN.md#原生-tab-交互)维护 Client 证据；安装后交付仍未完成。

| 字段       | 契约                                                                                                                                                                                                                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题       | 独立 Client 必须能够创建另一个独立终端 Tab，且不会引入第二个成员写入者、未跟踪的启动，或响应丢失后的重复效果。                                                                                                             |
| 输入与信任 | 严格命令包含请求标识、精确 Site/上下文、预期修订、原始标签和已验证几何；Server 选择同时允许 Workbench 命令与终端创建的授权。导航权限本身不授权启动。熵、租约时间和当前 Worker 放置都是适配器输入。                         |
| 数据       | 一份规范投影/修订/游标、创建时与当前 Tab 标签、不可变的 Presentation 创建标签、带已提交结果修订的有界 Pending/Settled 命令账本、每个已提交意图的一条保留终端记录，以及 Site 拥有的首终端槽位。每个标识域保留自己的所有者。 |
| 输出       | 提交前 denied/rejected/unavailable；精确重放的 pending；或携带 started、unavailable、failed、unknown 终端结果的最终 tab-created。所有已提交结果都保留相同 Tab/Panel/Presentation 元组及绑定。                              |
| 所有者     | 只有 WorkbenchAuthority 写入投影、修订、游标、账本和退役状态。只有 TerminalCoordinator 写入保留终端授权与阶段。SiteCoordinator 组合两个所有者及首槽位。适配器拥有熵、时钟、锁、IPC 和结果投递。                            |
| 模型       | 无修改地准备两个所有者，在 Site 门闩内共同提交，允许已通过自身授权的纯 rename 提交，在所有者门闩外派发一次，无修改地准备两个完成计划，再共同结算。Completion 保留 projection revision 与两个标签。                         |
| 依赖       | 现有有界协议验证器、投影发布、同用户接纳、当前 Worker 注册表、保留终端协调器和单个顺序混合 Client 载体。不向库引入 OBCS/OBCN、Worker 句柄或 Host 执行依赖。                                                                |
| 非目标     | 其他 Terminal Context、拆分/分组/固定/重排/关闭、持久化、重启接管、远程授权、取消、自动重试或替换启动、订阅、发布打包与旧实现切换。                                                                                        |

### 接纳与共同提交

库在不读取 authority 状态的情况下分类线请求。不透明的
`WorkbenchReadOrNavigationRequest` 只接纳快照读取、publication 读取和纯 Workbench 操作；
`TryFrom<WorkbenchRequest>` 在无法分类时返回完整的原始请求并转移其所有权。
服务显式地将被拒绝的 TabCreate 和 TabMove 命令路由到各自的 Site 组合路径；
其他被拒绝的请求得到明确的不可用响应。分类拒绝不会隐式选择终端创建，
并保留请求的全部元数据。
纯 `execute` 入口只接受该分类类型。终端创建走下述专用接纳/准备路径；
通用执行器不会仅因缺少终端计划就伪造 server-busy。分类不授权请求，
纯读取/导航继续独立于 Worker 注册表可用性。

判定优先级为组合授权、Workbench 缺失/退役、精确保留重放或语义冲突、账本条目数、Site、
预期修订、精确独立上下文、Tab/节点容量、全部三个结构标识、变化修订、当前 Worker、
保留终端资源、候选发布及账本字节。字节接纳也覆盖保留的拒绝结果。已知在提交前缺少 Worker
或资源忙的结果是瞬时结果，不创建账本条目。完整语义相等包含上下文、标签、解码几何和预期修订；
冲突从不替换已有记录。

适配器在锁外采样不产生效果的引用/租约候选；候选缺失不能覆盖授权或保留重放。它先取得
Worker 注册表，捕获并保留当前句柄及放置，再取得 Site 门闩。两个门闩都持有到共同提交完成，
防止同一 Worker 在放置捕获与发布之间更换代次。确定性所有者不接收时钟、熵或 Worker 能力。

Workbench 接纳和已准备发布保持对精确 authority 的独占借用。接纳先暂存结构标识和修订，再解释运行时。
使用终端计划的精确绑定完成准备时，验证并预建完整候选投影、其发布、pending 结果及所有可能的最终结果。
终端准备同样独占借用精确 coordinator。丢弃任一计划都没有效果。Site 分别借用其所有者字段；
两个计划都成功后，提交仅通过不会返回失败的移动来写入保留 Creating 记录、完整 Workbench 发布/账本，
以及适用时的首槽位。第一次修改之后不得再有序列化、候选验证或可能失败的发布步骤。
这里的不会失败指没有可恢复的 Result 失败；普通 Rust 容器分配仍可能中止进程。
OOM、panic 和崩溃原子性不在范围内，也不要求复制整个 map。
传入标签精确写入 Tab.label 和主 Presentation.label；重复标签仍然有效。

Pending 条目计入现有 4,096 条目与 8 MiB 规范请求/结果上限。提交前，条目预留请求字节，
再加 pending、started、两种 unavailable 原因、failed 和 unknown 中最大的结果字节数。
预验证的最终结果表始终私属于 WorkbenchAuthority。结算只选择表项并释放多余计费，
不推进修订或更改成员。精确重放为该次调用独立复制 pending 或 settled 结果。
最初的创建调用等待最终结算；只有重放可以返回 pending。后续结算或冲突不能改写已经复制、
正在等待投递的响应。

有界 composition variant 对已经通过自身授权、重放、条目、字节、目标和 publication 检查的外部
rename 最多建模一次，不重复构造该纯操作的 ledger。已提交 create 在 pending 与 final replay 中
保留其 membership revision；之后 Tab label 可以改变，而 Presentation label 保持创建 anchor。
Completion 只结算原 create ledger entry，不能推进 revision 或回写任一标签。原有 capacity 与
concurrency variant 设置 `MaxRenames = 0`；composition variant 设置 `MaxRenames = 1`，并要求
pending-rename-final 顺序以及 pending/final replay 见证。

### 完成与首终端对应关系

每个 Workbench Pending 条目及保留 Terminal Creating 记录都拥有私有的逐条目 `Arc` 封印；
对应的唯一不透明、不可 Clone 完成令牌保留同一个分配。指针相等标识精确条目，
无需线标识、随机 nonce 或额外计数器。不透明 Site pending 包装是唯一组合构造入口，
让两个令牌、其引用/绑定/修订关联，以及固定放置在 IPC 期间保持在一起。

两个完成所有者都先验证精确令牌、阶段和不可变关联，再允许任何一方修改。借用的完成计划
计算最终终端阶段并选择预验证 Workbench 结果。Site 检查两者关联与首槽位对应关系，
然后以不会返回失败的移动消费两个计划及适用的槽位转换。外来、过期、已消费或混配令牌
不改变任何语义所有者。理论上不可发生的提交后协调或锁失败会关闭混合载体，
不返回语义命令结果；Tab 已提交后，不能再声称提交前 service-unavailable。
Client 此时观察到命令结果未知。

已接受的 Move 组合中，独立 Tabs 为空当且仅当首槽位为 Empty。非空 Tabs 要求 Creating 或
Settled，并且整个成员关系中恰有一个绑定匹配保留引用导出的绑定，与位置无关。即使结果为
unavailable、failed 或 unknown，Settled 仍保留该引用。在先行准入决策之后、初次共同修改前，
检查两个方向及精确保留引用/绑定。
提交前组合不一致只退役 Workbench，不修改终端/槽位/效果：OBWP 返回 service-unavailable，
Open 返回现有 server-busy 结果。提交后不一致则使用上述关闭载体规则。

Open 和 tab.create 共享简单成员构建器与首槽位。如果 Open 先胜出，旧修订命令被拒绝；
如果 tab.create 先胜出，Open 加入已有 Creating 槽位，不产生另一引用或启动。
首终端的每种最终结果都先结算槽位，再唤醒全部等待者。已结算 Open 在无关 Workbench 退役后仍可读取。
退役阻止后续 Workbench 读取、命令及重放，但不取消先前有效的完成，也不会被清除。
Worker 更换从不替换引用、绑定或启动。这些所有者仍属于进程本地状态。

| Worker 观察                                               | 最终嵌套终端结果                           |
| --------------------------------------------------------- | ------------------------------------------ |
| 移交前已知队列背压                                        | unavailable / server-busy                  |
| 明确投递不可用，包括投递前代次错误                        | unavailable / terminal-runtime-unavailable |
| 已关联的 Host 拒绝或失败                                  | failed / terminal-create-failed            |
| 精确接受立即启动、请求几何，且固定 Worker 仍为当前 Worker | started                                    |
| 收到 Launch/Start 后发现当前 Worker 不匹配                | outcome-unknown                            |
| 投递不确定或移交后观察不匹配                              | outcome-unknown                            |

Worker IPC 和 Client 结果写入都在释放注册表与 Site 门闩之后发生。启动或响应投递停滞期间，
另一个已接纳 Client 仍能完成快照读取或纯导航命令。现有 Worker 载体仍串行处理一个进行中
和一个排队操作；不承诺同一 Worker 的启动停滞期间，viewer/终端操作能够完成。

### 创建证据与边界

| 声明                                          | 必需证据                                                                                                                          |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| 一次共享提交先于至多一次固定启动              | WorkbenchTabCreate 模型；Open 先胜、命令先胜、双命令及全部结果核心轨迹；停驻 Launch 的进程见证                                    |
| 预检拒绝不改变所有者及分配状态                | 每个优先级边界、保留记录/引用/标识/租约容量、候选发布与账本计费的核心案例；丢弃计划及代次隔离案例                                 |
| 精确重放保持效果数与不可变响应                | pending/final 重放、语义冲突、pending 响应投递、响应丢失及再次投递、退役；模型中的有界独立响应飞行状态                            |
| 创建 pending 期间的 rename 经最终结算后仍保留 | read/create/rename/read/settle/replay 进程轨迹及有界 rename-composition variant                                                   |
| 完成不能修改外来或部分组合                    | 相同状态不同所有者的令牌测试、过期/混配关联案例、编译器约束的借用计划和私有 Site 包装；两个准备失败边界                           |
| Pending 预留始终覆盖结算                      | 精确规范字节/最大最终结果及账本条目边界；带确定性缩减与重放的生成 begin/commit/replay/dispatch/settle/retire/capacity 轨迹        |
| 首槽位对应关系及等待者结算                    | 空/非空不一致测试、两种 Open/create 顺序、全部最终结果、退役后的有效结算，以及穿过共同结算的有界静默 Worker 超时                  |
| 其他 Client 进展且不产生第二次启动            | 停驻启动期间分离的 OBWP 快照/重放/导航流；预先附加 OBCS viewer 的 Poll 作为之后 Worker FIFO 的正向见证                            |
| 具体集成与抽象一致                            | 严格 Rust/TypeScript corpus、所有者/核心测试、受控 Server 进程轨迹、原生双 Tab PTY 与第二 Client 刷新、独立实现审查及选定仓库门禁 |

WorkbenchTabCreate 只检查安全性。三个注册 variant 都使用三个请求标识、两个语义指纹、
至多两个 Tab 和两个账本条目、三个抽象字节的 pending 计费与两个抽象字节的 final 计费，
以及一个独立重放飞行状态。Capacity variant 允许四个抽象账本字节；concurrent variant 允许六个，
从而两个命令可以同时 Pending 并以任意顺序结算；两者均设置 `MaxRenames = 0`。
Rename-composition variant 允许四个字节和一次抽象外部 rename，保留每个 create 的已提交 result
revision，并覆盖 completion 前后的 pending/final replay。它固定 Worker available、base revision
为零及 Started completion，因此其他 preflight 和 outcome branch 仍由原 variant 负责。必需覆盖只合并同一模型的这些 variant。它抽象 JSON 字节、具体指针分配、借用检查、
锁/条件变量调度及两个所有者的 Rust 发布；核心和进程测试负责这些细化。
WorkbenchNavigationCommit 继续拥有纯导航；LocalStandaloneTerminal 拥有详细 Open 加入/等待/超时。
现有 LocalWorkerObcnCarrier 和 LocalTerminalCoordinator 的截止时间/结算性质，
在各自声明的弱公平假设下拥有进展。仅等待者超时不能证明 pending 命令结算；
集成静默 Worker 轨迹必须将其有界结果送入两个完成所有者，并通知首槽位。

Linux/WSL 集成证据覆盖 40 个库测试、102 个二进制测试、26 个 Client 进程测试、4 个 Worker
进程测试和 3 个权威集成测试，且已构建所需 Worker 与 Terminal Host 前置产物；所有 target、
所有 feature 的 Clippy 也已通过。集成静默 Worker 用例使用生产环境的 20 秒响应超时，先结算
两个 owner，再通知首次 Open 的等待者。暂停 Launch 的轨迹在释放 Worker 前，从另一个 Client
观察第二个规范 Tab、待处理重放及导航提交，随后校验最终重放，并用确切的 viewer Poll 后续记录
排除重复派发。这些是受控进程证据，不代表原生 Client Tab 交互或崩溃/重启证据。定向检查不等于
根验证门通过。

schema、两种绑定及消费它们的二进制作为一个尚未发布的 v1 激活批次共同推进。
旧的封闭解码器拒绝扩展。回退恢复整个批次；不声明混合版本发布、持久迁移或跨重启重放。

Panel swap 使用共享结构 placement 权威，在同一 Tab 中原子交换完整叶子，保留 Split／slot 及终端身份。Windows CLI 通过原 Client 单独确认源聚焦；后续失败或聚焦未知不抹去已知 canonical 提交。[原生 Panel 交换](../../orchbee/docs/native-panel-swap.zh-CN.md) 拥有工作流程、回执及证据。

## 自动 Tab 命名

[Tab 标签契约](../../orchbee/docs/native-tab-labels.zh-CN.md) 拥有可选规范模式、兼容性和消费者细化。Server 将 `tab.create` 的 `automaticLabel` 纳入请求指纹，原子提交到 Tab；省略为固定名称，隐式首个独立 Tab 则使用自动模式。移动与关闭保留幸存命名元数据。重命名同时设置文本和固定模式；待处理创建的结算与重放绝不恢复旧模式。抽象标签比较包含文本和模式，因此关闭自动命名即使文本相等也属于 changed。

## Panel 展示元数据

`WorkbenchAuthority::panel_metadata` 按逻辑终端绑定（`workerId`、`terminalId`）拥有临时上报。
旧上报解析 Panel 当前的主绑定并要求精确匹配 `expectedBinding`；目标已变化、缺失或未绑定时，
在应用上报前失败。主成员及子成员关系让资源在移动和交换后保留。操作会清理已不属于任何
已提交成员的绑定。Site 释放或重启后丢弃这些状态。

输入是严格的 `PanelMetadataRequest`，包含规范化的来源、显示文本、token 键、可选序号和
1–86,400,000 ms TTL。读取接受 0–4 个有序且唯一的 Panel ID，返回绑定、tokens 和按来源保存的
展示记录。来源和文本复用 Workspace 元数据的规范形式。显示状态为 `idle`、`working`、
`blocked`、`done`、`unknown`。Agent 名称和权威来源守卫精确比较实际观察到的事实；缺少上下文
永不匹配。`hook` 等通用观察类别不是权威来源标识。

| 转移                         | 守卫和结果                                                                             |
| ---------------------------- | -------------------------------------------------------------------------------------- |
| 新上报设置展示字段且没有清除 | 替换该来源的整个展示记录，包括守卫和省略字段；给提供的字段分配写入顺序                 |
| 上报包含清除标记             | 仅清除指定字段，应用提供的值和守卫，保留未触及字段的优先级；不能同时设置和清除同一字段 |
| 展示 TTL                     | 设置值或显式 TTL 刷新来源展示记录的过期时间；仅清除且没有 TTL 时保留原期限             |
| Token 补丁                   | 更新或移除指定的全局终端 tokens；各值拥有独立期限；无关键保持原期限                    |
| 旧或相等的来源序号           | 返回 `stale`，不修改也不刷新期限；省略序号不会重置高水位                               |
| 读取或上报时间边界           | 根据传入的单调时间让值过期；序号守卫保留到绑定释放                                     |
| 容量或受检修订号／期限失败   | 拒绝整次上报，不消耗序号，也不部分应用字段                                             |

有效展示分别为标题、Agent 显示名称及各状态标签选择最新且符合守卫的值。清除无关字段不能
提高旧标题的优先级。元数据使用独立的受检修订号，不递增规范 Workbench 修订号。过期和清理
是有效操作前独立的维护转移。上限是每绑定 32 个来源、32 个 token 键，共 4,096 个保留绑定及
16 MiB 已计费保留负载；计费不是进程 RSS 上限。仅有序号的记录也占用来源限额。

Site 锁是唯一写入者，并在准入后采样时间。此 reducer 不含 I/O、后台过期计时器、持久化、
规范拓扑变更或通知事件。读写 DTO 及确定性 owner 测试覆盖替换、选择性清除、逐字段优先级、
守卫、序号重放、TTL 边界、原子容量失败、修订号耗尽以及主／子绑定生命周期。兼容参考是
Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的展示和 token 行为；provider 状态权威、
可恢复会话和 TUI 渲染仍是独立消费者。元数据上报不会建立 Agent 权威，也不会修改 OSC 终端标题。
公开 CLI 集成需要通信层和查询投影；仅 owner 本身不代表 CLI 已对齐。

[Agent 上报模型](agent-report-model.zh-CN.md) 拥有显式上报转换、有界物理代际注册表和 Windows
上报／读取承载。可信进程操作在本提交门下一起准备上报与元数据清理，保留 token 和较新的
自定义展示；之后匹配已退出 Agent 的元数据返回过期。规范观察保持原有所有者。自动 Host
进程生产者、名称和联合查询／TUI 上下文仍待集成；上报协议不能声明进程存在。

## 结构操作保留子 Presentation

Panel 即使拥有子 Presentation，仍是布局叶子。已绑定树成员按主项、各有序子项的顺序枚举，并保留每条精确 binding。`member_for_panel` 选择主项；成员数量是 Presentation 数量，不是 Panel 数量。旧的单叶成员入口仍拒绝额外 Presentation 及分组／固定 Tab。完整树成员在规范验证下包含分组／固定元数据。未绑定成员保留在规范 Panel 树中，但不进入运行时绑定向量；显式主成员标记防止已绑定子项满足未绑定主项的 `member_for_panel` 查询。部分成员未绑定时，Agent 注册仍可观察存续的已绑定成员。

规范占用状态与已绑定成员集合分开。引导要求上下文没有 Tab；Open 遇到完全未绑定的布局时返回 `terminal-runtime-unavailable`，不启动、不退役。
显式 Tab Create 或 Split 可以建立首个已绑定 standalone 引用。Close 根据规范 Tab／Panel 地址及拓扑确定范围，保留未绑定兄弟项；完全未绑定的范围使用保留的纯发布 Close。
混合成员及空范围用例覆盖注册、Open、分屏结算、重放、精确发布及 standalone 重定向。

Tab 创建和排序允许上下文包含子 Presentation。Panel 分屏将完整原 Panel 与一个新兄弟项包装为分屏树。同 Tab 移动／交换、跨 Tab／上下文迁移均转移完整 Panel 节点，包含有序子项身份、标签、binding 和输入策略；不重建、换绑或丢弃现存 Presentation。新 Workspace 的目录选择维持既有主终端所有权，不因迁移启动额外进程。迁移检查 Panel 每个成员是否已被未完成 Close 占用。Standalone 引用协调在既有 Site 提交门内验证前后完整 binding 集合。

证据比较分屏、移动／交换、迁移后的完整 Panel 内容，覆盖带子项的 Tab 创建及排序、主项与子项身份区别、精确 Standalone 引用组合与既有 Close 栅栏。原生子项选择／导航、子项创建、分组／固定属性创建和修改、未绑定成员生命周期仍为独立未完成义务。没有线路或检查点迁移，通过匹配 Server cohort 启用或回滚行为。

## 分组和固定项的结构操作

Workbench 的已绑定结构操作接受规范分组元数据和固定前缀。Tab 创建在末尾追加一个未分组／未固定 Tab。Panel 分屏、移动、交换及迁移保留完整既有 Panel 内容与存续 Tab／分组元数据；移除最后一个 Panel 时移除其 Tab，移到新 Tab 时创建未分组／未固定目标。空分组仍是独立元数据。这些操作均不隐式固定、取消固定、分组或取消分组。

Tab Move 先根据原方向或原始插入边界确定精确置换；no-op 在分配修订之前返回不变。候选必须保留固定前缀及每个分组的连续成员，否则保留 `tab-move-order-conflict`，不修改状态或退役权威。这是 Herdr 精确移除／插入语义之上的 Orchbee 元数据约束（固定 `e0507237ad8d41f60aece10d00ef29ce2339ca34`，`workspace.rs::move_tab`）；Herdr 本身不定义这些分组／固定语义。请求重放、上下文 Close 栅栏、Site 完整成员组合、容量及回滚沿用既有所有者，不发生进程操作。

证据通过独立移除／插入及连续区间参考算法比较完整记录，覆盖合法／非法位置、全部方向、所有插入边界、修订耗尽时的 no-op、精确拒绝重放及后续合法操作。恢复后的分组／固定 Site 组合，以及保留子项的创建／分屏／布局／迁移仍是必要集成见证。Rust／TypeScript／线路 schema 和原生失败分类同时接纳新原因。原生索引失败采用 v2，schema v1 保持封闭；方向操作仅针对新拒绝使用专用 v2 信封。匹配协议／Server／Client 的启用或源码回滚不补偿已提交顺序。分组／固定创建 UI、未绑定生命周期及真实 Windows 多 Client TUI 验收仍为独立义务。

## 精确 Presentation 元数据读取

Panel 元数据 `read-presentations` 按请求顺序接受最多四个唯一 Presentation ID，包括主项、子项以及空读取修订校验。每个 `presentations-read` 行包含 `presentationId` 和嵌套 `panel` 行，携带所属 Panel ID、所选成员绑定、tokens 及来源记录。不同成员可共享 Panel ID。未知成员拒绝整批；未绑定成员展示空元数据。两个读取路径同样要求来源唯一、字段次序非零且不超过元数据修订号。成员关系和绑定所属资源均在既有 Site 门内解析；读取不修改拓扑、选择或输入归属。既有主 Panel 读取与元数据写入保留原范围。

## 精确 Presentation 元数据写入

`report-presentation` 为既有元数据载体增加必需的 Presentation ID。Site 门先证明精确的
Panel 所属关系及非空、与期望一致的绑定，再应用来源守卫、序列顺序、token 补丁或 TTL。
流程复用绑定所属资源，主项和子项记录保持隔离，移动后仍保留同一终端数据。错误所属／
成员、未绑定或过期绑定返回明确失败，绝不重定向写入。`presentation-reported` 回传精确
身份与两类修订号，包括过期无操作报告。旧 `report` 仍仅针对主项。两种写入均要求导航权限。
共享契约与所有者轨迹覆盖关联、隔离、重放及拒绝；既有 TTL 和资源计费仍是权威。原生
[元数据命令](../../orchbee/docs/windows-panel-metadata.zh-CN.md#精确成员目标)拥有 CLI 选择及
未知结果。配套 cohort 发布不新增持久化迁移。

## 在既有 Panel 内创建终端

`panel.create-terminal` 追加子项或绑定精确的未绑定主项／子项。共享的 `terminal_panel_create`
权威拥有分屏及成员放置；放置类型只为真实分屏保留分割身份。分屏／子项／绑定分别分配五／一／零个身份。
规范 Panel、既有成员顺序、标签及覆盖值均保留。16 个子项上限、精确成员资格、当前修订、导航及
创建权限、Workspace Worker／目录、Close 范围和配置档案解析都是前置条件。叶子 Close 阻止
在该 Panel 内创建，聚合 Close 阻止对应 Tab。待结算创建对实际终端所属 Panel 设置迁移栅栏，
包括接收新子项的既有 Panel。

Site 冻结 cwd／环境变量／程序／来源，预备规范状态和终端计划，并在任何提交前检查全部关联。
随后转移两个所有者并派发；首个已绑定 standalone 成员安装 Creating 引用。既有预构造结果额度
和不透明结算凭证保持适用。新增待处理／最终结果携带 Context／Tab／Panel／Presentation，不伪造
分割字段。未知启动结果保持未知，精确重放不再次启动，已知失败保留已提交成员。定向证据覆盖身份
额度、上限／拒绝、丢弃预备计划、异源结算、主／子项绑定、字面值派发、首引用结算、范围栅栏及服务权限。
该私有创建路径是布局应用及成员控件的基础，消费方接入仍是独立待办。

## 未绑定布局创建

`tab.create-layout` 在一次规范事务中创建完整 Tab 结构。不含身份的布局包括主成员和有序子成员的标签、
可选手工标签、Panel 右键策略及二叉 Split 方向。Server 分配所有 Tab、Panel、Presentation、Split 和
槽位身份；所有 Presentation 初始均未绑定。不创建、不复用、也不要求 Worker 或终端。
导航授权允许创建结构；随后启动指定成员走独立的 `panel.create-terminal` 生命周期，要求导航及终端创建授权。

模板最多允许 24 个 Panel、含根节点的 16 层以及每个 Panel 的 16 个子成员，命令信封上限为 1 MiB。
现有 Context、Tab、投影、身份及保留请求限额仍然适用。进行中的 Context Close 阻止创建。
修订检查、发布、身份分配和最终 `tab-layout-created` 回执一起提交；完全相同的请求重放返回相同身份，
不产生另一次副作用。标签和输入策略在结构创建后保留。Client 焦点、比例、分组、固定和物理启动由各自
的所有者处理，不由此回执暗示。完全未绑定的 Tab 参与检查点、选择及零终端 Close。
