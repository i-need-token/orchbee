----------------- MODULE NativeClientPublicationRefresh -----------------
EXTENDS Naturals, Sequences, FiniteSets

\* Owned by apps/orchbee/docs/native-tab-session.md. Companion model covering the
\* publication-hint ingress latch -> canonical refresh debt -> refresh drain chain and the
\* ordinary Poll lease -> UI observation discipline the hint rides on. ProducerFold, the
\* mailbox ingress latch and the UI capture are three separate semantic boundaries here: a
\* folded publication is owed to the UI even when its carrying PollResult is never placed,
\* is dropped by a Switch admission, or is refused at refresh admission.
CONSTANTS MaxQueue, MaxSerial, MaxGeneration
ASSUME MaxQueue = 2 /\ MaxSerial = 8 /\ MaxGeneration = 2

Keys == {"k1", "k2"}
NoKey == "none"
Other(key) == IF key = "k1" THEN "k2" ELSE "k1"

PollRecords == [kind: {"poll"}, serial: 1..MaxSerial, key: Keys]
RefreshRecords == [kind: {"refresh"}, serial: 1..MaxSerial, gen: 1..MaxGeneration]
QueueRecords == PollRecords \cup RefreshRecords
LeaseRecords == [serial: 1..MaxSerial, kind: {"poll", "refresh"}, key: Keys \cup {NoKey}]
PollResultEvents == [kind: {"poll-result"}, serial: 1..MaxSerial, key: Keys]
RefreshResultEvents ==
  [kind: {"refresh-result"}, gen: 1..MaxGeneration,
   outcome: {"install", "refuse", "diverge"}]
FeedbackEvents == [kind: {"feedback"}, serial: 1..MaxSerial]
EventRecords == PollResultEvents \cup RefreshResultEvents \cup FeedbackEvents
CausalRecords ==
  [stage: {"awaiting", "settled"}, serial: 1..MaxSerial, key: Keys,
   kind: {"none", "feedback"}]
PendingDraws == [serial: 1..MaxSerial, key: Keys]
DebtStates == {"clean", "pending", "inflight", "parked"}
PollStages == {"idle", "probing", "publish-entry", "publishing", "awaiting-causal"}

\* Record sentinels keep optional cells comparable inside TLC record equality.
NoLease == [kind |-> "none", serial |-> 0, key |-> "none"]
NoEvent == [kind |-> "none"]
NoCausal == [kind |-> "none"]
NoDraw == [kind |-> "none"]

VARIABLE s
vars == <<s>>

Init ==
  \E desired \in Keys:
    s = [status |-> "live",
         desired |-> desired,
         switchFrom |-> NoKey,
         ready |-> TRUE,
         switching |-> FALSE,
         draftOpen |-> FALSE,
         mailboxBusy |-> FALSE,
         draftBudget |-> 1,
         busyBudget |-> 1,
         switchBudget |-> 1,
         queue |-> <<>>,
         nextSerial |-> 0,
         active |-> NoLease,
         pollExec |-> "idle",
         pollHint |-> FALSE,
         event |-> NoEvent,
         causal |-> NoCausal,
         latch |-> FALSE,
         outstanding |-> FALSE,
         uninstalled |-> FALSE,
         cursor |-> 0,
         refreshGen |-> 0,
         uiObserved |-> 0,
         pendingDraw |-> NoDraw,
         debt |-> "clean",
         late |-> FALSE]

PollQueued == \E i \in DOMAIN s.queue : s.queue[i].kind = "poll"
ActivePoll == s.active # NoLease /\ s.active.kind = "poll"
PollOccupied == PollQueued \/ ActivePoll
PollOutstanding(serial) ==
  \/ \E i \in DOMAIN s.queue : s.queue[i].kind = "poll" /\ s.queue[i].serial = serial
  \/ ActivePoll /\ s.active.serial = serial
RefreshTracked ==
  \/ \E i \in DOMAIN s.queue :
       s.queue[i].kind = "refresh" /\ s.queue[i].gen = s.refreshGen
  \/ s.active # NoLease /\ s.active.kind = "refresh"
  \/ s.event # NoEvent /\ s.event.kind = "refresh-result" /\ s.event.gen = s.refreshGen
RefreshAdmissionBlocked == s.draftOpen \/ s.switching
RefreshAdmissionFull ==
  s.refreshGen # 0 \/ s.mailboxBusy \/ Len(s.queue) = MaxQueue
SerialExhausted == s.nextSerial >= MaxSerial
GenerationExhausted == s.cursor >= MaxGeneration

Retired(state) ==
  [state EXCEPT
    !.status = "retired", !.queue = <<>>, !.active = NoLease, !.pollExec = "idle",
    !.event = NoEvent, !.causal = NoCausal, !.latch = FALSE, !.outstanding = FALSE,
    !.uninstalled = FALSE, !.refreshGen = 0, !.debt = "clean", !.late = FALSE,
    !.uiObserved = 0, !.pendingDraw = NoDraw, !.switching = FALSE, !.draftOpen = FALSE,
    !.mailboxBusy = FALSE, !.switchFrom = NoKey]

\* A peer Client commits a Workbench publication; the executor baseline has not folded it.
PeerPublishes ==
  /\ s.status = "live"
  /\ s' = [s EXCEPT !.outstanding = TRUE]

\* --- ordinary Poll scheduling -----------------------------------------------------

\* The UI schedules one ordinary Poll only when its observed serial is clear. Admission
\* re-checks canonical occupancy (any queued or executing ordinary Poll, regardless of
\* attachment) inside the mailbox mutex, so a cleared observation can never admit a second
\* Poll while a lease still lives.
SchedulePoll ==
  /\ s.status = "live" /\ s.ready
  /\ s.uiObserved = 0 /\ s.pendingDraw = NoDraw
  /\ ~PollOccupied /\ Len(s.queue) < MaxQueue /\ ~SerialExhausted
  /\ LET serial == s.nextSerial + 1 IN
       s' = [s EXCEPT
             !.queue = Append(@, [kind |-> "poll", serial |-> serial, key |-> s.desired]),
             !.nextSerial = serial, !.uiObserved = serial]

\* A second ordinary Poll while one is queued or executing is refused as a local no-op.
PollAdmissionBusy ==
  /\ s.status = "live" /\ s.ready /\ s.uiObserved = 0 /\ s.pendingDraw = NoDraw
  /\ PollOccupied
  /\ UNCHANGED s

PollAdmissionFull ==
  /\ s.status = "live" /\ s.ready /\ s.uiObserved = 0 /\ s.pendingDraw = NoDraw
  /\ ~PollOccupied /\ Len(s.queue) = MaxQueue
  /\ UNCHANGED s

\* Serial exhaustion on an admission attempt closes the mailbox and retires the session.
IdentityExhausted ==
  /\ s.status = "live"
  /\ \/ /\ s.ready /\ s.uiObserved = 0 /\ s.pendingDraw = NoDraw /\ ~PollOccupied
        /\ Len(s.queue) < MaxQueue /\ SerialExhausted
     \/ /\ ~RefreshAdmissionBlocked /\ ~RefreshAdmissionFull
        /\ (SerialExhausted \/ GenerationExhausted)
  /\ s' = Retired(s)

\* --- executor Poll lifecycle --------------------------------------------------------

Recv ==
  /\ s.status = "live" /\ s.active = NoLease /\ s.queue # <<>>
  /\ Head(s.queue).kind \in {"poll", "refresh"}
  /\ LET rec == Head(s.queue) IN
       s' = [s EXCEPT
             !.queue = Tail(@),
             !.active = [serial |-> rec.serial, kind |-> rec.kind,
                         key |-> IF rec.kind = "poll" THEN rec.key ELSE NoKey],
             !.pollExec = IF rec.kind = "poll" THEN "probing" ELSE "idle"]

\* A Poll whose key went stale before execution finishes silently: no probe, no event.
PollSkippedStale ==
  /\ s.status = "live" /\ s.pollExec = "probing"
  /\ s.active # NoLease /\ s.active.key # s.desired
  /\ s' = [s EXCEPT !.active = NoLease, !.pollExec = "idle"]

\* The executor probes the publication journal during the Poll body; a folded publication
\* rides with the result toward publication entry.
PollProbes ==
  /\ s.status = "live" /\ s.pollExec = "probing"
  /\ s.active # NoLease /\ s.active.key = s.desired
  /\ s' = [s EXCEPT
           !.pollExec = "publish-entry",
           !.pollHint = s.outstanding,
           !.uninstalled = s.uninstalled \/ s.outstanding,
           !.outstanding = FALSE]

\* The ingress latch is registered at publication entry, before the stale check and before
\* waiting on the event cell, so the hint survives every later publish outcome.
EnterPollPublish ==
  /\ s.status = "live" /\ s.pollExec = "publish-entry"
  /\ s' = [s EXCEPT !.pollExec = "publishing",
                    !.latch = s.latch \/ s.pollHint,
                    !.pollHint = FALSE]

PlacePollResult ==
  /\ s.status = "live" /\ s.pollExec = "publishing"
  /\ s.active # NoLease /\ s.active.key = s.desired
  /\ s.event = NoEvent /\ s.causal = NoCausal
  /\ s' = [s EXCEPT
           !.event = [kind |-> "poll-result", serial |-> s.active.serial,
                      key |-> s.active.key],
           !.causal = [stage |-> "awaiting", serial |-> s.active.serial,
                       key |-> s.active.key, kind |-> "none"],
           !.pollExec = "awaiting-causal"]

\* Publication returns Stale when desired moved between the probe and placement; the latch
\* was already registered at entry and is never rolled back.
StalePublishFinish ==
  /\ s.status = "live" /\ s.pollExec = "publishing"
  /\ s.active # NoLease /\ s.active.key # s.desired
  /\ s' = [s EXCEPT !.active = NoLease, !.pollExec = "idle"]

ConsumeEmptyPollResult ==
  /\ s.status = "live" /\ s.event # NoEvent /\ s.event.kind = "poll-result"
  /\ s' = [s EXCEPT
           !.event = NoEvent,
           !.uiObserved = IF s.uiObserved = s.event.serial THEN 0 ELSE s.uiObserved,
           !.causal = [stage |-> "settled", serial |-> s.event.serial,
                       key |-> s.event.key, kind |-> "none"]]

ConsumeFramedPollResult ==
  /\ s.status = "live" /\ s.event # NoEvent /\ s.event.kind = "poll-result"
  /\ s' = [s EXCEPT !.event = NoEvent,
                    !.pendingDraw = [serial |-> s.event.serial, key |-> s.event.key]]

\* after_draw deposits the causal feedback; a framed result owes the feedback round-trip.
SettleDraw ==
  /\ s.status = "live" /\ s.pendingDraw # NoDraw
  /\ s' = [s EXCEPT
           !.pendingDraw = NoDraw,
           !.causal = [stage |-> "settled", serial |-> s.pendingDraw.serial,
                       key |-> s.pendingDraw.key, kind |-> "feedback"]]

\* The executor takes the settled causal; a NoFeedback settlement finishes the lease
\* immediately, a Feedback settlement is delivered and owes one more round-trip.
FinishPollLease ==
  /\ s.status = "live" /\ s.pollExec = "awaiting-causal"
  /\ s.active # NoLease /\ s.causal # NoCausal /\ s.causal.stage = "settled"
  /\ s.causal.serial = s.active.serial /\ s.causal.kind = "none"
  /\ s' = [s EXCEPT !.causal = NoCausal, !.active = NoLease, !.pollExec = "idle"]

PublishFeedbackAccepted ==
  /\ s.status = "live" /\ s.pollExec = "awaiting-causal"
  /\ s.active # NoLease /\ s.causal # NoCausal /\ s.causal.stage = "settled"
  /\ s.causal.serial = s.active.serial /\ s.causal.kind = "feedback"
  /\ s.causal.key = s.desired /\ s.event = NoEvent
  /\ s' = [s EXCEPT
           !.event = [kind |-> "feedback", serial |-> s.active.serial],
           !.causal = [stage |-> "awaiting", serial |-> s.active.serial,
                       key |-> s.active.key, kind |-> "none"]]

\* A taken feedback causal is discarded when desired moved before FeedbackAccepted landed.
DiscardStaleFeedback ==
  /\ s.status = "live" /\ s.pollExec = "awaiting-causal"
  /\ s.active # NoLease /\ s.causal # NoCausal /\ s.causal.stage = "settled"
  /\ s.causal.serial = s.active.serial /\ s.causal.kind = "feedback"
  /\ s.causal.key # s.desired
  /\ s' = [s EXCEPT !.causal = NoCausal, !.active = NoLease, !.pollExec = "idle"]

ConsumeFeedbackAccepted ==
  /\ s.status = "live" /\ s.event # NoEvent /\ s.event.kind = "feedback"
  /\ s' = [s EXCEPT
           !.event = NoEvent,
           !.uiObserved = IF s.uiObserved = s.event.serial THEN 0 ELSE s.uiObserved,
           !.causal = IF s.causal # NoCausal /\ s.causal.serial = s.event.serial
                        THEN [stage |-> "settled", serial |-> s.event.serial,
                              key |-> s.causal.key, kind |-> "none"]
                        ELSE s.causal]

\* Event-cell or causal-wait expiry under a non-Switch barrier retires on the Deadline path.
PollWireExpires ==
  /\ s.status = "live"
  /\ \/ s.event # NoEvent /\ s.event.kind \in {"poll-result", "feedback"}
     \/ s.causal # NoCausal /\ s.causal.stage = "awaiting"
  /\ s' = Retired(s)

\* --- publication-hint capture and refresh debt --------------------------------------

\* One fair UI tick moves a latched hint into the canonical debt exactly once: a live
\* refresh slot turns it into late demand on that flight, otherwise it pends.
CaptureHint ==
  /\ s.status = "live" /\ s.latch
  /\ s' = [s EXCEPT
           !.latch = FALSE,
           !.debt = IF s.refreshGen # 0 THEN "inflight" ELSE "pending",
           !.late = IF s.refreshGen # 0 THEN TRUE ELSE s.late]

\* Draining pends on one fresh refresh flight; the flight adopts the debt.
DrainPendingRefresh ==
  /\ s.status = "live" /\ s.debt = "pending"
  /\ ~RefreshAdmissionBlocked /\ ~RefreshAdmissionFull
  /\ ~GenerationExhausted /\ ~SerialExhausted
  /\ LET gen == s.cursor + 1
         serial == s.nextSerial + 1
     IN
       s' = [s EXCEPT
             !.queue = Append(@, [kind |-> "refresh", serial |-> serial, gen |-> gen]),
             !.cursor = gen, !.nextSerial = serial, !.refreshGen = gen,
             !.debt = "inflight", !.late = FALSE]

\* Refused admission preserves the debt; it is retried by a later fair tick.
DrainRefusedBusy ==
  /\ s.status = "live" /\ s.debt = "pending" /\ RefreshAdmissionBlocked
  /\ UNCHANGED s

DrainRefusedFull ==
  /\ s.status = "live" /\ s.debt = "pending" /\ ~RefreshAdmissionBlocked
  /\ RefreshAdmissionFull
  /\ UNCHANGED s

\* An explicit refresh re-arms a parked debt; a clean explicit refresh runs unassociated.
RearmParkedRefresh ==
  /\ s.status = "live" /\ s.debt = "parked"
  /\ ~RefreshAdmissionBlocked /\ ~RefreshAdmissionFull
  /\ ~GenerationExhausted /\ ~SerialExhausted
  /\ LET gen == s.cursor + 1
         serial == s.nextSerial + 1
     IN
       s' = [s EXCEPT
             !.queue = Append(@, [kind |-> "refresh", serial |-> serial, gen |-> gen]),
             !.cursor = gen, !.nextSerial = serial, !.refreshGen = gen,
             !.debt = "inflight", !.late = FALSE]

ManualRefreshClean ==
  /\ s.status = "live" /\ s.debt = "clean"
  /\ ~RefreshAdmissionBlocked /\ ~RefreshAdmissionFull
  /\ ~GenerationExhausted /\ ~SerialExhausted
  /\ LET gen == s.cursor + 1
         serial == s.nextSerial + 1
     IN
       s' = [s EXCEPT
             !.queue = Append(@, [kind |-> "refresh", serial |-> serial, gen |-> gen]),
             !.cursor = gen, !.nextSerial = serial, !.refreshGen = gen]

\* A second refresh marker while a flight already owns the slot never admits a duplicate.
ManualRefreshWhileFlightLive ==
  /\ s.status = "live" /\ s.refreshGen # 0
  /\ UNCHANGED s

\* --- refresh flight -----------------------------------------------------------------

\* The executor's snapshot read publishes one typed result into the bounded event cell.
CompleteRefreshRead ==
  /\ s.status = "live" /\ s.active # NoLease /\ s.active.kind = "refresh"
  /\ s.event = NoEvent
  /\ \E outcome \in {"install", "refuse", "diverge"}:
       s' = [s EXCEPT !.event = [kind |-> "refresh-result", gen |-> s.refreshGen,
                                 outcome |-> outcome]]

\* An install settles the demand the flight adopted; a fold that landed during the flight
\* is late demand and pends one conservative follow-up read.
ApplyRefreshInstall ==
  /\ s.status = "live" /\ s.event # NoEvent /\ s.event.kind = "refresh-result"
  /\ s.event.gen = s.refreshGen /\ s.event.outcome = "install"
  /\ s' = [s EXCEPT
           !.event = NoEvent, !.active = NoLease, !.refreshGen = 0,
           !.uninstalled = FALSE,
           !.debt = IF s.debt = "inflight"
                      THEN IF s.late THEN "pending" ELSE "clean"
                      ELSE s.debt,
           !.late = FALSE,
           !.uiObserved = 0, !.pendingDraw = NoDraw]

\* A definite read refusal parks the demand until a new hint or explicit refresh re-arms it.
ApplyRefreshRefusal ==
  /\ s.status = "live" /\ s.event # NoEvent /\ s.event.kind = "refresh-result"
  /\ s.event.gen = s.refreshGen /\ s.event.outcome = "refuse"
  /\ s' = [s EXCEPT
           !.event = NoEvent, !.active = NoLease, !.refreshGen = 0,
           !.debt = IF s.debt = "inflight" THEN "parked" ELSE s.debt,
           !.late = FALSE]

ApplyRefreshDivergence ==
  /\ s.status = "live" /\ s.event # NoEvent /\ s.event.kind = "refresh-result"
  /\ s.event.gen = s.refreshGen /\ s.event.outcome = "diverge"
  /\ s' = Retired(s)

\* A refresh-result event is always published under the single live flight's generation, so
\* a stale refresh result is unrepresentable here: whichever step clears refreshGen also
\* consumes the event, and expiry retires the session instead of dropping the cell.

RefreshDeadlineExpires ==
  /\ s.status = "live" /\ s.refreshGen # 0
  /\ s' = Retired(s)

\* --- structural pressure and local drafts -------------------------------------------

\* A panel-focus Switch moves desired without clearing the UI's observed serial; stale Poll
\* cells are abandoned at admission. This asymmetry with SelectTab is the stall origin.
PanelFocusSwitch ==
  /\ s.status = "live" /\ ~s.switching /\ ~s.draftOpen /\ ~s.mailboxBusy
  /\ s.refreshGen = 0 /\ Len(s.queue) < MaxQueue /\ ~SerialExhausted
  /\ s.switchBudget > 0
  /\ LET next == Other(s.desired) IN
       s' = [s EXCEPT
             !.switchFrom = s.desired, !.desired = next,
             !.switching = TRUE, !.ready = FALSE, !.switchBudget = @ - 1,
             !.queue = Append(@, [kind |-> "switch", serial |-> s.nextSerial + 1]),
             !.nextSerial = s.nextSerial + 1,
             !.event = IF s.event # NoEvent /\ s.event.kind = "poll-result"
                          /\ s.event.key # next
                         THEN NoEvent ELSE s.event,
             !.causal = IF s.causal # NoCausal /\ s.causal.key # next
                          THEN NoCausal ELSE s.causal]

\* SelectTab clears the observed serial and pending draw on admission; panel focus does not.
SelectTabSwitch ==
  /\ s.status = "live" /\ ~s.switching /\ ~s.draftOpen /\ ~s.mailboxBusy
  /\ s.refreshGen = 0 /\ Len(s.queue) < MaxQueue /\ ~SerialExhausted
  /\ s.switchBudget > 0
  /\ LET next == Other(s.desired) IN
       s' = [s EXCEPT
             !.switchFrom = s.desired, !.desired = next,
             !.switching = TRUE, !.ready = FALSE, !.switchBudget = @ - 1,
             !.queue = Append(@, [kind |-> "switch", serial |-> s.nextSerial + 1]),
             !.nextSerial = s.nextSerial + 1,
             !.event = IF s.event # NoEvent /\ s.event.kind = "poll-result"
                          /\ s.event.key # next
                         THEN NoEvent ELSE s.event,
             !.causal = IF s.causal # NoCausal /\ s.causal.key # next
                          THEN NoCausal ELSE s.causal,
             !.uiObserved = 0, !.pendingDraw = NoDraw]

\* A queued Switch marker is drained by the executor before later commands run.
ExecSwitch ==
  /\ s.status = "live" /\ s.active = NoLease /\ s.queue # <<>>
  /\ Head(s.queue).kind = "switch"
  /\ s' = [s EXCEPT !.queue = Tail(@)]

\* The Switch settles without installing: the proven anchor restores the old attachment and
\* Ready polling resumes.
SwitchRestoresAnchor ==
  /\ s.status = "live" /\ s.switching
  /\ s' = [s EXCEPT !.desired = s.switchFrom, !.switchFrom = NoKey,
                    !.switching = FALSE, !.ready = TRUE]

SwitchInstalls ==
  /\ s.status = "live" /\ s.switching
  /\ s' = [s EXCEPT !.switching = FALSE, !.ready = TRUE, !.switchFrom = NoKey]

SwitchSettlesNoViewer ==
  /\ s.status = "live" /\ s.switching
  /\ s' = [s EXCEPT !.switching = FALSE, !.ready = FALSE, !.switchFrom = NoKey]

DraftOpens ==
  /\ s.status = "live" /\ ~s.draftOpen /\ s.draftBudget > 0
  /\ s.refreshGen = 0 /\ ~s.switching /\ ~s.mailboxBusy
  /\ s' = [s EXCEPT !.draftOpen = TRUE, !.draftBudget = @ - 1]

DraftCloses ==
  /\ s.status = "live" /\ s.draftOpen
  /\ s' = [s EXCEPT !.draftOpen = FALSE]

MailboxBusyOn ==
  /\ s.status = "live" /\ ~s.mailboxBusy /\ s.busyBudget > 0
  /\ s.refreshGen = 0 /\ ~s.switching
  /\ s' = [s EXCEPT !.mailboxBusy = TRUE, !.busyBudget = @ - 1]

MailboxBusyOff ==
  /\ s.status = "live" /\ s.mailboxBusy
  /\ s' = [s EXCEPT !.mailboxBusy = FALSE]

\* --- UI observation reconciliation ---------------------------------------------------

\* At every stable tick the observed serial is reconciled against canonical occupancy: a
\* serial that is neither queued nor executing is cleared so scheduling can resume.
ReconcileObservedPoll ==
  /\ s.status = "live" /\ s.uiObserved # 0 /\ ~PollOutstanding(s.uiObserved)
  /\ s' = [s EXCEPT !.uiObserved = 0]

\* --- lifecycle ------------------------------------------------------------------------

SessionRetires ==
  /\ s.status = "live"
  /\ s' = Retired(s)

SessionExits ==
  /\ s.status = "live"
  /\ s' = Retired(s)

Next ==
  PeerPublishes
  \/ SchedulePoll \/ PollAdmissionBusy \/ PollAdmissionFull \/ IdentityExhausted
  \/ Recv \/ PollSkippedStale \/ PollProbes \/ EnterPollPublish \/ PlacePollResult
  \/ StalePublishFinish \/ ConsumeEmptyPollResult \/ ConsumeFramedPollResult
  \/ SettleDraw \/ FinishPollLease \/ PublishFeedbackAccepted \/ DiscardStaleFeedback
  \/ ConsumeFeedbackAccepted \/ PollWireExpires
  \/ CaptureHint \/ DrainPendingRefresh \/ DrainRefusedBusy \/ DrainRefusedFull
  \/ RearmParkedRefresh \/ ManualRefreshClean \/ ManualRefreshWhileFlightLive
  \/ CompleteRefreshRead \/ ApplyRefreshInstall \/ ApplyRefreshRefusal
  \/ ApplyRefreshDivergence \/ RefreshDeadlineExpires
  \/ PanelFocusSwitch \/ SelectTabSwitch \/ ExecSwitch \/ SwitchRestoresAnchor
  \/ SwitchInstalls \/ SwitchSettlesNoViewer
  \/ DraftOpens \/ DraftCloses \/ MailboxBusyOn \/ MailboxBusyOff
  \/ ReconcileObservedPoll \/ SessionRetires \/ SessionExits

\* The single executor thread, the UI's stable consumption points and the event-driven
\* drain are weakly fair: a continuously enabled machine step eventually fires. User and
\* peer inputs (switches, drafts, busy windows, publications) are not forced.
Spec ==
  Init /\ [][Next]_vars
  /\ WF_vars(Recv) /\ WF_vars(ExecSwitch)
  /\ WF_vars(PollSkippedStale) /\ WF_vars(PollProbes) /\ WF_vars(EnterPollPublish)
  /\ WF_vars(PlacePollResult) /\ WF_vars(StalePublishFinish)
  /\ WF_vars(ConsumeEmptyPollResult) /\ WF_vars(ConsumeFramedPollResult)
  /\ WF_vars(SettleDraw) /\ WF_vars(FinishPollLease)
  /\ WF_vars(PublishFeedbackAccepted) /\ WF_vars(DiscardStaleFeedback)
  /\ WF_vars(ConsumeFeedbackAccepted)
  /\ WF_vars(CaptureHint) /\ WF_vars(ReconcileObservedPoll)
  /\ WF_vars(DrainPendingRefresh) /\ WF_vars(RearmParkedRefresh)
  /\ WF_vars(CompleteRefreshRead)
  /\ WF_vars(ApplyRefreshInstall) /\ WF_vars(ApplyRefreshRefusal)
  /\ WF_vars(ApplyRefreshDivergence)
  /\ WF_vars(RefreshDeadlineExpires) /\ WF_vars(PollWireExpires)
  /\ WF_vars(DraftCloses) /\ WF_vars(MailboxBusyOff)
  /\ WF_vars(IdentityExhausted)
  /\ WF_vars(SwitchRestoresAnchor) /\ WF_vars(SwitchInstalls)
  /\ WF_vars(SwitchSettlesNoViewer)

TypeOK ==
  /\ s.status \in {"live", "retired"}
  /\ s.desired \in Keys
  /\ s.switchFrom \in Keys \cup {NoKey}
  /\ s.ready \in BOOLEAN /\ s.switching \in BOOLEAN /\ s.draftOpen \in BOOLEAN
  /\ s.mailboxBusy \in BOOLEAN
  /\ s.draftBudget \in 0..1 /\ s.busyBudget \in 0..1 /\ s.switchBudget \in 0..1
  /\ Len(s.queue) <= MaxQueue
  /\ \A i \in DOMAIN s.queue :
       s.queue[i] \in QueueRecords \cup [kind : {"switch"}, serial : 1..MaxSerial]
  /\ s.nextSerial \in 0..MaxSerial
  /\ s.active \in LeaseRecords \cup {NoLease}
  /\ s.pollExec \in PollStages
  /\ s.pollHint \in BOOLEAN
  /\ s.event \in EventRecords \cup {NoEvent}
  /\ s.causal \in CausalRecords \cup {NoCausal}
  /\ s.latch \in BOOLEAN /\ s.outstanding \in BOOLEAN /\ s.uninstalled \in BOOLEAN
  /\ s.cursor \in 0..MaxGeneration /\ s.refreshGen \in 0..MaxGeneration
  /\ s.uiObserved \in 0..MaxSerial
  /\ s.pendingDraw \in PendingDraws \cup {NoDraw}
  /\ s.debt \in DebtStates /\ s.late \in BOOLEAN

\* At most one ordinary Poll is ever queued or executing per Session.
AtMostOneOrdinaryPoll ==
  Cardinality({i \in DOMAIN s.queue : s.queue[i].kind = "poll"})
    + (IF ActivePoll THEN 1 ELSE 0) <= 1

\* A folded but uninstalled publication is never forgotten while the session lives: it is
\* either latched at ingress or tracked by the canonical debt.
DemandNeverLost ==
  (s.status = "live" /\ s.uninstalled) => s.latch \/ s.debt # "clean" \/ s.pollHint

InFlightDebtHasFlight == s.debt = "inflight" => s.refreshGen # 0
LateOnlyInFlight == s.late => s.debt = "inflight"

\* A live refresh flight is always physically tracked: queued, executing, or awaiting UI.
FlightIsTracked == s.refreshGen # 0 => RefreshTracked

RetiredIsClean ==
  s.status = "retired" =>
    /\ ~s.latch /\ s.debt = "clean" /\ s.refreshGen = 0 /\ s.uiObserved = 0
    /\ ~s.uninstalled /\ s.queue = <<>> /\ s.active = NoLease /\ ~s.outstanding

PollResultHasCausal ==
  (s.event # NoEvent /\ s.event.kind = "poll-result") =>
    s.causal # NoCausal /\ s.causal.serial = s.event.serial

PollLeaseCoherent ==
  /\ ActivePoll => s.pollExec \in {"probing", "publish-entry", "publishing",
                                   "awaiting-causal"}
  /\ s.pollExec # "idle" => ActivePoll

\* A pending debt drains: bounded blocking plus a fair tick admits the refresh or the
\* session ends first.
PendingDebtDrains == (s.debt = "pending") ~> (s.debt # "pending")

\* A stale observed serial is reconciled so ordinary polling can resume.
StaleObservationReconciled ==
  (s.uiObserved # 0 /\ ~PollOutstanding(s.uiObserved)) ~> (s.uiObserved = 0)

\* A latched hint reaches the canonical debt unless the session ends first.
LatchReachesDebt == s.latch ~> (s.debt # "clean" \/ s.status # "live")

\* A live refresh flight always settles.
RefreshFlightSettles == s.refreshGen # 0 ~> s.refreshGen = 0
=============================================================================
