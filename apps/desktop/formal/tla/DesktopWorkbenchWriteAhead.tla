----------------------- MODULE DesktopWorkbenchWriteAhead -----------------------
EXTENDS Naturals, TLC

\* One abstract presentation and one reusable operation identity model the
\* write-ahead boundary. The identity is reused only when a new request starts;
\* renderer reload of a still-durable command retains its settled-result cache
\* and counters. More presentations and concurrent operation identities remain
\* outside this finite refinement bound.

Intents == {"absent", "provision", "retain", "close", "stop", "stopped"}
Commands == {"none", "provision", "restore", "close", "stop"}
CommandPhases == {"none", "queued", "dispatched", "completed"}
Actions == {"init", "request", "commit-started", "commit-succeeded",
  "commit-failed", "dispatch", "effect-completed", "result-recovered",
  "result-applied", "disposed", "reloaded"}

OperationIds == {1}
NoOperation == 0
OperationValues == OperationIds \cup {NoOperation}
OnlyOperation == 1
CounterValues == 0..2
ZeroCounters == [operation \in OperationIds |-> 0]
NoSettledCommands == [operation \in OperationIds |-> "none"]

CommandFor(intent) ==
  CASE intent = "absent" -> "none"
    [] intent = "provision" -> "provision"
    [] intent = "retain" -> "restore"
    [] intent = "close" -> "close"
    [] intent = "stop" -> "stop"
    [] intent = "stopped" -> "none"

CommandCovers(intent, command) ==
  \/ /\ intent = "provision" /\ command = "provision"
  \/ /\ intent = "retain" /\ command = "restore"
  \/ /\ intent = "close" /\ command = "close"
  \/ /\ intent = "stop" /\ command = "stop"

ResultIntent(command) ==
  CASE command = "provision" -> "retain"
    [] command = "restore" -> "retain"
    [] command = "close" -> "absent"
    [] command = "stop" -> "stopped"

VARIABLES
  storedIntent,
  memoryIntent,
  commitPending,
  commitCandidate,
  commandType,
  commandPhase,
  connected,
  writable,
  operationId,
  settledOperations,
  settledCommand,
  effectCount,
  creationCount,
  lastAction,
  lastCommand,
  lastOperation,
  previousStoredIntent,
  previousEffectCount,
  previousCreationCount

vars == <<storedIntent, memoryIntent, commitPending, commitCandidate,
  commandType, commandPhase, connected, writable, operationId,
  settledOperations, settledCommand, effectCount, creationCount,
  lastAction, lastCommand, lastOperation, previousStoredIntent,
  previousEffectCount, previousCreationCount>>

CachedResultCovers(operation, command) ==
  IF operation \in settledOperations
  THEN
    \/ settledCommand[operation] = command
    \/ /\ command = "restore"
       /\ settledCommand[operation] = "provision"
  ELSE FALSE

Record(action, command, operation) ==
  /\ lastAction' = action
  /\ lastCommand' = command
  /\ lastOperation' = operation
  /\ previousStoredIntent' = storedIntent
  /\ previousEffectCount' = effectCount
  /\ previousCreationCount' = creationCount

Init ==
  /\ storedIntent \in Intents
  /\ memoryIntent = storedIntent
  /\ commitPending = FALSE
  /\ commitCandidate = storedIntent
  /\ commandType = CommandFor(storedIntent)
  /\ commandPhase = IF commandType = "none" THEN "none" ELSE "queued"
  /\ connected = TRUE
  /\ writable \in BOOLEAN
  /\ operationId = IF commandType = "none" THEN NoOperation ELSE OnlyOperation
  /\ settledOperations = {}
  /\ settledCommand = NoSettledCommands
  /\ effectCount = ZeroCounters
  /\ creationCount = ZeroCounters
  /\ lastAction = "init"
  /\ lastCommand = "none"
  /\ lastOperation = NoOperation
  /\ previousStoredIntent = storedIntent
  /\ previousEffectCount = effectCount
  /\ previousCreationCount = creationCount

RequestProvision ==
  /\ connected
  /\ ~commitPending
  /\ storedIntent = "absent"
  /\ memoryIntent = storedIntent
  /\ commandPhase = "none"
  /\ memoryIntent' = "provision"
  /\ commandType' = "provision"
  /\ commandPhase' = "queued"
  /\ operationId' = OnlyOperation
  /\ settledOperations' = {}
  /\ settledCommand' = NoSettledCommands
  /\ effectCount' = ZeroCounters
  /\ creationCount' = ZeroCounters
  /\ UNCHANGED <<storedIntent, commitPending, commitCandidate, connected, writable>>
  /\ Record("request", "provision", OnlyOperation)

RequestClose ==
  /\ connected
  /\ ~commitPending
  /\ storedIntent = "retain"
  /\ memoryIntent = storedIntent
  /\ commandPhase = "none"
  /\ memoryIntent' = "close"
  /\ commandType' = "close"
  /\ commandPhase' = "queued"
  /\ operationId' = OnlyOperation
  /\ settledOperations' = {}
  /\ settledCommand' = NoSettledCommands
  /\ effectCount' = ZeroCounters
  /\ creationCount' = ZeroCounters
  /\ UNCHANGED <<storedIntent, commitPending, commitCandidate, connected, writable>>
  /\ Record("request", "close", OnlyOperation)

RequestStop ==
  /\ connected
  /\ ~commitPending
  /\ storedIntent = "retain"
  /\ memoryIntent = storedIntent
  /\ commandPhase = "none"
  /\ memoryIntent' = "stop"
  /\ commandType' = "stop"
  /\ commandPhase' = "queued"
  /\ operationId' = OnlyOperation
  /\ settledOperations' = {}
  /\ settledCommand' = NoSettledCommands
  /\ effectCount' = ZeroCounters
  /\ creationCount' = ZeroCounters
  /\ UNCHANGED <<storedIntent, commitPending, commitCandidate, connected, writable>>
  /\ Record("request", "stop", OnlyOperation)

StartCommit ==
  /\ connected
  /\ ~commitPending
  /\ memoryIntent # storedIntent
  /\ commitPending' = TRUE
  /\ commitCandidate' = memoryIntent
  /\ UNCHANGED <<storedIntent, memoryIntent, commandType, commandPhase,
       connected, writable, operationId, settledOperations, settledCommand,
       effectCount, creationCount>>
  /\ Record("commit-started", commandType, operationId)

CommitSucceeded ==
  /\ commitPending
  /\ writable
  /\ storedIntent' = commitCandidate
  /\ commitPending' = FALSE
  /\ UNCHANGED <<memoryIntent, commitCandidate, commandType, commandPhase,
       connected, writable, operationId, settledOperations, settledCommand,
       effectCount, creationCount>>
  /\ Record("commit-succeeded", commandType, operationId)

CommitFailed ==
  /\ commitPending
  /\ ~writable
  /\ commitPending' = FALSE
  /\ UNCHANGED <<storedIntent, memoryIntent, commitCandidate, commandType,
       commandPhase, connected, writable, operationId, settledOperations,
       settledCommand, effectCount, creationCount>>
  /\ Record("commit-failed", commandType, operationId)

Dispatch ==
  /\ connected
  /\ commandPhase = "queued"
  /\ operationId \in OperationIds
  /\ CommandCovers(storedIntent, commandType)
  /\ ~CachedResultCovers(operationId, commandType)
  /\ commandPhase' = "dispatched"
  /\ UNCHANGED <<storedIntent, memoryIntent, commitPending, commitCandidate,
       commandType, connected, writable, operationId, settledOperations,
       settledCommand, effectCount, creationCount>>
  /\ Record("dispatch", commandType, operationId)

CompleteEffect ==
  /\ commandPhase = "dispatched"
  /\ operationId \in OperationIds
  /\ commandPhase' = "completed"
  /\ settledOperations' = settledOperations \cup {operationId}
  /\ settledCommand' =
       [settledCommand EXCEPT ![operationId] = commandType]
  /\ effectCount' =
       [effectCount EXCEPT ![operationId] = @ + 1]
  /\ creationCount' =
       IF commandType = "provision"
       THEN [creationCount EXCEPT ![operationId] = @ + 1]
       ELSE creationCount
  /\ UNCHANGED <<storedIntent, memoryIntent, commitPending, commitCandidate,
       commandType, connected, writable, operationId>>
  /\ Record("effect-completed", commandType, operationId)

RecoverSettledResult ==
  /\ connected
  /\ commandPhase = "queued"
  /\ operationId \in OperationIds
  /\ CommandCovers(storedIntent, commandType)
  /\ CachedResultCovers(operationId, commandType)
  /\ commandPhase' = "completed"
  /\ UNCHANGED <<storedIntent, memoryIntent, commitPending, commitCandidate,
       commandType, connected, writable, operationId, settledOperations,
       settledCommand, effectCount, creationCount>>
  /\ Record("result-recovered", commandType, operationId)

ApplyResult ==
  /\ connected
  /\ ~commitPending
  /\ commandPhase = "completed"
  /\ memoryIntent' = ResultIntent(commandType)
  /\ commandType' = "none"
  /\ commandPhase' = "none"
  /\ UNCHANGED <<storedIntent, commitPending, commitCandidate, connected,
       writable, operationId, settledOperations, settledCommand, effectCount,
       creationCount>>
  /\ Record("result-applied", commandType, operationId)

Dispose ==
  /\ connected
  /\ connected' = FALSE
  /\ UNCHANGED <<storedIntent, memoryIntent, commitPending, commitCandidate,
       commandType, commandPhase, writable, operationId, settledOperations,
       settledCommand, effectCount, creationCount>>
  /\ Record("disposed", commandType, operationId)

Reload ==
  /\ ~connected
  /\ ~commitPending
  /\ connected' = TRUE
  /\ memoryIntent' = storedIntent
  /\ commandType' = CommandFor(storedIntent)
  /\ commandPhase' = IF commandType' = "none" THEN "none" ELSE "queued"
  /\ operationId' = IF commandType' = "none" THEN NoOperation ELSE OnlyOperation
  /\ settledOperations' =
       IF commandType' = "none" THEN {} ELSE settledOperations
  /\ settledCommand' =
       IF commandType' = "none" THEN NoSettledCommands ELSE settledCommand
  /\ effectCount' =
       IF commandType' = "none" THEN ZeroCounters ELSE effectCount
  /\ creationCount' =
       IF commandType' = "none" THEN ZeroCounters ELSE creationCount
  /\ UNCHANGED <<storedIntent, commitPending, commitCandidate, writable>>
  /\ Record("reloaded", commandType', operationId')

Next ==
  \/ RequestProvision
  \/ RequestClose
  \/ RequestStop
  \/ StartCommit
  \/ CommitSucceeded
  \/ CommitFailed
  \/ Dispatch
  \/ CompleteEffect
  \/ RecoverSettledResult
  \/ ApplyResult
  \/ Dispose
  \/ Reload

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(StartCommit)
  /\ WF_vars(CommitSucceeded)
  /\ WF_vars(Dispatch)
  /\ WF_vars(CompleteEffect)
  /\ WF_vars(RecoverSettledResult)
  /\ WF_vars(ApplyResult)

TypeOK ==
  /\ storedIntent \in Intents
  /\ memoryIntent \in Intents
  /\ commitPending \in BOOLEAN
  /\ commitCandidate \in Intents
  /\ commandType \in Commands
  /\ commandPhase \in CommandPhases
  /\ connected \in BOOLEAN
  /\ writable \in BOOLEAN
  /\ operationId \in OperationValues
  /\ settledOperations \subseteq OperationIds
  /\ settledCommand \in [OperationIds -> Commands]
  /\ effectCount \in [OperationIds -> CounterValues]
  /\ creationCount \in [OperationIds -> CounterValues]
  /\ lastAction \in Actions
  /\ lastCommand \in Commands
  /\ lastOperation \in OperationValues
  /\ previousStoredIntent \in Intents
  /\ previousEffectCount \in [OperationIds -> CounterValues]
  /\ previousCreationCount \in [OperationIds -> CounterValues]

CommandPhaseIntegrity ==
  /\ (commandType = "none") <=> (commandPhase = "none")
  /\ commandPhase # "none" => operationId \in OperationIds

ResultCacheIntegrity ==
  /\ \A operation \in OperationIds:
       (operation \in settledOperations) <=>
         (settledCommand[operation] # "none" /\ effectCount[operation] > 0)
  /\ \A operation \in OperationIds:
       creationCount[operation] <= effectCount[operation]

DispatchRequiresDurableIntent ==
  lastAction = "dispatch" =>
    /\ CommandCovers(previousStoredIntent, lastCommand)
    /\ ~CachedResultCovers(lastOperation, lastCommand)

FailedCommitDoesNotChangeStore ==
  lastAction = "commit-failed" => storedIntent = previousStoredIntent

EffectCompletionAccounting ==
  lastAction = "effect-completed" =>
    /\ lastOperation \in OperationIds
    /\ effectCount[lastOperation] = previousEffectCount[lastOperation] + 1
    /\ creationCount[lastOperation] =
         previousCreationCount[lastOperation] +
           (IF lastCommand = "provision" THEN 1 ELSE 0)

RecoveredResultDoesNotRepeatEffect ==
  lastAction = "result-recovered" =>
    /\ lastOperation \in OperationIds
    /\ CachedResultCovers(lastOperation, lastCommand)
    /\ effectCount = previousEffectCount
    /\ creationCount = previousCreationCount

EffectsExecuteAtMostOnce ==
  \A operation \in OperationIds: effectCount[operation] <= 1

OnlyProvisionCreates ==
  \A operation \in OperationIds:
    creationCount[operation] > 0 => settledCommand[operation] = "provision"

ProvisionCreatesExactlyOne ==
  \A operation \in OperationIds:
    settledCommand[operation] = "provision" =>
      /\ operation \in settledOperations
      /\ effectCount[operation] = 1
      /\ creationCount[operation] = 1

QueuedWhileLiveEventuallySettles ==
  connected /\ writable /\ commandPhase = "queued" ~>
    commandPhase = "completed" \/ ~connected

CachedRetryEventuallyRecovers ==
  connected /\ commandPhase = "queued" /\
    CachedResultCovers(operationId, commandType) ~>
      commandPhase = "completed" \/ ~connected

=============================================================================
