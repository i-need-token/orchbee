---------------- MODULE TerminalLaunchAuthorization ----------------
EXTENDS FiniteSets, Naturals, TLC

\* Models the Server-owned authorization envelope and Worker admission for
\* one retained terminal. The abstract binding corresponds to the complete
\* Server-Worker operation binding: Worker generation, operation identity,
\* per-terminal fence, stateful lease, capability version, operation kind,
\* and (for Start) the target Host epoch. Host effects happen only after an
\* admitted handoff. Transport loss alone never terminates a retained
\* terminal; Host process replacement is a separate epoch-changing action.
\* FreshOperation is this single-terminal model's projection of the producer
\* invariant that an authenticated active Server authority allocates operation
\* identities globally fresh across terminals, Workers, and Worker generations.
\* Worker reconnection does not reset that scope. A successor Server authority
\* may start a new scope only after its generation, lease, and fence transition
\* makes every prior authorization non-current; that transition and the
\* cross-terminal and cross-Worker namespace are outside this bounded model.

CONSTANTS
  FirstOperation,
  SecondOperation,
  FirstLease,
  SecondLease,
  MaxGeneration,
  MaxFence,
  MaxHostEpoch

ASSUME FirstOperation # SecondOperation
ASSUME FirstLease # SecondLease

Operations == {FirstOperation, SecondOperation}
Leases == {FirstLease, SecondLease}

OperationKinds == {"launch-immediate", "launch-deferred", "start"}
RequestPhases == {"none", "issued", "admitted", "handed-off"}
ResultOutcomes == {"host-accepted", "rejected", "failed", "outcome-unknown"}
ResultTransitions == {"none", "awaiting-start", "started", "already-started"}
TerminalStates == {"none", "awaiting-start", "started", "lost"}
Capabilities == {1, 2}
LegacyCapability == 1
SupportedCapability == 2
Generations == 1..MaxGeneration
Fences == 1..MaxFence
HostEpochs == 1..MaxHostEpoch

Bindings == [
  generation : Generations,
  fence : Fences,
  operation : Operations,
  lease : Leases,
  kind : OperationKinds,
  capability : Capabilities,
  targetEpoch : 0..MaxHostEpoch
]
BindingValues == [
  generation : 0..MaxGeneration,
  fence : 0..MaxFence,
  operation : Operations \cup {"no-operation"},
  lease : Leases \cup {"no-lease"},
  kind : OperationKinds \cup {"none"},
  capability : 0..2,
  targetEpoch : 0..MaxHostEpoch
]

NoBinding == [
  generation |-> 0,
  fence |-> 0,
  operation |-> "no-operation",
  lease |-> "no-lease",
  kind |-> "none",
  capability |-> 0,
  targetEpoch |-> 0
]

Results == [
  binding : Bindings,
  outcome : ResultOutcomes,
  hostEpoch : 0..MaxHostEpoch,
  transition : ResultTransitions
]
ResultValues == [
  binding : BindingValues,
  outcome : ResultOutcomes \cup {"none"},
  hostEpoch : 0..MaxHostEpoch,
  transition : ResultTransitions
]

NoResult == [
  binding |-> NoBinding,
  outcome |-> "none",
  hostEpoch |-> 0,
  transition |-> "none"
]

HostEffects == [
  generation : Generations,
  fence : Fences,
  operation : Operations,
  kind : OperationKinds,
  hostEpoch : HostEpochs
]

VARIABLES
  serverConnected,
  workerConnected,
  hostConnected,
  workerGeneration,
  hostEpoch,
  leaseValid,
  acceptedBinding,
  authority,
  request,
  requestPhase,
  pendingResult,
  cachedOutcome,
  cachedHostEpoch,
  cachedTransition,
  terminalState,
  terminalHostEpoch,
  hostEffects

DomainVars == <<
  serverConnected,
  workerConnected,
  hostConnected,
  workerGeneration,
  hostEpoch,
  leaseValid,
  acceptedBinding,
  authority,
  request,
  requestPhase,
  pendingResult,
  cachedOutcome,
  cachedHostEpoch,
  cachedTransition,
  terminalState,
  terminalHostEpoch,
  hostEffects
>>

vars == DomainVars

Binding(generation, fence, operation, lease, kind, capability, targetEpoch) ==
  [
    generation |-> generation,
    fence |-> fence,
    operation |-> operation,
    lease |-> lease,
    kind |-> kind,
    capability |-> capability,
    targetEpoch |-> targetEpoch
  ]

Effect(binding, epoch) ==
  [
    generation |-> binding.generation,
    fence |-> binding.fence,
    operation |-> binding.operation,
    kind |-> binding.kind,
    hostEpoch |-> epoch
  ]

Result(binding, outcome, epoch, transition) ==
  [
    binding |-> binding,
    outcome |-> outcome,
    hostEpoch |-> epoch,
    transition |-> transition
  ]

ResultMatchesAuthority(result) ==
  /\ result # NoResult
  /\ authority # NoBinding
  /\ result.binding = authority
  /\ result.binding.generation = workerGeneration
  /\ (result.hostEpoch = 0 \/ result.hostEpoch = hostEpoch)

HasCachedResult == cachedOutcome # "none"

CachedResult ==
  IF HasCachedResult
  THEN Result(
         acceptedBinding,
         cachedOutcome,
         cachedHostEpoch,
         cachedTransition
       )
  ELSE NoResult

HasEffect(binding, epoch) ==
  Effect(binding, epoch) \in hostEffects

HasOperationEffect(operation) ==
  \E effect \in hostEffects: effect.operation = operation

Init ==
  /\ serverConnected = TRUE
  /\ workerConnected = TRUE
  /\ hostConnected = TRUE
  /\ workerGeneration = 1
  /\ hostEpoch = 1
  /\ leaseValid = [lease \in Leases |-> FALSE]
  /\ acceptedBinding = NoBinding
  /\ authority = NoBinding
  /\ request = NoBinding
  /\ requestPhase = "none"
  /\ pendingResult = NoResult
  /\ cachedOutcome = "none"
  /\ cachedHostEpoch = 0
  /\ cachedTransition = "none"
  /\ terminalState = "none"
  /\ terminalHostEpoch = 0
  /\ hostEffects = {}

CanDispatch ==
  /\ serverConnected
  /\ workerConnected
  /\ requestPhase = "none"
  /\ pendingResult = NoResult

CanIssue ==
  /\ CanDispatch
  /\ authority = NoBinding \/ authority.fence < MaxFence

FreshOperation(operation) ==
  \* The adversarial Issue* branches below do not represent authoritative
  \* Server issuance. Current issuance uses this guard, whose bounded
  \* fence-to-operation mapping never selects an earlier operation again.
  /\ operation \in Operations
  /\ ~HasOperationEffect(operation)
  /\ (authority = NoBinding \/ operation # authority.operation)

OperationForFence(fence) ==
  IF fence = 1 THEN FirstOperation ELSE SecondOperation

LeaseForFence(fence) ==
  IF fence = 1 THEN FirstLease ELSE SecondLease

OtherOperation(operation) ==
  IF operation = FirstOperation THEN SecondOperation ELSE FirstOperation

IssueCurrentLaunchFor(kind) ==
  /\ CanIssue
  /\ kind \in {"launch-immediate", "launch-deferred"}
  /\ terminalState \in {"none", "lost"}
  /\ LET fence == IF authority = NoBinding
                    THEN 1
                    ELSE authority.fence + 1
         operation == OperationForFence(fence)
         lease == LeaseForFence(fence)
         next == Binding(
           workerGeneration,
           fence,
           operation,
           lease,
           kind,
           SupportedCapability,
           0
         )
     IN
       /\ FreshOperation(operation)
       /\ request' = next
       /\ authority' = next
       /\ requestPhase' = "issued"
       /\ leaseValid' = [leaseValid EXCEPT ![lease] = TRUE]
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, acceptedBinding, pendingResult,
       cachedOutcome, cachedHostEpoch, cachedTransition, terminalState,
       terminalHostEpoch, hostEffects>>

IssueCurrentImmediateLaunch ==
  /\ terminalState \in {"none", "lost"}
  /\ IssueCurrentLaunchFor("launch-immediate")

IssueCurrentDeferredLaunch ==
  /\ terminalState \in {"none", "lost"}
  /\ IssueCurrentLaunchFor("launch-deferred")

IssueCurrentLaunch ==
  /\ CanIssue
  /\ ( \/ IssueCurrentImmediateLaunch
       \/ IssueCurrentDeferredLaunch )

IssueStaleGenerationLaunch ==
  /\ CanDispatch
  /\ workerGeneration > 1
  /\ request' = Binding(
       workerGeneration - 1,
       1,
       FirstOperation,
       FirstLease,
       "launch-immediate",
       SupportedCapability,
       0
     )
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       terminalState, terminalHostEpoch,
       hostEffects>>

IssueLegacyCapabilityLaunch ==
  /\ CanDispatch
  /\ request' = Binding(
       workerGeneration,
       1,
       FirstOperation,
       FirstLease,
       "launch-immediate",
       LegacyCapability,
       0
     )
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       terminalState, terminalHostEpoch,
       hostEffects>>

IssueLowFenceReplay ==
  /\ CanDispatch
  /\ acceptedBinding # NoBinding
  /\ acceptedBinding.fence > 1
  /\ request' = Binding(
       workerGeneration,
       acceptedBinding.fence - 1,
       acceptedBinding.operation,
       acceptedBinding.lease,
       acceptedBinding.kind,
       SupportedCapability,
       acceptedBinding.targetEpoch
     )
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       terminalState, terminalHostEpoch,
       hostEffects>>

IssueExactReplay ==
  /\ CanDispatch
  /\ acceptedBinding # NoBinding
  /\ HasCachedResult
  /\ request' = acceptedBinding
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       terminalState, terminalHostEpoch,
       hostEffects>>

IssueConflictingReplay ==
  /\ CanDispatch
  /\ acceptedBinding # NoBinding
  /\ request' = [acceptedBinding EXCEPT
       !.operation = OtherOperation(acceptedBinding.operation)]
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       terminalState, terminalHostEpoch,
       hostEffects>>

IssueAuthorizedStart ==
  /\ CanIssue
  /\ terminalState = "awaiting-start"
  /\ terminalHostEpoch = hostEpoch
  /\ LET fence == authority.fence + 1
         operation == OperationForFence(fence)
         lease == LeaseForFence(fence)
         next == Binding(
           workerGeneration,
           fence,
           operation,
           lease,
           "start",
           SupportedCapability,
           terminalHostEpoch
         )
     IN
       /\ FreshOperation(operation)
       /\ request' = next
       /\ authority' = next
       /\ requestPhase' = "issued"
       /\ leaseValid' = [leaseValid EXCEPT ![lease] = TRUE]
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, acceptedBinding, pendingResult,
       cachedOutcome, cachedHostEpoch, cachedTransition, terminalState,
       terminalHostEpoch, hostEffects>>

QueueResult(outcome, epoch, transition) ==
  /\ pendingResult = NoResult
  /\ pendingResult' = Result(request, outcome, epoch, transition)
  /\ request' = NoBinding
  /\ requestPhase' = "none"

QueueUncachedResult(outcome, epoch, transition) ==
  /\ QueueResult(outcome, epoch, transition)
  /\ UNCHANGED <<cachedOutcome, cachedHostEpoch, cachedTransition>>

QueueAcceptedResult(outcome, epoch, transition) ==
  /\ QueueResult(outcome, epoch, transition)
  /\ cachedOutcome' = outcome
  /\ cachedHostEpoch' = epoch
  /\ cachedTransition' = transition

QueueCachedResult ==
  /\ pendingResult = NoResult
  /\ pendingResult' = CachedResult
  /\ request' = NoBinding
  /\ requestPhase' = "none"
  /\ UNCHANGED <<cachedOutcome, cachedHostEpoch, cachedTransition>>

RejectStaleGeneration ==
  /\ requestPhase = "issued"
  /\ request.generation # workerGeneration
  /\ QueueUncachedResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

RejectUnsupportedCapability ==
  /\ requestPhase = "issued"
  /\ request.generation = workerGeneration
  /\ request.capability # SupportedCapability
  /\ QueueUncachedResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

ExpireLeaseBeforeAdmission ==
  /\ requestPhase = "issued"
  /\ leaseValid[request.lease]
  /\ leaseValid' = [leaseValid EXCEPT ![request.lease] = FALSE]
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, acceptedBinding, authority, request,
       requestPhase, pendingResult, cachedOutcome, cachedHostEpoch,
       cachedTransition, terminalState,
       terminalHostEpoch, hostEffects>>

RejectExpiredBeforeAdmission ==
  /\ requestPhase = "issued"
  /\ ~leaseValid[request.lease]
  /\ (~HasCachedResult \/ request # acceptedBinding)
  /\ QueueUncachedResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

RejectLowFence ==
  /\ requestPhase = "issued"
  /\ request.generation = workerGeneration
  /\ request.capability = SupportedCapability
  /\ leaseValid[request.lease]
  /\ acceptedBinding # NoBinding
  /\ request.fence < acceptedBinding.fence
  /\ QueueUncachedResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

RejectConflictingReplay ==
  /\ requestPhase = "issued"
  /\ request.generation = workerGeneration
  /\ request.capability = SupportedCapability
  /\ leaseValid[request.lease]
  /\ acceptedBinding # NoBinding
  /\ request.fence = acceptedBinding.fence
  /\ request # acceptedBinding
  /\ QueueUncachedResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

RejectHostIdentityChanged ==
  /\ requestPhase = "issued"
  /\ request.kind = "start"
  /\ request.generation = workerGeneration
  /\ request.capability = SupportedCapability
  /\ leaseValid[request.lease]
  /\ request.targetEpoch # hostEpoch
  /\ (~HasCachedResult \/ request # acceptedBinding)
  /\ QueueUncachedResult("rejected", hostEpoch, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

CachedReplayReady ==
  /\ requestPhase = "issued"
  /\ request = acceptedBinding
  /\ request.generation = workerGeneration
  /\ HasCachedResult

ReplayCachedAcceptedResult ==
  /\ CachedReplayReady
  /\ leaseValid[request.lease]
  /\ cachedOutcome = "host-accepted"
  /\ QueueCachedResult
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

ReplayCachedRejectedResult ==
  /\ CachedReplayReady
  /\ leaseValid[request.lease]
  /\ cachedOutcome = "rejected"
  /\ QueueCachedResult
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

ReplayCachedFailedResult ==
  /\ CachedReplayReady
  /\ leaseValid[request.lease]
  /\ cachedOutcome = "failed"
  /\ QueueCachedResult
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

ReplayCachedUnknownResult ==
  /\ CachedReplayReady
  /\ leaseValid[request.lease]
  /\ cachedOutcome = "outcome-unknown"
  /\ QueueCachedResult
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

ReplayCachedExpiredLeaseResult ==
  /\ CachedReplayReady
  /\ ~leaseValid[request.lease]
  /\ QueueCachedResult
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

AdmitAuthorization ==
  /\ requestPhase = "issued"
  /\ workerConnected
  /\ pendingResult = NoResult
  /\ request.generation = workerGeneration
  /\ request.capability = SupportedCapability
  /\ leaseValid[request.lease]
  /\ (request.kind # "start" \/ request.targetEpoch = hostEpoch)
  /\ acceptedBinding = NoBinding \/ request.fence > acceptedBinding.fence
  /\ acceptedBinding' = request
  /\ requestPhase' = "admitted"
  /\ cachedOutcome' = "none"
  /\ cachedHostEpoch' = 0
  /\ cachedTransition' = "none"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, authority, request,
       pendingResult, terminalState, terminalHostEpoch,
       hostEffects>>

ExpireLeaseAfterAdmission ==
  /\ requestPhase = "admitted"
  /\ leaseValid[request.lease]
  /\ leaseValid' = [leaseValid EXCEPT ![request.lease] = FALSE]
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, acceptedBinding, authority, request,
       requestPhase, pendingResult, cachedOutcome, cachedHostEpoch,
       cachedTransition, terminalState,
       terminalHostEpoch, hostEffects>>

RejectExpiredAfterAdmission ==
  /\ requestPhase = "admitted"
  /\ ~leaseValid[request.lease]
  /\ QueueAcceptedResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

RejectHostIdentityChangedAfterAdmission ==
  /\ requestPhase = "admitted"
  /\ request.kind = "start"
  /\ hostConnected
  /\ leaseValid[request.lease]
  /\ request.targetEpoch # hostEpoch
  /\ QueueAcceptedResult("rejected", hostEpoch, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

FailBeforeHostHandoff ==
  /\ requestPhase = "admitted"
  /\ pendingResult = NoResult
  /\ QueueAcceptedResult("failed", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

HandoffToHost ==
  /\ requestPhase = "admitted"
  /\ pendingResult = NoResult
  /\ workerConnected
  /\ hostConnected
  /\ leaseValid[request.lease]
  /\ (request.kind # "start" \/ request.targetEpoch = hostEpoch)
  /\ ~HasOperationEffect(request.operation)
  /\ hostEffects' = hostEffects \cup {Effect(request, hostEpoch)}
  /\ requestPhase' = "handed-off"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       request, pendingResult, cachedOutcome, cachedHostEpoch,
       cachedTransition, terminalState,
       terminalHostEpoch>>

ExpireLeaseAfterHandoff ==
  /\ requestPhase = "handed-off"
  /\ leaseValid[request.lease]
  /\ leaseValid' = [leaseValid EXCEPT ![request.lease] = FALSE]
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, acceptedBinding, authority, request,
       requestPhase, pendingResult, cachedOutcome, cachedHostEpoch,
       cachedTransition, terminalState,
       terminalHostEpoch, hostEffects>>

ExpireCachedLease ==
  /\ requestPhase = "none"
  /\ pendingResult = NoResult
  /\ HasCachedResult
  /\ leaseValid[acceptedBinding.lease]
  /\ leaseValid' = [leaseValid EXCEPT ![acceptedBinding.lease] = FALSE]
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, acceptedBinding, authority, request,
       requestPhase, pendingResult, cachedOutcome, cachedHostEpoch,
       cachedTransition, terminalState, terminalHostEpoch, hostEffects>>

AcceptAtHost(kind, transition) ==
  /\ requestPhase = "handed-off"
  /\ hostConnected
  /\ pendingResult = NoResult
  /\ HasEffect(request, hostEpoch)
  /\ request.kind = kind
  /\ terminalState' = IF transition = "awaiting-start"
                       THEN "awaiting-start"
                       ELSE "started"
  /\ terminalHostEpoch' = hostEpoch
  /\ QueueAcceptedResult("host-accepted", hostEpoch, transition)
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       hostEffects>>

HostAcceptImmediateLaunch ==
  /\ request.kind = "launch-immediate"
  /\ AcceptAtHost("launch-immediate", "started")

HostAcceptDeferredLaunch ==
  /\ request.kind = "launch-deferred"
  /\ AcceptAtHost("launch-deferred", "awaiting-start")

HostAcceptStarted ==
  /\ request.kind = "start"
  /\ AcceptAtHost("start", "started")

HostAcceptAlreadyStarted ==
  /\ request.kind = "start"
  /\ AcceptAtHost("start", "already-started")

HostReject ==
  /\ requestPhase = "handed-off"
  /\ hostConnected
  /\ HasEffect(request, hostEpoch)
  /\ QueueAcceptedResult("rejected", hostEpoch, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

HostFail ==
  /\ requestPhase = "handed-off"
  /\ hostConnected
  /\ HasEffect(request, hostEpoch)
  /\ QueueAcceptedResult("failed", hostEpoch, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

DeliverCurrentResult ==
  /\ serverConnected
  /\ ResultMatchesAuthority(pendingResult)
  /\ pendingResult' = NoResult
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       request, requestPhase, cachedOutcome, cachedHostEpoch,
       cachedTransition, terminalState, terminalHostEpoch, hostEffects>>

DropStaleResult ==
  /\ serverConnected
  /\ pendingResult # NoResult
  /\ ~ResultMatchesAuthority(pendingResult)
  /\ pendingResult' = NoResult
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       request, requestPhase, cachedOutcome, cachedHostEpoch,
       cachedTransition, terminalState,
       terminalHostEpoch, hostEffects>>

LoseServerConnection ==
  /\ serverConnected
  /\ serverConnected' = FALSE
  /\ UNCHANGED <<workerConnected, hostConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, request, requestPhase,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       terminalState, terminalHostEpoch,
       hostEffects>>

RestoreServerConnection ==
  /\ ~serverConnected
  /\ serverConnected' = TRUE
  /\ UNCHANGED <<workerConnected, hostConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, request, requestPhase,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       terminalState, terminalHostEpoch,
       hostEffects>>

LoseIdleWorkerConnection ==
  /\ workerConnected
  /\ requestPhase = "none"
  /\ workerConnected' = FALSE
  /\ UNCHANGED <<serverConnected, hostConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, request, requestPhase,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       terminalState,
       terminalHostEpoch, hostEffects>>

LoseWorkerConnectionBeforeAdmission ==
  /\ workerConnected
  /\ requestPhase = "issued"
  /\ workerConnected' = FALSE
  /\ QueueUncachedResult(
       "outcome-unknown",
       IF request.kind = "start" THEN request.targetEpoch ELSE hostEpoch,
       "none"
     )
  /\ UNCHANGED <<serverConnected, hostConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, terminalState,
       terminalHostEpoch, hostEffects>>

LoseWorkerConnectionAfterAdmission ==
  /\ workerConnected
  /\ requestPhase = "admitted"
  /\ workerConnected' = FALSE
  /\ QueueAcceptedResult(
       "outcome-unknown",
       IF request.kind = "start" THEN request.targetEpoch ELSE hostEpoch,
       "none"
     )
  /\ UNCHANGED <<serverConnected, hostConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, terminalState,
       terminalHostEpoch, hostEffects>>

LoseWorkerConnectionAfterHandoff ==
  /\ workerConnected
  /\ requestPhase = "handed-off"
  /\ workerConnected' = FALSE
  /\ QueueAcceptedResult(
       "outcome-unknown",
       IF request.kind = "start" THEN request.targetEpoch ELSE hostEpoch,
       "none"
     )
  /\ UNCHANGED <<serverConnected, hostConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, terminalState,
       terminalHostEpoch, hostEffects>>

ReconnectWorker ==
  /\ ~workerConnected
  /\ workerGeneration < MaxGeneration
  /\ pendingResult = NoResult
  /\ workerConnected' = TRUE
  /\ workerGeneration' = workerGeneration + 1
  /\ UNCHANGED <<serverConnected, hostConnected, hostEpoch, leaseValid,
       acceptedBinding, authority, request, requestPhase, pendingResult,
       cachedOutcome, cachedHostEpoch, cachedTransition, terminalState,
       terminalHostEpoch, hostEffects>>

LoseHostConnection ==
  /\ hostConnected
  /\ hostConnected' = FALSE
  /\ IF requestPhase = "handed-off"
       THEN QueueAcceptedResult("outcome-unknown", hostEpoch, "none")
       ELSE UNCHANGED <<pendingResult, request, requestPhase,
              cachedOutcome, cachedHostEpoch, cachedTransition>>
  /\ UNCHANGED <<serverConnected, workerConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, terminalState,
       terminalHostEpoch, hostEffects>>

RestoreHostConnection ==
  /\ ~hostConnected
  /\ hostConnected' = TRUE
  /\ UNCHANGED <<serverConnected, workerConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, request, requestPhase,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       terminalState, terminalHostEpoch,
       hostEffects>>

ReplaceHostEpoch ==
  /\ ~hostConnected
  /\ hostEpoch < MaxHostEpoch
  /\ hostConnected' = TRUE
  /\ hostEpoch' = hostEpoch + 1
  /\ terminalState' = IF terminalState \in {"awaiting-start", "started"}
                       THEN "lost"
                       ELSE terminalState
  /\ terminalHostEpoch' = 0
  /\ UNCHANGED <<serverConnected, workerConnected, workerGeneration,
       leaseValid, acceptedBinding, authority, request, requestPhase,
       pendingResult, cachedOutcome, cachedHostEpoch, cachedTransition,
       hostEffects>>

Next ==
  \/ IssueCurrentLaunch
  \* Keep the concrete launch modes visible to TLC action coverage as well as
  \* the aggregate launch action; these branches generate the same states.
  \/ IssueCurrentImmediateLaunch
  \/ IssueCurrentDeferredLaunch
  \/ IssueStaleGenerationLaunch
  \/ IssueLegacyCapabilityLaunch
  \/ IssueLowFenceReplay
  \/ IssueExactReplay
  \/ IssueConflictingReplay
  \/ IssueAuthorizedStart
  \/ AdmitAuthorization
  \/ RejectStaleGeneration
  \/ RejectUnsupportedCapability
  \/ ExpireLeaseBeforeAdmission
  \/ RejectExpiredBeforeAdmission
  \/ RejectLowFence
  \/ RejectConflictingReplay
  \/ RejectHostIdentityChanged
  \/ ReplayCachedAcceptedResult
  \/ ReplayCachedRejectedResult
  \/ ReplayCachedFailedResult
  \/ ReplayCachedUnknownResult
  \/ ReplayCachedExpiredLeaseResult
  \/ ExpireLeaseAfterAdmission
  \/ RejectExpiredAfterAdmission
  \/ RejectHostIdentityChangedAfterAdmission
  \/ FailBeforeHostHandoff
  \/ HandoffToHost
  \/ ExpireLeaseAfterHandoff
  \/ ExpireCachedLease
  \/ HostAcceptImmediateLaunch
  \/ HostAcceptDeferredLaunch
  \/ HostAcceptStarted
  \/ HostAcceptAlreadyStarted
  \/ HostReject
  \/ HostFail
  \/ DeliverCurrentResult
  \/ DropStaleResult
  \/ LoseServerConnection
  \/ RestoreServerConnection
  \/ LoseIdleWorkerConnection
  \/ LoseWorkerConnectionBeforeAdmission
  \/ LoseWorkerConnectionAfterAdmission
  \/ LoseWorkerConnectionAfterHandoff
  \/ ReconnectWorker
  \/ LoseHostConnection
  \/ RestoreHostConnection
  \/ ReplaceHostEpoch

SettlePendingResult == DeliverCurrentResult \/ DropStaleResult

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SettlePendingResult)

TypeOK ==
  /\ serverConnected \in BOOLEAN
  /\ workerConnected \in BOOLEAN
  /\ hostConnected \in BOOLEAN
  /\ workerGeneration \in Generations
  /\ hostEpoch \in HostEpochs
  /\ leaseValid \in [Leases -> BOOLEAN]
  /\ acceptedBinding \in BindingValues
  /\ authority \in BindingValues
  /\ request \in BindingValues
  /\ requestPhase \in RequestPhases
  /\ pendingResult \in ResultValues
  /\ cachedOutcome \in ResultOutcomes \cup {"none"}
  /\ cachedHostEpoch \in 0..MaxHostEpoch
  /\ cachedTransition \in ResultTransitions
  /\ terminalState \in TerminalStates
  /\ terminalHostEpoch \in 0..MaxHostEpoch
  /\ hostEffects \subseteq HostEffects

RequestPhaseHasBinding ==
  (requestPhase = "none") <=> (request = NoBinding)

AcceptedBindingUsesSupportedCapability ==
  acceptedBinding # NoBinding => acceptedBinding.capability = SupportedCapability

OutstandingRequestOwnsEmptyResultSlot ==
  requestPhase # "none" => pendingResult = NoResult

CachedResultIsClosed ==
  /\ (HasCachedResult => acceptedBinding \in Bindings)
  /\ (cachedOutcome = "none" =>
       /\ cachedHostEpoch = 0
       /\ cachedTransition = "none")
  /\ (cachedOutcome = "host-accepted" =>
       /\ cachedHostEpoch \in HostEpochs
       /\ HasEffect(acceptedBinding, cachedHostEpoch)
       /\ (acceptedBinding.kind = "launch-deferred" =>
             cachedTransition = "awaiting-start")
       /\ (acceptedBinding.kind = "launch-immediate" =>
             cachedTransition = "started")
       /\ (acceptedBinding.kind = "start" =>
             cachedTransition \in {"started", "already-started"}))
  /\ (cachedOutcome \in {"rejected", "failed"} =>
       /\ cachedHostEpoch \in {0} \cup HostEpochs
       /\ ( \/ cachedHostEpoch = 0
            \/ HasEffect(acceptedBinding, cachedHostEpoch)
            \/ /\ cachedOutcome = "rejected"
               /\ acceptedBinding.kind = "start"
               /\ acceptedBinding.targetEpoch # cachedHostEpoch )
       /\ cachedTransition = "none")
  /\ (cachedOutcome = "outcome-unknown" =>
       /\ cachedHostEpoch \in HostEpochs
       /\ (acceptedBinding.kind = "start" =>
             acceptedBinding.targetEpoch = cachedHostEpoch)
       /\ cachedTransition = "none")

TerminalIdentityIsComplete ==
  (terminalState \in {"awaiting-start", "started"}) <=>
    (terminalHostEpoch \in HostEpochs)

AdmittedAuthorizationIsCurrentAuthority ==
  [][AdmitAuthorization => acceptedBinding' = authority']_vars

AdmissionRequiresConnectedWorker ==
  [][AdmitAuthorization => workerConnected]_vars

NewAdmissionClearsPriorCachedResult ==
  [][AdmitAuthorization =>
      /\ cachedOutcome' = "none"
      /\ cachedHostEpoch' = 0
      /\ cachedTransition' = "none"]_vars

AuthoritativeIssuance == IssueCurrentLaunch \/ IssueAuthorizedStart

AuthoritativeOperationFreshnessSpansWorkerReconnect ==
  /\ [][AuthoritativeIssuance => FreshOperation(request'.operation)]_vars
  /\ [][ReconnectWorker =>
      /\ authority' = authority
      /\ hostEffects' = hostEffects]_vars

HostEffectAtMostOncePerOperation ==
  \A operation \in Operations:
    Cardinality({effect \in hostEffects: effect.operation = operation}) <= 1

HandoffRequiresLiveLease ==
  [][HandoffToHost =>
      /\ request # NoBinding
      /\ leaseValid[request.lease]]_vars

AdmittedHostIdentityChangeIsCachedAndEffectFree ==
  [][RejectHostIdentityChangedAfterAdmission =>
      /\ pendingResult' = Result(request, "rejected", hostEpoch, "none")
      /\ cachedOutcome' = "rejected"
      /\ cachedHostEpoch' = hostEpoch
      /\ cachedTransition' = "none"
      /\ requestPhase' = "none"
      /\ ~HasOperationEffect(request.operation)
      /\ hostEffects' = hostEffects]_vars

ExpiredAcceptedLeaseCannotRevive ==
  [][( /\ acceptedBinding # NoBinding
       /\ ~leaseValid[acceptedBinding.lease]
       /\ acceptedBinding' = acceptedBinding ) =>
      ~leaseValid'[acceptedBinding.lease]]_vars

UnknownOutcomeRetainsDispatchObservation ==
  pendingResult # NoResult /\ pendingResult.outcome = "outcome-unknown" =>
    /\ pendingResult.binding \in Bindings
    /\ pendingResult.hostEpoch \in HostEpochs
    /\ (pendingResult.binding.kind = "start" =>
          pendingResult.binding.targetEpoch = pendingResult.hostEpoch)

HostAcceptedRequiresHostEffect ==
  pendingResult # NoResult /\ pendingResult.outcome = "host-accepted" =>
    HasEffect(pendingResult.binding, pendingResult.hostEpoch)

StartSuccessPreservesTargetHostEpoch ==
  pendingResult # NoResult /\ pendingResult.binding.kind = "start" /\
  pendingResult.outcome = "host-accepted" =>
    /\ pendingResult.binding.targetEpoch = pendingResult.hostEpoch
    /\ pendingResult.transition \in {"started", "already-started"}

CachedReplay ==
  \/ ReplayCachedAcceptedResult
  \/ ReplayCachedRejectedResult
  \/ ReplayCachedFailedResult
  \/ ReplayCachedUnknownResult
  \/ ReplayCachedExpiredLeaseResult

ExactReplayPreservesCachedResult ==
  [][CachedReplay =>
      /\ pendingResult' = CachedResult
      /\ cachedOutcome' = cachedOutcome
      /\ cachedHostEpoch' = cachedHostEpoch
      /\ cachedTransition' = cachedTransition
      /\ terminalState' = terminalState
      /\ terminalHostEpoch' = terminalHostEpoch
      /\ hostEffects' = hostEffects]_vars

ExpiredCachedReplayIsStableAndEffectFree ==
  [][ReplayCachedExpiredLeaseResult =>
      /\ ~leaseValid[acceptedBinding.lease]
      /\ pendingResult' = CachedResult
      /\ hostEffects' = hostEffects]_vars

WorkerLossBeforeAdmissionIsUnknown ==
  [][LoseWorkerConnectionBeforeAdmission =>
      /\ pendingResult'.binding = request
      /\ pendingResult'.outcome = "outcome-unknown"
      /\ requestPhase' = "none"]_vars

WorkerLossAfterAdmissionIsUnknown ==
  [][LoseWorkerConnectionAfterAdmission =>
      /\ pendingResult'.binding = request
      /\ pendingResult'.outcome = "outcome-unknown"
      /\ cachedOutcome' = "outcome-unknown"
      /\ requestPhase' = "none"]_vars

WorkerLossAfterHandoffIsUnknown ==
  [][LoseWorkerConnectionAfterHandoff =>
      /\ pendingResult'.binding = request
      /\ pendingResult'.outcome = "outcome-unknown"
      /\ cachedOutcome' = "outcome-unknown"
      /\ requestPhase' = "none"]_vars

WorkerGenerationActivationSettlesPendingObservation ==
  [][ReconnectWorker => pendingResult = NoResult]_vars

DeliveredResultWasCurrent ==
  [][DeliverCurrentResult =>
      /\ pendingResult.binding = authority
      /\ pendingResult.binding.generation = workerGeneration
      /\ pendingResult.hostEpoch \in {0, hostEpoch}]_vars

ConnectionLossPreservesRetainedTerminal ==
  [][( \/ LoseServerConnection
       \/ LoseIdleWorkerConnection
       \/ LoseWorkerConnectionBeforeAdmission
       \/ LoseWorkerConnectionAfterAdmission
       \/ LoseWorkerConnectionAfterHandoff
       \/ LoseHostConnection ) =>
      UNCHANGED <<terminalState, terminalHostEpoch>>]_vars

HealthyPendingResultEventuallySettles ==
  [](pendingResult # NoResult /\ serverConnected =>
      <>(pendingResult = NoResult \/ ~serverConnected))

=============================================================================
