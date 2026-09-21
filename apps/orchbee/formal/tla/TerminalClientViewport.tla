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
   never a valid one. *)
EXTENDS Naturals, Sequences, TLC

CONSTANTS MaxSeq, MaxEvent, MaxGen, MaxDelta, DeltaBound, MaxQueue, MaxDeadline

Modes == {"live", "entering", "viewing", "recovering", "lost"}
AckStates == {"none", "pending", "flight", "accepted"}
NavKinds == {"none", "delta", "jump"}
RespKinds == {"none", "window", "typed", "invalid", "limit", "hostrej"}
AckResps == {"none", "accepted", "rejected", "evicted"}
Anchors == {"committed", "evicted", "none"}
Demands == {"following", "viewing"}
Projs == {"active", "paused"}
ReadCommands == {<<"read", s, g>> : s \in 1..MaxSeq, g \in 1..MaxGen}
AckCommands == {<<"ack", s, g>> : s \in 1..MaxSeq, g \in 1..MaxGen}
WireSteps == ReadCommands \cup AckCommands

VARIABLES
    mode,           \* client viewport mode
    facts_gen,      \* newest validated HistoryChanged generation (0 = none)
    need_facts,     \* waiting on a fresh event before the next read
    win_seq,        \* installed window's read sequence (0 = none)
    win_gen,        \* installed window's generation
    ack,            \* RowsApplied lifecycle for the installed window
    read_flight,    \* emitted read awaiting its host outcome (0 = none)
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
    cmd_q,          \* admitted command order: "enter" "recover" "input" reads acks
    req,            \* read request in flight to the host (0 = none)
    resp,           \* host answer waiting for the client (RespKinds)
    win_gen_flight, \* generation carried by the in-flight window answer
    ack_out,        \* RowsApplied in flight to the host (0 = none)
    ack_resp,       \* host answer for the ack: AckResps
    enter_resp,     \* enter outcome: "none" | "applied" | "rejected"
    recover_resp,   \* recover outcome: "none" | "applied"
    delivered,      \* input byte delivered (ordering witness)
    evt_pending,    \* a HistoryChanged event awaits client intake
    evt_seq,        \* sequence of the pending event
    evt_gen,        \* generation of the pending event
    evt_anchor,     \* anchor status the pending event reports
    last_evt_seq,   \* last accepted event sequence
    observed_seq,   \* last HistoryObserved the client emitted
    host_gen,       \* host-side current generation
    host_pending,   \* host-side pending read pin (0 = none)
    host_committed, \* host-side committed anchor sequence (0 = none)
    host_read_watermark, \* host-side per-attachment last served read sequence
    host_dirty,     \* host-side unannounced history state exists
    demand,         \* host-side history demand
    proj,           \* host-side projection demand
    emitted_delta,  \* delta of the last committed emit (bound witness)
    action          \* last transition name (emit-time guard witness)

vars == <<mode, facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
          next_seq, read_issue_gen, pending_nav, nav_delta, committed,
          refresh, deadline, retry_used, halve_used, recover_done, drawn_gen,
          cmd_q, req, resp, win_gen_flight, ack_out, ack_resp, enter_resp,
          recover_resp, delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
          last_evt_seq, observed_seq, host_gen, host_pending, host_committed,
          host_read_watermark, host_dirty, demand, proj, emitted_delta,
          action>>

Init ==
    /\ mode = "live" /\ facts_gen = 0 /\ need_facts = 0
    /\ win_seq = 0 /\ win_gen = 0 /\ ack = "none" /\ read_flight = 0
    /\ next_seq = 1 /\ read_issue_gen = 0 /\ pending_nav = "none"
    /\ nav_delta = 0 /\ committed = 0
    /\ refresh = 0 /\ deadline = 0 /\ retry_used = 0 /\ halve_used = 0
    /\ recover_done = 0 /\ drawn_gen = 0 /\ cmd_q = <<>> /\ req = 0
    /\ resp = "none" /\ win_gen_flight = 0 /\ ack_out = 0 /\ ack_resp = "none"
    /\ enter_resp = "none" /\ recover_resp = "none" /\ delivered = 0
    /\ evt_pending = 0 /\ evt_seq = 0 /\ evt_gen = 0 /\ evt_anchor = "committed"
    /\ last_evt_seq = 0 /\ observed_seq = 0 /\ host_gen = 1 /\ host_pending = 0
    /\ host_committed = 0 /\ host_read_watermark = 0 /\ host_dirty = 0
    /\ demand = "following" /\ proj = "active"
    /\ emitted_delta = 0 /\ action = "init"

\* ---------------- client emit gates ----------------

ReadUnblocked ==
    /\ read_flight = 0              \* at most one read in flight (S-R)
    /\ ack \in {"none", "accepted"} \* never while an ack waits or flies (B1)
    /\ resp = "none"                \* no unprocessed host answer
    /\ facts_gen > 0                \* reads need current facts
    /\ need_facts = 0
    /\ next_seq <= MaxSeq
    /\ Len(cmd_q) < MaxQueue

\* Recovery drops every unadmitted read/ack step — they were minted under
\* facts or geometry that no longer hold — then queues exactly one Recover.
RECURSIVE DropSteps(_)
DropSteps(q) ==
    IF Len(q) = 0 THEN <<>>
    ELSE IF Head(q) \in WireSteps THEN DropSteps(Tail(q))
    ELSE <<Head(q)>> \o DropSteps(Tail(q))

QueueRecover ==
    LET q == DropSteps(cmd_q) IN
    IF \E i \in DOMAIN q : q[i] = "recover" THEN q ELSE Append(q, "recover")

\* A live-mode or fresh-episode reset: the per-incarnation next_seq is NOT
\* part of it — the watermark owner outlives episodes (rows.zig's
\* last_row_request_sequence is attachment-scoped), so re-entry resumes above
\* the last issued value.
ResetCore ==
    /\ win_seq' = 0 /\ win_gen' = 0 /\ ack' = "none" /\ read_flight' = 0
    /\ read_issue_gen' = 0 /\ pending_nav' = "none" /\ nav_delta' = 0
    /\ committed' = 0 /\ refresh' = 0 /\ need_facts' = 0 /\ deadline' = 0
    /\ retry_used' = 0 /\ halve_used' = 0 /\ drawn_gen' = 0 /\ recover_done' = 0
    /\ UNCHANGED next_seq

\* ---------------- client-local actions ----------------

PressEnter ==
    /\ mode = "live" /\ Len(cmd_q) < MaxQueue
    /\ mode' = "entering" /\ cmd_q' = Append(cmd_q, "enter")
    /\ deadline' = MaxDeadline /\ need_facts' = 1 /\ recover_done' = 0
    /\ action' = "PressEnter"
    /\ UNCHANGED <<facts_gen, win_seq, win_gen, ack, read_flight, next_seq,
                   read_issue_gen, pending_nav, nav_delta, committed, refresh,
                   retry_used, halve_used, drawn_gen, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj, emitted_delta>>

EmitTail ==
    /\ mode = "entering" \/ (mode = "viewing" /\ pending_nav = "jump")
    /\ ReadUnblocked
    /\ read_flight' = next_seq /\ read_issue_gen' = facts_gen
    /\ next_seq' = next_seq + 1
    /\ cmd_q' = Append(cmd_q, <<"read", next_seq, facts_gen>>)
    /\ pending_nav' = "none" /\ nav_delta' = 0 /\ refresh' = 0
    /\ emitted_delta' = 0 /\ action' = "EmitTail"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   committed, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

EmitCommitted ==
    /\ mode = "viewing" /\ committed = 1
    /\ (pending_nav = "delta" \/ refresh = 1)
    /\ ReadUnblocked
    /\ LET chunk == IF nav_delta > DeltaBound THEN DeltaBound
                    ELSE IF nav_delta < (0 - DeltaBound) THEN 0 - DeltaBound
                    ELSE nav_delta
       IN /\ emitted_delta' = chunk
          /\ nav_delta' = nav_delta - chunk
          /\ pending_nav' = IF nav_delta - chunk = 0 THEN "none" ELSE "delta"
    /\ refresh' = 0
    /\ read_flight' = next_seq /\ read_issue_gen' = facts_gen
    /\ next_seq' = next_seq + 1
    /\ cmd_q' = Append(cmd_q, <<"read", next_seq, facts_gen>>)
    /\ action' = "EmitCommitted"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   committed, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj>>

\* A fresh navigation intent re-arms the per-intent retry/halving budget;
\* merging into a pending or in-flight intent shares its budget (production:
\* the flags reset only when nav finds no pending and no in-flight intent).
ScrollDelta(d) ==
    /\ mode \in {"entering", "viewing"}
    /\ pending_nav /= "jump"    \* absolute intent owns the next read
    /\ nav_delta' = IF nav_delta + d > MaxDelta THEN MaxDelta
                    ELSE IF nav_delta + d < (0 - MaxDelta) THEN 0 - MaxDelta
                    ELSE nav_delta + d
    /\ pending_nav' = IF nav_delta' = 0 THEN "none" ELSE "delta"
    /\ IF pending_nav = "none" /\ read_flight = 0
       THEN /\ retry_used' = 0 /\ halve_used' = 0
       ELSE UNCHANGED <<retry_used, halve_used>>
    /\ action' = "ScrollDelta"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, committed, refresh,
                   deadline, recover_done, drawn_gen, cmd_q, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj, emitted_delta>>

PressJump ==
    /\ mode \in {"entering", "viewing"}
    /\ pending_nav' = "jump" /\ nav_delta' = 0 /\ action' = "PressJump"
    /\ IF pending_nav = "none" /\ read_flight = 0
       THEN /\ retry_used' = 0 /\ halve_used' = 0
       ELSE UNCHANGED <<retry_used, halve_used>>
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, committed, refresh,
                   deadline, recover_done, drawn_gen, cmd_q, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj, emitted_delta>>

PressExit ==
    /\ mode \in {"entering", "viewing"}
    /\ cmd_q' = QueueRecover /\ mode' = "recovering" /\ action' = "PressExit"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj, emitted_delta>>

\* B6: the recover leg is admitted ahead of the input on the ordered channel;
\* the byte never slips between the two demand legs. A Recover already staged
\* counts as decided — no second one is minted.
KeyTyped ==
    /\ mode \in {"entering", "viewing", "recovering"} /\ Len(cmd_q) < MaxQueue
    /\ LET q == DropSteps(cmd_q) IN
       IF \E i \in DOMAIN q : q[i] = "recover"
       THEN /\ cmd_q' = Append(q, "input") /\ UNCHANGED mode
       ELSE /\ cmd_q' = Append(Append(q, "recover"), "input")
            /\ mode' = "recovering"
    /\ action' = "KeyTyped"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj, emitted_delta>>

\* Every delivered event is acknowledged exactly once, in strict order (B5,
\* S-H, S-M) — live panes acknowledge too. A non-increasing sequence is a
\* protocol violation: the pane is lost rather than silently tolerated. An
\* event arriving after recovery was decided is stored but drives nothing.
FactsArrive ==
    /\ evt_pending = 1 /\ evt_pending' = 0
    /\ IF evt_seq <= last_evt_seq
       THEN /\ mode' = "lost"
            /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack,
                           committed, refresh, drawn_gen, cmd_q, last_evt_seq,
                           observed_seq>>
       ELSE /\ last_evt_seq' = evt_seq /\ observed_seq' = evt_seq
            /\ facts_gen' = evt_gen /\ need_facts' = 0
            /\ refresh' = IF mode = "viewing" /\ committed = 1 THEN 1 ELSE refresh
            /\ IF evt_anchor /= "committed" /\ committed = 1 /\ mode /= "recovering"
               THEN /\ mode' = "recovering" /\ cmd_q' = QueueRecover
                    /\ committed' = 0 /\ win_seq' = 0 /\ win_gen' = 0
                    /\ ack' = "none" /\ drawn_gen' = 0
               ELSE /\ mode' = mode /\ cmd_q' = cmd_q
                    /\ committed' = committed /\ win_seq' = win_seq
                    /\ win_gen' = win_gen /\ ack' = ack
                    /\ drawn_gen' = IF evt_gen > drawn_gen THEN 0 ELSE drawn_gen
    /\ action' = "FactsArrive"
    /\ UNCHANGED <<read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, deadline, retry_used, halve_used, recover_done,
                   req, resp, win_gen_flight, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_seq, evt_gen, evt_anchor,
                   host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* Install requires all gates: active mode, the answer matches the live
\* flight, and the window carries current facts (generation stands in for the
\* geometry/column gate — a stale-geometry window follows StaleWindow).
InstallWindow ==
    /\ resp = "window" /\ mode \in {"entering", "viewing"}
    /\ read_flight /= 0 /\ read_flight = req
    /\ win_gen_flight = facts_gen /\ need_facts = 0
    /\ win_seq' = req /\ win_gen' = win_gen_flight
    /\ read_flight' = 0 /\ read_issue_gen' = 0 /\ req' = 0 /\ resp' = "none"
    /\ ack' = "pending"
    /\ action' = "InstallWindow"
    /\ UNCHANGED <<mode, facts_gen, need_facts, next_seq, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, win_gen_flight,
                   ack_out, ack_resp, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* A window against superseded facts proves one of two things. If the held
\* facts advanced past the generation this read was issued under, a kind-53
\* transition already landed mid-flight — that transition is the host's pin
\* release, so the flight resolves and the next read proceeds under the fresh
\* generation. If the facts did not move, the host still owes the event that
\* describes the window's generation: the flight releases and a bounded wait
\* begins — never an indefinite one, never a fabricated window.
StaleWindow ==
    /\ resp = "window" /\ mode \in {"entering", "viewing"}
    /\ read_flight /= 0 /\ read_flight = req
    /\ (win_gen_flight /= facts_gen \/ need_facts = 1)
    /\ read_flight' = 0 /\ read_issue_gen' = 0 /\ req' = 0 /\ resp' = "none"
    /\ IF facts_gen /= read_issue_gen /\ need_facts = 0
       THEN /\ need_facts' = 0 /\ deadline' = deadline
       ELSE /\ need_facts' = 1
            /\ deadline' = IF deadline = 0 THEN MaxDeadline ELSE deadline
    /\ action' = "StaleWindow"
    /\ UNCHANGED <<mode, facts_gen, win_seq, win_gen, ack, next_seq,
                   pending_nav, nav_delta, committed, refresh, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, win_gen_flight,
                   ack_out, ack_resp, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* A window that matches no live flight cannot prove which pending it settled
\* (e.g. a HostFaultWindow) — recovery, never silent adoption.
StrayWindow ==
    /\ resp = "window" /\ mode \in {"entering", "viewing"}
    /\ (read_flight /= req \/ read_flight = 0)
    /\ read_flight' = 0 /\ req' = 0 /\ resp' = "none"
    /\ cmd_q' = QueueRecover /\ mode' = "recovering" /\ action' = "StrayWindow"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, next_seq,
                   read_issue_gen, pending_nav, nav_delta, committed, refresh,
                   deadline, retry_used, halve_used, recover_done, drawn_gen,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj, emitted_delta>>

\* Any host answer arriving while the core is gone or recovering is dropped;
\* its pending pin is cleared by the demand transition it raced. A rejection
\* for a flight the core no longer owns is absorbed the same way — silently.
DropOutcome ==
    /\ \/ /\ resp /= "none" /\ mode \in {"recovering", "lost", "live"}
          /\ resp' = "none" /\ req' = 0 /\ read_flight' = 0
       \/ /\ resp \in {"typed", "invalid", "limit", "hostrej"}
          /\ mode \in {"entering", "viewing"} /\ read_flight /= req
          /\ resp' = "none" /\ req' = 0 /\ UNCHANGED read_flight
    /\ action' = "DropOutcome"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   next_seq, read_issue_gen, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj, emitted_delta>>

AckEmit ==
    /\ ack = "pending" /\ mode \in {"entering", "viewing"}
    /\ Len(cmd_q) < MaxQueue
    /\ ack' = "flight" /\ ack_out' = win_seq
    /\ cmd_q' = Append(cmd_q, <<"ack", win_seq, win_gen>>)
    /\ action' = "AckEmit"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj, emitted_delta>>

\* Only an exact accepted match promotes the pending anchor to committed —
\* and with it the pane to Viewing (the Entering-to-Viewing promotion lives
\* here, matching ack_outcome in the core).
AckAccepted ==
    /\ ack_resp = "accepted" /\ ack = "flight"
    /\ mode \in {"entering", "viewing"}
    /\ ack' = "accepted" /\ committed' = 1 /\ deadline' = 0
    /\ mode' = "viewing"
    /\ ack_out' = 0 /\ ack_resp' = "none" /\ action' = "AckAccepted"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, refresh,
                   retry_used, halve_used, recover_done, drawn_gen, cmd_q, req,
                   resp, win_gen_flight, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* Acknowledgement rejection — including an evicted anchor — loses the
\* committed anchor: recovery with a notice; the position is never faked
\* from stale offsets (B4).
AckRejected ==
    /\ ack_resp \in {"rejected", "evicted"} /\ ack = "flight"
    /\ mode \in {"entering", "viewing"}
    /\ ack' = "none" /\ committed' = 0 /\ win_seq' = 0 /\ win_gen' = 0
    /\ drawn_gen' = 0 /\ ack_out' = 0 /\ ack_resp' = "none"
    /\ cmd_q' = QueueRecover /\ mode' = "recovering" /\ action' = "AckRejected"
    /\ UNCHANGED <<facts_gen, need_facts, read_flight, next_seq,
                   read_issue_gen, pending_nav, nav_delta, refresh, deadline,
                   retry_used, halve_used, recover_done, req, resp,
                   win_gen_flight, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* An ack outcome that can no longer match — wrong lifecycle state, wrong
\* installed window, or a recovery/dead mode — is absorbed without effect.
DropAckOutcome ==
    /\ ack_resp /= "none"
    /\ (mode \in {"recovering", "lost", "live"}
        \/ ack /= "flight" \/ win_seq /= ack_out)
    /\ ack_resp' = "none" /\ ack_out' = 0 /\ action' = "DropAckOutcome"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, req, resp,
                   win_gen_flight, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* Typed stale rejections retry once inside the same intent budget; a second
\* failure recovers (B2, M2). The outcome must name the live flight.
ReadTypedRetry ==
    /\ resp = "typed" /\ mode \in {"entering", "viewing"} /\ read_flight = req
    /\ IF retry_used = 0
       THEN /\ retry_used' = 1 /\ need_facts' = 1
            /\ deadline' = IF deadline = 0 THEN MaxDeadline ELSE deadline
            /\ cmd_q' = cmd_q /\ mode' = mode
       ELSE /\ cmd_q' = QueueRecover /\ mode' = "recovering"
            /\ UNCHANGED <<retry_used, need_facts, deadline>>
    /\ read_flight' = 0 /\ read_issue_gen' = 0 /\ req' = 0 /\ resp' = "none"
    /\ action' = "ReadTypedRetry"
    /\ UNCHANGED <<facts_gen, win_seq, win_gen, ack, next_seq, pending_nav,
                   nav_delta, committed, refresh, halve_used, recover_done,
                   drawn_gen, win_gen_flight, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj, emitted_delta>>

\* A typed invalid-field rejection — a reused sequence under the host's
\* watermark, or a read blocked by the pending pin — is not a protocol fault
\* and not retried: it recovers like every other non-stale typed refusal.
ReadInvalid ==
    /\ resp = "invalid" /\ mode \in {"entering", "viewing"} /\ read_flight = req
    /\ read_flight' = 0 /\ read_issue_gen' = 0 /\ req' = 0 /\ resp' = "none"
    /\ cmd_q' = QueueRecover /\ mode' = "recovering"
    /\ action' = "ReadInvalid"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, next_seq,
                   pending_nav, nav_delta, committed, refresh, deadline,
                   retry_used, halve_used, recover_done, drawn_gen,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj, emitted_delta>>

\* An opaque host rejection — a state race the wire cannot classify —
\* recovers rather than probing with another read or killing the pane (B2/M2).
ReadHostReject ==
    /\ resp = "hostrej" /\ mode \in {"entering", "viewing"} /\ read_flight = req
    /\ read_flight' = 0 /\ read_issue_gen' = 0 /\ req' = 0 /\ resp' = "none"
    /\ cmd_q' = QueueRecover /\ mode' = "recovering"
    /\ action' = "ReadHostReject"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, next_seq,
                   pending_nav, nav_delta, committed, refresh, deadline,
                   retry_used, halve_used, recover_done, drawn_gen,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj, emitted_delta>>

ReadLimit ==
    /\ resp = "limit" /\ mode \in {"entering", "viewing"} /\ read_flight = req
    /\ IF halve_used = 0
       THEN /\ halve_used' = 1 /\ refresh' = 1
            /\ cmd_q' = cmd_q /\ mode' = mode
       ELSE /\ cmd_q' = QueueRecover /\ mode' = "recovering"
            /\ UNCHANGED <<halve_used, refresh>>
    /\ read_flight' = 0 /\ read_issue_gen' = 0 /\ req' = 0 /\ resp' = "none"
    /\ action' = "ReadLimit"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, next_seq,
                   pending_nav, nav_delta, committed, deadline, retry_used,
                   recover_done, drawn_gen, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj, emitted_delta>>

\* ---------------- executor (ordered command channel) ----------------

ExecuteEnter ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) = "enter"
    /\ cmd_q' = Tail(cmd_q)
    /\ \/ /\ demand' = "viewing" /\ proj' = "paused" /\ enter_resp' = "applied"
          /\ host_dirty' = 1    \* SetHistoryDemand(viewing) marks dirty even
                              \* on a silent terminal — the first event comes.
          /\ UNCHANGED mode
       \/ /\ enter_resp' = "rejected" /\ UNCHANGED <<demand, proj, mode, host_dirty>>
       \/ /\ mode' = "lost" /\ enter_resp' = "none"
          /\ UNCHANGED <<demand, proj, host_dirty>>
    /\ action' = "ExecuteEnter"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, ack_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   emitted_delta>>

ExecuteRead ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) \in ReadCommands
    /\ LET r == Head(cmd_q) IN
       /\ cmd_q' = Tail(cmd_q) /\ req' = r[2]
       /\ IF r[2] <= host_read_watermark
          THEN \* The attachment's watermark rejects reuse — a typed
               \* invalid-field refusal, like rows.zig's sequence gate.
               /\ resp' = "invalid"
               /\ UNCHANGED <<host_pending, win_gen_flight, host_read_watermark>>
          ELSE IF host_pending /= 0
               THEN \* Blocked by the pending pin — again a typed refusal,
                    \* never an opaque fault.
                    /\ resp' = "invalid"
                    /\ UNCHANGED <<host_pending, win_gen_flight, host_read_watermark>>
               ELSE IF r[3] /= host_gen \/ demand /= "viewing"
                    THEN /\ resp' = "typed"
                         /\ UNCHANGED <<host_pending, win_gen_flight,
                                        host_read_watermark>>
                    ELSE /\ resp' \in {"window", "typed", "limit", "hostrej"}
                         /\ host_pending' = IF resp' = "window" THEN r[2] ELSE 0
                         /\ win_gen_flight' = IF resp' = "window" THEN r[3] ELSE 0
                         /\ host_read_watermark' = IF resp' = "window" THEN r[2]
                                                   ELSE host_read_watermark
    /\ action' = "ExecuteRead"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_committed, host_dirty, demand, proj, emitted_delta>>

\* An acknowledgement is accepted only when it binds the exact pending
\* sequence and generation (the abstraction folds revision into generation);
\* even then the host may report the anchor evicted. Anything else is a
\* typed rejection.
ExecuteAck ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) \in AckCommands
    /\ LET a == Head(cmd_q) IN
       /\ cmd_q' = Tail(cmd_q)
       /\ IF host_pending = a[2] /\ a[3] = host_gen
          THEN /\ host_pending' = 0
               /\ \/ /\ ack_resp' = "accepted" /\ host_committed' = a[2]
                  \/ /\ ack_resp' = "evicted" /\ UNCHANGED host_committed
          ELSE /\ ack_resp' = "rejected"
               /\ UNCHANGED <<host_pending, host_committed>>
    /\ action' = "ExecuteAck"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, req, resp,
                   win_gen_flight, ack_out, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_read_watermark, host_dirty,
                   demand, proj, emitted_delta>>

ExecuteRecover ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) = "recover"
    /\ cmd_q' = Tail(cmd_q)
    /\ \/ /\ demand' = "following" /\ proj' = "active" /\ host_pending' = 0
          /\ recover_resp' = "applied" /\ UNCHANGED mode
       \/ /\ mode' = "lost" /\ recover_resp' = "none"
          /\ UNCHANGED <<demand, proj, host_pending>>
    /\ action' = "ExecuteRecover"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_committed, host_read_watermark, host_dirty,
                   emitted_delta>>

ExecuteInput ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) = "input"
    /\ cmd_q' = Tail(cmd_q) /\ delivered' = 1 /\ action' = "ExecuteInput"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* ---------------- outcome consumption ----------------

ConsumeEnterApplied ==
    /\ enter_resp = "applied" /\ enter_resp' = "none"
    /\ action' = "ConsumeEnterApplied"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, req, resp,
                   win_gen_flight, ack_out, ack_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

ConsumeEnterRejected ==
    /\ enter_resp = "rejected" /\ enter_resp' = "none"
    /\ mode' = "live" /\ ResetCore /\ action' = "ConsumeEnterRejected"
    /\ UNCHANGED <<facts_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj, emitted_delta>>

ConsumeRecovered ==
    /\ recover_resp = "applied" /\ recover_resp' = "none"
    /\ recover_done' = 1 /\ action' = "ConsumeRecovered"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, drawn_gen, cmd_q, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* Only a frame arriving after the recovery demands were applied ends
\* Recovering: a frame already in flight before them cannot (T10).
FrameArrive ==
    /\ mode = "recovering" /\ recover_done = 1
    /\ mode' = "live" /\ ResetCore /\ action' = "FrameArrive"
    /\ UNCHANGED <<facts_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, enter_resp, recover_resp, delivered, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* ---------------- deadlines, resize, detach, host environment ----------------

DeadlineTick ==
    /\ deadline > 1 /\ deadline' = deadline - 1 /\ action' = "DeadlineTick"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, retry_used, halve_used,
                   recover_done, drawn_gen, cmd_q, req, resp, win_gen_flight,
                   ack_out, ack_resp, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

DeadlineExpire ==
    /\ deadline = 1 /\ mode \in {"entering", "viewing"}
    /\ deadline' = 0 /\ cmd_q' = QueueRecover /\ mode' = "recovering"
    /\ action' = "DeadlineExpire"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, committed,
                   refresh, retry_used, halve_used, recover_done, drawn_gen,
                   req, resp, win_gen_flight, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark,
                   host_dirty, demand, proj, emitted_delta>>

\* A client-side resize invalidates the painted window: old columns never
\* paint again, an unadmitted or in-flight acknowledgement resolves nothing
\* under the new geometry, staged wire steps minted under it are dropped, the
\* intent budget re-arms for the fresh wait episode, and fresh facts are
\* awaited before the next read.
ClientResize ==
    /\ mode \in {"entering", "viewing"}
    /\ (win_seq /= 0 \/ committed = 1 \/ need_facts = 0)
    /\ win_seq' = 0 /\ win_gen' = 0 /\ drawn_gen' = 0 /\ need_facts' = 1
    /\ read_issue_gen' = 0
    /\ ack' = IF ack \in {"pending", "flight"} THEN "none" ELSE ack
    /\ cmd_q' = DropSteps(cmd_q)
    /\ retry_used' = 0 /\ halve_used' = 0
    /\ refresh' = IF committed = 1 THEN 1 ELSE refresh
    /\ deadline' = IF deadline = 0 THEN MaxDeadline ELSE deadline
    /\ action' = "ClientResize"
    /\ UNCHANGED <<mode, facts_gen, read_flight, next_seq, pending_nav,
                   nav_delta, committed, recover_done, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, host_read_watermark, host_dirty, demand,
                   proj, emitted_delta>>

PaneDetach ==
    /\ mode /= "lost" /\ mode' = "lost" /\ action' = "PaneDetach"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, enter_resp, recover_resp, delivered, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* Read-sequence exhaustion is a terminal pane fault, never a wraparound.
SeqExhaust ==
    /\ mode \in {"entering", "viewing"} /\ next_seq > MaxSeq
    /\ facts_gen > 0 /\ read_flight = 0 /\ resp = "none"
    /\ (mode = "entering" \/ pending_nav /= "none" \/ refresh = 1)
    /\ mode' = "lost" /\ action' = "SeqExhaust"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, read_issue_gen, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, enter_resp, recover_resp, delivered, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

\* ---------------- host environment (unfair: silence is real) ----------------

\* The host emits HistoryChanged only while the demand is viewing, unannounced
\* history state exists, and neither a read flight nor a served-read pending
\* pin is outstanding — the suppression contract of events.zig.
HostEmitEvent ==
    /\ demand = "viewing" /\ host_dirty = 1 /\ req = 0 /\ host_pending = 0
    /\ evt_pending = 0 /\ evt_seq < MaxEvent
    /\ evt_pending' = 1 /\ evt_seq' = evt_seq + 1 /\ evt_gen' = host_gen
    /\ evt_anchor' \in Anchors
    /\ host_dirty' = 0
    /\ action' = "HostEmitEvent"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, host_read_watermark, demand,
                   proj, emitted_delta>>

HostResize ==
    /\ host_gen < MaxGen
    /\ host_gen' = host_gen + 1 /\ host_pending' = 0 /\ host_dirty' = 1
    /\ action' = "HostResize"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_committed,
                   host_read_watermark, demand, proj, emitted_delta>>

HostEvict ==
    /\ host_committed /= 0 /\ host_committed' = 0 /\ host_dirty' = 1
    /\ action' = "HostEvict"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_read_watermark, demand, proj, emitted_delta>>

\* An adversarial/faulty host can answer with a window no read asked for:
\* the sequence gate sends it to recovery.
HostFaultWindow ==
    /\ resp = "none" /\ resp' = "window" /\ req' = 0
    /\ win_gen_flight' = host_gen /\ action' = "HostFaultWindow"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, read_issue_gen, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, ack_out,
                   ack_resp, enter_resp, recover_resp, delivered, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_gen, host_pending, host_committed,
                   host_read_watermark, host_dirty, demand, proj,
                   emitted_delta>>

Next ==
    \/ PressEnter \/ EmitTail \/ EmitCommitted
    \/ \E d \in {-2, -1, 1, 2} : ScrollDelta(d)
    \/ PressJump \/ PressExit \/ KeyTyped
    \/ FactsArrive \/ InstallWindow \/ StaleWindow \/ StrayWindow \/ DropOutcome
    \/ AckEmit \/ AckAccepted \/ AckRejected \/ DropAckOutcome
    \/ ReadTypedRetry \/ ReadInvalid \/ ReadHostReject \/ ReadLimit
    \/ ExecuteEnter \/ ExecuteRead \/ ExecuteAck \/ ExecuteRecover \/ ExecuteInput
    \/ ConsumeEnterApplied \/ ConsumeEnterRejected \/ ConsumeRecovered \/ FrameArrive
    \/ DeadlineTick \/ DeadlineExpire \/ ClientResize \/ PaneDetach \/ SeqExhaust
    \/ HostEmitEvent \/ HostResize \/ HostEvict \/ HostFaultWindow

\* Fairness on client/executor/delivery actions only; host event emission,
\* resize, eviction, and fault windows may stay silent forever.
Spec == Init /\ [][Next]_vars
    /\ WF_vars(ExecuteEnter) /\ WF_vars(ExecuteRead) /\ WF_vars(ExecuteAck)
    /\ WF_vars(ExecuteRecover) /\ WF_vars(ExecuteInput)
    /\ WF_vars(ConsumeEnterApplied) /\ WF_vars(ConsumeEnterRejected)
    /\ WF_vars(ConsumeRecovered) /\ WF_vars(FrameArrive) /\ WF_vars(FactsArrive)
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
    /\ drawn_gen \in 0..MaxGen /\ Len(cmd_q) <= MaxQueue + 2
    /\ req \in 0..MaxSeq /\ resp \in RespKinds /\ win_gen_flight \in 0..MaxGen
    /\ ack_out \in 0..MaxSeq /\ ack_resp \in AckResps
    /\ enter_resp \in {"none", "applied", "rejected"}
    /\ recover_resp \in {"none", "applied"} /\ delivered \in 0..1
    /\ evt_pending \in 0..1 /\ evt_seq \in 0..MaxEvent /\ evt_gen \in 0..MaxGen
    /\ evt_anchor \in Anchors /\ last_evt_seq \in 0..MaxEvent
    /\ observed_seq \in 0..MaxEvent /\ host_gen \in 1..MaxGen
    /\ host_pending \in 0..MaxSeq /\ host_committed \in 0..MaxSeq
    /\ host_read_watermark \in 0..MaxSeq /\ host_dirty \in 0..1
    /\ demand \in Demands /\ proj \in Projs
    /\ emitted_delta \in (0 - DeltaBound)..DeltaBound

\* S-R: at most one read in flight; the wire request is exactly that flight.
\* An already-admitted read may still execute after its episode ended (the
\* core is gone but the wire command was admitted) — the host answers it and
\* the stale outcome is dropped; only dead modes loosen the binding.
OneReadFlight ==
    req = 0 \/ req = read_flight \/ mode \in {"recovering", "lost", "live"}

\* S-A: an acknowledgement waits or flies only for an installed window.
AckBoundToWindow == ack \in {"pending", "flight"} => win_seq /= 0

\* An installed window is always inside its acknowledgement lifecycle.
InstalledNeedsAck == win_seq /= 0 => ack /= "none"

\* S-C: a committed anchor exists only while its acknowledgement lifecycle is
\* live (pending, in flight, or accepted).
CommittedNeedsAck == committed = 1 => ack /= "none"

\* B1 guard witness: no read is ever emitted while an ack waits or flies, or
\* while an earlier host answer is still unprocessed.
EmitReadUnblocked ==
    action \in {"EmitTail", "EmitCommitted"}
        => (ack \in {"none", "accepted"} /\ resp = "none" /\ need_facts = 0)

\* S-C emit guard: a Committed read is emitted only with a committed anchor.
CommittedReadGuard == action = "EmitCommitted" => committed = 1

\* B3: every emitted committed step stays inside the bounded delta domain.
BoundedDelta == emitted_delta \in (0 - DeltaBound)..DeltaBound

\* S-W: install happens only in Entering/Viewing, against current facts, and
\* clears the flight it answers.
InstallGates ==
    action = "InstallWindow"
        => (mode \in {"entering", "viewing"} /\ read_flight = 0
            /\ win_seq /= 0 /\ win_gen = facts_gen)

\* S-H: every accepted event is acknowledged exactly once — observed always
\* equals the last accepted sequence.
EventAcked == observed_seq = last_evt_seq

\* S-M: a delivered event's sequence always exceeds the last accepted one.
MonotoneEvents == evt_pending = 1 => evt_seq > last_evt_seq

\* B6/S-I: an input on the admitted channel is always preceded by a recover
\* on that channel, or recovery was already applied, or the pane no longer
\* needs one (it recovered to live, or is already recovering).
InputOrdered ==
    \A i \in DOMAIN cmd_q :
        cmd_q[i] = "input"
            => \/ \E j \in 1..(i - 1) : cmd_q[j] = "recover"
               \/ recover_done = 1
               \/ mode \in {"recovering", "live"}

\* S-P: the painted surface never shows a generation behind current facts.
SurfaceCurrent == drawn_gen \in {0, facts_gen}

\* The host's pending pin always names the sequence it last served — exactly
\* the watermark: a pin can never point at a read the host never served, and
\* the pin and watermark move together until the pin clears.
HostPendingKnown ==
    host_pending /= 0 => host_pending = host_read_watermark

\* The host's watermark can only name a sequence the client already emitted:
\* a read is never served for a sequence that was never minted, and the
\* monotone client never emits one at or below the watermark.
ReadSeqMonotone == host_read_watermark < next_seq

\* ---------------- progress ----------------

\* Entering always resolves — facts, or the client-side deadline — even when
\* the host stays silent.
EnteredResolves == (mode = "entering") ~> (mode /= "entering")

=============================================================================
