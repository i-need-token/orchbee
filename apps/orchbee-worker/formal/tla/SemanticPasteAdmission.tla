--------------------- MODULE SemanticPasteAdmission ---------------------
EXTENDS FiniteSets, Naturals, TLC

\* Companion model for the semantic-paste admission chain authorized by
\* packet-06. One bounded raw-byte record crosses Server (request record),
\* Worker (grant admission plus a single Host handoff), and Host
\* (preparation snapshots the bracketed-paste mode under the terminal
\* mutex; commit publishes under the separate input-admission lock after
\* Core re-authorization -- no single mutex covers the whole span). The
\* model describes the request chain on one already-established carrier:
\* a partial or malformed record produces no request and enters the
\* terminal "failed" phase, matching the implementation where the carrier
\* fails closed on the connection rather than realigning the byte stream;
\* a complete record followed by the next record on the surviving carrier
\* is modeled only as the idle -> record-open transition -- byte-cursor
\* alignment is owned by codec tests and is not simulated here.
\* Admission consumes the sequence once and a consumed sequence is never
\* rolled back, so a replayed sequence is rejected rather than
\* re-handed-off; preparation holds a bounded reservation and cannot
\* produce an effect by itself; commit is the only queue effect and still
\* re-checks grant and terminal liveness; a definite rejection publishes
\* no effect; a lost or misclassified Host response settles this request
\* as unknown, retires the carrier, and is never retried or fabricated
\* into a definite outcome. Byte budgets, identity widths, and the four
\* typed rejection codes are conformance-tested in the owning crates;
\* this model keeps them abstract.
\*
\* Abstraction boundaries, stated honestly:
\* - Two-level sequence: Server's per-controller sequence and Worker's
\*   per-grant sequence are folded into one grantSequence; monotonic
\*   consumption and no-rollback survive the fold.
\* - Static grant set: Grants is fixed at Init; real grant
\*   acquire/replace cycles are outside scope.
\* - terminalAlive folds child exit, writer stop, failure fencing, and
\*   client input closure into one bit.
\* - Wrong/lost response: this request settles as unknown and the carrier
\*   enters "failed"; real broker retirement, replacement-carrier
\*   activation, and cross-request/incarnation cleanup are owned by
\*   LocalWorkerObcnCarrier.tla and LocalHostBroker.tla and their
\*   worker_obcn_carrier / process tests. This model does not prove any
\*   incarnation or broker-retirement lifecycle.
\* - No fairness is asserted. ReservationReleasedOnce is a transition
\*   safety property (the held state is left only through commit or
\*   cancel), not an eventual-release liveness claim.
\*   SettledRequestHasOutcome states only that a settled request has a
\*   non-"none" outcome; it is not by itself an exact-once proof.
\*   Exactly-once sequence consumption follows the unidirectional phase
\*   graph: request-ready -> admitted is the only transition that writes
\*   grantSequence, and no transition returns a request to record-open or
\*   request-ready; only RetireRequest resets the request fields for a
\*   fresh record on the surviving carrier.

CONSTANTS Grants, NoGrant, MaxSequence, QueueCapacity

GrantValues == Grants \cup {NoGrant}
\* Each grant may consume up to MaxSequence distinct sequence values, so the
\* number of admitted requests across all grants is bounded by their product.
MaxRequests == Cardinality(Grants) * MaxSequence
Modes == {"bracketed", "plain"}
ModeValues == Modes \cup {"none"}
Phases == {
  "idle", "record-open", "request-ready", "admitted", "handed",
  "prepared", "responded", "settled", "failed"
}
Responses == {"none", "accepted", "rejected"}
Outcomes == {"none", "accepted", "rejected", "unknown"}
Actions == {
  "init", "begin-paste-record", "abandon-partial-record",
  "complete-paste-record", "admit-paste", "reject-paste-admission",
  "handoff-to-host", "prepare-paste", "prepare-fails",
  "commit-prepared-paste", "cancel-prepared-paste",
  "flip-bracketed-mode", "revoke-grant", "kill-terminal",
  "drain-queue", "deliver-response", "lose-response",
  "deliver-wrong-response", "retire-request"
}

VARIABLES
  phase,
  grantAlive,
  grantSequence,
  requestGrant,
  requestSequence,
  requestHandoffs,
  requestCommitted,
  terminalAlive,
  liveMode,
  preparedMode,
  committedMode,
  reservation,
  queueUsed,
  totalHandoffs,
  totalCommits,
  response,
  outcome,
  protocolViolation,
  priorGrantSequence,
  priorPreparedMode,
  priorReservation,
  priorRequestHandoffs,
  priorTotalCommits,
  lastAction

DomainVars == <<
  phase, grantAlive, grantSequence, requestGrant, requestSequence,
  requestHandoffs, requestCommitted, terminalAlive, liveMode,
  preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
  totalCommits, response, outcome, protocolViolation
>>

AuditVars == <<
  priorGrantSequence, priorPreparedMode, priorReservation,
  priorRequestHandoffs, priorTotalCommits, lastAction
>>

vars == <<DomainVars, AuditVars>>

Record(action) ==
  /\ priorGrantSequence' = grantSequence
  /\ priorPreparedMode' = preparedMode
  /\ priorReservation' = reservation
  /\ priorRequestHandoffs' = requestHandoffs
  /\ priorTotalCommits' = totalCommits
  /\ lastAction' = action

Init ==
  /\ phase = "idle"
  /\ grantAlive = [grant \in Grants |-> TRUE]
  /\ grantSequence = [grant \in Grants |-> 0]
  /\ requestGrant = NoGrant
  /\ requestSequence = 0
  /\ requestHandoffs = 0
  /\ requestCommitted = FALSE
  /\ terminalAlive = TRUE
  /\ liveMode = "plain"
  /\ preparedMode = "none"
  /\ committedMode = "none"
  /\ reservation = "none"
  /\ queueUsed = 0
  /\ totalHandoffs = 0
  /\ totalCommits = 0
  /\ response = "none"
  /\ outcome = "none"
  /\ protocolViolation = FALSE
  /\ priorGrantSequence = grantSequence
  /\ priorPreparedMode = preparedMode
  /\ priorReservation = reservation
  /\ priorRequestHandoffs = requestHandoffs
  /\ priorTotalCommits = totalCommits
  /\ lastAction = "init"

\* --- Bounded record framing -------------------------------------------
\* A record-open phase is the partially read prefix of one bounded OBCQ or
\* OBNQ record. While it is open no request exists. Abandoning it enters
\* the terminal "failed" phase: the real carrier fails closed on the
\* connection, so no further record is admitted on this request path.
\* Successful completion of a record leaves the carrier alive and a later
\* record re-enters at idle -- the model represents that adjacent record
\* abstractly and does not simulate a byte cursor.

BeginPasteRecord ==
  /\ phase = "idle"
  /\ phase' = "record-open"
  /\ Record("begin-paste-record")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, response, outcome, protocolViolation>>

AbandonPartialRecord ==
  /\ phase = "record-open"
  /\ phase' = "failed"
  /\ Record("abandon-partial-record")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, response, outcome, protocolViolation>>

CompletePasteRecord(grant, sequence) ==
  /\ phase = "record-open"
  /\ grant \in Grants
  /\ sequence \in 1..MaxSequence
  /\ phase' = "request-ready"
  /\ requestGrant' = grant
  /\ requestSequence' = sequence
  /\ Record("complete-paste-record")
  /\ UNCHANGED <<grantAlive, grantSequence, requestHandoffs,
       requestCommitted, terminalAlive, liveMode, preparedMode,
       committedMode, reservation, queueUsed, totalHandoffs, totalCommits,
       response, outcome, protocolViolation>>

\* --- Grant admission ---------------------------------------------------

AdmitPaste ==
  /\ phase = "request-ready"
  /\ grantAlive[requestGrant]
  /\ requestSequence > grantSequence[requestGrant]
  /\ grantSequence' =
       [grantSequence EXCEPT ![requestGrant] = requestSequence]
  /\ phase' = "admitted"
  /\ Record("admit-paste")
  /\ UNCHANGED <<grantAlive, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, response, outcome, protocolViolation>>

RejectPasteAdmission ==
  /\ phase = "request-ready"
  /\ \/ ~grantAlive[requestGrant]
     \/ requestSequence <= grantSequence[requestGrant]
  /\ phase' = "responded"
  /\ response' = "rejected"
  /\ Record("reject-paste-admission")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, outcome, protocolViolation>>

\* --- Single Host handoff ----------------------------------------------

HandoffToHost ==
  /\ phase = "admitted"
  /\ phase' = "handed"
  /\ requestHandoffs' = requestHandoffs + 1
  /\ totalHandoffs' = totalHandoffs + 1
  /\ Record("handoff-to-host")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestCommitted, terminalAlive, liveMode, preparedMode,
       committedMode, reservation, queueUsed, totalCommits, response,
       outcome, protocolViolation>>

\* --- Host preparation --------------------------------------------------
\* Prepare snapshots the live bracketed-paste mode under the terminal
\* mutex and holds one bounded reservation against the queue budget. A
\* dead terminal or a full queue rejects before any effect; no
\* reservation is left behind. Commit runs later under the input-side
\* admission lock; the mode snapshot, not the live mode, encodes it.

PreparePaste ==
  /\ phase = "handed"
  /\ terminalAlive
  /\ queueUsed < QueueCapacity
  /\ phase' = "prepared"
  /\ preparedMode' = liveMode
  /\ reservation' = "held"
  /\ Record("prepare-paste")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       committedMode, queueUsed, totalHandoffs, totalCommits, response,
       outcome, protocolViolation>>

PrepareFails ==
  /\ phase = "handed"
  /\ \/ ~terminalAlive
     \/ queueUsed = QueueCapacity
  /\ phase' = "responded"
  /\ response' = "rejected"
  /\ Record("prepare-fails")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, outcome, protocolViolation>>

\* --- Commit: the only queue effect -------------------------------------
\* Final admission re-checks grant and terminal liveness; publishing uses
\* the prepared-mode snapshot, not the live mode.

CommitPreparedPaste ==
  /\ phase = "prepared"
  /\ grantAlive[requestGrant]
  /\ terminalAlive
  /\ phase' = "responded"
  /\ response' = "accepted"
  /\ committedMode' = preparedMode
  /\ reservation' = "none"
  /\ queueUsed' = queueUsed + 1
  /\ requestCommitted' = TRUE
  /\ totalCommits' = totalCommits + 1
  /\ Record("commit-prepared-paste")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, terminalAlive, liveMode, preparedMode,
       totalHandoffs, outcome, protocolViolation>>

CancelPreparedPaste ==
  /\ phase = "prepared"
  /\ \/ ~grantAlive[requestGrant]
     \/ ~terminalAlive
  /\ phase' = "responded"
  /\ response' = "rejected"
  /\ reservation' = "none"
  /\ Record("cancel-prepared-paste")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, queueUsed, totalHandoffs, totalCommits,
       outcome, protocolViolation>>

\* --- Environment -------------------------------------------------------

FlipBracketedMode ==
  /\ liveMode' = IF liveMode = "bracketed" THEN "plain" ELSE "bracketed"
  /\ Record("flip-bracketed-mode")
  /\ UNCHANGED <<phase, grantAlive, grantSequence, requestGrant,
       requestSequence, requestHandoffs, requestCommitted, terminalAlive,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, response, outcome, protocolViolation>>

RevokeGrant(grant) ==
  /\ grant \in Grants
  /\ grantAlive[grant]
  /\ grantAlive' = [grantAlive EXCEPT ![grant] = FALSE]
  /\ Record("revoke-grant")
  /\ UNCHANGED <<phase, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, response, outcome, protocolViolation>>

KillTerminal ==
  /\ terminalAlive
  /\ terminalAlive' = FALSE
  /\ Record("kill-terminal")
  /\ UNCHANGED <<phase, grantAlive, grantSequence, requestGrant,
       requestSequence, requestHandoffs, requestCommitted, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, response, outcome, protocolViolation>>

DrainQueue ==
  /\ queueUsed > 0
  /\ queueUsed' = queueUsed - 1
  /\ Record("drain-queue")
  /\ UNCHANGED <<phase, grantAlive, grantSequence, requestGrant,
       requestSequence, requestHandoffs, requestCommitted, terminalAlive,
       liveMode, preparedMode, committedMode, reservation, totalHandoffs,
       totalCommits, response, outcome, protocolViolation>>

\* --- Response delivery -------------------------------------------------
\* A correlated accepted or rejected result settles as the matching
\* definite outcome and the carrier survives for the next record. A lost,
\* late, or misclassified response settles this request as unknown and
\* terminates the carrier: the implementation retires the broker for any
\* ambiguous Host exchange, so no further record is admitted. The
\* cross-request aftermath (replacement carrier, incarnation cleanup) is
\* owned by the LocalWorkerObcnCarrier and LocalHostBroker models and
\* their tests, not by this slice.

DeliverResponse ==
  /\ phase = "responded"
  /\ response \in {"accepted", "rejected"}
  /\ outcome' = response
  /\ phase' = "settled"
  /\ Record("deliver-response")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, response, protocolViolation>>

LoseResponse ==
  /\ phase = "responded"
  /\ outcome' = "unknown"
  /\ phase' = "failed"
  /\ Record("lose-response")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, response, protocolViolation>>

DeliverWrongResponse ==
  /\ phase = "responded"
  /\ outcome' = "unknown"
  /\ protocolViolation' = TRUE
  /\ phase' = "failed"
  /\ Record("deliver-wrong-response")
  /\ UNCHANGED <<grantAlive, grantSequence, requestGrant, requestSequence,
       requestHandoffs, requestCommitted, terminalAlive, liveMode,
       preparedMode, committedMode, reservation, queueUsed, totalHandoffs,
       totalCommits, response>>

\* A request settled with a definite outcome retires and the surviving
\* carrier accepts a later record as a fresh request with its own
\* sequence check; nothing about the retired request is replayed. An
\* unknown-settled or failed carrier never reaches this point.
RetireRequest ==
  /\ phase = "settled"
  /\ reservation = "none"
  /\ phase' = "idle"
  /\ requestGrant' = NoGrant
  /\ requestSequence' = 0
  /\ requestHandoffs' = 0
  /\ requestCommitted' = FALSE
  /\ preparedMode' = "none"
  /\ committedMode' = "none"
  /\ response' = "none"
  /\ outcome' = "none"
  /\ Record("retire-request")
  /\ UNCHANGED <<grantAlive, grantSequence, terminalAlive, liveMode,
       reservation, queueUsed, totalHandoffs, totalCommits,
       protocolViolation>>

Next ==
  \/ BeginPasteRecord
  \/ AbandonPartialRecord
  \/ \E grant \in Grants:
       \E sequence \in 1..MaxSequence:
         CompletePasteRecord(grant, sequence)
  \/ AdmitPaste
  \/ RejectPasteAdmission
  \/ HandoffToHost
  \/ PreparePaste
  \/ PrepareFails
  \/ CommitPreparedPaste
  \/ CancelPreparedPaste
  \/ FlipBracketedMode
  \/ \E grant \in Grants: RevokeGrant(grant)
  \/ KillTerminal
  \/ DrainQueue
  \/ DeliverResponse
  \/ LoseResponse
  \/ DeliverWrongResponse
  \/ RetireRequest

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeOK ==
  /\ phase \in Phases
  /\ grantAlive \in [Grants -> BOOLEAN]
  /\ grantSequence \in [Grants -> 0..MaxSequence]
  /\ requestGrant \in GrantValues
  /\ requestSequence \in 0..MaxSequence
  /\ requestHandoffs \in 0..1
  /\ requestCommitted \in BOOLEAN
  /\ terminalAlive \in BOOLEAN
  /\ liveMode \in Modes
  /\ preparedMode \in ModeValues
  /\ committedMode \in ModeValues
  /\ reservation \in {"none", "held"}
  /\ queueUsed \in 0..QueueCapacity
  /\ totalHandoffs \in 0..MaxRequests
  /\ totalCommits \in 0..MaxRequests
  /\ response \in Responses
  /\ outcome \in Outcomes
  /\ protocolViolation \in BOOLEAN
  /\ priorGrantSequence \in [Grants -> 0..MaxSequence]
  /\ priorPreparedMode \in ModeValues
  /\ priorReservation \in {"none", "held"}
  /\ priorRequestHandoffs \in 0..1
  /\ priorTotalCommits \in 0..MaxRequests
  /\ lastAction \in Actions

\* A partially read record is never a request.
PartialRecordProducesNoRequest ==
  phase = "record-open" => requestGrant = NoGrant

\* Abandoning a partial record produces no request, no handoff, no
\* effect, and no reservation; the carrier is terminally failed.
AbandonedRecordProducesNoEffect ==
  lastAction = "abandon-partial-record" =>
    /\ requestGrant = NoGrant
    /\ requestHandoffs = 0
    /\ ~requestCommitted
    /\ reservation = "none"

\* Once the carrier enters "failed" it never leaves: every transition
\* observed there is the failure entry itself or a pure environment
\* action, and no request-path action can fire.
FailedPhaseIsTerminal ==
  phase = "failed" =>
    lastAction \in {
      "abandon-partial-record", "lose-response",
      "deliver-wrong-response", "flip-bracketed-mode", "revoke-grant",
      "kill-terminal", "drain-queue"}

\* An unknown outcome always coincides with a terminated carrier.
UnknownOutcomeTerminatesCarrier ==
  outcome = "unknown" => phase = "failed"

\* One request is handed to Host at most once; an unknown outcome
\* terminates the carrier so no request can be re-handed-off afterwards.
AtMostOneHandoffPerRequest ==
  requestHandoffs <= 1

NoReplayAfterUnknownOutcome ==
  outcome = "unknown" => requestHandoffs <= 1

\* Every commit travels through exactly one handoff.
CommitRequiresHandoff ==
  requestCommitted => requestHandoffs = 1

\* Queue publication never exceeds handoffs taken.
NoEffectWithoutHandoff ==
  totalCommits <= totalHandoffs

\* Admission consumes the sequence monotonically and never rolls it back:
\* the only transition that grows a grant sequence writes the request's
\* own value, and no transition decreases any grant sequence.
SequenceConsumedMonotonically ==
  \A grant \in Grants: grantSequence[grant] >= priorGrantSequence[grant]

AdmittedSequenceIsRequestSequence ==
  lastAction = "admit-paste" =>
    /\ requestSequence > priorGrantSequence[requestGrant]
    /\ grantSequence[requestGrant] = requestSequence

RejectedAdmissionNeverConsumesSequence ==
  lastAction = "reject-paste-admission" =>
    \A grant \in Grants: grantSequence[grant] = priorGrantSequence[grant]

\* A definite rejection never carried a queue effect for this request;
\* a definite acceptance required exactly one.
RejectionPublishesNoEffect ==
  outcome = "rejected" => ~requestCommitted

AcceptedOutcomeRequiresCommittedEffect ==
  outcome = "accepted" => requestCommitted

\* Preparation is a snapshot, not a decision: while a reservation is held
\* the prepared mode is frozen, and a commit publishes under that snapshot
\* regardless of any later live-mode flip.
HeldReservationHasPreparedMode ==
  reservation = "held" => preparedMode \in Modes

ModeFlipPreservesPreparation ==
  lastAction = "flip-bracketed-mode" => preparedMode = priorPreparedMode

CommitPublishesPreparedEncoding ==
  lastAction = "commit-prepared-paste" =>
    /\ priorReservation = "held"
    /\ committedMode = preparedMode
    /\ preparedMode \in Modes

\* A reservation leaves the held state only through commit or
\* cancellation; it cannot leak past retirement. This is a transition
\* safety property: with no fairness assumption the model does not claim
\* the reservation is eventually released.
ReservationReleasedOnce ==
  (priorReservation = "held" /\ reservation = "none") =>
    lastAction \in {"commit-prepared-paste", "cancel-prepared-paste"}

NoHeldReservationOutsidePreparation ==
  reservation = "held" => phase = "prepared"

\* A wrong-class or miscorrelated Host response is a protocol violation
\* that settles the request as unknown, never as a definite outcome.
WrongResponseNeverFabricatesOutcome ==
  lastAction = "deliver-wrong-response" =>
    /\ outcome = "unknown"
    /\ protocolViolation

\* Settling always produces a non-"none" outcome; only retirement clears
\* it for a fresh request. This states settled => outcome # "none"; it is
\* not by itself an exact-once proof. Exactly-once sequence consumption
\* follows from the unidirectional phase graph described in the header.
SettledRequestHasOutcome ==
  phase = "settled" => outcome # "none"

\* Queue occupancy only counts committed effects that have not drained.
QueueUsedCountsCommittedEffects ==
  queueUsed <= totalCommits

=============================================================================
