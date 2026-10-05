---------------------------- MODULE KeyboardCopyMode ----------------------------
EXTENDS Naturals, Sequences, FiniteSets

(* Copy episode coordination, not terminal text or the existing viewport handshake.
   CommitSurface abstracts accepted RowsApplied for primary and a valid live frame
   for alternate. Query tokens retain exact source; effect tokens retain episode,
   pane, Host, screen and generation after complete Host export. A cancelled OS
   call may still finish. A later explicit Copy creates a fresh export identity.
   No fairness or unbounded delivery/progress promise is part of this contract. *)

CONSTANTS MaxEpisodes, MaxCopies, MaxQueries, MaxQueued
ASSUME /\ MaxEpisodes = 2 /\ MaxCopies = 2 /\ MaxQueries = 2
       /\ MaxQueued \in 1..MaxCopies

Ids == 1..MaxCopies
Modes == {"live", "entering", "ready", "selecting", "waiting", "retired"}
Capturing == {"entering", "ready", "selecting", "waiting"}
NoQuery == [active |-> FALSE, id |-> 0, episode |-> 0, pane |-> 1, host |-> 1,
            screen |-> 1, generation |-> 1, revision |-> 1]

VARIABLE s
vars == <<s>>

Init == s = [mode |-> "live", episode |-> 0, pane |-> 1, host |-> 1,
    screen |-> 1, generation |-> 1, revision |-> 1,
    committed |-> FALSE, moving |-> FALSE, cursor |-> 0, desired |-> 0,
    selection |-> "none", query |-> NoQuery, queryCount |-> 0,
    queryViolation |-> FALSE, copies |-> 0, exportPending |-> 0,
    currentCopy |-> 0, copyEpisode |-> [i \in Ids |-> 0],
    copyPane |-> [i \in Ids |-> 1], copyHost |-> [i \in Ids |-> 1],
    copyScreen |-> [i \in Ids |-> 1], copyGeneration |-> [i \in Ids |-> 1],
    queue |-> <<>>, active |-> 0, validated |-> {}, attempted |-> {},
    settled |-> {}, cancelled |-> {}, attempts |-> [i \in Ids |-> 0],
    exitReason |-> "none", exitCopy |-> 0,
    observation |-> FALSE, packetTail |-> FALSE, leaked |-> FALSE]

RangeQueue == {s.queue[i] : i \in 1..Len(s.queue)}

QueryMatches == /\ s.query.episode = s.episode /\ s.query.pane = s.pane
    /\ s.query.host = s.host /\ s.query.screen = s.screen
    /\ s.query.generation = s.generation /\ s.query.revision = s.revision
    /\ s.mode \in {"ready", "selecting"} /\ s.committed /\ ~s.moving
CopyMatches(i) == /\ i = s.currentCopy /\ s.mode = "waiting"
    /\ s.copyEpisode[i] = s.episode /\ s.copyPane[i] = s.pane
    /\ s.copyHost[i] = s.host /\ s.copyScreen[i] = s.screen
    /\ s.copyGeneration[i] = s.generation /\ s.selection = "ready"

Enter(screen, pane) ==
    /\ s.mode = "live" /\ s.episode < MaxEpisodes
    /\ s' = [s EXCEPT !.episode = @ + 1, !.pane = pane, !.screen = screen,
        !.mode = IF screen = 1 THEN "entering" ELSE "ready",
        !.committed = screen = 2, !.moving = FALSE, !.selection = "none",
        !.currentCopy = 0, !.cursor = 0, !.desired = 0,
        !.exitReason = "none", !.exitCopy = 0, !.packetTail = TRUE]

CommitSurface ==
    /\ s.mode \in Capturing /\ ~s.committed /\ ~s.moving
    /\ s' = [s EXCEPT !.committed = TRUE,
        !.mode = IF @ = "entering" THEN "ready" ELSE @]

Move(target) ==
    /\ s.mode \in {"ready", "selecting"} /\ s.committed
    /\ ~s.query.active /\ ~s.moving /\ s.selection /= "pending" /\ target \in 0..2
    /\ s' = [s EXCEPT !.desired = target,
        !.moving = s.screen = 1, !.committed = s.screen = 2,
        !.cursor = IF s.screen = 2 THEN target ELSE @,
        !.selection = IF @ = "none" THEN @ ELSE "pending"]

ReplaceMove(target) ==
    /\ (s.moving \/ s.selection = "pending")
    /\ s.mode \in {"ready", "selecting"} /\ target \in 0..2
    /\ s' = [s EXCEPT !.desired = target]

CommitMove ==
    /\ s.moving /\ s.mode \in {"ready", "selecting"}
    /\ s' = [s EXCEPT !.cursor = s.desired, !.moving = FALSE,
        !.committed = TRUE,
        !.selection = IF @ = "none" THEN @ ELSE "pending"]

BeginSelection ==
    /\ s.mode = "ready" /\ s.selection = "none" /\ s.committed /\ ~s.moving /\ ~s.query.active
    /\ s' = [s EXCEPT !.mode = "selecting", !.selection = "pending"]

SelectionSettled ==
    /\ s.mode \in {"ready", "selecting"} /\ s.selection = "pending"
    /\ s.committed /\ ~s.moving /\ ~s.query.active
    /\ s' = [s EXCEPT
        !.selection = IF s.mode = "ready" THEN "none"
            ELSE IF s.desired = s.cursor THEN "ready" ELSE "pending",
        !.moving = s.desired /= s.cursor /\ s.screen = 1,
        !.committed = s.desired = s.cursor \/ s.screen = 2,
        !.cursor = IF s.screen = 2 THEN s.desired ELSE @]

CancelSelection ==
    /\ s.mode = "selecting" /\ s.selection = "ready" /\ ~s.moving /\ ~s.query.active
    /\ s' = [s EXCEPT !.mode = "ready", !.selection = "pending"]

RequestQuery ==
    /\ s.mode \in {"ready", "selecting"} /\ s.committed /\ ~s.moving
    /\ ~s.query.active /\ s.selection /= "pending" /\ s.queryCount < MaxQueries
    /\ s' = [s EXCEPT !.queryCount = @ + 1,
        !.query = [active |-> TRUE, id |-> s.queryCount + 1, episode |-> s.episode, pane |-> s.pane,
            host |-> s.host, screen |-> s.screen, generation |-> s.generation,
            revision |-> s.revision]]

QuerySuccess(id, target) ==
    /\ s.query.active /\ id = s.query.id /\ QueryMatches /\ target \in 0..2
    /\ s' = [s EXCEPT !.query.active = FALSE,
        !.queryViolation = @ \/ id /= s.query.id \/ ~QueryMatches,
        !.desired = target, !.moving = s.screen = 1,
        !.committed = s.screen = 2,
        !.cursor = IF s.screen = 2 THEN target ELSE @,
        !.selection = IF @ = "none" THEN @ ELSE "pending"]

QueryStale(id) ==
    /\ s.query.active /\ id = s.query.id /\ ~QueryMatches
    /\ s' = [s EXCEPT !.query.active = FALSE]

QueryForeign(id) ==
    /\ id \in 1..s.queryCount
    /\ (~s.query.active \/ id /= s.query.id)
    /\ s' = [s EXCEPT !.observation = ~@]

QueryFailure(id) ==
    /\ s.query.active /\ id = s.query.id
    /\ s' = [s EXCEPT !.query.active = FALSE]

RequestCopy ==
    /\ s.mode = "selecting" /\ s.selection = "ready" /\ s.committed /\ ~s.moving
    /\ ~s.query.active /\ s.exportPending = 0 /\ s.copies < MaxCopies
    /\ LET id == s.copies + 1 IN
       s' = [s EXCEPT !.copies = id, !.currentCopy = id,
           !.exportPending = id, !.mode = "waiting",
           !.copyEpisode[id] = s.episode, !.copyPane[id] = s.pane,
           !.copyHost[id] = s.host, !.copyScreen[id] = s.screen,
           !.copyGeneration[id] = s.generation]

ExportReady(id) ==
    /\ id = s.exportPending /\ s.exportPending /= 0 /\ CopyMatches(s.exportPending)
    /\ Len(s.queue) < MaxQueued
    /\ s' = [s EXCEPT !.validated = @ \cup {s.exportPending},
        !.queue = Append(@, s.exportPending), !.exportPending = 0]

ExportStale(id) ==
    /\ id = s.exportPending /\ s.exportPending /= 0 /\ ~CopyMatches(s.exportPending)
    /\ s' = [s EXCEPT !.exportPending = 0]

ExportForeign(id) ==
    /\ id \in 1..s.copies /\ id /= s.exportPending
    /\ s' = [s EXCEPT !.observation = ~@]

ExportFailure(id) ==
    /\ id = s.exportPending /\ s.exportPending /= 0 /\ CopyMatches(s.exportPending)
    /\ s' = [s EXCEPT !.exportPending = 0, !.currentCopy = 0,
        !.mode = "selecting"]

QueueRefused(id) ==
    /\ id = s.exportPending /\ s.exportPending /= 0 /\ CopyMatches(s.exportPending)
    /\ Len(s.queue) = MaxQueued
    /\ s' = [s EXCEPT !.exportPending = 0, !.currentCopy = 0,
        !.mode = "selecting"]

DispatchCopy ==
    /\ s.active = 0 /\ Len(s.queue) > 0
    /\ CopyMatches(Head(s.queue)) /\ Head(s.queue) \notin s.cancelled
    /\ Head(s.queue) \notin s.attempted
    /\ s' = [s EXCEPT !.active = Head(s.queue), !.queue = Tail(@),
        !.attempted = @ \cup {Head(s.queue)},
        !.attempts[Head(s.queue)] = @ + 1]

DiscardQueued ==
    /\ Len(s.queue) > 0
    /\ (~CopyMatches(Head(s.queue)) \/ Head(s.queue) \in s.cancelled)
    /\ s' = [s EXCEPT !.queue = Tail(@),
        !.cancelled = @ \cup {Head(s.queue)}]

CopyDelivered ==
    /\ s.active /= 0
    /\ IF CopyMatches(s.active)
       THEN s' = [s EXCEPT !.settled = @ \cup {s.active}, !.active = 0,
           !.mode = "live", !.selection = "none", !.committed = FALSE,
           !.currentCopy = 0, !.exitReason = "delivered", !.exitCopy = s.active,
           !.moving = FALSE]
       ELSE s' = [s EXCEPT !.settled = @ \cup {s.active}, !.active = 0]

CopyNotDelivered(result) ==
    /\ s.active /= 0 /\ result \in {"failed", "unknown"}
    /\ IF CopyMatches(s.active)
       THEN s' = [s EXCEPT !.settled = @ \cup {s.active}, !.active = 0,
           !.mode = "selecting", !.currentCopy = 0]
       ELSE s' = [s EXCEPT !.settled = @ \cup {s.active}, !.active = 0]

ForeignDelivered ==
    /\ s.mode \in Capturing
    /\ s' = [s EXCEPT !.observation = ~@]

Cancel(reason) ==
    /\ s.mode \in Capturing /\ reason \in {"cancel", "reload"}
    /\ s' = [s EXCEPT !.mode = "live", !.selection = "none",
        !.moving = FALSE, !.committed = FALSE, !.currentCopy = 0,
        !.cancelled = @ \cup (RangeQueue \ s.attempted),
        !.exitReason = reason, !.exitCopy = 0, !.packetTail = TRUE]

Invalidate ==
    /\ s.mode \in Capturing /\ s.generation = 1
    /\ s' = [s EXCEPT !.mode = "live", !.generation = 2,
        !.screen = IF @ = 1 THEN 2 ELSE 1, !.host = 2,
        !.selection = "none", !.moving = FALSE, !.committed = FALSE,
        !.currentCopy = 0, !.cancelled = @ \cup (RangeQueue \ s.attempted),
        !.exitReason = "invalidated", !.exitCopy = 0]

OutputRevision ==
    /\ s.mode \in Capturing /\ s.revision = 1
    /\ s' = [s EXCEPT !.revision = 2,
        !.committed = IF s.screen = 1 THEN FALSE ELSE @]

Busy ==
    /\ s.mode \in Capturing
    /\ (s.query.active \/ s.moving \/ s.mode = "waiting" \/ ~s.committed \/ s.selection = "pending")
    /\ s' = [s EXCEPT !.observation = ~@]

OwnedInput ==
    /\ s.mode \in Capturing
    /\ s' = [s EXCEPT !.observation = ~@, !.packetTail = TRUE]

ConsumeExitTail ==
    /\ s.mode /= "retired" /\ s.packetTail
    /\ s' = [s EXCEPT !.packetTail = FALSE]

ForwardOrdinary ==
    /\ s.mode = "live" /\ ~s.packetTail
    /\ s' = [s EXCEPT !.observation = ~@,
        !.leaked = @ \/ s.mode \in Capturing \/ s.packetTail]

Retire ==
    /\ s.mode /= "retired"
    /\ s' = [s EXCEPT !.mode = "retired", !.selection = "none",
        !.moving = FALSE, !.committed = FALSE, !.currentCopy = 0,
        !.query.active = FALSE, !.exportPending = 0, !.cancelled = @ \cup (RangeQueue \ s.attempted),
        !.exitReason = "retired", !.exitCopy = 0]

Next == (\E screen, pane \in 1..2 : Enter(screen, pane))
    \/ CommitSurface \/ (\E target \in 0..2 : Move(target))
    \/ (\E target \in 0..2 : ReplaceMove(target)) \/ CommitMove
    \/ BeginSelection \/ SelectionSettled \/ CancelSelection \/ RequestQuery
    \/ (\E id \in 1..MaxQueries, target \in 0..2 : QuerySuccess(id, target))
    \/ (\E id \in 1..MaxQueries : QueryStale(id) \/ QueryForeign(id) \/ QueryFailure(id))
    \/ RequestCopy
    \/ (\E id \in Ids : ExportReady(id) \/ ExportStale(id) \/ ExportForeign(id)
        \/ QueueRefused(id) \/ ExportFailure(id))
    \/ DispatchCopy \/ DiscardQueued \/ CopyDelivered
    \/ (\E result \in {"failed", "unknown"} : CopyNotDelivered(result))
    \/ Busy \/ ForeignDelivered \/ (\E reason \in {"cancel", "reload"} : Cancel(reason))
    \/ Invalidate \/ OutputRevision \/ OwnedInput \/ ConsumeExitTail
    \/ ForwardOrdinary \/ Retire

Spec == Init /\ [][Next]_vars
TypeOK == /\ s.mode \in Modes /\ s.episode \in 0..MaxEpisodes
    /\ s.pane \in 1..2 /\ s.host \in 1..2 /\ s.screen \in 1..2
    /\ s.generation \in 1..2 /\ s.revision \in 1..2
    /\ s.committed \in BOOLEAN /\ s.moving \in BOOLEAN /\ s.selection \in {"none", "pending", "ready"}
    /\ s.cursor \in 0..2 /\ s.desired \in 0..2
    /\ s.queryCount \in 0..MaxQueries /\ s.query.active \in BOOLEAN
    /\ s.query.id \in 0..MaxQueries /\ s.queryViolation \in BOOLEAN
    /\ s.copies \in 0..MaxCopies /\ s.exportPending \in 0..MaxCopies
    /\ s.currentCopy \in 0..MaxCopies /\ s.active \in 0..MaxCopies
    /\ s.queue \in Seq(Ids) /\ s.validated \subseteq Ids
    /\ s.attempted \subseteq Ids /\ s.settled \subseteq Ids /\ s.cancelled \subseteq Ids
    /\ s.copyEpisode \in [Ids -> 0..MaxEpisodes]
    /\ s.copyPane \in [Ids -> 1..2] /\ s.copyHost \in [Ids -> 1..2]
    /\ s.copyScreen \in [Ids -> 1..2] /\ s.copyGeneration \in [Ids -> 1..2]
    /\ s.attempts \in [Ids -> 0..MaxCopies]
    /\ s.observation \in BOOLEAN
    /\ s.packetTail \in BOOLEAN /\ s.leaked \in BOOLEAN

EffectQueueBounded == Len(s.queue) <= MaxQueued
NoAutomaticReplay == /\ RangeQueue \cap s.attempted = {}
    /\ (s.active = 0 \/ s.active \in s.attempted)
    /\ \A i \in Ids : s.attempts[i] <= 1
CompleteExportBeforeSubmission == s.attempted \subseteq s.validated
SettledEffectsWereAttempted == s.settled \subseteq s.attempted
SelectionRequiresEpisode == s.selection /= "none" => s.episode > 0 /\ s.mode \in Capturing
NoPaneInputLeak == ~s.leaked
NoStaleQueryAdoption == ~s.queryViolation
MatchingSuccessExit == s.exitReason = "delivered" =>
    /\ s.exitCopy \in s.settled
    /\ s.copyEpisode[s.exitCopy] = s.episode /\ s.copyPane[s.exitCopy] = s.pane
    /\ s.copyHost[s.exitCopy] = s.host /\ s.copyScreen[s.exitCopy] = s.screen
    /\ s.copyGeneration[s.exitCopy] = s.generation
RetiredHasNoLocalIntent == s.mode = "retired" =>
    /\ s.selection = "none" /\ ~s.query.active /\ ~s.moving
    /\ s.currentCopy = 0 /\ s.exportPending = 0

=============================================================================
