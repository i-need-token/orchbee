------------------------ MODULE LocalTerminalRelay ------------------------
EXTENDS LocalTerminalClientAuthority, Sequences

\* Transport refinement of LocalTerminalClientAuthority for one retained
\* terminal on the first Host -> Worker -> Server -> Client relay. The
\* imported viewers and controller are the only product authority: this
\* module adds attachment epochs, bounded per-attachment queues, opaque OBTF
\* records, exact feedback, and one abstract control-request flight. It does
\* not model a wire encoding, VT, history, runtime implementation, takeover,
\* or release resource budget.

CONSTANTS RequestIds, FirstOpaqueFrameBytes, SecondOpaqueFrameBytes,
  NoOpaqueFrame,
  MaxAttachmentGeneration, MaxRequestSequence, MaxFrameSequence,
  MaxProjectionGeneration, MaxContentRevision,
  MaxQueueDepth, MaxWorkerProgress, Scenario

ProjectionScenarios == {
  "projection-feedback", "backpressure-isolation", "reattach-feedback",
  "failure-interleavings"
}
ControlScenarios == {"control-outcomes"}
FailureScenarios == {"failure-interleavings"}
ControlFlightScenarios == ControlScenarios \cup FailureScenarios
LifecycleScenarios == {"attachment-lifecycle"}
Scenarios == ProjectionScenarios \cup ControlScenarios \cup LifecycleScenarios
RelaySessionRefs ==
  IF Scenario \in LifecycleScenarios
  THEN SessionRefs
  ELSE [client : Clients, generation : {1}]

ProjectionPhases == {
  "detached", "needs-snapshot", "streaming", "paused", "blocked"
}
FrameKinds == {"snapshot", "delta"}
FeedbackKinds == {"none", "frame-applied", "need-snapshot"}
FeedbackPhases == {"none", "at-server", "at-worker", "at-host"}
ControlKinds == {"none", "input", "resize"}
ControlPhases == {
  "idle", "at-server", "at-worker", "committed",
  "result-at-server", "unknown"
}
NoRequestId == "no-request"
OpaqueFrameBytes == {FirstOpaqueFrameBytes, SecondOpaqueFrameBytes}

\* OpaqueFrameBytes supplies bounded opaque payload tags for complete admitted
\* frame and ProjectionBlocked records. Relay actions compare and move these
\* values but expose no field derived from their bytes.
FrameRecord(session, attachment, sequence, generation, revision, kind, bytes) == [
  recordKind |-> "frame",
  session |-> session,
  attachment |-> attachment,
  sequence |-> sequence,
  generation |-> generation,
  revision |-> revision,
  frameKind |-> kind,
  bytes |-> bytes
]

SourceOpaqueBytes(sequence) ==
  IF sequence = 1 THEN FirstOpaqueFrameBytes ELSE SecondOpaqueFrameBytes

SourceFrameRecord(session, attachment, sequence, generation, revision, kind) ==
  FrameRecord(
    session,
    attachment,
    sequence,
    generation,
    revision,
    kind,
    SourceOpaqueBytes(sequence))

ProjectionBlockedRecord(session, attachment, revision, bytes) == [
  recordKind |-> "projection-blocked",
  session |-> session,
  attachment |-> attachment,
  sequence |-> 0,
  generation |-> 0,
  revision |-> revision,
  frameKind |-> "none",
  bytes |-> bytes
]

FrameRecords == [
  recordKind : {"frame"},
  session : RelaySessionRefs,
  attachment : 1..MaxAttachmentGeneration,
  sequence : 1..MaxFrameSequence,
  generation : 1..MaxProjectionGeneration,
  revision : 0..MaxContentRevision,
  frameKind : FrameKinds,
  bytes : OpaqueFrameBytes
]

ProjectionBlockedRecords == [
  recordKind : {"projection-blocked"},
  session : RelaySessionRefs,
  attachment : 1..MaxAttachmentGeneration,
  sequence : {0},
  generation : {0},
  revision : 0..MaxContentRevision,
  frameKind : {"none"},
  bytes : OpaqueFrameBytes
]

DownstreamRecords == FrameRecords \cup ProjectionBlockedRecords

VARIABLES
  attachmentGeneration,
  projectionPhase,
  hostWorkerQueues,
  workerServerQueues,
  serverClientQueues,
  sourceRecords,
  nextFrameSequence,
  lastAppliedSequence,
  hostAppliedSequence,
  blockedRevision,
  feedbackKind,
  feedbackPhase,
  feedbackSequence,
  feedbackAttachment,
  feedbackGeneration,
  feedbackRevision,
  blockedRecoveryRevisionValid,
  acceptedRequestSequence,
  usedRequestIds,
  unknownRequestIds,
  controlPhase,
  controlOwner,
  controlAttachment,
  controlKind,
  controlRequestId,
  controlSequence,
  controlRejections,
  workerProgress

RelayVars == <<
  attachmentGeneration,
  projectionPhase,
  hostWorkerQueues,
  workerServerQueues,
  serverClientQueues,
  sourceRecords,
  nextFrameSequence,
  lastAppliedSequence,
  hostAppliedSequence,
  blockedRevision,
  feedbackKind,
  feedbackPhase,
  feedbackSequence,
  feedbackAttachment,
  feedbackGeneration,
  feedbackRevision,
  blockedRecoveryRevisionValid,
  acceptedRequestSequence,
  usedRequestIds,
  unknownRequestIds,
  controlPhase,
  controlOwner,
  controlAttachment,
  controlKind,
  controlRequestId,
  controlSequence,
  controlRejections,
  workerProgress
>>

allVars == <<vars, RelayVars>>

RelaySemanticVars == <<
  vars,
  attachmentGeneration,
  projectionPhase,
  hostWorkerQueues,
  workerServerQueues,
  serverClientQueues,
  sourceRecords,
  nextFrameSequence,
  lastAppliedSequence,
  hostAppliedSequence,
  blockedRevision,
  feedbackKind,
  feedbackPhase,
  feedbackSequence,
  feedbackAttachment,
  feedbackGeneration,
  feedbackRevision,
  acceptedRequestSequence,
  usedRequestIds,
  unknownRequestIds,
  controlPhase,
  controlOwner,
  controlAttachment,
  controlKind,
  controlRequestId,
  controlSequence,
  controlRejections,
  workerProgress
>>

ASSUME
  /\ IsFiniteSet(Clients)
  /\ Cardinality(Clients) >= 2
  /\ IsFiniteSet(RequestIds)
  /\ Cardinality(RequestIds) >= 2
  /\ FirstOpaqueFrameBytes # SecondOpaqueFrameBytes
  /\ NoOpaqueFrame \notin OpaqueFrameBytes
  /\ NoRequestId \notin RequestIds
  /\ MaxAttachmentGeneration >= 1
  /\ MaxRequestSequence >= 1
  /\ MaxFrameSequence >= 2
  /\ MaxProjectionGeneration >= 2
  /\ MaxContentRevision >= 1
  /\ MaxQueueDepth >= 1
  /\ MaxWorkerProgress >= 1
  /\ Scenario \in Scenarios

CurrentAttachment(session) ==
  [session |-> session, generation |-> attachmentGeneration[session]]

AttachedProjectionSessions == {
  session \in RelaySessionRefs : projectionPhase[session] # "detached"
}

HostReadAttachments ==
  IF TerminalPathAvailable
  THEN {CurrentAttachment(session) : session \in viewers}
  ELSE {}

HostInputAttachments ==
  IF TerminalPathAvailable /\ controller # NoController
  THEN {CurrentAttachment(controller)}
  ELSE {}

HostResizeAttachments == HostInputAttachments

QueueIsBounded(queue) == Len(queue) <= MaxQueueDepth

QueueIsScoped(queue, session) ==
  \A index \in 1..Len(queue):
    /\ queue[index] \in DownstreamRecords
    /\ queue[index].session = session
    /\ queue[index].attachment = attachmentGeneration[session]

QueuePreservesSourceRecords(queue, session) ==
  \A index \in 1..Len(queue):
    sourceRecords[session] = {queue[index]}

EveryQueueIsEmptyFor(session) ==
  /\ hostWorkerQueues[session] = <<>>
  /\ workerServerQueues[session] = <<>>
  /\ serverClientQueues[session] = <<>>

FeedbackIsEmptyFor(session) ==
  /\ feedbackKind[session] = "none"
  /\ feedbackPhase[session] = "none"
  /\ feedbackSequence[session] = 0
  /\ feedbackAttachment[session] = 0
  /\ feedbackGeneration[session] = 0
  /\ feedbackRevision[session] = 0

DeliveryFlightIsEmptyFor(session) ==
  /\ EveryQueueIsEmptyFor(session)
  /\ FeedbackIsEmptyFor(session)

DownstreamFlightIsActive ==
  \E session \in RelaySessionRefs:
    \/ hostWorkerQueues[session] # <<>>
    \/ workerServerQueues[session] # <<>>
    \/ serverClientQueues[session] # <<>>

ResetControlFields ==
  /\ controlOwner' = NoController
  /\ controlAttachment' = 0
  /\ controlKind' = "none"
  /\ controlRequestId' = NoRequestId
  /\ controlSequence' = 0

ControlBelongsTo(session) ==
  /\ controlOwner = session
  /\ controlAttachment = attachmentGeneration[session]

ControlCouldHaveEscapedServer(session) ==
  /\ ControlBelongsTo(session)
  /\ controlPhase \in {
       "at-worker", "committed", "result-at-server", "unknown"
     }

ResetDeliveryFor(session, nextPhase) ==
  /\ projectionPhase' = [projectionPhase EXCEPT ![session] = nextPhase]
  /\ hostWorkerQueues' = [hostWorkerQueues EXCEPT ![session] = <<>>]
  /\ workerServerQueues' = [workerServerQueues EXCEPT ![session] = <<>>]
  /\ serverClientQueues' = [serverClientQueues EXCEPT ![session] = <<>>]
  /\ sourceRecords' = [sourceRecords EXCEPT ![session] = {}]
  /\ nextFrameSequence' = [nextFrameSequence EXCEPT ![session] = 0]
  /\ lastAppliedSequence' = [lastAppliedSequence EXCEPT ![session] = 0]
  /\ hostAppliedSequence' = [hostAppliedSequence EXCEPT ![session] = 0]
  /\ blockedRevision' = [blockedRevision EXCEPT ![session] = 0]
  /\ acceptedRequestSequence' =
       [acceptedRequestSequence EXCEPT ![session] = 0]

PreserveDeliverySequences ==
  UNCHANGED <<nextFrameSequence, lastAppliedSequence, hostAppliedSequence,
    acceptedRequestSequence>>

RelayInit ==
  /\ Init
  /\ attachmentGeneration = [session \in RelaySessionRefs |-> 0]
  /\ projectionPhase = [session \in RelaySessionRefs |-> "detached"]
  /\ hostWorkerQueues = [session \in RelaySessionRefs |-> <<>>]
  /\ workerServerQueues = [session \in RelaySessionRefs |-> <<>>]
  /\ serverClientQueues = [session \in RelaySessionRefs |-> <<>>]
  /\ sourceRecords = [session \in RelaySessionRefs |-> {}]
  /\ nextFrameSequence = [session \in RelaySessionRefs |-> 0]
  /\ lastAppliedSequence = [session \in RelaySessionRefs |-> 0]
  /\ hostAppliedSequence = [session \in RelaySessionRefs |-> 0]
  /\ blockedRevision = [session \in RelaySessionRefs |-> 0]
  /\ feedbackKind = [session \in RelaySessionRefs |-> "none"]
  /\ feedbackPhase = [session \in RelaySessionRefs |-> "none"]
  /\ feedbackSequence = [session \in RelaySessionRefs |-> 0]
  /\ feedbackAttachment = [session \in RelaySessionRefs |-> 0]
  /\ feedbackGeneration = [session \in RelaySessionRefs |-> 0]
  /\ feedbackRevision = [session \in RelaySessionRefs |-> 0]
  /\ blockedRecoveryRevisionValid = TRUE
  /\ acceptedRequestSequence = [session \in RelaySessionRefs |-> 0]
  /\ usedRequestIds = {}
  /\ unknownRequestIds = {}
  /\ controlPhase = "idle"
  /\ controlOwner = NoController
  /\ controlAttachment = 0
  /\ controlKind = "none"
  /\ controlRequestId = NoRequestId
  /\ controlSequence = 0
  /\ controlRejections = 0
  /\ workerProgress = 0

OpenRelaySession(client) ==
  /\ OpenInitialSession(client)
  /\ UNCHANGED RelayVars

ReconnectRelaySession(client) ==
  /\ Scenario \in LifecycleScenarios
  /\ ReconnectSession(client)
  /\ UNCHANGED RelayVars

AttachRelayViewer(client) ==
  /\ serverUp
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ attachmentGeneration[session] = 0
       /\ (AttachFirstViewer(client) \/ AttachAdditionalViewer(client))
       /\ attachmentGeneration' =
            [attachmentGeneration EXCEPT ![session] = 1]
       /\ projectionPhase' =
            [projectionPhase EXCEPT ![session] = "needs-snapshot"]
  /\ UNCHANGED <<hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections, workerProgress>>

ReattachRelayViewer(client) ==
  /\ (Scenario = "reattach-feedback" \/ Scenario \in LifecycleScenarios)
  /\ serverUp
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ session \notin viewers
       /\ attachmentGeneration[session] \in 1..(MaxAttachmentGeneration - 1)
       /\ (AttachFirstViewer(client) \/ AttachAdditionalViewer(client))
       /\ attachmentGeneration' =
            [attachmentGeneration EXCEPT
              ![session] = @ + 1]
       /\ ResetDeliveryFor(session, "needs-snapshot")
  /\ UNCHANGED <<feedbackKind, feedbackPhase, feedbackSequence,
       feedbackAttachment, feedbackGeneration, feedbackRevision,
       blockedRecoveryRevisionValid, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections, workerProgress>>

AcquireRelayController(client) ==
  /\ Scenario \in ControlFlightScenarios
  /\ AcquireUnownedController(client)
  /\ UNCHANGED RelayVars

SubmitControllerInput(client, requestId, requestSequence) ==
  /\ Scenario \in ControlFlightScenarios
  /\ TerminalPathAvailable
  /\ controlPhase = "idle"
  /\ requestId \in RequestIds \ usedRequestIds
  /\ requestSequence \in 1..MaxRequestSequence
  /\ LET session == CurrentSession(client)
     IN
       /\ session = controller
       /\ acceptedRequestSequence[session] < MaxRequestSequence
       /\ requestSequence = acceptedRequestSequence[session] + 1
       /\ controlPhase' = "at-server"
       /\ controlOwner' = session
       /\ controlAttachment' = attachmentGeneration[session]
       /\ controlKind' = "input"
       /\ controlRequestId' = requestId
       /\ controlSequence' = requestSequence
       /\ acceptedRequestSequence' =
            [acceptedRequestSequence EXCEPT ![session] = @ + 1]
  /\ usedRequestIds' = usedRequestIds \cup {requestId}
  /\ UNCHANGED <<DomainVars, AuditVars, attachmentGeneration,
       projectionPhase, hostWorkerQueues, workerServerQueues,
       serverClientQueues, sourceRecords, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       unknownRequestIds, controlRejections,
       workerProgress>>

SubmitControllerResize(client, requestId, requestSequence) ==
  /\ Scenario \in ControlScenarios
  /\ TerminalPathAvailable
  /\ controlPhase = "idle"
  /\ requestId \in RequestIds \ usedRequestIds
  /\ requestSequence \in 1..MaxRequestSequence
  /\ LET session == CurrentSession(client)
     IN
       /\ session = controller
       /\ acceptedRequestSequence[session] < MaxRequestSequence
       /\ requestSequence = acceptedRequestSequence[session] + 1
       /\ controlPhase' = "at-server"
       /\ controlOwner' = session
       /\ controlAttachment' = attachmentGeneration[session]
       /\ controlKind' = "resize"
       /\ controlRequestId' = requestId
       /\ controlSequence' = requestSequence
       /\ acceptedRequestSequence' =
            [acceptedRequestSequence EXCEPT ![session] = @ + 1]
  /\ usedRequestIds' = usedRequestIds \cup {requestId}
  /\ UNCHANGED <<DomainVars, AuditVars, attachmentGeneration,
       projectionPhase, hostWorkerQueues, workerServerQueues,
       serverClientQueues, sourceRecords, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       unknownRequestIds, controlRejections,
       workerProgress>>

RejectNonControllerInputRelay(client) ==
  /\ Scenario \in ControlScenarios
  /\ RejectNonControllerInput(client)
  /\ controlRejections < MaxRequestSequence
  /\ controlRejections' = controlRejections + 1
  /\ UNCHANGED <<attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, workerProgress>>

RejectNonControllerResizeRelay(client) ==
  /\ Scenario \in ControlScenarios
  /\ RejectNonControllerResize(client)
  /\ controlRejections < MaxRequestSequence
  /\ controlRejections' = controlRejections + 1
  /\ UNCHANGED <<attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, workerProgress>>

RejectOutOfOrderControlSequence(client, requestId, requestSequence) ==
  /\ Scenario \in ControlScenarios
  /\ TerminalPathAvailable
  /\ controlPhase = "idle"
  /\ CurrentSession(client) = controller
  /\ requestId \in RequestIds \ usedRequestIds
  /\ requestSequence \in 1..MaxRequestSequence
  /\ requestSequence # acceptedRequestSequence[controller] + 1
  /\ controlRejections < MaxRequestSequence
  /\ controlRejections' = controlRejections + 1
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, workerProgress>>

RejectUsedControlIdentity(client, requestId) ==
  /\ Scenario \in ControlScenarios
  /\ TerminalPathAvailable
  /\ controlPhase = "idle"
  /\ CurrentSession(client) = controller
  /\ requestId \in usedRequestIds
  /\ controlRejections < MaxRequestSequence
  /\ controlRejections' = controlRejections + 1
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, workerProgress>>

ForwardControlToWorker ==
  /\ Scenario \in ControlFlightScenarios
  /\ TerminalPathAvailable
  /\ controlPhase = "at-server"
  /\ ControlBelongsTo(controller)
  /\ controlPhase' = "at-worker"
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlOwner, controlAttachment, controlKind, controlRequestId,
       controlSequence, controlRejections, workerProgress>>

CommitInputEffect ==
  /\ Scenario \in ControlFlightScenarios
  /\ controlPhase = "at-worker"
  /\ controlKind = "input"
  /\ ControlBelongsTo(controller)
  /\ AcceptControllerInput(controlOwner.client)
  /\ controlPhase' = "committed"
  /\ UNCHANGED <<attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlOwner, controlAttachment, controlKind, controlRequestId,
       controlSequence, controlRejections, workerProgress>>

CommitResizeEffect ==
  /\ Scenario \in ControlScenarios
  /\ controlPhase = "at-worker"
  /\ controlKind = "resize"
  /\ ControlBelongsTo(controller)
  /\ AcceptControllerResize(controlOwner.client)
  /\ controlPhase' = "committed"
  /\ UNCHANGED <<attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlOwner, controlAttachment, controlKind, controlRequestId,
       controlSequence, controlRejections, workerProgress>>

ReturnControlResultToServer ==
  /\ Scenario \in ControlFlightScenarios
  /\ TerminalPathAvailable
  /\ controlPhase = "committed"
  /\ controlPhase' = "result-at-server"
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlOwner, controlAttachment, controlKind, controlRequestId,
       controlSequence, controlRejections, workerProgress>>

AcceptControlResult ==
  /\ Scenario \in ControlScenarios
  /\ TerminalPathAvailable
  /\ controlPhase = "result-at-server"
  /\ controlPhase' = "idle"
  /\ ResetControlFields
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlRejections, workerProgress>>

LoseUnknownControlOutcome ==
  /\ Scenario \in ControlFlightScenarios
  /\ controlPhase \in {"at-worker", "committed", "result-at-server"}
  /\ controlPhase' = "unknown"
  /\ unknownRequestIds' = unknownRequestIds \cup {controlRequestId}
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress>>

RetireUnknownControl ==
  /\ Scenario \in ControlFlightScenarios
  /\ controlPhase = "unknown"
  /\ controlPhase' = "idle"
  /\ ResetControlFields
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlRejections, workerProgress>>

HostEmitSnapshot(session, generation, revision) ==
  /\ Scenario \in ProjectionScenarios
  /\ TerminalPathAvailable
  /\ session \in viewers
  /\ projectionPhase[session] = "needs-snapshot"
  /\ nextFrameSequence[session] < MaxFrameSequence
  /\ DeliveryFlightIsEmptyFor(session)
  /\ generation \in 1..MaxProjectionGeneration
  /\ revision \in 0..MaxContentRevision
  /\ LET sequence == nextFrameSequence[session] + 1
         record == SourceFrameRecord(
           session,
           attachmentGeneration[session],
           sequence,
           generation,
           revision,
           "snapshot")
     IN
       /\ hostWorkerQueues' =
            [hostWorkerQueues EXCEPT ![session] = Append(@, record)]
       /\ sourceRecords' = [sourceRecords EXCEPT ![session] = {record}]
       /\ nextFrameSequence' =
            [nextFrameSequence EXCEPT ![session] = sequence]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       workerServerQueues, serverClientQueues, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress>>

HostEmitDelta(session, generation, revision) ==
  /\ Scenario = "projection-feedback"
  /\ TerminalPathAvailable
  /\ session \in viewers
  /\ projectionPhase[session] = "streaming"
  /\ nextFrameSequence[session] < MaxFrameSequence
  /\ DeliveryFlightIsEmptyFor(session)
  /\ generation \in 1..MaxProjectionGeneration
  /\ revision \in 0..MaxContentRevision
  /\ LET sequence == nextFrameSequence[session] + 1
         record == SourceFrameRecord(
           session,
           attachmentGeneration[session],
           sequence,
           generation,
           revision,
           "delta")
     IN
       /\ hostWorkerQueues' =
            [hostWorkerQueues EXCEPT ![session] = Append(@, record)]
       /\ sourceRecords' = [sourceRecords EXCEPT ![session] = {record}]
       /\ nextFrameSequence' =
            [nextFrameSequence EXCEPT ![session] = sequence]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       workerServerQueues, serverClientQueues, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress>>

HostEmitProjectionBlocked(session, revision, bytes) ==
  /\ Scenario = "projection-feedback"
  /\ TerminalPathAvailable
  /\ session \in viewers
  /\ projectionPhase[session] \in {"needs-snapshot", "streaming"}
  /\ DeliveryFlightIsEmptyFor(session)
  /\ revision \in 0..MaxContentRevision
  /\ bytes \in OpaqueFrameBytes
  /\ LET record == ProjectionBlockedRecord(
         session,
         attachmentGeneration[session],
         revision,
         bytes)
     IN
       /\ hostWorkerQueues' =
            [hostWorkerQueues EXCEPT ![session] = Append(@, record)]
       /\ sourceRecords' = [sourceRecords EXCEPT ![session] = {record}]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       workerServerQueues, serverClientQueues, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress>>

HostRecoverBlockedProjection(session, generation, revision) ==
  /\ Scenario = "projection-feedback"
  /\ TerminalPathAvailable
  /\ session \in viewers
  /\ projectionPhase[session] = "blocked"
  /\ nextFrameSequence[session] < MaxFrameSequence
  /\ DeliveryFlightIsEmptyFor(session)
  /\ generation \in 1..MaxProjectionGeneration
  /\ revision \in (blockedRevision[session] + 1)..MaxContentRevision
  /\ LET sequence == nextFrameSequence[session] + 1
         record == SourceFrameRecord(
           session,
           attachmentGeneration[session],
           sequence,
           generation,
           revision,
           "snapshot")
     IN
       /\ hostWorkerQueues' =
            [hostWorkerQueues EXCEPT ![session] = Append(@, record)]
       /\ sourceRecords' = [sourceRecords EXCEPT ![session] = {record}]
       /\ nextFrameSequence' =
            [nextFrameSequence EXCEPT ![session] = sequence]
       /\ projectionPhase' =
            [projectionPhase EXCEPT ![session] = "needs-snapshot"]
  /\ blockedRecoveryRevisionValid' =
       /\ blockedRecoveryRevisionValid
       /\ revision > blockedRevision[session]
       /\ Cardinality(sourceRecords'[session]) = 1
       /\ \A record \in sourceRecords'[session]:
            /\ record.recordKind = "frame"
            /\ record.frameKind = "snapshot"
            /\ record.revision > blockedRevision[session]
  /\ UNCHANGED <<vars, attachmentGeneration, workerServerQueues,
       serverClientQueues, lastAppliedSequence, hostAppliedSequence,
       blockedRevision, feedbackKind, feedbackPhase, feedbackSequence,
       feedbackAttachment, feedbackGeneration, feedbackRevision,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections, workerProgress>>

RejectBlockedRecoveryWithoutNewRevision(session, revision) ==
  /\ Scenario = "projection-feedback"
  /\ TerminalPathAvailable
  /\ session \in viewers
  /\ projectionPhase[session] = "blocked"
  /\ nextFrameSequence[session] < MaxFrameSequence
  /\ DeliveryFlightIsEmptyFor(session)
  /\ revision \in 0..blockedRevision[session]
  /\ UNCHANGED allVars

RelayHostToWorker(session) ==
  /\ Scenario \in ProjectionScenarios
  /\ TerminalPathAvailable
  /\ hostWorkerQueues[session] # <<>>
  /\ Len(workerServerQueues[session]) < MaxQueueDepth
  /\ workerServerQueues' =
       [workerServerQueues EXCEPT
         ![session] = Append(@, Head(hostWorkerQueues[session]))]
  /\ hostWorkerQueues' =
       [hostWorkerQueues EXCEPT ![session] = Tail(@)]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       serverClientQueues, sourceRecords, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress>>

RelayWorkerToServer(session) ==
  /\ Scenario \in ProjectionScenarios
  /\ TerminalPathAvailable
  /\ workerServerQueues[session] # <<>>
  /\ Len(serverClientQueues[session]) < MaxQueueDepth
  /\ serverClientQueues' =
       [serverClientQueues EXCEPT
         ![session] = Append(@, Head(workerServerQueues[session]))]
  /\ workerServerQueues' =
       [workerServerQueues EXCEPT ![session] = Tail(@)]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, sourceRecords, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress>>

FrameCanReachClient(session) ==
  /\ TerminalPathAvailable
  /\ serverClientQueues[session] # <<>>
  /\ projectionPhase[session] \notin {"paused", "blocked"}
  /\ feedbackPhase[session] = "none"
  /\ Head(serverClientQueues[session]).recordKind = "frame"
  /\ sourceRecords[session] = {Head(serverClientQueues[session])}

FeedbackCanBeValidatedAtServer(session, kind) ==
  /\ TerminalPathAvailable
  /\ feedbackPhase[session] = "at-server"
  /\ feedbackKind[session] = kind
  /\ Cardinality(sourceRecords[session]) = 1
  /\ LET record == CHOOSE value \in sourceRecords[session]: TRUE
     IN record.recordKind = "frame"

\* These actions model external malformed candidates at the Server boundary.
\* Each rejection class is a directly covered exact stutter; the supplied
\* tuple is never installed into the live feedback flight.
RejectWrongFrameAppliedSequence(session, sequence) ==
  /\ Scenario = "projection-feedback"
  /\ FeedbackCanBeValidatedAtServer(session, "frame-applied")
  /\ sequence \in 1..MaxFrameSequence
  /\ LET record == CHOOSE value \in sourceRecords[session]: TRUE
     IN sequence # record.sequence
  /\ UNCHANGED allVars

RejectWrongFrameAppliedGeneration(session, generation) ==
  /\ Scenario = "projection-feedback"
  /\ FeedbackCanBeValidatedAtServer(session, "frame-applied")
  /\ generation \in 1..MaxProjectionGeneration
  /\ LET record == CHOOSE value \in sourceRecords[session]: TRUE
     IN generation # record.generation
  /\ UNCHANGED allVars

RejectWrongFrameAppliedRevision(session, revision) ==
  /\ Scenario = "projection-feedback"
  /\ FeedbackCanBeValidatedAtServer(session, "frame-applied")
  /\ revision \in 0..MaxContentRevision
  /\ LET record == CHOOSE value \in sourceRecords[session]: TRUE
     IN revision # record.revision
  /\ UNCHANGED allVars

RejectWrongNeedSnapshotSequence(session, sequence) ==
  /\ Scenario = "projection-feedback"
  /\ FeedbackCanBeValidatedAtServer(session, "need-snapshot")
  /\ sequence \in 1..MaxFrameSequence
  /\ LET record == CHOOSE value \in sourceRecords[session]: TRUE
     IN sequence # record.sequence
  /\ UNCHANGED allVars

RelayServerToClient(session) ==
  /\ Scenario \in ProjectionScenarios
  /\ TerminalPathAvailable
  /\ serverClientQueues[session] # <<>>
  /\ projectionPhase[session] \notin {"paused", "blocked"}
  /\ LET record == Head(serverClientQueues[session])
     IN
       /\ record.session = session
       /\ sourceRecords[session] = {record}
       /\ serverClientQueues' =
            [serverClientQueues EXCEPT ![session] = Tail(@)]
       /\ hostWorkerQueues' = hostWorkerQueues
       /\ workerServerQueues' = workerServerQueues
       /\ IF record.recordKind = "projection-blocked"
             THEN
               /\ projectionPhase' =
                    [projectionPhase EXCEPT ![session] = "blocked"]
               /\ blockedRevision' =
                    [blockedRevision EXCEPT ![session] = record.revision]
               /\ UNCHANGED <<lastAppliedSequence, feedbackKind,
                    feedbackPhase, feedbackSequence, feedbackAttachment,
                    feedbackGeneration, feedbackRevision>>
             ELSE
               /\ feedbackPhase[session] = "none"
               /\ feedbackKind' =
                    [feedbackKind EXCEPT ![session] = "frame-applied"]
               /\ feedbackPhase' =
                    [feedbackPhase EXCEPT ![session] = "at-server"]
               /\ feedbackSequence' =
                    [feedbackSequence EXCEPT ![session] = record.sequence]
               /\ feedbackAttachment' =
                    [feedbackAttachment EXCEPT
                      ![session] = record.attachment]
               /\ feedbackGeneration' =
                    [feedbackGeneration EXCEPT ![session] = record.generation]
               /\ feedbackRevision' =
                    [feedbackRevision EXCEPT ![session] = record.revision]
               /\ lastAppliedSequence' =
                    [lastAppliedSequence EXCEPT
                      ![session] = record.sequence]
               /\ projectionPhase' =
                    [projectionPhase EXCEPT ![session] = "streaming"]
               /\ UNCHANGED blockedRevision
  /\ UNCHANGED <<vars, attachmentGeneration, sourceRecords,
       nextFrameSequence, hostAppliedSequence, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress>>

ClientNeedsSnapshot(session) ==
  /\ Scenario = "projection-feedback"
  /\ FrameCanReachClient(session)
  /\ LET record == Head(serverClientQueues[session])
     IN
       /\ feedbackKind' =
            [feedbackKind EXCEPT ![session] = "need-snapshot"]
       /\ feedbackPhase' =
            [feedbackPhase EXCEPT ![session] = "at-server"]
       /\ feedbackSequence' =
            [feedbackSequence EXCEPT ![session] = record.sequence]
       /\ feedbackAttachment' =
            [feedbackAttachment EXCEPT ![session] = record.attachment]
       /\ feedbackGeneration' =
            [feedbackGeneration EXCEPT ![session] = 0]
       /\ feedbackRevision' = [feedbackRevision EXCEPT ![session] = 0]
  /\ projectionPhase' =
       [projectionPhase EXCEPT ![session] = "needs-snapshot"]
  /\ hostWorkerQueues' = [hostWorkerQueues EXCEPT ![session] = <<>>]
  /\ workerServerQueues' = [workerServerQueues EXCEPT ![session] = <<>>]
  /\ serverClientQueues' = [serverClientQueues EXCEPT ![session] = <<>>]
  /\ UNCHANGED <<vars, attachmentGeneration, sourceRecords,
       nextFrameSequence, lastAppliedSequence, hostAppliedSequence,
       blockedRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress>>

RelayFeedbackToWorker(session) ==
  /\ Scenario = "projection-feedback"
  /\ TerminalPathAvailable
  /\ feedbackPhase[session] = "at-server"
  /\ feedbackPhase' =
       [feedbackPhase EXCEPT ![session] = "at-worker"]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackSequence,
       feedbackAttachment, feedbackGeneration, feedbackRevision,
       blockedRecoveryRevisionValid, acceptedRequestSequence, usedRequestIds,
       unknownRequestIds, controlPhase, controlOwner, controlAttachment,
       controlKind, controlRequestId, controlSequence, controlRejections,
       workerProgress>>

RelayFeedbackToHost(session) ==
  /\ Scenario = "projection-feedback"
  /\ TerminalPathAvailable
  /\ feedbackPhase[session] = "at-worker"
  /\ feedbackPhase' = [feedbackPhase EXCEPT ![session] = "at-host"]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackSequence,
       feedbackAttachment, feedbackGeneration, feedbackRevision,
       blockedRecoveryRevisionValid, acceptedRequestSequence, usedRequestIds,
       unknownRequestIds, controlPhase, controlOwner, controlAttachment,
       controlKind, controlRequestId, controlSequence, controlRejections,
       workerProgress>>

HostAcceptFrameApplied(session) ==
  /\ Scenario = "projection-feedback"
  /\ TerminalPathAvailable
  /\ session \in viewers
  /\ feedbackPhase[session] = "at-host"
  /\ feedbackKind[session] = "frame-applied"
  /\ Cardinality(sourceRecords[session]) = 1
  /\ LET record == CHOOSE value \in sourceRecords[session]: TRUE
     IN
       /\ record.recordKind = "frame"
       /\ feedbackAttachment[session] = record.attachment
       /\ feedbackSequence[session] = record.sequence
       /\ feedbackGeneration[session] = record.generation
       /\ feedbackRevision[session] = record.revision
  /\ hostAppliedSequence' =
       [hostAppliedSequence EXCEPT ![session] = feedbackSequence[session]]
  /\ sourceRecords' = [sourceRecords EXCEPT ![session] = {}]
  /\ feedbackKind' = [feedbackKind EXCEPT ![session] = "none"]
  /\ feedbackPhase' = [feedbackPhase EXCEPT ![session] = "none"]
  /\ feedbackSequence' = [feedbackSequence EXCEPT ![session] = 0]
  /\ feedbackAttachment' = [feedbackAttachment EXCEPT ![session] = 0]
  /\ feedbackGeneration' = [feedbackGeneration EXCEPT ![session] = 0]
  /\ feedbackRevision' = [feedbackRevision EXCEPT ![session] = 0]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       nextFrameSequence, lastAppliedSequence, blockedRevision,
       blockedRecoveryRevisionValid, acceptedRequestSequence, usedRequestIds,
       unknownRequestIds, controlPhase, controlOwner, controlAttachment,
       controlKind, controlRequestId, controlSequence, controlRejections,
       workerProgress>>

HostAcceptNeedSnapshot(session) ==
  /\ Scenario = "projection-feedback"
  /\ TerminalPathAvailable
  /\ session \in viewers
  /\ feedbackPhase[session] = "at-host"
  /\ feedbackKind[session] = "need-snapshot"
  /\ Cardinality(sourceRecords[session]) = 1
  /\ LET record == CHOOSE value \in sourceRecords[session]: TRUE
     IN
       /\ record.recordKind = "frame"
       /\ feedbackAttachment[session] = record.attachment
       /\ feedbackSequence[session] = record.sequence
  /\ projectionPhase' =
       [projectionPhase EXCEPT ![session] = "needs-snapshot"]
  /\ sourceRecords' = [sourceRecords EXCEPT ![session] = {}]
  /\ feedbackKind' = [feedbackKind EXCEPT ![session] = "none"]
  /\ feedbackPhase' = [feedbackPhase EXCEPT ![session] = "none"]
  /\ feedbackSequence' = [feedbackSequence EXCEPT ![session] = 0]
  /\ feedbackAttachment' = [feedbackAttachment EXCEPT ![session] = 0]
  /\ feedbackGeneration' = [feedbackGeneration EXCEPT ![session] = 0]
  /\ feedbackRevision' = [feedbackRevision EXCEPT ![session] = 0]
  /\ UNCHANGED <<vars, attachmentGeneration, hostWorkerQueues,
       workerServerQueues, serverClientQueues, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       blockedRecoveryRevisionValid, acceptedRequestSequence, usedRequestIds,
       unknownRequestIds, controlPhase, controlOwner, controlAttachment,
       controlKind, controlRequestId, controlSequence, controlRejections,
       workerProgress>>

DropStaleFeedback(session) ==
  /\ Scenario = "reattach-feedback"
  /\ feedbackPhase[session] # "none"
  /\ \/ session \notin viewers
     \/ feedbackAttachment[session] # attachmentGeneration[session]
  /\ feedbackKind' = [feedbackKind EXCEPT ![session] = "none"]
  /\ feedbackPhase' = [feedbackPhase EXCEPT ![session] = "none"]
  /\ feedbackSequence' = [feedbackSequence EXCEPT ![session] = 0]
  /\ feedbackAttachment' = [feedbackAttachment EXCEPT ![session] = 0]
  /\ feedbackGeneration' = [feedbackGeneration EXCEPT ![session] = 0]
  /\ feedbackRevision' = [feedbackRevision EXCEPT ![session] = 0]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress>>

PauseBackpressuredAttachment(session) ==
  /\ Scenario = "backpressure-isolation"
  /\ TerminalPathAvailable
  /\ session \in viewers
  /\ projectionPhase[session] \notin {"paused", "blocked"}
  /\ \/ Len(hostWorkerQueues[session]) = MaxQueueDepth
     \/ Len(workerServerQueues[session]) = MaxQueueDepth
     \/ Len(serverClientQueues[session]) = MaxQueueDepth
  /\ projectionPhase' =
       [projectionPhase EXCEPT ![session] = "paused"]
  /\ hostWorkerQueues' = [hostWorkerQueues EXCEPT ![session] = <<>>]
  /\ workerServerQueues' = [workerServerQueues EXCEPT ![session] = <<>>]
  /\ serverClientQueues' = [serverClientQueues EXCEPT ![session] = <<>>]
  /\ sourceRecords' = [sourceRecords EXCEPT ![session] = {}]
  /\ UNCHANGED <<vars, attachmentGeneration, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress>>

CloseBackpressuredAttachment(client) ==
  /\ Scenario = "backpressure-isolation"
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ projectionPhase[session] = "paused"
       /\ (DetachControllerAttachment(client) \/
           DetachViewerAttachment(client))
       /\ ResetDeliveryFor(session, "detached")
  /\ UNCHANGED <<attachmentGeneration, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, usedRequestIds,
       unknownRequestIds, controlPhase, controlOwner, controlAttachment,
       controlKind, controlRequestId, controlSequence, controlRejections,
       workerProgress>>

DetachRelayController(client) ==
  /\ Scenario \in (ControlScenarios \cup LifecycleScenarios)
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ DetachControllerAttachment(client)
       /\ ResetDeliveryFor(session, "detached")
       /\ IF ControlCouldHaveEscapedServer(session)
             THEN
               /\ controlPhase' = "unknown"
               /\ unknownRequestIds' =
                    unknownRequestIds \cup {controlRequestId}
               /\ UNCHANGED <<controlOwner, controlAttachment,
                    controlKind, controlRequestId, controlSequence>>
             ELSE IF controlOwner = session /\ controlPhase = "at-server"
             THEN
               /\ controlPhase' = "idle"
               /\ ResetControlFields
               /\ unknownRequestIds' = unknownRequestIds
             ELSE
               /\ UNCHANGED <<controlPhase, controlOwner,
                    controlAttachment, controlKind, controlRequestId,
                    controlSequence>>
               /\ unknownRequestIds' = unknownRequestIds
  /\ UNCHANGED <<attachmentGeneration, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, usedRequestIds,
       controlRejections, workerProgress>>

DetachRelayViewer(client) ==
  /\ (Scenario = "reattach-feedback" \/ Scenario \in LifecycleScenarios)
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ (Scenario # "reattach-feedback" \/
             feedbackPhase[session] # "none")
       /\ DetachViewerAttachment(client)
       /\ ResetDeliveryFor(session, "detached")
  /\ UNCHANGED <<attachmentGeneration, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, usedRequestIds,
       unknownRequestIds, controlPhase, controlOwner, controlAttachment,
       controlKind, controlRequestId, controlSequence, controlRejections,
       workerProgress>>

DisconnectRelayClientCore(client) ==
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ \/ DisconnectController(client)
          \/ DisconnectViewer(client)
          \/ DisconnectUnattachedSession(client)
       /\ IF session \in viewers
              THEN ResetDeliveryFor(session, "detached")
              ELSE UNCHANGED <<projectionPhase, hostWorkerQueues,
                     workerServerQueues, serverClientQueues, sourceRecords,
                     nextFrameSequence, lastAppliedSequence,
                     hostAppliedSequence, blockedRevision,
                     acceptedRequestSequence>>
       /\ IF ControlCouldHaveEscapedServer(session)
             THEN
               /\ controlPhase' = "unknown"
               /\ unknownRequestIds' =
                    unknownRequestIds \cup {controlRequestId}
               /\ UNCHANGED <<controlOwner, controlAttachment,
                    controlKind, controlRequestId, controlSequence>>
             ELSE IF controlOwner = session /\ controlPhase = "at-server"
             THEN
               /\ controlPhase' = "idle"
               /\ ResetControlFields
               /\ unknownRequestIds' = unknownRequestIds
             ELSE
               /\ UNCHANGED <<controlPhase, controlOwner,
                    controlAttachment, controlKind, controlRequestId,
                    controlSequence>>
               /\ unknownRequestIds' = unknownRequestIds
  /\ UNCHANGED <<attachmentGeneration, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, usedRequestIds,
       controlRejections, workerProgress>>

DisconnectRelayClient(client) ==
  /\ Scenario = "attachment-lifecycle"
  /\ DisconnectRelayClientCore(client)

DisconnectEscapedControlOwner(client) ==
  /\ Scenario = "failure-interleavings"
  /\ DownstreamFlightIsActive
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ session = controller
       /\ ControlCouldHaveEscapedServer(session)
  /\ DisconnectRelayClientCore(client)

RelayCleanupAny ==
  /\ Scenario \in LifecycleScenarios
  /\ CleanupAny
  /\ UNCHANGED RelayVars

LoseRelayServer ==
  /\ Scenario = "attachment-lifecycle"
  /\ LoseServer
  /\ UNCHANGED RelayVars

LoseRelayWorker ==
  /\ Scenario = "attachment-lifecycle"
  /\ LoseWorker
  /\ UNCHANGED RelayVars

LoseRelayHost ==
  /\ Scenario = "attachment-lifecycle"
  /\ LoseHost
  /\ UNCHANGED RelayVars

LoseRelayServerDuringFlight ==
  /\ Scenario = "failure-interleavings"
  /\ DownstreamFlightIsActive
  /\ controlOwner \in RelaySessionRefs
  /\ ControlCouldHaveEscapedServer(controlOwner)
  /\ LoseServer
  /\ UNCHANGED RelayVars

LoseRelayWorkerDuringFlight ==
  /\ Scenario = "failure-interleavings"
  /\ DownstreamFlightIsActive
  /\ controlOwner \in RelaySessionRefs
  /\ ControlCouldHaveEscapedServer(controlOwner)
  /\ LoseWorker
  /\ UNCHANGED RelayVars

LoseRelayHostDuringFlight ==
  /\ Scenario = "failure-interleavings"
  /\ DownstreamFlightIsActive
  /\ controlOwner \in RelaySessionRefs
  /\ ControlCouldHaveEscapedServer(controlOwner)
  /\ LoseHost
  /\ UNCHANGED RelayVars

AdvanceWorkerProgress ==
  /\ Scenario \in ProjectionScenarios
  /\ workerUp
  /\ workerProgress < MaxWorkerProgress
  /\ workerProgress' = workerProgress + 1
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections>>

RelayAction ==
  \/ \E client \in Clients: OpenRelaySession(client)
  \/ \E client \in Clients: ReconnectRelaySession(client)
  \/ \E client \in Clients: AttachRelayViewer(client)
  \/ \E client \in Clients: ReattachRelayViewer(client)
  \/ \E client \in Clients: AcquireRelayController(client)
  \/ \E client \in Clients, requestId \in RequestIds,
         requestSequence \in 1..MaxRequestSequence:
       SubmitControllerInput(client, requestId, requestSequence)
  \/ \E client \in Clients, requestId \in RequestIds,
         requestSequence \in 1..MaxRequestSequence:
       SubmitControllerResize(client, requestId, requestSequence)
  \/ \E client \in Clients: RejectNonControllerInputRelay(client)
  \/ \E client \in Clients: RejectNonControllerResizeRelay(client)
  \/ \E client \in Clients, requestId \in RequestIds,
         requestSequence \in 1..MaxRequestSequence:
       RejectOutOfOrderControlSequence(client, requestId, requestSequence)
  \/ \E client \in Clients, requestId \in RequestIds:
       RejectUsedControlIdentity(client, requestId)
  \/ ForwardControlToWorker
  \/ CommitInputEffect
  \/ CommitResizeEffect
  \/ ReturnControlResultToServer
  \/ AcceptControlResult
  \/ LoseUnknownControlOutcome
  \/ RetireUnknownControl
  \/ \E session \in RelaySessionRefs,
         generation \in 1..MaxProjectionGeneration,
         revision \in 0..MaxContentRevision:
       HostEmitSnapshot(session, generation, revision)
  \/ \E session \in RelaySessionRefs,
         generation \in 1..MaxProjectionGeneration,
         revision \in 0..MaxContentRevision:
       HostEmitDelta(session, generation, revision)
  \/ \E session \in RelaySessionRefs,
         revision \in 0..MaxContentRevision,
         bytes \in OpaqueFrameBytes:
       HostEmitProjectionBlocked(session, revision, bytes)
  \/ \E session \in RelaySessionRefs,
         generation \in 1..MaxProjectionGeneration,
         revision \in 0..MaxContentRevision:
       HostRecoverBlockedProjection(session, generation, revision)
  \/ \E session \in RelaySessionRefs, revision \in 0..MaxContentRevision:
       RejectBlockedRecoveryWithoutNewRevision(session, revision)
  \/ \E session \in RelaySessionRefs: RelayHostToWorker(session)
  \/ \E session \in RelaySessionRefs: RelayWorkerToServer(session)
  \/ \E session \in RelaySessionRefs, sequence \in 1..MaxFrameSequence:
       RejectWrongFrameAppliedSequence(session, sequence)
  \/ \E session \in RelaySessionRefs,
         generation \in 1..MaxProjectionGeneration:
       RejectWrongFrameAppliedGeneration(session, generation)
  \/ \E session \in RelaySessionRefs, revision \in 0..MaxContentRevision:
       RejectWrongFrameAppliedRevision(session, revision)
  \/ \E session \in RelaySessionRefs, sequence \in 1..MaxFrameSequence:
       RejectWrongNeedSnapshotSequence(session, sequence)
  \/ \E session \in RelaySessionRefs: RelayServerToClient(session)
  \/ \E session \in RelaySessionRefs: ClientNeedsSnapshot(session)
  \/ \E session \in RelaySessionRefs: RelayFeedbackToWorker(session)
  \/ \E session \in RelaySessionRefs: RelayFeedbackToHost(session)
  \/ \E session \in RelaySessionRefs: HostAcceptFrameApplied(session)
  \/ \E session \in RelaySessionRefs: HostAcceptNeedSnapshot(session)
  \/ \E session \in RelaySessionRefs: DropStaleFeedback(session)
  \/ \E session \in RelaySessionRefs: PauseBackpressuredAttachment(session)
  \/ \E client \in Clients: CloseBackpressuredAttachment(client)
  \/ \E client \in Clients: DetachRelayController(client)
  \/ \E client \in Clients: DetachRelayViewer(client)
  \/ \E client \in Clients: DisconnectRelayClient(client)
  \/ \E client \in Clients: DisconnectEscapedControlOwner(client)
  \/ RelayCleanupAny
  \/ LoseRelayServer
  \/ LoseRelayWorker
  \/ LoseRelayHost
  \/ LoseRelayServerDuringFlight
  \/ LoseRelayWorkerDuringFlight
  \/ LoseRelayHostDuringFlight
  \/ AdvanceWorkerProgress

RelayNext ==
  RelayAction

RelaySpec ==
  /\ RelayInit
  /\ [][RelayNext]_allVars
  /\ WF_allVars(AdvanceWorkerProgress)

RelayTypeOK ==
  /\ TypeOK
  /\ attachmentGeneration \in
       [RelaySessionRefs -> 0..MaxAttachmentGeneration]
  /\ projectionPhase \in [RelaySessionRefs -> ProjectionPhases]
  /\ hostWorkerQueues \in [RelaySessionRefs -> Seq(DownstreamRecords)]
  /\ workerServerQueues \in [RelaySessionRefs -> Seq(DownstreamRecords)]
  /\ serverClientQueues \in [RelaySessionRefs -> Seq(DownstreamRecords)]
  /\ sourceRecords \in [RelaySessionRefs -> SUBSET DownstreamRecords]
  /\ nextFrameSequence \in [RelaySessionRefs -> 0..MaxFrameSequence]
  /\ lastAppliedSequence \in [RelaySessionRefs -> 0..MaxFrameSequence]
  /\ hostAppliedSequence \in [RelaySessionRefs -> 0..MaxFrameSequence]
  /\ blockedRevision \in [RelaySessionRefs -> 0..MaxContentRevision]
  /\ feedbackKind \in [RelaySessionRefs -> FeedbackKinds]
  /\ feedbackPhase \in [RelaySessionRefs -> FeedbackPhases]
  /\ feedbackSequence \in [RelaySessionRefs -> 0..MaxFrameSequence]
  /\ feedbackAttachment \in
       [RelaySessionRefs -> 0..MaxAttachmentGeneration]
  /\ feedbackGeneration \in
       [RelaySessionRefs -> 0..MaxProjectionGeneration]
  /\ feedbackRevision \in [RelaySessionRefs -> 0..MaxContentRevision]
  /\ blockedRecoveryRevisionValid \in BOOLEAN
  /\ acceptedRequestSequence \in
       [RelaySessionRefs -> 0..MaxRequestSequence]
  /\ usedRequestIds \subseteq RequestIds
  /\ unknownRequestIds \subseteq usedRequestIds
  /\ controlPhase \in ControlPhases
  /\ controlOwner \in RelaySessionRefs \cup {NoController}
  /\ controlAttachment \in 0..MaxAttachmentGeneration
  /\ controlKind \in ControlKinds
  /\ controlRequestId \in RequestIds \cup {NoRequestId}
  /\ controlSequence \in 0..MaxRequestSequence
  /\ controlRejections \in 0..MaxRequestSequence
  /\ workerProgress \in 0..MaxWorkerProgress

RelayUsesImportedAuthority ==
  /\ AttachedProjectionSessions = viewers
  /\ Cardinality(HostReadAttachments) =
       IF TerminalPathAvailable THEN Cardinality(viewers) ELSE 0
  /\ HostInputAttachments = HostResizeAttachments
  /\ HostInputAttachments \subseteq HostReadAttachments
  /\ Cardinality(HostInputAttachments) <= 1
  /\ (controller = NoController => HostInputAttachments = {})

CurrentRelaySessionsAreRepresented ==
  \A client \in Clients:
    sessionOpen[client] => CurrentSession(client) \in RelaySessionRefs

AttachedSessionsHaveScopedGenerations ==
  \A session \in RelaySessionRefs:
    /\ (session \in viewers => attachmentGeneration[session] > 0)
    /\ (attachmentGeneration[session] = 0 =>
          projectionPhase[session] = "detached")

DownstreamQueuesAreBoundedAndScoped ==
  \A session \in RelaySessionRefs:
    /\ QueueIsBounded(hostWorkerQueues[session])
    /\ QueueIsBounded(workerServerQueues[session])
    /\ QueueIsBounded(serverClientQueues[session])
    /\ QueueIsScoped(hostWorkerQueues[session], session)
    /\ QueueIsScoped(workerServerQueues[session], session)
    /\ QueueIsScoped(serverClientQueues[session], session)

RelayedRecordsPreserveSourceRecord ==
  \A session \in RelaySessionRefs:
    /\ QueuePreservesSourceRecords(hostWorkerQueues[session], session)
    /\ QueuePreservesSourceRecords(workerServerQueues[session], session)
    /\ QueuePreservesSourceRecords(serverClientQueues[session], session)

SourceRecordIsUnique ==
  \A session \in RelaySessionRefs:
    Cardinality(sourceRecords[session]) <= 1

OneDeliveryFlightPerAttachment ==
  \A session \in RelaySessionRefs:
    /\ Len(hostWorkerQueues[session]) +
         Len(workerServerQueues[session]) +
         Len(serverClientQueues[session]) <= 1
    /\ (feedbackPhase[session] # "none" =>
          EveryQueueIsEmptyFor(session))

DetachedSessionsRetainNoDownstreamRecords ==
  \A session \in RelaySessionRefs:
    projectionPhase[session] = "detached" => EveryQueueIsEmptyFor(session)

FeedbackHasExactAttachmentScope ==
  \A session \in RelaySessionRefs:
    \/ /\ feedbackPhase[session] = "none"
       /\ FeedbackIsEmptyFor(session)
    \/ /\ feedbackPhase[session] # "none"
       /\ feedbackKind[session] \in {
            "frame-applied", "need-snapshot"
          }
       /\ feedbackSequence[session] \in 1..MaxFrameSequence
       /\ feedbackAttachment[session] \in
             1..MaxAttachmentGeneration
       /\ IF feedbackKind[session] = "frame-applied"
             THEN
               /\ feedbackGeneration[session] \in
                    1..MaxProjectionGeneration
               /\ feedbackRevision[session] \in 0..MaxContentRevision
             ELSE
               /\ feedbackGeneration[session] = 0
               /\ feedbackRevision[session] = 0

FeedbackHasExactFlightIdentity ==
  \A session \in RelaySessionRefs:
    /\ session \in viewers
    /\ feedbackPhase[session] # "none"
    /\ feedbackAttachment[session] = attachmentGeneration[session]
    =>
      /\ Cardinality(sourceRecords[session]) = 1
      /\ LET record == CHOOSE value \in sourceRecords[session]: TRUE
         IN
           /\ record.recordKind = "frame"
           /\ record.session = session
           /\ feedbackAttachment[session] = record.attachment
           /\ feedbackSequence[session] = record.sequence
           /\ IF feedbackKind[session] = "frame-applied"
                 THEN
                   /\ feedbackGeneration[session] = record.generation
                   /\ feedbackRevision[session] = record.revision
                 ELSE
                   /\ feedbackGeneration[session] = 0
                   /\ feedbackRevision[session] = 0

ControlRequestShapeIsStrict ==
  \/ /\ controlPhase = "idle"
     /\ controlOwner = NoController
     /\ controlAttachment = 0
     /\ controlKind = "none"
     /\ controlRequestId = NoRequestId
     /\ controlSequence = 0
  \/ /\ controlPhase # "idle"
     /\ controlOwner \in RelaySessionRefs
     /\ controlAttachment \in 1..MaxAttachmentGeneration
     /\ controlKind \in {"input", "resize"}
     /\ controlRequestId \in usedRequestIds
     /\ controlSequence \in 1..MaxRequestSequence

PendingControlRefinesCombinedController ==
  TerminalPathAvailable /\
  controlPhase \in {"at-server", "at-worker"} =>
    /\ controlOwner = controller
    /\ controlAttachment = attachmentGeneration[controller]
    /\ controlSequence = acceptedRequestSequence[controller]
    /\ controller \in viewers

UnknownControlIsNeverQueuedForReplay ==
  controlPhase = "unknown" =>
    /\ controlRequestId \in unknownRequestIds
    /\ controlPhase \notin {"at-server", "at-worker"}

AppliedSequencesNeverExceedEmission ==
  \A session \in RelaySessionRefs:
    /\ lastAppliedSequence[session] <= nextFrameSequence[session]
    /\ hostAppliedSequence[session] <= lastAppliedSequence[session]

MismatchedFeedbackRejection ==
  \/ \E session \in RelaySessionRefs, sequence \in 1..MaxFrameSequence:
       RejectWrongFrameAppliedSequence(session, sequence)
  \/ \E session \in RelaySessionRefs,
         generation \in 1..MaxProjectionGeneration:
       RejectWrongFrameAppliedGeneration(session, generation)
  \/ \E session \in RelaySessionRefs, revision \in 0..MaxContentRevision:
       RejectWrongFrameAppliedRevision(session, revision)
  \/ \E session \in RelaySessionRefs, sequence \in 1..MaxFrameSequence:
       RejectWrongNeedSnapshotSequence(session, sequence)

MismatchedFeedbackIsEffectFree ==
  [][MismatchedFeedbackRejection =>
      UNCHANGED RelaySemanticVars]_allVars

BlockedRecoveryRejection ==
  \E session \in RelaySessionRefs, revision \in 0..MaxContentRevision:
    RejectBlockedRecoveryWithoutNewRevision(session, revision)

BlockedRecoveryRejectionIsEffectFree ==
  [][BlockedRecoveryRejection =>
      UNCHANGED RelaySemanticVars]_allVars

BlockedRecoveryRequiresNewerRevision ==
  blockedRecoveryRevisionValid

ClientDetachRetainsTerminal ==
  lastAction \in {
    "detach-controller", "detach-viewer",
    "disconnect-controller", "disconnect-viewer",
    "disconnect-unattached-session"
  } => terminalState = priorTerminalState

WorkerProgressRemainsEnabled ==
  Scenario \in ProjectionScenarios /\
  workerUp /\ workerProgress < MaxWorkerProgress =>
    ENABLED AdvanceWorkerProgress

WorkerProgressIsQueueIndependent ==
  [](Scenario \in ProjectionScenarios /\
     workerUp /\ workerProgress < MaxWorkerProgress =>
       <> (workerProgress = MaxWorkerProgress \/ ~workerUp))

PausedClientDoesNotBlockPeerDelivery ==
  Scenario = "backpressure-isolation" =>
  \A paused, peer \in RelaySessionRefs:
    /\ paused # peer
    /\ projectionPhase[paused] = "paused"
    /\ TerminalPathAvailable
    =>
      /\ (hostWorkerQueues[peer] # <<>> /\
            Len(workerServerQueues[peer]) < MaxQueueDepth =>
              ENABLED RelayHostToWorker(peer))
      /\ (workerServerQueues[peer] # <<>> /\
            Len(serverClientQueues[peer]) < MaxQueueDepth =>
              ENABLED RelayWorkerToServer(peer))
      /\ (serverClientQueues[peer] # <<>> /\
            projectionPhase[peer] \notin {"paused", "blocked"} /\
            (Head(serverClientQueues[peer]).recordKind =
               "projection-blocked" \/ feedbackPhase[peer] = "none") =>
              ENABLED RelayServerToClient(peer))

=============================================================================
