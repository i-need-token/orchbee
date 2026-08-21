---------------- MODULE DesktopTerminalResizeLease ----------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS Attachments, NoAttachment, MaxRevision, Scenario

ResizeScenarios == {"lease", "broker-serialization"}

OperationKinds == {"idle", "open", "release"}
Actions == {
  "init",
  "begin-open",
  "commit-open",
  "begin-release",
  "commit-release",
  "resize",
  "runtime-advance",
  "synchronize-resize",
  "measure-broker-resize",
  "settle-broker-resize",
  "quiesce-broker-resize",
  "cleanup-broker-resize"
}

VARIABLES
  attached,
  owner,
  admissionRank,
  nextRank,
  operationKind,
  operationAttachment,
  releaseSuccessor,
  canonicalGeometry,
  canonicalRevision,
  observedGeometry,
  observedRevision,
  resizeObservationPending,
  resizeRequestGeometry,
  resizeRevision,
  lastAction,
  lastActor,
  previousGeometry,
  previousOwner,
  lastPromoted,
  brokerResizeActive,
  brokerActiveGeometry,
  brokerResizePending,
  brokerPendingGeometry,
  brokerMeasurementsOpen,
  brokerMeasurementSeen,
  brokerLatestGeometry,
  brokerCleanupRequested,
  brokerLastAction,
  brokerPreviousActive,
  brokerPreviousActiveGeometry,
  brokerPreviousPending,
  brokerPreviousPendingGeometry

LeaseVars == <<
  attached,
  owner,
  admissionRank,
  nextRank,
  operationKind,
  operationAttachment,
  releaseSuccessor,
  canonicalGeometry,
  canonicalRevision,
  observedGeometry,
  observedRevision,
  resizeObservationPending,
  resizeRequestGeometry,
  resizeRevision,
  lastAction,
  lastActor,
  previousGeometry,
  previousOwner,
  lastPromoted
>>

BrokerVars == <<
  brokerResizeActive,
  brokerActiveGeometry,
  brokerResizePending,
  brokerPendingGeometry,
  brokerMeasurementsOpen,
  brokerMeasurementSeen,
  brokerLatestGeometry,
  brokerCleanupRequested,
  brokerLastAction,
  brokerPreviousActive,
  brokerPreviousActiveGeometry,
  brokerPreviousPending,
  brokerPreviousPendingGeometry
>>

vars == <<LeaseVars, BrokerVars>>

Oldest(candidates) ==
  CHOOSE candidate \in candidates:
    \A other \in candidates: admissionRank[candidate] <= admissionRank[other]

Record(action, actor) ==
  /\ lastAction' = action
  /\ lastActor' = actor
  /\ previousGeometry' = canonicalGeometry
  /\ previousOwner' = owner

BrokerRecord(action) ==
  /\ brokerLastAction' = action
  /\ brokerPreviousActive' = brokerResizeActive
  /\ brokerPreviousActiveGeometry' = brokerActiveGeometry
  /\ brokerPreviousPending' = brokerResizePending
  /\ brokerPreviousPendingGeometry' = brokerPendingGeometry

LeaseInit ==
  /\ attached = {}
  /\ owner = NoAttachment
  /\ admissionRank = [attachment \in Attachments |-> 0]
  /\ nextRank = 1
  /\ operationKind = "idle"
  /\ operationAttachment = NoAttachment
  /\ releaseSuccessor = NoAttachment
  /\ canonicalGeometry = 0
  /\ canonicalRevision = 0
  /\ observedGeometry = 0
  /\ observedRevision = 0
  /\ resizeObservationPending = FALSE
  /\ resizeRequestGeometry = 0
  /\ resizeRevision = 0
  /\ lastAction = "init"
  /\ lastActor = NoAttachment
  /\ previousGeometry = canonicalGeometry
  /\ previousOwner = owner
  /\ lastPromoted = NoAttachment

BrokerInit ==
  /\ brokerResizeActive = FALSE
  /\ brokerActiveGeometry = 0
  /\ brokerResizePending = FALSE
  /\ brokerPendingGeometry = 0
  /\ brokerMeasurementsOpen = TRUE
  /\ brokerMeasurementSeen = FALSE
  /\ brokerLatestGeometry = 0
  /\ brokerCleanupRequested = FALSE
  /\ brokerLastAction = "init"
  /\ brokerPreviousActive = brokerResizeActive
  /\ brokerPreviousActiveGeometry = brokerActiveGeometry
  /\ brokerPreviousPending = brokerResizePending
  /\ brokerPreviousPendingGeometry = brokerPendingGeometry

Init ==
  /\ LeaseInit
  /\ BrokerInit

BeginOpenBase(attachment) ==
  /\ operationKind = "idle"
  /\ attachment \in Attachments \ attached
  /\ admissionRank[attachment] = 0
  /\ operationKind' = "open"
  /\ operationAttachment' = attachment
  /\ UNCHANGED <<
       attached, owner, admissionRank, nextRank, releaseSuccessor,
       canonicalGeometry, canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision,
       lastPromoted
     >>
  /\ Record("begin-open", attachment)

CommitOpenBase ==
  /\ operationKind = "open"
  /\ LET attachment == operationAttachment
     IN
       /\ attached' = attached \cup {attachment}
       /\ admissionRank' = [admissionRank EXCEPT ![attachment] = nextRank]
       /\ nextRank' = nextRank + 1
       /\ owner' = IF owner = NoAttachment THEN attachment ELSE owner
       /\ lastPromoted' = IF owner = NoAttachment THEN attachment ELSE NoAttachment
  /\ operationKind' = "idle"
  /\ operationAttachment' = NoAttachment
  /\ releaseSuccessor' = NoAttachment
  /\ UNCHANGED <<canonicalGeometry, canonicalRevision, observedGeometry,
       observedRevision, resizeObservationPending, resizeRequestGeometry,
       resizeRevision>>
  /\ Record("commit-open", operationAttachment)

BeginReleaseBase(attachment) ==
  /\ operationKind = "idle"
  /\ attachment \in attached
  /\ operationKind' = "release"
  /\ operationAttachment' = attachment
  /\ releaseSuccessor' =
       IF attachment = owner /\ attached \ {attachment} # {}
       THEN Oldest(attached \ {attachment})
       ELSE NoAttachment
  /\ UNCHANGED <<
       attached, owner, admissionRank, nextRank, canonicalGeometry,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision,
       lastPromoted
     >>
  /\ Record("begin-release", attachment)

CommitReleaseBase ==
  /\ operationKind = "release"
  /\ LET attachment == operationAttachment
     IN
       /\ attached' = attached \ {attachment}
       /\ owner' = IF attachment = owner THEN releaseSuccessor ELSE owner
       /\ lastPromoted' = IF attachment = owner THEN releaseSuccessor ELSE NoAttachment
  /\ operationKind' = "idle"
  /\ operationAttachment' = NoAttachment
  /\ releaseSuccessor' = NoAttachment
  /\ UNCHANGED <<admissionRank, nextRank, canonicalGeometry,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision>>
  /\ Record("commit-release", operationAttachment)

AttemptResizeBase(attachment) ==
  /\ operationKind = "idle"
  /\ ~resizeObservationPending
  /\ canonicalRevision < MaxRevision
  /\ attachment \in attached
  /\ IF attachment = owner
       THEN /\ canonicalGeometry' = 1 - canonicalGeometry
            /\ canonicalRevision' = canonicalRevision + 1
            /\ resizeObservationPending' = TRUE
            /\ resizeRequestGeometry' = 1 - canonicalGeometry
            /\ resizeRevision' = canonicalRevision + 1
       ELSE /\ UNCHANGED <<canonicalGeometry, canonicalRevision,
                 resizeObservationPending, resizeRequestGeometry,
                 resizeRevision>>
  /\ UNCHANGED <<
       attached, owner, admissionRank, nextRank, operationKind,
       operationAttachment, releaseSuccessor, observedGeometry,
       observedRevision, lastPromoted
     >>
  /\ Record("resize", attachment)

AdvanceRuntimeDuringResizeBase ==
  /\ operationKind = "idle"
  /\ resizeObservationPending
  /\ canonicalRevision < MaxRevision
  /\ canonicalRevision' = canonicalRevision + 1
  /\ canonicalGeometry' \in 0..1
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, operationKind,
       operationAttachment, releaseSuccessor, observedGeometry,
       observedRevision, resizeObservationPending, resizeRequestGeometry,
       resizeRevision, lastPromoted>>
  /\ Record("runtime-advance", owner)

SynchronizeResizeObservationBase ==
  /\ operationKind = "idle"
  /\ resizeObservationPending
  /\ observedGeometry' = canonicalGeometry
  /\ observedRevision' = canonicalRevision
  /\ resizeObservationPending' = FALSE
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, operationKind,
       operationAttachment, releaseSuccessor, canonicalGeometry,
       canonicalRevision, resizeRequestGeometry, resizeRevision,
       lastPromoted>>
  /\ Record("synchronize-resize", owner)

BeginOpen(attachment) ==
  /\ Scenario = "lease"
  /\ BeginOpenBase(attachment)
  /\ UNCHANGED BrokerVars

CommitOpen ==
  /\ Scenario = "lease"
  /\ CommitOpenBase
  /\ UNCHANGED BrokerVars

BeginRelease(attachment) ==
  /\ Scenario = "lease"
  /\ BeginReleaseBase(attachment)
  /\ UNCHANGED BrokerVars

CommitRelease ==
  /\ Scenario = "lease"
  /\ CommitReleaseBase
  /\ UNCHANGED BrokerVars

AttemptResize(attachment) ==
  /\ Scenario = "lease"
  /\ AttemptResizeBase(attachment)
  /\ UNCHANGED BrokerVars

AdvanceRuntimeDuringResize ==
  /\ Scenario = "lease"
  /\ AdvanceRuntimeDuringResizeBase
  /\ UNCHANGED BrokerVars

SynchronizeResizeObservation ==
  /\ Scenario = "lease"
  /\ SynchronizeResizeObservationBase
  /\ UNCHANGED BrokerVars

MeasureBrokerResize(geometry) ==
  /\ Scenario = "broker-serialization"
  /\ geometry \in 0..1
  /\ brokerMeasurementsOpen
  /\ ~brokerCleanupRequested
  /\ brokerLatestGeometry' = geometry
  /\ brokerMeasurementSeen' = TRUE
  /\ IF brokerResizeActive
       THEN /\ brokerResizeActive' = TRUE
            /\ brokerActiveGeometry' = brokerActiveGeometry
            /\ brokerResizePending' = TRUE
            /\ brokerPendingGeometry' = geometry
       ELSE /\ brokerResizeActive' = TRUE
            /\ brokerActiveGeometry' = geometry
            /\ brokerResizePending' = FALSE
            /\ brokerPendingGeometry' = 0
  /\ UNCHANGED <<brokerMeasurementsOpen, brokerCleanupRequested>>
  /\ BrokerRecord("measure-broker-resize")
  /\ UNCHANGED LeaseVars

SettleBrokerResize ==
  /\ Scenario = "broker-serialization"
  /\ brokerResizeActive
  /\ brokerResizeActive' = brokerResizePending
  /\ brokerActiveGeometry' =
       IF brokerResizePending THEN brokerPendingGeometry ELSE 0
  /\ brokerResizePending' = FALSE
  /\ brokerPendingGeometry' = 0
  /\ UNCHANGED <<brokerMeasurementsOpen, brokerMeasurementSeen,
       brokerLatestGeometry, brokerCleanupRequested>>
  /\ BrokerRecord("settle-broker-resize")
  /\ UNCHANGED LeaseVars

QuiesceBrokerResize ==
  /\ Scenario = "broker-serialization"
  /\ brokerMeasurementsOpen
  /\ brokerMeasurementsOpen' = FALSE
  /\ UNCHANGED <<brokerResizeActive, brokerActiveGeometry,
       brokerResizePending, brokerPendingGeometry, brokerMeasurementSeen,
       brokerLatestGeometry, brokerCleanupRequested>>
  /\ BrokerRecord("quiesce-broker-resize")
  /\ UNCHANGED LeaseVars

CleanupBrokerResize ==
  /\ Scenario = "broker-serialization"
  /\ ~brokerCleanupRequested
  /\ brokerCleanupRequested' = TRUE
  /\ brokerMeasurementsOpen' = FALSE
  /\ brokerResizePending' = FALSE
  /\ brokerPendingGeometry' = 0
  /\ UNCHANGED <<brokerResizeActive, brokerActiveGeometry,
       brokerMeasurementSeen, brokerLatestGeometry>>
  /\ BrokerRecord("cleanup-broker-resize")
  /\ UNCHANGED LeaseVars

Next ==
  \/ \E attachment \in Attachments: BeginOpen(attachment)
  \/ CommitOpen
  \/ \E attachment \in Attachments: BeginRelease(attachment)
  \/ CommitRelease
  \/ \E attachment \in Attachments: AttemptResize(attachment)
  \/ AdvanceRuntimeDuringResize
  \/ SynchronizeResizeObservation
  \/ \E geometry \in 0..1: MeasureBrokerResize(geometry)
  \/ SettleBrokerResize
  \/ QuiesceBrokerResize
  \/ CleanupBrokerResize

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(CommitOpen)
  /\ WF_vars(CommitRelease)
  /\ WF_vars(SynchronizeResizeObservation)
  /\ WF_vars(SettleBrokerResize)

TypeOK ==
  /\ Scenario \in ResizeScenarios
  /\ NoAttachment \notin Attachments
  /\ attached \subseteq Attachments
  /\ owner \in attached \cup {NoAttachment}
  /\ admissionRank \in [Attachments -> 0..Cardinality(Attachments)]
  /\ nextRank \in 1..(Cardinality(Attachments) + 1)
  /\ operationKind \in OperationKinds
  /\ operationAttachment \in Attachments \cup {NoAttachment}
  /\ releaseSuccessor \in Attachments \cup {NoAttachment}
  /\ canonicalGeometry \in 0..1
  /\ canonicalRevision \in 0..MaxRevision
  /\ observedGeometry \in 0..1
  /\ observedRevision \in 0..MaxRevision
  /\ resizeObservationPending \in BOOLEAN
  /\ resizeRequestGeometry \in 0..1
  /\ resizeRevision \in 0..MaxRevision
  /\ lastAction \in Actions
  /\ lastActor \in Attachments \cup {NoAttachment}
  /\ previousGeometry \in 0..1
  /\ previousOwner \in Attachments \cup {NoAttachment}
  /\ lastPromoted \in Attachments \cup {NoAttachment}
  /\ brokerResizeActive \in BOOLEAN
  /\ brokerActiveGeometry \in 0..1
  /\ brokerResizePending \in BOOLEAN
  /\ brokerPendingGeometry \in 0..1
  /\ brokerMeasurementsOpen \in BOOLEAN
  /\ brokerMeasurementSeen \in BOOLEAN
  /\ brokerLatestGeometry \in 0..1
  /\ brokerCleanupRequested \in BOOLEAN
  /\ brokerLastAction \in Actions
  /\ brokerPreviousActive \in BOOLEAN
  /\ brokerPreviousActiveGeometry \in 0..1
  /\ brokerPreviousPending \in BOOLEAN
  /\ brokerPreviousPendingGeometry \in 0..1

AdmissionRanksAreUnique ==
  \A left, right \in Attachments:
    /\ admissionRank[left] # 0
    /\ admissionRank[right] # 0
    /\ admissionRank[left] = admissionRank[right]
    => left = right

AttachedAttachmentsHaveAdmissionRank ==
  \A attachment \in Attachments:
    attachment \in attached => admissionRank[attachment] # 0

IdleAttachmentsHaveOneOwner ==
  operationKind = "idle" => ((attached = {}) = (owner = NoAttachment))

OnlyOwnerChangesCanonicalGeometry ==
  lastAction = "resize" /\ lastActor # previousOwner =>
    canonicalGeometry = previousGeometry

ReleasePromotesRecordedSuccessor ==
  lastAction = "commit-release" => owner = lastPromoted \/ owner = previousOwner

OneOperationAtATime ==
  operationKind = "idle" => operationAttachment = NoAttachment

ObservedRevisionNeverLeadsRuntime == observedRevision <= canonicalRevision

SynchronizedResizeDoesNotRegress ==
  lastAction = "synchronize-resize" => observedRevision >= resizeRevision

ExactResizeObservationMatchesRequest ==
  /\ lastAction = "synchronize-resize"
  /\ observedRevision = resizeRevision
  => observedGeometry = resizeRequestGeometry

NewestRuntimeObservationWins ==
  lastAction = "synchronize-resize" =>
    /\ observedRevision = canonicalRevision
    /\ observedGeometry = canonicalGeometry

BrokerInactiveGeometryIsEmpty ==
  /\ ~brokerResizeActive => brokerActiveGeometry = 0
  /\ ~brokerResizePending => brokerPendingGeometry = 0

BrokerPendingIsNewest ==
  brokerResizePending => brokerPendingGeometry = brokerLatestGeometry

BrokerActiveIsNewestWithoutPending ==
  /\ brokerMeasurementSeen
  /\ ~brokerCleanupRequested
  /\ brokerResizeActive
  /\ ~brokerResizePending
  => brokerActiveGeometry = brokerLatestGeometry

BrokerMeasurementDoesNotReplaceActive ==
  /\ brokerLastAction = "measure-broker-resize"
  /\ brokerPreviousActive
  => /\ brokerResizeActive
     /\ brokerActiveGeometry = brokerPreviousActiveGeometry

BrokerSettlementPromotesLatestPending ==
  /\ brokerLastAction = "settle-broker-resize"
  /\ brokerPreviousPending
  => /\ brokerResizeActive
     /\ brokerActiveGeometry = brokerPreviousPendingGeometry
     /\ ~brokerResizePending

BrokerCleanupDropsPending ==
  brokerCleanupRequested =>
    /\ ~brokerMeasurementsOpen
    /\ ~brokerResizePending

PendingOperationEventuallyCommits ==
  operationKind # "idle" ~> operationKind = "idle"

PendingResizeEventuallySynchronizes ==
  resizeObservationPending ~> ~resizeObservationPending

BrokerQuiescenceEventuallyDrains ==
  ~brokerMeasurementsOpen ~> (~brokerResizeActive /\ ~brokerResizePending)

=============================================================================
