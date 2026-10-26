# Windows Server 交接

## 范围与所有权

托管 Windows Worker 可以替换已准入的 Server 连接，同时保留 Worker、授权内核、Agent Drain
水位、后台任务所有者及 Terminal Host 连接。这是当前认证 Server 发起的显式两阶段操作。
普通 EOF 仍会退休 Worker，不会启动恢复或寻找另一个 Server。
Server 负责规范状态导出／导入、目标可执行文件与 cohort 兼容性、公开端点发布、客户端断开及
最终在线交接结果。仅实现此传输切片，不等于公开 `server.live_handoff` 已实现，也不证明进程交接。

在线交接断开客户端、保留终端进程，提交前失败恢复原所有者。Windows 使用已有的常驻 Worker／Host，而非 Unix 描述符
传递。冷检查点恢复和自动重连都不代表此操作。

## 输入与状态

`WorkerHandoffRequest` 回显操作 ID、精确 Worker ID／代际、动作及 `WorkerHandoffTarget`
（非零 128 位交接 ID 与已验证目标 cohort 的 32 字节摘要）。仅活动 Server 可以委派该目标。
编码有效的摘要不构成兼容性证据。runtime 所有者按已有准入 runtime 的哈希推导
`handoff-<handoff-id>` 命名管道；请求不能指定任意路径或文件系统叶子。同用户准入及精确票据／Worker／代际／cohort
回显认证备用连接。备用 Server 必须验证导入所有权授权后才发送 Ready。普通 Worker 准入及
cohort 检查保持原有规则。

Worker 最多持有一个 Prepared 连接、它的 15 秒租约，以及进程生命周期内最多 64 个已消费
交接 ID。每次拨号尝试消费 ID；取消、失败、过期和已提交 ID 均不复用。Prepare 的拨号及
握手共用一个 2 秒截止点；部分输入或输出均不能续期。过期仅关闭备用连接。
在空闲租约截止前开始的记录仍受正常 2 秒记录预算约束。

Server wire 所有者在已有高优先级队列串行执行请求。Prepare 封闭普通准入，并向已排队、
尚未写入的请求返回确定的不可用；已派发请求先完成，随后才处理 Prepare。上层 Server
所有者必须让规范状态写入者静止并等待结算后导出；队列封闭不能代替此义务。
封闭队列仅接纳匹配交接请求。确定 Prepare 失败且没有其他 Prepared 所有者、Abort 成功或
NotPrepared 后恢复准入；Busy 与 TicketMismatch 不能证明不存在，继续保留封闭。
已提交结果永久关闭旧队列。传输不确定沿用完整写入边界，不授权重放。

## 转移

| 状态     | 事件及守卫                                                    | 输出及下一状态                                                                                       |
| -------- | ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Active   | Prepare；托管、精确 Worker、无查看／控制附件、新 ID、容量可用 | 拨号推导端点；截止前精确 Hello／Ready -> Prepared                                                    |
| Active   | 身份错误、非托管、附件忙、ID 已消费、容量耗尽或拨号／握手失败 | 类型化拒绝；保留活动 Server 与全部物理所有者                                                         |
| Prepared | Prepare 或不同票据                                            | Busy 或 TicketMismatch；保留原 Prepared 所有者                                                       |
| Prepared | 匹配 Abort                                                    | 关闭备用连接；Aborted -> Active                                                                      |
| Prepared | 租约到期                                                      | 关闭备用连接 -> Active；后续 Commit 为 NotPrepared                                                   |
| Prepared | 到期前匹配 Commit                                             | 在旧 wire 完整写入 Committed 结果；交换唯一 Server 流；在新 wire 发送 Owned -> 新 Server 上的 Active |
| Prepared | 非交接请求                                                    | 在派发任何效果前拒绝协议继续                                                                         |
| Active   | 普通 EOF、部分／非法记录或 Host 丢失                          | 沿用原退休行为；不能从连接丢失推断交接                                                               |

完整写入 Committed 前，失败会释放备用连接而不切换授权。
该写入必须在 Prepared 租约及普通记录截止前完成；收到 Commit 和写入片段都不能续租。
写入之后，新 wire 是唯一连接；
Owned 发送失败属于结果不确定，不能连接回旧 Server。导入方仅在收到精确 Owned 后认领。
旧 Server 的 Committed 回执是传输提交边界，不是最终公开成功。
成功交换保留精确 Worker 代际、Host epoch、creation ID、terminal ID、内核 fence、计数器、
标题、Agent Drain 及任务句柄。交接不发送 Host 启停或终端 Launch／Start／Resize／Input／Dispose。

## 证据与边界

封闭定长编码以独立黄金字节、全部动作／结果、严格前缀、多余字节、零身份／代际，以及
动作／结果矛盾拒绝验证。实际 Windows 命名管道轨迹验证认证不匹配、Prepare／Abort／过期
回滚、旧／新 wire 选择、精确 Owned 回执与物理所有权保留。Carrier 测试验证封闭、队列请求
结算、关联、普通准入恢复与提交后退休。截止测试区分空闲过期和不完整记录。

这些检查证明此传输边界。无损规范导入、精确 cohort／可执行文件准入、保留进程句柄、公开
端点／激活锁转移、运行 ConPTY 保留、最终 CLI 参数与崩溃恢复仍需 Server 集成及组合证据。
定向检查不代表独立审查、全量回归、真实发布升级或真实终端视觉验收。此边界不启动服务或
后台线程；已有 Worker 循环负责备用清理，已有 carrier 负责请求结算。
准入沿用原有同用户原生开发信任边界；恶意同用户进程及进程崩溃不在此传输模型范围内。
