------------------ MODULE TerminalSelectionGestureCoordination ------------------
EXTENDS Naturals, TLC

CONSTANTS MaxGeneration, MaxRevision, MaxPublications, MaxSelectionVersion,
  MaxFailures, MaxGenerationCancellations

\* Models the client-owned serialized pointer-selection lane. One pointer
\* gesture captures one projection generation and carries viewport-relative
\* endpoints. Revision advancement represents arbitrarily frequent terminal
\* output: it never changes the gesture identity and never disables settlement.
\* A generation change invalidates the captured viewport and silently retires
\* the complete local gesture. Programmatic absolute selection is outside this
\* model and remains exact-revision work.

NoAction == "none"
GestureActions == {"start", "update", "end", "cancel"}
EndpointActions == {"update", "end", "cancel"}
ActionNames == {
  "init", "start-gesture", "restart-gesture", "move-gesture", "end-gesture",
  "cancel-gesture", "dispatch-pending", "accept-selection",
  "reject-stale-generation", "reject-selection", "advance-revision",
  "advance-generation"
}

VARIABLES generation, revision, pointerActive, hostGestureActive,
  gestureGeneration, inFlight, pending, flightGeneration, publicationCount,
  selectionVersion, copyCount, failureCount, generationCancellationCount,
  generationBarrier, selectionAtGenerationBarrier, copyAtGenerationBarrier,
  failureAtGenerationBarrier, lastAction

vars == <<generation, revision, pointerActive, hostGestureActive,
  gestureGeneration, inFlight, pending, flightGeneration, publicationCount,
  selectionVersion, copyCount, failureCount, generationCancellationCount,
  generationBarrier, selectionAtGenerationBarrier, copyAtGenerationBarrier,
  failureAtGenerationBarrier, lastAction>>

Init ==
  /\ generation = 1
  /\ revision = 1
  /\ pointerActive = FALSE
  /\ hostGestureActive = FALSE
  /\ gestureGeneration = 0
  /\ inFlight = NoAction
  /\ pending = NoAction
  /\ flightGeneration = 0
  /\ publicationCount = 0
  /\ selectionVersion = 0
  /\ copyCount = 0
  /\ failureCount = 0
  /\ generationCancellationCount = 0
  /\ generationBarrier = FALSE
  /\ selectionAtGenerationBarrier = 0
  /\ copyAtGenerationBarrier = 0
  /\ failureAtGenerationBarrier = 0
  /\ lastAction = "init"

PublishGesture(action, commandGeneration) ==
  /\ action \in GestureActions
  /\ commandGeneration \in 1..MaxGeneration
  /\ inFlight = NoAction
  /\ publicationCount < MaxPublications
  /\ inFlight' = action
  /\ pending' = NoAction
  /\ flightGeneration' = commandGeneration
  /\ publicationCount' = publicationCount + 1

StartGesture ==
  /\ ~generationBarrier
  /\ ~pointerActive
  /\ pending = NoAction
  /\ PublishGesture("start", generation)
  /\ pointerActive' = TRUE
  /\ gestureGeneration' = generation
  /\ lastAction' = "start-gesture"
  /\ UNCHANGED <<generation, revision, hostGestureActive, selectionVersion,
       copyCount, failureCount, generationCancellationCount,
       generationBarrier, selectionAtGenerationBarrier,
       copyAtGenerationBarrier, failureAtGenerationBarrier>>

RestartGesture ==
  /\ generationBarrier
  /\ ~pointerActive
  /\ pending = NoAction
  /\ PublishGesture("start", generation)
  /\ pointerActive' = TRUE
  /\ gestureGeneration' = generation
  /\ generationBarrier' = FALSE
  /\ lastAction' = "restart-gesture"
  /\ UNCHANGED <<generation, revision, hostGestureActive, selectionVersion,
       copyCount, failureCount, generationCancellationCount,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

MoveGesture ==
  /\ pointerActive
  /\ gestureGeneration \in 1..MaxGeneration
  /\ (inFlight # NoAction \/ publicationCount < MaxPublications)
  /\ IF inFlight = NoAction
       THEN /\ inFlight' = "update"
            /\ pending' = NoAction
            /\ flightGeneration' = gestureGeneration
            /\ publicationCount' = publicationCount + 1
       ELSE /\ pending' = "update"
            /\ UNCHANGED <<inFlight, flightGeneration, publicationCount>>
  /\ lastAction' = "move-gesture"
  /\ UNCHANGED <<generation, revision, pointerActive, hostGestureActive,
       gestureGeneration, selectionVersion, copyCount, failureCount,
       generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

EndGesture ==
  /\ pointerActive
  /\ gestureGeneration \in 1..MaxGeneration
  /\ (inFlight # NoAction \/ publicationCount < MaxPublications)
  /\ IF inFlight = NoAction
       THEN /\ inFlight' = "end"
            /\ pending' = NoAction
            /\ flightGeneration' = gestureGeneration
            /\ publicationCount' = publicationCount + 1
       ELSE /\ pending' = "end"
            /\ UNCHANGED <<inFlight, flightGeneration, publicationCount>>
  /\ pointerActive' = FALSE
  /\ lastAction' = "end-gesture"
  /\ UNCHANGED <<generation, revision, hostGestureActive,
       gestureGeneration, selectionVersion, copyCount, failureCount,
       generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

CancelGesture ==
  /\ pointerActive
  /\ gestureGeneration \in 1..MaxGeneration
  /\ (inFlight # NoAction \/ publicationCount < MaxPublications)
  /\ IF inFlight = NoAction
       THEN /\ inFlight' = "cancel"
            /\ pending' = NoAction
            /\ flightGeneration' = gestureGeneration
            /\ publicationCount' = publicationCount + 1
       ELSE /\ pending' = "cancel"
            /\ UNCHANGED <<inFlight, flightGeneration, publicationCount>>
  /\ pointerActive' = FALSE
  /\ lastAction' = "cancel-gesture"
  /\ UNCHANGED <<generation, revision, hostGestureActive,
       gestureGeneration, selectionVersion, copyCount, failureCount,
       generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

DispatchPending ==
  /\ ~generationBarrier
  /\ inFlight = NoAction
  /\ pending \in EndpointActions
  /\ gestureGeneration \in 1..MaxGeneration
  /\ publicationCount < MaxPublications
  /\ inFlight' = pending
  /\ pending' = NoAction
  /\ flightGeneration' = gestureGeneration
  /\ publicationCount' = publicationCount + 1
  /\ lastAction' = "dispatch-pending"
  /\ UNCHANGED <<generation, revision, pointerActive, hostGestureActive,
       gestureGeneration, selectionVersion, copyCount, failureCount,
       generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

AcceptSelection ==
  /\ inFlight \in GestureActions
  /\ flightGeneration = generation
  /\ (inFlight = "start" \/ hostGestureActive)
  /\ selectionVersion < MaxSelectionVersion
  /\ inFlight' = NoAction
  /\ flightGeneration' = 0
  /\ hostGestureActive' = (inFlight \in {"start", "update"})
  /\ gestureGeneration' =
       IF inFlight \in {"end", "cancel"} /\ pending = NoAction
       THEN 0
       ELSE gestureGeneration
  /\ selectionVersion' = selectionVersion + 1
  /\ copyCount' = copyCount + (IF inFlight = "end" THEN 1 ELSE 0)
  /\ lastAction' = "accept-selection"
  /\ UNCHANGED <<generation, revision, pointerActive, pending,
       publicationCount, failureCount, generationCancellationCount,
       generationBarrier, selectionAtGenerationBarrier,
       copyAtGenerationBarrier, failureAtGenerationBarrier>>

RejectStaleGeneration ==
  /\ inFlight \in GestureActions
  /\ flightGeneration # generation
  /\ pointerActive' = FALSE
  /\ gestureGeneration' = 0
  /\ inFlight' = NoAction
  /\ pending' = NoAction
  /\ flightGeneration' = 0
  /\ generationCancellationCount' =
       IF generationCancellationCount < MaxGenerationCancellations
       THEN generationCancellationCount + 1
       ELSE generationCancellationCount
  /\ generationBarrier' = TRUE
  /\ selectionAtGenerationBarrier' = selectionVersion
  /\ copyAtGenerationBarrier' = copyCount
  /\ failureAtGenerationBarrier' = failureCount
  /\ lastAction' = "reject-stale-generation"
  /\ UNCHANGED <<generation, revision, hostGestureActive, publicationCount,
       selectionVersion, copyCount, failureCount>>

RejectSelection ==
  /\ inFlight \in GestureActions
  /\ flightGeneration = generation
  /\ inFlight' = NoAction
  /\ flightGeneration' = 0
  /\ failureCount' =
       IF failureCount < MaxFailures THEN failureCount + 1 ELSE failureCount
  /\ lastAction' = "reject-selection"
  /\ UNCHANGED <<generation, revision, pointerActive, hostGestureActive,
       gestureGeneration, pending, publicationCount, selectionVersion,
       copyCount, generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

AdvanceRevision ==
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ lastAction' = "advance-revision"
  /\ UNCHANGED <<generation, pointerActive, hostGestureActive,
       gestureGeneration, inFlight, pending, flightGeneration,
       publicationCount, selectionVersion, copyCount, failureCount,
       generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

AdvanceGeneration ==
  /\ generation < MaxGeneration
  /\ generation' = generation + 1
  /\ revision' = 1
  /\ hostGestureActive' = FALSE
  /\ lastAction' = "advance-generation"
  /\ UNCHANGED <<pointerActive, gestureGeneration, inFlight, pending,
       flightGeneration, publicationCount, selectionVersion, copyCount,
       failureCount, generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

SettleFlight ==
  \/ AcceptSelection
  \/ RejectStaleGeneration
  \/ RejectSelection

Next ==
  \/ StartGesture
  \/ RestartGesture
  \/ MoveGesture
  \/ EndGesture
  \/ CancelGesture
  \/ DispatchPending
  \/ SettleFlight
  \/ AdvanceRevision
  \/ AdvanceGeneration

Spec == Init /\ [][Next]_vars /\ WF_vars(SettleFlight)

TypeOK ==
  /\ generation \in 1..MaxGeneration
  /\ revision \in 1..MaxRevision
  /\ pointerActive \in BOOLEAN
  /\ hostGestureActive \in BOOLEAN
  /\ gestureGeneration \in 0..MaxGeneration
  /\ inFlight \in GestureActions \cup {NoAction}
  /\ pending \in EndpointActions \cup {NoAction}
  /\ flightGeneration \in 0..MaxGeneration
  /\ publicationCount \in 0..MaxPublications
  /\ selectionVersion \in 0..MaxSelectionVersion
  /\ copyCount \in 0..MaxSelectionVersion
  /\ failureCount \in 0..MaxFailures
  /\ generationCancellationCount \in 0..MaxGenerationCancellations
  /\ generationBarrier \in BOOLEAN
  /\ selectionAtGenerationBarrier \in 0..MaxSelectionVersion
  /\ copyAtGenerationBarrier \in 0..MaxSelectionVersion
  /\ failureAtGenerationBarrier \in 0..MaxFailures
  /\ lastAction \in ActionNames

FlightIdentityIsComplete ==
  (inFlight = NoAction) = (flightGeneration = 0)

GestureWorkKeepsCapturedGeneration ==
  /\ (pointerActive \/ inFlight # NoAction \/ pending # NoAction) =>
       gestureGeneration \in 1..MaxGeneration
  /\ inFlight # NoAction => flightGeneration = gestureGeneration

HostGestureRequiresAcceptedSelection ==
  hostGestureActive => selectionVersion > 0

GenerationBarrierClearsGestureWork ==
  generationBarrier =>
    /\ ~pointerActive
    /\ gestureGeneration = 0
    /\ inFlight = NoAction
    /\ pending = NoAction

GenerationBarrierPreservesSelection ==
  generationBarrier => selectionVersion = selectionAtGenerationBarrier

GenerationBarrierDoesNotCopy ==
  generationBarrier => copyCount = copyAtGenerationBarrier

GenerationBarrierDoesNotSurfaceFailure ==
  generationBarrier => failureCount = failureAtGenerationBarrier

GenerationBarrierRecordsCancellation ==
  generationBarrier => generationCancellationCount > 0

EveryAdmittedFlightEventuallySettles ==
  [](inFlight # NoAction => <>(inFlight = NoAction))

=============================================================================
