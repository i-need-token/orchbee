---------------- MODULE TerminalLaunchAuthorization ----------------
EXTENDS FiniteSets, Naturals, TLC

\* Models the Server-owned authorization envelope and Worker admission for
\* one retained terminal. The abstract binding corresponds to the complete
\* Server-Worker operation binding: Worker generation, operation identity,
\* per-terminal fence, stateful lease, capability version, operation kind,
\* and (for Start) the target Host epoch. Host effects happen only after an
\* admitted handoff. Transport loss alone never terminates a retained
\* terminal; Host process replacement is a separate epoch-changing action.

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
  /\ terminalState = "none"
  /\ terminalHostEpoch = 0
  /\ hostEffects = {}

CanIssue ==
  /\ serverConnected
  /\ workerConnected
  /\ requestPhase = "none"
  /\ authority = NoBinding \/ authority.fence < MaxFence

FreshOperation(operation) ==
  /\ operation \in Operations
  /\ ~HasOperationEffect(operation)
  /\ (authority = NoBinding \/ operation # authority.operation)

OperationForFence(fence) ==
  IF fence = 1 THEN FirstOperation ELSE SecondOperation

LeaseForFence(fence) ==
  IF fence = 1 THEN FirstLease ELSE SecondLease

OtherOperation(operation) ==
  IF operation = FirstOperation THEN SecondOperation ELSE FirstOperation

IssueCurrentLaunch(kind) ==
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
           1,
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
       terminalState, terminalHostEpoch, hostEffects>>

IssueCurrentImmediateLaunch ==
  IssueCurrentLaunch("launch-immediate")

IssueCurrentDeferredLaunch ==
  IssueCurrentLaunch("launch-deferred")

IssueStaleGenerationLaunch ==
  /\ serverConnected
  /\ workerConnected
  /\ workerGeneration > 1
  /\ requestPhase = "none"
  /\ request' = Binding(
       workerGeneration - 1,
       1,
       FirstOperation,
       FirstLease,
       "launch-immediate",
       1,
       0
     )
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, terminalState, terminalHostEpoch,
       hostEffects>>

IssueUnsupportedCapabilityLaunch ==
  /\ serverConnected
  /\ workerConnected
  /\ requestPhase = "none"
  /\ request' = Binding(
       workerGeneration,
       1,
       FirstOperation,
       FirstLease,
       "launch-immediate",
       2,
       0
     )
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, terminalState, terminalHostEpoch,
       hostEffects>>

IssueLowFenceReplay ==
  /\ serverConnected
  /\ workerConnected
  /\ requestPhase = "none"
  /\ acceptedBinding # NoBinding
  /\ acceptedBinding.fence > 1
  /\ request' = Binding(
       workerGeneration,
       acceptedBinding.fence - 1,
       acceptedBinding.operation,
       acceptedBinding.lease,
       acceptedBinding.kind,
       1,
       acceptedBinding.targetEpoch
     )
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, terminalState, terminalHostEpoch,
       hostEffects>>

IssueExactReplay ==
  /\ serverConnected
  /\ workerConnected
  /\ requestPhase = "none"
  /\ acceptedBinding # NoBinding
  /\ leaseValid[acceptedBinding.lease]
  /\ request' = acceptedBinding
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, terminalState, terminalHostEpoch,
       hostEffects>>

IssueConflictingReplay ==
  /\ serverConnected
  /\ workerConnected
  /\ requestPhase = "none"
  /\ acceptedBinding # NoBinding
  /\ request' = [acceptedBinding EXCEPT
       !.operation = OtherOperation(acceptedBinding.operation)]
  /\ requestPhase' = "issued"
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       pendingResult, terminalState, terminalHostEpoch,
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
           1,
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
       terminalState, terminalHostEpoch, hostEffects>>

QueueResult(outcome, epoch, transition) ==
  /\ pendingResult = NoResult
  /\ pendingResult' = Result(request, outcome, epoch, transition)
  /\ request' = NoBinding
  /\ requestPhase' = "none"

RejectStaleGeneration ==
  /\ requestPhase = "issued"
  /\ request.generation # workerGeneration
  /\ QueueResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

RejectUnsupportedCapability ==
  /\ requestPhase = "issued"
  /\ request.generation = workerGeneration
  /\ request.capability # 1
  /\ QueueResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

ExpireLeaseBeforeAdmission ==
  /\ requestPhase = "issued"
  /\ leaseValid[request.lease]
  /\ leaseValid' = [leaseValid EXCEPT ![request.lease] = FALSE]
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, acceptedBinding, authority, request,
       requestPhase, pendingResult, terminalState,
       terminalHostEpoch, hostEffects>>

RejectExpiredBeforeAdmission ==
  /\ requestPhase = "issued"
  /\ ~leaseValid[request.lease]
  /\ QueueResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

RejectLowFence ==
  /\ requestPhase = "issued"
  /\ request.generation = workerGeneration
  /\ request.capability = 1
  /\ leaseValid[request.lease]
  /\ acceptedBinding # NoBinding
  /\ request.fence < acceptedBinding.fence
  /\ QueueResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

RejectConflictingReplay ==
  /\ requestPhase = "issued"
  /\ request.generation = workerGeneration
  /\ request.capability = 1
  /\ leaseValid[request.lease]
  /\ acceptedBinding # NoBinding
  /\ request.fence = acceptedBinding.fence
  /\ request # acceptedBinding
  /\ QueueResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

RejectHostIdentityChanged ==
  /\ requestPhase = "issued"
  /\ request.kind = "start"
  /\ request.generation = workerGeneration
  /\ request.capability = 1
  /\ leaseValid[request.lease]
  /\ request.targetEpoch # hostEpoch
  /\ QueueResult("rejected", hostEpoch, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

AcceptExactReplay ==
  /\ requestPhase = "issued"
  /\ request = acceptedBinding
  /\ HasOperationEffect(request.operation)
  /\ terminalState \in {"awaiting-start", "started"}
  /\ LET transition ==
       IF request.kind = "start"
       THEN "already-started"
       ELSE terminalState
     IN QueueResult("host-accepted", terminalHostEpoch, transition)
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

AdmitAuthorization ==
  /\ requestPhase = "issued"
  /\ request.generation = workerGeneration
  /\ request.capability = 1
  /\ leaseValid[request.lease]
  /\ (request.kind # "start" \/ request.targetEpoch = hostEpoch)
  /\ acceptedBinding = NoBinding \/ request.fence > acceptedBinding.fence
  /\ acceptedBinding' = request
  /\ requestPhase' = "admitted"
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
       requestPhase, pendingResult, terminalState,
       terminalHostEpoch, hostEffects>>

RejectExpiredAfterAdmission ==
  /\ requestPhase = "admitted"
  /\ ~leaseValid[request.lease]
  /\ QueueResult("rejected", 0, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

FailBeforeHostHandoff ==
  /\ requestPhase = "admitted"
  /\ pendingResult = NoResult
  /\ QueueResult("failed", 0, "none")
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
       request, pendingResult, terminalState,
       terminalHostEpoch>>

ExpireLeaseAfterHandoff ==
  /\ requestPhase = "handed-off"
  /\ leaseValid[request.lease]
  /\ leaseValid' = [leaseValid EXCEPT ![request.lease] = FALSE]
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, acceptedBinding, authority, request,
       requestPhase, pendingResult, terminalState,
       terminalHostEpoch, hostEffects>>

HostAccept ==
  /\ requestPhase = "handed-off"
  /\ hostConnected
  /\ pendingResult = NoResult
  /\ HasEffect(request, hostEpoch)
  /\ IF request.kind = "launch-deferred"
       THEN /\ terminalState' = "awaiting-start"
            /\ terminalHostEpoch' = hostEpoch
            /\ QueueResult("host-accepted", hostEpoch, "awaiting-start")
       ELSE /\ terminalState' = "started"
            /\ terminalHostEpoch' = hostEpoch
            /\ QueueResult("host-accepted", hostEpoch, "started")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       hostEffects>>

HostReject ==
  /\ requestPhase = "handed-off"
  /\ hostConnected
  /\ HasEffect(request, hostEpoch)
  /\ QueueResult("rejected", hostEpoch, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

HostFail ==
  /\ requestPhase = "handed-off"
  /\ hostConnected
  /\ HasEffect(request, hostEpoch)
  /\ QueueResult("failed", hostEpoch, "none")
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       terminalState, terminalHostEpoch, hostEffects>>

DeliverCurrentResult ==
  /\ serverConnected
  /\ ResultMatchesAuthority(pendingResult)
  /\ pendingResult' = NoResult
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       request, requestPhase, terminalState, terminalHostEpoch, hostEffects>>

DropStaleResult ==
  /\ serverConnected
  /\ pendingResult # NoResult
  /\ ~ResultMatchesAuthority(pendingResult)
  /\ pendingResult' = NoResult
  /\ UNCHANGED <<serverConnected, workerConnected, hostConnected,
       workerGeneration, hostEpoch, leaseValid, acceptedBinding, authority,
       request, requestPhase, terminalState,
       terminalHostEpoch, hostEffects>>

LoseServerConnection ==
  /\ serverConnected
  /\ serverConnected' = FALSE
  /\ UNCHANGED <<workerConnected, hostConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, request, requestPhase,
       pendingResult, terminalState, terminalHostEpoch,
       hostEffects>>

RestoreServerConnection ==
  /\ ~serverConnected
  /\ serverConnected' = TRUE
  /\ UNCHANGED <<workerConnected, hostConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, request, requestPhase,
       pendingResult, terminalState, terminalHostEpoch,
       hostEffects>>

LoseWorkerConnection ==
  /\ workerConnected
  /\ workerConnected' = FALSE
  /\ IF requestPhase \in {"issued", "admitted", "handed-off"} /\
        pendingResult = NoResult
       THEN /\ pendingResult' = Result(
                    request,
                    "outcome-unknown",
                    IF request.kind = "start"
                    THEN request.targetEpoch
                    ELSE hostEpoch,
                    "none"
                  )
            /\ request' = NoBinding
            /\ requestPhase' = "none"
       ELSE /\ UNCHANGED <<pendingResult, request, requestPhase>>
  /\ UNCHANGED <<serverConnected, hostConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, terminalState,
       terminalHostEpoch, hostEffects>>

ReconnectWorker ==
  /\ ~workerConnected
  /\ workerGeneration < MaxGeneration
  /\ workerConnected' = TRUE
  /\ workerGeneration' = workerGeneration + 1
  /\ UNCHANGED <<serverConnected, hostConnected, hostEpoch, leaseValid,
       acceptedBinding, authority, request, requestPhase, pendingResult,
       terminalState, terminalHostEpoch, hostEffects>>

LoseHostConnection ==
  /\ hostConnected
  /\ hostConnected' = FALSE
  /\ IF requestPhase = "handed-off" /\ pendingResult = NoResult
       THEN /\ pendingResult' = Result(
                    request,
                    "outcome-unknown",
                    hostEpoch,
                    "none"
                  )
            /\ request' = NoBinding
            /\ requestPhase' = "none"
       ELSE /\ UNCHANGED <<pendingResult, request, requestPhase>>
  /\ UNCHANGED <<serverConnected, workerConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, terminalState,
       terminalHostEpoch, hostEffects>>

RestoreHostConnection ==
  /\ ~hostConnected
  /\ hostConnected' = TRUE
  /\ UNCHANGED <<serverConnected, workerConnected, workerGeneration, hostEpoch,
       leaseValid, acceptedBinding, authority, request, requestPhase,
       pendingResult, terminalState, terminalHostEpoch,
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
       pendingResult, hostEffects>>

Next ==
  \/ IssueCurrentImmediateLaunch
  \/ IssueCurrentDeferredLaunch
  \/ IssueStaleGenerationLaunch
  \/ IssueUnsupportedCapabilityLaunch
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
  \/ AcceptExactReplay
  \/ ExpireLeaseAfterAdmission
  \/ RejectExpiredAfterAdmission
  \/ FailBeforeHostHandoff
  \/ HandoffToHost
  \/ ExpireLeaseAfterHandoff
  \/ HostAccept
  \/ HostReject
  \/ HostFail
  \/ DeliverCurrentResult
  \/ DropStaleResult
  \/ LoseServerConnection
  \/ RestoreServerConnection
  \/ LoseWorkerConnection
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
  /\ terminalState \in TerminalStates
  /\ terminalHostEpoch \in 0..MaxHostEpoch
  /\ hostEffects \subseteq HostEffects

RequestPhaseHasBinding ==
  (requestPhase = "none") <=> (request = NoBinding)

TerminalIdentityIsComplete ==
  (terminalState \in {"awaiting-start", "started"}) <=>
    (terminalHostEpoch \in HostEpochs)

AdmittedAuthorizationIsCurrentAuthority ==
  [][AdmitAuthorization => acceptedBinding' = authority']_vars

HostEffectAtMostOncePerOperation ==
  \A operation \in Operations:
    Cardinality({effect \in hostEffects: effect.operation = operation}) <= 1

HandoffRequiresLiveLease ==
  [][HandoffToHost =>
      /\ request # NoBinding
      /\ leaseValid[request.lease]]_vars

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

DeliveredResultWasCurrent ==
  [][DeliverCurrentResult =>
      /\ pendingResult.binding = authority
      /\ pendingResult.binding.generation = workerGeneration
      /\ pendingResult.hostEpoch \in {0, hostEpoch}]_vars

ConnectionLossPreservesRetainedTerminal ==
  [][(LoseServerConnection \/ LoseWorkerConnection \/ LoseHostConnection) =>
      UNCHANGED <<terminalState, terminalHostEpoch>>]_vars

HealthyPendingResultEventuallySettles ==
  [](pendingResult # NoResult /\ serverConnected =>
      <>(pendingResult = NoResult \/ ~serverConnected))

=============================================================================
