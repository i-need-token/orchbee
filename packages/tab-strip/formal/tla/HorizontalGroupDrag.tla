----------------------- MODULE HorizontalGroupDrag -----------------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

CONSTANTS
  PinnedTab,
  ResearchTab1,
  ResearchTab2,
  BuildTab1,
  BuildTab2,
  LooseTab,
  ResearchGroup,
  BuildGroup,
  PinnedNode,
  ResearchNode,
  BuildNode,
  LooseNode

Range(sequence) == {sequence[index] : index \in DOMAIN sequence}

Tabs == {
  PinnedTab,
  ResearchTab1,
  ResearchTab2,
  BuildTab1,
  BuildTab2,
  LooseTab
}

Groups == {ResearchGroup, BuildGroup}
PinnedTabs == {PinnedTab}
SourceMembers == {ResearchTab1, ResearchTab2}
BuildMembers == {BuildTab1, BuildTab2}
InitialOrder == <<PinnedNode, ResearchNode, BuildNode, LooseNode>>
Collections == Range(InitialOrder)

VisibleSurfaces(collapsedGroups) ==
  {PinnedTab, LooseTab}
    \cup (IF ResearchGroup \in collapsedGroups THEN {ResearchNode} ELSE SourceMembers)
    \cup (IF BuildGroup \in collapsedGroups THEN {BuildNode} ELSE BuildMembers)

ActiveSurface(tab, collapsedGroups) ==
  IF tab \in SourceMembers /\ ResearchGroup \in collapsedGroups
    THEN ResearchNode
    ELSE IF tab \in BuildMembers /\ BuildGroup \in collapsedGroups
      THEN BuildNode
      ELSE tab

Phases == {"idle", "armed", "dragging", "settling"}
Visuals == {"none", "expanded-group", "collapsed-group"}
Outcomes == {"none", "cancelled", "committed", "settled"}

VARIABLES
  order,
  collapsed,
  active,
  selected,
  phase,
  recordedActive,
  wasCollapsed,
  originalOrder,
  originalCollapsed,
  originalActive,
  originalSelected,
  visual,
  targetValid,
  dropVisible,
  lastOutcome

vars == <<
  order,
  collapsed,
  active,
  selected,
  phase,
  recordedActive,
  wasCollapsed,
  originalOrder,
  originalCollapsed,
  originalActive,
  originalSelected,
  visual,
  targetValid,
  dropVisible,
  lastOutcome
>>

Authoritative == <<order, collapsed, active, selected>>
OriginalAuthoritative == <<
  originalOrder,
  originalCollapsed,
  originalActive,
  originalSelected
>>

IndexOf(sequence, value) ==
  CHOOSE index \in 1..Len(sequence): sequence[index] = value

Swap(sequence, leftIndex, rightIndex) ==
  [index \in 1..Len(sequence) |->
    IF index = leftIndex
      THEN sequence[rightIndex]
      ELSE IF index = rightIndex
        THEN sequence[leftIndex]
        ELSE sequence[index]]

Init ==
  \E initialCollapsed \in SUBSET Groups,
     initialActive \in Tabs:
    /\ order = InitialOrder
    /\ collapsed = initialCollapsed
    /\ active = initialActive
    /\ selected = {initialActive}
    /\ phase = "idle"
    /\ recordedActive = initialActive
    /\ wasCollapsed = FALSE
    /\ originalOrder = InitialOrder
    /\ originalCollapsed = initialCollapsed
    /\ originalActive = initialActive
    /\ originalSelected = {initialActive}
    /\ visual = "none"
    /\ targetValid = FALSE
    /\ dropVisible = FALSE
    /\ lastOutcome = "none"

ArmGroup ==
  /\ phase = "idle"
  /\ originalOrder' = order
  /\ originalCollapsed' = collapsed
  /\ originalActive' = active
  /\ originalSelected' = selected
  /\ recordedActive' = IF active \in SourceMembers THEN active ELSE ResearchTab1
  /\ wasCollapsed' = (ResearchGroup \in collapsed)
  /\ visual' = IF ResearchGroup \in collapsed
      THEN "collapsed-group"
      ELSE "expanded-group"
  /\ phase' = "armed"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ lastOutcome' = "none"
  /\ UNCHANGED Authoritative

ActivateDrag ==
  /\ phase = "armed"
  /\ collapsed' = collapsed \cup {ResearchGroup}
  /\ active' = recordedActive
  /\ selected' = SourceMembers
  /\ phase' = "dragging"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ UNCHANGED <<
      order,
      recordedActive,
      wasCollapsed,
      originalOrder,
      originalCollapsed,
      originalActive,
      originalSelected,
      visual,
      lastOutcome
    >>

MoveLeft ==
  LET sourceIndex == IndexOf(order, ResearchNode) IN
    /\ phase = "dragging"
    /\ sourceIndex > 2
    /\ order' = Swap(order, sourceIndex, sourceIndex - 1)
    /\ targetValid' = TRUE
    /\ dropVisible' = TRUE
    /\ UNCHANGED <<
        collapsed,
        active,
        selected,
        phase,
        recordedActive,
        wasCollapsed,
        originalOrder,
        originalCollapsed,
        originalActive,
        originalSelected,
        visual,
        lastOutcome
      >>

MoveRight ==
  LET sourceIndex == IndexOf(order, ResearchNode) IN
    /\ phase = "dragging"
    /\ sourceIndex < Len(order)
    /\ order' = Swap(order, sourceIndex, sourceIndex + 1)
    /\ targetValid' = TRUE
    /\ dropVisible' = TRUE
    /\ UNCHANGED <<
        collapsed,
        active,
        selected,
        phase,
        recordedActive,
        wasCollapsed,
        originalOrder,
        originalCollapsed,
        originalActive,
        originalSelected,
        visual,
        lastOutcome
      >>

RejectInvalidTarget ==
  /\ phase = "dragging"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ UNCHANGED <<
      Authoritative,
      phase,
      recordedActive,
      wasCollapsed,
      originalOrder,
      originalCollapsed,
      originalActive,
      originalSelected,
      visual,
      lastOutcome
    >>

CommitDrag ==
  /\ phase = "dragging"
  /\ collapsed' = originalCollapsed
  /\ active' = recordedActive
  /\ selected' = {recordedActive}
  /\ phase' = "settling"
  /\ visual' = "none"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ lastOutcome' = "committed"
  /\ UNCHANGED <<
      order,
      recordedActive,
      wasCollapsed,
      originalOrder,
      originalCollapsed,
      originalActive,
      originalSelected
    >>

CancelDrag ==
  /\ phase \in {"armed", "dragging"}
  /\ order' = originalOrder
  /\ collapsed' = originalCollapsed
  /\ active' = originalActive
  /\ selected' = originalSelected
  /\ phase' = "idle"
  /\ visual' = "none"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ lastOutcome' = "cancelled"
  /\ UNCHANGED <<
      recordedActive,
      wasCollapsed,
      originalOrder,
      originalCollapsed,
      originalActive,
      originalSelected
    >>

SettleDrag ==
  /\ phase = "settling"
  /\ phase' = "idle"
  /\ lastOutcome' = "settled"
  /\ UNCHANGED <<
      Authoritative,
      recordedActive,
      wasCollapsed,
      originalOrder,
      originalCollapsed,
      originalActive,
      originalSelected,
      visual,
      targetValid,
      dropVisible
    >>

ToggleResearchGroup ==
  /\ phase = "idle"
  /\ collapsed' = IF ResearchGroup \in collapsed
      THEN collapsed \ {ResearchGroup}
      ELSE collapsed \cup {ResearchGroup}
  /\ selected' = {active}
  /\ lastOutcome' = "none"
  /\ UNCHANGED <<
      order,
      active,
      phase,
      recordedActive,
      wasCollapsed,
      originalOrder,
      originalCollapsed,
      originalActive,
      originalSelected,
      visual,
      targetValid,
      dropVisible
    >>

ToggleBuildGroup ==
  /\ phase = "idle"
  /\ collapsed' = IF BuildGroup \in collapsed
      THEN collapsed \ {BuildGroup}
      ELSE collapsed \cup {BuildGroup}
  /\ selected' = {active}
  /\ lastOutcome' = "none"
  /\ UNCHANGED <<
      order,
      active,
      phase,
      recordedActive,
      wasCollapsed,
      originalOrder,
      originalCollapsed,
      originalActive,
      originalSelected,
      visual,
      targetValid,
      dropVisible
    >>

ActivateOrCancel == ActivateDrag \/ CancelDrag
FinishDrag == CommitDrag \/ CancelDrag

Next ==
  \/ ArmGroup
  \/ ActivateDrag
  \/ MoveLeft
  \/ MoveRight
  \/ RejectInvalidTarget
  \/ CommitDrag
  \/ CancelDrag
  \/ SettleDrag
  \/ ToggleResearchGroup
  \/ ToggleBuildGroup

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(ActivateOrCancel)
  /\ WF_vars(FinishDrag)
  /\ WF_vars(SettleDrag)

TypeOK ==
  /\ order \in [1..Len(InitialOrder) -> Collections]
  /\ collapsed \in SUBSET Groups
  /\ active \in Tabs
  /\ selected \in SUBSET Tabs
  /\ phase \in Phases
  /\ recordedActive \in Tabs
  /\ wasCollapsed \in BOOLEAN
  /\ originalOrder \in [1..Len(InitialOrder) -> Collections]
  /\ originalCollapsed \in SUBSET Groups
  /\ originalActive \in Tabs
  /\ originalSelected \in SUBSET Tabs
  /\ visual \in Visuals
  /\ targetValid \in BOOLEAN
  /\ dropVisible \in BOOLEAN
  /\ lastOutcome \in Outcomes

UniqueCollectionIdentity ==
  /\ Len(order) = Len(InitialOrder)
  /\ Cardinality(Range(order)) = Len(order)
  /\ Range(order) = Collections

PinnedPrefix == order[1] = PinnedNode
ActiveTabExists == active \in Tabs
SelectionExists == selected # {} /\ selected \subseteq Tabs
DragSourceExists == SourceMembers \subseteq Tabs

StableActiveSurfaceExists ==
  phase = "dragging"
    \/ ActiveSurface(active, collapsed) \in VisibleSurfaces(collapsed)

GroupMoveAtomic ==
  /\ ResearchNode \in Range(order)
  /\ Cardinality({index \in DOMAIN order: order[index] = ResearchNode}) = 1

GroupTargetBoundary ==
  phase # "dragging" \/ IndexOf(order, ResearchNode) \in 2..Len(order)

CollapsedDragStaysCompact ==
  ~(phase \in {"armed", "dragging"} /\ wasCollapsed)
    \/ visual = "collapsed-group"

ExpandedDragKeepsFullVisual ==
  ~(phase \in {"armed", "dragging"} /\ ~wasCollapsed)
    \/ visual = "expanded-group"

DraggingBackingGroupIsCollapsed ==
  phase # "dragging" \/ ResearchGroup \in collapsed

CompletionRestoresCollapsedIntent ==
  ~(lastOutcome \in {"committed", "settled"})
    \/ collapsed = originalCollapsed

CompletionSelectsRecordedActive ==
  ~(lastOutcome \in {"committed", "settled"})
    \/ (active = recordedActive /\ selected = {recordedActive})

CollapsedActiveUsesGroupHeader ==
  /\ ~(ResearchGroup \in collapsed /\ active \in SourceMembers)
      \/ ActiveSurface(active, collapsed) = ResearchNode
  /\ ~(BuildGroup \in collapsed /\ active \in BuildMembers)
      \/ ActiveSurface(active, collapsed) = BuildNode

CancelRestoresAuthoritativeState ==
  lastOutcome # "cancelled" \/ Authoritative = OriginalAuthoritative

NoInvalidDropFeedback == targetValid \/ ~dropVisible

InteractionEventuallyIdle ==
  phase \in {"armed", "dragging", "settling"} ~> phase = "idle"

=============================================================================
