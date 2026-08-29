---------------- MODULE SshEnvironmentRemoteTerminalProvision ----------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

\* Bounded model of Slice 3.2B. One explicit Main request prepares a remote
\* Terminal Host relay, durably commits the first Presentation intent of an
\* admitted remote Workspace, and only then reconciles one Environment-scoped
\* binding. Attachment loss may make the Host outcome unknown. A later
\* explicit repeat uses inventory and the same binding identity; it never
\* falls back to the local Host or blindly creates a duplicate terminal.

CONSTANT Requests, Workspaces

NoRequest == "no-request"
NoWorkspace == "no-workspace"
RequestPhases == {
  "idle", "requested", "prepared", "queued", "committing", "committed",
  "existing", "cancelled", "prepare-failed", "commit-failed"
}
BindingPhases == {"none", "planned", "create-requested", "unknown", "retained"}
Actions == {
  "init", "connect", "disconnect", "request", "prepare-runtime",
  "preparation-fails", "cancel-before-queue", "enqueue", "cancel-after-queue",
  "start-commit", "commit-new", "commit-existing", "pre-commit-failure",
  "defer-unprepared", "plan-binding", "send-create", "host-settles",
  "lose-before-response-uncommitted", "lose-before-response-committed",
  "repeat-request", "reconcile-inventory-present",
  "reconcile-inventory-missing", "retire-runtime", "client-retain",
  "client-delete", "reject-client-forge", "reject-local-fallback"
}

VARIABLES
  connected,
  runtimeReady,
  requestPhase,
  requestWorkspace,
  queue,
  activeRequest,
  canonicalIntents,
  committedIntents,
  bindingPhase,
  hostTerminal,
  hostCreateCount,
  sendCount,
  localEffectCount,
  lastAction

vars == <<connected, runtimeReady, requestPhase, requestWorkspace, queue,
  activeRequest, canonicalIntents, committedIntents, bindingPhase,
  hostTerminal, hostCreateCount, sendCount, localEffectCount, lastAction>>

Init ==
  /\ connected = FALSE
  /\ runtimeReady = FALSE
  /\ requestPhase = [request \in Requests |-> "idle"]
  /\ requestWorkspace = [request \in Requests |-> NoWorkspace]
  /\ queue = <<>>
  /\ activeRequest = NoRequest
  /\ canonicalIntents = {}
  /\ committedIntents = {}
  /\ bindingPhase = [workspace \in Workspaces |-> "none"]
  /\ hostTerminal = [workspace \in Workspaces |-> FALSE]
  /\ hostCreateCount = [workspace \in Workspaces |-> 0]
  /\ sendCount = [workspace \in Workspaces |-> 0]
  /\ localEffectCount = 0
  /\ lastAction = "init"

Connect ==
  /\ ~connected
  /\ connected' = TRUE
  /\ UNCHANGED <<runtimeReady, requestPhase, requestWorkspace, queue,
       activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "connect"

Disconnect ==
  /\ connected
  /\ connected' = FALSE
  /\ runtimeReady' = FALSE
  /\ UNCHANGED <<requestPhase, requestWorkspace, queue, activeRequest,
       canonicalIntents, committedIntents, bindingPhase, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "disconnect"

Request(request, workspace) ==
  /\ connected
  /\ requestPhase[request] = "idle"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "requested"]
  /\ requestWorkspace' = [requestWorkspace EXCEPT ![request] = workspace]
  /\ UNCHANGED <<connected, runtimeReady, queue, activeRequest,
       canonicalIntents, committedIntents, bindingPhase, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "request"

PrepareRuntime(request) ==
  /\ connected
  /\ requestPhase[request] = "requested"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "prepared"]
  /\ runtimeReady' = TRUE
  /\ UNCHANGED <<connected, requestWorkspace, queue, activeRequest,
       canonicalIntents, committedIntents, bindingPhase, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "prepare-runtime"

PreparationFails(request) ==
  /\ requestPhase[request] = "requested"
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "prepare-failed"]
  /\ UNCHANGED <<connected, runtimeReady, requestWorkspace, queue,
       activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "preparation-fails"

CancelBeforeQueue(request) ==
  /\ requestPhase[request] \in {"requested", "prepared"}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "cancelled"]
  /\ UNCHANGED <<connected, runtimeReady, requestWorkspace, queue,
       activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "cancel-before-queue"

Enqueue(request) ==
  /\ requestPhase[request] = "prepared"
  /\ request \notin {queue[index] : index \in 1..Len(queue)}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "queued"]
  /\ queue' = Append(queue, request)
  /\ UNCHANGED <<connected, runtimeReady, requestWorkspace, activeRequest,
       canonicalIntents, committedIntents, bindingPhase, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "enqueue"

CancelAfterQueue(request) ==
  /\ requestPhase[request] \in {"queued", "committing"}
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "cancel-after-queue"

StartCommit ==
  /\ activeRequest = NoRequest
  /\ Len(queue) > 0
  /\ LET request == Head(queue) IN
       /\ activeRequest' = request
       /\ queue' = Tail(queue)
       /\ requestPhase' = [requestPhase EXCEPT ![request] = "committing"]
  /\ UNCHANGED <<connected, runtimeReady, requestWorkspace, canonicalIntents,
       committedIntents, bindingPhase, hostTerminal, hostCreateCount,
       sendCount, localEffectCount>>
  /\ lastAction' = "start-commit"

CommitNew(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ requestWorkspace[request] \notin canonicalIntents
  /\ canonicalIntents' = canonicalIntents \cup {requestWorkspace[request]}
  /\ committedIntents' = committedIntents \cup {requestWorkspace[request]}
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "committed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<connected, runtimeReady, requestWorkspace, queue,
       bindingPhase, hostTerminal, hostCreateCount, sendCount,
       localEffectCount>>
  /\ lastAction' = "commit-new"

CommitExisting(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ requestWorkspace[request] \in canonicalIntents
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "existing"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<connected, runtimeReady, requestWorkspace, queue,
       canonicalIntents, committedIntents, bindingPhase, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "commit-existing"

PreCommitFailure(request) ==
  /\ activeRequest = request
  /\ requestPhase[request] = "committing"
  /\ requestWorkspace[request] \notin canonicalIntents
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "commit-failed"]
  /\ activeRequest' = NoRequest
  /\ UNCHANGED <<connected, runtimeReady, requestWorkspace, queue,
       canonicalIntents, committedIntents, bindingPhase, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "pre-commit-failure"

DeferUnprepared(workspace) ==
  /\ workspace \in canonicalIntents
  /\ ~runtimeReady
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "defer-unprepared"

PlanBinding(workspace) ==
  /\ workspace \in canonicalIntents
  /\ runtimeReady
  /\ bindingPhase[workspace] = "none"
  /\ bindingPhase' = [bindingPhase EXCEPT ![workspace] = "planned"]
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "plan-binding"

SendCreate(workspace) ==
  /\ runtimeReady
  /\ bindingPhase[workspace] = "planned"
  /\ ~hostTerminal[workspace]
  /\ sendCount[workspace] < 2
  /\ bindingPhase' = [bindingPhase EXCEPT ![workspace] = "create-requested"]
  /\ sendCount' = [sendCount EXCEPT ![workspace] = @ + 1]
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, hostTerminal,
       hostCreateCount, localEffectCount>>
  /\ lastAction' = "send-create"

HostSettles(workspace) ==
  /\ runtimeReady
  /\ bindingPhase[workspace] = "create-requested"
  /\ ~hostTerminal[workspace]
  /\ bindingPhase' = [bindingPhase EXCEPT ![workspace] = "retained"]
  /\ hostTerminal' = [hostTerminal EXCEPT ![workspace] = TRUE]
  /\ hostCreateCount' = [hostCreateCount EXCEPT ![workspace] = @ + 1]
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, sendCount,
       localEffectCount>>
  /\ lastAction' = "host-settles"

LoseBeforeResponseUncommitted(workspace) ==
  /\ bindingPhase[workspace] = "create-requested"
  /\ ~hostTerminal[workspace]
  /\ connected' = FALSE
  /\ runtimeReady' = FALSE
  /\ bindingPhase' = [bindingPhase EXCEPT ![workspace] = "unknown"]
  /\ UNCHANGED <<requestPhase, requestWorkspace, queue, activeRequest,
       canonicalIntents, committedIntents, hostTerminal, hostCreateCount,
       sendCount, localEffectCount>>
  /\ lastAction' = "lose-before-response-uncommitted"

LoseBeforeResponseCommitted(workspace) ==
  /\ bindingPhase[workspace] = "create-requested"
  /\ ~hostTerminal[workspace]
  /\ connected' = FALSE
  /\ runtimeReady' = FALSE
  /\ bindingPhase' = [bindingPhase EXCEPT ![workspace] = "unknown"]
  /\ hostTerminal' = [hostTerminal EXCEPT ![workspace] = TRUE]
  /\ hostCreateCount' = [hostCreateCount EXCEPT ![workspace] = @ + 1]
  /\ UNCHANGED <<requestPhase, requestWorkspace, queue, activeRequest,
       canonicalIntents, committedIntents, sendCount, localEffectCount>>
  /\ lastAction' = "lose-before-response-committed"

RepeatRequest(request) ==
  /\ connected
  /\ requestPhase[request] \in {
       "committed", "existing", "prepare-failed", "commit-failed", "cancelled"
     }
  /\ requestWorkspace[request] \in canonicalIntents
  /\ requestPhase' = [requestPhase EXCEPT ![request] = "requested"]
  /\ UNCHANGED <<connected, runtimeReady, requestWorkspace, queue,
       activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "repeat-request"

ReconcileInventoryPresent(workspace) ==
  /\ runtimeReady
  /\ bindingPhase[workspace] = "unknown"
  /\ hostTerminal[workspace]
  /\ bindingPhase' = [bindingPhase EXCEPT ![workspace] = "retained"]
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "reconcile-inventory-present"

ReconcileInventoryMissing(workspace) ==
  /\ runtimeReady
  /\ bindingPhase[workspace] = "unknown"
  /\ ~hostTerminal[workspace]
  /\ bindingPhase' = [bindingPhase EXCEPT ![workspace] = "planned"]
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "reconcile-inventory-missing"

RetireRuntime ==
  /\ runtimeReady
  /\ runtimeReady' = FALSE
  /\ UNCHANGED <<connected, requestPhase, requestWorkspace, queue,
       activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "retire-runtime"

ClientRetain(workspace) ==
  /\ workspace \in canonicalIntents
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "client-retain"

ClientDelete(workspace) ==
  /\ workspace \in canonicalIntents
  /\ canonicalIntents' = canonicalIntents \ {workspace}
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, committedIntents, bindingPhase, hostTerminal,
       hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "client-delete"

RejectClientForge(workspace) ==
  /\ workspace \notin canonicalIntents
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "reject-client-forge"

RejectLocalFallback(workspace) ==
  /\ workspace \in canonicalIntents
  /\ ~runtimeReady
  /\ UNCHANGED <<connected, runtimeReady, requestPhase, requestWorkspace,
       queue, activeRequest, canonicalIntents, committedIntents, bindingPhase,
       hostTerminal, hostCreateCount, sendCount, localEffectCount>>
  /\ lastAction' = "reject-local-fallback"

Next ==
  \/ Connect
  \/ Disconnect
  \/ \E request \in Requests, workspace \in Workspaces: Request(request, workspace)
  \/ \E request \in Requests: PrepareRuntime(request)
  \/ \E request \in Requests: PreparationFails(request)
  \/ \E request \in Requests: CancelBeforeQueue(request)
  \/ \E request \in Requests: Enqueue(request)
  \/ \E request \in Requests: CancelAfterQueue(request)
  \/ StartCommit
  \/ \E request \in Requests: CommitNew(request)
  \/ \E request \in Requests: CommitExisting(request)
  \/ \E request \in Requests: PreCommitFailure(request)
  \/ \E workspace \in Workspaces: DeferUnprepared(workspace)
  \/ \E workspace \in Workspaces: PlanBinding(workspace)
  \/ \E workspace \in Workspaces: SendCreate(workspace)
  \/ \E workspace \in Workspaces: HostSettles(workspace)
  \/ \E workspace \in Workspaces: LoseBeforeResponseUncommitted(workspace)
  \/ \E workspace \in Workspaces: LoseBeforeResponseCommitted(workspace)
  \/ \E request \in Requests: RepeatRequest(request)
  \/ \E workspace \in Workspaces: ReconcileInventoryPresent(workspace)
  \/ \E workspace \in Workspaces: ReconcileInventoryMissing(workspace)
  \/ RetireRuntime
  \/ \E workspace \in Workspaces: ClientRetain(workspace)
  \/ \E workspace \in Workspaces: ClientDelete(workspace)
  \/ \E workspace \in Workspaces: RejectClientForge(workspace)
  \/ \E workspace \in Workspaces: RejectLocalFallback(workspace)

Spec == Init /\ [][Next]_vars

QueueRequests == {queue[index] : index \in 1..Len(queue)}

TypeOK ==
  /\ connected \in BOOLEAN
  /\ runtimeReady \in BOOLEAN
  /\ requestPhase \in [Requests -> RequestPhases]
  /\ requestWorkspace \in [Requests -> Workspaces \cup {NoWorkspace}]
  /\ queue \in Seq(Requests)
  /\ activeRequest \in Requests \cup {NoRequest}
  /\ canonicalIntents \subseteq Workspaces
  /\ committedIntents \subseteq Workspaces
  /\ bindingPhase \in [Workspaces -> BindingPhases]
  /\ hostTerminal \in [Workspaces -> BOOLEAN]
  /\ hostCreateCount \in [Workspaces -> 0..1]
  /\ sendCount \in [Workspaces -> 0..2]
  /\ localEffectCount \in Nat
  /\ lastAction \in Actions

QueueIsUnique == Len(queue) = Cardinality(QueueRequests)

CanonicalIntentWasCommitted == canonicalIntents \subseteq committedIntents

BindingRequiresCommittedIntent ==
  \A workspace \in Workspaces:
    bindingPhase[workspace] # "none" => workspace \in committedIntents

HostEffectRequiresCommittedIntent ==
  \A workspace \in Workspaces:
    hostTerminal[workspace] => workspace \in committedIntents

AtMostOneHostTerminalPerWorkspace ==
  \A workspace \in Workspaces:
    /\ hostCreateCount[workspace] \in 0..1
    /\ hostTerminal[workspace] = (hostCreateCount[workspace] = 1)

NoBlindDuplicateCreate ==
  \A workspace \in Workspaces:
    /\ sendCount[workspace] >= hostCreateCount[workspace]
    /\ bindingPhase[workspace] = "planned" => ~hostTerminal[workspace]

CreateRequiresReadyRemoteRuntime ==
  lastAction = "send-create" => runtimeReady

ReconnectDoesNotActivateRuntime ==
  lastAction = "connect" => ~runtimeReady

NoLocalFallback == localEffectCount = 0

RuntimeRetirementPreservesRemoteState ==
  lastAction = "retire-runtime" =>
    \A workspace \in Workspaces:
      hostTerminal[workspace] = (hostCreateCount[workspace] = 1)

=============================================================================
