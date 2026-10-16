# 共享分屏比例

## 范围与归属

Workbench 拥有每个二元分屏的名义比例。在一个 Windows TUI Client 中调整分屏后，所有 Client
看到同一比例，重启 checkpoint 也保留它。每个 Client 仍拥有自己的视口、焦点和 zoom。
终端尺寸仍由控制者的投影决定；改变共享比例不会取得终端控制权或重放输入。

本契约拥有 canonical 比例变更及其原生 Client 精化。键盘调整、分隔线手势、热升级和离线降级
转换属于独立能力。现有 CLI `panel resize`、`layout set-split-ratio` 及布局导出/应用是消费者。

## 数据与边界

canonical `PanelNode::Split` 包含有限的 `ratio`，范围为 `0.1..=0.9`，表示取整前第一个有序
slot 的份额。缺失表示 `0.5`；序列化省略均分默认值。split ID 与两个有序 slot ID 一起
标识分屏。新分屏从均分开始；移动现有子树保留比例，删除分屏则同时删除其比例。

`layout.set-split-ratio` 是经过认证的 Workbench 命令，携带 Site、request ID、精确的预期
Workbench revision、TerminalContext、Tab、分屏身份、原比例和请求比例。现有导航变更 grant
准入此命令。视口几何和进程句柄都不作为 canonical 状态。无效比例在有界协议入口拒绝；
CLI 归一化保留现有有限数值和夹取规则。

Workbench 协议版本 1 在匹配的可执行 cohort 内准入这一增量字段与命令。缺失比例的旧文档
仍可读取。共享比例最初在 checkpoint 版本 5 引入，当前版本 6 继续保留；版本 1–4 恢复均分，不能引入非默认比例。
session catalog 版本 4 引入比例准入；当前 catalog 版本 5 还准入可选 Project 组织，
并在激活或打开升级后的存储前发布。此前 cohort 拒绝此记录，且不修改
其字节。catalog 版本 3–5 使用现有带 framing 的 checkpoint 存储；frame 格式、三副本
保留、字节上限与自动保存时序不变。回退使用另一个 Site 或另行授权的离线转换。
不支持导入活跃 Client 的旧 override、自动降级或跨 cohort 连接。

## Canonical 转移

现有 Workbench authority 和 request ledger 拥有候选投影、publication 和保留结果的一次
原子提交。验证、分配、编码和 ledger 容量检查均在提交前完成。提交后的传输失败不能撤销比例。

| 事件或守卫                                     | 状态与结果                                                                 |
| ---------------------------------------------- | -------------------------------------------------------------------------- |
| 未授权、Site 错误、authority 已退役或容量耗尽  | 现有类型化拒绝/不可用；不写比例                                            |
| request ID 相同且完整命令相同                  | 重放保留结果，不再增加 delta 或 revision                                   |
| request ID 相同但命令不同                      | 请求身份冲突；不写入                                                       |
| revision 或原比例错误                          | Revision mismatch；不写入，也不自动重试                                    |
| context/Tab/split 缺失，或任一有序 slot 被替换 | 类型化目标不可用；不写入                                                   |
| context 有未完成的 close fence                 | Terminal busy；不写入                                                      |
| 请求比例等于当前比例                           | 保留 `unchanged`；revision 和 publication 不变                             |
| 全部守卫通过且比例改变                         | 恰好推进一次 revision、替换 canonical 比例并发布；`split-ratio-configured` |
| 从已准入的 checkpoint 重启                     | 重建图保留名义比例，不保留活绑定或 controller grant                        |

比例不改变成员、标签、选择或身份分配高水位。过期 Client 不能把先前解析出的 resize delta
写到替换后的分屏。同一 revision 上的并发命令不能同时提交；失败的调用者须明确取得新视图后
再发起新操作。

## 原生 Client 工作流

UI 根据已准入的 canonical Snapshot 与渲染缓存，仅解析一次几何。resize 保留现有最近有效
边界、方向、有符号步长和夹取规则；路径 setter 保留可选的预期身份。比例未变或没有边界时为
no-op，不发送命令、不要求绘制，也不调整终端尺寸。

| 阶段与事件                                  | 下一状态与可观察效果                                           |
| ------------------------------------------- | -------------------------------------------------------------- |
| 准备好有变更的 adjustment                   | 一个自有、可取消 worker 提交 canonical 命令；UI 不乐观写入比例 |
| 明确未提交或类型化拒绝                      | Rejected 回执；本操作未改变显示及 canonical 状态               |
| 提交结果不明                                | Incomplete 回执；保留已观察状态，绝不自动重放命令              |
| inactive/未访问 Tab 的 canonical 提交已确认 | 确认提交 revision，不选择、连接或绘制目标                      |
| selected Tab 的 canonical 提交已确认        | 等待现有 publication refresh 和一次成功的兼容绘制，随后确认    |
| 提交后输入、模态打断、超时、断连或关闭      | Incomplete 或未结算结果；不声称回滚，不隐式重发                |
| 确认后的 publication                        | 正常共享投影，不再加上原 delta                                 |

worker 拥有 deadline 和关闭唤醒端点；创建者在释放资源前 join。心跳独立运行。过期完成
不能结算另一操作。selected 成功要求渲染缓存包含已提交比例，且修订不早于提交，并保留原选择与 zoom。
提交后失败可能使新比例仍然可见。绘制成功不代表异步 Host resize 已完成。

规范布局查询及修改准备始终读取 canonical 比例。受守卫约束的[局部分隔线草稿](../../orchbee/docs/native-divider-resize.zh-CN.md)
可暂时为其来源 Client 的绘制、终端尺寸及指针区域提供一个比例。按住移动不发布共享比例；
松手仍只提交一次精确规范 CAS。该显示租约不能进入共享缓存、查询结果或 checkpoint。轴空间扣除共享
分隔线；现有半入取整、最小尺寸夹取、标题行、zoom 和小视口折叠仍为权威。即使取整或最小
尺寸限制了物理结果，名义比例也保持可观察。只读 pane 观察布局，只有当前控制者调整其终端。

## 正确性与验证

| 主张                                                    | 证据与精化                                                                           |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 单一写入者、精确身份/revision 守卫、重放与 no-op 稳定性 | Workbench authority 确定性和生成状态轨迹，含序列化重放与缩减                         |
| 两种语言中的有界有限字段与命令                          | Rust codec、独立 TypeScript/Zod 和 JSON Schema 契约案例                              |
| 提交、publication、绘制与打断独立排序                   | 确定性 Client control 与 worker 轨迹覆盖提交、发布、绘制与打断                       |
| 旧版默认值、升级准入与持久保留                          | checkpoint 往返/版本拒绝及 Windows catalog 准入案例                                  |
| 共享名义比例精化为实际几何                              | Windows ConPTY 双 Client、控制者/只读、detach/reattach、停止/重启及布局导出/应用轨迹 |

抽象模型检查一个已准入的 Site generation、两个 Client、两种 split incarnation、三种比例
等价类和每个 Client 一个请求；selected/inactive 阶段是显式的。checkpoint 发布和激活仍属于
独立的存储/启动模型，不把其状态空间乘进比例提交模型。唯一公平性假设是 Server 最终结算
完整提交的请求；UI 打断和缺失回执不假设绘制成功。它不证明无界拓扑、Win32 I/O 或超出
声明假设的调度公平性；生成层级轨迹补充该范围。真实 Windows 轨迹使用正常二进制和管道；
物理重启、真人 Windows Terminal 交互和其他平台仍须独立验收。除聚焦检查外，仓库合入 gate
同样适用。

selected 成功可使用更新修订，只要 split 身份及提交比例仍保持；回执记录实际绘制缓存修订。`splitRevision` 使用投影缓存的规范 Workbench 修订。
