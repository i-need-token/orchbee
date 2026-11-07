# Terminal Agent report transitions

The Server owns transient custom terminal reports separately from canonical hook observations in
`AgentRuntime`. [Native report commands](../../orchbee/docs/windows-panel-agent-reports.md) submit
source-attributed state; only correlated Host observations establish process presence or exit.

## Inputs and ownership

An author has an exact source and a normalized nonempty Agent label. Every source uses the same
custom-report policy. State is idle/working/blocked/done/error/unknown; explicit done/error describe
turn outcomes and are never inferred from screen matching or read acknowledgement. Source names
cannot grant provider identity, lifecycle authority or resumability. The public protocol admits only state, clear and release inputs; unknown kinds or fields are rejected before a transition.

One aggregate belongs to one physical terminal generation. It retains current authority, source
sequence watermarks, detected provider, fallback evidence and the latest confirmed exit. The core
has no I/O, clock, canonical Session allocator, Names registry or persistence. The caller supplies
owner-clock ticks; process observations retain their sample-start tick so an older exit cannot erase
a newer custom report. Preparation returns a candidate without changing the input.

Presence-only process confirmation preserves fallback and its evidence time. Exit, provider change
or a confirmed post-exit replacement clears old fallback when no fresh screen evidence is supplied.
A missing, failed or ambiguous Host sample is not an exit.

## Transition decisions

| Input   | Decision                                                                                                                                              |
| ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| State   | Reject a known provider conflicting with the detected process or a confirmed exit; then require a fresh source sequence and replace custom authority. |
| Clear   | Require the current matching source, or omit source to select it; sequence gates clearing authority.                                                  |
| Release | Reject foreign authority; require the matching effective Agent and fresh sequence. Preserve an independently confirmed live process.                  |
| Process | Reconcile provider/fallback evidence and clear matching exited-generation reports while preserving a newer custom report.                             |

Sequences use the full unsigned 64-bit domain encoded as canonical decimal strings. After a source
accepts a sequence, equal/lower and unsequenced reports are stale. Clear retains its watermark;
confirmed exit may reset the matching report source. A newer visible blocker may override the
reported state without inventing a report source. Effective views contain Agent label, state,
authority source and message. Clearing reports does not end a canonical Agent Session.

## Completion remains Done until later typing

The Session retains completion for the exact terminal generation. A Working turn that reaches Idle
publishes `awaiting-input`; Clients present green Done while preserving raw Idle for Agent rules.
Input accepted during Working cannot consume the future completion, including typing ahead of an
Idle observation. A transient Unknown does not lose that turn. The next Working observation starts
a new turn and clears the previous completion.

An error episode keeps an errored turn red. It starts when an Agent's effective status becomes Error
and ends when a new turn starts from any source (a Working observation, including a detection
fallback hidden by an explicit authority), the Agent changes, the process exits, or the authority is
cleared. During the episode, raw Idle and Done are presented as Error and never set
`awaiting-input`; Blocked is not overridden. After the episode ends, a Working turn that reaches Idle
publishes `awaiting-input` again.

When a new turn ends an error episode while an explicit Idle or Done is still current, that stale
status stands in for the turn: it is presented as Idle and publishes no `awaiting-input` until the
turn signals completion. The signal is an Idle or Done report that arrives after the turn began, or a
fallback that turns Idle. A signal that a blocker masks completes the turn once the status is visibly
Idle. The stale status reappearing when a blocker clears is not a signal.

Only accepted keys, text or paste **after** `awaiting-input` acknowledge it. Focus, read/unread
acknowledgements, viewport visibility and mouse reports never do. Semantic Focus/Mouse records are
excluded explicitly. Raw controller input skips complete focus and mouse reports, including
concatenated reports; a record with typed bytes as well still counts as typing. This classification
does not change or drop the bytes delivered to the child. One-shot send uses the same raw-byte
classification and inspects only its confirmed accepted record prefix, so an unsent key cannot
acknowledge a preceding report-only send.

Raw classification treats complete report encodings in one input record as reports; other raw
bytes retain their key/text meaning. A lone Escape remains a key. This record-level boundary is a
vetoable decision: Clients should send complete generated reports, or use typed semantic records,
rather than splitting terminal reports across requests.

An input arriving just before the Server observes Idle may leave Done visible until the next typed
input or Working edge. The user explicitly accepts this ordering in preference to losing Done.
TUI terminal chrome, sidebar and Agent board, and the corresponding GUI surfaces all project this
Server fact; Client-local unread acknowledgement does not change completion.

## Commit and verification boundary

`changed` describes candidate state independently of applied/ignored disposition. Report and matching
Agent-guarded metadata cleanup prepare and pass revision, capacity and encoded-result admission
before either commits. Exit cleanup preserves global tokens and newer presentations. Late metadata
for a confirmed exited Agent remains stale until process confirmation.

Each terminal admits at most 128 retained sources and 512 KiB of encoded report state. The registry
admits 4096 terminals and accounts at most 32 MiB. Refusal never consumes candidate sequence;
no-ops remain available at capacity. Membership removal or positive physical replacement retires
resources. Missing Host facts do not. Independent checked report revisions leave Workbench and
canonical AgentRuntime revisions unchanged. Owner-clock nanoseconds use a checked origin at 2^63.

## Windows report carrier

`workbench.panel-agent` uses OBWP request/result kinds 35/36. Read permission admits up to four
unique Panel IDs; navigation permission admits reports. Writes require current logical membership,
expected binding and retained Worker/Host generation. Source/message remain literal; labels use
shared normalization. Requests are bounded by 16 KiB and results by 512 KiB. Public requests cannot
submit process observations. Receipts carry independent revisions, binding, disposition, change flag
and effective view; reads preserve requested order and nullable unbound rows.

Decision traces cover source ordering, foreign clear/release, process preservation, delayed exit,
visible blockers and immutable preparation. Generated custom-source traces retain watermark and
no-invented-process invariants. Owner/carrier traces cover capacity, revision exhaustion, atomic
metadata cleanup, exact member fencing and physical replacement. Canonical Hook behavior and ordinary
terminal restoration remain separate contracts.

## Windows process sample adapter

The Windows viewer adapter is the first live producer of trusted process observations. It consumes only a fully correlated successful Host `ProcessInfo` response after I/O, then holds the Worker registry and Session coordinator gates through a second physical-generation check and commit. A sample retains its request-start time, logical binding, Worker generation and Host creation/epoch/terminal identity. The Host pins process objects and verifies identical Job membership before and after collecting all rows. A typed failure, missing identity or ambiguous Agent chain is not an exit.

Executable/entrypoint recognition and ancestry selection are shared pure Agent input rules; Windows command-line parsing remains in each OS adapter. The coordinator retains the selected PID plus creation time and the latest admitted sample start. Older/equal samples cannot supersede newer observations. Complete membership can prove that a previously confirmed process has left that Job or its PID was reused. Exit and metadata cleanup commit as one owner operation; a replacement is confirmed from a subsequent sample, rather than partially committing two transitions. Failed owner admission does not advance the retained process identity. Repeated identity with unchanged explicit-report input revision does not churn report revisions; a new report permits renewed confirmation. Missing retained facts never generate an exit. Cache entries are bounded by current registrations and released with their physical membership.

Process-only reads supply provider presence without inventing fresh fallback evidence. The [screen detection model](agent-screen-model.md) defines the periodic live producer; names and provider execution remain separate owners. The Windows TUI consumes correlated report and metadata observations through [Panel report presentation](../../orchbee/docs/windows-panel-report-presentation.md). Existing CLI diagnostic process reads remain available even if the report owner cannot admit the sample; they do not promise report freshness.

## Periodic Windows sampling owner

The Windows Session owns one periodic sampler, started after restored terminals are launched and before Client admission. It also starts in the unmanaged Windows Session. Each turn selects at most eight distinct committed logical bindings, including primary and child presentations. Known Agents and ordinary discovery have independent sorted round-robin cursors. With known Agents, one discovery sample precedes the Agent samples so idle terminals cannot delay an Agent for a whole scan and discovery cannot starve. Turns target 50 ms with known Agents and 250 ms otherwise, including time spent sampling; a 100 ms between-target budget limits a turn while admitting at least one Agent after discovery. Required I/O and cleanup still settle even if that budget expires. These private parameters bound scheduling work, not end-to-end latency under arbitrary load. A successful scan with no registrations releases obsolete tracking. Target discovery and admission separately verify the retained Worker generation and Host creation/epoch/terminal identity.

The sampler owns one temporary read-only viewer at a time. It attaches, collects process and coherent live screen evidence and detaches through the existing carrier and Host validation path; it never acquires a controller, sends input, focuses a Client or acknowledges display frames. The coordinator folds admitted process and optional screen facts together. Required carrier completions must be consumed: cancellation stops new sampling phases, settles the in-flight response and releases the owned viewer. It never abandons a required completion or sends cleanup before the Attach outcome settles. An uncertain Detach retains one exact `PendingCleanup` and blocks new Attach attempts until confirmed Detached or positively Disposed; later attempts only repeat idempotent cleanup for that same viewer. Shutdown joins this owner before Worker shutdown; a cooperative deadline failure retains the thread handle and pending ownership for the same shutdown attempt to retry. Once the producer stops, remaining cleanup is explicitly owned by Session shutdown and released only after the matching Worker/Host stop and actual Worker exit. Startup rollback also stops it. Drop signals cancellation; the thread keeps only a weak Session reference between turns and performs no I/O under registry or coordinator locks. The same owner serves bounded on-demand reads on that one viewer: physical explain, a current directory, and the evidence of a [conditional Tab or Panel Close](terminal-tab-close.md#tab-and-panel). Each kind holds one active and one waiting request and refuses a third; a waiting read takes the sampler after at most one periodic sample, and a Close's read goes first because a person waits on it.

Missing targets, capacity refusal, transport failure and invalid samples preserve prior report facts. A later round is a fresh observation, not mutation replay. The scheduler does not authenticate provider identity, create canonical Agent Sessions or persist report state. Deterministic turn selection, transport effect traces, lifecycle/deadline tests and an isolated Windows Session without user-driven process reads establish its claims.

## Physical semantic observations

Physical Agent observations reuse the report owner for primary and child presentations. `observe`
pins presentation, Worker generation, Host epoch and Host terminal identity. Current membership and
retained physical proof must agree under Worker registry and Session gates; missing proof refuses the
read. A replacement cannot inherit an old cursor. Results include the effective report view,
independent report revision and terminal-local semantic sequence. Names do not prove interactive readiness. Explicit reports may include done/error; screen observations do not infer those outcomes.

The sequence starts at zero with unknown/no Agent and advances by checked addition only when the
effective Agent label or state changes. Message, watermark and sample-time updates alone
do not advance it. All report and process writers prepare the resource
and history before commit. History retains at most 128 changes and 64 KiB per physical resource,
counted within the existing aggregate admission. Eviction advances an explicit floor. Empty report
state with history remains until membership removal or confirmed physical replacement.

Null `afterSequence` checkpoints the current head with no changes. A cursor at the retained floor
receives every subsequent change through that head in order; a cursor below it fails with
`history-limit`, and a future cursor fails with `invalid-request`. Reads never advance the sequence
or replace lost history with sampled success. Historical state is distinct from current state.
CLI physical Agent Get is the first consumer;
wait/prompt activity gates and Client attention remain dependent consumers, not completion claims
of this boundary. Traces cover quiet updates, transients, eviction, overflow, admission rollback,
generation reset and primary/child isolation. Shared wire vectors check closed shapes and cursor
coverage. State is transient; rollback replaces the matched cohort, without checkpoint migration.

## Exact Presentation reads

`read-presentations` reads up to four ordered unique primary or child Presentation IDs under the same read policy and Session gate as primary Panel reads. `presentations-read` retains independent Workbench/report revisions and wraps each exact member’s Panel row with its Presentation ID. Shared Panel IDs are valid; unknown members fail the whole batch with `not-found`. Unbound members have null binding/report; bound members without reports have the existing unknown view. Queries reuse binding-owned resources and never borrow the primary report. Empty reads fence report revisions. Existing primary read and write semantics are unchanged; callers must correlate Presentation, Panel, binding and Workbench revision.

## Exact Presentation writes

`report-presentation` names a Panel, one primary or child Presentation, its expected binding and
the existing report payload. Under the Session gate, the membership owner first proves that the
Presentation belongs to that Panel, then requires its nonnull matching binding and retained
physical fact. Wrong owners, missing members, unbound targets and stale bindings fail before the
report transition. This reuses the binding-keyed owner, source sequence, resource admission and
physical-generation lifecycle; canonical structure and other members' reports stay unchanged.
`presentation-reported` echoes Panel, Presentation and binding plus the existing independent
revisions, disposition, change flag and view. Result byte admission still precedes commit.
Navigation permission is required, as for legacy primary-only `report`. Exact-member traces cover
isolation, replay, wrong owner/binding and unbound targets. No durable migration is introduced;
replace the matched cohort to roll out or roll back.
