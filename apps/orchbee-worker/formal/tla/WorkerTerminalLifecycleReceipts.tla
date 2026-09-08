-------------------- MODULE WorkerTerminalLifecycleReceipts --------------------
EXTENDS Naturals, FiniteSets, TLC

\* One existing terminal association; Server has selected canonical Close A.
\* Receipt ordering is separate from WorkerTerminalLifecycleStep's transport and
\* cleanup product. A known Dispose completion is an input only after that model's
\* exact ACK and Worker cache cleanup. No Server recovery after Unknown is added.
\* Fresh IDs are derived to retain useful multi-query traces. Candidates also
\* include old IDs with changed attempt/ordinal, independently of that derivation.
\* The sum-type global slot is a structural premise, not a concurrency proof.
\* No fairness, state constraint, symmetry or eventual-success claim.

CONSTANTS MaxAttempt, MaxOrdinal, NoReceipt, OriginalSlot
None == NoReceipt
Owners == {"A", "B"}
Kinds == {"signal", "dispose", "query"}
Ids == (1..MaxAttempt) \X (1..MaxOrdinal) \X {1, 2}
Request(owner, attempt, ordinal, nonce, action) ==
  [owner |-> owner, attempt |-> attempt, ordinal |-> ordinal,
   id |-> <<attempt, ordinal, nonce>>, action |-> action]
FreshRequests == {Request(o, a, d, n, k) : o \in Owners,
  a \in 1..MaxAttempt, d \in 1..MaxOrdinal, n \in {1, 2}, k \in Kinds}
RequestType == [owner : Owners, attempt : 1..MaxAttempt, ordinal : 1..MaxOrdinal,
                id : Ids, action : Kinds]
Outcomes == {"signal-ack", "disposed", "query-known", "blocked", "unavailable", "unknown"}
ReceiptType == [request : RequestType, outcome : Outcomes]
IsMutation(request) == request.action # "query"

VARIABLE s
vars == <<s>>

Init == s = [owner |-> None, highAttempt |-> 0, highOrdinal |-> 0,
  phase |-> "empty", slot |-> None, result |-> None,
  lastStep |-> None, lastMutation |-> None, unknownMutation |-> FALSE,
  knownDisposed |-> FALSE, handoffs |-> [id \in Ids |-> 0], history |-> {},
  last |-> "init"]

ReceiptRequests ==
  (IF s.lastStep = None THEN {} ELSE {s.lastStep.request}) \cup
  (IF s.lastMutation = None THEN {} ELSE {s.lastMutation.request})
ChangedAttempts(request) ==
  {[request EXCEPT !.attempt = a] : a \in (1..MaxAttempt) \ {request.attempt}}
ChangedOrdinals(request) ==
  {[request EXCEPT !.ordinal = d] : d \in (1..MaxOrdinal) \ {request.ordinal}}
Candidates == FreshRequests \cup
  UNION {ChangedAttempts(r) \cup ChangedOrdinals(r) : r \in ReceiptRequests}
ExactReplay(request) == request \in ReceiptRequests
RetainedConflict(request) ==
  \E previous \in ReceiptRequests : request.id = previous.id /\ request # previous
FreshOrdering(request) ==
  \/ request.attempt = s.highAttempt /\ request.ordinal > s.highOrdinal
  \/ request.attempt > s.highAttempt /\ request.ordinal = 1
MutationAllowed(request) == ~IsMutation(request) \/ (~s.unknownMutation /\ ~s.knownDisposed)

Admit(request) ==
  s' = [s EXCEPT !.owner = request.owner, !.highAttempt = request.attempt,
    !.highOrdinal = request.ordinal, !.phase = "admitted", !.slot = request,
    !.last = "admit"]

AdmitFirstStep ==
  /\ s.phase = "empty" /\ s.owner = None
  /\ \E request \in Candidates :
    /\ request.owner = "A" /\ request.attempt = 1 /\ request.ordinal = 1
    /\ Admit(request)

AdmitNextStep ==
  /\ s.phase = "empty" /\ s.owner # None
  /\ \E request \in Candidates :
    /\ request.owner = s.owner /\ ~ExactReplay(request) /\ ~RetainedConflict(request)
    /\ FreshOrdering(request) /\ MutationAllowed(request)
    /\ Admit(request)

BeginHandoff ==
  /\ s.phase = "admitted"
  \* Never guard or clamp the counter at one: a second handoff must be observed
  \* as value two and fail the independent AtMostOneHostHandoffPerId invariant.
  /\ s' = [s EXCEPT !.phase = "handed", !.handoffs[s.slot.id] = @ + 1,
    !.history = @ \cup {[request |-> s.slot, unknownBefore |-> s.unknownMutation,
                         disposedBefore |-> s.knownDisposed]}, !.last = "handoff"]

Complete(outcome, label) ==
  LET receipt == [request |-> s.slot, outcome |-> outcome]
  IN s' = [s EXCEPT !.phase = "ready", !.result = receipt, !.lastStep = receipt,
    !.lastMutation = IF IsMutation(s.slot) THEN receipt ELSE @,
    !.unknownMutation = @ \/ (IsMutation(s.slot) /\ outcome = "unknown"),
    !.knownDisposed = @ \/ (s.slot.action = "dispose" /\ outcome = "disposed"),
    !.last = label]

CompleteKnownSignal ==
  /\ s.phase = "handed" /\ s.slot.action = "signal"
  /\ Complete("signal-ack", "signal-ack")
CompleteKnownDispose ==
  /\ s.phase = "handed" /\ s.slot.action = "dispose"
  /\ Complete("disposed", "disposed")
CompleteKnownQuery ==
  /\ s.phase = "handed" /\ s.slot.action = "query"
  /\ Complete("query-known", "query-known")
CompleteBlocked ==
  /\ s.phase = "handed" /\ Complete("blocked", "blocked")
CompletePreHandoffUnavailable ==
  /\ s.phase = "admitted" /\ Complete("unavailable", "unavailable")
CompleteMutationUnknown ==
  /\ s.phase = "handed" /\ IsMutation(s.slot)
  /\ Complete("unknown", "mutation-unknown")
CompleteQueryUnknown ==
  /\ s.phase = "handed" /\ ~IsMutation(s.slot)
  /\ Complete("unknown", "query-unknown")

DeliverResult ==
  /\ s.phase = "ready"
  /\ s' = [s EXCEPT !.phase = "empty", !.slot = None, !.result = None, !.last = "deliver"]

Replay(receipt, label) ==
  s' = [s EXCEPT !.phase = "ready", !.slot = receipt.request,
                 !.result = receipt, !.last = label]
ReplayLastStep ==
  /\ s.phase = "empty" /\ s.lastStep # None
  /\ Replay(s.lastStep, "replay-step")
ReplayLastMutationAfterQuery ==
  /\ s.phase = "empty" /\ s.lastMutation # None /\ s.lastStep # None
  /\ s.lastStep.request.action = "query"
  /\ Replay(s.lastMutation, "replay-mutation")
ReplayMaximumReceipt ==
  /\ s.phase = "empty" /\ s.lastStep # None
  /\ s.lastStep.request.attempt = MaxAttempt /\ s.lastStep.request.ordinal = MaxOrdinal
  /\ Replay(s.lastStep, "replay-maximum")

Reject(label) == s' = [s EXCEPT !.last = label]
RejectRetainedIdConflict ==
  /\ s.phase = "empty"
  /\ \E request \in Candidates : request.owner = s.owner /\ RetainedConflict(request)
  /\ Reject("reject-id-conflict")
RejectChangedAttemptWithRetainedId ==
  /\ s.phase = "empty"
  /\ \E previous \in ReceiptRequests : \E request \in ChangedAttempts(previous) :
       RetainedConflict(request)
  /\ Reject("reject-changed-attempt")
RejectChangedOrdinalWithRetainedId ==
  /\ s.phase = "empty"
  /\ \E previous \in ReceiptRequests : \E request \in ChangedOrdinals(previous) :
       RetainedConflict(request)
  /\ Reject("reject-changed-ordinal")
RejectOlderStep ==
  /\ s.phase = "empty" /\ s.owner # None
  /\ \E request \in Candidates :
       request.owner = s.owner /\ ~ExactReplay(request) /\ ~RetainedConflict(request)
       /\ ~FreshOrdering(request)
  /\ Reject("reject-older")
RejectDifferentOwner ==
  /\ s.phase = "empty" /\ s.owner # None
  /\ \E request \in Candidates : request.owner # s.owner
  /\ Reject("reject-owner")
RejectMutationAfterUnknown ==
  /\ s.phase = "empty" /\ s.unknownMutation
  /\ \E request \in Candidates :
       request.owner = s.owner /\ IsMutation(request) /\ ~ExactReplay(request)
       /\ ~RetainedConflict(request) /\ FreshOrdering(request)
  /\ Reject("reject-unknown")
RejectHigherAttemptMutationAfterUnknown ==
  /\ s.phase = "empty" /\ s.unknownMutation
  /\ \E request \in Candidates :
       request.owner = s.owner /\ IsMutation(request) /\ request.attempt > s.highAttempt
       /\ ~RetainedConflict(request) /\ FreshOrdering(request)
  /\ Reject("reject-unknown-higher-attempt")
RejectMutationAfterDisposed ==
  /\ s.phase = "empty" /\ s.knownDisposed
  /\ \E request \in Candidates :
       request.owner = s.owner /\ IsMutation(request) /\ ~ExactReplay(request)
       /\ ~RetainedConflict(request) /\ FreshOrdering(request)
  /\ Reject("reject-disposed")
RejectOccupiedSlot ==
  /\ s.phase # "empty" /\ Reject("reject-occupied")

ReadOriginalLaunchReplay ==
  /\ s.phase = "empty"
  /\ s' = [s EXCEPT !.phase = "original", !.slot = OriginalSlot,
    !.last = "original-replay"]
FinishOriginalLaunchReplay ==
  /\ s.phase = "original"
  /\ s' = [s EXCEPT !.phase = "empty", !.slot = None, !.last = "finish-original"]
RejectNewLaunchOrStart ==
  /\ s.owner # None /\ s.phase = "empty" /\ Reject("lifecycle-active")
ObserveStepExhaustion ==
  /\ s.phase = "empty" /\ s.lastStep # None /\ s.highOrdinal = MaxOrdinal
  /\ Reject("step-exhausted")
ObserveAttemptExhaustion ==
  /\ s.phase = "empty" /\ s.lastStep # None
  /\ s.highAttempt = MaxAttempt /\ s.highOrdinal = MaxOrdinal
  /\ Reject("attempt-exhausted")
ObserveKnownDisposedAfterQueryFailure ==
  /\ s.phase = "empty" /\ s.knownDisposed /\ s.lastStep # None
  /\ s.lastStep.request.action = "query" /\ s.lastStep.outcome = "unknown"
  /\ Reject("disposed-after-query-failure")

Next ==
  \/ AdmitFirstStep \/ AdmitNextStep \/ BeginHandoff
  \/ CompleteKnownSignal \/ CompleteKnownDispose \/ CompleteKnownQuery \/ CompleteBlocked
  \/ CompletePreHandoffUnavailable \/ CompleteMutationUnknown \/ CompleteQueryUnknown
  \/ DeliverResult \/ ReplayLastStep \/ ReplayLastMutationAfterQuery \/ ReplayMaximumReceipt
  \/ RejectRetainedIdConflict \/ RejectChangedAttemptWithRetainedId \/ RejectChangedOrdinalWithRetainedId
  \/ RejectOlderStep \/ RejectDifferentOwner \/ RejectMutationAfterUnknown
  \/ RejectHigherAttemptMutationAfterUnknown \/ RejectMutationAfterDisposed \/ RejectOccupiedSlot
  \/ ReadOriginalLaunchReplay \/ FinishOriginalLaunchReplay \/ RejectNewLaunchOrStart
  \/ ObserveStepExhaustion \/ ObserveAttemptExhaustion \/ ObserveKnownDisposedAfterQueryFailure

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.owner \in {None, "A"} /\ s.highAttempt \in 0..MaxAttempt /\ s.highOrdinal \in 0..MaxOrdinal
  /\ s.phase \in {"empty", "admitted", "handed", "ready", "original"}
  /\ s.slot \in {None, OriginalSlot} \cup RequestType
  /\ s.result \in {None} \cup ReceiptType /\ s.lastStep \in {None} \cup ReceiptType
  /\ s.lastMutation \in {None} \cup ReceiptType
  /\ s.unknownMutation \in BOOLEAN /\ s.knownDisposed \in BOOLEAN
  /\ s.handoffs \in [Ids -> 0..2]
  /\ s.history \subseteq [request : RequestType, unknownBefore : BOOLEAN, disposedBefore : BOOLEAN]
GlobalSlotIsStructuralSum ==
  /\ (s.phase = "empty" \equiv s.slot = None)
  /\ (s.phase = "original" \equiv s.slot = OriginalSlot)
  /\ (s.phase = "ready" \equiv s.result # None)
AtMostOneHostHandoffPerId == \A id \in Ids : s.handoffs[id] <= 1
UnknownNeverAllowsLaterMutation ==
  \A handoff \in s.history : handoff.unknownBefore => ~IsMutation(handoff.request)
DisposedNeverAllowsLaterMutation ==
  \A handoff \in s.history : handoff.disposedBefore => ~IsMutation(handoff.request)
KnownDisposeRequiresExactCompletedDispose ==
  s.knownDisposed => s.lastMutation # None /\ s.lastMutation.request.action = "dispose"
    /\ s.lastMutation.outcome = "disposed" /\ s.handoffs[s.lastMutation.request.id] = 1
UnknownRetainsExactMutation ==
  s.unknownMutation => s.lastMutation # None /\ IsMutation(s.lastMutation.request)
    /\ s.lastMutation.outcome = "unknown" /\ s.handoffs[s.lastMutation.request.id] = 1
CloseOwnerNeverChanges == [][s.owner # None => s'.owner = s.owner]_vars
HighWaterNeverRegresses ==
  [][s'.highAttempt > s.highAttempt \/
     (s'.highAttempt = s.highAttempt /\ s'.highOrdinal >= s.highOrdinal)]_vars
KnownFactsNeverRegress ==
  [][(s.knownDisposed => s'.knownDisposed) /\ (s.unknownMutation => s'.unknownMutation)]_vars
QueryNeverErasesMutationReceipt ==
  [][s.phase \in {"admitted", "handed"} /\ s.slot.action = "query" /\ s'.phase = "ready" => s'.lastMutation = s.lastMutation]_vars
ReplayNeverDispatches ==
  [][s'.last \in {"replay-step", "replay-mutation", "replay-maximum", "original-replay"} =>
     s'.handoffs = s.handoffs /\ s'.history = s.history]_vars
OriginalReplayPreservesClose ==
  [][s'.last = "original-replay" => s'.owner = s.owner /\ s'.lastStep = s.lastStep
     /\ s'.lastMutation = s.lastMutation /\ s'.knownDisposed = s.knownDisposed
     /\ s'.unknownMutation = s.unknownMutation]_vars
NewTargetEffectCannotResetClose ==
  [][s'.last = "lifecycle-active" => s'.owner = s.owner /\ s'.lastStep = s.lastStep
     /\ s'.lastMutation = s.lastMutation /\ s'.handoffs = s.handoffs]_vars
=============================================================================
