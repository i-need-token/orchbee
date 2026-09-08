--------------------- MODULE WorkbenchTerminalSelection ---------------------
EXTENDS Naturals, FiniteSets, TLC
CONSTANTS Clients, Site, OtherSite, Worker, OtherWorker, Terminal, OtherTerminal, NoTarget
Target == [site |-> Site, worker |-> Worker, terminal |-> Terminal]
Targets == {Target,
  [site |-> OtherSite, worker |-> Worker, terminal |-> Terminal],
  [site |-> Site, worker |-> OtherWorker, terminal |-> Terminal],
  [site |-> Site, worker |-> Worker, terminal |-> OtherTerminal]}
ASSUME Cardinality(Clients) = 2 /\ Site # OtherSite /\ Worker # OtherWorker
       /\ Terminal # OtherTerminal
VARIABLES phase, requested, pinnedGeneration, relayIssued, workerGeneration, workerActive,
  workbenchActive, exposed, terminalReady, action
vars == <<phase, requested, pinnedGeneration, relayIssued, workerGeneration, workerActive,
  workbenchActive, exposed, terminalReady, action>>
Init ==
  /\ phase = [c \in Clients |-> "idle"]
  /\ requested = [c \in Clients |-> NoTarget]
  /\ pinnedGeneration = [c \in Clients |-> 0]
  /\ relayIssued = [c \in Clients |-> FALSE]
  /\ workerGeneration = 1 /\ workerActive = TRUE
  /\ workbenchActive \in BOOLEAN /\ exposed \in BOOLEAN /\ terminalReady \in BOOLEAN
  /\ action = "Init"
Request(c, target) ==
  /\ phase[c] = "idle" /\ requested' = [requested EXCEPT ![c] = target]
  /\ phase' = [phase EXCEPT ![c] = "requested"] /\ action' = "Request"
  /\ UNCHANGED <<pinnedGeneration, relayIssued, workerGeneration, workerActive, workbenchActive,
                  exposed, terminalReady>>
Eligible(c) == workbenchActive /\ exposed /\ requested[c] = Target
  /\ workerActive /\ workerGeneration = 1 /\ terminalReady
Resolve(c) ==
  /\ phase[c] = "requested"
  /\ phase' = [phase EXCEPT ![c] = IF Eligible(c) THEN "prepared" ELSE "refused"]
  /\ pinnedGeneration' = [pinnedGeneration EXCEPT ![c] = IF Eligible(c) THEN 1 ELSE 0]
  /\ action' = "Resolve"
  /\ UNCHANGED <<requested, relayIssued, workerGeneration, workerActive, workbenchActive, exposed, terminalReady>>
Dispatch(c) ==
  /\ phase[c] = "prepared" /\ phase' = [phase EXCEPT ![c] = "in-flight"]
  /\ relayIssued' = [relayIssued EXCEPT ![c] = TRUE]
  /\ action' = "Dispatch"
  /\ UNCHANGED <<requested, pinnedGeneration, workerGeneration, workerActive,
                  workbenchActive, exposed, terminalReady>>
WorkerAccept(c) ==
  /\ phase[c] = "in-flight"
  /\ phase' = [phase EXCEPT ![c] =
       IF workerActive /\ workerGeneration = pinnedGeneration[c] THEN "accepted" ELSE "refused"]
  /\ action' = "WorkerAccept"
  /\ UNCHANGED <<requested, pinnedGeneration, relayIssued, workerGeneration, workerActive,
                  workbenchActive, exposed, terminalReady>>
Install(c) ==
  /\ phase[c] = "accepted"
  /\ phase' = [phase EXCEPT ![c] =
       IF workerActive /\ workerGeneration = pinnedGeneration[c] THEN "attached" ELSE "refused"]
  /\ action' = "Install"
  /\ UNCHANGED <<requested, pinnedGeneration, relayIssued, workerGeneration, workerActive,
                  workbenchActive, exposed, terminalReady>>
WorkerLoss ==
  /\ workerActive /\ workerActive' = FALSE /\ action' = "WorkerLoss"
  /\ UNCHANGED <<phase, requested, pinnedGeneration, relayIssued, workerGeneration,
                  workbenchActive, exposed, terminalReady>>
WorkerReplace ==
  /\ workerGeneration = 1 /\ workerGeneration' = 2 /\ workerActive' = TRUE
  /\ action' = "WorkerReplace"
  /\ UNCHANGED <<phase, requested, pinnedGeneration, relayIssued, workbenchActive, exposed, terminalReady>>
RetireWorkbench ==
  /\ workbenchActive /\ workbenchActive' = FALSE /\ action' = "RetireWorkbench"
  /\ UNCHANGED <<phase, requested, pinnedGeneration, relayIssued, workerGeneration, workerActive,
                  exposed, terminalReady>>
LaunchCompletes ==
  /\ ~terminalReady /\ terminalReady' = TRUE /\ action' = "LaunchCompletes"
  /\ UNCHANGED <<phase, requested, pinnedGeneration, relayIssued, workerGeneration, workerActive,
                  workbenchActive, exposed>>
Next ==
  \/ \E c \in Clients, target \in Targets: Request(c, target)
  \/ \E c \in Clients: Resolve(c) \/ Dispatch(c) \/ WorkerAccept(c) \/ Install(c)
  \/ WorkerLoss \/ WorkerReplace \/ RetireWorkbench \/ LaunchCompletes
Spec == Init /\ [][Next]_vars
TypeOK ==
  /\ phase \in [Clients -> {"idle", "requested", "prepared", "in-flight", "accepted", "attached", "refused"}]
  /\ requested \in [Clients -> Targets \cup {NoTarget}]
  /\ pinnedGeneration \in [Clients -> 0..1] /\ workerGeneration \in 1..2
  /\ relayIssued \in [Clients -> BOOLEAN]
  /\ action \in {"Init", "Request", "Resolve", "Dispatch", "WorkerAccept", "Install",
                   "WorkerLoss", "WorkerReplace", "RetireWorkbench", "LaunchCompletes"}
  /\ workerActive \in BOOLEAN /\ workbenchActive \in BOOLEAN
  /\ exposed \in BOOLEAN /\ terminalReady \in BOOLEAN
ExactSelection == \A c \in Clients:
  phase[c] \in {"prepared", "in-flight", "accepted", "attached"} =>
    requested[c] = Target /\ pinnedGeneration[c] = 1
DispatchWasResolved == \A c \in Clients: relayIssued[c] =>
  requested[c] = Target /\ pinnedGeneration[c] = 1
SelectionPreservesTerminal ==
  [][(action' \in {"Request", "Resolve", "Dispatch", "WorkerAccept", "Install"} =>
    UNCHANGED <<terminalReady, exposed, workbenchActive, workerActive, workerGeneration>>)]_vars
SelectionWasAuthorized ==
  [][(action' = "Resolve" => \A c \in Clients:
      (phase[c] = "requested" /\ phase'[c] = "prepared") =>
        workbenchActive /\ exposed /\ requested[c] = Target /\ workerActive
        /\ workerGeneration = 1 /\ terminalReady)]_vars
InstallationWasCurrent ==
  [][(action' = "Install" => \A c \in Clients:
      (phase[c] = "accepted" /\ phase'[c] = "attached") =>
        workerActive /\ workerGeneration = pinnedGeneration[c])]_vars
NoRetarget == [][(\A c \in Clients: pinnedGeneration[c] # 0 =>
  pinnedGeneration'[c] = pinnedGeneration[c])]_vars
=============================================================================
