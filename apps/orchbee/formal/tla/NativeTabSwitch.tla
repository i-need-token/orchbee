--------------------------- MODULE NativeTabSwitch ---------------------------
EXTENDS Integers, Naturals, Sequences, FiniteSets

CONSTANTS Tabs, AttachableTabs, InitialTab, MaxGeneration, MaxSwitchAdmissions,
          InitialMode, MaxQueue, MaxGeometry

ASSUME
  /\ Cardinality(Tabs) = 3
  /\ AttachableTabs \subseteq Tabs
  /\ Cardinality(AttachableTabs) = 2
  /\ InitialTab \in AttachableTabs
  /\ MaxGeneration \in {2, 4}
  /\ MaxSwitchAdmissions = 2
  /\ InitialMode \in {"controlling", "readonly"}
  /\ MaxQueue = 3
  /\ MaxGeometry = 1

NoTab == "NO_TAB"
NoGeneration == -1
NoGeometry == -1

ControlKinds == {"input", "resize"}
CommandKinds == ControlKinds \cup {"switch"}
Modes == {"none", "controlling", "readonly"}
ProjectionStates == {"none", "staged", "drawn", "ready"}
DetachReasons == {"none", "handoff", "superseded", "current-failure", "deadline",
                  "candidate-deadline"}

StablePhases == {"ready", "retryable", "selected-no-attach", "empty", "retired", "closed"}
CleanupPhases == {"cleanup-ready", "cleanup-wait"}
RequestWaitPhases == {
  "detach-wait", "attach-wait", "acquire-wait", "resize-wait",
  "poll-wait", "feedback-wait", "snapshot-wait"
}
SafeSetupPhases == {
  "switching-idle", "detach-ready", "attach-ready", "acquire-ready",
  "resize-ready", "await-frame", "frame-event", "frame-staged",
  "feedback-ready", "ready-publish", "snapshot-ready", "candidate-ready", "install-ready"
}
UnsettledPhases == CleanupPhases \cup RequestWaitPhases \cup SafeSetupPhases
Phases == StablePhases \cup UnsettledPhases

Command(kind, generation, tab) ==
  [kind |-> kind, generation |-> generation, tab |-> tab]

CommandRecords ==
  [kind : CommandKinds, generation : 0..MaxGeneration, tab : Tabs]

Attachment(generation, tab) ==
  [generation |-> generation, tab |-> tab]

AttachmentRecords ==
  [generation : 0..MaxGeneration, tab : AttachableTabs]

Frame(generation, tab, geometry) ==
  [generation |-> generation, tab |-> tab, geometry |-> geometry]

FrameRecords ==
  [generation : 0..MaxGeneration, tab : Tabs, geometry : 0..MaxGeometry]

Feedback(generation, geometry) ==
  [generation |-> generation, geometry |-> geometry]

FeedbackRecords ==
  [generation : 0..MaxGeneration, geometry : 0..MaxGeometry]

ContainsKind(queue, kind) ==
  \E index \in 1..Len(queue): queue[index].kind = kind

VARIABLE s
vars == <<s>>

Anchor(state) == [phase |-> state.phase, generation |-> state.desiredGeneration,
  tab |-> state.desiredTab, mode |-> state.mode, observed |-> state.observedGeometry,
  applied |-> state.appliedGeometry, frame |-> state.frameGeometry,
  proof |-> state.restorationProof]
NoAnchor == [phase |-> "none", generation |-> 0, tab |-> NoTab, mode |-> "none",
             observed |-> 0, applied |-> 0, frame |-> NoGeometry, proof |-> FALSE]
StableAnchorPhases == {"ready", "retryable", "selected-no-attach"}
AnchorKey == Attachment(s.anchor.generation, s.anchor.tab)
ReadyAnchorValid ==
  /\ s.anchor.phase = "ready" /\ s.anchor.proof
  /\ s.viewers = {AnchorKey}
  /\ s.controllers = IF s.anchor.mode = "controlling" THEN {AnchorKey} ELSE {}
  /\ s.observedGeometry = s.anchor.observed
  /\ Feedback(s.anchor.generation, s.anchor.frame) \in s.seenFrames
  /\ Feedback(s.anchor.generation, s.anchor.frame) \in s.acceptedFeedback
  /\ (s.anchor.mode = "controlling" => s.anchor.applied = s.anchor.observed
       /\ s.anchor.frame = s.anchor.applied)
NoViewerAnchorValid ==
  /\ s.anchor.phase \in {"retryable", "selected-no-attach"}
  /\ s.viewers = {} /\ s.controllers = {}
AnchorValid == ReadyAnchorValid \/ NoViewerAnchorValid
CurrentCandidate == s.workGeneration = s.desiredGeneration
                    /\ s.workTab = s.desiredTab
BeforeInstallation == s.candidateStage \in {"waiting", "validated"}
UnavailableDeposited == s.phase = "candidate-ready" /\ s.snapshotKind = "unavailable"

CoreSnapshot(state) == [
  phase |-> state.phase,
  desiredGeneration |-> state.desiredGeneration,
  desiredTab |-> state.desiredTab,
  generationCursor |-> state.generationCursor,
  markerGenerations |-> state.markerGenerations,
  anchor |-> state.anchor,
  candidateStage |-> state.candidateStage,
  candidateTab |-> state.candidateTab,
  workBudgetOrigin |-> state.workBudgetOrigin,
  commands |-> state.commands,
  events |-> state.events,
  viewers |-> state.viewers,
  controllers |-> state.controllers,
  projectionState |-> state.projectionState,
  projectionGeneration |-> state.projectionGeneration,
  projectionTab |-> state.projectionTab,
  observedGeometry |-> state.observedGeometry,
  appliedGeometry |-> state.appliedGeometry
]

Retired(state) ==
  [state EXCEPT
    !.phase = "retired",
    !.carrierLive = FALSE,
    !.commands = <<>>,
    !.events = <<>>,
    !.viewers = {},
    !.controllers = {},
    !.detachRequests = {},
    !.attachRequests = {},
    !.mode = "none",
    !.projectionState = "none",
    !.projectionGeneration = NoGeneration,
    !.projectionTab = NoTab,
    !.frameGeometry = NoGeometry,
    !.feedbackRequestedGeometry = NoGeometry,
    !.mustRecheck = FALSE]

Initial(controllerSet) == [
  phase |-> "ready",
  carrierLive |-> TRUE,
  desiredGeneration |-> 0,
  desiredTab |-> InitialTab,
  generationCursor |-> 0,
  markerGenerations |-> {},
  anchor |-> NoAnchor,
  restorationProof |-> TRUE,
  restoredBy |-> NoGeneration,
  restoredGeneration |-> NoGeneration,
  workBudgetOrigin |-> 0,
  candidateStage |-> "none",
  candidateTab |-> NoTab,
  snapshotKind |-> "none",
  preMarkerDrained |-> FALSE,
  workGeneration |-> 0,
  workTab |-> InitialTab,
  workAttachAttempted |-> FALSE,
  commands |-> <<>>,
  events |-> <<>>,
  viewers |-> {Attachment(0, InitialTab)},
  controllers |-> controllerSet,
  detachRequests |-> {},
  attachRequests |-> {},
  detachReason |-> "none",
  attachLicenses |-> {},
  mode |-> IF controllerSet = {} THEN "readonly" ELSE "controlling",
  mustRecheck |-> FALSE,
  projectionState |-> "ready",
  projectionGeneration |-> 0,
  projectionTab |-> InitialTab,
  observedGeometry |-> 0,
  resizeRequestedGeometry |-> NoGeometry,
  appliedGeometry |-> 0,
  frameGeometry |-> 0,
  feedbackRequestedGeometry |-> NoGeometry,
  seenFrames |-> {Feedback(0, 0)},
  acceptedFeedback |-> {Feedback(0, 0)},
  deliveredControls |-> {},
  refusalCoreChanged |-> FALSE,
  sameTargetCoreChanged |-> FALSE,
  rejectedSwitchSeen |-> FALSE,
  sameTargetNoOpSeen |-> FALSE,
  deadlineCleanupSupersededSeen |-> FALSE
]

Init ==
  s = Initial(IF InitialMode = "controlling" THEN {Attachment(0, InitialTab)} ELSE {})

SelectablePhase ==
  s.phase \in (UnsettledPhases \cup {"ready", "retryable", "selected-no-attach"})

QueueControl(kind) ==
  /\ kind \in ControlKinds
  /\ s.carrierLive
  /\ s.phase = "ready"
  /\ s.controllers # {}
  /\ Len(s.commands) < MaxQueue
  /\ s' = [s EXCEPT
       !.commands = Append(@,
         Command(kind, s.desiredGeneration, s.desiredTab))]

SwitchAdmissionGuard(target) ==
  /\ target \in Tabs
  /\ s.carrierLive
  /\ SelectablePhase
  /\ (target # s.desiredTab \/ s.phase = "retryable")
  /\ s.generationCursor < MaxGeneration
  \* Preserve the original two-operation exploration bound without inventing refusal.
  /\ Cardinality(s.markerGenerations) < MaxSwitchAdmissions
  /\ Len(s.commands) < MaxQueue

AdmitSwitch(target, deadlineCleanupWitness) ==
  /\ SwitchAdmissionGuard(target)
  /\ LET nextGeneration == s.generationCursor + 1 IN
       s' = [s EXCEPT
         !.desiredGeneration = nextGeneration,
         !.desiredTab = target,
          !.generationCursor = nextGeneration,
          !.markerGenerations = @ \cup {nextGeneration},
          !.anchor = IF s.phase \in StableAnchorPhases THEN Anchor(s) ELSE @,
          !.restoredBy = NoGeneration,
          !.restoredGeneration = NoGeneration,
         !.commands = Append(@, Command("switch", nextGeneration, target)),
         !.deadlineCleanupSupersededSeen =
           @ \/ deadlineCleanupWitness,
         !.phase = IF s.phase \in StablePhases THEN "switching-idle" ELSE @,
         !.projectionState = "none",
         !.projectionGeneration = NoGeneration,
         !.projectionTab = NoTab,
         !.frameGeometry = NoGeometry]

RequestSwitchDuringDeadlineCleanup(target) ==
  /\ s.phase \in CleanupPhases
  /\ s.detachReason \in {"deadline", "candidate-deadline"}
  /\ AdmitSwitch(target, TRUE)

RequestSwitch(target) ==
  /\ ~(s.phase \in CleanupPhases /\ s.detachReason \in {"deadline", "candidate-deadline"})
  /\ AdmitSwitch(target, FALSE)

RejectSwitch(target) ==
  /\ target \in Tabs
  /\ s.carrierLive
  /\ SelectablePhase
  /\ (target # s.desiredTab \/ s.phase = "retryable")
  /\ (s.generationCursor = MaxGeneration \/ Len(s.commands) = MaxQueue)
  /\ ~s.rejectedSwitchSeen
  /\ LET next == [s EXCEPT !.rejectedSwitchSeen = TRUE] IN
       s' = [next EXCEPT
         !.refusalCoreChanged = @ \/ (CoreSnapshot(next) # CoreSnapshot(s))]

SameTargetNoOp ==
  /\ s.carrierLive
  /\ SelectablePhase
  /\ s.phase # "retryable"
  /\ ~s.sameTargetNoOpSeen
  /\ LET next == [s EXCEPT !.sameTargetNoOpSeen = TRUE] IN
       s' = [next EXCEPT
         !.sameTargetCoreChanged = @ \/ (CoreSnapshot(next) # CoreSnapshot(s))]

DeliverAcceptedControl ==
  /\ s.commands # <<>>
  /\ Head(s.commands).kind \in ControlKinds
  /\ Attachment(Head(s.commands).generation, Head(s.commands).tab)
       \in s.controllers
  /\ s' = [s EXCEPT
        !.deliveredControls = @ \cup {Head(s.commands)},
        !.restorationProof = IF Head(s.commands).kind = "resize" THEN FALSE ELSE @,
        !.anchor = IF Head(s.commands).kind = "resize"
                      /\ Head(s.commands).generation = s.anchor.generation
                      /\ Head(s.commands).tab = s.anchor.tab
                   THEN [@ EXCEPT !.proof = FALSE] ELSE @,
        !.commands = Tail(@)]

ProcessSwitchMarker ==
  /\ s.phase = "switching-idle"
  /\ s.commands # <<>>
  /\ Head(s.commands).kind = "switch"
  /\ LET marker == Head(s.commands) IN
       LET current == marker.generation = s.desiredGeneration IN
          s' = [s EXCEPT
           !.commands = Tail(@),
           !.workGeneration = marker.generation,
           !.workTab = marker.tab,
            !.workAttachAttempted = FALSE,
            !.workBudgetOrigin = marker.generation,
            !.candidateStage = IF current THEN "waiting" ELSE "none",
            !.candidateTab = NoTab,
            !.snapshotKind = "none",
            !.preMarkerDrained = FALSE,
            !.detachReason = "none",
            !.phase = IF current THEN "snapshot-ready" ELSE "switching-idle"]

\* The existing Ready-flight adapter owns exact Poll/causal response bytes. This
\* boundary cannot skip a handed-off exchange or manufacture a new repair Poll.
DrainOldFlightPreservingAnchor ==
  /\ s.phase = "snapshot-ready" /\ ~s.preMarkerDrained
  /\ s' = [s EXCEPT !.preMarkerDrained = TRUE]
DrainOldFlightInvalidatingAnchor ==
  /\ s.phase = "snapshot-ready" /\ ~s.preMarkerDrained
  /\ s.anchor.phase = "ready"
  /\ s' = [s EXCEPT !.preMarkerDrained = TRUE,
       !.anchor = [@ EXCEPT !.proof = FALSE], !.restorationProof = FALSE]
RequestSwitchSnapshot ==
  /\ s.phase = "snapshot-ready" /\ s.preMarkerDrained /\ CurrentCandidate
  /\ s' = [s EXCEPT !.phase = "snapshot-wait"]
DepositSwitchSnapshot(kind) ==
  /\ kind \in {"available", "unavailable"} /\ s.phase = "snapshot-wait"
  \* Exact typed placement before the original forward cutoff. UI may consume late.
  /\ s' = [s EXCEPT !.phase = "candidate-ready", !.snapshotKind = kind]
\* Repeat the helper's phase guard so pinned TLC reports each typed action header.
DepositAvailableSwitchSnapshot ==
  /\ s.phase = "snapshot-wait" /\ DepositSwitchSnapshot("available")
DepositUnavailableSwitchSnapshot ==
  /\ s.phase = "snapshot-wait" /\ DepositSwitchSnapshot("unavailable")
RejectSwitchSnapshotTransport ==
  /\ s.phase \in {"snapshot-ready", "snapshot-wait"} /\ s' = Retired(s)
StageValidatedSwitchCandidate(selected) ==
  /\ selected \in Tabs \cup {NoTab}
  /\ s.phase = "candidate-ready" /\ s.snapshotKind = "available" /\ CurrentCandidate
  \* Membership owns the validated target/fallback/Empty choice, not this model.
  /\ s' = [s EXCEPT !.candidateStage = "validated", !.candidateTab = selected,
       !.detachReason = IF s.viewers = {} THEN "none" ELSE "handoff",
       !.phase = IF s.viewers = {} THEN "install-ready" ELSE "detach-ready"]
RejectInvalidSwitchCandidate ==
  /\ s.phase = "candidate-ready" /\ s.snapshotKind = "available" /\ CurrentCandidate
  /\ s' = Retired(s)
SkipSupersededCandidate ==
  /\ s.phase \in {"snapshot-ready", "candidate-ready", "detach-ready", "install-ready"}
  /\ BeforeInstallation /\ ~CurrentCandidate /\ ~s.mustRecheck
  /\ s.detachRequests = {} /\ s.attachRequests = {}
  /\ s' = [s EXCEPT !.phase = "switching-idle", !.candidateStage = "none",
       !.detachReason = "none", !.snapshotKind = "none"]
SkipSupersededBeforeSnapshot == /\ s.phase = "snapshot-ready" /\ SkipSupersededCandidate
SkipSupersededDepositedCandidate == /\ s.phase = "candidate-ready" /\ SkipSupersededCandidate

InstallSwitchCandidate ==
  /\ s.phase = "install-ready" /\ s.candidateStage = "validated" /\ CurrentCandidate
  /\ s.candidateTab \in Tabs /\ s.viewers = {} /\ s.controllers = {}
  /\ s.detachRequests = {} /\ s.attachRequests = {}
  /\ s.generationCursor < MaxGeneration
  /\ LET fresh == s.generationCursor + 1 IN
       s' = [s EXCEPT !.generationCursor = fresh, !.desiredGeneration = fresh,
         !.workGeneration = fresh, !.desiredTab = s.candidateTab, !.workTab = s.candidateTab,
         !.candidateStage = "installed", !.restorationProof = FALSE,
         !.restoredBy = NoGeneration, !.restoredGeneration = NoGeneration,
         !.attachLicenses = IF s.candidateTab \in AttachableTabs THEN @ \cup {fresh} ELSE @,
         !.mode = "none", !.phase = IF s.candidateTab \in AttachableTabs
                                    THEN "attach-ready" ELSE "selected-no-attach"]
InstallSurvivingSwitchCandidate ==
  /\ s.candidateTab = s.workTab /\ InstallSwitchCandidate
InstallFallbackSwitchCandidate ==
  /\ s.candidateTab # s.workTab /\ InstallSwitchCandidate
\* Positive coverage for a second installation after the first reached Ready.
\* Guard aliases add no state or behavior to the original transition union.
InstallSecondSwitchCandidate ==
  /\ s.markerGenerations = {1, 3} /\ s.generationCursor = 3
  /\ s.anchor.phase = "ready" /\ s.anchor.generation = 2
  /\ InstallSwitchCandidate
InstallEmptySwitchCandidate ==
  /\ s.phase = "install-ready" /\ s.candidateStage = "validated" /\ CurrentCandidate
  /\ s.candidateTab = NoTab /\ s.viewers = {} /\ s.controllers = {}
  /\ s.detachRequests = {} /\ s.attachRequests = {}
  /\ s' = [s EXCEPT !.desiredTab = NoTab, !.workTab = NoTab, !.phase = "empty",
       !.mode = "none", !.candidateStage = "installed", !.restorationProof = FALSE]
RejectSwitchSuccessorExhaustion ==
  /\ s.phase = "install-ready" /\ s.candidateStage = "validated" /\ CurrentCandidate
  /\ s.candidateTab \in Tabs /\ s.generationCursor = MaxGeneration /\ s' = Retired(s)

UnavailableCurrent ==
  /\ UnavailableDeposited /\ CurrentCandidate /\ s.preMarkerDrained
  /\ s.detachRequests = {} /\ s.attachRequests = {}
  /\ s.events = <<>>
RestoreUnavailableStableAnchor ==
  /\ UnavailableCurrent /\ AnchorValid
  /\ s' = [s EXCEPT !.phase = s.anchor.phase,
       !.desiredGeneration = s.anchor.generation, !.desiredTab = s.anchor.tab,
       !.workGeneration = s.anchor.generation, !.workTab = s.anchor.tab,
       !.restoredBy = s.workBudgetOrigin, !.restoredGeneration = s.anchor.generation,
       !.mode = s.anchor.mode, !.candidateStage = "none", !.snapshotKind = "none",
       !.projectionState = IF s.anchor.phase = "ready" THEN "ready" ELSE "none",
       !.projectionGeneration = IF s.anchor.phase = "ready" THEN s.anchor.generation ELSE NoGeneration,
       !.projectionTab = IF s.anchor.phase = "ready" THEN s.anchor.tab ELSE NoTab,
       !.frameGeometry = IF s.anchor.phase = "ready" THEN s.anchor.frame ELSE NoGeometry,
       !.appliedGeometry = IF s.anchor.phase = "ready" THEN s.anchor.applied ELSE @,
       !.restorationProof = (s.anchor.phase = "ready")]
RestoreUnavailableReadyAnchor == /\ ReadyAnchorValid /\ RestoreUnavailableStableAnchor
RestoreUnavailableNoViewerAnchor == /\ NoViewerAnchorValid /\ RestoreUnavailableStableAnchor
RetireUnavailableWithoutStableAnchor ==
  /\ UnavailableCurrent /\ ~AnchorValid /\ s' = Retired(s)
RetireUnavailableInvalidProof ==
  /\ s.anchor.phase = "ready" /\ ~s.anchor.proof /\ RetireUnavailableWithoutStableAnchor
RetireSecondSwitchWithoutStableAnchor ==
  /\ Cardinality(s.markerGenerations) = 2 /\ s.anchor.phase = "ready"
  /\ s.viewers # {AnchorKey} /\ RetireUnavailableWithoutStableAnchor
SettleSupersededUnavailable == /\ UnavailableDeposited /\ SkipSupersededCandidate
SettleTypedUnavailable == RestoreUnavailableStableAnchor
  \/ RetireUnavailableWithoutStableAnchor \/ SettleSupersededUnavailable

RequestDetach ==
  /\ s.phase \in {"detach-ready", "cleanup-ready"}
  /\ s.viewers # {}
  /\ s.detachRequests = {}
  /\ ~s.mustRecheck
  /\ s' = [s EXCEPT
       !.detachRequests = s.viewers,
       !.phase = IF s.phase = "cleanup-ready"
                THEN "cleanup-wait" ELSE "detach-wait"]

DetachBusy ==
  /\ s.phase \in {"detach-wait", "cleanup-wait"}
  /\ s.detachRequests # {}
  /\ s' = [s EXCEPT
       !.detachRequests = {},
       !.phase = IF s.detachReason = "handoff"
                THEN "detach-ready" ELSE "cleanup-ready",
       !.mustRecheck = TRUE]

DetachCompletesCurrentHandoff ==
  /\ s.phase = "detach-wait"
  /\ s.detachReason = "handoff"
  /\ s.workGeneration = s.desiredGeneration
  /\ s.detachRequests = s.viewers
  /\ s.detachRequests # {}
  /\ s' = [s EXCEPT
       !.viewers = {},
       !.controllers = {},
       !.detachRequests = {},
        !.mode = "none",
        !.detachReason = "none",
        !.phase = "install-ready"]

\* Only an actual Close-owner exact known-Closed proof for the planned key can
\* take this pre-handoff edge. No pending Detach is fabricated or consumed;
\* snapshot absence cannot supply this proof. Already-handed-off requests must
\* follow their original completion/ambiguity actions above and below.
KnownClosedBeforeDetach(provenKey) ==
  /\ s.phase = "detach-ready" /\ s.candidateStage = "validated"
  /\ s.preMarkerDrained /\ ~s.mustRecheck
  /\ provenKey \in s.viewers /\ s.detachRequests = {} /\ s.attachRequests = {}
  /\ s' = [s EXCEPT !.viewers = {}, !.controllers = {}, !.mode = "none",
       !.detachReason = "none",
       !.phase = IF CurrentCandidate THEN "install-ready" ELSE "switching-idle"]

DetachCompletesToNewerMarker ==
  /\ s.phase \in {"detach-wait", "cleanup-wait"}
  /\ (s.detachReason = "superseded"
       \/ s.workGeneration # s.desiredGeneration)
  /\ s.detachRequests = s.viewers
  /\ s.detachRequests # {}
  /\ s' = [s EXCEPT
       !.viewers = {},
       !.controllers = {},
       !.detachRequests = {},
       !.mode = "none",
       !.detachReason = "none",
       !.events = <<>>,
       !.projectionState = "none",
       !.projectionGeneration = NoGeneration,
       !.projectionTab = NoTab,
       !.frameGeometry = NoGeometry,
       !.phase = "switching-idle"]

DetachCompletesCurrentCleanup ==
  /\ s.phase = "cleanup-wait"
  /\ s.detachReason = "current-failure"
  /\ s.workGeneration = s.desiredGeneration
  /\ s.detachRequests = s.viewers
  /\ s.detachRequests # {}
  /\ s' = [s EXCEPT
       !.viewers = {},
       !.controllers = {},
       !.detachRequests = {},
       !.mode = "none",
       !.detachReason = "none",
       !.events = <<>>,
       !.projectionState = "none",
       !.projectionGeneration = NoGeneration,
       !.projectionTab = NoTab,
       !.frameGeometry = NoGeometry,
       !.phase = "retryable"]

DetachCompletesDeadline ==
  /\ s.phase = "cleanup-wait"
  /\ s.detachReason = "deadline"
  /\ s.workGeneration = s.desiredGeneration
  /\ s.detachRequests = s.viewers
  /\ s.detachRequests # {}
  /\ s' = [s EXCEPT
       !.viewers = {},
       !.controllers = {},
       !.detachRequests = {},
       !.mode = "none",
       !.detachReason = "none",
       !.events = <<>>,
       !.projectionState = "none",
       !.projectionGeneration = NoGeneration,
       !.projectionTab = NoTab,
       !.frameGeometry = NoGeometry,
       !.phase = IF s.desiredTab \in AttachableTabs
                  THEN "retryable" ELSE "selected-no-attach"]

DetachCompletesCandidateDeadline ==
  /\ s.phase = "cleanup-wait" /\ s.detachReason = "candidate-deadline"
  /\ s.workGeneration = s.desiredGeneration
  /\ s.detachRequests = s.viewers /\ s.detachRequests # {}
  /\ s' = Retired(s)

DetachDefiniteFailure ==
  /\ s.phase \in {"detach-wait", "cleanup-wait"}
  /\ s.detachRequests # {}
  /\ s' = Retired(s)

DetachAmbiguous ==
  /\ s.phase \in {"detach-wait", "cleanup-wait"}
  /\ s.detachRequests # {}
  /\ s' = Retired(s)

RecheckAfterBusy ==
  /\ s.mustRecheck
  /\ s.phase \in {"detach-ready", "cleanup-ready", "attach-ready",
                   "acquire-ready", "resize-ready", "await-frame",
                   "feedback-ready"}
  /\ s' = [s EXCEPT
       !.mustRecheck = FALSE,
        !.detachReason = IF s.workGeneration # s.desiredGeneration /\ BeforeInstallation
                         THEN "none"
                         ELSE IF s.workGeneration # s.desiredGeneration
                           /\ s.viewers # {}
                        THEN "superseded" ELSE @,
       !.phase = IF s.workGeneration # s.desiredGeneration
                  THEN IF BeforeInstallation THEN "switching-idle"
                       ELSE IF s.viewers # {}
                      THEN "cleanup-ready" ELSE "switching-idle"
                 ELSE @,
       !.events = IF s.workGeneration # s.desiredGeneration THEN <<>> ELSE @]

RequestAttach ==
  /\ s.phase = "attach-ready"
  /\ s.workGeneration = s.desiredGeneration
  /\ s.workTab = s.desiredTab
  /\ s.workTab \in AttachableTabs
  /\ s.workGeneration \in s.attachLicenses
  /\ s.viewers = {}
  /\ s.attachRequests = {}
  /\ ~s.mustRecheck
  /\ s' = [s EXCEPT
       !.attachRequests = {Attachment(s.workGeneration, s.workTab)},
       !.workAttachAttempted = TRUE,
       !.phase = "attach-wait"]

AttachCompletes ==
  /\ s.phase = "attach-wait"
  /\ s.attachRequests = {Attachment(s.workGeneration, s.workTab)}
  /\ LET attached == Attachment(s.workGeneration, s.workTab) IN
       s' = [s EXCEPT
         !.attachRequests = {},
         !.viewers = @ \cup {attached},
         !.phase = IF s.workGeneration = s.desiredGeneration
                       /\ s.workTab = s.desiredTab
                   THEN "acquire-ready" ELSE "cleanup-ready",
         !.detachReason = IF s.workGeneration = s.desiredGeneration
                          THEN "none" ELSE "superseded"]

AttachBusy ==
  /\ s.phase = "attach-wait"
  /\ s.attachRequests # {}
  /\ s' = [s EXCEPT
       !.attachRequests = {},
       !.phase = "attach-ready",
       !.mustRecheck = TRUE]

AttachDefiniteFailure ==
  /\ s.phase = "attach-wait"
  /\ s.attachRequests # {}
  /\ s' = [s EXCEPT
       !.attachRequests = {},
       !.phase = IF s.workGeneration = s.desiredGeneration
                 THEN "retryable" ELSE "switching-idle"]

AttachAmbiguous ==
  /\ s.phase = "attach-wait"
  /\ s.attachRequests # {}
  /\ s' = Retired(s)

RequestAcquire ==
  /\ s.phase = "acquire-ready"
  /\ s.viewers = {Attachment(s.workGeneration, s.workTab)}
  /\ s.workGeneration = s.desiredGeneration
  /\ ~s.mustRecheck
  /\ s' = [s EXCEPT !.phase = "acquire-wait"]

AcquireControlling ==
  /\ s.phase = "acquire-wait"
  /\ s.viewers = {Attachment(s.workGeneration, s.workTab)}
  /\ LET current == s.workGeneration = s.desiredGeneration IN
       s' = [s EXCEPT
         !.controllers = @ \cup {Attachment(s.workGeneration, s.workTab)},
         !.mode = "controlling",
         !.phase = IF current THEN "resize-ready" ELSE "cleanup-ready",
         !.detachReason = IF current THEN "none" ELSE "superseded"]

AcquireReadOnly ==
  /\ s.phase = "acquire-wait"
  /\ s.viewers = {Attachment(s.workGeneration, s.workTab)}
  /\ LET current == s.workGeneration = s.desiredGeneration IN
       s' = [s EXCEPT
         !.mode = "readonly",
         !.phase = IF current THEN "await-frame" ELSE "cleanup-ready",
         !.detachReason = IF current THEN "none" ELSE "superseded"]

AcquireBusy ==
  /\ s.phase = "acquire-wait"
  /\ s' = [s EXCEPT
       !.phase = "acquire-ready",
       !.mustRecheck = TRUE]

BeginCurrentCleanup ==
  /\ s.phase \in {"acquire-wait", "resize-wait", "poll-wait", "feedback-wait"}
  /\ s.viewers # {}
  /\ s' = [s EXCEPT
       !.phase = "cleanup-ready",
       !.detachReason = IF s.workGeneration = s.desiredGeneration
                        THEN "current-failure" ELSE "superseded"]

InFlightAmbiguous ==
  /\ s.phase \in {"acquire-wait", "resize-wait", "poll-wait", "feedback-wait"}
  /\ s' = Retired(s)

ObserveGeometry(cuts) ==
  /\ s.phase \in cuts
  /\ s.observedGeometry < MaxGeometry
  /\ s' = [s EXCEPT !.observedGeometry = @ + 1]

ObserveAtAcquireCut == ObserveGeometry({"acquire-ready", "acquire-wait"})
ObserveAtResizeCut == ObserveGeometry({"resize-ready", "resize-wait"})
ObserveAtPollCut == ObserveGeometry({"await-frame", "poll-wait"})
ObserveAtFrameCut == ObserveGeometry({"frame-event", "frame-staged"})
ObserveAtDrawCut == ObserveGeometry({"feedback-ready"})
ObserveAtFeedbackCut == ObserveGeometry({"feedback-wait", "ready-publish"})
\* Keep candidate-cut coverage distinct from the unchanged setup geometry aliases.
ObserveAtCandidateCut ==
  /\ s.phase \in {"snapshot-ready", "snapshot-wait", "candidate-ready"}
  /\ ObserveGeometry({"snapshot-ready", "snapshot-wait", "candidate-ready"})

RequestResize ==
  /\ s.phase = "resize-ready"
  /\ s.mode = "controlling"
  /\ s.controllers = {Attachment(s.workGeneration, s.workTab)}
  /\ s.workGeneration = s.desiredGeneration
  /\ ~s.mustRecheck
  /\ s' = [s EXCEPT
       !.resizeRequestedGeometry = s.observedGeometry,
       !.phase = "resize-wait"]

ResizeCompletes ==
  /\ s.phase = "resize-wait"
  /\ s.resizeRequestedGeometry \in 0..MaxGeometry
  /\ LET current == s.workGeneration = s.desiredGeneration IN
       s' = [s EXCEPT
         !.appliedGeometry = s.resizeRequestedGeometry,
         !.phase = IF current THEN "await-frame" ELSE "cleanup-ready",
         !.detachReason = IF current THEN "none" ELSE "superseded",
         !.projectionState = "none",
         !.projectionGeneration = NoGeneration,
         !.projectionTab = NoTab,
         !.frameGeometry = NoGeometry]

ResizeBusy ==
  /\ s.phase = "resize-wait"
  /\ s' = [s EXCEPT
       !.phase = "resize-ready",
       !.mustRecheck = TRUE]

RequestPoll ==
  /\ s.phase = "await-frame"
  /\ s.workGeneration = s.desiredGeneration
  /\ ~s.mustRecheck
  /\ s' = [s EXCEPT !.phase = "poll-wait"]

PollEmptyNeedSnapshotOrBlocked ==
  /\ s.phase = "poll-wait"
  /\ s' = [s EXCEPT
       !.phase = IF s.workGeneration = s.desiredGeneration
                 THEN "await-frame" ELSE "cleanup-ready",
       !.detachReason = IF s.workGeneration = s.desiredGeneration
                        THEN "none" ELSE "superseded"]

PollBusy ==
  /\ s.phase = "poll-wait"
  /\ s' = [s EXCEPT
       !.phase = "await-frame",
       !.mustRecheck = TRUE]

PollFullFrame(geometry) ==
  /\ geometry \in 0..MaxGeometry
  /\ s.phase = "poll-wait"
  /\ (s.mode # "controlling" \/ geometry = s.appliedGeometry)
  /\ Len(s.events) < MaxQueue
  /\ s' = [s EXCEPT
       !.events = Append(@, Frame(s.workGeneration, s.workTab, geometry)),
       !.seenFrames = @ \cup {Feedback(s.workGeneration, geometry)},
       !.phase = "frame-event"]

AcceptCurrentFrame ==
  /\ s.phase = "frame-event"
  /\ s.events # <<>>
  /\ Head(s.events).generation = s.desiredGeneration
  /\ Head(s.events).tab = s.desiredTab
  /\ s.workGeneration = s.desiredGeneration
  /\ s' = [s EXCEPT
       !.projectionState = "staged",
       !.projectionGeneration = Head(s.events).generation,
       !.projectionTab = Head(s.events).tab,
       !.frameGeometry = Head(s.events).geometry,
       !.events = Tail(@),
       !.phase = "frame-staged"]

DropStaleFrame ==
  /\ s.phase = "frame-event"
  /\ s.events # <<>>
  /\ (Head(s.events).generation # s.desiredGeneration
       \/ Head(s.events).tab # s.desiredTab)
  /\ s' = [s EXCEPT
       !.events = Tail(@),
       !.phase = IF s.viewers # {} THEN "cleanup-ready" ELSE "switching-idle",
       !.detachReason = IF s.viewers # {} THEN "superseded" ELSE "none"]

DrawFreshFrame ==
  /\ s.phase = "frame-staged"
  /\ s.workGeneration = s.desiredGeneration
  /\ s.projectionGeneration = s.desiredGeneration
  /\ s.projectionTab = s.desiredTab
  /\ s' = [s EXCEPT
       !.projectionState = "drawn",
       !.phase = "feedback-ready"]

RequestFeedback ==
  /\ s.phase = "feedback-ready"
  /\ s.workGeneration = s.desiredGeneration
  /\ s.projectionState = "drawn"
  /\ ~s.mustRecheck
  /\ s' = [s EXCEPT
       !.feedbackRequestedGeometry = s.frameGeometry,
       !.phase = "feedback-wait"]

DropStaleFeedback ==
  /\ s.phase = "feedback-ready"
  /\ s.workGeneration # s.desiredGeneration
  /\ s' = [s EXCEPT
       !.phase = "cleanup-ready",
       !.detachReason = "superseded"]

FeedbackAcceptedCurrent ==
  /\ s.phase = "feedback-wait"
  /\ s.workGeneration = s.desiredGeneration
  /\ s.projectionState = "drawn"
  /\ LET feedback == Feedback(s.workGeneration, s.feedbackRequestedGeometry) IN
       s' = [s EXCEPT
         !.acceptedFeedback = @ \cup {feedback},
         !.phase = IF s.mode = "controlling"
                       /\ s.observedGeometry # s.appliedGeometry
                   THEN "resize-ready" ELSE "ready-publish",
         !.projectionState = IF s.mode = "controlling"
                                  /\ s.observedGeometry # s.appliedGeometry
                              THEN "none" ELSE @,
         !.projectionGeneration = IF s.mode = "controlling"
                                       /\ s.observedGeometry # s.appliedGeometry
                                   THEN NoGeneration ELSE @,
         !.projectionTab = IF s.mode = "controlling"
                                /\ s.observedGeometry # s.appliedGeometry
                            THEN NoTab ELSE @,
         !.frameGeometry = IF s.mode = "controlling"
                                /\ s.observedGeometry # s.appliedGeometry
                            THEN NoGeometry ELSE @]

FeedbackAcceptedSuperseded ==
  /\ s.phase = "feedback-wait"
  /\ s.workGeneration # s.desiredGeneration
  /\ s' = [s EXCEPT
       !.acceptedFeedback =
         @ \cup {Feedback(s.workGeneration, s.feedbackRequestedGeometry)},
       !.phase = "cleanup-ready",
       !.detachReason = "superseded"]

FeedbackBusy ==
  /\ s.phase = "feedback-wait"
  /\ s' = [s EXCEPT
       !.phase = "feedback-ready",
       !.mustRecheck = TRUE]

DetectLateGeometryCatchup ==
  /\ s.phase = "ready-publish"
  /\ s.mode = "controlling"
  /\ s.observedGeometry # s.appliedGeometry
  /\ s' = [s EXCEPT
       !.phase = "resize-ready",
       !.projectionState = "none",
       !.projectionGeneration = NoGeneration,
       !.projectionTab = NoTab,
       !.frameGeometry = NoGeometry]

PublishReady ==
  /\ s.phase = "ready-publish"
  /\ s.workGeneration = s.desiredGeneration
  /\ s.workTab = s.desiredTab
  /\ s.viewers = {Attachment(s.workGeneration, s.workTab)}
  /\ s.projectionState = "drawn"
  /\ Feedback(s.workGeneration, s.frameGeometry) \in s.acceptedFeedback
  /\ (s.mode # "controlling"
       \/ (s.observedGeometry = s.appliedGeometry
           /\ s.frameGeometry = s.appliedGeometry))
  /\ s' = [s EXCEPT
        !.phase = "ready",
        !.restorationProof = TRUE,
        !.projectionState = "ready"]

PublishSecondSwitchReady ==
  /\ s.markerGenerations = {1, 3} /\ s.workGeneration = 4
  /\ s.candidateStage = "installed"
  /\ s.anchor.phase = "ready" /\ s.anchor.generation = 2
  /\ PublishReady

HandleSupersessionAtSafeCut ==
  /\ s.phase \in {"detach-ready", "attach-ready", "acquire-ready",
                   "resize-ready", "await-frame", "frame-staged",
                   "ready-publish"}
  /\ ~s.mustRecheck
  /\ s.workGeneration # s.desiredGeneration
  /\ ~BeforeInstallation
  /\ s' = [s EXCEPT
       !.phase = IF s.viewers # {} THEN "cleanup-ready" ELSE "switching-idle",
       !.detachReason = IF s.viewers # {} THEN "superseded" ELSE "none",
       !.events = <<>>]

ExpireOperationDeadline ==
  /\ s.phase \in UnsettledPhases \ CleanupPhases
  /\ s.phase # "detach-wait"
  /\ ~UnavailableDeposited
  /\ (~BeforeInstallation \/ s.phase \in {"snapshot-wait", "switching-idle"})
  /\ ~(s.phase = "attach-ready" /\ s.workAttachAttempted
        /\ s.workGeneration = s.desiredGeneration)
  /\ s' = IF s.phase \in RequestWaitPhases \/ s.phase = "switching-idle"
            THEN Retired(s)
            ELSE IF s.viewers # {}
                 THEN [s EXCEPT
                        !.phase = "cleanup-ready",
                        !.detachReason = "deadline",
                        !.events = <<>>,
                        !.mustRecheck = FALSE]
                 ELSE [s EXCEPT
                        !.phase = IF s.desiredTab \in AttachableTabs
                                  THEN "retryable" ELSE "selected-no-attach",
                        !.commands = <<>>,
                        !.mustRecheck = FALSE]

ExpireDetachForward ==
  /\ s.phase = "detach-wait"
  /\ s' = [s EXCEPT
       !.phase = "cleanup-wait",
        !.detachReason = IF BeforeInstallation THEN "candidate-deadline" ELSE "deadline"]

ExpireCandidateForward ==
  /\ BeforeInstallation /\ ~UnavailableDeposited
  /\ s.phase \in {"snapshot-ready", "candidate-ready", "detach-ready", "install-ready"}
  /\ s' = IF s.viewers = {} THEN Retired(s)
           ELSE [s EXCEPT !.phase = "cleanup-ready", !.detachReason = "candidate-deadline",
                 !.events = <<>>, !.mustRecheck = FALSE]
ExpireCandidateWithViewer == /\ s.viewers # {} /\ ExpireCandidateForward
ExpireCandidateWithoutViewer == /\ s.viewers = {} /\ ExpireCandidateForward

ExpireAttemptedAttachBudget ==
  /\ s.phase = "attach-ready"
  /\ s.workAttachAttempted
  /\ s.workGeneration = s.desiredGeneration
  /\ s' = Retired(s)

ExpireCleanupDeadline ==
  /\ s.phase \in CleanupPhases
  /\ s' = Retired(s)

Exit ==
  /\ s.phase # "closed"
  /\ s' = [Retired(s) EXCEPT !.phase = "closed"]

Next ==
  \/ Exit
  \/ \E kind \in ControlKinds: QueueControl(kind)
  \/ \E target \in Tabs: RequestSwitch(target)
  \/ \E target \in Tabs: RequestSwitchDuringDeadlineCleanup(target)
  \/ \E target \in Tabs: RejectSwitch(target)
  \/ SameTargetNoOp
  \/ DeliverAcceptedControl
  \/ ProcessSwitchMarker
  \/ DrainOldFlightPreservingAnchor
  \/ DrainOldFlightInvalidatingAnchor
  \/ RequestSwitchSnapshot
  \/ DepositAvailableSwitchSnapshot
  \/ DepositUnavailableSwitchSnapshot
  \/ RejectSwitchSnapshotTransport
  \/ \E selected \in Tabs \cup {NoTab}: StageValidatedSwitchCandidate(selected)
  \/ RejectInvalidSwitchCandidate
  \/ SkipSupersededCandidate
  \/ SkipSupersededBeforeSnapshot
  \/ SkipSupersededDepositedCandidate
  \/ InstallSurvivingSwitchCandidate
  \/ InstallFallbackSwitchCandidate
  \/ InstallSecondSwitchCandidate
  \/ InstallEmptySwitchCandidate
  \/ RejectSwitchSuccessorExhaustion
  \/ RestoreUnavailableReadyAnchor
  \/ RestoreUnavailableNoViewerAnchor
  \/ RetireUnavailableWithoutStableAnchor
  \/ RetireUnavailableInvalidProof
  \/ RetireSecondSwitchWithoutStableAnchor
  \/ SettleSupersededUnavailable
  \/ RequestDetach
  \/ DetachBusy
  \/ DetachCompletesCurrentHandoff
  \/ \E provenKey \in AttachmentRecords: KnownClosedBeforeDetach(provenKey)
  \/ DetachCompletesToNewerMarker
  \/ DetachCompletesCurrentCleanup
  \/ DetachCompletesDeadline
  \/ DetachCompletesCandidateDeadline
  \/ DetachDefiniteFailure
  \/ DetachAmbiguous
  \/ RecheckAfterBusy
  \/ RequestAttach
  \/ AttachCompletes
  \/ AttachBusy
  \/ AttachDefiniteFailure
  \/ AttachAmbiguous
  \/ RequestAcquire
  \/ AcquireControlling
  \/ AcquireReadOnly
  \/ AcquireBusy
  \/ BeginCurrentCleanup
  \/ InFlightAmbiguous
  \/ ObserveAtAcquireCut
  \/ ObserveAtResizeCut
  \/ ObserveAtPollCut
  \/ ObserveAtFrameCut
  \/ ObserveAtDrawCut
  \/ ObserveAtFeedbackCut
  \/ ObserveAtCandidateCut
  \/ RequestResize
  \/ ResizeCompletes
  \/ ResizeBusy
  \/ RequestPoll
  \/ PollEmptyNeedSnapshotOrBlocked
  \/ PollBusy
  \/ \E geometry \in 0..MaxGeometry: PollFullFrame(geometry)
  \/ AcceptCurrentFrame
  \/ DropStaleFrame
  \/ DrawFreshFrame
  \/ RequestFeedback
  \/ DropStaleFeedback
  \/ FeedbackAcceptedCurrent
  \/ FeedbackAcceptedSuperseded
  \/ FeedbackBusy
  \/ DetectLateGeometryCatchup
  \/ PublishReady
  \/ PublishSecondSwitchReady
  \/ HandleSupersessionAtSafeCut
  \/ ExpireOperationDeadline
  \/ ExpireDetachForward
  \/ ExpireCandidateWithViewer
  \/ ExpireCandidateWithoutViewer
  \/ ExpireAttemptedAttachBudget
  \/ ExpireCleanupDeadline

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(ExpireOperationDeadline \/ ExpireDetachForward
             \/ ExpireAttemptedAttachBudget \/ ExpireCandidateForward)
  /\ WF_vars(ExpireCleanupDeadline)
  /\ WF_vars(SettleTypedUnavailable)

TypeOK ==
  /\ s.phase \in Phases
  /\ s.carrierLive \in BOOLEAN
  /\ s.desiredGeneration \in 0..MaxGeneration
  /\ s.desiredTab \in Tabs \cup {NoTab}
  /\ s.generationCursor \in 0..MaxGeneration
  /\ s.workGeneration \in 0..MaxGeneration
  /\ s.workTab \in Tabs \cup {NoTab}
  /\ s.markerGenerations \subseteq 1..MaxGeneration
  /\ Cardinality(s.markerGenerations) <= MaxSwitchAdmissions
  /\ s.candidateStage \in {"none", "waiting", "validated", "installed"}
  /\ s.candidateTab \in Tabs \cup {NoTab}
  /\ s.snapshotKind \in {"none", "available", "unavailable"}
  /\ s.preMarkerDrained \in BOOLEAN /\ s.restorationProof \in BOOLEAN
  /\ s.workBudgetOrigin \in 0..MaxGeneration
  /\ s.restoredBy \in NoGeneration..MaxGeneration
  /\ s.restoredGeneration \in NoGeneration..MaxGeneration
  /\ s.anchor.phase \in StableAnchorPhases \cup {"none"}
  /\ s.anchor.generation \in 0..MaxGeneration
  /\ s.anchor.tab \in Tabs \cup {NoTab} /\ s.anchor.mode \in Modes
  /\ s.anchor.observed \in 0..MaxGeometry /\ s.anchor.applied \in 0..MaxGeometry
  /\ s.anchor.frame \in NoGeometry..MaxGeometry /\ s.anchor.proof \in BOOLEAN
  /\ s.workAttachAttempted \in BOOLEAN
  /\ s.commands \in Seq(CommandRecords)
  /\ Len(s.commands) <= MaxQueue
  /\ s.events \in Seq(FrameRecords)
  /\ Len(s.events) <= MaxQueue
  /\ s.viewers \subseteq AttachmentRecords
  /\ s.controllers \subseteq AttachmentRecords
  /\ s.detachRequests \subseteq AttachmentRecords
  /\ s.attachRequests \subseteq AttachmentRecords
  /\ s.detachReason \in DetachReasons
  /\ s.attachLicenses \subseteq 1..MaxGeneration
  /\ s.mode \in Modes
  /\ s.mustRecheck \in BOOLEAN
  /\ s.projectionState \in ProjectionStates
  /\ s.projectionGeneration \in NoGeneration..MaxGeneration
  /\ s.projectionTab \in Tabs \cup {NoTab}
  /\ s.observedGeometry \in 0..MaxGeometry
  /\ s.resizeRequestedGeometry \in NoGeometry..MaxGeometry
  /\ s.appliedGeometry \in 0..MaxGeometry
  /\ s.frameGeometry \in NoGeometry..MaxGeometry
  /\ s.feedbackRequestedGeometry \in NoGeometry..MaxGeometry
  /\ s.seenFrames \subseteq FeedbackRecords
  /\ s.acceptedFeedback \subseteq FeedbackRecords
  /\ s.deliveredControls \subseteq CommandRecords
  /\ s.refusalCoreChanged \in BOOLEAN
  /\ s.sameTargetCoreChanged \in BOOLEAN
  /\ s.rejectedSwitchSeen \in BOOLEAN
  /\ s.sameTargetNoOpSeen \in BOOLEAN
  /\ s.deadlineCleanupSupersededSeen \in BOOLEAN

AtMostOneViewerAndController ==
  /\ Cardinality(s.viewers) <= 1
  /\ Cardinality(s.controllers) <= 1
  /\ s.controllers \subseteq s.viewers

AttachIsLicensedByNoViewerOrExactDetach ==
  /\ \A attachment \in s.attachRequests:
       /\ attachment.generation \in s.attachLicenses
       /\ s.viewers = {}
  /\ \A attachment \in s.viewers:
       attachment.generation = 0
         \/ attachment.generation \in s.attachLicenses

ProjectionIsGenerationFenced ==
  /\ (s.projectionState = "none" =>
        s.projectionGeneration = NoGeneration /\ s.projectionTab = NoTab)
  /\ (s.projectionState # "none" =>
        s.projectionGeneration = s.desiredGeneration
          /\ s.projectionTab = s.desiredTab)

ReadyRequiresViewerFrameFeedbackAndLatestGeometry ==
  s.phase = "ready" =>
    /\ s.viewers = {Attachment(s.desiredGeneration, s.desiredTab)}
    /\ s.projectionState = "ready"
    /\ Feedback(s.desiredGeneration, s.frameGeometry) \in s.seenFrames
    /\ Feedback(s.desiredGeneration, s.frameGeometry) \in s.acceptedFeedback
    /\ (s.mode = "controlling" =>
         /\ s.controllers = s.viewers
         /\ s.observedGeometry = s.appliedGeometry
         /\ s.frameGeometry = s.appliedGeometry)
    /\ (s.mode = "readonly" => s.controllers = {})

ReadOnlyDoesNotAdmitControl ==
  s.phase = "ready" /\ s.mode = "readonly" =>
    ~ContainsKind(s.commands, "input") /\ ~ContainsKind(s.commands, "resize")

NoControlBehindSwitchMarker ==
  \A first \in 1..Len(s.commands):
    s.commands[first].kind = "switch" =>
      \A later \in (first + 1)..Len(s.commands):
        s.commands[later].kind = "switch"

QueuedGenerationsWereAllocated ==
  \A index \in 1..Len(s.commands):
    s.commands[index].generation <= s.generationCursor

AllocationOrExactRestoration ==
  /\ s.desiredGeneration <= s.generationCursor
  /\ (s.desiredGeneration # s.generationCursor =>
       /\ s.restoredBy \in s.markerGenerations
       /\ s.restoredGeneration = s.desiredGeneration
       /\ s.anchor.generation = s.desiredGeneration /\ s.anchor.tab = s.desiredTab)
MarkerKeysNeverLicenseAttachment == s.markerGenerations \cap s.attachLicenses = {}
CandidateWaitsForExactCleanup ==
  BeforeInstallation => s.workGeneration \notin s.attachLicenses
EmptyHasNoViewerControllerOrProjection == s.phase = "empty" =>
  /\ s.desiredTab = NoTab /\ s.workTab = NoTab
  /\ s.viewers = {} /\ s.controllers = {} /\ s.attachRequests = {}
  /\ s.projectionState = "none" /\ s.frameGeometry = NoGeometry

AllocationCursorNeverRewinds == [][s.generationCursor' >= s.generationCursor]_vars
InstalledNonemptySelectionUsesCheckedSuccessor == [][InstallSwitchCandidate =>
  /\ s.generationCursor' = s.generationCursor + 1
  /\ s.desiredGeneration' = s.generationCursor'
  /\ s.desiredGeneration' \notin s.markerGenerations]_vars
InstalledSelectionAndWorkAreAtomic == [][InstallSwitchCandidate =>
  /\ s.desiredGeneration' = s.workGeneration'
  /\ s.desiredTab' = s.candidateTab /\ s.workTab' = s.candidateTab
  /\ s.viewers = {} /\ s.controllers = {} /\ s.detachRequests = {}]_vars
CandidateInstallPreservesOriginalBudget ==
  [][InstallSwitchCandidate \/ InstallEmptySwitchCandidate =>
       UNCHANGED s.workBudgetOrigin]_vars
OnlyExactStableAnchorMayRestoreOlderDesired == [][s.desiredGeneration' < s.desiredGeneration =>
  /\ RestoreUnavailableStableAnchor /\ AnchorValid /\ s.preMarkerDrained
  /\ s.desiredGeneration' = s.anchor.generation /\ s.desiredTab' = s.anchor.tab
  /\ UNCHANGED s.generationCursor]_vars
UnavailableRestorationPreservesDrainedEffects == [][RestoreUnavailableStableAnchor =>
  UNCHANGED <<s.commands, s.deliveredControls, s.viewers, s.controllers,
              s.seenFrames, s.acceptedFeedback, s.observedGeometry,
              s.appliedGeometry, s.workBudgetOrigin>>]_vars
NoStableAnchorFromPartialSetup == [][s.anchor' # s.anchor =>
  (s.phase \in StableAnchorPhases /\ s.anchor' = Anchor(s))
  \/ s.anchor' = [s.anchor EXCEPT !.proof = FALSE]]_vars
GoneNeverReplacesHandedOffRequest ==
  [][(\E provenKey \in AttachmentRecords: KnownClosedBeforeDetach(provenKey)) =>
       s.detachRequests = {} /\ s.attachRequests = {} /\ s.preMarkerDrained]_vars
UnavailableNeverStartsSetup == [][SettleTypedUnavailable =>
  /\ s.attachLicenses' = s.attachLicenses /\ s.attachRequests' = {}
  /\ s.generationCursor' = s.generationCursor]_vars
SupersededCandidateCannotInstall == [][InstallSwitchCandidate \/ InstallEmptySwitchCandidate =>
  CurrentCandidate]_vars

BusyRequiresGenerationRecheck ==
  s.mustRecheck =>
    s.phase \in {"detach-ready", "cleanup-ready", "attach-ready",
                 "acquire-ready", "resize-ready", "await-frame",
                 "feedback-ready"}

RefusedAdmissionPreservesCore == ~s.refusalCoreChanged
SameTargetNoOpPreservesCore == ~s.sameTargetCoreChanged

NonAttachableTargetNeverAttaches ==
  \A attachment \in s.viewers \cup s.attachRequests:
    attachment.tab \in AttachableTabs

RetryableHasNoViewer ==
  s.phase = "retryable" =>
    /\ s.viewers = {}
    /\ s.controllers = {}
    /\ s.projectionState = "none"
    /\ s.projectionGeneration = NoGeneration
    /\ s.projectionTab = NoTab
    /\ s.frameGeometry = NoGeometry

SelectedNonAttachableHasNoViewer ==
  s.phase = "selected-no-attach" =>
    /\ s.desiredTab \notin AttachableTabs
    /\ s.viewers = {}
    /\ s.controllers = {}
    /\ s.projectionState = "none"
    /\ s.projectionGeneration = NoGeneration
    /\ s.projectionTab = NoTab
    /\ s.frameGeometry = NoGeometry

DeadlineCleanupSupersessionDoesNotStrandMarker ==
  s.deadlineCleanupSupersededSeen /\ s.carrierLive /\ s.phase \in StablePhases =>
    ~ContainsKind(s.commands, "switch")

ExitIsAlwaysAdmitted ==
  s.phase # "closed" => ENABLED (Exit /\ Next)

SwitchEventuallySettles ==
  (s.phase \in UnsettledPhases) ~> (s.phase \in StablePhases)

=============================================================================
