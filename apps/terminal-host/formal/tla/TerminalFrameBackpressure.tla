---------------------- MODULE TerminalFrameBackpressure ----------------------
EXTENDS FiniteSets, Naturals, TLC

\* Refines runtime-model claims TH-006, TH-010, TH-011, TH-012, and TH-016.
\* Mutate, ResizeOrScreenSwitch, SetDemand, Capture, PrepareFrame,
\* AcquireReadCommit, Emit, PublishFailure, ConfirmContainment, FrameApplied,
\* NeedSnapshot, and Finalize model the canonical-state and delivery-flight
\* transitions named by the package contract. Source-scoped input admission and
\* reply priority are owned by TerminalInputBackpressure so independent queue
\* state does not multiply the two-attachment frame state. ResizeOrScreenSwitch
\* abstracts every generation-changing canonical mutation, including application
\* clear and reset: acknowledged bases are generation-scoped and every attachment
\* is forced to recover through a full snapshot.

CONSTANTS Attachments, MaxRevision, MaxGeneration, MaxSequence

None == MaxRevision + 1
NoGeneration == MaxGeneration + 1

VARIABLES
  revision,
  generation,
  finalized,
  captured,
  capturedRevision,
  capturedGeneration,
  demand,
  blocked,
  blockedRevision,
  acknowledgedRevision,
  acknowledgedGeneration,
  inFlight,
  frameBase,
  frameRevision,
  frameGeneration,
  frameSequence,
  nextSequence,
  lastAppliedSequence,
  lastAbandonedSequence,
  pendingDirty,
  forceSnapshot,
  provisional,
  commitGuard,
  failurePublished,
  containmentConfirmed,
  handoffDuringUnconfirmed

vars ==
  <<revision, generation, finalized, captured, capturedRevision,
    capturedGeneration, demand, blocked, blockedRevision, acknowledgedRevision,
    acknowledgedGeneration, inFlight, frameBase, frameRevision,
    frameGeneration, frameSequence, nextSequence, lastAppliedSequence,
    lastAbandonedSequence, pendingDirty, forceSnapshot, provisional,
    commitGuard, failurePublished, containmentConfirmed,
    handoffDuringUnconfirmed>>

AnyDemand(demands) == \E attachment \in Attachments: demands[attachment]
Readable == ~failurePublished \/ containmentConfirmed

Init ==
  /\ revision = 0
  /\ generation = 1
  /\ finalized = FALSE
  /\ captured = FALSE
  /\ capturedRevision = 0
  /\ capturedGeneration = 1
  /\ demand = [attachment \in Attachments |-> FALSE]
  /\ blocked = [attachment \in Attachments |-> FALSE]
  /\ blockedRevision = [attachment \in Attachments |-> None]
  /\ acknowledgedRevision = [attachment \in Attachments |-> None]
  /\ acknowledgedGeneration = [attachment \in Attachments |-> NoGeneration]
  /\ inFlight = [attachment \in Attachments |-> FALSE]
  /\ frameBase = [attachment \in Attachments |-> None]
  /\ frameRevision = [attachment \in Attachments |-> 0]
  /\ frameGeneration = [attachment \in Attachments |-> 1]
  /\ frameSequence = [attachment \in Attachments |-> 0]
  /\ nextSequence = [attachment \in Attachments |-> 1]
  /\ lastAppliedSequence = [attachment \in Attachments |-> 0]
  /\ lastAbandonedSequence = [attachment \in Attachments |-> 0]
  /\ pendingDirty = [attachment \in Attachments |-> FALSE]
  /\ forceSnapshot = [attachment \in Attachments |-> TRUE]
  /\ provisional = [attachment \in Attachments |-> FALSE]
  /\ commitGuard = {}
  /\ failurePublished = FALSE
  /\ containmentConfirmed = FALSE
  /\ handoffDuringUnconfirmed = FALSE

Mutate ==
  /\ ~finalized
  /\ ~failurePublished
  /\ commitGuard = {}
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ pendingDirty' =
       [attachment \in Attachments |->
          IF demand[attachment] THEN TRUE ELSE pendingDirty[attachment]]
  /\ UNCHANGED <<generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, blockedRevision,
                  acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, lastAbandonedSequence, forceSnapshot,
                  provisional, commitGuard, failurePublished,
                  containmentConfirmed, handoffDuringUnconfirmed>>

ResizeOrScreenSwitch ==
  /\ ~finalized
  /\ ~failurePublished
  /\ commitGuard = {}
  /\ revision < MaxRevision
  /\ generation < MaxGeneration
  /\ revision' = revision + 1
  /\ generation' = generation + 1
  /\ captured' = FALSE
  /\ pendingDirty' =
       [attachment \in Attachments |->
          IF demand[attachment] THEN TRUE ELSE pendingDirty[attachment]]
  /\ forceSnapshot' = [attachment \in Attachments |-> TRUE]
  /\ provisional' = [attachment \in Attachments |-> FALSE]
  /\ UNCHANGED <<finalized, capturedRevision, capturedGeneration, demand,
                  blocked, blockedRevision,
                  acknowledgedRevision, acknowledgedGeneration, inFlight,
                  frameBase, frameRevision, frameGeneration, frameSequence,
                  nextSequence, lastAppliedSequence, lastAbandonedSequence,
                  commitGuard, failurePublished,
                  containmentConfirmed, handoffDuringUnconfirmed>>

SetDemand(attachment, active) ==
  /\ attachment \in Attachments
  /\ active \in BOOLEAN
  /\ commitGuard = {}
  /\ LET nextDemand == [demand EXCEPT ![attachment] = active]
         becomingActive == active /\ ~demand[attachment]
         becomingPaused == ~active /\ demand[attachment]
     IN
       /\ demand' = nextDemand
       /\ captured' = IF AnyDemand(nextDemand) THEN captured ELSE FALSE
       /\ blocked' = IF becomingActive \/ becomingPaused
                        THEN [blocked EXCEPT ![attachment] = FALSE]
                        ELSE blocked
       /\ blockedRevision' = IF becomingActive \/ becomingPaused
              THEN [blockedRevision EXCEPT ![attachment] = None]
              ELSE blockedRevision
       /\ acknowledgedRevision' = IF becomingPaused
              THEN [acknowledgedRevision EXCEPT ![attachment] = None]
              ELSE acknowledgedRevision
       /\ acknowledgedGeneration' = IF becomingPaused
              THEN [acknowledgedGeneration EXCEPT ![attachment] = NoGeneration]
              ELSE acknowledgedGeneration
       /\ inFlight' = IF becomingPaused
                          THEN [inFlight EXCEPT ![attachment] = FALSE]
                          ELSE inFlight
       /\ lastAbandonedSequence' = IF becomingPaused /\ inFlight[attachment]
              THEN [lastAbandonedSequence EXCEPT ![attachment] = frameSequence[attachment]]
              ELSE lastAbandonedSequence
       /\ pendingDirty' = IF becomingActive
              THEN [pendingDirty EXCEPT ![attachment] = TRUE]
              ELSE IF becomingPaused
                THEN [pendingDirty EXCEPT ![attachment] = FALSE]
                ELSE pendingDirty
       /\ forceSnapshot' = IF becomingActive \/ becomingPaused
              THEN [forceSnapshot EXCEPT ![attachment] = TRUE]
              ELSE forceSnapshot
       /\ provisional' = IF becomingActive \/ becomingPaused
              THEN [provisional EXCEPT ![attachment] = FALSE]
              ELSE provisional
  /\ UNCHANGED <<revision, generation, finalized, capturedRevision,
                  capturedGeneration, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, commitGuard, failurePublished,
                  containmentConfirmed, handoffDuringUnconfirmed>>

ReadyForProjection(attachment) ==
  /\ demand[attachment]
  /\ pendingDirty[attachment]
  /\ ~inFlight[attachment]
  /\ ~blocked[attachment]
  /\ nextSequence[attachment] <= MaxSequence

ReadyForCapture(attachment) ==
  /\ ReadyForProjection(attachment)
  /\ Readable
  /\ ~provisional[attachment]
  /\ commitGuard = {}

Capture ==
  /\ \E attachment \in Attachments: ReadyForCapture(attachment)
  /\ captured' = TRUE
  /\ capturedRevision' = revision
  /\ capturedGeneration' = generation
  /\ UNCHANGED <<revision, generation, finalized, demand, blocked,
                  blockedRevision,
                  acknowledgedRevision, acknowledgedGeneration, inFlight,
                  frameBase, frameRevision, frameGeneration, frameSequence,
                  nextSequence, lastAppliedSequence, lastAbandonedSequence,
                  pendingDirty, forceSnapshot, provisional, commitGuard,
                  failurePublished, containmentConfirmed,
                  handoffDuringUnconfirmed>>

CaptureFailure(attachment) ==
  /\ attachment \in Attachments
  /\ ReadyForCapture(attachment)
  /\ captured' = FALSE
  /\ blocked' = [blocked EXCEPT ![attachment] = TRUE]
  /\ blockedRevision' = [blockedRevision EXCEPT ![attachment] = revision]
  /\ UNCHANGED <<revision, generation, finalized, capturedRevision,
                  capturedGeneration, demand, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, lastAbandonedSequence,
                  pendingDirty, forceSnapshot, provisional, commitGuard,
                  failurePublished, containmentConfirmed,
                  handoffDuringUnconfirmed>>

ReadyToEncode(attachment) ==
  /\ ReadyForProjection(attachment)
  /\ captured
  /\ capturedRevision <= revision
  /\ capturedGeneration <= generation
  /\ ~provisional[attachment]

CapturedCanEncode(attachment) ==
  \/ forceSnapshot[attachment]
  \/ acknowledgedRevision[attachment] = None
  \/ acknowledgedGeneration[attachment] # capturedGeneration
  \/ acknowledgedRevision[attachment] < capturedRevision

EncodeFailure(attachment) ==
  /\ attachment \in Attachments
  /\ ReadyToEncode(attachment)
  /\ Readable
  /\ blocked' = [blocked EXCEPT ![attachment] = TRUE]
  /\ blockedRevision' =
       [blockedRevision EXCEPT ![attachment] = capturedRevision]
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, lastAbandonedSequence,
                  pendingDirty, forceSnapshot, provisional, commitGuard,
                  failurePublished, containmentConfirmed,
                  handoffDuringUnconfirmed>>

PrepareFrame(attachment) ==
  /\ attachment \in Attachments
  /\ ReadyToEncode(attachment)
  /\ CapturedCanEncode(attachment)
  /\ Readable
  /\ provisional' = [provisional EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, blockedRevision,
                  acknowledgedRevision, acknowledgedGeneration, inFlight,
                  frameBase, frameRevision, frameGeneration, frameSequence,
                  nextSequence, lastAppliedSequence, lastAbandonedSequence,
                  pendingDirty, forceSnapshot, commitGuard, failurePublished,
                  containmentConfirmed, handoffDuringUnconfirmed>>

AcquireReadCommit(attachment) ==
  /\ attachment \in Attachments
  /\ provisional[attachment]
  /\ commitGuard = {}
  /\ Readable
  /\ commitGuard' = {attachment}
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, blockedRevision,
                  acknowledgedRevision, acknowledgedGeneration, inFlight,
                  frameBase, frameRevision, frameGeneration, frameSequence,
                  nextSequence, lastAppliedSequence, lastAbandonedSequence,
                  pendingDirty, forceSnapshot, provisional, failurePublished,
                  containmentConfirmed, handoffDuringUnconfirmed>>

Emit(attachment) ==
  /\ attachment \in Attachments
  /\ provisional[attachment]
  /\ commitGuard = {attachment}
  /\ inFlight' = [inFlight EXCEPT ![attachment] = TRUE]
  /\ frameBase' = [frameBase EXCEPT
       ![attachment] = IF forceSnapshot[attachment]
                         \/ acknowledgedRevision[attachment] = None
                         \/ acknowledgedGeneration[attachment] # capturedGeneration
                      THEN None
                      ELSE acknowledgedRevision[attachment]]
  /\ frameRevision' = [frameRevision EXCEPT ![attachment] = capturedRevision]
  /\ frameGeneration' = [frameGeneration EXCEPT ![attachment] = capturedGeneration]
  /\ frameSequence' = [frameSequence EXCEPT ![attachment] = nextSequence[attachment]]
  /\ nextSequence' = [nextSequence EXCEPT ![attachment] = @ + 1]
  /\ pendingDirty' = [pendingDirty EXCEPT ![attachment] = FALSE]
  /\ forceSnapshot' = [forceSnapshot EXCEPT ![attachment] = FALSE]
  /\ provisional' = [provisional EXCEPT ![attachment] = FALSE]
  /\ commitGuard' = {}
  /\ handoffDuringUnconfirmed' =
       handoffDuringUnconfirmed \/ (failurePublished /\ ~containmentConfirmed)
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, blockedRevision,
                  acknowledgedRevision,
                  acknowledgedGeneration, lastAppliedSequence,
                  lastAbandonedSequence, failurePublished,
                  containmentConfirmed>>

RefreshBlocked(attachment) ==
  /\ attachment \in Attachments
  /\ blocked[attachment]
  /\ blockedRevision[attachment] < revision
  /\ commitGuard = {}
  /\ blocked' = [blocked EXCEPT ![attachment] = FALSE]
  /\ blockedRevision' = [blockedRevision EXCEPT ![attachment] = None]
  /\ pendingDirty' = [pendingDirty EXCEPT ![attachment] = TRUE]
  /\ forceSnapshot' = [forceSnapshot EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, lastAbandonedSequence, provisional,
                  commitGuard, failurePublished, containmentConfirmed,
                  handoffDuringUnconfirmed>>

PublishFailure ==
  /\ ~failurePublished
  /\ commitGuard = {}
  /\ failurePublished' = TRUE
  /\ captured' = FALSE
  /\ provisional' = [attachment \in Attachments |-> FALSE]
  /\ UNCHANGED <<revision, generation, finalized, capturedRevision,
                  capturedGeneration, demand, blocked, blockedRevision,
                  acknowledgedRevision, acknowledgedGeneration, inFlight,
                  frameBase, frameRevision, frameGeneration, frameSequence,
                  nextSequence, lastAppliedSequence, lastAbandonedSequence,
                  pendingDirty, forceSnapshot, commitGuard,
                  containmentConfirmed, handoffDuringUnconfirmed>>

ConfirmContainment ==
  /\ failurePublished
  /\ ~containmentConfirmed
  /\ commitGuard = {}
  /\ containmentConfirmed' = TRUE
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, blockedRevision,
                  acknowledgedRevision, acknowledgedGeneration, inFlight,
                  frameBase, frameRevision, frameGeneration, frameSequence,
                  nextSequence, lastAppliedSequence, lastAbandonedSequence,
                  pendingDirty, forceSnapshot, provisional, commitGuard,
                  failurePublished, handoffDuringUnconfirmed>>

FrameApplied(attachment) ==
  /\ attachment \in Attachments
  /\ inFlight[attachment]
  /\ acknowledgedRevision' =
       [acknowledgedRevision EXCEPT ![attachment] = frameRevision[attachment]]
  /\ acknowledgedGeneration' =
       [acknowledgedGeneration EXCEPT ![attachment] = frameGeneration[attachment]]
  /\ lastAppliedSequence' =
       [lastAppliedSequence EXCEPT ![attachment] = frameSequence[attachment]]
  /\ inFlight' = [inFlight EXCEPT ![attachment] = FALSE]
  /\ captured' = IF frameRevision[attachment] < revision
                       \/ frameGeneration[attachment] # generation
       THEN FALSE
       ELSE captured
  /\ pendingDirty' = IF frameRevision[attachment] < revision
                         \/ frameGeneration[attachment] # generation
       THEN [pendingDirty EXCEPT ![attachment] = TRUE]
       ELSE pendingDirty
  /\ UNCHANGED <<revision, generation, finalized, capturedRevision,
                  capturedGeneration, demand, blocked, blockedRevision, frameBase,
                  frameRevision, frameGeneration, frameSequence,
                  nextSequence, lastAbandonedSequence, forceSnapshot,
                  provisional, commitGuard, failurePublished,
                  containmentConfirmed, handoffDuringUnconfirmed>>

DuplicateFrameApplied(attachment) ==
  /\ attachment \in Attachments
  /\ ~inFlight[attachment]
  /\ lastAppliedSequence[attachment] > 0
  /\ UNCHANGED vars

AbandonedFrameApplied(attachment) ==
  /\ attachment \in Attachments
  /\ lastAbandonedSequence[attachment] > 0
  /\ UNCHANGED vars

NeedSnapshot(attachment) ==
  /\ attachment \in Attachments
  /\ inFlight[attachment]
  /\ inFlight' = [inFlight EXCEPT ![attachment] = FALSE]
  /\ lastAbandonedSequence' =
       [lastAbandonedSequence EXCEPT ![attachment] = frameSequence[attachment]]
  /\ pendingDirty' = [pendingDirty EXCEPT ![attachment] = TRUE]
  /\ forceSnapshot' = [forceSnapshot EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, blockedRevision,
                  acknowledgedRevision,
                  acknowledgedGeneration, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, provisional, commitGuard,
                  failurePublished, containmentConfirmed,
                  handoffDuringUnconfirmed>>

AbandonedNeedSnapshot(attachment) ==
  /\ attachment \in Attachments
  /\ lastAbandonedSequence[attachment] > 0
  /\ UNCHANGED vars

Finalize ==
  /\ ~finalized
  /\ commitGuard = {}
  /\ finalized' = TRUE
  /\ UNCHANGED <<revision, generation, captured, capturedRevision,
                  capturedGeneration, demand, blocked, blockedRevision,
                  acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, lastAbandonedSequence,
                  pendingDirty, forceSnapshot, provisional, commitGuard,
                  failurePublished, containmentConfirmed,
                  handoffDuringUnconfirmed>>

Next ==
  \/ Mutate
  \/ ResizeOrScreenSwitch
  \/ Capture
  \/ Finalize
  \/ PublishFailure
  \/ ConfirmContainment
  \/ \E attachment \in Attachments:
       \/ SetDemand(attachment, TRUE)
       \/ SetDemand(attachment, FALSE)
       \/ CaptureFailure(attachment)
       \/ EncodeFailure(attachment)
       \/ PrepareFrame(attachment)
       \/ AcquireReadCommit(attachment)
       \/ Emit(attachment)
       \/ RefreshBlocked(attachment)
       \/ FrameApplied(attachment)
       \/ DuplicateFrameApplied(attachment)
       \/ AbandonedFrameApplied(attachment)
       \/ NeedSnapshot(attachment)
       \/ AbandonedNeedSnapshot(attachment)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Capture)
  /\ \A attachment \in Attachments: WF_vars(PrepareFrame(attachment))
  /\ \A attachment \in Attachments: WF_vars(AcquireReadCommit(attachment))
  /\ \A attachment \in Attachments: WF_vars(Emit(attachment))
  /\ \A attachment \in Attachments: WF_vars(RefreshBlocked(attachment))
  /\ \A attachment \in Attachments: WF_vars(FrameApplied(attachment))

TypeOK ==
  /\ revision \in 0..MaxRevision
  /\ generation \in 1..MaxGeneration
  /\ finalized \in BOOLEAN
  /\ captured \in BOOLEAN
  /\ capturedRevision \in 0..MaxRevision
  /\ capturedGeneration \in 1..MaxGeneration
  /\ demand \in [Attachments -> BOOLEAN]
  /\ blocked \in [Attachments -> BOOLEAN]
  /\ blockedRevision \in [Attachments -> 0..None]
  /\ acknowledgedRevision \in [Attachments -> 0..None]
  /\ acknowledgedGeneration \in [Attachments -> 1..NoGeneration]
  /\ inFlight \in [Attachments -> BOOLEAN]
  /\ frameBase \in [Attachments -> 0..None]
  /\ frameRevision \in [Attachments -> 0..MaxRevision]
  /\ frameGeneration \in [Attachments -> 1..MaxGeneration]
  /\ frameSequence \in [Attachments -> Nat]
  /\ nextSequence \in [Attachments -> 1..(MaxSequence + 1)]
  /\ lastAppliedSequence \in [Attachments -> Nat]
  /\ lastAbandonedSequence \in [Attachments -> 0..MaxSequence]
  /\ pendingDirty \in [Attachments -> BOOLEAN]
  /\ forceSnapshot \in [Attachments -> BOOLEAN]
  /\ provisional \in [Attachments -> BOOLEAN]
  /\ commitGuard \subseteq Attachments
  /\ failurePublished \in BOOLEAN
  /\ containmentConfirmed \in BOOLEAN
  /\ handoffDuringUnconfirmed \in BOOLEAN

OneFrameAndDirtyMarker ==
  \A attachment \in Attachments:
    /\ inFlight[attachment] \in BOOLEAN
    /\ pendingDirty[attachment] \in BOOLEAN

DeltaBaseMatchesAcknowledgement ==
  \A attachment \in Attachments:
    inFlight[attachment] /\ frameBase[attachment] # None =>
      /\ frameBase[attachment] = acknowledgedRevision[attachment]
      /\ frameBase[attachment] < frameRevision[attachment]
      /\ frameGeneration[attachment] = acknowledgedGeneration[attachment]

FrameNamesCapturedCanonicalState ==
  \A attachment \in Attachments:
    inFlight[attachment] =>
      /\ frameRevision[attachment] <= revision
      /\ frameGeneration[attachment] <= generation

SequenceCommittedOnlyWithFrame ==
  \A attachment \in Attachments:
    /\ frameSequence[attachment] < nextSequence[attachment]
    /\ inFlight[attachment] => frameSequence[attachment] > lastAppliedSequence[attachment]

PausedOrBlockedCannotBecomeReady ==
  \A attachment \in Attachments:
    (~demand[attachment] \/ blocked[attachment]) => ~ReadyForProjection(attachment)

PausedOwnsNoDeliveryWork ==
  \A attachment \in Attachments:
    ~demand[attachment] =>
      /\ ~inFlight[attachment]
      /\ ~pendingDirty[attachment]
      /\ ~provisional[attachment]
      /\ attachment \notin commitGuard

NoDemandOwnsNoCapture ==
  ~AnyDemand(demand) =>
    /\ ~captured
    /\ commitGuard = {}
    /\ \A attachment \in Attachments: ~provisional[attachment]

BlockedRevisionNamesAttempt ==
  \A attachment \in Attachments:
    /\ blocked[attachment] => blockedRevision[attachment] <= revision
    /\ ~blocked[attachment] => blockedRevision[attachment] = None

ProvisionalOwnsNoFlight ==
  \A attachment \in Attachments:
    provisional[attachment] =>
      /\ demand[attachment]
      /\ pendingDirty[attachment]
      /\ ~blocked[attachment]
      /\ ~inFlight[attachment]

ProvisionalNamesCurrentGeneration ==
  \A attachment \in Attachments:
    provisional[attachment] => capturedGeneration = generation

CommitGuardOwnsProvisional ==
  /\ Cardinality(commitGuard) <= 1
  /\ \A attachment \in commitGuard: provisional[attachment]

UnconfirmedFailurePermitsNoNewFrameHandoff ==
  /\ ~handoffDuringUnconfirmed
  /\ (failurePublished /\ ~containmentConfirmed) => commitGuard = {}

UnconfirmedFailureOwnsNoProvisional ==
  (failurePublished /\ ~containmentConfirmed) =>
    \A attachment \in Attachments: ~provisional[attachment]

ContainmentConfirmationFollowsFailure ==
  containmentConfirmed => failurePublished

ReadyDemandEventuallyEmits ==
  \A attachment \in Attachments:
    ReadyToEncode(attachment) ~>
      (inFlight[attachment] \/ blocked[attachment] \/ ~demand[attachment] \/ ~captured)

InFlightEventuallyClears ==
  \A attachment \in Attachments:
    inFlight[attachment] ~> ~inFlight[attachment]

=============================================================================
