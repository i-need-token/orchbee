------------------------ MODULE TerminalHostShutdown ------------------------
EXTENDS TLC

\* Refines runtime-model claim TH-013 through RequestShutdown,
\* AttemptShutdown, and CompleteShutdownPass. Two runtimes are retained because
\* the promise under test is specifically that one failed shutdown attempt does
\* not short-circuit the other runtime. Registry, broker, and terminal lifecycle
\* details are intentionally absent from this teardown-pass proof.

CONSTANTS Runtimes

HostPhases == {"serving", "shutting_down", "stopped"}

VARIABLES hostPhase, attempted, failed

vars == <<hostPhase, attempted, failed>>

Init ==
  /\ hostPhase = "serving"
  /\ attempted = {}
  /\ failed = {}

RequestShutdown ==
  /\ hostPhase = "serving"
  /\ hostPhase' = "shutting_down"
  /\ attempted' = {}
  /\ failed' = {}

AttemptShutdown(runtime, succeeds) ==
  /\ hostPhase = "shutting_down"
  /\ runtime \in Runtimes
  /\ runtime \notin attempted
  /\ succeeds \in BOOLEAN
  /\ attempted' = attempted \cup {runtime}
  /\ failed' = IF succeeds THEN failed ELSE failed \cup {runtime}
  /\ UNCHANGED hostPhase

ShutdownAttempt(runtime) ==
  \E succeeds \in BOOLEAN: AttemptShutdown(runtime, succeeds)

AllRuntimesAttempted == attempted = Runtimes

CompleteShutdownPass ==
  /\ hostPhase = "shutting_down"
  /\ AllRuntimesAttempted
  /\ hostPhase' = "stopped"
  /\ UNCHANGED <<attempted, failed>>

Next ==
  \/ RequestShutdown
  \/ \E runtime \in Runtimes: ShutdownAttempt(runtime)
  \/ CompleteShutdownPass

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A runtime \in Runtimes: WF_vars(ShutdownAttempt(runtime))
  /\ WF_vars(CompleteShutdownPass)

TypeOK ==
  /\ hostPhase \in HostPhases
  /\ attempted \subseteq Runtimes
  /\ failed \subseteq attempted

ShutdownIntegrity ==
  /\ hostPhase = "serving" => attempted = {}
  /\ hostPhase = "stopped" => AllRuntimesAttempted

EveryAttemptIsRecorded ==
  [][\A runtime \in Runtimes, succeeds \in BOOLEAN:
       AttemptShutdown(runtime, succeeds) =>
         runtime \in attempted']_vars

ShutdownEventuallyAttemptsEveryRuntime ==
  [](hostPhase = "shutting_down" => <> (hostPhase = "stopped"))

=============================================================================
