# Windows Client 视图控制

本契约拥有原生 CLI 对运行中 Windows TUI Client 的发现与导航。TUI 拥有选择和缩放状态；Server 路由有界请求并保留观测。规范 Workbench 快照、修订和终端控制权不变。路由请求不激活操作系统窗口。

## 身份与发布

经认证的 Client Session 可发布一个交互视图。Client 身份是已接纳的非零 Session generation，编码为 32 位小写十六进制，作用域为 Site。身份由 Server 分配，generation 在一次 Server 生命周期内不重复。普通 CLI 连接不发布视图。同步不接受调用者指定的 Client 身份，不能确认其他 Session 的操作。

视图报告已观测 Workbench 修订、阶段（`starting`、`ready`、`busy`、`unavailable`）及必填可空的选择。选择包含 TerminalContext 身份、必填可空的 Tab／Panel 身份、显式缩放标记和活动 Tab 的 Panel 数（没有 Tab 时为零，有 Tab 时为正）。有 Panel 必须有 Tab；缩放必须有 Panel。这些是 Client 观测，不是新增的规范 Workbench 字段。

每个 UI 样本携带非零规范 u64 文本 heartbeat 和视图修订。heartbeat 随 UI 采样推进；报告的视图改变时视图修订推进。合并样本可以跳号。相同修订要求视图内容相同；更旧的修订或 heartbeat 冲突。没有新 UI heartbeat 的重复网络同步不延长新鲜期。新 UI 样本到达 Server 后五秒内视图为 current，之后为 stale，直到新样本到达。

## 控制与确认

控制命令包含操作身份、精确 Client 身份、预期视图修订、最低已观测 Workbench 修订，以及一种意图：

- Context：选择精确 TerminalContext（CLI 将 Workspace 解析为其 context）。
- Tab：选择所属 context 中的精确 Tab。
- Panel：选择所属 Tab／context 中的精确 Panel。
- Zoom：将精确选中 Panel 所在 Tab 的缩放设为显式布尔值。
- ToggleZoom：UI 所有者激活目标后，从观测到的状态切换该 Tab 一次。
- Layout：读取精确目标几何，不选择或调整尺寸；[布局所有者](windows-client-layout.zh-CN.md)定义其观测与预算。

toggle 由 TUI 解析，并为这次已投递操作保留解析结果。相同操作身份和完整命令返回保留的观测，即使 Client 已断开。命令数据改变会冲突。回执过期后的缺失不能证明原命令没有效果。

| 转移                | 守卫及观测                                                                                                                                                          |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Submit              | 已登记且 current 的 Client、精确视图修订、可用容量、该 Client 无活动命令；保留 `queued`                                                                             |
| Synchronize         | 仅连接所有者发布样本；精确排队前置条件允许一次投递，并原子记录 `delivered`                                                                                          |
| 排队超时            | 五秒未投递产生 `rejected: expired`；没有投递任何操作                                                                                                                |
| Client 确认         | 精确已投递操作及所有者；保留 `applied`、动作前的 `rejected`、带原状态的 `zoom-toggled`、单 Panel zoom-on/toggle 的 `zoom-unchanged` 或可能部分执行后的 `incomplete` |
| 投递前断开          | 移除运行中视图；排队工作变为未执行的拒绝                                                                                                                            |
| 投递后断开          | 移除运行中视图；未确认工作变为 incomplete，不声称最终视图                                                                                                           |
| 调用者超时或 Ctrl+C | 仅停止 CLI 等待；查询保留的操作身份                                                                                                                                 |
| 完成后保留          | 完成观测保留 60 秒；只能回收已过期的完成记录                                                                                                                        |

在既有 Client Session 接纳上限内最多有 32 个运行中视图，每个 Client 一条活动命令，共 256 条保留控制记录。容量拒绝绝不逐出 pending 或未过期证据。已投递工作在确认或断开前持续有所有者。Server 执行 Client 命令不产生终端输入、Git 或全局焦点副作用。

## TUI 执行

一条 Windows 控制通道拥有独立的已接纳连接及有界最新样本交接。它不阻塞终端载体，也不在自身线程修改 UI。UI 在本地输入之后、绘制之前消费语义命令，检查已发布视图前置条件，将规范成员关系刷新至命令最低修订，再使用既有 context、Tab、Panel 选择所有者。异步附件及刷新沿用原有所有者和 deadline。

本地导航或模态意图中断后续远程步骤。已接纳的原生工作不描述为已撤销；此类步骤之后的中断为 incomplete。目标缺失、视图过期、不支持状态及模态／忙碌接纳均有明确结果。几何变化在绘制前重新观测 viewport 尺寸并使指针命中图失效。Applied 确认要求已观测到目标选择／缩放且成功绘制。绘制失败或连接丢失不能伪造 applied 成功。viewport 小到无法显示内容时视为 busy。对单 Panel Tab 开启或切换缩放，在选中并绘制目标 Panel 后返回 `zoom-unchanged`，不伪造 zoom 标记；该结果要求已观测单 Panel 目标、`zoomed: false` 及请求的最低修订。成功绘制的观测固定保留到确认，后续本地导航不能改写回执。

TUI 退出时控制通道停止、唤醒阻塞 I/O 并 join。Server Session 清理移除发布并结算保留请求；受管 Site 停止会 join 这些 handler。Client 视图／控制状态易失，不跨 Server 重启保留。

## CLI 路由与线上边界

显式 Client 选择器选择该精确运行中 Client。省略时，经校验的调用者 Terminal／Worker 上下文选择唯一聚焦于该绑定的视图；没有此上下文时要求恰好一个 current 视图。缺失、无效、过期或歧义上下文均明确失败；不猜测最近或全局焦点。目标解析继续使用规范 Workbench 身份。

`workbench.client-view` 包含 Synchronize、List、Get、Submit、Read 动作，响应为 `workbench.client-view.result`。请求携带协议版本、交换请求身份及 Site 身份。Synchronize 携带必填可空 completion，响应携带必填可空的下一条 control。操作身份与交换身份分离。frame kind 25／26 沿用 OBWP 版本 1；分配前请求体限 16 KiB，结果限 64 KiB。Rust 与 TypeScript 独立校验封闭变体、必填可空字段、身份／序列边界及跨字段关系。

该边界在精确 cohort 接纳范围内增量添加。旧 cohort 不会默默重新解释新消息。回滚 cohort 只丢弃易失视图／控制观测，不改变规范 Workspace 或终端状态。新增 Mac/Linux 控制运行时和 Desktop CLI 暴露不在本次 Windows 实现范围内。

## 模型与证据

已登记的 `ClientViewControl` TLA 模型抽象所有者路由、发布版本约束、一次投递、UI 效果、绘制、中断及回执回收。精确时长、容量及序列化边界由维护的可执行测试负责。Server registry 是接纳 generation、Client 样本、单调时间、排队／已投递命令及完成观测的确定性状态所有者。认证载体是适配器。TUI controller 拥有有序语义步骤；连接通道拥有传输及 join；CLI 拥有选择、单次提交及有界等待。任何所有者均不写其他所有者的私有状态。

证据必须覆盖所有者隔离、两个 Client、过期／冻结 UI、修订变化、不重放投递、重复／冲突身份、容量／过期、部分执行、人工输入干扰、断开和停止。生成式有状态轨迹保留缩减反例。共享 Rust／TypeScript／schema 夹具和 frame 上限建立有界契约一致性。受控 Windows ConPTY 轨迹必须展示 CLI 选择到达指定 Client 且成功绘制，另一 Client 不变。Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 focus／zoom handler 是选定行为参考；独立 Client 路由属于 Orchbee 架构。用户已推迟 TLA 执行，必须如实报告模型资产及未执行检查。同作者检查不代表独立复核。

### 指定目标的缩放切换

`ToggleZoom` 是精确目标意图。UI 激活目标 Tab 后，只采样一次该 Tab 原有的缩放状态，并在 Panel 焦点切换和后续循环中保留此决定。`ZoomToggled` 携带 `previouslyZoomed` 及成功绘制后的观察，缩放值必须与原值相反。它不能确认普通的 `Zoom` 意图。单 Panel 目标返回 `ZoomUnchanged`，不声称发生了缩放切换。导航中断仍返回未完成，状态查询不会再次执行切换。

布局读取使用同一个单次投递通道，但无需绘制确认，直接以 `layout-read` 完成。专用帧类型 27/28 承载其较大的有界载荷。`ClientViewControl` 还抽象只读纯度以及待处理／结果内存预留；registry 测试验证精确载荷计费。
