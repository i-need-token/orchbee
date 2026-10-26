# 原生 Panel 交换

## 调用与归属

`panel swap [source] --with <target>` 交换同一 Tab 中两个叶子 Panel。另一种形式
`panel swap [source] --direction left|right|up|down` 使用 Windows TUI 未缩放的单元格几何解析
一个邻居。`--with` 与 `--direction` 互斥且必须提供一个。`--panel` 与 `--current` 是另一种来源
选择形式，彼此及与位置来源互斥。Panel 操作数只接受精确 ID 或 `@panel:0x<hex>`，不接受标签。
显式来源及 `--with` 且省略 `--client` 时使用无头共享路径。省略来源时先使用已验证的调用者上下文，再使用所选 Client 的当前 Panel。显式 `--client` 或
既有唯一 Client 解析器选择 TUI；Client 不明确时在修改前拒绝。

这是作用于现有 Session 的 Windows 工作流程。总超时默认 30000 ms；`--timeout` 接受
1 至 3600000 ms。不提供分离模式或调用方指定的工作流程请求 ID。CLI 不启动 Session、不创建
终端，也不拥有持久工作流程状态。

Server 拥有 canonical 位置及比例，TUI 拥有焦点和缩放，CLI 拥有连接这些操作的有限步骤及回执。
`local_client/workbench/panel_placement.rs` 拥有单次结构命令传输；Server 的
`terminal_panel_placement` 权威拥有移动／交换的共享准入。既有 Client control 协议承载布局
读取和聚焦。不引入新的 TUI 修改 mailbox 或全局选择权威。

## Canonical 交换

`panel.swap` 携带精确 Session、预期修订、请求 ID、context、Tab 及源／目标 Panel ID。
它在克隆的候选中交换两个完整叶子节点的既有位置。所有 Split 和 slot 身份、方向及结构保持
精确不变。Panel 与 Presentation 身份、创建／自定义标签、子 Presentation 和终端绑定随叶子
移动。不分配身份、不产生 Host 生命周期效果，其他 Tab 和 context 不变，因此现有共享 Split
比例保留原值。

准入检查导航授权、保留命令优先级、规范修订及准确 Workspace Context／树。聚合 Close 围栏阻止有变化的交换。候选编码、投影准入和账本容量检查先于单次提交。相同来源／目标返回不变，不改变修订或分配身份。目标缺失、修订过期、授权拒绝及容量耗尽均保留树。精确重放返回原结果，冲突复用不能替换它。

提交结果为 `panel-swapped`，带新修订及精确 context／Tab／Panel 对。传输层检查动作对应的
结果种类、所有身份及恰好一步的修订变化；移动回执不能确认交换。丢失或无效回复将传输层
退役为结果未知，不重放也不执行逆操作。

既有原生树准入仍拒绝子 Presentation；本操作不扩大支持的拓扑。交换完整节点绝不通过丢弃
元数据来绕过准入。

## CLI 工作流程

显式 Panel 对且省略 `--client` 时，从快照解析目标并交换一次，不查询 Client、不控制或聚焦。
确定提交即完成该共享流程。下表描述方向／隐式目标或显式 `--client` 使用的 Client 流程。

| 阶段        | 守卫、效果与后续结果                                                                                                                        |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| 预检        | 验证全部语法、监视中断、受理一个现有会话、读取一次 canonical 快照并选择一个当前且 ready 的 Client。固定其选择和精确来源。                   |
| 解析        | 显式 Panel 对使用该快照。方向模式读取同一 Client 未缩放布局，不改变选择；canonical 修订必须与快照相等。固定一个精确邻居，变化后不重新解析。 |
| 无变化      | 同一 Panel、跨 Tab 的 Panel 对或没有邻居返回明确的无变化原因。不提交 canonical 命令或聚焦。来源缺失／不可用是拒绝，不是没有邻居的成功。     |
| 交换        | 在固定修订上恰好提交一次 `panel.swap`。确定拒绝或结果未知时停止流程；确定无变化也不聚焦。                                                   |
| 观察 Client | 确定提交后，读取原 Client，直到它在提交修订上 ready。选择变化、Client 不可用、取消或截止时间到达均在此停止，并保留提交事实。                |
| 聚焦        | 使用刚观察到的 Client view 修订，单次提交源 Panel 聚焦。沿用 UI 准入、中断、附着收敛和绘制确认。                                            |
| 完成        | 只有 canonical 确定提交且聚焦 applied 才是完整成功。其他所有结果都独立保留 canonical 与聚焦事实，不补偿、不重试。                           |

全部读取与提交共享最初的绝对截止时间，轮询不续期。信号停止本次调用的等待及后续阶段，
不撤销已受理的交换或取消已受理的聚焦。输出为一份完整有界文档，即使中断决定退出码也包含
已知效果。每个已提交阶段有自己的关联身份。只读布局失败不能被报告为 canonical 修改未知。

聚焦使用原 Client 和既有 Panel focus 操作，不改变其他 Client 的选择。同 Tab 交换保留
本地缩放和共享比例；选择非活动目标使用既有 context／Tab 选择生命周期。Canonical
交换与 Client 聚焦是分开的提交，不声明跨权威原子性。选择在观察点比较，无法检测两次观察
之间的 ABA 选择变化。最终聚焦提交还使用新的 view 修订，控制受理后的输入沿用既有中断守卫。

## 回执与恢复

`orchbee.panel.swap` v1 用 `scope:shared` 以及 null 的 Client／焦点／几何字段标识无头交换，
独立保留规范请求及结果，包括未知结果。Client 流程使用 v1，区分目标解析、canonical 提交和聚焦观察，记录已知的 Session、所选
Client、精确 Panel 对及已提交请求 ID。Canonical 的未提交、拒绝、无变化、已提交及未知
保持不同。Focus 尚未开始时为 null，否则为包含 operationId、observation 和 wait 的紧凑记录。
观测保留权威聚焦状态（queued/delivered/applied/rejected/incomplete）、直接拒绝、
not-submitted 或 unknown，不重复整个控制命令。
超时、取消或提交后传输失败不能抹去已知 canonical 提交。

提交后非零结果需要先检查回执及当前布局，再决定下一步。重试整个命令会再次交换位置，
绝不自动进行。既有 Client control-status 可观察已提交的聚焦身份。本流程不增加持久日志或
重启恢复保证。Schema 目录描述两个版本。

## 证据与兼容性

Panel 交换提供两种交换模式、同 Tab
原子交换、成功后聚焦来源及无变化语义。公开身份、Client 路由、错误信封及进程生命周期内的
canonical 状态仍遵循原生契约。

独立树排列／逆变换测试检查完整叶子与固定拓扑的变化范围，包括嵌套树、元数据、保留请求
重放及提交拒绝。协议一致性覆盖 Rust、TypeScript 和 JSON schema。传输用例拒绝跨动作回执
并保留未知结果。回执状态矩阵检查终态错误／取消切点及已知效果保持。请求准入后的传输唤醒
验证取消会退役不确定交换且不重放。Windows 进程测试覆盖实际解析、输出／schema、阶段顺序、
回复丢失、过期布局／选择、拒绝、无操作及总截止时间；两个真实
TUI 验证嵌套交换、终端身份、比例、缩放保持及源焦点，同时不改变另一 Client。

<!-- documentation-pairing: english-sha256=1c7219b40bb123480f7c62f89617d56e59f489bd7e9e81efb3ae38856082d6ae zh-cn-sha256=f1ab60460b210d6edb17f2f99cba91d22cde8469b7fd0b15da1aa754ed4b7a55 -->
