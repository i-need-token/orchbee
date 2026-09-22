---------------- MODULE NativeClientPaneMembership ----------------
\* Bounded design witness for the pane-scoped membership collection that
\* replaced the single-attachment model: several logical panes share one
\* selection anchor, every queued command, poll key and worker event carries an
\* exact pane incarnation, and retired incarnations are fenced rather than
\* replayed onto a re-bound attachment.
\*
\* Abstract mapping to `terminal_session`:
\*   anchor        ~ AttachmentKey selection epoch (membership generation)
\*   pane          ~ PaneKey {anchor, panel} — a stable logical leaf identity
\*   incarnation   ~ PaneRef.incarnation — minted fresh on every record mint
\*   rec[p]        ~ PaneRecord for pane p (incarnation 0 = no record)
\*   viewer[p]     ~ the host-side attachment incarnation for pane p (0 = none)
\*   queue/inflight~ the command mailbox plus the executor's in-flight slot
\*   rec[p].pending~ a received poll frame waiting for its draw pass — the
\*                   mailbox causal settlement before stage_drawn_frame runs
\*   rec[p].staged ~ a frame that completed its draw and awaits FrameApplied
\*   appliedCount, ~ summary of the ghost log of delivered inputs: how many
\*   allTaggedValid   deliveries were recorded, and whether every recorded one
\*                   landed on the exact attachment incarnation it was tagged
\*                   for — proving no input lands on a stale or sibling
\*                   attachment. The full log's only readers were its length
\*                   bound and the per-entry tag = delivered check, so the
\*                   pair is the exact abstraction alpha(X, L) =
\*                   (X, Len(L), forall l in L: l.tag = l.delivered).
\*
\* Bounds: two panes, two anchors (one epoch switch), two incarnations per
\* pane, a bounded queue, two frame identities, and a bounded delivered-input
\* summary. The old `NativeClientPanelLifecycle` single-attachment invariants
\* remain the single-pane specialization and are not claimed as proof of this
\* collection.

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS MaxQueue, MaxIncarnation, MaxFrame, MaxViewers, MaxLog

ASSUME
  /\ MaxQueue \in Nat \ {0}
  /\ MaxIncarnation = 2
  /\ MaxFrame \in Nat \ {0}
  /\ MaxViewers \in Nat \ {0}
  /\ MaxLog \in Nat \ {0}

Panes == {"p0", "p1"}
Anchors == {"a0", "a1"}
NoRecord == 0
NoViewer == 0
NoFrame == 0
Frames == 1..MaxFrame
Incarnations == 1..MaxIncarnation

ControlKinds == {"control"}
CommandKinds == ControlKinds \cup {"poll"}
Idle == "idle"

Command(kind, pane, incarnation, focusAdmitted) ==
  [kind |-> kind, pane |-> pane, incarnation |-> incarnation,
   focusAdmitted |-> focusAdmitted]

CommandRecords ==
  [kind : CommandKinds \cup {Idle}, pane : Panes,
   incarnation : 0..MaxIncarnation, focusAdmitted : BOOLEAN]

\* The idle in-flight slot is a command-shaped record so equality against real
\* commands stays inside one comparable domain.
NoCommand == [kind |-> Idle, pane |-> "p0", incarnation |-> 0,
              focusAdmitted |-> FALSE]

NoIncarnation == [anchor |-> "a0", incarnation |-> NoRecord, attached |-> FALSE,
                  projected |-> FALSE, departing |-> FALSE, pending |-> NoFrame,
                  staged |-> NoFrame, accepted |-> NoFrame]

VARIABLES anchor, focus, leaf, rec, viewer, queue, inflight, nextInc,
          frameCursor, appliedCount, allTaggedValid

vars == <<anchor, focus, leaf, rec, viewer, queue, inflight, nextInc,
          frameCursor, appliedCount, allTaggedValid>>

TypeOK ==
  /\ anchor \in Anchors
  /\ focus \in Panes
  /\ leaf \in [Panes -> BOOLEAN]
  /\ rec \in [Panes -> [anchor : Anchors, incarnation : 0..MaxIncarnation,
                        attached : BOOLEAN, projected : BOOLEAN,
                        departing : BOOLEAN, pending : 0..MaxFrame,
                        staged : 0..MaxFrame, accepted : 0..MaxFrame]]
  /\ viewer \in [Panes -> 0..MaxIncarnation]
  /\ queue \in Seq(CommandRecords) /\ Len(queue) <= MaxQueue
  /\ inflight \in CommandRecords \cup {NoCommand}
  /\ nextInc \in [Panes -> Incarnations \cup {MaxIncarnation + 1}]
  /\ frameCursor \in 0..MaxFrame
  /\ appliedCount \in 0..MaxLog
  /\ allTaggedValid \in BOOLEAN

Init ==
  /\ anchor = "a0"
  /\ focus = "p0"
  /\ leaf = [p \in Panes |-> TRUE]
  /\ rec = [p \in Panes |->
       IF p = "p0"
       THEN [NoIncarnation EXCEPT !.incarnation = 1, !.attached = TRUE,
                                  !.projected = TRUE]
       ELSE NoIncarnation]
  /\ viewer = [p \in Panes |-> IF p = "p0" THEN 1 ELSE NoViewer]
  /\ queue = <<>>
  /\ inflight = NoCommand
  /\ nextInc = [p \in Panes |-> 2]
  /\ frameCursor = 0
  /\ appliedCount = 0
  /\ allTaggedValid = TRUE

HasRecord(p) == rec[p].incarnation # NoRecord
Live(p) == HasRecord(p) /\ ~rec[p].departing
ViewerCount == Cardinality({p \in Panes : viewer[p] # NoViewer})

\* Mailbox admission: control commands bound the logical focus pane at
\* enqueue time; the dequeue check only asks that the named pane record is
\* still a live member — an accepted command keeps its frozen target and does
\* not follow a later focus move. Polls admit only an exact non-departing
\* incarnation of the desired set.
Admitted(command) ==
  IF command.kind \in ControlKinds
  THEN HasRecord(command.pane) /\ ~rec[command.pane].departing
  ELSE HasRecord(command.pane)
       /\ rec[command.pane].incarnation = command.incarnation
       /\ ~rec[command.pane].departing

\* ------------------------- leaf membership -------------------------

MintPane ==
  \E p \in Panes :
    /\ leaf[p]
    /\ rec[p].incarnation = NoRecord
    /\ nextInc[p] <= MaxIncarnation
    /\ rec' = [rec EXCEPT ![p] =
         [anchor |-> anchor, incarnation |-> nextInc[p], attached |-> FALSE,
          projected |-> FALSE, departing |-> FALSE, pending |-> NoFrame,
          staged |-> NoFrame, accepted |-> NoFrame]]
    /\ nextInc' = [nextInc EXCEPT ![p] = @ + 1]
    /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, inflight,
                   frameCursor, appliedCount, allTaggedValid>>

VanishLeaf ==
  \E p \in Panes :
    /\ leaf[p] /\ HasRecord(p) /\ ~rec[p].departing
    /\ leaf' = [leaf EXCEPT ![p] = FALSE]
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.departing = TRUE,
                                  !.projected = FALSE, !.pending = NoFrame,
                                  !.staged = NoFrame, !.accepted = NoFrame]]
    /\ UNCHANGED <<anchor, focus, viewer, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

\* A reappearing leaf heals only a CURRENT-anchor departing record in place;
\* an old-anchor departing record can never come back — it waits for its own
\* disposal proof.
ReappearLeaf ==
  \E p \in Panes :
    /\ ~leaf[p]
    /\ leaf' = [leaf EXCEPT ![p] = TRUE]
    /\ IF HasRecord(p) /\ rec[p].departing /\ rec[p].anchor = anchor
       THEN rec' = [rec EXCEPT ![p] = [@ EXCEPT !.departing = FALSE]]
       ELSE UNCHANGED rec
    /\ UNCHANGED <<anchor, focus, viewer, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

\* Canonical disposal proof: only a departing record of the exact incarnation
\* detaches; the sibling record is untouched.
SettleDetachProof ==
  \E p \in Panes :
    /\ HasRecord(p) /\ rec[p].departing
    /\ rec' = [rec EXCEPT ![p] = NoIncarnation]
    /\ viewer' = [viewer EXCEPT ![p] = NoViewer]
    /\ UNCHANGED <<anchor, focus, leaf, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

RejectStaleDetachProof ==
  IF (\E p \in Panes, i \in Incarnations : i # rec[p].incarnation)
  THEN UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, inflight,
                  nextInc, frameCursor, appliedCount, allTaggedValid>>
  ELSE FALSE

\* Anchor epoch advance (tab switch / committed reload): every record of the
\* old anchor departs; a fresh desired set is installed by later mints.
AdvanceAnchorEpoch ==
  /\ anchor = "a0"
  /\ anchor' = "a1"
  /\ rec' = [p \in Panes |->
       IF rec[p].incarnation = NoRecord THEN rec[p]
       ELSE [rec[p] EXCEPT !.departing = TRUE, !.projected = FALSE,
                           !.pending = NoFrame, !.staged = NoFrame,
                           !.accepted = NoFrame]]
  /\ UNCHANGED <<focus, leaf, viewer, queue, inflight, nextInc,
                 frameCursor, appliedCount, allTaggedValid>>

\* Same-tab focus movement is a pure local selection change: it never mints,
\* departs, or re-tags anything already accepted.
MoveFocusLocal ==
  \E p \in Panes :
    /\ p # focus
    /\ focus' = p
    /\ UNCHANGED <<anchor, leaf, rec, viewer, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

\* ------------------------- attach / projection -------------------------

AttachPane ==
  \E p \in Panes :
    /\ Live(p) /\ ~rec[p].attached
    /\ ViewerCount < MaxViewers
    /\ viewer' = [viewer EXCEPT ![p] = rec[p].incarnation]
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.attached = TRUE]]
    /\ UNCHANGED <<anchor, focus, leaf, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

ProjectPane ==
  \E p \in Panes :
    /\ Live(p) /\ rec[p].attached /\ ~rec[p].projected
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.projected = TRUE]]
    /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

\* ------------------------- queue / executor -------------------------

EnqueueFocusControl ==
  \E kind \in ControlKinds :
    /\ Len(queue) < MaxQueue
    /\ HasRecord(focus) /\ ~rec[focus].departing
    /\ queue' = Append(queue,
         Command(kind, focus, rec[focus].incarnation, TRUE))
    /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

EnqueuePoll ==
  \E p \in Panes :
    /\ Len(queue) < MaxQueue
    /\ HasRecord(p)
    /\ queue' = Append(queue, Command("poll", p, rec[p].incarnation, FALSE))
    /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

\* Adversarial: a stale or cross-wired tag may sit in the queue — work
\* accepted before a rebind keeps its frozen incarnation, never the new one.
\* Forged tags name a strictly retired or never-held incarnation of that pane:
\* incarnation numbers only grow, so a forged tag can never coincide with a
\* not-yet-minted attachment and stay undeliverable forever — which is what
\* keeps the focus-provenance log claim honest.
EnqueueStaleCommand ==
  \E kind \in CommandKinds, p \in Panes, i \in 0..MaxIncarnation :
    /\ Len(queue) < MaxQueue
    /\ i < rec[p].incarnation
    /\ queue' = Append(queue, Command(kind, p, i, FALSE))
    /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

DequeueAdmit ==
  /\ queue # <<>>
  /\ inflight = NoCommand
  /\ Admitted(Head(queue))
  /\ inflight' = Head(queue)
  /\ queue' = Tail(queue)
  /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, nextInc, frameCursor,
                 appliedCount, allTaggedValid>>

DropStaleQueueCommand ==
  /\ queue # <<>>
  /\ inflight = NoCommand
  /\ ~Admitted(Head(queue))
  /\ queue' = Tail(queue)
  /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, inflight, nextInc,
                 frameCursor, appliedCount, allTaggedValid>>

\* The executor delivers control input only to the exact live attachment the
\* command named at enqueue — it does not re-check focus: an accepted command
\* keeps its frozen pane reference and never follows a later focus move. A
\* stale incarnation queued before a rebind cannot land on the new attachment,
\* and a control command cannot reach a sibling pane.
ExecuteControl ==
  /\ inflight # NoCommand /\ inflight.kind \in ControlKinds
  /\ LET p == inflight.pane IN
     IF viewer[p] # NoViewer /\ viewer[p] = inflight.incarnation /\ Live(p)
        /\ appliedCount < MaxLog
     THEN /\ appliedCount' = appliedCount + 1
          /\ allTaggedValid' = (allTaggedValid /\
                                (inflight.incarnation = viewer[p]))
     ELSE UNCHANGED <<appliedCount, allTaggedValid>>
  /\ inflight' = NoCommand
  /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, nextInc,
                 frameCursor>>

\* A poll for the exact live incarnation deposits a fresh frame awaiting its
\* draw pass on that record — the received-but-not-yet-drawn state the
\* mailbox's causal settlement holds; a poll whose incarnation retired
\* produces nothing.
ExecutePoll ==
  /\ inflight # NoCommand /\ inflight.kind = "poll"
  /\ LET p == inflight.pane IN
     IF rec[p].incarnation = inflight.incarnation /\ Live(p)
        /\ rec[p].attached /\ rec[p].projected /\ frameCursor < MaxFrame
     THEN /\ frameCursor' = frameCursor + 1
          /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.pending = frameCursor + 1]]
     ELSE UNCHANGED <<frameCursor, rec>>
  /\ inflight' = NoCommand
  /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, nextInc,
                 appliedCount, allTaggedValid>>

ExpireInflight ==
  /\ inflight # NoCommand
  /\ inflight' = NoCommand
  /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, nextInc,
                 frameCursor, appliedCount, allTaggedValid>>

\* ------------------------- draw / ack -------------------------

\* A completed draw stages exactly the pending frame on the exact live record
\* it belongs to — stage_drawn_frame refuses a departing, unattached or
\* unprojected record, and re-staging voids the previous acknowledgement.
DrawPendingFrame ==
  \E p \in Panes :
    /\ Live(p) /\ rec[p].attached /\ rec[p].projected
    /\ rec[p].pending # NoFrame
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.staged = rec[p].pending,
                                  !.pending = NoFrame, !.accepted = NoFrame]]
    /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

\* FrameApplied commits only for the exact record that completed the draw —
\* a frame that was received but never drawn, a departing record, or a
\* different frame identity can never produce an ack.
SettleFrameAck ==
  \E p \in Panes :
    /\ Live(p) /\ rec[p].attached /\ rec[p].projected
    /\ rec[p].staged # NoFrame
    /\ rec' = [rec EXCEPT ![p].accepted = rec[p].staged]
    /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

RejectStaleFrameAck ==
  IF (\E p \in Panes, i \in Incarnations, f \in Frames :
       \/ i # rec[p].incarnation
       \/ ~Live(p)
       \/ rec[p].staged # f)
  THEN UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, inflight,
                  nextInc, frameCursor, appliedCount, allTaggedValid>>
  ELSE FALSE

\* ------------------------- faults / close -------------------------

SettlePaneFault ==
  \E p \in Panes :
    /\ Live(p)
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.projected = FALSE,
                                  !.pending = NoFrame, !.staged = NoFrame,
                                  !.accepted = NoFrame]]
    /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

RejectStalePaneFault ==
  IF (\E p \in Panes, i \in Incarnations : i # rec[p].incarnation)
  THEN UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, inflight,
                  nextInc, frameCursor, appliedCount, allTaggedValid>>
  ELSE FALSE

ClosePane ==
  \E p \in Panes :
    /\ leaf[p] /\ HasRecord(p) /\ ~rec[p].departing
    /\ leaf' = [leaf EXCEPT ![p] = FALSE]
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.departing = TRUE,
                                  !.projected = FALSE, !.pending = NoFrame,
                                  !.staged = NoFrame, !.accepted = NoFrame]]
    /\ UNCHANGED <<anchor, focus, viewer, queue, inflight, nextInc,
                   frameCursor, appliedCount, allTaggedValid>>

Next ==
  \/ MintPane
  \/ VanishLeaf
  \/ ReappearLeaf
  \/ SettleDetachProof
  \/ RejectStaleDetachProof
  \/ AdvanceAnchorEpoch
  \/ MoveFocusLocal
  \/ AttachPane
  \/ ProjectPane
  \/ EnqueueFocusControl
  \/ EnqueuePoll
  \/ EnqueueStaleCommand
  \/ DequeueAdmit
  \/ DropStaleQueueCommand
  \/ ExecuteControl
  \/ ExecutePoll
  \/ ExpireInflight
  \/ DrawPendingFrame
  \/ SettleFrameAck
  \/ RejectStaleFrameAck
  \/ SettlePaneFault
  \/ RejectStalePaneFault
  \/ ClosePane

Fairness ==
  /\ WF_vars(SettleDetachProof)
  /\ WF_vars(ReappearLeaf)
  /\ WF_vars(DequeueAdmit)
  /\ WF_vars(DropStaleQueueCommand)
  /\ WF_vars(ExecuteControl)
  /\ WF_vars(ExecutePoll)
  /\ WF_vars(ExpireInflight)

Spec == Init /\ [][Next]_vars /\ Fairness

\* ------------------------- invariants -------------------------

\* Every non-departing record belongs to the current selection anchor: an old
\* epoch's pane is never mistaken for a live member.
LiveRecordsShareCurrentAnchor ==
  \A p \in Panes : Live(p) => rec[p].anchor = anchor

\* A departing record carries no pending draw, staged frame, accepted frame,
\* or projection: depart clears the frame bookkeeping even before disposal
\* lands, so a departed pane can never settle a late poll as FrameApplied.
DepartingCarriesNoFrameWork ==
  \A p \in Panes :
    rec[p].departing => rec[p].pending = NoFrame /\ rec[p].staged = NoFrame
                        /\ rec[p].accepted = NoFrame /\ ~rec[p].projected

\* A received-but-undrawn frame exists only on a live attached projected
\* record — a pending draw can never outlive its pane's membership.
PendingFrameIsLiveAttachedProjected ==
  \A p \in Panes :
    rec[p].pending # NoFrame =>
      ~rec[p].departing /\ rec[p].attached /\ rec[p].projected

\* An accepted frame is always the frame this very record staged — an ack can
\* never credit a sibling pane's or an old incarnation's staged work.
AcceptedFrameIsOwnStaged ==
  \A p \in Panes :
    rec[p].accepted # NoFrame => rec[p].accepted = rec[p].staged

\* A host viewer exists only as the exact incarnation of its record — a
\* departing record legitimately keeps its attachment until the disposal
\* proof lands, but the incarnation numbers always agree.
ViewerIsExactLiveIncarnation ==
  \A p \in Panes :
    viewer[p] # NoViewer =>
      HasRecord(p) /\ rec[p].incarnation = viewer[p]

ViewerBudget ==
  ViewerCount <= MaxViewers

\* Every delivered input landed on the exact live attachment incarnation it
\* was tagged for — no stale replay, no sibling delivery.
InputsDeliverOnlyToTaggedLiveFocus ==
  allTaggedValid

\* A control command that could still deliver to its named pane must have been
\* admitted while that pane held focus — focus gates admission at enqueue time
\* only, never delivery.
DeliverableControlWasFocusAdmitted ==
  inflight # NoCommand /\ inflight.kind \in ControlKinds
  /\ viewer[inflight.pane] # NoViewer
  /\ viewer[inflight.pane] = inflight.incarnation /\ Live(inflight.pane)
    => inflight.focusAdmitted

\* FrameApplied commits only onto a record that is live, attached and
\* projected — an ack can never credit a departing or unattached pane.
AcceptedImpliesAttachedLiveProjected ==
  \A p \in Panes :
    rec[p].accepted # NoFrame =>
      ~rec[p].departing /\ rec[p].attached /\ rec[p].projected

\* Departing panes are either disposed by their own exact proof or
\* re-admitted by reappearance; they cannot linger forever.
DepartedEventuallyDisposesOrReappears ==
  \A p \in Panes : rec[p].departing ~> ~rec[p].departing

===========================================================================
