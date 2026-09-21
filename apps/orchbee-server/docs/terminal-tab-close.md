# Canonical terminal Tab Close

Status: accepted consequential local Close contract with strict schema and Rust/TypeScript bindings.
Server/Worker execution, retained maintenance, exact session cleanup and canonical publication are
integrated, with deterministic carrier and Linux protocol-client process evidence. Native Close
control and membership installation are integrated; Server-side PanelSplit and aggregate Close now
have authority tests, a bounded model and a Linux real-terminal witness; the
[native Session contract](../../orchbee/docs/native-tab-session.md) records the Linux TUI
Close-to-Empty, manual Create and retained-shell reentry witness. These bounded checks do not
establish release readiness or production restart recovery. This Server-owned reference
governs the lifetime workflow, cross-owner settlement and refinement obligations for Server, Worker,
protocol and native Client maintainers. Update it with those implementations and their evidence.
[Workbench authority](workbench-authority.md) continues to govern the current
canonical projection and strict standalone membership.

## User outcome and lifetime authority

### Workspace lifetime extension

The local Server accepts Close for a simple Terminal Context owned by a canonical Workspace,
using the same grant, exact binding/runtime identity, acknowledgment, retained Read/Resume,
continuation budgets and cleanup-before-removal contract as standalone. No wire field or Worker
operation is added. Workspace native UI remains outside this extension. Directory availability
does not authorize or block Close; exact live runtime identity still gates a new primary.

`WorkbenchTerminalMembership` and `WorkbenchTerminalTab` describe strict single-leaf members for
legacy operations and exact tree members for the accepted PanelSplit/aggregate Close path in either
context. Membership carries an immutable context kind derived from canonical navigation; it is a
read result, not another authority. Unsupported groups, pinned/grouped Tabs, children or missing
bindings retain the existing divergence behavior. Standalone composition helpers reject Workspace
membership themselves, in addition to conditional callers. This preserves their owner boundary
after generic type names replace standalone-only names in the unpublished library.

Workspace admission and publication never read, retarget or settle the standalone bootstrap slot.
After known Dispose and exact target cleanup, publication re-extracts current target-context
membership and removes only the retained Tab/Panel/Presentation/binding tuple. Sibling Create,
Rename and Move, and changes in other contexts, survive that commit. A pending sibling Create
settles through its original token and retains its original result revision. Workspace publication
returns no standalone-change notification. Removing its last Tab leaves the Workspace and its
Directory reference intact with an empty context; no replacement launch occurs. A subsequent
explicit Workspace Create follows existing directory checks and allocates fresh identities.

Close primary capacity and revision credits are Site-wide: every non-Closed primary reserves one
future revision in the shared ledger, and publication consumes exactly that credit with one
revision increment. Admission, aliases and exact replay do not advance revision or duplicate
cleanup. Closed tombstones and immutable acknowledgments survive later context changes. Failed
or unknown disposal cannot publish removal; known disposal with blocked cleanup remains retained
for the existing explicit Resume path. This extension adds no persistence, remote execution,
cross-context transfer or recovery of unestablished runtime identity.

`WorkspaceTabClose` owns bounded context isolation, shared credit and current-membership
publication evidence, including sibling changes, three standalone slot states, last removal and
explicit recreation. Its two Close targets share one Workspace; one ordinary write in another
context exercises the shared revision. Simultaneous primary Close credits in different contexts
remain concrete authority-test evidence. Existing Close models retain detailed Host continuation, Resume and cleanup
obligations. Authority Workspace tests establish exact identity, replay and concurrent credits;
Site/carrier Workspace tests establish pending Create completion, all three standalone slot states
and exact cleanup isolation. The Linux process test
`real_workspace_close_preserves_standalone_and_can_recreate` observes the target shell PID disappear,
executes input through the unrelated original controller, checks both Clients' snapshots and
recreates a fresh Workspace terminal in the retained directory. These are local protocol-client
witnesses, not native Workspace UI evidence. Full verification and independent review precede
integration. The unpublished Server/library cohort activates and rolls back
together; rollback cannot resurrect a terminal already disposed by an accepted Close.

`Ctrl+B x` closes the selected canonical standalone Tab after localized confirmation that its
terminal execution ends for **all attached Clients**. Cancel changes nothing. An independent
trusted-local lifecycle grant authorizes this effect; navigation permission or Input controller
ownership alone does not. A read-only Viewer may Close if its Client has the lifecycle grant.
The target is an exact retained binding, never an ordinal. Creating/Starting targets are definite
pre-effect `terminal-busy` until settled identity is available. An unestablished live Worker
generation or exact Host identity permits neither killing nor canonical removal.

Closed means known Host Dispose, exact Worker/Server target-cache cleanup, and canonical removal.
It does not mean hiding a Tab with an unaddressable process. There is no automatic replacement.
Last-Tab removal leaves a usable **Empty** native Workbench: no selection, Viewer, controller or
provenance; Refresh and manual `c` remain available. A later bare Open is independent and may
bootstrap a fresh terminal. Remote authority, unknown-identity adoption and changed-epoch recovery
are excluded. Server restart persistence belongs to the separate persistence work.

The local transport integration grants `manage_terminal_lifecycle` independently of read,
navigation, Create, Viewer and controller authority; only trusted-local composition explicitly
sets it. Close and Resume require that grant; status Read requires read authority. Authorized
requests reach the retained Site/Workbench Close authority. Read/Resume check the retained Site
identity before operation lookup: wrong Site is Denied. A matching Read reports the retained
operation or NotClosing; unavailable authority remains unavailable. This identity check survives
publication retirement and does not materialize a snapshot. Maintenance availability gates new
primary acceptance and fresh Resume after retained replay/alias/receipt decisions; it does not
erase old acknowledgments or status. Existing Create/Move adapters
reject Close-only reasons, accepted outcomes and response envelopes as action-incompatible.

### Workspace scope Close

`workspace.close {projectId, workspaceId}` admits a scope Close over the named
Workspace's Terminal Context under the same lifecycle grant, fingerprint,
replay/join precedence and revision-credit rules as a member Close. Admission
resolves the current `(project, workspace, terminalContext)` triple, freezes the
context's complete ordered leaf membership — an Empty context with zero members is
a valid scope — and reserves one credit plus the bounded retained record. An open
member Close, an admitted-but-uncommitted structural write, or another scope Close
covering the context refuses `terminal-busy`; the retained record then fences
later member closes and structural writes on that context until publication or a
terminal status. A same-scope fresh command joins the operation with its own fixed
ledger acknowledgment; an identical retained fingerprint replays it.

Acceptance returns `workspace-close-accepted {closeRequestId, projectId,
workspaceId, terminalContextId, acceptedRevision}`: the primary command request ID
is `closeRequestId`, and `acceptedRevision` is the admitted base, exactly as for a
member Close. The effect-free `workbench.workspace-close.read {protocolVersion,
requestId, siteId, projectId, workspaceId}` allocates no ledger entry and returns
`not-closing`, `denied`, `unavailable {reason:server-busy}`, or
`available {closeRequestId, projectId, workspaceId, terminalContextId,
attemptGeneration, status, counts, members}`: `members` preserves the frozen
admission order with per-member lifecycle truth only — the close revision belongs
solely to the scope `status` — and `counts` aggregates the same rows. Scope lookup
includes the closed tombstone, so a scope already removed by publication still
answers `available`. `workbench.workspace-close.resume {protocolVersion,
requestId, siteId, projectId, workspaceId, closeRequestId,
expectedAttemptGeneration}` applies the identical generation discipline and
reserved control storage, echoing the scope fields in every result.

Once every captured member is known-disposed and exact cleanup is done,
publication removes the Workspace and its Terminal Context together in one
canonical commit and records the Closed tombstone. The Project, its directories,
the directory association and the Git checkout are never touched, and no member
row carries the close revision.

## Close command and retained status

The existing `workbench.command` envelope and full fingerprint
`(requestId, siteId, expectedRevision, action)` admit
`{kind:"tab.close", terminalContextId, tabId}`,
`{kind:"panel.close", terminalContextId, tabId, panelId}` and
`{kind:"workspace.close", projectId, workspaceId}`. Precedence is lifecycle grant; exact
replay/conflict; retirement for a new mutation; ledger entry capacity; Site/base/context/strict
membership; exact target and terminal association; Busy/unavailable runtime identity; resource
reservations; joint acceptance. Accepted Close replay remains readable after Workbench retirement.
Replays have no effects.

A Panel close resolves one exact leaf member inside the named Tab. When that leaf is the Tab's only
member, the command admits as an ordinary Tab close over the whole membership; otherwise it admits
as a leaf-scoped close over exactly that member. An unknown `panelId` on a surviving Tab returns
`PanelUnavailable`; a missing Tab returns `TabUnavailable`. Every scoped close still freezes its
captured leaf membership at admission, disposes and cleans each captured leaf through the same
continuation, and joins or refuses overlapping closes: a same-action replay over the same member
set joins with its fixed acknowledgment, while any open Close already covering a requested member
under a different action or membership is `terminal-busy`. Disjoint leaf closes on one Tab may
therefore retain separate primaries. An open aggregate Close also fences the same Tab's
`panel.split` and `panel.move` admissions: both are refused the transient `terminal-busy` rather
than being allowed to invalidate the complete ordered leaf membership that publication re-proves.
A leaf-scoped Close re-proves only its own captured leaf tuple, so it fences neither operation, and
operations on other Tabs are unaffected.

Acceptance spends no canonical revision. Its fixed outcome is
`tab-close-accepted {closeRequestId, terminalContextId, tabId, acceptedRevision}` or
`panel-close-accepted {closeRequestId, terminalContextId, tabId, panelId, acceptedRevision}`,
echoing the admitted action's fields. The primary
command request ID is `closeRequestId`; `acceptedRevision` is that primary command's admitted base,
not removal. Every alias acknowledgment preserves this canonical operation's original
`closeRequestId` and `acceptedRevision`, even when the alias has a newer expectedRevision; its own
full request fingerprint remains retained. Clients validate the target/canonical operation and
`acceptedRevision <=` their admitted base, rather than requiring equality for an alias.
Primary acknowledgments require equality with the admitted base. Every canonical receipt must
have `acceptedRevision < u64::MAX`: primary acceptance reserves a removal revision under
`r + c + 1 <= M`. An alias admitted at `u64::MAX` may still acknowledge an older accepted revision.
A fresh current-base Close of the same visible closing target joins the primary operation with
its own fixed ledger acknowledgment, but never resumes it or reserves a second lifetime owner or
revision credit. A fresh Close of an already removed target returns `TabUnavailable`; retained
replay/status still describe the old operation. Existing typed Denied/Rejected/Unavailable apply;
the precise added pre-effect Unavailable reasons are `terminal-busy` and `terminal-runtime-unavailable`.

`workbench.tab-close.read {protocolVersion, requestId, siteId, terminalContextId, tabId}` is
effect-free and uses Site read authorization, including after publication retirement. It allocates
no command-ledger entry. `workbench.tab-close.result` echoes the read identity and returns
`not-closing`, `denied`, `unavailable {reason:server-busy}`, or
`available {closeRequestId, terminalContextId, tabId, attemptGeneration, status}`. Lookup includes
closed tombstones; monotonic Tab IDs cannot alias them. Read authority does not authorize Resume.

| Status                                             | Retained meaning and permitted continuation                                                                                                             |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `closing {phase}`                                  | `queued`, `waiting-grace`, `waiting-outcome`, or `disposing`; Server owns continuation. Signal acknowledgment is not lifetime completion.               |
| `cleanup-blocked {reason,resumable}`               | Known state parked; only explicit Resume can restart an eligible attempt for the same identity and phase.                                               |
| `outcome-unknown {phase,reason}`                   | Terminate, Force or Dispose may have crossed the Host boundary. Preserve membership and known facts; no blind resend, deletion or replacement identity. |
| `disposed-awaiting-publication {reason,resumable}` | Dispose is known; only remaining exact local cleanup/publication may run, never termination again.                                                      |
| `closed {revision}`                                | Exact disposal, target-owner cleanup and canonical removal committed; immutable thereafter.                                                             |

Closed reason sets are:

- CleanupBlocked: `worker-unavailable`, `host-refused`, `cleanup-incomplete`, `inventory-incomplete`,
  `identity-conflict`, `continuation-budget-exhausted`, `resume-generation-exhausted`.
- OutcomeUnknown: `transport-lost`, `deadline`, `invalid-response`; phase identifies the affected
  step, exactly `terminate`, `force`, or `dispose`; a query failure alone is not a new unknown effect.
- Pending publication: `target-cleanup-pending`, `preparing`, `preparation-failed`, `authority-retired`.

Do not serialize arbitrary errors. A correlated Host typed failure maps conservatively to
host-refused/cleanup-incomplete, not a claimed no-effect rejection. Native status polling retains
the addressable target after blocked/unknown results and offers status Refresh. Retry Close needs
a displayed resumable status and fresh confirmation. Lost acceptance is recovered by status or
original command replay; aliases, reads and command replays never resume work.

The read result has exactly `kind`, `protocolVersion`, `requestId`, and `outcome`; only Available
contains the target fields above. A Read or Resume Site mismatch is Denied before target lookup.
This does not change the Close command's existing SiteMismatch precedence.

## Resume with reserved control storage

Primary admission reserves one bounded control cell and one last-accepted Resume receipt, including
maximum encoded response storage. Recovery requires no ordinary ledger entry or alias allocation.
The Close-specific control is
`workbench.tab-close.resume {protocolVersion,requestId,siteId,terminalContextId,tabId,closeRequestId,expectedAttemptGeneration}`;
the Workspace-scope control `workbench.workspace-close.resume
{protocolVersion,requestId,siteId,projectId,workspaceId,closeRequestId,expectedAttemptGeneration}`
follows the identical rules and echoes its scope fields.
It uses existing request-ID types, the lifecycle grant and renewed all-Clients confirmation.
`workbench.tab-close.resume.result` echoes request ID and canonical target and returns
`resumed {attemptGeneration}`, `stale {attemptGeneration}`, `denied`,
`unavailable {reason:server-busy}`, or `rejected {reason}`. Rejected reasons are
`operation-unavailable`, `operation-mismatch`, `request-identity-conflict`, `generation-mismatch`,
`not-resumable`, and `generation-exhausted`.

Every Resume result has exactly `kind`, `protocolVersion`, `requestId`, `siteId`,
`terminalContextId`, `tabId`, `closeRequestId`, and `outcome`. Target fields echo the submitted
request even for Denied: on failure they neither disclose nor confirm an existing operation.
Resumed and Stale refer to a validated operation. There is no expected-generation echo.
The [wire contract](../../../schemas/workbench/README.md#accepted-tab-close-wire-contract) owns
closed schemas, framing kinds and cross-language conformance; these parsers do not authorize effects.

`attemptGeneration` and `expectedAttemptGeneration` share one Close attempt-generation domain,
distinct from Workbench revision: nonzero u64 values,
encoded as canonical decimal strings in `1..=u64::MAX`. Initial generation is 1. Under the owner
gate, check the grant and exact operation/target, then apply these rules:

1. The retained accepted fingerprint replays its original Resumed acknowledgment without changing
   state or clocks; the same retained request ID with different fields is request-identity-conflict.
2. Otherwise, an older expected generation is Stale with the current generation; a future one is
   generation-mismatch. Equality requires a known resumable parked phase, no dispatched step awaiting
   settlement and the exact target dependencies. A nonresumable phase is not-resumable.
3. Checked increment, checked deadline preparation, running state and receipt replacement form one
   prepared commit. Resource/clock preparation failure is Unavailable with control state unchanged.
   No ordinary ledger entry, revision credit or new nonce identity is allocated.
4. Increment never wraps. Exhaustion retains known facts and refuses Resume with generation-exhausted;
   a blocked operation exposes nonresumable resume-generation-exhausted. A disposed operation retains
   its disposed classification and the generation-exhausted refusal.

One generation plus the phase and last accepted receipt prevents delayed retries from resuming a
later attempt. There is no immutable replay promise for all refused controls or accepted receipts
older than the retained one: an older accepted request becomes effect-free Stale after replacement.
Native never automatically repeats a definite refusal. A lost reply may replay the same control;
Resumed acknowledges that attempt, while a status read establishes its current phase.

## Server continuation and exact Host lifecycle

Each visible Tab has at most one Close owner, with 256 Tabs per context. Site-wide primaries and
aliases share the 4096-entry and 8 MiB command ledger limits. One
maintenance owner serves the current single `LocalTerminalService`/Site. Each turn admits at most
one Worker step, yields on backpressure and holds no registry/Site lock while waiting. Round-robin
selection is not an OS fairness guarantee. Blocking receipt waits serialize Close steps in that
Site; other Client threads may write Site state and share the existing bounded Worker carrier.
Client disconnect does not cancel the Server-owned attempt.

Both production Site constructors start one named maintenance thread before Client admission;
spawn failure prevents startup. The service retains its handle and the thread holds only a weak
service reference between turns. Existing process-lifetime ownership adds no production Drop join
or shutdown budget. Acceptance commits before lifecycle IPC, and returning its acknowledgment
never waits for the maintenance reply; response delivery is not a dispatch gate. Owner availability
gates only new primary acceptance and fresh Resume, after exact replay/alias/receipt decisions.
Reads and retained acknowledgments remain available if that owner stops.

The wake flag is cleared before scanning retained canonical operations, and notifications set it
after owner gates are released. Waiting rechecks that flag and the earliest retained due time, so
a wake between scan and wait cannot disappear. The round-robin cursor stores only a request ID;
all operation state remains in the primary authority record. Backpressure uses the core's 250 ms
retry-not-before time without renewing its deadline. Pre-token identity/lease candidate failure
uses the same 250 ms wake cadence, with the core deadline checked before each retry.
A prepared exact step seal is retained before
`submit_lifecycle`; successful enqueue is charged using `admitted_observed_at`, even after expiry,
and its receipt is always awaited and settled outside transport-spanning owner locks. KnownDisposed
Resume validates the retained old target and local dependencies without requiring a live Worker.

An attempt has **30 seconds**, **32 submitted Host steps**, and **at least 250 ms between query
submissions**. Sample Server-monotonic time at atomic primary acceptance or successful Resume;
checked-add the deadline before commit. Queueing, backpressure, grace and all pages consume that
same elapsed budget. Before the next step, require `now < deadline` and remaining step allowance;
count a step when admitted to the carrier. Backpressure consumes time without a Host step, clock
reset or busy loop. Each lifecycle step contains at most one Host request; no hidden page/retry loop.

Preserve the existing independent budgets: Worker HostExchange is **1s**; Server carrier frame I/O
is **2s**, with a separate **20s response wait**; submission waits on `result.recv()`. The 30s
attempt budget governs next-step admission, not Host join, scheduling or total return time.
Never discard an already-dispatched receipt at expiry or Resume over it: settle its exact known
acknowledgment or postdelivery Unknown first, then park if the budget is spent. Close does not
rewrite carrier timeouts.

The Worker lifecycle-step boundary carries product binding, Worker generation, full Host identity,
operation/step correlation and fresh authorization; a Viewer ID is not lifecycle authority. Reuse
Host Terminate, Force and Dispose. Host ACK carries terminal ID, while its envelope supplies epoch
and correlation; the prepared seal and full OBCN result retain creation/Worker-generation identity.
Validate these actual fields rather than require nonexistent ACK echoes. Retain exact
lifecycle observations or obtain them through inventory; validating and discarding events is insufficient.

An inventory walk begins without a cursor and stays in one Host epoch/inventory generation.
Validate bounded page size and contiguous advancing cursor/generation; retain bounded target-match
facts. Every page, stale-cursor response and restarted first page spends the same step/deadline/query
budget. A stale cursor discards the incomplete walk and may restart only within remaining allowance.
Churn, exhaustion or oversized-summary refusal parks inventory-incomplete. Only full same-generation
exhaustion establishes absence; a partial page never does. Identity mismatch is a fenced conflict.
Even proven absence of a previously known same-epoch target requires its exact idempotent Dispose
acknowledgment before cleanup/removal. Stable inventory membership does not prove lifecycle freshness;
Host revalidates each mutation. Do not adopt unknown identities or new epochs.

For a running target: exact Terminate acknowledgment → fixed **five-second grace** → Force if no
settled outcome → exact Exited/Failed outcome → Dispose. Grace and known signal facts survive
attempt expiry/Resume: no renewed grace or repeated acknowledged Force. AwaitingStart goes directly
to Dispose; Exited/Failed still obey Host outcome-publication and cleanup barriers. A typed blocked
Dispose may be reobserved/retried through its idempotent known-state path after Resume. Ambiguous
delivery parks Unknown under existing fail-closed carrier rules, with no blind resend or replacement.

## Admission fence and cleanup in every owner

Close acceptance fences new target Attach, **Control Acquire**, Input, Resize and Start. Acquire
creates a fresh Host input/resize attachment: Server checks the lifecycle gate before reservation
and again before installing a returned grant. A pre-Close Acquire acknowledged late cannot restore
the target lease. Already admitted traffic settles under existing causal/Worker ordering; there is
no fictional global drain or successful Detach. Other terminals stay usable. Target-scoped Close
errors remain distinct from existing protocol corruption that may retire the shared carrier.

Host Dispose alone does not clear Worker `HostViewers`/`HostControllers` or Server controller
reservations, each currently bounded to eight slots. Closed must release target slots without idle
Clients polling or dropping, and preserve unrelated entries.

1. Before Dispose, prepare bounded exact-target cleanup storage/seal. Worker validates the Host
   acknowledgment and retains KnownDispose before nonallocating removal of only the target's viewer,
   controller and queued frame/feedback entries in the exact Host epoch/Worker generation. Typed
   Disposed is returned only after this cleanup. Late events/old Detach/Release settle idempotently.
2. Server validates the full result identity/correlation, retains KnownDisposed, and removes only
   the matching `ControlTarget {worker,generation,hostEpoch,terminal}` plus exact controller ID.
   Retain target revocation so old `ControllerLease`/`ViewerAttachment` settlement is idempotent local
   cleanup; it cannot send stale Host operations, retain occupied slots or delete newer grants.
   Controller lease Drop first reads the current reservation under a short controller gate. If
   that readable reservation is absent or belongs to another controller ID, the old lease settles
   locally without a carrier request or map mutation. Exact-owner cleanup keeps its existing
   transport path outside the gate; an unreadable gate is not evidence of revocation. This check
   does not cancel cleanup already admitted before revocation.
3. Preserve registry → Site lock order; the controller gate covers only short exact mutations.
   Never hold owner locks across transport or lease Drop. A local cleanup failure retains
   DisposedAwaitingPublication(target-cleanup-pending); publication waits. After cleanup, a
   publication failure retries publication only.

Lost Host acknowledgment retains fenced affected facts as Unknown. Lost Worker delivery after
local cleanup retains its known effect through result/exit handoff; Server reports only what it
observed. No broad map clear, new-generation cleanup, entry recreation or successful-Detach fiction
is valid at these cuts.

The Server session refinement is implemented and covered by the carrier/process witnesses below.
Viewer/controller owners retain the product Worker/Terminal IDs, Worker generation and full Host
creation/epoch/terminal identity. The single Workbench Close primary, including its Closed
tombstone, supplies the new-effect fence. Exactly matching DisposedAwaitingPublication/Closed
supplies local attachment disposal proof; a Viewer-issued Start fence is historical, while
maintenance cleanup validates the primary's entire accepted runtime including its fence. Close
admission resolves exactly one retained binding: Creating/Starting is TerminalBusy; unknown,
unavailable, missing or duplicate association is TerminalRuntimeUnavailable. A known identity also
requires the current Worker generation. Historical Started never implies that Host is still Running.

Preserve existing session/request/sequence/current-Worker checks. Newly fenced Attach uses
TerminalUnavailable, Control uses ViewerUnavailable, and Start uses TerminalCreateUnavailable.
Start applies that fence after existing AlreadyStarted/GeometryConflict decisions and before new
fence/identity/dispatch preparation. Registry → Site → controllers protects Acquire reservation
and returned-grant installation; Attach also installs under Site after revalidation. Failure leaves
the candidate owned until all guards are released. Already-admitted Input/Resize retains its actual
result. Exactly disposed Viewer Detach settles local absence as Detached; Poll/feedback becomes
TerminalRuntimeUnavailable, with no Worker cleanup request. Closing/Unknown alone permits no such
disposal-derived settlement.

For Server cleanup, the retained canonical request ID and full runtime are the pre-Dispose identity
seal. Under the same uninterrupted Site guard, validate KnownDisposed, capture and remove only the
current exact controller ID under one controller gate, release that gate, then mark authority
cleanup complete. An absent reservation is already clear. This local cut allocates no target/map
storage, performs no transport or lease Drop, and requires no live Worker. Failed proof or controller
access retains target-cleanup-pending. Old lease settlement preserves unrelated/newer IDs; it never
holds controllers while acquiring Site. Actual eight-idle-Client capacity release and late Attach/
Acquire process/carrier traces must exercise the maintenance consumer before delivery is claimed.

For a new Attach/AttachBinding, an existing exactly known-disposed Viewer may settle locally before
fresh attachment. Without that full old-target proof, preserve AlreadyAttached. First prepare the
fresh request through its existing syntax, authority, target, Worker and Close-fence checks; failure
leaves the old owner intact. Only successful inert preparation permits taking the old Viewer,
clearing its Detach intent and disarming its controller's Host Release from the monotonic disposal
proof. Drop those old values outside every owner gate, preserving exact controller-ID removal,
then dispatch the prepared fresh Attach with its normal late-install checks. No old Host cleanup
is sent; failed fresh attachment cannot restore the disposed Viewer. This reuses one of the eight
Client ReadySlots without Poll, explicit Detach or Client Drop. Its process witness is paired with
an actual service/carrier eight-owner observation of seven reservations immediately after Closed,
before any old-session action: implicit local settlement alone cannot prove maintenance freed it.

The Linux Server process witness holds eight original admitted Client connections, closes one
target, observes its exact canonical removal, and creates/attaches/acquires a fresh target on the
same connection without old Poll, Detach or Client Drop. An unrelated original controller still
executes a shell command. The separate actual service/carrier witness observes the seven original
unrelated reservations immediately after Closed, before any old-session action. Carrier tests also
hold Attach/Acquire and already-admitted Input/Resize across Close, and check that later fenced
effects spend no sequence or identity. Run `cargo test --locked -p orchbee-server` with the exact
Host/Worker prerequisites described by the Server test harness. These are Server protocol-client
and lifecycle-composition checks, not native TUI confirmation, Empty interaction, restart recovery
or installed-cohort verification.

## Revision credit and rebased publication

For current canonical revision `r`, accepted unremoved Close count `c`, and maximum revision `M`,
maintain checked **`r + c <= M`**. Primary Close requires `r + c + 1 <= M` and reserves one credit
without advancing revision. Every ordinary canonical write, including initial bootstrap/Create,
requires `r + 1 + c <= M`. Replay, no-op and Create terminal-result settlement spend no revision.
Final Close consumes its credit with `(r,c) -> (r+1,c-1)`; aliases/Resume spend no second credit.
Unknown retains its credit and addressable owner. Other writes may proceed and cannot spend it.

Before Host effects, reserve the primary command and maximum encoded command/ack/status/tombstone
charge plus Resume cell/receipt, including maximum revision representation and every bounded reason.
Prebuild fixed acknowledgment/classification storage. Aliases reserve their own fixed ledger entry;
all writers account for reserved entries/bytes. Reads spend none; finalization needs no later ledger
insertion. Serialized-byte reservation does not prove allocation, panic or crash atomicity.

The retained reservation is measured, never flat-rated: one checked sum of the
maximum legal wire cells — primary command plus acknowledgment, the worst legal
status document (a scope Close's document additionally charges one worst-spelling
member row per captured member plus separators), the worst Resume request and the
worst Resume result — plus the ledger key's owned capacity, the committed record's
actual controlled heap (the record body, every owned identity buffer, both
continuation/target `Vec` capacities at element size, and one bounded
lifecycle-request payload per captured member), and two same-shape maximum
receipt payload slots. A leaf or aggregate member Close applies the same formula
over its own captured membership; an Empty scope Close charges the fixed share
with zero member payloads. Resume measures the real candidate receipt before any
state change: it must fit its reserved payload slot, and the retained record —
existing receipt included — plus the candidate must stay inside the original
reservation; over budget is busy, never malformed.

Retain KnownDisposed and complete exact local cleanup first. Under the Site gate, validate current
applicable membership and the exact target tuple, then prepare from the **current** snapshot: an
aggregate Close removes every captured tree leaf as one Tab publication, preserving other Tabs,
order and labels. A leaf-scoped Close removes exactly its captured leaf: the leaf's parent Split is
replaced by the sibling subtree, retaining the sibling's own slot and Split identities, and the
surviving Tab keeps its ID, label and remaining members; when the closed leaf is already the Tab
root, the whole Tab is removed as for an aggregate Close. Releasing the gate requires
revalidation/repreparation. No old admission snapshot replaces later writes, no Site-wide
freeze spans grace, and the credit does not preassign the final revision.

Check actual encoded projection limits and delta/full-snapshot fallback; no future full buffer is
assumed preallocated. Preparation failure retains disposal, membership, credit and addressable status;
it is neither a pre-effect refusal nor permission to terminate again. Invariant-driven Workbench
retirement still permits cleanup status reads. Successful preparation commits membership, live Site
association, credit consumption and Closed together without further fallible work. Response loss
does not undo that commit.

## Reference-current Site completion

The reference retargeting in this section applies only to standalone membership. Workspace
publication commits its canonical removal without entering this slot-settlement path.

Keep `Empty | Creating(reference) | Settled {reference,outcome}` and retain historical Terminal
creation results/closed identities without ID reuse. At Create completion retain the precise
standalone settlement value for each reference; do not reconstruct it from today's lossy terminal
phase. Closing/Disposed facts are separate from that original completion classification. Existing
native canonical Creates use immediate Start.

If the live reference survives removal, preserve it. Otherwise choose the first member in the
**current post-removal order**: no member → Empty; Creating B → Creating(B.reference); settled B →
Settled(B.reference, retained B outcome). A Closing B remains a visible eligible reference until its
own removal. Tree composition requires every remaining leaf to match a retained binding, while the
current reference still selects one anchor for standalone retargeting.

B's original Create seal, request, metadata, binding, historical `is_first` and replay result remain
immutable. Under the joint gate, validate its exact Workbench token and Terminal association and
settle both records. Also settle the Site **iff its current Creating reference is B.reference**;
otherwise leave it unchanged. Historical `settles_first` is not current settlement or notification
authority. Return explicit `standalone_changed` to the service and notify after releasing locks.
This covers B completing before A removal and A removal before B completion. Other Creates continue
during Close; Closing Creating/Starting B is still Busy. Empty followed by fresh C uses new IDs;
old completion cannot overwrite C because both its seal and the current reference are checked.

The Site refinement retains `Option<CreateOutcome>` on each Terminal binding: exact Create
completion stores it once, and later Start/lifecycle phases do not overwrite it. None means pending
only in Creating; missing original outcome in another phase is coordination failure. Joint Create
completion returns `CompletedTerminalTabCreate {result, standalone_changed}` while preserving the
historical first-member association checks. The prepared Close retarget helper borrows only the
Site slot and consumes the authority plan's current-before membership, prepared-after membership
and exact removed member. It validates pre-removal composition, one exact removed tuple, the strict
removal view and one retained reference for a chosen binding before preparing the next slot. Both
owner commits remain infallible under the same Site gate; notification follows gate release.
The maintenance finalizer consumes this helper. Actual carrier traces cover B completing before
and after A's joint Close publication, and bare Open selecting the surviving B in both orders.

## Native removal and barrier composition

Same-revision snapshots remain exact. Higher revisions may remove IDs; surviving IDs preserve their
immutable tuple/binding/Presentation label while Tab label/order may change. A leaf close likewise
appears as a higher-revision contraction: the surviving Tab keeps its identity with exactly one
fewer leaf, the removed leaf's parent Split replaced by the sibling subtree. A native refresh admits
that contraction only as the exact sibling replacement of the frozen baseline tree; any other tree
shape change, removed-survivor mutation, or multi-leaf loss is divergence. When the contracted leaf
was the attached selection the native session retires; a surviving attached leaf keeps its
attachment and viewport. An available higher
canonical snapshot proves membership removal without per-missing-Tab status reads. For own known
Closed, its target must be absent at every snapshot revision at or above `Closed.revision`.

| Observation                                     | Required settlement                                                                                                                                                                                                                                        |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inactive removal                                | Drop its provenance only; preserve active Viewer/controller/generation.                                                                                                                                                                                    |
| Active/desired removal                          | Settle or definitively invalidate its old attachment before publishing another selection. Choose the survivor at the old ordinal, else the last survivor, else Empty. Fresh Attach failure is retryable selected-unavailable, never implicit Start/Create. |
| Closed with unavailable follow-up Snapshot      | Preserve the known Close through worker handoff and retain recoverable projection/publication state.                                                                                                                                                       |
| Last removal and manual Create                  | Empty has no Ready proof. `c` captures geometry with an Empty origin; precommit failure restores Empty, commit does not Detach an imaginary Viewer, and typed created projection/outcome governs selection/Attach.                                         |
| Empty Refresh sees another Client's Create      | Select first canonical member via fresh Attach/retry; no duplicate bootstrap/Create.                                                                                                                                                                       |
| Exact Create-revision Snapshot                  | Require original baseline plus appended new member; missing old/new member is divergence.                                                                                                                                                                  |
| Later CreateSnapshot                            | Old/new members may have closed. Preserve known Create; select new member if present, otherwise settle superseded-by-removal using its original appended ordinal and the neighbor/Empty rule. Old Ready anchor is historical cleanup data.                 |
| Switch target/outgoing removal                  | Keep attachment-generation cleanup order and cache frozen through its outstanding lease. Definite already-gone may settle cleanup; ambiguity cannot. Select a current survivor with a fresh key, never resurrect an ID.                                    |
| Move result/recovery removal                    | Exact Move commit revision requires its target; higher revision may remove it. Preserve Move commit and settle removal. Precommit TabUnavailable also requires authoritative reconciliation.                                                               |
| Refresh/other flight before removal publication | Closing/DisposedAwaitingPublication may still be a member with unavailable attachment. Retain identity/status until a valid snapshot confirms removal. Precommit restoration cannot undo observed removal.                                                 |

Existing Create/Switch forward **45s** and total **50s** captured deadlines remain unchanged when
removal crosses their active barriers. After completed Refresh/Move/own-Closed transport placement
or explicit recovery, a validated candidate needing new attachment reconciliation captures exactly
one fresh **50s total** setup budget at candidate installation: **45s forward**, with the final
**5s reserved for exact cleanup**. This covers old exact cleanup before fresh Attach, following the
existing Started recovery policy. Progress or Busy never renews it. Inactive-only removal changes
no attachment and needs no setup budget. Timely typed UI placement ends the transport deadline;
UI consumption has no synthetic deadline. Symbolic model clocks establish ordering, not OS timing.

Structural flights retain exact consumption/cleanup fences. Waiting for one Client's Close does
not freeze another Client's canonical writes. No inter-read freshness or unbounded barrier theorem
is claimed; repeated admission and late acknowledgments require composed implementation traces.

## Model and implementation dependency graph

The following **finite model families are required**. Each owner must declare actual
domains, transition coverage, fairness and exclusions with immutable checker inputs/results before
dependent runtime integration. The Server model scope and executable checks are declared below;
remaining families still require their owning evidence.

| Model owner and finite family       | Required cuts and claims                                                                                                                                                                                                                                                                                                                              | Releases after accepted evidence                                                           |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Server coordination                 | `WorkbenchTabCloseCoordination` and `WorkbenchTabCloseCreateReference`: small revision/ledger/attempt domains with exhaustion, two Close credits and one outstanding carrier step; retained receipt/late result, retirement, current-state rebase, exact Create seals/current-reference settlement in both A-close/B-create orders and Empty/fresh C. | Server admission, continuation/publication and reference-current completion.               |
| Worker lifecycle and target cleanup | `WorkerTerminalLifecycleStep`: one exact lifecycle request/page, target/foreign identity and stale epoch/generation; identity and target-cleanup variants, bounded target/unrelated caches and old/new controller IDs, known/ambiguous ACK/cache/reservation cuts and late Acquire/lease settlement.                                                  | OBCN lifecycle adapter, Worker observations/cleanup and Server target-fence integration.   |
| Native Empty and removal barriers   | `NativeClientMembershipRemoval` plus existing CreateBarrier Ready/Empty variants: empty/nonempty, active/inactive/exact/later removal, attachment generations, bounded queues/IDs; Close with CreateSnapshot, Switch, Move, Refresh and result/UI/retirement handoff.                                                                                 | Native projection/state/Empty core, then Close flights, mailbox and localized interaction. |

Pure native Close transport validation may precede membership/state refinement. This cut validates
the accepted command, Read and Resume wire contracts, their correlations and fixed deadlines on the
existing mixed carrier. It neither admits a native structural flight nor authorizes lifetime effects,
replay, reconnect or UI recovery. Its next consumer is the separately modeled native Close flight.
Read failures are transport failures, not new Host outcome uncertainty; NotClosing remains only a
correlated observation. Cross-read generation/status consistency and retained Close facts belong to
the native state consumer. Runtime integration still requires the native model and composed evidence.

The private native adapters now preserve accepted primary/alias receipts, validate Available Read
identity and every Resume target echo, and enforce checked successor or strictly newer Stale
generations without interpreting replay as current status. Run
`cargo test --locked -p orchbee --bin orchbee local_client::tests::workbench::tab_close` on Unix.
Scripted-carrier tests cover aliases, refusal/status variants, fixed Resume replay, maximum attempt
generation, malformed/direction/size rejection, prewrite refusal, partial handoff and postdecode
deadline retirement. These tests establish the transport boundary; no native Close flight, Host
cleanup, Server continuation or recovery carrier is supplied by this adapter cut.

### Server finite evidence and refinement

Run the registered checks from the repository root:

```sh
node scripts/tla/run-models.mjs --models workbench-tab-close-coordination,workbench-tab-close-create-reference,workbench-panel-split,workspace-tab-close --jobs 1
```

`WorkbenchPanelSplit` also covers the leaf close cut: a Panel-scoped close freezes one leaf member,
disposes and cleans it through the same outstanding-step discipline, and publishes a membership
contraction that preserves the surviving leaf while the Split collapses to it.

[`WorkbenchTabCloseCoordination`](../formal/tla/WorkbenchTabCloseCoordination.tla) has two
configuration variants in the existing registry. `credits` explores two initially known exact
targets, one optional ordinary Create, ordinary Rename/Move, two simultaneous Close reservations,
and current-state publication. `resume` keeps one Close target and explores control histories.
Both use three ledger entries, seven abstract bytes (three reserved per primary, one per other
entry), revision offset `0..3`, one shared outstanding result, and generations `1..3`. Initial
revision zero abstracts an already-created two-member state; actual bootstrap still uses the
ordinary revision-credit rule. No state constraint or symmetry excludes an ordering.

| Owner claim                                                           | Checked property and action mapping                                                                                                                                                                         | Concrete refinement still required                                                                                                                     |
| --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Reserved removal capacity survives ordinary writes and aliases        | `RevisionCredits`, `ReservedLedgerCapacity`, `CanonicalAliasAcknowledgment`; `AdmitClose`, `JoinClose`, ordinary writes and `CommitRemoval`                                                                 | Every production revision writer, full command fingerprints, maximum encoded status/tombstone/control charge and allocation limits                     |
| Resume cannot restart old attempts or allocate another ledger entry   | `GenerationAndReceipt`, `RetainedReceiptPrecedesCurrentPhase`, `OlderControlCannotRestart`, `ResumeUsesReservedStorage`; explicit request ID/expected-generation input through `ResumeDecision`             | Nonzero-u64 canonical decimal parsing, maximum overflow, authorization/identity and clock-preparation failures                                         |
| Late results preserve effects and final removal rebases current state | `SingleExactOutstanding`, `OutstandingSurvivesExpiry`, `KnownEffectsAreMonotone`, `ClosedRequiresEveryCommit`, `RemovalRebasesCurrentSnapshot`; exact completion, cleanup, stale preparation and retirement | Actual Worker identity/seals, exact controller revocation, registry/Site/controller lock order, projection encoding and publication failure            |
| Attempts make bounded local progress without claiming cleanup success | `AttemptsExpireOrSettle`, `ExpiredSettledAttemptsPark`; weak fairness only for each local `ExpireAttempt` and `ParkBudget`                                                                                  | Monotonic clock, 30s/32-step/250ms accounting, inventory walks/churn and grace; no fairness assumption for Worker result arrival or successful cleanup |

The single submitted step per attempt is a finite continuation cut, not the full Host lifecycle or
inventory algorithm. Known-blocked and Unknown are distinct endpoints; a late exact Disposed result
is retained even after expiry. Worker Disposed abstracts the separate lifecycle model's exact
Host/Worker-cleanup result; Server cleanup is still required before publication. Reads/replay are
effect-free observations. Grant checking, invalid wire input, reason enumeration, full tuple
conversion and carrier parsing remain concrete tests, not additional abstract protocol claims.

[`WorkspaceTabClose`](../formal/tla/WorkspaceTabClose.tla) owns the bounded
Workspace-scope Close: one admission freezes the complete ordered leaf
membership — an Empty context included — under a structural-write fence that
holds in both directions while the operation is open, one bounded attempt window
with generation-fenced Resume, and one atomic publication that removes the
Workspace and its Terminal Context while the Project, directories, association
and checkout survive. A member reaching an Unknown terminal never publishes, but
the record stays joined, fenced and charged: `ScopeMemberUnknown` preserves the
reserved revision credit exactly like open/blocked, and `ScopeStateOwnsRecord`
releases credit only for absent or published-closed records. Required coverage
now includes `StructuralWriteAdmitted` plus the scope family
`ScopeCloseAcceptance`, `ScopeMemberCleaned`, `ScopeMemberBlocked`,
`ScopeMemberUnknown`, `ScopeResume`, `ScopePublication`, `ScopeCloseJoin`,
`ScopeRerunRejected`, `ScopeResumeStaleRejected`,
`ScopeResumeNonResumableRejected`, `ScopeStatusNotClosing`,
`ScopeStatusTombstone`, `ObserveScopeVacuousClose`,
`ObserveScopeMembershipCaptured`, `ObserveScopeResumed` and
`ObserveScopeTombstoneAfterRemoval`. These actions are registered requirements;
a recorded TLC pass over the extended coverage is still owed and is not claimed
here.

[`WorkbenchTabCloseCreateReference`](../formal/tla/WorkbenchTabCloseCreateReference.tla) explores
three never-reused lifetime identities, at most two live members, three retained Create records,
one Move, one Rename, and Started/Unknown creation outcomes. Started carries the independently
retained exact current-generation identity needed by Close; Unknown remains replayable and may be
the current Settled reference, but is not closeable or adopted in this bounded model. A captured
pending/final reply remains independent of the one legitimate pending-to-final ledger settlement.

Its pre-removal Create fields project to `WorkbenchTabCreate`: accepted request/fingerprint/base,
fixed result revision, tuple/creation label/historical `is_first`, dispatch count, and the exact
Workbench/Terminal seals. The post-removal extension retains those records after membership is
removed, separates original creation outcome from lifecycle, and stores the live reference/phase.
Existing Create/navigation modules and all their variants remain unchanged conditional slices.

`ExactCurrentReference`, `ExactCreateSeals`, `KnownIdentityBeforeClose`, `CapturedReplyIsExact`,
`AcceptedFactsAreImmutable`, `FinalCreateIsImmutable`, `CapturedRepliesAreImmutable`,
`CompletionUsesCurrentReference` and `RemovalPreservesCurrentSurvivors` check that relation.
Required actions include `CompleteRetargetedPending` and `RemoveReferenceAfterSurvivorSettled`
for both A-close/B-create orders, Closing/Unknown survivor retargeting, Empty then fresh C,
rejected old completion after C, and pending/final replay after settlement/removal. Close removal
abstracts the companion coordination model's exact cleanup/credit/preparation cut. Concrete Site
token validation, retained standalone outcome and post-lock `standalone_changed` notification need
deterministic/stateful refinement; this model does not assert Open-wait or Host-success liveness.

These checks establish bounded model safety and the declared local progress properties. They do
not release runtime integration without the mapped core, adapter and process witnesses.

Whole inventory walks, generation churn, grace and 30s/32-step/250ms accounting belong to Server
continuation modeling/refinement, not the Worker's one-step proof. Unknown forbids disposal-derived
cleanup or retry; it does not forbid legitimate exact lease cleanup. None of these families promises
cleanup success. Native symbolic forward/cleanup clocks encode the accepted deadline ordering;
existing no-removal models remain conditional slices and must stay checked.

Strict schema/Rust/TypeScript wire and corpus work follows this accepted closed contract. Runtime
lanes may proceed independently after their applicable model cuts; joint integration requires the
Server, Worker and native families, actual transition refinement, and process evidence together.
Preserve existing action obligations/evidence; finite Close checks do not replace prior families.

Required refinement witnesses include wire generation zero/noncanonical/max/overflow; full-ledger
Resume and stale/duplicate/exhausted controls; revision reservation/rebase and both reference-completion
orders; receipt after attempt expiry; inventory churn and complete last-page absence; all eight
idle-Client cache slots, exact old lease Drop and late Acquire; publication failure; real graceful
then forced descendant cleanup while another terminal survives; last Close → manual Create and
later bare Open; cross-Client removals during every barrier; lost results and repeated UI/worker races.
Tests must check actual numeric/encoded bounds abstracted by finite models. Failure mapping to the
closed reason set, inventory/carrier progress and measured cleanup timing need implementation evidence.

The unpublished local protocol/Server/Worker/native cohort activates and rolls back together.
Rollback cannot revive terminated execution or discard retained cleanup facts. Status and explicit
eligible Resume preserve truthful recovery; persistence across Server restart, permanent Host/platform
failure recovery, changed-epoch adoption, remote operations, packaging and universal eventual cleanup
remain excluded. Model and process results must state these limits rather than claim an unbounded proof.
