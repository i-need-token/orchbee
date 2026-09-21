---------------------- MODULE TerminalClientViewport ----------------------
(* Client-side retained-history viewport for one pane: demand legs, facts
   intake (kind 53), the row-window read/acknowledge handshake, navigation
   coalescing, bounded committed deltas, recovery, and the Host's pending-read
   pin. This is the contract for terminal_session/terminal_viewport.rs;
   see docs/native-terminal-viewport.md.

   Corrections modeled (recovery challenge report): B1 no read while a window
   is pending acknowledgement; B2 host pending/state rejections enter recovery,
   never kill; B3 committed deltas split at +/-DeltaBound; B4 anchor loss
   enters Recovering, position never faked; B5 every valid event is
   acknowledged exactly once; B6 Recover is admitted before ordinary input.
   Fairness is asserted only on client/executor/delivery actions — host event
   emission, resize, eviction, and fault windows stay unfair so silence and
   loss are real behaviours. The window column-count gate is subsumed by the
   generation gate: a stale-geometry window follows the same StaleWindow
   release path. *)
EXTENDS Naturals, Sequences, TLC

CONSTANTS MaxSeq, MaxEvent, MaxGen, MaxDelta, DeltaBound, MaxQueue, MaxDeadline

Modes == {"live", "entering", "viewing", "recovering", "lost"}
AckStates == {"none", "pending", "flight", "accepted"}
NavKinds == {"none", "delta", "jump"}
RespKinds == {"none", "window", "typed", "limit", "hostrej"}
Anchors == {"committed", "evicted"}
Demands == {"following", "viewing"}
Projs == {"active", "paused"}
ReadCommands == {<<"read", s, g>> : s \in 1..MaxSeq, g \in 1..MaxGen}
AckCommands == {<<"ack", s>> : s \in 1..MaxSeq}

VARIABLES
    mode,           \* client viewport mode
    facts_gen,      \* newest validated HistoryChanged generation (0 = none)
    need_facts,     \* waiting on a fresh event before the next read
    win_seq,        \* installed window's read sequence (0 = none)
    win_gen,        \* installed window's generation
    ack,            \* RowsApplied lifecycle for the installed window
    read_flight,    \* emitted read awaiting its host outcome (0 = none)
    next_seq,       \* next read sequence (1..MaxSeq+1; exhaustion is modelled)
    pending_nav,    \* coalesced navigation intent
    nav_delta,      \* merged signed line delta for pending_nav = "delta"
    committed,      \* accepted acknowledgement promoted the pending anchor
    refresh,        \* a fresh Committed{0} read is wanted after new facts
    deadline,       \* armed facts budget (>0); expires at 1
    retry_used,     \* one stale-generation/revision retry per wait episode
    halve_used,     \* one LimitExceeded row-count halving per window
    recover_done,   \* recovery demands applied; a post-recovery frame ends it
    drawn_gen,      \* generation of the painted surface (0 = nothing drawn)
    cmd_q,          \* admitted command order: "enter" "recover" "input" reads acks
    req,            \* read request in flight to the host (0 = none)
    resp,           \* host answer waiting for the client (RespKinds)
    win_gen_flight, \* generation carried by the in-flight window answer
    ack_out,        \* RowsApplied in flight to the host (0 = none)
    ack_resp,       \* host answer for the ack: "none" | "accepted" | "rejected"
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
    demand,         \* host-side history demand
    proj,           \* host-side projection demand
    emitted_delta,  \* delta of the last committed emit (bound witness)
    action          \* last transition name (emit-time guard witness)

vars == <<mode, facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
          next_seq, pending_nav, nav_delta, committed, refresh, deadline,
          retry_used, halve_used, recover_done, drawn_gen, cmd_q, req, resp,
          win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
          delivered, evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
          observed_seq, host_gen, host_pending, host_committed, demand, proj,
          emitted_delta, action>>

Init ==
    /\ mode = "live" /\ facts_gen = 0 /\ need_facts = 0
    /\ win_seq = 0 /\ win_gen = 0 /\ ack = "none" /\ read_flight = 0
    /\ next_seq = 1 /\ pending_nav = "none" /\ nav_delta = 0 /\ committed = 0
    /\ refresh = 0 /\ deadline = 0 /\ retry_used = 0 /\ halve_used = 0
    /\ recover_done = 0 /\ drawn_gen = 0 /\ cmd_q = <<>> /\ req = 0
    /\ resp = "none" /\ win_gen_flight = 0 /\ ack_out = 0 /\ ack_resp = "none"
    /\ enter_resp = "none" /\ recover_resp = "none" /\ delivered = 0
    /\ evt_pending = 0 /\ evt_seq = 0 /\ evt_gen = 0 /\ evt_anchor = "committed"
    /\ last_evt_seq = 0 /\ observed_seq = 0 /\ host_gen = 1 /\ host_pending = 0
    /\ host_committed = 0 /\ demand = "following" /\ proj = "active"
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

QueueRecover ==
    IF mode = "recovering" \/ \E i \in DOMAIN cmd_q : cmd_q[i] = "recover"
    THEN cmd_q ELSE Append(cmd_q, "recover")

ResetCore ==
    /\ win_seq' = 0 /\ win_gen' = 0 /\ ack' = "none" /\ read_flight' = 0
    /\ pending_nav' = "none" /\ nav_delta' = 0 /\ committed' = 0
    /\ refresh' = 0 /\ need_facts' = 0 /\ deadline' = 0 /\ retry_used' = 0
    /\ halve_used' = 0 /\ drawn_gen' = 0 /\ next_seq' = 1

\* ---------------- client-local actions ----------------

PressEnter ==
    /\ mode = "live" /\ Len(cmd_q) < MaxQueue
    /\ mode' = "entering" /\ cmd_q' = Append(cmd_q, "enter")
    /\ deadline' = MaxDeadline /\ need_facts' = 1 /\ recover_done' = 0
    /\ action' = "PressEnter"
    /\ UNCHANGED <<facts_gen, win_seq, win_gen, ack, read_flight, next_seq,
                   pending_nav, nav_delta, committed, refresh, retry_used,
                   halve_used, drawn_gen, req, resp, win_gen_flight, ack_out,
                   ack_resp, enter_resp, recover_resp, delivered, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_gen, host_pending, host_committed, demand, proj,
                   emitted_delta>>

EmitTail ==
    /\ mode = "entering" \/ (mode = "viewing" /\ pending_nav = "jump")
    /\ ReadUnblocked
    /\ read_flight' = next_seq /\ next_seq' = next_seq + 1
    /\ cmd_q' = Append(cmd_q, <<"read", next_seq, facts_gen>>)
    /\ pending_nav' = "none" /\ nav_delta' = 0 /\ refresh' = 0
    /\ emitted_delta' = 0 /\ action' = "EmitTail"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   committed, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj>>

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
    /\ read_flight' = next_seq /\ next_seq' = next_seq + 1
    /\ cmd_q' = Append(cmd_q, <<"read", next_seq, facts_gen>>)
    /\ action' = "EmitCommitted"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   committed, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj>>

ScrollDelta(d) ==
    /\ mode \in {"entering", "viewing"}
    /\ pending_nav /= "jump"    \* absolute intent owns the next read
    /\ nav_delta' = IF nav_delta + d > MaxDelta THEN MaxDelta
                    ELSE IF nav_delta + d < (0 - MaxDelta) THEN 0 - MaxDelta
                    ELSE nav_delta + d
    /\ pending_nav' = IF nav_delta' = 0 THEN "none" ELSE "delta"
    /\ action' = "ScrollDelta"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, committed, refresh, deadline,
                   retry_used, halve_used, recover_done, drawn_gen, cmd_q, req,
                   resp, win_gen_flight, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

PressJump ==
    /\ mode \in {"entering", "viewing"}
    /\ pending_nav' = "jump" /\ nav_delta' = 0 /\ action' = "PressJump"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, committed, refresh, deadline,
                   retry_used, halve_used, recover_done, drawn_gen, cmd_q, req,
                   resp, win_gen_flight, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

PressExit ==
    /\ mode \in {"entering", "viewing"}
    /\ cmd_q' = QueueRecover /\ mode' = "recovering" /\ action' = "PressExit"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, pending_nav, nav_delta, committed, refresh,
                   deadline, retry_used, halve_used, recover_done, drawn_gen,
                   req, resp, win_gen_flight, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

\* B6: the recover leg is admitted ahead of the input on the ordered channel;
\* the byte never slips between the two demand legs.
KeyTyped ==
    /\ mode \in {"entering", "viewing", "recovering"} /\ Len(cmd_q) < MaxQueue
    /\ IF mode = "recovering" \/ \E i \in DOMAIN cmd_q : cmd_q[i] = "recover"
       THEN /\ cmd_q' = Append(cmd_q, "input") /\ UNCHANGED mode
       ELSE /\ cmd_q' = Append(Append(cmd_q, "recover"), "input")
            /\ mode' = "recovering"
    /\ action' = "KeyTyped"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, pending_nav, nav_delta, committed, refresh,
                   deadline, retry_used, halve_used, recover_done, drawn_gen,
                   req, resp, win_gen_flight, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

\* Every delivered event is acknowledged exactly once, in strict order (B5,
\* S-H, S-M). A non-increasing sequence is a protocol violation: the pane is
\* lost rather than silently tolerated.
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
            /\ IF evt_anchor = "evicted" /\ committed = 1
               THEN /\ mode' = "recovering" /\ cmd_q' = QueueRecover
                    /\ committed' = 0 /\ win_seq' = 0 /\ win_gen' = 0
                    /\ ack' = "none" /\ drawn_gen' = 0
               ELSE /\ mode' = mode /\ cmd_q' = cmd_q
                    /\ committed' = committed /\ win_seq' = win_seq
                    /\ win_gen' = win_gen /\ ack' = ack
                    /\ drawn_gen' = IF evt_gen > drawn_gen THEN 0 ELSE drawn_gen
    /\ action' = "FactsArrive"
    /\ UNCHANGED <<read_flight, next_seq, pending_nav, nav_delta, deadline,
                   retry_used, halve_used, recover_done, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_seq, evt_gen, evt_anchor, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

\* Install requires all gates: active mode, the answer matches the live
\* flight, and the window carries current facts (generation stands in for the
\* geometry/column gate — a stale-geometry window follows StaleWindow).
InstallWindow ==
    /\ resp = "window" /\ mode \in {"entering", "viewing"}
    /\ read_flight /= 0 /\ read_flight = req
    /\ win_gen_flight = facts_gen /\ need_facts = 0
    /\ win_seq' = req /\ win_gen' = win_gen_flight
    /\ read_flight' = 0 /\ req' = 0 /\ resp' = "none" /\ ack' = "pending"
    /\ action' = "InstallWindow"
    /\ UNCHANGED <<mode, facts_gen, need_facts, next_seq, pending_nav,
                   nav_delta, committed, refresh, deadline, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, win_gen_flight,
                   ack_out, ack_resp, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   demand, proj, emitted_delta>>

\* A window against superseded facts or geometry proves the host's pending
\* pin was cleared by that transition: the flight is released and fresh facts
\* are awaited — the pending is never replaced by a later read (M6).
StaleWindow ==
    /\ resp = "window" /\ mode \in {"entering", "viewing"}
    /\ read_flight /= 0 /\ read_flight = req
    /\ (win_gen_flight /= facts_gen \/ need_facts = 1)
    /\ read_flight' = 0 /\ req' = 0 /\ resp' = "none" /\ need_facts' = 1
    /\ deadline' = IF deadline = 0 THEN MaxDeadline ELSE deadline
    /\ action' = "StaleWindow"
    /\ UNCHANGED <<mode, facts_gen, win_seq, win_gen, ack, next_seq,
                   pending_nav, nav_delta, committed, refresh, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, win_gen_flight,
                   ack_out, ack_resp, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   demand, proj, emitted_delta>>

\* A window that matches no live flight cannot prove which pending it settled
\* (e.g. a HostFaultWindow) — recovery, never silent adoption.
StrayWindow ==
    /\ resp = "window" /\ mode \in {"entering", "viewing"}
    /\ (read_flight /= req \/ read_flight = 0)
    /\ read_flight' = 0 /\ req' = 0 /\ resp' = "none"
    /\ cmd_q' = QueueRecover /\ mode' = "recovering" /\ action' = "StrayWindow"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, next_seq,
                   pending_nav, nav_delta, committed, refresh, deadline,
                   retry_used, halve_used, recover_done, drawn_gen,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, demand, proj, emitted_delta>>

\* Any host answer arriving while the core is gone or recovering is dropped;
\* its pending pin is cleared by the demand transition it raced.
DropOutcome ==
    /\ resp /= "none" /\ mode \in {"recovering", "lost", "live"}
    /\ resp' = "none" /\ req' = 0 /\ read_flight' = 0 /\ action' = "DropOutcome"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   next_seq, pending_nav, nav_delta, committed, refresh,
                   deadline, retry_used, halve_used, recover_done, drawn_gen,
                   cmd_q, win_gen_flight, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

AckEmit ==
    /\ ack = "pending" /\ mode \in {"entering", "viewing"}
    /\ Len(cmd_q) < MaxQueue
    /\ ack' = "flight" /\ ack_out' = win_seq
    /\ cmd_q' = Append(cmd_q, <<"ack", win_seq>>)
    /\ action' = "AckEmit"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, read_flight,
                   next_seq, pending_nav, nav_delta, committed, refresh,
                   deadline, retry_used, halve_used, recover_done, drawn_gen,
                   req, resp, win_gen_flight, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

AckAccepted ==
    /\ ack_resp = "accepted" /\ ack = "flight"
    /\ ack' = "accepted" /\ committed' = 1 /\ deadline' = 0
    /\ ack_out' = 0 /\ ack_resp' = "none" /\ action' = "AckAccepted"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, read_flight,
                   next_seq, pending_nav, nav_delta, refresh, retry_used,
                   halve_used, recover_done, drawn_gen, cmd_q, req, resp,
                   win_gen_flight, enter_resp, recover_resp, delivered,
                   evt_pending, evt_seq, evt_gen, evt_anchor, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   demand, proj, emitted_delta>>

\* Acknowledgement rejection loses the committed anchor: recovery with a
\* notice; the position is never faked from stale offsets (B4).
AckRejected ==
    /\ ack_resp = "rejected" /\ ack = "flight"
    /\ ack' = "none" /\ committed' = 0 /\ win_seq' = 0 /\ win_gen' = 0
    /\ drawn_gen' = 0 /\ ack_out' = 0 /\ ack_resp' = "none"
    /\ cmd_q' = QueueRecover /\ mode' = "recovering" /\ action' = "AckRejected"
    /\ UNCHANGED <<facts_gen, need_facts, read_flight, next_seq, pending_nav,
                   nav_delta, refresh, deadline, retry_used, halve_used,
                   recover_done, req, resp, win_gen_flight, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

\* Typed stale rejections retry once inside the same facts budget (M3); a
\* second failure — or any non-stale typed rejection — recovers (B2, M2).
ReadTypedRetry ==
    /\ resp = "typed" /\ mode \in {"entering", "viewing"}
    /\ IF retry_used = 0
       THEN /\ retry_used' = 1 /\ need_facts' = 1
            /\ deadline' = IF deadline = 0 THEN MaxDeadline ELSE deadline
            /\ cmd_q' = cmd_q /\ mode' = mode
       ELSE /\ cmd_q' = QueueRecover /\ mode' = "recovering"
            /\ UNCHANGED <<retry_used, need_facts, deadline>>
    /\ read_flight' = 0 /\ req' = 0 /\ resp' = "none"
    /\ action' = "ReadTypedRetry"
    /\ UNCHANGED <<facts_gen, win_seq, win_gen, ack, next_seq, pending_nav,
                   nav_delta, committed, refresh, halve_used, recover_done,
                   drawn_gen, win_gen_flight, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

\* An opaque host rejection — including the pending-blockage/state race —
\* recovers rather than probing with another read or killing the pane (B2/M2).
ReadHostReject ==
    /\ resp = "hostrej" /\ mode \in {"entering", "viewing"}
    /\ read_flight' = 0 /\ req' = 0 /\ resp' = "none"
    /\ cmd_q' = QueueRecover /\ mode' = "recovering"
    /\ action' = "ReadHostReject"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, next_seq,
                   pending_nav, nav_delta, committed, refresh, deadline,
                   retry_used, halve_used, recover_done, drawn_gen,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, demand, proj, emitted_delta>>

ReadLimit ==
    /\ resp = "limit" /\ mode \in {"entering", "viewing"}
    /\ IF halve_used = 0
       THEN /\ halve_used' = 1 /\ refresh' = 1
            /\ cmd_q' = cmd_q /\ mode' = mode
       ELSE /\ cmd_q' = QueueRecover /\ mode' = "recovering"
            /\ UNCHANGED <<halve_used, refresh>>
    /\ read_flight' = 0 /\ req' = 0 /\ resp' = "none"
    /\ action' = "ReadLimit"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, next_seq,
                   pending_nav, nav_delta, committed, deadline, retry_used,
                   recover_done, drawn_gen, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

\* ---------------- executor (ordered command channel) ----------------

ExecuteEnter ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) = "enter"
    /\ cmd_q' = Tail(cmd_q)
    /\ \/ /\ demand' = "viewing" /\ proj' = "paused" /\ enter_resp' = "applied"
          /\ UNCHANGED mode
       \/ /\ enter_resp' = "rejected" /\ UNCHANGED <<demand, proj, mode>>
       \/ /\ mode' = "lost" /\ enter_resp' = "none" /\ UNCHANGED <<demand, proj>>
    /\ action' = "ExecuteEnter"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, pending_nav, nav_delta, committed, refresh,
                   deadline, retry_used, halve_used, recover_done, drawn_gen,
                   req, resp, win_gen_flight, ack_out, ack_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, emitted_delta>>

ExecuteRead ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) \in ReadCommands
    /\ LET r == Head(cmd_q) IN
       /\ cmd_q' = Tail(cmd_q) /\ req' = r[2]
       /\ IF host_pending /= 0
          THEN /\ resp' = "hostrej" /\ UNCHANGED <<host_pending, win_gen_flight>>
          ELSE IF r[3] /= host_gen \/ demand /= "viewing"
               THEN /\ resp' = "typed" /\ UNCHANGED <<host_pending, win_gen_flight>>
               ELSE /\ resp' \in {"window", "typed", "limit", "hostrej"}
                    /\ host_pending' = IF resp' = "window" THEN r[2] ELSE 0
                    /\ win_gen_flight' = IF resp' = "window" THEN r[3] ELSE 0
    /\ action' = "ExecuteRead"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_committed,
                   demand, proj, emitted_delta>>

ExecuteAck ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) \in AckCommands
    /\ LET a == Head(cmd_q) IN
       /\ cmd_q' = Tail(cmd_q)
       /\ IF host_pending = a[2]
          THEN /\ ack_resp' = "accepted" /\ host_pending' = 0
               /\ host_committed' = a[2]
          ELSE /\ ack_resp' = "rejected"
               /\ UNCHANGED <<host_pending, host_committed>>
    /\ action' = "ExecuteAck"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen, demand,
                   proj, emitted_delta>>

ExecuteRecover ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) = "recover"
    /\ cmd_q' = Tail(cmd_q)
    /\ \/ /\ demand' = "following" /\ proj' = "active" /\ host_pending' = 0
          /\ recover_resp' = "applied" /\ UNCHANGED mode
       \/ /\ mode' = "lost" /\ recover_resp' = "none"
          /\ UNCHANGED <<demand, proj, host_pending>>
    /\ action' = "ExecuteRecover"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, pending_nav, nav_delta, committed, refresh,
                   deadline, retry_used, halve_used, recover_done, drawn_gen,
                   req, resp, win_gen_flight, ack_out, ack_resp, enter_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_committed,
                   emitted_delta>>

ExecuteInput ==
    /\ mode /= "lost" /\ Len(cmd_q) > 0 /\ Head(cmd_q) = "input"
    /\ cmd_q' = Tail(cmd_q) /\ delivered' = 1 /\ action' = "ExecuteInput"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

\* ---------------- outcome consumption ----------------

ConsumeEnterApplied ==
    /\ enter_resp = "applied" /\ enter_resp' = "none"
    /\ action' = "ConsumeEnterApplied"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

ConsumeEnterRejected ==
    /\ enter_resp = "rejected" /\ enter_resp' = "none"
    /\ mode' = "live" /\ ResetCore /\ action' = "ConsumeEnterRejected"
    /\ UNCHANGED <<facts_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

ConsumeRecovered ==
    /\ recover_resp = "applied" /\ recover_resp' = "none"
    /\ recover_done' = 1 /\ action' = "ConsumeRecovered"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, drawn_gen, cmd_q,
                   req, resp, win_gen_flight, ack_out, ack_resp, enter_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, demand, proj, emitted_delta>>

\* Only a frame arriving after the recovery demands were applied ends
\* Recovering: a frame already in flight before them cannot (T10).
FrameArrive ==
    /\ mode = "recovering" /\ recover_done = 1
    /\ mode' = "live" /\ ResetCore /\ action' = "FrameArrive"
    /\ UNCHANGED <<facts_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, enter_resp, recover_resp, delivered, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_gen, host_pending, host_committed, demand, proj,
                   emitted_delta>>

\* ---------------- deadlines, resize, detach, host environment ----------------

DeadlineTick ==
    /\ deadline > 1 /\ deadline' = deadline - 1 /\ action' = "DeadlineTick"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, retry_used, halve_used, recover_done, drawn_gen,
                   cmd_q, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

DeadlineExpire ==
    /\ deadline = 1 /\ mode \in {"entering", "viewing"}
    /\ deadline' = 0 /\ cmd_q' = QueueRecover /\ mode' = "recovering"
    /\ action' = "DeadlineExpire"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, pending_nav, nav_delta, committed, refresh,
                   retry_used, halve_used, recover_done, drawn_gen, req, resp,
                   win_gen_flight, ack_out, ack_resp, enter_resp, recover_resp,
                   delivered, evt_pending, evt_seq, evt_gen, evt_anchor,
                   last_evt_seq, observed_seq, host_gen, host_pending,
                   host_committed, demand, proj, emitted_delta>>

\* A client-side resize invalidates the painted window: old columns never
\* paint again, an unadmitted ack is dropped, and fresh facts are awaited
\* before the next read.
ClientResize ==
    /\ mode \in {"entering", "viewing"}
    /\ (win_seq /= 0 \/ committed = 1 \/ need_facts = 0)
    /\ win_seq' = 0 /\ win_gen' = 0 /\ drawn_gen' = 0 /\ need_facts' = 1
    /\ ack' = IF ack = "pending" THEN "none" ELSE ack
    /\ refresh' = IF committed = 1 THEN 1 ELSE refresh
    /\ deadline' = IF deadline = 0 THEN MaxDeadline ELSE deadline
    /\ action' = "ClientResize"
    /\ UNCHANGED <<mode, facts_gen, read_flight, next_seq, pending_nav,
                   nav_delta, committed, retry_used, halve_used, recover_done,
                   cmd_q, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

PaneDetach ==
    /\ mode /= "lost" /\ mode' = "lost" /\ action' = "PaneDetach"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, pending_nav, nav_delta, committed, refresh,
                   deadline, retry_used, halve_used, recover_done, drawn_gen,
                   cmd_q, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

\* Read-sequence exhaustion is a terminal pane fault, never a wraparound.
SeqExhaust ==
    /\ mode \in {"entering", "viewing"} /\ next_seq > MaxSeq
    /\ facts_gen > 0 /\ read_flight = 0 /\ resp = "none"
    /\ (mode = "entering" \/ pending_nav /= "none" \/ refresh = 1)
    /\ mode' = "lost" /\ action' = "SeqExhaust"
    /\ UNCHANGED <<facts_gen, need_facts, win_seq, win_gen, ack, read_flight,
                   next_seq, pending_nav, nav_delta, committed, refresh,
                   deadline, retry_used, halve_used, recover_done, drawn_gen,
                   cmd_q, req, resp, win_gen_flight, ack_out, ack_resp,
                   enter_resp, recover_resp, delivered, evt_pending, evt_seq,
                   evt_gen, evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

\* ---------------- host environment (unfair: silence is real) ----------------

HostEmitEvent ==
    /\ demand = "viewing" /\ evt_pending = 0 /\ evt_seq < MaxEvent
    /\ evt_pending' = 1 /\ evt_seq' = evt_seq + 1 /\ evt_gen' = host_gen
    /\ evt_anchor' \in Anchors
    /\ action' = "HostEmitEvent"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, enter_resp, recover_resp, delivered, last_evt_seq,
                   observed_seq, host_gen, host_pending, host_committed,
                   demand, proj, emitted_delta>>

HostResize ==
    /\ host_gen < MaxGen
    /\ host_gen' = host_gen + 1 /\ host_pending' = 0
    /\ action' = "HostResize"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, enter_resp, recover_resp, delivered, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_committed, demand, proj, emitted_delta>>

HostEvict ==
    /\ host_committed /= 0 /\ host_committed' = 0 /\ action' = "HostEvict"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, req, resp, win_gen_flight, ack_out,
                   ack_resp, enter_resp, recover_resp, delivered, evt_pending,
                   evt_seq, evt_gen, evt_anchor, last_evt_seq, observed_seq,
                   host_gen, host_pending, demand, proj, emitted_delta>>

\* An adversarial/faulty host can answer with a window no read asked for:
\* the sequence gate sends it to recovery.
HostFaultWindow ==
    /\ resp = "none" /\ resp' = "window" /\ req' = 0
    /\ win_gen_flight' = host_gen /\ action' = "HostFaultWindow"
    /\ UNCHANGED <<mode, facts_gen, need_facts, win_seq, win_gen, ack,
                   read_flight, next_seq, pending_nav, nav_delta, committed,
                   refresh, deadline, retry_used, halve_used, recover_done,
                   drawn_gen, cmd_q, ack_out, ack_resp, enter_resp,
                   recover_resp, delivered, evt_pending, evt_seq, evt_gen,
                   evt_anchor, last_evt_seq, observed_seq, host_gen,
                   host_pending, host_committed, demand, proj, emitted_delta>>

Next ==
    \/ PressEnter \/ EmitTail \/ EmitCommitted
    \/ \E d \in {-2, -1, 1, 2} : ScrollDelta(d)
    \/ PressJump \/ PressExit \/ KeyTyped
    \/ FactsArrive \/ InstallWindow \/ StaleWindow \/ StrayWindow \/ DropOutcome
    \/ AckEmit \/ AckAccepted \/ AckRejected
    \/ ReadTypedRetry \/ ReadHostReject \/ ReadLimit
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
    /\ WF_vars(DropOutcome)
    /\ WF_vars(AckEmit) /\ WF_vars(AckAccepted) /\ WF_vars(AckRejected)
    /\ WF_vars(ReadTypedRetry) /\ WF_vars(ReadHostReject) /\ WF_vars(ReadLimit)
    /\ WF_vars(DeadlineTick) /\ WF_vars(DeadlineExpire)

\* ---------------- invariants ----------------

TypeOK ==
    /\ mode \in Modes /\ facts_gen \in 0..MaxGen /\ need_facts \in 0..1
    /\ win_seq \in 0..MaxSeq /\ win_gen \in 0..MaxGen /\ ack \in AckStates
    /\ read_flight \in 0..MaxSeq /\ next_seq \in 1..(MaxSeq + 1)
    /\ pending_nav \in NavKinds /\ nav_delta \in (0 - MaxDelta)..MaxDelta
    /\ committed \in 0..1 /\ refresh \in 0..1 /\ deadline \in 0..MaxDeadline
    /\ retry_used \in 0..1 /\ halve_used \in 0..1 /\ recover_done \in 0..1
    /\ drawn_gen \in 0..MaxGen /\ Len(cmd_q) <= MaxQueue + 2
    /\ req \in 0..MaxSeq /\ resp \in RespKinds /\ win_gen_flight \in 0..MaxGen
    /\ ack_out \in 0..MaxSeq /\ ack_resp \in {"none", "accepted", "rejected"}
    /\ enter_resp \in {"none", "applied", "rejected"}
    /\ recover_resp \in {"none", "applied"} /\ delivered \in 0..1
    /\ evt_pending \in 0..1 /\ evt_seq \in 0..MaxEvent /\ evt_gen \in 0..MaxGen
    /\ evt_anchor \in Anchors /\ last_evt_seq \in 0..MaxEvent
    /\ observed_seq \in 0..MaxEvent /\ host_gen \in 1..MaxGen
    /\ host_pending \in 0..MaxSeq /\ host_committed \in 0..MaxSeq
    /\ demand \in Demands /\ proj \in Projs
    /\ emitted_delta \in (0 - DeltaBound)..DeltaBound

\* S-R: at most one read in flight; the wire request is exactly that flight.
OneReadFlight == req \in {0, read_flight}

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

\* The host's pending pin always names a client-known sequence: the current
\* flight, the installed window awaiting its ack, or a recovery in progress
\* that will clear it.
HostPendingKnown ==
    host_pending /= 0
        => \/ host_pending \in {read_flight, win_seq}
           \/ mode \in {"recovering", "lost"}
           \/ \E i \in DOMAIN cmd_q : cmd_q[i] = "recover"

\* ---------------- progress ----------------

\* Entering always resolves — facts, or the client-side deadline — even when
\* the host stays silent.
EnteredResolves == (mode = "entering") ~> (mode /= "entering")

=============================================================================
