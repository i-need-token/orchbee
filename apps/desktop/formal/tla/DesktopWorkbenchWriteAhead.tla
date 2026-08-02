----------------------- MODULE DesktopWorkbenchWriteAhead -----------------------
EXTENDS Naturals, TLC

Intents == {"absent", "provision", "retain", "close"}
Commands == {"none", "provision", "restore", "close"}
CommandPhases == {"none", "queued", "dispatched", "completed"}
Actions == {"init", "request", "commit-started", "commit-succeeded",
  "commit-failed", "dispatch", "effect-completed", "result-applied",
  "disposed", "reloaded"}

CommandFor(intent) ==
  CASE intent = "absent" -> "none"
    [] intent = "provision" -> "provision"
    [] intent = "retain" -> "restore"
    [] intent = "close" -> "close"

CommandCovers(intent, command) ==
  \/ /\ intent = "provision" /\ command = "provision"
  \/ /\ intent = "retain" /\ command = "restore"
  \/ /\ intent = "close" /\ command = "close"

ResultIntent(command) ==
  CASE command = "provision" -> "retain"
    [] command = "restore" -> "retain"
    [] command = "close" -> "absent"

VARIABLES
  storedIntent,
  memoryIntent,
  commitPending,
  commitCandidate,
  commandType,
  commandPhase,
  connected,
  writable,
  creationParity,
  lastAction,
  lastCommand,
  previousStoredIntent,
  previousCreationParity

vars == <<storedIntent, memoryIntent, commitPending, commitCandidate,
  commandType, commandPhase, connected, writable, creationParity,
  lastAction, lastCommand, previousStoredIntent, previousCreationParity>>

Record(action, command) ==
  /\ lastAction' = action
  /\ lastCommand' = command
  /\ previousStoredIntent' = storedIntent
  /\ previousCreationParity' = creationParity

Init ==
  /\ storedIntent \in Intents
  /\ memoryIntent = storedIntent
  /\ commitPending = FALSE
  /\ commitCandidate = storedIntent
  /\ commandType = CommandFor(storedIntent)
  /\ commandPhase = IF commandType = "none" THEN "none" ELSE "queued"
  /\ connected = TRUE
  /\ writable \in BOOLEAN
  /\ creationParity = FALSE
  /\ lastAction = "init"
  /\ lastCommand = "none"
  /\ previousStoredIntent = storedIntent
  /\ previousCreationParity = creationParity

RequestProvision ==
  /\ connected
  /\ ~commitPending
  /\ storedIntent = "absent"
  /\ memoryIntent = storedIntent
  /\ commandPhase = "none"
  /\ memoryIntent' = "provision"
  /\ commandType' = "provision"
  /\ commandPhase' = "queued"
  /\ UNCHANGED <<storedIntent, commitPending, commitCandidate, connected,
       writable, creationParity>>
  /\ Record("request", "provision")

RequestClose ==
  /\ connected
  /\ ~commitPending
  /\ storedIntent = "retain"
  /\ memoryIntent = storedIntent
  /\ commandPhase = "none"
  /\ memoryIntent' = "close"
  /\ commandType' = "close"
  /\ commandPhase' = "queued"
  /\ UNCHANGED <<storedIntent, commitPending, commitCandidate, connected,
       writable, creationParity>>
  /\ Record("request", "close")

StartCommit ==
  /\ connected
  /\ ~commitPending
  /\ memoryIntent # storedIntent
  /\ commitPending' = TRUE
  /\ commitCandidate' = memoryIntent
  /\ UNCHANGED <<storedIntent, memoryIntent, commandType, commandPhase,
       connected, writable, creationParity>>
  /\ Record("commit-started", commandType)

CommitSucceeded ==
  /\ commitPending
  /\ writable
  /\ storedIntent' = commitCandidate
  /\ commitPending' = FALSE
  /\ UNCHANGED <<memoryIntent, commitCandidate, commandType, commandPhase,
       connected, writable, creationParity>>
  /\ Record("commit-succeeded", commandType)

CommitFailed ==
  /\ commitPending
  /\ ~writable
  /\ commitPending' = FALSE
  /\ UNCHANGED <<storedIntent, memoryIntent, commitCandidate, commandType,
       commandPhase, connected, writable, creationParity>>
  /\ Record("commit-failed", commandType)

Dispatch ==
  /\ connected
  /\ commandPhase = "queued"
  /\ CommandCovers(storedIntent, commandType)
  /\ commandPhase' = "dispatched"
  /\ UNCHANGED <<storedIntent, memoryIntent, commitPending, commitCandidate,
       commandType, connected, writable, creationParity>>
  /\ Record("dispatch", commandType)

CompleteEffect ==
  /\ commandPhase = "dispatched"
  /\ commandPhase' = "completed"
  /\ creationParity' =
       IF commandType = "provision" THEN ~creationParity ELSE creationParity
  /\ UNCHANGED <<storedIntent, memoryIntent, commitPending, commitCandidate,
       commandType, connected, writable>>
  /\ Record("effect-completed", commandType)

ApplyResult ==
  /\ connected
  /\ ~commitPending
  /\ commandPhase = "completed"
  /\ memoryIntent' = ResultIntent(commandType)
  /\ commandType' = "none"
  /\ commandPhase' = "none"
  /\ UNCHANGED <<storedIntent, commitPending, commitCandidate, connected,
       writable, creationParity>>
  /\ Record("result-applied", commandType)

Dispose ==
  /\ connected
  /\ connected' = FALSE
  /\ UNCHANGED <<storedIntent, memoryIntent, commitPending, commitCandidate,
       commandType, commandPhase, writable, creationParity>>
  /\ Record("disposed", commandType)

Reload ==
  /\ ~connected
  /\ ~commitPending
  /\ connected' = TRUE
  /\ memoryIntent' = storedIntent
  /\ commandType' = CommandFor(storedIntent)
  /\ commandPhase' = IF commandType' = "none" THEN "none" ELSE "queued"
  /\ UNCHANGED <<storedIntent, commitPending, commitCandidate, writable,
       creationParity>>
  /\ Record("reloaded", commandType')

Next ==
  \/ RequestProvision
  \/ RequestClose
  \/ StartCommit
  \/ CommitSucceeded
  \/ CommitFailed
  \/ Dispatch
  \/ CompleteEffect
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
  /\ creationParity \in BOOLEAN
  /\ lastAction \in Actions
  /\ lastCommand \in Commands
  /\ previousStoredIntent \in Intents
  /\ previousCreationParity \in BOOLEAN

CommandPhaseIntegrity ==
  (commandType = "none") <=> (commandPhase = "none")

DispatchRequiresDurableIntent ==
  lastAction = "dispatch" => CommandCovers(previousStoredIntent, lastCommand)

FailedCommitDoesNotChangeStore ==
  lastAction = "commit-failed" => storedIntent = previousStoredIntent

OnlyProvisionCreates ==
  lastAction = "effect-completed" /\ lastCommand # "provision" =>
    creationParity = previousCreationParity

ProvisionCreatesExactlyOne ==
  lastAction = "effect-completed" /\ lastCommand = "provision" =>
    creationParity # previousCreationParity

QueuedWhileLiveEventuallySettles ==
  connected /\ writable /\ commandPhase = "queued" ~>
    commandPhase = "completed" \/ ~connected

=============================================================================
