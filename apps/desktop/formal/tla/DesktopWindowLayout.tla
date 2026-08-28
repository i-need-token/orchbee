------------------------ MODULE DesktopWindowLayout ------------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS Windows, Tabs, InitialWindow, InitialTab, MaxGeneration

NoWindow == "no-window"
WindowTargets == Windows \cup {NoWindow}
Phases == {"idle", "provisioning", "preparing-native", "persisting"}
Kinds == {"none", "move", "new-window-move", "create-window", "close"}
Results == {"none", "completed", "failed"}

VARIABLES
  phase,
  result,
  logicalWindows,
  primaryWindow,
  activeWindow,
  documentTabs,
  assignment,
  attached,
  rendererGeneration,
  closeIssued,
  pendingKind,
  pendingTabs,
  pendingSource,
  pendingTarget,
  proposedWindows,
  proposedPrimary,
  proposedAssignment,
  preparedNative,
  lastAction,
  lastKind,
  lastSource,
  lastTabs,
  previousAssignment,
  previousWindows,
  previousAttached

vars == <<phase, result, logicalWindows, primaryWindow, activeWindow,
  documentTabs, assignment, attached, rendererGeneration, closeIssued,
  pendingKind, pendingTabs, pendingSource, pendingTarget, proposedWindows,
  proposedPrimary, proposedAssignment, preparedNative, lastAction, lastKind,
  lastSource, lastTabs, previousAssignment, previousWindows, previousAttached>>

Reassign(currentAssignment, movedTabs, target) ==
  [tab \in Tabs |->
    IF tab \in movedTabs THEN target ELSE currentAssignment[tab]]

SourcesOf(movedTabs) ==
  {window \in logicalWindows :
    \E tab \in movedTabs: assignment[tab] = window}

EmptyMovedSources(nextAssignment, movedTabs) ==
  {window \in SourcesOf(movedTabs) :
    \A tab \in documentTabs: nextAssignment[tab] # window}

Record(action, kind, source, tabs) ==
  /\ lastAction' = action
  /\ lastKind' = kind
  /\ lastSource' = source
  /\ lastTabs' = tabs
  /\ previousAssignment' = assignment
  /\ previousWindows' = logicalWindows
  /\ previousAttached' = attached

Init ==
  /\ InitialWindow \in Windows
  /\ InitialTab \in Tabs
  /\ phase = "idle"
  /\ result = "none"
  /\ logicalWindows = {InitialWindow}
  /\ primaryWindow = InitialWindow
  /\ activeWindow = InitialWindow
  /\ documentTabs = {InitialTab}
  /\ assignment = [tab \in Tabs |->
       IF tab = InitialTab THEN InitialWindow ELSE NoWindow]
  /\ attached = [window \in Windows |-> window = InitialWindow]
  /\ rendererGeneration = [window \in Windows |-> 0]
  /\ closeIssued = {}
  /\ pendingKind = "none"
  /\ pendingTabs = {}
  /\ pendingSource = NoWindow
  /\ pendingTarget = NoWindow
  /\ proposedWindows = logicalWindows
  /\ proposedPrimary = primaryWindow
  /\ proposedAssignment = assignment
  /\ preparedNative = NoWindow
  /\ lastAction = "init"
  /\ lastKind = "none"
  /\ lastSource = NoWindow
  /\ lastTabs = {}
  /\ previousAssignment = assignment
  /\ previousWindows = logicalWindows
  /\ previousAttached = attached

FocusWindow(target) ==
  /\ target \in logicalWindows
  /\ activeWindow' = target
  /\ UNCHANGED <<phase, result, logicalWindows, primaryWindow, documentTabs,
       assignment, attached, rendererGeneration, closeIssued, pendingKind,
       pendingTabs, pendingSource, pendingTarget, proposedWindows,
       proposedPrimary, proposedAssignment, preparedNative>>
  /\ Record("focus-window", "none", target, {})

ReconcileAdd(tab) ==
  /\ phase = "idle"
  /\ tab \in Tabs \ documentTabs
  /\ documentTabs' = documentTabs \cup {tab}
  /\ assignment' = [assignment EXCEPT ![tab] = activeWindow]
  /\ proposedAssignment' = assignment'
  /\ result' = "completed"
  /\ UNCHANGED <<phase, logicalWindows, primaryWindow, activeWindow,
       attached, rendererGeneration, closeIssued, pendingKind, pendingTabs,
       pendingSource, pendingTarget, proposedWindows, proposedPrimary,
       preparedNative>>
  /\ Record("reconcile-add", "none", activeWindow, {tab})

ReconcileRemove(tab) ==
  /\ phase = "idle"
  /\ tab \in documentTabs
  /\ documentTabs' = documentTabs \ {tab}
  /\ assignment' = [assignment EXCEPT ![tab] = NoWindow]
  /\ proposedAssignment' = assignment'
  /\ result' = "completed"
  /\ UNCHANGED <<phase, logicalWindows, primaryWindow, activeWindow,
       attached, rendererGeneration, closeIssued, pendingKind, pendingTabs,
       pendingSource, pendingTarget, proposedWindows, proposedPrimary,
       preparedNative>>
  /\ Record("reconcile-remove", "none", NoWindow, {tab})

RequestMove(movedTabs, target) ==
  /\ phase = "idle"
  /\ movedTabs \in (SUBSET documentTabs) \ {{}}
  /\ target \in logicalWindows
  /\ \E tab \in movedTabs: assignment[tab] # target
  /\ LET nextAssignment == Reassign(assignment, movedTabs, target)
         removedWindows == EmptyMovedSources(nextAssignment, movedTabs)
     IN
       /\ phase' = "persisting"
       /\ result' = "none"
       /\ pendingKind' = "move"
       /\ pendingTabs' = movedTabs
       /\ pendingSource' = NoWindow
       /\ pendingTarget' = target
       /\ proposedWindows' = logicalWindows \ removedWindows
       /\ proposedPrimary' =
            IF primaryWindow \in removedWindows THEN target ELSE primaryWindow
       /\ proposedAssignment' = nextAssignment
  /\ preparedNative' = NoWindow
  /\ UNCHANGED <<logicalWindows, primaryWindow, activeWindow, documentTabs,
       assignment, attached, rendererGeneration, closeIssued>>
  /\ Record("request-move", "move", NoWindow, movedTabs)

RequestMoveNewWindow(movedTabs, target) ==
  /\ phase = "idle"
  /\ movedTabs \in (SUBSET documentTabs) \ {{}}
  /\ target \in Windows \ (logicalWindows \cup closeIssued)
  /\ LET nextAssignment == Reassign(assignment, movedTabs, target)
         removedWindows == EmptyMovedSources(nextAssignment, movedTabs)
     IN
       /\ phase' = "preparing-native"
       /\ result' = "none"
       /\ pendingKind' = "new-window-move"
       /\ pendingTabs' = movedTabs
       /\ pendingSource' = NoWindow
       /\ pendingTarget' = target
       /\ proposedWindows' = (logicalWindows \cup {target}) \ removedWindows
       /\ proposedPrimary' =
            IF primaryWindow \in removedWindows THEN target ELSE primaryWindow
       /\ proposedAssignment' = nextAssignment
  /\ preparedNative' = NoWindow
  /\ UNCHANGED <<logicalWindows, primaryWindow, activeWindow, documentTabs,
       assignment, attached, rendererGeneration, closeIssued>>
  /\ Record("request-move-new-window", "new-window-move", NoWindow, movedTabs)

RequestNewWindow(source, target, tab) ==
  /\ phase = "idle"
  /\ source \in logicalWindows
  /\ target \in Windows \ (logicalWindows \cup closeIssued)
  /\ tab \in Tabs \ documentTabs
  /\ phase' = "provisioning"
  /\ result' = "none"
  /\ pendingKind' = "create-window"
  /\ pendingTabs' = {tab}
  /\ pendingSource' = source
  /\ pendingTarget' = target
  /\ preparedNative' = NoWindow
  /\ UNCHANGED <<logicalWindows, primaryWindow, activeWindow, documentTabs,
       assignment, attached, rendererGeneration, closeIssued, proposedWindows,
       proposedPrimary, proposedAssignment>>
  /\ Record("request-new-window", "create-window", source, {tab})

TerminalProvisioned ==
  /\ phase = "provisioning"
  /\ pendingKind = "create-window"
  /\ pendingTabs \in (SUBSET (Tabs \ documentTabs)) \ {{}}
  /\ Cardinality(pendingTabs) = 1
  /\ phase' = "preparing-native"
  /\ documentTabs' = documentTabs \cup pendingTabs
  /\ assignment' = Reassign(assignment, pendingTabs, pendingSource)
  /\ proposedWindows' = logicalWindows \cup {pendingTarget}
  /\ proposedPrimary' = primaryWindow
  /\ proposedAssignment' = Reassign(assignment', pendingTabs, pendingTarget)
  /\ preparedNative' = NoWindow
  /\ UNCHANGED <<result, logicalWindows, primaryWindow, activeWindow, attached,
       rendererGeneration, closeIssued, pendingKind, pendingTabs,
       pendingSource, pendingTarget>>
  /\ Record("terminal-provisioned", pendingKind, pendingSource, pendingTabs)

TerminalProvisioningFailed ==
  /\ phase = "provisioning"
  /\ phase' = "idle"
  /\ result' = "failed"
  /\ pendingKind' = "none"
  /\ pendingTabs' = {}
  /\ pendingSource' = NoWindow
  /\ pendingTarget' = NoWindow
  /\ proposedWindows' = logicalWindows
  /\ proposedPrimary' = primaryWindow
  /\ proposedAssignment' = assignment
  /\ preparedNative' = NoWindow
  /\ UNCHANGED <<logicalWindows, primaryWindow, activeWindow, documentTabs,
       assignment, attached, rendererGeneration, closeIssued>>
  /\ Record("terminal-provisioning-failed", pendingKind, pendingSource, pendingTabs)

NativeWindowPrepared ==
  /\ phase = "preparing-native"
  /\ pendingTarget \in Windows \ (logicalWindows \cup closeIssued)
  /\ phase' = "persisting"
  /\ preparedNative' = pendingTarget
  /\ UNCHANGED <<result, logicalWindows, primaryWindow, activeWindow,
       documentTabs, assignment, attached, rendererGeneration, closeIssued,
       pendingKind, pendingTabs, pendingSource, pendingTarget, proposedWindows,
       proposedPrimary, proposedAssignment>>
  /\ Record("native-window-prepared", pendingKind, pendingSource, pendingTabs)

NativeWindowPreparationFailed ==
  /\ phase = "preparing-native"
  /\ phase' = "idle"
  /\ result' = "failed"
  /\ pendingKind' = "none"
  /\ pendingTabs' = {}
  /\ pendingSource' = NoWindow
  /\ pendingTarget' = NoWindow
  /\ proposedWindows' = logicalWindows
  /\ proposedPrimary' = primaryWindow
  /\ proposedAssignment' = assignment
  /\ preparedNative' = NoWindow
  /\ UNCHANGED <<logicalWindows, primaryWindow, activeWindow, documentTabs,
       assignment, attached, rendererGeneration, closeIssued>>
  /\ Record("native-window-preparation-failed", pendingKind, pendingSource, pendingTabs)

PersistSucceeded ==
  /\ phase = "persisting"
  /\ phase' = "idle"
  /\ result' = "completed"
  /\ logicalWindows' = proposedWindows
  /\ primaryWindow' = proposedPrimary
  /\ activeWindow' = pendingTarget
  /\ assignment' = proposedAssignment
  /\ attached' = [window \in Windows |->
       IF window \notin proposedWindows
       THEN FALSE
       ELSE IF window = preparedNative THEN TRUE ELSE attached[window]]
  /\ closeIssued' = closeIssued \cup (logicalWindows \ proposedWindows)
  /\ pendingKind' = "none"
  /\ pendingTabs' = {}
  /\ pendingSource' = NoWindow
  /\ pendingTarget' = NoWindow
  /\ preparedNative' = NoWindow
  /\ UNCHANGED <<documentTabs, rendererGeneration, proposedWindows,
       proposedPrimary, proposedAssignment>>
  /\ Record("persist-succeeded", pendingKind, pendingSource, pendingTabs)

PersistFailed ==
  /\ phase = "persisting"
  /\ phase' = "idle"
  /\ result' = "failed"
  /\ pendingKind' = "none"
  /\ pendingTabs' = {}
  /\ pendingSource' = NoWindow
  /\ pendingTarget' = NoWindow
  /\ proposedWindows' = logicalWindows
  /\ proposedPrimary' = primaryWindow
  /\ proposedAssignment' = assignment
  /\ preparedNative' = NoWindow
  /\ UNCHANGED <<logicalWindows, primaryWindow, activeWindow, documentTabs,
       assignment, attached, rendererGeneration, closeIssued>>
  /\ Record("persist-failed", pendingKind, pendingSource, pendingTabs)

RequestClose(source, target) ==
  /\ phase = "idle"
  /\ source \in logicalWindows
  /\ target \in logicalWindows \ {source}
  /\ phase' = "persisting"
  /\ result' = "none"
  /\ pendingKind' = "close"
  /\ pendingTabs' = {tab \in documentTabs : assignment[tab] = source}
  /\ pendingSource' = source
  /\ pendingTarget' = target
  /\ proposedWindows' = logicalWindows \ {source}
  /\ proposedPrimary' = IF primaryWindow = source THEN target ELSE primaryWindow
  /\ proposedAssignment' = Reassign(assignment, pendingTabs', target)
  /\ preparedNative' = NoWindow
  /\ UNCHANGED <<logicalWindows, primaryWindow, activeWindow, documentTabs,
       assignment, attached, rendererGeneration, closeIssued>>
  /\ Record("request-close", "close", source, pendingTabs')

RetireRenderer(window) ==
  /\ window \in logicalWindows
  /\ attached[window]
  /\ rendererGeneration[window] < MaxGeneration
  /\ attached' = [attached EXCEPT ![window] = FALSE]
  /\ rendererGeneration' =
       [rendererGeneration EXCEPT ![window] = @ + 1]
  /\ UNCHANGED <<phase, result, logicalWindows, primaryWindow, activeWindow,
       documentTabs, assignment, closeIssued, pendingKind, pendingTabs,
       pendingSource, pendingTarget, proposedWindows, proposedPrimary,
       proposedAssignment, preparedNative>>
  /\ Record("retire-renderer", "none", window, {})

AttachRenderer(window) ==
  /\ window \in logicalWindows
  /\ ~attached[window]
  /\ attached' = [attached EXCEPT ![window] = TRUE]
  /\ UNCHANGED <<phase, result, logicalWindows, primaryWindow, activeWindow,
       documentTabs, assignment, rendererGeneration, closeIssued, pendingKind,
       pendingTabs, pendingSource, pendingTarget, proposedWindows,
       proposedPrimary, proposedAssignment, preparedNative>>
  /\ Record("attach-renderer", "none", window, {})

IgnoreStaleCompletion ==
  /\ phase = "idle"
  /\ result # "none"
  /\ UNCHANGED <<phase, result, logicalWindows, primaryWindow, activeWindow,
       documentTabs, assignment, attached, rendererGeneration, closeIssued,
       pendingKind, pendingTabs, pendingSource, pendingTarget, proposedWindows,
       proposedPrimary, proposedAssignment, preparedNative>>
  /\ Record("ignore-stale-completion", "none", NoWindow, {})

ResolveProvision == TerminalProvisioned \/ TerminalProvisioningFailed
ResolveNative == NativeWindowPrepared \/ NativeWindowPreparationFailed
ResolvePersistence == PersistSucceeded \/ PersistFailed

Next ==
  \/ \E target \in Windows: FocusWindow(target)
  \/ \E tab \in Tabs: ReconcileAdd(tab)
  \/ \E tab \in Tabs: ReconcileRemove(tab)
  \/ \E movedTabs \in SUBSET Tabs, target \in Windows:
       RequestMove(movedTabs, target)
  \/ \E movedTabs \in SUBSET Tabs, target \in Windows:
       RequestMoveNewWindow(movedTabs, target)
  \/ \E source \in Windows, target \in Windows, tab \in Tabs:
       RequestNewWindow(source, target, tab)
  \/ TerminalProvisioned
  \/ TerminalProvisioningFailed
  \/ NativeWindowPrepared
  \/ NativeWindowPreparationFailed
  \/ PersistSucceeded
  \/ PersistFailed
  \/ \E source \in Windows, target \in Windows: RequestClose(source, target)
  \/ \E window \in Windows: RetireRenderer(window)
  \/ \E window \in Windows: AttachRenderer(window)
  \/ IgnoreStaleCompletion

TypeOK ==
  /\ phase \in Phases
  /\ result \in Results
  /\ logicalWindows \in (SUBSET Windows) \ {{}}
  /\ primaryWindow \in logicalWindows
  /\ activeWindow \in logicalWindows
  /\ documentTabs \subseteq Tabs
  /\ assignment \in [Tabs -> WindowTargets]
  /\ attached \in [Windows -> BOOLEAN]
  /\ rendererGeneration \in [Windows -> 0..MaxGeneration]
  /\ closeIssued \subseteq Windows
  /\ pendingKind \in Kinds
  /\ pendingTabs \subseteq Tabs
  /\ pendingSource \in WindowTargets
  /\ pendingTarget \in WindowTargets
  /\ proposedWindows \subseteq Windows
  /\ proposedPrimary \in Windows
  /\ proposedAssignment \in [Tabs -> WindowTargets]
  /\ preparedNative \in WindowTargets
  /\ lastKind \in Kinds
  /\ lastSource \in WindowTargets
  /\ lastTabs \subseteq Tabs
  /\ previousAssignment \in [Tabs -> WindowTargets]
  /\ previousWindows \subseteq Windows
  /\ previousAttached \in [Windows -> BOOLEAN]

AssignmentsPartitionCurrentTabs ==
  /\ \A tab \in documentTabs: assignment[tab] \in logicalWindows
  /\ \A tab \in Tabs \ documentTabs: assignment[tab] = NoWindow

AttachmentsBelongToLogicalWindows ==
  \A window \in Windows \ logicalWindows: ~attached[window]

FailedPreparationPreservesSource ==
  lastAction \in {"terminal-provisioning-failed",
      "native-window-preparation-failed", "persist-failed"}
    => /\ assignment = previousAssignment
       /\ logicalWindows = previousWindows

NewWindowFailureRetainsProvisionedTab ==
  (lastKind = "create-window"
    /\ lastAction \in {"native-window-preparation-failed", "persist-failed"})
    => \A tab \in lastTabs: assignment[tab] = lastSource

NonLastCloseRehomesBeforeEffect ==
  (lastAction = "persist-succeeded"
    /\ lastKind = "close")
    => /\ lastSource \in closeIssued
       /\ lastSource \notin logicalWindows
       /\ \A tab \in documentTabs: assignment[tab] # lastSource

ClosedWindowHasNoAssignment ==
  \A window \in closeIssued:
    /\ window \notin logicalWindows
    /\ \A tab \in documentTabs: assignment[tab] # window

RendererRetirementIsWindowScoped ==
  lastAction = "retire-renderer"
    => /\ \A window \in Windows \ {lastSource}:
            attached[window] = previousAttached[window]
       /\ assignment = previousAssignment
       /\ logicalWindows = previousWindows

PreparedOperationEventuallySettles ==
  phase # "idle" ~> phase = "idle"

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(ResolveProvision)
  /\ WF_vars(ResolveNative)
  /\ WF_vars(ResolvePersistence)

=============================================================================
