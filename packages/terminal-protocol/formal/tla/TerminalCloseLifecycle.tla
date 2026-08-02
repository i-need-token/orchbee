------------------------ MODULE TerminalCloseLifecycle ------------------------
EXTENDS TLC

CONSTANTS Terminals

TerminalPhases == {"absent", "running", "finishing", "exited"}
Authorities == {"desktop", "attachment"}

VARIABLES
  terminalPhase,
  terminateRequested,
  forceTerminateRequested

vars == <<terminalPhase, terminateRequested, forceTerminateRequested>>

Init ==
  /\ terminalPhase = [terminal \in Terminals |-> "absent"]
  /\ terminateRequested = [terminal \in Terminals |-> FALSE]
  /\ forceTerminateRequested = [terminal \in Terminals |-> FALSE]

Create(terminal) ==
  /\ terminal \in Terminals
  /\ terminalPhase[terminal] = "absent"
  /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "running"]
  /\ terminateRequested' =
       [terminateRequested EXCEPT ![terminal] = FALSE]
  /\ forceTerminateRequested' =
       [forceTerminateRequested EXCEPT ![terminal] = FALSE]

AttemptTerminate(authority, terminal) ==
  /\ authority \in Authorities
  /\ terminal \in Terminals
  /\ IF authority = "desktop" /\ terminalPhase[terminal] = "running"
        /\ ~terminateRequested[terminal]
     THEN
       /\ terminateRequested' =
            [terminateRequested EXCEPT ![terminal] = TRUE]
     ELSE
       /\ UNCHANGED terminateRequested
  /\ UNCHANGED <<terminalPhase, forceTerminateRequested>>

AttemptForceTerminate(authority, terminal) ==
  /\ authority \in Authorities
  /\ terminal \in Terminals
  /\ IF authority = "desktop" /\ terminateRequested[terminal]
        /\ terminalPhase[terminal] \in {"running", "finishing"}
        /\ ~forceTerminateRequested[terminal]
     THEN
       /\ forceTerminateRequested' =
            [forceTerminateRequested EXCEPT ![terminal] = TRUE]
     ELSE
       /\ UNCHANGED forceTerminateRequested
  /\ UNCHANGED <<terminalPhase, terminateRequested>>

BeginFinishing(terminal) ==
  /\ terminal \in Terminals
  /\ terminalPhase[terminal] = "running"
  /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "finishing"]
  /\ UNCHANGED <<terminateRequested, forceTerminateRequested>>

Exit(terminal) ==
  /\ terminal \in Terminals
  /\ terminalPhase[terminal] \in {"running", "finishing"}
  /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "exited"]
  /\ UNCHANGED <<terminateRequested, forceTerminateRequested>>

Dispose(terminal) ==
  /\ terminal \in Terminals
  /\ terminalPhase[terminal] = "exited"
  /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "absent"]
  /\ terminateRequested' =
       [terminateRequested EXCEPT ![terminal] = FALSE]
  /\ forceTerminateRequested' =
       [forceTerminateRequested EXCEPT ![terminal] = FALSE]

Next ==
  \/ \E terminal \in Terminals: Create(terminal)
  \/ \E authority \in Authorities, terminal \in Terminals:
       AttemptTerminate(authority, terminal)
  \/ \E authority \in Authorities, terminal \in Terminals:
       AttemptForceTerminate(authority, terminal)
  \/ \E terminal \in Terminals: BeginFinishing(terminal)
  \/ \E terminal \in Terminals: Exit(terminal)
  \/ \E terminal \in Terminals: Dispose(terminal)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ terminalPhase \in [Terminals -> TerminalPhases]
  /\ terminateRequested \in [Terminals -> BOOLEAN]
  /\ forceTerminateRequested \in [Terminals -> BOOLEAN]

AbsentTerminalHasNoRequests ==
  \A terminal \in Terminals:
    terminalPhase[terminal] = "absent" =>
      ~terminateRequested[terminal] /\ ~forceTerminateRequested[terminal]

ForceRequiresGracefulRequest ==
  \A terminal \in Terminals:
    forceTerminateRequested[terminal] => terminateRequested[terminal]

TargetedTerminatePreservesOtherTerminals ==
  [][\A terminal \in Terminals:
       AttemptTerminate("desktop", terminal) =>
         \A other \in Terminals \ {terminal}:
           UNCHANGED <<terminalPhase[other], terminateRequested[other],
                       forceTerminateRequested[other]>>]_vars

TargetedForcePreservesOtherTerminals ==
  [][\A terminal \in Terminals:
       AttemptForceTerminate("desktop", terminal) =>
         \A other \in Terminals \ {terminal}:
           UNCHANGED <<terminalPhase[other], terminateRequested[other],
                       forceTerminateRequested[other]>>]_vars

TargetedDisposePreservesOtherTerminals ==
  [][\A terminal \in Terminals:
       Dispose(terminal) =>
         \A other \in Terminals \ {terminal}:
           UNCHANGED <<terminalPhase[other], terminateRequested[other],
                       forceTerminateRequested[other]>>]_vars

ForcePreservesTerminalPhase ==
  [][\A authority \in Authorities, terminal \in Terminals:
       AttemptForceTerminate(authority, terminal) =>
         UNCHANGED terminalPhase]_vars

ScopedAuthorityCannotTerminate ==
  [][\A terminal \in Terminals:
       AttemptTerminate("attachment", terminal) => UNCHANGED vars]_vars

ScopedAuthorityCannotForce ==
  [][\A terminal \in Terminals:
       AttemptForceTerminate("attachment", terminal) => UNCHANGED vars]_vars

=============================================================================
