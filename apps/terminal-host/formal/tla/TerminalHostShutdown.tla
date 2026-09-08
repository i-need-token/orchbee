------------------------ MODULE TerminalHostShutdown ------------------------
EXTENDS TLC

\* Refines runtime-model claim TH-013 through RequestShutdown,
\* AttemptShutdown, and CompleteShutdownPass. Two runtimes are retained because
\* the promise under test is specifically that one failed shutdown attempt does
\* not short-circuit the other runtime. The one-pass result distinguishes clean
\* completion from aggregate failure, and retained records make cleanup
\* ownership falsifiable: a failed pass releases none of the runtimes.

CONSTANTS Runtimes

HostPhases == {"serving", "shutting_down", "pass_complete"}
ShutdownResults == {"none", "clean", "failed"}

VARIABLES hostPhase, attempted, failed, retained, shutdownResult

vars == <<hostPhase, attempted, failed, retained, shutdownResult>>

Init ==
  /\ hostPhase = "serving"
  /\ attempted = {}
  /\ failed = {}
  /\ retained = Runtimes
  /\ shutdownResult = "none"

RequestShutdown ==
  /\ hostPhase = "serving"
  /\ hostPhase' = "shutting_down"
  /\ attempted' = {}
  /\ failed' = {}
  /\ shutdownResult' = "none"
  /\ UNCHANGED retained

AttemptShutdown(runtime, succeeds) ==
  /\ hostPhase = "shutting_down"
  /\ runtime \in Runtimes
  /\ runtime \notin attempted
  /\ succeeds \in BOOLEAN
  /\ attempted' = attempted \cup {runtime}
  /\ failed' = IF succeeds THEN failed ELSE failed \cup {runtime}
  /\ UNCHANGED <<hostPhase, retained, shutdownResult>>

ShutdownAttempt(runtime) ==
  \E succeeds \in BOOLEAN: AttemptShutdown(runtime, succeeds)

AllRuntimesAttempted == attempted = Runtimes

CompleteShutdownPass ==
  /\ hostPhase = "shutting_down"
  /\ AllRuntimesAttempted
  /\ hostPhase' = "pass_complete"
  /\ shutdownResult' = IF failed = {} THEN "clean" ELSE "failed"
  /\ retained' = IF failed = {} THEN {} ELSE retained
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
  /\ retained \subseteq Runtimes
  /\ shutdownResult \in ShutdownResults

ShutdownIntegrity ==
  /\ (hostPhase = "serving" => attempted = {})
  /\ (hostPhase \in {"serving", "shutting_down"} =>
       /\ retained = Runtimes
       /\ shutdownResult = "none")
  /\ (hostPhase = "pass_complete" =>
       /\ AllRuntimesAttempted
       /\ IF failed = {}
            THEN /\ shutdownResult = "clean"
                 /\ retained = {}
            ELSE /\ shutdownResult = "failed"
                 /\ retained = Runtimes)

EveryAttemptIsRecorded ==
  [][\A runtime \in Runtimes, succeeds \in BOOLEAN:
       AttemptShutdown(runtime, succeeds) =>
         runtime \in attempted']_vars

ShutdownEventuallyAttemptsEveryRuntime ==
  [](hostPhase = "shutting_down" => <> (hostPhase = "pass_complete"))

FailedPassRetainsOwnershipAndReportsFailure ==
  [][CompleteShutdownPass /\ failed # {} =>
       /\ shutdownResult' = "failed"
       /\ retained' = Runtimes]_vars

CleanPassReleasesOwnershipAndReportsClean ==
  [][CompleteShutdownPass /\ failed = {} =>
       /\ shutdownResult' = "clean"
       /\ retained' = {}]_vars

=============================================================================
