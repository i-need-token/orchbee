---------------- MODULE DesktopTerminalResizeLease ----------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS Attachments, NoAttachment, MaxRevision

OperationKinds == {"idle", "open", "release"}
Actions == {
  "init",
  "begin-open",
  "commit-open",
  "begin-release",
  "commit-release",
  "resize",
  "runtime-advance",
  "synchronize-resize"
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
  lastPromoted

vars == <<
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

Oldest(candidates) ==
  CHOOSE candidate \in candidates:
    \A other \in candidates: admissionRank[candidate] <= admissionRank[other]

Record(action, actor) ==
  /\ lastAction' = action
  /\ lastActor' = actor
  /\ previousGeometry' = canonicalGeometry
  /\ previousOwner' = owner

Init ==
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

BeginOpen(attachment) ==
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

CommitOpen ==
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

BeginRelease(attachment) ==
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

CommitRelease ==
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

AttemptResize(attachment) ==
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

AdvanceRuntimeDuringResize ==
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

SynchronizeResizeObservation ==
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

Next ==
  \/ \E attachment \in Attachments: BeginOpen(attachment)
  \/ CommitOpen
  \/ \E attachment \in Attachments: BeginRelease(attachment)
  \/ CommitRelease
  \/ \E attachment \in Attachments: AttemptResize(attachment)
  \/ AdvanceRuntimeDuringResize
  \/ SynchronizeResizeObservation

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(CommitOpen)
  /\ WF_vars(CommitRelease)
  /\ WF_vars(SynchronizeResizeObservation)

TypeOK ==
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

PendingOperationEventuallyCommits ==
  operationKind # "idle" ~> operationKind = "idle"

PendingResizeEventuallySynchronizes ==
  resizeObservationPending ~> ~resizeObservationPending

=============================================================================
