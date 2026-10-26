# 原生当前 Panel 查询

## 权威与范围

本文负责 Windows `panel current` 与 `panel process-info` 的目标选择和进程生命周期。
[原生 CLI 查询](native-cli-query.zh-CN.md)负责结果投影；
[Windows Client 控制](windows-client-control.zh-CN.md)负责 Client 记录与路由失败。
现有调用者解析器负责 Terminal/Worker 身份校验。这些命令只观察已运行的 Session；
不激活 Session，不改变选择、输入、几何或控制权。

## 目标模型

语法恰好生成 `Current`、`Panel(target)`、`Client(id)` 之一：

| 输入                                         | 解析                                                                                |
| -------------------------------------------- | ----------------------------------------------------------------------------------- |
| `<panel-id-or-handle>` 位置参数              | 在快照中解析，忽略调用者环境且不读取 Client。                                       |
| `--client <id>`，省略目标或配合 `--current`  | 选择指定 current Client，忽略调用者环境。                                           |
| 省略目标或 `--current`，两个调用者变量均缺失 | 要求恰好一个 current Client。                                                       |
| 省略目标或 `--current`，存在调用者信息       | 校验 `ORCHBEE_TERMINAL_ID` 与 `ORCHBEE_WORKER_ID`，然后在快照中解析调用者的 Panel。 |

显式 Panel 与 Client 选择器互斥。部分缺失、格式错误或过期的调用者信息绝不回退到
Client 选择。多个 Client 构成歧义；显式 Client 缺失或过期则失败。`panel current` 接受
Ready 或 Busy 观察（历史阅读属于 Busy）；`process-info` 仍只接受 Ready。
所选 Panel 在快照中仍须归属于其报告的 Context 与 Tab。成员关系缺失或改变
返回 `client-panel-unavailable`；查询不会隐式跟随该 Panel 到另一位置。

快照和 Client 记录是先后观察。`panel current` 在补充精确当前成员的 Agent、
终端、报告、元数据和视口观察前，固定所选 Client 记录及其视图修订。它不会重新选择 Client，也不会
重定向到新 Panel。发布前，规范修订与所选 Client 的视图／修订必须仍然一致。
焦点移走后又移回也会因修订递增而失败。这些检查不使 Host／进程／滚动观察原子化；
每项观察保留各自来源。

显式／调用者 `panel current` 查询保持无界面可用：包含解析成员的 Agent 归属、终端标题、目录和进程观察，
不推断 Client 焦点。`scroll` 为 null，`scrollObservation.state: no-client`。
唯一／显式 Client 查询包含焦点、其观察到的实时／历史滚动或明确的不可用／等待状态，
以及读取操作来源。`process-info` 按下文读取精确的调用者／Client 成员；显式 Panel
目标使用主终端。不承诺可选 OS 信息在采样后保持不变。

## 生命周期与失败

一个绝对五秒期限覆盖发现、可取消的接入／快照、可选 Client 列表、信息补充及最终修订
检查。Client 列表保留两秒交换上限。一个中断负责人向每次可取消读取提供唤醒句柄；
没有二次接入、续期或重放。输出前结算待处理取消（退出码 130）；期限到达返回退出码 7。
增强查询将终端／进程不可用保留为明确数据；协议、权限或连接失败不输出部分成功。

会话归属遵循 `prepare → admit/snapshot → resolve → enrich/read → fence → drop → emit`。
只读 Agent 注意力、Presentation 及视口操作可创建有界 Client 操作回执；不确认注意力、不改变焦点、
滚动或尺寸，也不发送终端输入。每个 Host Viewer 按结果 detach；停用已接入连接是
清理兜底。部分回包超时会停用连接，真正的 EOF 仍判定为连接丢失。

路由拒绝保留 `orchbee.cli.failure.client-control` v1。信息补充采用现有 Workbench
查询失败，包括 `client-changed` 与 `workbench-changed`，不输出部分结果。通用当前
阶段失败保留精确的 `current-panel` 阶段。Windows 成功结果使用 `orchbee.panel.current`
v8，保留规范 Panel 结构并显式携带 `currentPresentation`。非 Windows current 保留 v3；
嵌入 schema 严格区分这两种形状。`orchbee.panel.process-info` 保持 v2。
目录 v100 包含 92 份文档和 109 条命令路径。

## 细化映射与证据

调用解析负责人约束目标和类型及选择器互斥。调用者负责人校验环境身份；纯 Client
选择负责人校验可用性、就绪状态和快照成员关系。Windows 查询组合负责期限、中断、
唯一会话及结果输出。Workbench 查询收集器负责共用的信息补充及最终一致性检查。
现有进程查询代码负责 Viewer 清理与 Host 解码。

语法／帮助／补全证据覆盖每种选择器及选项冲突。Windows 脚本化端点验证显式／调用者
路径不读取 Client、唯一／显式 Client 选择、歧义、无效调用者、过期成员关系、部分
回包的有界等待、增强／不可用观察、稳定 Busy／历史读取、进程查询就绪限制不变、Client 视图变化（包括移走再移回）、快照变化、
严格的旧版／当前 schema，以及不发送修改控制或输入请求。实际 Windows PowerShell 5.1/7 和两个
ConPTY Client 验证选择及焦点不变；部分回包配合 Ctrl+C 验证取消与对端释放。
这些检查不证明快照／Client／OS 观察的原子性，也不证明其他平台支持。

被动 Presentation 读取扩展配套私有线协议 cohort；不改变依赖、持久状态或迁移。
已有非 Windows 查询行为保持不变；新增
`--client` 模式仅支持 Windows。Windows current v3 消费者需接受 v8 增强契约。
发布和回滚在现有 cohort 内替换 CLI 可执行文件，不迁移保存数据。

## 进程信息的精确当前 Presentation

Windows `panel process-info` 对显式 Panel 使用主终端；提供调用者元数据时保留经过验证的
精确终端，包括子 Presentation。使用 Client 时，先被动读取 `Presentation { panelId }`
再查询 Host：Workbench Client 拥有所选 Presentation、绑定、附着代次和窗格实例编号。
本地成员不存在时返回不可用，不回退到主终端。现有 process-info v2 已标识精确终端和
Presentation，其封闭结构保持不变。

流程为快照 → 精确成员读取 → Host 进程读取及查看器清理 → 规范版本栅栏 → 精确成员读取。
两次 Client 读取都要求原始视图版本和相同的观察视图；Presentation ID、绑定、附着代次
及窗格实例编号必须一致。仅视口更新不会使进程信息失效。规范状态改变、Client 选择改变、
分离重附着或切走再切回都会在发布前失败。显式与调用者读取也检查规范成员版本。所有阶段
共用现有会话、五秒绝对截止时间及取消所有者；读取不发送输入或焦点请求。

被动 DTO 限制为 4 KiB，`presentation` 字段必须存在但可为 null。已绑定成员包含其精确
已安装视口的观察；成员缺席不生成视口事实。原生投影验证规范所有者及绑定后导出现有本地
代次标识。Server 仅保留有界操作回执。私有配套 cohort 一同发布并通过源码回滚，不需要
持久化迁移。Client control 回执 v6 接纳此新意图，v1–v5 仍保持封闭。

证据覆盖严格契约解析及回复身份、不会借用主终端的原生子成员投影、调用者子终端解析，
以及受控 Windows CLI 请求顺序和过时成员拒绝。完整 Windows/provider/多 Client 验收
仍留到最终阶段。下文的信息补充使用同一成员身份；[报告写入](windows-panel-agent-reports.zh-CN.md#精确成员目标)单独选择显式成员。

## 当前成员信息增强

Windows `panel current` v8 在组合观察前解析一个规范 Presentation。显式 Panel 目标选择
主项（包括未绑定主项），调用者元数据选择精确的已绑定终端，Client 目标使用被动精确成员
读取。`currentPresentation` 标识此成员、规范标签／覆盖／绑定及来源（`primary`、`caller`，
带本地附着代次／实例编号的 `client`，或带选择代次的 `client-unbound`）。规范 `primaryPresentation` 与 `childPresentations`
保留真实结构。已有 v3–v7 保持封闭；Panel Get/List 保留原有主项语义。

当前 Agent 归属、终端标题／目录／运行时、报告、元数据及 Client 滚动均使用同一已解析
成员。精确报告读取根据初始快照验证 Presentation ID、所属 Panel 及绑定；缺失或不匹配
的行会失败。元数据／报告修订分别通过已有空全局读取设栅栏：空行不指向任何主项，且其
修订所有者与精确成员读取共用。汇总限制和交叠读取／栅栏顺序保持不变。无界面读取不
推断 Client 焦点或滚动。

Client 成员读取包围信息组合及最终规范修订检查。两次回执都要求原始 Client 视图修订
及观察；本地成员标识必须一致，但允许仅视口更新。两次操作 ID 都保留在查询来源中。
成员缺失、规范变化、重附着或选择变化不返回部分结果。流程复用唯一已接入会话、原始
五秒期限以及现有中断／viewer 清理所有者，不声称 Host／进程／Agent 观察原子化，也不
产生输入或焦点效果。

纯目标测试与实际 Windows CLI 端点轨迹覆盖主项、子项调用者、所选子项、未绑定显式
主项、精确报告／元数据关联、过时代次／修订、失败封套及旧 schema 拒绝。最终 provider／
多 Client／全量检查仍后置。不需要新依赖或持久化迁移；未绑定线格式分支见下文。更新原生 CLI 与现有配套
cohort 一起发布，通过源码回滚。[报告写入](windows-panel-agent-reports.zh-CN.md#精确成员目标)单独选择显式成员。

## 未绑定的当前成员

稳定选中的空 Presentation 是真实的 Client 本地成员。其被动线协议选择为
`{presentationId, selectionGeneration}`；非零十进制代次来自现有选择锚点，不伪造挂接。
已绑定选择保留既有绑定、挂接、实例和 viewport 形状。Null 仍表示没有本地成员。只有 Ready
或稳定的 SelectedWithoutAttachment 可导出空身份；待完成 Switch 的目标不属于观测结果。
Client 回答前检查精确的规范 Panel／成员归属及绑定缺席。

Windows current v8 使用 `source: {kind: "client-unbound", selectionGeneration}`，且 `binding: null`。
前后两次成员读取必须保持相同种类、ID 和代次，同时维持既有规范修订与 Client view 围栏。
精确 report／metadata／Agent 关联保留空成员，scroll 为 not-attached，不查询任何物理终端。
Client process-info 以既有缺失终端结果失败，绝不借用已绑定兄弟。旧 current v7 与 control v6
拒绝新形状。严格跨绑定向量、状态／UI 读取和真实 Windows CLI 管道轨迹覆盖这些规则；
provider 与多 Client 视觉验收仍留在最终验证。这扩展匹配的私有 cohort 及公开 current v8／control v7，
按源码回滚无需数据迁移。
