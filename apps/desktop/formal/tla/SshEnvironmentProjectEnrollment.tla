---------------- MODULE SshEnvironmentProjectEnrollment ----------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded enrollment workflow for one fixed arbitrary registered SSH
\* Environment. Requests are two competing renderer ports; a monotonically
\* changing incarnation distinguishes successive operations on each port.
\* Registry and Workbench durability are independent. Canonical Workbench
\* commit, client-session publication, and initiating-view activation are
\* separate authority boundaries.
\*
\* Connection and project-root models remain independent refinements. An
\* in-flight attachment loss is represented here by the current listing or
\* inspection failure followed, once the workflow is idle, by LoseConnection.

CONSTANT Requests, HandleVariant, MaxGeneration, MaxDirectoryRevision,
  MaxRequestIncarnation

ASSUME Cardinality(Requests) = 2
ASSUME HandleVariant \in {"singleton", "pair"}
ASSUME MaxGeneration >= 1
ASSUME MaxDirectoryRevision >= 1
ASSUME MaxRequestIncarnation >= 1

Handles ==
  IF HandleVariant = "pair"
  THEN {"handle-a", "handle-b"}
  ELSE {"handle-a"}

NoRequest == "no-request"
NoHandle == "no-handle"
NoGeneration == MaxGeneration + 1
NoIncarnation == MaxRequestIncarnation + 1

ASSUME NoRequest \notin Requests
ASSUME NoHandle \notin Handles

PortSymmetry == Permutations(Requests)

RequestKinds == {"none", "enroll", "reconnect"}
RequestPhases == {
  "idle", "busy", "resolving", "registry-writing", "pending",
  "connecting", "interaction", "connected-for-enrollment", "inspecting",
  "choosing-directory", "directory-selected", "ready-to-commit",
  "committing", "committed-unpublished", "committed", "completed", "failed",
  "cancelled", "registry-outcome-unknown", "workbench-outcome-unknown"
}
RegistryStates == {"absent", "present", "indeterminate"}
ConnectionPhases == {
  "disconnected", "connecting", "interaction", "connected",
  "retry-wait", "blocked"
}
ConnectionOrigins == {"none", "existing", "request-started", "shared"}
PreCommitPhases == {
  "resolving", "registry-writing", "pending", "connecting", "interaction",
  "connected-for-enrollment", "choosing-directory", "directory-selected",
  "inspecting", "ready-to-commit"
}
TerminalPhases == {
  "busy", "committed", "completed", "failed", "cancelled",
  "registry-outcome-unknown", "workbench-outcome-unknown"
}

VARIABLE s

vars == <<s>>

Init ==
  s = [
    ports |-> {},
    requestKind |-> [request \in Requests |-> "none"],
    requestPhase |-> [request \in Requests |-> "idle"],
    requestIncarnation |-> [request \in Requests |-> 0],
    activeRequest |-> NoRequest,
    activeIncarnation |-> NoIncarnation,
    registryState |-> "absent",
    storedProject |-> FALSE,
    canonicalProject |-> FALSE,
    publishedEnrollment |-> FALSE,
    publishedIncarnation |-> [request \in Requests |-> 0],
    connectionPhase |-> "disconnected",
    connectionOrigin |-> "none",
    connectionOriginOwner |-> NoRequest,
    connectionOriginIncarnation |-> NoIncarnation,
    generation |-> 0,
    promptOwner |-> NoRequest,
    promptIncarnation |-> NoIncarnation,
    promptGeneration |-> NoGeneration,
    promptHandle |-> NoHandle,
    directoryRevision |-> [request \in Requests |-> 0],
    directoryAuthority |-> [request \in Requests |-> 0],
    selectedDirectoryAuthority |-> [request \in Requests |-> 0],
    activationLeaseIncarnation |-> [request \in Requests |-> 0],
    activatedIncarnation |-> [request \in Requests |-> 0],
    terminalEffectCount |-> 0,
    promptAcceptanceSafe |-> TRUE,
    directoryAcceptanceSafe |-> TRUE,
    directoryFailureSafe |-> TRUE,
    commitPublicationSafe |-> TRUE,
    publicationAuthoritySafe |-> TRUE,
    activationAuthoritySafe |-> TRUE,
    connectionCleanupSafe |-> TRUE
  ]

IsActive(request) ==
  /\ s.activeRequest = request
  /\ s.activeIncarnation = s.requestIncarnation[request]
  /\ s.activeIncarnation \in 1..MaxRequestIncarnation

OwnsStartedConnection(request) ==
  /\ s.connectionOrigin = "request-started"
  /\ s.connectionOriginOwner = request
  /\ s.connectionOriginIncarnation = s.requestIncarnation[request]

CleanupConnectionPhase(request) ==
  IF OwnsStartedConnection(request) /\ ~s.storedProject
  THEN "disconnected"
  ELSE IF s.promptOwner = request /\
          s.promptIncarnation = s.requestIncarnation[request]
       THEN "blocked"
       ELSE s.connectionPhase

CleanupConnectionOrigin(request) ==
  IF OwnsStartedConnection(request)
  THEN IF s.storedProject THEN "shared" ELSE "none"
  ELSE s.connectionOrigin

CleanupConnectionOwner(request) ==
  IF OwnsStartedConnection(request) THEN NoRequest
  ELSE s.connectionOriginOwner

CleanupConnectionIncarnation(request) ==
  IF OwnsStartedConnection(request) THEN NoIncarnation
  ELSE s.connectionOriginIncarnation

FinishPreCommitCleanup(next, request) ==
  [next EXCEPT
    !.connectionCleanupSafe = @ /\
      IF OwnsStartedConnection(request) /\ ~s.storedProject
      THEN /\ next.connectionPhase = "disconnected"
           /\ next.connectionOrigin = "none"
           /\ next.connectionOriginOwner = NoRequest
      ELSE IF s.connectionOrigin = "existing"
      THEN /\ next.connectionPhase = s.connectionPhase
           /\ next.connectionOrigin = "existing"
      ELSE IF OwnsStartedConnection(request) /\ s.storedProject
      THEN /\ next.connectionPhase # "disconnected"
           /\ next.connectionOrigin = "shared"
           /\ next.connectionOriginOwner = NoRequest
      ELSE TRUE]

FinishCanonicalCommit(next, request) ==
  [next EXCEPT
    !.commitPublicationSafe = @
      /\ next.canonicalProject
      /\ next.requestPhase[request] = "committed-unpublished"
      /\ next.publishedIncarnation[request] #
           s.requestIncarnation[request]]

OpenPort(request) ==
  /\ request \notin s.ports
  /\ s' = [s EXCEPT !.ports = @ \cup {request}]

RequestEnrollment(request) ==
  /\ request \in s.ports
  /\ s.requestPhase[request] = "idle"
  /\ s.activeRequest = NoRequest
  /\ s.requestIncarnation[request] < MaxRequestIncarnation
  /\ LET nextIncarnation == s.requestIncarnation[request] + 1
     IN s' = [s EXCEPT
          !.requestKind[request] = "enroll",
          !.requestPhase[request] = "resolving",
          !.requestIncarnation[request] = nextIncarnation,
          !.activeRequest = request,
          !.activeIncarnation = nextIncarnation,
          !.publishedIncarnation[request] = 0,
          !.directoryRevision[request] = 0,
          !.directoryAuthority[request] = 0,
          !.selectedDirectoryAuthority[request] = 0,
          !.activationLeaseIncarnation[request] = nextIncarnation,
          !.activatedIncarnation[request] = 0]

RejectBusy(request) ==
  /\ request \in s.ports
  /\ s.requestPhase[request] = "idle"
  /\ s.activeRequest # NoRequest
  /\ s.activeRequest # request
  /\ s.requestIncarnation[request] < MaxRequestIncarnation
  /\ s' = [s EXCEPT
       !.requestKind[request] = "enroll",
       !.requestPhase[request] = "busy",
       !.requestIncarnation[request] = @ + 1,
       !.publishedIncarnation[request] = 0,
       !.directoryRevision[request] = 0,
       !.directoryAuthority[request] = 0,
       !.selectedDirectoryAuthority[request] = 0,
       !.activationLeaseIncarnation[request] = 0,
       !.activatedIncarnation[request] = 0]

ResolveTarget(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "resolving"
  /\ s' = [s EXCEPT !.requestPhase[request] = "registry-writing"]

TargetResolutionFails(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "resolving"
  /\ s' = [s EXCEPT
       !.requestPhase[request] = "failed",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.activationLeaseIncarnation[request] = 0]

CommitRegistryNew(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "registry-writing"
  /\ s.registryState = "absent"
  /\ s' = [s EXCEPT
       !.registryState = "present",
       !.requestPhase[request] = "pending"]

ReuseRegistry(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "registry-writing"
  /\ s.registryState = "present"
  /\ s' = [s EXCEPT !.requestPhase[request] = "pending"]

RegistryPrecommitFails(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "registry-writing"
  /\ s.registryState = "absent"
  /\ s' = [s EXCEPT
       !.requestPhase[request] = "failed",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.activationLeaseIncarnation[request] = 0]

RegistryPostRenameIndeterminate(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "registry-writing"
  /\ s.registryState = "absent"
  /\ s' = [s EXCEPT
       !.registryState = "indeterminate",
       !.requestPhase[request] = "registry-outcome-unknown",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.activationLeaseIncarnation[request] = 0]

ReloadRegistryPresent ==
  /\ s.registryState = "indeterminate"
  /\ s' = [s EXCEPT !.registryState = "present"]

ReloadRegistryAbsent ==
  /\ s.registryState = "indeterminate"
  /\ s' = [s EXCEPT !.registryState = "absent"]

ObserveExistingConnection ==
  /\ s.ports # {}
  /\ s.activeRequest = NoRequest
  /\ s.registryState = "present"
  /\ s.connectionPhase = "disconnected"
  /\ s.generation < MaxGeneration
  /\ s' = [s EXCEPT
       !.connectionPhase = "connected",
       !.connectionOrigin = "existing",
       !.connectionOriginOwner = NoRequest,
       !.connectionOriginIncarnation = NoIncarnation,
       !.generation = @ + 1]

StartConnection(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "pending"
  /\ s.registryState = "present"
  /\ s.connectionPhase \in {"disconnected", "blocked"}
  /\ s.generation < MaxGeneration
  /\ s' = [s EXCEPT
       !.connectionPhase = "connecting",
       !.connectionOrigin = "request-started",
       !.connectionOriginOwner = request,
       !.connectionOriginIncarnation = s.requestIncarnation[request],
       !.generation = @ + 1,
       !.requestPhase[request] = "connecting"]

ReuseConnection(request) ==
  /\ IsActive(request)
  /\ s.requestKind[request] = "enroll"
  /\ s.requestPhase[request] = "pending"
  /\ s.registryState = "present"
  /\ s.connectionPhase = "connected"
  /\ s.connectionOrigin \in {"existing", "shared"}
  /\ s' = [s EXCEPT
       !.requestPhase[request] = "connected-for-enrollment"]

RequestInteraction(request, handle) ==
  /\ IsActive(request)
  /\ handle \in Handles
  /\ s.requestPhase[request] = "connecting"
  /\ s.connectionPhase = "connecting"
  /\ request \in s.ports
  /\ s.promptOwner = NoRequest
  /\ s' = [s EXCEPT
       !.connectionPhase = "interaction",
       !.requestPhase[request] = "interaction",
       !.promptOwner = request,
       !.promptIncarnation = s.requestIncarnation[request],
       !.promptGeneration = s.generation,
       !.promptHandle = handle]

SubmitInteraction(request, responseIncarnation, responseGeneration, responseHandle) ==
  /\ IsActive(request)
  /\ responseIncarnation \in 1..MaxRequestIncarnation
  /\ responseGeneration \in 1..MaxGeneration
  /\ responseHandle \in Handles
  /\ s.requestPhase[request] = "interaction"
  /\ s.connectionPhase = "interaction"
  /\ s.promptOwner = request
  /\ responseIncarnation = s.promptIncarnation
  /\ responseIncarnation = s.requestIncarnation[request]
  /\ responseGeneration = s.promptGeneration
  /\ responseGeneration = s.generation
  /\ responseHandle = s.promptHandle
  /\ request \in s.ports
  /\ s' = [s EXCEPT
       !.connectionPhase = "connecting",
       !.requestPhase[request] = "connecting",
       !.promptOwner = NoRequest,
       !.promptIncarnation = NoIncarnation,
       !.promptGeneration = NoGeneration,
       !.promptHandle = NoHandle,
       !.promptAcceptanceSafe = @
         /\ responseIncarnation = s.promptIncarnation
         /\ responseIncarnation = s.requestIncarnation[request]
         /\ responseGeneration = s.promptGeneration
         /\ responseGeneration = s.generation
         /\ responseHandle = s.promptHandle]

CancelInteraction(request, responseIncarnation, responseGeneration, responseHandle) ==
  /\ IsActive(request)
  /\ responseIncarnation \in 1..MaxRequestIncarnation
  /\ responseGeneration \in 1..MaxGeneration
  /\ responseHandle \in Handles
  /\ s.requestPhase[request] = "interaction"
  /\ s.connectionPhase = "interaction"
  /\ s.promptOwner = request
  /\ responseIncarnation = s.promptIncarnation
  /\ responseIncarnation = s.requestIncarnation[request]
  /\ responseGeneration = s.promptGeneration
  /\ responseGeneration = s.generation
  /\ responseHandle = s.promptHandle
  /\ LET next == [s EXCEPT
       !.connectionPhase = CleanupConnectionPhase(request),
       !.connectionOrigin = CleanupConnectionOrigin(request),
       !.connectionOriginOwner = CleanupConnectionOwner(request),
       !.connectionOriginIncarnation = CleanupConnectionIncarnation(request),
       !.requestPhase[request] = "failed",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.promptOwner = NoRequest,
       !.promptIncarnation = NoIncarnation,
       !.promptGeneration = NoGeneration,
       !.promptHandle = NoHandle,
       !.activationLeaseIncarnation[request] = 0,
       !.promptAcceptanceSafe = @
         /\ responseIncarnation = s.promptIncarnation
         /\ responseIncarnation = s.requestIncarnation[request]
         /\ responseGeneration = s.promptGeneration
         /\ responseGeneration = s.generation
         /\ responseHandle = s.promptHandle]
     IN s' = FinishPreCommitCleanup(next, request)

RejectStaleInteraction(request, responseIncarnation, responseGeneration, responseHandle) ==
  /\ s.connectionPhase = "interaction"
  /\ responseIncarnation \in 1..MaxRequestIncarnation
  /\ responseGeneration \in 1..MaxGeneration
  /\ responseHandle \in Handles
  /\ \/ request # s.promptOwner
     \/ responseIncarnation # s.promptIncarnation
     \/ responseGeneration # s.promptGeneration
     \/ responseHandle # s.promptHandle
  /\ UNCHANGED s

ConnectionReady(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "connecting"
  /\ s.connectionPhase = "connecting"
  /\ LET enrollment == s.requestKind[request] = "enroll"
     IN s' = [s EXCEPT
          !.connectionPhase = "connected",
          !.requestPhase[request] =
            IF enrollment THEN "connected-for-enrollment" ELSE "completed",
          !.activeRequest = IF enrollment THEN request ELSE NoRequest,
          !.activeIncarnation =
            IF enrollment THEN s.requestIncarnation[request]
            ELSE NoIncarnation,
          !.activationLeaseIncarnation[request] =
            IF enrollment THEN @ ELSE 0]

ConnectionReadyWithoutLease ==
  /\ s.activeRequest = NoRequest
  /\ s.connectionPhase = "connecting"
  /\ s.connectionOrigin \in {"existing", "shared"}
  /\ s' = [s EXCEPT !.connectionPhase = "connected"]

ConnectionFails(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] \in {"connecting", "interaction"}
  /\ s.connectionPhase \in {"connecting", "interaction"}
  /\ s' = [s EXCEPT
       !.connectionPhase = "blocked",
       !.connectionOrigin = IF s.storedProject THEN "shared" ELSE "none",
       !.connectionOriginOwner = NoRequest,
       !.connectionOriginIncarnation = NoIncarnation,
       !.requestPhase[request] = "failed",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.promptOwner = NoRequest,
       !.promptIncarnation = NoIncarnation,
       !.promptGeneration = NoGeneration,
       !.promptHandle = NoHandle,
       !.activationLeaseIncarnation[request] = 0]

PublishInitialDirectory(request) ==
  /\ IsActive(request)
  /\ s.requestKind[request] = "enroll"
  /\ s.requestPhase[request] = "connected-for-enrollment"
  /\ s.connectionPhase = "connected"
  /\ request \in s.ports
  /\ s.directoryRevision[request] = 0
  /\ s' = [s EXCEPT
       !.requestPhase[request] = "choosing-directory",
       !.directoryRevision[request] = 1,
       !.directoryAuthority[request] = 1]

InitialDirectoryListingFails(request) ==
  /\ IsActive(request)
  /\ s.requestKind[request] = "enroll"
  /\ s.requestPhase[request] = "connected-for-enrollment"
  /\ s.connectionPhase = "connected"
  /\ LET next == [s EXCEPT
       !.requestPhase[request] = "failed",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.connectionPhase = CleanupConnectionPhase(request),
       !.connectionOrigin = CleanupConnectionOrigin(request),
       !.connectionOriginOwner = CleanupConnectionOwner(request),
       !.connectionOriginIncarnation = CleanupConnectionIncarnation(request),
       !.activationLeaseIncarnation[request] = 0]
     IN s' = FinishPreCommitCleanup(next, request)

NavigateDirectory(request, responseIncarnation, responseRevision, responseAuthority) ==
  /\ IsActive(request)
  /\ responseIncarnation = s.requestIncarnation[request]
  /\ responseRevision = s.directoryRevision[request]
  /\ responseAuthority = s.directoryAuthority[request]
  /\ s.requestPhase[request] = "choosing-directory"
  /\ s.connectionPhase = "connected"
  /\ request \in s.ports
  /\ s.directoryRevision[request] < MaxDirectoryRevision
  /\ s' = [s EXCEPT
       !.directoryRevision[request] = @ + 1,
       !.directoryAuthority[request] = s.directoryRevision[request] + 1,
       !.directoryAcceptanceSafe = @
         /\ responseIncarnation = s.requestIncarnation[request]
         /\ responseRevision = s.directoryRevision[request]
         /\ responseAuthority = s.directoryAuthority[request]]

DirectoryNavigationFails(request, responseIncarnation, responseRevision, responseAuthority) ==
  /\ IsActive(request)
  /\ responseIncarnation = s.requestIncarnation[request]
  /\ responseRevision = s.directoryRevision[request]
  /\ responseAuthority = s.directoryAuthority[request]
  /\ s.requestPhase[request] = "choosing-directory"
  /\ s.connectionPhase = "connected"
  /\ request \in s.ports
  /\ s.directoryRevision[request] < MaxDirectoryRevision
  /\ LET next == [s EXCEPT
       !.directoryRevision[request] = @ + 1,
       !.directoryAcceptanceSafe = @
         /\ responseIncarnation = s.requestIncarnation[request]
         /\ responseRevision = s.directoryRevision[request]
         /\ responseAuthority = s.directoryAuthority[request]]
     IN s' = [next EXCEPT
          !.directoryFailureSafe = @
            /\ next.directoryRevision[request] =
                 s.directoryRevision[request] + 1
            /\ next.directoryAuthority[request] =
                 s.directoryAuthority[request]]

RejectStaleDirectorySelection(request, responseIncarnation, responseRevision, responseAuthority) ==
  /\ \E owner \in Requests:
       /\ IsActive(owner)
       /\ s.requestPhase[owner] = "choosing-directory"
       /\ s.connectionPhase = "connected"
       /\ responseIncarnation \in 1..MaxRequestIncarnation
       /\ responseRevision \in 1..MaxDirectoryRevision
       /\ responseAuthority \in 1..MaxDirectoryRevision
       /\ \/ request # owner
          \/ responseIncarnation # s.requestIncarnation[owner]
          \/ responseRevision # s.directoryRevision[owner]
          \/ responseAuthority # s.directoryAuthority[owner]
  /\ UNCHANGED s

SelectDirectory(request, responseIncarnation, responseRevision, responseAuthority) ==
  /\ IsActive(request)
  /\ responseIncarnation = s.requestIncarnation[request]
  /\ responseRevision = s.directoryRevision[request]
  /\ responseAuthority = s.directoryAuthority[request]
  /\ s.requestKind[request] = "enroll"
  /\ s.requestPhase[request] = "choosing-directory"
  /\ s.connectionPhase = "connected"
  /\ request \in s.ports
  /\ s' = [s EXCEPT
       !.requestPhase[request] = "directory-selected",
       !.selectedDirectoryAuthority[request] = responseAuthority,
       !.directoryAcceptanceSafe = @
         /\ responseIncarnation = s.requestIncarnation[request]
         /\ responseRevision = s.directoryRevision[request]
         /\ responseAuthority = s.directoryAuthority[request]]

InspectProjectRoot(request) ==
  /\ IsActive(request)
  /\ s.requestKind[request] = "enroll"
  /\ s.requestPhase[request] = "directory-selected"
  /\ s.connectionPhase = "connected"
  /\ s.selectedDirectoryAuthority[request] =
       s.directoryAuthority[request]
  /\ s.selectedDirectoryAuthority[request] > 0
  /\ s' = [s EXCEPT !.requestPhase[request] = "inspecting"]

InspectionCompletes(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "inspecting"
  /\ s.connectionPhase = "connected"
  /\ s' = [s EXCEPT !.requestPhase[request] = "ready-to-commit"]

InspectionFails(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "inspecting"
  /\ LET next == [s EXCEPT
       !.requestPhase[request] = "failed",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.connectionPhase = CleanupConnectionPhase(request),
       !.connectionOrigin = CleanupConnectionOrigin(request),
       !.connectionOriginOwner = CleanupConnectionOwner(request),
       !.connectionOriginIncarnation = CleanupConnectionIncarnation(request),
       !.activationLeaseIncarnation[request] = 0]
     IN s' = FinishPreCommitCleanup(next, request)

BeginWorkbenchCommit(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "ready-to-commit"
  /\ s.registryState = "present"
  /\ s.connectionPhase = "connected"
  /\ s' = [s EXCEPT !.requestPhase[request] = "committing"]

CommitProjectNew(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "committing"
  /\ ~s.storedProject
  /\ LET next == [s EXCEPT
       !.storedProject = TRUE,
       !.canonicalProject = TRUE,
       !.requestPhase[request] = "committed-unpublished",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.connectionOrigin = "shared",
       !.connectionOriginOwner = NoRequest,
       !.connectionOriginIncarnation = NoIncarnation]
     IN s' = FinishCanonicalCommit(next, request)

CommitProjectExisting(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "committing"
  /\ s.storedProject
  /\ LET next == [s EXCEPT
       !.canonicalProject = TRUE,
       !.requestPhase[request] = "committed-unpublished",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.connectionOrigin = "shared",
       !.connectionOriginOwner = NoRequest,
       !.connectionOriginIncarnation = NoIncarnation]
     IN s' = FinishCanonicalCommit(next, request)

WorkbenchCommitIndeterminate(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "committing"
  /\ s' = [s EXCEPT
       !.storedProject = TRUE,
       !.requestPhase[request] = "workbench-outcome-unknown",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.connectionOrigin = "shared",
       !.connectionOriginOwner = NoRequest,
       !.connectionOriginIncarnation = NoIncarnation]

ReconcileWorkbenchCommit(request) ==
  /\ s.requestPhase[request] = "workbench-outcome-unknown"
  /\ s.registryState = "present"
  /\ s.storedProject
  /\ LET next == [s EXCEPT
       !.canonicalProject = TRUE,
       !.requestPhase[request] = "committed-unpublished",
       !.connectionOrigin = "shared",
       !.connectionOriginOwner = NoRequest,
       !.connectionOriginIncarnation = NoIncarnation]
     IN s' = FinishCanonicalCommit(next, request)

PublishEnrollment(request, responseIncarnation) ==
  /\ responseIncarnation \in 1..MaxRequestIncarnation
  /\ responseIncarnation = s.requestIncarnation[request]
  /\ s.requestPhase[request] = "committed-unpublished"
  /\ s.canonicalProject
  /\ s.publishedIncarnation[request] # responseIncarnation
  /\ s' = [s EXCEPT
       !.publishedEnrollment = TRUE,
       !.publishedIncarnation[request] = responseIncarnation,
       !.requestPhase[request] = "committed",
       !.publicationAuthoritySafe = @
         /\ responseIncarnation = s.requestIncarnation[request]
         /\ s.requestPhase[request] = "committed-unpublished"
         /\ s.canonicalProject]

CancelBeforeWorkbenchCommit(request) ==
  /\ IsActive(request)
  /\ s.requestKind[request] = "enroll"
  /\ s.requestPhase[request] \in PreCommitPhases
  /\ LET next == [s EXCEPT
       !.requestPhase[request] = "cancelled",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.connectionPhase = CleanupConnectionPhase(request),
       !.connectionOrigin = CleanupConnectionOrigin(request),
       !.connectionOriginOwner = CleanupConnectionOwner(request),
       !.connectionOriginIncarnation = CleanupConnectionIncarnation(request),
       !.promptOwner = NoRequest,
       !.promptIncarnation = NoIncarnation,
       !.promptGeneration = NoGeneration,
       !.promptHandle = NoHandle,
       !.activationLeaseIncarnation[request] = 0]
     IN s' = FinishPreCommitCleanup(next, request)

CancelDuringWorkbenchCommit(request) ==
  /\ IsActive(request)
  /\ s.requestPhase[request] = "committing"
  /\ UNCHANGED s

RetirePortBeforeCommit(request) ==
  /\ request \in s.ports
  /\ IsActive(request)
  /\ s.requestPhase[request] \in PreCommitPhases
  /\ LET next == [s EXCEPT
       !.ports = @ \ {request},
       !.requestPhase[request] = "cancelled",
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.connectionPhase = CleanupConnectionPhase(request),
       !.connectionOrigin = CleanupConnectionOrigin(request),
       !.connectionOriginOwner = CleanupConnectionOwner(request),
       !.connectionOriginIncarnation = CleanupConnectionIncarnation(request),
       !.promptOwner = NoRequest,
       !.promptIncarnation = NoIncarnation,
       !.promptGeneration = NoGeneration,
       !.promptHandle = NoHandle,
       !.activationLeaseIncarnation[request] = 0,
       !.activatedIncarnation[request] = 0]
     IN s' = FinishPreCommitCleanup(next, request)

RetirePortDuringCommit(request) ==
  /\ request \in s.ports
  /\ IsActive(request)
  /\ s.requestPhase[request] = "committing"
  /\ s' = [s EXCEPT
       !.ports = @ \ {request},
       !.activationLeaseIncarnation[request] = 0,
       !.activatedIncarnation[request] = 0]

RetirePortAfterCommit(request) ==
  /\ request \in s.ports
  /\ s.requestPhase[request] \in
       TerminalPhases \cup {"committed-unpublished"}
  /\ s' = [s EXCEPT
       !.ports = @ \ {request},
       !.activationLeaseIncarnation[request] = 0,
       !.activatedIncarnation[request] = 0]

RetryRequest(request) ==
  /\ request \in s.ports
  /\ s.requestPhase[request] \in TerminalPhases
  /\ s.activeRequest # request
  /\ s' = [s EXCEPT
       !.requestKind[request] = "none",
       !.requestPhase[request] = "idle",
       !.publishedIncarnation[request] = 0,
       !.activationLeaseIncarnation[request] = 0,
       !.activatedIncarnation[request] = 0]

RequestReconnect(request) ==
  /\ request \in s.ports
  /\ s.requestPhase[request] = "idle"
  /\ s.activeRequest = NoRequest
  /\ s.registryState = "present"
  /\ s.canonicalProject
  /\ s.connectionPhase \in {"disconnected", "blocked"}
  /\ s.generation < MaxGeneration
  /\ s.requestIncarnation[request] < MaxRequestIncarnation
  /\ LET nextIncarnation == s.requestIncarnation[request] + 1
     IN s' = [s EXCEPT
          !.requestKind[request] = "reconnect",
          !.requestPhase[request] = "connecting",
          !.requestIncarnation[request] = nextIncarnation,
          !.activeRequest = request,
          !.activeIncarnation = nextIncarnation,
          !.connectionPhase = "connecting",
          !.connectionOrigin = "shared",
          !.connectionOriginOwner = NoRequest,
          !.connectionOriginIncarnation = NoIncarnation,
          !.generation = @ + 1,
          !.publishedIncarnation[request] = 0,
          !.directoryRevision[request] = 0,
          !.directoryAuthority[request] = 0,
          !.selectedDirectoryAuthority[request] = 0,
          !.activationLeaseIncarnation[request] = 0,
          !.activatedIncarnation[request] = 0]

RequestDisconnect ==
  /\ s.registryState = "present"
  /\ s.connectionPhase # "disconnected"
  /\ s.activeRequest = NoRequest
  /\ s.promptOwner = NoRequest
  /\ s' = [s EXCEPT
       !.connectionPhase = "disconnected",
       !.connectionOrigin = "none",
       !.connectionOriginOwner = NoRequest,
       !.connectionOriginIncarnation = NoIncarnation]

LoseConnection ==
  /\ s.connectionPhase = "connected"
  /\ s.activeRequest = NoRequest
  /\ s.generation < MaxGeneration
  /\ s' = [s EXCEPT
       !.connectionPhase = "retry-wait",
       !.generation = @ + 1]

AutomaticRetry ==
  /\ s.connectionPhase = "retry-wait"
  /\ s.activeRequest = NoRequest
  /\ s' = [s EXCEPT !.connectionPhase = "connecting"]

BlockPromptWithoutLease ==
  /\ s.connectionPhase = "connecting"
  /\ s.activeRequest = NoRequest
  /\ s.promptOwner = NoRequest
  /\ s' = [s EXCEPT !.connectionPhase = "blocked"]

ActivateInitiatingWorkspace(request, responseIncarnation) ==
  /\ responseIncarnation \in 1..MaxRequestIncarnation
  /\ responseIncarnation = s.requestIncarnation[request]
  /\ s.requestPhase[request] = "committed"
  /\ request \in s.ports
  /\ s.activationLeaseIncarnation[request] = responseIncarnation
  /\ s.publishedIncarnation[request] = responseIncarnation
  /\ s.activatedIncarnation[request] # responseIncarnation
  /\ s' = [s EXCEPT
       !.activatedIncarnation[request] = responseIncarnation,
       !.activationLeaseIncarnation[request] = 0,
       !.activationAuthoritySafe = @
         /\ request \in s.ports
         /\ responseIncarnation = s.requestIncarnation[request]
         /\ s.publishedIncarnation[request] = responseIncarnation
         /\ s.activationLeaseIncarnation[request] = responseIncarnation]

ApplicationRestart ==
  /\ s.registryState # "indeterminate"
  /\ s' = [s EXCEPT
       !.ports = {},
       !.requestKind = [request \in Requests |-> "none"],
       !.requestPhase = [request \in Requests |-> "idle"],
       !.activeRequest = NoRequest,
       !.activeIncarnation = NoIncarnation,
       !.canonicalProject = s.storedProject,
       !.publishedEnrollment = s.storedProject,
       !.publishedIncarnation = [request \in Requests |-> 0],
       !.connectionPhase = "disconnected",
       !.connectionOrigin = "none",
       !.connectionOriginOwner = NoRequest,
       !.connectionOriginIncarnation = NoIncarnation,
       !.generation =
         IF s.generation < MaxGeneration THEN @ + 1 ELSE @,
       !.promptOwner = NoRequest,
       !.promptIncarnation = NoIncarnation,
       !.promptGeneration = NoGeneration,
       !.promptHandle = NoHandle,
       !.directoryRevision = [request \in Requests |-> 0],
       !.directoryAuthority = [request \in Requests |-> 0],
       !.selectedDirectoryAuthority = [request \in Requests |-> 0],
       !.activationLeaseIncarnation = [request \in Requests |-> 0],
       !.activatedIncarnation = [request \in Requests |-> 0]]

Next ==
  \/ \E request \in Requests : OpenPort(request)
  \/ \E request \in Requests : RequestEnrollment(request)
  \/ \E request \in Requests : RejectBusy(request)
  \/ \E request \in Requests : ResolveTarget(request)
  \/ \E request \in Requests : TargetResolutionFails(request)
  \/ \E request \in Requests : CommitRegistryNew(request)
  \/ \E request \in Requests : ReuseRegistry(request)
  \/ \E request \in Requests : RegistryPrecommitFails(request)
  \/ \E request \in Requests : RegistryPostRenameIndeterminate(request)
  \/ ReloadRegistryPresent
  \/ ReloadRegistryAbsent
  \/ ObserveExistingConnection
  \/ \E request \in Requests : StartConnection(request)
  \/ \E request \in Requests : ReuseConnection(request)
  \/ \E request \in Requests, handle \in Handles :
       RequestInteraction(request, handle)
  \/ \E request \in Requests,
         incarnation \in 1..MaxRequestIncarnation,
         gen \in 1..MaxGeneration, handle \in Handles :
       SubmitInteraction(request, incarnation, gen, handle)
  \/ \E request \in Requests,
         incarnation \in 1..MaxRequestIncarnation,
         gen \in 1..MaxGeneration, handle \in Handles :
       CancelInteraction(request, incarnation, gen, handle)
  \/ \E request \in Requests,
         incarnation \in 1..MaxRequestIncarnation,
         gen \in 1..MaxGeneration, handle \in Handles :
       RejectStaleInteraction(request, incarnation, gen, handle)
  \/ \E request \in Requests : ConnectionReady(request)
  \/ ConnectionReadyWithoutLease
  \/ \E request \in Requests : ConnectionFails(request)
  \/ \E request \in Requests : PublishInitialDirectory(request)
  \/ \E request \in Requests : InitialDirectoryListingFails(request)
  \/ \E request \in Requests,
         incarnation \in 1..MaxRequestIncarnation,
         revision \in 1..MaxDirectoryRevision,
         authority \in 1..MaxDirectoryRevision :
       NavigateDirectory(request, incarnation, revision, authority)
  \/ \E request \in Requests,
         incarnation \in 1..MaxRequestIncarnation,
         revision \in 1..MaxDirectoryRevision,
         authority \in 1..MaxDirectoryRevision :
       DirectoryNavigationFails(request, incarnation, revision, authority)
  \/ \E request \in Requests,
         incarnation \in 1..MaxRequestIncarnation,
         revision \in 1..MaxDirectoryRevision,
         authority \in 1..MaxDirectoryRevision :
       RejectStaleDirectorySelection(
         request, incarnation, revision, authority)
  \/ \E request \in Requests,
         incarnation \in 1..MaxRequestIncarnation,
         revision \in 1..MaxDirectoryRevision,
         authority \in 1..MaxDirectoryRevision :
       SelectDirectory(request, incarnation, revision, authority)
  \/ \E request \in Requests : InspectProjectRoot(request)
  \/ \E request \in Requests : InspectionCompletes(request)
  \/ \E request \in Requests : InspectionFails(request)
  \/ \E request \in Requests : BeginWorkbenchCommit(request)
  \/ \E request \in Requests : CommitProjectNew(request)
  \/ \E request \in Requests : CommitProjectExisting(request)
  \/ \E request \in Requests : WorkbenchCommitIndeterminate(request)
  \/ \E request \in Requests : ReconcileWorkbenchCommit(request)
  \/ \E request \in Requests,
         incarnation \in 1..MaxRequestIncarnation :
       PublishEnrollment(request, incarnation)
  \/ \E request \in Requests : CancelBeforeWorkbenchCommit(request)
  \/ \E request \in Requests : CancelDuringWorkbenchCommit(request)
  \/ \E request \in Requests : RetirePortBeforeCommit(request)
  \/ \E request \in Requests : RetirePortDuringCommit(request)
  \/ \E request \in Requests : RetirePortAfterCommit(request)
  \/ \E request \in Requests : RetryRequest(request)
  \/ \E request \in Requests : RequestReconnect(request)
  \/ RequestDisconnect
  \/ LoseConnection
  \/ AutomaticRetry
  \/ BlockPromptWithoutLease
  \/ \E request \in Requests,
         incarnation \in 1..MaxRequestIncarnation :
       ActivateInitiatingWorkspace(request, incarnation)
  \/ ApplicationRestart

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.ports \subseteq Requests
  /\ s.requestKind \in [Requests -> RequestKinds]
  /\ s.requestPhase \in [Requests -> RequestPhases]
  /\ s.requestIncarnation \in
       [Requests -> 0..MaxRequestIncarnation]
  /\ s.activeRequest \in Requests \cup {NoRequest}
  /\ s.activeIncarnation \in
       (1..MaxRequestIncarnation) \cup {NoIncarnation}
  /\ s.registryState \in RegistryStates
  /\ s.storedProject \in BOOLEAN
  /\ s.canonicalProject \in BOOLEAN
  /\ s.publishedEnrollment \in BOOLEAN
  /\ s.publishedIncarnation \in
       [Requests -> 0..MaxRequestIncarnation]
  /\ s.connectionPhase \in ConnectionPhases
  /\ s.connectionOrigin \in ConnectionOrigins
  /\ s.connectionOriginOwner \in Requests \cup {NoRequest}
  /\ s.connectionOriginIncarnation \in
       (1..MaxRequestIncarnation) \cup {NoIncarnation}
  /\ s.generation \in 0..MaxGeneration
  /\ s.promptOwner \in Requests \cup {NoRequest}
  /\ s.promptIncarnation \in
       (1..MaxRequestIncarnation) \cup {NoIncarnation}
  /\ s.promptGeneration \in (1..MaxGeneration) \cup {NoGeneration}
  /\ s.promptHandle \in Handles \cup {NoHandle}
  /\ s.directoryRevision \in
       [Requests -> 0..MaxDirectoryRevision]
  /\ s.directoryAuthority \in
       [Requests -> 0..MaxDirectoryRevision]
  /\ s.selectedDirectoryAuthority \in
       [Requests -> 0..MaxDirectoryRevision]
  /\ s.activationLeaseIncarnation \in
       [Requests -> 0..MaxRequestIncarnation]
  /\ s.activatedIncarnation \in
       [Requests -> 0..MaxRequestIncarnation]
  /\ s.terminalEffectCount \in Nat
  /\ s.promptAcceptanceSafe \in BOOLEAN
  /\ s.directoryAcceptanceSafe \in BOOLEAN
  /\ s.directoryFailureSafe \in BOOLEAN
  /\ s.commitPublicationSafe \in BOOLEAN
  /\ s.publicationAuthoritySafe \in BOOLEAN
  /\ s.activationAuthoritySafe \in BOOLEAN
  /\ s.connectionCleanupSafe \in BOOLEAN

RemoteEffectRequiresDurableEnvironment ==
  s.connectionPhase # "disconnected" => s.registryState = "present"

WorkbenchReferenceRequiresEnvironment ==
  s.storedProject => s.registryState = "present"

CanonicalProjectWasStored ==
  s.canonicalProject => s.storedProject

EnrollmentPublicationRequiresCanonicalWorkbench ==
  s.publishedEnrollment => s.canonicalProject

ActiveRequestHasExactIncarnation ==
  /\ (s.activeRequest = NoRequest) =
       (s.activeIncarnation = NoIncarnation)
  /\ (s.activeRequest # NoRequest =>
       /\ s.activeIncarnation =
            s.requestIncarnation[s.activeRequest]
       /\ (s.activeRequest \in s.ports \/
            s.requestPhase[s.activeRequest] = "committing"))

InteractionHasExactLiveLease ==
  s.connectionPhase = "interaction" =>
    /\ s.promptOwner \in s.ports
    /\ IsActive(s.promptOwner)
    /\ s.requestPhase[s.promptOwner] = "interaction"
    /\ s.promptIncarnation =
         s.requestIncarnation[s.promptOwner]
    /\ s.promptGeneration = s.generation
    /\ s.promptHandle \in Handles

NoPromptOutsideInteraction ==
  s.connectionPhase # "interaction" =>
    /\ s.promptOwner = NoRequest
    /\ s.promptIncarnation = NoIncarnation
    /\ s.promptGeneration = NoGeneration
    /\ s.promptHandle = NoHandle

OnlyExactInteractionResponseIsAccepted ==
  s.promptAcceptanceSafe

DirectorySelectionHasExactLiveRequest ==
  \A request \in Requests:
    s.requestPhase[request] = "choosing-directory" =>
      /\ IsActive(request)
      /\ request \in s.ports
      /\ s.connectionPhase = "connected"
      /\ s.directoryRevision[request] >=
           s.directoryAuthority[request]
      /\ s.directoryAuthority[request] > 0

DirectoryFailureRetainsLastAuthority ==
  /\ s.directoryFailureSafe
  /\ \A request \in Requests:
       s.directoryAuthority[request] <= s.directoryRevision[request]

SelectedDirectoryHasCurrentAuthority ==
  \A request \in Requests:
    s.requestPhase[request] \in {
      "directory-selected", "inspecting", "ready-to-commit", "committing",
      "committed-unpublished", "committed"
    } =>
      /\ s.selectedDirectoryAuthority[request] > 0
      /\ s.selectedDirectoryAuthority[request] =
           s.directoryAuthority[request]

OnlyExactDirectoryResponseIsAccepted ==
  s.directoryAcceptanceSafe

CanonicalCommitPrecedesPublication ==
  s.commitPublicationSafe

RequestPublicationHasExactOwner ==
  /\ s.publicationAuthoritySafe
  /\ \A request \in Requests:
       s.publishedIncarnation[request] > 0 =>
         /\ s.publishedIncarnation[request] =
              s.requestIncarnation[request]
         /\ s.requestPhase[request] = "committed"
         /\ s.canonicalProject

CommittedRequestHasCanonicalProject ==
  \A request \in Requests:
    s.requestPhase[request] \in
      {"committed-unpublished", "committed"} =>
        s.canonicalProject

UnpublishedCommitCannotActivate ==
  \A request \in Requests:
    s.requestPhase[request] = "committed-unpublished" =>
      s.activatedIncarnation[request] = 0

ActivationLeaseIsLiveAndExact ==
  \A request \in Requests:
    s.activationLeaseIncarnation[request] > 0 =>
      /\ request \in s.ports
      /\ s.activationLeaseIncarnation[request] =
           s.requestIncarnation[request]
      /\ s.requestKind[request] = "enroll"
      /\ s.requestPhase[request] \notin
           {"idle", "busy", "completed", "failed", "cancelled"}

OnlyPublishedLiveEnrollmentActivatesView ==
  /\ s.activationAuthoritySafe
  /\ \A request \in Requests:
       s.activatedIncarnation[request] > 0 =>
         /\ request \in s.ports
         /\ s.activatedIncarnation[request] =
              s.requestIncarnation[request]
         /\ s.publishedIncarnation[request] =
              s.activatedIncarnation[request]
         /\ s.requestPhase[request] = "committed"

ConnectionOriginHasExactOwner ==
  /\ (s.connectionOrigin = "request-started") =
       (s.connectionOriginOwner # NoRequest)
  /\ (s.connectionOriginOwner = NoRequest) =
       (s.connectionOriginIncarnation = NoIncarnation)
  /\ (s.connectionOriginOwner # NoRequest =>
       /\ s.connectionOriginIncarnation =
            s.requestIncarnation[s.connectionOriginOwner]
       /\ s.connectionPhase \in
            {"connecting", "interaction", "connected"})

PreCommitCleanupRespectsConnectionOrigin ==
  s.connectionCleanupSafe

BusyRequestCannotStealLease ==
  \A request \in Requests:
    s.requestPhase[request] = "busy" =>
      s.activeRequest # request

FreshProcessDoesNotAutoConnect ==
  (s.ports = {} /\
    \A request \in Requests:
      s.requestKind[request] = "none" /\
      s.requestPhase[request] = "idle") =>
    /\ s.connectionPhase = "disconnected"
    /\ s.activeRequest = NoRequest
    /\ s.promptOwner = NoRequest

EnrollmentCreatesNoTerminal == s.terminalEffectCount = 0

=============================================================================
