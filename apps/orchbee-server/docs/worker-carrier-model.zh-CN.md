# Worker 载体调度模型

状态：已接受的 Server 侧容量切片。本参考规定 `worker_obcn_carrier` 在串行 OBCN 线上的
提交、排队、派发、排空与失败契约。Worker 侧 viewer/controller 容量与挂起帧字节预算归
`orchbee-worker` 应用所有，不在本文裁定。下述上界均为持续可服务条件下的派发步数，而非
墙上时间承诺。

## 角色与状态

一个 Worker generation 对应一个载体。单个 wire-owner 线程串行全部交换；任一时刻至多一个
在飞请求（single-writer 不变式）。生产者为 Client Session 处理线程；每个会话至多一个在途
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
detach 的 fire-forget 丢弃）。lane 之后不存在隐藏或无界队列。

## 调用方 lane 映射

lane 由生产者语义决定，而非仅由线协议变体决定：

| 提交路径                                                     | 变体                                                                 | lane | 准入                                        |
| ------------------------------------------------------------ | -------------------------------------------------------------------- | ---- | ------------------------------------------- |
| `request()`、`submit_lifecycle()`、`validate_terminal_cwd()` | CwdValidation、Lifecycle、Control（非 cleanup）、Launch、Start、Send | high | try-admit；满或存在 waiter → `Backpressure` |
| `request()`                                                  | Viewer（Attach/Poll/ReadRows/feedback/显式 Detach）                  | low  | try-admit；满 → `Backpressure`              |
| `cleanup_control()`                                          | Control Release                                                      | high | FIFO 等待；优先于 try-admit                 |
| `enqueue_detach()`                                           | Viewer Detach                                                        | high | FIFO 等待；优先于 try-admit                 |

同一个 Viewer Detach 变体，来自会话销毁 cleanup 时走 high，来自普通观测时走 low。
仅既有的两个 cleanup 生产者会阻塞；所有普通生产者 try-admit，满时获得既有的类型化
`Backpressure` 忙结果。

## 准入

- try-admit 在队列关闭时失败 `Closed`；在 lane 达容量时失败 `Full`——且仅对 high
  lane，`high_waiters` 非空时同样失败（waiter 优先；low lane 只查自身容量）。
  否则追加并唤醒 owner。载体存活时 `Full` 映射为 `Backpressure`，否则为
  `Unavailable`；`Closed` 映射为 `Unavailable`。
- block-admit（仅 high）在 lane 无法接纳时把载荷存入 `high_waiters`，随后等待该表项被
  移入 `high`（准入成功）或队列关闭。waiter 上界即阻塞 cleanup 生产者数——每个
  Client Session 至多一个在途请求，故当前会话上界下驻留 waiter 载荷 ≤ 32。

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
无 completion（`None`）的 cleanup 请求以丢弃结算；在飞请求的应答接收方丢失仍
保守地使载体退役。完整写入边界两侧的 `NotDelivered` 与
`PostDeliveryOutcomeUnknown` 区分不变。

## 锁纪律

生产者在阻塞准入或等待结果期间不持有任何 registry、coordinator 或 service 锁；wire
owner 不依赖生产者进展。队列锁的持有时间上界为有界双端队列操作；排空路径上的
completion 发送在锁外进行。

## 所有权核算

驻留载荷：`high` ≤ 8、`low` ≤ 8、`high_waiters` ≤ 32（生产者界）、在飞 1——均为
`PendingRequest`（已编码帧字节 + 类型化请求 + 一个 `sync_channel(1)` completion 对）。
每请求的结果接收方驻留在其生产者线程上。本文不声明整进程内存上界；此处枚举的是队列
持有与生产者持有的副本。
