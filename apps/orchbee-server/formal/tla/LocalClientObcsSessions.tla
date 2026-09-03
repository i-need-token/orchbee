---------------------- MODULE LocalClientObcsSessions ----------------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

\* Model-only refinement of an admitted LocalClientSessions generation onto
\* the same stream's OBCS phase. Every non-closed phase refines one active
\* Session and therefore owns one handler. CarrierFrames is peer-buffered input,
\* not a Server queue; OutstandingRequests contains at most one correlation
\* record per Session. Its denied result is a derived projection, so exact echo
\* cannot drift from the decoded request ID. No action creates terminal,
\* Worker, Host, retry, replay, cancellation, or request-ledger effects.

CONSTANTS
  Clients, RequestIds, MaxSessions, MaxHandlers, MaxGeneration, Scenario

Scenarios == {"stream-faults", "correlation"}
ServerPhases == {"listening", "stopped"}
NoRequestId == "none"
NoPair == "none"

SessionPhases == {
  "closed", "oblc-accepted-queued", "obcs-idle", "obcs-read-stalled",
  "obcs-partial-header", "obcs-partial-payload",
  "obcs-create-result-ready", "obcs-start-result-ready",
  "obcs-create-write-stalled", "obcs-start-write-stalled"
}
ObcsReadPhases == {
  "obcs-idle", "obcs-read-stalled",
  "obcs-partial-header", "obcs-partial-payload"
}
ObcsPartialPhases == {"obcs-partial-header", "obcs-partial-payload"}
CreateResultPhases == {
  "obcs-create-result-ready", "obcs-create-write-stalled"
}
StartResultPhases == {
  "obcs-start-result-ready", "obcs-start-write-stalled"
}
ObcsResultPhases == CreateResultPhases \cup StartResultPhases
ObcsStalledPhases == {
  "obcs-read-stalled", "obcs-partial-header", "obcs-partial-payload",
  "obcs-create-write-stalled", "obcs-start-write-stalled"
}

WorkerConstraints == {"none", "exact-present", "exact-absent"}
WorkingDirectories == {"null", "absolute"}
PairKinds == {NoPair, "duplicate", "distinct"}

NoOutstandingRequest == [
  kind |-> "none",
  requestId |-> NoRequestId,
  terminalCreateRequestId |-> NoRequestId
]
OutstandingRequest(kind, requestId, terminalCreateRequestId) == [
  kind |-> kind,
  requestId |-> requestId,
  terminalCreateRequestId |-> terminalCreateRequestId
]
CreateRequest(requestId) ==
  OutstandingRequest("create", requestId, NoRequestId)
StartRequest(requestId, terminalCreateRequestId) ==
  OutstandingRequest("start", requestId, terminalCreateRequestId)

OutstandingRequestValues ==
  {NoOutstandingRequest} \cup
  {CreateRequest(requestId) : requestId \in RequestIds} \cup
  UNION {
    {StartRequest(requestId, terminalCreateRequestId) :
       terminalCreateRequestId \in RequestIds \ {requestId}} :
    requestId \in RequestIds
  }

NoResult == [
  protocolVersion |-> 0,
  kind |-> "none",
  requestId |-> NoRequestId,
  outcome |-> [kind |-> "none", reason |-> "none"]
]

DeniedResult(kind, requestId) == [
  protocolVersion |-> 1,
  kind |-> kind,
  requestId |-> requestId,
  outcome |-> [kind |-> "rejected", reason |-> "not-authorized"]
]

DeniedProjection(request) ==
  IF request = NoOutstandingRequest
  THEN NoResult
  ELSE DeniedResult(
    IF request.kind = "create"
    THEN "terminal.create.result"
    ELSE "terminal.start.result",
    request.requestId)

CarrierFrameValues == OutstandingRequestValues \ {NoOutstandingRequest}

\* Singleton frames cover coalesced Create and Start. Duplicate pairs retain
\* the same Create request ID; distinct pairs represent Create followed by a
\* Start whose independent reference names that first Create request.
CarrierFrameSequences ==
  {<<>>} \cup
  {<<request>> : request \in CarrierFrameValues} \cup
  {<<CreateRequest(requestId), CreateRequest(requestId)>> :
     requestId \in RequestIds} \cup
  UNION {
    {<<CreateRequest(first), StartRequest(second, first)>> :
       second \in RequestIds \ {first}} :
    first \in RequestIds
  }

GenerationRefs == [client : Clients, generation : 1..MaxGeneration]
GenerationRef(client, generation) == [
  client |-> client,
  generation |-> generation
]

ASSUME
  /\ IsFiniteSet(Clients)
  /\ Cardinality(Clients) >= MaxHandlers
  /\ IsFiniteSet(RequestIds)
  /\ Cardinality(RequestIds) >= 2
  /\ Clients \intersect RequestIds = {}
  /\ NoRequestId \notin Clients \cup RequestIds
  /\ MaxSessions \in Nat \ {0}
  /\ MaxHandlers \in Nat \ {0}
  /\ MaxSessions < MaxHandlers
  /\ MaxGeneration \in Nat
  /\ MaxGeneration > MaxHandlers
  /\ Scenario \in Scenarios

StateValues == [
  serverPhase : ServerPhases,
  sessions : [Clients -> 0..MaxGeneration],
  sessionPhases : [Clients -> SessionPhases],
  outstandingRequests : [Clients -> OutstandingRequestValues],
  carrierFrames : [Clients -> CarrierFrameSequences],
  carrierPairKinds : [Clients -> PairKinds],
  slowCandidates : SUBSET Clients,
  nextGeneration : 1..(MaxGeneration + 1),
  staleGenerationRefs : SUBSET GenerationRefs,
  workerRegistered : BOOLEAN,
  handlerSaturationObserved : BOOLEAN,
  terminalEffectCount : Nat,
  workerRequestCount : Nat
]

VARIABLE state
vars == <<state>>

ActiveClients == {client \in Clients : state.sessions[client] # 0}
ActiveGenerations == {state.sessions[client] : client \in ActiveClients}
HandlerCount == Cardinality(ActiveClients) + Cardinality(state.slowCandidates)

ObcsSymmetry == {
  clientPermutation @@ requestPermutation :
    clientPermutation \in Permutations(Clients),
    requestPermutation \in Permutations(RequestIds)
}

Init ==
  state = [
    serverPhase |-> "listening",
    sessions |-> [client \in Clients |-> 0],
    sessionPhases |-> [client \in Clients |-> "closed"],
    outstandingRequests |->
      [client \in Clients |-> NoOutstandingRequest],
    carrierFrames |-> [client \in Clients |-> <<>>],
    carrierPairKinds |-> [client \in Clients |-> NoPair],
    slowCandidates |-> {},
    nextGeneration |-> 1,
    staleGenerationRefs |-> {},
    workerRegistered |-> FALSE,
    handlerSaturationObserved |-> FALSE,
    terminalEffectCount |-> 0,
    workerRequestCount |-> 0
  ]

BeginSlowCandidate(client) ==
  /\ Scenario = "stream-faults"
  /\ state.serverPhase = "listening"
  /\ client \in Clients
  /\ state.sessions[client] = 0
  /\ client \notin state.slowCandidates
  /\ HandlerCount < MaxHandlers
  /\ state' = [state EXCEPT !.slowCandidates = @ \cup {client}]

ReleaseSlowCandidate(client) ==
  /\ Scenario = "stream-faults"
  /\ state.serverPhase = "listening"
  /\ client \in state.slowCandidates
  /\ state' = [state EXCEPT !.slowCandidates = @ \ {client}]

CanAdmit(client) ==
  /\ state.serverPhase = "listening"
  /\ client \in Clients
  /\ state.sessions[client] = 0
  /\ client \notin state.slowCandidates
  /\ Cardinality(ActiveClients) < MaxSessions
  /\ HandlerCount < MaxHandlers
  /\ state.nextGeneration <= MaxGeneration

QueueOblcAccepted(client, frames, pairKind) ==
  /\ CanAdmit(client)
  /\ frames \in CarrierFrameSequences
  /\ pairKind \in PairKinds
  /\ (Len(frames) = 2) = (pairKind # NoPair)
  /\ (pairKind = "duplicate" =>
       frames[1].requestId = frames[2].requestId)
  /\ (pairKind = "distinct" =>
       frames[1].requestId # frames[2].requestId)
  /\ state' = [state EXCEPT
       !.sessions[client] = state.nextGeneration,
       !.sessionPhases[client] = "oblc-accepted-queued",
       !.carrierFrames[client] = frames,
       !.carrierPairKinds[client] = pairKind,
       !.nextGeneration = @ + 1]

QueueOblcAcceptedWithoutBufferedFrame(client) ==
  QueueOblcAccepted(client, <<>>, NoPair)

QueueOblcAcceptedWithBufferedCreate(client, requestId) ==
  /\ Scenario = "correlation"
  /\ requestId \in RequestIds
  /\ QueueOblcAccepted(client, <<CreateRequest(requestId)>>, NoPair)

QueueOblcAcceptedWithBufferedStart(client, requestId, terminalCreateRequestId) ==
  /\ Scenario = "correlation"
  /\ requestId \in RequestIds
  /\ terminalCreateRequestId \in RequestIds \ {requestId}
  /\ QueueOblcAccepted(
       client,
       <<StartRequest(requestId, terminalCreateRequestId)>>,
       NoPair)

QueueOblcAcceptedWithDuplicateFrames(client, requestId) ==
  /\ Scenario = "correlation"
  /\ requestId \in RequestIds
  /\ QueueOblcAccepted(
       client,
       <<CreateRequest(requestId), CreateRequest(requestId)>>,
       "duplicate")

QueueOblcAcceptedWithDistinctFrames(client, first, second) ==
  /\ Scenario = "correlation"
  /\ first \in RequestIds
  /\ second \in RequestIds \ {first}
  /\ QueueOblcAccepted(
       client,
       <<CreateRequest(first), StartRequest(second, first)>>,
       "distinct")

WriteOblcAccepted(client) ==
  /\ state.serverPhase = "listening"
  /\ state.sessionPhases[client] = "oblc-accepted-queued"
  /\ state' = [state EXCEPT !.sessionPhases[client] = "obcs-idle"]

StallObcsRead(client) ==
  /\ Scenario = "stream-faults"
  /\ state.serverPhase = "listening"
  /\ state.sessionPhases[client] = "obcs-idle"
  /\ state' = [state EXCEPT
       !.sessionPhases[client] = "obcs-read-stalled"]

ResumeObcsRead(client) ==
  /\ Scenario = "stream-faults"
  /\ state.serverPhase = "listening"
  /\ state.sessionPhases[client] = "obcs-read-stalled"
  /\ state' = [state EXCEPT !.sessionPhases[client] = "obcs-idle"]

ReadPartialHeader(client) ==
  /\ Scenario = "stream-faults"
  /\ state.serverPhase = "listening"
  /\ state.sessionPhases[client] \in {"obcs-idle", "obcs-read-stalled"}
  /\ state' = [state EXCEPT
       !.sessionPhases[client] = "obcs-partial-header"]

ReadPartialPayload(client) ==
  /\ Scenario = "stream-faults"
  /\ state.serverPhase = "listening"
  /\ state.sessionPhases[client] = "obcs-partial-header"
  /\ state' = [state EXCEPT
       !.sessionPhases[client] = "obcs-partial-payload"]

QueueDeniedResult(client, request, resultPhase) ==
  /\ state.serverPhase = "listening"
  /\ state.sessionPhases[client] \in ObcsReadPhases
  /\ state.outstandingRequests[client] = NoOutstandingRequest
  /\ request \in OutstandingRequestValues \ {NoOutstandingRequest}
  /\ resultPhase \in {
       "obcs-create-result-ready", "obcs-start-result-ready"
     }
  /\ state' = [state EXCEPT
       !.outstandingRequests[client] = request,
       !.sessionPhases[client] = resultPhase]

ReceiveImmediateCreate(client, requestId, workerConstraint, cwd) ==
  /\ requestId \in RequestIds
  /\ workerConstraint \in WorkerConstraints
  /\ cwd \in WorkingDirectories
  /\ state.carrierFrames[client] = <<>>
  /\ QueueDeniedResult(
       client, CreateRequest(requestId), "obcs-create-result-ready")

ReceiveDeferredCreate(client, requestId, workerConstraint, cwd) ==
  /\ requestId \in RequestIds
  /\ workerConstraint \in WorkerConstraints
  /\ cwd \in WorkingDirectories
  /\ state.carrierFrames[client] = <<>>
  /\ QueueDeniedResult(
       client, CreateRequest(requestId), "obcs-create-result-ready")

ReceiveStart(client, requestId, terminalCreateRequestId) ==
  /\ requestId \in RequestIds
  /\ terminalCreateRequestId \in RequestIds \ {requestId}
  /\ state.carrierFrames[client] = <<>>
  /\ QueueDeniedResult(
       client,
       StartRequest(requestId, terminalCreateRequestId),
       "obcs-start-result-ready")

ReceiveBufferedFrameCore(client) ==
  LET request == Head(state.carrierFrames[client]) IN
  /\ state.serverPhase = "listening"
  /\ state.sessionPhases[client] \in ObcsReadPhases
  /\ state.outstandingRequests[client] = NoOutstandingRequest
  /\ Len(state.carrierFrames[client]) \in {1, 2}
  /\ state' = [state EXCEPT
       !.outstandingRequests[client] = request,
       !.sessionPhases[client] =
         IF request.kind = "create"
         THEN "obcs-create-result-ready"
         ELSE "obcs-start-result-ready",
       !.carrierFrames[client] = Tail(@),
       !.carrierPairKinds[client] =
         IF Len(state.carrierFrames[client]) = 1 THEN NoPair ELSE @]

ReceiveBufferedCreate(client) ==
  /\ Scenario = "correlation"
  /\ state.carrierPairKinds[client] = NoPair
  /\ Len(state.carrierFrames[client]) = 1
  /\ Head(state.carrierFrames[client]).kind = "create"
  /\ ReceiveBufferedFrameCore(client)

ReceiveBufferedStart(client) ==
  /\ Scenario = "correlation"
  /\ state.carrierPairKinds[client] = NoPair
  /\ Len(state.carrierFrames[client]) = 1
  /\ Head(state.carrierFrames[client]).kind = "start"
  /\ ReceiveBufferedFrameCore(client)

ReceiveFirstBufferedDuplicate(client) ==
  /\ Scenario = "correlation"
  /\ Len(state.carrierFrames[client]) = 2
  /\ state.carrierPairKinds[client] = "duplicate"
  /\ ReceiveBufferedFrameCore(client)

ReceiveSecondBufferedDuplicate(client) ==
  /\ Scenario = "correlation"
  /\ Len(state.carrierFrames[client]) = 1
  /\ state.carrierPairKinds[client] = "duplicate"
  /\ ReceiveBufferedFrameCore(client)

ReceiveFirstBufferedDistinct(client) ==
  /\ Scenario = "correlation"
  /\ Len(state.carrierFrames[client]) = 2
  /\ state.carrierPairKinds[client] = "distinct"
  /\ ReceiveBufferedFrameCore(client)

ReceiveSecondBufferedDistinct(client) ==
  /\ Scenario = "correlation"
  /\ Len(state.carrierFrames[client]) = 1
  /\ state.carrierPairKinds[client] = "distinct"
  /\ ReceiveBufferedFrameCore(client)

StallObcsResultWrite(client) ==
  /\ state.serverPhase = "listening"
  /\ state.sessionPhases[client] \in {
       "obcs-create-result-ready", "obcs-start-result-ready"
     }
  /\ state' = [state EXCEPT
       !.sessionPhases[client] =
         IF @ = "obcs-create-result-ready"
         THEN "obcs-create-write-stalled"
         ELSE "obcs-start-write-stalled"]

WriteDeniedResult(client, request, result, phases) ==
  /\ state.serverPhase = "listening"
  /\ phases \subseteq ObcsResultPhases
  /\ state.sessionPhases[client] \in phases
  /\ state.outstandingRequests[client] = request
  /\ result = DeniedProjection(request)
  /\ result.requestId = request.requestId
  /\ result.outcome.kind = "rejected"
  /\ result.outcome.reason = "not-authorized"
  /\ state' = [state EXCEPT
       !.sessionPhases[client] = "obcs-idle",
       !.outstandingRequests[client] = NoOutstandingRequest]

WriteCreateResult(client, requestId) ==
  /\ requestId \in RequestIds
  /\ WriteDeniedResult(
       client,
       CreateRequest(requestId),
       DeniedResult("terminal.create.result", requestId),
       CreateResultPhases)

WriteStartResult(client, requestId, terminalCreateRequestId) ==
  /\ requestId \in RequestIds
  /\ terminalCreateRequestId \in RequestIds \ {requestId}
  /\ WriteDeniedResult(
       client,
       StartRequest(requestId, terminalCreateRequestId),
       DeniedResult("terminal.start.result", requestId),
       StartResultPhases)

RegisterWorker ==
  /\ Scenario = "stream-faults"
  /\ state.serverPhase = "listening"
  /\ ~state.workerRegistered
  /\ state' = [state EXCEPT !.workerRegistered = TRUE]

\* This proof observation makes the silent handler-saturation path countable;
\* no admission or OBCS decision reads it.
ObserveHandlerSaturation ==
  /\ Scenario = "stream-faults"
  /\ state.serverPhase = "listening"
  /\ HandlerCount = MaxHandlers
  /\ ~state.handlerSaturationObserved
  /\ state' = [state EXCEPT !.handlerSaturationObserved = TRUE]

CloseCurrentSession(client) ==
  LET generation == state.sessions[client] IN
  /\ state.serverPhase = "listening"
  /\ client \in ActiveClients
  /\ state' = [state EXCEPT
       !.sessions[client] = 0,
       !.sessionPhases[client] = "closed",
       !.outstandingRequests[client] = NoOutstandingRequest,
       !.carrierFrames[client] = <<>>,
       !.carrierPairKinds[client] = NoPair,
       !.staleGenerationRefs =
         @ \cup {GenerationRef(client, generation)}]

StrictMalformedObcsFrame(client) ==
  /\ Scenario = "stream-faults"
  /\ state.sessionPhases[client] \in ObcsReadPhases
  /\ CloseCurrentSession(client)

WrongDirectionObcsFrame(client) ==
  /\ Scenario = "stream-faults"
  /\ state.sessionPhases[client] \in ObcsReadPhases
  /\ CloseCurrentSession(client)

OversizedObcsFrame(client) ==
  /\ Scenario = "stream-faults"
  /\ state.sessionPhases[client] \in ObcsReadPhases
  /\ CloseCurrentSession(client)

TruncatedObcsFrame(client) ==
  /\ Scenario = "stream-faults"
  /\ state.sessionPhases[client] \in ObcsPartialPhases
  /\ CloseCurrentSession(client)

CleanSessionEof(client, generation) ==
  /\ generation = state.sessions[client]
  /\ state.sessionPhases[client] \notin ObcsPartialPhases
  /\ CloseCurrentSession(client)

SessionIoFailure(client, generation) ==
  /\ generation = state.sessions[client]
  /\ CloseCurrentSession(client)

ObcsResultWriteFailure(client) ==
  /\ state.sessionPhases[client] \in ObcsResultPhases
  /\ CloseCurrentSession(client)

RemoveStaleRef(client, generation) ==
  LET ref == GenerationRef(client, generation) IN
  /\ ref \in state.staleGenerationRefs
  /\ state.sessions[client] # generation
  /\ state' = [state EXCEPT !.staleGenerationRefs = @ \ {ref}]

StaleSessionEof(client, generation) == RemoveStaleRef(client, generation)
StaleSessionCleanup(client, generation) == RemoveStaleRef(client, generation)

StopServer ==
  /\ state.serverPhase = "listening"
  /\ state' = [state EXCEPT
       !.serverPhase = "stopped",
       !.sessions = [client \in Clients |-> 0],
       !.sessionPhases = [client \in Clients |-> "closed"],
       !.outstandingRequests =
         [client \in Clients |-> NoOutstandingRequest],
       !.carrierFrames = [client \in Clients |-> <<>>],
       !.carrierPairKinds = [client \in Clients |-> NoPair],
       !.slowCandidates = {},
       !.staleGenerationRefs = {},
       !.workerRegistered = FALSE]

\* Coverage aliases constrain real transitions without adding Server history.
ReceiveCoalescedCreateAfterAcceptance ==
  /\ Scenario = "correlation"
  /\ \E client \in Clients:
       /\ state.sessionPhases[client] = "obcs-idle"
       /\ Len(state.carrierFrames[client]) = 1
       /\ ReceiveBufferedCreate(client)

ReceiveCoalescedStartAfterAcceptance ==
  /\ Scenario = "correlation"
  /\ \E client \in Clients:
       /\ state.sessionPhases[client] = "obcs-idle"
       /\ Len(state.carrierFrames[client]) = 1
       /\ ReceiveBufferedStart(client)

ReceiveSameRequestIdInOtherSession ==
  /\ Scenario = "correlation"
  /\ \E first \in Clients:
       \E second \in Clients \ {first}:
         \E requestId \in RequestIds:
           /\ state.outstandingRequests[first] = CreateRequest(requestId)
           /\ ReceiveImmediateCreate(
                second, requestId, "none", "null")

WriteResultWhileOtherPhase(otherPhase) ==
  \E stalled \in Clients:
    \E writer \in Clients \ {stalled}:
      \E requestId \in RequestIds:
        /\ state.sessionPhases[stalled] = otherPhase
        /\ WriteCreateResult(writer, requestId)

OtherSessionCompletesDuringReadStall ==
  /\ Scenario = "stream-faults"
  /\ WriteResultWhileOtherPhase("obcs-read-stalled")

OtherSessionCompletesDuringPartialHeader ==
  /\ Scenario = "stream-faults"
  /\ WriteResultWhileOtherPhase("obcs-partial-header")

OtherSessionCompletesDuringPartialPayload ==
  /\ Scenario = "stream-faults"
  /\ WriteResultWhileOtherPhase("obcs-partial-payload")

OtherSessionCompletesDuringWriteStall ==
  /\ Scenario = "stream-faults"
  /\ WriteResultWhileOtherPhase("obcs-create-write-stalled")

OtherSessionCompletesWhileSlowCandidate ==
  /\ Scenario = "stream-faults"
  /\ \E slow \in state.slowCandidates:
       \E writer \in Clients \ {slow}:
         \E requestId \in RequestIds:
           WriteCreateResult(writer, requestId)

RegisterWorkerWhileObcsStalled ==
  /\ Scenario = "stream-faults"
  /\ \E client \in ActiveClients:
       state.sessionPhases[client] \in ObcsStalledPhases
  /\ RegisterWorker

ReceiveCreateWithoutWorker ==
  /\ Scenario = "stream-faults"
  /\ \E client \in Clients:
       \E requestId \in RequestIds:
         /\ ~state.workerRegistered
         /\ ReceiveImmediateCreate(
              client, requestId, "exact-absent", "absolute")

ReceiveCreateWithWorker ==
  /\ Scenario = "stream-faults"
  /\ \E client \in Clients:
       \E requestId \in RequestIds:
         /\ state.workerRegistered
         /\ ReceiveDeferredCreate(
              client, requestId, "exact-present", "absolute")

RejectHandlerSaturationWithStalledObcs ==
  /\ Scenario = "stream-faults"
  /\ \E client \in ActiveClients:
       state.sessionPhases[client] \in ObcsStalledPhases
  /\ ObserveHandlerSaturation

StaleEofAfterReplacement ==
  \E client \in ActiveClients:
    \E generation \in 1..MaxGeneration:
      StaleSessionEof(client, generation)

CleanupStaleAfterReplacement ==
  \E client \in ActiveClients:
    \E generation \in 1..MaxGeneration:
      StaleSessionCleanup(client, generation)

CommonNext ==
  \/ \E client \in Clients: QueueOblcAcceptedWithoutBufferedFrame(client)
  \/ \E client \in Clients: WriteOblcAccepted(client)
  \/ \E client \in Clients:
       \E requestId \in RequestIds:
         \E workerConstraint \in WorkerConstraints:
           \E cwd \in WorkingDirectories:
             ReceiveImmediateCreate(client, requestId, workerConstraint, cwd)
  \/ \E client \in Clients:
       \E requestId \in RequestIds:
         \E workerConstraint \in WorkerConstraints:
           \E cwd \in WorkingDirectories:
             ReceiveDeferredCreate(client, requestId, workerConstraint, cwd)
  \/ \E client \in Clients:
       \E requestId \in RequestIds:
         \E terminalCreateRequestId \in RequestIds:
           ReceiveStart(client, requestId, terminalCreateRequestId)
  \/ \E client \in Clients: StallObcsResultWrite(client)
  \/ \E client \in Clients:
       \E requestId \in RequestIds: WriteCreateResult(client, requestId)
  \/ \E client \in Clients:
       \E requestId \in RequestIds:
         \E terminalCreateRequestId \in RequestIds:
           WriteStartResult(client, requestId, terminalCreateRequestId)
  \/ \E client \in Clients:
       \E generation \in 1..MaxGeneration:
         CleanSessionEof(client, generation)
  \/ \E client \in Clients:
       \E generation \in 1..MaxGeneration:
         SessionIoFailure(client, generation)
  \/ \E client \in Clients: ObcsResultWriteFailure(client)
  \/ StaleEofAfterReplacement
  \/ CleanupStaleAfterReplacement
  \/ StopServer

Next ==
  \/ CommonNext
  \/ \E client \in Clients: BeginSlowCandidate(client)
  \/ \E client \in Clients: ReleaseSlowCandidate(client)
  \/ \E client \in Clients: StallObcsRead(client)
  \/ \E client \in Clients: ResumeObcsRead(client)
  \/ \E client \in Clients: ReadPartialHeader(client)
  \/ \E client \in Clients: ReadPartialPayload(client)
  \/ \E client \in Clients: StrictMalformedObcsFrame(client)
  \/ \E client \in Clients: WrongDirectionObcsFrame(client)
  \/ \E client \in Clients: OversizedObcsFrame(client)
  \/ \E client \in Clients: TruncatedObcsFrame(client)
  \/ RegisterWorker
  \/ ObserveHandlerSaturation
  \/ OtherSessionCompletesDuringReadStall
  \/ OtherSessionCompletesDuringPartialHeader
  \/ OtherSessionCompletesDuringPartialPayload
  \/ OtherSessionCompletesDuringWriteStall
  \/ OtherSessionCompletesWhileSlowCandidate
  \/ RegisterWorkerWhileObcsStalled
  \/ ReceiveCreateWithoutWorker
  \/ ReceiveCreateWithWorker
  \/ RejectHandlerSaturationWithStalledObcs
  \/ \E client \in Clients:
       \E requestId \in RequestIds:
         QueueOblcAcceptedWithBufferedCreate(client, requestId)
  \/ \E client \in Clients:
       \E requestId \in RequestIds:
         \E terminalCreateRequestId \in RequestIds:
           QueueOblcAcceptedWithBufferedStart(
             client, requestId, terminalCreateRequestId)
  \/ \E client \in Clients:
       \E requestId \in RequestIds:
         QueueOblcAcceptedWithDuplicateFrames(client, requestId)
  \/ \E client \in Clients:
       \E first \in RequestIds:
         \E second \in RequestIds:
           QueueOblcAcceptedWithDistinctFrames(client, first, second)
  \/ \E client \in Clients: ReceiveBufferedCreate(client)
  \/ \E client \in Clients: ReceiveBufferedStart(client)
  \/ \E client \in Clients: ReceiveFirstBufferedDuplicate(client)
  \/ \E client \in Clients: ReceiveSecondBufferedDuplicate(client)
  \/ \E client \in Clients: ReceiveFirstBufferedDistinct(client)
  \/ \E client \in Clients: ReceiveSecondBufferedDistinct(client)
  \/ ReceiveCoalescedCreateAfterAcceptance
  \/ ReceiveCoalescedStartAfterAcceptance
  \/ ReceiveSameRequestIdInOtherSession

Spec == Init /\ [][Next]_vars

TypeOK == state \in StateValues

SessionShapeIsConsistent ==
  \A client \in Clients:
    /\ (state.sessions[client] = 0) =
         (state.sessionPhases[client] = "closed")
    /\ (state.sessionPhases[client] \in ObcsResultPhases) =
         (state.outstandingRequests[client] # NoOutstandingRequest)
    /\ (state.sessionPhases[client] \in CreateResultPhases) =
         (state.outstandingRequests[client].kind = "create")
    /\ (state.sessionPhases[client] \in StartResultPhases) =
         (state.outstandingRequests[client].kind = "start")
    /\ ((state.sessionPhases[client] = "closed") =>
         /\ state.outstandingRequests[client] = NoOutstandingRequest
         /\ state.carrierFrames[client] = <<>>
         /\ state.carrierPairKinds[client] = NoPair)
    /\ (state.carrierPairKinds[client] = NoPair =>
         Len(state.carrierFrames[client]) \in {0, 1})
    /\ (Len(state.carrierFrames[client]) = 2 =>
         state.carrierPairKinds[client] # NoPair)
    /\ (state.carrierPairKinds[client] = "duplicate" =>
         /\ Len(state.carrierFrames[client]) \in {1, 2}
         /\ (Len(state.carrierFrames[client]) = 2 =>
              state.carrierFrames[client][1].requestId =
                state.carrierFrames[client][2].requestId))
    /\ (state.carrierPairKinds[client] = "distinct" =>
         /\ Len(state.carrierFrames[client]) \in {1, 2}
         /\ (Len(state.carrierFrames[client]) = 2 =>
              state.carrierFrames[client][1].requestId #
                state.carrierFrames[client][2].requestId))

RequestReferencesMatchKind ==
  \A client \in Clients:
    LET request == state.outstandingRequests[client] IN
    /\ (request.kind = "create" =>
         request.terminalCreateRequestId = NoRequestId)
    /\ (request.kind = "start" =>
         request.terminalCreateRequestId \in
           RequestIds \ {request.requestId})

SessionCapacityIsBounded == Cardinality(ActiveClients) <= MaxSessions
HandlerCapacityIsBounded == HandlerCount <= MaxHandlers

EveryActiveSessionOwnsOneHandler ==
  HandlerCount = Cardinality(ActiveClients) + Cardinality(state.slowCandidates)

SlowCandidatesOwnNoSession ==
  \A client \in state.slowCandidates: state.sessions[client] = 0

ActiveGenerationsAreUnique ==
  Cardinality(ActiveGenerations) = Cardinality(ActiveClients)

ActiveGenerationsNeverRepeat ==
  \A client \in ActiveClients:
    state.sessions[client] < state.nextGeneration

StaleRefsAreNotCurrent ==
  \A ref \in state.staleGenerationRefs:
    state.sessions[ref.client] # ref.generation

QueuedAcceptanceHasNoObcsResult ==
  \A client \in Clients:
    state.sessionPhases[client] = "oblc-accepted-queued" =>
      state.outstandingRequests[client] = NoOutstandingRequest

OutstandingRequestsProjectOnlyNotAuthorized ==
  \A client \in Clients:
    state.outstandingRequests[client] # NoOutstandingRequest =>
      LET result == DeniedProjection(state.outstandingRequests[client]) IN
      /\ result.protocolVersion = 1
      /\ result.requestId = state.outstandingRequests[client].requestId
      /\ result.requestId \in RequestIds
      /\ result.outcome.kind = "rejected"
      /\ result.outcome.reason = "not-authorized"
      /\ result.kind \in {
           "terminal.create.result", "terminal.start.result"
         }

NoTerminalOrWorkerRequestEffect ==
  /\ state.terminalEffectCount = 0
  /\ state.workerRequestCount = 0

StoppedServerOwnsNoConnection ==
  state.serverPhase = "stopped" =>
    /\ ActiveClients = {}
    /\ state.slowCandidates = {}
    /\ state.staleGenerationRefs = {}
    /\ ~state.workerRegistered

\* There is deliberately no WF/SF: byte arrival, writable sockets, EOF,
\* disconnect, and peer loss may stall indefinitely without blocking another
\* Session or the independent Worker registration action.

=============================================================================
