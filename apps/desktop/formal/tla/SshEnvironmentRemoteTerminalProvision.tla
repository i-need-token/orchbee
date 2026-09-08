---------------- MODULE SshEnvironmentRemoteTerminalProvision ----------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded downstream model for Slice 3.2B.  The creation-surface model owns
\* D194 front-door admission and deterministic Request-to-Presentation
\* creation.  This model begins when that canonical Presentation is durable.
\* Each Presentation retains its exact SSH Environment authority through
\* deferred runtime preparation, Host dispatch, response loss, and inventory
\* reconciliation.  Multiple Presentations may share one Workspace; replay is
\* idempotent per Presentation and never falls back to the local Host.

CONSTANT Requests, Workspaces, Environments

ASSUME Cardinality(Requests) = 2
ASSUME Cardinality(Workspaces) = 1
ASSUME Cardinality(Environments) = 2

\* Requests are the finite, deterministic Presentation identities supplied by
\* the front-door model.  No request lifecycle is duplicated here.
Presentations == Requests
NoPresentation == "no-presentation"
NoWorkspace == "no-workspace"
NoEnvironment == "no-environment"
BindingPhases == {
  "none", "planned", "create-requested", "unknown", "inventory-missing",
  "retained", "stale"
}
HostKeys == Environments \X Presentations
Actions == {
  "init", "connect", "disconnect", "prepare-runtime", "preparation-fails",
  "observe-committed-presentation", "observe-existing-presentation",
  "remove-canonical-presentation", "retarget-canonical-presentation",
  "defer-unprepared", "plan-binding", "send-create", "host-settles",
  "host-settles-stale-target", "lose-before-response-uncommitted",
  "lose-before-response-committed", "repeat-request",
  "reconcile-inventory-present", "reconcile-inventory-missing",
  "retire-runtime", "client-retain", "reject-client-forge",
  "reject-local-fallback"
}

VARIABLE s

vars == <<s>>

BindingKey(environment, presentation) == <<environment, presentation>>

Init ==
  s = [
    connected |-> {},
    runtimeReady |-> {},
    canonicalPresentations |-> {},
    committedPresentations |-> {},
    presentationWorkspace |->
      [presentation \in Presentations |-> NoWorkspace],
    committedEnvironment |->
      [presentation \in Presentations |-> NoEnvironment],
    canonicalEnvironment |->
      [presentation \in Presentations |-> NoEnvironment],
    bindingPhase |-> [presentation \in Presentations |-> "none"],
    hostInventory |-> {},
    hostCreateCount |-> [presentation \in Presentations |-> 0],
    sendCount |-> [presentation \in Presentations |-> 0],
    missingReconciled |-> {},
    localEffectCount |-> 0,
    lastAction |-> "init",
    lastPresentation |-> NoPresentation,
    lastEnvironment |-> NoEnvironment,
    previousHostInventory |-> {},
    previousRuntimeReady |-> {}
  ]

Record(next, action, presentation, environment) ==
  [next EXCEPT
    !.lastAction = action,
    !.lastPresentation = presentation,
    !.lastEnvironment = environment,
    !.previousHostInventory = s.hostInventory,
    !.previousRuntimeReady = s.runtimeReady]

TargetIsCurrent(presentation) ==
  /\ presentation \in s.canonicalPresentations
  /\ s.canonicalEnvironment[presentation] =
       s.committedEnvironment[presentation]

Connect(environment) ==
  /\ environment \notin s.connected
  /\ s' = Record(
       [s EXCEPT !.connected = @ \cup {environment}],
       "connect",
       NoPresentation,
       environment)

Disconnect(environment) ==
  /\ environment \in s.connected
  /\ ~(\E presentation \in Presentations:
       /\ s.bindingPhase[presentation] = "create-requested"
       /\ s.committedEnvironment[presentation] = environment)
  /\ s' = Record(
       [s EXCEPT
         !.connected = @ \ {environment},
         !.runtimeReady = @ \ {environment}],
       "disconnect",
       NoPresentation,
       environment)

PrepareRuntime(environment) ==
  /\ environment \in s.connected
  /\ environment \notin s.runtimeReady
  /\ s' = Record(
       [s EXCEPT !.runtimeReady = @ \cup {environment}],
       "prepare-runtime",
       NoPresentation,
       environment)

PreparationFails(environment) ==
  /\ environment \notin s.runtimeReady
  /\ s' = Record(
       s,
       "preparation-fails",
       NoPresentation,
       environment)

ObserveCommittedPresentation(presentation, workspace, environment) ==
  /\ presentation \notin s.committedPresentations
  /\ s' = Record(
       [s EXCEPT
         !.canonicalPresentations = @ \cup {presentation},
         !.committedPresentations = @ \cup {presentation},
         !.presentationWorkspace[presentation] = workspace,
         !.committedEnvironment[presentation] = environment,
         !.canonicalEnvironment[presentation] = environment],
       "observe-committed-presentation",
       presentation,
       environment)

\* A separate operator makes the permissive multi-Presentation witness
\* explicit without reproducing the front-door queue.
ObserveSecondPresentation(presentation, workspace, environment) ==
  /\ \E existing \in s.committedPresentations \ {presentation}:
       s.presentationWorkspace[existing] = workspace
  /\ ObserveCommittedPresentation(presentation, workspace, environment)

ObserveExistingPresentation(presentation) ==
  /\ presentation \in s.canonicalPresentations
  /\ s' = Record(
       s,
       "observe-existing-presentation",
       presentation,
       s.canonicalEnvironment[presentation])

RemoveCanonicalPresentation(presentation) ==
  /\ presentation \in s.canonicalPresentations
  /\ s' = Record(
       [s EXCEPT
         !.canonicalPresentations = @ \ {presentation},
         !.canonicalEnvironment[presentation] = NoEnvironment,
         !.bindingPhase[presentation] =
           IF @ = "create-requested" THEN @ ELSE "stale"],
       "remove-canonical-presentation",
       presentation,
       s.committedEnvironment[presentation])

RetargetCanonicalPresentation(presentation, environment) ==
  /\ presentation \in s.canonicalPresentations
  /\ environment # s.canonicalEnvironment[presentation]
  /\ s' = Record(
       [s EXCEPT
         !.canonicalEnvironment[presentation] = environment,
         !.bindingPhase[presentation] =
           IF @ = "create-requested" THEN @ ELSE "stale"],
       "retarget-canonical-presentation",
       presentation,
       environment)

DeferUnprepared(presentation) ==
  /\ TargetIsCurrent(presentation)
  /\ s.committedEnvironment[presentation] \notin s.runtimeReady
  /\ s' = Record(
       s,
       "defer-unprepared",
       presentation,
       s.committedEnvironment[presentation])

PlanBinding(presentation) ==
  LET environment == s.committedEnvironment[presentation] IN
  LET key == BindingKey(environment, presentation) IN
  /\ TargetIsCurrent(presentation)
  /\ environment \in s.runtimeReady
  /\ s.bindingPhase[presentation] = "none"
  /\ key \notin s.hostInventory
  /\ s' = Record(
       [s EXCEPT !.bindingPhase[presentation] = "planned"],
       "plan-binding",
       presentation,
       environment)

SendCreate(presentation) ==
  LET environment == s.committedEnvironment[presentation] IN
  LET phase == s.bindingPhase[presentation] IN
  /\ TargetIsCurrent(presentation)
  /\ environment \in s.runtimeReady
  /\ BindingKey(environment, presentation) \notin s.hostInventory
  /\ phase \in {"planned", "inventory-missing"}
  /\ IF phase = "planned"
       THEN s.sendCount[presentation] = 0
       ELSE
         /\ presentation \in s.missingReconciled
         /\ s.sendCount[presentation] = 1
  /\ s' = Record(
       [s EXCEPT
         !.bindingPhase[presentation] = "create-requested",
         !.sendCount[presentation] = @ + 1],
       "send-create",
       presentation,
       environment)

HostSettles(presentation) ==
  LET environment == s.committedEnvironment[presentation] IN
  LET key == BindingKey(environment, presentation) IN
  /\ s.bindingPhase[presentation] = "create-requested"
  /\ TargetIsCurrent(presentation)
  /\ key \notin s.hostInventory
  /\ s' = Record(
       [s EXCEPT
         !.bindingPhase[presentation] = "retained",
         !.hostInventory = @ \cup {key},
         !.hostCreateCount[presentation] = @ + 1],
       "host-settles",
       presentation,
       environment)

HostSettlesStaleTarget(presentation) ==
  LET environment == s.committedEnvironment[presentation] IN
  LET key == BindingKey(environment, presentation) IN
  /\ s.bindingPhase[presentation] = "create-requested"
  /\ ~TargetIsCurrent(presentation)
  /\ key \notin s.hostInventory
  /\ s' = Record(
       [s EXCEPT
         !.bindingPhase[presentation] = "stale",
         !.hostInventory = @ \cup {key},
         !.hostCreateCount[presentation] = @ + 1],
       "host-settles-stale-target",
       presentation,
       environment)

LoseBeforeResponseUncommitted(presentation) ==
  LET environment == s.committedEnvironment[presentation] IN
  /\ s.bindingPhase[presentation] = "create-requested"
  /\ environment \in s.connected
  /\ s' = Record(
       [s EXCEPT
         !.connected = @ \ {environment},
         !.runtimeReady = @ \ {environment},
         !.bindingPhase[presentation] = "unknown"],
       "lose-before-response-uncommitted",
       presentation,
       environment)

LoseBeforeResponseCommitted(presentation) ==
  LET environment == s.committedEnvironment[presentation] IN
  LET key == BindingKey(environment, presentation) IN
  /\ s.bindingPhase[presentation] = "create-requested"
  /\ environment \in s.connected
  /\ key \notin s.hostInventory
  /\ s' = Record(
       [s EXCEPT
         !.connected = @ \ {environment},
         !.runtimeReady = @ \ {environment},
         !.bindingPhase[presentation] = "unknown",
         !.hostInventory = @ \cup {key},
         !.hostCreateCount[presentation] = @ + 1],
       "lose-before-response-committed",
       presentation,
       environment)

RepeatRequest(presentation) ==
  /\ presentation \in s.canonicalPresentations
  /\ presentation \in s.committedPresentations
  /\ s' = Record(
       s,
       "repeat-request",
       presentation,
       s.canonicalEnvironment[presentation])

ReconcileInventoryPresent(presentation) ==
  LET environment == s.committedEnvironment[presentation] IN
  /\ s.bindingPhase[presentation] = "unknown"
  /\ TargetIsCurrent(presentation)
  /\ environment \in s.runtimeReady
  /\ BindingKey(environment, presentation) \in s.hostInventory
  /\ s' = Record(
       [s EXCEPT !.bindingPhase[presentation] = "retained"],
       "reconcile-inventory-present",
       presentation,
       environment)

ReconcileInventoryMissing(presentation) ==
  LET environment == s.committedEnvironment[presentation] IN
  /\ s.bindingPhase[presentation] = "unknown"
  /\ TargetIsCurrent(presentation)
  /\ environment \in s.runtimeReady
  /\ BindingKey(environment, presentation) \notin s.hostInventory
  /\ s' = Record(
       [s EXCEPT
         !.bindingPhase[presentation] = "inventory-missing",
         !.missingReconciled = @ \cup {presentation}],
       "reconcile-inventory-missing",
       presentation,
       environment)

RetireRuntime(environment) ==
  /\ environment \in s.runtimeReady
  /\ s' = Record(
       [s EXCEPT !.runtimeReady = @ \ {environment}],
       "retire-runtime",
       NoPresentation,
       environment)

ClientRetain(presentation) ==
  /\ presentation \in s.canonicalPresentations
  /\ s' = Record(
       s,
       "client-retain",
       presentation,
       s.canonicalEnvironment[presentation])

RejectClientForge(presentation) ==
  /\ presentation \notin s.canonicalPresentations
  /\ s' = Record(
       s,
       "reject-client-forge",
       presentation,
       NoEnvironment)

RejectLocalFallback(presentation) ==
  /\ TargetIsCurrent(presentation)
  /\ s.committedEnvironment[presentation] \notin s.runtimeReady
  /\ s' = Record(
       s,
       "reject-local-fallback",
       presentation,
       s.committedEnvironment[presentation])

Next ==
  \/ \E environment \in Environments: Connect(environment)
  \/ \E environment \in Environments: Disconnect(environment)
  \/ \E environment \in Environments: PrepareRuntime(environment)
  \/ \E environment \in Environments: PreparationFails(environment)
  \/ \E presentation \in Presentations, workspace \in Workspaces,
       environment \in Environments:
       ObserveCommittedPresentation(presentation, workspace, environment)
  \/ \E presentation \in Presentations, workspace \in Workspaces,
       environment \in Environments:
       ObserveSecondPresentation(presentation, workspace, environment)
  \/ \E presentation \in Presentations:
       ObserveExistingPresentation(presentation)
  \/ \E presentation \in Presentations:
       RemoveCanonicalPresentation(presentation)
  \/ \E presentation \in Presentations, environment \in Environments:
       RetargetCanonicalPresentation(presentation, environment)
  \/ \E presentation \in Presentations: DeferUnprepared(presentation)
  \/ \E presentation \in Presentations: PlanBinding(presentation)
  \/ \E presentation \in Presentations: SendCreate(presentation)
  \/ \E presentation \in Presentations: HostSettles(presentation)
  \/ \E presentation \in Presentations:
       HostSettlesStaleTarget(presentation)
  \/ \E presentation \in Presentations:
       LoseBeforeResponseUncommitted(presentation)
  \/ \E presentation \in Presentations:
       LoseBeforeResponseCommitted(presentation)
  \/ \E presentation \in Presentations: RepeatRequest(presentation)
  \/ \E presentation \in Presentations:
       ReconcileInventoryPresent(presentation)
  \/ \E presentation \in Presentations:
       ReconcileInventoryMissing(presentation)
  \/ \E environment \in Environments: RetireRuntime(environment)
  \/ \E presentation \in Presentations: ClientRetain(presentation)
  \/ \E presentation \in Presentations: RejectClientForge(presentation)
  \/ \E presentation \in Presentations: RejectLocalFallback(presentation)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.connected \subseteq Environments
  /\ s.runtimeReady \subseteq s.connected
  /\ s.canonicalPresentations \subseteq Presentations
  /\ s.committedPresentations \subseteq Presentations
  /\ s.presentationWorkspace \in
       [Presentations -> Workspaces \cup {NoWorkspace}]
  /\ s.committedEnvironment \in
       [Presentations -> Environments \cup {NoEnvironment}]
  /\ s.canonicalEnvironment \in
       [Presentations -> Environments \cup {NoEnvironment}]
  /\ s.bindingPhase \in [Presentations -> BindingPhases]
  /\ s.hostInventory \subseteq HostKeys
  /\ s.hostCreateCount \in [Presentations -> 0..1]
  /\ s.sendCount \in [Presentations -> 0..2]
  /\ s.missingReconciled \subseteq Presentations
  /\ s.localEffectCount \in Nat
  /\ s.lastAction \in Actions
  /\ s.lastPresentation \in Presentations \cup {NoPresentation}
  /\ s.lastEnvironment \in Environments \cup {NoEnvironment}
  /\ s.previousHostInventory \subseteq HostKeys
  /\ s.previousRuntimeReady \subseteq Environments

CanonicalPresentationWasCommitted ==
  s.canonicalPresentations \subseteq s.committedPresentations

CommittedPresentationHasExactAuthority ==
  \A presentation \in s.committedPresentations:
    /\ s.presentationWorkspace[presentation] \in Workspaces
    /\ s.committedEnvironment[presentation] \in Environments

BindingRequiresCommittedPresentation ==
  \A presentation \in Presentations:
    s.bindingPhase[presentation] # "none" =>
      presentation \in s.committedPresentations

HostEffectRequiresCommittedPresentation ==
  \A key \in s.hostInventory:
    /\ key[2] \in s.committedPresentations
    /\ key[1] = s.committedEnvironment[key[2]]

AtMostOneHostTerminalPerPresentation ==
  \A presentation \in Presentations:
    LET inventoryForPresentation ==
      {key \in s.hostInventory: key[2] = presentation}
    IN
      /\ Cardinality(inventoryForPresentation) =
           s.hostCreateCount[presentation]
      /\ s.hostCreateCount[presentation] \in 0..1

RetainedBindingMatchesExactTarget ==
  \A presentation \in Presentations:
    s.bindingPhase[presentation] = "retained" =>
      /\ TargetIsCurrent(presentation)
      /\ BindingKey(
           s.committedEnvironment[presentation],
           presentation
         ) \in s.hostInventory

NoBlindDuplicateCreate ==
  \A presentation \in Presentations:
    /\ s.sendCount[presentation] >= s.hostCreateCount[presentation]
    /\ (s.sendCount[presentation] = 2 =>
         presentation \in s.missingReconciled)
    /\ (s.bindingPhase[presentation] = "unknown" =>
         s.sendCount[presentation] >= 1)

CreateRequiresReadyExactEnvironment ==
  s.lastAction = "send-create" =>
    /\ s.lastPresentation \in Presentations
    /\ TargetIsCurrent(s.lastPresentation)
    /\ s.lastEnvironment =
         s.committedEnvironment[s.lastPresentation]
    /\ s.lastEnvironment \in s.runtimeReady

ReconnectDoesNotActivateRuntime ==
  s.lastAction = "connect" =>
    s.runtimeReady = s.previousRuntimeReady

NoLocalFallback == s.localEffectCount = 0

RuntimeRetirementPreservesRemoteState ==
  s.lastAction = "retire-runtime" =>
    s.hostInventory = s.previousHostInventory

ExactReplayDoesNotAddPresentation ==
  s.lastAction = "observe-existing-presentation" =>
    s.lastPresentation \in s.canonicalPresentations

=============================================================================
