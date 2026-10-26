# 原生 Panel 标签

## 范围与归属

`panel rename <panel> --label <text>` 设置精确叶子 Panel 主 Presentation 的标题覆盖项；
`--clear` 清除覆盖项。空标签或只含 Unicode 空白的标签也执行清除。两个 flag 互斥且必须
提供其一。目标保留现有 Panel ID 或 `@panel:0x<hex>` 语法；标签、隐式选择和当前 Panel
路由不属于此操作的目标形式。运行时验收平台为 Windows。

共享覆盖项归 Server 所有。`Presentation.label` 保持为不可变的创建时回退标签，必填的
`Presentation.customLabel` 保存非空、去除首尾空白的覆盖项或显式 `null`。
字段省略无效；null 表示没有覆盖项。动作的可空标签更新将缺失值、`null`、空文本或纯空白
文本视为清除。传入文本在去除首尾空白前最多为 512 个 Unicode 码点。空白规则遵循 Unicode
White_Space，包含 NEL、不包含 BOM；Rust、TypeScript 与 schema 证据必须在此边界上一致。

清除恢复已有创建时回退标签。它不开始派生 Host 自然标题，不修改 Tab 标题、子 Presentation
或外层终端窗口标题。这是原生模型的标签生命周期边界，不会偷偷替换回退文本。与其他原生
规范 Workbench 状态相同，覆盖项持续到 Session 进程结束；此操作不增加重启持久化。

实现归属包括协议标签类型及 Presentation／动作契约、Server 的纯 Workbench authority、
共享原生标签改名 carrier、CLI mutation 适配器，以及
`workbench_client/workbench_projection.rs` 的显示投影。本参考需与这些 owner、独立的
TypeScript 契约及 JSON schema 同步维护。

## 命令模型

| 要素     | 契约                                                                                                                                  |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 输入     | 已准入的 navigation grant、请求 ID、精确 Session 与预期 revision、context ID、Tab ID、Panel ID，以及规范化的设置／清除更新。          |
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

CLI 专用 `orchbee.panel.rename` 第 1 版结果记录原请求／Session／base、目标身份、结果 revision、
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

[Server 权威测试](../../orchbee-server/src/workbench_authority/tests/panel_labels.rs)覆盖规范化设置/清除、
无变化、重放、非目标保留、pending 创建、授权及容量。[原生投影轨迹](../src/workbench_client/workbench_projection/tests/label_evolution.rs)
覆盖自定义标签演进及不可变身份拒绝。这些维护中的测试不会针对同一 Session 运行两个原生 TUI。

原生 Windows 进程验证必须使用专属 Session 与两个 TUI，检查全局显示收敛、设置/清除/无变化、
嵌套 Panel 隔离及终端身份不变。运行适用根门禁，并单独记录该原生见证；仅通过所有者测试或
门禁本身不证明该交互。不保留独立标签 TLA 模型。

快照和布局 Presentation 要求显式可空覆盖项。匹配的所有 Client 使用这一形状；字段省略会被
拒绝，不进行迁移。规范回退标签不能只含空白。不得为了适应旧读取器而改写回退文本或丢弃覆盖项。

CLI Panel 读取保留创建时的 `label`，增加可空 `customLabel` 及实际显示的 `displayLabel`；
原始快照要求规范的可空自定义标题字段。当前 schema 版本与目录由
[原生 CLI 查询](native-cli-query.zh-CN.md)负责。

叶窗格标题中，已提供的自定义 Presentation 标题或 metadata 标题不再附加 Panel 序号，包括显式提供的数字标题。两者都没有时，生成的标题保留本地化资源前缀和 Panel 序号。创建标签若是继承的纯 ASCII 数字，则从该标题省略；非数字创建标签显示在序号后。这一显示选择不重写不可变标签。所有形式使用同一净化和显示单元截断规则。

<!-- documentation-pairing: english-sha256=c944e291d997fb57437773eb4ce9684e99fbdbe369375484b71406865589d25f zh-cn-sha256=834854d294a4cfad29d10f8194a8ef0167ea783d224f77f2fe5566d29c496574 -->
