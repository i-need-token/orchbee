# 原生控制权接管

## 归属与交互

Windows 内置 `Ctrl+B t` 动作显式申请控制当前已连接的原生 pane。
普通打字、点击、选择和 Viewer 轮询从不申请接管。Server 通过现有
OBCS v1 Takeover 动作继续拥有控制 grant 的权威；Client 不选择或撤销其他
Client 的身份。交接保留终端、程序与 Viewer 连接。

成功申请者提供其 pane 内容几何。原控制者继续接收输出；它下次输入或调整尺寸
被拒绝时，显示模式变为只读。静止 Client 的控制标签可滞后到这次交互。
输入被阻拦时，状态栏保留已知只读的解释和接管按键。
被阻拦的输入丢弃且从不重放。之后的显式动作可使用同一 Viewer 接管回来。
实际仍持有控制权时重复该动作，保留已经核实的当前 grant。已结束的终端不接受
controller：该动作不提交任何请求，状态栏保留已保存的退出事实，并以新建 Tab 代替接管按键。

尺寸回执匹配后，申请者在原始操作截止时间内暂停并重新激活同一 Viewer 的 projection
demand。重新激活会要求完整帧，即使终端空闲、交接前后尺寸相同或 grant 仍属于自身。
UI 在该帧绘制且反馈被接受前显示等待画面；仅有尺寸回执不会报告输入就绪。
任一 demand 步骤失败时保留已知 controller grant，提示画面同步未完成，
不重放操作或输入。

Ready 后，有效完整快照决定显示几何，即使它不同于本 Client 最后的尺寸回执。
在本 Client 收到控制拒绝前，peer 可能已经接管。快照不授予输入或尺寸控制权；
初始设置与交接首帧仍保留已确认几何的门禁。普通 delta 仍必须匹配已接受的
base、generation 与 geometry。因此，原控制者无需本地调整窗口或重新选择就能继续
观察新输出。

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
| Ready     | 尺寸与 projection 重新激活已确认；UI 等待绘制帧反馈。   |
| Refused   | 保留连接和焦点；提示未获得控制权。                      |
| Unknown   | 保留操作身份与任何已知 grant；必要时退役 carrier。      |

已知 grant 不能降为未知，也不能在同一次操作里换成另一个 grant。
投递期间失效时，executor 和 mailbox 都保留证据，直到 UI 消费。
回执结算必须匹配操作与 pane incarnation。已接受的帧和几何 gate 继续决定
何时可向程序输入；接管等待首个匹配的绘制／反馈后才允许输入。接受后，普通帧更新
沿用既有 Ready 输入规则，不重新开启这道交接屏障。待处理提示在注入时钟
达到 200 ms 后使用本地化状态显示，不弹模态确认。

## 证据与边界

串行化的原生交接使用受控实现证据，不保留独立 TLA 模型。可执行证据覆盖实际状态、
带帧 carrier、预留及 Host 操作边界：

| 声明                                                                       | 维护中的证据                                                                                                         |
| -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| 显式准入、捕获的几何、帧先于输入及退出后的单调回执                         | [`state/tests/control_takeover.rs`](../src/workbench_client/state/tests/control_takeover.rs)                         |
| 拒绝、未知结果不重新提交、resize／demand 丢失及 UI 投递丢失后保留授权      | [`connection/native/tests/control_takeover.rs`](../src/workbench_client/connection/native/tests/control_takeover.rs) |
| 绘制／反馈恢复输入且不重放；对端几何不授予权限                             | [`ui/tests/control_takeover.rs`](../src/workbench_client/ui/tests/control_takeover.rs)                               |
| 待处理预留排除竞争者；迟到清理保留替代授权                                 | [Server 预留测试](../../orchbee-server/src/local_terminal_service/controller_connection/tests.rs)                    |
| 确认 detach 后才 attach；失败不恢复旧权限；旧输入、resize 和清理继续受隔离 | [Worker 本地管道测试](../../orchbee-worker/src/host_connection/tests/controller_takeover.rs)                         |

保留的 [`ConnectionViewerOwnership`](../../orchbee-server/formal/tla/ConnectionViewerOwnership.tla)
模型在自身边界内单独检查共享 Viewer／grant 归属和清理，不建模原生交接序列。
上述实现轨迹不证明穷尽交错、公平性或崩溃恢复。Windows 进程证据使用两个尺寸不同的
原生 ConPTY Client，验证程序身份连续且输入可双向交接。这些检查不代表真人
Windows Terminal 或真实 Agent provider 验收，也不建立主动更新控制标签的通知契约。
同作者审读不等同独立确认。
