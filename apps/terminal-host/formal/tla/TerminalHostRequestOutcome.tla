---------------------- MODULE TerminalHostRequestOutcome ---------------------
EXTENDS Naturals, TLC

\* Refines runtime-model claims TH-005, TH-007, TH-010, and TH-016. Prepare, Commit,
\* HandoffAppliedReply, Disconnect, and ReplayIdempotent distinguish rejected,
\* applied, replayed, and unknown outcomes without duplicating an effect. The
\* prepared state includes the encoded reply and its reserved egress capacity;
\* Commit therefore introduces no later domain allocation. Prepare binds the
\* concrete clear and reset operations to the non_replayable class, so an
\* unknown committed result remains stable and can never enter
\* ReplayIdempotent.

CONSTANTS Requests

RequestKinds == {"none", "idempotent", "non_replayable"}
RequestOperations == {"none", "clear", "reset", "idempotent_query"}
RequestStates == {
  "unseen",
  "prepared",
  "retryable_rejected",
  "committed",
  "reply_handed",
  "confirmed",
  "unknown"
}
ReplyKinds == {"none", "applied", "replayed"}

VARIABLES
  brokerActive,
  requestOperation,
  requestKind,
  requestState,
  effectCount,
  replyKind

requestVars ==
  <<requestOperation, requestKind, requestState, effectCount, replyKind>>
vars == <<brokerActive, requestVars>>

Init ==
  /\ brokerActive = FALSE
  /\ requestOperation = [request \in Requests |-> "none"]
  /\ requestKind = [request \in Requests |-> "none"]
  /\ requestState = [request \in Requests |-> "unseen"]
  /\ effectCount = [request \in Requests |-> 0]
  /\ replyKind = [request \in Requests |-> "none"]

Connect ==
  /\ ~brokerActive
  /\ brokerActive' = TRUE
  /\ UNCHANGED requestVars

RetryClass(operation) ==
  IF operation \in {"clear", "reset"}
    THEN "non_replayable"
    ELSE "idempotent"

Prepare(request, operation) ==
  /\ brokerActive
  /\ request \in Requests
  /\ operation \in RequestOperations \ {"none"}
  /\ requestState[request] = "unseen"
  /\ requestOperation' =
       [requestOperation EXCEPT ![request] = operation]
  /\ requestKind' =
       [requestKind EXCEPT ![request] = RetryClass(operation)]
  /\ requestState' = [requestState EXCEPT ![request] = "prepared"]
  /\ UNCHANGED <<brokerActive, effectCount, replyKind>>

AbortBeforeCommit(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "prepared"
  /\ requestState' =
       [requestState EXCEPT ![request] = "retryable_rejected"]
  /\ UNCHANGED <<brokerActive, requestOperation, requestKind, effectCount,
                  replyKind>>

RetryRejected(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "retryable_rejected"
  /\ requestState' = [requestState EXCEPT ![request] = "prepared"]
  /\ UNCHANGED <<brokerActive, requestOperation, requestKind, effectCount,
                  replyKind>>

Commit(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "prepared"
  /\ requestState' = [requestState EXCEPT ![request] = "committed"]
  /\ effectCount' = [effectCount EXCEPT ![request] = 1]
  /\ UNCHANGED <<brokerActive, requestOperation, requestKind, replyKind>>

HandoffAppliedReply(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "committed"
  /\ requestState' =
       [requestState EXCEPT ![request] = "reply_handed"]
  /\ replyKind' = [replyKind EXCEPT ![request] = "applied"]
  /\ UNCHANGED <<brokerActive, requestOperation, requestKind, effectCount>>

ConfirmReply(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "reply_handed"
  /\ requestState' = [requestState EXCEPT ![request] = "confirmed"]
  /\ UNCHANGED <<brokerActive, requestOperation, requestKind, effectCount,
                  replyKind>>

Disconnect ==
  /\ brokerActive
  /\ brokerActive' = FALSE
  /\ requestState' = [request \in Requests |->
       IF requestState[request] = "prepared"
       THEN "retryable_rejected"
       ELSE IF requestState[request] \in {"committed", "reply_handed"}
       THEN "unknown"
       ELSE requestState[request]]
  /\ replyKind' = [request \in Requests |->
       IF requestState[request] \in {"committed", "reply_handed"}
       THEN "none"
       ELSE replyKind[request]]
  /\ UNCHANGED <<requestOperation, requestKind, effectCount>>

ReplayIdempotent(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "unknown"
  /\ requestKind[request] = "idempotent"
  /\ requestState' =
       [requestState EXCEPT ![request] = "reply_handed"]
  /\ replyKind' = [replyKind EXCEPT ![request] = "replayed"]
  /\ UNCHANGED <<brokerActive, requestOperation, requestKind, effectCount>>

ReplayConfirmedIdempotent(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "confirmed"
  /\ requestKind[request] = "idempotent"
  /\ requestState' =
       [requestState EXCEPT ![request] = "reply_handed"]
  /\ replyKind' = [replyKind EXCEPT ![request] = "replayed"]
  /\ UNCHANGED <<brokerActive, requestOperation, requestKind, effectCount>>

RejectNonReplayableRetry(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "unknown"
  /\ requestKind[request] = "non_replayable"
  /\ UNCHANGED vars

Next ==
  \/ Connect
  \/ \E request \in Requests,
        operation \in RequestOperations \ {"none"}:
       Prepare(request, operation)
  \/ \E request \in Requests: AbortBeforeCommit(request)
  \/ \E request \in Requests: RetryRejected(request)
  \/ \E request \in Requests: Commit(request)
  \/ \E request \in Requests: HandoffAppliedReply(request)
  \/ \E request \in Requests: ConfirmReply(request)
  \/ Disconnect
  \/ \E request \in Requests: ReplayIdempotent(request)
  \/ \E request \in Requests: ReplayConfirmedIdempotent(request)
  \/ \E request \in Requests: RejectNonReplayableRetry(request)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ brokerActive \in BOOLEAN
  /\ requestOperation \in [Requests -> RequestOperations]
  /\ requestKind \in [Requests -> RequestKinds]
  /\ requestState \in [Requests -> RequestStates]
  /\ effectCount \in [Requests -> 0..1]
  /\ replyKind \in [Requests -> ReplyKinds]

OutcomeIntegrity ==
  \A request \in Requests:
    /\ (requestState[request] = "unseen") <=>
         (requestKind[request] = "none")
    /\ (requestState[request] = "unseen") <=>
         (requestOperation[request] = "none")
    /\ requestState[request] \in {
         "unseen", "prepared", "retryable_rejected"
       } => effectCount[request] = 0
    /\ requestState[request] \in {
         "committed", "reply_handed", "confirmed", "unknown"
       } => effectCount[request] = 1
    /\ requestState[request] \in {
         "unseen", "prepared", "retryable_rejected", "committed", "unknown"
       } => replyKind[request] = "none"
    /\ replyKind[request] = "replayed" =>
         /\ requestKind[request] = "idempotent"
         /\ requestState[request] \in {"reply_handed", "confirmed"}

ClearAndResetAreNonReplayable ==
  \A request \in Requests:
    requestOperation[request] \in {"clear", "reset"} =>
      requestKind[request] = "non_replayable"

ClearAndResetUnknownCannotReplay ==
  \A request \in Requests:
    (requestOperation[request] \in {"clear", "reset"} /\
     requestState[request] = "unknown") =>
      ~ENABLED ReplayIdempotent(request)

NonReplayableUnknownIsStable ==
  [][\A request \in Requests:
       (/\ requestState[request] = "unknown"
        /\ requestKind[request] = "non_replayable")
       => /\ requestState'[request] = "unknown"
          /\ requestKind'[request] = "non_replayable"
          /\ effectCount'[request] = effectCount[request]
          /\ replyKind'[request] = "none"]_vars

ReplayNeverDuplicatesEffect ==
  [][\A request \in Requests:
       (ReplayIdempotent(request) \/ ReplayConfirmedIdempotent(request)) =>
         UNCHANGED effectCount]_vars

DisconnectNeverRollsBackEffect ==
  [][Disconnect => UNCHANGED effectCount]_vars

=============================================================================
