-------------------- MODULE DesktopTerminalLifecycleAdapter --------------------
EXTENDS FiniteSets, TLC

Presentations == {"presentation-1", "presentation-2"}
Bindings == {"binding-1", "binding-2"}
Geometries == {"geometry-1", "geometry-2"}
NoBinding == "no-binding"
NoGeometry == "no-geometry"
NoTarget == "no-target"
Phases == {"absent", "bound", "retired"}
Effects == {"none", "provision", "restore", "start", "close"}
StartPhases == {"none", "awaiting-mounted-geometry", "starting", "running"}
Actions == {
  "init",
  "fresh-provision",
  "retry-provision",
  "start-restore",
  "queue-close",
  "start-close",
  "provision-completed",
  "provision-failed",
  "restore-completed",
  "restore-failed",
  "mounted-geometry-observed",
  "start-completed",
  "start-failed",
  "close-completed",
  "close-failed",
  "unbound-close"
}

VARIABLES
  phase,
  binding,
  effect,
  startPhase,
  pendingGeometry,
  initialGeometry,
  queuedClose,
  lastAction,
  lastTarget,
  previousPhase,
  previousBinding,
  previousEffect,
  previousStartPhase,
  previousPendingGeometry,
  previousInitialGeometry,
  previousQueuedClose

vars ==
  <<phase, binding, effect, startPhase, pendingGeometry, initialGeometry,
    queuedClose, lastAction, lastTarget, previousPhase, previousBinding,
    previousEffect, previousStartPhase, previousPendingGeometry,
    previousInitialGeometry, previousQueuedClose>>

Record(action, target) ==
  /\ lastAction' = action
  /\ lastTarget' = target
  /\ previousPhase' = phase
  /\ previousBinding' = binding
  /\ previousEffect' = effect
  /\ previousStartPhase' = startPhase
  /\ previousPendingGeometry' = pendingGeometry
  /\ previousInitialGeometry' = initialGeometry
  /\ previousQueuedClose' = queuedClose

Init ==
  /\ phase = [presentation \in Presentations |-> "absent"]
  /\ binding = [presentation \in Presentations |-> NoBinding]
  /\ effect = [presentation \in Presentations |-> "none"]
  /\ startPhase = [presentation \in Presentations |-> "none"]
  /\ pendingGeometry = [presentation \in Presentations |-> NoGeometry]
  /\ initialGeometry = [presentation \in Presentations |-> NoGeometry]
  /\ queuedClose = [presentation \in Presentations |-> FALSE]
  /\ lastAction = "init"
  /\ lastTarget = NoTarget
  /\ previousPhase = phase
  /\ previousBinding = binding
  /\ previousEffect = effect
  /\ previousStartPhase = startPhase
  /\ previousPendingGeometry = pendingGeometry
  /\ previousInitialGeometry = initialGeometry
  /\ previousQueuedClose = queuedClose

FreshProvision(presentation, candidateBinding) ==
  /\ phase[presentation] = "absent"
  /\ effect[presentation] = "none"
  /\ \A owner \in Presentations: binding[owner] # candidateBinding
  /\ phase' = [phase EXCEPT ![presentation] = "bound"]
  /\ binding' = [binding EXCEPT ![presentation] = candidateBinding]
  /\ effect' = [effect EXCEPT ![presentation] = "provision"]
  /\ UNCHANGED <<startPhase, pendingGeometry, initialGeometry, queuedClose>>
  /\ Record("fresh-provision", presentation)

RetryProvision(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "none"
  /\ startPhase[presentation] = "none"
  /\ ~queuedClose[presentation]
  /\ effect' = [effect EXCEPT ![presentation] = "provision"]
  /\ UNCHANGED <<phase, binding, startPhase, pendingGeometry,
                  initialGeometry, queuedClose>>
  /\ Record("retry-provision", presentation)

StartRestore(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "none"
  /\ startPhase[presentation] = "none"
  /\ ~queuedClose[presentation]
  /\ effect' = [effect EXCEPT ![presentation] = "restore"]
  /\ UNCHANGED <<phase, binding, startPhase, pendingGeometry,
                  initialGeometry, queuedClose>>
  /\ Record("start-restore", presentation)

QueueClose(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] \in {"provision", "start"}
  /\ ~queuedClose[presentation]
  /\ queuedClose' = [queuedClose EXCEPT ![presentation] = TRUE]
  /\ UNCHANGED <<phase, binding, effect, startPhase,
                  pendingGeometry, initialGeometry>>
  /\ Record("queue-close", presentation)

StartClose(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "none"
  /\ ~queuedClose[presentation]
  /\ effect' = [effect EXCEPT ![presentation] = "close"]
  /\ UNCHANGED <<phase, binding, startPhase, pendingGeometry,
                  initialGeometry, queuedClose>>
  /\ Record("start-close", presentation)

FinishProvision(presentation, action) ==
  /\ action \in {"provision-completed", "provision-failed"}
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "provision"
  /\ effect' = [effect EXCEPT
       ![presentation] = IF queuedClose[presentation] THEN "close" ELSE "none"]
  /\ startPhase' = [startPhase EXCEPT
       ![presentation] = IF action = "provision-completed"
         THEN "awaiting-mounted-geometry"
         ELSE @]
  /\ queuedClose' = [queuedClose EXCEPT ![presentation] = FALSE]
  /\ UNCHANGED <<phase, binding, pendingGeometry, initialGeometry>>
  /\ Record(action, presentation)

FinishRestore(presentation, action, restoredGeometry) ==
  /\ action \in {"restore-completed", "restore-failed"}
  /\ restoredGeometry \in Geometries \cup {NoGeometry}
  /\ action = "restore-completed" => restoredGeometry \in Geometries \cup {NoGeometry}
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "restore"
  /\ effect' = [effect EXCEPT ![presentation] = "none"]
  /\ startPhase' = [startPhase EXCEPT
       ![presentation] = IF action = "restore-completed"
         THEN IF restoredGeometry = NoGeometry
           THEN "awaiting-mounted-geometry"
           ELSE "running"
         ELSE @]
  /\ initialGeometry' = [initialGeometry EXCEPT
       ![presentation] = IF action = "restore-completed" THEN restoredGeometry ELSE @]
  /\ UNCHANGED <<phase, binding, pendingGeometry, queuedClose>>
  /\ Record(action, presentation)

ObserveMountedGeometry(presentation, geometry) ==
  /\ geometry \in Geometries
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "none"
  /\ startPhase[presentation] = "awaiting-mounted-geometry"
  /\ ~queuedClose[presentation]
  /\ effect' = [effect EXCEPT ![presentation] = "start"]
  /\ startPhase' = [startPhase EXCEPT ![presentation] = "starting"]
  /\ pendingGeometry' = [pendingGeometry EXCEPT ![presentation] = geometry]
  /\ UNCHANGED <<phase, binding, initialGeometry, queuedClose>>
  /\ Record("mounted-geometry-observed", presentation)

FinishStart(presentation, action) ==
  /\ action \in {"start-completed", "start-failed"}
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "start"
  /\ startPhase[presentation] = "starting"
  /\ pendingGeometry[presentation] \in Geometries
  /\ effect' = [effect EXCEPT
       ![presentation] = IF queuedClose[presentation] THEN "close" ELSE "none"]
  /\ startPhase' = [startPhase EXCEPT
       ![presentation] = IF action = "start-completed"
         THEN "running"
         ELSE "awaiting-mounted-geometry"]
  /\ initialGeometry' = [initialGeometry EXCEPT
       ![presentation] = IF action = "start-completed"
         THEN pendingGeometry[presentation]
         ELSE NoGeometry]
  /\ pendingGeometry' = [pendingGeometry EXCEPT ![presentation] = NoGeometry]
  /\ queuedClose' = [queuedClose EXCEPT ![presentation] = FALSE]
  /\ UNCHANGED <<phase, binding>>
  /\ Record(action, presentation)

CloseCompletes(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "close"
  /\ phase' = [phase EXCEPT ![presentation] = "retired"]
  /\ binding' = [binding EXCEPT ![presentation] = NoBinding]
  /\ effect' = [effect EXCEPT ![presentation] = "none"]
  /\ startPhase' = [startPhase EXCEPT ![presentation] = "none"]
  /\ pendingGeometry' = [pendingGeometry EXCEPT ![presentation] = NoGeometry]
  /\ initialGeometry' = [initialGeometry EXCEPT ![presentation] = NoGeometry]
  /\ queuedClose' = [queuedClose EXCEPT ![presentation] = FALSE]
  /\ Record("close-completed", presentation)

CloseFails(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "close"
  /\ effect' = [effect EXCEPT ![presentation] = "none"]
  /\ queuedClose' = [queuedClose EXCEPT ![presentation] = FALSE]
  /\ UNCHANGED <<phase, binding, startPhase, pendingGeometry, initialGeometry>>
  /\ Record("close-failed", presentation)

CloseUnbound(presentation) ==
  /\ phase[presentation] = "absent"
  /\ effect[presentation] = "none"
  /\ phase' = [phase EXCEPT ![presentation] = "retired"]
  /\ UNCHANGED <<binding, effect, startPhase, pendingGeometry,
                  initialGeometry, queuedClose>>
  /\ Record("unbound-close", presentation)

Next ==
  \/ \E presentation \in Presentations, candidateBinding \in Bindings:
       FreshProvision(presentation, candidateBinding)
  \/ \E presentation \in Presentations: RetryProvision(presentation)
  \/ \E presentation \in Presentations: StartRestore(presentation)
  \/ \E presentation \in Presentations: QueueClose(presentation)
  \/ \E presentation \in Presentations: StartClose(presentation)
  \/ \E presentation \in Presentations:
       FinishProvision(presentation, "provision-completed")
  \/ \E presentation \in Presentations:
       FinishProvision(presentation, "provision-failed")
  \/ \E presentation \in Presentations:
       \E restoredGeometry \in Geometries \cup {NoGeometry}:
         FinishRestore(presentation, "restore-completed", restoredGeometry)
  \/ \E presentation \in Presentations:
       FinishRestore(presentation, "restore-failed", NoGeometry)
  \/ \E presentation \in Presentations, geometry \in Geometries:
       ObserveMountedGeometry(presentation, geometry)
  \/ \E presentation \in Presentations:
       FinishStart(presentation, "start-completed")
  \/ \E presentation \in Presentations:
       FinishStart(presentation, "start-failed")
  \/ \E presentation \in Presentations: CloseCompletes(presentation)
  \/ \E presentation \in Presentations: CloseFails(presentation)
  \/ \E presentation \in Presentations: CloseUnbound(presentation)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in [Presentations -> Phases]
  /\ binding \in [Presentations -> Bindings \cup {NoBinding}]
  /\ effect \in [Presentations -> Effects]
  /\ startPhase \in [Presentations -> StartPhases]
  /\ pendingGeometry \in [Presentations -> Geometries \cup {NoGeometry}]
  /\ initialGeometry \in [Presentations -> Geometries \cup {NoGeometry}]
  /\ queuedClose \in [Presentations -> BOOLEAN]
  /\ lastAction \in Actions
  /\ lastTarget \in Presentations \cup {NoTarget}
  /\ previousPhase \in [Presentations -> Phases]
  /\ previousBinding \in [Presentations -> Bindings \cup {NoBinding}]
  /\ previousEffect \in [Presentations -> Effects]
  /\ previousStartPhase \in [Presentations -> StartPhases]
  /\ previousPendingGeometry \in [Presentations -> Geometries \cup {NoGeometry}]
  /\ previousInitialGeometry \in [Presentations -> Geometries \cup {NoGeometry}]
  /\ previousQueuedClose \in [Presentations -> BOOLEAN]

MembershipMatchesPhase ==
  \A presentation \in Presentations:
    (phase[presentation] = "bound") <=> (binding[presentation] # NoBinding)

BindingBijection ==
  \A left, right \in Presentations:
    (left # right /\ binding[left] # NoBinding) => binding[left] # binding[right]

EffectRequiresLiveBinding ==
  \A presentation \in Presentations:
    effect[presentation] # "none" => phase[presentation] = "bound"

QueuedCloseFollowsProvision ==
  \A presentation \in Presentations:
    queuedClose[presentation] => effect[presentation] \in {"provision", "start"}

MountedGeometryGatesStart ==
  \A presentation \in Presentations:
    /\ (phase[presentation] # "bound") =>
         /\ startPhase[presentation] = "none"
         /\ pendingGeometry[presentation] = NoGeometry
         /\ initialGeometry[presentation] = NoGeometry
    /\ startPhase[presentation] = "none" =>
         /\ pendingGeometry[presentation] = NoGeometry
         /\ initialGeometry[presentation] = NoGeometry
    /\ startPhase[presentation] = "awaiting-mounted-geometry" =>
         /\ pendingGeometry[presentation] = NoGeometry
         /\ initialGeometry[presentation] = NoGeometry
    /\ startPhase[presentation] = "starting" =>
         /\ effect[presentation] = "start"
         /\ pendingGeometry[presentation] \in Geometries
         /\ initialGeometry[presentation] = NoGeometry
    /\ startPhase[presentation] = "running" =>
         /\ pendingGeometry[presentation] = NoGeometry
         /\ initialGeometry[presentation] \in Geometries

TargetedMutationPreservesOthers ==
  lastTarget # NoTarget =>
    \A other \in Presentations \ {lastTarget}:
      /\ phase[other] = previousPhase[other]
      /\ binding[other] = previousBinding[other]
      /\ effect[other] = previousEffect[other]
      /\ startPhase[other] = previousStartPhase[other]
      /\ pendingGeometry[other] = previousPendingGeometry[other]
      /\ initialGeometry[other] = previousInitialGeometry[other]
      /\ queuedClose[other] = previousQueuedClose[other]

FailurePreservesMembership ==
  lastAction \in {"provision-failed", "restore-failed", "close-failed"} =>
    /\ phase = previousPhase
    /\ binding = previousBinding

RestoreNeverAllocatesBinding ==
  lastAction \in {"start-restore", "restore-completed", "restore-failed"} =>
    /\ binding = previousBinding
    /\ binding[lastTarget] # NoBinding

CloseCompletionRetiresTarget ==
  lastAction \in {"close-completed", "unbound-close"} =>
    /\ phase[lastTarget] = "retired"
    /\ binding[lastTarget] = NoBinding

QueuedCloseStartsAfterProvision ==
  /\ lastAction \in {
       "provision-completed", "provision-failed", "start-completed", "start-failed"
     }
  /\ previousQueuedClose[lastTarget]
  =>
    /\ effect[lastTarget] = "close"
    /\ ~queuedClose[lastTarget]

StartCommitUsesMountedGeometry ==
  lastAction = "start-completed" =>
    /\ previousPendingGeometry[lastTarget] \in Geometries
    /\ startPhase[lastTarget] = "running"
    /\ initialGeometry[lastTarget] = previousPendingGeometry[lastTarget]
    /\ pendingGeometry[lastTarget] = NoGeometry

StartFailureLeavesNoChildGeometry ==
  lastAction = "start-failed" =>
    /\ phase = previousPhase
    /\ binding = previousBinding
    /\ startPhase[lastTarget] = "awaiting-mounted-geometry"
    /\ pendingGeometry[lastTarget] = NoGeometry
    /\ initialGeometry[lastTarget] = NoGeometry

=============================================================================
