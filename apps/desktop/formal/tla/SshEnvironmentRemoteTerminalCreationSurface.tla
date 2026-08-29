---------------- MODULE SshEnvironmentRemoteTerminalCreationSurface ----------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded model of Slice 3.2E. The Electron creation surface carries only a
\* Project/Workspace intent. Main resolves current canonical Environment
\* authority, requires an already-connected Environment, and converges every
\* admitted request on one canonical terminal. Renderer visibility is a later,
\* independently ordered client-session publication. Port loss never invents
\* cancellation, rolls back a possible commit, activates SSH, or falls back to
\* the local runtime.

CONSTANT Requests, Workspaces, Environments

ASSUME Cardinality(Requests) = 2
ASSUME Cardinality(Workspaces) = 1
ASSUME Cardinality(Environments) = 2

NoRequest == "no-request"
NoWorkspace == "no-workspace"
NoEnvironment == "no-environment"
Standalone == "standalone"

RequestPhases == {
  "idle", "sent", "admitted", "resolved", "prepared", "committing",
  "committed", "existing", "settled-created", "settled-existing",
  "cancelled", "failed", "indeterminate"
}
Locations == Workspaces \cup {Standalone}
Targets == Environments \cup {NoEnvironment}
InitialEnvironment == CHOOSE environment \in Environments : TRUE

CanonicalTerminals(state) ==
  {workspace \in Workspaces : state.terminalCount[workspace] = 1}

ExactClientContextIsEmpty(state, workspace) ==
  workspace \notin state.rendererVisible \cup state.unsupportedContext

CreationSurfaceIsAvailable(state, workspace) ==
  /\ state.portOpen
  /\ state.capabilityAvailable
  /\ state.canonicalTarget[workspace] # NoEnvironment
  /\ ExactClientContextIsEmpty(state, workspace)

VARIABLE s

vars == <<s>>

Init ==
  s = [
    portOpen |-> TRUE,
    capabilityAvailable |-> TRUE,
    connected |-> {},
    canonicalTarget |-> [workspace \in Workspaces |-> InitialEnvironment],
    terminalCount |-> [workspace \in Workspaces |-> 0],
    rendererVisible |-> {},
    unsupportedContext |-> {},
    activeRequest |-> NoRequest,
    requestPhase |-> [request \in Requests |-> "idle"],
    requestWorkspace |-> [request \in Requests |-> NoWorkspace],
    resolvedEnvironment |-> [request \in Requests |-> NoEnvironment],
    resolutionTarget |-> [request \in Requests |-> NoEnvironment],
    committedByRequest |-> {},
    committedEnvironment |-> [workspace \in Workspaces |-> NoEnvironment],
    requestedFromAvailable |-> {},
    rejectedUnsupported |-> {},
    rejectedUnavailable |-> {},
    rejectedClientAuthority |-> FALSE,
    localEffectCount |-> 0,
    automaticConnectCount |-> 0,
    activeLocation |-> Standalone,
    previousConnected |-> {},
    previousTerminalCount |-> [workspace \in Workspaces |-> 0],
    previousRendererVisible |-> {},
    previousActiveLocation |-> Standalone,
    lastAction |-> "init"
  ]

Record(next, action) ==
  [next EXCEPT
    !.previousConnected = s.connected,
    !.previousTerminalCount = s.terminalCount,
    !.previousRendererVisible = s.rendererVisible,
    !.previousActiveLocation = s.activeLocation,
    !.lastAction = action]

ExplicitConnect(environment) ==
  /\ environment \notin s.connected
  /\ s' = Record(
       [s EXCEPT !.connected = @ \cup {environment}],
       "explicit-connect")

Disconnect(environment) ==
  /\ environment \in s.connected
  /\ s' = Record(
       [s EXCEPT !.connected = @ \ {environment}],
       "disconnect")

RemoveCapability ==
  /\ s.capabilityAvailable
  /\ s' = Record(
       [s EXCEPT !.capabilityAvailable = FALSE],
       "remove-capability")

InvalidateClientContext(workspace) ==
  /\ workspace \notin CanonicalTerminals(s)
  /\ workspace \notin s.unsupportedContext
  /\ s' = Record(
       [s EXCEPT !.unsupportedContext = @ \cup {workspace}],
       "invalidate-client-context")

RequestDefault(request, workspace) ==
  /\ s.requestPhase[request] = "idle"
  /\ request \notin s.rejectedUnsupported \cup s.rejectedUnavailable
  /\ CreationSurfaceIsAvailable(s, workspace)
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[request] = "sent",
         !.requestWorkspace[request] = workspace,
         !.requestedFromAvailable = @ \cup {request}],
       "request-default")

RejectUnsupportedPlacement(request) ==
  /\ s.requestPhase[request] = "idle"
  /\ request \notin s.rejectedUnsupported \cup s.rejectedUnavailable
  /\ s' = Record(
       [s EXCEPT !.rejectedUnsupported = @ \cup {request}],
       "reject-unsupported-placement")

RejectUnavailableSurface(request, workspace) ==
  /\ s.requestPhase[request] = "idle"
  /\ request \notin s.rejectedUnsupported \cup s.rejectedUnavailable
  /\ ~CreationSurfaceIsAvailable(s, workspace)
  /\ s' = Record(
       [s EXCEPT !.rejectedUnavailable = @ \cup {request}],
       "reject-unavailable-surface")

AdmitRequest(request) ==
  /\ s.activeRequest = NoRequest
  /\ s.requestPhase[request] = "sent"
  /\ s' = Record(
       [s EXCEPT
         !.activeRequest = request,
         !.requestPhase[request] = "admitted"],
       "admit-request")

RejectBusy(request) ==
  /\ s.activeRequest # NoRequest
  /\ s.activeRequest # request
  /\ s.requestPhase[request] = "sent"
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "failed"],
       "reject-busy")

ResolveCanonicalTarget(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] = "admitted"
  /\ s.canonicalTarget[s.requestWorkspace[request]] # NoEnvironment
  /\ LET environment == s.canonicalTarget[s.requestWorkspace[request]] IN
       s' = Record(
         [s EXCEPT
           !.requestPhase[request] = "resolved",
           !.resolvedEnvironment[request] = environment,
           !.resolutionTarget[request] = environment],
         "resolve-canonical-target")

RejectMissingCanonicalTarget(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] = "admitted"
  /\ s.canonicalTarget[s.requestWorkspace[request]] = NoEnvironment
  /\ s' = Record(
       [s EXCEPT
         !.activeRequest = NoRequest,
         !.requestPhase[request] = "failed"],
       "reject-missing-canonical-target")

PrepareRuntime(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] = "resolved"
  /\ s.resolvedEnvironment[request] \in s.connected
  /\ s.canonicalTarget[s.requestWorkspace[request]] =
       s.resolvedEnvironment[request]
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "prepared"],
       "prepare-runtime")

RejectDisconnected(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] = "resolved"
  /\ s.resolvedEnvironment[request] \notin s.connected
  /\ s' = Record(
       [s EXCEPT
         !.activeRequest = NoRequest,
         !.requestPhase[request] = "failed"],
       "reject-disconnected")

ChangeCanonicalTarget(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] \in {"resolved", "prepared"}
  /\ s.terminalCount[s.requestWorkspace[request]] = 0
  /\ \E environment \in Environments :
       /\ environment # s.resolvedEnvironment[request]
       /\ s' = Record(
            [s EXCEPT
              !.canonicalTarget[s.requestWorkspace[request]] = environment],
            "change-canonical-target")

RemoveCanonicalTarget(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] \in {"resolved", "prepared"}
  /\ s.terminalCount[s.requestWorkspace[request]] = 0
  /\ s' = Record(
       [s EXCEPT
         !.canonicalTarget[s.requestWorkspace[request]] = NoEnvironment],
       "remove-canonical-target")

RejectStaleCanonicalTarget(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] \in {"resolved", "prepared"}
  /\ s.canonicalTarget[s.requestWorkspace[request]] #
       s.resolvedEnvironment[request]
  /\ s' = Record(
       [s EXCEPT
         !.activeRequest = NoRequest,
         !.requestPhase[request] = "failed"],
       "reject-stale-canonical-target")

CancelBeforeCommit(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] \in {"admitted", "resolved", "prepared"}
  /\ request \notin s.committedByRequest
  /\ s' = Record(
       [s EXCEPT
         !.activeRequest = NoRequest,
         !.requestPhase[request] = "cancelled"],
       "cancel-before-commit")

BeginCommit(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] = "prepared"
  /\ s.canonicalTarget[s.requestWorkspace[request]] =
       s.resolvedEnvironment[request]
  /\ s.terminalCount[s.requestWorkspace[request]] = 0
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "committing"],
       "begin-commit")

CommitNew(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] = "committing"
  /\ s.canonicalTarget[s.requestWorkspace[request]] =
       s.resolvedEnvironment[request]
  /\ s.terminalCount[s.requestWorkspace[request]] = 0
  /\ s' = Record(
       [s EXCEPT
         !.activeRequest = NoRequest,
         !.requestPhase[request] = "committed",
         !.terminalCount[s.requestWorkspace[request]] = 1,
         !.committedByRequest = @ \cup {request},
         !.committedEnvironment[s.requestWorkspace[request]] =
           s.resolvedEnvironment[request]],
       "commit-new")

ObserveExisting(request) ==
  /\ s.activeRequest = request
  /\ s.requestPhase[request] = "prepared"
  /\ s.canonicalTarget[s.requestWorkspace[request]] =
       s.resolvedEnvironment[request]
  /\ s.terminalCount[s.requestWorkspace[request]] = 1
  /\ s' = Record(
       [s EXCEPT
         !.activeRequest = NoRequest,
         !.requestPhase[request] = "existing"],
       "observe-existing")

SettleCreated(request) ==
  /\ s.requestPhase[request] = "committed"
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "settled-created"],
       "settle-created")

SettleExisting(request) ==
  /\ s.requestPhase[request] = "existing"
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "settled-existing"],
       "settle-existing")

PublishCanonicalTerminal(workspace) ==
  /\ workspace \in CanonicalTerminals(s)
  /\ workspace \notin s.rendererVisible
  /\ s' = Record(
       [s EXCEPT !.rendererVisible = @ \cup {workspace}],
       "publish-canonical-terminal")

RetirePortBeforeCommit(request) ==
  /\ s.portOpen
  /\ s.requestPhase[request] \in {"sent", "admitted", "resolved", "prepared"}
  /\ s' = Record(
       [s EXCEPT
         !.portOpen = FALSE,
         !.activeRequest = IF s.activeRequest = request
           THEN NoRequest ELSE @,
         !.requestPhase[request] = "indeterminate"],
       "retire-port-before-commit")

RetirePortDuringCommit(request) ==
  /\ s.portOpen
  /\ s.activeRequest = request
  /\ s.requestPhase[request] = "committing"
  /\ \/ s' = Record(
          [s EXCEPT
            !.portOpen = FALSE,
            !.activeRequest = NoRequest,
            !.requestPhase[request] = "indeterminate"],
          "retire-port-during-commit")
     \/ /\ s.terminalCount[s.requestWorkspace[request]] = 0
        /\ s' = Record(
             [s EXCEPT
               !.portOpen = FALSE,
               !.activeRequest = NoRequest,
               !.requestPhase[request] = "indeterminate",
               !.terminalCount[s.requestWorkspace[request]] = 1,
               !.committedByRequest = @ \cup {request},
               !.committedEnvironment[s.requestWorkspace[request]] =
                 s.resolvedEnvironment[request]],
             "retire-port-during-commit")

RetirePortAfterCommit(request) ==
  /\ s.portOpen
  /\ s.requestPhase[request] \in {"committed", "existing"}
  /\ s' = Record(
       [s EXCEPT
         !.portOpen = FALSE,
         !.requestPhase[request] = "indeterminate"],
       "retire-port-after-commit")

ReplacePort ==
  /\ ~s.portOpen
  /\ s' = Record(
       [s EXCEPT !.portOpen = TRUE],
       "replace-port")

RetryRequest(previous, fresh) ==
  /\ previous # fresh
  /\ s.requestPhase[previous] = "indeterminate"
  /\ s.requestPhase[fresh] = "idle"
  /\ fresh \notin s.rejectedUnsupported \cup s.rejectedUnavailable
  /\ CreationSurfaceIsAvailable(s, s.requestWorkspace[previous])
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[fresh] = "sent",
         !.requestWorkspace[fresh] = s.requestWorkspace[previous],
         !.requestedFromAvailable = @ \cup {fresh}],
       "retry-request")

ChangeClientLocation(location) ==
  /\ location \in Locations
  /\ location # s.activeLocation
  /\ s' = Record(
       [s EXCEPT !.activeLocation = location],
       "change-client-location")

RejectClientAuthority ==
  /\ ~s.rejectedClientAuthority
  /\ s' = Record(
       [s EXCEPT !.rejectedClientAuthority = TRUE],
       "reject-client-authority")

Next ==
  \/ \E environment \in Environments : ExplicitConnect(environment)
  \/ \E environment \in Environments : Disconnect(environment)
  \/ RemoveCapability
  \/ \E workspace \in Workspaces : InvalidateClientContext(workspace)
  \/ \E request \in Requests, workspace \in Workspaces :
       RequestDefault(request, workspace)
  \/ \E request \in Requests : RejectUnsupportedPlacement(request)
  \/ \E request \in Requests, workspace \in Workspaces :
       RejectUnavailableSurface(request, workspace)
  \/ \E request \in Requests : AdmitRequest(request)
  \/ \E request \in Requests : RejectBusy(request)
  \/ \E request \in Requests : ResolveCanonicalTarget(request)
  \/ \E request \in Requests : RejectMissingCanonicalTarget(request)
  \/ \E request \in Requests : PrepareRuntime(request)
  \/ \E request \in Requests : RejectDisconnected(request)
  \/ \E request \in Requests : ChangeCanonicalTarget(request)
  \/ \E request \in Requests : RemoveCanonicalTarget(request)
  \/ \E request \in Requests : RejectStaleCanonicalTarget(request)
  \/ \E request \in Requests : CancelBeforeCommit(request)
  \/ \E request \in Requests : BeginCommit(request)
  \/ \E request \in Requests : CommitNew(request)
  \/ \E request \in Requests : ObserveExisting(request)
  \/ \E request \in Requests : SettleCreated(request)
  \/ \E request \in Requests : SettleExisting(request)
  \/ \E workspace \in Workspaces : PublishCanonicalTerminal(workspace)
  \/ \E request \in Requests : RetirePortBeforeCommit(request)
  \/ \E request \in Requests : RetirePortDuringCommit(request)
  \/ \E request \in Requests : RetirePortAfterCommit(request)
  \/ ReplacePort
  \/ \E previous \in Requests, fresh \in Requests :
       RetryRequest(previous, fresh)
  \/ \E location \in Locations : ChangeClientLocation(location)
  \/ RejectClientAuthority

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.portOpen \in BOOLEAN
  /\ s.capabilityAvailable \in BOOLEAN
  /\ s.connected \subseteq Environments
  /\ s.canonicalTarget \in [Workspaces -> Targets]
  /\ s.terminalCount \in [Workspaces -> 0..1]
  /\ s.rendererVisible \subseteq Workspaces
  /\ s.unsupportedContext \subseteq Workspaces
  /\ s.activeRequest \in Requests \cup {NoRequest}
  /\ s.requestPhase \in [Requests -> RequestPhases]
  /\ s.requestWorkspace \in [Requests -> Workspaces \cup {NoWorkspace}]
  /\ s.resolvedEnvironment \in [Requests -> Targets]
  /\ s.resolutionTarget \in [Requests -> Targets]
  /\ s.committedByRequest \subseteq Requests
  /\ s.committedEnvironment \in [Workspaces -> Targets]
  /\ s.requestedFromAvailable \subseteq Requests
  /\ s.rejectedUnsupported \subseteq Requests
  /\ s.rejectedUnavailable \subseteq Requests
  /\ s.rejectedClientAuthority \in BOOLEAN
  /\ s.localEffectCount \in Nat
  /\ s.automaticConnectCount \in Nat
  /\ s.activeLocation \in Locations
  /\ s.previousConnected \subseteq Environments
  /\ s.previousTerminalCount \in [Workspaces -> 0..1]
  /\ s.previousRendererVisible \subseteq Workspaces
  /\ s.previousActiveLocation \in Locations

AtMostOneCanonicalTerminalPerWorkspace ==
  \A workspace \in Workspaces : s.terminalCount[workspace] <= 1

TerminalRequiresCanonicalEnvironment ==
  \A workspace \in CanonicalTerminals(s) :
    /\ s.canonicalTarget[workspace] \in Environments
    /\ s.committedEnvironment[workspace] = s.canonicalTarget[workspace]

MainDerivesEnvironmentFromCanonicalTarget ==
  \A request \in Requests :
    s.resolvedEnvironment[request] # NoEnvironment =>
      s.resolvedEnvironment[request] = s.resolutionTarget[request]

CancelledMeansNoCommit ==
  {request \in Requests : s.requestPhase[request] = "cancelled"} \cap
    s.committedByRequest = {}

PortRetirementNeverRollsBack ==
  \A workspace \in Workspaces :
    s.previousTerminalCount[workspace] <= s.terminalCount[workspace]

RendererVisibilityRequiresCanonicalPublication ==
  s.rendererVisible \subseteq CanonicalTerminals(s)

RendererVisibilityChangesOnlyOnCanonicalPublication ==
  s.rendererVisible # s.previousRendererVisible =>
    s.lastAction = "publish-canonical-terminal"

LateCompletionPreservesActiveLocation ==
  s.lastAction \in {
    "settle-created", "settle-existing", "publish-canonical-terminal",
    "retire-port-after-commit", "retire-port-during-commit"
  } => s.activeLocation = s.previousActiveLocation

UnsupportedPlacementNeverAdmitted ==
  s.rejectedUnsupported \cap s.requestedFromAvailable = {}

AvailabilityAdmissionIsSound ==
  {request \in Requests : s.requestPhase[request] # "idle"} \subseteq
    s.requestedFromAvailable

ConnectionGrowthIsExplicit ==
  s.connected \ s.previousConnected # {} => s.lastAction = "explicit-connect"

NoAutomaticSshActivation == s.automaticConnectCount = 0

NoLocalFallback == s.localEffectCount = 0

=============================================================================
