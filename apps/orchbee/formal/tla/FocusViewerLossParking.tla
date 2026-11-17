----------------------- MODULE FocusViewerLossParking -----------------------
EXTENDS Naturals

\* The focused pane of one Workbench Client and the operation that owns the
\* Client beside it. Only a lease that serves the projected focus (a poll, a
\* viewport or selection leg, a clear, a control frame) loses the focus viewer:
\* the Worker removes it, then publishes ViewerLost through the one event cell
\* the UI drains in order. The Worker runs one lease at a time, and an owner's
\* own lease never loses the focus viewer, so a loss reaches the UI either
\* before the owner's lease runs or after its result settled.
\*
\* A focus loss parks the focus beside every owner instead of retiring the
\* Client. "takeover" needs the focus viewer: its lease, finding the viewer
\* gone, settles it as refused, and a parked focus accepts only that answer.
\* "free" stands for every owner whose completion reads no viewer (Workspace
\* setup, Workspace close, a context selection, a retained uncertain Rename):
\* parking mid-flight leaves the same state as starting it from a parked focus.
\*
\* The claim implementation traces do not expose: no order of losses and
\* owner leases retires the Client, every owner settles, and the focus the UI
\* shows parked is exactly the focus whose viewer the Worker no longer holds.
\* Mapping: Queue/Lose/Drop = Ready-lease commands and dispose_lost_viewer;
\* Begin = plan and begin of an owner; ApplyLost = settle_pane_fault;
\* RunOwner = the owner's Worker lease (run_takeover and the refresh lane);
\* ApplyResult = the owner's UI settlement; Reattach = the bounded automatic
\* re-attach of a lost focus, which waits for every owner.
CONSTANTS Owners, MaxRuns

VARIABLES ui, worker, cell, runs
vars == <<ui, worker, cell, runs>>

Empty == [kind |-> "empty", value |-> "none"]

Init ==
  /\ ui = [phase |-> "ready", owner |-> "none"]
  /\ worker = [viewer |-> TRUE, poll |-> FALSE, queued |-> "none", waiting |-> FALSE]
  /\ cell = Empty
  /\ runs = 0

\* The UI queues focus commands only while it projects the focus.
Queue ==
  /\ ui.phase = "ready" /\ ~worker.poll
  /\ worker' = [worker EXCEPT !.poll = TRUE]
  /\ UNCHANGED <<ui, cell, runs>>

Lose ==
  /\ worker.poll /\ worker.viewer /\ ~worker.waiting /\ cell = Empty
  /\ worker' = [worker EXCEPT !.poll = FALSE, !.viewer = FALSE]
  /\ cell' = [kind |-> "lost", value |-> "none"]
  /\ UNCHANGED <<ui, runs>>

\* A command that keeps its viewer, or finds it already gone, settles nothing.
Drop ==
  /\ worker.poll /\ ~worker.waiting
  /\ worker' = [worker EXCEPT !.poll = FALSE]
  /\ UNCHANGED <<ui, cell, runs>>

Begin(o) ==
  /\ ui.owner = "none" /\ ui.phase # "retired" /\ runs < MaxRuns
  /\ worker.queued = "none" /\ ~worker.waiting
  /\ o = "takeover" => ui.phase = "ready"
  /\ ui' = [ui EXCEPT !.owner = o]
  /\ worker' = [worker EXCEPT !.queued = o]
  /\ runs' = runs + 1
  /\ UNCHANGED cell

ApplyLost ==
  /\ cell.kind = "lost"
  /\ ui' = IF ui.phase = "ready" THEN [ui EXCEPT !.phase = "parked"]
           ELSE [ui EXCEPT !.phase = "retired"]
  /\ cell' = Empty
  /\ UNCHANGED <<worker, runs>>

RunOwner ==
  /\ worker.queued # "none" /\ ~worker.waiting /\ cell = Empty
  /\ LET result == CASE worker.queued = "takeover" ->
                          IF worker.viewer THEN "granted" ELSE "refused"
                     [] OTHER -> "done"
     IN cell' = [kind |-> "result", value |-> result]
  /\ worker' = [worker EXCEPT !.queued = "none", !.waiting = TRUE]
  /\ UNCHANGED <<ui, runs>>

\* A result the focus cannot accept is a contract violation and retires.
ApplyResult ==
  /\ cell.kind = "result"
  /\ LET accepted == CASE cell.value = "granted" -> ui.phase = "ready"
                       [] cell.value = "refused" -> ui.phase \in {"ready", "parked"}
                       [] cell.value = "done" -> ui.phase \in {"ready", "parked"}
                       [] OTHER -> FALSE
     IN ui' = [phase |-> IF accepted THEN ui.phase ELSE "retired", owner |-> "none"]
  /\ cell' = Empty
  /\ worker' = [worker EXCEPT !.waiting = FALSE]
  /\ UNCHANGED runs

Reattach ==
  /\ ui.phase = "parked" /\ ui.owner = "none"
  /\ worker.queued = "none" /\ ~worker.waiting /\ cell = Empty
  /\ ui' = [ui EXCEPT !.phase = "ready"]
  /\ worker' = [worker EXCEPT !.viewer = TRUE]
  /\ UNCHANGED <<cell, runs>>

Next ==
  \/ Queue \/ Lose \/ Drop \/ (\E o \in Owners : Begin(o))
  \/ ApplyLost \/ RunOwner \/ ApplyResult \/ Reattach
Spec == Init /\ [][Next]_vars /\ WF_vars(RunOwner) /\ WF_vars(ApplyLost) /\ WF_vars(ApplyResult)

TypeOK ==
  /\ ui.phase \in {"ready", "parked", "retired"}
  /\ ui.owner \in Owners \cup {"none"}
  /\ worker.viewer \in BOOLEAN /\ worker.poll \in BOOLEAN /\ worker.waiting \in BOOLEAN
  /\ worker.queued \in Owners \cup {"none"}
  /\ cell.kind \in {"empty", "lost", "result"}
  /\ runs \in 0..MaxRuns
NeverRetired == ui.phase # "retired"
\* Once the UI drained every loss, it parks exactly the focus whose viewer is gone.
ParkedMatchesViewer == cell.kind # "lost" => (ui.phase = "parked" <=> ~worker.viewer)
OwnersSettle == ui.owner # "none" ~> ui.owner = "none"
=============================================================================
