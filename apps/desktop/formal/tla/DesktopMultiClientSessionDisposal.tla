---------------- MODULE DesktopMultiClientSessionDisposal ----------------
EXTENDS TLC

\* Bounded model of coordinator disposal around one accepted operation, one
\* operation queued behind it, and one representative session retirement.
\* Disposal seals admission synchronously, drains the captured FIFO tail
\* through commit, publication, and reconciliation, and retires attachment
\* capabilities only afterward. A failed retirement remains retryable.

GatePhases == {"open", "draining", "complete"}
ActivePhases == {"idle", "committing", "committed", "published", "reconciled"}
QueuedPhases == {"idle", "queued", "rejected"}
RetirementPhases == {"idle", "failed", "complete"}
LateRequestPhases == {"not-requested", "rejected"}

VARIABLES
  gate, activeOperation, activeWasAccepted, queuedOperation, retirement,
  lateRequest, sessionsRetired, attachmentsRetired, lastAction

vars == <<gate, activeOperation, activeWasAccepted, queuedOperation,
  retirement, lateRequest, sessionsRetired, attachmentsRetired, lastAction>>

Init ==
  /\ gate = "open"
  /\ activeOperation = "idle"
  /\ activeWasAccepted = FALSE
  /\ queuedOperation = "idle"
  /\ retirement = "idle"
  /\ lateRequest = "not-requested"
  /\ sessionsRetired = FALSE
  /\ attachmentsRetired = FALSE
  /\ lastAction = "init"

AdmitActiveOperation ==
  /\ gate = "open"
  /\ activeOperation = "idle"
  /\ activeOperation' = "committing"
  /\ activeWasAccepted' = TRUE
  /\ lastAction' = "admit-active-operation"
  /\ UNCHANGED <<gate, queuedOperation, retirement, lateRequest,
       sessionsRetired, attachmentsRetired>>

QueueOperationBehindActive ==
  /\ gate = "open"
  /\ activeOperation = "committing"
  /\ queuedOperation = "idle"
  /\ queuedOperation' = "queued"
  /\ lastAction' = "queue-operation-behind-active"
  /\ UNCHANGED <<gate, activeOperation, activeWasAccepted, retirement,
       lateRequest, sessionsRetired, attachmentsRetired>>

BeginDispose ==
  /\ gate = "open"
  /\ gate' = "draining"
  /\ lastAction' = "begin-dispose"
  /\ UNCHANGED <<activeOperation, activeWasAccepted, queuedOperation,
       retirement, lateRequest, sessionsRetired, attachmentsRetired>>

FinishAcceptedCommit ==
  /\ activeOperation = "committing"
  /\ activeOperation' = "committed"
  /\ lastAction' = "finish-accepted-commit"
  /\ UNCHANGED <<gate, activeWasAccepted, queuedOperation, retirement,
       lateRequest, sessionsRetired, attachmentsRetired>>

PublishAcceptedCommit ==
  /\ activeOperation = "committed"
  /\ activeOperation' = "published"
  /\ lastAction' = "publish-accepted-commit"
  /\ UNCHANGED <<gate, activeWasAccepted, queuedOperation, retirement,
       lateRequest, sessionsRetired, attachmentsRetired>>

ReconcileAcceptedCommit ==
  /\ activeOperation = "published"
  /\ activeOperation' = "reconciled"
  /\ lastAction' = "reconcile-accepted-commit"
  /\ UNCHANGED <<gate, activeWasAccepted, queuedOperation, retirement,
       lateRequest, sessionsRetired, attachmentsRetired>>

RejectCapturedQueue ==
  /\ gate = "draining"
  /\ queuedOperation = "queued"
  /\ queuedOperation' = "rejected"
  /\ lastAction' = "reject-captured-queue"
  /\ UNCHANGED <<gate, activeOperation, activeWasAccepted, retirement,
       lateRequest, sessionsRetired, attachmentsRetired>>

RejectPostSeal ==
  /\ gate \in {"draining", "complete"}
  /\ lateRequest = "not-requested"
  /\ lateRequest' = "rejected"
  /\ lastAction' = "reject-post-seal"
  /\ UNCHANGED <<gate, activeOperation, activeWasAccepted, queuedOperation,
       retirement, sessionsRetired, attachmentsRetired>>

CapturedTailDrained ==
  /\ activeOperation \in {"idle", "reconciled"}
  /\ queuedOperation # "queued"

AttemptSessionRetirement ==
  /\ gate = "draining"
  /\ retirement = "idle"
  /\ CapturedTailDrained
  /\ \/ /\ retirement' = "failed"
        /\ gate' = gate
        /\ sessionsRetired' = FALSE
        /\ attachmentsRetired' = FALSE
     \/ /\ retirement' = "complete"
        /\ gate' = "complete"
        /\ sessionsRetired' = TRUE
        /\ attachmentsRetired' = TRUE
  /\ lastAction' = "attempt-session-retirement"
  /\ UNCHANGED <<activeOperation, activeWasAccepted, queuedOperation, lateRequest>>

RetrySessionRetirement ==
  /\ gate = "draining"
  /\ retirement = "failed"
  /\ CapturedTailDrained
  /\ gate' = "complete"
  /\ retirement' = "complete"
  /\ sessionsRetired' = TRUE
  /\ attachmentsRetired' = TRUE
  /\ lastAction' = "retry-session-retirement"
  /\ UNCHANGED <<activeOperation, activeWasAccepted, queuedOperation, lateRequest>>

Next ==
  \/ AdmitActiveOperation
  \/ QueueOperationBehindActive
  \/ BeginDispose
  \/ FinishAcceptedCommit
  \/ PublishAcceptedCommit
  \/ ReconcileAcceptedCommit
  \/ RejectCapturedQueue
  \/ RejectPostSeal
  \/ AttemptSessionRetirement
  \/ RetrySessionRetirement

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(FinishAcceptedCommit)
  /\ WF_vars(PublishAcceptedCommit)
  /\ WF_vars(ReconcileAcceptedCommit)
  /\ WF_vars(RejectCapturedQueue)
  /\ WF_vars(AttemptSessionRetirement)
  /\ WF_vars(RetrySessionRetirement)

TypeOK ==
  /\ gate \in GatePhases
  /\ activeOperation \in ActivePhases
  /\ activeWasAccepted \in BOOLEAN
  /\ queuedOperation \in QueuedPhases
  /\ retirement \in RetirementPhases
  /\ lateRequest \in LateRequestPhases
  /\ sessionsRetired \in BOOLEAN
  /\ attachmentsRetired \in BOOLEAN
  /\ lastAction \in {
       "init", "admit-active-operation", "queue-operation-behind-active",
       "begin-dispose", "finish-accepted-commit", "publish-accepted-commit",
       "reconcile-accepted-commit", "reject-captured-queue",
       "reject-post-seal", "attempt-session-retirement",
       "retry-session-retirement"
     }

AcceptedOperationIsRemembered ==
  (activeOperation # "idle") <=> activeWasAccepted

RetirementRequiresCapturedTail ==
  retirement # "idle" => CapturedTailDrained

AcceptedOperationPrecedesRetirement ==
  sessionsRetired /\ activeWasAccepted => activeOperation = "reconciled"

CompletedDisposalRetiresCapabilities ==
  /\ sessionsRetired = attachmentsRetired
  /\ (gate = "complete") <=>
       (retirement = "complete" /\ sessionsRetired /\ attachmentsRetired)

QueuedOperationIsRejectedBeforeCompletion ==
  gate = "complete" => queuedOperation # "queued"

StartedDisposalEventuallyCompletes ==
  gate = "draining" ~> gate = "complete"

=============================================================================
