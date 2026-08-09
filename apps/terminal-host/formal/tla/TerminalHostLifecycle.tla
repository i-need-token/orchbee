------------------------ MODULE TerminalHostLifecycle ------------------------
EXTENDS TLC

\* Refines runtime-model claims TH-008 and TH-009 through ChildExited,
\* PtyDrained, WriterQuiesced, ClassifyNormalCompletion, EnterFailure,
\* ContainmentBlocked, CleanupRetryDue, RetryContainment,
\* ConfirmContainment, MutateVisibleState, and Dispose. Registry identity,
\* broker retention, and multi-runtime shutdown are intentionally owned by
\* TerminalHostRegistry and TerminalHostShutdown so their independent state
\* dimensions do not multiply this per-terminal lifecycle proof.
\* visibleVersion abstracts content, generation, geometry, configuration, and
\* revision. settledVersion captures the exact tuple published by the terminal
\* outcome and remains immutable until explicit disposal.

CONSTANTS Versions, InitialVersion

None == "none"
NormalPhases == {"running", "finishing_child", "finishing_output"}
FailurePhases == {"containing_failure", "containment_blocked", "failed"}
SettledPhases == {"exited", "failed"}
TerminalPhases ==
  NormalPhases \cup FailurePhases \cup SettledPhases \cup {"disposed"}
TerminationIntents == {None, "graceful", "force"}
CleanupFailures == {
  "contain_process",
  "release_output",
  "release_process_identity"
}
ProcessResolutions == {"pending", "observed", "unavailable"}
CompletedProcessResolutions == {"observed", "unavailable"}

VARIABLES
  phase,
  terminationIntent,
  writerQuiesced,
  normalCompletionReady,
  cleanupFailure,
  cleanupRetryDue,
  processResolution,
  visibleVersion,
  settledVersion

vars ==
  <<phase, terminationIntent, writerQuiesced, normalCompletionReady,
    cleanupFailure, cleanupRetryDue, processResolution, visibleVersion,
    settledVersion>>

Init ==
  /\ InitialVersion \in Versions
  /\ phase = "running"
  /\ terminationIntent = None
  /\ writerQuiesced = FALSE
  /\ normalCompletionReady = FALSE
  /\ cleanupFailure = None
  /\ cleanupRetryDue = FALSE
  /\ processResolution = "pending"
  /\ visibleVersion = InitialVersion
  /\ settledVersion = None

ChildExited ==
  /\ \/ phase = "running"
     \/ /\ phase = "finishing_output"
        /\ normalCompletionReady
  /\ phase' = IF phase = "finishing_output" THEN "exited"
                ELSE "finishing_child"
  /\ settledVersion' = IF phase = "finishing_output" THEN visibleVersion
                         ELSE settledVersion
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  processResolution, visibleVersion>>

PtyDrained ==
  /\ \/ phase = "running"
     \/ /\ phase = "finishing_child"
        /\ normalCompletionReady
  /\ phase' = IF phase = "finishing_child" THEN "exited"
                ELSE "finishing_output"
  /\ settledVersion' = IF phase = "finishing_child" THEN visibleVersion
                         ELSE settledVersion
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  processResolution, visibleVersion>>

WriterQuiesced ==
  /\ phase \in NormalPhases
  /\ ~writerQuiesced
  /\ writerQuiesced' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, normalCompletionReady,
                  cleanupFailure, cleanupRetryDue, processResolution,
                  visibleVersion, settledVersion>>

ClassifyNormalCompletion ==
  /\ phase \in {"finishing_child", "finishing_output"}
  /\ writerQuiesced
  /\ ~normalCompletionReady
  /\ normalCompletionReady' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  cleanupFailure, cleanupRetryDue, processResolution,
                  visibleVersion, settledVersion>>

EnterFailure ==
  /\ phase \in NormalPhases
  /\ phase' = "containing_failure"
  /\ terminationIntent' = "force"
  /\ normalCompletionReady' = FALSE
  /\ cleanupFailure' = None
  /\ cleanupRetryDue' = FALSE
  /\ processResolution' = "pending"
  /\ UNCHANGED <<writerQuiesced, visibleVersion, settledVersion>>

RequestGraceful(effectSucceeds) ==
  /\ phase \in NormalPhases
  /\ effectSucceeds \in BOOLEAN
  /\ IF terminationIntent # None
     THEN UNCHANGED vars
     ELSE IF effectSucceeds
     THEN
       /\ terminationIntent' = "graceful"
       /\ UNCHANGED <<phase, writerQuiesced, normalCompletionReady,
                       cleanupFailure, cleanupRetryDue, processResolution,
                       visibleVersion, settledVersion>>
     ELSE EnterFailure

RequestForce(effectSucceeds) ==
  /\ phase \in NormalPhases
  /\ effectSucceeds \in BOOLEAN
  /\ IF terminationIntent = "force"
     THEN UNCHANGED vars
     ELSE IF effectSucceeds
     THEN
       /\ terminationIntent' = "force"
       /\ UNCHANGED <<phase, writerQuiesced, normalCompletionReady,
                       cleanupFailure, cleanupRetryDue, processResolution,
                       visibleVersion, settledVersion>>
     ELSE EnterFailure

ContainmentBlocked(cleanup) ==
  /\ phase = "containing_failure"
  /\ cleanup \in CleanupFailures
  /\ phase' = "containment_blocked"
  /\ cleanupFailure' = cleanup
  /\ cleanupRetryDue' = FALSE
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, processResolution, visibleVersion,
                  settledVersion>>

CleanupRetryDue ==
  /\ phase = "containment_blocked"
  /\ ~cleanupRetryDue
  /\ cleanupRetryDue' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, processResolution,
                  visibleVersion, settledVersion>>

RetryContainment(succeeds, nextCleanup, observation) ==
  /\ phase = "containment_blocked"
  /\ cleanupRetryDue
  /\ succeeds \in BOOLEAN
  /\ nextCleanup \in CleanupFailures
  /\ observation \in CompletedProcessResolutions
  /\ phase' = IF succeeds THEN "failed" ELSE phase
  /\ cleanupFailure' = IF succeeds THEN None ELSE nextCleanup
  /\ cleanupRetryDue' = FALSE
  /\ processResolution' = IF succeeds THEN observation ELSE "pending"
  /\ settledVersion' = IF succeeds THEN visibleVersion ELSE None
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, visibleVersion>>

CleanupRetryAttempt ==
  \E succeeds \in BOOLEAN, nextCleanup \in CleanupFailures,
     observation \in CompletedProcessResolutions:
    RetryContainment(succeeds, nextCleanup, observation)

ConfirmContainment(observation) ==
  /\ phase = "containing_failure"
  /\ observation \in CompletedProcessResolutions
  /\ phase' = "failed"
  /\ cleanupFailure' = None
  /\ cleanupRetryDue' = FALSE
  /\ processResolution' = observation
  /\ settledVersion' = visibleVersion
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, visibleVersion>>

MutateVisibleState(nextVersion) ==
  /\ phase \notin SettledPhases \cup {"disposed"}
  /\ ~normalCompletionReady
  /\ nextVersion \in Versions
  /\ nextVersion # visibleVersion
  /\ visibleVersion' = nextVersion
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  processResolution, settledVersion>>

Dispose ==
  /\ phase \in SettledPhases
  /\ phase' = "disposed"
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  processResolution, visibleVersion, settledVersion>>

Next ==
  \/ ChildExited
  \/ PtyDrained
  \/ WriterQuiesced
  \/ ClassifyNormalCompletion
  \/ EnterFailure
  \/ \E succeeds \in BOOLEAN: RequestGraceful(succeeds)
  \/ \E succeeds \in BOOLEAN: RequestForce(succeeds)
  \/ \E cleanup \in CleanupFailures: ContainmentBlocked(cleanup)
  \/ CleanupRetryDue
  \/ CleanupRetryAttempt
  \/ \E observation \in CompletedProcessResolutions:
       ConfirmContainment(observation)
  \/ \E version \in Versions: MutateVisibleState(version)
  \/ Dispose

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(CleanupRetryDue)
  /\ WF_vars(CleanupRetryAttempt)

TypeOK ==
  /\ phase \in TerminalPhases
  /\ terminationIntent \in TerminationIntents
  /\ writerQuiesced \in BOOLEAN
  /\ normalCompletionReady \in BOOLEAN
  /\ cleanupFailure \in CleanupFailures \cup {None}
  /\ cleanupRetryDue \in BOOLEAN
  /\ processResolution \in ProcessResolutions
  /\ visibleVersion \in Versions
  /\ settledVersion \in Versions \cup {None}

LifecycleIntegrity ==
  /\ (phase # "disposed" /\ normalCompletionReady) =>
       phase \in {"finishing_child", "finishing_output", "exited"}
  /\ phase = "running" =>
       /\ ~normalCompletionReady
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution = "pending"
       /\ settledVersion = None
  /\ phase \in {"finishing_child", "finishing_output"} =>
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution = "pending"
       /\ settledVersion = None
  /\ phase = "containing_failure" =>
       /\ terminationIntent = "force"
       /\ ~normalCompletionReady
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution = "pending"
       /\ settledVersion = None
  /\ phase = "containment_blocked" =>
       /\ terminationIntent = "force"
       /\ ~normalCompletionReady
       /\ cleanupFailure \in CleanupFailures
       /\ processResolution = "pending"
       /\ settledVersion = None
  /\ phase = "exited" =>
       /\ writerQuiesced
       /\ normalCompletionReady
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution = "pending"
       /\ settledVersion = visibleVersion
  /\ phase = "failed" =>
       /\ terminationIntent = "force"
       /\ ~normalCompletionReady
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution \in CompletedProcessResolutions
       /\ settledVersion = visibleVersion
  /\ phase = "disposed" => settledVersion = visibleVersion

NormalExitRequiresClassification ==
  phase = "exited" => normalCompletionReady

NormalExitRequiresQuiescedWriter ==
  phase = "exited" => writerQuiesced

CleanupFailureIsContextual ==
  /\ (cleanupFailure # None) <=> (phase = "containment_blocked")
  /\ cleanupRetryDue => phase = "containment_blocked"

CleanupRetryIsAlwaysAvailable ==
  phase = "containment_blocked" /\ cleanupRetryDue =>
    ENABLED CleanupRetryAttempt

FailureCannotBecomeNormalExit ==
  [][(phase \in FailurePhases) => (phase' # "exited")]_vars

SettledStateIsFrozen ==
  [][(/\ phase \in SettledPhases
      /\ phase' \in SettledPhases) =>
       UNCHANGED <<visibleVersion, settledVersion>>]_vars

BlockedCleanupEventuallyBecomesDue ==
  (phase = "containment_blocked" /\ ~cleanupRetryDue) ~>
    (cleanupRetryDue \/ phase # "containment_blocked")

BlockedCleanupEventuallyRetries ==
  (phase = "containment_blocked" /\ cleanupRetryDue) ~>
    ~cleanupRetryDue

=============================================================================
