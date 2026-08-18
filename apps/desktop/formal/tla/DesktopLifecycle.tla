------------------------- MODULE DesktopLifecycle -------------------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

\* Scenario-isolated suite for seven independent Desktop-owned lifecycles.
\* Each variant retains the owning transition vocabulary and properties while
\* the shared artifact, tool configuration, coverage gate, and trigger mapping
\* prevent small lifecycle checks from drifting into unregistered islands.

CONSTANTS Scenario, MaxGeneration, MaxCaptures, Terminals

LifecycleScenarios == {
  "application-lifetime", "browser-access", "development-runtime",
  "project-folder-acquisition", "renderer-document", "terminal-binding-close",
  "window-state"
}

\* application-lifetime
AppResources == {"referenced", "unreferenced"}
AppModes == {"none", "user-preserve", "user-stop", "update-restart",
  "development-host-restart"}
AppDestructiveModes == {"user-stop", "development-host-restart"}
AppPhases == {"idle", "awaiting-choice", "flushing", "persisting-stop",
  "stopping", "staging-restart", "preparing-plan", "failed", "ready",
  "quitting"}
AppActions == {"init", "request-user-protected", "request-user-unprotected",
  "request-update", "request-development",
  "choose-preserve", "choose-stop", "cancel", "flush", "flush-failed",
  "persist-stop", "persist-failed", "dispatch-close", "close-completed",
  "close-failed", "retry", "orphan-detected", "finish-stop",
  "finish-development-stop", "stage-restart", "prepare-plan",
  "prepare-plan-failed", "quit", "quit-anyway"}

VARIABLES
  appMode,
  appPhase,
  appPrompted,
  appTopologyDurable,
  appRestartProvisionDurable,
  appRestartPlanReady,
  appBindings,
  appCloseIssued,
  appHostOrphan,
  appEffectsStarted,
  appForcedQuit,
  appQuit,
  appWritable,
  appPlanWritable,
  appLastAction,
  appPreviousBindings,
  appPreviousEffectsStarted

Appvars == <<appMode, appPhase, appPrompted, appTopologyDurable, appRestartProvisionDurable,
  appRestartPlanReady, appBindings, appCloseIssued, appHostOrphan, appEffectsStarted,
  appForcedQuit, appQuit, appWritable, appPlanWritable, appLastAction, appPreviousBindings,
  appPreviousEffectsStarted>>

AppRecord(action) ==
  /\ appLastAction' = action
  /\ appPreviousBindings' = appBindings
  /\ appPreviousEffectsStarted' = appEffectsStarted

AppInit ==
  /\ appMode = "none"
  /\ appPhase = "idle"
  /\ appPrompted = FALSE
  /\ appTopologyDurable = FALSE
  /\ appRestartProvisionDurable = FALSE
  /\ appRestartPlanReady = FALSE
  /\ appBindings = [resource \in AppResources |-> TRUE]
  /\ appCloseIssued = [resource \in AppResources |-> FALSE]
  /\ appHostOrphan \in (IF Scenario = "application-lifetime" THEN BOOLEAN ELSE {FALSE})
  /\ appEffectsStarted = FALSE
  /\ appForcedQuit = FALSE
  /\ appQuit = FALSE
  /\ appWritable \in (IF Scenario = "application-lifetime" THEN BOOLEAN ELSE {FALSE})
  /\ appPlanWritable \in (IF Scenario = "application-lifetime" THEN BOOLEAN ELSE {FALSE})
  /\ appLastAction = "init"
  /\ appPreviousBindings = appBindings
  /\ appPreviousEffectsStarted = appEffectsStarted

\* These two actions model the zero/nonzero result of the Workbench-owned
\* close-confirmation count sampled for one user-quit request.
AppRequestProtectedUserQuitBase ==
  /\ appPhase = "idle"
  /\ appMode' = "user-preserve"
  /\ appPhase' = "awaiting-choice"
  /\ appPrompted' = TRUE
  /\ UNCHANGED <<appTopologyDurable, appRestartProvisionDurable, appRestartPlanReady,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("request-user-protected")

AppRequestUnprotectedUserQuitBase ==
  /\ appPhase = "idle"
  /\ appMode' = "user-preserve"
  /\ appPhase' = "flushing"
  /\ appPrompted' = FALSE
  /\ UNCHANGED <<appTopologyDurable, appRestartProvisionDurable, appRestartPlanReady,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("request-user-unprotected")

AppRequestUpdateRestartBase ==
  /\ appPhase = "idle"
  /\ appMode' = "update-restart"
  /\ appPhase' = "flushing"
  /\ appPrompted' = FALSE
  /\ UNCHANGED <<appTopologyDurable, appRestartProvisionDurable, appRestartPlanReady,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("request-update")

AppRequestDevelopmentHostRestartBase ==
  /\ appPhase = "idle"
  /\ appMode' = "development-host-restart"
  /\ appPhase' = "persisting-stop"
  /\ appPrompted' = FALSE
  /\ UNCHANGED <<appTopologyDurable, appRestartProvisionDurable, appRestartPlanReady,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("request-development")

AppChoosePreserveBase ==
  /\ appPhase = "awaiting-choice"
  /\ appMode' = "user-preserve"
  /\ appPhase' = "flushing"
  /\ appPrompted' = FALSE
  /\ UNCHANGED <<appTopologyDurable, appRestartProvisionDurable, appRestartPlanReady,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("choose-preserve")

AppChooseStopBase ==
  /\ appPhase = "awaiting-choice"
  /\ appMode' = "user-stop"
  /\ appPhase' = "persisting-stop"
  /\ appPrompted' = FALSE
  /\ UNCHANGED <<appTopologyDurable, appRestartProvisionDurable, appRestartPlanReady,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("choose-stop")

AppCancelBase ==
  /\ appPhase = "awaiting-choice"
  /\ ~appEffectsStarted
  /\ appMode' = "none"
  /\ appPhase' = "idle"
  /\ appPrompted' = FALSE
  /\ UNCHANGED <<appTopologyDurable, appRestartProvisionDurable, appRestartPlanReady,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("cancel")

AppFlushBase ==
  /\ appPhase = "flushing"
  /\ appWritable
  /\ appTopologyDurable' = TRUE
  /\ appPhase' = "ready"
  /\ UNCHANGED <<appMode, appPrompted, appRestartProvisionDurable, appRestartPlanReady,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("flush")

AppFlushFailsBase ==
  /\ appPhase = "flushing"
  /\ ~appWritable
  /\ appPhase' = "failed"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appRestartPlanReady, appBindings, appCloseIssued, appHostOrphan, appEffectsStarted,
       appForcedQuit, appQuit, appWritable, appPlanWritable>>
  /\ AppRecord("flush-failed")

AppPersistStopBase ==
  /\ appPhase = "persisting-stop"
  /\ appWritable
  /\ appTopologyDurable' = TRUE
  /\ appPhase' = "stopping"
  /\ UNCHANGED <<appMode, appPrompted, appRestartProvisionDurable, appRestartPlanReady,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("persist-stop")

AppPersistStopFailsBase ==
  /\ appPhase = "persisting-stop"
  /\ ~appWritable
  /\ appPhase' = "failed"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appRestartPlanReady, appBindings, appCloseIssued, appHostOrphan, appEffectsStarted,
       appForcedQuit, appQuit, appWritable, appPlanWritable>>
  /\ AppRecord("persist-failed")

AppDispatchCloseBase(resource) ==
  /\ appMode \in AppDestructiveModes
  /\ appPhase = "stopping"
  /\ appTopologyDurable
  /\ appBindings[resource]
  /\ ~appCloseIssued[resource]
  /\ appCloseIssued' = [appCloseIssued EXCEPT ![resource] = TRUE]
  /\ appEffectsStarted' = TRUE
  /\ UNCHANGED <<appMode, appPhase, appPrompted, appTopologyDurable,
       appRestartProvisionDurable, appRestartPlanReady, appBindings, appHostOrphan,
       appForcedQuit, appQuit, appWritable, appPlanWritable>>
  /\ AppRecord("dispatch-close")

AppCloseCompletesBase(resource) ==
  /\ appMode \in AppDestructiveModes
  /\ appPhase = "stopping"
  /\ appCloseIssued[resource]
  /\ appBindings' = [appBindings EXCEPT ![resource] = FALSE]
  /\ appCloseIssued' = [appCloseIssued EXCEPT ![resource] = FALSE]
  /\ UNCHANGED <<appMode, appPhase, appPrompted, appTopologyDurable,
       appRestartProvisionDurable, appRestartPlanReady, appHostOrphan, appEffectsStarted,
       appForcedQuit, appQuit, appWritable, appPlanWritable>>
  /\ AppRecord("close-completed")

AppCloseFailsBase(resource) ==
  /\ appMode \in AppDestructiveModes
  /\ appPhase = "stopping"
  /\ appCloseIssued[resource]
  /\ appCloseIssued' = [appCloseIssued EXCEPT ![resource] = FALSE]
  /\ appPhase' = "failed"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appRestartPlanReady, appBindings, appHostOrphan, appEffectsStarted, appForcedQuit,
       appQuit, appWritable, appPlanWritable>>
  /\ AppRecord("close-failed")

AppRetryBase ==
  /\ appMode \in AppDestructiveModes
  /\ appPhase = "failed"
  /\ appTopologyDurable
  /\ appRestartPlanReady' = FALSE
  /\ appPhase' = "stopping"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("retry")

AppDetectOrphanBase ==
  /\ appMode \in AppDestructiveModes
  /\ appPhase = "stopping"
  /\ \A resource \in AppResources: ~appBindings[resource]
  /\ appHostOrphan
  /\ appPhase' = "failed"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appRestartPlanReady, appBindings, appCloseIssued, appHostOrphan, appEffectsStarted,
       appForcedQuit, appQuit, appWritable, appPlanWritable>>
  /\ AppRecord("orphan-detected")

AppFinishStopBase ==
  /\ appMode = "user-stop"
  /\ appPhase = "stopping"
  /\ \A resource \in AppResources: ~appBindings[resource]
  /\ ~appHostOrphan
  /\ appPhase' = "ready"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appRestartPlanReady, appBindings, appCloseIssued, appHostOrphan, appEffectsStarted,
       appForcedQuit, appQuit, appWritable, appPlanWritable>>
  /\ AppRecord("finish-stop")

AppFinishDevelopmentStopBase ==
  /\ appMode = "development-host-restart"
  /\ appPhase = "stopping"
  /\ \A resource \in AppResources: ~appBindings[resource]
  /\ ~appHostOrphan
  /\ appPhase' = "staging-restart"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appRestartPlanReady, appBindings, appCloseIssued, appHostOrphan, appEffectsStarted,
       appForcedQuit, appQuit, appWritable, appPlanWritable>>
  /\ AppRecord("finish-development-stop")

AppStageDevelopmentRestartBase ==
  /\ appMode = "development-host-restart"
  /\ appPhase = "staging-restart"
  /\ appWritable
  /\ appRestartProvisionDurable' = TRUE
  /\ appPhase' = "preparing-plan"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartPlanReady, appBindings,
       appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit, appWritable,
       appPlanWritable>>
  /\ AppRecord("stage-restart")

AppPrepareDevelopmentPlanBase ==
  /\ appMode = "development-host-restart"
  /\ appPhase = "preparing-plan"
  /\ appRestartProvisionDurable
  /\ appPlanWritable
  /\ appRestartPlanReady' = TRUE
  /\ appPhase' = "ready"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appBindings, appCloseIssued, appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
       appWritable, appPlanWritable>>
  /\ AppRecord("prepare-plan")

AppPrepareDevelopmentPlanFailsBase ==
  /\ appMode = "development-host-restart"
  /\ appPhase = "preparing-plan"
  /\ ~appPlanWritable
  /\ appPhase' = "failed"
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appRestartPlanReady, appBindings, appCloseIssued, appHostOrphan, appEffectsStarted,
       appForcedQuit, appQuit, appWritable, appPlanWritable>>
  /\ AppRecord("prepare-plan-failed")

AppQuitBase ==
  /\ appPhase = "ready"
  /\ appPhase' = "quitting"
  /\ appQuit' = TRUE
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appRestartPlanReady, appBindings, appCloseIssued, appHostOrphan, appEffectsStarted,
       appForcedQuit, appWritable, appPlanWritable>>
  /\ AppRecord("quit")

AppQuitAnywayBase ==
  /\ appPhase = "failed"
  /\ appPhase' = "quitting"
  /\ appForcedQuit' = TRUE
  /\ appQuit' = TRUE
  /\ UNCHANGED <<appMode, appPrompted, appTopologyDurable, appRestartProvisionDurable,
       appRestartPlanReady, appBindings, appCloseIssued, appHostOrphan, appEffectsStarted,
       appWritable, appPlanWritable>>
  /\ AppRecord("quit-anyway")

\* browser-access
BrowserModes == {"none", "loopback", "local-network"}
BrowserPhases == {
  "stopped",
  "server-starting",
  "gateway-starting",
  "ready",
  "stopping",
  "disposed"
}

VARIABLES
  browserPhase,
  browserActiveMode,
  browserDesiredMode,
  browserServerOwned,
  browserGatewayOwned,
  browserDisposeRequested,
  browserLastAction

Browservars == <<
  browserPhase,
  browserActiveMode,
  browserDesiredMode,
  browserServerOwned,
  browserGatewayOwned,
  browserDisposeRequested,
  browserLastAction
>>

BrowserInit ==
  /\ browserPhase = "stopped"
  /\ browserActiveMode = "none"
  /\ browserDesiredMode = "none"
  /\ browserServerOwned = FALSE
  /\ browserGatewayOwned = FALSE
  /\ browserDisposeRequested = FALSE
  /\ browserLastAction = "init"

BrowserRequestStartBase(mode) ==
  /\ mode \in BrowserModes \ {"none"}
  /\ ~browserDisposeRequested
  /\ browserDesiredMode' = mode
  /\ browserLastAction' = "request-start"
  /\ UNCHANGED <<browserPhase, browserActiveMode, browserServerOwned, browserGatewayOwned, browserDisposeRequested>>

BrowserRequestStopBase ==
  /\ ~browserDisposeRequested
  /\ browserDesiredMode' = "none"
  /\ browserLastAction' = "request-stop"
  /\ UNCHANGED <<browserPhase, browserActiveMode, browserServerOwned, browserGatewayOwned, browserDisposeRequested>>

BrowserRequestDisposeBase ==
  /\ ~browserDisposeRequested
  /\ browserDisposeRequested' = TRUE
  /\ browserDesiredMode' = "none"
  /\ browserLastAction' = "request-dispose"
  /\ UNCHANGED <<browserPhase, browserActiveMode, browserServerOwned, browserGatewayOwned>>

BrowserBeginStartBase ==
  /\ browserPhase = "stopped"
  /\ browserDesiredMode \in BrowserModes \ {"none"}
  /\ ~browserDisposeRequested
  /\ browserPhase' = "server-starting"
  /\ browserActiveMode' = browserDesiredMode
  /\ browserLastAction' = "begin-start"
  /\ UNCHANGED <<browserDesiredMode, browserServerOwned, browserGatewayOwned, browserDisposeRequested>>

BrowserServerStartedBase ==
  /\ browserPhase = "server-starting"
  /\ browserServerOwned' = TRUE
  /\ browserPhase' = IF browserDisposeRequested \/ browserDesiredMode # browserActiveMode
                THEN "stopping"
                ELSE "gateway-starting"
  /\ browserLastAction' = "server-started"
  /\ UNCHANGED <<browserActiveMode, browserDesiredMode, browserGatewayOwned, browserDisposeRequested>>

BrowserGatewayStartedBase ==
  /\ browserPhase = "gateway-starting"
  /\ browserServerOwned
  /\ browserGatewayOwned' = TRUE
  /\ browserPhase' = IF browserDisposeRequested \/ browserDesiredMode # browserActiveMode
                THEN "stopping"
                ELSE "ready"
  /\ browserLastAction' = "gateway-started"
  /\ UNCHANGED <<browserActiveMode, browserDesiredMode, browserServerOwned, browserDisposeRequested>>

BrowserStartFailedBase ==
  /\ browserPhase \in {"server-starting", "gateway-starting"}
  /\ browserPhase' = "stopping"
  /\ browserLastAction' = "start-failed"
  /\ UNCHANGED <<browserActiveMode, browserDesiredMode, browserServerOwned, browserGatewayOwned, browserDisposeRequested>>

BrowserBeginStopBase ==
  /\ browserPhase = "ready"
  /\ (browserDisposeRequested \/ browserDesiredMode # browserActiveMode)
  /\ browserPhase' = "stopping"
  /\ browserLastAction' = "begin-stop"
  /\ UNCHANGED <<browserActiveMode, browserDesiredMode, browserServerOwned, browserGatewayOwned, browserDisposeRequested>>

BrowserDisposeStoppedBase ==
  /\ browserPhase = "stopped"
  /\ browserDisposeRequested
  /\ browserPhase' = "disposed"
  /\ browserActiveMode' = "none"
  /\ browserLastAction' = "dispose-stopped"
  /\ UNCHANGED <<browserDesiredMode, browserServerOwned, browserGatewayOwned, browserDisposeRequested>>

BrowserCleanupCompletedBase ==
  /\ browserPhase = "stopping"
  /\ browserPhase' = IF browserDisposeRequested THEN "disposed" ELSE "stopped"
  /\ browserActiveMode' = "none"
  /\ browserServerOwned' = FALSE
  /\ browserGatewayOwned' = FALSE
  /\ browserLastAction' = "cleanup-completed"
  /\ UNCHANGED <<browserDesiredMode, browserDisposeRequested>>

\* development-runtime
DevelopmentGenerations == 0..MaxGeneration
DevelopmentNoGeneration == MaxGeneration + 1
DevelopmentElectronStates == {"running", "stopped"}
DevelopmentDocumentStates == {"empty", "retain", "provision"}
DevelopmentPlanStates == {"absent", "pending", "ready"}
DevelopmentPhases == {
  "idle",
  "preparing-host-restart",
  "stopping-developmentElectron-for-desktop",
  "stopping-developmentElectron-for-host",
  "stopping-host",
  "starting-host",
  "starting-developmentElectron-for-desktop",
  "starting-developmentElectron-for-host",
  "closed"
}
DevelopmentActions == {
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
  developmentActiveHost,
  developmentAvailableHost,
  developmentBlockedHost,
  developmentTargetHost,
  developmentActiveDesktop,
  developmentAvailableDesktop,
  developmentElectron,
  developmentHostHasTerminal,
  developmentDocumentState,
  developmentRestartPlan,
  developmentPhase,
  developmentClosed,
  developmentDesktopBuildFailed,
  developmentHostBuildFailed,
  developmentRestartPreparationAvailable,
  developmentLastAction,
  developmentPreviousActiveHost,
  developmentPreviousElectron,
  developmentPreviousHostHasTerminal,
  developmentPreviousDocumentState,
  developmentPreviousRestartPlan,
  developmentPreviousTargetHost

Developmentvars == <<
  developmentActiveHost,
  developmentAvailableHost,
  developmentBlockedHost,
  developmentTargetHost,
  developmentActiveDesktop,
  developmentAvailableDesktop,
  developmentElectron,
  developmentHostHasTerminal,
  developmentDocumentState,
  developmentRestartPlan,
  developmentPhase,
  developmentClosed,
  developmentDesktopBuildFailed,
  developmentHostBuildFailed,
  developmentRestartPreparationAvailable,
  developmentLastAction,
  developmentPreviousActiveHost,
  developmentPreviousElectron,
  developmentPreviousHostHasTerminal,
  developmentPreviousDocumentState,
  developmentPreviousRestartPlan,
  developmentPreviousTargetHost
>>

DevelopmentRecord(action) ==
  /\ developmentLastAction' = action
  /\ developmentPreviousActiveHost' = developmentActiveHost
  /\ developmentPreviousElectron' = developmentElectron
  /\ developmentPreviousHostHasTerminal' = developmentHostHasTerminal
  /\ developmentPreviousDocumentState' = developmentDocumentState
  /\ developmentPreviousRestartPlan' = developmentRestartPlan
  /\ developmentPreviousTargetHost' = developmentTargetHost

DevelopmentInit ==
  /\ developmentActiveHost = 0
  /\ developmentAvailableHost = 0
  /\ developmentBlockedHost = 0
  /\ developmentTargetHost = DevelopmentNoGeneration
  /\ developmentActiveDesktop = 0
  /\ developmentAvailableDesktop = 0
  /\ developmentElectron = "running"
  /\ developmentHostHasTerminal \in (IF Scenario = "development-runtime" THEN BOOLEAN ELSE {FALSE})
  /\ developmentDocumentState = IF developmentHostHasTerminal THEN "retain" ELSE "empty"
  /\ developmentRestartPlan = "absent"
  /\ developmentPhase = "idle"
  /\ developmentClosed = FALSE
  /\ developmentDesktopBuildFailed = FALSE
  /\ developmentHostBuildFailed = FALSE
  /\ developmentRestartPreparationAvailable \in (IF Scenario = "development-runtime" THEN BOOLEAN ELSE {FALSE})
  /\ developmentLastAction = "init"
  /\ developmentPreviousActiveHost = developmentActiveHost
  /\ developmentPreviousElectron = developmentElectron
  /\ developmentPreviousHostHasTerminal = developmentHostHasTerminal
  /\ developmentPreviousDocumentState = developmentDocumentState
  /\ developmentPreviousRestartPlan = developmentRestartPlan
  /\ developmentPreviousTargetHost = developmentTargetHost

DevelopmentDesktopBuildFailsBase ==
  /\ ~developmentClosed
  /\ ~developmentDesktopBuildFailed
  /\ developmentDesktopBuildFailed' = TRUE
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop,
       developmentAvailableDesktop, developmentElectron, developmentHostHasTerminal, developmentDocumentState,
       developmentRestartPlan, developmentPhase, developmentClosed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("desktop-build-failed")

DevelopmentDesktopBuildSucceedsBase ==
  /\ ~developmentClosed
  /\ developmentAvailableDesktop < MaxGeneration
  /\ developmentAvailableDesktop' = developmentAvailableDesktop + 1
  /\ developmentDesktopBuildFailed' = FALSE
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop, developmentElectron,
       developmentHostHasTerminal, developmentDocumentState, developmentRestartPlan, developmentPhase, developmentClosed,
       developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("desktop-build-succeeded")

DevelopmentHostBuildFailsBase ==
  /\ ~developmentClosed
  /\ ~developmentHostBuildFailed
  /\ developmentHostBuildFailed' = TRUE
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop,
       developmentAvailableDesktop, developmentElectron, developmentHostHasTerminal, developmentDocumentState,
       developmentRestartPlan, developmentPhase, developmentClosed, developmentDesktopBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("host-build-failed")

DevelopmentHostBuildSucceedsBase ==
  /\ ~developmentClosed
  /\ developmentAvailableHost < MaxGeneration
  /\ developmentAvailableHost' = developmentAvailableHost + 1
  /\ developmentHostBuildFailed' = FALSE
  /\ UNCHANGED <<
       developmentActiveHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop, developmentAvailableDesktop, developmentElectron,
       developmentHostHasTerminal, developmentDocumentState, developmentRestartPlan, developmentPhase, developmentClosed,
       developmentDesktopBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("host-build-succeeded")

DevelopmentBeginDesktopRestartBase ==
  /\ ~developmentClosed
  /\ developmentPhase = "idle"
  /\ ~(developmentAvailableHost > developmentActiveHost /\ developmentAvailableHost > developmentBlockedHost)
  /\ developmentAvailableDesktop > developmentActiveDesktop
  /\ developmentElectron = "running"
  /\ developmentPhase' = "stopping-developmentElectron-for-desktop"
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop,
       developmentAvailableDesktop, developmentElectron, developmentHostHasTerminal, developmentDocumentState,
       developmentRestartPlan, developmentClosed, developmentDesktopBuildFailed, developmentHostBuildFailed,
       developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("begin-desktop-restart")

DevelopmentElectronStopsForDesktopBase ==
  /\ developmentPhase = "stopping-developmentElectron-for-desktop"
  /\ developmentElectron = "running"
  /\ developmentElectron' = "stopped"
  /\ developmentPhase' = "starting-developmentElectron-for-desktop"
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop,
       developmentAvailableDesktop, developmentHostHasTerminal, developmentDocumentState, developmentRestartPlan,
       developmentClosed, developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("electron-stopped-for-desktop")

DevelopmentElectronStartsForDesktopBase ==
  /\ developmentPhase = "starting-developmentElectron-for-desktop"
  /\ developmentElectron = "stopped"
  /\ developmentElectron' = "running"
  /\ developmentActiveDesktop' = developmentAvailableDesktop
  /\ developmentPhase' = "idle"
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentAvailableDesktop,
       developmentHostHasTerminal, developmentDocumentState, developmentRestartPlan, developmentClosed,
       developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("electron-started-for-desktop")

DevelopmentBeginHostRestartBase ==
  /\ ~developmentClosed
  /\ developmentPhase = "idle"
  /\ developmentAvailableHost > developmentActiveHost
  /\ developmentAvailableHost > developmentBlockedHost
  /\ developmentElectron = "running"
  /\ developmentRestartPlan = "absent"
  /\ developmentTargetHost' = developmentAvailableHost
  /\ developmentRestartPlan' = "pending"
  /\ developmentPhase' = "preparing-host-restart"
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentActiveDesktop, developmentAvailableDesktop,
       developmentElectron, developmentHostHasTerminal, developmentDocumentState, developmentClosed,
       developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("begin-host-restart")

DevelopmentPrepareHostRestartBase ==
  /\ developmentPhase = "preparing-host-restart"
  /\ developmentRestartPlan = "pending"
  /\ developmentRestartPreparationAvailable
  /\ developmentElectron = "running"
  /\ developmentDocumentState \in {"empty", "retain"}
  /\ developmentHostHasTerminal = (developmentDocumentState = "retain")
  /\ developmentDocumentState' = IF developmentHostHasTerminal THEN "provision" ELSE "empty"
  /\ developmentHostHasTerminal' = FALSE
  /\ developmentRestartPlan' = "ready"
  /\ developmentPhase' = "stopping-developmentElectron-for-host"
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop,
       developmentAvailableDesktop, developmentElectron, developmentClosed, developmentDesktopBuildFailed,
       developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("host-restart-prepared")

DevelopmentAbortHostRestartBase ==
  /\ developmentPhase = "preparing-host-restart"
  /\ developmentRestartPlan = "pending"
  /\ ~developmentRestartPreparationAvailable
  /\ developmentBlockedHost' = developmentTargetHost
  /\ developmentTargetHost' = DevelopmentNoGeneration
  /\ developmentRestartPlan' = "absent"
  /\ developmentPhase' = "idle"
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentActiveDesktop, developmentAvailableDesktop,
       developmentElectron, developmentHostHasTerminal, developmentDocumentState, developmentClosed,
       developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("host-restart-aborted")

DevelopmentElectronStopsForHostBase ==
  /\ developmentPhase = "stopping-developmentElectron-for-host"
  /\ developmentRestartPlan = "ready"
  /\ developmentElectron = "running"
  /\ developmentElectron' = "stopped"
  /\ developmentPhase' = "stopping-host"
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop,
       developmentAvailableDesktop, developmentHostHasTerminal, developmentDocumentState, developmentRestartPlan,
       developmentClosed, developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("electron-stopped-for-host")

DevelopmentHostStopsBase ==
  /\ developmentPhase = "stopping-host"
  /\ developmentElectron = "stopped"
  /\ developmentRestartPlan = "ready"
  /\ ~developmentHostHasTerminal
  /\ developmentActiveHost # DevelopmentNoGeneration
  /\ developmentActiveHost' = DevelopmentNoGeneration
  /\ developmentPhase' = "starting-host"
  /\ UNCHANGED <<
       developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop, developmentAvailableDesktop,
       developmentElectron, developmentHostHasTerminal, developmentDocumentState, developmentRestartPlan, developmentClosed,
       developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("host-stopped")

DevelopmentHostStartsBase ==
  /\ developmentPhase = "starting-host"
  /\ developmentActiveHost = DevelopmentNoGeneration
  /\ developmentTargetHost \in DevelopmentGenerations
  /\ developmentTargetHost <= developmentAvailableHost
  /\ developmentActiveHost' = developmentTargetHost
  /\ developmentPhase' = "starting-developmentElectron-for-host"
  /\ UNCHANGED <<
       developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop, developmentAvailableDesktop,
       developmentElectron, developmentHostHasTerminal, developmentDocumentState, developmentRestartPlan, developmentClosed,
       developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("host-started")

DevelopmentElectronStartsForHostBase ==
  /\ developmentPhase = "starting-developmentElectron-for-host"
  /\ developmentActiveHost \in DevelopmentGenerations
  /\ developmentElectron = "stopped"
  /\ developmentElectron' = "running"
  /\ developmentActiveDesktop' = developmentAvailableDesktop
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentAvailableDesktop,
       developmentHostHasTerminal, developmentDocumentState, developmentRestartPlan, developmentPhase, developmentClosed,
       developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("electron-started-for-host")

DevelopmentCompleteHostRestartBase ==
  /\ developmentPhase = "starting-developmentElectron-for-host"
  /\ developmentElectron = "running"
  /\ developmentRestartPlan = "ready"
  /\ developmentDocumentState \in {"empty", "provision"}
  /\ developmentHostHasTerminal' = (developmentDocumentState = "provision")
  /\ developmentDocumentState' = IF developmentDocumentState = "provision" THEN "retain" ELSE "empty"
  /\ developmentRestartPlan' = "absent"
  /\ developmentTargetHost' = DevelopmentNoGeneration
  /\ developmentPhase' = "idle"
  /\ UNCHANGED <<
       developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentActiveDesktop, developmentAvailableDesktop,
       developmentElectron, developmentClosed, developmentDesktopBuildFailed, developmentHostBuildFailed,
       developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("host-restart-completed")

DevelopmentCloseDevelopmentBase ==
  /\ ~developmentClosed
  /\ developmentClosed' = TRUE
  /\ developmentActiveHost' = DevelopmentNoGeneration
  /\ developmentTargetHost' = DevelopmentNoGeneration
  /\ developmentElectron' = "stopped"
  /\ developmentHostHasTerminal' = FALSE
  /\ developmentRestartPlan' = "absent"
  /\ developmentPhase' = "closed"
  /\ UNCHANGED <<
       developmentAvailableHost, developmentBlockedHost, developmentActiveDesktop, developmentAvailableDesktop, developmentDocumentState,
       developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable
     >>
  /\ DevelopmentRecord("close")

\* renderer-document
RendererNoGeneration == MaxGeneration + 1
RendererNavigationKinds == {
  "main-document",
  "main-same-document",
  "subframe-document",
  "subframe-same-document"
}
RendererActions == {
  "init",
  "start-document",
  "ignored-navigation",
  "finish-document"
}

VARIABLES
  rendererDocumentGeneration,
  rendererPendingGeneration,
  rendererControlGeneration,
  rendererLastAction,
  rendererLastNavigationKind,
  rendererPreviousDocumentGeneration,
  rendererPreviousPendingGeneration,
  rendererPreviousControlGeneration

Renderervars == <<
  rendererDocumentGeneration,
  rendererPendingGeneration,
  rendererControlGeneration,
  rendererLastAction,
  rendererLastNavigationKind,
  rendererPreviousDocumentGeneration,
  rendererPreviousPendingGeneration,
  rendererPreviousControlGeneration
>>

RendererRecord(action, navigationKind) ==
  /\ rendererLastAction' = action
  /\ rendererLastNavigationKind' = navigationKind
  /\ rendererPreviousDocumentGeneration' = rendererDocumentGeneration
  /\ rendererPreviousPendingGeneration' = rendererPendingGeneration
  /\ rendererPreviousControlGeneration' = rendererControlGeneration

RendererInit ==
  /\ rendererDocumentGeneration = 0
  /\ rendererPendingGeneration = RendererNoGeneration
  /\ rendererControlGeneration = RendererNoGeneration
  /\ rendererLastAction = "init"
  /\ rendererLastNavigationKind = "main-document"
  /\ rendererPreviousDocumentGeneration = rendererDocumentGeneration
  /\ rendererPreviousPendingGeneration = rendererPendingGeneration
  /\ rendererPreviousControlGeneration = rendererControlGeneration

RendererStartDocumentNavigationBase ==
  /\ rendererDocumentGeneration < MaxGeneration
  /\ rendererDocumentGeneration' = rendererDocumentGeneration + 1
  /\ rendererPendingGeneration' = rendererDocumentGeneration + 1
  /\ rendererControlGeneration' = RendererNoGeneration
  /\ RendererRecord("start-document", "main-document")

RendererStartIgnoredNavigationBase(navigationKind) ==
  /\ navigationKind \in RendererNavigationKinds \ {"main-document"}
  /\ UNCHANGED <<rendererDocumentGeneration, rendererPendingGeneration, rendererControlGeneration>>
  /\ RendererRecord("ignored-navigation", navigationKind)

RendererFinishDocumentBase ==
  /\ rendererPendingGeneration # RendererNoGeneration
  /\ rendererPendingGeneration' = RendererNoGeneration
  /\ rendererControlGeneration' = rendererDocumentGeneration
  /\ UNCHANGED rendererDocumentGeneration
  /\ RendererRecord("finish-document", "main-document")

RendererFinishDocumentWithoutObservedStartBase ==
  /\ rendererPendingGeneration = RendererNoGeneration
  /\ rendererDocumentGeneration < MaxGeneration
  /\ rendererDocumentGeneration' = rendererDocumentGeneration + 1
  /\ rendererPendingGeneration' = RendererNoGeneration
  /\ rendererControlGeneration' = rendererDocumentGeneration + 1
  /\ RendererRecord("finish-document", "main-document")

\* window-state
WindowDurableModes == {"normal", "maximized"}
WindowWindowModes == WindowDurableModes \union {"full-screen"}
WindowNoFlushCaptureCount == MaxCaptures + 1

VARIABLES
  windowMode,
  windowCaptures,
  windowProcessedCount,
  windowDurableOrdinal,
  windowFlushRequested,
  windowFlushCaptureCount,
  windowFlushed,
  windowDisposed,
  windowWritable

Windowvars == <<
  windowMode,
  windowCaptures,
  windowProcessedCount,
  windowDurableOrdinal,
  windowFlushRequested,
  windowFlushCaptureCount,
  windowFlushed,
  windowDisposed,
  windowWritable
>>

WindowInit ==
  /\ windowMode \in (IF Scenario = "window-state" THEN WindowWindowModes ELSE {"normal"})
  /\ windowCaptures = <<>>
  /\ windowProcessedCount = 0
  /\ windowDurableOrdinal = 0
  /\ windowFlushRequested = FALSE
  /\ windowFlushCaptureCount = WindowNoFlushCaptureCount
  /\ windowFlushed = FALSE
  /\ windowDisposed = FALSE
  /\ windowWritable \in (IF Scenario = "window-state" THEN BOOLEAN ELSE {FALSE})

WindowSetWindowModeBase(nextMode) ==
  /\ nextMode \in WindowWindowModes
  /\ ~windowFlushRequested
  /\ ~windowDisposed
  /\ windowMode' = nextMode
  /\ UNCHANGED <<windowCaptures, windowProcessedCount, windowDurableOrdinal,
       windowFlushRequested, windowFlushCaptureCount, windowFlushed, windowDisposed, windowWritable>>

WindowCaptureBase ==
  /\ windowMode \in WindowDurableModes
  /\ Len(windowCaptures) < MaxCaptures
  /\ ~windowFlushRequested
  /\ ~windowDisposed
  /\ windowCaptures' = Append(windowCaptures, windowMode)
  /\ UNCHANGED <<windowMode, windowProcessedCount, windowDurableOrdinal,
       windowFlushRequested, windowFlushCaptureCount, windowFlushed, windowDisposed, windowWritable>>

WindowProcessNextBase ==
  /\ windowProcessedCount < Len(windowCaptures)
  /\ ~windowDisposed
  /\ windowProcessedCount' = windowProcessedCount + 1
  /\ windowDurableOrdinal' = IF windowWritable THEN windowProcessedCount' ELSE windowDurableOrdinal
  /\ UNCHANGED <<windowMode, windowCaptures, windowFlushRequested, windowFlushCaptureCount,
       windowFlushed, windowDisposed, windowWritable>>

WindowRequestFlushBase ==
  /\ ~windowFlushRequested
  /\ ~windowDisposed
  /\ windowFlushRequested' = TRUE
  /\ windowFlushCaptureCount' = Len(windowCaptures)
  /\ UNCHANGED <<windowMode, windowCaptures, windowProcessedCount, windowDurableOrdinal,
       windowFlushed, windowDisposed, windowWritable>>

WindowCompleteFlushBase ==
  /\ windowFlushRequested
  /\ ~windowFlushed
  /\ windowProcessedCount = Len(windowCaptures)
  /\ windowFlushed' = TRUE
  /\ UNCHANGED <<windowMode, windowCaptures, windowProcessedCount, windowDurableOrdinal,
       windowFlushRequested, windowFlushCaptureCount, windowDisposed, windowWritable>>

WindowDisposeBase ==
  /\ windowFlushed
  /\ ~windowDisposed
  /\ windowDisposed' = TRUE
  /\ UNCHANGED <<windowMode, windowCaptures, windowProcessedCount, windowDurableOrdinal,
       windowFlushRequested, windowFlushCaptureCount, windowFlushed, windowWritable>>

\* terminal-binding-close
BindingBindingStates == {"retained", "closing", "removed"}
BindingTerminalPhases == {"running", "exited", "failed", "absent"}

VARIABLES
  bindingDesktopActive,
  bindingBindingState,
  bindingProjectionAttached,
  bindingTerminalPhase,
  bindingGracefulRequested,
  bindingGraceExpired,
  bindingForceRequested

Bindingvars == <<
  bindingDesktopActive,
  bindingBindingState,
  bindingProjectionAttached,
  bindingTerminalPhase,
  bindingGracefulRequested,
  bindingGraceExpired,
  bindingForceRequested
>>

BindingInit ==
  /\ bindingDesktopActive = TRUE
  /\ bindingBindingState = [terminal \in Terminals |-> "retained"]
  /\ bindingProjectionAttached = [terminal \in Terminals |-> TRUE]
  /\ bindingTerminalPhase = [terminal \in Terminals |-> "running"]
  /\ bindingGracefulRequested = [terminal \in Terminals |-> FALSE]
  /\ bindingGraceExpired = [terminal \in Terminals |-> FALSE]
  /\ bindingForceRequested = [terminal \in Terminals |-> FALSE]

BindingBeginCloseBase(terminal) ==
  /\ bindingDesktopActive
  /\ terminal \in Terminals
  /\ bindingBindingState[terminal] = "retained"
  /\ bindingBindingState' = [bindingBindingState EXCEPT ![terminal] = "closing"]
  /\ UNCHANGED <<bindingDesktopActive, bindingProjectionAttached,
                  bindingTerminalPhase, bindingGracefulRequested, bindingGraceExpired,
                  bindingForceRequested>>

BindingRetireProjectionBase(terminal) ==
  /\ bindingDesktopActive
  /\ terminal \in Terminals
  /\ bindingBindingState[terminal] = "closing"
  /\ bindingProjectionAttached[terminal]
  /\ bindingProjectionAttached' =
       [bindingProjectionAttached EXCEPT ![terminal] = FALSE]
  /\ UNCHANGED <<bindingDesktopActive, bindingBindingState,
                  bindingTerminalPhase, bindingGracefulRequested, bindingGraceExpired,
                  bindingForceRequested>>

BindingRequestGracefulBase(terminal) ==
  /\ bindingDesktopActive
  /\ terminal \in Terminals
  /\ bindingBindingState[terminal] = "closing"
  /\ ~bindingProjectionAttached[terminal]
  /\ bindingTerminalPhase[terminal] = "running"
  /\ ~bindingGracefulRequested[terminal]
  /\ bindingGracefulRequested' =
       [bindingGracefulRequested EXCEPT ![terminal] = TRUE]
  /\ UNCHANGED <<bindingDesktopActive, bindingBindingState,
                  bindingProjectionAttached, bindingTerminalPhase, bindingGraceExpired,
                  bindingForceRequested>>

BindingExpireGraceBase(terminal) ==
  /\ bindingDesktopActive
  /\ terminal \in Terminals
  /\ bindingBindingState[terminal] = "closing"
  /\ bindingGracefulRequested[terminal]
  /\ bindingTerminalPhase[terminal] = "running"
  /\ ~bindingGraceExpired[terminal]
  /\ bindingGraceExpired' = [bindingGraceExpired EXCEPT ![terminal] = TRUE]
  /\ UNCHANGED <<bindingDesktopActive, bindingBindingState,
                  bindingProjectionAttached, bindingTerminalPhase, bindingGracefulRequested,
                  bindingForceRequested>>

BindingRequestForceBase(terminal) ==
  /\ bindingDesktopActive
  /\ terminal \in Terminals
  /\ bindingBindingState[terminal] = "closing"
  /\ ~bindingProjectionAttached[terminal]
  /\ bindingTerminalPhase[terminal] = "running"
  /\ bindingGracefulRequested[terminal]
  /\ bindingGraceExpired[terminal]
  /\ ~bindingForceRequested[terminal]
  /\ bindingForceRequested' = [bindingForceRequested EXCEPT ![terminal] = TRUE]
  /\ UNCHANGED <<bindingDesktopActive, bindingBindingState,
                  bindingProjectionAttached, bindingTerminalPhase, bindingGracefulRequested,
                  bindingGraceExpired>>

BindingHostExitBase(terminal) ==
  /\ terminal \in Terminals
  /\ bindingTerminalPhase[terminal] = "running"
  /\ bindingTerminalPhase' = [bindingTerminalPhase EXCEPT ![terminal] = "exited"]
  /\ UNCHANGED <<bindingDesktopActive, bindingBindingState,
                  bindingProjectionAttached, bindingGracefulRequested, bindingGraceExpired,
                  bindingForceRequested>>

BindingHostFailBase(terminal) ==
  /\ terminal \in Terminals
  /\ bindingTerminalPhase[terminal] = "running"
  /\ bindingTerminalPhase' = [bindingTerminalPhase EXCEPT ![terminal] = "failed"]
  /\ UNCHANGED <<bindingDesktopActive, bindingBindingState,
                  bindingProjectionAttached, bindingGracefulRequested, bindingGraceExpired,
                  bindingForceRequested>>

BindingDisposeBase(terminal) ==
  /\ bindingDesktopActive
  /\ terminal \in Terminals
  /\ bindingBindingState[terminal] = "closing"
  /\ ~bindingProjectionAttached[terminal]
  /\ bindingTerminalPhase[terminal] \in {"exited", "failed"}
  /\ bindingTerminalPhase' = [bindingTerminalPhase EXCEPT ![terminal] = "absent"]
  /\ UNCHANGED <<bindingDesktopActive, bindingBindingState,
                  bindingProjectionAttached, bindingGracefulRequested, bindingGraceExpired,
                  bindingForceRequested>>

BindingRemoveBindingBase(terminal) ==
  /\ bindingDesktopActive
  /\ terminal \in Terminals
  /\ bindingBindingState[terminal] = "closing"
  /\ ~bindingProjectionAttached[terminal]
  /\ bindingTerminalPhase[terminal] = "absent"
  /\ bindingBindingState' = [bindingBindingState EXCEPT ![terminal] = "removed"]
  /\ bindingGracefulRequested' =
       [bindingGracefulRequested EXCEPT ![terminal] = FALSE]
  /\ bindingGraceExpired' = [bindingGraceExpired EXCEPT ![terminal] = FALSE]
  /\ bindingForceRequested' = [bindingForceRequested EXCEPT ![terminal] = FALSE]
  /\ UNCHANGED <<bindingDesktopActive, bindingProjectionAttached,
                  bindingTerminalPhase>>

BindingCrashDesktopBase ==
  /\ bindingDesktopActive
  /\ bindingDesktopActive' = FALSE
  /\ bindingProjectionAttached' = [terminal \in Terminals |-> FALSE]
  /\ UNCHANGED <<bindingBindingState, bindingTerminalPhase,
                  bindingGracefulRequested, bindingGraceExpired, bindingForceRequested>>

BindingRestartDesktopBase ==
  /\ ~bindingDesktopActive
  /\ bindingDesktopActive' = TRUE
  /\ UNCHANGED <<bindingBindingState, bindingProjectionAttached,
                  bindingTerminalPhase, bindingGracefulRequested, bindingGraceExpired,
                  bindingForceRequested>>

\* project-folder-acquisition
FolderRequests == 1..MaxGeneration
FolderTargets == {"create-project", "add-project-folder"}
FolderClientPhases == {"idle", "selecting", "selected", "failed", "disposed"}
FolderBrokerPhases == {"idle", "selecting", "disposed"}
FolderNativeOutcomes == {"none", "selected", "cancelled", "failed"}
FolderActions == {"init", "request", "busy", "invalidate-target", "duplicate-target",
  "native-selected", "native-cancelled", "native-failed", "deliver", "commit",
  "reject-selected", "dismiss-failure", "retire", "late-native", "unknown-completion"}

VARIABLES
  folderClientPhase,
  folderBrokerPhase,
  folderIssuedRequests,
  folderActiveRequest,
  folderRequestTargets,
  folderActiveTarget,
  folderTargetAvailable,
  folderTargetDuplicate,
  folderNativePending,
  folderNativeOutcome,
  folderSelectedRequests,
  folderCancelledRequests,
  folderFailedRequests,
  folderDeliveredRequests,
  folderCommittedRequests,
  folderCommittedAtRetirement,
  folderBusyRejected,
  folderLateCompletionIgnored,
  folderLastAction

FolderVars == <<folderClientPhase, folderBrokerPhase, folderIssuedRequests,
  folderActiveRequest, folderRequestTargets, folderActiveTarget, folderTargetAvailable,
  folderTargetDuplicate, folderNativePending, folderNativeOutcome, folderSelectedRequests,
  folderCancelledRequests, folderFailedRequests, folderDeliveredRequests,
  folderCommittedRequests, folderCommittedAtRetirement, folderBusyRejected,
  folderLateCompletionIgnored, folderLastAction>>

FolderInit ==
  /\ folderClientPhase = "idle"
  /\ folderBrokerPhase = "idle"
  /\ folderIssuedRequests = {}
  /\ folderActiveRequest = 0
  /\ folderRequestTargets = [request \in FolderRequests |-> "none"]
  /\ folderActiveTarget = "none"
  /\ folderTargetAvailable = TRUE
  /\ folderTargetDuplicate = FALSE
  /\ folderNativePending = FALSE
  /\ folderNativeOutcome = "none"
  /\ folderSelectedRequests = {}
  /\ folderCancelledRequests = {}
  /\ folderFailedRequests = {}
  /\ folderDeliveredRequests = {}
  /\ folderCommittedRequests = {}
  /\ folderCommittedAtRetirement = {}
  /\ folderBusyRejected = FALSE
  /\ folderLateCompletionIgnored = FALSE
  /\ folderLastAction = "init"

FolderRequestSelectionBase(request, target) ==
  /\ folderClientPhase = "idle"
  /\ folderBrokerPhase = "idle"
  /\ request \in FolderRequests \ folderIssuedRequests
  /\ target \in FolderTargets
  /\ folderClientPhase' = "selecting"
  /\ folderBrokerPhase' = "selecting"
  /\ folderIssuedRequests' = folderIssuedRequests \cup {request}
  /\ folderActiveRequest' = request
  /\ folderRequestTargets' = [folderRequestTargets EXCEPT ![request] = target]
  /\ folderActiveTarget' = target
  /\ folderTargetAvailable' = TRUE
  /\ folderTargetDuplicate' = FALSE
  /\ folderNativePending' = TRUE
  /\ folderNativeOutcome' = "none"
  /\ folderBusyRejected' = FALSE
  /\ folderLastAction' = "request"
  /\ UNCHANGED <<folderSelectedRequests, folderCancelledRequests, folderFailedRequests,
                  folderDeliveredRequests, folderCommittedRequests,
                  folderCommittedAtRetirement, folderLateCompletionIgnored>>

FolderRejectBusyBase ==
  /\ folderClientPhase = "selecting"
  /\ folderBrokerPhase = "selecting"
  /\ ~folderBusyRejected
  /\ folderBusyRejected' = TRUE
  /\ folderLastAction' = "busy"
  /\ UNCHANGED <<folderClientPhase, folderBrokerPhase, folderIssuedRequests,
                  folderActiveRequest, folderRequestTargets, folderActiveTarget,
                  folderTargetAvailable, folderTargetDuplicate, folderNativePending,
                  folderNativeOutcome, folderSelectedRequests, folderCancelledRequests,
                  folderFailedRequests, folderDeliveredRequests, folderCommittedRequests,
                  folderCommittedAtRetirement, folderLateCompletionIgnored>>

FolderInvalidateTargetBase ==
  /\ folderClientPhase \in {"selecting", "selected"}
  /\ folderActiveTarget = "add-project-folder"
  /\ folderTargetAvailable
  /\ folderTargetAvailable' = FALSE
  /\ folderLastAction' = "invalidate-target"
  /\ UNCHANGED <<folderClientPhase, folderBrokerPhase, folderIssuedRequests,
                  folderActiveRequest, folderRequestTargets, folderActiveTarget,
                  folderTargetDuplicate, folderNativePending, folderNativeOutcome,
                  folderSelectedRequests, folderCancelledRequests, folderFailedRequests,
                  folderDeliveredRequests, folderCommittedRequests,
                  folderCommittedAtRetirement, folderBusyRejected,
                  folderLateCompletionIgnored>>

FolderMakeTargetDuplicateBase ==
  /\ folderClientPhase \in {"selecting", "selected"}
  /\ folderActiveTarget = "add-project-folder"
  /\ ~folderTargetDuplicate
  /\ folderTargetDuplicate' = TRUE
  /\ folderLastAction' = "duplicate-target"
  /\ UNCHANGED <<folderClientPhase, folderBrokerPhase, folderIssuedRequests,
                  folderActiveRequest, folderRequestTargets, folderActiveTarget,
                  folderTargetAvailable, folderNativePending, folderNativeOutcome,
                  folderSelectedRequests, folderCancelledRequests, folderFailedRequests,
                  folderDeliveredRequests, folderCommittedRequests,
                  folderCommittedAtRetirement, folderBusyRejected,
                  folderLateCompletionIgnored>>

FolderNativeSelectsBase ==
  /\ folderBrokerPhase = "selecting"
  /\ folderNativePending
  /\ folderNativePending' = FALSE
  /\ folderNativeOutcome' = "selected"
  /\ folderSelectedRequests' = folderSelectedRequests \cup {folderActiveRequest}
  /\ folderLastAction' = "native-selected"
  /\ UNCHANGED <<folderClientPhase, folderBrokerPhase, folderIssuedRequests,
                  folderActiveRequest, folderRequestTargets, folderActiveTarget,
                  folderTargetAvailable, folderTargetDuplicate, folderCancelledRequests,
                  folderFailedRequests, folderDeliveredRequests, folderCommittedRequests,
                  folderCommittedAtRetirement, folderBusyRejected,
                  folderLateCompletionIgnored>>

FolderNativeCancelsBase ==
  /\ folderBrokerPhase = "selecting"
  /\ folderNativePending
  /\ folderNativePending' = FALSE
  /\ folderNativeOutcome' = "cancelled"
  /\ folderCancelledRequests' = folderCancelledRequests \cup {folderActiveRequest}
  /\ folderLastAction' = "native-cancelled"
  /\ UNCHANGED <<folderClientPhase, folderBrokerPhase, folderIssuedRequests,
                  folderActiveRequest, folderRequestTargets, folderActiveTarget,
                  folderTargetAvailable, folderTargetDuplicate, folderSelectedRequests,
                  folderFailedRequests, folderDeliveredRequests, folderCommittedRequests,
                  folderCommittedAtRetirement, folderBusyRejected,
                  folderLateCompletionIgnored>>

FolderNativeFailsBase ==
  /\ folderBrokerPhase = "selecting"
  /\ folderNativePending
  /\ folderNativePending' = FALSE
  /\ folderNativeOutcome' = "failed"
  /\ folderFailedRequests' = folderFailedRequests \cup {folderActiveRequest}
  /\ folderLastAction' = "native-failed"
  /\ UNCHANGED <<folderClientPhase, folderBrokerPhase, folderIssuedRequests,
                  folderActiveRequest, folderRequestTargets, folderActiveTarget,
                  folderTargetAvailable, folderTargetDuplicate, folderSelectedRequests,
                  folderCancelledRequests, folderDeliveredRequests,
                  folderCommittedRequests, folderCommittedAtRetirement,
                  folderBusyRejected, folderLateCompletionIgnored>>

FolderDeliverResultBase ==
  /\ folderClientPhase = "selecting"
  /\ folderBrokerPhase = "selecting"
  /\ folderNativeOutcome \in {"selected", "cancelled", "failed"}
  /\ folderClientPhase' =
       IF folderNativeOutcome = "selected" THEN "selected"
       ELSE IF folderNativeOutcome = "failed" THEN "failed"
       ELSE "idle"
  /\ folderBrokerPhase' = "idle"
  /\ folderDeliveredRequests' = folderDeliveredRequests \cup {folderActiveRequest}
  /\ folderActiveRequest' =
       IF folderNativeOutcome = "selected" THEN folderActiveRequest ELSE 0
  /\ folderActiveTarget' =
       IF folderNativeOutcome = "selected" THEN folderActiveTarget ELSE "none"
  /\ folderTargetAvailable' =
       IF folderNativeOutcome = "selected" THEN folderTargetAvailable ELSE TRUE
  /\ folderTargetDuplicate' =
       IF folderNativeOutcome = "selected" THEN folderTargetDuplicate ELSE FALSE
  /\ folderNativeOutcome' = "none"
  /\ folderLastAction' = "deliver"
  /\ UNCHANGED <<folderIssuedRequests, folderRequestTargets, folderNativePending,
                  folderSelectedRequests, folderCancelledRequests, folderFailedRequests,
                  folderCommittedRequests, folderCommittedAtRetirement,
                  folderBusyRejected, folderLateCompletionIgnored>>

FolderCommitSelectedBase ==
  /\ folderClientPhase = "selected"
  /\ folderBrokerPhase = "idle"
  /\ folderTargetAvailable
  /\ ~folderTargetDuplicate
  /\ folderClientPhase' = "idle"
  /\ folderActiveRequest' = 0
  /\ folderActiveTarget' = "none"
  /\ folderTargetAvailable' = TRUE
  /\ folderTargetDuplicate' = FALSE
  /\ folderCommittedRequests' = folderCommittedRequests \cup {folderActiveRequest}
  /\ folderLastAction' = "commit"
  /\ UNCHANGED <<folderBrokerPhase, folderIssuedRequests, folderRequestTargets,
                  folderNativePending, folderNativeOutcome, folderSelectedRequests,
                  folderCancelledRequests, folderFailedRequests, folderDeliveredRequests,
                  folderCommittedAtRetirement, folderBusyRejected,
                  folderLateCompletionIgnored>>

FolderRejectSelectedBase ==
  /\ folderClientPhase = "selected"
  /\ folderBrokerPhase = "idle"
  /\ (~folderTargetAvailable \/ folderTargetDuplicate)
  /\ folderClientPhase' = "failed"
  /\ folderFailedRequests' = folderFailedRequests \cup {folderActiveRequest}
  /\ folderActiveRequest' = 0
  /\ folderActiveTarget' = "none"
  /\ folderTargetAvailable' = TRUE
  /\ folderTargetDuplicate' = FALSE
  /\ folderLastAction' = "reject-selected"
  /\ UNCHANGED <<folderBrokerPhase, folderIssuedRequests, folderRequestTargets,
                  folderNativePending, folderNativeOutcome, folderSelectedRequests,
                  folderCancelledRequests, folderDeliveredRequests,
                  folderCommittedRequests, folderCommittedAtRetirement,
                  folderBusyRejected, folderLateCompletionIgnored>>

FolderDismissFailureBase ==
  /\ folderClientPhase = "failed"
  /\ folderBrokerPhase = "idle"
  /\ folderClientPhase' = "idle"
  /\ folderLastAction' = "dismiss-failure"
  /\ UNCHANGED <<folderBrokerPhase, folderIssuedRequests, folderActiveRequest,
                  folderRequestTargets, folderActiveTarget, folderTargetAvailable,
                  folderTargetDuplicate, folderNativePending, folderNativeOutcome,
                  folderSelectedRequests, folderCancelledRequests, folderFailedRequests,
                  folderDeliveredRequests, folderCommittedRequests,
                  folderCommittedAtRetirement, folderBusyRejected,
                  folderLateCompletionIgnored>>

FolderRetireBase ==
  /\ folderClientPhase # "disposed"
  /\ folderBrokerPhase # "disposed"
  /\ folderClientPhase' = "disposed"
  /\ folderBrokerPhase' = "disposed"
  /\ folderActiveRequest' = 0
  /\ folderActiveTarget' = "none"
  /\ folderTargetAvailable' = TRUE
  /\ folderTargetDuplicate' = FALSE
  /\ folderNativeOutcome' = "none"
  /\ folderCommittedAtRetirement' = folderCommittedRequests
  /\ folderLastAction' = "retire"
  /\ UNCHANGED <<folderIssuedRequests, folderRequestTargets, folderNativePending,
                  folderSelectedRequests, folderCancelledRequests, folderFailedRequests,
                  folderDeliveredRequests, folderCommittedRequests, folderBusyRejected,
                  folderLateCompletionIgnored>>

FolderNativeCompletesAfterRetireBase ==
  /\ folderClientPhase = "disposed"
  /\ folderBrokerPhase = "disposed"
  /\ folderNativePending
  /\ folderNativePending' = FALSE
  /\ folderLateCompletionIgnored' = TRUE
  /\ folderLastAction' = "late-native"
  /\ UNCHANGED <<folderClientPhase, folderBrokerPhase, folderIssuedRequests,
                  folderActiveRequest, folderRequestTargets, folderActiveTarget,
                  folderTargetAvailable, folderTargetDuplicate, folderNativeOutcome,
                  folderSelectedRequests, folderCancelledRequests, folderFailedRequests,
                  folderDeliveredRequests, folderCommittedRequests,
                  folderCommittedAtRetirement, folderBusyRejected>>

FolderRejectUnknownCompletionBase ==
  /\ folderClientPhase = "idle"
  /\ folderBrokerPhase = "idle"
  /\ folderClientPhase' = "disposed"
  /\ folderBrokerPhase' = "disposed"
  /\ folderCommittedAtRetirement' = folderCommittedRequests
  /\ folderLastAction' = "unknown-completion"
  /\ UNCHANGED <<folderIssuedRequests, folderActiveRequest, folderRequestTargets,
                  folderActiveTarget, folderTargetAvailable, folderTargetDuplicate,
                  folderNativePending, folderNativeOutcome, folderSelectedRequests,
                  folderCancelledRequests, folderFailedRequests, folderDeliveredRequests,
                  folderCommittedRequests, folderBusyRejected,
                  folderLateCompletionIgnored>>

AppOtherVars == <<browserPhase, browserActiveMode, browserDesiredMode, browserServerOwned,
  browserGatewayOwned, browserDisposeRequested, browserLastAction, developmentActiveHost,
  developmentAvailableHost, developmentBlockedHost, developmentTargetHost, developmentActiveDesktop,
  developmentAvailableDesktop, developmentElectron, developmentHostHasTerminal, developmentDocumentState,
  developmentRestartPlan, developmentPhase, developmentClosed, developmentDesktopBuildFailed,
  developmentHostBuildFailed, developmentRestartPreparationAvailable, developmentLastAction, developmentPreviousActiveHost,
  developmentPreviousElectron, developmentPreviousHostHasTerminal, developmentPreviousDocumentState, developmentPreviousRestartPlan,
  developmentPreviousTargetHost, rendererDocumentGeneration, rendererPendingGeneration, rendererControlGeneration,
  rendererLastAction, rendererLastNavigationKind, rendererPreviousDocumentGeneration, rendererPreviousPendingGeneration,
  rendererPreviousControlGeneration, windowMode, windowCaptures, windowProcessedCount,
  windowDurableOrdinal, windowFlushRequested, windowFlushCaptureCount, windowFlushed,
  windowDisposed, windowWritable, bindingDesktopActive, bindingBindingState,
  bindingProjectionAttached, bindingTerminalPhase, bindingGracefulRequested, bindingGraceExpired,
  bindingForceRequested, FolderVars>>
BrowserOtherVars == <<appMode, appPhase, appPrompted, appTopologyDurable,
  appRestartProvisionDurable, appRestartPlanReady, appBindings, appCloseIssued,
  appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
  appWritable, appPlanWritable, appLastAction, appPreviousBindings,
  appPreviousEffectsStarted, developmentActiveHost, developmentAvailableHost, developmentBlockedHost,
  developmentTargetHost, developmentActiveDesktop, developmentAvailableDesktop, developmentElectron,
  developmentHostHasTerminal, developmentDocumentState, developmentRestartPlan, developmentPhase,
  developmentClosed, developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable,
  developmentLastAction, developmentPreviousActiveHost, developmentPreviousElectron, developmentPreviousHostHasTerminal,
  developmentPreviousDocumentState, developmentPreviousRestartPlan, developmentPreviousTargetHost, rendererDocumentGeneration,
  rendererPendingGeneration, rendererControlGeneration, rendererLastAction, rendererLastNavigationKind,
  rendererPreviousDocumentGeneration, rendererPreviousPendingGeneration, rendererPreviousControlGeneration, windowMode,
  windowCaptures, windowProcessedCount, windowDurableOrdinal, windowFlushRequested,
  windowFlushCaptureCount, windowFlushed, windowDisposed, windowWritable,
  bindingDesktopActive, bindingBindingState, bindingProjectionAttached, bindingTerminalPhase,
  bindingGracefulRequested, bindingGraceExpired, bindingForceRequested, FolderVars>>
DevelopmentOtherVars == <<appMode, appPhase, appPrompted, appTopologyDurable,
  appRestartProvisionDurable, appRestartPlanReady, appBindings, appCloseIssued,
  appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
  appWritable, appPlanWritable, appLastAction, appPreviousBindings,
  appPreviousEffectsStarted, browserPhase, browserActiveMode, browserDesiredMode,
  browserServerOwned, browserGatewayOwned, browserDisposeRequested, browserLastAction,
  rendererDocumentGeneration, rendererPendingGeneration, rendererControlGeneration, rendererLastAction,
  rendererLastNavigationKind, rendererPreviousDocumentGeneration, rendererPreviousPendingGeneration, rendererPreviousControlGeneration,
  windowMode, windowCaptures, windowProcessedCount, windowDurableOrdinal,
  windowFlushRequested, windowFlushCaptureCount, windowFlushed, windowDisposed,
  windowWritable, bindingDesktopActive, bindingBindingState, bindingProjectionAttached,
  bindingTerminalPhase, bindingGracefulRequested, bindingGraceExpired, bindingForceRequested,
  FolderVars>>
RendererOtherVars == <<appMode, appPhase, appPrompted, appTopologyDurable,
  appRestartProvisionDurable, appRestartPlanReady, appBindings, appCloseIssued,
  appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
  appWritable, appPlanWritable, appLastAction, appPreviousBindings,
  appPreviousEffectsStarted, browserPhase, browserActiveMode, browserDesiredMode,
  browserServerOwned, browserGatewayOwned, browserDisposeRequested, browserLastAction,
  developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost,
  developmentActiveDesktop, developmentAvailableDesktop, developmentElectron, developmentHostHasTerminal,
  developmentDocumentState, developmentRestartPlan, developmentPhase, developmentClosed,
  developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable, developmentLastAction,
  developmentPreviousActiveHost, developmentPreviousElectron, developmentPreviousHostHasTerminal, developmentPreviousDocumentState,
  developmentPreviousRestartPlan, developmentPreviousTargetHost, windowMode, windowCaptures,
  windowProcessedCount, windowDurableOrdinal, windowFlushRequested, windowFlushCaptureCount,
  windowFlushed, windowDisposed, windowWritable, bindingDesktopActive,
  bindingBindingState, bindingProjectionAttached, bindingTerminalPhase, bindingGracefulRequested,
  bindingGraceExpired, bindingForceRequested, FolderVars>>
WindowOtherVars == <<appMode, appPhase, appPrompted, appTopologyDurable,
  appRestartProvisionDurable, appRestartPlanReady, appBindings, appCloseIssued,
  appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
  appWritable, appPlanWritable, appLastAction, appPreviousBindings,
  appPreviousEffectsStarted, browserPhase, browserActiveMode, browserDesiredMode,
  browserServerOwned, browserGatewayOwned, browserDisposeRequested, browserLastAction,
  developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost,
  developmentActiveDesktop, developmentAvailableDesktop, developmentElectron, developmentHostHasTerminal,
  developmentDocumentState, developmentRestartPlan, developmentPhase, developmentClosed,
  developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable, developmentLastAction,
  developmentPreviousActiveHost, developmentPreviousElectron, developmentPreviousHostHasTerminal, developmentPreviousDocumentState,
  developmentPreviousRestartPlan, developmentPreviousTargetHost, rendererDocumentGeneration, rendererPendingGeneration,
  rendererControlGeneration, rendererLastAction, rendererLastNavigationKind, rendererPreviousDocumentGeneration,
  rendererPreviousPendingGeneration, rendererPreviousControlGeneration, bindingDesktopActive, bindingBindingState,
  bindingProjectionAttached, bindingTerminalPhase, bindingGracefulRequested, bindingGraceExpired,
  bindingForceRequested, FolderVars>>
BindingOtherVars == <<appMode, appPhase, appPrompted, appTopologyDurable,
  appRestartProvisionDurable, appRestartPlanReady, appBindings, appCloseIssued,
  appHostOrphan, appEffectsStarted, appForcedQuit, appQuit,
  appWritable, appPlanWritable, appLastAction, appPreviousBindings,
  appPreviousEffectsStarted, browserPhase, browserActiveMode, browserDesiredMode,
  browserServerOwned, browserGatewayOwned, browserDisposeRequested, browserLastAction,
  developmentActiveHost, developmentAvailableHost, developmentBlockedHost, developmentTargetHost,
  developmentActiveDesktop, developmentAvailableDesktop, developmentElectron, developmentHostHasTerminal,
  developmentDocumentState, developmentRestartPlan, developmentPhase, developmentClosed,
  developmentDesktopBuildFailed, developmentHostBuildFailed, developmentRestartPreparationAvailable, developmentLastAction,
  developmentPreviousActiveHost, developmentPreviousElectron, developmentPreviousHostHasTerminal, developmentPreviousDocumentState,
  developmentPreviousRestartPlan, developmentPreviousTargetHost, rendererDocumentGeneration, rendererPendingGeneration,
  rendererControlGeneration, rendererLastAction, rendererLastNavigationKind, rendererPreviousDocumentGeneration,
  rendererPreviousPendingGeneration, rendererPreviousControlGeneration, windowMode, windowCaptures,
  windowProcessedCount, windowDurableOrdinal, windowFlushRequested, windowFlushCaptureCount,
  windowFlushed, windowDisposed, windowWritable, FolderVars>>

FolderOtherVars == <<appMode, appPhase, appPrompted, appTopologyDurable,
  appRestartProvisionDurable, appRestartPlanReady, appBindings, appCloseIssued,
  appHostOrphan, appEffectsStarted, appForcedQuit, appQuit, appWritable,
  appPlanWritable, appLastAction, appPreviousBindings, appPreviousEffectsStarted,
  browserPhase, browserActiveMode, browserDesiredMode, browserServerOwned,
  browserGatewayOwned, browserDisposeRequested, browserLastAction,
  developmentActiveHost, developmentAvailableHost, developmentBlockedHost,
  developmentTargetHost, developmentActiveDesktop, developmentAvailableDesktop,
  developmentElectron, developmentHostHasTerminal, developmentDocumentState,
  developmentRestartPlan, developmentPhase, developmentClosed,
  developmentDesktopBuildFailed, developmentHostBuildFailed,
  developmentRestartPreparationAvailable, developmentLastAction,
  developmentPreviousActiveHost, developmentPreviousElectron,
  developmentPreviousHostHasTerminal, developmentPreviousDocumentState,
  developmentPreviousRestartPlan, developmentPreviousTargetHost,
  rendererDocumentGeneration, rendererPendingGeneration, rendererControlGeneration,
  rendererLastAction, rendererLastNavigationKind, rendererPreviousDocumentGeneration,
  rendererPreviousPendingGeneration, rendererPreviousControlGeneration, windowMode,
  windowCaptures, windowProcessedCount, windowDurableOrdinal, windowFlushRequested,
  windowFlushCaptureCount, windowFlushed, windowDisposed, windowWritable,
  bindingDesktopActive, bindingBindingState, bindingProjectionAttached,
  bindingTerminalPhase, bindingGracefulRequested, bindingGraceExpired,
  bindingForceRequested>>

AppRequestProtectedUserQuit ==
  /\ Scenario = "application-lifetime"
  /\ AppRequestProtectedUserQuitBase
  /\ UNCHANGED AppOtherVars

AppRequestUnprotectedUserQuit ==
  /\ Scenario = "application-lifetime"
  /\ AppRequestUnprotectedUserQuitBase
  /\ UNCHANGED AppOtherVars

AppRequestUpdateRestart ==
  /\ Scenario = "application-lifetime"
  /\ AppRequestUpdateRestartBase
  /\ UNCHANGED AppOtherVars

AppRequestDevelopmentHostRestart ==
  /\ Scenario = "application-lifetime"
  /\ AppRequestDevelopmentHostRestartBase
  /\ UNCHANGED AppOtherVars

AppChoosePreserve ==
  /\ Scenario = "application-lifetime"
  /\ AppChoosePreserveBase
  /\ UNCHANGED AppOtherVars

AppChooseStop ==
  /\ Scenario = "application-lifetime"
  /\ AppChooseStopBase
  /\ UNCHANGED AppOtherVars

AppCancel ==
  /\ Scenario = "application-lifetime"
  /\ AppCancelBase
  /\ UNCHANGED AppOtherVars

AppFlush ==
  /\ Scenario = "application-lifetime"
  /\ AppFlushBase
  /\ UNCHANGED AppOtherVars

AppFlushFails ==
  /\ Scenario = "application-lifetime"
  /\ AppFlushFailsBase
  /\ UNCHANGED AppOtherVars

AppPersistStop ==
  /\ Scenario = "application-lifetime"
  /\ AppPersistStopBase
  /\ UNCHANGED AppOtherVars

AppPersistStopFails ==
  /\ Scenario = "application-lifetime"
  /\ AppPersistStopFailsBase
  /\ UNCHANGED AppOtherVars

AppDispatchClose(resource) ==
  /\ Scenario = "application-lifetime"
  /\ AppDispatchCloseBase(resource)
  /\ UNCHANGED AppOtherVars

AppCloseCompletes(resource) ==
  /\ Scenario = "application-lifetime"
  /\ AppCloseCompletesBase(resource)
  /\ UNCHANGED AppOtherVars

AppCloseFails(resource) ==
  /\ Scenario = "application-lifetime"
  /\ AppCloseFailsBase(resource)
  /\ UNCHANGED AppOtherVars

AppRetry ==
  /\ Scenario = "application-lifetime"
  /\ AppRetryBase
  /\ UNCHANGED AppOtherVars

AppDetectOrphan ==
  /\ Scenario = "application-lifetime"
  /\ AppDetectOrphanBase
  /\ UNCHANGED AppOtherVars

AppFinishStop ==
  /\ Scenario = "application-lifetime"
  /\ AppFinishStopBase
  /\ UNCHANGED AppOtherVars

AppFinishDevelopmentStop ==
  /\ Scenario = "application-lifetime"
  /\ AppFinishDevelopmentStopBase
  /\ UNCHANGED AppOtherVars

AppStageDevelopmentRestart ==
  /\ Scenario = "application-lifetime"
  /\ AppStageDevelopmentRestartBase
  /\ UNCHANGED AppOtherVars

AppPrepareDevelopmentPlan ==
  /\ Scenario = "application-lifetime"
  /\ AppPrepareDevelopmentPlanBase
  /\ UNCHANGED AppOtherVars

AppPrepareDevelopmentPlanFails ==
  /\ Scenario = "application-lifetime"
  /\ AppPrepareDevelopmentPlanFailsBase
  /\ UNCHANGED AppOtherVars

AppQuit ==
  /\ Scenario = "application-lifetime"
  /\ AppQuitBase
  /\ UNCHANGED AppOtherVars

AppQuitAnyway ==
  /\ Scenario = "application-lifetime"
  /\ AppQuitAnywayBase
  /\ UNCHANGED AppOtherVars

BrowserRequestStart(mode) ==
  /\ Scenario = "browser-access"
  /\ BrowserRequestStartBase(mode)
  /\ UNCHANGED BrowserOtherVars

BrowserRequestStop ==
  /\ Scenario = "browser-access"
  /\ BrowserRequestStopBase
  /\ UNCHANGED BrowserOtherVars

BrowserRequestDispose ==
  /\ Scenario = "browser-access"
  /\ BrowserRequestDisposeBase
  /\ UNCHANGED BrowserOtherVars

BrowserBeginStart ==
  /\ Scenario = "browser-access"
  /\ BrowserBeginStartBase
  /\ UNCHANGED BrowserOtherVars

BrowserServerStarted ==
  /\ Scenario = "browser-access"
  /\ BrowserServerStartedBase
  /\ UNCHANGED BrowserOtherVars

BrowserGatewayStarted ==
  /\ Scenario = "browser-access"
  /\ BrowserGatewayStartedBase
  /\ UNCHANGED BrowserOtherVars

BrowserStartFailed ==
  /\ Scenario = "browser-access"
  /\ BrowserStartFailedBase
  /\ UNCHANGED BrowserOtherVars

BrowserBeginStop ==
  /\ Scenario = "browser-access"
  /\ BrowserBeginStopBase
  /\ UNCHANGED BrowserOtherVars

BrowserDisposeStopped ==
  /\ Scenario = "browser-access"
  /\ BrowserDisposeStoppedBase
  /\ UNCHANGED BrowserOtherVars

BrowserCleanupCompleted ==
  /\ Scenario = "browser-access"
  /\ BrowserCleanupCompletedBase
  /\ UNCHANGED BrowserOtherVars

DevelopmentDesktopBuildFails ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentDesktopBuildFailsBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentDesktopBuildSucceeds ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentDesktopBuildSucceedsBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentHostBuildFails ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentHostBuildFailsBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentHostBuildSucceeds ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentHostBuildSucceedsBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentBeginDesktopRestart ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentBeginDesktopRestartBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentElectronStopsForDesktop ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentElectronStopsForDesktopBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentElectronStartsForDesktop ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentElectronStartsForDesktopBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentBeginHostRestart ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentBeginHostRestartBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentPrepareHostRestart ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentPrepareHostRestartBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentAbortHostRestart ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentAbortHostRestartBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentElectronStopsForHost ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentElectronStopsForHostBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentHostStops ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentHostStopsBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentHostStarts ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentHostStartsBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentElectronStartsForHost ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentElectronStartsForHostBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentCompleteHostRestart ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentCompleteHostRestartBase
  /\ UNCHANGED DevelopmentOtherVars

DevelopmentCloseDevelopment ==
  /\ Scenario = "development-runtime"
  /\ DevelopmentCloseDevelopmentBase
  /\ UNCHANGED DevelopmentOtherVars

RendererStartDocumentNavigation ==
  /\ Scenario = "renderer-document"
  /\ RendererStartDocumentNavigationBase
  /\ UNCHANGED RendererOtherVars

RendererStartIgnoredNavigation(navigationKind) ==
  /\ Scenario = "renderer-document"
  /\ RendererStartIgnoredNavigationBase(navigationKind)
  /\ UNCHANGED RendererOtherVars

RendererFinishDocument ==
  /\ Scenario = "renderer-document"
  /\ RendererFinishDocumentBase
  /\ UNCHANGED RendererOtherVars

RendererFinishDocumentWithoutObservedStart ==
  /\ Scenario = "renderer-document"
  /\ RendererFinishDocumentWithoutObservedStartBase
  /\ UNCHANGED RendererOtherVars

WindowSetWindowMode(nextMode) ==
  /\ Scenario = "window-state"
  /\ WindowSetWindowModeBase(nextMode)
  /\ UNCHANGED WindowOtherVars

WindowCapture ==
  /\ Scenario = "window-state"
  /\ WindowCaptureBase
  /\ UNCHANGED WindowOtherVars

WindowProcessNext ==
  /\ Scenario = "window-state"
  /\ WindowProcessNextBase
  /\ UNCHANGED WindowOtherVars

WindowRequestFlush ==
  /\ Scenario = "window-state"
  /\ WindowRequestFlushBase
  /\ UNCHANGED WindowOtherVars

WindowCompleteFlush ==
  /\ Scenario = "window-state"
  /\ WindowCompleteFlushBase
  /\ UNCHANGED WindowOtherVars

WindowDispose ==
  /\ Scenario = "window-state"
  /\ WindowDisposeBase
  /\ UNCHANGED WindowOtherVars

BindingBeginClose(terminal) ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingBeginCloseBase(terminal)
  /\ UNCHANGED BindingOtherVars

BindingRetireProjection(terminal) ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingRetireProjectionBase(terminal)
  /\ UNCHANGED BindingOtherVars

BindingRequestGraceful(terminal) ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingRequestGracefulBase(terminal)
  /\ UNCHANGED BindingOtherVars

BindingExpireGrace(terminal) ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingExpireGraceBase(terminal)
  /\ UNCHANGED BindingOtherVars

BindingRequestForce(terminal) ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingRequestForceBase(terminal)
  /\ UNCHANGED BindingOtherVars

BindingHostExit(terminal) ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingHostExitBase(terminal)
  /\ UNCHANGED BindingOtherVars

BindingHostFail(terminal) ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingHostFailBase(terminal)
  /\ UNCHANGED BindingOtherVars

BindingDispose(terminal) ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingDisposeBase(terminal)
  /\ UNCHANGED BindingOtherVars

BindingRemoveBinding(terminal) ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingRemoveBindingBase(terminal)
  /\ UNCHANGED BindingOtherVars

BindingCrashDesktop ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingCrashDesktopBase
  /\ UNCHANGED BindingOtherVars

BindingRestartDesktop ==
  /\ Scenario = "terminal-binding-close"
  /\ BindingRestartDesktopBase
  /\ UNCHANGED BindingOtherVars

FolderRequestSelection(request, target) ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderRequestSelectionBase(request, target)
  /\ UNCHANGED FolderOtherVars

FolderRejectBusy ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderRejectBusyBase
  /\ UNCHANGED FolderOtherVars

FolderInvalidateTarget ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderInvalidateTargetBase
  /\ UNCHANGED FolderOtherVars

FolderMakeTargetDuplicate ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderMakeTargetDuplicateBase
  /\ UNCHANGED FolderOtherVars

FolderNativeSelects ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderNativeSelectsBase
  /\ UNCHANGED FolderOtherVars

FolderNativeCancels ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderNativeCancelsBase
  /\ UNCHANGED FolderOtherVars

FolderNativeFails ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderNativeFailsBase
  /\ UNCHANGED FolderOtherVars

FolderDeliverResult ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderDeliverResultBase
  /\ UNCHANGED FolderOtherVars

FolderCommitSelected ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderCommitSelectedBase
  /\ UNCHANGED FolderOtherVars

FolderRejectSelected ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderRejectSelectedBase
  /\ UNCHANGED FolderOtherVars

FolderDismissFailure ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderDismissFailureBase
  /\ UNCHANGED FolderOtherVars

FolderRetire ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderRetireBase
  /\ UNCHANGED FolderOtherVars

FolderNativeCompletesAfterRetire ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderNativeCompletesAfterRetireBase
  /\ UNCHANGED FolderOtherVars

FolderRejectUnknownCompletion ==
  /\ Scenario = "project-folder-acquisition"
  /\ FolderRejectUnknownCompletionBase
  /\ UNCHANGED FolderOtherVars

vars == <<FolderOtherVars, FolderVars>>

Init ==
  /\ AppInit
  /\ BrowserInit
  /\ DevelopmentInit
  /\ RendererInit
  /\ WindowInit
  /\ BindingInit
  /\ FolderInit

Next ==
  \/ AppRequestProtectedUserQuit
  \/ AppRequestUnprotectedUserQuit
  \/ AppRequestUpdateRestart
  \/ AppRequestDevelopmentHostRestart
  \/ AppChoosePreserve
  \/ AppChooseStop
  \/ AppCancel
  \/ AppFlush
  \/ AppFlushFails
  \/ AppPersistStop
  \/ AppPersistStopFails
  \/ AppRetry
  \/ AppDetectOrphan
  \/ AppFinishStop
  \/ AppFinishDevelopmentStop
  \/ AppStageDevelopmentRestart
  \/ AppPrepareDevelopmentPlan
  \/ AppPrepareDevelopmentPlanFails
  \/ AppQuit
  \/ AppQuitAnyway
  \/ \E resource \in AppResources: AppDispatchClose(resource)
  \/ \E resource \in AppResources: AppCloseCompletes(resource)
  \/ \E resource \in AppResources: AppCloseFails(resource)
  \/ \E mode \in BrowserModes \ {"none"}: BrowserRequestStart(mode)
  \/ BrowserRequestStop
  \/ BrowserRequestDispose
  \/ BrowserBeginStart
  \/ BrowserServerStarted
  \/ BrowserGatewayStarted
  \/ BrowserStartFailed
  \/ BrowserBeginStop
  \/ BrowserDisposeStopped
  \/ BrowserCleanupCompleted
  \/ DevelopmentDesktopBuildFails
  \/ DevelopmentDesktopBuildSucceeds
  \/ DevelopmentHostBuildFails
  \/ DevelopmentHostBuildSucceeds
  \/ DevelopmentBeginDesktopRestart
  \/ DevelopmentElectronStopsForDesktop
  \/ DevelopmentElectronStartsForDesktop
  \/ DevelopmentBeginHostRestart
  \/ DevelopmentPrepareHostRestart
  \/ DevelopmentAbortHostRestart
  \/ DevelopmentElectronStopsForHost
  \/ DevelopmentHostStops
  \/ DevelopmentHostStarts
  \/ DevelopmentElectronStartsForHost
  \/ DevelopmentCompleteHostRestart
  \/ DevelopmentCloseDevelopment
  \/ RendererStartDocumentNavigation
  \/ \E navigationKind \in RendererNavigationKinds \ {"main-document"}:
       RendererStartIgnoredNavigation(navigationKind)
  \/ RendererFinishDocument
  \/ RendererFinishDocumentWithoutObservedStart
  \/ \E nextMode \in WindowWindowModes: WindowSetWindowMode(nextMode)
  \/ WindowCapture
  \/ WindowProcessNext
  \/ WindowRequestFlush
  \/ WindowCompleteFlush
  \/ WindowDispose
  \/ \E terminal \in Terminals: BindingBeginClose(terminal)
  \/ \E terminal \in Terminals: BindingRetireProjection(terminal)
  \/ \E terminal \in Terminals: BindingRequestGraceful(terminal)
  \/ \E terminal \in Terminals: BindingExpireGrace(terminal)
  \/ \E terminal \in Terminals: BindingRequestForce(terminal)
  \/ \E terminal \in Terminals: BindingHostExit(terminal)
  \/ \E terminal \in Terminals: BindingHostFail(terminal)
  \/ \E terminal \in Terminals: BindingDispose(terminal)
  \/ \E terminal \in Terminals: BindingRemoveBinding(terminal)
  \/ BindingCrashDesktop
  \/ BindingRestartDesktop
  \/ \E request \in FolderRequests, target \in FolderTargets:
       FolderRequestSelection(request, target)
  \/ FolderRejectBusy
  \/ FolderInvalidateTarget
  \/ FolderMakeTargetDuplicate
  \/ FolderNativeSelects
  \/ FolderNativeCancels
  \/ FolderNativeFails
  \/ FolderDeliverResult
  \/ FolderCommitSelected
  \/ FolderRejectSelected
  \/ FolderDismissFailure
  \/ FolderRetire
  \/ FolderNativeCompletesAfterRetire
  \/ FolderRejectUnknownCompletion

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(BrowserBeginStart)
  /\ WF_vars(BrowserServerStarted)
  /\ WF_vars(BrowserGatewayStarted)
  /\ WF_vars(BrowserBeginStop)
  /\ WF_vars(BrowserDisposeStopped)
  /\ WF_vars(BrowserCleanupCompleted)
  /\ WF_vars(DevelopmentBeginDesktopRestart)
  /\ WF_vars(DevelopmentElectronStopsForDesktop)
  /\ WF_vars(DevelopmentElectronStartsForDesktop)
  /\ WF_vars(DevelopmentBeginHostRestart)
  /\ WF_vars(DevelopmentPrepareHostRestart)
  /\ WF_vars(DevelopmentAbortHostRestart)
  /\ WF_vars(DevelopmentElectronStopsForHost)
  /\ WF_vars(DevelopmentHostStops)
  /\ WF_vars(DevelopmentHostStarts)
  /\ WF_vars(DevelopmentElectronStartsForHost)
  /\ WF_vars(DevelopmentCompleteHostRestart)
  /\ WF_vars(RendererFinishDocument)
  /\ WF_vars(WindowProcessNext)
  /\ WF_vars(WindowCompleteFlush)
  /\ WF_vars(WindowDispose)
  /\ WF_vars(FolderDeliverResult)
  /\ WF_vars(FolderCommitSelected)
  /\ WF_vars(FolderRejectSelected)

ScenarioTypeOK == Scenario \in LifecycleScenarios

AppTypeOK ==
  /\ appMode \in AppModes
  /\ appPhase \in AppPhases
  /\ appPrompted \in BOOLEAN
  /\ appTopologyDurable \in BOOLEAN
  /\ appRestartProvisionDurable \in BOOLEAN
  /\ appRestartPlanReady \in BOOLEAN
  /\ appBindings \in [AppResources -> BOOLEAN]
  /\ appCloseIssued \in [AppResources -> BOOLEAN]
  /\ appHostOrphan \in BOOLEAN
  /\ appEffectsStarted \in BOOLEAN
  /\ appForcedQuit \in BOOLEAN
  /\ appQuit \in BOOLEAN
  /\ appWritable \in BOOLEAN
  /\ appPlanWritable \in BOOLEAN
  /\ appLastAction \in AppActions

AppUpdateNeverPrompts == appMode = "update-restart" => ~appPrompted

AppDevelopmentRestartNeverPrompts == appMode = "development-host-restart" => ~appPrompted

AppProtectedUserQuitPrompts ==
  appLastAction = "request-user-protected" =>
    /\ appMode = "user-preserve"
    /\ appPhase = "awaiting-choice"
    /\ appPrompted

AppUnprotectedUserQuitPreservesWithoutPrompt ==
  appLastAction = "request-user-unprotected" =>
    /\ appMode = "user-preserve"
    /\ appPhase = "flushing"
    /\ ~appPrompted

AppCloseRequiresDurableStop ==
  appLastAction = "dispatch-close" => appMode \in AppDestructiveModes /\ appTopologyDurable

AppPreserveNeverClosesBindings ==
  appMode \in {"user-preserve", "update-restart"} => appBindings = appPreviousBindings

AppCancelPrecedesEffects == appLastAction = "cancel" => ~appPreviousEffectsStarted

AppCleanStopQuitHasNoOwnedBinding ==
  appQuit /\ appMode = "user-stop" /\ ~appForcedQuit =>
    \A resource \in AppResources: ~appBindings[resource]

AppCleanDevelopmentRestartIsReady ==
  appQuit /\ appMode = "development-host-restart" /\ ~appForcedQuit =>
    /\ appRestartProvisionDurable
    /\ appRestartPlanReady
    /\ \A resource \in AppResources: ~appBindings[resource]

BrowserTypeOK ==
  /\ browserPhase \in BrowserPhases
  /\ browserActiveMode \in BrowserModes
  /\ browserDesiredMode \in BrowserModes
  /\ browserServerOwned \in BOOLEAN
  /\ browserGatewayOwned \in BOOLEAN
  /\ browserDisposeRequested \in BOOLEAN

BrowserGatewayRequiresServer == browserGatewayOwned => browserServerOwned

BrowserReadyOwnsBothResources ==
  browserPhase = "ready" =>
    /\ browserServerOwned
    /\ browserGatewayOwned
    /\ browserActiveMode # "none"

BrowserInactiveOwnsNoResources ==
  browserPhase \in {"stopped", "disposed"} =>
    /\ browserActiveMode = "none"
    /\ ~browserServerOwned
    /\ ~browserGatewayOwned

BrowserOwnedResourcesHaveOneMode ==
  (browserServerOwned \/ browserGatewayOwned) => browserActiveMode \in BrowserModes \ {"none"}

BrowserDisposedIsTerminal ==
  browserPhase = "disposed" =>
    /\ browserDisposeRequested
    /\ browserDesiredMode = "none"

BrowserDisposeEventuallyRevokes ==
  browserDisposeRequested ~>
    /\ browserPhase = "disposed"
    /\ ~browserServerOwned
    /\ ~browserGatewayOwned

DevelopmentTypeOK ==
  /\ developmentActiveHost \in DevelopmentGenerations \cup {DevelopmentNoGeneration}
  /\ developmentAvailableHost \in DevelopmentGenerations
  /\ developmentBlockedHost \in DevelopmentGenerations
  /\ developmentTargetHost \in DevelopmentGenerations \cup {DevelopmentNoGeneration}
  /\ developmentActiveDesktop \in DevelopmentGenerations
  /\ developmentAvailableDesktop \in DevelopmentGenerations
  /\ developmentElectron \in DevelopmentElectronStates
  /\ developmentHostHasTerminal \in BOOLEAN
  /\ developmentDocumentState \in DevelopmentDocumentStates
  /\ developmentRestartPlan \in DevelopmentPlanStates
  /\ developmentPhase \in DevelopmentPhases
  /\ developmentClosed \in BOOLEAN
  /\ developmentDesktopBuildFailed \in BOOLEAN
  /\ developmentHostBuildFailed \in BOOLEAN
  /\ developmentRestartPreparationAvailable \in BOOLEAN
  /\ developmentLastAction \in DevelopmentActions
  /\ developmentPreviousActiveHost \in DevelopmentGenerations \cup {DevelopmentNoGeneration}
  /\ developmentPreviousElectron \in DevelopmentElectronStates
  /\ developmentPreviousHostHasTerminal \in BOOLEAN
  /\ developmentPreviousDocumentState \in DevelopmentDocumentStates
  /\ developmentPreviousRestartPlan \in DevelopmentPlanStates
  /\ developmentPreviousTargetHost \in DevelopmentGenerations \cup {DevelopmentNoGeneration}

DevelopmentActiveHostWasBuilt ==
  developmentActiveHost = DevelopmentNoGeneration \/ developmentActiveHost <= developmentAvailableHost

DevelopmentRunningElectronRequiresHost ==
  developmentElectron = "running" => developmentActiveHost # DevelopmentNoGeneration

DevelopmentReadyPlanHasDurableRestart ==
  developmentRestartPlan = "ready" =>
    /\ ~developmentHostHasTerminal
    /\ developmentDocumentState \in {"empty", "provision"}

DevelopmentHostStopRequiresReadyPlan ==
  developmentLastAction = "host-stopped" =>
    /\ developmentPreviousRestartPlan = "ready"
    /\ ~developmentPreviousHostHasTerminal

DevelopmentHostActivationUsesPreparedTarget ==
  developmentLastAction = "host-started" =>
    /\ developmentActiveHost = developmentPreviousTargetHost
    /\ developmentActiveHost <= developmentAvailableHost

DevelopmentDesktopRestartPreservesHost ==
  developmentLastAction \in {
    "begin-desktop-restart",
    "electron-stopped-for-desktop",
    "electron-started-for-desktop"
  } =>
    /\ developmentActiveHost = developmentPreviousActiveHost
    /\ developmentHostHasTerminal = developmentPreviousHostHasTerminal
    /\ developmentDocumentState = developmentPreviousDocumentState
    /\ developmentRestartPlan = developmentPreviousRestartPlan

DevelopmentFailedBuildPreservesRuntime ==
  developmentLastAction \in {"desktop-build-failed", "host-build-failed"} =>
    /\ developmentActiveHost = developmentPreviousActiveHost
    /\ developmentElectron = developmentPreviousElectron
    /\ developmentHostHasTerminal = developmentPreviousHostHasTerminal
    /\ developmentDocumentState = developmentPreviousDocumentState
    /\ developmentRestartPlan = developmentPreviousRestartPlan

DevelopmentAbortedHostRestartPreservesRuntime ==
  developmentLastAction = "host-restart-aborted" =>
    /\ developmentActiveHost = developmentPreviousActiveHost
    /\ developmentElectron = developmentPreviousElectron
    /\ developmentHostHasTerminal = developmentPreviousHostHasTerminal
    /\ developmentDocumentState = developmentPreviousDocumentState

DevelopmentClosedOwnsNoProcesses ==
  developmentClosed =>
    /\ developmentPhase = "closed"
    /\ developmentActiveHost = DevelopmentNoGeneration
    /\ developmentElectron = "stopped"
    /\ ~developmentHostHasTerminal

DevelopmentSuccessfulHostBuildEventuallyActivates ==
  []((~developmentClosed /\ developmentRestartPreparationAvailable /\ developmentAvailableHost > developmentActiveHost) ~>
    (developmentClosed \/ developmentActiveHost = developmentAvailableHost))

DevelopmentSuccessfulDesktopBuildEventuallyActivates ==
  []((~developmentClosed /\ developmentAvailableDesktop > developmentActiveDesktop) ~>
    (developmentClosed \/ developmentActiveDesktop = developmentAvailableDesktop))

RendererTypeOK ==
  /\ rendererDocumentGeneration \in 0..MaxGeneration
  /\ rendererPendingGeneration \in 0..RendererNoGeneration
  /\ rendererControlGeneration \in 0..RendererNoGeneration
  /\ rendererLastAction \in RendererActions
  /\ rendererLastNavigationKind \in RendererNavigationKinds
  /\ rendererPreviousDocumentGeneration \in 0..MaxGeneration
  /\ rendererPreviousPendingGeneration \in 0..RendererNoGeneration
  /\ rendererPreviousControlGeneration \in 0..RendererNoGeneration

RendererControlsBelongToCurrentDocument ==
  rendererControlGeneration = RendererNoGeneration \/ rendererControlGeneration = rendererDocumentGeneration

RendererPendingDocumentOwnsNoControls ==
  rendererPendingGeneration = RendererNoGeneration \/
    /\ rendererPendingGeneration = rendererDocumentGeneration
    /\ rendererControlGeneration = RendererNoGeneration

RendererIgnoredNavigationPreservesDocument ==
  rendererLastAction = "ignored-navigation" =>
    /\ rendererDocumentGeneration = rendererPreviousDocumentGeneration
    /\ rendererPendingGeneration = rendererPreviousPendingGeneration
    /\ rendererControlGeneration = rendererPreviousControlGeneration

RendererDocumentStartRetiresPreviousControls ==
  rendererLastAction = "start-document" =>
    /\ rendererPendingGeneration = rendererDocumentGeneration
    /\ rendererControlGeneration = RendererNoGeneration

RendererDocumentFinishPublishesCurrentControls ==
  rendererLastAction = "finish-document" =>
    /\ rendererPendingGeneration = RendererNoGeneration
    /\ rendererControlGeneration = rendererDocumentGeneration

RendererPendingDocumentEventuallyFinishes ==
  rendererPendingGeneration # RendererNoGeneration ~>
    /\ rendererPendingGeneration = RendererNoGeneration
    /\ rendererControlGeneration = rendererDocumentGeneration

WindowTypeOK ==
  /\ windowMode \in WindowWindowModes
  /\ windowCaptures \in Seq(WindowDurableModes)
  /\ windowProcessedCount \in 0..MaxCaptures
  /\ windowDurableOrdinal \in 0..MaxCaptures
  /\ windowFlushRequested \in BOOLEAN
  /\ windowFlushCaptureCount \in (0..MaxCaptures) \union {WindowNoFlushCaptureCount}
  /\ windowFlushed \in BOOLEAN
  /\ windowDisposed \in BOOLEAN
  /\ windowWritable \in BOOLEAN

WindowCaptureBound == Len(windowCaptures) <= MaxCaptures

WindowWritesProcessInOrder ==
  /\ windowProcessedCount <= Len(windowCaptures)
  /\ windowDurableOrdinal <= windowProcessedCount

WindowFullScreenIsTransient ==
  \A ordinal \in 1..Len(windowCaptures): windowCaptures[ordinal] \in WindowDurableModes

WindowWritablePersistsLatest ==
  windowWritable /\ windowProcessedCount > 0 => windowDurableOrdinal = windowProcessedCount

WindowUnavailableStoreNeverAdvances ==
  ~windowWritable => windowDurableOrdinal = 0

WindowFlushFreezesCaptures ==
  windowFlushRequested => Len(windowCaptures) = windowFlushCaptureCount

WindowFlushCompletesQueue ==
  windowFlushed => windowProcessedCount = Len(windowCaptures)

WindowDisposeFollowsFlush == windowDisposed => windowFlushed

WindowFlushEventuallyDisposes == windowFlushRequested ~> windowDisposed

FolderTypeOK ==
  /\ folderClientPhase \in FolderClientPhases
  /\ folderBrokerPhase \in FolderBrokerPhases
  /\ folderIssuedRequests \subseteq FolderRequests
  /\ folderActiveRequest \in {0} \cup FolderRequests
  /\ folderRequestTargets \in [FolderRequests -> FolderTargets \cup {"none"}]
  /\ folderActiveTarget \in FolderTargets \cup {"none"}
  /\ folderTargetAvailable \in BOOLEAN
  /\ folderTargetDuplicate \in BOOLEAN
  /\ folderNativePending \in BOOLEAN
  /\ folderNativeOutcome \in FolderNativeOutcomes
  /\ folderSelectedRequests \subseteq FolderRequests
  /\ folderCancelledRequests \subseteq FolderRequests
  /\ folderFailedRequests \subseteq FolderRequests
  /\ folderDeliveredRequests \subseteq FolderRequests
  /\ folderCommittedRequests \subseteq FolderRequests
  /\ folderCommittedAtRetirement \subseteq FolderRequests
  /\ folderBusyRejected \in BOOLEAN
  /\ folderLateCompletionIgnored \in BOOLEAN
  /\ folderLastAction \in FolderActions

FolderOneActiveRequest ==
  /\ (folderClientPhase \in {"selecting", "selected"} =>
        /\ folderActiveRequest \in FolderRequests
        /\ folderActiveTarget \in FolderTargets)
  /\ (folderClientPhase \in {"idle", "failed", "disposed"} =>
        /\ folderActiveRequest = 0
        /\ folderActiveTarget = "none")
  /\ (folderBrokerPhase = "selecting" => folderClientPhase = "selecting")
  /\ (folderNativePending => folderBrokerPhase \in {"selecting", "disposed"})
  /\ (folderNativeOutcome # "none" =>
        /\ folderBrokerPhase = "selecting"
        /\ ~folderNativePending)

FolderCommitRequiresSelectedDelivery ==
  folderCommittedRequests \subseteq
    (folderSelectedRequests \intersect folderDeliveredRequests)

FolderCancellationAndFailureNeverCommit ==
  /\ (folderCancelledRequests \intersect folderCommittedRequests) = {}
  /\ (folderFailedRequests \intersect folderCommittedRequests) = {}

FolderRetirementRevokesCommit ==
  folderClientPhase = "disposed" =>
    folderCommittedRequests = folderCommittedAtRetirement

FolderCreateTargetRemainsCurrent ==
  folderActiveTarget = "create-project" =>
    /\ folderTargetAvailable
    /\ ~folderTargetDuplicate

FolderResolvedLiveEventuallySettles ==
  (folderClientPhase = "selecting" /\
    folderBrokerPhase = "selecting" /\
    folderNativeOutcome \in {"selected", "cancelled", "failed"}) ~>
      folderClientPhase \in {"idle", "failed", "disposed"}

BindingTypeOK ==
  /\ bindingDesktopActive \in BOOLEAN
  /\ bindingBindingState \in [Terminals -> BindingBindingStates]
  /\ bindingProjectionAttached \in [Terminals -> BOOLEAN]
  /\ bindingTerminalPhase \in [Terminals -> BindingTerminalPhases]
  /\ bindingGracefulRequested \in [Terminals -> BOOLEAN]
  /\ bindingGraceExpired \in [Terminals -> BOOLEAN]
  /\ bindingForceRequested \in [Terminals -> BOOLEAN]

BindingHostMutationRequiresDurableIntent ==
  \A terminal \in Terminals:
    (bindingGracefulRequested[terminal] \/ bindingTerminalPhase[terminal] = "absent") =>
      bindingBindingState[terminal] \in {"closing", "removed"}

BindingHostMutationRequiresRetiredProjection ==
  \A terminal \in Terminals:
    (bindingGracefulRequested[terminal] \/ bindingTerminalPhase[terminal] = "absent") =>
      ~bindingProjectionAttached[terminal]

BindingForceRequiresGraceAndTimeout ==
  \A terminal \in Terminals:
    bindingForceRequested[terminal] =>
      /\ bindingBindingState[terminal] = "closing"
      /\ bindingGracefulRequested[terminal]
      /\ bindingGraceExpired[terminal]
      /\ ~bindingProjectionAttached[terminal]

BindingRemovedBindingNamesNoHostTerminal ==
  \A terminal \in Terminals:
    bindingBindingState[terminal] = "removed" =>
      /\ bindingTerminalPhase[terminal] = "absent"
      /\ ~bindingProjectionAttached[terminal]
      /\ ~bindingGracefulRequested[terminal]
      /\ ~bindingGraceExpired[terminal]
      /\ ~bindingForceRequested[terminal]

BindingTargetedClosePreservesOtherBindings ==
  [][\A terminal \in Terminals:
       BindingBeginClose(terminal) =>
         \A other \in Terminals \ {terminal}:
           UNCHANGED <<bindingBindingState[other], bindingProjectionAttached[other],
                       bindingTerminalPhase[other], bindingGracefulRequested[other],
                       bindingGraceExpired[other], bindingForceRequested[other]>>]_vars

BindingTargetedDisposePreservesOtherBindings ==
  [][\A terminal \in Terminals:
       BindingDispose(terminal) =>
         \A other \in Terminals \ {terminal}:
           UNCHANGED <<bindingBindingState[other], bindingProjectionAttached[other],
                       bindingTerminalPhase[other], bindingGracefulRequested[other],
                       bindingGraceExpired[other], bindingForceRequested[other]>>]_vars

=============================================================================
