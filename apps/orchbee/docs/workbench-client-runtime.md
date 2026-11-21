# Workbench Client Runtime

[Native context selection](native-context-selection.md) owns Workspace navigation, per-Client Tab/Panel
selection memory and the current-context projection extension. Its location identity, Empty selection and retained
provenance rules extend the Workspace startup and same-context Tab workflows below.
[Native Workspace setup](native-workspace-setup.md) owns staged creation and first-directory input,
sharing this Workbench Client's structural admission, carrier and membership settlement.

Workbench owns status selection in `workbench_client/ui/status_text`: Worktree, takeover, Close, terminal-end, duplication and setup/retirement precedence read the same immutable view in the same order. Localization receives the selected semantic facts and bounded close/launch display descriptions; it never receives the Workbench view or its private operation tokens. Display vocabularies are shared below the owners, while startup lifecycle, notice expiry, capability-to-action selection and all executable authority remain with their feature owners.

## Accepted Close refinement and local integration

[Canonical terminal Tab Close](../../orchbee-server/docs/terminal-tab-close.md) owns the accepted
Server/Worker lifecycle, wire/status/Resume and native removal contract. Native `Ctrl+b x` targets
the selected Tab in the active context and confirms in both locales that execution ends for all attached Clients;
an independent lifecycle grant is required. Closed requires known disposal, target-owner cleanup
and canonical removal. The last removal leaves a usable Empty Workbench with Refresh/manual `c`,
without a replacement shell or Client exit.

The record specifies active/inactive removal, old-ordinal survivor selection, Empty Create origin,
known Close retention and exact/later CreateSnapshot, Switch, Move and Refresh settlement. A later
snapshot may remove IDs while preserving surviving immutable identities; an exact Create/Move commit
revision retains its stricter expectation.

## Close control and recovery contract

Windows selected/group/other/trailing Close is a bounded coordinator over these existing owners.
The [close-set contract](native-tab-close-sets.md) governs frozen targets, one confirmation,
the sequence cutoff, pre-install local identity fences, partial counts and the intentional
source-group-only cleanup boundary. It adds no batch wire mutation or replacement Close authority.

The native state owner retains Close knowledge independently of terminal selection and projection.
Linux native input, confirmation drawing, Close/Read execution and explicit status-only recovery are
integrated. Closed membership installation now uses the common exact-cleanup consumer, and record
pruning occurs only after both the exchange and reconciliation references are released. The protocol
adapter validates wire correlation; the native owner validates exact operation identity,
nonregressing attempt generation, retained disposal/Closed facts and UI flight identity.

Retirement at an unresolved submitted mutation, including cutoff or explicit retirement, preserves
local submission uncertainty and the exact identity for status recovery. A queued-only cancellation
or a definite typed precommit result does not invent submission uncertainty. Each joined worker
must have return evidence for that exact carrier; a fresh recovery carrier cannot borrow the old
worker's completed-return fact. Retirement preserves an already observed current-worker return.

Finalizing native retirement invalidates the displayed Workbench Client once, even when unchanged
geometry observation discovers the closed mailbox before the next UI tick. A successful intervening
draw also consumes this notification; unchanged retired ticks do not redraw continuously. Retirement
clears the navigator's transient Refresh display together with its pending generation.

| Boundary                      | Required native transition                                                                                                                                                                                        |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Initial Close                 | Capture the exact selected Session/context/Tab and canonical base. Render the localized all-attached-Clients warning before arming confirmation; coalesced unseen confirmation cannot submit.                     |
| Close again                   | A stopped close takes the ordinary close path and asks again. Its record restarts its follow, unreported for the pending delay, with its Tab shown until a read finds `closing`.                                  |
| Cancel or local refusal       | Preserve selection, attachment, knowledge, request cursor and mailbox position.                                                                                                                                   |
| Accepted command              | Retain the canonical primary operation and accepted revision; an alias may acknowledge an older accepted base. Acceptance does not imply removal.                                                                 |
| Lost command or control reply | Retain the submitted identity and known facts; retire the ambiguous carrier without mutation replay. An early NotClosing read after ambiguous submission is inconclusive because acceptance may still be pending. |
| Read observation              | Denied/unavailable/query failure does not become Host outcome unknown. Known acceptance followed by NotClosing, changed operation or contradictory Closed revision is divergence; preserve known facts.           |
| Closed observation            | Retain exact target, canonical operation, attempt and immutable removal revision before any fallible Snapshot or UI handoff. Export the membership owner's exact target/minimum-revision expectation.             |

Successful terminal draws report exact opaque prompt and status tokens. Semantic status consumption
does not establish that the status was displayed. A confirmation requires its own exact current
prompt to be shown; renderer assumptions do not replace this state-owner check.

Prompt presentation and exact draw proof remain separate owner facts. `Presented` admits the
incarnation only with its exact prompt token. A missing or covered receipt revokes current shown
authority without classifying geometry or discarding the presentation phase. An evaluated
`NotPresented` geometry result refuses and removes a new prompt that has never painted, leaving the
localized resize fallback in that frame and no invisible input owner. After a successful paint, the
same result suspends the existing prompt while preserving its token, frozen intent and local focus;
`Enter` and stale pointer releases remain inert, local `Esc` may cancel, and a later complete paint of
the same token resumes it. Accepted Close work already owns its operation independently;
hiding or closing its former prompt cannot undo or cancel that work.

Observing an actual viewport or geometry change revokes both prompt and status display
qualifications before dispatching an already-read input packet; a resize followed by confirmation
in the same loop iteration cannot use the old draw. The shell signals any changed outer
WindowDimensions through `invalidate_close_display` before dispatch, even when a sidebar change
leaves terminal geometry unchanged; `observe_geometry` separately invalidates actual terminal
geometry changes. Equal outer and terminal dimensions do not invalidate on every loop. The prompt's
phase, intent and facts survive revocation of current draw proof and remain subject to the next
complete or geometry-blocked draw. Prompt-owned input retains the remainder of its decision packet
after confirmation or cancellation, so trailing bytes from that packet cannot reach the shell;
subsequent separately decoded input belongs to the then-current owner. Unrecognized prompt input,
including Enter, cannot confirm.

The renderer measures the sanitized target, warning and actions with Unicode cell widths and
ratatui's actual wrapped line count in the final modal content rectangle. A grapheme wider than
that rectangle is incomplete even when the wrapper would omit it. Incomplete content shows the
localized resize/cancel hint and yields `NotPresented`, never a successful prompt token. The owner
then applies the refused-or-suspended rule above. Both the complete modal and its hint suppress the
old terminal cursor before terminal cells are drawn. A status token describes exact text still
completely visible in the final frame; notice replacement, clipping or later coverage cannot
qualify it.

Keep at most 256 target-keyed Close records. At capacity, refuse a new record as local Busy before
request or queue allocation. A record can outlive current membership while an unresolved result or
worker/UI handoff references it. Prune only settled removed records after membership consumption
and release of every handoff reference. Monotonicity concerns owned retained facts, not an
unbounded registry after permitted pruning. Settling A and admitting B before the worker consumes
A's acknowledgment must preserve both identities; a late A handoff cannot replace B's flight.

Known own acceptance suppresses new target Input, Resize and ordinary Poll while preserving exact
attachment cleanup identity. Already admitted causal work settles truthfully; unsent queued work
is not acknowledged as delivered. Other targets become usable only after exact fence settlement.
Each independently admitted command or Read captures one fixed 50-second transport/deposit
deadline. Earlier causal deadlines remain earlier. Timely typed placement ends that exchange
budget; UI semantic consumption has no timeout. A later Read obtains a new budget only after
the preceding exact key settles. Busy, progress and UI delay never renew an admitted exchange
or existing Create/Switch 45-second forward and 50-second total budgets.

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
generation and known facts unchanged; manual Refresh remains available and
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

Status-only recovery admits only Reads: it opens no confirmation and sends no close, and it grants
no general session or changed-authority recovery.

While the close carrier differs from the Client carrier — status-only recovery, or
MembershipRecoveryPending before its atomic membership install — the ordinary
pane-reconcile enqueue stays suppressed. The carrier mailbox's desired anchor is
not the Client's retained anchor, so an ordinary admission would mint work the
carrier must reject as stale. The predicate is the close carrier itself, not a
Client phase: CloseRead and ReconcileClose scheduling stay active, and the carrier resets only
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
creates one fresh carrier, admits only Read exchanges, and never
runs bootstrap. The old retired attachment remains unavailable. Its already-finalized retirement
cause cannot retire a successful new status observation; failures from the new mailbox still do.
An early NotClosing after uncertain submission settles that Read but preserves the original
submitted exchange and unavailable observation. Only a validated canonical Available status can
resolve that uncertainty. Foreign target/status responses preserve the original identity and
retire the fresh carrier without adopting the response.

The first semantically settled exact Read establishing Available Closed owns the immutable
Close reconciliation origin: its original flight key, canonical operation, attempt, Closed revision
and captured binding/attachment association. Repeated Closed observations cannot replace that
origin. A separate Snapshot marker pairs it with a fresh request ID and independent fixed 50-second
transport/deposit deadline. The common membership result validates both identities. Snapshot
failure retains the unconsumed origin; no raw absence or unavailable result proves cleanup.

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
moving an old Presentation into another Panel is rejected. This preserves the user's layout while keeping Presentation binding
identity immutable. Session allocators, rather than this two-snapshot comparison, own
lifetime non-reuse.

Replacement of any Panel in the active Tab uses the existing whole-anchor membership cleanup and
install path, retaining the selected Panel and maximized state while minting a new attachment
generation. Old Viewer and controller effects finish or are disposal-proven before attaching the
fresh binding. Replacement clears old Tab launch provenance; failed or unknown old startup never
becomes the new shell's startup state. Replacement in another Tab updates its canonical binding
without detaching the active Tab; selection later attaches the new binding.

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

From the forward cutoff onward, known cleanup permits retirement only, without candidate
installation or a new Attach. Manual Create from Empty and the other Create/Move/Switch/Close origin
consumers have separate origin-specific refinements described below. This Refresh slice does not
construct or consume a Close Gone proof and does not change Switch supersession.

One membership owner prepares a validated candidate, remapped provenance and selection by preferred
ID, then historical ordinal, then last survivor, then Empty. Empty-origin selection uses the first
survivor. Inactive-only replacement preserves the exact attachment, projection, geometry and cursor.
Other candidates freeze old state until exact attachment cleanup under the stored deadline owner.
Snapshot absence is never cleanup evidence. Close alone may construct the opaque exact-generation
`DefinitiveClosedAttachment`; it cannot override an already handed-off Detach.

`Preserve` carries the entire current `PaneKey` (Tab anchor and Panel ID), or `None` for Empty,
unchanged through both plans. It must not reconstruct the desired Panel from a surviving Tab's first
leaf. Ready selection, pane incarnation, selected presentation, controller, frame, geometry and
sibling records remain intact.

## Move Tab keys and identity rules

`Ctrl+b <` and `Ctrl+b >` run the global menu's Move Tab Previous and Move Tab Next rows on the
current Tab. A key and its row are one invocation: the same availability rule, the same
`tabs.configure` `move-tab` operation, the same setup transaction and the same statuses. The Client
keeps no second move flow and never sends `tab.move` or `tab.move-index`.
[Native groups](native-tab-groups.md#shared-authority-and-receipts) owns the operation and its
receipt check, and [Native action menus](native-action-menus.md) owns the row rule.

A key therefore gives its row's result in every position. A Tab moves only among the Tabs pinned as
it is: with none to pass the key submits nothing and shows nothing, and past either end of them it
wraps to the other end as `first` or `last`, which also takes the Tab out of its group. Beside a Tab
of another group one step changes membership before order: a grouped Tab leaves its group where it
stands, and an ungrouped Tab joins the neighbouring group where it stands, or passes the whole group
when that group is collapsed. The Client computes none of these results. It names the Tab, the
direction and whether the neighbouring group is collapsed, and installs what the Server's rule made
of them.

Settlement is the setup transaction's. The receipt must be `tabs-configured` at the checked
successor of the baseline revision. At that revision the whole Snapshot must equal the baseline
with its context replaced by `configure_tabs`; a later revision passes the shared strict identity
validator instead. Admission allocates no attachment generation, and a surviving target keeps its
viewer, controller, terminal projection and historical Ready proof. A refusal shows the Tab
configuration's not-applied status, a commit whose Snapshot cannot be read shows the
committed-snapshot-unavailable status until a refresh installs it, and an unknown outcome is never
retried. No move-specific phase, notice or recovery record exists.

Only one Tab configuration is in flight at a time, so a press made before the previous move
installed cannot be submitted at once, and dropping it would lose a keystroke. Such presses are
serialized instead. A Move Tab press waits in the held-invocation slot described below while this
Client's own Tab configuration is in flight, or while a publication refresh or landing switch holds
the projection. Presses held together form one run of at most 32, and each press renews the run's
five-second bound. Every tick submits the oldest press once the change before it has installed, so
each request is computed from the order then installed and carries that revision: the run sends one
request at a time, in press order, and none of its own requests can refuse another as stale. A
change that is refused or whose outcome is unknown ends the run, so later presses never act on an
order the person did not see; its status says so. A committed change that cannot be shown ends the
run the same way. A run that outlives its bound ends with the localized busy notice, and a press
beyond the run's limit is refused with the same notice.

Tab ID and `AttachmentKey { generation, tab_id }` are authoritative. An index is derived from one
accepted cache vector and may be retained only while that vector is frozen. Cache replacement
prepares provenance by exact ID, assigns BindingOnly to new snapshot-only IDs, resolves each
phase-required active/desired/committed ID exactly once, then atomically installs cache/provenance
and derives the selected index. Only validated correlated Create overlays creator provenance.
No retained ordinal alone decides identity, cleanup, feedback, replay, provenance or rendering.
For `[A,B,C] -> [B,A,C]`, active A moves from index 0 to 1 and
`[Started(A),Unavailable(B),BindingOnly(C)]` becomes
`[Unavailable(B),Started(A),BindingOnly(C)]`; attachment identity does not change.

Fresh startup still joins the retained Open reference/outcome and selects current snapshot
`tabs[0]`: after moving B before bootstrap A, new Clients select B and existing Clients keep their
active ID. Failed/unavailable/unknown anchor outcome fails before Snapshot; a Started anchor with
nonattachable moved head fails at that head's AttachBinding. No fallback or scan is allowed.
Other live Clients learn committed changes through the ready-loop `workbench.publication.read`
probe, which routes one refresh hint through the same `Ctrl+b w` admission; there is no
terminal switch. The UI also registers one State-mode `workbench.snapshot.read` subscription before
its first domain read. The Server sends a record only when the snapshot bytes differ, so commits
that leave the Workbench snapshot unchanged, such as background terminal output, raise no hint.
A publication refresh is background work and never makes a user action unavailable: availability
consults only the Client's own refresh. An invocation that needs the single refresh slot while a
publication refresh holds it — a confirmed Workspace or Project Close, a conditional Workspace
close, a Worktree form or listing, or a Context step — is held for at most five seconds and
replayed through its ordinary entry point when that refresh settles, before the next publication
refresh is admitted. A Tab or Panel action of the current Workspace (maximize, Panel focus, Tab
selection or move, split, Panel move, new Tab) needs a settled projection, so a publication
refresh or the user's own landing Workspace switch holds it the same way instead of refusing it;
a Client control that addresses the same handlers is answered by the Client's state and is never
held. A Context navigation refused only by a publication refresh pauses further publication
refreshes for the same bound while it retries. While a held Context step, Panel focus move or Tab
selection waits, terminal input is refused with a visible notice rather than reaching the Panel
the user is leaving. An expired hold ends with the localized busy notice.
A `published` probe sets one coalescing ingress latch inside the still-open mailbox before any stale-result or causal-wait handling, so a dropped or stale Poll reply cannot
erase the demand. At the next stable point the UI captures that latch into canonical Client
state as bounded publication debt (`Clean`/`Pending`/`InFlight { late }`/`Parked`) exactly once.
`Busy` or `Full` refresh admission leaves the debt pending for a later fair tick rather than
dropping it; a definite admission failure parks the debt with no hot-loop retry until a new
accepted hint or an explicit manual refresh re-arms it. A setup flight that adopted the debt but
was definitely refused read no snapshot, so it returns the debt to `Pending` instead of parking
it; a refusal proving the captured revision is stale creates `Pending` demand even without a hint.
Admission checks run in mailbox order —
`Full` first, then generation and serial exhaustion, then the phase-dependent `Busy` outcome — so
an exhausted identity space retires the Client even while a draft or switch would otherwise
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

## Scope and owners

This record fixes the native Client consumer of the canonical OBWP `tab.create`, `tabs.configure`
`move-tab`, and Panel-tree projection contracts, plus the `Ctrl+b c`, `<`, `>`, `n`, `p`, `1` through `9`, `h`, `j`, `k`,
`l`, and `w` actions. It preserves the existing one sequential admitted Client carrier, one OBCS viewer, one combined input-and-resize controller at
most, bounded command/event storage, and TUI restoration boundary.

The Server-owned Workbench snapshot remains the only source of Tab order, labels, Panel and
Presentation membership, and terminal bindings. One native invocation owns only its cached strict
projection, local active Tab ID, attachment generation, navigator state, geometry observation,
terminal projection, and transient statuses. Local selection never writes or advances a Workbench
revision.

Transport delivery uses the shared multiplexed Client carrier and subscription reconstruction described in the [application owner](../README.md). This projection boundary does not itself own pin/group mutation, child Presentation creation, persistence, remote transport, terminal attachment reconstruction, hidden carriers or multi-viewer composition.

## Pure strict projection

The startup projector selects a real Workspace by canonical order, or an identity-less empty landing state when no Workspaces/Contexts exist. Current-context projection validates the exact Workspace/Context pair. All Contexts must have exactly one owner. The selected Workspace admits 0–256 ordered Tabs, groups and a pinned prefix. Every Tab preserves the complete binary tree and exact Panel, Split, slot, orientation, Presentation and binding identities. Unsupported membership rejects the entire projection; no member is silently filtered or selected as a fallback. Nonactive layouts remain canonical snapshot data.

The accepted reorder refinement replaces the legacy `validate_append_only_refresh` prefix policy:
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

Selected duplication of several Tabs is one retained Server operation, not a Client sequence: the
UI starts it and follows it by operation ID outside the command mailbox, and the copies install
through ordinary refresh without a Create barrier. The owning
[group contract](native-tab-groups.md#terminal-creation-and-duplication) describes the follow;
single-Create and recovery rules remain unchanged.

The projector owns no Tab-move expectation. A Tab move is a Tab configuration, and its receipt
check belongs to the setup transaction: at the commit revision the whole Snapshot must equal the
baseline with `configure_tabs` applied, and above it the shared strict identity validator admits
later supported history.

## Panel projection and local focus

The accepted `ClientPanelTree` is a strict, depth-first representation of the canonical PanelNode.
Leaf order is the order used by attachment state; the tree retains split orientation and both slot
identities so a later snapshot cannot silently replace a leaf or reshape a Tab. Every visible leaf
owns its own viewer attachment inside the shared per-Client HostViewers budget (8): the Client
keeps one pane record per leaf under a stable `PaneKey` — the original selection anchor plus the
leaf's `WorkbenchPanelId` — and each record's physical attachment is a `PaneRef` incarnation minted
when its attach target was issued. Polls continue for every admitted pane incarnation, so inactive
leaves keep rendering their own live frames. Input admits only the focused pane; resize admits
each visible controlling pane's exact incarnation. Read-only panes never acquire control for resize.

In a connected Workbench Client, `Ctrl+b` followed by an arrow key, or by `h`, `j`, `k`, or `l`,
is a Client-local focus action; help and menus show the arrows first. When a
direction has a neighboring leaf, the Client moves the focused-pane selection to that leaf's
existing record without detaching, re-attaching, or re-tagging any already accepted work — no
switch cleanup, snapshot, or attach chain runs for a same-Tab move. A focus action never writes a
Workbench revision and never submits
`panel.split`, `panel.close`, or `panel.move`. A single
Panel or an edge with no neighbor is a local no-op. The renderer divides the canonical tree and
draws each leaf's own live frame; the focused leaf owns control input. When a leaf vanishes its
pane record departs and converges to canonical disposal, and a re-observed leaf mints a fresh
incarnation — late input, frames, feedback, and settlements tagged with the retired incarnation
are fenced at the exact `PaneRef` and never land on the re-bound attachment. A peer-created single-leaf split is accepted by the ordinary
refresh path; the Client preserves the selected Panel by identity while reindexing the cached tree
and retained provenance (`peer_panel_split_refresh_preserves_active_panel_identity`).

The provisional `Ctrl+b -`, `_`, `\`, `|`, `Shift+X`, and `Shift+H`/`Shift+J`/`Shift+K`/`Shift+L` bindings submit the three
structural panel commands through the same mailbox mutation admission, typed settlement,
pending-commit retention, and publication-refresh recovery used by Tab operations; the bindings
are provisional pending the input-design review and may be rebound. `-`, `_`, `\`, and `|` submit
`panel.split` on the focused leaf in the Below / Above / Left / Right direction — the glyph set
follows the input-vocabulary draft table — assign the next available numeric creation label, and use
the Client's real controlling cell/pixel geometry rather than a fabricated size. `Shift+X` arms the
leaf-scoped panel-close confirmation on the focused leaf under
[native panel close confirmation](native-panel-close-confirmation.md): only a drawn, still-valid
confirmation admits the ordinary `panel.close` on the frozen leaf — `Enter` confirms after the
complete draw and `Esc` cancels — and the confirmed close commits through the ordinary
publication path. Closing the Tab's last leaf removes the whole Tab — its terminals are disposed
and the Tab ends for every attached Client, the same blast radius the `x` Tab close confirms
against — and the drawn confirmation names exactly that impact before any `Enter` can take effect. `Shift+H`, `Shift+J`, `Shift+K`, and `Shift+L` submit `panel.move` on
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
Client Connection, viewer, Worker generation, and Host terminal
([`schemas/client-server/README.md`](../../../schemas/client-server/README.md)). Holding terminal
input or resize control never authorizes a Workbench mutation, and a read-only attachment cannot
input or resize regardless of any Workbench grant.

## Startup

An empty selection creates no terminal or attachment. Startup retains its existing startup-wide deadline and never obtains selection from an implicit Open result. Directory creation and recovery follow their owning contracts rather than introducing another terminal creation path.

Invalid projections, expired event placement or UI settlement, and attachment-generation exhaustion
cannot partially install selection. Already issued request identities stay consumed. Duplicate or
superseded bootstrap replies cannot allocate another generation or attach again.

## Presentation scheduling and retained pixels

The Windows input-to-echo path is: the inherited console's single record reader → byte decoder /
input dispatcher → bounded command mailbox and executor lease → local transport / Host PTY →
validated terminal projection → UI draw → causal feedback. A worker publication used to wait
for the shell's input-only 50 ms wait even when the console was empty. Switch setup repeats this
handoff for snapshot, detach, attach, configuration, frame, feedback and readiness, so successive
UI handoffs could each incur the wait. This is a scheduling cause, not evidence of slow PTY echo.

`ui_wakeup.rs` binds one coalesced notification to the active UI thread. Mailbox event
publication, lease release and closure signal it. The Windows console reader blocks on the
console handle and an `InputWake` together, so input and a publication each end the wait at
once. A reader with no bound wake parks for at most 1 ms between console probes. Existing pending records, UTF-16 state, repeat tails and bracketed-paste
bytes remain owned by the same reader, with the existing 4096-byte/record and 4 ms burst bounds.
The reader takes every record already queued, up to 256, in one console read and keeps those it
has not yet decoded for the next burst; the 4 ms bound starts once a burst's first record is in
hand, so the read that brought it does not count against the rest of its batch. Each console call is a round trip to the console host, so
on a busy machine one record per call let the 4 ms bound end a burst after one character: a mouse
report then reached the decoder one byte per frame and a click landed a second or more late.
Replacing or leaving a surface unregisters its waiter. Notification carries no input, output,
request identity or acknowledgement: the ordered mailbox and its single causal cell remain
authoritative. Unix polls the input descriptor and the notification's pipe together, with the same
effect. Transport framing and its backpressure are unchanged.

Terminal output reaches this scheduler as a hint. The Server's `ViewerRecordReady` names a viewer
holding something to Poll; the connection actor records it in the mailbox and wakes the UI, which
polls that pane at once, the focused pane first. A hint is advice: it moves no record and grants
nothing, so a pane whose hint was lost recovers through a fallback poll every 250 ms, shared among
the projected panes in rotation. That fallback also carries the Worker-loss and publication
observations an ordinary poll makes. A record on the UI's own snapshot subscription wakes it the
same way, so a change made outside this Client shows without waiting for a poll or the outer ceiling.

Two cases keep the earlier timer. Before this connection has delivered its first hint, and while any
pollable pane is still owed a full frame, whose hint can arrive before the projection that would
recognize it, empty ordinary poll replies set a monotonic 8 ms earliest-next-poll deadline, which
an admitted input clears. In
every case the shell waits for the shorter of the next deadline and its 50 ms outer ceiling, and
repeated notifications cannot admit another empty poll early. The scheduler still permits only one
ordinary poll in flight. Windows pipe readiness is event-driven, as
[Local IPC](../../../crates/local-ipc/README.md) describes, and adds no cadence here.

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
membership changes prune it, faults suppress it, and Client retirement releases all images.
Ordinary projection cleanup during switching preserves only this bounded pixel cache.

## Refused projection records

A projection record the Client refuses never retires the Session or the connection; it stays with
its pane. The record is dropped. A program effect (bell, clipboard write) changes nothing else.
Any other refused record invalidates the pane's frame and its frame proof, and the pane waits for
a full frame. A refused frame whose route is still readable is answered with `NeedSnapshot` naming
that frame's sequence, which is the Server's current flight. Any other refused record sends no
feedback, because the Server holds no flight for it; the next frame then requests the snapshot
through the ordinary unappliable-frame path.

Each pane counts its resyncs since its last applied frame. A refused record that would exceed three
resyncs, or one the Client cannot read at all (it may be a frame the Server still waits on), stops
that pane alone: its frame and retained pixels go, it takes no further polls, a result already in
flight for it is dropped, and it shows `Terminal disconnected`. Other panes and the Session keep
running, and a new attachment of the pane starts with a fresh count. Every refusal writes a
`client-record-contained` log record with the record family, validation stage and containment
(`dropped`, `resynced` or `stopped`). A stop always writes; drops and resyncs write at most once a
minute per stage. Answers that break the viewer contract still retire the connection.

[`ProjectionRecordResync`](../formal/tla/ProjectionRecordResync.tla) checks that every answer names
the Server's flight, so a refused record never escalates to refused feedback and a released viewer;
that the resync count stays bounded; and that a running pane always answers the frame the Server
waits on. Its mutation controls remove the stop on an unreadable record and the bound.

[`RequestScopedUnknownOutcome`](../formal/tla/RequestScopedUnknownOutcome.tla) models the
request-scoped unknown outcome of workspace close, tab close and tab rename on one live multiplexed
carrier, with two requests in flight, a competing close, a retry after an unknown outcome, a typed
Server answer, a stream-scoped unknown, a timer, a Client keypress, a publication, a transport loss
and a protocol violation. Each design flag has one mutant that TLC rejects. The model checks that a
stream-scoped unknown never replays its request identity and never retires a live carrier; that a
typed Server answer (`Unavailable`, `Denied` or an unknown status) keeps the close fence and its
evidence held; that at most one close is in flight; that only a Client keypress clears an unknown
notice; and that an owed publication read on a live carrier is eventually performed. Only a
stream-scoped unknown releases the close fence and leaves a shown Tab interactive; transport loss
and protocol violations still retire the Session. A startup-directory launch whose outcome is
unknown retires the carrier, because the client has no surface yet to settle it before bootstrap;
this model does not cover startup.

## Client state and bounded mailbox

`WorkbenchClient` owns the typed state shared by the native Windows and Unix shells. Platform
terminal-device and transport adapters own console/TTY waiting and I/O. `client_view.rs` contains
protocol-free key/overlay/notice decisions; Client and mailbox code retain protocol and attachment
authority. The scheduling change above does not introduce a second input reader or transport owner.

`BindingOnly` means the Client learned a canonical binding without creator-known terminal outcome.
Started and BindingOnly Tabs may be attached. Pending/unavailable/failed/outcome-unknown results
are retained by exact Tab ID as stable non-attachable provenance for this Client Connection. Refresh
does not erase that knowledge. Cold-start projection and every refresh merge provenance by exact Tab
ID: retain every existing entry unchanged; assign `BindingOnly` to each snapshot-only ID not already
known; and, after created-member validation, assign the just-returned ID its correlated Create
result provenance instead of the snapshot default. A switch-time definite Attach failure for an otherwise attachable
Tab is instead retryable through a later explicit same-Tab selection.

Input admission remains byte-bounded inside the same eight-record FIFO. An admitted Tab close
refuses Input and viewport steps only for panes of the Tab it names, from the queued exchange
through its status reads to the membership reconciliation that follows; Input and viewport steps
for any other Tab queue behind the close records and reach the worker in order once those leases
finish, so typing into the focused pane survives a background close. The UI refuses the close
target's own focused pane before the mailbox while the close holds an operation. Polls, Resizes
and selection records still wait until no close step is outstanding. After the ordinary closed,
bootstrap, close-target and focused-pane gates pass, a new Input may append under the
mailbox mutex only to the queued tail Input for the exact same `PaneRef` (including incarnation),
with no retry deferral and a combined payload of at most 4096 bytes. It never mutates the active
worker lease. The append preserves the tail record serial, so `try_enqueue` returns a queue-record
identity that can repeat across Input calls; it is not a per-call request receipt. The worker later
rechecks the exact attachment and controller, then submits one Input request whose acknowledgement
covers the exact combined byte count. A different pane incarnation or any Poll, Resize, Viewport,
recovery, selection or structural record is an ordering boundary. Payload overflow starts a new
record when a slot remains; when all eight records are occupied and the tail cannot accept the
bytes, admission returns `Full` without claiming delivery.

There is exactly one request-identity allocator for the Workbench Client. Move the existing cursor from
connection-worker ownership into this mailbox; never clone its seed or counter. The shell and worker
hold the same mutex only long enough to check and commit one next ID and never across I/O. Create
admission checks phase and capacity, computes the next generation and request ID without mutation,
then appends its complete marker and commits both cursors as one critical section. Queue/generation
refusal occurs before the ID commit and burns no ID. Once appended, both values are burned even after
a precommit refusal. Identity overflow appends nothing and burns no generation; because no future
OBCS or OBWP request can be correlated, it closes the mailbox and retires the carrier as a definite
local pre-handoff failure instead of reporting a retryable busy condition.

The shell always records the latest measured valid geometry, even for a Ready ReadOnly viewer and
when terminal Resize admission is paused during Creating or Switching. A per-pane report also
publishes the focused leaf's measured geometry to both Client state and the setup mailbox before
ordinary admission guards. Background leaves never replace that focus measurement, and an omitted
focus leaves the last valid measurement unchanged. Reattaching a Tab therefore starts with its
actual content geometry, not a stale startup rectangle that would resize the PTY down and back up.
This observation changes Client-local layout data only; it does not authorize an OBCS Resize. The
Client separately tracks last admitted/applied controller geometry. A Create marker captures the observed geometry
at its atomic admission; a later successful Acquire begins with a Resize carrying the then-latest
measured geometry and uses the setup catch-up rule below for subsequent observations. A precommit refusal resumes the old controller and, if measurements changed,
admits one catch-up Resize before polling resumes. Read-only viewers never send Resize.

Structural flows share one recovery fence table in `crates/workbench-client/src/workbench_state/recovery_fence.rs`.
A known Panel or Rename commit whose canonical projection has not installed yet keeps a
recovery record, and an uncertain Rename keeps its operation. Another mutation waits for every
projection recovery. Context removal and Workspace Close also wait for an uncertain Rename. Context
selection and retired-membership repair wait for Rename recovery and an uncertain Rename,
but not Panel recovery. Workspace setup authority waits only for Rename recovery. An
installed canonical refresh resolves every projection recovery at once. Flows name their fence
instead of listing records, so a new recovery kind is decided in that table once.

## Local selection

Selection resolves only against the latest applied strict cache. `n` and `p` use the local active
target's index and wrap modulo the Tab count in both directions. `1` through `9` are one-based exact
indices; an absent index is a local no-op with an out-of-range notice. Zero Tabs have no selectable
target.

For a one-Tab cache, n/p resolve to the same member: Ready is a no-op and
RetryableAttachUnavailable is an explicit retry only when no Refresh is pending. While Refresh is pending,
the same-target no-op guard applies in all otherwise selectable phases. A full-queue or out-of-range action does not
dismiss an already open navigator. Panel keys move local focus through the active Tab's split tree;
they are a no-op for a single Panel or when no directional neighbor exists. The provisional
split/close/move bindings described under Panel projection admit one structural panel mutation
against the focused leaf under the same phase, queue, and fence rules as the other structural
submissions.

Exact-Panel selection owns one narrower admission of its own. When a selection names a Panel
whose pane viewer is dead while the Tab's selected attachment is still live, recorded
dead-target-pane evidence lets it mint a fresh marker and generation for the same desired Tab.
The ordinary switch pipeline then detaches and re-attaches every viewer in that Tab — an accepted
whole-Tab cost, because one attachment epoch owns the Tab's viewers — before the selection lands
on the exact Panel. The evidence gate is what separates this from the ordinary same-target no-op:
it never manufactures another identity, and the admission does not exist without recorded
dead-target proof. The admission exists in Ready only — a selected non-attachable Tab cannot
hold dead-viewer evidence for an attachable desired target. A failed reattach settles through the
ordinary switch failure machinery.

## Create is a draining FIFO barrier

`Ctrl+b c` is admissible only from a settled Ready identity: carrier live; active, desired,
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
Sidebar overlay changes remain local; they do not admit terminal or Workbench effects.

Creating rejects further Create, switch, refresh, user input, and effectful Resize. Resize
observations still update latest measured geometry. It stops scheduling new old-target Polls. The
UI continues draining the bounded event channel and may integrate and draw an already accepted
old-generation Poll result while visibly reporting Creating.

The same drain includes a background pane's earlier accepted Resize. During Creating, its exact
live, attached, nondeparting `PaneRef` must still belong to the retained Ready anchor and the anchor
must remain desired; the returned controlling grant must match that pane's own grant. Settlement
updates only that pane's accepted geometry and invalidates its old frame proof. It does not change
focus, the Create identity or the cache, nor admit new Resize work. The executor's existing lease,
exact Resize response validation and event/reply slot prove the outstanding operation; the reducer
must not compare an earlier FIFO result with a newer observed or admitted geometry.
`CreateCommittedNeedDetach` grants no such background allowance, because the prior Resize and its UI
reply finish before Create submission.

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
undrawn frame. A pre-marker Poll that finds the focus runtime gone instead disposes the viewer and
parks the focus beside the barrier, as [panel lifecycle](native-panel-lifecycle.md) states: the
barrier keeps its operation and holds the unattached anchor, a definite precommit result restores
the parked selection, and a committed result cleans up only the live siblings.

The first `tab.create` submission uses its one captured request ID. There is no automatic new-ID
retry, fallback OBCS Create/Open/Start, cancellation claim, or inference from a newly visible Tab.
A later human `c` after a definite precommit refusal is a distinct operation. While a Create remains
inside its barrier or committed-projection recovery, no later `c` is admitted. Once it has settled
and selection is again Ready, a later human `c` is a new operation with a fresh request ID.

The result transition is:

| Result                                                                                                                                                                                                    | Transition                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Definite top-level denied/rejected/unavailable or proven not-submitted                                                                                                                                    | Preserve the exact old viewer/controller, active ID, projection, and valid feedback; atomically return to Ready with the event-driven precommit notice defined below, issue one catch-up Resize if controlling and needed, and resume Poll. Do not refresh or retry automatically.                                                                                                                                                                           |
| `tab-create-pending`                                                                                                                                                                                      | Retain the committed tuple/revision/provenance and reserved generation 1. Read and validate a fresh Snapshot before old-viewer cleanup. Freeze the exact canonical candidate, then complete exact old cleanup and atomically install the candidate. Creating remains visible until installation; the created Tab is never attached in this cohort. Preserve the request identity; do not submit a new identity.                                              |
| `tab-created/started`                                                                                                                                                                                     | Retain the committed facts and reserved generation 1; read and validate the fresh Snapshot; freeze its exact candidate; only then complete exact old cleanup and atomically install the cache-backed active selection. If the returned member survives, AttachBinding uses its snapshot-derived binding and generation 1. If a later snapshot removed it, the frozen ordinal selects the surviving candidate at generation 2, or Empty; do not resurrect it. |
| `tab-created/unavailable`, `/failed`, or `/outcome-unknown`                                                                                                                                               | Retain stable non-attachable provenance and generation 1; read and validate the fresh Snapshot; freeze its exact candidate; only then complete exact old cleanup and atomically install the matching selected-without-attachment state. A removed returned member remains historical and uses generation 2 for a surviving fallback. Send no Attach.                                                                                                         |
| Any possible OBWP handoff followed by write/read timeout or failure, malformed/truncated/wrong-kind/miscorrelated/action-incompatible result, wrong committed context/revision, or wrong started geometry | The command outcome is unknown. Retire the mixed carrier and discard unsent commands. Do not infer, retry, refresh, detach on the ambiguous carrier, or select a returned identity that was not validly correlated.                                                                                                                                                                                                                                          |

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

This handoff covers expected typed errors and cooperative closure, including closure between the
adapter return and event publication. An unexpected worker panic terminates its owning failure
domain: the existing exit guard closes the mailbox and the owner joins, but an unwound thread does
not promise return of its pending stack value. No generic panic recovery is introduced.

Ready-origin staging then licenses cleanup of only the exact old attachment. Exact correlated OBCS
`Detached` proves it. An opaque retained known-Closed fact may prove the same result only when it
matches the exact old Tab/binding/generation and owner cleanup is complete, and only before a Detach
has been handed off. Raw snapshot absence is never that proof. Definite `ServerBusy` retries only
inside the operation deadline. A definite non-busy rejection/unavailable, total expiry, or any
ambiguous handed-off Detach retires the carrier. Empty-origin staging skips cleanup because no old
viewer exists.

A definite Snapshot denied/unavailable before candidate staging enters `ProjectionUnavailable`.
It retains the committed tuple, revision, terminal outcome and reserved generation 1 together with
the exact old validated cache, viewer/controller, active selection and projection/feedback. Those old
facts are diagnostic only: the structural fence remains active, so Input, Resize, ordinary Poll,
selection, Create and Tab moves are unavailable. `Ctrl+b w` may admit the explicit recovery below; exit
remains available. A valid recovery stages a candidate before cleanup and installation. A
valid-but-inconsistent snapshot is authority divergence and retires the carrier, as do malformed,
miscorrelated, or transport-ambiguous responses; none licenses a guessed Detach.

For Started recovery through a later Refresh, the original Create deadline and finished Refresh
transport deadline are not reused. After pure projection and created-member validation, the UI
samples the shared operation clock before taking the mailbox lock. Recovery atomically installs
`total = sampled_at.checked_add(50s)` and `forward_cutoff = total - 5s` with the attach
continuation. This fresh, conservative setup budget covers Attach, Acquire, Resize, Poll, actual
draw, feedback and exact cleanup; progress never renews it. It uses the retained Create attachment
generation, which has not yet attached, while every exchange still allocates a fresh request
identity from the single cursor. It neither resubmits Create nor admits a user Switch marker. If
deadline arithmetic overflows, consume and clear that exact pending Refresh as a refresh failure,
retain ProjectionUnavailable and the pre-recovery cache, committed facts and all identity/generation
cursors at their post-SnapshotRead values, send no Attach, and keep the carrier reusable. The
already issued Refresh request identity remains burned. Recovery of known non-attachable provenance
needs no setup deadline.

## Switch setup and supersession

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

A timely definite Unavailable result restores only an owner-held anchor from a genuine stable
origin. Ready restoration covers every live nondeparting pane in the original anchor, not just its
focus. Each pane requires the same exact attachment incarnation, binding, viewer/controller,
observed/admitted geometry, retained UI projection and accepted full-frame/draw/feedback proof after
truthful pre-marker settlement. Preparation is effect-free; commit restores all captured projections
together without Attach, input replay or a fresh generation. The owner-minted restoration proof also
publishes those exact pane incarnations into the mailbox in the same desired-install transaction,
so ordinary Poll and input admission can resume without a new reconcile. Already departing panes
remain departing. A missing or unfinished pane proof, such as a frame proof that a Resize
acknowledged before the Switch cleared, or a controller mode that changed while the Switch was in
flight, as when the Host refused a Resize admitted before it, prevents restoration; the Switch then
parks its origin as described below. Abandonment of any captured pane invalidates the anchor.
Unpreservable old Poll/causal abandonment, or viewer cleanup or replacement invalidates that
proof. The anchor follows those facts; an admission-time clone cannot roll them back. A Resize
acknowledged while the Switch is in flight does not. A controlling pane whose
Resize the Host has not yet acknowledged at its observed geometry is retained with its geometry
unknown. The executor runs a Resize admitted before the Switch ahead of it, so that
acknowledgement, for the old focus or a sibling alike, settles while the Switch is in flight: it
leaves the retained pane's geometry unknown, keeps the anchor and settles as superseded. A restore
clears the admitted geometry of each pane whose geometry is unknown, and the next observation
re-sends its Resize as it resizes any visible pane. A Host resize is idempotent, so the re-sent
Resize needs no proof of what the Host applied. A window observed at another size while the Switch
is in flight does not invalidate the anchor either: no Resize reaches the departing panes, so the
restore returns the focus to the geometry it last observed, and the next observation resizes the
restored panes the same way. A recorded selected-without-attachment or retryable origin may restore only
while it still has no viewer/controller, except an unbound selection or a retryable focus with
live siblings uses the same exact whole-anchor projection proof described below; a retryable one
restores still retryable. A partly completed Switch never creates a stable anchor;
in particular, a second marker can have no anchor after the first marker's not-yet-Ready viewer was
cleaned up, or after the first marker's cleanup detached the origin's viewers.

A refused Switch never retires the Client. This covers a definite Denied or Unavailable result, a
candidate the action menu no longer accepts and a snapshot that removes the context. Without a
usable anchor, the Switch parks its origin. The origin is the selection active at admission. A
Switch admitted while another one awaits its snapshot inherits that one's origin, and one admitted
while an installed selection is still attaching returns to that selection. When viewers remain
under the origin's anchor, the Switch first detaches all of them through the exact old cleanup an
expired Switch uses, and their records drop. That cleanup is not an expiry: its detach may land any
time within the total reserve. The origin is then selected without a viewer in the same
desired-install transaction that returns the mailbox to it, with no pane incarnations. Its focus
record is revived or freshly minted. An attachable focus
without a definite attach fault parks as a lost viewer, so the bounded automatic re-attach of
[native panel lifecycle](native-panel-lifecycle.md) recovers it after its pause; a definite fault
keeps its parked state, and a focus that never attaches is selected without attachment. The
origin's siblings attach again through reconcile once an observation recreates their records. A
snapshot that removed the context also owes the publication read on this path. Real protocol
violations on either path still retire, as do an expired Switch and an Available candidate that
does not project.

Superseded results only settle their old flight. Exact restoration may return desired to the old
allocated key, while the allocation cursor never rewinds and the intent generation remains burned.
Timely Unavailable placement ends its transport budget, so exact UI settlement may occur later;
it starts no forward work and has no synthetic post-deposit deadline.

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
abandoned directly.

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

## Ctrl+b w navigator and refresh

Until the UI consumes that one result, a differing-target n/p/number selection is locally refused
and Create remains inadmissible; a same-target selection remains its ordinary no-op. Therefore no
refresh completion is applied during Switching, and every admitted Switch copies its target and
binding from a fully settled cache.

The navigator is informational. While it is visible, ordinary terminal byte batches continue
through normal Ready/Controlling admission; ReadOnly and barrier states apply their existing
rejection rules. The ClientView never treats a raw Escape byte as a UI event and never splits an
arrow, Alt, or other terminal sequence to implement dismissal.

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
Refresh applies the strict projector and ID-authoritative replacement rule.
Lower revision, same-revision difference, missing/duplicate old ID, changed immutable tuple/binding/
Presentation label, Session/context mismatch or loss of the phase-required active identity retires.
Higher-revision supported reorder and Tab-label changes are valid. Success prepares provenance by
ID, atomically replaces cache/provenance, preserves active ID and derives its new index.
ProjectionUnavailable additionally validates retained create facts. No push or freshness between
reads is claimed.

## Rendering and localized observable states

Canonical labels are never normalized, trimmed, deduplicated, or truncated in Client data.
Presentation derives a temporary safe single-line string by replacing every Unicode control scalar
(including C0, DEL, and C1) and U+2028/U+2029 with U+FFFD. Ratatui lays out that safe string in a
one-line bounded `Rect`; buffer clipping, measured in terminal cells, never writes beyond it and
does not split a wide cell. When the compact strip width can hold the active token prefix `[*N `,
horizontal scrolling exposes that prefix in full while preserving canonical order. A narrower Rect
renders only its safely clipped leading cells with no overflow or control bytes. Navigator rows
vertically scroll to keep the active row visible.

`FixedViewportBackend::draw` is the final trust-boundary guard because it calls
`Print(cell.symbol())`. Before `Print`, it applies the same forbidden-scalar replacement to every
cell symbol. No control/newline/C1 byte sequence from a Workbench label can become a terminal
command.

Ordinary control is silent: a Ready Controlling Tab shows no status at rest and ordinary terminal
passthrough has no status bar, while view-only input stays explicit at rest. `Ctrl+b` replaces the
status with prefix hints and does not reveal readiness. The local-busy row applies to every
structural admission refusal, including a Move Tab key the Client could not admit.

Refresh statuses render inside the open navigator. They are retained but visually silent while it
is closed. PTY observers read the resting frame: one active Tab token, and for control the absence
of the view-only status, or for view-only input its exact string. The first frame can be drawn
before the Client reaches phase `ready`, and input in that window is refused with a notice, so a
journey that types also waits until `client list` reads the Client's phase `ready` on consecutive
reads.
A transient Creating/Refreshing draw is never required.

A definite Create precommit settlement never creates a timer-driven phase. The UI consumes the
typed result by restoring the exact attachment directly to Ready and storing the localized notice
in `Ready.notice`; that notice opens the status overlay until the next decoded
`Ctrl+b` Client action atomically clears or replaces it. Ordinary terminal bytes, Poll/frame
traffic, and physical Resize observations do not clear it. A Pending committed target remains
SelectedWithoutAttachment with the exact Pending status. `Ctrl+b w` can refresh canonical
membership, but it does not replay Create, attach that Tab, or erase its retained Pending
provenance.

## Rename Unicode draft buffer

Empty is valid while editing. Producing a label calls the existing `WorkbenchLabel::parse`, so
Empty cannot submit and whitespace-only text remains valid. Whitespace, normalization form,
combining scalars, supplementary scalars and schema-valid controls remain exact; display
sanitization never modifies the buffer. Removing a scalar is not a grapheme-deletion or physical
Backspace contract. A returned label owns its text independently of later buffer edits.

## Native Rename consumer

`Ctrl+b F2` opens a state-owned draft for the exact selected Tab in the active context in a live stable phase.
Capture the canonical baseline, Session/context/Tab and attachment generation without allocating a
request or deadline. Empty, structural/recovery flights and Close confirmation refuse entry.
The draft and Close confirmation are mutually exclusive. Applied authority or selection changes
invalidate the draft; Enter revalidates the captured tuple atomically and never retargets its text.

The draft preserves exact schema-valid text and uses the bounded `RenameText` operations. Initial
successful drawing of its exact opaque token is required before Enter. Missing/failed drawing,
hidden editor, viewport invalidation or draft replacement revokes qualification. Later text edits
do not require a draw per scalar. A returned label remains exact; display sanitization is separate.
Input faults clear only on an explicit decoded deletion — Backspace, Delete, or a shared kill key
such as Ctrl+u — including Empty; these still pop or clear. Successful typing/paste/yank, word
movement, ignored sequences, failed edits and Enter cannot clear a fault.
The localized error names Backspace and Ctrl+u. Cancellation destroys the draft and its fault.

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

`Unchanged` requires the baseline revision and equal requested text, with no SnapshotRead.
`Renamed` requires changed text and the checked successor; retain the exact operation and known
revision before one same-budget SnapshotRead. The result has no target echo. Exact revision
requires the label-only projection; later strict membership may overwrite or remove the target.
Definite command precommit clears the exact flight after UI settlement. A request-scoped unknown
rename outcome on a live multiplexed carrier settles without a recovery fence, owes a publication
read so canonical publications synchronize the label, and shows the persistent unknown-outcome notice
until the next keypress without replaying Rename. Only a disconnected ambiguous exchange or postcommit
snapshot unavailability retains a recovery fence/historical cache (`Explicit Refresh` never replays Rename).

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
`session-mismatch`, `revision-mismatch`, `terminal-context-unavailable`, `resource-limit`,
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

The deadline origin of an admitted operation is the clock sample taken before acquiring the mailbox lock. A private,
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
the exact event key and fixed deadline under the acquired guard. A narrowly scoped `yield_now` retry
handles mutex contention; queue, event, and Busy waits continue to use the condition variable. A
pre-lock sample followed by blocking acquisition cannot authorize settlement after ordinary mutex
waiting crosses the deadline. Scheduler suspension after the final sample and kernel stalls remain
environmental exclusions; ordinary lock contention cannot renew or extend the budget.

Snapshot reads remain non-effectful, but malformed/wrong/correlation/transport failures also retire
the shared carrier. Definite denied/unavailable may leave it live. All IDs/bindings are converted
between OBWP and OBCS with checked exact textual parity; no protocol crate depends on the other.

## Retired peer membership recovery

This entry is for a non-initiating native Client whose exact current ordinary Poll returned
`TerminalRuntimeUnavailable`. It retains that correlated request and attachment key, immutable
binding, canonical Session/context baseline and historical ordinal. Stale or setup Polls, transport
uncertainty, malformed replies and other failures do not establish this origin. Activation requires
successful exact old-task join, closed old transport and settlement of all returned facts. Panic or
missing return evidence cannot establish eligibility. Any retained Close/Read record or flight, or
unresolved structural mutation, commit, setup or cleanup handoff excludes this entry. Existing
Close status recovery retains priority and all its facts.

The same correlated `TerminalRuntimeUnavailable` on a Ready focus is pane-local beside every
owner, as [panel lifecycle](native-panel-lifecycle.md) states: each owner completes against the
parked focus, and a pending takeover settles as refused. The lost viewer is disposed and the exact
selection is precisely parked for the existing publication reconciliation, so the loss does not
establish this retired origin. A loss whose typed notice never reaches the UI still does. The worker records the Poll before it disposes
the viewer. When it cannot place the `ViewerLost` notice before the Poll's cutoff it exits with
`Deadline`, and a later worker failure can close the mailbox before the UI takes a placed notice.
If the Client then retires while the focus is still attached and that Poll is still the UI's
ordinary Poll, a clean join establishes the origin.

An explicit `Ctrl+b w` after finalized retirement may start one Snapshot-only connection through the
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

Before installation the only request is SnapshotRead. The full candidate must preserve
Session/context, nondecreasing revision, exact same-revision membership and every immutable retained
identity tuple. The old unavailable Tab must be absent, or its selected Workspace Panel must now
carry both a fresh Presentation and a fresh terminal binding under the replacement rule above. A
surviving old physical identity, unavailable Snapshot or invalid candidate leaves the old canonical
state and origin parked, with no Attach. Valid replacement keeps the selected Tab and Panel, mints a
fresh attachment generation and clears old launch provenance. Absence or replacement is membership
knowledge, not cleanup evidence. The existing explicit `Ctrl+b w` entry also covers this
replacement; it does not reconnect automatically. Bootstrap, Open, activation, Create, Start and
Close are not part of this observation entry.

While an armed attempt owns the mailbox and before any candidate installs, the Client
admits no ordinary scheduled work against it: the post-event tick scheduler block
(ordinary poll reconcile, publication refresh, close reconciliation, close read, pane
reconcile, and poll), the observed-pane geometry Resize admission, and the pane
reconcile enqueue itself are suppressed by the exact attempt marker, not by Client
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

## Context removal recovery

A workspace Close owned by another Client removes this Client's current context from canonical
membership. The first visible fact is an ordinary Refresh reply whose validated navigation no
longer contains the current context id while the preferred surviving Workspace still projects —
or, for an already retired Client, the same shape inside one bounded recovery attempt. A Switch
whose snapshot shows that shape does not land: it restores its selection, and the snapshot itself
owes one publication read, so the next publication Refresh classifies the removal even before the
peer's hint arrives. Snapshot absence is membership knowledge, not cleanup evidence; nothing in
this entry relaxes the real Detach lifecycle.

Every candidate snapshot classifies before it commits. `Candidate` keeps the ordinary membership
path: the current context still projects. `Removed` requires the full conjunction — validated
envelope, unchanged Session, nondecreasing revision, the current context id absent from every
canonical navigation owner, the preferred surviving Workspace projecting, the removed context's creation provenance
carrying no `Pending`/`OutcomeUnknown`, and every structural guard quiet — no pending mutation,
commit, setup, cleanup handoff, retired attempt, in-flight Create/Close or live
Read/Poll ownership. Only `Removed` mints a
`ContextRemovalPermit{generation, removed, landing, revision}`; it lives as a bounded private
field inside the prepared candidate, never on ambient Client state, and is spent exactly once
by the install it authorizes. `Divergent` — wrong session, regressed revision, malformed navigation
or the old context id still present elsewhere — and every guard failure retire truthfully
without touching canonical state.

Landing prefers the first canonical Workspace in display order, including an empty Workspace,
then nonempty compatibility content. With neither, the required empty compatibility Context
represents no open Workspace and offers explicit creation. It is absent from ordinary navigation;
this projection changes no canonical IDs or terminal resources and never automatically creates
replacement content after Close. Missing, duplicate or overlapping compatibility membership still
diverges, even when that empty slot is not displayed.

With viewers alive the Client never installs first: it stages the exact membership cleanup
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

Without viewers the install gate already requires a clean pane set: no attached record — the
direct leg installs unchanged. Projection and frame evidence exist only inside a record's
attachment, so a record without a viewer cannot carry a stray `projected`/`staged`/`accepted`
fact.

`ReconcilingMembership` suppresses Ready-based input admission from staging until the commit
returns; no new gate or phase is added. An input admitted before staging carries its tagged
old-pane incarnation and settles only there; after the epoch commit drops old records the same
input misses its incarnation fence and is dropped — it is never re-tagged to the landing pane.

The retired entry is the existing `Ctrl+b w` recovery composition, not a new origin: only a
Ready-phase ordinary Poll that returned `TerminalRuntimeUnavailable` plus a clean old-task join
mints origin evidence. Its bounded attempt — one connection, one refresh flight, its own sampled
deadline — runs the same classify; a `Removed` result mints a generation-2 permit and installs
through the same candidate transaction with panes already empty, while divergence, expiry or
cancellation parks retired. Cleanup-failure retirement mints no origin: external release came
from the old Client and worker actually settling, not from clearing records.

## Automatic Tab naming

The [Tab label contract](native-tab-labels.md) refines the label facts above: projection retains raw text and naming mode, and display derives from context position. Create checks the requested mode at its commit; reordering preserves metadata. Rename clears automatic mode, so an equal-text automatic rename is a commit and only a fixed equal-text rename is a no-op. The strip, rename draft and close confirmation use the strip's display name for a live Tab.

## Background terminal titles

`workbench_client/presentation_rows.rs` owns the UI's `presentation-titles` State streams on the
shared connection, with no bridge thread, as the window-title stream does. Each control tick polls
them with the bound Presentations of the latest accepted snapshot, in the canonical order the
Agent index keeps; they open once the control observation names the Session, close when the
Machine suspends or the Client stops, and keep no title. A record that does not answer its request,
or answers with another outcome, closes them all, since a Server of this cohort sends neither.
`ui/presentation_titles.rs` applies the [shared naming rule](native-tab-labels.md#background-terminal-titles):
one cursor per Presentation of the latest snapshot, created by its first frame title or row and
dropped when a snapshot no longer holds the Presentation, so each holds at most a few projected
titles of at most 1 KiB. A frame title is recorded after each successful draw; before that, the
view shows what recording it would show. A frame title is compared after projection, so a
spinner frame in an activity prefix is no arrival. Cursors belong to one Session, ride in the
retained bank across a targeted surface replacement, and are released when the Client retires.
Naming no longer reads retained frames, which keep only pixels.

## Agent confirmation rows

The same module follows the Server's `confirmations` rows, one per bound Presentation, on State
streams opened only once the Session's handshake answer lists `agent-confirmation`; the question is
asked again every 5 seconds while the answer is unknown, and another Session drops the streams and
every row. `ui/agent_confirmations.rs` keeps the latest row for each Presentation with the
terminal it was read for, and a row for another terminal than the Presentation now shows names no
Agent. The Server owns every value. A choice the person makes is sent as `confirmation-choose` or
`confirmation-decline` with the row's terminal as `expected_binding`, one request at a time with a
10 second reply window; the choice shown changes only when the Server's answer or its next row
reports it, and a request that was refused, lost or not answered says so on the status line.

What the rows say to the person:

- **Offer.** While the Server offers and no other notice shows, the status line asks the question
  and names the key that answers it. The subject is the focused Panel's Agent, else another Agent
  the active Tab shows.
- **Allowed.** A press is said once, for 4 seconds, only for a terminal the active Tab shows, and
  never when it was made before this Client first read that terminal's row. A newer press replaces
  an older one still showing; no press replaces another notice.
- **Stopped.** Said once for a terminal in any Tab.
- **Why it waits.** The Agent row's state tooltip adds the row's reason after the state.

The [action menu contract](native-action-menus.md) owns the menus that answer, and the
[Server reference](../../orchbee-server/docs/agent-confirmation.md) owns what is pressed and when.

## Native grouped and pinned Tab projection

The selected-context projection retains every canonical group record, including unused groups,
and each Tab's pinned flag and resolved group metadata. Bounds, unique group IDs, valid labels,
existing group references, ungrouped pinned prefix and contiguous group membership are required;
invalid metadata refuses the complete selected projection without filtering terminals. Nonactive
context trees keep their independent admission boundary. Child Presentation navigation is refined below;
unbound selection is refined below, and children are not flattened into layout leaves.

Same-revision refresh and return to a retained context compare the complete group metadata. A
higher canonical revision may update group/pin chrome while preserving unchanged Panel/binding
identities, viewer/controller ownership, frame and history. Exact Create/Split/PanelMove/Rename
receipts also preserve unrelated group/pin metadata; a new Tab starts ungrouped and unpinned.
Workbench owns canonical metadata, Workbench Client owns selection and attachments, and rendering
only derives labels. No group metadata change alone starts or disposes a terminal attachment.

## Child Presentation navigation

A Panel remains one layout leaf and retains primary-first Presentation order, with at most sixteen
children. Presentation and bound Terminal identities are unique across the context; sharing their
own Panel is valid. Retained Presentation identities preserve Panel, label and binding, while the
existing Workspace primary replacement rule stays separate from child addition/removal. Exact
structural receipts preserve complete surviving member payloads.

Workbench Client owns a volatile Presentation preference per Panel. Live PaneRecord binding determines
the displayed member; departure records stable choices, and missing choices fall back to primary.
A local choice never edits Workbench revision or layout. Switching member uses a fresh anchor and
the existing Switch cleanup/attach owner, so old input, frames and controller grants cannot retarget.
Pre-cleanup refusal restores the previous live member; unavailable attach remains an explicit state.
Sibling viewers may be reattached by this initial complete-anchor cleanup. Child attachment uses
binding-only provenance and cannot inherit a primary terminal creation outcome.

Previous/next Presentation has no default key; the selection wraps in canonical order and is a no-op
with one member. Stable Presentation targeting serves Agent focus. Rendered label/count and
attention refer to the chosen binding; Panel movement and Close still operate on the complete Panel.
The report lane queries the selected Presentation and correlates its owning Panel and binding under
both revision fences. A local member change discards the old observation before publishing the
child's report or metadata. Header arrows provide exact member selection through the [native pointer
owner](native-mouse-input.md#presentation-selector).

## Unbound Presentation selection

A canonical Presentation may have no binding. Native projection retains its identity, label,
Panel membership and order; it never borrows another member's terminal or drops an empty leaf.
Terminal identity uniqueness applies only to actual bindings. At a higher revision, an unchanged
unbound Presentation may acquire a fresh terminal binding; existing bound identity and same-revision
guards remain strict. The Server remains the only canonical binding writer.

Client selection retains the exact Presentation ID independently of attachment. An empty selected
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

No new wire contract, persistence or physical process owner is introduced.

Native splits focus the committed new Panel. Both the initial validated snapshot and later recovery resolve the new Panel ID through the paired membership cleanup/install path. If the new Panel has already disappeared, normal surviving-leaf repair applies. Peer splits preserve local focus.
