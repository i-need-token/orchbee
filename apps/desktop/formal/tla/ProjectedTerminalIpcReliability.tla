---------------- MODULE ProjectedTerminalIpcReliability ----------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

\* Finite cross-boundary abstraction for one projected terminal surface.
\* The three configurations isolate control release/open races, ordered Host
\* response delivery, and attachment recovery with atomic user-operation
\* identities. Transport delay is represented only by action order. Loss and
\* disconnect may stop progress; no fixed latency or reliable-network axiom is
\* part of the safety claims.

CONSTANT Scenario

Scenarios == {"release-race", "attach-order", "recovery-operations"}
Attempts == 1..2
NoAttempt == 0
Operations == {"selection", "find"}
OperationStates == {
  "not-issued", "admitted", "searching", "handed-off",
  "host-completed", "completed", "cancelled", "unknown"
}
HostMessages == {"attach-response", "first-frame", "close"}
Actions == {
  "init",
  "begin-open", "client-release", "deliver-release-command",
  "service-attach", "publish-open", "deliver-expected-open",
  "discard-late-open", "begin-detach", "detach-fails", "deliver-cleanup-failure",
  "retire-control", "retirement-cleanup-succeeds", "detach-succeeds",
  "deliver-release-ack", "service-open-fails-after-release",
  "end-request-transport", "settle-connection-cleanup",
  "observe-duplicate-connection-cleanup",
  "queue-attach-response", "queue-first-frame", "queue-host-close",
  "deliver-attach-response", "install-projection-continuation",
  "resume-inbound", "deliver-first-frame", "initialize-renderer",
  "acknowledge-first-frame", "deliver-host-close",
  "start-surface", "resolve-projection", "candidate-paints-frame",
  "promote-candidate", "issue-selection", "forward-selection",
  "host-completes-selection", "deliver-selection",
  "open-find-with-selection", "open-find-without-selection",
  "forward-search-read", "complete-search", "transport-unavailable",
  "fire-retry-timer", "candidate-unavailable",
  "framed-candidate-unavailable", "fail-open-attempt",
  "exhaust-recovery-budget"
}

VARIABLE s

vars == <<s>>

Init ==
  s = [
    clientRequests |-> {},
    allocatedRequests |-> {},
    serverRequests |-> {},
    clientReleased |-> {},
    serverReleaseRequested |-> {},
    leases |-> {},
    openEvents |-> {},
    releaseEvents |-> {},
    cleanupFailureEvents |-> {},
    cleanupFailures |-> {},
    cleanupEvidence |-> {},
    activated |-> {},
    latePortsClosed |-> {},
    clientAcknowledged |-> {},
    clientCleanupFailed |-> {},
    connectionCleanupDelivered |-> FALSE,
    connectionSettled |-> {},
    controlClosed |-> FALSE,
    detachInFlight |-> {},
    detachRequests |-> {},
    detachRequestsAtEnd |-> {},
    hostQueue |-> <<>>,
    hostQueued |-> {},
    responseResolved |-> FALSE,
    inboundBarrier |-> FALSE,
    listenerInstalled |-> FALSE,
    frameDelivered |-> FALSE,
    rendererReady |-> FALSE,
    frameAcknowledged |-> FALSE,
    hostCloseDelivered |-> FALSE,
    nextAttempt |-> 1,
    pending |-> NoAttempt,
    candidate |-> NoAttempt,
    frozen |-> NoAttempt,
    active |-> NoAttempt,
    visible |-> NoAttempt,
    framed |-> {},
    sealed |-> {},
    releasedAttempts |-> {},
    retryScheduled |-> FALSE,
    recoveryFailed |-> FALSE,
    requestTransportEnded |-> FALSE,
    operationState |-> [operation \in Operations |-> "not-issued"],
    operationAttempt |-> [operation \in Operations |-> NoAttempt],
    operationAttempts |-> [operation \in Operations |-> {}],
    selectionObservedAttempt |-> NoAttempt,
    selectionDeliveredAttempts |-> {},
    findSeedAttempt |-> NoAttempt,
    lastAction |-> "init",
    lastAttempt |-> NoAttempt,
    previousClientRequests |-> {}
  ]

Record(next, action, attempt) ==
  [next EXCEPT
    !.lastAction = action,
    !.lastAttempt = attempt,
    !.previousClientRequests = s.clientRequests]

BeginOpen(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ ~s.requestTransportEnded
  /\ attempt \in Attempts \ s.allocatedRequests
  /\ s' = Record(
       [s EXCEPT
         !.clientRequests = @ \cup {attempt},
         !.allocatedRequests = @ \cup {attempt},
         !.serverRequests = @ \cup {attempt}],
       "begin-open",
       attempt)

ClientRelease(attempt) ==
  /\ Scenario = "release-race"
  /\ attempt \in s.clientRequests \ s.clientReleased
  /\ s' = Record(
       [s EXCEPT
         !.clientReleased = @ \cup {attempt},
         !.activated = @ \ {attempt}],
       "client-release",
       attempt)

DeliverReleaseCommand(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ attempt \in (s.clientReleased \cap s.serverRequests) \ s.serverReleaseRequested
  /\ s' = Record(
       [s EXCEPT !.serverReleaseRequested = @ \cup {attempt}],
       "deliver-release-command",
       attempt)

ServiceAttach(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.requestTransportEnded
  /\ attempt \in s.serverRequests \ s.leases
  /\ s' = Record(
       [s EXCEPT !.leases = @ \cup {attempt}],
       "service-attach",
       attempt)

PublishOpen(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ ~s.requestTransportEnded
  /\ attempt \in s.leases \cap s.serverRequests
  /\ attempt \notin s.serverReleaseRequested \cup s.openEvents \cup s.activated
  /\ s' = Record(
       [s EXCEPT !.openEvents = @ \cup {attempt}],
       "publish-open",
       attempt)

DeliverExpectedOpen(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ attempt \in s.openEvents \ s.clientReleased
  /\ s' = Record(
       [s EXCEPT
         !.openEvents = @ \ {attempt},
         !.activated = @ \cup {attempt}],
       "deliver-expected-open",
       attempt)

DiscardLateOpen(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ attempt \in s.openEvents \cap s.clientReleased \cap s.clientRequests
  /\ s' = Record(
       [s EXCEPT
         !.openEvents = @ \ {attempt},
         !.latePortsClosed = @ \cup {attempt}],
       "discard-late-open",
       attempt)

BeginDetach(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.requestTransportEnded
  /\ attempt \in s.leases \cap s.serverRequests \cap s.serverReleaseRequested
  /\ attempt \notin s.detachInFlight
  /\ s' = Record(
       [s EXCEPT
         !.detachInFlight = @ \cup {attempt},
         !.detachRequests = @ \cup {attempt}],
       "begin-detach",
       attempt)

DetachFails(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.requestTransportEnded
  /\ attempt \in s.detachInFlight
  /\ s' = Record(
       [s EXCEPT
         !.detachInFlight = @ \ {attempt},
         !.cleanupFailures = @ \cup {attempt},
         !.cleanupFailureEvents = @ \cup {attempt}],
       "detach-fails",
       attempt)

DeliverCleanupFailure(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ attempt \in s.cleanupFailureEvents \cap s.clientRequests
  /\ attempt \notin s.openEvents
  /\ s' = Record(
       [s EXCEPT
         !.cleanupFailureEvents = @ \ {attempt},
         !.clientRequests = @ \ {attempt},
         !.clientCleanupFailed = @ \cup {attempt}],
       "deliver-cleanup-failure",
       attempt)

RetireControl ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ s.serverRequests # {}
  /\ s' = Record(
       [s EXCEPT
         !.controlClosed = TRUE,
         !.clientRequests = {},
         !.activated = {},
         !.serverReleaseRequested = @ \cup s.serverRequests],
       "retire-control",
       NoAttempt)

RetirementCleanupSucceeds(attempt) ==
  /\ Scenario = "release-race"
  /\ s.controlClosed
  /\ ~s.requestTransportEnded
  /\ attempt \in s.cleanupFailures \cap s.detachInFlight
  /\ s' = Record(
       [s EXCEPT
         !.leases = @ \ {attempt},
         !.serverRequests = @ \ {attempt},
         !.detachInFlight = @ \ {attempt},
         !.cleanupFailures = @ \ {attempt},
         !.cleanupFailureEvents = @ \ {attempt},
         !.cleanupEvidence = @ \cup {attempt}],
       "retirement-cleanup-succeeds",
       attempt)

DetachSucceeds(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ ~s.requestTransportEnded
  /\ attempt \in s.detachInFlight
  /\ s' = Record(
       [s EXCEPT
         !.leases = @ \ {attempt},
         !.serverRequests = @ \ {attempt},
         !.detachInFlight = @ \ {attempt},
         !.cleanupFailures = @ \ {attempt},
         !.cleanupFailureEvents = @ \ {attempt},
         !.cleanupEvidence = @ \cup {attempt},
         !.releaseEvents = @ \cup {attempt}],
       "detach-succeeds",
       attempt)

DeliverReleaseAck(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ attempt \in s.releaseEvents \cap s.clientRequests
  /\ attempt \notin s.openEvents
  /\ s' = Record(
       [s EXCEPT
         !.releaseEvents = @ \ {attempt},
         !.clientRequests = @ \ {attempt},
         !.clientAcknowledged = @ \cup {attempt}],
       "deliver-release-ack",
       attempt)

ServiceOpenFailsAfterRelease(attempt) ==
  /\ Scenario = "release-race"
  /\ ~s.controlClosed
  /\ ~s.requestTransportEnded
  /\ attempt \in (s.serverRequests \cap s.serverReleaseRequested) \ s.leases
  /\ s' = Record(
       [s EXCEPT
         !.serverRequests = @ \ {attempt},
         !.cleanupEvidence = @ \cup {attempt},
         !.releaseEvents = @ \cup {attempt}],
       "service-open-fails-after-release",
       attempt)

EndRequestTransport ==
  /\ Scenario = "release-race"
  /\ ~s.requestTransportEnded
  /\ s' = Record(
       [s EXCEPT
         !.requestTransportEnded = TRUE,
         !.detachRequestsAtEnd = s.detachRequests],
       "end-request-transport",
       NoAttempt)

SettleConnectionCleanup ==
  /\ Scenario = "release-race"
  /\ s.requestTransportEnded
  /\ ~s.connectionCleanupDelivered
  /\ LET owned ==
       s.serverRequests \cup s.leases \cup s.cleanupFailures \cup s.detachInFlight
     IN s' = Record(
       [s EXCEPT
         !.leases = {},
         !.serverRequests = {},
         !.openEvents = {},
         !.cleanupFailureEvents = {},
         !.cleanupFailures = {},
         !.cleanupEvidence = @ \cup owned,
         !.activated = {},
         !.detachInFlight = {},
         !.connectionCleanupDelivered = TRUE,
         !.connectionSettled = @ \cup owned,
         !.releaseEvents = @ \cup
           (owned \cap s.serverReleaseRequested \cap s.clientRequests)],
       "settle-connection-cleanup",
       NoAttempt)

ObserveDuplicateConnectionCleanup ==
  /\ Scenario = "release-race"
  /\ s.connectionCleanupDelivered
  /\ s' = Record(s, "observe-duplicate-connection-cleanup", NoAttempt)

QueueAttachResponse ==
  /\ Scenario = "attach-order"
  /\ "attach-response" \notin s.hostQueued
  /\ s' = Record(
       [s EXCEPT
         !.hostQueue = Append(@, "attach-response"),
         !.hostQueued = @ \cup {"attach-response"}],
       "queue-attach-response",
       NoAttempt)

QueueFirstFrame ==
  /\ Scenario = "attach-order"
  /\ "attach-response" \in s.hostQueued
  /\ "first-frame" \notin s.hostQueued
  /\ "close" \notin s.hostQueued
  /\ s' = Record(
       [s EXCEPT
         !.hostQueue = Append(@, "first-frame"),
         !.hostQueued = @ \cup {"first-frame"}],
       "queue-first-frame",
       NoAttempt)

QueueHostClose ==
  /\ Scenario = "attach-order"
  /\ "attach-response" \in s.hostQueued
  /\ "close" \notin s.hostQueued
  /\ s' = Record(
       [s EXCEPT
         !.hostQueue = Append(@, "close"),
         !.hostQueued = @ \cup {"close"}],
       "queue-host-close",
       NoAttempt)

DeliverAttachResponse ==
  /\ Scenario = "attach-order"
  /\ ~s.inboundBarrier
  /\ s.hostQueue # <<>>
  /\ Head(s.hostQueue) = "attach-response"
  /\ s' = Record(
       [s EXCEPT
         !.hostQueue = Tail(@),
         !.responseResolved = TRUE,
         !.inboundBarrier = TRUE],
       "deliver-attach-response",
       NoAttempt)

InstallProjectionContinuation ==
  /\ Scenario = "attach-order"
  /\ s.responseResolved
  /\ s.inboundBarrier
  /\ ~s.listenerInstalled
  /\ s' = Record(
       [s EXCEPT !.listenerInstalled = TRUE],
       "install-projection-continuation",
       NoAttempt)

ResumeInbound ==
  /\ Scenario = "attach-order"
  /\ s.inboundBarrier
  /\ s.listenerInstalled
  /\ s' = Record(
       [s EXCEPT !.inboundBarrier = FALSE],
       "resume-inbound",
       NoAttempt)

DeliverFirstFrame ==
  /\ Scenario = "attach-order"
  /\ ~s.inboundBarrier
  /\ s.hostQueue # <<>>
  /\ Head(s.hostQueue) = "first-frame"
  /\ s' = Record(
       [s EXCEPT
         !.hostQueue = Tail(@),
         !.frameDelivered = TRUE],
       "deliver-first-frame",
       NoAttempt)

InitializeRenderer ==
  /\ Scenario = "attach-order"
  /\ s.listenerInstalled
  /\ ~s.rendererReady
  /\ s' = Record(
       [s EXCEPT !.rendererReady = TRUE],
       "initialize-renderer",
       NoAttempt)

AcknowledgeFirstFrame ==
  /\ Scenario = "attach-order"
  /\ s.frameDelivered
  /\ s.rendererReady
  /\ ~s.frameAcknowledged
  /\ ~s.hostCloseDelivered
  /\ s' = Record(
       [s EXCEPT !.frameAcknowledged = TRUE],
       "acknowledge-first-frame",
       NoAttempt)

DeliverHostClose ==
  /\ Scenario = "attach-order"
  /\ ~s.inboundBarrier
  /\ s.hostQueue # <<>>
  /\ Head(s.hostQueue) = "close"
  /\ s' = Record(
       [s EXCEPT
         !.hostQueue = Tail(@),
         !.hostCloseDelivered = TRUE],
       "deliver-host-close",
       NoAttempt)

StartSurface ==
  /\ Scenario = "recovery-operations"
  /\ s.nextAttempt = 1
  /\ s.pending = NoAttempt
  /\ LET attempt == s.nextAttempt IN
       s' = Record(
         [s EXCEPT
           !.clientRequests = @ \cup {attempt},
           !.allocatedRequests = @ \cup {attempt},
           !.pending = attempt,
           !.nextAttempt = attempt + 1],
         "start-surface",
         attempt)

ResolveProjection ==
  /\ Scenario = "recovery-operations"
  /\ s.pending \in Attempts
  /\ s.candidate = NoAttempt
  /\ LET attempt == s.pending IN
       s' = Record(
         [s EXCEPT
           !.candidate = attempt,
           !.leases = @ \cup {attempt}],
         "resolve-projection",
         attempt)

CandidatePaintsFrame ==
  /\ Scenario = "recovery-operations"
  /\ s.candidate \in Attempts \ s.framed
  /\ s' = Record(
       [s EXCEPT !.framed = @ \cup {s.candidate}],
       "candidate-paints-frame",
       s.candidate)

PromoteCandidate ==
  /\ Scenario = "recovery-operations"
  /\ s.candidate \in s.framed
  /\ LET attempt == s.candidate
         old == s.frozen
     IN
       s' = Record(
         [s EXCEPT
           !.active = attempt,
           !.visible = attempt,
           !.candidate = NoAttempt,
           !.pending = NoAttempt,
           !.frozen = NoAttempt,
           !.selectionObservedAttempt = NoAttempt,
           !.releasedAttempts =
             IF old = NoAttempt THEN @ ELSE @ \cup {old}],
         "promote-candidate",
         attempt)

IssueSelection ==
  /\ Scenario = "recovery-operations"
  /\ s.active \in Attempts \ s.sealed
  /\ s.operationState["selection"] = "not-issued"
  /\ LET attempt == s.active IN
       s' = Record(
         [s EXCEPT
           !.operationState["selection"] = "admitted",
           !.operationAttempt["selection"] = attempt,
           !.operationAttempts["selection"] = @ \cup {attempt}],
         "issue-selection",
         attempt)

ForwardSelection ==
  /\ Scenario = "recovery-operations"
  /\ s.operationState["selection"] = "admitted"
  /\ s.operationAttempt["selection"] = s.active
  /\ s' = Record(
       [s EXCEPT !.operationState["selection"] = "handed-off"],
       "forward-selection",
       s.active)

HostCompletesSelection ==
  /\ Scenario = "recovery-operations"
  /\ s.operationState["selection"] = "handed-off"
  /\ s.operationAttempt["selection"] = s.active
  /\ s' = Record(
       [s EXCEPT !.operationState["selection"] = "host-completed"],
       "host-completes-selection",
       s.active)

DeliverSelection ==
  /\ Scenario = "recovery-operations"
  /\ s.operationState["selection"] = "host-completed"
  /\ s.operationAttempt["selection"] = s.active
  /\ LET attempt == s.active IN
       s' = Record(
         [s EXCEPT
           !.operationState["selection"] = "completed",
           !.selectionObservedAttempt = attempt,
           !.selectionDeliveredAttempts = @ \cup {attempt}],
         "deliver-selection",
         attempt)

OpenFindWithSelection ==
  /\ Scenario = "recovery-operations"
  /\ s.active \in Attempts \ s.sealed
  /\ s.operationState["find"] = "not-issued"
  /\ s.selectionObservedAttempt = s.active
  /\ LET attempt == s.active IN
       s' = Record(
         [s EXCEPT
           !.operationState["find"] = "searching",
           !.operationAttempt["find"] = attempt,
           !.operationAttempts["find"] = @ \cup {attempt},
           !.findSeedAttempt = attempt],
         "open-find-with-selection",
         attempt)

OpenFindWithoutSelection ==
  /\ Scenario = "recovery-operations"
  /\ s.active \in Attempts \ s.sealed
  /\ s.operationState["find"] = "not-issued"
  /\ s.selectionObservedAttempt # s.active
  /\ LET attempt == s.active IN
       s' = Record(
         [s EXCEPT
           !.operationState["find"] = "searching",
           !.operationAttempt["find"] = attempt,
           !.operationAttempts["find"] = @ \cup {attempt},
           !.findSeedAttempt = NoAttempt],
         "open-find-without-selection",
         attempt)

ForwardSearchRead ==
  /\ Scenario = "recovery-operations"
  /\ s.operationState["find"] = "searching"
  /\ s.operationAttempt["find"] = s.active
  /\ s' = Record(
       [s EXCEPT !.operationState["find"] = "handed-off"],
       "forward-search-read",
       s.active)

CompleteSearch ==
  /\ Scenario = "recovery-operations"
  /\ s.operationState["find"] = "handed-off"
  /\ s.operationAttempt["find"] = s.active
  /\ s' = Record(
       [s EXCEPT !.operationState["find"] = "completed"],
       "complete-search",
       s.active)

RetiredOperationState(operation, attempt) ==
  IF s.operationAttempt[operation] # attempt
  THEN s.operationState[operation]
  ELSE IF s.operationState[operation] \in {"admitted", "searching"}
       THEN "cancelled"
       ELSE IF s.operationState[operation] \in {"handed-off", "host-completed"}
            THEN "unknown"
            ELSE s.operationState[operation]

TransportUnavailable ==
  /\ Scenario = "recovery-operations"
  /\ s.active \in Attempts
  /\ LET attempt == s.active IN
       s' = Record(
         [s EXCEPT
           !.active = NoAttempt,
           !.frozen = attempt,
           !.sealed = @ \cup {attempt},
           !.clientReleased = @ \cup {attempt},
           !.releasedAttempts = @ \cup {attempt},
           !.retryScheduled = s.nextAttempt \in Attempts,
           !.operationState =
             [operation \in Operations |-> RetiredOperationState(operation, attempt)]],
         "transport-unavailable",
         attempt)

FireRetryTimer ==
  /\ Scenario = "recovery-operations"
  /\ s.retryScheduled
  /\ s.nextAttempt \in Attempts
  /\ s.active = NoAttempt
  /\ s.pending = NoAttempt
  /\ s.candidate = NoAttempt
  /\ LET attempt == s.nextAttempt IN
       s' = Record(
         [s EXCEPT
           !.clientRequests = @ \cup {attempt},
           !.allocatedRequests = @ \cup {attempt},
           !.pending = attempt,
           !.nextAttempt = attempt + 1,
           !.retryScheduled = FALSE],
         "fire-retry-timer",
         attempt)

CandidateUnavailable ==
  /\ Scenario = "recovery-operations"
  /\ s.candidate \in Attempts \ s.framed
  /\ LET attempt == s.candidate IN
       s' = Record(
         [s EXCEPT
           !.candidate = NoAttempt,
           !.pending = NoAttempt,
           !.sealed = @ \cup {attempt},
           !.clientReleased = @ \cup {attempt},
           !.releasedAttempts = @ \cup {attempt},
           !.retryScheduled = s.nextAttempt \in Attempts],
         "candidate-unavailable",
         attempt)

FramedCandidateUnavailable ==
  /\ Scenario = "recovery-operations"
  /\ s.candidate \in s.framed
  /\ LET attempt == s.candidate IN
       s' = Record(
         [s EXCEPT
           !.candidate = NoAttempt,
           !.pending = NoAttempt,
           !.sealed = @ \cup {attempt},
           !.clientReleased = @ \cup {attempt},
           !.releasedAttempts = @ \cup {attempt},
           !.retryScheduled = s.nextAttempt \in Attempts],
         "framed-candidate-unavailable",
         attempt)

FailOpenAttempt ==
  /\ Scenario = "recovery-operations"
  /\ s.pending \in Attempts
  /\ s.candidate = NoAttempt
  /\ LET attempt == s.pending IN
       s' = Record(
         [s EXCEPT
           !.pending = NoAttempt,
           !.clientReleased = @ \cup {attempt},
           !.releasedAttempts = @ \cup {attempt},
           !.retryScheduled = s.nextAttempt \in Attempts],
         "fail-open-attempt",
         attempt)

ExhaustRecoveryBudget ==
  /\ Scenario = "recovery-operations"
  /\ s.nextAttempt = 3
  /\ s.active = NoAttempt
  /\ s.pending = NoAttempt
  /\ s.candidate = NoAttempt
  /\ ~s.retryScheduled
  /\ ~s.recoveryFailed
  /\ s' = Record(
       [s EXCEPT !.recoveryFailed = TRUE],
       "exhaust-recovery-budget",
       NoAttempt)

Next ==
  \/ \E attempt \in Attempts: BeginOpen(attempt)
  \/ \E attempt \in Attempts: ClientRelease(attempt)
  \/ \E attempt \in Attempts: DeliverReleaseCommand(attempt)
  \/ \E attempt \in Attempts: ServiceAttach(attempt)
  \/ \E attempt \in Attempts: PublishOpen(attempt)
  \/ \E attempt \in Attempts: DeliverExpectedOpen(attempt)
  \/ \E attempt \in Attempts: DiscardLateOpen(attempt)
  \/ \E attempt \in Attempts: BeginDetach(attempt)
  \/ \E attempt \in Attempts: DetachFails(attempt)
  \/ \E attempt \in Attempts: DeliverCleanupFailure(attempt)
  \/ RetireControl
  \/ \E attempt \in Attempts: RetirementCleanupSucceeds(attempt)
  \/ \E attempt \in Attempts: DetachSucceeds(attempt)
  \/ \E attempt \in Attempts: DeliverReleaseAck(attempt)
  \/ \E attempt \in Attempts: ServiceOpenFailsAfterRelease(attempt)
  \/ EndRequestTransport
  \/ SettleConnectionCleanup
  \/ ObserveDuplicateConnectionCleanup
  \/ QueueAttachResponse
  \/ QueueFirstFrame
  \/ QueueHostClose
  \/ DeliverAttachResponse
  \/ InstallProjectionContinuation
  \/ ResumeInbound
  \/ DeliverFirstFrame
  \/ InitializeRenderer
  \/ AcknowledgeFirstFrame
  \/ DeliverHostClose
  \/ StartSurface
  \/ ResolveProjection
  \/ CandidatePaintsFrame
  \/ PromoteCandidate
  \/ IssueSelection
  \/ ForwardSelection
  \/ HostCompletesSelection
  \/ DeliverSelection
  \/ OpenFindWithSelection
  \/ OpenFindWithoutSelection
  \/ ForwardSearchRead
  \/ CompleteSearch
  \/ TransportUnavailable
  \/ FireRetryTimer
  \/ CandidateUnavailable
  \/ FramedCandidateUnavailable
  \/ FailOpenAttempt
  \/ ExhaustRecoveryBudget

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(DeliverAttachResponse)
  /\ WF_vars(InstallProjectionContinuation)
  /\ WF_vars(ResumeInbound)
  /\ WF_vars(DeliverFirstFrame)
  /\ WF_vars(DeliverHostClose)
  /\ WF_vars(SettleConnectionCleanup)

TypeOK ==
  /\ Scenario \in Scenarios
  /\ s.clientRequests \subseteq Attempts
  /\ s.allocatedRequests \subseteq Attempts
  /\ s.serverRequests \subseteq Attempts
  /\ s.clientReleased \subseteq Attempts
  /\ s.serverReleaseRequested \subseteq Attempts
  /\ s.leases \subseteq Attempts
  /\ s.openEvents \subseteq Attempts
  /\ s.releaseEvents \subseteq Attempts
  /\ s.cleanupFailureEvents \subseteq Attempts
  /\ s.cleanupFailures \subseteq Attempts
  /\ s.cleanupEvidence \subseteq Attempts
  /\ s.activated \subseteq Attempts
  /\ s.latePortsClosed \subseteq Attempts
  /\ s.clientAcknowledged \subseteq Attempts
  /\ s.clientCleanupFailed \subseteq Attempts
  /\ s.connectionCleanupDelivered \in BOOLEAN
  /\ s.connectionSettled \subseteq Attempts
  /\ s.controlClosed \in BOOLEAN
  /\ s.detachInFlight \subseteq Attempts
  /\ s.detachRequests \subseteq Attempts
  /\ s.detachRequestsAtEnd \subseteq Attempts
  /\ s.hostQueue \in Seq(HostMessages)
  /\ s.hostQueued \subseteq HostMessages
  /\ s.responseResolved \in BOOLEAN
  /\ s.inboundBarrier \in BOOLEAN
  /\ s.listenerInstalled \in BOOLEAN
  /\ s.frameDelivered \in BOOLEAN
  /\ s.rendererReady \in BOOLEAN
  /\ s.frameAcknowledged \in BOOLEAN
  /\ s.hostCloseDelivered \in BOOLEAN
  /\ s.nextAttempt \in 1..3
  /\ s.pending \in Attempts \cup {NoAttempt}
  /\ s.candidate \in Attempts \cup {NoAttempt}
  /\ s.frozen \in Attempts \cup {NoAttempt}
  /\ s.active \in Attempts \cup {NoAttempt}
  /\ s.visible \in Attempts \cup {NoAttempt}
  /\ s.framed \subseteq Attempts
  /\ s.sealed \subseteq Attempts
  /\ s.releasedAttempts \subseteq Attempts
  /\ s.retryScheduled \in BOOLEAN
  /\ s.recoveryFailed \in BOOLEAN
  /\ s.requestTransportEnded \in BOOLEAN
  /\ s.operationState \in [Operations -> OperationStates]
  /\ s.operationAttempt \in [Operations -> Attempts \cup {NoAttempt}]
  /\ s.operationAttempts \in [Operations -> SUBSET Attempts]
  /\ s.selectionObservedAttempt \in Attempts \cup {NoAttempt}
  /\ s.selectionDeliveredAttempts \subseteq Attempts
  /\ s.findSeedAttempt \in Attempts \cup {NoAttempt}
  /\ s.lastAction \in Actions
  /\ s.lastAttempt \in Attempts \cup {NoAttempt}
  /\ s.previousClientRequests \subseteq Attempts

ClientReleasedIsNotActivated ==
  Scenario # "release-race" \/ s.activated \cap s.clientReleased = {}

ActivatedRequestHasOwnedLease ==
  Scenario # "release-race" \/ s.activated \subseteq (s.leases \cap s.serverRequests)

ControlCloseRetiresActivation ==
  Scenario # "release-race" \/ ~s.controlClosed \/ s.activated = {}

LateOpenIsRequestLocal ==
  Scenario # "release-race" \/
    /\ s.latePortsClosed \subseteq s.clientReleased
    /\ s.latePortsClosed \cap s.activated = {}
    /\ (s.lastAction = "discard-late-open" =>
          /\ ~s.controlClosed
          /\ s.clientRequests = s.previousClientRequests
          /\ s.lastAttempt \in s.clientRequests)

CleanupFailureRetainsServerOwnership ==
  Scenario # "release-race" \/
    s.cleanupFailures \subseteq
      (s.leases \cap s.serverRequests \cap s.serverReleaseRequested)

ReleaseAcknowledgementHasCleanupEvidence ==
  Scenario # "release-race" \/
    /\ s.clientAcknowledged \subseteq s.cleanupEvidence
    /\ s.releaseEvents \subseteq s.cleanupEvidence

TransportEndSealsDetachAdmission ==
  Scenario # "release-race" \/
    ~s.requestTransportEnded \/ s.detachRequests = s.detachRequestsAtEnd

ConnectionCleanupConvergesLocalOwnership ==
  Scenario # "release-race" \/
    ~s.connectionCleanupDelivered \/
      /\ s.requestTransportEnded
      /\ s.leases = {}
      /\ s.serverRequests = {}
      /\ s.cleanupFailures = {}
      /\ s.detachInFlight = {}

ConnectionCleanupSettlesOnlyAllocatedAttempts ==
  Scenario # "release-race" \/ s.connectionSettled \subseteq s.allocatedRequests

OpenPublicationRequiresUnreleasedServerRequest ==
  Scenario # "release-race" \/
    s.lastAction # "publish-open" \/
      s.lastAttempt \notin s.serverReleaseRequested

ResponseBarrierProtectsContinuation ==
  Scenario # "attach-order" \/
    ~s.responseResolved \/ s.listenerInstalled \/ s.inboundBarrier

FrameRequiresInstalledListener ==
  Scenario # "attach-order" \/ ~s.frameDelivered \/ s.listenerInstalled

FrameAcknowledgementRequiresReadyFrame ==
  Scenario # "attach-order" \/
    ~s.frameAcknowledged \/
      /\ s.frameDelivered
      /\ s.rendererReady

CloseRequiresInstalledListener ==
  Scenario # "attach-order" \/ ~s.hostCloseDelivered \/ s.listenerInstalled

ActiveAttemptOwnsVisibleFramedProjection ==
  Scenario # "recovery-operations" \/
    s.active = NoAttempt \/
      /\ s.active = s.visible
      /\ s.active \in s.framed
      /\ s.active \notin s.sealed

FrozenProjectionRemainsVisibleDuringCandidateRecovery ==
  Scenario # "recovery-operations" \/
    s.frozen = NoAttempt \/ s.visible = s.frozen

VisibleProjectionHasPaintedFrame ==
  Scenario # "recovery-operations" \/
    s.visible = NoAttempt \/ s.visible \in s.framed

ReleasedFramedAttemptIsSealed ==
  Scenario # "recovery-operations" \/
    (s.framed \cap s.releasedAttempts) \subseteq s.sealed

CandidateUsesFreshIdentity ==
  Scenario # "recovery-operations" \/
    s.candidate = NoAttempt \/ s.frozen = NoAttempt \/ s.candidate # s.frozen

NoAutomaticOperationReplay ==
  Scenario # "recovery-operations" \/
    \A operation \in Operations: Cardinality(s.operationAttempts[operation]) <= 1

OperationIdentityRemainsPinned ==
  Scenario # "recovery-operations" \/
    \A operation \in Operations:
      IF s.operationAttempt[operation] = NoAttempt
      THEN s.operationAttempts[operation] = {}
      ELSE s.operationAttempts[operation] = {s.operationAttempt[operation]}

UnknownOutcomeBelongsToSealedAttempt ==
  Scenario # "recovery-operations" \/
    \A operation \in Operations:
      s.operationState[operation] = "unknown" =>
        s.operationAttempt[operation] \in s.sealed

FindSeedUsesDeliveredSelectionFromSameAttempt ==
  Scenario # "recovery-operations" \/
    s.findSeedAttempt = NoAttempt \/
      /\ s.findSeedAttempt = s.operationAttempt["find"]
      /\ s.findSeedAttempt \in s.selectionDeliveredAttempts

RetryDoesNotAdmitOperationsToCandidate ==
  Scenario # "recovery-operations" \/
    s.candidate = NoAttempt \/
      \A operation \in Operations: s.candidate \notin s.operationAttempts[operation]

ResponseContinuationEventuallyInstalls ==
  [](s.responseResolved ~> s.listenerInstalled)

QueuedFrameEventuallyDelivers ==
  []("first-frame" \in s.hostQueued ~> s.frameDelivered)

QueuedCloseEventuallyDelivers ==
  []("close" \in s.hostQueued ~> s.hostCloseDelivered)

RequestTransportEndEventuallySettles ==
  Scenario # "release-race" \/
    [](s.requestTransportEnded => <>s.connectionCleanupDelivered)

=============================================================================
