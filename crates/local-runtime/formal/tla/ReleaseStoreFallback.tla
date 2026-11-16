------------------------- MODULE ReleaseStoreFallback -------------------------
(* The release store's fallback record, `extensions.fallback` in active.json, under every      *)
(* interleaving of its writers: completions that move the pointer, recorders of starts and      *)
(* failures that decide without the lock first, and a crash at any point. Version 0 stands for  *)
(* a release before slice 1 or a build whose automatic-update switch is off; a version folder   *)
(* in `disk` stands for its intact release.json. Each flag is a guard of the protocol that a    *)
(* negative control turns off.                                                                  *)
EXTENDS Integers, FiniteSets

CONSTANTS Versions, Floor, Gens, Writers, Nobody, Threshold, Window, MaxTime, MaxMoves,
          LockedUpdate, BindRecord, HoldInSameWrite, SkipHeld, AttributeStarts,
          ProofRequired, FloorChecked

\* The highest component floor in version v's own manifest, which the replaced version must meet.
FloorOfVersion == [v \in Versions |-> IF v = 3 THEN 2 ELSE 0]

None == -1
Times == 0..MaxTime
RecordKinds == {"cstart", "cfail", "sstart", "sfail"}

Rec(held, entry, back) ==
  [held |-> held, entry |-> entry, back |-> back, csince |-> None, cfail |-> 0,
   ssince |-> None, starts |-> {}, failed |-> {}]
\* A pointer without a record behaves as one whose move counts for no entry and holds nothing.
NoRec == Rec({}, None, FALSE)
Op(kind, v, g) == [kind |-> kind, v |-> v, g |-> g]
NoOp == Op("none", None, None)
NoSnap == [active |-> None, previous |-> None, rec |-> NoRec]

VARIABLES active, previous, disk, rec, lock, pc, op, snap, now, gver, moves,
          fell, hist, gcf, gsrc, gstarts, gfailed, faults

store == <<active, previous, disk, rec>>
ghost == <<fell, hist, gcf, gsrc, gstarts, gfailed, faults>>
vars == <<active, previous, disk, rec, lock, pc, op, snap, now, gver, moves,
          fell, hist, gcf, gsrc, gstarts, gfailed, faults>>

Cur == [active |-> active, previous |-> previous, rec |-> rec]
\* What a commit computes from: the pointer read under the lock, or the one read before it.
\* `snap` is kept only when LockedUpdate is FALSE, since nothing else reads it.
Base(w) == IF LockedUpdate THEN Cur ELSE snap[w]

Counting(p) == p.rec.entry = p.active
Open(since) == since = None \/ now < since + Window
Since(since) == IF since = None THEN now ELSE since
Bound(p, o) == BindRecord => o.v = p.rec.entry
Above(set, v) == {h \in set : h > v}
\* Every change of `active` drops from `hist` only the versions below the new active one.
KeepHist(set, v) == {h \in set : h >= v}

\* Whether a record counts toward p's move: a recorder's check before the lock and under it.
Counts(p, o) ==
  /\ Counting(p) /\ p.rec.back
  /\ CASE o.kind = "cstart" -> p.rec.csince = None /\ Bound(p, o)
       [] o.kind = "cfail" -> Open(p.rec.csince) /\ p.rec.cfail < Threshold /\ Bound(p, o)
       [] o.kind = "sstart" ->
            Open(p.rec.ssince) /\ Cardinality(p.rec.failed) < Threshold /\ Bound(p, o)
       [] o.kind = "sfail" ->
            /\ p.rec.ssince # None /\ Open(p.rec.ssince)
            /\ Cardinality(p.rec.failed) < Threshold /\ o.g \notin p.rec.failed
            /\ AttributeStarts => o.g \in p.rec.starts
       [] OTHER -> FALSE

Failures(p, o) == IF o.kind = "cfail" THEN p.rec.cfail + 1 ELSE Cardinality(p.rec.failed) + 1

\* The write with a role's Threshold-th failure falls back when the folder `previous` names exists.
FallsBack(p, o) ==
  /\ o.kind \in {"cfail", "sfail"} /\ Counts(p, o) /\ Failures(p, o) = Threshold
  /\ p.previous # None /\ p.previous \in disk

Recorded(p, o) ==
  CASE o.kind = "cstart" -> [p.rec EXCEPT !.csince = now]
    [] o.kind = "cfail" -> [p.rec EXCEPT !.csince = Since(@), !.cfail = @ + 1]
    [] o.kind = "sstart" -> [p.rec EXCEPT !.ssince = Since(@), !.starts = @ \cup {o.g}]
    [] o.kind = "sfail" -> [p.rec EXCEPT !.failed = @ \cup {o.g}]

\* The version a failure belongs to: the Client's, or the one that started the generation.
Source(o) == IF o.kind = "cfail" THEN o.v ELSE gver[o.g]

Init ==
  /\ active = 0 /\ previous = None /\ disk = {0} /\ rec = NoRec
  /\ lock = Nobody /\ pc = [w \in Writers |-> "idle"] /\ op = [w \in Writers |-> NoOp]
  /\ snap = [w \in Writers |-> NoSnap] /\ now = 0 /\ gver = [g \in Gens |-> None]
  /\ moves = 0 /\ fell = FALSE /\ hist = {} /\ gcf = 0 /\ gsrc = {} /\ gstarts = {}
  /\ gfailed = {} /\ faults = {}

Queue(w, o) ==
  /\ pc' = [pc EXCEPT ![w] = "waiting"] /\ op' = [op EXCEPT ![w] = o]
  /\ snap' = [snap EXCEPT ![w] = IF LockedUpdate THEN NoSnap ELSE Cur]

Release(w) ==
  /\ lock' = Nobody /\ pc' = [pc EXCEPT ![w] = "idle"] /\ op' = [op EXCEPT ![w] = NoOp]
  /\ snap' = [snap EXCEPT ![w] = NoSnap]

\* A start notice ends, written or not: the activator starts the generation's Server.
Started(w) == IF op[w].kind = "sstart" THEN [gver EXCEPT ![op[w].g] = op[w].v] ELSE gver

\* The counts restart with each pointer change. `fell` says whether the current move has fallen
\* back; a fallback is no move, so it sets the flag and a later fallback in the same move sees it.
FreshCounts == /\ gcf' = 0 /\ gsrc' = {} /\ gstarts' = {} /\ gfailed' = {}
FreshMove == /\ fell' = FALSE /\ FreshCounts

Offer(w, v) ==
  /\ pc[w] = "idle" /\ moves < MaxMoves /\ v > active
  /\ SkipHeld => v \notin rec.held
  /\ Queue(w, Op("move", v, None))
  /\ UNCHANGED <<store, lock, now, gver, moves, ghost>>

ExplicitInstall(w, v) ==
  /\ pc[w] = "idle" /\ moves < MaxMoves /\ v # 0
  /\ Queue(w, Op("move", v, None))
  /\ UNCHANGED <<store, lock, now, gver, moves, ghost>>

LegacyInstall(w) ==
  /\ pc[w] = "idle" /\ lock = Nobody /\ moves < MaxMoves /\ active # 0
  /\ active' = 0 /\ previous' = active
  /\ disk' = disk \cup {0} /\ moves' = moves + 1
  /\ UNCHANGED <<rec, lock, pc, op, snap, now, gver, ghost>>

StartClient(w) ==
  /\ pc[w] = "idle" /\ Counts(Cur, Op("cstart", active, None))
  /\ Queue(w, Op("cstart", active, None))
  /\ UNCHANGED <<store, lock, now, gver, moves, ghost>>

FailClient(w) ==
  /\ pc[w] = "idle" /\ previous # None /\ Counts(Cur, Op("cfail", active, None))
  /\ Queue(w, Op("cfail", active, None))
  /\ UNCHANGED <<store, lock, now, gver, moves, ghost>>

StartGeneration(w, g) ==
  /\ pc[w] = "idle" /\ gver[g] = None
  /\ \A x \in Writers : ~(op[x].kind = "sstart" /\ op[x].g = g)
  /\ IF Counts(Cur, Op("sstart", active, g))
     THEN Queue(w, Op("sstart", active, g)) /\ UNCHANGED gver
     ELSE gver' = [gver EXCEPT ![g] = active] /\ UNCHANGED <<pc, op, snap>>
  /\ UNCHANGED <<store, lock, now, moves, ghost>>

FailGeneration(w, g) ==
  /\ pc[w] = "idle" /\ gver[g] # None /\ Counts(Cur, Op("sfail", None, g))
  /\ Queue(w, Op("sfail", None, g))
  /\ UNCHANGED <<store, lock, now, gver, moves, ghost>>

Acquire(w) ==
  /\ pc[w] = "waiting" /\ lock = Nobody
  /\ lock' = w /\ pc' = [pc EXCEPT ![w] = "holding"]
  /\ UNCHANGED <<store, op, snap, now, gver, moves, ghost>>

Drop(w) ==
  /\ pc[w] = "waiting"
  /\ pc' = [pc EXCEPT ![w] = "idle"] /\ op' = [op EXCEPT ![w] = NoOp]
  /\ snap' = [snap EXCEPT ![w] = NoSnap] /\ gver' = Started(w)
  /\ UNCHANGED <<store, lock, now, moves, ghost>>

CommitMove(w) ==
  LET p == Base(w)
      t == op[w].v
      back == /\ ProofRequired => Counting(p)
              /\ p.active \in disk
              /\ FloorChecked => p.active >= Floor[t]
  IN /\ pc[w] = "holding" /\ op[w].kind = "move"
     /\ ~(SkipHeld /\ t \in p.rec.held)
     /\ IF t = p.active
        THEN /\ active' = p.active /\ previous' = p.previous /\ rec' = p.rec
             /\ UNCHANGED <<moves, fell, gcf, gsrc, gstarts, gfailed>>
        ELSE /\ active' = t /\ previous' = p.active
             /\ rec' = Rec(Above(p.rec.held, t), t, back)
             /\ moves' = moves + 1 /\ FreshMove
     /\ hist' = KeepHist(hist, active')
     /\ disk' = disk \cup {t}
     /\ Release(w)
     /\ UNCHANGED <<now, gver, faults>>

RefuseHeld(w) ==
  /\ pc[w] = "holding" /\ op[w].kind = "move"
  /\ SkipHeld /\ op[w].v \in Base(w).rec.held
  /\ Release(w)
  /\ UNCHANGED <<store, now, gver, moves, ghost>>

\* A write below the threshold, or a release without a write when the check under the lock fails.
CommitRecord(w) ==
  LET p == Base(w)
      o == op[w]
  IN /\ pc[w] = "holding" /\ o.kind \in RecordKinds
     /\ ~FallsBack(p, o)
     /\ IF Counts(p, o)
        THEN /\ active' = p.active /\ previous' = p.previous /\ rec' = Recorded(p, o)
             /\ gcf' = IF o.kind = "cfail" THEN gcf + 1 ELSE gcf
             /\ gsrc' = IF o.kind \in {"cfail", "sfail"} THEN gsrc \cup {Source(o)} ELSE gsrc
             /\ gstarts' = IF o.kind = "sstart" THEN gstarts \cup {o.g} ELSE gstarts
             /\ gfailed' = IF o.kind = "sfail" THEN gfailed \cup {o.g} ELSE gfailed
        ELSE UNCHANGED <<active, previous, rec, gcf, gsrc, gstarts, gfailed>>
     /\ hist' = KeepHist(hist, active')
     /\ gver' = Started(w)
     /\ Release(w)
     /\ UNCHANGED <<disk, now, moves, fell, faults>>

CommitFallback(w) ==
  LET p == Base(w)
      o == op[w]
      failed == p.active
      target == p.previous
      client == o.kind = "cfail"
  IN /\ pc[w] = "holding" /\ FallsBack(p, o)
     /\ active' = target /\ previous' = None
     /\ rec' = Rec(Above(p.rec.held \cup (IF HoldInSameWrite THEN {failed} ELSE {}), target),
                   target, FALSE)
     /\ FreshCounts
     /\ hist' = KeepHist(hist \cup Above({failed}, target), target)
     \* Each fallback is checked as it is written; a fault, once seen, stays.
     /\ fell' = TRUE
     /\ faults' = faults \cup
          {f \in {"unproven", "floor", "window", "twice"} :
             \/ f = "unproven" /\ target = 0
             \/ f = "floor" /\ target < Floor[failed]
             \/ f = "window" /\ now >= Since(IF client THEN p.rec.csince ELSE p.rec.ssince) + Window
             \/ f = "twice" /\ fell}
     /\ IF HoldInSameWrite
        THEN Release(w)
        ELSE /\ lock' = Nobody /\ pc' = [pc EXCEPT ![w] = "rehold"]
             /\ op' = [op EXCEPT ![w] = Op("hold", failed, None)]
             /\ snap' = [snap EXCEPT ![w] = NoSnap]
     /\ UNCHANGED <<disk, now, gver, moves>>

HoldLater(w) ==
  /\ ~HoldInSameWrite /\ pc[w] = "rehold" /\ lock = Nobody
  /\ rec' = [rec EXCEPT !.held = Above(@ \cup {op[w].v}, active)]
  /\ pc' = [pc EXCEPT ![w] = "idle"] /\ op' = [op EXCEPT ![w] = NoOp]
  /\ UNCHANGED <<active, previous, disk, lock, snap, now, gver, moves, ghost>>

\* The lock is released, and nothing the writer had not written survives.
Crash(w) ==
  /\ pc[w] \in {"waiting", "holding", "rehold"}
  /\ lock' = IF lock = w THEN Nobody ELSE lock
  /\ pc' = [pc EXCEPT ![w] = "idle"] /\ op' = [op EXCEPT ![w] = NoOp]
  /\ snap' = [snap EXCEPT ![w] = NoSnap]
  /\ UNCHANGED <<store, now, gver, moves, ghost>>

\* Retention after a completion: every folder that is neither active nor previous may go.
Cleanup ==
  /\ disk \ {active, previous} # {} /\ disk' = disk \cap {active, previous}
  /\ UNCHANGED <<active, previous, rec, lock, pc, op, snap, now, gver, moves, ghost>>

Tick ==
  /\ now < MaxTime /\ now' = now + 1
  /\ UNCHANGED <<store, lock, pc, op, snap, gver, moves, ghost>>

Next ==
  \/ \E w \in Writers :
       \/ \E v \in Versions : Offer(w, v) \/ ExplicitInstall(w, v)
       \/ LegacyInstall(w) \/ StartClient(w) \/ FailClient(w)
       \/ \E g \in Gens : StartGeneration(w, g) \/ FailGeneration(w, g)
       \/ Acquire(w) \/ Drop(w) \/ CommitMove(w) \/ RefuseHeld(w)
       \/ CommitRecord(w) \/ CommitFallback(w) \/ HoldLater(w) \/ Crash(w)
  \/ Cleanup \/ Tick

\* Weak fairness of each writer's lock steps together, which the weak fairness of each one alone
\* implies.
Settle(w) ==
  \/ Acquire(w) \/ Drop(w) \/ CommitMove(w) \/ RefuseHeld(w)
  \/ CommitRecord(w) \/ CommitFallback(w) \/ HoldLater(w)

Spec == Init /\ [][Next]_vars /\ \A w \in Writers : WF_vars(Settle(w))

TypeOK ==
  /\ active \in Versions /\ previous \in Versions \cup {None} /\ disk \subseteq Versions
  /\ rec.held \subseteq Versions /\ rec.entry \in Versions \cup {None} /\ rec.back \in BOOLEAN
  /\ rec.csince \in Times \cup {None} /\ rec.cfail \in 0..Threshold
  /\ rec.ssince \in Times \cup {None} /\ rec.starts \subseteq Gens /\ rec.failed \subseteq Gens
  /\ lock \in Writers \cup {Nobody}
  /\ pc \in [Writers -> {"idle", "waiting", "holding", "rehold"}]
  /\ now \in Times /\ gver \in [Gens -> Versions \cup {None}]
  /\ moves \in 0..(MaxMoves + Cardinality(Writers))

\* Every start and failure committed since the last fresh move is still counted.
NoLostRecord == gcf <= rec.cfail /\ gstarts \subseteq rec.starts /\ gfailed \subseteq rec.failed
\* A failure counts only toward the move of the version it belongs to.
RecordBound == gsrc \subseteq {rec.entry}
Attribution == rec.failed \subseteq rec.starts
\* No version a fallback left becomes active again before a higher one has.
NoReturn == active \notin hist
TargetProven == "unproven" \notin faults
FallbackAboveFloor == "floor" \notin faults
HeldNeverActive == active \notin rec.held /\ previous \notin rec.held
BackImpliesPreviousOnDisk ==
  (Counting(Cur) /\ rec.back) => (previous # None /\ previous \in disk)
FallbackWithinWindow == "window" \notin faults
AtMostOncePerMove == "twice" \notin faults

MoveNeverReverted ==
  [][<<active, previous>>' # <<active, previous>> =>
       \E w \in Writers : CommitMove(w) \/ CommitFallback(w) \/ LegacyInstall(w)]_vars

EveryOpEnds == \A w \in Writers : pc[w] # "idle" ~> pc[w] = "idle"
================================================================================
