--------------------------- MODULE AgentRuntime ---------------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS
  Scenario,
  MaxRuntimeRevision,
  MaxSequence,
  MaxRequestVersion,
  MaxTransportRevision

ReducerScenario == "reducer"
TransportScenario == "transport-order"
Scenarios == {ReducerScenario, TransportScenario}

Sessions == {"session-1", "session-2"}
Clients == {"client-1", "client-2"}
NoSession == "no-session"

Lifecycles == {"unobserved", "live", "ended"}
TurnStates == {"unknown", "active", "idle"}
Outcomes == {"none", "succeeded", "failed", "canceled", "interrupted"}
FreshnessStates == {"fresh", "stale", "disconnected"}
VisibilityStates == {"detached", "unknown", "hidden", "visible"}
Generations == 0..1

SessionRecordType == [
  lifecycle: Lifecycles,
  turn: TurnStates,
  pending: BOOLEAN,
  outcome: Outcomes,
  freshness: FreshnessStates,
  generation: Generations,
  sequence: 0..MaxSequence,
  requestVersion: 0..MaxRequestVersion,
  activity: BOOLEAN
]

EmptySession == [
  lifecycle |-> "unobserved",
  turn |-> "unknown",
  pending |-> FALSE,
  outcome |-> "none",
  freshness |-> "disconnected",
  generation |-> 0,
  sequence |-> 0,
  requestVersion |-> 0,
  activity |-> FALSE
]

Actions == {
  "init",
  "correlation-half-received",
  "trusted-half-received",
  "joined-observation-accepted",
  "session-observed",
  "presentation-attached",
  "client-hidden",
  "client-visible",
  "turn-started",
  "turn-activity-changed",
  "limited-activity-observed",
  "attention-requested",
  "client-acknowledged",
  "attention-resolved",
  "session-idle-observed",
  "turn-succeeded",
  "turn-failed",
  "turn-canceled",
  "turn-interrupted",
  "presentation-detached",
  "observation-became-stale",
  "observation-reconnected",
  "repeated-sequence-rejected",
  "stale-sequence-rejected",
  "wrong-generation-rejected",
  "wrong-identity-rejected",
  "invalid-observation-rejected",
  "session-ended",
  "snapshot-published",
  "client-observed-publication",
  "renderer-reconnected",
  "eventful-transition-applied",
  "event-snapshot-delivered",
  "semantic-event-delivered"
}

CanonicalActionNames == {
  "session-observed",
  "presentation-attached",
  "client-hidden",
  "client-visible",
  "turn-started",
  "turn-activity-changed",
  "limited-activity-observed",
  "attention-requested",
  "attention-resolved",
  "session-idle-observed",
  "turn-succeeded",
  "turn-failed",
  "turn-canceled",
  "turn-interrupted",
  "presentation-detached",
  "observation-became-stale",
  "observation-reconnected",
  "session-ended"
}

RejectedActionNames == {
  "repeated-sequence-rejected",
  "stale-sequence-rejected",
  "wrong-generation-rejected",
  "wrong-identity-rejected",
  "invalid-observation-rejected"
}

VARIABLES
  sessionState,
  attached,
  sourceVisibility,
  correlationHalf,
  trustedHalf,
  joinAccepted,
  acknowledged,
  candidates,
  runtimeRevision,
  publishedRevision,
  publishedCanonical,
  clientRevision,
  clientCanonical,
  clientHasSnapshot,
  previousCanonical,
  previousCandidates,
  previousRuntimeRevision,
  lastAction,
  lastSession,
  transportSourceRevision,
  transportEventPending,
  pendingEventRevision,
  deliveredSnapshotRevision,
  deliveredEventRevision

canonical == <<sessionState, attached, sourceVisibility>>

CanonicalDomain ==
  [Sessions -> SessionRecordType]
    \X [Clients -> [Sessions -> BOOLEAN]]
    \X [Clients -> [Sessions -> VisibilityStates]]

vars == <<
  sessionState,
  attached,
  sourceVisibility,
  correlationHalf,
  trustedHalf,
  joinAccepted,
  acknowledged,
  candidates,
  runtimeRevision,
  publishedRevision,
  publishedCanonical,
  clientRevision,
  clientCanonical,
  clientHasSnapshot,
  previousCanonical,
  previousCandidates,
  previousRuntimeRevision,
  lastAction,
  lastSession,
  transportSourceRevision,
  transportEventPending,
  pendingEventRevision,
  deliveredSnapshotRevision,
  deliveredEventRevision
>>

PublicationState == <<
  publishedRevision,
  publishedCanonical,
  clientRevision,
  clientCanonical,
  clientHasSnapshot
>>

TransportState == <<
  transportSourceRevision,
  transportEventPending,
  pendingEventRevision,
  deliveredSnapshotRevision,
  deliveredEventRevision
>>

AttachedClients(session) ==
  {client \in Clients: attached[client][session]}

HasAttachment(session) == AttachedClients(session) # {}

NotificationEligible(session) ==
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].freshness = "fresh"
  /\ HasAttachment(session)
  /\ \A client \in AttachedClients(session):
       sourceVisibility[client][session] = "hidden"

ReducerOpen ==
  /\ Scenario = ReducerScenario
  /\ publishedRevision = 0

CanMutate(session) ==
  /\ ReducerOpen
  /\ runtimeRevision < MaxRuntimeRevision
  /\ sessionState[session].sequence < MaxSequence

RecordCanonical(action, session) ==
  /\ runtimeRevision' = runtimeRevision + 1
  /\ previousRuntimeRevision' = runtimeRevision
  /\ previousCanonical' = canonical
  /\ previousCandidates' = candidates
  /\ lastAction' = action
  /\ lastSession' = session

RecordObserved(action, session) ==
  /\ runtimeRevision' = runtimeRevision
  /\ previousRuntimeRevision' = runtimeRevision
  /\ previousCanonical' = canonical
  /\ previousCandidates' = candidates
  /\ lastAction' = action
  /\ lastSession' = session

Init ==
  /\ Scenario \in Scenarios
  /\ sessionState = [session \in Sessions |-> EmptySession]
  /\ attached =
       [client \in Clients |-> [session \in Sessions |-> FALSE]]
  /\ sourceVisibility =
       [client \in Clients |-> [session \in Sessions |-> "detached"]]
  /\ correlationHalf = [session \in Sessions |-> FALSE]
  /\ trustedHalf = [session \in Sessions |-> FALSE]
  /\ joinAccepted = [session \in Sessions |-> FALSE]
  /\ acknowledged = {}
  /\ candidates = {}
  /\ runtimeRevision = 0
  /\ publishedRevision = 0
  /\ publishedCanonical = canonical
  /\ clientRevision = [client \in Clients |-> 0]
  /\ clientCanonical = [client \in Clients |-> canonical]
  /\ clientHasSnapshot = [client \in Clients |-> FALSE]
  /\ previousCanonical = canonical
  /\ previousCandidates = {}
  /\ previousRuntimeRevision = 0
  /\ lastAction = "init"
  /\ lastSession = NoSession
  /\ transportSourceRevision = 0
  /\ transportEventPending = FALSE
  /\ pendingEventRevision = 0
  /\ deliveredSnapshotRevision = 0
  /\ deliveredEventRevision = 0

ReceiveCorrelationHalf(session) ==
  /\ ReducerOpen
  /\ sessionState[session].lifecycle = "unobserved"
  /\ ~correlationHalf[session]
  /\ correlationHalf' = [correlationHalf EXCEPT ![session] = TRUE]
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, trustedHalf,
       joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordObserved("correlation-half-received", session)

ReceiveTrustedHalf(session) ==
  /\ ReducerOpen
  /\ sessionState[session].lifecycle = "unobserved"
  /\ ~trustedHalf[session]
  /\ trustedHalf' = [trustedHalf EXCEPT ![session] = TRUE]
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordObserved("trusted-half-received", session)

AcceptJoinedObservation(session) ==
  /\ ReducerOpen
  /\ sessionState[session].lifecycle = "unobserved"
  /\ correlationHalf[session]
  /\ trustedHalf[session]
  /\ ~joinAccepted[session]
  /\ joinAccepted' = [joinAccepted EXCEPT ![session] = TRUE]
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordObserved("joined-observation-accepted", session)

SessionObserved(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "unobserved"
  /\ joinAccepted[session]
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].lifecycle = "live",
         ![session].freshness = "fresh",
         ![session].sequence = 1]
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordCanonical("session-observed", session)

AttachPresentation(client, session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ ~attached[client][session]
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].sequence = @ + 1]
  /\ attached' = [attached EXCEPT ![client][session] = TRUE]
  /\ sourceVisibility' =
       [sourceVisibility EXCEPT ![client][session] = "unknown"]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<correlationHalf, trustedHalf, joinAccepted, acknowledged,
       PublicationState, TransportState>>
  /\ RecordCanonical("presentation-attached", session)

SetClientHidden(client, session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ attached[client][session]
  /\ sourceVisibility[client][session] # "hidden"
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].sequence = @ + 1]
  /\ sourceVisibility' =
       [sourceVisibility EXCEPT ![client][session] = "hidden"]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, correlationHalf, trustedHalf, joinAccepted,
       acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("client-hidden", session)

SetClientVisible(client, session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ attached[client][session]
  /\ sourceVisibility[client][session] # "visible"
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].sequence = @ + 1]
  /\ sourceVisibility' =
       [sourceVisibility EXCEPT ![client][session] = "visible"]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, correlationHalf, trustedHalf, joinAccepted,
       acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("client-visible", session)

TurnStarted(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].freshness = "fresh"
  /\ sessionState[session].turn \in {"unknown", "idle"}
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].turn = "active",
         ![session].pending = FALSE,
         ![session].activity = FALSE,
         ![session].sequence = @ + 1]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("turn-started", session)

TurnActivityChanged(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].turn = "active"
  /\ ~sessionState[session].activity
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].activity = TRUE,
         ![session].sequence = @ + 1]
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordCanonical("turn-activity-changed", session)

LimitedActivityObserved(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].turn = "unknown"
  /\ ~sessionState[session].activity
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].activity = TRUE,
         ![session].sequence = @ + 1]
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordCanonical("limited-activity-observed", session)

AttentionRequested(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].freshness = "fresh"
  /\ sessionState[session].turn = "active"
  /\ ~sessionState[session].pending
  /\ sessionState[session].requestVersion < MaxRequestVersion
  /\ NotificationEligible(session)
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].pending = TRUE,
         ![session].requestVersion = @ + 1,
         ![session].sequence = @ + 1]
  /\ candidates' = candidates \cup {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("attention-requested", session)

ClientAcknowledged(client, session) ==
  /\ ReducerOpen
  /\ sessionState[session].pending
  /\ attached[client][session]
  /\ <<client, session, sessionState[session].requestVersion>>
       \notin acknowledged
  /\ acknowledged' =
       acknowledged
         \cup {<<client, session, sessionState[session].requestVersion>>}
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, PublicationState, TransportState>>
  /\ RecordObserved("client-acknowledged", session)

AttentionResolved(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].pending
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].pending = FALSE,
         ![session].sequence = @ + 1]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("attention-resolved", session)

SessionIdleObserved(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].turn = "active"
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].turn = "idle",
         ![session].pending = FALSE,
         ![session].sequence = @ + 1]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("session-idle-observed", session)

TurnSucceeded(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].turn = "active"
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].turn = "idle",
         ![session].pending = FALSE,
         ![session].outcome = "succeeded",
         ![session].sequence = @ + 1]
  /\ candidates' =
       IF NotificationEligible(session)
       THEN candidates \cup {session}
       ELSE candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("turn-succeeded", session)

TurnFailed(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].turn = "active"
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].turn = "idle",
         ![session].pending = FALSE,
         ![session].outcome = "failed",
         ![session].sequence = @ + 1]
  /\ candidates' =
       IF NotificationEligible(session)
       THEN candidates \cup {session}
       ELSE candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("turn-failed", session)

TurnCanceled(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].turn = "active"
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].turn = "idle",
         ![session].pending = FALSE,
         ![session].outcome = "canceled",
         ![session].sequence = @ + 1]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("turn-canceled", session)

TurnInterrupted(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].turn = "active"
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].turn = "idle",
         ![session].pending = FALSE,
         ![session].outcome = "interrupted",
         ![session].sequence = @ + 1]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("turn-interrupted", session)

DetachPresentation(client, session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ attached[client][session]
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].sequence = @ + 1]
  /\ attached' = [attached EXCEPT ![client][session] = FALSE]
  /\ sourceVisibility' =
       [sourceVisibility EXCEPT ![client][session] = "detached"]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<correlationHalf, trustedHalf, joinAccepted, acknowledged,
       PublicationState, TransportState>>
  /\ RecordCanonical("presentation-detached", session)

ObservationBecameStale(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].freshness = "fresh"
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].freshness = "stale",
         ![session].sequence = @ + 1]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("observation-became-stale", session)

ObservationReconnected(session) ==
  /\ ReducerOpen
  /\ runtimeRevision < MaxRuntimeRevision
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].freshness \in {"stale", "disconnected"}
  /\ sessionState[session].generation < 1
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].freshness = "fresh",
         ![session].generation = @ + 1,
         ![session].sequence = 1]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("observation-reconnected", session)

RejectRepeatedSequence(session) ==
  /\ ReducerOpen
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].sequence > 0
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordObserved("repeated-sequence-rejected", session)

RejectStaleSequence(session) ==
  /\ ReducerOpen
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState[session].sequence > 0
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordObserved("stale-sequence-rejected", session)

RejectWrongGeneration(session) ==
  /\ ReducerOpen
  /\ sessionState[session].lifecycle = "live"
  /\ \E receivedGeneration \in Generations:
       receivedGeneration # sessionState[session].generation
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordObserved("wrong-generation-rejected", session)

RejectWrongIdentity(session) ==
  /\ ReducerOpen
  /\ \E otherSession \in Sessions: otherSession # session
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordObserved("wrong-identity-rejected", session)

RejectInvalidObservation(session) ==
  /\ ReducerOpen
  /\ sessionState[session].lifecycle \in {"unobserved", "ended"}
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates, PublicationState,
       TransportState>>
  /\ RecordObserved("invalid-observation-rejected", session)

SessionEnded(session) ==
  /\ CanMutate(session)
  /\ sessionState[session].lifecycle = "live"
  /\ sessionState' =
       [sessionState EXCEPT
         ![session].lifecycle = "ended",
         ![session].turn = "unknown",
         ![session].pending = FALSE,
         ![session].outcome = "interrupted",
         ![session].freshness = "disconnected",
         ![session].sequence = @ + 1]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<attached, sourceVisibility, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, PublicationState, TransportState>>
  /\ RecordCanonical("session-ended", session)

PublishSnapshot ==
  /\ Scenario = ReducerScenario
  /\ publishedRevision = 0
  /\ runtimeRevision > 0
  /\ publishedRevision' = runtimeRevision
  /\ publishedCanonical' = canonical
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates, clientRevision,
       clientCanonical, clientHasSnapshot, TransportState>>
  /\ RecordObserved("snapshot-published", NoSession)

ClientObservePublication(client) ==
  /\ Scenario = ReducerScenario
  /\ publishedRevision > 0
  /\ ~clientHasSnapshot[client]
  /\ clientRevision' =
       [clientRevision EXCEPT ![client] = publishedRevision]
  /\ clientCanonical' =
       [clientCanonical EXCEPT ![client] = publishedCanonical]
  /\ clientHasSnapshot' = [clientHasSnapshot EXCEPT ![client] = TRUE]
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates,
       publishedRevision, publishedCanonical, TransportState>>
  /\ RecordObserved("client-observed-publication", NoSession)

RendererReconnect(client) ==
  /\ Scenario = ReducerScenario
  /\ clientHasSnapshot[client]
  /\ clientRevision' = [clientRevision EXCEPT ![client] = 0]
  /\ clientHasSnapshot' = [clientHasSnapshot EXCEPT ![client] = FALSE]
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates,
       publishedRevision, publishedCanonical, clientCanonical,
       TransportState>>
  /\ RecordObserved("renderer-reconnected", NoSession)

ApplyEventfulTransition ==
  /\ Scenario = TransportScenario
  /\ ~transportEventPending
  /\ transportSourceRevision < MaxTransportRevision
  /\ transportSourceRevision' = transportSourceRevision + 1
  /\ transportEventPending' = TRUE
  /\ pendingEventRevision' = transportSourceRevision + 1
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates,
       PublicationState, deliveredSnapshotRevision, deliveredEventRevision>>
  /\ RecordObserved("eventful-transition-applied", NoSession)

DeliverEventSnapshot ==
  /\ Scenario = TransportScenario
  /\ transportEventPending
  /\ deliveredSnapshotRevision < pendingEventRevision
  /\ deliveredSnapshotRevision' = pendingEventRevision
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates,
       PublicationState, transportSourceRevision, transportEventPending,
       pendingEventRevision, deliveredEventRevision>>
  /\ RecordObserved("event-snapshot-delivered", NoSession)

DeliverSemanticEvent ==
  /\ Scenario = TransportScenario
  /\ transportEventPending
  /\ deliveredSnapshotRevision >= pendingEventRevision
  /\ deliveredEventRevision < pendingEventRevision
  /\ deliveredEventRevision' = pendingEventRevision
  /\ transportEventPending' = FALSE
  /\ UNCHANGED <<sessionState, attached, sourceVisibility, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates,
       PublicationState, transportSourceRevision, pendingEventRevision,
       deliveredSnapshotRevision>>
  /\ RecordObserved("semantic-event-delivered", NoSession)

Next ==
  \/ \E session \in Sessions: ReceiveCorrelationHalf(session)
  \/ \E session \in Sessions: ReceiveTrustedHalf(session)
  \/ \E session \in Sessions: AcceptJoinedObservation(session)
  \/ \E session \in Sessions: SessionObserved(session)
  \/ \E client \in Clients, session \in Sessions:
       AttachPresentation(client, session)
  \/ \E client \in Clients, session \in Sessions:
       SetClientHidden(client, session)
  \/ \E client \in Clients, session \in Sessions:
       SetClientVisible(client, session)
  \/ \E session \in Sessions: TurnStarted(session)
  \/ \E session \in Sessions: TurnActivityChanged(session)
  \/ \E session \in Sessions: LimitedActivityObserved(session)
  \/ \E session \in Sessions: AttentionRequested(session)
  \/ \E client \in Clients, session \in Sessions:
       ClientAcknowledged(client, session)
  \/ \E session \in Sessions: AttentionResolved(session)
  \/ \E session \in Sessions: SessionIdleObserved(session)
  \/ \E session \in Sessions: TurnSucceeded(session)
  \/ \E session \in Sessions: TurnFailed(session)
  \/ \E session \in Sessions: TurnCanceled(session)
  \/ \E session \in Sessions: TurnInterrupted(session)
  \/ \E client \in Clients, session \in Sessions:
       DetachPresentation(client, session)
  \/ \E session \in Sessions: ObservationBecameStale(session)
  \/ \E session \in Sessions: ObservationReconnected(session)
  \/ \E session \in Sessions: RejectRepeatedSequence(session)
  \/ \E session \in Sessions: RejectStaleSequence(session)
  \/ \E session \in Sessions: RejectWrongGeneration(session)
  \/ \E session \in Sessions: RejectWrongIdentity(session)
  \/ \E session \in Sessions: RejectInvalidObservation(session)
  \/ \E session \in Sessions: SessionEnded(session)
  \/ PublishSnapshot
  \/ \E client \in Clients: ClientObservePublication(client)
  \/ \E client \in Clients: RendererReconnect(client)
  \/ ApplyEventfulTransition
  \/ DeliverEventSnapshot
  \/ DeliverSemanticEvent

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ Scenario \in Scenarios
  /\ MaxRuntimeRevision \in Nat
  /\ MaxSequence \in Nat
  /\ MaxRequestVersion \in Nat
  /\ MaxTransportRevision \in Nat
  /\ sessionState \in [Sessions -> SessionRecordType]
  /\ attached \in [Clients -> [Sessions -> BOOLEAN]]
  /\ sourceVisibility \in
       [Clients -> [Sessions -> VisibilityStates]]
  /\ correlationHalf \in [Sessions -> BOOLEAN]
  /\ trustedHalf \in [Sessions -> BOOLEAN]
  /\ joinAccepted \in [Sessions -> BOOLEAN]
  /\ acknowledged \subseteq
       Clients \X Sessions \X (0..MaxRequestVersion)
  /\ candidates \subseteq Sessions
  /\ runtimeRevision \in 0..MaxRuntimeRevision
  /\ publishedRevision \in 0..MaxRuntimeRevision
  /\ publishedCanonical \in CanonicalDomain
  /\ clientRevision \in [Clients -> 0..MaxRuntimeRevision]
  /\ clientCanonical \in [Clients -> CanonicalDomain]
  /\ clientHasSnapshot \in [Clients -> BOOLEAN]
  /\ previousCanonical \in CanonicalDomain
  /\ previousCandidates \subseteq Sessions
  /\ previousRuntimeRevision \in 0..MaxRuntimeRevision
  /\ lastAction \in Actions
  /\ lastSession \in Sessions \cup {NoSession}
  /\ transportSourceRevision \in 0..MaxTransportRevision
  /\ transportEventPending \in BOOLEAN
  /\ pendingEventRevision \in 0..MaxTransportRevision
  /\ deliveredSnapshotRevision \in 0..MaxTransportRevision
  /\ deliveredEventRevision \in 0..MaxTransportRevision

JoinRequiresBothHalves ==
  Scenario # ReducerScenario \/
    \A session \in Sessions:
      joinAccepted[session] =>
        correlationHalf[session] /\ trustedHalf[session]

AttachmentVisibilityConsistent ==
  Scenario # ReducerScenario \/
    \A client \in Clients, session \in Sessions:
      attached[client][session] =
        (sourceVisibility[client][session] # "detached")

CanonicalMutationIsRevisioned ==
  Scenario # ReducerScenario \/
    (canonical # previousCanonical =>
      /\ lastAction \in CanonicalActionNames
      /\ runtimeRevision = previousRuntimeRevision + 1)

AcknowledgementCannotResolveRequest ==
  Scenario # ReducerScenario \/
    (lastAction = "client-acknowledged" =>
      /\ canonical = previousCanonical
      /\ sessionState[lastSession].pending =
           previousCanonical[1][lastSession].pending)

RejectedObservationPreservesCanonical ==
  Scenario # ReducerScenario \/
    (lastAction \in RejectedActionNames =>
      /\ canonical = previousCanonical
      /\ candidates = previousCandidates)

LimitedActivityOnlyChangesHint ==
  Scenario # ReducerScenario \/
    (lastAction = "limited-activity-observed" =>
      /\ sessionState =
           [previousCanonical[1] EXCEPT
             ![lastSession].activity = TRUE,
             ![lastSession].sequence = @ + 1]
      /\ attached = previousCanonical[2]
      /\ sourceVisibility = previousCanonical[3]
      /\ candidates = previousCandidates)

IdleDoesNotFabricateOutcomeOrCandidate ==
  Scenario # ReducerScenario \/
    (lastAction = "session-idle-observed" =>
      /\ sessionState[lastSession].outcome =
           previousCanonical[1][lastSession].outcome
      /\ lastSession \notin candidates)

ActivityDoesNotCreateCandidate ==
  Scenario # ReducerScenario \/
    (lastAction \in
       {"turn-activity-changed", "limited-activity-observed"} =>
      candidates \subseteq previousCandidates)

NotificationCandidateHasAuthority ==
  Scenario # ReducerScenario \/
    \A session \in candidates:
      /\ sessionState[session].lifecycle = "live"
      /\ sessionState[session].freshness = "fresh"
      /\ NotificationEligible(session)
      /\ \/ sessionState[session].pending
         \/ sessionState[session].outcome \in {"succeeded", "failed"}

OutcomeNotificationClasses ==
  Scenario # ReducerScenario \/
    /\ (lastAction \in {"turn-succeeded", "turn-failed"} =>
         (lastSession \in candidates) = NotificationEligible(lastSession))
    /\ (lastAction \in {"turn-canceled", "turn-interrupted"} =>
         lastSession \notin candidates)

GenerationChangesOnlyOnReconnect ==
  Scenario # ReducerScenario \/
    (sessionState # previousCanonical[1] =>
      \A session \in Sessions:
        sessionState[session].generation #
          previousCanonical[1][session].generation =>
            /\ lastAction = "observation-reconnected"
            /\ lastSession = session
            /\ sessionState[session].generation =
                 previousCanonical[1][session].generation + 1)

EndedSessionHasNoActiveWork ==
  Scenario # ReducerScenario \/
    \A session \in Sessions:
      sessionState[session].lifecycle = "ended" =>
        /\ sessionState[session].turn # "active"
        /\ ~sessionState[session].pending
        /\ session \notin candidates

PublicationIsExactFinalSnapshot ==
  Scenario # ReducerScenario \/
    (lastAction = "snapshot-published" =>
      /\ publishedRevision = runtimeRevision
      /\ publishedCanonical = canonical)

ClientRevisionDoesNotLeadPublication ==
  Scenario # ReducerScenario \/
    \A client \in Clients:
      clientRevision[client] <= publishedRevision

ObservedClientSnapshotIsExact ==
  Scenario # ReducerScenario \/
    \A client \in Clients:
      clientHasSnapshot[client] =>
        /\ clientRevision[client] = publishedRevision
        /\ clientCanonical[client] = publishedCanonical

RendererReconnectClearsOnlyClientCache ==
  Scenario # ReducerScenario \/
    (lastAction = "renderer-reconnected" =>
      /\ canonical = previousCanonical
      /\ sourceVisibility = previousCanonical[3]
      /\ \E client \in Clients: ~clientHasSnapshot[client])

TransportRevisionConsistency ==
  Scenario # TransportScenario \/
    /\ deliveredEventRevision <= deliveredSnapshotRevision
    /\ deliveredSnapshotRevision <= transportSourceRevision
    /\ pendingEventRevision <= transportSourceRevision
    /\ (transportEventPending =>
         /\ pendingEventRevision = transportSourceRevision
         /\ deliveredEventRevision < pendingEventRevision)

SemanticEventFollowsItsSnapshot ==
  Scenario # TransportScenario \/
    (lastAction = "semantic-event-delivered" =>
      deliveredSnapshotRevision >= deliveredEventRevision)

=============================================================================
