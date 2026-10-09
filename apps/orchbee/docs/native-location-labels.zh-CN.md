# 原生位置标签

## 范围与归属

原生 Client 使用 `Ctrl+B r` 重命名当前 Workspace，使用 `Ctrl+B Shift+R` 重命名其所属 Project。
Standalone 没有这两种快捷键目标，在本地拒绝操作且不发送命令。
已绘制 Project 的重命名动作从 Workspace 或 Standalone 为准确的目标打开同一编辑器，
包括空 Project 和非当前 Project；从不选择子 Workspace。Server 既有 `project.rename` 和
`workspace.rename` 动作拥有共享标签。本参考扩展 [Workspace 设置](native-workspace-setup.zh-CN.md)
和[上下文选择](native-context-selection.zh-CN.md)，不引入另一套 mutation owner。

编辑器预填 canonical 标签，精确保留最多 512 个 Unicode scalar，Enter 提交、Escape 取消、
Backspace 删除、Ctrl+U 清空。每次打开分配新 token，并要求首次完整绘制。共享 modal 的 UTF-8、
粘贴、fault 和 quiet-drain 规则不变。打开时捕获目标 ID、Site、revision 和原始标签；并发变化
不能重新定向草稿或自动 rebase。

重命名保留位置身份、终端 binding、目录关联及 shell 执行。其他 Client 通过显式刷新观察新标签。
导航至没有 Workspace 的 Project、删除、Project 或 Workspace 重排、目录重新绑定、Panel 布局、
远程、持久化和打包不在本片范围内。既有 Tab 重命名保持独立。

## 状态与结果契约

这些动作使用既有结构槽、carrier、单调操作预算及不可变 worker-return 证据。草稿、在途命令和
未恢复的已知结果排斥并发结构操作。打开、编辑、取消或准入失败不分配命令。

Project 菜单请求单独捕获完整目标 Project，以及发起方的 Site、revision、context、desired pane
和焦点 pane incarnation。原菜单截止时间约束 mailbox 锁的获取，并在准入锁内再次核对。
成功准入后该截止时间结束；人工编辑不计时，提交时启动既有操作预算。
草稿显示并编辑请求的 Project，即使另一个 Project 具有相同标签。
目标删除或替换、来源或焦点改变时，主动丢弃草稿、释放捕获并显示本地化拒绝。
终端输入恢复前仍要求新帧。该证明在接受提交时结束：已提交的重命名不能使自身保留的回执或恢复失效。

| 事件                 | 结果                                                                            |
| -------------------- | ------------------------------------------------------------------------------- |
| Project 菜单准入     | 要求有效的准确目标／来源证明及未过期截止时间；只打开一个草稿，不导航或发送命令。 |
| 目标／来源／焦点失效 | 丢弃尚未提交的草稿，释放结构所有权并报告拒绝，不重新定向。                       |
| 提交                 | 验证目标、输入与首次绘制后，冻结一个确切 Project 或 Workspace 重命名请求。      |
| Unchanged            | 要求捕获的 revision 和未改变的原始标签；保留已知结果。                          |
| Renamed              | 要求 checked 后继 revision 及已改变文本；在 snapshot 或 UI 交接前保留确切请求。 |
| 精确结果 snapshot    | 只允许请求的标签变化，Unchanged 则无变化；保留其他所有投影 canonical 事实。     |
| 更晚 snapshot        | 验证 Site 和确切目标归属，显示实际 canonical 标签与成员，包括 peer 变化。       |
| Snapshot 不可用      | 保留已知重命名结果；显式 `w` 再观察，不重放 mutation。                          |
| 确定拒绝或排队时退役 | 保留确切 NotApplied 证据与已有终端身份。                                        |
| 交付存在歧义         | 以确切 unknown 证据退役 carrier，不重试或虚构结果。                             |

Snapshot 安装使用既有成员 owner。仅标签变化不 detach 或重新 attach 终端；真实 peer 成员变化
遵循既有精确清理规则。退役分别保留 executor 和 mailbox 证据，包括已结算前一动作及新排队动作。
取消不撤销已提交标签。

## 正确性证据

有界位置标签模型覆盖两类目标、无变化及变化结果、首次绘制、revision 竞争、精确身份、
已知结果保留、显式观察及至多一次提交。既有 Server 和成员模型保留 canonical 提交及清理义务。
确定性测试覆盖有类型结果兼容、归属、no-op 与溢出、同包输入、旧绘制和退役证据。Linux 原生
进程测试执行两种重命名控制、peer 刷新、取消与 shell 身份保留。这些开发见证不证明安装后或
跨平台就绪。

[`NativeLocationLabels`](../formal/tla/NativeLocationLabels.tla) 探索一个 Client 和 Site、
一个 Project 及其一个 Workspace、一个独立 Project、两个标签值、两个请求/草稿身份，以及一次 peer 标签或成员写入。
其 57 个必需动作覆盖显式目标转交、截止时间到期、证明失效和主动拒绝结算，以及前一 executor
与后一排队 mailbox 证据重叠。这些有限边界不限制产品交互。
精确 UTF-8 和 scalar 计数、整数溢出、实际 mailbox cell 及完整 Worker 行为，仍由确定性测试或
下层模型承担。

`ProjectRenamePreservesOrigin` 区分目标与发起位置；`ProjectRenameAdmissionIsFresh` 约束准入，
`SubmissionCapturesExactDraft` 和既有回执属性保留选定身份。证明失效抽象目标、来源及焦点变化；
确定性用例区分具体记录以及非首个 Tab／Panel 焦点。owner 用例覆盖重复标签、Empty 来源、
取消、一次转交、无变化／变化回执、精确 snapshot 差异和只读恢复。目标扩展只有源码审阅证据；
此处不声称已有执行的 TLC、原生测试或主机结果。
