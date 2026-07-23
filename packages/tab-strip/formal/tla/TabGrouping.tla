--------------------------- MODULE TabGrouping ---------------------------
EXTENDS FiniteSets, Naturals, Sequences

CONSTANTS
  Tab1,
  Tab2,
  Tab3,
  Tab4,
  NoGroup,
  NewGroup

Tabs == {Tab1, Tab2, Tab3, Tab4}
GroupStates == {NoGroup, NewGroup}
InitialOrder == <<Tab1, Tab2, Tab3, Tab4>>

ContiguousSelections == {
  {Tab1},
  {Tab2},
  {Tab3},
  {Tab4},
  {Tab1, Tab2},
  {Tab2, Tab3},
  {Tab3, Tab4},
  {Tab1, Tab2, Tab3},
  {Tab2, Tab3, Tab4},
  Tabs
}

Range(sequence) == {sequence[index] : index \in DOMAIN sequence}

VARIABLES
  order,
  membership,
  selected,
  phase,
  originalOrder

vars == <<order, membership, selected, phase, originalOrder>>

Init ==
  /\ order = InitialOrder
  /\ membership = [tab \in Tabs |-> NoGroup]
  /\ selected = {}
  /\ phase = "idle"
  /\ originalOrder = InitialOrder

CreateGroup ==
  /\ phase = "idle"
  /\ \E tabsToGroup \in ContiguousSelections:
    /\ selected' = tabsToGroup
    /\ membership' = [tab \in Tabs |->
      IF tab \in tabsToGroup THEN NewGroup ELSE NoGroup]
  /\ phase' = "grouped"
  /\ originalOrder' = order
  /\ order' = order

ClearGroup ==
  /\ phase = "grouped"
  /\ membership' = [tab \in Tabs |-> NoGroup]
  /\ selected' = {}
  /\ phase' = "idle"
  /\ originalOrder' = order
  /\ order' = order

Next == CreateGroup \/ ClearGroup

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeOK ==
  /\ order \in [1..Len(InitialOrder) -> Tabs]
  /\ membership \in [Tabs -> GroupStates]
  /\ selected \in SUBSET Tabs
  /\ phase \in {"idle", "grouped"}
  /\ originalOrder \in [1..Len(InitialOrder) -> Tabs]

UniqueTabIdentity ==
  /\ Len(order) = Len(InitialOrder)
  /\ Cardinality(Range(order)) = Len(order)
  /\ Range(order) = Tabs

GroupedSelectionIsContiguous ==
  phase # "grouped" \/ selected \in ContiguousSelections

GroupedSelectionMatchesMembership ==
  phase # "grouped" \/ selected = {tab \in Tabs: membership[tab] = NewGroup}

GroupingPreservesOriginalOrder ==
  phase # "grouped" \/ order = originalOrder

TabOrderNeverChanges == order = InitialOrder

=============================================================================
