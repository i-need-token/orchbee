---------------------- MODULE LocalWorkerObcnCarrier ----------------------
EXTENDS FiniteSets, Naturals, TLC

\* Models the Server-owned carrier after one OBLS WorkerAccepted response is
\* completely written. The registry publishes a generation-scoped request
\* handle, while one WorkerWireOwner alone owns the stream. The handle has one
\* submitted slot, and the wire owner has at most one request. The three
\* operation identities include two Launch operations, so correlation is not
\* reducible to result kind and the third request can encounter a genuinely
\* full carrier.
\*
\* An incoming response is environment-owned state. ReceiveExactWorkerResult
\* validates and retains that exact state; it never constructs a replacement.
\* The response domain distinguishes direction, result kind, every echoed
\* authorization field, the Start target, and the Launch OutcomeUnknown Host
\* epoch. A finite non-resetting deadline covers the complete request write,
\* then CompleteRequestWrite installs a fresh finite non-resetting deadline
\* for the complete result read. External response and disconnect actions have
\* no fairness assumption; locally fair time advancement and expiry settle a
\* silent peer.
\*
\* OBCS policy, Worker authorization/admission semantics, Host effects,
\* retries initiated by another owner, and durable recovery are outside this
\* carrier model.

CONSTANTS
  Workers,
  NoWorker,
  FirstTerminal,
  SecondTerminal,
  NoTerminal,
  FirstLaunchOperation,
  SecondLaunchOperation,
  StartOperation,
  NoOperation,
  FirstLease,
  SecondLease,
  ThirdLease,
  NoLease,
  FirstHostCreation,
  SecondHostCreation,
  NoHostCreation,
  FirstHostTerminal,
  SecondHostTerminal,
  NoHostTerminal,
  FirstDefiniteOutcomePayload,
  SecondDefiniteOutcomePayload,
  NoOutcomePayload,
  MaxGeneration,
  MaxFence,
  MaxLeaseExpiry,
  MaxHostEpoch,
  MaxTick,
  Scenario

ASSUME
  /\ IsFiniteSet(Workers)
  /\ Cardinality(Workers) >= 2
  /\ NoWorker \notin Workers
  /\ FirstTerminal # SecondTerminal
  /\ NoTerminal \notin {FirstTerminal, SecondTerminal}
  /\ Cardinality({
       FirstLaunchOperation,
       SecondLaunchOperation,
       StartOperation
     }) = 3
  /\ NoOperation \notin {
       FirstLaunchOperation,
       SecondLaunchOperation,
       StartOperation
     }
  /\ Cardinality({FirstLease, SecondLease, ThirdLease}) = 3
  /\ NoLease \notin {FirstLease, SecondLease, ThirdLease}
  /\ FirstHostCreation # SecondHostCreation
  /\ NoHostCreation \notin {FirstHostCreation, SecondHostCreation}
  /\ FirstHostTerminal # SecondHostTerminal
  /\ NoHostTerminal \notin {FirstHostTerminal, SecondHostTerminal}
  /\ Cardinality({
       FirstDefiniteOutcomePayload,
       SecondDefiniteOutcomePayload
     }) = 2
  /\ NoOutcomePayload \notin {
       FirstDefiniteOutcomePayload,
       SecondDefiniteOutcomePayload
     }
  /\ MaxGeneration \in Nat
  /\ MaxGeneration >= 2
  /\ MaxFence \in Nat
  /\ MaxFence >= 2
  /\ MaxLeaseExpiry \in Nat
  /\ MaxLeaseExpiry >= 3
  /\ MaxHostEpoch \in Nat
  /\ MaxHostEpoch >= 2
  /\ MaxTick \in Nat
  /\ MaxTick >= 3

LaunchOperations == {FirstLaunchOperation, SecondLaunchOperation}
Operations == LaunchOperations \cup {StartOperation}
Terminals == {FirstTerminal, SecondTerminal}
LeaseIds == {FirstLease, SecondLease, ThirdLease}
Generations == 1..MaxGeneration
Fences == 1..MaxFence
LeaseExpiries == 1..MaxLeaseExpiry
Capabilities == 1..2
HostEpochs == 1..MaxHostEpoch
DefiniteOutcomePayloads == {
  FirstDefiniteOutcomePayload,
  SecondDefiniteOutcomePayload
}
Scenarios == {
  "response-validation",
  "disconnect-order",
  "stale-routing",
  "capacity"
}

ASSUME Scenario \in Scenarios

WorkerRefs == [worker : Workers, generation : Generations]

WorkerRef(worker, generation) == [
  worker |-> worker,
  generation |-> generation
]

NoWorkerRef == WorkerRef(NoWorker, 0)
WorkerRefValues == WorkerRefs \cup {NoWorkerRef}

HostIdentity(creation, epoch, terminal) == [
  creation |-> creation,
  epoch |-> epoch,
  terminal |-> terminal
]

StartTarget == HostIdentity(FirstHostCreation, 1, FirstHostTerminal)
OtherStartTarget == HostIdentity(
  SecondHostCreation,
  2,
  SecondHostTerminal
)
NoTarget == HostIdentity(NoHostCreation, 0, NoHostTerminal)
HostTargetValues == {NoTarget, StartTarget, OtherStartTarget}

OperationKind(operation) ==
  IF operation \in LaunchOperations THEN "launch" ELSE "start"

TerminalForOperation(operation) ==
  IF operation = SecondLaunchOperation
  THEN SecondTerminal
  ELSE FirstTerminal

FenceForOperation(operation) ==
  IF operation = StartOperation THEN 2 ELSE 1

LeaseForOperation(operation) ==
  CASE operation = FirstLaunchOperation -> FirstLease
    [] operation = SecondLaunchOperation -> SecondLease
    [] OTHER -> ThirdLease

LeaseExpiryForOperation(operation) ==
  CASE operation = FirstLaunchOperation -> 1
    [] operation = SecondLaunchOperation -> 2
    [] OTHER -> 3

Authorization(
  terminal,
  operation,
  worker,
  generation,
  fence,
  leaseId,
  leaseExpiry,
  capability
) == [
  terminal |-> terminal,
  operation |-> operation,
  worker |-> worker,
  generation |-> generation,
  fence |-> fence,
  leaseId |-> leaseId,
  leaseExpiry |-> leaseExpiry,
  capability |-> capability
]

ExpectedAuthorization(ref, operation) == Authorization(
  TerminalForOperation(operation),
  operation,
  ref.worker,
  ref.generation,
  FenceForOperation(operation),
  LeaseForOperation(operation),
  LeaseExpiryForOperation(operation),
  2
)

NoAuthorization == Authorization(
  NoTerminal,
  NoOperation,
  NoWorker,
  0,
  0,
  NoLease,
  0,
  0
)

Request(ref, operation) == [
  kind |-> OperationKind(operation),
  authorization |-> ExpectedAuthorization(ref, operation),
  target |-> IF operation = StartOperation THEN StartTarget ELSE NoTarget
]

NoRequest == [
  kind |-> "none",
  authorization |-> NoAuthorization,
  target |-> NoTarget
]

Requests == {
  Request(ref, operation) :
    ref \in WorkerRefs,
    operation \in Operations
}
RequestValues == Requests \cup {NoRequest}

RequestOperation(request) == request.authorization.operation
RequestRef(request) == WorkerRef(
  request.authorization.worker,
  request.authorization.generation
)

OtherTerminal(terminal) ==
  IF terminal = FirstTerminal THEN SecondTerminal ELSE FirstTerminal

OtherWorker(worker) ==
  CHOOSE candidate \in Workers: candidate # worker

OtherGeneration(generation) ==
  CHOOSE candidate \in Generations: candidate # generation

OtherFence(fence) ==
  IF fence = 1 THEN 2 ELSE 1

OtherLeaseId(leaseId) ==
  IF leaseId = FirstLease THEN SecondLease ELSE FirstLease

OtherLeaseExpiry(expiry) ==
  IF expiry = 1 THEN 2 ELSE 1

OtherCapability(capability) ==
  IF capability = 1 THEN 2 ELSE 1

OtherSameKindOperation(operation) ==
  IF operation = FirstLaunchOperation
  THEN SecondLaunchOperation
  ELSE FirstLaunchOperation

WrongTerminalAuthorization(authorization) ==
  [authorization EXCEPT
    !.terminal = OtherTerminal(authorization.terminal)]

WrongWorkerAuthorization(authorization) ==
  [authorization EXCEPT
    !.worker = OtherWorker(authorization.worker)]

WrongGenerationAuthorization(authorization) ==
  [authorization EXCEPT
    !.generation = OtherGeneration(authorization.generation)]

WrongFenceAuthorization(authorization) ==
  [authorization EXCEPT
    !.fence = OtherFence(authorization.fence)]

WrongLeaseIdAuthorization(authorization) ==
  [authorization EXCEPT
    !.leaseId = OtherLeaseId(authorization.leaseId)]

WrongLeaseExpiryAuthorization(authorization) ==
  [authorization EXCEPT
    !.leaseExpiry = OtherLeaseExpiry(authorization.leaseExpiry)]

WrongCapabilityAuthorization(authorization) ==
  [authorization EXCEPT
    !.capability = OtherCapability(authorization.capability)]

WrongOperationAuthorization(authorization) ==
  [authorization EXCEPT
    !.operation = OtherSameKindOperation(authorization.operation)]

Outcome(classification, payload, launchUnknownHostEpoch) == [
  classification |-> classification,
  payload |-> payload,
  launchUnknownHostEpoch |-> launchUnknownHostEpoch
]

DefiniteOutcome(payload) == Outcome("definite", payload, 0)
DefiniteOutcomes == {
  DefiniteOutcome(payload) : payload \in DefiniteOutcomePayloads
}
CanonicalDefiniteOutcome == DefiniteOutcome(FirstDefiniteOutcomePayload)
LaunchUnknownOutcome(epoch) ==
  Outcome("outcome-unknown", NoOutcomePayload, epoch)
StartUnknownOutcome ==
  Outcome("outcome-unknown", NoOutcomePayload, 0)
NoOutcome == Outcome("none", NoOutcomePayload, 0)

Directions == {"server-to-worker", "worker-to-server"}
ResultKinds == {"launch-result", "start-result"}

ExpectedResultKind(request) ==
  IF request.kind = "launch" THEN "launch-result" ELSE "start-result"

OtherResultKind(resultKind) ==
  IF resultKind = "launch-result"
  THEN "start-result"
  ELSE "launch-result"

Response(direction, resultKind, authorization, target, outcome) == [
  direction |-> direction,
  kind |-> resultKind,
  authorization |-> authorization,
  target |-> target,
  outcome |-> outcome
]

NoResponse == Response(
  "none",
  "none",
  NoAuthorization,
  NoTarget,
  NoOutcome
)

DefiniteResponse(request, payload) == Response(
  "worker-to-server",
  ExpectedResultKind(request),
  request.authorization,
  request.target,
  DefiniteOutcome(payload)
)

ExactResponseValues(request) ==
  {
    DefiniteResponse(request, payload) :
      payload \in DefiniteOutcomePayloads
  } \cup
  IF request.kind = "launch"
  THEN {
    Response(
      "worker-to-server",
      ExpectedResultKind(request),
      request.authorization,
      request.target,
      LaunchUnknownOutcome(epoch)
    ) : epoch \in HostEpochs
  }
  ELSE {
    Response(
      "worker-to-server",
      ExpectedResultKind(request),
      request.authorization,
      request.target,
      StartUnknownOutcome
    )
  }

AuthorizationTypeOK(authorization) ==
  /\ authorization.terminal \in Terminals
  /\ authorization.operation \in Operations
  /\ authorization.worker \in Workers
  /\ authorization.generation \in Generations
  /\ authorization.fence \in Fences
  /\ authorization.leaseId \in LeaseIds
  /\ authorization.leaseExpiry \in LeaseExpiries
  /\ authorization.capability \in Capabilities

OutcomeTypeOK(outcome) ==
  /\ outcome.classification \in {"definite", "outcome-unknown"}
  /\ outcome.payload \in DefiniteOutcomePayloads \cup {NoOutcomePayload}
  /\ outcome.launchUnknownHostEpoch \in 0..MaxHostEpoch

ResponseTypeOK(response) ==
  \/ response = NoResponse
  \/ /\ response.direction \in Directions
     /\ response.kind \in ResultKinds
     /\ AuthorizationTypeOK(response.authorization)
     /\ response.target \in HostTargetValues
     /\ OutcomeTypeOK(response.outcome)

OutcomeValidForRequest(request, outcome) ==
  \/ outcome \in DefiniteOutcomes
  \/ /\ outcome.classification = "outcome-unknown"
     /\ outcome.payload = NoOutcomePayload
     /\ IF request.kind = "launch"
          THEN outcome.launchUnknownHostEpoch \in HostEpochs
          ELSE outcome.launchUnknownHostEpoch = 0

ResponseMatchesRequest(response, request) ==
  /\ response # NoResponse
  /\ response.direction = "worker-to-server"
  /\ response.kind = ExpectedResultKind(request)
  /\ response.authorization = request.authorization
  /\ response.target = request.target
  /\ OutcomeValidForRequest(request, response.outcome)

CompletionClasses == {
  "known-unchanged", "exact-result", "effect-unknown"
}

Completion(classification, request, response) == [
  classification |-> classification,
  request |-> request,
  response |-> response
]

NoCompletion == Completion("none", NoRequest, NoResponse)
KnownUnchanged(request) ==
  Completion("known-unchanged", request, NoResponse)
ExactCompletion(request, response) ==
  Completion("exact-result", request, response)
EffectUnknown(request) ==
  Completion("effect-unknown", request, NoResponse)

CompletionTypeOK(completion) ==
  \/ completion = NoCompletion
  \/ /\ completion.classification \in CompletionClasses
     /\ completion.request \in Requests
     /\ ResponseTypeOK(completion.response)
     /\ ((completion.classification = "exact-result") <=>
          (completion.response # NoResponse))

WirePhases == {
  "idle",
  "ready-to-write",
  "write-complete",
  "response-ready",
  "result-received"
}

ExactIncomingClasses == {
  "exact-definite",
  "exact-launch-unknown",
  "exact-start-unknown"
}

InvalidIncomingClasses == {
  "wrong-direction",
  "wrong-result-kind",
  "wrong-operation",
  "wrong-terminal",
  "wrong-worker",
  "wrong-generation",
  "wrong-fence",
  "wrong-lease-id",
  "wrong-lease-expiry",
  "wrong-capability",
  "wrong-start-target",
  "invalid-unknown-payload"
}

IncomingClasses ==
  {"none"} \cup ExactIncomingClasses \cup InvalidIncomingClasses

VARIABLE state
vars == <<state>>

CarrierActive == state.activeRef # NoWorkerRef

AttemptedOperations == {
  operation \in Operations :
    state.attemptedRequests[operation] # NoRequest
}

ScenarioAttemptLimit ==
  CASE Scenario = "response-validation" -> 1
    [] Scenario = "capacity" -> 3
    [] OTHER -> 2

FreshOperation(operation) ==
  state.attemptedRequests[operation] = NoRequest

CanBeginAttempt(operation) ==
  /\ FreshOperation(operation)
  /\ Cardinality(AttemptedOperations) < ScenarioAttemptLimit

PutCompletion(completions, request, completion) ==
  [completions EXCEPT ![RequestOperation(request)] = completion]

CompleteKnownUnchanged(completions, request) ==
  IF request = NoRequest
  THEN completions
  ELSE PutCompletion(completions, request, KnownUnchanged(request))

CompleteEffectUnknown(completions, request) ==
  IF request = NoRequest
  THEN completions
  ELSE PutCompletion(completions, request, EffectUnknown(request))

SubmittedOperations ==
  IF state.submittedSlot = NoRequest
  THEN {}
  ELSE {RequestOperation(state.submittedSlot)}

WireOperations ==
  IF state.wireRequest = NoRequest
  THEN {}
  ELSE {RequestOperation(state.wireRequest)}

InFlightOperations ==
  IF state.wirePhase \in {
       "ready-to-write", "write-complete", "response-ready"
     }
  THEN WireOperations
  ELSE {}

PendingOperations == {
  operation \in Operations :
    state.attemptedRequests[operation] # NoRequest /\
    state.completions[operation] = NoCompletion
}

PendingPublications == {
  operation \in Operations :
    state.completions[operation] # NoCompletion /\
    state.publications[operation] = NoCompletion
}

AllAttemptsSettled ==
  /\ PendingOperations = {}
  /\ PendingPublications = {}

Init ==
  state = [
    activeRef |-> NoWorkerRef,
    requestHandleRef |-> NoWorkerRef,
    wireOwnerRef |-> NoWorkerRef,
    nextGeneration |-> 1,
    retiredHandleRefs |-> {},
    rejectedActivationRefs |-> {},
    cleanupRefs |-> {},
    submittedSlot |-> NoRequest,
    wireRequest |-> NoRequest,
    wirePhase |-> "idle",
    wireExpiry |-> 0,
    now |-> 0,
    previousNow |-> 0,
    incomingResponse |-> NoResponse,
    incomingClass |-> "none",
    receivedResponse |-> NoResponse,
    attemptedRequests |-> [operation \in Operations |-> NoRequest],
    observedResponses |-> [operation \in Operations |-> NoResponse],
    observedResponseClasses |-> [operation \in Operations |-> "none"],
    lateIncomingResponses |-> [operation \in Operations |-> NoResponse],
    completions |-> [operation \in Operations |-> NoCompletion],
    publications |-> [operation \in Operations |-> NoCompletion],
    completeWriteCount |-> [operation \in Operations |-> 0],
    effectUnknownOperations |-> {},
    nonRetryableOperations |-> {},
    discardedLateResults |-> {},
    invalidResponseOperations |-> {},
    backpressuredOperations |-> {},
    fullBackpressuredOperations |-> {},
    inactiveStaleHandleOperations |-> {},
    unavailableOperations |-> {},
    notDeliveredOperations |-> {},
    replacementBarrierHeld |-> TRUE
  ]

ActivateCarrier(worker) ==
  /\ worker \in Workers
  /\ ~CarrierActive
  /\ state.requestHandleRef = NoWorkerRef
  /\ state.wireOwnerRef = NoWorkerRef
  /\ state.submittedSlot = NoRequest
  /\ state.wireRequest = NoRequest
  /\ state.wirePhase = "idle"
  /\ state.incomingResponse = NoResponse
  /\ state.receivedResponse = NoResponse
  /\ state.nextGeneration <= MaxGeneration
  /\ AllAttemptsSettled
  /\ LET ref == WorkerRef(worker, state.nextGeneration) IN
       state' = [state EXCEPT
         !.activeRef = ref,
         !.requestHandleRef = ref,
         !.wireOwnerRef = ref,
         !.nextGeneration = @ + 1,
         !.replacementBarrierHeld =
           @ /\ (state.nextGeneration = 1 \/ AllAttemptsSettled)]

ActivateInitialCarrier ==
  /\ state.nextGeneration = 1
  /\ \E worker \in Workers: ActivateCarrier(worker)

ActivateReplacementCarrier ==
  /\ Scenario \in {"disconnect-order", "stale-routing"}
  /\ state.nextGeneration > 1
  /\ \E worker \in Workers: ActivateCarrier(worker)

RejectStaleCarrierActivation ==
  /\ Scenario = "stale-routing"
  /\ CarrierActive
  /\ \E ref \in
       state.retiredHandleRefs \ state.rejectedActivationRefs:
       /\ ref # state.activeRef
       /\ state' = [state EXCEPT
            !.rejectedActivationRefs = @ \cup {ref}]

SubmitCurrent(operation) ==
  /\ operation \in Operations
  /\ CarrierActive
  /\ state.submittedSlot = NoRequest
  /\ state.now < MaxTick
  /\ CanBeginAttempt(operation)
  /\ LET request == Request(state.requestHandleRef, operation) IN
       state' = [state EXCEPT
         !.submittedSlot = request,
         !.attemptedRequests[operation] = request]

SubmitFirstLaunch ==
  /\ FirstLaunchOperation \in LaunchOperations
  /\ state.wireRequest = NoRequest
  /\ SubmitCurrent(FirstLaunchOperation)

SubmitSecondLaunch ==
  /\ SecondLaunchOperation \in LaunchOperations
  /\ state.wireRequest = NoRequest
  /\ SubmitCurrent(SecondLaunchOperation)

SubmitStart ==
  /\ StartOperation \in Operations
  /\ state.wireRequest = NoRequest
  /\ SubmitCurrent(StartOperation)

QueueBehindInFlight ==
  /\ Scenario \in {"disconnect-order", "capacity"}
  /\ state.wireRequest # NoRequest
  /\ \E operation \in Operations: SubmitCurrent(operation)

RejectRouteMismatch(request) ==
  /\ Scenario = "stale-routing"
  /\ CarrierActive
  /\ request \in Requests
  /\ RequestRef(request) # state.requestHandleRef
  /\ CanBeginAttempt(RequestOperation(request))
  /\ LET operation == RequestOperation(request) IN
       state' = [state EXCEPT
         !.attemptedRequests[operation] = request,
         !.completions[operation] = KnownUnchanged(request)]

RejectWrongWorkerRoute ==
  /\ Scenario = "stale-routing"
  /\ \E operation \in Operations:
       LET ref == WorkerRef(
         OtherWorker(state.requestHandleRef.worker),
         state.requestHandleRef.generation
       )
       IN RejectRouteMismatch(Request(ref, operation))

RejectWrongGenerationRoute ==
  /\ Scenario = "stale-routing"
  /\ \E operation \in Operations:
       LET ref == WorkerRef(
         state.requestHandleRef.worker,
         OtherGeneration(state.requestHandleRef.generation)
       )
       IN RejectRouteMismatch(Request(ref, operation))

RejectStaleHandleSubmission ==
  /\ Scenario = "stale-routing"
  /\ CarrierActive
  /\ \E ref \in state.retiredHandleRefs \ {state.requestHandleRef}:
       \E operation \in Operations:
         /\ CanBeginAttempt(operation)
         /\ LET request == Request(ref, operation) IN
              state' = [state EXCEPT
                !.attemptedRequests[operation] = request,
                !.completions[operation] = KnownUnchanged(request),
                !.unavailableOperations = @ \cup {operation}]

RejectStaleHandleWhileInactive ==
  /\ Scenario = "stale-routing"
  /\ ~CarrierActive
  /\ \E ref \in state.retiredHandleRefs:
       \E operation \in Operations:
         /\ CanBeginAttempt(operation)
         /\ LET request == Request(ref, operation) IN
              state' = [state EXCEPT
                !.attemptedRequests[operation] = request,
                !.completions[operation] = KnownUnchanged(request),
                !.unavailableOperations = @ \cup {operation},
                !.inactiveStaleHandleOperations = @ \cup {operation}]

RecordBackpressure(request, isFull) ==
  /\ request \in Requests
  /\ isFull \in BOOLEAN
  /\ CanBeginAttempt(RequestOperation(request))
  /\ LET operation == RequestOperation(request) IN
       state' = [state EXCEPT
         !.attemptedRequests[operation] = request,
         !.completions[operation] = KnownUnchanged(request),
         !.backpressuredOperations = @ \cup {operation},
         !.fullBackpressuredOperations =
           IF isFull THEN @ \cup {operation} ELSE @]

RejectBackpressure ==
  /\ Scenario = "capacity"
  /\ CarrierActive
  /\ state.wireRequest = NoRequest
  /\ state.submittedSlot # NoRequest
  /\ \E operation \in Operations:
       RecordBackpressure(
         Request(state.requestHandleRef, operation),
         FALSE
       )

RejectFullCarrierBackpressure ==
  /\ Scenario = "capacity"
  /\ CarrierActive
  /\ state.wireRequest # NoRequest
  /\ Cardinality(InFlightOperations) = 1
  /\ state.submittedSlot # NoRequest
  /\ Cardinality(AttemptedOperations) = 2
  /\ \E operation \in Operations:
       RecordBackpressure(
         Request(state.requestHandleRef, operation),
         TRUE
       )

DequeueRequest ==
  /\ CarrierActive
  /\ state.wirePhase = "idle"
  /\ state.wireRequest = NoRequest
  /\ state.submittedSlot # NoRequest
  /\ state.now < MaxTick
  /\ state' = [state EXCEPT
       !.wireRequest = state.submittedSlot,
       !.submittedSlot = NoRequest,
       !.wirePhase = "ready-to-write",
       !.wireExpiry = state.now + 1]

CompleteRequestWrite ==
  /\ CarrierActive
  /\ state.wirePhase = "ready-to-write"
  /\ state.now < state.wireExpiry
  /\ RequestRef(state.wireRequest) = state.wireOwnerRef
  /\ state.completeWriteCount[RequestOperation(state.wireRequest)] = 0
  /\ state' = [state EXCEPT
       !.wirePhase = "write-complete",
       !.wireExpiry = state.now + 1,
       !.completeWriteCount[RequestOperation(state.wireRequest)] = 1]

AdvanceMonotonicTime ==
  /\ state.wirePhase \in {"ready-to-write", "write-complete"}
  /\ state.incomingResponse = NoResponse
  /\ state.now < state.wireExpiry
  /\ state' = [state EXCEPT
       !.previousNow = state.now,
       !.now = @ + 1]

ArriveIncomingResponse(response, responseClass) ==
  /\ state.wirePhase = "write-complete"
  /\ state.incomingResponse = NoResponse
  /\ state.now < state.wireExpiry
  /\ response # NoResponse
  /\ ResponseTypeOK(response)
  /\ responseClass \in IncomingClasses \ {"none"}
  /\ LET operation == RequestOperation(state.wireRequest) IN
       state' = [state EXCEPT
         !.wirePhase = "response-ready",
         !.incomingResponse = response,
         !.incomingClass = responseClass,
         !.observedResponses[operation] = response,
         !.observedResponseClasses[operation] = responseClass,
         !.invalidResponseOperations =
           IF responseClass \in InvalidIncomingClasses
           THEN @ \cup {operation}
           ELSE @]

ArriveExactDefiniteResult ==
  /\ Scenario \in {"response-validation", "disconnect-order"}
  /\ \E payload \in DefiniteOutcomePayloads:
       ArriveIncomingResponse(
         DefiniteResponse(state.wireRequest, payload),
         "exact-definite"
       )

ArriveExactLaunchUnknownResult ==
  /\ Scenario \in {"response-validation", "disconnect-order"}
  /\ state.wireRequest.kind = "launch"
  /\ \E epoch \in HostEpochs:
       ArriveIncomingResponse(
         Response(
           "worker-to-server",
           ExpectedResultKind(state.wireRequest),
           state.wireRequest.authorization,
           state.wireRequest.target,
           LaunchUnknownOutcome(epoch)
         ),
         "exact-launch-unknown"
       )

ArriveExactStartUnknownResult ==
  /\ Scenario \in {"response-validation", "disconnect-order"}
  /\ state.wireRequest.kind = "start"
  /\ ArriveIncomingResponse(
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         state.wireRequest.authorization,
         state.wireRequest.target,
         StartUnknownOutcome
       ),
       "exact-start-unknown"
     )

ArriveInvalidResponse(responseClass, response) ==
  /\ Scenario = "response-validation"
  /\ responseClass \in InvalidIncomingClasses
  /\ ArriveIncomingResponse(response, responseClass)

ArriveWrongDirection ==
  /\ state.wireRequest # NoRequest
  /\ ArriveInvalidResponse(
       "wrong-direction",
       Response(
         "server-to-worker",
         ExpectedResultKind(state.wireRequest),
         state.wireRequest.authorization,
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongResultKind ==
  /\ state.wireRequest # NoRequest
  /\ ArriveInvalidResponse(
       "wrong-result-kind",
       Response(
         "worker-to-server",
         OtherResultKind(ExpectedResultKind(state.wireRequest)),
         state.wireRequest.authorization,
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongOperationCorrelation ==
  /\ state.wireRequest.kind = "launch"
  /\ ArriveInvalidResponse(
       "wrong-operation",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         WrongOperationAuthorization(state.wireRequest.authorization),
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongTerminalEcho ==
  /\ state.wireRequest # NoRequest
  /\ ArriveInvalidResponse(
       "wrong-terminal",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         WrongTerminalAuthorization(state.wireRequest.authorization),
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongWorkerEcho ==
  /\ state.wireRequest # NoRequest
  /\ ArriveInvalidResponse(
       "wrong-worker",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         WrongWorkerAuthorization(state.wireRequest.authorization),
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongGenerationEcho ==
  /\ state.wireRequest # NoRequest
  /\ ArriveInvalidResponse(
       "wrong-generation",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         WrongGenerationAuthorization(state.wireRequest.authorization),
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongFenceEcho ==
  /\ state.wireRequest # NoRequest
  /\ ArriveInvalidResponse(
       "wrong-fence",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         WrongFenceAuthorization(state.wireRequest.authorization),
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongLeaseIdEcho ==
  /\ state.wireRequest # NoRequest
  /\ ArriveInvalidResponse(
       "wrong-lease-id",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         WrongLeaseIdAuthorization(state.wireRequest.authorization),
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongLeaseExpiryEcho ==
  /\ state.wireRequest # NoRequest
  /\ ArriveInvalidResponse(
       "wrong-lease-expiry",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         WrongLeaseExpiryAuthorization(state.wireRequest.authorization),
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongCapabilityEcho ==
  /\ state.wireRequest # NoRequest
  /\ ArriveInvalidResponse(
       "wrong-capability",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         WrongCapabilityAuthorization(state.wireRequest.authorization),
         state.wireRequest.target,
         CanonicalDefiniteOutcome
       )
     )

ArriveWrongStartTarget ==
  /\ state.wireRequest.kind = "start"
  /\ ArriveInvalidResponse(
       "wrong-start-target",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         state.wireRequest.authorization,
         OtherStartTarget,
         CanonicalDefiniteOutcome
       )
     )

ArriveInvalidLaunchUnknownPayload ==
  /\ state.wireRequest.kind = "launch"
  /\ ArriveInvalidResponse(
       "invalid-unknown-payload",
       Response(
         "worker-to-server",
         ExpectedResultKind(state.wireRequest),
         state.wireRequest.authorization,
         state.wireRequest.target,
         LaunchUnknownOutcome(0)
       )
     )

ReceiveExactWorkerResult ==
  /\ state.wirePhase = "response-ready"
  /\ state.incomingClass \in ExactIncomingClasses
  /\ ResponseMatchesRequest(state.incomingResponse, state.wireRequest)
  /\ LET request == state.wireRequest
         response == state.incomingResponse
         operation == RequestOperation(request)
     IN state' = [state EXCEPT
          !.wirePhase = "result-received",
          !.wireExpiry = 0,
          !.incomingResponse = NoResponse,
          !.incomingClass = "none",
          !.receivedResponse = response,
          !.completions[operation] = ExactCompletion(request, response),
          !.nonRetryableOperations =
            IF response.outcome.classification = "outcome-unknown"
            THEN @ \cup {operation}
            ELSE @]

CompletionMapAfterRetirement ==
  LET afterWire ==
        CASE state.wirePhase = "ready-to-write" ->
               CompleteKnownUnchanged(
                 state.completions,
                 state.wireRequest
               )
          [] state.wirePhase \in {"write-complete", "response-ready"} ->
               CompleteEffectUnknown(
                 state.completions,
                 state.wireRequest
               )
          [] OTHER -> state.completions
  IN IF state.submittedSlot = NoRequest
     THEN afterWire
     ELSE CompleteKnownUnchanged(afterWire, state.submittedSlot)

RetireCarrier ==
  /\ CarrierActive
  /\ LET ref == state.wireOwnerRef
         writtenOperations ==
           IF state.wirePhase \in {"write-complete", "response-ready"}
           THEN WireOperations
           ELSE {}
         notDeliveredOperations ==
           IF state.wirePhase = "ready-to-write"
           THEN WireOperations
           ELSE {}
         unavailableOperations == SubmittedOperations
     IN state' = [state EXCEPT
          !.activeRef = NoWorkerRef,
          !.requestHandleRef = NoWorkerRef,
          !.wireOwnerRef = NoWorkerRef,
          !.retiredHandleRefs = @ \cup {ref},
          !.cleanupRefs = @ \cup {ref},
          !.submittedSlot = NoRequest,
          !.wireRequest = NoRequest,
          !.wirePhase = "idle",
          !.wireExpiry = 0,
          !.incomingResponse = NoResponse,
          !.incomingClass = "none",
          !.receivedResponse = NoResponse,
          !.completions = CompletionMapAfterRetirement,
          !.effectUnknownOperations = @ \cup writtenOperations,
          !.nonRetryableOperations = @ \cup writtenOperations,
          !.unavailableOperations = @ \cup unavailableOperations,
          !.notDeliveredOperations = @ \cup notDeliveredOperations]

RejectMismatchedWorkerResult ==
  /\ state.wirePhase = "response-ready"
  /\ state.incomingClass \in InvalidIncomingClasses
  /\ ~ResponseMatchesRequest(state.incomingResponse, state.wireRequest)
  /\ RetireCarrier

ExpireBeforeCompleteWrite ==
  /\ state.wirePhase = "ready-to-write"
  /\ state.now = state.wireExpiry
  /\ RetireCarrier

ExpireAfterCompleteWrite ==
  /\ state.wirePhase = "write-complete"
  /\ state.submittedSlot = NoRequest
  /\ state.now = state.wireExpiry
  /\ RetireCarrier

ExpireAfterCompleteWriteWithQueuedRequest ==
  /\ state.wirePhase = "write-complete"
  /\ state.submittedSlot # NoRequest
  /\ state.now = state.wireExpiry
  /\ RetireCarrier

DisconnectIdle ==
  /\ Scenario \in {"disconnect-order", "stale-routing"}
  /\ state.wirePhase = "idle"
  /\ RetireCarrier

DisconnectBeforeWrite ==
  /\ Scenario = "disconnect-order"
  /\ state.wirePhase = "ready-to-write"
  /\ RetireCarrier

DisconnectAfterWrite ==
  /\ Scenario = "disconnect-order"
  /\ state.wirePhase = "write-complete"
  /\ state.submittedSlot = NoRequest
  /\ RetireCarrier

DisconnectAfterWriteWithQueuedRequest ==
  /\ Scenario = "disconnect-order"
  /\ state.wirePhase = "write-complete"
  /\ state.submittedSlot # NoRequest
  /\ RetireCarrier

DisconnectAfterResult ==
  /\ Scenario = "disconnect-order"
  /\ state.wirePhase = "result-received"
  /\ RetireCarrier

PublishCompletion(operation, classification) ==
  /\ operation \in Operations
  /\ classification \in CompletionClasses
  /\ state.completions[operation].classification = classification
  /\ state.publications[operation] = NoCompletion
  /\ LET ownsReceivedResult ==
       /\ state.wirePhase = "result-received"
       /\ RequestOperation(state.wireRequest) = operation
     IN state' = [state EXCEPT
          !.publications[operation] = state.completions[operation],
          !.wireRequest =
            IF ownsReceivedResult THEN NoRequest ELSE @,
          !.wirePhase =
            IF ownsReceivedResult THEN "idle" ELSE @,
          !.receivedResponse =
            IF ownsReceivedResult THEN NoResponse ELSE @]

PublishKnownUnchanged ==
  \E operation \in Operations:
    /\ state.completions[operation].classification = "known-unchanged"
    /\ PublishCompletion(operation, "known-unchanged")

PublishExactResult ==
  \E operation \in Operations:
    /\ state.completions[operation].classification = "exact-result"
    /\ PublishCompletion(operation, "exact-result")

PublishEffectUnknown ==
  \E operation \in Operations:
    /\ state.completions[operation].classification = "effect-unknown"
    /\ PublishCompletion(operation, "effect-unknown")

DiscardLateResultAfterReplacement ==
  /\ Scenario = "disconnect-order"
  /\ \E operation \in
       state.effectUnknownOperations \ state.discardedLateResults:
       /\ CarrierActive
       /\ RequestRef(state.attemptedRequests[operation]) # state.activeRef
       /\ \E response \in
            ExactResponseValues(state.attemptedRequests[operation]):
            state' = [state EXCEPT
              !.discardedLateResults = @ \cup {operation},
              !.lateIncomingResponses[operation] = response]

CleanupWorkerGeneration(ref) ==
  /\ ref \in state.cleanupRefs
  /\ ref # state.activeRef
  /\ state' = [state EXCEPT !.cleanupRefs = @ \ {ref}]

CleanupAnyWorkerGeneration ==
  /\ ~CarrierActive
  /\ state.cleanupRefs # {}
  /\ \E ref \in WorkerRefs: CleanupWorkerGeneration(ref)

CleanupStaleAfterReplacement ==
  /\ Scenario \in {"disconnect-order", "stale-routing"}
  /\ CarrierActive
  /\ \E ref \in state.cleanupRefs:
       /\ ref # state.activeRef
       /\ CleanupWorkerGeneration(ref)

SettleCleanup ==
  CleanupAnyWorkerGeneration \/ CleanupStaleAfterReplacement

Next ==
  \/ ActivateInitialCarrier
  \/ ActivateReplacementCarrier
  \/ RejectStaleCarrierActivation
  \/ SubmitFirstLaunch
  \/ SubmitSecondLaunch
  \/ SubmitStart
  \/ QueueBehindInFlight
  \/ RejectWrongWorkerRoute
  \/ RejectWrongGenerationRoute
  \/ RejectStaleHandleSubmission
  \/ RejectStaleHandleWhileInactive
  \/ RejectBackpressure
  \/ RejectFullCarrierBackpressure
  \/ DequeueRequest
  \/ CompleteRequestWrite
  \/ AdvanceMonotonicTime
  \/ ArriveExactDefiniteResult
  \/ ArriveExactLaunchUnknownResult
  \/ ArriveExactStartUnknownResult
  \/ ArriveWrongDirection
  \/ ArriveWrongResultKind
  \/ ArriveWrongOperationCorrelation
  \/ ArriveWrongTerminalEcho
  \/ ArriveWrongWorkerEcho
  \/ ArriveWrongGenerationEcho
  \/ ArriveWrongFenceEcho
  \/ ArriveWrongLeaseIdEcho
  \/ ArriveWrongLeaseExpiryEcho
  \/ ArriveWrongCapabilityEcho
  \/ ArriveWrongStartTarget
  \/ ArriveInvalidLaunchUnknownPayload
  \/ ReceiveExactWorkerResult
  \/ RejectMismatchedWorkerResult
  \/ ExpireBeforeCompleteWrite
  \/ ExpireAfterCompleteWrite
  \/ ExpireAfterCompleteWriteWithQueuedRequest
  \/ DisconnectIdle
  \/ DisconnectBeforeWrite
  \/ DisconnectAfterWrite
  \/ DisconnectAfterWriteWithQueuedRequest
  \/ DisconnectAfterResult
  \/ PublishKnownUnchanged
  \/ PublishExactResult
  \/ PublishEffectUnknown
  \/ DiscardLateResultAfterReplacement
  \/ CleanupAnyWorkerGeneration
  \/ CleanupStaleAfterReplacement

SettleExpiredWire ==
  \/ ExpireBeforeCompleteWrite
  \/ ExpireAfterCompleteWrite
  \/ ExpireAfterCompleteWriteWithQueuedRequest

SettleIncomingResponse ==
  ReceiveExactWorkerResult \/ RejectMismatchedWorkerResult

PublishAnyCompletion ==
  PublishKnownUnchanged \/ PublishExactResult \/ PublishEffectUnknown

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(DequeueRequest)
  /\ WF_vars(AdvanceMonotonicTime)
  /\ WF_vars(SettleExpiredWire)
  /\ WF_vars(SettleIncomingResponse)
  /\ WF_vars(PublishAnyCompletion)
  /\ WF_vars(SettleCleanup)

TypeOK ==
  /\ state.activeRef \in WorkerRefValues
  /\ state.requestHandleRef \in WorkerRefValues
  /\ state.wireOwnerRef \in WorkerRefValues
  /\ state.nextGeneration \in 1..(MaxGeneration + 1)
  /\ state.retiredHandleRefs \subseteq WorkerRefs
  /\ state.rejectedActivationRefs \subseteq WorkerRefs
  /\ state.cleanupRefs \subseteq WorkerRefs
  /\ state.submittedSlot \in RequestValues
  /\ state.wireRequest \in RequestValues
  /\ state.wirePhase \in WirePhases
  /\ state.wireExpiry \in 0..MaxTick
  /\ state.now \in 0..MaxTick
  /\ state.previousNow \in 0..MaxTick
  /\ ResponseTypeOK(state.incomingResponse)
  /\ state.incomingClass \in IncomingClasses
  /\ ResponseTypeOK(state.receivedResponse)
  /\ state.attemptedRequests \in [Operations -> RequestValues]
  /\ \A operation \in Operations:
       ResponseTypeOK(state.observedResponses[operation])
  /\ state.observedResponseClasses \in [Operations -> IncomingClasses]
  /\ \A operation \in Operations:
       ResponseTypeOK(state.lateIncomingResponses[operation])
  /\ \A operation \in Operations:
       /\ CompletionTypeOK(state.completions[operation])
       /\ CompletionTypeOK(state.publications[operation])
  /\ state.completeWriteCount \in [Operations -> 0..1]
  /\ state.effectUnknownOperations \subseteq Operations
  /\ state.nonRetryableOperations \subseteq Operations
  /\ state.discardedLateResults \subseteq Operations
  /\ state.invalidResponseOperations \subseteq Operations
  /\ state.backpressuredOperations \subseteq Operations
  /\ state.fullBackpressuredOperations \subseteq Operations
  /\ state.inactiveStaleHandleOperations \subseteq Operations
  /\ state.unavailableOperations \subseteq Operations
  /\ state.notDeliveredOperations \subseteq Operations
  /\ state.replacementBarrierHeld \in BOOLEAN

OneActiveGenerationAndOneWireOwner ==
  /\ (state.activeRef = NoWorkerRef) <=>
       ( /\ state.requestHandleRef = NoWorkerRef
         /\ state.wireOwnerRef = NoWorkerRef )
  /\ (state.activeRef # NoWorkerRef =>
       /\ state.requestHandleRef = state.activeRef
       /\ state.wireOwnerRef = state.activeRef
       /\ state.activeRef.generation < state.nextGeneration)
  /\ state.activeRef \notin state.retiredHandleRefs
  /\ state.activeRef \notin state.rejectedActivationRefs
  /\ state.activeRef \notin state.cleanupRefs
  /\ state.rejectedActivationRefs \subseteq state.retiredHandleRefs

QueueAndWireFlightAreBounded ==
  /\ Cardinality(
       IF state.submittedSlot = NoRequest
       THEN {}
       ELSE {state.submittedSlot}
     ) <= 1
  /\ Cardinality(
       IF state.wireRequest = NoRequest
       THEN {}
       ELSE {state.wireRequest}
     ) <= 1
  /\ Cardinality(InFlightOperations) <= 1
  /\ SubmittedOperations \intersect WireOperations = {}
  /\ Cardinality(AttemptedOperations) <= ScenarioAttemptLimit

WireAndDeadlineStateAreExact ==
  /\ (state.wirePhase = "idle") <=>
       (state.wireRequest = NoRequest)
  /\ (state.incomingResponse # NoResponse) <=>
       (state.wirePhase = "response-ready")
  /\ (state.incomingClass # "none") <=>
       (state.incomingResponse # NoResponse)
  /\ (state.receivedResponse # NoResponse) <=>
       (state.wirePhase = "result-received")
  /\ (state.wirePhase \in {
       "ready-to-write", "write-complete", "response-ready"
     } =>
       /\ state.wireExpiry \in 1..MaxTick
       /\ state.now <= state.wireExpiry)
  /\ (state.wirePhase \in {"idle", "result-received"} =>
       state.wireExpiry = 0)

LiveRequestsMatchAttemptHistory ==
  /\ (state.submittedSlot # NoRequest =>
       state.attemptedRequests[RequestOperation(state.submittedSlot)] =
         state.submittedSlot)
  /\ (state.wireRequest # NoRequest =>
       state.attemptedRequests[RequestOperation(state.wireRequest)] =
         state.wireRequest)

CompletionsRemainCorrelated ==
  \A operation \in Operations:
    /\ (state.attemptedRequests[operation] = NoRequest =>
         /\ state.completions[operation] = NoCompletion
         /\ state.publications[operation] = NoCompletion
         /\ state.observedResponses[operation] = NoResponse
         /\ state.observedResponseClasses[operation] = "none"
         /\ state.lateIncomingResponses[operation] = NoResponse
         /\ state.completeWriteCount[operation] = 0)
    /\ (state.completions[operation] # NoCompletion =>
         /\ state.attemptedRequests[operation] # NoRequest
         /\ state.completions[operation].request =
              state.attemptedRequests[operation]
         /\ RequestOperation(state.completions[operation].request) =
              operation)
    /\ (state.publications[operation] # NoCompletion =>
         state.publications[operation] = state.completions[operation])
    /\ (state.observedResponses[operation] # NoResponse =>
         /\ state.attemptedRequests[operation] # NoRequest
         /\ state.observedResponseClasses[operation] # "none")

KnownUnchangedMeansNoCompleteWrite ==
  \A operation \in Operations:
    state.completions[operation].classification = "known-unchanged" =>
      /\ state.completeWriteCount[operation] = 0
      /\ state.completions[operation].response = NoResponse

ExactResultsPreserveValidatedIncomingResponse ==
  \A operation \in Operations:
    state.completions[operation].classification = "exact-result" =>
      /\ state.completeWriteCount[operation] = 1
      /\ state.completions[operation].response =
           state.observedResponses[operation]
      /\ state.observedResponseClasses[operation] \in
           ExactIncomingClasses
      /\ ResponseMatchesRequest(
           state.completions[operation].response,
           state.completions[operation].request
         )
      /\ state.completions[operation].response.authorization =
           state.attemptedRequests[operation].authorization
      /\ state.completions[operation].response.target =
           state.attemptedRequests[operation].target

LaunchUnknownPayloadIsExact ==
  \A operation \in Operations:
    /\ state.completions[operation].classification = "exact-result"
    /\ state.completions[operation].request.kind = "launch"
    /\ state.completions[operation].response.outcome.classification =
         "outcome-unknown"
    =>
      /\ state.completions[operation].response.outcome.payload =
           NoOutcomePayload
      /\ state.completions[operation].response.outcome.launchUnknownHostEpoch
           \in HostEpochs

EffectUnknownFollowsOneCompleteWrite ==
  /\ state.effectUnknownOperations = {
       operation \in Operations :
         state.completions[operation].classification = "effect-unknown"
     }
  /\ \A operation \in state.effectUnknownOperations:
       /\ state.completeWriteCount[operation] = 1
       /\ state.completions[operation].response = NoResponse

UnknownEffectsAreNeverRetried ==
  /\ state.nonRetryableOperations =
       state.effectUnknownOperations \cup {
         operation \in Operations :
           /\ state.completions[operation].classification = "exact-result"
           /\ state.completions[operation].response.outcome.classification =
                "outcome-unknown"
       }
  /\ \A operation \in state.nonRetryableOperations:
       /\ state.completeWriteCount[operation] = 1
       /\ operation \notin SubmittedOperations
       /\ operation \notin InFlightOperations

InvalidResponsesNeverPublishAsExact ==
  /\ state.invalidResponseOperations = {
       operation \in Operations :
         state.observedResponseClasses[operation] \in InvalidIncomingClasses
     }
  /\ \A operation \in state.invalidResponseOperations:
       /\ state.completions[operation].classification # "exact-result"
       /\ state.publications[operation].classification # "exact-result"
  /\ \A operation \in Operations:
       state.observedResponseClasses[operation] =
         "invalid-unknown-payload" =>
         state.publications[operation].classification # "exact-result"

BackpressureIsARecordedKnownUnchangedAttempt ==
  /\ state.fullBackpressuredOperations \subseteq
       state.backpressuredOperations
  /\ \A operation \in state.backpressuredOperations:
       /\ state.attemptedRequests[operation] \in Requests
       /\ state.completions[operation] =
            KnownUnchanged(state.attemptedRequests[operation])
       /\ state.completeWriteCount[operation] = 0

MismatchedAndLateResultsNeverPublishAsExact ==
  /\ state.discardedLateResults \subseteq
       state.effectUnknownOperations
  /\ \A operation \in state.discardedLateResults:
       /\ state.completions[operation].classification = "effect-unknown"
       /\ state.publications[operation] = state.completions[operation]
       /\ state.lateIncomingResponses[operation] # NoResponse
       /\ ResponseMatchesRequest(
            state.lateIncomingResponses[operation],
            state.attemptedRequests[operation]
          )
       /\ RequestRef(state.attemptedRequests[operation]) # state.activeRef

StaleCleanupCannotOwnCurrentGeneration ==
  state.cleanupRefs \cap {state.activeRef} = {}

MonotonicTime ==
  state.previousNow <= state.now

ReplacementActivationRequiresSettledHistory ==
  state.replacementBarrierHeld

FullBackpressureIsFreshThirdAttempt ==
  state.fullBackpressuredOperations # {} =>
    /\ Cardinality(AttemptedOperations) = 3
    /\ \A operation \in state.fullBackpressuredOperations:
         state.attemptedRequests[operation] \in Requests

InactiveStaleHandleIsKnownUnchanged ==
  \A operation \in state.inactiveStaleHandleOperations:
    /\ state.completions[operation] =
         KnownUnchanged(state.attemptedRequests[operation])
    /\ operation \in state.unavailableOperations
    /\ RequestRef(state.attemptedRequests[operation]) \in
         state.retiredHandleRefs

PendingAttemptEventuallyCompletes ==
  \A operation \in Operations:
    [](operation \in PendingOperations =>
       <>(operation \notin PendingOperations))

PendingCompletionEventuallyPublishes ==
  \A operation \in Operations:
    [](operation \in PendingPublications =>
       <>(operation \notin PendingPublications))

PendingCleanupEventuallySettles ==
  [](state.cleanupRefs # {} => <>(state.cleanupRefs = {}))

=============================================================================
