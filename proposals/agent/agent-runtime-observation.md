# Agent Runtime Observation and Attention

| Field             | Value                                                                                                                                                                                                                                                                                                                                |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Status            | Phase-one core, local observation, the accepted SSH, tmux, and Zellij topology, the explicit multi-Agent adapters, and explicit local integration management for Claude Code, Codex, Grok, OpenCode, Pi, and OMP are authorized; later reserved decisions remain unresolved                                                          |
| Decision owner    | The active requester acting as Orchbee product and architecture decision owner for the accepted phase-one authorization; each reserved decision still requires explicit acceptance by its required owner                                                                                                                             |
| Semantic owners   | Agent capability, application runtime, Workbench application composition, and platform notification adapters                                                                                                                                                                                                                         |
| Current authority | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md), [`@orchbee/agent`](../../packages/agent/README.md), [`@orchbee/terminal-protocol`](../../packages/terminal-protocol/README.md), [`@orchbee/workbench`](../../packages/workbench/README.md), and the [unified-search contract](../../packages/workbench/docs/unified-search.md) |
| Exit condition    | Implement separately accepted slices, move maintained state, adapter, presentation, notification, and evidence contracts into their owning package and application guides and tests, resolve or remove every remaining future decision, then delete this proposal and its translation                                                |

Simplified Chinese review translation:
[agent-runtime-observation.zh-CN.md](agent-runtime-observation.zh-CN.md). English remains
canonical.

## Change authorization

| Field             | Proposed authorization                                                                                                                                                                                                                                                                                                                                                                                   |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tier              | Modeled because the feature owns asynchronous state, ordering, hierarchy, cancellation, and multi-client projections; Consequential because it introduces cross-workspace contracts, consumes untrusted process events, and can cause operating-system notifications                                                                                                                                     |
| Problem           | A user can leave a terminal-backed Agent running without Orchbee reliably distinguishing work, idle time, a question, an approval request, completion, failure, or lost observation. Inactive terminal surfaces can be unmounted, so renderer-local observation cannot provide complete background attention or completion feedback.                                                                     |
| Phase-one outcome | Observe terminal-bound Claude Code, Codex, Grok, OpenCode, Pi, and OMP sessions through explicitly installed, capability-bounded adapters, including a local Orchbee tab connected by SSH to a remote tmux or Zellij client; maintain one canonical observed-session model outside renderer lifetime; and project truthful state into every exact attached presentation, Tab, and eligible notification. |
| Non-goals         | Launching Agents, owning prompts or submission, orchestrating work, parsing terminal text as semantic authority, responding to questions or approvals inline, modifying PTY or Agent process authority, durable transcript or prompt history, real-time delivery after every SSH attachment is disconnected, mobile push, or cross-device exactly-once notification delivery                             |
| Compatibility     | Existing terminals, shell integration, command-completion reminders, OSC title, progress, close activity, and terminal lifecycle continue unchanged when Agent observation is absent, disabled, unsupported, stale, or failed.                                                                                                                                                                           |
| Delivery          | Each accepted slice has one owner and evidence boundary. The domain model precedes adapters, transports, UI, and notification effects.                                                                                                                                                                                                                                                                   |
| Recovery          | Disable or remove an Agent adapter without stopping its terminal. Adapter or monitor failure degrades the displayed observation to stale or unknown and never invents idle, success, approval, or process termination.                                                                                                                                                                                   |

Current terminal progress, close activity, and command completion are bounded presentation facts,
not Agent lifecycle authority. They remain useful only as low-fidelity fallback evidence. The
current all-tab monitoring limitation is documented by the
[shell-integration contract](../../apps/web/docs/terminal-settings.md#shell-integration-and-completion-reminders).

## Accepted architecture

| Alternative                            | Consequence                                                                                                   | Decision |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------- | -------- |
| One flat `AgentStatus` enum            | Conflates session lifetime, turn activity, pending requests, outcomes, and observation health                 | Reject   |
| Renderer or Workbench owns observation | Misses unmounted tabs, couples framework lifetime to Agent state, and gives Workbench orchestration authority | Reject   |
| Terminal Host owns Agent semantics     | Pollutes PTY, process, Ghostty, and terminal-protocol authority with provider-specific product behavior       | Reject   |
| Agent capability with typed adapters   | Preserves one canonical model, explicit trust and fidelity, background monitoring, and reusable projections   | Accepted |

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

| Owner                         | Responsibility                                                                                                                                                                                                                                                                                                         |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Agent capability              | Canonical observed Session, Turn, Attention Request, Outcome, hierarchy, observation provenance, deterministic transition model, aggregation, and provider-neutral read models                                                                                                                                         |
| Provider adapter              | Validate one external source, correlate its fixed adapter identity to one Agent Session and opaque execution location, and translate supported facts into canonical events; the application-owned registry grants capabilities, and the adapter owns neither capability policy nor presentation or notification policy |
| Application runtime           | Own adapter lifetime, local and reverse-forwarded ingress, bounded two-channel correlation, presentation attachment generations, event ordering at ingestion, subscriptions independent of renderer mounting, device notification policy and attempt records, and platform-effect composition                          |
| Workbench and application UI  | Consume Agent-owned read models and navigation Actions, compose status into existing screen regions, and own no Agent transition or provider payload                                                                                                                                                                   |
| Terminal Host                 | Continue to own PTY, process, Ghostty state, terminal history, attachments, and terminal transport; it does not load Agent providers or decide Agent state                                                                                                                                                             |
| Client view                   | Own presentation-scoped visibility, focus, attachment generation, and event acknowledgement without resolving shared Agent requests                                                                                                                                                                                    |
| Platform notification adapter | Attempt one already-authorized, localized, bounded notification effect and report delivery API outcomes without changing Agent state                                                                                                                                                                                   |

The first accepted implementation slice should create `@orchbee/agent` only when its deterministic
model has a real application consumer. A serialized Agent protocol should be created only with its
first real cross-process producer and consumer; its schema may represent validated wire data but
must not own transition legality or product policy. The accepted SSH slice keeps this runtime
Desktop-co-located and uses an application-owned local Unix socket; later process extraction and
protocol-package naming remain reserved decisions.

## Inputs and observation quality

| Input                                       | Admission and meaning                                                                                                                                                 | Absence or failure                                                                           |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Managed native provider event               | Parse at the application trust boundary; accept only registry-granted provider capabilities and exact correlated identities                                           | Preserve the last observation and move freshness to stale after the accepted policy boundary |
| Managed provider-hook event                 | Parse a bounded allowlist before transport; admit rich facts only from the local Unix socket or its authenticated SSH reverse forwarding                              | The hook must not block the Agent; missing events never imply idle or completion             |
| Application presentation lifecycle          | Supplies an exact local presentation and binding generation; one execution may attach to several presentations and each may retire independently                      | Retire only that attachment; disconnect observation only when no trusted route remains       |
| Structured terminal in-band signal          | Treat ordinary OSC as an untrusted display hint; an exact correlation token is useful only when joined with its matching trusted rich event                           | No rich semantic transition from this input alone                                            |
| Process-name, activity, or screen heuristic | May detect a generic provider identity or recent terminal activity only; it cannot assert a question, approval, idle state, successful outcome, or actionable request | Display generic unknown status or no Agent status                                            |
| Authenticated client visibility and receipt | Supplies exact presentation, hidden/visible state, attachment generation, and acknowledged semantic event identity                                                    | Does not change canonical Agent state                                                        |
| Normalized freshness tick                   | The runtime converts monotonic time policy into an explicit stale event; the reducer never reads a clock                                                              | Time passing alone is not a reducer transition                                               |

Observation quality has independent dimensions:

| Dimension   | Values                                                         | Meaning                                                                                                   |
| ----------- | -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Source      | `native`, `hook`, `terminal`, `heuristic`                      | Mechanism that produced the fact                                                                          |
| Fidelity    | `rich`, `basic`, `heuristic`                                   | Semantic distinctions the adapter can support                                                             |
| Freshness   | `fresh`, `stale`, `disconnected`                               | Whether the runtime can still support the last observed state                                             |
| Capability  | Named flags such as `turnLifecycle`, `questions`, `approvals`  | Grants the application registry may make; adapter input, source, or branding never grants a capability    |
| Action path | `navigate-only` or, after separate acceptance, `native-action` | Whether Orchbee can only reveal the provider terminal or can submit a correlated provider-native decision |

Fidelity and trust are not confidence percentages. A richer source may assert more named facts; a
terminal or heuristic source cannot become actionable because its output happens to resemble a
known prompt.

## Canonical data model

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

- Orchbee identities are opaque and stable inside their declared lifecycle. Provider identifiers
  are provider-scoped correlation data and never become global identity by string equality.
- `AgentExecutionLocation` is an opaque provider-neutral execution anchor. It does not expose SSH,
  tmux, Zellij, host, pane, path, process, or provider details to business policy.
- A Session has zero or more generation-fenced `AgentPresentationAttachment` records. The same
  remote execution can therefore update all local Orchbee presentations that are actually attached,
  while unrelated Tabs remain untouched. Reusing a pane, terminal path, process ID, or provider
  Session ID cannot retarget an old event.
- Attachment visibility is presentation-scoped. A hidden tmux or Zellij Agent pane remains attached
  and navigable even when another inner pane is active; that hidden state can make attention,
  completion, or failure eligible for a notification without inventing Agent semantics.
- Sequential Agents in one terminal receive different Session identities. A parent and child
  Agent receive different Session identities and one explicit acyclic parent relationship.
- One Session has at most one directly active Turn. Parallel subagents are child Sessions rather
  than multiple direct Turns hidden inside one Session.
- `idle` is an affirmative adapter observation. Lack of an active Turn from a basic or heuristic
  source is `unknown`, not `idle`.
- An Attention Request is durable until a correlated resolution, cancellation, or Session end. It
  is not represented by a transient status boolean.
- An Outcome is a semantic event and recent fact, not Session lifecycle. A failed Turn may be
  followed by another Turn in the same live Session.
- The runtime retains a bounded set of recent canonical Session and event records for the Activity
  projection. Exact bounds, acknowledgement persistence, and restart retention remain reserved;
  retained records contain no additional provider content.
- `starting` is not phase-one Session authority because this proposal does not own Agent launch. A
  reliably observed queued Turn may be displayed as queued; pre-observation launch state belongs to
  a later launch owner.
- Provider prompt text, command text, tool inputs and outputs, transcripts, filesystem paths, and
  environment values are absent from the phase-one canonical model.

## Transition model

| Event                           | Guard                                                                                                | Canonical result and commands                                                                                                                                                    |
| ------------------------------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `SessionObserved`               | New Orchbee Session ID, admitted provider, opaque execution location, zero or more valid attachments | Create a live Session with `turn: unknown`, empty pending requests, no outcome, exact provenance, and generation-fenced attachments; publish one snapshot                        |
| `TurnStarted`                   | Live Session, current observation generation, no different active Turn                               | Set the exact Turn to `queued` or `working`; a duplicate exact event is a no-op; a different active Turn requires an explicit prior interruption                                 |
| `TurnActivityChanged`           | Exact active Turn and declared activity capability                                                   | Replace only activity or `waiting-external`; activity changes never create notification candidates                                                                               |
| `AttentionRequested`            | Live Session, unique request identity, declared attention capability, matching optional Turn         | Add one pending request and publish one attention semantic event; do not infer that all work or child Sessions stopped                                                           |
| `AttentionResolved`             | Exact open request and current generation                                                            | Remove the pending request and publish one resolution event; client acknowledgement alone never resolves it                                                                      |
| `TurnFinished`                  | Exact active Turn, declared outcome capability                                                       | Record one outcome, move Turn to affirmative `idle`, cancel unresolved Turn-scoped requests with an explicit reason, and publish one outcome event and eligible effect candidate |
| `ObservationBecameStale`        | Live or ended retained Session, current generation                                                   | Retain the last Turn, requests, and outcome; change only freshness and publish a degraded snapshot                                                                               |
| `ObservationReconnected`        | Higher generation plus a validated current snapshot or explicit current event                        | Adopt only the supplied facts; reject older-generation events and never reconstruct missed success or resolution from absence                                                    |
| `PresentationAttached`          | Existing execution location, exact presentation generation, no conflicting current attachment        | Add or refresh that presentation only; do not duplicate Session semantics or notification events                                                                                 |
| `PresentationVisibilityChanged` | Exact current attachment generation and authenticated client visibility                              | Change only that attachment's `visible`, `hidden`, or `unknown` view; do not change Agent state                                                                                  |
| `PresentationDetached`          | Exact current attachment generation                                                                  | Retire navigation for that presentation only; mark observation disconnected only when no trusted route or navigable attachment remains; never infer Session end or Turn success  |
| `SessionEnded`                  | Live Session, current generation                                                                     | End the Session, interrupt any still-active Turn unless an exact outcome already exists, cancel open requests, and publish one Session-end semantic event                        |
| `ClientAcknowledged`            | Authenticated client and retained semantic event                                                     | Update only that client view; do not change Session, Turn, request, or outcome authority                                                                                         |

Malformed, duplicate, stale-generation, out-of-order, unsupported-capability, and mismatched
identity events produce a bounded diagnostic and no canonical mutation. Adapters must translate a
provider-specific supersession into an explicit interruption; the reducer never silently overwrites
one active Turn with another.

## Invariants

- Every mutable Agent fact has one Agent capability owner; adapters and presentations cannot write
  canonical state directly.
- Every accepted event belongs to one Session generation and one monotonically ordered ingestion
  sequence; an older generation cannot alter a replacement Session, execution location, or
  presentation attachment.
- Ingestion sequence orders normalized arrival and does not claim upstream causality. Exact Session,
  Turn, request, event, and generation guards reject a late callback that no longer matches current
  state.
- A live Session has one of `unknown`, affirmative `idle`, or one exact active Turn.
- An ended Session has no active Turn and no open Attention Request.
- Every Turn-scoped request belongs to the current exact Turn when admitted; every request identity
  is unique within its Session generation.
- Session parentage is acyclic, each child has at most one parent, and a child never changes roots
  through a late event.
- Observation staleness never becomes idle, success, failure, request resolution, or Session end.
- Terminal and heuristic signals cannot create a native action, answer, approval decision, or
  high-trust notification candidate.
- A client event acknowledgement never resolves a shared request; request resolution never implies
  that every client saw the event.
- Adding a second presentation attachment never duplicates the underlying Turn, Attention Request,
  Outcome, semantic event, or notification attempt. Detaching one presentation cannot disconnect a
  Session that still has another trusted route.
- An OSC correlation half cannot create, modify, or finish a rich Session until a bounded matching
  event half arrives through the trusted Unix-socket channel; unmatched halves expire without a
  domain transition.
- Presentation and aggregation are pure deterministic projections of canonical Agent state and an
  explicit client view.

## Provider adapters

| Capability         | Provider-neutral meaning                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------------------------ |
| `sessionLifecycle` | Exact observed Session start and end                                                                         |
| `turnLifecycle`    | Exact Turn start, finish, affirmative idle, interruption, and outcome                                        |
| `questions`        | Correlated user-question request and resolution                                                              |
| `approvals`        | Correlated permission or approval request and resolution                                                     |
| `authentication`   | Correlated authentication-required observation                                                               |
| `activity`         | Bounded activity category without raw tool input or output                                                   |
| `subagents`        | Stable parent-child Session relationship                                                                     |
| `progress`         | Provider-owned bounded progress, not generic terminal progress promoted to Agent authority                   |
| `decisionChannel`  | Provider-native correlated response path; excluded from phase one even when the source supports it           |
| `resume`           | Provider-native Session resume or navigation; excluded from phase-one observation unless separately accepted |

| Adapter source                    | Permitted phase-one projection                                                                                            |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Native provider API or app server | Every declared observed capability; native decisions remain disabled                                                      |
| Managed provider hooks            | Declared Session, Turn, attention, activity, hierarchy, and outcome facts; navigation only                                |
| Structured terminal signal        | Generic provider identity and bounded basic activity with an explicit untrusted or limited-status label                   |
| Process or screen heuristic       | Generic detected/unknown status only; no attention, idle, outcome, progress, or operating-system notification eligibility |

Claude Code exposes lifecycle, tool, permission, subagent, stop, and failure events through its
official [Hooks reference](https://code.claude.com/docs/en/hooks). Codex exposes corresponding
hook events through [Codex Hooks](https://learn.chatgpt.com/docs/hooks), while the
[Codex App Server](https://learn.chatgpt.com/docs/app-server) offers a later native integration for
thread, turn, item, approval, and user-input flows. These upstream mechanisms establish adapter
feasibility; their payloads are not Orchbee's domain model or compatibility authority.

Provider identity and semantic status remain separate. A curated product registry owns localized
provider names, icons, and safe brand accents. An adapter may supply a stable provider ID and
namespaced diagnostic metadata, but it cannot inject arbitrary markup, icons, colors, or
product-facing strings. Unsupported providers use the generic Agent presentation and advertise only
their proven capabilities.

Agent integration setup must be explicit, reversible, and unable to block the Agent when Orchbee is
absent. No implementation may silently edit a user-global Agent configuration. Launch-scoped setup,
managed plugin installation, and explicit persistent configuration are materially different
choices that require an accepted integration decision. The accepted Claude Code choice and the
accepted multi-Agent extension are recorded below.

### Accepted Claude Code setup and SSH/mux dual-channel ingress

On 2026-08-27, the active requester acting as product and architecture decision owner accepted
Claude Code as the first provider and selected an explicit managed-plugin setup. Orchbee may present
the configuration for review, but the user installs and enables it; Orchbee never silently edits
global Claude Code configuration. The managed command hook reads a bounded provider event, discards
prompts, tool inputs and outputs, commands, transcript paths, working directories, arbitrary
messages, and provider error text, and emits only allowlisted facts. Hook failure is nonblocking,
and disable or uninstall is the rollback. The official
[Hooks reference](https://code.claude.com/docs/en/hooks) and
[plugin reference](https://code.claude.com/docs/en/plugins-reference) are upstream setup evidence,
not Orchbee compatibility authority.

The previously delivered random loopback HTTP endpoint, Bearer capability, and secret-bearing
overlay values are superseded by the 2026-08-28 acceptance. A non-persistent, generation-bound
environment overlay remains only to supply the local Unix-socket path and carries no credential.
The required end-to-end criterion is: a local Orchbee Tab opens SSH to a remote host, Claude Code
runs inside remote tmux or Zellij, and switching that mux client to another pane does not prevent
running, waiting-for-user, completion, or failure from updating every correct local Tab adornment
and eligible notification. An unrelated Tab must not change. Real-time delivery after every SSH
attachment disconnects is not claimed.

The accepted realization deliberately separates semantic authority from presentation correlation:

1. Desktop owns a bounded local Unix-domain observation listener. A local hook connects directly.
   Orchbee shell integration augments eligible user-invoked SSH connections with a reverse Unix-
   socket forwarding option, without retaining credentials or host records. OpenSSH authenticates
   and encrypts that transport; a remote hook discovers only same-user, live forwarded sockets and
   sends its already-sanitized rich envelope to each candidate. HTTP, TCP ports, Bearer headers, and
   inherited terminal secrets are absent.
2. The hook resolves the attached outer client TTYs for its mux execution. tmux resolution uses
   server-owned pane/session/client identity; Zellij resolution uses its client/session identity and
   the operating-system TTY owner. The resolver can walk nested mux layers. It writes one bounded
   OSC 777 correlation hint directly to each resolved outer TTY, so delivery does not depend on the
   Agent pane being selected or on tmux/Zellij passthrough configuration.
3. Each hint contains only a schema version, opaque event identity, one-time correlation identity,
   bounded activity hint, and presentation-scoped visibility. The matching rich socket envelope
   carries the same identities and allowlisted semantic fact. Desktop keeps a size- and time-bounded
   two-half join cache, accepts either arrival order, deduplicates exact events, and dispatches a rich
   transition only after both halves match. An unmatched or forged OSC half can create at most the
   existing limited activity projection.

When one execution has several attached mux clients, the hook emits one correlation half per outer
TTY and the trusted envelope names all of them. The runtime applies the semantic event once and
attaches every matched local presentation; Tab state fans out while operating-system notification
attempts remain deduplicated by semantic event identity. Closing or replacing one local terminal
retires only that attachment generation. A later SSH attachment can re-establish a new generation
without retargeting an old event.

Remote mux servers may outlive every SSH client, but they receive no persistent Orchbee capability.
The hook re-discovers live forwarded sockets and attached client TTYs for each event. If forwarding
is disabled, a mux/client mapping is ambiguous, either channel is unavailable, input is malformed,
the cache expires, or the listener is lost, the hook exits without blocking Claude Code and Orchbee
shows only limited, stale, or unknown state. Terminal input, output, and mux operation remain
independent.

### Accepted explicit multi-Agent adapter setup

On 2026-08-29, the active requester acting as product and architecture decision owner accepted an
explicit installed-adapter slice for Claude Code, Codex, Grok, OpenCode, Pi, and OMP. The adapter is
installed on every execution host where the Agent runs, including a remote host behind SSH, tmux,
or Zellij. Orchbee may package the adapter and show reviewed installation instructions, but it does
not silently modify global Agent configuration, copy an adapter to a remote host, or enable it
without the user's action. Disabling or uninstalling the plugin or extension is the rollback, and
every callback remains fail-open so observation cannot block Agent work.

The application owns a closed adapter registry. A rich envelope carries one of six fixed adapter
IDs, never a provider-supplied capability list. Desktop maps that ID to a curated provider ID, an
allowlist of accepted normalized events, and the maximum canonical capabilities below. Unknown
adapter IDs, unsupported normalized events, malformed identities, and observations exceeding that
maximum are rejected. Provider session keys, operation keys, subagent keys, and external event IDs
are namespaced by adapter ID before they enter shared ingress state, so equal upstream strings from
different Agents cannot alias.

| Adapter ID    | Reference contract                        | Maximum accepted capabilities                                                          | Exact upstream evidence used by the adapter                                                   |
| ------------- | ----------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `claude-code` | Claude Code Hooks reviewed 2026-08-29     | `sessionLifecycle`, `turnLifecycle`, `questions`, `approvals`, `activity`, `subagents` | Session, prompt, tool, permission, compaction, subagent, stop, failure, and session-end hooks |
| `codex`       | Codex CLI `0.150.1` hooks/plugin contract | `sessionLifecycle`, `turnLifecycle`, `questions`, `approvals`, `activity`, `subagents` | Session, prompt, tool, permission, compaction, subagent, stop, and session-end hooks          |
| `grok`        | Grok CLI `1.0.5` hooks/plugin contract    | `sessionLifecycle`, `turnLifecycle`, `approvals`, `activity`, `subagents`              | Session, prompt, tool, subagent, stop/failure/cancel, and typed `Notification` hooks          |
| `opencode`    | OpenCode `1.18.25` plugin event contract  | `sessionLifecycle`, `turnLifecycle`, `questions`, `approvals`, `activity`              | `session.*`, `question.*`, `permission.*`, tool, and error events                             |
| `pi`          | Pi `0.84.4` extension contract            | `sessionLifecycle`, `turnLifecycle`, `questions`, `activity`                           | Session, agent/settled, blocking UI-prompt, turn, and tool-execution events                   |
| `omp`         | OMP `18.0.10` extension contract          | `sessionLifecycle`, `turnLifecycle`, `questions`, `approvals`, `activity`              | Session, agent/turn, Ask-tool execution, exact tool-approval, and tool-execution events       |

Codex and Grok use reviewed command-hook plugins; OpenCode uses its documented JavaScript plugin
event surface; Pi and OMP use their documented extension APIs. Each provider entry point validates
and sanitizes its own raw payload before calling a shared nonblocking Unix-socket and OSC transport
library. Raw prompts, messages, tool arguments and results, commands, paths, error text, and
arbitrary provider metadata never cross that boundary. Sharing transport code does not make one
provider's event names or payload interpretation authoritative for another provider.

These reference versions and checked fixtures define the compatibility claim; upstream branding or
API resemblance does not. Pi `0.84.4` is the first claimed Pi contract in this slice because its
current package and settled/UI-prompt events differ from both the older package line and OMP.
Versions outside the fixture matrix may continue to run as Agents, but Orchbee makes no rich-state
parity claim until their adapter fixtures and capability grant are reviewed. A provider callback
that lacks an exact event degrades to the capabilities it can prove; absence of an event never
means idle, success, failure, approval, or question resolution.

All six adapters reuse the accepted local or SSH reverse-forwarded Unix socket plus exact outer-TTY
OSC 777 correlation join. They add no ACP, App Server, HTTP listener, TCP port, screen scraping, or
process-name authority. The same multi-attachment, hidden mux pane, generation fencing,
deduplication, notification eligibility, and disconnected-route exclusions apply unchanged.

## Presentation and aggregation

The presentation resolver uses this semantic priority:

```text
open attention > unacknowledged failure > unacknowledged success > active Turn > idle
```

Freshness is an overlay on the resolved state. A stale working Session appears as “last observed
working, status stale”; it never silently moves to idle. When several states share the highest
priority, the compact surface shows a count and the detailed surface orders stable event identities
oldest first instead of choosing an arbitrary request kind.

| Canonical condition                        | Pane or panel header                                                         | Tab and window                                                                   | Activity surface                                     | Operating-system candidate                                  |
| ------------------------------------------ | ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | ---------------------------------------------------- | ----------------------------------------------------------- |
| Open question, approval, or authentication | Persistent localized Agent chip or banner with exact navigation              | Standard attention overlay and unresolved count; optional window attention count | Pinned under Needs attention until resolved          | Yes when the exact location is not visible                  |
| Unacknowledged failed Turn                 | Failure label without raw provider text; terminal remains usable             | Standard error overlay                                                           | Durable recent failure until acknowledged or expired | Yes when not visible                                        |
| Unacknowledged successful Turn             | Bounded completed label                                                      | Standard completed or unread overlay                                             | Recent completion                                    | Yes when no current attachment is visible                   |
| Active Turn                                | Provider identity plus working, waiting-external, or bounded activity detail | Standard active overlay and active count                                         | Running                                              | No                                                          |
| Affirmative idle                           | Provider identity plus Ready                                                 | Neutral Agent icon                                                               | Active Session without a pending event               | No                                                          |
| Unknown, stale, or disconnected            | Explicit limited, stale, or disconnected label without guessing              | Neutral health overlay                                                           | Diagnostic and last reliable observation             | Only after an accepted grace policy while previously active |
| Ended Session                              | No active status after its location projection retires                       | No active aggregate                                                              | Retained only as a bounded recent event              | No additional candidate                                     |

- The panel-header left region is the preferred compact provider and status composition
  point. The centered custom, OSC, or CWD-derived title remains stable and does not become a status
  ticker.
- Tab aggregation covers every Agent Session bound to every panel and child terminal in that tab.
  It shows the highest semantic priority plus counts rather than only the focused pane.
- A remote mux execution is projected through its matched local presentation attachments. Switching
  the remote client away from the Agent pane changes attachment visibility, not attachment identity;
  the local Tab icon therefore continues to show running, attention, completion, or failure.
- Curated provider presentation identifies Claude Code, Codex, Grok, OpenCode, Pi, or OMP and
  falls back to a generic Agent for unknown provenance. A separate standard overlay conveys
  attention, failure, completion, work, or degraded health consistently across providers.
- Existing generic Tab fields such as `loading` and `crashed` must not be reinterpreted as Agent
  work or Turn failure. A reviewed status-adornment composition should carry the richer projection.
- The Activity surface groups Needs attention, Running, and Recent. It is the durable in-product
  recovery path when an operating-system notification is dismissed or cannot navigate.
- Unified Search consumes the Agent-owned searchable read model and prepared navigation Action
  already reserved by the [unified-search proposal](../workbench/unified-search-surface.md); search
  never starts or submits Agent work automatically.
- Every status has visible text or an accessible name, never color alone. Active animation honors
  reduced motion, and all product copy is localized in `en-US` and `zh-CN`.

## Notification policy

Adapters never deliver notifications. The deterministic Agent transition returns a semantic event;
application policy may then produce a device-scoped notification command.

| Transition                                | Eligibility                                                                           |
| ----------------------------------------- | ------------------------------------------------------------------------------------- |
| New Attention Request                     | Fresh exact Session, declared attention capability, no current attachment visible     |
| Failed Turn                               | Fresh exact semantic outcome, no current attachment visible                           |
| Successful Turn                           | Fresh exact semantic outcome, no current attachment visible                           |
| Observation lost while previously active  | Accepted grace period elapses without recovery; no notification on transient flapping |
| Activity, retry, tool, or progress update | Never                                                                                 |
| Idle, acknowledgement, or duplicate event | Never                                                                                 |

- In-product state is always available without notification permission. This slice never prompts
  automatically: operating-system delivery requires an already-enabled Agent notification policy
  and existing platform permission. A mux Agent pane reported hidden counts as not visible even
  when its enclosing Orchbee Tab is selected; any current visible attachment suppresses delivery.
- One semantic event ID produces at most one delivery attempt per device policy generation. An
  ambiguous operating-system API result is recorded and not retried automatically; the proposal
  makes no claim that the operating system displayed the notification.
- Cross-device delivery may duplicate. Phase one claims per-device deduplication only; shared
  request resolution remains canonical while event acknowledgement and delivery receipts remain
  client or device scoped.
- Notification content contains only a curated provider name and localized state, such as “Codex
  needs approval” or “Agent task completed.” It excludes prompt, question, command, tool, output,
  transcript, directory, project, environment, user, and provider error text.
- Activating a notification navigates to a deterministic current attachment, preferring a hidden
  navigable presentation in the active application; when none remains, it opens the Activity
  surface. It never types into a terminal, answers a question, approves a permission, or replays a
  stale action.
- Notification failure does not change the Agent outcome or suppress the in-product event.

## Background transport and recovery

- The application-level Agent runtime remains subscribed while terminal React surfaces and inactive
  tabs are unmounted. Hidden renderer instances are not a monitoring mechanism.
- The initial local deployment may co-locate the Agent runtime with Desktop, but logical ownership
  and serialized contracts cannot depend on Electron or React. A separate supervised process is a
  later topology decision.
- Desktop creates one mode-restricted Unix socket in its private runtime directory and removes it
  on shutdown. Local Agent hooks receive only that non-secret socket path. Eligible interactive SSH
  invocations add one random same-user remote socket forwarding to the local listener; shell
  integration preserves the user's executable, arguments, authentication, host policy, and exit
  status.
- The remote hook does not depend on environment inherited by a long-lived mux server. For every
  event it discovers live same-user forwarded sockets and attached outer client TTYs, emits bounded
  correlation halves, and sends the same sanitized rich event through the candidate sockets.
- Terminal Host retains admitted OSC correlation halves in a 64-entry terminal-sequence FIFO. A
  reader rollback publishes none, outgoing backpressure preserves the exact head, and a full FIFO
  drops only the new untrusted hint; rapid lifecycle events therefore cannot overwrite an earlier
  admitted correlation with a later one.
- Desktop joins correlation and rich halves in bounded pending maps keyed by exact event and
  one-time correlation identities. Join entries expire, accepted event identities are deduplicated,
  and presentation generations fence terminal reuse. Neither pending map is canonical Agent state.
- Browser and future mobile clients consume authorized snapshots and semantic events through a
  lightweight application subscription. They do not attach hidden terminal renderers to observe
  status.
- Renderer restart preserves runtime observation and obtains a fresh snapshot. Phase one does not
  promise Agent-state continuity across Desktop, monitor, Terminal Host, or operating-system
  restart; after such loss it reports stale or unknown until a provider supplies current evidence.
- The Unix-socket envelope and OSC correlation hint use exact versioning, bounded payloads, strict
  boundary parsing, event identity, generation, and sequence. They never transport prompt,
  transcript, command, tool content, filesystem paths, or provider error text.
- A remote mux with no live Orchbee SSH attachment cannot produce a real-time local presentation
  update. Reattachment establishes new correlation and presentation generations; it does not replay
  or infer missed semantic events. Offline relay and durable remote history remain reserved.
- If the Agent runtime or any adapter fails, terminal input, output, lifecycle, and close behavior
  continue independently.

## Security and privacy

- Parse every provider, process, terminal, storage, IPC, and network payload at its first trusted
  boundary with explicit size, string, collection, identity, and version limits.
- Whitelist canonical metadata. Do not ingest a provider transcript path, prompt, question text,
  command, tool input or output, environment, or arbitrary provider message merely because an
  upstream hook supplies it.
- The local and remote Unix sockets are user-private, ephemeral observation endpoints rather than
  general Desktop capabilities. They accept only the bounded observation schema and grant no input,
  filesystem, command, response, notification, or native-decision authority. Socket paths and
  correlation identities are absent from product diagnostics, telemetry, and notification payloads.
- Treat terminal escape sequences and screen content as attacker-controlled display input. They
  cannot create an actionable request, native action, or trusted notification. Rich observation
  additionally requires a matching envelope received through the user-private local socket or its
  SSH-authenticated reverse forwarding.
- The trust boundary assumes the local desktop user and the authenticated remote account are not
  hostile to themselves. A same-user process can access that user's PTYs and forwarded sockets; this
  slice limits blast radius and content but does not sandbox mutually untrusted processes sharing
  one operating-system account.
- Phase one is navigate-only. Inline Answer or Approve requires a separately accepted provider-native
  request identity, authorization and revalidation model, cancellation behavior, audit boundary,
  and protection against stale or replayed decisions. Simulated keystrokes are not an action
  channel.
- Hook setup and failure are observable and reversible. Orchbee unavailability cannot block, deny,
  approve, or alter the provider's Agent operation.

## Claims and evidence

| Claim    | Observable rule                                                                                                                                                     | Required evidence                                                                                                           | Limits                                                                                               |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `AR-001` | User-facing status never exceeds the declaring adapter capability or freshness                                                                                      | Adapter schema fixtures, reducer unit tests, and projection decision tables                                                 | Does not prove an upstream provider reported truthful facts                                          |
| `AR-002` | Duplicate, stale-generation, mismatched, and reordered events cannot mutate another Session, Turn, request, execution location, or presentation attachment          | Deterministic reducer tests, stateful property tests with replay, and `AgentRuntime` TLA+ safety checks                     | Model checking remains bounded                                                                       |
| `AR-003` | Attention remains open until exact resolution, cancellation, or Session end; client acknowledgement cannot resolve it                                               | Transition tests, generated request sequences, and TLA+ request-lifecycle invariants                                        | Native provider response is outside phase one                                                        |
| `AR-004` | Pane, Tab, Activity, Search, and notification candidates resolve the same semantic priority from one canonical snapshot and explicit client view                    | Pure projection tests, aggregation property tests, accessibility tests, and deterministic application traces                | Pixel appearance and actual OS delivery are separate evidence                                        |
| `AR-005` | An unmounted inactive terminal surface does not prevent an admitted runtime Agent event from updating product state                                                 | Controlled integration trace with the target tab unmounted, runtime event injection, snapshot publication, and remount      | Does not claim continuity across application or OS restart                                           |
| `AR-006` | A notification attempt is visibility-gated, preference-gated, per-device deduplicated, navigable, and free of prohibited content                                    | Policy boundary tests, payload allowlist tests, failure injection, and platform-capability call traces                      | Does not claim the operating system displayed or delivered it                                        |
| `AR-007` | Terminal and heuristic input cannot answer, approve, submit, or acquire a native decision capability                                                                | Trust-boundary negative fixtures, capability tests, and implementation security review                                      | Compromise of the trusted application runtime is outside this boundary                               |
| `AR-008` | Missing, disabled, stale, unsupported, or failed Agent observation cannot impair terminal operation or fabricate a terminal or Agent lifecycle outcome              | Adapter failure tests and application integration traces                                                                    | Provider-side failures outside observed signals may remain unknown                                   |
| `AR-009` | A remote Agent hidden behind another tmux or Zellij pane updates every matched local Tab, no unrelated Tab, and at most one notification attempt per semantic event | SSH/mux resolver fixtures, two-half ingress race tests, multi-attachment projection tests, and controlled end-to-end traces | Requires at least one live Orchbee SSH attachment; does not prove arbitrary third-party mux versions |

Selected correctness evidence:

- Deterministic reducer tests cover every state, event, guard, result, cancellation, no-op, and
  generation boundary before UI integration.
- Stateful property tests generate multiple Sessions, parent-child relationships, Turns, requests,
  execution locations, presentation attachments, duplicate events, reordered join halves,
  disconnections, reconnections, cancellation, and acknowledgement sequences with stable seeds,
  shrinking, replay, and coverage classes.
- TLA+ checks the smallest asynchronous model containing at least two Sessions, two presentation
  attachments, two observation generations, and competing join, detach, request, completion,
  acknowledgement, and publication events. It checks ownership, generation fencing, request
  persistence, one-active-Turn, attachment fan-out, and notification-candidate safety; it does not
  prove unbounded executions or OS delivery.
- Adapter conformance tests cover the command-hook sanitizer's strict allowlist, forbidden-content
  stripping and nonblocking failures; Unix-socket permissions, body bound, schema, SSH reverse-
  forwarding arguments, tmux and Zellij resolver fixtures, both join arrival orders, expiry,
  location correlation and retirement; and the terminal signal's strict limited-activity ceiling.
  Updating an upstream adapter requires a fixture and capability audit; Orchbee does not claim full
  upstream UI or protocol parity.
- Deterministic application traces cover active-visible suppression, inactive-unmounted attention,
  remote hidden-pane running, attention, completion and failure, multiple mux clients, unrelated
  Tabs, stale observation, navigation, missing targets, localization, accessibility, and reduced
  motion.
- Lean is not selected because phase one has no critical universal pure transformation beyond the
  reducer and projection invariants covered by stateful tests and TLA+. Warp, Ghostty, Claude Code,
  and Codex are not differential reference implementations for Orchbee's product policy.
- Native operating-system display, notification-center persistence, and focus transfer remain
  platform integration limits. The manual-only native-window focus test remains excluded unless a
  human explicitly authorizes that exact test in the active request.

## Delivery and rollback

1. Completed: accept the phase-one authorization; remaining decisions continue to block their first
   affected slice.
2. Completed: create `@orchbee/agent` with the accepted model, provider-neutral read model,
   deterministic reducer, projection policy, property evidence, TLA+ model, project contract, and
   owning bilingual reference.
3. Completed: Desktop owns the volatile application runtime, exact snapshot subscription,
   renderer-restart recovery, terminal signal transport, and failure isolation. Loopback HTTP and
   Bearer ingress are removed; the non-persistent generation overlay carries only the Unix-socket
   path without weakening terminal launch or recovery compatibility.
4. Completed: the canonical model separates one opaque execution location from multiple
   generation-fenced presentation attachments, with reducer, projection, property, and TLA+
   evidence ahead of application consumers.
5. Completed for all six explicit adapters: the user-private Unix-socket listener, managed SSH
   reverse forwarding, strict two-half join, tmux and Zellij outer-TTY resolution, nested-mux
   traversal, expiry, deduplication, closed capability registry, provider-specific sanitization,
   and nonblocking failure paths are implemented.
6. Completed: the same semantic priority projects into exact local Panel and Tab adornments;
   attention, success, and failure can produce permission-gated, hidden-attachment notification
   attempts. Activity and Unified Search remain later separately governed surfaces.
7. Consider provider-native Answer, Approve, resume, durable history, offline remote relay, mobile
   push, and cross-device delivery only as separately authorized consequential slices.

Every slice can roll back by removing its consumer and adapter registration while retaining the
terminal. A wire-contract rollback follows its exact negotiated compatibility policy; no slice may
depend on silently accepting an unknown schema. No phase-one data migration is required because the
proposal authorizes no durable Agent content or history.

## Accepted phase-one authorization

On 2026-08-27, the active requester explicitly accepted these decisions together while acting as
Orchbee product and architecture decision owner:

1. The Agent capability is the sole owner of Orchbee's observed Agent Session, Turn, Attention,
   Outcome, hierarchy, provenance, aggregation, and provider-neutral read model.
2. `@orchbee/agent` is created with the first deterministic domain slice and a real application
   consumer; no empty protocol or adapter package is created speculatively.
3. The application runtime owns always-on local observation and initially may co-locate with
   Desktop. Workbench and Terminal Host remain non-owners and consume or transport only their
   bounded contracts.
4. The canonical model uses orthogonal lifecycle, Turn, Attention, Outcome, and observation-quality
   dimensions; no flat status enum or presentation boolean becomes domain authority.
5. Phase one targets terminal-bound Claude Code and Codex through capability-declared observation
   adapters, plus a generic limited-status fallback. Screen text and process heuristics are never
   semantic authority.
6. Phase one is navigate-only. No notification, terminal signal, hook event, or UI surface can
   answer, approve, submit, type, or invoke a provider-native decision.
7. Pane header, Tab aggregation, Activity, and Unified Search consume one Agent-owned projection;
   provider identity and semantic status remain visually and semantically separate.
8. In-product attention and outcomes are always available. Operating-system effects require
   explicit permission and preference, exact visibility and event gating, safe localized metadata,
   and per-device deduplication.
9. Phase one preserves observation across renderer unmount and restart but makes no continuity claim
   across Desktop, monitor, Terminal Host, or operating-system restart. It stores no prompt,
   transcript, command, tool content, or durable Agent history.
10. Deterministic reducer, property, TLA+, adapter, application trace, accessibility, privacy, and
    failure-isolation evidence are required within their stated bounds before completion.

## Accepted SSH and mux topology authorization

On 2026-08-28, the same active requester explicitly accepted this additional consequential and
modeled slice while acting as Orchbee product and architecture decision owner:

1. The required scenario is one local Orchbee Tab connected by SSH to a remote host, with an Agent
   inside tmux or Zellij. Switching the mux client to another pane must not prevent running,
   waiting-for-user, success, or failure from updating the correct local Tab icon and eligible
   notification.
2. Business state remains provider-neutral and topology-neutral. The domain distinguishes an opaque
   execution location from zero or more local presentation attachments; provider, SSH, tmux, and
   Zellij details stay in adapters and ingress diagnostics.
3. Rich allowlisted observations use an application-owned local Unix socket, directly for local
   hooks and through an OpenSSH reverse Unix-socket forwarding for remote hooks. Loopback HTTP,
   Bearer credentials, and inherited generation secrets are removed.
4. OSC 777 remains a limited, attacker-controlled terminal channel. It may carry a bounded one-time
   correlation half to the exact outer PTY, but only a matching trusted socket half can authorize a
   rich Agent transition or notification candidate.
5. The managed hook resolves attached tmux and Zellij client TTYs for every event and may traverse
   nested mux layers. It must not require the Agent pane to be active, user passthrough settings, or
   a capability persisted in the remote mux environment.
6. One semantic event may attach to several correct local presentations but is reduced once and
   produces at most one device notification attempt. Unrelated Tabs never update; retiring one
   presentation never retires another current attachment.
7. This slice never prompts for notification permission. Existing explicit preference and platform
   permission gates remain required; attention, success, and failure are eligible when no current
   attachment is visible, including when the Agent pane is hidden inside a selected local Tab.
8. The rollback removes the SSH augmentation and rich socket consumer while preserving ordinary
   SSH, terminal operation, and limited OSC hints. Real-time behavior with all SSH attachments
   disconnected, offline replay, and exact arbitrary third-party mux-version parity are excluded.

## Accepted explicit multi-Agent adapter authorization

On 2026-08-29, the same active requester explicitly accepted this consequential and modeled slice
while acting as Orchbee product and architecture decision owner:

1. Claude Code, Codex, Grok, OpenCode, Pi, and OMP receive explicit provider-bound adapters while
   `@orchbee/agent`, Workbench projections, Tab aggregation, and notification policy remain entirely
   provider-neutral.
2. Adapters are installed by the user on the Agent execution host. Orchbee does not silently edit
   global configuration, bootstrap a remote machine, or make observation a prerequisite for Agent
   operation; disable or uninstall is the rollback.
3. Provider-specific code validates and removes content before using one shared nonblocking UDS and
   OSC transport. The accepted SSH reverse forwarding and tmux/Zellij outer-TTY correlation apply
   to all six adapters without a new HTTP, ACP, App Server, screen-parsing, or process-heuristic path.
4. A closed Desktop registry, not adapter input, grants provider identity, accepted event kinds, and
   maximum capabilities. Unknown adapters and over-capability events are rejected, and all external
   identities are adapter-namespaced before shared correlation or domain reduction.
5. Compatibility is reference refinement against pinned upstream versions and deterministic
   sanitized fixtures. A new upstream version expands no capability until its fixture and grant are
   reviewed; an unsupported version may run normally but has no guaranteed rich-state parity.
6. The slice remains observation-only and navigate-only. No adapter can answer, approve, submit,
   resume, control, or persist Agent content, even when an upstream extension API permits it.

## Accepted explicit local integration-management authorization

On 2026-08-29, the same active requester explicitly accepted this additional consequential and
modeled slice while acting as Orchbee product and architecture decision owner:

1. Orchbee may inspect local Agent executable and integration state without changing it, but it
   never installs, enables, disables, updates, or repairs an adapter automatically. Opening
   Settings and application startup are read-only.
2. One explicit user action in Settings may execute a provider's official local plugin command or
   create or remove one exact Orchbee-owned integration entry. The operation must never answer an
   interactive trust prompt, overwrite a conflicting file, or rewrite unrelated user configuration.
3. Local operations are serialized and follow inspect, preflight, mutate, and verify. A pre-commit
   failure preserves the prior state. An ambiguous or post-commit failure triggers read-only
   reconciliation and cannot claim success or rollback until the resulting state is observed.
4. Disable removes or disables only the exact Orchbee-owned plugin, extension, or symlink. It
   preserves other plugins, user settings, marketplaces, project configuration, and Agent data.
5. Provider-specific inspection and mutation remain behind one provider-neutral Desktop capability.
   Settings consumes bounded integration state and failure codes and does not branch on CLI output,
   filesystem layout, shell, or provider configuration syntax.
6. Remote hosts remain manual-only. Orchbee may show reviewed commands for the user to run on the
   execution host, but it never copies the adapter, invokes a remote command, edits remote files, or
   treats a local enablement as evidence that a remote integration is enabled.
7. A dismissible, device-local reminder may appear only when read-only inspection finds an available
   local Agent without its Orchbee integration. Dismissal is scoped to that integration, the reminder
   links to Settings, and adapter enablement remains independent from Agent notification preference
   and operating-system notification permission.
8. This slice adds no background updater and makes no compatibility claim beyond the reviewed
   fixture matrix. A later bundled adapter may be offered only through another explicit user action.

## Reserved decisions

The accepted phase-one domain slice would not decide:

- whether the runtime remains Desktop-co-located or becomes a separately supervised Agent process;
- any future cross-process Agent-runtime protocol naming, process extraction, version negotiation,
  and retention across application or operating-system restart; the Desktop Unix-socket ingress,
  terminal-signal ingress, and renderer snapshot protocol are current authority for their narrower
  boundaries;
- automatic local or remote adapter deployment, every Orchbee-initiated remote configuration
  change, silent local configuration edits, a background adapter updater, and compatibility claims
  outside the pinned provider fixture matrix;
- notification defaults beyond the existing explicit opt-in, stale grace period, recent-event
  bounds, per-client acknowledgement persistence, and cross-device suppression;
- the final Activity surface composition, window or Dock badge policy, and experience-component
  reuse or admission;
- provider-native question and approval payloads, response authorization, audit, cancellation,
  replay protection, and inline interaction;
- offline remote relay, replay after all SSH attachments disconnect, durable remote identity,
  reconnect recovery, non-OpenSSH transports, and unsupported mux-version compatibility;
- durable Agent history, prompt or transcript content, search indexing, telemetry, retention,
  deletion, export, or synchronization;
- native Codex App Server ownership, Agent launching, composer, submission, orchestration, resume,
  scheduling, and external cost policy; and
- mobile push, multiple users, account-level acknowledgement, and exactly-once cross-device
  delivery.

Implementation stops at the earliest slice affected by one of these decisions until the required
decision owner accepts its alternatives, consequences, evidence limits, rollout, and recovery.
