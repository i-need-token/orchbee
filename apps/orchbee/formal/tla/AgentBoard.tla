-------------------------- MODULE AgentBoard --------------------------
EXTENDS Integers, Sequences, FiniteSets

CONSTANT MaxEpoch, MaxRound, MaxSerial
ASSUME MaxEpoch \in Nat \ {0}
ASSUME MaxRound \in Nat \ {0}
ASSUME MaxSerial \in Nat \ {0}

\* The Agent board: an input-capturing overlay that lists Server-owned Agent
\* sessions and jumps to an already-existing Panel. This module proves the
\* board's own obligations — generation-scoped results, one bounded read round,
\* single-flight lane scheduling with one coalesced intent, stale-jump
\* rejection, exact staged landing, input ownership and lane-thread
\* reclamation. Mailbox transport, first-frame setup, context cleanup reserves
\* and carrier retries refine to the existing native Client models; the OBAP
\* wire contract is the Server's, not the board's.
Sessions == {"agent-a", "agent-b"}
Ord(session) == IF session = "agent-a" THEN 1 ELSE 2
NoSession == "no-session"

\* A complete walk is a bounded page sequence; two pages model the six-page
\* ceiling (one hundred sessions per page over the 512-session cap).
Pages == {1, 2}
PageMembers(page) == IF page = 1 THEN {"agent-a"} ELSE {"agent-b"}
Revisions == {1, 2}
OtherRevision(revision) == IF revision = 1 THEN 2 ELSE 1

\* F1: one full re-walk per round bounds every walk to two page sets — the
\* shape of the real six-page/one-rewalk/twelve-exchange budget.
MaxRetries == 1

\* The little Workbench world. Placement is canonical: a presentation lives at
\* exactly one context/tab/panel triple, and agent-b's only presentation is
\* deliberately not on the first panel so a hardcoded panel-0 landing is a
\* violation, not an accident. ContextHome is where a context selection lands
\* before the tab stage resolves the requested tab.
Presentations == {"p-1", "p-2", "p-3"}
PresOrd(p) == IF p = "p-1" THEN 1 ELSE IF p = "p-2" THEN 2 ELSE 3
Placement(p) == CASE p = "p-1" -> [context |-> "ctx-a", tab |-> "tab-1", panel |-> "panel-1"]
                  [] p = "p-2" -> [context |-> "ctx-a", tab |-> "tab-1", panel |-> "panel-2"]
                  [] p = "p-3" -> [context |-> "ctx-b", tab |-> "tab-2", panel |-> "panel-1"]
ContextHome(context) == IF context = "ctx-a" THEN Placement("p-1") ELSE Placement("p-3")
TabHomePanel(tab) == "panel-1"
\* Canonical Agent→presentation bindings. agent-a spans both contexts so its
\* deterministic pick exercises the current-context-first rule.
Bindings(session) == IF session = "agent-a" THEN {"p-1", "p-3"} ELSE {"p-2"}
Locations == {Placement(p): p \in Presentations}
NoLocation == [context |-> "no-context", tab |-> "no-tab", panel |-> "no-panel"]
NoTarget == [session |-> NoSession, presentation |-> "no-presentation"]

\* Row completeness honesty: a complete row set is exactly the full canonical
\* session domain assembled under one revision; anything else is partial,
\* unread or failed and must never present itself as complete.
RowStates == {"unread", "complete", "incomplete", "failed"}

VARIABLE s
vars == <<s>>

Init ==
  s = [board |-> "closed",
       view |-> Placement("p-1"),
       epoch |-> 0,
       round |-> 0,
       serial |-> 0,
       laneAlive |-> FALSE,
       lanePhase |-> "idle",
       laneRound |-> 0,
       intent |-> "none",
       intentTarget |-> NoSession,
       cooldown |-> FALSE,
       walkRev |-> 0,
       walkPages |-> {},
       retries |-> 0,
       serverRevision |-> 1,
       bumps |-> 0,
       retired |-> {},
       rows |-> {},
       rowState |-> "unread",
       fullWalk |-> FALSE,
       lastGood |-> {},
       lastGoodStale |-> FALSE,
       selected |-> NoSession,
       jumpStage |-> "none",
       jumpTarget |-> NoTarget,
       jumpSerial |-> 0,
       landedPlacement |-> NoLocation,
       landedTarget |-> "no-presentation",
       landedSerial |-> 0,
       landedLive |-> FALSE,
       outcome |-> "none",
       paneBytes |-> FALSE,
       paneBytesAtOpen |-> FALSE,
       consumed |-> {},
       \* Per-presentation viewer liveness (the pane record's
       \* attached/projected/not-departing/mode-known facts folded into one
       \* target-level bit) and the paint verdict. A dead viewer never
       \* revives on its own — only the jump's reattach stage re-installs
       \* it. `deadSeen` bounds environment deaths to one per presentation
       \* per run, the same shape as `bumps`/`retired`.
       viewerLive |-> [p \in Presentations |-> TRUE],
       deadSeen |-> {},
       paintable |-> TRUE]

\* Deterministic presentation pick for a live session: a presentation in the
\* currently-viewed context first, then the smallest presentation id — the
\* model folds "current-tab usable" and "visible" into context membership
\* because tab granularity inside the pick changes no safety claim here.
LiveBindings(session) == Bindings(session) \ s.retired
Pick(session) ==
  LET live == LiveBindings(session) IN
    IF live = {} THEN "no-presentation"
    ELSE LET here == {p \in live: Placement(p).context = s.view.context} IN
      IF here # {} THEN CHOOSE p \in here: \A q \in here: PresOrd(p) <= PresOrd(q)
      ELSE CHOOSE p \in live: \A q \in live: PresOrd(p) <= PresOrd(q)

\* Selection repair is by opaque session id, never by row position.
RepairSelection(rows) ==
  IF s.selected \in rows THEN s.selected
  ELSE IF rows = {} THEN NoSession
       ELSE CHOOSE x \in rows: \A y \in rows: Ord(x) <= Ord(y)

\* A List stays valid across jump-serial advances (F5: the Get waits behind
\* the bounded List); only the round generation can stale it.
ListCurrent == s.laneRound = s.round
\* A jump's generation is (epoch, serial): a read round advancing the shared
\* round counter does not stale a staged navigation — only close/reopen or a
\* newer jump does (F5).
JumpCurrent == s.jumpSerial # 0 /\ s.jumpSerial = s.serial

\* ------------------------- board lifecycle -------------------------

\* Opening is a generation boundary: the epoch advances, every prior
\* in-flight intent/jump/lane result is fenced, and admission is refused
\* until the previous lane thread has actually exited — never two live
\* lanes, never a recycled generation. The accepted display cache survives
\* close/reopen unchanged: `rows`/`rowState`/`fullWalk`, `lastGood` with its
\* stale mark, the repaired `selected`, and any residual `cooldown` all keep
\* their original age — a reopen may not launder freshness or skip the
\* cooldown, while a fresh lane, fresh intents and a fresh jump generation
\* are still minted. Viewer facts are world state, not board state: a dead
\* presentation stays dead across reopen.
Open ==
  /\ s.board = "closed"
  /\ ~s.laneAlive
  /\ s.epoch < MaxEpoch
  /\ s' = [s EXCEPT !.board = "open", !.epoch = @ + 1, !.round = 0,
                    !.laneAlive = TRUE, !.lanePhase = "idle",
                    !.laneRound = 0, !.intent = "none",
                    !.intentTarget = NoSession,
                    !.walkRev = 0, !.walkPages = {}, !.retries = 0,
                    !.bumps = 0, !.consumed = {}, !.deadSeen = {},
                    !.jumpStage = "none", !.jumpTarget = NoTarget,
                    !.jumpSerial = 0, !.landedSerial = 0,
                    !.landedLive = FALSE, !.outcome = "none",
                    !.paintable = TRUE, !.paneBytesAtOpen = s.paneBytes]

\* Observable reopen coverage: the second open is a distinct action name so
\* the bounded run proves close → lane exit → reopen is actually exercised.
Reopen == s.epoch >= 1 /\ Open

\* Closing cancels the lane and the jump in the same ordered transition: no
\* new submit may follow, and the thread must actually exit before any
\* reopen. Results already on the wire die with the dead generation — the
\* lane owns its outcome receiver, so nothing the dead lane computed can be
\* delivered into the next epoch.
CloseBoard ==
  [s EXCEPT !.board = "closed", !.lanePhase = "exiting",
            !.intent = "none", !.intentTarget = NoSession,
            !.jumpStage = "none", !.jumpTarget = NoTarget,
            !.jumpSerial = 0, !.outcome = "none",
            !.serial = IF s.serial < MaxSerial THEN @ + 1 ELSE @]

Close == s.board = "open" /\ s' = CloseBoard

\* The draw's honest verdict: the overlay does not fit the current terminal
\* — the board must not remain an invisible input owner.
PaintRefused ==
  /\ s.board = "open"
  /\ s.paintable
  /\ s' = [s EXCEPT !.paintable = FALSE]

\* A board that cannot be painted auto-closes through the same ordered
\* transition as an explicit close; consumed input is never replayed.
UnpaintableClose ==
  /\ s.board = "open"
  /\ ~s.paintable
  /\ s' = CloseBoard

\* The owner reclaims the thread: a cancelled lane exits, and only then may a
\* later Open admit a fresh one.
LaneExit ==
  /\ s.board = "closed"
  /\ s.laneAlive
  /\ s.lanePhase = "exiting"
  /\ s' = [s EXCEPT !.laneAlive = FALSE, !.lanePhase = "idle", !.laneRound = 0]

\* --------------------------- input ownership ------------------------

\* While the board owns input every complete key or paste event is consumed:
\* nothing reaches the pane, and an outside wheel is swallowed without
\* closing the board — only an ordinary outside press dismisses it.
BoardByte ==
  /\ s.board = "open"
  /\ "byte" \notin s.consumed
  /\ s' = [s EXCEPT !.consumed = @ \cup {"byte"}]

OutsideWheel ==
  /\ s.board = "open"
  /\ "wheel" \notin s.consumed
  /\ s' = [s EXCEPT !.consumed = @ \cup {"wheel"}]

BackdropPress ==
  /\ s.board = "open"
  /\ s' = CloseBoard

PaneByte ==
  /\ s.board = "closed"
  /\ ~s.paneBytes
  /\ s' = [s EXCEPT !.paneBytes = TRUE]

\* ---------------------------- selection ----------------------------

SelectNext ==
  /\ s.board = "open"
  /\ s.rowState \in {"complete", "incomplete"}
  /\ s.rows # {}
  \* Bounded selection establishment: unselected picks the first row, then at
  \* most one move reaches the other session — enough for either opaque id to
  \* be jumped without free selection churn.
  /\ s.selected \in {NoSession, "agent-a"}
  /\ s' = [s EXCEPT !.selected =
             LET above == {x \in s.rows: Ord(x) > Ord(s.selected)} IN
               IF above # {} THEN CHOOSE x \in above: \A y \in above: Ord(x) <= Ord(y)
               ELSE CHOOSE x \in s.rows: \A y \in s.rows: Ord(x) <= Ord(y)]

\* ------------------------------ intents ----------------------------

\* One coalesced intent slot behind the single active op. A refresh request
\* while a jump is pending is consumed without displacing it — the pending
\* jump already carries fresher verification than the list it replaced.
RequestRefresh ==
  /\ s.board = "open"
  /\ s.intent = "none"
  /\ s' = [s EXCEPT !.intent = "refresh"]

RequestRefreshWhileJumpPending ==
  /\ s.board = "open"
  /\ s.intent = "jump"
  /\ "refresh-pending" \notin s.consumed
  /\ s' = [s EXCEPT !.consumed = @ \cup {"refresh-pending"}]

\* A jump request replaces a pending refresh outright (F5); while an earlier
\* jump is still in flight the new request cancels it by serial advance alone
\* — the old Get result and every staged step then fail their generation
\* check (GetStaleDrop, StageStaleDrop) before they can navigate. The stale
\* stage survives one step so the drop is an observable transition, matching
\* the production rule that each stage re-checks its generation at its own
\* boundary.
RequestJump ==
  /\ s.board = "open"
  /\ s.selected \in s.rows
  /\ s.serial < MaxSerial
  /\ s' = [s EXCEPT !.intent = "jump", !.intentTarget = s.selected,
                    !.outcome = "none",
                    !.serial = @ + 1]

\* ------------------------------ lane ops -----------------------------

\* Single flight: the idle lane drains the coalesced intent into exactly one
\* active op. A read round mints its round generation and pins the revision
\* observed on the first page; a Get mints its serial generation.
StartList ==
  /\ s.board = "open"
  /\ s.laneAlive
  /\ s.lanePhase = "idle"
  /\ s.intent = "refresh"
  /\ ~s.cooldown
  /\ s.round < MaxRound
  /\ s' = [s EXCEPT !.intent = "none", !.round = @ + 1,
                    !.lanePhase = "list", !.laneRound = s.round + 1,
                    !.walkRev = 0, !.walkPages = {}, !.retries = 0]

\* A refresh intent that survives past the per-epoch round budget drains
\* honestly: the board keeps its last settled state instead of submitting an
\* unbounded walk.
StartListExhausted ==
  /\ s.board = "open"
  /\ s.laneAlive
  /\ s.lanePhase = "idle"
  /\ s.intent = "refresh"
  /\ s.round >= MaxRound
  /\ s' = [s EXCEPT !.intent = "none"]

StartGet ==
  /\ s.board = "open"
  /\ s.laneAlive
  /\ s.lanePhase = "idle"
  /\ s.intent = "jump"
  /\ s' = [s EXCEPT !.intent = "none", !.intentTarget = NoSession,
                    !.lanePhase = "get",
                    !.jumpStage = "verify",
                    !.jumpTarget = [session |-> s.intentTarget,
                                    presentation |-> "no-presentation"],
                    !.jumpSerial = s.serial,
                    !.landedPlacement = NoLocation,
                    !.landedTarget = "no-presentation",
                    !.landedSerial = 0,
                    !.landedLive = FALSE,
                    !.outcome = "none"]

\* Each page response sees the revision the Server currently publishes. The
\* first response pins the walk's revision; a page arriving under a different
\* revision means the cursor went stale mid-walk — restart within the one
\* re-walk budget or settle incomplete. Pages from different revisions are
\* never spliced and a partial walk never reports complete.
ListPage ==
  /\ s.board = "open"
  /\ s.lanePhase = "list"
  /\ ListCurrent
  /\ LET rev == IF s.walkRev = 0 THEN s.serverRevision ELSE s.walkRev IN
       IF s.serverRevision # rev
         THEN IF s.retries < MaxRetries
                THEN s' = [s EXCEPT !.retries = @ + 1,
                                    !.walkRev = s.serverRevision, !.walkPages = {}]
                ELSE LET partial == UNION {PageMembers(p): p \in s.walkPages}
                          shown == IF s.lastGood # {} THEN s.lastGood ELSE partial IN
                       s' = [s EXCEPT !.lanePhase = "idle",
                                      !.cooldown = TRUE, !.rowState = "incomplete",
                                      !.fullWalk = FALSE, !.rows = shown,
                                      !.lastGoodStale = s.lastGood # {},
                                      !.selected = RepairSelection(shown)]
         ELSE LET pages == s.walkPages \cup
                             {CHOOSE p \in Pages: p \notin s.walkPages} IN
                IF pages = Pages
                  THEN \* Same-revision full walk: the new complete result
                       \* becomes last_good and repairs selection by opaque id.
                       s' = [s EXCEPT !.lanePhase = "idle",
                                      !.cooldown = TRUE, !.rowState = "complete",
                                      !.fullWalk = TRUE, !.rows = Sessions,
                                      !.lastGood = Sessions,
                                      !.lastGoodStale = FALSE,
                                      !.selected = RepairSelection(Sessions)]
                  ELSE s' = [s EXCEPT !.walkRev = rev,
                                      !.walkPages = pages]

\* The shared round deadline expiring mid-walk settles the round as a failed
\* read: last_good survives only as stale-marked and nothing reports complete.
ListDeadline ==
  /\ s.board = "open"
  /\ s.lanePhase = "list"
  /\ ListCurrent
  /\ s' = [s EXCEPT !.lanePhase = "idle", !.cooldown = TRUE,
                    !.rowState = "failed", !.fullWalk = FALSE,
                    !.rows = s.lastGood,
                    !.lastGoodStale = s.lastGood # {},
                    !.selected = RepairSelection(s.lastGood)]

\* The Get revalidation: only the exact current generation may proceed. A
\* session whose bindings all retired fails as an explicit target-gone;
\* otherwise the deterministic pick chooses the presentation and staged
\* navigation begins.
GetResolve ==
  /\ s.board = "open"
  /\ s.lanePhase = "get"
  /\ JumpCurrent
  /\ s.jumpStage = "verify"
  /\ LET session == s.jumpTarget.session IN
       IF LiveBindings(session) = {}
         THEN s' = [s EXCEPT !.lanePhase = "idle", !.jumpStage = "gone",
                             !.outcome = "target-gone"]
         ELSE s' = [s EXCEPT !.lanePhase = "idle", !.jumpStage = "context",
                             !.jumpTarget = [session |-> session,
                                             presentation |-> Pick(session)]]

\* A Get whose serial generation was superseded by a newer jump deposits
\* nothing and leaves no navigation trace; a superseded staged jump dies
\* before its next stage can move the view. This is the same-epoch supersede
\* path: close retires the lane itself, so no result an old lane computed
\* can ever be delivered into a later epoch — the epoch fence is structural,
\* not a race the model needs to schedule.
GetStaleDrop ==
  /\ s.board = "open"
  /\ s.lanePhase = "get"
  /\ ~JumpCurrent
  /\ s' = [s EXCEPT !.lanePhase = "idle", !.laneRound = 0,
                    !.jumpStage = "none", !.jumpTarget = NoTarget]

StageStaleDrop ==
  /\ s.board = "open"
  /\ s.lanePhase = "idle"
  /\ s.jumpStage \in {"context", "tab", "panel", "reattach"}
  /\ ~JumpCurrent
  /\ s' = [s EXCEPT !.jumpStage = "none", !.jumpTarget = NoTarget,
                    !.outcome = "none"]

\* ------------------------- staged navigation ------------------------

\* Every stage re-resolves the stable identity against the canonical world at
\* that instant; a stage whose presentation retired fails target-gone without
\* recording a landing. Intermediate stages land on real canonical locations —
\* the context's home, then the requested tab's home — and only the final
\* stage installs the exact requested panel.
ResolveContext ==
  /\ s.board = "open"
  /\ s.jumpStage = "context"
  /\ JumpCurrent
  /\ IF s.jumpTarget.presentation \in s.retired
       THEN s' = [s EXCEPT !.jumpStage = "gone", !.outcome = "target-gone"]
       ELSE s' = [s EXCEPT !.jumpStage = "tab",
                           !.view = IF Placement(s.jumpTarget.presentation).context
                                        = s.view.context
                                      THEN s.view
                                      ELSE ContextHome(
                                             Placement(s.jumpTarget.presentation).context)]

ResolveTab ==
  /\ s.board = "open"
  /\ s.jumpStage = "tab"
  /\ JumpCurrent
  /\ IF s.jumpTarget.presentation \in s.retired
       THEN s' = [s EXCEPT !.jumpStage = "gone", !.outcome = "target-gone"]
       ELSE s' = [s EXCEPT !.jumpStage = "panel",
                           !.view = [context |->
                                       Placement(s.jumpTarget.presentation).context,
                                     tab |-> Placement(s.jumpTarget.presentation).tab,
                                     panel |-> TabHomePanel(
                                               Placement(s.jumpTarget.presentation).tab)]]

ResolvePanel ==
  /\ s.board = "open"
  /\ s.jumpStage = "panel"
  /\ JumpCurrent
  /\ IF s.jumpTarget.presentation \in s.retired
       THEN s' = [s EXCEPT !.jumpStage = "gone", !.outcome = "target-gone"]
       ELSE IF ~s.viewerLive[s.jumpTarget.presentation]
         THEN \* The exact panel is reached but its viewer is dead: desired
              \* focus identity alone is never a landing. The jump takes the
              \* narrow reattach stage — a dead same-id target can never
              \* produce `landed`.
              s' = [s EXCEPT !.jumpStage = "reattach"]
         ELSE s' = [s EXCEPT !.jumpStage = "done", !.outcome = "landed",
                             !.view = Placement(s.jumpTarget.presentation),
                             !.landedPlacement = Placement(s.jumpTarget.presentation),
                             !.landedTarget = s.jumpTarget.presentation,
                             !.landedSerial = s.jumpSerial,
                             !.landedLive = TRUE]

\* The Agent-scoped same-target reattach: the target presentation's pane
\* viewer is dead, so the narrow admission mints a fresh anchor generation
\* through the ordinary switch pipeline — every viewer in the active Tab
\* detaches and re-attaches under the new generation (the accepted cost; the
\* Server's terminals are not restarted) and focus lands on the exact target
\* panel. The next panel pass then sees a live viewer and may land.
ResolveReattach ==
  /\ s.board = "open"
  /\ s.jumpStage = "reattach"
  /\ JumpCurrent
  /\ IF s.jumpTarget.presentation \in s.retired
       THEN s' = [s EXCEPT !.jumpStage = "gone", !.outcome = "target-gone"]
       ELSE s' = [s EXCEPT
            !.viewerLive = [@ EXCEPT ![s.jumpTarget.presentation] = TRUE],
            !.jumpStage = "panel",
            !.view = Placement(s.jumpTarget.presentation)]

JumpSettled ==
  /\ s.board = "open"
  /\ s.jumpStage \in {"done", "gone"}
  /\ s' = [s EXCEPT !.jumpStage = "none", !.jumpTarget = NoTarget,
                    !.jumpSerial = 0, !.outcome = "none"]

\* ------------------------- server mutation --------------------------

\* The canonical world moves under the board: read revisions advance and
\* presentations retire. Both may interleave anywhere the board can observe
\* them — a closed board has no observable to race, so mutation is open-only
\* rather than constraining any real cancel/stale interleaving away. One
\* retirement per bound run still reaches the explicit target-gone branches
\* (agent-b's only binding) and every stale-stage path.
ServerBump ==
  /\ s.board = "open"
  /\ s.bumps < 2
  /\ s' = [s EXCEPT !.serverRevision = OtherRevision(s.serverRevision),
                    !.bumps = @ + 1]

ServerRetire ==
  /\ s.board = "open"
  /\ s.retired = {}
  /\ \E p \in Presentations:
       s' = [s EXCEPT !.retired = {p}]

\* A presentation's pane viewer dies under the board: canonical disposal or
\* controller loss is world state the board merely observes. One death per
\* presentation per run bounds the interleaving the same way `bumps`/`retired`
\* do; only the jump's reattach stage revives a viewer.
ViewerDies ==
  /\ s.board = "open"
  /\ \E p \in Presentations:
       /\ s.viewerLive[p]
       /\ p \notin s.deadSeen
       /\ s' = [s EXCEPT !.viewerLive = [@ EXCEPT ![p] = FALSE],
                         !.deadSeen = @ \cup {p}]

\* The post-round cooldown passes abstractly; it gates the next List submit,
\* never the intent bookkeeping.
CooldownElapse ==
  /\ s.cooldown
  /\ s' = [s EXCEPT !.cooldown = FALSE]

Next ==
  \/ Open \/ Reopen \/ Close \/ UnpaintableClose \/ LaneExit
  \/ BoardByte \/ OutsideWheel \/ BackdropPress \/ PaneByte
  \/ SelectNext
  \/ RequestRefresh \/ RequestRefreshWhileJumpPending \/ RequestJump
  \/ StartList \/ StartListExhausted \/ StartGet
  \/ ListPage \/ ListDeadline
  \/ GetResolve \/ GetStaleDrop \/ StageStaleDrop
  \/ ResolveContext \/ ResolveTab \/ ResolvePanel \/ ResolveReattach
  \/ JumpSettled
  \/ ServerBump \/ ServerRetire \/ ViewerDies \/ PaintRefused
  \/ CooldownElapse

Spec == Init /\ [][Next]_vars
       /\ WF_vars(ListPage) /\ WF_vars(ListDeadline)
       /\ WF_vars(GetResolve) /\ WF_vars(GetStaleDrop) /\ WF_vars(StageStaleDrop)
       /\ WF_vars(ResolveContext) /\ WF_vars(ResolveTab) /\ WF_vars(ResolvePanel)
       /\ WF_vars(ResolveReattach) /\ WF_vars(StartList) /\ WF_vars(StartGet)
       /\ WF_vars(JumpSettled) /\ WF_vars(LaneExit) /\ WF_vars(UnpaintableClose)

\* ------------------------------ invariants ---------------------------

TypeOK ==
  /\ s.board \in {"closed", "open"}
  /\ s.lanePhase \in {"idle", "list", "get", "exiting"}
  /\ s.intent \in {"none", "refresh", "jump"}
  /\ s.rowState \in RowStates
  /\ s.jumpStage \in {"none", "verify", "context", "tab", "panel",
                      "reattach", "done", "gone"}
  /\ s.retries \in 0..MaxRetries
  /\ s.retired \subseteq Presentations
  /\ s.rows \subseteq Sessions
  /\ s.lastGood \subseteq Sessions
  /\ s.view \in Locations \cup {NoLocation}
  /\ s.selected \in Sessions \cup {NoSession}
  /\ s.outcome \in {"none", "landed", "target-gone"}
  /\ s.epoch \in 0..MaxEpoch
  /\ s.round \in 0..MaxRound
  /\ s.serial \in 0..MaxSerial
  /\ s.jumpSerial \in 0..MaxSerial
  /\ s.landedSerial \in 0..MaxSerial
  /\ s.laneRound \in 0..MaxRound
  /\ s.bumps \in 0..2
  /\ s.consumed \subseteq {"byte", "wheel", "refresh-pending"}
  /\ s.paneBytes \in BOOLEAN
  /\ s.paneBytesAtOpen \in BOOLEAN
  /\ s.viewerLive \in [Presentations -> BOOLEAN]
  /\ s.deadSeen \subseteq Presentations
  /\ s.paintable \in BOOLEAN
  /\ s.landedLive \in BOOLEAN

\* At most one lane thread exists, it lives only inside an open board's
\* generation, and no submit follows close or cancel.
SingleLiveLane == s.board = "open" => s.laneAlive
NoSubmitWhileClosed == s.board = "closed" => s.lanePhase \in {"idle", "exiting"}

\* A displayed complete row set is always the full canonical domain of one
\* finished walk; incomplete and failed reads never masquerade as it, and
\* last_good is marked stale the moment a later round supersedes it.
CompleteRowsFromCurrentWalk ==
  s.rowState = "complete" => (s.fullWalk /\ s.rows = Sessions)
IncompleteNeverComplete == s.rowState \in {"incomplete", "failed"} => ~s.fullWalk
LastGoodHonest ==
  s.lastGoodStale => s.lastGood # {}

\* Selection is an opaque session id that still exists — never a row index.
SelectedIsLive == s.selected \in Sessions => s.selected \in s.rows

\* Nothing the board consumes reaches the pane while it owns input.
InputFrozenWhileOpen == s.board = "open" => s.paneBytes = s.paneBytesAtOpen

\* A landed jump always carries the current serial, installs the exact
\* canonical placement — right context, right tab, right panel, never a
\* positional index or a panel-0 fallback — and lands exactly the
\* presentation the request resolved.
LandedIsCurrent ==
  s.outcome = "landed" => s.landedSerial = s.serial /\ s.landedSerial # 0
LandedExact ==
  s.outcome = "landed" => s.view = s.landedPlacement /\ s.landedPlacement \in Locations
LandedIsRequested ==
  s.outcome = "landed" => s.landedTarget = s.jumpTarget.presentation

\* A landing is recorded only when the exact target's pane viewer was live
\* at the land step — a same-identity dead viewer is never reported as a
\* landing; the reattach stage must re-install it first.
LandedIsLive == s.outcome = "landed" => s.landedLive

\* A vanished target reports target-gone and never records a landing — a
\* failed jump cannot masquerade as a successful one.
TargetGoneHonest ==
  s.outcome = "target-gone" => s.landedPlacement = NoLocation

\* Every staged navigation in flight carries a generation minted inside this
\* epoch and never newer than the board's serial; a superseded stage is dead
\* and can only be dropped (StageStaleDrop) or overwritten by the newer Get —
\* the Resolve*/landed guards keep it from ever moving the view.
StagedIsCurrent ==
  s.jumpStage \in {"verify", "context", "tab", "panel", "reattach"} =>
    (s.jumpSerial # 0 /\ s.jumpSerial <= s.serial)

\* A closed board holds no live navigation in flight.
ClosedBoardIdle == s.board = "closed" => s.jumpStage = "none"

\* ------------------------- temporal properties ------------------------

\* Necessary progress: an open board that wants a read eventually settles one
\* (complete, incomplete or failed — bumping can delay, never prevent, since
\* the retry budget is finite), a closed board's lane thread is reclaimed,
\* and a staged jump always reaches its explicit outcome.
RoundEventuallySettles ==
  (s.board = "open" /\ s.intent = "refresh" /\ ~s.cooldown)
    ~> (s.rowState \in {"complete", "incomplete", "failed"} \/ s.board = "closed")
LaneEventuallyReclaimed == (s.board = "closed" /\ s.laneAlive) ~> ~s.laneAlive
JumpEventuallySettles ==
  (s.jumpStage \in {"verify", "context", "tab", "panel", "reattach"})
    ~> (s.jumpStage = "none" \/ s.board = "closed")

\* A board the renderer refuses to paint closes rather than remaining an
\* invisible modal — either the auto-close edge or an ordinary close ends
\* the episode.
UnpaintableEventuallyCloses ==
  (s.board = "open" /\ ~s.paintable) ~> s.board = "closed"

=============================================================================
