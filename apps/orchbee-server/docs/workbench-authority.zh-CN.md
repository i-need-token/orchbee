# 原生 Workbench 权威

[重启检查点契约](workbench-checkpoint.zh-CN.md) 拥有保留配置和新权威重建；它本身不启用磁盘
持久化，也不恢复终端进程。

## Close 权威与本地集成

[规范终端 Tab Close](terminal-tab-close.zh-CN.md) 拥有移除前的已知 Host 释放及准确目标清理、预留修订／账本／Resume 容量，以及基于当前成员关系的发布。清理期间其他规范写入可继续。移除最后一个 Tab 留下空 Workspace；关闭最后一个 Workspace 留下空 Workbench。新的执行要求显式 Workspace／Tab 创建。

Server 库实现了下述保留权威和续接内核。Site、精确 controller/Worker 清理及进程生命周期
维护所有者将其与本地 Close/Read/Resume 入口组合。确定性及 Linux 协议客户端进程测试覆盖
credit、保留控制、当前快照删除、空闲预留释放以及八个活动 Client 连接。Close 记录另有
原生确认及成员关系/Empty 运行时的 Linux TUI 进程证据；仅凭这些 Server 检查不能证明它们。
已安装 cohort 的收敛仍未完成。

## Close 权威运行时细化

已接受的实现切片在现有命令账本中用单个 `CloseRecord` 保留每个主 Close。它拥有不可变命令、
固定确认、完整 Workspace 成员及已验证的 Worker generation/Host 身份/fence、尝试 generation、最后一次
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
交付前，Site 和进程测试必须证明联合关联、清理及实际 carrier/Host 边界。不保留独立 Close 流程
TLA 模型。`cargo test -p orchbee-server --lib --locked` 运行这些
权威/续接测试；`cargo clippy -p orchbee-server --lib --locked -- -D warnings` 检查库实现。
这些聚焦检查不替代根门禁或所属跨进程证据。

## 已接受的 Panel 分割与聚合 Tab Close 细化

`panel.split` 是第一项已接受的树形变换。Workbench 验证确切的上下文、Tab 和 leaf Panel，限制
变换后的树大小，并在派发第二个 terminal launch 前提交源 leaf、新 leaf、split 方向、两个 slot
identity 和 presentation identity。Server 与 Site 将所有 leaf binding 作为一份确切的树成员关系保留；
任何缺失或 Unknown 的 leaf 结果都会隔离待处理操作，不能发布部分树。

聚合 Close 捕获 Tab 的完整有序已绑定成员，释放并清理每个目标，全部证明已知后才发布。叶范围的 `panel.close` 移除该叶并折叠父 Split；唯一叶关闭整个 Tab。正常授权／重放／修订／上下文检查后，聚合范围用暂时 `terminal-busy` 围栏阻止重叠 split／move，无关 Tab 可继续。发布基于当前成员关系重新证明捕获范围。原生成员恢复附着存活叶，或保留可重试不可用状态；协议／期限失败沿用既有退役规则。临时 TUI 分屏、关闭、移动快捷键使用此权威。

## 已接受的 Panel 迁移模型

`panel.relocate` 将完整 Panel 移至另一 Tab、已有 Workspace 中的新 Tab，或 Project 成员关系可空的新 Workspace。Workbench 拥有拓扑；Client 视图权威拥有 zoom 与焦点，规范分割比例保留既有所有者。迁移不启动、停止或重新绑定终端，也不删除目录或 Git 树。

源元组和预期修订必须精确。现有目标形式保留默认源上下文；显式目标上下文支持跨上下文迁移，
可选目标 Panel 指定嫁接叶子，缺省保留现有根部嫁接契约。新 Workspace 指定可空的 Project 归属、
Site 目录池中的可选 Directory 及标签。新增 `primaryDirectoryPath` 可在同次提交中创建 Directory，与已有目录 ID 互斥。本地服务通过当前 Worker 探测路径、复核其代次，并要求目录关联权限；已验证 Worker 必须拥有保留的来源终端。缺少证明、容量不足或路径失败均不能发布拓扑。新目录多分配一个身份，并由 `transfer.createdDirectoryId` 返回；仅分配新结构与目录身份。移动叶子保留 Panel、Presentation、绑定、自定义标签和
输入策略，包括全部有序子 Presentation。未绑定成员的生命周期仍是单独跟踪的限制。

| 守卫或转换                                     | 可观察结果                                                             |
| ---------------------------------------------- | ---------------------------------------------------------------------- |
| 源／目标缺失、目录归属错误、修订过期或容量耗尽 | 拓扑和身份分配发布前拒绝。                                             |
| 目标是源 Tab                                   | 保留带类型的 same-Tab 拒绝；CLI 工作流负责参考实现的无变化投影。       |
| 源仍有其他叶子                                 | 脱离叶子并折叠其父 Split。                                             |
| 源只有该叶子                                   | 移除源 Tab，但不释放终端。                                             |
| 跨上下文移动使源 Workspace 变空                | 移除 Workspace／上下文及显示顺序项，保留 Project／目录和全部终端绑定。 |
| 同上下文移动                                   | 保留上下文；达到集合上限时，新 Tab 可以替换刚移除的最后叶子 Tab。      |
| 目标为现有 Tab                                 | 在指定叶子或根部嫁接新 Split 和两个 slot，检查深度及结果节点上限。     |
| 目标为新 Tab 或 Workspace                      | 追加新 Tab；新 Workspace 追加到显式显示顺序，归属明确。                |
| 生命周期操作与源／目标或待结算的移动终端重叠   | 临时 terminal-busy，不保留成功或部分移动。                             |

克隆候选在账本准入前完成校验与编码。单次提交发布快照、身份高水位及准确结果。重放返回该结果；冲突与发布失败不能分配另一次移动。迁移事实标识目标 Context 及被移除／创建的 Tab／Workspace 身份。全 Site 绑定多重集合在共享提交 gate 下保持不变。

公开 Windows 集成还必须解析所选 Client 的目标叶子、处理缩放／无变化，并通过该 Client 应用比例／焦点。
这些阶段分别确认，后续失败保留已知拓扑提交。结构测试覆盖决策表、守恒、重放和有界交换，
共享线上夹具覆盖两种语言绑定。完整用户验收要求真实 Windows 多 Client 移动、shell 存活、刷新和重启。
通用语义事件继续由独立事件契约负责。

## 已接受的 Workspace 排序模型

用户重排整个 Site 的 Workspace 列表，不移动 Project、目录、终端上下文或运行中的终端。
Navigation 保留按身份排序的成员关系，以及可选的完整 `workspaceOrder` 排列；缺省／空列表采用
全局 Workspace 身份顺序。历史 checkpoint 迁移将原来的 Project 遍历顺序实体化为显式排列。
显示顺序独立于可选的 Project 归属，支持跨分组的块移动。

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

移动使用原始插入边界、按调用者顺序收集不连续块、原子验证和逻辑选择保留。
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

### Workspace 上下文扩展

Move 解析由准确 Workspace 拥有的终端上下文。未分组 Workspace 的 Project ID 为 null，与已分组 Workspace 具有相同变更权限。Move 不要求已接受的 Directory 或在线 Worker。模型没有预留上下文、首终端槽位或需要协调的 Open 引用。

导航授权、退役、保留结果重放／冲突、账本容量、Site 和预期修订检查先于上下文与目标查询。待完成的聚合 Close 围栏阻止成员关系变更；边界上的不变移动保留修订。候选校验、发布和结果字节预留均在单次提交前完成。精确重放即使在后续写入后也返回原始顺序和修订；同请求 ID 的内容变化产生冲突。移动 Tab 保留每个 Panel、Presentation、绑定、命名模式以及分组／固定不变式，不分发 Worker 效果。

## 已接受的多叶成员模型

每个终端上下文恰好属于一个 Workspace。成员枚举按规范 Tab／树顺序遍历主、子 Presentation，保留完整 Context／Tab／Panel／Presentation／Worker／Terminal 元组。无绑定主 Presentation 不能借用子成员的绑定。只读清单可以保留无绑定布局，要求完整绑定的操作执行各自严格准入。

Split、迁移和聚合 Close 保留无关叶及准确运行时身份。Close 在效果前捕获范围，仅在已知释放与准确清理后移除，并在发布时基于无关的已提交写入重新计算。移除最后一个 Tab 留下空 Workspace；移除最后一个 Workspace 留下合法空 Workbench。两种转换都不创建终端。

待完成创建结算始终关联原操作 token，包括其他 Tab 移动或关闭之后。精确重放及过期／外部结算检查必须保留原终端结果。保留的权威、服务和进程测试使用真实 Workspace 所有者覆盖这些交错。

## 范围与所有权

原生 Server 为每个 Site 拥有一个规范 Workbench。Project 保存组织元数据；Workspace 与 Directory 属于 Site。导航保存全局按身份排序的注册表及精确可选显示顺序排列。每个 Workspace 恰好拥有一个 Context，其 Project 或主 Directory 可为 null。零 Workspace／Context 合法。每个 Context 必须有所有者；旧无所有者导航被拒绝。数量、路径、身份唯一性和引用在发布前校验。

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
其 [Panel 标签契约](../../orchbee/docs/native-panel-labels.zh-CN.md)保持创建时回退标签不可变，
并定义有界规范化设置／清除、无操作及投影语义。[Panel 标签权威测试][panel-label-tests]覆盖
规范化、设置/清除、重放、缺失目标、授权及容量，并保持绑定及回退标签。

| 字段       | 契约                                                                                                                                                                                                                                                                                                             |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题       | 独立准入的 Client 必须修改和观察同一份共享导航结构，避免丢失更新或重复逻辑效果。                                                                                                                                                                                                                                 |
| 输入       | 严格 snapshot 或 publication 读取 intent，或包含 request identity、确切 Site、预期 revision 及已准入的纯 action的严格命令。创建标签与 Tab 标签保留原始非空文本，最多 512 个 Unicode code point；窗格自定义标题按上文规范化有界可空更新。Tab rename fingerprint 包含 context、Tab、label 和 base revision。       |
| 数据       | Active 或 retired 权威、一份已校验 Snapshot、Server 拥有且不复用的 identity cursor，以及有界 request/result ledger。初始 revision 为零，Workspace Terminal Context 为空。Adapter 从操作系统随机源取得非零 Site identity。Cursor 不复用导航聚合内的身份；无关 Terminal/operation 身份域保留现有 owner。           |
| 输出       | 读取返回相关联的 available { snapshot }、denied {} 或 unavailable { reason }。Publication 读取返回 current { revision }、published { publications }、denied {} 或 unavailable { reason }。命令返回带类型的导航提交、unchanged、rejected、denied 或 unavailable。新身份仅出现于对应成功变体中。                   |
| 不变量     | 只有 Server 提交共享结构。每个 Workspace 恰好引用一个 context。拒绝保留结构、revision 和分配状态。发生变更时 revision 恰好加一。固定名称的等值重命名保留 revision。Tab rename 只改变目标 `Tab.label` 和 `Tab.automaticLabel`；非目标标签及所有其他 projection 事实保持精确。一个 request identity 不能提交两次。 |
| 决策       | 授权先于 retired 状态及 ledger 检查。Active 精确重放先于预期 revision 检查。保留的 request identity 对应内容改变时是冲突。拒绝命令没有结构效果。意外内部 invariant 失败会原子停用权威；后续所有已授权读取和命令（包括重放）都不可用。                                                                            |
| 模型       | 对完整 candidate 执行确定性串行转换；网络、时钟、文件系统、随机源及进程效果留在 adapter。                                                                                                                                                                                                                        |
| 依赖       | 既有 Workbench snapshot 校验及上限、严格 OBWP framing、已准入本地 Session。这些导航操作不需要 Worker 或 Host。                                                                                                                                                                                                   |
| 所有权     | 权威一起持有 snapshot/cursor/ledger。Service 拥有初始化及调用串行化。Application-session adapter 拥有协议选择及释放锁之后的有界响应交付。                                                                                                                                                                        |
| 验证       | schema 定义的 Rust/TypeScript 一致性、可缩减的确定性有状态命令轨迹及真实双 Client Server 进程证据覆盖导航与重放行为。                                                                                                                                                                                            |
| 正确性限制 | 确定性测试覆盖具体标签、身份编码及 JSON；生成轨迹及进程调度是有限的。Linux 进程 cohort 需要其原生前置条件。仅此转换组不证明持久化、传输重试、重启身份或完整 Client 集成。                                                                                                                                        |

| Action                   | Guard                                                             | 提交结果                                                          |
| ------------------------ | ----------------------------------------------------------------- | ----------------------------------------------------------------- |
| project.create           | Label 和 projection 容量被接受                                    | 一个没有 Workspace 的新 Project                                   |
| project.rename           | 确切 Project 存在                                                 | 修改其 label；等值文本为 unchanged                                |
| workspace.create         | 可选 Project 存在且 context 容量被接受                            | 一个新 Workspace 及其唯一空 Terminal Context                      |
| workspace.rename         | 确切 Workspace 存在                                               | 修改其 label；等值文本为 unchanged                                |
| workspace.set-project    | 确切 Workspace 及可选目标 Project 存在；改变归属时没有 Close 围栏 | 仅修改可空的 Project 归属；相同归属为 unchanged                   |
| workspace.create-project | 确切 Workspace 存在，没有 Close 围栏，且标签与容量被接受          | 原子创建 Project 并将该 Workspace 移入                            |
| project.dissolve         | 确切 Project 存在，且其成员没有 Close 围栏                        | 移除 Project 并取消其 Workspace 分组，保留执行资源                |
| tab.rename               | 确切 context 包含确切 Tab                                         | 修改其 Tab label 并关闭自动命名；仅固定模式的等值文本为 unchanged |
| panel.rename             | 精确 context/Tab 包含目标 Panel                                   | 主 Presentation 自定义标题设置或清除；规范化后相等则不变          |

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
| Workspace 缺失或不适用于所寻址的操作                            | rejected: workspace-unavailable                                                |
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

| 主张                                              | 必需证据                                                               | 边界                                                       |
| ------------------------------------------------- | ---------------------------------------------------------------------- | ---------------------------------------------------------- |
| 竞争预期 revision 只提交一次                      | [权威轨迹][authority-tests]与[双 Client 进程竞争][authority-process]   | 受控竞争请求；进程内串行权威                               |
| 读取观察一份已提交状态；拒绝调用不泄露状态        | [publication 历史及 grant 测试][authority-tests]                       | 读取在 commit gate 下克隆时线性化                          |
| 停用阻止后续读取、写入及重放                      | [授权/退役测试][authority-tests]                                       | 进程内停用，不声明恢复                                     |
| 慢 snapshot writer 不阻塞其他 Client              | [阻塞 writer 的 application-session 测试][session-tests]               | 受控阻塞 writer；响应 I/O 位于克隆及释放锁之后             |
| 响应丢失后精确重放不产生新效果                    | [重放/冲突轨迹][authority-stateful]与[断连进程轨迹][authority-process] | 仅保留的 Site 进程 ledger                                  |
| Tab rename 只改变目标标签，后续提交后仍重放原修订 | [具体 rename 测试][authority-tests]与[有状态轨迹][authority-stateful]  | 精确目标、Unicode 及字节案例；有限生成历史                 |
| 导航保持有效且拒绝无效果                          | [有状态命令轨迹][authority-stateful]及 snapshot validator              | 既有 snapshot collection、identity 和字节上限              |
| 跨语言契约含义一致                                | Rust 与 TypeScript 消费共享导航命令 corpus                             | 严格已解码 JSON 及编码 carrier 上限                        |
| 大 snapshot 保持 framing 与方向                   | OBWP codec 和[进程轨迹][authority-process]                             | 请求/命令结果 16 KiB；snapshot 结果最多 1 MiB + 1,024 字节 |

从仓库根目录运行：

```sh
cargo test -p orchbee-server -p orchbee-workbench-protocol --locked
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol typecheck
cargo test --locked -p orchbee-server --all-targets --all-features
pnpm verify
```

[有状态权威测试][authority-stateful]运行 96 个固定种子，每个 80 步，支持重放及删除缩减。
它覆盖导航、精确重放、冲突、过期修订、拒绝准入及读取；[确定性测试][authority-tests]覆盖
具体目标解析、Unicode 标签、身份分配、publication 历史及分离的条目/字节预检边界。
[application-session 测试][session-tests]与[Server 进程轨迹][authority-process]负责传输及竞争
Client 见证。这些有限轨迹不是穷尽交错证明，仅通过核心或 binding 测试不等于完成原生进程见证。

## Workspace 终端成员模型

新权威包含零个 Workspace 和零个终端上下文。`workspace.create` 创建一个真实 Workspace 及其空上下文；`workspace.launch` 另外提交其首个终端。Workspace 的 Project 成员字段必填且可为 null。导航与上下文是一一对应关系，上限为 1,024 个 Workspace 和上下文，不存在无身份的兼容所有者。

终端创建使用 Workspace 已接受的 Directory 及准确 Worker。缺少目录配置或运行时不可用时返回类型化拒绝，不回退到 home、不创建隐藏 Workspace。规范快照通过绑定支持 `attach-binding`，附着不授予创建或导航权限。

成员关系准备与终端预留在 Site gate 下联合提交之前均无效果。提交前失败保留身份、修订、账本和终端状态。创建、关闭、移动及检查点恢复均不再参与任何保留的单例 Open 状态。

## 从 Workbench 绑定选择终端

已准入本地 Client 通过 `attach-binding` 选择规范快照中的准确 Site／Worker／逻辑 Terminal 绑定。Workbench 在运行时查询前证明当前暴露关系；Site 随后检查保留 Worker 代际及 Host 身份。选择不授予创建、导航、输入或 resize 权限。成员缺失、运行时不可用及附着身份变化仍是不同类型化结果。

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

[Session 路由所有者](../README.md#accepted-connection-viewer-routing-modeling-record) 先回收精确
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

附着证据须覆盖外部 Site 与成员缺失优先级、保留代际／Host 身份、准确主／子选择、终端失败／未知结果、退役及附着竞争。必须证明选择期间没有启动或变更，并且无关规范成员保持不变。

Client 读取错误与结构选择规则归 [Workspace 启动](../../orchbee/README.zh-CN.md#workspace-终端启动)所有。类型化身份域及当前快照成员关系始终是权威，标签或行号不能替代它们。

## Workspace Tab 创建模型

`tab.create` 要求导航与终端创建授权、准确 Workspace 上下文，以及由所选 Worker 拥有的已接受 Directory。规范命令预检保留授权、退役、精确重放／冲突、容量、Site 和修订优先级。放置操作校验目标 Tab／分组及结果集合／树上限。几何、启动 Profile／选项和目录事实在变更前冻结。

### 准入与联合提交

Workbench 准备候选 Tab／Panel／Presentation、发布、身份及待完成／最终账本预留。终端协调器独立准备准确运行时预留。Site 在同一 gate 下先校验两份计划，再提交两者；Worker I/O 在释放 gate 后执行。丢弃准备无效果。提交前的暂时 Worker／资源拒绝不保留成功。已知启动失败保留已提交成员并报告失败；不确定结果保持未知，绝不自动二次启动。

### 完成与重放

绑定来源且消费一次的 token 标识原待完成命令、保留运行时请求及准确绑定。完成在发布最终结果前校验这些事实。过期或外部 token 不能结算其他请求。Close 或迁移可以改变当前成员关系，但不能重写历史 Create 结果；重放返回原结果且不分发 Worker 效果。提交后的内部不一致关闭混合载体，不伪造结果。

### 创建证据与限制

保留的权威／服务测试覆盖无效果准备、目录／Worker 准入、放置与资源上限、精确重放、失败／未知结算以及与 Close／Move 的并发。进程证据使用 [Workspace 终端操作][workspace-process]、[暂停的 Tab 创建][create-process]、[创建期间重命名][rename-process]及[多叶交错][multi-leaf-process]。它们要求支持的 Linux 进程 cohort 与 Worker／Terminal Host 前置产物；Windows 检查或交叉目标编译不会执行它们。测试源码及旧候选结果不代表变更后的候选已经通过最终门禁。

## 自动 Tab 命名

[Tab 标签契约](../../orchbee/docs/native-tab-labels.zh-CN.md) 拥有可选规范模式、兼容性和消费者细化。Server 将 `tab.create` 的 `automaticLabel` 纳入请求指纹，原子提交到 Tab；省略为固定名称，Workspace 启动则请求自动模式。移动与关闭保留幸存命名元数据。重命名同时设置文本和固定模式；待处理创建的结算与重放绝不恢复旧模式。抽象标签比较包含文本和模式，因此关闭自动命名即使文本相等也属于 changed。

## Panel 展示元数据

`WorkbenchAuthority::panel_metadata` 按逻辑终端绑定（`workerId`、`terminalId`）拥有临时上报。
旧上报解析 Panel 当前的主绑定并要求精确匹配 `expectedBinding`；目标已变化、缺失或未绑定时，
在应用上报前失败。主成员及子成员关系让资源在移动和交换后保留。操作会清理已不属于任何
已提交成员的绑定。Site 释放或重启后丢弃这些状态。

输入是严格的 `PanelMetadataRequest`，包含规范化的来源、显示文本、token 键、可选序号和
1–86,400,000 ms TTL。读取接受 0–4 个有序且唯一的 Panel ID，返回绑定、tokens 和按来源保存的
展示记录。来源和文本复用 Workspace 元数据的规范形式。显示状态为 `idle`、`working`、
`blocked`、`done`、`error`、`unknown`。Agent 名称和权威来源守卫精确比较实际观察到的事实；缺少上下文
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
守卫、序号重放、TTL 边界、原子容量失败、修订号耗尽以及主／子绑定生命周期。provider 状态权威和 TUI 渲染仍是独立消费者。元数据上报不会建立 Agent 权威，也不会修改 OSC 终端标题。
公开 CLI 集成需要通信层和查询投影；仅 owner 本身不代表 CLI 已对齐。

[Agent 上报模型](agent-report-model.zh-CN.md) 拥有显式上报转换、有界物理代际注册表和 Windows
上报／读取承载。可信进程操作在本提交门下一起准备上报与元数据清理，保留 token 和较新的
自定义展示；之后匹配已退出 Agent 的元数据返回过期。规范观察保持原有所有者。自动 Host
进程生产者、名称和联合查询／TUI 上下文仍待集成；上报协议不能声明进程存在。

## 结构操作保留子 Presentation

Panel 即使拥有子 Presentation，仍是布局叶子。已绑定树成员按主项、各有序子项的顺序枚举，并保留每条精确 binding。`member_for_panel` 选择主项；成员数量是 Presentation 数量，不是 Panel 数量。旧的单叶成员入口仍拒绝额外 Presentation 及分组／固定 Tab。完整树成员在规范验证下包含分组／固定元数据。未绑定成员保留在规范 Panel 树中，但不进入运行时绑定向量；显式主成员标记防止已绑定子项满足未绑定主项的 `member_for_panel` 查询。部分成员未绑定时，Agent 注册仍可观察存续的已绑定成员。

规范占用与已绑定清单分离。无绑定布局仍被占用，附着不创建终端。显式 Tab Create、Split 或成员创建通过 Workspace 目录准入建立绑定。Close 捕获规范地址／拓扑；无绑定同级成员保留，完全无绑定范围使用仅发布 Close。混合及空范围证据覆盖注册、结算、重放和准确发布。

Tab 创建与排序接纳子 Presentation。Split 包装完整来源 Panel 和一个新同级叶；同 Tab 移动／交换及跨 Tab／Context 迁移传递完整 Panel 内容，包括有序子成员、标签、绑定和输入策略。不重新创建或绑定已有 Presentation。迁移保留 Workspace 目录所有权，检查包含无绑定成员的规范 Close 围栏，不启动其他进程。

证据比较 split／move／swap／迁移后的完整 Panel 内容，保留主／子身份，并验证 Close 围栏与绑定守恒。[原生分组](../../orchbee/docs/native-tab-groups.zh-CN.md)拥有分组／固定消费者覆盖。当前严格线协议／检查点形状拒绝旧的不兼容文档，不保留兼容所有者或迁移路径。

## 分组和固定项的结构操作

Workbench 的已绑定结构操作接受规范分组元数据和固定前缀。Tab 创建省略 placement 时在末尾追加一个未分组／未固定 Tab；typed placement 也支持相对目标创建、组尾创建、原子新组加 Tab 及精确单 Tab 复制。Panel 分屏、移动、交换及迁移保留完整既有 Panel 内容与存续 Tab／分组元数据；移除最后一个 Panel 时移除其 Tab，移到新 Tab 时创建未分组／未固定目标。除新组创建时的清理外，空分组仍是独立元数据。没有显式 Tab placement 的结构操作不隐式固定、取消固定、分组或取消分组。

Tab Move 先根据原方向或原始插入边界确定精确置换；no-op 在分配修订之前返回不变。候选必须保留固定前缀及每个分组的连续成员，否则保留 `tab-move-order-conflict`，不修改状态或退役权威。请求重放、上下文 Close 栅栏、Site 完整成员组合、容量及回滚沿用既有所有者，不发生进程操作。

证据通过独立移除／插入及连续区间参考算法比较完整记录，覆盖合法／非法位置、全部方向、所有插入边界、修订耗尽时的 no-op、精确拒绝重放及后续合法操作。恢复后的分组／固定 Site 组合，以及保留子项的创建／分屏／布局／迁移仍是必要集成见证。Rust／TypeScript／线路 schema 和原生失败分类同时接纳新原因。原生索引失败采用 v2，schema v1 保持封闭；方向操作仅针对新拒绝使用专用 v2 信封。匹配协议／Server／Client 的启用或源码回滚不补偿已提交顺序。原生分组／固定创建 UI 由[原生分组](../../orchbee/docs/native-tab-groups.zh-CN.md)覆盖；未绑定生命周期及真实 Windows 多 Client TUI 验收仍为独立义务。

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

Site 冻结 cwd／环境／程序／来源，在联合提交与分发前准备规范／运行时计划。预建有界结果与不透明结算凭据适用于主、子创建。回执携带实际 Context／Tab／Panel／Presentation 身份。未知结果保持未知，重放不再次启动，已知失败保留已提交成员。证据覆盖分配预算、上限／拒绝、丢弃准备、外部结算、准确字面分发和 Close 围栏；布局／成员消费者共用此所有者。

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

[multi-leaf-tests]: ../src/site_coordinator/tests/multileaf_membership.rs
[move-tests]: ../src/site_coordinator/tests/tab_create/tab_move.rs
[create-tests]: ../src/site_coordinator/tests/tab_create.rs
[failed-close-tests]: ../src/site_coordinator/tests/failed_launch_close.rs
[create-service-tests]: ../src/local_terminal_service/tab_create/tests.rs
[multi-leaf-process]: ../tests/client_process/tab_create/multileaf.rs
[panel-label-tests]: ../src/workbench_authority/tests/panel_labels.rs
[authority-tests]: ../src/workbench_authority/tests.rs
[authority-stateful]: ../src/workbench_authority/tests/stateful.rs
[authority-process]: ../tests/client_process/workbench.rs
[session-tests]: ../src/local_client_service/application_connection/tests/workbench.rs
[Workspace-tests]: ../src/site_coordinator/tests/Workspace.rs
[workspace-process]: ../tests/client_process/terminal_operations/workspace.rs
[selection-tests]: ../src/site_coordinator/tests/terminal_selection.rs
[viewer-tests]: ../src/local_terminal_service/viewer_connection/tests.rs
[placement-tests]: ../src/workbench_authority/terminal_tab_create/tests/placement.rs
[rename-process]: ../tests/client_process/tab_create/tab_rename.rs
[create-process]: ../tests/client_process/tab_create.rs
[create-authority-tests]: ../src/workbench_authority/terminal_tab_create/tests.rs

<!-- documentation-pairing: english-sha256=5903c670bf4897ffaaf8ee2abe0016afeac9dea6a09216dec565aded41b1887d zh-cn-sha256=c13884757b9112840943d16a73c387f8793203107e3303e31dd00f77c5ae7c13 -->
