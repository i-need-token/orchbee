# 原生当前 Panel 查询

## 权威与范围

本文负责 Windows `panel current` 与 `panel process-info` 的目标选择和进程生命周期。
[原生 CLI 查询](native-cli-query.zh-CN.md)负责结果投影；
[Windows Client 控制](windows-client-control.zh-CN.md)负责 Client 记录与路由失败。
现有调用者解析器负责 Terminal/Worker 身份校验。这些命令只观察已运行的 Site；
不激活 Site，不改变选择、输入、几何或控制权。

## 目标模型

语法恰好生成 `Current`、`Panel(target)`、`Client(id)` 之一：

| 输入                                                           | 解析                                                                                |
| -------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `--panel <id-or-suffix>`；`process-info` 也接受位置 Panel 参数 | 在快照中解析，忽略调用者环境且不读取 Client。                                       |
| `--client <id>`，省略目标或配合 `--current`                    | 选择指定 current Client，忽略调用者环境。                                           |
| 省略目标或 `--current`，两个调用者变量均缺失                   | 要求恰好一个 current Client。                                                       |
| 省略目标或 `--current`，存在调用者信息                         | 校验 `ORCHBEE_TERMINAL_ID` 与 `ORCHBEE_WORKER_ID`，然后在快照中解析调用者的 Panel。 |

显式 Panel 与 Client 选择器互斥。部分缺失、格式错误或过期的调用者信息绝不回退到
Client 选择。多个 Client 构成歧义；显式 Client 缺失或过期则失败。`panel current` 接受
Ready 或 Busy 观察（历史阅读属于 Busy）；`process-info` 仍只接受 Ready。
所选 Panel 在快照中仍须归属于其报告的 Context 与 Tab。成员关系缺失或改变
返回 `client-panel-unavailable`；查询不会隐式跟随该 Panel 到另一位置。

快照和 Client 记录是先后观察。`panel current` 在补充与 `panel get` 相同的 Agent、
终端和视口观察前，固定所选 Client 记录及其视图修订。它不会重新选择 Client，也不会
重定向到新 Panel。发布前，规范修订与所选 Client 的视图／修订必须仍然一致。
焦点移走后又移回也会因修订递增而失败。这些检查不使 Host／进程／滚动观察原子化；
每项观察保留各自来源。

显式／调用者 Panel 查询保持无界面可用：包含主 Agent 归属、终端标题、目录和进程观察，
不推断 Client 焦点。`scroll` 为 null，`scrollObservation.state: no-client`。
唯一／显式 Client 查询包含焦点、其观察到的实时／历史滚动或明确的不可用／等待状态，
以及读取操作来源。`process-info` 仍只读取解析出的主 Terminal，不承诺可选 OS 信息
在采样后保持不变。

## 生命周期与失败

一个绝对五秒期限覆盖发现、可取消的接入／快照、可选 Client 列表、信息补充及最终修订
检查。Client 列表保留两秒交换上限。一个中断负责人向每次可取消读取提供唤醒句柄；
没有二次接入、续期或重放。输出前结算待处理取消（退出码 130）；期限到达返回退出码 7。
增强查询将终端／进程不可用保留为明确数据；协议、权限或连接失败不输出部分成功。

会话归属遵循 `prepare → admit/snapshot → resolve → enrich/read → fence → drop → emit`。
只读 Agent 注意力及视口操作可创建有界 Client 操作回执；不确认注意力、不改变焦点、
滚动或尺寸，也不发送终端输入。每个 Host Viewer 按结果 detach；停用已接入连接是
清理兜底。部分回包超时会停用连接，真正的 EOF 仍判定为连接丢失。

路由拒绝保留 `orchbee.cli.failure.client-control` v1。信息补充采用现有 Workbench
查询失败，包括 `client-changed` 与 `workbench-changed`，不输出部分结果。通用当前
阶段失败保留精确的 `current-panel` 阶段。Windows 成功结果使用 `orchbee.panel.current`
v4，采用增强 Panel Get v7 形状及独立 schema ID。非 Windows current 保留 v3；
嵌入 schema 严格区分这两种形状。`orchbee.panel.process-info` 保持 v2。
目录 v83 包含 88 份文档和 99 条命令路径。

## 细化映射与证据

调用解析负责人约束目标和类型及选择器互斥。调用者负责人校验环境身份；纯 Client
选择负责人校验可用性、就绪状态和快照成员关系。Windows 查询组合负责期限、中断、
唯一会话及结果输出。Workbench 查询收集器负责共用的信息补充及最终一致性检查。
现有进程查询代码负责 Viewer 清理与 Host 解码。

语法／帮助／补全证据覆盖每种选择器及选项冲突。Windows 脚本化端点验证显式／调用者
路径不读取 Client、唯一／显式 Client 选择、歧义、无效调用者、过期成员关系、部分
回包的有界等待、增强／不可用观察、稳定 Busy／历史读取、进程查询就绪限制不变、Client 视图变化（包括移走再移回）、快照变化、
严格的 v3/v4 schema，以及不发送修改控制或输入请求。实际 Windows PowerShell 5.1/7 和两个
ConPTY Client 验证选择及焦点不变；部分回包配合 Ctrl+C 验证取消与对端释放。
这些检查不证明快照／Client／OS 观察的原子性，也不证明其他平台支持。

不改变共享线协议、依赖、持久状态或迁移。已有非 Windows 查询行为保持不变；新增
`--client` 模式仅支持 Windows。Windows current v3 消费者需接受 v4 增强契约。
发布和回滚在现有 cohort 内替换 CLI 可执行文件，不迁移保存数据。
