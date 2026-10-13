# Native Tab Session

Status: accepted implementation and correctness contract for the native Client Tab Session.
Native Move runtime has deterministic integration and Linux cohort process evidence. This remains
an unpublished local development path. Native Close has the bounded Linux process evidence below;
packaging and production recovery remain separate work.

[Native context selection](native-context-selection.md) owns Workspace navigation, per-Client Tab/Panel
selection memory and the current-context projection extension. Its location identity, Empty selection and retained
provenance rules extend the standalone startup and same-context Tab workflows below.
[Native Workspace setup](native-workspace-setup.md) owns staged creation and first-directory input,
sharing this Session's structural admission, carrier and membership settlement.

## Accepted Close refinement and local integration

[Canonical terminal Tab Close](../../orchbee-server/docs/terminal-tab-close.md) owns the accepted
Server/Worker lifecycle, wire/status/Resume and native removal contract. Native `Ctrl+B x` targets
the selected Tab in the active context and confirms in both locales that execution ends for all attached Clients;
an independent lifecycle grant is required. Closed requires known disposal, target-owner cleanup
and canonical removal. The last removal leaves a usable Empty Workbench with Refresh/manual `c`,
without a replacement shell or Client exit.

The record specifies active/inactive removal, old-ordinal survivor selection, Empty Create origin,
known Close retention and exact/later CreateSnapshot, Switch, Move and Refresh settlement. A later
snapshot may remove IDs while preserving surviving immutable identities; an exact Create/Move
commit revision retains its stricter expectation. The common membership implementation and
registered removal/barrier models refine these rules. Earlier no-removal model slices remain
conditional evidence; exact UI consumption, cleanup and worker-exit facts remain mandatory.

## Close control and recovery contract

Windows selected/group/other/trailing Close is a bounded coordinator over these existing owners.
The [close-set contract](native-tab-close-sets.md) governs frozen targets, one confirmation,
the sequence cutoff, pre-install local identity fences, partial counts and the intentional
source-group-only cleanup boundary. It adds no batch wire mutation or replacement Close authority.

The native state owner retains Close knowledge independently of terminal selection and projection.
Linux native input, confirmation drawing, Close/Read/Resume execution and explicit status-only
recovery are integrated. Closed membership installation now uses the common exact-cleanup consumer,
and record pruning occurs only after both the exchange and reconciliation references are released.
The protocol adapter validates wire correlation; the native owner validates exact operation identity,
nonregressing attempt generation, retained disposal/Closed facts and UI flight identity. These checks
are complemented by the Linux multi-Client Close-to-Empty process witness described below.

Retirement at an unresolved submitted mutation, including cutoff or explicit retirement, preserves
local submission uncertainty and the exact identity for status recovery. A queued-only cancellation
or a definite typed precommit result does not invent submission uncertainty. Each joined worker
must have return evidence for that exact carrier; a fresh recovery carrier cannot borrow the old
worker's completed-return fact. Retirement preserves an already observed current-worker return.

Finalizing native retirement invalidates the displayed session once, even when unchanged geometry
observation discovers the closed mailbox before the next UI tick. A successful intervening draw
also consumes this notification; unchanged retired ticks do not redraw continuously. Retirement
clears the navigator's transient Refresh display together with its pending generation. The real
Poll-unavailable regression covers both a Ready screen and a queued Refresh screen, exact worker
join and the next redraw. It establishes visible retirement, not peer-removal recovery or the
separate multi-Client Close-to-Empty process tail.

| Boundary                      | Required native transition                                                                                                                                                                                        |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Initial Close                 | Capture the exact selected Session/context/Tab and canonical base. Render the localized all-attached-Clients warning before arming confirmation; coalesced unseen confirmation cannot submit.                     |
| Resume                        | Require the exact currently displayed resumable operation/attempt, a fresh rendered confirmation and revalidation at admission. Unknown is never resumable.                                                       |
| Cancel or local refusal       | Preserve selection, attachment, knowledge, request cursor and mailbox position.                                                                                                                                   |
| Accepted command              | Retain the canonical primary operation and accepted revision; an alias may acknowledge an older accepted base. Acceptance does not imply removal.                                                                 |
| Lost command or control reply | Retain the submitted identity and known facts; retire the ambiguous carrier without mutation replay. An early NotClosing read after ambiguous submission is inconclusive because acceptance may still be pending. |
| Read observation              | Denied/unavailable/query failure does not become Host outcome unknown. Known acceptance followed by NotClosing, changed operation or contradictory Closed revision is divergence; preserve known facts.           |
| Closed observation            | Retain exact target, canonical operation, attempt and immutable removal revision before any fallible Snapshot or UI handoff. Export the membership owner's exact target/minimum-revision expectation.             |

Successful terminal draws report exact opaque prompt and status tokens. Semantic status consumption
does not establish that the status was displayed. Every Resume, including on the original carrier,
requires both the exact current prompt and its eligible status to be shown. A complete Resume modal
renders that status itself when it covers the ordinary status line. Missing status revokes the
retained Resume prompt's shown qualification while preserving intent; renderer assumptions do not
replace this state-owner check. Every successful draw also reports absence or
incomplete visibility: a missing complete prompt revokes its shown qualification while retaining
the intent and facts. A later complete draw may re-arm the same current intent. Thus a complete
wide-window draw followed by a clipped narrow-window draw cannot authorize confirmation. Failed
draws emit no successful receipt and follow existing failure/retirement handling. Observing an
actual viewport or geometry change revokes both prompt and status display qualifications before
dispatching an already-read input packet; a resize followed by confirmation in the same loop
iteration cannot use the old draw. The shell signals any changed outer WindowDimensions through
`invalidate_close_display` before dispatch, even when a sidebar change leaves terminal geometry
unchanged; `observe_geometry` separately invalidates actual terminal geometry changes. Equal
outer and terminal dimensions do not invalidate on every loop. Intent and facts survive until a complete draw can re-arm them. Prompt-owned
input retains the remainder of its input packet after confirmation or cancellation, so trailing
bytes cannot reach the shell. Unrecognized prompt input, including Enter, cannot confirm.

The renderer measures the sanitized target, warning and actions with Unicode cell widths and
ratatui's actual wrapped line count in the final modal content rectangle. A grapheme wider than
that rectangle is incomplete even when the wrapper would omit it. Incomplete content shows the
localized resize/cancel hint and emits no prompt token. Both the complete modal and its hint
suppress the old terminal cursor before terminal cells are drawn. A status token describes exact
text still completely visible in the final frame; notice replacement, clipping or later coverage
cannot qualify it. A complete Resume modal supplies its own exact status token.

A definite Close or Resume refusal displays “Close request was not applied” / “关闭请求未执行”.
When a retained Close status exists, this local notice uses the secondary notice area and preserves
that status. Refusal alone grants no display qualification; the normal exact visibility checks
still govern a later Resume prompt.

The real-session Close interaction corpus drives raw packets through NativeUi, successful
TestBackend draws, the causal mailbox and a scripted wire peer. It covers both localized Close
confirmations, unseen coalesced confirmation, cancellation and packet-tail capture, incomplete
redraw, a sanitized 512-scalar Unicode target, and a same-loop 100-to-70-column resize whose
terminal width remains 68 columns. Resume coverage exercises clipped and notice-replaced status
plus the complete modal's two tokens. These checks establish the input/draw/admission boundary;
the scripted peer does not establish Host disposal or the composed multi-Client process witness.

Keep at most 256 target-keyed Close records. At capacity, refuse a new record as local Busy before
request or queue allocation. A record can outlive current membership while an unresolved result or
worker/UI handoff references it. Prune only settled removed records after membership consumption
and release of every handoff reference. Monotonicity concerns owned retained facts, not an
unbounded registry after permitted pruning. Settling A and admitting B before the worker consumes
A's acknowledgment must preserve both identities; a late A handoff cannot replace B's flight.

Known own acceptance suppresses new target Input, Resize and ordinary Poll while preserving exact
attachment cleanup identity. Already admitted causal work settles truthfully; unsent queued work
is not acknowledged as delivered. Other targets become usable only after exact fence settlement.
Each independently admitted command, Read or Resume captures one fixed 50-second transport/deposit
deadline. Earlier causal deadlines remain earlier. Timely typed placement ends that exchange
budget; UI semantic consumption has no timeout. A later Read obtains a new budget only after
the preceding exact key settles. Busy, progress and UI delay never renew an admitted exchange
or existing Create/Switch 45-second forward and 50-second total budgets.

One global round-robin scheduler serves retained Closing records and the bounded disposed class
below. There is one exact Read flight and at least 250 ms between actual admissions; there are no
per-record timers or threads. Selection changes do not erase another target's addressability.
The existing navigator Refresh intent also requests one bounded manual pass over retained
unsettled records, including removed targets. Its at most 256 target IDs are future intents:
allocate no request, queue position or clock until individual admission. Repeated requests coalesce.
On a live carrier the pass begins after the existing Snapshot Refresh semantically settles;
individual reads release their lease before further admission. Parked records remain selectable
through existing navigator rows; removed-target observations use the global status/notice area
with their retained target label. Stale status for an absent target cannot authorize Resume.

DisposedAwaitingPublication with reason target-cleanup-pending or preparing and resumable=false
remains automatically observable. Active publication and generation-exhausted parking can share
that wire shape. Retain disposed_auto_admissions per exact canonical operation/attempt, initially
zero and capped at 32. Each automatic Read admitted because of that status spends one slot
atomically with mailbox admission; later NotSubmitted, Busy or failure still spends it.
Pre-admission refusal spends none; the Read first revealing disposal is not charged retroactively.
Same-generation status changes, manual reads, selection, retirement, join and reconnect never
renew the quota. Only a strictly higher validated attempt may obtain a fresh quota. Other disposed
reasons or resumable=true require manual observation; Closing keeps its ordinary automatic policy.
Exhaustion produces only a localized automatic-observation-paused notice. Keep the Server status,
generation, known facts and Resume eligibility unchanged; manual Refresh remains available and
does not replenish the quota.

A normal trace is Closing, disposed(preparing,false), an automatic Read, then Closed. After
exact typed consumption, the membership owner validates a Snapshot at or above Closed.revision
with the target absent, settles exact old cleanup and installs a survivor or Empty. A successful
normal disposal/publication therefore needs no manual gesture. No eventual Close guarantee
follows when the finite observation quota, transport or Server continuation is exhausted.

Carrier retirement stops automatic scheduling. Only explicit user status Refresh may connect
again, after exact old-worker join, to the retained trusted-local endpoint/cohort and retained
Session/target. Mismatch is unavailable, with no adoption. This status-only phase cannot activate a
Session, Open, Create, Start, Attach or replay a mutation. Retain known facts across join and
reconnection; never revive the old Viewer. Reconciliation/Attach can begin only through the
membership owner's validated Closed/Snapshot handoff outside this phase. Restart persistence
and changed-authority recovery remain excluded.

After a successful exact Read on the fresh carrier has semantically settled and displayed the
latest eligible parked status for the same retained authority/Session/target/operation, a new user
Resume intent may leave status-only recovery for one explicit control. Its new confirmation must
actually render, and admission revalidates the status, generation and display token. Failed Read,
Unknown, NotClosing, an old token or unseen confirmation cannot open this control stage. Admit one
fresh Resume request and fixed exchange budget; retain the Server lifecycle-grant check. No old
mutation is replayed and no activation, Open, Create, Start, Attach or Viewer restoration occurs.
A lost reply retires this carrier again and retains the new identity. Exact result settlement
returns to status observation; another Resume needs a latest eligible status and fresh drawn
confirmation. This transition does not grant general session or changed-authority recovery.

While the close carrier is not the session — status-only recovery, or
MembershipRecoveryPending before its atomic membership install — the ordinary
pane-reconcile enqueue stays suppressed. The carrier mailbox's desired anchor is
not the session's retained anchor, so an ordinary admission would mint work the
carrier must reject as stale. The predicate is the close carrier itself, not a
session phase: CloseRead and ReconcileClose scheduling stay active, the
separately confirmed Resume control is unchanged, and the carrier resets only
through truthful retirement/finalization or the successful membership
consumption commit inside the reply transaction. Because the carrier swap keeps
the retained desired anchor untouched, ordinary scheduling resumes against a
consistent mailbox once that commit clears the predicate. Drawn pane
observations stay outside the canonical pane set entirely inside this window:
the carrier's Retired-phase shape requires an empty pane set, and the record
sync could mint an unattached canonical record under the retained desired
anchor — a fact that would poison the carrier's own next settlement. Latest
UI and mailbox geometry still updates through the admission-free focus-geometry observation
path outside the record sync; Resize is unreachable because no record is
attached; and the reconcile enqueue is separately suppressed at its own
funnel, so no ordinary work is admitted.

The Linux native recovery implementation retains the original local endpoint/cohort and the
Workbench request, attachment/Refresh generation and command/UI-wait cursors after joining the
exact retired task. The fresh mailbox copies these allocator positions, with no old queue, flight,
event or Viewer. A new explicit status Refresh
creates one fresh carrier, admits only Read and separately confirmed Resume exchanges, and never
runs bootstrap. The old retired attachment remains unavailable. Its already-finalized retirement
cause cannot retire a successful new status observation; failures from the new mailbox still do.
An early NotClosing after uncertain submission settles that Read but preserves the original
submitted exchange and unavailable observation. Only a validated canonical Available status can
resolve that uncertainty. Foreign target/status responses preserve the original identity and
retire the fresh carrier without adopting the response.

Real Unix-socket tests cover two consecutive lost mutation replies with two separate recovery
joins, fresh status and confirmation draws before Resume, recovery to Closed without attachment
setup, early NotClosing followed by explicit resolving Read, and foreign-target rejection. These
tests refine status-only recovery; they do not establish the separate Closed membership handoff,
record pruning, process termination or recovery on other operating systems.

The first semantically settled exact Read establishing Available Closed owns the immutable
Close reconciliation origin: its original flight key, canonical operation, attempt, Closed revision
and captured binding/attachment association. Repeated Closed observations cannot replace that
origin. A separate Snapshot marker pairs it with a fresh request ID and independent fixed 50-second
transport/deposit deadline. The common membership result validates both identities. Snapshot
failure retains the unconsumed origin; no raw absence or unavailable result proves cleanup.

An ordinary live origin retains one coalesced initial Snapshot intent across pre-admission Full or
Busy without allocating an ID/deadline. Successful admission spends that initial opportunity;
subsequent failure requires a new explicit `w`. Status-only Closed instead requires an exact
current status draw and new explicit `w` to enter MembershipRecoveryPending for that origin before
Snapshot admission. Full keeps that narrow stage and requires another explicit `w`; it grants no
normal commands. Only validated candidate, exact cleanup and atomic membership installation
permit a survivor attachment. A prevalidated Close-consumption token remains inside the exclusive
state install plan, and commits infallibly with membership. Record pruning waits for removal,
consumption and release of all exact worker/UI/handoff references. A private Closed attachment
proof binds the immutable target binding and exact old attachment generation; once real Detach
has been handed off, only its actual correlated result can finish that cleanup.

The native producer and common membership consumer now carry that marker through the existing
bounded command queue and single executor. A timely Snapshot placement remains consumable on a
late UI tick. An actual earlier UI-wait serial is stale before any mutation closure runs; combining
a current serial with a foreign correlation is a protocol violation and cannot install membership
or manufacture a worker acknowledgement. Depositing or taking a UI reply does not release the
worker's marker: exact lease completion, or the completed worker join, releases that reference.
An earlier Snapshot acknowledgement cannot release a later retry's consumed record. An
acknowledgement-only tick requests a redraw when pruning removes the retained Closed status.

`mailbox::tests::close_reconciliation` and `state::tests::tab_close::reconciliation` cover Full
admission, immutable origins, manual retry, stale/current correlation, exact cleanup proof and
consumption/pruning fences, including inactive removal preserving the survivor's Ready proof.
`ui::tests::membership` additionally checks the acknowledgement-only redraw and Snapshot placement
at time zero with UI consumption at 75 seconds, followed by Gone at 0, 44.999, 45, 49.999 and
50 seconds of the one newly captured reconciliation budget. The three Unix-socket traces in
`tests::close_recovery::close_membership` cover live Close → Closed Read → fresh Snapshot → Gone →
Empty, drawn status-only Closed plus a new explicit `w` → Empty, and Denied Snapshot plus manual
fresh-ID retry → Empty. These witnesses do not establish peer recovery, Server/Host process
termination, surviving-neighbor attachment, manual Create from Empty or other operating systems.

Required refinement includes actual eight-slot causal mailbox storage, the 256-record boundary,
32-read counts 31/32/33, checked generation/identity limits, same-generation nonrenewal through
failures and join, raw-key/draw confirmation in both locales, and A-settled/B-admitted/late-A traces.
Model fairness covers only local expiry, lock release, exact UI consumption and completed-worker
join; no network, lifecycle, reconnect or OS-success fairness is assumed. The composed Linux
witness must close a shell and resisting descendant from an authorized read-only Viewer while
another terminal survives, show removal to two Clients, then reach Empty and manual fresh Create.
These obligations do not authorize the manual focus-stealing native-window test.

### Control and handoff model boundary

Control and UI handoff use deterministic state, mailbox and connection tests with explicit delayed completion and retirement. The pane-incarnation core covers cross-incarnation effects; operation-specific local scheduling, focus and display decisions remain implementation-test obligations.

Retirement and stale-incarnation rejection tests capture the exact `PaneRef` before removal and
invoke the production state operation directly. The [legacy single-pane adapters](../src/terminal_session/state/tests/pane_identity_compatibility.rs)
resolve current keys and can reject before entering production code, so they cannot establish
post-removal or stale-incarnation rejection. The [retirement handoff tests](../src/terminal_session/state/tests/retirement_handoff.rs)
use retained references; [generated state traces](../src/terminal_session/state/tests/generated.rs)
separately cover removal and reattachment with a fresh incarnation.

## Canonical membership implementation boundary

The shared membership implementation replaces the earlier append-only assumption below. Pure
validation requires the same Session/context, nondecreasing revision and exact equality of Tabs,
navigation and projects at the same revision. A higher revision may add, remove, reorder and
rename Tabs; candidate Tab identities stay globally unique within the existing bound, and Panel,
Presentation and Terminal identities each stay globally unique. Within this Session/context, overlap
of any current and candidate Panel, Presentation or Terminal identity requires the same complete
immutable member tuple — Panel, Presentation, Presentation label and the complete Worker/terminal
binding — while the owning Tab is the one authorized delta: a member may relocate across Tabs
through folded legal moves. A Tab identity that survives must still own at least one of its
original members inside that same Tab; a vanished Tab needs none, so a Tab ID can never be reused
to alias a different member set. Worker ID alone is not a member identity: one Worker may own
several terminals. Genuine removal plus entirely fresh identities is allowed. This surviving-Tab
floor is a conservative consumer boundary, not a completeness claim over every reachable producer
history — a step-legal exchange that moves all of a Tab's members elsewhere is still rejected.
This compares two snapshots only and adds no registry of every previously removed identity.
Workspace shell recovery is an explicit exception for the Panel slot only: at a higher
revision the same Panel and Worker may carry both a fresh Presentation ID and a fresh
Terminal ID, with unchanged Presentation label. Neither fresh identity may alias a
member in the compared current or retained Context. Surviving Presentation/Terminal
identities still require their complete original tuple; changing only a binding or
moving an old Presentation into another Panel is rejected. Standalone Contexts retain
the original rule. This preserves the user's layout while keeping Presentation binding
identity immutable. Session allocators, rather than this two-snapshot comparison, own
lifetime non-reuse.

Replacement of any Panel in the active Tab uses the existing whole-anchor membership
cleanup and install path, retaining the selected Panel and zoom while minting a new
attachment generation. Old Viewer and
controller effects finish or are disposal-proven before attaching the fresh binding.
Replacement clears old Tab launch provenance; failed or unknown old startup never
becomes the new shell's startup state. Replacement in another Tab updates its canonical binding without detaching the active
Tab; selection later attaches the new binding. Tests must cover live and retained-context
refresh, fresh attachment selection, stale generations, simultaneous old/new identity
alias rejection and unchanged-Presentation binding refusal.

Exact Create and Move commit revisions retain their original strict expectations. At a later
revision the committed target may be absent, represented by an optional validated target index;
its historical commit is never erased. A projection-owned `ClosedTabExpectation` checks exact
Session/context/Tab and minimum Closed revision with the target absent. It proves membership only.
This expectation is implemented in `workbench_projection/closed.rs` with private identity fields
and read-only accessors. Its pure validator reuses canonical candidate validation, then checks exact
scope, minimum revision and target absence; tests reject wrong scope, old revision and target
presence. The Close state owner separately supplies the retained attachment identity and cleanup
proof when connecting its consumer.

Live Empty retains the validated canonical Session/context/revision and an empty Tab vector, with no
provenance entries, active/desired key, viewer/controller, projection, frame or Ready fact. Cold
bootstrap still requires a nonempty projection. `MailboxMembership::AwaitingBootstrap` is distinct
from `Live { desired: None }`; Empty permits manual Create and Refresh, plus Close-owned status and
retained-record observations that require no attachment. It never creates a replacement shell.

The mailbox discriminator is implemented independently of state installation: a live Empty mailbox
admits and dispatches Refresh while preserving the attachment cursor, rejects old attachment-tagged
commands, and rejects a second Bootstrap event. AwaitingBootstrap still blocks ordinary admission
and dispatch. Deterministic mailbox tests exercise these boundaries; the state owner supplies
the live Empty transition through the common atomic Refresh installation described below.

The state stores `desired: Option<AttachmentKey>` and exposes an optional reference. Existing
nonempty setup, cleanup, frame, feedback and mutation paths explicitly require their exact key;
they never manufacture a key when the option is absent. The existing nonempty Client tests remain
the regression boundary for this representation change. The membership installation below connects
Empty production and optional Refresh settlement.

The first live installation consumer is Refresh from an already truthful no-viewer stable origin,
plus Empty Refresh. `state/membership.rs` prepares canonical membership, survivor provenance and
preferred-ID/old-ordinal/last-survivor selection outside the reply mutex. Inside that mutex,
`mailbox/membership.rs` prepares the optional desired key and checked cursor; the state prepares its
matching cache/phase plan. Both plans exclusively borrow their owner until commit or drop. Two
infallible commits move the prepared values without intervening validation, allocation or I/O.
An attached target cannot be removed through this entry: it requires the pending exact-cleanup
continuation. Inactive replacement may preserve the same key and Ready/frame/geometry facts.

The actual UI Refresh reply can install Empty, settle a same-revision Empty refresh or definite
Unavailable with no key, then install a peer's genuinely fresh member using the cursor successor
and canonical binding. Changed selection samples its one fresh 45s/50s budget before locking;
checked deadline failure or reaching the forward cutoff prevents installation. Same-selection
replacement samples no new attachment budget. Timely typed placement may be consumed late; a UI
witness consumes results after 75s and 150s without renewing their transport budget. The witness
drives real UI admission, mailbox flight/result/reply and reducer paths, and checks matching desired
keys/cursor and absence of a queued Open/Create/Start. It does not execute the returned peer Attach.

Plan tests independently exercise drop, a rejected second plan, generation exhaustion, reservation
reuse and cold-empty rejection. The attached Refresh continuation now stages the exact result
generation, old attachment and validated candidate in `ReconcilingMembership`. Cache, selection and
attachment facts remain frozen, and Ready-based admission stops. The existing executor sends the
old viewer's exact Detach using the stored total deadline; only a matching Detached response can
produce the common cleanup event. The UI rechecks the pending result and old key, prepares both
owner plans, then installs Empty or a checked fresh successor. The pending candidate stays owned by
state while preparation is abandoned; its bounded copy is prepared outside the reply mutex.

Actual UI tests cover exact versus wrong-generation cleanup and consumption at 44.999s, 45s,
49.999s and 50s. From the forward cutoff onward, known cleanup permits retirement only, without
candidate installation or a new Attach. Scripted carrier evidence executes Snapshot, exact Detach,
Empty settlement or successor Attach through real Ready; an unexpected viewer response never
installs. Manual Create from Empty and the other Create/Move/Switch/Close origin consumers
have separate origin-specific refinements described below. This Refresh slice does not construct or consume a Close Gone
proof and does not change Switch supersession.

Create now carries an immutable `CreateOriginKind::{Ready, Empty}` and a real
`CreateAnchor::{Ready(ReadyAttachment), Empty}`. Empty admission reserves the next mailbox generation
and captures the current canonical scope/revision and geometry without inventing an attachment.
Definite precommit refusal restores Empty with no Resize, Detach or Ready publication. A known
Empty-origin commit retains its exact operation and provenance even when subsequent work retires
before publication. State, actual UI and scripted carrier witnesses cover this origin/refusal
boundary and the next peer selection after its burned reservation. Successful committed Create
uses the same common membership consumer for both origins.

The actual UI Create Snapshot handler now prepares canonical/commit validation outside the reply
mutex, retaining the exact baseline and committed operation in a non-Clone prepared result. The
reply transaction rechecks that baseline and current operation before staging the exact old cleanup
or installing an Empty-origin candidate. The executor obtains this Snapshot before any old Detach.
The former `CreateOldDetached` publication event is removed: only the common dual-plan installation
publishes a selection. A surviving C reuses the reservation; later absence selects a checked fresh
fallback or Empty. Typed Snapshot Unavailable keeps old attachment/projection facts frozen with no
Ready authority. Explicit Refresh recovery validates the retained commit again and samples one new
45s/50s reconciliation budget; timely typed placement permits late UI consumption without renewing
the completed transport budget.

Scripted carriers execute Started and non-attachable Create through candidate validation and exact
old Detach, Empty-origin Started through Snapshot and real Ready without a Create-time Detach, and
explicit recovery through late UI settlement, exact cleanup and the same reserved generation.
Actual UI tests exercise reserved C, a fresh fallback to old A, Empty, and newer admission/refusal
and pending commits that preserve the prior installed receipt. The generated mailbox lifecycle
fixture also uses candidate-first staging and the common installation. These are local refinement
witnesses; they do not establish Close Gone consumption or a composed two-Client Create scenario.

The candidate-first Create continuation keeps one `settled_create` receipt for the latest Create
whose membership was successfully installed. The common state-plan commit replaces this historical
receipt atomically with the canonical installation. Admission, queueing, definite refusal or a known
but uninstalled newer commit cannot replace it: pending B and settled A may coexist. Exact old
worker/UI references remain tied to their own operation. Later absence of the created Tab does not
erase the historical commit, and this receipt is not the current attachment or a retirement-shape
anchor. This is one retained receipt, not a registry of all Create operations.

One membership owner prepares a validated candidate, remapped provenance and selection by preferred
ID, then historical ordinal, then last survivor, then Empty. Empty-origin selection uses the first
survivor. Inactive-only replacement preserves the exact attachment, projection, geometry and cursor.
Other candidates freeze old state until exact attachment cleanup under the stored deadline owner.
Snapshot absence is never cleanup evidence. Close alone may construct the opaque exact-generation
`DefinitiveClosedAttachment`; it cannot override an already handed-off Detach.

`Preserve` carries the entire current `PaneKey` (Tab anchor and Panel ID), or `None` for Empty,
unchanged through both plans. It must not reconstruct the desired Panel from a surviving Tab's
first leaf. Ready selection, pane incarnation, selected presentation, controller, frame, geometry
and sibling records remain intact. `InactiveRemovalPreservesExactAttachment` and
`LabelOnlyPreservesShell` abstract this guarantee; the nonfirst Tab/Panel location-label trace
refines the concrete identity.

The existing UI-reply mutex owns atomic installation. A non-Clone state plan and a prepared mailbox
desired plan (`Preserve`, `ReuseReserved`, checked `Fresh`, or `Empty`) both validate before either
commit. Their infallible commits install canonical cache/provenance/selection and optional desired
key/cursor without intervening validation, allocation or I/O. Abandoning either plan changes neither
owner. Fresh post-transport reconciliation samples one 45s forward/50s total budget before that
mutex; Create/Switch retain their existing deadlines. Timely typed placement ends the transport
budget; delayed UI consumption cannot renew the stored budget.

This record releases the pure projection, Empty, mailbox sum and shared prepare/stage/install
boundary. Reordered Create and Switch execution remain dependent on their corrected owning models
and refinement review. The typed Snapshot-Unavailable blanket effect fence applies only to Create;
Move preserves its existing Ready effects and Refresh its existing typed settlement. Switch's
definite-Unavailable restoration remains an explicit reserved decision until its owner model and
stable-anchor proof are accepted. Implementation evidence is added beside these owners as each
consumer is integrated; this boundary alone does not establish whole Close delivery.

## Close model scope and refinement

Close behavior is governed by the canonical membership implementation boundary above and the [Server Close contract](../../orchbee-server/docs/terminal-tab-close.md). Native state/mailbox generated tests and connection/projection traces exercise active/inactive removal, Empty, stale completion, peer changes, Create/Move/Refresh barriers and cleanup. The retained pane-incarnation model covers only identity fencing and late cleanup; it does not claim exhaustive Close composition or prove the Server/Worker implementation.

## Accepted Move integration and identity rules

The native Move projector, mailbox, adapter, state, key and UI consumers implement the accepted
rules below. Their deterministic evidence is separate from full cohort process acceptance.
[Server ownership and evidence](../../orchbee-server/docs/workbench-authority.md#accepted-canonical-tab-move-model)
fix the reference-derived bootstrap composition, pure commit and rollout boundary.

Move targets the active selected Tab only, from Ready, SelectedWithoutAttachment or
RetryableAttachUnavailable with a live carrier and exact cache/selection identity.
`Ctrl+B <` and `Ctrl+B >` move toward previous/next; at a boundary the high-level action submits
`last`/`first` to wrap. Wire previous/next never wrap. Move admission allocates no attachment generation;
when its active target survives the validating projection, settlement preserves viewer/controller,
terminal projection and historical Ready proof. Ordinary frame
invalidation still does not add a fresh-frame prerequisite to Create.

Tab ID and `AttachmentKey { generation, tab_id }` are authoritative. An index is derived from one
accepted cache vector and may be retained only while that vector is frozen. Cache replacement
prepares provenance by exact ID, assigns BindingOnly to new snapshot-only IDs, resolves each
phase-required active/desired/committed ID exactly once, then atomically installs cache/provenance
and derives the selected index. Only validated correlated Create overlays creator provenance.
No retained ordinal alone decides identity, cleanup, feedback, replay, provenance or rendering.
For `[A,B,C] -> [B,A,C]`, active A moves from index 0 to 1 and
`[Started(A),Unavailable(B),BindingOnly(C)]` becomes
`[Unavailable(B),Started(A),BindingOnly(C)]`; attachment identity does not change.

Ordinary Refresh/Move installation is limited to its matching stable phase or exact explicit
recovery. Starting, Creating, CreateCommittedNeedDetach, Switching and Retired prohibit ordinary
replacement. Correlated CreateSnapshot validates the retained commit before staging old cleanup.
Explicit Create ProjectionUnavailable recovery validates the same facts and samples a fresh
operation budget for candidate cleanup and installation. Started may continue to setup within that
budget; non-Started installs without Attach. Cold Bootstrap owns initial installation. These exceptions never admit a second structural
flight or allow ordinary refresh to impersonate CreateSnapshot.

One admission-time `structural_mutation_pending` Move flight spans queued marker, command I/O,
SnapshotRead, typed event and UI settlement. Under the same mailbox/canonical-state lock, reject
Move if Create/Switch/Refresh/Move is queued, active, in flight or awaiting UI. Once admitted, reject
new structural markers until exact UI settlement or retirement; explicit recovery continues to
block order-dependent operations. Earlier Switch/Refresh similarly freezes the cache through its
setup/cleanup/UI settlement. FIFO alone is insufficient: never construct Switch(B, old index 1)
behind Move(B, first). Input/Resize/Poll/causal feedback keep existing generation/Tab-ID FIFO rules;
later effects cannot execute while Move awaits semantic settlement. `n`, `p` and numbers use latest
applied order; while Switching, n/p anchor to its desired Tab ID and the frozen vector.

Use the existing injected monotonic clock and one request cursor. Sample before lock and compute
`sampled_at.checked_add(50s)` once; overflow refuses unchanged. Nonblocking lock/re-sample discipline
refuses at `now >= deadline`, including equality and contention across expiry, without extending
the deadline. Atomically check phase, exact target, queue capacity, exclusivity and identity
availability, then capture request ID, baseline, context, Tab, direction, deadline and flight.
Any initial time/queue/target/identity refusal preserves cursor, marker, cache, attachment,
selection, projection and navigator. No second cursor is allowed.

| Transition                                          | Guard and result                                                                                                                               | Structural fence / effects                                                                                                                                                   |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Drain then submit                                   | Earlier FIFO and causal feedback finish truthfully; submit before deadline                                                                     | One 50-second budget covers drain, Move and validating Snapshot; existing earlier deadlines remain earlier                                                                   |
| Definite precommit                                  | Action-compatible Denied/Rejected/Unavailable, or proven local not-submitted expiry; TabUnavailable reads one Snapshot                         | Exact UI settlement shows notice and clears flight; a validated removal uses common membership settlement                                                                    |
| No-op                                               | `Unchanged.revision = baseline`                                                                                                                | No Snapshot; exact UI settlement clears flight without identity/cache change                                                                                                 |
| Known commit                                        | Exact context/Tab and `TabMoved.revision = checked(baseline + 1)`                                                                              | Retain commit; allocate one Snapshot ID from same cursor at execution; exhaustion retires without replay or cursor rewind                                                    |
| Snapshot available                                  | Revision at least committed; exact expected full baseline permutation at committed revision; shared strict higher-revision validation above it | Bounded typed event; preserve target or enter common cleanup; exact atomic installation releases fence                                                                       |
| Snapshot definitely denied/unavailable after commit | Move is known committed, canonical projection unavailable                                                                                      | Enter MoveProjectionUnavailable with old historical cache, prior stable phase, viewer/controller/generation and Ready proof retained                                         |
| Explicit recovery                                   | `Ctrl+B w` admits one Refresh using its own existing 50-second transport/deposit split                                                         | Require at least known commit revision and exact permutation there or allowed later history; remap and restore prior phase; repeated definite unavailable preserves recovery |
| Corrupt/ambiguous exchange                          | Wrong correlation/kind/outcome, impossible revision, corrupt projection or possible handoff then ambiguity                                     | Retire mixed carrier without new-ID replay; known commit remains known                                                                                                       |
| Exit/retirement                                     | Exact existing carrier lifecycle                                                                                                               | Releases waiting executor; retirement is not successful completion                                                                                                           |

Typed deposit itself must precede `now >= deadline`, using outside-lock sampling, nonblocking lock
and exact event-key/deadline revalidation. If a safe complete typed event cannot be deposited,
retire the outstanding transport flight. After deposit the transport budget is finished: UI
semantic settlement has no synthetic wall-clock limit, and the fence remains until exact
consumption or carrier/Exit retirement. Late consumption must not expire an already deposited
result. UI-loop progress remains an environmental assumption, not a new fairness guarantee.
Move transport has no Detach reserve, renewed Snapshot budget or Move retry. A surviving target
needs no attachment cleanup/setup. A later projection that removes the target instead enters the
common membership continuation described below.

MoveProjectionUnavailable blocks selection/Create/Move until recovery succeeds. Input/Poll and
controlling Resize may continue because target identity is unchanged. Recovery submits only Snapshot,
never Open, Create, Start or Move. A surviving target preserves its attachment; validated removal
uses common exact cleanup and optional successor setup. Identity exhaustion or corrupt/ambiguous
recovery retires.
Fresh startup still joins the retained Open reference/outcome and selects current snapshot
`tabs[0]`: after moving B before bootstrap A, new Clients select B and existing Clients keep their
active ID. Failed/unavailable/unknown anchor outcome fails before Snapshot; a Started anchor with
nonattachable moved head fails at that head's AttachBinding. No fallback or scan is allowed.
Other live Clients learn committed changes through the ready-loop `workbench.publication.read`
probe, which routes one refresh hint through the same `Ctrl+B w` admission; there is no push or
terminal switch. A `published` probe sets one coalescing ingress latch inside the still-open
mailbox before any stale-result or causal-wait handling, so a dropped or stale Poll reply cannot
erase the demand. At the next stable point the UI captures that latch into canonical session
state as bounded publication debt (`Clean`/`Pending`/`InFlight { late }`/`Parked`) exactly once.
`Busy` or `Full` refresh admission leaves the debt pending for a later fair tick rather than
dropping it; a definite admission failure parks the debt with no hot-loop retry until a new
accepted hint or an explicit manual refresh re-arms it. Admission checks run in mailbox order —
`Full` first, then generation and serial exhaustion, then the phase-dependent `Busy` outcome — so
an exhausted identity space retires the session even while a draft or switch would otherwise
refuse the attempt. A hint arriving while a flight is live is captured against the debt itself,
not against the live slot: only a flight that adopted the demand at admission can carry it as
late demand, so a hint during an unassociated clean manual refresh simply pends and drains into
one follow-up refresh after that flight settles, while a hint during an adopted flight marks it
late and settles into exactly one follow-up demand. Every refresh's transport read and result
placement stay deadline-bounded; once the typed result is placed, an ordinary publication/manual
refresh waits for the UI reply without a deadline, while context-selection, setup-recovery and
bounded membership-recovery flights keep their marker deadline. Ordinary Poll admission is
keyed on the actual queued/leased command rather than UI observation bookkeeping, so consuming a
Poll result or clearing its observed serial never permits a second ordinary Poll early nor
permanently blocks the next probe; setup and bootstrap Poll keys stay outside this occupancy.

Move uses common cleanup and installation. Controlled UI/carrier tests cover transport/FIFO ordering, known-commit preservation, later active-target removal, exact cleanup, fallback and Empty. Combined native integration remains separate evidence. The exact committed revision requires the full expected permutation; only a later revision may remove the moved target. Preserve immutable survivor identities and creator provenance, then choose the historical-ordinal neighbor or Empty. Raw absence never proves Gone.

Retain the exact known Move commit before any fallible membership continuation. An attached removal
keeps the frozen old cache/key until the common owner settles exact cleanup; its one post-transport
budget is 45 seconds forward plus 5 seconds cleanup reserve. No-viewer replacement uses the same
exclusive state/mailbox installation plans directly. The plans atomically install cache, provenance,
optional desired key and prevalidated Move finish facts; failed or dropped plans change neither
membership nor cursors, while known commits remain retained. Fresh replacement uses the shared
generation cursor; Empty preserves it. The exact Move flight stays fenced through cleanup settlement.
Old-key Input and Resize queued after Move are discarded before controller lookup or sequence allocation once
replacement/Empty is installed. Validate the current lease first, then discard only when both the
actual attachment and desired key differ from the queued key; earlier pre-Switch Input and Resize still drain
through its owned old controller after desired advances. A surviving Move target likewise continues
to send on the same controller.

Definite `TabUnavailable` requests one Snapshot within the original Move transport budget, without
inventing a commit. Its trusted rejection revision is a lower bound for that sequential Snapshot,
in addition to ordinary canonical validation; it supplies no permutation expectation. Available
membership uses the common continuation. Typed Unavailable retains the ordinary refusal notice and
old stable core, clears the exact flight, and permits later ordinary explicit Refresh; it creates
neither committed recovery nor automatic retry. Known-commit Unavailable instead keeps the existing
MoveProjectionUnavailable structural fence and ordinary Ready effects. Explicit recovery carries
the original Move expectation into the common Refresh cleanup/install and never replays Move.
Concrete evidence must distinguish Snapshot revision below/equal/above the retained refusal revision, no-viewer
installation, attached cleanup before Empty/real successor Ready, queued stale Input, exact reply
correlation, 45/50-second boundaries, late typed UI consumption, and retained A versus newer B.
The refusal floor refines reachable sequential model reads: `ReceiveMoveTabUnavailable` follows
removal and `ReadAvailableSnapshot` observes the nondecreasing Server revision. It is not a separate
checked refusal-floor invariant; concrete wire boundary tests own that defensive check.
At exactly the refusal revision, a Snapshot containing the refused target contradicts the observed
absence and retires as authority divergence without retaining a commit or installing membership.
Higher revisions still use ordinary canonical validation; no historical removed-ID registry is added.
The actual Server checks expected revision before target lookup, so ordinary peer removal after a
valid native baseline normally yields RevisionMismatch. TabUnavailable exercises defensive handling
of an action-compatible reply, not the main peer-removal race. Same-revision absence consistency is
a concrete reply refinement, not a newly checked model property.

`state/move_tab.rs` retains the exact commit before fallible continuation and validates refusal
revision facts. `ui/move_membership.rs` consumes the actual Move result, while the existing common
membership owner settles Detached and commits both installation plans under the reply mutex.
`ui/tests/move_membership.rs` covers delayed typed-result consumption, exact old cleanup, 45/50-second
boundaries, wrong generation, direct no-viewer installation, and ordinary refusal recovery.
`connection/native/tests/tab_move.rs` drives real carrier Move/Snapshot/Detach through Empty or a
successor's Attach/Resize/Poll/frame-feedback/Ready; a following Snapshot is the FIFO witness that
old Input and Resize emitted no request. It also checks still-owned pre-Switch Input/Resize and Snapshot-only committed
recovery. Run `cargo test --locked -p orchbee --bin orchbee terminal_session::` for these state and
consumer checks. Full native binary tests also require the existing Server, Worker and terminal-host
process binaries. These checks do not construct a Gone proof or establish complete Close integration.
The production entry is `TerminalIntent::Standalone`. Legacy `Create`/`Attach`, their executor and
renderer, and the connected legacy TUI entry are compiled only for tests. Their existing process
witness retains its rendering, input, resize and terminal-mode restoration assertions; it does not
establish the production startup path. Close inspection helpers and reexports used only by tests
have the same explicit boundary. Create reducer fixtures prepare the candidate before cleanup and
use the common installation plans. Retirement cases cover retained commit, validated cleanup pending,
definite projection unavailability before cleanup, and installed selection. There is no separate
detach-before-Snapshot state or success consumer. Reducer fixtures do not claim carrier or mailbox
receipt proof; those obligations remain with the native executor and NativeUi membership tests.

Deterministic tests own strict 0..256 identity/shape and exact
same/create/Move revision checks, reused created ID before overlay, immutable tuple/binding/
Presentation-label refusal, all three stable phases, generated create/rename/move histories with
shrinking/replay, historical Ready, stale UI, admission races and recovery. Fake-clock/carrier tests
own checked-add overflow, deadline minus one/equality/plus one, lock contention, prior deadline,
shared budget, initial and postcommit ID exhaustion, timed deposit and UI consumption after expiry.
Controlled PTY tests own active PID/marker continuity, usable input/Resize and second-Client
refresh; worker/UI traces own exact viewer/controller/generation preservation. Internal
retirement-to-completion is Session/stateful evidence only. These sampled traces do not prove arbitrary permutations, physical platforms or guaranteed recovery success.

## Projection refinement evidence

The strict projector now accepts higher-revision create/rename/reorder histories by immutable Tab
identity. `workbench_projection/tests/reorder.rs` checks every cache length from 0 through 256,
selected reverse/rotation histories, structural-ID uniqueness, immutable-member refusal and
created-member lookup after later moves. `label_evolution.rs` compares generated histories with an
independent sorted-membership oracle. Exact commit-prefix and same-revision equality checks remain
required. These checks establish the pure projection boundary; Move command runtime integration
and cohort process acceptance have separate evidence below.

State replacement now prepares provenance by Tab ID and resolves the phase-required selected and
committed identities before installing the cache. `state/tests/reorder.rs` checks all six historical
provenance classes, all three stable phases, nonfirst selection, historical Ready after frame
invalidation, moved correlated CreateSnapshot and explicit recovery, and reused-created-ID refusal
before overlay. The Started recovery case checks the moved binding/index and fresh setup budget.
Ordinary replacement remains prohibited during Create/Switch setup and cleanup.

`state/tests/reorder_history.rs` compares four fixed seeds of 101 create/rename/move steps in each
stable phase against an independent canonical snapshot and identity-keyed provenance history.
The corpus starts with six members and adds at most one per step; it covers both move directions,
no-op moves and active-index changes. It logs seed/phase/coverage and shrinks failing action chunks
and values; `ORCHBEE_NATIVE_REORDER_REPLAY` accepts the emitted comma-separated `c`, `r:index:value`,
and `m:from:to` trace. Snapshot-only creates exercise BindingOnly; correlated creator overlays have
separate deterministic evidence. Run these checks with
`cargo test --locked -p orchbee --bin orchbee terminal_session::` on Unix. This evidence does not
establish Move mailbox admission, transport deadlines, exact Move-commit permutation, or live PTY
continuity; the separate Move integration and process checks below own those boundaries.

## Scope and owners

This record fixes the native Client consumer of the canonical OBWP `tab.create`, `tab.move`, and
Panel-tree projection contracts, plus the `Ctrl+B c`, `<`, `>`, `n`, `p`, `1` through `9`, `h`, `j`, `k`,
`l`, and `w` actions. It preserves the existing one sequential admitted Client carrier, one OBCS viewer, one combined input-and-resize controller at
most, bounded command/event storage, and TUI restoration boundary.

The Server-owned Workbench snapshot remains the only source of Tab order, labels, Panel and
Presentation membership, and terminal bindings. One native invocation owns only its cached strict
projection, local active Tab ID, attachment generation, navigator state, geometry observation,
terminal projection, and transient statuses. Local selection never writes or advances a Workbench
revision.

Pin/group mutation and child Presentation creation, persistence, push/delta subscriptions, remote transport, reconnect, hidden carriers,
carrier multiplexing, and multi-viewer composition inside one Client remain excluded.

## Pure strict projection

Replace `select_standalone_binding` with this Unix-only pure projection in
`terminal_session/workbench_projection.rs`:

```rust
struct NativeContextTabs {
    site_id: SiteId,
    revision: WorkbenchRevision,
    tabs: Vec<NativeSimpleTab>,
    location: NativeContextLocation,
    navigation: Arc<[NativeContext]>,
}

struct NativeSimpleTab {
    tab_id: WorkbenchTabId,
    tab_label: WorkbenchLabel,
    automatic_label: bool,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    presentation_label: WorkbenchLabel,
    binding: NativeTerminalBinding,
    panels: Box<[NativePanel]>,
    tree: NativePanelTree,
}

struct NativePanel {
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    presentation_label: WorkbenchLabel,
    binding: NativeTerminalBinding,
}

enum NativePanelTree {
    Panel { panel_index: usize },
    Split {
        first: Box<NativePanelTree>,
        second: Box<NativePanelTree>,
        first_slot_id: WorkbenchPanelSlotId,
        second_slot_id: WorkbenchPanelSlotId,
        orientation: PanelOrientation,
        split_id: WorkbenchSplitId,
    },
}

struct NativeTerminalBinding {
    worker_id: WorkerId,
    terminal_id: TerminalId,
}

struct CreatedTabExpectation {
    committed_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    requested_label: WorkbenchLabel,
    automatic_label: bool,
    placement: Option<TabCreationPlacement>,
}

fn project_native_standalone_tabs(
    snapshot: &Snapshot,
) -> Result<NativeContextTabs, NativeProjectionError>;
```

The startup projector requires exactly one Terminal Context whose ID equals
`navigation.standaloneTerminalContextId`. The current-context projector instead validates the
selected canonical location. Other Project/Workspace navigation metadata is retained; nonactive
contexts' Tab layouts are not validated here. The active context may contain exactly 0 through 256
ordered Tabs, canonical groups and a pinned prefix. Group identities, labels, references and
contiguous membership are validated locally. Every Tab has either one Panel root or a recursively
binary Panel split tree, with one bound primary Presentation and up to sixteen bound child Presentations at every leaf.
One unsupported member rejects the whole projection; the projector never filters, reorders, or chooses
the first attachable member. It retains both labels exactly, including duplicates and whitespace, and
preserves every Panel, split, slot, orientation, Presentation, and terminal identity in the tree.
Panel, Presentation, and terminal IDs are unique across all leaves; split and slot IDs are unique in
the complete active-context projection.

Two additional pure validators own cache replacement:

```rust
fn validate_canonical_refresh(
    current: &NativeContextTabs,
    candidate: NativeContextTabs,
) -> Result<NativeContextTabs, NativeProjectionDivergence>;

fn validate_created_tab(
    current: &NativeContextTabs,
    expected: &CreatedTabExpectation,
    candidate: NativeContextTabs,
) -> Result<(NativeContextTabs, usize), NativeProjectionDivergence>;
```

The accepted Move refinement replaces the legacy `validate_append_only_refresh` prefix policy:
require the same Session/context and reject lower revision; same revision requires exact equality of
the complete projection, including order and labels. At greater revision the shared canonical
membership rule applies: overlap of any current and candidate Panel, Presentation or Terminal
identity requires the same complete immutable member tuple with the owning Tab as the authorized
relocation delta, a surviving Tab identity retains at least one of its original members inside
that same Tab, and `Tab.label`, member ownership across Tabs, and order may differ. Retained
structural identities stay exact: a surviving Split keeps its owning Tab, both ordered Slot
identities and orientation, and a surviving Slot keeps its parent Split; removed or entirely
fresh Split/Slot identities need no comparison, and every candidate Panel, Presentation,
Terminal, Split and Slot identity must be unique. Because `publication.read` folds every
committed publication since the retained revision
into one final snapshot, and a journal gap answers with one full Snapshot, a single refresh may
carry the sum of several committed splits, closes and moves; the same baseline-to-final comparison
admits the folded result. Any survivor mutation, identity reassignment, retained-structure
reshaping or member loss disguised as retention is divergence. At most 256 Tabs need only exact
scans, no stored map.

`CreatedTabExpectation` is projection-owned and retains the canonical commit facts, requested label
and naming mode, plus optional typed placement. The session state constructs it; request identity,
terminal provenance, attachment generation and mailbox types never enter the projector.
`validate_created_tab` retains the original full pre-create canonical snapshot through recovery.
Returned Tab/Panel/Presentation IDs must be absent from every baseline context. Its commit revision
must equal the baseline's checked successor, and the candidate revision must be at least that commit.

At the exact revision, reconstruct the complete expected Snapshot using shared `create_terminal_tab`:
append, insert at group end or after an exact target, atomically create an unnamed grey group, or duplicate exact source metadata
at index + 1. New group identity must also be absent from every baseline context. Compare the entire
projected candidate, including unrelated contexts, navigation, trees and metadata. A fresh root must
be one bound Panel with no children. The source raw label anchors duplicate Presentation creation;
other placements use the submitted label. Later revisions use the existing strict membership rule,
allowing supported later rename/move/removal without assuming the created ordinal. The result has
no binding; only the validated correlated snapshot supplies it. Prepare provenance by ID, overlay
only after every check, then atomically install. Known creation provenance remains historical after
Worker exit. Only this snapshot-derived binding may reach `AttachBinding`.

Background duplicate uses the same Create barrier and exact cleanup before installing canonical
membership. If it survives, the old selected Tab/Panel remains selected at a fresh generation and
attaches according to its own provenance; the created terminal's failed/unknown result stays attached
to its own new ID and a localized notice. Foreground placements activate the new Tab and expand its
group. Extra selection marks clear only after installing a candidate that still contains that new
Tab, whether grouped or ungrouped; Duplicate and candidates that already removed it preserve marks. Missing selected members follow established fallback. No duplicate clears Client-local marking.
[Native groups](native-tab-groups.md#terminal-creation-and-duplication) owns the menu and placement
contract; the response table below describes foreground creation unless explicitly stated otherwise.

Selected duplication uses each unchanged single-Create lifecycle in sequence. Its private pre-install
witness validates the old concrete pane, selected Presentation binding, marks and full Session Snapshot
before this membership owner changes cache, desired selection or old pane records. Only that correlated, one-shot install may adopt a
fresh generation; original selection must reach Ready and the prior lease/reply/poll settlement must
finish before atomic mailbox admission of the next source. A fixed 120-second total bounds per-member
50-second deadlines and their five-second cleanup reserve. Cancellation, drift, partial failure,
unknown results or recovery stop the tail while retained member facts outlive temporary status.
The owning group contract and deterministic sequence tests check this inter-member boundary; single-Create and recovery rules remain unchanged.

`MovedTabExpectation` retains committed revision, context, Tab ID and the submitted wire direction.
`validate_moved_tab` validates the admitted baseline's checked successor and refuses an impossible
committed no-op. At that revision it requires the exact full-vector permutation with both labels
unchanged; above it the shared strict identity validator admits later supported history. The
`workbench_projection/tests/moved_tab.rs` corpus checks first/middle/last positions, all four wire
directions, singleton and 255/256 boundaries, wrong commits and immutable-member divergence.
The Move result and explicit recovery state transitions consume this validator; their mailbox,
transport and real UI evidence is recorded below.

The pure projector accepts an empty standalone context for full 0..256 conformance. Cold startup
and every active-session refresh separately require the state-specific active-member invariant.

## Panel projection and local focus

The accepted `NativePanelTree` is a strict, depth-first representation of the canonical PanelNode.
Leaf order is the order used by attachment state; the tree retains split orientation and both slot
identities so a later snapshot cannot silently replace a leaf or reshape a Tab. Every visible leaf
owns its own viewer attachment inside the shared per-Client HostViewers budget (8): the Session
keeps one pane record per leaf under a stable `PaneKey` — the original selection anchor plus the
leaf's `WorkbenchPanelId` — and each record's physical attachment is a `PaneRef` incarnation minted
when its attach target was issued. Polls continue for every admitted pane incarnation, so inactive
leaves keep rendering their own live frames. Input admits only the focused pane; resize admits
each visible controlling pane's exact incarnation. Read-only panes never acquire control for resize.

In a connected Native Session, `Ctrl+B h`, `j`, `k`, and `l` are Client-local focus actions. When a
direction has a neighboring leaf, the Session moves the focused-pane selection to that leaf's
existing record without detaching, re-attaching, or re-tagging any already accepted work — no
switch cleanup, snapshot, or attach chain runs for a same-Tab move. A focus action never writes a
Workbench revision and never submits
`panel.split`, `panel.close`, or `panel.move`. A single
Panel or an edge with no neighbor is a local no-op. The renderer divides the canonical tree and
draws each leaf's own live frame; the focused leaf owns control input. When a leaf vanishes its
pane record departs and converges to canonical disposal, and a re-observed leaf mints a fresh
incarnation — late input, frames, feedback, and settlements tagged with the retired incarnation
are fenced at the exact `PaneRef` and never land on the re-bound attachment. A peer-created single-leaf split is accepted by the ordinary
refresh path; the session preserves the selected Panel by identity while reindexing the cached tree
and retained provenance (`peer_panel_split_refresh_preserves_active_panel_identity`).

The provisional `Ctrl+B -`, `_`, `\`, `|`, `X`, and `H`/`J`/`K`/`L` bindings submit the three
structural panel commands through the same mailbox mutation admission, typed settlement,
pending-commit retention, and publication-refresh recovery used by Tab operations; the bindings
are provisional pending the input-design review and may be rebound. `-`, `_`, `\`, and `|` submit
`panel.split` on the focused leaf in the Below / Above / Left / Right direction — the glyph set
follows the input-vocabulary draft table — carry the leaf's existing presentation label, and use
the Client's real controlling cell/pixel geometry rather than a fabricated size. `X` arms the
leaf-scoped panel-close confirmation on the focused leaf under
[native panel close confirmation](native-panel-close-confirmation.md): only a drawn, still-valid
confirmation admits the ordinary `panel.close` on the frozen leaf — `y` confirms after the
complete draw and `Esc` cancels — and the confirmed close commits through the ordinary
publication path. Closing the Tab's last leaf removes the whole Tab — its terminals are disposed
and the Tab ends for every attached Client, the same blast radius the `x` Tab close confirms
against — and the drawn confirmation names exactly that impact before any `y` can take effect. `H`, `J`, `K`, and `L` submit `panel.move` on
the focused leaf with the directional neighbor as `target_panel_id` — interim top-level stand-ins
for the draft's `PANEL`-mode `h`/`j`/`k`/`l` move, since mode structure is out of this slice's
scope; a single Panel or an edge with no directional neighbor is a local no-op before admission. All three operations admit only from a
settled Ready, SelectedWithoutAttachment, or RetryableAttachUnavailable phase, keep their exact
request correlation, and classify definite rejected/denied/unavailable outcomes — including the
terminal-busy refusal an overlapping aggregate Close produces for same-Tab split/move — as
not-applied results that preserve the carrier. A committed split or move retains its pending commit
until a snapshot validates the exact expected candidate; a validated candidate that removed the
previously selected leaf while its Tab survives enters the common membership cleanup/install path
and attaches a surviving leaf of the same Tab. Unavailable snapshots retain the commit for a later
refresh, divergent or ambiguous results retire the carrier without replay, and an executor exit
after commit hands the retained commit to ordinary recovery.

These structural mutations are authorized by the Server's `WorkbenchGrant` capabilities —
`mutate_navigation` and `create_terminal` for split, `mutate_navigation` for move,
`close_terminal` for close
([Workbench authority](../../orchbee-server/docs/workbench-authority.md)) — a layer independent of
terminal-side authority: a viewer attachment grants neither input nor resize — its
`read`/`input`/`resize` capability flags scope that attachment itself — and input and resize flow
only through the terminal's single combined input-and-resize controller grant, bound to one
Session, viewer, Worker generation, and Host terminal
([`schemas/client-server/README.md`](../../../schemas/client-server/README.md)). Holding terminal
input or resize control never authorizes a Workbench mutation, and a read-only attachment cannot
input or resize regardless of any Workbench grant.

## Startup

Cold startup remains exactly:

1. measure complete real terminal geometry;
2. submit one `terminal.open` ensure/join and wait for its final `Opened` result;
3. submit one `SnapshotRead` on the same carrier;
4. validate the full `NativeContextTabs` projection and require `tabs[0]`;
5. set `active_tab_id = tabs[0].tab_id`, allocate the initial attachment generation, and send the
   exact snapshot-derived `AttachBinding`;
6. Acquire, conditionally Resize, receive a fresh full projection, draw it, send matching
   `FrameApplied`, and require `FeedbackAccepted` before publishing Ready.

No Open-returned create reference or terminal identity participates in selection. Startup has no
snapshot retry or fallback and retains the existing startup-wide deadline.

The native mailbox exists before Open with no desired attachment. Its one request cursor supplies
both Open and SnapshotRead identities. The worker prepares the strict nonempty projection, then
places one Bootstrap event in the existing `EventCell`. At its exact UI settlement, the UI checks
the original startup deadline and atomically allocates the first attachment generation, installs
the cache and Starting selection, publishes the desired mirror, and deposits
`BootstrapApplied(BootstrapTarget)`. The target contains only the validated first Tab and its
snapshot-derived binding; it carries no new Switch/Create deadline. Only that reply licenses
AttachBinding. No command dequeues before initialization, and no temporary Tab identity, extra
channel, or independent request cursor is introduced.

Invalid or empty projection, expired placement or UI settlement, and generation exhaustion leave
the cache and desired attachment uninstalled. Already issued request identities remain burned.
Duplicate or stale unowned bootstrap replies cannot allocate another generation or attachment.
Deterministic mailbox tests cover these boundaries and close while awaiting UI; scripted-carrier
tests cover Open, SnapshotRead, and Attach failures and exact identity order. The three Tab models
start after bootstrap and do not establish these cold-start implementation claims.

## Presentation scheduling and retained pixels

The Windows input-to-echo path is: the inherited console's single record reader → byte decoder /
input dispatcher → bounded command mailbox and executor lease → local transport / Host PTY →
validated terminal projection → UI draw → causal feedback. A worker publication used to wait
for the shell's input-only 50 ms wait even when the console was empty. Switch setup repeats this
handoff for snapshot, detach, attach, configuration, frame, feedback and readiness, so successive
UI handoffs could each incur the wait. This is a scheduling cause, not evidence of slow PTY echo.

`terminal_session/wakeup.rs` binds one coalesced notification and an unpark token to the active UI
thread. Mailbox event publication, lease release and closure signal it. The Windows console
reader checks it while empty and parks for at most 1 ms between console probes; a publication
interrupts that park. Existing pending records, UTF-16 state, repeat tails and bracketed-paste
bytes remain owned by the same reader, with the existing 4096-byte/record and 4 ms burst bounds.
Replacing or leaving a surface unregisters its waiter. Notification carries no input, output,
request identity or acknowledgement: the ordered mailbox and its single causal cell remain
authoritative. Unix consumes already-pending notifications before its existing input wait;
mid-wait interruption here is Windows-specific. Transport framing and its backpressure are unchanged.

Empty ordinary poll replies set a monotonic 8 ms earliest-next-poll deadline. The shell waits for
the shorter of that deadline and its 50 ms outer ceiling; repeated notifications cannot admit
another empty poll early. An admitted input clears this idle delay. Nonempty output and setup
handoffs have no added idle delay. The scheduler still permits only one ordinary poll in flight
and rotates among projected panes. The Windows transport's existing 2 ms readiness probes are
separate from this deadline.

The fixed-viewport backend starts every draw call from an explicit cursor-hidden, reset-style
baseline, then emits adjacent same-style cells as one cursor run and queues only color and
modifier changes. Its run position advances by the sanitized symbol's display width, so wide
glyphs and real gaps still receive exact cursor placement. Shared terminal attributes such as
Bold/Dim and Slow/Rapid Blink are reset and then re-applied from the complete target style.
Ratatui restores the requested final cursor visibility and position before the synchronized-update
pair closes. A failed write leaves the existing synchronized-update recovery contract unchanged.

`ui/retained_frames.rs` owns at most **8 immutable text images**, with at most **16 MiB of accounted
frame allocations** across the cache. Accounting includes frame/row/cell storage capacities,
text bytes and Arc counters; shared text is conservatively charged per reference. Fixed identity
metadata and allocator overhead are outside this accounting, so it is not a total-process RSS
limit. Insertion or replacement evicts least-recently-used images until both limits fit; a valid
draw of an unchanged Arc refreshes its recency without duplicating the allocation. An individually
oversized image is omitted whole, never truncated. The live projection and cache share immutable
frame storage; applying a new frame cannot mutate old pixels, and the last owner releases each
allocation.

Only a validated live projection in Ready, outside retained-history mode, may enter this cache
after a successful shell draw. Cache membership is not proof that overlays left the pane uncovered
or that its latest frame was acknowledged; it cannot supply either receipt. Membership is checked,
without reprojecting or copying the Snapshot, against the active cache's freshly accepted complete
canonical Snapshot across every still-navigable context. A targeted fresh Client lane may shallowly
inherit this render-only bank from the exact same endpoint's active or cached surface. The
candidate's own image wins an exact identity collision. Removing a context or changing its exact
placement or binding prunes its pixels; no retained-context membership, attachment or controller
authority crosses the handoff.

A revisit during Starting/Switching, or a Ready viewer converging on a replacement full frame, may
paint the retained image only while projection status is Empty or WaitingForFullFrame and Session,
terminal context, Tab, Panel, selected presentation, worker/terminal binding and the rendered
column/row dimensions all match. A known new HostEpoch must also match. A first visit, an evicted
image, an unbound or faulted terminal, an explicit retryable attachment failure, a different
presentation or binding, and a changed cell grid use the normal connecting/failure path. Resizing
does not stretch cached pixels; returning to a matching grid can reuse a still-retained image.
NeedSnapshot keeps exact pixels visible while the replacement full frame is pending; a fresh
validated frame takes over immediately through the existing draw/feedback/readiness sequence.
Cache pixels never enable cursor, selection, links, graphics placement, graphics draw receipts,
controller input or frame ACK; they do not retain the old viewer attachment. The renderer leaves
the phase visible in chrome. Blocked invalidates the matching cache entry immediately, canonical
membership changes prune it, faults suppress it, and Session retirement releases all images.
Ordinary projection cleanup during switching preserves only this bounded pixel cache.

The modeled risk is interaction between worker notification, UI scheduling, attachment retirement
and shared frame lifetime. The refinement obligations are:

| Invariant                                                                                              | Executable evidence                                                                                                                                                                                                                                   |
| ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A publication between inspection and parking remains observable; notifications never replace payloads. | Owned wake implementation tests; wake-token race and real worker-completion tests.                                                                                                                                                                    |
| Empty replies cannot spin through the mailbox; input remains ordered.                                  | Controlled console clock: publication at 5 ms returns at 5 ms, versus 50 ms without interruption; 20,020 UI ticks over 1000 ms admit 126 empty polls including the initial poll; repeat/paste tests.                                                  |
| Retained pixels match canonical identity and confer no readiness or feedback.                          | Owned retained-pixel implementation tests; fresh Beta → Alpha → Alpha-2 bank handoff, active/cached exact-endpoint and replacement gates, candidate-newer precedence, wrong-binding/HostEpoch/geometry rejection, and real renderer isolation checks. |
| Shared pixels are immutable, bounded and released by their final consumer.                             | Projection Arc lifetime, membership pruning, cache count and aggregate byte-eviction tests.                                                                                                                                                           |

## Client state and bounded mailbox

`TerminalSession` owns the typed state shared by the native Windows and Unix shells. Platform
terminal-device and transport adapters own console/TTY waiting and I/O. `client_view.rs` contains
protocol-free key/overlay/notice decisions; Session and mailbox code retain protocol and attachment
authority. The scheduling change above does not introduce a second input reader or transport owner.

The session state is an explicit sum. The names are implementation guidance, not a second wire
contract:

```rust
enum SessionPhase {
    Starting,
    Ready(ReadyAttachment),
    Creating(CreatingBarrier),
    CreateCommittedNeedDetach(CommittedTabCreate),
    ProjectionUnavailableBeforeCleanup(CommittedTabCreate),
    Switching(SwitchSetup),
    SelectedWithoutAttachment(SelectedTerminalState),
    RetryableAttachUnavailable(SelectedTab),
    Retired(RetiredState),
}

enum ControllerMode { Controlling(ControllerId), ReadOnly }

enum CreateTerminalProvenance {
    Started,
    Pending,
    Unavailable(WorkbenchTerminalUnavailableReason),
    Failed(WorkbenchTerminalFailureReason),
    OutcomeUnknown,
    BindingOnly,
}
```

`BindingOnly` means the Client learned a canonical binding without creator-known terminal outcome.
Started and BindingOnly Tabs may be attached. Pending/unavailable/failed/outcome-unknown results
are retained by exact Tab ID as stable non-attachable provenance for this Client Session. Refresh
does not erase that knowledge. Cold-start projection and every refresh merge provenance by exact Tab
ID: retain every existing entry unchanged; assign `BindingOnly` to each snapshot-only ID not already
known; and, after created-member validation, assign the just-returned ID its correlated Create
result provenance instead of the snapshot default. A switch-time definite Attach failure for an otherwise attachable
Tab is instead retryable through a later explicit same-Tab selection.

Ordinary `active_tab_id` exists only in cache-backed states and must name one member of the last
validated projection. `desired_tab_id` may name a Switch target already copied from that cache.
An unprojected Create result remains inside the immutable committed operation; it does not populate
`desired_tab_id` or a public `committed_target`. Successful created-member validation instead freezes
a private candidate containing the canonical order, provenance, selected identity and generation.
Only after exact old cleanup does one atomic transition install that candidate as cache-backed
`active_tab_id` and `desired_tab_id`; the stale cache is never treated as containing the returned Tab.

Replace the raw `SyncSender<Command>` with one `Arc<CommandMailbox>` containing a mutex,
condition variable, an eight-record FIFO, a closed flag, a checked non-reusing attachment-generation
cursor, the published desired generation/Tab, the existing OS-random-seeded `Identities(u128)`
cursor, and one reserved causal-feedback slot. The single
reserved slot is bounded and is not another carrier: it lets feedback caused by a pre-barrier Poll
settle before the FIFO head barrier. One bounded event slot remains. The only I/O owner consumes
all records and performs every OBCS/OBWP exchange sequentially on the admitted carrier.

```rust
enum QueuedCommand {
    Input(Tagged<ControlInput>),
    Resize(Tagged<Geometry>),
    Poll(Tagged<()>),
    Switch(SwitchMarker),
    Create(CreateMarker),
    Refresh(RefreshMarker),
}

struct SwitchMarker {
    generation: u64,
    tab_id: WorkbenchTabId,
    binding: NativeTerminalBinding,
    provenance: CreateTerminalProvenance,
    forward_cutoff: Instant,
    setup_deadline: Instant,
}

struct CreateMarker {
    reserved_generation: u64,
    request_id: WorkbenchRequestId,
    site_id: SiteId,
    expected_revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    label: WorkbenchLabel,
    automatic_label: bool,
    geometry: WorkbenchTerminalGeometry,
}

struct RefreshMarker { refresh_generation: u64 }
```

Input admission remains byte-bounded inside the same eight-record FIFO. After the ordinary
closed, bootstrap, close-flight and focused-pane gates pass, a new Input may append under the
mailbox mutex only to the queued tail Input for the exact same `PaneRef` (including incarnation),
with no retry deferral and a combined payload of at most 4096 bytes. It never mutates the active
worker lease. The append preserves the tail record serial, so `try_enqueue` returns a queue-record
identity that can repeat across Input calls; it is not a per-call request receipt. The worker later
rechecks the exact attachment and controller, then submits one Input request whose acknowledgement
covers the exact combined byte count. A different pane incarnation or any Poll, Resize, Viewport,
recovery, selection or structural record is an ordering boundary. Payload overflow starts a new
record when a slot remains; when all eight records are occupied and the tail cannot accept the
bytes, admission returns `Full` without claiming delivery.

There is exactly one request-identity allocator for the Session. Move the existing cursor from
connection-worker ownership into this mailbox; never clone its seed or counter. The shell and worker
hold the same mutex only long enough to check and commit one next ID and never across I/O. Create
admission checks phase and capacity, computes the next generation and request ID without mutation,
then appends its complete marker and commits both cursors as one critical section. Queue/generation
refusal occurs before the ID commit and burns no ID. Once appended, both values are burned even after
a precommit refusal. Identity overflow appends nothing and burns no generation; because no future
OBCS or OBWP request can be correlated, it closes the mailbox and retires the carrier as a definite
local pre-handoff failure instead of reporting a retryable busy condition.

Switch admission holds the mutex once while it checks capacity and `checked_add`, copies the exact
target ID/binding plus its retained per-target provenance, appends the marker, advances both the
allocation cursor and desired generation, and publishes the target as active. Create admission similarly reserves and burns one future
attachment generation but does not publish it until a committed Snapshot candidate is validated,
the exact old target is cleaned up, and the candidate is atomically installed.
Burned generations are never reused after precommit refusal. Queue or generation refusal appends
nothing and changes no cache, active/desired Tab, generation, attachment, or projection; the shell
may replace its transient notice with localized local-busy copy.

The shell always records the latest measured valid geometry, even for a Ready ReadOnly viewer and
when terminal Resize admission is paused during Creating or Switching. A per-pane report also
publishes the focused leaf's measured geometry to both session state and the setup mailbox before
ordinary admission guards. Background leaves never replace that focus measurement, and an omitted
focus leaves the last valid measurement unchanged. Reattaching a Tab therefore starts with its
actual content geometry, not a stale startup rectangle that would resize the PTY down and back up.
This observation changes Client-local layout data only; it does not authorize an OBCS Resize. The
session separately tracks last admitted/applied controller geometry. A Create marker captures the observed geometry
at its atomic admission; a later successful Acquire begins with a Resize carrying the then-latest
measured geometry and uses the setup catch-up rule below for subsequent observations. A precommit refusal resumes the old controller and, if measurements changed,
admits one catch-up Resize before polling resumes. Read-only viewers never send Resize.

## Local selection

Selection resolves only against the latest applied strict cache. `n` and `p` use the local active
target's index and wrap modulo the Tab count in both directions. `1` through `9` are one-based exact
indices; an absent index is a local no-op with an out-of-range notice. Zero Tabs have no selectable
target.

| Current phase and resolved target                                                                                  | Exact result                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Any otherwise selectable phase, target differs, and one Refresh is queued/in flight/awaiting UI settlement         | Reject the switch locally: no marker, generation, selection, attachment, projection, or navigator change. Report local busy. This guard precedes the differing-target rows below.                                                                                                                                                                                                                                      |
| Any otherwise selectable phase, same target, and one Refresh is queued/in flight/awaiting UI settlement            | No-op, including RetryableAttachUnavailable: no marker, generation, selection, attachment, projection, or navigator change. This guard precedes every same-target row below.                                                                                                                                                                                                                                           |
| Ready, target differs                                                                                              | Atomically admit a Switch marker. On success update active/desired immediately, clear visible projection/feedback, enter Switching, and dismiss the navigator.                                                                                                                                                                                                                                                         |
| Ready, target is the active Tab                                                                                    | No marker, generation, attachment, projection, or navigator change.                                                                                                                                                                                                                                                                                                                                                    |
| Switching, target differs from the current desired Tab                                                             | Admit another marker if capacity/generation permit; it supersedes prior setup. Resolution for n/p starts at the current desired Tab.                                                                                                                                                                                                                                                                                   |
| Switching, target equals current desired Tab                                                                       | No-op; no duplicate marker or generation.                                                                                                                                                                                                                                                                                                                                                                              |
| RetryableAttachUnavailable, attachable target equals active                                                        | An explicit selection admits a fresh marker and generation; the executor skips Detach because cleanup left no viewer. There is no automatic retry.                                                                                                                                                                                                                                                                     |
| RetryableAttachUnavailable, target differs                                                                         | Atomically admit a marker/generation, skip Detach because no viewer exists, and dispatch by that target's retained provenance exactly as for SelectedWithoutAttachment.                                                                                                                                                                                                                                                |
| Ready, Agent-board reattach admission: target equals the desired Tab and recorded dead-target-pane evidence exists | Mint a fresh marker and generation for the same desired Tab through the ordinary switch pipeline — every viewer in that Tab detaches and re-attaches under the new anchor generation (the accepted cost; the Server's terminals are not restarted) — then the jump lands on the exact target Panel. Without recorded dead-target evidence this admission does not exist; ordinary same-target selection stays a no-op. |
| SelectedWithoutAttachment, target differs                                                                          | Atomically admit the same marker/generation transition. The executor skips Detach because no viewer exists, then dispatches by the **target's** retained provenance: Started/BindingOnly performs normal Attach setup; Pending/unavailable/failed/outcome-unknown settles SelectedWithoutAttachment without Attach.                                                                                                    |
| SelectedWithoutAttachment, target is the selected non-attachable Tab                                               | No-op; no marker or generation.                                                                                                                                                                                                                                                                                                                                                                                        |
| Creating, CreateCommittedNeedDetach, ProjectionUnavailable, Starting, or Retired                                   | Reject effectful selection. Only the recovery actions explicitly named below remain available.                                                                                                                                                                                                                                                                                                                         |
| Queue/generation full                                                                                              | Preserve selection and attachment state; report local busy.                                                                                                                                                                                                                                                                                                                                                            |

For a one-Tab cache, n/p resolve to the same member: Ready is a no-op and
RetryableAttachUnavailable is an explicit retry only when no Refresh is pending. While Refresh is pending,
the same-target no-op guard applies in all otherwise selectable phases. A full-queue or out-of-range action does not
dismiss an already open navigator. Panel keys move local focus through the active Tab's split tree;
they are a no-op for a single Panel or when no directional neighbor exists. The provisional
split/close/move bindings described under Panel projection admit one structural panel mutation
against the focused leaf under the same phase, queue, and fence rules as the other structural
submissions.

The Agent board's jump pipeline owns one narrower admission of its own. When a jump reaches a
Panel whose pane viewer is dead while the Tab's selected attachment is still live, recorded
dead-target-pane evidence lets it mint a fresh marker and generation for the same desired Tab.
The ordinary switch pipeline then detaches and re-attaches every viewer in that Tab — an accepted
whole-Tab cost, because one attachment epoch owns the Tab's viewers — before the jump lands on the
exact Panel. The evidence gate is what separates this from the ordinary same-target no-op: it
never manufactures another identity, and the admission does not exist without recorded
dead-target proof. The admission exists in Ready only — a selected non-attachable Tab cannot
hold dead-viewer evidence for an attachable desired target. The board-side reattach stage
abstracts an admitted-and-successful pipeline; a failed reattach settles through the ordinary
switch failure machinery, and mapping it to a board-level Unavailable/Expired notice remains
production-seam work outside this delta.

## Create is a draining FIFO barrier

`Ctrl+B c` is admissible only from a settled Ready identity: carrier live; active, desired,
attachment, and projection generation all name the same cached Tab; the first full-frame feedback
was accepted during setup for that attachment generation; no Create/Switch is active; and no
Refresh is queued, in flight, or awaiting UI semantic settlement. Ready retains this completed-setup
fact through ordinary projection updates and precommit restoration. A later staged frame,
NeedSnapshot, ProjectionBlocked, or accepted Resize may invalidate the current frame proof without
revoking Create eligibility. An in-flight or queued old-target Poll does not prevent admission
because the barrier owns its draw and causal settlement drain. This does not relax the initial
full-frame, geometry, or feedback requirements for publishing Ready. Ordinary Input continues to
use Ready/Controlling authority; successful Create admission closes further effect admission
atomically with the marker, while earlier admitted effects drain in FIFO order.
Admission does not extend an earlier request's own deadline or guarantee Create submission: a
failed or expired pre-marker request may retire the carrier before the marker is dispatched.
The proposed label is decimal `cached.tabs.len() + 1`; at the 256-Tab bound this is `257` and the
single submitted command lets Server capacity policy return the typed result. Duplicate labels are
valid. A successful admission dismisses the navigator and enters Creating. Exit remains available.
Help/sidebar overlay changes remain local; they do not admit terminal or Workbench effects.

Creating rejects further Create, switch, refresh, user input, and effectful Resize. Resize
observations still update latest measured geometry. It stops scheduling new old-target Polls. The
UI continues draining the bounded event channel and may integrate and draw an already accepted
old-generation Poll result while visibly reporting Creating.

The same drain includes a background pane's earlier accepted Resize. During Creating, its exact
live, attached, nondeparting `PaneRef` must still belong to the retained Ready anchor and the
anchor must remain desired; the returned controlling grant must match that pane's own grant.
Settlement updates only that pane's accepted geometry and invalidates its old frame proof. It
does not change focus, the Create identity or the cache, nor admit new Resize work. The executor's
existing lease, exact Resize response validation and event/reply slot prove the outstanding
operation; the reducer must not compare an earlier FIFO result with a newer observed or admitted
geometry. `CreateCommittedNeedDetach` grants no such background allowance, because the prior
Resize and its UI reply finish before Create submission. Deterministic per-pane state and executor traces check this settlement.

Before the connection worker submits `tab.create`, it drains every FIFO record ahead of the marker:

1. accepted Input/Resize continues through the existing bounded ServerBusy policy to a truthful
   definite or unknown settlement, even if another local state changes;
2. a pre-marker Poll result is delivered to the UI; a valid frame is actually drawn before its
   `FrameApplied`, and NeedSnapshot is emitted only after the corresponding no-frame redraw;
3. the causal settlement occupies the reserved slot ahead of the barrier. If it carries feedback,
   the worker sends it and requires exact `FeedbackAccepted`; ProjectionBlocked instead settles
   without wire feedback only after its no-frame redraw;
4. only with no old projection flight or feedback outstanding may the worker submit the one
   captured OBWP command.

This drain is necessary because a precommit result keeps the same viewer. Dropping its flight and
assuming a later Detach could leave that retained viewer blocked. The UI never acknowledges an
undrawn frame.

The first `tab.create` submission uses its one captured request ID. There is no automatic new-ID
retry, fallback OBCS Create/Open/Start, cancellation claim, or inference from a newly visible Tab.
A later human `c` after a definite precommit refusal is a distinct operation. While a Create remains
inside its barrier or committed-projection recovery, no later `c` is admitted. Once it has settled
and selection is again Ready, a later human `c` is a new operation with a fresh request ID.

Create admission fixes one monotonic total barrier deadline from the pre-lock clock sample plus 50 seconds and one
cleanup cutoff five seconds earlier. Pre-marker drain, the single OBWP exchange, committed-old
SnapshotRead/candidate staging, exact old cleanup, and Started setup all share that absolute deadline;
progress never renews it.
Forward work stops at the cleanup cutoff. A known viewer may use the final reserve for exact
Detach and its correlated cleanup settlement as defined below; final expiry retires the carrier if
the barrier has not reached Ready,
SelectedWithoutAttachment, RetryableAttachUnavailable, ProjectionUnavailable, or a definite
precommit restoration. This is the operational settlement bound; it does not assume Worker or Host
progress.

The result transition is:

| Result                                                                                                                                                                                                    | Transition                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Definite top-level denied/rejected/unavailable or proven not-submitted                                                                                                                                    | Preserve the exact old viewer/controller, active ID, projection, and valid feedback; atomically return to Ready with the event-driven precommit notice defined below, issue one catch-up Resize if controlling and needed, and resume Poll. Do not refresh or retry automatically.                                                                                                                                                                           |
| `tab-create-pending`                                                                                                                                                                                      | Retain the committed tuple/revision/provenance and reserved generation 1. Read and validate a fresh Snapshot before old-viewer cleanup. Freeze the exact canonical candidate, then complete exact old cleanup and atomically install the candidate. Creating remains visible until installation; the created Tab is never attached in this cohort. Preserve the request identity; do not submit a new identity.                                              |
| `tab-created/started`                                                                                                                                                                                     | Retain the committed facts and reserved generation 1; read and validate the fresh Snapshot; freeze its exact candidate; only then complete exact old cleanup and atomically install the cache-backed active selection. If the returned member survives, AttachBinding uses its snapshot-derived binding and generation 1. If a later snapshot removed it, the frozen ordinal selects the surviving candidate at generation 2, or Empty; do not resurrect it. |
| `tab-created/unavailable`, `/failed`, or `/outcome-unknown`                                                                                                                                               | Retain stable non-attachable provenance and generation 1; read and validate the fresh Snapshot; freeze its exact candidate; only then complete exact old cleanup and atomically install the matching selected-without-attachment state. A removed returned member remains historical and uses generation 2 for a surviving fallback. Send no Attach.                                                                                                         |
| Any possible OBWP handoff followed by write/read timeout or failure, malformed/truncated/wrong-kind/miscorrelated/action-incompatible result, wrong committed context/revision, or wrong started geometry | The command outcome is unknown. Retire the mixed carrier and discard unsent commands. Do not infer, retry, refresh, detach on the ambiguous carrier, or select a returned identity that was not validly correlated.                                                                                                                                                                                                                                          |

Every action-specific committed result must carry the captured context and revision exactly
`expected_revision + 1`; overflow makes a committed result invalid. `started.geometry` must equal
the captured geometry. The adapter validates these before the state machine treats a result as
committed.

An adapter-validated committed result is known as soon as the adapter returns it, including before
mailbox publication. The native executor immediately retains the full immutable `CreateOperation`
and committed result in one worker-owned pending value, before another fallible fence, deadline,
or publication step. It never overwrites an earlier pending value. Only consumption of the exact
`CommitRetained` reply clears it; publication, UI take, or reply deposit alone does not. The value
is returned through the existing native worker join handle on typed failure or cooperative close.
It is neither a last-commit cache nor another mailbox, event, or causal slot.

`CreateCommittedKnown` uses the original total deadline for placement and UI retention. This only
retains immutable facts; it does not select the new target or license forward work. The worker
must consume `CommitRetained` before SnapshotRead, candidate staging, or exact old cleanup. A reply
deposited before total expiry remains
consumable afterward: consuming it clears the pending value because the state already owns the
facts, but `now >= total` still prohibits SnapshotRead or Detach. Before total expiry, acknowledgment
after the forward cutoff cannot begin SnapshotRead, because no candidate was staged before the
cutoff. The final cleanup reserve applies only after a validated candidate already exists; within
it the executor may begin the exact old Detach or complete exact old cleanup, and completing it
there retires without installing the candidate or attaching. It never renews the original
45-second forward or 50-second total deadline.

Native UI retirement first closes admission, records the retirement request, and clears visible projection
and pending draw. It preserves the current state and mailbox until the owning `TerminalSession`
has shut down the socket and joined the worker without a mailbox lock. It then finalizes state
retirement using the mailbox's first failure and returned pending value, and releases the mailbox.
Every mutable product entry point completes that sequence before returning; no draw, action, or
read-only view can interleave with its private intermediate state. Drop uses the same shutdown and
join ordering. UI error paths must not discard a Creating barrier before this finalization.

The state finalizer accepts a pending value only for the exact full Creating operation, or as an
exact duplicate of an already retained operation and committed result. Commit construction checks
the captured revision, context, geometry, and reserved generation; recognizing a retained duplicate
compares the complete tuple and provenance. Snapshot validation continues to own created-member
membership and binding checks.
It preserves the old diagnostic active identity without publishing a live committed target.
Stale, future, or conflicting facts are not injected; a protocol mismatch cannot replace an
earlier retirement cause. Repeated finalization preserves the first cause and retained facts,
and a completed earlier Create cannot supply diagnostics for a later operation.

This handoff covers expected typed errors and cooperative closure, including closure between the
adapter return and event publication. An unexpected worker panic terminates its owning failure
domain: the existing exit guard closes the mailbox and the owner joins, but an unwound thread does
not promise return of its pending stack value. No generic panic recovery is introduced. The Create
model abstracts result acquisition and retention together; deterministic worker/UI tests establish
this refinement across publication, take, reply deposit, consumption, deadline equality, and close.

After a valid committed result, the same fixed forward budget first permits one SnapshotRead. A
valid exact or later response must pass `validate_created_tab`, preserve the retained historical
Create fact, and freeze the complete canonical candidate: order, revision, identity-aligned
provenance, selected ID and generation. Neither the returned ID nor any replacement selection is
published before that staging step. The returned member uses the already reserved generation 1. If
a valid later snapshot has removed it, its absence never erases the Create fact or rewinds the
cursor; the frozen historical ordinal selects a surviving fallback with generation 2, or Empty.

Ready-origin staging then licenses cleanup of only the exact old attachment. Exact correlated OBCS
`Detached` proves it. An opaque retained known-Closed fact may prove the same result only when it
matches the exact old Tab/binding/generation and owner cleanup is complete, and only before a Detach
has been handed off. Raw snapshot absence is never that proof. Definite `ServerBusy` retries only
inside the operation deadline. A definite non-busy rejection/unavailable, total expiry, or any
ambiguous handed-off Detach retires the carrier. Empty-origin staging skips cleanup because no old
viewer exists.

After exact cleanup, one UI transition atomically installs the staged cache, provenance, active and
desired identity, derived index, and generation. Only that transition can publish the replacement
selection or authorize Started attachment. If an exact Detach finishes in the final cleanup reserve,
the Client retires without installing the candidate. This preserves the known Create commit and old
diagnostic selection; it does not reclassify the command outcome as unknown. Repeated retirement
preserves those facts and the first cause; none grants input, selection, refresh, or recovery.

A definite Snapshot denied/unavailable before candidate staging enters `ProjectionUnavailable`.
It retains the committed tuple, revision, terminal outcome and reserved generation 1 together with
the exact old validated cache, viewer/controller, active selection and projection/feedback. Those old
facts are diagnostic only: the structural fence remains active, so Input, Resize, ordinary Poll,
selection, Create and Move are unavailable. `Ctrl+B w` may admit the explicit recovery below; exit
remains available. A valid recovery stages a candidate before cleanup and installation. A
valid-but-inconsistent snapshot is authority divergence and retires the carrier, as do malformed,
miscorrelated, or transport-ambiguous responses; none licenses a guessed Detach.

For Started recovery through a later Refresh, the original Create deadline and finished Refresh
transport deadline are not reused. After pure projection and created-member validation, the UI
samples the shared operation clock before taking the mailbox lock. Recovery atomically installs
`total = sampled_at.checked_add(50s)` and `forward_cutoff = total - 5s` with the attach continuation.
This fresh, conservative setup budget covers Attach, Acquire, Resize, Poll, actual draw, feedback
and exact cleanup; progress never renews it. It uses the retained Create attachment generation,
which has not yet attached, while every exchange still allocates a fresh request identity from the
single cursor. It neither resubmits Create nor admits a user Switch marker. If deadline arithmetic
overflows, consume and clear that exact pending Refresh as a refresh failure, retain
ProjectionUnavailable and the pre-recovery cache, committed facts and all identity/generation
cursors at their post-SnapshotRead values, send no Attach, and keep the carrier reusable. The already
issued Refresh request identity remains burned. Recovery of known non-attachable provenance
needs no setup deadline. Deterministic traces cover UI settlement delayed past both earlier
deadlines, successful recovery, fixed setup expiry, overflow preservation and fresh exchange IDs;
these implementation traces do not claim exhaustive coverage of this cross-product.

## Switch setup and supersession

The shared Switch installation and handoff foundation, Snapshot-first executor and late-discard
anchor invalidation are integrated and covered by state, mailbox and actual UI tests. This evidence
establishes the single-Client Switch runtime described below; the multi-Client PTY peer-removal
recovery witness remains separate.

The runtime maps Switch call handoff to one receipt under the existing mailbox mutex. The exact
active lease, original marker, cleanup token and old attachment, shared request cursor, and original
cutoff are checked together before authorizing the synchronous carrier call. This receipt proves
authorization, not transport submission or success; IPC never holds the mutex. A newer marker
cannot erase an authorized call. A compatible response retains the same receipt as answered until
the exact UI semantic commit and reply deposition release it together. Therefore both a new marker
before UI consumption and one after UI commit but before the worker reads its reply preserve truthful
settlement. Current Busy releases and rechecks under the gate; superseded no-effect replies retain
their receipt through typed settlement without clearing the viewer. Ambiguous calls keep their
authorization evidence through retirement. The sole shared membership record owns the candidate
and cleanup policy; this receipt adds no candidate, deadline, identity counter, or cleanup owner.

A returned old Poll can become stale after marker admission but before publication. The mailbox
retains that actual discard, and any relinquished already-taken Feedback settlement, as an exact
marker/source-attachment fact. Successful superseding admission transfers the fact to the matching
inherited anchor; a refused admission preserves it. Current Snapshot UI settlement consumes it
before preparing restoration or installation. Only the same source key loses its proof. A pre-call
skip, Busy without a frame result, or an empty stale callback creates no discard fact. This event
receipt survives the queued marker becoming active and allocates no request, generation, or budget.

The runtime membership owner separates an admitted Switch intent from its installed
attachment key. Intent admission consumes one generation. After the existing FIFO and exact
old-flight drain, one SnapshotRead uses the shared identity cursor and the marker's original
45-second forward / 50-second total budget. Its exact typed result must be placed before the
forward cutoff. A validated Available candidate remains staged behind the old attachment. Every
nonempty installation, including a surviving target and a fallback, consumes the checked successor
and atomically publishes cache, selection, desired/work key, cursor and attachment license. Empty
preserves the consumed cursor without a key, viewer, controller or fabricated Ready proof. The
membership model owns candidate validation and ordinal choice; the Switch model receives that
validated choice and checks the concrete key transition, cleanup and setup continuation.
An intent cannot install a projection, accept target setup configuration, publish Ready, or settle
as an unattached setup. Those paths require the installed candidate. The former direct
Switch-old-Detached event and its intent-to-Attach continuation have been removed.

A timely definite Unavailable result restores only an owner-held anchor from a genuine stable
origin. Ready restoration covers every live nondeparting pane in the original anchor, not just its
focus. Each pane requires the same exact attachment incarnation, binding, viewer/controller,
observed/admitted geometry, retained UI projection and accepted full-frame/draw/feedback proof after
truthful pre-marker settlement. Preparation is effect-free; commit restores all captured projections
together without Attach, input replay or a fresh generation. The owner-minted restoration proof also
publishes those exact pane incarnations into the mailbox in the same desired-install transaction,
so ordinary Poll and input admission can resume without a new reconcile. Already departing panes
remain departing. A missing or unfinished pane proof
prevents restoration, and abandonment of any captured pane invalidates the anchor. Accepted Resize that clears
the frame, unmatched latest geometry, unpreservable old Poll/causal abandonment, or viewer cleanup
or replacement invalidates that proof. The anchor follows those facts; an admission-time clone
cannot roll them back. A recorded selected-without-attachment or retryable origin may restore only
while it still has no viewer/controller, except an unbound selection with live siblings uses the
same exact whole-anchor projection proof described below. A partly completed Switch never creates a stable anchor;
in particular, a second marker can have no anchor after the first marker's not-yet-Ready viewer was
cleaned up. Current Unavailable without a valid anchor retires, without a repair Poll or setup.
Superseded results only settle their old flight. Exact restoration may return desired to the old
allocated key, while the allocation cursor never rewinds and the intent generation remains burned.
Timely Unavailable placement ends its transport budget, so exact UI settlement may occur later;
it starts no forward work and has no synthetic post-deposit deadline.

Gone can settle only a planned exact old cleanup before Detach handoff, using the actual Close
owner's exact known-Closed proof and with all earlier work settled and no wire request outstanding.
Snapshot absence is insufficient. Gone creates no pending Detach and cannot replace a response to
an already-handed-off request. Such a request still requires its original exact response or
truthful ambiguity retirement. Before installation, forward expiry permits only exact old cleanup
within the original total reserve followed by retirement; with no viewer it retires directly.
After installation, the existing no-attempt, attempted-setup and cleanup expiry distinctions below
remain in force. Candidate processing never renews either deadline.
The current Switch executor always obtains an exact Detach response for its old viewer; it does
not yet consume the Close owner's Gone proof as a shortcut.

The worker processes a current Switch marker as one setup operation:

1. settle earlier accepted Input/Resize;
2. settle any already handed-off Ready-generation Poll/Feedback response on the sequential carrier,
   then read, deposit and validate the current candidate before exact old cleanup and atomic fresh-key
   installation as above; Detach clears only an unsent/staged or exactly answered old projection flight;
3. exact AttachBinding for the installed candidate's validated Session/Worker/Terminal binding;
4. build a fresh `TerminalProjection` from returned AttachmentId/HostEpoch;
5. Acquire; controller-busy becomes ReadOnly, while Acquired begins Resize using latest real
   geometry and a fresh controller sequence;
6. Poll until the fresh projection accepts a full snapshot frame. Empty repeats. NeedSnapshot is
   fed back after the no-frame redraw and polling continues. ProjectionBlocked remains visibly
   Switching/blocked and requires a later-revision full snapshot;
7. stage an exact generation/Tab frame, draw it, send matching FrameApplied, require
   FeedbackAccepted, and only then publish Ready Controlling or Ready ReadOnly.

For a controlling setup, every accepted Resize records its exact geometry separately from the
latest measured geometry. The frame gate accepts only a fresh full frame whose geometry equals the
last accepted Resize. Immediately before Ready publication, the UI holds the mailbox mutex and
compares the latest measurement again. If it differs, Ready remains unavailable, post-Ready
Input/Poll admission remains closed, and the worker sends one catch-up Resize carrying the newest
measurement and a fresh controller sequence. It then restarts the full-frame, draw,
FrameApplied/FeedbackAccepted gate. A geometry change at Acquire, Resize, Poll, staged frame, draw,
or feedback therefore either becomes the first Resize value or causes this bounded catch-up loop.
An already drawn older-sized frame was genuinely visible and receives truthful feedback; it cannot
license Ready. ReadOnly setup records and renders to the latest local viewport but sends no Resize.
The shared setup deadline bounds continuous physical resizing.

Every admitted Switch marker captures its own monotonic 50-second absolute deadline and a forward
cutoff five seconds earlier; progress never renews either value. If a viewer exists, the worker may
start and finish exact Detach during the final reserve. Exact
cleanup after a failure of the current desired target ends in RetryableAttachUnavailable. Exact
cleanup caused by supersession continues directly toward the newest admitted marker and never
publishes the stale target as Retryable or Ready. A newer marker admitted while an older generation
is in cleanup retains its own independently captured budget; exact cleanup continues to it, while
its own already-expired budget forbids Attach and settles RetryableAttachUnavailable for an
attachable target or SelectedWithoutAttachment for a known non-attachable target. Non-Detached
cleanup or delivery ambiguity retires the carrier.
This bounds repeated Empty/NeedSnapshot/ProjectionBlocked and ServerBusy sequences without claiming
eventual Host progress. Initial startup continues to use its older startup-wide ceiling.

For a Switch candidate that still owns the previous viewer, exact old Detach and its correlated UI settlement
use that Switch marker's total deadline, including when the forward cutoff passes during the
exchange or before the UI consumes its result. UI settlement before the forward cutoff may return
the attach continuation. At or after the forward cutoff but before total expiry, it instead
atomically clears only the exact old attachment and retires without installing the candidate or
issuing Attach. After a candidate was already installed, current setup cleanup retains the
RetryableAttachUnavailable or known SelectedWithoutAttachment settlement below.
Supersession clears only the old attachment and continues to the newest marker. A reply deposited
before total expiry remains consumable later; taking it cannot license forward work past a cutoff.

A current installed Switch candidate with no viewer and no Attach attempt may exhaust its forward
budget before entering setup. This includes taking an installed attach continuation after the cutoff
even though its reply was deposited earlier. A marker or candidate that has not installed cannot
use this stable settlement; it follows the preinstallation retirement rule above.
With `now < total`, the same exact no-attempt expiry settlement publishes
RetryableAttachUnavailable or known SelectedWithoutAttachment without Attach; a stale marker
instead skips to the newer marker. The worker rechecks this boundary before the initial Attach;
an earlier clock sample or deposited continuation cannot authorize a late request.
This distinct no-attempt boundary is not a Detach claim and does not extend an attempted setup:
Attach rejection/unavailable and continuous Attach Busy still use forward-only setup settlement.

Exact cleanup includes placing its correlated current-generation settlement in the existing
`EventCell`. With `now < total`, the UI atomically publishes RetryableAttachUnavailable or
SelectedWithoutAttachment and deposits the matching `Reply` under the mailbox lock. At
`now >= total`, unfinished settlement expires and retires the carrier without publishing a stable
status. Timeout and UI settlement serialize through that lock. Once the reply is deposited,
the old deadline cannot retire or undo the completed settlement. The worker may consume that exact
reply later, then rechecks the desired generation, queued markers, and their own deadlines before
any further effect. The reply occupies the cell until consumption, preventing reuse. If a newer
marker supersedes the cleanup before UI settlement, the old result clears only the exact old
attachment and continues toward the newest marker without
publishing a stale status. Reusing the cell may invalidate only the exact expired `UiWait` key;
a late old reply cannot clear or overwrite a newer event, reply, or state. There is no third slot.

The reserve does not extend Attach, Acquire, Resize, Poll, Feedback, SnapshotRead, or ordinary
setup and semantic UI waits. Create precommit restoration, candidate validation that licenses old
cleanup, and any ordinary cleanup completion that permits candidate installation still require the
forward cutoff. An earlier-handed-off exchange may
finish under its original deadline, but its result cannot license new forward work after the
cutoff. Fake-clock traces cover each cleanup event placement, consumption, and reply boundary,
including supersession and exact deadline equality. These traces establish the named cleanup
orders, not exhaustive interleavings.

After every definite ServerBusy and every other blocking Detach/Attach/Acquire/Resize/Poll/Feedback
result, the worker reloads the mailbox's desired generation. Accepted pre-marker Input/Resize must
continue to truthful settlement. Old Detach remains a prerequisite even when superseded. For
superseded Attach/Acquire/Resize/Poll/Feedback, stop the retry: before Attach, skip; after any
Attached result, exact Detach before taking the newer marker. A possible handoff with unknown
delivery retires the carrier instead of guessing or continuing.

The one carrier never lets Detach overtake an already submitted Poll or Feedback. For a handed-off
old Ready Poll, read the exact response first; a definite Busy stops retry, an exact frame may be
dropped without drawing because the following Detach clears its feedback flight, and ambiguity
retires. For handed-off FrameApplied/NeedSnapshot, preserve its immutable request facts until the
exact response; definite Accepted/Busy/rejection settles it without retry when superseded, then
Detach, while ambiguity retires. Only a Poll/feedback action that was staged but never handed off is
abandoned directly. Scripted-carrier tests cut supersession before handoff, after Poll handoff, after
frame delivery, after draw, and after Feedback handoff.

An Attached viewer is also exactly Detached after any definite setup failure following Attach.
Attach rejection/unavailable before a viewer exists needs no cleanup. Acquire failure other than
controller-busy, Resize failure, Poll failure, feedback failure, deadline, or supersession after
Attach all require cleanup. Only one viewer/controller is therefore retained.

Every command/event is scoped by exact attachment generation and Tab ID. Stale event, projection,
feedback-accepted, or Ready reports are discarded. If a frame was already successfully drawn before
a later switch was admitted, it was genuinely visible; the Client schedules a clearing redraw and
never claims that no stale pixel was ever published. The guarantee is that a superseded generation
cannot publish Ready, accept post-marker input, or retain its viewer/controller. If FrameApplied was
already sent, it may settle before the mandatory Detach; if it was not sent, Detach clears the
flight.

A definite setup failure leaves the selected Tab selected and attachable provenance retryable. No
other Tab, old binding, Open, Create, Start, or terminal-lifecycle fallback is chosen.

## Ctrl+B w navigator and refresh

`Ctrl+B w` is an exact toggle. If the Workbench navigator is open, it closes and submits nothing.
If closed with no pending refresh, it opens immediately over the cached projection and attempts exactly one
`RefreshSnapshot { refresh_generation }`. Opening during a stable Ready,
RetryableAttachUnavailable, SelectedWithoutAttachment, or ProjectionUnavailable state may admit;
Starting, Creating, Switching, Retired, full mailbox, or refresh-generation exhaustion shows local
busy and sends nothing. If one admitted refresh has not yet been consumed by the UI, reopening only
displays that pending generation and cached content; it does not enqueue another SnapshotRead or
advance generation. There is at most one pending refresh across FIFO marker, in-flight exchange,
and unconsumed result.

Until the UI consumes that one result, a differing-target n/p/number selection is locally refused
and Create remains inadmissible; a same-target selection remains its ordinary no-op. Therefore no
refresh completion is applied during Switching, and every admitted Switch copies its target and
binding from a fully settled cache. Deterministic reducer traces cover refresh queued, in flight,
result queued while the navigator is closed, UI success/failure/divergence consumption, refused
selection with exact core preservation, RetryableAttachUnavailable same-target no-ops in all three
pending stages, and a later admitted selection or explicit retry after settlement. The
implementation traces do not exhaust this cross-product.

A full/refused opening still shows cached rows and local-busy status; cache, active/desired Tab,
attachment generation, projection, and latest admitted refresh generation do not change. Closing,
replacing the navigator with shortcut help/sidebar, or a successful selection/admitted Create only
hides it and never cancels an admitted SnapshotRead. Outside-prefix Escape remains terminal input
and does not dismiss an overlay. Same-Tab no-op, out-of-range selection, and admission refusal leave
the navigator open.

The navigator is informational. While it is visible, ordinary terminal byte batches continue
through normal Ready/Controlling admission; ReadOnly and barrier states apply their existing
rejection rules. The ClientView never treats a raw Escape byte as a UI event and never splits an
arrow, Alt, or other terminal sequence to implement dismissal.

Refresh shares the carrier and FIFO. It may not abandon a generation-current projection flight:
the same reserved causal-feedback rule drains any preceding Poll through draw and
FeedbackAccepted before SnapshotRead. Input/Resize admitted after a refresh marker waits behind the
read. The worker emits a refresh-generation-tagged typed result, then waits on the mailbox condition
variable for the UI to settle that exact pending generation. Later accepted Input/Resize remains in
FIFO and cannot execute while the result awaits semantic validation. The UI atomically applies
success/failure or retires on divergence, clears the pending generation, and signals the condition
variable; only then may the executor continue. Exit or carrier retirement also wakes it and discards
unsent records. Framing, correlation, wrong-kind, or transport ambiguity is detected before a typed
result is emitted and retires immediately. Thus the Refresh barrier spans pre-read causal-feedback
drain, wire read, and UI semantic settlement.

The executor retains the latest accepted snapshot as its baseline. Every logical Workbench read —
bootstrap, refresh, recovery, or post-commit projection — resolves to `workbench.snapshot.read`
while no baseline exists and to `workbench.publication.read` once one does. A `current` outcome
returns the retained baseline; `published` folds the ordered publications into it and stores the
result. A publication that cannot advance the baseline is authority divergence and retires the
carrier, never overwriting the prior cache. Ready-phase polling additionally probes
`workbench.publication.read` at most once every 250 ms; a `published` outcome sets the mailbox
ingress latch described above instead of entering admission directly, so an occupied refresh no
longer drops the demand: the UI captures it into canonical publication debt at the next stable
point and a later fair tick retries admission. `current`, denied and unavailable probes stay
silent and set no latch.

`RefreshMarker` intentionally carries no request ID. When it reaches the FIFO head after the causal
flight drain, the worker locks the same mailbox identity cursor, allocates exactly one SnapshotRead
ID, unlocks, and then performs I/O. Local identity exhaustion occurs before wire handoff: the worker
emits one exact local failure for that pending refresh generation, closes the mailbox, and retires
the now-unusable carrier; it never invents a second cursor or request ID. The refresh transport
budget is one monotonic 50-second deadline fixed at admission and shared by the preceding flight
drain and SnapshotRead. It may retire a queued/in-flight exchange at expiry. Once a typed result is
in the bounded UI event slot, the transport deadline is finished; navigator dismissal cannot cancel
it, and normal event-loop consumption settles it exactly once without a synthetic timeout.

The UI consumes the one pending generation exactly once. It defensively discards any typed
completion whose generation differs from that pending/latest-admitted generation without changing
cache, active/desired Tab, projection, navigator status, or committed-create recovery state. Such a
stale completion is an adversarial/test cut, not permission to queue concurrent reads.
Wire/framing/correlation failure has already compromised the one carrier and retires it even if the
associated logical generation later proves stale.

A current definite denied/unavailable result keeps cache, active ID and terminal projection and
sets refresh failure; definite unavailability during explicit recovery preserves that recovery.
The accepted Move refinement applies the strict projector and ID-authoritative replacement rule.
Lower revision, same-revision difference, missing/duplicate old ID, changed immutable tuple/binding/
Presentation label, Session/context mismatch or loss of the phase-required active identity retires.
Higher-revision supported reorder and Tab-label changes are valid. Success prepares provenance by
ID, atomically replaces cache/provenance, preserves active ID and derives its new index.
ProjectionUnavailable additionally validates retained create facts. MoveProjectionUnavailable
requires the known committed permutation at its revision, or allowed later history, before
restoring the retained stable phase without attachment effects. No push or freshness between reads
is claimed.

The navigator rows are one per canonical ordered member:

```text
[*N] <safe-label>   active
[ N] <safe-label>   inactive
```

`N` is the one-based decimal position and exactly one space follows `]`. The always-visible compact
strip uses `[*N <safe-label>]` and `[ N <safe-label>]`. Successful selection or admitted Create
dismisses the navigator. When `Ctrl+B w` closes it, its title and every row disappear; the compact
strip and Ready status remain unchanged. An in-flight refresh continues silently while closed.

## Rendering and localized observable states

Canonical labels are never normalized, trimmed, deduplicated, or truncated in Client data.
Presentation derives a temporary safe single-line string by replacing every Unicode control scalar
(including C0, DEL, and C1) and U+2028/U+2029 with U+FFFD. Ratatui lays out that safe string in a
one-line bounded `Rect`; buffer clipping, measured in terminal cells, never writes beyond it and
does not split a wide cell. When the compact strip width can hold the active token prefix `[*N `,
horizontal scrolling exposes that prefix in full while preserving canonical order. A narrower Rect
renders only its safely clipped leading cells with no overflow or control bytes. Navigator rows
vertically scroll to keep the active row visible. PTY witnesses use a viewport wide enough for the
complete active token.

`FixedViewportBackend::draw` is the final trust-boundary guard because it calls
`Print(cell.symbol())`. Before `Print`, it applies the same forbidden-scalar replacement to every
cell symbol. No control/newline/C1 byte sequence from a Workbench label can become a terminal
command. Renderer/backend tests cover duplicate labels, nonempty whitespace-only labels,
C0/ESC/CR/LF/DEL/C1, U+2028/U+2029, combining and double-width characters,
one-column clipping, an active token after many Tabs, and exact canonical data after rendering.

The stable English and Chinese status strings are below. Ready and control-mode
context appears on demand with `Ctrl+B`; ordinary terminal passthrough has no status bar. Ordinary Move admission refusal preserves
the existing notice without reacquiring the mailbox; the local-busy row applies to the other
structural admission paths.

| Semantic state                       | en-US                                             | zh-CN                                 |
| ------------------------------------ | ------------------------------------------------- | ------------------------------------- |
| Creating                             | `Creating Tab…`                                   | `正在创建 Tab…`                       |
| Definite Create precommit refusal    | `Tab not created · previous Tab unchanged`        | `未创建 Tab · 原 Tab 未更改`          |
| Refreshing                           | `Refreshing Tabs…`                                | `正在刷新 Tab…`                       |
| Refresh success                      | `Tabs refreshed · revision {R}`                   | `Tab 已刷新 · 修订 {R}`               |
| Refresh failure                      | `Tab refresh failed · showing revision {R}`       | `Tab 刷新失败 · 正在显示修订 {R}`     |
| Local admission refusal              | `Client busy · no command sent`                   | `Client 正忙 · 未发送命令`            |
| Numeric selection out of range       | `No Tab at position {N}`                          | `第 {N} 个 Tab 不存在`                |
| Switching                            | `Tab {N} · Connecting…`                           | `Tab {N} · 正在连接…`                 |
| NeedSnapshot before first frame      | `Tab {N} · Connecting · waiting for full frame`   | `Tab {N} · 正在连接 · 等待完整帧`     |
| ProjectionBlocked before first frame | `Tab {N} · Connecting · display blocked`          | `Tab {N} · 正在连接 · 显示已阻塞`     |
| Ready Controlling                    | `Tab {N} · Ready · controlling`                   | `Tab {N} · 已就绪 · 可控制`           |
| Ready ReadOnly                       | `Tab {N} · Ready · view only`                     | `Tab {N} · 已就绪 · 仅查看`           |
| Committed snapshot unavailable       | `Created Tab committed · refresh required`        | `已提交创建的 Tab · 需要刷新`         |
| Known terminal pending               | `Tab {N} · terminal creation pending · no replay` | `Tab {N} · 终端创建等待中 · 不会重放` |
| Known terminal unavailable           | `Tab {N} · terminal unavailable`                  | `Tab {N} · 终端不可用`                |
| Known terminal failure               | `Tab {N} · terminal creation failed`              | `Tab {N} · 终端创建失败`              |
| Known terminal outcome unknown       | `Tab {N} · terminal result unknown · no replay`   | `Tab {N} · 终端结果未知 · 不会重放`   |

Refresh statuses render inside the open navigator. They are retained but visually silent while it
is closed. PTY observers explicitly open the prefix status overlay and require one active
compact token with the exact Ready string, then dismiss the overlay before continuing input.
A transient Creating/Refreshing draw is never required.

A definite Create precommit settlement never creates a timer-driven phase. The UI consumes the
typed result by restoring the exact attachment directly to Ready and storing the localized notice
in `Ready.notice`; that notice opens the status overlay until the next decoded
`Ctrl+B` Client action atomically clears or replaces it. Ordinary terminal bytes, Poll/frame
traffic, and physical Resize observations do not clear it. A Pending committed target remains
SelectedWithoutAttachment with the exact Pending status above. `Ctrl+B w` can refresh canonical
membership, but it does not replay Create, attach that Tab, or erase its retained Pending
provenance.

## Rename transport adapter

The private `WorkbenchTabRenameRequest(WorkbenchCommand)` accepts only `TabRename` and preserves
the complete command, including its request identity, expected revision, target, and exact label.
`LocalClient::submit_workbench_tab_rename(request, deadline)` uses the existing single exchange.
It requires a command result with the original request identity: `Renamed` must have exactly
`checked(expectedRevision + 1)` and `Unchanged` exactly `expectedRevision`, including the maximum
revision. `Renamed` carries only revision; there is no target or label echo to validate.

Compatible precommit rejections are exactly `site-mismatch`, `revision-mismatch`,
`terminal-context-unavailable`, `tab-unavailable`, `resource-limit`, `revision-exhausted`, and
`request-identity-conflict`; preserve their reported revision. `denied`, `request-ledger-full`, and
`service-unavailable` are also compatible. Other outcomes or reasons, wrong correlation, malformed
responses, and impossible success revisions retire the carrier as `OutcomeUnknown` with the
original request identity. Encoding and failures before any possible write are
`DefinitelyNotSubmitted`; uncertain completion after possible handoff retires without retry.

The deadline remains the earliest caller, per-exchange and startup deadline. There is no SnapshotRead, Start, Attach, selection, label mutation or request identity allocation. The classifier adds no state, clock or retry semantics. Adapter tests check complete typed-request preservation through decoding, compatible/incompatible outcomes, revision bounds and prewrite/post-handoff deadline failures; bytes and revision arithmetic remain concrete implementation evidence.

The native Rename command and mailbox/UI flow consume this transport foundation. The adapter
alone does not own editor activation, membership installation or attachment cleanup.

## Rename projection evidence

`workbench_projection::renamed` validates a snapshot against a captured canonical baseline and
known Rename revision. The exact context/Tab must exist in the baseline, the requested label must
differ from its old label, and the committed revision must be `checked(baseline + 1)`. At that exact
revision, the complete projected vector must equal the baseline with only the target Tab label and naming mode
replaced by the exact requested text. Other label changes, reorder, addition, removal or immutable
member changes are divergent.

A later revision uses the shared strict canonical membership rules. The renamed Tab may have a
later label or be absent; the validated result carries the candidate and an optional target index.
Neither case invalidates the historical commit. The expectation and baseline are borrowed and
remain unchanged. A same-label `Unchanged` result is outside this known-commit validator.

This is pure projection evidence, not proof of terminal disposal, attachment cleanup or UI
installation. Focused tests cover exact label preservation, revision overflow, forbidden changes
at commit, later evolution/removal/Empty and overlapping identity rejection. It adds no state
machine or model claim. The native Rename consumer uses this evidence before the shared membership
installer and retains the common cleanup owner's independent proof requirements.

## Rename Unicode draft buffer

`state::rename_text::RenameText` holds exact editable label text for the native Rename
draft. It copies a validated canonical label, exposes borrowed text, appends complete UTF-8 text
atomically within `MAXIMUM_LABEL_LENGTH`, removes one final Unicode scalar, and clears to Empty.
An over-limit append preserves the complete old text; an empty append and clearing Empty are
no-ops. The buffer uses one String and counts scalars without a second maintained counter.

Empty is valid while editing. Producing a label calls the existing `WorkbenchLabel::parse`, so
Empty cannot submit and whitespace-only text remains valid. Whitespace, normalization form,
combining scalars, supplementary scalars and schema-valid controls remain exact; display
sanitization never modifies the buffer. Removing a scalar is not a grapheme-deletion or physical
Backspace contract. A returned label owns its text independently of later buffer edits.

Tests cover exact text, Empty, the 511/512/513-scalar boundaries, atomic refusal, scalar deletion
and independent returned labels. This deterministic bounded String transformation introduces no
clock or state-machine model. It does not parse raw input, establish draft identity or drawing,
allocate requests, admit commands or perform I/O. The state-owned RenameDraft and actual input
dispatcher consume this buffer; command admission remains with the mailbox transaction.

## Native Rename consumer

The Rename submission consumer uses the concrete shared Move/Rename pending mutation in state and
mailbox. Under the existing mailbox lock, it replans the exact visible draft, prepares request and
serial candidates, and installs the operation, marker and cursor only after all refusals pass.
Typed result correlation and flight stage are checked before the joint state/membership commit.
The worker uses the stored request once; known commit retention precedes Snapshot and survives
transport retirement. The persistent input dispatcher and actual renderer consume these methods.

The private Rename operation owns one immutable typed request, captured draft intent and original
deadline. Result preparation independently checks unchanged-label/base-revision and changed-label/
checked-successor semantics, then calls the strict Rename projector. Valid known commits remain
available when projection validation fails or Snapshot is unavailable. This operation preparation
is consumed by the mailbox/executor/UI flow; preparation itself does not allocate identities, submit a
command, install membership or create another flight/cleanup owner.

Rename's last-settled receipt follows Move's latest UI-consumed known commit, not Create's latest
successful membership install. A survives B admission, queueing, refusal, precommit and no-op.
Worker-known B coexists with state A until exact B UI consumption retains B, before any fallible
membership continuation. Projection unavailability retains B plus recovery B and the historical
cache. A's worker reference survives until its exact ACK; the single executor cannot execute B
while that ACK is pending. Thus replacement by UI-consumed known B cannot leave a legitimate
unacknowledged A reference. No permanent old-request ledger is introduced.

After timely typed placement, pure candidate validation precedes any membership budget. A valid
candidate requiring an attachment transition gets one clock sample after validation and before
the reply lock, with forward cutoff at 45 seconds and total cutoff at 50 seconds. The common owner
stores that exact pair through cleanup/setup and delayed UI consumption; it never renews it.
Preserving the current key allocates no new membership budget.

The state-owned Rename draft consumes `RenameText`, captures the complete canonical baseline and
exact selected attachment generation, and exposes an opaque draw token. It allocates no command
identity or deadline. Exact successful drawing qualifies submission; a missing token revokes that
qualification. The state independently revalidates authority before returning a submission intent,
and refuses structural admission while the draft exists. Explicit cancellation and retirement
destroy the draft. The input, mailbox and UI consumers connect these state operations to the
keyboard entry and exact-token submission without letting the decoder construct requests.
`state/tests/rename_draft.rs` exercises visible and revoked tokens, exact unchanged intents,
atomic text-limit faults, explicit correction, incomplete input, independent baseline/generation
staleness, cancellation, retirement and reciprocal state admission. Actual mailbox tests cover
capacity, cursor/serial exhaustion, fixed-cutoff refusals, exact ACK and retained worker-loss evidence.

`Ctrl+B ,` opens a state-owned draft for the exact selected Tab in the active context in a live stable phase.
Capture the canonical baseline, Session/context/Tab and attachment generation without allocating a
request or deadline. Empty, structural/recovery flights and Close/Resume confirmation refuse entry.
The draft and Close confirmation are mutually exclusive. Applied authority or selection changes
invalidate the draft; Enter revalidates the captured tuple atomically and never retargets its text.

The draft preserves exact schema-valid text and uses the bounded `RenameText` operations. Initial
successful drawing of its exact opaque token is required before Enter. Missing/failed drawing,
hidden editor, viewport invalidation or draft replacement revokes qualification. Later text edits
do not require a draw per scalar. A returned label remains exact; display sanitization is separate.
Input faults clear only on explicit decoded Backspace or Ctrl+U, including Empty; these still pop
or clear. Successful typing/paste, ignored sequences, failed edits and Enter cannot clear a fault.
The localized error names Backspace and Ctrl+U. Cancellation destroys the draft and its fault.

One existing input reader and terminal-mode owner handle the editor. ESC waits 50ms; CSI/SS3 uses a
32-byte bound; fragmented UTF-8 retains at most three prefix bytes. Bracketed paste appends
atomically within remaining label capacity and recognizes its six-byte terminator across reads.
Payload controls do not submit, cancel, clear faults or execute prefix actions. Overflow or invalid
paste drains through the terminator without accepting a prefix. Paste-mode activation failure
refuses entry. The persistent input parser maintains these states across reads/ticks.

Cancellation/submission retains recognized framed drains. Only after those complete may a 50ms
quiet guard restore ordinary input; each discarded byte resets that guard. At equality, expire
the guard before processing newly sampled input. Already recognized framing remains isolated
across arbitrary gaps. An opener completed only after ESC classification and quiet handoff expiry
is interpreted as new input: no provenance guarantee covers arbitrarily late unframed bytes.
There is no extra resume gesture, second stdin reader or transport-budget renewal.

Enter captures one fixed 50-second budget before locking. Phase, shown token, complete/valid input,
captured authority, capacity, cursor and exclusivity validate before atomic request/marker admission.
Use a concrete Move/Rename enum in the existing structural mutation slot; Close's flight checks
exclude it reciprocally. Editing/cancellation/refusal burns no request identity. Unchanged text is
still submitted once for Server authority. Pure Rename preserves attachment and Ready facts.

`Unchanged` requires the baseline revision and equal requested text, with no SnapshotRead.
`Renamed` requires changed text and the checked successor; retain the exact operation and known
revision before one same-budget SnapshotRead. The result has no target echo. Exact revision
requires the label-only projection; later strict membership may overwrite or remove the target.
Definite command precommit clears the exact flight after UI settlement. Only postcommit projection
unavailability retains a recovery fence and historical cache, while same-target effects may remain
usable. Explicit Refresh gets its own fixed budget and never replays Rename.

Timely exact typed placement ends its transport budget; UI consumption is untimed. Distinguish
placement, UI consumption, executor acknowledgment, worker return and join. Known commits and
uncertain submissions retain exact operation keys. A consumed A followed by admitted B cannot let
a late A acknowledgment clear B. An older worker return cannot prove current-worker return.
The single executor waits for A's acknowledgment before either handing B off or completing B as
definitely not submitted; B's UI settlement cannot overwrite a pending A acknowledgment.
Queued/definite non-submission retirement never invents submitted uncertainty.

Validated candidate installation uses the shared exclusive state/mailbox transaction. Snapshot
absence never proves cleanup. The shared installer refuses active-target replacement while
attachment facts remain; the Rename consumer must use the common cleanup continuation and its
stored deadline owner. Rename cannot bypass it or acquire new Ready from membership alone.

Successful Rename entry dismisses an open navigator through `ClientView.commit`; refusal preserves
that overlay. The initial token is acknowledged only after the complete localized editor is drawn.
The same renderer receipt carries independent optional Close and Rename tokens, and outer viewport
changes revoke Rename qualification before input even when terminal columns remain unchanged.
The existing terminal mode lease enables bracketed paste before entry and retains it through any
recognized drain and quiet handoff; enable failure creates no draft or command.

`terminal_session/tests/rename_interaction.rs` drives the persistent dispatcher, actual renderer,
NativeUi, mailbox and executor over a local scripted socket. It covers both locales, unseen Enter,
sticky input faults, exact maximum scalar paste, cancellation drain, viewport invalidation,
navigator dismissal, authoritative no-op, changed projection, explicit recovery, malformed known
projection, ambiguous exchange and lost postcommit Snapshot. Later target removal exercises the
common Detach before Empty or successor Attach; a definite unavailable successor does not become
Ready. These tests reject unsolicited Start, Close or repeated Rename requests.

### Rename lifecycle model boundary and evidence

Rename uses the production state, Unicode draft, mailbox and connection tests described above. Controlled completion orders cover stale drafts, result handoff, publication, cancellation and retirement. This single-owner workflow has no separate TLA model; shared pane identity remains covered by the pane-incarnation protocol.

## Effectful adapter contract

Add an opaque `WorkbenchTabCreateRequest(WorkbenchCommand)` beside snapshot submission. Its pure
`TryFrom<WorkbenchCommand>` accepts only `TabCreate` and returns every other owned command unchanged
as the conversion error; construction performs no state mutation or I/O. The only effectful entry
is `LocalClient::submit_workbench_tab_create(request, caller_deadline: Instant)`. There is no generic
Workbench-command submit method and no locally invented unsupported-action wire result. Before
handoff the method encodes the validated request. After handoff it requires
`workbench.command.result`, exact request ID, and an action-compatible outcome. For `tab.create`,
`denied` and every closed
`WorkbenchCommandUnavailableReason` are compatible. Its compatible `rejected` reasons are exactly
`site-mismatch`, `revision-mismatch`, `terminal-context-unavailable`, `resource-limit`,
`identity-exhausted`, `revision-exhausted`, and `request-identity-conflict`;
`project-unavailable` and `workspace-unavailable` are action-incompatible. Project/workspace result
variants and `unchanged` are also incompatible; only TabCreatePending/TabCreated committed variants
match `tab.create`.

For a committed Tab result the adapter also requires captured context ID, revision exactly
`expected + 1`, and, for Started, exact requested geometry. It returns an explicit
`DefinitelyNotSubmitted`, `DefinitePrecommit`, `Committed`, or `OutcomeUnknown` classification.
Encoding/no-write failures are definitely not submitted. Once any write may have begun,
write/read/deadline errors, malformed/duplicate/truncated responses, unexpected response kind,
correlation mismatch, action mismatch, wrong committed fields, or wrong Started geometry retire the
carrier and are OutcomeUnknown. The state machine never converts them to a safe retry.

Add internal caller-deadline forms for terminal viewer, terminal control, and SnapshotRead. Each
reuses `exchange_once` and clamps its ordinary per-exchange deadline to the earlier of the supplied
deadline and any existing startup ceiling; the existing ordinary methods keep their current
own-budget behavior. `next_operation_deadline` reads the existing injected monotonic clock, never a
separate `Instant::now`. Switch/Create capture `total = sampled_at.checked_add(50s)` and
`forward_cutoff = total - 5s` once: every forward exchange/retry receives the fixed cutoff, while an
exact cleanup Detach and its correlated UI settlement receive the fixed total deadline. Neither
renews. Refresh passes its one fixed 50-second transport deadline; startup keeps its existing startup ceiling.

The deadline origin above is the clock sample taken before acquiring the mailbox lock. A private,
cloneable `LocalOperationClock` wraps `Arc<dyn MonotonicClock>`; the private clock trait requires
`Debug + Send + Sync`. Clone the same handle before moving `LocalClient` into the worker, so UI
admission and I/O share one injected source. Only the test stepping clock's receive endpoint needs
a `Mutex`; the production clock has no lock. UI admission samples before locking, computes
`total = sampled_at.checked_add(50s)`, and atomically commits the fixed deadlines with the marker,
request identity, and generation. Overflow refuses before any mutation. Successful admission thus
has a conservative budget of at most 50 seconds, including time spent waiting for the lock; no
clock guard or I/O crosses the mailbox lock. The private `WorkbenchTabCreatePrecommit` and
`WorkbenchTabCreateCommit` payload enums contain exactly the compatible precommit and committed
variants above, rather than admitting unrelated command results. These representations add no
wire outcome and do not change the models' abstract clock or result classes.

Deadline-bound mailbox event operations sample the shared clock outside the mutex and use
nonblocking lock acquisition. After contention they resample before another attempt and revalidate
the exact event key and fixed deadline under the acquired guard. A narrowly scoped `yield_now`
retry handles mutex contention; queue, event, and Busy waits continue to use the condition variable.
A pre-lock sample followed by blocking acquisition cannot authorize settlement after ordinary
mutex waiting crosses the deadline. Injected-clock tests hold the mutex across expiry and assert
that the settlement callback never runs. Scheduler suspension after the final sample and kernel
stalls remain environmental exclusions; ordinary lock contention cannot renew or extend the budget.

Snapshot reads remain non-effectful, but malformed/wrong/correlation/transport failures also retire
the shared carrier. Definite denied/unavailable may leave it live. All IDs/bindings are converted
between OBWP and OBCS with checked exact textual parity; no protocol crate depends on the other.

## Claim/evidence matrix

The matrix summarizes the current Client evidence. The Move-specific record above and refinement
checks below extend the original Create/selection/Refresh models; append-only checks alone do not
establish reorder preservation.

| Claim                                                                                                                       | Lowest evidence                                                                                                     | Additional witness                                                                            | Limits                                                                                       |
| --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Strict 0..256 projection and identity-preserving refresh                                                                    | exhaustive partition/unit cases and corpus-derived fixtures                                                         | generated create/rename/move histories and exact commit-permutation tests                     | Higher revisions may reorder and rename Tabs; immutable member facts survive.                |
| Canonical split-tree projection preserves leaf/split/slot identity and orientation                                          | `projects_split_tree_and_directional_leaf_neighbors` plus strict structural-ID checks                               | canonical snapshot projection through the real `NativeTabView`                                | Canonical pin/group metadata and bound and unbound primary/child Presentations are accepted. |
| Local Panel focus keeps the Tab and switches to the exact selected leaf binding                                             | `panel_focus_switch_keeps_the_tab_and_targets_the_selected_leaf_binding` plus NativeUi switch admission             | existing Switch cleanup/Attach lifecycle and renderer tree traversal                          | One viewer is attached at a time; inactive leaves do not show a second live frame.           |
| Panel split/close/move submit through structural admission and settle exact commit, refusal, unknown, and membership repair | `native/tests/panel.rs` mailbox/worker settlement cases plus exact split/move candidate validators                  | local-client outcome classification tests and the common membership cleanup/install lifecycle | Provisional key bindings may change; close relies on the ordinary publication path.          |
| Selection wrap/bounds/same/retry and atomic refusal                                                                         | pure selection table tests                                                                                          | generated mailbox traces with stable seeds, semantic shrinking, and replay text               | Finite generated trace sizes are reported, not universal proof.                              |
| Create drains old Poll/Feedback before submit and precommit restores                                                        | controlled implementation tests plus fake-carrier traces                                                            | deterministic old-frame/draw/feedback checkpoint trace                                        | Fake-carrier traces do not prove physical rendering or arbitrary scheduling.                 |
| Commit validates/stages a Snapshot candidate before exact old cleanup and atomic selection; only surviving Started attaches | Actual UI dual-plan installation, candidate-first carrier and generated mailbox traces                              | fake carrier at each blocking response                                                        | Carrier retirement cleanup is not a successful Detach or candidate-install claim.            |
| Rapid switch cleanup/generation/first-frame/geometry gate                                                                   | controlled implementation tests plus deterministic core traces                                                      | real PTY active-token + Ready checkpoints                                                     | Controlled traces do not prove arbitrary Create/refresh interleavings or physical pixels.    |
| Refresh single-pending generation fence/identity/active preservation and navigator toggle                                   | controlled implementation tests plus deterministic stale-event tests                                                | two-Client/process refresh witness                                                            | No push or inter-read freshness.                                                             |
| Peer publication demand retained through Busy/Full admission and installed without manual refresh or a second mutation      | controlled implementation tests plus deterministic mailbox/UI debt tests                                            | real two-Client PTY/Server/Worker/Host publication witness                                    | One observer Client and one committed change; no push or concurrent-publication claim.       |
| Move preserves the selected terminal and exact known commit                                                                 | controlled implementation tests, mailbox/state, worker and actual NativeUi/outer retirement traces                  | two Linux cohort Move scenarios                                                               | PTY shell continuity does not independently prove absence of viewer reattachment.            |
| Total setup termination                                                                                                     | fake monotonic clock at Empty/NeedSnapshot/Blocked/Busy and cleanup reserve boundaries                              | PTY uses named Ready/unavailable checkpoints, never sleep                                     | Does not prove Host eventually responds successfully.                                        |
| Safe exact label rendering                                                                                                  | pure sanitizer/layout/backend byte tests                                                                            | PTY numeric labels and active marker                                                          | OS font/pixel fidelity is excluded.                                                          |
| Platform dependency boundary                                                                                                | Windows target `check/test` for help/version/unavailability plus Cargo metadata assertion; Unix build/tests for TUI | Linux real PTY                                                                                | No Windows TUI claim.                                                                        |

Stateful generated tests use an in-package deterministic generator rather than a new runtime
dependency. Stable seed corpus, maximum trace length, action/outcome coverage counters, and a
canonical serialized trace are logged. On failure the harness removes action chunks, then shrinks
indices, queue occupancy, generation, outcome, and geometry values; every minimized trace is
directly replayable and becomes a named deterministic regression. Coverage assertions require
Create/precommit/each committed class, full admission, pre-Poll feedback, refresh stale/current,
switch supersession at every blocking boundary, controller-busy, NeedSnapshot, Blocked, timeout,
cleanup, and carrier retirement. A named repeated-Create coverage class must execute at least two
human operations in one Session. It covers Ready-to-second-`c`, fresh checked identities and burned
generations, preservation of each Tab's provenance, rejection of reuse of the first committed
tuple, and re-entry after a definite precommit refusal.

The Move mailbox captures one request ID, unchanged attachment key,
baseline, direction, and deadline in `MoveOperation`. Its structural fence spans queued,
in-flight, and awaiting-UI stages, and releases only when the exact typed UI settlement is
deposited. Timely result placement permits deadline-free UI consumption. Admission refuses
deadline, capacity, identity, and target failures without consuming an identity or attachment
generation. Deterministic mailbox tests cover these cuts, including lock contention and a
second Move queued before the first worker acknowledgment is consumed.

`state/move_tab.rs` retains pending operation, committed recovery and exact settled-commit records
over the unchanged stable attachment phase. `MoveProjectionUnavailable` is an explicit display/state
kind; its recovery record blocks Create, Switch and Move while preserving Ready input, Poll, Resize,
controller and historical frame state. Raw `Ctrl+B w` admits fresh Snapshot-only recovery, validates
the original Move expectation and remaps the selected ID without an attachment generation or setup.
The outer worker join retains both Create and Move known-commit handoffs.

Six focused state tests cover controlling and read-only historical Ready, both unattached phases,
nonfirst selection, definite refusal, singleton no-op, corruption and higher-revision recovery.
The combined canonical-state/mailbox trace settles Move A, queues B before A's worker reply is read,
then retires and recognizes only A's exact commit without replacing B. The real NativeUi/carrier test
drives raw Move and `w` keys, renders the reordered active position, exercises input/Poll/Resize during
explicit unavailability, rejects structural actions, and permits only SnapshotRead during recovery.
Nine worker transport tests separately check cutoff placement, deadline-free UI settlement, exact
request allocation, retained viewer authority and known commits across transport/retirement cuts.
An actual NativeUi contention regression holds the mailbox through admission expiry and requires
the refusal to return before unlocking, preserving the notice, viewer and identity cursor. Ordinary
Move refusal therefore performs no second lock acquisition. A separate outer TerminalSession test
interrupts a postcommit Snapshot read, joins the worker and verifies the exact retained Move commit
and first retirement cause after finalization.

At runtime source `f2467e8e`, the native binary suite passed 311 tests with one ignored, and the
complete `local_startup_process` target passed all 23 tests on Linux under WSL. Its two Move
scenarios exercise six Move steps over three Tabs, both directions and boundary wrap, distinct shell
PIDs, fresh output and Resize, independent peer selection and explicit refresh, and canonical-first
reopen followed by revisiting surviving terminals. Ordered completed-frame observers require one
active marker and its exact Ready context in an explicitly requested prefix overlay. These process checks establish observable shell
continuity; the worker and UI traces separately establish attachment preservation. They do not
establish other operating systems, installation, restart recovery or Close.

## Model scope

Formal evidence for this owner is the pane-incarnation core described in [Native Panel lifecycle evidence](native-panel-lifecycle.md#finite-model-evidence), plus the separate viewport and explicit control-takeover protocols. It does not enumerate every Create, Close, Move, Rename, refresh or context-removal combination.

The production state and mailbox generated tests, deterministic connection completions, workbench-projection traces and process tests own those operation-specific contracts. Keep stale identity, unknown outcome without replay, committed-result publication and cleanup-before-replacement cases in these suites when changing an operation. A retained protocol pass does not replace the corresponding implementation test.

## Model verification

Run `cargo test --locked -p orchbee --all-targets --all-features` for the owning implementation evidence. Run `node scripts/tla/run-models.mjs --workspace apps/orchbee` when retained protocol state, transitions, assumptions or mappings change. The full TLA gate is `pnpm test:tla`; it includes fresh complete model checks and mutation controls.

Historical state counts and narrowed scenario families are not current proof authority. The registered modules/configurations and their current owner mappings define the finite formal scope.

## Retired peer membership recovery

This entry is for a non-initiating native Client whose exact current ordinary Poll returned
`TerminalRuntimeUnavailable`. It retains that correlated request and attachment key, immutable
binding, canonical Session/context baseline and historical ordinal. Stale or setup Polls, transport
uncertainty, malformed replies and other failures do not establish this origin. Activation requires
successful exact old-task join, closed old transport and settlement of all returned facts. Panic or
missing return evidence cannot establish eligibility. Any retained Close/Read record or flight, or
unresolved structural mutation, commit, setup or cleanup handoff excludes this entry. Existing
Close status recovery retains priority and all its facts.

The same correlated `TerminalRuntimeUnavailable` on a structurally quiescent Ready focus is
pane-local: the lost viewer is disposed and the exact selection is precisely parked for the
existing publication reconciliation, so it does not establish this retired origin on its own.
The whole-Client proof still applies while a structural owner is armed — an opened but
uncommitted panel-close confirmation makes local settlement inadmissible; retirement cancels
that confirmation, and the origin then retains its original facts (same Poll serial, unreplaced
Ready, exact selected+desired and still-attached canonical PaneRef, no submitted mutation or
refresh). A submitted Panel operation and every other structural owner keep their existing
exclusion.

An explicit `Ctrl+B w` after finalized retirement may start one Snapshot-only connection through the
retained local origin. There is no automatic reconnect. The same task owner and race-safe shutdown
handle govern the connection. A late connection after cancellation is closed. The opaque retired
mailbox seed preserves identity, attachment, Refresh, command and UI-wait allocation progress;
new queues and flights are empty and the new mailbox begins `Live { desired: None }`. Request IDs
continue across connections; neither the old selected key nor the new connection generation is a
replacement for the retained allocation cursor or canonical authority check.

Exact join prohibits **new old-local submissions and Viewer reactivation**. It does not prohibit
completion of previously handed-off remote work. Server Drop may enqueue asynchronous Detach;
local retirement is neither Detached nor Host Gone. The new connection inherits no Viewer or
controller lease. An old remote reference may remain and consume capacity; this entry does not
claim its eventual release or permit the new connection to adopt or release it. This is a separate
retired entry into common membership installation, not a relaxation of live cleanup evidence.

Each explicit attempt captures one checked forward deadline at 45 seconds and total deadline at
50 seconds. This new origin policy starts at the user action, covering connect, Snapshot placement,
validation, UI installation and neighbor setup. It does not change ordinary Refresh's candidate-time
budget or the separate Close status budget. There is no deadline renewal after a result or UI wait.
Only exact cleanup of a newly attempted setup may use the final reserve. A further parked attempt
requires another explicit action after the previous carrier has closed and joined.

Before installation the only request is SnapshotRead. The full candidate must preserve Session/context,
nondecreasing revision, exact same-revision membership and every immutable retained identity tuple.
The old unavailable Tab must be absent, or its selected Workspace Panel must now carry both
a fresh Presentation and a fresh terminal binding under the replacement rule above. A surviving
old physical identity, unavailable Snapshot or invalid candidate leaves the old canonical state
and origin parked, with no Attach. Valid replacement keeps the selected Tab and Panel, mints a
fresh attachment generation and clears old launch provenance. Absence or replacement is membership
knowledge, not cleanup evidence. The existing explicit `Ctrl+B w` entry also covers this replacement;
it does not reconnect automatically. Deterministic UI tests cover the replacement branch; the
retired-recovery model results below predate it and do not establish that extension. Bootstrap, Open, activation, Create, Start and Close are not part
of this observation entry.

While an armed attempt owns the mailbox and before any candidate installs, the session
admits no ordinary scheduled work against it: the post-event tick scheduler block
(ordinary poll reconcile, publication refresh, close reconciliation, close read, pane
reconcile, and poll), the observed-pane geometry Resize admission, and the pane
reconcile enqueue itself are suppressed by the exact attempt marker, not by session
phase. Admission-free bookkeeping continues unchanged — latest UI and mailbox
geometry, updated by the admission-free focus-geometry observation path outside the record
sync, close acknowledgement, event drain, and pending-draw reporting — and no
retained fact is cleared nor deadline renewed to escape the window. Canonical
pane records stay untouched: a drawn pane observation returns before the
record sync inside this window, because a minted unattached pane fact would
violate the Retired pane-empty shape the install-time origin check must still
see. A draw-driven pane
observation or a tick landing inside the window cannot mint a Resize, a reconcile
episode, or any ordinary command serial: the queue carries the recovery Refresh alone,
and the executor clears its membership flag after the Refresh reply before the next
command. The suppression is scoped to the membership attempt marker alone; the separate
status-only Close recovery keeps its own close-carrier suppression of the same reconcile
funnel. After the atomic
candidate installation clears the marker, ordinary scheduling may resume in the same
tick.

Prepare the candidate outside the reply mutex, then atomically commit the state and mailbox plans
under that mutex after checking the exact retired origin, baseline, flight and forward deadline.
Choose the clamped historical ordinal or Empty. A survivor advances the actual burned attachment
cursor once at installation; Empty preserves it. Retained provenance follows the common projector.
A survivor uses the remaining setup budget on the new connection without automatic Start; Empty
accepts Refresh and never automatically creates a terminal. Pre-installation expiry preserves the
old canonical state. Post-installation failure preserves the committed candidate and cursor while
retiring truthfully. Stale results cannot install into another flight.

Runtime refinement uses the existing task owner, reconnect origin and shutdown fence. The private
state origin and UI attempt refine the model's eligibility and installation actions; the ordinary
Poll executor returns correlated evidence only through a normal exact join. The armed-attempt
marker refines the model's ordinary gate; the tick scheduler block, the observed-geometry Resize
admission, and the pane reconcile enqueue are its three guarded sites. The existing mailbox
seed, Snapshot flight, common membership transaction and setup executor retain their ownership.

Deterministic tests in `terminal_session/tests/retired_membership.rs` deliver the typed loss
only after a real drawn uncommitted panel-close confirmation is armed over the leased ordinary
Poll; together with `terminal_session/ui/tests/retired_membership.rs` they cover
Snapshot-first Empty and successful neighbor setup, parked explicit retry,
wrong-ID/transport/other-unavailability rejection, setup-Poll rejection,
retained conflicts, missing return, stale replies, cancellation during connect and Snapshot, original
45 s forward / 50 s total boundary checks, and candidate/cursor retention after setup refusal.
`mailbox/tests/tab_close.rs` checks that all five allocator cursors survive retirement, exhaustion
cannot wrap, and recovery starts without live work or bootstrap. Run
`cargo test --locked -p orchbee --bin orchbee retired_` on Linux.

The Linux PTY Close witness observes a new Disconnected frame before one explicit Refresh, then
both Clients' Empty state, full public Snapshot equality after exact removal, original shell exit
and a bounded 500 ms fresh-observation check against automatic Create. The same witness then
issues manual `c` from the former view-only Client, verifies fresh Tab/Panel/Presentation/Terminal
identities in the same Session/context and a real new shell, and refreshes the peer into view-only mode.
Ordinary exit preserves the new shell and original Server/Worker/Host processes. Later bare Open
returns the same canonical Snapshot, shell PID and shell-local marker. This does not change the
separate accepted ability of bare Open to bootstrap when the Session is still Empty. Retirement finalization
closes and joins the task synchronously before the tick returns for drawing. Run
`cargo test --locked -p orchbee --all-targets --all-features` with the staged local Server/Worker/Host
cohort. These checks do not establish remote-reference cleanup, restart adoption or general recovery.

## Context removal recovery

A workspace Close owned by another Client removes this session's current context from canonical
membership. The first visible fact is an ordinary Refresh reply whose validated navigation no
longer contains the current context id while the surviving standalone context still projects —
or, for an already retired session, the same shape inside one bounded recovery attempt. Snapshot
absence is membership knowledge, not cleanup evidence; nothing in this entry relaxes the real
Detach lifecycle.

Every candidate snapshot classifies before it commits. `Candidate` keeps the ordinary membership
path: the current context still projects. `Removed` requires the full conjunction — validated
envelope, unchanged Session, nondecreasing revision, the current context id absent from every
navigation entry, the surviving standalone projecting, the removed context's creation provenance
carrying no `Pending`/`OutcomeUnknown`, and every structural guard quiet — no pending mutation,
commit, setup, cleanup handoff, retired attempt, in-flight Create/Close or live
Read/Poll/Move ownership. Only `Removed` mints a
`ContextRemovalPermit{generation, removed, landing, revision}`; it lives as a bounded private
field inside the prepared candidate, never on ambient session state, and is spent exactly once
by the install it authorizes. `Divergent` — wrong site, regressed revision, malformed navigation
or the old context id still present elsewhere — and every guard failure retire truthfully
without touching canonical state.

With viewers alive the session never installs first: it stages the exact membership cleanup
under `ReconcilingMembership`, keeping the old cache, desired pane and mailbox epoch so the
executor detaches the removed epoch and nothing else. The workspace close published every member
`Disposed` before the deletion became visible, so each ordinary `Detach` receives `Detached`;
a stale worker releases its slot once and reports `Unavailable`; transport failure likewise
fails the cleanup. `MembershipDetached` then installs the pending candidate copy inside the
exact-cleanup transaction — old anchor records drop, desired commits to the landing pane or to
`Empty`, and the retained-context update skips reinserting the removed cache while preserving
every other context's `Pending`/`OutcomeUnknown` evidence entry. Any cleanup failure, expiry or
rejected install retires with no fabricated `Detached` proof, no retired origin and no partial
landing.

Without viewers the install gate already requires a clean pane set: `attached` empty and no
stray `projected`/`staged`/`accepted` record — the direct leg installs unchanged. A flagged
record without a viewer has nothing to `Detach` and must not land; it retires honestly.

`ReconcilingMembership` suppresses Ready-based input admission from staging until the commit
returns; no new gate or phase is added. An input admitted before staging carries its tagged
old-pane incarnation and settles only there; after the epoch commit drops old records the same
input misses its incarnation fence and is dropped — it is never re-tagged to the landing pane.

The retired entry is the existing `Ctrl+B w` recovery composition, not a new origin: only a
Ready-phase ordinary Poll that returned `TerminalRuntimeUnavailable` plus a clean old-task join
mints origin evidence. Its bounded attempt — one connection, one refresh flight, its own sampled
deadline — runs the same classify; a `Removed` result mints a generation-2 permit and installs
through the same candidate transaction with panes already empty, while divergence, expiry or
cancellation parks retired. Cleanup-failure retirement mints no origin: external release came
from the old session and worker actually settling, not from clearing records.

One `needs_recovery_deadlines` sample minted at reply deposit covers staging, cleanup and
install; it is stored on the cleanup record and never renewed or resampled. The retired attempt
carries its own separate bounded window.

Runtime refinement uses the existing refresh reply deposit, membership cleanup, executor Detach
and candidate install paths unchanged. The classifier refines the model's delivery actions
through `prepare_context_removal` inside refresh reply deposit; the permit refines to the
bounded private `context_removal` field on `PreparedMembershipCandidate`, copied by
`prepare_cleaned_membership_candidate`; `validate_candidate_provenance` gains the permit's
authorized third branch; `prepare_retained_contexts` skips exactly `permit.removed`; the staged
cleanup reuses `stage_refresh_membership_cleanup` and `detach_anchor_panes` unchanged; the
install gates stay the existing exact-cleanup and four-flag rules. Deterministic session-state
tests cover each classified path and the retired-entry UI tests cover the bounded attempt; the
composed CLI-to-TUI journey belongs to the workspace-close integration record.

## Automatic Tab naming

The [Tab label contract](native-tab-labels.md) refines the label facts above: projection retains raw text and naming mode, and display derives from context position. Create checks the requested mode at its commit; reordering preserves metadata. Rename clears automatic mode, so an equal-text automatic rename is a commit and only a fixed equal-text rename is a no-op. The strip, rename draft, close confirmation and Agent board use the effective name at the captured position.

## Native grouped and pinned Tab projection

The selected-context projection retains every canonical group record, including unused groups,
and each Tab's pinned flag and resolved group metadata. Bounds, unique group IDs, valid labels,
existing group references, ungrouped pinned prefix and contiguous group membership are required;
invalid metadata refuses the complete selected projection without filtering terminals. Nonactive
context trees keep their independent admission boundary. Child Presentation navigation is refined below;
unbound selection is refined below, and children are not flattened into layout leaves.

Same-revision refresh and return to a retained context compare the complete group metadata. A
higher canonical revision may update group/pin chrome while preserving unchanged Panel/binding
identities, viewer/controller ownership, frame and history. Exact Create/Move/Split/PanelMove/Rename
receipts also preserve unrelated group/pin metadata; a new Tab starts ungrouped and unpinned.
Workbench owns canonical metadata, Native Session owns selection and attachments, and rendering
only derives labels. No group metadata change alone starts or disposes a terminal attachment.

The compact strip and navigator annotate pinned Tabs with localized `Pinned` / `固定` and grouped
Tabs with their group label. Raw Tab labels, automatic ordinals and stable-ID pointer targets stay
unchanged. Existing terminal-text sanitization, cell-width clipping and semantic theme roles apply
to the complete annotation. Group color is retained canonically, without inventing an ANSI color
meaning in chrome. Empty groups do not create a selectable terminal row.

A definite `tab-move-order-conflict` retains selection and attachment and shows a localized notice
explaining that the move would break pin/group order. It settles the pending move through the
existing owner, without mutation retry or retirement. Evidence covers metadata validation, exact
receipt tampering, same-revision refusal, higher-revision viewer-preserving refresh, actual native
wire startup/context selection, clipping and pointer geometry. No new wire schema, dependency or
persistence is added. Matched-source rollout/rollback leaves canonical data unchanged; actual
Windows multi-Client visual acceptance and group/pin mutation UI remain separate obligations.

## Child Presentation navigation

A Panel remains one layout leaf and retains primary-first Presentation order, with at most sixteen
children. Presentation and bound Terminal identities are unique across the context; sharing their
own Panel is valid. Retained Presentation identities preserve Panel, label and binding, while the
existing Workspace primary replacement rule stays separate from child addition/removal. Exact
structural receipts preserve complete surviving member payloads.

Native Session owns a volatile Presentation preference per Panel. Live PaneRecord binding determines
the displayed member; departure records stable choices, and missing choices fall back to primary.
A local choice never edits Workbench revision or layout. Switching member uses a fresh anchor and
the existing Switch cleanup/attach owner, so old input, frames and controller grants cannot retarget.
Pre-cleanup refusal restores the previous live member; unavailable attach remains an explicit state.
Sibling viewers may be reattached by this initial complete-anchor cleanup. Child attachment uses
binding-only provenance and cannot inherit a primary terminal creation outcome.

`Ctrl+B (` / `Ctrl+B )` select previous/next Presentation, wrap in canonical order, and are no-ops with one member. The shared help registry provides both localized descriptions. Stable
Presentation targeting serves Agent focus. Rendered label/count and attention refer to the
chosen binding; Panel movement and Close still operate on the complete Panel. The report lane queries
the selected Presentation and correlates its owning Panel and binding under both revision fences.
A local member change discards the old observation before publishing the child's report or metadata. Header arrows provide exact member selection through the [native pointer owner](native-mouse-input.md#presentation-selector). [CLI member creation](windows-panel-terminals.md) and unbound selection below provide empty-slot integration; exact public member focus, individual removal and final multi-Client
visual acceptance remain open. Focused tests cover full member identity, rollback, stale frames,
remembered selection, canonical removal, bilingual shortcut decoding, rendered child label/count and
actual native wire cleanup-before-attach; final full regression remains deferred under the implementation plan.

## Unbound Presentation selection

A canonical Presentation may have no binding. Native projection retains its identity, label,
Panel membership and order; it never borrows another member's terminal or drops an empty leaf.
Terminal identity uniqueness applies only to actual bindings. At a higher revision, an unchanged
unbound Presentation may acquire a fresh terminal binding; existing bound identity and same-revision
guards remain strict. The Server remains the only canonical binding writer.

Session selection retains the exact Presentation ID independently of attachment. An empty selected
member uses `SelectedWithoutAttachment`, renders localized empty-terminal guidance and cannot send
terminal input, Attach, TakeControl or Resize. It is still a valid navigation target. Bound siblings
keep their ordinary per-pane reconciliation and polling. Selection never launches a shell.
When no viewers exist, a one-second UI cadence admits the existing bounded Refresh operation;
in-flight or busy refreshes are not duplicated. Later binding therefore does not depend on a
nonexistent terminal Poll. Tab/Panel Close confirmations capture optional binding facts and still
close the named canonical structure, with the existing display and commit fences.
Cleanup validates live sibling viewers under the exact old anchor even when the selected member
has no viewer. A typed Closed result retains its exact operation, Tab, generation and optional
selected binding before authorizing disappearance. Pre-cleanup refusal from an empty selection
restores only retained sibling projections with matching incarnation, binding, controller, geometry
and accepted frame; missing or changed evidence cannot revive a viewer.

| Input                                     | Transition and effect                                                                                                                                                   |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Startup with an unbound primary           | Select its structural identity without a PaneRecord or viewer; enter the ordinary command loop.                                                                         |
| Choose an unbound Panel/member            | Admit the usual fresh Switch marker with exact Presentation ID and optional binding; validate the snapshot, finish old-anchor cleanup, then commit the empty selection. |
| Pre-cleanup refusal or stale Switch       | Restore/retain the previous selection and live viewers through the existing Switch owner.                                                                               |
| Higher revision binds the selected member | Validate fresh binding identity, allocate a new attachment epoch through membership repair, and attach only that real binding.                                          |
| Member disappears                         | Use the existing primary/Panel fallback, never retain an orphan ID or send input to the previous member.                                                                |

The projection, Session and native wire tests cover empty/mixed structures, exact empty-child
selection, later binding, cleanup and absence of invented attachment effects. Render tests cover
the empty state and ordinary member controls. They do not establish real provider or multi-Client
visual acceptance; those remain final Windows validation obligations. No new wire contract,
persistence or physical process owner is introduced.
