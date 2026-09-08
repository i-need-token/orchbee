---------------- MODULE TerminalTabPresentationHandoff ----------------
\* This model covers handoff to a surviving non-null target. The projector's
\* zero-layer clear, two-layer bound, and same-revision key reuse are owned by
\* deterministic projection tests rather than derived TLA+ layer assertions.
EXTENDS FiniteSets, Naturals, TLC

Tabs == {"tab-a", "tab-b", "tab-c"}
Targets == {"tab-a-layout-1", "tab-a-layout-2", "tab-b-layout-1", "tab-c-layout-1"}
TargetTab == [target \in Targets |->
  IF target \in {"tab-a-layout-1", "tab-a-layout-2"}
  THEN "tab-a"
  ELSE IF target = "tab-b-layout-1" THEN "tab-b" ELSE "tab-c"]
Presentations == {"presentation-a", "presentation-b"}
ExpectedSets == SUBSET Presentations
None == "none"
Phases == {"settled", "preparing", "activating"}
MaxEpoch == 3

VARIABLES
  phase,
  liveTabs,
  displayedTarget,
  target,
  epoch,
  expected,
  displayReady,
  activationReady,
  targetDisplayCommitted,
  interactiveTarget,
  lastReportIgnored,
  previousPhase,
  previousLiveTabs,
  previousDisplayedTarget,
  previousTarget,
  previousEpoch,
  previousExpected,
  previousDisplayReady,
  previousActivationReady,
  previousTargetDisplayCommitted,
  previousInteractiveTarget

vars ==
  <<phase, liveTabs, displayedTarget, target, epoch, expected, displayReady,
    activationReady, targetDisplayCommitted, interactiveTarget,
    lastReportIgnored, previousPhase, previousLiveTabs, previousDisplayedTarget,
    previousTarget, previousEpoch, previousExpected, previousDisplayReady,
    previousActivationReady, previousTargetDisplayCommitted, previousInteractiveTarget>>

RecordNormal ==
  /\ lastReportIgnored' = FALSE
  /\ previousPhase' = phase
  /\ previousLiveTabs' = liveTabs
  /\ previousDisplayedTarget' = displayedTarget
  /\ previousTarget' = target
  /\ previousEpoch' = epoch
  /\ previousExpected' = expected
  /\ previousDisplayReady' = displayReady
  /\ previousActivationReady' = activationReady
  /\ previousTargetDisplayCommitted' = targetDisplayCommitted
  /\ previousInteractiveTarget' = interactiveTarget

RecordIgnored ==
  /\ lastReportIgnored' = TRUE
  /\ previousPhase' = phase
  /\ previousLiveTabs' = liveTabs
  /\ previousDisplayedTarget' = displayedTarget
  /\ previousTarget' = target
  /\ previousEpoch' = epoch
  /\ previousExpected' = expected
  /\ previousDisplayReady' = displayReady
  /\ previousActivationReady' = activationReady
  /\ previousTargetDisplayCommitted' = targetDisplayCommitted
  /\ previousInteractiveTarget' = interactiveTarget

Init ==
  /\ phase = "settled"
  /\ liveTabs = Tabs
  /\ displayedTarget = "tab-a-layout-1"
  /\ target = "tab-a-layout-1"
  /\ epoch = 0
  /\ expected = {}
  /\ displayReady = {}
  /\ activationReady = {}
  /\ targetDisplayCommitted = TRUE
  /\ interactiveTarget = "tab-a-layout-1"
  /\ lastReportIgnored = FALSE
  /\ previousPhase = phase
  /\ previousLiveTabs = liveTabs
  /\ previousDisplayedTarget = displayedTarget
  /\ previousTarget = target
  /\ previousEpoch = epoch
  /\ previousExpected = expected
  /\ previousDisplayReady = displayReady
  /\ previousActivationReady = activationReady
  /\ previousTargetDisplayCommitted = targetDisplayCommitted
  /\ previousInteractiveTarget = interactiveTarget

BeginSwitch(nextTarget, nextExpected) ==
  /\ phase = "settled"
  /\ nextTarget \in Targets
  /\ TargetTab[nextTarget] \in liveTabs \ {TargetTab[displayedTarget]}
  /\ nextExpected \in ExpectedSets \ {{}}
  /\ epoch < MaxEpoch
  /\ phase' = "preparing"
  /\ target' = nextTarget
  /\ epoch' = epoch + 1
  /\ expected' = nextExpected
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = FALSE
  /\ interactiveTarget' = None
  /\ UNCHANGED <<liveTabs, displayedTarget>>
  /\ RecordNormal

BeginRevisionReplacement(nextTarget, nextExpected) ==
  /\ phase = "settled"
  /\ nextTarget \in Targets \ {displayedTarget}
  /\ TargetTab[nextTarget] = TargetTab[displayedTarget]
  /\ nextExpected \in ExpectedSets \ {{}}
  /\ epoch < MaxEpoch
  /\ phase' = "preparing"
  /\ target' = nextTarget
  /\ epoch' = epoch + 1
  /\ expected' = nextExpected
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = FALSE
  /\ interactiveTarget' = None
  /\ UNCHANGED <<liveTabs, displayedTarget>>
  /\ RecordNormal

Supersede(nextTarget, nextExpected) ==
  /\ phase \in {"preparing", "activating"}
  /\ nextTarget \in Targets \ {target}
  /\ TargetTab[nextTarget] \in liveTabs
  /\ nextExpected \in ExpectedSets \ {{}}
  /\ epoch < MaxEpoch
  /\ phase' = "preparing"
  /\ target' = nextTarget
  /\ epoch' = epoch + 1
  /\ expected' = nextExpected
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = FALSE
  /\ interactiveTarget' = None
  /\ UNCHANGED <<liveTabs, displayedTarget>>
  /\ RecordNormal

SupersedeWithoutSurface(nextTarget) ==
  /\ phase \in {"preparing", "activating"}
  /\ nextTarget \in Targets \ {target}
  /\ TargetTab[nextTarget] \in liveTabs
  /\ epoch < MaxEpoch
  /\ phase' = "settled"
  /\ displayedTarget' = nextTarget
  /\ target' = nextTarget
  /\ epoch' = epoch + 1
  /\ expected' = {}
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = TRUE
  /\ interactiveTarget' = nextTarget
  /\ UNCHANGED liveTabs
  /\ RecordNormal

SwitchWithoutSurface(nextTarget) ==
  /\ phase = "settled"
  /\ nextTarget \in Targets \ {displayedTarget}
  /\ TargetTab[nextTarget] \in liveTabs
  /\ epoch < MaxEpoch
  /\ phase' = "settled"
  /\ displayedTarget' = nextTarget
  /\ target' = nextTarget
  /\ epoch' = epoch + 1
  /\ expected' = {}
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = TRUE
  /\ interactiveTarget' = nextTarget
  /\ UNCHANGED liveTabs
  /\ RecordNormal

RemoveOutgoingAndSwitch(nextTarget, nextExpected) ==
  /\ phase = "settled"
  /\ TargetTab[displayedTarget] \in liveTabs
  /\ nextTarget \in Targets
  /\ TargetTab[nextTarget] \in liveTabs \ {TargetTab[displayedTarget]}
  /\ nextExpected \in ExpectedSets \ {{}}
  /\ epoch < MaxEpoch
  /\ phase' = "preparing"
  /\ liveTabs' = liveTabs \ {TargetTab[displayedTarget]}
  /\ target' = nextTarget
  /\ epoch' = epoch + 1
  /\ expected' = nextExpected
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = FALSE
  /\ interactiveTarget' = None
  /\ UNCHANGED displayedTarget
  /\ RecordNormal

Retarget(nextExpected) ==
  /\ phase \in {"preparing", "activating"}
  /\ nextExpected \in ExpectedSets \ {expected}
  /\ expected' = nextExpected
  /\ IF nextExpected \subseteq activationReady
     THEN
       /\ phase' = "settled"
       /\ displayedTarget' = target
       /\ targetDisplayCommitted' = TRUE
       /\ interactiveTarget' = target
     ELSE IF nextExpected \subseteq displayReady
     THEN
       /\ phase' = "activating"
       /\ displayedTarget' = target
       /\ targetDisplayCommitted' = TRUE
       /\ interactiveTarget' = None
     ELSE
       /\ phase' = "preparing"
       /\ targetDisplayCommitted' = FALSE
       /\ interactiveTarget' = None
       /\ UNCHANGED displayedTarget
  /\ UNCHANGED <<liveTabs, target, epoch, displayReady, activationReady>>
  /\ RecordNormal

RemovePendingOutgoing ==
  /\ phase = "preparing"
  /\ displayedTarget \in Targets
  /\ TargetTab[displayedTarget] \in liveTabs \ {TargetTab[target]}
  /\ liveTabs' = liveTabs \ {TargetTab[displayedTarget]}
  /\ UNCHANGED
       <<phase, displayedTarget, target, epoch, expected, displayReady, activationReady,
         targetDisplayCommitted, interactiveTarget>>
  /\ RecordNormal

ReportDisplay(presentation) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in Presentations \ displayReady
  /\ LET nextDisplayReady == displayReady \cup {presentation}
     IN
       /\ displayReady' = nextDisplayReady
       /\ IF expected \subseteq activationReady
          THEN
            /\ displayedTarget' = target
            /\ targetDisplayCommitted' = TRUE
            /\ phase' = "settled"
            /\ interactiveTarget' = target
          ELSE IF expected \subseteq nextDisplayReady
          THEN
            /\ displayedTarget' = target
            /\ targetDisplayCommitted' = TRUE
            /\ phase' = "activating"
            /\ interactiveTarget' = None
          ELSE
            /\ phase' = "preparing"
            /\ targetDisplayCommitted' = FALSE
            /\ interactiveTarget' = None
            /\ UNCHANGED displayedTarget
  /\ UNCHANGED <<liveTabs, target, epoch, expected, activationReady>>
  /\ RecordNormal

ReportActivation(presentation) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in Presentations \ activationReady
  /\ LET nextActivationReady == activationReady \cup {presentation}
         nextDisplayReady == displayReady \cup {presentation}
     IN
       /\ activationReady' = nextActivationReady
       /\ displayReady' = nextDisplayReady
       /\ IF expected \subseteq nextActivationReady
          THEN
            /\ phase' = "settled"
            /\ displayedTarget' = target
            /\ targetDisplayCommitted' = TRUE
            /\ interactiveTarget' = target
          ELSE IF expected \subseteq nextDisplayReady
          THEN
            /\ phase' = "activating"
            /\ displayedTarget' = target
            /\ targetDisplayCommitted' = TRUE
            /\ interactiveTarget' = None
          ELSE
            /\ phase' = "preparing"
            /\ targetDisplayCommitted' = FALSE
            /\ interactiveTarget' = None
            /\ UNCHANGED displayedTarget
  /\ UNCHANGED <<liveTabs, target, epoch, expected>>
  /\ RecordNormal

RejectStaleDisplay(presentation, candidateEpoch) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in Presentations
  /\ candidateEpoch \in 0..MaxEpoch
  /\ candidateEpoch # epoch
  /\ UNCHANGED
       <<phase, liveTabs, displayedTarget, target, epoch, expected, displayReady,
         activationReady, targetDisplayCommitted, interactiveTarget>>
  /\ RecordIgnored

RejectStaleActivation(presentation, candidateEpoch) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in Presentations
  /\ candidateEpoch \in 0..MaxEpoch
  /\ candidateEpoch # epoch
  /\ UNCHANGED
       <<phase, liveTabs, displayedTarget, target, epoch, expected, displayReady,
         activationReady, targetDisplayCommitted, interactiveTarget>>
  /\ RecordIgnored

RejectWrongTargetReport(presentation, candidateTarget) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in Presentations
  /\ candidateTarget \in Targets \ {target}
  /\ UNCHANGED
       <<phase, liveTabs, displayedTarget, target, epoch, expected, displayReady,
         activationReady, targetDisplayCommitted, interactiveTarget>>
  /\ RecordIgnored

RejectDuplicateReport(presentation) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in displayReady \cup activationReady
  /\ UNCHANGED
       <<phase, liveTabs, displayedTarget, target, epoch, expected, displayReady,
         activationReady, targetDisplayCommitted, interactiveTarget>>
  /\ RecordIgnored

Next ==
  \/ \E nextTarget \in Targets, nextExpected \in ExpectedSets:
       BeginSwitch(nextTarget, nextExpected)
  \/ \E nextTarget \in Targets, nextExpected \in ExpectedSets:
       BeginRevisionReplacement(nextTarget, nextExpected)
  \/ \E nextTarget \in Targets, nextExpected \in ExpectedSets:
       Supersede(nextTarget, nextExpected)
  \/ \E nextTarget \in Targets: SupersedeWithoutSurface(nextTarget)
  \/ \E nextTarget \in Targets: SwitchWithoutSurface(nextTarget)
  \/ \E nextTarget \in Targets, nextExpected \in ExpectedSets:
       RemoveOutgoingAndSwitch(nextTarget, nextExpected)
  \/ \E nextExpected \in ExpectedSets: Retarget(nextExpected)
  \/ RemovePendingOutgoing
  \/ \E presentation \in Presentations: ReportDisplay(presentation)
  \/ \E presentation \in Presentations: ReportActivation(presentation)
  \/ \E presentation \in Presentations, candidateEpoch \in 0..MaxEpoch:
       RejectStaleDisplay(presentation, candidateEpoch)
  \/ \E presentation \in Presentations, candidateEpoch \in 0..MaxEpoch:
       RejectStaleActivation(presentation, candidateEpoch)
  \/ \E presentation \in Presentations, candidateTarget \in Targets:
       RejectWrongTargetReport(presentation, candidateTarget)
  \/ \E presentation \in Presentations: RejectDuplicateReport(presentation)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in Phases
  /\ liveTabs \subseteq Tabs
  /\ displayedTarget \in Targets \cup {None}
  /\ target \in Targets
  /\ epoch \in 0..MaxEpoch
  /\ expected \subseteq Presentations
  /\ displayReady \subseteq Presentations
  /\ activationReady \subseteq Presentations
  /\ targetDisplayCommitted \in BOOLEAN
  /\ interactiveTarget \in Targets \cup {None}
  /\ lastReportIgnored \in BOOLEAN
  /\ previousPhase \in Phases
  /\ previousLiveTabs \subseteq Tabs
  /\ previousDisplayedTarget \in Targets \cup {None}
  /\ previousTarget \in Targets
  /\ previousEpoch \in 0..MaxEpoch
  /\ previousExpected \subseteq Presentations
  /\ previousDisplayReady \subseteq Presentations
  /\ previousActivationReady \subseteq Presentations
  /\ previousTargetDisplayCommitted \in BOOLEAN
  /\ previousInteractiveTarget \in Targets \cup {None}

ActivationImpliesDisplay == activationReady \subseteq displayReady

TargetRemainsCanonical == TargetTab[target] \in liveTabs

PreparingRetainsMeaningfulOutgoing ==
  phase = "preparing" => displayedTarget # None

DetachedDisplayIsInert ==
  displayedTarget # None /\ TargetTab[displayedTarget] \notin liveTabs =>
    /\ phase = "preparing"
    /\ interactiveTarget = None
    /\ ~targetDisplayCommitted

VisualCommitRequiresDisplay ==
  targetDisplayCommitted /\ expected # {} => expected \subseteq displayReady

PendingInteractionIsInert ==
  phase \in {"preparing", "activating"} => interactiveTarget = None

PreparationHasNoVisualCommit ==
  phase = "preparing" => ~targetDisplayCommitted

SettledInteractionIsExclusive ==
  phase = "settled" =>
    /\ displayedTarget = target
    /\ interactiveTarget = displayedTarget
    /\ targetDisplayCommitted
    /\ expected = {} \/ expected \subseteq activationReady

ActivationBarrierFollowsDisplayBarrier ==
  phase = "activating" =>
    /\ displayedTarget = target
    /\ targetDisplayCommitted
    /\ expected \subseteq displayReady
    /\ ~(expected \subseteq activationReady)

IgnoredReportsDoNotMutate ==
  lastReportIgnored =>
    /\ phase = previousPhase
    /\ liveTabs = previousLiveTabs
    /\ displayedTarget = previousDisplayedTarget
    /\ target = previousTarget
    /\ epoch = previousEpoch
    /\ expected = previousExpected
    /\ displayReady = previousDisplayReady
    /\ activationReady = previousActivationReady
    /\ targetDisplayCommitted = previousTargetDisplayCommitted
    /\ interactiveTarget = previousInteractiveTarget

=============================================================================
