------------------------- MODULE NativeClientCloseControl -------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Scenario, MaxReads, MaxResumes, DisposedQuota
ASSUME /\ Scenario \in {"command", "resume"}
       /\ MaxReads \in 2..3 /\ MaxResumes \in 0..2 /\ DisposedQuota = 2

Kinds == {"none", "close", "read", "resume"}
Statuses == {"none", "closing", "blocked", "blocked-fixed", "unknown",
             "disposed-active", "disposed-parked", "disposed-fixed", "closed"}
DisposedStatuses == {"disposed-active", "disposed-parked", "disposed-fixed", "closed"}
ResumableStatuses == {"blocked", "disposed-parked"}
TransportStages == {"queued", "handed-off", "result-ready"}
PendingStages == TransportStages \cup {"await-ui"}
MaxKeys == 1 + MaxReads + MaxResumes
NoKnowledge == [operationKnown |-> FALSE, generation |-> 0, status |-> "none",
                disposed |-> FALSE, revision |-> 0]
Known(g, status) == [operationKnown |-> TRUE, generation |-> g, status |-> status,
                    disposed |-> status \in DisposedStatuses,
                    revision |-> IF status = "closed" THEN 2 ELSE 0]
KnowledgeType == [operationKnown: BOOLEAN, generation: 0..3, status: Statuses,
                  disposed: BOOLEAN, revision: {0,2}]

VARIABLE s
vars == <<s>>

\* Current-key receipts replace dead lifetime key sets. Fresh admission alone
\* advances key; SameKeyNeverReentersQueue preserves historical no-replay.
Init ==
  \E status \in IF Scenario = "command" THEN {"none"}
                ELSE {"blocked", "disposed-active", "disposed-parked"},
     alias \in IF Scenario = "command" THEN BOOLEAN ELSE {FALSE},
     predecessor \in BOOLEAN:
    LET knowledge == IF Scenario = "command" THEN NoKnowledge ELSE Known(1,status)
    IN s = [
      stage |-> "idle", carrier |-> "original", workerReturned |-> FALSE,
      workerIdentity |-> 1, returnedWorker |-> 0,
      key |-> 0, kind |-> "none", age |-> 0, lockFree |-> TRUE,
      predecessor |-> predecessor, predecessorAge |-> 0,
      prompt |-> "none", promptKind |-> "none", promptGeneration |-> 0,
      promptVersion |-> 0, displayVersion |-> 0, promptCount |-> 0,
      confirmedKey |-> 0, submittedKey |-> 0,
      closeCount |-> 0, resumeCount |-> 0, readCount |-> 0,
      manual |-> FALSE, readMode |-> "none", intervalDue |-> TRUE,
      quota |-> 0, quotaGeneration |-> knowledge.generation,
      knowledge |-> knowledge, displayed |-> knowledge,
      uncertain |-> FALSE, alias |-> alias,
      admittedBase |-> IF alias THEN 2 ELSE 1,
      canonicalOperation |-> IF knowledge.operationKnown THEN "P" ELSE "none",
      acceptedBase |-> IF knowledge.operationKnown THEN 1 ELSE 0,
      receiptKnown |-> knowledge.operationKnown, initialReadPending |-> FALSE,
      expectedGeneration |-> 0, serverGeneration |-> IF knowledge.operationKnown THEN 1 ELSE 0,
      resultKey |-> 0, resultKind |-> "none", reconnections |-> 0,
      membershipExport |-> FALSE, forbiddenEffects |-> 0, freshReadSettled |-> FALSE,
      statusDrawn |-> Scenario = "resume"
    ]

Connected == s.carrier \in {"original", "status-only", "confirmed-control"}
Idle == Connected /\ s.stage = "idle"
CanMutate == Idle /\ (s.carrier = "original"
                      \/ (s.carrier = "status-only" /\ s.freshReadSettled /\ s.statusDrawn))
BeforeCutoff == s.age < 1
CanRead ==
  Idle /\ s.prompt = "none" /\ s.readCount < MaxReads /\ s.intervalDue
  /\ (s.knowledge.operationKnown \/ s.uncertain)
AutoEligible ==
  s.knowledge.status = "closing"
  \/ (s.knowledge.status = "disposed-active" /\ s.quota < DisposedQuota)
InitialReadEligible == s.initialReadPending
UnansweredMutation(state) ==
  state.stage = "handed-off" /\ state.kind \in {"close","resume"}
  /\ state.submittedKey = state.key
Retired(state) ==
  [state EXCEPT !.stage = "retired", !.carrier = "await-join", !.prompt = "none",
       !.promptKind = "none", !.promptGeneration = 0, !.promptVersion = 0,
       !.resultKey = 0, !.resultKind = "none", !.readMode = "none",
       !.manual = FALSE, !.lockFree = TRUE, !.freshReadSettled = FALSE,
       !.displayVersion = 0, !.initialReadPending = FALSE, !.statusDrawn = FALSE,
       !.age = 2, !.uncertain = state.uncertain \/ UnansweredMutation(state)]
CurrentPrompt ==
  s.prompt = "shown" /\ s.promptVersion = s.displayVersion
  /\ (s.promptKind = "close"
      \/ (s.promptGeneration = s.displayed.generation
          /\ s.displayed.status \in ResumableStatuses /\ s.statusDrawn))
NextKnowledge(g, status) ==
  [Known(g,status) EXCEPT
    !.disposed = s.knowledge.disposed \/ @,
    !.revision = IF s.knowledge.revision # 0 THEN s.knowledge.revision ELSE @]
ValidStatus(g,status) ==
  /\ g \in 1..3 /\ status \in Statuses \ {"none"}
  /\ g >= s.knowledge.generation
  /\ (s.knowledge.disposed => status \in DisposedStatuses)
  /\ (s.knowledge.revision # 0 => status = "closed")
  /\ (s.knowledge.status = "unknown" => status = "unknown" /\ g = s.knowledge.generation)

ClockAdvance ==
  \* 2 is the ended transport budget, including arbitrarily late UI consumption.
  \* Deposit/retirement map directly to 2; only active transport advances 0 -> 1.
  /\ s.stage \in TransportStages /\ s.age = 0
  /\ s' = [s EXCEPT !.age = @ + 1]
EarlierClockAdvance ==
  /\ Connected /\ s.predecessor /\ s.predecessorAge = 0
  /\ s' = [s EXCEPT !.predecessorAge = 1]
IntervalBecomesDue ==
  /\ Connected /\ ~s.intervalDue
  /\ s' = [s EXCEPT !.intervalDue = TRUE]
HoldMailboxLock ==
  /\ Connected /\ s.lockFree
  /\ s' = [s EXCEPT !.lockFree = FALSE]
ReleaseMailboxLock ==
  /\ Connected /\ ~s.lockFree
  /\ s' = [s EXCEPT !.lockFree = TRUE]

OpenConfirmation(kind) ==
  /\ CanMutate /\ s.prompt = "none" /\ s.promptCount < 2
  /\ kind \in {"close","resume"}
  /\ IF kind = "close"
     THEN Scenario = "command" /\ s.closeCount = 0 /\ ~s.knowledge.operationKnown
          /\ s.carrier = "original"
     ELSE s.resumeCount < MaxResumes /\ s.displayed.status \in ResumableStatuses
          /\ s.statusDrawn
  /\ s' = [s EXCEPT !.prompt = "unshown", !.promptKind = kind,
       !.promptGeneration = s.displayed.generation, !.promptVersion = s.displayVersion,
       !.promptCount = @ + 1]
RenderExactConfirmation ==
  /\ Connected /\ s.prompt = "unshown"
  /\ s.promptKind = "resume" => s.statusDrawn
  /\ s' = [s EXCEPT !.prompt = "shown"]
RevokePromptVisibility ==
  /\ Connected /\ s.prompt = "shown"
  /\ s' = [s EXCEPT !.prompt = "unshown"]
RenderExactStatus ==
  /\ Idle /\ ~s.statusDrawn
  /\ s.displayed.status \in ResumableStatuses
  /\ s' = [s EXCEPT !.statusDrawn = TRUE]
RevokeStatusVisibility ==
  /\ Idle /\ s.statusDrawn
  /\ s' = [s EXCEPT !.statusDrawn = FALSE,
       !.prompt = IF s.promptKind = "resume" /\ @ = "shown" THEN "unshown" ELSE @]
ObserveGeometryChange ==
  /\ Connected /\ (s.prompt = "shown" \/ s.statusDrawn)
  /\ s' = [s EXCEPT !.prompt = IF @ = "shown" THEN "unshown" ELSE @,
                    !.statusDrawn = FALSE]
CancelConfirmation ==
  /\ s.prompt # "none"
  /\ s' = [s EXCEPT !.prompt = "none", !.promptKind = "none",
                    !.promptGeneration = 0, !.promptVersion = 0, !.displayVersion = 0]
RejectUnseenConfirmation ==
  /\ s.prompt = "unshown"
  /\ UNCHANGED vars
InvalidateConfirmation ==
  /\ Connected /\ s.prompt # "none" /\ s.promptVersion = s.displayVersion
  /\ s' = [s EXCEPT !.displayVersion = 1]
RejectStaleConfirmation ==
  /\ s.prompt = "shown" /\ ~CurrentPrompt
  /\ s' = [s EXCEPT !.prompt = "none", !.promptKind = "none",
                    !.promptGeneration = 0, !.promptVersion = 0, !.displayVersion = 0]
RefuseAdmissionWithoutAllocation ==
  /\ Idle /\ (s.prompt # "none" \/ s.manual)
  /\ UNCHANGED vars
ConfirmAndAdmit ==
  /\ CanMutate /\ CurrentPrompt /\ s.lockFree /\ s.key < MaxKeys
  /\ s' = [s EXCEPT !.stage = "queued", !.key = @ + 1, !.kind = s.promptKind,
       !.age = 0, !.prompt = "none", !.resultKey = 0, !.resultKind = "none",
       !.promptKind = "none", !.promptGeneration = 0, !.promptVersion = 0,
       !.confirmedKey = s.key+1, !.submittedKey = 0,
       !.expectedGeneration = s.promptGeneration,
       !.closeCount = @ + (IF s.promptKind = "close" THEN 1 ELSE 0),
       !.resumeCount = @ + (IF s.promptKind = "resume" THEN 1 ELSE 0),
       !.carrier = IF s.carrier = "status-only" THEN "confirmed-control" ELSE @,
       !.freshReadSettled = FALSE, !.statusDrawn = FALSE]
ConfirmRecoveredResume ==
  /\ s.carrier = "status-only" /\ s.promptKind = "resume" /\ ConfirmAndAdmit
SettleEarlierCausalWork ==
  /\ Connected /\ s.predecessor /\ s.predecessorAge = 0
  /\ s' = [s EXCEPT !.predecessor = FALSE]
ExpireEarlierCausalWork ==
  /\ Connected /\ s.predecessor /\ s.predecessorAge = 1
  /\ s' = Retired(s)

RequestManualRead ==
  /\ Idle /\ s.prompt = "none" /\ ~s.manual
  /\ s' = [s EXCEPT !.manual = TRUE]
AdmitRead(mode) ==
  /\ CanRead /\ s.lockFree /\ s.key < MaxKeys
  /\ mode \in {"initial","auto","manual"}
  /\ CASE mode = "initial" -> InitialReadEligible /\ ~s.manual
       [] mode = "auto" -> AutoEligible /\ ~s.manual
       [] OTHER -> s.manual
  /\ s' = [s EXCEPT !.stage = "queued", !.key = @ + 1, !.kind = "read",
       !.age = 0, !.resultKey = 0, !.resultKind = "none",
       !.confirmedKey = 0, !.submittedKey = 0,
       !.readMode = mode, !.readCount = @ + 1, !.intervalDue = FALSE,
       !.manual = FALSE, !.initialReadPending = FALSE, !.statusDrawn = FALSE,
       !.quota = @ + (IF mode = "auto" /\ s.knowledge.status = "disposed-active"
                      THEN 1 ELSE 0)]
AdmitDisposedAutoRead ==
  /\ s.knowledge.status = "disposed-active" /\ AdmitRead("auto")
AdmitManualWithoutQuotaRenewal ==
  /\ s.quota = DisposedQuota /\ AdmitRead("manual")
RejectExhaustedDisposedAutoRead ==
  /\ Idle /\ s.knowledge.status = "disposed-active" /\ s.quota = DisposedQuota
  /\ UNCHANGED vars
SubmitOnce ==
  /\ s.stage = "queued" /\ BeforeCutoff /\ ~s.predecessor
  /\ s.submittedKey # s.key
  /\ s' = [s EXCEPT !.stage = "handed-off",
       !.submittedKey = IF s.kind = "read" THEN 0 ELSE s.key]
ReceiveDefinitelyNotSubmitted ==
  /\ s.stage = "queued" /\ BeforeCutoff /\ ~s.predecessor
  /\ s' = [s EXCEPT !.stage = "result-ready", !.resultKey = s.key,
                    !.resultKind = "not-submitted"]
ReceivePrecommit ==
  /\ s.stage = "handed-off" /\ s.kind = "close" /\ BeforeCutoff
  /\ s' = [s EXCEPT !.stage = "result-ready", !.resultKey = s.key,
                    !.resultKind = "precommit"]
ReceiveAcceptance ==
  /\ s.stage = "handed-off" /\ s.kind = "close" /\ BeforeCutoff
  /\ s' = [s EXCEPT !.stage = "result-ready", !.resultKey = s.key,
       !.resultKind = "accepted", !.knowledge = Known(1,"none"),
       !.canonicalOperation = "P", !.acceptedBase = 1,
       !.receiptKnown = TRUE, !.initialReadPending = TRUE,
       !.quotaGeneration = 1, !.serverGeneration = 1]
ReceiveAliasAcceptance ==
  /\ s.alias /\ ReceiveAcceptance
ReceivePrimaryAcceptance ==
  /\ ~s.alias /\ ReceiveAcceptance

ReceiveAvailableStatus(g,status) ==
  /\ s.stage = "handed-off" /\ s.kind = "read" /\ BeforeCutoff
  /\ ValidStatus(g,status)
  /\ s' = [s EXCEPT !.stage = "result-ready", !.resultKey = s.key,
       !.resultKind = "available", !.knowledge = NextKnowledge(g,status),
       !.canonicalOperation = "P", !.uncertain = FALSE,
       !.quota = IF g > s.quotaGeneration THEN 0 ELSE @,
       !.quotaGeneration = g, !.serverGeneration = g]
ReceiveDisposedStatus ==
  /\ \E g \in 1..3: ReceiveAvailableStatus(g,"disposed-active")
ReceiveClosedStatus ==
  /\ \E g \in 1..3: ReceiveAvailableStatus(g,"closed")
ReceiveHigherAttempt ==
  /\ \E g \in 1..3, status \in Statuses:
       g > s.knowledge.generation /\ ReceiveAvailableStatus(g,status)
ReceiveStatusWithoutCommandReceipt ==
  /\ ~s.receiptKnown
  /\ \E g \in 1..3, status \in Statuses: ReceiveAvailableStatus(g,status)
ReceiveReadUnavailable ==
  /\ s.stage = "handed-off" /\ s.kind = "read" /\ BeforeCutoff
  /\ s' = [s EXCEPT !.stage = "result-ready", !.resultKey = s.key,
                    !.resultKind = "read-failed"]
ReceiveNotClosingAfterAmbiguity ==
  /\ s.stage = "handed-off" /\ s.kind = "read" /\ BeforeCutoff
  /\ s.uncertain /\ ~s.knowledge.operationKnown
  /\ s' = [s EXCEPT !.stage = "result-ready", !.resultKey = s.key,
                    !.resultKind = "not-closing"]
RejectContradictoryRead ==
  /\ s.stage = "handed-off" /\ s.kind = "read" /\ s.knowledge.operationKnown
  /\ s' = Retired(s)
RejectForeignTargetOrOperation ==
  /\ s.stage = "handed-off"
  /\ s' = Retired(s)

ExternalAttemptAdvance ==
  /\ Connected /\ s.knowledge.operationKnown /\ s.serverGeneration < 3
  /\ s.knowledge.status \in ResumableStatuses
  /\ s' = [s EXCEPT !.serverGeneration = @ + 1]
ReceiveResumeAcknowledgment ==
  /\ s.stage = "handed-off" /\ s.kind = "resume" /\ BeforeCutoff
  /\ s.expectedGeneration = s.serverGeneration /\ s.serverGeneration < 3
  /\ LET g == s.serverGeneration + 1 IN
     s' = [s EXCEPT !.stage = "result-ready", !.resultKey = s.key,
       !.resultKind = "resumed", !.serverGeneration = g,
       !.knowledge = [s.knowledge EXCEPT !.generation = g, !.status = "none"],
       !.quota = 0, !.quotaGeneration = g, !.initialReadPending = TRUE]
ReceiveResumeStale ==
  /\ s.stage = "handed-off" /\ s.kind = "resume" /\ BeforeCutoff
  /\ s.expectedGeneration < s.serverGeneration
  /\ s' = [s EXCEPT !.stage = "result-ready", !.resultKey = s.key,
       !.resultKind = "stale",
       !.knowledge = [s.knowledge EXCEPT !.generation = s.serverGeneration, !.status = "none"],
       !.quota = 0, !.quotaGeneration = s.serverGeneration, !.initialReadPending = TRUE]
ReceiveResumeRefusal ==
  /\ s.stage = "handed-off" /\ s.kind = "resume" /\ BeforeCutoff
  /\ s' = [s EXCEPT !.stage = "result-ready", !.resultKey = s.key,
                    !.resultKind = "resume-refused"]

LoseMutationReply ==
  /\ s.stage = "handed-off" /\ s.kind \in {"close","resume"}
  /\ s' = [Retired(s) EXCEPT !.uncertain = TRUE]
LoseReadReply ==
  /\ s.stage = "handed-off" /\ s.kind = "read"
  /\ s' = Retired(s)
DepositTypedResult ==
  /\ s.stage = "result-ready" /\ BeforeCutoff /\ s.lockFree
  /\ s.resultKey = s.key
  /\ s' = [s EXCEPT !.stage = "await-ui", !.age = 2]
ConsumeExactResult ==
  /\ s.stage = "await-ui" /\ s.resultKey = s.key
  /\ s' = [s EXCEPT !.stage = "idle", !.displayed = s.knowledge,
       !.displayVersion = 0,
       !.resultKey = 0, !.resultKind = "none", !.readMode = "none",
       !.expectedGeneration = 0,
       !.carrier = IF s.carrier = "confirmed-control" THEN "status-only" ELSE @,
       !.freshReadSettled = s.carrier = "status-only" /\ s.kind = "read"
          /\ s.resultKind = "available" /\ s.knowledge.status \in ResumableStatuses,
       !.statusDrawn = FALSE]
ConsumeAfterTransportCutoff ==
  /\ s.age >= 1 /\ ConsumeExactResult
IgnoreStaleUiEvent ==
  /\ s.stage = "await-ui" /\ s.key > 0
  /\ UNCHANGED vars
ExpireUndepositedFlight ==
  /\ s.stage \in TransportStages /\ ~BeforeCutoff
  /\ s' = Retired(s)
ExpireSubmittedMutation ==
  /\ UnansweredMutation(s) /\ ExpireUndepositedFlight
RequestRetirement ==
  /\ Connected
  /\ s' = Retired(s)
RetireSubmittedMutation ==
  /\ UnansweredMutation(s) /\ RequestRetirement
RetireRecoveredCarrierBeforeReturn ==
  /\ s.workerIdentity = 2 /\ ~s.workerReturned /\ RequestRetirement
WorkerReturns ==
  /\ s.carrier = "await-join" /\ ~s.workerReturned
  /\ s' = [s EXCEPT !.workerReturned = TRUE, !.returnedWorker = s.workerIdentity]
RejectOldWorkerCompletion ==
  /\ s.carrier = "await-join" /\ s.workerIdentity = 2
  /\ s.returnedWorker = 1 /\ ~s.workerReturned
  /\ UNCHANGED vars
JoinCompletedWorker ==
  /\ s.carrier = "await-join" /\ s.workerReturned
  /\ s.returnedWorker = s.workerIdentity
  /\ s' = [s EXCEPT !.carrier = "joined", !.displayed = s.knowledge]
JoinRecoveredWorker ==
  /\ s.workerIdentity = 2 /\ JoinCompletedWorker
ExplicitStatusReconnect ==
  /\ s.carrier = "joined" /\ s.reconnections = 0
  /\ s.knowledge.operationKnown \/ s.uncertain
  /\ s' = [s EXCEPT !.carrier = "status-only", !.stage = "idle",
       !.reconnections = 1, !.manual = TRUE, !.predecessor = FALSE,
       !.workerIdentity = @ + 1, !.workerReturned = FALSE,
       !.freshReadSettled = FALSE, !.statusDrawn = FALSE]
RejectMismatchedRecovery ==
  /\ s.carrier = "joined"
  /\ UNCHANGED vars
ExportKnownClosed ==
  /\ s.stage = "idle" /\ s.displayed.revision = 2
  /\ ~s.membershipExport
  /\ s' = [s EXCEPT !.membershipExport = TRUE]

Next ==
  \/ ClockAdvance \/ EarlierClockAdvance \/ IntervalBecomesDue
  \/ HoldMailboxLock \/ ReleaseMailboxLock
  \/ \E kind \in {"close","resume"}: OpenConfirmation(kind)
  \/ RenderExactConfirmation \/ CancelConfirmation \/ RejectUnseenConfirmation
  \/ RevokePromptVisibility \/ RenderExactStatus \/ RevokeStatusVisibility
  \/ ObserveGeometryChange
  \/ InvalidateConfirmation \/ RejectStaleConfirmation \/ RefuseAdmissionWithoutAllocation
  \/ ConfirmAndAdmit \/ ConfirmRecoveredResume \/ SettleEarlierCausalWork \/ ExpireEarlierCausalWork
  \/ RequestManualRead \/ \E mode \in {"initial","auto","manual"}: AdmitRead(mode)
  \/ AdmitDisposedAutoRead \/ AdmitManualWithoutQuotaRenewal \/ RejectExhaustedDisposedAutoRead
  \/ SubmitOnce \/ ReceiveDefinitelyNotSubmitted \/ ReceivePrecommit
  \/ ReceiveAliasAcceptance \/ ReceivePrimaryAcceptance
  \/ \E g \in 1..3, status \in Statuses: ReceiveAvailableStatus(g,status)
  \/ ReceiveDisposedStatus \/ ReceiveClosedStatus \/ ReceiveHigherAttempt
  \/ ReceiveStatusWithoutCommandReceipt
  \/ ReceiveReadUnavailable \/ ReceiveNotClosingAfterAmbiguity
  \/ RejectContradictoryRead \/ RejectForeignTargetOrOperation
  \/ ExternalAttemptAdvance \/ ReceiveResumeAcknowledgment \/ ReceiveResumeStale
  \/ ReceiveResumeRefusal \/ LoseMutationReply \/ LoseReadReply
  \/ DepositTypedResult \/ ConsumeExactResult \/ ConsumeAfterTransportCutoff
  \/ IgnoreStaleUiEvent \/ ExpireUndepositedFlight \/ RequestRetirement
  \/ ExpireSubmittedMutation \/ RetireSubmittedMutation \/ RetireRecoveredCarrierBeforeReturn
  \/ WorkerReturns \/ JoinCompletedWorker \/ ExplicitStatusReconnect
  \/ RejectOldWorkerCompletion \/ JoinRecoveredWorker
  \/ RejectMismatchedRecovery \/ ExportKnownClosed

Spec == Init /\ [][Next]_vars
  /\ WF_vars(ClockAdvance) /\ WF_vars(EarlierClockAdvance)
  /\ WF_vars(ReleaseMailboxLock) /\ WF_vars(ExpireUndepositedFlight)
  /\ WF_vars(ExpireEarlierCausalWork) /\ WF_vars(ConsumeExactResult)
  /\ WF_vars(JoinCompletedWorker)

TypeOK ==
  /\ s.stage \in {"idle","retired"} \cup PendingStages
  /\ s.carrier \in {"original","await-join","joined","status-only","confirmed-control"}
  /\ s.key \in 0..MaxKeys /\ s.kind \in Kinds /\ s.age \in 0..2
  /\ s.knowledge \in KnowledgeType /\ s.displayed \in KnowledgeType
  /\ s.quota \in 0..DisposedQuota /\ s.quotaGeneration \in 0..3
  /\ s.readCount \in 0..MaxReads /\ s.resumeCount \in 0..MaxResumes
  /\ s.closeCount \in 0..1 /\ s.promptCount \in 0..2
  /\ s.resultKey \in 0..MaxKeys /\ s.serverGeneration \in 0..3
  /\ s.confirmedKey \in 0..MaxKeys /\ s.submittedKey \in 0..MaxKeys
  /\ s.workerIdentity \in 1..2 /\ s.returnedWorker \in 0..2
CurrentWorkerReturnHasExactIdentity ==
  s.workerReturned => s.returnedWorker = s.workerIdentity
JoinedCarrierHasCurrentReturn ==
  s.carrier = "joined" => s.workerReturned /\ s.returnedWorker = s.workerIdentity
NoUnconfirmedMutation ==
  s.kind \in {"close","resume"} => s.confirmedKey = s.key
NoMutationReplay ==
  /\ s.submittedKey # 0 =>
       s.submittedKey = s.key /\ s.confirmedKey = s.key /\ s.stage # "queued"
  /\ s.carrier = "status-only" => s.kind \notin {"close","resume"} \/ s.stage = "idle"
ExactResultFence ==
  s.stage \in {"result-ready","await-ui"} => s.resultKey = s.key
PendingUiIsUntimed ==
  s.stage = "await-ui" => ~ENABLED ExpireUndepositedFlight
AliasHasCanonicalPrimary ==
  /\ s.knowledge.operationKnown => s.canonicalOperation = "P"
  /\ IF s.receiptKnown THEN s.acceptedBase = 1 /\ s.acceptedBase <= s.admittedBase
     ELSE s.acceptedBase = 0
KnownFactsAreCoherent ==
  /\ s.knowledge.revision = 2 => s.knowledge.disposed /\ s.knowledge.operationKnown
  /\ s.displayed.revision = 2 => s.displayed.disposed /\ s.displayed.operationKnown
  /\ s.knowledge.disposed => s.knowledge.operationKnown
UnknownIsNotResumable ==
  s.promptKind = "resume" /\ s.prompt # "none" =>
    s.displayed.status \in ResumableStatuses
StatusRecoveryNeverBootstraps == s.forbiddenEffects = 0
RecoveredControlIsOneConfirmedResume ==
  s.carrier = "confirmed-control" =>
    s.kind = "resume" /\ s.confirmedKey = s.key /\ s.stage \in PendingStages
ClosedExportIsExact ==
  s.membershipExport => s.displayed.revision = 2 /\ s.displayed.disposed
                        /\ s.canonicalOperation = "P"
QuotaExhaustionBlocksOnlyAutomaticDisposed ==
  s.knowledge.status = "disposed-active" /\ s.quota = DisposedQuota => ~AutoEligible

CancelPreservesCore == [][
  CancelConfirmation => UNCHANGED <<s.key,s.knowledge,s.displayed,s.submittedKey>>
]_vars
ConfirmedAdmissionRequiresDraw == [][
  s'.key > s.key /\ s'.kind \in {"close","resume"} => CurrentPrompt
    /\ (s.carrier = "original" \/ (s.carrier = "status-only" /\ s.freshReadSettled /\ s.statusDrawn))
]_vars
EachSubmissionIsNew == [][
  s'.submittedKey # 0 /\ s'.submittedKey # s.submittedKey =>
    s.submittedKey = 0 /\ s.stage = "queued" /\ s'.key = s.key
]_vars
SameKeyNeverReentersQueue == [][
  s'.key = s.key /\ s'.stage = "queued" => s.stage = "queued"
]_vars
SubmissionKeepsExactKey == [][
  SubmitOnce => s'.key = s.key /\ s'.stage = "handed-off"
]_vars
KnownKnowledgeNeverRegresses == [][
  /\ s.knowledge.operationKnown => s'.knowledge.operationKnown
  /\ s.knowledge.disposed => s'.knowledge.disposed
  /\ s.knowledge.revision # 0 => s'.knowledge.revision = s.knowledge.revision
  /\ s'.knowledge.generation >= s.knowledge.generation
]_vars
DisplayedFactsNeverRegress == [][
  /\ s.displayed.operationKnown => s'.displayed.operationKnown
  /\ s.displayed.disposed => s'.displayed.disposed
  /\ s.displayed.revision # 0 => s'.displayed.revision = s.displayed.revision
]_vars
SameAttemptDoesNotRenewQuota == [][
  s'.quotaGeneration = s.quotaGeneration => s'.quota >= s.quota
]_vars
QuotaChargeIsAdmission == [][
  s'.quota > s.quota =>
    s'.key = s.key+1 /\ s'.stage = "queued" /\ s'.kind = "read"
    /\ s'.readMode = "auto" /\ s.knowledge.status = "disposed-active"
]_vars
HigherAttemptAloneRenewsQuota == [][
  s'.quota < s.quota => s'.quotaGeneration > s.quotaGeneration
]_vars
TransportAgeNeverRenewsForSameKey == [][
  s'.key = s.key => s'.age >= s.age
]_vars
AdmissionFollowsSemanticSettlement == [][
  s'.key > s.key => s.stage = "idle"
]_vars
TimelyDeposit == [][
  s.stage = "result-ready" /\ s'.stage = "await-ui" => BeforeCutoff
]_vars
ReadFailurePreservesKnowledge == [][
  ReceiveReadUnavailable \/ LoseReadReply => UNCHANGED s.knowledge
]_vars
EarlyNotClosingIsInconclusive == [][
  ReceiveNotClosingAfterAmbiguity => s'.uncertain /\ UNCHANGED s.knowledge
]_vars
ReconnectFollowsExactJoin == [][
  s.carrier = "joined" /\ s'.carrier = "status-only" =>
    s.carrier = "joined" /\ s.workerReturned /\ s.returnedWorker = s.workerIdentity
]_vars
WorkerReturnHasCurrentSource == [][
  s'.returnedWorker # s.returnedWorker => WorkerReturns
    /\ s'.returnedWorker = s.workerIdentity
]_vars
FreshCarrierCannotBorrowOldReturn == [][
  ExplicitStatusReconnect => s'.workerIdentity = s.workerIdentity+1
    /\ ~s'.workerReturned /\ s'.returnedWorker = s.returnedWorker
]_vars
RetirementPreservesCurrentReturn == [][
  s'.stage = "retired" /\ s.workerReturned => s'.workerReturned
    /\ s'.returnedWorker = s.returnedWorker
]_vars
SubmittedRetirementRetainsRecovery == [][
  s'.stage = "retired" /\ UnansweredMutation(s) => s'.uncertain /\ s'.key = s.key
]_vars
NoInventedSubmissionUncertainty == [][
  ~s.uncertain /\ s'.uncertain => UnansweredMutation(s)
]_vars
RecoveryResumeRequiresFreshReadAndDraw == [][
  s.carrier # "confirmed-control" /\ s'.carrier = "confirmed-control" =>
    s.carrier = "status-only" /\ s.freshReadSettled /\ s.statusDrawn /\ CurrentPrompt
    /\ s.promptKind = "resume" /\ s.stage = "idle"
]_vars
OnlyCommandResponseEstablishesReceipt == [][
  ~s.receiptKnown /\ s'.receiptKnown =>
    s.kind = "close" /\ s.stage = "handed-off" /\ s'.resultKind = "accepted"
]_vars
KnownReceiptNeverRegresses == [][
  s.receiptKnown => s'.receiptKnown /\ s'.acceptedBase = s.acceptedBase
]_vars
ResumePromptRequiresDisplayedStatus == [][
  OpenConfirmation("resume") => s.statusDrawn
]_vars
AbsentPromptDrawRevokesShown == [][
  RevokePromptVisibility => s'.prompt = "unshown"
]_vars
GeometryChangeRevokesDrawBeforeInput == [][
  ObserveGeometryChange => s'.prompt # "shown" /\ ~s'.statusDrawn
]_vars
AbsentStatusDrawUnarmsResume == [][
  RevokeStatusVisibility /\ s.promptKind = "resume" => s'.prompt # "shown"
]_vars
EarlierDeadlineNeverRenews == [][s'.predecessorAge >= s.predecessorAge]_vars
ReadAdmissionConsumesInterval == [][
  s'.readCount > s.readCount => s.intervalDue /\ ~s'.intervalDue
]_vars
RequestCursorNeverRewinds == [][s'.key >= s.key]_vars
PendingEventuallySettlesOrRetires ==
  (s.stage \in PendingStages) ~> (s.stage \notin PendingStages)

=============================================================================
