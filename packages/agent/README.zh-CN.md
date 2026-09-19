# @orchbee/agent

Orchbee 中 observed Agent 状态的 provider-neutral 领域模型与确定性运行时。

该包是 canonical observed Agent Session、Turn、Attention Request、Outcome、hierarchy、provenance、freshness、generation fencing 与 presentation projection 语义的唯一 owner。它不包含 provider adapter、Electron 或 React 依赖、transport 实现、平台 effect、prompt submission 或 Agent orchestration 权威。其机器可读 scope 由 [`project.json`](project.json) 定义。

## 模型

`AgentSessionSnapshot` 保留相互独立的事实，而不把它们压成单个 status enum：

- `lifecycle` 记录 observed Session 是 live 还是 ended；
- `turn` 是 unknown、肯定性 idle，或一个具有有界 phase 与 activity 的精确 active Turn；
- `activityHint` 只在 `turn` 保持 unknown 时保留 terminal-scoped、non-semantic activity hint；
- `pendingRequests` 保留有关联的问题、审批、身份验证需求或未分类 attention，直到精确 resolution、Turn outcome 或 Session end；
- `lastOutcome` 记录最近一次精确 Turn result，而不结束 live Session；
- `observation` 记录 source、fidelity、freshness、generation 与 monotonic ingestion sequence；
- `executionLocation` 是 opaque、provider-neutral 且 topology-neutral 的 execution anchor；以及
- `presentationAttachments` 把该 execution 绑定到零个或多个精确 local terminal presentation generation，每个 attachment 有独立 navigation 与 visibility。

Session、Turn、Request、Event 与 Provider identity 都是有界 opaque string。一个 runtime snapshot 最多包含 512 个 Session，一个 Session 最多包含 128 个 pending request。Parent Session 必须存在于同一 snapshot 中，且 parentage 必须无环。Ended Session 不能保留 active work 或 pending request。

Session、pending request 与 presentation attachment 的 canonical 排序是原始 UTF-16 code-unit 字典序：在任何进程 locale 下结果相同，不做规范化、大小写折叠或数字排序，且只有完全相同的字符串才判等。Identity 相等仍是精确字符串相等。Presentation ID 是非空、至多 256 个 UTF-16 code unit 的字符串，且必须是 well-formed Unicode——孤立 surrogate code unit 被拒绝，而 NUL、格式字符、组合符与合法代理对仍属合法值。

第一阶段只接受 `navigate-only` attention path。该包不导出 provider-native decision 或 resume capability。Prompt text、question text、command、tool input/output、transcript、filesystem path、environment value 与 provider error text 都不是模型字段。

## Transition 与信任

`transitionAgentRuntime(...)` 是作用于可信、有类型 `AgentObservationEvent` 的纯 reducer。
`AgentRuntime` 拥有一个 snapshot，只发布 applied revision，并把 semantic attention、resolution、outcome
与 Session-end event 同 state subscription 分开发出。它还把最近 512 个 applied revision 保留为
content-free change journal。每条 record 只包含 runtime revision、canonical transition kind 与当前
Session summary；unchanged 与 rejected input 不创建 record。`readChanges(...)` 返回 ordered page、
`current`、`revision-ahead`，或在调用者落后于 retention 时返回显式 `reset-required` result。重复或乱序
sequence value 保持 unchanged；generation、identity、capability、parent、location 或 Turn/Request
correlation 不匹配时拒绝且不发生 mutation。对 revision 已达最大安全整数的 snapshot，一次本可
applied 的 transition 会以 `capacity-exceeded` 拒绝，不产生 semantic event、不发生 mutation；
绝不抛出异常，也绝不抢先改变任何更先达成的 unchanged 或 rejected 结局。

Source 与 capability 相互独立。Heuristic source 不能声明 semantic capability。Terminal source 只能通过 `limited-activity-observed` 声明有界 activity；产生的 hint 绝不创建 Turn、肯定性 idle、outcome、attention request、semantic event 或 notification 权威。Native 与 hook source 只能声明其实际实现的 observational capability。`presentation-attached`、`presentation-visibility-changed` 与 `presentation-detached` 更新精确、受 generation fencing 的 presentation view，但不会重复 Session 语义；只 detach 一个 presentation 时，只要仍有其他 navigable attachment，Session 就保持 connected。Reconnect 需要显式 higher-generation snapshot，会重新验证 source capability 上限与每项 semantic claim，并且绝不从缺失中重建 completion 或 resolution。Staleness 只改变 freshness。Client acknowledgement 有意位于 canonical reducer 之外，不能解决 shared request。

精确的 upstream idle 或 settled event 可以关闭 active Turn，并且只清理该 Turn 的 pending request。它保留 `lastOutcome`，且不发出 outcome semantic event：idle 不是成功证据。Session-scoped request 会一直保留到自己的精确 resolution 或 Session end。

Reducer 不解析 arbitrary external value。每个 provider、IPC、storage 或 network adapter 都必须在第一个 trusted boundary 解析自己的有界 wire schema，然后才能构造 `AgentObservationEvent`。

## Projection

`projectAgentSession(...)`、`projectAgentLocation(...)` 与 `projectAgentLocations(...)` 按以下 semantic priority 派生 compact read model：

```text
open attention > failed/canceled/interrupted > succeeded > active Turn > idle > unknown > ended
```

Freshness 保持为独立 overlay，不能把 stale work 静默变成 idle。每个 attachment 获得同一份 Session 语义，因此一个 remote execution 可以如实 fan out 到多个 local Panel 或 Tab，而不重复 Turn、request、outcome 或 semantic event。当仍有 live Session 时，presentation aggregation 排除 retained ended Session，并从产生 resolved status 的 Session 或 attention 集合派生 freshness，因此较低优先级的 disconnected Session 不会降低无关 fresh work 的状态。对于具有多个 retained attachment generation 的 presentation ID，依次优先选择 navigable generation、更新鲜的 observation、更高 semantic priority，最后按稳定 generation 顺序选择。Provider identity 与 semantic status 保持分离，因此应用可以使用 curated localized name 与 icon，而不会接受 provider 控制的 product copy 或 markup。

## Semantic command read model

该 package 拥有 `agent.session.list`、`agent.session.get`、`agent.session.explain`、
`agent.session.wait` 与 `agent.session.changes` 的严格 version-1 契约。这些 descriptor、schema 与
确定性 projection helper 独立于 transport 与 application。Desktop application composition 可以在
`AgentSnapshotSource` 上准入 point-in-time read 与 wait，并在 `AgentChangeSource` 上准入 change feed；
CLI 只有在完整 live descriptor 匹配该 package 契约后，才能公开高层 projection。

`agent.session.list` 从一个 runtime revision 投影稳定 summary，默认只包含 live Session，可选包含保留的
ended Session，并把一页限制在 100 项。`agent.session.get` 返回精确保留的 canonical Session snapshot 与
其 semantic status。`agent.session.explain` 额外提供 source、fidelity、freshness meaning、generation、
sequence、capability ceiling 与 canonical status basis。Explanation 保持 `navigate-only` 且不含内容：
它绝不伪造 prompt、transcript、command、tool、path、provider error、adapter correlation 或
screen-matcher evidence。

`agent.session.wait` 定义 exact-Session matching，但不拥有 clock、subscription、execution ledger、
timeout effect 或 coordinator lifecycle。其默认 condition 包含 attention、全部 outcome、idle 与 ended。
显式 condition 可以选择任意 semantic status，或 `fresh`、`stale` 与 `disconnected` health value。
Semantic status 只在 observation fresh 时匹配；health 只匹配精确请求的值。可选 `afterRevision` 要求严格
更新的全局 runtime revision。Application-owned asynchronous handler 必须先 subscribe 再进行 decisive
recheck，并返回一个精确 matching snapshot，或显式 missing、ahead、timeout、cancellation 或
coordinator-retirement outcome。

`agent.session.changes` 定义 global、content-free change-page 契约。没有提供 cursor 时，application 会在
当前 revision 返回空 baseline。随后，签名 cursor 选择该 baseline 之后的下一个 revision；一页最多包含
100 条有序 record。Current cursor 可以长轮询，timeout 以空 `current` page 完成，retention loss 以空
`reset-required` page 完成，并提供位于当前 revision 的 cursor。Consumer 必须在 reset 后刷新 canonical
Session snapshot，再继续跟随。Journal 与 cursor 都是易失的，不声明跨 Desktop 或未来 Hub restart 连续性。

## 当前应用组合

Desktop 在 application lifetime 内拥有一个 volatile `DesktopAgentObservationRuntime`，包括有界 change
journal。Renderer document replacement 只退役旧 broker；runtime 与 journal 保持存活，下一个 renderer
通过严格的 `@orchbee/desktop-client-protocol` Agent-observation port 请求 fresh full snapshot。该 port
的 version 2 还会在 snapshot 更新后发布精确 semantic event，使 renderer 可以评估一次 notification
effect，而不用从状态重建事件。Web adapter 拒绝 malformed 或 uncorrelated message，并忽略
non-increasing revision。Workbench 消费狭窄的 `AgentSnapshotSource`，把非 ended 状态投影到精确
terminal panel header，并把一个 Tab 内的全部 terminal presentation 去重为一个 provider identity icon
与一个本地化的非 ended status adornment。它还会在不挂载非活动 terminal surface 的前提下，为每个
Workspace 派生 live Session 摘要。保留的 ended state 会从 panel、Tab、Workspace 摘要与 Workspace
详情展示中排除，因此普通 terminal 或 layout identity 会立即恢复。只有 Desktop semantic-command
composition 会为 signed cursor read 与 long polling 消费 `AgentChangeSource`。

Desktop 准入两类有意区分的 observation。Shell integration 发出的准确 private-APC v3 launch envelope 只能创建 fresh heuristic Session；它没有 capability，status 为 unknown，不发 semantic event 或 notification candidate，并会在未刷新时于 30 秒后 stale、120 秒后 retire。Retirement 只会断开并结束本地 heuristic presence projection，不会发出 semantic Session-end event。相同 provider、presentation 与 terminal generation 的 rich observation 成功准入后，会先 retire heuristic projection 再接管；当该准确 rich route 仍为 live 时，后续 passive evidence 会保持 suppressed。需要显式安装且分别绑定 provider 的 Claude Code、Codex、Grok、OpenCode、Pi、OMP、Gemini CLI、GitHub Copilot CLI、Cline 与 Qwen Code adapter，会各自验证 upstream contract，再剥离 prompt、tool content、transcript path、working directory、error text 与未知字段。它们通过私有 Unix-domain socket 发送共享的 normalized v3 rich allowlist，其中包含有界 correlation visibility。只有至少一个 candidate 返回准确 acceptance ACK 后，它们才向每个解析出的 outer TTY 单独写入不含内容的 private-APC correlation hint。封闭的应用 registry 授予 provider provenance、accepted event 与 maximum capability；adapter payload 不能给自己授予权威。任一 rich transport half 单独都没有足够权威：Desktop 只有对 live terminal generation 完成准确、有界的 event/correlation join 后，才接受 rich semantics。共享 resolver 覆盖 direct terminal、每个已 attached 的 tmux client、保守解析 Zellij client，以及 nested mux target。Shell integration 会通过 OpenSSH reverse Unix-socket forwarding，为符合条件的交互式 SSH session 提供一个随机 remote socket；不存在 HTTP port、Bearer credential、继承的 terminal secret、automatic remote installation、durable remote cache 或公开 terminal-notification channel。任一路径都可以失败，而不影响 Agent 或 terminal。

一个 Session 可以 attach 到多个 presentation，而不重复其 Turn、request、outcome 或 semantic event。Workbench 在 panel header、Tab icon 与去重后的 live Workspace 摘要中显示 aggregate。同一 Session 在一个 Workspace 的多个 presentation 中只计数一次，而跨 Workspace attachment 会在每个被表示的 Workspace 中各出现一次。当用户显式开启 Agent notification 且 system permission 已经 granted 时，精确 attention、success 与 failure event 只会在所有 attached destination 均隐藏时通知；激活通知会选择对应 Tab 与 panel。通知文案是 curated 的，绝不包含 provider 控制的内容。Provider identity 只作为 provenance 与可选 curated presentation identity；业务 transition 与 consumer 只依赖 canonical capability 与 event。自动 provider setup、SSH 完全断开期间的 delivery、durable Agent history，以及 Desktop 或操作系统 restart 后的连续性仍不属于这一切片。

## 正确性证据

Unit 与 property test 覆盖 transition guard、精确 attention capability 准入、durable request、outcome cleanup、无 outcome 的 idle settlement、stale overlay、semantic Session end、nonsemantic heuristic retirement 与 rich takeover、source capability ceiling、reconnect、presentation-generation fencing、multi-attachment fan-out 与 retirement、schema invariant、runtime disposal 与 deterministic projection。

Semantic-command contract 与 projection test 还覆盖默认和显式 wait condition、fresh-only semantic
matching、health matching、status-basis explanation、content absence、ended filtering、bounded schema、
ordered change batching、current read、overflow 与 explicit reset。Desktop operation test refinement
覆盖 pagination、exact-identity read、两种 subscribe-before-recheck wait、revision fencing、signed
change cursor、timeout、cancellation、overflow/reset 与 synchronous subscription race。

[`formal/tla/AgentRuntime.tla`](formal/tla/AgentRuntime.tla) 包含两个相互独立的有界场景。Reducer 场景在两个 Session、两个 client、observation generation 0 到 1、sequence 0 到 6 和 runtime revision 0 到 6 的范围内探索 event 顺序。它把重复及过时 sequence、错误 generation、错误 identity 和无效 observation 作为具名 no-op 纳入。权威 attachment 的 `unknown`、`hidden` 和 `visible` visibility 属于 canonical snapshot；renderer reconnect 只清除 client 缓存的 snapshot。该场景检查精确的 joined-observation 准入、acknowledgement 隔离、limited activity、无 outcome 的 idle settlement、reconnect fencing、ended Session 清理、精确的最终 snapshot 恢复，以及 attention、success 和 failure 的 notification candidate；cancelled 和 interrupted outcome 不产生通知。只发布一次最终 snapshot，是对中间 runtime publication 的显式约简。

Transport-order 场景单独探索 revision 0 到 2，并检查每个 semantic event 只会在包含该 event 的 snapshot revision 投递之后投递。两个场景都不假设公平性，也不证明无界 Session、event 或 revision 数量、upstream provider truthfulness、adapter payload correctness、provider termination、平台 notification delivery、pixel appearance，或 application restart 后的连续性。Required action coverage 横跨两个场景，防止任一场景在未触达其维护 transition 时空洞通过。

[`../../apps/desktop/formal/tla/AgentSemanticWait.tla`](../../apps/desktop/formal/tla/AgentSemanticWait.tla)
是 application-composition wait model。其有界状态包含两个 Session、两个 semantic status、三个 health
value、两次 Agent mutation（revision 0 到 2）与两个 coordinator generation。TLC 检查 exact subscription ownership、
subscribe-before-decisive-recheck eligibility、fresh-only semantic match、精确 matched output、没有伪造的
unmatched output、Agent-only revision ownership，以及显式 missing、future-baseline、timeout、
cancellation 与 coordinator-retirement path。这些边界不证明无界 waiter、time、revision、Session 或
restart continuity。

[`../../apps/desktop/formal/tla/AgentSemanticChanges.tla`](../../apps/desktop/formal/tla/AgentSemanticChanges.tla)
是 application-composition change-feed model。它使用 revision 0 到 4、retention 2、page bound 2 与两个
coordinator generation；TLC 检查有界 ordered journal suffix、baseline、ordered batching、精确 current
subscription、decisive recheck 后的 `WaitingCursorIsCurrent`、空 timeout、invalid cursor、cancellation、
coordinator retirement，以及用显式 reset 取代静默 retention gap。这些边界不证明无界 retention、
revision、reader、cryptographic cursor security 或 restart continuity。

## 验证

在仓库根目录运行：

```sh
pnpm --filter @orchbee/agent verify:correctness
```

更广泛的未来设计与仍保留的产品决策继续记录在双语 [Agent runtime observation 提案](../../proposals/agent/agent-runtime-observation.zh-CN.md)中。
