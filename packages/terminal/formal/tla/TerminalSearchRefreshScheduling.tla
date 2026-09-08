------------------ MODULE TerminalSearchRefreshScheduling ------------------
EXTENDS Naturals

CONSTANTS MaxRevision, MaxScans

\* bound-exhausted is an exploration sentinel, not a product search error.
Phases == {"scheduled", "scanning", "complete", "bound-exhausted", "disposed"}
Actions == {
  "init", "start-scan", "advance-while-scheduled", "advance-while-scanning",
  "advance-after-completion", "complete-current", "settle-obsolete", "dispose"
}

VARIABLES active, revision, phase, snapshotRevision, scanCount,
  publishedRevision, priorPhase, priorSnapshotRevision,
  priorScanCount, priorPublishedRevision, lastAction

vars == <<active, revision, phase, snapshotRevision, scanCount,
  publishedRevision, priorPhase, priorSnapshotRevision,
  priorScanCount, priorPublishedRevision, lastAction>>

Init ==
  /\ active = TRUE
  /\ revision = 1
  /\ phase = "scheduled"
  /\ snapshotRevision = 0
  /\ scanCount = 0
  /\ publishedRevision = 0
  /\ priorPhase = phase
  /\ priorSnapshotRevision = snapshotRevision
  /\ priorScanCount = scanCount
  /\ priorPublishedRevision = publishedRevision
  /\ lastAction = "init"

Record(action, oldPhase, oldSnapshotRevision, oldScanCount,
       oldPublishedRevision) ==
  /\ priorPhase' = oldPhase
  /\ priorSnapshotRevision' = oldSnapshotRevision
  /\ priorScanCount' = oldScanCount
  /\ priorPublishedRevision' = oldPublishedRevision
  /\ lastAction' = action

StartScan ==
  /\ active
  /\ phase = "scheduled"
  /\ scanCount < MaxScans
  /\ phase' = "scanning"
  /\ snapshotRevision' = revision
  /\ scanCount' = scanCount + 1
  /\ UNCHANGED <<active, revision, publishedRevision>>
  /\ Record("start-scan", phase, snapshotRevision, scanCount,
       publishedRevision)

AdvanceWhileScheduled ==
  /\ active
  /\ phase = "scheduled"
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ UNCHANGED <<active, phase, snapshotRevision, scanCount, publishedRevision>>
  /\ Record("advance-while-scheduled", phase, snapshotRevision,
       scanCount, publishedRevision)

AdvanceWhileScanning ==
  /\ active
  /\ phase = "scanning"
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ UNCHANGED <<active, phase, snapshotRevision, scanCount, publishedRevision>>
  /\ Record("advance-while-scanning", phase, snapshotRevision,
       scanCount, publishedRevision)

AdvanceAfterCompletion ==
  /\ active
  /\ phase = "complete"
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ phase' = IF scanCount < MaxScans THEN "scheduled" ELSE "bound-exhausted"
  /\ UNCHANGED <<active, snapshotRevision, scanCount, publishedRevision>>
  /\ Record("advance-after-completion", phase, snapshotRevision,
       scanCount, publishedRevision)

CompleteCurrent ==
  /\ active
  /\ phase = "scanning"
  /\ snapshotRevision = revision
  /\ phase' = "complete"
  /\ publishedRevision' = snapshotRevision
  /\ UNCHANGED <<active, revision, snapshotRevision, scanCount>>
  /\ Record("complete-current", phase, snapshotRevision, scanCount,
       publishedRevision)

SettleObsolete ==
  /\ active
  /\ phase = "scanning"
  /\ snapshotRevision # revision
  /\ phase' = IF scanCount < MaxScans THEN "scheduled" ELSE "bound-exhausted"
  /\ UNCHANGED <<active, revision, snapshotRevision, scanCount,
       publishedRevision>>
  /\ Record("settle-obsolete", phase, snapshotRevision, scanCount,
       publishedRevision)

Dispose ==
  /\ active
  /\ active' = FALSE
  /\ phase' = "disposed"
  /\ UNCHANGED <<revision, snapshotRevision, scanCount, publishedRevision>>
  /\ Record("dispose", phase, snapshotRevision, scanCount,
       publishedRevision)

SettleByConsistency == CompleteCurrent \/ SettleObsolete

Next ==
  \/ StartScan
  \/ AdvanceWhileScheduled
  \/ AdvanceWhileScanning
  \/ AdvanceAfterCompletion
  \/ CompleteCurrent
  \/ SettleObsolete
  \/ Dispose

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(StartScan)
  /\ WF_vars(SettleByConsistency)

TypeOK ==
  /\ active \in BOOLEAN
  /\ revision \in 1..MaxRevision
  /\ phase \in Phases
  /\ snapshotRevision \in 0..MaxRevision
  /\ scanCount \in 0..MaxScans
  /\ publishedRevision \in 0..MaxRevision
  /\ priorPhase \in Phases
  /\ priorSnapshotRevision \in 0..MaxRevision
  /\ priorScanCount \in 0..MaxScans
  /\ priorPublishedRevision \in 0..MaxRevision
  /\ lastAction \in Actions

ScheduledInvalidationPreservesAdmission ==
  lastAction = "advance-while-scheduled" =>
    /\ phase = "scheduled"
    /\ snapshotRevision = priorSnapshotRevision
    /\ scanCount = priorScanCount

ScanningInvalidationDoesNotCancel ==
  lastAction = "advance-while-scanning" =>
    /\ priorPhase = "scanning"
    /\ phase = "scanning"
    /\ snapshotRevision = priorSnapshotRevision
    /\ scanCount = priorScanCount

CurrentResultsOnly ==
  phase = "complete" =>
    /\ snapshotRevision = revision
    /\ publishedRevision = revision

ObsoleteResultsNeverPublish ==
  lastAction = "settle-obsolete" =>
    publishedRevision = priorPublishedRevision

DisposalClosesScheduling == ~active => phase = "disposed"

EveryScheduledSearchEventuallyStarts ==
  [](active /\ phase = "scheduled" /\ scanCount < MaxScans =>
     <>(phase # "scheduled" \/ ~active))

EveryAdmittedScanEventuallySettles ==
  [](phase = "scanning" => <>(phase # "scanning"))

=============================================================================
