------------------------ MODULE TerminalThemeRevision ------------------------
EXTENDS FiniteSets, Naturals, TLC

Operations == {1, 2}
HostFields == {"defaults", "palette", "cursor"}
PresentationFields == {"pixels", "overlays", "cursor", "app-surface"}
HostPhases == {"absent", "in-flight", "committed", "failed", "unknown", "stale"}
ClientPhases == {"absent", "in-flight", "ready", "failed", "cancelled"}
Outcomes == {"absent", "pending", "completed", "cancelled"}
Modes == {"active", "hidden", "suspended"}
MaxRevision == Cardinality(Operations)

VARIABLES
  hostPhase,
  hostRetryable,
  clientPhase,
  clientRetryable,
  outcome,
  eligible,
  expectedRevision,
  latest,
  hostRevision,
  hostOperation,
  hostPublished,
  presentedRevision,
  presentedOperation,
  presentationPublished,
  mode,
  lastAction,
  previousLatest,
  previousHostPhase,
  previousHostRevision,
  previousHostOperation,
  previousHostPublished,
  previousPresentedRevision,
  previousPresentedOperation,
  previousPresentationPublished

vars ==
  <<hostPhase, hostRetryable, clientPhase, clientRetryable, outcome,
    eligible, expectedRevision, latest, hostRevision, hostOperation,
    hostPublished, presentedRevision, presentedOperation,
    presentationPublished, mode, lastAction, previousLatest,
    previousHostPhase, previousHostRevision, previousHostOperation,
    previousHostPublished, previousPresentedRevision,
    previousPresentedOperation, previousPresentationPublished>>

Record(action) ==
  /\ lastAction' = action
  /\ previousLatest' = latest
  /\ previousHostPhase' = hostPhase
  /\ previousHostRevision' = hostRevision
  /\ previousHostOperation' = hostOperation
  /\ previousHostPublished' = hostPublished
  /\ previousPresentedRevision' = presentedRevision
  /\ previousPresentedOperation' = presentedOperation
  /\ previousPresentationPublished' = presentationPublished

Init ==
  /\ hostPhase = [operation \in Operations |-> "absent"]
  /\ hostRetryable = [operation \in Operations |-> FALSE]
  /\ clientPhase = [operation \in Operations |-> "absent"]
  /\ clientRetryable = [operation \in Operations |-> FALSE]
  /\ outcome = [operation \in Operations |-> "absent"]
  /\ eligible = [operation \in Operations |-> FALSE]
  /\ expectedRevision = [operation \in Operations |-> 0]
  /\ latest = 0
  /\ hostRevision = 0
  /\ hostOperation = 0
  /\ hostPublished = [field \in HostFields |-> 0]
  /\ presentedRevision = 0
  /\ presentedOperation = 0
  /\ presentationPublished = [field \in PresentationFields |-> 0]
  /\ mode = "active"
  /\ lastAction = "init"
  /\ previousLatest = latest
  /\ previousHostPhase = hostPhase
  /\ previousHostRevision = hostRevision
  /\ previousHostOperation = hostOperation
  /\ previousHostPublished = hostPublished
  /\ previousPresentedRevision = presentedRevision
  /\ previousPresentedOperation = presentedOperation
  /\ previousPresentationPublished = presentationPublished

Request(operation) ==
  /\ operation \in Operations
  /\ operation = latest + 1
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
  /\ expectedRevision' = [expectedRevision EXCEPT ![operation] = hostRevision]
  /\ latest' = operation
  /\ UNCHANGED <<hostRevision, hostOperation, hostPublished,
       presentedRevision, presentedOperation, presentationPublished, mode>>
  /\ Record(IF latest = 0 THEN "request" ELSE "supersede")

HostCanCommit(operation) ==
  /\ hostPhase[operation] = "in-flight"
  /\ expectedRevision[operation] = hostRevision
  /\ hostRevision < MaxRevision

HostCommits(operation) ==
  /\ HostCanCommit(operation)
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "committed"]
  /\ hostRevision' = hostRevision + 1
  /\ hostOperation' = operation
  /\ hostPublished' = [field \in HostFields |-> hostRevision + 1]
  /\ UNCHANGED <<hostRetryable, clientPhase, clientRetryable, outcome,
       eligible, expectedRevision, latest, presentedRevision,
       presentedOperation, presentationPublished, mode>>
  /\ Record("host-commit")

HostCommitOutcomeUnknown(operation) ==
  /\ HostCanCommit(operation)
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "unknown"]
  /\ hostRevision' = hostRevision + 1
  /\ hostOperation' = operation
  /\ hostPublished' = [field \in HostFields |-> hostRevision + 1]
  /\ UNCHANGED <<hostRetryable, clientPhase, clientRetryable, outcome,
       eligible, expectedRevision, latest, presentedRevision,
       presentedOperation, presentationPublished, mode>>
  /\ Record("host-outcome-unknown")

HostRejectsStale(operation) ==
  /\ hostPhase[operation] = "in-flight"
  /\ expectedRevision[operation] # hostRevision
  /\ hostOperation # operation
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "stale"]
  /\ UNCHANGED <<hostRetryable, clientPhase, clientRetryable, outcome,
       eligible, expectedRevision, latest, hostRevision, hostOperation,
       hostPublished, presentedRevision, presentedOperation,
       presentationPublished, mode>>
  /\ Record("host-stale")

HostFails(operation, retryable) ==
  /\ hostPhase[operation] = "in-flight"
  /\ expectedRevision[operation] = hostRevision
  /\ retryable \in BOOLEAN
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "failed"]
  /\ hostRetryable' = [hostRetryable EXCEPT ![operation] = retryable]
  /\ UNCHANGED <<clientPhase, clientRetryable, outcome, eligible,
       expectedRevision, latest, hostRevision, hostOperation, hostPublished,
       presentedRevision, presentedOperation, presentationPublished, mode>>
  /\ Record("host-failed")

ReconcileHost(operation) ==
  /\ hostPhase[operation] = "unknown"
  /\ hostPhase' = [hostPhase EXCEPT ![operation] =
       IF hostOperation = operation THEN "committed" ELSE "stale"]
  /\ UNCHANGED <<hostRetryable, clientPhase, clientRetryable, outcome,
       eligible, expectedRevision, latest, hostRevision, hostOperation,
       hostPublished, presentedRevision, presentedOperation,
       presentationPublished, mode>>
  /\ Record("host-reconcile")

RetryHost(operation) ==
  /\ eligible[operation]
  /\ operation = latest
  /\ \/ hostPhase[operation] = "stale"
     \/ /\ hostPhase[operation] = "failed"
        /\ hostRetryable[operation]
  /\ hostPhase' = [hostPhase EXCEPT ![operation] = "in-flight"]
  /\ hostRetryable' = [hostRetryable EXCEPT ![operation] = FALSE]
  /\ expectedRevision' = [expectedRevision EXCEPT ![operation] = hostRevision]
  /\ UNCHANGED <<clientPhase, clientRetryable, outcome, eligible, latest,
       hostRevision, hostOperation, hostPublished, presentedRevision,
       presentedOperation, presentationPublished, mode>>
  /\ Record("host-retry")

HostReplays(operation) ==
  /\ hostOperation = operation
  /\ hostPhase[operation] = "committed"
  /\ UNCHANGED <<hostPhase, hostRetryable, clientPhase, clientRetryable,
       outcome, eligible, expectedRevision, latest, hostRevision,
       hostOperation, hostPublished, presentedRevision, presentedOperation,
       presentationPublished, mode>>
  /\ Record("host-replay")

HostRejectsConflict(operation) ==
  /\ hostOperation = operation
  /\ hostPhase[operation] = "committed"
  /\ UNCHANGED <<hostPhase, hostRetryable, clientPhase, clientRetryable,
       outcome, eligible, expectedRevision, latest, hostRevision,
       hostOperation, hostPublished, presentedRevision, presentedOperation,
       presentationPublished, mode>>
  /\ Record("host-conflict")

ClientPreparationCompletes(operation) ==
  /\ clientPhase[operation] = "in-flight"
  /\ clientPhase' = [clientPhase EXCEPT ![operation] =
       IF eligible[operation] THEN "ready" ELSE "cancelled"]
  /\ UNCHANGED <<hostPhase, hostRetryable, clientRetryable, outcome,
       eligible, expectedRevision, latest, hostRevision, hostOperation,
       hostPublished, presentedRevision, presentedOperation,
       presentationPublished, mode>>
  /\ Record("client-prepared")

ClientPreparationFails(operation, retryable) ==
  /\ clientPhase[operation] = "in-flight"
  /\ retryable \in BOOLEAN
  /\ clientPhase' = [clientPhase EXCEPT ![operation] =
       IF eligible[operation] THEN "failed" ELSE "cancelled"]
  /\ clientRetryable' = [clientRetryable EXCEPT ![operation] =
       eligible[operation] /\ retryable]
  /\ UNCHANGED <<hostPhase, hostRetryable, outcome, eligible,
       expectedRevision, latest, hostRevision, hostOperation, hostPublished,
       presentedRevision, presentedOperation, presentationPublished, mode>>
  /\ Record("client-failed")

RetryClient(operation) ==
  /\ eligible[operation]
  /\ operation = latest
  /\ clientPhase[operation] = "failed"
  /\ clientRetryable[operation]
  /\ clientPhase' = [clientPhase EXCEPT ![operation] = "in-flight"]
  /\ clientRetryable' = [clientRetryable EXCEPT ![operation] = FALSE]
  /\ UNCHANGED <<hostPhase, hostRetryable, outcome, eligible,
       expectedRevision, latest, hostRevision, hostOperation, hostPublished,
       presentedRevision, presentedOperation, presentationPublished, mode>>
  /\ Record("client-retry")

Presentable(operation) ==
  /\ mode = "active"
  /\ eligible[operation]
  /\ operation = latest
  /\ hostPhase[operation] = "committed"
  /\ hostOperation = operation
  /\ clientPhase[operation] = "ready"

Present(operation) ==
  /\ Presentable(operation)
  /\ outcome' = [outcome EXCEPT ![operation] = "completed"]
  /\ eligible' = [eligible EXCEPT ![operation] = FALSE]
  /\ presentedRevision' = hostRevision
  /\ presentedOperation' = operation
  /\ presentationPublished' = [field \in PresentationFields |-> hostRevision]
  /\ UNCHANGED <<hostPhase, hostRetryable, clientPhase, clientRetryable,
       expectedRevision, latest, hostRevision, hostOperation, hostPublished,
       mode>>
  /\ Record("present")

Hide ==
  /\ mode = "active"
  /\ mode' = "hidden"
  /\ UNCHANGED <<hostPhase, hostRetryable, clientPhase, clientRetryable,
       outcome, eligible, expectedRevision, latest, hostRevision,
       hostOperation, hostPublished, presentedRevision, presentedOperation,
       presentationPublished>>
  /\ Record("hide")

Suspend ==
  /\ mode = "active"
  /\ mode' = "suspended"
  /\ UNCHANGED <<hostPhase, hostRetryable, clientPhase, clientRetryable,
       outcome, eligible, expectedRevision, latest, hostRevision,
       hostOperation, hostPublished, presentedRevision, presentedOperation,
       presentationPublished>>
  /\ Record("suspend")

Resume ==
  /\ mode \in {"hidden", "suspended"}
  /\ mode' = "active"
  /\ UNCHANGED <<hostPhase, hostRetryable, clientPhase, clientRetryable,
       outcome, eligible, expectedRevision, latest, hostRevision,
       hostOperation, hostPublished, presentedRevision, presentedOperation,
       presentationPublished>>
  /\ Record("resume")

Next ==
  \/ \E operation \in Operations: Request(operation)
  \/ \E operation \in Operations: HostCommits(operation)
  \/ \E operation \in Operations: HostCommitOutcomeUnknown(operation)
  \/ \E operation \in Operations: HostRejectsStale(operation)
  \/ \E operation \in Operations, retryable \in BOOLEAN:
       HostFails(operation, retryable)
  \/ \E operation \in Operations: ReconcileHost(operation)
  \/ \E operation \in Operations: RetryHost(operation)
  \/ \E operation \in Operations: HostReplays(operation)
  \/ \E operation \in Operations: HostRejectsConflict(operation)
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
  /\ latest \in 0..Cardinality(Operations)
  /\ hostRevision \in 0..MaxRevision
  /\ hostOperation \in 0..Cardinality(Operations)
  /\ hostPublished \in [HostFields -> 0..MaxRevision]
  /\ presentedRevision \in 0..MaxRevision
  /\ presentedOperation \in 0..Cardinality(Operations)
  /\ presentationPublished \in [PresentationFields -> 0..MaxRevision]
  /\ mode \in Modes

HostCommitIsAtomic ==
  \A field \in HostFields: hostPublished[field] = hostRevision

PresentationIsAtomic ==
  \A field \in PresentationFields:
    presentationPublished[field] = presentedRevision

PresentationNeverLeadsHost == presentedRevision <= hostRevision

LatestOnlyPresents ==
  lastAction = "present" => presentedOperation = previousLatest

HostChangesOnlyOnCommit ==
  (hostRevision # previousHostRevision \/
   hostOperation # previousHostOperation \/
   hostPublished # previousHostPublished) =>
    lastAction \in {"host-commit", "host-outcome-unknown"}

PresentationChangesOnlyOnPresent ==
  (presentedRevision # previousPresentedRevision \/
   presentedOperation # previousPresentedOperation \/
   presentationPublished # previousPresentationPublished) =>
    lastAction = "present"

NonCommitHostResultsPreserveHost ==
  lastAction \in {
    "host-failed", "host-stale", "host-reconcile", "host-retry",
    "host-replay", "host-conflict"
  } =>
    /\ hostRevision = previousHostRevision
    /\ hostOperation = previousHostOperation
    /\ hostPublished = previousHostPublished

NonPresentationActionsPreservePresentation ==
  lastAction # "present" =>
    /\ presentedRevision = previousPresentedRevision
    /\ presentedOperation = previousPresentedOperation
    /\ presentationPublished = previousPresentationPublished

SupersessionPreservesPresentation ==
  lastAction = "supersede" =>
    /\ presentedRevision = previousPresentedRevision
    /\ presentationPublished = previousPresentationPublished

UnknownOutcomeRequiresReconciliation ==
  lastAction = "host-retry" =>
    previousHostPhase[latest] \in {"failed", "stale"}

EligibleReadyEventuallyResolves ==
  \A operation \in Operations:
    [](Presentable(operation)
      ~> (presentedOperation = operation \/ latest # operation \/ mode # "active"))

=============================================================================
