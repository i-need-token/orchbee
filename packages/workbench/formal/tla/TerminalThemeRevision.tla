------------------------ MODULE TerminalThemeRevision ------------------------
\* Refines the runtime-theme request/host-commit/host-replay/host-stale/
\* host-failed/host-outcome-unknown/host-reconcile/client-prepare/present
\* vocabulary in docs/terminal-theme-revision.md and Terminal Host claim TH-018.
EXTENDS FiniteSets, Naturals, TLC

Operations == {1, 2}
SnapshotTokens == {"snapshot-a", "snapshot-b"}
HostFields == {"defaults", "palette", "cursor"}
PresentationFields == {"pixels", "overlays", "cursor", "app-surface"}
HostPhases == {"absent", "in-flight", "committed", "failed", "unknown", "stale"}
ClientPhases == {"absent", "in-flight", "ready", "failed", "cancelled"}
Outcomes == {"absent", "pending", "completed", "cancelled"}
Modes == {"active", "hidden", "suspended"}
Actions == {
  "init", "request", "supersede", "host-commit", "host-outcome-unknown",
  "host-stale", "host-failed", "host-reconcile", "host-retry",
  "host-replay", "host-conflict", "client-prepared", "client-failed",
  "client-retry", "present", "hide", "suspend", "resume"
}
None == "none"
MaxRevision == Cardinality(Operations)

VARIABLES
  hostPhase,
  hostRetryable,
  clientPhase,
  clientRetryable,
  outcome,
  eligible,
  expectedRevision,
  desiredSnapshot,
  latest,
  actualHostRevision,
  actualHostOperation,
  actualHostSnapshot,
  actualHostPublished,
  observedHostRevision,
  observedHostOperation,
  observedHostSnapshot,
  observedHostPublished,
  presentedRevision,
  presentedOperation,
  presentedSnapshot,
  presentationPublished,
  mode,
  lastAction,
  lastOperation,
  lastSnapshot,
  previousLatest,
  previousHostPhase,
  previousActualHostRevision,
  previousActualHostOperation,
  previousActualHostSnapshot,
  previousActualHostPublished,
  previousObservedHostRevision,
  previousObservedHostOperation,
  previousObservedHostSnapshot,
  previousObservedHostPublished,
  previousPresentedRevision,
  previousPresentedOperation,
  previousPresentedSnapshot,
  previousPresentationPublished

vars ==
  <<hostPhase, hostRetryable, clientPhase, clientRetryable, outcome, eligible,
    expectedRevision, desiredSnapshot, latest, actualHostRevision,
    actualHostOperation, actualHostSnapshot, actualHostPublished,
    observedHostRevision, observedHostOperation, observedHostSnapshot,
    observedHostPublished, presentedRevision, presentedOperation,
    presentedSnapshot, presentationPublished, mode, lastAction, lastOperation,
    lastSnapshot, previousLatest, previousHostPhase,
    previousActualHostRevision, previousActualHostOperation,
    previousActualHostSnapshot, previousActualHostPublished,
    previousObservedHostRevision, previousObservedHostOperation,
    previousObservedHostSnapshot, previousObservedHostPublished,
    previousPresentedRevision, previousPresentedOperation,
    previousPresentedSnapshot, previousPresentationPublished>>

Record(action, operation, snapshot) ==
  /\ lastAction' = action
  /\ lastOperation' = operation
  /\ lastSnapshot' = snapshot
  /\ previousLatest' = latest
  /\ previousHostPhase' = hostPhase
  /\ previousActualHostRevision' = actualHostRevision
  /\ previousActualHostOperation' = actualHostOperation
  /\ previousActualHostSnapshot' = actualHostSnapshot
  /\ previousActualHostPublished' = actualHostPublished
  /\ previousObservedHostRevision' = observedHostRevision
  /\ previousObservedHostOperation' = observedHostOperation
  /\ previousObservedHostSnapshot' = observedHostSnapshot
  /\ previousObservedHostPublished' = observedHostPublished
  /\ previousPresentedRevision' = presentedRevision
  /\ previousPresentedOperation' = presentedOperation
  /\ previousPresentedSnapshot' = presentedSnapshot
  /\ previousPresentationPublished' = presentationPublished

Init ==
  /\ hostPhase = [operation \in Operations |-> "absent"]
  /\ hostRetryable = [operation \in Operations |-> FALSE]
  /\ clientPhase = [operation \in Operations |-> "absent"]
  /\ clientRetryable = [operation \in Operations |-> FALSE]
  /\ outcome = [operation \in Operations |-> "absent"]
  /\ eligible = [operation \in Operations |-> FALSE]
  /\ expectedRevision = [operation \in Operations |-> 0]
  /\ desiredSnapshot = [operation \in Operations |-> None]
  /\ latest = 0
  /\ actualHostRevision = 0
  /\ actualHostOperation = 0
  /\ actualHostSnapshot = None
  /\ actualHostPublished = [field \in HostFields |-> 0]
  /\ observedHostRevision = 0
  /\ observedHostOperation = 0
  /\ observedHostSnapshot = None
  /\ observedHostPublished = [field \in HostFields |-> 0]
  /\ presentedRevision = 0
  /\ presentedOperation = 0
  /\ presentedSnapshot = None
  /\ presentationPublished = [field \in PresentationFields |-> 0]
  /\ mode = "active"
  /\ lastAction = "init"
  /\ lastOperation = 0
  /\ lastSnapshot = None
  /\ previousLatest = latest
  /\ previousHostPhase = hostPhase
  /\ previousActualHostRevision = actualHostRevision
  /\ previousActualHostOperation = actualHostOperation
  /\ previousActualHostSnapshot = actualHostSnapshot
  /\ previousActualHostPublished = actualHostPublished
  /\ previousObservedHostRevision = observedHostRevision
  /\ previousObservedHostOperation = observedHostOperation
  /\ previousObservedHostSnapshot = observedHostSnapshot
  /\ previousObservedHostPublished = observedHostPublished
  /\ previousPresentedRevision = presentedRevision
  /\ previousPresentedOperation = presentedOperation
  /\ previousPresentedSnapshot = presentedSnapshot
  /\ previousPresentationPublished = presentationPublished

Request(operation, snapshot) ==
  /\ operation \in Operations
  /\ operation = latest + 1
  /\ snapshot \in SnapshotTokens
  /\ hostPhase[operation] = "absent"
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "in-flight"]
  /\ hostRetryable' = [hostRetryable EXCEPT ![operation] = FALSE]
  /\ clientPhase' = [clientPhase EXCEPT ![operation] = "in-flight"]
  /\ clientRetryable' = [clientRetryable EXCEPT ![operation] = FALSE]
  /\ outcome' = [candidate \in Operations |->
       IF candidate = operation
       THEN "pending"
       ELSE IF eligible[candidate]
       THEN "cancelled"
       ELSE outcome[candidate]]
  /\ eligible' = [candidate \in Operations |-> candidate = operation]
  /\ expectedRevision' =
       [expectedRevision EXCEPT ![operation] = observedHostRevision]
  /\ desiredSnapshot' = [desiredSnapshot EXCEPT ![operation] = snapshot]
  /\ latest' = operation
  /\ UNCHANGED
       <<actualHostRevision, actualHostOperation, actualHostSnapshot,
         actualHostPublished, observedHostRevision, observedHostOperation,
         observedHostSnapshot, observedHostPublished, presentedRevision,
         presentedOperation, presentedSnapshot, presentationPublished, mode>>
  /\ Record(IF latest = 0 THEN "request" ELSE "supersede", operation, snapshot)

HostCanCommit(operation) ==
  /\ hostPhase[operation] = "in-flight"
  /\ expectedRevision[operation] = actualHostRevision
  /\ actualHostRevision < MaxRevision

HostCommits(operation) ==
  /\ HostCanCommit(operation)
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "committed"]
  /\ actualHostRevision' = actualHostRevision + 1
  /\ actualHostOperation' = operation
  /\ actualHostSnapshot' = desiredSnapshot[operation]
  /\ actualHostPublished' =
       [field \in HostFields |-> actualHostRevision + 1]
  /\ observedHostRevision' = actualHostRevision + 1
  /\ observedHostOperation' = operation
  /\ observedHostSnapshot' = desiredSnapshot[operation]
  /\ observedHostPublished' =
       [field \in HostFields |-> actualHostRevision + 1]
  /\ UNCHANGED
       <<hostRetryable, clientPhase, clientRetryable, outcome, eligible,
         expectedRevision, desiredSnapshot, latest, presentedRevision,
         presentedOperation, presentedSnapshot, presentationPublished, mode>>
  /\ Record("host-commit", operation, desiredSnapshot[operation])

HostCommitOutcomeUnknown(operation, didCommit) ==
  /\ HostCanCommit(operation)
  /\ didCommit \in BOOLEAN
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "unknown"]
  /\ IF didCommit
     THEN
       /\ actualHostRevision' = actualHostRevision + 1
       /\ actualHostOperation' = operation
       /\ actualHostSnapshot' = desiredSnapshot[operation]
       /\ actualHostPublished' =
            [field \in HostFields |-> actualHostRevision + 1]
     ELSE
       /\ UNCHANGED
            <<actualHostRevision, actualHostOperation, actualHostSnapshot,
              actualHostPublished>>
  /\ UNCHANGED
       <<hostRetryable, clientPhase, clientRetryable, outcome, eligible,
         expectedRevision, desiredSnapshot, latest, observedHostRevision,
         observedHostOperation, observedHostSnapshot, observedHostPublished,
         presentedRevision, presentedOperation, presentedSnapshot,
         presentationPublished, mode>>
  /\ Record("host-outcome-unknown", operation, desiredSnapshot[operation])

HostRejectsStale(operation) ==
  /\ hostPhase[operation] = "in-flight"
  /\ expectedRevision[operation] # actualHostRevision
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "stale"]
  /\ observedHostRevision' = actualHostRevision
  /\ observedHostOperation' = actualHostOperation
  /\ observedHostSnapshot' = actualHostSnapshot
  /\ observedHostPublished' = actualHostPublished
  /\ UNCHANGED
       <<hostRetryable, clientPhase, clientRetryable, outcome, eligible,
         expectedRevision, desiredSnapshot, latest, actualHostRevision,
         actualHostOperation, actualHostSnapshot, actualHostPublished,
         presentedRevision, presentedOperation, presentedSnapshot,
         presentationPublished, mode>>
  /\ Record("host-stale", operation, desiredSnapshot[operation])

HostFails(operation, retryable) ==
  /\ hostPhase[operation] = "in-flight"
  /\ expectedRevision[operation] = actualHostRevision
  /\ retryable \in BOOLEAN
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "failed"]
  /\ hostRetryable' = [hostRetryable EXCEPT ![operation] = retryable]
  /\ UNCHANGED
       <<clientPhase, clientRetryable, outcome, eligible, expectedRevision,
         desiredSnapshot, latest, actualHostRevision, actualHostOperation,
         actualHostSnapshot, actualHostPublished, observedHostRevision,
         observedHostOperation, observedHostSnapshot, observedHostPublished,
         presentedRevision, presentedOperation, presentedSnapshot,
         presentationPublished, mode>>
  /\ Record("host-failed", operation, desiredSnapshot[operation])

ReconcileHost(operation) ==
  /\ hostPhase[operation] = "unknown"
  /\ hostPhase' = [hostPhase EXCEPT ![operation] =
       IF /\ actualHostOperation = operation
          /\ actualHostSnapshot = desiredSnapshot[operation]
       THEN "committed"
       ELSE IF actualHostOperation = operation THEN "failed" ELSE "stale"]
  /\ hostRetryable' = [hostRetryable EXCEPT ![operation] =
       IF /\ actualHostOperation = operation
          /\ actualHostSnapshot # desiredSnapshot[operation]
       THEN FALSE
       ELSE @]
  /\ observedHostRevision' = actualHostRevision
  /\ observedHostOperation' = actualHostOperation
  /\ observedHostSnapshot' = actualHostSnapshot
  /\ observedHostPublished' = actualHostPublished
  /\ UNCHANGED
       <<clientPhase, clientRetryable, outcome, eligible, expectedRevision,
         desiredSnapshot, latest, actualHostRevision, actualHostOperation,
         actualHostSnapshot, actualHostPublished, presentedRevision,
         presentedOperation, presentedSnapshot, presentationPublished, mode>>
  /\ Record("host-reconcile", operation, desiredSnapshot[operation])

RetryHost(operation) ==
  /\ eligible[operation]
  /\ operation = latest
  /\ \/ hostPhase[operation] = "stale"
     \/ /\ hostPhase[operation] = "failed"
        /\ hostRetryable[operation]
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "in-flight"]
  /\ hostRetryable' = [hostRetryable EXCEPT ![operation] = FALSE]
  /\ expectedRevision' =
       [expectedRevision EXCEPT ![operation] = observedHostRevision]
  /\ UNCHANGED
       <<clientPhase, clientRetryable, outcome, eligible, desiredSnapshot,
         latest, actualHostRevision, actualHostOperation, actualHostSnapshot,
         actualHostPublished, observedHostRevision, observedHostOperation,
         observedHostSnapshot, observedHostPublished, presentedRevision,
         presentedOperation, presentedSnapshot, presentationPublished, mode>>
  /\ Record("host-retry", operation, desiredSnapshot[operation])

HostReplays(operation, candidateSnapshot) ==
  /\ operation \in Operations
  /\ candidateSnapshot \in SnapshotTokens
  /\ actualHostRevision > 0
  /\ actualHostOperation = operation
  /\ hostPhase[operation] = "committed"
  /\ candidateSnapshot = actualHostSnapshot
  /\ UNCHANGED
       <<hostPhase, hostRetryable, clientPhase, clientRetryable, outcome,
         eligible, expectedRevision, desiredSnapshot, latest,
         actualHostRevision, actualHostOperation, actualHostSnapshot,
         actualHostPublished, observedHostRevision, observedHostOperation,
         observedHostSnapshot, observedHostPublished, presentedRevision,
         presentedOperation, presentedSnapshot, presentationPublished, mode>>
  /\ Record("host-replay", operation, candidateSnapshot)

HostRejectsConflict(operation, candidateSnapshot) ==
  /\ operation \in Operations
  /\ candidateSnapshot \in SnapshotTokens
  /\ actualHostRevision > 0
  /\ actualHostOperation = operation
  /\ hostPhase[operation] = "committed"
  /\ candidateSnapshot # actualHostSnapshot
  /\ UNCHANGED
       <<hostPhase, hostRetryable, clientPhase, clientRetryable, outcome,
         eligible, expectedRevision, desiredSnapshot, latest,
         actualHostRevision, actualHostOperation, actualHostSnapshot,
         actualHostPublished, observedHostRevision, observedHostOperation,
         observedHostSnapshot, observedHostPublished, presentedRevision,
         presentedOperation, presentedSnapshot, presentationPublished, mode>>
  /\ Record("host-conflict", operation, candidateSnapshot)

ClientPreparationCompletes(operation) ==
  /\ clientPhase[operation] = "in-flight"
  /\ clientPhase' = [clientPhase EXCEPT ![operation] =
       IF eligible[operation] THEN "ready" ELSE "cancelled"]
  /\ UNCHANGED
       <<hostPhase, hostRetryable, clientRetryable, outcome, eligible,
         expectedRevision, desiredSnapshot, latest, actualHostRevision,
         actualHostOperation, actualHostSnapshot, actualHostPublished,
         observedHostRevision, observedHostOperation, observedHostSnapshot,
         observedHostPublished, presentedRevision, presentedOperation,
         presentedSnapshot, presentationPublished, mode>>
  /\ Record("client-prepared", operation, desiredSnapshot[operation])

ClientPreparationFails(operation, retryable) ==
  /\ clientPhase[operation] = "in-flight"
  /\ retryable \in BOOLEAN
  /\ clientPhase' = [clientPhase EXCEPT ![operation] =
       IF eligible[operation] THEN "failed" ELSE "cancelled"]
  /\ clientRetryable' = [clientRetryable EXCEPT ![operation] =
       eligible[operation] /\ retryable]
  /\ UNCHANGED
       <<hostPhase, hostRetryable, outcome, eligible, expectedRevision,
         desiredSnapshot, latest, actualHostRevision, actualHostOperation,
         actualHostSnapshot, actualHostPublished, observedHostRevision,
         observedHostOperation, observedHostSnapshot, observedHostPublished,
         presentedRevision, presentedOperation, presentedSnapshot,
         presentationPublished, mode>>
  /\ Record("client-failed", operation, desiredSnapshot[operation])

RetryClient(operation) ==
  /\ eligible[operation]
  /\ operation = latest
  /\ clientPhase[operation] = "failed"
  /\ clientRetryable[operation]
  /\ clientPhase' = [clientPhase EXCEPT ![operation] = "in-flight"]
  /\ clientRetryable' = [clientRetryable EXCEPT ![operation] = FALSE]
  /\ UNCHANGED
       <<hostPhase, hostRetryable, outcome, eligible, expectedRevision,
         desiredSnapshot, latest, actualHostRevision, actualHostOperation,
         actualHostSnapshot, actualHostPublished, observedHostRevision,
         observedHostOperation, observedHostSnapshot, observedHostPublished,
         presentedRevision, presentedOperation, presentedSnapshot,
         presentationPublished, mode>>
  /\ Record("client-retry", operation, desiredSnapshot[operation])

Presentable(operation) ==
  /\ mode = "active"
  /\ eligible[operation]
  /\ operation = latest
  /\ hostPhase[operation] = "committed"
  /\ actualHostOperation = operation
  /\ actualHostSnapshot = desiredSnapshot[operation]
  /\ observedHostRevision = actualHostRevision
  /\ observedHostOperation = actualHostOperation
  /\ observedHostSnapshot = actualHostSnapshot
  /\ clientPhase[operation] = "ready"

Present(operation) ==
  /\ Presentable(operation)
  /\ outcome' = [outcome EXCEPT ![operation] = "completed"]
  /\ eligible' = [eligible EXCEPT ![operation] = FALSE]
  /\ presentedRevision' = observedHostRevision
  /\ presentedOperation' = operation
  /\ presentedSnapshot' = desiredSnapshot[operation]
  /\ presentationPublished' =
       [field \in PresentationFields |-> observedHostRevision]
  /\ UNCHANGED
       <<hostPhase, hostRetryable, clientPhase, clientRetryable,
         expectedRevision, desiredSnapshot, latest, actualHostRevision,
         actualHostOperation, actualHostSnapshot, actualHostPublished,
         observedHostRevision, observedHostOperation, observedHostSnapshot,
         observedHostPublished, mode>>
  /\ Record("present", operation, desiredSnapshot[operation])

Hide ==
  /\ mode = "active"
  /\ mode' = "hidden"
  /\ UNCHANGED
       <<hostPhase, hostRetryable, clientPhase, clientRetryable, outcome,
         eligible, expectedRevision, desiredSnapshot, latest,
         actualHostRevision, actualHostOperation, actualHostSnapshot,
         actualHostPublished, observedHostRevision, observedHostOperation,
         observedHostSnapshot, observedHostPublished, presentedRevision,
         presentedOperation, presentedSnapshot, presentationPublished>>
  /\ Record("hide", 0, None)

Suspend ==
  /\ mode = "active"
  /\ mode' = "suspended"
  /\ UNCHANGED
       <<hostPhase, hostRetryable, clientPhase, clientRetryable, outcome,
         eligible, expectedRevision, desiredSnapshot, latest,
         actualHostRevision, actualHostOperation, actualHostSnapshot,
         actualHostPublished, observedHostRevision, observedHostOperation,
         observedHostSnapshot, observedHostPublished, presentedRevision,
         presentedOperation, presentedSnapshot, presentationPublished>>
  /\ Record("suspend", 0, None)

Resume ==
  /\ mode \in {"hidden", "suspended"}
  /\ mode' = "active"
  /\ UNCHANGED
       <<hostPhase, hostRetryable, clientPhase, clientRetryable, outcome,
         eligible, expectedRevision, desiredSnapshot, latest,
         actualHostRevision, actualHostOperation, actualHostSnapshot,
         actualHostPublished, observedHostRevision, observedHostOperation,
         observedHostSnapshot, observedHostPublished, presentedRevision,
         presentedOperation, presentedSnapshot, presentationPublished>>
  /\ Record("resume", 0, None)

Next ==
  \/ \E operation \in Operations, snapshot \in SnapshotTokens:
       Request(operation, snapshot)
  \/ \E operation \in Operations: HostCommits(operation)
  \/ \E operation \in Operations, didCommit \in BOOLEAN:
       HostCommitOutcomeUnknown(operation, didCommit)
  \/ \E operation \in Operations: HostRejectsStale(operation)
  \/ \E operation \in Operations, retryable \in BOOLEAN:
       HostFails(operation, retryable)
  \/ \E operation \in Operations: ReconcileHost(operation)
  \/ \E operation \in Operations: RetryHost(operation)
  \/ \E operation \in Operations, candidateSnapshot \in SnapshotTokens:
       HostReplays(operation, candidateSnapshot)
  \/ \E operation \in Operations, candidateSnapshot \in SnapshotTokens:
       HostRejectsConflict(operation, candidateSnapshot)
  \/ \E operation \in Operations: ClientPreparationCompletes(operation)
  \/ \E operation \in Operations, retryable \in BOOLEAN:
       ClientPreparationFails(operation, retryable)
  \/ \E operation \in Operations: RetryClient(operation)
  \/ \E operation \in Operations: Present(operation)
  \/ Hide
  \/ Suspend
  \/ Resume

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A operation \in Operations: WF_vars(Present(operation))

TypeOK ==
  /\ hostPhase \in [Operations -> HostPhases]
  /\ hostRetryable \in [Operations -> BOOLEAN]
  /\ clientPhase \in [Operations -> ClientPhases]
  /\ clientRetryable \in [Operations -> BOOLEAN]
  /\ outcome \in [Operations -> Outcomes]
  /\ eligible \in [Operations -> BOOLEAN]
  /\ expectedRevision \in [Operations -> 0..MaxRevision]
  /\ desiredSnapshot \in [Operations -> SnapshotTokens \cup {None}]
  /\ latest \in 0..Cardinality(Operations)
  /\ actualHostRevision \in 0..MaxRevision
  /\ actualHostOperation \in 0..Cardinality(Operations)
  /\ actualHostSnapshot \in SnapshotTokens \cup {None}
  /\ actualHostPublished \in [HostFields -> 0..MaxRevision]
  /\ observedHostRevision \in 0..MaxRevision
  /\ observedHostOperation \in 0..Cardinality(Operations)
  /\ observedHostSnapshot \in SnapshotTokens \cup {None}
  /\ observedHostPublished \in [HostFields -> 0..MaxRevision]
  /\ presentedRevision \in 0..MaxRevision
  /\ presentedOperation \in 0..Cardinality(Operations)
  /\ presentedSnapshot \in SnapshotTokens \cup {None}
  /\ presentationPublished \in [PresentationFields -> 0..MaxRevision]
  /\ mode \in Modes
  /\ lastAction \in Actions
  /\ lastOperation \in 0..Cardinality(Operations)
  /\ lastSnapshot \in SnapshotTokens \cup {None}
  /\ previousLatest \in 0..Cardinality(Operations)
  /\ previousHostPhase \in [Operations -> HostPhases]
  /\ previousActualHostRevision \in 0..MaxRevision
  /\ previousActualHostOperation \in 0..Cardinality(Operations)
  /\ previousActualHostSnapshot \in SnapshotTokens \cup {None}
  /\ previousActualHostPublished \in [HostFields -> 0..MaxRevision]
  /\ previousObservedHostRevision \in 0..MaxRevision
  /\ previousObservedHostOperation \in 0..Cardinality(Operations)
  /\ previousObservedHostSnapshot \in SnapshotTokens \cup {None}
  /\ previousObservedHostPublished \in [HostFields -> 0..MaxRevision]
  /\ previousPresentedRevision \in 0..MaxRevision
  /\ previousPresentedOperation \in 0..Cardinality(Operations)
  /\ previousPresentedSnapshot \in SnapshotTokens \cup {None}
  /\ previousPresentationPublished \in
       [PresentationFields -> 0..MaxRevision]

HostStateIdentityIsCoherent ==
  /\ actualHostRevision = 0 =>
       /\ actualHostOperation = 0
       /\ actualHostSnapshot = None
  /\ actualHostRevision > 0 =>
       /\ actualHostOperation \in Operations
       /\ actualHostSnapshot = desiredSnapshot[actualHostOperation]
       /\ actualHostSnapshot \in SnapshotTokens
  /\ observedHostRevision = 0 =>
       /\ observedHostOperation = 0
       /\ observedHostSnapshot = None
  /\ observedHostRevision > 0 =>
       /\ observedHostOperation \in Operations
       /\ observedHostSnapshot = desiredSnapshot[observedHostOperation]
       /\ observedHostSnapshot \in SnapshotTokens

HostCommitIsAtomic ==
  /\ \A field \in HostFields:
       actualHostPublished[field] = actualHostRevision
  /\ \A field \in HostFields:
       observedHostPublished[field] = observedHostRevision

PresentationIsAtomic ==
  /\ \A field \in PresentationFields:
       presentationPublished[field] = presentedRevision
  /\ presentedRevision = 0 =>
       /\ presentedOperation = 0
       /\ presentedSnapshot = None
  /\ presentedRevision > 0 =>
       /\ presentedOperation \in Operations
       /\ presentedSnapshot = desiredSnapshot[presentedOperation]

PresentationNeverLeadsHost ==
  presentedRevision <= actualHostRevision

LatestOnlyPresents ==
  lastAction = "present" =>
    /\ lastOperation = previousLatest
    /\ presentedOperation = lastOperation
    /\ presentedSnapshot = desiredSnapshot[lastOperation]

HostChangesOnlyOnCommit ==
  (actualHostRevision # previousActualHostRevision \/
   actualHostOperation # previousActualHostOperation \/
   actualHostSnapshot # previousActualHostSnapshot \/
   actualHostPublished # previousActualHostPublished) =>
    lastAction \in {"host-commit", "host-outcome-unknown"}

ObservedHostChangesOnlyOnResult ==
  (observedHostRevision # previousObservedHostRevision \/
   observedHostOperation # previousObservedHostOperation \/
   observedHostSnapshot # previousObservedHostSnapshot \/
   observedHostPublished # previousObservedHostPublished) =>
    lastAction \in {"host-commit", "host-stale", "host-reconcile"}

PresentationChangesOnlyOnPresent ==
  (presentedRevision # previousPresentedRevision \/
   presentedOperation # previousPresentedOperation \/
   presentedSnapshot # previousPresentedSnapshot \/
   presentationPublished # previousPresentationPublished) =>
    lastAction = "present"

NonCommitHostResultsPreserveHost ==
  lastAction \in {
    "host-failed", "host-stale", "host-reconcile", "host-retry",
    "host-replay", "host-conflict"
  } =>
    /\ actualHostRevision = previousActualHostRevision
    /\ actualHostOperation = previousActualHostOperation
    /\ actualHostSnapshot = previousActualHostSnapshot
    /\ actualHostPublished = previousActualHostPublished

NonPresentationActionsPreservePresentation ==
  lastAction # "present" =>
    /\ presentedRevision = previousPresentedRevision
    /\ presentedOperation = previousPresentedOperation
    /\ presentedSnapshot = previousPresentedSnapshot
    /\ presentationPublished = previousPresentationPublished

SupersessionPreservesPresentation ==
  lastAction = "supersede" =>
    /\ presentedRevision = previousPresentedRevision
    /\ presentedOperation = previousPresentedOperation
    /\ presentedSnapshot = previousPresentedSnapshot
    /\ presentationPublished = previousPresentationPublished

UnknownOutcomePreservesObservation ==
  lastAction = "host-outcome-unknown" =>
    /\ observedHostRevision = previousObservedHostRevision
    /\ observedHostOperation = previousObservedHostOperation
    /\ observedHostSnapshot = previousObservedHostSnapshot
    /\ observedHostPublished = previousObservedHostPublished

ReconciliationAdoptsActualHost ==
  lastAction = "host-reconcile" =>
    /\ observedHostRevision = actualHostRevision
    /\ observedHostOperation = actualHostOperation
    /\ observedHostSnapshot = actualHostSnapshot
    /\ observedHostPublished = actualHostPublished

ReconciliationClassifiesHostIdentity ==
  lastAction = "host-reconcile" =>
    IF /\ actualHostOperation = lastOperation
       /\ actualHostSnapshot = desiredSnapshot[lastOperation]
    THEN hostPhase[lastOperation] = "committed"
    ELSE IF actualHostOperation = lastOperation
         THEN
           /\ hostPhase[lastOperation] = "failed"
           /\ ~hostRetryable[lastOperation]
         ELSE hostPhase[lastOperation] = "stale"

HostReplayIsExactAndIdempotent ==
  lastAction = "host-replay" =>
    /\ lastOperation = previousActualHostOperation
    /\ lastSnapshot = previousActualHostSnapshot
    /\ actualHostRevision = previousActualHostRevision
    /\ actualHostOperation = previousActualHostOperation
    /\ actualHostSnapshot = previousActualHostSnapshot
    /\ actualHostPublished = previousActualHostPublished
    /\ observedHostRevision = previousObservedHostRevision
    /\ observedHostOperation = previousObservedHostOperation
    /\ observedHostSnapshot = previousObservedHostSnapshot
    /\ observedHostPublished = previousObservedHostPublished

HostConflictIsDifferentAndNonMutating ==
  lastAction = "host-conflict" =>
    /\ lastOperation = previousActualHostOperation
    /\ lastSnapshot # previousActualHostSnapshot
    /\ ~hostRetryable[lastOperation]
    /\ actualHostRevision = previousActualHostRevision
    /\ actualHostOperation = previousActualHostOperation
    /\ actualHostSnapshot = previousActualHostSnapshot
    /\ actualHostPublished = previousActualHostPublished
    /\ observedHostRevision = previousObservedHostRevision
    /\ observedHostOperation = previousObservedHostOperation
    /\ observedHostSnapshot = previousObservedHostSnapshot
    /\ observedHostPublished = previousObservedHostPublished

UnknownOutcomeRequiresReconciliation ==
  lastAction = "host-retry" =>
    /\ lastOperation \in Operations
    /\ previousHostPhase[lastOperation] \in {"failed", "stale"}

EligibleReadyEventuallyResolves ==
  \A operation \in Operations:
    [](Presentable(operation)
      ~> (presentedOperation = operation \/ latest # operation \/
           mode # "active"))

=============================================================================
