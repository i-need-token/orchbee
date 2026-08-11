---------------- MODULE DesktopBrowserAccessLifecycle ----------------
EXTENDS TLC

Modes == {"none", "loopback", "local-network"}
Phases == {
  "stopped",
  "server-starting",
  "gateway-starting",
  "ready",
  "stopping",
  "disposed"
}

VARIABLES
  phase,
  activeMode,
  desiredMode,
  serverOwned,
  gatewayOwned,
  disposeRequested,
  lastAction

vars == <<
  phase,
  activeMode,
  desiredMode,
  serverOwned,
  gatewayOwned,
  disposeRequested,
  lastAction
>>

Init ==
  /\ phase = "stopped"
  /\ activeMode = "none"
  /\ desiredMode = "none"
  /\ serverOwned = FALSE
  /\ gatewayOwned = FALSE
  /\ disposeRequested = FALSE
  /\ lastAction = "init"

RequestStart(mode) ==
  /\ mode \in Modes \ {"none"}
  /\ ~disposeRequested
  /\ desiredMode' = mode
  /\ lastAction' = "request-start"
  /\ UNCHANGED <<phase, activeMode, serverOwned, gatewayOwned, disposeRequested>>

RequestStop ==
  /\ ~disposeRequested
  /\ desiredMode' = "none"
  /\ lastAction' = "request-stop"
  /\ UNCHANGED <<phase, activeMode, serverOwned, gatewayOwned, disposeRequested>>

RequestDispose ==
  /\ ~disposeRequested
  /\ disposeRequested' = TRUE
  /\ desiredMode' = "none"
  /\ lastAction' = "request-dispose"
  /\ UNCHANGED <<phase, activeMode, serverOwned, gatewayOwned>>

BeginStart ==
  /\ phase = "stopped"
  /\ desiredMode \in Modes \ {"none"}
  /\ ~disposeRequested
  /\ phase' = "server-starting"
  /\ activeMode' = desiredMode
  /\ lastAction' = "begin-start"
  /\ UNCHANGED <<desiredMode, serverOwned, gatewayOwned, disposeRequested>>

ServerStarted ==
  /\ phase = "server-starting"
  /\ serverOwned' = TRUE
  /\ phase' = IF disposeRequested \/ desiredMode # activeMode
                THEN "stopping"
                ELSE "gateway-starting"
  /\ lastAction' = "server-started"
  /\ UNCHANGED <<activeMode, desiredMode, gatewayOwned, disposeRequested>>

GatewayStarted ==
  /\ phase = "gateway-starting"
  /\ serverOwned
  /\ gatewayOwned' = TRUE
  /\ phase' = IF disposeRequested \/ desiredMode # activeMode
                THEN "stopping"
                ELSE "ready"
  /\ lastAction' = "gateway-started"
  /\ UNCHANGED <<activeMode, desiredMode, serverOwned, disposeRequested>>

StartFailed ==
  /\ phase \in {"server-starting", "gateway-starting"}
  /\ phase' = "stopping"
  /\ lastAction' = "start-failed"
  /\ UNCHANGED <<activeMode, desiredMode, serverOwned, gatewayOwned, disposeRequested>>

BeginStop ==
  /\ phase = "ready"
  /\ (disposeRequested \/ desiredMode # activeMode)
  /\ phase' = "stopping"
  /\ lastAction' = "begin-stop"
  /\ UNCHANGED <<activeMode, desiredMode, serverOwned, gatewayOwned, disposeRequested>>

DisposeStopped ==
  /\ phase = "stopped"
  /\ disposeRequested
  /\ phase' = "disposed"
  /\ activeMode' = "none"
  /\ lastAction' = "dispose-stopped"
  /\ UNCHANGED <<desiredMode, serverOwned, gatewayOwned, disposeRequested>>

CleanupCompleted ==
  /\ phase = "stopping"
  /\ phase' = IF disposeRequested THEN "disposed" ELSE "stopped"
  /\ activeMode' = "none"
  /\ serverOwned' = FALSE
  /\ gatewayOwned' = FALSE
  /\ lastAction' = "cleanup-completed"
  /\ UNCHANGED <<desiredMode, disposeRequested>>

Next ==
  \/ \E mode \in Modes \ {"none"}: RequestStart(mode)
  \/ RequestStop
  \/ RequestDispose
  \/ BeginStart
  \/ ServerStarted
  \/ GatewayStarted
  \/ StartFailed
  \/ BeginStop
  \/ DisposeStopped
  \/ CleanupCompleted

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(BeginStart)
  /\ WF_vars(ServerStarted)
  /\ WF_vars(GatewayStarted)
  /\ WF_vars(BeginStop)
  /\ WF_vars(DisposeStopped)
  /\ WF_vars(CleanupCompleted)

TypeOK ==
  /\ phase \in Phases
  /\ activeMode \in Modes
  /\ desiredMode \in Modes
  /\ serverOwned \in BOOLEAN
  /\ gatewayOwned \in BOOLEAN
  /\ disposeRequested \in BOOLEAN

GatewayRequiresServer == gatewayOwned => serverOwned

ReadyOwnsBothResources ==
  phase = "ready" =>
    /\ serverOwned
    /\ gatewayOwned
    /\ activeMode # "none"

InactiveOwnsNoResources ==
  phase \in {"stopped", "disposed"} =>
    /\ activeMode = "none"
    /\ ~serverOwned
    /\ ~gatewayOwned

OwnedResourcesHaveOneMode ==
  (serverOwned \/ gatewayOwned) => activeMode \in Modes \ {"none"}

DisposedIsTerminal ==
  phase = "disposed" =>
    /\ disposeRequested
    /\ desiredMode = "none"

DisposeEventuallyRevokes ==
  disposeRequested ~>
    /\ phase = "disposed"
    /\ ~serverOwned
    /\ ~gatewayOwned

=============================================================================
