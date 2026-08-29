---------------- MODULE SemanticCommandExecutionLedger ----------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

\* Bounded abstraction for Desktop's process-lifetime semantic-command
\* execution ledger. Feature input validation, operation effects, encoded
\* transport, and durable restart recovery remain outside this model.

CONSTANT Requests, Fingerprints, Executions, HeavyExecutions,
  MaxActive, MaxTerminalRecords, MaxTerminalWeight, MaxGeneration

NoFingerprint == "none"
NoExecution == "none"
NoRequest == "none"
NoResult == "none"

Phases == {"absent", "active", "terminal"}
Results == {"completed", "unchanged", "blocked", "cancelled", "failed", "unsupported"}
UncommittedResults == Results
Actions == {
  "init", "reject-invalid", "reject-unauthorized", "reject-capacity",
  "admit", "replay-same", "reject-conflict", "read-retained", "read-missing",
  "request-cancellation", "repeat-cancellation", "cancel-terminal",
  "cancel-committed",
  "mark-committed", "complete-uncommitted", "complete-committed",
  "disconnect", "reconnect", "restart"
}

VARIABLES
  requestFingerprint,
  requestExecution,
  executionRequest,
  executionPhase,
  terminalResult,
  cancellationForwards,
  commitAttempts,
  handlerStarts,
  usedExecutions,
  terminalOrder,
  connected,
  generation,
  lastAction

vars == <<requestFingerprint, requestExecution, executionRequest,
  executionPhase, terminalResult, cancellationForwards, commitAttempts,
  handlerStarts, usedExecutions, terminalOrder, connected, generation,
  lastAction>>

ActiveExecutions == {execution \in Executions : executionPhase[execution] = "active"}
TerminalExecutions == {execution \in Executions : executionPhase[execution] = "terminal"}
ResultWeight(execution) == IF execution \in HeavyExecutions THEN 2 ELSE 1

SequenceSet(sequence) == {sequence[index] : index \in 1..Len(sequence)}

RECURSIVE SequenceWeight(_)
SequenceWeight(sequence) ==
  IF Len(sequence) = 0
  THEN 0
  ELSE ResultWeight(Head(sequence)) + SequenceWeight(Tail(sequence))

RECURSIVE TrimTerminalOrder(_)
TrimTerminalOrder(sequence) ==
  IF Len(sequence) <= MaxTerminalRecords /\ SequenceWeight(sequence) <= MaxTerminalWeight
  THEN sequence
  ELSE TrimTerminalOrder(Tail(sequence))

Init ==
  /\ requestFingerprint = [request \in Requests |-> NoFingerprint]
  /\ requestExecution = [request \in Requests |-> NoExecution]
  /\ executionRequest = [execution \in Executions |-> NoRequest]
  /\ executionPhase = [execution \in Executions |-> "absent"]
  /\ terminalResult = [execution \in Executions |-> NoResult]
  /\ cancellationForwards = [execution \in Executions |-> 0]
  /\ commitAttempts = [execution \in Executions |-> 0]
  /\ handlerStarts = [request \in Requests |-> 0]
  /\ usedExecutions = {}
  /\ terminalOrder = <<>>
  /\ connected = TRUE
  /\ generation = 1
  /\ lastAction = "init"

RejectInvalid(request) ==
  /\ request \in Requests
  /\ requestExecution[request] = NoExecution
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "reject-invalid"

RejectUnauthorized(request) ==
  /\ request \in Requests
  /\ requestExecution[request] = NoExecution
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "reject-unauthorized"

RejectCapacity(request, fingerprint) ==
  /\ request \in Requests
  /\ fingerprint \in Fingerprints
  /\ requestExecution[request] = NoExecution
  /\ Cardinality(ActiveExecutions) >= MaxActive
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "reject-capacity"

Admit(request, fingerprint, execution) ==
  /\ request \in Requests
  /\ fingerprint \in Fingerprints
  /\ execution \in Executions
  /\ requestExecution[request] = NoExecution
  /\ Cardinality(ActiveExecutions) < MaxActive
  /\ execution \notin usedExecutions
  /\ requestFingerprint' = [requestFingerprint EXCEPT ![request] = fingerprint]
  /\ requestExecution' = [requestExecution EXCEPT ![request] = execution]
  /\ executionRequest' = [executionRequest EXCEPT ![execution] = request]
  /\ executionPhase' = [executionPhase EXCEPT ![execution] = "active"]
  /\ handlerStarts' = [handlerStarts EXCEPT ![request] = @ + 1]
  /\ usedExecutions' = usedExecutions \cup {execution}
  /\ UNCHANGED <<terminalResult, cancellationForwards, commitAttempts,
       terminalOrder, connected, generation>>
  /\ lastAction' = "admit"

ReplaySame(request, fingerprint) ==
  /\ request \in Requests
  /\ fingerprint \in Fingerprints
  /\ requestExecution[request] # NoExecution
  /\ requestFingerprint[request] = fingerprint
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "replay-same"

RejectConflict(request, fingerprint) ==
  /\ request \in Requests
  /\ fingerprint \in Fingerprints
  /\ requestExecution[request] # NoExecution
  /\ requestFingerprint[request] # fingerprint
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "reject-conflict"

ReadRetained(request) ==
  /\ request \in Requests
  /\ requestExecution[request] # NoExecution
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "read-retained"

ReadMissing(request) ==
  /\ request \in Requests
  /\ requestExecution[request] = NoExecution
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "read-missing"

RequestCancellation(execution) ==
  /\ execution \in ActiveExecutions
  /\ commitAttempts[execution] = 0
  /\ cancellationForwards[execution] = 0
  /\ cancellationForwards' = [cancellationForwards EXCEPT ![execution] = 1]
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, commitAttempts, handlerStarts,
       usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "request-cancellation"

RepeatCancellation(execution) ==
  /\ execution \in ActiveExecutions
  /\ commitAttempts[execution] = 0
  /\ cancellationForwards[execution] = 1
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "repeat-cancellation"

CancelTerminal(execution) ==
  /\ execution \in TerminalExecutions
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "cancel-terminal"

CancelCommitted(execution) ==
  /\ execution \in ActiveExecutions
  /\ commitAttempts[execution] = 1
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "cancel-committed"

MarkCommitted(execution) ==
  /\ execution \in ActiveExecutions
  /\ commitAttempts[execution] = 0
  /\ commitAttempts' = [commitAttempts EXCEPT ![execution] = 1]
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, handlerStarts,
       usedExecutions, terminalOrder, connected, generation>>
  /\ lastAction' = "mark-committed"

FinishExecution(execution, result) ==
  LET retainedOrder == TrimTerminalOrder(Append(terminalOrder, execution))
      retained == SequenceSet(retainedOrder)
      discarded == (TerminalExecutions \cup {execution}) \ retained
  IN
    /\ terminalOrder' = retainedOrder
    /\ requestFingerprint' =
         [request \in Requests |->
           IF requestExecution[request] \in discarded
           THEN NoFingerprint
           ELSE requestFingerprint[request]]
    /\ requestExecution' =
         [request \in Requests |->
           IF requestExecution[request] \in discarded
           THEN NoExecution
           ELSE requestExecution[request]]
    /\ executionRequest' =
         [candidate \in Executions |->
           IF candidate \in discarded
           THEN NoRequest
           ELSE executionRequest[candidate]]
    /\ executionPhase' =
         [candidate \in Executions |->
           IF candidate = execution
           THEN IF candidate \in retained THEN "terminal" ELSE "absent"
           ELSE IF candidate \in discarded THEN "absent" ELSE executionPhase[candidate]]
    /\ terminalResult' =
         [candidate \in Executions |->
           IF candidate = execution
           THEN IF candidate \in retained THEN result ELSE NoResult
           ELSE IF candidate \in discarded THEN NoResult ELSE terminalResult[candidate]]
    /\ cancellationForwards' =
         [candidate \in Executions |->
           IF candidate \in discarded THEN 0 ELSE cancellationForwards[candidate]]
    /\ commitAttempts' =
         [candidate \in Executions |->
           IF candidate \in discarded THEN 0 ELSE commitAttempts[candidate]]
    /\ handlerStarts' =
         [request \in Requests |->
           IF requestExecution[request] \in discarded THEN 0 ELSE handlerStarts[request]]
    /\ UNCHANGED <<usedExecutions, connected, generation>>

CompleteUncommitted(execution, result) ==
  /\ execution \in ActiveExecutions
  /\ result \in UncommittedResults
  /\ commitAttempts[execution] = 0
  /\ FinishExecution(execution, result)
  /\ lastAction' = "complete-uncommitted"

CompleteCommitted(execution) ==
  /\ execution \in ActiveExecutions
  /\ commitAttempts[execution] = 1
  /\ FinishExecution(execution, "completed")
  /\ lastAction' = "complete-committed"

Disconnect ==
  /\ connected
  /\ connected' = FALSE
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, generation>>
  /\ lastAction' = "disconnect"

Reconnect ==
  /\ ~connected
  /\ connected' = TRUE
  /\ UNCHANGED <<requestFingerprint, requestExecution, executionRequest,
       executionPhase, terminalResult, cancellationForwards, commitAttempts,
       handlerStarts, usedExecutions, terminalOrder, generation>>
  /\ lastAction' = "reconnect"

Restart ==
  /\ generation < MaxGeneration
  /\ requestFingerprint' = [request \in Requests |-> NoFingerprint]
  /\ requestExecution' = [request \in Requests |-> NoExecution]
  /\ executionRequest' = [execution \in Executions |-> NoRequest]
  /\ executionPhase' = [execution \in Executions |-> "absent"]
  /\ terminalResult' = [execution \in Executions |-> NoResult]
  /\ cancellationForwards' = [execution \in Executions |-> 0]
  /\ commitAttempts' = [execution \in Executions |-> 0]
  /\ handlerStarts' = [request \in Requests |-> 0]
  /\ usedExecutions' = {}
  /\ terminalOrder' = <<>>
  /\ generation' = generation + 1
  /\ UNCHANGED connected
  /\ lastAction' = "restart"

Next ==
  \/ \E request \in Requests: RejectInvalid(request)
  \/ \E request \in Requests: RejectUnauthorized(request)
  \/ \E request \in Requests, fingerprint \in Fingerprints:
       RejectCapacity(request, fingerprint)
  \/ \E request \in Requests, fingerprint \in Fingerprints,
       execution \in Executions: Admit(request, fingerprint, execution)
  \/ \E request \in Requests, fingerprint \in Fingerprints: ReplaySame(request, fingerprint)
  \/ \E request \in Requests, fingerprint \in Fingerprints:
       RejectConflict(request, fingerprint)
  \/ \E request \in Requests: ReadRetained(request)
  \/ \E request \in Requests: ReadMissing(request)
  \/ \E execution \in Executions: RequestCancellation(execution)
  \/ \E execution \in Executions: RepeatCancellation(execution)
  \/ \E execution \in Executions: CancelTerminal(execution)
  \/ \E execution \in Executions: CancelCommitted(execution)
  \/ \E execution \in Executions: MarkCommitted(execution)
  \/ \E execution \in Executions, result \in UncommittedResults:
       CompleteUncommitted(execution, result)
  \/ \E execution \in Executions: CompleteCommitted(execution)
  \/ Disconnect
  \/ Reconnect
  \/ Restart

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ HeavyExecutions \subseteq Executions
  /\ requestFingerprint \in [Requests -> Fingerprints \cup {NoFingerprint}]
  /\ requestExecution \in [Requests -> Executions \cup {NoExecution}]
  /\ executionRequest \in [Executions -> Requests \cup {NoRequest}]
  /\ executionPhase \in [Executions -> Phases]
  /\ terminalResult \in [Executions -> Results \cup {NoResult}]
  /\ cancellationForwards \in [Executions -> 0..1]
  /\ commitAttempts \in [Executions -> 0..1]
  /\ handlerStarts \in [Requests -> 0..1]
  /\ usedExecutions \subseteq Executions
  /\ terminalOrder \in Seq(Executions)
  /\ connected \in BOOLEAN
  /\ generation \in 1..MaxGeneration
  /\ lastAction \in Actions

RequestAndExecutionMappingsAgree ==
  /\ \A request \in Requests:
       requestExecution[request] = NoExecution
       <=> requestFingerprint[request] = NoFingerprint
  /\ \A request \in Requests:
       requestExecution[request] # NoExecution
       => /\ executionRequest[requestExecution[request]] = request
          /\ executionPhase[requestExecution[request]] # "absent"
  /\ \A execution \in Executions:
       executionPhase[execution] # "absent"
       => /\ executionRequest[execution] # NoRequest
          /\ requestExecution[executionRequest[execution]] = execution

RetainedRequestHasOneHandlerStart ==
  \A request \in Requests:
    requestExecution[request] # NoExecution <=> handlerStarts[request] = 1

ExecutionIdentityIsProcessUnique ==
  \A execution \in Executions:
    executionPhase[execution] # "absent" => execution \in usedExecutions

ActiveExecutionCapacity == Cardinality(ActiveExecutions) <= MaxActive

TerminalRetentionCapacity ==
  /\ Len(terminalOrder) <= MaxTerminalRecords
  /\ SequenceWeight(terminalOrder) <= MaxTerminalWeight

TerminalOrderMatchesState ==
  /\ SequenceSet(terminalOrder) = TerminalExecutions
  /\ Len(terminalOrder) = Cardinality(TerminalExecutions)

ExecutionResultMatchesPhase ==
  \A execution \in Executions:
    terminalResult[execution] # NoResult <=> executionPhase[execution] = "terminal"

CancellationForwardedAtMostOnce ==
  \A execution \in Executions: cancellationForwards[execution] <= 1

CommittedExecutionNeverCancels ==
  \A execution \in Executions:
    terminalResult[execution] = "cancelled" => commitAttempts[execution] = 0

ActiveExecutionsAreNotTerminalRecords ==
  ActiveExecutions \cap SequenceSet(terminalOrder) = {}

=============================================================================
