---------------- MODULE DesktopTerminalResizeLease ----------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS Attachments, NoAttachment, MaxRevision, MaxRank, Scenario

ResizeScenarios == {
  "lease",
  "handoff",
  "eligibility",
  "grant-convergence",
  "broker-serialization"
}

Participants ==
  IF Scenario \in {"eligibility", "grant-convergence"}
  THEN CHOOSE candidates \in SUBSET Attachments: Cardinality(candidates) = 2
  ELSE Attachments

OperationKinds == {"idle", "open", "release", "claim"}
Actions == {
  "init",
  "begin-open",
  "commit-open",
  "begin-release",
  "commit-release",
  "record-activity",
  "quiesce-activity",
  "begin-claim",
  "commit-claim",
  "claim-current",
  "reject-claim-overflow",
  "revoke-input",
  "stop-terminal",
  "reject-ineligible-activity",
  "reject-in-flight-claim",
  "resize",
  "runtime-advance",
  "synchronize-resize",
  "observe-canonical",
  "submit-granted-geometry",
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
  claimRank,
  nextClaimRank,
  activityRank,
  nextActivityRank,
  desiredOwner,
  activitiesOpen,
  claimRejected,
  lastActivity,
  lastClaimed,
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
  previousDesiredOwner,
  lastPromoted,
  inputEligible,
  resizeEligible,
  terminalRunning,
  measuredGeometry,
  projectedGeometry,
  grantSubmissionPending,
  grantAttachment,
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
  claimRank,
  nextClaimRank,
  activityRank,
  nextActivityRank,
  desiredOwner,
  activitiesOpen,
  claimRejected,
  lastActivity,
  lastClaimed,
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
  previousDesiredOwner,
  lastPromoted
>>

PolicyVars == <<
  inputEligible,
  resizeEligible,
  terminalRunning,
  measuredGeometry,
  projectedGeometry,
  grantSubmissionPending,
  grantAttachment
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

vars == <<LeaseVars, PolicyVars, BrokerVars>>

Oldest(candidates) ==
  CHOOSE candidate \in candidates:
    \A other \in candidates: admissionRank[candidate] <= admissionRank[other]

MostRecentBy(rank, candidates) ==
  CHOOSE candidate \in candidates:
    \A other \in candidates:
      \/ rank[candidate] > rank[other]
      \/ /\ rank[candidate] = rank[other]
         /\ admissionRank[candidate] <= admissionRank[other]

MostRecentClaimed(candidates) == MostRecentBy(claimRank, candidates)

PreferredRemaining(candidates) ==
  LET active == {attachment \in candidates: activityRank[attachment] > 0}
  IN IF active # {} THEN MostRecentBy(activityRank, active)
     ELSE MostRecentClaimed(candidates)

ClaimEligible(attachment) ==
  /\ attachment \in attached
  /\ inputEligible[attachment]
  /\ resizeEligible[attachment]
  /\ terminalRunning

EligibleRemaining(candidates) ==
  {attachment \in candidates:
    /\ inputEligible[attachment]
    /\ resizeEligible[attachment]
    /\ terminalRunning}

PreferredEligibleRemaining(candidates) ==
  LET eligible == EligibleRemaining(candidates)
      active == {attachment \in eligible: activityRank[attachment] > 0}
  IN IF eligible = {} THEN NoAttachment
     ELSE IF active # {} THEN MostRecentBy(activityRank, active)
     ELSE MostRecentClaimed(eligible)

Record(action, actor) ==
  /\ lastAction' = action
  /\ lastActor' = actor
  /\ previousGeometry' = canonicalGeometry
  /\ previousOwner' = owner
  /\ previousDesiredOwner' = desiredOwner

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
  /\ claimRank = [attachment \in Attachments |-> 0]
  /\ nextClaimRank = 1
  /\ activityRank = [attachment \in Attachments |-> 0]
  /\ nextActivityRank = 1
  /\ desiredOwner = NoAttachment
  /\ activitiesOpen = TRUE
  /\ claimRejected = FALSE
  /\ lastActivity = NoAttachment
  /\ lastClaimed = NoAttachment
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
  /\ previousDesiredOwner = desiredOwner
  /\ lastPromoted = NoAttachment

PolicyInit ==
  /\ inputEligible = [attachment \in Attachments |-> FALSE]
  /\ resizeEligible = [attachment \in Attachments |-> FALSE]
  /\ terminalRunning = TRUE
  /\ measuredGeometry \in
       IF Scenario = "grant-convergence"
       THEN [Attachments -> 0..1]
       ELSE {[attachment \in Attachments |-> 0]}
  /\ projectedGeometry = measuredGeometry
  /\ grantSubmissionPending = FALSE
  /\ grantAttachment = NoAttachment

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
  /\ PolicyInit
  /\ BrokerInit

BeginOpenBase(attachment) ==
  /\ operationKind = "idle"
  /\ attachment \in Attachments \ attached
  /\ admissionRank[attachment] = 0
  /\ owner # NoAttachment \/ nextClaimRank <= MaxRank
  /\ operationKind' = "open"
  /\ operationAttachment' = attachment
  /\ UNCHANGED <<
       attached, owner, admissionRank, nextRank, claimRank, nextClaimRank,
       activityRank, nextActivityRank, desiredOwner, activitiesOpen,
       claimRejected, lastActivity, lastClaimed, releaseSuccessor,
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
       /\ claimRank' =
            IF owner = NoAttachment
            THEN [claimRank EXCEPT ![attachment] = nextClaimRank]
            ELSE claimRank
       /\ nextClaimRank' =
            IF owner = NoAttachment THEN nextClaimRank + 1 ELSE nextClaimRank
       /\ desiredOwner' =
            IF owner = NoAttachment THEN attachment ELSE desiredOwner
       /\ lastClaimed' =
            IF owner = NoAttachment THEN attachment ELSE lastClaimed
       /\ lastPromoted' = IF owner = NoAttachment THEN attachment ELSE NoAttachment
  /\ operationKind' = "idle"
  /\ operationAttachment' = NoAttachment
  /\ releaseSuccessor' = NoAttachment
  /\ UNCHANGED <<activityRank, nextActivityRank, activitiesOpen,
       claimRejected, lastActivity, canonicalGeometry, canonicalRevision, observedGeometry,
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
       THEN MostRecentClaimed(attached \ {attachment})
       ELSE NoAttachment
  /\ UNCHANGED <<
       attached, owner, admissionRank, nextRank, claimRank, nextClaimRank,
       activityRank, nextActivityRank, desiredOwner, activitiesOpen,
       claimRejected, lastActivity, lastClaimed, canonicalGeometry,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision,
       lastPromoted
     >>
  /\ Record("begin-release", attachment)

CommitReleaseBase ==
  /\ operationKind = "release"
  /\ LET attachment == operationAttachment
         remaining == attached \ {attachment}
     IN
       /\ attached' = attached \ {attachment}
       /\ owner' = IF attachment = owner THEN releaseSuccessor ELSE owner
       /\ desiredOwner' =
            IF remaining = {} THEN NoAttachment
            ELSE IF desiredOwner \in remaining
                 THEN IF inputEligible[desiredOwner]
                           /\ resizeEligible[desiredOwner]
                           /\ terminalRunning
                      THEN desiredOwner
                      ELSE PreferredEligibleRemaining(remaining)
                 ELSE PreferredEligibleRemaining(remaining)
       /\ lastPromoted' = IF attachment = owner THEN releaseSuccessor ELSE NoAttachment
  /\ operationKind' = "idle"
  /\ operationAttachment' = NoAttachment
  /\ releaseSuccessor' = NoAttachment
  /\ UNCHANGED <<admissionRank, nextRank, claimRank, nextClaimRank,
       activityRank, nextActivityRank, activitiesOpen, claimRejected,
       lastActivity, lastClaimed, canonicalGeometry,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision>>
  /\ Record("commit-release", operationAttachment)

RecordActivityBase(attachment) ==
  /\ activitiesOpen
  /\ attachment \in attached
  /\ nextActivityRank <= MaxRank
  /\ activityRank' = [activityRank EXCEPT ![attachment] = nextActivityRank]
  /\ nextActivityRank' = nextActivityRank + 1
  /\ desiredOwner' = attachment
  /\ claimRejected' = FALSE
  /\ lastActivity' = attachment
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activitiesOpen, lastClaimed, operationKind,
       operationAttachment, releaseSuccessor, canonicalGeometry,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision,
       lastPromoted>>
  /\ Record("record-activity", attachment)

QuiesceActivityBase ==
  /\ activitiesOpen
  /\ activitiesOpen' = FALSE
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       claimRejected, lastActivity, lastClaimed, operationKind,
       operationAttachment, releaseSuccessor, canonicalGeometry,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision,
       lastPromoted>>
  /\ Record("quiesce-activity", NoAttachment)

BeginClaimBase ==
  /\ operationKind = "idle"
  /\ desiredOwner \in attached
  /\ desiredOwner # owner
  /\ nextClaimRank <= MaxRank
  /\ operationKind' = "claim"
  /\ operationAttachment' = desiredOwner
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, claimRejected, lastActivity, lastClaimed,
       releaseSuccessor, canonicalGeometry, canonicalRevision,
       observedGeometry, observedRevision, resizeObservationPending,
       resizeRequestGeometry, resizeRevision, lastPromoted>>
  /\ Record("begin-claim", desiredOwner)

CommitClaimBase ==
  /\ operationKind = "claim"
  /\ LET attachment == operationAttachment
     IN
       /\ owner' = attachment
       /\ claimRank' = [claimRank EXCEPT ![attachment] = nextClaimRank]
       /\ nextClaimRank' = nextClaimRank + 1
       /\ lastClaimed' = attachment
  /\ operationKind' = "idle"
  /\ operationAttachment' = NoAttachment
  /\ lastPromoted' = NoAttachment
  /\ UNCHANGED <<attached, admissionRank, nextRank, activityRank,
       nextActivityRank, desiredOwner, activitiesOpen, claimRejected,
       lastActivity, releaseSuccessor, canonicalGeometry, canonicalRevision,
       observedGeometry, observedRevision, resizeObservationPending,
       resizeRequestGeometry, resizeRevision>>
  /\ Record("commit-claim", operationAttachment)

ClaimCurrentOwnerBase(attachment) ==
  /\ operationKind = "idle"
  /\ attachment = owner
  /\ attachment \in attached
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, claimRejected, lastActivity, lastClaimed,
       operationKind, operationAttachment, releaseSuccessor,
       canonicalGeometry, canonicalRevision, observedGeometry,
       observedRevision, resizeObservationPending, resizeRequestGeometry,
       resizeRevision, lastPromoted>>
  /\ Record("claim-current", attachment)

RejectClaimOverflowBase ==
  /\ operationKind = "idle"
  /\ desiredOwner \in attached
  /\ desiredOwner # owner
  /\ nextClaimRank > MaxRank
  /\ claimRejected' = TRUE
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner, activitiesOpen,
       lastActivity, lastClaimed, operationKind, operationAttachment,
       releaseSuccessor, canonicalGeometry, canonicalRevision,
       observedGeometry, observedRevision, resizeObservationPending,
       resizeRequestGeometry, resizeRevision, lastPromoted>>
  /\ Record("reject-claim-overflow", desiredOwner)

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
       attached, owner, admissionRank, nextRank, claimRank, nextClaimRank,
       activityRank, nextActivityRank, desiredOwner, activitiesOpen,
       claimRejected, lastActivity, lastClaimed, operationKind,
       operationAttachment, releaseSuccessor, observedGeometry,
       observedRevision, lastPromoted
     >>
  /\ Record("resize", attachment)

AttemptHandoffResizeBase(attachment) ==
  /\ operationKind = "idle"
  /\ attachment \in attached
  /\ IF attachment = owner
       THEN canonicalGeometry' = 1 - canonicalGeometry
       ELSE UNCHANGED canonicalGeometry
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, claimRejected, lastActivity, lastClaimed,
       operationKind, operationAttachment, releaseSuccessor,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision,
       lastPromoted>>
  /\ Record("resize", attachment)

AdvanceRuntimeDuringResizeBase ==
  /\ operationKind = "idle"
  /\ resizeObservationPending
  /\ canonicalRevision < MaxRevision
  /\ canonicalRevision' = canonicalRevision + 1
  /\ canonicalGeometry' \in 0..1
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, claimRejected, lastActivity, lastClaimed, operationKind,
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
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, claimRejected, lastActivity, lastClaimed, operationKind,
       operationAttachment, releaseSuccessor, canonicalGeometry,
       canonicalRevision, resizeRequestGeometry, resizeRevision,
       lastPromoted>>
  /\ Record("synchronize-resize", owner)

BeginOpen(attachment) ==
  /\ Scenario \in {"lease", "handoff", "eligibility", "grant-convergence"}
  /\ attachment \in Participants
  /\ terminalRunning
  /\ BeginOpenBase(attachment)
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

CommitOpen ==
  /\ Scenario \in {"lease", "handoff", "eligibility", "grant-convergence"}
  /\ terminalRunning
  /\ CommitOpenBase
  /\ inputEligible' = [inputEligible EXCEPT ![operationAttachment] = TRUE]
  /\ resizeEligible' = [resizeEligible EXCEPT ![operationAttachment] = TRUE]
  /\ UNCHANGED <<terminalRunning, measuredGeometry, projectedGeometry,
       grantSubmissionPending, grantAttachment>>
  /\ UNCHANGED BrokerVars

BeginRelease(attachment) ==
  /\ Scenario \in {"lease", "handoff", "eligibility", "grant-convergence"}
  /\ BeginReleaseBase(attachment)
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

CommitRelease ==
  /\ Scenario \in {"lease", "handoff", "eligibility", "grant-convergence"}
  /\ CommitReleaseBase
  /\ inputEligible' = [inputEligible EXCEPT ![operationAttachment] = FALSE]
  /\ resizeEligible' = [resizeEligible EXCEPT ![operationAttachment] = FALSE]
  /\ grantSubmissionPending' =
       grantSubmissionPending /\ grantAttachment # operationAttachment
  /\ grantAttachment' =
       IF grantAttachment = operationAttachment THEN NoAttachment ELSE grantAttachment
  /\ UNCHANGED <<terminalRunning, measuredGeometry, projectedGeometry>>
  /\ UNCHANGED BrokerVars

RecordActivity(attachment) ==
  /\ Scenario \in {"handoff", "eligibility", "grant-convergence"}
  /\ ClaimEligible(attachment)
  /\ RecordActivityBase(attachment)
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

QuiesceActivity ==
  /\ Scenario \in {"handoff", "eligibility", "grant-convergence"}
  /\ QuiesceActivityBase
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

BeginClaim ==
  /\ Scenario \in {"handoff", "eligibility", "grant-convergence"}
  /\ ClaimEligible(desiredOwner)
  /\ BeginClaimBase
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

CommitClaim ==
  /\ Scenario \in {"handoff", "eligibility", "grant-convergence"}
  /\ ClaimEligible(operationAttachment)
  /\ CommitClaimBase
  /\ IF Scenario = "grant-convergence"
       THEN /\ grantSubmissionPending' = TRUE
            /\ grantAttachment' = operationAttachment
       ELSE /\ UNCHANGED <<grantSubmissionPending, grantAttachment>>
  /\ UNCHANGED <<inputEligible, resizeEligible, terminalRunning,
       measuredGeometry, projectedGeometry>>
  /\ UNCHANGED BrokerVars

ClaimCurrentOwner(attachment) ==
  /\ Scenario \in {"handoff", "eligibility", "grant-convergence"}
  /\ ClaimEligible(attachment)
  /\ ClaimCurrentOwnerBase(attachment)
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

RejectClaimOverflow ==
  /\ Scenario \in {"handoff", "eligibility", "grant-convergence"}
  /\ ClaimEligible(desiredOwner)
  /\ RejectClaimOverflowBase
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

AttemptResize(attachment) ==
  /\ \/ /\ Scenario = "lease"
         /\ AttemptResizeBase(attachment)
     \/ /\ Scenario \in {"handoff", "eligibility", "grant-convergence"}
         /\ AttemptHandoffResizeBase(attachment)
  /\ terminalRunning
  /\ resizeEligible[attachment]
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

AdvanceRuntimeDuringResize ==
  /\ Scenario = "lease"
  /\ AdvanceRuntimeDuringResizeBase
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

SynchronizeResizeObservation ==
  /\ Scenario = "lease"
  /\ SynchronizeResizeObservationBase
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

RejectIneligibleActivity(attachment) ==
  /\ Scenario = "eligibility"
  /\ activitiesOpen
  /\ attachment \in attached
  /\ ~ClaimEligible(attachment)
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, claimRejected, lastActivity, lastClaimed,
       operationKind, operationAttachment, releaseSuccessor,
       canonicalGeometry, canonicalRevision, observedGeometry,
       observedRevision, resizeObservationPending, resizeRequestGeometry,
       resizeRevision, lastPromoted>>
  /\ Record("reject-ineligible-activity", attachment)
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

RevokeInput(attachment) ==
  /\ Scenario = "eligibility"
  /\ operationKind \in {"idle", "claim"}
  /\ attachment \in attached
  /\ inputEligible[attachment]
  /\ inputEligible' = [inputEligible EXCEPT ![attachment] = FALSE]
  /\ desiredOwner' =
       IF attachment = desiredOwner
       THEN PreferredEligibleRemaining(attached \ {attachment})
       ELSE desiredOwner
  /\ claimRejected' =
       (claimRejected \/ (attachment = desiredOwner /\ attachment # owner))
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank,
       activitiesOpen, lastActivity, lastClaimed, operationKind,
       operationAttachment, releaseSuccessor, canonicalGeometry,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision,
       lastPromoted>>
  /\ Record("revoke-input", attachment)
  /\ UNCHANGED <<resizeEligible, terminalRunning, measuredGeometry,
       projectedGeometry, grantSubmissionPending, grantAttachment>>
  /\ UNCHANGED BrokerVars

StopTerminal ==
  /\ Scenario = "eligibility"
  /\ operationKind \in {"idle", "claim"}
  /\ terminalRunning
  /\ terminalRunning' = FALSE
  /\ claimRejected' = (claimRejected \/ desiredOwner # owner)
  /\ grantSubmissionPending' = FALSE
  /\ grantAttachment' = NoAttachment
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, lastActivity, lastClaimed, operationKind,
       operationAttachment, releaseSuccessor, canonicalGeometry,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision,
       lastPromoted>>
  /\ Record("stop-terminal", NoAttachment)
  /\ UNCHANGED <<inputEligible, resizeEligible, measuredGeometry,
       projectedGeometry>>
  /\ UNCHANGED BrokerVars

RejectInFlightClaim ==
  /\ Scenario = "eligibility"
  /\ operationKind = "claim"
  /\ ~ClaimEligible(operationAttachment)
  /\ operationKind' = "idle"
  /\ operationAttachment' = NoAttachment
  /\ claimRejected' = TRUE
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, lastActivity, lastClaimed, releaseSuccessor,
       canonicalGeometry, canonicalRevision, observedGeometry,
       observedRevision, resizeObservationPending, resizeRequestGeometry,
       resizeRevision, lastPromoted>>
  /\ Record("reject-in-flight-claim", operationAttachment)
  /\ UNCHANGED PolicyVars
  /\ UNCHANGED BrokerVars

ObserveCanonical(attachment) ==
  /\ Scenario = "grant-convergence"
  /\ attachment \in attached
  /\ projectedGeometry' =
       [projectedGeometry EXCEPT ![attachment] = canonicalGeometry]
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, claimRejected, lastActivity, lastClaimed,
       operationKind, operationAttachment, releaseSuccessor,
       canonicalGeometry, canonicalRevision, observedGeometry,
       observedRevision, resizeObservationPending, resizeRequestGeometry,
       resizeRevision, lastPromoted>>
  /\ Record("observe-canonical", attachment)
  /\ UNCHANGED <<inputEligible, resizeEligible, terminalRunning,
       measuredGeometry, grantSubmissionPending, grantAttachment>>
  /\ UNCHANGED BrokerVars

SubmitGrantedGeometry ==
  /\ Scenario = "grant-convergence"
  /\ grantSubmissionPending
  /\ grantAttachment = owner
  /\ ClaimEligible(grantAttachment)
  /\ canonicalGeometry' = measuredGeometry[grantAttachment]
  /\ grantSubmissionPending' = FALSE
  /\ grantAttachment' = NoAttachment
  /\ UNCHANGED <<attached, owner, admissionRank, nextRank, claimRank,
       nextClaimRank, activityRank, nextActivityRank, desiredOwner,
       activitiesOpen, claimRejected, lastActivity, lastClaimed,
       operationKind, operationAttachment, releaseSuccessor,
       canonicalRevision, observedGeometry, observedRevision,
       resizeObservationPending, resizeRequestGeometry, resizeRevision,
       lastPromoted>>
  /\ Record("submit-granted-geometry", grantAttachment)
  /\ UNCHANGED <<inputEligible, resizeEligible, terminalRunning,
       measuredGeometry, projectedGeometry>>
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
  /\ UNCHANGED PolicyVars

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
  /\ UNCHANGED PolicyVars

QuiesceBrokerResize ==
  /\ Scenario = "broker-serialization"
  /\ brokerMeasurementsOpen
  /\ brokerMeasurementsOpen' = FALSE
  /\ UNCHANGED <<brokerResizeActive, brokerActiveGeometry,
       brokerResizePending, brokerPendingGeometry, brokerMeasurementSeen,
       brokerLatestGeometry, brokerCleanupRequested>>
  /\ BrokerRecord("quiesce-broker-resize")
  /\ UNCHANGED LeaseVars
  /\ UNCHANGED PolicyVars

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
  /\ UNCHANGED PolicyVars

Next ==
  \/ \E attachment \in Attachments: BeginOpen(attachment)
  \/ CommitOpen
  \/ \E attachment \in Attachments: BeginRelease(attachment)
  \/ CommitRelease
  \/ \E attachment \in Attachments: RecordActivity(attachment)
  \/ QuiesceActivity
  \/ BeginClaim
  \/ CommitClaim
  \/ \E attachment \in Attachments: ClaimCurrentOwner(attachment)
  \/ RejectClaimOverflow
  \/ \E attachment \in Attachments: RejectIneligibleActivity(attachment)
  \/ \E attachment \in Attachments: RevokeInput(attachment)
  \/ StopTerminal
  \/ RejectInFlightClaim
  \/ \E attachment \in Attachments: AttemptResize(attachment)
  \/ AdvanceRuntimeDuringResize
  \/ SynchronizeResizeObservation
  \/ \E attachment \in Attachments: ObserveCanonical(attachment)
  \/ SubmitGrantedGeometry
  \/ \E geometry \in 0..1: MeasureBrokerResize(geometry)
  \/ SettleBrokerResize
  \/ QuiesceBrokerResize
  \/ CleanupBrokerResize

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(CommitOpen)
  /\ WF_vars(CommitRelease)
  /\ WF_vars(BeginClaim)
  /\ WF_vars(CommitClaim)
  /\ WF_vars(RejectClaimOverflow)
  /\ WF_vars(RejectInFlightClaim)
  /\ WF_vars(SynchronizeResizeObservation)
  /\ WF_vars(SubmitGrantedGeometry)
  /\ WF_vars(SettleBrokerResize)

TypeOK ==
  /\ Scenario \in ResizeScenarios
  /\ NoAttachment \notin Attachments
  /\ attached \subseteq Attachments
  /\ owner \in attached \cup {NoAttachment}
  /\ admissionRank \in [Attachments -> 0..Cardinality(Attachments)]
  /\ nextRank \in 1..(Cardinality(Attachments) + 1)
  /\ claimRank \in [Attachments -> 0..MaxRank]
  /\ nextClaimRank \in 1..(MaxRank + 1)
  /\ activityRank \in [Attachments -> 0..MaxRank]
  /\ nextActivityRank \in 1..(MaxRank + 1)
  /\ desiredOwner \in attached \cup {NoAttachment}
  /\ activitiesOpen \in BOOLEAN
  /\ claimRejected \in BOOLEAN
  /\ lastActivity \in Attachments \cup {NoAttachment}
  /\ lastClaimed \in Attachments \cup {NoAttachment}
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
  /\ previousDesiredOwner \in Attachments \cup {NoAttachment}
  /\ lastPromoted \in Attachments \cup {NoAttachment}
  /\ inputEligible \in [Attachments -> BOOLEAN]
  /\ resizeEligible \in [Attachments -> BOOLEAN]
  /\ terminalRunning \in BOOLEAN
  /\ measuredGeometry \in [Attachments -> 0..1]
  /\ projectedGeometry \in [Attachments -> 0..1]
  /\ grantSubmissionPending \in BOOLEAN
  /\ grantAttachment \in Attachments \cup {NoAttachment}
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

DesiredOwnerIsAttached == desiredOwner \in attached \cup {NoAttachment}

EligibilityBelongsToAttached ==
  \A attachment \in Attachments:
    inputEligible[attachment] \/ resizeEligible[attachment] => attachment \in attached

ClaimRanksAreUnique ==
  \A left, right \in attached:
    /\ claimRank[left] # 0
    /\ claimRank[right] # 0
    /\ claimRank[left] = claimRank[right]
    => left = right

LatestActivityBecomesDesired ==
  lastAction = "record-activity" => desiredOwner = lastActivity

RecordedActivityRequiresEligibility ==
  lastAction = "record-activity" => ClaimEligible(lastActivity)

RejectedActivityPreservesDesired ==
  lastAction = "reject-ineligible-activity" => desiredOwner = previousDesiredOwner

OnlyLifecycleOrClaimChangesOwner ==
  owner # previousOwner =>
    lastAction \in {"commit-open", "commit-release", "commit-claim"}

ClaimCommitSelectsTarget ==
  lastAction = "commit-claim" => owner = lastClaimed

ClaimCommitRequiresEligibility ==
  lastAction = "commit-claim" => ClaimEligible(lastClaimed)

OverflowRejectionPreservesIntent ==
  lastAction = "reject-claim-overflow" => desiredOwner = lastActor

InFlightRejectionPreservesIntent ==
  lastAction = "reject-in-flight-claim" => desiredOwner = previousDesiredOwner

OnlyOwnerChangesCanonicalGeometry ==
  lastAction = "resize" /\ lastActor # previousOwner =>
    canonicalGeometry = previousGeometry

ReleasePromotesRecordedSuccessor ==
  lastAction = "commit-release" => owner = lastPromoted \/ owner = previousOwner

ReleasePrefersNewestClaim ==
  /\ lastAction = "commit-release"
  /\ lastPromoted # NoAttachment
  => \A other \in attached:
       \/ claimRank[lastPromoted] > claimRank[other]
       \/ /\ claimRank[lastPromoted] = claimRank[other]
          /\ admissionRank[lastPromoted] <= admissionRank[other]

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

GrantPendingBelongsToOwner ==
  grantSubmissionPending =>
    /\ grantAttachment = owner
    /\ grantAttachment \in attached

GrantedSubmissionUsesMeasuredGeometry ==
  lastAction = "submit-granted-geometry" =>
    canonicalGeometry = measuredGeometry[lastActor]

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
  Scenario = "lease" => (resizeObservationPending ~> ~resizeObservationPending)

QuiescedActivityEventuallySettles ==
  Scenario \in {"handoff", "eligibility", "grant-convergence"} =>
    \A attachment \in Attachments:
      ( /\ ~activitiesOpen
        /\ operationKind = "idle"
        /\ desiredOwner = attachment
        ~> \/ /\ operationKind = "idle"
               /\ owner = attachment
           \/ claimRejected
           \/ desiredOwner # attachment )

GrantedOwnerEventuallySubmits ==
  Scenario = "grant-convergence" =>
    (grantSubmissionPending ~> ~grantSubmissionPending)

EqualProjectedGrantEventuallySubmits ==
  Scenario = "grant-convergence" =>
    ( /\ grantSubmissionPending
      /\ measuredGeometry[grantAttachment] = projectedGeometry[grantAttachment]
      ~> ~grantSubmissionPending )

BrokerQuiescenceEventuallyDrains ==
  ~brokerMeasurementsOpen ~> (~brokerResizeActive /\ ~brokerResizePending)

=============================================================================
