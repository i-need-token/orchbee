---------------- MODULE SshEnvironmentConnectionManagement ----------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded model of reusable SSH Environment management in Electron Settings.
\* Main owns the append-only registry and process-local connection. Each
\* renderer owns one broker request at a time plus independently ordered
\* catalog reads. Discovery and Add never activate SSH or create Project,
\* directory, Workspace, or terminal effects. Only an explicit Connect may
\* start a connection, and only its exact interaction lease may answer a
\* prompt. A newer catalog request fences an older asynchronous completion.

CONSTANT Renderers, MaxCatalogGeneration, Scenario

ASSUME Cardinality(Renderers) = 2
ASSUME MaxCatalogGeneration >= 1
ASSUME Scenario \in {"catalog", "connection"}

NoRenderer == "no-renderer"
NoGeneration == MaxCatalogGeneration + 1
RendererSymmetry == Permutations(Renderers)

RequestKinds == {"none", "add", "connect", "disconnect"}
RequestPhases == {
  "idle", "resolving", "registry-writing", "connecting", "interaction",
  "disconnecting", "completed", "failed", "cancelled"
}
RegistryStates == {"absent", "present", "unavailable"}
ConnectionPhases == {
  "disconnected", "connecting", "interaction", "connected", "blocked"
}
DiscoveryIssues == {"none", "invalid", "partial", "unreadable"}

AddActions == {
  "request-add", "resolve-add", "add-resolution-fails", "commit-add",
  "reuse-add", "registry-write-fails", "cancel-add"
}

CatalogActions == {
  "request-catalog", "reject-stale-catalog", "publish-catalog-healthy",
  "publish-catalog-partial"
}

TerminalRequestPhases == {"completed", "failed", "cancelled"}

VARIABLE s

vars == <<s>>

Init ==
  s = [
    openPorts |-> {},
    requestKind |-> [renderer \in Renderers |-> "none"],
    requestPhase |-> [renderer \in Renderers |-> "idle"],
    registryState |-> "absent",
    connectionPhase |-> "disconnected",
    connectionOwner |-> NoRenderer,
    interactionGeneration |-> 0,
    promptOwner |-> NoRenderer,
    promptGeneration |-> NoGeneration,
    catalogRequested |-> [renderer \in Renderers |-> 0],
    catalogPublished |-> [renderer \in Renderers |-> 0],
    catalogManaged |-> [renderer \in Renderers |-> FALSE],
    catalogConnection |-> [renderer \in Renderers |-> "disconnected"],
    catalogIssue |-> [renderer \in Renderers |-> "none"],
    staleCatalogRejected |-> [renderer \in Renderers |-> 0],
    stalePromptRejected |-> [renderer \in Renderers |-> 0],
    startupObserved |-> FALSE,
    automaticActivationCount |-> 0,
    projectEffectCount |-> 0,
    directoryEffectCount |-> 0,
    workspaceEffectCount |-> 0,
    terminalEffectCount |-> 0,
    registryGrowthSafe |-> TRUE,
    addEffectsSafe |-> TRUE,
    catalogAuthoritySafe |-> TRUE,
    staleCatalogSafe |-> TRUE,
    connectionStartSafe |-> TRUE,
    disconnectSafe |-> TRUE
  ]

Record(next, action, renderer) ==
  [next EXCEPT
    !.registryGrowthSafe = @ /\
      IF s.registryState = "absent" /\ next.registryState = "present"
      THEN action = "commit-add"
      ELSE TRUE,
    !.addEffectsSafe = @ /\
      IF action \in AddActions
      THEN /\ next.connectionPhase = s.connectionPhase
           /\ next.promptOwner = s.promptOwner
           /\ next.projectEffectCount = s.projectEffectCount
           /\ next.directoryEffectCount = s.directoryEffectCount
           /\ next.workspaceEffectCount = s.workspaceEffectCount
           /\ next.terminalEffectCount = s.terminalEffectCount
      ELSE TRUE,
    !.catalogAuthoritySafe = @ /\
      IF action \in CatalogActions
      THEN /\ next.registryState = s.registryState
           /\ next.connectionPhase = s.connectionPhase
           /\ next.promptOwner = s.promptOwner
           /\ next.projectEffectCount = s.projectEffectCount
           /\ next.directoryEffectCount = s.directoryEffectCount
           /\ next.workspaceEffectCount = s.workspaceEffectCount
           /\ next.terminalEffectCount = s.terminalEffectCount
      ELSE TRUE,
    !.staleCatalogSafe = @ /\
      IF action = "reject-stale-catalog"
      THEN next.catalogPublished = s.catalogPublished
      ELSE TRUE,
    !.connectionStartSafe = @ /\
      IF next.connectionPhase = "connecting" /\
           s.connectionPhase # "connecting"
      THEN action \in {"request-connect", "submit-prompt"}
      ELSE TRUE,
    !.disconnectSafe = @ /\
      IF next.connectionPhase = "disconnected" /\
           s.connectionPhase # "disconnected"
      THEN action = "complete-disconnect"
      ELSE TRUE]

OpenPort(renderer) ==
  /\ renderer \notin s.openPorts
  /\ s' = Record(
       [s EXCEPT !.openPorts = @ \cup {renderer}],
       "open-port",
       renderer)

StartupNoOp ==
  /\ Scenario = "catalog"
  /\ ~s.startupObserved
  /\ s' = Record(
       [s EXCEPT !.startupObserved = TRUE],
       "startup-no-op",
       NoRenderer)

RequestCatalog(renderer) ==
  /\ Scenario = "catalog"
  /\ renderer \in s.openPorts
  /\ s.catalogRequested[renderer] < MaxCatalogGeneration
  /\ s' = Record(
       [s EXCEPT !.catalogRequested[renderer] = @ + 1],
       "request-catalog",
       renderer)

RejectStaleCatalog(renderer) ==
  /\ Scenario = "catalog"
  /\ renderer \in s.openPorts
  /\ s.catalogRequested[renderer] >= 2
  /\ s.staleCatalogRejected[renderer] < s.catalogRequested[renderer] - 1
  /\ s' = Record(
       [s EXCEPT
         !.staleCatalogRejected[renderer] = s.catalogRequested[renderer] - 1],
       "reject-stale-catalog",
       renderer)

PublishCatalogHealthy(renderer) ==
  /\ Scenario = "catalog"
  /\ renderer \in s.openPorts
  /\ s.catalogPublished[renderer] < s.catalogRequested[renderer]
  /\ s' = Record(
       [s EXCEPT
         !.catalogPublished[renderer] = s.catalogRequested[renderer],
         !.catalogManaged[renderer] = (s.registryState = "present"),
         !.catalogConnection[renderer] = s.connectionPhase,
         !.catalogIssue[renderer] = "none"],
       "publish-catalog-healthy",
       renderer)

PublishCatalogPartial(renderer) ==
  /\ Scenario = "catalog"
  /\ renderer \in s.openPorts
  /\ s.catalogPublished[renderer] < s.catalogRequested[renderer]
  /\ s' = Record(
       [s EXCEPT
         !.catalogPublished[renderer] = s.catalogRequested[renderer],
         !.catalogManaged[renderer] = (s.registryState = "present"),
         !.catalogConnection[renderer] = s.connectionPhase,
         !.catalogIssue[renderer] = "partial"],
       "publish-catalog-partial",
       renderer)

RequestAdd(renderer) ==
  /\ renderer \in s.openPorts
  /\ s.requestPhase[renderer] = "idle"
  /\ s' = Record(
       [s EXCEPT
         !.requestKind[renderer] = "add",
         !.requestPhase[renderer] = "resolving"],
       "request-add",
       renderer)

ResolveAdd(renderer) ==
  /\ s.requestKind[renderer] = "add"
  /\ s.requestPhase[renderer] = "resolving"
  /\ s' = Record(
       [s EXCEPT !.requestPhase[renderer] = "registry-writing"],
       "resolve-add",
       renderer)

AddResolutionFails(renderer) ==
  /\ s.requestKind[renderer] = "add"
  /\ s.requestPhase[renderer] = "resolving"
  /\ s' = Record(
       [s EXCEPT !.requestPhase[renderer] = "failed"],
       "add-resolution-fails",
       renderer)

CommitAdd(renderer) ==
  /\ s.requestKind[renderer] = "add"
  /\ s.requestPhase[renderer] = "registry-writing"
  /\ s.registryState = "absent"
  /\ s' = Record(
       [s EXCEPT
         !.registryState = "present",
         !.requestPhase[renderer] = "completed"],
       "commit-add",
       renderer)

ReuseAdd(renderer) ==
  /\ s.requestKind[renderer] = "add"
  /\ s.requestPhase[renderer] = "registry-writing"
  /\ s.registryState = "present"
  /\ s' = Record(
       [s EXCEPT !.requestPhase[renderer] = "completed"],
       "reuse-add",
       renderer)

RegistryWriteFails(renderer) ==
  /\ s.requestKind[renderer] = "add"
  /\ s.requestPhase[renderer] = "registry-writing"
  /\ s.registryState \in {"absent", "unavailable"}
  /\ s' = Record(
       [s EXCEPT !.requestPhase[renderer] = "failed"],
       "registry-write-fails",
       renderer)

CancelAdd(renderer) ==
  /\ s.requestKind[renderer] = "add"
  /\ s.requestPhase[renderer] \in {"resolving", "registry-writing"}
  /\ s' = Record(
       [s EXCEPT !.requestPhase[renderer] = "cancelled"],
       "cancel-add",
       renderer)

RequestConnect(renderer) ==
  /\ Scenario = "connection"
  /\ renderer \in s.openPorts
  /\ s.requestPhase[renderer] = "idle"
  /\ s.registryState = "present"
  /\ s.connectionOwner = NoRenderer
  /\ s.connectionPhase \in {"disconnected", "blocked"}
  /\ s.interactionGeneration < MaxCatalogGeneration
  /\ s' = Record(
       [s EXCEPT
         !.requestKind[renderer] = "connect",
         !.requestPhase[renderer] = "connecting",
         !.connectionOwner = renderer,
         !.connectionPhase = "connecting",
         !.interactionGeneration = @ + 1],
       "request-connect",
       renderer)

RejectEnvironmentBusy(renderer) ==
  /\ Scenario = "connection"
  /\ renderer \in s.openPorts
  /\ s.requestPhase[renderer] = "idle"
  /\ s.registryState = "present"
  /\ s.connectionOwner # NoRenderer
  /\ s.connectionOwner # renderer
  /\ s' = Record(
       [s EXCEPT
         !.requestKind[renderer] = "connect",
         !.requestPhase[renderer] = "failed"],
       "reject-environment-busy",
       renderer)

RequestPrompt(renderer) ==
  /\ Scenario = "connection"
  /\ s.connectionOwner = renderer
  /\ s.requestKind[renderer] = "connect"
  /\ s.requestPhase[renderer] = "connecting"
  /\ s.promptOwner = NoRenderer
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[renderer] = "interaction",
         !.connectionPhase = "interaction",
         !.promptOwner = renderer,
         !.promptGeneration = s.interactionGeneration],
       "request-prompt",
       renderer)

SubmitPrompt(renderer) ==
  /\ Scenario = "connection"
  /\ s.connectionOwner = renderer
  /\ s.promptOwner = renderer
  /\ s.promptGeneration = s.interactionGeneration
  /\ s.requestPhase[renderer] = "interaction"
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[renderer] = "connecting",
         !.connectionPhase = "connecting",
         !.promptOwner = NoRenderer,
         !.promptGeneration = NoGeneration],
       "submit-prompt",
       renderer)

RejectStalePrompt(renderer) ==
  /\ Scenario = "connection"
  /\ renderer \in s.openPorts
  /\ s.requestPhase[renderer] = "idle"
  /\ s.promptOwner # NoRenderer
  /\ s.promptOwner # renderer
  /\ s.stalePromptRejected[renderer] < MaxCatalogGeneration
  /\ s' = Record(
       [s EXCEPT !.stalePromptRejected[renderer] = @ + 1],
       "reject-stale-prompt",
       renderer)

ConnectionReady(renderer) ==
  /\ Scenario = "connection"
  /\ s.connectionOwner = renderer
  /\ s.requestKind[renderer] = "connect"
  /\ s.requestPhase[renderer] = "connecting"
  /\ s.connectionPhase = "connecting"
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[renderer] = "completed",
         !.connectionPhase = "connected",
         !.connectionOwner = NoRenderer],
       "connection-ready",
       renderer)

ConnectionFails(renderer) ==
  /\ Scenario = "connection"
  /\ s.connectionOwner = renderer
  /\ s.requestKind[renderer] = "connect"
  /\ s.requestPhase[renderer] \in {"connecting", "interaction"}
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[renderer] = "failed",
         !.connectionPhase = "blocked",
         !.connectionOwner = NoRenderer,
         !.promptOwner = NoRenderer,
         !.promptGeneration = NoGeneration],
       "connection-fails",
       renderer)

CancelConnection(renderer) ==
  /\ Scenario = "connection"
  /\ s.connectionOwner = renderer
  /\ s.requestKind[renderer] = "connect"
  /\ s.requestPhase[renderer] \in {"connecting", "interaction"}
  /\ LET nextConnection ==
       IF s.requestPhase[renderer] = "interaction"
       THEN "blocked"
       ELSE s.connectionPhase
     IN s' = Record(
          [s EXCEPT
            !.requestPhase[renderer] = "cancelled",
            !.connectionPhase = nextConnection,
            !.connectionOwner = NoRenderer,
            !.promptOwner = NoRenderer,
            !.promptGeneration = NoGeneration],
          "cancel-connection",
          renderer)

ConnectionReadyWithoutRequest ==
  /\ Scenario = "connection"
  /\ s.connectionOwner = NoRenderer
  /\ s.connectionPhase = "connecting"
  /\ s' = Record(
       [s EXCEPT !.connectionPhase = "connected"],
       "connection-ready-without-request",
       NoRenderer)

RequestDisconnect(renderer) ==
  /\ Scenario = "connection"
  /\ renderer \in s.openPorts
  /\ s.requestPhase[renderer] = "idle"
  /\ s.registryState = "present"
  /\ s.connectionOwner = NoRenderer
  /\ s.connectionPhase \in {"connected", "disconnected", "blocked"}
  /\ s' = Record(
       [s EXCEPT
         !.requestKind[renderer] = "disconnect",
         !.requestPhase[renderer] = "disconnecting",
         !.connectionOwner = renderer],
       "request-disconnect",
       renderer)

CompleteDisconnect(renderer) ==
  /\ Scenario = "connection"
  /\ s.connectionOwner = renderer
  /\ s.requestKind[renderer] = "disconnect"
  /\ s.requestPhase[renderer] = "disconnecting"
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[renderer] = "completed",
         !.connectionPhase = "disconnected",
         !.connectionOwner = NoRenderer],
       "complete-disconnect",
       renderer)

RetirePort(renderer) ==
  /\ Scenario = "connection"
  /\ renderer \in s.openPorts
  /\ LET ownsConnection == s.connectionOwner = renderer IN
       LET nextConnection ==
         IF ownsConnection /\ s.requestPhase[renderer] = "interaction"
         THEN "blocked"
         ELSE s.connectionPhase
       IN s' = Record(
            [s EXCEPT
              !.openPorts = @ \ {renderer},
              !.requestPhase[renderer] =
                IF @ = "idle" THEN "idle" ELSE "cancelled",
              !.connectionPhase = nextConnection,
              !.connectionOwner =
                IF ownsConnection THEN NoRenderer ELSE @,
              !.promptOwner =
                IF @ = renderer THEN NoRenderer ELSE @,
              !.promptGeneration =
                IF s.promptOwner = renderer THEN NoGeneration ELSE @],
            "retire-port",
            renderer)

ReplacePort(renderer) ==
  /\ Scenario = "connection"
  /\ renderer \notin s.openPorts
  /\ s.requestPhase[renderer] \in TerminalRequestPhases \cup {"idle"}
  /\ s' = Record(
       [s EXCEPT
         !.openPorts = @ \cup {renderer},
         !.requestKind[renderer] = "none",
         !.requestPhase[renderer] = "idle"],
       "replace-port",
       renderer)

ResetRequest(renderer) ==
  /\ renderer \in s.openPorts
  /\ s.requestPhase[renderer] \in TerminalRequestPhases
  /\ s.connectionOwner # renderer
  /\ s.promptOwner # renderer
  /\ s' = Record(
       [s EXCEPT
         !.requestKind[renderer] = "none",
         !.requestPhase[renderer] = "idle"],
       "reset-request",
       renderer)

Next ==
  \/ \E renderer \in Renderers : OpenPort(renderer)
  \/ StartupNoOp
  \/ \E renderer \in Renderers : RequestCatalog(renderer)
  \/ \E renderer \in Renderers : RejectStaleCatalog(renderer)
  \/ \E renderer \in Renderers : PublishCatalogHealthy(renderer)
  \/ \E renderer \in Renderers : PublishCatalogPartial(renderer)
  \/ \E renderer \in Renderers : RequestAdd(renderer)
  \/ \E renderer \in Renderers : ResolveAdd(renderer)
  \/ \E renderer \in Renderers : AddResolutionFails(renderer)
  \/ \E renderer \in Renderers : CommitAdd(renderer)
  \/ \E renderer \in Renderers : ReuseAdd(renderer)
  \/ \E renderer \in Renderers : RegistryWriteFails(renderer)
  \/ \E renderer \in Renderers : CancelAdd(renderer)
  \/ \E renderer \in Renderers : RequestConnect(renderer)
  \/ \E renderer \in Renderers : RejectEnvironmentBusy(renderer)
  \/ \E renderer \in Renderers : RequestPrompt(renderer)
  \/ \E renderer \in Renderers : SubmitPrompt(renderer)
  \/ \E renderer \in Renderers : RejectStalePrompt(renderer)
  \/ \E renderer \in Renderers : ConnectionReady(renderer)
  \/ \E renderer \in Renderers : ConnectionFails(renderer)
  \/ \E renderer \in Renderers : CancelConnection(renderer)
  \/ ConnectionReadyWithoutRequest
  \/ \E renderer \in Renderers : RequestDisconnect(renderer)
  \/ \E renderer \in Renderers : CompleteDisconnect(renderer)
  \/ \E renderer \in Renderers : RetirePort(renderer)
  \/ \E renderer \in Renderers : ReplacePort(renderer)
  \/ \E renderer \in Renderers : ResetRequest(renderer)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.openPorts \subseteq Renderers
  /\ s.requestKind \in [Renderers -> RequestKinds]
  /\ s.requestPhase \in [Renderers -> RequestPhases]
  /\ s.registryState \in RegistryStates
  /\ s.connectionPhase \in ConnectionPhases
  /\ s.connectionOwner \in Renderers \cup {NoRenderer}
  /\ s.interactionGeneration \in 0..MaxCatalogGeneration
  /\ s.promptOwner \in Renderers \cup {NoRenderer}
  /\ s.promptGeneration \in (0..MaxCatalogGeneration) \cup {NoGeneration}
  /\ s.catalogRequested \in [Renderers -> 0..MaxCatalogGeneration]
  /\ s.catalogPublished \in [Renderers -> 0..MaxCatalogGeneration]
  /\ s.catalogManaged \in [Renderers -> BOOLEAN]
  /\ s.catalogConnection \in [Renderers -> ConnectionPhases]
  /\ s.catalogIssue \in [Renderers -> DiscoveryIssues]
  /\ s.staleCatalogRejected \in [Renderers -> 0..MaxCatalogGeneration]
  /\ s.stalePromptRejected \in [Renderers -> 0..MaxCatalogGeneration]
  /\ s.startupObserved \in BOOLEAN
  /\ s.automaticActivationCount \in Nat
  /\ s.projectEffectCount \in Nat
  /\ s.directoryEffectCount \in Nat
  /\ s.workspaceEffectCount \in Nat
  /\ s.terminalEffectCount \in Nat
  /\ s.registryGrowthSafe \in BOOLEAN
  /\ s.addEffectsSafe \in BOOLEAN
  /\ s.catalogAuthoritySafe \in BOOLEAN
  /\ s.staleCatalogSafe \in BOOLEAN
  /\ s.connectionStartSafe \in BOOLEAN
  /\ s.disconnectSafe \in BOOLEAN

AtMostOneEnvironmentOperation ==
  s.connectionOwner # NoRenderer =>
    /\ s.connectionOwner \in s.openPorts
    /\ \/ /\ s.requestKind[s.connectionOwner] = "connect"
          /\ s.requestPhase[s.connectionOwner] \in {"connecting", "interaction"}
       \/ /\ s.requestKind[s.connectionOwner] = "disconnect"
          /\ s.requestPhase[s.connectionOwner] = "disconnecting"

PromptHasExactLiveLease ==
  s.promptOwner # NoRenderer =>
    /\ s.promptOwner \in s.openPorts
    /\ s.connectionOwner = s.promptOwner
    /\ s.requestKind[s.promptOwner] = "connect"
    /\ s.requestPhase[s.promptOwner] = "interaction"
    /\ s.connectionPhase = "interaction"
    /\ s.promptGeneration = s.interactionGeneration

RegistryGrowthRequiresResolvedAdd ==
  s.registryGrowthSafe

CompletedAddRequiresDurableIntent ==
  \A renderer \in Renderers :
    s.requestKind[renderer] = "add" /\
      s.requestPhase[renderer] = "completed" =>
        s.registryState = "present"

AddNeverActivatesOrCreatesResources ==
  s.addEffectsSafe

CatalogReadHasNoAuthority ==
  s.catalogAuthoritySafe

CatalogNeverInventsManagedIntent ==
  \A renderer \in Renderers :
    s.catalogManaged[renderer] => s.registryState = "present"

CatalogPublicationNeverExceedsLatestRequest ==
  \A renderer \in Renderers :
    s.catalogPublished[renderer] <= s.catalogRequested[renderer]

StaleCatalogCompletionCannotPublish ==
  s.staleCatalogSafe

ConnectionStartIsExplicit ==
  s.connectionStartSafe

DisconnectIsExplicit ==
  s.disconnectSafe

NoAutomaticSshActivation == s.automaticActivationCount = 0

NoProjectDirectoryWorkspaceOrTerminalEffect ==
  /\ s.projectEffectCount = 0
  /\ s.directoryEffectCount = 0
  /\ s.workspaceEffectCount = 0
  /\ s.terminalEffectCount = 0

=============================================================================
