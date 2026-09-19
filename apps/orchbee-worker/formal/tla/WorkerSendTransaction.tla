--------------------- MODULE WorkerSendTransaction ---------------------
EXTENDS Naturals, FiniteSets, TLC

\* One already-admitted Server-authorized terminal send on the shared Host
\* carrier: attach -> one input record -> detach, each a fresh correlation,
\* one attempt per stage, no retry. The request pins a fixed target identity
\* and a Server-minted attachment id; the model keeps a single Target and
\* treats identity checks as exact. The pre-admission fence is a declared
\* exclusion: a deterministic local decision that emits zero Host bytes and
\* contains no interleaving, so its evidence stays with the scripted fence
\* test. Fresh-ID correlation distinctness is a disclosed oracle assumption
\* on the three draws (worker_identity::fresh_identity reads /dev/urandom),
\* not a collision-freedom proof; the attachment id is Server-minted and not
\* part of that assumption.
\*
\* Host apply and Worker-observed replies are independent: hostAttachments
\* and hostInputApplied advance through HostApply* actions and may interleave
\* with any response loss, so an applied-but-unacknowledged input is
\* reachable -- that reachability is what makes OutcomeUnknown honest rather
\* than assumed-unapplied. The existing TUI controller attachment ("tui")
\* shares the same carrier; carrier death legitimately releases it too --
\* fault sharing is real and must not be mis-proven as isolation.
\* host.stream().shutdown(Both) is only a local close; HostTeardown is the
\* separate Host-side event that eventually releases orphaned attachments.
\* Interleaved Host events drained by the exchange are legal stutters, never
\* errors, bounded by eventDrains.

Phases == {"idle", "attach-sent", "attached", "input-sent", "input-accepted",
           "input-rejected", "detach-sent", "result-ready", "done", "closed"}
Outcomes == {"none", "rejected", "accepted", "unavailable", "unknown"}
RejectReasons == {"none", "attach", "input"}
UnavailableReasons == {"none", "host-unavailable", "host-protocol-fault"}
Cleanups == {"none", "not-needed", "confirmed", "unconfirmed"}
ByteCounts == {0, 1, 2}
Attachments == {"tui", "send"}
PayloadKinds == {"text", "key"}

\* Exact echo acceptance by payload: text must echo its exact record length
\* (abstracted to 1), a semantic key may be ignored by mode (0) or count its
\* encoded bytes (1); 2 is over the negotiated maximum and desyncs.
ValidAcks(payload) == IF payload = "text" THEN {1} ELSE {0, 1}
SingletonAcks(payload) == { {b} : b \in ValidAcks(payload) }
SentPhases == {"attach-sent", "input-sent", "detach-sent"}
BetweenPhases == {"idle", "attached", "input-accepted", "input-rejected"}

VARIABLES payload, s
vars == <<payload, s>>
fixed == <<payload>>

Init ==
  /\ payload \in PayloadKinds
  /\ s = [phase |-> "idle", outcome |-> "none", rejectReason |-> "none",
          unavailReason |-> "none", cleanup |-> "none",
          acceptedBytes |-> {}, inputAcked |-> FALSE, detachAcked |-> FALSE,
          untrusted |-> FALSE, hostAttachments |-> {"tui"},
          hostInputApplied |-> FALSE, teardownPending |-> FALSE,
          carrierAlive |-> TRUE, serverAlive |-> TRUE,
          attachSent |-> 0, inputSent |-> 0, detachSent |-> 0,
          deliveries |-> 0, delivered |-> FALSE, eventDrains |-> 0,
          last |-> "init"]

\* ---------------------------------------------------------------------
\* Worker commands: each stage emits at most one record. Sends require a
\* live carrier and a live server -- a dead peer is settled by its own
\* loss action, matching exchange_fresh's prepare/wait peer checks.
\* ---------------------------------------------------------------------

SendAttach ==
  /\ s.phase = "idle" /\ s.carrierAlive /\ s.serverAlive
  /\ s' = [s EXCEPT !.phase = "attach-sent", !.attachSent = @ + 1,
                    !.last = "send-attach"]
  /\ UNCHANGED fixed

SendInput ==
  /\ s.phase = "attached" /\ s.carrierAlive /\ s.serverAlive
  /\ s' = [s EXCEPT !.phase = "input-sent", !.inputSent = @ + 1,
                    !.last = "send-input"]
  /\ UNCHANGED fixed

SendDetach ==
  /\ s.phase \in {"input-accepted", "input-rejected"}
  /\ s.carrierAlive /\ s.serverAlive
  /\ s' = [s EXCEPT !.phase = "detach-sent", !.detachSent = @ + 1,
                    !.last = "send-detach"]
  /\ UNCHANGED fixed

\* ---------------------------------------------------------------------
\* Host-side application, independent of any reply the Worker sees.
\* ---------------------------------------------------------------------

HostApplyAttach ==
  /\ s.phase = "attach-sent"
  /\ s' = [s EXCEPT !.hostAttachments = @ \cup {"send"},
                    !.last = "host-apply-attach"]
  /\ UNCHANGED fixed

HostApplyInput ==
  /\ s.phase = "input-sent" /\ ~s.hostInputApplied
  /\ s' = [s EXCEPT !.hostInputApplied = TRUE, !.last = "host-apply-input"]
  /\ UNCHANGED fixed

HostApplyDetach ==
  /\ s.phase = "detach-sent" /\ "send" \in s.hostAttachments
  /\ s' = [s EXCEPT !.hostAttachments = @ \ {"send"},
                    !.last = "host-apply-detach"]
  /\ UNCHANGED fixed

\* Carrier teardown on the Host side: the connection drop releases every
\* attachment the carrier carried, send's own and the TUI controller alike.
HostTeardown ==
  /\ s.teardownPending
  /\ s' = [s EXCEPT !.teardownPending = FALSE, !.hostAttachments = {},
                    !.last = "host-teardown"]
  /\ UNCHANGED fixed

\* ---------------------------------------------------------------------
\* Attach stage answers. An exact echo requires the host to have applied
\* the attachment; a typed refusal means it never was. Desync and malformed
\* envelopes are both protocol-class faults that retire the carrier.
\* ---------------------------------------------------------------------

AttachExact ==
  /\ s.phase = "attach-sent" /\ "send" \in s.hostAttachments
  /\ s' = [s EXCEPT !.phase = "attached", !.last = "attach-exact"]
  /\ UNCHANGED fixed

AttachRefused ==
  /\ s.phase = "attach-sent" /\ "send" \notin s.hostAttachments
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "rejected",
                    !.rejectReason = "attach", !.cleanup = "not-needed",
                    !.last = "attach-refused"]
  /\ UNCHANGED fixed

AttachDesync ==
  /\ s.phase = "attach-sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "unavailable",
                    !.unavailReason = "host-protocol-fault",
                    !.cleanup = "unconfirmed", !.untrusted = TRUE,
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "attach-desync"]
  /\ UNCHANGED fixed

AttachMalformed ==
  /\ s.phase = "attach-sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "unavailable",
                    !.unavailReason = "host-protocol-fault",
                    !.cleanup = "unconfirmed", !.untrusted = TRUE,
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "attach-malformed"]
  /\ UNCHANGED fixed

AttachLost ==
  /\ s.phase = "attach-sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "unavailable",
                    !.unavailReason = "host-unavailable",
                    !.cleanup = "unconfirmed", !.untrusted = TRUE,
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "attach-lost"]
  /\ UNCHANGED fixed

\* LocalFault before the attach record leaves: provably unsent, but the
\* carrier is still retired conservatively.
AttachLocalFault ==
  /\ s.phase = "idle" /\ s.carrierAlive
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "unavailable",
                    !.unavailReason = "host-unavailable",
                    !.cleanup = "unconfirmed", !.carrierAlive = FALSE,
                    !.teardownPending = TRUE, !.last = "attach-local-fault"]
  /\ UNCHANGED fixed

\* ---------------------------------------------------------------------
\* Input stage answers. Exact requires the host to have applied the input;
\* a typed refusal means it never did. Everything uncertain settles
\* OutcomeUnknown and retires the carrier without a detach.
\* ---------------------------------------------------------------------

InputExact(bytes) ==
  /\ s.phase = "input-sent" /\ s.hostInputApplied /\ bytes \in ValidAcks(payload)
  /\ s' = [s EXCEPT !.phase = "input-accepted", !.outcome = "accepted",
                    !.acceptedBytes = {bytes}, !.inputAcked = TRUE,
                    !.last = "input-exact"]
  /\ UNCHANGED fixed

InputRefused ==
  /\ s.phase = "input-sent" /\ ~s.hostInputApplied
  /\ s' = [s EXCEPT !.phase = "input-rejected", !.outcome = "rejected",
                    !.rejectReason = "input", !.last = "input-refused"]
  /\ UNCHANGED fixed

InputDesync ==
  /\ s.phase = "input-sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "unknown",
                    !.cleanup = "unconfirmed", !.untrusted = TRUE,
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "input-desync"]
  /\ UNCHANGED fixed

InputMalformed ==
  /\ s.phase = "input-sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "unknown",
                    !.cleanup = "unconfirmed", !.untrusted = TRUE,
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "input-malformed"]
  /\ UNCHANGED fixed

InputLost ==
  /\ s.phase = "input-sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "unknown",
                    !.cleanup = "unconfirmed", !.untrusted = TRUE,
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "input-lost"]
  /\ UNCHANGED fixed

\* LocalFault before the input record leaves: the input is provably unsent,
\* so the outcome is Unavailable, not Unknown -- but the already-committed
\* attachment makes the carrier unrecoverable, so it still retires.
InputLocalFault ==
  /\ s.phase = "attached" /\ s.carrierAlive
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "unavailable",
                    !.unavailReason = "host-unavailable",
                    !.cleanup = "unconfirmed", !.carrierAlive = FALSE,
                    !.teardownPending = TRUE, !.last = "input-local-fault"]
  /\ UNCHANGED fixed

\* ---------------------------------------------------------------------
\* Detach stage answers. Only a positively echoed DetachTerminalAccepted
\* confirms cleanup; a typed refusal and every uncertain outcome are both
\* Unconfirmed and both retire the carrier -- refused stays a distinct
\* action name because it is a legible typed failure, not stream decay.
\* ---------------------------------------------------------------------

DetachExact ==
  /\ s.phase = "detach-sent" /\ "send" \notin s.hostAttachments
  /\ s' = [s EXCEPT !.phase = "result-ready", !.cleanup = "confirmed",
                    !.detachAcked = TRUE, !.last = "detach-exact"]
  /\ UNCHANGED fixed

DetachRefused ==
  /\ s.phase = "detach-sent" /\ "send" \in s.hostAttachments
  /\ s' = [s EXCEPT !.phase = "result-ready", !.cleanup = "unconfirmed",
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "detach-refused"]
  /\ UNCHANGED fixed

DetachUnreachable ==
  /\ s.phase = "detach-sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.cleanup = "unconfirmed",
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "detach-unreachable"]
  /\ UNCHANGED fixed

\* LocalFault before the detach record leaves: provably unsent (detachSent
\* stays 0), folded into the same unreachable cleanup settlement -- the
\* committed input fact is preserved and the result is still produced.
DetachLocalFault ==
  /\ s.phase \in {"input-accepted", "input-rejected"} /\ s.carrierAlive
  /\ s' = [s EXCEPT !.phase = "result-ready", !.cleanup = "unconfirmed",
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "detach-local-fault"]
  /\ UNCHANGED fixed

\* ---------------------------------------------------------------------
\* Peer losses and local aborts.
\* ServerLost is phase-asymmetric: inside attach/input exchanges the error
\* propagates and the Worker dies with no result; from a committed input
\* fact onward the pending detach folds it into Unreachable and the result
\* is still produced -- delivery is then best-effort.
\* ---------------------------------------------------------------------

ServerLostBeforeInputFact ==
  /\ s.serverAlive
  /\ s.phase \in {"idle", "attach-sent", "attached", "input-sent"}
  /\ s' = [s EXCEPT !.phase = "closed", !.serverAlive = FALSE,
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "server-lost-early"]
  /\ UNCHANGED fixed

ServerLostAfterInputFact ==
  /\ s.serverAlive
  /\ s.phase \in {"input-accepted", "input-rejected", "detach-sent"}
  /\ s' = [s EXCEPT !.phase = "result-ready", !.serverAlive = FALSE,
                    !.cleanup = "unconfirmed", !.carrierAlive = FALSE,
                    !.teardownPending = TRUE, !.last = "server-lost-late"]
  /\ UNCHANGED fixed

ServerLostAtResult ==
  /\ s.serverAlive /\ s.phase = "result-ready"
  /\ s' = [s EXCEPT !.serverAlive = FALSE, !.last = "server-lost-result"]
  /\ UNCHANGED fixed

\* Carrier death between exchanges is discovered at the next write; the
\* settlement equals that stage's transport-loss outcome: Unavailable before
\* any record left, Unknown while the input was still unwritten, and the
\* committed input fact preserved once it exists.
CarrierLost ==
  /\ s.carrierAlive /\ s.phase \in BetweenPhases
  /\ s' = [s EXCEPT !.phase = "result-ready",
                    !.outcome = IF s.phase \in {"input-accepted", "input-rejected"}
                                  THEN s.outcome
                                  ELSE IF s.phase = "idle"
                                         THEN "unavailable" ELSE "unknown",
                    !.unavailReason = IF s.phase = "idle"
                                        THEN "host-unavailable" ELSE @,
                    !.cleanup = "unconfirmed", !.untrusted = TRUE,
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "carrier-lost"]
  /\ UNCHANGED fixed

\* The deterministic post-attach abort: a `?` propagation after a committed
\* attach kills the Worker with no result and no detach; the attachment is
\* released only when the dead connection tears down.
PostAttachAbort ==
  /\ s.phase = "attached"
  /\ s' = [s EXCEPT !.phase = "closed", !.carrierAlive = FALSE,
                    !.serverAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "post-attach-abort"]
  /\ UNCHANGED fixed

\* Legal Host events interleaved inside an exchange are drained, bounded,
\* and never an error.
DrainHostEvent ==
  /\ s.phase \in SentPhases /\ s.eventDrains < 3
  /\ s' = [s EXCEPT !.eventDrains = @ + 1, !.last = "drain-event"]
  /\ UNCHANGED fixed

\* ---------------------------------------------------------------------
\* Result emission: the record is produced either way once result-ready is
\* reached; delivery succeeds only while the Server link still stands.
\* ---------------------------------------------------------------------

DeliverResult ==
  /\ s.phase = "result-ready" /\ s.serverAlive
  /\ s' = [s EXCEPT !.phase = "done", !.deliveries = @ + 1,
                    !.delivered = TRUE, !.last = "deliver-result"]
  /\ UNCHANGED fixed

AbandonResult ==
  /\ s.phase = "result-ready" /\ ~s.serverAlive
  /\ s' = [s EXCEPT !.phase = "done", !.deliveries = @ + 1,
                    !.last = "abandon-result"]
  /\ UNCHANGED fixed

Next ==
  \/ SendAttach \/ SendInput \/ SendDetach
  \/ HostApplyAttach \/ HostApplyInput \/ HostApplyDetach \/ HostTeardown
  \/ AttachExact \/ AttachRefused \/ AttachDesync \/ AttachMalformed
  \/ AttachLost \/ AttachLocalFault
  \/ (\E bytes \in {0, 1, 2} : InputExact(bytes)) \/ InputRefused
  \/ InputDesync \/ InputMalformed \/ InputLost \/ InputLocalFault
  \/ DetachExact \/ DetachRefused \/ DetachUnreachable \/ DetachLocalFault
  \/ ServerLostBeforeInputFact \/ ServerLostAfterInputFact \/ ServerLostAtResult
  \/ CarrierLost \/ PostAttachAbort \/ DrainHostEvent
  \/ DeliverResult \/ AbandonResult

\* Weak fairness over the whole step: every continuously enabled resolution
\* -- reply, expiry, loss, or local settlement -- may fire; no peer is
\* assumed to answer successfully.
Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

\* ---------------------------------------------------------------------
\* Safety
\* ---------------------------------------------------------------------

TypeOK ==
  /\ payload \in PayloadKinds
  /\ s.phase \in Phases /\ s.outcome \in Outcomes
  /\ s.rejectReason \in RejectReasons
  /\ s.unavailReason \in UnavailableReasons /\ s.cleanup \in Cleanups
  /\ s.acceptedBytes \in SUBSET ByteCounts
  /\ s.hostAttachments \subseteq Attachments
  \* Counters are real occurrence counts (@ + 1 at emit), not occurred bits;
  \* the domain deliberately includes 2 so a second emission is representable
  \* and caught by the named AtMostOnce / ResultProducedOnce properties, not
  \* by the type domain.
  /\ s.attachSent \in 0..2 /\ s.inputSent \in 0..2 /\ s.detachSent \in 0..2
  /\ s.deliveries \in 0..2 /\ s.eventDrains \in 0..3
  /\ \A field \in {"inputAcked", "detachAcked", "untrusted", "hostInputApplied",
       "teardownPending", "carrierAlive", "serverAlive", "delivered"} :
       s[field] \in BOOLEAN

AttachAtMostOnce == s.attachSent <= 1
InputAtMostOnce == s.inputSent <= 1
DetachAtMostOnce == s.detachSent <= 1

\* outcome/cleanup pairings that cannot arise from attach -> input -> detach
\* are unreachable once the result is settled; Accepted + NotNeeded is
\* impossible because an accepted input required a real attachment. Cleanup
\* is "none" until the detach axis resolves, so the matrix applies only to
\* settled phases.
PairingMatrix ==
  s.phase \in {"result-ready", "done"} =>
    /\ s.outcome = "accepted" =>
         s.cleanup \in {"confirmed", "unconfirmed"} /\
         s.acceptedBytes \in SingletonAcks(payload)
    /\ s.outcome = "unknown" => s.cleanup = "unconfirmed"
    /\ s.outcome = "unavailable" =>
         s.cleanup = "unconfirmed" /\ s.unavailReason # "none"
    /\ s.cleanup = "not-needed" => s.outcome = "rejected" /\ s.rejectReason = "attach"
    /\ s.outcome = "rejected" /\ s.rejectReason = "input" =>
         s.cleanup \in {"confirmed", "unconfirmed"}

\* Once the input fact is committed it survives every later cleanup outcome.
AcceptedFactSurvivesCleanup ==
  s.inputAcked /\ s.phase \in {"result-ready", "done"} =>
    s.outcome = "accepted" /\ s.acceptedBytes \in SingletonAcks(payload)

\* The accepted byte count is always a singleton drawn from ValidAcks: key 0
\* is legal, text must be exact, and an over-max count is never accepted.
AcceptedBytesMatchesPayload ==
  s.inputAcked => s.acceptedBytes \in SingletonAcks(payload)

ConfirmedRequiresDetachAck == s.cleanup = "confirmed" => s.detachAcked

AttachRefusalNeedsNoDetach ==
  s.rejectReason = "attach" => s.detachSent = 0 /\ s.cleanup = "not-needed" /\ s.carrierAlive

\* A desynchronized, malformed, or lost stream never carries another record:
\* no detach is attempted and no input is re-sent.
UntrustedStreamNoDetach ==
  s.untrusted => s.detachSent = 0 /\ (s.phase = "input-sent" => s.inputSent <= 1)

\* Cleanup that was never confirmed retires the carrier: the result is the
\* last record this Host session produces.
UnconfirmedCleanupClosesCarrier ==
  s.phase = "done" /\ s.cleanup = "unconfirmed" => ~s.carrierAlive

\* The TUI controller's authorization is never taken by the send attachment;
\* only carrier death can release it.
TuiLossOnlyViaCarrierDeath == "tui" \notin s.hostAttachments => ~s.carrierAlive

\* The Host cannot hold the send attachment without our attach record.
SendAttachmentMatchesAdmission == "send" \in s.hostAttachments => s.attachSent = 1

\* Early Server loss propagates as an error: the Worker dies having produced
\* no result at all.
NoResultOnEarlyServerLoss ==
  s.phase = "closed" => s.outcome = "none" /\ s.deliveries = 0 /\ ~s.delivered

ResultProducedOnce == s.deliveries <= 1

\* ---------------------------------------------------------------------
\* Progress
\* ---------------------------------------------------------------------

EventualSettlement == <>(s.phase \in {"done", "closed"})

TeardownEventuallyReleasesAttachments ==
  s.teardownPending ~> (s.hostAttachments = {})

\* A detach record is emitted only after the input fact is committed one
\* way or the other -- never into an unresolved stream.
DetachOnlyAfterCommittedInputFact ==
  [][s'.detachSent > s.detachSent => s.phase \in {"input-accepted", "input-rejected"}]_vars

\* An untrusted stream never gains another send of any kind.
UnknownNeverResends ==
  [][s.untrusted => s'.inputSent = s.inputSent /\ s'.detachSent = s.detachSent]_vars

\* The accepted input fact, once observed, is never reverted.
AcceptedNeverDowngraded == [][s.inputAcked => s'.inputAcked]_vars

\* The acknowledged byte count is part of that fact: once inputAcked it is
\* immutable in every successor state.
AcceptedBytesStable ==
  [][s.inputAcked => s'.acceptedBytes = s.acceptedBytes]_vars

=============================================================================
