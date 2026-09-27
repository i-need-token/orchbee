# 终端控制权接管

## 权威与模型

显式请求的接管替换一个既有终端的输入及 resize 控制者。两个只读 Viewer 均保持附着，
不启动、停止或选择终端。普通 Acquire 仍拒绝已占用终端。OBCS `takeover` 仅携带提交
Session 的 `attachmentId`；Server 选择旧控制者并分配新的操作及 controller 身份。
Client 不能指定另一 Session 的控制者，也不能自行提供授权。

Server 拥有最多八项的预留表，以 Worker 身份/代次及 Host epoch/终端为键。
预留包含 Viewer、controller 及 Pending 或 Active 阶段。Worker 拥有对应的 Host attachment，
并串行执行完整控制操作；Host 拥有实际输入能力和 resize lease。按 Registry、Site、
预留表的顺序加锁，在任何传输等待前释放这些锁。

| 状态          | 事件与守卫                                          | 下一状态及效果                                               |
| ------------- | --------------------------------------------------- | ------------------------------------------------------------ |
| Vacant        | Acquire 或 Takeover，目标当前有效且未封锁           | Pending 新授权；提交 Worker Acquire                          |
| Active 旧授权 | 普通 Acquire                                        | 拒绝 controller-busy；保留旧授权                             |
| Active 旧授权 | 显式 Takeover                                       | Pending 新授权；阻止旧 Server 效果；候选者负责新旧清理       |
| Pending       | 另一个 Acquire 或 Takeover                          | 拒绝 controller-busy；不能夺取尚未完成的操作                 |
| Pending       | Worker 证明新 Host 授权，目标仍当前有效且未封锁     | Active 新授权；发布 Acquired                                 |
| Pending       | 拒绝、取消、传输丢失或复核失败                      | 清理精确新旧授权；仅在证明完成、目标销毁或 Worker 退役后移除 |
| Active        | 精确 controller 的 Input、Resize 或 Paste，序号递增 | 在该授权下提交效果                                           |
| Active        | 过期 controller 的效果或清理                        | 拒绝效果；绝不释放替代授权                                   |

## 交接及失败语义

OBCN Takeover 携带预期的旧 controller。Worker 首先验证新 Viewer、精确终端、controller
唯一性及预期旧控制者。允许旧控制者已经释放；若当前存在不同控制者则拒绝。在一个串行
操作内，Worker detach 旧 Host controller，收到精确确认后才 attach 新控制者。
中间可能暂时不可用，但不会同时存在两个有效控制者。已排队的旧输入可能在交接前完成；
交接后才派发的旧 input、paste 或 resize，即使此前已通过 Server 准入，也会被 Worker
拒绝。不重放任何命令。

Server 在预留接管时、提交传输前转移清理责任。因此，接管失败即使没有授予替代权限，
也可能撤销旧控制者；绝不静默恢复旧权限。Worker 确认获取前，候选者负责两个精确清理
请求，包括投递前拒绝或背压。缺失清理证明时保留 Pending 封锁，不能视为终端空闲。
旧授权的迟到清理不能移除新预留或其 Host attachment。已知目标销毁和 Worker 退役可
结算责任。既有类型化 Rejected、Unavailable 及 OutcomeUnknown 结果保持区分。

## 兼容性及证据

此尚未发布的 OBCS v2 新 action 与私有 OBCN action tag 5 要求同一开发版本整体部署。
旧 peer 拒绝此 action；不会回退为断开另一 Client 或重试不确定效果。无需新增 Host 协议
或依赖。原生 stream control 和 attach 在后续切片消费此权威；本契约本身不交付这些
CLI 命令。

协议 corpus 检查覆盖严格 action 解码及关联。预留测试覆盖 Pending 排他、清理责任和
过期移除。Windows 可运行的 Worker 本地管道检查覆盖确认 detach 后才 attach、拒绝/失败及旧 token 的 input/
resize 封锁。同作者定向测试不代表独立审查、形式化验证、崩溃恢复或打包 Windows 交互
验收。最终整体版本验收仍然必需；TLA 执行暂缓。
