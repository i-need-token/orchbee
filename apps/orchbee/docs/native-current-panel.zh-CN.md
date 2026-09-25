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
Client 选择。多个 Client 构成歧义；显式 Client 缺失或过期则失败。所选 Client 必须
Ready，且所选 Panel 在快照中仍归属于其报告的 Context 与 Tab。成员关系缺失或改变
返回 `client-panel-unavailable`；查询不会隐式跟随该 Panel 到另一位置。

快照和 Client 记录是先后观察，并非原子的焦点读取。结果标识从这些观察解析出的
Panel；后续焦点变化不会重定向查询。`process-info` 通过现有只读 Viewer 进程查询
读取该 Panel 的主 Terminal，不承诺可选 OS 信息在采样后仍保持不变。

## 生命周期与失败

一个绝对五秒期限覆盖 cohort 发现、接入、快照、可选 Client 列表和可选 Host 进程
读取。Client 列表还保留现有两秒交换上限。接入前启用中断负责人；Client 与 Viewer
读取借用其唤醒句柄。现有接入／快照阶段有界，但不借用该句柄。追加读取前检查待处理
取消，输出前结算取消，返回退出码 130 和通用取消文档。超时返回退出码 7。请求不重试。

LocalClient 交换负责人将超时 I/O（包括部分 Client 回包）判定为超时，并停用该连接。
真正的 EOF 仍为连接丢失；两种结果都不允许重放。

会话归属遵循 `prepare → admit/snapshot → resolve → project/read → drop → emit`。
进程查询保留现有按结果释放 Viewer 的规则；detach 无法完成时以释放会话兜底。
路由拒绝保留不带快照来源的 `orchbee.cli.failure.client-control` v1；其他失败保留
各自负责的 schema。成功结果仍为 `orchbee.panel.current` v3 和
`orchbee.panel.process-info` v2。通用失败 v1 schema 为取消、超时和 Client 交换失败添加精确的 `current-panel` 阶段。
目录版本 41 含 72 份文档、85 条命令路径；成功结果形状和命令路径保持不变。

## 细化映射与证据

调用解析负责人约束目标和类型及选择器互斥。调用者负责人校验环境身份；纯 Client
选择负责人校验可用性、就绪状态和快照成员关系。Windows 查询组合负责期限、中断、
唯一会话及结果输出。现有进程查询代码负责 Viewer 清理与 Host 解码。

语法／帮助／补全证据覆盖每种选择器及选项冲突。Windows 脚本化端点验证显式／调用者
路径不读取 Client、唯一／显式 Client 选择、歧义、无效调用者、过期成员关系、部分
回包的有界等待，以及不发送控制或输入请求。实际 Windows PowerShell 5.1/7 和两个
ConPTY Client 验证选择及焦点不变；部分回包配合 Ctrl+C 验证取消与对端释放。
这些检查不证明快照／Client／OS 观察的原子性，也不证明其他平台支持。

不改变共享线协议、依赖、持久状态或迁移。已有非 Windows 查询行为保持不变；新增
`--client` 模式仅支持 Windows。旧 CLI 会拒绝该选项。发布和回滚在现有 cohort 内替换
CLI 可执行文件。
