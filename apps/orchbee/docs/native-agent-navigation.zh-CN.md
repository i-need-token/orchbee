# 原生 Agent 导航

## 权威与语法

Windows `agent focus` 将存活 Agent 解析为一个可导航的主 Panel，并向一个现有 TUI Client
路由带确认的聚焦请求。支持一个原生 Agent 位置目标，或 `--session`、`--name`、`--panel`，
沿用既有优先级与互斥规则。`--client`、`--request-id`、`--detach`、`--timeout` 和
`--format text|json` 沿用 Client 控制契约。默认总时限为 30000 毫秒。帮助、补全和解析
保持离线。不增加 macOS 或 Linux 运行时。

该命令改变 Client 选择。[Client 提示状态所有者](native-client-attention.zh-CN.md)
只在成功绘制可见窗格后确认精确 Agent 版本，已知失焦时禁止确认。
规范 Agent 生命周期、语义状态和新鲜度是观测，不由聚焦操作重写。

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
选择 Client 后运行有界原生 Agent 解析器及一次 Agent Get。显式 Panel 选择其主
Presentation；Session／名称必须在快照中对应恰好一个当前可导航主 Panel。
不以仅有的子附件替代主内容。

解析出的 Panel 标识转换为既有 `ClientViewIntent::Panel`，使用选中 Client 视图修订及
最低 Workbench 修订作为栅栏。共享纯意图构建器不执行 Agent I/O。目标消失或视图变化
沿用既有 Client 控制拒绝。Agent／Workbench 观测与后续聚焦并非原子；回执确认 Client
导航，不证明绘制时 Agent 仍运行或已完成一轮。

每次解析／Get／控制等待借用同一取消源及原始时限。失败或过期的前置检查不提交控制。
提交后，已知结果与不确定性在后续读取失败、取消和超时后保留。可以不重新解析 Agent
名称，直接检查 Server 保留操作。不回滚，也不自动执行替代聚焦。

## 交付与验证

新增一个命令路径（共 84 个），不新增 schema；内嵌目录保持 v48/72。
共享 `orchbee.client.control` v1 回执及既有 Agent／Client 选择失败保留来源归属。
人类输出和帮助本地化，JSON 不依赖语言。

验证名称、显式 ID、Panel 选择器、调用者与显式 Client 路由、存活主绑定，以及结束／缺失／
歧义／仅子附件的拒绝。跟踪单次 Submit 及答复丢失后的保留 Read，验证部分答复取消和
单一总时限。真实双 TUI Windows 证据检查选中 Panel 以及另一 Client／系统焦点不变，
之后停止创建的所有 Agent 进程、Site 和控制台。审查由作者完成；不宣称执行 TLA 或
独立审查。seen/unseen 行为仍是单独、明确的报告义务。

客户端必须已就绪；否则在 Agent 查询前返回现有客户端选择失败文档中的 `client-not-ready`，保留客户端 ID。目录 v39 已扩充该 v1 原因集合，不新增 schema。
