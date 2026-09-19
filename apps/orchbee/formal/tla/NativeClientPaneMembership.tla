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
\*   appliedInputs ~ ghost log of inputs the host actually delivered, used to
\*                   prove no input ever lands on a stale or sibling attachment
\*
\* Bounds: two panes, two anchors (one epoch switch), two incarnations per
\* pane, a bounded queue, two frame identities, and a bounded ghost-rejection
\* counter. The old `NativeClientPanelLifecycle` single-attachment invariants
\* remain the single-pane specialization and are not claimed as proof of this
\* collection.

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS MaxQueue, MaxIncarnation, MaxFrame, MaxReject, MaxViewers, MaxLog

ASSUME
  /\ MaxQueue \in Nat \ {0}
  /\ MaxIncarnation = 2
  /\ MaxFrame \in Nat \ {0}
  /\ MaxReject \in Nat \ {0}
  /\ MaxViewers \in Nat \ {0}
  /\ MaxLog \in Nat \ {0}

Panes == {"p0", "p1"}
Anchors == {"a0", "a1"}
NoRecord == 0
NoViewer == 0
NoFrame == 0
Frames == 1..MaxFrame
Incarnations == 1..MaxIncarnation

ControlKinds == {"input", "resize"}
CommandKinds == ControlKinds \cup {"poll"}
Idle == "idle"

Command(kind, pane, incarnation) ==
  [kind |-> kind, pane |-> pane, incarnation |-> incarnation]

CommandRecords ==
  [kind : CommandKinds \cup {Idle}, pane : Panes,
   incarnation : 0..MaxIncarnation]

\* The idle in-flight slot is a command-shaped record so equality against real
\* commands stays inside one comparable domain.
NoCommand == [kind |-> Idle, pane |-> "p0", incarnation |-> 0]

NoIncarnation == [anchor |-> "a0", incarnation |-> NoRecord, attached |-> FALSE,
                  projected |-> FALSE, departing |-> FALSE,
                  staged |-> NoFrame, accepted |-> NoFrame]

VARIABLES anchor, focus, leaf, rec, viewer, queue, inflight, nextInc,
          frameCursor, appliedInputs, rejections

vars == <<anchor, focus, leaf, rec, viewer, queue, inflight, nextInc,
          frameCursor, appliedInputs, rejections>>

TypeOK ==
  /\ anchor \in Anchors
  /\ focus \in Panes
  /\ leaf \in [Panes -> BOOLEAN]
  /\ rec \in [Panes -> [anchor : Anchors, incarnation : 0..MaxIncarnation,
                        attached : BOOLEAN, projected : BOOLEAN,
                        departing : BOOLEAN, staged : 0..MaxFrame,
                        accepted : 0..MaxFrame]]
  /\ viewer \in [Panes -> 0..MaxIncarnation]
  /\ queue \in Seq(CommandRecords) /\ Len(queue) <= MaxQueue
  /\ inflight \in CommandRecords \cup {NoCommand}
  /\ nextInc \in [Panes -> Incarnations \cup {MaxIncarnation + 1}]
  /\ frameCursor \in 0..MaxFrame
  /\ appliedInputs \in Seq([pane : Panes, tag : 0..MaxIncarnation,
                           delivered : 0..MaxIncarnation, wasFocus : BOOLEAN])
  /\ Len(appliedInputs) <= MaxLog
  /\ rejections \in 0..MaxReject

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
  /\ appliedInputs = <<>>
  /\ rejections = 0

HasRecord(p) == rec[p].incarnation # NoRecord
Live(p) == HasRecord(p) /\ ~rec[p].departing
ViewerCount == Cardinality({p \in Panes : viewer[p] # NoViewer})

\* Mailbox admission: control commands admit on the logical focus pane
\* (incarnation is fenced later at execution); polls admit only an exact
\* non-departing incarnation of the desired set.
Admitted(command) ==
  IF command.kind \in ControlKinds
  THEN command.pane = focus /\ HasRecord(command.pane)
       /\ ~rec[command.pane].departing
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
          projected |-> FALSE, departing |-> FALSE, staged |-> NoFrame,
          accepted |-> NoFrame]]
    /\ nextInc' = [nextInc EXCEPT ![p] = @ + 1]
    /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, inflight,
                   frameCursor, appliedInputs, rejections>>

VanishLeaf ==
  \E p \in Panes :
    /\ leaf[p] /\ HasRecord(p) /\ ~rec[p].departing
    /\ leaf' = [leaf EXCEPT ![p] = FALSE]
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.departing = TRUE,
                                  !.projected = FALSE, !.staged = NoFrame,
                                  !.accepted = NoFrame]]
    /\ UNCHANGED <<anchor, focus, viewer, queue, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

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
                   frameCursor, appliedInputs, rejections>>

\* Canonical disposal proof: only a departing record of the exact incarnation
\* detaches; the sibling record is untouched.
SettleDetachProof ==
  \E p \in Panes :
    /\ HasRecord(p) /\ rec[p].departing
    /\ rec' = [rec EXCEPT ![p] = NoIncarnation]
    /\ viewer' = [viewer EXCEPT ![p] = NoViewer]
    /\ UNCHANGED <<anchor, focus, leaf, queue, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

RejectStaleDetachProof ==
  \E p \in Panes, i \in Incarnations :
    /\ i # rec[p].incarnation
    /\ rejections < MaxReject
    /\ rejections' = rejections + 1
    /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, inflight,
                   nextInc, frameCursor, appliedInputs>>

\* Anchor epoch advance (tab switch / committed reload): every record of the
\* old anchor departs; a fresh desired set is installed by later mints.
AdvanceAnchorEpoch ==
  /\ anchor = "a0"
  /\ anchor' = "a1"
  /\ rec' = [p \in Panes |->
       IF rec[p].incarnation = NoRecord THEN rec[p]
       ELSE [rec[p] EXCEPT !.departing = TRUE, !.projected = FALSE,
                           !.staged = NoFrame, !.accepted = NoFrame]]
  /\ UNCHANGED <<focus, leaf, viewer, queue, inflight, nextInc,
                 frameCursor, appliedInputs, rejections>>

\* Same-tab focus movement is a pure local selection change: it never mints,
\* departs, or re-tags anything already accepted.
MoveFocusLocal ==
  \E p \in Panes :
    /\ p # focus
    /\ focus' = p
    /\ UNCHANGED <<anchor, leaf, rec, viewer, queue, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

\* ------------------------- attach / projection -------------------------

AttachPane ==
  \E p \in Panes :
    /\ Live(p) /\ ~rec[p].attached
    /\ ViewerCount < MaxViewers
    /\ viewer' = [viewer EXCEPT ![p] = rec[p].incarnation]
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.attached = TRUE]]
    /\ UNCHANGED <<anchor, focus, leaf, queue, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

ProjectPane ==
  \E p \in Panes :
    /\ Live(p) /\ rec[p].attached /\ ~rec[p].projected
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.projected = TRUE]]
    /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

\* ------------------------- queue / executor -------------------------

EnqueueFocusControl ==
  \E kind \in ControlKinds :
    /\ Len(queue) < MaxQueue
    /\ HasRecord(focus) /\ ~rec[focus].departing
    /\ queue' = Append(queue,
         Command(kind, focus, rec[focus].incarnation))
    /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

EnqueuePoll ==
  \E p \in Panes :
    /\ Len(queue) < MaxQueue
    /\ HasRecord(p)
    /\ queue' = Append(queue, Command("poll", p, rec[p].incarnation))
    /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

\* Adversarial: a stale or cross-wired tag may sit in the queue — work
\* accepted before a rebind keeps its frozen incarnation, never the new one.
EnqueueStaleCommand ==
  \E kind \in CommandKinds, p \in Panes, i \in Incarnations :
    /\ Len(queue) < MaxQueue
    /\ i # rec[p].incarnation
    /\ queue' = Append(queue, Command(kind, p, i))
    /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

DequeueAdmit ==
  /\ queue # <<>>
  /\ inflight = NoCommand
  /\ Admitted(Head(queue))
  /\ inflight' = Head(queue)
  /\ queue' = Tail(queue)
  /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, nextInc, frameCursor,
                 appliedInputs, rejections>>

DropStaleQueueCommand ==
  /\ queue # <<>>
  /\ inflight = NoCommand
  /\ ~Admitted(Head(queue))
  /\ queue' = Tail(queue)
  /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, inflight, nextInc,
                 frameCursor, appliedInputs, rejections>>

\* The executor delivers control input only to the exact live attachment the
\* command named — a stale incarnation queued before a rebind cannot land on
\* the new attachment, and a control command cannot reach a sibling pane.
ExecuteControl ==
  /\ inflight # NoCommand /\ inflight.kind \in ControlKinds
  /\ LET p == inflight.pane IN
     IF viewer[p] = inflight.incarnation /\ p = focus /\ Live(p)
        /\ Len(appliedInputs) < MaxLog
     THEN appliedInputs' = Append(appliedInputs,
            [pane |-> p, tag |-> inflight.incarnation,
             delivered |-> viewer[p], wasFocus |-> TRUE])
     ELSE UNCHANGED appliedInputs
  /\ inflight' = NoCommand
  /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, nextInc,
                 frameCursor, rejections>>

\* A poll for the exact live incarnation stages a fresh frame on that record;
\* a poll whose incarnation retired produces nothing.
ExecutePoll ==
  /\ inflight # NoCommand /\ inflight.kind = "poll"
  /\ LET p == inflight.pane IN
     IF rec[p].incarnation = inflight.incarnation /\ Live(p)
        /\ rec[p].attached /\ rec[p].projected /\ frameCursor < MaxFrame
     THEN /\ frameCursor' = frameCursor + 1
          /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.staged = frameCursor + 1,
                                        !.accepted = NoFrame]]
          /\ UNCHANGED rejections
     ELSE /\ rejections < MaxReject
          /\ rejections' = rejections + 1
          /\ UNCHANGED <<frameCursor, rec>>
  /\ inflight' = NoCommand
  /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, nextInc,
                 appliedInputs>>

ExpireInflight ==
  /\ inflight # NoCommand
  /\ inflight' = NoCommand
  /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, nextInc,
                 frameCursor, appliedInputs, rejections>>

\* ------------------------- draw / ack -------------------------

\* A draw commits FrameApplied only for the exact record it staged on.
SettleFrameAck ==
  \E p \in Panes :
    /\ Live(p) /\ rec[p].attached /\ rec[p].projected
    /\ rec[p].staged # NoFrame
    /\ rec' = [rec EXCEPT ![p].accepted = rec[p].staged]
    /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

RejectStaleFrameAck ==
  \E p \in Panes, i \in Incarnations, f \in Frames :
    /\ rejections < MaxReject
    /\ \/ i # rec[p].incarnation
       \/ ~Live(p)
       \/ rec[p].staged # f
    /\ rejections' = rejections + 1
    /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, inflight,
                   nextInc, frameCursor, appliedInputs>>

\* ------------------------- faults / close -------------------------

SettlePaneFault ==
  \E p \in Panes :
    /\ Live(p)
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.projected = FALSE,
                                  !.staged = NoFrame, !.accepted = NoFrame]]
    /\ UNCHANGED <<anchor, focus, leaf, viewer, queue, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

RejectStalePaneFault ==
  \E p \in Panes, i \in Incarnations :
    /\ i # rec[p].incarnation
    /\ rejections < MaxReject
    /\ rejections' = rejections + 1
    /\ UNCHANGED <<anchor, focus, leaf, rec, viewer, queue, inflight,
                   nextInc, frameCursor, appliedInputs>>

ClosePane ==
  \E p \in Panes :
    /\ leaf[p] /\ HasRecord(p) /\ ~rec[p].departing
    /\ leaf' = [leaf EXCEPT ![p] = FALSE]
    /\ rec' = [rec EXCEPT ![p] = [@ EXCEPT !.departing = TRUE,
                                  !.projected = FALSE, !.staged = NoFrame,
                                  !.accepted = NoFrame]]
    /\ UNCHANGED <<anchor, focus, viewer, queue, inflight, nextInc,
                   frameCursor, appliedInputs, rejections>>

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

\* A departing record carries no projection, staged frame, or accepted frame:
\* depart clears the frame bookkeeping even before disposal lands.
DepartingCarriesNoFrameWork ==
  \A p \in Panes :
    rec[p].departing => rec[p].staged = NoFrame /\ rec[p].accepted = NoFrame
                        /\ ~rec[p].projected

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

\* Every delivered input landed on the attachment incarnation it was tagged
\* for, while that pane held focus — no stale replay, no sibling delivery.
InputsDeliverOnlyToTaggedLiveFocus ==
  \A i \in 1..Len(appliedInputs) :
    /\ appliedInputs[i].tag = appliedInputs[i].delivered
    /\ appliedInputs[i].wasFocus

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
