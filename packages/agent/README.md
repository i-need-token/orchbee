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

`transitionAgentRuntime(...)` is a pure reducer over a trusted, typed `AgentObservationEvent`. `AgentRuntime` owns one snapshot, publishes only applied revisions, and emits semantic attention, resolution, outcome, and Session-end events separately from state subscriptions. Duplicate or reordered sequence values are unchanged; mismatched generation, identity, capability, parent, location, or Turn and Request correlation is rejected without mutation.

Source and capability are independent. A heuristic source can claim no semantic capability. A terminal source can claim only bounded activity through `limited-activity-observed`; the resulting hint never creates a Turn, affirmative idle, outcome, attention request, semantic event, or notification authority. Native and hook sources may claim only the observational capabilities they actually implement. `presentation-attached`, `presentation-visibility-changed`, and `presentation-detached` update exact generation-fenced presentation views without duplicating Session semantics; detaching one presentation leaves the Session connected while another navigable attachment remains. Reconnect requires an explicit higher-generation snapshot, revalidates its source capability ceiling and every semantic claim, and never reconstructs a missing completion or resolution. Staleness changes only freshness. Client acknowledgement is intentionally outside the canonical reducer and cannot resolve a shared request.

An exact upstream idle or settled event may close an active Turn and clear only that Turn's pending requests. It preserves `lastOutcome` and emits no outcome semantic event: idle is not evidence of success. Session-scoped requests remain until their own exact resolution or Session end.

The reducer does not parse arbitrary external values. Every provider, IPC, storage, or network adapter must parse its own bounded wire schema at the first trusted boundary before constructing an `AgentObservationEvent`.

## Projection

`projectAgentSession(...)`, `projectAgentLocation(...)`, and `projectAgentLocations(...)` derive compact read models with this semantic priority:

```text
open attention > failed/canceled/interrupted > succeeded > active Turn > idle > unknown > ended
```

Freshness remains a separate overlay and cannot silently turn stale work into idle. Every attachment receives the same Session semantics, so one remote execution can truthfully fan out to several local panels or Tabs without duplicating the Turn, request, outcome, or semantic event. Presentation aggregation excludes retained ended Sessions while a live Session remains and derives freshness from the Session or attention set that produced the resolved status, so a lower-priority disconnected Session cannot degrade unrelated fresh work. A presentation ID with multiple retained attachment generations prefers a navigable generation, then fresher observation, then higher semantic priority, and finally the stable generation order. Provider identity stays separate from semantic status so applications can apply curated localized names and icons without accepting provider-controlled product copy or markup.

## Current application composition

Desktop owns one volatile `DesktopAgentObservationRuntime` for the application lifetime. Renderer document replacement retires only the previous broker; the runtime remains alive and the next renderer requests a fresh full snapshot through the strict `@orchbee/desktop-client-protocol` Agent-observation port. Version 2 of that port also publishes exact semantic events after their snapshot update so a renderer can evaluate one notification effect without reconstructing an event from state. The Web adapter rejects malformed or uncorrelated messages and ignores non-increasing revisions. Workbench consumes the narrow `AgentSnapshotSource`, projects non-ended state into exact terminal panel headers, and deduplicates all terminal presentations in a Tab into a provider identity icon plus one localized non-ended status adornment. Retained ended state keeps only the provider identity icon.

Desktop admits two non-blocking observation halves. A strict terminal-signal adapter can emit only `limited-activity-observed` from a bounded OSC 777 version-1 payload, so the untrusted hint changes only `activityHint`. Explicitly installed, provider-bound adapters for Claude Code, Codex, Grok, OpenCode, Pi, OMP, Gemini CLI, GitHub Copilot CLI, Cline, and Qwen Code each validate their own upstream contract before stripping prompts, tool content, transcript paths, working directories, error text, and unknown fields. They send the shared normalized rich allowlist over a private Unix-domain socket and write a separate OSC 777 version-2 correlation hint to every resolved outer TTY. A closed application registry grants provider provenance, accepted events, and maximum capabilities; no adapter payload grants itself authority. Neither transport half has enough authority alone: Desktop accepts rich semantics only after an exact bounded event/correlation join for a live terminal generation. The shared resolver covers direct terminals, every attached tmux client, Zellij clients conservatively, and nested mux targets. The shell integration gives eligible interactive SSH sessions a random remote socket through OpenSSH reverse Unix-socket forwarding; there is no HTTP port, Bearer credential, inherited terminal secret, automatic remote installation, or durable remote cache. Either half may fail without affecting the Agent or terminal.

One Session may attach to several presentations without duplicating its Turn, request, outcome, or semantic event. Workbench shows the aggregate in panel headers and Tab icons. When the user explicitly enables Agent notifications and system permission is already granted, exact attention, success, and failure events notify only while every attached destination is hidden; activation selects the corresponding Tab and panel. Notification copy is curated and never includes provider-controlled content. Provider identity remains provenance and optional curated presentation identity; business transitions and consumers depend only on canonical capabilities and events. Automatic provider setup, durable Agent history, delivery while SSH is fully disconnected, and continuity across Desktop or operating-system restart remain outside this slice.

## Correctness evidence

Unit and property tests cover transition guards, exact attention capability admission, durable requests, outcome cleanup, outcome-free idle settlement, stale overlays, Session end, source capability ceilings, reconnect, presentation-generation fencing, multi-attachment fan-out and retirement, schema invariants, runtime disposal, and deterministic projections.

[`formal/tla/AgentRuntime.tla`](formal/tla/AgentRuntime.tla) is the bounded asynchronous model. It contains two Sessions, two client views, two observation generations, interleaved attention and reconnect lifecycles, limited and Turn activity, outcome-free idle settlement, client acknowledgement, visibility, notification candidates, snapshot publication, and renderer reconnect. TLC checks that acknowledgement cannot resolve a request, stale-generation rejection preserves canonical state, limited activity changes only its hint, an idle observation cannot fabricate an outcome or notification candidate, activity creates no notification candidate, candidates are fresh/live/invisible and attention-or-failure based, generation changes only on reconnect, ended Sessions have no active work, client revisions never exceed publication, and an observed or recovered snapshot exactly matches the published canonical state. Required action coverage prevents a vacuous passing model.

The model does not prove unbounded Session or event counts, upstream provider truthfulness, adapter payload correctness, liveness or provider termination, platform notification delivery, pixel appearance, or continuity across application restart.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/agent verify:correctness
```

The broader future design and still-reserved product decisions remain in the bilingual [Agent runtime observation proposal](../../proposals/agent/agent-runtime-observation.md).
