# Terminal Agent report transitions

This model owns explicit terminal report facts: source authority, provider Session references,
source watermarks and suppression of stale lifecycle reports. It is distinct from canonical
hook-observation Sessions in `AgentRuntime`. The deterministic core is currently compiled for
conformance; live registry, physical-binding admission, public commands and query/TUI publication
remain integration obligations. No public operation returns success from this core alone.

## Inputs and ownership

The reference is Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, including public
`AppEvent` routing as well as `TerminalState` transitions. An author has an exact source and a
normalized, nonempty Agent label. Report state is idle/working/blocked/unknown; the metadata
display label `done` is not a report state. Reports carry explicit state/message, an optional validated
provider reference, optional full-u64 sequence and optional recognized start reason. Provider
reference validation and source-specific policies come from `orchbee-agent-observation-protocol`.
A separate trusted process observation supplies detected provider, fallback state, visible
blocker, exit flag and observation time. Report strings cannot establish process existence.

One report state belongs to one terminal generation. Its data consists of current authority,
persisted provider reference, source sequence watermarks, suppressed full-lifecycle owners,
pending replacement references/reports and stale Session identities. The core has no I/O,
clock access, canonical Session allocator, names registry, Workbench topology or persistence.
The caller supplies ordered owner-clock ticks; process input separately retains its original
sample tick so it cannot erase a newer custom authority. New clear suppression uses current
owner time even when a delivered process sample is older; fallback, exit and metadata cleanup
retain the sample time. State and observations have canonical
JSON traces. The transition prepares a new state; its caller owns admission and commit.

The eventual registry belongs beside terminal metadata under the Site/Workbench gate. A process
exit can invalidate both authority and source metadata; these changes must be prepared and
committed together. The core emits the Agent/time needed for that metadata decision and does not
mutate another owner. Canonical observation Sessions and their aliases remain with `AgentState`.
The binary must supply exact retained physical-generation facts before admitting a report or
process observation; a Client-provided label, PID or generic observation category is insufficient.

## Transition decisions

| Input | Decision and resulting facts |
| ----- | ---------------------------- |
| State report from reserved native source | Route to Session-reference reporting without a start reason; requested state and message do not override native observations. |
| State report from an identity-only source | Ignore state authority; the public session-report operation owns its reference. Qwen first follows the reserved-native route. |
| Other state report | Check process/owner/session/suppression guards, then source sequence, before replacing authority. Missing reference clears the previous report reference. |
| Session report without a valid reference | No state or sequence change. |
| Session report with a valid reference | Apply exact provider/start-source rules; full lifecycle may stage a replacement until process confirmation. Later guards can refuse after consuming sequence, matching the reference. |
| Clear authority | Only the current matching source (or omitted source) is eligible; sequence gates the clear. Persisted-only identity is not clearable through this operation. |
| Release an official source/Agent pair | Public routing is an unconditional no-op, including sequence. |
| Release a custom report | Reject a foreign authority, require matching effective Agent/reference and sequence; preserve independently confirmed live process and foreign persisted identity. |
| Process observation | Preserve a live full-lifecycle authority against weaker detection; reject observations older than release suppression, reconcile replacement claims, and clean matching exited-generation reports. |

Once a source accepts a sequenced report, subsequent unsequenced reports are stale. Equal or
lower sequence values never advance it. Only the reference's confirmed generation/session
reanchor rules reset that watermark. Clear/exit suppression and stale identities prevent late
reports from resurrecting old sessions. A pending lifecycle report is not effective authority.
OpenCode's unsequenced `select` path is explicit and must wait for process confirmation when absent.
A recognized start reason alone is not process proof; `branch` does not qualify for lifecycle reanchor.

An effective observation contains label, state, actual authority source/message and current
provider reference. Full-lifecycle authority is effective only with its matching live process.
Otherwise detected process/fallback state remains available. A newer visible blocker may override
a non-full-lifecycle report, without claiming a new report source. Clearing report facts never
ends a canonical `AgentRuntime` Session or fabricates a turn, approval, readiness or provider ID.

## Commit and verification boundary

`changed` describes the prepared state, independently of applied/ignored/deferred disposition.
In particular, an ignored Session report can advance a watermark; a caller must not use the
disposition as a substitute for change detection. The previous state remains untouched until
its owner accepts the complete candidate, metadata effects and encoded result.

The core maps describe an abstract finite trace, not a bounded network service. Live integration
must add per-source/history/terminal and total-byte admission, checked revisions, exact generation
reconciliation, permission checks and correlated receipts before exposing writers. It must fail
closed rather than discard sequence/stale history and resurrect an old report. Unavailable process
sampling must remain unavailable, never a fabricated exit or fresh-process proof. Report-associated
name changes, metadata sequence cleanup, public query guard context and TUI projection remain
required consumers of these transitions.

Decision tests and serialized traces cover stale/unsequenced reports, source conflicts, the public
native/official routing rules, session-only identity, clear/release/process preservation, pending
full-lifecycle references, old-session suppression, provider-specific replacement and delayed
observations. Generated custom-source traces check immutable preparation, watermark preservation,
foreign-clear isolation and absence of invented provider/process facts; failures print a reduced
replayable trace. Pinned-reference differential traces additionally compare complete report state,
effective views and release outcomes through the public routing rules. These prove the core only; matched Windows end-to-end reports, process observations,
query/TUI behavior and bounded registry admission remain required before claiming CLI/API parity.
