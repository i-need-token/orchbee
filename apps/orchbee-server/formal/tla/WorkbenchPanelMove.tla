------------------------ MODULE WorkbenchPanelMove ------------------------
EXTENDS Integers, Sequences, FiniteSets, TLC

(* A bounded refinement of panel.move over one Tab's binary Panel tree.
   A real move detaches the source leaf — collapsing its parent Split into the
   sibling subtree — and reattaches the retained leaf under a fresh Split beside
   the target leaf.  The fresh Split owns a fresh Split identity and two fresh
   Slot identities; the moved leaf keeps its complete Panel node.  A move whose
   placement is already satisfied — same Panel, or the pair already adjacent
   under a matching-orientation Split in the direction-implied order — is a
   no-op that consumes no identities and no revision.  A move that names a
   missing source or target leaf, or a stale expected revision, is rejected
   without touching the tree.

   Leaves are abstract Panels; tree nodes carry Split/Slot identities so the
   model can pin identity freshness.  TLC cannot compare records against a
   string sentinel, so partial tree operators return {present, node} records.
   At most two first-seen commands settle in the shared ledger; retries replay
   the recorded result as a stutter and never extend it.  Terminal lifecycle is
   out of scope: a move never starts or ends a terminal.  No fairness or
   progress claim is made. *)

Panels == {"a", "b", "c"}
Directions == {"above", "below", "left", "right"}
RequestLimit == 2
MaxRevision == 2

Before(d) == d \in {"above", "left"}
Orient(d) == IF d \in {"above", "below"} THEN "v" ELSE "h"

Leaf(p) == [kind |-> "leaf", panel |-> p]
SplitNode(i, o, s1, s2, l, r) ==
  [kind |-> "split", id |-> i, orient |-> o,
   slots |-> <<s1, s2>>, children |-> <<l, r>>]

RECURSIVE LeafNodes(_)
LeafNodes(t) ==
  IF t.kind = "leaf" THEN {t}
  ELSE LeafNodes(t.children[1]) \cup LeafNodes(t.children[2])

LeafRecord(t, p) == CHOOSE l \in LeafNodes(t) : l.panel = p

RECURSIVE LeafCount(_)
LeafCount(t) ==
  IF t.kind = "leaf" THEN 1
  ELSE LeafCount(t.children[1]) + LeafCount(t.children[2])

RECURSIVE LeavesOf(_)
LeavesOf(t) ==
  IF t.kind = "leaf" THEN {t.panel}
  ELSE LeavesOf(t.children[1]) \cup LeavesOf(t.children[2])

RECURSIVE SplitNodes(_)
SplitNodes(t) ==
  IF t.kind = "leaf" THEN {}
  ELSE {t} \cup SplitNodes(t.children[1]) \cup SplitNodes(t.children[2])

RECURSIVE SplitIdsOf(_)
SplitIdsOf(t) ==
  IF t.kind = "leaf" THEN {}
  ELSE {t.id} \cup SplitIdsOf(t.children[1]) \cup SplitIdsOf(t.children[2])

RECURSIVE SlotIdsOf(_)
SlotIdsOf(t) ==
  IF t.kind = "leaf" THEN {}
  ELSE {t.slots[1], t.slots[2]}
       \cup SlotIdsOf(t.children[1]) \cup SlotIdsOf(t.children[2])

Absent(t) == [present |-> FALSE, node |-> t]
Found(n) == [present |-> TRUE, node |-> n]

(* Detaches leaf p, replacing its parent Split with the sibling subtree, and
   returns the reduced tree; absent when p is missing or has no parent Split. *)
RECURSIVE Take(_, _)
Take(t, p) ==
  IF t.kind = "leaf" THEN Absent(t)
  ELSE IF t.children[1].kind = "leaf" /\ t.children[1].panel = p
       THEN Found(t.children[2])
  ELSE IF t.children[2].kind = "leaf" /\ t.children[2].panel = p
       THEN Found(t.children[1])
  ELSE LET l == Take(t.children[1], p) IN
       IF l.present
       THEN Found([t EXCEPT !.children = <<l.node, t.children[2]>>])
       ELSE LET r == Take(t.children[2], p) IN
            IF r.present
            THEN Found([t EXCEPT !.children = <<t.children[1], r.node>>])
            ELSE Absent(t)

(* Replaces the target leaf with the fresh Split graft holding the moved leaf
   and the target leaf in the direction-implied order. *)
RECURSIVE Graft(_, _, _, _, _)
Graft(t, target, moved, d, ids) ==
  IF t.kind = "leaf"
  THEN IF t.panel = target
       THEN Found(SplitNode(ids[1], Orient(d), ids[2], ids[3],
                            IF Before(d) THEN moved ELSE t,
                            IF Before(d) THEN t ELSE moved))
       ELSE Absent(t)
  ELSE LET l == Graft(t.children[1], target, moved, d, ids) IN
       IF l.present
       THEN Found([t EXCEPT !.children = <<l.node, t.children[2]>>])
       ELSE LET r == Graft(t.children[2], target, moved, d, ids) IN
            IF r.present
            THEN Found([t EXCEPT !.children = <<t.children[1], r.node>>])
            ELSE Absent(t)

(* The placement is already satisfied when the two leaves are the direct
   children of a matching-orientation Split in the direction-implied order. *)
RECURSIVE Adjacent(_, _, _, _)
Adjacent(t, p, q, d) ==
  IF t.kind = "leaf" THEN FALSE
  ELSE
    \/ /\ t.orient = Orient(d)
       /\ t.children[1].kind = "leaf" /\ t.children[2].kind = "leaf"
       /\ LET order == IF Before(d) THEN <<p, q>> ELSE <<q, p>>
          IN t.children[1].panel = order[1] /\ t.children[2].panel = order[2]
    \/ Adjacent(t.children[1], p, q, d)
    \/ Adjacent(t.children[2], p, q, d)

(* The Split that directly parents leaf p; absent when p is the root or
   missing. *)
RECURSIVE ParentSplit(_, _)
ParentSplit(t, p) ==
  IF t.kind = "leaf" THEN Absent(t)
  ELSE IF (t.children[1].kind = "leaf" /\ t.children[1].panel = p)
          \/ (t.children[2].kind = "leaf" /\ t.children[2].panel = p)
       THEN Found(t)
  ELSE LET l == ParentSplit(t.children[1], p) IN
       IF l.present THEN l ELSE ParentSplit(t.children[2], p)

(* Replaces the graft Split with the child that is not the moved leaf. *)
RECURSIVE PruneGraft(_, _, _)
PruneGraft(t, sid, p) ==
  IF t.kind = "leaf" THEN Absent(t)
  ELSE IF t.id = sid
       THEN IF t.children[1].kind = "leaf" /\ t.children[1].panel = p
            THEN Found(t.children[2]) ELSE Found(t.children[1])
  ELSE LET l == PruneGraft(t.children[1], sid, p) IN
       IF l.present
       THEN Found([t EXCEPT !.children = <<l.node, t.children[2]>>])
       ELSE LET r == PruneGraft(t.children[2], sid, p) IN
            IF r.present
            THEN Found([t EXCEPT !.children = <<t.children[1], r.node>>])
            ELSE Absent(t)

Command(moved, target, direction, base) ==
  [moved |-> moved, target |-> target, direction |-> direction, base |-> base]
Commands ==
  [moved: Panels, target: Panels, direction: Directions, base: 0..RequestLimit]
NoCommand == Command("a", "a", "left", 0)
Reply(kind, command, result) ==
  [kind |-> kind, command |-> command, result |-> result]

VARIABLES tree, revision, nextId, leafset, ledger, commits,
          prevTree, prevIds, graft, lastMove, response
vars == <<tree, revision, nextId, leafset, ledger, commits,
          prevTree, prevIds, graft, lastMove, response>>

InitialTrees == {
  SplitNode(1, "h", 11, 12, Leaf("a"), Leaf("b")),
  SplitNode(1, "h", 11, 12, Leaf("a"),
            SplitNode(2, "v", 13, 14, Leaf("b"), Leaf("c"))),
  SplitNode(1, "v", 11, 12, SplitNode(2, "h", 13, 14, Leaf("a"), Leaf("b")),
            Leaf("c"))
}

Init ==
  /\ tree \in InitialTrees
  /\ leafset = LeavesOf(tree)
  /\ revision = 0
  /\ nextId = 100
  /\ ledger = <<>>
  /\ commits = 0
  /\ prevTree = tree
  /\ prevIds = {}
  /\ graft = {}
  /\ lastMove = NoCommand
  /\ response = Reply("none", NoCommand, "none")

(* One direct action owns all panel.move dispatch; every first-seen command
   appends to the shared ledger. *)
NewMove ==
  /\ Len(ledger) < RequestLimit
  /\ \E moved \in Panels, target \in Panels, direction \in Directions,
        base \in {0, revision}:
       LET command == Command(moved, target, direction, base)
           present == moved \in LeavesOf(tree) /\ target \in LeavesOf(tree)
           noop == moved = target \/ Adjacent(tree, moved, target, direction)
           reduced == Take(tree, moved)
           ids == <<nextId, nextId + 1, nextId + 2>>
           result == IF base # revision THEN "stale"
                     ELSE IF ~present THEN "panel-unavailable"
                     ELSE IF noop \/ ~reduced.present THEN "unchanged"
                     ELSE "panel-moved"
       IN /\ prevTree' = tree
          /\ prevIds' = SplitIdsOf(tree) \cup SlotIdsOf(tree)
          /\ lastMove' = IF result = "panel-moved" THEN command ELSE NoCommand
          /\ graft' = IF result = "panel-moved" THEN {ids[1], ids[2], ids[3]}
                      ELSE {}
          /\ tree' = IF result = "panel-moved"
                     THEN Graft(reduced.node, target, LeafRecord(tree, moved),
                                direction, ids).node
                     ELSE tree
          /\ nextId' = nextId + (IF result = "panel-moved" THEN 3 ELSE 0)
          /\ revision' = revision + (IF result = "panel-moved" THEN 1 ELSE 0)
          /\ commits' = commits + (IF result = "panel-moved" THEN 1 ELSE 0)
          /\ response' = Reply("result", command, result)
          /\ ledger' = Append(ledger,
               [command |-> command, result |-> result,
                revision |-> revision'])
          /\ UNCHANGED leafset

(* A retry of a settled command replays the recorded result; the Site state
   is untouched, so the step is a pure stutter. *)
Replay ==
  /\ Len(ledger) > 0
  /\ UNCHANGED vars

(* Independent witnesses keep direction, structure, and outcome coverage
   visible without action aliases. *)
MovedRecently == response.kind = "result" /\ response.result = "panel-moved"
ObserveAbove ==
  /\ MovedRecently /\ lastMove.direction = "above" /\ UNCHANGED vars
ObserveBelow ==
  /\ MovedRecently /\ lastMove.direction = "below" /\ UNCHANGED vars
ObserveLeft ==
  /\ MovedRecently /\ lastMove.direction = "left" /\ UNCHANGED vars
ObserveRight ==
  /\ MovedRecently /\ lastMove.direction = "right" /\ UNCHANGED vars
ObserveNoop ==
  /\ response.kind = "result" /\ response.result = "unchanged" /\ UNCHANGED vars
ObserveStale ==
  /\ response.kind = "result" /\ response.result = "stale" /\ UNCHANGED vars
ObserveMissing ==
  /\ response.kind = "result" /\ response.result = "panel-unavailable"
  /\ UNCHANGED vars
ObserveNestedMove ==
  /\ MovedRecently
  /\ \E s \in SplitNodes(tree) :
       \/ s.children[1].kind = "split" /\ s.children[1].id \in graft
       \/ s.children[2].kind = "split" /\ s.children[2].id \in graft
  /\ UNCHANGED vars
ObserveFlatSwap ==
  /\ MovedRecently
  /\ tree.kind = "split"
  /\ tree.children[1].kind = "leaf" /\ tree.children[2].kind = "leaf"
  /\ UNCHANGED vars
ObserveReplay ==
  /\ Len(ledger) = RequestLimit /\ UNCHANGED vars

Next ==
  \/ NewMove
  \/ Replay
  \/ ObserveAbove
  \/ ObserveBelow
  \/ ObserveLeft
  \/ ObserveRight
  \/ ObserveNoop
  \/ ObserveStale
  \/ ObserveMissing
  \/ ObserveNestedMove
  \/ ObserveFlatSwap
  \/ ObserveReplay

Spec == Init /\ [][Next]_vars

RECURSIVE IsTree(_)
IsTree(t) ==
  IF t.kind = "leaf" THEN t.panel \in Panels
  ELSE
    /\ t.id \in Nat
    /\ t.orient \in {"h", "v"}
    /\ Len(t.slots) = 2 /\ t.slots[1] \in Nat /\ t.slots[2] \in Nat
    /\ t.slots[1] # t.slots[2]
    /\ IsTree(t.children[1]) /\ IsTree(t.children[2])

TypeOK ==
  /\ IsTree(tree)
  /\ IsTree(prevTree)
  /\ revision \in 0..MaxRevision
  /\ nextId \in Nat
  /\ leafset \in SUBSET Panels
  /\ prevIds \subseteq Nat
  /\ graft \subseteq Nat
  /\ lastMove \in Commands
  /\ ledger \in Seq([command: Commands,
                     result: {"stale", "panel-unavailable", "panel-moved",
                              "unchanged"},
                     revision: 0..MaxRevision])
  /\ commits \in 0..MaxRevision
  /\ response.kind \in {"none", "result"}

(* Every tree is a full binary tree over exactly the initial leaf set, with
   globally unique Panel, Split, and Slot identities. *)
TreeIntegrity ==
  /\ LeavesOf(tree) = leafset
  /\ LeafCount(tree) = Cardinality(leafset)
  /\ Cardinality(SplitNodes(tree)) = Cardinality(SplitIdsOf(tree))
  /\ Cardinality(SlotIdsOf(tree)) = 2 * Cardinality(SplitNodes(tree))
  /\ Cardinality(SplitNodes(tree)) = Cardinality(leafset) - 1

(* Identity allocation is monotone; every live identity sits below the cursor. *)
IdentitiesBelowCursor ==
  \A i \in SplitIdsOf(tree) \cup SlotIdsOf(tree) : i < nextId

(* The remaining obligations only fire on steps that committed a real move. *)
MoveCommitted ==
  response.kind = "result" /\ response.result = "panel-moved"

(* A committed move grafts exactly one fresh Split: its identities were absent
   from the pre-move tree, and it directly parents the moved leaf beside the
   target leaf in the direction-implied order. *)
MoveGraftExact ==
  MoveCommitted =>
    /\ graft \cap prevIds = {}
    /\ LET parent == ParentSplit(tree, lastMove.moved)
       IN /\ parent.present
          /\ parent.node.id \in graft
          /\ parent.node.slots[1] \in graft /\ parent.node.slots[2] \in graft
          /\ parent.node.orient = Orient(lastMove.direction)
          /\ parent.node.children[1].kind = "leaf"
          /\ parent.node.children[2].kind = "leaf"
          /\ IF Before(lastMove.direction)
             THEN parent.node.children[1].panel = lastMove.moved
                  /\ parent.node.children[2].panel = lastMove.target
             ELSE parent.node.children[1].panel = lastMove.target
                  /\ parent.node.children[2].panel = lastMove.moved

(* The moved leaf node is retained whole. *)
MovedLeafRetained ==
  MoveCommitted =>
    LeafRecord(tree, lastMove.moved) = LeafRecord(prevTree, lastMove.moved)

(* The vacated parent Split collapsed: its identity is gone from the tree. *)
CollapsedParentGone ==
  /\ MoveCommitted
  /\ ParentSplit(prevTree, lastMove.moved).present
  => ParentSplit(prevTree, lastMove.moved).node.id \notin SplitIdsOf(tree)

(* Everything outside the take+graft pair is untouched: reducing both trees
   yields the same structure — the equivalence the Client's projection checks. *)
ReducedTreesEqual ==
  /\ MoveCommitted
  /\ ParentSplit(prevTree, lastMove.moved).present
  => LET sid == CHOOSE i \in graft : i \in SplitIdsOf(tree)
     IN PruneGraft(tree, sid, lastMove.moved).node
        = Take(prevTree, lastMove.moved).node

(* Rejected and no-op commands never touch the tree or identities. *)
RejectedKeepsState ==
  /\ response.kind = "result"
  /\ response.result \in {"stale", "panel-unavailable", "unchanged"}
  => tree = prevTree

(* The ledger records each settled command once; a committed move always
   advances the revision exactly once above its admitted base. *)
LedgerExact ==
  /\ Len(ledger) <= RequestLimit
  /\ \A i \in 1..Len(ledger) :
       ledger[i].result = "panel-moved" =>
         ledger[i].revision = ledger[i].command.base + 1

RevisionOnlyFromMoves == revision = commits

=============================================================================
