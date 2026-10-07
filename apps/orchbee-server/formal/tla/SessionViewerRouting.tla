-------------------------- MODULE SessionViewerRouting --------------------------
EXTENDS Naturals, FiniteSets

\* Two Sessions, two already-authorized retained targets on one Worker/Host,
\* three never-reused viewer and grant identities, and one ordered flight per
\* Session. Independent Session/Worker limits are both two, refining runtime
\* limits of eight through boundary tests. A replacement Worker invalidates
\* these generation-one bindings; restart adoption is deliberately absent.
\* Scenario variants separate frame routing, control sequences and their
\* cleanup composition. No state constraint removes reachable failures.
\* Settled flights and retired projection metadata are normalized: future
\* routing uses retained viewer/grant identities and cleanup evidence, never
\* a completed flight or a permanently removed Viewer's frame counters.
\* Existing selection/authority/relay models and tests own target resolution,
\* wire bytes, the three relay hops, Host effects and physical cleanup queues.
\* Disposal is an external exact completed proof, never a routing operation.
\* The routing variant retains a third viewer identity to exercise shared-capacity
\* refusal and never-reused identity allocation. Control and cleanup need only two
\* identities to exercise independent controllers, equal/gapped sequences, and
\* sibling cleanup; their smaller identity domain keeps those composed proofs
\* tractable without changing any production bound.
\* The kind-53 history lane is a second pollable record source: a pending event
\* is deliverable on Poll even while a frame flight is unsettled, it never
\* settles frame feedback, and it is a strict route error on any non-Poll
\* action. The relay model owns FIFO depth, byte accounting, and source
\* arbitration; this model owns the routing-level delivery and rejection shape.
CONSTANT Scenario, FirstSession, FirstTarget, SecondSession, SecondTarget, FirstGrantViewer
CONSTANT MaxRoutingHistory
ASSUME Scenario \in {"routing", "control", "cleanup"}
Sessions == {"a", "b"}
Targets == {"x", "y"}
ASSUME /\ FirstSession \in Sessions \cup {"any"}
  /\ SecondSession \in Sessions \cup {"any"}
  /\ FirstTarget \in Targets \cup {"any"}
  /\ SecondTarget \in Targets \cup {"any"}
  /\ FirstGrantViewer \in 0..2
Viewers == IF Scenario = "routing" THEN 1..3 ELSE 1..2
Grants == 1..3
SessionLimit == 2
WorkerLimit == 2
WithFrames == Scenario \in {"routing", "cleanup"}
WithControl == Scenario \in {"control", "cleanup"}
MaxSequence == IF Scenario = "control" THEN 2 ELSE 1
MaxFrame == IF Scenario = "routing" THEN 2 ELSE 1
MaxHistory == IF Scenario = "routing" THEN MaxRoutingHistory ELSE 1
Registered == {"candidate", "issued", "acknowledged", "active"}
GrantHeld == {"reserved", "active", "closing"}
NoViewer == [owner |-> "none", target |-> "none", generation |-> 0,
             state |-> "unused", grant |-> 0]
NoGrant == [owner |-> "none", viewer |-> 0, target |-> "none", generation |-> 0,
            state |-> "unused", sequence |-> 0]
NoFrame == [viewer |-> 0, host |-> 0, sequence |-> 0, generation |-> 0, revision |-> 0, kind |-> "none"]
NoFlight == [viewer |-> 0, grant |-> 0, kind |-> "none", sequence |-> 0]
Frame(v, seq, kind) == [viewer |-> v, host |-> 1, sequence |-> seq, generation |-> 1, revision |-> seq, kind |-> kind]

VARIABLE s
vars == <<s>>
Init == s = [live |-> [a \in Sessions |-> TRUE], phase |-> [a \in Sessions |-> "idle"],
  flight |-> [a \in Sessions |-> NoFlight], viewers |-> [v \in Viewers |-> NoViewer],
  grants |-> [g \in Grants |-> NoGrant], cursor |-> 0, grantCursor |-> 0,
  workerGeneration |-> 1, workerOwned |-> {},
  disposed |-> {},
  pending |-> [v \in Viewers |-> NoFrame], baseline |-> [v \in Viewers |-> FALSE],
  frameSequence |-> [v \in Viewers |-> 0],
  history |-> [v \in Viewers |-> 0], historyFlight |-> [v \in Viewers |-> 0],
  historySequence |-> [v \in Viewers |-> 0]]

Idle(a) == s.live[a] /\ s.phase[a] = "idle"
Owned(a, v) == IF v \in Viewers THEN s.viewers[v].owner = a /\ s.viewers[v].state = "active" ELSE FALSE
Current(v) == s.viewers[v].generation = s.workerGeneration /\ s.viewers[v].target \notin s.disposed
Routable(a, v) == Idle(a) /\ Owned(a, v) /\ Current(v)
PossibleViewer(v) == s.viewers[v].state \in {"issued", "acknowledged", "active", "closing"}
SessionCount(a) == Cardinality({v \in Viewers: s.viewers[v].owner = a /\ s.viewers[v].state \in Registered})
Duplicate(a, target) == \E v \in Viewers:
  s.viewers[v].owner = a /\ s.viewers[v].state \in Registered /\ s.viewers[v].target = target
HeldOn(target) == {g \in Grants: s.grants[g].target = target /\ s.grants[g].state \in GrantHeld}
GrantsOf(v) == {g \in Grants: s.grants[g].viewer = v /\ s.grants[g].state \in GrantHeld}
V(a) == s.flight[a].viewer
G(a) == s.flight[a].grant
ActiveGrant(v) == IF s.viewers[v].grant = 0 THEN FALSE
  ELSE s.grants[s.viewers[v].grant].state = "active"
Projection(v) == <<s.viewers[v], s.pending[v], s.baseline[v], s.frameSequence[v],
  s.history[v], s.historyFlight[v], s.historySequence[v],
  {s.grants[g]: g \in {id \in Grants: s.grants[id].viewer = v}}>>

\* Partition only the first two admitted identities. Eight representatives
\* fix the first Session/target to a/x and cover the complete 32 assignments
\* under independent Session/target renaming; viewer/grant order is unchanged.
\* The owning README records the graph bijection and its fairness obligations.
\* Traces with fewer operations occur in several partitions. Neither admission
\* nor reservation has a fairness obligation; cleanup/flight fairness is
\* unchanged. "any" and grant viewer 0 retain unrestricted routing/control.
AttachPartitionMatches(a, target) ==
  IF s.cursor = 0 THEN
    (FirstSession = "any" \/ a = FirstSession) /\ (FirstTarget = "any" \/ target = FirstTarget)
  ELSE IF s.cursor = 1 THEN
    (SecondSession = "any" \/ a = SecondSession) /\ (SecondTarget = "any" \/ target = SecondTarget)
  ELSE TRUE

AdmitAttach(a, target) ==
  /\ Idle(a) /\ target \in Targets /\ SessionCount(a) < SessionLimit
  /\ ~Duplicate(a, target) /\ target \notin s.disposed /\ s.workerGeneration = 1
  /\ s.cursor < Cardinality(Viewers)
  /\ AttachPartitionMatches(a, target)
  /\ s' = [s EXCEPT !.cursor = @+1, !.phase[a] = "attach-admitted",
       !.flight[a] = [viewer |-> s.cursor+1, grant |-> 0, kind |-> "attach", sequence |-> 0],
       !.viewers[s.cursor+1] = [owner |-> a, target |-> target, generation |-> 1, state |-> "candidate", grant |-> 0]]
RejectSessionCapacity(a) == /\ Idle(a) /\ SessionCount(a) = SessionLimit /\ UNCHANGED vars
RejectFullSessionDuplicate(a, target) ==
  /\ target \in Targets /\ Duplicate(a, target) /\ RejectSessionCapacity(a)
RejectDuplicateTarget(a, target) ==
  /\ Idle(a) /\ SessionCount(a) < SessionLimit /\ target \in Targets /\ Duplicate(a, target)
  /\ UNCHANGED vars
DispatchAttach(a) ==
  /\ s.live[a] /\ s.phase[a] = "attach-admitted"
  /\ s' = [s EXCEPT !.phase[a] = "attach-issued", !.viewers[V(a)].state = "issued"]
AttachDefiniteRefusalCore(a) ==
  /\ s.live[a] /\ s.phase[a] = "attach-issued" /\ V(a) \notin s.workerOwned
  /\ s' = [s EXCEPT !.phase[a] = "idle", !.flight[a] = NoFlight, !.viewers[V(a)].state = "rejected"]
AttachDefiniteRefusal(a) ==
  /\ Cardinality(s.workerOwned) # WorkerLimit
  /\ AttachDefiniteRefusalCore(a)
RejectWorkerCapacity(a) ==
  /\ Cardinality(s.workerOwned) = WorkerLimit
  /\ AttachDefiniteRefusalCore(a)
WorkerAcceptAttach(a) ==
  /\ s.live[a] /\ s.phase[a] = "attach-issued" /\ Current(V(a))
  /\ Cardinality(s.workerOwned) < WorkerLimit
  /\ s' = [s EXCEPT !.phase[a] = "attach-acknowledged", !.viewers[V(a)].state = "acknowledged",
       !.workerOwned = @ \cup {V(a)}]
InstallAttach(a) ==
  /\ s.live[a] /\ s.phase[a] = "attach-acknowledged" /\ Current(V(a))
  /\ s' = [s EXCEPT !.phase[a] = "attach-published", !.viewers[V(a)].state = "active"]
PublishAttach(a) ==
  /\ s.live[a] /\ s.phase[a] = "attach-published"
  /\ s' = [s EXCEPT !.phase[a] = "idle", !.flight[a] = NoFlight]
CloseViewer(state, v) == [state EXCEPT !.viewers[v].state = "closing", !.pending[v] = NoFrame,
  !.baseline[v] = FALSE, !.frameSequence[v] = 0,
  !.history[v] = 0, !.historyFlight[v] = 0, !.historySequence[v] = 0,
  !.grants = [g \in Grants |-> IF state.grants[g].viewer = v /\ state.grants[g].state \in GrantHeld
    THEN [state.grants[g] EXCEPT !.state = "closing"] ELSE state.grants[g]]]
AttachUnknown(a) ==
  /\ s.live[a] /\ s.phase[a] \in {"attach-issued", "attach-acknowledged"}
  /\ s' = CloseViewer([s EXCEPT !.phase[a] = "idle", !.flight[a] = NoFlight], V(a))
RejectInstallFence(a) ==
  /\ s.live[a] /\ s.phase[a] = "attach-acknowledged" /\ ~Current(V(a))
  /\ AttachUnknown(a)

RejectForeignViewer(a, v) ==
  /\ Idle(a) /\ v \in Viewers /\ s.viewers[v].state = "active" /\ s.viewers[v].owner # a
  /\ UNCHANGED vars
RejectUnknownViewer(a) == /\ Idle(a) /\ UNCHANGED vars
RejectStaleViewer(a, v) ==
  /\ Idle(a) /\ v \in Viewers /\ s.viewers[v].owner = a
  /\ s.viewers[v].state \in {"closing", "retired", "rejected"} /\ UNCHANGED vars
AbsentDetach(a, v) == /\ Idle(a) /\ v \in 0..3 /\ ~Owned(a, v) /\ UNCHANGED vars
DetachViewer(a, v) == /\ Routable(a, v) /\ s' = CloseViewer(s, v)
RejectDisposedRoute(a, v) ==
  /\ Idle(a) /\ Owned(a, v) /\ s.viewers[v].target \in s.disposed
  /\ s' = CloseViewer(s, v)
RejectStaleGeneration(a, v) ==
  /\ Idle(a) /\ Owned(a, v) /\ s.viewers[v].generation # s.workerGeneration
  /\ s' = CloseViewer(s, v)
ReapDisposed(a) ==
  /\ Idle(a) /\ \E v \in Viewers: Owned(a, v) /\ s.viewers[v].target \in s.disposed
  /\ s' = [s EXCEPT !.viewers = [v \in Viewers |->
       IF Owned(a, v) /\ s.viewers[v].target \in s.disposed
       THEN [s.viewers[v] EXCEPT !.state = "closing"] ELSE s.viewers[v]],
       !.pending = [v \in Viewers |-> IF Owned(a, v) /\ s.viewers[v].target \in s.disposed THEN NoFrame ELSE s.pending[v]],
       !.baseline = [v \in Viewers |-> IF Owned(a, v) /\ s.viewers[v].target \in s.disposed THEN FALSE ELSE s.baseline[v]],
       !.frameSequence = [v \in Viewers |-> IF Owned(a, v) /\ s.viewers[v].target \in s.disposed THEN 0 ELSE s.frameSequence[v]],
       !.history = [v \in Viewers |-> IF Owned(a, v) /\ s.viewers[v].target \in s.disposed THEN 0 ELSE s.history[v]],
       !.historyFlight = [v \in Viewers |-> IF Owned(a, v) /\ s.viewers[v].target \in s.disposed THEN 0 ELSE s.historyFlight[v]],
       !.historySequence = [v \in Viewers |-> IF Owned(a, v) /\ s.viewers[v].target \in s.disposed THEN 0 ELSE s.historySequence[v]]]

PollRecord(a, v) ==
  /\ WithFrames /\ Routable(a, v) /\ s.pending[v] = NoFrame /\ s.frameSequence[v] < MaxFrame
  /\ s' = [s EXCEPT !.pending[v] = Frame(v, s.frameSequence[v]+1, IF s.baseline[v] THEN "delta" ELSE "snapshot"),
       !.frameSequence[v] = @+1, !.baseline[v] = TRUE]
PollWhileSiblingPending(a, v) ==
  /\ \E other \in Viewers: other # v /\ Owned(a, other) /\ s.pending[other] # NoFrame
  /\ PollRecord(a, v)
\* Poll is no longer blocked globally by an unsettled frame flight: a queued
\* kind-53 event is always an eligible Poll result. The flight alone still
\* produces no frame record, so an empty poll stays empty only when neither
\* source has anything to return.
PollAwaitingFeedback(a, v) == /\ WithFrames /\ Routable(a, v) /\ s.pending[v] # NoFrame
  /\ s.history[v] = 0 /\ UNCHANGED vars

\* The Host signals history growth upstream; the worker holds at most one
\* routed event here (the relay model owns the bounded FIFO refinement).
EmitHistoryChanged(a, v) ==
  /\ WithFrames /\ Routable(a, v) /\ s.history[v] = 0
  /\ s.historySequence[v] < MaxHistory
  /\ s' = [s EXCEPT !.history[v] = s.historySequence[v]+1,
       !.historySequence[v] = @+1]

\* Poll serves the pending kind-53 record and starts the independent
\* HistoryObserved flight; the frame flight is not consulted or settled.
PollHistory(a, v) ==
  /\ WithFrames /\ Routable(a, v) /\ s.history[v] # 0
  /\ s' = [s EXCEPT !.history[v] = 0, !.historyFlight[v] = s.history[v]]

\* An exact HistoryObserved echo settles only the history flight.
AcceptHistoryObserved(a, v) ==
  /\ WithFrames /\ Routable(a, v) /\ s.historyFlight[v] # 0
  /\ s' = [s EXCEPT !.historyFlight[v] = 0]

\* A stale or duplicate HistoryObserved carries no authority and is dropped.
RejectStaleHistoryObserved(a, v) ==
  /\ WithFrames /\ Routable(a, v) /\ s.historyFlight[v] = 0
  /\ s.historySequence[v] > 0 /\ UNCHANGED vars

\* A kind-53 record surfacing on any non-Poll action is a strict route error
\* and retires the offending viewer, never a sibling.
RejectHistoryWrongRoute(a, v) ==
  /\ WithFrames /\ Routable(a, v) /\ s.history[v] # 0
  /\ s' = CloseViewer(s, v)
AcceptFeedback(a, v, kind) ==
  /\ WithFrames /\ Routable(a, v) /\ kind \in {"applied", "snapshot"} /\ s.pending[v] # NoFrame
  /\ s.pending[v].viewer = v /\ s.pending[v].host = 1
  /\ s' = [s EXCEPT !.pending[v] = NoFrame, !.baseline[v] = kind # "snapshot"]
FrameApplied(a, v) == /\ WithFrames /\ AcceptFeedback(a, v, "applied")
NeedSnapshot(a, v) == /\ WithFrames /\ AcceptFeedback(a, v, "snapshot")
AcceptEqualCounterSiblingFeedback(a, v) ==
  /\ \E other \in Viewers: other # v /\ Owned(a, other) /\ s.pending[other] # NoFrame
       /\ s.pending[other].sequence = s.pending[v].sequence
       /\ s.pending[other].generation = s.pending[v].generation /\ s.pending[other].revision = s.pending[v].revision
  /\ FrameApplied(a, v)
RejectFeedbackMismatch(a, v) == /\ WithFrames /\ Routable(a, v) /\ s.pending[v] # NoFrame /\ UNCHANGED vars
RejectWrongRecordRoute(a, v) ==
  /\ WithFrames /\ Routable(a, v) /\ s.pending[v] = NoFrame
  /\ s' = CloseViewer(s, v)

ReserveController(a, v) ==
  /\ WithControl /\ Routable(a, v) /\ HeldOn(s.viewers[v].target) = {} /\ s.grantCursor < 3
  /\ (FirstGrantViewer = 0 \/ s.grantCursor # 0 \/ v = FirstGrantViewer)
  /\ s' = [s EXCEPT !.grantCursor = @+1, !.phase[a] = "acquire", !.viewers[v].grant = s.grantCursor+1,
       !.flight[a] = [viewer |-> v, grant |-> s.grantCursor+1, kind |-> "acquire", sequence |-> 0],
       !.grants[s.grantCursor+1] = [owner |-> a, viewer |-> v, target |-> s.viewers[v].target,
         generation |-> s.viewers[v].generation, state |-> "reserved", sequence |-> 0]]
RejectControllerBusy(a, v) == /\ WithControl /\ Routable(a, v) /\ HeldOn(s.viewers[v].target) # {} /\ UNCHANGED vars
AcceptControllerCore(a) ==
  /\ s.live[a] /\ s.phase[a] = "acquire" /\ Current(V(a)) /\ s.grants[G(a)].state = "reserved"
  /\ s' = [s EXCEPT !.phase[a] = "idle", !.flight[a] = NoFlight, !.grants[G(a)].state = "active"]
NextRenewedGrant(a) == IF s.phase[a] = "acquire" THEN
  \E old \in Grants: old # G(a) /\ s.grants[old].target = s.grants[G(a)].target
    /\ s.grants[old].state = "released" /\ s.grants[old].sequence > 0
  ELSE FALSE
AcceptController(a) == /\ ~NextRenewedGrant(a) /\ AcceptControllerCore(a)
NewGrantResetsSequence(a) ==
  /\ NextRenewedGrant(a) /\ AcceptControllerCore(a)
RejectControllerAcquire(a) ==
  /\ s.live[a] /\ s.phase[a] = "acquire" /\ s.grants[G(a)].state = "reserved"
  /\ s' = [s EXCEPT !.phase[a] = "idle", !.flight[a] = NoFlight, !.grants[G(a)].state = "released"]
UnknownControllerAcquire(a) ==
  /\ s.live[a] /\ s.phase[a] = "acquire"
  /\ s' = [s EXCEPT !.phase[a] = "idle", !.flight[a] = NoFlight, !.grants[G(a)].state = IF @ = "reserved" THEN "closing" ELSE @]
BeginControlCore(a, v, kind, sequence) ==
  /\ WithControl /\ Routable(a, v) /\ ActiveGrant(v) /\ kind \in {"input", "resize"}
  /\ sequence \in 1..MaxSequence /\ sequence > s.grants[s.viewers[v].grant].sequence
  /\ s' = [s EXCEPT !.phase[a] = "control", !.grants[s.viewers[v].grant].sequence = sequence,
       !.flight[a] = [viewer |-> v, grant |-> s.viewers[v].grant, kind |-> kind, sequence |-> sequence]]
NextInputGap(v, sequence) == IF ActiveGrant(v)
  THEN sequence > s.grants[s.viewers[v].grant].sequence+1 ELSE FALSE
NextInputEqual(a, v, sequence) ==
  \E other \in Viewers: other # v /\ Owned(a, other) /\ ActiveGrant(other)
    /\ s.grants[s.viewers[other].grant].sequence = sequence
BeginInput(a, v, sequence) ==
  /\ ~NextInputGap(v, sequence) /\ ~NextInputEqual(a, v, sequence)
  /\ BeginControlCore(a, v, "input", sequence)
BeginResize(a, v, sequence) == /\ WithControl /\ BeginControlCore(a, v, "resize", sequence)
AcceptSequenceGap(a, v, sequence) ==
  /\ NextInputGap(v, sequence) /\ BeginControlCore(a, v, "input", sequence)
AcceptIndependentEqualSequence(a, v, sequence) ==
  /\ NextInputEqual(a, v, sequence) /\ BeginControlCore(a, v, "input", sequence)
RejectWrongGrant(a, v) == /\ WithControl /\ Routable(a, v) /\ UNCHANGED vars
RejectOldGrant(a, v) ==
  /\ WithControl /\ Routable(a, v) /\ ActiveGrant(v)
  /\ \E old \in Grants: old # s.viewers[v].grant /\ s.grants[old].state = "released"
  /\ UNCHANGED vars
RejectStaleSequence(a, v) ==
  /\ WithControl /\ Routable(a, v) /\ ActiveGrant(v) /\ s.grants[s.viewers[v].grant].sequence > 0
  /\ UNCHANGED vars
CommitControl(a) ==
  /\ s.live[a] /\ s.phase[a] = "control" /\ Current(V(a)) /\ s.grants[G(a)].state = "active"
  /\ s' = [s EXCEPT !.phase[a] = "control-committed"]
CompleteControl(a) == /\ s.live[a] /\ s.phase[a] = "control-committed" /\ s' = [s EXCEPT !.phase[a] = "idle", !.flight[a] = NoFlight]
ControlBusyConsumesSequence(a) ==
  /\ s.live[a] /\ s.phase[a] = "control"
  /\ s' = [s EXCEPT !.phase[a] = "idle", !.flight[a] = NoFlight]
ControlOutcomeUnknown(a) ==
  /\ s.live[a] /\ s.phase[a] \in {"control", "control-committed"}
  /\ s' = [s EXCEPT !.phase[a] = "idle", !.flight[a] = NoFlight, !.grants[G(a)].state = IF @ = "active" THEN "closing" ELSE @]

DisposeTarget(target) ==
  /\ Scenario = "cleanup" /\ target \in Targets /\ target \notin s.disposed
  /\ s' = [s EXCEPT !.disposed = @ \cup {target},
       !.workerOwned = {v \in s.workerOwned: s.viewers[v].target # target},
       !.grants = [g \in Grants |-> IF s.grants[g].target = target /\ s.grants[g].state \in GrantHeld
         THEN [s.grants[g] EXCEPT !.state = "closing"] ELSE s.grants[g]]]
ReplaceWorker ==
  /\ Scenario = "cleanup" /\ s.workerGeneration = 1
  /\ s' = [s EXCEPT !.workerGeneration = 2, !.workerOwned = {},
       !.grants = [g \in Grants |-> IF s.grants[g].state \in GrantHeld
         THEN [s.grants[g] EXCEPT !.state = "closing"] ELSE s.grants[g]]]
DropSessionCore(a) ==
  /\ s.live[a]
  /\ s' = [s EXCEPT !.live[a] = FALSE, !.phase[a] = "dead", !.flight[a] = NoFlight,
       !.viewers = [v \in Viewers |-> IF s.viewers[v].owner = a /\ s.viewers[v].state \in Registered
         THEN [s.viewers[v] EXCEPT !.state = IF PossibleViewer(v) THEN "closing" ELSE "retired"] ELSE s.viewers[v]],
       !.pending = [v \in Viewers |-> IF s.viewers[v].owner = a THEN NoFrame ELSE s.pending[v]],
       !.baseline = [v \in Viewers |-> IF s.viewers[v].owner = a THEN FALSE ELSE s.baseline[v]],
       !.frameSequence = [v \in Viewers |-> IF s.viewers[v].owner = a THEN 0 ELSE s.frameSequence[v]],
       !.history = [v \in Viewers |-> IF s.viewers[v].owner = a THEN 0 ELSE s.history[v]],
       !.historyFlight = [v \in Viewers |-> IF s.viewers[v].owner = a THEN 0 ELSE s.historyFlight[v]],
       !.historySequence = [v \in Viewers |-> IF s.viewers[v].owner = a THEN 0 ELSE s.historySequence[v]],
       !.grants = [g \in Grants |-> IF s.grants[g].owner = a /\ s.grants[g].state \in GrantHeld
         THEN [s.grants[g] EXCEPT !.state = "closing"] ELSE s.grants[g]]]
NextDropBranch(a) ==
  IF SessionCount(a) = 2 THEN "multiple"
  ELSE IF s.phase[a] = "attach-published" THEN "publication"
  ELSE IF s.phase[a] \in {"acquire", "control", "control-committed"} THEN "carrier"
  ELSE IF s.phase[a] \in {"attach-admitted", "attach-issued", "attach-acknowledged"} THEN "flight"
  ELSE "ordinary"
DropSession(a) == /\ NextDropBranch(a) = "ordinary" /\ DropSessionCore(a)
LoseAttachPublication(a) ==
  /\ NextDropBranch(a) = "publication" /\ DropSessionCore(a)
CarrierCorrelationFailure(a) ==
  /\ NextDropBranch(a) = "carrier" /\ DropSessionCore(a)
DropMultipleViewers(a) ==
  /\ NextDropBranch(a) = "multiple" /\ DropSessionCore(a)
ExpireFlight(a) ==
  /\ NextDropBranch(a) = "flight" /\ DropSessionCore(a)
ExpireFlightFair(a) ==
  /\ s.phase[a] \notin {"idle", "dead"} /\ DropSessionCore(a)
CleanupController(g) ==
  /\ g \in Grants /\ s.grants[g].state = "closing"
  /\ s.grants[g].generation = s.workerGeneration /\ s.grants[g].target \notin s.disposed
  /\ s' = [s EXCEPT !.grants[g].state = "released"]
ForgetUnavailableController(g) ==
  /\ g \in Grants /\ s.grants[g].state = "closing"
  /\ (s.grants[g].generation # s.workerGeneration \/ s.grants[g].target \in s.disposed)
  /\ s' = [s EXCEPT !.grants[g].state = "released"]
CleanupViewer(v) ==
  /\ v \in Viewers /\ s.viewers[v].state = "closing" /\ GrantsOf(v) = {}
  /\ Current(v)
  /\ s' = [s EXCEPT !.viewers[v].state = "retired", !.workerOwned = @ \ {v}]
ForgetUnavailableViewer(v) ==
  /\ v \in Viewers /\ s.viewers[v].state = "closing" /\ GrantsOf(v) = {} /\ ~Current(v)
  /\ s' = [s EXCEPT !.viewers[v].state = "retired", !.workerOwned = @ \ {v}]
DisposedCleanupSkipsWorker(v) ==
  /\ v \in Viewers /\ s.viewers[v].target \in s.disposed /\ ForgetUnavailableViewer(v)
StaleCleanupNeverUsesReplacement(v) ==
  /\ v \in Viewers /\ s.viewers[v].generation # s.workerGeneration /\ ForgetUnavailableViewer(v)
RejectAcquireDuringCleanup(a, v) ==
  /\ \E g \in Grants: s.grants[g].target = s.viewers[v].target /\ s.grants[g].state = "closing"
  /\ RejectControllerBusy(a, v)
CleanupAny == (\E g \in Grants: CleanupController(g) \/ ForgetUnavailableController(g))
  \/ (\E v \in Viewers: CleanupViewer(v) \/ ForgetUnavailableViewer(v))

Next ==
  \/ \E a \in Sessions, target \in Targets:
       AdmitAttach(a, target) \/ RejectFullSessionDuplicate(a, target) \/ RejectDuplicateTarget(a, target)
  \/ \E a \in Sessions: RejectSessionCapacity(a) \/ DispatchAttach(a) \/ AttachDefiniteRefusal(a)
       \/ RejectWorkerCapacity(a) \/ WorkerAcceptAttach(a) \/ InstallAttach(a) \/ PublishAttach(a)
       \/ AttachUnknown(a) \/ RejectInstallFence(a) \/ RejectUnknownViewer(a) \/ ReapDisposed(a)
       \/ AcceptController(a) \/ NewGrantResetsSequence(a) \/ RejectControllerAcquire(a)
       \/ UnknownControllerAcquire(a) \/ CommitControl(a) \/ CompleteControl(a)
       \/ ControlBusyConsumesSequence(a) \/ ControlOutcomeUnknown(a) \/ DropSession(a)
       \/ LoseAttachPublication(a) \/ CarrierCorrelationFailure(a) \/ DropMultipleViewers(a) \/ ExpireFlight(a)
  \/ \E a \in Sessions, v \in Viewers:
       RejectForeignViewer(a, v) \/ RejectStaleViewer(a, v) \/ DetachViewer(a, v)
       \/ RejectDisposedRoute(a, v) \/ RejectStaleGeneration(a, v) \/ PollRecord(a, v)
       \/ PollWhileSiblingPending(a, v) \/ PollAwaitingFeedback(a, v) \/ EmitHistoryChanged(a, v)
       \/ PollHistory(a, v) \/ AcceptHistoryObserved(a, v) \/ RejectStaleHistoryObserved(a, v)
       \/ RejectHistoryWrongRoute(a, v) \/ FrameApplied(a, v)
       \/ NeedSnapshot(a, v) \/ AcceptEqualCounterSiblingFeedback(a, v) \/ RejectFeedbackMismatch(a, v)
       \/ RejectWrongRecordRoute(a, v) \/ ReserveController(a, v) \/ RejectControllerBusy(a, v)
       \/ RejectWrongGrant(a, v) \/ RejectOldGrant(a, v) \/ RejectStaleSequence(a, v) \/ RejectAcquireDuringCleanup(a, v)
  \/ \E a \in Sessions, v \in 0..3: AbsentDetach(a, v)
  \/ \E a \in Sessions, v \in Viewers, sequence \in 1..MaxSequence:
       BeginInput(a, v, sequence) \/ BeginResize(a, v, sequence)
       \/ AcceptSequenceGap(a, v, sequence) \/ AcceptIndependentEqualSequence(a, v, sequence)
  \/ \E target \in Targets: DisposeTarget(target)
  \/ ReplaceWorker \/ CleanupAny
  \/ \E v \in Viewers: DisposedCleanupSkipsWorker(v) \/ StaleCleanupNeverUsesReplacement(v)

\* Existing bounded carrier waits settle or retire. Cleanup convergence assumes
\* a cleanup response eventually arrives or the pinned dependency is unavailable;
\* no fairness is asserted for Client polling, input or scheduling among Viewers.
Spec == Init /\ [][Next]_vars /\ WF_vars(CleanupAny)
  /\ (\A a \in Sessions: WF_vars(ExpireFlightFair(a)))

TypeOK ==
  /\ s.live \in [Sessions -> BOOLEAN]
  /\ s.phase \in [Sessions -> {"idle", "dead", "attach-admitted", "attach-issued", "attach-acknowledged",
       "attach-published", "acquire", "control", "control-committed"}]
  /\ s.flight \in [Sessions -> [viewer : 0..3, grant : 0..3, kind : {"none", "attach", "acquire", "input", "resize"}, sequence : 0..2]]
  /\ s.viewers \in [Viewers -> [owner : Sessions \cup {"none"}, target : Targets \cup {"none"}, generation : 0..1,
       state : Registered \cup {"unused", "closing", "retired", "rejected"}, grant : 0..3]]
  /\ s.grants \in [Grants -> [owner : Sessions \cup {"none"}, viewer : 0..3, target : Targets \cup {"none"}, generation : 0..1,
       state : GrantHeld \cup {"unused", "released"}, sequence : 0..2]]
  /\ s.cursor \in 0..3 /\ s.grantCursor \in 0..3 /\ s.workerGeneration \in 1..2
  /\ s.workerOwned \subseteq Viewers
  /\ s.disposed \subseteq Targets
  /\ s.pending \in [Viewers -> [viewer : 0..3, host : 0..1, sequence : 0..2, generation : 0..1, revision : 0..2,
       kind : {"none", "snapshot", "delta"}]]
  /\ s.baseline \in [Viewers -> BOOLEAN] /\ s.frameSequence \in [Viewers -> 0..2]
  /\ s.history \in [Viewers -> 0..MaxHistory]
  /\ s.historyFlight \in [Viewers -> 0..MaxHistory]
  /\ s.historySequence \in [Viewers -> 0..MaxHistory]
SeparateCapacityBounds == /\ \A a \in Sessions: SessionCount(a) <= SessionLimit
  /\ Cardinality(s.workerOwned) <= WorkerLimit
UniqueLiveTargetPerSession == \A a \in Sessions, target \in Targets:
  Cardinality({v \in Viewers: s.viewers[v].owner = a /\ s.viewers[v].target = target /\ s.viewers[v].state \in Registered}) <= 1
ExclusiveControllerIncludingCleanup == \A target \in Targets: Cardinality(HeldOn(target)) <= 1
ExactGrantOwnership == \A g \in Grants: s.grants[g].state # "unused" =>
  s.grants[g].owner = s.viewers[s.grants[g].viewer].owner
  /\ s.grants[g].target = s.viewers[s.grants[g].viewer].target
  /\ s.grants[g].generation = s.viewers[s.grants[g].viewer].generation
ExactPendingFrame == \A v \in Viewers: s.pending[v] # NoFrame =>
  s.viewers[v].state = "active" /\ s.pending[v].viewer = v /\ s.pending[v].host = 1
  /\ s.pending[v].sequence = s.frameSequence[v] /\ s.baseline[v]
CandidateOwnershipIsAccounted ==
  s.workerOwned \subseteq {v \in Viewers: PossibleViewer(v)}
RejectedAttachNeverOwnsCleanup ==
  \A v \in Viewers: s.viewers[v].state = "rejected" =>
    v \notin s.workerOwned
NoViewerCleanupAheadOfController ==
  \A v \in Viewers: s.viewers[v].state \in {"retired", "rejected"} => GrantsOf(v) = {}
DeadSessionHasNoRegisteredViewer == \A a \in Sessions: ~s.live[a] => SessionCount(a) = 0
RetiredViewerHasNoFlight == \A v \in Viewers: s.viewers[v].state \in {"retired", "rejected"} =>
  s.pending[v] = NoFrame /\ v \notin s.workerOwned
  /\ s.history[v] = 0 /\ s.historyFlight[v] = 0 /\ s.historySequence[v] = 0

\* A queued or delivered kind-53 event always belongs to an active viewer and
\* never outruns the events actually emitted for it.
ExactHistoryScope == \A v \in Viewers:
  /\ s.history[v] # 0 => s.viewers[v].state = "active" /\ s.history[v] = s.historySequence[v]
  /\ s.historyFlight[v] # 0 =>
       s.viewers[v].state = "active" /\ s.historyFlight[v] <= s.historySequence[v]

\* An unsettled frame flight never gates the history lane out of Poll: the
\* queued event stays an eligible Poll result the whole time it waits.
HistoryDeliverableDuringFrameFlight ==
  \A a \in Sessions, v \in Viewers:
    (WithFrames /\ Routable(a, v) /\ s.pending[v] # NoFrame /\ s.history[v] # 0)
      => ENABLED PollHistory(a, v)

AttachAllocatesFreshIdentity == [][\A a \in Sessions, target \in Targets: AdmitAttach(a, target) =>
  s'.cursor = s.cursor+1 /\ s.viewers[s'.cursor] = NoViewer /\ s'.baseline[s'.cursor] = FALSE]_vars
ViewerIdentityNeverRetargets == [][\A v \in 1..s.cursor:
  UNCHANGED <<s.viewers[v].owner, s.viewers[v].target, s.viewers[v].generation>>]_vars
ConsumedSequenceNeverRegresses == [][\A g \in Grants: s'.grants[g].sequence >= s.grants[g].sequence]_vars
ControlDispatchUsesExactOwner == [][\A a \in Sessions: CommitControl(a) =>
  Owned(a, V(a)) /\ Current(V(a)) /\ s.viewers[V(a)].grant = G(a)
  /\ s.grants[G(a)].sequence = s.flight[a].sequence]_vars
LocalRequest(a, v) == DetachViewer(a, v) \/ PollRecord(a, v) \/ PollAwaitingFeedback(a, v)
  \/ EmitHistoryChanged(a, v) \/ PollHistory(a, v) \/ AcceptHistoryObserved(a, v)
  \/ RejectStaleHistoryObserved(a, v) \/ RejectHistoryWrongRoute(a, v)
  \/ FrameApplied(a, v) \/ NeedSnapshot(a, v) \/ RejectFeedbackMismatch(a, v) \/ RejectWrongRecordRoute(a, v)
  \/ ReserveController(a, v) \/ RejectControllerBusy(a, v) \/ RejectWrongGrant(a, v) \/ RejectStaleSequence(a, v)
  \/ (\E sequence \in 1..MaxSequence:
       BeginInput(a, v, sequence) \/ BeginResize(a, v, sequence)
         \/ AcceptSequenceGap(a, v, sequence)
         \/ AcceptIndependentEqualSequence(a, v, sequence))
TargetLocalRequestsPreserveSiblings == [][\A a \in Sessions, v \in Viewers:
  LocalRequest(a, v) => \A other \in Viewers \ {v}: Projection(other)' = Projection(other)]_vars
LocalCompletion(a) == AcceptController(a) \/ RejectControllerAcquire(a) \/ UnknownControllerAcquire(a)
  \/ CommitControl(a) \/ CompleteControl(a) \/ ControlBusyConsumesSequence(a) \/ ControlOutcomeUnknown(a)
TargetLocalCompletionsPreserveSiblings == [][\A a \in Sessions:
  LocalCompletion(a) => \A other \in Viewers \ {V(a)}: Projection(other)' = Projection(other)]_vars
FreshBaselineRequiresSnapshot == [][\A a \in Sessions, v \in Viewers:
  PollRecord(a, v) /\ ~s.baseline[v] => s'.pending[v].kind = "snapshot"]_vars
FeedbackCountersDoNotChooseViewer == [][\A a \in Sessions, v \in Viewers:
  AcceptEqualCounterSiblingFeedback(a, v) => \A other \in Viewers \ {v}: s'.pending[other] = s.pending[other]]_vars
LocalCapacityDoesNotDispatch == [][\A a \in Sessions: RejectSessionCapacity(a) =>
  UNCHANGED <<s.cursor, s.workerOwned, s.viewers>>]_vars
WorkerCapacityRejectsOnlyCandidate == [][\A a \in Sessions: RejectWorkerCapacity(a) =>
  s'.viewers[V(a)].state = "rejected"
    /\ UNCHANGED s.workerOwned]_vars
CleanupUsesExactPinnedOwnership == [][\A v \in Viewers: CleanupViewer(v) =>
  Current(v) /\ GrantsOf(v) = {}]_vars
DisposalAndStaleCleanupEmitNoWorkerRequest == [][\A v \in Viewers: ForgetUnavailableViewer(v) =>
  UNCHANGED <<s.workerOwned, s.grants>>]_vars
OnlyExternalDisposalEndsTerminal == [][s'.disposed # s.disposed =>
  \E target \in Targets: DisposeTarget(target)]_vars
DisposedProofNeverRegresses == [][s.disposed \subseteq s'.disposed]_vars
ReleasedGrantNeverRevives == [][\A g \in Grants: s.grants[g].state = "released" => s'.grants[g].state = "released"]_vars

\* Serving or acknowledging the history lane never disturbs the frame lane:
\* pending record, baseline, and emitted frame counters are untouched.
HistoryLanePreservesFrameFlight == [][\A a \in Sessions, v \in Viewers:
  (PollHistory(a, v) \/ AcceptHistoryObserved(a, v)) =>
    UNCHANGED <<s.pending, s.baseline, s.frameSequence>>]_vars

\* A kind-53 record on a non-Poll route retires exactly the offending viewer.
HistoryWrongRouteClosesOnlyThatViewer == [][\A a \in Sessions, v \in Viewers:
  RejectHistoryWrongRoute(a, v) =>
    s.viewers[v].state = "active" /\ s'.viewers[v].state = "closing"]_vars
ClosingEventuallyRetires == \A v \in Viewers: (s.viewers[v].state = "closing") ~> (s.viewers[v].state = "retired")
OrderedFlightEventuallySettles == \A a \in Sessions:
  (s.phase[a] \notin {"idle", "dead"}) ~> (s.phase[a] \in {"idle", "dead"})
=============================================================================
