-------------------- MODULE DesktopTerminalLifecycleAdapter --------------------
EXTENDS FiniteSets, TLC

Presentations == {"presentation-1", "presentation-2"}
Bindings == {"binding-1", "binding-2"}
NoBinding == "no-binding"
NoTarget == "no-target"
Phases == {"absent", "bound", "retired"}
Effects == {"none", "provision", "close"}
Actions == {
  "init",
  "fresh-provision",
  "retry-provision",
  "queue-close",
  "start-close",
  "provision-completed",
  "provision-failed",
  "close-completed",
  "close-failed",
  "unbound-close"
}

VARIABLES
  phase,
  binding,
  effect,
  queuedClose,
  lastAction,
  lastTarget,
  previousPhase,
  previousBinding,
  previousQueuedClose

vars ==
  <<phase, binding, effect, queuedClose, lastAction, lastTarget,
    previousPhase, previousBinding, previousQueuedClose>>

Record(action, target) ==
  /\ lastAction' = action
  /\ lastTarget' = target
  /\ previousPhase' = phase
  /\ previousBinding' = binding
  /\ previousQueuedClose' = queuedClose

Init ==
  /\ phase = [presentation \in Presentations |-> "absent"]
  /\ binding = [presentation \in Presentations |-> NoBinding]
  /\ effect = [presentation \in Presentations |-> "none"]
  /\ queuedClose = [presentation \in Presentations |-> FALSE]
  /\ lastAction = "init"
  /\ lastTarget = NoTarget
  /\ previousPhase = phase
  /\ previousBinding = binding
  /\ previousQueuedClose = queuedClose

FreshProvision(presentation, candidateBinding) ==
  /\ phase[presentation] = "absent"
  /\ effect[presentation] = "none"
  /\ \A owner \in Presentations: binding[owner] # candidateBinding
  /\ phase' = [phase EXCEPT ![presentation] = "bound"]
  /\ binding' = [binding EXCEPT ![presentation] = candidateBinding]
  /\ effect' = [effect EXCEPT ![presentation] = "provision"]
  /\ UNCHANGED queuedClose
  /\ Record("fresh-provision", presentation)

RetryProvision(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "none"
  /\ ~queuedClose[presentation]
  /\ effect' = [effect EXCEPT ![presentation] = "provision"]
  /\ UNCHANGED <<phase, binding, queuedClose>>
  /\ Record("retry-provision", presentation)

QueueClose(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "provision"
  /\ ~queuedClose[presentation]
  /\ queuedClose' = [queuedClose EXCEPT ![presentation] = TRUE]
  /\ UNCHANGED <<phase, binding, effect>>
  /\ Record("queue-close", presentation)

StartClose(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "none"
  /\ ~queuedClose[presentation]
  /\ effect' = [effect EXCEPT ![presentation] = "close"]
  /\ UNCHANGED <<phase, binding, queuedClose>>
  /\ Record("start-close", presentation)

FinishProvision(presentation, action) ==
  /\ action \in {"provision-completed", "provision-failed"}
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "provision"
  /\ effect' = [effect EXCEPT
       ![presentation] = IF queuedClose[presentation] THEN "close" ELSE "none"]
  /\ queuedClose' = [queuedClose EXCEPT ![presentation] = FALSE]
  /\ UNCHANGED <<phase, binding>>
  /\ Record(action, presentation)

CloseCompletes(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "close"
  /\ phase' = [phase EXCEPT ![presentation] = "retired"]
  /\ binding' = [binding EXCEPT ![presentation] = NoBinding]
  /\ effect' = [effect EXCEPT ![presentation] = "none"]
  /\ queuedClose' = [queuedClose EXCEPT ![presentation] = FALSE]
  /\ Record("close-completed", presentation)

CloseFails(presentation) ==
  /\ phase[presentation] = "bound"
  /\ effect[presentation] = "close"
  /\ effect' = [effect EXCEPT ![presentation] = "none"]
  /\ queuedClose' = [queuedClose EXCEPT ![presentation] = FALSE]
  /\ UNCHANGED <<phase, binding>>
  /\ Record("close-failed", presentation)

CloseUnbound(presentation) ==
  /\ phase[presentation] = "absent"
  /\ effect[presentation] = "none"
  /\ phase' = [phase EXCEPT ![presentation] = "retired"]
  /\ UNCHANGED <<binding, effect, queuedClose>>
  /\ Record("unbound-close", presentation)

Next ==
  \/ \E presentation \in Presentations, candidateBinding \in Bindings:
       FreshProvision(presentation, candidateBinding)
  \/ \E presentation \in Presentations: RetryProvision(presentation)
  \/ \E presentation \in Presentations: QueueClose(presentation)
  \/ \E presentation \in Presentations: StartClose(presentation)
  \/ \E presentation \in Presentations:
       FinishProvision(presentation, "provision-completed")
  \/ \E presentation \in Presentations:
       FinishProvision(presentation, "provision-failed")
  \/ \E presentation \in Presentations: CloseCompletes(presentation)
  \/ \E presentation \in Presentations: CloseFails(presentation)
  \/ \E presentation \in Presentations: CloseUnbound(presentation)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in [Presentations -> Phases]
  /\ binding \in [Presentations -> Bindings \cup {NoBinding}]
  /\ effect \in [Presentations -> Effects]
  /\ queuedClose \in [Presentations -> BOOLEAN]
  /\ lastAction \in Actions
  /\ lastTarget \in Presentations \cup {NoTarget}
  /\ previousPhase \in [Presentations -> Phases]
  /\ previousBinding \in [Presentations -> Bindings \cup {NoBinding}]
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
    queuedClose[presentation] => effect[presentation] = "provision"

TargetedMutationPreservesOthers ==
  lastTarget # NoTarget =>
    \A other \in Presentations \ {lastTarget}:
      /\ phase[other] = previousPhase[other]
      /\ binding[other] = previousBinding[other]

FailurePreservesMembership ==
  lastAction \in {"provision-failed", "close-failed"} =>
    /\ phase = previousPhase
    /\ binding = previousBinding

CloseCompletionRetiresTarget ==
  lastAction \in {"close-completed", "unbound-close"} =>
    /\ phase[lastTarget] = "retired"
    /\ binding[lastTarget] = NoBinding

QueuedCloseStartsAfterProvision ==
  /\ lastAction \in {"provision-completed", "provision-failed"}
  /\ previousQueuedClose[lastTarget]
  =>
    /\ effect[lastTarget] = "close"
    /\ ~queuedClose[lastTarget]

=============================================================================
