---------------------- MODULE TerminalHostRequestOutcome ---------------------
EXTENDS Naturals, TLC

\* Refines runtime-model claims TH-005, TH-007, and TH-010. Prepare, Commit,
\* HandoffAppliedReply, Disconnect, and ReplayIdempotent distinguish rejected,
\* applied, replayed, and unknown outcomes without duplicating an effect. The
\* prepared state includes the encoded reply and its reserved egress capacity;
\* Commit therefore introduces no later domain allocation.

CONSTANTS Requests

RequestKinds == {"none", "idempotent", "non_replayable"}
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
  requestKind,
  requestState,
  effectCount,
  replyKind

requestVars == <<requestKind, requestState, effectCount, replyKind>>
vars == <<brokerActive, requestVars>>

Init ==
  /\ brokerActive = FALSE
  /\ requestKind = [request \in Requests |-> "none"]
  /\ requestState = [request \in Requests |-> "unseen"]
  /\ effectCount = [request \in Requests |-> 0]
  /\ replyKind = [request \in Requests |-> "none"]

Connect ==
  /\ ~brokerActive
  /\ brokerActive' = TRUE
  /\ UNCHANGED requestVars

Prepare(request, kind) ==
  /\ brokerActive
  /\ request \in Requests
  /\ kind \in {"idempotent", "non_replayable"}
  /\ requestState[request] = "unseen"
  /\ requestKind' = [requestKind EXCEPT ![request] = kind]
  /\ requestState' = [requestState EXCEPT ![request] = "prepared"]
  /\ UNCHANGED <<brokerActive, effectCount, replyKind>>

AbortBeforeCommit(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "prepared"
  /\ requestState' =
       [requestState EXCEPT ![request] = "retryable_rejected"]
  /\ UNCHANGED <<brokerActive, requestKind, effectCount, replyKind>>

RetryRejected(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "retryable_rejected"
  /\ requestState' = [requestState EXCEPT ![request] = "prepared"]
  /\ UNCHANGED <<brokerActive, requestKind, effectCount, replyKind>>

Commit(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "prepared"
  /\ requestState' = [requestState EXCEPT ![request] = "committed"]
  /\ effectCount' = [effectCount EXCEPT ![request] = 1]
  /\ UNCHANGED <<brokerActive, requestKind, replyKind>>

HandoffAppliedReply(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "committed"
  /\ requestState' =
       [requestState EXCEPT ![request] = "reply_handed"]
  /\ replyKind' = [replyKind EXCEPT ![request] = "applied"]
  /\ UNCHANGED <<brokerActive, requestKind, effectCount>>

ConfirmReply(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "reply_handed"
  /\ requestState' = [requestState EXCEPT ![request] = "confirmed"]
  /\ UNCHANGED <<brokerActive, requestKind, effectCount, replyKind>>

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
  /\ UNCHANGED <<requestKind, effectCount>>

ReplayIdempotent(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "unknown"
  /\ requestKind[request] = "idempotent"
  /\ requestState' =
       [requestState EXCEPT ![request] = "reply_handed"]
  /\ replyKind' = [replyKind EXCEPT ![request] = "replayed"]
  /\ UNCHANGED <<brokerActive, requestKind, effectCount>>

ReplayConfirmedIdempotent(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "confirmed"
  /\ requestKind[request] = "idempotent"
  /\ requestState' =
       [requestState EXCEPT ![request] = "reply_handed"]
  /\ replyKind' = [replyKind EXCEPT ![request] = "replayed"]
  /\ UNCHANGED <<brokerActive, requestKind, effectCount>>

RejectNonReplayableRetry(request) ==
  /\ brokerActive
  /\ request \in Requests
  /\ requestState[request] = "unknown"
  /\ requestKind[request] = "non_replayable"
  /\ UNCHANGED vars

Next ==
  \/ Connect
  \/ \E request \in Requests,
        kind \in {"idempotent", "non_replayable"}:
       Prepare(request, kind)
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
  /\ requestKind \in [Requests -> RequestKinds]
  /\ requestState \in [Requests -> RequestStates]
  /\ effectCount \in [Requests -> 0..1]
  /\ replyKind \in [Requests -> ReplyKinds]

OutcomeIntegrity ==
  \A request \in Requests:
    /\ (requestState[request] = "unseen") <=>
         (requestKind[request] = "none")
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

UnknownRequiresCommittedEffect ==
  \A request \in Requests:
    requestState[request] = "unknown" => effectCount[request] = 1

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
