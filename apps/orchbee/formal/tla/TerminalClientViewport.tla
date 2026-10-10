---------------------- MODULE TerminalClientViewport ----------------------
(* Client-side retained-history viewport for one pane incarnation: demand
   legs, facts intake (kind 53), the row-window read/acknowledge handshake,
   navigation coalescing, bounded committed deltas, recovery, and the Host's
   pending-read pin plus its per-attachment read-sequence watermark. This is
   the contract for terminal_session/terminal_viewport.rs; see
   docs/native-terminal-viewport.md.

   Corrections modeled (recovery challenge report): B1 no read while a window
   is pending acknowledgement; B2 host pending/state rejections enter recovery,
   never kill; B3 committed deltas split at +/-DeltaBound; B4 anchor loss
   enters Recovering, position never faked; B5 every valid event is
   acknowledged exactly once; B6 Recover is admitted before ordinary input.
   Fairness is asserted only on client/executor/delivery actions — host event
   emission, resize, eviction, and fault windows stay unfair so silence and
   loss are real behaviours. The window column-count gate is subsumed by the
   generation gate: a stale-geometry window follows the same StaleWindow
   release path.

   Consolidation (post-review): AckAccepted promotes Entering to Viewing so
   the committed-read path is reachable; the host carries a per-attachment
   read-sequence watermark that survives history episodes and rejects reuse
   exactly like rows.zig; host event emission is suppressed unless the demand
   is viewing, history is dirty, and no read flight or pending pin exists; a
   pending-pin or reused-sequence blockage is a typed rejection, not an opaque
   fault; acknowledgements bind sequence and generation (revision folds into
   generation in this abstraction); and an absent event anchor is invalid,
   never a valid one.

   Admission/outcome revision (task-12): the client is non-omniscient — mint
   and admission are separate steps. `staged` holds at most one un-admitted
   step (a Read, an Ack, or a Recover — production `pending_commands`); the
   wire `cmd_q` holds only admitted commands and is never pruned: recovery
   and resize can only replace the un-admitted staged step, while admitted
   steps always execute and always produce (droppable) outcomes. `next_seq`
   is consumed at mint exactly like `next_read_sequence`, so minted-then-
   dropped sequences leave honest gaps and are never reissued; `read_flight`
   binds only when a Read is admitted (`command_admitted`), and `ack` enters
   Flight only when an Ack is admitted. A staged Recover is decided recovery
   (`recovery_pending()`: late facts, windows, and read/ack outcomes are
   absorbed while it pends), but Recovering begins only at its admission —
   `QueueRecover` clears the intent state at mint and the admission carries
   the mode transition plus the flight/generation clears. Outcomes are a
   FIFO of at most two typed records — one published, one computed-and-
   blocked at publish behind it — matching the single worker draining the
   single EventCell. A published-unconsumed outcome never blocks the next
   command's execution (WorkerFree bounds producers, never consumers); only
   a second blocked outcome does. Input behind an applied-but-unconsumed
   Recover stays reachable. Enter keeps the direct-mint abstraction: the
   core does not exist until the Enter is admitted, so the model appends it
   to cmd_q in PressEnter and never spends a read sequence on it. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxSeq, MaxEvent, MaxGen, MaxDelta, DeltaBound, MaxQueue, MaxDeadline

Modes == {"live", "entering", "viewing", "recovering", "lost"}
AckStates == {"none", "pending", "flight", "accepted"}
NavKinds == {"none", "delta", "jump"}
ReadResults == {"window", "typed", "invalid", "limit", "hostrej"}
AckResults == {"accepted", "rejected", "evicted"}
EnterResults == {"applied", "rejected"}
Anchors == {"committed", "evicted", "none"}
Demands == {"following", "viewing"}
Projs == {"active", "paused"}
ReadCommands == {<<"read", s, g>> : s \in 1..MaxSeq, g \in 1..MaxGen}
AckCommands == {<<"ack", s, g>> : s \in 1..MaxSeq, g \in 1..MaxGen}
WireSteps == ReadCommands \cup AckCommands
\* At most one un-admitted step is staged (production `pending_commands` is
\* normalized to one by `queue_recover`); a staged Recover is a uniform
\* three-tuple so index access never ranges out.
RecoverStep == <<"recover", 0, 0>>
StagedSteps == WireSteps \cup {RecoverStep}
EnterStep == <<"enter", 0, 0>>
InputStep == <<"input", 0, 0>>
\* All commands have the same tuple shape so TLC never compares a string
\* with a tuple. No-payload tags carry zero slots; Read/Ack keep their
\* sequence and generation. Once admitted, only the worker removes them.
AdmittedCommands == {EnterStep, RecoverStep, InputStep} \cup WireSteps
\* Typed outcome records: one slot per produced outcome, FIFO — slot 1 is
\* published in the cell, slot 2 is computed-but-blocked at publish.
EnterSlot ==
    {[type |-> "enter", e |-> e, sq |-> 0, g |-> 0, r |-> "none",
      a |-> "none", v |-> "none"] : e \in EnterResults}
ReadSlot ==
    {[type |-> "read", r |-> r, sq |-> s, g |-> g, a |-> "none",
      e |-> "none", v |-> "none"] :
        r \in ReadResults, s \in 0..MaxSeq, g \in 0..MaxGen}
AckSlot ==
    {[type |-> "ack", a |-> a, sq |-> s, g |-> 0, r |-> "none",
      e |-> "none", v |-> "none"] : a \in AckResults, s \in 0..MaxSeq}
RecoverSlot ==
    [type |-> "recover", v |-> "applied", sq |-> 0, g |-> 0, r |-> "none",
     a |-> "none", e |-> "none"]
OutcomeSlots == EnterSlot \cup ReadSlot \cup AckSlot \cup {RecoverSlot}

VARIABLES
    mode,           \* client viewport mode
    facts_gen,      \* newest validated HistoryChanged generation (0 = none)
    need_facts,     \* waiting on a fresh event before the next read
    win_seq,        \* installed window's read sequence (0 = none)
    win_gen,        \* installed window's generation
    ack,            \* RowsApplied lifecycle for the installed window
    read_flight,    \* ADMITTED read awaiting its host outcome (0 = none)
    next_seq,       \* next read sequence — persists across episodes; the
                    \* attachment-scoped watermark owner never reuses one
    read_issue_gen, \* facts generation the in-flight read was issued under
    pending_nav,    \* coalesced navigation intent
    nav_delta,      \* merged signed line delta for pending_nav = "delta"
    committed,      \* accepted acknowledgement promoted the pending anchor
    refresh,        \* a fresh Committed{0} read is wanted after new facts
    deadline,       \* armed facts budget (>0); expires at 1
    retry_used,     \* one stale-generation/revision retry per intent
    halve_used,     \* one LimitExceeded row-count halving per intent
    recover_done,   \* recovery demands applied; a post-recovery frame ends it
    drawn_gen,      \* generation of the painted surface (0 = nothing drawn)
    cmd_q,          \* ADMITTED command order — irrevocable, never pruned
    staged,         \* at most one un-admitted step (production pending_commands)
    outcomes,       \* typed outcome FIFO: [1] published, [2] blocked at publish
    evt_pending,    \* a HistoryChanged event awaits client intake
    evt_seq,        \* sequence of the pending event
    evt_gen,        \* generation of the pending event
    evt_anchor,     \* anchor status the pending event reports
    last_evt_seq,   \* last accepted event sequence
    observed_seq,   \* last HistoryObserved the client emitted
    host_gen,       \* host-side current generation
    host_pending,   \* host-side pending read pin (0 = none)
    host_committed, \* host-side committed anchor presence (0 = none, 1 = some)
    host_read_watermark, \* host-side per-attachment last served read sequence
    host_dirty,     \* host-side unannounced history state exists
    demand,         \* host-side history demand
    proj           \* host-side projection demand

vars == <<mode, facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
          next_seq, read_issue_gen, pending_nav, nav_delta, committed,
          refresh, deadline, retry_used, halve_used, recover_done, drawn_gen,
          cmd_q, staged, outcomes, evt_pending, evt_seq, evt_gen,
          evt_anchor, last_evt_seq, observed_seq, host_gen, host_pending,
          host_committed, host_read_watermark, host_dirty, demand, proj>>

Init ==
    /\ mode = "live" /\ facts_gen = 0 /\ need_facts = 0
    /\ win_seq = 0 /\ win_gen = 0 /\ ack = "none" /\ read_flight = 0
    /\ next_seq = 1 /\ read_issue_gen = 0 /\ pending_nav = "none"
    /\ nav_delta = 0 /\ committed = 0
    /\ refresh = 0 /\ deadline = 0 /\ retry_used = 0 /\ halve_used = 0
    /\ recover_done = 0 /\ drawn_gen = 0 /\ cmd_q = <<>>
    /\ staged = <<>> /\ outcomes = <<>>
    /\ evt_pending = 0 /\ evt_seq = 0 /\ evt_gen = 0 /\ evt_anchor = "committed"
    /\ last_evt_seq = 0 /\ observed_seq = 0 /\ host_gen = 1 /\ host_pending = 0
    /\ host_committed = 0 /\ host_read_watermark = 0 /\ host_dirty = 0
    /\ demand = "following" /\ proj = "active"

\* ---------------- shared predicates and effects ----------------

\* The worker is free to execute the next command while at most one outcome
\* is outstanding: a published-unconsumed outcome does not block execution,
\* a computed-but-blocked second one does.
WorkerFree == Len(outcomes) <= 1

\* Recovery is decided once Recovering begins OR a Recover still waits for
\* admission — production `recovery_pending()`. From here on, late facts,
\* windows, and read/ack outcomes are absorbed, and navigation/typing mint
\* no second Recover.
RecoveryPending ==
    mode = "recovering" \/ (staged /= <<>> /\ staged[1][1] = "recover")

\* `queue_recover()`: the staged step — whatever non-Recover step it is —
\* becomes one Recover, and the intent state it owned clears at mint. The
\* read flight and issue generation do NOT clear here: they belong to the
\* wire command the core still believes admitted, and they clear only when
\* the Recover itself is admitted (production `command_admitted`). The mode
\* stays — Recovering begins at admission, not at the decision.
QueueRecoverStep ==
    /\ staged' = <<RecoverStep>>
    /\ pending_nav' = "none" /\ nav_delta' = 0
    /\ refresh' = 0 /\ deadline' = 0

\* `command_admitted`'s Recover arm: Recovering begins, the intent state
\* clears, and the admitted read flight and its issue generation release —
\* exactly the production bundle.
\* Normalize fields whose readers are disabled throughout Recovering.
\* Production retains them until reset; all exits reset them before reuse.
\* Keep window/ack identity, wire state, facts and recovery completion.
RecoverAdmitted ==
    /\ retry_used' = 0 /\ halve_used' = 0 /\ win_gen' = 0
    /\ committed' = 0 /\ need_facts' = 0
    /\ mode' = "recovering"
    /\ pending_nav' = "none" /\ nav_delta' = 0
    /\ read_flight' = 0 /\ read_issue_gen' = 0
    /\ refresh' = 0 /\ deadline' = 0

StagedIsRead == staged /= <<>> /\ staged[1][1] = "read"
StagedIsAck == staged /= <<>> /\ staged[1][1] = "ack"
StagedIsRecover == staged /= <<>> /\ staged[1][1] = "recover"

\* ---------------- client emit gates ----------------

\* Everything the core can see before minting a read: flight, ack lifecycle,
\* facts freshness, the staged slot, and the sequence bound. The wire queue,
\* worker, and outcome cell are invisible to the core — admission pressure
\* lives at AdmitStaged, never at mint.
ReadUnblocked ==
    /\ read_flight = 0              \* at most one admitted read in flight
    /\ ack \in {"none", "accepted"} \* never while an ack waits or flies (B1)
    /\ need_facts = 0
    /\ facts_gen > 0                \* reads need current facts
    /\ staged = <<>>                \* the single staged slot is free
    /\ next_seq <= MaxSeq           \* a sequence can still be minted

\* A live-mode or fresh-episode reset: the per-incarnation next_seq is NOT
\* part of it — the watermark owner outlives episodes (rows.zig's
\* last_row_request_sequence is attachment-scoped), so re-entry resumes above
\* the last issued value. The staged slot dies with the core; the admitted
\* queue and the published outcomes do not — they belong to the wire.
ResetCore ==
    /\ win_seq' = 0 /\ win_gen' = 0 /\ ack' = "none" /\ read_flight' = 0
    /\ read_issue_gen' = 0 /\ pending_nav' = "none" /\ nav_delta' = 0
    /\ committed' = 0 /\ refresh' = 0 /\ need_facts' = 0 /\ deadline' = 0
    /\ retry_used' = 0 /\ halve_used' = 0 /\ drawn_gen' = 0
    /\ recover_done' = 0 /\ staged' = <<>>
    /\ UNCHANGED next_seq

\* A lost incarnation has no core. Keep wire queues, event and read witnesses.
LoseCore ==
    /\ ResetCore /\ facts_gen' = 0

\* ---------------- client-local actions ----------------

\* Enter rides the admitted queue directly (the core only exists after the
\* Enter is admitted); production mints it through enter_history with the
\* watermark sequence of the projection — the model keeps it an
\* ordering-only atom and never spends next_seq on it.
PressEnter ==
    /\ mode = "live" /\ Len(cmd_q) < MaxQueue
    /\ mode' = "entering" /\ cmd_q' = Append(cmd_q, EnterStep)
    /\ deadline' = MaxDeadline /\ need_facts' = 1 /\ recover_done' = 0

    /\ UNCHANGED <<facts_gen, win_seq, win_gen, ack, read_flight, next_seq,
                   read_issue_gen, pending_nav, nav_delta, committed, refresh,
                   retry_used, halve_used, drawn_gen, staged, outcomes,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

EmitTail ==
    /\ (mode = "entering" \/ (mode = "viewing" /\ pending_nav = "jump"))
    /\ ReadUnblocked
    \* Mint consumes the sequence and stamps the issue generation now; the
    \* flight binds only when this staged step is admitted.
    /\ staged' = <<<<"read", next_seq, facts_gen>>>>
    /\ read_issue_gen' = facts_gen
    /\ next_seq' = next_seq + 1
    /\ pending_nav' = "none" /\ nav_delta' = 0 /\ refresh' = 0
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, committed, deadline, retry_used, halve_used,
                   recover_done, drawn_gen, cmd_q, outcomes,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj>>

EmitCommitted ==
    /\ mode = "viewing" /\ committed = 1
    /\ (pending_nav = "delta" \/ refresh = 1)
    /\ ReadUnblocked
    /\ LET chunk == IF nav_delta > DeltaBound THEN DeltaBound
                    ELSE IF nav_delta < (0 - DeltaBound) THEN 0 - DeltaBound
                    ELSE nav_delta
       IN /\ nav_delta' = nav_delta - chunk
          /\ pending_nav' = IF nav_delta - chunk = 0 THEN "none" ELSE "delta"
    /\ refresh' = 0
    /\ staged' = <<<<"read", next_seq, facts_gen>>>>
    /\ read_issue_gen' = facts_gen
    /\ next_seq' = next_seq + 1

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, committed, deadline, retry_used, halve_used,
                   recover_done, drawn_gen, cmd_q, outcomes,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj>>

\* A fresh navigation intent re-arms the per-intent retry/halving budget;
\* merging into a pending, staged, or in-flight intent shares its budget
\* (production: the flags reset only when nav finds no pending and no
\* in-flight intent — a staged read IS the in-flight intent, a staged ack
\* is not).
ScrollDelta(d) ==
    /\ mode \in {"entering", "viewing"} /\ ~StagedIsRecover
    /\ pending_nav /= "jump"    \* absolute intent owns the next read
    /\ nav_delta' = IF nav_delta + d > MaxDelta THEN MaxDelta
                    ELSE IF nav_delta + d < (0 - MaxDelta) THEN 0 - MaxDelta
                    ELSE nav_delta + d
    /\ pending_nav' = IF nav_delta' = 0 THEN "none" ELSE "delta"
    /\ IF pending_nav = "none" /\ read_flight = 0 /\ ~StagedIsRead
       THEN /\ retry_used' = 0 /\ halve_used' = 0
       ELSE UNCHANGED <<retry_used, halve_used>>

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, committed, refresh,
                   deadline, recover_done, drawn_gen, cmd_q, staged, outcomes,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

PressJump ==
    /\ mode \in {"entering", "viewing"} /\ ~StagedIsRecover
    /\ pending_nav' = "jump" /\ nav_delta' = 0
    /\ IF pending_nav = "none" /\ read_flight = 0 /\ ~StagedIsRead
       THEN /\ retry_used' = 0 /\ halve_used' = 0
       ELSE UNCHANGED <<retry_used, halve_used>>
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, committed, refresh,
                   deadline, recover_done, drawn_gen, cmd_q, staged, outcomes,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* Exit while recovery is already decided — Recovering, or a Recover still
\* staged — is absorbed: production nav() returns on recovery_pending()
\* before its match arm, so no second Recover is ever minted.
PressExit ==
    /\ mode \in {"entering", "viewing"} /\ ~StagedIsRecover
    /\ QueueRecoverStep
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, committed,
                   retry_used, halve_used, recover_done, drawn_gen, cmd_q,
                   outcomes, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

\* ui.forward: while the pane already reports Recovering the byte wires
\* directly — capacity decides. Otherwise typed() mints (or keeps) the
\* staged Recover, the owner tries to admit it, and only once the Recover
\* is admitted does the byte get behind it: room for both -> Recover+Input;
\* room for one -> the Recover admits and the byte drops; no room -> the
\* Recover stays staged and the byte drops — a dropped byte is never
\* replayed, never forwarded while the pane still scrolls. A Recover
\* already staged is retried under the same rule, never re-minted.
KeyTyped ==
    /\ mode \in {"entering", "viewing", "recovering"}
    /\ LET room == MaxQueue - Len(cmd_q) IN
       \/ /\ mode = "recovering" /\ room >= 1
          /\ cmd_q' = Append(cmd_q, InputStep)
          /\ UNCHANGED <<mode, staged, pending_nav, nav_delta, read_flight,
                         read_issue_gen, refresh, deadline>>
       \/ /\ mode /= "recovering" /\ room >= 2
          /\ staged' = <<>>
          /\ cmd_q' = Append(Append(cmd_q, RecoverStep), InputStep)
          /\ RecoverAdmitted
       \/ /\ mode /= "recovering" /\ room = 1
          /\ staged' = <<>>
          /\ cmd_q' = Append(cmd_q, RecoverStep)
          /\ RecoverAdmitted
          \* the input byte drops — never replayed
       \/ /\ mode /= "recovering" /\ room = 0
          /\ IF StagedIsRecover
             THEN UNCHANGED <<staged, pending_nav, nav_delta, refresh,
                              deadline>>
             ELSE QueueRecoverStep
          /\ UNCHANGED <<mode, cmd_q, read_flight, read_issue_gen>>

    /\ UNCHANGED <<facts_gen, win_seq, ack, next_seq, recover_done, drawn_gen,
                   outcomes, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>
    \* Only the branches that admit Recover call RecoverAdmitted.
    /\ IF mode /= "recovering" /\ Len(cmd_q) < MaxQueue
       THEN TRUE
       ELSE UNCHANGED <<retry_used, halve_used, win_gen, committed, need_facts>>

\* Every delivered event is acknowledged exactly once, in strict order (B5,
\* S-H, S-M) — live panes acknowledge too. A non-increasing sequence is a
\* protocol violation: the pane is lost rather than silently tolerated. An
\* event arriving after recovery was decided is stored but drives nothing.
ApplyCurrentFacts ==
    /\ IF evt_seq <= last_evt_seq
       THEN /\ mode' = "lost" /\ LoseCore
            /\ UNCHANGED <<cmd_q, last_evt_seq, observed_seq>>
       ELSE /\ last_evt_seq' = evt_seq /\ observed_seq' = evt_seq
            /\ facts_gen' = evt_gen /\ need_facts' = 0
            /\ IF RecoveryPending
               THEN UNCHANGED <<mode, win_seq, win_gen, ack, committed,
                               refresh, deadline, drawn_gen, cmd_q, staged,
                               pending_nav, nav_delta>>
               ELSE IF evt_anchor /= "committed" /\ committed = 1
                    THEN /\ committed' = 0 /\ win_seq' = 0 /\ win_gen' = 0
                         /\ ack' = "none" /\ drawn_gen' = 0
                         \* QueueRecoverStep writes pending_nav/nav_delta;
                         \* they must not also sit in the tail UNCHANGED.
                         /\ QueueRecoverStep /\ UNCHANGED <<mode, cmd_q>>
                    ELSE /\ refresh' = IF mode = "viewing" /\ committed = 1
                                       THEN 1 ELSE refresh
                         /\ drawn_gen' = IF evt_gen > drawn_gen THEN 0
                                         ELSE drawn_gen
                         /\ UNCHANGED <<mode, win_seq, win_gen, ack,
                                        committed, deadline, cmd_q, staged,
                                        pending_nav, nav_delta>>

FactsArrive ==
    /\ evt_pending = 1 /\ evt_pending' = 0
    \* A consumed event's payload has no reader; HostEmitEvent overwrites it.
    \* Keep evt_seq: it is the independent event-sequence watermark.
    /\ evt_gen' = 0 /\ evt_anchor' = "committed"
    /\ IF mode = "lost"
       THEN UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                        committed, refresh, deadline, drawn_gen, cmd_q,
                        last_evt_seq, observed_seq, pending_nav, nav_delta,
                        staged>>
       ELSE ApplyCurrentFacts

    /\ IF mode /= "lost" /\ mode' = "lost" THEN TRUE
       ELSE UNCHANGED <<read_flight, read_issue_gen, retry_used, halve_used, recover_done>>
    /\ UNCHANGED <<next_seq, outcomes, evt_seq, host_gen, host_pending, host_committed, host_read_watermark, host_dirty, demand, proj>>

\* The owner admits the staged step onto the wire queue — one ordered slot.
\* Busy/Full keeps it staged for a later attempt; admission binds the step
\* to its wire state exactly like production `command_admitted`: a Read
\* becomes the in-flight sequence, an Ack enters Flight, a Recover begins
\* Recovering and clears the intent/flight state. Sequence consumption
\* already happened at mint — an unadmitted staged read dropped later never
\* leaves flight residue.
AdmitStaged ==
    /\ mode /= "lost"
    /\ staged /= <<>> /\ Len(cmd_q) < MaxQueue
    \* No-payload Recover retains the same tag at admission; Read/Ack keep
    \* their sequence and generation (production `command_admitted`).
    /\ cmd_q' = Append(cmd_q, IF staged[1][1] = "recover"
                              THEN RecoverStep ELSE staged[1])
    /\ staged' = <<>>
    /\ IF staged[1][1] = "recover"
       THEN /\ RecoverAdmitted
            /\ UNCHANGED ack
       ELSE IF staged[1][1] = "read"
            THEN /\ read_flight' = staged[1][2]
                 /\ UNCHANGED <<mode, pending_nav, nav_delta, read_issue_gen,
                                refresh, deadline, ack>>
            ELSE /\ ack' = "flight"
                 /\ UNCHANGED <<mode, pending_nav, nav_delta, read_flight,
                                read_issue_gen, refresh, deadline>>

    /\ UNCHANGED <<facts_gen, win_seq, next_seq, recover_done, drawn_gen,
                   outcomes, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>
    /\ IF StagedIsRecover THEN TRUE
       ELSE UNCHANGED <<retry_used, halve_used, win_gen, committed, need_facts>>

\* Install requires all gates: recovery not decided, active mode, the
\* outcome's sequence matches the live flight, and the window carries
\* current facts (generation stands in for the geometry/column gate — a
\* stale-geometry window follows StaleWindow).
InstallWindow ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "read"
    /\ outcomes[1].r = "window"
    /\ ~RecoveryPending /\ mode \in {"entering", "viewing"}
    /\ read_flight /= 0 /\ read_flight = outcomes[1].sq
    /\ outcomes[1].g = facts_gen /\ need_facts = 0
    /\ win_seq' = outcomes[1].sq /\ win_gen' = outcomes[1].g
    /\ read_flight' = 0 /\ read_issue_gen' = 0
    /\ ack' = "pending"
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, next_seq, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* A window against superseded facts proves one of two things. If the held
\* facts advanced past the generation this read was issued under, a kind-53
\* transition already landed mid-flight — that transition is the host's pin
\* release, so the flight resolves and the next read proceeds under the
\* fresh generation. If the facts did not move, the host still owes the
\* event that describes the window's generation: the flight releases and a
\* bounded wait begins — never an indefinite one, never a fabricated window.
StaleWindow ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "read"
    /\ outcomes[1].r = "window"
    /\ ~RecoveryPending /\ mode \in {"entering", "viewing"}
    /\ read_flight /= 0 /\ read_flight = outcomes[1].sq
    /\ (outcomes[1].g /= facts_gen \/ need_facts = 1)
    /\ read_flight' = 0 /\ read_issue_gen' = 0
    /\ IF facts_gen /= read_issue_gen /\ need_facts = 0
       THEN /\ need_facts' = 0 /\ deadline' = deadline
       ELSE /\ need_facts' = 1
            /\ deadline' = IF deadline = 0 THEN MaxDeadline ELSE deadline
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, win_seq, win_gen, ack, next_seq,
                   pending_nav, nav_delta, committed, refresh, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* A window that matches no live flight cannot prove which pending it
\* settled (e.g. a HostFaultWindow) — recovery is minted, never silent
\* adoption. The flight it might have belonged to stays bound until the
\* Recover itself is admitted; queue_recover never clears it early.
StrayWindow ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "read"
    /\ outcomes[1].r = "window"
    /\ ~RecoveryPending /\ mode \in {"entering", "viewing"}
    /\ (outcomes[1].sq /= read_flight \/ read_flight = 0)
    /\ QueueRecoverStep
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, committed,
                   retry_used, halve_used, recover_done, drawn_gen, cmd_q,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* A read outcome that can no longer match — recovery already decided, no
\* core to apply to, or a rejection naming a flight that is not live — is
\* absorbed without effect: production read_outcome returns default on
\* recovery_pending() or a flight mismatch, and window() returns default on
\* recovery_pending(). Nothing about the live flight is cleared by a drop.
DropOutcome ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "read"
    /\ \/ RecoveryPending
       \/ mode \in {"live", "lost"}
       \/ /\ mode \in {"entering", "viewing"}
          /\ outcomes[1].r \in {"typed", "invalid", "limit", "hostrej"}
          /\ outcomes[1].sq /= read_flight
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

AckEmit ==
    /\ ack = "pending" /\ mode \in {"entering", "viewing"} /\ staged = <<>>
    /\ staged' = <<<<"ack", win_seq, win_gen>>>>

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, outcomes,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* Only an exact accepted match promotes the pending anchor to committed —
\* and with it the pane to Viewing (the Entering-to-Viewing promotion lives
\* here, matching ack_outcome in the core). The outcome's sequence must
\* name the installed window, exactly like production's request_sequence
\* comparison.
AckAccepted ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "ack"
    /\ outcomes[1].a = "accepted"
    /\ ~RecoveryPending /\ mode \in {"entering", "viewing"}
    /\ ack = "flight" /\ win_seq = outcomes[1].sq
    /\ ack' = "accepted" /\ committed' = 1 /\ deadline' = 0
    /\ mode' = "viewing"
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, refresh,
                   retry_used, halve_used, recover_done, drawn_gen, cmd_q,
                   staged, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

\* Acknowledgement rejection — including an evicted anchor — loses the
\* committed anchor: recovery is minted; the position is never faked from
\* stale offsets (B4).
AckRejected ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "ack"
    /\ outcomes[1].a \in {"rejected", "evicted"}
    /\ ~RecoveryPending /\ mode \in {"entering", "viewing"}
    /\ ack = "flight" /\ win_seq = outcomes[1].sq
    /\ ack' = "none" /\ committed' = 0 /\ win_seq' = 0 /\ win_gen' = 0
    /\ drawn_gen' = 0
    /\ QueueRecoverStep
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, read_flight, next_seq,
                   read_issue_gen, retry_used, halve_used, recover_done,
                   cmd_q, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

\* An ack outcome that can no longer match — recovery decided, no core to
\* apply to, wrong lifecycle state, or a window it does not name — is
\* absorbed without effect, exactly like production ack_outcome's early
\* return.
DropAckOutcome ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "ack"
    /\ \/ RecoveryPending
       \/ mode \in {"live", "lost"}
       \/ /\ mode \in {"entering", "viewing"}
          /\ (ack /= "flight" \/ win_seq /= outcomes[1].sq)
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* Typed stale rejections retry once inside the same intent budget; a
\* second failure recovers (B2, M2). The outcome must name the live flight.
ReadTypedRetry ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "read"
    /\ outcomes[1].r = "typed"
    /\ ~RecoveryPending /\ mode \in {"entering", "viewing"}
    /\ read_flight = outcomes[1].sq
    /\ IF retry_used = 0
       THEN /\ retry_used' = 1 /\ need_facts' = 1
            /\ deadline' = IF deadline = 0 THEN MaxDeadline ELSE deadline
            /\ UNCHANGED <<staged, pending_nav, nav_delta, refresh>>
       ELSE /\ QueueRecoverStep
            /\ UNCHANGED <<retry_used, need_facts>>
    /\ read_flight' = 0 /\ read_issue_gen' = 0
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, win_seq, win_gen, ack, next_seq,
                   committed, halve_used, recover_done, drawn_gen, cmd_q,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* A typed invalid-field rejection — a reused sequence under the host's
\* watermark, or a read blocked by the pending pin — is not a protocol
\* fault and not retried: it recovers like every other non-stale typed
\* refusal.
ReadInvalid ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "read"
    /\ outcomes[1].r = "invalid"
    /\ ~RecoveryPending /\ mode \in {"entering", "viewing"}
    /\ read_flight = outcomes[1].sq
    /\ read_flight' = 0 /\ read_issue_gen' = 0
    /\ QueueRecoverStep
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   next_seq, committed, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

\* An opaque host rejection — a state race the wire cannot classify —
\* recovers rather than probing with another read or killing the pane
\* (B2/M2).
ReadHostReject ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "read"
    /\ outcomes[1].r = "hostrej"
    /\ ~RecoveryPending /\ mode \in {"entering", "viewing"}
    /\ read_flight = outcomes[1].sq
    /\ read_flight' = 0 /\ read_issue_gen' = 0
    /\ QueueRecoverStep
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   next_seq, committed, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

ReadLimit ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "read"
    /\ outcomes[1].r = "limit"
    /\ ~RecoveryPending /\ mode \in {"entering", "viewing"}
    /\ read_flight = outcomes[1].sq
    /\ IF halve_used = 0
       THEN /\ halve_used' = 1 /\ refresh' = 1
            /\ UNCHANGED <<staged, pending_nav, nav_delta, deadline>>
       ELSE /\ QueueRecoverStep /\ UNCHANGED halve_used
    /\ read_flight' = 0 /\ read_issue_gen' = 0
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   next_seq, committed, retry_used, recover_done, drawn_gen,
                   cmd_q, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

\* ---------------- executor (ordered admitted channel) ----------------

\* One worker drains cmd_q strictly in order. Each execution runs its host
\* legs and produces its outcome atomically; the outcome appends a typed
\* slot behind anything already published or blocked. Lost arms append
\* nothing — production bypasses publication for them.

ExecuteEnter ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) = EnterStep /\ WorkerFree
    /\ cmd_q' = Tail(cmd_q)
    /\ \/ /\ demand' = "viewing" /\ proj' = "paused" /\ host_dirty' = 1
          \* SetHistoryDemand(viewing) marks dirty even on a silent
          \* terminal — the first event comes.
          /\ outcomes' = Append(outcomes, [type |-> "enter", e |-> "applied",
                                           sq |-> 0, g |-> 0, r |-> "none",
                                           a |-> "none", v |-> "none"])
          /\ UNCHANGED mode
       \/ /\ outcomes' = Append(outcomes, [type |-> "enter", e |-> "rejected",
                                           sq |-> 0, g |-> 0, r |-> "none",
                                           a |-> "none", v |-> "none"])
          /\ UNCHANGED <<demand, proj, mode, host_dirty>>
       \/ /\ mode' = "lost" /\ LoseCore /\ UNCHANGED <<demand, proj, host_dirty, outcomes>>

    /\ IF mode' = "lost" THEN TRUE
       ELSE UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight, read_issue_gen, pending_nav, nav_delta, committed, refresh, deadline, retry_used, halve_used, recover_done, drawn_gen, staged>>
    /\ UNCHANGED <<next_seq, evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen, host_pending, host_committed, host_read_watermark>>

ExecuteRead ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) \in ReadCommands
    /\ WorkerFree
    /\ LET r == Head(cmd_q) IN
       /\ cmd_q' = Tail(cmd_q)
       /\ IF r[2] <= host_read_watermark
          THEN \* The attachment's watermark rejects reuse — a typed
               \* invalid-field refusal, like rows.zig's sequence gate.
               /\ outcomes' = Append(outcomes, [type |-> "read", r |-> "invalid",
                                                sq |-> r[2], g |-> 0,
                                                a |-> "none", e |-> "none",
                                                v |-> "none"])
               /\ UNCHANGED <<host_pending, host_read_watermark>>
          ELSE IF host_pending /= 0
               THEN \* Blocked by the pending pin — again a typed refusal,
                    \* never an opaque fault.
                    /\ outcomes' = Append(outcomes, [type |-> "read",
                                                     r |-> "invalid",
                                                     sq |-> r[2], g |-> 0,
                                                     a |-> "none", e |-> "none",
                                                     v |-> "none"])
                    /\ UNCHANGED <<host_pending, host_read_watermark>>
               ELSE IF r[3] /= host_gen \/ demand /= "viewing"
                    THEN /\ outcomes' = Append(outcomes, [type |-> "read",
                                                          r |-> "typed",
                                                          sq |-> r[2], g |-> 0,
                                                          a |-> "none",
                                                          e |-> "none",
                                                          v |-> "none"])
                         /\ UNCHANGED <<host_pending, host_read_watermark>>
                    ELSE \E res \in {"window", "typed", "limit", "hostrej"} :
                         /\ outcomes' = Append(outcomes,
                                [type |-> "read", r |-> res, sq |-> r[2],
                                 g |-> IF res = "window" THEN r[3] ELSE 0,
                                 a |-> "none", e |-> "none", v |-> "none"])
                         /\ host_pending' = IF res = "window" THEN r[2] ELSE 0
                         /\ host_read_watermark' = IF res = "window" THEN r[2]
                                                       ELSE host_read_watermark

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_committed, host_dirty,
                   demand, proj>>

\* An acknowledgement is accepted only when it binds the exact pending
\* sequence and generation (the abstraction folds revision into generation);
\* even then the host may report the anchor evicted. Anything else is a
\* typed rejection.
ExecuteAck ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) \in AckCommands
    /\ WorkerFree
    /\ LET a == Head(cmd_q) IN
       /\ cmd_q' = Tail(cmd_q)
       /\ IF host_pending = a[2] /\ a[3] = host_gen
          THEN /\ host_pending' = 0
               /\ \/ /\ outcomes' = Append(outcomes, [type |-> "ack",
                                                      a |-> "accepted",
                                                      sq |-> a[2], g |-> 0,
                                                      r |-> "none",
                                                      e |-> "none",
                                                      v |-> "none"])
                     \* Only presence is read (HostEvict); the exact issued
                     \* sequence remains in the outcome, window and watermark.
                     /\ host_committed' = 1
                  \/ /\ outcomes' = Append(outcomes, [type |-> "ack",
                                                      a |-> "evicted",
                                                      sq |-> a[2], g |-> 0,
                                                      r |-> "none",
                                                      e |-> "none",
                                                      v |-> "none"])
                     /\ UNCHANGED host_committed
          ELSE /\ outcomes' = Append(outcomes, [type |-> "ack",
                                                a |-> "rejected",
                                                sq |-> a[2], g |-> 0,
                                                r |-> "none", e |-> "none",
                                                v |-> "none"])
               /\ UNCHANGED <<host_pending, host_committed>>

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_read_watermark, host_dirty,
                   demand, proj>>

ExecuteRecover ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) = RecoverStep /\ WorkerFree
    /\ cmd_q' = Tail(cmd_q)
    /\ \/ /\ demand' = "following" /\ proj' = "active" /\ host_pending' = 0
          /\ outcomes' = Append(outcomes, RecoverSlot)
          /\ UNCHANGED mode
       \/ /\ mode' = "lost" /\ LoseCore /\ UNCHANGED <<demand, proj, host_pending,
                                          outcomes>>

    /\ IF mode' = "lost" THEN TRUE
       ELSE UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight, read_issue_gen, pending_nav, nav_delta, committed, refresh, deadline, retry_used, halve_used, recover_done, drawn_gen, staged>>
    /\ UNCHANGED <<next_seq, evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen, host_committed, host_read_watermark, host_dirty>>

ExecuteInput ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) = InputStep /\ WorkerFree
    /\ cmd_q' = Tail(cmd_q) /\ UNCHANGED outcomes

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, staged, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj>>

\* ---------------- outcome consumption (FIFO head only) ----------------

\* Applied entries are state-neutral: production enter_outcome(true)
\* returns the default effect whatever the mode — the consumer pops the
\* slot and moves on.
ConsumeEnterApplied ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "enter"
    /\ outcomes[1].e = "applied"
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* A rejected entry disposes the episode while a core exists to dispose —
\* production enter_outcome(false) reports live and the core is dropped.
\* Live and lost panes have no core left; those outcomes drain through
\* DropDeadOutcome instead.
ConsumeEnterRejected ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "enter"
    /\ outcomes[1].e = "rejected"
    /\ mode \in {"entering", "viewing", "recovering"}
    /\ mode' = "live" /\ ResetCore
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<facts_gen, cmd_q, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

\* The recovery demands were applied; the first frame after them ends
\* Recovering. Production recovered() sets the applied flag unconditionally
\* — harmless wherever it lands because frame_applied checks the mode.
ConsumeRecovered ==
    /\ Len(outcomes) > 0 /\ outcomes[1].type = "recover"
    /\ mode \in {"entering", "viewing", "recovering"}
    /\ outcomes' = Tail(outcomes)
    /\ recover_done' = 1
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, drawn_gen, cmd_q, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj>>

\* Outcomes for enter/recover commands landing where no core survives to
\* apply them — the pane is Live or lost — drain silently.
DropDeadOutcome ==
    /\ Len(outcomes) > 0 /\ mode \in {"live", "lost"}
    /\ outcomes[1].type \in {"enter", "recover"}
    /\ outcomes' = Tail(outcomes)

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* Only a frame arriving after the recovery demands were applied ends
\* Recovering: a frame already in flight before them cannot (T10). The
\* staged slot dies with the core; the admitted queue and outcomes are the
\* wire's and survive.
FrameArrive ==
    /\ mode = "recovering" /\ recover_done = 1
    /\ mode' = "live" /\ ResetCore
    /\ UNCHANGED <<facts_gen, cmd_q, outcomes, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj>>

\* ---------------- deadlines, resize, detach, host environment ----------------

DeadlineTick ==
    /\ deadline > 1 /\ deadline' = deadline - 1
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, retry_used, halve_used,
                   recover_done, drawn_gen, cmd_q, staged, outcomes,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

\* The facts budget expired: recovery is minted. A staged Recover already
\* cleared the deadline, so this cannot double-mint.
DeadlineExpire ==
    /\ deadline = 1 /\ mode \in {"entering", "viewing"}
    /\ QueueRecoverStep
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, committed,
                   retry_used, halve_used, recover_done, drawn_gen, cmd_q,
                   outcomes, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

\* A client-side resize invalidates the painted window: old columns never
\* paint again, an unadmitted or in-flight acknowledgement resolves nothing
\* under the new geometry, the unadmitted staged step minted under it is
\* dropped unless it is a Recover, the intent budget re-arms for the fresh
\* wait episode, and fresh facts are awaited before the next read. The
\* admitted queue and the produced outcomes are the wire's — they survive.
ClientResize ==
    /\ mode \in {"entering", "viewing"}
    /\ (win_seq /= 0 \/ committed = 1 \/ need_facts = 0)
    /\ win_seq' = 0 /\ win_gen' = 0 /\ drawn_gen' = 0 /\ need_facts' = 1
    /\ read_issue_gen' = 0
    /\ ack' = IF ack \in {"pending", "flight"} THEN "none" ELSE ack
    /\ staged' = IF StagedIsRecover THEN staged ELSE <<>>
    /\ retry_used' = 0 /\ halve_used' = 0
    /\ refresh' = IF committed = 1 THEN 1 ELSE refresh
    /\ deadline' = IF deadline = 0 THEN MaxDeadline ELSE deadline

    /\ UNCHANGED <<mode, facts_gen, read_flight, next_seq, pending_nav,
                   nav_delta, committed, recover_done, cmd_q, outcomes,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

PaneDetach ==
    /\ mode /= "lost" /\ mode' = "lost" /\ LoseCore
    /\ UNCHANGED <<next_seq, cmd_q, outcomes, evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen, host_pending, host_committed, host_read_watermark, host_dirty, demand, proj>>

\* Read-sequence exhaustion is a terminal pane fault, never a wraparound.
\* Only genuinely outstanding, still-unminted read demand triggers it —
\* every other mint gate must already pass so the sequence bound alone is
\* the blocker: no live flight, no pending acknowledgement, no facts wait,
\* and no staged step already covering the demand it was minted for.
SeqExhaust ==
    /\ mode \in {"entering", "viewing"} /\ next_seq > MaxSeq
    /\ facts_gen > 0 /\ read_flight = 0 /\ staged = <<>>
    /\ ack \in {"none", "accepted"} /\ need_facts = 0
    /\ (mode = "entering" \/ pending_nav /= "none" \/ refresh = 1)
    /\ mode' = "lost" /\ LoseCore
    /\ UNCHANGED <<next_seq, cmd_q, outcomes, evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen, host_pending, host_committed, host_read_watermark, host_dirty, demand, proj>>

\* ---------------- host environment (unfair: silence is real) ----------------

\* The host emits HistoryChanged only while the demand is viewing,
\* unannounced history state exists, and no read outcome is still in flight
\* or pending in the cell — the suppression contract of events.zig. The
\* single shared EventCell means a pending read answer also delays event
\* deposit; the model keeps that suppression as the "no read slot
\* outstanding" guard rather than pretending the client can see it.
HostEmitEvent ==
    /\ demand = "viewing" /\ host_dirty = 1 /\ host_pending = 0
    /\ ~(\E i \in DOMAIN outcomes : outcomes[i].type = "read")
    /\ evt_pending = 0 /\ evt_seq < MaxEvent
    /\ evt_pending' = 1 /\ evt_seq' = evt_seq + 1 /\ evt_gen' = host_gen
    \* Both absent-anchor tags take the same recovery branch. Preserve that
    \* branch, representing "none" and "evicted" by "evicted".
    /\ evt_anchor' \in {"committed", "evicted"}
    /\ host_dirty' = 0

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   outcomes, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark, demand,
                   proj>>

HostResize ==
    /\ host_gen < MaxGen
    /\ host_gen' = host_gen + 1 /\ host_pending' = 0 /\ host_dirty' = 1

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   outcomes, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_committed,
                   host_read_watermark, demand, proj>>

HostEvict ==
    /\ host_committed /= 0 /\ host_committed' = 0 /\ host_dirty' = 1

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   outcomes, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_read_watermark, demand, proj>>

\* A faulty row payload can carry a sequence no read asked for. Corrupt an
\* existing, unconsumed window, in either the published or blocked slot.
\* The single worker owns production: this action cannot invent another
\* outcome or occupy capacity independently of executing an admitted Read.
\* Moving payload corruption from ExecuteRead to this step preserves the
\* response's FIFO position and Host pin. sq = 0 represents any alien
\* sequence outside the issued domain, not a literal accepted wire zero.
HostFaultWindow ==
    /\ \E i \in DOMAIN outcomes :
        /\ outcomes[i].type = "read" /\ outcomes[i].r = "window"
        /\ outcomes[i].sq /= 0
        /\ outcomes' = [outcomes EXCEPT ![i].sq = 0]

    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, staged,
                   evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj>>

Next ==
    \/ PressEnter \/ EmitTail \/ EmitCommitted
    \/ \E d \in {-2, -1, 1, 2} : ScrollDelta(d)
    \/ PressJump \/ PressExit \/ KeyTyped
    \/ FactsArrive \/ AdmitStaged
    \/ InstallWindow \/ StaleWindow \/ StrayWindow \/ DropOutcome
    \/ AckEmit \/ AckAccepted \/ AckRejected \/ DropAckOutcome
    \/ ReadTypedRetry \/ ReadInvalid \/ ReadHostReject \/ ReadLimit
    \/ ExecuteEnter \/ ExecuteRead \/ ExecuteAck \/ ExecuteRecover \/ ExecuteInput
    \/ ConsumeEnterApplied \/ ConsumeEnterRejected \/ ConsumeRecovered
    \/ DropDeadOutcome \/ FrameArrive
    \/ DeadlineTick \/ DeadlineExpire \/ ClientResize \/ PaneDetach \/ SeqExhaust
    \/ HostEmitEvent \/ HostResize \/ HostEvict \/ HostFaultWindow

\* Fairness on client/executor/delivery actions only; host event emission,
\* resize, eviction, and fault windows may stay silent forever.
Spec == Init /\ [][Next]_vars
    /\ WF_vars(AdmitStaged)
    /\ WF_vars(ExecuteEnter) /\ WF_vars(ExecuteRead) /\ WF_vars(ExecuteAck)
    /\ WF_vars(ExecuteRecover) /\ WF_vars(ExecuteInput)
    /\ WF_vars(ConsumeEnterApplied) /\ WF_vars(ConsumeEnterRejected)
    /\ WF_vars(ConsumeRecovered) /\ WF_vars(DropDeadOutcome)
    /\ WF_vars(FrameArrive) /\ WF_vars(FactsArrive)
    /\ WF_vars(InstallWindow) /\ WF_vars(StaleWindow) /\ WF_vars(StrayWindow)
    /\ WF_vars(DropOutcome) /\ WF_vars(DropAckOutcome)
    /\ WF_vars(AckEmit) /\ WF_vars(AckAccepted) /\ WF_vars(AckRejected)
    /\ WF_vars(ReadTypedRetry) /\ WF_vars(ReadInvalid) /\ WF_vars(ReadHostReject)
    /\ WF_vars(ReadLimit)
    /\ WF_vars(DeadlineTick) /\ WF_vars(DeadlineExpire)

\* ---------------- invariants ----------------

TypeOK ==
    /\ mode \in Modes /\ facts_gen \in 0..MaxGen /\ need_facts \in 0..1
    /\ win_seq \in 0..MaxSeq /\ win_gen \in 0..MaxGen /\ ack \in AckStates
    /\ read_flight \in 0..MaxSeq /\ next_seq \in 1..(MaxSeq + 1)
    /\ read_issue_gen \in 0..MaxGen
    /\ pending_nav \in NavKinds /\ nav_delta \in (0 - MaxDelta)..MaxDelta
    /\ committed \in 0..1 /\ refresh \in 0..1 /\ deadline \in 0..MaxDeadline
    /\ retry_used \in 0..1 /\ halve_used \in 0..1 /\ recover_done \in 0..1
    /\ drawn_gen \in 0..MaxGen /\ Len(cmd_q) <= MaxQueue
    /\ \A i \in DOMAIN cmd_q : cmd_q[i] \in AdmittedCommands
    /\ staged = <<>>
       \/ (Len(staged) = 1 /\ staged[1] \in StagedSteps)
    /\ Len(outcomes) <= 2
    /\ \A i \in DOMAIN outcomes : outcomes[i] \in OutcomeSlots
    /\ evt_pending \in 0..1 /\ evt_seq \in 0..MaxEvent /\ evt_gen \in 0..MaxGen
    /\ evt_anchor \in Anchors /\ last_evt_seq \in 0..MaxEvent
    /\ observed_seq \in 0..MaxEvent /\ host_gen \in 1..MaxGen
    /\ host_pending \in 0..MaxSeq /\ host_committed \in 0..MaxSeq
    /\ host_read_watermark \in 0..MaxSeq /\ host_dirty \in 0..1
    /\ demand \in Demands /\ proj \in Projs

\* S-A: an acknowledgement waits or flies only for an installed window.
AckBoundToWindow == ack \in {"pending", "flight"} => win_seq /= 0

\* An installed window is always inside its acknowledgement lifecycle.
InstalledNeedsAck == win_seq /= 0 => ack /= "none"

\* Every emit leaves exactly one staged Read carrying the just-consumed
\* sequence, the flight still unbound (it binds at admission), the ack
\* lifecycle clear, and current facts — the accurate post-state witness of
\* the read-mint gate. (Replaces the earlier EmitReadUnblocked, which
\* asserted post-state emptiness that a minted step cannot satisfy and an
\* outcome-cell check the core cannot see.)
EmitLeavesStagedRead ==
    [][(EmitTail \/ EmitCommitted)
        => /\ (StagedIsRead)'
           /\ staged'[1][2] = next_seq' - 1
           /\ read_flight' = 0
           /\ ack' \in {"none", "accepted"}
           /\ need_facts' = 0 /\ facts_gen' > 0]_vars

\* S-C emit guard: a Committed read is emitted only with a committed anchor.
CommittedReadGuard ==
    [][EmitCommitted => committed' = 1]_vars

\* B3: every emitted committed step stays inside the bounded delta domain.
BoundedDelta ==
    [][EmitCommitted
        => (nav_delta - nav_delta') \in (0 - DeltaBound)..DeltaBound]_vars

\* S-W: install happens only in Entering/Viewing, against current facts,
\* and clears the flight it answers.
InstallGates ==
    [][InstallWindow
        => (mode' \in {"entering", "viewing"} /\ read_flight' = 0
            /\ win_seq' /= 0 /\ win_gen' = facts_gen')]_vars

\* S-H: every accepted event is acknowledged exactly once — observed always
\* equals the last accepted sequence.
EventAcked == observed_seq = last_evt_seq

\* S-M: a delivered event's sequence always exceeds the last accepted one.
MonotoneEvents == evt_pending = 1 => evt_seq > last_evt_seq

\* S-P: the painted surface never shows a generation behind current facts.
SurfaceCurrent == drawn_gen \in {0, facts_gen}

\* The host's pending pin always names the sequence it last served —
\* exactly the watermark: a pin can never point at a read the host never
\* served, and the pin and watermark move together until the pin clears.
HostPendingKnown ==
    host_pending /= 0 => host_pending = host_read_watermark

\* The host's watermark can only name a sequence the client already
\* emitted: a read is never served for a sequence that was never minted,
\* and the monotone client never emits one at or below the watermark.
ReadSeqMonotone == host_read_watermark < next_seq

\* A staged read can only exist while the flight is clear: mint requires
\* read_flight = 0 and admission is the only thing that ever binds it —
\* the state-level witness that makes AdmittedReadNeedsClearFlight hold.
StagedReadImpliesClearFlight == StagedIsRead => read_flight = 0

\* Ordering, not one-flight: no read sequence still on the wire — staged,
\* admitted, or produced — is newer than the live flight. A read that
\* outlived its episode is strictly older, because emission is monotone
\* and a newer mint requires the older flight cleared first.
ReadSeqsNeverAhead ==
    read_flight = 0
    \/ /\ (StagedIsRead => staged[1][2] <= read_flight)
       /\ \A i \in DOMAIN cmd_q :
           (cmd_q[i] \in ReadCommands) => cmd_q[i][2] <= read_flight
       /\ \A i \in DOMAIN outcomes :
           (outcomes[i].type = "read") => outcomes[i].sq <= read_flight

\* Non-reuse: every read sequence appearing in the staged slot, the
\* admitted queue, or a produced outcome is distinct — minted sequences
\* are never reissued, so no two claims on the wire share one.
ReadSeqsUnique ==
    /\ \A i, j \in DOMAIN cmd_q :
        (i /= j /\ cmd_q[i] \in ReadCommands /\ cmd_q[j] \in ReadCommands)
            => cmd_q[i][2] /= cmd_q[j][2]
    /\ \A i \in DOMAIN cmd_q :
        (cmd_q[i] \in ReadCommands)
            => ~(StagedIsRead /\ staged[1][2] = cmd_q[i][2])
    /\ \A i \in DOMAIN cmd_q, k \in DOMAIN outcomes :
        (cmd_q[i] \in ReadCommands /\ outcomes[k].type = "read")
            => cmd_q[i][2] /= outcomes[k].sq
    \* Outcome-vs-outcome pairs compare only issued reads (sq > 0): sq = 0
    \* records are HostFaultWindow's corrupted responses. Their alien
    \* sequences do not identify issued requests.
    /\ \A i, k \in DOMAIN outcomes :
        (i /= k /\ outcomes[i].type = "read" /\ outcomes[k].type = "read"
         /\ outcomes[i].sq > 0 /\ outcomes[k].sq > 0)
            => outcomes[i].sq /= outcomes[k].sq
    /\ StagedIsRead
        => ~(\E k \in DOMAIN outcomes :
              outcomes[k].type = "read" /\ outcomes[k].sq = staged[1][2])

\* ---------------- temporal properties ----------------

\* Only the single executor can produce a result, by consuming an admitted
\* command. In particular a faulty payload cannot act as a second producer
\* that repeatedly fills the blocked slot and starves weakly fair execution.
OutcomeProductionOwned ==
    [][Len(outcomes') > Len(outcomes)
        => /\ Len(cmd_q') = Len(cmd_q) - 1
           /\ (ExecuteEnter \/ ExecuteRead \/ ExecuteAck \/ ExecuteRecover)]_vars

\* S-C: only an exact accepted acknowledgement can promote an anchor.
\* Once committed, the host tracks it independently of a newer window's
\* acknowledgement lifecycle; resize may cancel that newer Ack while the
\* previously accepted anchor survives. Init has no committed anchor.
CommittedNeedsAck ==
    [][committed = 0 /\ committed' = 1 => AckAccepted]_vars

\* A dead incarnation has no Client core to revive.
PaneIncarnationFence ==
    [][mode = "lost" => mode' = "lost"]_vars

\* Entering always resolves — facts, or the client-side deadline — even
\* when the host stays silent.
EnteredResolves == (mode = "entering") ~> (mode /= "entering")

\* The real one-flight safety claim, stated on the admission edge itself:
\* a staged Read may only be admitted while no flight is live — never as
\* an overlapping second flight. Asserted as a step assertion on the
\* action's prestate, not smuggled into AdmitStaged's guard.
AdmittedReadNeedsClearFlight ==
    [][StagedIsRead /\ AdmitStaged => read_flight = 0]_vars

\* B6/S-I, stated on the admission edge: every ordinary input newly
\* appended to the admitted queue is already covered — either the core
\* was in Recovering (reachable only through an admitted Recover), or a
\* recover sits earlier in the post-state queue. The state-level
\* InputOrdered could not express this: after ResetCore/re-entry it had
\* lost the witness and flagged legal old inputs. Admission appends at
\* the tail and the worker removes from the head; no action removes and
\* appends in the same step, so i > Len(cmd_q) identifies exactly the
\* newly admitted entries. A past Recover that already executed —
\* consumed from the wire — needs no re-witness: mode = "recovering" in
\* the prestate is only reachable through one. If a future queue action
\* ever removes and appends in one step, this property must be revisited.
InputAppendedUnderCover ==
    [][\A i \in DOMAIN cmd_q' :
        (i > Len(cmd_q) /\ cmd_q'[i] = InputStep)
            => (mode = "recovering"
                \/ \E j \in 1..(i - 1) : cmd_q'[j] = RecoverStep)]_vars

=============================================================================
