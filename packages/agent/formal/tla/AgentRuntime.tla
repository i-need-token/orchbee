--------------------------- MODULE AgentRuntime ---------------------------
EXTENDS FiniteSets, Naturals, TLC

AttentionSession == "attention-session"
ReconnectSession == "reconnect-session"
Sessions == {AttentionSession, ReconnectSession}
PrimaryClient == "client-1"
SecondaryClient == "client-2"
Clients == {PrimaryClient, SecondaryClient}
Lifecycles == {"unobserved", "live", "ended"}
TurnStates == {"unknown", "active", "idle"}
Outcomes == {"none", "succeeded", "failed", "canceled", "interrupted"}
FreshnessStates == {"fresh", "stale", "disconnected"}
Generations == 0..1
MaxSequence == 6
MaxRequestVersion == 1
MaxPublicationRevision == 1

VARIABLES
  lifecycle,
  turn,
  pending,
  outcome,
  freshness,
  generation,
  sequence,
  requestVersion,
  activityBit,
  attached,
  visible,
  correlationHalf,
  trustedHalf,
  joinAccepted,
  acknowledged,
  candidates,
  publicationRevision,
  publishedCanonical,
  clientRevision,
  clientCanonical,
  clientHasSnapshot,
  rendererReconnected,
  rejectionCount,
  lastAction,
  previousLifecycle,
  previousTurn,
  previousPending,
  previousOutcome,
  previousFreshness,
  previousGeneration,
  previousSequence,
  previousAttached,
  previousCandidates

canonical ==
  <<lifecycle, turn, pending, outcome, freshness, generation, sequence,
    requestVersion, activityBit, attached>>

vars ==
  <<lifecycle, turn, pending, outcome, freshness, generation, sequence,
    requestVersion, activityBit, attached, visible, correlationHalf,
    trustedHalf, joinAccepted, acknowledged, candidates,
    publicationRevision, publishedCanonical, clientRevision,
    clientCanonical, clientHasSnapshot, rendererReconnected,
    rejectionCount, lastAction, previousLifecycle, previousTurn,
    previousPending, previousOutcome, previousFreshness,
    previousGeneration, previousSequence, previousAttached,
    previousCandidates>>

Record(action) ==
  /\ lastAction' = action
  /\ previousLifecycle' = lifecycle
  /\ previousTurn' = turn
  /\ previousPending' = pending
  /\ previousOutcome' = outcome
  /\ previousFreshness' = freshness
  /\ previousGeneration' = generation
  /\ previousSequence' = sequence
  /\ previousAttached' = attached
  /\ previousCandidates' = candidates

AttachedClientsFor(session) ==
  {client \in Clients: attached[client][session]}

HasAttachment(session) == AttachedClientsFor(session) # {}

AnyAttachmentVisible(session) ==
  \E client \in AttachedClientsFor(session): visible[client][session]

NotificationEligible(session) ==
  HasAttachment(session) /\ ~AnyAttachmentVisible(session)

Init ==
  /\ lifecycle = [session \in Sessions |-> "unobserved"]
  /\ turn = [session \in Sessions |-> "unknown"]
  /\ pending = [session \in Sessions |-> FALSE]
  /\ outcome = [session \in Sessions |-> "none"]
  /\ freshness = [session \in Sessions |-> "disconnected"]
  /\ generation = [session \in Sessions |-> 0]
  /\ sequence = [session \in Sessions |-> 0]
  /\ requestVersion = [session \in Sessions |-> 0]
  /\ activityBit = [session \in Sessions |-> FALSE]
  /\ attached = [client \in Clients |-> [session \in Sessions |-> FALSE]]
  /\ visible = [client \in Clients |-> [session \in Sessions |-> FALSE]]
  /\ correlationHalf = [session \in Sessions |-> FALSE]
  /\ trustedHalf = [session \in Sessions |-> FALSE]
  /\ joinAccepted = [session \in Sessions |-> FALSE]
  /\ acknowledged = {}
  /\ candidates = {}
  /\ publicationRevision = 0
  /\ publishedCanonical = canonical
  /\ clientRevision = [client \in Clients |-> 0]
  /\ clientCanonical = [client \in Clients |-> canonical]
  /\ clientHasSnapshot = [client \in Clients |-> FALSE]
  /\ rendererReconnected = FALSE
  /\ rejectionCount = 0
  /\ lastAction = "init"
  /\ previousLifecycle = lifecycle
  /\ previousTurn = turn
  /\ previousPending = pending
  /\ previousOutcome = outcome
  /\ previousFreshness = freshness
  /\ previousGeneration = generation
  /\ previousSequence = sequence
  /\ previousAttached = attached
  /\ previousCandidates = candidates

ReceiveCorrelationHalf(session) ==
  /\ session = AttentionSession
  /\ lifecycle[session] = "unobserved"
  /\ ~correlationHalf[session]
  /\ correlationHalf' = [correlationHalf EXCEPT ![session] = TRUE]
  /\ UNCHANGED <<canonical, visible, trustedHalf, joinAccepted,
       acknowledged, candidates, publicationRevision,
       publishedCanonical, clientRevision, clientCanonical,
       clientHasSnapshot, rendererReconnected, rejectionCount>>
  /\ Record("correlation-half-received")

ReceiveTrustedHalf(session) ==
  /\ session = AttentionSession
  /\ lifecycle[session] = "unobserved"
  /\ ~trustedHalf[session]
  /\ trustedHalf' = [trustedHalf EXCEPT ![session] = TRUE]
  /\ UNCHANGED <<canonical, visible, correlationHalf, joinAccepted,
       acknowledged, candidates, publicationRevision,
       publishedCanonical, clientRevision, clientCanonical,
       clientHasSnapshot, rendererReconnected, rejectionCount>>
  /\ Record("trusted-half-received")

AcceptJoinedObservation(session) ==
  /\ session = AttentionSession
  /\ lifecycle[session] = "unobserved"
  /\ correlationHalf[session]
  /\ trustedHalf[session]
  /\ ~joinAccepted[session]
  /\ joinAccepted' = [joinAccepted EXCEPT ![session] = TRUE]
  /\ UNCHANGED <<canonical, visible, correlationHalf, trustedHalf,
       acknowledged, candidates, publicationRevision,
       publishedCanonical, clientRevision, clientCanonical,
       clientHasSnapshot, rendererReconnected, rejectionCount>>
  /\ Record("joined-observation-accepted")

SessionObserved(session) ==
  /\ lifecycle[session] = "unobserved"
  /\ IF session = AttentionSession
       THEN joinAccepted[session]
       ELSE session = ReconnectSession
  /\ lifecycle' = [lifecycle EXCEPT ![session] = "live"]
  /\ freshness' = [freshness EXCEPT ![session] = "fresh"]
  /\ sequence' = [sequence EXCEPT ![session] = 1]
  /\ attached' = [attached EXCEPT ![PrimaryClient][session] = TRUE]
  /\ UNCHANGED <<turn, pending, outcome, generation, requestVersion,
       activityBit, visible, correlationHalf, trustedHalf, joinAccepted,
       acknowledged, candidates,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected,
       rejectionCount>>
  /\ Record("session-observed")

AttachPresentation(client, session) ==
  /\ client = SecondaryClient
  /\ session = AttentionSession
  /\ lifecycle[session] = "live"
  /\ sequence[session] = 1
  /\ attached[PrimaryClient][session]
  /\ ~attached[client][session]
  /\ attached' = [attached EXCEPT ![client][session] = TRUE]
  /\ UNCHANGED <<lifecycle, turn, pending, outcome, freshness,
       generation, sequence, requestVersion, activityBit, visible,
       correlationHalf, trustedHalf, joinAccepted, acknowledged,
       candidates, publicationRevision, publishedCanonical,
       clientRevision, clientCanonical, clientHasSnapshot,
       rendererReconnected, rejectionCount>>
  /\ Record("presentation-attached")

SetClientVisible(client, session) ==
  /\ client = PrimaryClient
  /\ session = AttentionSession
  /\ lifecycle[session] = "live"
  /\ sequence[session] = 1
  /\ attached[client][session]
  /\ attached[SecondaryClient][session]
  /\ ~visible[client][session]
  /\ visible' = [visible EXCEPT ![client][session] = TRUE]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<canonical, correlationHalf, trustedHalf, joinAccepted,
       acknowledged, publicationRevision,
       publishedCanonical, clientRevision, clientCanonical,
       clientHasSnapshot, rendererReconnected, rejectionCount>>
  /\ Record("client-visible")

TurnStarted(session) ==
  /\ lifecycle[session] = "live"
  /\ sequence[session] = 1
  /\ IF session = AttentionSession
       THEN visible[PrimaryClient][session] /\ attached[SecondaryClient][session]
       ELSE session = ReconnectSession
  /\ turn' = [turn EXCEPT ![session] = "active"]
  /\ sequence' = [sequence EXCEPT ![session] = 2]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<lifecycle, pending, outcome, freshness, generation,
       requestVersion, activityBit, attached, visible, correlationHalf,
       trustedHalf, joinAccepted, acknowledged,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected,
       rejectionCount>>
  /\ Record("turn-started")

SetClientHidden(client, session) ==
  /\ client = PrimaryClient
  /\ session = AttentionSession
  /\ lifecycle[session] = "live"
  /\ turn[session] = "active"
  /\ sequence[session] = 2
  /\ attached[client][session]
  /\ visible[client][session]
  /\ visible' = [visible EXCEPT ![client][session] = FALSE]
  /\ UNCHANGED <<canonical, correlationHalf, trustedHalf, joinAccepted,
       acknowledged, candidates, publicationRevision,
       publishedCanonical, clientRevision, clientCanonical,
       clientHasSnapshot, rendererReconnected, rejectionCount>>
  /\ Record("client-hidden")

TurnActivityChanged(session) ==
  /\ session = ReconnectSession
  /\ lifecycle[session] = "live"
  /\ turn[session] = "active"
  /\ sequence[session] = 2
  /\ activityBit' = [activityBit EXCEPT ![session] = TRUE]
  /\ sequence' = [sequence EXCEPT ![session] = 3]
  /\ UNCHANGED <<lifecycle, turn, pending, outcome, freshness,
       generation, requestVersion, attached, visible, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected,
       rejectionCount>>
  /\ Record("turn-activity-changed")

LimitedActivityObserved(session) ==
  /\ lifecycle[session] = "live"
  /\ turn[session] = "unknown"
  /\ sequence[session] = 1
  /\ ~activityBit[session]
  /\ activityBit' = [activityBit EXCEPT ![session] = TRUE]
  /\ sequence' = [sequence EXCEPT ![session] = 2]
  /\ UNCHANGED <<lifecycle, turn, pending, outcome, freshness,
       generation, requestVersion, attached, visible, correlationHalf,
       trustedHalf, joinAccepted, acknowledged, candidates,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected,
       rejectionCount>>
  /\ Record("limited-activity-observed")

AttentionRequested(session) ==
  /\ session = AttentionSession
  /\ lifecycle[session] = "live"
  /\ turn[session] = "active"
  /\ sequence[session] = 2
  /\ ~pending[session]
  /\ NotificationEligible(session)
  /\ pending' = [pending EXCEPT ![session] = TRUE]
  /\ requestVersion' = [requestVersion EXCEPT ![session] = 1]
  /\ sequence' = [sequence EXCEPT ![session] = 3]
  /\ candidates' = candidates \cup {session}
  /\ UNCHANGED <<lifecycle, turn, outcome, freshness, generation,
       activityBit, attached, visible, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, publicationRevision,
       publishedCanonical, clientRevision, clientCanonical,
       clientHasSnapshot, rendererReconnected, rejectionCount>>
  /\ Record("attention-requested")

ClientAcknowledged(client, session) ==
  /\ client = SecondaryClient
  /\ session = AttentionSession
  /\ sequence[session] = 3
  /\ pending[session]
  /\ <<client, session, requestVersion[session]>> \notin acknowledged
  /\ acknowledged' =
       acknowledged \cup {<<client, session, requestVersion[session]>>}
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<canonical, visible, correlationHalf, trustedHalf,
       joinAccepted, publicationRevision,
       publishedCanonical, clientRevision, clientCanonical,
       clientHasSnapshot, rendererReconnected, rejectionCount>>
  /\ Record("client-acknowledged")

AttentionResolved(session) ==
  /\ session = AttentionSession
  /\ lifecycle[session] = "live"
  /\ sequence[session] = 3
  /\ pending[session]
  /\ <<SecondaryClient, session, requestVersion[session]>> \in acknowledged
  /\ pending' = [pending EXCEPT ![session] = FALSE]
  /\ sequence' = [sequence EXCEPT ![session] = 4]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<lifecycle, turn, outcome, freshness, generation,
       requestVersion, activityBit, attached, visible, correlationHalf,
       trustedHalf, joinAccepted, acknowledged,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected,
       rejectionCount>>
  /\ Record("attention-resolved")

SessionIdleObserved(session) ==
  /\ session = AttentionSession
  /\ lifecycle[session] = "live"
  /\ turn[session] = "active"
  /\ sequence[session] = 4
  /\ turn' = [turn EXCEPT ![session] = "idle"]
  /\ pending' = [pending EXCEPT ![session] = FALSE]
  /\ sequence' = [sequence EXCEPT ![session] = 5]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<lifecycle, outcome, freshness, generation,
       requestVersion, activityBit, attached, visible, correlationHalf,
       trustedHalf, joinAccepted, acknowledged,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected,
       rejectionCount>>
  /\ Record("session-idle-observed")

TurnFinished(session, result) ==
  /\ session = AttentionSession
  /\ result = "failed"
  /\ lifecycle[session] = "live"
  /\ turn[session] = "active"
  /\ sequence[session] = 4
  /\ turn' = [turn EXCEPT ![session] = "idle"]
  /\ outcome' = [outcome EXCEPT ![session] = result]
  /\ sequence' = [sequence EXCEPT ![session] = 5]
  /\ candidates' = (candidates \ {session}) \cup {session}
  /\ UNCHANGED <<lifecycle, pending, freshness, generation,
       requestVersion, activityBit, attached, visible, correlationHalf,
       trustedHalf, joinAccepted, acknowledged,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected,
       rejectionCount>>
  /\ Record("turn-finished")

DetachPresentation(client, session) ==
  /\ client = PrimaryClient
  /\ session = AttentionSession
  /\ lifecycle[session] = "live"
  /\ sequence[session] = 5
  /\ attached[client][session]
  /\ attached[SecondaryClient][session]
  /\ attached' = [attached EXCEPT ![client][session] = FALSE]
  /\ visible' = [visible EXCEPT ![client][session] = FALSE]
  /\ UNCHANGED <<lifecycle, turn, pending, outcome, freshness,
       generation, sequence, requestVersion, activityBit,
       correlationHalf, trustedHalf, joinAccepted, acknowledged,
       candidates, publicationRevision, publishedCanonical,
       clientRevision, clientCanonical, clientHasSnapshot,
       rendererReconnected, rejectionCount>>
  /\ Record("presentation-detached")

ObservationBecameStale(session) ==
  /\ session = ReconnectSession
  /\ lifecycle[session] = "live"
  /\ freshness[session] = "fresh"
  /\ sequence[session] = 3
  /\ freshness' = [freshness EXCEPT ![session] = "stale"]
  /\ sequence' = [sequence EXCEPT ![session] = 4]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<lifecycle, turn, pending, outcome, generation,
       requestVersion, activityBit, attached, visible, correlationHalf,
       trustedHalf, joinAccepted, acknowledged,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected,
       rejectionCount>>
  /\ Record("observation-became-stale")

ObservationReconnected(session) ==
  /\ session = ReconnectSession
  /\ lifecycle[session] = "live"
  /\ freshness[session] = "stale"
  /\ generation[session] = 0
  /\ sequence[session] = 4
  /\ freshness' = [freshness EXCEPT ![session] = "fresh"]
  /\ generation' = [generation EXCEPT ![session] = 1]
  /\ sequence' = [sequence EXCEPT ![session] = 5]
  /\ UNCHANGED <<lifecycle, turn, pending, outcome, requestVersion,
       activityBit, attached, visible, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, candidates,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected,
       rejectionCount>>
  /\ Record("observation-reconnected")

RejectStaleGeneration(session) ==
  /\ session = ReconnectSession
  /\ lifecycle[session] = "live"
  /\ generation[session] = 1
  /\ sequence[session] = 5
  /\ rejectionCount = 0
  /\ rejectionCount' = 1
  /\ UNCHANGED <<canonical, visible, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, candidates,
       publicationRevision, publishedCanonical, clientRevision,
       clientCanonical, clientHasSnapshot, rendererReconnected>>
  /\ Record("reject-stale-generation")

SessionEnded(session) ==
  /\ session = ReconnectSession
  /\ lifecycle[session] = "live"
  /\ sequence[session] = 5
  /\ rejectionCount = 1
  /\ lifecycle' = [lifecycle EXCEPT ![session] = "ended"]
  /\ turn' = [turn EXCEPT ![session] = "unknown"]
  /\ outcome' = [outcome EXCEPT ![session] = "interrupted"]
  /\ freshness' = [freshness EXCEPT ![session] = "disconnected"]
  /\ sequence' = [sequence EXCEPT ![session] = 6]
  /\ candidates' = candidates \ {session}
  /\ UNCHANGED <<pending, generation, requestVersion, activityBit,
       attached, visible, correlationHalf, trustedHalf, joinAccepted,
       acknowledged, publicationRevision, publishedCanonical,
       clientRevision, clientCanonical, clientHasSnapshot,
       rendererReconnected, rejectionCount>>
  /\ Record("session-ended")

PublishSnapshot ==
  /\ sequence[AttentionSession] = 5
  /\ sequence[ReconnectSession] = 6
  /\ publicationRevision = 0
  /\ publicationRevision' = 1
  /\ publishedCanonical' = canonical
  /\ UNCHANGED <<canonical, visible, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, candidates,
       clientRevision, clientCanonical, clientHasSnapshot,
       rendererReconnected, rejectionCount>>
  /\ Record("snapshot-published")

ClientObservePublication(client) ==
  /\ publicationRevision = 1
  /\ clientRevision[client] < publicationRevision
  /\ clientRevision' =
       [clientRevision EXCEPT ![client] = publicationRevision]
  /\ clientCanonical' =
       [clientCanonical EXCEPT ![client] = publishedCanonical]
  /\ clientHasSnapshot' = [clientHasSnapshot EXCEPT ![client] = TRUE]
  /\ UNCHANGED <<canonical, visible, correlationHalf, trustedHalf,
       joinAccepted, acknowledged, candidates,
       publicationRevision, publishedCanonical, rendererReconnected,
       rejectionCount>>
  /\ Record("client-observed-publication")

RendererReconnect(client) ==
  /\ client = PrimaryClient
  /\ publicationRevision = 1
  /\ clientRevision[PrimaryClient] = publicationRevision
  /\ clientRevision[SecondaryClient] = publicationRevision
  /\ ~rendererReconnected
  /\ clientRevision' = [clientRevision EXCEPT ![client] = 0]
  /\ clientHasSnapshot' = [clientHasSnapshot EXCEPT ![client] = FALSE]
  /\ visible' =
       [visible EXCEPT ![client] = [session \in Sessions |-> FALSE]]
  /\ rendererReconnected' = TRUE
  /\ UNCHANGED <<canonical, correlationHalf, trustedHalf, joinAccepted,
       acknowledged, candidates,
       publicationRevision, publishedCanonical, clientCanonical,
       rejectionCount>>
  /\ Record("renderer-reconnected")

Next ==
  \/ \E session \in Sessions: ReceiveCorrelationHalf(session)
  \/ \E session \in Sessions: ReceiveTrustedHalf(session)
  \/ \E session \in Sessions: AcceptJoinedObservation(session)
  \/ \E session \in Sessions: SessionObserved(session)
  \/ \E client \in Clients, session \in Sessions:
       AttachPresentation(client, session)
  \/ \E client \in Clients, session \in Sessions:
       SetClientVisible(client, session)
  \/ \E session \in Sessions: TurnStarted(session)
  \/ \E client \in Clients, session \in Sessions:
       SetClientHidden(client, session)
  \/ \E session \in Sessions: TurnActivityChanged(session)
  \/ \E session \in Sessions: LimitedActivityObserved(session)
  \/ \E session \in Sessions: AttentionRequested(session)
  \/ \E client \in Clients, session \in Sessions:
       ClientAcknowledged(client, session)
  \/ \E session \in Sessions: AttentionResolved(session)
  \/ \E session \in Sessions: SessionIdleObserved(session)
  \/ \E session \in Sessions, result \in Outcomes \ {"none"}:
       TurnFinished(session, result)
  \/ \E client \in Clients, session \in Sessions:
       DetachPresentation(client, session)
  \/ \E session \in Sessions: ObservationBecameStale(session)
  \/ \E session \in Sessions: ObservationReconnected(session)
  \/ \E session \in Sessions: RejectStaleGeneration(session)
  \/ \E session \in Sessions: SessionEnded(session)
  \/ PublishSnapshot
  \/ \E client \in Clients: ClientObservePublication(client)
  \/ \E client \in Clients: RendererReconnect(client)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ lifecycle \in [Sessions -> Lifecycles]
  /\ turn \in [Sessions -> TurnStates]
  /\ pending \in [Sessions -> BOOLEAN]
  /\ outcome \in [Sessions -> Outcomes]
  /\ freshness \in [Sessions -> FreshnessStates]
  /\ generation \in [Sessions -> Generations]
  /\ sequence \in [Sessions -> 0..MaxSequence]
  /\ requestVersion \in [Sessions -> 0..MaxRequestVersion]
  /\ activityBit \in [Sessions -> BOOLEAN]
  /\ attached \in [Clients -> [Sessions -> BOOLEAN]]
  /\ visible \in [Clients -> [Sessions -> BOOLEAN]]
  /\ correlationHalf \in [Sessions -> BOOLEAN]
  /\ trustedHalf \in [Sessions -> BOOLEAN]
  /\ joinAccepted \in [Sessions -> BOOLEAN]
  /\ acknowledged \subseteq Clients \X Sessions \X (1..MaxRequestVersion)
  /\ candidates \subseteq Sessions
  /\ publicationRevision \in 0..MaxPublicationRevision
  /\ clientRevision \in [Clients -> 0..MaxPublicationRevision]
  /\ clientHasSnapshot \in [Clients -> BOOLEAN]
  /\ rendererReconnected \in BOOLEAN
  /\ rejectionCount \in 0..1

EndedHasNoActiveWork ==
  \A session \in Sessions:
    lifecycle[session] = "ended" =>
      /\ turn[session] # "active"
      /\ ~pending[session]

UnobservedHasNoSemanticClaim ==
  \A session \in Sessions:
    lifecycle[session] = "unobserved" =>
      /\ turn[session] = "unknown"
      /\ ~pending[session]
      /\ outcome[session] = "none"

AcknowledgementNeverResolvesRequest ==
  lastAction = "client-acknowledged" => pending = previousPending

RejectedGenerationPreservesCanonicalState ==
  lastAction = "reject-stale-generation" =>
    /\ lifecycle = previousLifecycle
    /\ turn = previousTurn
    /\ pending = previousPending
    /\ outcome = previousOutcome
    /\ freshness = previousFreshness
    /\ generation = previousGeneration
    /\ sequence = previousSequence
    /\ candidates = previousCandidates

GenerationChangesOnlyOnReconnect ==
  generation # previousGeneration => lastAction = "observation-reconnected"

ActivityNeverCreatesCandidate ==
  lastAction = "turn-activity-changed" => candidates = previousCandidates

LimitedActivityIsOnlyHint ==
  lastAction = "limited-activity-observed" =>
    /\ turn = previousTurn
    /\ pending = previousPending
    /\ outcome = previousOutcome
    /\ candidates = previousCandidates

IdleObservationDoesNotFabricateOutcome ==
  lastAction = "session-idle-observed" =>
    /\ outcome = previousOutcome
    /\ candidates = previousCandidates

NotificationCandidatesAreSafe ==
  \A session \in candidates:
    /\ lifecycle[session] = "live"
    /\ freshness[session] = "fresh"
    /\ NotificationEligible(session)
    /\ (pending[session] \/ outcome[session] = "failed")

VisiblePresentationIsAttached ==
  \A client \in Clients, session \in Sessions:
    visible[client][session] => attached[client][session]

RemoteObservationRequiresBothHalves ==
  lifecycle[AttentionSession] # "unobserved" =>
    /\ correlationHalf[AttentionSession]
    /\ trustedHalf[AttentionSession]
    /\ joinAccepted[AttentionSession]

UnmatchedHalfPreservesCanonicalState ==
  lastAction \in {"correlation-half-received", "trusted-half-received"} =>
    /\ lifecycle = previousLifecycle
    /\ turn = previousTurn
    /\ pending = previousPending
    /\ outcome = previousOutcome
    /\ freshness = previousFreshness
    /\ generation = previousGeneration
    /\ sequence = previousSequence
    /\ attached = previousAttached
    /\ candidates = previousCandidates

DetachingOnePresentationPreservesTheOther ==
  lastAction = "presentation-detached" =>
    /\ ~attached[PrimaryClient][AttentionSession]
    /\ attached[SecondaryClient][AttentionSession]
    /\ freshness = previousFreshness
    /\ candidates = previousCandidates

ClientRevisionIsNeverFromTheFuture ==
  \A client \in Clients: clientRevision[client] <= publicationRevision

ObservedSnapshotIsExact ==
  \A client \in Clients:
    clientRevision[client] = publicationRevision /\ publicationRevision > 0 =>
      /\ clientHasSnapshot[client]
      /\ clientCanonical[client] = publishedCanonical

=============================================================================
