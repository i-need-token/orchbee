# WorkerSendTransaction 有界模型

`formal/tla/WorkerSendTransaction.tla` 检查 `src/terminal_send_operation.rs` 实现、`src/terminal_operations.rs` 派发的 Server 授权发送通道。其并发证据缺口是独立排序的 Host 响应、carrier 丢失及 Server 丢失。单命令授权与生命周期回执使用实现测试；Send 不进入它们的账本，也不在 `admit_launch`/`admit_start` 前获取内核参数。

## 范围与可信假设

- 一条已准入的 `TerminalSendRequest`，走一条共享 Host carrier。请求钉住固定的
  48-byte `HostTerminalIdentity` 与 Server 铸造的 `attachment_id`；模型只保留一个
  `Target`，把身份检查视为精确。准入前 fence（worker 不符、generation 过期、
  epoch 过期、terminal 已处置、超出协商上限）是显式排除项：它是零 Host 字节的
  确定性本地判定，不含交错，因此其证据留在脚本化 fence 测试
  （`send_fence_rejections_never_reach_the_host`）。模型从已准入请求开始。
- 三条 correlation 构造上两两相异：模型假设 fresh-ID 源
  （`worker_identity::fresh_identity`，16 字节 `/dev/urandom`）在一次请求内不碰撞。
  这是此处披露的可信 oracle 假设，不是无碰撞的数学证明；它只作用于三次
  correlation 抽签——attachment id 由 Server 铸造。
- 同一终端上既有的 TUI controller 附件建模为 `hostAttachments` 中的 `tui`；它共享
  carrier，所以 carrier 死亡合法地一并释放它（故障共享是事实，不冒称隔离）。
- attach 已提交后的本地中止（`apply`/`input_command` 中 `?` 上抛）建模为
  `PostAttachAbort`——尽管 Server 侧校验使其在生产中不可达；它是确定性本地路径，
  显式建模而非默默假设消失。
- 字节数抽象为 `{0, 1, 2}`：1 是 text 的精确回显，0 是合法的被忽略 key 计数，
  2 超出协商上限。`payload` 在 Init 选择 `text`（回显必须等于 1）或 `key`
  （接受 0 与 1）。`acceptedBytes` 一旦 `inputAcked` 即为取自
  `ValidAcks(payload)` 的单元素集合——永不为 `{2}`——其后不可变
  （`AcceptedBytesMatchesPayload`、`AcceptedBytesStable`）。
- 阶段计数（`attachSent`/`inputSent`/`detachSent`）与 `deliveries` 是真实发生
  次数（发射处 `@ + 1`），不是"是否发生"位。`TypeOK` 刻意允许 `0..2`，使第二次
  发射在状态空间可表示、由命名性质 `*AtMostOnce`/`ResultProducedOnce` 抓住——
  绝不靠类型域抓，也不允许 guard 消除第二次发送的可能性。

## 源码映射

- Fence 检查 → 按上文范围边界排除；`Rejected` + `NotNeeded` 在模型内仅经
  `AttachRefused` 可达。
- 每步 `exchange_fresh` → `SendAttach`/`SendInput`/`SendDetach` 加应答族：
  `*Exact`、`*Refused`（TypedFailure）、`*Desync`（载荷级回显或越界违例）、
  `*Malformed`（信封契约违例）、`*Lost`（EOF/写失败/步期限）、`*LocalFault`
  （写前失败），以及对端事件 `ServerLost`/`CarrierLost`。`StepAnswer` 映射：
  Record→exact，TypedFailure→refused，Desync 与 ProtocolFault 都关闭 carrier 但记为
  不同动作；detach 处一切非 `Released` 归 `Unreachable`——refused 与 unreachable
  保持不同名，因为 refused 是 typed 而 unreachable 折叠 desync/malformed/lost
  失败。detach 阶段的 LocalFault 是发送前失败（`DetachLocalFault`，`detachSent`
  保持 0），折进同一 unreachable 结算——绝不能写成已发出的 detach。
- Host 应用与 worker 观察到的应答是独立变量：`hostAttachments`/`hostInputApplied`
  经 `HostApply*` 动作推进，可与任意应答丢失交错——已应用但未被应答的输入是可达
  的，这正是 `OutcomeUnknown` 诚实性的来源。
- `host.stream().shutdown(Both)` 仅是本地关闭；`HostTeardown` 是独立的 Host 侧
  动作，在弱公平下最终释放孤儿附件（send 自身的与 TUI 的）。
- `cleanup_closed` 与 `prepare`/`round_trip` 排空的交错 Host 事件由
  `DrainHostEvent` 表示——有界自循环，合法、绝不为错误、也不能结算该步。
- `server_transport::send` → `DeliverResult`（链路存活）或 `AbandonResult`
  （链路已失）——一旦到达 `result-ready`，结果记录无论如何都已产生。

## 不变量与进展

`PairingMatrix`、`AcceptedFactSurvivesCleanup`、`AcceptedBytesMatchesPayload`、
`ConfirmedRequiresDetachAck`、`AttachRefusalNeedsNoDetach`、
`UntrustedStreamNoDetach`、`UnconfirmedCleanupClosesCarrier`、
`TuiLossOnlyViaCarrierDeath`、`SendAttachmentMatchesAdmission`、
`NoResultOnEarlyServerLoss`、`ResultProducedOnce`、每阶段 `*AtMostOnce`。
时态：`EventualSettlement`、`TeardownEventuallyReleasesAttachments`、
`DetachOnlyAfterCommittedInputFact`、`UnknownNeverResends`、
`AcceptedNeverDowngraded`、`AcceptedBytesStable`。进展是对 `Next` 的弱公平——
每个持续使能的解算（应答、过期、丢失或本地结算）都可能发生；不假定对端
永远回成功。

`PairingMatrix` 只在结算后（`result-ready`/`done`）适用：detach 轴未解算时
cleanup 为 `none`。健康 carrier 只在两种结算下存活——`AttachRefused` 与
`DetachExact`；其余终态路径一律退役它，`done` 处 unconfirmed cleanup 蕴含
carrier 已死。

## 界与证据消费者

一条请求、一对终端、一个 TUI 附件、字节域 0..2（`acceptedBytes` 在输入被应答后
是单元素集合，此前为 `{}`）、每阶段发送计数 ≤1、事件排空 ≤3。refinement 消费者
是 scripted-Host 套件（`tests/process/terminal_operations/send.rs`，15 例）与公共
send 测试；本模型不替代它们——wire 字节由它们钉住而模型做抽象。

## 验证

作者定点正向对照（`/home/liushuai/orchbee-batches/heavy.lock` 锁内，
pinned `tla2tools-1.7.4.jar`，1 worker，512 MiB，上限 120 秒）：

```sh
java -Xmx512m -XX:+UseParallelGC -jar tla2tools-1.7.4.jar \
  -fp 0 -lncheck final -coverage 1 -workers 1 \
  -metadir <dir> -config WorkerSendTransaction.cfg WorkerSendTransaction.tla
```

结果：exit 0，2688 generated / 1834 distinct states，深度 15，每个 action 均有
覆盖（无零覆盖行）。日志与直接子进程 exit 码存于批次证据目录
`a3/evidence-14/positive/`（旧 `a3/evidence-13/` 先于计数修正，原样保留）。

权威运行采用既有 runner 资源预算——当前记录为 2048 MiB 堆、单 worker、
120 秒上限（见 `scripts/tla/models.json` 的 `workers` 1、
`timeoutMilliseconds` 120000，以及 `scripts/tla/model-checks.mjs` 的
`maximumHeapMiB` 上限）。

## 负控

mutant 模块按批次在树外生成，每个只改一处 guard/action，各自在同一锁内
单独运行，完整反例与 exit 码均保留。当前批次：`a3/evidence-14/mutants/`，
建于冻结模块 digest。

- `m1-detach-after-desync`——`InputDesync` 仍发出 detach 记录：违反
  `UntrustedStreamNoDetach`（exit 12）。
- `m2-erase-accepted-fact`——`DetachUnreachable` 抹掉已提交的输入事实：违反
  `AcceptedFactSurvivesCleanup`（exit 12）。
- `m3-duplicate-input-guard-only`——纯 guard 放宽 `SendInput`（一条输入在飞
  时重入；基线 `@ + 1` 计数代码不动）：`inputSent = 2` 由 `InputAtMostOnce`
  而非 `TypeOK` 抓住（exit 12）。这才是承重的重放检查——计数位写法会让同一
  guard 变异以 stutter 形式不可见。
- （旧批次 `a3/evidence-13/` 另有 `m4-tui-takeover`，违反
  `TuiLossOnlyViaCarrierDeath`；其反例先于计数修正，原样保留。）

具名可达性见证（对目标状态取反的 probe 不变量，树外运行于
`a3/evidence-14/named-traces/`；每条 TLC 违例即见证轨迹）：

- `ProbeKeyAckZero`——`payload="key" /\ outcome="accepted" /\
acceptedBytes={0}`：语义键合法地以零字节应答。
- `ProbeAppliedResponseLost`——`hostInputApplied /\ outcome="unknown"`：
  Host 已应用输入但应答从未到达 Worker。

## 已知抽象（如实界注，不修正）

- `ServerLostAtResult` 后 `done` 处 `carrierAlive`/`hostAttachments` 冻结为存活
  态；真实 worker 退出会断 carrier 并释放 `tui`。该终态偏乐观但不支撑任何假
  性质。
- `eventDrains <= 3` 是任意取界；真实一秒步内可排空更多交错 Host 事件。此界
  仅限制 stutter 深度。
- `CarrierLost` 在交换间相位把"下一次写才发现死亡"折为无损转移；主导真实迹
  （`prepare` 内零字节发现）已覆盖，而写到半途才失败的子情形被抽象掉。

## Inventory 建议（交 A6）

- 模型名：`worker-send-transaction`
- 模块：`apps/orchbee-worker/formal/tla/WorkerSendTransaction.tla`
- 配置：`apps/orchbee-worker/formal/tla/WorkerSendTransaction.cfg`
- owner 记录：`apps/orchbee-worker/docs/worker-send-transaction.md`（+ zh-CN）
- 源码映射：`src/terminal_send_operation.rs`、`src/terminal_operations.rs` 分发臂、
  `src/host_connection/exchange.rs`（`exchange_fresh`、`PeerFailure`）、Host carrier
  teardown。
- `requiredCoverage`：全部 action——`SendAttach`、`SendInput`、`SendDetach`、
  `HostApplyAttach`、`HostApplyInput`、`HostApplyDetach`、`HostTeardown`、
  `AttachExact`、`AttachRefused`、`AttachDesync`、`AttachMalformed`、
  `AttachLost`、`AttachLocalFault`、`InputExact`、`InputRefused`、
  `InputDesync`、`InputMalformed`、`InputLost`、`InputLocalFault`、
  `DetachExact`、`DetachRefused`、`DetachUnreachable`、`DetachLocalFault`、
  `ServerLostBeforeInputFact`、`ServerLostAfterInputFact`、
  `ServerLostAtResult`、`CarrierLost`、`PostAttachAbort`、`DrainHostEvent`、
  `DeliverResult`、`AbandonResult`。
- 作者对照资源界：1 worker / 512 MiB / 120 s（实测 <1 s，1834
  distinct states）；权威运行采用上述 runner 预算。
- 已知排除：准入前 fence（确定性、零 Host 字节；由脚本化测试
  `send_fence_rejections_never_reach_the_host` 持有）、fresh-ID 无碰撞（已披露的
  可信 oracle）、真实字节数与 wire 编码（由 scripted-Host 套件钉住）、进程级 TUI
  隔离（本模型只覆盖共享 carrier 边界）。

本模型已登记于 `scripts/tla/models.json`，id 为 `worker-send-transaction`
（owner `orchbee-worker`）。

## 语义 stroke 细化

有序 send 实现把每个已校验 stroke 映射成一个语义 Host 按下事件。按键身份、文本、已消费 Shift 和 Super 影响编码，不改变 Attach/Input/Detach 所有权或顺序。Host 在终端锁内按当前模式选择字节；编码为零字节的按键仍合法。这些确定性映射细化模型中的抽象输入动作，以 parser/codec/Worker/Host 边界回归为证据。抽象模型不证明键盘编码或多记录字节语义。
