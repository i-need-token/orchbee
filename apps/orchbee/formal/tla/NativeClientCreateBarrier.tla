------------------------- MODULE NativeClientCreateBarrier -------------------------
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS MaxQueue, InitialOrigin

ASSUME /\ MaxQueue \in Nat \ {0}
       /\ InitialOrigin \in {"ready", "empty"}

OldTab == "old-tab"
NewTab == "new-tab"
OtherTab == "other-tab"
NoTab == "no-tab"

BaselineOrder == IF InitialOrigin = "ready" THEN <<OldTab, OtherTab>> ELSE <<>>
CreatedOrder == BaselineOrder \o <<NewTab>>
MovedCreatedOrder == IF InitialOrigin = "ready"
                     THEN <<NewTab, OtherTab, OldTab>> ELSE CreatedOrder
RemovedCreatedOrder == BaselineOrder
InitialActive == IF InitialOrigin = "ready" THEN OldTab ELSE NoTab
InitialProvenance == IF InitialOrigin = "ready" THEN <<"started", "unavailable">> ELSE <<>>
ProjectionOrders == {BaselineOrder, CreatedOrder, MovedCreatedOrder}
IndexOf(order, tab) ==
  IF tab = NoTab THEN 0
  ELSE CHOOSE index \in 1..Len(order): order[index] = tab
ContainsTab(order, tab) ==
  \E index \in 1..Len(order): order[index] = tab
CandidateTarget(order) ==
  IF ContainsTab(order, NewTab) THEN NewTab
  ELSE IF order = <<>> THEN NoTab
  ELSE order[Len(order)]
CandidateGeneration(order) ==
  IF CandidateTarget(order) = NoTab THEN 0
  ELSE IF CandidateTarget(order) = NewTab THEN 1
  ELSE 2
AlignedProvenance(order, outcome) ==
  [index \in 1..Len(order) |->
    IF order[index] = OldTab THEN "started"
    ELSE IF order[index] = OtherTab THEN "unavailable"
    ELSE outcome]

OldAttachment == [generation |-> 0, tab |-> OldTab]
NewAttachment == [generation |-> 1, tab |-> NewTab]
Attachments == {OldAttachment, NewAttachment}
InitialViewers == IF InitialOrigin = "ready" THEN {OldAttachment} ELSE {}

CommandKinds == {"input", "resize", "poll", "create"}
Flights == {"idle", "polling", "waiting-draw", "feedback-ready", "feedback-in-flight"}
CreateOutcomes == {"none", "started", "pending", "unavailable", "failed", "unknown"}
OldCleanupPhases == {"old-cleanup-ready", "old-cleanup-wait"}

UnsettledPhases == {
  "creating", "need-snapshot", "detach-old", "detach-old-wait", "install-candidate",
  "handoff", "attach-wait", "acquire", "first-frame", "frame-staged",
  "frame-drawn", "feedback-wait", "cleanup", "cleanup-wait"
} \cup OldCleanupPhases

StablePhases == {
  "ready", "empty", "precommit-restored", "projection-unavailable",
  "selected-no-attach", "retryable-unavailable", "retired", "closed"
}

Phases == UnsettledPhases \cup StablePhases

CoreSnapshot(state) == [
  active |-> state.active,
  desired |-> state.desired,
  cacheHasNew |-> state.cacheHasNew,
  cacheRevision |-> state.cacheRevision,
  cacheOrder |-> state.cacheOrder,
  provenance |-> state.provenance,
  activeIndex |-> state.activeIndex,
  activeGeneration |-> state.activeGeneration,
  viewers |-> state.viewers,
  controllers |-> state.controllers,
  projection |-> state.projection
]

Retired(state) ==
  [state EXCEPT
    !.phase = "retired",
    !.carrierLive = FALSE,
    !.mailbox = <<>>,
    !.flight = "idle",
    !.viewers = {},
    !.controllers = {},
    !.detachRequests = {},
    !.attachRequests = {},
    !.projection = NoTab]

\* Initial Ready represents an OldAttachment generation whose setup already
\* completed its first draw and accepted the matching feedback.  The two
\* booleans below describe only a future NewAttachment setup, so they start
\* FALSE and do not duplicate that historical Ready fact.
Initial(controllerSet) == [
  phase |-> InitialOrigin,
  active |-> InitialActive,
  desired |-> InitialActive,
  cacheHasNew |-> FALSE,
  cacheRevision |-> 1,
  cacheOrder |-> BaselineOrder,
  provenance |-> InitialProvenance,
  activeIndex |-> IF InitialOrigin = "ready" THEN 1 ELSE 0,
  serverOrder |-> BaselineOrder,
  serverRevision |-> 1,
  returnedTab |-> NewTab,
  candidateOrder |-> BaselineOrder,
  candidateRevision |-> 1,
  candidateTarget |-> NoTab,
  candidateProvenance |-> InitialProvenance,
  candidateGeneration |-> 0,
  generationCursor |-> 0,
  activeGeneration |-> 0,
  oldCleanupProof |-> "none",
  viewers |-> InitialViewers,
  controllers |-> controllerSet,
  projection |-> InitialActive,
  mailbox |-> <<>>,
  flight |-> "idle",
  carrierLive |-> TRUE,
  createDispatched |-> FALSE,
  createOrigin |-> "none",
  observedGeometry |-> 0,
  capturedGeometry |-> 0,
  geometryChanged |-> FALSE,
  commitKnown |-> FALSE,
  createOutcome |-> "none",
  snapshotVerified |-> FALSE,
  detachRequests |-> {},
  attachRequests |-> {},
  freshFrameDrawn |-> FALSE,
  newFeedbackAccepted |-> FALSE,
  saved |-> [
    active |-> InitialActive,
    desired |-> InitialActive,
    cacheHasNew |-> FALSE,
    cacheRevision |-> 1,
    cacheOrder |-> BaselineOrder,
    provenance |-> InitialProvenance,
    activeIndex |-> IF InitialOrigin = "ready" THEN 1 ELSE 0,
    activeGeneration |-> 0,
    viewers |-> InitialViewers,
    controllers |-> controllerSet,
    projection |-> InitialActive
  ],
  refusedAdmissionSeen |-> FALSE,
  refusalCoreChanged |-> FALSE,
  exitObserved |-> FALSE
]

VARIABLE s

vars == <<s>>

Init == \E controllerSet \in SUBSET InitialViewers: s = Initial(controllerSet)

Contains(queue, kind) ==
  \E index \in 1..Len(queue): queue[index] = kind

QueueControl(kind) ==
  /\ kind \in {"input", "resize"}
  /\ s.carrierLive
  /\ s.phase = "ready"
  /\ s.controllers # {}
  /\ Len(s.mailbox) < MaxQueue
  /\ s' = [s EXCEPT !.mailbox = Append(@, kind)]

QueuePoll ==
  /\ s.carrierLive
  /\ s.phase = "ready"
  /\ s.flight = "idle"
  /\ ~Contains(s.mailbox, "poll")
  /\ Len(s.mailbox) < MaxQueue
  /\ s' = [s EXCEPT !.mailbox = Append(@, "poll")]

AdmitCreateCore ==
  /\ s.carrierLive
  /\ s.phase \in {"ready", "empty"}
  /\ ~s.commitKnown
  /\ Len(s.mailbox) < MaxQueue
  /\ s' = [s EXCEPT
       !.mailbox = Append(@, "create"),
       !.phase = "creating",
       !.createOrigin = s.phase,
       !.capturedGeometry = s.observedGeometry,
       !.saved = CoreSnapshot(s)]

AdmitCreate ==
  /\ s.phase = "ready"
  /\ s.flight = "idle"
  /\ AdmitCreateCore

AdmitEmptyCreate ==
  /\ s.phase = "empty"
  /\ s.flight = "idle"
  /\ AdmitCreateCore

CaptureGeometryChange ==
  /\ s.phase \notin {"retired", "closed"}
  /\ ~s.geometryChanged
  /\ s' = [s EXCEPT !.observedGeometry = 1, !.geometryChanged = TRUE]

\* Admission is based on the already-established Ready generation.  An old
\* Poll flight may drain ahead of the FIFO Create barrier; Create dispatch
\* remains forbidden until that flight becomes idle.
AdmitCreateWithOutstandingFlight ==
  /\ s.flight # "idle"
  /\ AdmitCreateCore

RejectCreateAtCapacity ==
  /\ s.carrierLive
  /\ s.phase = "ready"
  /\ Len(s.mailbox) = MaxQueue
  /\ ~s.refusedAdmissionSeen
  /\ LET next == [s EXCEPT !.refusedAdmissionSeen = TRUE] IN
       s' = [next EXCEPT
         !.refusalCoreChanged =
           @ \/ (CoreSnapshot(next) # CoreSnapshot(s))]

DeliverOldControl ==
  /\ s.mailbox # <<>>
  /\ Head(s.mailbox) \in {"input", "resize"}
  /\ s' = [s EXCEPT !.mailbox = Tail(@)]

BeginOldPoll ==
  /\ s.mailbox # <<>>
  /\ Head(s.mailbox) = "poll"
  /\ s.flight = "idle"
  /\ s' = [s EXCEPT
       !.mailbox = Tail(@),
       !.flight = "polling"]

OldPollProducesFeedback ==
  /\ s.flight = "polling"
  /\ s' = [s EXCEPT !.flight = "waiting-draw"]

DrawOldPollResult ==
  /\ s.flight = "waiting-draw"
  /\ s' = [s EXCEPT !.flight = "feedback-ready"]

\* A valid projection or NeedSnapshot continues through causal feedback above.
\* Ordinary ProjectionBlocked draws no frame and therefore owes no feedback.
DrawOldBlockedWithoutFeedback ==
  /\ s.flight = "waiting-draw"
  /\ s' = [s EXCEPT !.flight = "idle"]

SendCausalFeedback ==
  /\ s.flight = "feedback-ready"
  /\ s' = [s EXCEPT !.flight = "feedback-in-flight"]

AcceptCausalFeedback ==
  /\ s.flight = "feedback-in-flight"
  /\ s' = [s EXCEPT !.flight = "idle"]

DispatchCreate ==
  /\ s.phase = "creating"
  /\ s.mailbox # <<>>
  /\ Head(s.mailbox) = "create"
  /\ s.flight = "idle"
  /\ s.detachRequests = {}
  /\ s.attachRequests = {}
  /\ s' = [s EXCEPT
       !.mailbox = Tail(@),
       !.createDispatched = TRUE]

RestorePrecommitCore ==
  /\ s.phase = "creating"
  /\ s.createDispatched
  /\ s' = [s EXCEPT
       !.phase = "precommit-restored",
       !.active = s.saved.active,
       !.desired = s.saved.desired,
       !.cacheHasNew = s.saved.cacheHasNew,
       !.cacheRevision = s.saved.cacheRevision,
       !.cacheOrder = s.saved.cacheOrder,
       !.provenance = s.saved.provenance,
       !.activeIndex = s.saved.activeIndex,
       !.activeGeneration = s.saved.activeGeneration,
       !.viewers = s.saved.viewers,
       !.controllers = s.saved.controllers,
       !.projection = s.saved.projection,
       !.createDispatched = FALSE]

CreatePrecommitRefusal ==
  /\ s.createOrigin = "ready"
  /\ RestorePrecommitCore

RestoreEmptyPrecommit ==
  /\ s.createOrigin = "empty"
  /\ RestorePrecommitCore

\* The saved CoreSnapshot restores the same historically Ready attachment
\* generation; a precommit refusal does not establish a new readiness fact.
ResumeAfterPrecommit ==
  /\ s.phase = "precommit-restored"
  /\ s' = [s EXCEPT !.phase = s.createOrigin]

CreateCommits(outcome, returnedTab) ==
  /\ returnedTab \in {NewTab, OldTab}
  /\ outcome \in CreateOutcomes \ {"none"}
  /\ s.phase = "creating"
  /\ s.createDispatched
  /\ s' = [s EXCEPT
       !.phase = "need-snapshot",
       !.createDispatched = FALSE,
       !.commitKnown = TRUE,
       !.createOutcome = outcome,
       !.serverOrder = CreatedOrder,
       !.serverRevision = 2,
       !.returnedTab = returnedTab,
       !.generationCursor = 1]

VerifyCommittedSnapshot(revision) ==
  /\ revision \in 2..3
  /\ s.phase = "need-snapshot"
  /\ s.commitKnown
  /\ ~ContainsTab(BaselineOrder, s.returnedTab)
  /\ s.returnedTab = NewTab
  /\ revision = s.serverRevision
  /\ (revision = 2 => s.serverOrder = CreatedOrder)
  /\ LET target == CandidateTarget(s.serverOrder)
         generation == CandidateGeneration(s.serverOrder)
         candidateProvenance == AlignedProvenance(s.serverOrder, s.createOutcome)
     IN s' = [s EXCEPT
          !.candidateOrder = s.serverOrder,
          !.candidateRevision = revision,
          !.candidateTarget = target,
          !.candidateProvenance = candidateProvenance,
          !.candidateGeneration = generation,
          !.snapshotVerified = TRUE,
          !.oldCleanupProof = IF s.createOrigin = "empty" THEN "no-old" ELSE "none",
          !.phase = IF s.createOrigin = "ready" THEN "detach-old"
                    ELSE "install-candidate"]

CommittedSnapshotUnavailable ==
  /\ s.phase = "need-snapshot"
  /\ s' = [s EXCEPT !.phase = "projection-unavailable"]

CommittedSnapshotDiverges ==
  /\ s.phase = "need-snapshot"
  /\ s' = Retired(s)

MoveCreatedBeforeProjection ==
  /\ s.commitKnown
  /\ s.phase \in {"need-snapshot", "projection-unavailable"}
  /\ s.serverRevision = 2
  /\ s' = [s EXCEPT !.serverOrder = MovedCreatedOrder, !.serverRevision = 3]

RemoveCreatedBeforeProjection ==
  /\ s.commitKnown
  /\ s.phase = "need-snapshot"
  /\ s.serverRevision = 2
  /\ s' = [s EXCEPT !.serverOrder = RemovedCreatedOrder, !.serverRevision = 3]

VerifyMovedCreatedProjection ==
  /\ s.serverOrder = MovedCreatedOrder
  /\ s.serverRevision = 3
  /\ VerifyCommittedSnapshot(3)

VerifyRemovedCreatedProjection ==
  /\ s.serverOrder = RemovedCreatedOrder
  /\ s.serverRevision = 3
  /\ VerifyCommittedSnapshot(3)

RejectReusedCreatedId ==
  /\ ContainsTab(BaselineOrder, s.returnedTab)
  /\ ~s.snapshotVerified
  /\ CommittedSnapshotDiverges

RequestOldDetach ==
  /\ s.phase = "detach-old"
  /\ s.snapshotVerified
  /\ s.createOrigin = "ready"
  /\ s.oldCleanupProof = "none"
  /\ s.viewers = {OldAttachment}
  /\ s.detachRequests = {}
  /\ s' = [s EXCEPT
       !.phase = "detach-old-wait",
       !.detachRequests = {OldAttachment}]

OldDetachCompletes ==
  /\ s.phase = "detach-old-wait"
  /\ s.detachRequests = {OldAttachment}
  /\ s' = [s EXCEPT
       !.phase = "install-candidate",
       !.detachRequests = {},
       !.viewers = @ \ {OldAttachment},
       !.controllers = @ \ {OldAttachment},
       !.projection = NoTab,
       !.oldCleanupProof = "detached"]

CompleteOldCleanupFromKnownClosed ==
  /\ s.phase = "detach-old"
  /\ s.snapshotVerified
  /\ s.createOrigin = "ready"
  /\ s.viewers = {OldAttachment}
  /\ s.detachRequests = {}
  /\ s.oldCleanupProof = "none"
  /\ s' = [s EXCEPT
       !.phase = "install-candidate",
       !.viewers = @ \ {OldAttachment},
       !.controllers = @ \ {OldAttachment},
       !.projection = NoTab,
       !.oldCleanupProof = "known-closed"]

OldDetachFailsOrIsAmbiguous ==
  /\ s.phase \in {"detach-old", "detach-old-wait"} \cup OldCleanupPhases
  /\ s.commitKnown
  /\ s' = Retired(s)

ExpireOldDetachBeforeHandoff ==
  /\ s.phase = "detach-old"
  /\ s.createOrigin = "ready"
  /\ s.snapshotVerified
  /\ s' = [s EXCEPT !.phase = "old-cleanup-ready"]

ExpireOldDetachAfterHandoff ==
  /\ s.phase = "detach-old-wait"
  /\ s' = [s EXCEPT !.phase = "old-cleanup-wait"]

RequestOldDetachInReserve ==
  /\ s.phase = "old-cleanup-ready"
  /\ s.viewers = {OldAttachment}
  /\ s.detachRequests = {}
  /\ s' = [s EXCEPT
       !.phase = "old-cleanup-wait",
       !.detachRequests = {OldAttachment}]

OldDetachCompletesInReserve ==
  /\ s.phase = "old-cleanup-wait"
  /\ s.detachRequests = {OldAttachment}
  /\ s' = [Retired(s) EXCEPT !.oldCleanupProof = "detached"]

CompleteOldCleanupFromKnownClosedInReserve ==
  /\ s.phase = "old-cleanup-ready"
  /\ s.snapshotVerified
  /\ s.createOrigin = "ready"
  /\ s.viewers = {OldAttachment}
  /\ s.detachRequests = {}
  /\ s.oldCleanupProof = "none"
  /\ s' = [Retired(s) EXCEPT !.oldCleanupProof = "known-closed"]

InstallValidatedCandidate ==
  /\ s.phase = "install-candidate"
  /\ s.snapshotVerified
  /\ s.viewers = {}
  /\ s.detachRequests = {}
  /\ s.attachRequests = {}
  /\ s.oldCleanupProof \in {"detached", "known-closed", "no-old"}
  /\ (s.createOrigin = "ready" => s.oldCleanupProof \in {"detached", "known-closed"})
  /\ (s.createOrigin = "empty" => s.oldCleanupProof = "no-old")
  /\ LET target == s.candidateTarget
     IN s' = [s EXCEPT
          !.cacheHasNew = ContainsTab(s.candidateOrder, NewTab),
          !.cacheRevision = s.candidateRevision,
          !.cacheOrder = s.candidateOrder,
          !.provenance = s.candidateProvenance,
          !.active = target,
          !.desired = target,
          !.activeIndex = IndexOf(s.candidateOrder, target),
          !.activeGeneration = s.candidateGeneration,
          !.generationCursor = IF s.candidateGeneration = 2 THEN 2 ELSE s.generationCursor,
          !.projection = NoTab,
          !.phase = IF target = NoTab THEN "empty"
                    ELSE IF target = NewTab /\ s.createOutcome = "started"
                    THEN "handoff"
                    ELSE "selected-no-attach"]

RequestNewAttach ==
  /\ s.phase = "handoff"
  /\ s.createOutcome = "started"
  /\ s.snapshotVerified
  /\ s.viewers = {}
  /\ s.attachRequests = {}
  /\ s' = [s EXCEPT
       !.phase = "attach-wait",
       !.attachRequests = {NewAttachment}]

NewAttachCompletes ==
  /\ s.phase = "attach-wait"
  /\ s.attachRequests = {NewAttachment}
  /\ s' = [s EXCEPT
       !.phase = "acquire",
       !.attachRequests = {},
       !.viewers = @ \cup {NewAttachment}]

NewAttachFails ==
  /\ s.phase \in {"handoff", "attach-wait"}
  /\ s.viewers = {}
  /\ s' = [s EXCEPT
       !.phase = "retryable-unavailable",
       !.attachRequests = {}]

NewAttachAmbiguous ==
  /\ s.phase = "attach-wait"
  /\ s' = Retired(s)

AcquireNewController ==
  /\ s.phase = "acquire"
  /\ s.viewers = {NewAttachment}
  /\ s.controllers = {}
  /\ s' = [s EXCEPT
       !.phase = "first-frame",
       !.controllers = @ \cup {NewAttachment}]

AcquireNewReadOnly ==
  /\ s.phase = "acquire"
  /\ s.viewers = {NewAttachment}
  /\ s.controllers = {}
  /\ s' = [s EXCEPT !.phase = "first-frame"]

ReceiveFreshFullFrame ==
  /\ s.phase = "first-frame"
  /\ s.viewers = {NewAttachment}
  /\ s' = [s EXCEPT
       !.phase = "frame-staged",
       !.projection = NewTab]

DrawFreshFrame ==
  /\ s.phase = "frame-staged"
  /\ s.projection = NewTab
  /\ s' = [s EXCEPT
       !.phase = "frame-drawn",
       !.freshFrameDrawn = TRUE]

SendFreshFrameFeedback ==
  /\ s.phase = "frame-drawn"
  /\ s.freshFrameDrawn
  /\ s' = [s EXCEPT !.phase = "feedback-wait"]

AcceptFreshFrameFeedback ==
  /\ s.phase = "feedback-wait"
  /\ s' = [s EXCEPT
       !.phase = "ready",
       !.newFeedbackAccepted = TRUE]

BeginSetupCleanup ==
  /\ s.phase \in {
       "acquire", "first-frame", "frame-staged", "frame-drawn", "feedback-wait"
     }
  /\ s.viewers = {NewAttachment}
  /\ s' = [s EXCEPT
       !.phase = "cleanup-wait",
       !.detachRequests = {NewAttachment}]

SetupCleanupCompletes ==
  /\ s.phase = "cleanup-wait"
  /\ s.detachRequests = {NewAttachment}
  /\ s' = [s EXCEPT
       !.phase = "retryable-unavailable",
       !.detachRequests = {},
       !.viewers = @ \ {NewAttachment},
       !.controllers = @ \ {NewAttachment},
       !.projection = NoTab]

SetupCleanupFailsOrIsAmbiguous ==
  /\ s.phase \in {"cleanup", "cleanup-wait"}
  /\ s' = Retired(s)

ExpireOperationDeadline ==
  /\ s.phase \in UnsettledPhases \ (OldCleanupPhases \cup
       {"cleanup", "cleanup-wait", "detach-old", "detach-old-wait"})
  /\ s' = IF s.phase \in {"creating", "need-snapshot", "attach-wait", "feedback-wait"}
                \/ s.viewers = {}
            THEN Retired(s)
            ELSE [s EXCEPT
                   !.phase = "cleanup-wait",
                  !.detachRequests = s.viewers,
                  !.attachRequests = {}]

ExpireCleanupReserve ==
  /\ s.phase \in {"cleanup", "cleanup-wait"} \cup OldCleanupPhases
  /\ s' = Retired(s)

Exit ==
  /\ s.phase # "closed"
  /\ s' = [Retired(s) EXCEPT
       !.phase = "closed",
       !.exitObserved = TRUE]

Next ==
  \/ Exit
  \/ \E kind \in {"input", "resize"}: QueueControl(kind)
  \/ QueuePoll
  \/ AdmitCreate
  \/ AdmitEmptyCreate
  \/ CaptureGeometryChange
  \/ AdmitCreateWithOutstandingFlight
  \/ RejectCreateAtCapacity
  \/ DeliverOldControl
  \/ BeginOldPoll
  \/ OldPollProducesFeedback
  \/ DrawOldPollResult
  \/ DrawOldBlockedWithoutFeedback
  \/ SendCausalFeedback
  \/ AcceptCausalFeedback
  \/ DispatchCreate
  \/ CreatePrecommitRefusal
  \/ RestoreEmptyPrecommit
  \/ ResumeAfterPrecommit
  \/ \E outcome \in CreateOutcomes \ {"none"}, returnedTab \in {NewTab, OldTab}:
       CreateCommits(outcome, returnedTab)
  \/ InstallValidatedCandidate
  \/ RequestOldDetach
  \/ OldDetachCompletes
  \/ CompleteOldCleanupFromKnownClosed
  \/ OldDetachFailsOrIsAmbiguous
  \/ ExpireOldDetachBeforeHandoff
  \/ ExpireOldDetachAfterHandoff
  \/ RequestOldDetachInReserve
  \/ OldDetachCompletesInReserve
  \/ CompleteOldCleanupFromKnownClosedInReserve
  \/ \E revision \in 2..3: VerifyCommittedSnapshot(revision)
  \/ CommittedSnapshotUnavailable
  \/ CommittedSnapshotDiverges
  \/ MoveCreatedBeforeProjection
  \/ RemoveCreatedBeforeProjection
  \/ VerifyMovedCreatedProjection
  \/ VerifyRemovedCreatedProjection
  \/ RejectReusedCreatedId
  \/ RequestNewAttach
  \/ NewAttachCompletes
  \/ NewAttachFails
  \/ NewAttachAmbiguous
  \/ AcquireNewController
  \/ AcquireNewReadOnly
  \/ ReceiveFreshFullFrame
  \/ DrawFreshFrame
  \/ SendFreshFrameFeedback
  \/ AcceptFreshFrameFeedback
  \/ BeginSetupCleanup
  \/ SetupCleanupCompletes
  \/ SetupCleanupFailsOrIsAmbiguous
  \/ ExpireOperationDeadline
  \/ ExpireCleanupReserve

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(ExpireOperationDeadline \/ ExpireOldDetachBeforeHandoff
             \/ ExpireOldDetachAfterHandoff)
  /\ WF_vars(InstallValidatedCandidate)
  /\ WF_vars(ExpireCleanupReserve)

TypeOK ==
  /\ s.phase \in Phases
  /\ s.active \in {OldTab, OtherTab, NewTab, NoTab}
  /\ s.desired \in {OldTab, OtherTab, NewTab, NoTab}
  /\ s.cacheHasNew \in BOOLEAN
  /\ s.cacheRevision \in 1..3
  /\ s.cacheOrder \in ProjectionOrders
  /\ s.serverOrder \in ProjectionOrders
  /\ s.serverRevision \in 1..3
  /\ s.candidateOrder \in ProjectionOrders
  /\ s.candidateRevision \in 1..3
  /\ s.candidateTarget \in {OtherTab, NewTab, NoTab}
  /\ s.candidateProvenance \in Seq(CreateOutcomes)
  /\ Len(s.candidateProvenance) = Len(s.candidateOrder)
  /\ s.candidateGeneration \in 0..2
  /\ s.generationCursor \in 0..2
  /\ s.activeGeneration \in 0..2
  /\ s.oldCleanupProof \in {"none", "no-old", "detached", "known-closed"}
  /\ s.returnedTab \in {NewTab, OldTab}
  /\ s.activeIndex \in 0..3
  /\ s.provenance \in Seq(CreateOutcomes \cup {"binding-only"})
  /\ Len(s.provenance) = Len(s.cacheOrder)
  /\ s.viewers \subseteq Attachments
  /\ s.controllers \subseteq Attachments
  /\ s.projection \in {OldTab, NewTab, NoTab}
  /\ s.mailbox \in Seq(CommandKinds)
  /\ Len(s.mailbox) <= MaxQueue
  /\ s.flight \in Flights
  /\ s.carrierLive \in BOOLEAN
  /\ s.createDispatched \in BOOLEAN
  /\ s.createOrigin \in {"none", "ready", "empty"}
  /\ s.observedGeometry \in 0..1
  /\ s.capturedGeometry \in 0..1
  /\ s.geometryChanged \in BOOLEAN
  /\ s.commitKnown \in BOOLEAN
  /\ s.createOutcome \in CreateOutcomes
  /\ s.snapshotVerified \in BOOLEAN
  /\ s.detachRequests \subseteq Attachments
  /\ s.attachRequests \subseteq Attachments
  /\ s.freshFrameDrawn \in BOOLEAN
  /\ s.newFeedbackAccepted \in BOOLEAN
  /\ s.refusedAdmissionSeen \in BOOLEAN
  /\ s.refusalCoreChanged \in BOOLEAN
  /\ s.exitObserved \in BOOLEAN

AtMostOneViewerAndController ==
  /\ Cardinality(s.viewers) <= 1
  /\ Cardinality(s.controllers) <= 1
  /\ s.controllers \subseteq s.viewers

CreateMarkerIsLast ==
  \A index \in 1..Len(s.mailbox):
    s.mailbox[index] = "create" => index = Len(s.mailbox)

CreateDispatchIsQuiescent ==
  s.createDispatched =>
    /\ s.mailbox = <<>>
    /\ s.flight = "idle"

PrecommitRestoresExactCore ==
  s.phase = "precommit-restored" => CoreSnapshot(s) = s.saved

RefusedAdmissionPreservesCore ==
  ~s.refusalCoreChanged

ValidatedCandidatePrecedesOldCleanup ==
  (OldAttachment \in s.detachRequests \/ s.oldCleanupProof \in {"detached", "known-closed"}) =>
    /\ s.snapshotVerified
    /\ s.candidateRevision \in 2..3
    /\ s.candidateTarget = CandidateTarget(s.candidateOrder)
    /\ s.candidateProvenance = AlignedProvenance(s.candidateOrder, s.createOutcome)

CandidateWaitsForExactCleanup ==
  s.phase \in {"detach-old", "detach-old-wait"} \cup OldCleanupPhases =>
    CoreSnapshot(s) = s.saved

NoReplacementSelectionBeforeCleanup ==
  s.cacheRevision > 1 =>
    /\ s.snapshotVerified
    /\ OldAttachment \notin s.viewers
    /\ s.oldCleanupProof \in {"detached", "known-closed", "no-old"}
    /\ s.cacheOrder = s.candidateOrder
    /\ s.active = s.candidateTarget
    /\ s.desired = s.candidateTarget
    /\ s.activeGeneration = s.candidateGeneration

CacheBackedActiveSelection ==
  /\ s.desired = s.active
  /\ (s.active = NoTab <=> s.cacheOrder = <<>>)
  /\ (s.active # NoTab => ContainsTab(s.cacheOrder, s.active))

SnapshotUnavailablePreservesOldCore ==
  s.phase = "projection-unavailable" =>
    /\ s.commitKnown /\ ~s.snapshotVerified
    /\ CoreSnapshot(s) = s.saved

ReservedGenerationIsHistorical ==
  /\ (s.commitKnown => s.generationCursor >= 1)
  /\ (~s.commitKnown => s.generationCursor = 0)
  /\ (s.cacheRevision = 1 /\ s.commitKnown => s.generationCursor = 1)
  /\ s.activeGeneration <= s.generationCursor
  /\ (s.active = NewTab => s.activeGeneration = 1)
  /\ (s.active = OtherTab => s.activeGeneration = 2 /\ s.generationCursor = 2)
  /\ (s.active = NoTab => s.activeGeneration = 0)

NewAttachRequestRequiresOldDetachAndSnapshot ==
  NewAttachment \in s.attachRequests =>
    /\ OldAttachment \notin s.viewers
    /\ s.createOutcome = "started"
    /\ s.snapshotVerified
    /\ s.active = NewTab

NewViewerRequiresStartedVerifiedSnapshot ==
  NewAttachment \in s.viewers =>
    /\ s.createOutcome = "started"
    /\ s.snapshotVerified
    /\ s.active = NewTab

NonStartedNeverAttaches ==
  s.createOutcome \in {"pending", "unavailable", "failed", "unknown"} =>
    NewAttachment \notin s.viewers

ReserveCleanupNeverInstallsCandidate ==
  s.phase \in OldCleanupPhases =>
    /\ s.commitKnown
    /\ s.active = OldTab
    /\ s.desired = OldTab
    /\ ~s.cacheHasNew
    /\ s.snapshotVerified
    /\ s.cacheRevision = 1
    /\ s.attachRequests = {}

CreatedReadyRequiresDrawAndFeedback ==
  s.phase = "ready" /\ s.active = NewTab =>
    /\ s.projection = NewTab
    /\ s.freshFrameDrawn
    /\ s.newFeedbackAccepted

ExitIsAlwaysEnabled ==
  s.phase # "closed" => ENABLED (Exit /\ Next)

BarrierEventuallySettles ==
  (s.phase \in UnsettledPhases) ~> (s.phase \in StablePhases)


ActiveOrdinalFollowsId ==
  s.activeIndex = IndexOf(s.cacheOrder, s.active)

CreatedProjectionPreservesPriorProvenance ==
  /\ (InitialOrigin = "ready" =>
       /\ s.provenance[IndexOf(s.cacheOrder, OldTab)] = "started"
       /\ s.provenance[IndexOf(s.cacheOrder, OtherTab)] = "unavailable")
  /\ (s.cacheRevision > 1 =>
       /\ s.returnedTab = NewTab
       /\ ~ContainsTab(BaselineOrder, s.returnedTab)
       /\ (ContainsTab(s.cacheOrder, NewTab) =>
            s.provenance[IndexOf(s.cacheOrder, NewTab)] = s.createOutcome)
       /\ s.provenance = AlignedProvenance(s.cacheOrder, s.createOutcome)
       /\ (s.cacheRevision = 2 => s.cacheOrder = CreatedOrder))

NoCacheReplacementDuringDetach ==
  [][(s.phase \in {"detach-old", "detach-old-wait"} \cup OldCleanupPhases) =>
    UNCHANGED <<s.cacheOrder, s.provenance>>]_vars

EmptyHasNoFabricatedReadyOrAttachment ==
  s.phase = "empty" =>
    /\ s.cacheOrder = <<>> /\ s.active = NoTab /\ s.desired = NoTab
    /\ s.viewers = {} /\ s.controllers = {} /\ s.projection = NoTab

EmptyOriginNeverDetachesImaginaryViewer ==
  s.createOrigin = "empty" => OldAttachment \notin s.detachRequests

CapturedGeometryIsImmutableDuringBarrier ==
  [][(s.phase \in UnsettledPhases) =>
    UNCHANGED <<s.capturedGeometry, s.createOrigin>>]_vars

CandidateIsImmutable ==
  [][s.snapshotVerified => UNCHANGED <<s.snapshotVerified, s.candidateOrder,
       s.candidateRevision, s.candidateTarget, s.candidateProvenance,
       s.candidateGeneration>>]_vars

HistoricalCommitNeverForgotten ==
  [][s.commitKnown => s'.commitKnown]_vars

GenerationNeverRewinds ==
  [][s'.generationCursor >= s.generationCursor]_vars

OldViewerCannotDisappearOnALiveCarrierWithoutProof ==
  [][(s.carrierLive /\ s'.carrierLive /\ OldAttachment \in s.viewers
       /\ OldAttachment \notin s'.viewers) =>
       (OldDetachCompletes \/ CompleteOldCleanupFromKnownClosed)]_vars

ReserveCompletionNeverInstalls ==
  [][(s.phase \in OldCleanupPhases /\ s'.phase \notin OldCleanupPhases) =>
       /\ ~s'.carrierLive
       /\ UNCHANGED <<s.cacheOrder, s.cacheRevision, s.active, s.desired,
                       s.activeGeneration, s.generationCursor>>]_vars

=============================================================================
