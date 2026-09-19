# `NativeCliWaitSettlement`——`wait-output` 结算模型

`formal/tla/NativeCliWaitSettlement.tla` 是对一次 `terminal wait-output` 调用的客户端结算的
有界 TLA+ 模型。它存在的理由：该命令把取消、异步 deadline 与独立排序的传输结果组合在一起——
测试套件只能采样代表性交错，而模型在界内穷举全部交错。建模对象是模型头中记录的固定基
commit 上的 `src/cli/query.rs`。

## 范围

模型覆盖单终端上的单次调用：

- preamble 门（`session_preamble` 在 socket cohort 准入前先结算该门，因此已采样 flag 可以
  在零 attach 下结束命令）；
- `terminal.viewer` attach 交换：`Ok` 应答建立具名 viewer，此后一切出口走共享 cleanup 路径；
  `Err` 应答可在无 cleanup 义务下结束调用；
- observe 循环在 `Poll`、`FrameFeedback`、`ReadRows` 之前的逐步 flag 门；
- 有界 `read-rows` 序号：序号在步 deadline 检查之前被消耗，stale 应答强制以新序号重试，
  耗尽是真实的 internal 失败——绝不重置或回绕；
- detach 决策（`step_deadline` 先采样 cancel 再采样 expiry；已采样 flag 可跳过提交，同时
  记录真实 cleanup 来源）；
- detach 之后的无条件迟到采样：它可改变 primary 分类，但绝不改写已发射的 cleanup 事实。

环境 flag（`cancelled`、`expired`）可落在任意两步之间，也可在发射之后到达——模型记录发射
时刻证据（`emitAction`、`cancelledAtDone`、`expiredAtDone`、`matchedOnce`），正是为了让迟到
且不可观察的环境步无法伪造一份已完成的应答。

## 源码映射

| 模型元素 | `src/cli/query.rs` 中的运行锚点 |
|---|---|
| `PreambleGate` / `PreambleFail` | `session_preamble` 在 `run.check()` 结算该门后返回；已采样 flag 以 `cleanup = "absent"` 且无 attach 结束调用 |
| `IssueAttach` / `AnswerAttachOk` / `AnswerAttachErr` | `run.check()` + `step_deadline` 之后的 `attach_viewer`；`Ok` 产生具名 viewer，`Err` 进入 `PreViewerSettle` |
| `ObserveGate` / `IssuePoll` | `wait_observe` 中每次 poll 之前的循环顶 `run.check()` |
| `IssueFeedback` / `IssueReadRows` | 每次交换前的 `step_deadline`；`wait_read_window` 先消耗 `next_sequence` |
| `Settle` | 交换后的 `run.check()`，让已采样 flag 胜过交换自身的分类 |
| `AttemptDetach` | 提交前的 `step_deadline`：`cancel` 以 `unconfirmed` 跳过，`expiry` 以 `requested` 跳过 |
| `LateSample` | detach 之后决定最终 primary 的无条件 `run.check()`，同时保留 cleanup 事实 |
| `matchedOnce` | 仅当一致窗口通过门且匹配器接受 match 时置位——绝不在 raw 应答时提前置位 |

## 性质

安全不变量：`TypeOK`、`OutputWrittenOnce`、`NoFabricatedTerminalAnswers`、
`LateSampleDecides`、`CleanupHonest`、`ObserveOutcomeSurvives`、
`ResultRequiresConfirmedDetach`、`MatchFactPersists`、`SeqStrictIncrease` 与
`ObservedConsistentOnly`。时态性质：在命令自身动作的弱公平下 `Termination` 与
`MatchedSettles`；signal flag 本身不要求公平。界限：`MaxSeq = 2`，观察计数饱和，
`CHECK_DEADLOCK FALSE`。

## 证据

权威运行使用固定的 `tla2tools` jar、单 worker、512 MiB 堆与共享 `heavy.lock`，每次运行的
日志与退出码各自保存在独立的批次证据文件中。正向运行通过全部不变量与两条时态性质，且每个
动作覆盖计数非零。两个批外变异必须持续产生真反例：detach 错误时跳过迟到采样违反
`LateSampleDecides`；让迟到 flag 抹除已 confirmed 的 cleanup 违反 `CleanupHonest`。可达性探针
（`ProbeNo*`）见证快乐路径、无 flag 的 typed detach、cleanup 为 `requested` 或 `confirmed`
的取消、序号耗尽、第二个序号、typed 结果在 detach 错误后存活、以及 absent cleanup 的
pre-viewer 取消。

## 边界

模型不覆盖 Server 或 Worker 内部、wire codec、匹配算法、墙钟时序、多次调用或任何 TUI 表面；
它不对 HERDR 对齐作任何声明。它只建模结算契约——发射哪种分类、报告什么 cleanup 状态、以及
哪些已采样事实被允许决定它们。
