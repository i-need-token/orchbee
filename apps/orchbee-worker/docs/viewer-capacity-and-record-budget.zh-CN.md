# 查看附件容量与保留记录预算

`src/host_connection/viewers.rs` 拥有 Worker 侧 Server 签发终端查看附件的边界：最多
`MAXIMUM_VIEWERS`（24）个已准入附件，每个查看附件最多一条投影记录、四条 FIFO 事件和一条终端结束记录，以及覆盖全部
保留记录载荷的 `MAXIMUM_RETAINED_VIEWER_RECORD_BYTES`（128 MiB）聚合预算。
`src/host_connection/controllers.rs` 保持独立的 `MAXIMUM_CONTROLLERS`（8）授权边界，
`src/host_connection/target_cleanup.rs` 用同一个 `MAXIMUM_VIEWERS` 常量确定每个
retired-target 查看附件 tombstone 数组大小，使 prepared cleanup 仍能捕获其终端上的全部
查看附件。这些是实现边界，属于已授权 Worker 容量切片，不是发布性能预算，也不是进程
RSS 上限。

## 预算域

- 计入：每个查看附件 `record` 槽、事件 FIFO 和 lifecycle 槽中保留的 `Vec<u8>` 的逻辑 `len()`——包括
  `TerminalFrame` 与 `ProjectionBlocked` 两种记录。超出 `len` 的 `Vec` capacity 不计入。
- 不计入：`flight` 路由元数据（不持载荷）、做出存储决定前的临时传入 `record: Vec<u8>`
  （仍由 wire `MAXIMUM_VIEWER_RECORD_BYTES` 限制管辖，返回后即释放）、已被 `poll`
  移交给 session 的记录、row-window 读取，以及任何 session 侧或协议 codec 副本。
- `retained_record_bytes` 是 `HostViewers` 的私有计数器；只有 owner 代码修改它，且始终
  与记录、事件或 lifecycle 槽转换配对。上限保存在默认等于常量的私有 owner 字段中，测试只能通过
  真实检查路径使用更小预算——绝不直接篡改计数器。

## 转换与恰一次记帐

- 获取：`receive` 中两个经校验的存储点（TerminalFrame、ProjectionBlocked）仅当
  `retained_record_bytes.checked_add(record.len())` 不越上限时接纳记录；先把受检
  总和写入计数器，再赋值 `viewer.record`。
- 释放：每条已存记录恰一次——`poll` 取走、`Viewer::close` 丢弃 record（队列饱和或聚合
  压力），或 `remove`（显式 detach、attach 失败回滚、retired-target commit）。递减发生
  在取走或丢弃该槽的同一路径上，因此任何记录不会被重复计入或重复释放。
- 已关闭查看附件在 `remove` 前仍占准入槽；其后续 Host event 仍经校验后被丢弃，不再
  保留。

## 压力退役

- `receive` 内的次序：畸形 envelope、错误 kind/encoding、route/epoch 不一致或未知附件
  永远先使 Host 连接失败。已关闭查看附件的记录随后被丢弃，已占用交付槽使其查看附件
  退役——两者皆为先于任何流校验运行的既有语义。只有空槽的活动查看附件才执行投影流
  校验（sequence、snapshot 要求、blocked-revision 递增）；聚合预算检查在其后进行，因此
  压力绝不会把违反流的记录变成静默退役——它仍会以 `InvalidData` 使 Host 连接失败。
  Worker 校验 envelope 与路由前缀并不透明地保留记录；完整 OBTF 结构属于
  Client/协议解码器的域，不是本 owner 的。
- 会把聚合量推过上限的记录只通过既有 `Viewer::close` 退役其寻址查看附件：其 record 与
  flight 被丢弃、`closed` 置位，并沿普通 `closed_ids` → `cleanup_closed` 路径在下一个
  普通 Server operation 之前从 Host detach。此后 poll 与 feedback 返回既有
  `AttachmentUnavailable`；Client 可以用 Server 新签发的身份重新附着。不驱逐任何其他
  查看附件，Host 与 retained 终端均不受伤害。
- 相等即保留：`retained + incoming == ceiling` 时存储；仅 `>` 才退役。单条记录永远不
  会自我退役，因为 wire 记录上限远低于预算。

## 终端结束状态交付

kind 15 和 19 是经校验的 Host 终端事实，仅分发给该终端的 viewer。每个 viewer 保留一个
可替换的 lifecycle 槽，计入聚合预算，并在 frame／event 之前轮询交付，不触碰 frame flight。
替换先释放旧记录费用再接纳新记录；poll、close 和 remove 恰一次释放。预算压力只退役受
影响的 viewer。四事件 FIFO 保持独立，因此事件突发不能把结束状态埋在普通事件之后，也
不会增加无界队列。

Host attach 和 projection-demand 准入之后，Worker 在已有两秒观察预算内读取有界保留
inventory，最多八页、每页 128 项。已结束 summary 使用 Host 的身份、退出结果和最终修订
初始化同一 lifecycle 槽。读取校验页代际、cursor 进展、终端／creation 身份唯一性及 Host
epoch，不改变终端状态。因此，即使原退出通知先于附着，也能提供重新进入的状态证据。
目标缺失或无效使 exchange 失败，绝不显示为运行中的 Shell。

## Retired-target 传播

`RemovedTarget.viewers` 是 `[Option<OperationId>; MAXIMUM_VIEWERS]`，
`HostViewers::target_ids` 返回同一数组类型。准入提升到 24 会让 tombstone 同步增长，
因此 prepared cleanup 能无截断地捕获一个终端上可能绑定的全部 24 个查看附件。
owner 测试直接断言该存储的界限：
`size_of::<RemovedTarget>() * MAXIMUM_RETAINED_TERMINAL_BINDINGS <= 1 MiB`——
独立有界的 owner 分配，不是进程 RSS 声明。

## 证据

确定性 owner 测试——该机制在单一 broker 内是顺序的，因此证据缺口是记帐正确性而非
交错：

- 准入：24 个查看附件被接纳，第 25 个返回类型化 `CapacityExceeded`；controller grant
  边界保持 8。
- 记帐：两种记录按逻辑长度计费；相等保留；`poll`、`close`、`remove` 与 retired-target
  commit 各自恰一次释放。
- 压力：只有被寻址的查看附件退役，同级记录不受影响，已关闭查看附件丢弃后续 event，
  poll/feedback 返回 `AttachmentUnavailable`。
- 安全：畸形 envelope 永远返回 `InvalidData`；空槽活动查看附件的违反流记录在压力下
  仍失败，而非静默退役。
- Tombstone：24 查看附件终端的 prepared cleanup 捕获全部 24 个 id。
- 边界：一个测试以经合法 optional extension 记录扩充、且 canonical
  `decode_terminal_text_frame` 解码器接受的帧触及真实 128 MiB 上限；更小预算只能
  经由 owner 上限字段到达同一检查。

session 持有的已交付记录与临时传入分配按构造不在此预算内，此处不声称其有界。

<!-- documentation-pairing: english-sha256=db3051314047ab2e7309721772b8fd08ae787b9f6450da3d1a3cecfce552b30b zh-cn-sha256=1e38a878e77a26705581aae7e9b503a00b31f09b3c9593f96a209f27799fd0ca -->
