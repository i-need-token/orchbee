------------------------ MODULE LocalTerminalRelay ------------------------
EXTENDS LocalTerminalClientAuthority, Sequences

\* Transport refinement of LocalTerminalClientAuthority for one retained
\* terminal on the first Host -> Worker -> Server -> Client relay. The
\* imported viewers and controller are the only product authority: this
\* module adds attachment epochs, bounded per-attachment queues, opaque OBTF
\* records, exact feedback, and one abstract control-request flight. It does
\* not model a wire encoding, VT, runtime implementation, takeover,
\* or release resource budget.
\* The "history-relay" scenario adds the kind-53 HistoryChanged lane: each
\* viewer carries an independent bounded event FIFO beside the frame record
\* slot, a two-source worker poll alternates the sources through an explicit
\* next-preference, and HistoryObserved rides its own acknowledgement flight
\* back to the Host without touching the frame flight. Saturation closes only
\* the saturated viewer; retained bytes are accounted exactly once.

CONSTANTS RequestIds, FirstOpaqueFrameBytes, SecondOpaqueFrameBytes,
  NoOpaqueFrame,
  MaxAttachmentGeneration, MaxRequestSequence, MaxFrameSequence,
  MaxProjectionGeneration, MaxContentRevision,
  MaxQueueDepth, MaxWorkerProgress, MaxHistorySequence, MaxHistoryQueueDepth,
  Scenario

ProjectionScenarios == {
  "projection-feedback", "backpressure-isolation", "reattach-feedback",
  "failure-interleavings", "history-relay"
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

\* Kind-53 events are opaque to the relay: attachment-local sequence, the
\* attachment epoch they were emitted under, and nothing else is consulted.
HistoryRecords == [
  recordKind : {"history"},
  session : RelaySessionRefs,
  attachment : 1..MaxAttachmentGeneration,
  sequence : 1..MaxHistorySequence
]

HistoryRecord(session, attachment, sequence) == [
  recordKind |-> "history",
  session |-> session,
  attachment |-> attachment,
  sequence |-> sequence
]

NoHistoryEvent == [sequence |-> 0, attachment |-> 0]
HistoryDeliveries == [sequence : 0..MaxHistorySequence,
                      attachment : 0..MaxAttachmentGeneration]
HistoryAckPhases == {"none", "at-server", "at-worker", "at-host"}
PollSources == {"record", "history"}

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
  workerProgress,
  historySequence,
  hostHistoryFlight,
  hostWorkerHistory,
  workerHistoryQueues,
  serverClientHistory,
  deliveredHistory,
  historyAckPhase,
  historyAckSequence,
  historyAckAttachment,
  workerPreference,
  recordOvertakes,
  historyOvertakes,
  retainedUnits

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
  workerProgress,
  historySequence,
  hostHistoryFlight,
  hostWorkerHistory,
  workerHistoryQueues,
  serverClientHistory,
  deliveredHistory,
  historyAckPhase,
  historyAckSequence,
  historyAckAttachment,
  workerPreference,
  recordOvertakes,
  historyOvertakes,
  retainedUnits
>>

allVars == <<vars, RelayVars>>

\* The complete history-lane state: emission counters, the host flight, the
\* three per-session queues, the delivered/acknowledgement echo, the worker's
\* poll preference and overtake counters, and the shared retained-unit budget.
HistoryVars == <<
  historySequence,
  hostHistoryFlight,
  hostWorkerHistory,
  workerHistoryQueues,
  serverClientHistory,
  deliveredHistory,
  historyAckPhase,
  historyAckSequence,
  historyAckAttachment,
  workerPreference,
  recordOvertakes,
  historyOvertakes,
  retainedUnits
>>

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
  workerProgress,
  historySequence,
  hostHistoryFlight,
  hostWorkerHistory,
  workerHistoryQueues,
  serverClientHistory,
  deliveredHistory,
  historyAckPhase,
  historyAckSequence,
  historyAckAttachment,
  workerPreference,
  recordOvertakes,
  historyOvertakes,
  retainedUnits
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
  /\ MaxHistorySequence >= 2
  /\ MaxHistoryQueueDepth >= 1
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

\* The history lane is scoped, bounded, and independently accounted: queued
\* events always belong to the session's current attachment epoch, the FIFO
\* never exceeds its depth, and the worker's retained units equal the record
\* slot plus the queued events exactly.
HistoryQueueIsBounded(queue) == Len(queue) <= MaxHistoryQueueDepth

HistoryQueueIsScoped(queue, session) ==
  \A index \in 1..Len(queue):
    /\ queue[index] \in HistoryRecords
    /\ queue[index].session = session
    /\ queue[index].attachment = attachmentGeneration[session]

HistoryStateIsEmptyFor(session) ==
  /\ hostWorkerHistory[session] = <<>>
  /\ workerHistoryQueues[session] = <<>>
  /\ serverClientHistory[session] = <<>>
  /\ deliveredHistory[session] = NoHistoryEvent
  /\ historyAckPhase[session] = "none"
  /\ historyAckSequence[session] = 0
  /\ historyAckAttachment[session] = 0
  /\ hostHistoryFlight[session] = 0
  /\ historySequence[session] = 0
  /\ retainedUnits[session] = 0

ResetHistoryFor(session) ==
  /\ historySequence' = [historySequence EXCEPT ![session] = 0]
  /\ hostHistoryFlight' = [hostHistoryFlight EXCEPT ![session] = 0]
  /\ hostWorkerHistory' = [hostWorkerHistory EXCEPT ![session] = <<>>]
  /\ workerHistoryQueues' = [workerHistoryQueues EXCEPT ![session] = <<>>]
  /\ serverClientHistory' = [serverClientHistory EXCEPT ![session] = <<>>]
  /\ deliveredHistory' = [deliveredHistory EXCEPT ![session] = NoHistoryEvent]
  /\ historyAckPhase' = [historyAckPhase EXCEPT ![session] = "none"]
  /\ historyAckSequence' = [historyAckSequence EXCEPT ![session] = 0]
  /\ historyAckAttachment' = [historyAckAttachment EXCEPT ![session] = 0]
  /\ workerPreference' = [workerPreference EXCEPT ![session] = "record"]
  /\ recordOvertakes' = [recordOvertakes EXCEPT ![session] = 0]
  /\ historyOvertakes' = [historyOvertakes EXCEPT ![session] = 0]
  /\ retainedUnits' = [retainedUnits EXCEPT ![session] = 0]

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
  \* Attachment teardown also retires the history lane: queued events, the
  \* acknowledgement flight, the poll preference, and every retained byte are
  \* released exactly once with the viewer.
  /\ ResetHistoryFor(session)

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
  /\ historySequence = [session \in RelaySessionRefs |-> 0]
  /\ hostHistoryFlight = [session \in RelaySessionRefs |-> 0]
  /\ hostWorkerHistory = [session \in RelaySessionRefs |-> <<>>]
  /\ workerHistoryQueues = [session \in RelaySessionRefs |-> <<>>]
  /\ serverClientHistory = [session \in RelaySessionRefs |-> <<>>]
  /\ deliveredHistory = [session \in RelaySessionRefs |-> NoHistoryEvent]
  /\ historyAckPhase = [session \in RelaySessionRefs |-> "none"]
  /\ historyAckSequence = [session \in RelaySessionRefs |-> 0]
  /\ historyAckAttachment = [session \in RelaySessionRefs |-> 0]
  /\ workerPreference = [session \in RelaySessionRefs |-> "record"]
  /\ recordOvertakes = [session \in RelaySessionRefs |-> 0]
  /\ historyOvertakes = [session \in RelaySessionRefs |-> 0]
  /\ retainedUnits = [session \in RelaySessionRefs |-> 0]

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
       controlRequestId, controlSequence, controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRequestId, controlSequence, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRequestId, controlSequence, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRequestId, controlSequence, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRequestId, controlSequence, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlSequence, controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlSequence, controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlSequence, controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlSequence, controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       controlRequestId, controlSequence, controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
  /\ retainedUnits' =
       [retainedUnits EXCEPT ![session] = @ + 1]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       serverClientQueues, sourceRecords, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes>>

\* The worker poll is a two-source arbitration: the frame record slot and the
\* history event FIFO. The preferred source wins when both are pending; a
\* single pending source is always served once its destination has room; a
\* completed take flips the preference to the other source, so a continuously
\* pending head is overtaken by the other source at most once. Each source
\* additionally requires room in its own destination queue — a full
\* destination stalls the abstract transfer and never diverts the poll to the
\* other source. That stall abstracts a later relay hop and is not an input
\* to the real preference.
RelayWorkerToServer(session) ==
  /\ Scenario \in ProjectionScenarios
  /\ TerminalPathAvailable
  /\ workerServerQueues[session] # <<>>
  /\ Len(serverClientQueues[session]) < MaxQueueDepth
  /\ \/ workerPreference[session] = "record"
     \/ workerHistoryQueues[session] = <<>>
  /\ serverClientQueues' =
       [serverClientQueues EXCEPT
         ![session] = Append(@, Head(workerServerQueues[session]))]
  /\ workerServerQueues' =
       [workerServerQueues EXCEPT ![session] = Tail(@)]
  /\ workerPreference' =
       [workerPreference EXCEPT ![session] = "history"]
  /\ historyOvertakes' =
       [historyOvertakes EXCEPT ![session] =
         IF workerHistoryQueues[session] # <<>> THEN @ + 1 ELSE 0]
  /\ recordOvertakes' = [recordOvertakes EXCEPT ![session] = 0]
  /\ retainedUnits' =
       [retainedUnits EXCEPT ![session] = @ - 1]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, sourceRecords, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment>>

\* A Host HistoryChanged event for an attached viewer. Emission carries the
\* attachment-local sequence and supersedes the Host's pending observation
\* flight; stale acknowledgements of superseded events stay idempotent.
HostEmitHistoryChanged(session) ==
  /\ Scenario = "history-relay"
  /\ TerminalPathAvailable
  /\ session \in viewers
  /\ historySequence[session] < MaxHistorySequence
  /\ Len(hostWorkerHistory[session]) < MaxHistoryQueueDepth
  /\ LET sequence == historySequence[session] + 1
         event == HistoryRecord(
           session,
           attachmentGeneration[session],
           sequence)
     IN
       /\ hostWorkerHistory' =
            [hostWorkerHistory EXCEPT ![session] = Append(@, event)]
       /\ historySequence' =
            [historySequence EXCEPT ![session] = sequence]
       /\ hostHistoryFlight' =
            [hostHistoryFlight EXCEPT ![session] = sequence]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress, workerHistoryQueues,
       serverClientHistory, deliveredHistory, historyAckPhase,
       historyAckSequence, historyAckAttachment, workerPreference,
       recordOvertakes, historyOvertakes, retainedUnits>>

\* The worker's per-viewer event FIFO keeps original record order; depth
\* saturation closes only that viewer through the ordinary detach path.
RelayHistoryToWorker(session) ==
  /\ Scenario = "history-relay"
  /\ TerminalPathAvailable
  /\ hostWorkerHistory[session] # <<>>
  /\ Len(workerHistoryQueues[session]) < MaxHistoryQueueDepth
  /\ workerHistoryQueues' =
       [workerHistoryQueues EXCEPT
         ![session] = Append(@, Head(hostWorkerHistory[session]))]
  /\ hostWorkerHistory' =
       [hostWorkerHistory EXCEPT ![session] = Tail(@)]
  /\ retainedUnits' =
       [retainedUnits EXCEPT ![session] = @ + 1]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, workerServerQueues, serverClientQueues,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress, historySequence, hostHistoryFlight,
       serverClientHistory, deliveredHistory, historyAckPhase,
       historyAckSequence, historyAckAttachment, workerPreference,
       recordOvertakes, historyOvertakes>>

HistoryOverflowClose(session) ==
  /\ Scenario = "history-relay"
  /\ TerminalPathAvailable
  /\ hostWorkerHistory[session] # <<>>
  /\ Len(workerHistoryQueues[session]) = MaxHistoryQueueDepth
  /\ \E client \in Clients:
       /\ sessionOpen[client]
       /\ CurrentSession(client) = session
       /\ DetachViewerAttachment(client)
  /\ ResetDeliveryFor(session, "detached")
  /\ UNCHANGED <<attachmentGeneration, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, usedRequestIds,
       unknownRequestIds, controlPhase, controlOwner, controlAttachment,
       controlKind, controlRequestId, controlSequence, controlRejections,
       workerProgress>>

\* The history half of the two-source arbitration documented at
\* RelayWorkerToServer: the preference or an empty record queue admits the
\* take, the event's own destination still requires room, and the frame
\* queue's state never diverts the poll between sources.
WorkerPollHistory(session) ==
  /\ Scenario = "history-relay"
  /\ TerminalPathAvailable
  /\ workerHistoryQueues[session] # <<>>
  /\ Len(serverClientHistory[session]) < MaxHistoryQueueDepth
  /\ \/ workerPreference[session] = "history"
     \/ workerServerQueues[session] = <<>>
  /\ serverClientHistory' =
       [serverClientHistory EXCEPT
         ![session] = Append(@, Head(workerHistoryQueues[session]))]
  /\ workerHistoryQueues' =
       [workerHistoryQueues EXCEPT ![session] = Tail(@)]
  /\ workerPreference' =
       [workerPreference EXCEPT ![session] = "record"]
  /\ recordOvertakes' =
       [recordOvertakes EXCEPT ![session] =
         IF workerServerQueues[session] # <<>> THEN @ + 1 ELSE 0]
  /\ historyOvertakes' = [historyOvertakes EXCEPT ![session] = 0]
  /\ retainedUnits' =
       [retainedUnits EXCEPT ![session] = @ - 1]
  \* History delivery never touches the frame flight: delivered flags, frame
  \* sequences, projection phase, and the feedback lane are all unchanged.
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, hostWorkerHistory, workerServerQueues,
       serverClientQueues, sourceRecords, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress, historySequence, hostHistoryFlight, deliveredHistory,
       historyAckPhase, historyAckSequence, historyAckAttachment>>

\* The client applies each delivered event in order and answers exactly that
\* event through its own acknowledgement flight; no projection-phase gate and
\* no correlation applies to the uncorrelated event record.
RelayHistoryToClient(session) ==
  /\ Scenario = "history-relay"
  /\ TerminalPathAvailable
  /\ serverClientHistory[session] # <<>>
  /\ historyAckPhase[session] = "none"
  /\ LET event == Head(serverClientHistory[session])
     IN
       /\ event.session = session
       /\ event.attachment = attachmentGeneration[session]
       /\ serverClientHistory' =
            [serverClientHistory EXCEPT ![session] = Tail(@)]
       /\ deliveredHistory' =
            [deliveredHistory EXCEPT ![session] =
              [sequence |-> event.sequence, attachment |-> event.attachment]]
       /\ historyAckPhase' =
            [historyAckPhase EXCEPT ![session] = "at-server"]
       /\ historyAckSequence' =
            [historyAckSequence EXCEPT ![session] = event.sequence]
       /\ historyAckAttachment' =
            [historyAckAttachment EXCEPT ![session] = event.attachment]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, hostWorkerHistory, workerServerQueues,
       serverClientQueues, workerHistoryQueues, sourceRecords,
       nextFrameSequence, lastAppliedSequence, hostAppliedSequence,
       blockedRevision, feedbackKind, feedbackPhase, feedbackSequence,
       feedbackAttachment, feedbackGeneration, feedbackRevision,
       blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress, historySequence, hostHistoryFlight,
       workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

RelayHistoryAckToWorker(session) ==
  /\ Scenario = "history-relay"
  /\ TerminalPathAvailable
  /\ historyAckPhase[session] = "at-server"
  /\ historyAckPhase' =
       [historyAckPhase EXCEPT ![session] = "at-worker"]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, hostWorkerHistory, workerServerQueues,
       serverClientQueues, workerHistoryQueues, serverClientHistory,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress, historySequence, hostHistoryFlight,
       deliveredHistory, historyAckSequence, historyAckAttachment,
       workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

RelayHistoryAckToHost(session) ==
  /\ Scenario = "history-relay"
  /\ TerminalPathAvailable
  /\ historyAckPhase[session] = "at-worker"
  /\ historyAckPhase' =
       [historyAckPhase EXCEPT ![session] = "at-host"]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, hostWorkerHistory, workerServerQueues,
       serverClientQueues, workerHistoryQueues, serverClientHistory,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress, historySequence, hostHistoryFlight,
       deliveredHistory, historyAckSequence, historyAckAttachment,
       workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

\* The Host clears only the flight whose sequence and attachment the
\* acknowledgement echoes exactly; anything older is an idempotent no-op.
HostAcceptHistoryObserved(session) ==
  /\ Scenario = "history-relay"
  /\ TerminalPathAvailable
  /\ historyAckPhase[session] = "at-host"
  /\ historyAckSequence[session] = hostHistoryFlight[session]
  /\ historyAckAttachment[session] = attachmentGeneration[session]
  /\ hostHistoryFlight' = [hostHistoryFlight EXCEPT ![session] = 0]
  /\ historyAckPhase' = [historyAckPhase EXCEPT ![session] = "none"]
  /\ historyAckSequence' = [historyAckSequence EXCEPT ![session] = 0]
  /\ historyAckAttachment' = [historyAckAttachment EXCEPT ![session] = 0]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, hostWorkerHistory, workerServerQueues,
       serverClientQueues, workerHistoryQueues, serverClientHistory,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress, historySequence, deliveredHistory,
       workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

HostDropStaleHistoryObserved(session) ==
  /\ Scenario = "history-relay"
  /\ TerminalPathAvailable
  /\ historyAckPhase[session] = "at-host"
  /\ \/ historyAckSequence[session] # hostHistoryFlight[session]
     \/ historyAckAttachment[session] # attachmentGeneration[session]
  /\ historyAckPhase' = [historyAckPhase EXCEPT ![session] = "none"]
  /\ historyAckSequence' = [historyAckSequence EXCEPT ![session] = 0]
  /\ historyAckAttachment' = [historyAckAttachment EXCEPT ![session] = 0]
  /\ UNCHANGED <<vars, attachmentGeneration, projectionPhase,
       hostWorkerQueues, hostWorkerHistory, workerServerQueues,
       serverClientQueues, workerHistoryQueues, serverClientHistory,
       sourceRecords, nextFrameSequence, lastAppliedSequence,
       hostAppliedSequence, blockedRevision, feedbackKind, feedbackPhase,
       feedbackSequence, feedbackAttachment, feedbackGeneration,
       feedbackRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress, historySequence, hostHistoryFlight,
       deliveredHistory, workerPreference, recordOvertakes, historyOvertakes,
       retainedUnits>>

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
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
  \* A snapshot resync drops queued frames only: the event FIFO survives and
  \* its retained units stay accounted.
  /\ retainedUnits' =
       [retainedUnits EXCEPT ![session] = Len(workerHistoryQueues[session])]
  /\ UNCHANGED <<vars, attachmentGeneration, sourceRecords,
       nextFrameSequence, lastAppliedSequence, hostAppliedSequence,
       blockedRevision, blockedRecoveryRevisionValid, acceptedRequestSequence,
       usedRequestIds, unknownRequestIds, controlPhase, controlOwner,
       controlAttachment, controlKind, controlRequestId, controlSequence,
       controlRejections, workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes>>

RelayFeedbackToWorker(session) ==
  /\ Scenario \in {"projection-feedback", "history-relay"}
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
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

RelayFeedbackToHost(session) ==
  /\ Scenario \in {"projection-feedback", "history-relay"}
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
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

HostAcceptFrameApplied(session) ==
  /\ Scenario \in {"projection-feedback", "history-relay"}
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
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes, retainedUnits>>

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
  \* A projection pause drops queued frames only: the history lane keeps
  \* flowing and its retained units stay accounted.
  /\ retainedUnits' =
       [retainedUnits EXCEPT ![session] = Len(workerHistoryQueues[session])]
  /\ UNCHANGED <<vars, attachmentGeneration, nextFrameSequence,
       lastAppliedSequence, hostAppliedSequence, blockedRevision,
       feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
       feedbackGeneration, feedbackRevision, blockedRecoveryRevisionValid,
       acceptedRequestSequence, usedRequestIds, unknownRequestIds,
       controlPhase, controlOwner, controlAttachment, controlKind,
       controlRequestId, controlSequence, controlRejections,
       workerProgress,
       historySequence, hostHistoryFlight, hostWorkerHistory, workerHistoryQueues, serverClientHistory, deliveredHistory, historyAckPhase, historyAckSequence, historyAckAttachment, workerPreference, recordOvertakes, historyOvertakes>>

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
              ELSE /\ UNCHANGED <<projectionPhase, hostWorkerQueues,
                     workerServerQueues, serverClientQueues, sourceRecords,
                     nextFrameSequence, lastAppliedSequence,
                     hostAppliedSequence, blockedRevision,
                     acceptedRequestSequence>>
                   /\ UNCHANGED HistoryVars
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
  /\ UNCHANGED HistoryVars

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
  \/ \E session \in RelaySessionRefs: HostEmitHistoryChanged(session)
  \/ \E session \in RelaySessionRefs: RelayHistoryToWorker(session)
  \/ \E session \in RelaySessionRefs: HistoryOverflowClose(session)
  \/ \E session \in RelaySessionRefs: WorkerPollHistory(session)
  \/ \E session \in RelaySessionRefs: RelayHistoryToClient(session)
  \/ \E session \in RelaySessionRefs: RelayHistoryAckToWorker(session)
  \/ \E session \in RelaySessionRefs: RelayHistoryAckToHost(session)
  \/ \E session \in RelaySessionRefs: HostAcceptHistoryObserved(session)
  \/ \E session \in RelaySessionRefs: HostDropStaleHistoryObserved(session)
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
  /\ historySequence \in [RelaySessionRefs -> 0..MaxHistorySequence]
  /\ hostHistoryFlight \in [RelaySessionRefs -> 0..MaxHistorySequence]
  /\ hostWorkerHistory \in [RelaySessionRefs -> Seq(HistoryRecords)]
  /\ workerHistoryQueues \in [RelaySessionRefs -> Seq(HistoryRecords)]
  /\ serverClientHistory \in [RelaySessionRefs -> Seq(HistoryRecords)]
  /\ deliveredHistory \in [RelaySessionRefs -> HistoryDeliveries]
  /\ historyAckPhase \in [RelaySessionRefs -> HistoryAckPhases]
  /\ historyAckSequence \in [RelaySessionRefs -> 0..MaxHistorySequence]
  /\ historyAckAttachment \in
       [RelaySessionRefs -> 0..MaxAttachmentGeneration]
  /\ workerPreference \in [RelaySessionRefs -> PollSources]
  /\ recordOvertakes \in [RelaySessionRefs -> 0..2]
  /\ historyOvertakes \in [RelaySessionRefs -> 0..2]
  /\ retainedUnits \in
       [RelaySessionRefs -> 0..(MaxQueueDepth + MaxHistoryQueueDepth)]

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

HistoryQueuesAreBoundedAndScoped ==
  \A session \in RelaySessionRefs:
    /\ HistoryQueueIsBounded(hostWorkerHistory[session])
    /\ HistoryQueueIsBounded(workerHistoryQueues[session])
    /\ HistoryQueueIsBounded(serverClientHistory[session])
    /\ HistoryQueueIsScoped(hostWorkerHistory[session], session)
    /\ HistoryQueueIsScoped(workerHistoryQueues[session], session)
    /\ HistoryQueueIsScoped(serverClientHistory[session], session)

\* A continuously pending source head is overtaken by the other source at
\* most once: the bounded-overtake counters are mechanism state, not a
\* fairness axiom over poll timing.
PollOvertakeIsBounded ==
  \A session \in RelaySessionRefs:
    /\ recordOvertakes[session] <= 1
    /\ historyOvertakes[session] <= 1

RetainedUnitsMatchQueues ==
  \A session \in RelaySessionRefs:
    retainedUnits[session] =
      Len(workerServerQueues[session]) + Len(workerHistoryQueues[session])

DetachedSessionsRetainNoHistoryState ==
  \A session \in RelaySessionRefs:
    projectionPhase[session] = "detached" => HistoryStateIsEmptyFor(session)

\* The acknowledgement in flight always echoes the last delivered event
\* exactly; nothing is acknowledged that was not applied.
HistoryObservedEchoesDelivery ==
  \A session \in RelaySessionRefs:
    historyAckPhase[session] # "none" =>
      /\ deliveredHistory[session].sequence \in 1..MaxHistorySequence
      /\ historyAckSequence[session] = deliveredHistory[session].sequence
      /\ historyAckAttachment[session] =
           deliveredHistory[session].attachment
      /\ deliveredHistory[session].attachment =
           attachmentGeneration[session]

HistorySequencesStayInsideEmission ==
  \A session \in RelaySessionRefs:
    /\ hostHistoryFlight[session] <= historySequence[session]
    /\ deliveredHistory[session].sequence <= historySequence[session]
    /\ historyAckSequence[session] <= historySequence[session]

HistoryDelivery ==
  \E session \in RelaySessionRefs:
    WorkerPollHistory(session) \/ RelayHistoryToClient(session)

\* Serving the history source never disturbs the frame lane: record queues,
\* delivery marks, sequences, projection phase, and feedback are untouched.
HistoryDeliveryLeavesFrameFlightUnchanged ==
  [][HistoryDelivery =>
      UNCHANGED <<projectionPhase, hostWorkerQueues, workerServerQueues,
        serverClientQueues, sourceRecords, nextFrameSequence,
        lastAppliedSequence, hostAppliedSequence, blockedRevision,
        feedbackKind, feedbackPhase, feedbackSequence, feedbackAttachment,
        feedbackGeneration, feedbackRevision>>]_allVars

\* Saturation retires only the saturated viewer: sibling sessions' record
\* queues, history queues, preference, and retained accounting are untouched.
HistoryOverflowCloseIsSessionLocal ==
  [][\A session, other \in RelaySessionRefs:
       HistoryOverflowClose(session) /\ session # other =>
        /\ hostWorkerQueues'[other] = hostWorkerQueues[other]
        /\ workerServerQueues'[other] = workerServerQueues[other]
        /\ serverClientQueues'[other] = serverClientQueues[other]
        /\ hostWorkerHistory'[other] = hostWorkerHistory[other]
        /\ workerHistoryQueues'[other] = workerHistoryQueues[other]
        /\ serverClientHistory'[other] = serverClientHistory[other]
        /\ workerPreference'[other] = workerPreference[other]
        /\ retainedUnits'[other] = retainedUnits[other]]_allVars

=============================================================================
