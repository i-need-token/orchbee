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
\*
\* An intact Host answer never retires the carrier: an answer that contradicts
\* the attach or the input settles that stage and the attachment is detached,
\* and a Detach answered otherwise than by the acknowledgement or a refusal is
\* left to a later cleanup on the kept carrier (deferred), which lies outside
\* this one-request model. Only a malformed envelope or a lost stream retires
\* the carrier. The Host refuses a Detach only for an attachment it no longer
\* holds, since its Detach is idempotent, so a refusal settles cleanup like
\* the acknowledgement: a trusted Host assumption, disclosed here.

Phases == {"idle", "attach-sent", "attached", "input-sent", "input-accepted",
           "input-rejected", "detach-pending", "detach-sent", "result-ready",
           "done", "closed"}
Outcomes == {"none", "rejected", "accepted", "unavailable", "unknown"}
RejectReasons == {"none", "attach", "attach-desync", "input"}
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
\* A stage has settled and the detach comes next: an accepted or refused
\* input, or a contradicting attach or input answer, or an unsent input record
\* (detach-pending, whose outcome is already set).
DetachPhases == {"input-accepted", "input-rejected", "detach-pending"}
BetweenPhases == {"idle", "attached"} \cup DetachPhases

VARIABLES payload, s
vars == <<payload, s>>
fixed == <<payload>>

Init ==
  /\ payload \in PayloadKinds
  /\ s = [phase |-> "idle", outcome |-> "none", rejectReason |-> "none",
          unavailReason |-> "none", cleanup |-> "none",
          acceptedBytes |-> {}, inputAcked |-> FALSE, detachAcked |-> FALSE,
          untrusted |-> FALSE, deferred |-> FALSE, hostAttachments |-> {"tui"},
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
  /\ s.phase \in DetachPhases
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
\* the attachment; a typed refusal means it never was. A desync, an intact
\* answer that grants something else, refuses the send and the attachment
\* asked for is detached, whether or not the Host applied it; a malformed
\* envelope or a lost stream retires the carrier.
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
  /\ s' = [s EXCEPT !.phase = "detach-pending", !.outcome = "rejected",
                    !.rejectReason = "attach-desync", !.last = "attach-desync"]
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

\* LocalFault before the attach record leaves: provably unsent, so nothing
\* needs cleanup and the carrier stays in step.
AttachLocalFault ==
  /\ s.phase = "idle" /\ s.carrierAlive
  /\ s' = [s EXCEPT !.phase = "result-ready", !.outcome = "unavailable",
                    !.unavailReason = "host-unavailable",
                    !.cleanup = "not-needed", !.last = "attach-local-fault"]
  /\ UNCHANGED fixed

\* ---------------------------------------------------------------------
\* Input stage answers. Exact requires the host to have applied the input;
\* a typed refusal means it never did. A desync, an intact answer that
\* contradicts the input, settles OutcomeUnknown and the attachment is
\* detached; a malformed envelope or a lost stream settles OutcomeUnknown and
\* retires the carrier without a detach. No input is ever sent again.
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
  /\ s' = [s EXCEPT !.phase = "detach-pending", !.outcome = "unknown",
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
\* so the outcome is Unavailable, not Unknown; the carrier stays in step and
\* the committed attachment is detached.
InputLocalFault ==
  /\ s.phase = "attached" /\ s.carrierAlive
  /\ s' = [s EXCEPT !.phase = "detach-pending", !.outcome = "unavailable",
                    !.unavailReason = "host-unavailable",
                    !.last = "input-local-fault"]
  /\ UNCHANGED fixed

\* ---------------------------------------------------------------------
\* Detach stage answers. The echoed DetachTerminalAccepted and a typed
\* refusal confirm cleanup: the Host holds the attachment no longer. Another
\* intact answer leaves it Unconfirmed and deferred to a later cleanup on the
\* kept carrier; a malformed envelope or a lost stream leaves it Unconfirmed
\* and retires the carrier.
\* ---------------------------------------------------------------------

DetachExact ==
  /\ s.phase = "detach-sent" /\ "send" \notin s.hostAttachments
  /\ s' = [s EXCEPT !.phase = "result-ready", !.cleanup = "confirmed",
                    !.detachAcked = TRUE, !.last = "detach-exact"]
  /\ UNCHANGED fixed

DetachRefused ==
  /\ s.phase = "detach-sent" /\ "send" \notin s.hostAttachments
  /\ s' = [s EXCEPT !.phase = "result-ready", !.cleanup = "confirmed",
                    !.detachAcked = TRUE, !.last = "detach-refused"]
  /\ UNCHANGED fixed

DetachMismatch ==
  /\ s.phase = "detach-sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.cleanup = "unconfirmed",
                    !.deferred = TRUE, !.last = "detach-mismatch"]
  /\ UNCHANGED fixed

DetachUnreachable ==
  /\ s.phase = "detach-sent"
  /\ s' = [s EXCEPT !.phase = "result-ready", !.cleanup = "unconfirmed",
                    !.carrierAlive = FALSE, !.teardownPending = TRUE,
                    !.last = "detach-unreachable"]
  /\ UNCHANGED fixed

\* LocalFault before the detach record leaves, or no room left for it in the
\* request: provably unsent (detachSent stays 0), so the carrier stays in step
\* and the attachment is deferred to a later cleanup -- the committed input
\* fact is preserved and the result is still produced.
DetachLocalFault ==
  /\ s.phase \in DetachPhases /\ s.carrierAlive
  /\ s' = [s EXCEPT !.phase = "result-ready", !.cleanup = "unconfirmed",
                    !.deferred = TRUE, !.last = "detach-local-fault"]
  /\ UNCHANGED fixed

\* ---------------------------------------------------------------------
\* Peer losses and local aborts.
\* ServerLost is phase-asymmetric: inside attach/input exchanges the error
\* propagates and the Worker dies with no result; once a stage has settled
\* the pending detach folds it into Unreachable and the result is still
\* produced -- delivery is then best-effort.
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
  /\ s.phase \in DetachPhases \cup {"detach-sent"}
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
\* settled stage's outcome preserved once it exists.
CarrierLost ==
  /\ s.carrierAlive /\ s.phase \in BetweenPhases
  /\ s' = [s EXCEPT !.phase = "result-ready",
                    !.outcome = IF s.phase \in DetachPhases
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
  \/ DetachExact \/ DetachRefused \/ DetachMismatch \/ DetachUnreachable
  \/ DetachLocalFault
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
  /\ \A field \in {"inputAcked", "detachAcked", "untrusted", "deferred",
       "hostInputApplied", "teardownPending", "carrierAlive", "serverAlive",
       "delivered"} :
       s[field] \in BOOLEAN

AttachAtMostOnce == s.attachSent <= 1
InputAtMostOnce == s.inputSent <= 1
DetachAtMostOnce == s.detachSent <= 1

\* outcome/cleanup pairings that cannot arise from attach -> input -> detach
\* are unreachable once the result is settled; Accepted + NotNeeded is
\* impossible because an accepted input required a real attachment, and
\* NotNeeded otherwise needs an attach that was refused or never sent. Cleanup
\* is "none" until the detach axis resolves, so the matrix applies only to
\* settled phases.
PairingMatrix ==
  s.phase \in {"result-ready", "done"} =>
    /\ s.outcome = "accepted" =>
         s.cleanup \in {"confirmed", "unconfirmed"} /\
         s.acceptedBytes \in SingletonAcks(payload)
    /\ s.outcome = "unknown" => s.cleanup \in {"confirmed", "unconfirmed"}
    /\ s.outcome = "unavailable" =>
         /\ s.unavailReason # "none"
         /\ s.cleanup = "not-needed" => s.attachSent = 0
    /\ s.cleanup = "not-needed" =>
         \/ s.outcome = "rejected" /\ s.rejectReason = "attach"
         \/ s.outcome = "unavailable" /\ s.attachSent = 0
    /\ s.outcome = "rejected" /\ s.rejectReason \in {"input", "attach-desync"} =>
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

\* Confirmed cleanup is never a false claim: the Host holds no send attachment.
ConfirmedMeansReleased == s.cleanup = "confirmed" => "send" \notin s.hostAttachments

AttachRefusalNeedsNoDetach ==
  s.rejectReason = "attach" => s.detachSent = 0 /\ s.cleanup = "not-needed" /\ s.carrierAlive

\* A malformed or lost stream never carries another record: no detach is
\* attempted and no input is re-sent.
UntrustedStreamNoDetach ==
  s.untrusted => s.detachSent = 0 /\ (s.phase = "input-sent" => s.inputSent <= 1)

\* An intact Host answer, or a record that never left, never retires the
\* carrier.
IntactAnswerKeepsCarrier ==
  s.last \in {"attach-desync", "attach-local-fault", "input-desync",
             "input-local-fault", "detach-refused", "detach-mismatch",
             "detach-local-fault"} => s.carrierAlive

\* Cleanup that was never confirmed is handed on: either the carrier retires
\* and the Host's teardown releases the attachment, or the kept carrier's
\* later cleanup detaches it.
UnconfirmedCleanupIsHandedOn ==
  s.phase = "done" /\ s.cleanup = "unconfirmed" => ~s.carrierAlive \/ s.deferred

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

\* A detach record is emitted only after a stage has settled one way or the
\* other -- never into an unresolved stream.
DetachOnlyAfterSettledStage ==
  [][s'.detachSent > s.detachSent => s.phase \in DetachPhases]_vars

\* An untrusted stream never gains another send of any kind, and an input
\* whose outcome is unknown is never sent again.
UnknownNeverResends ==
  [][/\ s.untrusted => s'.inputSent = s.inputSent /\ s'.detachSent = s.detachSent
     /\ s.outcome = "unknown" => s'.inputSent = s.inputSent]_vars

\* The accepted input fact, once observed, is never reverted.
AcceptedNeverDowngraded == [][s.inputAcked => s'.inputAcked]_vars

\* The acknowledged byte count is part of that fact: once inputAcked it is
\* immutable in every successor state.
AcceptedBytesStable ==
  [][s.inputAcked => s'.acceptedBytes = s.acceptedBytes]_vars

=============================================================================
