-------------------------- MODULE LocalHostBroker --------------------------
EXTENDS FiniteSets, Naturals, TLC

\* Models one admitted OBCN Launch or Start dispatch composed with the local
\* Worker-to-Terminal-Host broker. Server admission still precedes Host
\* contact. The admitted dispatch is an input from the separately checked
\* TerminalLaunchAuthorization model; this module owns only the adapter order:
\* begin_host_handoff (including its cached expired-lease ResultReady path),
\* one exactly correlated Terminal Protocol command, one terminal result, and
\* bounded Server delivery. Its detailed slot phases refine the separate
\* WorkerTerminalBindingCapacity model's one operation-pending/result slot.
\* An exactly validated generic TypedFailure has no
\* authoritative rejection discriminator and therefore maps to
\* HostReportedFailure; a mismatched record is protocol-unknown, never
\* HostRejected. The active broker connection's HostEpoch is immutable; a Host
\* replacement requires connection teardown and a fresh Hello outside this
\* one-connection lifecycle. A Server loss closes Host immediately. A Host loss may retain
\* Server only for one bounded result delivery, then closes it. A retained Host
\* observed after Worker loss is classified RecoveryRequired and is never
\* silently adopted.

WorkerPhases == {
  "absent", "connecting-server", "connecting-host", "host-handshake",
  "host-admitted", "ready", "draining-result", "stopped"
}
LinkPhases == {"closed", "connecting", "active"}
StopCauses == {
  "none", "server-rejected", "server-lost", "host-peer-rejected",
  "host-wrong-role", "host-incompatible", "host-malformed",
  "host-handshake-timeout", "host-lost", "host-command-timeout",
  "result-delivery-timeout"
}
DispatchKinds == {"none", "launch", "start"}
OperationPhases == {
  "none", "admitted", "handed-off", "awaiting-host", "result-ready",
  "completed", "abandoned"
}
HostCommandKinds == {"none", "create-terminal", "start-terminal"}
ObservedHostKinds == {
  "none", "create-terminal", "start-terminal", "terminal-start-accepted",
  "typed-failure"
}
HostResponseRoles == {"none", "request", "response"}
HostEpochs == 0..2
ResultOutcomes == {"none", "accepted", "rejected", "failed", "unknown"}
CompletionSources == {
  "none", "handoff-lease-expired", "host-unavailable-before-handoff", "host-accepted",
  "host-reported-failure", "host-protocol", "host-loss", "host-timeout",
  "server-loss"
}
DeliveryPhases == {"none", "pending", "delivered", "abandoned"}
Correlations == {"none", "operation", "other"}
KernelSlotPhases == {
  "empty", "awaiting-handoff", "handed-off", "result-ready"
}
CapacitySlotPhases == {"empty", "operation-pending", "result-ready"}
HandoffRejectionSources == {"handoff-lease-expired"}
RejectedHostResponseClasses == {
  "wrong-role", "wrong-kind", "wrong-correlation", "wrong-host-epoch"
}
RetentionDispositions == {
  "absent", "current-worker", "unmatched", "recovery-required"
}
Actions == {
  "init", "start-worker", "accept-server", "reject-server",
  "connect-host", "reject-host-peer", "accept-host-handshake",
  "reject-host-wrong-role", "reject-host-incompatible",
  "reject-host-malformed", "host-handshake-timeout", "emit-ready",
  "admit-launch-dispatch", "admit-start-dispatch", "begin-host-handoff",
  "handoff-lease-expired-result", "fail-before-host-handoff",
  "dispatch-host-command",
  "host-applies-command", "reject-wrong-host-response-role",
  "reject-wrong-host-response-kind", "reject-wrong-host-correlation",
  "reject-wrong-host-epoch", "host-accepts-command",
  "host-reports-typed-failure", "host-command-timeout",
  "block-retry-after-unknown", "deliver-result", "result-delivery-timeout",
  "lose-server-before-ready", "lose-server-idle",
  "lose-server-before-handoff", "lose-server-after-handoff",
  "lose-server-with-pending-result", "lose-server-during-drain",
  "lose-host-before-ready", "lose-host-idle", "lose-host-before-handoff",
  "lose-host-after-handoff", "lose-host-with-pending-result",
  "require-recovery-for-retained-host"
}

VARIABLES
  workerPhase,
  serverLink,
  hostLink,
  stopCause,
  helloSent,
  readinessEmitted,
  hostEpoch,
  dispatchKind,
  operationPhase,
  kernelSlotPhase,
  handoffAttemptCount,
  handoffCount,
  startTargetHostEpoch,
  handoffHostEpoch,
  hostCommandKind,
  hostCommandCount,
  hostCommandApplied,
  hostCorrelation,
  hostResponseRole,
  hostResponseKind,
  hostResponseCorrelation,
  hostResponseEpoch,
  rejectedHostResponses,
  resultOutcome,
  completionSource,
  bindingResultCached,
  deliveryPhase,
  resultDeliveryCount,
  retryAfterUnknownBlocked,
  retainedHostTerminal,
  retentionDisposition,
  lastAction

brokerState == <<
  workerPhase, serverLink, hostLink, stopCause, helloSent, readinessEmitted,
  hostEpoch
>>
operationState == <<
  dispatchKind, operationPhase, kernelSlotPhase, handoffAttemptCount,
  handoffCount, startTargetHostEpoch, handoffHostEpoch, hostCommandKind,
  hostCommandCount, hostCommandApplied, hostCorrelation, hostResponseRole,
  hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
  rejectedHostResponses, resultOutcome, completionSource,
  bindingResultCached, deliveryPhase, resultDeliveryCount,
  retryAfterUnknownBlocked
>>
retentionState == <<retainedHostTerminal, retentionDisposition>>
vars == <<brokerState, operationState, retentionState, lastAction>>

HandoffResultProjection == <<
  dispatchKind,
  resultOutcome,
  completionSource,
  startTargetHostEpoch,
  handoffHostEpoch
>>

CapacitySlotProjection ==
  CASE kernelSlotPhase = "empty" -> "empty"
    [] kernelSlotPhase \in {"awaiting-handoff", "handed-off"} ->
         "operation-pending"
    [] OTHER -> "result-ready"

ExpectedHostCommand(kind) ==
  CASE kind = "launch" -> "create-terminal"
    [] kind = "start" -> "start-terminal"
    [] OTHER -> "none"

ExpectedSuccessResponse(kind) ==
  CASE kind = "launch" -> "create-terminal"
    [] kind = "start" -> "terminal-start-accepted"
    [] OTHER -> "none"

OtherSuccessResponse(kind) ==
  CASE kind = "launch" -> "terminal-start-accepted"
    [] kind = "start" -> "create-terminal"
    [] OTHER -> "create-terminal"

OtherHostEpoch(epoch) == IF epoch = 1 THEN 2 ELSE 1

Init ==
  /\ workerPhase = "absent"
  /\ serverLink = "closed"
  /\ hostLink = "closed"
  /\ stopCause = "none"
  /\ helloSent = FALSE
  /\ readinessEmitted = FALSE
  /\ hostEpoch = 0
  /\ dispatchKind = "none"
  /\ operationPhase = "none"
  /\ kernelSlotPhase = "empty"
  /\ handoffAttemptCount = 0
  /\ handoffCount = 0
  /\ startTargetHostEpoch = 0
  /\ handoffHostEpoch = 0
  /\ hostCommandKind = "none"
  /\ hostCommandCount = 0
  /\ hostCommandApplied = FALSE
  /\ hostCorrelation = "none"
  /\ hostResponseRole = "none"
  /\ hostResponseKind = "none"
  /\ hostResponseCorrelation = "none"
  /\ hostResponseEpoch = 0
  /\ rejectedHostResponses = {}
  /\ resultOutcome = "none"
  /\ completionSource = "none"
  /\ bindingResultCached = FALSE
  /\ deliveryPhase = "none"
  /\ resultDeliveryCount = 0
  /\ retryAfterUnknownBlocked = FALSE
  /\ retainedHostTerminal = FALSE
  /\ retentionDisposition = "absent"
  /\ lastAction = "init"

StartWorker ==
  /\ workerPhase = "absent"
  /\ workerPhase' = "connecting-server"
  /\ serverLink' = "connecting"
  /\ lastAction' = "start-worker"
  /\ UNCHANGED <<hostLink, stopCause, helloSent, readinessEmitted,
       hostEpoch, operationState, retentionState>>

AcceptServer ==
  /\ workerPhase = "connecting-server"
  /\ workerPhase' = "connecting-host"
  /\ serverLink' = "active"
  /\ lastAction' = "accept-server"
  /\ UNCHANGED <<hostLink, stopCause, helloSent, readinessEmitted,
       hostEpoch, operationState, retentionState>>

RejectServer ==
  /\ workerPhase = "connecting-server"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ stopCause' = "server-rejected"
  /\ lastAction' = "reject-server"
  /\ UNCHANGED <<hostLink, helloSent, readinessEmitted, hostEpoch,
       operationState, retentionState>>

ConnectHost ==
  /\ workerPhase = "connecting-host"
  /\ workerPhase' = "host-handshake"
  /\ hostLink' = "connecting"
  /\ helloSent' = TRUE
  /\ lastAction' = "connect-host"
  /\ UNCHANGED <<serverLink, stopCause, readinessEmitted, hostEpoch,
       operationState, retentionState>>

StopForHostAdmissionFailure(cause, action) ==
  /\ workerPhase \in {"connecting-host", "host-handshake", "host-admitted"}
  /\ cause \in StopCauses \ {
       "none", "server-rejected", "server-lost", "host-lost",
       "host-command-timeout", "result-delivery-timeout"
     }
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = cause
  /\ lastAction' = action
  /\ UNCHANGED <<helloSent, readinessEmitted, hostEpoch, operationState,
       retentionState>>

RejectHostPeer ==
  /\ workerPhase = "connecting-host"
  /\ StopForHostAdmissionFailure("host-peer-rejected", "reject-host-peer")

AcceptHostHandshake ==
  /\ workerPhase = "host-handshake"
  /\ workerPhase' = "host-admitted"
  /\ hostLink' = "active"
  /\ hostEpoch' = 1
  /\ lastAction' = "accept-host-handshake"
  /\ UNCHANGED <<serverLink, stopCause, helloSent, readinessEmitted,
       operationState, retentionState>>

RejectHostWrongRole ==
  /\ workerPhase = "host-handshake"
  /\ StopForHostAdmissionFailure("host-wrong-role", "reject-host-wrong-role")

RejectHostIncompatible ==
  /\ workerPhase = "host-handshake"
  /\ StopForHostAdmissionFailure(
       "host-incompatible",
       "reject-host-incompatible"
     )

RejectHostMalformed ==
  /\ workerPhase = "host-handshake"
  /\ StopForHostAdmissionFailure("host-malformed", "reject-host-malformed")

HostHandshakeTimeout ==
  /\ workerPhase = "host-handshake"
  /\ StopForHostAdmissionFailure(
       "host-handshake-timeout",
       "host-handshake-timeout"
     )

EmitReady ==
  /\ workerPhase = "host-admitted"
  /\ workerPhase' = "ready"
  /\ readinessEmitted' = TRUE
  /\ lastAction' = "emit-ready"
  /\ UNCHANGED <<serverLink, hostLink, stopCause, helloSent, hostEpoch,
       operationState, retentionState>>

CanAdmitDispatch ==
  \* Capacity rejection and SlotOccupied remain authorization-kernel results.
  \* Only the kernel's one globally admitted dispatch reaches this adapter.
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ dispatchKind = "none"
  /\ operationPhase = "none"
  /\ kernelSlotPhase = "empty"
  /\ deliveryPhase = "none"

AdmitLaunchDispatch ==
  /\ CanAdmitDispatch
  /\ dispatchKind' = "launch"
  /\ operationPhase' = "admitted"
  /\ kernelSlotPhase' = "awaiting-handoff"
  /\ startTargetHostEpoch' = 0
  /\ lastAction' = "admit-launch-dispatch"
  /\ UNCHANGED <<brokerState, handoffAttemptCount, handoffCount,
       handoffHostEpoch, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseRole, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultOutcome,
       completionSource, bindingResultCached, deliveryPhase,
       resultDeliveryCount, retryAfterUnknownBlocked, retentionState>>

AdmitStartDispatch ==
  /\ CanAdmitDispatch
  \* The exact authorized Start target is an already retained deferred terminal.
  /\ dispatchKind' = "start"
  /\ operationPhase' = "admitted"
  /\ kernelSlotPhase' = "awaiting-handoff"
  /\ startTargetHostEpoch' = hostEpoch
  /\ retainedHostTerminal' = TRUE
  /\ retentionDisposition' = "current-worker"
  /\ lastAction' = "admit-start-dispatch"
  /\ UNCHANGED <<brokerState, handoffAttemptCount, handoffCount,
       handoffHostEpoch, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseRole, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultOutcome,
       completionSource, bindingResultCached, deliveryPhase,
       resultDeliveryCount, retryAfterUnknownBlocked>>

BeginHostHandoff ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "admitted"
  /\ kernelSlotPhase = "awaiting-handoff"
  /\ handoffAttemptCount = 0
  /\ \/ dispatchKind = "launch"
     \/ startTargetHostEpoch = hostEpoch
  /\ operationPhase' = "handed-off"
  /\ kernelSlotPhase' = "handed-off"
  /\ handoffAttemptCount' = 1
  /\ handoffCount' = 1
  /\ handoffHostEpoch' = hostEpoch
  /\ lastAction' = "begin-host-handoff"
  /\ UNCHANGED <<brokerState, dispatchKind, startTargetHostEpoch,
       hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseRole, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultOutcome,
       completionSource, bindingResultCached, deliveryPhase,
       resultDeliveryCount, retryAfterUnknownBlocked, retentionState>>

QueueResult(outcome, source) ==
  /\ outcome \in ResultOutcomes \ {"none"}
  /\ source \in CompletionSources \ {"none"}
  /\ resultOutcome' = outcome
  /\ completionSource' = source
  /\ bindingResultCached' = TRUE
  /\ deliveryPhase' = "pending"
  /\ operationPhase' = "result-ready"
  /\ kernelSlotPhase' = "result-ready"
  /\ retryAfterUnknownBlocked' = (outcome = "unknown")

HandoffLeaseExpiredResult ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "admitted"
  /\ kernelSlotPhase = "awaiting-handoff"
  /\ handoffAttemptCount = 0
  /\ handoffAttemptCount' = 1
  /\ handoffHostEpoch' = hostEpoch
  /\ QueueResult("rejected", "handoff-lease-expired")
  /\ lastAction' = "handoff-lease-expired-result"
  /\ UNCHANGED <<brokerState, dispatchKind, handoffCount,
       startTargetHostEpoch, hostCommandKind, hostCommandCount,
       hostCommandApplied, hostCorrelation, hostResponseRole,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultDeliveryCount, retentionState>>

FailBeforeHostHandoff ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ operationPhase = "admitted"
  /\ QueueResult("failed", "host-unavailable-before-handoff")
  /\ lastAction' = "fail-before-host-handoff"
  /\ UNCHANGED <<brokerState, dispatchKind, handoffAttemptCount,
       handoffCount, startTargetHostEpoch, handoffHostEpoch,
       hostCommandKind, hostCommandCount, hostCommandApplied,
       hostCorrelation, hostResponseRole, hostResponseKind,
       hostResponseCorrelation, hostResponseEpoch, rejectedHostResponses,
       resultDeliveryCount, retentionState>>

DispatchHostCommand ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "handed-off"
  /\ kernelSlotPhase = "handed-off"
  /\ handoffCount = 1
  /\ handoffHostEpoch \in HostEpochs \ {0}
  /\ operationPhase' = "awaiting-host"
  /\ hostCommandKind' = ExpectedHostCommand(dispatchKind)
  /\ hostCommandCount' = 1
  /\ hostCorrelation' = "operation"
  /\ lastAction' = "dispatch-host-command"
  /\ UNCHANGED <<brokerState, dispatchKind, kernelSlotPhase,
       handoffAttemptCount, handoffCount, startTargetHostEpoch,
       handoffHostEpoch, hostCommandApplied, hostResponseRole,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource,
       bindingResultCached, deliveryPhase, resultDeliveryCount,
       retryAfterUnknownBlocked, retentionState>>

HostAppliesCommand ==
  /\ workerPhase = "ready"
  /\ hostLink = "active"
  /\ operationPhase = "awaiting-host"
  /\ ~hostCommandApplied
  /\ hostCommandApplied' = TRUE
  /\ retainedHostTerminal' = TRUE
  /\ retentionDisposition' = "current-worker"
  /\ lastAction' = "host-applies-command"
  /\ UNCHANGED <<brokerState, dispatchKind, operationPhase,
       kernelSlotPhase, handoffAttemptCount, handoffCount,
       startTargetHostEpoch, handoffHostEpoch, hostCommandKind,
       hostCommandCount, hostCorrelation, hostResponseRole,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource,
       bindingResultCached, deliveryPhase, resultDeliveryCount,
       retryAfterUnknownBlocked>>

KnownHostResponseMatches(role, kind, correlation, epoch) ==
  /\ role = "response"
  /\ correlation = hostCorrelation
  /\ epoch = handoffHostEpoch
  /\ kind \in {ExpectedSuccessResponse(dispatchKind), "typed-failure"}

RejectHostResponse(class, role, kind, correlation, epoch, action) ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "awaiting-host"
  /\ kernelSlotPhase = "handed-off"
  /\ class \in RejectedHostResponseClasses
  /\ class \notin rejectedHostResponses
  /\ role \in HostResponseRoles \ {"none"}
  /\ kind \in ObservedHostKinds \ {"none"}
  /\ correlation \in Correlations \ {"none"}
  /\ epoch \in HostEpochs \ {0}
  /\ ~KnownHostResponseMatches(role, kind, correlation, epoch)
  /\ workerPhase' = "draining-result"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-malformed"
  /\ rejectedHostResponses' = rejectedHostResponses \cup {class}
  /\ hostResponseRole' = role
  /\ hostResponseKind' = kind
  /\ hostResponseCorrelation' = correlation
  /\ hostResponseEpoch' = epoch
  /\ QueueResult("unknown", "host-protocol")
  /\ lastAction' = action
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, hostEpoch,
       dispatchKind, handoffAttemptCount, handoffCount,
       startTargetHostEpoch, handoffHostEpoch, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       resultDeliveryCount, retentionState>>

RejectWrongHostResponseRole ==
  /\ "wrong-role" \notin rejectedHostResponses
  /\ RejectHostResponse(
       "wrong-role",
       "request",
       hostCommandKind,
       hostCorrelation,
       handoffHostEpoch,
       "reject-wrong-host-response-role"
     )

RejectWrongHostResponseKind ==
  /\ "wrong-kind" \notin rejectedHostResponses
  /\ RejectHostResponse(
       "wrong-kind",
       "response",
       OtherSuccessResponse(dispatchKind),
       hostCorrelation,
       handoffHostEpoch,
       "reject-wrong-host-response-kind"
     )

RejectWrongHostCorrelation ==
  /\ "wrong-correlation" \notin rejectedHostResponses
  /\ RejectHostResponse(
       "wrong-correlation",
       "response",
       "typed-failure",
       "other",
       handoffHostEpoch,
       "reject-wrong-host-correlation"
     )

RejectWrongHostEpoch ==
  /\ "wrong-host-epoch" \notin rejectedHostResponses
  /\ RejectHostResponse(
       "wrong-host-epoch",
       "response",
       ExpectedSuccessResponse(dispatchKind),
       hostCorrelation,
       OtherHostEpoch(handoffHostEpoch),
       "reject-wrong-host-epoch"
     )

CompleteFromHost(outcome, source, role, kind, correlation, epoch, action) ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "awaiting-host"
  /\ kernelSlotPhase = "handed-off"
  /\ hostCommandCount = 1
  /\ hostCorrelation = "operation"
  /\ \/ outcome = "accepted" /\ source = "host-accepted"
     \/ outcome = "failed" /\ source = "host-reported-failure"
  /\ KnownHostResponseMatches(role, kind, correlation, epoch)
  /\ \/ outcome = "accepted" /\ kind = ExpectedSuccessResponse(dispatchKind)
     \/ outcome = "failed" /\ kind = "typed-failure"
  /\ QueueResult(outcome, source)
  /\ hostResponseRole' = role
  /\ hostResponseKind' = kind
  /\ hostResponseCorrelation' = correlation
  /\ hostResponseEpoch' = epoch
  /\ lastAction' = action
  /\ UNCHANGED <<brokerState, dispatchKind, handoffAttemptCount,
       handoffCount, startTargetHostEpoch, handoffHostEpoch,
       hostCommandKind, hostCommandCount, hostCommandApplied,
       hostCorrelation, rejectedHostResponses, resultDeliveryCount,
       retentionState>>

HostAcceptsCommand ==
  /\ hostCommandApplied
  /\ CompleteFromHost(
       "accepted",
       "host-accepted",
       "response",
       ExpectedSuccessResponse(dispatchKind),
       hostCorrelation,
       handoffHostEpoch,
       "host-accepts-command"
     )

HostReportsTypedFailure ==
  /\ ~hostCommandApplied
  /\ CompleteFromHost(
       "failed",
       "host-reported-failure",
       "response",
       "typed-failure",
       hostCorrelation,
       handoffHostEpoch,
       "host-reports-typed-failure"
     )

HostCommandTimeout ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase \in {"handed-off", "awaiting-host"}
  /\ workerPhase' = "draining-result"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-command-timeout"
  /\ QueueResult("unknown", "host-timeout")
  /\ lastAction' = "host-command-timeout"
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, hostEpoch,
       dispatchKind, handoffAttemptCount, handoffCount,
       startTargetHostEpoch, handoffHostEpoch, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseRole, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultDeliveryCount,
       retentionState>>

BlockRetryAfterUnknown ==
  \* An attempted resend is rejected by the barrier installed with unknown.
  \* Only lastAction records the blocked attempt; no Host state changes.
  /\ resultOutcome = "unknown"
  /\ operationPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ retryAfterUnknownBlocked
  /\ lastAction # "block-retry-after-unknown"
  /\ lastAction' = "block-retry-after-unknown"
  /\ UNCHANGED <<brokerState, operationState, retentionState>>

DeliverResult ==
  /\ serverLink = "active"
  /\ operationPhase = "result-ready"
  /\ kernelSlotPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ operationPhase' = "completed"
  /\ kernelSlotPhase' = "empty"
  /\ deliveryPhase' = "delivered"
  /\ resultDeliveryCount' = 1
  /\ workerPhase' = IF workerPhase = "draining-result"
                     THEN "stopped"
                     ELSE workerPhase
  /\ serverLink' = IF workerPhase = "draining-result"
                   THEN "closed"
                   ELSE serverLink
  /\ retentionDisposition' =
       IF workerPhase = "draining-result" /\ retainedHostTerminal
       THEN "unmatched"
       ELSE retentionDisposition
  /\ lastAction' = "deliver-result"
  /\ UNCHANGED <<hostLink, stopCause, helloSent, readinessEmitted,
       hostEpoch, dispatchKind, handoffAttemptCount, handoffCount,
       startTargetHostEpoch, handoffHostEpoch, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseRole, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultOutcome,
       completionSource, bindingResultCached, retryAfterUnknownBlocked,
       retainedHostTerminal>>

ResultDeliveryTimeout ==
  /\ serverLink = "active"
  /\ operationPhase = "result-ready"
  /\ kernelSlotPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "result-delivery-timeout"
  /\ operationPhase' = "abandoned"
  /\ kernelSlotPhase' = "empty"
  /\ deliveryPhase' = "abandoned"
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition
  /\ lastAction' = "result-delivery-timeout"
  /\ UNCHANGED <<helloSent, readinessEmitted, hostEpoch, dispatchKind,
       handoffAttemptCount, handoffCount, startTargetHostEpoch,
       handoffHostEpoch, hostCommandKind, hostCommandCount,
       hostCommandApplied, hostCorrelation, hostResponseRole,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource,
       bindingResultCached, resultDeliveryCount, retryAfterUnknownBlocked,
       retainedHostTerminal>>

LoseServerBeforeReady ==
  /\ serverLink = "active"
  /\ workerPhase \in {"connecting-host", "host-handshake", "host-admitted"}
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ lastAction' = "lose-server-before-ready"
  /\ UNCHANGED <<helloSent, readinessEmitted, hostEpoch, operationState,
       retentionState>>

LoseServerIdle ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ operationPhase \in {"none", "completed"}
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition
  /\ lastAction' = "lose-server-idle"
  /\ UNCHANGED <<helloSent, readinessEmitted, hostEpoch, operationState,
       retainedHostTerminal>>

AbandonForServerLoss ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ operationPhase' = "abandoned"
  /\ kernelSlotPhase' = "empty"
  /\ resultOutcome' = "unknown"
  /\ completionSource' = "server-loss"
  /\ bindingResultCached' = FALSE
  /\ deliveryPhase' = "abandoned"
  /\ retryAfterUnknownBlocked' = TRUE
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition

LoseServerBeforeHandoff ==
  /\ operationPhase = "admitted"
  /\ AbandonForServerLoss
  /\ lastAction' = "lose-server-before-handoff"
  /\ UNCHANGED <<helloSent, readinessEmitted, hostEpoch, dispatchKind,
       handoffAttemptCount, handoffCount, startTargetHostEpoch,
       handoffHostEpoch, hostCommandKind, hostCommandCount,
       hostCommandApplied, hostCorrelation, hostResponseRole,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultDeliveryCount, retainedHostTerminal>>

LoseServerAfterHandoff ==
  /\ operationPhase \in {"handed-off", "awaiting-host"}
  /\ AbandonForServerLoss
  /\ lastAction' = "lose-server-after-handoff"
  /\ UNCHANGED <<helloSent, readinessEmitted, hostEpoch, dispatchKind,
       handoffAttemptCount, handoffCount, startTargetHostEpoch,
       handoffHostEpoch, hostCommandKind, hostCommandCount,
       hostCommandApplied, hostCorrelation, hostResponseRole,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultDeliveryCount, retainedHostTerminal>>

LoseServerWithPendingResult ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ operationPhase = "result-ready"
  /\ kernelSlotPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ operationPhase' = "abandoned"
  /\ kernelSlotPhase' = "empty"
  /\ deliveryPhase' = "abandoned"
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition
  /\ lastAction' = "lose-server-with-pending-result"
  /\ UNCHANGED <<helloSent, readinessEmitted, hostEpoch, dispatchKind,
       handoffAttemptCount, handoffCount, startTargetHostEpoch,
       handoffHostEpoch, hostCommandKind, hostCommandCount,
       hostCommandApplied, hostCorrelation, hostResponseRole,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource,
       bindingResultCached, resultDeliveryCount, retryAfterUnknownBlocked,
       retainedHostTerminal>>

LoseServerDuringDrain ==
  /\ workerPhase = "draining-result"
  /\ serverLink = "active"
  /\ hostLink = "closed"
  /\ operationPhase = "result-ready"
  /\ kernelSlotPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ operationPhase' = "abandoned"
  /\ kernelSlotPhase' = "empty"
  /\ deliveryPhase' = "abandoned"
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition
  /\ lastAction' = "lose-server-during-drain"
  /\ UNCHANGED <<hostLink, helloSent, readinessEmitted, hostEpoch,
       dispatchKind, handoffAttemptCount, handoffCount,
       startTargetHostEpoch, handoffHostEpoch, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseRole, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultOutcome,
       completionSource, bindingResultCached, resultDeliveryCount,
       retryAfterUnknownBlocked, retainedHostTerminal>>

LoseHostBeforeReady ==
  /\ hostLink \in {"connecting", "active"}
  /\ workerPhase \in {"host-handshake", "host-admitted"}
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-lost"
  /\ lastAction' = "lose-host-before-ready"
  /\ UNCHANGED <<helloSent, readinessEmitted, hostEpoch, operationState,
       retentionState>>

LoseHostIdle ==
  /\ workerPhase = "ready"
  /\ hostLink = "active"
  /\ operationPhase \in {"none", "completed"}
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-lost"
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition
  /\ lastAction' = "lose-host-idle"
  /\ UNCHANGED <<helloSent, readinessEmitted, hostEpoch, operationState,
       retainedHostTerminal>>

DrainForHostLoss(outcome, source, action) ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ workerPhase' = "draining-result"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-lost"
  /\ QueueResult(outcome, source)
  /\ lastAction' = action

LoseHostBeforeHandoff ==
  /\ operationPhase = "admitted"
  /\ kernelSlotPhase = "awaiting-handoff"
  /\ DrainForHostLoss(
       "failed",
       "host-unavailable-before-handoff",
       "lose-host-before-handoff"
     )
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, hostEpoch,
       dispatchKind, handoffAttemptCount, handoffCount,
       startTargetHostEpoch, handoffHostEpoch, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseRole, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultDeliveryCount,
       retentionState>>

LoseHostAfterHandoff ==
  /\ operationPhase \in {"handed-off", "awaiting-host"}
  /\ kernelSlotPhase = "handed-off"
  /\ DrainForHostLoss("unknown", "host-loss", "lose-host-after-handoff")
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, hostEpoch,
       dispatchKind, handoffAttemptCount, handoffCount,
       startTargetHostEpoch, handoffHostEpoch, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseRole, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultDeliveryCount,
       retentionState>>

LoseHostWithPendingResult ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "result-ready"
  /\ kernelSlotPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ workerPhase' = "draining-result"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-lost"
  /\ lastAction' = "lose-host-with-pending-result"
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, hostEpoch,
       operationState, retentionState>>

RequireRecoveryForRetainedHost ==
  /\ workerPhase = "stopped"
  /\ retainedHostTerminal
  /\ retentionDisposition = "unmatched"
  /\ retentionDisposition' = "recovery-required"
  /\ lastAction' = "require-recovery-for-retained-host"
  /\ UNCHANGED <<brokerState, operationState, retainedHostTerminal>>

Next ==
  \/ StartWorker
  \/ AcceptServer
  \/ RejectServer
  \/ ConnectHost
  \/ RejectHostPeer
  \/ AcceptHostHandshake
  \/ RejectHostWrongRole
  \/ RejectHostIncompatible
  \/ RejectHostMalformed
  \/ HostHandshakeTimeout
  \/ EmitReady
  \/ AdmitLaunchDispatch
  \/ AdmitStartDispatch
  \/ BeginHostHandoff
  \/ HandoffLeaseExpiredResult
  \/ FailBeforeHostHandoff
  \/ DispatchHostCommand
  \/ HostAppliesCommand
  \/ RejectWrongHostResponseRole
  \/ RejectWrongHostResponseKind
  \/ RejectWrongHostCorrelation
  \/ RejectWrongHostEpoch
  \/ HostAcceptsCommand
  \/ HostReportsTypedFailure
  \/ HostCommandTimeout
  \/ BlockRetryAfterUnknown
  \/ DeliverResult
  \/ ResultDeliveryTimeout
  \/ LoseServerBeforeReady
  \/ LoseServerIdle
  \/ LoseServerBeforeHandoff
  \/ LoseServerAfterHandoff
  \/ LoseServerWithPendingResult
  \/ LoseServerDuringDrain
  \/ LoseHostBeforeReady
  \/ LoseHostIdle
  \/ LoseHostBeforeHandoff
  \/ LoseHostAfterHandoff
  \/ LoseHostWithPendingResult
  \/ RequireRecoveryForRetainedHost

SettlePendingWorker ==
  AcceptServer \/ RejectServer \/ ConnectHost \/ RejectHostPeer \/
  AcceptHostHandshake \/ RejectHostWrongRole \/ RejectHostIncompatible \/
  RejectHostMalformed \/ HostHandshakeTimeout \/ EmitReady \/
  LoseServerBeforeReady \/ LoseHostBeforeReady

SettleAdmittedOperation ==
  BeginHostHandoff \/ HandoffLeaseExpiredResult \/
  FailBeforeHostHandoff \/ LoseServerBeforeHandoff \/ LoseHostBeforeHandoff

SettleHandedOffOperation ==
  DispatchHostCommand \/ HostCommandTimeout \/ LoseServerAfterHandoff \/
  LoseHostAfterHandoff

SettleHostCommand ==
  HostAppliesCommand \/ HostAcceptsCommand \/ HostReportsTypedFailure \/
  HostCommandTimeout \/ LoseServerAfterHandoff \/ LoseHostAfterHandoff

SettlePendingResult ==
  DeliverResult \/ ResultDeliveryTimeout \/ LoseServerWithPendingResult \/
  LoseServerDuringDrain \/ LoseHostWithPendingResult

SettleRetainedHost ==
  RequireRecoveryForRetainedHost

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SettlePendingWorker)
  /\ WF_vars(SettleAdmittedOperation)
  /\ WF_vars(SettleHandedOffOperation)
  /\ WF_vars(SettleHostCommand)
  /\ WF_vars(SettlePendingResult)
  /\ WF_vars(SettleRetainedHost)

TypeOK ==
  /\ workerPhase \in WorkerPhases
  /\ serverLink \in LinkPhases
  /\ hostLink \in LinkPhases
  /\ stopCause \in StopCauses
  /\ helloSent \in BOOLEAN
  /\ readinessEmitted \in BOOLEAN
  /\ hostEpoch \in HostEpochs
  /\ dispatchKind \in DispatchKinds
  /\ operationPhase \in OperationPhases
  /\ kernelSlotPhase \in KernelSlotPhases
  /\ handoffAttemptCount \in 0..1
  /\ handoffCount \in 0..1
  /\ startTargetHostEpoch \in HostEpochs
  /\ handoffHostEpoch \in HostEpochs
  /\ hostCommandKind \in HostCommandKinds
  /\ hostCommandCount \in 0..1
  /\ hostCommandApplied \in BOOLEAN
  /\ hostCorrelation \in Correlations
  /\ hostResponseRole \in HostResponseRoles
  /\ hostResponseKind \in ObservedHostKinds
  /\ hostResponseCorrelation \in Correlations
  /\ hostResponseEpoch \in HostEpochs
  /\ rejectedHostResponses \subseteq RejectedHostResponseClasses
  /\ resultOutcome \in ResultOutcomes
  /\ completionSource \in CompletionSources
  /\ bindingResultCached \in BOOLEAN
  /\ deliveryPhase \in DeliveryPhases
  /\ resultDeliveryCount \in 0..1
  /\ retryAfterUnknownBlocked \in BOOLEAN
  /\ retainedHostTerminal \in BOOLEAN
  /\ retentionDisposition \in RetentionDispositions
  /\ lastAction \in Actions

HostContactRequiresServerAdmission ==
  hostLink # "closed" => serverLink = "active"

ReadinessRequiresBothAdmissions ==
  workerPhase \in {"ready", "draining-result"} =>
    /\ serverLink = "active"
    /\ readinessEmitted
    /\ helloSent
    /\ hostEpoch # 0

ReadinessWasNeverForged ==
  readinessEmitted =>
    /\ helloSent
    /\ workerPhase \in {"ready", "draining-result", "stopped"}

ReadyWorkerOwnsBothLinks ==
  workerPhase = "ready" =>
    /\ serverLink = "active"
    /\ hostLink = "active"

StoppedWorkerOwnsNoLink ==
  workerPhase = "stopped" =>
    /\ serverLink = "closed"
    /\ hostLink = "closed"

DrainOwnsOnlyPendingResult ==
  workerPhase = "draining-result" =>
    /\ serverLink = "active"
    /\ hostLink = "closed"
    /\ operationPhase = "result-ready"
    /\ deliveryPhase = "pending"

OneAdmittedDispatch ==
  /\ (dispatchKind = "none") <=> (operationPhase = "none")
  /\ dispatchKind # "none" => readinessEmitted
  /\ dispatchKind = "launch" => startTargetHostEpoch = 0
  /\ dispatchKind = "start" => startTargetHostEpoch # 0

KernelGlobalSlotRefinesOperation ==
  /\ operationPhase = "none" => kernelSlotPhase = "empty"
  /\ operationPhase = "admitted" =>
       kernelSlotPhase = "awaiting-handoff"
  /\ operationPhase \in {"handed-off", "awaiting-host"} =>
       kernelSlotPhase = "handed-off"
  /\ operationPhase = "result-ready" =>
       kernelSlotPhase = "result-ready"
  /\ operationPhase \in {"completed", "abandoned"} =>
       kernelSlotPhase = "empty"

CapacityGlobalSlotRefinement ==
  /\ CapacitySlotProjection \in CapacitySlotPhases
  /\ (CapacitySlotProjection = "empty") <=>
       operationPhase \in {"none", "completed", "abandoned"}
  /\ (CapacitySlotProjection = "operation-pending") <=>
       operationPhase \in {"admitted", "handed-off", "awaiting-host"}
  /\ (CapacitySlotProjection = "result-ready") <=>
       operationPhase = "result-ready"

HandoffObservationIsRetained ==
  /\ handoffAttemptCount = 0 =>
       /\ handoffCount = 0
       /\ handoffHostEpoch = 0
  /\ handoffCount = 1 =>
       /\ handoffAttemptCount = 1
       /\ handoffHostEpoch # 0
  /\ handoffAttemptCount = 1 /\ handoffCount = 0 =>
       /\ completionSource \in HandoffRejectionSources
       /\ bindingResultCached

HostResponseObservationIsComplete ==
  /\ (hostResponseRole = "none") <=> (hostResponseKind = "none")
  /\ (hostResponseRole = "none") <=> (hostResponseCorrelation = "none")
  /\ (hostResponseRole = "none") <=> (hostResponseEpoch = 0)

HostCommandRefinesDispatch ==
  /\ hostCommandCount = 0 => hostCommandKind = "none"
  /\ hostCommandCount = 1 =>
       /\ handoffCount = 1
       /\ hostCorrelation = "operation"
       /\ hostCommandKind = ExpectedHostCommand(dispatchKind)

HostApplicationRequiresCommand ==
  hostCommandApplied => hostCommandCount = 1

ResultLifecycleIsClosed ==
  /\ operationPhase \in {"none", "admitted", "handed-off", "awaiting-host"} =>
       /\ resultOutcome = "none"
       /\ completionSource = "none"
       /\ ~bindingResultCached
       /\ deliveryPhase = "none"
  /\ operationPhase = "result-ready" =>
       /\ resultOutcome # "none"
       /\ completionSource # "none"
       /\ bindingResultCached
       /\ deliveryPhase = "pending"
  /\ operationPhase = "completed" =>
       /\ resultOutcome # "none"
       /\ completionSource # "none"
       /\ bindingResultCached
       /\ deliveryPhase = "delivered"
       /\ resultDeliveryCount = 1
  /\ operationPhase = "abandoned" =>
       /\ resultOutcome # "none"
       /\ completionSource # "none"
       /\ deliveryPhase = "abandoned"

HandoffResultReadyIsExact ==
  /\ completionSource = "handoff-lease-expired" =>
       /\ resultOutcome = "rejected"
       /\ handoffAttemptCount = 1
       /\ handoffCount = 0
       /\ handoffHostEpoch # 0
       /\ hostCommandCount = 0
       /\ ~hostCommandApplied
       /\ bindingResultCached
  /\ completionSource = "host-unavailable-before-handoff" =>
       /\ resultOutcome = "failed"
       /\ handoffAttemptCount = 0
       /\ handoffCount = 0
       /\ hostCommandCount = 0
       /\ ~hostCommandApplied
       /\ bindingResultCached

RejectedResultIsKernelClassified ==
  resultOutcome = "rejected" =>
    completionSource \in HandoffRejectionSources

HostCompletionIsStrictlyCorrelated ==
  completionSource \in {"host-accepted", "host-reported-failure"} =>
    /\ hostResponseRole = "response"
    /\ hostResponseCorrelation = hostCorrelation
    /\ hostResponseCorrelation = "operation"
    /\ hostResponseEpoch = handoffHostEpoch
    /\ handoffCount = 1
    /\ (completionSource = "host-accepted" =>
         /\ resultOutcome = "accepted"
         /\ hostResponseKind = ExpectedSuccessResponse(dispatchKind))
    /\ (completionSource = "host-reported-failure" =>
         /\ resultOutcome = "failed"
         /\ hostResponseKind = "typed-failure")

ExactGenericTypedFailureIsHostReportedFailure ==
  /\ hostResponseKind = "typed-failure"
  /\ KnownHostResponseMatches(
       hostResponseRole,
       hostResponseKind,
       hostResponseCorrelation,
       hostResponseEpoch
     )
  =>
    /\ completionSource = "host-reported-failure"
    /\ resultOutcome = "failed"

InvalidGenericTypedFailureIsUnknown ==
  /\ hostResponseKind = "typed-failure"
  /\ ~KnownHostResponseMatches(
       hostResponseRole,
       hostResponseKind,
       hostResponseCorrelation,
       hostResponseEpoch
     )
  =>
    /\ completionSource = "host-protocol"
    /\ resultOutcome = "unknown"

GenericTypedFailureIsNeverRejected ==
  hostResponseKind = "typed-failure" => resultOutcome # "rejected"

AcceptedHostCompletionRequiresApplication ==
  resultOutcome = "accepted" => hostCommandApplied

ObservableUnknownBeginsAfterHandoff ==
  resultOutcome = "unknown" /\ deliveryPhase # "abandoned" =>
    handoffCount = 1

UnknownOutcomeBlocksRetry ==
  resultOutcome = "unknown" => retryAfterUnknownBlocked

RetainedHostHasOneDisposition ==
  /\ ~retainedHostTerminal => retentionDisposition = "absent"
  /\ retainedHostTerminal /\ workerPhase # "stopped" =>
       retentionDisposition = "current-worker"
  /\ retainedHostTerminal /\ workerPhase = "stopped" =>
       retentionDisposition \in {"unmatched", "recovery-required"}
  /\ retentionDisposition = "recovery-required" =>
       /\ retainedHostTerminal
       /\ workerPhase = "stopped"

MismatchedHostResponseCannotProduceKnownOutcome ==
  [][( \/ RejectWrongHostResponseRole
       \/ RejectWrongHostResponseKind
       \/ RejectWrongHostCorrelation
       \/ RejectWrongHostEpoch ) =>
      /\ resultOutcome' = "unknown"
      /\ completionSource' = "host-protocol"
      /\ ~KnownHostResponseMatches(
           hostResponseRole',
           hostResponseKind',
           hostResponseCorrelation',
           hostResponseEpoch'
         )
      /\ bindingResultCached'
      /\ UNCHANGED <<hostCommandCount, hostCommandApplied,
           retentionState>>]_vars

PreserveExactHandoffResult ==
  completionSource \in HandoffRejectionSources =>
    /\ UNCHANGED HandoffResultProjection
    /\ bindingResultCached' = TRUE

ExactHandoffResultSurvivesDelivery ==
  [][PreserveExactHandoffResult]_vars

BeginHandoffPrecedesEveryHostEffect ==
  [][(DispatchHostCommand \/ HostAppliesCommand) => handoffCount = 1]_vars

UnknownOutcomeNeverRetriesHost ==
  [][resultOutcome = "unknown" =>
      hostCommandCount' = hostCommandCount]_vars

ResultDeliveryIsAtMostOnce ==
  [][resultDeliveryCount = 1 => resultDeliveryCount' = 1]_vars

RetainedHostIsNeverSilentlyAdopted ==
  [][(retentionDisposition \in {"unmatched", "recovery-required"}) =>
      (retentionDisposition' \in {"unmatched", "recovery-required"})]_vars

ActiveHostConnectionKeepsEpoch ==
  [][hostLink = "active" => UNCHANGED hostEpoch]_vars

HostEpochInstallationRequiresHandshake ==
  [][hostEpoch' # hostEpoch => AcceptHostHandshake]_vars

UnmatchedRetainedHostEventuallyRequiresRecovery ==
  [](retentionDisposition = "unmatched" =>
    <>(retentionDisposition = "recovery-required"))

PendingWorkerEventuallySettles ==
  [](workerPhase \in {
       "connecting-server", "connecting-host", "host-handshake",
       "host-admitted"
     } => <>(workerPhase \in {"ready", "stopped"}))

PendingOperationEventuallySettles ==
  [](operationPhase \in {"admitted", "handed-off", "awaiting-host"} =>
    <>(operationPhase \in {"result-ready", "completed", "abandoned"}))

PendingResultDeliveryEventuallySettles ==
  [](deliveryPhase = "pending" =>
    <>(deliveryPhase \in {"delivered", "abandoned"}))

DrainingWorkerEventuallyStops ==
  [](workerPhase = "draining-result" => <>(workerPhase = "stopped"))

=============================================================================
