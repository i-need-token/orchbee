---------------------- MODULE TerminalFrameBackpressure ----------------------
EXTENDS Naturals, TLC

CONSTANTS Attachments, MaxRevision, MaxGeneration, MaxSequence, MaxInput

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
  acknowledgedRevision,
  acknowledgedGeneration,
  inFlight,
  frameBase,
  frameRevision,
  frameGeneration,
  frameSequence,
  nextSequence,
  lastAppliedSequence,
  pendingDirty,
  forceSnapshot,
  inputBuffer

vars ==
  <<revision, generation, finalized, captured, capturedRevision,
    capturedGeneration, demand, blocked, acknowledgedRevision,
    acknowledgedGeneration, inFlight, frameBase, frameRevision,
    frameGeneration, frameSequence, nextSequence, lastAppliedSequence,
    pendingDirty, forceSnapshot, inputBuffer>>

Init ==
  /\ revision = 0
  /\ generation = 1
  /\ finalized = FALSE
  /\ captured = FALSE
  /\ capturedRevision = 0
  /\ capturedGeneration = 1
  /\ demand = [attachment \in Attachments |-> FALSE]
  /\ blocked = [attachment \in Attachments |-> FALSE]
  /\ acknowledgedRevision = [attachment \in Attachments |-> None]
  /\ acknowledgedGeneration = [attachment \in Attachments |-> NoGeneration]
  /\ inFlight = [attachment \in Attachments |-> FALSE]
  /\ frameBase = [attachment \in Attachments |-> None]
  /\ frameRevision = [attachment \in Attachments |-> 0]
  /\ frameGeneration = [attachment \in Attachments |-> 1]
  /\ frameSequence = [attachment \in Attachments |-> 0]
  /\ nextSequence = [attachment \in Attachments |-> 1]
  /\ lastAppliedSequence = [attachment \in Attachments |-> 0]
  /\ pendingDirty = [attachment \in Attachments |-> TRUE]
  /\ forceSnapshot = [attachment \in Attachments |-> TRUE]
  /\ inputBuffer = 0

Mutate ==
  /\ ~finalized
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ pendingDirty' = [attachment \in Attachments |-> TRUE]
  /\ UNCHANGED <<generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, forceSnapshot, inputBuffer>>

ResizeOrScreenSwitch ==
  /\ ~finalized
  /\ revision < MaxRevision
  /\ generation < MaxGeneration
  /\ revision' = revision + 1
  /\ generation' = generation + 1
  /\ captured' = FALSE
  /\ pendingDirty' = [attachment \in Attachments |-> TRUE]
  /\ forceSnapshot' = [attachment \in Attachments |-> TRUE]
  /\ blocked' = [attachment \in Attachments |-> FALSE]
  /\ UNCHANGED <<finalized, capturedRevision, capturedGeneration, demand,
                  acknowledgedRevision, acknowledgedGeneration, inFlight,
                  frameBase, frameRevision, frameGeneration, frameSequence,
                  nextSequence, lastAppliedSequence, inputBuffer>>

SetDemand(attachment, active) ==
  /\ attachment \in Attachments
  /\ active \in BOOLEAN
  /\ demand' = [demand EXCEPT ![attachment] = active]
  /\ pendingDirty' = IF active /\ ~demand[attachment] /\ ~inFlight[attachment]
                            /\ (acknowledgedRevision[attachment] # revision
                                \/ acknowledgedGeneration[attachment] # generation)
       THEN [pendingDirty EXCEPT ![attachment] = TRUE]
       ELSE pendingDirty
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, blocked, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, forceSnapshot, inputBuffer>>

ReadyForCapture(attachment) ==
  /\ demand[attachment]
  /\ pendingDirty[attachment]
  /\ ~inFlight[attachment]
  /\ ~blocked[attachment]
  /\ nextSequence[attachment] <= MaxSequence

Capture ==
  /\ \E attachment \in Attachments: ReadyForCapture(attachment)
  /\ captured' = TRUE
  /\ capturedRevision' = revision
  /\ capturedGeneration' = generation
  /\ UNCHANGED <<revision, generation, finalized, demand, blocked,
                  acknowledgedRevision, acknowledgedGeneration, inFlight,
                  frameBase, frameRevision, frameGeneration, frameSequence,
                  nextSequence, lastAppliedSequence, pendingDirty,
                  forceSnapshot, inputBuffer>>

CaptureFailure(attachment) ==
  /\ attachment \in Attachments
  /\ ReadyForCapture(attachment)
  /\ captured' = FALSE
  /\ blocked' = [blocked EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, finalized, capturedRevision,
                  capturedGeneration, demand, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, pendingDirty, forceSnapshot,
                  inputBuffer>>

ReadyToEncode(attachment) ==
  /\ ReadyForCapture(attachment)
  /\ captured
  /\ capturedRevision = revision
  /\ capturedGeneration = generation

EncodeFailure(attachment) ==
  /\ attachment \in Attachments
  /\ ReadyToEncode(attachment)
  /\ blocked' = [blocked EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, pendingDirty, forceSnapshot,
                  inputBuffer>>

Emit(attachment) ==
  /\ attachment \in Attachments
  /\ ReadyToEncode(attachment)
  /\ inFlight' = [inFlight EXCEPT ![attachment] = TRUE]
  /\ frameBase' = [frameBase EXCEPT
       ![attachment] = IF forceSnapshot[attachment]
                         \/ acknowledgedRevision[attachment] = None
                         \/ acknowledgedGeneration[attachment] # generation
                      THEN None
                      ELSE acknowledgedRevision[attachment]]
  /\ frameRevision' = [frameRevision EXCEPT ![attachment] = capturedRevision]
  /\ frameGeneration' = [frameGeneration EXCEPT ![attachment] = capturedGeneration]
  /\ frameSequence' = [frameSequence EXCEPT ![attachment] = nextSequence[attachment]]
  /\ nextSequence' = [nextSequence EXCEPT ![attachment] = @ + 1]
  /\ pendingDirty' = [pendingDirty EXCEPT ![attachment] = FALSE]
  /\ forceSnapshot' = [forceSnapshot EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, acknowledgedRevision,
                  acknowledgedGeneration, lastAppliedSequence, inputBuffer>>

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
  /\ pendingDirty' = IF frameRevision[attachment] < revision
                         \/ frameGeneration[attachment] # generation
       THEN [pendingDirty EXCEPT ![attachment] = TRUE]
       ELSE pendingDirty
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, frameBase,
                  frameRevision, frameGeneration, frameSequence,
                  nextSequence, forceSnapshot, inputBuffer>>

DuplicateFrameApplied(attachment) ==
  /\ attachment \in Attachments
  /\ ~inFlight[attachment]
  /\ lastAppliedSequence[attachment] > 0
  /\ UNCHANGED vars

NeedSnapshot(attachment) ==
  /\ attachment \in Attachments
  /\ inFlight[attachment]
  /\ inFlight' = [inFlight EXCEPT ![attachment] = FALSE]
  /\ pendingDirty' = [pendingDirty EXCEPT ![attachment] = TRUE]
  /\ forceSnapshot' = [forceSnapshot EXCEPT ![attachment] = TRUE]
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, acknowledgedRevision,
                  acknowledgedGeneration, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, inputBuffer>>

Finalize ==
  /\ ~finalized
  /\ finalized' = TRUE
  /\ UNCHANGED <<revision, generation, captured, capturedRevision,
                  capturedGeneration, demand, blocked, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, pendingDirty, forceSnapshot,
                  inputBuffer>>

AcceptInput(bytes) ==
  /\ bytes \in 1..MaxInput
  /\ inputBuffer + bytes <= MaxInput
  /\ inputBuffer' = inputBuffer + bytes
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, pendingDirty, forceSnapshot>>

WriteInput ==
  /\ inputBuffer > 0
  /\ inputBuffer' = 0
  /\ UNCHANGED <<revision, generation, finalized, captured, capturedRevision,
                  capturedGeneration, demand, blocked, acknowledgedRevision,
                  acknowledgedGeneration, inFlight, frameBase, frameRevision,
                  frameGeneration, frameSequence, nextSequence,
                  lastAppliedSequence, pendingDirty, forceSnapshot>>

Next ==
  \/ Mutate
  \/ ResizeOrScreenSwitch
  \/ Capture
  \/ Finalize
  \/ \E attachment \in Attachments:
       \/ SetDemand(attachment, TRUE)
       \/ SetDemand(attachment, FALSE)
       \/ CaptureFailure(attachment)
       \/ EncodeFailure(attachment)
       \/ Emit(attachment)
       \/ FrameApplied(attachment)
       \/ DuplicateFrameApplied(attachment)
       \/ NeedSnapshot(attachment)
  \/ \E bytes \in 1..MaxInput: AcceptInput(bytes)
  \/ WriteInput

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Capture)
  /\ \A attachment \in Attachments: WF_vars(Emit(attachment))
  /\ \A attachment \in Attachments: WF_vars(FrameApplied(attachment))
  /\ WF_vars(WriteInput)

TypeOK ==
  /\ revision \in 0..MaxRevision
  /\ generation \in 1..MaxGeneration
  /\ finalized \in BOOLEAN
  /\ captured \in BOOLEAN
  /\ capturedRevision \in 0..MaxRevision
  /\ capturedGeneration \in 1..MaxGeneration
  /\ demand \in [Attachments -> BOOLEAN]
  /\ blocked \in [Attachments -> BOOLEAN]
  /\ acknowledgedRevision \in [Attachments -> 0..None]
  /\ acknowledgedGeneration \in [Attachments -> 1..NoGeneration]
  /\ inFlight \in [Attachments -> BOOLEAN]
  /\ frameBase \in [Attachments -> 0..None]
  /\ frameRevision \in [Attachments -> 0..MaxRevision]
  /\ frameGeneration \in [Attachments -> 1..MaxGeneration]
  /\ frameSequence \in [Attachments -> Nat]
  /\ nextSequence \in [Attachments -> 1..(MaxSequence + 1)]
  /\ lastAppliedSequence \in [Attachments -> Nat]
  /\ pendingDirty \in [Attachments -> BOOLEAN]
  /\ forceSnapshot \in [Attachments -> BOOLEAN]
  /\ inputBuffer \in 0..MaxInput

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

InputMemoryIsBounded == inputBuffer <= MaxInput

PausedOrBlockedCannotBecomeReady ==
  \A attachment \in Attachments:
    (~demand[attachment] \/ blocked[attachment]) => ~ReadyForCapture(attachment)

ReadyDemandEventuallyEmits ==
  \A attachment \in Attachments:
    ReadyToEncode(attachment) ~>
      (inFlight[attachment] \/ blocked[attachment] \/ ~demand[attachment])

InFlightEventuallyClears ==
  \A attachment \in Attachments:
    inFlight[attachment] ~> ~inFlight[attachment]

AcceptedInputEventuallyWrites == inputBuffer > 0 ~> inputBuffer = 0

=============================================================================
