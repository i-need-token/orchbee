------------------- MODULE WorkspaceDirectoryAssociation -------------------
EXTENDS Integers, Naturals, FiniteSets, TLC

(*
Two admitted Clients race to associate one empty Workspace with a local path.
Validation is read-only and may overlap. Client flights are distinct from
request and Directory identities, so two Clients can carry the same exact
command concurrently. The Server captures the exact Worker generation before
releasing its locks, then repeats admission after validation before atomically
allocating a Directory, selecting it, advancing the Site revision and retaining
the result.

Filesystem validity is an external observation. An invalid, unavailable or
wrong-Worker observation, a stale generation/revision, or resource exhaustion
must leave the association identity cursor and retained ledger unchanged.
*)

CONSTANTS Clients, Requests, Paths, Worker, NoWorker, ExternalDirectory,
          NoDirectory, NoPath, MaxDirectories, MaxLedgerEntries,
          MaxLedgerBytes, AcceptedBytes, MaxValidationSlots, MaxRevision,
          SharedRequestMode

ASSUME /\ Cardinality(Clients) = 2
       /\ Cardinality(Requests) = 2
       /\ Cardinality(Paths) = 2
       /\ Worker # NoWorker
       /\ ExternalDirectory \notin Requests
       /\ NoDirectory \notin Requests \cup {ExternalDirectory}
       /\ NoPath \notin Paths
       /\ MaxDirectories \in 1..2
       /\ MaxLedgerEntries \in 1..2
       /\ MaxLedgerBytes \in 1..2
       /\ AcceptedBytes = 1
       /\ MaxValidationSlots \in 1..2
       /\ MaxRevision = 3
       /\ SharedRequestMode \in BOOLEAN

DirectoryIds == Requests \cup {ExternalDirectory}
FirstClient == CHOOSE client \in Clients : TRUE
FirstRequest == CHOOSE request \in Requests : TRUE
SecondRequest == CHOOSE request \in Requests : request # FirstRequest
FirstPath == CHOOSE path \in Paths : TRUE
SecondPath == CHOOSE path \in Paths : path # FirstPath
RequestForClient(client) ==
  IF SharedRequestMode \/ client = FirstClient
  THEN FirstRequest
  ELSE SecondRequest
PathForRequest(request) ==
  IF request = FirstRequest THEN FirstPath ELSE SecondPath
DefaultPath == CHOOSE path \in Paths : TRUE

Phases == {"idle", "validating", "observed", "finished"}
ValidationStates == {"none", "valid", "invalid", "unavailable", "wrong-worker"}
Replies ==
  {"none", "denied", "stale", "workspace-directory-conflict",
   "worker-unavailable", "directory-unavailable", "capacity-unavailable",
   "slot-unavailable", "accepted-pending", "accepted-delivered",
   "accepted-lost", "accepted-replayed", "replay-conflict"}
LedgerStates == {"absent", "accepted"}

LedgerRecordType ==
  [status : LedgerStates,
   path : Paths \cup {NoPath},
   base : -1..MaxRevision,
   resultRevision : -1..MaxRevision,
   directory : DirectoryIds \cup {NoDirectory},
   worker : {Worker, NoWorker}]

EmptyLedgerRecord ==
  [status |-> "absent",
   path |-> NoPath,
   base |-> -1,
   resultRevision |-> -1,
   directory |-> NoDirectory,
   worker |-> NoWorker]

VARIABLES
  phase,
  grant,
  requestPath,
  requestId,
  requestBase,
  capturedWorker,
  capturedGeneration,
  validation,
  reply,
  workerAvailable,
  workerGeneration,
  revision,
  externalRevisionCount,
  workspaceEmpty,
  directoryIds,
  primaryDirectory,
  directoryWorker,
  directoryPath,
  ledger,
  associationEffects,
  externalLedgerEntries,
  externalLedgerBytes,
  lastAction

requestVars ==
  <<phase, requestId, requestPath, requestBase, capturedWorker, capturedGeneration,
    validation, reply>>
environmentVars == <<workerAvailable, workerGeneration>>
canonicalVars ==
  <<revision, externalRevisionCount, workspaceEmpty, directoryIds,
    primaryDirectory, directoryWorker, directoryPath, ledger,
    associationEffects, externalLedgerEntries, externalLedgerBytes>>
vars == <<requestVars, grant, environmentVars, canonicalVars, lastAction>>

ActiveValidations ==
  {client \in Clients : phase[client] \in {"validating", "observed"}}
AcceptedEntries ==
  {request \in Requests : ledger[request].status = "accepted"}
AssociationCommitCount ==
  Cardinality({request \in Requests : associationEffects[request] = 1})
DirectoryCapacityAvailable == Cardinality(directoryIds) < MaxDirectories
LedgerCapacityAvailable ==
  /\ externalLedgerEntries + Cardinality(AcceptedEntries) < MaxLedgerEntries
  /\ externalLedgerBytes
       + AcceptedBytes * (Cardinality(AcceptedEntries) + 1)
       <= MaxLedgerBytes
AdmissionCapacityAvailable ==
  DirectoryCapacityAvailable /\ LedgerCapacityAvailable

Init ==
  /\ phase = [client \in Clients |-> "idle"]
  /\ grant \in [Clients -> BOOLEAN]
  /\ requestId = [client \in Clients |-> NoDirectory]
  /\ requestPath = [client \in Clients |-> NoPath]
  /\ requestBase = [client \in Clients |-> -1]
  /\ capturedWorker = [client \in Clients |-> NoWorker]
  /\ capturedGeneration = [client \in Clients |-> 0]
  /\ validation = [client \in Clients |-> "none"]
  /\ reply = [client \in Clients |-> "none"]
  /\ workerAvailable = TRUE
  /\ workerGeneration = 1
  /\ revision = 0
  /\ externalRevisionCount = 0
  /\ workspaceEmpty = TRUE
  /\ directoryIds = {}
  /\ primaryDirectory = NoDirectory
  /\ directoryWorker = [directory \in DirectoryIds |-> NoWorker]
  /\ directoryPath = [directory \in DirectoryIds |-> NoPath]
  /\ ledger = [request \in Requests |-> EmptyLedgerRecord]
  /\ associationEffects = [request \in Requests |-> 0]
  /\ externalLedgerEntries = 0
  /\ externalLedgerBytes = 0
  /\ lastAction = "Init"

FinishWithoutCanonicalEffect(client, outcome, actionName) ==
  /\ phase' = [phase EXCEPT ![client] = "finished"]
  /\ reply' = [reply EXCEPT ![client] = outcome]
  /\ lastAction' = actionName
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker, capturedGeneration,
         validation, environmentVars, canonicalVars>>

DenyAssociation(client) ==
  /\ phase[client] = "idle"
  /\ ~grant[client]
  /\ FinishWithoutCanonicalEffect(client, "denied", "DenyAssociation")

RefuseStaleRevision(client, path, base) ==
  /\ phase[client] = "idle"
  /\ grant[client]
  /\ ledger[RequestForClient(client)].status = "absent"
  /\ path \in Paths
  /\ base \in 0..MaxRevision
  /\ base # revision
  /\ FinishWithoutCanonicalEffect(client, "stale", "RefuseStaleRevision")

RefuseWorkspaceConflict(client, path, base) ==
  /\ phase[client] = "idle"
  /\ grant[client]
  /\ ledger[RequestForClient(client)].status = "absent"
  /\ path \in Paths
  /\ base = revision
  /\ (~workspaceEmpty \/ primaryDirectory # NoDirectory)
  /\ FinishWithoutCanonicalEffect(
       client, "workspace-directory-conflict", "RefuseWorkspaceConflict")

RefuseWorkerUnavailable(client, path, base) ==
  /\ phase[client] = "idle"
  /\ grant[client]
  /\ ledger[RequestForClient(client)].status = "absent"
  /\ path \in Paths
  /\ base = revision
  /\ workspaceEmpty
  /\ primaryDirectory = NoDirectory
  /\ ~workerAvailable
  /\ FinishWithoutCanonicalEffect(
       client, "worker-unavailable", "RefuseWorkerUnavailable")

RefuseCapacity(client, path, base) ==
  /\ phase[client] = "idle"
  /\ grant[client]
  /\ ledger[RequestForClient(client)].status = "absent"
  /\ path \in Paths
  /\ base = revision
  /\ workspaceEmpty
  /\ primaryDirectory = NoDirectory
  /\ workerAvailable
  /\ ~AdmissionCapacityAvailable
  /\ FinishWithoutCanonicalEffect(
       client, "capacity-unavailable", "RefuseCapacity")

RefuseValidationSlot(client, path, base) ==
  /\ phase[client] = "idle"
  /\ grant[client]
  /\ ledger[RequestForClient(client)].status = "absent"
  /\ path \in Paths
  /\ base = revision
  /\ workspaceEmpty
  /\ primaryDirectory = NoDirectory
  /\ workerAvailable
  /\ AdmissionCapacityAvailable
  /\ Cardinality(ActiveValidations) = MaxValidationSlots
  /\ FinishWithoutCanonicalEffect(
       client, "slot-unavailable", "RefuseValidationSlot")

BeginValidation(client, path, base) ==
  /\ phase[client] = "idle"
  /\ grant[client]
  /\ ledger[RequestForClient(client)].status = "absent"
  /\ path \in Paths
  /\ path = PathForRequest(RequestForClient(client))
  /\ base = revision
  /\ workspaceEmpty
  /\ primaryDirectory = NoDirectory
  /\ workerAvailable
  /\ AdmissionCapacityAvailable
  /\ Cardinality(ActiveValidations) < MaxValidationSlots
  /\ phase' = [phase EXCEPT ![client] = "validating"]
  /\ requestId' = [requestId EXCEPT ![client] = RequestForClient(client)]
  /\ requestPath' = [requestPath EXCEPT ![client] = path]
  /\ requestBase' = [requestBase EXCEPT ![client] = base]
  /\ capturedWorker' = [capturedWorker EXCEPT ![client] = Worker]
  /\ capturedGeneration' =
       [capturedGeneration EXCEPT ![client] = workerGeneration]
  /\ validation' = [validation EXCEPT ![client] = "none"]
  /\ lastAction' = "BeginValidation"
  /\ UNCHANGED <<grant, reply, environmentVars, canonicalVars>>

ObserveDirectoryValid(client) ==
  /\ phase[client] = "validating"
  /\ phase' = [phase EXCEPT ![client] = "observed"]
  /\ validation' = [validation EXCEPT ![client] = "valid"]
  /\ lastAction' = "ObserveDirectoryValid"
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker, capturedGeneration,
         reply, environmentVars, canonicalVars>>

ObserveDirectoryInvalid(client) ==
  /\ phase[client] = "validating"
  /\ phase' = [phase EXCEPT ![client] = "observed"]
  /\ validation' = [validation EXCEPT ![client] = "invalid"]
  /\ lastAction' = "ObserveDirectoryInvalid"
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, reply, environmentVars, canonicalVars>>

ObserveDirectoryUnavailable(client) ==
  /\ phase[client] = "validating"
  /\ phase' = [phase EXCEPT ![client] = "observed"]
  /\ validation' = [validation EXCEPT ![client] = "unavailable"]
  /\ lastAction' = "ObserveDirectoryUnavailable"
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, reply, environmentVars, canonicalVars>>

ObserveWrongWorkerResult(client) ==
  /\ phase[client] = "validating"
  /\ phase' = [phase EXCEPT ![client] = "observed"]
  /\ validation' = [validation EXCEPT ![client] = "wrong-worker"]
  /\ lastAction' = "ObserveWrongWorkerResult"
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, reply, environmentVars, canonicalVars>>

ResolveDirectoryInvalid(client) ==
  /\ phase[client] = "observed"
  /\ validation[client] = "invalid"
  /\ ledger[requestId[client]].status = "absent"
  /\ FinishWithoutCanonicalEffect(
       client, "directory-unavailable", "ResolveDirectoryInvalid")

ResolveDirectoryUnavailable(client) ==
  /\ phase[client] = "observed"
  /\ validation[client] = "unavailable"
  /\ ledger[requestId[client]].status = "absent"
  /\ FinishWithoutCanonicalEffect(
       client, "directory-unavailable", "ResolveDirectoryUnavailable")

ResolveWrongWorkerResult(client) ==
  /\ phase[client] = "observed"
  /\ validation[client] = "wrong-worker"
  /\ ledger[requestId[client]].status = "absent"
  /\ FinishWithoutCanonicalEffect(
       client, "worker-unavailable", "ResolveWrongWorkerResult")

ReplayAcceptedFlight(client) ==
  /\ phase[client] = "observed"
  /\ ledger[requestId[client]].status = "accepted"
  /\ requestPath[client] = ledger[requestId[client]].path
  /\ requestBase[client] = ledger[requestId[client]].base
  /\ phase' = [phase EXCEPT ![client] = "finished"]
  /\ reply' = [reply EXCEPT ![client] = "accepted-replayed"]
  /\ lastAction' = "ReplayAcceptedFlight"
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, validation, environmentVars, canonicalVars>>

ResolveObservedReplayConflict(client) ==
  /\ phase[client] = "observed"
  /\ ledger[requestId[client]].status = "accepted"
  /\ (requestPath[client] # ledger[requestId[client]].path
       \/ requestBase[client] # ledger[requestId[client]].base)
  /\ phase' = [phase EXCEPT ![client] = "finished"]
  /\ reply' = [reply EXCEPT ![client] = "replay-conflict"]
  /\ lastAction' = "ResolveObservedReplayConflict"
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, validation, environmentVars, canonicalVars>>

RejectValidationRevision(client) ==
  /\ phase[client] = "observed"
  /\ validation[client] = "valid"
  /\ ledger[requestId[client]].status = "absent"
  /\ requestBase[client] # revision
  /\ FinishWithoutCanonicalEffect(client, "stale", "RejectValidationRevision")

RejectValidationWorker(client) ==
  /\ phase[client] = "observed"
  /\ validation[client] = "valid"
  /\ ledger[requestId[client]].status = "absent"
  /\ requestBase[client] = revision
  /\ (~workerAvailable
       \/ capturedWorker[client] # Worker
       \/ capturedGeneration[client] # workerGeneration)
  /\ FinishWithoutCanonicalEffect(
       client, "worker-unavailable", "RejectValidationWorker")

RejectValidationCapacity(client) ==
  /\ phase[client] = "observed"
  /\ validation[client] = "valid"
  /\ ledger[requestId[client]].status = "absent"
  /\ requestBase[client] = revision
  /\ workerAvailable
  /\ capturedWorker[client] = Worker
  /\ capturedGeneration[client] = workerGeneration
  /\ workspaceEmpty
  /\ primaryDirectory = NoDirectory
  /\ ~AdmissionCapacityAvailable
  /\ FinishWithoutCanonicalEffect(
       client, "capacity-unavailable", "RejectValidationCapacity")

CommitAssociation(client) ==
  /\ phase[client] = "observed"
  /\ validation[client] = "valid"
  /\ ledger[requestId[client]].status = "absent"
  /\ requestBase[client] = revision
  /\ workerAvailable
  /\ capturedWorker[client] = Worker
  /\ capturedGeneration[client] = workerGeneration
  /\ workspaceEmpty
  /\ primaryDirectory = NoDirectory
  /\ AdmissionCapacityAvailable
  /\ revision < MaxRevision
  /\ phase' = [phase EXCEPT ![client] = "finished"]
  /\ reply' = [reply EXCEPT ![client] = "accepted-pending"]
  /\ revision' = revision + 1
  /\ directoryIds' = directoryIds \cup {requestId[client]}
  /\ primaryDirectory' = requestId[client]
  /\ directoryWorker' =
       [directoryWorker EXCEPT ![requestId[client]] = Worker]
  /\ directoryPath' =
       [directoryPath EXCEPT ![requestId[client]] = requestPath[client]]
  /\ ledger' =
       [ledger EXCEPT
         ![requestId[client]] =
           [status |-> "accepted",
            path |-> requestPath[client],
            base |-> requestBase[client],
            resultRevision |-> revision + 1,
            directory |-> requestId[client],
            worker |-> Worker]]
  /\ associationEffects' =
       [associationEffects EXCEPT ![requestId[client]] = 1]
  /\ lastAction' = "CommitAssociation"
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker, capturedGeneration,
         validation, environmentVars, externalRevisionCount, workspaceEmpty,
         externalLedgerEntries, externalLedgerBytes>>

DeliverAccepted(client) ==
  /\ reply[client] = "accepted-pending"
  /\ reply' = [reply EXCEPT ![client] = "accepted-delivered"]
  /\ lastAction' = "DeliverAccepted"
  /\ UNCHANGED
       <<grant, phase, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, validation, environmentVars, canonicalVars>>

LoseAcceptedResult(client) ==
  /\ reply[client] = "accepted-pending"
  /\ reply' = [reply EXCEPT ![client] = "accepted-lost"]
  /\ lastAction' = "LoseAcceptedResult"
  /\ UNCHANGED
       <<grant, phase, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, validation, environmentVars, canonicalVars>>

ReplayAccepted(client, path, base) ==
  /\ phase[client] = "finished"
  /\ grant[client]
  /\ ledger[RequestForClient(client)].status = "accepted"
  /\ path = ledger[RequestForClient(client)].path
  /\ base = ledger[RequestForClient(client)].base
  /\ reply[client] \in
       {"accepted-delivered", "accepted-lost", "accepted-replayed"}
  /\ reply' = [reply EXCEPT ![client] = "accepted-replayed"]
  /\ lastAction' = "ReplayAccepted"
  /\ UNCHANGED
       <<grant, phase, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, validation, environmentVars, canonicalVars>>

ReplayRetainedBeforeProbe(client, path, base) ==
  /\ phase[client] = "idle"
  /\ grant[client]
  /\ ledger[RequestForClient(client)].status = "accepted"
  /\ path = ledger[RequestForClient(client)].path
  /\ base = ledger[RequestForClient(client)].base
  /\ phase' = [phase EXCEPT ![client] = "finished"]
  /\ reply' = [reply EXCEPT ![client] = "accepted-replayed"]
  /\ lastAction' = "ReplayRetainedBeforeProbe"
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, validation, environmentVars, canonicalVars>>

ReplayConflict(client, path, base) ==
  /\ phase[client] \in {"idle", "finished"}
  /\ grant[client]
  /\ ledger[RequestForClient(client)].status = "accepted"
  /\ path \in Paths
  /\ base \in 0..MaxRevision
  /\ (path # ledger[RequestForClient(client)].path
       \/ base # ledger[RequestForClient(client)].base)
  /\ phase' = [phase EXCEPT ![client] = "finished"]
  /\ reply' = [reply EXCEPT ![client] = "replay-conflict"]
  /\ lastAction' = "ReplayConflict"
  /\ UNCHANGED
       <<grant, requestId, requestPath, requestBase, capturedWorker,
         capturedGeneration, validation, environmentVars, canonicalVars>>

LoseWorker ==
  /\ workerAvailable
  /\ workerAvailable' = FALSE
  /\ lastAction' = "LoseWorker"
  /\ UNCHANGED <<requestVars, grant, workerGeneration, canonicalVars>>

RestoreWorker ==
  /\ ~workerAvailable
  /\ workerAvailable' = TRUE
  /\ lastAction' = "RestoreWorker"
  /\ UNCHANGED <<requestVars, grant, workerGeneration, canonicalVars>>

ReplaceWorker ==
  /\ workerGeneration = 1
  /\ workerGeneration' = 2
  /\ workerAvailable' = TRUE
  /\ lastAction' = "ReplaceWorker"
  /\ UNCHANGED <<requestVars, grant, canonicalVars>>

AdvanceRevision ==
  /\ primaryDirectory = NoDirectory
  /\ workspaceEmpty
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ externalRevisionCount' = externalRevisionCount + 1
  /\ lastAction' = "AdvanceRevision"
  /\ UNCHANGED
       <<requestVars, grant, environmentVars, workspaceEmpty, directoryIds,
         primaryDirectory, directoryWorker, directoryPath, ledger,
         associationEffects, externalLedgerEntries, externalLedgerBytes>>

PopulateWorkspace ==
  /\ primaryDirectory = NoDirectory
  /\ workspaceEmpty
  /\ revision < MaxRevision
  /\ workspaceEmpty' = FALSE
  /\ revision' = revision + 1
  /\ externalRevisionCount' = externalRevisionCount + 1
  /\ lastAction' = "PopulateWorkspace"
  /\ UNCHANGED
       <<requestVars, grant, environmentVars, directoryIds, primaryDirectory,
         directoryWorker, directoryPath, ledger, associationEffects,
         externalLedgerEntries, externalLedgerBytes>>

FillDirectoryCapacity ==
  /\ primaryDirectory = NoDirectory
  /\ ExternalDirectory \notin directoryIds
  /\ Cardinality(directoryIds) < MaxDirectories
  /\ revision < MaxRevision
  /\ directoryIds' = directoryIds \cup {ExternalDirectory}
  /\ directoryWorker' =
       [directoryWorker EXCEPT ![ExternalDirectory] = Worker]
  /\ directoryPath' =
       [directoryPath EXCEPT ![ExternalDirectory] = DefaultPath]
  /\ revision' = revision + 1
  /\ externalRevisionCount' = externalRevisionCount + 1
  /\ lastAction' = "FillDirectoryCapacity"
  /\ UNCHANGED
       <<requestVars, grant, environmentVars, workspaceEmpty, primaryDirectory,
         ledger, associationEffects, externalLedgerEntries,
         externalLedgerBytes>>

FillExternalLedger ==
  /\ primaryDirectory = NoDirectory
  /\ externalLedgerEntries = 0
  /\ externalLedgerEntries + Cardinality(AcceptedEntries) + 1
       <= MaxLedgerEntries
  /\ externalLedgerBytes
       + AcceptedBytes * (Cardinality(AcceptedEntries) + 1)
       <= MaxLedgerBytes
  /\ externalLedgerEntries' = 1
  /\ externalLedgerBytes' = AcceptedBytes
  /\ lastAction' = "FillExternalLedger"
  /\ UNCHANGED
       <<requestVars, grant, environmentVars, revision,
         externalRevisionCount, workspaceEmpty, directoryIds,
         primaryDirectory, directoryWorker, directoryPath, ledger,
         associationEffects>>

ObserveConcurrentValidation ==
  /\ Cardinality(ActiveValidations) = 2
  /\ UNCHANGED vars

ObserveNegativeProbeReplayed ==
  /\ SharedRequestMode
  /\ \E client \in Clients:
       /\ validation[client] \in {"invalid", "unavailable", "wrong-worker"}
       /\ reply[client] = "accepted-replayed"
       /\ ledger[requestId[client]].status = "accepted"
  /\ UNCHANGED vars

ObserveStaleGenerationProbeReplayed ==
  /\ SharedRequestMode
  /\ \E client \in Clients:
       /\ capturedGeneration[client] \in 1..2
       /\ capturedGeneration[client] # workerGeneration
       /\ reply[client] = "accepted-replayed"
       /\ ledger[requestId[client]].status = "accepted"
  /\ UNCHANGED vars

Next ==
  \/ \E client \in Clients: DenyAssociation(client)
  \/ \E client \in Clients, path \in Paths, base \in 0..MaxRevision:
       \/ RefuseStaleRevision(client, path, base)
       \/ RefuseWorkspaceConflict(client, path, base)
       \/ RefuseWorkerUnavailable(client, path, base)
       \/ RefuseCapacity(client, path, base)
       \/ RefuseValidationSlot(client, path, base)
       \/ BeginValidation(client, path, base)
       \/ ReplayAccepted(client, path, base)
       \/ ReplayRetainedBeforeProbe(client, path, base)
       \/ ReplayConflict(client, path, base)
  \/ \E client \in Clients:
       \/ ObserveDirectoryValid(client)
       \/ ObserveDirectoryInvalid(client)
       \/ ObserveDirectoryUnavailable(client)
       \/ ObserveWrongWorkerResult(client)
       \/ ResolveDirectoryInvalid(client)
       \/ ResolveDirectoryUnavailable(client)
       \/ ResolveWrongWorkerResult(client)
       \/ ReplayAcceptedFlight(client)
       \/ ResolveObservedReplayConflict(client)
       \/ RejectValidationRevision(client)
       \/ RejectValidationWorker(client)
       \/ RejectValidationCapacity(client)
       \/ CommitAssociation(client)
       \/ DeliverAccepted(client)
       \/ LoseAcceptedResult(client)
  \/ LoseWorker
  \/ RestoreWorker
  \/ ReplaceWorker
  \/ AdvanceRevision
  \/ PopulateWorkspace
  \/ FillDirectoryCapacity
  \/ FillExternalLedger
  \/ ObserveConcurrentValidation
  \/ ObserveNegativeProbeReplayed
  \/ ObserveStaleGenerationProbeReplayed

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in [Clients -> Phases]
  /\ grant \in [Clients -> BOOLEAN]
  /\ requestId \in [Clients -> Requests \cup {NoDirectory}]
  /\ requestPath \in [Clients -> Paths \cup {NoPath}]
  /\ requestBase \in [Clients -> -1..MaxRevision]
  /\ capturedWorker \in [Clients -> {Worker, NoWorker}]
  /\ capturedGeneration \in [Clients -> 0..2]
  /\ validation \in [Clients -> ValidationStates]
  /\ reply \in [Clients -> Replies]
  /\ workerAvailable \in BOOLEAN
  /\ workerGeneration \in 1..2
  /\ revision \in 0..MaxRevision
  /\ externalRevisionCount \in 0..MaxRevision
  /\ workspaceEmpty \in BOOLEAN
  /\ directoryIds \subseteq DirectoryIds
  /\ primaryDirectory \in DirectoryIds \cup {NoDirectory}
  /\ directoryWorker \in [DirectoryIds -> {Worker, NoWorker}]
  /\ directoryPath \in [DirectoryIds -> Paths \cup {NoPath}]
  /\ ledger \in [Requests -> LedgerRecordType]
  /\ associationEffects \in [Requests -> 0..1]
  /\ externalLedgerEntries \in 0..1
  /\ externalLedgerBytes \in {0, AcceptedBytes}
  /\ lastAction \in
       {"Init", "DenyAssociation", "RefuseStaleRevision",
        "RefuseWorkspaceConflict", "RefuseWorkerUnavailable",
        "RefuseCapacity", "RefuseValidationSlot", "BeginValidation",
        "ObserveDirectoryValid", "ObserveDirectoryInvalid",
        "ObserveDirectoryUnavailable", "ObserveWrongWorkerResult",
        "ResolveDirectoryInvalid", "ResolveDirectoryUnavailable",
        "ResolveWrongWorkerResult", "ReplayAcceptedFlight",
        "ResolveObservedReplayConflict",
        "RejectValidationRevision", "RejectValidationWorker",
        "RejectValidationCapacity", "CommitAssociation",
        "DeliverAccepted", "LoseAcceptedResult", "ReplayAccepted",
        "ReplayRetainedBeforeProbe", "ReplayConflict", "LoseWorker",
        "RestoreWorker", "ReplaceWorker",
        "AdvanceRevision", "PopulateWorkspace", "FillDirectoryCapacity",
        "FillExternalLedger"}

AtMostOneAssociation == AssociationCommitCount <= 1

RevisionAccountsForCommits ==
  revision = externalRevisionCount + AssociationCommitCount

DirectoryIdentityAccounting ==
  directoryIds =
    {request \in Requests : associationEffects[request] = 1}
    \cup (IF ExternalDirectory \in directoryIds
          THEN {ExternalDirectory} ELSE {})

PrimaryReferencesAcceptedDirectory ==
  /\ (primaryDirectory = NoDirectory <=> AssociationCommitCount = 0)
  /\ (primaryDirectory # NoDirectory =>
        /\ primaryDirectory \in Requests
        /\ primaryDirectory \in directoryIds
        /\ associationEffects[primaryDirectory] = 1)

DirectoryRecordsAreComplete ==
  \A directory \in DirectoryIds:
    IF directory \in directoryIds
    THEN /\ directoryWorker[directory] = Worker
         /\ directoryPath[directory] \in Paths
    ELSE /\ directoryWorker[directory] = NoWorker
         /\ directoryPath[directory] = NoPath

RetainedAcceptanceMatchesCanonical ==
  \A request \in Requests:
    /\ (ledger[request].status = "accepted" <=>
          associationEffects[request] = 1)
    /\ (ledger[request].status = "accepted" =>
          /\ ledger[request].directory = request
          /\ ledger[request].worker = Worker
          /\ ledger[request].path = directoryPath[request]
          /\ ledger[request].base + 1 = ledger[request].resultRevision
          /\ ledger[request].resultRevision <= revision)
    /\ (ledger[request].status = "absent" =>
          ledger[request] = EmptyLedgerRecord)

ValidationCapturesExactWorker ==
  \A client \in ActiveValidations:
    /\ requestId[client] = RequestForClient(client)
    /\ requestPath[client] \in Paths
    /\ requestPath[client] = PathForRequest(requestId[client])
    /\ requestBase[client] \in 0..MaxRevision
    /\ capturedWorker[client] = Worker
    /\ capturedGeneration[client] \in 1..2

ResourceBounds ==
  /\ Cardinality(directoryIds) <= MaxDirectories
  /\ externalLedgerEntries + Cardinality(AcceptedEntries)
       <= MaxLedgerEntries
  /\ externalLedgerBytes
       + AcceptedBytes * Cardinality(AcceptedEntries)
       <= MaxLedgerBytes
  /\ Cardinality(ActiveValidations) <= MaxValidationSlots

CommitIsAtomic == [][\A request \in Requests:
  associationEffects'[request] > associationEffects[request] =>
    /\ revision' = revision + 1
    /\ primaryDirectory' = request
    /\ request \in directoryIds'
    /\ directoryWorker'[request] = Worker
    /\ directoryPath'[request] = ledger'[request].path
    /\ directoryPath'[request] \in Paths
    /\ ledger'[request].status = "accepted"
    /\ ledger'[request].resultRevision = revision']_vars

AssociationIsSetOnce == [][primaryDirectory # NoDirectory =>
  /\ primaryDirectory' = primaryDirectory
  /\ directoryIds' = directoryIds
  /\ directoryWorker' = directoryWorker
  /\ directoryPath' = directoryPath
  /\ associationEffects' = associationEffects]_vars

ReplayAndLossPreserveCanonical == [][
  lastAction' \in
    {"DeliverAccepted", "LoseAcceptedResult", "ReplayAccepted",
     "ReplayRetainedBeforeProbe", "ReplayAcceptedFlight", "ReplayConflict",
     "ResolveObservedReplayConflict"}
  => UNCHANGED canonicalVars]_vars

NonCommitResolutionPreservesCanonical == [][
  lastAction' \in
    {"DenyAssociation", "RefuseStaleRevision", "RefuseWorkspaceConflict",
     "RefuseWorkerUnavailable", "RefuseCapacity", "RefuseValidationSlot",
     "ResolveDirectoryInvalid", "ResolveDirectoryUnavailable",
     "ResolveWrongWorkerResult", "RejectValidationRevision",
     "RejectValidationWorker", "RejectValidationCapacity"}
  => UNCHANGED canonicalVars]_vars

=============================================================================
