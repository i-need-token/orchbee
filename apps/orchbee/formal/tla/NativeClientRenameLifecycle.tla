--------------------- MODULE NativeClientRenameLifecycle ---------------------
EXTENDS Naturals, FiniteSets

CONSTANT InitiallyReady
ASSUME InitiallyReady \in BOOLEAN
Targets == {"A", "B"}
RequestIds == 1..4
NoCommit == [target |-> "none", revision |-> 0]
Transport == {"queued", "command", "commit", "snapshot", "deposit"}
Phases == Transport \cup {"idle", "ui", "recovery", "membership", "retired", "joined"}

VARIABLE s
vars == <<s>>
Init ==
  \E target \in Targets:
    s = [phase |-> "idle", selected |-> target, cacheRevision |-> 0,
         ready |-> InitiallyReady, other |-> "none",
         draft |-> FALSE, token |-> 0, shown |-> 0, draftTarget |-> target,
         draftBase |-> 0, draftLabel |-> "same", fault |-> FALSE,
         complete |-> TRUE, authorityChanged |-> FALSE,
         cursor |-> 0, admissions |-> 0, operation |-> 0, exchange |-> 0, budget |-> 0,
         target |-> target, baseline |-> 0, label |-> "same", mode |-> "rename",
         clock |-> "before", sent |-> {}, known |-> [id \in RequestIds |-> NoCommit],
         typedKey |-> <<0,0>>, result |-> "none", candidateRevision |-> 0,
         recoveryUsed |-> FALSE, ackPending |-> 0, lastConsumed |-> 0,
         uncertain |-> 0, worker |-> 2, returnedWorker |-> 0, oldReturned |-> FALSE,
         ownerClock |-> "before", ownerResult |-> "none"]

Live == s.phase \notin {"retired", "joined"}
Before == s.clock = "before"
CurrentKnown == IF s.operation = 0 THEN FALSE ELSE s.known[s.operation] # NoCommit
DraftCurrent == s.draftTarget = s.selected /\ s.draftBase = s.cacheRevision
EffectsEnabled == Live /\ s.phase \in {"idle", "recovery"} /\ s.ready
Retired(state) ==
  [state EXCEPT !.phase = "retired", !.draft = FALSE, !.shown = 0,
    !.other = "none", !.ready = FALSE,
    !.uncertain = IF state.operation \in state.sent
                       /\ state.result = "none"
                       /\ state.known[state.operation] = NoCommit
                  THEN state.operation ELSE 0]

OpenDraft ==
  /\ s.phase = "idle" /\ s.other = "none" /\ ~s.draft /\ s.selected \in Targets /\ s.token < 3
  /\ s' = [s EXCEPT !.draft = TRUE, !.token = @+1, !.shown = 0,
       !.draftTarget = s.selected, !.draftBase = s.cacheRevision,
       !.draftLabel = "same", !.fault = FALSE, !.complete = TRUE,
       !.authorityChanged = FALSE]
DrawExact ==
  /\ s.draft /\ DraftCurrent /\ s.shown # s.token
  /\ s' = [s EXCEPT !.shown = s.token]
RevokeDraw ==
  /\ s.draft /\ s.shown = s.token
  /\ s' = [s EXCEPT !.shown = 0]
RejectStaleDraw == /\ s.draft /\ UNCHANGED vars
EditLabel(relation) ==
  /\ s.draft /\ relation \in {"same", "changed"}
  /\ s' = [s EXCEPT !.draftLabel = relation]
InputFault == /\ s.draft /\ ~s.fault /\ s' = [s EXCEPT !.fault = TRUE]
CorrectWithBackspaceOrClear(relation) ==
  /\ s.draft /\ s.complete /\ relation \in {"same", "changed"}
  /\ s' = [s EXCEPT !.fault = FALSE, !.draftLabel = relation]
InputCompleteness(complete) ==
  /\ s.draft /\ complete \in BOOLEAN
  /\ s' = [s EXCEPT !.complete = complete]
AuthorityChanges(kind) ==
  /\ s.phase = "idle" /\ s.draft /\ ~s.authorityChanged
  /\ kind \in {"base", "selection"}
  /\ s' = [s EXCEPT !.cacheRevision = IF kind = "base" THEN @+1 ELSE @,
       !.selected = IF kind = "selection" THEN IF @ = "A" THEN "B" ELSE "A" ELSE @,
       !.ready = IF kind = "selection" THEN FALSE ELSE @, !.authorityChanged = TRUE]
DismissStaleDraft ==
  /\ s.draft /\ ~DraftCurrent
  /\ s' = [s EXCEPT !.draft = FALSE, !.shown = 0]
CancelDraft ==
  /\ s.draft /\ s' = [s EXCEPT !.draft = FALSE, !.shown = 0]
RefuseDraftAdmission ==
  /\ s.draft
  /\ (~DraftCurrent \/ s.shown # s.token \/ s.fault \/ ~s.complete
      \/ s.other # "none" \/ s.cursor = 4 \/ s.admissions = 2)
  /\ UNCHANGED vars
AdmitRename ==
  /\ s.phase = "idle" /\ s.draft /\ DraftCurrent /\ s.shown = s.token
  /\ ~s.fault /\ s.complete /\ s.other = "none" /\ s.cursor < 4 /\ s.admissions < 2
  /\ s' = [s EXCEPT !.phase = "queued", !.draft = FALSE, !.shown = 0,
       !.cursor = @+1, !.operation = s.cursor+1, !.exchange = s.cursor+1, !.budget = s.cursor+1,
       !.admissions = @+1, !.target = s.draftTarget, !.baseline = s.draftBase,
       !.label = s.draftLabel, !.clock = "before", !.mode = "rename",
       !.typedKey = <<0,0>>, !.result = "none", !.recoveryUsed = FALSE]
AdmitCompetingFlight(kind) ==
  /\ kind \in {"status", "structure", "confirmation"}
  /\ s.phase = "idle" /\ s.other = "none" /\ (s.draft => kind = "status")
  /\ s' = [s EXCEPT !.other = kind]
FinishCompetingFlight ==
  /\ s.other # "none" /\ s' = [s EXCEPT !.other = "none"]
RejectCompetingFlight == /\ Live /\ s.phase # "idle" /\ UNCHANGED vars

HandOffOnce ==
  /\ s.phase = "queued" /\ Before /\ s.ackPending = 0
  /\ s' = [s EXCEPT !.phase = "command", !.sent = @ \cup {s.operation}]
DefiniteNotSubmitted ==
  /\ s.phase = "queued" /\ Before /\ s.ackPending = 0
  /\ s' = [s EXCEPT !.phase = "deposit", !.result = "precommit"]
CompatiblePrecommit ==
  /\ s.phase = "command" /\ Before
  /\ s' = [s EXCEPT !.phase = "deposit", !.result = "precommit"]
ReceiveNoop ==
  /\ s.phase = "command" /\ Before /\ s.label = "same"
  /\ s' = [s EXCEPT !.phase = "deposit", !.result = "noop"]
ReceiveRenamed ==
  /\ s.phase = "command" /\ Before /\ s.label = "changed"
  /\ s' = [s EXCEPT !.phase = "commit",
       !.known[s.operation] = [target |-> s.target, revision |-> s.baseline+1]]
RejectImpossibleResult ==
  /\ s.phase = "command" /\ Before
  /\ s' = Retired(s)
AllocateSnapshot ==
  /\ s.phase = "commit" /\ Before /\ s.cursor < 4
  /\ s' = [s EXCEPT !.phase = "snapshot", !.cursor = @+1, !.exchange = s.cursor+1]
SnapshotIdentityExhausted ==
  /\ s.phase = "commit" /\ s.cursor = 4
  /\ s' = Retired(s)
ReceiveSnapshot(kind) ==
  /\ s.phase = "snapshot" /\ Before
  /\ kind \in {"exact", "later", "removed", "unavailable"}
  /\ s' = [s EXCEPT !.phase = "deposit", !.result = kind,
       !.candidateRevision = s.baseline + IF kind = "exact" THEN 1 ELSE 2]
AmbiguousSnapshot == /\ s.phase = "snapshot" /\ s' = Retired(s)
DepositTypedResult ==
  /\ s.phase = "deposit" /\ Before
  /\ s' = [s EXCEPT !.phase = "ui", !.typedKey = <<s.operation,s.exchange>>,
       !.clock = "ended"]
AdvanceTransportClock ==
  /\ s.phase \in Transport /\ Before
  /\ s' = [s EXCEPT !.clock = "expired"]
ExpireBeforePlacement ==
  /\ s.phase \in Transport /\ s.clock = "expired"
  /\ s' = Retired(s)
ConsumeExactResult ==
  /\ s.phase = "ui" /\ s.typedKey = <<s.operation,s.exchange>>
  /\ s' = [s EXCEPT
       !.phase = CASE s.result = "unavailable" -> "recovery"
                   [] s.result = "removed" -> "membership"
                   [] OTHER -> "idle",
       !.cacheRevision = IF s.result \in {"exact","later"}
                          THEN s.candidateRevision ELSE @,
       !.ackPending = s.operation, !.lastConsumed = s.operation,
       !.ownerClock = "before", !.ownerResult = "none"]
RejectStaleResult ==
  /\ s.lastConsumed # 0 /\ s.operation # s.lastConsumed /\ Live
  /\ UNCHANGED vars
WorkerAcknowledge ==
  /\ s.ackPending # 0
  /\ s' = [s EXCEPT !.ackPending = 0]
LateAcknowledgeAfterBAdmission ==
  /\ s.phase = "queued" /\ s.ackPending # 0 /\ s.ackPending # s.operation
  /\ WorkerAcknowledge
AdmitExplicitRefresh ==
  /\ s.phase = "recovery" /\ ~s.recoveryUsed /\ s.cursor < 4 /\ s.ackPending = 0
  /\ s' = [s EXCEPT !.phase = "snapshot", !.mode = "refresh", !.recoveryUsed = TRUE,
       !.cursor = @+1, !.exchange = s.cursor+1, !.budget = s.cursor+1, !.clock = "before",
       !.typedKey = <<0,0>>, !.result = "none"]

OfferOwnerCompletion(result) ==
  /\ s.phase = "membership" /\ s.ownerResult = "none" /\ s.ownerClock = "before"
  /\ result \in {"survivor", "empty", "unavailable"}
  /\ s' = [s EXCEPT !.ownerResult = result, !.ownerClock = "ended"]
ConsumeOwnerCompletion ==
  /\ s.phase = "membership" /\ s.ownerResult # "none"
  /\ IF s.ownerResult = "unavailable" THEN s' = Retired(s)
     ELSE s' = [s EXCEPT !.phase = "idle", !.cacheRevision = s.candidateRevision,
       !.selected = IF s.ownerResult = "empty" THEN "none"
                     ELSE IF s.target = "A" THEN "B" ELSE "A", !.ready = FALSE]
AdvanceOwnerClock ==
  /\ s.phase = "membership" /\ s.ownerClock = "before" /\ s.ownerResult = "none"
  /\ s' = [s EXCEPT !.ownerClock = "expired"]
ExpireOwnerWait ==
  /\ s.phase = "membership" /\ s.ownerClock = "expired" /\ s.ownerResult = "none"
  /\ s' = Retired(s)
Retire == /\ Live /\ s' = Retired(s)
OldWorkerReturns ==
  /\ ~s.oldReturned /\ s' = [s EXCEPT !.oldReturned = TRUE]
CurrentWorkerReturns ==
  /\ s.phase = "retired" /\ s.returnedWorker = 0
  /\ s' = [s EXCEPT !.returnedWorker = s.worker]
JoinCurrentWorker ==
  /\ s.phase = "retired" /\ s.returnedWorker = s.worker
  /\ s' = [s EXCEPT !.phase = "joined"]

Next == OpenDraft \/ DrawExact \/ RevokeDraw \/ RejectStaleDraw
     \/ (\E relation \in {"same","changed"}: EditLabel(relation)
          \/ CorrectWithBackspaceOrClear(relation))
     \/ InputFault \/ (\E complete \in BOOLEAN: InputCompleteness(complete))
     \/ (\E kind \in {"base","selection"}: AuthorityChanges(kind))
     \/ DismissStaleDraft \/ CancelDraft \/ RefuseDraftAdmission
     \/ AdmitRename \/ (\E kind \in {"status","structure","confirmation"}: AdmitCompetingFlight(kind))
     \/ FinishCompetingFlight \/ RejectCompetingFlight
     \/ HandOffOnce \/ DefiniteNotSubmitted \/ CompatiblePrecommit \/ ReceiveNoop
     \/ ReceiveRenamed \/ RejectImpossibleResult \/ AllocateSnapshot \/ SnapshotIdentityExhausted
     \/ (\E kind \in {"exact","later","removed","unavailable"}: ReceiveSnapshot(kind))
     \/ AmbiguousSnapshot \/ DepositTypedResult \/ AdvanceTransportClock \/ ExpireBeforePlacement
     \/ ConsumeExactResult \/ RejectStaleResult \/ WorkerAcknowledge
     \/ LateAcknowledgeAfterBAdmission \/ AdmitExplicitRefresh
     \/ (\E result \in {"survivor","empty","unavailable"}: OfferOwnerCompletion(result))
     \/ ConsumeOwnerCompletion \/ AdvanceOwnerClock \/ ExpireOwnerWait \/ Retire
     \/ OldWorkerReturns \/ CurrentWorkerReturns \/ JoinCurrentWorker

Spec == Init /\ [][Next]_vars
     /\ WF_vars(AdvanceTransportClock) /\ WF_vars(ExpireBeforePlacement)
     /\ WF_vars(ConsumeExactResult) /\ WF_vars(WorkerAcknowledge)
     /\ WF_vars(AdvanceOwnerClock) /\ WF_vars(ExpireOwnerWait)
     /\ WF_vars(ConsumeOwnerCompletion)

TypeOK == /\ s.phase \in Phases /\ s.selected \in Targets \cup {"none"}
          /\ s.cursor \in 0..4 /\ s.admissions \in 0..2 /\ s.token \in 0..3
          /\ s.operation \in 0..4 /\ s.exchange \in 0..4 /\ s.budget \in 0..4
          /\ s.ackPending \in 0..4 /\ s.lastConsumed \in 0..4 /\ s.shown \in 0..3
          /\ s.cacheRevision \in 0..8 /\ s.draftBase \in 0..8 /\ s.baseline \in 0..8
          /\ s.candidateRevision \in 0..8 /\ s.target \in Targets /\ s.draftTarget \in Targets
          /\ s.label \in {"same","changed"} /\ s.draftLabel \in {"same","changed"}
          /\ s.mode \in {"rename","refresh"} /\ s.sent \subseteq RequestIds
          /\ s.ready \in BOOLEAN /\ s.other \in {"none","status","structure","confirmation"} /\ s.draft \in BOOLEAN
          /\ s.fault \in BOOLEAN /\ s.complete \in BOOLEAN /\ s.authorityChanged \in BOOLEAN
          /\ s.recoveryUsed \in BOOLEAN /\ s.oldReturned \in BOOLEAN /\ s.worker = 2
          /\ s.typedKey \in {<<op,exchange>>: op \in 0..4, exchange \in 0..4}
          /\ s.result \in {"none","precommit","noop","exact","later","removed","unavailable"}
          /\ s.ownerResult \in {"none","survivor","empty","unavailable"}
          /\ s.ownerClock \in {"before","expired","ended"}
          /\ s.clock \in {"before","expired","ended"}
          /\ s.returnedWorker \in {0,s.worker} /\ s.uncertain \in 0..4
          /\ s.known \in [RequestIds -> [target : Targets \cup {"none"}, revision : 0..8]]
StructuralFlightsExclusive == s.phase # "idle" => s.other = "none"
DraftExcludesCloseConfirmation == s.draft => s.other # "confirmation"
PendingUiIsUntimed == s.phase = "ui" => s.clock = "ended" /\ ~ENABLED ExpireBeforePlacement
AvailableOwnerResultIsUntimed == s.phase = "membership" /\ s.ownerResult # "none" =>
  s.ownerClock = "ended" /\ ~ENABLED ExpireOwnerWait
TypedResultNamesCurrent == s.phase = "ui" => s.typedKey = <<s.operation,s.exchange>>
KnownCommitRequiresHandoff == \A id \in RequestIds: s.known[id] # NoCommit => id \in s.sent
CurrentCommitFacts == CurrentKnown => /\ s.label = "changed"
  /\ s.known[s.operation] = [target |-> s.target, revision |-> s.baseline+1]
NoopUsesSameLabel == s.result = "noop" => s.label = "same" /\ s.exchange = s.operation
RecoveryHasKnownCommit == s.phase \in {"recovery","membership"} => CurrentKnown
MembershipDoesNotEnableEffects == s.phase = "membership" => ~EffectsEnabled
JoinedHasCurrentReturn == s.phase = "joined" => s.returnedWorker = s.worker
NoInventedUncertainty == s.uncertain # 0 =>
  /\ s.uncertain \in s.sent /\ s.known[s.uncertain] = NoCommit /\ s.result = "none"
AdmissionRequiresExactDraft == [][
  s.phase = "idle" /\ s'.phase = "queued" =>
    /\ s.draft /\ DraftCurrent /\ s.shown = s.token /\ ~s.fault /\ s.complete
    /\ s'.target = s.draftTarget /\ s'.baseline = s.draftBase
    /\ s'.operation = s.cursor+1 /\ s'.exchange = s'.operation
]_vars
KnownCommitNeverRegresses == [][\A id \in RequestIds:
  s.known[id] # NoCommit => s'.known[id] = s.known[id]]_vars
NoRepeatHandoff == [][HandOffOnce => s.operation \notin s.sent]_vars
LocalDraftEditsDoNotAllocate == [][
  s.draft /\ s'.phase # "queued" => s'.cursor = s.cursor]_vars
EditDoesNotClearFault == [][(\E relation \in {"same","changed"}: EditLabel(relation)) => s'.fault = s.fault]_vars
OnlyExplicitCorrectionClearsFault == [][
  s.draft /\ s'.draft /\ s.fault /\ ~s'.fault =>
    \E relation \in {"same","changed"}: CorrectWithBackspaceOrClear(relation)]_vars
CapturedOperationNeverRetargets == [][s.operation = s'.operation =>
  UNCHANGED <<s.target,s.baseline,s.label>>]_vars
ExistingCutoffNeverRenews == [][
  s.budget = s'.budget /\ s.clock # "before" => s'.clock # "before"]_vars
AcknowledgmentPreservesCurrentFlight == [][WorkerAcknowledge =>
  UNCHANGED <<s.operation,s.exchange,s.phase,s.known>>]_vars
PendingAcknowledgmentIsNotOverwritten == [][
  s.ackPending # 0 => s'.ackPending \in {s.ackPending,0}]_vars
MembershipInstallNeedsExactOwner == [][
  s.phase = "membership" /\ s'.phase = "idle" =>
    /\ s.ownerResult \in {"survivor","empty"} /\ ~s'.ready]_vars
RetirementIsNotReplay == [][s.phase \in {"retired","joined"} => s'.sent = s.sent]_vars
PendingEventuallySettlesOrRetires ==
  (s.phase \in Transport \cup {"ui","membership"}) ~> (s.phase \in {"idle","recovery","retired","joined"})
=============================================================================
