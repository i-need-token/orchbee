-------------------- MODULE DesktopTerminalBindingClose --------------------
EXTENDS TLC

CONSTANTS Terminals

BindingStates == {"retained", "closing", "removed"}
TerminalPhases == {"running", "exited", "failed", "absent"}

VARIABLES
  desktopActive,
  bindingState,
  projectionAttached,
  terminalPhase,
  gracefulRequested,
  graceExpired,
  forceRequested

vars == <<
  desktopActive,
  bindingState,
  projectionAttached,
  terminalPhase,
  gracefulRequested,
  graceExpired,
  forceRequested
>>

Init ==
  /\ desktopActive = TRUE
  /\ bindingState = [terminal \in Terminals |-> "retained"]
  /\ projectionAttached = [terminal \in Terminals |-> TRUE]
  /\ terminalPhase = [terminal \in Terminals |-> "running"]
  /\ gracefulRequested = [terminal \in Terminals |-> FALSE]
  /\ graceExpired = [terminal \in Terminals |-> FALSE]
  /\ forceRequested = [terminal \in Terminals |-> FALSE]

BeginClose(terminal) ==
  /\ desktopActive
  /\ terminal \in Terminals
  /\ bindingState[terminal] = "retained"
  /\ bindingState' = [bindingState EXCEPT ![terminal] = "closing"]
  /\ UNCHANGED <<desktopActive, projectionAttached,
                  terminalPhase, gracefulRequested, graceExpired,
                  forceRequested>>

RetireProjection(terminal) ==
  /\ desktopActive
  /\ terminal \in Terminals
  /\ bindingState[terminal] = "closing"
  /\ projectionAttached[terminal]
  /\ projectionAttached' =
       [projectionAttached EXCEPT ![terminal] = FALSE]
  /\ UNCHANGED <<desktopActive, bindingState,
                  terminalPhase, gracefulRequested, graceExpired,
                  forceRequested>>

RequestGraceful(terminal) ==
  /\ desktopActive
  /\ terminal \in Terminals
  /\ bindingState[terminal] = "closing"
  /\ ~projectionAttached[terminal]
  /\ terminalPhase[terminal] = "running"
  /\ ~gracefulRequested[terminal]
  /\ gracefulRequested' =
       [gracefulRequested EXCEPT ![terminal] = TRUE]
  /\ UNCHANGED <<desktopActive, bindingState,
                  projectionAttached, terminalPhase, graceExpired,
                  forceRequested>>

ExpireGrace(terminal) ==
  /\ desktopActive
  /\ terminal \in Terminals
  /\ bindingState[terminal] = "closing"
  /\ gracefulRequested[terminal]
  /\ terminalPhase[terminal] = "running"
  /\ ~graceExpired[terminal]
  /\ graceExpired' = [graceExpired EXCEPT ![terminal] = TRUE]
  /\ UNCHANGED <<desktopActive, bindingState,
                  projectionAttached, terminalPhase, gracefulRequested,
                  forceRequested>>

RequestForce(terminal) ==
  /\ desktopActive
  /\ terminal \in Terminals
  /\ bindingState[terminal] = "closing"
  /\ ~projectionAttached[terminal]
  /\ terminalPhase[terminal] = "running"
  /\ gracefulRequested[terminal]
  /\ graceExpired[terminal]
  /\ ~forceRequested[terminal]
  /\ forceRequested' = [forceRequested EXCEPT ![terminal] = TRUE]
  /\ UNCHANGED <<desktopActive, bindingState,
                  projectionAttached, terminalPhase, gracefulRequested,
                  graceExpired>>

HostExit(terminal) ==
  /\ terminal \in Terminals
  /\ terminalPhase[terminal] = "running"
  /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "exited"]
  /\ UNCHANGED <<desktopActive, bindingState,
                  projectionAttached, gracefulRequested, graceExpired,
                  forceRequested>>

HostFail(terminal) ==
  /\ terminal \in Terminals
  /\ terminalPhase[terminal] = "running"
  /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "failed"]
  /\ UNCHANGED <<desktopActive, bindingState,
                  projectionAttached, gracefulRequested, graceExpired,
                  forceRequested>>

Dispose(terminal) ==
  /\ desktopActive
  /\ terminal \in Terminals
  /\ bindingState[terminal] = "closing"
  /\ ~projectionAttached[terminal]
  /\ terminalPhase[terminal] \in {"exited", "failed"}
  /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "absent"]
  /\ UNCHANGED <<desktopActive, bindingState,
                  projectionAttached, gracefulRequested, graceExpired,
                  forceRequested>>

RemoveBinding(terminal) ==
  /\ desktopActive
  /\ terminal \in Terminals
  /\ bindingState[terminal] = "closing"
  /\ ~projectionAttached[terminal]
  /\ terminalPhase[terminal] = "absent"
  /\ bindingState' = [bindingState EXCEPT ![terminal] = "removed"]
  /\ gracefulRequested' =
       [gracefulRequested EXCEPT ![terminal] = FALSE]
  /\ graceExpired' = [graceExpired EXCEPT ![terminal] = FALSE]
  /\ forceRequested' = [forceRequested EXCEPT ![terminal] = FALSE]
  /\ UNCHANGED <<desktopActive, projectionAttached,
                  terminalPhase>>

CrashDesktop ==
  /\ desktopActive
  /\ desktopActive' = FALSE
  /\ projectionAttached' = [terminal \in Terminals |-> FALSE]
  /\ UNCHANGED <<bindingState, terminalPhase,
                  gracefulRequested, graceExpired, forceRequested>>

RestartDesktop ==
  /\ ~desktopActive
  /\ desktopActive' = TRUE
  /\ UNCHANGED <<bindingState, projectionAttached,
                  terminalPhase, gracefulRequested, graceExpired,
                  forceRequested>>

Next ==
  \/ \E terminal \in Terminals: BeginClose(terminal)
  \/ \E terminal \in Terminals: RetireProjection(terminal)
  \/ \E terminal \in Terminals: RequestGraceful(terminal)
  \/ \E terminal \in Terminals: ExpireGrace(terminal)
  \/ \E terminal \in Terminals: RequestForce(terminal)
  \/ \E terminal \in Terminals: HostExit(terminal)
  \/ \E terminal \in Terminals: HostFail(terminal)
  \/ \E terminal \in Terminals: Dispose(terminal)
  \/ \E terminal \in Terminals: RemoveBinding(terminal)
  \/ CrashDesktop
  \/ RestartDesktop

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ desktopActive \in BOOLEAN
  /\ bindingState \in [Terminals -> BindingStates]
  /\ projectionAttached \in [Terminals -> BOOLEAN]
  /\ terminalPhase \in [Terminals -> TerminalPhases]
  /\ gracefulRequested \in [Terminals -> BOOLEAN]
  /\ graceExpired \in [Terminals -> BOOLEAN]
  /\ forceRequested \in [Terminals -> BOOLEAN]

HostMutationRequiresDurableIntent ==
  \A terminal \in Terminals:
    (gracefulRequested[terminal] \/ terminalPhase[terminal] = "absent") =>
      bindingState[terminal] \in {"closing", "removed"}

HostMutationRequiresRetiredProjection ==
  \A terminal \in Terminals:
    (gracefulRequested[terminal] \/ terminalPhase[terminal] = "absent") =>
      ~projectionAttached[terminal]

ForceRequiresGraceAndTimeout ==
  \A terminal \in Terminals:
    forceRequested[terminal] =>
      /\ bindingState[terminal] = "closing"
      /\ gracefulRequested[terminal]
      /\ graceExpired[terminal]
      /\ ~projectionAttached[terminal]

RemovedBindingNamesNoHostTerminal ==
  \A terminal \in Terminals:
    bindingState[terminal] = "removed" =>
      /\ terminalPhase[terminal] = "absent"
      /\ ~projectionAttached[terminal]
      /\ ~gracefulRequested[terminal]
      /\ ~graceExpired[terminal]
      /\ ~forceRequested[terminal]

TargetedClosePreservesOtherBindings ==
  [][\A terminal \in Terminals:
       BeginClose(terminal) =>
         \A other \in Terminals \ {terminal}:
           UNCHANGED <<bindingState[other], projectionAttached[other],
                       terminalPhase[other], gracefulRequested[other],
                       graceExpired[other], forceRequested[other]>>]_vars

TargetedDisposePreservesOtherBindings ==
  [][\A terminal \in Terminals:
       Dispose(terminal) =>
         \A other \in Terminals \ {terminal}:
           UNCHANGED <<bindingState[other], projectionAttached[other],
                       terminalPhase[other], gracefulRequested[other],
                       graceExpired[other], forceRequested[other]>>]_vars

=============================================================================
