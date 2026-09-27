# 原生 Panel 标签

## 范围与归属

`panel rename <panel> --label <text>` 设置精确叶子 Panel 主 Presentation 的标题覆盖项；
`--clear` 清除覆盖项。空标签或只含 Unicode 空白的标签也执行清除。两个 flag 互斥且必须
提供其一。目标保留现有 Panel ID 或 `@panel:<suffix>` 语法；标签、隐式选择和当前 Panel
路由不属于此操作的目标形式。运行时验收平台为 Windows。

共享覆盖项归 Server 所有。`Presentation.label` 保持为不可变的创建时回退标签，可选的
`Presentation.customLabel` 仅保存非空、去除首尾空白的覆盖项。无覆盖项时快照省略该字段；
显式 `null` 不是合法的已存覆盖项。动作的可空标签更新将缺失值、`null`、空文本或纯空白
文本视为清除。传入文本在去除首尾空白前最多为 512 个 Unicode 码点。空白规则遵循 Unicode
White_Space，包含 NEL、不包含 BOM；Rust、TypeScript 与 schema 证据必须在此边界上一致。

清除恢复已有创建时回退标签。它不开始派生 Host 自然标题，不修改 Tab 标题、子 Presentation
或外层终端窗口标题。这是原生模型的标签生命周期边界，不会偷偷替换回退文本。与其他原生
规范 Workbench 状态相同，覆盖项持续到 Site 进程结束；此操作不增加重启持久化。

实现归属包括协议标签类型及 Presentation／动作契约、Server 的纯 Workbench authority、
共享原生标签改名 carrier、CLI mutation 适配器，以及
`terminal_session/workbench_projection.rs` 的显示投影。本参考需与这些 owner、独立的
TypeScript 契约及 JSON schema 同步维护。

## 命令模型

| 要素     | 契约                                                                                                                                  |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 输入     | 已准入的 navigation grant、请求 ID、精确 Site 与预期 revision、context ID、Tab ID、Panel ID，以及规范化的设置／清除更新。             |
| 状态     | 一份已验证的规范快照及既有有界命令 ledger；没有另一份标签缓存或私有回退映射。                                                         |
| 目标     | 精确 context 包含精确 Tab 与叶子 Panel。仅更新其主 Presentation，包括终端创建尚未完成时。                                             |
| 变化结果 | 在提交前准备完整候选、编码投影和 ledger 结果，然后恰好推进一个 revision。使用原请求 ID 返回 `renamed`。                               |
| 无操作   | 规范化覆盖项相同（包括清除不存在的覆盖项）时返回 `unchanged`，不推进 revision、分配身份或产生 Host 效果。既有 ledger 容量规则仍适用。 |
| 失败     | 授权、过期 revision、目标缺失、revision 耗尽及编码／ledger 容量拒绝均保留全部规范状态。                                               |
| 重放     | 沿用语义请求相等和保留结果的既有优先级。重复请求不能应用两次；冲突复用不能替换第一份回执。                                            |
| 响应丢失 | carrier 退役结果不明的提交并报告 unknown outcome，绝不重试、撤销或编造确认。                                                          |
| 确认     | 证明规范提交或无操作，不宣称任何 Client 已绘制新标题。                                                                                |

动作仅修改主 Presentation 的可选覆盖项。回退文本、全部身份、绑定、拓扑、顺序、目录及
生命周期事实保持精确不变。它不获取终端 controller，也不改变焦点、缩放、比例或 attachment。
后续 split、move 或终端创建结算必须保留存续 Presentation 上的已有覆盖项。

CLI 专用 `orchbee.panel.rename` 第 1 版结果记录原请求／Site／base、目标身份、结果 revision、
`renamed` 或 `unchanged`，以及可空的规范化 `customLabel`。请求侧元数据是关联输入，不是
编造的 Server 观察。沿用现有 text／JSON 失败 envelope 与单一绝对提交截止时间。CLI 只进行
一次快照读取及一次 mutation；不等待 Client，也不单独提交聚焦操作。

## Client 投影

`NativePanel` 分别保留不可变的创建时标签和可选自定义标签。显示优先使用覆盖项，否则使用
回退标签。Panel 标题、Agent 面板及确认文案对显示标签应用既有终端文本净化与宽度限制。
规范 Panel 查询暴露覆盖项及有效显示文本。创建默认值继续使用创建时标签，不改变既有字段
的含义。

相同 revision 的投影必须精确相等，包括自定义标签。更高 revision 允许自定义标签变化，
但保留的 Panel／Presentation／绑定／回退标签元组必须不变。合法标签刷新更新显示缓存，
保留选择、pane incarnation、viewer／controller attachment、帧及终端几何。旧发布、相同
revision 的矛盾，以及不可变回退标签或绑定变化仍按原规则拒绝。自定义标签不是 attachment
身份，也绝不授权输入。

## 证据与兼容性

既有 `WorkbenchNavigationCommit` 标签／ledger 抽象覆盖并发 Client、保留结果、过期 base、
无操作及响应丢失；目标域还需区分精确 Panel 目标。具体纯 authority 测试证明单字段修改、
创建未完成时的保留语义及字节／ledger／revision 边界。协议用例独立检查 Rust、TypeScript
与 JSON schema 的准入、规范化及存储省略语义。原生生成历史增加设置／清除和发布交错，
同时保留回退、身份及绑定对抗用例，并支持确定性重放及精简反例。

Windows 进程证据使用自有 Site 和两个 TUI 检查全局显示收敛、设置／清除／无操作、嵌套
Panel 隔离，以及不变的终端身份和选择。这与 carrier 中 definitely-not-submitted 及结果不明
的确定性测试分开。仍须通过根合入验收。维护 TLA 资产及清单；按用户要求延期执行模型，
不把它声称为已通过的证据。

没有覆盖项的快照保留既有 wire 形状。新增字段与动作要求协调升级开发 cohort：旧的严格
Client 可能拒绝包含覆盖项的快照，不声称混合版本兼容。没有持久数据迁移。合入前可整体
回退契约切片；合入后使用匹配 cohort，或先通过该 cohort 清除覆盖项再回退。绝不为了让旧
读取方接受快照而悄悄改写回退标签或丢弃规范覆盖项。

CLI Panel 读取保留创建时的 `label`，增加可空 `customLabel` 及实际显示的 `displayLabel`；
原始快照保留缺省时省略的规范自定义标题字段。当前 schema 版本与目录由
[原生 CLI 查询](native-cli-query.zh-CN.md)负责。

叶窗格标题栏中，显式自定义标题先于序号显示，保证窄窗格优先显示用户文本。没有自定义标题的标题栏保留既有本地化资源前缀。两种形式使用同一净化和显示单元截断规则。
