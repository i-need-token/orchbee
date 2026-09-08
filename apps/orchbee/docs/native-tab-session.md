# Native Tab Session

Status: accepted implementation and correctness contract for the native Client Tab Session.
Native Move runtime has deterministic integration and Linux cohort process evidence. This remains
an unpublished local development path; Close, packaging and production recovery are separate work.

## Accepted Close refinement, implementation pending

[Canonical terminal Tab Close](../../orchbee-server/docs/terminal-tab-close.md) owns the accepted
Server/Worker lifecycle, wire/status/Resume and native removal contract. Native `Ctrl+B x` targets
the active standalone Tab and confirms in both locales that execution ends for all attached Clients;
an independent lifecycle grant is required. Closed requires known disposal, target-owner cleanup
and canonical removal. The last removal leaves a usable Empty Workbench with Refresh/manual `c`,
without a replacement shell or Client exit.

The record specifies active/inactive removal, old-ordinal survivor selection, Empty Create origin,
known Close retention and exact/later CreateSnapshot, Switch, Move and Refresh settlement. A later
snapshot may remove IDs while preserving surviving immutable identities; an exact Create/Move
commit revision retains its stricter expectation. These are accepted **pending refinements** to the
implemented Create/Move cohort below, whose current replacement rules reject removal and whose
model/process evidence does not establish Close. Native removal/barrier models must precede the
dependent implementation; exact UI consumption, cleanup and worker-exit facts remain mandatory.

## Close control and recovery contract

The native state owner retains Close knowledge independently of terminal selection and projection.
Linux native input, confirmation drawing, Close/Read/Resume execution and explicit status-only
recovery are integrated. Closed membership installation and record pruning remain pending their
common consumers. The protocol adapter validates wire correlation; the native owner validates exact
operation identity, nonregressing attempt generation, retained disposal/Closed facts and UI flight
identity. These checks do not establish the complete multi-Client Close-to-Empty result.

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
| Initial Close                 | Capture the exact selected Site/context/Tab and canonical base. Render the localized all-attached-Clients warning before arming confirmation; coalesced unseen confirmation cannot submit.                        |
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
Site/target. Mismatch is unavailable, with no adoption. This status-only phase cannot activate a
Site, Open, Create, Start, Attach or replay a mutation. Retain known facts across join and
reconnection; never revive the old Viewer. Reconciliation/Attach can begin only through the
membership owner's validated Closed/Snapshot handoff outside this phase. Restart persistence
and changed-authority recovery remain excluded.

After a successful exact Read on the fresh carrier has semantically settled and displayed the
latest eligible parked status for the same retained authority/Site/target/operation, a new user
Resume intent may leave status-only recovery for one explicit control. Its new confirmation must
actually render, and admission revalidates the status, generation and display token. Failed Read,
Unknown, NotClosing, an old token or unseen confirmation cannot open this control stage. Admit one
fresh Resume request and fixed exchange budget; retain the Server lifecycle-grant check. No old
mutation is replayed and no activation, Open, Create, Start, Attach or Viewer restoration occurs.
A lost reply retires this carrier again and retains the new identity. Exact result settlement
returns to status observation; another Resume needs a latest eligible status and fresh drawn
confirmation. This transition does not grant general session or changed-authority recovery.

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
`ui::membership_tests` additionally checks the acknowledgement-only redraw and Snapshot placement
at time zero with UI consumption at 75 seconds, followed by Gone at 0, 44.999, 45, 49.999 and
50 seconds of the one newly captured reconciliation budget. The three Unix-socket traces in
`tests::close_recovery::close_membership` cover live Close → Closed Read → fresh Snapshot → Gone →
Empty, drawn status-only Closed plus a new explicit `w` → Empty, and Denied Snapshot plus manual
fresh-ID retry → Empty. These witnesses do not establish peer recovery, Server/Host process
termination, surviving-neighbor attachment, manual Create from Empty or other operating systems.

The finite control evidence is NativeClientCloseControl with command/status and
Resume/status configurations, plus NativeClientCloseHandoff for two legal targets. The control
cut has one target and one outstanding exchange, canonical/alias operation representatives,
attempts 1..3, at most one Close, two Resume controls and bounded Read admissions. The
command configuration allows two Reads and no Resume; the Resume configuration allows three Reads
and two Resumes. Both explore at most two opened confirmation intents and one explicit reconnect.
Symbolic exchange time is before/cutoff/transport-budget-ended; the disposed quota uses 0..2.
The handoff cut has two legal
targets, one late worker acknowledgment, one possible later flight, bounded manual target intents
and one Read slot; it contains no status-body, Resume, confirmation or clock product.
Neither cut models Host cleanup, tab permutations or attachment setup.

Required refinement includes actual eight-slot causal mailbox storage, the 256-record boundary,
32-read counts 31/32/33, checked generation/identity limits, same-generation nonrenewal through
failures and join, raw-key/draw confirmation in both locales, and A-settled/B-admitted/late-A traces.
Model fairness covers only local expiry, lock release, exact UI consumption and completed-worker
join; no network, lifecycle, reconnect or OS-success fairness is assumed. The composed Linux
witness must close a shell and resisting descendant from an authorized read-only Viewer while
another terminal survives, show removal to two Clients, then reach Empty and manual fresh Create.
These obligations do not authorize the manual focus-stealing native-window test.

### Control and handoff model boundary

The control model retains exact current confirmation/submission keys through pending work,
retirement and join. Older completed key sets are unnecessary: every new admission strictly
advances the cursor, submission leaves queued, and the same key cannot re-enter queued.
Canonical operation knowledge is separate from an acceptance receipt; an available Read after
lost acceptance can establish the operation without inventing its accepted revision.
Timely typed deposit maps to the ended-budget clock class, which includes arbitrarily late UI
consumption. It does not start another deadline. Earlier causal time remains independent.

| Native obligation                                              | Finite assertions                                                                                                                                                      | Required concrete refinement                                                                                                    |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Actual current drawing before Close/Resume                     | `ConfirmedAdmissionRequiresDraw`, `ResumePromptRequiresDisplayedStatus`, `GeometryChangeRevokesDrawBeforeInput`, `RecoveryResumeRequiresFreshReadAndDraw`              | Real successful draw receipts, same-packet unseen confirmation, resize-plus-confirm dispatch order, both locales.               |
| One exact mutation submission and retained canonical facts     | `NoUnconfirmedMutation`, `NoMutationReplay`, `EachSubmissionIsNew`, `SameKeyNeverReentersQueue`, `OnlyCommandResponseEstablishesReceipt`, `KnownReceiptNeverRegresses` | Byte adapter correlation, checked cursor/generation boundaries, exact mailbox key and ambiguous worker-exit handoff.            |
| Fixed exchange budgets and untimed semantic consumption        | `ExactResultFence`, `PendingUiIsUntimed`, `TransportAgeNeverRenewsForSameKey`, `EarlierDeadlineNeverRenews`, `PendingEventuallySettlesOrRetires`                       | Actual eight-slot causal reservation, cutoff equality, lock contention, late UI consumption and unchanged Create/Switch clocks. |
| Bounded disposed observation without invented lifecycle status | `QuotaChargeIsAdmission`, `SameAttemptDoesNotRenewQuota`, `HigherAttemptAloneRenewsQuota`, `ReadAdmissionConsumesInterval`                                             | Actual 31/32/33 counts and 250 ms global admission interval; failure/manual/join nonrenewal.                                    |
| Two legal targets, retained addresses and guarded pruning      | `ACompletionCannotReplaceB`, `ReadCompletionPreservesOtherTarget`, `ManualPassIncludesRemovedUnsettledTargets`, `PruningRequiresAllFences`                             | A settled/B admitted/late A mailbox and join traces; 256-record refusal and membership-consumed pruning.                        |

The control model begins at validated typed adapter results. Its rejected stale/foreign event
actions are defensive no-ops or carrier retirement; they do not model a second concrete packet
store or prove byte filtering. The separate handoff model contains two legal retained targets,
fact ranks for acceptance/disposal/Closed, an exact later flight and old worker reference. Its
manual intents and one round-robin slot preserve removed-target addressability without combining
status bodies, Resume generations, confirmations or clocks with that two-target graph.
Neither model establishes Server/Host cleanup, attachment setup or eventual Close success.

`AbsentStatusDrawUnarmsResume` checks missing status independently of modal rendering.
`JoinedCarrierHasCurrentReturn`, `WorkerReturnHasCurrentSource` and
`FreshCarrierCannotBorrowOldReturn` bind join to the actual current worker identity across the
one reconnect; retirement retains completed current-worker evidence.
`SubmittedRetirementRetainsRecovery` and `NoInventedSubmissionUncertainty` distinguish unresolved
handed-off mutations from queued-only cancellation and definite typed results. Required coverage
includes cutoff and explicit retirement after submission, second-carrier retirement before return,
rejection of the old worker completion and join after the current recovered worker returns.

The registered three-configuration run on 2026-09-08 passed safety, temporal checks and every
required action-coverage marker with pinned TLC 1.7.4, one worker per configuration, `--jobs 1`
and the unchanged 180-second limit. The module-plus-effective-config SHA-256 values are:

| Configuration                                | Generated / distinct / depth | SHA-256                                                            |
| -------------------------------------------- | ---------------------------- | ------------------------------------------------------------------ |
| `native-client-close-control/command-status` | 623,726 / 121,398 / 29       | `9dbfcc7b02378d2d41b7024eabf85ba8a9c6c22aa9523f51308ec3ab591639f5` |
| `native-client-close-control/resume-status`  | 3,215,267 / 574,031 / 40     | `41eaed7818966f5bf599e48b979b27468f7c9baa8c50bdb1fea5871af240fa59` |
| `native-client-close-handoff/default`        | 257,104 / 39,662 / 24        | `60fbfb6b8f4cec742c114ef6fc1de0e03fb1a2cce8b1b191655ab4428ed4e52e` |

```sh
node scripts/tla/run-models.mjs --models native-client-close-control,native-client-close-handoff --jobs 1
```

This is finite model evidence. The native Close runtime and composed process refinements above
remain pending; earlier timeout or diagnostic runs are not substitutes for this registered run.

## Canonical membership implementation boundary

The shared membership implementation replaces the earlier append-only assumption below. Pure
validation requires the same Site/context, nondecreasing revision and exact equality at the same
revision. A higher revision may add, remove, reorder and rename Tab labels; each surviving Tab
preserves its Panel/Presentation identities, Presentation label and Worker/terminal binding.
Within this Site/context, overlap of any current and candidate Tab, Panel, Presentation or Terminal
identity requires the same complete immutable member tuple. Changing only the Tab ID cannot disguise
identity reassignment as removal plus addition. Worker ID alone is not a member identity: one Worker
may own several terminals. Genuine removal plus entirely fresh identities is allowed. This compares
two snapshots only and adds no registry of every previously removed identity.
Exact Create and Move commit revisions retain their original strict expectations. At a later
revision the committed target may be absent, represented by an optional validated target index;
its historical commit is never erased. A projection-owned `ClosedTabExpectation` checks exact
Site/context/Tab and minimum Closed revision with the target absent. It proves membership only.
This expectation is implemented in `workbench_projection/closed.rs` with private identity fields
and read-only accessors. Its pure validator reuses canonical candidate validation, then checks exact
scope, minimum revision and target absence; tests reject wrong scope, old revision and target
presence. The Close state owner separately supplies the retained attachment identity and cleanup
proof when connecting its consumer.

Live Empty retains the validated canonical Site/context/revision and an empty Tab vector, with no
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
installs. Manual Create from Empty and the remaining Create/Move/Switch/Close origin consumers
remain separate incremental work. This Refresh slice does not construct or consume a Close Gone
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

The first concrete projector slice is `workbench_projection.rs`: canonical refresh accepts higher
revision removal, and `ValidatedCreatedProjection` / `ValidatedMovedProjection` retain the validated
candidate with an optional target index. Existing Create consumers still require that index until
the common membership continuation is connected. This slice does not claim attachment cleanup or
Empty installation. Four deterministic membership regressions cover removal at 0/1/255/256 members,
later missing Create/Move targets, each retained structural identity domain, and a genuinely fresh
terminal sharing the same Worker. The generated label history now admits removal, allocates fresh
inserted identities independently of current length, and compares an independent per-domain identity
map across 64 histories of 38 steps. Its scope is adjacent projections, not lifetime ID reuse.

`NativeClientMembershipRemoval.tla/.cfg` owns the finite removal composition across Refresh,
correlated Create projection, Move result/recovery, Switch cleanup and own known Closed. The five
origins use nineteen configurations partitioned by initial Ready controlling, Ready read-only,
selected without attachment and retryable state; Create starts only in the two Ready modes, and
Refresh also starts in Empty. The partition changes neither transitions nor the union of supported
initial states. Each configuration has one TLC worker and a 180-second limit, with no state constraint.

The Tab names are explicit role representatives, not a TLC symmetry reduction. A is initially
active. Refresh, Move and own Close use `[A]`, `[A,B]`, `[B,A]`, `[A,B,C]`, `[B,A,C]` and
`[B,C,A]`; Refresh additionally uses `[]`. Switch uses active A, desired B, both two-member orders
and all six three-member orders. Its lease may be outgoing A/generation 0 or already attached
B/generation 1; replacement setup uses generation 2. Create uses `[A,B]` and `[B,A]`, active A,
and new C. These cases cover empty/singleton and all active ordinals, not arbitrary permutations.

The membership slice records C's already reserved generation as 1 when retaining its known commit.
Surviving C keeps 1; a fallback consumes the checked successor 2. Other new selections consume the
checked successor of their initial cursor, and Empty preserves the consumed cursor without a key.
Installation publishes the selected Tab and generation atomically after exact cleanup; Attach uses
that key without another allocation. The single recovery precedes installation and preserves the
reservation. Concrete admission refusal and `u64` exhaustion require the identity/mailbox witnesses.

The model admits one structural flight and one explicit recovery, keys 1..2, generations 0..2,
one membership-removal publication, and one peer Create observed by an initially Empty Refresh.
Canonical revisions use 1..4; an untrusted relative predecessor 0 exercises lower-revision rejection.
Same-revision and exact Create/Move-commit candidates remain exact. Later removal keeps historical
commit facts and surviving provenance. Immutable tuple/binding/Presentation-label validation is an
opaque valid/invalid boundary; concrete values and 0..256-member histories require projector evidence.
No retained registry of all peer-removed IDs is modeled. Local selection/restoration follows the
accepted cache; own known Closed additionally fences its target by the retained commit revision.

A removal-bearing candidate waits behind the exact historical attachment before replacing cache or
selection. `CompleteAlreadyGoneCleanup` represents a distinct, exact-target definitive cleanup
observation; snapshot absence alone is not its implementation witness. Inactive removal preserves
Viewer/controller/generation and historical Ready. Active/desired removal chooses the survivor at
the historical ordinal, otherwise the last survivor, otherwise Empty. Empty Refresh observes peer
Create through fresh Attach/retry, without a native Create or Start. Fresh Ready is an abstract
completed draw/feedback proof; full geometry, lease and first-frame ordering stay with the existing
Switch model and deterministic adapter traces.

Time is symbolic 0..8. Transport admission captures its own deadline; timely typed placement ends
that budget and UI consumption has no synthetic timeout. Existing Create/Switch forward and total
budgets remain unchanged. A new post-transport reconciliation captures forward `+2` and final cleanup
`+1` once at validated-candidate staging, before old cleanup; these encode the accepted 45s forward
and 50s total ordering, not 50s plus another 5s. Transport, forward and reserve expiry have explicit
pending-phase/deadline guards. Weak fairness applies only to advancing this finite clock, enabled
expiry and exact UI consumption. The checked progress claim is settlement, explicit unavailable
recovery or retirement; successful Snapshot/Attach, successful Close, OS scheduling and real elapsed
time have no fairness or timing theorem. Known Close remains retained through native Client worker
thread exit and UI retirement; the one abstract nonzero attempt is not a wire-generation proof.

Defensive stale input is projected directly to stuttering while a live UI fence is held. The
`StaleResultCannotSettleCurrentFlight` property declares that refinement boundary; it is not
independent evidence for buffer storage or transport filtering. `NativeClientRefreshFence` and
concrete byte/mailbox tests own those mechanisms. Repeated stale observations remain admitted.

`NativeClientCreateBarrier` separately refines one manual Create from Ready or typed Empty, with
queue capacity two and geometry values 0..1. Admission captures origin/geometry, Empty refusal
restores Empty, and only Started permits new attachment setup. After a retained commit, it validates
and stages the exact or later canonical candidate before any old-viewer cleanup. Ready then requires
the exact Detach result or an opaque exact-target known-Closed proof before the candidate is installed;
Empty has no imaginary old Detach. The model includes one later removal of the returned member:
the retained Create fact and generation 1 remain historical, while the frozen ordinal chooses the
surviving fallback at generation 2, or leaves Empty. It preserves all earlier Ready
FIFO/drain/failure/cleanup actions. It does not prove repeated Create reentry, arbitrary removal, or
ProjectionUnavailable recovery. Those removal/recovery combinations remain owned by
`NativeClientMembershipRemoval`. The unchanged RefreshFence and MoveBarrier models remain conditional
no-removal slices of their own timing/FIFO/setup behavior; their old preservation properties do not
establish removal. TabSwitch separately checks the candidate, exact-cleanup and successor-install
boundary described below. The membership family exposes these cleanup/result boundaries instead
of combining all their internal queues and clocks into one product.

| Claim / model boundary                                                       | Required runtime refinement and executable witness                                                                                                                              | Formal properties                                                                                                                                                                         |
| ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exact/later revision, immutable survivors and historical Create/Move results | Strict projector corpus plus independent identity-keyed generated create/rename/move/remove histories, with shrinking/replay and 0/1/255/256 boundaries.                        | `SameRevisionIsExact`, `SurvivingFactsAreImmutable`, `HistoricalCommitNeverForgotten`, `KnownClosedAbsentAtCommitOrLater`                                                                 |
| Frozen old lease, ordinal selection, no restoration of absent IDs            | Deterministic state and NativeUi traces for inactive/active/desired removal in every barrier, exact gone cleanup, ambiguity, selected-unavailable and Empty.                    | `CandidateWaitsForExactCleanup`, `NoReplacementSelectionBeforeCleanup`, `NeighborUsesHistoricalOrdinal`, `NoRemovedIdResurrection`, `InactiveRemovalPreservesExactAttachment`             |
| Known Closed/attempt, exact UI result and retirement handoff                 | Native worker/mailbox/UI traces at status receipt, snapshot failure, typed placement, delayed consumption, stale key, cleanup and worker-exit cuts.                             | `KnownClosedNeverForgotten`, `WorkerExitPreservesKnownClosed`, `PendingResultFencesExecutor`, `StaleResultCannotSettleCurrentFlight`                                                      |
| Empty manual Create and Empty peer-Create Refresh                            | Captured geometry and typed-origin state tests; fake carrier asserts zero imaginary Detach, zero duplicate Open/Create/Start and correct typed terminal-outcome attachment.     | `EmptyHasNoAttachmentOrReady`, `NoAutomaticCreateOrStart`, `EmptyHasNoFabricatedReadyOrAttachment`, `EmptyOriginNeverDetachesImaginaryViewer`, `CapturedGeometryIsImmutableDuringBarrier` |
| Fresh attachment proof and integrated observable result                      | Existing setup refinement plus controlled multi-Client process traces for active/inactive Close, last Close → Empty → manual Create, and historical Create/Move target removal. | `AtMostOneViewerAndController`, `FreshSelectionUsesFreshKey`, `FreshReadyRequiresProof`, `CreatedReadyRequiresDrawAndFeedback`                                                            |

`TypeOK` supplies the structural state checks. `ExistingSetupBudgetNeverRenews`,
`ReconciliationEventuallySettles` and Create's `BarrierEventuallySettles` map to deterministic
clock traces at the 45s/50s boundaries, checked-add refusal, silent I/O, cleanup reserve and delayed
UI consumption. Their fairness assumptions do not establish successful external completion.

These artifacts precede dependent native runtime changes. Their bounded checks do not establish
runtime refinement, byte/JSON validation, real PTY behavior, restart adoption, remote transport or
whole Close delivery; the executable runtime and process witnesses above remain required.

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
recovery. Starting, Creating, CreateCommittedNeedDetach, AwaitingCommittedProjection, Switching and
Retired prohibit ordinary replacement. Correlated CreateSnapshot from AwaitingCommittedProjection,
after its retained-commit handshake and before old cleanup, remains allowed. Explicit Create ProjectionUnavailable recovery
validates retained create facts: Started gets its existing fresh setup deadline, non-Started needs
none. Cold Bootstrap owns initial installation. These exceptions never admit a second structural
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
Other live Clients update only via explicit `Ctrl+B w`, with no push or terminal switch.

The Move membership consumer implements the common cleanup and installation refinement. `NativeClientMoveBarrier` continues
to own the no-removal transport/FIFO/commit-preservation slice; the checked move origin of
`NativeClientMembershipRemoval` owns later active-target removal, exact cleanup, fallback and Empty.
Its deterministic UI/carrier evidence is separate from the final combined native integration.
Exact committed revision still requires the full expected permutation; only a later
revision may remove the moved target. Preserve immutable survivor identities and creator provenance,
then choose the common historical-ordinal neighbor or Empty. Raw absence never proves Gone.

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
`ui/move_membership_tests.rs` covers delayed typed-result consumption, exact old cleanup, 45/50-second
boundaries, wrong generation, direct no-viewer installation, and ordinary refusal recovery.
`connection/native/tests/tab_move.rs` drives real carrier Move/Snapshot/Detach through Empty or a
successor's Attach/Resize/Poll/frame-feedback/Ready; a following Snapshot is the FIFO witness that
old Input and Resize emitted no request. It also checks still-owned pre-Switch Input/Resize and Snapshot-only committed
recovery. Run `cargo test --locked -p orchbee --bin orchbee terminal_session::` for these state and
consumer checks. Full native binary tests also require the existing Server, Worker and terminal-host
process binaries. These checks do not construct a Gone proof or establish complete Close integration.
Strict Clippy still reports the independent staged Close/Rename/Switch interfaces until their
consumers are integrated; these warnings are not suppressed or counted as a passing lint gate.

Required owner-model extensions are ID/provenance remapping and structural flight/recovery in
`NativeClientRefreshFence`, created-member moved before correlated projection in
`NativeClientCreateBarrier`, and candidate-staged exact cleanup with fresh successor installation
in `NativeTabSwitch`. Preserve existing
action obligations. Deterministic tests additionally own strict 0..256 identity/shape and exact
same/create/Move revision checks, reused created ID before overlay, immutable tuple/binding/
Presentation-label refusal, all three stable phases, generated create/rename/move histories with
shrinking/replay, historical Ready, stale UI, admission races and recovery. Fake-clock/carrier tests
own checked-add overflow, deadline minus one/equality/plus one, lock contention, prior deadline,
shared budget, initial and postcommit ID exhaustion, timed deposit and UI consumption after expiry.
Controlled PTY tests own active PID/marker continuity, usable input/Resize and second-Client
refresh; worker/UI traces own exact viewer/controller/generation preservation. Internal
retirement-to-completion is Site/stateful evidence only. No JSON, real-time,
unbounded permutation, OS/PTY or recovery-success claim follows from disposable narrowing TLC.

## Projection refinement evidence

The strict projector now accepts higher-revision create/rename/reorder histories by immutable Tab
identity. `workbench_projection/tests/reorder.rs` checks every cache length from 0 through 256,
selected reverse/rotation histories, structural-ID uniqueness, immutable-member refusal and
created-member lookup after later moves. `label_evolution.rs` compares generated histories with an
independent sorted-membership oracle. Exact commit-prefix and same-revision equality checks remain
required. These checks establish the pure projection boundary; Move command runtime integration
and cohort process acceptance have separate evidence below.

The checked `NativeClientCreateBarrier` and `NativeClientRefreshFence` refinements model the created
member moving before projection and ID/provenance/index remapping. Create uses two-to-three-member
histories; Refresh enumerates one-to-three-member caches. Both use selected permutations, with
abstract provenance and validity.
They do not prove concrete 256-member permutations, schema validation, immutable labels/bindings,
wire correlation or OS/PTY behavior. `NativeTabSwitch` retains its Tab-ID/generation cleanup
abstraction and has no cache vector or ordinal; concrete cache replacement needs state evidence.

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

This record fixes the native Client consumer of the canonical OBWP `tab.create` and `tab.move`
contracts and the `Ctrl+B c`, `<`, `>`, `n`, `p`, `1` through `9`, and `w` actions. It preserves the existing one
sequential admitted Client carrier, one OBCS viewer, one combined input-and-resize controller at
most, bounded command/event storage, and TUI restoration boundary.

The Server-owned Workbench snapshot remains the only source of Tab order, labels, Panel and
Presentation membership, and terminal bindings. One native invocation owns only its cached strict
projection, local active Tab ID, attachment generation, navigator state, geometry observation,
terminal projection, and transient statuses. Local selection never writes or advances a Workbench
revision.

Terminal close/removal, native rename interaction, pin/group, Split or child Presentation support, Workspace
selection, persistence, push/delta subscriptions, remote transport, reconnect, hidden carriers,
carrier multiplexing, and multi-viewer composition inside one Client remain excluded.

## Pure strict projection

Replace `select_standalone_binding` with this Unix-only pure projection in
`terminal_session/workbench_projection.rs`:

```rust
struct NativeStandaloneTabs {
    site_id: SiteId,
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tabs: Vec<NativeSimpleTab>,
}

struct NativeSimpleTab {
    tab_id: WorkbenchTabId,
    tab_label: WorkbenchLabel,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    presentation_label: WorkbenchLabel,
    binding: NativeTerminalBinding,
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
}

fn project_native_standalone_tabs(
    snapshot: &Snapshot,
) -> Result<NativeStandaloneTabs, NativeProjectionError>;
```

The projector requires exactly one Terminal Context whose ID equals
`navigation.standaloneTerminalContextId`. Other Project/Workspace contexts may coexist and are
ignored. The standalone context must have no groups and may contain exactly 0 through 256 ordered
Tabs. Every Tab must be ungrouped and unpinned, have one Panel root, have no child Presentations,
and have one bound primary Presentation. One unsupported member rejects the whole projection; the
projector never filters, reorders, or chooses the first attachable member. It retains both labels
exactly, including duplicates and whitespace.

Two additional pure validators own cache replacement:

```rust
fn validate_append_only_refresh(
    current: &NativeStandaloneTabs,
    candidate: NativeStandaloneTabs,
) -> Result<NativeStandaloneTabs, NativeProjectionDivergence>;

fn validate_created_tab(
    current: &NativeStandaloneTabs,
    expected: &CreatedTabExpectation,
    candidate: NativeStandaloneTabs,
) -> Result<(NativeStandaloneTabs, usize), NativeProjectionDivergence>;
```

The accepted Move refinement replaces the legacy `validate_append_only_refresh` prefix policy:
require the same Site/context and reject lower revision; same revision requires exact equality of
the complete projection, including order and labels. At greater revision, each old Tab ID must occur
exactly once anywhere with exact Panel/Presentation identities, shape, binding, terminal-state
constraints and `Presentation.label`; only `Tab.label` and order may differ. New members pass the
complete schema/strict projector and structural identity uniqueness. Removal, duplicate ID or any
other immutable-member change is divergence. At most 256 Tabs need only exact scans, no stored map.

`CreatedTabExpectation` is projection-owned and carries only the six canonical facts needed for
pure validation. The session state embeds or constructs it; request identity, terminal provenance,
attachment generation, and mailbox types never enter the projector. `validate_created_tab`
retains the original pre-create baseline through projection recovery. The returned created ID must
be absent from that baseline before provenance overlay or attachment allocation. Its committed
revision must equal the checked successor of the admitted baseline, and candidate revision must be
at least that commit. Locate the returned Tab exactly once anywhere; context/Tab/Panel/Presentation
IDs equal the result and `Presentation.label` equals the requested creation label. At exactly the
commit revision, require one new member, the entire baseline as an exact unchanged prefix, and the
created member appended with both labels equal to the requested label. Above it, use the shared
strict higher-revision rule: old/created Tab labels and order may reflect later rename/move and
strict later members are allowed. No old-length index assumption applies. The result has no binding;
only this validated correlated snapshot supplies it, and any earlier retained binding for that ID
must remain exact. Prepare provenance by ID, overlay only after all checks, then atomically install.
Known creation provenance remains historical after Worker exit. Only this snapshot-derived binding
may reach `AttachBinding`.

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

## Startup

Cold startup remains exactly:

1. measure complete real terminal geometry;
2. submit one `terminal.open` ensure/join and wait for its final `Opened` result;
3. submit one `SnapshotRead` on the same carrier;
4. validate the full `NativeStandaloneTabs` projection and require `tabs[0]`;
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

## Client state and bounded mailbox

`TerminalSession` owns the Unix-only typed state. `client_view.rs` stays unconditional and contains
only protocol-free key/overlay/notice decisions; no Workbench, OBCS, Ratatui, Unix, or TTY type is
added to it. The renderer and session remain `cfg(unix)`. Cargo protocol dependencies stay under
`target.'cfg(unix)'.dependencies`, preserving Windows help, version, usage, and unavailable-TUI
compilation without expanding its dependency graph.

The session state is an explicit sum. The names are implementation guidance, not a second wire
contract:

```rust
enum SessionPhase {
    Starting,
    Ready(ReadyAttachment),
    Creating(CreatingBarrier),
    CreateCommittedNeedDetach(CommittedTabCreate),
    AwaitingCommittedProjection(CommittedTabCreate),
    ProjectionUnavailable(CommittedTabCreate),
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
    geometry: WorkbenchTerminalGeometry,
}

struct RefreshMarker { refresh_generation: u64 }
```

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
when terminal Resize admission is paused during Creating or Switching. This observation changes
Client-local layout data only; it does not authorize an OBCS Resize. The session separately tracks
last admitted/applied controller geometry. A Create marker captures the observed geometry
at its atomic admission; a later successful Acquire begins with a Resize carrying the then-latest
measured geometry and uses the setup catch-up rule below for subsequent observations. A precommit refusal resumes the old controller and, if measurements changed,
admits one catch-up Resize before polling resumes. Read-only viewers never send Resize.

## Local selection

Selection resolves only against the latest applied strict cache. `n` and `p` use the local active
target's index and wrap modulo the Tab count in both directions. `1` through `9` are one-based exact
indices; an absent index is a local no-op with an out-of-range notice. Zero Tabs have no selectable
target.

| Current phase and resolved target                                                                          | Exact result                                                                                                                                                                                                                                                                                                        |
| ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Any otherwise selectable phase, target differs, and one Refresh is queued/in flight/awaiting UI settlement | Reject the switch locally: no marker, generation, selection, attachment, projection, or navigator change. Report local busy. This guard precedes the differing-target rows below.                                                                                                                                   |
| Any otherwise selectable phase, same target, and one Refresh is queued/in flight/awaiting UI settlement    | No-op, including RetryableAttachUnavailable: no marker, generation, selection, attachment, projection, or navigator change. This guard precedes every same-target row below.                                                                                                                                        |
| Ready, target differs                                                                                      | Atomically admit a Switch marker. On success update active/desired immediately, clear visible projection/feedback, enter Switching, and dismiss the navigator.                                                                                                                                                      |
| Ready, target is the active Tab                                                                            | No marker, generation, attachment, projection, or navigator change.                                                                                                                                                                                                                                                 |
| Switching, target differs from the current desired Tab                                                     | Admit another marker if capacity/generation permit; it supersedes prior setup. Resolution for n/p starts at the current desired Tab.                                                                                                                                                                                |
| Switching, target equals current desired Tab                                                               | No-op; no duplicate marker or generation.                                                                                                                                                                                                                                                                           |
| RetryableAttachUnavailable, attachable target equals active                                                | An explicit selection admits a fresh marker and generation; the executor skips Detach because cleanup left no viewer. There is no automatic retry.                                                                                                                                                                  |
| RetryableAttachUnavailable, target differs                                                                 | Atomically admit a marker/generation, skip Detach because no viewer exists, and dispatch by that target's retained provenance exactly as for SelectedWithoutAttachment.                                                                                                                                             |
| SelectedWithoutAttachment, target differs                                                                  | Atomically admit the same marker/generation transition. The executor skips Detach because no viewer exists, then dispatches by the **target's** retained provenance: Started/BindingOnly performs normal Attach setup; Pending/unavailable/failed/outcome-unknown settles SelectedWithoutAttachment without Attach. |
| SelectedWithoutAttachment, target is the selected non-attachable Tab                                       | No-op; no marker or generation.                                                                                                                                                                                                                                                                                     |
| Creating, AwaitingCommittedProjection, ProjectionUnavailable, Starting, or Retired                         | Reject effectful selection. Only the recovery actions explicitly named below remain available.                                                                                                                                                                                                                      |
| Queue/generation full                                                                                      | Preserve selection and attachment state; report local busy.                                                                                                                                                                                                                                                         |

For a one-Tab cache, n/p resolve to the same member: Ready is a no-op and
RetryableAttachUnavailable is an explicit retry only when no Refresh is pending. While Refresh is pending,
the same-target no-op guard applies in all otherwise selectable phases. A full-queue or out-of-range action does not
dismiss an already open navigator. Panel keys remain unavailable.

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
the separate Refresh and Switch models do not claim this cross-product.

## Switch setup and supersession

The shared Switch installation and handoff foundation has state, mailbox and actual UI tests.
The Snapshot-first executor and late-discard anchor invalidation still require integration; the
foundation alone does not establish the complete Switch runtime described below.

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
origin. Ready restoration requires the same exact viewer/controller and a still-current full-frame,
draw/feedback and geometry proof after truthful pre-marker settlement. Accepted Resize that clears
the frame, unmatched latest geometry, unpreservable old Poll/causal abandonment, or viewer cleanup
or replacement invalidates that proof. The anchor follows those facts; an admission-time clone
cannot roll them back. A recorded selected-without-attachment or retryable origin may restore only
while it still has no viewer/controller. A partly completed Switch never creates a stable anchor;
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

`NativeTabSwitch` bounds two admitted intents independently of the generation cursor. Its four
configuration variants partition initial controlling/read-only mode and generation bounds four/two.
Every configuration retains all admitted targets, same-target retry and candidate/fallback choices;
no state constraint or fixed Tab-name shortcut is used.
Four permits two complete operations (intent 1 / install 2 / intent 3 / install 4); two additionally
checks successor exhaustion. Neither bound fabricates a refusal when the two-operation exploration
limit is reached. All original 51 required actions remain configured, with candidate, Gone, exact
anchor restoration, invalid-proof and second-marker-without-anchor retirement witnesses added.
Two guarded action witnesses additionally require the exact intent set `{1, 3}`, a retained Ready
anchor at installed generation 2, installation from cursor 3 to generation 4, and the second
generation's full Ready proof. They witness two completed switches without adding state or changing
the transition union; merely allocating cursor 4 does not satisfy this evidence obligation.
This model declares a 600,000 ms verification timeout per configuration with one TLC worker. Two
earlier 180,000 ms checks did not complete, including a first-target-class partition that did not
reduce exploration sufficiently. Those timeouts remain failed evidence; neither the observed
state counts nor the larger fixed limit guarantees completion. The verification resource limit
does not change either runtime deadline, finite bound, checked property or fairness assumption.
Monotone allocation and exact restoration replace the obsolete unconditional cursor/desired
equality. The only added fairness is exact live-UI settlement of a timely Unavailable result; no
successful Snapshot, restoration eligibility or Host completion is assumed. Shared identity bytes,
immutable snapshot tuples, real old-flight proof updates, numeric deadlines and runtime restoration
remain deterministic adapter/core evidence, not conclusions from this finite model.

The worker processes a current Switch marker as one setup operation:

1. settle earlier accepted Input/Resize;
2. settle any already handed-off Ready-generation Poll/Feedback response on the sequential carrier,
   then read, deposit and validate the current candidate before exact old cleanup and atomic fresh-key
   installation as above; Detach clears only an unsent/staged or exactly answered old projection flight;
3. exact AttachBinding for the installed candidate's validated Site/Worker/Terminal binding;
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
including supersession and exact deadline equality; the switch model abstracts these cleanup
steps into one settlement transition.

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
frame delivery, after draw, and after Feedback handoff; the switch TLA models the setup-generation
frame/feedback counterparts, including supersession after Feedback handoff.

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
separate bounded refresh and switch models intentionally do not claim this cross-product.

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
Presentation label, Site/context mismatch or loss of the phase-required active identity retires.
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

The stable English and Chinese status strings are below. Ordinary Move admission refusal preserves
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
is closed. The ordinary status bar therefore gives PTY evidence as the conjunction of one active
compact token and the exact Ready string; a transient Creating/Refreshing draw is never required.

A definite Create precommit settlement never creates a timer-driven phase. The UI consumes the
typed result by restoring the exact attachment directly to Ready and storing the localized notice
in `Ready.notice`; that notice replaces the ordinary Ready footer until the next decoded
`Ctrl+B` Client action atomically clears or replaces it. Ordinary terminal bytes, Poll/frame
traffic, and physical Resize observations do not clear it. A Pending committed target remains
SelectedWithoutAttachment with the exact Pending status above. `Ctrl+B w` can refresh canonical
membership, but it does not replay Create, attach that Tab, or erase its retained Pending
provenance.

## Staged Rename transport adapter

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

The deadline remains the earliest caller, per-exchange, and existing startup deadline. There is
no SnapshotRead, Start, Attach, selection, label mutation, or request identity allocation. This
deterministic classifier maps to the existing `LocalClientSubmission` exchange and invalid-result
classes; it adds no state, clock, or retry semantics. Adapter tests exercise complete typed request
preservation through decoding, compatible and incompatible outcomes, revision bounds, and prewrite versus
post-handoff deadline failures. The abstract model does not prove label bytes or revision arithmetic.

This is a staged transport foundation. Its intended consumer is the native Rename command and
mailbox/UI flow, which remains unimplemented; this adapter does not deliver native Rename UI.

## Staged Rename projection evidence

`workbench_projection::renamed` validates a snapshot against a captured canonical baseline and
known Rename revision. The exact context/Tab must exist in the baseline, the requested label must
differ from its old label, and the committed revision must be `checked(baseline + 1)`. At that exact
revision, the complete projected vector must equal the baseline with only the target Tab label
replaced by the exact requested text. Other label changes, reorder, addition, removal or immutable
member changes are divergent.

A later revision uses the shared strict canonical membership rules. The renamed Tab may have a
later label or be absent; the validated result carries the candidate and an optional target index.
Neither case invalidates the historical commit. The expectation and baseline are borrowed and
remain unchanged. A same-label `Unchanged` result is outside this known-commit validator.

This is pure projection evidence, not proof of terminal disposal, attachment cleanup or UI
installation. Focused tests cover exact label preservation, revision overflow, forbidden changes
at commit, later evolution/removal/Empty and overlapping identity rejection. It adds no state
machine or model claim. The native Rename command, mailbox, editor and membership consumer remain
unimplemented; they must preserve this distinction when consuming the helper.

## Staged Rename Unicode draft buffer

`state::rename_text::RenameText` holds exact editable label text for the future native Rename
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
allocate requests, admit commands or perform I/O. Its required next consumer is the native
RenameDraft; that integration and the editor remain pending, so this buffer is not feature delivery.

## Accepted Rename consumer, runtime pending

The private Rename operation owns one immutable typed request, captured draft intent and original
deadline. Result preparation independently checks unchanged-label/base-revision and changed-label/
checked-successor semantics, then calls the strict Rename projector. Valid known commits remain
available when projection validation fails or Snapshot is unavailable. This operation preparation
is staged for the actual mailbox/executor/UI consumer; it does not allocate identities, submit a
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
destroy the draft. These state operations remain staged until the input, mailbox and UI consumers
are connected; they do not themselves submit Rename or enable a keyboard entry.
`state/tests/rename_draft.rs` exercises visible and revoked tokens, exact unchanged intents,
atomic text-limit faults, explicit correction, incomplete input, independent baseline/generation
staleness, cancellation, retirement and reciprocal state admission. Mailbox capacity, cursor
allocation and in-flight Close exclusion remain obligations of the submission consumer.

`Ctrl+B ,` opens a state-owned draft for the exact selected standalone Tab in a live stable phase.
Capture the canonical baseline, Site/context/Tab and attachment generation without allocating a
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
refuses entry. The next input-parser implementation must maintain these states across reads/ticks.

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

The Rename lifecycle model must precede dependent runtime. Its bounded abstraction covers exact
draft qualification, two Rename admissions, one competing structural flight, original exchange
cutoff/typed placement, keyed historical facts, one stale receipt and current-worker join. Input
bytes, scalar arithmetic, eight-slot storage, concrete labels and full membership vectors remain
executable refinement obligations. Shared cleanup/setup proof is an external owner obligation;
fairness covers local consumption of available exact completions and existing local expiry, never
production of Host cleanup or Attach success. Native input/process refinement remains pending;
existing Move model receipts are not Rename evidence.

### Rename lifecycle model boundary and evidence

`NativeClientRenameLifecycle` has one fixed Site/context, two target representatives, at most three
draft openings and two Rename admissions. Its four request identities include Snapshot/Refresh
allocation from the same cursor; one recovery admission is modeled per Rename. Same/changed text,
valid/invalid/incomplete input and before/expired/ended-budget clock classes replace concrete
strings and time. Ready and selected-without-attachment configurations cover the two effect
classes; RetryableAttachUnavailable maps to the latter because this pure command attempts no Attach.
One old worker and one current worker are distinct representatives; no reconnection is modeled.
Competing status, structural work and Close confirmation are separate classes: status may coexist
with an unsubmitted draft, while confirmation cannot. Base-only and selection-only changes test
the two captured-authority guards independently. A selection change denotes an already applied
external-owner transition to a non-Ready state, not new attachment proof produced by Rename.

| Owned obligation                                   | Model assertion or transition                                                                                                    | Concrete refinement still required                                                                                      |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Exact visible draft and explicit error correction  | `AdmissionRequiresExactDraft`, `OnlyExplicitCorrectionClearsFault`, `CapturedOperationNeverRetargets`                            | Actual draw receipts, stale baseline/selection, Backspace/Ctrl+U and fragmented input.                                  |
| One flight and one command handoff                 | `StructuralFlightsExclusive`, `NoRepeatHandoff`, `LocalDraftEditsDoNotAllocate`                                                  | Real eight-slot/causal storage, cursor no-burn, Move/Close/Refresh/Create/Switch exclusion in both directions.          |
| Known commit and fixed budget                      | `CurrentCommitFacts`, `NoopUsesSameLabel`, `KnownCommitNeverRegresses`, `ExistingCutoffNeverRenews`                              | Exact label-only/later projection, checked revisions and original 50-second budget including Snapshot and deposit.      |
| Untimed exact consumption and late acknowledgments | `PendingUiIsUntimed`, `TypedResultNamesCurrent`, `AcknowledgmentPreservesCurrentFlight`, `PendingAcknowledgmentIsNotOverwritten` | A consumed/B admitted/late A acknowledgment, exact UI/worker keys and no flight replacement.                            |
| Retirement and external membership evidence        | `NoInventedUncertainty`, `JoinedHasCurrentReturn`, `MembershipInstallNeedsExactOwner`, `AvailableOwnerResultIsUntimed`           | Possible-handoff versus definite non-submission, current-worker return, exact cleanup/Empty handoff and deadline owner. |

The membership completion input denotes an already produced exact owner result. Its availability
ends that owner wait budget; local consumption may occur later. No fairness forces such a result
to be produced. Existing local owner expiry may retire a wait with no result. The model never
constructs new Ready from a removed-target snapshot or owner membership result.

Pinned TLC 1.7.4 passed both registered configurations, safety and temporal checks, and all 42
required semantic action coverage entries, with one worker, `--jobs 1` and the unchanged 180-second
timeout. Both complete graphs have depth 37 and zero queued states. These are bounded model receipts.

| Configuration                 | Generated / distinct states | Module plus effective configuration SHA-256                        |
| ----------------------------- | --------------------------- | ------------------------------------------------------------------ |
| `ready`                       | 2,140,506 / 318,440         | `a146d4ef78797c84f6750ad7dc6b661a4132f9c77d9cf33436e02358e29555b1` |
| `selected-without-attachment` | 2,031,640 / 304,420         | `0de583638a9b4ee9c894d410609f74a8b5a3dffc99897b73ab33278878f0a454` |

```sh
node scripts/tla/run-models.mjs --models native-client-rename-lifecycle --jobs 1
```

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

| Claim                                                                                                                       | Lowest evidence                                                                                                     | Additional witness                                                              | Limits                                                                            |
| --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Strict 0..256 projection and identity-preserving refresh                                                                    | exhaustive partition/unit cases and corpus-derived fixtures                                                         | generated create/rename/move histories and exact commit-permutation tests       | Higher revisions may reorder and rename Tabs; immutable member facts survive.     |
| Selection wrap/bounds/same/retry and atomic refusal                                                                         | pure selection table tests                                                                                          | generated mailbox traces with stable seeds, semantic shrinking, and replay text | Finite generated trace sizes are reported, not universal proof.                   |
| Create drains old Poll/Feedback before submit and precommit restores                                                        | `NativeClientCreateBarrier.tla` plus fake-carrier traces                                                            | deterministic old-frame/draw/feedback checkpoint trace                          | TLA abstracts bytes, clock, geometry, and rendering.                              |
| Commit validates/stages a Snapshot candidate before exact old cleanup and atomic selection; only surviving Started attaches | Checked Create model; actual UI dual-plan installation, candidate-first carrier and generated mailbox traces        | fake carrier at each blocking response                                          | Carrier retirement cleanup is not a successful Detach or candidate-install claim. |
| Rapid switch cleanup/generation/first-frame/geometry gate                                                                   | `NativeTabSwitch.tla` plus deterministic core traces                                                                | real PTY active-token + Ready checkpoints                                       | This model is separate from Create/refresh and abstracts real pixels.             |
| Refresh single-pending generation fence/identity/active preservation and navigator toggle                                   | `NativeClientRefreshFence.tla` plus deterministic stale-event tests                                                 | two-Client/process refresh witness                                              | No push or inter-read freshness.                                                  |
| Move preserves the selected terminal and exact known commit                                                                 | `NativeClientMoveBarrier.tla`, mailbox/state, worker and actual NativeUi/outer retirement traces                    | two Linux cohort Move scenarios                                                 | PTY shell continuity does not independently prove absence of viewer reattachment. |
| Total setup termination                                                                                                     | fake monotonic clock at Empty/NeedSnapshot/Blocked/Busy and cleanup reserve boundaries                              | PTY uses named Ready/unavailable checkpoints, never sleep                       | Does not prove Host eventually responds successfully.                             |
| Safe exact label rendering                                                                                                  | pure sanitizer/layout/backend byte tests                                                                            | PTY numeric labels and active marker                                            | OS font/pixel fidelity is excluded.                                               |
| Platform dependency boundary                                                                                                | Windows target `check/test` for help/version/unavailability plus Cargo metadata assertion; Unix build/tests for TUI | Linux real PTY                                                                  | No Windows TUI claim.                                                             |

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
active marker and its exact Ready footer. These process checks establish observable shell
continuity; the worker and UI traces separately establish attachment preservation. They do not
establish other operating systems, installation, restart recovery or Close.

## Model scope

The following scope records the checked Create/selection/Refresh baseline and its Move refinements.
Existing action obligations remain required; each model retains its stated finite limits.

`NativeClientCreateBarrier.tla/.cfg` checks one arbitrary Create barrier from a settled Ready
anchor or the typed Empty origin described above: bounded FIFO drain, causal old-flight feedback, effect admission freeze, precommit
restoration, retained commit before Snapshot, validated-candidate staging before exact old cleanup,
atomic selection installation, and Started-only Attach abstraction. One later response may move or
remove the returned member. The returned member keeps historical generation 1; a surviving frozen-
ordinal fallback uses generation 2, while Empty installs no active member. A definite Snapshot
unavailable preserves the old exact core behind the structural fence. The model intentionally has
one Old/New pair and does not prove sequential Create re-entry, arbitrary membership removal,
explicit ProjectionUnavailable recovery, or per-operation field reset; the named at-least-two-
operation generated/replay trace and `NativeClientMembershipRemoval` own those claims. The
`NativeTabSwitch.tla/.cfg` checks staged candidate installation, distinct intent and
successor allocation, conditional exact stable-anchor restoration for definite Unavailable,
pre-handoff Gone, explicit Detach request/result sequencing, viewer/controller uniqueness, superseded attached cleanup,
post-Attach failure/deadline cleanup, ambiguity retirement, generation fencing, first-frame
feedback, and final-geometry catch-up. `NativeClientRefreshFence.tla/.cfg` checks compound
navigator/open admission, at most one pending refresh, reopen-without-read, full refusal core
preservation, non-reusing refresh generations, exact pending-result application, append-only cache
replacement, cache-backed active preservation, defensive stale-completion invisibility, and one
abstract accepted Input/Resize record that remains queued through UI semantic settlement and may
execute only afterward. That abstract control checks the executor fence; deterministic reducer and
scripted-carrier tests distinguish Input from Resize and cover their concrete FIFO results.

These models do not claim JSON/correlation validation, exact label or pixel values, real time,
renderer safety, OS scheduling, or OBCS Busy retry timing; deterministic adapter, reducer,
fake-clock, renderer, and PTY tests own those edges. Each TLC result applies only to its named
Create, switch, or refresh abstraction; none is cited for either of the other two barriers.

The corrected CreateBarrier Ready/Empty family retains `MaxQueue = 2`, geometry `0..1`, and
one Create with exact revision 2 or one later revision 3 reorder/removal. Candidate generation 2
is a prepared successor: only `InstallValidatedCandidate` consumes it. Retirement before install
keeps the committed cursor at 1; Empty also keeps cursor 1 and has no active generation. The
known-Closed Gone actions take an externally verified capability for the exact old attachment as
input; this model does not acquire that proof or model independent Host identity. Membership absence
supplies no such proof, and an in-flight Detach cannot take that shortcut. Canonical Move/removal
interleavings occur before staging; candidate immutability is checked afterward, with no later
canonical-write action in this bounded model.
The install action belongs to the unsettled set and has weak fairness; deadline fairness guarantees
only a truthful stable outcome, not successful cleanup or Attach. Explicit recovery and attachable
fallback composition remain obligations of `NativeClientMembershipRemoval` and runtime refinement.

TLA+ 1.7.4 with one worker checked both corrected configurations, with zero queued states and all
53 required action names covered across the family. All previous failure and expiry actions remain;
`InstallValidatedCandidate` replaces the obsolete `CommitEmptyWithoutOldDetach` action.

| CreateBarrier variant | Effective model/config SHA-256                                     | Generated | Distinct | Depth |
| --------------------- | ------------------------------------------------------------------ | --------- | -------- | ----- |
| Ready                 | `da47bdb693b77398139b8825de235f9c0f4a60f2489693a5153c03619d8b2318` | 20,872    | 6,784    | 27    |
| Empty                 | `a277a6e320a5239fa0161559bea4329ecb66eac0f4fd947b04b98b2368de6c90` | 3,704     | 1,267    | 23    |

Create models old-viewer reserve completion as retirement with the known commit, staged candidate,
and old diagnostic selection retained; it cannot install or publish the replacement selection.
Switch models an
in-flight old Detach crossing the forward cutoff as the same exact request continuing in cleanup,
with current or superseded settlement. Weak fairness applies to the union of forward-expiry
actions: Busy phase changes cannot restart or indefinitely evade the one fixed deadline. Exact UI
placement, consumption, reply-deposit equality, and late reply consumption remain deterministic
mailbox and scripted-carrier evidence.
Current pre-installation cleanup after forward expiry retires; exact superseded cleanup may hand
control to the newer marker. The Switch model records whether the current work generation has attempted Attach. A new marker
resets that fact, while Attach Busy preserves it. Expiry after an attempted Attach retires rather
than using the no-attempt entry settlement, and required action coverage includes that distinction.
The Refresh liveness property assumes weak fairness for a live UI event loop: once a typed result
occupies the event slot, the transport deadline has ended and UI semantic settlement has no wall
clock bound. Exit or retirement wakes the waiting executor; absent either of those and UI fairness,
the design makes no eventual-settlement claim.

`NativeClientMoveBarrier.tla/.cfg` separately checks one user Move from Ready,
SelectedWithoutAttachment, or RetryableAttachUnavailable, with active A initially first, middle, or last among
three members; a Ready singleton variant covers no-op. Its transitions sample the checked
deadline before mailbox admission, preserve core state on admission refusal, capture the
request identity and baseline together, drain earlier queued/Poll/draw/Feedback work, and
submit Move at most once. A definite commit is retained internally through postcommit
Snapshot identity exhaustion, corrupt or ambiguous exchange, deposit failure, retirement,
and exit; the abstraction requires no intermediate UI commit acknowledgment. Exact commit
revision requires the expected permutation; one later canonical Move permits a higher
revision. Typed deposit must precede the fixed deadline, while exact UI consumption may
occur after it and retains the structural fence until consumption. Successful application
remaps provenance and active index by ID without changing the live Viewer/controller or
historical Ready fact. Definite projection unavailability preserves those facts and admits
only explicit recovery among structural operations. Recovery has a fresh sampled budget
and generation, and shares the request cursor. Other Create/Switch/Refresh flights are
opaque mutually exclusive operations whose cache remains frozen; their internal behavior
and correlated CreateSnapshot exception remain owned by the other three native models.

The finite bounds are one user Move, at most two recovery admissions, three request IDs,
queue capacity two, clock values 0..8, and budget two. The symbolic clock partitions model
checked addition, equality expiry, and a single unchanged transport deadline; they do not
measure real 50-second scheduling. Weak fairness of time, mailbox release, transport
expiry, and UI consumption establishes eventual settlement or retirement of an admitted
pending flight, not eventual recovery success. Mailbox contention is abstracted as held/free
admission and deposit cuts; exact nonblocking lock calls, byte/correlation rejection,
allocation, worker-join handoff, key decoding, rendering, and OS/PTY behavior require
implementation evidence. This is bounded model evidence for the accepted Move cohort;
runtime refinement and Linux cohort process acceptance have the separate evidence above.
It is not an unbounded concurrency or permutation theorem.
The one-Move bound excludes repeated-Move reentry and the race in which UI settlement of
Move A admits Move B before the worker consumes A's acknowledgment. Mandatory deterministic
state/adapter traces must check that B cannot execute or replace retained commit facts until
A's worker completion, and that retirement during this interval recognizes settled A while
preserving B and any newer facts.
Ready models a controlling attachment; read-only Ready additionally requires deterministic evidence.

## Model verification

### Close refinement checks

On 2026-09-08, the two changed Create configurations and nineteen membership-removal
configurations passed in separate family runs with pinned TLA+ tools 1.7.4 and one worker per
configuration. All safety and temporal checks completed with empty state queues; the Create family
covered all 49 required actions and membership removal covered all 48. The digests below identify
UTF-8 LF model sources and each variant's effective configuration. The finite scope, refinement
boundaries and fairness limits above still apply; these are not runtime or whole Close delivery
results. Both families can be rerun together:

```sh
node scripts/tla/run-models.mjs --models native-client-create-barrier,native-client-membership-removal --jobs 1
```

| Configuration              | Generated / distinct / depth | Model/configuration SHA-256                                        |
| -------------------------- | ---------------------------- | ------------------------------------------------------------------ |
| `create-barrier/ready`     | 13,592 / 4,054 / 26          | `4662ebe9d98dbe674b4f6eb9095adba16d3681dd55a1f31390f440f92311d652` |
| `create-barrier/empty`     | 3,519 / 1,162 / 23           | `f075eeccb739782ad04732ebf20a59f4edb6196872a65491dcd21a3fba263ff1` |
| `refresh-ready-control`    | 421,752 / 144,062 / 24       | `3cc839e14828c3e39963f531c2488db043154cb3737f924d601b8580720777aa` |
| `refresh-ready-readonly`   | 421,752 / 144,062 / 24       | `5cf22dfcbe74e082552598ba222ed5536212b2a7904d4327dd53753438a5d955` |
| `refresh-selected`         | 376,000 / 131,462 / 22       | `7bd0ed28386f95694b40ce8e5ae64d03f58729b7e34c9228af6d08a655b471d9` |
| `refresh-retryable`        | 374,040 / 127,542 / 22       | `45ee4f719fe1de3297b88e06bd57b28ece6da82f3cca1a2528a07c715c11eb0c` |
| `refresh-empty`            | 40,618 / 13,283 / 22         | `e35c4e16cce57e798c0426d06dd3f9621fac5c5aaa4dd661a101e6fb4d6b71a8` |
| `create-ready-control`     | 2,697,042 / 836,248 / 30     | `a99536a3e2885b07eeeebb827b92b2ec65af481483d7dec456760e1d756c2e7f` |
| `create-ready-readonly`    | 2,697,042 / 836,248 / 30     | `e3a9e79e6e6916b1ef055cccffa7514393156b4b9f11cdcd10638377d54a23f3` |
| `move-ready-control`       | 739,636 / 251,162 / 29       | `1c329f1d59c0c60efbb56284cbca691b4bd5640ac74ddac2e4bf17aed21fb2dd` |
| `move-ready-readonly`      | 739,636 / 251,162 / 29       | `61a1ef1c96f11e3bc601730d5baab56d7c14f002943202b6d2496ceb7a90043e` |
| `move-selected`            | 661,250 / 231,002 / 27       | `ec253efcfded19d107f8a41fa51a419af6bb86e45ff6c5bd624aa3234556b0b3` |
| `move-retryable`           | 656,938 / 222,378 / 27       | `2e2f9627805ccafa4d5a17ce5dbaf8e15b0f5fecb17ab77eca8d12d6a71e0269` |
| `switch-ready-control`     | 1,738,552 / 533,328 / 25     | `7333286df772274308ca7d5c3291170787f269ba6dca2e501cdc14b13bae08c7` |
| `switch-ready-readonly`    | 1,738,552 / 533,328 / 25     | `cc065cd7889955163660be460e73cc845a0ecb1f5ef6c688f94b8888fd4f6388` |
| `switch-selected`          | 759,482 / 237,978 / 22       | `d1553ee025912b097283d4c53bc93e558d0c2c894519ccc9c5fcbc2d606eb077` |
| `switch-retryable`         | 779,378 / 232,500 / 22       | `88cbb7206ed8884723f8df0093d62acfe5bb7006b546904c843bc92ca631fc6d` |
| `own-close-ready-control`  | 1,115,736 / 346,322 / 29     | `8985add91e8339c7b78c6d0bdefaa8f19377fdce4f77edc7b57298af447b219a` |
| `own-close-ready-readonly` | 1,115,736 / 346,322 / 29     | `4cb64ac9656b21ded2b10cd67016786f91ef6a9ce68012701097deab13fcbfe4` |
| `own-close-selected`       | 945,468 / 306,758 / 27       | `8b08a06211580c24d31e5e79374980a821374bd7d49822e7fbdefdc12b30dd74` |
| `own-close-retryable`      | 925,868 / 285,198 / 27       | `5ae9e77eb7f67c286b842776709681bcb3fcebbdac8d31a62b4e1c2a9302e0ea` |

### Switch membership installation evidence

Deterministic tests exercise the actual sequential executor and scripted carrier for Snapshot
before cleanup, surviving/fallback/Empty installation, fresh successor keys and original deadlines.
Separate production `NativeUi` tests cover same-lock cleanup receipts, supersession and expiry.
The returned-old-Poll and already-taken-Feedback cuts require an exact discard fact; disabling
either producer fails its regression. Busy without a frame preserves restoration, while admission
tests cover discarded event/cell, missing callback, refusal preservation, exact lineage transfer
and queued-to-active retention. Scripted UI drivers are not evidence for terminal rendering or a
real PTY process; the finite model results below remain unchanged.
The actual worker also preserves a timely Unavailable Snapshot until UI settlement after the
original total deadline; a bounded post-placement reply wait fails that deterministic regression.

Run `node scripts/tla/run-models.mjs --models native-tab-switch --jobs 1` with pinned TLA+ tools
1.7.4. All four configurations below complete safety and final temporal checking with empty
queues and positive generated coverage for all 77 required actions across the family, including
the two-completed-Switch witnesses. Each configuration has three Tabs, two attachable, two admitted
intents, `MaxQueue = 3` and `MaxGeometry = 1`; `two-switch` uses `MaxGeneration = 4`, and
`exhaustion` uses two. The fixed verification timeout is 600,000 ms with one worker per configuration.
These results establish only the finite Switch interface described above, not membership-vector
validation, runtime anchor proof preservation, numeric deadlines or real carrier/UI behavior.

| Configuration         | Generated / distinct / depth | Model/configuration SHA-256                                        |
| --------------------- | ---------------------------- | ------------------------------------------------------------------ |
| `two-switch-control`  | 8,790,448 / 2,220,844 / 69   | `946b0ac5cab403701cb53254f28087375cc6bd57869b1046cb968cfcea26682c` |
| `two-switch-readonly` | 2,306,238 / 567,200 / 65     | `ad32026f23e6c5d2b783f954d4ecf6382093408cad40f5066d049ff71b149660` |
| `exhaustion-control`  | 355,697 / 97,340 / 44        | `6186c9818a558457bd0399becc76d699f16fe2fab099f06482b79a8328b43b87` |
| `exhaustion-readonly` | 112,009 / 30,832 / 40        | `06e0c19c36637188962a091f6b5d6db1a815d2b1bda3271f06be8c0ce5752ebc` |

### Earlier no-removal evidence

Run the registered Client checks from the repository root:

```sh
node scripts/tla/run-models.mjs --models native-client-create-barrier,native-client-refresh-fence,native-tab-switch --jobs 1
```

The following pre-Move configurations passed with pinned TLA+ tools 1.7.4, one worker per model, completed temporal
checking, empty state queues, and positive generated coverage for all 116 required semantic
actions. The model/configuration digests identify the exact registered inputs; the preceding
scope and fairness limits still apply. These hashes/counts are baseline evidence, not Move evidence;
changed model inputs require separately reported checks and hashes before dependent integration.

| Model                       | Finite bounds                                                                      | Generated / distinct / depth | Model/configuration SHA-256                                        |
| --------------------------- | ---------------------------------------------------------------------------------- | ---------------------------- | ------------------------------------------------------------------ |
| `NativeClientCreateBarrier` | `MaxQueue = 2`; one Old/New pair                                                   | 2,717 / 842 / 24             | `38629543117f6ebdb1db4b84f01ec563a197fe2d8ad4f3c25b80b65c3e005368` |
| `NativeClientRefreshFence`  | `MaxQueue = 1`, `MaxRefreshGeneration = 2`                                         | 56,530 / 14,710 / 20         | `01a82fe7e298c1cae9561e7c3a30d234874fb15f5df4c7b33e99199a5cfc31f2` |
| `NativeTabSwitch`           | Three Tabs, two attachable; `MaxGeneration = 2`, `MaxQueue = 3`, `MaxGeometry = 1` | 1,904,382 / 425,800 / 61     | `510b243c0d5164dd6d5e885cb276ff2258aecd0c521634334d2d2381a32aaa1a` |

Run `node scripts/tla/run-models.mjs --models native-client-move-barrier --jobs 1` with
TLA+ tools 1.7.4. The four configurations complete temporal checks with empty queues and
all 41 required semantic actions covered across the family:

| Configuration        | Generated / distinct / depth | Model/configuration SHA-256                                        |
| -------------------- | ---------------------------- | ------------------------------------------------------------------ |
| `ready`              | 4,710,774 / 777,516 / 31     | `90d94e5d928e0808097f268774e8a7fa2c36e22f1d4ed94198dd8e7fe1ac8dbd` |
| `selected-no-attach` | 470,349 / 113,340 / 28       | `eb5fe3b64033cfbceb44fceaaa688cc31817b314aa4ebabf5553ebf3cf390b6a` |
| `retryable`          | 470,349 / 113,340 / 28       | `144f9a7b682c4ada75a2089dac158aa8ac945071f1ff5e06f72ecf860be0dd3d` |
| `singleton`          | 1,056,290 / 147,508 / 20     | `4699f37d9ce6bcb6d030a29f13b6837c7c938fab2b1ce817d2ab809d7b3c73f3` |

## Retired peer membership recovery

This entry is for a non-initiating native Client whose exact current ordinary Poll returned
`TerminalRuntimeUnavailable`. It retains that correlated request and attachment key, immutable
binding, canonical Site/context baseline and historical ordinal. Stale or setup Polls, transport
uncertainty, malformed replies and other failures do not establish this origin. Activation requires
successful exact old-task join, closed old transport and settlement of all returned facts. Panic or
missing return evidence cannot establish eligibility. Any retained Close/Read record or flight, or
unresolved structural mutation, commit, setup or cleanup handoff excludes this entry. Existing
Close status recovery retains priority and all its facts.

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

Before installation the only request is SnapshotRead. The full candidate must preserve Site/context,
nondecreasing revision, exact same-revision membership and every immutable retained identity tuple.
The old unavailable Tab must be absent. A still-present target, unavailable Snapshot or invalid
candidate leaves the old canonical state and origin parked, with no Attach. Absence is membership
knowledge, not cleanup evidence. Bootstrap, Open, activation, Create, Start and Close are not part
of this observation entry.

Prepare the candidate outside the reply mutex, then atomically commit the state and mailbox plans
under that mutex after checking the exact retired origin, baseline, flight and forward deadline.
Choose the clamped historical ordinal or Empty. A survivor advances the actual burned attachment
cursor once at installation; Empty preserves it. Retained provenance follows the common projector.
A survivor uses the remaining setup budget on the new connection without automatic Start; Empty
accepts Refresh and never automatically creates a terminal. Pre-installation expiry preserves the
old canonical state. Post-installation failure preserves the committed candidate and cursor while
retiring truthfully. Stale results cannot install into another flight.

`NativeClientRetiredMembershipRecovery` owns the proposed independent finite entry model; the
nineteen `NativeClientMembershipRemoval` configurations retain their existing terminal Retired and
Detached/Gone meanings. Bounds are old A and at most two neighbors B/C, Empty and ordered candidates,
two explicit attempts, prior attachment cursor 1 or burned 2 with maximum 3, bounded monotonic
allocation/exhaustion, and independently retained/released old remote reference. Validation is a
typed external input refined by the real canonical projector. Required checks cover origin and
conflict guards, single carrier, no old-local submission after join, no fabricated cleanup/Close
fact, target absence, atomic installation, stale results, fixed deadlines, cursor preservation and
no automatic retry/Create. Progress assumes only local deadline and carrier termination fairness;
it promises a live result or truthful parked retirement, not remote cleanup or recovery success.

Runtime refinement is pending: correlated Poll handoff and join, real Snapshot-first reconnect,
conflicting retained facts, still-present/invalid/unavailable candidates, late UI and cancellation,
common Empty/neighbor installation and allocator exhaustion require actual carrier/state tests.
The Linux PTY Close witness must observe a new Disconnected frame before explicit Refresh, then
both Clients' Empty state, full public Snapshot equality after exact removal, original shell exit
and a bounded 500 ms fresh-observation check against automatic Create. Existing failing process
traces remain evidence of the missing entry, not a passing implementation claim.
