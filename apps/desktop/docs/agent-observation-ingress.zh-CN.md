# Agent Observation Ingress

## 权威与范围

Desktop Main 拥有易失 join，负责把一条经过清理的 rich Agent hook event 与用于识别其 live
presentation 的有界 terminal correlation hint 关联起来。私有 Unix socket half 拥有 semantic
authority，但没有 presentation authority；private APC version-3 hint 只对一个 live terminal
generation 拥有 presentation authority，但没有 semantic authority。Visibility 属于可信 socket
envelope，而不属于 terminal half。任何一半都不能单独修改 canonical Agent state。

本 contract 覆盖 endpoint acceptance、精确配对、两种到达顺序、有界 settlement、重复处理、
presentation generation 替换、独立 fan-out、过期与 dispose。它不改变 provider event 含义、Agent
reducer、Workbench projection、notification policy、持久化或用户可见 copy。

## Transport 准入

Desktop 创建一个 user-private Unix-domain endpoint，并且只发布
`ORCHBEE_AGENT_OBSERVATION_SOCKET_V3`。Hook 先发送一条 strict、content-free v3 rich envelope；只有
收到完全一致的 `{"accepted":true,"version":3}\n` 后，才可以发送任何 terminal half。没有 endpoint、
拒绝、超时、ACK 缺少或多出 byte、断连与 malformed input 都会静默停止。只有 schema validation 与
closed-registry admission 成功后，Desktop 才写 ACK。

唯一 terminal frame 是 `ESC _ orchbee.agent.v3;<base64url-payload> ESC \\`。解码后的 JSON 只包含
`kind`、`version`、opaque `eventId` 与 opaque `correlationId`，不含 provider、activity、visibility、path、
hostname、mux identity、content 或 notification copy。OSC 9、OSC 777、OSC 99 与 OSC 1337 等公开
notification channel 都不是 observation transport。Socket acceptance 后，adapter 可以直接向解析出的
outer tmux/Zellij client TTY 写 private frame，不依赖 mux passthrough。

## 参考范围

2026-08-31 检查了下列本地 source revision，并把它们作为有界 design evidence：

| 产品             | 已审阅机制                                                                                                                   | Orchbee 决策                                                                                                                                                 |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Herdr `c2637dc1` | 先检测 foreground process，再选用 lifecycle hook 或 live screen 作为唯一 authority；其指南明确排除在内层 tmux 中启动的 Agent | Passive detection 保持无 capability，并把 evidence capture 与 deterministic transition 分离；不从 process 或 screen inspection 推断精确 nested-mux semantics |
| Muxy `a5d276e1`  | 带 total execution budget、bounded retry 与 acknowledgement 的 nonblocking Unix-socket hook client                           | 采用 private socket、准确 ACK 与 execution bound；保留 Orchbee 不含内容的 terminal half 和 generation-fenced two-half join                                   |
| tty7 `a2b5ae56`  | Provider hook diagnostics，以及通过 OSC 777 直接写 controlling TTY                                                           | 采用明确的 integration missing/outdated diagnostic；拒绝其公开 OSC notification transport                                                                    |
| Warp `086dade6`  | OSC 777 会调用 pluggable desktop-notification callback；已审阅 parser 会忽略非 Kitty APC payload                             | 把它作为选择 private APC、拒绝 OSC 777 的具体 compatibility evidence，而不是对所有 Warp 或 terminal version 的声明                                           |

这不是 compatibility 或 replication claim。竞品的 screen rule、payload、flat status、terminal UI、
provider lifecycle 与 timing constant 均不在范围内。当实现不同时，以 Orchbee 经验证的 rich hook fact、
精确 two-half correlation、presentation generation 与 canonical Agent runtime 为准。

## 输入与信任边界

| 输入                      | 准入与缺失语义                                                                                                                                                                           |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rich hook event           | Socket boundary 已执行 body limit、strict v3 schema、可信 visibility、准确 ACK，以及 closed-registry provider/event grant。没有匹配 hint 时，它保持 pending，且不产生 canonical effect。 |
| Terminal correlation hint | Terminal Host 已准入准确 private APC prefix 与有界 base64url UTF-8 payload，随后 Desktop 应用 strict hint schema。Join 只接受准确的 current presentation generation。                    |
| Presentation registration | 建立后续 hint 能为该 presentation 指定的唯一 generation。替换时先 retire previous generation。                                                                                           |
| Presentation retirement   | 只移除指定 presentation generation 的 pending 与 canonical navigation membership。                                                                                                       |
| Settlement deadline       | 指定安排它的准确 pending-event incarnation。Stale deadline、expired event 或没有 live match 的 event 均为 no-op。                                                                        |
| Expiry observation        | Logical time 由 ingress 提供。接受任一 half 之前以及 settlement 之前都会判断 expiry。                                                                                                    |
| Disposal                  | 取消全部 deadline，并清除 noncanonical join state，不发布 Agent 或 terminal state。                                                                                                      |

Provider payload、prompt、tool content、transcript path、path、error text 与 terminal screen text
永远不会进入该状态机。

## 数据、所有权与输出

`AgentObservationJoinStateMachine` 是 current presentation generation、pending event 与 hint、
correlation-to-event claim、matched identity 与 attachment、历史 presentation-generation match，以及
有界 recent-event replay fence 的唯一 writer。

状态机不依赖 socket、clock、timer、Agent runtime 或 renderer。它接收 validated input 与 logical
time，并返回 command。`AgentHookObservationIngress` 拥有执行 command 的外部机制：

| Command              | Effect owner 与结果                                                                                                                                                                                    |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Schedule settlement  | Ingress 创建一个以 event identity 与 expiry incarnation 为 key 的 timer。                                                                                                                              |
| Cancel settlement    | Ingress 只清除具有准确 identity 与 incarnation 的 timer。                                                                                                                                              |
| Apply observation    | Ingress 把 rich event 与所有 current matched attachment 一次性 dispatch；Desktop runtime 会先 retire 这些 presentation generation 上相同 provider 的准确 passive presence，且不发 semantic end event。 |
| Attach presentations | Ingress 把 late match 添加到已经 applied 的 live Session，但不重放其 semantic event。                                                                                                                  |
| Retire generation    | Ingress 从每个 canonical Agent Session detach 该准确 generation。                                                                                                                                      |

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

Visibility merge 只使用 socket 声明的值：`hidden` 优先；两个 `visible` value 仍为 `visible`；其他组合
均为 `unknown`。APC 不能改变 visibility。因此，同一 presentation generation 的多个 correlation 只产生
一个 attachment。打包 resolver 只会根据明确的 inner-mux evidence 声明 `hidden`，其他情况均声明
`unknown`；Workbench 负责把它与当前 application Tab/Panel visibility 进行最终组合。

## 边界与时间

仅识别身份的 hook 使用 `session-identified`。通过严格信封、provider 事件门禁和精确展示关联后，
转换器确保会话存在并附加展示，随后不改变回合、活动、请求或结果。新会话保持 `unknown` 回合；
重复身份保留所有已知状态。这些状态均由规范运行时持有。Cursor、Qoder、Antigravity、Mastracode、Droid 和 Kimi 发出此事件；胶囊 `0.4.7` 的旧
`session-started` 对 Cursor 具有相同的仅识别身份含义。其他 provider 保持原有的
`session-started` 行为，也不会获得新事件的准入权限。身份不证明就绪或进程存活。

胶囊 `0.4.13` 与匹配的原生和 Desktop 接收器一起发布。旧接收器拒绝新事件且不确认接收。
回滚整组运行时会恢复旧语义；无需持久状态迁移。直接边界与原生入口测试覆盖新旧身份、未知状态、
重复上报及其他 provider 拒绝。转换器测试另外通过合成 adapter 输入验证活动回合、待处理请求和
结果的保留；这不是公开准入证据。历史语料指纹保持不变。经过认证的 provider 执行和完整 Windows
流程验收仍是单独的证据义务。

Production 最多准入 512 个 pending rich event 与 1,024 个 pending hint。每个 half 的 join lifetime
为 10 秒。第一次 valid partial match 会在 25 毫秒后 schedule settlement。Applied event 保留在 pending
entry 中直到 join expiry，随后再进入 60 秒 replay fence。Cleanup 是 demand-driven，但每次 deadline
都会先执行 cleanup，因此 event-loop delay 不能发布 expired event。

这些边界不声明 Desktop restart 后的 delivery、所有 SSH attachment 都 disconnected 时的 delivery、
无界 replay protection、provider termination 或 wall-clock liveness。
已注册的 TLA+ variant 同时覆盖一个 declared correlation 与一对 correlation；singleton variant 要求
立即 apply，且不 schedule settlement。

Socket 最多接受 64 个 concurrent connection、每个 body 16 KiB、每个 connection 500 毫秒。继承
`ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` 的 local Agent process 只能使用该准确 owned socket；stale 或
malformed 的显式 path 不会回退到另一个 Desktop instance。只有没有 overlay 的 remote process 才扫描
最多 32 个 same-user forwarded socket candidate，并且该扫描要求继承的 `SSH_CONNECTION`、`SSH_CLIENT`
或 `SSH_TTY` marker 至少一个非空。没有准确 overlay 的普通 local terminal 不执行 socket scan。每次
connection 与 outer-TTY resolver 限制为 250 毫秒。ACK 只表示 envelope admission，不证明 terminal
delivery、correlation、semantic application 或 notification；没有 matching APC 的 accepted envelope
到期后不会产生 canonical effect。

## 验证与可追溯性

| Claim                                  | Executable evidence                                                                                             | Limit                                           |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| 精确 two-half authority                | 状态机与 ingress unit trace；`AgentObservationJoin.tla` property `ApplicationRequiresExactLiveJoin`             | Validated input；有限 TLA identity              |
| 已支持的 arrival-order equivalence     | 状态机 unit trace `admits one exact event in either half-arrival order`                                         | 配对的 singleton trace；不声明形式化 confluence |
| Singleton 立即 application             | 状态机 singleton trace；TLA+ action `JoinSingleton` 与 property `SingletonNeverSchedulesSettlement`             | 单 correlation 有限 variant                     |
| At-most-once semantics 与 late fan-out | 状态机与 multi-presentation ingress trace；property `ApplicationIsAtMostOnce` 与 `LateJoinNeverReapplies`       | 有界 pending 与 recent window                   |
| Generation fencing 与 local retirement | Replacement regression；property `MatchedPresentationNeverChangesGeneration` 与 `RetirementIsPresentationLocal` | TLC 中两个 presentation 与 generation           |
| Expiry、stale deadline 与 disposal     | Logical-time 与 delayed-timer regression；property `ExpiredOrDisposedCannotApply` 与 `StaleDeadlineCannotApply` | 有界 logical time；不声明 progress              |
| Fail-closed collision 与 capacity      | 状态机 boundary test 与 strict ingress test                                                                     | 已记录的 same-user trust assumption             |
| ACK-before-APC 与 private framing      | Adapter socket fixture 与 Terminal Host APC/OSC negative test                                                   | 仅限已审阅 terminal 与 mux 版本                 |

`ClineObservationOrdering` 是一个单独的 finite model，使用两个固定 conversation，且每个
conversation 的 run retention bound 为 2。它检查 completion-before-start、重复 completion、
activity、session isolation、按 completion 顺序进行的 run FIFO eviction，以及非 completion event
不修改 retention。Conversation-cache eviction 不在此 model 内，仍由 implementation test 提供证据。

修改本 contract 或实现后，应运行 Desktop unit test、已注册的
`desktop-agent-observation-join` TLA+ check 与 root merge gate。
