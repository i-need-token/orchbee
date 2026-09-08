---------------------- MODULE WorkerTerminalLifecycleStep ----------------------
EXTENDS Naturals, FiniteSets, TLC

\* Worker-owned one-request Close refinement. Server owns whole attempts, grace,
\* inventory walks, revision credits and publication; Host owns process cleanup.
\* One immutable admitted connection is an Init premise from LocalHostBroker.
\* prepared/sent/result-ready/finished map to its admitted/awaiting-host/
\* result-ready/completed-or-abandoned phases; no Launch binding is evicted.
\* Host ACK echoes terminal only, with epoch/correlation in the envelope. The
\* creation and Worker identity remain in the prepared seal and OBCN result.
\* Independent timeout actions abstract 1s HostExchange, 2s Server frame I/O,
\* 20s response wait and 30s admission expiry; they are NOT a timed simulation.
\* No fairness, state constraint, adoption, hidden retry or eventual-success claim.

CONSTANT CacheVariant

Target == [product |-> 1, worker |-> 1, generation |-> 1,
           epoch |-> 1, creation |-> 1, terminal |-> 1]
IdentityFields == {"product", "worker", "generation", "epoch", "creation", "terminal"}
Identities == {Target} \cup {[Target EXCEPT ![field] = 2] : field \in IdentityFields}
Kinds == {"terminate", "force", "dispose", "inventory-page"}
CacheTargets == {"target", "other"}
ControllerIds == {"none", "old", "new"}
EmptyAck == [role |-> "none", kind |-> "none", correlation |-> 0,
             epoch |-> 0, terminal |-> 0]
ExactAck(kind) == [role |-> "response", kind |-> kind, correlation |-> 1,
                   epoch |-> Target.epoch, terminal |-> Target.terminal]
BadAcks(kind) == {
  [ExactAck(kind) EXCEPT !.role = "event"],
  [ExactAck(kind) EXCEPT !.kind = "other"],
  [ExactAck(kind) EXCEPT !.correlation = 2],
  [ExactAck(kind) EXCEPT !.epoch = 2],
  [ExactAck(kind) EXCEPT !.terminal = 2]
}
ExactResult == [identity |-> Target, operation |-> 1, attempt |-> 1, step |-> 1]
BadResults == {[ExactResult EXCEPT !.identity = identity] : identity \in Identities \ {Target}}
  \cup {[ExactResult EXCEPT ![field] = 2] : field \in {"operation", "attempt", "step"}}
EmptyResult == [identity |-> Target, operation |-> 0, attempt |-> 0, step |-> 0]

VARIABLES identity, kind, initialViewers, initialController, s
vars == <<identity, kind, initialViewers, initialController, s>>
fixed == <<identity, kind, initialViewers, initialController>>

Init ==
  /\ identity \in IF CacheVariant THEN {Target} ELSE Identities
  /\ kind \in IF CacheVariant THEN {"dispose"} ELSE Kinds
  /\ initialViewers \in IF CacheVariant THEN SUBSET CacheTargets ELSE {{}}
  /\ initialController \in IF "target" \in initialViewers THEN ControllerIds ELSE {"none"}
  /\ s = [phase |-> "idle", accepted |-> FALSE, fresh |-> TRUE,
           grant |-> TRUE, prepared |-> FALSE, handoff |-> FALSE,
           requests |-> 0, applied |-> FALSE, ack |-> EmptyAck,
           worker |-> "none", workerDisposed |-> FALSE, workerClean |-> FALSE,
           server |-> "pending", serverResult |-> EmptyResult,
           serverDisposed |-> FALSE, revoked |-> FALSE, serverClean |-> FALSE,
           serverActive |-> TRUE, expired |-> FALSE, hostExpired |-> FALSE,
           ioExpired |-> FALSE, responseExpired |-> FALSE,
           viewers |-> initialViewers, frame |-> "target" \in initialViewers,
           feedback |-> "target" \in initialViewers,
           controller |-> initialController, otherController |-> "other" \in initialViewers,
           reservation |-> initialController, otherReservation |-> "other" \in initialViewers,
           sealController |-> "none", oldLease |-> "target" \in initialViewers,
           oldViewer |-> "target" \in initialViewers,
           pendingAcquire |-> initialController = "new", installedLease |-> FALSE,
           leaseHostCleanup |-> 0, last |-> "init"]

AcceptClose ==
  /\ ~s.accepted
  /\ s' = [s EXCEPT !.accepted = TRUE, !.installedLease = FALSE, !.last = "accept"]
  /\ UNCHANGED fixed

RejectAuthorization ==
  /\ s.accepted /\ s.phase = "idle" /\ (identity # Target \/ ~s.grant)
  /\ s' = [s EXCEPT !.phase = "result-ready", !.worker = "unavailable", !.last = "reject-authorization"]
  /\ UNCHANGED fixed

InvalidateGrant ==
  /\ s.phase = "idle" /\ s.grant
  /\ s' = [s EXCEPT !.grant = FALSE, !.last = "invalidate-grant"]
  /\ UNCHANGED fixed

PrepareStep ==
  /\ s.accepted /\ s.phase = "idle" /\ identity = Target /\ s.grant
  /\ ~s.expired /\ s.serverActive
  /\ s' = [s EXCEPT !.phase = "prepared", !.prepared = TRUE,
                     !.sealController = s.reservation, !.last = "prepare"]
  /\ UNCHANGED fixed

InvalidateBeforeHandoff ==
  /\ s.phase = "prepared" /\ s.fresh
  /\ s' = [s EXCEPT !.fresh = FALSE, !.last = "invalidate-handoff"]
  /\ UNCHANGED fixed

RejectFreshHandoff ==
  /\ s.phase = "prepared" /\ ~s.fresh
  /\ s' = [s EXCEPT !.phase = "result-ready", !.worker = "unavailable", !.last = "reject-handoff"]
  /\ UNCHANGED fixed

FailBeforeHandoff ==
  /\ s.accepted /\ s.phase \in {"idle", "prepared"}
  /\ s' = [s EXCEPT !.phase = "result-ready", !.worker = "unavailable", !.last = "pre-handoff-failure"]
  /\ UNCHANGED fixed

SubmitHostRequest ==
  /\ s.phase = "prepared" /\ s.fresh /\ s.serverActive
  /\ s' = [s EXCEPT !.phase = "sent", !.handoff = TRUE, !.requests = @ + 1, !.last = "submit"]
  /\ UNCHANGED fixed

ApplyHostRequest ==
  /\ s.phase = "sent" /\ ~s.applied
  /\ s' = [s EXCEPT !.applied = TRUE, !.last = "host-apply"]
  /\ UNCHANGED fixed

ObserveAck ==
  /\ s.phase = "sent" /\ s.applied /\ ~s.hostExpired
  /\ s' = [s EXCEPT !.phase = IF kind = "dispose" THEN "ack-known" ELSE "result-ready",
                     !.ack = ExactAck(kind), !.worker = "success",
                     !.workerDisposed = kind = "dispose", !.last = "exact-ack"]
  /\ UNCHANGED fixed

ReceiveExactAck == /\ ~s.expired /\ ObserveAck

ReceiveLateExactAck == /\ s.expired /\ ObserveAck

ReceiveTypedFailure ==
  /\ s.phase = "sent" /\ ~s.hostExpired
  /\ s' = [s EXCEPT !.phase = "result-ready", !.ack = [ExactAck("typed-failure") EXCEPT !.terminal = 0],
                     !.worker = "failure", !.last = "typed-failure"]
  /\ UNCHANGED fixed

ReceiveInvalidAck ==
  /\ s.phase = "sent"
  /\ \E ack \in BadAcks(kind) : s' = [s EXCEPT !.phase = "result-ready", !.ack = ack,
                     !.worker = "unknown", !.last = "invalid-ack"]
  /\ UNCHANGED fixed

RetainUnknown ==
  /\ s.phase = "sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.worker = "unknown", !.last = "unknown"]
  /\ UNCHANGED fixed

ExpireAttempt ==
  /\ s.accepted /\ ~s.expired
  /\ s' = [s EXCEPT !.expired = TRUE, !.last = "attempt-expiry"]
  /\ UNCHANGED fixed

ExpireHostExchange ==
  /\ s.phase = "sent" /\ ~s.hostExpired
  /\ s' = [s EXCEPT !.hostExpired = TRUE, !.phase = "result-ready",
                     !.worker = "unknown", !.last = "host-expiry"]
  /\ UNCHANGED fixed

ExpireServerFrameIo ==
  /\ s.handoff /\ s.server = "pending" /\ s.serverActive /\ ~s.ioExpired
  /\ s' = [s EXCEPT !.ioExpired = TRUE, !.serverActive = FALSE,
                     !.server = "unknown", !.last = "io-expiry"]
  /\ UNCHANGED fixed

ExpireServerResponseWait ==
  /\ s.handoff /\ s.server = "pending" /\ s.serverActive /\ ~s.responseExpired
  /\ s' = [s EXCEPT !.responseExpired = TRUE, !.serverActive = FALSE,
                     !.server = "unknown", !.last = "response-expiry"]
  /\ UNCHANGED fixed

CommitWorkerTargetCleanup ==
  /\ s.phase = "ack-known" /\ s.workerDisposed /\ s.prepared
  /\ s' = [s EXCEPT !.phase = "result-ready", !.workerClean = TRUE,
                     !.viewers = @ \ {"target"}, !.controller = "none",
                     !.frame = FALSE, !.feedback = FALSE, !.last = "worker-cleanup"]
  /\ UNCHANGED fixed

PublishWorkerResult ==
  /\ s.phase = "result-ready"
  /\ s' = [s EXCEPT !.phase = "published", !.last = "publish-result"]
  /\ UNCHANGED fixed

LoseWorkerResult ==
  /\ s.phase \in {"result-ready", "published"}
  /\ s' = [s EXCEPT !.phase = "finished", !.serverActive = FALSE,
                     !.server = IF s.handoff THEN "unknown" ELSE "unavailable",
                     !.last = "lose-result"]
  /\ UNCHANGED fixed

ObserveExactWorkerResult ==
  /\ s.phase = "published" /\ s.serverActive
  /\ s' = [s EXCEPT !.phase = "finished", !.serverResult = ExactResult,
                     !.server = IF s.workerDisposed THEN "cleanup-pending" ELSE s.worker,
                     !.serverDisposed = s.workerDisposed, !.last = "observe-result"]
  /\ UNCHANGED fixed

RejectForeignWorkerResult(result) ==
  /\ s.phase = "published" /\ s.serverActive /\ result \in BadResults
  /\ s' = [s EXCEPT !.phase = "finished", !.serverResult = result,
                     !.server = IF s.handoff THEN "unknown" ELSE "unavailable",
                     !.serverActive = FALSE, !.last = "reject-result"]
  /\ UNCHANGED fixed

FailServerTargetCleanup ==
  /\ s.serverDisposed /\ ~s.serverClean /\ s.last # "server-cleanup-failed"
  /\ s' = [s EXCEPT !.last = "server-cleanup-failed"]
  /\ UNCHANGED fixed

CommitServerTargetCleanup ==
  /\ s.serverDisposed /\ ~s.serverClean
  /\ s' = [s EXCEPT !.serverClean = TRUE, !.revoked = TRUE,
                     !.reservation = IF @ = s.sealController THEN "none" ELSE @,
                     !.server = "cleanup-complete", !.last = "server-cleanup"]
  /\ UNCHANGED fixed

CompleteAcquireBeforeClose ==
  /\ ~s.accepted /\ s.pendingAcquire
  /\ s' = [s EXCEPT !.pendingAcquire = FALSE, !.installedLease = TRUE, !.last = "acquire-before-close"]
  /\ UNCHANGED fixed

RejectAcquireAfterClose ==
  /\ s.accepted /\ s.pendingAcquire
  /\ s' = [s EXCEPT !.pendingAcquire = FALSE, !.installedLease = FALSE, !.last = "reject-late-acquire"]
  /\ UNCHANGED fixed

RejectNewTargetAdmission ==
  /\ s.accepted /\ s.last # "reject-new-admission"
  /\ s' = [s EXCEPT !.last = "reject-new-admission"]
  /\ UNCHANGED fixed

\* Legitimate old lease cleanup remains possible after Close Unknown. It is
\* controller-ID exact and independent of disposal-derived cache invalidation.
DropOldLeaseExactMatch ==
  /\ s.accepted /\ s.oldLease /\ s.reservation = "old"
  /\ s' = [s EXCEPT !.oldLease = FALSE, !.reservation = "none",
                     !.controller = IF @ = "old" THEN "none" ELSE @,
                     !.leaseHostCleanup = IF s.revoked \/ s.workerClean THEN @ ELSE @ + 1,
                     !.last = "drop-exact-lease"]
  /\ UNCHANGED fixed

DropOldLeaseMismatchedReservation ==
  /\ s.accepted /\ s.oldLease /\ s.reservation = "new"
  /\ s' = [s EXCEPT !.oldLease = FALSE,
                     !.leaseHostCleanup = IF s.revoked \/ s.workerClean THEN @ ELSE @ + 1,
                     !.last = "drop-mismatched-lease"]
  /\ UNCHANGED fixed

DropOldLeaseAfterReservationRemoval ==
  /\ s.accepted /\ s.oldLease /\ s.reservation = "none"
  /\ s' = [s EXCEPT !.oldLease = FALSE,
                     !.leaseHostCleanup = IF s.revoked \/ s.workerClean THEN @ ELSE @ + 1,
                     !.last = "drop-removed-lease"]
  /\ UNCHANGED fixed
DropOldViewer ==
  /\ s.accepted /\ s.oldViewer
  /\ s' = [s EXCEPT !.oldViewer = FALSE, !.viewers = @ \ {"target"},
                     !.frame = FALSE, !.feedback = FALSE,
                     !.leaseHostCleanup = IF s.revoked \/ s.workerClean THEN @ ELSE @ + 1,
                     !.last = "drop-viewer"]
  /\ UNCHANGED fixed

DrainLateTargetEvent ==
  /\ s.workerClean /\ s.last # "drain-late-event"
  /\ s' = [s EXCEPT !.last = "drain-late-event"]
  /\ UNCHANGED fixed

ObserveUnrelatedTargetUsable ==
  /\ "other" \in s.viewers /\ s.last # "other-usable"
  /\ s' = [s EXCEPT !.last = "other-usable"]
  /\ UNCHANGED fixed

BlockHiddenRetry ==
  /\ s.handoff /\ s.worker # "none" /\ s.last # "block-retry"
  /\ s' = [s EXCEPT !.last = "block-retry"]
  /\ UNCHANGED fixed

Next ==
  \/ AcceptClose \/ RejectAuthorization \/ InvalidateGrant \/ PrepareStep
  \/ InvalidateBeforeHandoff \/ RejectFreshHandoff \/ FailBeforeHandoff
  \/ SubmitHostRequest \/ ApplyHostRequest \/ ReceiveExactAck \/ ReceiveLateExactAck \/ ReceiveTypedFailure
  \/ ReceiveInvalidAck \/ RetainUnknown
  \/ ExpireAttempt \/ ExpireHostExchange \/ ExpireServerFrameIo \/ ExpireServerResponseWait
  \/ CommitWorkerTargetCleanup \/ PublishWorkerResult \/ LoseWorkerResult
  \/ ObserveExactWorkerResult \/ (\E result \in BadResults : RejectForeignWorkerResult(result))
  \/ FailServerTargetCleanup \/ CommitServerTargetCleanup
  \/ CompleteAcquireBeforeClose \/ RejectAcquireAfterClose \/ RejectNewTargetAdmission
  \/ DropOldLeaseExactMatch \/ DropOldLeaseMismatchedReservation \/ DropOldLeaseAfterReservationRemoval \/ DropOldViewer
  \/ DrainLateTargetEvent \/ ObserveUnrelatedTargetUsable \/ BlockHiddenRetry

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ identity \in Identities /\ kind \in Kinds
  /\ initialViewers \subseteq CacheTargets /\ initialController \in ControllerIds
  /\ s.phase \in {"idle", "prepared", "sent", "ack-known", "result-ready", "published", "finished"}
  /\ s.worker \in {"none", "unavailable", "success", "failure", "unknown"}
  /\ s.server \in {"pending", "unavailable", "success", "failure", "unknown", "cleanup-pending", "cleanup-complete"}
  /\ s.controller \in ControllerIds /\ s.reservation \in ControllerIds /\ s.sealController \in ControllerIds
  /\ s.viewers \subseteq CacheTargets /\ s.requests \in 0..1 /\ s.leaseHostCleanup \in 0..2
  /\ \A field \in {"accepted", "fresh", "grant", "prepared", "handoff", "applied",
       "workerDisposed", "workerClean", "serverDisposed", "revoked", "serverClean",
       "serverActive", "expired", "hostExpired", "ioExpired", "responseExpired", "frame",
       "feedback", "otherController", "otherReservation", "oldLease", "oldViewer",
       "pendingAcquire", "installedLease"} : s[field] \in BOOLEAN

OneHostRequestPerStep == s.requests <= 1 /\ (s.handoff \equiv s.requests = 1)
HandoffRequiresFreshExactAuthorization == s.handoff => identity = Target /\ s.fresh /\ s.grant /\ s.prepared
KnownAckRequiresFullCorrelation ==
  /\ (s.worker = "success" => s.ack = ExactAck(kind) /\ s.applied)
  /\ (s.worker = "failure" => s.ack = [ExactAck("typed-failure") EXCEPT !.terminal = 0])
PossibleDeliveryFailureIsUnknown == s.worker = "unavailable" => ~s.handoff
UnknownCannotAuthorizeDisposeCleanup == s.worker = "unknown" => ~s.workerDisposed /\ ~s.workerClean
WorkerDisposedResultRequiresTargetCleanup ==
  s.workerDisposed /\ s.phase \in {"result-ready", "published", "finished"} => s.workerClean
CleanupPreparedBeforeDisposeHandoff == kind = "dispose" /\ s.handoff => s.prepared
TargetFrameAndFeedbackFreed ==
  s.workerClean => "target" \notin s.viewers /\ s.controller = "none" /\ ~s.frame /\ ~s.feedback
TargetCleanupPreservesUnrelatedEntries ==
  /\ ("other" \in s.viewers \equiv "other" \in initialViewers)
  /\ (s.otherController \equiv "other" \in initialViewers)
  /\ (s.otherReservation \equiv "other" \in initialViewers)
ServerCleanupRequiresExactKnownWorkerDispose ==
  s.serverDisposed => s.workerDisposed /\ s.workerClean /\ s.serverResult = ExactResult
ServerCompletionRequiresExactReservationRevocation ==
  s.serverClean => s.serverDisposed /\ s.revoked /\ s.reservation = "none"
LateAcquireCannotRestoreClosingTarget == s.accepted => ~s.installedLease
KnownDisposeNeverRegresses == [][(s.workerDisposed => s'.workerDisposed) /\ (s.serverDisposed => s'.serverDisposed)]_vars
UnknownNeverDispatchesAnotherLifecycleRequest == [][s.worker = "unknown" => s'.requests = s.requests]_vars
OldLeaseCannotDeleteDifferentReservation ==
  [][s'.last = "drop-mismatched-lease" => s'.reservation = s.reservation /\ s'.controller = s.controller]_vars
OldLeaseAfterRevocationIsLocal == [][s.revoked \/ s.workerClean => s'.leaseHostCleanup = s.leaseHostCleanup]_vars
ExpiryCannotDiscardOutstandingOrKnownReceipt ==
  [][s'.last = "attempt-expiry" => s'.phase = s.phase /\ s'.worker = s.worker /\ s'.requests = s.requests]_vars
=============================================================================
