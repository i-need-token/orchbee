# Agent Observation Ingress

## 权威与范围

Desktop Main 拥有易失 join，负责把一条经过清理的 rich Agent hook event 与用于识别其 live
presentation 的有界 terminal correlation hint 关联起来。私有 Unix socket half 拥有 semantic
authority，但没有 presentation authority；OSC 777 version-2 hint 只对一个 live terminal generation
拥有 presentation authority，但没有 semantic authority。任何一半都不能单独修改 canonical Agent
state。

本 contract 覆盖精确配对、两种到达顺序、有界 settlement、重复处理、presentation generation 替换、
独立 fan-out、过期与 dispose。它不改变 wire contract、provider event 含义、Agent reducer、Workbench
projection、SSH forwarding、notification policy、持久化或用户可见 copy。

## 参考范围

2026-08-30 检查了 Herdr source revision `c2637dc1`，将其作为 design evidence。Orchbee 只采用两个
有限原则：evidence 可以早于 identity acquisition 到达；只有 evidence 的 owner 确实被替换时才清除
retained evidence。Orchbee 也遵循把 passive evidence capture 与 deterministic transition decision
分离的做法。

这不是 compatibility 或 replication claim。Herdr 的 screen/process detection、flat status、terminal
UI、provider lifecycle 与 timing constant 均不在范围内。当实现不同时，以 Orchbee 经验证的 rich hook
fact、精确 two-half correlation、presentation generation 与 canonical Agent runtime 为准。

## 输入与信任边界

| 输入                      | 准入与缺失语义                                                                                                                                                                             |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Rich hook event           | Socket boundary 已执行 body limit 与 strict schema，closed registry 已授予 provider provenance 与 accepted-event capability。没有匹配 hint 时，它保持 pending，且不产生 canonical effect。 |
| Terminal correlation hint | Terminal signal 已通过 strict schema。只有准确的 current presentation generation 才能准入。没有匹配 rich event 时，它保持 pending，且不产生 canonical effect。                             |
| Presentation registration | 建立后续 hint 能为该 presentation 指定的唯一 generation。替换时先 retire previous generation。                                                                                             |
| Presentation retirement   | 只移除指定 presentation generation 的 pending 与 canonical navigation membership。                                                                                                         |
| Settlement deadline       | 指定安排它的准确 pending-event incarnation。Stale deadline、expired event 或没有 live match 的 event 均为 no-op。                                                                          |
| Expiry observation        | Logical time 由 ingress 提供。接受任一 half 之前以及 settlement 之前都会判断 expiry。                                                                                                      |
| Disposal                  | 取消全部 deadline，并清除 noncanonical join state，不发布 Agent 或 terminal state。                                                                                                        |

Provider payload、prompt、tool content、transcript path、path、error text 与 terminal screen text
永远不会进入该状态机。

## 数据、所有权与输出

`AgentObservationJoinStateMachine` 是 current presentation generation、pending event 与 hint、
correlation-to-event claim、matched identity 与 attachment、历史 presentation-generation match，以及
有界 recent-event replay fence 的唯一 writer。

状态机不依赖 socket、clock、timer、Agent runtime 或 renderer。它接收 validated input 与 logical
time，并返回 command。`AgentHookObservationIngress` 拥有执行 command 的外部机制：

| Command              | Effect owner 与结果                                                                   |
| -------------------- | ------------------------------------------------------------------------------------- |
| Schedule settlement  | Ingress 创建一个以 event identity 与 expiry incarnation 为 key 的 timer。             |
| Cancel settlement    | Ingress 只清除具有准确 identity 与 incarnation 的 timer。                             |
| Apply observation    | Ingress 把 rich event 与所有 current matched attachment 一次性 dispatch。             |
| Attach presentations | Ingress 把 late match 添加到已经 applied 的 live Session，但不重放其 semantic event。 |
| Retire generation    | Ingress 从每个 canonical Agent Session detach 该准确 generation。                     |

Command execution 不会把隐藏的 success 反馈给 join。因此 Agent runtime rejection 仍表示 observation
已消费，从而保持现有 at-most-once contract。

## 不变量

1. **精确 two-half authority**：apply 要求至少一个已声明 correlation 在 current registered
   presentation generation 上与准确 event 匹配。
2. **顺序无关**：当相同 valid half 在 expiry 前到达时，hint-first 与 rich-first delivery 产生相同
   attachment set。
3. **At-most-once semantics**：一条 retained transport event 最多发出一次 apply command；之后的
   correlation 只能发出 attachment command。
4. **Generation 不重定向**：event 匹配过一个 presentation generation 后，其任何 correlation 都不能
   把同一 presentation identity 绑定到另一个 generation。
5. **独立 fan-out**：retire 一个 presentation generation 不能 retire 或 downgrade 另一个 matched
   presentation。
6. **Expiry 与 deadline fencing**：expired half、expired event、stale timer incarnation 或 disposed
   state 都不能发出 apply 或 attachment command。
7. **Fail-closed collision**：冲突 duplicate 与被另一个 event claim 的 correlation 不改变 admitted
   state。
8. **有界 state**：overflow 拒绝新的 untrusted input，且不 evict 已准入 state。

## Transition 模型

| State/event            | Guard                                                    | State change                                                                   | Command                                             |
| ---------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------- |
| Register generation    | 没有 current generation                                  | 记录 generation                                                                | 无                                                  |
| Replace generation     | current generation 不同                                  | Retire 旧 hint 与 live attachment，保留 event history fence，记录新 generation | Retire 旧 generation                                |
| Receive first half     | Valid 且 capacity 内                                     | Retain 到 expiry                                                               | 无                                                  |
| Join first correlation | 准确 event 与已声明 correlation 匹配 current generation  | 消费 hint，retain generation history，merge visibility                         | 除非全部 correlation 匹配，否则 schedule settlement |
| Join final correlation | 全部已声明 correlation 均已匹配                          | 标记 applied 并清除 settlement                                                 | 存在 settlement 时先 cancel，再 apply 一次          |
| Settlement fires       | 准确 incarnation、unexpired event 且存在 live attachment | 标记 applied 并清除 settlement                                                 | Apply 一次                                          |
| Late correlation       | 准确、未过期、未匹配且 generation compatible             | 消费 hint 并 merge attachment                                                  | 只 attach                                           |
| Retarget attempt       | 同一 event 此前在另一 generation 匹配过该 presentation   | 保留 history fence                                                             | 无                                                  |
| Event expiry           | 达到 expiry                                              | 移除 claim 与 attachment；把 applied identity 移入 recent fence                | 存在 settlement 时 cancel                           |
| Hint expiry            | 达到 expiry                                              | 移除 unmatched hint                                                            | 无                                                  |
| Dispose                | 任意 state                                               | 清除全部 noncanonical fact                                                     | Ingress cancel 全部 timer                           |

Visibility merge 是保守的：`hidden` 优先；两个 `visible` hint 仍为 `visible`；其他组合均为
`unknown`。因此，同一 presentation generation 的多个 correlation 只产生一个 attachment。

## 边界与时间

Production 最多准入 512 个 pending rich event 与 1,024 个 pending hint。每个 half 的 join lifetime
为 10 秒。第一次 valid partial match 会在 25 毫秒后 schedule settlement。Applied event 保留在 pending
entry 中直到 join expiry，随后再进入 60 秒 replay fence。Cleanup 是 demand-driven，但每次 deadline
都会先执行 cleanup，因此 event-loop delay 不能发布 expired event。

这些边界不声明 Desktop restart 后的 delivery、所有 SSH attachment 都 disconnected 时的 delivery、
无界 replay protection、provider termination 或 wall-clock liveness。

## 验证与可追溯性

| Claim                                  | Executable evidence                                                                                             | Limit                                 |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| 精确 authority 与顺序无关              | 状态机与 ingress unit trace；`AgentObservationJoin.tla` property `ApplicationRequiresExactLiveJoin`             | Validated input；有限 TLA identity    |
| At-most-once semantics 与 late fan-out | 状态机与 multi-presentation ingress trace；property `ApplicationIsAtMostOnce` 与 `LateJoinNeverReapplies`       | 有界 pending 与 recent window         |
| Generation fencing 与 local retirement | Replacement regression；property `MatchedPresentationNeverChangesGeneration` 与 `RetirementIsPresentationLocal` | TLC 中两个 presentation 与 generation |
| Expiry、stale deadline 与 disposal     | Logical-time 与 delayed-timer regression；property `ExpiredOrDisposedCannotApply` 与 `StaleDeadlineCannotApply` | 有界 logical time；不声明 progress    |
| Fail-closed collision 与 capacity      | 状态机 boundary test 与 strict ingress test                                                                     | 已记录的 same-user trust assumption   |

修改本 contract 或实现后，应运行 Desktop unit test、已注册的
`desktop-agent-observation-join` TLA+ check 与 root merge gate。
