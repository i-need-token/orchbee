---------------- MODULE TerminalTabPresentationHandoff ----------------
EXTENDS FiniteSets, Naturals, TLC

Tabs == {"tab-a", "tab-b", "tab-c"}
Presentations == {"presentation-a", "presentation-b"}
ExpectedSets == SUBSET Presentations
None == "none"
Phases == {"settled", "preparing", "activating"}
MaxEpoch == 3

VARIABLES
  phase,
  displayedTab,
  targetTab,
  epoch,
  expected,
  displayReady,
  activationReady,
  targetDisplayCommitted,
  interactiveTab,
  lastReportIgnored,
  previousPhase,
  previousDisplayedTab,
  previousTargetTab,
  previousEpoch,
  previousExpected,
  previousDisplayReady,
  previousActivationReady,
  previousTargetDisplayCommitted,
  previousInteractiveTab

vars ==
  <<phase, displayedTab, targetTab, epoch, expected, displayReady,
    activationReady, targetDisplayCommitted, interactiveTab,
    lastReportIgnored, previousPhase, previousDisplayedTab,
    previousTargetTab, previousEpoch, previousExpected,
    previousDisplayReady, previousActivationReady,
    previousTargetDisplayCommitted, previousInteractiveTab>>

RecordNormal ==
  /\ lastReportIgnored' = FALSE
  /\ previousPhase' = phase
  /\ previousDisplayedTab' = displayedTab
  /\ previousTargetTab' = targetTab
  /\ previousEpoch' = epoch
  /\ previousExpected' = expected
  /\ previousDisplayReady' = displayReady
  /\ previousActivationReady' = activationReady
  /\ previousTargetDisplayCommitted' = targetDisplayCommitted
  /\ previousInteractiveTab' = interactiveTab

RecordIgnored ==
  /\ lastReportIgnored' = TRUE
  /\ previousPhase' = phase
  /\ previousDisplayedTab' = displayedTab
  /\ previousTargetTab' = targetTab
  /\ previousEpoch' = epoch
  /\ previousExpected' = expected
  /\ previousDisplayReady' = displayReady
  /\ previousActivationReady' = activationReady
  /\ previousTargetDisplayCommitted' = targetDisplayCommitted
  /\ previousInteractiveTab' = interactiveTab

Init ==
  /\ phase = "settled"
  /\ displayedTab = "tab-a"
  /\ targetTab = "tab-a"
  /\ epoch = 0
  /\ expected = {}
  /\ displayReady = {}
  /\ activationReady = {}
  /\ targetDisplayCommitted = TRUE
  /\ interactiveTab = "tab-a"
  /\ lastReportIgnored = FALSE
  /\ previousPhase = phase
  /\ previousDisplayedTab = displayedTab
  /\ previousTargetTab = targetTab
  /\ previousEpoch = epoch
  /\ previousExpected = expected
  /\ previousDisplayReady = displayReady
  /\ previousActivationReady = activationReady
  /\ previousTargetDisplayCommitted = targetDisplayCommitted
  /\ previousInteractiveTab = interactiveTab

BeginSwitch(nextTab, nextExpected) ==
  /\ phase = "settled"
  /\ nextTab \in Tabs \ {displayedTab}
  /\ nextExpected \in ExpectedSets \ {{}}
  /\ epoch < MaxEpoch
  /\ phase' = "preparing"
  /\ targetTab' = nextTab
  /\ epoch' = epoch + 1
  /\ expected' = nextExpected
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = FALSE
  /\ interactiveTab' = None
  /\ UNCHANGED displayedTab
  /\ RecordNormal

Supersede(nextTab, nextExpected) ==
  /\ phase \in {"preparing", "activating"}
  /\ nextTab \in Tabs \ {targetTab}
  /\ nextExpected \in ExpectedSets \ {{}}
  /\ epoch < MaxEpoch
  /\ phase' = "preparing"
  /\ targetTab' = nextTab
  /\ epoch' = epoch + 1
  /\ expected' = nextExpected
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = FALSE
  /\ interactiveTab' = None
  /\ UNCHANGED displayedTab
  /\ RecordNormal

SupersedeWithoutSurface(nextTab) ==
  /\ phase \in {"preparing", "activating"}
  /\ nextTab \in Tabs \ {targetTab}
  /\ epoch < MaxEpoch
  /\ phase' = "settled"
  /\ displayedTab' = nextTab
  /\ targetTab' = nextTab
  /\ epoch' = epoch + 1
  /\ expected' = {}
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = TRUE
  /\ interactiveTab' = nextTab
  /\ RecordNormal

SwitchWithoutSurface(nextTab) ==
  /\ phase = "settled"
  /\ nextTab \in Tabs \ {displayedTab}
  /\ epoch < MaxEpoch
  /\ phase' = "settled"
  /\ displayedTab' = nextTab
  /\ targetTab' = nextTab
  /\ epoch' = epoch + 1
  /\ expected' = {}
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = TRUE
  /\ interactiveTab' = nextTab
  /\ RecordNormal

RemoveOutgoingAndSwitch(nextTab, nextExpected) ==
  /\ phase = "settled"
  /\ nextTab \in Tabs \ {displayedTab}
  /\ nextExpected \in ExpectedSets \ {{}}
  /\ epoch < MaxEpoch
  /\ phase' = "preparing"
  /\ displayedTab' = None
  /\ targetTab' = nextTab
  /\ epoch' = epoch + 1
  /\ expected' = nextExpected
  /\ displayReady' = {}
  /\ activationReady' = {}
  /\ targetDisplayCommitted' = FALSE
  /\ interactiveTab' = None
  /\ RecordNormal

Retarget(nextExpected) ==
  /\ phase \in {"preparing", "activating"}
  /\ nextExpected \in ExpectedSets \ {expected}
  /\ expected' = nextExpected
  /\ IF nextExpected \subseteq activationReady
     THEN
       /\ phase' = "settled"
       /\ displayedTab' = targetTab
       /\ targetDisplayCommitted' = TRUE
       /\ interactiveTab' = targetTab
     ELSE IF nextExpected \subseteq displayReady
     THEN
       /\ phase' = "activating"
       /\ displayedTab' = targetTab
       /\ targetDisplayCommitted' = TRUE
       /\ interactiveTab' = None
     ELSE
       /\ phase' = "preparing"
       /\ targetDisplayCommitted' = FALSE
       /\ interactiveTab' = None
       /\ UNCHANGED displayedTab
  /\ UNCHANGED <<targetTab, epoch, displayReady, activationReady>>
  /\ RecordNormal

RemovePendingOutgoing ==
  /\ phase = "preparing"
  /\ displayedTab \in Tabs \ {targetTab}
  /\ displayedTab' = None
  /\ UNCHANGED
       <<phase, targetTab, epoch, expected, displayReady, activationReady,
         targetDisplayCommitted, interactiveTab>>
  /\ RecordNormal

ReportDisplay(presentation) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in Presentations \ displayReady
  /\ LET nextDisplayReady == displayReady \cup {presentation}
     IN
       /\ displayReady' = nextDisplayReady
       /\ IF expected \subseteq activationReady
          THEN
            /\ displayedTab' = targetTab
            /\ targetDisplayCommitted' = TRUE
            /\ phase' = "settled"
            /\ interactiveTab' = targetTab
          ELSE IF expected \subseteq nextDisplayReady
          THEN
            /\ displayedTab' = targetTab
            /\ targetDisplayCommitted' = TRUE
            /\ phase' = "activating"
            /\ interactiveTab' = None
          ELSE
            /\ phase' = "preparing"
            /\ targetDisplayCommitted' = FALSE
            /\ interactiveTab' = None
            /\ UNCHANGED displayedTab
  /\ UNCHANGED <<targetTab, epoch, expected, activationReady>>
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
            /\ displayedTab' = targetTab
            /\ targetDisplayCommitted' = TRUE
            /\ interactiveTab' = targetTab
          ELSE IF expected \subseteq nextDisplayReady
          THEN
            /\ phase' = "activating"
            /\ displayedTab' = targetTab
            /\ targetDisplayCommitted' = TRUE
            /\ interactiveTab' = None
          ELSE
            /\ phase' = "preparing"
            /\ targetDisplayCommitted' = FALSE
            /\ interactiveTab' = None
            /\ UNCHANGED displayedTab
  /\ UNCHANGED <<targetTab, epoch, expected>>
  /\ RecordNormal

RejectStaleDisplay(presentation, candidateEpoch) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in Presentations
  /\ candidateEpoch \in 0..MaxEpoch
  /\ candidateEpoch # epoch
  /\ UNCHANGED
       <<phase, displayedTab, targetTab, epoch, expected, displayReady,
         activationReady, targetDisplayCommitted, interactiveTab>>
  /\ RecordIgnored

RejectStaleActivation(presentation, candidateEpoch) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in Presentations
  /\ candidateEpoch \in 0..MaxEpoch
  /\ candidateEpoch # epoch
  /\ UNCHANGED
       <<phase, displayedTab, targetTab, epoch, expected, displayReady,
         activationReady, targetDisplayCommitted, interactiveTab>>
  /\ RecordIgnored

RejectWrongTargetReport(presentation, candidateTab) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in Presentations
  /\ candidateTab \in Tabs \ {targetTab}
  /\ UNCHANGED
       <<phase, displayedTab, targetTab, epoch, expected, displayReady,
         activationReady, targetDisplayCommitted, interactiveTab>>
  /\ RecordIgnored

RejectDuplicateReport(presentation) ==
  /\ phase \in {"preparing", "activating"}
  /\ presentation \in displayReady \cup activationReady
  /\ UNCHANGED
       <<phase, displayedTab, targetTab, epoch, expected, displayReady,
         activationReady, targetDisplayCommitted, interactiveTab>>
  /\ RecordIgnored

Next ==
  \/ \E nextTab \in Tabs, nextExpected \in ExpectedSets:
       BeginSwitch(nextTab, nextExpected)
  \/ \E nextTab \in Tabs, nextExpected \in ExpectedSets:
       Supersede(nextTab, nextExpected)
  \/ \E nextTab \in Tabs: SupersedeWithoutSurface(nextTab)
  \/ \E nextTab \in Tabs: SwitchWithoutSurface(nextTab)
  \/ \E nextTab \in Tabs, nextExpected \in ExpectedSets:
       RemoveOutgoingAndSwitch(nextTab, nextExpected)
  \/ \E nextExpected \in ExpectedSets: Retarget(nextExpected)
  \/ RemovePendingOutgoing
  \/ \E presentation \in Presentations: ReportDisplay(presentation)
  \/ \E presentation \in Presentations: ReportActivation(presentation)
  \/ \E presentation \in Presentations, candidateEpoch \in 0..MaxEpoch:
       RejectStaleDisplay(presentation, candidateEpoch)
  \/ \E presentation \in Presentations, candidateEpoch \in 0..MaxEpoch:
       RejectStaleActivation(presentation, candidateEpoch)
  \/ \E presentation \in Presentations, candidateTab \in Tabs:
       RejectWrongTargetReport(presentation, candidateTab)
  \/ \E presentation \in Presentations: RejectDuplicateReport(presentation)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in Phases
  /\ displayedTab \in Tabs \cup {None}
  /\ targetTab \in Tabs
  /\ epoch \in 0..MaxEpoch
  /\ expected \subseteq Presentations
  /\ displayReady \subseteq Presentations
  /\ activationReady \subseteq Presentations
  /\ targetDisplayCommitted \in BOOLEAN
  /\ interactiveTab \in Tabs \cup {None}
  /\ lastReportIgnored \in BOOLEAN
  /\ previousPhase \in Phases
  /\ previousDisplayedTab \in Tabs \cup {None}
  /\ previousTargetTab \in Tabs
  /\ previousEpoch \in 0..MaxEpoch
  /\ previousExpected \subseteq Presentations
  /\ previousDisplayReady \subseteq Presentations
  /\ previousActivationReady \subseteq Presentations
  /\ previousTargetDisplayCommitted \in BOOLEAN
  /\ previousInteractiveTab \in Tabs \cup {None}

ActivationImpliesDisplay == activationReady \subseteq displayReady

VisualCommitRequiresDisplay ==
  targetDisplayCommitted /\ expected # {} => expected \subseteq displayReady

PendingInteractionIsInert ==
  phase \in {"preparing", "activating"} => interactiveTab = None

PreparationHasNoVisualCommit ==
  phase = "preparing" => ~targetDisplayCommitted

SettledInteractionIsExclusive ==
  phase = "settled" =>
    /\ displayedTab = targetTab
    /\ interactiveTab = displayedTab
    /\ targetDisplayCommitted
    /\ expected = {} \/ expected \subseteq activationReady

ActivationBarrierFollowsDisplayBarrier ==
  phase = "activating" =>
    /\ displayedTab = targetTab
    /\ targetDisplayCommitted
    /\ expected \subseteq displayReady
    /\ ~(expected \subseteq activationReady)

IgnoredReportsDoNotMutate ==
  lastReportIgnored =>
    /\ phase = previousPhase
    /\ displayedTab = previousDisplayedTab
    /\ targetTab = previousTargetTab
    /\ epoch = previousEpoch
    /\ expected = previousExpected
    /\ displayReady = previousDisplayReady
    /\ activationReady = previousActivationReady
    /\ targetDisplayCommitted = previousTargetDisplayCommitted
    /\ interactiveTab = previousInteractiveTab

=============================================================================
