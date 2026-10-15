# Workbench 重启检查点

## 范围与归属

`workbench_authority/checkpoint.rs` 拥有 Workbench 保留配置的私有版本化投影，以及将其重建为
新权威的过程。Windows 托管 Site 是存储／启动消费者。领域编解码器与 Windows 文件系统准入、
检查点发布及终端重新启动分离。命名 Site 路由和其他恢复所有者仍有独立义务，不新增其他平台适配器。

检查点保留可选 Project 成员关系、Project 标签、全局本地目录路径、Workspace 及其显示顺序、终端上下文、有序 Tab、分组、固定状态、
分屏结构及比例、Panel 输入策略及 Presentation 标签，同时保留 Launch Profile 定义和分配高水位，
包括已删除 ID。主／子 Presentation 均保留结构，但不携带终端绑定。
版本 2 还保留已校验的外部 provider 引用，详见 [Agent 重启引用](agent-restart-reference.zh-CN.md)。

它不保留运行进程、Site／Worker 身份、控制权、租约、Client 视图／焦点／zoom、请求结果、发布
游标、未完成操作、Agent 状态或终端历史；这些恢复责任属于各自运行时所有者。检查点从不授权
重放终端命令或 Git 操作。

## 模型

| 操作       | 前提                                                                                         | 结果与不变式                                                                                                   |
| ---------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| 捕获       | 权威未退役；已提交对象图有效；每个 Directory 和已绑定 Presentation 都属于传入的本地 Worker。 | 一个有界检查点；源权威不变；移除进程绑定和 Worker 身份。                                                       |
| 解码       | 严格 version 1–7 文档、字节有界、导航与版本匹配、配置图、计数器及 provider 引用有效。        | 完整保留配置或类型化拒绝；不返回部分结果或空成功。                                                             |
| 重建       | 调用者传入新启动的 Site 身份及其已准入本地 Worker。                                          | 保留结构 ID 和本地路径，使用新目录所有者、无绑定 Presentation、新 Profile Site 前缀、revision 0 和空临时状态。 |
| 恢复后分配 | 恢复的高水位允许递增。                                                                       | 新 ID 高于所有已分配计数器；耗尽时保留既有类型化拒绝。                                                         |

调用者负责新 Site 身份的新鲜性。Profile ID 为 `<siteHex>:<counterHex>`，存储形式仅保留
计数器、名称和启动定义，重建使用新 Site 前缀。旧 Site 授权和 Profile ID 不获得新启动的权威。
名称不要求唯一，既有选择器歧义语义保持不变。

## 私有格式与校验

### 规范化导航与历史迁移

写入器输出标识为 `orchbee.workbench.checkpoint`、数字版本为 `version: 7` 的文档。
`checkpoint/navigation.rs` 分别拥有当前与历史导航 DTO。版本 6 和 7 保存按身份排序的全局
`directories`、`projects`、`workspaces` 数组。Project 记录仅含 `projectId` 和 `label`；
每个 Workspace 必须包含可为 null 的 `projectId`。本地 Directory 记录仅含 `directoryId`
和 `path`，重建时赋予新的 Worker 所有权。显式 `workspaceOrder` 是全部 Workspace 身份的排列。
未分组 Workspace、空 Project、共享 Directory 引用及未使用 Directory 均在捕获和重启后保留。
规范化对象图最多容纳 65,536 个 Directory、1,023 个 Workspace，每个 Project 最多 256 个
Workspace，同时受下述编码上限约束。

版本 7 增加独立的 `projectOrder`：非空值必须是全部 Project 身份的完整排列，省略或空值使用规范
ID 顺序。版本 6 在不含项目顺序时仍可读取；将非空 Project 顺序标为版本 6 会拒绝。旧 cohort
显式拒绝版本 7 并保留已提交文件，不提供自动降级或丢弃顺序的转换。Workspace 和 Project 顺序在
捕获／重启后保留，身份注册表继续按 ID 排序。

版本 1–5 必须采用历史形状：每个 Project 拥有自己的 `directories` 和 `workspaces`，历史
Workspace 记录不含 `projectId`。迁移先校验原始各 Project 数组的排序、256 个 Directory
和 256 个 Workspace 的上限，以及主目录确属同一 Project。扁平化不能将无效的跨 Project
引用变为有效。随后将每个 Workspace 关联到原所属 Project，保留全部身份和 Directory，
对全局数组排序，再执行完整对象图及身份／高水位校验。显式历史显示顺序保持不变；省略或空
顺序转换为历史 Project／Workspace 遍历顺序，可能不同于全局 Workspace 身份顺序。

导航形状必须与声明版本匹配。向历史记录注入当前字段、将历史导航标为版本 6 或 7、未知或重复
字段、重复身份、缺失引用，以及低于保留身份的高水位，都会拒绝整份文档。编解码器从不将
畸形输入转换为空 Workbench。下述存储消费者的全部副本无效显式提醒属于独立的恢复决定。

历史规范化可能因新增字段而超过仍为 1 MiB 的 Snapshot 传输上限，即使原版本 1–5 Snapshot
符合该上限、检查点也未超过 2 MiB。`HistoricalMigrationTooLarge { version, actual, maximum }`
会显式拒绝这种迁移；它不准入超限对象图、不丢弃保留内容，也不提高任一上限。源 DTO 仍在
规范化前拒绝未知或重复字段。不提供自动降级或转换。

版本 2–7 要求 `agentSessions`，版本 3–7 要求 `terminalDirectories`，版本 4–7 要求
`managedNames`；更早版本拒绝这些字段。版本 5–7 保留规范分割比例；版本 1–4 恢复等比例，
并拒绝注入的非默认权重。见[共享比例](shared-split-ratios.zh-CN.md)。最多 512 个唯一名称／
Presentation 声明仅携带 `presentationId`、`name`、`providerId`，且必须有匹配的已接纳
官方 provider 引用。见[托管名称](agent-canonical-model.zh-CN.md#托管名称重启)。
`lastIdentity` 和每个 Profile 的 `counter` 都是规范化、非零的 32 字符小写十六进制数。
终端上下文要求所有 Presentation 绑定为 `null`。不支持的版本均拒绝。

### 逐终端目录恢复

版本 3 要求 `terminalDirectories` 列表，每项为稳定 `presentationId` 与已校验 `path`。主、子 Presentation 各自独立。版本 1、2 仅在没有此字段时仍可读取；null、重复／外部成员及非法路径拒绝整份文档。继续使用 2 MiB 上限；旧 cohort 明确拒绝版本 3。

请求／后台工作静止后，正常停止在 Worker 释放前，通过现有私有查看器所有者采集新的有界观察。有效终端上报目录优先于根进程目录；不会用子进程目录冒充根目录。最终物理成员关系与 Worker 代际必须仍匹配。未知 detach 由已停止的进程观察器继续负责，使本次停止失败并保留之前的检查点；后续停止可结算该精确附着。

正常运行时，既有进程采样器仅缓存已经观测并验证的目录，同时保留确切 Worker generation、Host
终端身份及采样起始时间。缓存捕获在 Site gate 下重新检查当前物理成员；过期、移除或替换的绑定
不贡献目录。观测不可用时不伪造目录、不清除已有恢复意图。保存不打开 viewer、不采样进程、
不读取屏幕内容。

捕获将准入观察映射到当前 Presentation 身份。观察不可用时保留已有恢复意图，否则使用普通工作区／home 默认值；已移除成员不再保存。全新启动优先使用保留目录，其次工作区默认目录；所选路径已不是目录时，Windows 恢复使用 Worker 的 home 默认值。此策略不改变用户显式启动的校验。共享 PowerShell 提示符同步也应用于恢复后的编码命令 shell，因此 provider 退出后切换目录仍可被观察。

证据必须覆盖精确身份／修订围栏、主子分离、旧格式兼容、非法记录、不恢复活跃权威的捕获／重建、未结算查看器清理、实际 Windows 目录切换及保存目录被删除。启动进程证据本身不能证明此目录生命周期。

检查点编码上限为 2 MiB。重建 Snapshot 必须通过既有完整对象图校验和 1 MiB 传输限制；Profile 目录保留
64 项、256 KiB 限制。全部自有结构／Profile 计数器必须互不重复，且不超过 `lastIdentity`。
引用由 Snapshot 所有者校验。伪造已绑定 Presentation、捕获时的外部 Worker、无效路径、缺失
成员关系、非法启动定义或低于保留 ID 的高水位都会拒绝整个操作。仅 JSON 编码不能证明对象图有效。

领域层不访问文件系统、不启动进程。存储层必须准入 Windows 私有路径、发布完整记录、区分
不存在和损坏，并保留失败证据。启动恢复必须建立新的物理绑定，并处理被中断操作；编解码器
返回无绑定权威不代表已恢复可用会话。格式变更需要明确的版本／迁移决定，不提供宽松回退。

## 终端重新绑定准备

只读物理注册遍历所有上下文的主、子 Presentation；破坏性树操作守卫保持不变。恢复目标分别携带保留目录与 Workspace 默认目录意图。
`checkpoint/restore.rs` 按上下文、Tab 和深度优先树顺序枚举每个未绑定的主／子 Presentation。
每个私有恢复目标携带新的 Site／本地 Worker，以及准确的上下文／Tab／Panel／Presentation
成员关系，以及分别保留的恢复目录／Workspace 目录意图。没有保留 cwd 的独立上下文和未配置目录的 Workspace 使用 Worker home。

`prepare_restored_terminal` 重新检查 Site、本地 Worker、成员关系、未绑定状态和 cwd 默认值，
再校验并准备完整的候选投影／发布，之后才允许变更。丢弃准备结果不改变任何状态。提交仅设置
一个绑定并递增修订；结构身份、分配高水位、标签、顺序、分组、输入策略和 Profile 均不变。
拒绝已绑定的目标及重复的物理终端绑定。

这层准备不创建进程、不重放命令。托管协调器必须同时准备新的终端预留和此绑定，在同一把锁下
提交两者，再分发并结算准确的物理请求；暴露 Client 前还须重建独立终端 Open 选择。
规划和绑定测试不证明这些运行时集成已完成。

## Windows 托管启动与正常停止

托管启动在 `LOCALAPPDATA` 下打开独占 `WindowsSessionStore`，并在启动 Worker 前解析已提交
副本。存储 owner 从三个槽位选择最新且完整验证的 payload。无效副本保留并产生可见恢复提醒；
全部副本无效时明确报告空恢复。数据缺失表示新 Workbench；私有路径或文件系统准入失败仍中止启动。
详见 [Windows checkpoint 存储契约](../../../crates/local-site-runtime/docs/windows-checkpoint-storage.zh-CN.md)。
默认和命名 Site 激活均在使用 checkpoint v7 存储前发布 catalog v5；catalog reader 接受 v1–v5。
最多支持 v4 的旧 reader 会拒绝新记录，包括正常停止后的记录。Server 在打开 store 前保留已准入
的 durable generation，因此直接 Server 入口也会在清理 pending 文件前拒绝不支持的 catalog。

`WindowsSessionStore::try_read_validated` 区分普通无效副本（`Ok(None)`）和致命领域准入错误
（`Err`）；`read_validated` 保留其回调不可失败的既有行为。任一完整且通过存储准入的槽位若触发
历史迁移容量拒绝，Server 加载便会中止，即使另有更旧或更新的有效槽位。类型化 checkpoint 错误
作为 I/O 错误源向上传播，所有已提交副本保持不变，不发布恢复／空恢复提醒。不启动 Worker、
恢复权威或后台保存者。普通损坏仍使用既有回退／空恢复策略。打开 store 时清理未提交 pending
文件的行为保持不变。
`WorkbenchCheckpoint` 是已准入配置值：私有对象图校验身份不会成为可运行权威，只有使用新 Site
及已准入 Worker 重建后才产生新的权威。

首个启动 IPC 前，协调器准备全部新 shell／provider 预留及 Workbench 绑定。此阶段失败尚无 Host 或
终端，结束创建时持有的准确 Worker 进程并确认退出。随后每个准备请求仅分发、结算一次；启动
失败或未知结果保持显式状态，从不自动重试。使用保留终端目录、Workspace／home 默认值，初始几何为 80x24、
单元格 8x16，直到 Client 调整尺寸。第一个恢复的独立主 Presentation 提供保留的 Open 结果，
不会额外创建 Tab。保存／配置目录不存在时，恢复使用 Worker home。
Open 检查完整的主／子 Presentation 成员，包括分组及固定 Tab；该读取不放宽独立的破坏性操作
限制。启动期限前未发送的请求结算为不可用。之后启动失败会停止后台维护，请求 Worker 所拥有的
Host／终端关闭，再确认 Worker／传输线程退出并停止观察入口。未确认的清理保留需要恢复的激活
状态；单独终止 Worker 不代表已清理 Host。

正常停止在请求和后台操作收敛后、释放 Worker 前保存。编码或发布失败阻止释放，保持停止失败；此前
有效存档仍保留，被移走的无效原件仍在 raw 归档中。正常停止先结算唯一后台保存者，再捕获最新目录并发布最终存档，最后才释放
Worker；延迟的后台写不能覆盖最终存档。新物理创建只播种保留的 provider 引用，不恢复存活 Agent
状态或来源权威。Client／历史恢复仍属独立任务。已完成托管名称在新物理创建及引用播种之后、Client 发布之前恢复；
普通名称和待完成启动不恢复。正常捕获将当前 Names 声明与精确物理引用组合。启动失败时的激活标记
仍遵循既有恢复语义。

## 运行中保存模型

Windows 托管 Server 在终端恢复及引用／名称播种完成后拥有唯一后台保存者。它观察已提交的
Workbench revision；每次变化重置 500ms 防抖。每 30 秒还捕获已缓存目录及当前精确代际的 provider
resume 引用。payload 不包含终端屏幕、历史、Agent 对话或活跃控制者状态。待办工作只有一份最新
快照，不使用无界队列。
当全部保留副本都无效时，保存者在内存中保留准确的初始空 checkpoint，不发布该基线。之后只有布局、
已缓存目录或 provider 引用发生变化，使完整 checkpoint 不同时，自动发布才可替换无效槽位。
该抑制仅适用于无人参与的后台保存。显式 `server stop --confirm` 表示确认当前工作区，因此其最新 final
捕获即使仍是空状态也可以发布；存储 owner 会在提交前归档被替换的无效原始字节。

捕获仅在构建有界配置值时持有既有 Site gate，随后释放。文件系统验证、原件保留、flush 与替换
在 gate 外由唯一写者执行。I/O 期间的新 revision 仍标记为 dirty，安排下一次捕获。保存失败保留
此前有效副本及有界错误状态；不终止终端、不授权新 Site。后台保存者必须在最终正常停止捕获前
结算；无法 join 时停止不能完成，不能产生两个存储写者。

## 证据

确定性测试通过独立对象图预期覆盖捕获／重建、嵌套分屏及子 Presentation、Profile 重映射、
删除身份后的分配、外部所有权、损坏、重复／未知字段、绑定注入、对象图错误和大小限制。
历史夹具保持版本 1–5 的嵌套形状。迁移证据覆盖交错的全局身份、显式与隐式历史显示顺序、
未使用 Directory、扁平化前拒绝跨 Project 主目录、各 Project 上限、重复身份、高水位错误，
以及版本／字段不匹配。版本 6/7 捕获／重启保留未分组成员关系、目录引用和适用版本中的
Agent、cwd 及托管名称重启意图。
构造的版本 1–5 夹具对比恰好 1 MiB 的历史 Snapshot、规范化后的超限形式，以及恰好符合上限的
对照。原生存储与 Server 加载证据覆盖存在更旧／更新可用副本或没有回退副本时的致命拒绝、
类型化错误传播、已提交字节不变，以及普通损坏恢复。直接 Server 准入还验证在 durable catalog
版本不受支持时保留 pending 文件。
协调器测试覆盖物理预留与对象图的联合准备、分组／固定／子成员、Open 复用，以及不重放的显式
不可用结果。Windows 存储／进程测试覆盖私有替换、锁和准确句柄终止。真实打包 Windows 正常
停止／重启验收对比保留的布局／Profile、新运行时身份、shell 执行效果及 TUI Open 复用；
替换受阻时必须保留旧字节和运行中的终端，直到显式重试成功；无效副本必须保留准确原始字节，
产生声明的回退／空恢复提醒，且不得绕过 Site owner 准入。缓存捕获测试覆盖物理替换、乱序采样与
已移除成员。调度测试使用显式时间及延迟写入，证明防抖、定期补存、有界待办与最终停止顺序。
原生崩溃／重启验收须比较多个 Tab、嵌套分屏、标签、目录及 provider resume 引用；真实 provider
连续性、机器重启及交互负载仍须单独提供证据。
