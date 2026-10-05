---------------------- MODULE KeyboardCopySelectionBarrier ----------------------
EXTENDS Naturals, Sequences, FiniteSets

(* Independent projection-local admission ordering, not clipboard/query state.
   The Worker owns one FIFO lease and completes its wire call before the next.
   Final cleanup admission or an exact known rejection releases the boundary.
   Admitted, settled and rejected proof phases remain distinct, not success.
   Rejection clears obsolete future cleanup without replaying it; viewport may continue.
   Commands for an old pane incarnation may finish, but cannot clear a new fence.
   Proofs retain exact boundary tokens while current intent or a viewport consumer exists;
   consumed/unreferenced history is not authoritative model state. *)
CONSTANTS MaxSerial, MaxQueue
ASSUME /\ MaxSerial = 5 /\ MaxQueue = 3
Panes == {1, 2}
Generations == {1, 2}
Kinds == {"old", "clear", "cancel", "cancelExport", "enter", "recover"}
Cleanups == {<<"cancel">>, <<"cancelExport">>, <<"cancelExport", "cancel">>,
             <<"clear", "cancel">>, <<"cancelExport", "clear", "cancel">>}
NoCommand == [pane |-> 0, generation |-> 0, serial |-> 0, kind |-> "none",
              boundary |-> 0]
Command(p, g, n, k, b) == [pane |-> p, generation |-> g, serial |-> n,
                           kind |-> k, boundary |-> b]
VARIABLE s
vars == <<s>>
Init == s = [generation |-> [p \in Panes |-> 1], serial |-> [p \in Panes |-> 0],
    driverFlight |-> [p \in Panes |-> 0], cleanup |-> [p \in Panes |-> <<>>],
    fence |-> [p \in Panes |-> 0], intent |-> [p \in Panes |-> "none"],
    boundary |-> [p \in Panes |-> 0], phase |-> [p \in Panes |-> "idle"],
    queue |-> <<>>, active |-> NoCommand, cleanupAdmitted |-> {}, cleanupSettled |-> {},
    boundaryRejected |-> {},
    retired |-> FALSE, observation |-> FALSE]
Token(cmd) == <<cmd.pane, cmd.generation, cmd.serial>>
Current(cmd) == cmd.generation = s.generation[cmd.pane]
IsCleanup(cmd) == cmd.kind \in {"clear", "cancel", "cancelExport"}
IsViewport(cmd) == cmd.kind \in {"enter", "recover"}
BoundaryToken(cmd) == <<cmd.pane, cmd.generation, cmd.boundary>>
BoundaryTokens == {<<p, g, b>> : p \in Panes, g \in Generations, b \in 1..MaxSerial}
References(viewport, cmd) == IsViewport(viewport) /\ BoundaryToken(viewport) = BoundaryToken(cmd)
HasConsumer(cmd) == References(s.active, cmd)
    \/ (\E i \in 1..Len(s.queue) : References(s.queue[i], cmd))
PendingPriorBoundary(cmd) == Current(cmd) /\ ~s.retired
    /\ s.phase[cmd.pane] = "waiting" /\ s.fence[cmd.pane] # 0
    /\ s.boundary[cmd.pane] # 0 /\ s.intent[cmd.pane] # "none"
NeedsProof(cmd) == HasConsumer(cmd) \/
    (Current(cmd) /\ ~s.retired /\ s.phase[cmd.pane] = "waiting"
     /\ s.boundary[cmd.pane] = cmd.boundary /\ s.intent[cmd.pane] # "none")

StartOld(p) ==
    /\ ~s.retired /\ s.phase[p] = "idle" /\ s.driverFlight[p] = 0
    /\ s.serial[p] < MaxSerial /\ Len(s.queue) < MaxQueue
    /\ LET n == s.serial[p] + 1
           cmd == Command(p, s.generation[p], n, "old", 0)
       IN s' = [s EXCEPT !.serial[p] = n, !.driverFlight[p] = n,
                          !.queue = Append(@, cmd)]

RequestBoundary(p, kind, chain) ==
    /\ ~s.retired /\ s.phase[p] = "idle" /\ s.serial[p] + Len(chain) < MaxSerial
    /\ kind \in {"enter", "recover"} /\ chain \in Cleanups
    /\ LET first == s.serial[p] + 1
           last == s.serial[p] + Len(chain)
           commands == [i \in 1..Len(chain) |->
               Command(p, s.generation[p], first + i - 1, chain[i], last)]
       IN s' = [s EXCEPT !.serial[p] = last, !.cleanup[p] = commands,
          !.fence[p] = last, !.boundary[p] = last,
          !.intent[p] = kind, !.phase[p] = "waiting"]

AdmitCleanup(p) ==
    /\ ~s.retired /\ Len(s.cleanup[p]) > 0 /\ s.driverFlight[p] = 0
    /\ Len(s.queue) < MaxQueue
    /\ LET cmd == Head(s.cleanup[p])
       IN s' = [s EXCEPT !.cleanup[p] = Tail(@), !.driverFlight[p] = cmd.serial,
          !.queue = Append(@, cmd),
          !.cleanupAdmitted = IF cmd.serial = cmd.boundary THEN @ \cup {Token(cmd)} ELSE @,
          !.fence[p] = IF @ = cmd.serial THEN 0 ELSE @]

AdmitViewport(p) ==
    /\ ~s.retired /\ s.phase[p] = "waiting" /\ s.fence[p] = 0
    /\ s.intent[p] /= "none" /\ s.serial[p] < MaxSerial
    /\ Len(s.queue) < MaxQueue
    /\ LET n == s.serial[p] + 1
           cmd == Command(p, s.generation[p], n, s.intent[p], s.boundary[p])
       IN s' = [s EXCEPT !.serial[p] = n, !.queue = Append(@, cmd),
                          !.intent[p] = "none", !.phase[p] = "admitted"]

TakeLease ==
    /\ ~s.retired /\ s.active = NoCommand /\ Len(s.queue) > 0
    /\ s' = [s EXCEPT !.active = Head(s.queue), !.queue = Tail(@)]

CompleteOld ==
    /\ s.active.kind = "old"
    /\ LET cmd == s.active
       IN s' = [s EXCEPT !.active = NoCommand,
          !.driverFlight[cmd.pane] = IF Current(cmd) /\ @ = cmd.serial THEN 0 ELSE @]

RejectOld ==
    /\ s.active.kind = "old" /\ ~PendingPriorBoundary(s.active)
    /\ LET cmd == s.active
           p == cmd.pane
       IN s' = [s EXCEPT !.active = NoCommand,
          !.driverFlight[p] = IF Current(cmd) THEN 0 ELSE @,
          !.cleanup[p] = IF Current(cmd) THEN <<>> ELSE @,
          !.fence[p] = IF Current(cmd) THEN 0 ELSE @]

RejectPriorBoundary ==
    /\ s.active.kind = "old" /\ PendingPriorBoundary(s.active)
    /\ LET p == s.active.pane
           proof == <<p, s.active.generation, s.boundary[p]>>
       IN s' = [s EXCEPT !.active = NoCommand, !.driverFlight[p] = 0,
          !.cleanup[p] = <<>>, !.fence[p] = 0,
          !.cleanupAdmitted = @ \ {proof}, !.cleanupSettled = @ \ {proof},
          !.boundaryRejected = @ \cup {proof}]

CompleteCleanup ==
    /\ IsCleanup(s.active)
    /\ LET cmd == s.active
           keep == cmd.serial = cmd.boundary /\ NeedsProof(cmd)
       IN s' = [s EXCEPT !.active = NoCommand,
          !.cleanupAdmitted = IF cmd.serial = cmd.boundary THEN @ \ {Token(cmd)} ELSE @,
          !.cleanupSettled = IF keep THEN @ \cup {Token(cmd)} ELSE @,
          !.driverFlight[cmd.pane] = IF Current(cmd) /\ @ = cmd.serial THEN 0 ELSE @]

RejectCleanup ==
    /\ IsCleanup(s.active)
    /\ LET cmd == s.active
           p == cmd.pane
           proof == BoundaryToken(cmd)
           keep == NeedsProof(cmd)
       IN s' = [s EXCEPT !.active = NoCommand,
          !.cleanupAdmitted = @ \ {proof}, !.cleanupSettled = @ \ {proof},
          !.boundaryRejected = IF keep THEN @ \cup {proof} ELSE @,
          !.driverFlight[p] = IF Current(cmd) THEN 0 ELSE @,
          !.cleanup[p] = IF Current(cmd) THEN <<>> ELSE @,
          !.fence[p] = IF Current(cmd) THEN 0 ELSE @]

CompleteViewport ==
    /\ IsViewport(s.active)
    /\ LET cmd == s.active
           proof == <<cmd.pane, cmd.generation, cmd.boundary>>
       IN s' = [s EXCEPT !.active = NoCommand,
          !.cleanupAdmitted = @ \ {proof}, !.cleanupSettled = @ \ {proof},
          !.boundaryRejected = @ \ {proof},
          !.phase[cmd.pane] = IF Current(cmd) /\ ~s.retired THEN "idle" ELSE @]

InvalidatePane(p) ==
    /\ ~s.retired /\ s.generation[p] = 1
    /\ LET old == s.generation[p]
           referenced == (s.active /= NoCommand /\ IsViewport(s.active)
               /\ s.active.pane = p /\ s.active.generation = old)
               \/ (\E i \in 1..Len(s.queue) : IsViewport(s.queue[i])
                   /\ s.queue[i].pane = p /\ s.queue[i].generation = old)
       IN s' = [s EXCEPT !.generation[p] = 2, !.serial[p] = 0,
       !.cleanupAdmitted = IF referenced THEN @ ELSE {t \in @ : t[1] /= p \/ t[2] /= old},
       !.cleanupSettled = IF referenced THEN @ ELSE {t \in @ : t[1] /= p \/ t[2] /= old},
       !.boundaryRejected = IF referenced THEN @ ELSE {t \in @ : t[1] /= p \/ t[2] /= old},
       !.driverFlight[p] = 0, !.cleanup[p] = <<>>, !.fence[p] = 0,
       !.intent[p] = "none", !.boundary[p] = 0, !.phase[p] = "idle"]

Retire ==
    /\ ~s.retired
    /\ LET proof == <<s.active.pane, s.active.generation, s.active.boundary>>
       IN s' = [s EXCEPT !.retired = TRUE, !.queue = <<>>,
       !.serial = [p \in Panes |-> 0], !.boundary = [p \in Panes |-> 0],
       !.cleanupAdmitted = IF IsViewport(s.active) THEN @ \cap {proof} ELSE {},
       !.cleanupSettled = IF IsViewport(s.active) THEN @ \cap {proof} ELSE {},
       !.boundaryRejected = IF IsViewport(s.active) THEN @ \cap {proof} ELSE {},
       !.driverFlight = [p \in Panes |-> 0], !.cleanup = [p \in Panes |-> <<>>],
       !.fence = [p \in Panes |-> 0], !.intent = [p \in Panes |-> "none"],
       !.phase = [p \in Panes |-> "retired"]]

ForeignReply == /\ ~s.retired /\ s' = [s EXCEPT !.observation = ~@]
Next == \/ \E p \in Panes : StartOld(p)
        \/ \E p \in Panes, k \in {"enter", "recover"}, chain \in Cleanups : RequestBoundary(p, k, chain)
        \/ \E p \in Panes : AdmitCleanup(p)
        \/ \E p \in Panes : AdmitViewport(p)
        \/ TakeLease \/ CompleteOld \/ RejectOld \/ RejectPriorBoundary \/ CompleteCleanup \/ RejectCleanup
        \/ CompleteViewport \/ (\E p \in Panes : InvalidatePane(p))
        \/ Retire \/ ForeignReply
Spec == Init /\ [][Next]_vars

TypeOK == /\ s.generation \in [Panes -> Generations]
    /\ s.serial \in [Panes -> 0..MaxSerial]
    /\ s.driverFlight \in [Panes -> 0..MaxSerial]
    /\ s.fence \in [Panes -> 0..MaxSerial]
    /\ s.phase \in [Panes -> {"idle", "waiting", "admitted", "retired"}]
    /\ s.intent \in [Panes -> {"none", "enter", "recover"}]
    /\ s.retired \in BOOLEAN /\ s.observation \in BOOLEAN
    /\ s.cleanupAdmitted \subseteq BoundaryTokens /\ s.cleanupSettled \subseteq BoundaryTokens
    /\ s.boundaryRejected \subseteq BoundaryTokens
    /\ s.cleanupAdmitted \cap s.cleanupSettled = {}
    /\ s.cleanupAdmitted \cap s.boundaryRejected = {}
    /\ s.cleanupSettled \cap s.boundaryRejected = {}
QueueBounded == Len(s.queue) <= MaxQueue
ViewportProof(cmd) == ~IsViewport(cmd) \/
    BoundaryToken(cmd) \in s.cleanupAdmitted \cup s.cleanupSettled \cup s.boundaryRejected
ViewportAdmittedAfterBoundary ==
    /\ \A i \in 1..Len(s.queue) : ViewportProof(s.queue[i])
    /\ s.active = NoCommand \/ ViewportProof(s.active)
ViewportExecutedAfterBoundary == ~IsViewport(s.active) \/
    BoundaryToken(s.active) \in s.cleanupSettled \cup s.boundaryRejected
FencesBoundToLocalWork == \A p \in Panes :
    s.fence[p] = 0 \/ (s.phase[p] = "waiting" /\ Len(s.cleanup[p]) > 0)
RetiredHasNoAdmission == s.retired =>
    /\ s.queue = <<>> /\ \A p \in Panes : s.fence[p] = 0 /\ s.cleanup[p] = <<>>
================================================================================
