# @orchbee/agent

私有包入口只导出仓库内有消费者的符号，Knip 的 `includeEntryExports` 检查该边界。内部契约、生成资产与独立一致性证据按其所有权保留；它们的存在不自动扩大公共入口。新增客户端应按已准入用途添加所需导出。

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

提交后，单个 snapshot 或 semantic 观察者的异常不能改变 applied 结果，也不能阻断其余发布。
每个 applied transition 提交后捕获一次 snapshot 回调；semantic 回调则针对每个 event 分别捕获。
[Runtime 测试](src/__tests__/AgentRuntime.unit.test.ts) 覆盖异常隔离和稳定的 snapshot 回调集合。

Source 与 capability 相互独立。Heuristic source 不能声明 semantic capability。Terminal source 不能声明语义生命周期或 attention 权威。Native 与 hook source 只能声明其实际实现的 observational capability。`presentation-attached`、`presentation-visibility-changed` 与 `presentation-detached` 更新精确、受 generation fencing 的 presentation view，但不会重复 Session 语义；只 detach 一个 presentation 时，只要仍有其他 navigable attachment，Session 就保持 connected。Reconnect 需要显式 higher-generation snapshot，会重新验证 source capability 上限与每项 semantic claim，并且绝不从缺失中重建 completion 或 resolution。Staleness 只改变 freshness。Client acknowledgement 有意位于 canonical reducer 之外，不能解决 shared request。

精确的 upstream idle 或 settled event 可以关闭 active Turn，并且只清理该 Turn 的 pending request。它保留 `lastOutcome`，且不发出 outcome semantic event：idle 不是成功证据。Session-scoped request 会一直保留到自己的精确 resolution 或 Session end。

Reducer 不解析 arbitrary external value。每个 provider、IPC、storage 或 network adapter 都必须在第一个 trusted boundary 解析自己的有界 wire schema，然后才能构造 `AgentObservationEvent`。

## Projection

`projectAgentSession(...)`、`projectAgentLocation(...)` 与 `projectAgentPresentationGroup(...)` 按以下 semantic priority 派生 compact read model：

```text
open attention > failed/canceled/interrupted > succeeded > active Turn > idle > unknown > ended
```

Freshness 保持为独立 overlay，不能把 stale work 静默变成 idle。每个 attachment 获得同一份 Session 语义，因此一个 remote execution 可以如实 fan out 到多个 local Panel 或 Tab，而不重复 Turn、request、outcome 或 semantic event。当仍有 live Session 时，presentation aggregation 排除 retained ended Session，并从产生 resolved status 的 Session 或 attention 集合派生 freshness，因此较低优先级的 disconnected Session 不会降低无关 fresh work 的状态。对于具有多个 retained attachment generation 的 presentation ID，依次优先选择 navigable generation、更新鲜的 observation、更高 semantic priority，最后按稳定 generation 顺序选择。Provider identity 与 semantic status 保持分离，因此应用可以使用 curated localized name 与 icon，而不会接受 provider 控制的 product copy 或 markup。

## Semantic command read model

该包保留规范语料生成器使用的三个确定性参考投影：`projectAgentSessionSummary`、`explainAgentSession` 和 `matchingAgentWaitConditions`。生成器执行所属包的构建产物；在私有开发构建边界准入前，这些参考仍通过现有入口提供。它们不恢复已退役的 TypeScript CLI 或 Desktop semantic-command coordinator。

Summary 投影规范 Session 状态。Explanation 保留 source、fidelity、freshness meaning、generation、sequence、capability ceiling 和 status basis，保持 `navigate-only` 且不含 provider 内容。Semantic wait condition 仅匹配 fresh observation；显式 health condition 匹配所请求的 freshness 值。这些纯投影不拥有订阅、计时器或命令生命周期。

有界 runtime change journal 及其 revision／reset 行为仍由 `AgentRuntime` 和 `changes.ts` 负责。删除退役 CLI 信封不删除这些运行证据，也不承诺跨重启的持久历史。

## 当前应用组合

Desktop 在 application lifetime 内拥有一个 volatile `DesktopAgentObservationRuntime`，包括有界 change
journal。Renderer document replacement 只退役旧 broker；runtime 与 journal 保持存活，下一个 renderer
通过严格的 `@orchbee/desktop-client-protocol` Agent-observation port 请求 fresh full snapshot。该 port
的 version 1 还会在 snapshot 更新后发布精确 semantic event，使 renderer 可以评估一次 notification
effect，而不用从状态重建事件。Web adapter 拒绝 malformed 或 uncorrelated message，并忽略
non-increasing revision。Workbench 消费狭窄的 `AgentSnapshotSource`，把非 ended 状态投影到精确
terminal panel header，并把一个 Tab 内的全部 terminal presentation 去重为一个 provider identity icon
与一个本地化的非 ended status adornment。它还会在不挂载非活动 terminal surface 的前提下，为每个
Workspace 派生 live Session 摘要。保留的 ended state 会从 panel、Tab、Workspace 摘要与 Workspace
详情展示中排除，因此普通 terminal 或 layout identity 会立即恢复。已退役的 Desktop semantic-command coordinator 不是 change journal 的当前消费者。

Desktop 准入两类有意区分的 observation。Shell integration 发出的准确 private-APC v3 launch envelope 只能创建 fresh heuristic Session；它没有 capability，status 为 unknown，不发 semantic event 或 notification candidate，并会在未刷新时于 30 秒后 stale、120 秒后 retire。Retirement 只会断开并结束本地 heuristic presence projection，不会发出 semantic Session-end event。相同 provider、presentation 与 terminal generation 的 rich observation 成功准入后，会先 retire heuristic projection 再接管；当该准确 rich route 仍为 live 时，后续 passive evidence 会保持 suppressed。需要显式安装且分别绑定 provider 的 Claude Code、Codex、Grok、OpenCode、Pi、OMP、Gemini CLI、GitHub Copilot CLI、Cline 与 Qwen Code adapter，会各自验证 upstream contract，再剥离 prompt、tool content、transcript path、working directory、error text 与未知字段。它们通过私有 Unix-domain socket 发送共享的 normalized v3 rich allowlist，其中包含有界 correlation visibility。只有至少一个 candidate 返回准确 acceptance ACK 后，它们才向每个解析出的 outer TTY 单独写入不含内容的 private-APC correlation hint。封闭的应用 registry 授予 provider provenance、accepted event 与 maximum capability；adapter payload 不能给自己授予权威。任一 rich transport half 单独都没有足够权威：Desktop 只有对 live terminal generation 完成准确、有界的 event/correlation join 后，才接受 rich semantics。共享 resolver 覆盖 direct terminal、每个已 attached 的 tmux client、保守解析 Zellij client，以及 nested mux target。Shell integration 会通过 OpenSSH reverse Unix-socket forwarding，为符合条件的交互式 SSH session 提供一个随机 remote socket；不存在 HTTP port、Bearer credential、继承的 terminal secret、automatic remote installation、durable remote cache 或公开 terminal-notification channel。任一路径都可以失败，而不影响 Agent 或 terminal。

一个 Session 可以 attach 到多个 presentation，而不重复其 Turn、request、outcome 或 semantic event。Workbench 在 panel header、Tab icon 与去重后的 live Workspace 摘要中显示 aggregate。同一 Session 在一个 Workspace 的多个 presentation 中只计数一次，而跨 Workspace attachment 会在每个被表示的 Workspace 中各出现一次。当用户显式开启 Agent notification 且 system permission 已经 granted 时，精确 attention、success 与 failure event 只会在所有 attached destination 均隐藏时通知；激活通知会选择对应 Tab 与 panel。通知文案是 curated 的，绝不包含 provider 控制的内容。Provider identity 只作为 provenance 与可选 curated presentation identity；业务 transition 与 consumer 只依赖 canonical capability 与 event。自动 provider setup、SSH 完全断开期间的 delivery、durable Agent history，以及 Desktop 或操作系统 restart 后的连续性仍不属于这一切片。

## 正确性证据

Unit 与 property test 覆盖 transition guard、精确 attention capability 准入、durable request、outcome cleanup、无 outcome 的 idle settlement、stale overlay、semantic Session end、nonsemantic heuristic retirement 与 rich takeover、source capability ceiling、reconnect、presentation-generation fencing、multi-attachment fan-out 与 retirement、schema invariant、runtime disposal 与 deterministic projection。

Projection 测试保留 fresh-only semantic matching、显式 health matching、status-basis explanation 和 content absence。Runtime journal 测试保留有序变化、current read、overflow 和 reset。规范语料执行所保留的参考投影，并由 Rust Server 独立消费。

## 验证

在仓库根目录运行：

```sh
pnpm --filter @orchbee/agent verify:correctness
```

更广泛的未来设计与仍保留的产品决策继续记录在双语 [Agent runtime observation 提案](../../proposals/agent/agent-runtime-observation.zh-CN.md)中。
