------------------------ MODULE TerminalHostLifecycle ------------------------
EXTENDS Naturals, TLC

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
\* Core geometry, or visible frame. Start receives the mounted geometry and
\* publishes it only after spawn succeeds. A failed start is a stutter that
\* retains the same startable owner. visibleGeometry carries only geometry;
\* visibleRevision separately counts every canonical resize commit or content
\* mutation. resizeEffectInFlight exposes the production interval in which a
\* platform resize has started but its canonical commit or compensation has
\* not settled. Normal classification waits for that interval to close.
\* EnterFailure is the visible-state fence: the geometry and revision committed
\* before that transition remain immutable throughout containment and after
\* settlement. settledGeometry and settledRevision capture the exact tuple
\* published by the terminal outcome
\* and remains immutable until explicit disposal.
\* RetainSettledCleanup covers both fatal aggregate teardown and a Dispose whose
\* runtime shutdown fails after the terminal outcome has settled. Cleanup then
\* retries independently without changing the settled outcome.

CONSTANTS Geometries, MaxRevision

None == "none"
NoRevision == MaxRevision + 1
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
  resizeTargetGeometry,
  visibleGeometry,
  visibleRevision,
  settledGeometry,
  settledRevision

vars ==
  <<phase, terminationIntent, writerQuiesced, normalCompletionReady,
    cleanupFailure, cleanupRetryDue, outputAbortRequested, processResolution,
    retainedCleanupState, retainedCleanupFailure, retainedCleanupRetryDue,
    resizeEffectInFlight, resizeTargetGeometry, visibleGeometry,
    visibleRevision, settledGeometry, settledRevision>>

Init ==
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
  /\ resizeTargetGeometry = None
  /\ visibleGeometry = None
  /\ visibleRevision = NoRevision
  /\ settledGeometry = None
  /\ settledRevision = NoRevision

Start(initialGeometry, succeeds) ==
  /\ phase = "awaiting_start"
  /\ initialGeometry \in Geometries
  /\ succeeds \in BOOLEAN
  /\ IF succeeds
     THEN
       /\ phase' = "running"
       /\ visibleGeometry' = initialGeometry
       /\ visibleRevision' = 0
       /\ UNCHANGED <<terminationIntent, writerQuiesced,
                       normalCompletionReady, cleanupFailure, cleanupRetryDue,
                       outputAbortRequested,
                       retainedCleanupState, retainedCleanupFailure,
                       retainedCleanupRetryDue, processResolution,
                       resizeEffectInFlight, resizeTargetGeometry,
                       settledGeometry, settledRevision>>
     ELSE UNCHANGED vars

ChildExited ==
  /\ \/ phase = "running"
     \/ /\ phase = "finishing_output"
        /\ normalCompletionReady
  /\ phase' = IF phase = "finishing_output" THEN "exited"
                ELSE "finishing_child"
  /\ settledGeometry' = IF phase = "finishing_output" THEN visibleGeometry
                         ELSE settledGeometry
  /\ settledRevision' = IF phase = "finishing_output" THEN visibleRevision
                         ELSE settledRevision
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, resizeEffectInFlight,
                  resizeTargetGeometry, visibleGeometry, visibleRevision>>

PtyDrained ==
  /\ \/ phase = "running"
     \/ /\ phase = "finishing_child"
        /\ normalCompletionReady
  /\ ~outputAbortRequested
  /\ phase' = IF phase = "finishing_child" THEN "exited"
                ELSE "finishing_output"
  /\ settledGeometry' = IF phase = "finishing_child" THEN visibleGeometry
                         ELSE settledGeometry
  /\ settledRevision' = IF phase = "finishing_child" THEN visibleRevision
                         ELSE settledRevision
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, resizeEffectInFlight,
                  resizeTargetGeometry, visibleGeometry, visibleRevision>>

WriterQuiesced ==
  /\ phase \in NormalPhases
  /\ ~writerQuiesced
  /\ writerQuiesced' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, normalCompletionReady,
                  cleanupFailure, cleanupRetryDue, outputAbortRequested,
                  processResolution,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeEffectInFlight, resizeTargetGeometry, visibleGeometry,
                  settledGeometry, visibleRevision, settledRevision>>

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
                  resizeEffectInFlight, resizeTargetGeometry, visibleGeometry,
                  settledGeometry, visibleRevision, settledRevision>>

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
                  resizeTargetGeometry, visibleGeometry, settledGeometry,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, visibleRevision, settledRevision>>

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
                       resizeEffectInFlight, resizeTargetGeometry,
                       visibleGeometry, settledGeometry, visibleRevision, settledRevision>>
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
                       resizeEffectInFlight, resizeTargetGeometry,
                       visibleGeometry, settledGeometry, visibleRevision, settledRevision>>
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
                  resizeEffectInFlight, resizeTargetGeometry, visibleGeometry,
                  settledGeometry, visibleRevision, settledRevision>>

CleanupRetryDue ==
  /\ phase = "containment_blocked"
  /\ ~cleanupRetryDue
  /\ cleanupRetryDue' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure,
                  outputAbortRequested, processResolution,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeEffectInFlight, resizeTargetGeometry, visibleGeometry,
                  settledGeometry, visibleRevision, settledRevision>>

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
  /\ settledGeometry' = IF succeeds THEN visibleGeometry ELSE None
  /\ settledRevision' = IF succeeds THEN visibleRevision ELSE NoRevision
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, outputAbortRequested,
                  resizeEffectInFlight,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeTargetGeometry, visibleGeometry, visibleRevision>>

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
  /\ settledGeometry' = visibleGeometry
  /\ settledRevision' = visibleRevision
  /\ UNCHANGED <<terminationIntent, writerQuiesced,
                  normalCompletionReady, outputAbortRequested,
                  resizeEffectInFlight,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue,
                  resizeTargetGeometry, visibleGeometry, visibleRevision>>

RequestOutputAbort ==
  /\ phase \in NormalPhases \cup FailurePhases \cup SettledPhases
  /\ ~outputAbortRequested
  /\ outputAbortRequested' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution,
                  resizeEffectInFlight, resizeTargetGeometry, visibleGeometry,
                  settledGeometry, visibleRevision, settledRevision>>

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
                  resizeTargetGeometry, visibleGeometry, settledGeometry, visibleRevision, settledRevision>>

RetainedCleanupRetryDue ==
  /\ retainedCleanupState = "blocked"
  /\ ~retainedCleanupRetryDue
  /\ retainedCleanupRetryDue' = TRUE
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  processResolution, resizeEffectInFlight,
                  resizeTargetGeometry, visibleGeometry, settledGeometry, visibleRevision, settledRevision>>

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
                  resizeTargetGeometry, visibleGeometry, settledGeometry, visibleRevision, settledRevision>>

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
                  resizeEffectInFlight, resizeTargetGeometry, visibleGeometry,
                  settledGeometry, visibleRevision, settledRevision>>

BeginResizeEffect(nextGeometry) ==
  /\ phase \in NormalPhases
  /\ ~normalCompletionReady
  /\ ~resizeEffectInFlight
  /\ nextGeometry \in Geometries
  /\ nextGeometry # visibleGeometry
  /\ resizeEffectInFlight' = TRUE
  /\ resizeTargetGeometry' = nextGeometry
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, visibleGeometry,
                  settledGeometry, visibleRevision, settledRevision>>

CommitResizeEffect ==
  /\ resizeEffectInFlight
  /\ phase \in NormalPhases
  /\ ~normalCompletionReady
  /\ visibleRevision < MaxRevision
  /\ visibleGeometry' = resizeTargetGeometry
  /\ visibleRevision' = visibleRevision + 1
  /\ resizeEffectInFlight' = FALSE
  /\ resizeTargetGeometry' = None
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution,
                  settledGeometry, settledRevision>>

CompensateResizeEffect ==
  /\ resizeEffectInFlight
  /\ phase # "disposed"
  /\ resizeEffectInFlight' = FALSE
  /\ resizeTargetGeometry' = None
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, visibleGeometry,
                  settledGeometry, visibleRevision, settledRevision>>

MutateVisibleState ==
  /\ phase \in NormalPhases
  /\ ~normalCompletionReady
  /\ visibleRevision < MaxRevision
  /\ visibleRevision' = visibleRevision + 1
  /\ UNCHANGED <<phase, terminationIntent, writerQuiesced,
                  normalCompletionReady, cleanupFailure, cleanupRetryDue,
                  outputAbortRequested,
                  retainedCleanupState, retainedCleanupFailure,
                  retainedCleanupRetryDue, processResolution, resizeEffectInFlight,
                  resizeTargetGeometry, visibleGeometry, settledGeometry,
                  settledRevision>>

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
                  resizeTargetGeometry, visibleGeometry, settledGeometry, visibleRevision, settledRevision>>

Next ==
  \/ \E initialGeometry \in Geometries, succeeds \in BOOLEAN:
       Start(initialGeometry, succeeds)
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
  \/ \E geometry \in Geometries: BeginResizeEffect(geometry)
  \/ CommitResizeEffect
  \/ CompensateResizeEffect
  \/ MutateVisibleState
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
  /\ resizeTargetGeometry \in Geometries \cup {None}
  /\ visibleGeometry \in Geometries \cup {None}
  /\ visibleRevision \in 0..NoRevision
  /\ settledGeometry \in Geometries \cup {None}
  /\ settledRevision \in 0..NoRevision

LifecycleIntegrity ==
  /\ resizeEffectInFlight <=> resizeTargetGeometry # None
  /\ resizeEffectInFlight => ~normalCompletionReady
  /\ (phase # "disposed" /\ normalCompletionReady) =>
       phase \in {"finishing_child", "finishing_output", "exited"}
  /\ phase = "awaiting_start" =>
       /\ visibleGeometry = None
       /\ visibleRevision = NoRevision
       /\ settledGeometry = None
       /\ settledRevision = NoRevision
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
       /\ resizeTargetGeometry = None
  /\ phase = "running" =>
       /\ visibleGeometry \in Geometries
       /\ visibleRevision \in 0..MaxRevision
       /\ ~normalCompletionReady
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution = "pending"
       /\ settledGeometry = None
       /\ settledRevision = NoRevision
  /\ phase \in {"finishing_child", "finishing_output"} =>
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution = "pending"
       /\ settledGeometry = None
       /\ settledRevision = NoRevision
  /\ phase = "containing_failure" =>
       /\ terminationIntent = "force"
       /\ ~normalCompletionReady
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution = "pending"
       /\ settledGeometry = None
       /\ settledRevision = NoRevision
  /\ phase = "containment_blocked" =>
       /\ terminationIntent = "force"
       /\ ~normalCompletionReady
       /\ cleanupFailure \in CleanupFailures
       /\ processResolution = "pending"
       /\ settledGeometry = None
       /\ settledRevision = NoRevision
  /\ phase = "exited" =>
       /\ writerQuiesced
       /\ normalCompletionReady
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution = "pending"
       /\ settledGeometry = visibleGeometry
       /\ settledRevision = visibleRevision
  /\ phase = "failed" =>
       /\ terminationIntent = "force"
       /\ ~normalCompletionReady
       /\ cleanupFailure = None
       /\ ~cleanupRetryDue
       /\ processResolution \in CompletedProcessResolutions
       /\ settledGeometry = visibleGeometry
       /\ settledRevision = visibleRevision
  /\ phase = "disposed" =>
       \/ /\ visibleGeometry = None
           /\ visibleRevision = NoRevision
           /\ settledGeometry = None
           /\ settledRevision = NoRevision
       \/ /\ visibleGeometry \in Geometries
           /\ visibleRevision \in 0..MaxRevision
           /\ settledGeometry = visibleGeometry
           /\ settledRevision = visibleRevision
  /\ phase \in FailurePhases => outputAbortRequested
  /\ retainedCleanupState \in {"blocked", "exhausted"} =>
       phase \in SettledPhases
  /\ retainedCleanupState = "complete" =>
       phase \in SettledPhases \cup {"disposed"}

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

StartPublishesOnlyItsMountedGeometry ==
  [][\A initialGeometry \in Geometries:
       Start(initialGeometry, TRUE) =>
         /\ visibleGeometry' = initialGeometry
         /\ visibleRevision' = 0]_vars

ResizeCommitAdvancesCurrentRevision ==
  [][CommitResizeEffect =>
       /\ visibleGeometry' = resizeTargetGeometry
       /\ visibleRevision' = visibleRevision + 1]_vars

ContentMutationPreservesGeometry ==
  [][MutateVisibleState =>
       /\ UNCHANGED visibleGeometry
       /\ visibleRevision' = visibleRevision + 1]_vars

FailureCannotBecomeNormalExit ==
  [][(phase \in FailurePhases) => (phase' # "exited")]_vars

SettledStateIsFrozen ==
  [][(/\ phase \in SettledPhases
      /\ phase' \in SettledPhases) =>
       UNCHANGED <<visibleGeometry, visibleRevision,
                   settledGeometry, settledRevision>>]_vars

RetainedCleanupPreservesSettlement ==
  [][(/\ phase \in SettledPhases
      /\ retainedCleanupState # "inactive"
      /\ phase' \in SettledPhases) =>
       UNCHANGED <<phase, visibleGeometry, visibleRevision,
                   settledGeometry, settledRevision>>]_vars

FailureEntryFreezesVisibleState ==
  [][(phase \in FailurePhases \/ phase' \in FailurePhases) =>
       UNCHANGED <<visibleGeometry, visibleRevision>>]_vars

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
