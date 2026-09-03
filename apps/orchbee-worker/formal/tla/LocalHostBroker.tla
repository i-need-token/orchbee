-------------------------- MODULE LocalHostBroker --------------------------
EXTENDS FiniteSets, Naturals, TLC

\* Models one admitted OBCN Launch or Start dispatch composed with the local
\* Worker-to-Terminal-Host broker. Server admission still precedes Host
\* contact. The admitted dispatch is an input from the separately checked
\* TerminalLaunchAuthorization model; this module owns only the adapter order:
\* begin_host_handoff, one exactly correlated Terminal Protocol command, one
\* terminal result, and bounded Server delivery. A Server loss closes Host
\* immediately. A Host loss may retain Server only for one bounded result
\* delivery, then closes it. A retained Host observed after Worker loss is
\* classified RecoveryRequired and is never silently adopted.

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
HostResponseKinds == {
  "none", "create-terminal", "terminal-start-accepted", "typed-failure"
}
ResultOutcomes == {"none", "accepted", "rejected", "failed", "unknown"}
CompletionSources == {
  "none", "pre-handoff", "host-response", "host-protocol", "host-loss",
  "host-timeout", "server-loss"
}
DeliveryPhases == {"none", "pending", "delivered", "abandoned"}
Correlations == {"none", "operation"}
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
  "fail-before-host-handoff", "dispatch-host-command",
  "host-applies-command", "reject-wrong-host-response-role",
  "reject-wrong-host-response-kind", "reject-wrong-host-correlation",
  "reject-wrong-host-epoch", "host-accepts-command",
  "host-rejects-command", "host-fails-command", "host-command-timeout",
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
  dispatchKind,
  operationPhase,
  handoffCount,
  hostCommandKind,
  hostCommandCount,
  hostCommandApplied,
  hostCorrelation,
  hostResponseKind,
  hostResponseCorrelation,
  hostResponseEpoch,
  rejectedHostResponses,
  resultOutcome,
  completionSource,
  deliveryPhase,
  resultDeliveryCount,
  retryAfterUnknownBlocked,
  retainedHostTerminal,
  retentionDisposition,
  lastAction

brokerState == <<
  workerPhase, serverLink, hostLink, stopCause, helloSent, readinessEmitted
>>
operationState == <<
  dispatchKind, operationPhase, handoffCount, hostCommandKind,
  hostCommandCount, hostCommandApplied, hostCorrelation, hostResponseKind,
  hostResponseCorrelation, hostResponseEpoch, rejectedHostResponses,
  resultOutcome, completionSource, deliveryPhase, resultDeliveryCount,
  retryAfterUnknownBlocked
>>
retentionState == <<retainedHostTerminal, retentionDisposition>>
vars == <<brokerState, operationState, retentionState, lastAction>>

ExpectedHostCommand(kind) ==
  CASE kind = "launch" -> "create-terminal"
    [] kind = "start" -> "start-terminal"
    [] OTHER -> "none"

ExpectedSuccessResponse(kind) ==
  CASE kind = "launch" -> "create-terminal"
    [] kind = "start" -> "terminal-start-accepted"
    [] OTHER -> "none"

Init ==
  /\ workerPhase = "absent"
  /\ serverLink = "closed"
  /\ hostLink = "closed"
  /\ stopCause = "none"
  /\ helloSent = FALSE
  /\ readinessEmitted = FALSE
  /\ dispatchKind = "none"
  /\ operationPhase = "none"
  /\ handoffCount = 0
  /\ hostCommandKind = "none"
  /\ hostCommandCount = 0
  /\ hostCommandApplied = FALSE
  /\ hostCorrelation = "none"
  /\ hostResponseKind = "none"
  /\ hostResponseCorrelation = "none"
  /\ hostResponseEpoch = 0
  /\ rejectedHostResponses = {}
  /\ resultOutcome = "none"
  /\ completionSource = "none"
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
       operationState, retentionState>>

AcceptServer ==
  /\ workerPhase = "connecting-server"
  /\ workerPhase' = "connecting-host"
  /\ serverLink' = "active"
  /\ lastAction' = "accept-server"
  /\ UNCHANGED <<hostLink, stopCause, helloSent, readinessEmitted,
       operationState, retentionState>>

RejectServer ==
  /\ workerPhase = "connecting-server"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ stopCause' = "server-rejected"
  /\ lastAction' = "reject-server"
  /\ UNCHANGED <<hostLink, helloSent, readinessEmitted, operationState,
       retentionState>>

ConnectHost ==
  /\ workerPhase = "connecting-host"
  /\ workerPhase' = "host-handshake"
  /\ hostLink' = "connecting"
  /\ helloSent' = TRUE
  /\ lastAction' = "connect-host"
  /\ UNCHANGED <<serverLink, stopCause, readinessEmitted, operationState,
       retentionState>>

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
  /\ UNCHANGED <<helloSent, readinessEmitted, operationState, retentionState>>

RejectHostPeer ==
  /\ workerPhase = "connecting-host"
  /\ StopForHostAdmissionFailure("host-peer-rejected", "reject-host-peer")

AcceptHostHandshake ==
  /\ workerPhase = "host-handshake"
  /\ workerPhase' = "host-admitted"
  /\ hostLink' = "active"
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
  /\ UNCHANGED <<serverLink, hostLink, stopCause, helloSent, operationState,
       retentionState>>

CanAdmitDispatch ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ dispatchKind = "none"
  /\ operationPhase = "none"
  /\ deliveryPhase = "none"

AdmitLaunchDispatch ==
  /\ CanAdmitDispatch
  /\ dispatchKind' = "launch"
  /\ operationPhase' = "admitted"
  /\ lastAction' = "admit-launch-dispatch"
  /\ UNCHANGED <<brokerState, handoffCount, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource, deliveryPhase,
       resultDeliveryCount, retryAfterUnknownBlocked, retentionState>>

AdmitStartDispatch ==
  /\ CanAdmitDispatch
  \* The exact authorized Start target is an already retained deferred terminal.
  /\ dispatchKind' = "start"
  /\ operationPhase' = "admitted"
  /\ retainedHostTerminal' = TRUE
  /\ retentionDisposition' = "current-worker"
  /\ lastAction' = "admit-start-dispatch"
  /\ UNCHANGED <<brokerState, handoffCount, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource, deliveryPhase,
       resultDeliveryCount, retryAfterUnknownBlocked>>

BeginHostHandoff ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "admitted"
  /\ operationPhase' = "handed-off"
  /\ handoffCount' = 1
  /\ lastAction' = "begin-host-handoff"
  /\ UNCHANGED <<brokerState, dispatchKind, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource, deliveryPhase,
       resultDeliveryCount, retryAfterUnknownBlocked, retentionState>>

QueueResult(outcome, source) ==
  /\ outcome \in ResultOutcomes \ {"none"}
  /\ source \in CompletionSources \ {"none"}
  /\ resultOutcome' = outcome
  /\ completionSource' = source
  /\ deliveryPhase' = "pending"
  /\ operationPhase' = "result-ready"
  /\ retryAfterUnknownBlocked' = (outcome = "unknown")

FailBeforeHostHandoff ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ operationPhase = "admitted"
  /\ QueueResult("failed", "pre-handoff")
  /\ lastAction' = "fail-before-host-handoff"
  /\ UNCHANGED <<brokerState, dispatchKind, handoffCount, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultDeliveryCount, retentionState>>

DispatchHostCommand ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "handed-off"
  /\ handoffCount = 1
  /\ operationPhase' = "awaiting-host"
  /\ hostCommandKind' = ExpectedHostCommand(dispatchKind)
  /\ hostCommandCount' = 1
  /\ hostCorrelation' = "operation"
  /\ lastAction' = "dispatch-host-command"
  /\ UNCHANGED <<brokerState, dispatchKind, handoffCount,
       hostCommandApplied, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultOutcome,
       completionSource, deliveryPhase, resultDeliveryCount,
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
  /\ UNCHANGED <<brokerState, dispatchKind, operationPhase, handoffCount,
       hostCommandKind, hostCommandCount, hostCorrelation, hostResponseKind,
       hostResponseCorrelation, hostResponseEpoch, rejectedHostResponses,
       resultOutcome, completionSource, deliveryPhase, resultDeliveryCount,
       retryAfterUnknownBlocked>>

RejectHostResponse(class, action) ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "awaiting-host"
  /\ class \in RejectedHostResponseClasses
  /\ class \notin rejectedHostResponses
  /\ workerPhase' = "draining-result"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-malformed"
  /\ rejectedHostResponses' = rejectedHostResponses \cup {class}
  /\ QueueResult("unknown", "host-protocol")
  /\ lastAction' = action
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, dispatchKind, handoffCount,
       hostCommandKind, hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       resultDeliveryCount, retentionState>>

RejectWrongHostResponseRole ==
  /\ "wrong-role" \notin rejectedHostResponses
  /\ RejectHostResponse("wrong-role", "reject-wrong-host-response-role")

RejectWrongHostResponseKind ==
  /\ "wrong-kind" \notin rejectedHostResponses
  /\ RejectHostResponse("wrong-kind", "reject-wrong-host-response-kind")

RejectWrongHostCorrelation ==
  /\ "wrong-correlation" \notin rejectedHostResponses
  /\ RejectHostResponse("wrong-correlation", "reject-wrong-host-correlation")

RejectWrongHostEpoch ==
  /\ "wrong-host-epoch" \notin rejectedHostResponses
  /\ RejectHostResponse("wrong-host-epoch", "reject-wrong-host-epoch")

CompleteFromHost(outcome, responseKind, action) ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "awaiting-host"
  /\ hostCommandCount = 1
  /\ hostCorrelation = "operation"
  /\ outcome \in {"accepted", "rejected", "failed"}
  /\ responseKind \in HostResponseKinds \ {"none"}
  /\ QueueResult(outcome, "host-response")
  /\ hostResponseKind' = responseKind
  /\ hostResponseCorrelation' = "operation"
  /\ hostResponseEpoch' = 1
  /\ lastAction' = action
  /\ UNCHANGED <<brokerState, dispatchKind, handoffCount, hostCommandKind,
       hostCommandCount, hostCommandApplied, hostCorrelation,
       rejectedHostResponses, resultDeliveryCount, retentionState>>

HostAcceptsCommand ==
  /\ hostCommandApplied
  /\ CompleteFromHost(
       "accepted",
       ExpectedSuccessResponse(dispatchKind),
       "host-accepts-command"
     )

HostRejectsCommand ==
  /\ ~hostCommandApplied
  /\ CompleteFromHost("rejected", "typed-failure", "host-rejects-command")

HostFailsCommand ==
  /\ ~hostCommandApplied
  /\ CompleteFromHost("failed", "typed-failure", "host-fails-command")

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
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, dispatchKind,
       handoffCount, hostCommandKind, hostCommandCount, hostCommandApplied,
       hostCorrelation, hostResponseKind, hostResponseCorrelation,
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
  /\ UNCHANGED <<brokerState, dispatchKind, operationPhase, handoffCount,
       hostCommandKind, hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource, deliveryPhase,
       resultDeliveryCount, retryAfterUnknownBlocked, retentionState>>

DeliverResult ==
  /\ serverLink = "active"
  /\ operationPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ operationPhase' = "completed"
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
       dispatchKind, handoffCount, hostCommandKind, hostCommandCount,
       hostCommandApplied, hostCorrelation, hostResponseKind,
       hostResponseCorrelation, hostResponseEpoch, rejectedHostResponses,
       resultOutcome, completionSource, retryAfterUnknownBlocked,
       retainedHostTerminal>>

ResultDeliveryTimeout ==
  /\ serverLink = "active"
  /\ operationPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "result-delivery-timeout"
  /\ operationPhase' = "abandoned"
  /\ deliveryPhase' = "abandoned"
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition
  /\ lastAction' = "result-delivery-timeout"
  /\ UNCHANGED <<helloSent, readinessEmitted, dispatchKind, handoffCount,
       hostCommandKind, hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource,
       resultDeliveryCount, retryAfterUnknownBlocked, retainedHostTerminal>>

LoseServerBeforeReady ==
  /\ serverLink = "active"
  /\ workerPhase \in {"connecting-host", "host-handshake", "host-admitted"}
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ lastAction' = "lose-server-before-ready"
  /\ UNCHANGED <<helloSent, readinessEmitted, operationState, retentionState>>

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
  /\ UNCHANGED <<helloSent, readinessEmitted, operationState,
       retainedHostTerminal>>

AbandonForServerLoss ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ operationPhase' = "abandoned"
  /\ resultOutcome' = "unknown"
  /\ completionSource' = "server-loss"
  /\ deliveryPhase' = "abandoned"
  /\ retryAfterUnknownBlocked' = TRUE
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition

LoseServerBeforeHandoff ==
  /\ operationPhase = "admitted"
  /\ AbandonForServerLoss
  /\ lastAction' = "lose-server-before-handoff"
  /\ UNCHANGED <<helloSent, readinessEmitted, dispatchKind, handoffCount,
       hostCommandKind, hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultDeliveryCount, retainedHostTerminal>>

LoseServerAfterHandoff ==
  /\ operationPhase \in {"handed-off", "awaiting-host"}
  /\ AbandonForServerLoss
  /\ lastAction' = "lose-server-after-handoff"
  /\ UNCHANGED <<helloSent, readinessEmitted, dispatchKind, handoffCount,
       hostCommandKind, hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultDeliveryCount, retainedHostTerminal>>

LoseServerWithPendingResult ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ operationPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ operationPhase' = "abandoned"
  /\ deliveryPhase' = "abandoned"
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition
  /\ lastAction' = "lose-server-with-pending-result"
  /\ UNCHANGED <<helloSent, readinessEmitted, dispatchKind, handoffCount,
       hostCommandKind, hostCommandCount, hostCommandApplied, hostCorrelation,
       hostResponseKind, hostResponseCorrelation, hostResponseEpoch,
       rejectedHostResponses, resultOutcome, completionSource,
       resultDeliveryCount, retryAfterUnknownBlocked, retainedHostTerminal>>

LoseServerDuringDrain ==
  /\ workerPhase = "draining-result"
  /\ serverLink = "active"
  /\ hostLink = "closed"
  /\ operationPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ operationPhase' = "abandoned"
  /\ deliveryPhase' = "abandoned"
  /\ retentionDisposition' = IF retainedHostTerminal
                              THEN "unmatched"
                              ELSE retentionDisposition
  /\ lastAction' = "lose-server-during-drain"
  /\ UNCHANGED <<hostLink, helloSent, readinessEmitted, dispatchKind,
       handoffCount, hostCommandKind, hostCommandCount, hostCommandApplied,
       hostCorrelation, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultOutcome,
       completionSource, resultDeliveryCount, retryAfterUnknownBlocked,
       retainedHostTerminal>>

LoseHostBeforeReady ==
  /\ hostLink \in {"connecting", "active"}
  /\ workerPhase \in {"host-handshake", "host-admitted"}
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-lost"
  /\ lastAction' = "lose-host-before-ready"
  /\ UNCHANGED <<helloSent, readinessEmitted, operationState, retentionState>>

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
  /\ UNCHANGED <<helloSent, readinessEmitted, operationState,
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
  /\ DrainForHostLoss(
       "failed",
       "pre-handoff",
       "lose-host-before-handoff"
     )
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, dispatchKind,
       handoffCount, hostCommandKind, hostCommandCount, hostCommandApplied,
       hostCorrelation, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultDeliveryCount,
       retentionState>>

LoseHostAfterHandoff ==
  /\ operationPhase \in {"handed-off", "awaiting-host"}
  /\ DrainForHostLoss("unknown", "host-loss", "lose-host-after-handoff")
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, dispatchKind,
       handoffCount, hostCommandKind, hostCommandCount, hostCommandApplied,
       hostCorrelation, hostResponseKind, hostResponseCorrelation,
       hostResponseEpoch, rejectedHostResponses, resultDeliveryCount,
       retentionState>>

LoseHostWithPendingResult ==
  /\ workerPhase = "ready"
  /\ serverLink = "active"
  /\ hostLink = "active"
  /\ operationPhase = "result-ready"
  /\ deliveryPhase = "pending"
  /\ workerPhase' = "draining-result"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-lost"
  /\ lastAction' = "lose-host-with-pending-result"
  /\ UNCHANGED <<serverLink, helloSent, readinessEmitted, operationState,
       retentionState>>

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
  \/ FailBeforeHostHandoff
  \/ DispatchHostCommand
  \/ HostAppliesCommand
  \/ RejectWrongHostResponseRole
  \/ RejectWrongHostResponseKind
  \/ RejectWrongHostCorrelation
  \/ RejectWrongHostEpoch
  \/ HostAcceptsCommand
  \/ HostRejectsCommand
  \/ HostFailsCommand
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
  BeginHostHandoff \/ FailBeforeHostHandoff \/ LoseServerBeforeHandoff \/
  LoseHostBeforeHandoff

SettleHandedOffOperation ==
  DispatchHostCommand \/ HostCommandTimeout \/ LoseServerAfterHandoff \/
  LoseHostAfterHandoff

SettleHostCommand ==
  HostAppliesCommand \/ HostAcceptsCommand \/ HostRejectsCommand \/
  HostFailsCommand \/ HostCommandTimeout \/ LoseServerAfterHandoff \/
  LoseHostAfterHandoff

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
  /\ dispatchKind \in DispatchKinds
  /\ operationPhase \in OperationPhases
  /\ handoffCount \in 0..1
  /\ hostCommandKind \in HostCommandKinds
  /\ hostCommandCount \in 0..1
  /\ hostCommandApplied \in BOOLEAN
  /\ hostCorrelation \in Correlations
  /\ hostResponseKind \in HostResponseKinds
  /\ hostResponseCorrelation \in Correlations
  /\ hostResponseEpoch \in 0..1
  /\ rejectedHostResponses \subseteq RejectedHostResponseClasses
  /\ resultOutcome \in ResultOutcomes
  /\ completionSource \in CompletionSources
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
       /\ deliveryPhase = "none"
  /\ operationPhase = "result-ready" =>
       /\ resultOutcome # "none"
       /\ completionSource # "none"
       /\ deliveryPhase = "pending"
  /\ operationPhase = "completed" =>
       /\ resultOutcome # "none"
       /\ completionSource # "none"
       /\ deliveryPhase = "delivered"
       /\ resultDeliveryCount = 1
  /\ operationPhase = "abandoned" =>
       /\ resultOutcome # "none"
       /\ completionSource # "none"
       /\ deliveryPhase = "abandoned"

HostCompletionIsStrictlyCorrelated ==
  completionSource = "host-response" =>
    /\ hostResponseCorrelation = "operation"
    /\ hostResponseEpoch = 1
    /\ (resultOutcome = "accepted" =>
         hostResponseKind = ExpectedSuccessResponse(dispatchKind))
    /\ (resultOutcome \in {"rejected", "failed"} =>
         hostResponseKind = "typed-failure")

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
      /\ hostResponseKind' = "none"
      /\ hostResponseCorrelation' = "none"
      /\ hostResponseEpoch' = 0
      /\ UNCHANGED <<hostCommandCount, hostCommandApplied,
           retentionState>>]_vars

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
