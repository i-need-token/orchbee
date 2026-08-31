------------------------ MODULE TerminalHostLifecycle ------------------------
EXTENDS TLC

\* Refines runtime-model claims TH-008 and TH-009 through Start,
\* ChildExited, PtyDrained, WriterQuiesced, ClassifyNormalCompletion, EnterFailure,
\* ContainmentBlocked, CleanupRetryDue, RetryContainment,
\* ConfirmContainment, RequestOutputAbort, RetainSettledCleanup,
\* RetryRetainedCleanup,
\* ExhaustRetainedCleanup, BeginResizeEffect, CommitResizeEffect,
\* CompensateResizeEffect, MutateVisibleState, and Dispose. Registry identity,
\* broker retention, and multi-runtime shutdown are intentionally owned by
\* TerminalHostRegistry and TerminalHostShutdown so their independent state
\* dimensions do not multiply this per-terminal lifecycle proof.
\* Awaiting start retains identity and launch ownership without a child, PTY,
\* Core geometry, or visible frame. Start receives the mounted geometry; its
\* abstract version becomes the first runtime-visible version only after spawn
\* succeeds. A failed start is a stutter that retains the same startable owner.
\* visibleVersion abstracts content, generation, geometry, configuration, and
\* revision. resizeEffectInFlight exposes the production interval in which a
\* platform resize has started but its canonical commit or compensation has
\* not settled. Normal classification waits for that interval to close.
\* EnterFailure is the visible-state fence: the version committed before that
\* transition remains immutable throughout containment and after settlement.
\* settledVersion captures the exact tuple published by the terminal outcome
\* and remains immutable until explicit disposal.
\* RetainSettledCleanup covers both fatal aggregate teardown and a Dispose whose
\* runtime shutdown fails after the terminal outcome has settled. Cleanup then
\* retries independently without changing the settled outcome.

CONSTANTS Versions, InitialVersion

None == "none"
NormalPhases == {"running", "finishing_child", "finishing_output"}
FailurePhases == {"containing_failure", "containment_blocked", "failed"}
SettledPhases == {"exited", "failed"}
TerminalPhases ==
  NormalPhases \cup FailurePhases \cup SettledPhases \cup {"awaiting_start", "disposed"}
TerminationIntents == {None, "graceful", "force"}
CleanupFailures == {
  "contain_process",
  "release_output",
  "release_process_identity"
}
RetainedCleanupStates == {"inactive", "blocked", "complete", "exhausted"}
ProcessResolutions == {"pending", "observed", "unavailable"}
CompletedProcessResolutions == {"observed", "unavailable"}

VARIABLES
  phase,
  terminationIntent,
  writerQuiesced,
  normalCompletionReady,
  cleanupFailure,
  cleanupRetryDue,
  outputAbortRequested,
  retainedCleanupState,
  retainedCleanupFailure,
  retainedCleanupRetryDue,
  processResolution,
  resizeEffectInFlight,
  resizeTargetVersion,
  visibleVersion,
  settledVersion

vars ==
  <<phase, terminationIntent, writerQuiesced, normalCompletionReady,
    cleanupFailure, cleanupRetryDue, outputAbortRequested, processResolution,
    retainedCleanupState, retainedCleanupFailure, retainedCleanupRetryDue,
    resizeEffectInFlight, resizeTargetVersion, visibleVersion,
    settledVersion>>

Init ==
  /\ InitialVersion \in Versions
  /\ phase = "awaiting_start"
  /\ terminationIntent = None
  /\ writerQuiesced = FALSE
  /\ normalCompletionReady = FALSE
  /\ cleanupFailure = None
  /\ cleanupRetryDue = FALSE
  /\ outputAbortRequested = FALSE
  /\ retainedCleanupState = "inactive"
  /\ retainedCleanupFailure = None
  /\ retainedCleanupRetryDue = FALSE
  /\ processResolution = "pending"
  /\ resizeEffectInFlight = FALSE
  /\ resizeTargetVersion = None
  /\ visibleVersion = None
  /\ settledVersion = None

Start(initialVersion, succeeds) ==
  /\ phase = "awaiting_start"
  /\ initialVersion \in Versions
  /\ succeeds \in BOOLEAN
  /\ IF succeeds
     THEN
       /\ phase' = "running"
       /\ visibleVersion' = initialVersion
       /\ UNCHANGED <<terminationIntent, writerQuiesced,
                       normalCompletionReady, cleanupFailure, cleanupRetryDue,
                       outputAbortRequested,
                       retainedCleanupState, retainedCleanupFailure,
                       retainedCleanupRetryDue, processResolution,
                       resizeEffectInFlight, resizeTargetVersion,
                       settledVersion>>
     ELSE UNCHANGED vars

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
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, resizeEffectInFlight,
                  resizeTargetVersion, visibleVersion>>

PtyDrained ==
  /\ \/ phase = "running"
     \/ /\ phase = "finishing_child"
        /\ normalCompletionReady
  /\ ~outputAbortRequested
  /\ phase' = IF phase = "finishing_child" THEN "exited"
                ELSE "finishing_output"
  /\ settledVersion' = IF phase = "finishing_child" THEN visibleVersion
                         ELSE settledVersion
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, resizeEffectInFlight,
                  resizeTargetVersion, visibleVersion>>

WriterQuiesced ==
  /\ phase \in NormalPhases
  /\ ~writerQuiesced
  /\ writerQuiesced' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, normalCompletionReady,
                  cleanupFailure, cleanupRetryDue, outputAbortRequested,
                  processResolution,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeEffectInFlight, resizeTargetVersion, visibleVersion,
                  settledVersion>>

ClassifyNormalCompletion ==
  /\ phase \in {"finishing_child", "finishing_output"}
  /\ writerQuiesced
  /\ ~normalCompletionReady
  /\ ~resizeEffectInFlight
  /\ normalCompletionReady' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  cleanupFailure, cleanupRetryDue, outputAbortRequested,
                  processResolution,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeEffectInFlight, resizeTargetVersion, visibleVersion,
                  settledVersion>>

EnterFailure ==
  /\ phase \in NormalPhases
  /\ phase' = "containing_failure"
  /\ terminationIntent' = "force"
  /\ normalCompletionReady' = FALSE
  /\ cleanupFailure' = None
  /\ cleanupRetryDue' = FALSE
  /\ outputAbortRequested' = TRUE
  /\ processResolution' = "pending"
  /\ UNCHANGED <<writerQuiesced, resizeEffectInFlight,
                  resizeTargetVersion, visibleVersion, settledVersion,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue>>

RequestGraceful(effectSucceeds) ==
  /\ phase \in NormalPhases
  /\ effectSucceeds \in BOOLEAN
  /\ IF terminationIntent # None
     THEN UNCHANGED vars
     ELSE IF effectSucceeds
     THEN
       /\ terminationIntent' = "graceful"
       /\ UNCHANGED <<phase, writerQuiesced, normalCompletionReady,
                       cleanupFailure, cleanupRetryDue, outputAbortRequested,
                       processResolution,
                       retainedCleanupState, retainedCleanupFailure,
                       retainedCleanupRetryDue,
                       resizeEffectInFlight, resizeTargetVersion,
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
                       cleanupFailure, cleanupRetryDue, outputAbortRequested,
                       processResolution,
                       retainedCleanupState, retainedCleanupFailure,
                       retainedCleanupRetryDue,
                       resizeEffectInFlight, resizeTargetVersion,
                       visibleVersion, settledVersion>>
     ELSE EnterFailure

ContainmentBlocked(cleanup) ==
  /\ phase = "containing_failure"
  /\ cleanup \in CleanupFailures
  /\ phase' = "containment_blocked"
  /\ cleanupFailure' = cleanup
  /\ cleanupRetryDue' = FALSE
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, outputAbortRequested,
                  processResolution,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeEffectInFlight, resizeTargetVersion, visibleVersion,
                  settledVersion>>

CleanupRetryDue ==
  /\ phase = "containment_blocked"
  /\ ~cleanupRetryDue
  /\ cleanupRetryDue' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure,
                  outputAbortRequested, processResolution,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeEffectInFlight, resizeTargetVersion, visibleVersion,
                  settledVersion>>

RetryContainment(succeeds, nextCleanup, observation) ==
  /\ phase = "containment_blocked"
  /\ cleanupRetryDue
  /\ succeeds \in BOOLEAN
  /\ nextCleanup \in CleanupFailures
  /\ observation \in CompletedProcessResolutions
  /\ (succeeds => ~resizeEffectInFlight)
  /\ phase' = IF succeeds THEN "failed" ELSE phase
  /\ cleanupFailure' = IF succeeds THEN None ELSE nextCleanup
  /\ cleanupRetryDue' = FALSE
  /\ processResolution' = IF succeeds THEN observation ELSE "pending"
  /\ settledVersion' = IF succeeds THEN visibleVersion ELSE None
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, outputAbortRequested,
                  resizeEffectInFlight,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeTargetVersion, visibleVersion>>

CleanupRetryAttempt ==
  \E succeeds \in BOOLEAN, nextCleanup \in CleanupFailures,
     observation \in CompletedProcessResolutions:
    RetryContainment(succeeds, nextCleanup, observation)

ConfirmContainment(observation) ==
  /\ phase = "containing_failure"
  /\ observation \in CompletedProcessResolutions
  /\ ~resizeEffectInFlight
  /\ phase' = "failed"
  /\ cleanupFailure' = None
  /\ cleanupRetryDue' = FALSE
  /\ processResolution' = observation
  /\ settledVersion' = visibleVersion
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, outputAbortRequested,
                  resizeEffectInFlight,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeTargetVersion, visibleVersion>>

RequestOutputAbort ==
  /\ phase \in NormalPhases \cup FailurePhases \cup SettledPhases
  /\ ~outputAbortRequested
  /\ outputAbortRequested' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution,
                  resizeEffectInFlight, resizeTargetVersion, visibleVersion,
                  settledVersion>>

RetainSettledCleanup(cleanup) ==
  /\ phase \in SettledPhases
  /\ retainedCleanupState = "inactive"
  /\ cleanup \in CleanupFailures
  /\ retainedCleanupState' = "blocked"
  /\ retainedCleanupFailure' = cleanup
  /\ retainedCleanupRetryDue' = FALSE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  processResolution, resizeEffectInFlight,
                  resizeTargetVersion, visibleVersion, settledVersion>>

RetainedCleanupRetryDue ==
  /\ retainedCleanupState = "blocked"
  /\ ~retainedCleanupRetryDue
  /\ retainedCleanupRetryDue' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  processResolution, resizeEffectInFlight,
                  resizeTargetVersion, visibleVersion, settledVersion>>

RetryRetainedCleanup(succeeds, nextCleanup) ==
  /\ retainedCleanupState = "blocked"
  /\ retainedCleanupRetryDue
  /\ succeeds \in BOOLEAN
  /\ nextCleanup \in CleanupFailures
  /\ retainedCleanupState' = IF succeeds THEN "complete" ELSE "blocked"
  /\ retainedCleanupFailure' = IF succeeds THEN None ELSE nextCleanup
  /\ retainedCleanupRetryDue' = FALSE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  processResolution, resizeEffectInFlight,
                  resizeTargetVersion, visibleVersion, settledVersion>>

RetainedCleanupRetryAttempt ==
  \E succeeds \in BOOLEAN, nextCleanup \in CleanupFailures:
    RetryRetainedCleanup(succeeds, nextCleanup)

ExhaustRetainedCleanup ==
  /\ retainedCleanupState = "blocked"
  /\ retainedCleanupState' = "exhausted"
  /\ retainedCleanupRetryDue' = FALSE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupFailure, processResolution,
                  resizeEffectInFlight, resizeTargetVersion, visibleVersion,
                  settledVersion>>

BeginResizeEffect(nextVersion) ==
  /\ phase \in NormalPhases
  /\ ~normalCompletionReady
  /\ ~resizeEffectInFlight
  /\ nextVersion \in Versions
  /\ nextVersion # visibleVersion
  /\ resizeEffectInFlight' = TRUE
  /\ resizeTargetVersion' = nextVersion
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, visibleVersion,
                  settledVersion>>

CommitResizeEffect ==
  /\ resizeEffectInFlight
  /\ phase \in NormalPhases
  /\ ~normalCompletionReady
  /\ visibleVersion' = resizeTargetVersion
  /\ resizeEffectInFlight' = FALSE
  /\ resizeTargetVersion' = None
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, settledVersion>>

CompensateResizeEffect ==
  /\ resizeEffectInFlight
  /\ phase # "disposed"
  /\ resizeEffectInFlight' = FALSE
  /\ resizeTargetVersion' = None
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, visibleVersion,
                  settledVersion>>

MutateVisibleState(nextVersion) ==
  /\ phase \in NormalPhases
  /\ ~normalCompletionReady
  /\ nextVersion \in Versions
  /\ nextVersion # visibleVersion
  /\ visibleVersion' = nextVersion
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, resizeEffectInFlight,
                  resizeTargetVersion, settledVersion>>

Dispose ==
  /\ phase \in SettledPhases \cup {"awaiting_start"}
  /\ retainedCleanupState \in {"inactive", "complete"}
  /\ ~resizeEffectInFlight
  /\ phase' = "disposed"
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, resizeEffectInFlight,
                  resizeTargetVersion, visibleVersion, settledVersion>>

Next ==
  \/ \E initialVersion \in Versions, succeeds \in BOOLEAN:
       Start(initialVersion, succeeds)
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
  \/ RequestOutputAbort
  \/ \E cleanup \in CleanupFailures: RetainSettledCleanup(cleanup)
  \/ RetainedCleanupRetryDue
  \/ RetainedCleanupRetryAttempt
  \/ ExhaustRetainedCleanup
  \/ \E version \in Versions: BeginResizeEffect(version)
  \/ CommitResizeEffect
  \/ CompensateResizeEffect
  \/ \E version \in Versions: MutateVisibleState(version)
  \/ Dispose

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(CleanupRetryDue)
  /\ WF_vars(CleanupRetryAttempt)
  /\ WF_vars(RetainedCleanupRetryDue)
  /\ WF_vars(RetainedCleanupRetryAttempt)

TypeOK ==
  /\ phase \in TerminalPhases
  /\ terminationIntent \in TerminationIntents
  /\ writerQuiesced \in BOOLEAN
  /\ normalCompletionReady \in BOOLEAN
  /\ cleanupFailure \in CleanupFailures \cup {None}
  /\ cleanupRetryDue \in BOOLEAN
  /\ outputAbortRequested \in BOOLEAN
  /\ retainedCleanupState \in RetainedCleanupStates
  /\ retainedCleanupFailure \in CleanupFailures \cup {None}
  /\ retainedCleanupRetryDue \in BOOLEAN
  /\ processResolution \in ProcessResolutions
  /\ resizeEffectInFlight \in BOOLEAN
  /\ resizeTargetVersion \in Versions \cup {None}
  /\ visibleVersion \in Versions \cup {None}
  /\ settledVersion \in Versions \cup {None}

LifecycleIntegrity ==
  /\ resizeEffectInFlight <=> resizeTargetVersion # None
  /\ resizeEffectInFlight => ~normalCompletionReady
  /\ (phase # "disposed" /\ normalCompletionReady) =>
       phase \in {"finishing_child", "finishing_output", "exited"}
  /\ phase = "awaiting_start" =>
       /\ visibleVersion = None
       /\ settledVersion = None
       /\ terminationIntent = None
       /\ ~writerQuiesced
       /\ ~normalCompletionReady
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ ~outputAbortRequested
       /\ retainedCleanupState = "inactive"
       /\ retainedCleanupFailure = None
       /\ ~retainedCleanupRetryDue
       /\ processResolution = "pending"
       /\ ~resizeEffectInFlight
       /\ resizeTargetVersion = None
  /\ phase = "running" =>
       /\ visibleVersion \in Versions
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
  /\ phase = "disposed" =>
       \/ /\ visibleVersion = None
          /\ settledVersion = None
       \/ /\ visibleVersion \in Versions
          /\ settledVersion = visibleVersion
  /\ phase \in FailurePhases => outputAbortRequested
  /\ retainedCleanupState \in {"blocked", "exhausted"} =>
       phase \in SettledPhases
  /\ retainedCleanupState = "complete" =>
       phase \in SettledPhases \cup {"disposed"}

NormalExitRequiresClassification ==
  phase = "exited" => normalCompletionReady

NormalExitRequiresQuiescedWriter ==
  phase = "exited" => writerQuiesced

AbortPreventsNaturalDrainPublication ==
  [][outputAbortRequested =>
       ~((phase = "running" /\ phase' = "finishing_output") \/
         (phase = "finishing_child" /\ phase' = "exited"))]_vars

CleanupFailureIsContextual ==
  /\ (cleanupFailure # None) <=> (phase = "containment_blocked")
  /\ cleanupRetryDue => phase = "containment_blocked"
  /\ (retainedCleanupFailure # None) <=>
       retainedCleanupState \in {"blocked", "exhausted"}
  /\ retainedCleanupRetryDue => retainedCleanupState = "blocked"

CleanupRetryIsAlwaysAvailable ==
  /\ phase = "containment_blocked" /\ cleanupRetryDue =>
       ENABLED CleanupRetryAttempt
  /\ retainedCleanupState = "blocked" /\ retainedCleanupRetryDue =>
       ENABLED RetainedCleanupRetryAttempt

AwaitingStartHasNoVisibleVersion ==
  phase = "awaiting_start" => visibleVersion = None

StartPublishesOnlyItsMountedVersion ==
  [][(/\ phase = "awaiting_start"
      /\ phase' = "running") =>
       /\ visibleVersion = None
       /\ visibleVersion' \in Versions]_vars

FailureCannotBecomeNormalExit ==
  [][(phase \in FailurePhases) => (phase' # "exited")]_vars

SettledStateIsFrozen ==
  [][(/\ phase \in SettledPhases
      /\ phase' \in SettledPhases) =>
       UNCHANGED <<visibleVersion, settledVersion>>]_vars

RetainedCleanupPreservesSettlement ==
  [][(/\ phase \in SettledPhases
      /\ retainedCleanupState # "inactive"
      /\ phase' \in SettledPhases) =>
       UNCHANGED <<phase, visibleVersion, settledVersion>>]_vars

FailureEntryFreezesVisibleState ==
  [][(phase \in FailurePhases \/ phase' \in FailurePhases) =>
       UNCHANGED visibleVersion]_vars

BlockedCleanupEventuallyBecomesDue ==
  (phase = "containment_blocked" /\ ~cleanupRetryDue) ~>
    (cleanupRetryDue \/ phase # "containment_blocked")

BlockedCleanupEventuallyRetries ==
  (phase = "containment_blocked" /\ cleanupRetryDue) ~>
    ~cleanupRetryDue

RetainedCleanupEventuallyBecomesDue ==
  (retainedCleanupState = "blocked" /\ ~retainedCleanupRetryDue) ~>
    (retainedCleanupRetryDue \/ retainedCleanupState # "blocked")

RetainedCleanupEventuallyRetries ==
  (retainedCleanupState = "blocked" /\ retainedCleanupRetryDue) ~>
    (~retainedCleanupRetryDue \/ retainedCleanupState # "blocked")

=============================================================================
