---------------- MODULE NativeClientRetiredMembershipRecovery ----------------
EXTENDS Integers, Sequences, FiniteSets

CONSTANTS InitialOrdinal, SeedClass, MaxAttempts, MaxCursor, MaxAllocator, MaxTime, Budget, CleanupReserve
ASSUME /\ MaxAttempts = 2 /\ MaxCursor = 3 /\ MaxAllocator = 5 /\ MaxTime = 8
       /\ Budget = 2 /\ CleanupReserve = 1
       /\ InitialOrdinal \in 0..3 /\ SeedClass \in {"normal", "near-limit", "exhausted"}

Tabs == {"A", "B", "C"}
Members(order) == {order[i] : i \in 1..Len(order)}
Baselines == {<<"A">>, <<"A", "B", "C">>, <<"B", "A", "C">>,
              <<"B", "C", "A">>}
Candidates == {<<>>, <<"B">>, <<"C">>, <<"B", "C">>, <<"C", "B">>}
Orders == Baselines \cup Candidates
Ordinal(order) == CHOOSE i \in 1..Len(order): order[i] = "A"
Neighbor(order, ordinal) == IF order = <<>> THEN "none"
                           ELSE order[IF ordinal <= Len(order) THEN ordinal
                                      ELSE Len(order)]
AllocatorNames == {"identity", "refresh", "command", "wait"}
NormalSeeds == {[name \in AllocatorNames |-> 1]}
ExhaustedSeeds == {[name \in AllocatorNames |-> IF name = exhausted THEN MaxAllocator ELSE 1]
           : exhausted \in AllocatorNames}
NearLimitSeeds == {[name \in AllocatorNames |-> IF name = "identity" THEN remaining ELSE 1]
           : remaining \in {MaxAllocator - 2, MaxAllocator - 1}}
Seeds == CASE SeedClass = "normal" -> NormalSeeds
              [] SeedClass = "near-limit" -> NearLimitSeeds
              [] OTHER -> ExhaustedSeeds
InitialBaseline == CASE InitialOrdinal = 0 -> <<"A">>
                        [] InitialOrdinal = 1 -> <<"A", "B", "C">>
                        [] InitialOrdinal = 2 -> <<"B", "A", "C">>
                        [] OTHER -> <<"B", "C", "A">>
BeforeInstall == {"connect", "snapshot-ready", "snapshot-wait", "received",
                  "await-ui", "staged"}
AfterInstall == {"await-ack", "attach-ready", "setup-wait", "cleanup", "cleanup-wait"}
Active == BeforeInstall \cup AfterInstall
Phases == Active \cup {"observing", "close-old", "join-old", "parked",
                       "retiring", "join-attempt", "live", "closed"}

VARIABLE s
vars == <<s>>

Initial(order, conflict, seed, generation) == [
  phase |-> "observing", baseline |-> order, cache |-> order,
  revision |-> 1, ordinal |-> Ordinal(order), conflict |-> conflict,
  observed |-> "none", origin |-> FALSE, oldClosed |-> FALSE,
  oldJoined |-> FALSE, oldLocalViewer |-> TRUE, oldLocalRequests |-> 0,
  oldRemoteReference |-> TRUE, newRemoteReference |-> FALSE,
  carrier |-> "old", localViewer |-> "A", desired |-> "A",
  initialCursor |-> generation, cursor |-> generation, alloc |-> seed,
  attempt |-> 0, flight |-> 0, typedFlight |-> 0, now |-> 0,
  forward |-> -1, total |-> -1, candidate |-> <<>>,
  candidateRevision |-> 0, fault |-> "none", installed |-> FALSE,
  installTime |-> -1, freshReady |-> FALSE, cancelled |-> FALSE,
  lastRequest |-> "none", requests |-> 0
]

Init == \E order \in {InitialBaseline}, conflict \in {"none", "close-read", "structural"},
           seed \in Seeds, generation \in {1, 2, 3}:
          s = Initial(order, conflict, seed, generation)

ObserveCurrentUnavailable ==
  /\ s.phase = "observing"
  /\ s' = [s EXCEPT !.phase = "close-old", !.observed = "current-typed",
              !.oldLocalRequests = 1]

IgnoreStaleUnavailable ==
  /\ s.phase = "observing"
  /\ s' = [s EXCEPT !.phase = "close-old", !.observed = "stale"]

ObserveTransportAmbiguity ==
  /\ s.phase = "observing"
  /\ s' = [s EXCEPT !.phase = "close-old", !.observed = "unknown"]

CloseOldTransport ==
  /\ s.phase = "close-old"
  /\ s' = [s EXCEPT !.phase = "join-old", !.oldClosed = TRUE,
              !.carrier = "none"]

JoinOldTask ==
  /\ s.phase = "join-old" /\ s.oldClosed
  /\ s' = [s EXCEPT !.phase = "parked", !.oldJoined = TRUE,
              !.oldLocalViewer = FALSE, !.localViewer = "none",
              !.origin = (s.observed = "current-typed" /\ s.conflict = "none")]

JoinMissingReturn ==
  /\ s.phase = "join-old" /\ s.oldClosed
  /\ s' = [s EXCEPT !.phase = "parked", !.oldJoined = TRUE,
              !.oldLocalViewer = FALSE, !.localViewer = "none", !.origin = FALSE]

\* Remote work already handed off can finish after local join. No fairness
\* demands this action, and it is not a Detached/Gone proof for installation.
ReleaseOldRemoteReference ==
  /\ s.oldRemoteReference
  /\ s' = [s EXCEPT !.oldRemoteReference = FALSE]

RejectOldLocalEffect ==
  /\ s.oldJoined
  /\ UNCHANGED s

Eligible == s.origin /\ s.oldJoined /\ s.oldClosed /\ s.conflict = "none"
AdmissionCapacity == s.alloc.command < MaxAllocator /\ s.alloc.refresh < MaxAllocator

BeginExplicitRecovery ==
  /\ s.phase = "parked" /\ Eligible /\ AdmissionCapacity
  /\ s.attempt < MaxAttempts /\ s.now + Budget + CleanupReserve <= MaxTime
  /\ s' = [s EXCEPT !.phase = "connect", !.attempt = @ + 1,
              !.flight = s.alloc.refresh + 1,
              !.alloc.command = @ + 1, !.alloc.refresh = @ + 1,
              !.forward = s.now + Budget, !.total = s.now + Budget + CleanupReserve,
              !.candidate = <<>>, !.candidateRevision = 0, !.typedFlight = 0,
              !.fault = "none", !.carrier = "connecting"]

RejectRecoveryAdmission ==
  /\ s.phase = "parked" /\ (~Eligible \/ ~AdmissionCapacity)
  /\ UNCHANGED s

ConnectRecovery ==
  /\ s.phase = "connect" /\ s.now < s.forward
  /\ s' = [s EXCEPT !.phase = "snapshot-ready", !.carrier = "fresh"]

ConnectFailure ==
  /\ s.phase = "connect"
  /\ s' = [s EXCEPT !.phase = "retiring"]

SubmitSnapshot ==
  /\ s.phase = "snapshot-ready" /\ s.now < s.forward
  /\ s.alloc.identity < MaxAllocator
  /\ s' = [s EXCEPT !.phase = "snapshot-wait", !.alloc.identity = @ + 1,
              !.lastRequest = "snapshot", !.requests = @ + 1]

RequestIdentityExhausted ==
  /\ s.phase = "snapshot-ready" /\ s.alloc.identity = MaxAllocator
  /\ s' = [s EXCEPT !.phase = "retiring"]

ReceiveSnapshot ==
  /\ s.phase = "snapshot-wait" /\ s.now < s.forward
  /\ \E candidate \in Candidates \cup {s.baseline}, revision \in {0, 1, 2},
         fault \in {"none", "scope", "tuple"}:
       s' = [s EXCEPT !.phase = "received", !.candidate = candidate,
              !.candidateRevision = revision, !.fault = fault, !.typedFlight = s.flight]

SnapshotUnavailable ==
  /\ s.phase = "snapshot-wait"
  /\ s' = [s EXCEPT !.phase = "retiring"]

WrongSnapshotCorrelation ==
  /\ s.phase = "snapshot-wait"
  /\ s' = [s EXCEPT !.phase = "retiring"]

PlaceCandidate ==
  /\ s.phase = "received" /\ s.now < s.forward /\ s.alloc.wait < MaxAllocator
  /\ s' = [s EXCEPT !.phase = "await-ui", !.alloc.wait = @ + 1]

UiWaitIdentityExhausted ==
  /\ s.phase = "received" /\ s.alloc.wait = MaxAllocator
  /\ s' = [s EXCEPT !.phase = "retiring"]

ValidCandidate ==
  /\ s.fault = "none" /\ s.typedFlight = s.flight
  /\ s.candidateRevision >= s.revision
  /\ (s.candidateRevision = s.revision => s.candidate = s.cache)
  /\ "A" \notin Members(s.candidate)

StageCandidate ==
  /\ s.phase = "await-ui" /\ s.now < s.forward /\ ValidCandidate
  /\ s' = [s EXCEPT !.phase = "staged"]

RejectCandidate ==
  /\ s.phase = "await-ui" /\ ~ValidCandidate
  /\ s' = [s EXCEPT !.phase = "retiring"]

InstallCandidate ==
  /\ s.phase = "staged" /\ s.now < s.forward /\ Eligible /\ ValidCandidate
  /\ (s.candidate = <<>> \/ s.cursor < MaxCursor)
  /\ s' = [s EXCEPT !.phase = "await-ack", !.cache = s.candidate,
              !.revision = s.candidateRevision, !.installed = TRUE,
              !.installTime = s.now, !.origin = FALSE,
              !.desired = Neighbor(s.candidate, s.ordinal),
              !.cursor = IF s.candidate = <<>> THEN @ ELSE @ + 1]

AttachmentIdentityExhausted ==
  /\ s.phase = "staged" /\ s.candidate # <<>> /\ s.cursor = MaxCursor
  /\ s' = [s EXCEPT !.phase = "retiring"]

ConsumeInstallAcknowledgment ==
  /\ s.phase = "await-ack" /\ s.now < s.total
  /\ s' = [s EXCEPT !.phase = IF s.desired = "none" THEN "live"
                             ELSE IF s.now < s.forward THEN "attach-ready"
                             ELSE "retiring"]

SubmitFreshSetup ==
  /\ s.phase = "attach-ready" /\ s.now < s.forward
  /\ s.alloc.identity < MaxAllocator
  /\ s' = [s EXCEPT !.phase = "setup-wait", !.alloc.identity = @ + 1,
              !.lastRequest = "fresh-setup", !.requests = @ + 1,
              !.newRemoteReference = TRUE]

SetupIdentityExhausted ==
  /\ s.phase = "attach-ready" /\ s.alloc.identity = MaxAllocator
  /\ s' = [s EXCEPT !.phase = "retiring"]

\* This completion composes the existing new-connection setup contract,
\* including exact attachment/frame/feedback proof. It is not merely an ACK.
CompleteFreshSetup ==
  /\ s.phase = "setup-wait" /\ s.now < s.forward
  /\ s' = [s EXCEPT !.phase = "live", !.localViewer = s.desired,
              !.freshReady = TRUE]

FreshSetupUnavailable ==
  /\ s.phase = "setup-wait" /\ s.now < s.forward
  /\ s' = [s EXCEPT !.phase = "live", !.newRemoteReference = FALSE]

FreshSetupUnknown ==
  /\ s.phase = "setup-wait"
  /\ s' = [s EXCEPT !.phase = "retiring"]

FreshSetupNeedsCleanup ==
  /\ s.phase = "setup-wait" /\ s.now < s.total
  /\ s' = [s EXCEPT !.phase = "cleanup", !.localViewer = s.desired]

SubmitFreshCleanup ==
  /\ s.phase = "cleanup" /\ s.now < s.total
  /\ s.alloc.identity < MaxAllocator
  /\ s' = [s EXCEPT !.phase = "cleanup-wait", !.alloc.identity = @ + 1,
              !.lastRequest = "fresh-cleanup", !.requests = @ + 1]

CleanupIdentityExhausted ==
  /\ s.phase = "cleanup" /\ s.alloc.identity = MaxAllocator
  /\ s' = [s EXCEPT !.phase = "retiring"]

CompleteFreshCleanup ==
  /\ s.phase = "cleanup-wait" /\ s.now < s.total
  /\ s' = [s EXCEPT !.phase = "retiring", !.localViewer = "none",
              !.newRemoteReference = FALSE]

ExpireBeforeInstall ==
  /\ s.phase \in BeforeInstall /\ s.now >= s.forward
  /\ s' = [s EXCEPT !.phase = "retiring"]

ExpireAfterInstall ==
  /\ s.phase \in AfterInstall
  /\ IF s.phase \in {"await-ack", "cleanup", "cleanup-wait"} THEN s.now >= s.total
     ELSE s.now >= s.forward
  /\ s' = [s EXCEPT !.phase = "retiring"]

CancelRecovery ==
  /\ s.phase \in Active
  /\ s' = [s EXCEPT !.phase = "retiring", !.cancelled = TRUE]

CloseAttemptTransport ==
  /\ s.phase = "retiring"
  /\ s' = [s EXCEPT !.phase = "join-attempt", !.carrier = "none",
              !.localViewer = "none"]

RejectLateConnection ==
  /\ s.phase = "join-attempt" /\ s.cancelled
  /\ UNCHANGED s

JoinAttempt ==
  /\ s.phase = "join-attempt" /\ s.carrier = "none"
  /\ s' = [s EXCEPT !.phase = IF s.cancelled THEN "closed" ELSE "parked"]

IgnoreOldResult ==
  /\ s.oldJoined
  /\ UNCHANGED s

RejectForbiddenObservationRequest ==
  /\ s.phase \in BeforeInstall
  /\ UNCHANGED s

Tick ==
  /\ s.now < MaxTime
  /\ s' = [s EXCEPT !.now = @ + 1]

Next == ObserveCurrentUnavailable \/ IgnoreStaleUnavailable \/ ObserveTransportAmbiguity
  \/ CloseOldTransport \/ JoinOldTask \/ JoinMissingReturn \/ ReleaseOldRemoteReference
  \/ RejectOldLocalEffect \/ BeginExplicitRecovery \/ RejectRecoveryAdmission
  \/ ConnectRecovery \/ ConnectFailure \/ SubmitSnapshot \/ RequestIdentityExhausted
  \/ ReceiveSnapshot \/ SnapshotUnavailable \/ WrongSnapshotCorrelation
  \/ PlaceCandidate \/ UiWaitIdentityExhausted \/ StageCandidate \/ RejectCandidate
  \/ InstallCandidate \/ AttachmentIdentityExhausted \/ ConsumeInstallAcknowledgment
  \/ SubmitFreshSetup \/ SetupIdentityExhausted \/ CompleteFreshSetup
  \/ FreshSetupUnavailable \/ FreshSetupUnknown \/ FreshSetupNeedsCleanup
  \/ SubmitFreshCleanup \/ CleanupIdentityExhausted \/ CompleteFreshCleanup \/ ExpireBeforeInstall \/ ExpireAfterInstall \/ CancelRecovery
  \/ CloseAttemptTransport \/ RejectLateConnection \/ JoinAttempt \/ IgnoreOldResult
  \/ RejectForbiddenObservationRequest \/ Tick

Spec == Init /\ [][Next]_vars /\ WF_vars(Tick)
        /\ WF_vars(ExpireBeforeInstall) /\ WF_vars(ExpireAfterInstall)
        /\ WF_vars(CloseAttemptTransport) /\ WF_vars(JoinAttempt)

TypeOK ==
  /\ s.phase \in Phases /\ s.cache \in Orders /\ s.baseline \in Baselines
  /\ s.candidate \in Orders /\ s.revision \in {0, 1, 2}
  /\ s.candidateRevision \in 0..2 /\ s.cursor \in 1..MaxCursor
  /\ s.alloc \in [AllocatorNames -> 1..MaxAllocator]
  /\ s.attempt \in 0..MaxAttempts /\ s.now \in 0..MaxTime
  /\ s.carrier \in {"old", "none", "connecting", "fresh"}
  /\ s.desired \in Tabs \cup {"none"} /\ s.localViewer \in Tabs \cup {"none"}

OneCarrier == (s.carrier \in {"connecting", "fresh"} => s.oldClosed /\ s.oldJoined)
NoOldViewerReactivation == s.oldJoined => ~s.oldLocalViewer /\ s.localViewer # "A"
OriginIsExact == s.origin => s.observed = "current-typed" /\ s.conflict = "none"
InstalledHasAuthority == s.installed =>
  /\ s.conflict = "none" /\ s.observed = "current-typed" /\ s.oldJoined
  /\ "A" \notin Members(s.cache) /\ s.revision = 2 /\ s.fault = "none"
  /\ s.installTime < s.forward
AtomicSelection == s.installed =>
  /\ s.cache = s.candidate /\ s.desired = Neighbor(s.cache, s.ordinal)
  /\ s.cursor = s.initialCursor + IF s.cache = <<>> THEN 0 ELSE 1
EmptyHasNoViewer == s.installed /\ s.cache = <<>> =>
  s.desired = "none" /\ s.localViewer = "none" /\ ~s.newRemoteReference /\ ~s.freshReady
ReadyHasFreshProof == s.freshReady => s.installed /\ s.localViewer = s.desired
PreinstallPreservesCanonical == ~s.installed => s.cache = s.baseline /\ s.revision = 1
ConflictPreservesFacts == s.conflict # "none" => s.attempt = 0 /\ ~s.installed

NoOldLocalSubmissionAfterJoin == [][s.oldJoined => s'.oldLocalRequests = s.oldLocalRequests]_vars
AllocatorsNeverRewind == [][s'.cursor >= s.cursor /\
  \A name \in AllocatorNames: s'.alloc[name] >= s.alloc[name]]_vars
OnlyExplicitRetry == [][s'.attempt > s.attempt => BeginExplicitRecovery]_vars
OnlySnapshotBeforeInstall == [][s'.requests > s.requests /\ ~s.installed =>
  s'.lastRequest = "snapshot"]_vars
DeadlineNeverRenews == [][s.attempt = s'.attempt =>
  s.forward = s'.forward /\ s.total = s'.total]_vars
CommitNeverForgotten == [][s.installed =>
  s'.installed /\ s'.cache = s.cache /\ s'.cursor = s.cursor]_vars
StaleResultCannotInstall == [][(IgnoreOldResult \/ WrongSnapshotCorrelation \/ RejectLateConnection)
  => s'.cache = s.cache /\ s'.cursor = s.cursor /\ s'.flight = s.flight]_vars
AttemptEventuallySettles == s.phase \in Active \cup {"retiring", "join-attempt"}
                           ~> s.phase \in {"parked", "live", "closed"}
=============================================================================
