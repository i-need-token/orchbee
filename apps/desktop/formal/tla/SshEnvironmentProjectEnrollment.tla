---------------- MODULE SshEnvironmentProjectEnrollment ----------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded model of Slice 3.3A's Electron SSH Project enrollment workflow.
\* The append-only Environment registry owns durable identity; the canonical
\* Workbench document alone owns whether the remote Project is enrolled.
\* Renderer requests own only one transient interaction lease. Registry and
\* Workbench commits are deliberately not modeled as one atomic transaction.

CONSTANT Requests, MaxGeneration

NoRequest == "no-request"
NoGeneration == MaxGeneration + 1

RequestKinds == {"none", "enroll", "reconnect"}
RequestPhases == {
  "idle", "busy", "resolving", "registry-writing", "pending",
  "connecting", "interaction", "connected-for-enrollment", "inspecting",
  "ready-to-commit", "committing", "committed", "completed", "failed",
  "cancelled", "outcome-unknown"
}
RegistryStates == {"absent", "present", "indeterminate"}
ConnectionPhases == {
  "disconnected", "connecting", "interaction", "connected",
  "retry-wait", "blocked"
}
PreCommitPhases == {
  "resolving", "registry-writing", "pending", "connecting", "interaction",
  "connected-for-enrollment", "inspecting", "ready-to-commit"
}
Actions == {
  "init", "open-port", "request-enrollment", "reject-busy",
  "resolve-target", "target-resolution-fails", "commit-registry-new",
  "reuse-registry", "registry-precommit-fails",
  "registry-post-rename-indeterminate", "reload-registry-present",
  "reload-registry-absent", "start-connection", "reuse-connection",
  "request-interaction", "submit-interaction", "cancel-interaction",
  "reject-stale-interaction", "connection-ready",
  "connection-ready-without-lease", "connection-fails",
  "inspect-project-root", "inspection-completes", "inspection-fails",
  "begin-workbench-commit", "commit-project-new", "commit-project-existing",
  "workbench-commit-indeterminate", "reconcile-workbench-commit",
  "cancel-before-workbench-commit", "cancel-during-workbench-commit",
  "retire-port-before-commit", "retire-port-during-commit",
  "retire-port-after-commit", "retry-request", "request-reconnect",
  "request-disconnect", "lose-connection", "automatic-retry",
  "block-prompt-without-lease", "activate-initiating-workspace",
  "application-restart"
}

VARIABLES
  ports,
  requestKind,
  requestPhase,
  activeRequest,
  registryState,
  storedProject,
  canonicalProject,
  publishedEnrollment,
  connectionPhase,
  connectedOnce,
  generation,
  promptOwner,
  promptGeneration,
  activatedRequests,
  terminalEffectCount,
  lastAction,
  lastRequest

vars == <<ports, requestKind, requestPhase, activeRequest, registryState,
  storedProject, canonicalProject, publishedEnrollment, connectionPhase,
  connectedOnce, generation, promptOwner, promptGeneration,
  activatedRequests, terminalEffectCount, lastAction, lastRequest>>

Init ==
  /\ ports = {}
  /\ requestKind = [request \in Requests |-> "none"]
  /\ requestPhase = [request \in Requests |-> "idle"]
  /\ activeRequest = NoRequest
  /\ registryState = "absent"
  /\ storedProject = FALSE
  /\ canonicalProject = FALSE
  /\ publishedEnrollment = FALSE
  /\ connectionPhase = "disconnected"
  /\ connectedOnce = FALSE
  /\ generation = 0
  /\ promptOwner = NoRequest
  /\ promptGeneration = NoGeneration
  /\ activatedRequests = {}
  /\ terminalEffectCount = 0
  /\ lastAction = "init"
  /\ lastRequest = NoRequest

OpenPort(request) ==
  /\ request \notin ports
  /\ ports' = ports \cup {request}
  /\ UNCHANGED <<requestKind, requestPhase, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "open-port"
  /\ lastRequest' = request

RequestEnrollment(request) ==
  /\ request \in ports
  /\ requestPhase[request] = "idle"
  /\ activeRequest = NoRequest
  /\ requestKind' = [requestKind EXCEPT ![request] = "enroll"]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "resolving"]
  /\ activeRequest' = request
  /\ UNCHANGED <<ports, registryState, storedProject, canonicalProject,
       publishedEnrollment, connectionPhase, connectedOnce, generation,
       promptOwner, promptGeneration, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "request-enrollment"
  /\ lastRequest' = request

RejectBusy(request) ==
  /\ request \in ports
  /\ requestPhase[request] = "idle"
  /\ activeRequest # NoRequest
  /\ activeRequest # request
  /\ requestKind' = [requestKind EXCEPT ![request] = "enroll"]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "busy"]
  /\ UNCHANGED <<ports, activeRequest, registryState, storedProject,
       canonicalProject, publishedEnrollment, connectionPhase, connectedOnce,
       generation, promptOwner, promptGeneration, activatedRequests,
       terminalEffectCount>>
  /\ lastAction' = "reject-busy"
  /\ lastRequest' = request

ResolveTarget(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "resolving"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "registry-writing"]
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "resolve-target"
  /\ lastRequest' = request

TargetResolutionFails(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "resolving"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, requestKind, registryState, storedProject,
       canonicalProject, publishedEnrollment, connectionPhase, connectedOnce,
       generation, promptOwner, promptGeneration, activatedRequests,
       terminalEffectCount>>
  /\ lastAction' = "target-resolution-fails"
  /\ lastRequest' = request

CommitRegistryNew(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "registry-writing"
  /\ registryState = "absent"
  /\ registryState' = "present"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "pending"]
  /\ UNCHANGED <<ports, requestKind, activeRequest, storedProject,
       canonicalProject, publishedEnrollment, connectionPhase, connectedOnce,
       generation, promptOwner, promptGeneration, activatedRequests,
       terminalEffectCount>>
  /\ lastAction' = "commit-registry-new"
  /\ lastRequest' = request

ReuseRegistry(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "registry-writing"
  /\ registryState = "present"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "pending"]
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "reuse-registry"
  /\ lastRequest' = request

RegistryPrecommitFails(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "registry-writing"
  /\ registryState = "absent"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, requestKind, registryState, storedProject,
       canonicalProject, publishedEnrollment, connectionPhase, connectedOnce,
       generation, promptOwner, promptGeneration, activatedRequests,
       terminalEffectCount>>
  /\ lastAction' = "registry-precommit-fails"
  /\ lastRequest' = request

RegistryPostRenameIndeterminate(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "registry-writing"
  /\ registryState = "absent"
  /\ registryState' = "indeterminate"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "outcome-unknown"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, requestKind, storedProject, canonicalProject,
       publishedEnrollment, connectionPhase, connectedOnce, generation,
       promptOwner, promptGeneration, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "registry-post-rename-indeterminate"
  /\ lastRequest' = request

ReloadRegistryPresent ==
  /\ registryState = "indeterminate"
  /\ registryState' = "present"
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "reload-registry-present"
  /\ lastRequest' = NoRequest

ReloadRegistryAbsent ==
  /\ registryState = "indeterminate"
  /\ registryState' = "absent"
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "reload-registry-absent"
  /\ lastRequest' = NoRequest

StartConnection(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "pending"
  /\ registryState = "present"
  /\ connectionPhase \in {"disconnected", "blocked"}
  /\ generation < MaxGeneration
  /\ connectionPhase' = "connecting"
  /\ generation' = generation + 1
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "connecting"]
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectedOnce,
       promptOwner, promptGeneration, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "start-connection"
  /\ lastRequest' = request

ReuseConnection(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "pending"
  /\ registryState = "present"
  /\ connectionPhase = "connected"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "connected-for-enrollment"]
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "reuse-connection"
  /\ lastRequest' = request

RequestInteraction(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "connecting"
  /\ connectionPhase = "connecting"
  /\ request \in ports
  /\ promptOwner = NoRequest
  /\ connectionPhase' = "interaction"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "interaction"]
  /\ promptOwner' = request
  /\ promptGeneration' = generation
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectedOnce,
       generation, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "request-interaction"
  /\ lastRequest' = request

SubmitInteraction(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "interaction"
  /\ connectionPhase = "interaction"
  /\ promptOwner = request
  /\ promptGeneration = generation
  /\ request \in ports
  /\ connectionPhase' = "connecting"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "connecting"]
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectedOnce,
       generation, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "submit-interaction"
  /\ lastRequest' = request

CancelInteraction(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "interaction"
  /\ connectionPhase = "interaction"
  /\ promptOwner = request
  /\ promptGeneration = generation
  /\ connectionPhase' = "blocked"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<ports, requestKind, registryState, storedProject,
       canonicalProject, publishedEnrollment, connectedOnce, generation,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "cancel-interaction"
  /\ lastRequest' = request

RejectStaleInteraction(request, staleGeneration) ==
  /\ connectionPhase = "interaction"
  /\ staleGeneration \in 0..MaxGeneration
  /\ \/ request # promptOwner
     \/ staleGeneration # promptGeneration
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       registryState, storedProject, canonicalProject, publishedEnrollment,
       connectionPhase, connectedOnce, generation, promptOwner,
       promptGeneration, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "reject-stale-interaction"
  /\ lastRequest' = request

ConnectionReady(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "connecting"
  /\ connectionPhase = "connecting"
  /\ connectionPhase' = "connected"
  /\ connectedOnce' = TRUE
  /\ requestPhase' = [requestPhase EXCEPT
       ![request] = IF requestKind[request] = "enroll"
         THEN "connected-for-enrollment"
         ELSE "completed"]
  /\ activeRequest' = IF requestKind[request] = "enroll" THEN request ELSE NoRequest
  /\ UNCHANGED <<ports, requestKind, registryState, storedProject,
       canonicalProject, publishedEnrollment, generation, promptOwner,
       promptGeneration, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "connection-ready"
  /\ lastRequest' = request

ConnectionReadyWithoutLease ==
  /\ activeRequest = NoRequest
  /\ connectionPhase = "connecting"
  /\ connectionPhase' = "connected"
  /\ connectedOnce' = TRUE
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       registryState, storedProject, canonicalProject, publishedEnrollment,
       generation, promptOwner, promptGeneration, activatedRequests,
       terminalEffectCount>>
  /\ lastAction' = "connection-ready-without-lease"
  /\ lastRequest' = NoRequest

ConnectionFails(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] \in {"connecting", "interaction"}
  /\ connectionPhase \in {"connecting", "interaction"}
  /\ connectionPhase' = "blocked"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<ports, requestKind, registryState, storedProject,
       canonicalProject, publishedEnrollment, connectedOnce, generation,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "connection-fails"
  /\ lastRequest' = request

InspectProjectRoot(request) ==
  /\ activeRequest = request
  /\ requestKind[request] = "enroll"
  /\ requestPhase[request] = "connected-for-enrollment"
  /\ connectionPhase = "connected"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "inspecting"]
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "inspect-project-root"
  /\ lastRequest' = request

InspectionCompletes(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "inspecting"
  /\ connectionPhase = "connected"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "ready-to-commit"]
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "inspection-completes"
  /\ lastRequest' = request

InspectionFails(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "inspecting"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, requestKind, registryState, storedProject,
       canonicalProject, publishedEnrollment, connectionPhase, connectedOnce,
       generation, promptOwner, promptGeneration, activatedRequests,
       terminalEffectCount>>
  /\ lastAction' = "inspection-fails"
  /\ lastRequest' = request

BeginWorkbenchCommit(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "ready-to-commit"
  /\ registryState = "present"
  /\ connectionPhase = "connected"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "committing"]
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "begin-workbench-commit"
  /\ lastRequest' = request

CommitProjectNew(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ ~storedProject
  /\ storedProject' = TRUE
  /\ canonicalProject' = TRUE
  /\ publishedEnrollment' = TRUE
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "committed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, requestKind, registryState, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "commit-project-new"
  /\ lastRequest' = request

CommitProjectExisting(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ storedProject
  /\ canonicalProject' = TRUE
  /\ publishedEnrollment' = TRUE
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "committed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, requestKind, registryState, storedProject,
       connectionPhase, connectedOnce, generation, promptOwner,
       promptGeneration, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "commit-project-existing"
  /\ lastRequest' = request

WorkbenchCommitIndeterminate(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ storedProject' = TRUE
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "outcome-unknown"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, requestKind, registryState, canonicalProject,
       publishedEnrollment, connectionPhase, connectedOnce, generation,
       promptOwner, promptGeneration, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "workbench-commit-indeterminate"
  /\ lastRequest' = request

ReconcileWorkbenchCommit(request) ==
  /\ requestPhase[request] = "outcome-unknown"
  /\ registryState = "present"
  /\ storedProject
  /\ canonicalProject' = TRUE
  /\ publishedEnrollment' = TRUE
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "committed"]
  /\ UNCHANGED <<ports, requestKind, activeRequest, registryState,
       storedProject, connectionPhase, connectedOnce, generation,
       promptOwner, promptGeneration, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "reconcile-workbench-commit"
  /\ lastRequest' = request

CancelBeforeWorkbenchCommit(request) ==
  /\ activeRequest = request
  /\ requestKind[request] = "enroll"
  /\ requestPhase[request] \in PreCommitPhases
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "cancelled"]
  /\ activeRequest' = NoRequest
  /\ connectionPhase' = IF ~storedProject
       THEN "disconnected"
       ELSE IF promptOwner = request THEN "blocked" ELSE connectionPhase
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<ports, requestKind, registryState, storedProject,
       canonicalProject, publishedEnrollment, connectedOnce, generation,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "cancel-before-workbench-commit"
  /\ lastRequest' = request

CancelDuringWorkbenchCommit(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       registryState, storedProject, canonicalProject, publishedEnrollment,
       connectionPhase, connectedOnce, generation, promptOwner,
       promptGeneration, activatedRequests, terminalEffectCount>>
  /\ lastAction' = "cancel-during-workbench-commit"
  /\ lastRequest' = request

RetirePortBeforeCommit(request) ==
  /\ request \in ports
  /\ activeRequest = request
  /\ requestPhase[request] \in PreCommitPhases
  /\ ports' = ports \ {request}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "cancelled"]
  /\ activeRequest' = NoRequest
  /\ connectionPhase' = IF ~storedProject
       THEN "disconnected"
       ELSE IF promptOwner = request THEN "blocked" ELSE connectionPhase
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<requestKind, registryState, storedProject, canonicalProject,
       publishedEnrollment, connectedOnce, generation, activatedRequests,
       terminalEffectCount>>
  /\ lastAction' = "retire-port-before-commit"
  /\ lastRequest' = request

RetirePortDuringCommit(request) ==
  /\ request \in ports
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ ports' = ports \ {request}
  /\ UNCHANGED <<requestKind, requestPhase, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "retire-port-during-commit"
  /\ lastRequest' = request

RetirePortAfterCommit(request) ==
  /\ request \in ports
  /\ requestPhase[request] \in {"committed", "completed", "failed", "busy",
       "cancelled", "outcome-unknown"}
  /\ ports' = ports \ {request}
  /\ UNCHANGED <<requestKind, requestPhase, activeRequest, registryState,
       storedProject, canonicalProject, publishedEnrollment, connectionPhase,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "retire-port-after-commit"
  /\ lastRequest' = request

RetryRequest(request) ==
  /\ request \in ports
  /\ requestPhase[request] \in {"busy", "committed", "completed", "failed",
       "cancelled", "outcome-unknown"}
  /\ requestKind' = [requestKind EXCEPT ![request] = "none"]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "idle"]
  /\ activatedRequests' = activatedRequests \ {request}
  /\ UNCHANGED <<ports, activeRequest, registryState, storedProject,
       canonicalProject, publishedEnrollment, connectionPhase, connectedOnce,
       generation, promptOwner, promptGeneration, terminalEffectCount>>
  /\ lastAction' = "retry-request"
  /\ lastRequest' = request

RequestReconnect(request) ==
  /\ request \in ports
  /\ requestPhase[request] = "idle"
  /\ activeRequest = NoRequest
  /\ registryState = "present"
  /\ canonicalProject
  /\ connectionPhase \in {"disconnected", "blocked"}
  /\ generation < MaxGeneration
  /\ requestKind' = [requestKind EXCEPT ![request] = "reconnect"]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "connecting"]
  /\ activeRequest' = request
  /\ connectionPhase' = "connecting"
  /\ generation' = generation + 1
  /\ UNCHANGED <<ports, registryState, storedProject, canonicalProject,
       publishedEnrollment, connectedOnce, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "request-reconnect"
  /\ lastRequest' = request

RequestDisconnect ==
  /\ registryState = "present"
  /\ connectionPhase # "disconnected"
  /\ activeRequest = NoRequest
  /\ promptOwner = NoRequest
  /\ connectionPhase' = "disconnected"
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       registryState, storedProject, canonicalProject, publishedEnrollment,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "request-disconnect"
  /\ lastRequest' = NoRequest

LoseConnection ==
  /\ connectionPhase = "connected"
  /\ activeRequest = NoRequest
  /\ generation < MaxGeneration
  /\ connectionPhase' = "retry-wait"
  /\ generation' = generation + 1
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       registryState, storedProject, canonicalProject, publishedEnrollment,
       connectedOnce, promptOwner, promptGeneration, activatedRequests,
       terminalEffectCount>>
  /\ lastAction' = "lose-connection"
  /\ lastRequest' = NoRequest

AutomaticRetry ==
  /\ connectionPhase = "retry-wait"
  /\ activeRequest = NoRequest
  /\ connectionPhase' = "connecting"
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       registryState, storedProject, canonicalProject, publishedEnrollment,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "automatic-retry"
  /\ lastRequest' = NoRequest

BlockPromptWithoutLease ==
  /\ connectionPhase = "connecting"
  /\ activeRequest = NoRequest
  /\ promptOwner = NoRequest
  /\ connectionPhase' = "blocked"
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       registryState, storedProject, canonicalProject, publishedEnrollment,
       connectedOnce, generation, promptOwner, promptGeneration,
       activatedRequests, terminalEffectCount>>
  /\ lastAction' = "block-prompt-without-lease"
  /\ lastRequest' = NoRequest

ActivateInitiatingWorkspace(request) ==
  /\ requestPhase[request] = "committed"
  /\ request \notin activatedRequests
  /\ activatedRequests' = activatedRequests \cup {request}
  /\ UNCHANGED <<ports, requestKind, requestPhase, activeRequest,
       registryState, storedProject, canonicalProject, publishedEnrollment,
       connectionPhase, connectedOnce, generation, promptOwner,
       promptGeneration, terminalEffectCount>>
  /\ lastAction' = "activate-initiating-workspace"
  /\ lastRequest' = request

ApplicationRestart ==
  /\ registryState # "indeterminate"
  /\ ports' = {}
  /\ requestKind' = [request \in Requests |-> "none"]
  /\ requestPhase' = [request \in Requests |-> "idle"]
  /\ activeRequest' = NoRequest
  /\ canonicalProject' = storedProject
  /\ publishedEnrollment' = storedProject
  /\ connectionPhase' = "disconnected"
  /\ generation' = IF generation < MaxGeneration THEN generation + 1 ELSE generation
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ activatedRequests' = {}
  /\ UNCHANGED <<registryState, storedProject, connectedOnce,
       terminalEffectCount>>
  /\ lastAction' = "application-restart"
  /\ lastRequest' = NoRequest

Next ==
  \/ \E request \in Requests: OpenPort(request)
  \/ \E request \in Requests: RequestEnrollment(request)
  \/ \E request \in Requests: RejectBusy(request)
  \/ \E request \in Requests: ResolveTarget(request)
  \/ \E request \in Requests: TargetResolutionFails(request)
  \/ \E request \in Requests: CommitRegistryNew(request)
  \/ \E request \in Requests: ReuseRegistry(request)
  \/ \E request \in Requests: RegistryPrecommitFails(request)
  \/ \E request \in Requests: RegistryPostRenameIndeterminate(request)
  \/ ReloadRegistryPresent
  \/ ReloadRegistryAbsent
  \/ \E request \in Requests: StartConnection(request)
  \/ \E request \in Requests: ReuseConnection(request)
  \/ \E request \in Requests: RequestInteraction(request)
  \/ \E request \in Requests: SubmitInteraction(request)
  \/ \E request \in Requests: CancelInteraction(request)
  \/ \E request \in Requests, staleGeneration \in 0..MaxGeneration:
       RejectStaleInteraction(request, staleGeneration)
  \/ \E request \in Requests: ConnectionReady(request)
  \/ ConnectionReadyWithoutLease
  \/ \E request \in Requests: ConnectionFails(request)
  \/ \E request \in Requests: InspectProjectRoot(request)
  \/ \E request \in Requests: InspectionCompletes(request)
  \/ \E request \in Requests: InspectionFails(request)
  \/ \E request \in Requests: BeginWorkbenchCommit(request)
  \/ \E request \in Requests: CommitProjectNew(request)
  \/ \E request \in Requests: CommitProjectExisting(request)
  \/ \E request \in Requests: WorkbenchCommitIndeterminate(request)
  \/ \E request \in Requests: ReconcileWorkbenchCommit(request)
  \/ \E request \in Requests: CancelBeforeWorkbenchCommit(request)
  \/ \E request \in Requests: CancelDuringWorkbenchCommit(request)
  \/ \E request \in Requests: RetirePortBeforeCommit(request)
  \/ \E request \in Requests: RetirePortDuringCommit(request)
  \/ \E request \in Requests: RetirePortAfterCommit(request)
  \/ \E request \in Requests: RetryRequest(request)
  \/ \E request \in Requests: RequestReconnect(request)
  \/ RequestDisconnect
  \/ LoseConnection
  \/ AutomaticRetry
  \/ BlockPromptWithoutLease
  \/ \E request \in Requests: ActivateInitiatingWorkspace(request)
  \/ ApplicationRestart

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ ports \subseteq Requests
  /\ requestKind \in [Requests -> RequestKinds]
  /\ requestPhase \in [Requests -> RequestPhases]
  /\ activeRequest \in Requests \cup {NoRequest}
  /\ registryState \in RegistryStates
  /\ storedProject \in BOOLEAN
  /\ canonicalProject \in BOOLEAN
  /\ publishedEnrollment \in BOOLEAN
  /\ connectionPhase \in ConnectionPhases
  /\ connectedOnce \in BOOLEAN
  /\ generation \in 0..MaxGeneration
  /\ promptOwner \in Requests \cup {NoRequest}
  /\ promptGeneration \in 0..MaxGeneration \cup {NoGeneration}
  /\ activatedRequests \subseteq Requests
  /\ terminalEffectCount \in Nat
  /\ lastAction \in Actions
  /\ lastRequest \in Requests \cup {NoRequest}

RemoteEffectRequiresDurableEnvironment ==
  connectionPhase # "disconnected" => registryState = "present"

WorkbenchReferenceRequiresEnvironment ==
  storedProject => registryState = "present"

CanonicalProjectWasStored ==
  canonicalProject => storedProject

EnrollmentPublicationMatchesCanonicalWorkbench ==
  publishedEnrollment = canonicalProject

InteractionHasExactLiveLease ==
  connectionPhase = "interaction" =>
    /\ promptOwner \in ports
    /\ activeRequest = promptOwner
    /\ requestPhase[promptOwner] = "interaction"
    /\ promptGeneration = generation

NoPromptOutsideInteraction ==
  connectionPhase # "interaction" =>
    /\ promptOwner = NoRequest
    /\ promptGeneration = NoGeneration

CommittedRequestHasCanonicalProject ==
  \A request \in Requests:
    requestPhase[request] = "committed" => canonicalProject

BusyRequestCannotStealLease ==
  lastAction = "reject-busy" =>
    /\ lastRequest # activeRequest
    /\ activeRequest # NoRequest

CancellationDuringCommitDoesNotRollBack ==
  lastAction = "cancel-during-workbench-commit" =>
    /\ activeRequest = lastRequest
    /\ requestPhase[lastRequest] = "committing"

RestartDoesNotAutoConnect ==
  lastAction = "application-restart" =>
    /\ connectionPhase = "disconnected"
    /\ activeRequest = NoRequest
    /\ promptOwner = NoRequest

OnlyCommittedEnrollmentActivatesView ==
  \A request \in activatedRequests: requestPhase[request] = "committed"

EnrollmentCreatesNoTerminal == terminalEffectCount = 0

=============================================================================
