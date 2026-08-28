---------------- MODULE PanelBroadcastInputCoordination ----------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

Targets == {"target-a", "target-b", "target-c"}
TargetSets == {targetSet \in SUBSET Targets: Cardinality(targetSet) >= 2}
Inputs == 1..3
MaxEpoch == 2
None == 0
InputsOrNone == 0..3
Phases == {"idle", "active", "stopped"}
Reasons == {"none", "partial-delivery", "target-invalidated"}
Outcomes ==
  {"none", "started", "enqueued", "dispatching", "success", "partial",
   "visibility", "ended", "invalidated", "dismissed", "stale"}

VARIABLES
  phase,
  epoch,
  targets,
  snapshots,
  queue,
  inFlight,
  nextInput,
  inputHidden,
  stoppedReason,
  deliveryLog,
  lastOutcome,
  lastSentTargets,
  previousPhase,
  previousEpoch,
  previousTargets,
  previousSnapshots,
  previousQueue,
  previousInFlight,
  previousNextInput,
  previousInputHidden,
  previousStoppedReason,
  previousDeliveryLog,
  previousLastSentTargets

vars ==
  <<phase, epoch, targets, snapshots, queue, inFlight, nextInput,
    inputHidden, stoppedReason, deliveryLog, lastOutcome, lastSentTargets,
    previousPhase, previousEpoch, previousTargets, previousSnapshots,
    previousQueue, previousInFlight, previousNextInput, previousInputHidden,
    previousStoppedReason, previousDeliveryLog, previousLastSentTargets>>

RecordPrevious ==
  /\ previousPhase' = phase
  /\ previousEpoch' = epoch
  /\ previousTargets' = targets
  /\ previousSnapshots' = snapshots
  /\ previousQueue' = queue
  /\ previousInFlight' = inFlight
  /\ previousNextInput' = nextInput
  /\ previousInputHidden' = inputHidden
  /\ previousStoppedReason' = stoppedReason
  /\ previousDeliveryLog' = deliveryLog
  /\ previousLastSentTargets' = lastSentTargets

Init ==
  /\ phase = "idle"
  /\ epoch = 0
  /\ targets = {}
  /\ snapshots = [candidate \in 0..MaxEpoch |-> {}]
  /\ queue = <<>>
  /\ inFlight = None
  /\ nextInput = 1
  /\ inputHidden = FALSE
  /\ stoppedReason = "none"
  /\ deliveryLog = [target \in Targets |-> <<>>]
  /\ lastOutcome = "none"
  /\ lastSentTargets = {}
  /\ previousPhase = phase
  /\ previousEpoch = epoch
  /\ previousTargets = targets
  /\ previousSnapshots = snapshots
  /\ previousQueue = queue
  /\ previousInFlight = inFlight
  /\ previousNextInput = nextInput
  /\ previousInputHidden = inputHidden
  /\ previousStoppedReason = stoppedReason
  /\ previousDeliveryLog = deliveryLog
  /\ previousLastSentTargets = lastSentTargets

Start(nextTargets) ==
  /\ phase = "idle"
  /\ epoch < MaxEpoch
  /\ nextTargets \in TargetSets
  /\ phase' = "active"
  /\ epoch' = epoch + 1
  /\ targets' = nextTargets
  /\ snapshots' = [snapshots EXCEPT ![epoch'] = nextTargets]
  /\ queue' = <<>>
  /\ inFlight' = None
  /\ nextInput' = 1
  /\ inputHidden' = FALSE
  /\ stoppedReason' = "none"
  /\ lastOutcome' = "started"
  /\ lastSentTargets' = {}
  /\ UNCHANGED deliveryLog
  /\ RecordPrevious

Enqueue ==
  /\ phase = "active"
  /\ nextInput \in Inputs
  /\ queue' = Append(queue, nextInput)
  /\ nextInput' = nextInput + 1
  /\ lastOutcome' = "enqueued"
  /\ lastSentTargets' = {}
  /\ UNCHANGED
       <<phase, epoch, targets, snapshots, inFlight, inputHidden,
         stoppedReason, deliveryLog>>
  /\ RecordPrevious

Dispatch ==
  /\ phase = "active"
  /\ inFlight = None
  /\ Len(queue) > 0
  /\ inFlight' = Head(queue)
  /\ queue' = Tail(queue)
  /\ lastOutcome' = "dispatching"
  /\ lastSentTargets' = {}
  /\ UNCHANGED
       <<phase, epoch, targets, snapshots, nextInput, inputHidden,
         stoppedReason, deliveryLog>>
  /\ RecordPrevious

AppendDelivery(log, sentTargets, deliveredEpoch, deliveredInput) ==
  [target \in Targets |->
    IF target \in sentTargets
    THEN Append(log[target], [epoch |-> deliveredEpoch, input |-> deliveredInput])
    ELSE log[target]]

CompleteSuccess(candidateEpoch, candidateInput) ==
  /\ phase = "active"
  /\ inFlight # None
  /\ candidateEpoch = epoch
  /\ candidateInput = inFlight
  /\ deliveryLog' =
       AppendDelivery(deliveryLog, targets, candidateEpoch, candidateInput)
  /\ inFlight' = None
  /\ lastOutcome' = "success"
  /\ lastSentTargets' = targets
  /\ UNCHANGED
       <<phase, epoch, targets, snapshots, queue, nextInput, inputHidden,
         stoppedReason>>
  /\ RecordPrevious

CompletePartial(candidateEpoch, candidateInput, sentTargets) ==
  /\ phase = "active"
  /\ inFlight # None
  /\ candidateEpoch = epoch
  /\ candidateInput = inFlight
  /\ sentTargets \subseteq targets
  /\ sentTargets # targets
  /\ deliveryLog' =
       AppendDelivery(deliveryLog, sentTargets, candidateEpoch, candidateInput)
  /\ phase' = "stopped"
  /\ queue' = <<>>
  /\ inFlight' = None
  /\ inputHidden' = FALSE
  /\ stoppedReason' = "partial-delivery"
  /\ lastOutcome' = "partial"
  /\ lastSentTargets' = sentTargets
  /\ UNCHANGED <<epoch, targets, snapshots, nextInput>>
  /\ RecordPrevious

ToggleHidden ==
  /\ phase = "active"
  /\ inputHidden' = ~inputHidden
  /\ lastOutcome' = "visibility"
  /\ lastSentTargets' = {}
  /\ UNCHANGED
       <<phase, epoch, targets, snapshots, queue, inFlight, nextInput,
         stoppedReason, deliveryLog>>
  /\ RecordPrevious

End ==
  /\ phase = "active"
  /\ phase' = "idle"
  /\ targets' = {}
  /\ queue' = <<>>
  /\ inFlight' = None
  /\ inputHidden' = FALSE
  /\ stoppedReason' = "none"
  /\ lastOutcome' = "ended"
  /\ lastSentTargets' = {}
  /\ UNCHANGED <<epoch, snapshots, nextInput, deliveryLog>>
  /\ RecordPrevious

Invalidate ==
  /\ phase = "active"
  /\ phase' = "stopped"
  /\ queue' = <<>>
  /\ inFlight' = None
  /\ inputHidden' = FALSE
  /\ stoppedReason' = "target-invalidated"
  /\ lastOutcome' = "invalidated"
  /\ lastSentTargets' = {}
  /\ UNCHANGED <<epoch, targets, snapshots, nextInput, deliveryLog>>
  /\ RecordPrevious

DismissStopped ==
  /\ phase = "stopped"
  /\ phase' = "idle"
  /\ targets' = {}
  /\ stoppedReason' = "none"
  /\ lastOutcome' = "dismissed"
  /\ lastSentTargets' = {}
  /\ UNCHANGED
       <<epoch, snapshots, queue, inFlight, nextInput, inputHidden,
         deliveryLog>>
  /\ RecordPrevious

RejectStaleCompletion(candidateEpoch, candidateInput) ==
  /\ candidateEpoch \in 0..MaxEpoch
  /\ candidateInput \in Inputs
  /\ ~(
       /\ phase = "active"
       /\ candidateEpoch = epoch
       /\ candidateInput = inFlight
     )
  /\ lastOutcome' = "stale"
  /\ UNCHANGED
       <<phase, epoch, targets, snapshots, queue, inFlight, nextInput,
         inputHidden, stoppedReason, deliveryLog, lastSentTargets>>
  /\ RecordPrevious

SettleCurrent ==
  \/ CompleteSuccess(epoch, inFlight)
  \/ \E sentTargets \in SUBSET targets:
       CompletePartial(epoch, inFlight, sentTargets)

Next ==
  \/ \E nextTargets \in TargetSets: Start(nextTargets)
  \/ Enqueue
  \/ Dispatch
  \/ \E candidateEpoch \in 0..MaxEpoch, candidateInput \in Inputs:
       CompleteSuccess(candidateEpoch, candidateInput)
  \/ \E candidateEpoch \in 0..MaxEpoch,
       candidateInput \in Inputs,
       sentTargets \in SUBSET Targets:
       CompletePartial(candidateEpoch, candidateInput, sentTargets)
  \/ ToggleHidden
  \/ End
  \/ Invalidate
  \/ DismissStopped
  \/ \E candidateEpoch \in 0..MaxEpoch, candidateInput \in Inputs:
       RejectStaleCompletion(candidateEpoch, candidateInput)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Dispatch)
  /\ WF_vars(SettleCurrent)

TypeOK ==
  /\ phase \in Phases
  /\ epoch \in 0..MaxEpoch
  /\ targets \subseteq Targets
  /\ snapshots \in [0..MaxEpoch -> SUBSET Targets]
  /\ queue \in Seq(Inputs)
  /\ inFlight \in InputsOrNone
  /\ nextInput \in 1..4
  /\ inputHidden \in BOOLEAN
  /\ stoppedReason \in Reasons
  /\ deliveryLog \in [Targets -> Seq([epoch: 1..MaxEpoch, input: Inputs])]
  /\ lastOutcome \in Outcomes
  /\ lastSentTargets \subseteq Targets
  /\ previousPhase \in Phases
  /\ previousEpoch \in 0..MaxEpoch
  /\ previousTargets \subseteq Targets
  /\ previousSnapshots \in [0..MaxEpoch -> SUBSET Targets]
  /\ previousQueue \in Seq(Inputs)
  /\ previousInFlight \in InputsOrNone
  /\ previousNextInput \in 1..4
  /\ previousInputHidden \in BOOLEAN
  /\ previousStoppedReason \in Reasons
  /\ previousDeliveryLog \in [Targets -> Seq([epoch: 1..MaxEpoch, input: Inputs])]
  /\ previousLastSentTargets \subseteq Targets

ActiveSnapshotIsExact ==
  phase = "active" =>
    /\ targets \in TargetSets
    /\ snapshots[epoch] = targets

InactiveHasNoPendingInput ==
  phase # "active" =>
    /\ queue = <<>>
    /\ inFlight = None
    /\ ~inputHidden

StoppedReasonIsExact ==
  (phase = "stopped") <=> (stoppedReason # "none")

SingleInFlightFIFO ==
  /\ (inFlight # None => phase = "active")
  /\ \A first, second \in 1..Len(queue):
       first < second => queue[first] < queue[second]
  /\ inFlight # None =>
       \A index \in 1..Len(queue): inFlight < queue[index]

DeliveryUsesCapturedTargets ==
  \A target \in Targets:
    \A index \in 1..Len(deliveryLog[target]):
      target \in snapshots[deliveryLog[target][index].epoch]

DeliveryOrderIsFIFO ==
  \A target \in Targets:
    \A first, second \in 1..Len(deliveryLog[target]):
      first < second =>
        \/ deliveryLog[target][first].epoch < deliveryLog[target][second].epoch
        \/ /\ deliveryLog[target][first].epoch = deliveryLog[target][second].epoch
           /\ deliveryLog[target][first].input < deliveryLog[target][second].input

SuccessRequiresEveryTarget ==
  lastOutcome = "success" => lastSentTargets = targets

PartialDeliveryStops ==
  lastOutcome = "partial" =>
    /\ phase = "stopped"
    /\ stoppedReason = "partial-delivery"
    /\ lastSentTargets \subseteq targets
    /\ lastSentTargets # targets

StaleCompletionIsInert ==
  lastOutcome = "stale" =>
    /\ phase = previousPhase
    /\ epoch = previousEpoch
    /\ targets = previousTargets
    /\ snapshots = previousSnapshots
    /\ queue = previousQueue
    /\ inFlight = previousInFlight
    /\ nextInput = previousNextInput
    /\ inputHidden = previousInputHidden
    /\ stoppedReason = previousStoppedReason
    /\ deliveryLog = previousDeliveryLog
    /\ lastSentTargets = previousLastSentTargets

CurrentInputEventuallySettles ==
  \A candidateEpoch \in 1..MaxEpoch, candidateInput \in Inputs:
    (phase = "active" /\ epoch = candidateEpoch /\ inFlight = candidateInput)
      ~>
    (phase # "active" \/ epoch # candidateEpoch \/ inFlight # candidateInput)

QueuedInputEventuallyDispatchesOrStops ==
  \A candidateEpoch \in 1..MaxEpoch, candidateInput \in Inputs:
    (phase = "active" /\ epoch = candidateEpoch /\
      Len(queue) > 0 /\ Head(queue) = candidateInput /\ inFlight = None)
      ~>
    (phase # "active" \/ epoch # candidateEpoch \/ inFlight = candidateInput)

=============================================================================
