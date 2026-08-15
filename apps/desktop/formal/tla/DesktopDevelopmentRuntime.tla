-------------------- MODULE DesktopDevelopmentRuntime --------------------
EXTENDS Naturals, TLC

CONSTANT MaxGeneration

Generations == 0..MaxGeneration
NoGeneration == MaxGeneration + 1
ElectronStates == {"running", "stopped"}
DocumentStates == {"empty", "retain", "provision"}
PlanStates == {"absent", "pending", "ready"}
Phases == {
  "idle",
  "preparing-host-restart",
  "stopping-electron-for-desktop",
  "stopping-electron-for-host",
  "stopping-host",
  "starting-host",
  "starting-electron-for-desktop",
  "starting-electron-for-host",
  "closed"
}
Actions == {
  "init",
  "desktop-build-failed",
  "desktop-build-succeeded",
  "host-build-failed",
  "host-build-succeeded",
  "begin-desktop-restart",
  "begin-host-restart",
  "host-restart-aborted",
  "host-restart-prepared",
  "electron-stopped-for-desktop",
  "electron-stopped-for-host",
  "host-stopped",
  "host-started",
  "electron-started-for-desktop",
  "electron-started-for-host",
  "host-restart-completed",
  "close"
}

VARIABLES
  activeHost,
  availableHost,
  blockedHost,
  targetHost,
  activeDesktop,
  availableDesktop,
  electron,
  hostHasTerminal,
  documentState,
  restartPlan,
  phase,
  closed,
  desktopBuildFailed,
  hostBuildFailed,
  restartPreparationAvailable,
  lastAction,
  previousActiveHost,
  previousElectron,
  previousHostHasTerminal,
  previousDocumentState,
  previousRestartPlan,
  previousTargetHost

vars == <<
  activeHost,
  availableHost,
  blockedHost,
  targetHost,
  activeDesktop,
  availableDesktop,
  electron,
  hostHasTerminal,
  documentState,
  restartPlan,
  phase,
  closed,
  desktopBuildFailed,
  hostBuildFailed,
  restartPreparationAvailable,
  lastAction,
  previousActiveHost,
  previousElectron,
  previousHostHasTerminal,
  previousDocumentState,
  previousRestartPlan,
  previousTargetHost
>>

Record(action) ==
  /\ lastAction' = action
  /\ previousActiveHost' = activeHost
  /\ previousElectron' = electron
  /\ previousHostHasTerminal' = hostHasTerminal
  /\ previousDocumentState' = documentState
  /\ previousRestartPlan' = restartPlan
  /\ previousTargetHost' = targetHost

Init ==
  /\ activeHost = 0
  /\ availableHost = 0
  /\ blockedHost = 0
  /\ targetHost = NoGeneration
  /\ activeDesktop = 0
  /\ availableDesktop = 0
  /\ electron = "running"
  /\ hostHasTerminal \in BOOLEAN
  /\ documentState = IF hostHasTerminal THEN "retain" ELSE "empty"
  /\ restartPlan = "absent"
  /\ phase = "idle"
  /\ closed = FALSE
  /\ desktopBuildFailed = FALSE
  /\ hostBuildFailed = FALSE
  /\ restartPreparationAvailable \in BOOLEAN
  /\ lastAction = "init"
  /\ previousActiveHost = activeHost
  /\ previousElectron = electron
  /\ previousHostHasTerminal = hostHasTerminal
  /\ previousDocumentState = documentState
  /\ previousRestartPlan = restartPlan
  /\ previousTargetHost = targetHost

DesktopBuildFails ==
  /\ ~closed
  /\ ~desktopBuildFailed
  /\ desktopBuildFailed' = TRUE
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, targetHost, activeDesktop,
       availableDesktop, electron, hostHasTerminal, documentState,
       restartPlan, phase, closed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("desktop-build-failed")

DesktopBuildSucceeds ==
  /\ ~closed
  /\ availableDesktop < MaxGeneration
  /\ availableDesktop' = availableDesktop + 1
  /\ desktopBuildFailed' = FALSE
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, targetHost, activeDesktop, electron,
       hostHasTerminal, documentState, restartPlan, phase, closed,
       hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("desktop-build-succeeded")

HostBuildFails ==
  /\ ~closed
  /\ ~hostBuildFailed
  /\ hostBuildFailed' = TRUE
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, targetHost, activeDesktop,
       availableDesktop, electron, hostHasTerminal, documentState,
       restartPlan, phase, closed, desktopBuildFailed, restartPreparationAvailable
     >>
  /\ Record("host-build-failed")

HostBuildSucceeds ==
  /\ ~closed
  /\ availableHost < MaxGeneration
  /\ availableHost' = availableHost + 1
  /\ hostBuildFailed' = FALSE
  /\ UNCHANGED <<
       activeHost, blockedHost, targetHost, activeDesktop, availableDesktop, electron,
       hostHasTerminal, documentState, restartPlan, phase, closed,
       desktopBuildFailed, restartPreparationAvailable
     >>
  /\ Record("host-build-succeeded")

BeginDesktopRestart ==
  /\ ~closed
  /\ phase = "idle"
  /\ ~(availableHost > activeHost /\ availableHost > blockedHost)
  /\ availableDesktop > activeDesktop
  /\ electron = "running"
  /\ phase' = "stopping-electron-for-desktop"
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, targetHost, activeDesktop,
       availableDesktop, electron, hostHasTerminal, documentState,
       restartPlan, closed, desktopBuildFailed, hostBuildFailed,
       restartPreparationAvailable
     >>
  /\ Record("begin-desktop-restart")

ElectronStopsForDesktop ==
  /\ phase = "stopping-electron-for-desktop"
  /\ electron = "running"
  /\ electron' = "stopped"
  /\ phase' = "starting-electron-for-desktop"
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, targetHost, activeDesktop,
       availableDesktop, hostHasTerminal, documentState, restartPlan,
       closed, desktopBuildFailed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("electron-stopped-for-desktop")

ElectronStartsForDesktop ==
  /\ phase = "starting-electron-for-desktop"
  /\ electron = "stopped"
  /\ electron' = "running"
  /\ activeDesktop' = availableDesktop
  /\ phase' = "idle"
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, targetHost, availableDesktop,
       hostHasTerminal, documentState, restartPlan, closed,
       desktopBuildFailed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("electron-started-for-desktop")

BeginHostRestart ==
  /\ ~closed
  /\ phase = "idle"
  /\ availableHost > activeHost
  /\ availableHost > blockedHost
  /\ electron = "running"
  /\ restartPlan = "absent"
  /\ targetHost' = availableHost
  /\ restartPlan' = "pending"
  /\ phase' = "preparing-host-restart"
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, activeDesktop, availableDesktop,
       electron, hostHasTerminal, documentState, closed,
       desktopBuildFailed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("begin-host-restart")

PrepareHostRestart ==
  /\ phase = "preparing-host-restart"
  /\ restartPlan = "pending"
  /\ restartPreparationAvailable
  /\ electron = "running"
  /\ documentState \in {"empty", "retain"}
  /\ hostHasTerminal = (documentState = "retain")
  /\ documentState' = IF hostHasTerminal THEN "provision" ELSE "empty"
  /\ hostHasTerminal' = FALSE
  /\ restartPlan' = "ready"
  /\ phase' = "stopping-electron-for-host"
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, targetHost, activeDesktop,
       availableDesktop, electron, closed, desktopBuildFailed,
       hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("host-restart-prepared")

AbortHostRestart ==
  /\ phase = "preparing-host-restart"
  /\ restartPlan = "pending"
  /\ ~restartPreparationAvailable
  /\ blockedHost' = targetHost
  /\ targetHost' = NoGeneration
  /\ restartPlan' = "absent"
  /\ phase' = "idle"
  /\ UNCHANGED <<
       activeHost, availableHost, activeDesktop, availableDesktop,
       electron, hostHasTerminal, documentState, closed,
       desktopBuildFailed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("host-restart-aborted")

ElectronStopsForHost ==
  /\ phase = "stopping-electron-for-host"
  /\ restartPlan = "ready"
  /\ electron = "running"
  /\ electron' = "stopped"
  /\ phase' = "stopping-host"
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, targetHost, activeDesktop,
       availableDesktop, hostHasTerminal, documentState, restartPlan,
       closed, desktopBuildFailed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("electron-stopped-for-host")

HostStops ==
  /\ phase = "stopping-host"
  /\ electron = "stopped"
  /\ restartPlan = "ready"
  /\ ~hostHasTerminal
  /\ activeHost # NoGeneration
  /\ activeHost' = NoGeneration
  /\ phase' = "starting-host"
  /\ UNCHANGED <<
       availableHost, blockedHost, targetHost, activeDesktop, availableDesktop,
       electron, hostHasTerminal, documentState, restartPlan, closed,
       desktopBuildFailed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("host-stopped")

HostStarts ==
  /\ phase = "starting-host"
  /\ activeHost = NoGeneration
  /\ targetHost \in Generations
  /\ targetHost <= availableHost
  /\ activeHost' = targetHost
  /\ phase' = "starting-electron-for-host"
  /\ UNCHANGED <<
       availableHost, blockedHost, targetHost, activeDesktop, availableDesktop,
       electron, hostHasTerminal, documentState, restartPlan, closed,
       desktopBuildFailed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("host-started")

ElectronStartsForHost ==
  /\ phase = "starting-electron-for-host"
  /\ activeHost \in Generations
  /\ electron = "stopped"
  /\ electron' = "running"
  /\ activeDesktop' = availableDesktop
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, targetHost, availableDesktop,
       hostHasTerminal, documentState, restartPlan, phase, closed,
       desktopBuildFailed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("electron-started-for-host")

CompleteHostRestart ==
  /\ phase = "starting-electron-for-host"
  /\ electron = "running"
  /\ restartPlan = "ready"
  /\ documentState \in {"empty", "provision"}
  /\ hostHasTerminal' = (documentState = "provision")
  /\ documentState' = IF documentState = "provision" THEN "retain" ELSE "empty"
  /\ restartPlan' = "absent"
  /\ targetHost' = NoGeneration
  /\ phase' = "idle"
  /\ UNCHANGED <<
       activeHost, availableHost, blockedHost, activeDesktop, availableDesktop,
       electron, closed, desktopBuildFailed, hostBuildFailed,
       restartPreparationAvailable
     >>
  /\ Record("host-restart-completed")

CloseDevelopment ==
  /\ ~closed
  /\ closed' = TRUE
  /\ activeHost' = NoGeneration
  /\ targetHost' = NoGeneration
  /\ electron' = "stopped"
  /\ hostHasTerminal' = FALSE
  /\ restartPlan' = "absent"
  /\ phase' = "closed"
  /\ UNCHANGED <<
       availableHost, blockedHost, activeDesktop, availableDesktop, documentState,
       desktopBuildFailed, hostBuildFailed, restartPreparationAvailable
     >>
  /\ Record("close")

Next ==
  \/ DesktopBuildFails
  \/ DesktopBuildSucceeds
  \/ HostBuildFails
  \/ HostBuildSucceeds
  \/ BeginDesktopRestart
  \/ ElectronStopsForDesktop
  \/ ElectronStartsForDesktop
  \/ BeginHostRestart
  \/ PrepareHostRestart
  \/ AbortHostRestart
  \/ ElectronStopsForHost
  \/ HostStops
  \/ HostStarts
  \/ ElectronStartsForHost
  \/ CompleteHostRestart
  \/ CloseDevelopment

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(BeginDesktopRestart)
  /\ WF_vars(ElectronStopsForDesktop)
  /\ WF_vars(ElectronStartsForDesktop)
  /\ WF_vars(BeginHostRestart)
  /\ WF_vars(PrepareHostRestart)
  /\ WF_vars(AbortHostRestart)
  /\ WF_vars(ElectronStopsForHost)
  /\ WF_vars(HostStops)
  /\ WF_vars(HostStarts)
  /\ WF_vars(ElectronStartsForHost)
  /\ WF_vars(CompleteHostRestart)

TypeOK ==
  /\ activeHost \in Generations \cup {NoGeneration}
  /\ availableHost \in Generations
  /\ blockedHost \in Generations
  /\ targetHost \in Generations \cup {NoGeneration}
  /\ activeDesktop \in Generations
  /\ availableDesktop \in Generations
  /\ electron \in ElectronStates
  /\ hostHasTerminal \in BOOLEAN
  /\ documentState \in DocumentStates
  /\ restartPlan \in PlanStates
  /\ phase \in Phases
  /\ closed \in BOOLEAN
  /\ desktopBuildFailed \in BOOLEAN
  /\ hostBuildFailed \in BOOLEAN
  /\ restartPreparationAvailable \in BOOLEAN
  /\ lastAction \in Actions
  /\ previousActiveHost \in Generations \cup {NoGeneration}
  /\ previousElectron \in ElectronStates
  /\ previousHostHasTerminal \in BOOLEAN
  /\ previousDocumentState \in DocumentStates
  /\ previousRestartPlan \in PlanStates
  /\ previousTargetHost \in Generations \cup {NoGeneration}

ActiveHostWasBuilt ==
  activeHost = NoGeneration \/ activeHost <= availableHost

RunningElectronRequiresHost ==
  electron = "running" => activeHost # NoGeneration

ReadyPlanHasDurableRestart ==
  restartPlan = "ready" =>
    /\ ~hostHasTerminal
    /\ documentState \in {"empty", "provision"}

HostStopRequiresReadyPlan ==
  lastAction = "host-stopped" =>
    /\ previousRestartPlan = "ready"
    /\ ~previousHostHasTerminal

HostActivationUsesPreparedTarget ==
  lastAction = "host-started" =>
    /\ activeHost = previousTargetHost
    /\ activeHost <= availableHost

DesktopRestartPreservesHost ==
  lastAction \in {
    "begin-desktop-restart",
    "electron-stopped-for-desktop",
    "electron-started-for-desktop"
  } =>
    /\ activeHost = previousActiveHost
    /\ hostHasTerminal = previousHostHasTerminal
    /\ documentState = previousDocumentState
    /\ restartPlan = previousRestartPlan

FailedBuildPreservesRuntime ==
  lastAction \in {"desktop-build-failed", "host-build-failed"} =>
    /\ activeHost = previousActiveHost
    /\ electron = previousElectron
    /\ hostHasTerminal = previousHostHasTerminal
    /\ documentState = previousDocumentState
    /\ restartPlan = previousRestartPlan

AbortedHostRestartPreservesRuntime ==
  lastAction = "host-restart-aborted" =>
    /\ activeHost = previousActiveHost
    /\ electron = previousElectron
    /\ hostHasTerminal = previousHostHasTerminal
    /\ documentState = previousDocumentState

ClosedOwnsNoProcesses ==
  closed =>
    /\ phase = "closed"
    /\ activeHost = NoGeneration
    /\ electron = "stopped"
    /\ ~hostHasTerminal

SuccessfulHostBuildEventuallyActivates ==
  []((~closed /\ restartPreparationAvailable /\ availableHost > activeHost) ~>
    (closed \/ activeHost = availableHost))

SuccessfulDesktopBuildEventuallyActivates ==
  []((~closed /\ availableDesktop > activeDesktop) ~>
    (closed \/ activeDesktop = availableDesktop))

=============================================================================
