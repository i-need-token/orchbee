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
粘贴、fault 和 quiet-drain 规则不变。打开时捕获目标 ID、Session、revision 和原始标签；并发变化
不能重新定向草稿或自动 rebase。

Workspace 名称必须包含非空白 Unicode 标量。编辑器拒绝空白输入时保留窗口、草稿及规范标签，
并显示本地化校验说明。有效名称的前导、尾随和内部空格均精确保留。共享命令边界在 Rust、
TypeScript 与 JSON Schema 中将此规则应用于 Workspace 创建、启动和重命名。历史快照标签
仍可读取：空白 Workspace 名称在导航和当前位置栏中显示本地化的“未命名工作区”，可通过
普通重命名编辑器修复确切目标。Tab 和 Project 命名策略仍分别归属各自所有者。

重命名保留位置身份、终端 binding、目录关联及 shell 执行。其他 Client 通过显式刷新观察新标签。
导航至没有 Workspace 的 Project、删除、Project 或 Workspace 重排、目录重新绑定、Panel 布局、
远程、持久化和打包不在本片范围内。既有 Tab 重命名保持独立。

## 状态与结果契约

这些动作使用既有结构槽、carrier、单调操作预算及不可变 worker-return 证据。草稿、在途命令和
未恢复的已知结果排斥并发结构操作。打开、编辑、取消或准入失败不分配命令。

Project 菜单请求单独捕获完整目标 Project，以及发起方的 Session、revision、context、desired pane
和焦点 pane incarnation。原菜单截止时间约束 mailbox 锁的获取，并在准入锁内再次核对。
成功准入后该截止时间结束；人工编辑不计时，提交时启动既有操作预算。
草稿显示并编辑请求的 Project，即使另一个 Project 具有相同标签。
目标删除或替换、来源或焦点改变时，主动丢弃草稿、释放捕获并显示本地化拒绝。
终端输入恢复前仍要求新帧。该证明在接受提交时结束：已提交的重命名不能使自身保留的回执或恢复失效。

| 事件                 | 结果                                                                             |
| -------------------- | -------------------------------------------------------------------------------- |
| Project 菜单准入     | 要求有效的准确目标／来源证明及未过期截止时间；只打开一个草稿，不导航或发送命令。 |
| 目标／来源／焦点失效 | 丢弃尚未提交的草稿，释放结构所有权并报告拒绝，不重新定向。                       |
| 提交                 | 验证目标、输入与首次绘制后，冻结一个确切 Project 或 Workspace 重命名请求。       |
| Unchanged            | 要求捕获的 revision 和未改变的原始标签；保留已知结果。                           |
| Renamed              | 要求 checked 后继 revision 及已改变文本；在 snapshot 或 UI 交接前保留确切请求。  |
| 精确结果 snapshot    | 只允许请求的标签变化，Unchanged 则无变化；保留其他所有投影 canonical 事实。      |
| 更晚 snapshot        | 验证 Session 和确切目标归属，显示实际 canonical 标签与成员，包括 peer 变化。     |
| Snapshot 不可用      | 保留已知重命名结果；显式 `w` 再观察，不重放 mutation。                           |
| 确定拒绝或排队时退役 | 保留确切 NotApplied 证据与已有终端身份。                                         |
| 交付存在歧义         | 以确切 unknown 证据退役 carrier，不重试或虚构结果。                              |

Snapshot 安装使用既有成员 owner。仅标签变化不 detach 或重新 attach 终端；真实 peer 成员变化
遵循既有精确清理规则。退役分别保留 executor 和 mailbox 证据，包括已结算前一动作及新排队动作。
取消不撤销已提交标签。

## 正确性证据

所属实现测试覆盖Unicode 与字节上限、重复标签、Empty 和非当前目标、来源保留、准确发布、过期绘制及不明结果不重放。

在 `src/localization/tests/location_labels.rs`、`src/local_client/tests/workbench/workspace_setup/location_labels.rs` 及相邻的状态、carrier、输入和渲染套件中维护实际证据。流程具有多种结果时，使用受控完成/时钟顺序及生成轨迹重放。

运行 `cargo test --locked -p orchbee --all-targets --all-features`。此流程不另设 TLA 模型。证据覆盖命名/生成的实现轨迹，不证明全部交错、物理终端外观或未测试平台。共享身份、所有权和提交协议仍由单独注册的核心模型负责。
