-------------------- MODULE TerminalSemanticRead --------------------
EXTENDS Naturals, TLC

\* Bounded abstraction for one Desktop semantic Terminal output read.
\* Terminal contents, MessagePort transport, semantic-command receipt
\* retention, and archive persistence remain outside this model.

CONSTANT MaxGeneration, MaxRevision

Phases == {
  "absent", "opening", "waiting-frame", "ready",
  "waiting-rows", "cleaning", "settled"
}
Results == {
  "none", "success", "stale", "cancelled",
  "unavailable", "target-retired"
}

VARIABLES
  phase,
  targetCurrent,
  leaseAcquired,
  readCapability,
  inputCapability,
  resizeCapability,
  frameGeneration,
  frameRevision,
  requestGeneration,
  requestRevision,
  pendingResult,
  pendingText,
  leaseDisposed,
  publishedResult,
  publishedText,
  settlementCount,
  lastAction

vars == <<phase, targetCurrent, leaseAcquired, readCapability,
  inputCapability, resizeCapability, frameGeneration, frameRevision,
  requestGeneration, requestRevision, pendingResult, pendingText,
  leaseDisposed, publishedResult, publishedText, settlementCount,
  lastAction>>

Init ==
  /\ phase = "absent"
  /\ targetCurrent = TRUE
  /\ leaseAcquired = FALSE
  /\ readCapability = FALSE
  /\ inputCapability = FALSE
  /\ resizeCapability = FALSE
  /\ frameGeneration = 0
  /\ frameRevision = 0
  /\ requestGeneration = 0
  /\ requestRevision = 0
  /\ pendingResult = "none"
  /\ pendingText = FALSE
  /\ leaseDisposed = FALSE
  /\ publishedResult = "none"
  /\ publishedText = FALSE
  /\ settlementCount = 0
  /\ lastAction = "init"

BeginRead ==
  /\ phase = "absent"
  /\ phase' = "opening"
  /\ UNCHANGED <<targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, frameGeneration, frameRevision,
       requestGeneration, requestRevision, pendingResult, pendingText,
       leaseDisposed, publishedResult, publishedText, settlementCount>>
  /\ lastAction' = "begin-read"

OpenReadOnlyLease ==
  /\ phase = "opening"
  /\ targetCurrent
  /\ phase' = "waiting-frame"
  /\ leaseAcquired' = TRUE
  /\ readCapability' = TRUE
  /\ inputCapability' = FALSE
  /\ resizeCapability' = FALSE
  /\ UNCHANGED <<targetCurrent, frameGeneration, frameRevision,
       requestGeneration, requestRevision, pendingResult, pendingText,
       leaseDisposed, publishedResult, publishedText, settlementCount>>
  /\ lastAction' = "open-read-only-lease"

OpenUnavailable ==
  /\ phase = "opening"
  /\ phase' = "cleaning"
  /\ pendingResult' = "unavailable"
  /\ pendingText' = FALSE
  /\ UNCHANGED <<targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, frameGeneration, frameRevision,
       requestGeneration, requestRevision, leaseDisposed, publishedResult,
       publishedText, settlementCount>>
  /\ lastAction' = "open-unavailable"

ObserveFrame(generation, revision) ==
  /\ phase = "waiting-frame"
  /\ generation \in 1..MaxGeneration
  /\ revision \in 1..MaxRevision
  /\ phase' = "ready"
  /\ frameGeneration' = generation
  /\ frameRevision' = revision
  /\ UNCHANGED <<targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, requestGeneration,
       requestRevision, pendingResult, pendingText, leaseDisposed,
       publishedResult, publishedText, settlementCount>>
  /\ lastAction' = "observe-frame"

RequestRows ==
  /\ phase = "ready"
  /\ phase' = "waiting-rows"
  /\ requestGeneration' = frameGeneration
  /\ requestRevision' = frameRevision
  /\ UNCHANGED <<targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, frameGeneration, frameRevision,
       pendingResult, pendingText, leaseDisposed, publishedResult,
       publishedText, settlementCount>>
  /\ lastAction' = "request-rows"

AdvanceFrame ==
  /\ phase = "waiting-rows"
  /\ \/ /\ frameRevision < MaxRevision
        /\ frameRevision' = frameRevision + 1
        /\ UNCHANGED frameGeneration
     \/ /\ frameGeneration < MaxGeneration
        /\ frameGeneration' = frameGeneration + 1
        /\ frameRevision' = 1
  /\ UNCHANGED <<phase, targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, requestGeneration,
       requestRevision, pendingResult, pendingText, leaseDisposed,
       publishedResult, publishedText, settlementCount>>
  /\ lastAction' = "advance-frame"

RowsMatched ==
  /\ phase = "waiting-rows"
  /\ targetCurrent
  /\ requestGeneration = frameGeneration
  /\ requestRevision = frameRevision
  /\ phase' = "cleaning"
  /\ pendingResult' = "success"
  /\ pendingText' = TRUE
  /\ UNCHANGED <<targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, frameGeneration, frameRevision,
       requestGeneration, requestRevision, leaseDisposed, publishedResult,
       publishedText, settlementCount>>
  /\ lastAction' = "rows-matched"

RowsStale ==
  /\ phase = "waiting-rows"
  /\ \/ requestGeneration # frameGeneration
     \/ requestRevision # frameRevision
  /\ phase' = "cleaning"
  /\ pendingResult' = "stale"
  /\ pendingText' = FALSE
  /\ UNCHANGED <<targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, frameGeneration, frameRevision,
       requestGeneration, requestRevision, leaseDisposed, publishedResult,
       publishedText, settlementCount>>
  /\ lastAction' = "rows-stale"

Cancel ==
  /\ phase \in {"opening", "waiting-frame", "ready", "waiting-rows", "cleaning"}
  /\ phase' = "cleaning"
  /\ pendingResult' = "cancelled"
  /\ pendingText' = FALSE
  /\ UNCHANGED <<targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, frameGeneration, frameRevision,
       requestGeneration, requestRevision, leaseDisposed, publishedResult,
       publishedText, settlementCount>>
  /\ lastAction' = "cancel"

TargetRetired ==
  /\ targetCurrent
  /\ phase \in {"opening", "waiting-frame", "ready", "waiting-rows", "cleaning"}
  /\ targetCurrent' = FALSE
  /\ phase' = "cleaning"
  /\ pendingResult' = "target-retired"
  /\ pendingText' = FALSE
  /\ UNCHANGED <<leaseAcquired, readCapability, inputCapability,
       resizeCapability, frameGeneration, frameRevision, requestGeneration,
       requestRevision, leaseDisposed, publishedResult, publishedText,
       settlementCount>>
  /\ lastAction' = "target-retired"

DisposeLease ==
  /\ phase = "cleaning"
  /\ leaseAcquired
  /\ ~leaseDisposed
  /\ leaseDisposed' = TRUE
  /\ UNCHANGED <<phase, targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, frameGeneration, frameRevision,
       requestGeneration, requestRevision, pendingResult, pendingText,
       publishedResult, publishedText, settlementCount>>
  /\ lastAction' = "dispose-lease"

PublishResult ==
  /\ phase = "cleaning"
  /\ pendingResult # "none"
  /\ ~leaseAcquired \/ leaseDisposed
  /\ phase' = "settled"
  /\ publishedResult' = pendingResult
  /\ publishedText' = pendingText
  /\ settlementCount' = settlementCount + 1
  /\ UNCHANGED <<targetCurrent, leaseAcquired, readCapability,
       inputCapability, resizeCapability, frameGeneration, frameRevision,
       requestGeneration, requestRevision, pendingResult, pendingText,
       leaseDisposed>>
  /\ lastAction' = "publish-result"

Next ==
  \/ BeginRead
  \/ OpenReadOnlyLease
  \/ OpenUnavailable
  \/ \E generation \in 1..MaxGeneration, revision \in 1..MaxRevision:
       ObserveFrame(generation, revision)
  \/ RequestRows
  \/ AdvanceFrame
  \/ RowsMatched
  \/ RowsStale
  \/ Cancel
  \/ TargetRetired
  \/ DisposeLease
  \/ PublishResult

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in Phases
  /\ targetCurrent \in BOOLEAN
  /\ leaseAcquired \in BOOLEAN
  /\ readCapability \in BOOLEAN
  /\ inputCapability \in BOOLEAN
  /\ resizeCapability \in BOOLEAN
  /\ frameGeneration \in 0..MaxGeneration
  /\ frameRevision \in 0..MaxRevision
  /\ requestGeneration \in 0..MaxGeneration
  /\ requestRevision \in 0..MaxRevision
  /\ pendingResult \in Results
  /\ pendingText \in BOOLEAN
  /\ leaseDisposed \in BOOLEAN
  /\ publishedResult \in Results
  /\ publishedText \in BOOLEAN
  /\ settlementCount \in 0..1

LeaseIsCaptureOnly ==
  leaseAcquired =>
    /\ readCapability
    /\ ~inputCapability
    /\ ~resizeCapability

PublishedSuccessIsExact ==
  publishedResult = "success" =>
    /\ targetCurrent
    /\ leaseAcquired
    /\ readCapability
    /\ requestGeneration = frameGeneration
    /\ requestRevision = frameRevision
    /\ publishedText

NonSuccessNeverPublishesText ==
  publishedResult # "success" => ~publishedText

AcquiredLeaseIsDisposedBeforeSettlement ==
  phase = "settled" /\ leaseAcquired => leaseDisposed

SettlementIsSingleAndTerminal ==
  /\ settlementCount <= 1
  /\ (phase = "settled") = (settlementCount = 1)
  /\ settlementCount = 1 => publishedResult # "none"

====================================================================
