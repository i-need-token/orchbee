--------------- MODULE SshEnvironmentProjectFolderAttachment ---------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded model of attaching one SSH Environment Folder to an existing
\* canonical Workbench Project. The append-only registry owns Environment
\* identity, while the canonical Workbench document owns Folder membership.
\* Renderer prompts and directory snapshots remain one-request transient
\* authority and Workbench publication is distinct from durable commit.

CONSTANTS Requests, Projects, Environments, Paths, MaxGeneration,
          RequestSymmetryGroup, ProjectSymmetryGroup,
          EnvironmentSymmetryGroup, PathSymmetryGroup

NoRequest == "no-request"
ForeignRequest == "foreign-request"
NoProject == "no-project"
NoEnvironment == "no-environment"
NoPath == "no-path"
NoGeneration == MaxGeneration + 1

AttachmentSymmetry == {
  requestPermutation @@ projectPermutation @@ environmentPermutation @@ pathPermutation :
    requestPermutation \in Permutations(RequestSymmetryGroup),
    projectPermutation \in Permutations(ProjectSymmetryGroup),
    environmentPermutation \in Permutations(EnvironmentSymmetryGroup),
    pathPermutation \in Permutations(PathSymmetryGroup)
}

RequestPhases == {
  "idle", "busy", "resolving", "registry-writing", "pending",
  "connecting", "interaction", "connected-for-attachment",
  "choosing-directory", "directory-selected", "inspecting",
  "ready-to-commit", "committing", "outcome-unknown", "committed",
  "published", "completed", "failed", "cancelled", "retired"
}
ConnectionPhases == {
  "disconnected", "connecting", "interaction", "connected", "blocked"
}
PreCommitPhases == {
  "resolving", "registry-writing", "pending", "connecting", "interaction",
  "connected-for-attachment", "choosing-directory", "directory-selected",
  "inspecting", "ready-to-commit"
}
TerminalRequestPhases == {"busy", "completed", "failed", "cancelled", "retired"}

DirectoryRecord(project, environment, path) ==
  [project |-> project, environment |-> environment, path |-> path]

FoldersForProject(folders, project) ==
  {entry \in folders : entry.project = project}

LocationOwners(folders, environment, path) ==
  {entry.project :
    entry \in {candidate \in folders :
      candidate.environment = environment /\ candidate.path = path}}

ProjectEnvironmentAgrees(folders, projectEnvironment) ==
  \A entry \in folders:
    projectEnvironment[entry.project] = entry.environment

Actions == {
  "init", "open-port", "seed-folder", "remove-project",
  "request-attachment", "reject-busy", "resolve-target",
  "target-resolution-fails", "commit-registry-new", "reuse-registry",
  "registry-write-fails", "start-connection", "reuse-connection",
  "request-interaction", "submit-interaction", "cancel-interaction",
  "reject-stale-interaction", "connection-ready", "connection-fails",
  "publish-initial-directory", "initial-directory-listing-fails",
  "navigate-directory", "directory-navigation-fails",
  "reject-stale-directory-selection", "select-directory",
  "inspect-project-root", "inspection-completes", "inspection-fails",
  "begin-workbench-commit", "commit-folder-new", "settle-folder-existing",
  "reject-missing-target", "reject-environment-conflict",
  "reject-cross-project-location", "workbench-commit-indeterminate",
  "restart-load-committed", "restart-load-prior",
  "reconcile-workbench-commit", "reconcile-workbench-missing",
  "publish-canonical-folder", "observe-published-folder",
  "publication-target-missing", "cancel-before-workbench-commit",
  "cancel-during-workbench-commit", "retire-port-before-commit",
  "retire-port-during-commit", "retire-port-after-commit", "retry-request"
}

VARIABLES
  ports,
  liveProjects,
  registryEnvironments,
  storedProjectEnvironment,
  canonicalProjectEnvironment,
  publishedProjectEnvironment,
  storedFolders,
  canonicalFolders,
  publishedFolders,
  requestPhase,
  requestProject,
  requestEnvironment,
  requestPath,
  activeRequest,
  connectionPhase,
  generation,
  promptOwner,
  promptGeneration,
  projectNames,
  primaryDirectories,
  workspaceCounts,
  activeLocation,
  terminalEffectCount,
  coordinatorAvailable,
  lastAction,
  lastRequest

vars == <<
  ports, liveProjects, registryEnvironments, storedProjectEnvironment,
  canonicalProjectEnvironment, publishedProjectEnvironment, storedFolders,
  canonicalFolders, publishedFolders, requestPhase, requestProject,
  requestEnvironment, requestPath, activeRequest, connectionPhase, generation,
  promptOwner, promptGeneration, projectNames, primaryDirectories,
  workspaceCounts, activeLocation, terminalEffectCount, coordinatorAvailable,
  lastAction, lastRequest
>>

Init ==
  /\ ports = {}
  /\ liveProjects = Projects
  /\ registryEnvironments = {}
  /\ storedProjectEnvironment = [project \in Projects |-> NoEnvironment]
  /\ canonicalProjectEnvironment = [project \in Projects |-> NoEnvironment]
  /\ publishedProjectEnvironment = [project \in Projects |-> NoEnvironment]
  /\ storedFolders = {}
  /\ canonicalFolders = {}
  /\ publishedFolders = {}
  /\ requestPhase = [request \in Requests |-> "idle"]
  /\ requestProject = [request \in Requests |-> NoProject]
  /\ requestEnvironment = [request \in Requests |-> NoEnvironment]
  /\ requestPath = [request \in Requests |-> NoPath]
  /\ activeRequest = NoRequest
  /\ connectionPhase = [environment \in Environments |-> "disconnected"]
  /\ generation = [environment \in Environments |-> 0]
  /\ promptOwner = NoRequest
  /\ promptGeneration = NoGeneration
  /\ projectNames = [project \in Projects |-> project]
  /\ primaryDirectories = [project \in Projects |-> project]
  /\ workspaceCounts = [project \in Projects |-> 1]
  /\ activeLocation = NoProject
  /\ terminalEffectCount = 0
  /\ coordinatorAvailable = TRUE
  /\ lastAction = "init"
  /\ lastRequest = NoRequest

OpenPort(request) ==
  /\ request \notin ports
  /\ ports' = ports \cup {request}
  /\ UNCHANGED <<liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPhase, requestProject, requestEnvironment,
       requestPath, activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "open-port"
  /\ lastRequest' = request

SeedFolder(project, environment, path) ==
  /\ project \in liveProjects
  /\ activeRequest = NoRequest
  /\ \A request \in Requests: requestPhase[request] = "idle"
  /\ Cardinality(storedFolders) < 1
  /\ storedFolders = canonicalFolders
  /\ canonicalFolders = publishedFolders
  /\ storedProjectEnvironment = canonicalProjectEnvironment
  /\ canonicalProjectEnvironment = publishedProjectEnvironment
  /\ storedProjectEnvironment[project] \in {NoEnvironment, environment}
  /\ canonicalProjectEnvironment[project] \in {NoEnvironment, environment}
  /\ LocationOwners(canonicalFolders, environment, path) = {}
  /\ registryEnvironments' = registryEnvironments \cup {environment}
  /\ storedProjectEnvironment' = [storedProjectEnvironment EXCEPT ![project] = environment]
  /\ canonicalProjectEnvironment' = [canonicalProjectEnvironment EXCEPT ![project] = environment]
  /\ publishedProjectEnvironment' = [publishedProjectEnvironment EXCEPT ![project] = environment]
  /\ storedFolders' = storedFolders \cup {DirectoryRecord(project, environment, path)}
  /\ canonicalFolders' = canonicalFolders \cup {DirectoryRecord(project, environment, path)}
  /\ publishedFolders' = publishedFolders \cup {DirectoryRecord(project, environment, path)}
  /\ UNCHANGED <<ports, liveProjects, requestPhase, requestProject,
       requestEnvironment, requestPath, activeRequest, connectionPhase,
       generation, promptOwner, promptGeneration, projectNames,
       primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "seed-folder"
  /\ lastRequest' = NoRequest

RemoveProject(project) ==
  /\ project \in liveProjects
  /\ liveProjects' = liveProjects \ {project}
  /\ storedProjectEnvironment' = [storedProjectEnvironment EXCEPT ![project] = NoEnvironment]
  /\ canonicalProjectEnvironment' = [canonicalProjectEnvironment EXCEPT ![project] = NoEnvironment]
  /\ publishedProjectEnvironment' = [publishedProjectEnvironment EXCEPT ![project] = NoEnvironment]
  /\ storedFolders' = storedFolders \ FoldersForProject(storedFolders, project)
  /\ canonicalFolders' = canonicalFolders \ FoldersForProject(canonicalFolders, project)
  /\ publishedFolders' = publishedFolders \ FoldersForProject(publishedFolders, project)
  /\ requestPhase' = [request \in Requests |->
       IF requestProject[request] = project /\ requestPhase[request] = "completed"
       THEN "retired"
       ELSE requestPhase[request]]
  /\ UNCHANGED <<ports, registryEnvironments, requestProject,
       requestEnvironment, requestPath, activeRequest, connectionPhase,
       generation, promptOwner, promptGeneration, projectNames,
       primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "remove-project"
  /\ lastRequest' = NoRequest

RequestAttachment(request, project, environment) ==
  /\ request \in ports
  /\ requestPhase[request] = "idle"
  /\ activeRequest = NoRequest
  /\ project \in liveProjects
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "resolving"]
  /\ requestProject' = [requestProject EXCEPT ![request] = project]
  /\ requestEnvironment' = [requestEnvironment EXCEPT ![request] = environment]
  /\ activeRequest' = request
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPath, connectionPhase, generation,
       promptOwner, promptGeneration, projectNames, primaryDirectories,
       workspaceCounts, activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "request-attachment"
  /\ lastRequest' = request

RejectBusy(project, environment) ==
  /\ activeRequest # NoRequest
  /\ project \in liveProjects
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPhase, requestProject, requestEnvironment,
       requestPath, activeRequest, connectionPhase,
       generation, promptOwner, promptGeneration, projectNames,
       primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reject-busy"
  /\ lastRequest' = ForeignRequest

ResolveTarget(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "resolving"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "registry-writing"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "resolve-target"
  /\ lastRequest' = request

TargetResolutionFails(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "resolving"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "target-resolution-fails"
  /\ lastRequest' = request

CommitRegistryNew(request) ==
  LET environment == requestEnvironment[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "registry-writing"
  /\ environment \notin registryEnvironments
  /\ registryEnvironments' = registryEnvironments \cup {environment}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "pending"]
  /\ UNCHANGED <<ports, liveProjects, storedProjectEnvironment,
       canonicalProjectEnvironment, publishedProjectEnvironment,
       storedFolders, canonicalFolders, publishedFolders, requestProject,
       requestEnvironment, requestPath, activeRequest, connectionPhase,
       generation, promptOwner, promptGeneration, projectNames,
       primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "commit-registry-new"
  /\ lastRequest' = request

ReuseRegistry(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "registry-writing"
  /\ requestEnvironment[request] \in registryEnvironments
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "pending"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reuse-registry"
  /\ lastRequest' = request

RegistryWriteFails(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "registry-writing"
  /\ requestEnvironment[request] \notin registryEnvironments
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "registry-write-fails"
  /\ lastRequest' = request

StartConnection(request) ==
  LET environment == requestEnvironment[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "pending"
  /\ environment \in registryEnvironments
  /\ connectionPhase[environment] \in {"disconnected", "blocked"}
  /\ generation[environment] < MaxGeneration
  /\ connectionPhase' = [connectionPhase EXCEPT ![environment] = "connecting"]
  /\ generation' = [generation EXCEPT ![environment] = @ + 1]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "connecting"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, promptOwner, promptGeneration, projectNames,
       primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "start-connection"
  /\ lastRequest' = request

ReuseConnection(request) ==
  LET environment == requestEnvironment[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "pending"
  /\ environment \in registryEnvironments
  /\ connectionPhase[environment] = "connected"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "connected-for-attachment"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reuse-connection"
  /\ lastRequest' = request

RequestInteraction(request) ==
  LET environment == requestEnvironment[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "connecting"
  /\ request \in ports
  /\ connectionPhase[environment] = "connecting"
  /\ promptOwner = NoRequest
  /\ connectionPhase' = [connectionPhase EXCEPT ![environment] = "interaction"]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "interaction"]
  /\ promptOwner' = request
  /\ promptGeneration' = generation[environment]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, generation, projectNames, primaryDirectories,
       workspaceCounts, activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "request-interaction"
  /\ lastRequest' = request

SubmitInteraction(request) ==
  LET environment == requestEnvironment[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "interaction"
  /\ request \in ports
  /\ connectionPhase[environment] = "interaction"
  /\ promptOwner = request
  /\ promptGeneration = generation[environment]
  /\ connectionPhase' = [connectionPhase EXCEPT ![environment] = "connecting"]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "connecting"]
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, generation, projectNames, primaryDirectories,
       workspaceCounts, activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "submit-interaction"
  /\ lastRequest' = request

CancelInteraction(request) ==
  LET environment == requestEnvironment[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "interaction"
  /\ connectionPhase[environment] = "interaction"
  /\ promptOwner = request
  /\ promptGeneration = generation[environment]
  /\ connectionPhase' = [connectionPhase EXCEPT ![environment] = "blocked"]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "cancelled"]
  /\ activeRequest' = NoRequest
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       generation, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "cancel-interaction"
  /\ lastRequest' = request

RejectStaleInteraction(staleGeneration) ==
  /\ promptOwner # NoRequest
  /\ staleGeneration \in 0..MaxGeneration
  /\ staleGeneration # promptGeneration
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPhase, requestProject, requestEnvironment,
       requestPath, activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reject-stale-interaction"
  /\ lastRequest' = ForeignRequest

ConnectionReady(request) ==
  LET environment == requestEnvironment[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "connecting"
  /\ connectionPhase[environment] = "connecting"
  /\ connectionPhase' = [connectionPhase EXCEPT ![environment] = "connected"]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "connected-for-attachment"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "connection-ready"
  /\ lastRequest' = request

ConnectionFails(request) ==
  LET environment == requestEnvironment[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] \in {"connecting", "interaction"}
  /\ connectionPhase[environment] \in {"connecting", "interaction"}
  /\ connectionPhase' = [connectionPhase EXCEPT ![environment] = "blocked"]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       generation, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "connection-fails"
  /\ lastRequest' = request

PublishInitialDirectory(request) ==
  LET environment == requestEnvironment[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "connected-for-attachment"
  /\ connectionPhase[environment] = "connected"
  /\ request \in ports
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "choosing-directory"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "publish-initial-directory"
  /\ lastRequest' = request

InitialDirectoryListingFails(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "connected-for-attachment"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "initial-directory-listing-fails"
  /\ lastRequest' = request

NavigateDirectory(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "choosing-directory"
  /\ request \in ports
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPhase, requestProject, requestEnvironment,
       requestPath, activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "navigate-directory"
  /\ lastRequest' = request

DirectoryNavigationFails(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "choosing-directory"
  /\ request \in ports
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPhase, requestProject, requestEnvironment,
       requestPath, activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "directory-navigation-fails"
  /\ lastRequest' = request

RejectStaleDirectorySelection ==
  /\ \E owner \in Requests:
       /\ activeRequest = owner
       /\ requestPhase[owner] = "choosing-directory"
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPhase, requestProject, requestEnvironment,
       requestPath, activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reject-stale-directory-selection"
  /\ lastRequest' = ForeignRequest

SelectDirectory(request, path) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "choosing-directory"
  /\ request \in ports
  /\ requestPath' = [requestPath EXCEPT ![request] = path]
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "directory-selected"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, activeRequest,
       connectionPhase, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "select-directory"
  /\ lastRequest' = request

InspectProjectRoot(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "directory-selected"
  /\ requestPath[request] \in Paths
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "inspecting"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "inspect-project-root"
  /\ lastRequest' = request

InspectionCompletes(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "inspecting"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "ready-to-commit"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "inspection-completes"
  /\ lastRequest' = request

InspectionFails(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "inspecting"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "inspection-fails"
  /\ lastRequest' = request

BeginWorkbenchCommit(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "ready-to-commit"
  /\ requestEnvironment[request] \in registryEnvironments
  /\ \A other \in Requests: requestPhase[other] # "outcome-unknown"
  /\ storedFolders = canonicalFolders
  /\ storedProjectEnvironment = canonicalProjectEnvironment
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "committing"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "begin-workbench-commit"
  /\ lastRequest' = request

CommitFolderNew(request) ==
  LET project == requestProject[request] IN
  LET environment == requestEnvironment[request] IN
  LET path == requestPath[request] IN
  LET directory == DirectoryRecord(project, environment, path) IN
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ project \in liveProjects
  /\ canonicalProjectEnvironment[project] \in {NoEnvironment, environment}
  /\ LocationOwners(canonicalFolders, environment, path) = {}
  /\ directory \notin canonicalFolders
  /\ storedProjectEnvironment' = [storedProjectEnvironment EXCEPT ![project] = environment]
  /\ canonicalProjectEnvironment' = [canonicalProjectEnvironment EXCEPT ![project] = environment]
  /\ storedFolders' = storedFolders \cup {directory}
  /\ canonicalFolders' = canonicalFolders \cup {directory}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "committed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       publishedProjectEnvironment, publishedFolders, requestProject,
       requestEnvironment, requestPath, connectionPhase, generation,
       promptOwner, promptGeneration, projectNames, primaryDirectories,
       workspaceCounts, activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "commit-folder-new"
  /\ lastRequest' = request

SettleFolderExisting(request) ==
  LET project == requestProject[request] IN
  LET environment == requestEnvironment[request] IN
  LET path == requestPath[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ project \in liveProjects
  /\ DirectoryRecord(project, environment, path) \in publishedFolders
  /\ canonicalProjectEnvironment[project] = environment
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "published"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "settle-folder-existing"
  /\ lastRequest' = request

RejectMissingTarget(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ requestProject[request] \notin liveProjects
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reject-missing-target"
  /\ lastRequest' = request

RejectEnvironmentConflict(request) ==
  LET project == requestProject[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ project \in liveProjects
  /\ canonicalProjectEnvironment[project] \notin {NoEnvironment, requestEnvironment[request]}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reject-environment-conflict"
  /\ lastRequest' = request

RejectCrossProjectLocation(request) ==
  LET project == requestProject[request] IN
  LET environment == requestEnvironment[request] IN
  LET path == requestPath[request] IN
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ project \in liveProjects
  /\ LocationOwners(canonicalFolders, environment, path) \ {project} # {}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, promptOwner, promptGeneration,
       projectNames, primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reject-cross-project-location"
  /\ lastRequest' = request

WorkbenchCommitIndeterminate(request) ==
  LET project == requestProject[request] IN
  LET environment == requestEnvironment[request] IN
  LET path == requestPath[request] IN
  LET directory == DirectoryRecord(project, environment, path) IN
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ project \in liveProjects
  /\ storedProjectEnvironment[project] \in {NoEnvironment, environment}
  /\ LocationOwners(storedFolders, environment, path) = {}
  /\ storedProjectEnvironment' = [storedProjectEnvironment EXCEPT ![project] = environment]
  /\ storedFolders' = storedFolders \cup {directory}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "outcome-unknown"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       canonicalProjectEnvironment, publishedProjectEnvironment,
       canonicalFolders, publishedFolders, requestProject, requestEnvironment,
       requestPath, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = FALSE
  /\ lastAction' = "workbench-commit-indeterminate"
  /\ lastRequest' = request

ReconcileWorkbenchCommit(request) ==
  LET project == requestProject[request] IN
  LET environment == requestEnvironment[request] IN
  LET directory == DirectoryRecord(project, environment, requestPath[request]) IN
  /\ requestPhase[request] = "outcome-unknown"
  /\ project \in liveProjects
  /\ directory \in storedFolders
  /\ storedProjectEnvironment[project] = environment
  /\ canonicalProjectEnvironment' = [canonicalProjectEnvironment EXCEPT ![project] = environment]
  /\ canonicalFolders' = canonicalFolders \cup {directory}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "committed"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, publishedProjectEnvironment, storedFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reconcile-workbench-commit"
  /\ lastRequest' = request

ReconcileWorkbenchMissing(request) ==
  LET directory == DirectoryRecord(requestProject[request], requestEnvironment[request], requestPath[request]) IN
  /\ requestPhase[request] = "outcome-unknown"
  /\ directory \notin storedFolders
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "reconcile-workbench-missing"
  /\ lastRequest' = request

\* A write-indeterminate result freezes the current coordinator.  A later
\* process generation must load durable state before it can reconcile.  The
\* post-rename directory-sync failure permits either the renamed document or
\* the prior document to be durable after a crash; ordinary project deletion
\* is never used to manufacture that uncertainty.
RestartLoadCommitted(request) ==
  LET directory == DirectoryRecord(
        requestProject[request], requestEnvironment[request], requestPath[request]
      ) IN
  /\ ~coordinatorAvailable
  /\ requestPhase[request] = "outcome-unknown"
  /\ directory \in storedFolders
  /\ storedProjectEnvironment[requestProject[request]] = requestEnvironment[request]
  /\ coordinatorAvailable' = TRUE
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPhase, requestProject, requestEnvironment,
       requestPath, activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ lastAction' = "restart-load-committed"
  /\ lastRequest' = request

RestartLoadPrior(request) ==
  LET project == requestProject[request] IN
  LET directory == DirectoryRecord(
        project, requestEnvironment[request], requestPath[request]
      ) IN
  LET priorFolders == storedFolders \ {directory} IN
  /\ ~coordinatorAvailable
  /\ requestPhase[request] = "outcome-unknown"
  /\ directory \in storedFolders
  /\ storedProjectEnvironment[project] = requestEnvironment[request]
  /\ storedFolders' = priorFolders
  /\ storedProjectEnvironment' =
       IF FoldersForProject(priorFolders, project) = {}
       THEN [storedProjectEnvironment EXCEPT ![project] = NoEnvironment]
       ELSE storedProjectEnvironment
  /\ coordinatorAvailable' = TRUE
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       canonicalProjectEnvironment, publishedProjectEnvironment,
       canonicalFolders, publishedFolders, requestPhase, requestProject,
       requestEnvironment, requestPath, activeRequest, connectionPhase,
       generation, promptOwner, promptGeneration, projectNames,
       primaryDirectories, workspaceCounts, activeLocation,
       terminalEffectCount>>
  /\ lastAction' = "restart-load-prior"
  /\ lastRequest' = request

PublishCanonicalFolder(request) ==
  LET project == requestProject[request] IN
  LET environment == requestEnvironment[request] IN
  LET directory == DirectoryRecord(project, environment, requestPath[request]) IN
  /\ requestPhase[request] = "committed"
  /\ project \in liveProjects
  /\ directory \in canonicalFolders
  /\ canonicalProjectEnvironment[project] = environment
  /\ publishedProjectEnvironment' = [publishedProjectEnvironment EXCEPT ![project] = environment]
  /\ publishedFolders' = publishedFolders \cup {directory}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "published"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment, storedFolders,
       canonicalFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "publish-canonical-folder"
  /\ lastRequest' = request

ObservePublishedFolder(request) ==
  /\ requestPhase[request] = "published"
  /\ request \in ports
  /\ requestProject[request] \in liveProjects
  /\ DirectoryRecord(
       requestProject[request], requestEnvironment[request], requestPath[request]
     ) \in publishedFolders
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "completed"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "observe-published-folder"
  /\ lastRequest' = request

PublicationTargetMissing(request) ==
  /\ requestPhase[request] \in {"committed", "published"}
  /\ requestProject[request] \notin liveProjects
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "failed"]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "publication-target-missing"
  /\ lastRequest' = request

CancelBeforeWorkbenchCommit(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] \in PreCommitPhases
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "cancelled"]
  /\ activeRequest' = NoRequest
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, projectNames, primaryDirectories,
       workspaceCounts, activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "cancel-before-workbench-commit"
  /\ lastRequest' = request

CancelDuringWorkbenchCommit(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPhase, requestProject, requestEnvironment,
       requestPath, activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "cancel-during-workbench-commit"
  /\ lastRequest' = request

RetirePortBeforeCommit(request) ==
  /\ request \in ports
  /\ activeRequest = request
  /\ requestPhase[request] \in PreCommitPhases
  /\ ports' = ports \ {request}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "cancelled"]
  /\ activeRequest' = NoRequest
  /\ promptOwner' = NoRequest
  /\ promptGeneration' = NoGeneration
  /\ UNCHANGED <<liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       connectionPhase, generation, projectNames, primaryDirectories,
       workspaceCounts, activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "retire-port-before-commit"
  /\ lastRequest' = request

RetirePortDuringCommit(request) ==
  /\ request \in ports
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ ports' = ports \ {request}
  /\ UNCHANGED <<liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestPhase, requestProject, requestEnvironment,
       requestPath, activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "retire-port-during-commit"
  /\ lastRequest' = request

RetirePortAfterCommit(request) ==
  /\ request \in ports
  /\ requestPhase[request] \in {"committed", "published", "completed",
       "failed", "busy", "cancelled", "outcome-unknown"}
  /\ ports' = ports \ {request}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "retired"]
  /\ UNCHANGED <<liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, requestProject, requestEnvironment, requestPath,
       activeRequest, connectionPhase, generation, promptOwner,
       promptGeneration, projectNames, primaryDirectories, workspaceCounts,
       activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "retire-port-after-commit"
  /\ lastRequest' = request

RetryRequest(request) ==
  /\ request \in ports
  /\ requestPhase[request] \in TerminalRequestPhases \cup {"published"}
  /\ activeRequest = NoRequest
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "idle"]
  /\ requestProject' = [requestProject EXCEPT ![request] = NoProject]
  /\ requestEnvironment' = [requestEnvironment EXCEPT ![request] = NoEnvironment]
  /\ requestPath' = [requestPath EXCEPT ![request] = NoPath]
  /\ UNCHANGED <<ports, liveProjects, registryEnvironments,
       storedProjectEnvironment, canonicalProjectEnvironment,
       publishedProjectEnvironment, storedFolders, canonicalFolders,
       publishedFolders, activeRequest, connectionPhase, generation,
       promptOwner, promptGeneration, projectNames, primaryDirectories,
       workspaceCounts, activeLocation, terminalEffectCount>>
  /\ coordinatorAvailable
  /\ coordinatorAvailable' = TRUE
  /\ lastAction' = "retry-request"
  /\ lastRequest' = request

Next ==
  \/ \E request \in Requests: OpenPort(request)
  \/ \E project \in Projects, environment \in Environments, path \in Paths:
       SeedFolder(project, environment, path)
  \/ \E project \in Projects: RemoveProject(project)
  \/ \E request \in Requests, project \in Projects, environment \in Environments:
       RequestAttachment(request, project, environment)
  \/ \E project \in Projects, environment \in Environments:
       RejectBusy(project, environment)
  \/ \E request \in Requests: ResolveTarget(request)
  \/ \E request \in Requests: TargetResolutionFails(request)
  \/ \E request \in Requests: CommitRegistryNew(request)
  \/ \E request \in Requests: ReuseRegistry(request)
  \/ \E request \in Requests: RegistryWriteFails(request)
  \/ \E request \in Requests: StartConnection(request)
  \/ \E request \in Requests: ReuseConnection(request)
  \/ \E request \in Requests: RequestInteraction(request)
  \/ \E request \in Requests: SubmitInteraction(request)
  \/ \E request \in Requests: CancelInteraction(request)
  \/ \E staleGeneration \in 0..MaxGeneration:
       RejectStaleInteraction(staleGeneration)
  \/ \E request \in Requests: ConnectionReady(request)
  \/ \E request \in Requests: ConnectionFails(request)
  \/ \E request \in Requests: PublishInitialDirectory(request)
  \/ \E request \in Requests: InitialDirectoryListingFails(request)
  \/ \E request \in Requests: NavigateDirectory(request)
  \/ \E request \in Requests: DirectoryNavigationFails(request)
  \/ RejectStaleDirectorySelection
  \/ \E request \in Requests, path \in Paths: SelectDirectory(request, path)
  \/ \E request \in Requests: InspectProjectRoot(request)
  \/ \E request \in Requests: InspectionCompletes(request)
  \/ \E request \in Requests: InspectionFails(request)
  \/ \E request \in Requests: BeginWorkbenchCommit(request)
  \/ \E request \in Requests: CommitFolderNew(request)
  \/ \E request \in Requests: SettleFolderExisting(request)
  \/ \E request \in Requests: RejectMissingTarget(request)
  \/ \E request \in Requests: RejectEnvironmentConflict(request)
  \/ \E request \in Requests: RejectCrossProjectLocation(request)
  \/ \E request \in Requests: WorkbenchCommitIndeterminate(request)
  \/ \E request \in Requests: ReconcileWorkbenchCommit(request)
  \/ \E request \in Requests: ReconcileWorkbenchMissing(request)
  \/ \E request \in Requests: PublishCanonicalFolder(request)
  \/ \E request \in Requests: ObservePublishedFolder(request)
  \/ \E request \in Requests: PublicationTargetMissing(request)
  \/ \E request \in Requests: CancelBeforeWorkbenchCommit(request)
  \/ \E request \in Requests: CancelDuringWorkbenchCommit(request)
  \/ \E request \in Requests: RetirePortBeforeCommit(request)
  \/ \E request \in Requests: RetirePortDuringCommit(request)
  \/ \E request \in Requests: RetirePortAfterCommit(request)
  \/ \E request \in Requests: RetryRequest(request)
  \/ \E request \in Requests: RestartLoadCommitted(request)
  \/ \E request \in Requests: RestartLoadPrior(request)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ ports \subseteq Requests
  /\ liveProjects \subseteq Projects
  /\ registryEnvironments \subseteq Environments
  /\ storedProjectEnvironment \in [Projects -> Environments \cup {NoEnvironment}]
  /\ canonicalProjectEnvironment \in [Projects -> Environments \cup {NoEnvironment}]
  /\ publishedProjectEnvironment \in [Projects -> Environments \cup {NoEnvironment}]
  /\ storedFolders \subseteq [project: Projects, environment: Environments, path: Paths]
  /\ canonicalFolders \subseteq [project: Projects, environment: Environments, path: Paths]
  /\ publishedFolders \subseteq [project: Projects, environment: Environments, path: Paths]
  /\ requestPhase \in [Requests -> RequestPhases]
  /\ requestProject \in [Requests -> Projects \cup {NoProject}]
  /\ requestEnvironment \in [Requests -> Environments \cup {NoEnvironment}]
  /\ requestPath \in [Requests -> Paths \cup {NoPath}]
  /\ activeRequest \in Requests \cup {NoRequest}
  /\ connectionPhase \in [Environments -> ConnectionPhases]
  /\ generation \in [Environments -> 0..MaxGeneration]
  /\ promptOwner \in Requests \cup {NoRequest}
  /\ promptGeneration \in 0..MaxGeneration \cup {NoGeneration}
  /\ projectNames \in [Projects -> Projects]
  /\ primaryDirectories \in [Projects -> Projects]
  /\ workspaceCounts \in [Projects -> Nat]
  /\ activeLocation \in Projects \cup {NoProject}
  /\ terminalEffectCount \in Nat
  /\ coordinatorAvailable \in BOOLEAN
  /\ lastAction \in Actions
  /\ lastRequest \in Requests \cup {NoRequest, ForeignRequest}

RegistryBeforeRemoteEffect ==
  \A environment \in Environments:
    connectionPhase[environment] # "disconnected" => environment \in registryEnvironments

CanonicalFoldersWereStored == canonicalFolders \subseteq storedFolders

PublishedFoldersAreCanonical == publishedFolders \subseteq canonicalFolders

StoredProjectHasOneEnvironment ==
  ProjectEnvironmentAgrees(storedFolders, storedProjectEnvironment)

CanonicalProjectHasOneEnvironment ==
  ProjectEnvironmentAgrees(canonicalFolders, canonicalProjectEnvironment)

PublishedProjectHasOneEnvironment ==
  ProjectEnvironmentAgrees(publishedFolders, publishedProjectEnvironment)

RemovedProjectsOwnNoFolders ==
  \A project \in Projects \ liveProjects:
    /\ FoldersForProject(storedFolders, project) = {}
    /\ FoldersForProject(canonicalFolders, project) = {}
    /\ FoldersForProject(publishedFolders, project) = {}
    /\ storedProjectEnvironment[project] = NoEnvironment
    /\ canonicalProjectEnvironment[project] = NoEnvironment
    /\ publishedProjectEnvironment[project] = NoEnvironment

LocationHasOneProjectOwner ==
  \A environment \in Environments, path \in Paths:
    Cardinality(LocationOwners(canonicalFolders, environment, path)) <= 1

InteractionHasExactLiveLease ==
  promptOwner # NoRequest =>
    /\ activeRequest = promptOwner
    /\ promptOwner \in ports
    /\ requestPhase[promptOwner] = "interaction"
    /\ promptGeneration = generation[requestEnvironment[promptOwner]]

NoPromptOutsideInteraction ==
  activeRequest = NoRequest \/ requestPhase[activeRequest] # "interaction" =>
    /\ promptOwner = NoRequest
    /\ promptGeneration = NoGeneration

DirectorySelectionHasExactLiveRequest ==
  \A request \in Requests:
    requestPhase[request] = "choosing-directory" =>
      /\ activeRequest = request
      /\ request \in ports
      /\ connectionPhase[requestEnvironment[request]] = "connected"

PublishedCompletionMatchesExactTarget ==
  \A request \in Requests:
    requestPhase[request] = "completed" =>
      DirectoryRecord(requestProject[request], requestEnvironment[request], requestPath[request])
        \in publishedFolders

PublicationNeverPrecedesCanonicalCommit == publishedFolders \subseteq canonicalFolders

AttachmentPreservesProjectMetadata ==
  /\ projectNames = [project \in Projects |-> project]
  /\ primaryDirectories = [project \in Projects |-> project]
  /\ workspaceCounts = [project \in Projects |-> 1]
  /\ activeLocation = NoProject
  /\ terminalEffectCount = 0

BusyRequestCannotStealLease ==
  lastAction = "reject-busy" =>
    /\ lastRequest = ForeignRequest
    /\ activeRequest # NoRequest

CancellationDuringCommitDoesNotRollBack ==
  lastAction = "cancel-during-workbench-commit" =>
    /\ activeRequest = lastRequest
    /\ requestPhase[lastRequest] = "committing"

IndeterminateFreezesCurrentCoordinator ==
  lastAction = "workbench-commit-indeterminate" => ~coordinatorAvailable

ReconciliationRunsAfterRestart ==
  lastAction \in {"reconcile-workbench-commit", "reconcile-workbench-missing"} =>
    coordinatorAvailable

=============================================================================
