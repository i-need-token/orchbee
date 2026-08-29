---------------- MODULE DesktopAgentIntegrationManagement ----------------
EXTENDS TLC

Adapters == {"claude-code", "codex"}
States == {"not-installed", "disabled", "enabled", "conflict"}
ObservedStates == States \cup {"unknown"}
Operations == {"none", "inspect", "enable", "disable", "verify"}
Results == {
  "none", "pending", "completed", "conflict", "inspection-failed",
  "operation-failed", "verification-failed"
}

VARIABLES
  phase,
  actual,
  observed,
  operation,
  target,
  desired,
  mutationCommitted,
  result,
  lastAction,
  previousActual,
  previousObserved,
  previousOperation,
  previousTarget,
  previousDesired,
  previousMutationCommitted,
  previousResult

vars == <<phase, actual, observed, operation, target, desired,
  mutationCommitted, result, lastAction, previousActual, previousObserved,
  previousOperation, previousTarget, previousDesired,
  previousMutationCommitted, previousResult>>

Record(action) ==
  /\ lastAction' = action
  /\ previousActual' = actual
  /\ previousObserved' = observed
  /\ previousOperation' = operation
  /\ previousTarget' = target
  /\ previousDesired' = desired
  /\ previousMutationCommitted' = mutationCommitted
  /\ previousResult' = result

Init ==
  /\ phase = "uninitialized"
  /\ actual \in [Adapters -> States]
  /\ observed = [adapter \in Adapters |-> "unknown"]
  /\ operation = "none"
  /\ target = "claude-code"
  /\ desired = "enabled"
  /\ mutationCommitted = FALSE
  /\ result = "none"
  /\ lastAction = "init"
  /\ previousActual = actual
  /\ previousObserved = observed
  /\ previousOperation = operation
  /\ previousTarget = target
  /\ previousDesired = desired
  /\ previousMutationCommitted = mutationCommitted
  /\ previousResult = result

Initialize ==
  /\ phase = "uninitialized"
  /\ phase' = "ready"
  /\ operation' = "inspect"
  /\ mutationCommitted' = FALSE
  /\ result' = "pending"
  /\ UNCHANGED <<actual, observed, target, desired>>
  /\ Record("initialize")

RequestRefresh ==
  /\ phase = "ready"
  /\ operation = "none"
  /\ operation' = "inspect"
  /\ mutationCommitted' = FALSE
  /\ result' = "pending"
  /\ UNCHANGED <<phase, actual, observed, target, desired>>
  /\ Record("request-refresh")

InspectCompletes ==
  /\ phase = "ready"
  /\ operation = "inspect"
  /\ observed' = actual
  /\ operation' = "none"
  /\ result' = "completed"
  /\ UNCHANGED <<phase, actual, target, desired, mutationCommitted>>
  /\ Record("inspect-completes")

InspectFails ==
  /\ phase = "ready"
  /\ operation = "inspect"
  /\ operation' = "none"
  /\ result' = "inspection-failed"
  /\ UNCHANGED <<phase, actual, observed, target, desired, mutationCommitted>>
  /\ Record("inspect-fails")

RequestEnable(adapter) ==
  /\ phase = "ready"
  /\ operation = "none"
  /\ adapter \in Adapters
  /\ actual[adapter] # "conflict"
  /\ operation' = "enable"
  /\ target' = adapter
  /\ desired' = "enabled"
  /\ mutationCommitted' = FALSE
  /\ result' = "pending"
  /\ UNCHANGED <<phase, actual, observed>>
  /\ Record("request-enable")

RequestDisable(adapter) ==
  /\ phase = "ready"
  /\ operation = "none"
  /\ adapter \in Adapters
  /\ actual[adapter] # "conflict"
  /\ operation' = "disable"
  /\ target' = adapter
  /\ desired' = "disabled"
  /\ mutationCommitted' = FALSE
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
  /\ UNCHANGED <<phase, actual, observed, operation, desired, mutationCommitted>>
  /\ Record("reject-conflict")

CommitMutation ==
  /\ phase = "ready"
  /\ operation \in {"enable", "disable"}
  /\ actual[target] # "conflict"
  /\ actual' = [actual EXCEPT ![target] = desired]
  /\ operation' = "verify"
  /\ mutationCommitted' = TRUE
  /\ result' = "pending"
  /\ UNCHANGED <<phase, observed, target, desired>>
  /\ Record("commit-mutation")

MutationFails ==
  /\ phase = "ready"
  /\ operation \in {"enable", "disable"}
  /\ operation' = "none"
  /\ result' = "operation-failed"
  /\ UNCHANGED <<phase, actual, observed, target, desired, mutationCommitted>>
  /\ Record("mutation-fails")

MutationOutcomeUnknown(nextState) ==
  /\ phase = "ready"
  /\ operation \in {"enable", "disable"}
  /\ nextState \in States
  /\ actual[target] # "conflict"
  /\ actual' = [actual EXCEPT ![target] = nextState]
  /\ operation' = "verify"
  /\ mutationCommitted' = TRUE
  /\ result' = "pending"
  /\ UNCHANGED <<phase, observed, target, desired>>
  /\ Record("mutation-outcome-unknown")

Verify ==
  /\ phase = "ready"
  /\ operation = "verify"
  /\ observed' = [observed EXCEPT ![target] = actual[target]]
  /\ operation' = "none"
  /\ result' = IF actual[target] = desired
       THEN "completed"
       ELSE "verification-failed"
  /\ UNCHANGED <<phase, actual, target, desired, mutationCommitted>>
  /\ Record("verify")

RendererReload ==
  /\ phase = "ready"
  /\ UNCHANGED <<phase, actual, observed, operation, target, desired,
    mutationCommitted, result>>
  /\ Record("renderer-reload")

Dispose ==
  /\ phase # "disposed"
  /\ phase' = "disposed"
  /\ operation' = "none"
  /\ result' = "none"
  /\ UNCHANGED <<actual, observed, target, desired, mutationCommitted>>
  /\ Record("dispose")

Next ==
  \/ Initialize
  \/ RequestRefresh
  \/ InspectCompletes
  \/ InspectFails
  \/ \E adapter \in Adapters: RequestEnable(adapter)
  \/ \E adapter \in Adapters: RequestDisable(adapter)
  \/ \E adapter \in Adapters: RejectConflict(adapter)
  \/ CommitMutation
  \/ MutationFails
  \/ \E nextState \in States: MutationOutcomeUnknown(nextState)
  \/ Verify
  \/ RendererReload
  \/ Dispose

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in {"uninitialized", "ready", "disposed"}
  /\ actual \in [Adapters -> States]
  /\ observed \in [Adapters -> ObservedStates]
  /\ operation \in Operations
  /\ target \in Adapters
  /\ desired \in {"enabled", "disabled"}
  /\ mutationCommitted \in BOOLEAN
  /\ result \in Results
  /\ lastAction \in {
       "init", "initialize", "request-refresh", "inspect-completes",
       "inspect-fails", "request-enable", "request-disable",
       "reject-conflict", "commit-mutation", "mutation-fails",
       "mutation-outcome-unknown", "verify", "renderer-reload", "dispose"
     }
  /\ previousActual \in [Adapters -> States]
  /\ previousObserved \in [Adapters -> ObservedStates]
  /\ previousOperation \in Operations
  /\ previousTarget \in Adapters
  /\ previousDesired \in {"enabled", "disabled"}
  /\ previousMutationCommitted \in BOOLEAN
  /\ previousResult \in Results

ActualChangesOnlyThroughRequestedMutation ==
  actual # previousActual =>
    lastAction \in {"commit-mutation", "mutation-outcome-unknown"}

ConflictIsNeverOverwritten ==
  previousActual[target] = "conflict" => actual[target] = "conflict"

PreCommitFailurePreservesState ==
  lastAction = "mutation-fails" =>
    /\ actual = previousActual
    /\ observed = previousObserved
    /\ previousMutationCommitted = FALSE

SuccessfulResultWasVerified ==
  lastAction = "verify" /\ result = "completed" =>
    /\ observed[target] = desired
    /\ actual[target] = desired

VerificationFailureIsTruthful ==
  lastAction = "verify" /\ result = "verification-failed" =>
    /\ observed[target] = actual[target]
    /\ actual[target] # desired

RendererReloadPreservesManagementState ==
  lastAction = "renderer-reload" =>
    /\ actual = previousActual
    /\ observed = previousObserved
    /\ operation = previousOperation
    /\ result = previousResult

=============================================================================
