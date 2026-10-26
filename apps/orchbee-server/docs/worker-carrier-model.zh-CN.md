# Worker 载体调度模型

状态：已接受的 Server 侧容量切片。本参考规定 `worker_obcn_carrier` 在串行 OBCN 线上的
提交、排队、派发、排空与失败契约。Worker 侧 viewer/controller 容量与挂起帧字节预算归
`orchbee-worker` 应用所有，不在本文裁定。下述上界均为持续可服务条件下的派发步数，而非
墙上时间承诺。

## 角色与状态

一个 Worker generation 对应一个载体。单个 wire-owner 线程串行全部交换；任一时刻至多一个
在飞请求（single-writer 不变式）。生产者为 Client Connection 处理线程；每个会话至多一个在途
请求，故生产者数受活动会话上界（32）约束。

载体队列是单个 `Mutex`+`Condvar` 单元，持有：

- `high`：有界 FIFO，容量 8。
- `low`：有界 FIFO，容量 8。
- `high_waiters`：阻塞 cleanup 生产者的 FIFO，各自持有已存入的载荷。
- `consecutive_high`：连发计数。
- `open_handles`：存活请求句柄计数，沿用先前 sender 存活性语义。
- `closed`：仅由 owner 拆除设置的墓碑标记。

任一请求在任一时刻恰好存在于一处：其生产者线程（准入前）、`high_waiters`、`high`、
`low`、owner 在飞槽位、或已结算（一次 completion 发送，或对无 completion 的 cleanup
detach 的 fire-forget 丢弃，或对被弃 drain 交换的"消费后丢弃"应答）。lane 之后不存在
隐藏或无界队列。

## 调用方 lane 映射

lane 由生产者语义决定，而非仅由线协议变体决定：

| 提交路径                                                     | 变体                                                                 | lane | 准入                                        |
| ------------------------------------------------------------ | -------------------------------------------------------------------- | ---- | ------------------------------------------- |
| `request()`、`submit_lifecycle()`、`validate_terminal_cwd()` | CwdValidation、Lifecycle、Control（非 cleanup）、Launch、Start、Send | high | try-admit；满或存在 waiter → `Backpressure` |
| `request()`                                                  | Viewer（Attach/Poll/ReadRows/feedback/显式 Detach）                  | low  | try-admit；满 → `Backpressure`              |
| `cleanup_control()`                                          | Control Release                                                      | high | FIFO 等待；优先于 try-admit                 |
| `enqueue_detach()`                                           | Viewer Detach                                                        | high | FIFO 等待；优先于 try-admit                 |
| `admit_agent_drain()`                                        | AgentDrain                                                           | low  | try-admit；满 → `Backpressure`              |

同一个 Viewer Detach 变体，来自会话销毁 cleanup 时走 high，来自普通观测时走 low。
仅既有的两个 cleanup 生产者会阻塞；所有普通生产者 try-admit，满时获得既有的类型化
`Backpressure` 忙结果。`admit_agent_drain` 接缝为生产：Server drain 读取泵经同一
worker/generation 栅栏准入一个类型化 AgentDrain 请求，并返回 `PendingExchange`——其
有界 `recv_deadline` 等待在超时后保留该尝试，使读取泵可以按自己的期限等待，
而非使用 `submit` 的阻塞接收。

## 准入

- try-admit 在队列关闭时失败 `Closed`；在 lane 达容量时失败 `Full`——且仅对 high
  lane，`high_waiters` 非空时同样失败（waiter 优先；low lane 只查自身容量）。
  否则追加并唤醒 owner。载体存活时 `Full` 映射为 `Backpressure`，否则为
  `Unavailable`；`Closed` 映射为 `Unavailable`。
- block-admit（仅 high）在 lane 无法接纳时把载荷存入 `high_waiters`，随后等待该表项被
  移入 `high`（准入成功）或队列关闭。waiter 上界即阻塞 cleanup 生产者数——每个
  Client Connection 至多一个在途请求，故当前会话上界下驻留 waiter 载荷 ≤ 32。

## 派发

wire owner 在同一锁下选取：

```
若 consecutive_high < 4 且 high 非空 → 取 high；consecutive_high += 1
否则若 low 非空                       → 取 low；consecutive_high = 0
否则（low 空、high 非空）             → 取 high；consecutive_high += 1
```

弹出 `high` 立即释放其 lane 槽位（在飞请求单独计数），并把最老的 `high_waiters` 表项
原子移入空出的队尾槽位，唤醒对应生产者。两条 lane 均饱和时，high 连发不超过 4 次后必
服务一个 low；参考模型验证了每请求派发步等待 ≤ 10（high 第 8 位：⌈8/4⌉·5）与 ≤ 40
（low 第 8 位：8·5），waiter 进度受生产者数约束。无可派发工作时 owner 在条件变量上至
多等待 25 毫秒空闲轮询周期，然后照旧探测流。

## 排空与墓碑

owner 退出或析构时先将载体标记为不活跃，按 complete-write 标记结算在飞请求（写入完成
前 `Unavailable`，完成后 `PostDeliveryOutcomeUnknown(CarrierStopped)`），随后关闭队列，
把每个已排队与等待中的请求以 `Unavailable` 恰好结算一次；在飞请求不会被重复结算。
`open_handles` 归零使 `take_next` 返回 `Closed`——但在此之前 `dispatch` 先把队列中
残余载荷全部送上线，因此无 completion 的 cleanup detach 在最终拆除时仍会送达 worker——
随后 owner 以 `Shutdown` 退出并执行同样的排空，即先前全体 sender 销毁的语义。
completion 所有权为 `Required`、`DiscardableDrain`、`Detached` 三者之一，在准入构造时
按请求种类固定：`Detached` cleanup 请求以丢弃结算；在飞 `Required` 请求的应答接收方
丢失仍保守地使载体退役；被弃的 `DiscardableDrain` 交换则消费并丢弃合法应答，使健康的
载体流量存活——畸形帧、错误种类或方向、echo 不匹配与传输失败仍如从前一样致命。
完整写入边界两侧的 `NotDelivered` 与 `PostDeliveryOutcomeUnknown` 区分不变。

## drain 目标

`WorkerRegistry::agent_drain_carrier()` 回答 `request_handle()` 无法回答的
drain 域载体问题：仅当 `active == None` 时为 `NoCarrier`；待准入、存在但已不活跃的
载体记录、或身份无法在 drain 线上命名（CNP 拒绝全零 `WorkerId`）的载体均为
`CarrierUnavailable`；`Live` 携带克隆的请求句柄、完整 CNP `WorkerId` +
`WorkerGeneration` 身份对、以及共享的 per-carrier `AgentDrainMint`。

mint 在载体注册成功时创建、随记录消亡。`mint()` 经检查型 CAS 发放下一个 drain
token——到达 `u64::MAX` 时返回 `None` 且 `last_issued` 不变：耗尽是一个值应答，
绝不使载体退役、绝不复用 token。`floor()` 只窥探下一个将发 token 而不发放。克隆的
target 可在退役后继续存活，但只伴随死身份——mint 不是存活证明，句柄的准入栅栏才
是。生产消费者是 `LocalTerminalService` 的 prepared-read 提供者（`agent_drain_read`）。

## native 叶

`WorkerRegistry::register_carrier_with_agents()` 走与 `register_carrier()`
相同的注册路径——相同检查、相同发布——并把 canonical `AgentService` 的 native 叶附
在结果上。在持有 registry 守卫期间，从已准入的 worker 与 registry 铸造的 generation
导出精确的 CNP `WorkerId` + `WorkerGeneration` 身份对（与 `agent_drain_carrier()`
重新导出的同一对），然后在同一守卫内运行
`AgentService::native_admit_carrier()`。契约拒绝或已毒化的 cell 均以失败关闭注册：
载体记录绝不安装，因此可读的 Live 载体必然意味着其叶已准入。

注册以 `{identity, Weak<AgentService>}` 携带该叶。退役时 wire owner 仍先收尾；随后
注册守卫在 registry 守卫内退役该叶——先于 `retire_carrier` 释放槽位——因此替代者
绝不可能在叶退役与槽位释放之间被暴露。服务消亡（弱引用升级失败）或已毒化 cell
（类型化 `Err`）都不能阻塞 registry 退役；陈旧注册持有的旧身份也无法退役替代者的
叶：叶的全等身份检查与 registry 的 generation 检查双重拒绝。没有任何互斥锁跨越
socket I/O 持有；非托管 `register_carrier()` 行为保持不变——它是同一个
`register_carrier_impl`，仅不带 service 实参。托管 listener 路径在生产中接线
该缝：共享 `AgentService` 存在时 `local_worker_service` 调用
`register_carrier_with_agents`，否则调用普通 `register_carrier`；生命周期权威
留在 session 组合根。

## 锁纪律

生产者在阻塞准入或等待结果期间不持有任何 registry、coordinator 或 service 锁；wire
owner 不依赖生产者进展。队列锁的持有时间上界为有界双端队列操作；排空路径上的
completion 发送在锁外进行。

## 所有权核算

驻留载荷：`high` ≤ 8、`low` ≤ 8、`high_waiters` ≤ 32（生产者界）、在飞 1——均为
`PendingRequest`（已编码帧字节 + 类型化请求 + 一个在 `Required`/`DiscardableDrain`/
`Detached` 所有权策略下的 completion 端点）。每请求的结果接收方驻留在其生产者线程上。
本文不声明整进程内存上界；此处枚举的是队列持有与生产者持有的副本。

## Windows 交接准入封闭

`WorkerHandoffRequest` 使用高优先级队列及精确 Worker／代际／目标回显。Prepare 在队列锁下
将 Normal 变成一个封闭票据。所有排队的高／低优先级请求和阻塞清理载荷都在锁外结算为
尚未写入的 Unavailable。已派发交换先完成，随后 Prepare 才到达 wire。普通准入及并发
Shutdown 不能进入此封闭。最多一个匹配交接交换处于排队或派发中。此边界关闭传输准入，
不能代替上层 coordinator 的规范状态写入者静止。

Prepared 保持封闭。Abort 成功、确定 NotPrepared、Prepare 失败且没有其他 Prepared
所有者，或 Commit 的 TargetUnavailable 恢复 Normal。Busy 和 TicketMismatch 不能证明
不存在，继续封闭。完整写入后的传输丢失保持 Unknown，不恢复准入或授权重放。Committed
关闭旧队列、退休活动 carrier 并在再次写入前结算残留载荷。导入方仅在备用 wire 收到精确
Owned 后准入新 carrier；Worker 代际保持原值。

[Worker 模型](../../orchbee-worker/docs/windows-server-handoff.zh-CN.md) 负责备用准入、
15 秒租约、物理资源及过期。确定过期通过匹配 Abort／Commit -> NotPrepared 对账；队列
不能续租或推断 Worker 租约。原生命名管道及确定性调度见证覆盖在飞顺序、阻塞清理释放、
精确回显、回滚准入及提交后退休。规范导入与公开在线交接结果仍需队列边界之外的 Server 集成。

<!-- documentation-pairing: english-sha256=a8e946c611a7126fd0b3cd83379f2fd3c982846587040bee5bfeb4d7c8d955c6 zh-cn-sha256=7693656503c064a43c79b3c1ef7e63c36f257c4781b0790e4f118183649acaa6 -->
