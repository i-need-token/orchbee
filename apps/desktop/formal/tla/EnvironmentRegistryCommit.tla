------------------------ MODULE EnvironmentRegistryCommit ------------------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

\* Bounded abstraction for the Desktop-colocated Environment registry's
\* append-only create workflow. JSON parsing, filesystem permissions, SSH,
\* credentials, Project placement, renderer APIs, and deletion remain outside.

CONSTANT EnvironmentIds, Targets, MaxEnvironments

NoValue == "none"
Phases == {"idle", "writing", "renamed", "indeterminate"}
Actions == {
  "init", "create-new", "replay-same", "reject-id-conflict",
  "reject-target-conflict", "reject-capacity", "rename",
  "confirm-durability", "durability-indeterminate", "reload-indeterminate",
  "restart-keeps-rename", "restart-loses-rename", "precommit-failure",
  "restart-before-rename"
}

VARIABLES
  memoryRecords,
  memoryOrder,
  memoryRevision,
  diskRecords,
  diskOrder,
  diskRevision,
  phase,
  pendingId,
  pendingTarget,
  createdClaims,
  lastAction

vars == <<memoryRecords, memoryOrder, memoryRevision,
  diskRecords, diskOrder, diskRevision, phase, pendingId,
  pendingTarget, createdClaims, lastAction>>

RecordIds(records) == {environmentId \in EnvironmentIds : records[environmentId] # NoValue}
SequenceSet(sequence) == {sequence[index] : index \in 1..Len(sequence)}
TargetUsed(records, target) ==
  \E environmentId \in EnvironmentIds : records[environmentId] = target

Init ==
  /\ memoryRecords = [environmentId \in EnvironmentIds |-> NoValue]
  /\ memoryOrder = <<>>
  /\ memoryRevision = 0
  /\ diskRecords = [environmentId \in EnvironmentIds |-> NoValue]
  /\ diskOrder = <<>>
  /\ diskRevision = 0
  /\ phase = "idle"
  /\ pendingId = NoValue
  /\ pendingTarget = NoValue
  /\ createdClaims = {}
  /\ lastAction = "init"

CreateNew(environmentId, target) ==
  /\ phase = "idle"
  /\ environmentId \in EnvironmentIds
  /\ target \in Targets
  /\ memoryRecords[environmentId] = NoValue
  /\ ~TargetUsed(memoryRecords, target)
  /\ Len(memoryOrder) < MaxEnvironments
  /\ phase' = "writing"
  /\ pendingId' = environmentId
  /\ pendingTarget' = target
  /\ UNCHANGED <<memoryRecords, memoryOrder, memoryRevision,
       diskRecords, diskOrder, diskRevision, createdClaims>>
  /\ lastAction' = "create-new"

ReplaySame(environmentId, target) ==
  /\ phase = "idle"
  /\ environmentId \in EnvironmentIds
  /\ target \in Targets
  /\ memoryRecords[environmentId] = target
  /\ UNCHANGED <<memoryRecords, memoryOrder, memoryRevision,
       diskRecords, diskOrder, diskRevision, phase, pendingId,
       pendingTarget, createdClaims>>
  /\ lastAction' = "replay-same"

RejectIdConflict(environmentId, target) ==
  /\ phase = "idle"
  /\ environmentId \in EnvironmentIds
  /\ target \in Targets
  /\ memoryRecords[environmentId] # NoValue
  /\ memoryRecords[environmentId] # target
  /\ UNCHANGED <<memoryRecords, memoryOrder, memoryRevision,
       diskRecords, diskOrder, diskRevision, phase, pendingId,
       pendingTarget, createdClaims>>
  /\ lastAction' = "reject-id-conflict"

RejectTargetConflict(environmentId, target) ==
  /\ phase = "idle"
  /\ environmentId \in EnvironmentIds
  /\ target \in Targets
  /\ memoryRecords[environmentId] = NoValue
  /\ TargetUsed(memoryRecords, target)
  /\ UNCHANGED <<memoryRecords, memoryOrder, memoryRevision,
       diskRecords, diskOrder, diskRevision, phase, pendingId,
       pendingTarget, createdClaims>>
  /\ lastAction' = "reject-target-conflict"

RejectCapacity(environmentId, target) ==
  /\ phase = "idle"
  /\ environmentId \in EnvironmentIds
  /\ target \in Targets
  /\ memoryRecords[environmentId] = NoValue
  /\ ~TargetUsed(memoryRecords, target)
  /\ Len(memoryOrder) = MaxEnvironments
  /\ UNCHANGED <<memoryRecords, memoryOrder, memoryRevision,
       diskRecords, diskOrder, diskRevision, phase, pendingId,
       pendingTarget, createdClaims>>
  /\ lastAction' = "reject-capacity"

Rename ==
  /\ phase = "writing"
  /\ diskRevision = memoryRevision
  /\ diskRecords' = [diskRecords EXCEPT ![pendingId] = pendingTarget]
  /\ diskOrder' = Append(diskOrder, pendingId)
  /\ diskRevision' = diskRevision + 1
  /\ phase' = "renamed"
  /\ UNCHANGED <<memoryRecords, memoryOrder, memoryRevision,
       pendingId, pendingTarget, createdClaims>>
  /\ lastAction' = "rename"

ConfirmDurability ==
  /\ phase = "renamed"
  /\ memoryRecords' = diskRecords
  /\ memoryOrder' = diskOrder
  /\ memoryRevision' = diskRevision
  /\ phase' = "idle"
  /\ pendingId' = NoValue
  /\ pendingTarget' = NoValue
  /\ createdClaims' = createdClaims \cup {pendingId}
  /\ UNCHANGED <<diskRecords, diskOrder, diskRevision>>
  /\ lastAction' = "confirm-durability"

DurabilityIndeterminate ==
  /\ phase = "renamed"
  /\ phase' = "indeterminate"
  /\ UNCHANGED <<memoryRecords, memoryOrder, memoryRevision,
       diskRecords, diskOrder, diskRevision, pendingId, pendingTarget,
       createdClaims>>
  /\ lastAction' = "durability-indeterminate"

ReloadIndeterminate ==
  /\ phase = "indeterminate"
  /\ memoryRecords' = diskRecords
  /\ memoryOrder' = diskOrder
  /\ memoryRevision' = diskRevision
  /\ phase' = "idle"
  /\ pendingId' = NoValue
  /\ pendingTarget' = NoValue
  /\ UNCHANGED <<diskRecords, diskOrder, diskRevision, createdClaims>>
  /\ lastAction' = "reload-indeterminate"

RestartKeepsRename ==
  /\ phase = "indeterminate"
  /\ memoryRecords' = diskRecords
  /\ memoryOrder' = diskOrder
  /\ memoryRevision' = diskRevision
  /\ phase' = "idle"
  /\ pendingId' = NoValue
  /\ pendingTarget' = NoValue
  /\ UNCHANGED <<diskRecords, diskOrder, diskRevision, createdClaims>>
  /\ lastAction' = "restart-keeps-rename"

RestartLosesRename ==
  /\ phase = "indeterminate"
  /\ diskRecords' = memoryRecords
  /\ diskOrder' = memoryOrder
  /\ diskRevision' = memoryRevision
  /\ phase' = "idle"
  /\ pendingId' = NoValue
  /\ pendingTarget' = NoValue
  /\ UNCHANGED <<memoryRecords, memoryOrder, memoryRevision, createdClaims>>
  /\ lastAction' = "restart-loses-rename"

PrecommitFailure ==
  /\ phase = "writing"
  /\ phase' = "idle"
  /\ pendingId' = NoValue
  /\ pendingTarget' = NoValue
  /\ UNCHANGED <<memoryRecords, memoryOrder, memoryRevision,
       diskRecords, diskOrder, diskRevision, createdClaims>>
  /\ lastAction' = "precommit-failure"

RestartBeforeRename ==
  /\ phase = "writing"
  /\ memoryRecords' = diskRecords
  /\ memoryOrder' = diskOrder
  /\ memoryRevision' = diskRevision
  /\ phase' = "idle"
  /\ pendingId' = NoValue
  /\ pendingTarget' = NoValue
  /\ UNCHANGED <<diskRecords, diskOrder, diskRevision, createdClaims>>
  /\ lastAction' = "restart-before-rename"

Next ==
  \/ \E environmentId \in EnvironmentIds, target \in Targets:
       CreateNew(environmentId, target)
  \/ \E environmentId \in EnvironmentIds, target \in Targets:
       ReplaySame(environmentId, target)
  \/ \E environmentId \in EnvironmentIds, target \in Targets:
       RejectIdConflict(environmentId, target)
  \/ \E environmentId \in EnvironmentIds, target \in Targets:
       RejectTargetConflict(environmentId, target)
  \/ \E environmentId \in EnvironmentIds, target \in Targets:
       RejectCapacity(environmentId, target)
  \/ Rename
  \/ ConfirmDurability
  \/ DurabilityIndeterminate
  \/ ReloadIndeterminate
  \/ RestartKeepsRename
  \/ RestartLosesRename
  \/ PrecommitFailure
  \/ RestartBeforeRename

Spec == Init /\ [][Next]_vars /\ WF_vars(ReloadIndeterminate)

TypeOK ==
  /\ memoryRecords \in [EnvironmentIds -> Targets \cup {NoValue}]
  /\ memoryOrder \in Seq(EnvironmentIds)
  /\ memoryRevision \in 0..MaxEnvironments
  /\ diskRecords \in [EnvironmentIds -> Targets \cup {NoValue}]
  /\ diskOrder \in Seq(EnvironmentIds)
  /\ diskRevision \in 0..MaxEnvironments
  /\ phase \in Phases
  /\ pendingId \in EnvironmentIds \cup {NoValue}
  /\ pendingTarget \in Targets \cup {NoValue}
  /\ createdClaims \subseteq EnvironmentIds
  /\ lastAction \in Actions

MemoryDocumentIsCanonical ==
  /\ SequenceSet(memoryOrder) = RecordIds(memoryRecords)
  /\ Cardinality(SequenceSet(memoryOrder)) = Len(memoryOrder)
  /\ memoryRevision = Len(memoryOrder)
  /\ \A leftId, rightId \in RecordIds(memoryRecords):
       leftId # rightId => memoryRecords[leftId] # memoryRecords[rightId]

DiskDocumentIsCanonical ==
  /\ SequenceSet(diskOrder) = RecordIds(diskRecords)
  /\ Cardinality(SequenceSet(diskOrder)) = Len(diskOrder)
  /\ diskRevision = Len(diskOrder)
  /\ \A leftId, rightId \in RecordIds(diskRecords):
       leftId # rightId => diskRecords[leftId] # diskRecords[rightId]

StableCommitRelationship ==
  IF phase \in {"renamed", "indeterminate"}
  THEN
    /\ diskRecords = [memoryRecords EXCEPT ![pendingId] = pendingTarget]
    /\ diskOrder = Append(memoryOrder, pendingId)
    /\ diskRevision = memoryRevision + 1
  ELSE memoryRecords = diskRecords /\ memoryOrder = diskOrder /\ memoryRevision = diskRevision

PendingIdentityIsScoped ==
  /\ (pendingId = NoValue) = (pendingTarget = NoValue)
  /\ (phase = "idle") <=> (pendingId = NoValue /\ pendingTarget = NoValue)
  /\ phase # "idle" =>
       (pendingId \in EnvironmentIds /\ pendingTarget \in Targets)

CreatedClaimsRemainPresent ==
  /\ createdClaims \subseteq RecordIds(memoryRecords)
  /\ createdClaims \subseteq RecordIds(diskRecords)

CapacityIsBounded ==
  /\ Len(memoryOrder) <= MaxEnvironments
  /\ Len(diskOrder) <= MaxEnvironments

IndeterminateEventuallyReconciles ==
  [](phase = "indeterminate" ~> phase # "indeterminate")

=============================================================================
