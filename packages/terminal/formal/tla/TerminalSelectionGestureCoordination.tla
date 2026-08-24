------------------ MODULE TerminalSelectionGestureCoordination ------------------
EXTENDS Naturals, TLC

CONSTANTS MaxGeneration, MaxRevision, MaxPublications, MaxSelectionVersion,
  MaxFailures, MaxStaleCancellations

\* Models the client-owned serialized selection-gesture lane. The abstraction
\* retains one current pointer gesture and one coalesced unsent endpoint. Host
\* selection content and endpoint semantics remain authoritative outside this
\* model. A broker response is assumed to settle the current serialized gesture
\* command; concurrently issued programmatic selection commands are excluded.

NoAction == "none"
GestureActions == {"start", "update", "end", "cancel"}
EndpointActions == {"update", "end", "cancel"}
ActionNames == {
  "init", "start-gesture", "restart-gesture", "move-gesture", "end-gesture",
  "cancel-gesture", "dispatch-pending", "accept-selection",
  "reject-stale-revision", "reject-stale-generation", "reject-selection",
  "advance-revision", "advance-generation"
}

VARIABLES generation, revision, pointerActive, hostGestureActive, inFlight,
  pending, flightGeneration, flightRevision, publicationCount, selectionVersion,
  copyCount, failureCount, staleCancellationCount, staleBarrier,
  selectionAtStale, copyAtStale, failureAtStale, lastAction

vars == <<generation, revision, pointerActive, hostGestureActive, inFlight,
  pending, flightGeneration, flightRevision, publicationCount, selectionVersion,
  copyCount, failureCount, staleCancellationCount, staleBarrier,
  selectionAtStale, copyAtStale, failureAtStale, lastAction>>

Init ==
  /\ generation = 1
  /\ revision = 1
  /\ pointerActive = FALSE
  /\ hostGestureActive = FALSE
  /\ inFlight = NoAction
  /\ pending = NoAction
  /\ flightGeneration = 0
  /\ flightRevision = 0
  /\ publicationCount = 0
  /\ selectionVersion = 0
  /\ copyCount = 0
  /\ failureCount = 0
  /\ staleCancellationCount = 0
  /\ staleBarrier = FALSE
  /\ selectionAtStale = 0
  /\ copyAtStale = 0
  /\ failureAtStale = 0
  /\ lastAction = "init"

PublishGesture(action) ==
  /\ action \in GestureActions
  /\ inFlight = NoAction
  /\ publicationCount < MaxPublications
  /\ inFlight' = action
  /\ pending' = NoAction
  /\ flightGeneration' = generation
  /\ flightRevision' = revision
  /\ publicationCount' = publicationCount + 1

StartGesture ==
  /\ ~staleBarrier
  /\ ~pointerActive
  /\ pending = NoAction
  /\ PublishGesture("start")
  /\ pointerActive' = TRUE
  /\ lastAction' = "start-gesture"
  /\ UNCHANGED <<generation, revision, hostGestureActive, selectionVersion,
       copyCount, failureCount, staleCancellationCount, staleBarrier,
       selectionAtStale, copyAtStale, failureAtStale>>

RestartGesture ==
  /\ staleBarrier
  /\ ~pointerActive
  /\ pending = NoAction
  /\ PublishGesture("start")
  /\ pointerActive' = TRUE
  /\ staleBarrier' = FALSE
  /\ lastAction' = "restart-gesture"
  /\ UNCHANGED <<generation, revision, hostGestureActive, selectionVersion,
       copyCount, failureCount, staleCancellationCount, selectionAtStale,
       copyAtStale, failureAtStale>>

MoveGesture ==
  /\ pointerActive
  /\ (inFlight # NoAction \/ publicationCount < MaxPublications)
  /\ IF inFlight = NoAction
       THEN /\ inFlight' = "update"
            /\ pending' = NoAction
            /\ flightGeneration' = generation
            /\ flightRevision' = revision
            /\ publicationCount' = publicationCount + 1
       ELSE /\ pending' = "update"
            /\ UNCHANGED <<inFlight, flightGeneration, flightRevision,
                 publicationCount>>
  /\ lastAction' = "move-gesture"
  /\ UNCHANGED <<generation, revision, pointerActive, hostGestureActive,
       selectionVersion, copyCount, failureCount, staleCancellationCount,
       staleBarrier, selectionAtStale, copyAtStale, failureAtStale>>

EndGesture ==
  /\ pointerActive
  /\ (inFlight # NoAction \/ publicationCount < MaxPublications)
  /\ IF inFlight = NoAction
       THEN /\ inFlight' = "end"
            /\ pending' = NoAction
            /\ flightGeneration' = generation
            /\ flightRevision' = revision
            /\ publicationCount' = publicationCount + 1
       ELSE /\ pending' = "end"
            /\ UNCHANGED <<inFlight, flightGeneration, flightRevision,
                 publicationCount>>
  /\ pointerActive' = FALSE
  /\ lastAction' = "end-gesture"
  /\ UNCHANGED <<generation, revision, hostGestureActive, selectionVersion,
       copyCount, failureCount, staleCancellationCount, staleBarrier,
       selectionAtStale, copyAtStale, failureAtStale>>

CancelGesture ==
  /\ pointerActive
  /\ (inFlight # NoAction \/ publicationCount < MaxPublications)
  /\ IF inFlight = NoAction
       THEN /\ inFlight' = "cancel"
            /\ pending' = NoAction
            /\ flightGeneration' = generation
            /\ flightRevision' = revision
            /\ publicationCount' = publicationCount + 1
       ELSE /\ pending' = "cancel"
            /\ UNCHANGED <<inFlight, flightGeneration, flightRevision,
                 publicationCount>>
  /\ pointerActive' = FALSE
  /\ lastAction' = "cancel-gesture"
  /\ UNCHANGED <<generation, revision, hostGestureActive, selectionVersion,
       copyCount, failureCount, staleCancellationCount, staleBarrier,
       selectionAtStale, copyAtStale, failureAtStale>>

DispatchPending ==
  /\ ~staleBarrier
  /\ inFlight = NoAction
  /\ pending \in EndpointActions
  /\ publicationCount < MaxPublications
  /\ inFlight' = pending
  /\ pending' = NoAction
  /\ flightGeneration' = generation
  /\ flightRevision' = revision
  /\ publicationCount' = publicationCount + 1
  /\ lastAction' = "dispatch-pending"
  /\ UNCHANGED <<generation, revision, pointerActive, hostGestureActive,
       selectionVersion, copyCount, failureCount, staleCancellationCount,
       staleBarrier, selectionAtStale, copyAtStale, failureAtStale>>

AcceptSelection ==
  /\ inFlight \in GestureActions
  /\ flightGeneration = generation
  /\ flightRevision = revision
  /\ (inFlight = "start" \/ hostGestureActive)
  /\ selectionVersion < MaxSelectionVersion
  /\ inFlight' = NoAction
  /\ flightGeneration' = 0
  /\ flightRevision' = 0
  /\ hostGestureActive' = (inFlight \in {"start", "update"})
  /\ selectionVersion' = selectionVersion + 1
  /\ copyCount' = copyCount + (IF inFlight = "end" THEN 1 ELSE 0)
  /\ lastAction' = "accept-selection"
  /\ UNCHANGED <<generation, revision, pointerActive, pending,
       publicationCount, failureCount, staleCancellationCount, staleBarrier,
       selectionAtStale, copyAtStale, failureAtStale>>

RejectStale(action) ==
  /\ staleCancellationCount < MaxStaleCancellations
  /\ pointerActive' = FALSE
  /\ inFlight' = NoAction
  /\ pending' = NoAction
  /\ flightGeneration' = 0
  /\ flightRevision' = 0
  /\ staleCancellationCount' = staleCancellationCount + 1
  /\ staleBarrier' = TRUE
  /\ selectionAtStale' = selectionVersion
  /\ copyAtStale' = copyCount
  /\ failureAtStale' = failureCount
  /\ lastAction' = action
  /\ UNCHANGED <<generation, revision, hostGestureActive, publicationCount,
       selectionVersion, copyCount, failureCount>>

RejectStaleRevision ==
  /\ inFlight \in GestureActions
  /\ flightGeneration = generation
  /\ flightRevision # revision
  /\ RejectStale("reject-stale-revision")

RejectStaleGeneration ==
  /\ inFlight \in GestureActions
  /\ flightGeneration # generation
  /\ RejectStale("reject-stale-generation")

RejectSelection ==
  /\ inFlight \in GestureActions
  /\ flightGeneration = generation
  /\ flightRevision = revision
  /\ failureCount < MaxFailures
  /\ inFlight' = NoAction
  /\ flightGeneration' = 0
  /\ flightRevision' = 0
  /\ failureCount' = failureCount + 1
  /\ lastAction' = "reject-selection"
  /\ UNCHANGED <<generation, revision, pointerActive, hostGestureActive,
       pending, publicationCount, selectionVersion, copyCount,
       staleCancellationCount, staleBarrier, selectionAtStale, copyAtStale,
       failureAtStale>>

AdvanceRevision ==
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ lastAction' = "advance-revision"
  /\ UNCHANGED <<generation, pointerActive, hostGestureActive, inFlight,
       pending, flightGeneration, flightRevision, publicationCount,
       selectionVersion, copyCount, failureCount, staleCancellationCount,
       staleBarrier, selectionAtStale, copyAtStale, failureAtStale>>

AdvanceGeneration ==
  /\ generation < MaxGeneration
  /\ generation' = generation + 1
  /\ revision' = 1
  /\ hostGestureActive' = FALSE
  /\ lastAction' = "advance-generation"
  /\ UNCHANGED <<pointerActive, inFlight, pending, flightGeneration,
       flightRevision, publicationCount, selectionVersion, copyCount,
       failureCount, staleCancellationCount, staleBarrier, selectionAtStale,
       copyAtStale, failureAtStale>>

Next ==
  \/ StartGesture
  \/ RestartGesture
  \/ MoveGesture
  \/ EndGesture
  \/ CancelGesture
  \/ DispatchPending
  \/ AcceptSelection
  \/ RejectStaleRevision
  \/ RejectStaleGeneration
  \/ RejectSelection
  \/ AdvanceRevision
  \/ AdvanceGeneration

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ generation \in 1..MaxGeneration
  /\ revision \in 1..MaxRevision
  /\ pointerActive \in BOOLEAN
  /\ hostGestureActive \in BOOLEAN
  /\ inFlight \in GestureActions \cup {NoAction}
  /\ pending \in EndpointActions \cup {NoAction}
  /\ flightGeneration \in 0..MaxGeneration
  /\ flightRevision \in 0..MaxRevision
  /\ publicationCount \in 0..MaxPublications
  /\ selectionVersion \in 0..MaxSelectionVersion
  /\ copyCount \in 0..MaxSelectionVersion
  /\ failureCount \in 0..MaxFailures
  /\ staleCancellationCount \in 0..MaxStaleCancellations
  /\ staleBarrier \in BOOLEAN
  /\ selectionAtStale \in 0..MaxSelectionVersion
  /\ copyAtStale \in 0..MaxSelectionVersion
  /\ failureAtStale \in 0..MaxFailures
  /\ lastAction \in ActionNames

FlightIdentityIsComplete ==
  (inFlight = NoAction) = (flightGeneration = 0 /\ flightRevision = 0)

HostGestureRequiresAcceptedSelection ==
  hostGestureActive => selectionVersion > 0

StaleBarrierClearsGestureWork ==
  staleBarrier =>
    /\ ~pointerActive
    /\ inFlight = NoAction
    /\ pending = NoAction

StaleBarrierPreservesSelection ==
  staleBarrier => selectionVersion = selectionAtStale

StaleBarrierDoesNotCopy ==
  staleBarrier => copyCount = copyAtStale

StaleBarrierDoesNotSurfaceFailure ==
  staleBarrier => failureCount = failureAtStale

StaleBarrierRecordsCancellation ==
  staleBarrier => staleCancellationCount > 0

=============================================================================
