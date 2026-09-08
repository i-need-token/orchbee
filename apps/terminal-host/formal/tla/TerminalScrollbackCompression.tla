---------------- MODULE TerminalScrollbackCompression ----------------
EXTENDS Naturals, TLC

\* Refines runtime-model claim TH-024. Ghostty compression activity and the
\* Terminal-owned interaction activity is observed only while a non-blocking
\* terminal-lock attempt succeeds. Compression activity starts a fresh idle
\* interval. Interaction activity restarts an existing waiting or pending
\* interval without waking dormant work. Once that interval expires, one fair
\* Host maintenance turn may perform at most one bounded Ghostty incremental
\* compression step without changing terminal revision, generation, or content.
\* interactionDirty is the finite abstraction of "the interaction serial has
\* changed at least once since observation". Unlike a wrapping token, it cannot
\* erase two intervening interactions through an ABA return to equality.
\* Refinement from the runtime's wrapping u64 therefore assumes fewer than
\* 2^64 interactions between successful observations.
\*
\* The interference scenario permits arbitrary activity and lock contention.
\* The quiescent scenario removes both and uses weak fairness for maintenance,
\* proving that finite supported work settles while unsupported targets become
\* dormant. Terminal mutation is intentionally outside the compression action:
\* this model treats Ghostty's representation-only step as the refined effect.

CONSTANTS TerminalCount, IdleTurns, MaxWork, SupportedTerminals, Scenario

Terminals == 1..TerminalCount
Tokens == {"token-a", "token-b"}
ObservedTokens == Tokens \cup {"none"}
Outcomes == {"idle", "pending", "complete", "unsupported"}
Scenarios == {"interference", "quiescent"}
Actions == {
  "init",
  "compression-activity-changes",
  "interaction-activity-changes",
  "compression-lock-becomes-busy",
  "compression-lock-becomes-available",
  "compression-observe-activity",
  "interaction-observe-activity",
  "dormant-interaction-observation",
  "compression-idle-wait",
  "compression-lock-busy",
  "compression-step-pending",
  "compression-step-complete",
  "compression-step-unsupported",
  "compression-dormant"
}

VARIABLES
  compressionToken,
  interactionDirty,
  observedCompressionToken,
  idleRemaining,
  outcome,
  workRemaining,
  lockBusy,
  cursor,
  revision,
  generation,
  attemptsInTurn,
  lastAction,
  previousCompressionToken,
  previousInteractionDirty,
  previousObservedCompressionToken,
  previousIdleRemaining,
  previousOutcome,
  previousWorkRemaining,
  previousLockBusy,
  previousCursor,
  previousRevision,
  previousGeneration

vars == <<compressionToken, interactionDirty,
  observedCompressionToken, idleRemaining, outcome,
  workRemaining, lockBusy, cursor, revision, generation, attemptsInTurn,
  lastAction, previousCompressionToken, previousInteractionDirty,
  previousObservedCompressionToken,
  previousIdleRemaining, previousOutcome, previousWorkRemaining,
  previousLockBusy, previousCursor, previousRevision, previousGeneration>>

Init ==
  /\ Scenario \in Scenarios
  /\ TerminalCount > 0
  /\ IdleTurns > 0
  /\ MaxWork > 0
  /\ SupportedTerminals \subseteq Terminals
  /\ compressionToken = [terminal \in Terminals |-> "token-a"]
  /\ interactionDirty = [terminal \in Terminals |-> FALSE]
  /\ observedCompressionToken = [terminal \in Terminals |-> "none"]
  /\ idleRemaining = [terminal \in Terminals |-> 0]
  /\ outcome = [terminal \in Terminals |-> "idle"]
  /\ workRemaining = [terminal \in Terminals |->
       IF terminal \in SupportedTerminals THEN MaxWork ELSE 0]
  /\ lockBusy = [terminal \in Terminals |-> FALSE]
  /\ cursor = 1
  /\ revision = [terminal \in Terminals |-> 1]
  /\ generation = [terminal \in Terminals |-> 1]
  /\ attemptsInTurn = 0
  /\ lastAction = "init"
  /\ previousCompressionToken = compressionToken
  /\ previousInteractionDirty = interactionDirty
  /\ previousObservedCompressionToken = observedCompressionToken
  /\ previousIdleRemaining = idleRemaining
  /\ previousOutcome = outcome
  /\ previousWorkRemaining = workRemaining
  /\ previousLockBusy = lockBusy
  /\ previousCursor = cursor
  /\ previousRevision = revision
  /\ previousGeneration = generation

NextCursor == IF cursor = TerminalCount THEN 1 ELSE cursor + 1

AgedIdle ==
  [terminal \in Terminals |->
    IF idleRemaining[terminal] = 0
      THEN 0
      ELSE idleRemaining[terminal] - 1]

Record(action) ==
  /\ lastAction' = action
  /\ previousCompressionToken' = compressionToken
  /\ previousInteractionDirty' = interactionDirty
  /\ previousObservedCompressionToken' = observedCompressionToken
  /\ previousIdleRemaining' = idleRemaining
  /\ previousOutcome' = outcome
  /\ previousWorkRemaining' = workRemaining
  /\ previousLockBusy' = lockBusy
  /\ previousCursor' = cursor
  /\ previousRevision' = revision
  /\ previousGeneration' = generation

CompressionActivityChanges(terminal) ==
  /\ Scenario = "interference"
  /\ terminal \in Terminals
  /\ compressionToken' = [compressionToken EXCEPT ![terminal] =
       IF @ = "token-a" THEN "token-b" ELSE "token-a"]
  /\ workRemaining' = [workRemaining EXCEPT ![terminal] =
       IF terminal \in SupportedTerminals THEN MaxWork ELSE 0]
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<interactionDirty, observedCompressionToken,
       idleRemaining, outcome, lockBusy, cursor,
       revision, generation>>
  /\ Record("compression-activity-changes")

InteractionActivityChanges(terminal) ==
  /\ Scenario = "interference"
  /\ terminal \in Terminals
  /\ interactionDirty' =
       [interactionDirty EXCEPT ![terminal] = TRUE]
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<compressionToken, observedCompressionToken,
       idleRemaining, outcome, workRemaining,
       lockBusy, cursor, revision, generation>>
  /\ Record("interaction-activity-changes")

CompressionLockBecomesBusy(terminal) ==
  /\ Scenario = "interference"
  /\ terminal \in Terminals
  /\ ~lockBusy[terminal]
  /\ lockBusy' = [lockBusy EXCEPT ![terminal] = TRUE]
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<compressionToken, interactionDirty,
       observedCompressionToken, idleRemaining,
       outcome, workRemaining, cursor, revision, generation>>
  /\ Record("compression-lock-becomes-busy")

CompressionLockBecomesAvailable(terminal) ==
  /\ Scenario = "interference"
  /\ terminal \in Terminals
  /\ lockBusy[terminal]
  /\ lockBusy' = [lockBusy EXCEPT ![terminal] = FALSE]
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<compressionToken, interactionDirty,
       observedCompressionToken, idleRemaining,
       outcome, workRemaining, cursor, revision, generation>>
  /\ Record("compression-lock-becomes-available")

CompressionLockBusy ==
  /\ lockBusy[cursor]
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<compressionToken, interactionDirty,
       observedCompressionToken, outcome,
       workRemaining, lockBusy, revision, generation>>
  /\ Record("compression-lock-busy")

CompressionObserveActivity ==
  /\ ~lockBusy[cursor]
  /\ observedCompressionToken[cursor] # compressionToken[cursor]
  /\ observedCompressionToken' =
       [observedCompressionToken EXCEPT ![cursor] = compressionToken[cursor]]
  /\ interactionDirty' =
       [interactionDirty EXCEPT ![cursor] = FALSE]
  /\ idleRemaining' = [AgedIdle EXCEPT ![cursor] = IdleTurns]
  /\ outcome' = [outcome EXCEPT ![cursor] = "idle"]
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<compressionToken, workRemaining,
       lockBusy, revision, generation>>
  /\ Record("compression-observe-activity")

InteractionObserveActivity ==
  /\ ~lockBusy[cursor]
  /\ observedCompressionToken[cursor] = compressionToken[cursor]
  /\ interactionDirty[cursor]
  /\ outcome[cursor] \in {"idle", "pending"}
  /\ interactionDirty' =
       [interactionDirty EXCEPT ![cursor] = FALSE]
  /\ idleRemaining' = [AgedIdle EXCEPT ![cursor] = IdleTurns]
  /\ outcome' = [outcome EXCEPT ![cursor] = "idle"]
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<compressionToken, observedCompressionToken, workRemaining,
       lockBusy, revision, generation>>
  /\ Record("interaction-observe-activity")

DormantInteractionObservation ==
  /\ ~lockBusy[cursor]
  /\ observedCompressionToken[cursor] = compressionToken[cursor]
  /\ interactionDirty[cursor]
  /\ outcome[cursor] \in {"complete", "unsupported"}
  /\ interactionDirty' =
       [interactionDirty EXCEPT ![cursor] = FALSE]
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<compressionToken, observedCompressionToken, outcome,
       workRemaining, lockBusy,
       revision, generation>>
  /\ Record("dormant-interaction-observation")

CompressionIdleWait ==
  /\ ~lockBusy[cursor]
  /\ observedCompressionToken[cursor] = compressionToken[cursor]
  /\ ~interactionDirty[cursor]
  /\ idleRemaining[cursor] > 0
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<compressionToken, interactionDirty,
       observedCompressionToken, outcome,
       workRemaining, lockBusy, revision, generation>>
  /\ Record("compression-idle-wait")

CompressionStepPending ==
  /\ ~lockBusy[cursor]
  /\ observedCompressionToken[cursor] = compressionToken[cursor]
  /\ ~interactionDirty[cursor]
  /\ idleRemaining[cursor] = 0
  /\ cursor \in SupportedTerminals
  /\ outcome[cursor] \in {"idle", "pending"}
  /\ workRemaining[cursor] > 1
  /\ outcome' = [outcome EXCEPT ![cursor] = "pending"]
  /\ workRemaining' = [workRemaining EXCEPT ![cursor] = @ - 1]
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 1
  /\ UNCHANGED <<compressionToken, interactionDirty,
       observedCompressionToken, lockBusy,
       revision, generation>>
  /\ Record("compression-step-pending")

CompressionStepComplete ==
  /\ ~lockBusy[cursor]
  /\ observedCompressionToken[cursor] = compressionToken[cursor]
  /\ ~interactionDirty[cursor]
  /\ idleRemaining[cursor] = 0
  /\ cursor \in SupportedTerminals
  /\ outcome[cursor] \in {"idle", "pending"}
  /\ workRemaining[cursor] <= 1
  /\ outcome' = [outcome EXCEPT ![cursor] = "complete"]
  /\ workRemaining' = [workRemaining EXCEPT ![cursor] = 0]
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 1
  /\ UNCHANGED <<compressionToken, interactionDirty,
       observedCompressionToken, lockBusy,
       revision, generation>>
  /\ Record("compression-step-complete")

CompressionStepUnsupported ==
  /\ ~lockBusy[cursor]
  /\ observedCompressionToken[cursor] = compressionToken[cursor]
  /\ ~interactionDirty[cursor]
  /\ idleRemaining[cursor] = 0
  /\ cursor \notin SupportedTerminals
  /\ outcome[cursor] \in {"idle", "pending"}
  /\ outcome' = [outcome EXCEPT ![cursor] = "unsupported"]
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 1
  /\ UNCHANGED <<compressionToken, interactionDirty,
       observedCompressionToken, workRemaining,
       lockBusy, revision, generation>>
  /\ Record("compression-step-unsupported")

CompressionDormant ==
  /\ ~lockBusy[cursor]
  /\ observedCompressionToken[cursor] = compressionToken[cursor]
  /\ ~interactionDirty[cursor]
  /\ idleRemaining[cursor] = 0
  /\ outcome[cursor] \in {"complete", "unsupported"}
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<compressionToken, interactionDirty,
       observedCompressionToken, outcome,
       workRemaining, lockBusy, revision, generation>>
  /\ Record("compression-dormant")

Maintenance ==
  \/ CompressionLockBusy
  \/ CompressionObserveActivity
  \/ InteractionObserveActivity
  \/ DormantInteractionObservation
  \/ CompressionIdleWait
  \/ CompressionStepPending
  \/ CompressionStepComplete
  \/ CompressionStepUnsupported
  \/ CompressionDormant

Next ==
  \/ \E terminal \in Terminals: CompressionActivityChanges(terminal)
  \/ \E terminal \in Terminals: InteractionActivityChanges(terminal)
  \/ \E terminal \in Terminals: CompressionLockBecomesBusy(terminal)
  \/ \E terminal \in Terminals: CompressionLockBecomesAvailable(terminal)
  \/ Maintenance

Spec == Init /\ [][Next]_vars /\ WF_vars(Maintenance)

TypeOK ==
  /\ compressionToken \in [Terminals -> Tokens]
  /\ interactionDirty \in [Terminals -> BOOLEAN]
  /\ observedCompressionToken \in [Terminals -> ObservedTokens]
  /\ idleRemaining \in [Terminals -> 0..IdleTurns]
  /\ outcome \in [Terminals -> Outcomes]
  /\ workRemaining \in [Terminals -> 0..MaxWork]
  /\ lockBusy \in [Terminals -> BOOLEAN]
  /\ cursor \in Terminals
  /\ revision \in [Terminals -> Nat]
  /\ generation \in [Terminals -> Nat]
  /\ attemptsInTurn \in 0..1
  /\ lastAction \in Actions

CompressionNeverChangesSemanticState ==
  /\ revision = previousRevision
  /\ generation = previousGeneration

CompressionRequiresObservedIdle ==
  attemptsInTurn = 1 =>
    /\ previousObservedCompressionToken[previousCursor] =
         previousCompressionToken[previousCursor]
    /\ ~previousInteractionDirty[previousCursor]
    /\ previousIdleRemaining[previousCursor] = 0
    /\ ~previousLockBusy[previousCursor]

LockContentionNeverAttempts ==
  lastAction = "compression-lock-busy" => attemptsInTurn = 0

ActivityObservationRestartsIdle ==
  lastAction \in {"compression-observe-activity", "interaction-observe-activity"} =>
    /\ idleRemaining[previousCursor] = IdleTurns
    /\ attemptsInTurn = 0

DormantInteractionPreservesDormancy ==
  lastAction = "dormant-interaction-observation" =>
    /\ outcome[previousCursor] = previousOutcome[previousCursor]
    /\ outcome[previousCursor] \in {"complete", "unsupported"}
    /\ attemptsInTurn = 0

RoundRobinMaintenance ==
  lastAction \in {
    "compression-observe-activity", "interaction-observe-activity",
    "dormant-interaction-observation", "compression-idle-wait",
    "compression-lock-busy", "compression-step-pending",
    "compression-step-complete", "compression-step-unsupported",
    "compression-dormant"
  } => cursor = IF previousCursor = TerminalCount THEN 1 ELSE previousCursor + 1

ScenarioProgress ==
  IF Scenario = "quiescent"
    THEN <> (\A terminal \in Terminals:
      outcome[terminal] \in {"complete", "unsupported"})
    ELSE []TypeOK

=============================================================================
