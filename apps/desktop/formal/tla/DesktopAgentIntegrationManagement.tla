---------------- MODULE DesktopAgentIntegrationManagement ----------------
EXTENDS TLC, Naturals

\* Two representative adapters expose controller serialization and target
\* isolation. actual is the stable provider state observed at adapter preflight
\* or after that adapter's own effect. Concurrent third-party edits and the
\* shared scheduling boundary with launch-scoped inspection are outside this
\* model and remain owned by adapter/controller integration evidence.

Adapters == {"claude-code", "codex"}
States == {"not-installed", "disabled", "enabled", "update-required", "conflict"}
ObservedStates == States \cup {"unknown"}
Operations == {"none", "inspect", "enable", "disable", "verify"}
DesiredStates == {"enabled", "disabled"}
AttemptOutcomes == {"none", "attempted", "failed"}
Results == {
  "none", "pending", "completed", "conflict", "operation-failed",
  "verification-failed"
}

VARIABLES
  phase,
  actual,
  observed,
  operation,
  target,
  desired,
  mutationCommitted,
  attemptOutcome,
  pendingOperations,
  result,
  lastAction,
  previousActual,
  previousObserved,
  previousOperation,
  previousTarget,
  previousDesired,
  previousMutationCommitted,
  previousAttemptOutcome,
  previousPendingOperations,
  previousResult

vars == <<phase, actual, observed, operation, target, desired,
  mutationCommitted, attemptOutcome, pendingOperations, result, lastAction,
  previousActual, previousObserved, previousOperation, previousTarget,
  previousDesired, previousMutationCommitted, previousAttemptOutcome,
  previousPendingOperations, previousResult>>

DesiredObserved(requestedState, observedState) ==
  \/ /\ requestedState = "enabled"
     /\ observedState = "enabled"
  \/ /\ requestedState = "disabled"
     /\ observedState \in {"disabled", "not-installed"}

Record(action) ==
  /\ lastAction' = action
  /\ previousActual' = actual
  /\ previousObserved' = observed
  /\ previousOperation' = operation
  /\ previousTarget' = target
  /\ previousDesired' = desired
  /\ previousMutationCommitted' = mutationCommitted
  /\ previousAttemptOutcome' = attemptOutcome
  /\ previousPendingOperations' = pendingOperations
  /\ previousResult' = result

Init ==
  /\ phase = "uninitialized"
  /\ actual \in [Adapters -> States]
  /\ observed = [adapter \in Adapters |-> "unknown"]
  /\ operation = "none"
  /\ target = "claude-code"
  /\ desired = "enabled"
  /\ mutationCommitted = FALSE
  /\ attemptOutcome = "none"
  /\ pendingOperations = 0
  /\ result = "none"
  /\ lastAction = "init"
  /\ previousActual = actual
  /\ previousObserved = observed
  /\ previousOperation = operation
  /\ previousTarget = target
  /\ previousDesired = desired
  /\ previousMutationCommitted = mutationCommitted
  /\ previousAttemptOutcome = attemptOutcome
  /\ previousPendingOperations = pendingOperations
  /\ previousResult = result

Initialize ==
  /\ phase = "uninitialized"
  /\ phase' = "ready"
  /\ operation' = "none"
  /\ mutationCommitted' = FALSE
  /\ attemptOutcome' = "none"
  /\ pendingOperations' = 0
  /\ result' = "none"
  /\ UNCHANGED <<actual, observed, target, desired>>
  /\ Record("initialize")

RequestRefresh ==
  /\ phase = "ready"
  /\ operation = "none"
  /\ operation' = "inspect"
  /\ mutationCommitted' = FALSE
  /\ attemptOutcome' = "none"
  /\ pendingOperations' = pendingOperations + 1
  /\ result' = "pending"
  /\ UNCHANGED <<phase, actual, observed, target, desired>>
  /\ Record("request-refresh")

InspectCompletes ==
  /\ phase = "ready"
  /\ operation = "inspect"
  /\ observed' = actual
  /\ operation' = "none"
  /\ pendingOperations' = pendingOperations - 1
  /\ result' = "completed"
  /\ UNCHANGED <<phase, actual, target, desired, mutationCommitted,
    attemptOutcome>>
  /\ Record("inspect-completes")

InspectFails(failedAdapters) ==
  /\ phase = "ready"
  /\ operation = "inspect"
  /\ failedAdapters \in (SUBSET Adapters) \ {{}}
  /\ observed' = [adapter \in Adapters |->
       IF adapter \in failedAdapters THEN "unknown" ELSE actual[adapter]]
  /\ operation' = "none"
  /\ pendingOperations' = pendingOperations - 1
  /\ result' = "completed"
  /\ UNCHANGED <<phase, actual, target, desired, mutationCommitted,
    attemptOutcome>>
  /\ Record("inspect-fails")

RequestEnable(adapter) ==
  /\ phase = "ready"
  /\ operation = "none"
  /\ adapter \in Adapters
  /\ actual[adapter] # "conflict"
  /\ ~DesiredObserved("enabled", actual[adapter])
  /\ operation' = "enable"
  /\ target' = adapter
  /\ desired' = "enabled"
  /\ mutationCommitted' = FALSE
  /\ attemptOutcome' = "none"
  /\ pendingOperations' = pendingOperations + 1
  /\ result' = "pending"
  /\ UNCHANGED <<phase, actual, observed>>
  /\ Record("request-enable")

RequestDisable(adapter) ==
  /\ phase = "ready"
  /\ operation = "none"
  /\ adapter \in Adapters
  /\ actual[adapter] # "conflict"
  /\ ~DesiredObserved("disabled", actual[adapter])
  /\ operation' = "disable"
  /\ target' = adapter
  /\ desired' = "disabled"
  /\ mutationCommitted' = FALSE
  /\ attemptOutcome' = "none"
  /\ pendingOperations' = pendingOperations + 1
  /\ result' = "pending"
  /\ UNCHANGED <<phase, actual, observed>>
  /\ Record("request-disable")

RejectConflict(adapter) ==
  /\ phase = "ready"
  /\ operation = "none"
  /\ adapter \in Adapters
  /\ actual[adapter] = "conflict"
  /\ target' = adapter
  /\ result' = "conflict"
  /\ UNCHANGED <<phase, actual, observed, operation, desired,
    mutationCommitted, attemptOutcome, pendingOperations>>
  /\ Record("reject-conflict")

CommitMutation(nextState) ==
  /\ phase = "ready"
  /\ operation \in {"enable", "disable"}
  /\ nextState \in States
  /\ actual' = [actual EXCEPT ![target] = nextState]
  /\ operation' = "verify"
  /\ mutationCommitted' = TRUE
  /\ attemptOutcome' = "attempted"
  /\ result' = "pending"
  /\ UNCHANGED <<phase, observed, target, desired, pendingOperations>>
  /\ Record("commit-mutation")

MutationFails ==
  /\ phase = "ready"
  /\ operation \in {"enable", "disable"}
  /\ operation' = "verify"
  /\ mutationCommitted' = FALSE
  /\ attemptOutcome' = "failed"
  /\ result' = "pending"
  /\ UNCHANGED <<phase, actual, observed, target, desired,
    pendingOperations>>
  /\ Record("mutation-fails")

MutationOutcomeUnknown(nextState) ==
  /\ phase = "ready"
  /\ operation \in {"enable", "disable"}
  /\ nextState \in States
  /\ actual' = [actual EXCEPT ![target] = nextState]
  /\ operation' = "verify"
  /\ mutationCommitted' = TRUE
  /\ attemptOutcome' = "failed"
  /\ result' = "pending"
  /\ UNCHANGED <<phase, observed, target, desired, pendingOperations>>
  /\ Record("mutation-outcome-unknown")

Verify ==
  /\ phase = "ready"
  /\ operation = "verify"
  /\ attemptOutcome \in {"attempted", "failed"}
  /\ observed' = [observed EXCEPT ![target] = actual[target]]
  /\ operation' = "none"
  /\ attemptOutcome' = "none"
  /\ pendingOperations' = pendingOperations - 1
  /\ result' =
       IF DesiredObserved(desired, actual[target])
       THEN "completed"
       ELSE IF attemptOutcome = "attempted"
       THEN "verification-failed"
       ELSE "operation-failed"
  /\ UNCHANGED <<phase, actual, target, desired, mutationCommitted>>
  /\ Record("verify")

ReconciliationFails ==
  /\ phase = "ready"
  /\ operation = "verify"
  /\ attemptOutcome \in {"attempted", "failed"}
  /\ observed' = [observed EXCEPT ![target] = "unknown"]
  /\ operation' = "none"
  /\ attemptOutcome' = "none"
  /\ pendingOperations' = pendingOperations - 1
  /\ result' =
       IF attemptOutcome = "attempted"
       THEN "verification-failed"
       ELSE "operation-failed"
  /\ UNCHANGED <<phase, actual, target, desired, mutationCommitted>>
  /\ Record("reconciliation-fails")

RendererReload ==
  /\ phase = "ready"
  /\ UNCHANGED <<phase, actual, observed, operation, target, desired,
    mutationCommitted, attemptOutcome, pendingOperations, result>>
  /\ Record("renderer-reload")

Dispose ==
  /\ phase # "disposed"
  /\ phase' = "disposed"
  /\ operation' = "none"
  /\ attemptOutcome' = "none"
  /\ pendingOperations' = 0
  /\ result' = "none"
  /\ UNCHANGED <<actual, observed, target, desired, mutationCommitted>>
  /\ Record("dispose")

Next ==
  \/ Initialize
  \/ RequestRefresh
  \/ InspectCompletes
  \/ \E failedAdapters \in (SUBSET Adapters) \ {{}}:
       InspectFails(failedAdapters)
  \/ \E adapter \in Adapters: RequestEnable(adapter)
  \/ \E adapter \in Adapters: RequestDisable(adapter)
  \/ \E adapter \in Adapters: RejectConflict(adapter)
  \/ \E nextState \in States: CommitMutation(nextState)
  \/ MutationFails
  \/ \E nextState \in States: MutationOutcomeUnknown(nextState)
  \/ Verify
  \/ ReconciliationFails
  \/ RendererReload
  \/ Dispose

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in {"uninitialized", "ready", "disposed"}
  /\ actual \in [Adapters -> States]
  /\ observed \in [Adapters -> ObservedStates]
  /\ operation \in Operations
  /\ target \in Adapters
  /\ desired \in DesiredStates
  /\ mutationCommitted \in BOOLEAN
  /\ attemptOutcome \in AttemptOutcomes
  /\ pendingOperations \in 0..2
  /\ result \in Results
  /\ lastAction \in {
       "init", "initialize", "request-refresh", "inspect-completes",
       "inspect-fails", "request-enable", "request-disable",
       "reject-conflict", "commit-mutation", "mutation-fails",
       "mutation-outcome-unknown", "verify", "reconciliation-fails",
       "renderer-reload", "dispose"
     }
  /\ previousActual \in [Adapters -> States]
  /\ previousObserved \in [Adapters -> ObservedStates]
  /\ previousOperation \in Operations
  /\ previousTarget \in Adapters
  /\ previousDesired \in DesiredStates
  /\ previousMutationCommitted \in BOOLEAN
  /\ previousAttemptOutcome \in AttemptOutcomes
  /\ previousPendingOperations \in 0..2
  /\ previousResult \in Results

InitializationDoesNotInspect ==
  lastAction = "initialize" =>
    /\ actual = previousActual
    /\ observed = [adapter \in Adapters |-> "unknown"]
    /\ operation = "none"
    /\ pendingOperations = 0

AtMostOneAdmittedOperation ==
  /\ pendingOperations <= 1
  /\ (operation = "none" <=> pendingOperations = 0)

OperationAdmissionWasIdle ==
  lastAction \in {"request-refresh", "request-enable", "request-disable"} =>
    /\ previousOperation = "none"
    /\ previousPendingOperations = 0
    /\ pendingOperations = 1

OperationAdmissionWasAuthorized ==
  /\ (lastAction = "request-enable" =>
       /\ desired = "enabled"
       /\ previousActual[target] # "conflict"
       /\ ~DesiredObserved("enabled", previousActual[target]))
  /\ (lastAction = "request-disable" =>
       /\ desired = "disabled"
       /\ previousActual[target] # "conflict"
       /\ ~DesiredObserved("disabled", previousActual[target]))

MutationWasExplicitlyAdmitted ==
  lastAction \in {"commit-mutation", "mutation-fails", "mutation-outcome-unknown"} =>
    /\ previousOperation \in {"enable", "disable"}
    /\ previousPendingOperations = 1
    /\ target = previousTarget
    /\ desired = previousDesired
    /\ previousDesired =
         IF previousOperation = "enable" THEN "enabled" ELSE "disabled"
    /\ previousActual[previousTarget] # "conflict"
    /\ ~DesiredObserved(previousDesired, previousActual[previousTarget])

EveryAttemptIsReconciled ==
  lastAction \in {"commit-mutation", "mutation-fails", "mutation-outcome-unknown"} =>
    /\ operation = "verify"
    /\ pendingOperations = 1
    /\ result = "pending"

ActualChangesOnlyThroughRequestedMutation ==
  actual # previousActual =>
    lastAction \in {"commit-mutation", "mutation-outcome-unknown"}

ConflictIsNeverOverwritten ==
  \A adapter \in Adapters:
    previousActual[adapter] = "conflict" => actual[adapter] = "conflict"

PreCommitFailurePreservesState ==
  lastAction = "mutation-fails" =>
    /\ actual = previousActual
    /\ observed = previousObserved
    /\ previousMutationCommitted = FALSE
    /\ attemptOutcome = "failed"

SuccessfulResultWasVerified ==
  lastAction = "verify" /\ result = "completed" =>
    /\ observed[target] = actual[target]
    /\ DesiredObserved(desired, observed[target])
    /\ previousAttemptOutcome \in {"attempted", "failed"}

VerificationFailureIsTruthful ==
  lastAction = "verify" /\ result = "verification-failed" =>
    /\ observed[target] = actual[target]
    /\ ~DesiredObserved(desired, observed[target])
    /\ previousAttemptOutcome = "attempted"

OperationFailureIsTruthful ==
  lastAction = "verify" /\ result = "operation-failed" =>
    /\ observed[target] = actual[target]
    /\ ~DesiredObserved(desired, observed[target])
    /\ previousAttemptOutcome = "failed"

ReconciliationFailureIsUnknown ==
  lastAction = "reconciliation-fails" =>
    /\ observed = [previousObserved EXCEPT ![previousTarget] = "unknown"]
    /\ actual = previousActual
    /\ result =
         IF previousAttemptOutcome = "attempted"
         THEN "verification-failed"
         ELSE "operation-failed"

RefreshPublishesCurrentOrUnknown ==
  lastAction \in {"inspect-completes", "inspect-fails"} =>
    \A adapter \in Adapters:
      observed[adapter] \in {actual[adapter], "unknown"}

InspectionFailureIsVisible ==
  lastAction = "inspect-fails" =>
    \E adapter \in Adapters: observed[adapter] = "unknown"

ObservedChangesOnlyThroughInspection ==
  observed # previousObserved =>
    lastAction \in {
      "inspect-completes", "inspect-fails", "verify", "reconciliation-fails"
    }

RendererReloadPreservesManagementState ==
  lastAction = "renderer-reload" =>
    /\ actual = previousActual
    /\ observed = previousObserved
    /\ operation = previousOperation
    /\ target = previousTarget
    /\ desired = previousDesired
    /\ mutationCommitted = previousMutationCommitted
    /\ attemptOutcome = previousAttemptOutcome
    /\ pendingOperations = previousPendingOperations
    /\ result = previousResult

=============================================================================
