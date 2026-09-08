---------------------- MODULE TerminalPresentationStop ----------------------
EXTENDS FiniteSets, Naturals, TLC

Presentations == {"visible", "hidden"}
Intents == {"retain", "stop", "stopped"}
Phases == {"running", "stopping", "stop-failed", "stopped"}
BindingStates == {"present", "absent"}
WorkflowPhases == {"idle", "persisting", "closing", "failed", "complete"}
Actions == {
  "init", "request-stop", "commit-stop", "reject-stop", "dispatch-close",
  "close-completed", "close-failed", "retry-close", "finish-stop",
  "start-fresh"
}
None == "none"
MaxAttempt == 2

VARIABLES
  intent,
  phase,
  binding,
  attempt,
  closeIssued,
  sealed,
  workflow,
  writable,
  identityGeneration,
  lastAction,
  lastTarget,
  previousIntent,
  previousPhase,
  previousBinding,
  previousAttempt,
  previousCloseIssued,
  previousSealed,
  previousWorkflow,
  previousIdentityGeneration

vars ==
  <<intent, phase, binding, attempt, closeIssued, sealed, workflow, writable,
    identityGeneration, lastAction, lastTarget, previousIntent, previousPhase,
    previousBinding, previousAttempt, previousCloseIssued, previousSealed,
    previousWorkflow, previousIdentityGeneration>>

Record(action, target) ==
  /\ lastAction' = action
  /\ lastTarget' = target
  /\ previousIntent' = intent
  /\ previousPhase' = phase
  /\ previousBinding' = binding
  /\ previousAttempt' = attempt
  /\ previousCloseIssued' = closeIssued
  /\ previousSealed' = sealed
  /\ previousWorkflow' = workflow
  /\ previousIdentityGeneration' = identityGeneration

Init ==
  /\ intent = [presentation \in Presentations |-> "retain"]
  /\ phase = [presentation \in Presentations |-> "running"]
  /\ binding = [presentation \in Presentations |-> "present"]
  /\ attempt = [presentation \in Presentations |-> 0]
  /\ closeIssued = [presentation \in Presentations |-> FALSE]
  /\ sealed = FALSE
  /\ workflow = "idle"
  /\ writable \in BOOLEAN
  /\ identityGeneration = [presentation \in Presentations |-> 0]
  /\ lastAction = "init"
  /\ lastTarget = None
  /\ previousIntent = intent
  /\ previousPhase = phase
  /\ previousBinding = binding
  /\ previousAttempt = attempt
  /\ previousCloseIssued = closeIssued
  /\ previousSealed = sealed
  /\ previousWorkflow = workflow
  /\ previousIdentityGeneration = identityGeneration

RequestStop ==
  /\ workflow = "idle"
  /\ sealed' = TRUE
  /\ phase' = [presentation \in Presentations |-> "stopping"]
  /\ attempt' = [presentation \in Presentations |-> 1]
  /\ workflow' = "persisting"
  /\ UNCHANGED
       <<intent, binding, closeIssued, writable, identityGeneration>>
  /\ Record("request-stop", None)

CommitStop ==
  /\ workflow = "persisting"
  /\ writable
  /\ intent' = [presentation \in Presentations |-> "stop"]
  /\ workflow' = "closing"
  /\ UNCHANGED
       <<phase, binding, attempt, closeIssued, sealed, writable,
         identityGeneration>>
  /\ Record("commit-stop", None)

RejectStopCommit ==
  /\ workflow = "persisting"
  /\ ~writable
  /\ phase' = [presentation \in Presentations |-> "running"]
  /\ attempt' = [presentation \in Presentations |-> 0]
  /\ sealed' = FALSE
  /\ workflow' = "idle"
  /\ UNCHANGED
       <<intent, binding, closeIssued, writable, identityGeneration>>
  /\ Record("reject-stop", None)

DispatchClose(presentation) ==
  /\ workflow \in {"closing", "failed"}
  /\ phase[presentation] = "stopping"
  /\ intent[presentation] = "stop"
  /\ ~closeIssued[presentation]
  /\ closeIssued' = [closeIssued EXCEPT ![presentation] = TRUE]
  /\ UNCHANGED
       <<intent, phase, binding, attempt, sealed, workflow, writable,
         identityGeneration>>
  /\ Record("dispatch-close", presentation)

CloseCompletes(presentation, candidateAttempt) ==
  /\ candidateAttempt \in 1..MaxAttempt
  /\ IF /\ phase[presentation] = "stopping"
        /\ attempt[presentation] = candidateAttempt
        /\ closeIssued[presentation]
     THEN
       /\ phase' = [phase EXCEPT ![presentation] = "stopped"]
       /\ binding' = [binding EXCEPT ![presentation] = "absent"]
       /\ closeIssued' = [closeIssued EXCEPT ![presentation] = FALSE]
     ELSE
       /\ UNCHANGED <<phase, binding, closeIssued>>
  /\ UNCHANGED
       <<intent, attempt, sealed, workflow, writable, identityGeneration>>
  /\ Record("close-completed", presentation)

CloseFails(presentation, candidateAttempt) ==
  /\ candidateAttempt \in 1..MaxAttempt
  /\ phase[presentation] = "stopping"
  /\ attempt[presentation] = candidateAttempt
  /\ closeIssued[presentation]
  /\ phase' = [phase EXCEPT ![presentation] = "stop-failed"]
  /\ closeIssued' = [closeIssued EXCEPT ![presentation] = FALSE]
  /\ workflow' = "failed"
  /\ UNCHANGED
       <<intent, binding, attempt, sealed, writable, identityGeneration>>
  /\ Record("close-failed", presentation)

FailedPresentations ==
  {presentation \in Presentations: phase[presentation] = "stop-failed"}

RetryableFailedPresentations ==
  {presentation \in FailedPresentations:
     attempt[presentation] < MaxAttempt}

RetryClose ==
  /\ workflow = "failed"
  /\ RetryableFailedPresentations # {}
  /\ phase' =
       [presentation \in Presentations |->
         IF presentation \in RetryableFailedPresentations
         THEN "stopping"
         ELSE phase[presentation]]
  /\ attempt' =
       [presentation \in Presentations |->
         IF presentation \in RetryableFailedPresentations
         THEN attempt[presentation] + 1
         ELSE attempt[presentation]]
  /\ closeIssued' =
       [presentation \in Presentations |->
         IF presentation \in RetryableFailedPresentations
         THEN FALSE
         ELSE closeIssued[presentation]]
  /\ workflow' = "closing"
  /\ UNCHANGED
       <<intent, binding, sealed, writable, identityGeneration>>
  /\ Record("retry-close", None)

FinishStop ==
  /\ workflow \in {"closing", "failed"}
  /\ \A presentation \in Presentations:
       phase[presentation] = "stopped" /\ binding[presentation] = "absent"
  /\ intent' = [presentation \in Presentations |-> "stopped"]
  /\ workflow' = "complete"
  /\ UNCHANGED
       <<phase, binding, attempt, closeIssued, sealed, writable,
         identityGeneration>>
  /\ Record("finish-stop", None)

StartFresh(presentation) ==
  /\ workflow = "complete"
  /\ phase[presentation] = "stopped"
  /\ intent[presentation] = "stopped"
  /\ phase' = [phase EXCEPT ![presentation] = "running"]
  /\ intent' = [intent EXCEPT ![presentation] = "retain"]
  /\ binding' = [binding EXCEPT ![presentation] = "present"]
  /\ identityGeneration' =
       [identityGeneration EXCEPT ![presentation] = @ + 1]
  /\ UNCHANGED
       <<attempt, closeIssued, sealed, workflow, writable>>
  /\ Record("start-fresh", presentation)

Next ==
  \/ RequestStop
  \/ CommitStop
  \/ RejectStopCommit
  \/ FinishStop
  \/ RetryClose
  \/ \E presentation \in Presentations: DispatchClose(presentation)
  \/ \E presentation \in Presentations: StartFresh(presentation)
  \/ \E presentation \in Presentations, candidateAttempt \in 1..MaxAttempt:
       CloseCompletes(presentation, candidateAttempt)
  \/ \E presentation \in Presentations, candidateAttempt \in 1..MaxAttempt:
       CloseFails(presentation, candidateAttempt)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ intent \in [Presentations -> Intents]
  /\ phase \in [Presentations -> Phases]
  /\ binding \in [Presentations -> BindingStates]
  /\ attempt \in [Presentations -> 0..MaxAttempt]
  /\ closeIssued \in [Presentations -> BOOLEAN]
  /\ sealed \in BOOLEAN
  /\ workflow \in WorkflowPhases
  /\ writable \in BOOLEAN
  /\ identityGeneration \in [Presentations -> Nat]
  /\ lastAction \in Actions
  /\ lastTarget \in Presentations \cup {None}
  /\ previousIntent \in [Presentations -> Intents]
  /\ previousPhase \in [Presentations -> Phases]
  /\ previousBinding \in [Presentations -> BindingStates]
  /\ previousAttempt \in [Presentations -> 0..MaxAttempt]
  /\ previousCloseIssued \in [Presentations -> BOOLEAN]
  /\ previousSealed \in BOOLEAN
  /\ previousWorkflow \in WorkflowPhases
  /\ previousIdentityGeneration \in [Presentations -> Nat]

CloseRequiresDurableStop ==
  lastAction = "dispatch-close" =>
    /\ lastTarget \in Presentations
    /\ previousIntent[lastTarget] = "stop"
    /\ previousPhase[lastTarget] = "stopping"
    /\ previousBinding[lastTarget] = "present"
    /\ ~previousCloseIssued[lastTarget]
    /\ closeIssued[lastTarget]
    /\ intent = previousIntent
    /\ phase = previousPhase
    /\ binding = previousBinding
    /\ attempt = previousAttempt
    /\ sealed = previousSealed
    /\ workflow = previousWorkflow
    /\ identityGeneration = previousIdentityGeneration
    /\ \A presentation \in Presentations \ {lastTarget}:
         closeIssued[presentation] = previousCloseIssued[presentation]

RetryCoversEveryFailedPresentation ==
  lastAction = "retry-close" =>
    /\ lastTarget = None
    /\ previousWorkflow = "failed"
    /\ {presentation \in Presentations:
          /\ previousPhase[presentation] = "stop-failed"
          /\ previousAttempt[presentation] < MaxAttempt} # {}
    /\ \A presentation \in Presentations:
         IF /\ previousPhase[presentation] = "stop-failed"
            /\ previousAttempt[presentation] < MaxAttempt
         THEN
           /\ phase[presentation] = "stopping"
           /\ attempt[presentation] = previousAttempt[presentation] + 1
           /\ ~closeIssued[presentation]
         ELSE
           /\ phase[presentation] = previousPhase[presentation]
           /\ attempt[presentation] = previousAttempt[presentation]
           /\ closeIssued[presentation] = previousCloseIssued[presentation]
    /\ intent = previousIntent
    /\ binding = previousBinding
    /\ sealed = previousSealed
    /\ workflow = "closing"
    /\ identityGeneration = previousIdentityGeneration

StoppedHasNoBinding ==
  \A presentation \in Presentations:
    phase[presentation] = "stopped" => binding[presentation] = "absent"

FreshStartChangesIdentity ==
  lastAction = "start-fresh" =>
    /\ lastTarget \in Presentations
    /\ previousIntent[lastTarget] = "stopped"
    /\ previousPhase[lastTarget] = "stopped"
    /\ previousBinding[lastTarget] = "absent"
    /\ intent[lastTarget] = "retain"
    /\ phase[lastTarget] = "running"
    /\ binding[lastTarget] = "present"
    /\ identityGeneration[lastTarget] =
         previousIdentityGeneration[lastTarget] + 1
    /\ attempt = previousAttempt
    /\ closeIssued = previousCloseIssued
    /\ sealed = previousSealed
    /\ workflow = previousWorkflow
    /\ \A presentation \in Presentations \ {lastTarget}:
         /\ intent[presentation] = previousIntent[presentation]
         /\ phase[presentation] = previousPhase[presentation]
         /\ binding[presentation] = previousBinding[presentation]
         /\ identityGeneration[presentation] =
              previousIdentityGeneration[presentation]

FailedCommitHasNoCloseEffect ==
  lastAction = "reject-stop" =>
    /\ lastTarget = None
    /\ previousWorkflow = "persisting"
    /\ previousSealed
    /\ intent = previousIntent
    /\ binding = previousBinding
    /\ closeIssued = previousCloseIssued
    /\ identityGeneration = previousIdentityGeneration
    /\ \A presentation \in Presentations:
         /\ previousIntent[presentation] = "retain"
         /\ intent[presentation] = "retain"
         /\ previousPhase[presentation] = "stopping"
         /\ previousAttempt[presentation] = 1
         /\ previousBinding[presentation] = "present"
         /\ binding[presentation] = "present"
         /\ ~previousCloseIssued[presentation]
         /\ ~closeIssued[presentation]
         /\ phase[presentation] = "running"
         /\ attempt[presentation] = 0
    /\ ~sealed
    /\ workflow = "idle"

=============================================================================
