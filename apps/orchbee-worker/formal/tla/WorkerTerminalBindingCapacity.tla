------------------- MODULE WorkerTerminalBindingCapacity -------------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded refinement of the generation-local Worker terminal-binding map
\* composed with the Worker's one-operation/result global slot. The production
\* binding limit is 1,024; TLC uses a smaller Capacity with one extra TerminalId
\* to exhaustively cover the threshold. The minimal "operation-pending" phase
\* collapses the existing handoff phases; Host handoff and time monotonicity
\* remain in TerminalLaunchAuthorization.

CONSTANTS TerminalIds, Capacity, MaxFence

NoTerminal == "none"
NoFence == 0
NoResult == "none"
CachedResult == "cached-result"

SlotPhases == {"empty", "operation-pending", "result-ready"}
SlotKinds == {"none", "binding-result", "uncached-rejection"}

Decisions == {
  "none", "dispatch", "replay", "wrong-worker", "stale-generation",
  "unsupported-capability", "lease-expired", "stale-fence",
  "conflicting-replay", "capacity-exceeded", "host-identity-changed",
  "slot-occupied", "result-cached", "delivered"
}

Actions == {
  "init", "admit-new-binding", "cache-binding-result", "deliver-result",
  "reject-slot-occupied", "reject-wrong-worker-at-capacity",
  "reject-stale-generation-at-capacity",
  "reject-unsupported-capability-at-capacity",
  "reject-expired-lease-at-capacity", "reject-new-terminal-at-capacity",
  "reject-capacity-before-start-identity", "replay-exact-at-capacity",
  "replace-higher-fence-at-capacity", "reject-lower-fence-at-capacity",
  "reject-conflicting-fence-at-capacity"
}

SlotAdmissionActions == {
  "admit-new-binding", "reject-wrong-worker-at-capacity",
  "reject-stale-generation-at-capacity",
  "reject-unsupported-capability-at-capacity",
  "reject-expired-lease-at-capacity", "reject-new-terminal-at-capacity",
  "reject-capacity-before-start-identity", "replay-exact-at-capacity",
  "replace-higher-fence-at-capacity", "reject-lower-fence-at-capacity",
  "reject-conflicting-fence-at-capacity"
}

ASSUME
  /\ IsFiniteSet(TerminalIds)
  /\ Capacity \in Nat \ {0}
  /\ Cardinality(TerminalIds) = Capacity + 1
  /\ NoTerminal \notin TerminalIds
  /\ MaxFence \in Nat
  /\ MaxFence >= 2

VARIABLES
  bindingFences,
  cachedResults,
  everBound,
  slotPhase,
  slotTerminal,
  slotFence,
  slotKind,
  priorBindingFences,
  priorCachedResults,
  priorEverBound,
  priorSlotPhase,
  priorSlotTerminal,
  priorSlotFence,
  priorSlotKind,
  lastTerminal,
  lastDecision,
  lastAction

vars == <<
  bindingFences, cachedResults, everBound,
  slotPhase, slotTerminal, slotFence, slotKind,
  priorBindingFences, priorCachedResults, priorEverBound,
  priorSlotPhase, priorSlotTerminal, priorSlotFence, priorSlotKind,
  lastTerminal, lastDecision, lastAction
>>

BoundTerminals(fences) ==
  {terminal \in TerminalIds : fences[terminal] # NoFence}

BindingCount == Cardinality(BoundTerminals(bindingFences))

UncachedBindings ==
  {terminal \in BoundTerminals(bindingFences) :
    cachedResults[terminal] = NoResult}

OutstandingSlot ==
  IF slotPhase = "empty"
  THEN {}
  ELSE {<<slotTerminal, slotFence, slotKind>>}

OutstandingUncachedEffectOrResult ==
  IF slotPhase = "operation-pending"
  THEN {<<slotTerminal, slotFence, "effect">>}
  ELSE IF slotPhase = "result-ready" /\ slotKind = "uncached-rejection"
  THEN {<<slotTerminal, slotFence, "result">>}
  ELSE {}

Classify(terminal, requestFence, exactBinding, workerMatches,
    generationMatches, capabilityMatches, leaseIsLive, startIdentityMatches) ==
  IF slotPhase # "empty"
  THEN "slot-occupied"
  ELSE IF ~workerMatches
  THEN "wrong-worker"
  ELSE IF ~generationMatches
  THEN "stale-generation"
  ELSE IF
    terminal \in BoundTerminals(bindingFences)
      /\ requestFence = bindingFences[terminal]
      /\ exactBinding
      /\ cachedResults[terminal] = CachedResult
  THEN "replay"
  ELSE IF ~capabilityMatches
  THEN "unsupported-capability"
  ELSE IF ~leaseIsLive
  THEN "lease-expired"
  ELSE IF
    terminal \in BoundTerminals(bindingFences)
      /\ requestFence < bindingFences[terminal]
  THEN "stale-fence"
  ELSE IF
    terminal \in BoundTerminals(bindingFences)
      /\ requestFence = bindingFences[terminal]
  THEN "conflicting-replay"
  ELSE IF terminal \notin BoundTerminals(bindingFences) /\ BindingCount = Capacity
  THEN "capacity-exceeded"
  ELSE IF ~startIdentityMatches
  THEN "host-identity-changed"
  ELSE "dispatch"

Record(action, terminal, decision) ==
  /\ priorBindingFences' = bindingFences
  /\ priorCachedResults' = cachedResults
  /\ priorEverBound' = everBound
  /\ priorSlotPhase' = slotPhase
  /\ priorSlotTerminal' = slotTerminal
  /\ priorSlotFence' = slotFence
  /\ priorSlotKind' = slotKind
  /\ lastTerminal' = terminal
  /\ lastDecision' = decision
  /\ lastAction' = action

Init ==
  /\ bindingFences = [terminal \in TerminalIds |-> NoFence]
  /\ cachedResults = [terminal \in TerminalIds |-> NoResult]
  /\ everBound = {}
  /\ slotPhase = "empty"
  /\ slotTerminal = NoTerminal
  /\ slotFence = NoFence
  /\ slotKind = "none"
  /\ priorBindingFences = bindingFences
  /\ priorCachedResults = cachedResults
  /\ priorEverBound = everBound
  /\ priorSlotPhase = slotPhase
  /\ priorSlotTerminal = slotTerminal
  /\ priorSlotFence = slotFence
  /\ priorSlotKind = slotKind
  /\ lastTerminal = NoTerminal
  /\ lastDecision = "none"
  /\ lastAction = "init"

AdmitNewBinding ==
  \E terminal \in TerminalIds:
    /\ bindingFences[terminal] = NoFence
    /\ BindingCount < Capacity
    /\ Classify(terminal, 1, FALSE, TRUE, TRUE, TRUE, TRUE, TRUE) = "dispatch"
    /\ bindingFences' = [bindingFences EXCEPT ![terminal] = 1]
    /\ cachedResults' = cachedResults
    /\ everBound' = everBound \cup {terminal}
    /\ slotPhase' = "operation-pending"
    /\ slotTerminal' = terminal
    /\ slotFence' = 1
    /\ slotKind' = "binding-result"
    /\ Record("admit-new-binding", terminal, "dispatch")

CacheBindingResult ==
  /\ slotPhase = "operation-pending"
  /\ slotKind = "binding-result"
  /\ bindingFences[slotTerminal] = slotFence
  /\ cachedResults[slotTerminal] = NoResult
  /\ cachedResults' =
       [cachedResults EXCEPT ![slotTerminal] = CachedResult]
  /\ slotPhase' = "result-ready"
  /\ UNCHANGED <<
       bindingFences, everBound, slotTerminal, slotFence, slotKind
     >>
  /\ Record("cache-binding-result", slotTerminal, "result-cached")

DeliverResult ==
  /\ slotPhase = "result-ready"
  /\ slotPhase' = "empty"
  /\ slotTerminal' = NoTerminal
  /\ slotFence' = NoFence
  /\ slotKind' = "none"
  /\ UNCHANGED <<bindingFences, cachedResults, everBound>>
  /\ Record("deliver-result", slotTerminal, "delivered")

QueueUncachedRejection(action, terminal, requestFence, decision) ==
  /\ slotPhase = "empty"
  /\ slotPhase' = "result-ready"
  /\ slotTerminal' = terminal
  /\ slotFence' = requestFence
  /\ slotKind' = "uncached-rejection"
  /\ UNCHANGED <<bindingFences, cachedResults, everBound>>
  /\ Record(action, terminal, decision)

RejectSlotOccupied ==
  \E terminal \in TerminalIds:
    /\ Classify(terminal, 1, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
         = "slot-occupied"
    /\ UNCHANGED <<
         bindingFences, cachedResults, everBound,
         slotPhase, slotTerminal, slotFence, slotKind
       >>
    /\ Record("reject-slot-occupied", terminal, "slot-occupied")

UnseenAtCapacity(terminal) ==
  /\ terminal \in TerminalIds
  /\ bindingFences[terminal] = NoFence
  /\ BindingCount = Capacity

RejectWrongWorkerAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
         = "wrong-worker"
    /\ QueueUncachedRejection(
         "reject-wrong-worker-at-capacity", terminal, 1, "wrong-worker")

RejectStaleGenerationAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE)
         = "stale-generation"
    /\ QueueUncachedRejection(
         "reject-stale-generation-at-capacity", terminal, 1,
         "stale-generation")

RejectUnsupportedCapabilityAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE)
         = "unsupported-capability"
    /\ QueueUncachedRejection(
         "reject-unsupported-capability-at-capacity", terminal, 1,
         "unsupported-capability")

RejectExpiredLeaseAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, TRUE, TRUE, FALSE, FALSE)
         = "lease-expired"
    /\ QueueUncachedRejection(
         "reject-expired-lease-at-capacity", terminal, 1, "lease-expired")

RejectNewTerminalAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, TRUE, TRUE, TRUE, TRUE)
         = "capacity-exceeded"
    /\ QueueUncachedRejection(
         "reject-new-terminal-at-capacity", terminal, 1,
         "capacity-exceeded")

RejectCapacityBeforeStartIdentity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, TRUE, TRUE, TRUE, FALSE)
         = "capacity-exceeded"
    /\ QueueUncachedRejection(
         "reject-capacity-before-start-identity", terminal, 1,
         "capacity-exceeded")

ReplayExactAtCapacity ==
  \E terminal \in BoundTerminals(bindingFences):
    /\ BindingCount = Capacity
    /\ cachedResults[terminal] = CachedResult
    /\ Classify(
         terminal,
         bindingFences[terminal],
         TRUE,
         TRUE,
         TRUE,
         TRUE,
         FALSE,
         FALSE
       ) = "replay"
    /\ slotPhase' = "result-ready"
    /\ slotTerminal' = terminal
    /\ slotFence' = bindingFences[terminal]
    /\ slotKind' = "binding-result"
    /\ UNCHANGED <<bindingFences, cachedResults, everBound>>
    /\ Record("replay-exact-at-capacity", terminal, "replay")

ReplaceHigherFenceAtCapacity ==
  \E terminal \in BoundTerminals(bindingFences):
    /\ BindingCount = Capacity
    /\ bindingFences[terminal] < MaxFence
    /\ Classify(
         terminal,
         bindingFences[terminal] + 1,
         FALSE,
         TRUE,
         TRUE,
         TRUE,
         TRUE,
         TRUE
       ) = "dispatch"
    /\ bindingFences' =
         [bindingFences EXCEPT ![terminal] = @ + 1]
    /\ cachedResults' =
         [cachedResults EXCEPT ![terminal] = NoResult]
    /\ everBound' = everBound
    /\ slotPhase' = "operation-pending"
    /\ slotTerminal' = terminal
    /\ slotFence' = bindingFences[terminal] + 1
    /\ slotKind' = "binding-result"
    /\ Record("replace-higher-fence-at-capacity", terminal, "dispatch")

RejectLowerFenceAtCapacity ==
  \E terminal \in BoundTerminals(bindingFences):
    /\ BindingCount = Capacity
    /\ bindingFences[terminal] > 1
    /\ Classify(
         terminal,
         bindingFences[terminal] - 1,
         FALSE,
         TRUE,
         TRUE,
         TRUE,
         TRUE,
         TRUE
       ) = "stale-fence"
    /\ QueueUncachedRejection(
         "reject-lower-fence-at-capacity", terminal,
         bindingFences[terminal] - 1, "stale-fence")

RejectConflictingFenceAtCapacity ==
  \E terminal \in BoundTerminals(bindingFences):
    /\ BindingCount = Capacity
    /\ Classify(
         terminal,
         bindingFences[terminal],
         FALSE,
         TRUE,
         TRUE,
         TRUE,
         TRUE,
         TRUE
       ) = "conflicting-replay"
    /\ QueueUncachedRejection(
         "reject-conflicting-fence-at-capacity", terminal,
         bindingFences[terminal], "conflicting-replay")

Next ==
  \/ AdmitNewBinding
  \/ CacheBindingResult
  \/ DeliverResult
  \/ RejectSlotOccupied
  \/ RejectWrongWorkerAtCapacity
  \/ RejectStaleGenerationAtCapacity
  \/ RejectUnsupportedCapabilityAtCapacity
  \/ RejectExpiredLeaseAtCapacity
  \/ RejectNewTerminalAtCapacity
  \/ RejectCapacityBeforeStartIdentity
  \/ ReplayExactAtCapacity
  \/ ReplaceHigherFenceAtCapacity
  \/ RejectLowerFenceAtCapacity
  \/ RejectConflictingFenceAtCapacity

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ bindingFences \in [TerminalIds -> 0..MaxFence]
  /\ cachedResults \in [TerminalIds -> {NoResult, CachedResult}]
  /\ everBound \subseteq TerminalIds
  /\ slotPhase \in SlotPhases
  /\ slotTerminal \in TerminalIds \cup {NoTerminal}
  /\ slotFence \in 0..MaxFence
  /\ slotKind \in SlotKinds
  /\ priorBindingFences \in [TerminalIds -> 0..MaxFence]
  /\ priorCachedResults \in [TerminalIds -> {NoResult, CachedResult}]
  /\ priorEverBound \subseteq TerminalIds
  /\ priorSlotPhase \in SlotPhases
  /\ priorSlotTerminal \in TerminalIds \cup {NoTerminal}
  /\ priorSlotFence \in 0..MaxFence
  /\ priorSlotKind \in SlotKinds
  /\ lastTerminal \in TerminalIds \cup {NoTerminal}
  /\ lastDecision \in Decisions
  /\ lastAction \in Actions

CapacityIsBounded == BindingCount <= Capacity

BindingAndCacheAreConsistent ==
  /\ everBound = BoundTerminals(bindingFences)
  /\ \A terminal \in TerminalIds:
       bindingFences[terminal] = NoFence => cachedResults[terminal] = NoResult

RetainedBindingsNeverDisappear ==
  BoundTerminals(priorBindingFences) \subseteq BoundTerminals(bindingFences)

GlobalSlotIsConsistent ==
  CASE slotPhase = "empty" ->
    /\ slotTerminal = NoTerminal
    /\ slotFence = NoFence
    /\ slotKind = "none"
  [] slotPhase = "operation-pending" ->
    /\ slotTerminal \in BoundTerminals(bindingFences)
    /\ slotFence = bindingFences[slotTerminal]
    /\ cachedResults[slotTerminal] = NoResult
    /\ slotKind = "binding-result"
  [] slotPhase = "result-ready" ->
    /\ slotTerminal \in TerminalIds
    /\ slotFence \in 1..MaxFence
    /\ IF slotKind = "binding-result"
       THEN
         /\ slotTerminal \in BoundTerminals(bindingFences)
         /\ slotFence = bindingFences[slotTerminal]
         /\ cachedResults[slotTerminal] = CachedResult
       ELSE slotKind = "uncached-rejection"

AtMostOneOutstandingUncachedEffectOrResult ==
  /\ Cardinality(OutstandingSlot) <= 1
  /\ Cardinality(OutstandingUncachedEffectOrResult) <= 1
  /\ Cardinality(UncachedBindings) <= 1
  /\ slotPhase = "operation-pending" =>
       UncachedBindings = {slotTerminal}
  /\ slotPhase # "operation-pending" => UncachedBindings = {}

RejectedRequestPreservesBindings ==
  lastDecision \in {
    "wrong-worker", "stale-generation", "unsupported-capability",
    "lease-expired", "stale-fence", "conflicting-replay",
    "capacity-exceeded", "host-identity-changed", "slot-occupied"
  } =>
    /\ bindingFences = priorBindingFences
    /\ cachedResults = priorCachedResults
    /\ everBound = priorEverBound

SlotOccupiedPreservesOwnedWork ==
  lastAction = "reject-slot-occupied" =>
    /\ priorSlotPhase # "empty"
    /\ slotPhase = priorSlotPhase
    /\ slotTerminal = priorSlotTerminal
    /\ slotFence = priorSlotFence
    /\ slotKind = priorSlotKind
    /\ bindingFences = priorBindingFences
    /\ cachedResults = priorCachedResults
    /\ everBound = priorEverBound

SlotOccupiedHasPriority ==
  slotPhase # "empty" =>
    \A terminal \in TerminalIds:
      Classify(terminal, 1, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
        = "slot-occupied"

OnlyEmptySlotCanAdmitOrQueue ==
  lastAction \in SlotAdmissionActions => priorSlotPhase = "empty"

FullCapacityExistingBindingRespectsSlot ==
  BindingCount = Capacity /\ slotPhase # "empty" =>
    \A terminal \in BoundTerminals(bindingFences):
      /\ Classify(
           terminal,
           bindingFences[terminal],
           TRUE,
           TRUE,
           TRUE,
           TRUE,
           FALSE,
           FALSE
         ) = "slot-occupied"
      /\ bindingFences[terminal] < MaxFence =>
           Classify(
             terminal,
             bindingFences[terminal] + 1,
             FALSE,
             TRUE,
             TRUE,
             TRUE,
             TRUE,
             TRUE
           ) = "slot-occupied"

ResultDeliveryPreservesBindingsAndClearsSlot ==
  lastAction = "deliver-result" =>
    /\ priorSlotPhase = "result-ready"
    /\ slotPhase = "empty"
    /\ slotTerminal = NoTerminal
    /\ slotFence = NoFence
    /\ slotKind = "none"
    /\ bindingFences = priorBindingFences
    /\ cachedResults = priorCachedResults
    /\ everBound = priorEverBound

CapacityRejectsOnlyUnseenTerminals ==
  lastDecision = "capacity-exceeded" =>
    /\ priorSlotPhase = "empty"
    /\ lastTerminal \in TerminalIds
    /\ priorBindingFences[lastTerminal] = NoFence
    /\ bindingFences[lastTerminal] = NoFence
    /\ cachedResults[lastTerminal] = NoResult

ExactReplayPreservesBindingAndCache ==
  lastAction = "replay-exact-at-capacity" =>
    /\ priorSlotPhase = "empty"
    /\ slotPhase = "result-ready"
    /\ slotKind = "binding-result"
    /\ bindingFences = priorBindingFences
    /\ cachedResults = priorCachedResults
    /\ cachedResults[lastTerminal] = CachedResult

HigherFenceReplacementPreservesMembership ==
  lastAction = "replace-higher-fence-at-capacity" =>
    /\ priorSlotPhase = "empty"
    /\ slotPhase = "operation-pending"
    /\ slotKind = "binding-result"
    /\ BoundTerminals(bindingFences) = BoundTerminals(priorBindingFences)
    /\ Cardinality(BoundTerminals(bindingFences)) = Capacity
    /\ bindingFences[lastTerminal] = priorBindingFences[lastTerminal] + 1
    /\ cachedResults[lastTerminal] = NoResult
    /\ \A terminal \in TerminalIds \ {lastTerminal}:
         /\ bindingFences[terminal] = priorBindingFences[terminal]
         /\ cachedResults[terminal] = priorCachedResults[terminal]

=============================================================================
