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
  BuildGroup

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
NoGroup == "no-group"
UnknownGroup == "unknown-group"
GroupValues == Groups \cup {NoGroup, UnknownGroup}
PinnedTabs == {PinnedTab}
SourceMembers == {ResearchTab1, ResearchTab2}
Outsiders == Tabs \ SourceMembers

InitialTabs ==
  <<PinnedTab, ResearchTab1, ResearchTab2, BuildTab1, BuildTab2, LooseTab>>

AfterBuildTabs ==
  <<PinnedTab, BuildTab1, BuildTab2, ResearchTab1, ResearchTab2, LooseTab>>

AfterLooseTabs ==
  <<PinnedTab, BuildTab1, BuildTab2, LooseTab, ResearchTab1, ResearchTab2>>

InitialGroupOf ==
  [tab \in Tabs |->
    IF tab \in SourceMembers
    THEN ResearchGroup
    ELSE IF tab \in {BuildTab1, BuildTab2}
      THEN BuildGroup
      ELSE NoGroup]

Phases == {"idle", "armed", "dragging", "settling"}
Visuals == {"none", "expanded-group", "collapsed-group"}
Outcomes == {"none", "cancelled", "committed", "settled"}
Commands == {"none", "activate", "move", "commit", "cancel", "toggle"}

Actions == {
  "init",
  "arm-group",
  "activate-drag",
  "move-left",
  "move-right",
  "invalidate-source-membership",
  "reject-stale-activation",
  "reject-missing-group",
  "reject-out-of-phase-event",
  "reject-repeated-event",
  "reject-invalid-target",
  "commit-drag",
  "cancel-drag",
  "settle-drag",
  "toggle-research-group",
  "toggle-build-group"
}

PassiveRejectedActions == {
  "reject-missing-group",
  "reject-out-of-phase-event",
  "reject-repeated-event"
}

VARIABLES
  tabs,
  groupOf,
  collapsed,
  active,
  selected,
  selectionAnchor,
  phase,
  capturedMembers,
  recordedActive,
  originalTabs,
  originalGroupOf,
  originalCollapsed,
  originalActive,
  originalSelected,
  originalSelectionAnchor,
  visual,
  targetValid,
  dropVisible,
  lastOutcome,
  previousAuthoritative,
  previousInteraction,
  lastAction,
  lastCommand

Authoritative ==
  <<tabs, groupOf, collapsed, active, selected, selectionAnchor>>

OriginalAuthoritative ==
  <<originalTabs, originalGroupOf, originalCollapsed, originalActive,
    originalSelected, originalSelectionAnchor>>

Interaction == <<phase, capturedMembers, recordedActive, visual,
  targetValid, dropVisible, lastOutcome>>

TabOrderType == [1..Len(InitialTabs) -> Tabs]
GroupMapType == [Tabs -> GroupValues]
AuthoritativeDomain ==
  TabOrderType
    \X GroupMapType
    \X (SUBSET Groups)
    \X Tabs
    \X (SUBSET Tabs)
    \X Tabs

vars == <<
  tabs,
  groupOf,
  collapsed,
  active,
  selected,
  selectionAnchor,
  phase,
  capturedMembers,
  recordedActive,
  originalTabs,
  originalGroupOf,
  originalCollapsed,
  originalActive,
  originalSelected,
  originalSelectionAnchor,
  visual,
  targetValid,
  dropVisible,
  lastOutcome,
  previousAuthoritative,
  previousInteraction,
  lastAction,
  lastCommand
>>

IndexOf(sequence, value) ==
  CHOOSE index \in DOMAIN sequence: sequence[index] = value

GroupContiguousFor(sequence, membership, group) ==
  \A left \in DOMAIN sequence, right \in DOMAIN sequence:
    /\ left < right
    /\ membership[sequence[left]] = group
    /\ membership[sequence[right]] = group
    =>
      \A between \in left..right:
        membership[sequence[between]] = group

SourceBlockIsExact(sequence, membership) ==
  /\ ResearchTab1 \in Range(sequence)
  /\ ResearchTab2 \in Range(sequence)
  /\ membership[ResearchTab1] = ResearchGroup
  /\ membership[ResearchTab2] = ResearchGroup
  /\ IndexOf(sequence, ResearchTab2) =
       IndexOf(sequence, ResearchTab1) + 1

CapturedSourceIsCurrent ==
  /\ capturedMembers = <<ResearchTab1, ResearchTab2>>
  /\ SourceBlockIsExact(tabs, groupOf)

OutsiderOrderUnchanged(before, after) ==
  \A first \in Outsiders, second \in Outsiders:
    (IndexOf(before, first) < IndexOf(before, second)) =
      (IndexOf(after, first) < IndexOf(after, second))

VisibleSurfaces ==
  {tab \in Tabs: groupOf[tab] \notin collapsed} \cup collapsed

ActiveSurface ==
  IF groupOf[active] \in collapsed
  THEN groupOf[active]
  ELSE active

Record(action, command) ==
  /\ previousAuthoritative' = Authoritative
  /\ previousInteraction' = Interaction
  /\ lastAction' = action
  /\ lastCommand' = command

Init ==
  \E initialCollapsed \in SUBSET Groups,
     initialActive \in Tabs:
    /\ tabs = InitialTabs
    /\ groupOf = InitialGroupOf
    /\ collapsed = initialCollapsed
    /\ active = initialActive
    /\ selected = {initialActive}
    /\ selectionAnchor = initialActive
    /\ phase = "idle"
    /\ capturedMembers = <<ResearchTab1, ResearchTab2>>
    /\ recordedActive = initialActive
    /\ originalTabs = InitialTabs
    /\ originalGroupOf = InitialGroupOf
    /\ originalCollapsed = initialCollapsed
    /\ originalActive = initialActive
    /\ originalSelected = {initialActive}
    /\ originalSelectionAnchor = initialActive
    /\ visual = "none"
    /\ targetValid = FALSE
    /\ dropVisible = FALSE
    /\ lastOutcome = "none"
    /\ previousAuthoritative = Authoritative
    /\ previousInteraction = Interaction
    /\ lastAction = "init"
    /\ lastCommand = "none"

ArmGroup ==
  /\ phase = "idle"
  /\ SourceBlockIsExact(tabs, groupOf)
  /\ originalTabs' = tabs
  /\ originalGroupOf' = groupOf
  /\ originalCollapsed' = collapsed
  /\ originalActive' = active
  /\ originalSelected' = selected
  /\ originalSelectionAnchor' = selectionAnchor
  /\ capturedMembers' = <<ResearchTab1, ResearchTab2>>
  /\ recordedActive' =
       IF active \in SourceMembers THEN active ELSE ResearchTab1
  /\ visual' =
       IF ResearchGroup \in collapsed
       THEN "collapsed-group"
       ELSE "expanded-group"
  /\ phase' = "armed"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ lastOutcome' = "none"
  /\ UNCHANGED Authoritative
  /\ Record("arm-group", "none")

ActivateDrag ==
  /\ phase = "armed"
  /\ CapturedSourceIsCurrent
  /\ collapsed' = collapsed \cup {ResearchGroup}
  /\ active' = recordedActive
  /\ selected' = SourceMembers
  /\ selectionAnchor' = recordedActive
  /\ phase' = "dragging"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ UNCHANGED <<tabs, groupOf, capturedMembers, recordedActive,
       originalTabs, originalGroupOf, originalCollapsed, originalActive,
       originalSelected, originalSelectionAnchor, visual, lastOutcome>>
  /\ Record("activate-drag", "activate")

MoveLeft ==
  /\ phase = "dragging"
  /\ CapturedSourceIsCurrent
  /\ tabs \in {AfterBuildTabs, AfterLooseTabs}
  /\ tabs' =
       IF tabs = AfterLooseTabs THEN AfterBuildTabs ELSE InitialTabs
  /\ targetValid' = TRUE
  /\ dropVisible' = TRUE
  /\ UNCHANGED <<groupOf, collapsed, active, selected, selectionAnchor,
       phase, capturedMembers, recordedActive, originalTabs,
       originalGroupOf, originalCollapsed, originalActive, originalSelected,
       originalSelectionAnchor, visual, lastOutcome>>
  /\ Record("move-left", "move")

MoveRight ==
  /\ phase = "dragging"
  /\ CapturedSourceIsCurrent
  /\ tabs \in {InitialTabs, AfterBuildTabs}
  /\ tabs' =
       IF tabs = InitialTabs THEN AfterBuildTabs ELSE AfterLooseTabs
  /\ targetValid' = TRUE
  /\ dropVisible' = TRUE
  /\ UNCHANGED <<groupOf, collapsed, active, selected, selectionAnchor,
       phase, capturedMembers, recordedActive, originalTabs,
       originalGroupOf, originalCollapsed, originalActive, originalSelected,
       originalSelectionAnchor, visual, lastOutcome>>
  /\ Record("move-right", "move")

InvalidateSourceMembership ==
  /\ phase = "armed"
  /\ CapturedSourceIsCurrent
  /\ groupOf' = [groupOf EXCEPT ![ResearchTab2] = NoGroup]
  /\ UNCHANGED <<tabs, collapsed, active, selected, selectionAnchor, phase,
       capturedMembers, recordedActive, originalTabs, originalGroupOf,
       originalCollapsed, originalActive, originalSelected,
       originalSelectionAnchor, visual, targetValid, dropVisible, lastOutcome>>
  /\ Record("invalidate-source-membership", "none")

RejectStaleActivation ==
  /\ phase = "armed"
  /\ ~CapturedSourceIsCurrent
  /\ phase' = "idle"
  /\ visual' = "none"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ UNCHANGED <<Authoritative, capturedMembers, recordedActive,
       originalTabs, originalGroupOf, originalCollapsed, originalActive,
       originalSelected, originalSelectionAnchor, lastOutcome>>
  /\ Record("reject-stale-activation", "none")

RejectMissingGroup ==
  /\ phase \in Phases
  /\ UNCHANGED <<Authoritative, Interaction, originalTabs, originalGroupOf,
       originalCollapsed, originalActive, originalSelected,
       originalSelectionAnchor>>
  /\ Record("reject-missing-group", "none")

RejectOutOfPhaseEvent ==
  /\ phase # "dragging"
  /\ UNCHANGED <<Authoritative, Interaction, originalTabs, originalGroupOf,
       originalCollapsed, originalActive, originalSelected,
       originalSelectionAnchor>>
  /\ Record("reject-out-of-phase-event", "none")

RejectRepeatedEvent ==
  /\ lastAction # "init"
  /\ UNCHANGED <<Authoritative, Interaction, originalTabs, originalGroupOf,
       originalCollapsed, originalActive, originalSelected,
       originalSelectionAnchor>>
  /\ Record("reject-repeated-event", "none")

RejectInvalidTarget ==
  /\ phase = "dragging"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ UNCHANGED <<Authoritative, phase, capturedMembers, recordedActive,
       originalTabs, originalGroupOf, originalCollapsed, originalActive,
       originalSelected, originalSelectionAnchor, visual, lastOutcome>>
  /\ Record("reject-invalid-target", "none")

CommitDrag ==
  /\ phase = "dragging"
  /\ collapsed' = originalCollapsed
  /\ active' = recordedActive
  /\ selected' = {recordedActive}
  /\ selectionAnchor' = recordedActive
  /\ phase' = "settling"
  /\ visual' = "none"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ lastOutcome' = "committed"
  /\ UNCHANGED <<tabs, groupOf, capturedMembers, recordedActive,
       originalTabs, originalGroupOf, originalCollapsed, originalActive,
       originalSelected, originalSelectionAnchor>>
  /\ Record("commit-drag", "commit")

CancelDrag ==
  /\ phase = "dragging" \/ (phase = "armed" /\ CapturedSourceIsCurrent)
  /\ tabs' = originalTabs
  /\ groupOf' = originalGroupOf
  /\ collapsed' = originalCollapsed
  /\ active' = originalActive
  /\ selected' = originalSelected
  /\ selectionAnchor' = originalSelectionAnchor
  /\ phase' = "idle"
  /\ visual' = "none"
  /\ targetValid' = FALSE
  /\ dropVisible' = FALSE
  /\ lastOutcome' = "cancelled"
  /\ UNCHANGED <<capturedMembers, recordedActive, originalTabs,
       originalGroupOf, originalCollapsed, originalActive, originalSelected,
       originalSelectionAnchor>>
  /\ Record("cancel-drag", "cancel")

SettleDrag ==
  /\ phase = "settling"
  /\ phase' = "idle"
  /\ lastOutcome' = "settled"
  /\ UNCHANGED <<Authoritative, capturedMembers, recordedActive,
       originalTabs, originalGroupOf, originalCollapsed, originalActive,
       originalSelected, originalSelectionAnchor, visual, targetValid,
       dropVisible>>
  /\ Record("settle-drag", "none")

ToggleResearchGroup ==
  /\ phase = "idle"
  /\ collapsed' =
       IF ResearchGroup \in collapsed
       THEN collapsed \ {ResearchGroup}
       ELSE collapsed \cup {ResearchGroup}
  /\ selected' = {active}
  /\ selectionAnchor' = active
  /\ lastOutcome' = "none"
  /\ UNCHANGED <<tabs, groupOf, active, phase, capturedMembers,
       recordedActive, originalTabs, originalGroupOf, originalCollapsed,
       originalActive, originalSelected, originalSelectionAnchor, visual,
       targetValid, dropVisible>>
  /\ Record("toggle-research-group", "toggle")

ToggleBuildGroup ==
  /\ phase = "idle"
  /\ collapsed' =
       IF BuildGroup \in collapsed
       THEN collapsed \ {BuildGroup}
       ELSE collapsed \cup {BuildGroup}
  /\ selected' = {active}
  /\ selectionAnchor' = active
  /\ lastOutcome' = "none"
  /\ UNCHANGED <<tabs, groupOf, active, phase, capturedMembers,
       recordedActive, originalTabs, originalGroupOf, originalCollapsed,
       originalActive, originalSelected, originalSelectionAnchor, visual,
       targetValid, dropVisible>>
  /\ Record("toggle-build-group", "toggle")

Next ==
  \/ ArmGroup
  \/ ActivateDrag
  \/ MoveLeft
  \/ MoveRight
  \/ InvalidateSourceMembership
  \/ RejectStaleActivation
  \/ RejectMissingGroup
  \/ RejectOutOfPhaseEvent
  \/ RejectRepeatedEvent
  \/ RejectInvalidTarget
  \/ CommitDrag
  \/ CancelDrag
  \/ SettleDrag
  \/ ToggleResearchGroup
  \/ ToggleBuildGroup

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ tabs \in TabOrderType
  /\ groupOf \in GroupMapType
  /\ collapsed \in SUBSET Groups
  /\ active \in Tabs
  /\ selected \in SUBSET Tabs
  /\ selectionAnchor \in Tabs
  /\ phase \in Phases
  /\ capturedMembers \in [1..2 -> Tabs]
  /\ recordedActive \in Tabs
  /\ originalTabs \in TabOrderType
  /\ originalGroupOf \in GroupMapType
  /\ originalCollapsed \in SUBSET Groups
  /\ originalActive \in Tabs
  /\ originalSelected \in SUBSET Tabs
  /\ originalSelectionAnchor \in Tabs
  /\ visual \in Visuals
  /\ targetValid \in BOOLEAN
  /\ dropVisible \in BOOLEAN
  /\ lastOutcome \in Outcomes
  /\ previousAuthoritative \in AuthoritativeDomain
  /\ previousInteraction \in
       Phases \X [1..2 -> Tabs] \X Tabs \X Visuals
         \X BOOLEAN \X BOOLEAN \X Outcomes
  /\ lastAction \in Actions
  /\ lastCommand \in Commands

UniqueTabIdentity ==
  /\ Len(tabs) = Len(InitialTabs)
  /\ Cardinality(Range(tabs)) = Len(tabs)
  /\ Range(tabs) = Tabs

KnownGroupMembership ==
  \A tab \in Tabs: groupOf[tab] \in Groups \cup {NoGroup}

PinnedPrefix ==
  /\ tabs[1] = PinnedTab
  /\ groupOf[PinnedTab] = NoGroup

GroupContiguous ==
  \A group \in Groups: GroupContiguousFor(tabs, groupOf, group)

ActiveTabExists == active \in Range(tabs)

SelectionExists ==
  /\ selected # {}
  /\ selected \subseteq Range(tabs)
  /\ selectionAnchor \in Range(tabs)

StableActiveSurfaceExists ==
  phase = "dragging" \/ ActiveSurface \in VisibleSurfaces

DragSourceExists ==
  phase = "idle" \/
    /\ Range(capturedMembers) = SourceMembers
    /\ SourceMembers \subseteq Range(tabs)

GroupMovePermutation ==
  lastAction \notin {"move-left", "move-right"} \/
    /\ Len(tabs) = Len(previousAuthoritative[1])
    /\ Range(tabs) = Range(previousAuthoritative[1])

GroupMoveAtomic ==
  lastAction \notin {"move-left", "move-right"} \/
    /\ SourceBlockIsExact(tabs, groupOf)
    /\ groupOf = previousAuthoritative[2]
    /\ OutsiderOrderUnchanged(previousAuthoritative[1], tabs)
    /\ IF lastAction = "move-right"
       THEN
         \/ /\ previousAuthoritative[1] = InitialTabs
            /\ tabs = AfterBuildTabs
         \/ /\ previousAuthoritative[1] = AfterBuildTabs
            /\ tabs = AfterLooseTabs
       ELSE
         \/ /\ previousAuthoritative[1] = AfterLooseTabs
            /\ tabs = AfterBuildTabs
         \/ /\ previousAuthoritative[1] = AfterBuildTabs
            /\ tabs = InitialTabs

GroupTargetBoundary ==
  phase # "dragging" \/
    IndexOf(tabs, ResearchTab1) \in {2, 4, 5}

CollapsedDragStaysCompact ==
  ~(phase \in {"armed", "dragging"} /\
      ResearchGroup \in originalCollapsed)
    \/ visual = "collapsed-group"

ExpandedDragKeepsFullVisual ==
  ~(phase \in {"armed", "dragging"} /\
      ResearchGroup \notin originalCollapsed)
    \/ visual = "expanded-group"

DraggingBackingGroupIsCollapsed ==
  phase # "dragging" \/ ResearchGroup \in collapsed

CompletionRestoresCollapsedIntent ==
  lastOutcome \notin {"committed", "settled"} \/
    collapsed = originalCollapsed

CompletionSelectsRecordedActive ==
  lastOutcome \notin {"committed", "settled"} \/
    /\ active = recordedActive
    /\ selected = {recordedActive}
    /\ selectionAnchor = recordedActive

CollapsedActiveUsesGroupHeader ==
  ~(groupOf[active] \in collapsed) \/ ActiveSurface = groupOf[active]

CancelRestoresAuthoritativeState ==
  lastOutcome # "cancelled" \/ Authoritative = OriginalAuthoritative

NoInvalidDropFeedback == targetValid \/ ~dropVisible

RejectedInputPreservesAuthorityAndCommands ==
  lastAction \notin
    (PassiveRejectedActions
      \cup {"reject-stale-activation", "reject-invalid-target"})
  \/
    /\ Authoritative = previousAuthoritative
    /\ lastCommand = "none"

PassiveRejectedInputIsExactNoOp ==
  lastAction \notin PassiveRejectedActions \/
    /\ Interaction = previousInteraction
    /\ Authoritative = previousAuthoritative
    /\ lastCommand = "none"

StaleActivationRetiresWithoutMutation ==
  lastAction # "reject-stale-activation" \/
    /\ previousInteraction[1] = "armed"
    /\ phase = "idle"
    /\ Authoritative = previousAuthoritative
    /\ lastCommand = "none"

InvalidTargetCannotMutateOrCommand ==
  lastAction # "reject-invalid-target" \/
    /\ Authoritative = previousAuthoritative
    /\ ~targetValid
    /\ ~dropVisible
    /\ lastCommand = "none"

=============================================================================
