---------------- MODULE TerminalScrollbackCompression ----------------
EXTENDS Naturals, TLC

\* Refines runtime-model claim TH-024. Terminal-owned compression activity is
\* observed only while a non-blocking terminal-lock attempt succeeds. A changed
\* token starts a fresh idle interval. Once that interval expires, one fair
\* Host maintenance turn may perform at most one bounded Ghostty incremental
\* compression step without changing terminal revision, generation, or content.
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
  "compression-lock-becomes-busy",
  "compression-lock-becomes-available",
  "compression-observe-activity",
  "compression-idle-wait",
  "compression-lock-busy",
  "compression-step-pending",
  "compression-step-complete",
  "compression-step-unsupported",
  "compression-dormant"
}

VARIABLES
  activityToken,
  observedToken,
  idleRemaining,
  outcome,
  workRemaining,
  lockBusy,
  cursor,
  revision,
  generation,
  attemptsInTurn,
  lastAction,
  previousActivityToken,
  previousObservedToken,
  previousIdleRemaining,
  previousOutcome,
  previousWorkRemaining,
  previousLockBusy,
  previousCursor,
  previousRevision,
  previousGeneration

vars == <<activityToken, observedToken, idleRemaining, outcome,
  workRemaining, lockBusy, cursor, revision, generation, attemptsInTurn,
  lastAction, previousActivityToken, previousObservedToken,
  previousIdleRemaining, previousOutcome, previousWorkRemaining,
  previousLockBusy, previousCursor, previousRevision, previousGeneration>>

Init ==
  /\ Scenario \in Scenarios
  /\ TerminalCount > 0
  /\ IdleTurns > 0
  /\ MaxWork > 0
  /\ SupportedTerminals \subseteq Terminals
  /\ activityToken = [terminal \in Terminals |-> "token-a"]
  /\ observedToken = [terminal \in Terminals |-> "none"]
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
  /\ previousActivityToken = activityToken
  /\ previousObservedToken = observedToken
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
  /\ previousActivityToken' = activityToken
  /\ previousObservedToken' = observedToken
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
  /\ activityToken' = [activityToken EXCEPT ![terminal] =
       IF @ = "token-a" THEN "token-b" ELSE "token-a"]
  /\ outcome' = [outcome EXCEPT ![terminal] = "idle"]
  /\ workRemaining' = [workRemaining EXCEPT ![terminal] =
       IF terminal \in SupportedTerminals THEN MaxWork ELSE 0]
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<observedToken, idleRemaining, lockBusy, cursor,
       revision, generation>>
  /\ Record("compression-activity-changes")

CompressionLockBecomesBusy(terminal) ==
  /\ Scenario = "interference"
  /\ terminal \in Terminals
  /\ ~lockBusy[terminal]
  /\ lockBusy' = [lockBusy EXCEPT ![terminal] = TRUE]
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<activityToken, observedToken, idleRemaining, outcome,
       workRemaining, cursor, revision, generation>>
  /\ Record("compression-lock-becomes-busy")

CompressionLockBecomesAvailable(terminal) ==
  /\ Scenario = "interference"
  /\ terminal \in Terminals
  /\ lockBusy[terminal]
  /\ lockBusy' = [lockBusy EXCEPT ![terminal] = FALSE]
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<activityToken, observedToken, idleRemaining, outcome,
       workRemaining, cursor, revision, generation>>
  /\ Record("compression-lock-becomes-available")

CompressionLockBusy ==
  /\ lockBusy[cursor]
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<activityToken, observedToken, outcome, workRemaining,
       lockBusy, revision, generation>>
  /\ Record("compression-lock-busy")

CompressionObserveActivity ==
  /\ ~lockBusy[cursor]
  /\ observedToken[cursor] # activityToken[cursor]
  /\ observedToken' = [observedToken EXCEPT ![cursor] = activityToken[cursor]]
  /\ idleRemaining' = [AgedIdle EXCEPT ![cursor] = IdleTurns]
  /\ outcome' = [outcome EXCEPT ![cursor] = "idle"]
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<activityToken, workRemaining, lockBusy, revision, generation>>
  /\ Record("compression-observe-activity")

CompressionIdleWait ==
  /\ ~lockBusy[cursor]
  /\ observedToken[cursor] = activityToken[cursor]
  /\ idleRemaining[cursor] > 0
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<activityToken, observedToken, outcome, workRemaining,
       lockBusy, revision, generation>>
  /\ Record("compression-idle-wait")

CompressionStepPending ==
  /\ ~lockBusy[cursor]
  /\ observedToken[cursor] = activityToken[cursor]
  /\ idleRemaining[cursor] = 0
  /\ cursor \in SupportedTerminals
  /\ outcome[cursor] \in {"idle", "pending"}
  /\ workRemaining[cursor] > 1
  /\ outcome' = [outcome EXCEPT ![cursor] = "pending"]
  /\ workRemaining' = [workRemaining EXCEPT ![cursor] = @ - 1]
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 1
  /\ UNCHANGED <<activityToken, observedToken, lockBusy, revision, generation>>
  /\ Record("compression-step-pending")

CompressionStepComplete ==
  /\ ~lockBusy[cursor]
  /\ observedToken[cursor] = activityToken[cursor]
  /\ idleRemaining[cursor] = 0
  /\ cursor \in SupportedTerminals
  /\ outcome[cursor] \in {"idle", "pending"}
  /\ workRemaining[cursor] <= 1
  /\ outcome' = [outcome EXCEPT ![cursor] = "complete"]
  /\ workRemaining' = [workRemaining EXCEPT ![cursor] = 0]
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 1
  /\ UNCHANGED <<activityToken, observedToken, lockBusy, revision, generation>>
  /\ Record("compression-step-complete")

CompressionStepUnsupported ==
  /\ ~lockBusy[cursor]
  /\ observedToken[cursor] = activityToken[cursor]
  /\ idleRemaining[cursor] = 0
  /\ cursor \notin SupportedTerminals
  /\ outcome[cursor] \in {"idle", "pending"}
  /\ outcome' = [outcome EXCEPT ![cursor] = "unsupported"]
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 1
  /\ UNCHANGED <<activityToken, observedToken, workRemaining, lockBusy,
       revision, generation>>
  /\ Record("compression-step-unsupported")

CompressionDormant ==
  /\ ~lockBusy[cursor]
  /\ observedToken[cursor] = activityToken[cursor]
  /\ idleRemaining[cursor] = 0
  /\ outcome[cursor] \in {"complete", "unsupported"}
  /\ idleRemaining' = AgedIdle
  /\ cursor' = NextCursor
  /\ attemptsInTurn' = 0
  /\ UNCHANGED <<activityToken, observedToken, outcome, workRemaining,
       lockBusy, revision, generation>>
  /\ Record("compression-dormant")

Maintenance ==
  \/ CompressionLockBusy
  \/ CompressionObserveActivity
  \/ CompressionIdleWait
  \/ CompressionStepPending
  \/ CompressionStepComplete
  \/ CompressionStepUnsupported
  \/ CompressionDormant

Next ==
  \/ \E terminal \in Terminals: CompressionActivityChanges(terminal)
  \/ \E terminal \in Terminals: CompressionLockBecomesBusy(terminal)
  \/ \E terminal \in Terminals: CompressionLockBecomesAvailable(terminal)
  \/ Maintenance

Spec == Init /\ [][Next]_vars /\ WF_vars(Maintenance)

TypeOK ==
  /\ activityToken \in [Terminals -> Tokens]
  /\ observedToken \in [Terminals -> ObservedTokens]
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
    /\ previousObservedToken[previousCursor] = previousActivityToken[previousCursor]
    /\ previousIdleRemaining[previousCursor] = 0
    /\ ~previousLockBusy[previousCursor]

LockContentionNeverAttempts ==
  lastAction = "compression-lock-busy" => attemptsInTurn = 0

ActivityObservationRestartsIdle ==
  lastAction = "compression-observe-activity" =>
    /\ idleRemaining[previousCursor] = IdleTurns
    /\ attemptsInTurn = 0

OneBoundedAttemptPerTurn == attemptsInTurn <= 1

RoundRobinMaintenance ==
  lastAction \in {
    "compression-observe-activity", "compression-idle-wait",
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
