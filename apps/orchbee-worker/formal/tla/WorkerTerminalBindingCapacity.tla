------------------- MODULE WorkerTerminalBindingCapacity -------------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded refinement of the generation-local Worker terminal-binding map.
\* The production limit is 1,024; TLC uses a smaller Capacity with one extra
\* TerminalId to exhaustively cover the threshold. Slot phases, Host handoff,
\* time monotonicity, and result delivery remain in TerminalLaunchAuthorization.

CONSTANTS TerminalIds, Capacity, MaxFence

NoTerminal == "none"
NoFence == 0
NoResult == "none"
CachedResult == "cached-result"

Decisions == {
  "none", "dispatch", "replay", "wrong-worker", "stale-generation",
  "unsupported-capability", "lease-expired", "stale-fence",
  "conflicting-replay", "capacity-exceeded", "host-identity-changed"
}

Actions == {
  "init", "admit-new-binding", "cache-binding-result",
  "reject-wrong-worker-at-capacity", "reject-stale-generation-at-capacity",
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
  /\ MaxFence \in Nat
  /\ MaxFence >= 2

VARIABLES
  bindingFences,
  cachedResults,
  everBound,
  priorBindingFences,
  priorCachedResults,
  priorEverBound,
  lastTerminal,
  lastDecision,
  lastAction

vars == <<bindingFences, cachedResults, everBound, priorBindingFences,
  priorCachedResults, priorEverBound, lastTerminal, lastDecision, lastAction>>

BoundTerminals(fences) ==
  {terminal \in TerminalIds : fences[terminal] # NoFence}

BindingCount == Cardinality(BoundTerminals(bindingFences))

Classify(terminal, requestFence, exactBinding, workerMatches,
    generationMatches, capabilityMatches, leaseIsLive, startIdentityMatches) ==
  IF ~workerMatches
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
  /\ lastTerminal' = terminal
  /\ lastDecision' = decision
  /\ lastAction' = action

Init ==
  /\ bindingFences = [terminal \in TerminalIds |-> NoFence]
  /\ cachedResults = [terminal \in TerminalIds |-> NoResult]
  /\ everBound = {}
  /\ priorBindingFences = bindingFences
  /\ priorCachedResults = cachedResults
  /\ priorEverBound = everBound
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
    /\ Record("admit-new-binding", terminal, "dispatch")

CacheBindingResult ==
  \E terminal \in BoundTerminals(bindingFences):
    /\ cachedResults[terminal] = NoResult
    /\ cachedResults' = [cachedResults EXCEPT ![terminal] = CachedResult]
    /\ UNCHANGED <<bindingFences, everBound>>
    /\ Record("cache-binding-result", terminal, CachedResult)

PreserveRejectedRequest(action, terminal, decision) ==
  /\ UNCHANGED <<bindingFences, cachedResults, everBound>>
  /\ Record(action, terminal, decision)

UnseenAtCapacity(terminal) ==
  /\ terminal \in TerminalIds
  /\ bindingFences[terminal] = NoFence
  /\ BindingCount = Capacity

RejectWrongWorkerAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
         = "wrong-worker"
    /\ PreserveRejectedRequest(
         "reject-wrong-worker-at-capacity", terminal, "wrong-worker")

RejectStaleGenerationAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE)
         = "stale-generation"
    /\ PreserveRejectedRequest(
         "reject-stale-generation-at-capacity", terminal, "stale-generation")

RejectUnsupportedCapabilityAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE)
         = "unsupported-capability"
    /\ PreserveRejectedRequest(
         "reject-unsupported-capability-at-capacity", terminal,
         "unsupported-capability")

RejectExpiredLeaseAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, TRUE, TRUE, FALSE, FALSE)
         = "lease-expired"
    /\ PreserveRejectedRequest(
         "reject-expired-lease-at-capacity", terminal, "lease-expired")

RejectNewTerminalAtCapacity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, TRUE, TRUE, TRUE, TRUE)
         = "capacity-exceeded"
    /\ PreserveRejectedRequest(
         "reject-new-terminal-at-capacity", terminal, "capacity-exceeded")

RejectCapacityBeforeStartIdentity ==
  \E terminal \in TerminalIds:
    /\ UnseenAtCapacity(terminal)
    /\ Classify(terminal, 1, FALSE, TRUE, TRUE, TRUE, TRUE, FALSE)
         = "capacity-exceeded"
    /\ PreserveRejectedRequest(
         "reject-capacity-before-start-identity", terminal, "capacity-exceeded")

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
    /\ PreserveRejectedRequest(
         "reject-lower-fence-at-capacity", terminal, "stale-fence")

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
    /\ PreserveRejectedRequest(
         "reject-conflicting-fence-at-capacity", terminal,
         "conflicting-replay")

Next ==
  \/ AdmitNewBinding
  \/ CacheBindingResult
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
  /\ priorBindingFences \in [TerminalIds -> 0..MaxFence]
  /\ priorCachedResults \in [TerminalIds -> {NoResult, CachedResult}]
  /\ priorEverBound \subseteq TerminalIds
  /\ lastTerminal \in TerminalIds \cup {NoTerminal}
  /\ lastDecision \in Decisions \cup {CachedResult}
  /\ lastAction \in Actions

CapacityIsBounded == BindingCount <= Capacity

BindingAndCacheAreConsistent ==
  /\ everBound = BoundTerminals(bindingFences)
  /\ \A terminal \in TerminalIds:
       bindingFences[terminal] = NoFence => cachedResults[terminal] = NoResult

RetainedBindingsNeverDisappear ==
  BoundTerminals(priorBindingFences) \subseteq BoundTerminals(bindingFences)

RejectedRequestPreservesBindings ==
  lastDecision \in {
    "wrong-worker", "stale-generation", "unsupported-capability",
    "lease-expired", "stale-fence", "conflicting-replay",
    "capacity-exceeded", "host-identity-changed"
  } =>
    /\ bindingFences = priorBindingFences
    /\ cachedResults = priorCachedResults
    /\ everBound = priorEverBound

CapacityRejectsOnlyUnseenTerminals ==
  lastDecision = "capacity-exceeded" =>
    /\ lastTerminal \in TerminalIds
    /\ priorBindingFences[lastTerminal] = NoFence
    /\ bindingFences[lastTerminal] = NoFence
    /\ cachedResults[lastTerminal] = NoResult

ExactReplayPreservesBindingAndCache ==
  lastAction = "replay-exact-at-capacity" =>
    /\ bindingFences = priorBindingFences
    /\ cachedResults = priorCachedResults
    /\ cachedResults[lastTerminal] = CachedResult

HigherFenceReplacementPreservesMembership ==
  lastAction = "replace-higher-fence-at-capacity" =>
    /\ BoundTerminals(bindingFences) = BoundTerminals(priorBindingFences)
    /\ Cardinality(BoundTerminals(bindingFences)) = Capacity
    /\ bindingFences[lastTerminal] = priorBindingFences[lastTerminal] + 1
    /\ cachedResults[lastTerminal] = NoResult
    /\ \A terminal \in TerminalIds \ {lastTerminal}:
         /\ bindingFences[terminal] = priorBindingFences[terminal]
         /\ cachedResults[terminal] = priorCachedResults[terminal]

=============================================================================
