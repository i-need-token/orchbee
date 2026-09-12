------------------------ MODULE WorkbenchPanelSplit ------------------------
EXTENDS Integers, Sequences, FiniteSets, TLC

(*
   A bounded refinement of one Panel Split followed by Close lifecycles.
   The split commits the source and new leaf together before the terminal
   outcome is known, so `members` is the strict leaf membership a standalone
   Open reads: a committed pending split leaf is already a member, and neither
   it nor a legal reorder changes the slot anchor.  A Tab-scoped close freezes
   the exact ordered membership; a Panel-scoped close freezes one leaf.  A
   close can publish only after every captured leaf is disposed and locally
   cleaned.  A leaf publication contracts the membership to the surviving
   siblings; a tab publication removes every member.  Unknown is a quarantine
   for the affected close; it never becomes a successful publication.
*)
CONSTANTS Source, Peer, None, MaxRevision
ASSUME /\ Cardinality({Source, Peer, None}) = 3
       /\ MaxRevision = 2

Leafs == {Source, Peer}
EmptyClose == [phase |-> "idle", frozen |-> <<>>, disposed |-> {}, cleaned |-> {},
               aggregate |-> FALSE]

VARIABLE s
vars == <<s>>

MemberSet(seq) == {seq[i] : i \in DOMAIN seq}
RemoveMembers(seq, dropped) == SelectSeq(seq, LAMBDA m : m \notin dropped)

(*
   An open aggregate Close fences the same Tab's structural admission: its
   publication re-proves the complete captured ordered leaf membership, which a
   same-Tab split or move would invalidate.  A leaf-scoped Close re-proves only
   its own captured leaf tuple and fences neither operation.
*)
PendingAggregateClose ==
  s.close.phase \in {"closing", "unknown"} /\ s.close.aggregate

Init == s =
  [members |-> <<Source>>, revision |-> 0,
   splitPhase |-> "absent", close |-> EmptyClose,
   flight |-> None, retired |-> FALSE]

AdmitSplit ==
  /\ ~s.retired
  /\ s.splitPhase = "absent"
  /\ s.members = <<Source>>
  /\ ~PendingAggregateClose
  /\ s.revision + 1 <= MaxRevision
  /\ s' = [s EXCEPT !.members = <<Source, Peer>>,
       !.revision = @ + 1, !.splitPhase = "pending"]

AdmitMove ==
  /\ ~s.retired
  /\ Len(s.members) = 2
  /\ ~PendingAggregateClose
  /\ s.revision + 1 <= MaxRevision
  /\ s' = [s EXCEPT !.members = <<s.members[2], s.members[1]>>,
       !.revision = @ + 1]

CompleteSplitStarted ==
  /\ s.splitPhase = "pending"
  /\ s' = [s EXCEPT !.splitPhase = "started"]

CompleteSplitUnknown ==
  /\ s.splitPhase = "pending"
  /\ s' = [s EXCEPT !.splitPhase = "unknown"]

AdmitClose ==
  /\ s.splitPhase = "started"
  /\ s.close.phase = "idle"
  /\ s' = [s EXCEPT !.close = [EmptyClose EXCEPT
       !.phase = "closing", !.frozen = s.members, !.aggregate = TRUE]]

AdmitPanelClose(target) ==
  /\ ~s.retired
  /\ target \in MemberSet(s.members)
  /\ s.close.phase = "idle"
  /\ s' = [s EXCEPT !.close = [EmptyClose EXCEPT
       !.phase = "closing", !.frozen = <<target>>, !.aggregate = FALSE]]

SubmitDispose(target) ==
  /\ target \in MemberSet(s.close.frozen)
  /\ s.close.phase = "closing"
  /\ target \notin s.close.disposed
  /\ s.flight = None
  /\ s' = [s EXCEPT !.flight = target]

CompleteDispose ==
  /\ s.flight \in Leafs
  /\ s' = [s EXCEPT !.close.disposed = @ \cup {s.flight}, !.flight = None]

CompleteCloseUnknown ==
  /\ s.flight \in Leafs
  /\ s.close.phase = "closing"
  /\ s' = [s EXCEPT !.close.phase = "unknown", !.flight = None]

Cleanup(target) ==
  /\ target \in s.close.disposed
  /\ target \notin s.close.cleaned
  /\ s' = [s EXCEPT !.close.cleaned = @ \cup {target}]

Publish ==
  /\ s.close.phase = "closing"
  /\ s.flight = None
  /\ s.revision + 1 <= MaxRevision
  /\ MemberSet(s.close.frozen) \subseteq s.close.disposed
  /\ MemberSet(s.close.frozen) \subseteq s.close.cleaned
  /\ LET remaining == RemoveMembers(s.members, MemberSet(s.close.frozen))
     IN s' = [s EXCEPT !.members = remaining, !.revision = @ + 1,
           !.splitPhase = IF Len(remaining) = 1 THEN "absent" ELSE @,
           !.close.phase = "closed"]

Retire ==
  /\ ~s.retired
  /\ s' = [s EXCEPT !.retired = TRUE]

ObserveSplitPending ==
  /\ s.splitPhase = "pending"
  /\ UNCHANGED vars

ObserveSplitUnknown ==
  /\ s.splitPhase = "unknown"
  /\ UNCHANGED vars

ObserveTwoLeafSplit ==
  /\ s.members = <<Source, Peer>>
  /\ s.splitPhase = "started"
  /\ UNCHANGED vars

ObserveAggregateClose ==
  /\ s.close.phase = "closed"
  /\ s.members = <<>>
  /\ UNCHANGED vars

ObserveLeafContraction ==
  /\ s.close.phase = "closed"
  /\ Len(s.members) = 1
  /\ UNCHANGED vars

ObserveMovedMembers ==
  /\ s.members = <<Peer, Source>>
  /\ UNCHANGED vars

ObserveMoveDuringLeafClose ==
  /\ s.close.phase = "closing"
  /\ ~s.close.aggregate
  /\ s.members = <<Peer, Source>>
  /\ UNCHANGED vars

Next ==
  \/ AdmitSplit
  \/ AdmitMove
  \/ CompleteSplitStarted
  \/ CompleteSplitUnknown
  \/ AdmitClose
  \/ \E target \in Leafs: AdmitPanelClose(target)
  \/ \E target \in Leafs: SubmitDispose(target)
  \/ CompleteDispose
  \/ CompleteCloseUnknown
  \/ \E target \in Leafs: Cleanup(target)
  \/ Publish
  \/ Retire
  \/ ObserveSplitPending
  \/ ObserveSplitUnknown
  \/ ObserveTwoLeafSplit
  \/ ObserveAggregateClose
  \/ ObserveLeafContraction
  \/ ObserveMovedMembers
  \/ ObserveMoveDuringLeafClose

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.members \in Seq(Leafs)
  /\ s.revision \in 0..MaxRevision
  /\ s.splitPhase \in {"absent", "pending", "started", "unknown"}
  /\ s.close.phase \in {"idle", "closing", "unknown", "closed"}
  /\ s.close.frozen \in Seq(Leafs)
  /\ s.close.disposed \subseteq Leafs
  /\ s.close.cleaned \subseteq Leafs
  /\ s.flight \in Leafs \cup {None}
  /\ s.retired \in BOOLEAN
  /\ s.close.aggregate \in BOOLEAN

SplitMembershipExact ==
  IF s.splitPhase = "absent"
  THEN s.members \in {<<Source>>, <<Peer>>, <<>>}
  ELSE IF s.close.phase = "closed" THEN s.members = <<>>
       ELSE s.members \in {<<Source, Peer>>, <<Peer, Source>>}

CloseCapturesExact ==
  s.close.phase = "idle" \/
    /\ s.close.frozen \in {<<Source>>, <<Peer>>, <<Source, Peer>>, <<Peer, Source>>}
    /\ MemberSet(s.close.frozen) \subseteq MemberSet(
         IF s.splitPhase = "absent" /\ s.close.phase # "closed"
         THEN <<Source>> ELSE <<Source, Peer>>)

AggregateCloseStable ==
  s.close.phase \in {"closing", "unknown"} /\ s.close.aggregate =>
    s.members = s.close.frozen

FrozenRemainUntilPublished ==
  s.close.phase \in {"closing", "unknown"} =>
    MemberSet(s.close.frozen) \subseteq MemberSet(s.members)

ClosedRequiresProof ==
  s.close.phase # "closed" \/
    /\ MemberSet(s.close.frozen) \subseteq s.close.disposed
    /\ MemberSet(s.close.frozen) \subseteq s.close.cleaned
    /\ MemberSet(s.close.frozen) \cap MemberSet(s.members) = {}
    /\ s.members = <<>> \/ Len(s.members) + Len(s.close.frozen) = 2
    /\ s.revision >= 1

UnknownNeverPublishes ==
  s.close.phase = "closed" => s.splitPhase # "unknown"

CleanupRequiresDispose ==
  s.close.cleaned \subseteq s.close.disposed

SingleOutstanding ==
  s.flight = None \/ s.flight \in Leafs

KnownEffectsMonotone == [][
  /\ s.close.disposed \subseteq s'.close.disposed
  /\ s.close.cleaned \subseteq s'.close.cleaned
  /\ (s.close.phase = "closed" => s'.close.phase = "closed")
]_vars

=============================================================================
