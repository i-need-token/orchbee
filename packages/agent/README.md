# @orchbee/agent

Provider-neutral domain model and deterministic runtime for observed Agent state in Orchbee.

The package is the sole owner of canonical observed Agent Session, Turn, Attention Request, Outcome, hierarchy, provenance, freshness, generation fencing, and presentation projection semantics. It contains no provider adapter, Electron or React dependency, transport implementation, platform effect, prompt submission, or Agent orchestration authority. Its machine-readable scope is defined by [`project.json`](project.json).

## Model

An `AgentSessionSnapshot` keeps independent facts instead of collapsing them into one status enum:

- `lifecycle` records whether the observed Session is live or ended;
- `turn` is unknown, affirmatively idle, or one exact active Turn with a bounded phase and activity;
- `activityHint` retains only a terminal-scoped, non-semantic activity hint while `turn` remains unknown;
- `pendingRequests` retains correlated questions, approvals, authentication needs, or unclassified attention until an exact resolution, Turn outcome, or Session end;
- `lastOutcome` records the most recent exact Turn result without ending a live Session;
- `observation` records source, fidelity, freshness, generation, and monotonic ingestion sequence;
- `executionLocation` is an opaque provider- and topology-neutral execution anchor; and
- `presentationAttachments` binds that execution to zero or more exact local terminal presentation generations, each with independent navigation and visibility.

Session, Turn, Request, Event, and Provider identities are bounded opaque strings. A runtime snapshot contains at most 512 Sessions, and one Session contains at most 128 pending requests. Parent Sessions must exist in the same snapshot and parentage must be acyclic. An ended Session cannot retain active work or pending requests.

Phase one accepts only `navigate-only` attention paths. The package does not export a provider-native decision or resume capability. Prompt text, question text, commands, tool inputs or outputs, transcripts, filesystem paths, environment values, and provider error text are not model fields.

## Transitions and trust

`transitionAgentRuntime(...)` is a pure reducer over a trusted, typed `AgentObservationEvent`.
`AgentRuntime` owns one snapshot, publishes only applied revisions, and emits semantic attention,
resolution, outcome, and Session-end events separately from state subscriptions. It also retains the
latest 512 applied revisions as a content-free change journal. Each record contains only the runtime
revision, canonical transition kind, and current Session summary; unchanged and rejected input
creates no record. `readChanges(...)` returns ordered pages, `current`, `revision-ahead`, or an
explicit `reset-required` result when a caller is behind retention. Duplicate or reordered sequence
values are unchanged; mismatched generation, identity, capability, parent, location, or Turn and
Request correlation is rejected without mutation.

Source and capability are independent. A heuristic source can claim no semantic capability. A terminal source can claim only bounded activity through `limited-activity-observed`; the resulting hint never creates a Turn, affirmative idle, outcome, attention request, semantic event, or notification authority. Native and hook sources may claim only the observational capabilities they actually implement. `presentation-attached`, `presentation-visibility-changed`, and `presentation-detached` update exact generation-fenced presentation views without duplicating Session semantics; detaching one presentation leaves the Session connected while another navigable attachment remains. Reconnect requires an explicit higher-generation snapshot, revalidates its source capability ceiling and every semantic claim, and never reconstructs a missing completion or resolution. Staleness changes only freshness. Client acknowledgement is intentionally outside the canonical reducer and cannot resolve a shared request.

An exact upstream idle or settled event may close an active Turn and clear only that Turn's pending requests. It preserves `lastOutcome` and emits no outcome semantic event: idle is not evidence of success. Session-scoped requests remain until their own exact resolution or Session end.

The reducer does not parse arbitrary external values. Every provider, IPC, storage, or network adapter must parse its own bounded wire schema at the first trusted boundary before constructing an `AgentObservationEvent`.

## Projection

`projectAgentSession(...)`, `projectAgentLocation(...)`, and `projectAgentLocations(...)` derive compact read models with this semantic priority:

```text
open attention > failed/canceled/interrupted > succeeded > active Turn > idle > unknown > ended
```

Freshness remains a separate overlay and cannot silently turn stale work into idle. Every attachment receives the same Session semantics, so one remote execution can truthfully fan out to several local panels or Tabs without duplicating the Turn, request, outcome, or semantic event. Presentation aggregation excludes retained ended Sessions while a live Session remains and derives freshness from the Session or attention set that produced the resolved status, so a lower-priority disconnected Session cannot degrade unrelated fresh work. A presentation ID with multiple retained attachment generations prefers a navigable generation, then fresher observation, then higher semantic priority, and finally the stable generation order. Provider identity stays separate from semantic status so applications can apply curated localized names and icons without accepting provider-controlled product copy or markup.

## Semantic command read model

The package owns strict version-1 contracts for `agent.session.list`, `agent.session.get`,
`agent.session.explain`, `agent.session.wait`, and `agent.session.changes`. These descriptors,
schemas, and deterministic projection helpers are transport- and application-independent. Desktop
application composition may admit point-in-time reads and waits over an `AgentSnapshotSource` and
the change feed over an `AgentChangeSource`; the CLI may expose a high-level projection only after
the complete live descriptor matches this package's contract.

`agent.session.list` projects stable summaries from one runtime revision, defaults to live Sessions,
optionally includes retained ended Sessions, and caps one page at 100 items. `agent.session.get`
returns the exact retained canonical Session snapshot plus its semantic status.
`agent.session.explain` adds source, fidelity, freshness meaning, generation, sequence, capability
ceiling, and the canonical status basis. Explanation remains `navigate-only` and content-free: it
never fabricates prompt, transcript, command, tool, path, provider-error, adapter-correlation, or
screen-matcher evidence.

`agent.session.wait` defines exact-Session matching, but does not own a clock, subscription,
execution ledger, timeout effect, or coordinator lifecycle. Its default conditions are attention,
all outcomes, idle, and ended. Explicit conditions can select any semantic status or the `fresh`,
`stale`, and `disconnected` health values. A semantic status matches only while its observation is
fresh; health matches only its exact requested value. An optional `afterRevision` requires a
strictly newer global runtime revision. The application-owned asynchronous handler must subscribe
before its decisive recheck and return one exact matching snapshot or an explicit missing, ahead,
timeout, cancellation, or coordinator-retirement outcome.

`agent.session.changes` defines a global, content-free change-page contract. An application returns
an empty baseline at the current revision when no cursor is supplied. A signed cursor then selects
the next revision after that baseline; one page contains at most 100 ordered records. A current
cursor may long-poll, timeout completes as an empty `current` page, and retention loss completes as
an empty `reset-required` page with a cursor at the current revision. Consumers must refresh the
canonical Session snapshot before continuing after reset. The journal and cursors are volatile and
claim no continuity across Desktop or future Hub restart.

## Current application composition

Desktop owns one volatile `DesktopAgentObservationRuntime` for the application lifetime, including
the bounded change journal. Renderer document replacement retires only the previous broker; the
runtime and journal remain alive, and the next renderer requests a fresh full snapshot through the
strict `@orchbee/desktop-client-protocol` Agent-observation port. Version 2 of that port also
publishes exact semantic events after their snapshot update so a renderer can evaluate one
notification effect without reconstructing an event from state. The Web adapter rejects malformed
or uncorrelated messages and ignores non-increasing revisions. Workbench consumes the narrow
`AgentSnapshotSource`, projects non-ended state into exact terminal panel headers, and deduplicates
all terminal presentations in a Tab into a provider identity icon plus one localized non-ended
status adornment. It also derives live Session summaries for every Workspace without mounting
inactive terminal surfaces. Retained ended state is excluded from panel, Tab, Workspace summary,
and Workspace detail presentation, so the ordinary terminal or layout identity returns immediately.
Desktop semantic-command composition alone consumes `AgentChangeSource` for signed cursor reads and
long polling.

Desktop admits two deliberately different observation classes. An exact private-APC v3 launch envelope from shell integration can create only a fresh heuristic Session with no capabilities and unknown status. It emits no semantic event or notification candidate, becomes stale after 30 seconds, and retires after 120 seconds unless refreshed. Retirement disconnects and ends only the local heuristic presence projection without emitting a semantic Session-end event. A successfully admitted rich observation for the same provider, presentation, and terminal generation retires the heuristic projection before taking over; later passive evidence remains suppressed while that exact rich route is live. Explicitly installed, provider-bound adapters for Claude Code, Codex, Grok, OpenCode, Pi, OMP, Gemini CLI, GitHub Copilot CLI, Cline, and Qwen Code each validate their own upstream contract before stripping prompts, tool content, transcript paths, working directories, error text, and unknown fields. They send the shared normalized v3 rich allowlist, including bounded correlation visibility, over a private Unix-domain socket. Only after at least one candidate returns the exact acceptance ACK do they write the separate content-free private-APC correlation hint to every resolved outer TTY. A closed application registry grants provider provenance, accepted events, and maximum capabilities; no adapter payload grants itself authority. Neither rich transport half has enough authority alone: Desktop accepts rich semantics only after an exact bounded event/correlation join for a live terminal generation. The shared resolver covers direct terminals, every attached tmux client, Zellij clients conservatively, and nested mux targets. The shell integration gives eligible interactive SSH sessions a random remote socket through OpenSSH reverse Unix-socket forwarding; there is no HTTP port, Bearer credential, inherited terminal secret, automatic remote installation, durable remote cache, or public terminal-notification channel. Any path may fail without affecting the Agent or terminal.

One Session may attach to several presentations without duplicating its Turn, request, outcome, or semantic event. Workbench shows the aggregate in panel headers, Tab icons, and deduplicated live Workspace summaries. A Session attached to several presentations in one Workspace counts once there, while a cross-Workspace attachment appears once in each represented Workspace. When the user explicitly enables Agent notifications and system permission is already granted, exact attention, success, and failure events notify only while every attached destination is hidden; activation selects the corresponding Tab and panel. Notification copy is curated and never includes provider-controlled content. Provider identity remains provenance and optional curated presentation identity; business transitions and consumers depend only on canonical capabilities and events. Automatic provider setup, durable Agent history, delivery while SSH is fully disconnected, and continuity across Desktop or operating-system restart remain outside this slice.

## Correctness evidence

Unit and property tests cover transition guards, exact attention capability admission, durable requests, outcome cleanup, outcome-free idle settlement, stale overlays, semantic Session end, nonsemantic heuristic retirement and rich takeover, source capability ceilings, reconnect, presentation-generation fencing, multi-attachment fan-out and retirement, schema invariants, runtime disposal, and deterministic projections.

Semantic-command contract and projection tests additionally cover default and explicit wait
conditions, fresh-only semantic matching, health matching, status-basis explanations, content
absence, ended filtering, bounded schemas, ordered change batching, current reads, overflow, and
explicit reset. Desktop operation tests refine pagination, exact-identity reads, both
subscribe-before-recheck waits, revision fencing, signed change cursors, timeout, cancellation,
overflow/reset, and synchronous subscription races.

[`formal/tla/AgentRuntime.tla`](formal/tla/AgentRuntime.tla) has two independent bounded scenarios. The reducer scenario explores event order across two Sessions and two clients, observation generations 0 through 1, sequences 0 through 6, and runtime revisions 0 through 6. It admits repeated and stale sequences plus wrong-generation, wrong-identity, and invalid observations as named no-ops. Authoritative `unknown`, `hidden`, and `visible` attachment visibility is part of the canonical snapshot; renderer reconnect clears only a client's cached snapshot. The scenario checks exact joined-observation admission, acknowledgement isolation, limited activity, outcome-free idle settlement, reconnect fencing, ended-Session cleanup, exact final-snapshot recovery, and notification candidates for attention, success, and failure while cancelled and interrupted outcomes remain non-notifying. Its single final publication is an explicit reduction of intermediate runtime publications.

The transport-order scenario separately explores revisions 0 through 2 and checks that each semantic event is delivered only after a snapshot whose revision includes that event. Neither scenario assumes fairness or proves unbounded Session, event, or revision counts, upstream provider truthfulness, adapter payload correctness, provider termination, platform notification delivery, pixel appearance, or continuity across application restart. Required action coverage spans both scenarios and prevents either one from passing without reaching its maintained transitions.

[`../../apps/desktop/formal/tla/AgentSemanticWait.tla`](../../apps/desktop/formal/tla/AgentSemanticWait.tla)
is the application-composition wait model. Its bounded state has two Sessions, two semantic statuses,
three health values, two Agent mutations (revisions 0 through 2), and two coordinator generations. TLC checks exact
subscription ownership, subscribe-before-decisive-recheck eligibility, fresh-only semantic matches,
exact matched output, no invented unmatched output, Agent-only revision ownership, and explicit
missing, future-baseline, timeout, cancellation, and coordinator-retirement paths. Those bounds do
not prove unbounded waiters, time, revisions, Sessions, or restart continuity.

[`../../apps/desktop/formal/tla/AgentSemanticChanges.tla`](../../apps/desktop/formal/tla/AgentSemanticChanges.tla)
is the application-composition change-feed model. With revisions 0 through 4, retention 2, page
bound 2, and two coordinator generations, TLC checks the bounded ordered journal suffix, baseline,
ordered batching, exact current subscription, `WaitingCursorIsCurrent` after the decisive recheck,
empty timeout, invalid cursor, cancellation, coordinator retirement, and explicit reset instead of
a silent retention gap. Those bounds do not prove unbounded retention, revisions, readers,
cryptographic cursor security, or restart continuity.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/agent verify:correctness
```

The broader future design and still-reserved product decisions remain in the bilingual [Agent runtime observation proposal](../../proposals/agent/agent-runtime-observation.md).
