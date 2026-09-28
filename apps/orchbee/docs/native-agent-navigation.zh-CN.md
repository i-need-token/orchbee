# 原生 Agent 导航

## 权威与语法

Windows `agent focus` 将规范或物理 Agent 解析为一个可导航的主 Panel，并向一个现有 TUI Client
路由带确认的聚焦请求。支持一个原生 Agent 位置目标，或 `--session`、`--name`、`--panel`，
沿用既有优先级与互斥规则。`--client`、`--request-id`、`--detach`、`--timeout` 和
`--format text|json` 沿用 Client 控制契约。默认总时限为 30000 毫秒。帮助、补全和解析
保持离线。不增加 macOS 或 Linux 运行时。

该命令改变 Client 选择，通过 [Client 提示状态所有者](native-client-attention.zh-CN.md)
显式确认所选 Tab 中当前已知版本。这一显式效果独立于普通可见绘制确认及其外层焦点门槛。
规范 Agent 生命周期、语义状态和新鲜度仍由 Server 观察负责。

## 模型与归属

| 要素     | 契约                                                                                                                       |
| -------- | -------------------------------------------------------------------------------------------------------------------------- |
| 问题     | 脚本可以启动或解析具名 Agent，但目前必须手工把位置转换为 Panel 聚焦命令。                                                  |
| 输入     | 类型化 Agent 目标、可选 Client／请求标识、等待／格式参数、已验证调用者元数据、一次快照、Agent 和 Client 观测、时限及取消。 |
| 状态     | 已解析、已准入、已选择 Client、已解析 Agent、已选择精确主 Panel、已单次提交控制、观察中或已结算。                          |
| 输出     | 既有 Client 控制回执、保留的操作标识与等待状态，或所属类型化读取／选择／用法失败。                                         |
| 不变量   | 一个 Session 和总时限；不激活、不重连；不启动终端／进程、不输入、不调整尺寸、不转移控制权、不激活操作系统窗口。            |
| 标识     | Agent 解析器拥有名称／ID／Panel 优先级；Workbench 拥有主 Presentation 与绑定；Client 拥有视图修订与已应用焦点。            |
| 歧义     | 缺失／结束／不可导航的 Agent 绑定，以及多个匹配主 Panel，在控制前失败。多个可用 Client 要求显式消歧。                      |
| 生命周期 | CLI 在呈现前释放 Session，通过公共输出终结器结算信号归属。                                                                 |
| 恢复     | 丢失的 Submit 不重复；保留的 Client 操作 ID 支持 `client control-status`。取消／超时只结束观察，不取消已接受的聚焦请求。   |
| 证据     | 纯语法／选择反例、脚本化命名管道轨迹、真实 Windows Agent／TUI 聚焦、语言／schema／补全检查及根检查。                       |

## 转换边界

解析在运行时影响前拒绝冲突选择器、多余参数与未知选项。显式 Client 绕过调用者元数据；
否则沿用调用者验证及唯一当前 Client 选择。Server 准入与初始快照共用原始时限。
选择 Client 后运行有界 Agent 解析器及规范 Get 或物理 Observe。显式 Panel 选择其主
Presentation；Session／名称必须在快照中对应恰好一个当前可导航主 Panel。
不以仅有的子附件替代主内容。

解析后的身份形成 `ClientViewIntent::AgentFocus`，同时保留 Agent 目标、主 Presentation、
Panel、所选 Client 视图修订及最低 Workbench 修订。Client 复核自己的状态来源和物理帧
身份。目标消失或视图变化沿用既有拒绝。Agent／Workbench 读取与后续导航并非原子：
回执确认已应用选择及当前 Client 确认，不证明绘制时 provider 存活或轮次完成。

每次解析／Get／控制等待借用同一取消源及原始时限。失败或过期的前置检查不提交控制。
提交后，已知结果与不确定性在后续读取失败、取消和超时后保留。可以不重新解析 Agent
名称，直接检查 Server 保留操作。不回滚，也不自动执行替代聚焦。

## 交付与验证

不增加命令路径或 schema 标识。`orchbee.client.control` v5 携带保留的 Agent-focus 操作，
旧版本保持封闭。既有 Agent／Client 选择失败保留来源归属。人类输出和帮助本地化，
JSON 不受语言影响。

验证名称、显式 ID、Panel 选择器、调用者与显式 Client 路由、存活主绑定，以及结束／缺失／
歧义／仅子附件的拒绝。跟踪单次 Submit 及答复丢失后的保留 Read，验证部分答复取消和
单一总时限。真实双 TUI Windows 证据检查选中 Panel 以及另一 Client／系统焦点不变，
之后停止创建的所有 Agent 进程、Site 和控制台。审查由作者完成；不宣称执行 TLA 或
独立审查。最终真实聚焦／确认验收仍须联合 Windows 验证，不能仅凭定向检查结算。

Client 必须报告 Ready 或 Busy；Starting 和 Unavailable 在 Agent 查询前返回
`client-not-ready`，保留 Client ID。Busy 准入允许 Client 执行下述模式退出前置检查，
并不保证它可以导航。

## Agent 身份与显式确认

`agent-focus` 保留解析后的规范 Session 或完整物理绑定，以及精确 TerminalContext、Tab 和
主 Panel。CLI 只解析一次，并检查所属 Agent 来源；Client 在产生效果和完成前重新验证目标。
物理目标成功前还要求所选帧的实际 Host epoch／终端身份匹配。旧帧或替换后的终端不能授权
聚焦确认。不用父内容替代只有子展示的目标；其可导航界面仍属于独立生命周期义务。

Client 执行既有顺序导航，成功绘制后完成，包括目标已经聚焦的情况。显式 Agent 聚焦成功时
确认所选 Tab 中当前已知的 Agent 版本，包括被缩放隐藏的主面板，即使外层焦点为 false。
这是与固定 Herdr `app/agents.rs` 的 `mark_active_tab_seen` 一致的显式命令效果，并非自动
绘制规则。只读 attention 和普通绘制保持既有限制。在同一 UI 边界消费当前快照；之后的
状态变化仍未读。目标状态不可读时不能宣称成功；无关观察来源不可用时不会提供虚构确认。
不增加 provider 输入或操作系统窗口激活。

目标丢失、替换、操作被中断或过期均不能确认。导航前失败为 rejected，已执行导航步骤后
失败为 incomplete。保留操作使用既有结果，状态读取不会重复效果。Client control v5 携带
这一意图；旧版本保持封闭。匹配的 Client／Server cohort 必须一起更新。纯身份／确认和
有界控制器／协议检查先于最终 Windows 实机验收；它们本身不证明多 Client 或登录 provider
行为。

## 返回终端模式

Agent 聚焦是有副作用的意图，可以在 Client 报告 Busy 时交付。普通导航和只读分类不变。
取消模式前，Client 检查精确 Agent 来源、可用终端尺寸、Ready 或 Empty 会话状态，
以及不存在待刷新、已提交或结果不确定的结构操作和关闭恢复。前置检查失败保持模式不变。
未提交的重命名／配置草稿、关闭确认及 Agent 面板可通过各自所有者取消；不能丢弃保留的
操作回执。

控制器为每条命令只执行一次退出准备。它请求 shell 一次性关闭浮层／前缀，并让每个保留的
历史视口通过既有 Recover 队列退出。不发送 provider 输入。恢复中的视口保留归属，直到
恢复及之后的实时帧完成；聚焦按原操作时限等待。模式退出也计为部分效果，即使后续导航
失败。之后的本地输入、新弹层、目标替换或过期按情况产生 rejected／incomplete，不能
确认 Agent。重复 tick 或状态读取不能再次取消新打开的模式。

模式退出后执行既有顺序导航及绘制后身份检查／确认。共享准入及纯转换检查覆盖 Busy 与
Starting、一次性退出、中断、过期和效果前失败。原生所有者检查覆盖草稿及不带输入的历史
恢复；最终联合 Windows 验收覆盖真实 shell 浮层及 Server／Client 交付。
