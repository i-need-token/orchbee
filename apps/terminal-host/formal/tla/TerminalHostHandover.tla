------------------------ MODULE TerminalHostHandover ------------------------
EXTENDS Naturals, Sequences

\* Checks the move of one terminal from an old Terminal Host to a new one under
\* the H1 design of the Host handover proposal
\* (proposals/platform/host-handover.md: "How a terminal moves", "The Keeper
\* protocol", "The Host's side", and "The move's model"). It covers that
\* section's invariants: one reader, no loss or repeat with records that end
\* only at ground, each held key once, an abort that restores the old Host,
\* only a claim committing, and holdings that name only Hosts that may read.
\*
\* Transition vocabulary: Quiesce is the Worker's hold and QuiesceTerminal;
\* Park and Withdraw are the reader parking at ground and a quiesce refused as
\* passing; Mark is the export followed by `mark`; Resume is `resume` at the
\* mark's deadline or on the Worker's ResumeTerminal; Restore is
\* RestoreTerminal with `inspect`; Claim and Decline are `claim` and `decline`,
\* the latter on DiscardImport or at the import's deadline; OldReply and
\* NewReply read a Keeper answer; Hold is `hold` after AttachKeeper or a
\* resume; Drop is DropTerminal; ResumeWithoutKeeper is ResumeTerminal made
\* without the Keeper; Release sends the Worker's held input.
\*
\* The Keeper applies one request at a time, so each request is applied in the
\* step that sends it. Its answer travels separately: it arrives, it is lost
\* with its connection, which fails that link, or it cannot be decoded, which
\* leaves the link up. A request may also be lost before the Keeper applies
\* it. Every Host deadline may pass at any moment, and the new Host may claim
\* or decline whenever it holds an import, which covers every order in which
\* the Worker's requests and the deadlines arrive. The Worker acts on facts the
\* Hosts publish, so a lost Worker request only delays it.
\*
\* The pipe holds the program's whole output from the start. No reader has to
\* read, so this covers every moment at which the program writes it.
\*
\* Bounds: one terminal whose output is two bytes forming one sequence, so a
\* reader stops before, inside, or after it; two quiesces, each holding one
\* key; one undecodable answer; each Host, Keeper, and the first Worker crash
\* at most once, and the second Keeper may start at any moment. Left out: a
\* lost `inspect`, which fails the restore like a missing mark; lost `hold`
\* answers; the terminal's own end, and `release`, during a move;
\* DetachKeeper and the Keeper change's status comparison, so a Keeper leaves
\* only by ending; the writer and signal waits and the size limits, which
\* refuse a quiesce like a withdrawn pause; a third Keeper; moves started by
\* a replacement Worker or back to the old Host; and lost Worker requests.
\*
\* Claims: the proposal's TH-028 to TH-034 join the runtime model with the
\* behavior they state; until then this model maps to no runtime claim.
\*
\* The eight design flags are TRUE in the checked configuration; each mutant
\* configuration sets one to FALSE.

CONSTANTS
  MaxBytes,                 \* output bytes in the pipe
  SequenceStart,            \* the first byte of the one two-byte sequence
  MaxQuiesces,              \* QuiesceTerminal requests; each mints one mark id
  MaxUnreadable,            \* Keeper answers a Host cannot decode
  ClaimNeedsMark,           \* a claim needs a mark for the claimant
  ClaimNeedsCurrentMark,    \* a claim needs the holding's current mark
  ResumeRefusedAfterClaim,  \* a resume after a claim is answered claimed
  MarkAfterPark,            \* the old Host marks only after its reader parked
  ParkAtGround,             \* the reader parks only at ground
  ReadAfterClaimed,         \* the new Host reads only after seeing claimed
  ResumeNeedsAnswer,        \* the old Host reads again only on resumed
  HoldOnlyRead              \* a Host holds only a terminal it reads

Hosts == {"old", "new"}
Keepers == {"k1", "k2"}
Marks == 1..MaxQuiesces
Other(h) == IF h = "old" THEN "new" ELSE "old"

\* A Keeper's process state and its one holding. `consumed` is the mark a
\* claim from the old Host consumed, which the holding remembers until the old
\* Host ends. A holding whose owner has ended is orphaned.
KeeperStates ==
  [state: {"off", "live", "ended"}, owner: {"none", "old", "new"},
   mark: 0..MaxQuiesces, consumed: 0..MaxQuiesces]

Empty(state) == [state |-> state, owner |-> "none", mark |-> 0, consumed |-> 0]

VARIABLES
  taken,         \* bytes the readers have taken from the pipe
  applied,       \* each Host's applied bytes, a restored record included
  alive,         \* each Host process runs
  oReader,       \* old Host reader: "on", "pause", "parked", "off"
  oStep,         \* old Host quiesce step
  oMark,         \* the mark id of the old Host's current quiesce
  oKeeper,       \* the Keeper that holds that mark
  nStep,         \* new Host import step
  nImport,       \* the mark id its import names
  nKeeper,       \* the Keeper it inspected
  nSaw,          \* the new Host has seen claimed
  keeper,        \* each Keeper
  link,          \* each Host's connection to each Keeper
  answer,        \* the Keeper answer in flight to each Host
  unreadable,    \* answers spent on the undecodable budget
  records,       \* each mark's exported record, empty until exported
  relay,         \* records the Worker relays for RestoreTerminal
  worker,        \* "first", or "second" after a crash and replacement
  holding,       \* the first Worker holds the terminal's input
  quiesces,      \* QuiesceTerminal requests made
  typed,         \* keys the first Worker accepted while holding
  delivered,     \* keys the Worker sent on to a Host
  misdelivered   \* a key reached a Host while the other read

oldVars == <<oReader, oStep, oMark, oKeeper>>
newVars == <<nStep, nImport, nKeeper, nSaw>>
netVars == <<keeper, link, answer, unreadable>>
moveVars == <<records, relay>>
workerVars == <<worker, holding, quiesces, typed, delivered, misdelivered>>
vars == <<taken, applied, alive, oldVars, newVars, netVars, moveVars, workerVars>>

Taken == [i \in 1..taken |-> i]
AtGround(bytes) == IF bytes = <<>> THEN TRUE ELSE bytes[Len(bytes)] # SequenceStart

\* A Host's reader runs: it takes the next output from the pipe.
Reads(h) ==
  /\ alive[h]
  /\ IF h = "old" THEN oReader \in {"on", "pause"} ELSE nStep = "reading"

\* A Host serves the terminal outside any move: it reads and takes input.
Serving(h) ==
  /\ alive[h]
  /\ IF h = "old" THEN oReader = "on" /\ oStep = "none" ELSE nStep = "reading"

QuiesceSteps == {"pausing", "marking", "quiesced", "resuming", "stopped"}
Attached(h) == {k \in Keepers : link[h][k] # "none"}
Orphaned(k) == keeper[k].owner \in Hosts /\ ~alive[keeper[k].owner]

MarkTakes(h) == h.owner = "old" /\ h.mark = 0
ResumeTakes(h, m) == h.owner = "old" /\ h.mark \in {0, m}
ClaimedUnder(h, m) == h.owner = "new" /\ h.consumed = m
ClaimTakes(h, m) ==
  /\ h.owner = "old"
  /\ IF h.mark = 0 THEN ~ClaimNeedsMark ELSE h.mark = m \/ ~ClaimNeedsCurrentMark
Inspectable(k, m) ==
  link["new"][k] = "ok" /\ keeper[k].owner = "old" /\ keeper[k].mark = m

Init ==
  /\ taken = 0
  /\ applied = [h \in Hosts |-> <<>>]
  /\ alive = [h \in Hosts |-> TRUE]
  /\ oReader = "on"
  /\ oStep = "none"
  /\ oMark = 0
  /\ oKeeper = "k1"
  /\ nStep = "none"
  /\ nImport = 0
  /\ nKeeper = "k1"
  /\ nSaw = FALSE
  /\ keeper = [k \in Keepers |->
       IF k = "k1" THEN [Empty("live") EXCEPT !.owner = "old"] ELSE Empty("off")]
  /\ link = [h \in Hosts |-> [k \in Keepers |-> IF k = "k1" THEN "ok" ELSE "none"]]
  /\ answer = [h \in Hosts |-> "none"]
  /\ unreadable = 0
  /\ records = [m \in Marks |-> <<>>]
  /\ relay = {}
  /\ worker = "first"
  /\ holding = FALSE
  /\ quiesces = 0
  /\ typed = 0
  /\ delivered = 0
  /\ misdelivered = FALSE

\* The old Host reads again outside any move.
Serve ==
  /\ oReader' = "on"
  /\ oStep' = "none"
  /\ oMark' = 0
  /\ oKeeper' = "k1"

\* The connection breaks: the request or its answer is lost.
Lost(h, k) ==
  /\ link' = [link EXCEPT ![h][k] = IF @ = "ok" THEN "failed" ELSE @]
  /\ UNCHANGED <<keeper, unreadable>>

\* The answer arrives but the Host cannot decode it.
Unreadable ==
  /\ unreadable < MaxUnreadable
  /\ unreadable' = unreadable + 1
  /\ UNCHANGED <<keeper, link>>

NoAnswer(h, k) == Lost(h, k) \/ Unreadable

Discard ==
  /\ nStep' = "discarded"
  /\ nImport' = 0
  /\ applied' = [applied EXCEPT !["new"] = <<>>]

Open ==
  /\ nStep' = "reading"
  /\ nSaw' = TRUE
  /\ UNCHANGED <<nImport, applied>>

-----------------------------------------------------------------------------
\* The reader.

Read(h) ==
  /\ Reads(h)
  /\ taken < MaxBytes
  /\ taken' = taken + 1
  /\ applied' = [applied EXCEPT ![h] = Append(@, taken + 1)]
  /\ UNCHANGED <<alive, oldVars, newVars, netVars, moveVars, workerVars>>

-----------------------------------------------------------------------------
\* The old Host's quiesce.

\* The Worker holds input, accepts a key, and asks an eligible old Host to
\* quiesce: it serves the terminal, holds it in its one Keeper, and has no
\* second attachment.
Quiesce ==
  /\ worker = "first"
  /\ ~holding
  /\ quiesces < MaxQuiesces
  /\ Serving("old")
  /\ \E k \in Keepers:
       /\ Attached("old") = {k}
       /\ link["old"][k] = "ok"
       /\ keeper[k].owner = "old"
       /\ oKeeper' = k
  /\ oReader' = "pause"
  /\ oStep' = "pausing"
  /\ quiesces' = quiesces + 1
  /\ holding' = TRUE
  /\ typed' = typed + 1
  /\ UNCHANGED <<taken, applied, alive, oMark, newVars, netVars, moveVars,
                 worker, delivered, misdelivered>>

Park ==
  /\ alive["old"]
  /\ oReader = "pause"
  /\ ParkAtGround => AtGround(applied["old"])
  /\ oReader' = "parked"
  /\ UNCHANGED <<taken, applied, alive, oStep, oMark, oKeeper, newVars, netVars,
                 moveVars, workerVars>>

\* A quiesce refused as passing, or an export that failed: the reader reads on.
Withdraw ==
  /\ alive["old"]
  /\ oStep = "pausing"
  /\ Serve
  /\ UNCHANGED <<taken, applied, alive, newVars, netVars, moveVars, workerVars>>

\* Export the record, then mark the holding for the new Host. A mark that
\* cannot be sent is a refused one; a mark left unanswered counts as made.
Mark ==
  /\ alive["old"]
  /\ oStep = "pausing"
  /\ answer["old"] = "none"
  /\ oReader = "parked" \/ (~MarkAfterPark /\ oReader = "pause")
  /\ records' = [records EXCEPT ![quiesces] = applied["old"]]
  /\ IF link["old"][oKeeper] = "ok"
       THEN LET h == keeper[oKeeper] IN
            \/ /\ keeper' = [keeper EXCEPT ![oKeeper] =
                    IF MarkTakes(h) THEN [h EXCEPT !.mark = quiesces] ELSE h]
               /\ answer' = [answer EXCEPT !["old"] =
                    IF MarkTakes(h) THEN "ok" ELSE "refused"]
               /\ oStep' = "marking"
               /\ oMark' = quiesces
               /\ UNCHANGED <<link, unreadable, oReader, oKeeper>>
            \/ /\ Lost("old", oKeeper)
               /\ oStep' = "stopped"
               /\ oMark' = quiesces
               /\ UNCHANGED <<answer, oReader, oKeeper>>
       ELSE /\ Serve
            /\ UNCHANGED netVars
  /\ UNCHANGED <<taken, applied, alive, newVars, relay, workerVars>>

\* `resume` to the Keeper that holds the mark, at the mark's deadline or on the
\* Worker's ResumeTerminal.
Resume ==
  /\ alive["old"]
  /\ oStep \in {"quiesced", "stopped"}
  /\ answer["old"] = "none"
  /\ link["old"][oKeeper] = "ok"
  /\ LET h == keeper[oKeeper] IN
     \/ /\ keeper' = [keeper EXCEPT ![oKeeper] =
             IF ResumeTakes(h, oMark) THEN [h EXCEPT !.mark = 0]
             ELSE IF ClaimedUnder(h, oMark) /\ ~ResumeRefusedAfterClaim
               THEN [h EXCEPT !.owner = "old", !.consumed = 0]
             ELSE h]
        /\ answer' = [answer EXCEPT !["old"] =
             IF ResumeTakes(h, oMark) THEN "resumed"
             ELSE IF ClaimedUnder(h, oMark)
               THEN IF ResumeRefusedAfterClaim THEN "claimed" ELSE "resumed"
             ELSE "refused"]
        /\ oStep' = "resuming"
        /\ UNCHANGED <<link, unreadable>>
     \/ /\ Lost("old", oKeeper)
        /\ oStep' = "stopped"
        /\ UNCHANGED answer
  /\ UNCHANGED <<taken, applied, alive, oReader, oMark, oKeeper, newVars,
                 moveVars, workerVars>>

\* `resumed` reads again and `claimed` reports the terminal as moved; any
\* other answer, or none, leaves the terminal stopped.
OldReply ==
  /\ answer["old"] # "none"
  /\ answer' = [answer EXCEPT !["old"] = "none"]
  /\ \/ /\ UNCHANGED <<keeper, link, unreadable>>
        /\ IF oStep = "marking" /\ answer["old"] = "ok"
             THEN /\ oStep' = "quiesced"
                  /\ relay' = relay \cup {oMark}
                  /\ UNCHANGED <<oReader, oMark, oKeeper>>
           ELSE IF oStep = "marking" \/ answer["old"] = "resumed"
             THEN /\ Serve
                  /\ UNCHANGED relay
           ELSE /\ oStep' = IF answer["old"] = "claimed" THEN "moved" ELSE "stopped"
                /\ UNCHANGED <<relay, oReader, oMark, oKeeper>>
     \/ /\ NoAnswer("old", oKeeper)
        /\ oStep' = "stopped"
        /\ UNCHANGED <<relay, oReader, oMark, oKeeper>>
  /\ UNCHANGED <<taken, applied, alive, newVars, records, workerVars>>

\* Present only in the mutant that resumes at the deadline without an answer.
DeadlineResume ==
  /\ ~ResumeNeedsAnswer
  /\ alive["old"]
  /\ oStep \in {"quiesced", "stopped"}
  /\ answer["old"] = "none"
  /\ Serve
  /\ UNCHANGED <<taken, applied, alive, newVars, netVars, moveVars, workerVars>>

-----------------------------------------------------------------------------
\* The new Host's import.

\* RestoreTerminal: validate, `inspect` the current mark, pull, check, pack,
\* and create the gated threads. A failed inspect fails the restore, and a
\* second import of the holding is refused as moving.
Restore(m) ==
  /\ m \in relay
  /\ relay' = relay \ {m}
  /\ IF nStep \in {"none", "discarded"} /\ \E k \in Keepers: Inspectable(k, m)
       THEN \E k \in Keepers:
              /\ Inspectable(k, m)
              /\ nStep' = "restored"
              /\ nImport' = m
              /\ nKeeper' = k
              /\ applied' = [applied EXCEPT !["new"] = records[m]]
       ELSE UNCHANGED <<nStep, nImport, nKeeper, applied>>
  /\ UNCHANGED <<taken, alive, oldVars, nSaw, netVars, records, workerVars>>

\* `claim`, the commit.
Claim ==
  /\ alive["new"]
  /\ nStep = "restored"
  /\ answer["new"] = "none"
  /\ link["new"][nKeeper] = "ok"
  /\ LET h == keeper[nKeeper] IN
     \/ /\ keeper' = [keeper EXCEPT ![nKeeper] =
             IF ClaimTakes(h, nImport)
               THEN [h EXCEPT !.owner = "new", !.mark = 0, !.consumed = nImport]
               ELSE h]
        /\ answer' = [answer EXCEPT !["new"] =
             IF ClaimTakes(h, nImport) \/ ClaimedUnder(h, nImport)
               THEN "claimed" ELSE "refused"]
        /\ nStep' = "claiming"
        /\ UNCHANGED <<link, unreadable>>
     \/ /\ Lost("new", nKeeper)
        /\ UNCHANGED <<answer, nStep>>
  /\ UNCHANGED <<taken, applied, alive, oldVars, nImport, nKeeper, nSaw, moveVars,
                 workerVars>>

\* `decline` before discarding, on DiscardImport or at the import's deadline.
Decline ==
  /\ alive["new"]
  /\ nStep \in {"restored", "refused"}
  /\ answer["new"] = "none"
  /\ IF link["new"][nKeeper] = "ok"
       THEN LET h == keeper[nKeeper] IN
            \/ /\ keeper' = [keeper EXCEPT ![nKeeper] =
                    IF h.owner = "old" /\ h.mark = nImport THEN [h EXCEPT !.mark = 0] ELSE h]
               /\ answer' = [answer EXCEPT !["new"] =
                    IF ClaimedUnder(h, nImport) THEN "claimed" ELSE "declined"]
               /\ nStep' = "declining"
               /\ UNCHANGED <<link, unreadable, nImport, applied>>
            \/ /\ Lost("new", nKeeper)
               /\ Discard
               /\ UNCHANGED answer
       ELSE /\ Discard
            /\ UNCHANGED netVars
  /\ UNCHANGED <<taken, alive, oldVars, nKeeper, nSaw, moveVars, workerVars>>

\* Whichever answer first shows claimed completes the claim.
NewReply ==
  /\ answer["new"] # "none"
  /\ answer' = [answer EXCEPT !["new"] = "none"]
  /\ \/ /\ UNCHANGED <<keeper, link, unreadable>>
        /\ IF answer["new"] = "claimed"
             THEN Open
           ELSE IF nStep = "claiming"
             THEN /\ nStep' = "refused"
                  /\ UNCHANGED <<nImport, nSaw, applied>>
           ELSE /\ Discard
                /\ UNCHANGED nSaw
     \/ /\ NoAnswer("new", nKeeper)
        /\ IF nStep = "claiming"
             THEN /\ nStep' = IF ReadAfterClaimed THEN "restored" ELSE "reading"
                  /\ UNCHANGED <<nImport, nSaw, applied>>
           ELSE /\ Discard
                /\ UNCHANGED nSaw
  /\ UNCHANGED <<taken, alive, oldVars, nKeeper, moveVars, workerVars>>

-----------------------------------------------------------------------------
\* Keepers.

Holdable(h) ==
  \/ Serving(h)
  \/ ~HoldOnlyRead /\ h = "old" /\ oStep \in QuiesceSteps

\* `hold` after AttachKeeper, or once a quiesced terminal reads again.
Hold(h, k) ==
  /\ alive[h]
  /\ keeper[k].state = "live"
  /\ link[h][k] = "ok"
  /\ keeper[k].owner = "none"
  /\ Holdable(h)
  /\ keeper' = [keeper EXCEPT ![k].owner = h]
  /\ UNCHANGED <<taken, applied, alive, oldVars, newVars, link, answer, unreadable,
                 moveVars, workerVars>>

\* The Worker starts the second Keeper, registers each running Host, and sends
\* it AttachKeeper.
StartSecondKeeper ==
  /\ keeper["k2"].state = "off"
  /\ keeper' = [keeper EXCEPT !["k2"].state = "live"]
  /\ link' = [h \in Hosts |-> [link[h] EXCEPT !["k2"] = IF alive[h] THEN "ok" ELSE "none"]]
  /\ UNCHANGED <<taken, applied, alive, oldVars, newVars, answer, unreadable,
                 moveVars, workerVars>>

\* A Keeper ends: it crashes, or the Worker ends it. Its duplicates close, so
\* its holding is gone, and every Host loses its link.
KeeperEnds(k) ==
  /\ keeper[k].state = "live"
  /\ keeper' = [keeper EXCEPT ![k] = Empty("ended")]
  /\ link' = [h \in Hosts |-> [link[h] EXCEPT ![k] = "none"]]
  /\ UNCHANGED <<taken, applied, alive, oldVars, newVars, answer, unreadable,
                 moveVars, workerVars>>

KeeperCrash == \E k \in Keepers: KeeperEnds(k)

\* The Worker ends a Keeper that missed a Host's deadline.
WorkerEndsKeeper ==
  \E k \in Keepers: (\E h \in Hosts: link[h][k] = "failed") /\ KeeperEnds(k)

-----------------------------------------------------------------------------
\* The Worker.

\* Held input goes to the Host the move settled on: the new Host once it has
\* seen claimed, or the old Host once it reads again.
Release ==
  /\ worker = "first"
  /\ holding
  /\ \E h \in Hosts:
       /\ IF h = "new" THEN nSaw /\ Serving("new") ELSE Serving("old")
       /\ misdelivered' = (misdelivered \/ Reads(Other(h)))
  /\ holding' = FALSE
  /\ delivered' = typed
  /\ UNCHANGED <<taken, applied, alive, oldVars, newVars, netVars, moveVars,
                 worker, quiesces, typed>>

\* DropTerminal, only after the new Host has seen claimed, whether or not the
\* old Host has seen it too.
Drop ==
  /\ nSaw
  /\ alive["old"]
  /\ answer["old"] = "none"
  /\ oStep \in {"quiesced", "stopped", "moved"}
  /\ oReader' = "off"
  /\ oStep' = "dropped"
  /\ oMark' = 0
  /\ oKeeper' = "k1"
  /\ applied' = [applied EXCEPT !["old"] = <<>>]
  /\ UNCHANGED <<taken, alive, newVars, netVars, moveVars, workerVars>>

\* ResumeTerminal without the Keeper: only after the Keeper holding the mark
\* has ended and the new Host discarded its import without seeing claimed.
ResumeWithoutKeeper ==
  /\ alive["old"]
  /\ oStep \in {"quiesced", "stopped", "moved"}
  /\ answer["old"] = "none"
  /\ keeper[oKeeper].state = "ended"
  /\ alive["new"]
  /\ nStep \in {"none", "discarded"}
  /\ ~nSaw
  /\ Serve
  /\ UNCHANGED <<taken, applied, alive, newVars, netVars, moveVars, workerVars>>

\* The first Worker crashes, losing the keys it held, and a replacement runs.
WorkerCrash ==
  /\ worker = "first"
  /\ worker' = "second"
  /\ holding' = FALSE
  /\ UNCHANGED <<taken, applied, alive, oldVars, newVars, netVars, moveVars,
                 quiesces, typed, delivered, misdelivered>>

-----------------------------------------------------------------------------
\* Host crashes. A Host's end orphans the holdings it owns, and a holding
\* forgets the claim from the old Host once that Host ends.

OldCrash ==
  /\ alive["old"]
  /\ alive' = [alive EXCEPT !["old"] = FALSE]
  /\ oReader' = "off"
  /\ oStep' = "none"
  /\ oMark' = 0
  /\ oKeeper' = "k1"
  /\ applied' = [applied EXCEPT !["old"] = <<>>]
  /\ answer' = [answer EXCEPT !["old"] = "none"]
  /\ link' = [link EXCEPT !["old"] = [k \in Keepers |-> "none"]]
  /\ keeper' = [k \in Keepers |->
       IF keeper[k].owner = "new" THEN [keeper[k] EXCEPT !.consumed = 0] ELSE keeper[k]]
  /\ UNCHANGED <<taken, newVars, unreadable, moveVars, workerVars>>

NewCrash ==
  /\ alive["new"]
  /\ alive' = [alive EXCEPT !["new"] = FALSE]
  /\ nStep' = "none"
  /\ nImport' = 0
  /\ nKeeper' = "k1"
  /\ applied' = [applied EXCEPT !["new"] = <<>>]
  /\ answer' = [answer EXCEPT !["new"] = "none"]
  /\ link' = [link EXCEPT !["new"] = [k \in Keepers |-> "none"]]
  /\ relay' = {}
  /\ UNCHANGED <<taken, oldVars, nSaw, keeper, unreadable, records, workerVars>>

-----------------------------------------------------------------------------

Next ==
  \/ \E h \in Hosts: Read(h)
  \/ Quiesce
  \/ Park
  \/ Withdraw
  \/ Mark
  \/ Resume
  \/ OldReply
  \/ DeadlineResume
  \/ \E m \in Marks: Restore(m)
  \/ Claim
  \/ Decline
  \/ NewReply
  \/ \E h \in Hosts, k \in Keepers: Hold(h, k)
  \/ StartSecondKeeper
  \/ KeeperCrash
  \/ WorkerEndsKeeper
  \/ Release
  \/ Drop
  \/ ResumeWithoutKeeper
  \/ WorkerCrash
  \/ OldCrash
  \/ NewCrash

\* The proposal's progress assumptions: a quiesce settles within its second,
\* answers arrive or are lost, each deadline passes, and a Worker runs, ending
\* a Keeper that missed a deadline and resuming without it.
Fairness ==
  /\ WF_vars(Withdraw)
  /\ WF_vars(Mark)
  /\ WF_vars(Resume)
  /\ WF_vars(OldReply)
  /\ WF_vars(Decline)
  /\ WF_vars(NewReply)
  /\ WF_vars(WorkerEndsKeeper)
  /\ WF_vars(ResumeWithoutKeeper)

Spec == Init /\ [][Next]_vars /\ Fairness

-----------------------------------------------------------------------------

TypeOK ==
  /\ taken \in 0..MaxBytes
  /\ applied \in [Hosts -> Seq(1..MaxBytes)]
  /\ alive \in [Hosts -> BOOLEAN]
  /\ oReader \in {"on", "pause", "parked", "off"}
  /\ oStep \in {"none", "dropped", "moved"} \cup QuiesceSteps
  /\ oMark \in 0..MaxQuiesces
  /\ oKeeper \in Keepers
  /\ nStep \in {"none", "restored", "claiming", "refused", "declining", "reading", "discarded"}
  /\ nImport \in 0..MaxQuiesces
  /\ nKeeper \in Keepers
  /\ nSaw \in BOOLEAN
  /\ keeper \in [Keepers -> KeeperStates]
  /\ link \in [Hosts -> [Keepers -> {"none", "ok", "failed"}]]
  /\ answer \in [Hosts -> {"none", "ok", "refused", "resumed", "claimed", "declined"}]
  /\ unreadable \in 0..MaxUnreadable
  /\ records \in [Marks -> Seq(1..MaxBytes)]
  /\ relay \subseteq Marks
  /\ worker \in {"first", "second"}
  /\ holding \in BOOLEAN
  /\ quiesces \in 0..MaxQuiesces
  /\ typed \in 0..MaxQuiesces
  /\ delivered \in 0..typed
  /\ misdelivered \in BOOLEAN

\* At most one Host reads the terminal, and no running Keeper names another
\* Host as its owner while it does: either it is the owner, or the Keeper has
\* ended and the Worker resumed it.
OneReader ==
  /\ ~(Reads("old") /\ Reads("new"))
  /\ \A h \in Hosts, k \in Keepers:
       Reads(h) /\ keeper[k].state = "live" => keeper[k].owner \in {"none", h}

\* The reading Host has applied every byte taken from the pipe, a restored
\* record's included, once and in order, so its bytes followed by the pipe's
\* are the program's output.
NoLossOrRepeat == \A h \in Hosts: Reads(h) => applied[h] = Taken

\* No record ends inside a sequence.
RecordsEndAtGround == \A m \in Marks: AtGround(records[m])

\* Held keys reach only a Host that is the terminal's one reader, each once;
\* while the first Worker lives, every key it accepted is held or delivered.
KeysOnce ==
  /\ ~misdelivered
  /\ worker = "first" /\ ~holding => delivered = typed

\* From the export until it reads again or drops the terminal, the old Host
\* keeps exactly the state it exported, so an abort leaves its state plus what
\* it reads afterwards.
AbortRestoresOld ==
  alive["old"] /\ oStep \in {"marking", "quiesced", "resuming", "stopped", "moved"}
    => applied["old"] = records[oMark]

\* The new Host reads only after it has seen claimed, and the old Host never
\* reads after that.
OnlyClaimCommits ==
  /\ Reads("new") => nSaw
  /\ nSaw => ~Reads("old")

\* A running Keeper's holding names a Host that reads the terminal or may read
\* it through that Keeper: the old Host while its quiesce there is unsettled,
\* and the new Host through the Keeper it claimed in until it discards. A
\* claim whose answer the new Host lost names it until the Worker ends that
\* Keeper. An orphaned holding names no one.
HoldingsNameReaders ==
  \A k \in Keepers:
    keeper[k].state = "live" /\ ~Orphaned(k) =>
      /\ keeper[k].owner = "old" =>
           Serving("old") \/ (oStep \in QuiesceSteps /\ oKeeper = k)
      /\ keeper[k].owner = "new" =>
           \/ nStep = "reading"
           \/ nStep \in {"restored", "claiming", "refused", "declining", "discarded"}
                /\ nKeeper = k

HasReader == Reads("old") \/ Reads("new")
HostEnded == ~alive["old"] \/ ~alive["new"]

\* If neither Host crashes and a Worker runs, the terminal again has a reader.
Progress == ~HasReader ~> (HasReader \/ HostEnded)

=============================================================================
