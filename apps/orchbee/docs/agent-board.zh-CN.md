# Agent 看板

## 范围与归属

Agent 看板是 [native Tab Session](native-tab-session.md) 中一个捕获输入的覆盖层，用于列出
Server 持有的 Agent 会话并跳转到 Agent 已存在的 Panel。Server 拥有一切 Agent 事实——会话
身份、provider、状态、规范新鲜度、注意状态与 presentation 绑定。看板仅是 Client 投影与
导航面：它绝不从终端像素推断 Agent 状态，绝不改写 Agent 状态，也绝不创建 Panel。本参考文
档随私有 `terminal_session/agent_board/` 模块、其进入 `client_view`、`ui`、`tui_render`、
`tui_pointer` 与 dispatcher 的窄访问点、OBAP 读取通道及它们的正确性证据一同维护。

`Ctrl+B g` 切换看板。打开即立即接管输入——早于首次绘制——任何按键都不会漏到终端
shell；随后首次绘制对该接管做出裁决。若覆盖层无法呈现（视口过小），绘制裁决按与 `Esc`
相同的有序取消关闭看板，并单独上报拒绝，绝不留下一个不可见的输入持有者；过期的已呈现
回执绝不能重新打开看板。看板打开时在 modal 与确认覆盖层及已触发的 prefix 之后接管输入：
Up/Down 与 `j`/`k` 按不透明 Agent 会话 id 选择行，`Enter` 请求跳转，`r` 请求刷新，
`Esc` 或普通背景按下关闭，点击选中、内圈滚轮滚动行。外圈滚轮、未知按键与粘贴均被吞掉；
任何字节都不会到达终端 shell。modal 或确认持有输入时拒绝打开；prefix 和弦仍然生效，
因此 `Ctrl+B g` 同样会关闭看板。打开与关闭两个所有权边界都会使已发布的指针命中表失效，
因此点击绝不会落在已消失的看板行上，也不会穿过覆盖层命中过期前台区域。名称、通知、
任务发送、调度、持久化与新 Panel 创建均不在范围内。

## 状态与迁移契约

看板在每个打开的 epoch 内运行一条通道线程：一个活跃操作加一个合并的待决意图。跳转请求
会替换待决的刷新，并立即取代进行中的跳转——serial 一推进，逻辑跳转即死，无论它已走到
哪一级。被取代的 `Get` 不同：它的结果可能仍在网络上，因此它继续持有物理通道配额，直到
该结果排空或通道退出，新跳转的 `Get` 排在它之后。过期结果只释放它持有的配额——绝不能
清除或取代更新的跳转。在跳转待决期间的刷新请求被吞掉而不会取代它。每一次完成的 List
尝试——成功、不完整或失败——都会启动两秒冷却，`r` 无法绕过；冷却在关闭与重开后仍然
保留，因此重开后立即请求的刷新仍需等完残余冷却。若通道线程在请求未决时死亡，已入队且
仍匹配的结果先照常适用；结果永不会到达的请求诚实地释放其配额——跳转以 `Unavailable`
失败、List 按失败轮次落定——看板绝不悬挂在 flight 中。

| 迁移      | 守卫与结果                                                                                                                                                                                                                      |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 打开      | 无 modal/确认占用时看板接纳新 epoch 与一条通道线程；上一条通道必须已经实际退出。last_good 行及其安装年龄、残余冷却在重开后保留；选择按不透明 id 对保留行修复。                                                                  |
| 绘制裁决  | 首次绘制要么呈现覆盖层、要么拒绝；拒绝按 `Esc` 的方式关闭看板并上报 `overlay_refused`，绝不留下不可见的输入持有者。过期的已呈现回执不能重开看板。                                                                               |
| List 轮次 | 一次有界遍历：在单一绝对期限下至多六个同 revision 页、stale cursor 时一次重走、十二次交换共 12 MiB。                                                                                                                            |
| 读取落定  | 同 revision 的完整遍历为 complete 并成为 `last_good`；预算耗尽或期限到达为 incomplete 或 failed——绝不伪装成空成功，也绝不改写 `last_good` 的安装年龄。                                                                          |
| 请求跳转  | 按不透明会话 id 选择；通道 `Get` 在当前有界 List 之后重新校验目标，具有独立期限与 128 KiB 上限。更新的请求立即取代逻辑跳转，被取代的 `Get` 同时排空其通道配额。                                                                 |
| 逐级解析  | 每一级都按当时的规范身份重新解析 context、Tab、Panel；同 context 已附着的 viewer 本地聚焦，否则走 attach/switch。若尚无规范快照喂入 presentation 索引，解析在跳转自身期限内等待并请求会话刷新——未喂入的索引不是 `Gone` 的证据。 |
| 落定      | 安装精确请求的 Panel，包括非零下标；目标已退役或缺失时报 `TargetGone`，绝不回退。位于当前活跃 Tab 但 viewer 已死的目标原地重挂——按稳定 Panel 身份在同一位置发起新代际 switch；viewer 存活的同 Tab 目标保持无操作。              |
| 关闭      | 作废代际、取消意图与进行中工作，随后由所有者回收线程、wake 句柄、socket 与意图；实际退出后才可重开。                                                                                                                            |

不同 revision 的行绝不拼接。不完整读取保留标记为 stale 的 `last_good`；没有 `last_good` 时
只可展示同 revision 的行并标记 incomplete——且保留 `last_good` 绝不改写其安装年龄，因此
保留行在多次读取以及关闭重开之间保持真实新鲜度。需要注意的行排在普通行之前，组内仍保持
规范顺序；每行原样携带不透明 Agent 会话 id，连同状态、provider、新鲜度与解析出的位置。
侧栏只展示最后一次已完成读取的计数、时间戳与完整性标记——关闭或未读绝不意味着存活或为
零。Get 新鲜度与 List 新鲜度是独立域，绝不互相替代。

导航所经解析的 presentation 索引只由会话实际接受的规范快照喂入——bootstrap 与每个携带
快照的应用器都在其接受分支内重建，被拒绝或未安装的结论绝不进入索引。解析等待索引就绪而
非猜测，过期或无效候选一律保守失败，绝不落到别处。

旧 epoch 通道算出的结果始终被栅栏在所属 epoch——通道持有自己的结果接收端，关闭
之后它解析出的任何东西都不会被投递。重连级抽象的是一次已准入且成功的 viewer 重连：真实
的 detach/candidate/attach 管线及其失败空间归 `NativeTabSwitch`；失败或到期的重连浮现为
Agent Unavailable/Expired 提示属跨层反馈，超出本模型 outcome 词汇——由生产修复与其接缝
证据负责，因此模型的阶段进展绝不声称真实重连必定成功。

## 正确性证据

`AgentBoard` 覆盖两个 Agent 会话、分布在两个 context 与 Tab 上的三个 presentation、一个
刻意的非零 Panel 目标、带一次重走的有界分页、revision 推进、一次退役、按不透明 id 的选择、
合并意图、stale Get 与 stale stage 丢弃、精确逐级落定、输入吞掉与通道回收。当前界——
两个 epoch、serial 预算三——额外地覆盖包含完整"跳转→关闭→通道退出→重开→跳转"路径
的关闭/重开、按 presentation 的 viewer 死亡环境动作、窄同目标重连级，以及必须关闭看板
而非保持不可见模态的不可绘制绘制。`LandedIsLive` 记录落定时刻的观测；已注册边界性质
`LandedOnlyOnLiveViewer` 独立要求新产生 `landed` 的步骤在 pre-state 中找到精确目标
viewer 存活——其后的 ViewerDies 无法证伪一次当时合法的落定。

注册族检查组合机器的三个投影，每个仍为两个 epoch、两个 round 和 serial 预算三。
`ModelScope` 是私有验证配置：

| 投影         | 检查的观察                                                           | 保留的公平性                  |
| ------------ | -------------------------------------------------------------------- | ----------------------------- |
| `navigation` | List/Get、缓存行、选择、精确逐级落定、viewer 死亡/重连和通道生命周期 | 全部导航 stage 与通道退出义务 |
| `input`      | 看板打开时的输入相等关系及共享生命周期                               | 通道退出                      |
| `paint`      | 拒绝绘制、最终关闭及共享生命周期                                     | 不可绘制关闭和通道退出        |

只有 `Next` dispatcher 选择事件族；primitive 动作、guard 和配置中的不变量/性质均共享且
保持不变。其有界存在量词分支的域是常量空集或单元素集合，精确选择请求的事件族，
并使固定版本 TLC 的动作提取器保留必需覆盖统计的原始动作名称。
`combined` scope 保留完整 Cartesian 机器作为可执行参考；其直接运行尚未在
声明的资源界内完成。注册检查不声称直接枚举了这个乘积。

组合论证是前向行为投影。除共享 Open/Close 初始化外，导航动作与公平 guard 不读取两个
输入位或 paintability。其投影保留导航记录，把两个输入位映射为 false、paintability 为 true，
PaneByte/PaintRefused 映射为 stutter，UnpaintableClose 映射为已有 Close，生命周期后态
相同；没有给 Close 添加公平性假设。全部导航与通道退出的公平 guard 和非 stutter 步骤仍
相同；不可绘制义务单独检查。输入与绘制投影保留各自的局部位、board、epoch 和 lane 存活。
开放通道映射 idle，已取消的活通道映射 exiting，已回收通道映射 idle；无关字段采用该族
有类型的初始值及共享生命周期后态，包括以关闭次数的 capped 值作为 serial。仅导航动作
投影 stutter，每次关闭映射 Close，Open 接纳和 LaneExit 保持精确 guard 与非 stutter 后态。
绘制投影保留 UnpaintableClose guard 与弱公平性；输入投影完整保留 InputFrozenWhileOpen
的真值。其他公平动作在局部禁用，无需额外假设。

因此每条公平的组合行为都投影到每个已检查 Spec。导航不变量、round/jump 进展与独立
landing edge 由 navigation 检查，输入隔离由 input 检查，不可绘制进展由 paint 检查，通道/
类型/生命周期事实按各自字段检查。这些结果共同建立声明界内的组合断言；无需把任意组件
行为反向提升为乘积。这个论证是显式的精化信任边界。新增任何跨族读写或 joint property
都会使它失效，必须更新映射或恢复直接乘积检查。只有三个 variant 全部成功后才合并必需
动作覆盖率。

模型刻意不保留看板字节、外圈滚轮或跳转待决时刷新是否已经发生。擦除这组原本只服务覆盖率
的历史会得到双向 stutter 商：旧模型的每一步都投影为相同的产品迁移或这三个具名自环之一；
商模型的每条行为则可通过让旧模型各执行一次对应具名步骤，并把重复自环视为 `[Next]_vars`
本就允许的 stutter 来提升。没有弱公平动作读取被擦除的历史，因此动作 enabledness 与每项
`WF_vars` 义务均保持不变。三个必需覆盖名称仍显式存在；即使后继状态已经发现，runner 也按
generated successor 计数。

该商还会在 walk 结算或看板关闭后规范化 `laneRound`、`walkRev`、`walkPages` 与 `retries`。
这四个 scratch 值只由 `lanePhase = "list"` 时的 list 动作读取；`StartList` 会在后续读取前
初始化每个值。投影在 list 期间保留进度及 pinned revision 的相等关系，其余阶段映射为有类型的
初始值；不透明 revision 的重命名见下文。旧模型每一步都
投影为相同的具名产品步骤；商模型的步骤则可通过保留旧模型已结算的 scratch，直到下次初始化来
提升。有类型的初始值仍满足它们的 `TypeOK` 范围检查；没有业务 guard、产品不变量或时序性质在
list 阶段以外读取这些被丢弃的值。List 动作在
该阶段之外不可用，其他每个弱公平动作在投影下保持相同 enabledness。这消除无效历史，不限制
读取、重开、取消、导航或环境交错。

同一投影只在 Open、Close、RequestJump、StageStaleDrop 或 JumpSettled 显式把 `outcome`
清为 `none` 时，清理 `landedPlacement`、`landedTarget`、`landedSerial` 与 `landedLive`。
业务 guard 不读取这些诊断值；除类型范围检查外，不变量只在有效的 `landed` 或
`target-gone` outcome 下读取它们，这些有效 outcome 的精确值全部保留。`StartGet` 在再次
使用前初始化全部四个字段，`ResolvePanel` 在产生 `landed` 前完整写入四个字段。lift 后的
历史在 outcome 缺席期间携带旧诊断值，直到原来的初始化替换它们。landing-edge 性质保留
旧前置状态的 viewer 事实和精确后置目标，因此仍是独立检查，不只依赖记录位。每个弱公平
动作的 guard 与非 stutter 产品转移均保留；缓存行、导航视图与有效 outcome 不变。

关闭还会规范化 `round`、`bumps`、`deadSeen`、`paintable` 与 `paneBytesAtOpen`。
这些字段的业务读取要求看板打开或 list 活跃，Open 在复用前初始化它们。关闭状态的投影令
`paneBytesAtOpen` 等于 `paneBytes`，所以关闭状态的 PaneByte 仍投影为相同具名步骤。
lift 保留旧关闭 scratch，直到 Open；缓存及其年龄、选择、冷却与 serial 预算全部保留。
viewer 存活与已退役绑定在未来导航仍可观察时保持精确，具体见下文。
GetStaleDrop 与 StageStaleDrop 同样只在移除跳转时清零
`jumpSerial`；`jumpStage = "none"` 时业务动作不读取这个 serial，StartGet 会替换它。
全部类型范围、关闭及打开状态的观察和公平动作 guard 均保持有效。

Revision 标签在这里没有数值或排序含义，只观察 `serverRevision = walkRev`。双射重命名
把当前 Server 标签映射为 1、不同的 pinned 标签映射为 2、尚未读取的哨兵保留为 0。
ServerBump 保留原预算递增，翻转非零 pinned 关系，并保持当前标签为规范值。因此 ListPage
仍看到完全相同的首 page、同 revision、retry 与 incomplete 分支。两次 bump 仍允许原有
ABA 相等关系，没有移除任何 mutation 交错。lift 后的历史携带原当前标签，每次 ServerBump
仍执行原翻转。每个具名步骤、弱公平 enabledness、不变量和时序观察都在重命名下保持不变。

ServerRetire 还只会把永久移除的 presentation 的 viewer 事实规范为 `viewerLive = FALSE`
及不属于 `deadSeen`。退役绑定永不返回；GetResolve 排除它们，每个逐级 resolver 在读取或
重连 viewer 前先检查退役。其他 viewer 的精确存活事实与死亡预算均保留。旧模型中被移除
presentation 的 ViewerDies 投影为 stutter；ViewerDies 没有公平性义务。lift 保留旧 viewer
的事实，可在原 guard 允许时插入其不可观察的死亡。每个业务步骤与公平动作 guard 仍然匹配。先前记录
的落定诊断保持精确，新的落定不可能以退役绑定为目标，所以独立 landing-edge 检查保留
全部相关的前置 viewer 事实。退役、target-gone 结果与缓存导航状态都被保留；这个投影不会
复活 viewer，也没有移除任何有效目标的死亡。

在导航 serial 达到上限时，有限模型为最后一个待决 intent 或正在验证的 Get 保留全部可能
绑定的精确 viewer 事实。GetResolve 固定 presentation 后，只有这个精确目标仍可能被读取
或重连。RequestJump 是 jump intent 唯一创建者，且要求剩余 serial 预算；serial 永不递减，
Open 也不重置。旧 serial 的 stage 不能落定。因此被擦除的 peer 不会再成为后续目标，每个
新的 landing edge 仍检查原始前置目标 viewer。Open、RequestJump、StartGet、GetResolve、
stale-drop 结算、JumpSettled、CloseBoard 与 ServerRetire 保持这个投影。擦除 peer 的环境
死亡投影为 stutter；ViewerDies 没有公平性义务。lift 携带这些 peer 的旧 world facts，
只有原 guard 允许时才插入其死亡。

serial 达到上限且没有待决 jump intent、Get 或 stage，或最后一个 epoch 已关闭时，导航
永久不可用。此时全部 viewer／death／retirement world 不再被观察，映射为 false viewer 位
与空集合。ServerRetire 仍显式存在，但在此处只产生 stutter；它没有公平性义务。lift 可在
原 guard 允许时执行旧预算内的退役，额外不可观察步骤使用 `[Next]_vars` 本就允许的
stutter。导航仍可能发生时，精确退役事实与原预算全部保留。刷新、list、缓存、选择、绘制、
通道回收与任何已接纳的重开继续照旧进行。每个公平动作 guard 与非 stutter 步骤保持不变。
这个后缀约简只依赖声明的有限模型界，不是生产程序的导航或重开次数限制。

导航投影在这些有限后缀之前直接返回原记录：ServerRetire 已维护每个被移除 viewer 的
规范事实。单元素函数应用绑定已经求值的记录，固定标准 `TLCEval` 恒等操作物化小集合与
viewer 函数，避免保留嵌套惰性闭包。这只影响求值；状态、动作、guard、公平性和性质均不变。

最后一个 round 结算后，`bumps` 同样不再被读取，直到 Open 重新初始化它。投影在 ListPage
或 ListDeadline 结算时清零它，并在 `round = MaxRound` 且没有活跃 list 时保留 ServerBump
为显式 stutter。每个活跃 walk 的原 mutation 预算、revision 相等关系、ABA、retry 和 deadline
交错全部保留。lift 携带旧耗尽 round 的 bump 计数，原 guard 允许时执行预算内 mutation，
之后使用 stutter；任何后续 list 前，Open 都会重置两种表示。ServerBump 没有公平性义务，
全部公平 guard、缓存观察和已配置性质保持不变。

已接受的 1,083,010 distinct 状态运行仍是先前单 epoch 变体的有限界证据——`MaxEpoch = 1`
不可达重开。已注册的双 epoch 配置是当前有限界范围，由上述三个投影检查。在注册 runner
完整跑完全部三个之前，本文不声称这一范围已经通过；其重开、viewer 死亡/重连与不可绘制
关闭断言目前由本文所述的模型映射、
不变量、弱公平进展性质与确定性测试共同支撑。

使用 `node scripts/tla/run-models.mjs --models agent-board` 运行已注册模型。OBAP 帧与身
份契约精化线上步骤；`NativeContextSelection`、`NativeTabSwitch` 与 local-client 模型精化
context 切换、attach 与传输接入——它们不覆盖看板本身。

确定性测试覆盖 cursor 与同 revision 行装配、完整性诚实、错误通道隔离、stale 代际跳转拒
绝、带通道配额保留的跳转取代、`Enter` 被拒时保留既有跳转、死通道已入队结果先于残余配额
释放适用、残余冷却与 last_good 安装年龄、绘制裁决输入所有权、两个所有权边界上的
dispatcher 命中表失效、真实 UI 接缝上的受栅栏索引喂入与未喂入索引等待、同 Tab 死 viewer
重挂至真实存活落定、modal 输入不泄漏、非零 Panel 上的精确跨 context 落定、`TargetGone`
不误落、有界线程关闭，以及两条已接纳通道各自独立退役。看板字节绝不进入 shell。渲染测试
覆盖两个语言环境、终端文本消毒与有界视口。

Presentation 索引将每个已绑定子项归于其原 Panel 身份。跳转落定时重新检查精确 Presentation：若 Panel 当前显示其他成员，则调用原生 Presentation 选择；共享 Panel 不足以证明已落定。成员标签各自保留，布局叶子与 Tab 位置不变。
