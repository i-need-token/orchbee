# Terminal Agent report transitions

This model owns explicit terminal report facts: source authority, provider Session references,
source watermarks and suppression of stale lifecycle reports. It is distinct from canonical
hook-observation Sessions in `AgentRuntime`. The core now serves a bounded Windows Server registry and the OBWP report/read family.
The [native CLI commands](../../orchbee/docs/windows-panel-agent-reports.md) submit these reports.
Panel/Agent queries join the real report view and metadata guards under independent revision fences.
Correlated Windows Host samples from the Site-owned periodic sampler and diagnostic viewers feed the trusted process operation. Names and TUI publication remain integration obligations. The public wire cannot submit process observations.

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

A trusted process observation carries optional fresh screen fallback at the same sample-start time. Presence-only confirmation preserves fallback and its evidence time; it cannot refresh an old visible blocker over a newer custom report. Exit, provider change or a confirmed post-exit replacement clears old fallback to unknown when no new screen is supplied. A fresh fallback retains the full-observation semantics.

The registry belongs beside terminal metadata under the Site/Workbench gate. A trusted process
operation prepares both report and metadata exit changes before committing either. Canonical
observation Sessions and their aliases remain with `AgentState`. The binary supplies the current
retained Worker generation and Host creation/epoch/terminal identity for each committed binding;
a Client-provided label, PID or generic observation category is insufficient. The Windows viewer
adapter calls the trusted operation only after a fully correlated successful Host sample; the core itself does not collect processes.

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

The live registry admits at most 128 retained source identities, 256 stale Session identities and
512 KiB encoded core state per terminal, 4096 logical terminals and 32 MiB accounted aggregate
state. Failure does not consume the candidate sequence. No-op reports remain valid at capacity.
An aggregate with no remaining report, process, sequence or history facts is reclaimed.
Independent report revisions use checked addition. Housekeeping retires absent primary/child
memberships or positively replaced physical generations; missing Host facts never mean exit.
Housekeeping has its own checked revision and may settle before a later request refusal. Shared
Workbench navigation and canonical AgentRuntime revisions are unchanged.

Owner-clock ticks are centered at 2^63 around the first committed admission Instant. Both sample
and admission time use that same origin, including a sample predating the first admission; checked
nanosecond conversion refuses overflow. No wall clock, wrapping or saturation defines ordering.

The trusted process commit also clears matching official metadata, older Agent-guarded presentation
and exited-Agent sequence ownership. It preserves global tokens and newer custom presentation.
The metadata owner retains presentation report time separately from per-field precedence and records
the Agent associated with accepted sequences, including token-only reports. Late matching metadata
is a stale no-op until real process confirmation. Both candidates pass capacity/revision checks
before either commits, so metadata or report revision exhaustion cannot leave half an exit.
Report-associated names and TUI context remain required consumers; native queries now join report and metadata state.

## Windows report carrier

`workbench.panel-agent` uses OBWP kind 35 for requests and kind 36 for results. Read permission
admits zero to four unique Panel IDs; navigation permission admits the four explicit report kinds.
Writes require the expected logical primary binding and a currently retained physical fact. Source
and message remain literal, Agent labels use shared normalization, and sequences are full-u64 decimal
strings. Invalid provider references and unknown start reasons normalize to absence as in the reference;
a valid reference remains distinct from canonical observation Session IDs. Windows composition supplies
path absoluteness without filesystem access or provider execution. Requests are bounded by the existing
16 KiB envelope; results by 512 KiB. No process-proof field is accepted on the public wire.

A receipt carries Workbench/report revisions, resolved binding, applied/ignored/deferred disposition,
actual change flag and effective report view. Reads preserve requested order and nullable unbound rows.
The owner validates the encoded receipt before committing a report candidate. A deferred full-lifecycle
report stays deferred until trusted process evidence arrives; no readiness is fabricated. Raw references
in decoded results validate source/kind/value eligibility; their platform absoluteness is a Server
publication obligation and must be rechecked before any future provider execution. The shared Rust,
TypeScript and JSON contracts retain older closed variants and are rolled out as a matched private cohort;
older peers reject the new kinds. State is transient, with source/cohort revert as rollback.

Decision tests and serialized traces cover stale/unsequenced reports, source conflicts, the public
native/official routing rules, session-only identity, clear/release/process preservation, pending
full-lifecycle references, old-session suppression, provider-specific replacement and delayed
observations. Generated custom-source traces check immutable preparation, watermark preservation,
foreign-clear isolation and absence of invented provider/process facts; failures print a reduced
replayable trace. Pinned-reference differential traces additionally compare complete report state,
effective views and release outcomes through the public routing rules. These cover the core and owner
admission only. The native CLI has separate carrier/command acceptance; actual Host observations,
name/TUI behavior and the final full regression remain required before claiming CLI/API parity.

## Windows process sample adapter

The Windows viewer adapter is the first live producer of trusted process observations. It consumes only a fully correlated successful Host `ProcessInfo` response after I/O, then holds the Worker registry and Site coordinator gates through a second physical-generation check and commit. A sample retains its request-start time, logical binding, Worker generation and Host creation/epoch/terminal identity. The Host pins process objects and verifies identical Job membership before and after collecting all rows. A typed failure, missing identity or ambiguous Agent chain is not an exit.

Executable/entrypoint recognition and ancestry selection are shared pure Agent input rules; Windows command-line parsing remains in each OS adapter. The coordinator retains the selected PID plus creation time and the latest admitted sample start. Older/equal samples cannot supersede newer observations. Complete membership can prove that a previously confirmed process has left that Job or its PID was reused. Exit and metadata cleanup commit as one owner operation; a replacement is confirmed from a subsequent sample, rather than partially committing two transitions. Failed owner admission does not advance the retained process identity. Repeated identity with unchanged explicit-report input revision does not churn report revisions; a new pending report permits renewed confirmation. Missing retained facts never generate an exit. Cache entries are bounded by current registrations and released with their physical membership.

Process-only reads supply provider presence without inventing fresh fallback evidence. The [screen detection model](agent-screen-model.md) defines the periodic live producer; names, persistence and provider execution remain required integrations. The Windows TUI consumes correlated report and metadata observations through [Panel report presentation](../../orchbee/docs/windows-panel-report-presentation.md). Existing CLI diagnostic process reads remain available even if the report owner cannot admit the sample; they do not promise report freshness.

## Periodic Windows sampling owner

The Windows Site owns one periodic sampler, started after restored terminals are launched and before Client admission. It also starts in the unmanaged Windows Site. Each turn selects at most eight distinct committed logical bindings in sorted round-robin order, including primary and child presentations. A one-second between-target budget and 250 ms wait prevent an unbounded scan or idle spin; these are private scheduling parameters, not freshness guarantees. A successful scan with no registrations releases obsolete tracking. Target discovery and admission separately verify the retained Worker generation and Host creation/epoch/terminal identity.

The sampler owns one temporary read-only viewer at a time. It attaches, collects process and coherent live screen evidence and detaches through the existing carrier and Host validation path; it never acquires a controller, sends input, focuses a Client or acknowledges display frames. The coordinator folds admitted process and optional screen facts together. Required carrier completions must be consumed: cancellation stops new sampling phases, settles the in-flight response and releases the owned viewer. It never abandons a required completion or sends cleanup before the Attach outcome settles. An uncertain Detach retains one exact `PendingCleanup` and blocks new Attach attempts until confirmed Detached or positively Disposed; later attempts only repeat idempotent cleanup for that same viewer. Shutdown joins this owner before Worker shutdown; a cooperative deadline failure retains the thread handle and pending ownership for the same shutdown attempt to retry. Once the producer stops, remaining cleanup is explicitly owned by Site shutdown and released only after the matching Worker/Host stop and actual Worker exit. Startup rollback also stops it. Drop signals cancellation; the thread keeps only a weak Site reference between turns and performs no I/O under registry or coordinator locks.

Missing targets, capacity refusal, transport failure and invalid samples preserve prior report facts. A later round is a fresh observation, not mutation replay. The scheduler does not authenticate provider identity, create canonical Agent Sessions or persist report state. Deterministic turn selection, transport effect traces, lifecycle/deadline tests and an isolated Windows Site without user-driven process reads establish its claims.
