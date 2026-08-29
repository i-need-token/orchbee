---------------- MODULE SshEnvironmentRemoteProjectAdmission ----------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

\* Bounded model of Main-owned remote Project and Workspace admission through
\* Slice 3.2A. Inspection must complete on an already-connected Environment
\* before a key enters the canonical mutation queue. Queue admission is the
\* cancellation commit point. Project, Directory, Workspace, and its empty
\* Terminal Context share one durable commit. Keys abstract the global
\* EnvironmentId plus canonical-path idempotency identity.

CONSTANT Keys, Requests

NoKey == "no-key"
RequestPhases == {
  "idle", "inspecting", "resolved", "queued", "committing",
  "created", "existing", "cancelled", "inspection-failed",
  "commit-failed", "indeterminate"
}
Actions == {
  "init", "seed-legacy-project", "connect", "disconnect", "request",
  "resolve", "inspection-fails", "cancel-before-admission",
  "lose-before-admission", "enqueue-admission", "cancel-after-admission",
  "lose-after-admission", "start-commit", "commit-new", "commit-existing",
  "pre-rename-failure", "post-rename-indeterminate", "retry",
  "reconcile-indeterminate", "client-retain", "client-delete",
  "reject-client-forge"
}

VARIABLES
  connected,
  requestPhase,
  requestKey,
  queue,
  activeRequest,
  storedProjects,
  canonicalProjects,
  storedWorkspaces,
  canonicalWorkspaces,
  emptyTerminalContexts,
  remoteDirectoriesAvailable,
  terminalEffectCount,
  lastAction

vars == <<connected, requestPhase, requestKey, queue, activeRequest,
  storedProjects, canonicalProjects, storedWorkspaces, canonicalWorkspaces,
  emptyTerminalContexts, remoteDirectoriesAvailable, terminalEffectCount,
  lastAction>>

Init ==
  /\ connected = FALSE
  /\ requestPhase = [request \in Requests |-> "idle"]
  /\ requestKey = [request \in Requests |-> NoKey]
  /\ queue = <<>>
  /\ activeRequest = "none"
  /\ storedProjects = {}
  /\ canonicalProjects = {}
  /\ storedWorkspaces = {}
  /\ canonicalWorkspaces = {}
  /\ emptyTerminalContexts = {}
  /\ remoteDirectoriesAvailable = [key \in Keys |-> FALSE]
  /\ terminalEffectCount = 0
  /\ lastAction = "init"

SeedLegacyProject(key) ==
  /\ key \notin storedProjects
  /\ \A request \in Requests: requestKey[request] # key
  /\ storedProjects' = storedProjects \cup {key}
  /\ canonicalProjects' = canonicalProjects \cup {key}
  /\ UNCHANGED <<connected, requestPhase, requestKey, queue, activeRequest,
       storedWorkspaces, canonicalWorkspaces, emptyTerminalContexts,
       remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "seed-legacy-project"

Connect ==
  /\ ~connected
  /\ connected' = TRUE
  /\ UNCHANGED <<requestPhase, requestKey, queue, activeRequest,
       storedProjects, canonicalProjects, storedWorkspaces,
       canonicalWorkspaces, emptyTerminalContexts, remoteDirectoriesAvailable,
       terminalEffectCount>>
  /\ lastAction' = "connect"

Disconnect ==
  /\ connected
  /\ connected' = FALSE
  /\ UNCHANGED <<requestPhase, requestKey, queue, activeRequest,
       storedProjects, canonicalProjects, storedWorkspaces,
       canonicalWorkspaces, emptyTerminalContexts, remoteDirectoriesAvailable,
       terminalEffectCount>>
  /\ lastAction' = "disconnect"

Request(request, key) ==
  /\ connected
  /\ requestPhase[request] = "idle"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "inspecting"]
  /\ requestKey' = [requestKey EXCEPT ![request] = key]
  /\ UNCHANGED <<connected, queue, activeRequest, storedProjects,
       canonicalProjects, storedWorkspaces, canonicalWorkspaces,
       emptyTerminalContexts, remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "request"

Resolve(request) ==
  /\ connected
  /\ requestPhase[request] = "inspecting"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "resolved"]
  /\ UNCHANGED <<connected, requestKey, queue, activeRequest, storedProjects,
       canonicalProjects, storedWorkspaces, canonicalWorkspaces,
       emptyTerminalContexts, remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "resolve"

InspectionFails(request) ==
  /\ requestPhase[request] = "inspecting"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "inspection-failed"]
  /\ UNCHANGED <<connected, requestKey, queue, activeRequest, storedProjects,
       canonicalProjects, storedWorkspaces, canonicalWorkspaces,
       emptyTerminalContexts, remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "inspection-fails"

CancelBeforeAdmission(request) ==
  /\ requestPhase[request] \in {"inspecting", "resolved"}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "cancelled"]
  /\ UNCHANGED <<connected, requestKey, queue, activeRequest, storedProjects,
       canonicalProjects, storedWorkspaces, canonicalWorkspaces,
       emptyTerminalContexts, remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "cancel-before-admission"

LoseBeforeAdmission(request) ==
  /\ connected
  /\ requestPhase[request] = "inspecting"
  /\ connected' = FALSE
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "inspection-failed"]
  /\ UNCHANGED <<requestKey, queue, activeRequest, storedProjects,
       canonicalProjects, storedWorkspaces, canonicalWorkspaces,
       emptyTerminalContexts, remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "lose-before-admission"

EnqueueAdmission(request) ==
  /\ requestPhase[request] = "resolved"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "queued"]
  /\ queue' = Append(queue, request)
  /\ UNCHANGED <<connected, requestKey, activeRequest, storedProjects,
       canonicalProjects, storedWorkspaces, canonicalWorkspaces,
       emptyTerminalContexts, remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "enqueue-admission"

CancelAfterAdmission(request) ==
  /\ requestPhase[request] \in {"queued", "committing"}
  /\ UNCHANGED <<connected, requestPhase, requestKey, queue, activeRequest,
       storedProjects, canonicalProjects, storedWorkspaces,
       canonicalWorkspaces, emptyTerminalContexts, remoteDirectoriesAvailable,
       terminalEffectCount>>
  /\ lastAction' = "cancel-after-admission"

LoseAfterAdmission(request) ==
  /\ connected
  /\ requestPhase[request] \in {"queued", "committing"}
  /\ connected' = FALSE
  /\ UNCHANGED <<requestPhase, requestKey, queue, activeRequest,
       storedProjects, canonicalProjects, storedWorkspaces,
       canonicalWorkspaces, emptyTerminalContexts, remoteDirectoriesAvailable,
       terminalEffectCount>>
  /\ lastAction' = "lose-after-admission"

StartCommit ==
  /\ activeRequest = "none"
  /\ Len(queue) > 0
  /\ LET request == Head(queue) IN
       /\ activeRequest' = request
       /\ queue' = Tail(queue)
       /\ requestPhase' = [requestPhase EXCEPT ![request] = "committing"]
  /\ UNCHANGED <<connected, requestKey, storedProjects, canonicalProjects,
       storedWorkspaces, canonicalWorkspaces, emptyTerminalContexts,
       remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "start-commit"

CommitNew(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ requestKey[request] \notin storedProjects
  /\ storedProjects' = storedProjects \cup {requestKey[request]}
  /\ canonicalProjects' = canonicalProjects \cup {requestKey[request]}
  /\ storedWorkspaces' = storedWorkspaces \cup {requestKey[request]}
  /\ canonicalWorkspaces' = canonicalWorkspaces \cup {requestKey[request]}
  /\ emptyTerminalContexts' = emptyTerminalContexts \cup {requestKey[request]}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "created"]
  /\ activeRequest' = "none"
  /\ UNCHANGED <<connected, requestKey, queue, remoteDirectoriesAvailable,
       terminalEffectCount>>
  /\ lastAction' = "commit-new"

CommitExisting(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ requestKey[request] \in storedProjects
  /\ canonicalProjects' = canonicalProjects \cup {requestKey[request]}
  /\ storedWorkspaces' = storedWorkspaces \cup {requestKey[request]}
  /\ canonicalWorkspaces' = canonicalWorkspaces \cup {requestKey[request]}
  /\ emptyTerminalContexts' = emptyTerminalContexts \cup {requestKey[request]}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "existing"]
  /\ activeRequest' = "none"
  /\ UNCHANGED <<connected, requestKey, queue, storedProjects,
       remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "commit-existing"

PreRenameFailure(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ requestKey[request] \notin storedWorkspaces
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "commit-failed"]
  /\ activeRequest' = "none"
  /\ UNCHANGED <<connected, requestKey, queue, storedProjects,
       canonicalProjects, storedWorkspaces, canonicalWorkspaces,
       emptyTerminalContexts, remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "pre-rename-failure"

PostRenameIndeterminate(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ requestKey[request] \notin storedWorkspaces
  /\ storedProjects' = storedProjects \cup {requestKey[request]}
  /\ storedWorkspaces' = storedWorkspaces \cup {requestKey[request]}
  /\ emptyTerminalContexts' = emptyTerminalContexts \cup {requestKey[request]}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "indeterminate"]
  /\ activeRequest' = "none"
  /\ UNCHANGED <<connected, requestKey, queue, canonicalProjects,
       canonicalWorkspaces, remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "post-rename-indeterminate"

Retry(request) ==
  /\ requestPhase[request] \in {"commit-failed", "indeterminate"}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "queued"]
  /\ queue' = Append(queue, request)
  /\ UNCHANGED <<connected, requestKey, activeRequest, storedProjects,
       canonicalProjects, storedWorkspaces, canonicalWorkspaces,
       emptyTerminalContexts, remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "retry"

ReconcileIndeterminate(request) ==
  /\ requestPhase[request] = "indeterminate"
  /\ requestKey[request] \in storedWorkspaces
  /\ canonicalProjects' = canonicalProjects \cup {requestKey[request]}
  /\ canonicalWorkspaces' = canonicalWorkspaces \cup {requestKey[request]}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "existing"]
  /\ UNCHANGED <<connected, requestKey, queue, activeRequest,
       storedProjects, storedWorkspaces, emptyTerminalContexts,
       remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "reconcile-indeterminate"

ClientRetain(key) ==
  /\ key \in canonicalWorkspaces
  /\ UNCHANGED <<connected, requestPhase, requestKey, queue, activeRequest,
       storedProjects, canonicalProjects, storedWorkspaces,
       canonicalWorkspaces, emptyTerminalContexts, remoteDirectoriesAvailable,
       terminalEffectCount>>
  /\ lastAction' = "client-retain"

ClientDelete(key) ==
  /\ key \in canonicalProjects
  /\ canonicalProjects' = canonicalProjects \ {key}
  /\ storedProjects' = storedProjects \ {key}
  /\ canonicalWorkspaces' = canonicalWorkspaces \ {key}
  /\ storedWorkspaces' = storedWorkspaces \ {key}
  /\ emptyTerminalContexts' = emptyTerminalContexts \ {key}
  /\ UNCHANGED <<connected, requestPhase, requestKey, queue, activeRequest,
       remoteDirectoriesAvailable, terminalEffectCount>>
  /\ lastAction' = "client-delete"

RejectClientForge(key) ==
  /\ key \notin canonicalWorkspaces
  /\ UNCHANGED <<connected, requestPhase, requestKey, queue, activeRequest,
       storedProjects, canonicalProjects, storedWorkspaces,
       canonicalWorkspaces, emptyTerminalContexts, remoteDirectoriesAvailable,
       terminalEffectCount>>
  /\ lastAction' = "reject-client-forge"

Next ==
  \/ \E key \in Keys: SeedLegacyProject(key)
  \/ Connect
  \/ Disconnect
  \/ \E request \in Requests, key \in Keys: Request(request, key)
  \/ \E request \in Requests: Resolve(request)
  \/ \E request \in Requests: InspectionFails(request)
  \/ \E request \in Requests: CancelBeforeAdmission(request)
  \/ \E request \in Requests: LoseBeforeAdmission(request)
  \/ \E request \in Requests: EnqueueAdmission(request)
  \/ \E request \in Requests: CancelAfterAdmission(request)
  \/ \E request \in Requests: LoseAfterAdmission(request)
  \/ StartCommit
  \/ \E request \in Requests: CommitNew(request)
  \/ \E request \in Requests: CommitExisting(request)
  \/ \E request \in Requests: PreRenameFailure(request)
  \/ \E request \in Requests: PostRenameIndeterminate(request)
  \/ \E request \in Requests: Retry(request)
  \/ \E request \in Requests: ReconcileIndeterminate(request)
  \/ \E key \in Keys: ClientRetain(key)
  \/ \E key \in Keys: ClientDelete(key)
  \/ \E key \in Keys: RejectClientForge(key)

Spec == Init /\ [][Next]_vars

QueueRequests == {queue[index] : index \in 1..Len(queue)}

TypeOK ==
  /\ connected \in BOOLEAN
  /\ requestPhase \in [Requests -> RequestPhases]
  /\ requestKey \in [Requests -> Keys \cup {NoKey}]
  /\ queue \in Seq(Requests)
  /\ activeRequest \in Requests \cup {"none"}
  /\ storedProjects \subseteq Keys
  /\ canonicalProjects \subseteq Keys
  /\ storedWorkspaces \subseteq Keys
  /\ canonicalWorkspaces \subseteq Keys
  /\ emptyTerminalContexts \subseteq Keys
  /\ remoteDirectoriesAvailable \in [Keys -> BOOLEAN]
  /\ terminalEffectCount \in Nat
  /\ lastAction \in Actions

QueueIsUnique == Len(queue) = Cardinality(QueueRequests)

InspectionFailureIsNotAdmitted ==
  \A request \in Requests:
    requestPhase[request] = "inspection-failed" =>
      /\ request \notin QueueRequests
      /\ activeRequest # request

CanonicalWasStored ==
  /\ canonicalProjects \subseteq storedProjects
  /\ canonicalWorkspaces \subseteq storedWorkspaces
  /\ canonicalWorkspaces \subseteq canonicalProjects

WorkspaceAndContextCommitAtomically ==
  /\ storedWorkspaces \subseteq storedProjects
  /\ storedWorkspaces = emptyTerminalContexts

CompletedAdmissionHasWorkspace ==
  \A request \in Requests:
    (requestPhase[request] \in {"created", "existing"}
      /\ requestKey[request] \in canonicalProjects) =>
      requestKey[request] \in canonicalWorkspaces

RemoteDirectoriesStayUnavailable ==
  \A key \in Keys: ~remoteDirectoriesAvailable[key]

AdmissionCreatesNoTerminalEffect == terminalEffectCount = 0

CancellationAfterAdmissionDoesNotRollBack ==
  lastAction = "cancel-after-admission" =>
    \E request \in Requests: requestPhase[request] \in {"queued", "committing"}

ClientCannotForgeRemotePlacement ==
  lastAction = "reject-client-forge" => canonicalWorkspaces \subseteq storedWorkspaces

=============================================================================
