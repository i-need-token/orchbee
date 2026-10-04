# 原生控制权接管

## 归属与交互

Windows 内置 `Ctrl+B t` 动作显式申请控制当前已连接的原生 pane。
普通打字、点击、选择和 Viewer 轮询从不申请接管。Server 通过现有
OBCS v2 Takeover 动作继续拥有控制 grant 的权威；Client 不选择或撤销其他
Client 的身份。交接保留终端、程序与 Viewer 连接。

成功申请者提供其 pane 内容几何。原控制者继续接收输出；它下次输入或调整尺寸
被拒绝时，显示模式变为只读。静止 Client 的控制标签可滞后到这次交互。
输入被阻拦时，状态栏保留已知只读的解释和接管按键。
被阻拦的输入丢弃且从不重放。之后的显式动作可使用同一 Viewer 接管回来。
实际仍持有控制权时重复该动作，保留已经核实的当前 grant。

## 操作模型

原生会话拥有一个待处理操作：确切 pane incarnation、refresh 代次、请求身份、
捕获的 pane 几何和原始 forward／total deadline。准入使用现有串行 refresh
屏障，排除并发结构操作。待处理期间，输入与切换其他原生上下文受阻。
marker 只有一种类型化用途；接管不伪装为 Workbench 快照修改。

| 状态      | 证据及允许的转换                                        |
| --------- | ------------------------------------------------------- |
| Idle      | 只有在当前已连接 Ready pane 上显式操作才能准入。        |
| Queued    | 尚未提交；排队和执行共用原始 deadline。                 |
| Submitted | 缺失回执即未知；不自动再次接管或重新申请。              |
| Granted   | 在分配、调整尺寸或 UI 结算前保留确切返回的 controller。 |
| Ready     | 匹配的尺寸回执确定新控制者几何。                        |
| Refused   | 保留连接和焦点；提示未获得控制权。                      |
| Unknown   | 保留操作身份与任何已知 grant；必要时退役 carrier。      |

已知 grant 不能降为未知，也不能在同一次操作里换成另一个 grant。
投递期间失效时，executor 和 mailbox 都保留证据，直到 UI 消费。
回执结算必须匹配操作与 pane incarnation。已接受的帧和几何 gate 继续决定
何时可向程序输入；接管等待首个匹配的绘制／反馈后才允许输入。接受后，普通帧更新
沿用既有 Ready 输入规则，不重新开启这道交接屏障。待处理提示在注入时钟
达到 200 ms 后使用本地化状态显示，不弹模态确认。

## 证据与边界

确定性核心和 carrier 检查覆盖准入、过期身份、单调回执保留、原始 deadline
和输入屏障。Server 回归检查使用同一批 Viewer 验证反复交接和过期清理。
有界模型检查仅显式交接、唯一真实权限、旧输入拒绝、几何归属和证据保留。
Windows 进程证据使用两个尺寸不同的原生 ConPTY Client，验证程序身份连续
且输入可双向交接。这些检查不代表真人 Windows Terminal 或真实 Agent provider
验收，也不建立主动更新控制标签的通知契约。

`formal/tla/NativeControlTakeover.tla` 限定两个 Client 和三次动作，区分预留、已确认
detach、attach、grant 回执、resize、UI 投递和绘制反馈。它检查唯一 Host 权限、仅显式
提交、旧输入与帧屏障、控制者几何和退出时回执保留，不宣称公平性／恢复性质。
Server 核实自身 grant 的捷径与真实编码由实现测试桥接，不建模身份变换。
同作者审读不等同独立确认。
