# 产品语言

## 权威与用法

本参考负责每个已交付 locale 中获准使用的产品术语和重复交互文案。完整消息仍以运行时目录为权威；本参考约束用词，但不取代描述符、译者上下文或 locale 文件。

添加或更改产品副本之前：

1. 确定产品概念或用户操作，再使用下方获准的术语或 Action 标签。
2. 搜索受影响目录和相邻界面，找出相互竞争的措辞。
3. 只有具体产品概念反复出现或可能产生不一致翻译时，才添加术语。
4. 只有重复出现的产品场景存在多个合理标签时，才添加交互规则。
5. 获准决策发生变化时，在同一变更中更新本参考和每个受影响的 locale 目录。

完整消息的语法可以变化，但底层产品概念和 Action 含义必须稳定。可见文本相同并不足以证明可以复用语义无关的消息 ID。

## 产品术语

| 概念              | `en-US`           | `zh-CN`    | 用法                                                    | 避免用作同义词              |
| ----------------- | ----------------- | ---------- | ------------------------------------------------------- | --------------------------- |
| Orchbee           | Orchbee           | Orchbee    | 产品名称                                                | 音译或翻译的产品名称        |
| Browser access    | Browser access    | 浏览器访问 | Desktop 中用于通过浏览器使用终端的界面                  | Web 访问、浏览器连接        |
| Host              | Host              | 主机       | 保留 Host 投影颜色的终端显示选项                        | Server、设备                |
| Keyboard shortcut | Keyboard shortcut | 键盘快捷键 | 绑定到一个产品语义 Action 的物理组合键                  | 热键、快捷方式              |
| Planned           | Planned           | 规划中     | 明确不可用且不产生行为的可见未来设置                    | Coming soon、即将推出、未来 |
| Project           | Project           | 项目       | 导航中表示的产品项目                                    | 工程                        |
| Settings          | Settings          | 设置       | 产品配置入口或界面                                      | 偏好设置、配置              |
| Sidebar           | Sidebar           | 边栏       | 位于边缘、用于辅助导航或工具的界面                      | 侧边栏、侧栏                |
| Tab               | Tab               | 标签页     | 可选择的项目及其关联内容界面                            | 选项卡、页签                |
| Tab group         | Tab group         | 标签页组   | 拥有一个或多个标签页的集合                              | 标签组、分组                |
| Tab strip         | Tab strip         | 标签栏     | 呈现和管理标签页的容器                                  | 标签页栏、选项卡栏          |
| Terminal          | Terminal          | 终端       | 终端会话或终端界面                                      | 控制台、命令行              |
| Terminal session  | Terminal session  | 终端会话   | 由一个 Terminal 呈现所拥有的单次 shell/进程运行生命周期 | 控制台会话、命令行会话      |
| Window            | Window            | 窗口       | 原生 Desktop 应用窗口                                   | 页面、视图                  |

不要仅为扩充表格而加入通用词汇。只记录稳定翻译确实能避免歧义的产品术语。

## 交互 Action

优先使用具体的动作动词，不使用泛化的确认标签。表单位于对话框内并不意味着其主要 Action 就是 `Submit`；应直接说明该 Action 的结果。

| 意图                          | `en-US` | `zh-CN` | 用法                               | 不要替换为                       |
| ----------------------------- | ------- | ------- | ---------------------------------- | -------------------------------- |
| Create a durable entity       | Create  | 创建    | 成功后存在一个新的具名实体         | Submit、OK、Confirm              |
| Persist edits                 | Save    | 保存    | 当前编辑变为持久状态               | Submit、Confirm                  |
| Add to a collection           | Add     | 添加    | 附加或插入项目，但不表示创建新实体 | Create、Submit                   |
| Send for processing           | Submit  | 提交    | 请求、工作流或审批进入下游处理     | Save、OK；不要用于所有对话框表单 |
| Apply a choice                | Apply   | 应用    | 设置或选择开始生效                 | Submit                           |
| Confirm a stated decision     | Confirm | 确认    | 交互明确要求用户确认先前选择       | 泛化的表单完成；优先使用具体动词 |
| Acknowledge information       | OK      | 确定    | 对话框不包含领域 Action 或数据变更 | Submit、Save                     |
| Abort an unfinished operation | Cancel  | 取消    | 退出且不提交待处理操作             | Close                            |
| Dismiss a completed/read view | Close   | 关闭    | 离开信息界面或已完成界面           | Cancel                           |
| Destroy an owned entity       | Delete  | 删除    | 实体被销毁                         | Remove、Confirm                  |
| Detach without destroying     | Remove  | 移除    | 成员关系或关联终止，但实体仍然存在 | Delete                           |
| Repeat a failed operation     | Retry   | 重试    | 再次尝试同一项失败操作             | Submit                           |

- 破坏性确认使用具体的破坏性动词，例如 `Delete`，而不是 `OK` 或 `Confirm`。
- 成对的状态 Action 使用稳定的反义组合，例如显示/隐藏、展开/折叠、固定/取消固定和添加/移除。
- 按钮标签使用简洁、无句末标点的动作短语。解释、后果和恢复说明属于周围消息。
- 在一次交互的按钮、菜单项、Tooltip、通知和辅助文本中一致复用标签；语义不同时仍保留各自拥有的消息 ID。

## 审查

对于每条受影响的产品消息：

- 对照本参考验证术语和 Action 含义。
- 检查同级控件和同一工作流，找出相互竞争的同义词。
- 位置、对象、语气或后果不明确时，提供译者上下文。
- 同时审查 `en-US` 和 `zh-CN` 的语义等价性，而不是逐词替换。
- 只为反复出现的决策更新本参考；完整的上下文专用句子留在其所属目录中。
