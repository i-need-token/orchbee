# Agent Runtime 观测与待处理事项

| 字段       | 内容                                                                                                                                                                                                                                                                                                                                                        |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态       | 第一阶段 core、本地 observation、已接受的 SSH、tmux、Zellij 拓扑、显式 multi-Agent adapter，以及 Claude Code、Codex、Grok、OpenCode、Pi 与 OMP 的显式本地集成管理已获授权；后续保留决策仍未解决                                                                                                                                                             |
| 决策 Owner | Active requester 以 Orchbee 产品与架构 decision owner 身份接受了第一阶段授权；每项保留决策仍需其所需 owner 明确接受                                                                                                                                                                                                                                         |
| 语义 Owner | Agent capability、应用运行时、Workbench 应用组合层与平台通知 adapter                                                                                                                                                                                                                                                                                        |
| 当前权威   | [`docs/ARCHITECTURE.md`](../../docs/zh-CN/ARCHITECTURE.md)、[`@orchbee/agent`](../../packages/agent/README.zh-CN.md)、[`@orchbee/terminal-protocol`](../../packages/terminal-protocol/README.zh-CN.md)、[`@orchbee/workbench`](../../packages/workbench/README.zh-CN.md)，以及 [unified-search 契约](../../packages/workbench/docs/unified-search.zh-CN.md) |
| 退出条件   | 实现分别接受的切片，把持续维护的状态、adapter、呈现、通知与证据契约迁入各自所属 package 和 application 指南与测试，解决或移除全部剩余未来决策，然后删除本提案及其译本                                                                                                                                                                                       |

英文 [agent-runtime-observation.md](agent-runtime-observation.md) 是 canonical 版本；本文用于简体中文评审。

## 变更授权

| 字段         | 建议授权                                                                                                                                                                                                                                                                                                              |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 分级         | 该能力拥有异步状态、顺序、层级、取消与多客户端投影，因此属于 Modeled；它引入跨 workspace 契约、消费不可信进程事件且可产生操作系统通知，因此同时属于 Consequential                                                                                                                                                     |
| 问题         | 用户可以让 terminal-backed Agent 持续运行，但 Orchbee 无法可靠区分工作、空闲、提问、批准请求、完成、失败或观测丢失。非活动终端 surface 可能被卸载，因此 renderer-local 观测不能提供完整的后台待处理事项或完成反馈。                                                                                                   |
| 第一阶段结果 | 通过显式安装且 capability-bounded 的 adapter 观测 terminal-bound Claude Code、Codex、Grok、OpenCode、Pi 与 OMP session，包括本地 Orchbee Tab 通过 SSH 连接远端 tmux 或 Zellij client；在 renderer 生命周期之外维护一个 canonical observed-session 模型；并把可信状态投影到每个精确 attachment、Tab 与符合条件的通知。 |
| 非目标       | 启动 Agent、拥有 prompt 或提交、编排工作、把终端文本解析为语义权威、在界面内回答问题或批准、修改 PTY 或 Agent 进程权威、持久 transcript 或 prompt 历史、所有 SSH attachment 断开后的实时交付、移动端推送，或跨设备 exactly-once 通知交付                                                                              |
| 兼容性       | Agent 观测缺失、禁用、不支持、过期或失败时，现有 terminal、shell integration、命令完成提醒、OSC title、progress、close activity 与 terminal lifecycle 均保持不变。                                                                                                                                                    |
| 交付         | 每个已接受切片只有一个 owner 和证据边界。领域模型先于 adapter、transport、UI 与通知 effect。                                                                                                                                                                                                                          |
| 恢复         | 可以禁用或移除 Agent adapter，而不停止其 terminal。Adapter 或 monitor 失败时，显示的观测降级为 stale 或 unknown，绝不凭空产生 idle、success、approval 或进程终止。                                                                                                                                                    |

当前 terminal progress、close activity 与命令完成都是有界的呈现事实，而不是 Agent lifecycle
权威。它们只能继续作为低保真 fallback 证据使用。当前全 Tab 监控限制由
[shell-integration 契约](../../apps/web/docs/terminal-settings.zh-CN.md#shell-integration-与完成提醒)维护。

## 已接受架构

| 备选方案                            | 后果                                                                                    | 决策 |
| ----------------------------------- | --------------------------------------------------------------------------------------- | ---- |
| 单个扁平 `AgentStatus` enum         | 混淆 Session 生命周期、Turn activity、pending request、outcome 与观测健康               | 拒绝 |
| Renderer 或 Workbench 拥有观测      | 遗漏已卸载 Tab，把框架生命周期耦合到 Agent 状态，并让 Workbench 获得 orchestration 权威 | 拒绝 |
| Terminal Host 拥有 Agent 语义       | 用 provider-specific 产品行为污染 PTY、进程、Ghostty 与 terminal-protocol 权威          | 拒绝 |
| Agent capability 加有类型的 adapter | 保留一个 canonical 模型、明确的信任与保真度、后台监控和可复用投影                       | 接受 |

```text
Provider native API / managed hooks
                  |
      Bounded adapter facts
                  |
 Local Unix socket or SSH reverse-forwarded Unix socket
                  |                         Exact outer PTY
                  |                    bounded OSC correlation hint
                  +-------------+-------------+
                                |
                    Application ingress join
                                |
                   Agent observation reducer
                                |
      Canonical execution location and attachments
                                |
              Pane / Tab / Activity / notification
```

| Owner               | 职责                                                                                                                                                                                                                              |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Agent capability    | Canonical observed Session、Turn、Attention Request、Outcome、层级、观测来源、确定性 transition model、聚合与 provider-neutral read model                                                                                         |
| Provider adapter    | 校验一个外部来源，把其固定 adapter identity 关联到一个 Agent Session 与 opaque execution location，并把受支持事实翻译为 canonical event；由 application-owned registry 授予 capability，adapter 不拥有 capability、呈现或通知策略 |
| 应用运行时          | 拥有 adapter 生命周期、本地与 reverse-forwarded ingress、有界双通道关联、presentation attachment generation、ingestion 时的事件排序、独立于 renderer 挂载的订阅、设备通知策略与尝试记录，以及平台 effect 组合                     |
| Workbench 与应用 UI | 消费 Agent-owned read model 与 navigation Action，把状态组合到现有屏幕区域，不拥有 Agent transition 或 provider payload                                                                                                           |
| Terminal Host       | 继续拥有 PTY、进程、Ghostty state、terminal history、attachment 与 terminal transport；不加载 Agent provider，也不决定 Agent 状态                                                                                                 |
| Client view         | 拥有 presentation-scoped visibility、focus、attachment generation 与事件 acknowledgement，不解决 shared Agent request                                                                                                             |
| 平台通知 adapter    | 尝试一个已经授权、本地化且有界的通知 effect，并报告 delivery API outcome，不改变 Agent 状态                                                                                                                                       |

第一个已接受的实现切片只能在确定性模型拥有真实 application consumer 时创建 `@orchbee/agent`。
Serialized Agent protocol 只能随第一个真实的跨进程 producer 和 consumer 创建；其 schema 可以表示经过
校验的 wire data，但不得拥有 transition 合法性或产品策略。已接受的 SSH 切片让该 runtime 继续与
Desktop 共址，并使用 application-owned local Unix socket；后续进程拆分与 protocol package 命名仍是
保留决策。

## 输入与观测质量

| 输入                                 | 准入与含义                                                                                                                                     | 缺失或失败                                                      |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Managed native provider event        | 在应用信任边界解析；只接受 registry-granted provider capability 与精确关联的 identity                                                          | 保留最后一次观测，并在已接受的策略边界后把 freshness 转为 stale |
| Managed provider-hook event          | 在传输前解析有界 allowlist；只有 local Unix socket 或其已鉴权 SSH reverse forwarding 才能准入 rich fact                                        | Hook 不能阻塞 Agent；事件缺失绝不表示 idle 或 completion        |
| 应用 presentation lifecycle          | 提供精确 local presentation 与 binding generation；一个 execution 可以 attach 到多个 presentation，且每个 presentation 可独立退役              | 只退役该 attachment；仅当不再有可信 route 时断开 observation    |
| Structured terminal in-band signal   | 普通 OSC 作为不可信 display hint；精确 correlation token 只有与匹配的 trusted rich event 拼接后才有用                                          | 该输入单独不能产生 rich semantic transition                     |
| 进程名、activity 或 screen heuristic | 只能检测 generic provider identity 或近期 terminal activity；不能断言 question、approval、idle state、successful outcome 或 actionable request | 显示 generic unknown 状态或不显示 Agent 状态                    |
| 已鉴权 client visibility 与 receipt  | 提供精确 presentation、hidden/visible state、attachment generation 与已 acknowledgement 的 semantic event identity                             | 不改变 canonical Agent 状态                                     |
| 规范化 freshness tick                | 运行时把 monotonic time 策略转换为显式 stale event；reducer 从不读取 clock                                                                     | 仅时间流逝本身不是 reducer transition                           |

观测质量具有相互独立的维度：

| 维度        | 取值                                                  | 含义                                                                                       |
| ----------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Source      | `native`、`hook`、`terminal`、`heuristic`             | 产生该事实的机制                                                                           |
| Fidelity    | `rich`、`basic`、`heuristic`                          | Adapter 能支持的语义区分                                                                   |
| Freshness   | `fresh`、`stale`、`disconnected`                      | 运行时能否继续支持最后一次 observed state                                                  |
| Capability  | `turnLifecycle`、`questions`、`approvals` 等命名 flag | Application registry 可以作出的授权；adapter input、source 或 branding 绝不授予 capability |
| Action path | `navigate-only`，或在单独接受后使用 `native-action`   | Orchbee 只能展示 provider terminal，还是可以提交有关联的 provider-native decision          |

Fidelity 与 trust 不是置信度百分比。更丰富的 source 可以断言更多命名事实；terminal 或 heuristic
source 不能因为输出碰巧类似已知 prompt 就变得 actionable。

## Canonical 数据模型

```ts
interface AgentSessionSnapshot {
	readonly sessionId: AgentSessionId;
	readonly providerId: AgentProviderId;
	readonly executionLocation: AgentExecutionLocation;
	readonly presentationAttachments: readonly AgentPresentationAttachment[];
	readonly parentSessionId: AgentSessionId | null;
	readonly lifecycle: 'live' | 'ended';
	readonly turn: AgentTurnState;
	readonly pendingRequests: readonly AgentAttentionRequest[];
	readonly lastOutcome: AgentTurnOutcome | null;
	readonly observation: AgentObservation;
}

interface AgentExecutionLocation {
	readonly executionLocationId: AgentExecutionLocationId;
}

interface AgentPresentationAttachment {
	readonly generation: string;
	readonly navigable: boolean;
	readonly presentationId: string;
	readonly visibility: 'visible' | 'hidden' | 'unknown';
}

type AgentTurnState =
	| { readonly kind: 'unknown' }
	| { readonly kind: 'idle' }
	| {
			readonly activity: AgentActivity | null;
			readonly kind: 'active';
			readonly phase: 'queued' | 'working' | 'waiting-external';
			readonly turnId: AgentTurnId;
	  };

type AgentActivity =
	'thinking' | 'tool' | 'editing' | 'command' | 'subagent' | 'compacting' | 'retrying' | 'other';

interface AgentAttentionRequest {
	readonly actionPath: 'navigate-only' | 'native-action';
	readonly kind: 'question' | 'approval' | 'authentication' | 'unclassified';
	readonly requestId: AgentRequestId;
	readonly turnId: AgentTurnId | null;
}

interface AgentTurnOutcome {
	readonly eventId: AgentEventId;
	readonly kind: 'succeeded' | 'failed' | 'canceled' | 'interrupted';
	readonly turnId: AgentTurnId;
}

interface AgentObservation {
	readonly fidelity: 'rich' | 'basic' | 'heuristic';
	readonly freshness: 'fresh' | 'stale' | 'disconnected';
	readonly generation: number;
	readonly sequence: number;
	readonly source: 'native' | 'hook' | 'terminal' | 'heuristic';
}
```

- Orchbee identity 在其声明的 lifecycle 内保持 opaque 且稳定。Provider identifier 是
  provider-scoped correlation data，绝不因为字符串相等就成为 global identity。
- `AgentExecutionLocation` 是 opaque provider-neutral execution anchor。它不会向业务策略暴露 SSH、
  tmux、Zellij、host、Pane、path、process 或 provider detail。
- 一个 Session 有零个或多个受 generation fencing 的 `AgentPresentationAttachment` record。因此同一个
  remote execution 可以更新全部真实 attached 的本地 Orchbee presentation，而 unrelated Tab 保持不变。
  重用 Pane、terminal path、process ID 或 provider Session ID 都不能让旧 event 重新指向新目标。
- Attachment visibility 属于 presentation scope。即使另一个 inner pane 处于 active，隐藏的 tmux 或
  Zellij Agent Pane 仍保持 attached 且可导航；该 hidden state 可以让 attention、completion 或 failure
  获得 notification 资格，但不会制造 Agent 语义。
- 同一 terminal 中顺序运行的 Agent 获得不同 Session identity。父 Agent 与子 Agent 获得不同
  Session identity，并具有一条显式的无环父级关系。
- 一个 Session 最多有一个直接 active Turn。并行 subagent 是 child Session，而不是隐藏在一个
  Session 内的多个 direct Turn。
- `idle` 是 adapter 的肯定性观测。Basic 或 heuristic source 缺少 active Turn 时是 `unknown`，
  而不是 `idle`。
- Attention Request 会一直存在，直到有关联的 resolution、cancellation 或 Session end；它不由
  瞬态 status boolean 表示。
- Outcome 是 semantic event 与近期事实，而不是 Session lifecycle。一个 failed Turn 之后，同一
  live Session 仍可运行另一个 Turn。
- Runtime 为 Activity projection 保留一组有界 recent canonical Session 与 event record。精确 bound、
  acknowledgement persistence 与 restart retention 仍为保留决策；retained record 不包含额外 provider
  content。
- `starting` 不是第一阶段 Session 权威，因为本提案不拥有 Agent launch。可靠 observed queued Turn 可以
  显示为 queued；观测开始前的 launch state 属于后续 launch owner。
- 第一阶段 canonical 模型不包含 provider prompt text、command text、tool input/output、
  transcript、filesystem path 或 environment value。

## Transition 模型

| 事件                            | Guard                                                                                          | Canonical result 与 command                                                                                                                              |
| ------------------------------- | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `SessionObserved`               | 新 Orchbee Session ID、已准入 provider、opaque execution location、零个或多个 valid attachment | 创建 `turn: unknown`、pending request 为空、没有 outcome、来源精确且 attachment 受 generation fencing 的 live Session；发布一个 snapshot                 |
| `TurnStarted`                   | Live Session、当前 observation generation、没有不同的 active Turn                              | 把精确 Turn 设为 `queued` 或 `working`；重复的精确事件为 no-op；不同的 active Turn 必须先有显式 interruption                                             |
| `TurnActivityChanged`           | 精确 active Turn 与已声明 activity capability                                                  | 只替换 activity 或 `waiting-external`；activity 变化绝不产生 notification candidate                                                                      |
| `AttentionRequested`            | Live Session、唯一 request identity、已声明 attention capability、匹配可选 Turn                | 添加一个 pending request 并发布一个 attention semantic event；不推断所有 work 或 child Session 已停止                                                    |
| `AttentionResolved`             | 精确 open request 与当前 generation                                                            | 删除 pending request 并发布一个 resolution event；仅 client acknowledgement 绝不解决该请求                                                               |
| `TurnFinished`                  | 精确 active Turn、已声明 outcome capability                                                    | 记录一个 outcome，把 Turn 转为肯定性 `idle`，用显式原因取消未解决的 Turn-scoped request，并发布一个 outcome event 与符合条件的 effect candidate          |
| `ObservationBecameStale`        | Live 或 ended retained Session、当前 generation                                                | 保留最后的 Turn、request 与 outcome；只改变 freshness 并发布 degraded snapshot                                                                           |
| `ObservationReconnected`        | 更高 generation，加上已校验的 current snapshot 或显式 current event                            | 只接纳提供的事实；拒绝更旧 generation event，绝不从缺失中重建遗漏的 success 或 resolution                                                                |
| `PresentationAttached`          | 已有 execution location、精确 presentation generation、没有冲突的 current attachment           | 只添加或刷新该 presentation；不重复 Session 语义或 notification event                                                                                    |
| `PresentationVisibilityChanged` | 精确 current attachment generation 与已鉴权 client visibility                                  | 只改变该 attachment 的 `visible`、`hidden` 或 `unknown` view；不改变 Agent 状态                                                                          |
| `PresentationDetached`          | 精确 current attachment generation                                                             | 只退役该 presentation 的 navigation；仅在没有可信 route 或 navigable attachment 时把 observation 标为 disconnected；绝不推断 Session end 或 Turn success |
| `SessionEnded`                  | Live Session、当前 generation                                                                  | 结束 Session；除非已有精确 outcome，否则 interrupt 仍 active 的 Turn；取消 open request；发布一个 Session-end semantic event                             |
| `ClientAcknowledged`            | 已鉴权 client 与 retained semantic event                                                       | 只更新对应 client view；不改变 Session、Turn、request 或 outcome 权威                                                                                    |

Malformed、duplicate、stale-generation、out-of-order、unsupported-capability 与 identity mismatch
事件只产生有界 diagnostic，不发生 canonical mutation。Adapter 必须把 provider-specific supersession
翻译成显式 interruption；reducer 绝不静默地用一个 active Turn 覆盖另一个。

## 不变量

- 每个 mutable Agent fact 都只有一个 Agent capability owner；adapter 与 presentation 不能直接写
  canonical state。
- 每个 accepted event 属于一个 Session generation 与一个 monotonic ordered ingestion sequence；
  更旧 generation 不能改变 replacement Session、execution location 或 presentation attachment。
- Ingestion sequence 对 normalized arrival 排序，不声称 upstream causality。精确 Session、Turn、request、
  event 与 generation guard 会拒绝不再匹配 current state 的晚到 callback。
- Live Session 恰好处于 `unknown`、肯定性 `idle` 或一个精确 active Turn 之一。
- Ended Session 没有 active Turn，也没有 open Attention Request。
- 每个 Turn-scoped request 在准入时属于当前精确 Turn；每个 request identity 在其 Session
  generation 内唯一。
- Session parentage 无环，每个 child 至多有一个 parent，且 child 不能通过晚到事件改变 root。
- Observation staleness 绝不变成 idle、success、failure、request resolution 或 Session end。
- Terminal 与 heuristic signal 不能创建 native action、answer、approval decision 或 high-trust
  notification candidate。
- Client event acknowledgement 绝不解决 shared request；request resolution 也不表示每个 client
  都看到了该事件。
- 添加第二个 presentation attachment 绝不重复底层 Turn、Attention Request、Outcome、semantic event 或
  notification attempt。只 detach 一个 presentation 不能断开仍有其他 trusted route 的 Session。
- OSC correlation half 在 trusted Unix-socket channel 收到有界匹配 event half 前，不能创建、修改或结束
  rich Session；未匹配的 half 会过期，不发生 domain transition。
- Presentation 与 aggregation 是 canonical Agent state 和显式 client view 的纯确定性投影。

## Provider adapter

| Capability         | Provider-neutral 含义                                                               |
| ------------------ | ----------------------------------------------------------------------------------- |
| `sessionLifecycle` | 精确 observed Session start 与 end                                                  |
| `turnLifecycle`    | 精确 Turn start、finish、肯定性 idle、interruption 与 outcome                       |
| `questions`        | 有关联的用户 question request 与 resolution                                         |
| `approvals`        | 有关联的 permission 或 approval request 与 resolution                               |
| `authentication`   | 有关联的 authentication-required 观测                                               |
| `activity`         | 不包含 raw tool input/output 的有界 activity category                               |
| `subagents`        | 稳定的 parent-child Session 关系                                                    |
| `progress`         | Provider-owned bounded progress；不会把 generic terminal progress 提升为 Agent 权威 |
| `decisionChannel`  | Provider-native correlated response path；即使 source 支持，第一阶段也排除          |
| `resume`           | Provider-native Session resume 或 navigation；除非单独接受，否则不属于第一阶段观测  |

| Adapter source                    | 第一阶段允许的投影                                                                             |
| --------------------------------- | ---------------------------------------------------------------------------------------------- |
| Native provider API 或 app server | 每项已声明的 observed capability；native decision 保持禁用                                     |
| Managed provider hook             | 已声明的 Session、Turn、attention、activity、hierarchy 与 outcome fact；只能导航               |
| Structured terminal signal        | Generic provider identity 与有界 basic activity，并明确标为 untrusted 或 limited-status        |
| Process 或 screen heuristic       | 仅 generic detected/unknown 状态；不具备 attention、idle、outcome、progress 或操作系统通知资格 |

Claude Code 通过官方 [Hooks reference](https://code.claude.com/docs/en/hooks) 暴露 lifecycle、tool、
permission、subagent、stop 与 failure event。Codex 通过 [Codex Hooks](https://learn.chatgpt.com/docs/hooks)
暴露对应 hook event，而 [Codex App Server](https://learn.chatgpt.com/docs/app-server) 为未来的 thread、
turn、item、approval 与 user-input flow 提供 native integration。这些上游机制证明 adapter 可行性；
它们的 payload 不是 Orchbee 的领域模型或兼容性权威。

Provider identity 与 semantic status 保持分离。Curated product registry 拥有本地化 provider name、
icon 与安全 brand accent。Adapter 可以提供 stable provider ID 和 namespaced diagnostic metadata，
但不能注入 arbitrary markup、icon、color 或 product-facing string。不支持的 provider 使用 generic
Agent presentation，并只声明已经证明的 capability。

Agent integration setup 必须显式、可恢复，并且 Orchbee 不存在时不能阻塞 Agent。任何实现都不能静默
编辑用户全局 Agent configuration。Launch-scoped setup、managed plugin installation 与显式
persistent configuration 是实质不同的选择，需要被接受的 integration decision。已接受的 Claude Code
选择与已接受的 multi-Agent 扩展记录在下文。

### 已接受的 Claude Code setup 与 SSH/mux 双通道 ingress

2026-08-27，active requester 以产品与架构 decision owner 身份接受 Claude Code 作为第一个 provider，
并选择显式 managed-plugin setup。Orchbee 可以展示 configuration 供评审，但由用户安装并启用；Orchbee
绝不静默编辑全局 Claude Code configuration。Managed command hook 读取有界 provider event，丢弃
prompt、tool input/output、command、transcript path、working directory、arbitrary message 与 provider
error text，只发出 allowlisted fact。Hook failure 不阻塞，禁用或卸载即为 rollback。官方
[Hooks reference](https://code.claude.com/docs/en/hooks) 与
[plugin reference](https://code.claude.com/docs/en/plugins-reference) 是 upstream setup 证据，不是
Orchbee compatibility authority。

先前交付的随机 loopback HTTP endpoint、Bearer capability 与 overlay 中携带 secret 的值被 2026-08-28
的授权取代。非持久、generation-bound 的 environment overlay 仍然保留，但仅用于提供 local Unix-
socket path，不携带 credential。必需的端到端标准是：本地 Orchbee Tab 通过 SSH 连接远端，Claude
Code 在远端 tmux 或 Zellij 内运行；即使该 mux client 切换到其他 Pane，running、waiting-for-user、
completion 或 failure 仍会更新每个正确本地 Tab adornment 与符合条件的通知。Unrelated Tab 不能改变。
不声明所有 SSH attachment 断开后的实时交付。

已接受实现刻意分离 semantic authority 与 presentation correlation：

1. Desktop 拥有有界 local Unix-domain observation listener。本地 hook 直接连接。Orchbee shell
   integration 为符合条件、由用户调用的 SSH connection 增加 reverse Unix-socket forwarding option，
   但不保留 credential 或 host record。OpenSSH 对该 transport 鉴权并加密；远端 hook 只发现同用户、
   存活的 forwarded socket，并向每个 candidate 发送已经 sanitization 的 rich envelope。不存在 HTTP、
   TCP port、Bearer header 或继承的 terminal secret。
2. Hook 解析该 mux execution 的 attached outer client TTY。tmux resolver 使用 server-owned
   Pane/session/client identity；Zellij resolver 使用其 client/session identity 与 operating-system TTY
   owner。Resolver 可以遍历 nested mux layer。它直接向每个 resolved outer TTY 写入一个有界 OSC 777
   correlation hint，因此交付不依赖 Agent Pane 被选中，也不依赖 tmux/Zellij passthrough configuration。
3. 每个 hint 只包含 schema version、opaque event identity、一次性 correlation identity、有界 activity
   hint 与 presentation-scoped visibility。匹配的 rich socket envelope 携带同一 identity 与 allowlisted
   semantic fact。Desktop 保存受 size 与 time 限制的 two-half join cache，接受任意到达顺序，对精确 event
   去重，且只在两片匹配后 dispatch rich transition。未匹配或伪造的 OSC half 最多只能产生既有 limited
   activity projection。

当一个 execution 有多个 attached mux client 时，hook 为每个 outer TTY 发出一个 correlation half，
trusted envelope 命名它们全部。Runtime 只应用一次 semantic event，并 attach 每个匹配的本地
presentation；Tab state 会 fan out，而操作系统 notification attempt 仍按 semantic event identity 去重。
关闭或替换一个本地 terminal 只退役对应 attachment generation。后续 SSH attachment 可以建立新
generation，但不能重新指向旧 event。

Remote mux server 可以比全部 SSH client 存活更久，但不会获得持久 Orchbee capability。Hook 为每个
event 重新发现 live forwarded socket 与 attached client TTY。若 forwarding 被禁用、mux/client mapping
含糊、任一 channel 不可用、input malformed、cache 过期或 listener 丢失，hook 会退出而不阻塞 Claude
Code；Orchbee 只显示 limited、stale 或 unknown state。Terminal input/output 与 mux operation 保持独立。

### 已接受的显式 multi-Agent adapter setup

2026-08-29，active requester 以产品与架构 decision owner 身份接受 Claude Code、Codex、Grok、OpenCode、
Pi 与 OMP 的显式 installed-adapter 切片。每台实际运行 Agent 的 execution host 都要安装 adapter，包括位于
SSH、tmux 或 Zellij 后面的远端 host。Orchbee 可以打包 adapter 并展示经评审的安装说明，但不会静默修改
全局 Agent configuration、把 adapter 复制到远端 host，或在没有用户操作时启用它。禁用或卸载 plugin 或
extension 即为 rollback，且每个 callback 都保持 fail-open，因此 observation 不能阻塞 Agent 工作。

Application 拥有一个封闭 adapter registry。Rich envelope 只携带六个固定 adapter ID 之一，绝不携带
provider 自行提供的 capability list。Desktop 把该 ID 映射到 curated provider ID、已接受 normalized event
allowlist 和下表 canonical capability 上限。Unknown adapter ID、不支持的 normalized event、malformed
identity 以及超过该上限的 observation 都会被拒绝。Provider session key、operation key、subagent key 与
external event ID 在进入共享 ingress state 前都按 adapter ID namespace，因而不同 Agent 的相同 upstream
string 不会 alias。

| Adapter ID    | Reference contract                        | 接受的最大 capability                                                                  | Adapter 使用的精确 upstream 证据                                                           |
| ------------- | ----------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `claude-code` | 2026-08-29 评审的 Claude Code Hooks       | `sessionLifecycle`、`turnLifecycle`、`questions`、`approvals`、`activity`、`subagents` | Session、prompt、tool、permission、compaction、subagent、stop、failure 与 session-end hook |
| `codex`       | Codex CLI `0.150.1` hooks/plugin contract | `sessionLifecycle`、`turnLifecycle`、`questions`、`approvals`、`activity`、`subagents` | Session、prompt、tool、permission、compaction、subagent、stop 与 session-end hook          |
| `grok`        | Grok CLI `1.0.5` hooks/plugin contract    | `sessionLifecycle`、`turnLifecycle`、`approvals`、`activity`、`subagents`              | Session、prompt、tool、subagent、stop/failure/cancel 与 typed `Notification` hook          |
| `opencode`    | OpenCode `1.18.25` plugin event contract  | `sessionLifecycle`、`turnLifecycle`、`questions`、`approvals`、`activity`              | `session.*`、`question.*`、`permission.*`、tool 与 error event                             |
| `pi`          | Pi `0.84.4` extension contract            | `sessionLifecycle`、`turnLifecycle`、`questions`、`activity`                           | Session、agent/settled、blocking UI-prompt、turn 与 tool-execution event                   |
| `omp`         | OMP `18.0.10` extension contract          | `sessionLifecycle`、`turnLifecycle`、`questions`、`approvals`、`activity`              | Session、agent/turn、Ask-tool execution、精确 tool-approval 与 tool-execution event        |

Codex 与 Grok 使用经评审的 command-hook plugin；OpenCode 使用其 documented JavaScript plugin event
surface；Pi 与 OMP 使用各自 documented extension API。每个 provider entry point 都先校验和 sanitize 自己
的 raw payload，再调用共享的 nonblocking Unix-socket 与 OSC transport library。Raw prompt、message、tool
argument/result、command、path、error text 与 arbitrary provider metadata 绝不越过该边界。共享 transport
code 不会让一个 provider 的 event name 或 payload interpretation 对另一个 provider 具有权威。

这些 reference version 与已检查 fixture 定义 compatibility claim；upstream branding 或 API 相似性不能
定义它。Pi `0.84.4` 是本切片第一个声明支持的 Pi contract，因为其当前 package 与 settled/UI-prompt
event 不同于旧 package line 和 OMP。Matrix 外版本仍可正常作为 Agent 运行，但在其 adapter fixture 与
capability grant 经过评审前，Orchbee 不声明 rich-state parity。缺少精确 event 的 provider callback 会
降级为其可以证明的 capability；event 缺失绝不表示 idle、success、failure、approval 或 question 已解决。

全部六个 adapter 复用已接受的 local 或 SSH reverse-forwarded Unix socket，加 exact outer-TTY OSC 777
correlation join。它们不增加 ACP、App Server、HTTP listener、TCP port、screen scraping 或 process-name
authority。同一 multi-attachment、hidden mux Pane、generation fencing、deduplication、notification
eligibility 与 disconnected-route 排除项保持不变。

## 呈现与聚合

Presentation resolver 使用以下语义优先级：

```text
open attention > unacknowledged failure > unacknowledged success > active Turn > idle
```

Freshness 是 resolved state 上的覆盖层。Stale working Session 显示为“最后观测时仍在工作，状态已过期”；
它绝不静默转为 idle。当多个状态具有相同最高优先级时，compact surface 显示数量，detail surface 按
stable event identity 从旧到新排列，而不是任意选择一种 request kind。

| Canonical condition                       | Pane 或 panel header                                                  | Tab 与 window                                                           | Activity 界面                                    | 操作系统候选                                     |
| ----------------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------ | ------------------------------------------------ |
| Open question、approval 或 authentication | 持久本地化 Agent chip 或 banner，并提供精确 navigation                | 标准 attention overlay 与 unresolved count；可选 window attention count | 固定在 Needs attention，直到解决                 | 精确 location 不可见时允许                       |
| Unacknowledged failed Turn                | Failure label，不包含 raw provider text；terminal 保持可用            | 标准 error overlay                                                      | 持久 recent failure，直到 acknowledgement 或过期 | 不可见时允许                                     |
| Unacknowledged successful Turn            | 有界 completed label                                                  | 标准 completed 或 unread overlay                                        | Recent completion                                | 没有 current attachment 可见时允许               |
| Active Turn                               | Provider identity 加 working、waiting-external 或有界 activity detail | 标准 active overlay 与 active count                                     | Running                                          | 不允许                                           |
| 肯定性 idle                               | Provider identity 加 Ready                                            | Neutral Agent icon                                                      | 没有 pending event 的 active Session             | 不允许                                           |
| Unknown、stale 或 disconnected            | 显式 limited、stale 或 disconnected label，不做猜测                   | Neutral health overlay                                                  | Diagnostic 与最后一次可靠观测                    | 仅在先前 active 且经过已接受 grace policy 后允许 |
| Ended Session                             | Location projection 退役后不再显示 active status                      | 不进入 active aggregate                                                 | 仅作为有界 recent event 保留                     | 不增加候选                                       |

- Panel-header 左侧区域是 compact provider 与 status 的首选组合点。中间 custom、OSC 或 CWD-derived
  title 保持稳定，不变成 status ticker。
- Tab aggregation 覆盖该 Tab 中每个 Panel 和 child terminal 绑定的所有 Agent Session。它显示最高
  semantic priority 与数量，而不是只显示 focused Pane。
- Remote mux execution 通过匹配的本地 presentation attachment 投影。把 remote client 切换离开 Agent
  Pane 只改变 attachment visibility，不改变 attachment identity；因此本地 Tab icon 会继续显示 running、
  attention、completion 或 failure。
- Curated provider presentation 标识 Claude Code、Codex、Grok、OpenCode、Pi 或 OMP，并对未知
  provenance 回退为 generic Agent。独立的标准 overlay 在不同 provider 之间一致表达 attention、
  failure、completion、work 或 degraded health。
- `loading`、`crashed` 等现有 generic Tab field 不能被重新解释为 Agent work 或 Turn failure。经过
  评审的 status-adornment composition 应承载更丰富的投影。
- Activity 界面按 Needs attention、Running 与 Recent 分组。操作系统通知被关闭或无法导航时，它是
  持久的产品内恢复路径。
- Unified Search 消费 [unified-search 提案](../workbench/unified-search-surface.zh-CN.md) 已经预留的
  Agent-owned searchable read model 与 prepared navigation Action；search 绝不自动启动或提交 Agent
  work。
- 每种 status 都有 visible text 或 accessible name，绝不只用颜色。Active animation 遵守 reduced
  motion，全部产品文案均在 `en-US` 与 `zh-CN` 中本地化。

## 通知策略

Adapter 永不直接交付通知。确定性 Agent transition 返回 semantic event；应用策略随后可以产生
device-scoped notification command。

| Transition                               | 资格                                                                           |
| ---------------------------------------- | ------------------------------------------------------------------------------ |
| New Attention Request                    | Fresh exact Session、已声明 attention capability、没有 current attachment 可见 |
| Failed Turn                              | Fresh exact semantic outcome、没有 current attachment 可见                     |
| Successful Turn                          | Fresh exact semantic outcome、没有 current attachment 可见                     |
| 先前 active 时 observation lost          | 经过已接受 grace period 仍未恢复；短暂 flapping 不通知                         |
| Activity、retry、tool 或 progress update | 永不通知                                                                       |
| Idle、acknowledgement 或 duplicate event | 永不通知                                                                       |

- 产品内状态始终可用，不需要 notification permission。本切片绝不自动请求 permission：操作系统交付要求
  Agent notification policy 已启用且已有 platform permission。即使外层 Orchbee Tab 被选中，mux Agent
  Pane 被报告为 hidden 时仍视为不可见；任一 current attachment 可见都会抑制交付。
- 一个 semantic event ID 在每个 device policy generation 中至多产生一次 delivery attempt。操作系统
  API 结果不明确时记录该结果且不自动重试；本提案不声称操作系统实际显示了通知。
- 跨设备交付可能重复。第一阶段只声明 per-device deduplication；shared request resolution 保持
  canonical，event acknowledgement 与 delivery receipt 保持 client 或 device scoped。
- Notification content 只包含 curated provider name 与本地化状态，例如“Codex 需要你的批准”或
  “Agent 任务已完成”。它排除 prompt、question、command、tool、output、transcript、directory、
  project、environment、user 与 provider error text。
- 激活通知会导航到确定性的 current attachment，优先选择 active application 中 hidden 且 navigable 的
  presentation；若不存在则打开 Activity 界面。它绝不向 terminal 输入、回答问题、批准 permission 或
  重放 stale action。
- Notification failure 不改变 Agent outcome，也不抑制产品内事件。

## 后台 transport 与恢复

- Terminal React surface 与 inactive Tab 卸载时，应用级 Agent runtime 继续订阅。隐藏 renderer
  instance 不是监控机制。
- 初始本地部署可以让 Agent runtime 与 Desktop 共址，但逻辑 ownership 与 serialized contract
  不能依赖 Electron 或 React。独立 supervised process 是后续拓扑决策。
- Desktop 在其 private runtime directory 中创建一个受 mode 限制的 Unix socket，并在 shutdown 时删除。
  本地 Agent hook 只获得该非 secret socket path。符合条件的 interactive SSH invocation 会增加一个随机、
  同用户的 remote socket forwarding 指向 local listener；shell integration 保留用户的 executable、
  argument、authentication、host policy 与 exit status。
- Remote hook 不依赖长生命周期 mux server 继承的 environment。每个 event 都重新发现 live same-user
  forwarded socket 与 attached outer client TTY，发出有界 correlation half，并通过 candidate socket 发送
  同一份 sanitized rich event。
- Terminal Host 会按 terminal sequence 把已准入 OSC correlation half 保留在一个 64-entry FIFO 中。
  Reader rollback 不发布任何内容，outgoing backpressure 保留精确 head，FIFO 已满时只丢弃新的不可信
  hint；因此快速 lifecycle event 不能用后来的 correlation 覆盖早先已准入的 correlation。
- Desktop 在受限 pending map 中按精确 event 与一次性 correlation identity 拼接 correlation 与 rich half。
  Join entry 会过期，已接受 event identity 会去重，presentation generation 会 fence terminal reuse。两个
  pending map 都不是 canonical Agent state。
- Browser 与未来 mobile client 通过 lightweight application subscription 消费已授权 snapshot 与
  semantic event。它们不挂载隐藏 terminal renderer 来观测状态。
- Renderer restart 会保留 runtime observation 并获取 fresh snapshot。第一阶段不承诺 Agent state 在
  Desktop、monitor、Terminal Host 或操作系统 restart 后保持连续；发生此类丢失后，在 provider 提供
  current evidence 前报告 stale 或 unknown。
- Unix-socket envelope 与 OSC correlation hint 使用精确 version、有界 payload、严格 boundary parsing、
  event identity、generation 与 sequence。它们绝不运输 prompt、transcript、command、tool content、
  filesystem path 或 provider error text。
- 没有 live Orchbee SSH attachment 的 remote mux 不能产生实时 local presentation update。重新 attach 会建立
  新 correlation 与 presentation generation；不会 replay 或推断遗漏 semantic event。Offline relay 与
  durable remote history 仍为保留决策。
- Agent runtime 或任一 adapter 失败时，terminal input、output、lifecycle 与 close behavior 继续独立工作。

## 安全与隐私

- 在第一个 trusted boundary 解析每个 provider、process、terminal、storage、IPC 与 network payload，
  并使用显式 size、string、collection、identity 与 version limit。
- 对 canonical metadata 使用 allowlist。不能仅仅因为 upstream hook 提供 provider transcript path、
  prompt、question text、command、tool input/output、environment 或 arbitrary provider message 就摄取它。
- Local 与 remote Unix socket 是 user-private、ephemeral observation endpoint，而不是 general Desktop
  capability。它们只接受有界 observation schema，不授予 input、filesystem、command、response、
  notification 或 native-decision authority。Socket path 与 correlation identity 不进入 product diagnostic、
  telemetry 或 notification payload。
- 把 terminal escape sequence 与 screen content 当作 attacker-controlled display input。它们不能创建
  actionable request、native action 或 trusted notification。Rich observation 还要求 matching envelope
  通过 user-private local socket 或其 SSH-authenticated reverse forwarding 到达。
- Trust boundary 假设 local desktop user 与已鉴权 remote account 不会攻击自己。Same-user process 可以访问
  该用户的 PTY 与 forwarded socket；本切片限制 blast radius 与 content，但不会 sandbox 共用一个操作系统
  account 的互不信任进程。
- 第一阶段只能导航。Inline Answer 或 Approve 需要单独接受的 provider-native request identity、
  authorization 与 revalidation model、cancellation behavior、audit boundary，以及 stale/replayed decision
  防护。模拟按键不是 action channel。
- Hook setup 与 failure 可观测且可恢复。Orchbee 不可用时不能阻塞、拒绝、批准或改变 provider 的
  Agent operation。

## 声明与证据

| 声明     | 可观测规则                                                                                                                                      | 必需证据                                                                                                              | 限制                                                                |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `AR-001` | User-facing status 绝不超出 adapter 声明的 capability 或 freshness                                                                              | Adapter schema fixture、reducer unit test 与 projection decision table                                                | 不证明 upstream provider 报告的事实真实                             |
| `AR-002` | Duplicate、stale-generation、mismatched 与 reordered event 不能改变另一个 Session、Turn、request、execution location 或 presentation attachment | Deterministic reducer test、可 replay 的 stateful property test 与 `AgentRuntime` TLA+ safety check                   | Model checking 保持有界                                             |
| `AR-003` | Attention 持续 open，直到精确 resolution、cancellation 或 Session end；client acknowledgement 不能解决它                                        | Transition test、生成的 request sequence 与 TLA+ request-lifecycle invariant                                          | Native provider response 不属于第一阶段                             |
| `AR-004` | Pane、Tab、Activity、Search 与 notification candidate 从一个 canonical snapshot 和显式 client view 解析出相同 semantic priority                 | Pure projection test、aggregation property test、accessibility test 与 deterministic application trace                | Pixel appearance 与真实 OS delivery 使用单独证据                    |
| `AR-005` | Inactive terminal surface 被卸载时，已准入 runtime Agent event 仍能更新 product state                                                           | Target Tab 已卸载、runtime event injection、snapshot publication 与 remount 的 controlled integration trace           | 不声称 application 或 OS restart 后连续                             |
| `AR-006` | Notification attempt 经过 visibility 与 preference gate，按设备去重，可导航且不含禁止内容                                                       | Policy boundary test、payload allowlist test、failure injection 与 platform-capability call trace                     | 不声称操作系统实际显示或交付                                        |
| `AR-007` | Terminal 与 heuristic input 不能回答、批准、提交或获得 native decision capability                                                               | Trust-boundary negative fixture、capability test 与 implementation security review                                    | Trusted application runtime 被攻破不属于此边界                      |
| `AR-008` | Agent 观测缺失、禁用、过期、不支持或失败时，不能损害 terminal operation，也不能制造 terminal 或 Agent lifecycle outcome                         | Adapter failure test 与 application integration trace                                                                 | Observed signal 之外的 provider-side failure 可能保持 unknown       |
| `AR-009` | 被其他 tmux 或 Zellij Pane 隐藏的 remote Agent 更新每个 matched local Tab、不更新 unrelated Tab，且每个 semantic event 最多尝试一次通知         | SSH/mux resolver fixture、two-half ingress race test、multi-attachment projection test 与 controlled end-to-end trace | 需要至少一个 live Orchbee SSH attachment；不证明任意第三方 mux 版本 |

选定的 correctness evidence：

- Deterministic reducer test 在 UI integration 前覆盖每个 state、event、guard、result、cancellation、
  no-op 与 generation boundary。
- Stateful property test 生成多个 Session、parent-child relationship、Turn、request、execution location、
  presentation attachment、duplicate event、reordered join half、disconnection、reconnection、
  cancellation 与 acknowledgement sequence，具有 stable seed、shrinking、replay 与 coverage class。
- TLA+ 检查最小异步模型，该模型至少包含两个 Session、两个 presentation attachment、两个 observation
  generation，以及相互竞争的 join、detach、request、completion、acknowledgement 与 publication event。
  它检查 ownership、generation fencing、request persistence、one-active-Turn、attachment fan-out 与
  notification-candidate safety；不证明无界 execution 或 OS delivery。
- Adapter conformance test 覆盖 command-hook sanitizer 的 strict allowlist、forbidden-content stripping
  与 nonblocking failure；Unix-socket permission、body bound、schema、SSH reverse-forwarding argument、
  tmux 与 Zellij resolver fixture、两种 join arrival order、expiry、location correlation 与 retirement；
  以及 terminal signal 的 strict limited-activity ceiling。更新 upstream adapter 需要完成 fixture 与
  capability audit；Orchbee 不声明完整 upstream UI 或 protocol parity。
- Deterministic application trace 覆盖 active-visible suppression、inactive-unmounted attention、remote
  hidden-pane running、attention、completion 与 failure、多个 mux client、unrelated Tab、stale
  observation、navigation、missing target、localization、accessibility 与 reduced motion。
- 不选择 Lean，因为第一阶段除了由 stateful test 和 TLA+ 覆盖的 reducer 与 projection invariant 外，
  没有关键 universal pure transformation。Warp、Ghostty、Claude Code 与 Codex 都不是 Orchbee 产品策略的
  differential reference implementation。
- Native 操作系统显示、notification-center persistence 与 focus transfer 仍属于 platform integration
  limit。除非 human 在当前请求中明确授权该精确测试，否则继续排除 manual-only native-window focus test。

## 交付与回滚

1. 已完成：接受第一阶段授权；剩余决策继续阻塞其首个受影响切片。
2. 已完成：使用已接受模型、provider-neutral read model、deterministic reducer、projection policy、
   property evidence、TLA+ model、project contract 与所属双语 reference 创建 `@orchbee/agent`。
3. 已完成：Desktop 拥有 volatile application runtime、精确 snapshot subscription、renderer-restart
   recovery、terminal signal transport 与 failure isolation。Loopback HTTP 与 Bearer ingress 已移除；
   非持久 generation overlay 只携带 Unix-socket path，且未削弱 terminal launch 或 recovery compatibility。
4. 已完成：canonical 模型把一个 opaque execution location 与多个受 generation fencing 的 presentation
   attachment 分离，并在 application consumer 前具备 reducer、projection、property 与 TLA+ 证据。
5. 六个显式 adapter 均已完成：user-private Unix-socket listener、managed SSH reverse forwarding、
   严格 two-half join、tmux 与 Zellij outer-TTY resolution、nested-mux traversal、expiry、
   deduplication、封闭 capability registry、provider-specific sanitization 与 nonblocking failure
   path 均已实现。
6. 已完成：同一 semantic priority 会投影到精确本地 Panel 与 Tab adornment；attention、success 与
   failure 可以产生 permission-gated、hidden-attachment notification attempt。Activity 与 Unified
   Search 仍是后续分别治理的界面。
7. 只有作为分别授权的 consequential 切片，才考虑 provider-native Answer、Approve、resume、durable
   history、offline remote relay、mobile push 与 cross-device delivery。

每个切片都可以通过删除其 consumer 与 adapter registration 回滚，同时保留 terminal。Wire-contract
rollback 遵守其精确 negotiated compatibility policy；任何切片都不能依赖静默接受 unknown schema。
由于本提案不授权 durable Agent content 或 history，第一阶段不需要 data migration。

## 已接受的第一阶段授权

2026-08-27，active requester 以 Orchbee 产品与架构 decision owner 身份明确一起接受了以下决策：

1. Agent capability 是 Orchbee observed Agent Session、Turn、Attention、Outcome、hierarchy、
   provenance、aggregation 与 provider-neutral read model 的唯一 owner。
2. `@orchbee/agent` 随第一个 deterministic domain 切片和真实 application consumer 创建；不投机创建
   空 protocol 或 adapter package。
3. 应用运行时拥有 always-on local observation，初始可以与 Desktop 共址。Workbench 与 Terminal Host
   保持 non-owner，只消费或运输各自的有界 contract。
4. Canonical 模型使用相互正交的 lifecycle、Turn、Attention、Outcome 与 observation-quality 维度；
   扁平 status enum 或 presentation boolean 都不能成为领域权威。
5. 第一阶段通过声明 capability 的 observation adapter 支持 terminal-bound Claude Code 与 Codex，
   并提供 generic limited-status fallback。Screen text 与 process heuristic 永远不是 semantic authority。
6. 第一阶段只能导航。Notification、terminal signal、hook event 与 UI surface 都不能 answer、approve、
   submit、type 或调用 provider-native decision。
7. Pane header、Tab aggregation、Activity 与 Unified Search 消费同一个 Agent-owned projection；provider
   identity 与 semantic status 在视觉和语义上保持分离。
8. 产品内 attention 与 outcome 始终可用。操作系统 effect 需要显式 permission 与 preference、精确
   visibility 与 event gating、安全本地化 metadata 和 per-device deduplication。
9. 第一阶段在 renderer unmount 与 restart 后保持 observation，但不声明 Desktop、monitor、Terminal Host
   或操作系统 restart 后连续。它不存储 prompt、transcript、command、tool content 或 durable Agent history。
10. 完成前，必须在各自所述边界内提供 deterministic reducer、property、TLA+、adapter、application
    trace、accessibility、privacy 与 failure-isolation evidence。

## 已接受的 SSH 与 mux 拓扑授权

2026-08-28，同一 active requester 以 Orchbee 产品与架构 decision owner 身份明确接受了这一额外
Consequential 与 Modeled 切片：

1. 必需场景是一个本地 Orchbee Tab 通过 SSH 连接远端，Agent 位于 tmux 或 Zellij 内。把 mux client
   切换到另一个 Pane 不能阻止 running、waiting-for-user、success 或 failure 更新正确本地 Tab icon 与
   符合条件的通知。
2. 业务状态保持 provider-neutral 与 topology-neutral。领域区分 opaque execution location 与零个或多个
   local presentation attachment；provider、SSH、tmux 与 Zellij detail 留在 adapter 和 ingress diagnostic。
3. Rich allowlisted observation 使用 application-owned local Unix socket；本地 hook 直接连接，remote
   hook 通过 OpenSSH reverse Unix-socket forwarding 连接。移除 loopback HTTP、Bearer credential 与继承的
   generation secret。
4. OSC 777 仍是 limited、attacker-controlled terminal channel。它可以把有界一次性 correlation half
   带到精确 outer PTY，但只有 matching trusted socket half 才能授权 rich Agent transition 或 notification
   candidate。
5. Managed hook 为每个 event 解析 attached tmux 与 Zellij client TTY，并可遍历 nested mux layer。它不能
   要求 Agent Pane active、用户 passthrough setting，或 remote mux environment 中持久化 capability。
6. 一个 semantic event 可以 attach 到多个正确本地 presentation，但只 reduce 一次，且最多产生一次 device
   notification attempt。Unrelated Tab 绝不更新；退役一个 presentation 绝不退役另一个 current attachment。
7. 本切片绝不请求 notification permission。仍需要既有显式 preference 与 platform permission gate；当
   没有 current attachment 可见时，attention、success 与 failure 符合条件，包括 Agent Pane 隐藏在已选中
   local Tab 内的情况。
8. Rollback 会删除 SSH augmentation 与 rich socket consumer，同时保留普通 SSH、terminal operation 与
   limited OSC hint。不包括所有 SSH attachment 断开时的实时行为、offline replay 与任意第三方 mux version
   的精确 parity。

## 已接受的显式 multi-Agent adapter 授权

2026-08-29，同一 active requester 以 Orchbee 产品与架构 decision owner 身份明确接受了这一
Consequential 与 Modeled 切片：

1. Claude Code、Codex、Grok、OpenCode、Pi 与 OMP 获得显式 provider-bound adapter，而
   `@orchbee/agent`、Workbench projection、Tab aggregation 与 notification policy 保持完全
   provider-neutral。
2. Adapter 由用户安装在 Agent execution host。Orchbee 不静默编辑全局 configuration、不 bootstrap
   远端 machine，也不让 observation 成为 Agent operation 的前置条件；禁用或卸载即为 rollback。
3. Provider-specific code 在使用一个共享 nonblocking UDS 与 OSC transport 前校验并移除 content。已接受
   SSH reverse forwarding 与 tmux/Zellij outer-TTY correlation 适用于全部六个 adapter，不增加 HTTP、
   ACP、App Server、screen-parsing 或 process-heuristic path。
4. 由封闭 Desktop registry 而不是 adapter input 授予 provider identity、已接受 event kind 与最大
   capability。Unknown adapter 与 over-capability event 会被拒绝，所有 external identity 在共享
   correlation 或领域 reduction 前都按 adapter namespace。
5. Compatibility 是针对固定 upstream version 与 deterministic sanitized fixture 的 reference refinement。
   新 upstream version 在 fixture 与 grant 经过评审前不会扩大 capability；unsupported version 可以正常
   运行，但不保证 rich-state parity。
6. 本切片保持 observation-only 与 navigate-only。即便 upstream extension API 允许，adapter 也不能
   answer、approve、submit、resume、control 或持久化 Agent content。

## 已接受的显式本地集成管理授权

2026-08-29，同一 active requester 以 Orchbee 产品与架构 decision owner 身份明确接受了这一额外的
Consequential 与 Modeled 切片：

1. Orchbee 可以在不改变本地 Agent executable 与集成状态的前提下检查它们，但绝不自动安装、启用、
   关闭、更新或修复 adapter。打开 Settings 与应用启动均为只读。
2. Settings 中的一次显式用户操作可以执行 provider 官方的本地 plugin command，或创建、移除一个
   精确的 Orchbee-owned 集成条目。操作绝不能代替用户回答交互式 trust prompt、覆盖冲突文件，或
   改写无关的用户 configuration。
3. 本地操作串行执行，并遵循 inspect、preflight、mutate 与 verify。提交前失败保留先前状态。结果
   不明确或提交后失败会触发只读 reconciliation，在观察到结果状态前不能宣称成功或 rollback。
4. 关闭仅移除或禁用精确的 Orchbee-owned plugin、extension 或 symlink，并保留其他 plugin、用户
   settings、marketplace、项目 configuration 与 Agent data。
5. Provider-specific 检查与变更保持在一个 provider-neutral Desktop capability 之后。Settings 只消费
   有界集成状态与 failure code，不依据 CLI output、filesystem layout、Shell 或 provider configuration
   syntax 分支。
6. Remote host 始终只能手动配置。Orchbee 可以展示经过审阅、由用户在 execution host 运行的命令，
   但绝不复制 adapter、调用 remote command、编辑 remote file，也不把本地已启用当作 remote 集成
   已启用的证据。
7. 只有只读检查发现本地 Agent 可用但缺少 Orchbee 集成时，才可以显示可关闭、device-local 的提醒。
   关闭按该集成划分，提醒链接到 Settings；adapter 启用状态仍与 Agent notification preference 和
   operating-system notification permission 相互独立。
8. 此切片不增加 background updater，也不在已审阅的 fixture matrix 之外作 compatibility claim。
   后续随应用捆绑的 adapter 只能通过另一次显式用户操作提供。

## 保留决策

已接受的第一阶段领域切片不会决定：

- Runtime 保持与 Desktop 共址，还是成为单独 supervised Agent process；
- 任何未来 cross-process Agent-runtime protocol 命名、process extraction、version negotiation，以及
  application 或 operating-system restart 后 retention；Desktop Unix-socket ingress、terminal-signal
  ingress 与 renderer snapshot protocol 是各自更窄边界的当前权威；
- 自动 local 或 remote adapter deployment、任何由 Orchbee 发起的 remote configuration change、静默
  local configuration edit、background adapter updater，以及 pinned provider fixture matrix 之外的
  compatibility claim；
- 除既有显式 opt-in 外的 notification default、stale grace period、recent-event bound、per-client
  acknowledgement persistence 与 cross-device suppression；
- 最终 Activity 界面组合、window 或 Dock badge policy，以及 experience-component reuse 或 admission；
- Provider-native question 与 approval payload、response authorization、audit、cancellation、replay
  protection 与 inline interaction；
- Offline remote relay、所有 SSH attachment 断开后的 replay、durable remote identity、reconnect recovery、
  non-OpenSSH transport 与 unsupported mux-version compatibility；
- Durable Agent history、prompt 或 transcript content、search indexing、telemetry、retention、deletion、
  export 或 synchronization；
- Native Codex App Server ownership、Agent launching、composer、submission、orchestration、resume、
  scheduling 与 external cost policy；以及
- Mobile push、multiple user、account-level acknowledgement 与 exactly-once cross-device delivery。

实现遇到受这些决策影响的最早切片时必须停止，直到所需 decision owner 接受相应的 alternatives、
consequences、evidence limit、rollout 与 recovery。
