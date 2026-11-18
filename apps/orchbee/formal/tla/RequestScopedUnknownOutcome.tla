---------------- MODULE RequestScopedUnknownOutcome ----------------
EXTENDS Integers, FiniteSets

(*
Request-scoped unknown outcomes on one live multiplexed carrier (ORC-506).

Scope: workspace close, tab close and tab rename submissions on the native executor, the
carrier under them, and the notices and evidence they leave behind.

Claims. Each design flag is TRUE in the checked configuration. Each flag has one mutant in
scripts/tla/model-mutations.test.mjs, and TLC must reject every mutant:
  C1 NoReplayOfUnknownId (flag ReplayGuard): an identity whose outcome was a stream-scoped
     unknown is never submitted again under that identity.
  C2 StreamUnknownNeverRetires (flag KeepCarrierOnStreamUnknown): a stream-scoped unknown on a
     live carrier does not retire the carrier.
  C3 TypedHoldKeepsFence (flag ReleaseOnlyOnStreamUnknown): a typed Server answer (Unavailable,
     Denied or ServerUnknown) keeps the close fence held; only a stream-scoped unknown releases it.
  C4 CloseSerialized (flag BlockCompetingClose): at most one close (workspace or tab) is in flight.
  C5 UnknownNoticeKeptUntilKey (flag TimerKeepsUnknown; action property): only a Client keypress
     clears an unknown notice.
  C6 PublicationCatchesUp (liveness, fairness of Publish): an owed publication read on a live
     carrier is eventually performed, or the carrier retires.

Interleavings modeled: two request slots, so two requests may be in flight; a second close while
one is in flight; a new submission after an unknown outcome; a typed answer; a stream-scoped
unknown; a timer tick; a keypress; a publication; a real disconnect; a protocol violation.

Bounds: two request slots, MaxIds request identities, one carrier, one close fence, one rename
fence, one unknown notice and one publication debt.

Abstractions:
  - The three typed answers share one effect. Their label is kept for the source map and is not
    stored, so the state does not record which answer arrived.
  - A rename is modeled only by its structural fence; rename evidence is not recorded.
  - A close is admitted while a rename is in flight. The code may forbid this, so the model
    checks a superset of its interleavings.
  - A typed answer's report is modeled as a transient notice that a timer may clear. The code keeps
    that report until a later validated Closed; the over-approximation cannot hide a C5 violation,
    because C5 concerns only the unknown notice.
  - Startup, setup and tab-list navigation are not modeled.

Assumptions: the Server answers each request at most once; a typed answer arrives on a live
carrier and leaves it live; a timer clears only transient notices; a keypress is the only action
that clears an unknown notice or unknown close evidence.

Action to source (paths under apps/orchbee/crates; function names are stable, line numbers are not):
  Submit(s, k)       workbench-client/src/workbench_client/connection/native/workspace_close.rs
                     (run_workspace_close, submit_single_workspace_close, submit_project_close_all),
                     native/tab_close.rs and native/tab_rename.rs. Each takes its request identity
                     from workbench-client/src/workbench_client/mailbox.rs allocate_workbench_request_id.
  Retry(s)           a new submission for the same target after an unknown outcome. ReplayGuard
                     decides whether it takes a fresh identity from allocate_workbench_request_id.
  Ok(s)              workbench-client/src/workbench_state/workspace_close.rs observe_workspace_close
                     and accept_workspace_close_event with a settled result; the settled tab close
                     result in workbench-client/src/workbench_state/tab_close.rs; the settled rename
                     result in workbench-client/src/workbench_state/tab_rename.rs.
  TypedAnswer(s, a)  workbench_state/workspace_close.rs observe_workspace_close with
                     WorkspaceCloseObservation::Unanswered or a typed Answered status; tab close:
                     workbench_state/tab_close.rs observation_failed, which sets no request-scoped flag.
  StreamUnknown(s)   connection/native/workspace_close.rs publishes WorkerEvent::WorkspaceCloseResult
                     with request_unknown set, or WorkspaceCloseObservation::RequestUnknown. Both are
                     gated by connection/native.rs ensure_unknown_is_request_scoped and
                     generation().is_some(). Tab close: workbench_state/tab_close.rs
                     live_observation_failed, and the Ambiguous arms of the Close and CloseIfIdle operations
                     in tab_close/flight.rs (apply_result and apply_close_if_idle). The arms set the
                     request-scoped flag and keep the uncertain evidence, conditional or not;
                     live_observation_failed sets the flag alone. A keypress removes an uncertain record
                     through dismiss_unknown_outcome_closes only while it holds no operation and no close of
                     that Tab is pending, handed off, under an armed confirmation, reconciling or manual. A
                     record that holds an operation keeps it. Acceptance and a successful observation reset
                     the flag, so a keypress leaves such a record unchanged until a later live observation
                     failure sets the flag again; the keypress then dismisses that report. The model's
                     StreamUnknown sets the unknown notice and, for a close, the unknown close evidence; these
                     arms are its Tab close counterpart.
  Publish            workbench_state.rs owe_publication_read, released by release_unread_publication_debt.
  KeyDismiss         workbench_state.rs dismiss_unknown_outcome_notices, called from the keypress
                     paths in workbench_client/ui.rs and ui/input_routing.rs, and
                     workbench_state/workspace_close.rs clear_settled_unknown_workspace_close. Tab
                     close: dismiss_unknown_outcome_closes (tab_close.rs) removes settled uncertain
                     records and dismisses the report of request-scoped records only. Its last step,
                     dismiss_lost_close_set_summary (tab_close/set.rs), hides the summary of a Lost set only,
                     which the model does not represent. In the model KeyDismiss clears unknown notices and
                     unknown close evidence; TimerTick expires transient notices. The model has no
                     reconciliation, handoff, confirmation or manual state, so its KeyDismiss has none of
                     the guards above.
  TimerTick          workbench_state.rs: expiry of transient notices (ClientNotice::transient).
  RealDisconnect     local-client/src/local_client.rs forget_connection_after_unknown on transport
                     loss; the native executor retires the session.
  ProtocolViolation  local-client/src/local_client.rs forget_connection_after_unknown on correlation
                     and contract failures.
*)

CONSTANTS
  MaxIds,                     \* request identities the Client may mint
  ReplayGuard,                \* design: a retry of an unknown request mints a fresh identity
  KeepCarrierOnStreamUnknown, \* design: a stream-scoped unknown keeps the carrier live
  ReleaseOnlyOnStreamUnknown, \* design: typed Server answers keep the close fence held
  BlockCompetingClose,        \* design: at most one close in flight
  TimerKeepsUnknown           \* design: a timer never clears an unknown notice

Slots == {1, 2}
Kinds == {"Close", "TabClose", "Rename"}
TypedAnswers == {"Unavailable", "Denied", "ServerUnknown"}
IsClose(k) == k = "Close" \/ k = "TabClose"

VARIABLES
  conn,            \* "live" or "retired"
  retiredBy,       \* "none", "real", "protocol" or "stream" (stream only under a mutant)
  slotState,       \* "idle" or "out" per request slot
  slotKind,        \* the kind of request last in the slot, or "none" before first use
  slotId,          \* the request identity in the slot, 0 before first use
  nextId,          \* the next fresh identity
  unknownIds,      \* identities whose outcome was a stream-scoped unknown
  replayed,        \* an unknown identity was submitted again
  closeFence,      \* structural fence (and a tab's input suppression) held by a close
  typedHold,       \* the close fence is held by a typed Server answer
  renameFence,     \* a rename is unresolved
  closeEvidence,   \* "none", "held" (in flight or kept for recovery) or "unknown" (shown until a key)
  notice,          \* "none", "transient" or "unknown"
  pubOwed,         \* a publication read is owed
  lastKey          \* the last step was a Client keypress

vars == <<conn, retiredBy, slotState, slotKind, slotId, nextId, unknownIds, replayed,
          closeFence, typedHold, renameFence, closeEvidence, notice, pubOwed, lastKey>>

CloseOut == {s \in Slots : slotState[s] = "out" /\ IsClose(slotKind[s])}
RenameOut == {s \in Slots : slotState[s] = "out" /\ slotKind[s] = "Rename"}

Admissible(k) ==
  IF IsClose(k)
  THEN (~closeFence /\ (BlockCompetingClose => CloseOut = {}))
  ELSE (~renameFence /\ ~closeFence /\ CloseOut = {})

TypeOK ==
  /\ conn \in {"live", "retired"}
  /\ retiredBy \in {"none", "real", "protocol", "stream"}
  /\ slotState \in [Slots -> {"idle", "out"}]
  /\ slotKind \in [Slots -> {"none"} \cup Kinds]
  /\ slotId \in [Slots -> 0..MaxIds]
  /\ nextId \in 1..(MaxIds + 1)
  /\ unknownIds \subseteq 1..MaxIds
  /\ replayed \in BOOLEAN
  /\ closeFence \in BOOLEAN
  /\ typedHold \in BOOLEAN
  /\ renameFence \in BOOLEAN
  /\ closeEvidence \in {"none", "held", "unknown"}
  /\ notice \in {"none", "transient", "unknown"}
  /\ pubOwed \in BOOLEAN
  /\ lastKey \in BOOLEAN

Init ==
  /\ conn = "live"
  /\ retiredBy = "none"
  /\ slotState = [s \in Slots |-> "idle"]
  /\ slotKind = [s \in Slots |-> "none"]
  /\ slotId = [s \in Slots |-> 0]
  /\ nextId = 1
  /\ unknownIds = {}
  /\ replayed = FALSE
  /\ closeFence = FALSE
  /\ typedHold = FALSE
  /\ renameFence = FALSE
  /\ closeEvidence = "none"
  /\ notice = "none"
  /\ pubOwed = FALSE
  /\ lastKey = FALSE

\* Submit: a new request in an idle slot. Two requests may be in flight at once; a second close
\* is admitted only when BlockCompetingClose is FALSE (mutant).
Submit(s, k) ==
  /\ conn = "live"
  /\ slotState[s] = "idle"
  /\ nextId <= MaxIds
  /\ Admissible(k)
  /\ slotState' = [slotState EXCEPT ![s] = "out"]
  /\ slotKind' = [slotKind EXCEPT ![s] = k]
  /\ slotId' = [slotId EXCEPT ![s] = nextId]
  /\ nextId' = nextId + 1
  /\ closeEvidence' = IF IsClose(k) THEN "held" ELSE closeEvidence
  /\ renameFence' = IF k = "Rename" THEN TRUE ELSE renameFence
  /\ lastKey' = FALSE
  /\ UNCHANGED <<conn, retiredBy, unknownIds, replayed, closeFence, typedHold, notice, pubOwed>>

\* Retry: a new submission for the same target after a stream-scoped unknown. Under the design
\* it takes a fresh identity; under the mutant it reuses the unknown identity, which is a replay.
Retry(s) ==
  /\ conn = "live"
  /\ slotState[s] = "idle"
  /\ slotKind[s] /= "none"
  /\ slotId[s] \in unknownIds
  /\ nextId <= MaxIds
  /\ Admissible(slotKind[s])
  /\ slotState' = [slotState EXCEPT ![s] = "out"]
  /\ slotId' = IF ReplayGuard THEN [slotId EXCEPT ![s] = nextId] ELSE slotId
  /\ nextId' = IF ReplayGuard THEN nextId + 1 ELSE nextId
  /\ replayed' = IF ReplayGuard THEN replayed ELSE TRUE
  /\ closeEvidence' = IF IsClose(slotKind[s]) THEN "held" ELSE closeEvidence
  /\ renameFence' = IF slotKind[s] = "Rename" THEN TRUE ELSE renameFence
  /\ lastKey' = FALSE
  /\ UNCHANGED <<conn, retiredBy, slotKind, unknownIds, closeFence, typedHold, notice, pubOwed>>

\* Ok: the Server settles the request with a result.
Ok(s) ==
  /\ conn = "live"
  /\ slotState[s] = "out"
  /\ slotState' = [slotState EXCEPT ![s] = "idle"]
  /\ closeFence' = IF IsClose(slotKind[s]) THEN FALSE ELSE closeFence
  /\ typedHold' = IF IsClose(slotKind[s]) THEN FALSE ELSE typedHold
  /\ closeEvidence' = IF IsClose(slotKind[s]) THEN "none" ELSE closeEvidence
  /\ renameFence' = IF slotKind[s] = "Rename" THEN FALSE ELSE renameFence
  /\ lastKey' = FALSE
  /\ UNCHANGED <<conn, retiredBy, slotKind, slotId, nextId, unknownIds, replayed, notice, pubOwed>>

\* TypedAnswer: a Server answer that is not a stream-scoped signal. The carrier stays live. A typed
\* close answer keeps the close fence and its evidence held (design); the mutant releases them, so
\* the evidence is shown as an unknown outcome. A rename's typed answer clears its fence.
TypedAnswer(s, a) ==
  /\ a \in TypedAnswers
  /\ conn = "live"
  /\ slotState[s] = "out"
  /\ slotState' = [slotState EXCEPT ![s] = "idle"]
  /\ closeFence' = IF IsClose(slotKind[s]) THEN ReleaseOnlyOnStreamUnknown ELSE closeFence
  /\ typedHold' = IF IsClose(slotKind[s]) THEN TRUE ELSE typedHold
  /\ closeEvidence' =
       IF IsClose(slotKind[s])
       THEN (IF ReleaseOnlyOnStreamUnknown THEN "held" ELSE "unknown")
       ELSE closeEvidence
  /\ renameFence' = IF slotKind[s] = "Rename" THEN FALSE ELSE renameFence
  /\ notice' = IF notice = "unknown" THEN "unknown" ELSE "transient"
  /\ lastKey' = FALSE
  /\ UNCHANGED <<conn, retiredBy, slotKind, slotId, nextId, unknownIds, replayed, pubOwed>>

\* StreamUnknown: the Server closed this one request as unknown on a live carrier. The carrier
\* stays live (design), the fence is released, the evidence stays visible until a keypress, the
\* notice persists, and a publication read is owed.
StreamUnknown(s) ==
  /\ conn = "live"
  /\ slotState[s] = "out"
  /\ slotState' = [slotState EXCEPT ![s] = "idle"]
  /\ unknownIds' = unknownIds \cup {slotId[s]}
  /\ conn' = IF KeepCarrierOnStreamUnknown THEN conn ELSE "retired"
  /\ retiredBy' = IF KeepCarrierOnStreamUnknown THEN retiredBy ELSE "stream"
  /\ closeFence' = IF IsClose(slotKind[s]) THEN FALSE ELSE closeFence
  /\ typedHold' = IF IsClose(slotKind[s]) THEN FALSE ELSE typedHold
  /\ closeEvidence' = IF IsClose(slotKind[s]) THEN "unknown" ELSE closeEvidence
  /\ renameFence' = IF slotKind[s] = "Rename" THEN FALSE ELSE renameFence
  /\ notice' = "unknown"
  /\ pubOwed' = TRUE
  /\ lastKey' = FALSE
  /\ UNCHANGED <<slotKind, slotId, nextId, replayed>>

\* Publish: the canonical publication arrives on a live carrier and settles the debt.
Publish ==
  /\ conn = "live"
  /\ pubOwed
  /\ pubOwed' = FALSE
  /\ lastKey' = FALSE
  /\ UNCHANGED <<conn, retiredBy, slotState, slotKind, slotId, nextId, unknownIds, replayed,
                 closeFence, typedHold, renameFence, closeEvidence, notice>>

\* KeyDismiss: a Client keypress clears an unknown notice and the unknown close evidence. A
\* transient notice is left to TimerTick, as the code leaves transient notices to their timer.
KeyDismiss ==
  /\ (notice = "unknown" \/ closeEvidence = "unknown")
  /\ notice' = IF notice = "unknown" THEN "none" ELSE notice
  /\ closeEvidence' = IF closeEvidence = "unknown" THEN "none" ELSE closeEvidence
  /\ lastKey' = TRUE
  /\ UNCHANGED <<conn, retiredBy, slotState, slotKind, slotId, nextId, unknownIds, replayed,
                 closeFence, typedHold, renameFence, pubOwed>>

\* TimerTick: a transient notice expires. The unknown notice survives unless the mutant lets
\* timers clear it.
TimerTick ==
  /\ notice /= "none"
  /\ notice' = IF notice = "transient" THEN "none" ELSE IF TimerKeepsUnknown THEN "unknown" ELSE "none"
  /\ lastKey' = FALSE
  /\ UNCHANGED <<conn, retiredBy, slotState, slotKind, slotId, nextId, unknownIds, replayed,
                 closeFence, typedHold, renameFence, closeEvidence, pubOwed>>

\* Retire: the carrier is lost or violated. In-flight requests end; a lost close or rename keeps
\* its fence and evidence.
Retire(cause) ==
  /\ conn' = "retired"
  /\ retiredBy' = cause
  /\ slotState' = [s \in Slots |-> "idle"]
  /\ closeFence' = IF CloseOut /= {} THEN TRUE ELSE closeFence
  /\ closeEvidence' = IF CloseOut /= {} THEN "held" ELSE closeEvidence
  /\ renameFence' = IF RenameOut /= {} THEN TRUE ELSE renameFence
  /\ lastKey' = FALSE
  /\ UNCHANGED <<slotKind, slotId, nextId, unknownIds, replayed, typedHold, notice, pubOwed>>

RealDisconnect ==
  /\ conn = "live"
  /\ Retire("real")

ProtocolViolation ==
  /\ conn = "live"
  /\ (\E s \in Slots : slotState[s] = "out")
  /\ Retire("protocol")

Next ==
  \/ \E s \in Slots, k \in Kinds : Submit(s, k)
  \/ \E s \in Slots : Retry(s)
  \/ \E s \in Slots : Ok(s)
  \/ \E s \in Slots, a \in TypedAnswers : TypedAnswer(s, a)
  \/ \E s \in Slots : StreamUnknown(s)
  \/ Publish
  \/ KeyDismiss
  \/ TimerTick
  \/ RealDisconnect
  \/ ProtocolViolation

Spec == Init /\ [][Next]_vars /\ WF_vars(Publish)

\* ---------------------------------------------------------------------------
\* Invariants and properties (claims C1 to C6)
\* ---------------------------------------------------------------------------

NoReplayOfUnknownId == ~replayed

StreamUnknownNeverRetires == retiredBy /= "stream"

TypedHoldKeepsFence == typedHold => closeFence

CloseSerialized == Cardinality(CloseOut) <= 1

UnknownNoticeKeptUntilKey ==
  [][ (notice = "unknown" /\ notice' /= "unknown") => lastKey' ]_vars

PublicationCatchesUp ==
  []((pubOwed /\ conn = "live") => <>(~pubOwed \/ conn = "retired"))

=============================================================================
