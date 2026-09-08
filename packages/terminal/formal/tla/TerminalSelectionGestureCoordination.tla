------------------ MODULE TerminalSelectionGestureCoordination ------------------
EXTENDS Naturals, Sequences, TLC

CONSTANTS MaxGeneration, MaxRevision, MaxPublications, MaxSelectionVersion,
  MaxFailures, MaxGenerationCancellations

\* Models the client-owned serialized pointer-selection lane. One pointer
\* gesture captures one projection generation and carries viewport-relative
\* endpoints through a bounded FIFO exploration queue. Fresh queued starts and
\* browser callback fairness remain outside this gesture-scoped model.
\* Revision advancement represents arbitrarily frequent terminal
\* output: it never changes the gesture identity and never disables settlement.
\* A generation change invalidates the captured viewport and silently retires
\* the complete local gesture. Programmatic absolute selection is outside this
\* model and remains exact-revision work.

NoAction == "none"
GestureActions == {"start", "update", "end", "cancel"}
EndpointActions == {"update", "end", "cancel"}
PendingCommands == [action: EndpointActions, generation: 1..MaxGeneration]
ActionNames == {
  "init", "start-gesture", "restart-gesture", "move-gesture", "end-gesture",
  "cancel-gesture", "dispatch-pending", "accept-selection",
  "reject-stale-generation", "reject-selection", "advance-revision",
  "advance-generation"
}

VARIABLES generation, revision, pointerActive, hostGestureActive,
  gestureGeneration, inFlight, pendingQueue, flightGeneration, publicationCount,
  selectionVersion, copyCount, failureCount, generationCancellationCount,
  generationBarrier, selectionAtGenerationBarrier, copyAtGenerationBarrier,
  failureAtGenerationBarrier, lastAction, priorPendingQueue

vars == <<generation, revision, pointerActive, hostGestureActive,
  gestureGeneration, inFlight, pendingQueue, flightGeneration, publicationCount,
  selectionVersion, copyCount, failureCount, generationCancellationCount,
  generationBarrier, selectionAtGenerationBarrier, copyAtGenerationBarrier,
  failureAtGenerationBarrier, lastAction, priorPendingQueue>>

Init ==
  /\ generation = 1
  /\ revision = 1
  /\ pointerActive = FALSE
  /\ hostGestureActive = FALSE
  /\ gestureGeneration = 0
  /\ inFlight = NoAction
  /\ pendingQueue = <<>>
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
  /\ priorPendingQueue = <<>>

Record(action) ==
  /\ lastAction' = action
  /\ priorPendingQueue' = pendingQueue

PendingCommand(action) ==
  [action |-> action, generation |-> gestureGeneration]

ReplacesTrailingUpdate(action) ==
  /\ Len(pendingQueue) > 0
  /\ pendingQueue[Len(pendingQueue)].action = "update"
  /\ action \in {"update", "cancel"}

DropLast(queue) ==
  IF Len(queue) = 1 THEN <<>> ELSE SubSeq(queue, 1, Len(queue) - 1)

QueueAfter(action) ==
  IF ReplacesTrailingUpdate(action)
  THEN Append(DropLast(pendingQueue), PendingCommand(action))
  ELSE Append(pendingQueue, PendingCommand(action))

CanQueue(action) ==
  /\ action \in EndpointActions
  /\ IF ReplacesTrailingUpdate(action)
       THEN TRUE
       ELSE publicationCount + Len(pendingQueue) < MaxPublications

PublishGesture(action, commandGeneration) ==
  /\ action \in GestureActions
  /\ commandGeneration \in 1..MaxGeneration
  /\ inFlight = NoAction
  /\ Len(pendingQueue) = 0
  /\ publicationCount < MaxPublications
  /\ inFlight' = action
  /\ pendingQueue' = <<>>
  /\ flightGeneration' = commandGeneration
  /\ publicationCount' = publicationCount + 1

StartGesture ==
  /\ ~generationBarrier
  /\ ~pointerActive
  /\ Len(pendingQueue) = 0
  /\ PublishGesture("start", generation)
  /\ pointerActive' = TRUE
  /\ gestureGeneration' = generation
  /\ Record("start-gesture")
  /\ UNCHANGED <<generation, revision, hostGestureActive, selectionVersion,
       copyCount, failureCount, generationCancellationCount,
       generationBarrier, selectionAtGenerationBarrier,
       copyAtGenerationBarrier, failureAtGenerationBarrier>>

RestartGesture ==
  /\ generationBarrier
  /\ ~pointerActive
  /\ Len(pendingQueue) = 0
  /\ PublishGesture("start", generation)
  /\ pointerActive' = TRUE
  /\ gestureGeneration' = generation
  /\ generationBarrier' = FALSE
  /\ Record("restart-gesture")
  /\ UNCHANGED <<generation, revision, hostGestureActive, selectionVersion,
       copyCount, failureCount, generationCancellationCount,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

MoveGesture ==
  /\ pointerActive
  /\ gestureGeneration \in 1..MaxGeneration
  /\ IF inFlight = NoAction /\ Len(pendingQueue) = 0
       THEN /\ publicationCount < MaxPublications
            /\ inFlight' = "update"
            /\ pendingQueue' = <<>>
            /\ flightGeneration' = gestureGeneration
            /\ publicationCount' = publicationCount + 1
       ELSE /\ CanQueue("update")
            /\ pendingQueue' = QueueAfter("update")
            /\ UNCHANGED <<inFlight, flightGeneration, publicationCount>>
  /\ Record("move-gesture")
  /\ UNCHANGED <<generation, revision, pointerActive, hostGestureActive,
       gestureGeneration, selectionVersion, copyCount, failureCount,
       generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

EndGesture ==
  /\ pointerActive
  /\ gestureGeneration \in 1..MaxGeneration
  /\ IF inFlight = NoAction /\ Len(pendingQueue) = 0
       THEN /\ publicationCount < MaxPublications
            /\ inFlight' = "end"
            /\ pendingQueue' = <<>>
            /\ flightGeneration' = gestureGeneration
            /\ publicationCount' = publicationCount + 1
       ELSE /\ CanQueue("end")
            /\ pendingQueue' = QueueAfter("end")
            /\ UNCHANGED <<inFlight, flightGeneration, publicationCount>>
  /\ pointerActive' = FALSE
  /\ Record("end-gesture")
  /\ UNCHANGED <<generation, revision, hostGestureActive,
       gestureGeneration, selectionVersion, copyCount, failureCount,
       generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

CancelGesture ==
  /\ pointerActive
  /\ gestureGeneration \in 1..MaxGeneration
  /\ IF inFlight = NoAction /\ Len(pendingQueue) = 0
       THEN /\ publicationCount < MaxPublications
            /\ inFlight' = "cancel"
            /\ pendingQueue' = <<>>
            /\ flightGeneration' = gestureGeneration
            /\ publicationCount' = publicationCount + 1
       ELSE /\ CanQueue("cancel")
            /\ pendingQueue' = QueueAfter("cancel")
            /\ UNCHANGED <<inFlight, flightGeneration, publicationCount>>
  /\ pointerActive' = FALSE
  /\ Record("cancel-gesture")
  /\ UNCHANGED <<generation, revision, hostGestureActive,
       gestureGeneration, selectionVersion, copyCount, failureCount,
       generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

DispatchPending ==
  LET command == Head(pendingQueue)
  IN
    /\ ~generationBarrier
    /\ inFlight = NoAction
    /\ Len(pendingQueue) > 0
    /\ publicationCount < MaxPublications
    /\ inFlight' = command.action
    /\ pendingQueue' = Tail(pendingQueue)
    /\ flightGeneration' = command.generation
    /\ publicationCount' = publicationCount + 1
    /\ Record("dispatch-pending")
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
       IF inFlight \in {"end", "cancel"} /\ Len(pendingQueue) = 0
       THEN 0
       ELSE gestureGeneration
  /\ selectionVersion' = selectionVersion + 1
  /\ copyCount' = copyCount + (IF inFlight = "end" THEN 1 ELSE 0)
  /\ Record("accept-selection")
  /\ UNCHANGED <<generation, revision, pointerActive, pendingQueue,
       publicationCount, failureCount, generationCancellationCount,
       generationBarrier, selectionAtGenerationBarrier,
       copyAtGenerationBarrier, failureAtGenerationBarrier>>

RejectStaleGeneration ==
  /\ inFlight \in GestureActions
  /\ flightGeneration # generation
  /\ pointerActive' = FALSE
  /\ gestureGeneration' = 0
  /\ inFlight' = NoAction
  /\ pendingQueue' = <<>>
  /\ flightGeneration' = 0
  /\ generationCancellationCount' =
       IF generationCancellationCount < MaxGenerationCancellations
       THEN generationCancellationCount + 1
       ELSE generationCancellationCount
  /\ generationBarrier' = TRUE
  /\ selectionAtGenerationBarrier' = selectionVersion
  /\ copyAtGenerationBarrier' = copyCount
  /\ failureAtGenerationBarrier' = failureCount
  /\ Record("reject-stale-generation")
  /\ UNCHANGED <<generation, revision, hostGestureActive, publicationCount,
       selectionVersion, copyCount, failureCount>>

RejectSelection ==
  /\ inFlight \in GestureActions
  /\ flightGeneration = generation
  /\ inFlight' = NoAction
  /\ flightGeneration' = 0
  /\ failureCount' =
       IF failureCount < MaxFailures THEN failureCount + 1 ELSE failureCount
  /\ Record("reject-selection")
  /\ UNCHANGED <<generation, revision, pointerActive, hostGestureActive,
       gestureGeneration, pendingQueue, publicationCount, selectionVersion,
       copyCount, generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

AdvanceRevision ==
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ Record("advance-revision")
  /\ UNCHANGED <<generation, pointerActive, hostGestureActive,
       gestureGeneration, inFlight, pendingQueue, flightGeneration,
       publicationCount, selectionVersion, copyCount, failureCount,
       generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

AdvanceGeneration ==
  /\ generation < MaxGeneration
  /\ generation' = generation + 1
  /\ revision' = 1
  /\ hostGestureActive' = FALSE
  /\ Record("advance-generation")
  /\ UNCHANGED <<pointerActive, gestureGeneration, inFlight, pendingQueue,
       flightGeneration, publicationCount, selectionVersion, copyCount,
       failureCount, generationCancellationCount, generationBarrier,
       selectionAtGenerationBarrier, copyAtGenerationBarrier,
       failureAtGenerationBarrier>>

SettleFlight ==
  \/ AcceptSelection
  \/ RejectStaleGeneration
  \/ RejectSelection

CoverageRapidReleaseQueue ==
  /\ Len(pendingQueue) = 2
  /\ pendingQueue[1].action = "update"
  /\ pendingQueue[2].action = "end"
  /\ pendingQueue[1].generation = pendingQueue[2].generation
  /\ UNCHANGED vars

CoverageDispatchUpdateAheadOfRelease ==
  /\ lastAction = "dispatch-pending"
  /\ Len(priorPendingQueue) = 2
  /\ priorPendingQueue[1].action = "update"
  /\ priorPendingQueue[2].action = "end"
  /\ inFlight = "update"
  /\ Len(pendingQueue) = 1
  /\ pendingQueue[1].action = "end"
  /\ UNCHANGED vars

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
  \/ CoverageRapidReleaseQueue
  \/ CoverageDispatchUpdateAheadOfRelease

Spec == Init /\ [][Next]_vars /\ WF_vars(SettleFlight)

TypeOK ==
  /\ generation \in 1..MaxGeneration
  /\ revision \in 1..MaxRevision
  /\ pointerActive \in BOOLEAN
  /\ hostGestureActive \in BOOLEAN
  /\ gestureGeneration \in 0..MaxGeneration
  /\ inFlight \in GestureActions \cup {NoAction}
  /\ pendingQueue \in Seq(PendingCommands)
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
  /\ priorPendingQueue \in Seq(PendingCommands)

FlightIdentityIsComplete ==
  (inFlight = NoAction) = (flightGeneration = 0)

GestureWorkKeepsCapturedGeneration ==
  /\ (pointerActive \/ inFlight # NoAction \/ Len(pendingQueue) > 0) =>
       gestureGeneration \in 1..MaxGeneration
  /\ inFlight # NoAction => flightGeneration = gestureGeneration
  /\ \A index \in 1..Len(pendingQueue):
       pendingQueue[index].generation = gestureGeneration

PendingQueueFitsPublicationBudget ==
  publicationCount + Len(pendingQueue) <= MaxPublications

DispatchIsFIFO ==
  lastAction = "dispatch-pending" =>
    LET command == Head(priorPendingQueue)
    IN
      /\ Len(priorPendingQueue) > 0
      /\ inFlight = command.action
      /\ flightGeneration = command.generation
      /\ pendingQueue = Tail(priorPendingQueue)

ReleaseRetainsQueuedUpdate ==
  /\ lastAction = "end-gesture"
  /\ Len(priorPendingQueue) > 0
  /\ priorPendingQueue[Len(priorPendingQueue)].action = "update"
  => pendingQueue = Append(priorPendingQueue,
       [action |-> "end", generation |-> gestureGeneration])

HostGestureRequiresAcceptedSelection ==
  hostGestureActive => selectionVersion > 0

GenerationBarrierClearsGestureWork ==
  generationBarrier =>
    /\ ~pointerActive
    /\ gestureGeneration = 0
    /\ inFlight = NoAction
    /\ Len(pendingQueue) = 0

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
