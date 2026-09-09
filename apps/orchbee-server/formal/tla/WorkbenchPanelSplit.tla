------------------------ MODULE WorkbenchPanelSplit ------------------------
EXTENDS Integers, Sequences, FiniteSets, TLC

(*
   A bounded refinement of one Panel Split followed by aggregate Tab Close.
   The split commits the source and new leaf together before the terminal
   outcome is known.  Close freezes that exact ordered membership and can
   publish only after every captured leaf is disposed and locally cleaned.
   Unknown is a quarantine for the affected close; it never becomes a
   successful publication.
*)
CONSTANTS Source, Peer, None, MaxRevision
ASSUME /\ Cardinality({Source, Peer, None}) = 3
       /\ MaxRevision = 2

Leafs == {Source, Peer}
EmptyClose == [phase |-> "idle", frozen |-> <<>>, disposed |-> {}, cleaned |-> {}]

VARIABLE s
vars == <<s>>

Init == s =
  [members |-> <<Source>>, revision |-> 0,
   splitPhase |-> "absent", close |-> EmptyClose,
   flight |-> None, retired |-> FALSE]

AdmitSplit ==
  /\ ~s.retired
  /\ s.splitPhase = "absent"
  /\ s.members = <<Source>>
  /\ s.revision + 1 <= MaxRevision
  /\ s' = [s EXCEPT !.members = <<Source, Peer>>,
       !.revision = @ + 1, !.splitPhase = "pending"]

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
       !.phase = "closing", !.frozen = s.members]]

SubmitDispose(target) ==
  /\ target \in Leafs
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
  /\ s.close.frozen = <<Source, Peer>>
  /\ s.close.disposed = Leafs
  /\ s.close.cleaned = Leafs
  /\ s.flight = None
  /\ s' = [s EXCEPT !.members = <<>>, !.revision = @ + 1,
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
  /\ UNCHANGED vars

Next ==
  \/ AdmitSplit
  \/ CompleteSplitStarted
  \/ CompleteSplitUnknown
  \/ AdmitClose
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

SplitMembershipExact ==
  s.members =
    IF s.close.phase = "closed" THEN <<>>
    ELSE IF s.splitPhase = "absent" THEN <<Source>>
    ELSE <<Source, Peer>>

CloseCapturesExact ==
  s.close.phase = "idle" \/ s.close.frozen = <<Source, Peer>>

ClosedRequiresProof ==
  s.close.phase # "closed"
  \/ /\ s.members = <<>>
     /\ s.close.disposed = Leafs
     /\ s.close.cleaned = Leafs
     /\ s.revision = 2

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
