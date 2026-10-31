# @orchbee/agent

Private package entries export only symbols consumed in this repository, enforced by Knip `includeEntryExports`. Internal contracts, generated assets and independent conformance evidence retain their owners; their presence does not automatically expand the public entry. New clients add exports for admitted uses.

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

Canonical ordering of Sessions, pending requests, and presentation attachments is the raw UTF-16 code-unit lexicographic order: identical under every process locale, with no normalization, case folding, or numeric collation, and equal only for identical strings. Identity equality remains exact string equality. A presentation ID is a non-empty string of at most 256 UTF-16 code units that must be well-formed Unicode — lone surrogate code units are rejected while NUL, format characters, combining marks, and valid supplementary pairs remain legal.

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
Request correlation is rejected without mutation. A would-be-applied transition against a snapshot
already at the maximum safe revision is rejected as `capacity-exceeded` with no semantic event and
no mutation; it never throws and never preempts an earlier unchanged or rejected outcome.

After commit, an individual snapshot or semantic observer failure cannot change the applied
result or suppress remaining publications. Snapshot callbacks are captured once per applied transition after commit;
semantic callbacks are captured separately for each event. The
[runtime tests](src/__tests__/AgentRuntime.unit.test.ts) cover fault isolation and stable snapshot callback cohorts.

Source and capability are independent. A heuristic source can claim no semantic capability. A terminal source cannot claim semantic lifecycle or attention authority. Native and hook sources may claim only the observational capabilities they actually implement. `presentation-attached`, `presentation-visibility-changed`, and `presentation-detached` update exact generation-fenced presentation views without duplicating Session semantics; detaching one presentation leaves the Session connected while another navigable attachment remains. Reconnect requires an explicit higher-generation snapshot, revalidates its source capability ceiling and every semantic claim, and never reconstructs a missing completion or resolution. Staleness changes only freshness. Client acknowledgement is intentionally outside the canonical reducer and cannot resolve a shared request.

An exact upstream idle or settled event may close an active Turn and clear only that Turn's pending requests. It preserves `lastOutcome` and emits no outcome semantic event: idle is not evidence of success. Session-scoped requests remain until their own exact resolution or Session end.

The reducer does not parse arbitrary external values. Every provider, IPC, storage, or network adapter must parse its own bounded wire schema at the first trusted boundary before constructing an `AgentObservationEvent`.

## Projection

`projectAgentSession(...)`, `projectAgentLocation(...)`, and `projectAgentPresentationGroup(...)` derive compact read models with this semantic priority:

```text
open attention > failed/canceled/interrupted > succeeded > active Turn > idle > unknown > ended
```

Freshness remains a separate overlay and cannot silently turn stale work into idle. Every attachment receives the same Session semantics, so one remote execution can truthfully fan out to several local panels or Tabs without duplicating the Turn, request, outcome, or semantic event. Presentation aggregation excludes retained ended Sessions while a live Session remains and derives freshness from the Session or attention set that produced the resolved status, so a lower-priority disconnected Session cannot degrade unrelated fresh work. A presentation ID with multiple retained attachment generations prefers a navigable generation, then fresher observation, then higher semantic priority, and finally the stable generation order. Provider identity stays separate from semantic status so applications can apply curated localized names and icons without accepting provider-controlled product copy or markup.

## Semantic command read model

The package retains three deterministic reference projections used by the canonical corpus generator: `projectAgentSessionSummary`, `explainAgentSession`, and `matchingAgentWaitConditions`. The generator imports these references through the relative compiled path `../dist/__tests__/conformance/agent-reference.js`, executing `dist/__tests__/conformance/agent-reference.js`. `conformance:build` first builds the runtime, then compiles the same-owner `src/__tests__/conformance/agent-reference.ts` with the installed Rslib API. Both generation and checking run this build. This development output is excluded from package contents and has no public export subpath. The full Knip graph maps only this relative compiled import to its owning source through the exact workspace `paths` mapping, including before output exists; this is resolution rather than an entry or ignore, so removing all consumers leaves orphan source files; strict production scope excludes only the two reference implementation/type files, while all production entry checks remain enabled. They do not restore the retired TypeScript CLI or Desktop semantic-command coordinator.

Summaries project canonical Session state. Explanations preserve source, fidelity, freshness meaning, generation, sequence, capability ceiling and status basis, remain `navigate-only`, and contain no provider content. Semantic wait conditions match only fresh observations; explicit health conditions match the requested freshness value. These pure projections own no subscription, timer or command lifecycle.

The bounded runtime change journal and its revision/reset behavior remain owned by `AgentRuntime` and `changes.ts`. Removing retired CLI envelopes does not remove this runtime evidence or promise durable history across restarts.

## Current application composition

Desktop owns one volatile `DesktopAgentObservationRuntime` for the application lifetime, including
the bounded change journal. Renderer document replacement retires only the previous broker; the
runtime and journal remain alive, and the next renderer requests a fresh full snapshot through the
strict `@orchbee/desktop-client-protocol` Agent-observation port. Version 1 of that port also
publishes exact semantic events after their snapshot update so a renderer can evaluate one
notification effect without reconstructing an event from state. The Web adapter rejects malformed
or uncorrelated messages and ignores non-increasing revisions. Workbench consumes the narrow
`AgentSnapshotSource`, projects non-ended state into exact terminal panel headers, and deduplicates
all terminal presentations in a Tab into a provider identity icon plus one localized non-ended
status adornment. It also derives live Session summaries for every Workspace without mounting
inactive terminal surfaces. Retained ended state is excluded from panel, Tab, Workspace summary,
and Workspace detail presentation, so the ordinary terminal or layout identity returns immediately.
The retired Desktop semantic-command coordinator is not a current consumer of the change journal.

Desktop admits two deliberately different observation classes. An exact private-APC v3 launch envelope from shell integration can create only a fresh heuristic Session with no capabilities and unknown status. It emits no semantic event or notification candidate, becomes stale after 30 seconds, and retires after 120 seconds unless refreshed. Retirement disconnects and ends only the local heuristic presence projection without emitting a semantic Session-end event. A successfully admitted rich observation for the same provider, presentation, and terminal generation retires the heuristic projection before taking over; later passive evidence remains suppressed while that exact rich route is live. Explicitly installed, provider-bound adapters for Claude Code, Codex, Grok, OpenCode, Pi, OMP, Gemini CLI, GitHub Copilot CLI, Cline, and Qwen Code each validate their own upstream contract before stripping prompts, tool content, transcript paths, working directories, error text, and unknown fields. They send the shared normalized v3 rich allowlist, including bounded correlation visibility, over a private Unix-domain socket. Only after at least one candidate returns the exact acceptance ACK do they write the separate content-free private-APC correlation hint to every resolved outer TTY. A closed application registry grants provider provenance, accepted events, and maximum capabilities; no adapter payload grants itself authority. Neither rich transport half has enough authority alone: Desktop accepts rich semantics only after an exact bounded event/correlation join for a live terminal generation. The shared resolver covers direct terminals, every attached tmux client, Zellij clients conservatively, and nested mux targets. The shell integration gives eligible interactive SSH sessions a random remote socket through OpenSSH reverse Unix-socket forwarding; there is no HTTP port, Bearer credential, inherited terminal secret, automatic remote installation, durable remote cache, or public terminal-notification channel. Any path may fail without affecting the Agent or terminal.

One Session may attach to several presentations without duplicating its Turn, request, outcome, or semantic event. Workbench shows the aggregate in panel headers, Tab icons, and deduplicated live Workspace summaries. A Session attached to several presentations in one Workspace counts once there, while a cross-Workspace attachment appears once in each represented Workspace. When the user explicitly enables Agent notifications and system permission is already granted, exact attention, success, and failure events notify only while every attached destination is hidden; activation selects the corresponding Tab and panel. Notification copy is curated and never includes provider-controlled content. Provider identity remains provenance and optional curated presentation identity; business transitions and consumers depend only on canonical capabilities and events. Automatic provider setup, durable Agent history, delivery while SSH is fully disconnected, and continuity across Desktop or operating-system restart remain outside this slice.

## Correctness evidence

Unit and property tests cover transition guards, exact attention capability admission, durable requests, outcome cleanup, outcome-free idle settlement, stale overlays, semantic Session end, nonsemantic heuristic retirement and rich takeover, source capability ceilings, reconnect, presentation-generation fencing, multi-attachment fan-out and retirement, schema invariants, runtime disposal, and deterministic projections.

Projection tests preserve fresh-only semantic matching, explicit health matching, status-basis explanations and content absence. Runtime journal tests preserve ordered changes, current reads, overflow and reset. The canonical corpus executes the retained reference projections and is consumed independently by the Rust Server.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/agent verify:correctness
```

The broader future design and still-reserved product decisions remain in the [Agent runtime observation proposal](../../proposals/agent/agent-runtime-observation.md).
